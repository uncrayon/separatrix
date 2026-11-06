// Cobweb plots of 1-D maps in the browser preview (mocked core): the
// logistic map's default view, the transient, adding and dragging orbits on
// the diagonal, the View control, Hénon's disabled cobweb, and the exports.
// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5198/";

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
const viewRadio = (page, name) => page.locator(".panel-right [role=radiogroup][aria-label=View]").getByRole("radio", { name, exact: true });

/** The pixel (in the canvas) of plot point (x, y), from the last request's ranges (the ranges as drawn). */
async function toPx(page, x, y) {
  const { req } = await last(page);
  const box = await page.locator("canvas.plot-canvas").boundingBox();
  const [x0, x1] = req.xRange;
  const [y0, y1] = req.yRange;
  return {
    x: box.x + ((x - x0) / (x1 - x0)) * box.width,
    y: box.y + (1 - (y - y0) / (y1 - y0)) * box.height,
  };
}

/** Waits until the mock's last portrait request satisfies `pred` (a function body over `req` and `res`). */
async function waitPortrait(page, pred) {
  await page.waitForFunction(
    (src) => {
      const p = window.__sepxMock?.lastPortrait;
      return !!p && new Function("req", "res", `return (${src});`)(p.req, p.res);
    },
    pred,
    { timeout: 5000 },
  );
}

test("the logistic map opens as a cobweb: view, axis names, multipliers, legend, f's graph", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Logistic map");
    assert.equal(await viewRadio(page, "Cobweb").getAttribute("aria-checked"), "true");
    const canvas = page.locator("canvas.plot-canvas");
    assert.equal(await canvas.getAttribute("aria-roledescription"), "cobweb plot");
    assert.match(await canvas.getAttribute("aria-label"), /^Cobweb plot of xₙ₊₁ = f\(xₙ\): xₙ from .* across, xₙ₊₁ from .* up\. 1 orbit, starting at x₀ = 0\.1\. Fixed points: unstable at x\* = 0 \(λ = 3\.2\); unstable at x\* = 0\.688 \(λ = −1\.2\)\.$/);
    // Axis names x_n and x_{n+1}, typeset (MathLive's spoken text names the subscripts).
    const xName = await page.locator(".plot-wrap .axis-label-x").innerText();
    const yName = await page.locator(".plot-wrap .axis-label-y").innerText();
    assert.match(xName, /x sub n(?! plus)/);
    assert.match(yName, /x sub n plus 1/);
    assert.match(await page.getByRole("button", { name: /^Cobweb:/ }).getAttribute("aria-label"), /^Cobweb: x n horizontal, x n\+1 vertical$/);
    // Fixed points with their multiplier λ = f′(x*).
    const rows = page.locator('.panel-right ul[aria-label="Fixed points"] .ds-fp-row');
    await rows.first().waitFor();
    assert.equal(await rows.count(), 2);
    const texts = (await rows.allInnerTexts()).map((t) => t.replace(/\u00a0/g, " "));
    for (const t of texts) assert.match(t, /λ = f′\(x\*\) = /);
    assert.match(texts.join(" "), /x\* = 0\.688\nλ = f′\(x\*\) = −1\.200, \|λ\| = 1\.200/);
    await rows.nth(1).click();
    assert.match(await page.locator(".panel-right .ds-fp-detail").innerText(), /\|λ\| > 1, so x\* is unstable\. λ < 0: nearby orbits alternate sides of x\*\./);
    // The legend.
    const legend = await page.locator(".legend-bar .ds-phase-legend").innerText();
    assert.match(legend, /Orbit, cobweb/);
    assert.match(legend, /Graph/);
    assert.match(legend, /Diagonal/);
    assert.match(legend, /Start point, drag along the diagonal/);
    // One request brought the graph, the staircase and the fixed points.
    const { req, res } = await last(page);
    assert.equal(req.graphSamples, 2048);
    assert.deepEqual(req.xAxis, { kind: "state", index: 0 });
    assert.deepEqual(req.yAxis, { kind: "state", index: 0 });
    assert.equal(req.fieldGrid, 0);
    assert.equal(req.nullclines, false);
    assert.ok(res.graph.length > 0 && res.graph[0].length > 100, "f's graph came back");
    assert.equal(res.fixedPoints.length, 2);
    // Equal scales: the same units per pixel across and up.
    const box = await page.locator("canvas.plot-canvas").boundingBox();
    const ux = (req.xRange[1] - req.xRange[0]) / box.width;
    const uy = (req.yRange[1] - req.yRange[0]) / box.height;
    assert.ok(Math.abs(ux / uy - 1) < 0.01, `equal scales (${ux} vs ${uy})`);
  } finally {
    await browser.close();
  }
});

