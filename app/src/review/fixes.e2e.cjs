const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const URL = process.env.UI_URL || "http://localhost:5198/";

async function open(viewport = { width: 1440, height: 900 }) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport });
    await page.goto(URL);
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

const selectedRow = (page, list) =>
  page.evaluate((list) => {
    const b = document.querySelector(
      `.panel-right ul[aria-label="${list}"] :is(.ds-traj-select, .ds-fp-row)[aria-pressed="true"]`,
    );
    return b ? b.getAttribute("aria-label") || b.textContent : "";
  }, list);
const selectedTrajectory = (page) => selectedRow(page, "Trajectories");
const selectedFixedPoint = (page) => selectedRow(page, "Fixed points");
const FIXED_POINT_NAME =
  /^(Saddle|Center|Stable|Unstable|Half-stable|Non-hyperbolic|Non-isolated|Sink|Source)\b.* at (\(|\S+\* = )/;
const icCount = (page) => page.locator(".ic-row").count();

test("picking a template from the keyboard leaves focus on the plot, not the page body", async () => {
  const { browser, page } = await open();
  try {
    await page.keyboard.press("Tab");
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
    const button = page.getByRole("button", { name: /^Axes:/ });
    await button.focus();
    await page.keyboard.press("ArrowDown");
    await page.waitForSelector("[role=menu]");
    const checked = await page.evaluate(() => document.activeElement.getAttribute("aria-checked"));
    assert.equal(checked, "true", "opening the menu focuses the current choice");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await page.waitForSelector("[role=menu]", { state: "detached" });
    const back = await page.evaluate(() => document.activeElement.getAttribute("aria-label"));
    assert.match(back, /^Axes: t horizontal/, "focus returns to the menu button, which names the new axis");
    const xLabel = await page.textContent(".plot-wrap .axis-label-x");
    assert.match(xLabel, /t/);
    assert.doesNotMatch(xLabel, /x/);
    await page.keyboard.press("ArrowDown");
    await page.waitForSelector("[role=menu]");
    await page.keyboard.press("Escape");
    await page.waitForSelector("[role=menu]", { state: "detached" });
    assert.match(
      await page.evaluate(() => document.activeElement.getAttribute("aria-label")),
      /^Axes: t horizontal/,
      "Escape returns focus to the menu button and changes nothing",
    );
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
    assert.match(await selectedTrajectory(page), /^Trajectory \d+, start \(/);
    await page.keyboard.press("f");
    await page.waitForTimeout(200);
    assert.match(await selectedFixedPoint(page), FIXED_POINT_NAME);
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
    assert.equal(await page.locator(".ds-diagnostic--error").count(), 0);
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

test("an equation line starts editing wherever it is clicked, not only on the typeset maths", async () => {
  const { browser, page } = await open();
  try {
    await page.locator(".gallery .card.blank").click();
    await page.waitForSelector(".gallery", { state: "detached" });
    await page.waitForTimeout(600);
    const box = await page.locator(".panel-left .ds-equation-editor-field").first().boundingBox();
    await page.mouse.click(box.x + box.width * 0.8, box.y + box.height / 2);
    await page.waitForTimeout(300);
    assert.equal(await page.evaluate(() => document.activeElement.tagName), "MATH-FIELD");
    const before = await page.evaluate(() => document.querySelector(".panel-left math-field").value);
    await page.keyboard.type("+1");
    await page.waitForTimeout(300);
    const after = await page.evaluate(() => document.querySelector(".panel-left math-field").value);
    assert.notEqual(after, before, "typing after the click edits the equation");
    assert.match(after, /\+1$/);
  } finally {
    await browser.close();
  }
});

test("Text mode: the status bar's error button moves focus to the bad line", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    await page.getByRole("radio", { name: "Text" }).click();
    const first = page.locator(".panel-left input.ds-equation-editor-field").first();
    await first.click();
    await page.keyboard.press("End");
    await page.keyboard.type("+");
    await page.waitForTimeout(800);
    await page.locator("canvas.plot-canvas").focus();
    const errors = page.locator(".statusbar .ds-statusbar-item--danger.ds-statusbar-item--button");
    assert.equal(await errors.count(), 1, "the status bar counts the error");
    await errors.focus();
    await page.keyboard.press("Enter");
    const focused = await page.evaluate(() => ({
      tag: document.activeElement.tagName,
      label: document.activeElement.getAttribute("aria-label"),
    }));
    assert.deepEqual(focused, { tag: "INPUT", label: "Equation 1" });
  } finally {
    await browser.close();
  }
});
