// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5601/";

/**
 * Replays what WebKitGTK sends, as traced in the real app: the mouse press
 * that opens a <select>'s popup never gets its pointerup, and the next press
 * brings mousedown, pointerup and click but no pointerdown.
 */
const WEBKITGTK_SELECT_PRESS = () => {
  let stuck = false;
  window.addEventListener(
    "pointerdown",
    (e) => {
      if (!e.isTrusted) return;
      if (stuck) {
        stuck = false;
        e.stopImmediatePropagation();
      } else if (e.target instanceof HTMLSelectElement) {
        stuck = true;
      }
    },
    true,
  );
  window.addEventListener("pointerup", (e) => stuck && e.isTrusted && e.stopImmediatePropagation(), true);
};

const STREAMS = ["Chromium", "WebKitGTK"];

async function open(stream, title) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    if (stream === "WebKitGTK") await page.addInitScript(WEBKITGTK_SELECT_PRESS);
    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    await page.locator(".gallery .card", { hasText: title }).first().click();
    await page.waitForSelector(".gallery", { state: "detached" });
    await page.waitForTimeout(600);
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

/** A mouse press on the select, as when it opens its popup, then the choice. */
async function choose(select, option) {
  await select.click();
  await select.selectOption(option);
}

for (const stream of STREAMS) {
  test(`${stream}: the first click on the phase plane after choosing an integration method adds a trajectory`, async () => {
    const { browser, page } = await open(stream, "Van der Pol");
    try {
      const rows = page.locator(".ic-row");
      const before = await rows.count();
      await choose(page.getByLabel("Integration method"), { label: "Runge–Kutta 4" });
      await page.waitForTimeout(300);
      const box = await page.locator("canvas.plot-canvas").boundingBox();
      await page.mouse.click(box.x + box.width * 0.8, box.y + box.height * 0.85);
      await page.waitForTimeout(300);
      assert.equal(await rows.count(), before + 1, "the first click added a trajectory");
    } finally {
      await browser.close();
    }
  });

  test(`${stream}: the first click on a branch after choosing the continuation parameter sets it`, async () => {
    const { browser, page } = await open(stream, "Sel'kov");
    try {
      await page.locator(".panel-right [role=radiogroup][aria-label=View]").getByRole("radio", { name: "Continuation", exact: true }).click();
      await choose(page.locator(".panel-right").getByLabel("Parameter", { exact: true }), { label: "b" });
      await page.waitForFunction(() => {
        const list = document.querySelector("[data-branches]");
        return !!list && !list.textContent.includes("Computing…");
      });
      await page.waitForTimeout(300);
      const canvas = page.locator(".branch-canvas");
      const box = await canvas.boundingBox();
      const column = 0.75;
      const row = await canvas.evaluate((c, column) => {
        const x = Math.round(c.width * column);
        const px = c.getContext("2d").getImageData(x, 0, 1, c.height).data;
        for (let y = 0; y < c.height; y++) {
          const [r, g, b] = px.subarray(4 * y, 4 * y + 3);
          if (b - r > 80) return y / c.height;
        }
        return null;
      }, column);
      assert.notEqual(row, null, "the branch crosses the probed column");
      const value = page.getByRole("textbox", { name: "Value of b", exact: true });
      const before = await value.inputValue();
      await page.mouse.click(box.x + box.width * column, box.y + box.height * row);
      await page.waitForTimeout(300);
      const after = await value.inputValue();
      assert.notEqual(after, before, "the first click on the branch set b");
      assert.ok(Math.abs(Number(after) - 1.1) < 0.1, `b moved to the clicked point, got ${after}`);
    } finally {
      await browser.close();
    }
  });
}
