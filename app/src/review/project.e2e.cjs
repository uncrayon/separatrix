// Project files in the browser preview: the real studio and document hooks
// (review/project.harness.tsx) over the in-memory files of `window.__sepxMock`.
// CommonJS so Playwright can come from a global install (NODE_PATH).

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");

const BASE = process.env.UI_URL || "http://localhost:5198/";
const HARNESS = new URL("src/review/project.harness.html", BASE).href;
const FIXTURES = path.join(__dirname, "../../../crates/project/tests/fixtures");
const fixtureText = (name) => fs.readFileSync(path.join(FIXTURES, name), "utf8");

/** A page with the harness mounted; `files` are put in the mock file system before it starts. */
async function open({ files = {}, launch = [] } = {}) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
    await page.addInitScript(
      ({ files, launch }) => {
        window.__sepxMock = {
          files: new Map(Object.entries(files)),
          launchFiles: launch,
          saved: [],
          document: { title: "Separatrix", dirty: false },
        };
      },
      { files, launch },
    );
    await page.goto(HARNESS);
    await page.waitForFunction(() => !!window.__projectHarness);
    return { browser, page };
  } catch (err) {
    await browser.close();
    throw err;
  }
}

const h = (page, fn, ...args) => page.evaluate(([fn, args]) => window.__projectHarness[fn](...args), [fn, args]);
const fileText = (page, p) => page.evaluate((p) => window.__sepxMock.files.get(p) ?? null, p);
const setNextSave = (page, p) => page.evaluate((p) => (window.__sepxMock.nextSavePath = p), p);
const last = (page) => page.evaluate(() => window.__projectLast);
const confirmTitle = (page) => page.locator(".confirm .confirm-title").textContent();

test("every template, in LaTeX and in text, saved and reopened gives the same state", async () => {
  const { browser, page } = await open();
  try {
    const ids = await h(page, "templateIds");
    for (const format of ["latex", "text"]) {
      for (const id of ids) {
        await h(page, "newFromTemplate", id);
        if (format === "text") await h(page, "setFormat", "text");
        // A few edits, so the file holds more than the template does.
        await page.evaluate(() => {
          const s = window.__projectHarness.snapshot();
          for (const p of s.params) window.__projectHarness.setParam(p.name, p.value + 0.5);
          window.__projectHarness.editMore();
        });
        const before = await h(page, "snapshot");
        const file = `/mock/${id}-${format}.sepx`;
        await setNextSave(page, file);
        assert.equal(await h(page, "saveAs"), true, id);
        assert.equal((await h(page, "doc")).dirty, false, `${id}: clean after saving`);
        const saved = JSON.parse(await fileText(page, file));
        assert.equal(saved.format, "separatrix");
        assert.equal(saved.system.input_format, format);

        // Something else in between, then the file again.
        await h(page, "newFromTemplate", id === "lorenz" ? "van-der-pol" : "lorenz");
        await h(page, "openPath", file);
        const after = await h(page, "snapshot");
        assert.deepEqual(after, before, `${id} (${format})`);
        const doc = await h(page, "doc");
        assert.equal(doc.dirty, false, `${id}: clean after opening`);
        assert.equal(doc.title, `${id}-${format} — Separatrix`);
        assert.deepEqual(await h(page, "banner"), [], id);
      }
    }
  } finally {
    await browser.close();
  }
});

