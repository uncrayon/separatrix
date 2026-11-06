// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5612/";
const FIXTURE = fs.readFileSync(path.join(__dirname, "../../../crates/project/tests/fixtures/lyapunov-view.sepx"), "utf8");

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

async function waitRun(page, pred) {
  await page.waitForFunction(
    (src) => {
      const l = window.__sepxMock?.lastLyapunov;
      return !!l && new Function("req", "res", `return (${src});`)(l.req, l.res);
    },
    pred,
    { timeout: 30000 },
  );
  return page.evaluate(() => window.__sepxMock.lastLyapunov);
}

const inspector = (page) => page.locator(".panel-right");
const viewRadio = (page, name) =>
  inspector(page).locator("[role=radiogroup][aria-label=View]").getByRole("radio", { name, exact: true });
const readout = (page) => inspector(page).locator("[data-lyapunov-readout]");
const statusSolver = (page) => page.locator(".statusbar button", { hasText: "·" });

async function readoutRows(page) {
  return readout(page)
    .locator(".ds-panel-section-kv-row")
    .evaluateAll((rows) =>
      rows.map((r) => [r.querySelector("dt").innerText.trim(), r.querySelector("dd").innerText.trim()]),
    );
}

const num = (text) => Number(text.replace("−", "-"));

async function colourCounts(page, n) {
  return page.evaluate((n) => {
    const css = getComputedStyle(document.documentElement);
    const canvas = document.querySelector(".lyapunov-canvas");
    const ctx = canvas.getContext("2d");
    const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const probe = document.createElement("canvas").getContext("2d");
    const rgb = (c) => {
      probe.fillStyle = c;
      probe.fillRect(0, 0, 1, 1);
      return [...probe.getImageData(0, 0, 1, 1).data.slice(0, 3)];
    };
    const want = Array.from({ length: n }, (_, i) => rgb(css.getPropertyValue(`--traj-${i + 1}`).trim()));
    const counts = new Array(n).fill(0);
    for (let k = 0; k < data.length; k += 4) {
      want.forEach((c, i) => {
        if (Math.abs(data[k] - c[0]) + Math.abs(data[k + 1] - c[1]) + Math.abs(data[k + 2] - c[2]) < 24) counts[i]++;
      });
    }
    return counts;
  }, n);
}

test("Lorenz: the Lyapunov view runs the first start point and reads out λ₁ … λ₃ with ± and the trace check", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Lorenz");
    await viewRadio(page, "Lyapunov spectrum").click();
    const { req, res } = await waitRun(page, "req.total === 1000");
    assert.deepEqual(
      { transient: req.transient, total: req.total, interval: req.interval, blocks: req.blocks },
      { transient: 100, total: 1000, interval: 0.1, blocks: 10 },
    );
    assert.deepEqual(req.params, [10, 28, 8 / 3]);
    assert.equal(req.solver.method, "dopri5", "the studio's solver");
    await readout(page).waitFor();
    const rows = await readoutRows(page);
    assert.deepEqual(
      rows.map((r) => r[0]),
      ["λ₁", "λ₂", "λ₃", "Σλ", "⟨tr J⟩", "Difference"],
    );
    for (const [, v] of rows.slice(0, 3)) assert.match(v, /^−?\d+\.\d+ ± \d\.\d+$/, v);
    const [l1, l2, l3] = rows.slice(0, 3).map(([, v]) => num(v.split(" ± ")[0]));
    assert.ok(Math.abs(l1 - 0.9056) < 0.05, `λ₁ = ${l1}`);
    assert.ok(Math.abs(l2) < 0.05, `λ₂ = ${l2}`);
    assert.ok(Math.abs(l3 + 14.572) < 0.1, `λ₃ = ${l3}`);
    const sum = num(rows[3][1]);
    const trace = num(rows[4][1]);
    assert.ok(Math.abs(trace + 41 / 3) < 1e-3, `⟨tr J⟩ = −(σ + 1 + b) = −41/3, read ${trace}`);
    assert.ok(Math.abs(sum - trace) < 1e-3, `Σλ ${sum} agrees with ⟨tr J⟩ ${trace}`);
    assert.match(rows[5][1], /^−?\d\.\d × 10[⁻⁰¹²³⁴⁵⁶⁷⁸⁹]+$|^0$/);
    assert.equal(res.exponents.length, 3);

    assert.equal(
      (await statusSolver(page).locator(".ds-statusbar-text").innerText()).trim(),
      "10⁴ renormalizations · DOPRI5 · rtol 1e−6",
    );
    await page.waitForTimeout(300);
    const counts = await colourCounts(page, 3);
    assert.ok(counts.every((c) => c > 200), `pixels per colour: ${counts}`);
    assert.equal(await page.getByRole("button", { name: "Direction field" }).count(), 0);
    assert.equal(await inspector(page).getByRole("heading", { name: "Fixed points" }).count(), 0);
    assert.equal(await page.getByRole("button", { name: "Lyapunov spectrum: t horizontal, λ vertical" }).count(), 1);

    const group = inspector(page).locator("[role=radiogroup][aria-label=View]");
    assert.deepEqual(await group.getByRole("radio").allInnerTexts(), ["Phase", "Series", "Section", "3D", "λ"]);
    await page.setViewportSize({ width: 1024, height: 768 });
    await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
    await page.waitForTimeout(200);
    const fits = await group.getByRole("radio").evaluateAll((els) =>
      els.map((el) => {
        const cs = getComputedStyle(el);
        const range = document.createRange();
        range.selectNodeContents(el);
        const pad = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
        return range.getBoundingClientRect().width + pad <= el.getBoundingClientRect().width + 0.5;
      }),
    );
    assert.deepEqual(fits, [true, true, true, true, true]);
  } finally {
    await browser.close();
  }
});

