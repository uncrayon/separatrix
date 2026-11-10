// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports (a file or a folder's index.ts) and its JSON imports
// resolve through module hooks.

import test from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";

registerHooks({
  resolve(specifier, context, next) {
    try {
      return next(specifier, context);
    } catch (err) {
      if (!/^\.\.?\//.test(specifier) || /\.[cm]?[jt]sx?$|\.json$/.test(specifier)) throw err;
      try {
        return next(`${specifier}.ts`, context);
      } catch {
        return next(`${specifier}/index.ts`, context);
      }
    }
  },
  load(url, context, next) {
    if (url.endsWith(".json")) return next(url, { ...context, importAttributes: { type: "json" } });
    return next(url, context);
  },
});

const model = await import("../state/model.ts");
const curves = await import("../state/curves.ts");
const { projectFromState } = await import("../project/serialize.ts");
const { mockCompile, mockContinuation, mockCurve } = await import("../api/mock.ts");
const { TEMPLATES } = await import("../templates/index.ts");
const { refusalText } = await import("../math/format.ts");

const st = (name) => ({ name, latex: name, derived: false });
const system = (kind, states, params, extra = {}) => ({
  ok: true,
  systemId: 7,
  kind,
  hamiltonian: false,
  separable: false,
  autonomous: true,
  states: states.map(st),
  params: params.map((name) => ({ name, latex: name, default: 1 })),
  equationsLatex: [],
  diagnostics: [],
  ...extra,
});
const co = system("ode", ["x", "y", "s"], ["k_2", "K"]);
const logistic = system("map", ["x"], ["r"]);
const sliders = (values, ranges) =>
  values.map((value, i) => ({ name: ["k_2", "K"][i], latex: ["k_2", "K"][i], value, min: ranges[i][0], max: ranges[i][1] }));
const CO_SLIDERS = sliders([1.05, 0.4], [
  [0.5, 2.5],
  [0, 2],
]);

const hopf = { kind: "hopf", index: 40, p: 1.051557, x: [0.0164, 0.524, 0.3283], period: 1, tangent: [0, 0, 0, 1], eigenvalues: [], omega: 0.05074, lyapunov: null };
const fold = { ...hopf, kind: "fold", p: 1.042049, x: [0.0247, 0.4503, 0.375], omega: null };

test("the Parameter plane is the last choice, offered with two parameters and no t or n", () => {
  assert.equal(model.viewChoicesFor(co).at(-1), "curves");
  assert.equal(model.VIEW_CHOICE_LABELS.curves, "Parameter plane");
  assert.equal(model.VIEW_CHOICE_SHORT.curves, "Parameter plane");
  assert.deepEqual(model.viewChoiceAvailability(co).curves, { ok: true });
  assert.deepEqual(model.viewChoiceAvailability(logistic, "Logistic map").curves, {
    ok: false,
    reason: "The parameter plane needs two parameters; Logistic map has 1.",
  });
  assert.deepEqual(curves.curvesAvailability({ ...co, autonomous: false }, "Forced CO"), {
    ok: false,
    reason: "The parameter plane follows bifurcations of fixed points; Forced CO changes with $t$, so it has none.",
  });
});

test("one job per seed: the free pair from the seed, the other sliders from now, keyed by the body", () => {
  const seed = curves.planeSeed(hopf, [1.05, 0.4], 0);
  assert.deepEqual(seed, { kind: "hopf", x: [0.0164, 0.524, 0.3283], period: 1, params: [1.051557, 0.4] });
  const base = curves.curvesView(CO_SLIDERS, 0, 1, null, model.defaultView(co));
  const view = { ...base, curves: { ...base.curves, seeds: [seed] } };
  const [job] = curves.buildCurveJobs({ compiled: co, params: CO_SLIDERS, view });
  assert.deepEqual(job.body, {
    systemId: 7,
    params: [1.051557, 0.4],
    free: [0, 1],
    ranges: [
      [0.5, 2.5],
      [0, 2],
    ],
    seed: { kind: "hopf", x: [0.0164, 0.524, 0.3283], period: 1 },
    direction: "both",
    ds: 0.01,
    dsMax: 0.02,
    maxPoints: 2000,
  });
  assert.equal(job.meta.key, JSON.stringify(job.body));
  const moved = curves.buildCurveJobs({ compiled: co, params: sliders([1.3, 0.9], [[0.5, 2.5], [0, 2]]), view });
  assert.equal(moved[0].meta.key, job.meta.key, "clicking the plane moves both sliders and asks for nothing");
  assert.deepEqual(curves.buildCurveJobs({ compiled: logistic, params: CO_SLIDERS, view }), [], "one parameter: no plane");
});

test("a branch point is followed too: its row gives a branch-point seed, and opening the plane searches from it", () => {
  assert.equal(curves.curveKindOf("branchPoint"), "branchPoint");
  const bp = { kind: "branchPoint", index: 12, p: 1.666667, x: [0.4, 0], period: 1, tangent: [0, 0, 1], eigenvalues: [[1, 0], [0.333333, 0]], omega: null, lyapunov: null };
  const pd = { ...bp, kind: "flip", index: 30, p: 3, x: [0.666667, 0], eigenvalues: [[-1, 0], [1.4, 0]] };
  assert.deepEqual(curves.planeSeed(bp, [5.3, 1], 0), { kind: "branchPoint", x: [0.4, 0], period: 1, params: [1.666667, 1] });
  const pp = system("map", ["x", "y"], ["a", "varepsilon"]);
  const params = [
    { name: "a", latex: "a", value: 5.3, min: 0, max: 16 },
    { name: "varepsilon", latex: "\\varepsilon", value: 1, min: -6, max: 3 },
  ];
  const branchView = model.continuationView(params, [0, 16], 0, [model.pointSeed([0.8113, 0], 5.3, "both")]);
  const [job] = model.buildContinuationJobs({ compiled: pp, params, view: branchView });
  const res = {
    requestId: 1,
    elapsedMs: 1,
    branch: { period: 1, p: [0, 16], x: [], stable: [], unstable: [], special: [pd, bp], end: { first: "range", last: "range" } },
  };
  const s = { compiled: pp, view: { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [0, 1], yRange: [0, 1] }, solver: model.DEFAULT_SOLVER, params, branches: { [job.meta.key]: { ok: true, res, roundTripMs: 1 } } };
  const plane = model.switchView("curves", s, { continuation: branchView });
  assert.deepEqual(plane.curves.seeds, [
    { kind: "branchPoint", x: [0.4, 0], period: 1, params: [1.666667, 1] },
    { kind: "flip", x: [0.666667, 0], period: 1, params: [3, 1] },
  ]);
});

test("opening the plane searches from the Branch view's special points, across its parameter, up the next", () => {
  const branchView = model.continuationView(CO_SLIDERS, [0, 0.12], 0, [model.pointSeed([0.0646, 0.2683, 0.4765], 1.05, "both")]);
  const [job] = model.buildContinuationJobs({ compiled: co, params: CO_SLIDERS, view: branchView });
  const res = {
    requestId: 1,
    elapsedMs: 1,
    branch: { period: 1, p: [1, 1.1], x: [], stable: [], unstable: [], special: [hopf, fold], end: { first: "range", last: "range" } },
  };
  const memory = { continuation: branchView };
  const s = { compiled: co, view: { xAxis: { kind: "state", index: 1 }, yAxis: { kind: "state", index: 2 }, xRange: [0, 1], yRange: [0, 1] }, solver: model.DEFAULT_SOLVER, params: CO_SLIDERS, branches: { [job.meta.key]: { ok: true, res, roundTripMs: 1 } } };
  const plane = model.switchView("curves", s, memory);
  assert.equal(plane.kind, "curves");
  assert.deepEqual([plane.across, plane.up], [0, 1]);
  assert.deepEqual(plane.curves.seeds.map((x) => [x.kind, x.params]), [
    ["fold", [1.042049, 0.4]],
    ["hopf", [1.051557, 0.4]],
  ]);
  assert.equal(plane.branch, branchView);
  assert.deepEqual(plane.branchParams, [1.05, 0.4], "the slice is drawn at the K the branches were computed at");
  assert.equal(plane.home, branchView, "a project file saves the Branch view");
  const movedK = CO_SLIDERS.map((p, i) => (i === 1 ? { ...p, value: 0.7 } : p));
  const away = model.switchView("phase", { ...s, view: plane, params: movedK }, memory);
  const back = model.switchView("curves", { ...s, view: away, params: movedK }, memory);
  assert.deepEqual(back.branchParams, [1.05, 0.4], "moving K in the plane does not move the slice");
  const empty = model.switchView("curves", { ...s, branches: {} }, {});
  assert.deepEqual(empty.curves.seeds, [], "nothing to search from: the empty state");
  assert.equal(empty.branch, null);
});

const curveOut = (kind, p1, p2, extra = {}) => ({
  kind,
  period: 1,
  p1,
  p2,
  x: p1.flatMap(() => [0, 0, 0]),
  neutral: p1.map(() => false),
  special: [],
  end: { first: "range", last: "range" },
  ...extra,
});
const RANGES = [
  [0.5, 2.5],
  [0, 2],
];
const job = (kind, a, b, seed = 0) => ({
  body: { systemId: 7, params: [a, b], free: [0, 1], ranges: RANGES, seed: { kind, x: [0, 0, 0], period: 1 }, direction: "both", ds: 0.01, dsMax: 0.02, maxPoints: 2000 },
  meta: { key: `${kind}-${a}-${b}`, seed },
});
const run = (curve) => ({ ok: true, res: { requestId: 1, curve, elapsedMs: 2 }, roundTripMs: 3 });

test("a curve that repeats one already drawn is merged into it; the next seed is the one computing", () => {
  const hopfCurve = curveOut("hopf", [0.9, 1.0, 1.1, 1.2], [0.2, 0.35, 0.45, 0.7]);
  const rows = curves.curveRows(
    [
      { job: job("hopf", 1.0, 0.35, 0), run: run(hopfCurve) },
      { job: job("fold", 1.04, 0.4, 1), run: run(curveOut("fold", [1.0, 1.04, 1.2], [0.36, 0.4, 0.72])) },
      { job: job("hopf", 1.15, 0.575, 2), run: run(hopfCurve) },
      { job: job("fold", 1.05, 0.4, 3), run: { ok: false, error: "The preview has no curve through this point." } },
      { job: job("hopf", 2, 2, 4), run: undefined },
      { job: job("hopf", 2.1, 2, 5), run: undefined },
    ],
    RANGES,
  );
  assert.deepEqual(rows.map((r) => r.status), ["done", "done", "merged", "failed", "computing"]);
  assert.equal(rows[2].into, 0);
  assert.equal(curves.mergedText(rows[2].into), "Same curve as the first: merged");
  assert.deepEqual(curves.curvesSummary(rows, RANGES), { elapsedMs: 4, roundTripMs: 6, points: 7, curves: 2, special: 0 });
});

test("a point found on two curves is one row, with both curve kinds; rows by kind, then across", () => {
  const bt1 = { kind: "bogdanovTakens", index: 3, p: [1.16119871, 0.72233907], x: [0.0163, 0.6384, 0.2005], period: 1 };
  const bt2 = { ...bt1, p: [1.41762764, 0.97139757] };
  const cp = { kind: "cusp", index: 1, p: [1.00640833, 0.35599127], x: [0.0359, 0.352, 0.4514], period: 1 };
  const gh = { kind: "bautin", index: 0, p: [0.92425502, 0.30587927], x: [0.0643, 0.2111, 0.5549], period: 1 };
  const rows = curves.planePointRows(
    [curveOut("hopf", [1], [1], { special: [bt2, gh, bt1] }), curveOut("fold", [1], [1], { special: [bt1, cp, bt2] })],
    RANGES,
  );
  assert.deepEqual(
    rows.map((r) => [curves.CODIM2_KINDS[r.kind].label, r.p[0], curves.onCurvesText(r.on)]),
    [
      ["BT", 1.16119871, "On the fold and Hopf curves"],
      ["BT", 1.41762764, "On the fold and Hopf curves"],
      ["CP", 1.00640833, "On the fold curve"],
      ["GH", 0.92425502, "On the Hopf curve"],
    ],
  );
});

test("the fold-flip found on a flip curve and on a branch-point curve is one row, on both, in kind order", () => {
  const lppd = (index, bracket) => ({ kind: "foldFlip", index, p: [3, 2], x: [0.666667, 0], period: 1, bracket });
  const rows = curves.planePointRows(
    [curveOut("flip", [3], [2], { special: [lppd(40, 1.5e-13)] }), curveOut("branchPoint", [3], [2], { special: [lppd(170, 7.3e-14)] })],
    [
      [0, 16],
      [-6, 3],
    ],
  );
  assert.deepEqual(rows, [{ kind: "foldFlip", p: [3, 2], x: [0.666667, 0], period: 1, on: ["branchPoint", "flip"], bracket: 1.5e-13 }]);
  assert.equal(curves.onCurvesText(rows[0].on), "On the branch-point and flip curves");
  assert.equal(curves.locatedText(rows[0].bracket), "located to 1.5 × 10⁻¹³");
});

test("a point the preview found without a bisection says nothing about how finely it was located", () => {
  const r2 = { kind: "resonance2", index: 9, p: [14, 35 / 26], x: [0.464286, 3.520833], period: 1, bracket: null };
  const [row] = curves.planePointRows([curveOut("neimarkSacker", [14], [35 / 26], { special: [r2] })], [
    [0, 16],
    [-6, 3],
  ]);
  assert.equal(row.bracket, null);
  assert.equal(curves.locatedText(row.bracket), null);
  assert.equal(curves.locatedText(2.9e-13), "located to 2.9 × 10⁻¹³");
});

test("a row says where each end stopped, a closed curve and an unfolded branch point included", () => {
  assert.equal(model.endText({ first: "range", last: "range" }), "stopped at the range's end");
  assert.equal(model.endText({ first: null, last: "maxPoints" }), "stopped at Max points");
  assert.equal(model.endText({ first: "stepTooSmall", last: "range" }), "stopped at a step too small to go on and at the range's end");
  assert.equal(model.endText({ first: null, last: "closed" }), "stopped back at its start");
  assert.equal(
    model.endText({ first: "range", last: "unfolded" }),
    "stopped at the range's end and where the parameters no longer keep the branch point",
  );
});

test("a stalled curve says where, why, and what to try, for a flow and for a map", () => {
  const stalled = curveOut("fold", [1.2, 1.0064], [0.7, 0.356], { end: { first: "range", last: "stepTooSmall" } });
  assert.equal(
    curves.stallText(stalled, ["k₂", "K"], "ode"),
    "The step became too small near k₂ = 1.0064, K = 0.3560. A cusp or Bogdanov–Takens point may be there. Lower Δs.",
  );
  assert.equal(
    curves.stallText({ ...stalled, p1: [3, 3], p2: [-1.499, 1], end: { first: "stepTooSmall", last: "range" } }, ["a", "ε"], "map"),
    "The step became too small near a = 3.0000, ε = −1.4990. The map may be singular there, or a special point may be near. Lower Δs.",
  );
  assert.equal(curves.stallText(curveOut("fold", [1, 2], [0, 1]), ["a", "b"], "ode"), null);
  assert.equal(curves.retryStep(0.01), 0.005);
});

test("a neutral stretch is named by the special point it starts at, else where it is dotted", () => {
  const n = [true, true, false, false, false, true, true];
  const bt = (index) => ({ kind: "bogdanovTakens", index, p: [0, 0], x: [], period: 1 });
  assert.equal(curves.neutralText({ neutral: n, special: [bt(2), bt(5)] }), "neutral past the BT points");
  assert.equal(curves.neutralText({ neutral: [false, false, true, true], special: [{ ...bt(2), kind: "resonance2" }] }), "neutral past R2");
  assert.equal(curves.neutralText({ neutral: n, special: [bt(2)] }), "neutral where dotted");
  assert.equal(curves.neutralText({ neutral: [false, false], special: [] }), "");
});

test("saving while the plane is shown saves the Branch view it came from", () => {
  const branchView = model.continuationView(CO_SLIDERS, [0, 0.12], 0, []);
  const plane = curves.openPlane(null, branchView, branchView, CO_SLIDERS, []);
  const doc = (view) => ({
    format: "latex",
    equations: [{ id: 1, source: "\\dot{x}=y" }],
    compiled: co,
    params: CO_SLIDERS,
    view,
    baseState: [0, 0, 0],
    fieldTime: 0,
    ics: [],
    layers: model.DEFAULT_LAYERS,
    solver: model.DEFAULT_SOLVER,
    sourceTemplateId: "co-oxidation",
  });
  assert.deepEqual(projectFromState(doc(plane)).views, projectFromState(doc(branchView)).views);
  assert.equal(projectFromState(doc(plane)).views[0].kind, "continuation");
});

// The preview's curves pass through the reference points of docs/codim2-reference.md.
async function compiled(id) {
  const t = TEMPLATES.find((x) => x.id === id);
  return mockCompile({ source: t.equations.join("\n"), format: "latex" });
}
function distance(curve, [a, b], [[lo1, hi1], [lo2, hi2]]) {
  const w1 = hi1 - lo1;
  const w2 = hi2 - lo2;
  let best = Infinity;
  for (let i = 0; i + 1 < curve.p1.length; i++) {
    const [x0, y0] = [(curve.p1[i] - a) / w1, (curve.p2[i] - b) / w2];
    const [dx, dy] = [(curve.p1[i + 1] - curve.p1[i]) / w1, (curve.p2[i + 1] - curve.p2[i]) / w2];
    const len2 = dx * dx + dy * dy;
    const t = len2 > 0 ? Math.min(1, Math.max(0, -(x0 * dx + y0 * dy) / len2)) : 0;
    best = Math.min(best, Math.hypot(x0 + t * dx, y0 + t * dy));
  }
  return best;
}
const curveRequest = (systemId, params, ranges, seed) => ({
  requestId: 1,
  systemId,
  params,
  free: [0, 1],
  ranges,
  seed,
  direction: "both",
  ds: 0.01,
  dsMax: 0.02,
  maxPoints: 2000,
});

test("CO oxidation: the Hopf curve from K = 0.4 passes both BT and both GH points; the fold curve the cusp and both BT", async () => {
  const sys = await compiled("co-oxidation");
  const ranges = [
    [0.5, 2.5],
    [0, 2],
  ];
  const { curve: h } = await mockCurve(
    curveRequest(sys.systemId, [1.051557, 0.4], ranges, { kind: "hopf", x: [0.0164, 0.524, 0.3283], period: 1 }),
  );
  for (const p of [
    [1.16119871, 0.72233907],
    [1.41762764, 0.97139757],
    [0.89131898, 0.23248735],
    [0.92425502, 0.30587927],
  ])
    assert.ok(distance(h, p, ranges) < 1e-4, `Hopf curve through ${p}: ${distance(h, p, ranges)}`);
  assert.deepEqual(h.special.map((s) => s.kind).sort(), ["bautin", "bautin", "bogdanovTakens", "bogdanovTakens"]);
  const { curve: f } = await mockCurve(
    curveRequest(sys.systemId, [1.042049, 0.4], ranges, { kind: "fold", x: [0.0247, 0.4503, 0.375], period: 1 }),
  );
  for (const p of [
    [1.00640833, 0.35599127],
    [1.16119871, 0.72233907],
    [1.41762764, 0.97139757],
  ])
    assert.ok(distance(f, p, ranges) < 1e-4, `fold curve through ${p}: ${distance(f, p, ranges)}`);
});

test("the predator–prey map: the NS curve from a = 5 passes R4, R3, R2 and CH", async () => {
  const sys = await compiled("predator-prey-map");
  const ranges = [
    [0, 16],
    [-6, 3],
  ];
  const { curve } = await mockCurve(
    curveRequest(sys.systemId, [5, 1], ranges, { kind: "neimarkSacker", x: [0.4, 0.9333], period: 1 }),
  );
  for (const p of [
    [7, 7 / 6],
    [10.5, 49 / 38],
    [14, 35 / 26],
    [1.3257299842, -4.6400549446],
  ])
    assert.ok(distance(curve, p, ranges) < 1e-4, `NS curve through ${p}: ${distance(curve, p, ranges)}`);
  assert.deepEqual(curve.special.map((s) => curves.CODIM2_KINDS[s.kind].label).sort(), ["CH", "R2", "R3", "R4"]);
  assert.deepEqual(curve.special.map((s) => s.bracket), [null, null, null, null]);
});

test("the predator–prey map: F2's flip curve from a = 9/11 passes the generalized flip, and its other one the 1:2 resonance at a = 3", async () => {
  const sys = await compiled("predator-prey-map");
  const ranges = [
    [0, 16],
    [-6, 3],
  ];
  const label = (c) => c.special.map((s) => [curves.CODIM2_KINDS[s.kind].label, s.p, s.x]);
  const { curve: upper } = await mockCurve(
    curveRequest(sys.systemId, [9 / 11, 1], ranges, { kind: "flip", x: [-2 / 9, 0], period: 1 }),
  );
  assert.deepEqual(label(upper), [["GPD", [0.64644661, -1.6715729], [-0.54691816, 0]]]);
  const { curve: lower } = await mockCurve(curveRequest(sys.systemId, [2, -5.5], ranges, { kind: "flip", x: [0.5, 0], period: 1 }));
  assert.deepEqual(label(lower), [["R2", [3, -5], [0.66666667, 0]]]);
});

test("a refusal names a parameter as the lists do, from the preview and from the Rust core alike", async () => {
  const sys = await compiled("co-oxidation");
  const req = curveRequest(sys.systemId, [1.042049, 0.4], [[0.5, 2.5], [0, 2]], { kind: "fold", x: [0.0247, 0.4503, 0.375], period: 1 });
  const refused = await mockCurve({ ...req, free: [1, 0] }).then(
    () => null,
    (err) => err.message,
  );
  assert.equal(refusalText(refused), "This browser preview follows this system's curves with k₂ across and K up only.");
  // compute_curve's NoEffect sentence for the predator–prey map's flip at a = 3 (app/src-tauri/tests/curves.rs).
  assert.equal(
    refusalText("\\varepsilon doesn't move this flip; choose another parameter to follow it in."),
    "ε doesn't move this flip; choose another parameter to follow it in.",
  );
});

test("the predator–prey map: the branch-point curve from a = 5/3 runs from ε = −6 to a = 16 through the fold-flip", async () => {
  const sys = await compiled("predator-prey-map");
  const ranges = [
    [0, 16],
    [-6, 3],
  ];
  const { curve } = await mockCurve(curveRequest(sys.systemId, [5 / 3, 1], ranges, { kind: "branchPoint", x: [0.4, 0], period: 1 }));
  assert.equal(curve.kind, "branchPoint");
  assert.ok(distance(curve, [3, 2], ranges) < 1e-4, `BP curve through the LPPD: ${distance(curve, [3, 2], ranges)}`);
  assert.deepEqual(curve.special.map((s) => [curves.CODIM2_KINDS[s.kind].label, s.p, s.x]), [["LPPD", [3, 2], [0.66666667, 0]]]);
  const ends = [0, curve.p1.length - 1].map((i) => [Number(curve.p1[i].toFixed(4)), Number(curve.p2[i].toFixed(4))]);
  assert.deepEqual(ends, [
    [1.1176, -6],
    [16, 2.4333],
  ], "from (19/17, −6) to (16, 73/30)");
  assert.deepEqual(curve.end, { first: "range", last: "range" });
  assert.equal(curve.neutral.includes(true), false);
});

test("the mock follower finds the predator–prey map's BP at a = 5/3 and NS at a = 5 on F3", async () => {
  const sys = await compiled("predator-prey-map");
  const res = await mockContinuation({
    requestId: 1,
    systemId: sys.systemId,
    params: [5.3, 1],
    param: 0,
    range: [0, 16],
    start: { kind: "point", x0: [0.4, 1.0173333], period: 1 },
    direction: "both",
    ds: 0.16,
    dsMax: 0.32,
    maxPoints: 500,
  });
  assert.deepEqual(
    res.branch.special.map((s) => [s.kind, Number(s.p.toFixed(5)), s.x.map((v) => Number(v.toFixed(4)))]),
    [
      ["branchPoint", 1.66667, [0.4, 0]],
      ["neimarkSacker", 5, [0.4, 0.9333]],
    ],
  );
});

test("the mock follower finds CO oxidation's H, LP, LP, H in k₂ at the template's defaults", async () => {
  const sys = await compiled("co-oxidation");
  const res = await mockContinuation({
    requestId: 1,
    systemId: sys.systemId,
    params: [1.05, 0.4],
    param: 0,
    range: [0.5, 2.5],
    start: { kind: "point", x0: [0.0646, 0.2683, 0.4765], period: 1 },
    direction: "both",
    ds: 0.02,
    dsMax: 0.04,
    maxPoints: 500,
  });
  const found = res.branch.special.map((s) => [s.kind, Number(s.p.toFixed(5))]).sort((a, b) => a[1] - b[1]);
  assert.deepEqual(found, [
    ["hopf", 1.04099],
    ["fold", 1.04205],
    ["hopf", 1.05156],
    ["fold", 1.0522],
  ]);
});
