// Project files in the UI: the studio state <-> a .sepx document. Node strips
// the types from the .ts modules; JSON comes in with an import attribute.

import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_LAYERS,
  DEFAULT_SOLVER,
  applyProjectLoad,
  defaultView,
  leastUsedPaletteIndex,
  resolveParam,
  templateView,
} from "../state/model.ts";
import {
  KNOWN_METHODS,
  dirtyKey,
  icUid,
  primaryViewIndex,
  projectFromState,
  stateFromProject,
} from "../project/serialize.ts";
import { documentName, projectStem, slug, suggestedProjectName, windowTitle } from "../project/names.ts";
import { isMacPlatform, matchShortcut, shortcutLabel, shortcutLetter } from "../project/shortcuts.ts";
import { mergeExportSettings, readExportOptions } from "../export/options.ts";
import templates from "../templates/templates.json" with { type: "json" };
import textLines from "../templates/templates.text.generated.json" with { type: "json" };

const FIXTURES = new URL("../../../crates/project/tests/fixtures/", import.meta.url);
const fixture = (name) => JSON.parse(readFileSync(fileURLToPath(new URL(name, FIXTURES)), "utf8"));
const existsFixture = (name) => existsSync(fileURLToPath(new URL(name, FIXTURES)));

const st = (name, latex = name) => ({ name, latex, derived: false });

/**
 * A stand-in for the compile of a template: its kind, its parameters with
 * their defaults, and states named after its view's axes (padded to the
 * dimension of its start points). Only consistency matters here.
 */
function fakeCompile(t) {
  const map = t.equations.some((e) => e.includes("_{n+1}"));
  const named = [t.view.x, t.view.y].filter((n) => n !== "t" && n !== "n");
  // A cobweb names its one state on both axes.
  const states = [...new Set(named)];
  const dim = Math.max(states.length, ...t.initialConditions.map((x0) => x0.length), 1);
  while (states.length < dim) states.push(`q${states.length}`);
  return {
    ok: true,
    systemId: 1,
    kind: map ? "map" : "ode",
    hamiltonian: false,
    autonomous: true,
    states: states.map((n) => st(n)),
    params: Object.keys(t.params).map((name) => ({ name, latex: name, default: 1 })),
    equationsLatex: [],
    diagnostics: [],
  };
}

/** The state useStudio's template load builds (runCompile with a template), plus a few edits. */
function templateState(t, format, sys) {
  let nextId = 1;
  const dim = sys.states.length;
  const ics = [];
  for (const x0 of t.initialConditions) {
    const id = nextId++;
    ics.push({ id, uid: `ic-${id}`, x0: Array.from({ length: dim }, (_, i) => x0[i] ?? 0), t0: 0, color: leastUsedPaletteIndex(ics) });
  }
  if (ics.length) ics[ics.length - 1] = { ...ics[ics.length - 1], hidden: true, t0: 0.5 };
  let line = 1;
  return {
    format,
    equations: (format === "text" ? textLines[t.id] : t.equations).map((source) => ({ id: line++, source })),
    compiled: sys,
    params: sys.params.map((p) => resolveParam(p, undefined, undefined, t, undefined)),
    view: templateView(t, sys),
    baseState: sys.states.map((_, i) => 0.25 * i),
    fieldTime: 0.75,
    ics,
    layers: { ...DEFAULT_LAYERS, backward: t.backward ?? true, nullclines: dim <= 2 },
    solver: { ...DEFAULT_SOLVER, tEnd: t.tEnd },
    sourceTemplateId: t.id,
  };
}

/** Saves `s` to text, reads it back, and applies it for `sys`, as Open does. */
function reopen(s, sys, opts) {
  const text = JSON.stringify(projectFromState(s, opts), null, 2) + "\n";
  const doc = JSON.parse(text);
  const load = stateFromProject(doc);
  let id = 100;
  const applied = applyProjectLoad(load, sys, () => id++);
  return { doc, load, applied, text };
}

/** What the reopened state must give back, start points without their (session) numeric ids. */
function comparable(s) {
  return {
    format: s.format,
    lines: s.equations.map((l) => l.source),
    template: s.sourceTemplateId,
    params: s.params,
    view: s.view,
    baseState: s.baseState,
    fieldTime: s.fieldTime,
    ics: s.ics.map((ic) => ({ uid: icUid(ic), x0: ic.x0, t0: ic.t0, color: ic.color, hidden: !!ic.hidden })),
    layers: s.layers,
    solver: s.solver,
  };
}

