// The Parameter plane's rows and inset as words: each special point's
// sentence and coefficients, the resonance ticks' legend, and the live
// inset's request and caption.

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

const curves = await import("../state/curves.ts");
const inset = await import("../state/inset.ts");
const model = await import("../state/model.ts");

const row = (kind, extra = {}) => ({
  kind,
  p: [1, 1],
  x: [0.1, 0.2],
  period: 1,
  on: ["hopf"],
  bracket: null,
  normalForm: null,
  angle: null,
  positiveToward: null,
  ...extra,
});
const square = [1, 1];

test("each special point says what it is, and no more than holds where it was found", () => {
  const say = (kind, extra) => curves.pointSentence(row(kind, extra), square);
  assert.equal(say("bogdanovTakens", { on: ["fold", "hopf"] }), "A fold curve and a Hopf curve meet. A curve of homoclinic orbits starts here.");
  assert.equal(say("cusp", { on: ["fold"] }), "Two fold curves meet. Inside the wedge there are three fixed points, outside one.");
  assert.equal(say("cusp", { on: ["fold"], period: 2 }), "Two fold curves meet. Inside the wedge there are three cycles of period 2, outside one.");
  assert.equal(say("zeroHopf", { on: ["fold", "hopf"] }), "An eigenvalue 0 and a pair ±iω together. The fold and Hopf curves meet here.");
  assert.equal(say("zeroHopf", { on: ["hopf"] }), "An eigenvalue 0 and a pair ±iω together.");
  assert.equal(say("hopfHopf"), "Two pairs ±iω₁ and ±iω₂ together: two Hopf curves cross here.");
  assert.equal(say("resonance1", { on: ["neimarkSacker"] }), "Both multipliers are +1. Past it the Neimark–Sacker curve is a neutral saddle.");
  assert.equal(say("resonance1", { on: ["fold"] }), "Both multipliers are +1.");
  assert.equal(
    say("resonance2", { on: ["flip", "neimarkSacker"] }),
    "Both multipliers are −1. A flip curve crosses here. Past it the Neimark–Sacker curve is a neutral saddle.",
  );
  assert.equal(say("resonance2", { on: ["flip"] }), "Both multipliers are −1.", "F2's 1:2 point on its flip line has no NS curve to promise");
  assert.equal(say("resonance3", { on: ["neimarkSacker"] }), "The multipliers are cube roots of unity. A period-3 cycle replaces the invariant circle.");
  assert.equal(
    say("resonance4", { on: ["neimarkSacker"], period: 2 }),
    "The multipliers are ±i, a 4th root of unity. A period-8 cycle can replace the invariant circle.",
  );
});

test("the fold-flip promises a fold only where a fold curve passes", () => {
  const say = (on) => curves.pointSentence(row("foldFlip", { on }), square);
  const predatorPrey = say(["branchPoint", "flip"]);
  assert.equal(
    predatorPrey,
    "A multiplier +1 and a multiplier −1 together. Here the +1 is a branch point, where another branch of fixed points crosses, so there is no fold.",
  );
  assert.equal(say(["fold", "flip"]), "A multiplier +1 and a multiplier −1 together. A fold curve and a flip curve meet here.");
  assert.equal(say(["flip"]), "A multiplier +1 and a multiplier −1 together.");
  assert.doesNotMatch(say(["flip"]), /fold curve/);
});

test("Bautin, Chenciner and the generalized flip name the side of each kind, as the plot shows it", () => {
  const say = (kind, positiveToward, spans = square) => curves.pointSentence(row(kind, { positiveToward }), spans);
  assert.equal(say("bautin", [0.01, 1]), "The Hopf bifurcation changes from subcritical (above) to supercritical (below).");
  assert.equal(say("bautin", [0.01, -1]), "The Hopf bifurcation changes from supercritical (above) to subcritical (below).");
  assert.equal(say("chenciner", [-1, 0.2]), "The Neimark–Sacker bifurcation changes from supercritical (right) to subcritical (left).");
  assert.equal(say("generalizedFlip", [1, 0]), "The flip changes from supercritical (right) to subcritical (left).");
  assert.equal(say("bautin", [1, 0.5], [10, 1]), "The Hopf bifurcation changes from subcritical (above) to supercritical (below).", "up spans 1, across 10");
  assert.equal(say("bautin", null), "The Hopf bifurcation changes between supercritical and subcritical here.");
});

