// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5611/";
const HINT = "These steps look stiff: Dormand–Prince is limited by stability, not accuracy.";

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
const statusStiff = (page) => page.locator(".statusbar [data-stiff-status]");
const useRosenbrock = (page) => inspector(page).getByRole("button", { name: "Use Rosenbrock 2(3)" });

async function stiffRelaxationOscillator(page) {
  await pick(page, "Relaxation oscillator");
  await waitPortrait(page, "req.solver.method === 'dopri5' && req.params[0] === 10");
  assert.equal(await inspector(page).locator("[data-stiff-hint]").count(), 0, "μ = 10 is not stiff");
  assert.equal(await statusStiff(page).count(), 0);
  await commit(page.getByRole("textbox", { name: "Value of mu" }), "50");
  await waitPortrait(page, "req.params[0] === 50 && res.trajectories.some((tr) => tr.stiff === true)");
}

test("a stiff Dormand–Prince run says so and offers Rosenbrock 2(3)", async () => {
  const { browser, page } = await open();
  try {
    await stiffRelaxationOscillator(page);
    const hint = inspector(page).locator("[data-stiff-hint]");
    assert.equal(await hint.innerText(), HINT);
    assert.equal(await useRosenbrock(page).getAttribute("aria-describedby"), await hint.getAttribute("id"));
    assert.equal((await statusStiff(page).locator(".ds-statusbar-text").innerText()).trim(), "Steps look stiff");
    assert.equal(await statusStiff(page).getAttribute("data-tooltip-desc"), HINT);
    assert.ok(await statusStiff(page).evaluate((el) => el.classList.contains("ds-statusbar-item--warn")));

    await useRosenbrock(page).click();
    await waitPortrait(page, "req.solver.method === 'rosenbrock23' && req.solver.h === 0");
    assert.equal(await methodSelect(page).inputValue(), "rosenbrock23");
    assert.ok(await methodSelect(page).evaluate((el) => el === document.activeElement), "focus stays on the method chosen");
    assert.equal(await inspector(page).locator("[data-stiff-hint]").count(), 0);
    assert.equal(await statusStiff(page).count(), 0);
    assert.equal((await statusSolver(page).locator(".ds-statusbar-text").innerText()).trim(), "Rosenbrock 2(3) · rtol 1e−6");
    assert.match(await inspector(page).getByText(/^Rosenbrock 2\(3\), as MATLAB's ode23s/).innerText(), /stiff systems/);
    assert.equal(await inspector(page).getByRole("textbox", { name: "Relative tolerance rtol" }).count(), 1, "adaptive: rtol, no step");
    assert.equal(await inspector(page).getByRole("textbox", { name: "Step size h" }).count(), 0);

    await methodSelect(page).selectOption("dopri5");
    await waitPortrait(page, "req.solver.method === 'dopri5'");
    await inspector(page).locator("[data-stiff-hint]").waitFor();
  } finally {
    await browser.close();
  }
});

test("the method picker lists Rosenbrock 2(3) for every flow, and the status bar item opens the solver settings", async () => {
  const { browser, page } = await open();
  try {
    await stiffRelaxationOscillator(page);
    const options = await methodSelect(page)
      .locator("option")
      .evaluateAll((os) => os.map((o) => [o.value, o.textContent, o.disabled]));
    assert.deepEqual(options.at(-1), ["rosenbrock23", "Rosenbrock 2(3)", false]);
    assert.equal(options.length, 7);

    await statusStiff(page).click();
    assert.ok(
      await page.evaluate(() => !!document.activeElement?.closest("#solver-settings")),
      "focus moved into the Solver section",
    );
  } finally {
    await browser.close();
  }
});
