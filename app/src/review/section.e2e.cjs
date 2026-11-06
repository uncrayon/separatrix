// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5611/";

async function open(viewport = { width: 1440, height: 900 }) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport });
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

const last = (page) => page.evaluate(() => window.__sepxMock.lastPortrait);
const viewGroup = (page) => page.locator(".panel-right [role=radiogroup][aria-label=View]");
const viewRadio = (page, name) => viewGroup(page).getByRole("radio", { name, exact: true });
const inspector = (page) => page.locator(".panel-right");

async function waitPortrait(page, pred) {
  await page.waitForFunction(
    (src) => {
      const p = window.__sepxMock?.lastPortrait;
      return !!p && new Function("req", "res", `return (${src});`)(p.req, p.res);
    },
    pred,
    { timeout: 15000 },
  );
}

async function toPx(page, x, y) {
  const { req } = await last(page);
  const box = await page.locator("canvas.plot-canvas").boundingBox();
  const [x0, x1] = req.xRange;
  const [y0, y1] = req.yRange;
  return { x: box.x + ((x - x0) / (x1 - x0)) * box.width, y: box.y + (1 - (y - y0) / (y1 - y0)) * box.height };
}

async function commit(locator, value) {
  await locator.click();
  await locator.fill(value);
  await locator.press("Enter");
}

async function lorenzZ27(page) {
  await pick(page, "Lorenz equations");
  assert.deepEqual(await viewGroup(page).getByRole("radio").allInnerTexts(), ["Phase", "Series", "Section", "3D", "λ"]);
  await viewRadio(page, "Poincaré section").click();
  await waitPortrait(page, "!!req.section");
  await inspector(page).getByLabel("State", { exact: true }).selectOption("s2");
  await commit(inspector(page).getByRole("textbox", { name: "Value of the section plane" }), "27");
  await commit(inspector(page).getByRole("textbox", { name: /^Time span T/ }), "500");
  await commit(inspector(page).getByLabel("Crossings skipped at the start of each orbit", { exact: true }), "10");
  await waitPortrait(
    page,
    "req.section.kind === 'plane' && req.section.state === 2 && req.section.value === 27 && req.section.direction === 'rising' && req.transient === 10 && req.trajectories[0].t1 === 500",
  );
}

test("Lorenz: the section z = 27 crossed upward is a two-lobed set of at least 20 dots", async () => {
  const { browser, page } = await open();
  try {
    await lorenzZ27(page);
    const { req, res } = await last(page);
    assert.deepEqual([req.xAxis, req.yAxis], [{ kind: "state", index: 0 }, { kind: "state", index: 1 }], "x and y across and up");
    assert.equal(req.fieldGrid, 0);
    assert.equal(req.nullclines, false);
    const tr = res.trajectories[0];
    const n = tr.t.length;
    assert.ok(n >= 20, `${n} crossings`);
    const xs = [];
    for (let k = 0; k < n; k++) {
      assert.ok(Math.abs(tr.x[3 * k + 2] - 27) < 1e-9, `crossing ${k} is on z = 27`);
      xs.push(tr.x[3 * k]);
    }
    assert.ok(xs.some((x) => x > 1) && xs.some((x) => x < -1), "both wings of the butterfly");

    const canvas = page.locator("canvas.plot-canvas");
    assert.equal(await canvas.getAttribute("aria-roledescription"), "Poincaré section");
    assert.match(await canvas.getAttribute("aria-label"), /^Poincaré section: x from .* across, y from .* up\./);
    const count = page.locator(".statusbar [data-section-count]");
    assert.equal((await count.innerText()).replace(/\s+/g, " "), `${n.toLocaleString("en-US")} crossings, first 10 per orbit skipped`);
    assert.equal(await page.locator(".plot-toolbar [role=group][aria-label=Layers]").count(), 0);
    assert.equal(await inspector(page).getByRole("switch", { name: "Fixed points" }).isDisabled(), true);
    assert.equal(await inspector(page).getByText("Not searched: a Poincaré section shows only the crossings.").count(), 1);
    assert.match(await inspector(page).locator(".ds-panel-section-hint", { hasText: "Dots where" }).innerText(), /^Dots where an orbit crosses z = 27 upward, after the first 10 ones\.$/);
  } finally {
    await browser.close();
  }
});

test("a map shows Phase, Series and Cobweb, and a one-state flow's plane section says why it can't", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Hénon map");
    assert.deepEqual(await viewGroup(page).getByRole("radio").allInnerTexts(), ["Phase", "Series", "Cobweb", "λ", "Orbit"]);
    const cobweb = viewRadio(page, "Cobweb");
    assert.equal(await cobweb.getAttribute("aria-disabled"), "true");
    assert.match(await page.locator(`[id="${await cobweb.getAttribute("aria-describedby")}"]`).textContent(), /; Hénon map has 2 states\.$/);
    await page.getByRole("button", { name: /^Axes:/ }).click();
    const labels = await page.getByRole("menu").locator("[data-view-choice] .axes-menu-label").allInnerTexts();
    assert.deepEqual(labels, ["Phase plane", "Time series", "Cobweb", "Lyapunov spectrum", "Orbit diagram"]);
    await page.keyboard.press("Escape");

    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    await pick(page, "Slope field");
    const phase = viewRadio(page, "Phase plane");
    assert.equal(await phase.getAttribute("aria-disabled"), "true");
    assert.equal(await page.locator(`[id="${await phase.getAttribute("aria-describedby")}"]`).innerText(), "A phase plane needs at least 2 states. Slope field has 1.");
    await viewRadio(page, "Poincaré section").click();
    await waitPortrait(page, "req.section && req.section.kind === 'stroboscopic'");
    assert.ok(Math.abs((await last(page)).req.section.period - 2 * Math.PI) < 1e-15);
    const planeSeg = inspector(page).getByRole("radiogroup", { name: "Section kind" }).getByRole("radio", { name: "Plane" });
    assert.equal(await planeSeg.getAttribute("aria-disabled"), "true");
    assert.equal(
      await page.locator(`[id="${await planeSeg.getAttribute("aria-describedby")}"]`).innerText(),
      "A plane section needs at least 2 states. Slope field has 1; take a stroboscopic section.",
    );
  } finally {
    await browser.close();
  }
});

