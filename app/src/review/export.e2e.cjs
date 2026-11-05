// The export dialog in the browser preview (mocked core), opened over the
// studio's real state by the dev-server harness (src/review/export.harness.html).
// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5198/";
const harness = (q = "") => new URL(`src/review/export.harness.html${q}`, BASE).href;

async function open(q = "") {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    await page.goto(harness(q));
    await page.waitForSelector("[data-harness-ready=yes]", { state: "attached" });
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

async function openDialog(page) {
  await page.click("[data-harness-open]");
  await page.waitForSelector(".export-dialog[role=dialog]");
}

const dialog = (page) => page.locator(".export-dialog");
const mock = (page, expr) => page.evaluate(`(() => { const m = window.__sepxMock; return ${expr}; })()`);
const snippet = (page) => page.locator(".ds-export-snippet").innerText();

async function format(page, name) {
  await dialog(page).getByRole("radio", { name, exact: true }).click();
}

/** Non-background pixels in the preview canvas, sampled. */
async function inked(page) {
  return page.$eval(".export-dialog .ds-export-preview-well canvas", (c) => {
    const ctx = c.getContext("2d");
    const { data } = ctx.getImageData(0, 0, c.width, c.height);
    const bg = [data[0], data[1], data[2]];
    let n = 0;
    for (let i = 0; i < data.length; i += 16) {
      if (Math.abs(data[i] - bg[0]) + Math.abs(data[i + 1] - bg[1]) + Math.abs(data[i + 2] - bg[2]) > 60) n++;
    }
    return { n, bg };
  });
}

test("opens on LaTeX with the preview drawn, and Copy LaTeX copies the snippet", async () => {
  const { browser, page } = await open();
  try {
    await openDialog(page);
    const d = dialog(page);
    assert.equal(await d.getAttribute("aria-modal"), "true");
    assert.equal(await page.locator("#" + (await d.getAttribute("aria-labelledby"))).innerText(), "Export figure");
    assert.match(await page.locator(".ds-export-sub").innerText(), /^Rabbits vs sheep, phase plane x–y, as you see it now$/);
    assert.equal(await page.evaluate(() => document.activeElement?.textContent), "LaTeX (TikZ/pgfplots)", "focus on the format");
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{axis}"));
    await page.waitForTimeout(400);
    const ink = await inked(page);
    assert.ok(ink.n > 200, `the preview is drawn (${ink.n} inked samples)`);
    assert.deepEqual(ink.bg, [255, 255, 255], "projector palette by default");
    assert.match(await page.locator(".ds-export-preview-well canvas").getAttribute("aria-label"), /^Preview: Rabbits vs sheep with direction field, nullclines, \d+ trajectories and \d+ fixed points$/);
    assert.match(await page.locator(".ds-export-foot-note").innerText(), /Save… writes rabbits-vs-sheep\.tex/);
    const primary = d.locator(".ds-export-actions .ds-button--primary");
    assert.equal((await primary.innerText()).trim(), "Copy LaTeX");
    await primary.click();
    await page.waitForFunction(() => document.querySelector(".ds-export-actions .ds-button--primary")?.textContent.includes("Copied"));
    assert.equal(await mock(page, "m.clipboardText"), (await snippet(page)).replace(/\n?$/, "\n"));
    await page.waitForFunction(() => [...document.querySelectorAll(".export-dialog [aria-live]")].some((e) => e.textContent === "LaTeX copied"));
    await page.waitForFunction(() => document.querySelector(".ds-export-actions .ds-button--primary")?.textContent.trim() === "Copy LaTeX", null, { timeout: 4000 });
    // Comment lines are dimmed.
    assert.ok((await page.locator(".ds-export-snippet .ds-export-cm").count()) >= 2);
  } finally {
    await browser.close();
  }
});

test("options change the snippet and preview; a bad size is flagged and the last good one kept", async () => {
  const { browser, page } = await open();
  try {
    await openDialog(page);
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("width=8cm"));
    const width = page.getByRole("textbox", { name: "Width in cm" });
    await width.fill("12");
    await width.press("Enter");
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("width=12cm"));
    await width.fill("0.5");
    await width.press("Tab");
    await page.getByText("Use a width between 1 and 40 cm.").waitFor();
    assert.equal(await page.locator(".ds-numfield.ds-numfield:has([aria-invalid=true])").count(), 1);
    assert.ok((await snippet(page)).includes("width=12cm"), "the last valid size stays");
    await dialog(page).getByRole("checkbox", { name: "Standalone document" }).check();
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\documentclass"));
    await dialog(page).getByRole("radio", { name: "Dark" }).click();
    await page.waitForTimeout(500);
    assert.deepEqual((await inked(page)).bg, [10, 14, 16], "dark palette");
  } finally {
    await browser.close();
  }
});

