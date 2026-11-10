// Exports and project files in the real app shell (App.tsx, the top bar),
// in the browser preview: the mocked core, and the in-memory files,
// dialogs and clipboard of `window.__sepxMock` (api/mock.ts).
// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const URL = process.env.UI_URL || "http://localhost:5198/";

async function open() {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
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
  await page.waitForTimeout(500);
}

const topbar = (page) => page.locator(".topbar");
const button = (page, name) => topbar(page).getByRole("button", { name, exact: true });
const dialog = (page) => page.locator(".export-dialog");
const mock = (page, expr) => page.evaluate(`(() => { const m = window.__sepxMock; return ${expr}; })()`);
const fileText = (page, p) => page.evaluate((p) => window.__sepxMock.files.get(p) ?? null, p);
const docName = (page) => page.locator(".topbar .doc-name").textContent();
const dirtyDot = (page) => page.locator(".topbar .doc-dirty").count();
const textInputs = (page) =>
  page.$$eval(".panel-left input.ds-equation-editor-field", (els) => els.map((e) => e.value));

async function openExport(page) {
  await button(page, "Export…").click();
  await page.waitForSelector(".export-dialog[role=dialog]");
}

async function setFormat(page, name) {
  await dialog(page).getByRole("radio", { name, exact: true }).click();
}

async function setMode(page, name) {
  await page.locator(".panel-left").getByRole("radio", { name, exact: true }).click();
  await page.waitForFunction(
    (name) =>
      document.querySelector(".panel-left [role=radio][aria-checked=true]")?.textContent === name &&
      document.querySelectorAll(name === "Text" ? ".panel-left input.ds-equation-editor-field" : ".panel-left math-field").length > 0,
    name,
  );
  await page.waitForTimeout(300);
}

async function setParam(page, name, value) {
  const field = page.getByRole("textbox", { name: `Value of ${name}` });
  await field.click();
  await field.fill(value);
  await field.press("Enter");
  await page.locator("canvas.plot-canvas").focus();
  await page.waitForTimeout(300);
}

const nullclines = (page) => page.getByRole("button", { name: "Nullclines", exact: true });

const confirmBox = (page) => page.locator(".confirm[role=alertdialog]");
async function answer(page, label) {
  await confirmBox(page).getByRole("button", { name: label, exact: true }).click();
  await page.waitForSelector(".confirm", { state: "detached" });
  await page.waitForTimeout(300);
}

test("Export… opens the dialog on LaTeX; Copy LaTeX copies the packages comment and a tikzpicture", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await openExport(page);
    assert.equal(await page.locator(".app").evaluate((e) => e.inert), true, "the app is inert behind the dialog");
    assert.equal(await dialog(page).getByRole("radio", { name: "LaTeX (TikZ/pgfplots)" }).getAttribute("aria-checked"), "true");
    assert.equal(await dialog(page).getByRole("radio", { name: "Figure" }).getAttribute("aria-checked"), "true");
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{tikzpicture}"));
    const primary = dialog(page).locator(".ds-export-actions .ds-button--primary");
    assert.equal((await primary.innerText()).trim(), "Copy LaTeX");
    await primary.click();
    await page.waitForFunction(() => typeof window.__sepxMock.clipboardText === "string");
    const tex = await mock(page, "m.clipboardText");
    assert.match(tex, /^%.*\\usepackage\{pgfplots\}.*\\pgfplotsset\{compat=1\.18\}/m, "a comment names the packages");
    assert.ok(tex.indexOf("\\usepackage{pgfplots}") < tex.indexOf("\\begin{tikzpicture}"), "the comment comes first");
    assert.match(tex, /\\begin\{tikzpicture\}[\s\S]*\\begin\{axis\}[\s\S]*\\end\{tikzpicture\}/);

    // Escape closes, and focus goes back to Export….
    await page.keyboard.press("Escape");
    await page.waitForSelector(".export-dialog", { state: "detached" });
    assert.equal(await page.locator(".app").evaluate((e) => e.inert), false);
    assert.equal(await page.evaluate(() => document.activeElement?.textContent?.trim()), "Export…");

    // Ctrl+E opens it too, from the plot.
    await page.locator("canvas.plot-canvas").focus();
    await page.keyboard.press("ControlOrMeta+e");
    await page.waitForSelector(".export-dialog[role=dialog]");
    await page.keyboard.press("Escape");
    await page.waitForSelector(".export-dialog", { state: "detached" });
  } finally {
    await browser.close();
  }
});

