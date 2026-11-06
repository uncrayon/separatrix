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
const { SECTION_MISFIT, SPACE_MISFIT } = await import("../api/ipc.generated.ts");
const draw = await import("../canvas/draw.ts");
const { makeViewport, sx, sy } = await import("../canvas/viewport.ts");
const { projectFromState, stateFromProject } = await import("../project/serialize.ts");
const { viewAxisLabels } = await import("../math/format.ts");
const { sectionCounts } = await import("../components/runStatus.ts");

const st = (name) => ({ name, latex: name, derived: false });
const flow = (names, extra = {}) => ({
  ok: true,
  systemId: 7,
  kind: "ode",
  hamiltonian: false,
  autonomous: true,
  states: names.map(st),
  params: [],
  equationsLatex: [],
  diagnostics: [],
  ...extra,
});
const lorenz = flow(["x", "y", "z"]);
const forced = flow(["theta", "omega"], { autonomous: false });
const logistic = { ...flow(["x"]), kind: "map" };

const plane = (state, value, direction = "rising") => ({ kind: "plane", state, value, direction });
const S = (i) => ({ kind: "state", index: i });

test("the View control offers each kind its own options, with reasons for the rest", () => {
  assert.deepEqual(model.viewChoicesFor(lorenz), ["phase", "series", "section", "space", "lyapunov"]);
  assert.deepEqual(model.viewChoicesFor(logistic), ["phase", "series", "cobweb", "lyapunov", "orbit"]);
  assert.equal(model.VIEW_CHOICE_SHORT.section, "Section");
  assert.deepEqual(model.sectionAvailability(lorenz), { ok: true });
  assert.deepEqual(model.sectionAvailability(flow(["x"])), { ok: true }, "a stroboscopic section works in one dimension");
  assert.deepEqual(model.sectionAvailability(logistic, "Logistic map"), {
    ok: false,
    reason: "A Poincaré section needs a flow; Logistic map is a map.",
  });
  assert.equal(model.planeSectionAvailability(flow(["x"]), "Logistic growth").ok, false);
  assert.match(model.planeSectionAvailability(flow(["x"]), "Logistic growth").reason, /^A plane section needs at least 2 states\. Logistic growth has 1/);
});

test("switching to the section view and back keeps the remembered view", () => {
  const phase = { xAxis: S(0), yAxis: S(2), xRange: [-25, 25], yRange: [-2, 64] };
  const memory = {};
  const s = { compiled: lorenz, view: phase, solver: model.DEFAULT_SOLVER, portrait: null };

  const section = model.switchView("section", s, memory);
  assert.deepEqual(section, {
    kind: "section",
    section: plane(0, 0),
    xAxis: S(1),
    yAxis: S(2),
    xRange: [-5, 5],
    yRange: [-2, 64],
  });
  assert.equal(model.viewChoice(section), "section");
  assert.deepEqual(memory.phase, phase, "the phase plane is remembered");

  const z27 = model.withSection(section, lorenz, plane(2, 27), () => [-20, 20]);
  assert.deepEqual([z27.xAxis, z27.yAxis], [S(0), S(1)], "the plane's own state left the axes");
  assert.equal(model.switchView("phase", { ...s, view: z27 }, memory), phase);
  assert.deepEqual(model.switchView("section", { ...s, view: phase }, memory), z27);

  const crossings = { trajectories: [{ t: [0, 1], x: [0, -3, 27, 0, 4, 27], status: "completed" }] };
  assert.deepEqual(model.knownStateRange(0, [z27, phase], crossings, 3), [-20, 20]);
  assert.deepEqual(model.knownStateRange(0, [], crossings, 3), [-5, 5], "x is 0 at every crossing of x = 0");
  assert.deepEqual(model.knownStateRange(1, [], crossings, 3), [-3.7, 4.7]);

  const strobe = model.switchView("section", { ...s, compiled: forced, view: { xAxis: S(0), yAxis: S(1), xRange: [-4, 4], yRange: [-3, 3] } }, {});
  assert.deepEqual(strobe.section, { kind: "stroboscopic", period: 2 * Math.PI, phase: 0 });
  assert.deepEqual([strobe.xAxis, strobe.yAxis], [S(0), S(1)]);
  assert.equal(model.switchView("section", { ...s, compiled: logistic, view: model.cobwebView([0, 1]) }, {}), null);
});

