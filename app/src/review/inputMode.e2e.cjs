// Input modes in the browser preview. CommonJS so Playwright can come from a
// global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { chromium } = require("playwright");

const URL = process.env.UI_URL || "http://localhost:5198/";
const TEMPLATES = require(path.join(__dirname, "../templates/templates.json"));
const TEXT = require(path.join(__dirname, "../templates/templates.text.generated.json"));

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

async function pickCard(page, card) {
  await card.click();
  await page.waitForSelector(".gallery", { state: "detached" });
  await page.waitForTimeout(500);
}

const pick = (page, title) => pickCard(page, page.locator(".gallery .card", { hasText: title }).first());

async function openGallery(page) {
  await page.getByRole("button", { name: /Templates/ }).click();
  await page.waitForSelector(".gallery");
}

async function setMode(page, name) {
  await page.getByRole("radio", { name }).click();
  await page.waitForFunction(
    (name) => document.querySelector(`[role=radio][aria-checked=true]`)?.textContent === name,
    name,
  );
  await page.waitForTimeout(300);
}

const textInputs = (page) =>
  page.$$eval(".panel-left input.ds-equation-editor-field", (els) => els.map((e) => e.value));
const mathFields = (page) => page.$$eval(".panel-left math-field", (els) => els.map((e) => e.value));
const edited = (page) => page.locator(".topbar .doc-edited").count();

test("every template: Text shows its generated text, and LaTeX comes back exactly as authored", async () => {
  const { browser, page } = await open();
  try {
    const cards = [...TEMPLATES.map((t) => ({ id: t.id, title: t.title })), { id: "blank", title: null }];
    for (const [k, t] of cards.entries()) {
      if (k > 0) await openGallery(page);
      const card = t.title
        ? page.locator(".gallery .card", { hasText: t.title }).first()
        : page.locator(".gallery .card.blank");
      await pickCard(page, card);
      const authored = await mathFields(page);
      const template = TEMPLATES.find((x) => x.id === t.id);
      if (template) assert.deepEqual(authored, template.equations, `${t.id}: loads as authored`);
      await setMode(page, "Text");
      assert.deepEqual(await textInputs(page), TEXT[t.id], `${t.id}: text`);
      assert.equal(await edited(page), 0, `${t.id}: not Edited in text mode`);
      assert.equal(await page.locator(".ds-equation-editor .ds-diagnostic--error").count(), 0, `${t.id}: compiles`);
      await setMode(page, "LaTeX");
      assert.deepEqual(await mathFields(page), authored, `${t.id}: back to LaTeX`);
      assert.equal(await edited(page), 0, `${t.id}: not Edited back in LaTeX`);
    }
  } finally {
    await browser.close();
  }
});

test("the input mode is remembered, and a template opens in it", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    await setMode(page, "Text");
    await openGallery(page);
    await pick(page, "Lorenz");
    const lorenz = TEMPLATES.find((t) => t.id === "lorenz");
    assert.deepEqual(await textInputs(page), TEXT.lorenz);
    assert.equal(await page.locator(".panel-left math-field").count(), 0);
    await page.reload();
    await page.waitForSelector(".gallery");
    await pick(page, "Lorenz");
    assert.deepEqual(await textInputs(page), TEXT.lorenz, "still text mode after a reload");
    await setMode(page, "LaTeX");
    assert.deepEqual(await mathFields(page), lorenz.equations, "converted back to the template's LaTeX");
  } finally {
    await browser.close();
  }
});

test("switching modes keeps a parameter the user set", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    const value = page.getByRole("textbox", { name: "Value of mu" });
    await value.click();
    await value.fill("3.25");
    await value.press("Enter");
    await page.locator("canvas.plot-canvas").focus();
    await page.waitForTimeout(300);
    const slider = page.getByRole("slider", { name: "Parameter mu" });
    assert.equal(await slider.inputValue(), "3.25");
    await setMode(page, "Text");
    assert.equal(await slider.inputValue(), "3.25", "kept in text mode");
    await setMode(page, "LaTeX");
    assert.equal(await slider.inputValue(), "3.25", "kept back in LaTeX");
  } finally {
    await browser.close();
  }
});

test("a line the converter could not convert is kept as typed, with a note", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    await setMode(page, "Text");
    const second = page.locator(".panel-left input.ds-equation-editor-field").nth(1);
    await second.click();
    await page.keyboard.press("End");
    await page.keyboard.type(" + 1");
    await page.waitForTimeout(400);
    await setMode(page, "LaTeX");
    const fields = await mathFields(page);
    assert.equal(fields[0], "\\dot{x}=y", "the template line converts");
    assert.equal(fields[1], `${TEXT["van-der-pol"][1]} + 1`, "the edited line is kept as typed");
    const notes = page.locator(".ds-equation-editor-lines .ds-diagnostic--info");
    assert.equal(await notes.count(), 1);
    assert.match(await notes.textContent(), /converts template lines only/);
  } finally {
    await browser.close();
  }
});

test("Nullclines: the toggle is there in a time-series view of a 2-D system, where they are drawn", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    const toggle = page.locator('.canvas-toolbar button[aria-label="Nullclines"]');
    assert.equal(await toggle.count(), 1);
    const button = page.getByRole("button", { name: /^Axes:/ });
    await button.focus();
    await page.keyboard.press("ArrowDown");
    await page.waitForSelector("[role=menu]");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await page.waitForSelector("[role=menu]", { state: "detached" });
    assert.match(await button.getAttribute("aria-label"), /^Axes: t horizontal/);
    assert.equal(await toggle.count(), 1, "the toggle is still shown");
    assert.equal(await toggle.getAttribute("aria-pressed"), "true");
    await toggle.click();
    assert.equal(await toggle.getAttribute("aria-pressed"), "false", "and it turns them off");
  } finally {
    await browser.close();
  }
});