test("unknown fields at every level survive opening and saving in the UI", async () => {
  const { browser, page } = await open({ files: { "/mock/unknown-fields.sepx": fixtureText("unknown-fields.sepx") } });
  try {
    await h(page, "openPath", "/mock/unknown-fields.sepx");
    const banner = await h(page, "banner");
    assert.ok(banner.some((d) => d.severity === "warning" && d.message.includes("rosenbrock23")), JSON.stringify(banner));
    await setNextSave(page, "/mock/out.sepx");
    assert.equal(await h(page, "saveAs"), true);
    const doc = JSON.parse(fixtureText("unknown-fields.sepx"));
    const out = JSON.parse(await fileText(page, "/mock/out.sepx"));
    assert.deepEqual(out.zz_top_level, doc.zz_top_level);
    assert.ok("assignment" in out && out.assignment === null);
    assert.equal(out.system.notes, doc.system.notes);
    assert.deepEqual(out.views[1], doc.views[1]);
    assert.deepEqual(out.analyses, doc.analyses);
    assert.deepEqual(out.export_settings, doc.export_settings);
    assert.equal(out.initial_conditions[0].label, "near the origin");
    assert.deepEqual(out.initial_conditions[1].style, doc.initial_conditions[1].style);
    assert.equal(out.views[0].title, "Phase plane");
    assert.equal(out.views[0].x_axis.scale, "linear");
    assert.equal(out.views[0].slice.r, 1.5);
    assert.equal(out.views[0].layers.cobweb, false);
    assert.equal(out.views[0].layers.field_style, "arrows");
    assert.deepEqual(out.solver.events, doc.solver.events);
    assert.deepEqual(out.initial_conditions.map((ic) => ic.id), ["ic-1", "ic-2"]);
    // The system has no parameter k, so it isn't written; the method fell back.
    assert.deepEqual(out.system.params, []);
    assert.equal(out.solver.method, "dopri5");
  } finally {
    await browser.close();
  }
});