test("r = 3.5 with a transient of 200 shows the 4-cycle", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Logistic map");
    const canvas = page.locator("canvas.plot-canvas");
    const before = await canvas.screenshot();
    const r = page.getByLabel("Value of r", { exact: true });
    await r.fill("3.5");
    await r.press("Enter");
    const transient = page.getByRole("spinbutton", { name: "Transient iterates, computed but not drawn" });
    await transient.fill("200");
    await transient.press("Enter");
    await waitPortrait(page, "req.params[0] === 3.5 && req.transient === 200");
    const { req, res } = await last(page);
    assert.equal(req.params[0], 3.5);
    assert.equal(req.transient, 200);
    const tr = res.trajectories[0];
    assert.equal(tr.t[0], 200, "drawn from n = 200");
    assert.equal(tr.t.length, 61);
    const tail = tr.x.slice(-8);
    const distinct = [];
    for (const v of tail) if (!distinct.some((d) => Math.abs(d - v) < 1e-6)) distinct.push(v);
    assert.equal(distinct.length, 4, `period 4: ${tail.join(", ")}`);
    assert.match(await page.locator("[data-transient-hint]").innerText(), /^Hides x₀ … x₁₉₉; the orbit is drawn from x₂₀₀\.$/);
    assert.match(await canvas.getAttribute("aria-label"), /starting at x₀ = 0\.1, first 200 iterates hidden\./);
    assert.match(await page.locator('.panel-right ul[aria-label="Trajectories"]').innerText(), /n ∈ \[200, 260\]/);
    await page.waitForTimeout(200);
    const after = await canvas.screenshot();
    assert.ok(!before.equals(after), "the plot changed");
    // The stepper adds 10, Shift+↑ 100.
    await transient.press("ArrowUp");
    await waitPortrait(page, "req.transient === 210");
    await transient.press("Shift+ArrowUp");
    await waitPortrait(page, "req.transient === 310");
    await transient.fill("200");
    await transient.press("Enter");
    await waitPortrait(page, "req.transient === 200");
    // The time series shows the orbit, at n in [200, 260].
    await viewRadio(page, "Time series").click();
    await waitPortrait(page, "req.xAxis.kind === 'time'");
    let { req: sreq } = await last(page);
    assert.ok(sreq.xRange[0] <= 200 && sreq.xRange[1] >= 260, `n range ${sreq.xRange}`);
    // A changed transient moves the n axis with the orbit.
    await transient.fill("500");
    await transient.press("Enter");
    await waitPortrait(page, "req.transient === 500");
    ({ req: sreq } = await last(page));
    assert.ok(sreq.xRange[0] <= 500 && sreq.xRange[1] >= 560, `n range ${sreq.xRange}`);
  } finally {
    await browser.close();
  }
});

test("Add trajectory: a click starts an orbit at the pointer's x, on the diagonal; a handle drags along it", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Logistic map");
    await page.locator('.canvas-toolbar button[aria-label="Add orbit"]').click();
    const at = await toPx(page, 0.5, 0.15);
    await page.mouse.click(at.x, at.y);
    await waitPortrait(page, "req.trajectories.length === 2");
    let { req } = await last(page);
    assert.ok(Math.abs(req.trajectories[1].x0[0] - 0.5) < 0.01, `x0 = ${req.trajectories[1].x0[0]}`);
    assert.equal(await page.locator('.panel-right ul[aria-label="Trajectories"] > li').count(), 2);

    // Drag the first orbit's handle (at (0.1, 0.1)) to x ≈ 0.3; the pointer's y is ignored.
    await page.locator('.canvas-toolbar button[aria-label="Select"]').click();
    const from = await toPx(page, 0.1, 0.1);
    const to = await toPx(page, 0.3, 0.9);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
    await page.mouse.move(to.x, to.y, { steps: 4 });
    await page.mouse.up();
    await waitPortrait(page, "Math.abs(req.trajectories[0].x0[0] - 0.3) < 0.01");
    ({ req } = await last(page));
    assert.ok(Math.abs(req.trajectories[0].x0[0] - 0.3) < 0.01);
    assert.equal(
      await page.locator('.panel-right ul[aria-label="Trajectories"] > li').first().locator(".ds-traj-ic-value").innerText(),
      "0.30",
    );
  } finally {
    await browser.close();
  }
});