function reopenedComparable(load, applied) {
  return {
    format: load.format,
    lines: load.lines,
    template: load.templateId,
    params: applied.params,
    view: applied.view,
    baseState: applied.baseState,
    fieldTime: applied.fieldTime,
    ics: applied.ics.map((ic) => ({ uid: ic.uid, x0: ic.x0, t0: ic.t0, color: ic.color, hidden: !!ic.hidden })),
    layers: applied.layers,
    solver: applied.solver,
  };
}

test("every template, in LaTeX and in text, saved and reopened gives the same state", () => {
  for (const t of templates) {
    for (const format of ["latex", "text"]) {
      const sys = fakeCompile(t);
      const s = templateState(t, format, sys);
      const { load, applied, doc } = reopen(s, sys);
      assert.deepEqual(reopenedComparable(load, applied), comparable(s), `${t.id} (${format})`);
      assert.deepEqual(applied.warnings, [], t.id);
      assert.deepEqual(load.warnings, [], t.id);
      assert.equal(doc.format, "separatrix");
      assert.equal(doc.views.length, 1);
      assert.equal(doc.views[0].id, "main");
      assert.deepEqual(doc.system.states, sys.states.map((x) => x.name));
      // Saving the reopened state again changes nothing.
      const again = { ...s, ics: applied.ics, params: applied.params };
      assert.equal(dirtyKey(again, { loaded: doc }), dirtyKey(s, { loaded: doc }), t.id);
    }
  }
});

test("blank lines are kept exactly as typed", () => {
  const t = templates.find((x) => x.id === "lorenz");
  const sys = fakeCompile(t);
  const s = templateState(t, "text", sys);
  s.equations = [...s.equations.slice(0, 2), { id: 99, source: "" }, ...s.equations.slice(2)];
  const { load } = reopen(s, sys);
  assert.deepEqual(load.lines, s.equations.map((l) => l.source));
  assert.equal(load.lines[2], "");
});

/** The state the fixture opens to, for a system with its states and `params`. */
function openFixture(doc, params = []) {
  const sys = {
    ok: true,
    systemId: 1,
    kind: "ode",
    hamiltonian: false,
    autonomous: true,
    states: doc.system.states.map((n) => st(n)),
    params: params.map((name) => ({ name, latex: name, default: 1 })),
    equationsLatex: [],
    diagnostics: [],
  };
  const load = stateFromProject(doc);
  let id = 1;
  const a = applyProjectLoad(load, sys, () => id++);
  let line = 1;
  const state = {
    format: load.format,
    equations: load.lines.map((source) => ({ id: line++, source })),
    compiled: sys,
    params: a.params,
    view: a.view,
    baseState: a.baseState,
    fieldTime: a.fieldTime,
    ics: a.ics,
    layers: a.layers,
    solver: a.solver,
    sourceTemplateId: load.templateId,
  };
  return { sys, load, applied: a, state };
}

test("unknown fields at every level survive the UI round trip", () => {
  const doc = fixture("unknown-fields.sepx");
  const { state, load } = openFixture(doc, ["k"]);
  assert.match(load.warnings.join(" "), /rosenbrock23/);
  const out = projectFromState(state, { loaded: doc });
  // What the UI changes: the unknown method falls back to dopri5 (with the
  // warning above), and the slice lists every state's value.
  const expected = structuredClone(doc);
  expected.solver.method = "dopri5";
  expected.views[0].slice.states = { x: 0, y: 0 };
  assert.deepEqual(JSON.parse(JSON.stringify(out)), expected);
  // Explicitly: the reserved v2 key, null included, and the second view.
  assert.ok("assignment" in out && out.assignment === null);
  assert.deepEqual(out.views[1], doc.views[1]);
  assert.deepEqual(out.analyses, doc.analyses);
  assert.deepEqual(out.export_settings, doc.export_settings);
  assert.equal(out.initial_conditions[0].label, "near the origin");
  assert.deepEqual(out.system.params[0], doc.system.params[0]);
  // A second save is identical.
  const again = projectFromState(openFixture(out, ["k"]).state, { loaded: out });
  assert.equal(JSON.stringify(again), JSON.stringify(out));
});