test("a file whose system doesn't compile keeps its data on save, and gets it back once fixed", async () => {
  const doc = JSON.parse(fixtureText("rabbits-vs-sheep.sepx"));
  const good = doc.system.lines[0];
  const broken = { ...doc, system: { ...doc.system, lines: [good.slice(0, -7), doc.system.lines[1]] } };
  const { browser, page } = await open({ files: { "/mock/broken.sepx": JSON.stringify(broken, null, 2) } });
  try {
    // Start from another document, so a careless save would write its state.
    await h(page, "newFromTemplate", "lorenz");
    await h(page, "openPath", "/mock/broken.sepx");
    let d = await h(page, "doc");
    assert.equal(d.pending, true);
    assert.equal(d.dirty, false);
    const banner = await h(page, "banner");
    assert.ok(banner.some((m) => m.severity === "info" && /don't compile/.test(m.message)), JSON.stringify(banner));

    await setNextSave(page, "/mock/broken-saved.sepx");
    assert.equal(await h(page, "saveAs"), true);
    const out = JSON.parse(await fileText(page, "/mock/broken-saved.sepx"));
    assert.deepEqual(out.system.lines, broken.system.lines);
    assert.deepEqual(out.system.params, doc.system.params);
    assert.deepEqual(out.system.states, doc.system.states);
    assert.deepEqual(out.initial_conditions, doc.initial_conditions);
    assert.deepEqual(out.views, doc.views);
    assert.deepEqual(out.solver, doc.solver);

    // Fixing the line applies the file's start points and view.
    await h(page, "setLine", 0, good);
    await h(page, "waitPendingApplied");
    const s = await h(page, "snapshot");
    assert.deepEqual(
      s.ics.map((ic) => [ic.uid, ic.x0]),
      doc.initial_conditions.map((ic) => [ic.id, ic.x0]),
    );
    assert.deepEqual(s.view.xRange, doc.views[0].x_range);
    assert.equal(s.solver.tEnd, doc.solver.t_end);
    d = await h(page, "doc");
    assert.equal(d.pending, false);
    assert.equal(d.dirty, true, "the line differs from the saved file");
  } finally {
    await browser.close();
  }
});

test("Save as… onto a name that exists once .sepx is added asks in the page before replacing it", async () => {
  const { browser, page } = await open({ files: { "/mock/a.sepx": "{}" } });
  try {
    await h(page, "newFromTemplate", "rabbits-vs-sheep");
    await setNextSave(page, "/mock/a");
    await h(page, "start", "saveAs");
    await page.waitForSelector(".confirm");
    assert.equal(await confirmTitle(page), "“a.sepx” already exists. Replace it?");
    await page.getByRole("button", { name: "Cancel" }).click();
    assert.equal(await last(page), false);
    assert.equal(await fileText(page, "/mock/a.sepx"), "{}");
    assert.equal(await page.locator(".confirm").count(), 0);

    await setNextSave(page, "/mock/a");
    await h(page, "start", "saveAs");
    await page.waitForSelector(".confirm");
    await page.getByRole("button", { name: "Replace" }).click();
    assert.equal(await last(page), true);
    assert.equal(JSON.parse(await fileText(page, "/mock/a.sepx")).system.template, "rabbits-vs-sheep");
    assert.equal((await h(page, "doc")).title, "a — Separatrix");
  } finally {
    await browser.close();
  }
});

test("unsaved changes: the in-page confirm before New, with Cancel, Don't save and Save", async () => {
  const { browser, page } = await open();
  try {
    await h(page, "newFromTemplate", "van-der-pol");
    assert.equal((await h(page, "doc")).dirty, false);
    await h(page, "setParam", "mu", 2.5);
    await page.waitForFunction(() => window.__projectHarness.doc().dirty);
    assert.match((await h(page, "doc")).title, /^● Van der Pol/);

    await h(page, "start", "newFromTemplate", "lorenz");
    await page.waitForSelector(".confirm");
    assert.match(await confirmTitle(page), /^Save changes to “Van der Pol/);
    // Focus starts on Save; Escape is Cancel.
    assert.equal(await page.evaluate(() => document.activeElement?.textContent), "Save");
    await page.keyboard.press("Escape");
    assert.equal(await last(page), false);
    assert.equal((await h(page, "snapshot")).template, "van-der-pol");

    await h(page, "start", "newFromTemplate", "lorenz");
    await page.waitForSelector(".confirm");
    await page.getByRole("button", { name: "Don't save" }).click();
    assert.equal(await last(page), true);
    await page.waitForFunction(() => window.__projectHarness.snapshot().template === "lorenz");
    await page.waitForFunction(() => !window.__projectHarness.doc().dirty);

    // Save: an untitled document goes through Save as… first, then New goes ahead.
    await h(page, "setParam", "r", 30);
    await page.waitForFunction(() => window.__projectHarness.doc().dirty);
    await setNextSave(page, "/mock/lorenz-r30.sepx");
    await h(page, "start", "newBlank");
    await page.waitForSelector(".confirm");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    assert.equal(await last(page), true);
    const saved = JSON.parse(await fileText(page, "/mock/lorenz-r30.sepx"));
    assert.equal(saved.system.params.find((p) => p.id === "r").value, 30);
    await page.waitForFunction(() => window.__projectHarness.snapshot().template === null);
    assert.equal((await h(page, "doc")).path, null);
  } finally {
    await browser.close();
  }
});

test("an edit undone by hand leaves the document clean", async () => {
  const { browser, page } = await open();
  try {
    await h(page, "newFromTemplate", "saddle-node");
    const r = (await h(page, "snapshot")).params.find((p) => p.name === "r").value;
    await h(page, "setParam", "r", r + 1);
    await page.waitForFunction(() => window.__projectHarness.doc().dirty);
    await h(page, "setParam", "r", r);
    await page.waitForFunction(() => !window.__projectHarness.doc().dirty);
  } finally {
    await browser.close();
  }
});

test("the plot moving the view by itself leaves a clean document clean, and a dirty one dirty", async () => {
  const { browser, page } = await open();
  try {
    await h(page, "newFromTemplate", "rabbits-vs-sheep");
    const y1 = (await h(page, "snapshot")).view.yRange[1];
    await h(page, "autoAdjust");
    await page.waitForFunction((y1) => window.__projectHarness.snapshot().view.yRange[1] > y1, y1);
    assert.equal((await h(page, "doc")).dirty, false);
    await h(page, "editMore");
    await page.waitForFunction(() => window.__projectHarness.doc().dirty);
    await h(page, "autoAdjust");
    await page.waitForTimeout(100);
    assert.equal((await h(page, "doc")).dirty, true);
  } finally {
    await browser.close();
  }
});

test("closing the window while dirty asks first; Cancel keeps it open", async () => {
  const { browser, page } = await open();
  try {
    await h(page, "newFromTemplate", "van-der-pol");
    await h(page, "setParam", "mu", 3);
    await page.waitForFunction(() => window.__projectHarness.doc().dirty);
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("sepx:close-requested")));
    await page.waitForSelector(".confirm");
    assert.match(await confirmTitle(page), /^Save changes to/);
    await page.getByRole("button", { name: "Cancel" }).click();
    assert.equal(await page.locator(".confirm").count(), 0);
    assert.equal((await h(page, "doc")).dirty, true);
  } finally {
    await browser.close();
  }
});

