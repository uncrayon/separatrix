// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5613/";
const FIXTURE = fs.readFileSync(path.join(__dirname, "../../../crates/project/tests/fixtures/orbit-view.sepx"), "utf8");

async function open() {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

async function pick(page, title) {
  await page.locator(".gallery .card", { hasText: title }).first().click();
  await page.waitForSelector(".gallery", { state: "detached" });
  await page.waitForTimeout(600);
}

async function commit(locator, value) {
  await locator.click();
  await locator.fill(value);
  await locator.press("Enter");
}

/** Waits for a `compute_orbit_diagram` whose request and response satisfy `pred` (a JS expression in req, res). */
async function waitDiagram(page, pred) {
  await page.waitForFunction(
    (src) => {
      const l = window.__sepxMock?.lastOrbit;
      return !!l && new Function("req", "res", `return (${src});`)(l.req, l.res);
    },
    pred,
    { timeout: 30000 },
  );
  // The canvas draws on the next frame after the response.
  await page.waitForTimeout(300);
  return page.evaluate(() => window.__sepxMock.lastOrbit);
}

const inspector = (page) => page.locator(".panel-right");
const viewRadio = (page, name) =>
  inspector(page).locator("[role=radiogroup][aria-label=View]").getByRole("radio", { name, exact: true });
const doublings = (page) =>
  inspector(page)
    .locator("[data-orbit-doublings] .ds-panel-section-kv-row")
    .evaluateAll((rows) => rows.map((r) => [r.querySelector("dt").innerText.trim(), r.querySelector("dd").innerText.trim()]));

/**
 * Clusters of inked pixels (far from the plot background) down the canvas
 * column at parameter `r`, over x in [yLo, yHi]: runs of inked pixels, gaps of
 * up to 2 px merged.
 */
async function bandsAt(page, r, [yLo, yHi]) {
  return page.evaluate(
    ({ r, yLo, yHi }) => {
      const canvas = document.querySelector(".orbit-canvas");
      const ctx = canvas.getContext("2d");
      const css = getComputedStyle(document.documentElement);
      const probe = document.createElement("canvas").getContext("2d");
      probe.fillStyle = css.getPropertyValue("--plot-bg").trim();
      probe.fillRect(0, 0, 1, 1);
      const bg = probe.getImageData(0, 0, 1, 1).data;
      const v = window.__orbitTestView;
      const W = canvas.width;
      const H = canvas.height;
      const px = Math.floor(((r - v.xRange[0]) / (v.xRange[1] - v.xRange[0])) * W);
      const row = (y) => Math.floor((1 - (y - v.yRange[0]) / (v.yRange[1] - v.yRange[0])) * H);
      const { data } = ctx.getImageData(px, 0, 1, H);
      let clusters = 0;
      let gap = Infinity;
      for (let y = row(yHi); y <= row(yLo); y++) {
        const d = Math.abs(data[4 * y] - bg[0]) + Math.abs(data[4 * y + 1] - bg[1]) + Math.abs(data[4 * y + 2] - bg[2]);
        if (d > 90) {
          if (gap > 2) clusters++;
          gap = 0;
        } else gap++;
      }
      return clusters;
    },
    { r, yLo, yHi },
  );
}

async function openLogisticDiagram(page) {
  await pick(page, "Logistic map");
  await viewRadio(page, "Orbit diagram").click();
  await waitDiagram(page, "req.range[0] < 0.01");
  await commit(inspector(page).getByRole("textbox", { name: "First value of r" }), "2.8");
  return waitDiagram(page, "req.range[0] > 2.8");
}

test("the logistic map's orbit diagram shows the period-doubling route and lists r₁ … r₄", async () => {
  const { browser, page } = await open();
  try {
    const { req, res } = await openLogisticDiagram(page);
    const width = await page.locator(".orbit-canvas").evaluate((c) => c.width);
    assert.equal(req.columns, width, "one column per device pixel");
    const half = 1.2 / (2 * width);
    assert.ok(Math.abs(req.range[0] - (2.8 + half)) < 1e-12 && Math.abs(req.range[1] - (4 - half)) < 1e-12);
    assert.deepEqual([req.transient, req.kept, req.doublings, req.x0], [1000, 200, true, [0.1]]);
    assert.equal(res.doublings.length >= 4, true);

    assert.deepEqual((await doublings(page)).slice(0, 4), [
      ["r₁", "3.000000"],
      ["r₂", "3.449490"],
      ["r₃", "3.544090"],
      ["r₄", "3.564407"],
    ]);

    // The marker sits at r = 3.2 now; move it out of the columns the bands are counted in.
    await commit(page.getByRole("textbox", { name: "Value of r", exact: true }), "2.9");
    await page.waitForTimeout(300);
    await page.evaluate(() => {
      const v = { xRange: [2.8, 4], yRange: [-0.05, 1.05] };
      window.__orbitTestView = v;
    });
    // Strogatz §10.2: a 2-cycle at r = 3.2, a 4-cycle at 3.5, the period-3 window at 3.83.
    const band = [0.05, 1.0];
    assert.equal(await bandsAt(page, 3.2, band), 2);
    assert.equal(await bandsAt(page, 3.5, band), 4);
    assert.equal(await bandsAt(page, 3.83, band), 3);

    const status = await page.locator(".statusbar").innerText();
    assert.match(status, /Orbit \d/);
    assert.match(status, new RegExp(`${width.toLocaleString("en-US")} columns · 200 iterates each`));
  } finally {
    await browser.close();
  }
});

test("clicking the diagram sets r to its column, and zooming asks for the new range", async () => {
  const { browser, page } = await open();
  try {
    const { req } = await openLogisticDiagram(page);
    const canvas = page.locator(".orbit-canvas");
    const box = await canvas.boundingBox();
    const at = (r) => box.x + ((r - 2.8) / 1.2) * box.width;
    await page.mouse.click(at(3.83), box.y + box.height / 2);
    const value = Number(await page.getByRole("textbox", { name: "Value of r", exact: true }).inputValue());
    const column = 1.2 / req.columns;
    assert.ok(Math.abs(value - 3.83) <= column, `r = ${value}, within one column (${column}) of 3.83`);
    // The swept parameter's own value is not part of the request.
    await page.waitForTimeout(400);
    assert.equal(await page.evaluate(() => window.__sepxMock.lastOrbit.req.range[0]), req.range[0]);

    // A drag pans, it doesn't set r.
    await page.mouse.move(at(3.0), box.y + 200);
    await page.mouse.down();
    await page.mouse.move(at(3.1), box.y + 200, { steps: 4 });
    await page.mouse.up();
    assert.equal(Number(await page.getByRole("textbox", { name: "Value of r", exact: true }).inputValue()), value);
    const panned = await waitDiagram(page, `req.range[0] < ${req.range[0] - 0.05}`);
    assert.equal(panned.req.columns, req.columns);

    await page.mouse.move(at(3.5), box.y + box.height / 2);
    await page.mouse.wheel(0, -400);
    const zoomed = await waitDiagram(page, `req.range[1] - req.range[0] < ${0.8 * (req.range[1] - req.range[0])}`);
    assert.equal(zoomed.req.columns, req.columns, "the same columns over a narrower range");
  } finally {
    await browser.close();
  }
});

test("a 2-D map gets a diagram of its chosen state and parameter, without doublings", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Hénon map");
    await viewRadio(page, "Orbit diagram").click();
    const { req } = await waitDiagram(page, "req.param === 0");
    assert.equal(req.doublings, false);
    assert.deepEqual([req.range[0] < 0.01, req.range[1] > 1.49], [true, true], "a over its slider's range [0, 1.5]");
    assert.equal(await inspector(page).locator("[data-orbit-doublings]").count(), 0);
    await inspector(page).getByRole("combobox", { name: "Vertical" }).selectOption({ label: "y" });
    await waitDiagram(page, "req.state === 1");
    await inspector(page).getByRole("combobox", { name: "Horizontal" }).selectOption({ label: "b" });
    const b = await waitDiagram(page, "req.param === 1");
    assert.ok(b.req.range[0] < -0.49 && b.req.range[1] > 0.49, "b over its slider's range [−0.5, 0.5]");
  } finally {
    await browser.close();
  }
});

