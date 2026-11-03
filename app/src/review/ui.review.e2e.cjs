// UI review end-to-end checks, driven with Playwright against the Vite dev
// server (browser preview: the JS mock computes the gallery templates).
//
// Run from app/ with a dev server up:
//   npx vite --port 5198 &
//   UI_URL=http://localhost:5198/ NODE_PATH=$(npm root -g) node --test src/review/ui.review.e2e.cjs
//
// CommonJS + plain JS so `tsc --noEmit` (npm run build) never sees it, and
// Playwright can come from a global install (NODE_PATH).
//
// Each test asserts what the UI promises; a failing test is a review finding.

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

test("1024x768 (common projector): the centre column stays inside its grid track", async () => {
  const { browser, page } = await open({ width: 1024, height: 768 });
  try {
    await pick(page, "Lorenz");
    const m = await page.evaluate(() => {
      const box = (s) => document.querySelector(s).getBoundingClientRect();
      return { area: box(".canvas-area").right, plot: box(".plot-wrap").right, inspector: box(".panel-right").left };
    });
    // Today the legend's one-line help text forces the plot to 468px in a
    // 384px column: the canvas covers the inspector and swallows its clicks.
    assert.ok(m.plot <= m.inspector + 0.5, `plot ends at x=${m.plot}, inspector starts at x=${m.inspector}`);
    const onTop = await page.evaluate(([x, y]) => document.elementFromPoint(x, y).closest(".panel-right") !== null, [
      m.inspector + 20,
      300,
    ]);
    assert.ok(onTop, "a click 20px inside the inspector lands on the plot canvas");
  } finally {
    await browser.close();
  }
});

test("first-run gallery keeps keyboard focus inside the dialog", async () => {
  const { browser, page } = await open();
  try {
    const cards = await page.locator(".gallery .card").count();
    for (let i = 0; i < cards + 2; i++) await page.keyboard.press("Tab");
    const inside = await page.evaluate(() => !!document.activeElement.closest(".modal"));
    assert.ok(inside, "Tab past the last card moves focus to controls hidden behind the backdrop");
  } finally {
    await browser.close();
  }
});

test("Van der Pol: the fixed point at the origin can be selected with one click", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    const b = await page.locator("canvas.plot-canvas").boundingBox();
    // Template view x in [-4, 4], y in [-5, 5]; plot margins from viewport.ts (dark theme).
    const left = 48, top = 14, w = b.width - 48 - 14, h = b.height - 14 - 30;
    await page.mouse.click(b.x + left + w / 2, b.y + top + h / 2);
    await page.waitForTimeout(300);
    const heading = await page.textContent(".panel-right .section-head h2");
    // Today the start dot of the template's IC (0.1, 0) is 9px away and wins
    // the hit test, so the click selects "Trajectory 1" instead.
    assert.match(heading, /Fixed point/);
  } finally {
    await browser.close();
  }
});
