// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5611/";

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

const inspector = (page) => page.locator(".panel-right");
const methodSelect = (page) => inspector(page).getByLabel("Integration method", { exact: true });
const statusSolver = (page) => page.locator(".statusbar button", { hasText: "·" });

async function optionStates(page) {
  return methodSelect(page)
    .locator("option")
    .evaluateAll((os) => os.map((o) => [o.value, o.textContent, o.disabled]));
}

test("the Hamiltonian pendulum runs Störmer–Verlet and Yoshida 4 and reads out the energy drift", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Hamiltonian pendulum");
    assert.deepEqual(await optionStates(page), [
      ["dopri5", "Dormand–Prince 5(4)", false],
      ["rk4", "Runge–Kutta 4", false],
      ["heun", "Heun", false],
      ["euler", "Euler", false],
      ["verlet", "Störmer–Verlet", false],
      ["yoshida4", "Yoshida 4", false],
      ["rosenbrock23", "Rosenbrock 2(3)", false],
    ]);
    assert.equal(await inspector(page).locator("[data-method-refusal]").count(), 0);
    const drift = inspector(page).locator("[data-energy-drift]");
    assert.match(await drift.innerText(), /^\|ΔH\| max = \d\.\d × 10[⁻⁰¹²³⁴⁵⁶⁷⁸⁹]+$/, "the default method reports its drift too");

    await methodSelect(page).selectOption("verlet");
    await commit(inspector(page).getByRole("textbox", { name: "Step size h" }), "0.05");
    await waitPortrait(page, "req.solver.method === 'verlet' && req.solver.h === 0.05");
    assert.equal((await statusSolver(page).locator(".ds-statusbar-text").innerText()).trim(), "Störmer–Verlet · h 0.05");
    assert.match(await inspector(page).getByText(/^Störmer–Verlet \(leapfrog\): symplectic/).innerText(), /energy error stays bounded/);
    const { res } = await page.evaluate(() => window.__sepxMock.lastPortrait);
    const worst = Math.max(...res.trajectories.map((tr) => tr.energy.maxAbsDrift));
    const [m, e] = worst.toExponential(1).split("e");
    const sup = String(Number(e)).replace(/./g, (c) => "⁻⁰¹²³⁴⁵⁶⁷⁸⁹"["-0123456789".indexOf(c)]);
    assert.equal(await drift.innerText(), `|ΔH| max = ${m} × 10${sup}`, "the largest drift over the drawn trajectories");
    assert.doesNotMatch(await drift.innerText(), /-/, "a real minus, never a hyphen");

    await methodSelect(page).selectOption("yoshida4");
    await waitPortrait(page, "req.solver.method === 'yoshida4' && req.solver.h === 0.05");
    assert.equal((await statusSolver(page).locator(".ds-statusbar-text").innerText()).trim(), "Yoshida 4 · h 0.05");
  } finally {
    await browser.close();
  }
});

test("a system written as equations of motion disables both symplectic methods and says why", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Simple pendulum");
    const states = await optionStates(page);
    assert.deepEqual(
      states.filter(([, , disabled]) => disabled).map(([v]) => v),
      ["verlet", "yoshida4"],
    );
    const reason = inspector(page).locator("[data-method-refusal]");
    assert.equal(
      await reason.innerText(),
      "Störmer–Verlet and Yoshida 4 need a Hamiltonian H(q, p); this system is given by its equations of motion.",
    );
    const described = await methodSelect(page).getAttribute("aria-describedby");
    assert.ok(described.split(" ").includes(await reason.getAttribute("id")), "the picker is described by the reason");
    assert.equal(await inspector(page).locator("[data-energy-drift]").count(), 0, "no energy for a system without H");
  } finally {
    await browser.close();
  }
});

test("loading equations of motion after a symplectic method switches to Dormand–Prince and still draws", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Hamiltonian pendulum");
    await methodSelect(page).selectOption("verlet");
    await waitPortrait(page, "req.solver.method === 'verlet'");
    await page.getByRole("button", { name: "Templates…" }).click();
    await page.waitForSelector(".gallery");
    await page.locator(".gallery .card", { hasText: "Lorenz equations" }).first().click();
    await page.getByRole("button", { name: "Don't save" }).click();
    await page.waitForSelector(".gallery", { state: "detached" });
    await waitPortrait(page, "req.solver.method === 'dopri5' && req.solver.h === 0 && res.trajectories.length > 0");
    assert.equal(await methodSelect(page).inputValue(), "dopri5");
    assert.equal(
      await inspector(page).locator("[data-method-note]").innerText(),
      "Switched from Störmer–Verlet to Dormand–Prince 5(4).",
    );
    assert.equal(await page.locator(".statusbar .ds-statusbar-item--danger").count(), 0, "no failed computation");
    assert.equal(await page.getByText(/^Could not compute/).count(), 0, "no error banner");
    await methodSelect(page).selectOption("rk4");
    await page.waitForSelector("[data-method-note]", { state: "detached" });
  } finally {
    await browser.close();
  }
});