test("an assignment object rides along unchanged", () => {
  const doc = { ...fixture("rabbits-vs-sheep.sepx"), assignment: { id: "hw3", tasks: [{ ref: "params.a" }] } };
  const { state } = openFixture(doc);
  assert.deepEqual(projectFromState(state, { loaded: doc }).assignment, doc.assignment);
});

test("while the opened file's system doesn't compile, a save keeps its parameters, start points, views and solver", () => {
  const doc = fixture("unknown-fields.sepx");
  const load = stateFromProject(doc);
  // The studio still holds the previous document's state; only the lines are the file's (and then edited).
  const other = templates.find((t) => t.id === "van-der-pol");
  const stale = templateState(other, "latex", fakeCompile(other));
  const s = {
    ...stale,
    format: load.format,
    equations: [{ id: 1, source: "\\dot{x}=x\\left(3-x-" }, { id: 2, source: load.lines[1] }],
    sourceTemplateId: load.templateId,
  };
  const out = projectFromState(s, { loaded: doc, pending: load });
  assert.deepEqual(out.system.lines, ["\\dot{x}=x\\left(3-x-", load.lines[1]]);
  assert.deepEqual(out.system.params, doc.system.params);
  assert.deepEqual(out.system.states, doc.system.states);
  assert.equal(out.system.kind, doc.system.kind);
  assert.deepEqual(out.initial_conditions, doc.initial_conditions);
  assert.deepEqual(out.views, doc.views);
  assert.deepEqual(out.solver, doc.solver);
  // Without the pending flag the stale state would have been written.
  assert.notDeepEqual(projectFromState(s, { loaded: doc }).initial_conditions, doc.initial_conditions);
});

test("an axis switched from a state to time is written fresh, with no stale name", () => {
  const doc = fixture("unknown-fields.sepx");
  const { state } = openFixture(doc, ["k"]);
  const out = projectFromState({ ...state, view: { ...state.view, xAxis: { kind: "time" } } }, { loaded: doc });
  assert.deepEqual(out.views[0].x_axis, { kind: "time" });
  // Same kind: unknown keys of the axis are kept.
  assert.deepEqual(out.views[0].y_axis, { kind: "state", name: "y" });
  const kept = projectFromState(state, { loaded: doc });
  assert.deepEqual(kept.views[0].x_axis, { kind: "state", name: "x", scale: "linear" });
  // And back from time to a state: a fresh object again.
  const timeDoc = projectFromState({ ...state, view: { ...state.view, xAxis: { kind: "time" } } }, { loaded: doc });
  timeDoc.views[0].x_axis.extra = 1;
  const back = projectFromState(state, { loaded: timeDoc });
  assert.deepEqual(back.views[0].x_axis, { kind: "state", name: "x" });
});

test("a start point without a uid is saved as ic-<id>; repeated ids get a suffix", () => {
  assert.equal(icUid({ id: 7 }), "ic-7");
  assert.equal(icUid({ id: 7, uid: "ic-2" }), "ic-2");
  const t = templates.find((x) => x.id === "rabbits-vs-sheep");
  const sys = fakeCompile(t);
  const s = templateState(t, "latex", sys);
  s.ics = [
    { id: 3, x0: [0, 0], t0: 0, color: 0 },
    { id: 4, uid: "ic-3", x0: [1, 1], t0: 0, color: 1 },
  ];
  const out = projectFromState(s);
  assert.deepEqual(
    out.initial_conditions.map((ic) => ic.id),
    ["ic-3", "ic-3-2"],
  );
});

test("x0 follows the file's states by name", () => {
  const doc = fixture("rabbits-vs-sheep.sepx");
  doc.system.states = ["y", "x"];
  doc.initial_conditions = [{ id: "ic-1", x0: [2, 1], t0: 0, color: 9, hidden: false }];
  doc.views[0].slice = { states: { x: 0.5, y: 0.25 }, t: 3 };
  const sys = { ...openFixture(fixture("rabbits-vs-sheep.sepx")).sys };
  let id = 1;
  const a = applyProjectLoad(stateFromProject(doc), sys, () => id++);
  assert.deepEqual(a.ics[0].x0, [1, 2]);
  assert.equal(a.ics[0].color, 2, "colours are taken mod 7");
  assert.deepEqual(a.baseState, [0.5, 0.25]);
  assert.equal(a.fieldTime, 3);
});

