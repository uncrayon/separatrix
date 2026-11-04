// Regression tests for the eigenvector helpers behind "Show eigenvectors"
// and their projection onto the plot axes (app/src/canvas/eigenvectors.ts,
// React-free; draw.ts re-exports eigenDirections).
//
// Plain JavaScript for the same reason as model.review.test.mjs: tsc never
// sees it, and Node (>= 22.18) strips the types from the .ts modules.
//
// Run from app/:   node --test src/review/

import test from "node:test";
import assert from "node:assert/strict";
import {
  displayEigenvalues,
  hasComplexEigenvalue,
  numberedEigenvalues,
  othersHaveRealEigenvectors,
  realEigenvectors,
  realMask,
  eigenDirections,
} from "../canvas/eigenvectors.ts";

const r2 = Math.SQRT2;
const r3 = Math.sqrt(3);

/** Rabbits vs sheep saddle at (1, 1): J = [[-1, -2], [-1, -1]], λ = −1 ± √2, as the core returns it. */
const saddle = {
  x: [1, 1],
  eigenvalues: [
    [r2 - 1, 0],
    [-1 - r2, 0],
  ],
  eigenvectors: [
    [r2 / r3, -1 / r3],
    [r2 / r3, 1 / r3],
  ],
  eigenvectorsIm: [null, null],
  classification: "saddle",
  stable: false,
};

/** A stable spiral: complex pair, no real eigenvector. */
const spiral = {
  x: [0, 0],
  eigenvalues: [
    [-0.1, 1],
    [-0.1, -1],
  ],
  eigenvectors: [
    [Math.SQRT1_2, 0],
    [Math.SQRT1_2, 0],
  ],
  eigenvectorsIm: [
    [0, Math.SQRT1_2],
    [0, -Math.SQRT1_2],
  ],
  classification: "stable spiral",
  stable: true,
};

/**
 * J = [[-1, 1], [-1e-12, -1]] as the core returns it: λ = −1 ± 1.00004e-6 i by
 * rounding, classified a degenerate node, with a real vector and Im null.
 */
const roundedDegenerate = {
  x: [0, 0],
  eigenvalues: [
    [-1, 1.00004e-6],
    [-1, -1.00004e-6],
  ],
  eigenvectors: [[1, 0], null],
  eigenvectorsIm: [null, null],
  classification: "stable degenerate node",
  stable: true,
};

const close = (a, b, tol = 1e-12) => a.length === b.length && a.every((v, i) => Math.abs(v - b[i]) <= tol);

test("saddle: unstable λ ≈ 0.414 flows out along (0.816, −0.577), stable λ ≈ −2.414 flows in along (0.816, 0.577)", () => {
  const evs = realEigenvectors(saddle, "ode");
  assert.equal(evs.length, 2);
  const out = evs.find((e) => e.value > 0);
  const inn = evs.find((e) => e.value < 0);
  assert.equal(out.flow, 1);
  assert.equal(inn.flow, -1);
  assert.ok(Math.abs(out.value - 0.41421356) < 1e-6, String(out.value));
  assert.ok(Math.abs(inn.value + 2.41421356) < 1e-6, String(inn.value));
  assert.ok(close(out.v, [0.816496580927726, -0.5773502691896258]), String(out.v));
  assert.ok(close(inn.v, [0.816496580927726, 0.5773502691896258]), String(inn.v));
  // Labels follow the Inspector row: reals ascending, λ₁ is the smaller.
  assert.equal(inn.label, "λ₁");
  assert.equal(out.label, "λ₂");
});

test("maps: the flow follows |μ| − 1, and labels use μ", () => {
  const fp = {
    ...saddle,
    eigenvalues: [
      [1.5, 0],
      [-0.5, 0],
    ],
    classification: "saddle (n-D)",
  };
  const evs = realEigenvectors(fp, "map");
  assert.deepEqual(
    evs.map((e) => [e.value, e.flow, e.label]),
    [
      [1.5, 1, "μ₂"],
      [-0.5, -1, "μ₁"],
    ],
  );
});

test("complex eigenvalues have no real eigenvectors", () => {
  assert.deepEqual(realEigenvectors(spiral, "ode"), []);
  assert.equal(hasComplexEigenvalue(spiral), true);
  assert.deepEqual(realMask(spiral), [false, false]);
  assert.equal(hasComplexEigenvalue(saddle), false);
});

test("a degenerate node that rounding made slightly complex still shows its real eigenvector", () => {
  // The core treats it as real (vector present, Im null): trust it rather than re-testing Im λ.
  const evs = realEigenvectors(roundedDegenerate, "ode");
  assert.equal(evs.length, 1);
  assert.deepEqual(evs[0].v, [1, 0]);
  assert.equal(evs[0].value, -1);
  assert.equal(evs[0].flow, -1);
  assert.equal(evs[0].label, "λ", "a double eigenvalue is labelled plain λ");
  assert.equal(hasComplexEigenvalue(roundedDegenerate), false);
  assert.deepEqual(displayEigenvalues(roundedDegenerate), [
    [-1, 0],
    [-1, 0],
  ]);
  // The same in 3D: λ = −1 ± 1e-6 i with real vector (1, 0, 0), plus λ = 3.
  const fp3 = {
    x: [0, 0, 0],
    eigenvalues: [
      [-1, 1e-6],
      [-1, -1e-6],
      [3, 0],
    ],
    eigenvectors: [[1, 0, 0], null, [0, 0, 1]],
    eigenvectorsIm: [null, null, null],
    classification: "saddle (n-D)",
    stable: false,
  };
  const e3 = realEigenvectors(fp3, "ode");
  assert.deepEqual(
    e3.map((e) => [e.v, e.flow]),
    [
      [[1, 0, 0], -1],
      [[0, 0, 1], 1],
    ],
  );
  assert.equal(hasComplexEigenvalue(fp3), false);
});