test("the saved orbit diagram opens over its range, and saving writes it back", async () => {
  const { browser, page } = await open();
  try {
    await page.evaluate((text) => window.__sepxMock.files.set("/mock/orbit-view.sepx", text), FIXTURE);
    await pick(page, "Lorenz");
    await page.evaluate(() => (window.__sepxMock.nextOpenPath = "/mock/orbit-view.sepx"));
    await page.getByRole("button", { name: "Open…" }).click();
    const { req } = await waitDiagram(page, "req.range[0] > 2.8 && req.range[1] < 4");
    assert.deepEqual([req.transient, req.kept], [1000, 200]);
    assert.equal(await viewRadio(page, "Orbit diagram").getAttribute("aria-checked"), "true");
    assert.equal(await page.evaluate(() => window.__sepxMock.document.dirty), false);
    await page.evaluate(() => (window.__sepxMock.nextSavePath = "/mock/out.sepx"));
    await page.getByRole("button", { name: "Save as…" }).click();
    await page.waitForFunction(() => window.__sepxMock.files.has("/mock/out.sepx"));
    const out = JSON.parse(await page.evaluate(() => window.__sepxMock.files.get("/mock/out.sepx")));
    const saved = JSON.parse(FIXTURE).views[0];
    const v = out.views[0];
    assert.equal(v.kind, "orbit");
    assert.deepEqual([v.x_axis, v.y_axis, v.x_range, v.orbit], [saved.x_axis, saved.y_axis, saved.x_range, saved.orbit]);
  } finally {
    await browser.close();
  }
});