test("a bad range, or an axis the system lacks, gives the default view with a notice", () => {
  const sys = openFixture(fixture("rabbits-vs-sheep.sepx")).sys;
  for (const change of [
    (d) => (d.views[0].x_range = [2, 1]),
    (d) => (d.views[0].y_range = [0, null]),
    (d) => (d.views[0].x_axis = { kind: "state", name: "w" }),
    (d) => (d.views[0].x_axis = { kind: "parameter", name: "k" }),
    (d) => (d.views[0].y_axis = { kind: "state", name: "x" }),
  ]) {
    const doc = fixture("rabbits-vs-sheep.sepx");
    change(doc);
    const a = applyProjectLoad(stateFromProject(doc), sys, () => 1);
    assert.deepEqual(a.view, defaultView(sys));
    assert.equal(a.warnings.length, 1);
  }
});

test("the primary view is `main`, otherwise the first", () => {
  assert.equal(primaryViewIndex([{ id: "a" }, { id: "main" }]), 1);
  assert.equal(primaryViewIndex([{ id: "a" }, { id: "b" }]), 0);
  assert.equal(primaryViewIndex([]), -1);
  const doc = fixture("unknown-fields.sepx");
  doc.views.reverse();
  const { state, load } = openFixture(doc, ["k"]);
  assert.equal(load.view.id, "main");
  const out = projectFromState({ ...state, fieldTime: 4 }, { loaded: doc });
  assert.equal(out.views[0].id, "bifurcation");
  assert.deepEqual(out.views[0], doc.views[0]);
  assert.equal(out.views[1].slice.t, 4);
});

test("parameters: the file's values and ranges, a default for one it lacks, a repaired range", () => {
  const doc = fixture("rabbits-vs-sheep.sepx");
  doc.system.params = [
    { id: "a", value: 3, min: 5, max: 1 },
    { id: "gone", value: 1, min: 0, max: 2 },
  ];
  const { applied } = openFixture(doc, ["a", "b"]);
  assert.deepEqual(applied.params, [
    { name: "a", latex: "a", value: 3, min: 0, max: 10 },
    { name: "b", latex: "b", value: 1, min: 0, max: 2 },
  ]);
});

test("the solver: an unknown method falls back to dopri5 with a notice; missing values take the defaults", () => {
  const doc = fixture("rabbits-vs-sheep.sepx");
  doc.solver = { method: "rk45", t_end: 12 };
  const load = stateFromProject(doc);
  assert.equal(load.warnings.length, 1);
  assert.match(load.warnings[0], /“rk45”/);
  const { applied } = openFixture(doc);
  assert.deepEqual(applied.solver, { ...DEFAULT_SOLVER, tEnd: 12 });
  assert.deepEqual([...KNOWN_METHODS].sort(), ["dopri5", "euler", "heun", "rk4", "verlet", "yoshida4"]);
});

test("dirty tracking ignores the export options and the app version", () => {
  const doc = fixture("rabbits-vs-sheep.sepx");
  const { state } = openFixture(doc);
  const a = dirtyKey(state, { loaded: doc });
  assert.equal(a, dirtyKey(state, { loaded: { ...doc, export_settings: { png: 2 }, app_version: "9.9.9" } }));
  assert.notEqual(a, dirtyKey({ ...state, fieldTime: 1 }, { loaded: doc }));
  const withSettings = projectFromState(state, { loaded: doc, exportSettings: { latex: { widthCm: 8 } } });
  assert.deepEqual(withSettings.export_settings, { latex: { widthCm: 8 } });
  assert.ok(!("export_settings" in projectFromState(state, { loaded: doc })));
});

test("export options merge onto export_settings: unknown keys survive a change", () => {
  const doc = { ...fixture("unknown-fields.sepx"), export_settings: { format: "svg", lyapunovDigits: 6, future: { a: 1 } } };
  const { state } = openFixture(doc);
  // The dialog reads the options, the user changes the width, the dialog reports.
  const opts = { ...readExportOptions(doc.export_settings), widthCm: 10 };
  const merged = mergeExportSettings(doc.export_settings, opts);
  const out = projectFromState(state, { loaded: doc, exportSettings: merged });
  assert.equal(out.export_settings.lyapunovDigits, 6);
  assert.deepEqual(out.export_settings.future, { a: 1 });
  assert.equal(out.export_settings.format, "svg");
  assert.equal(out.export_settings.widthCm, 10);
  // Nothing stored, or something that isn't an object: just the options.
  assert.deepEqual(mergeExportSettings(undefined, opts), { ...opts });
  assert.deepEqual(mergeExportSettings([1], opts), { ...opts });
});

