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