test("coefficients read as the row shows them: a and b with the sign of ab, the cusp's c, and θ in degrees", () => {
  assert.equal(curves.normalFormText({ kind: "bogdanovTakens", a: 0.04822563, b: 1.937632 }), "a = 0.0482, b = 1.9376 · ab > 0");
  assert.equal(curves.normalFormText({ kind: "bogdanovTakens", a: 0.0838, b: -2.13628 }), "a = 0.0838, b = −2.1363 · ab < 0");
  assert.equal(curves.normalFormText({ kind: "cusp", c: 0.3627844 }), "c = 0.3628");
  assert.equal(curves.normalFormText(null), null);
  assert.equal(curves.angleText(Math.PI / 2), "θ = 90°");
  assert.equal(curves.angleText((2 * Math.PI) / 3), "θ = 120°");
  assert.equal(curves.angleText(0.6248), "θ = 35.8°");
});

test("a point found on two curves keeps the coefficients and angle of whichever has them", () => {
  const sp = (extra) => ({ kind: "resonance2", index: 3, p: [14, 1.346154], x: [0.46, 3.52], period: 1, bracket: 1e-12, normalForm: null, angle: null, positiveToward: null, ...extra });
  const curve = (kind, special) => ({ kind, period: 1, p1: [14], p2: [1.346154], x: [0.46, 3.52], neutral: [false], special, end: { first: null, last: "range" }, ticks: [] });
  const rows = curves.planePointRows(
    [curve("flip", [sp({})]), curve("neimarkSacker", [sp({ angle: Math.PI })])],
    [
      [0, 16],
      [-6, 3],
    ],
  );
  assert.equal(rows.length, 1);
  assert.deepEqual(rows[0].on, ["flip", "neimarkSacker"]);
  assert.equal(rows[0].angle, Math.PI);
});

test("the ticks' legend line names the range of q drawn", () => {
  assert.equal(curves.ticksText([5, 6, 7, 8, 9]), "Ticks 1:5 to 1:9 mark where the rotation is a rational turn: each is the tip of a resonance tongue.");
  assert.equal(curves.ticksText([7]), "The tick 1:7 marks where the rotation is a rational turn: each is the tip of a resonance tongue.");
  assert.equal(curves.ticksText([]), "");
});

test("the inset's caption counts the fixed points in view by kind", () => {
  const fp = (classification, stable) => ({ classification, stable });
  assert.equal(
    inset.fixedPointsText([fp("stable spiral", true), fp("saddle (n-D)", false), fp("stable node", true)]),
    "3 fixed points in view: 2 stable, 1 saddle. Where you start decides which stable state you reach.",
  );
  assert.equal(inset.fixedPointsText([fp("unstable spiral", false)]), "One fixed point in view, an unstable spiral.");
  assert.equal(inset.fixedPointsText([fp("saddle", false), fp("center", false)]), "2 fixed points in view: 1 saddle, 1 non-hyperbolic.");
  assert.equal(inset.fixedPointsText([]), "No fixed point in view.");
});

test("the inset says how a map's orbits end: a cycle's period, no repeat, or a blow-up", () => {
  const orbit = (f, n = 200) => ({ x: Array.from({ length: n }, (_, k) => f(k)).flat(), status: "completed" });
  const meta = { dim: 2, kept: 3000, view: { xRange: [0, 1], yRange: [0, 1] } };
  const period4 = orbit((k) => [[0.1, 0.2, 0.3, 0.4][k % 4], 0.5]);
  const circle = orbit((k) => [Math.cos(k * 0.7), Math.sin(k * 0.7)]);
  assert.equal(inset.orbitFate(period4, 2, 1), 4);
  assert.equal(inset.orbitFate(orbit(() => [0.3, 0.3]), 2, 1), 1);
  assert.equal(inset.orbitFate(circle, 2, 1), "wanders");
  assert.equal(inset.orbitFate({ ...circle, status: "blewUp" }, 2, 1), "escaped");
  assert.equal(inset.orbitsText({ trajectories: [period4] }, meta), "The orbit settles on a period-4 cycle.");
  assert.equal(inset.orbitsText({ trajectories: [circle, circle] }, meta), "The orbits do not repeat within 3,000 iterates.");
  assert.equal(inset.orbitsText({ trajectories: [circle, period4] }, meta), "The orbits end differently: where you start decides.");
});

