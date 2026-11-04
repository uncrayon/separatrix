// CommonJS so Playwright can come from a global install (NODE_PATH).

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
    await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
    await page.waitForTimeout(300);
    const fixedPoint = await page.evaluate(() => {
      const b = document.querySelector('.panel-right ul[aria-label="Fixed points"] .ds-fp-row[aria-pressed="true"]');
      return b ? b.getAttribute("aria-label") : "";
    });
    assert.match(fixedPoint, /^(Unstable|Stable) (spiral|node)\b.* at \(0\.000, 0\.000\)/);
  } finally {
    await browser.close();
  }
});

async function selectFixedPoint(page, name) {
  const row = page.locator(`.panel-right ul[aria-label="Fixed points"] .ds-fp-row[aria-label="${name}"]`);
  await row.waitFor();
  await row.click();
  await page.waitForTimeout(300);
}
const eigenSwitch = (page) => page.getByRole("switch", { name: "Show eigenvectors" });
const legendText = (page) =>
  page.evaluate(() => document.querySelector('.legend-bar ul[aria-label="Legend"]')?.textContent ?? "");

test("Show eigenvectors: the saddle's switch draws stable and unstable lines, with legend keys", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await selectFixedPoint(page, "Saddle at (1.000, 1.000)");
    const sw = eigenSwitch(page);
    assert.equal(await sw.getAttribute("aria-checked"), "false");
    assert.ok(!(await legendText(page)).includes("eigenvector"), "no eigenvector keys while off");
    await sw.click();
    await page.waitForTimeout(400);
    assert.equal(await sw.getAttribute("aria-checked"), "true");
    const legend = await legendText(page);
    assert.ok(legend.includes("Unstable eigenvector"), legend);
    assert.ok(legend.includes("Stable eigenvector"), legend);
    const label = await page.locator("canvas.plot-canvas").getAttribute("aria-label");
    assert.match(label, /Eigenvector directions are drawn through the fixed points\./);
  } finally {
    await browser.close();
  }
});

test("Show eigenvectors: a spiral's hint only promises other points' lines when there are some", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await selectFixedPoint(page, "Saddle at (1.000, 1.000)");
    await eigenSwitch(page).click();
    await page.waitForTimeout(300);
    await page.getByRole("button", { name: /Templates/ }).click();
    await page.waitForSelector(".gallery");
    await pick(page, "Van der Pol");
    const row = page.locator('.panel-right ul[aria-label="Fixed points"] .ds-fp-row').first();
    await row.click();
    await page.waitForTimeout(300);
    const sw = eigenSwitch(page);
    assert.equal(await sw.getAttribute("aria-checked"), "true");
    const hint = await page.locator(`#${(await sw.getAttribute("aria-describedby")).replace(/:/g, "\\:")}`).textContent();
    assert.match(hint, /no real eigenvectors\./);
    assert.ok(!hint.includes("Other fixed points show theirs."), hint);
    assert.match(hint, /No fixed point in this system has real eigenvectors\./);
    assert.ok(!(await legendText(page)).includes("eigenvector"), "nothing is drawn, so no keys");
  } finally {
    await browser.close();
  }
});

test("trajectory colours restart at traj-1 after Delete all trajectories", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await page.locator('.panel-right [aria-label="Delete all trajectories"]').click();
    await page.waitForTimeout(200);
    assert.equal(await page.locator('.panel-right ul[aria-label="Trajectories"] .ds-traj-row').count(), 0);
    await page.locator('.canvas-toolbar [aria-label="Add trajectory"]').click();
    const b = await page.locator("canvas.plot-canvas").boundingBox();
    await page.mouse.click(b.x + b.width * 0.3, b.y + b.height * 0.4);
    await page.waitForTimeout(400);
    const rows = page.locator('.panel-right ul[aria-label="Trajectories"] .ds-traj-row');
    assert.equal(await rows.count(), 1);
    const style = (await rows.first().getAttribute("style")).replace(/\s+/g, "");
    assert.equal(style, "--traj:var(--traj-1);");
  } finally {
    await browser.close();
  }
});

test("trajectory rows: the show/hide eye toggle has no outline or fill when on", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    const looks = await page.evaluate(() =>
      [...document.querySelectorAll('.panel-right .ds-traj-actions .ds-icon-button[aria-pressed="true"]')].map((b) => {
        const cs = getComputedStyle(b);
        return [cs.borderTopColor, cs.backgroundColor];
      }),
    );
    assert.ok(looks.length > 0, "the template has visible trajectories");
    for (const [border, bg] of looks) {
      assert.equal(border, "rgba(0, 0, 0, 0)");
      assert.equal(bg, "rgba(0, 0, 0, 0)");
    }
    const toolbar = await page.evaluate(() => {
      const b = document.querySelector('.canvas-toolbar .ds-icon-button:not(.ds-icon-button--tool)[aria-pressed="true"]');
      return b ? getComputedStyle(b).borderTopColor : null;
    });
    assert.ok(toolbar && toolbar !== "rgba(0, 0, 0, 0)", String(toolbar));
  } finally {
    await browser.close();
  }
});

test("Text mode: editing a template tags it Edited, and undoing the edit removes the tag", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    const name = await page.locator(".topbar .doc-name").textContent();
    assert.equal(await page.locator(".topbar .doc-edited").count(), 0);
    await page.getByRole("radio", { name: "Text" }).click();
    const first = page.locator(".panel-left input.ds-equation-editor-field").first();
    await first.click();
    await page.keyboard.press("End");
    await page.keyboard.type("1");
    await page.waitForTimeout(800);
    assert.equal(await page.locator(".topbar .doc-edited").count(), 1, "Edited appears");
    assert.equal(await page.locator(".topbar .doc-name").textContent(), name, "the name stays the template's");
    await page.keyboard.press("Backspace");
    await page.waitForTimeout(800);
    assert.equal(await page.locator(".topbar .doc-edited").count(), 0, "Edited goes away");
    assert.equal(await page.locator(".topbar .doc-name").textContent(), name);
  } finally {
    await browser.close();
  }
});

test("the plot's accessible name reads near-zero fixed-point coordinates as 0", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    const label = await page.locator("canvas.plot-canvas").getAttribute("aria-label");
    const fixed = label.slice(label.indexOf("Fixed points:"));
    assert.ok(!/e[-−+]?\d/.test(fixed), fixed);
    assert.match(fixed, /unstable node at \(0, 0\)/);
    assert.match(fixed, /saddle at \(1, 1\)/);
  } finally {
    await browser.close();
  }
});