test("SVG and PNG tabs write their files; the CSV tab previews rows under a header row", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await openExport(page);
    const primary = dialog(page).locator(".ds-export-actions .ds-button--primary");
    const ready = () => page.waitForFunction(() => !document.querySelector(".ds-export-actions .ds-button--primary")?.disabled);

    await setFormat(page, "SVG");
    assert.equal((await primary.innerText()).trim(), "Save…");
    await ready();
    await primary.click();
    await page.waitForFunction(() => window.__sepxMock.saved.length === 1);
    const svg = await page.evaluate(() => {
      const f = window.__sepxMock.saved[0];
      const doc = new DOMParser().parseFromString(f.contents, "image/svg+xml");
      return {
        name: f.name,
        error: doc.querySelector("parsererror")?.textContent ?? null,
        root: doc.documentElement.nodeName,
        paths: doc.querySelectorAll("path").length,
        texts: doc.querySelectorAll("text").length,
      };
    });
    assert.equal(svg.name, "rabbits-vs-sheep.svg");
    assert.equal(svg.error, null);
    assert.equal(svg.root, "svg");
    assert.ok(svg.paths > 10, `drawn as paths (${svg.paths})`);
    assert.ok(svg.texts > 4, "labels are text");

    await setFormat(page, "PNG");
    await ready();
    await primary.click();
    await page.waitForFunction(() => window.__sepxMock.saved.length === 2, null, { timeout: 15000 });
    const png = await page.evaluate(() => {
      const f = window.__sepxMock.saved[1];
      const b = f.contents;
      const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
      return { name: f.name, sig: Array.from(b.slice(0, 8)), w: dv.getUint32(16), h: dv.getUint32(20), bytes: b.length };
    });
    assert.equal(png.name, "rabbits-vs-sheep.png");
    assert.deepEqual(png.sig, [137, 80, 78, 71, 13, 10, 26, 10]);
    assert.ok(png.w > 0 && png.h > 0 && png.bytes > 1000, JSON.stringify(png));

    await setFormat(page, "CSV");
    await page.waitForFunction(() => (document.querySelector(".ds-export-snippet")?.textContent ?? "").length > 0);
    const rows = (await page.locator(".ds-export-snippet").innerText()).split("\n").filter((l) => l.trim() !== "");
    assert.equal(rows[0], "t,x,y", "one header row");
    assert.ok(rows.length >= 2, "data rows follow");
    assert.ok(rows.slice(1).every((r) => /^[-0-9.e+]+(,[-0-9.e+]+){2}$/.test(r)), rows.slice(1, 4).join(" | "));
  } finally {
    await browser.close();
  }
});

test("Save, then Open…, gives back a modified system; the title follows the file", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    // The preview's stand-in core compiles the templates only, so the edits
    // are the input mode, a parameter and a layer, not the equations.
    await setMode(page, "Text");
    await setParam(page, "mu", "2.5");
    await nullclines(page).click();
    assert.equal(await nullclines(page).getAttribute("aria-pressed"), "false");
    assert.equal(await dirtyDot(page), 1, "edits mark the document");
    assert.match(await page.title(), /^● /);

    // Ctrl+S on a document without a file goes through Save as…
    await page.locator("canvas.plot-canvas").focus();
    await page.keyboard.press("ControlOrMeta+s");
    await page.waitForFunction(() => window.__sepxMock.files.has("/mock/van-der-pol.sepx"));
    await page.waitForFunction(() => !document.querySelector(".topbar .doc-dirty"));
    const saved = JSON.parse(await fileText(page, "/mock/van-der-pol.sepx"));
    assert.equal(saved.format, "separatrix");
    assert.equal(saved.system.input_format, "text");
    assert.deepEqual(saved.system.lines, ["x' = y", "y' = -x - mu*(x^2 - 1)*y"]);
    assert.equal(saved.views[0].layers.nullclines, false);
    assert.equal(saved.system.params.find((p) => p.id === "mu").value, 2.5);
    assert.equal(await docName(page), "van-der-pol");
    assert.equal(await page.title(), "van-der-pol — Separatrix");

    // Another system in between; the document is clean, so nothing asks.
    await button(page, "Templates…").click();
    await page.waitForSelector(".gallery");
    await pick(page, "Lorenz");
    assert.equal(await confirmBox(page).count(), 0);
    assert.notEqual(await docName(page), "van-der-pol");

    // Open… (the mock dialog returns the only file).
    await button(page, "Open…").click();
    await page.waitForFunction(() => document.title === "van-der-pol — Separatrix");
    await page.waitForFunction(() => document.querySelector(".statusbar")?.textContent.includes("Compiled"));
    await page.waitForTimeout(400);
    assert.deepEqual(await textInputs(page), ["x' = y", "y' = -x - mu*(x^2 - 1)*y"]);
    assert.equal(await nullclines(page).getAttribute("aria-pressed"), "false");
    assert.equal(await page.getByRole("slider", { name: "Parameter mu" }).inputValue(), "2.5");
    assert.equal(await page.locator(".panel-left [role=radio][aria-checked=true]").textContent(), "Text");
    assert.equal(await dirtyDot(page), 0, "clean after opening");

    // Saving again writes to the same file, with no dialog.
    await setParam(page, "mu", "3");
    assert.equal(await dirtyDot(page), 1);
    await page.keyboard.press("ControlOrMeta+s");
    await page.waitForFunction(() => !document.querySelector(".topbar .doc-dirty"));
    const again = JSON.parse(await fileText(page, "/mock/van-der-pol.sepx"));
    assert.equal(again.system.params.find((p) => p.id === "mu").value, 3);
    assert.equal(await mock(page, "m.files.size"), 1);
  } finally {
    await browser.close();
  }
});

