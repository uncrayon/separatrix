// UI review regression tests for the pure model helpers (app/src/state/model.ts).
//
// Plain JavaScript on purpose: `npm run build` type-checks app/src with tsc and
// the app has no @types/node, so a .ts test importing node:test would break
// the build. Node (>= 22.18) strips the types from model.ts when importing it.
//
// Run from app/:   node --test src/review/
//
// Each test states the behaviour the UI promises; a failing test is a bug
// found in the review (see the test name).

import test from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_LAYERS, DEFAULT_SOLVER, stateExtent, trajectorySpans } from "../state/model.ts";

const timeOnX = {
  xAxis: { kind: "time" },
  yAxis: { kind: "state", index: 0 },
  xRange: [-2, 50],
  yRange: [-30, 30],
};
const phasePlane = {
  xAxis: { kind: "state", index: 0 },
  yAxis: { kind: "state", index: 2 },
  xRange: [-25, 25],
  yRange: [-2, 54],
};
const ic = { id: 1, x0: [0, 1, 0], t0: 0, color: 0 };

test("backward time off: no backward run in the phase plane (baseline)", () => {
  const spans = trajectorySpans("ode", phasePlane, ic, DEFAULT_SOLVER, { ...DEFAULT_LAYERS, backward: false });
  assert.deepEqual(
    spans.map((s) => s.direction),
    ["forward"],
  );
});

test("backward time off is respected when time is on an axis", () => {
  // Lorenz template (backward: false) after choosing t on the horizontal axis.
  // Today a backward run t: 0 -> -2 is still requested; it hits the step
  // limit, draws a solid band at the left edge and raises
  // "1 run stopped by the solver" although the user switched backward time off.
  const spans = trajectorySpans("ode", timeOnX, ic, DEFAULT_SOLVER, { ...DEFAULT_LAYERS, backward: false });
  assert.deepEqual(
    spans.map((s) => s.direction),
    ["forward"],
    "the Backward time chip is off, yet a backward run is requested",
  );
});

/** A PortraitResponse with one 3-state trajectory per entry. */
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

test("stateExtent: a run the solver abandoned does not stretch the axis range", () => {
  // Lorenz: the attractor has |x| < 25; the abandoned backward run reached
  // x = -4.5e5 before hitting maxSteps. setAxis() uses this extent for the new
  // axis range AND stores it as the home view, so Reset view cannot undo it.
  const res = response(
    { status: "completed", x: [[0, 1, 0], [-18, -20, 40], [20, 25, 45]] },
    { status: "maxSteps", x: [[0, 1, 0], [-4.5e5, 2e5, 9e5]] },
  );
  const ext = stateExtent(res, 3, 0);
  assert.ok(ext, "extent exists");
  assert.ok(ext[0] >= -25, `x extent starts at ${ext[0]}, far outside the attractor`);
});
