// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5613/";

async function open(width = 1440, height = 900) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width, height } });
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

const inspector = (page) => page.locator(".panel-right");
const viewRadio = (page, name) =>
  inspector(page).locator("[role=radiogroup][aria-label=View]").getByRole("radio", { name, exact: true });
const paramValue = (page, name) => page.getByRole("textbox", { name: `Value of ${name}`, exact: true }).inputValue();

async function branchSettled(page) {
  await page.waitForFunction(
    () => {
      const list = document.querySelector("[data-branches]");
      return !!list && !list.textContent.includes("Computing…") && document.querySelectorAll("[data-special-kind]").length > 0;
    },
    null,
    { timeout: 30000 },
  );
}

async function curvesSettled(page) {
  await page.waitForFunction(
    () => {
      const list = document.querySelector("[data-curves]");
      return !!list && !list.querySelector("[data-curve-status=computing]") && !document.querySelector("[data-curves-search]");
    },
    null,
    { timeout: 30000 },
  );
}

test("CO oxidation: from an H row in the Branch view to a BT row of the Parameter plane, which sets both parameters", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "CO oxidation");
    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    const kinds = await inspector(page)
      .locator("[data-special-points] [data-special-kind]")
      .evaluateAll((rows) => rows.map((r) => r.dataset.specialKind));
    assert.deepEqual(kinds, ["hopf", "fold", "hopf", "fold"], "H, LP, H, LP in increasing k₂");

    const row = inspector(page).locator("[data-special-kind=hopf]").last();
    await row.getByRole("button", { name: "Follow in two parameters…" }).click();
    const panel = row.getByRole("group", { name: "Follow this Hopf point in two parameters" });
    await panel.getByLabel("Free also").selectOption({ label: "K" });
    assert.equal(await panel.locator("[data-follow-note]").innerText(), "Opens the Parameter plane with k₂ across and K up, from 0 to 2.");
    await panel.getByRole("button", { name: "Follow", exact: true }).click();

    assert.equal(await viewRadio(page, "Parameter plane").getAttribute("aria-checked"), "true");
    await curvesSettled(page);
    assert.equal(
      await inspector(page).locator("[data-plane-caption]").innerText(),
      "Across is always the Branch view’s parameter, so the two views line up.",
    );
    const curveRow = inspector(page).locator("[data-curves] > li");
    assert.equal(await curveRow.count(), 1);
    assert.match(await curveRow.innerText(), /Hopf curve[\s\S]*From H at k₂ = 1\.0516, K = 0\.4[\s\S]*534 points · stopped at the range's end · neutral past the BT points/);
    const points = inspector(page).locator("[data-plane-points] [data-plane-point]");
    assert.deepEqual(await points.evaluateAll((rows) => rows.map((r) => r.dataset.planePoint)), [
      "bogdanovTakens",
      "bogdanovTakens",
      "bautin",
      "bautin",
    ]);
    assert.match(await page.locator(".legend-bar").innerText(), /Hopf \(H\)[\s\S]*No bifurcation \(neutral saddle\)[\s\S]*Branch view’s range, K = 0\.4/);

    const bt = points.first();
    assert.match(await bt.innerText(), /BT Bogdanov–Takens\s+k₂ = 1\.161199 {2}K = 0\.722339/);
    await bt.getByRole("button").click();
    await page.waitForTimeout(200);
    assert.match(await paramValue(page, "k_2"), /^1\.161/);
    assert.match(await paramValue(page, "K"), /^0\.7223/);
    assert.match(await page.locator(".legend-bar").innerText(), /Branch view’s range, K = 0\.4\n/, "the slice stays at the K the branches were computed at");
  } finally {
    await browser.close();
  }
});

test("Search in two parameters fills the list curve by curve and merges the repeated fold and Hopf curves", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "CO oxidation");
    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    await inspector(page).getByRole("button", { name: "Search in two parameters" }).click();
    await curvesSettled(page);
    const rows = await inspector(page)
      .locator("[data-curves] > li")
      .evaluateAll((els) => els.map((el) => [el.dataset.curveKind, el.dataset.curveStatus]));
    assert.deepEqual(rows, [
      ["hopf", "done"],
      ["fold", "done"],
      ["hopf", "merged"],
      ["fold", "merged"],
    ]);
    assert.match(await inspector(page).locator("[data-curves] > li").nth(2).innerText(), /Same curve as the first: merged/);
    assert.match(await inspector(page).locator("[data-curves] > li").nth(3).innerText(), /Same curve as the second: merged/);
    assert.equal(await inspector(page).locator("[data-plane-points] [data-plane-point]").count(), 5);
    assert.match(
      await inspector(page).locator("[data-plane-point=bogdanovTakens]").first().innerText(),
      /On the fold and Hopf curves/,
    );
    const status = await page.locator(".statusbar").innerText();
    assert.match(status, /Curves \d/);
    assert.match(status, /2 curves · 960 points · 5 special/);
    assert.equal(await page.getByRole("button", { name: "Export…" }).isDisabled(), true, "the plane has no export yet");
  } finally {
    await browser.close();
  }
});

test("the plane opened with nothing to search from shows its empty state, and a one-parameter map says why it can't", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "CO oxidation");
    await viewRadio(page, "Parameter plane").click();
    const empty = page.locator(".plot-empty");
    assert.match(await empty.innerText(), /No bifurcation to follow yet/);
    await empty.getByRole("button", { name: "Go to the Branch view" }).click();
    assert.equal(await viewRadio(page, "Continuation").getAttribute("aria-checked"), "true");

    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    await pick(page, "Logistic map");
    const plane = viewRadio(page, "Parameter plane");
    assert.equal(await plane.getAttribute("aria-disabled"), "true");
    const why = await plane.getAttribute("aria-describedby");
    assert.equal(await page.locator(`[id="${why}"]`).innerText(), "The parameter plane needs two parameters; Logistic map has 1.");
  } finally {
    await browser.close();
  }
});

test("the predator–prey map: F2's flip line stalls at the map's singularity, and Retry halves Δs", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Predator–prey map");
    await viewRadio(page, "Continuation").click();
    await branchSettled(page);
    await inspector(page).getByRole("button", { name: "Search in two parameters" }).click();
    await curvesSettled(page);
    const stall = inspector(page).locator(".inspector-stall");
    assert.equal(await stall.count(), 1);
    assert.match(
      await stall.innerText(),
      /The step became too small near a = 3\.0000, ε = −1\.4990\. The map may be singular there, or a special point may be near\. Lower Δs\./,
    );
    await stall.getByRole("button", { name: "Retry with Δs = 0.005" }).click();
    await curvesSettled(page);
    assert.equal(await inspector(page).getByLabel("First step in arclength").inputValue(), "0.005");
    const kinds = await inspector(page)
      .locator("[data-plane-points] [data-plane-point]")
      .evaluateAll((rows) => rows.map((r) => r.dataset.planePoint));
    assert.deepEqual(kinds, ["resonance2", "resonance3", "resonance4", "chenciner", "foldFlip"]);
  } finally {
    await browser.close();
  }
});