test("the keyboard cursor moves along the diagonal; Enter starts an orbit there", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Logistic map");
    const canvas = page.locator("canvas.plot-canvas");
    await canvas.focus();
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Shift+ArrowUp");
    const cursor = page.locator(".kbd-cursor");
    assert.equal(await cursor.count(), 1);
    const { req } = await last(page);
    const box = await canvas.boundingBox();
    const pos = await cursor.evaluate((el) => [parseFloat(el.style.left), parseFloat(el.style.top)]);
    const x = req.xRange[0] + (pos[0] / box.width) * (req.xRange[1] - req.xRange[0]);
    const y = req.yRange[0] + (1 - pos[1] / box.height) * (req.yRange[1] - req.yRange[0]);
    const onePx = (req.xRange[1] - req.xRange[0]) / box.width;
    assert.ok(Math.abs(x - y) < onePx, `on the diagonal (${x}, ${y})`);
    await page.keyboard.press("Enter");
    await waitPortrait(page, "req.trajectories.length === 2");
    const after = await last(page);
    assert.ok(Math.abs(after.req.trajectories[1].x0[0] - x) < onePx);
  } finally {
    await browser.close();
  }
});

test("Time series and back: no graph asked for, then the cobweb's ranges come back; arrow keys move the View radios", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Logistic map");
    await page.waitForTimeout(300);
    const first = (await last(page)).req;
    await viewRadio(page, "Time series").click();
    await waitPortrait(page, "req.xAxis.kind === 'time'");
    let { req } = await last(page);
    assert.equal(req.graphSamples, undefined);
    assert.deepEqual(req.yAxis, { kind: "state", index: 0 });
    assert.match(await page.getByRole("button", { name: /^Axes:/ }).getAttribute("aria-label"), /^Axes: n horizontal, x vertical$/);
    await viewRadio(page, "Cobweb").click();
    await waitPortrait(page, "req.graphSamples === 2048");
    await page.waitForTimeout(300);
    ({ req } = await last(page));
    // The cobweb's ranges as they were (the narrower one exactly; the other is widened to the plot's shape).
    assert.deepEqual(req.yRange, first.yRange);
    assert.ok(Math.abs(req.xRange[0] + req.xRange[1] - first.xRange[0] - first.xRange[1]) < 1e-9, "the same centre");

    // The radiogroup's arrow keys skip the impossible Phase plane.
    await viewRadio(page, "Cobweb").focus();
    await page.keyboard.press("ArrowLeft");
    assert.equal(await viewRadio(page, "Time series").getAttribute("aria-checked"), "true");
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute("aria-label")), "Time series");
    // The Lyapunov view and the orbit diagram start a run when they open, so wait for each choice to be checked.
    const checked = (name) => viewRadio(page, name).and(page.locator('[aria-checked="true"]')).waitFor({ timeout: 5000 });
    await page.keyboard.press("ArrowLeft");
    await checked("Orbit diagram"); // Phase plane is skipped, wrapping round to the last choice
    await page.keyboard.press("Home");
    await checked("Time series");
    await page.keyboard.press("End");
    await checked("Orbit diagram");
    await page.keyboard.press("ArrowLeft");
    await checked("Lyapunov spectrum");
    await page.keyboard.press("ArrowLeft");
    await checked("Cobweb");
    const phase = viewRadio(page, "Phase plane");
    assert.equal(await phase.getAttribute("aria-disabled"), "true");
    const why = await phase.getAttribute("aria-describedby");
    assert.equal(await page.locator(`[id="${why}"]`).innerText(), "A phase plane needs at least 2 states. Logistic map has 1.");
    // The group names the reason too (arrow keys skip the option), and the labels are short, with the full name kept.
    const group = page.locator(".panel-right [role=radiogroup][aria-label=View]");
    assert.equal(await group.getAttribute("aria-describedby"), why);
    assert.deepEqual(await group.getByRole("radio").allInnerTexts(), ["Phase", "Series", "Cobweb", "λ", "Orbit"]);
    assert.equal(await phase.getAttribute("title") ?? (await phase.getAttribute("data-tooltip")), "Phase plane");
    // Every label fits inside its segment, in projector mode at 1,024 px too.
    await page.setViewportSize({ width: 1024, height: 768 });
    await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
    await page.waitForTimeout(200);
    const fits = await group.getByRole("radio").evaluateAll((els) =>
      els.map((el) => {
        const cs = getComputedStyle(el);
        const pad = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
        const range = document.createRange();
        range.selectNodeContents(el);
        return range.getBoundingClientRect().width + pad <= el.getBoundingClientRect().width + 0.5;
      }),
    );
    assert.deepEqual(fits, [true, true, true, true, true]);
    await page.evaluate(() => (document.documentElement.dataset.theme = "dark"));
    await page.setViewportSize({ width: 1440, height: 900 });

    // The toolbar menu offers the same View choices.
    await page.getByRole("button", { name: /^Cobweb:/ }).click();
    const menu = page.getByRole("menu");
    const items = menu.getByRole("menuitemradio");
    assert.deepEqual(await items.locator(".axes-menu-label").allInnerTexts(), ["Phase plane", "Time series", "Cobweb", "Lyapunov spectrum", "Orbit diagram"]);
    // The disabled one says why, visibly, under it.
    const disabled = menu.getByRole("menuitemradio", { name: /^Phase plane/ });
    assert.equal(await disabled.getAttribute("aria-disabled"), "true");
    const menuWhy = page.locator(`[id="${await disabled.getAttribute("aria-describedby")}"]`);
    assert.ok(await menuWhy.isVisible());
    assert.equal(await menuWhy.innerText(), "A phase plane needs at least 2 states. Logistic map has 1.");
    await menu.getByRole("menuitemradio", { name: /^Time series/ }).click();
    await waitPortrait(page, "req.xAxis.kind === 'time'");
  } finally {
    await browser.close();
  }
});