test("a document that never had a file writes complete sections", () => {
  const t = templates.find((x) => x.id === "logistic-map");
  const sys = fakeCompile(t);
  const out = projectFromState(templateState(t, "latex", sys), { appVersion: "0.2.0" });
  assert.deepEqual(Object.keys(out), [
    "format",
    "format_version",
    "min_reader_version",
    "app_version",
    "system",
    "initial_conditions",
    "views",
    "analyses",
    "solver",
  ]);
  assert.equal(out.system.kind, "map");
  // The logistic map opens as its cobweb: x_n across, x_{n+1} (the next iterate) up.
  assert.deepEqual(Object.keys(out.views[0]).slice(0, 4), ["id", "kind", "x_axis", "y_axis"]);
  assert.equal(out.views[0].kind, "cobweb");
  assert.deepEqual(out.views[0].x_axis, { kind: "state", name: "x" });
  assert.deepEqual(out.views[0].y_axis, { kind: "next", name: "x" });
  assert.equal(out.app_version, "0.2.0");
});

test("names: file stem, window title, the Save dialog's suggestion", () => {
  assert.equal(projectStem("/home/a/rabbits-vs-sheep.sepx"), "rabbits-vs-sheep");
  assert.equal(projectStem("C:\\Users\\a\\Lorenz.SEPX"), "Lorenz");
  assert.equal(documentName("lorenz.sepx", "Lorenz system"), "lorenz");
  assert.equal(documentName(null, "Lorenz system"), "Lorenz system");
  assert.equal(windowTitle("rabbits-vs-sheep", false), "rabbits-vs-sheep — Separatrix");
  assert.equal(windowTitle("rabbits-vs-sheep", true), "● rabbits-vs-sheep — Separatrix");
  assert.equal(suggestedProjectName("hw.sepx", "lorenz", "Lorenz"), "hw.sepx");
  assert.equal(suggestedProjectName(null, "lorenz", "Lorenz"), "lorenz.sepx");
  assert.equal(suggestedProjectName(null, null, "Untitled system"), "untitled-system.sepx");
  assert.equal(suggestedProjectName(null, "blank", "Untitled system"), "untitled-system.sepx");
  assert.equal(suggestedProjectName(null, null, "§§"), "untitled.sepx");
  assert.equal(slug("Rössler attractor"), "rossler-attractor");
});

const key = (over) => ({
  key: "s",
  code: "KeyS",
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  altKey: false,
  isComposing: false,
  keyCode: 83,
  ...over,
});
const PC = { mac: false, tauri: true };

test("shortcuts: Mod+S, Mod+Shift+S, Mod+O, and Mod+N only in the desktop app", () => {
  assert.equal(matchShortcut(key({ ctrlKey: true }), PC), "save");
  assert.equal(matchShortcut(key({ ctrlKey: true, shiftKey: true, key: "S" }), PC), "saveAs");
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "o", code: "KeyO" }), PC), "open");
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "n", code: "KeyN" }), PC), "new");
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "n", code: "KeyN" }), { mac: false, tauri: false }), null);
  assert.equal(matchShortcut(key({ metaKey: true }), { mac: true, tauri: true }), "save");
  assert.equal(matchShortcut(key({ ctrlKey: true }), { mac: true, tauri: true }), null);
  assert.equal(matchShortcut(key({ metaKey: true }), PC), null);
  assert.equal(matchShortcut(key({ ctrlKey: true, altKey: true }), PC), null);
  assert.equal(matchShortcut(key({}), PC), null);
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "x", code: "KeyX" }), PC), null);
});

test("shortcuts: Mod+E opens the export dialog, in the browser preview too", () => {
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "e", code: "KeyE" }), PC), "export");
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "e", code: "KeyE" }), { mac: false, tauri: false }), "export");
  assert.equal(matchShortcut(key({ metaKey: true, key: "e", code: "KeyE" }), { mac: true, tauri: true }), "export");
  assert.equal(matchShortcut(key({ ctrlKey: true, shiftKey: true, key: "E", code: "KeyE" }), PC), null);
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "у", code: "KeyE" }), PC), "export");
  assert.equal(shortcutLabel("s", { mac: false, shift: true }), "Ctrl+Shift+S");
  assert.equal(shortcutLabel("e", { mac: true }), "⌘E");
});

