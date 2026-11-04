// Node (>= 22.18) strips the types from model.ts when importing it.

import test from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_LAYERS,
  DEFAULT_SOLVER,
  FIXED_POINT_MATCH_FRACTION,
  buildPortraitJob,
  fixedPointMatchTolerance,
  nearestFixedPoint,
  selectFixedPoint,
  selectedFixedPointIndex,
  trackSelection,
} from "../state/model.ts";

const view = { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [-5, 5], yRange: [-2, 2] };
const fp = (...x) => ({ x });

test("the selected fixed point keeps its identity when the list is re-sorted", () => {
  const before = [fp(-1, 0), fp(0, 0), fp(1, 0)];
  const sel = selectFixedPoint(before[2].x);
  assert.equal(selectedFixedPointIndex(sel, before, view), 2);
  // A parameter change moves the points slightly and a new one appears in
  // front, so the sorted list shifts: index 2 is now a different point.
  const after = [fp(-3, 0), fp(-1.01, 0), fp(0, 0.01), fp(1.03, 0.02)];
  const next = trackSelection(sel, after, view);
  assert.equal(next.kind, "fp");
  assert.deepEqual(next.x, [1.03, 0.02]);
  assert.equal(next.key, sel.key, "tracking is the same selection");
  assert.equal(selectedFixedPointIndex(next, after, view), 3);
});

test("the selection is cleared when the selected fixed point disappears", () => {
  const sel = selectFixedPoint([1, 0]);
  // Its nearest neighbour is too far away to be the same point.
  assert.equal(trackSelection(sel, [fp(-1, 0), fp(2, 1)], view), null);
  assert.equal(trackSelection(sel, [], view), null);
  assert.equal(selectedFixedPointIndex(sel, [fp(-1, 0)], view), null);
});

test("an unchanged point keeps the same selection object; other selections pass through", () => {
  const sel = selectFixedPoint([0.5, 0.5]);
  assert.equal(trackSelection(sel, [fp(0, 0), fp(0.5, 0.5)], view), sel);
  const ic = { kind: "ic", id: 4 };
  assert.equal(trackSelection(ic, [], view), ic);
  assert.equal(trackSelection(null, [fp(0, 0)], view), null);
  assert.equal(selectedFixedPointIndex(ic, [fp(0, 0)], view), null);
  assert.notEqual(selectFixedPoint([0, 0]).key, selectFixedPoint([0, 0]).key);
});

test("the matching tolerance scales with the view", () => {
  assert.equal(fixedPointMatchTolerance(view), FIXED_POINT_MATCH_FRACTION * 10);
  const zoomed = { ...view, xRange: [-0.05, 0.05], yRange: [-0.05, 0.05] };
  assert.ok(Math.abs(fixedPointMatchTolerance(zoomed) - FIXED_POINT_MATCH_FRACTION * 0.1) < 1e-15);
  // Zoomed in, two nearby points are told apart; zoomed out they are not.
  const sel = selectFixedPoint([0, 0]);
  const moved = [fp(0.02, 0)];
  assert.equal(trackSelection(sel, moved, zoomed), null);
  assert.deepEqual(trackSelection(sel, moved, view).x, [0.02, 0]);
  // A time axis does not count: the state axis sets the scale.
  const tx = { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 1000], yRange: [-1, 1] };
  assert.equal(fixedPointMatchTolerance(tx), FIXED_POINT_MATCH_FRACTION * 2);
});

test("the nearest point within the tolerance wins, and dimensions must agree", () => {
  const fps = [fp(0.1, 0), fp(0.05, 0), fp(0, 0, 0)];
  assert.equal(nearestFixedPoint(fps, [0, 0], 0.2), 1);
  assert.equal(nearestFixedPoint(fps, [0, 0], 0.01), -1);
  assert.equal(nearestFixedPoint(fps, [0, 0, 0], 0.01), 2);
});

test("a point that moves fast during a slider drag stays selected", () => {
  // x' = r + x^2 near the collision: r from -0.17 to -0.1 moves the
  // upper point from 0.41 to 0.32, about 2% of the 5-unit view.
  const sn = { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 6], yRange: [-2.5, 2.5] };
  const prev = [fp(-Math.sqrt(0.17)), fp(Math.sqrt(0.17))];
  const sel = selectFixedPoint(prev[1].x);
  const next = trackSelection(sel, [fp(-Math.sqrt(0.1)), fp(Math.sqrt(0.1))], sn, prev);
  assert.deepEqual(next.x, [Math.sqrt(0.1)]);
  // Past the collision both are gone.
  assert.equal(trackSelection(next, [], sn, [fp(-0.05), fp(0.05)]), null);
});

test("a point that merges into a neighbour does not hand the selection to it", () => {
  // Lorenz below r = 1: C- shrinks into the origin and vanishes. The origin
  // is the nearest new point, but it is the origin's continuation.
  const lv = { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 2 }, xRange: [-25, 25], yRange: [0, 50] };
  const prev = [fp(0, 0, 0), fp(-0.89, -0.89, 0.3), fp(0.89, 0.89, 0.3)];
  const sel = selectFixedPoint(prev[1].x);
  assert.equal(trackSelection(sel, [fp(0, 0, 0)], lv, prev), null);
  // Without the previous list only the distance can be checked.
  assert.deepEqual(trackSelection(sel, [fp(0, 0, 0)], lv).x, [0, 0, 0]);
  // While it still exists it is followed, even with the origin closer than the view tolerance.
  const moved = [fp(0, 0, 0), fp(-0.7, -0.7, 0.2), fp(0.7, 0.7, 0.2)];
  assert.deepEqual(trackSelection(sel, moved, lv, prev).x, [-0.7, -0.7, 0.2]);
});

test("a portrait records whether it searched for fixed points, so hiding the layer keeps the selection", () => {
  const snapshot = (fixedPoints) => ({
    compiled: {
      ok: true,
      systemId: 1,
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
    },
    params: [],
    view,
    baseState: [0, 0],
    fieldTime: 0,
    ics: [],
    layers: { ...DEFAULT_LAYERS, fixedPoints },
    solver: DEFAULT_SOLVER,
    plotAspect: null,
  });
  for (const on of [true, false]) {
    const job = buildPortraitJob(snapshot(on));
    assert.equal(job.body.fixedPoints, on);
    assert.equal(job.meta.fixedPoints, on);
  }
});