test("the dirty marker, and the in-page confirm before New and before a gallery pick", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    assert.equal(await dirtyDot(page), 0, "a fresh template is clean");
    await setParam(page, "mu", "2.5");
    assert.equal(await dirtyDot(page), 1);
    assert.match(await page.locator(".topbar .doc-title").textContent(), /Unsaved changes/, "said to screen readers too");

    // New: Cancel keeps everything.
    await button(page, "New system").click();
    await confirmBox(page).waitFor();
    assert.equal(await confirmBox(page).locator(".confirm-title").textContent(), "Save changes to “Van der Pol oscillator”?");
    assert.equal(await page.evaluate(() => document.activeElement?.textContent), "Save", "Save has focus");
    assert.equal(await page.locator(".app").evaluate((e) => e.inert), true);
    await answer(page, "Cancel");
    assert.equal(await page.getByRole("slider", { name: "Parameter mu" }).inputValue(), "2.5");
    assert.equal(await dirtyDot(page), 1, "still dirty");

    // New: Don't save starts a blank system.
    await button(page, "New system").click();
    await confirmBox(page).waitFor();
    await answer(page, "Don't save");
    await page.waitForFunction(() => document.querySelector(".statusbar")?.textContent.includes("Compiled"));
    assert.equal(await page.getByRole("slider", { name: "Parameter mu" }).count(), 0);
    assert.equal(await docName(page), "Untitled system");
    assert.equal(await dirtyDot(page), 0);
    assert.equal(await mock(page, "m.files.size"), 0, "nothing was saved");

    // A gallery pick asks over the gallery; Cancel keeps it open.
    await button(page, "Templates…").click();
    await page.waitForSelector(".gallery");
    await pick(page, "Van der Pol");
    await setParam(page, "mu", "2.5");
    await button(page, "Templates…").click();
    await page.waitForSelector(".gallery");
    await page.locator(".gallery .card", { hasText: "Lorenz" }).first().click();
    await confirmBox(page).waitFor();
    assert.equal(await page.locator(".gallery").count(), 1, "the gallery stays behind the confirm");
    await answer(page, "Cancel");
    assert.equal(await page.locator(".gallery").count(), 1, "Cancel keeps the gallery open");
  } finally {
    await browser.close();
  }
});

test("a gallery pick over unsaved changes: Save writes the file, then the template opens", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    await setParam(page, "mu", "4");
    await button(page, "Templates…").click();
    await page.waitForSelector(".gallery");
    await page.locator(".gallery .card", { hasText: "Lorenz" }).first().click();
    await confirmBox(page).waitFor();
    await answer(page, "Save");
    await page.waitForSelector(".gallery", { state: "detached" });
    await page.waitForFunction(() => window.__sepxMock.files.has("/mock/van-der-pol.sepx"));
    const saved = JSON.parse(await fileText(page, "/mock/van-der-pol.sepx"));
    assert.equal(saved.system.params.find((p) => p.id === "mu").value, 4);
    await page.waitForFunction(() => document.querySelector(".topbar .doc-name")?.textContent.startsWith("Lorenz"));
    assert.equal(await dirtyDot(page), 0);
  } finally {
    await browser.close();
  }
});

