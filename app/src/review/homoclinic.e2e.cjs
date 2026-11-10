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
const canvasPixels = (page) => page.locator("canvas.curves-canvas").evaluate((c) => c.toDataURL());
const homoclinicRows = (page) => inspector(page).locator("[data-homoclinic-status]");

async function planeWithBt(page) {
  await pick(page, "CO oxidation");
  await viewRadio(page, "Continuation").click();
  await page.waitForFunction(
    () => {
      const list = document.querySelector("[data-branches]");
      return !!list && !list.textContent.includes("Computing…") && document.querySelectorAll("[data-special-kind]").length > 0;
    },
    null,
    { timeout: 30000 },
  );
  await inspector(page).getByRole("button", { name: "Search in two parameters" }).click();
  await page.waitForFunction(
    () => {
      const list = document.querySelector("[data-curves]");
      return !!list && !list.querySelector("[data-curve-status=computing]") && !document.querySelector("[data-curves-search]");
    },
    null,
    { timeout: 30000 },
  );
}

/** Clicks Follow homoclinic on the first Bogdanov–Takens row the preview has a curve for; returns its row. */
async function follow(page) {
  const bts = inspector(page).locator("[data-plane-point=bogdanovTakens]");
  const n = await bts.count();
  for (let i = 0; i < n; i++) {
    const row = bts.nth(i);
    const button = row.getByRole("button", { name: "Follow homoclinic" });
    if ((await button.count()) === 0) continue;
    await button.click();
    await homoclinicRows(page).first().waitFor();
    const settled = await page.waitForFunction(
      () => {
        const r = document.querySelector("[data-homoclinic-status]");
        return !r || r.dataset.homoclinicStatus !== "computing" || !!r.querySelector("[data-homoclinic-progress]")?.textContent.match(/\d+ points?,/);
      },
      null,
      { timeout: 10000 },
    );
    void settled;
    if ((await homoclinicRows(page).first().getAttribute("data-homoclinic-status")) === "failed") {
      await homoclinicRows(page).first().getByRole("button", { name: /Remove the homoclinic curve/ }).click();
      continue;
    }
    return row;
  }
  throw new Error("no Bogdanov–Takens row has a homoclinic curve in the preview");
}

test("Follow homoclinic shows progress in place, Cancel leaves nothing, and following again draws the curve with its legend and row", async () => {
  const { browser, page } = await open();
  try {
    await planeWithBt(page);
    assert.doesNotMatch(await page.locator(".legend-bar").innerText(), /Homoclinic/);
    const before = await canvasPixels(page);

    await follow(page);
    const row = homoclinicRows(page);
    assert.equal(await row.count(), 1);
    assert.equal(await row.getAttribute("data-homoclinic-status"), "computing");
    assert.match(await row.innerText(), /Homoclinic curve[\s\S]*From BT at k₂ = [\d.]+, K = [\d.]+/);
    const progress = row.locator("[data-homoclinic-progress]");
    const first = await progress.innerText();
    assert.match(first, /^Computing the homoclinic curve: \d+ points?, at k₂ = [\d.−]+, K = [\d.−]+$/);
    await page.waitForFunction(
      (text) => document.querySelector("[data-homoclinic-progress]")?.textContent !== text,
      first,
      { timeout: 5000 },
    );
    assert.equal(await row.count(), 1, "the same row, updated in place");

    await row.getByRole("button", { name: /Cancel the homoclinic curve/ }).click();
    assert.equal(await homoclinicRows(page).count(), 0, "no row");
    await page.waitForTimeout(2500);
    assert.equal(await homoclinicRows(page).count(), 0, "no row once the cancelled job has ended");
    assert.doesNotMatch(await page.locator(".legend-bar").innerText(), /Homoclinic/);
    assert.equal(await canvasPixels(page), before, "nothing drawn");
    assert.equal(await inspector(page).locator("[data-homoclinic-followed]").count(), 0);

    await follow(page);
    await page.waitForSelector("[data-homoclinic-status=done]", { timeout: 15000 });
    const text = await homoclinicRows(page).innerText();
    assert.match(text, /Homoclinic curve/);
    assert.match(text, /\d+ points · (one end|both ends|the end) /);
    assert.match(text, /σ (changes sign at|stays)/);
    assert.match(text, /Shilnikov chaos|No Shilnikov chaos/);
    assert.match(await page.locator(".legend-bar").innerText(), /Homoclinic \(Hom\)/);
    assert.notEqual(await canvasPixels(page), before, "the curve is drawn");
    assert.equal(await inspector(page).locator("[data-homoclinic-followed]").count(), 1);
    assert.equal(
      await inspector(page).locator("[data-curves] > li:not([data-homoclinic-status])").count(),
      4,
      "the fold and Hopf rows stay",
    );

    await homoclinicRows(page).getByRole("button", { name: /Remove the homoclinic curve/ }).click();
    assert.equal(await homoclinicRows(page).count(), 0);
    await page.waitForTimeout(300);
    assert.doesNotMatch(await page.locator(".legend-bar").innerText(), /Homoclinic/);
    assert.equal(await canvasPixels(page), before, "removing the row removes the curve");
  } finally {
    await browser.close();
  }
});