test("a click lands on the section: the plane's state is its value exactly, a stroboscopic start is at the phase", () => {
  const value = 27.000000000000004;
  const view = { kind: "section", section: plane(2, value), xAxis: S(0), yAxis: S(1), xRange: [-20, 20], yRange: [-25, 25] };
  const { x0, t0 } = model.liftPlotPoint(lorenz, view, [0.5, 0.5, 3], 1.5, -7.25, 11.5);
  assert.deepEqual(x0, [-7.25, 11.5, value]);
  assert.equal(x0[2], value);
  assert.equal(t0, 1.5);
  assert.equal(model.liftPlotPoint(lorenz, view, [0, 0, 0], 0, 1, 2, { x0: [9, 9, 9], t0: 4 }).x0[2], value);

  const strobe = { ...view, section: { kind: "stroboscopic", period: 2 * Math.PI, phase: 0.75 } };
  assert.deepEqual(model.liftPlotPoint(forced, strobe, [0, 0], 3, 1, -2), { x0: [1, -2], t0: 0.75 });

  assert.equal(model.canAddOnPlot({ ...view, yAxis: S(0) }), false);
  assert.equal(model.canAddOnPlot(view), true);
});

test("the return map plots each crossing against the next, and labels them x_k and x_{k+1}", () => {
  const vp = makeViewport(400, 300, [0, 10], [0, 10], true);
  const out = { t: [0, 1, 2, 3], x: [9, 1, 9, 2, 9, 4, 9, 8], status: "completed", stats: { accepted: 0, rejected: 0, evals: 0 } };
  const expected = [
    [1, 2],
    [2, 4],
    [4, 8],
  ].flatMap(([a, b]) => [sx(vp, a), sy(vp, b)]);
  const scene = { kind: "ode", dim: 2, xAxis: S(1), yAxis: S(1), section: { returnMap: true } };
  assert.deepEqual([...draw.trajectoryPx(vp, scene, out)], expected);
  assert.deepEqual([...draw.trajectoryPx(vp, { ...scene, xAxis: S(0), section: { returnMap: false } }, out)], [1, 2, 4, 8].flatMap((v) => [sx(vp, 9), sy(vp, v)]));

  const view = { kind: "section", section: plane(0, 0), xAxis: S(1), yAxis: S(1), xRange: [0, 10], yRange: [0, 10] };
  assert.equal(model.isReturnMap(view), true);
  assert.deepEqual(viewAxisLabels(view, [st("x"), st("y")], "ode"), { x: "y_{k}", y: "y_{k+1}" });

  const calls = [];
  const ctx = new Proxy({}, {
    get: (_, k) => (k in ctx2 ? ctx2[k] : (...a) => calls.push([k, ...a])),
    set: () => true,
  });
  const ctx2 = { measureText: () => ({ width: 10 }) };
  const sceneFull = {
    ...scene,
    arrows: [],
    fieldGrid: 0,
    nullclines: [],
    fixedPoints: [],
    trajectories: [{ out, meta: { icId: 1, direction: "forward" }, color: 0 }],
    ics: [],
    selectedIc: null,
    selectedFp: null,
    hoverIc: null,
    hoverFp: null,
    eigenvectors: false,
    stale: false,
    avoid: { labels: [], field: [] },
  };
  const theme = new Proxy({ traj: ["#111"], lineScale: 1, glyphScale: 1 }, { get: (o, k) => (k in o ? o[k] : "#000") });
  draw.drawScene(ctx, 400, 300, vp, sceneFull, theme);
  const dots = calls.filter(([k, , , r]) => k === "arc" && r === 2.6).length;
  assert.equal(dots, 3);
});

test("a section portrait asks for the crossings only, after the skipped ones", () => {
  const view = { kind: "section", section: plane(2, 27), xAxis: S(0), yAxis: S(1), xRange: [-20, 20], yRange: [-25, 25] };
  const job = model.buildPortraitJob({
    compiled: lorenz,
    params: [],
    view,
    baseState: [0, 0, 0],
    fieldTime: 0,
    ics: [{ id: 1, x0: [0, 1, 0], t0: 0, color: 0 }],
    layers: model.DEFAULT_LAYERS,
    solver: { ...model.DEFAULT_SOLVER, tEnd: 500, transient: 10 },
    plotAspect: null,
    shapeId: 1,
  });
  assert.deepEqual(job.body.section, plane(2, 27));
  assert.equal(job.body.transient, 10);
  assert.equal(job.body.fieldGrid, 0);
  assert.equal(job.body.nullclines, false);
  assert.equal(job.body.fixedPoints, false);
  assert.deepEqual(job.body.trajectories, [{ x0: [0, 1, 0], t0: 0, t1: 500 }], "forward only, tEnd long");
  assert.deepEqual(sectionCounts({ trajectories: [{ t: [1, 2, 3] }, { t: [4] }] }, 10), { crossings: 4, skipPerOrbit: 10 });
});

