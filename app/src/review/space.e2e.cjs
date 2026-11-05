// The 3D view in the browser preview (mocked core): Lorenz in WebGL turns
// under a drag and labels its axes, the Canvas2D fallback when WebGL is
// unavailable, the reason Van der Pol has no 3D view, and the time a frame
// of dragging takes with 8 trajectories of 20,000 samples.
// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5612/";

/** Records every string the 2D canvases fill, so the axis names can be read back. */
function recordText() {
  window.__filled = [];
  const fill = CanvasRenderingContext2D.prototype.fillText;
  CanvasRenderingContext2D.prototype.fillText = function (text, ...rest) {
    window.__filled.push(String(text));
    return fill.call(this, text, ...rest);
  };
}

/** A browser without WebGL: every WebGL context request fails, as on a machine without a GPU driver. */
function noWebGl() {
  const get = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function (type, ...rest) {
    return /webgl/.test(type) ? null : get.call(this, type, ...rest);
  };
}

async function open({ webgl = true } = {}) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await page.addInitScript(recordText);
    if (!webgl) await page.addInitScript(noWebGl);
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
const overlay = (page) => page.locator("canvas.space-overlay");

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

async function lorenz3d(page) {
  await pick(page, "Lorenz equations");
  assert.deepEqual(await viewGroup(page).getByRole("radio").allInnerTexts(), ["Phase", "Series", "Section", "3D"]);
  await viewRadio(page, "3D view").click();
  await overlay(page).waitFor();
  await waitPortrait(page, "req.zAxis && req.zAxis.index === 2");
  await page.waitForTimeout(400);
}

/** The plot's pixels (a PNG of the plot well). */
const shot = (page) => page.locator(".space-wrap").screenshot();

/** How many bytes two same-size PNG screenshots differ in (the PNGs differ where the pixels do). */
function differs(a, b) {
  let n = Math.abs(a.length - b.length);
  for (let i = 0; i < Math.min(a.length, b.length); i++) if (a[i] !== b[i]) n++;
  return n;
}

test("Lorenz in 3D: a 100 px drag turns the box, the axes are named x, y and z, and the camera asks the core for nothing", async () => {
  const { browser, page } = await open();
  try {
    await lorenz3d(page);
    assert.equal(await page.locator("canvas.space-gl").isVisible(), true, "drawn with WebGL");
    assert.equal(await page.getByText("drawn without WebGL").count(), 0);
    const { req } = await last(page);
    assert.deepEqual([req.xAxis, req.yAxis, req.zAxis], [0, 1, 2].map((index) => ({ kind: "state", index })));
    assert.ok(req.zRange[0] < 27 && req.zRange[1] > 27, `z range ${req.zRange} holds C±`);
    assert.equal(req.fieldGrid, 0);
    assert.equal(req.nullclines, false);
    assert.equal(req.fixedPoints, true);
    const fps = (await last(page)).res.fixedPoints;
    const c = Math.sqrt((8 / 3) * 27);
    for (const [x, y, z] of [[0, 0, 0], [c, c, 27], [-c, -c, 27]]) {
      assert.ok(fps.some((fp) => Math.hypot(fp.x[0] - x, fp.x[1] - y, fp.x[2] - z) < 1e-6), `fixed point (${x}, ${y}, ${z})`);
    }

    const filled = await page.evaluate(() => window.__filled);
    for (const name of ["x", "y", "z"]) assert.ok(filled.includes(name), `axis name ${name} drawn`);

    const before = await shot(page);
    const id = req.requestId;
    const box = await overlay(page).boundingBox();
    const [cx, cy] = [box.x + box.width / 2, box.y + box.height / 2];
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.mouse.move(cx + 100, cy, { steps: 10 });
    await page.mouse.up();
    await page.waitForTimeout(300);
    const after = await shot(page);
    assert.ok(differs(before, after) > 1000, "the drawing changed");
    assert.equal((await last(page)).req.requestId, id, "no new portrait for a turn of the camera");

    // Arrow keys turn it too; 0 puts it back.
    await overlay(page).focus();
    await page.keyboard.press("ArrowLeft");
    await page.waitForTimeout(150);
    const keyed = await shot(page);
    assert.ok(differs(after, keyed) > 1000, "an arrow key turned the box");
    await page.keyboard.press("0");
    await overlay(page).evaluate((el) => el.blur());
    await page.waitForTimeout(150);
    assert.equal(differs(before, await shot(page)), 0, "0 restores the first camera");
  } finally {
    await browser.close();
  }
});

