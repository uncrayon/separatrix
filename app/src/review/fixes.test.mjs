// Node (>= 22.18) strips the types from the .ts modules.

import test from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_LAYERS, DEFAULT_SOLVER, stateExtent, trajectorySpans } from "../state/model.ts";
import { firstOrderDisplay } from "../math/format.ts";

const ic = { id: 1, x0: [0.5], t0: 0, color: 0 };
const tx = { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [-2, 6], yRange: [-3, 3] };

test("time on an axis: backward on covers the whole visible window", () => {
  const spans = trajectorySpans("ode", tx, ic, DEFAULT_SOLVER, { ...DEFAULT_LAYERS, backward: true });
  assert.deepEqual(
    spans.map((s) => [s.direction, s.req.t0, s.req.t1]),
    [
      ["forward", 0, 6],
      ["backward", 0, -2],
    ],
  );
});

test("time on the vertical axis: backward off still drops the backward run", () => {
  const ty = { ...tx, xAxis: tx.yAxis, yAxis: tx.xAxis, xRange: tx.yRange, yRange: tx.xRange };
  const spans = trajectorySpans("ode", ty, ic, DEFAULT_SOLVER, { ...DEFAULT_LAYERS, backward: false });
  assert.deepEqual(
    spans.map((s) => s.direction),
    ["forward"],
  );
});

test("time on an axis: a start point right of the window gets only a backward run (when on)", () => {
  const late = { ...ic, t0: 10 };
  const on = trajectorySpans("ode", tx, late, DEFAULT_SOLVER, { ...DEFAULT_LAYERS, backward: true });
  assert.deepEqual(
    on.map((s) => [s.direction, s.req.t1]),
    [["backward", -2]],
  );
  const off = trajectorySpans("ode", tx, late, DEFAULT_SOLVER, { ...DEFAULT_LAYERS, backward: false });
  assert.equal(off.length, 0);
});

test("phase plane: the time span T sets both directions", () => {
  const plane = { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [-1, 1], yRange: [-1, 1] };
  const two = { ...ic, x0: [0, 1] };
  const spans = trajectorySpans("ode", plane, two, { ...DEFAULT_SOLVER, tEnd: 7 }, { ...DEFAULT_LAYERS, backward: true });
  assert.deepEqual(
    spans.map((s) => [s.direction, s.req.t1]),
    [
      ["forward", 7],
      ["backward", -7],
    ],
  );
});

function response(...runs) {
  return {
    requestId: 1,
    arrows: [],
    nullclines: [],
    fixedPoints: [],
    elapsedMs: 0,
    trajectories: runs.map(({ status, x }) => ({
      t: x.map((_, k) => k),
      x: x.flat(),
      status,
      stats: { accepted: 0, rejected: 0, evals: 0 },
    })),
  };
}

test("stateExtent: only abandoned runs gives no extent (callers fall back to a default)", () => {
  const res = response({ status: "maxSteps", x: [[0, 0], [5e5, 1]] }, { status: "blewUp", x: [[1, 1], [2e12, 3]] });
  assert.equal(stateExtent(res, 2, 0), null);
});

test("stateExtent: completed runs set the range, non-finite samples are ignored", () => {
  const res = response({ status: "completed", x: [[-1, 0], [NaN, 0], [3, 0]] });
  assert.deepEqual(stateExtent(res, 2, 0), [-1, 3]);
});

const pendulumStates = [
  { name: "theta", latex: "\\theta", derived: false },
  { name: "theta'", latex: "\\dot{\\theta}", derived: true },
];

test("first-order form of a 2nd-order equation does not read as θ̇ = θ̇", () => {
  const out = firstOrderDisplay(
    ["\\dot{\\theta} = \\dot{\\theta}", "\\ddot{\\theta} = -\\sin\\theta"],
    pendulumStates,
    "ode",
  );
  assert.deepEqual(out, ["\\frac{d\\theta}{dt} = \\dot{\\theta}", "\\frac{d\\dot{\\theta}}{dt} = -\\sin\\theta"]);
});