test("an opened-file event (a second instance, a drop) closes the export dialog and asks first when dirty", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    await setParam(page, "mu", "2");
    await page.evaluate(() => {
      // A file the "second instance" passes on.
      window.__sepxMock.nextSavePath = "/mock/other.sepx";
    });
    await button(page, "Save as…").click();
    await page.waitForFunction(() => window.__sepxMock.files.has("/mock/other.sepx"));
    await page.waitForFunction(() => document.title === "other — Separatrix");
    await setParam(page, "mu", "3.5");
    assert.equal(await dirtyDot(page), 1);

    await openExport(page);
    await page.evaluate(() =>
      window.dispatchEvent(new CustomEvent("sepx:project-open-requested", { detail: { path: "/mock/other.sepx" } })),
    );
    await page.waitForSelector(".export-dialog", { state: "detached" });
    await confirmBox(page).waitFor();
    await answer(page, "Don't save");
    await page.waitForFunction(() => document.title === "other — Separatrix");
    await page.waitForTimeout(300);
    assert.equal(await page.getByRole("slider", { name: "Parameter mu" }).inputValue(), "2");
    assert.equal(await dirtyDot(page), 0);

    // An unreadable file: the banner says so, and the document stays.
    await page.evaluate(() => {
      window.__sepxMock.files.set("/mock/broken.sepx", "{ not json");
      window.dispatchEvent(new CustomEvent("sepx:project-open-requested", { detail: { path: "/mock/broken.sepx" } }));
    });
    await page.locator(".project-banner").waitFor();
    assert.match(await page.locator(".project-banner").innerText(), /isn't a Separatrix project/);
    assert.equal(await page.title(), "other — Separatrix");
    await page.locator(".project-banner").getByRole("button", { name: "Dismiss" }).click();
    await page.waitForSelector(".project-banner", { state: "detached" });
  } finally {
    await browser.close();
  }
});

test("a banner never covers the canvas toolbar", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    await page.evaluate(() => {
      window.__sepxMock.files.set("/mock/broken.sepx", "{ not json");
      window.dispatchEvent(new CustomEvent("sepx:project-open-requested", { detail: { path: "/mock/broken.sepx" } }));
    });
    await page.locator(".project-banner").waitFor();
    const covered = await page.$$eval(".canvas-toolbar button", (buttons) =>
      buttons
        .map((b) => {
          const r = b.getBoundingClientRect();
          const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
          return hit && (hit === b || b.contains(hit)) ? null : b.getAttribute("aria-label") || b.textContent;
        })
        .filter(Boolean),
    );
    assert.deepEqual(covered, [], "every toolbar button can be clicked while the banner shows");
    // Said once, assertively, by the always-mounted region outside the app.
    assert.match(await page.locator("body > #root > [role=alert], [role=alert].ds-sr").first().textContent(), /broken\.sepx/);
  } finally {
    await browser.close();
  }
});

test("a file passed at launch that can't be read keeps the start gallery, with the reason", async () => {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await page.addInitScript(() => {
      window.__sepxMock = {
        files: new Map([["/mock/x.sepx", "{"]]),
        launchFiles: ["/mock/x.sepx"],
        saved: [],
        document: { title: "Separatrix", dirty: false },
      };
    });
    await page.goto(URL);
    await page.waitForSelector(".gallery .project-banner .ds-diagnostic--error");
    assert.match(await page.locator(".gallery .project-banner").innerText(), /x\.sepx/);
    assert.equal(await page.locator(".gallery").count(), 1, "the gallery stays open");
  } finally {
    await browser.close();
  }
});