const sys = (kind, states, extra = {}) => ({
  ok: true,
  systemId: 3,
  kind,
  hamiltonian: false,
  separable: false,
  autonomous: true,
  states: states.map((name) => ({ name, latex: name, derived: false })),
  params: [
    { name: "a", latex: "a", default: 1 },
    { name: "b", latex: "b", default: 1 },
  ],
  equationsLatex: [],
  diagnostics: [],
  ...extra,
});
const phase = { xAxis: { kind: "state", index: 2 }, yAxis: { kind: "state", index: 1 }, xRange: [0.25, 0.55], yRange: [0.1, 0.6] };
const snapshot = (compiled, extra = {}) => ({
  compiled,
  params: [
    { name: "a", latex: "a", value: 1.18, min: 0.5, max: 2.5 },
    { name: "b", latex: "b", value: 0.6, min: 0, max: 2 },
  ],
  view: { kind: "curves" },
  baseState: [0, 0, 0],
  fieldTime: 0,
  ics: [],
  layers: { ...model.DEFAULT_LAYERS, field: true, nullclines: true, backward: true },
  solver: { ...model.DEFAULT_SOLVER, tEnd: 400 },
  plotAspect: 2,
  shapeId: 1,
  ...extra,
});

test("the inset asks for the Phase view's states and range at the crosshair, with fixed points and no field", () => {
  const flow = sys("ode", ["x", "y", "s"]);
  const job = inset.buildInsetJob(snapshot(flow), phase);
  assert.deepEqual(job.body.params, [1.18, 0.6]);
  assert.deepEqual([job.body.xAxis, job.body.yAxis], [phase.xAxis, phase.yAxis]);
  assert.equal(job.body.fixedPoints, true);
  assert.equal(job.body.fieldGrid, 0);
  assert.equal(job.body.nullclines, false);
  assert.equal(job.body.trajectories.length, 5, "no start points: five spread over the view, forward only");
  assert.ok(job.body.trajectories.every((t) => t.t1 > t.t0));
  assert.deepEqual(job.body.trajectories[0].x0, [0, 0.35, 0.4], "the centre of the view, the hidden state at the slice");
  assert.deepEqual(job.meta.colors, [0, 1, 2, 3, 4]);

  const own = inset.buildInsetJob(snapshot(flow, { ics: [{ id: 9, x0: [0.02, 0.5, 0.33], t0: 0, color: 3 }] }), phase);
  assert.equal(own.body.trajectories.length, 1, "the Phase view's own start point, forward only");
  assert.deepEqual(own.meta.colors, [3]);
});

test("a map's inset hides at least a thousand iterates first", () => {
  const map = sys("map", ["x", "y"]);
  const view = { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [0, 1], yRange: [-0.1, 1.5] };
  const ics = [{ id: 1, x0: [0.4, 1], t0: 0, color: 0 }];
  const job = inset.buildInsetJob(snapshot(map, { ics, baseState: [0, 0], solver: { ...model.DEFAULT_SOLVER, tEnd: 3000, transient: 0 } }), view);
  assert.equal(job.body.transient, 1000);
  assert.equal(job.meta.kept, 3000);
  const longer = inset.buildInsetJob(snapshot(map, { ics, baseState: [0, 0], solver: { ...model.DEFAULT_SOLVER, tEnd: 3000, transient: 2500 } }), view);
  assert.equal(longer.body.transient, 2500);
});

test("the inset draws the Phase view last shown, else the default phase plane, and nothing for one state", () => {
  const flow = sys("ode", ["x", "y", "s"]);
  assert.equal(inset.insetView(phase, flow), phase);
  const series = { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 10], yRange: [0, 1] };
  assert.deepEqual(inset.insetView(series, flow), { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [-5, 5], yRange: [-5, 5] });
  assert.equal(inset.insetView(undefined, sys("ode", ["x"])), null);
  assert.equal(inset.insetParamsText(["k₂", "K"], [1.18, -0.6]), "k₂ = 1.180, K = −0.600");
});