test("first-order form is unchanged for systems without derived states, and for maps", () => {
  const xy = [
    { name: "x", latex: "x", derived: false },
    { name: "y", latex: "y", derived: false },
  ];
  const eqs = ["\\dot{x} = y", "\\dot{y} = -x"];
  assert.deepEqual(firstOrderDisplay(eqs, xy, "ode"), eqs);
  const map = ["x_{n+1} = y_{n}", "y_{n+1} = x_{n}"];
  assert.deepEqual(firstOrderDisplay(map, xy, "map"), map);
});

test("user-entered small values are shown, computed noise snaps to zero", async () => {
  const { fmt, COMPUTED_ZERO } = await import("../math/format.ts");
  assert.equal(fmt(9.109e-31), "9.11e-31");
  assert.equal(fmt(1e-14, 2), "1e-14");
  assert.equal(fmt(3e-17, 4, COMPUTED_ZERO), "0");
  assert.equal(fmt(0), "0");
});

test("an index joins a state's own subscript instead of doubling it (x_{1}_0 is drawn x₁₀)", async () => {
  const { withSubscript, axisLabelLatex } = await import("../math/format.ts");
  assert.equal(withSubscript("x", "0"), "x_{0}");
  assert.equal(withSubscript("x_{1}", "0"), "x_{1,0}");
  assert.equal(withSubscript("\\dot{x}_{1}", "0"), "\\dot{x}_{1,0}");
  assert.equal(withSubscript("k_{\\mathrm{on}}", "n"), "k_{\\mathrm{on},n}");
  assert.equal(withSubscript("\\dot{x}", "0"), "\\dot{x}_{0}");
  assert.equal(withSubscript("\\mathrm{x1}", "0"), "\\mathrm{x1}_{0}");
  assert.equal(withSubscript("x_{1}^{(4)}", "0"), "x_{1,0}^{(4)}");
  assert.equal(withSubscript("x^{(4)}", "0"), "x^{(4)}_{0}");
  const states = [{ name: "x_1", latex: "x_{1}", derived: false }];
  const axis = { kind: "state", index: 0 };
  assert.equal(axisLabelLatex(axis, { kind: "time" }, states, "map"), "x_{1,n}");
  assert.equal(axisLabelLatex(axis, { kind: "time" }, states, "ode"), "x_{1}");
});

test("fixed points count only when the core searched them: never for a field that changes with time", async () => {
  const { searchedFixedPoints } = await import("../state/model.ts");
  const fp = { x: [0, 0] };
  const portrait = (systemId, fixedPoints, shapeId = 1) => ({
    res: { fixedPoints: [fp] },
    meta: { systemId, shapeId, fixedPoints },
  });
  const sys = (autonomous) => ({ systemId: 3, autonomous });
  assert.deepEqual(searchedFixedPoints(sys(true), portrait(3, true), 1), [fp]);
  assert.equal(searchedFixedPoints(sys(true), portrait(3, false), 1), null, "asked for with the layer off");
  assert.equal(searchedFixedPoints(sys(true), portrait(2, true, 0), 1), null, "a system of another shape");
  assert.deepEqual(searchedFixedPoints(sys(true), portrait(2, true), 1), [fp], "a recompile keeping the states");
  assert.equal(searchedFixedPoints(sys(false), { ...portrait(3, true), res: { fixedPoints: [] } }, 1), null, "x' = x² − t");
  assert.equal(searchedFixedPoints(null, portrait(3, true), 1), null);
});