test("a click starts an orbit on the section; the return map pairs crossings and adds none", async () => {
  const { browser, page } = await open();
  try {
    await lorenzZ27(page);
    const before = (await last(page)).req.trajectories.length;
    const at = await toPx(page, 5, -4);
    await page.mouse.click(at.x, at.y);
    await waitPortrait(page, `req.trajectories.length === ${before + 1}`);
    const { req, res } = await last(page);
    const added = req.trajectories[before];
    assert.ok(Math.abs(added.x0[0] - 5) < 0.2 && Math.abs(added.x0[1] + 4) < 0.2, `x0 ${added.x0}`);
    assert.equal(added.x0[2], 27, "on the plane exactly");
    assert.ok(res.trajectories[before].t.length >= 20, "the new orbit crosses too");

    await inspector(page).getByLabel("Vertical", { exact: true }).selectOption("s0");
    await waitPortrait(page, "req.yAxis.kind === 'state' && req.yAxis.index === 0 && req.xAxis.index === 0");
    assert.match(await page.getByRole("button", { name: /^Section:/ }).getAttribute("aria-label"), /^Section: x k horizontal, x k\+1 vertical$/);
    assert.match(await page.locator(".plot-wrap .axis-label-y").innerText(), /x sub k plus 1/);
    const shown = (label) => inspector(page).getByLabel(label, { exact: true }).evaluate((s) => s.selectedOptions[0].textContent);
    assert.deepEqual([await shown("Horizontal"), await shown("Vertical")], ["xₖ", "xₖ₊₁"]);
    const n = (await last(page)).req.trajectories.length;
    const mid = await toPx(page, 0, 0);
    await page.mouse.click(mid.x, mid.y);
    await page.waitForTimeout(500);
    assert.equal((await last(page)).req.trajectories.length, n, "no start point added on a return map");
  } finally {
    await browser.close();
  }
});

test("export: a section's SVG is one dot per crossing, without field or nullcline options, and says crossings", async () => {
  const { browser, page } = await open();
  try {
    await lorenzZ27(page);
    await page.getByRole("button", { name: /^Export/ }).first().click();
    await page.waitForSelector(".export-dialog[role=dialog]");
    const d = page.locator(".export-dialog");
    await d.getByRole("radio", { name: "SVG", exact: true }).click();
    assert.equal(await d.getByRole("checkbox", { name: /direction field/ }).count(), 0);
    assert.equal(await d.getByRole("checkbox", { name: /nullclines/ }).count(), 0);
    assert.match(await d.getByText(/Include fixed points/).innerText(), /none: a Poincaré section shows only the crossings/);
    assert.match(await page.locator(".ds-export-sub").innerText(), /^Lorenz equations, Poincaré section x–y, as you see it now$/);
    await page.waitForFunction(() => !document.querySelector(".ds-export-actions .ds-button--primary")?.disabled);
    await page.waitForTimeout(800);
    await d.locator(".ds-export-actions .ds-button--primary").click();
    await page.waitForFunction(() => window.__sepxMock.saved.length === 1);
    const { desc, dots } = await page.evaluate(() => {
      const doc = new DOMParser().parseFromString(window.__sepxMock.saved[0].contents, "image/svg+xml");
      const dots = {};
      for (const p of doc.querySelectorAll("path[fill]")) {
        const arcs = (p.getAttribute("d").match(/A/g) ?? []).length / 2;
        if (arcs) dots[p.getAttribute("fill").toLowerCase()] = (dots[p.getAttribute("fill").toLowerCase()] ?? 0) + arcs;
      }
      for (const r of doc.querySelectorAll("rect[fill]")) {
        const f = r.getAttribute("fill").toLowerCase();
        if (f !== "#ffffff") dots[f] = (dots[f] ?? 0) + 1;
      }
      return { desc: doc.querySelector("svg > desc")?.textContent ?? null, dots };
    });
    const crossings = (await last(page)).res.trajectories.map((t) => t.t.length);
    const total = crossings.reduce((a, b) => a + b, 0);
    assert.match(desc, new RegExp(`^Lorenz equations with ${total.toLocaleString("en-US").replace(",", ",?")} crossings$`));
    assert.deepEqual(Object.values(dots).sort((a, b) => a - b), [...crossings].sort((a, b) => a - b), "one dot per crossing, per orbit");
  } finally {
    await browser.close();
  }
});