test("Equations: typeset preview from the core, notation and numbering", async () => {
  const { browser, page } = await open();
  try {
    await openDialog(page);
    await dialog(page).getByRole("radio", { name: "Equations" }).click();
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{align}"));
    await page.waitForSelector(".export-equations .ML__latex, .export-equations math, .export-equations span");
    assert.match(await page.locator(".ds-export-foot-note").innerText(), /rabbits-vs-sheep-equations\.tex/);
    await dialog(page).getByRole("checkbox", { name: "Number the equations" }).uncheck();
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{align*}"));
    await dialog(page).getByRole("radio", { name: "Fixed points" }).click();
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\toprule"));
    await dialog(page).locator("[data-export-action=save]").click();
    await page.waitForFunction(() => window.__sepxMock.saved.length === 1);
    assert.equal(await mock(page, "m.saved[0].name"), "rabbits-vs-sheep-fixed-points.tex");
  } finally {
    await browser.close();
  }
});

test("SVG: Save… writes well-formed SVG with text as text; Copy SVG copies it", async () => {
  const { browser, page } = await open();
  try {
    await openDialog(page);
    await format(page, "SVG");
    assert.equal(await page.locator(".ds-export-snippet").count(), 0, "no code block for SVG");
    const save = dialog(page).locator(".ds-export-actions .ds-button--primary");
    assert.equal((await save.innerText()).trim(), "Save…");
    await page.waitForFunction(() => !document.querySelector(".ds-export-actions .ds-button--primary")?.disabled);
    await save.click();
    await page.waitForFunction(() => window.__sepxMock.saved.length === 1);
    const res = await page.evaluate(() => {
      const f = window.__sepxMock.saved[0];
      const doc = new DOMParser().parseFromString(f.contents, "image/svg+xml");
      return {
        name: f.name,
        error: doc.querySelector("parsererror")?.textContent ?? null,
        root: doc.documentElement.nodeName,
        width: doc.documentElement.getAttribute("width"),
        texts: doc.querySelectorAll("text").length,
        tspans: doc.querySelectorAll("text tspan").length,
        raw: f.contents,
      };
    });
    assert.equal(res.name, "rabbits-vs-sheep.svg");
    assert.equal(res.error, null);
    assert.equal(res.root, "svg");
    assert.equal(res.width, "8cm");
    assert.ok(res.texts > 6, "tick labels and axis names are text");
    assert.ok(res.tspans >= 2, "maths names as tspans");
    assert.doesNotMatch(res.raw, /baseline-shift|dominant-baseline|<image/);
    assert.match(await page.locator(".export-dialog [aria-live=polite]").last().innerText(), /Saved rabbits-vs-sheep\.svg/);
    await dialog(page).getByRole("button", { name: "Copy SVG" }).click();
    await page.waitForFunction(() => window.__sepxMock.clipboardText?.startsWith("<?xml"));
  } finally {
    await browser.close();
  }
});

test("PNG: 3× scale gives 3× the pixels with pHYs at 288 dpi; Copy image; clipboard hint", async () => {
  const { browser, page } = await open();
  try {
    await openDialog(page);
    await format(page, "PNG");
    await page.getByText("Copy image pastes at screen size; Save… keeps the physical size.").waitFor();
    await dialog(page).getByRole("radio", { name: "3×" }).click();
    assert.match(await page.locator(".ds-export-preview-cap").innerText(), /2400 × 1920 px, 288 dpi/);
    await page.waitForFunction(() => !document.querySelector(".ds-export-actions .ds-button--primary")?.disabled);
    await dialog(page).locator(".ds-export-actions .ds-button--primary").click();
    await page.waitForFunction(() => window.__sepxMock.saved.length === 1, null, { timeout: 15000 });
    const png = await page.evaluate(() => {
      const f = window.__sepxMock.saved[0];
      const b = f.contents;
      const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
      const types = [];
      let ppm = null;
      for (let p = 8; p + 12 <= b.length; ) {
        const len = dv.getUint32(p);
        const type = String.fromCharCode(b[p + 4], b[p + 5], b[p + 6], b[p + 7]);
        types.push(type);
        if (type === "pHYs") ppm = dv.getUint32(p + 8);
        p += 12 + len;
      }
      return { name: f.name, sig: Array.from(b.slice(0, 4)), w: dv.getUint32(16), h: dv.getUint32(20), types, ppm };
    });
    assert.equal(png.name, "rabbits-vs-sheep.png");
    assert.deepEqual(png.sig, [137, 80, 78, 71]);
    assert.deepEqual([png.w, png.h], [2400, 1920]);
    assert.deepEqual(png.types.slice(0, 2), ["IHDR", "pHYs"]);
    assert.equal(Math.round(png.ppm * 0.0254), 288);
    await dialog(page).getByRole("button", { name: "Copy image" }).click();
    await page.waitForFunction(() => window.__sepxMock.clipboardPngBytes > 1000, null, { timeout: 15000 });
  } finally {
    await browser.close();
  }
});