test("a defective complex pair's missing direction stays complex", () => {
  // 4D [[R, I], [0, R]] with R a rotation: ±i double, one vector per conjugate.
  const h = Math.SQRT1_2;
  const fp = {
    x: [0, 0, 0, 0],
    eigenvalues: [
      [0, 1],
      [0, -1],
      [0, 1],
      [0, -1],
    ],
    eigenvectors: [[h, 0, 0, 0], [h, 0, 0, 0], null, null],
    eigenvectorsIm: [[0, h, 0, 0], [0, -h, 0, 0], null, null],
    classification: "non-hyperbolic",
    stable: false,
  };
  assert.deepEqual(realMask(fp), [false, false, false, false]);
  assert.deepEqual(realEigenvectors(fp, "ode"), []);
  assert.deepEqual(displayEigenvalues(fp), fp.eigenvalues);
});

test("non-finite or missing vectors are skipped", () => {
  const fp = { ...saddle, eigenvectors: [[NaN, 1], [1]] };
  assert.deepEqual(realEigenvectors(fp, "ode"), []);
  const noVectors = { ...saddle, eigenvectors: undefined, eigenvectorsIm: undefined };
  assert.deepEqual(realEigenvectors(noVectors, "ode"), []);
});

test("othersHaveRealEigenvectors: only another fixed point with a real vector counts", () => {
  assert.equal(othersHaveRealEigenvectors([spiral], 0, "ode"), false);
  assert.equal(othersHaveRealEigenvectors([spiral, spiral], 0, "ode"), false);
  assert.equal(othersHaveRealEigenvectors([spiral, saddle], 0, "ode"), true);
  assert.equal(othersHaveRealEigenvectors([saddle, spiral], 0, "ode"), false);
});

test("eigenDirections: projected onto the axes; none with time on an axis or seen end-on", () => {
  const axes = (x, y, dim = 2) => ({ kind: "ode", dim, xAxis: x, yAxis: y });
  const s = (i) => ({ kind: "state", index: i });
  const dirs = eigenDirections(axes(s(0), s(1)), saddle);
  assert.equal(dirs.length, 2);
  assert.ok(close(dirs[0].p, saddle.eigenvectors[0]));
  // Swapped axes swap the projection.
  const swapped = eigenDirections(axes(s(1), s(0)), saddle);
  assert.ok(close(swapped[0].p, [saddle.eigenvectors[0][1], saddle.eigenvectors[0][0]]));
  assert.deepEqual(eigenDirections(axes({ kind: "time" }, s(0)), saddle), []);
  // 3D: the z direction is seen end-on in the x–y view.
  const fp3 = {
    x: [0, 0, 0],
    eigenvalues: [
      [-1, 0],
      [2, 0],
      [-3, 0],
    ],
    eigenvectors: [
      [1, 0, 0],
      [0, 1, 0],
      [0, 0, 1],
    ],
    eigenvectorsIm: [null, null, null],
    classification: "saddle (n-D)",
    stable: false,
  };
  assert.deepEqual(
    eigenDirections(axes(s(0), s(1), 3), fp3).map((d) => d.value),
    [-1, 2],
  );
});

test("numberedEigenvalues: the tooltip numbers eigenvalues as the Inspector row and the line labels do", () => {
  // The core returns the saddle's λ ≈ 0.414 first; the row (and the λ₁ label on the stable line) puts −2.414 first.
  const sad = numberedEigenvalues(saddle);
  assert.deepEqual(
    sad.map((e) => e.numbers),
    [[1], [2]],
  );
  assert.ok(Math.abs(sad[0].re - (-1 - r2)) < 1e-12 && Math.abs(sad[1].re - (r2 - 1)) < 1e-12);
  const labels = Object.fromEntries(realEigenvectors(saddle, "ode").map((e) => [e.label, e.value]));
  assert.equal(labels["λ₁"], sad[0].re);
  assert.equal(labels["λ₂"], sad[1].re);
  // A planar complex pair is one unnumbered entry, α ± |ω|i.
  assert.deepEqual(numberedEigenvalues(spiral), [{ numbers: [], re: -0.1, im: 1, double: false }]);
  // A rounded double eigenvalue is written once.
  assert.deepEqual(numberedEigenvalues(roundedDegenerate), [{ numbers: [], re: -1, im: 0, double: true }]);
  // Lorenz C+: the real λ first, then the pair as λ₂,₃ (the core may list the pair first).
  const lorenz = {
    x: [8.485, 8.485, 27],
    eigenvalues: [
      [0.094, 10.195],
      [0.094, -10.195],
      [-13.855, 0],
    ],
    eigenvectors: [[0.5, 0, 0.5], [0.5, 0, 0.5], [0.85, -0.33, -0.4]],
    eigenvectorsIm: [[0, 0.7, 0], [0, -0.7, 0], null],
    classification: "saddle (n-D)",
    stable: false,
  };
  assert.deepEqual(numberedEigenvalues(lorenz), [
    { numbers: [1], re: -13.855, im: 0, double: false },
    { numbers: [2, 3], re: 0.094, im: 10.195, double: false },
  ]);
});