test("without WebGL the 3D view says so and draws the lines on the 2D canvas", async () => {
  const { browser, page } = await open({ webgl: false });
  try {
    await lorenz3d(page);
    await page.getByText("This 3D view is drawn without WebGL, so the trajectories are redrawn when you stop dragging.").waitFor();
    assert.equal(await page.locator("canvas.space-gl").isVisible(), false);
    // Pixels in the first trajectory colour: the lines are on the 2D canvas.
    const traj = await overlay(page).evaluate((el) => {
      const css = getComputedStyle(document.documentElement);
      const probe = document.createElement("canvas").getContext("2d");
      const rgb = (c) => {
        probe.fillStyle = c;
        probe.fillRect(0, 0, 1, 1);
        return [...probe.getImageData(0, 0, 1, 1).data.slice(0, 3)];
      };
      const [r, g, b] = rgb(css.getPropertyValue("--traj-1").trim());
      const [br, bg, bb] = rgb(css.getPropertyValue("--plot-bg").trim());
      const d = el.getContext("2d").getImageData(0, 0, el.width, el.height).data;
      let line = 0;
      let drawn = 0;
      for (let i = 0; i < d.length; i += 4) {
        if (Math.abs(d[i] - r) + Math.abs(d[i + 1] - g) + Math.abs(d[i + 2] - b) < 24) line++;
        if (Math.abs(d[i] - br) + Math.abs(d[i + 1] - bg) + Math.abs(d[i + 2] - bb) > 24) drawn++;
      }
      return { line, drawn };
    });
    assert.ok(traj.line > 2000, `${traj.line} px in the trajectory colour`);
    assert.ok(traj.drawn > 5000, `${traj.drawn} px off the background`);
    const filled = await page.evaluate(() => window.__filled);
    for (const name of ["x", "y", "z"]) assert.ok(filled.includes(name), `axis name ${name} drawn`);
  } finally {
    await browser.close();
  }
});

test("Van der Pol has 2 states, so its 3D option is disabled with the reason", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol oscillator");
    const space = viewRadio(page, "3D view");
    assert.equal(await space.getAttribute("aria-disabled"), "true");
    assert.equal(
      await page.locator(`[id="${await space.getAttribute("aria-describedby")}"]`).innerText(),
      "The 3D view needs a flow with at least 3 states. Van der Pol oscillator has 2.",
    );
    await page.getByRole("button", { name: /^Axes:/ }).click();
    const item = page.getByRole("menu").locator("[data-view-choice=space]");
    assert.equal(await item.getAttribute("aria-disabled"), "true");
    await page.keyboard.press("Escape");
  } finally {
    await browser.close();
  }
});

test("a frame of dragging Lorenz in 3D with 8 trajectories of 20,000 samples takes under 16 ms (median of 60)", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Lorenz equations");
    // Seven more start points, placed on the phase plane with the Add trajectory tool.
    await page.locator(".plot-toolbar").getByRole("button", { name: "Add trajectory" }).click();
    const plot = page.locator("canvas.plot-canvas");
    const box = await plot.boundingBox();
    for (let k = 0; k < 7; k++) {
      await page.mouse.click(box.x + box.width * (0.3 + 0.06 * k), box.y + box.height * (0.35 + 0.04 * k));
      await page.waitForTimeout(600);
    }
    assert.equal(await page.locator('.panel-left ul[aria-label="Initial conditions"] > li').count(), 8);
    const commit = async (name, value) => {
      const field = page.locator(".panel-right").getByLabel(name, { exact: true });
      await field.click();
      await field.fill(value);
      await field.press("Enter");
    };
    await commit("Samples per trajectory (0 = automatic)", "20000");
    await commit("Time span T: integrate for this long from each start point", "200");
    await viewRadio(page, "3D view").click();
    await overlay(page).waitFor();
    await waitPortrait(page, "req.zAxis && req.trajectories.length === 8 && req.samples === 20000 && req.trajectories[0].t1 === 200");
    await page.waitForTimeout(800);
    const { res } = await last(page);
    assert.equal(res.trajectories.length, 8);
    assert.ok(res.trajectories.every((t) => t.t.length >= 20000), res.trajectories.map((t) => t.t.length).join());

    await page.evaluate(() => {
      const gl = document.querySelector("canvas.space-gl").getContext("webgl2");
      window.__frames = [];
      let t0 = 0;
      window.addEventListener("pointermove", () => (t0 = performance.now()), { capture: true });
      // Reading a pixel back waits for the GPU to finish the frame (gl.finish() does not, in Chromium).
      const px = new Uint8Array(4);
      window.addEventListener("pointermove", () => {
        gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
        window.__frames.push(performance.now() - t0);
      });
    });
    const ob = await overlay(page).boundingBox();
    const [cx, cy] = [ob.x + ob.width / 2, ob.y + ob.height / 2];
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    for (let k = 1; k <= 60; k++) await page.mouse.move(cx + 3 * k, cy + Math.round(20 * Math.sin(k / 9)));
    await page.mouse.up();
    const frames = await page.evaluate(() => window.__frames.slice(-60));
    assert.equal(frames.length, 60);
    const sorted = [...frames].sort((a, b) => a - b);
    const median = (sorted[29] + sorted[30]) / 2;
    console.log(`# frame ms over 60 drag moves: median ${median.toFixed(2)}, min ${sorted[0].toFixed(2)}, max ${sorted[59].toFixed(2)}`);
    assert.ok(median < 16, `median frame ${median.toFixed(2)} ms`);
  } finally {
    await browser.close();
  }
});
