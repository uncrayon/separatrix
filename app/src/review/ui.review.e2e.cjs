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
    // Template view x in [-4, 4], y in [-5, 5], centred on the origin. The plot
    // fills the canvas (viewport.ts MARGIN is 0 on every side) and equal scales
    // widen the view about its centre, so the origin is the canvas centre.
    // Keep this in step with MARGIN in src/canvas/viewport.ts.
    const MARGIN = { left: 0, right: 0, top: 0, bottom: 0 };
    const w = b.width - MARGIN.left - MARGIN.right;
    const h = b.height - MARGIN.top - MARGIN.bottom;
    await page.mouse.click(b.x + MARGIN.left + w / 2, b.y + MARGIN.top + h / 2);
    await page.waitForTimeout(300);
    // The inspector marks the selection with a pressed row in its Fixed points
    // list, named by what the user reads there ("Unstable spiral at (0.000, 0.000)").
    const fixedPoint = await page.evaluate(() => {
      const b = document.querySelector('.panel-right ul[aria-label="Fixed points"] .ds-fp-row[aria-pressed="true"]');
      return b ? b.getAttribute("aria-label") : "";
    });
    // Today the start dot of the template's IC (0.1, 0) is 9px away and wins
    // the hit test, so the click selects "Trajectory 1" instead.
    assert.match(fixedPoint, /^(Unstable|Stable) (spiral|node)\b.* at \(0\.000, 0\.000\)/);
  } finally {
    await browser.close();
  }
});
