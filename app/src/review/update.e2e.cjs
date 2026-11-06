// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5601/";
const VERSION = JSON.parse(fs.readFileSync(path.join(__dirname, "../../package.json"), "utf8")).version;

async function open(update) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await page.addInitScript((update) => {
      window.__sepxMock = {
        files: new Map(),
        launchFiles: [],
        saved: [],
        document: { title: "Separatrix", dirty: false },
        ...(update ? { update } : {}),
      };
    }, update);
    await page.goto(BASE);
    await page.waitForSelector(".gallery");
    await page.locator(".gallery .card", { hasText: "Van der Pol" }).first().click();
    await page.waitForSelector(".gallery", { state: "detached" });
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

const versionButton = (page) => page.locator(".statusbar").getByRole("button", { name: `v${VERSION} . Check for updates` });
const panel = (page) => page.getByRole("dialog", { name: "Updates" });

test(`clicking v${VERSION} in the preview says updates are off`, async () => {
  const { browser, page } = await open(null);
  try {
    await versionButton(page).click();
    await assert.doesNotReject(panel(page).getByText("Updates are off in this build", { exact: true }).waitFor());
    assert.equal(await panel(page).getByRole("button").count(), 0, "nothing to install");
    await page.keyboard.press("Escape");
    await panel(page).waitFor({ state: "detached" });
    assert.equal(await versionButton(page).evaluate((b) => b === document.activeElement), true, "focus back on the button");
  } finally {
    await browser.close();
  }
});

test("a newer release offers Install and restart; a click elsewhere closes the panel", async () => {
  const { browser, page } = await open({ kind: "available", version: "1.0.1", notes: null, date: null });
  try {
    await versionButton(page).click();
    await assert.doesNotReject(panel(page).getByText("Separatrix 1.0.1 is available", { exact: true }).waitFor());
    await assert.doesNotReject(panel(page).getByRole("button", { name: "Install and restart" }).waitFor());
    await assert.doesNotReject(page.locator(".statusbar").getByText("Update available").waitFor(), "the button says so too");
    await page.locator(".topbar").click({ position: { x: 600, y: 10 } });
    await panel(page).waitFor({ state: "detached" });
  } finally {
    await browser.close();
  }
});

test("up to date, and a failed check, in plain words", async () => {
  for (const [update, want] of [
    [{ kind: "upToDate", version: VERSION }, "Separatrix is up to date"],
    [{ kind: "failed", message: "error sending request" }, "Couldn't check for updates. You can keep working offline."],
  ]) {
    const { browser, page } = await open(update);
    try {
      await versionButton(page).click();
      await assert.doesNotReject(panel(page).getByText(want, { exact: true }).waitFor(), want);
    } finally {
      await browser.close();
    }
  }
});

test("Install and restart asks about unsaved changes first; Cancel installs nothing", async () => {
  const { browser, page } = await open({ kind: "available", version: "1.0.1", notes: null, date: null });
  try {
    const value = page.getByRole("textbox", { name: "Value of mu" });
    await value.click();
    await value.fill("2.75");
    await value.press("Enter");
    await page.waitForFunction(() => window.__sepxMock.document.dirty);
    const installs = () => page.evaluate(() => window.__sepxMock.installs ?? 0);
    const confirm = page.getByRole("alertdialog");

    await versionButton(page).click();
    await panel(page).getByRole("button", { name: "Install and restart" }).click();
    await assert.doesNotReject(confirm.waitFor(), "the save question appears");
    assert.match(await confirm.textContent(), /Save changes to “Van der Pol/);
    assert.equal(await installs(), 0, "nothing installs while the question is open");
    await confirm.getByRole("button", { name: "Cancel" }).click();
    await confirm.waitFor({ state: "detached" });
    await page.waitForTimeout(200);
    assert.equal(await installs(), 0, "Cancel installs nothing");
    assert.equal(await page.evaluate(() => window.__sepxMock.document.dirty), true, "the edit is still there");

    if (!(await panel(page).isVisible())) await page.locator("[data-update-button]").click();
    await panel(page).getByRole("button", { name: "Install and restart" }).click();
    await confirm.getByRole("button", { name: "Don't save" }).click();
    await page.waitForFunction(() => window.__sepxMock.installs === 1);
  } finally {
    await browser.close();
  }
});

test("Install and restart on a saved document installs without asking", async () => {
  const { browser, page } = await open({ kind: "available", version: "1.0.1", notes: null, date: null });
  try {
    await versionButton(page).click();
    await panel(page).getByRole("button", { name: "Install and restart" }).click();
    await page.waitForFunction(() => window.__sepxMock.installs === 1);
    await assert.doesNotReject(panel(page).getByText("Installing Separatrix 1.0.1…", { exact: true }).waitFor());
  } finally {
    await browser.close();
  }
});
