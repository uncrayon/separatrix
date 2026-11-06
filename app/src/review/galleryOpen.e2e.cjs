// The launch gallery opens project files: Open…, its Mod+O shortcut, the
// Recent rows and a file dropped on the window, in the browser preview over
// the in-memory files of `window.__sepxMock`.
// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5601/";
const FIXTURES = path.join(__dirname, "../../../crates/project/tests/fixtures");
const fixtureText = (name) => fs.readFileSync(path.join(FIXTURES, name), "utf8");

const LORENZ = "/mock/lorenz-text-mode.sepx";
const BRANCH = "/mock/course/continuation-view.sepx";
const FILES = {
  [LORENZ]: fixtureText("lorenz-text-mode.sepx"),
  [BRANCH]: fixtureText("continuation-view.sepx"),
};

async function open() {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await page.addInitScript((files) => {
      window.__sepxMock = {
        files: new Map(Object.entries(files)),
        launchFiles: [],
        saved: [],
        document: { title: "Separatrix", dirty: false },
      };
    }, FILES);
    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

const title = (page) => page.evaluate(() => window.__sepxMock.document.title);
const waitTitle = (page, want) => page.waitForFunction((want) => window.__sepxMock.document.title === want, want);
const nextOpen = (page, p) => page.evaluate((p) => (window.__sepxMock.nextOpenPath = p), p);
const openButton = (page) => page.locator(".gallery").getByRole("button", { name: "Open…", exact: true });
const recentRows = (page) => page.locator(".gallery .gallery-recent-row");

async function reopenGallery(page) {
  await page.locator(".topbar").getByRole("button", { name: "Templates…" }).click();
  await page.waitForSelector(".gallery");
}

test("the first-launch gallery has Open… and no Recent section; Open… opens the file and closes the gallery", async () => {
  const { browser, page } = await open();
  try {
    await assert.doesNotReject(openButton(page).waitFor(), "Open… on first launch");
    assert.equal(await page.locator(".gallery").getByRole("button", { name: "Close gallery" }).count(), 0, "first launch");
    assert.equal(await page.locator("#gallery-recent").count(), 0, "nothing opened yet");

    await nextOpen(page, LORENZ);
    await openButton(page).click();
    await page.waitForSelector(".gallery", { state: "detached" });
    await waitTitle(page, "lorenz-text-mode — Separatrix");
  } finally {
    await browser.close();
  }
});

test("Cancel in the Open dialog leaves the gallery open", async () => {
  const { browser, page } = await open();
  try {
    const before = await title(page);
    await nextOpen(page, null);
    await openButton(page).click();
    await page.waitForTimeout(300);
    assert.equal(await page.locator(".gallery").count(), 1);
    assert.equal(await title(page), before);
  } finally {
    await browser.close();
  }
});

test("Mod+O works while the gallery is open", async () => {
  const { browser, page } = await open();
  try {
    await nextOpen(page, LORENZ);
    await page.keyboard.press("Control+O");
    await page.waitForSelector(".gallery", { state: "detached" });
    await waitTitle(page, "lorenz-text-mode — Separatrix");
  } finally {
    await browser.close();
  }
});

test("Recent lists opened files newest first, and a row reopens its file", async () => {
  const { browser, page } = await open();
  try {
    await nextOpen(page, LORENZ);
    await openButton(page).click();
    await waitTitle(page, "lorenz-text-mode — Separatrix");
    await reopenGallery(page);
    await nextOpen(page, BRANCH);
    await openButton(page).click();
    await waitTitle(page, "continuation-view — Separatrix");

    await reopenGallery(page);
    await recentRows(page).first().waitFor();
    const rows = await recentRows(page).evaluateAll((els) =>
      els.map((el) => [el.querySelector(".gallery-recent-name").textContent, el.querySelector(".gallery-recent-folder").textContent]),
    );
    assert.deepEqual(rows, [
      ["continuation-view.sepx", "/mock/course"],
      ["lorenz-text-mode.sepx", "/mock"],
    ]);
    const sections = await page.locator(".gallery .gallery-chapter-head").allTextContents();
    assert.equal(sections[0], "Recent", "above Chapter 1");
    assert.match(sections[1], /^Chapter 2/);

    await recentRows(page).filter({ hasText: "lorenz-text-mode" }).click();
    await page.waitForSelector(".gallery", { state: "detached" });
    await waitTitle(page, "lorenz-text-mode — Separatrix");

    await reopenGallery(page);
    await recentRows(page).first().waitFor();
    assert.deepEqual(await page.locator(".gallery .gallery-recent-name").allTextContents(), [
      "lorenz-text-mode.sepx",
      "continuation-view.sepx",
    ]);
  } finally {
    await browser.close();
  }
});

test("a file that no longer exists is not listed", async () => {
  const { browser, page } = await open();
  try {
    await nextOpen(page, BRANCH);
    await openButton(page).click();
    await waitTitle(page, "continuation-view — Separatrix");
    await page.evaluate((p) => window.__sepxMock.files.delete(p), BRANCH);
    await reopenGallery(page);
    await page.waitForTimeout(300);
    assert.equal(await recentRows(page).count(), 0);
    assert.equal(await page.locator("#gallery-recent").count(), 0);
  } finally {
    await browser.close();
  }
});

test("a file dropped on the window while the gallery is open opens it and closes the gallery", async () => {
  const { browser, page } = await open();
  try {
    await page.evaluate((p) => window.dispatchEvent(new CustomEvent("sepx:project-open-requested", { detail: { path: p } })), BRANCH);
    await page.waitForSelector(".gallery", { state: "detached" });
    await waitTitle(page, "continuation-view — Separatrix");
  } finally {
    await browser.close();
  }
});