function documentOf(sys, view, transient = 10) {
  return {
    format: "latex",
    equations: sys.states.map((s, i) => ({ id: i + 1, source: `\\dot{${s.name}}=0` })),
    compiled: sys,
    params: [],
    view,
    baseState: sys.states.map(() => 0),
    fieldTime: 0,
    ics: [{ id: 1, uid: "ic-1", x0: sys.states.map((_, i) => i + 0.5), t0: 0, color: 0 }],
    layers: model.DEFAULT_LAYERS,
    solver: { ...model.DEFAULT_SOLVER, tEnd: 500, transient },
    sourceTemplateId: null,
  };
}

test("plane, stroboscopic and return-map section views round-trip through .sepx identically", () => {
  const cases = [
    ["plane z = 27 rising", lorenz, { kind: "section", section: plane(2, 27), xAxis: S(0), yAxis: S(1), xRange: [-20, 20], yRange: [-25, 25] }],
    ["plane x = 0 falling, return map of y", lorenz, { kind: "section", section: plane(0, 0, "falling"), xAxis: S(1), yAxis: S(1), xRange: [-30, 30], yRange: [-30, 30] }],
    ["stroboscopic", forced, { kind: "section", section: { kind: "stroboscopic", period: 2 * Math.PI, phase: 0.5 }, xAxis: S(0), yAxis: S(1), xRange: [-4, 4], yRange: [-3, 3] }],
    ["stroboscopic return map", forced, { kind: "section", section: { kind: "stroboscopic", period: 3, phase: 0 }, xAxis: S(1), yAxis: S(1), xRange: [-3, 3], yRange: [-3, 3] }],
  ];
  for (const [name, sys, view] of cases) {
    const doc = JSON.parse(JSON.stringify(projectFromState(documentOf(sys, view))));
    assert.equal(doc.views[0].kind, "section", name);
    const returnMap = model.isReturnMap(view);
    assert.deepEqual(doc.views[0].y_axis, { kind: returnMap ? "next" : "state", name: sys.states[view.yAxis.index].name }, name);
    const load = stateFromProject(doc);
    let id = 1;
    const applied = model.applyProjectLoad(load, sys, () => id++);
    assert.deepEqual(applied.warnings, [], name);
    assert.deepEqual(applied.view, view, name);
    assert.equal(applied.solver.transient, 10, name);
    const again = projectFromState({ ...documentOf(sys, applied.view), ics: applied.ics }, { loaded: doc });
    assert.equal(JSON.stringify(again), JSON.stringify(doc), name);
  }
  const doc = projectFromState(documentOf(lorenz, cases[0][2]));
  assert.deepEqual(doc.views[0].section, { kind: "plane", state: "z", value: 27, direction: "rising" });
  const planeView = { xAxis: S(0), yAxis: S(2), xRange: [-25, 25], yRange: [-2, 64] };
  const back = projectFromState(documentOf(lorenz, planeView, 0), { loaded: doc });
  assert.equal("kind" in back.views[0], false);
  assert.equal("section" in back.views[0], false);
});

test("a saved section that doesn't fit falls back to the default view with the core's sentence", () => {
  const view = { kind: "section", section: plane(2, 27), xAxis: S(0), yAxis: S(1), xRange: [-20, 20], yRange: [-25, 25] };
  const doc = projectFromState(documentOf(lorenz, view));
  const open = (edit, sys = lorenz) => {
    const d = JSON.parse(JSON.stringify(doc));
    edit(d.views[0]);
    return model.applyProjectLoad(stateFromProject(d), sys, () => 1);
  };
  const unknown = open((v) => (v.section.kind = "hyperbola"));
  assert.deepEqual(unknown.warnings, ["The section of the view “main” is of a kind (“hyperbola”) this version doesn't know, so the default view is used."]);
  assert.equal(unknown.view.kind, undefined);
  assert.deepEqual(open((v) => (v.section.state = "w")).warnings, ["The section of the view “main” holds the state w fixed, which the system doesn't have, so the default view is used."]);
  assert.deepEqual(open((v) => (v.section.direction = "sideways")).warnings, [SECTION_MISFIT]);
  assert.deepEqual(open(() => {}, { ...lorenz, kind: "map" }).warnings, [SECTION_MISFIT]);
});