test("shortcuts: a Cyrillic layout matches by the physical key", () => {
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "ы", code: "KeyS" }), PC), "save");
  assert.equal(matchShortcut(key({ ctrlKey: true, shiftKey: true, key: "Ы", code: "KeyS" }), PC), "saveAs");
  assert.equal(matchShortcut(key({ ctrlKey: true, key: "щ", code: "KeyO" }), PC), "open");
  // An ASCII letter wins over the physical key (Dvorak: the S key types "o").
  assert.equal(shortcutLetter({ key: "o", code: "KeyS" }), "o");
  assert.equal(shortcutLetter({ key: "1", code: "Digit1" }), null);
});

test("shortcuts: ignored while an IME is composing", () => {
  assert.equal(matchShortcut(key({ ctrlKey: true, isComposing: true }), PC), null);
  assert.equal(matchShortcut(key({ ctrlKey: true, keyCode: 229 }), PC), null);
});

test("platform: Cmd on macOS", () => {
  assert.equal(isMacPlatform({ platform: "MacIntel" }), true);
  assert.equal(isMacPlatform({ platform: "Win32" }), false);
  assert.equal(isMacPlatform({ platform: "", userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)" }), true);
  assert.equal(isMacPlatform(undefined), false);
});

// --- cobweb views (docs/project-format.md: views[].kind, the "next" axis, solver.transient) ---

const mapSys = (states = ["x"], autonomous = true) => ({
  ok: true,
  systemId: 1,
  kind: "map",
  hamiltonian: false,
  autonomous,
  states: states.map((n) => st(n)),
  params: [{ name: "r", latex: "r", default: 3.2 }],
  equationsLatex: [],
  diagnostics: [],
});

function cobwebDoc() {
  const t = templates.find((x) => x.id === "logistic-map");
  const sys = mapSys();
  const s = { ...templateState(t, "latex", sys), solver: { ...DEFAULT_SOLVER, tEnd: 60, transient: 200 } };
  return { s, sys, doc: JSON.parse(JSON.stringify(projectFromState(s, { appVersion: "0.2.0" }))) };
}

test("a cobweb document writes kind after id, the next-iterate axis, and the transient after t_end", () => {
  const { doc } = cobwebDoc();
  const v = doc.views[0];
  assert.deepEqual(Object.keys(v).slice(0, 4), ["id", "kind", "x_axis", "y_axis"]);
  assert.equal(v.kind, "cobweb");
  assert.deepEqual(v.x_axis, { kind: "state", name: "x" });
  assert.deepEqual(v.y_axis, { kind: "next", name: "x" });
  assert.deepEqual(v.x_range, [-0.05, 1.05]);
  const keys = Object.keys(doc.solver);
  assert.equal(doc.solver.transient, 200);
  assert.equal(keys.indexOf("transient"), keys.indexOf("t_end") + 1);
});

test("a plane document is written as before: no kind, no transient", () => {
  const doc = fixture("rabbits-vs-sheep.sepx");
  const { state } = openFixture(doc);
  const out = JSON.parse(JSON.stringify(projectFromState(state, { loaded: doc })));
  assert.ok(!("kind" in out.views[0]));
  assert.ok(!("transient" in out.solver));
  assert.deepEqual(Object.keys(out.views[0]), Object.keys(doc.views[0]));
  assert.deepEqual(Object.keys(out.solver), Object.keys(doc.solver));
  // A view that was a cobweb and is now a time series drops kind and the next axis.
  const { s, doc: cdoc } = cobwebDoc();
  const series = { ...s, view: { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 60], yRange: [0, 1] }, solver: { ...s.solver, transient: 0 } };
  const back = projectFromState(series, { loaded: cdoc });
  assert.ok(!("kind" in back.views[0]));
  assert.deepEqual(back.views[0].y_axis, { kind: "state", name: "x" });
  assert.deepEqual(back.views[0].x_axis, { kind: "time" });
  assert.ok(!("transient" in back.solver));
});

test("a cobweb document reopens as the same cobweb, transient included, and saves again unchanged", () => {
  const { s, sys, doc } = cobwebDoc();
  const load = stateFromProject(doc);
  assert.equal(load.view.kind, "cobweb");
  assert.deepEqual(load.view.yAxis, { kind: "next", name: "x" });
  assert.equal(load.solver.transient, 200);
  const a = applyProjectLoad(load, sys, () => 1);
  assert.deepEqual(a.warnings, []);
  assert.deepEqual(a.view, s.view);
  assert.equal(a.solver.transient, 200);
  const again = projectFromState({ ...s, view: a.view, solver: a.solver }, { loaded: doc, appVersion: "0.2.0" });
  assert.equal(JSON.stringify(again), JSON.stringify(doc));
});

test("the cobweb fixture (crates/project) opens as a cobweb", { skip: !existsFixture("cobweb.sepx") && "no fixture yet" }, () => {
  const doc = fixture("cobweb.sepx");
  const sys = mapSys(doc.system.states);
  const a = applyProjectLoad(stateFromProject(doc), sys, () => 1);
  assert.deepEqual(a.warnings, []);
  assert.equal(a.view.kind, "cobweb");
  assert.deepEqual(a.view.xAxis, { kind: "state", index: 0 });
  assert.deepEqual(a.view.yAxis, { kind: "state", index: 0 });
});

test("a saved cobweb that doesn't fit the system, or an unknown kind, gives the default view with a notice", () => {
  const { doc } = cobwebDoc();
  const cases = [
    { sys: mapSys(["x", "y"]), change: () => {} },
    { sys: mapSys(["x"], false), change: () => {} },
    { sys: { ...mapSys(), kind: "ode" }, change: () => {} },
    // An older build re-saved it with time axes but kept `kind`.
    { sys: mapSys(), change: (d) => (d.views[0].x_axis = { kind: "time" }) },
    { sys: mapSys(), change: (d) => (d.views[0].y_axis = { kind: "state", name: "x" }) },
    { sys: mapSys(), change: (d) => (d.views[0].y_axis = { kind: "next", name: "q" }) },
    { sys: mapSys(), change: (d) => (d.views[0].kind = "bifurcation") },
  ];
  for (const c of cases) {
    const d = structuredClone(doc);
    c.change(d);
    const a = applyProjectLoad(stateFromProject(d), c.sys, () => 1);
    // The default view for the file's solver: a 1-D map's time series starts at its transient.
    assert.deepEqual(a.view, defaultView(c.sys, a.solver));
    if (c.sys.kind === "map" && c.sys.states.length === 1 && !c.sys.autonomous) {
      assert.equal(a.solver.transient, 200);
      assert.deepEqual(a.view.xRange, [200, 250], "n from the transient on");
    }
    assert.deepEqual(a.warnings, ["The saved cobweb view doesn't fit this system, so the default view is shown."]);
  }
  // A plane view naming the next-iterate axis gets the sentence project_read
  // gives it (docs/project-format.md), so the banner says it once.
  const d = structuredClone(doc);
  delete d.views[0].kind;
  const a = applyProjectLoad(stateFromProject(d), mapSys(), () => 1);
  assert.deepEqual(a.view, defaultView(mapSys()));
  assert.deepEqual(a.warnings, ["The saved cobweb view doesn't fit this system, so the default view is shown."]);
  // A plane view of an axis the system lacks keeps the general sentence.
  const e = structuredClone(doc);
  delete e.views[0].kind;
  e.views[0].x_axis = { kind: "state", name: "q" };
  e.views[0].y_axis = { kind: "state", name: "x" };
  const b = applyProjectLoad(stateFromProject(e), mapSys(), () => 1);
  assert.deepEqual(b.warnings, ["The saved view doesn't fit this system, so the default view is shown."]);
});

test("the transient: a whole float is read as a whole number; out of range it is clamped", () => {
  const { doc } = cobwebDoc();
  for (const [saved, read] of [
    [200.0, 200],
    [12.4, 12],
    [-3, 0],
    [5e6, 1_000_000],
  ]) {
    const d = structuredClone(doc);
    d.solver.transient = saved;
    assert.equal(stateFromProject(d).solver.transient, read);
  }
  const d = structuredClone(doc);
  delete d.solver.transient;
  assert.equal(stateFromProject(d).solver.transient, undefined);
  assert.equal(applyProjectLoad(stateFromProject(d), mapSys(), () => 1).solver.transient, 0);
  // With N iterates after it, transient + N stays within the core's limit.
  d.solver.transient = 1_000_000;
  assert.equal(applyProjectLoad(stateFromProject(d), mapSys(), () => 1).solver.transient, 1_000_000 - 60);
});
