// End-to-end checks for the fixes made after the UI review, driven with
// Playwright against the Vite dev server (browser preview: the JS mock).
//
// Run from app/ with a dev server up (same as ui.review.e2e.cjs):
//   npx vite --port 5198 &
//   UI_URL=http://localhost:5198/ NODE_PATH=$(npm root -g) node --test src/review/fixes.e2e.cjs

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const URL = process.env.UI_URL || "http://localhost:5198/";

async function open(viewport = { width: 1440, height: 900 }) {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport });
  await page.goto(URL);
  await page.waitForSelector(".gallery");
  return { browser, page };
}

async function pick(page, title) {
  await page.locator(".gallery .card", { hasText: title }).first().click();
  await page.waitForSelector(".gallery", { state: "detached" });
  await page.waitForTimeout(600);
}

const heading = (page) => page.textContent(".panel-right .section-head h2");
const icCount = (page) => page.locator(".ic-row").count();

test("picking a template from the keyboard leaves focus on the plot, not the page body", async () => {
  const { browser, page } = await open();
  try {
    await page.keyboard.press("Tab"); // first card
    await page.keyboard.press("Enter");
    await page.waitForSelector(".gallery", { state: "detached" });
    const focused = await page.evaluate(() => document.activeElement.className);
    assert.match(String(focused), /plot-canvas/);
  } finally {
    await browser.close();
  }
});

test("closing the gallery with Escape gives focus back to the Templates button", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await page.getByRole("button", { name: "Templates" }).focus();
    await page.keyboard.press("Enter");
    await page.waitForSelector(".gallery");
    // The app behind the dialog is inert while it is open.
    assert.equal(await page.evaluate(() => document.querySelector(".app").inert), true);
    await page.keyboard.press("Escape");
    await page.waitForSelector(".gallery", { state: "detached" });
    const label = await page.evaluate(() => document.activeElement.textContent);
    assert.match(label, /Templates/);
  } finally {
    await browser.close();
  }
});

test("the axis menu works with the arrow keys", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    const button = page.getByRole("button", { name: "Horizontal axis" });
    await button.focus();
    await page.keyboard.press("ArrowDown");
    await page.waitForSelector(".menu");
    const checked = await page.evaluate(() => document.activeElement.getAttribute("aria-checked"));
    assert.equal(checked, "true", "opening the menu focuses the current choice");
    await page.keyboard.press("ArrowDown"); // x -> y
    await page.keyboard.press("ArrowDown"); // y -> t
    await page.keyboard.press("Enter");
    await page.waitForSelector(".menu", { state: "detached" });
    const back = await page.evaluate(() => document.activeElement.getAttribute("aria-label"));
    assert.equal(back, "Horizontal axis", "focus returns to the menu button");
    const xLabel = await page.textContent(".plot-wrap .axis-label-x");
    assert.match(xLabel, /t/);
    assert.doesNotMatch(xLabel, /x/);
    // Escape closes without choosing.
    await page.keyboard.press("ArrowDown");
    await page.waitForSelector(".menu");
    await page.keyboard.press("Escape");
    await page.waitForSelector(".menu", { state: "detached" });
  } finally {
    await browser.close();
  }
});

test("the plot can add a starting point and select fixed points from the keyboard", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    const before = await icCount(page);
    await page.locator("canvas.plot-canvas").focus();
    await page.keyboard.press("ArrowLeft");
    assert.equal(await page.locator(".kbd-cursor").count(), 1, "the keyboard cursor is shown");
    await page.keyboard.press("Enter");
    await page.waitForTimeout(200);
    assert.equal(await icCount(page), before + 1);
    assert.match(await heading(page), /Trajectory/);
    await page.keyboard.press("f");
    await page.waitForTimeout(200);
    assert.match(await heading(page), /Fixed point/);
  } finally {
    await browser.close();
  }
});

test("browser preview: the Blank system card compiles", async () => {
  const { browser, page } = await open();
  try {
    await page.locator(".gallery .card.blank").click();
    await page.waitForSelector(".gallery", { state: "detached" });
    await page.waitForTimeout(600);
    assert.equal(await page.locator(".diag.error").count(), 0);
    assert.match(await page.textContent(".statusbar"), /fixed point/);
  } finally {
    await browser.close();
  }
});

test("status bar: the fixed-point count says it covers the view only", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    assert.match(await page.textContent(".statusbar"), /fixed points? in view/);
  } finally {
    await browser.close();
  }
});

test("1280x720 projector mode: every toolbar control is fully visible", async () => {
  const { browser, page } = await open({ width: 1280, height: 720 });
  try {
    await pick(page, "Lorenz");
    await page.getByRole("button", { name: "Projector mode" }).click();
    await page.waitForTimeout(300);
    const clipped = await page.evaluate(() => {
      const bar = document.querySelector(".canvas-toolbar").getBoundingClientRect();
      return [...document.querySelectorAll(".canvas-toolbar .chip, .canvas-toolbar button")]
        .map((el) => ({ text: el.textContent, r: el.getBoundingClientRect() }))
        .filter(({ r }) => r.right > bar.right + 0.5 || r.left < bar.left - 0.5)
        .map(({ text }) => text);
    });
    assert.deepEqual(clipped, []);
  } finally {
    await browser.close();
  }
});

test("Lorenz: the slice card folds to one line and opens again", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Lorenz");
    await page.getByRole("button", { name: "Fold the slice controls" }).click();
    assert.equal(await page.locator(".slice-card input[type=range]").count(), 0);
    await page.locator(".slice-folded").click();
    assert.equal(await page.locator(".slice-card input[type=range]").count(), 1);
  } finally {
    await browser.close();
  }
});