test("CSV: first rows shown, trajectories chosen, Save… writes .csv and .meta.json", async () => {
  const { browser, page } = await open();
  try {
    await openDialog(page);
    await format(page, "CSV");
    await page.waitForFunction(() => /^t,x,y\n/.test(document.querySelector(".ds-export-snippet")?.textContent ?? ""));
    const boxes = dialog(page).getByRole("checkbox", { name: /^Trajectory \d/ });
    const n = await boxes.count();
    assert.ok(n >= 2, "one checkbox per trajectory");
    await boxes.first().uncheck();
    await page.waitForTimeout(300);
    await dialog(page).locator(".ds-export-actions .ds-button--primary").click();
    await page.waitForFunction(() => window.__sepxMock.saved.length > 0);
    const names = await mock(page, "m.saved.map((s) => s.name)");
    assert.equal(names.length, 2 * (n - 1), "one pair per chosen trajectory");
    assert.ok(names.every((x) => /\.csv$|\.meta\.json$/.test(x)));
    if (n - 1 > 1) {
      // Numbered as the checkboxes are: Trajectory 1 was left out.
      assert.match(names[0], /-trajectory-2\.csv$/, names[0]);
      assert.ok(names.every((x) => /-trajectory-\d+\.(csv|meta\.json)$/.test(x)), names.join(", "));
    }
    await dialog(page).getByRole("button", { name: "Copy CSV" }).click();
    await page.waitForFunction(() => /^t,x,y\n/.test(window.__sepxMock.clipboardText ?? ""));
  } finally {
    await browser.close();
  }
});

test("Escape closes and returns focus; the options are remembered for the file", async () => {
  const { browser, page } = await open();
  try {
    await openDialog(page);
    await format(page, "CSV");
    await page.keyboard.press("Escape");
    await page.waitForSelector(".export-dialog", { state: "detached" });
    assert.equal(await page.evaluate(() => document.activeElement?.hasAttribute("data-harness-open")), true);
    const saved = await page.evaluate(() => window.__exportHarness.settings.at(-1));
    assert.equal(saved.format, "csv");
    await openDialog(page);
    assert.equal(await dialog(page).getByRole("radio", { name: "CSV" }).getAttribute("aria-checked"), "true");
    // Tab stays inside.
    for (let i = 0; i < 30; i++) await page.keyboard.press("Tab");
    assert.equal(await page.evaluate(() => !!document.activeElement?.closest(".export-dialog")), true);
  } finally {
    await browser.close();
  }
});

test("a system that doesn't compile: Equations and Fixed points are disabled with a reason", async () => {
  const { browser, page } = await open();
  try {
    await page.evaluate(() => window.__exportHarness.setEquation(0, "\\dot{x}=x\\left(3-"));
    await page.waitForTimeout(800);
    await openDialog(page);
    const eq = dialog(page).getByRole("radio", { name: "Equations" });
    assert.equal(await eq.getAttribute("aria-disabled"), "true");
    assert.equal(await dialog(page).getByRole("radio", { name: "Fixed points" }).getAttribute("aria-disabled"), "true");
    await page.getByText("Equations and Fixed points need equations that compile.").waitFor();
    // The figure of the last good compile can still be exported.
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{axis}"));
  } finally {
    await browser.close();
  }
});

test("a map: no direction field or nullclines, with the reason on the checkbox", async () => {
  const { browser, page } = await open("?template=henon-map");
  try {
    await openDialog(page);
    const field = dialog(page).getByRole("checkbox", { name: /Include direction field/ });
    assert.equal(await field.isDisabled(), true);
    assert.match(await field.evaluate((e) => e.closest("label").textContent), /\(none: a map\)/);
    await page.waitForTimeout(500);
    assert.ok((await inked(page)).n > 50, "iterates drawn");
  } finally {
    await browser.close();
  }
});