test("changing the run or the method asks for a new run; methods the core can't use are disabled", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Lorenz");
    await viewRadio(page, "Lyapunov spectrum").click();
    await waitRun(page, "req.total === 1000");
    await commit(inspector(page).getByRole("textbox", { name: "Time measured" }), "2000");
    await waitRun(page, "req.total === 2000");
    assert.equal(
      (await statusSolver(page).locator(".ds-statusbar-text").innerText()).trim(),
      "20,000 renormalizations · DOPRI5 · rtol 1e−6",
    );
    const blocks = inspector(page).getByLabel("Blocks for the uncertainty", { exact: true });
    await commit(blocks, "30000");
    assert.match(await inspector(page).innerText(), /Must be a whole number from 2 to 20,000/);
    await blocks.press("Escape");

    const method = inspector(page).getByLabel("Integration method", { exact: true });
    const options = await method.locator("option").evaluateAll((os) => os.map((o) => [o.value, o.disabled]));
    assert.deepEqual(
      options.filter(([, disabled]) => disabled).map(([v]) => v).sort(),
      ["rosenbrock23", "verlet", "yoshida4"],
    );
    await method.selectOption("rk4");
    const { req } = await waitRun(page, "req.solver.method === 'rk4'");
    assert.equal(req.solver.h, 0.01);
    assert.ok(req.solver.maxSteps >= (100 + 2000) / 0.01, `step limit ${req.solver.maxSteps} covers the run`);
    assert.equal(
      (await statusSolver(page).locator(".ds-statusbar-text").innerText()).trim(),
      "20,000 renormalizations · RK4 · h 0.01",
    );
  } finally {
    await browser.close();
  }
});

test("Hénon map: two exponents per iterate, ⟨ln |det J|⟩ = ln b, and no integrator", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Hénon");
    await viewRadio(page, "Lyapunov spectrum").click();
    const { req } = await waitRun(page, "req.total === 100000");
    assert.deepEqual([req.transient, req.interval, req.blocks], [1000, 1, 10]);
    const rows = await readoutRows(page);
    assert.deepEqual(
      rows.map((r) => r[0]),
      ["λ₁", "λ₂", "Σλ", "⟨ln |det J|⟩", "Difference"],
    );
    const l1 = num(rows[0][1].split(" ± ")[0]);
    const l2 = num(rows[1][1].split(" ± ")[0]);
    assert.ok(Math.abs(l1 - 0.4192) < 0.01, `λ₁ = ${l1}`);
    assert.ok(Math.abs(l2 + 1.6232) < 0.01, `λ₂ = ${l2}`);
    assert.ok(Math.abs(num(rows[3][1]) - Math.log(0.3)) < 1e-5, "det J = −b everywhere");
    assert.match(await inspector(page).innerText(), /Exponents, per iterate/);
    assert.equal(await inspector(page).getByLabel("Integration method", { exact: true }).count(), 0);
    assert.equal((await statusSolver(page).count()) + 0, 0);
    assert.match(await page.locator(".statusbar").innerText(), /10⁵ renormalizations/);
  } finally {
    await browser.close();
  }
});

test("the saved Lyapunov view opens with its run, and saving writes it back", async () => {
  const { browser, page } = await open();
  try {
    await page.evaluate((text) => window.__sepxMock.files.set("/mock/lyapunov-view.sepx", text), FIXTURE);
    await pick(page, "Lorenz");
    await page.evaluate(() => (window.__sepxMock.nextOpenPath = "/mock/lyapunov-view.sepx"));
    await page.getByRole("button", { name: "Open…" }).click();
    const { req } = await waitRun(page, "req.total === 10000");
    assert.deepEqual([req.transient, req.interval, req.blocks], [100, 0.1, 10]);
    assert.equal(req.solver.method, "rk4");
    assert.equal(await viewRadio(page, "Lyapunov spectrum").getAttribute("aria-checked"), "true");
    assert.equal(
      (await statusSolver(page).locator(".ds-statusbar-text").innerText()).trim(),
      "10⁵ renormalizations · RK4 · h 0.01",
    );
    await readout(page).waitFor();
    assert.equal(await page.evaluate(() => window.__sepxMock.document.dirty), false);
    await page.evaluate(() => (window.__sepxMock.nextSavePath = "/mock/out.sepx"));
    await page.getByRole("button", { name: "Save as…" }).click();
    await page.waitForFunction(() => window.__sepxMock.files.has("/mock/out.sepx"));
    const out = JSON.parse(await page.evaluate(() => window.__sepxMock.files.get("/mock/out.sepx")));
    const v = out.views[0];
    assert.equal(v.kind, "lyapunov");
    assert.deepEqual(v.lyapunov, JSON.parse(FIXTURE).views[0].lyapunov);
    assert.deepEqual(v.y_axis, { kind: "exponent" });
    const saved = JSON.parse(FIXTURE).views[0];
    assert.deepEqual([v.x_range, v.y_range], [saved.x_range, saved.y_range], "the file's framing, not a fit of the run");
  } finally {
    await browser.close();
  }
});
