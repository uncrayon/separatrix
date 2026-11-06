// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports resolve to .ts through a module hook.

import test from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";

registerHooks({
  resolve(specifier, context, next) {
    try {
      return next(specifier, context);
    } catch (err) {
      if (/^\.\.?\//.test(specifier) && !/\.[cm]?[jt]sx?$/.test(specifier)) return next(`${specifier}.ts`, context);
      throw err;
    }
  },
});

const model = await import("../state/model.ts");
const { MAX_ORBIT_COLUMNS, MAX_ORBIT_POINTS, ORBIT_MISFIT } = await import("../api/ipc.generated.ts");
const { projectFromState, stateFromProject } = await import("../project/serialize.ts");
const { densityAlpha, densityPixels, orbitHistogram } = await import("../canvas/orbit.ts");

const st = (name) => ({ name, latex: name, derived: false });
const map = (names, params, systemId = 5, extra = {}) => ({
  ok: true,
  systemId,
  kind: "map",
  hamiltonian: false,
  separable: false,
  autonomous: true,
  states: names.map(st),
  params: params.map((name) => ({ name, latex: name, default: 1 })),
  equationsLatex: [],
  diagnostics: [],
  ...extra,
});
const logistic = map(["x"], ["r"]);
const henon = map(["x", "y"], ["a", "b"], 6);
const ic = (id, x0) => ({ id, uid: `ic-${id}`, x0, t0: 0, color: id % 7 });
const params = (sys, values, ranges) =>
  sys.params.map((p, i) => ({ name: p.name, latex: p.latex, value: values[i], min: ranges[i][0], max: ranges[i][1] }));

function snapshot(over = {}) {
  const sys = over.compiled ?? logistic;
  const ps = over.params ?? params(sys, [3.2], [[0, 4]]);
  return {
    compiled: sys,
    params: ps,
    view: model.orbitView(ps, [-0.05, 1.05]),
    ics: [ic(1, sys.states.map(() => 0.1))],
    shapeId: 3,
    selection: null,
    orbitColumns: 1000,
    ...over,
  };
}

test("the orbit diagram is a map's view, offered only with a parameter to sweep", () => {
  assert.deepEqual(model.viewChoicesFor(logistic).at(-1), "orbit");
  assert.equal(model.viewChoicesFor({ ...logistic, kind: "ode" }).includes("orbit"), false);
  assert.deepEqual(model.viewChoiceAvailability(logistic).orbit, { ok: true });
  assert.deepEqual(model.orbitAvailability(map(["x"], []), "Tent map"), {
    ok: false,
    reason: "The orbit diagram needs a map with a parameter; Tent map has none.",
  });
  assert.equal(model.VIEW_CHOICE_LABELS.orbit, "Orbit diagram");
  assert.equal(model.viewChoice(model.orbitView([{ min: 0, max: 4 }], [0, 1])), "orbit");
});

test("one column per device pixel, at most MAX_ORBIT_COLUMNS", () => {
  assert.equal(model.orbitColumns(900, 1), 900);
  assert.equal(model.orbitColumns(900, 2), 1800);
  assert.equal(model.orbitColumns(900.4, 1.25), 1126);
  assert.equal(model.orbitColumns(3000, 2), MAX_ORBIT_COLUMNS);
  assert.equal(model.orbitColumns(0.5, 1), 2);
  assert.ok(model.ORBIT_MAX_KEPT * MAX_ORBIT_COLUMNS <= MAX_ORBIT_POINTS, "the most iterates fit the widest plot");
});

test("column j is the j-th pixel's centre, and a click sets the r of the column under it", () => {
  const xRange = [2.8, 4];
  const [lo, hi] = model.orbitRange(xRange, 1200);
  assert.ok(Math.abs(lo - 2.8005) < 1e-12 && Math.abs(hi - 3.9995) < 1e-12);
  const at = (x) => model.orbitColumnAt(xRange, 1200, x);
  assert.equal(at(3.8304).column, 1030);
  assert.ok(Math.abs(at(3.8304).r - 3.8305) < 1e-12);
  assert.ok(Math.abs(at(3.83).r - 3.83) <= 0.001, "within one column width of the click");
  assert.equal(at(2.8).column, 0);
  assert.equal(at(4).column, 1199, "the right edge is the last column");
  assert.equal(at(1).column, 0, "outside the range: the nearest column");
  const step = (hi - lo) / 1199;
  for (const j of [0, 17, 600, 1199]) assert.equal(at(2.8 + (j + 0.5) * 0.001).r, lo + j * step);
});

test("the request sweeps the view's range, from the start point, with doublings for a 1-D map", () => {
  const s = snapshot();
  const job = model.buildOrbitJob(s);
  assert.deepEqual(job.body, {
    systemId: 5,
    params: [0.002],
    param: 0,
    range: [0.002, 3.998],
    columns: 1000,
    state: 0,
    x0: [0.1],
    transient: model.ORBIT_DEFAULTS.transient,
    kept: model.ORBIT_DEFAULTS.kept,
    doublings: true,
  });
  assert.deepEqual(model.ORBIT_DEFAULTS, { transient: 1000, kept: 200 });

  const key = (over) => model.buildOrbitJob({ ...s, ...over })?.meta.key;
  assert.equal(key({ params: params(logistic, [3.83], [[0, 4]]) }), job.meta.key, "moving r's slider asks for nothing");
  assert.notEqual(key({ view: { ...s.view, xRange: [2.8, 4] } }), job.meta.key, "zooming across asks again");
  assert.equal(key({ view: { ...s.view, yRange: [0.2, 0.9] } }), job.meta.key, "zooming up asks for nothing");
  assert.notEqual(key({ orbitColumns: 1200 }), job.meta.key, "a wider plot asks again");
  assert.notEqual(key({ view: { ...s.view, orbit: { transient: 2000, kept: 200 } } }), job.meta.key);

  assert.equal(key({ ics: [] }), undefined, "no start point, no diagram");
  assert.equal(key({ orbitColumns: 0 }), undefined, "no width yet");
  assert.equal(key({ view: { ...s.view, orbit: { transient: 1000, kept: model.ORBIT_MAX_KEPT + 1 } } }), undefined);
  assert.equal(key({ view: { ...s.view, orbit: { transient: 0.5, kept: 200 } } }), undefined);
  assert.equal(key({ compiled: { ...logistic, kind: "ode" } }), undefined);

  const hs = snapshot({ compiled: henon, params: params(henon, [1.4, 0.3], [[0, 1.5], [-0.5, 0.5]]) });
  const hjob = model.buildOrbitJob({ ...hs, view: { ...hs.view, yAxis: model.stateAxis(1) } });
  assert.equal(hjob.body.doublings, false, "doublings are for one-dimensional maps");
  assert.deepEqual(hjob.body.params, [hjob.body.range[0], 0.3], "the other parameters as set");
  assert.equal(hjob.body.state, 1);
  assert.equal(model.buildOrbitJob({ ...snapshot(), compiled: { ...logistic, autonomous: false } }).body.doublings, false);
});

test("a diagram belongs to the shape, parameter and state it was drawn for", () => {
  const s = snapshot();
  const job = model.buildOrbitJob(s);
  const run = { res: { r: [], x: [] }, meta: job.meta, roundTripMs: 1 };
  assert.equal(model.orbitCurrent({ ...s, orbit: run }), true);
  assert.equal(model.orbitCurrent({ ...s, orbitColumns: 999, orbit: run }), false);
  assert.equal(model.orbitFits(logistic, 3, s.view, run), true);
  assert.equal(model.orbitFits(logistic, 4, s.view, run), false, "another shape of system");
  assert.equal(model.orbitFits(logistic, 3, { ...s.view, param: 1 }, run), false, "another parameter");
});

test("switching to the orbit diagram sweeps the first parameter's slider range over the state's range now", () => {
  const ps = params(logistic, [3.2], [[2.5, 4]]);
  const cobweb = model.cobwebView([-0.05, 1.05]);
  const v = model.switchView("orbit", { compiled: logistic, view: cobweb, solver: model.DEFAULT_SOLVER, params: ps }, {});
  assert.deepEqual(v, {
    kind: "orbit",
    xAxis: { kind: "time" },
    yAxis: { kind: "state", index: 0 },
    xRange: [2.5, 4],
    yRange: [-0.05, 1.05],
    param: 0,
    orbit: { transient: 1000, kept: 200 },
  });
  assert.equal(model.buildPortraitJob({ ...snapshot(), view: v, baseState: [0], fieldTime: 0, layers: model.DEFAULT_LAYERS, solver: model.DEFAULT_SOLVER, plotAspect: 1 }), null);
  assert.equal(model.canAddOnPlot(v), false);
  assert.equal(model.shiftTimeAxes(v, 10), v, "the transient of the Iteration panel moves nothing here");
});

test("after a recompile the diagram follows its parameter and state by name", () => {
  const ps = params(henon, [1.4, 0.3], [[0, 1.5], [-0.5, 0.5]]);
  const v = { ...model.orbitView(ps, [-1.5, 1.5], 1), yAxis: model.stateAxis(1) };
  const P = (...names) => names.map((name) => ({ name }));
  assert.equal(model.followOrbitParam(v, P("a", "b"), P("a", "b")), v);
  assert.equal(model.followOrbitParam(v, P("a", "b"), P("b", "c", "a")).param, 0, "b moved first");
  assert.equal(model.followOrbitParam(v, P("a", "b"), P("a", "beta")).param, 1, "renamed in place");
  assert.equal(model.followOrbitParam(v, P("a", "b"), P("a")), null, "gone");
  const states = (...names) => names.map(st);
  assert.deepEqual(model.adaptView(v, states("x", "y"), states("y", "x"), henon).yAxis, model.stateAxis(0));
  assert.equal(model.adaptView(v, states("x", "y"), states("x", "y"), { kind: "ode", autonomous: true }), null);
});

test("Strogatz's r₁, r₂, … name the cascade the doublings start with, other doublings their periods", () => {
  const d = [
    { r: 3, period: 1 },
    { r: 3.449489742783178, period: 2 },
    { r: 3.5440903, period: 4 },
    { r: 3.5644073, period: 8 },
    { r: 3.8414990, period: 3 },
  ];
  assert.deepEqual(
    model.doublingRows(d).map((row) => row.label),
    ["r₁", "r₂", "r₃", "r₄", "3 → 6"],
  );
  assert.deepEqual(model.doublingRows(d.slice(2), "a").map((row) => row.label), ["a₃", "a₄", "3 → 6"], "a zoomed range starts at r₃");
  assert.deepEqual(model.doublingRows([]), []);
  assert.equal(model.doublingRows(d)[1].r, d[1].r);
});

test("iterates are binned per pixel, escaped and off-plot ones left out", () => {
  const res = { r: [0.5, 1.5], x: [0.1, 0.1, 0.9, null, 0.6, 1.5, -0.2, 0.6] };
  const h = orbitHistogram(res, { xRange: [0, 2], yRange: [0, 1] }, 2, 4);
  assert.deepEqual([...h.counts], [1, 0, 0, 2, 0, 0, 2, 0]);
  assert.equal(h.max, 2);
  const off = orbitHistogram({ r: [5], x: [0.5] }, { xRange: [0, 2], yRange: [0, 1] }, 2, 4);
  assert.equal(off.max, 0, "a column outside the range draws nothing");
});

test("density is log-scaled opacity: empty pixels clear, the busiest opaque, one visit still visible", () => {
  assert.equal(densityAlpha(0, 100), 0);
  assert.equal(densityAlpha(100, 100), 1);
  assert.equal(densityAlpha(1, 1), 1);
  assert.ok(densityAlpha(1, 100) >= 0.25 && densityAlpha(1, 100) < 0.5);
  assert.ok(Math.abs(densityAlpha(10, 100) - (densityAlpha(1, 100) + 1) / 2) < 1e-12, "log scale: 10 is halfway to 100");
  const px = densityPixels({ width: 2, height: 1, counts: new Uint32Array([0, 4]), max: 4 }, [10, 20, 30]);
  assert.deepEqual([...px], [0, 0, 0, 0, 10, 20, 30, 255]);
});

test("the vertical fit spans every finite iterate, padded", () => {
  const [lo, hi] = model.fitOrbitRange({ x: [0.2, null, 0.7, 0.45] });
  assert.ok(Math.abs(lo - 0.16) < 1e-12 && Math.abs(hi - 0.74) < 1e-12);
  assert.equal(model.fitOrbitRange({ x: [null] }), null);
});

const doc = (view, systemKind = "map", sysParams = [{ id: "r", value: 3.2, min: 0, max: 4 }]) => ({
  format: "separatrix",
  format_version: 1,
  min_reader_version: 1,
  app_version: "",
  system: { kind: systemKind, input_format: "latex", lines: [], states: ["x"], params: sysParams, template: null },
  initial_conditions: [],
  views: [view],
  analyses: [],
  solver: {},
});

test("an orbit diagram is saved as kind orbit with r by name and its columns, and opens as it was", () => {
  const ps = params(logistic, [3.2], [[0, 4]]);
  const view = { ...model.orbitView(ps, [-0.05, 1.05], 0, 0, { transient: 2000, kept: 300 }), xRange: [2.8, 4] };
  const state = {
    format: "latex",
    equations: [{ id: 1, source: "x_{n+1}=rx_n\\left(1-x_n\\right)" }],
    compiled: logistic,
    params: ps,
    view,
    baseState: [0],
    fieldTime: 0,
    ics: [ic(1, [0.1])],
    layers: model.DEFAULT_LAYERS,
    solver: model.DEFAULT_SOLVER,
    sourceTemplateId: "logistic-map",
  };
  const file = projectFromState(state);
  const v = file.views[0];
  assert.equal(v.kind, "orbit");
  assert.deepEqual(v.x_axis, { kind: "param", name: "r" });
  assert.deepEqual(v.y_axis, { kind: "state", name: "x" });
  assert.deepEqual(v.orbit, { transient: 2000, kept: 300 });
  assert.deepEqual(v.x_range, [2.8, 4]);
  assert.equal(v.lyapunov, undefined);

  const load = stateFromProject(JSON.parse(JSON.stringify(file)));
  const applied = model.applyProjectLoad(load, logistic, () => 1);
  assert.deepEqual(applied.warnings, []);
  assert.deepEqual(applied.view, view);
  const reopened = { ...file, views: [{ ...v, orbit: { ...v.orbit, note: "kept" }, x_axis: { ...v.x_axis, tag: 1 } }] };
  const again = projectFromState({ ...state, view: applied.view }, { loaded: reopened });
  assert.deepEqual(again.views[0], reopened.views[0], "unknown keys are kept");

  const cobweb = projectFromState({ ...state, view: model.cobwebView([0, 1]) }, { loaded: reopened });
  assert.equal(cobweb.views[0].orbit, undefined, "leaving the view drops its columns");
  assert.deepEqual(cobweb.views[0].x_axis, { kind: "state", name: "x" });
});

test("a saved orbit diagram that can't be drawn opens the default view with one warning", () => {
  const base = {
    id: "main",
    kind: "orbit",
    x_axis: { kind: "param", name: "r" },
    y_axis: { kind: "state", name: "x" },
    x_range: [2.8, 4],
    y_range: [0, 1],
    orbit: { transient: 1000, kept: 200 },
    slice: { states: {}, t: 0 },
    layers: {},
  };
  const open = (view, sys = logistic) => model.applyProjectLoad(stateFromProject(doc(view)), sys, () => 1);
  assert.deepEqual(open(base).warnings, []);
  assert.equal(open(base).view.kind, "orbit");
  for (const [why, view, sys] of [
    ["no columns", { ...base, orbit: undefined }],
    ["nothing drawn", { ...base, orbit: { transient: 1000, kept: 0 } }],
    ["too many drawn", { ...base, orbit: { transient: 1000, kept: 977 } }],
    ["a parameter it lacks", { ...base, x_axis: { kind: "param", name: "mu" } }],
    ["time up the side", { ...base, y_axis: { kind: "time" } }],
    ["a flow", base, { ...logistic, kind: "ode" }],
  ]) {
    const applied = open(view, sys);
    assert.deepEqual(applied.warnings, [ORBIT_MISFIT], why);
    assert.notEqual(applied.view.kind, "orbit", why);
  }
});