test("a recompile that keeps the kind and states keeps the last portrait up until its own arrives", async () => {
  const { portraitFits, buildPortraitJob, DEFAULT_LAYERS, DEFAULT_SOLVER } = await import("../state/model.ts");
  const compiled = (systemId) => ({
    ok: true,
    systemId,
    kind: "ode",
    hamiltonian: false,
    autonomous: true,
    states: [
      { name: "x", latex: "x", derived: false },
      { name: "y", latex: "y", derived: false },
    ],
    params: [],
    equationsLatex: [],
    diagnostics: [],
  });
  const snapshot = (systemId, shapeId) => ({
    compiled: compiled(systemId),
    params: [],
    view: { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [-5, 5], yRange: [-5, 5] },
    baseState: [0, 0],
    fieldTime: 0,
    ics: [],
    layers: DEFAULT_LAYERS,
    solver: DEFAULT_SOLVER,
    plotAspect: 2,
    shapeId,
  });
  const portraitOf = (s) => ({ res: { fixedPoints: [] }, meta: buildPortraitJob(s).meta, roundTripMs: 1 });
  const p = portraitOf(snapshot(7, 4));
  assert.ok(portraitFits(compiled(7), 4, p), "its own system");
  assert.ok(portraitFits(compiled(8), 4, p), "an edit that kept the states: no blank plot while it recomputes");
  assert.ok(!portraitFits(compiled(8), 5, p), "a template load or new states: another stride or another system");
  assert.ok(!portraitFits(null, 4, p) && !portraitFits(compiled(7), 4, null));
  // The selection's match tolerance is a fraction of the spans shown (equal scales widen x here), not the stored ones.
  assert.deepEqual(p.meta.view.xRange, [-10, 10]);
  assert.deepEqual(p.meta.view.yRange, [-5, 5]);
});

/** viewport.ts imports "../state/model" as the bundler resolves it; Node needs the ".ts". */
async function importViewport() {
  const { register } = await import("node:module");
  const hook = `export async function resolve(s, c, next) {
    try { return await next(s, c); } catch (e) {
      if (/^\\.\\.?\\//.test(s) && !/\\.[a-z]+$/.test(s)) return next(s + ".ts", c);
      throw e;
    }
  }`;
  register("data:text/javascript," + encodeURIComponent(hook));
  return import("../canvas/viewport.ts");
}

test("tick labels show the digit neighbouring ticks differ in, at any size and down to the zoom floor", async () => {
  const { niceTicks, tickLabel } = await importViewport();
  const labels = (lo, hi) => {
    const { ticks, step } = niceTicks(lo, hi, 8);
    return ticks.map((v) => tickLabel(v, step));
  };
  for (const [lo, hi] of [
    [1e7, 1.05e7],
    [896000, 1020000],
    [100000, 100010],
    [2e-5, 2.4e-5],
    [1 - 5e-9, 1 + 5e-9],
    [1 - 5e-13, 1 + 5e-13],
  ]) {
    const l = labels(lo, hi);
    assert.ok(l.length >= 2, `[${lo}, ${hi}]: ${l}`);
    assert.equal(new Set(l).size, l.length, `[${lo}, ${hi}]: every tick reads the same: ${l}`);
  }
  assert.deepEqual(labels(1e7, 1.05e7).slice(0, 2), ["1.000e7", "1.005e7"]);
  assert.deepEqual(labels(0, 2e5).slice(-2), ["1.8e5", "2.0e5"], "unchanged where one decimal was enough");
  assert.deepEqual(labels(-1, 1), ["−1", "−0.8", "−0.6", "−0.4", "−0.2", "0", "0.2", "0.4", "0.6", "0.8", "1"]);
});

test("zoom to fit keeps states that are large but not a blow-up", async () => {
  const { fitRanges } = await importViewport();
  // A population of 1e7 growing to 6e7 (time on x): the start point sets the scale.
  const ts = [0, 5, 10];
  const xs = [1e7, 3e7, 6e7];
  const fit = fitRanges(ts, xs, 0, [Infinity, 1e7]);
  assert.ok(fit.yRange[0] <= 1e7 && fit.yRange[1] >= 6e7, JSON.stringify(fit));
  // Started near 1, a run that blows up to 1e12 is still left out.
  const blow = fitRanges([0, 1, 2], [1, 2, 1e12], 0, [Infinity, 1]);
  assert.ok(blow.yRange[1] < 10, JSON.stringify(blow));
  assert.ok(fitRanges([0, 1, 2], [1, 2, 1e12]).yRange[1] < 10, "the default scale is 1");
});