test("focus goes back to the button that asked once the confirm closes", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Van der Pol");
    await setParam(page, "mu", "2.5");
    await button(page, "New system").focus();
    await page.keyboard.press("Enter");
    await confirmBox(page).waitFor();
    await page.keyboard.press("Escape");
    await page.waitForSelector(".confirm", { state: "detached" });
    assert.equal(await page.evaluate(() => document.activeElement?.getAttribute("aria-label")), "New system");
    // Cancel with the mouse too.
    await button(page, "New system").click();
    await confirmBox(page).waitFor();
    await answer(page, "Cancel");
    assert.equal(await page.evaluate(() => document.activeElement?.getAttribute("aria-label")), "New system");
    // Don't save, then a new document: focus on the plot, never on <body>.
    await button(page, "New system").click();
    await confirmBox(page).waitFor();
    await answer(page, "Don't save");
    await page.waitForTimeout(200);
    assert.notEqual(await page.evaluate(() => document.activeElement?.tagName), "BODY");
  } finally {
    await browser.close();
  }
});

test("export: the snippet's Copy says Copied itself; fixed points preview the table; files take the project's name", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await page.evaluate(() => (window.__sepxMock.nextSavePath = "/mock/hw.sepx"));
    await button(page, "Save as…").click();
    await page.waitForFunction(() => document.title === "hw — Separatrix");
    await openExport(page);
    await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{tikzpicture}"));
    assert.match(await dialog(page).locator(".ds-export-foot-note").innerText(), /hw\.tex/);
    const head = dialog(page).locator(".ds-export-code-head button");
    const foot = dialog(page).locator(".ds-export-actions .ds-button--primary");
    const footBox = await foot.boundingBox();
    await head.click();
    await page.waitForFunction(() => document.querySelector(".ds-export-code-head button")?.textContent.includes("Copied"));
    assert.equal((await foot.textContent()).trim(), "Copy LaTeX", "the footer button keeps its label");
    assert.match(await head.getAttribute("aria-describedby"), /.+/, "the head Copy says what it copies");
    await foot.click();
    await page.waitForFunction(() => document.querySelector(".ds-export-actions .ds-button--primary")?.textContent.includes("Copied"));
    const after = await foot.boundingBox();
    assert.equal(Math.round(after.width), Math.round(footBox.width), "Copied doesn't resize the button");
    assert.equal(Math.round(after.x), Math.round(footBox.x));

    await dialog(page).getByRole("radio", { name: "Fixed points" }).click();
    await page.waitForSelector(".export-fp-table");
    assert.equal(await dialog(page).locator(".ds-export-preview-well").count(), 0, "no figure preview for a table");
    assert.match(await dialog(page).locator(".export-fp-table").innerText(), /Saddle/);
    assert.match(await dialog(page).locator(".ds-export-sub").innerText(), /fixed points/);
    assert.equal(await page.locator(".export-dialog h2").innerText(), "Export");
  } finally {
    await browser.close();
  }
});

test("at 1024 × 768 the LaTeX snippet shows a few lines without scrolling, in both themes", async () => {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1024, height: 768 } });
    await page.goto(URL);
    await page.waitForSelector(".gallery");
    await pick(page, "Rabbits vs sheep");
    for (const projector of [false, true]) {
      if (projector) await page.getByRole("button", { name: "Projector mode" }).click();
      await openExport(page);
      await page.waitForFunction(() => document.querySelector(".ds-export-snippet")?.textContent.includes("\\begin{tikzpicture}"));
      const shown = await page.evaluate(() => {
        const body = document.querySelector(".ds-export-body");
        const pre = document.querySelector(".ds-export-snippet");
        const line = parseFloat(getComputedStyle(pre).lineHeight) || 18;
        const visible = body.getBoundingClientRect().bottom - pre.getBoundingClientRect().top;
        return visible / line;
      });
      assert.ok(shown >= 4, `${projector ? "projector" : "dark"}: ${shown.toFixed(1)} lines of the snippet show`);
      await page.keyboard.press("Escape");
      await page.waitForSelector(".export-dialog", { state: "detached" });
    }
  } finally {
    await browser.close();
  }
});

test("Export from an equation field and Escape: focus lands somewhere, never on <body>", async () => {
  const { browser, page } = await open();
  try {
    await pick(page, "Rabbits vs sheep");
    await page.locator(".panel-left math-field").first().click();
    await page.keyboard.press("ControlOrMeta+e");
    await page.waitForSelector(".export-dialog[role=dialog]");
    await page.keyboard.press("Escape");
    await page.waitForSelector(".export-dialog", { state: "detached" });
    await page.waitForTimeout(100);
    assert.notEqual(await page.evaluate(() => document.activeElement?.tagName), "BODY");
  } finally {
    await browser.close();
  }
});