test("Hénon: Cobweb is shown but disabled, with its reason", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Hénon map");
    const radio = viewRadio(page, "Cobweb");
    assert.equal(await radio.getAttribute("aria-disabled"), "true");
    assert.equal(await viewRadio(page, "Phase plane").getAttribute("aria-checked"), "true");
    const why = await radio.getAttribute("aria-describedby");
    assert.ok(why);
    const reason = page.locator(`[id="${why}"]`);
    assert.match(await reason.textContent(), /^The cobweb view needs a one-dimensional map .*; Hénon map has 2 states\.$/);
    assert.equal(await reason.locator(".math .tex").count(), 1, "x_{n+1} = f(x_n) is typeset");
    assert.equal(await page.locator(".panel-right [role=radiogroup][aria-label=View]").getAttribute("aria-describedby"), why);
    await radio.click({ force: true });
    assert.equal(await viewRadio(page, "Phase plane").getAttribute("aria-checked"), "true", "clicking it changes nothing");
    assert.equal((await last(page)).req.graphSamples, undefined);
  } finally {
    await browser.close();
  }
});

// --- exports, through the dialog's harness ------------------------------------------

async function openExport() {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    await page.goto(new URL("src/review/export.harness.html?template=logistic-map", BASE).href);
    await page.waitForSelector("[data-harness-ready=yes]", { state: "attached" });
    await page.waitForTimeout(300);
    await page.click("[data-harness-open]");
    await page.waitForSelector(".export-dialog[role=dialog]");
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

const exportFormat = (page, name) => page.locator(".export-dialog").getByRole("radio", { name, exact: true }).click();

test("export: the SVG has the diagonal at 45° in plot-axis and a staircase in traj-1; only Fixed points to include", async () => {
  const { browser, page } = await openExport();
  try {
    await exportFormat(page, "SVG");
    const d = page.locator(".export-dialog");
    assert.equal(await d.getByRole("checkbox", { name: /direction field/ }).count(), 0);
    assert.equal(await d.getByRole("checkbox", { name: /nullclines/ }).count(), 0);
    assert.equal(await d.getByRole("checkbox", { name: /fixed points/ }).count(), 1);
    assert.match(await page.locator(".ds-export-sub").innerText(), /^Logistic map, cobweb of x, as you see it now$/);
    const save = d.locator(".ds-export-actions .ds-button--primary");
    await page.waitForFunction(() => !document.querySelector(".ds-export-actions .ds-button--primary")?.disabled);
    await page.waitForTimeout(500);
    await save.click();
    await page.waitForFunction(() => window.__sepxMock.saved.length === 1);
    const paths = await page.evaluate(() => {
      const f = window.__sepxMock.saved[0];
      const doc = new DOMParser().parseFromString(f.contents, "image/svg+xml");
      return {
        desc: doc.querySelector("svg > desc")?.textContent ?? null,
        paths: [...doc.querySelectorAll("path[stroke]")].map((p) => ({ d: p.getAttribute("d"), stroke: p.getAttribute("stroke") })),
      };
    });
    assert.match(paths.desc, /^Logistic map with the graph of f, the diagonal, 1 orbit and 2 fixed points$/);
    const pts = (d) => [...d.matchAll(/[ML](-?[\d.]+) (-?[\d.]+)/g)].map((m) => [Number(m[1]), Number(m[2]), m[0][0]]);
    const axis = "#7d888b";
    const traj1 = "#0072b2";
    const diag = paths.paths.filter((p) => p.stroke.toLowerCase() === axis && pts(p.d).length === 2);
    assert.equal(diag.length, 1, "one 2-point path in the axis colour");
    const [[ax, ay], [bx, by]] = pts(diag[0].d);
    assert.ok(Math.abs(Math.abs(bx - ax) - Math.abs(by - ay)) < 0.05, `at 45° (${ax},${ay} → ${bx},${by})`);
    const stair = paths.paths.filter((p) => p.stroke.toLowerCase() === traj1);
    assert.equal(stair.length, 1);
    const sp = pts(stair[0].d);
    assert.ok(sp.length >= 9, `${sp.length} vertices`);
    for (let k = 1; k < sp.length; k++) {
      if (sp[k][2] !== "L") continue;
      const vertical = Math.abs(sp[k][0] - sp[k - 1][0]) < 0.01;
      const horizontal = Math.abs(sp[k][1] - sp[k - 1][1]) < 0.01;
      assert.ok(vertical || horizontal, `segment ${k} is vertical or horizontal`);
      if (k >= 2 && sp[k - 1][2] === "L") {
        const prevVertical = Math.abs(sp[k - 1][0] - sp[k - 2][0]) < 0.01;
        assert.notEqual(vertical && !horizontal, prevVertical && !(Math.abs(sp[k - 1][1] - sp[k - 2][1]) < 0.01), `segments ${k - 1} and ${k} alternate`);
      }
    }
  } finally {
    await browser.close();
  }
});

test("export: the LaTeX figure comments its diagonal and cobweb; the CSV is the orbit", async () => {
  const { browser, page } = await openExport();
  try {
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{axis}"));
    const tex = await page.locator(".ds-export-snippet").innerText();
    assert.match(tex, /% Diagonal x_\{n\+1\} = x_n/);
    assert.match(tex, /% Graph of f/);
    assert.match(tex, /% Cobweb of /);
    assert.match(tex, /x_\{n\+1\}/);
    await exportFormat(page, "CSV");
    await page.waitForFunction(() => /^n,x\n/.test(document.querySelector(".ds-export-snippet")?.textContent ?? ""));
    assert.match(await page.locator(".ds-export-snippet").innerText(), /^n,x\n0,0\.1\n/);
  } finally {
    await browser.close();
  }
});