test("Ctrl+S saves to the open file; Ctrl+Shift+S asks where", async () => {
  const { browser, page } = await open({ files: { "/mock/rabbits.sepx": fixtureText("rabbits-vs-sheep.sepx") } });
  try {
    await h(page, "openPath", "/mock/rabbits.sepx");
    const before = await fileText(page, "/mock/rabbits.sepx");
    await page.locator("body").click();
    // Make it dirty by moving to text mode, then save with the keyboard.
    await h(page, "setFormat", "text");
    await page.waitForFunction(() => window.__projectHarness.doc().dirty);
    await page.keyboard.press("Control+s");
    await page.waitForFunction(() => !window.__projectHarness.doc().dirty);
    const after = JSON.parse(await fileText(page, "/mock/rabbits.sepx"));
    assert.notEqual(JSON.stringify(after), before);
    assert.equal(after.system.input_format, "text");

    await setNextSave(page, "/mock/copy.sepx");
    await page.keyboard.press("Control+Shift+S");
    await page.waitForFunction(() => window.__projectHarness.doc().path === "/mock/copy.sepx");
    assert.ok(await fileText(page, "/mock/copy.sepx"));
  } finally {
    await browser.close();
  }
});

test("a file passed at launch opens, and an open request from Rust opens another", async () => {
  const files = {
    "/mock/rabbits-vs-sheep.sepx": fixtureText("rabbits-vs-sheep.sepx"),
    "/mock/lorenz-text-mode.sepx": fixtureText("lorenz-text-mode.sepx"),
  };
  const { browser, page } = await open({ files, launch: ["/mock/rabbits-vs-sheep.sepx"] });
  try {
    await page.waitForFunction(() => window.__projectHarness.doc().title === "rabbits-vs-sheep — Separatrix");
    await page.waitForFunction(() => window.__sepxMock.document.title === "rabbits-vs-sheep — Separatrix");
    const icCount = JSON.parse(files["/mock/rabbits-vs-sheep.sepx"]).initial_conditions.length;
    await page.waitForFunction((n) => window.__projectHarness.snapshot().ics.length === n, icCount);
    await page.evaluate(() =>
      window.dispatchEvent(new CustomEvent("sepx:project-open-requested", { detail: { path: "/mock/lorenz-text-mode.sepx" } })),
    );
    await page.waitForFunction(() => window.__projectHarness.doc().title === "lorenz-text-mode — Separatrix");
    const s = await h(page, "snapshot");
    assert.equal(s.format, "text");
    assert.deepEqual(s.lines, JSON.parse(files["/mock/lorenz-text-mode.sepx"]).system.lines);
    await page.waitForFunction(() => window.__projectHarness.snapshot().kind === "ode");
  } finally {
    await browser.close();
  }
});

test("a file that can't be read shows the reason in the banner and keeps the document", async () => {
  const { browser, page } = await open({ files: { "/mock/not-json.sepx": fixtureText("not-json.sepx") } });
  try {
    await h(page, "newFromTemplate", "pendulum");
    assert.equal(await h(page, "openPath", "/mock/not-json.sepx"), false);
    await page.waitForSelector(".project-banner .ds-diagnostic--error");
    assert.match(await page.locator(".project-banner").textContent(), /“not-json\.sepx”/);
    assert.equal((await h(page, "snapshot")).template, "pendulum");
    await page.getByRole("button", { name: "Dismiss" }).click();
    assert.equal(await page.locator(".project-banner").count(), 0);
  } finally {
    await browser.close();
  }
});

test("reloading a dirty document under automation isn't held up by the tab's prompt", async () => {
  const { browser, page } = await open();
  try {
    await h(page, "newFromTemplate", "van-der-pol");
    await h(page, "setParam", "mu", 3);
    await page.waitForFunction(() => window.__projectHarness.doc().dirty);
    await page.reload({ timeout: 5000 });
    await page.waitForFunction(() => !!window.__projectHarness);
  } finally {
    await browser.close();
  }
});
