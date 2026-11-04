// Real eigenvectors of a fixed point, read from the core's output (pure: no
// React, no DOM, only type imports, so `node --test` can import it; see
// src/review/eigen.test.mjs).
//
// The core (dyn-analysis, docs/contracts.md) decides which eigenvalues are real:
// an entry with a vector and a `null` imaginary part is a real eigenvector of
// Re λ. That includes a repeated real eigenvalue that rounding turned into a
// slightly complex pair (a degenerate node computed as −1 ± 1e-6 i), so this
// module trusts the core instead of re-testing Im λ against a tight tolerance.

import type { FixedPointOut, SystemKind } from "../api/types";
import type { SceneAxes } from "./draw";

/** -1: stable direction (arrowheads in), 1: unstable (out), 0: neither (no heads). */
export type EigenFlow = -1 | 0 | 1;

/** One real eigenvector of a fixed point, in state coordinates. */
export interface RealEigenvector {
  /** Unit vector, one entry per state. */
  v: number[];
  /** The eigenvalue (flows) or multiplier (maps): Re λ. */
  value: number;
  flow: EigenFlow;
  /** "λ₁", "λ₂", … as the Inspector row numbers them (reals ascending); "λ" for a single or double one. */
  label: string;
}

const SUB = "₀₁₂₃₄₅₆₇₈₉";
const sub = (k: number) => String(k).replace(/\d/g, (d) => SUB[Number(d)]);
const IM_TOL = 1e-9;
/** Below this, an eigenvalue (or |μ| − 1) counts as zero: no arrowheads. */
const NEUTRAL_TOL = 1e-9;
/**
 * The core groups eigenvalues within δ = sqrt(REL_TOL)·s of each other
 * (REL_TOL = 1e-9; s = max|λ|, or up to 2·max|λ| for a 2D flow), and every
 * group has at least one vector (dyn-analysis eigvec.rs). Twice that bounds
 * the distance from a member without a vector to one of its group with one.
 */
const GROUP_REACH = 2 * Math.sqrt(1e-9);

/** Is the vector finite and of the state's length? */
const usable = (v: number[] | null | undefined, dim: number): v is number[] =>
  !!v && v.length === dim && v.every(Number.isFinite);

/**
 * Which eigenvalues the core treats as real, one flag per eigenvalue: those
 * with a real vector (a vector and a `null` Im part), and those without a
 * vector (a defective eigenvalue's missing direction) that have |Im λ| < 1e-9
 * or sit next to an eigenvalue with a real vector (the rest of their group).
 */
export function realMask(fp: FixedPointOut): boolean[] {
  const evs = fp.eigenvalues;
  const vecs = fp.eigenvectors ?? [];
  const ims = fp.eigenvectorsIm ?? [];
  const dim = fp.x.length;
  const s = evs.reduce((m, [re, im]) => Math.max(m, Math.hypot(re, im)), 0);
  const reach = GROUP_REACH * s;
  const withRealVector = evs.map((_, i) => (ims[i] ?? null) === null && usable(vecs[i], dim));
  return evs.map(([re, im], i) => {
    if ((ims[i] ?? null) !== null) return false;
    if (withRealVector[i] || Math.abs(im) < IM_TOL) return true;
    return evs.some(([r, m], j) => withRealVector[j] && Math.hypot(re - r, im - m) <= reach);
  });
}

/** The eigenvalues as the Inspector shows them: those the core treats as real get Im = 0. */
export function displayEigenvalues(fp: FixedPointOut): [number, number][] {
  const real = realMask(fp);
  return fp.eigenvalues.map(([re, im], i) => [re, real[i] ? 0 : im]);
}

/** One eigenvalue, or a complex pair α ± ωi, numbered as the Inspector row writes it. */
export interface NumberedEigenvalue {
  /** 1-based numbers: [] unnumbered ("λ"), [k] ("λₖ"), or [k, k + 1] for a pair ("λₖ,ₖ₊₁"). */
  numbers: number[];
  re: number;
  /** |Im| of a complex pair (shown as α ± ωi); 0 for a real eigenvalue. */
  im: number;
  /** A planar flow's repeated real eigenvalue, written once as "λ = … (double)". */
  double: boolean;
}

/**
 * The eigenvalues in the Inspector row's order and numbering, so every place
 * that shows them (the row, the canvas tooltip, the λₖ labels on the
 * eigenvector lines) agrees: one eigenvalue or a planar complex pair is
 * unnumbered, a double one is written once, otherwise the reals come first in
 * ascending order (λ₁, λ₂, …) and then each complex pair (λₖ,ₖ₊₁ = α ± ωi).
 */
export function numberedEigenvalues(fp: FixedPointOut): NumberedEigenvalue[] {
  const evs = displayEigenvalues(fp);
  const isReal = ([, im]: [number, number]) => Math.abs(im) < IM_TOL;
  const one = (numbers: number[], re: number, im = 0, double = false): NumberedEigenvalue => ({
    numbers,
    re,
    im: Math.abs(im),
    double,
  });
  if (evs.length === 1) return [one([], evs[0][0])];
  if (evs.length === 2) {
    const [a, b] = evs;
    if (!isReal(a)) return [one([], a[0], a[1])];
    if (Math.abs(a[0] - b[0]) <= 1e-9 * Math.max(1, Math.abs(a[0]))) return [one([], a[0], 0, true)];
  }
  const reals = evs.filter(isReal).map(([re]) => re).sort((x, y) => x - y);
  const pairs = evs.filter(([, im]) => im > IM_TOL);
  return [
    ...reals.map((re, k) => one([k + 1], re)),
    ...pairs.map(([re, im], j) => {
      const k = reals.length + 2 * j + 1;
      return one([k, k + 1], re, im);
    }),
  ];
}

/** Does the fixed point have a genuinely complex eigenvalue (no real eigenvector for it)? */
export function hasComplexEigenvalue(fp: FixedPointOut): boolean {
  return realMask(fp).some((r) => !r);
}

/**
 * The real eigenvectors of a fixed point, skipping complex eigenvalues and the
 * missing directions of defective ones (`null` in the core's output). A flow's
 * direction is stable when λ < 0; a map's when |μ| < 1.
 */
export function realEigenvectors(fp: FixedPointOut, kind: SystemKind | undefined): RealEigenvector[] {
  const evs = displayEigenvalues(fp);
  const vecs = fp.eigenvectors ?? [];
  const ims = fp.eigenvectorsIm ?? [];
  const dim = fp.x.length;
  const real = realMask(fp);
  const realIdx = evs.map((_, i) => i).filter((i) => real[i]);
  // Numbered as the Inspector row writes them: reals ascending, from 1.
  const ranked = [...realIdx].sort((a, b) => evs[a][0] - evs[b][0] || a - b);
  const sym = kind === "map" ? "μ" : "λ";
  const double =
    evs.length === 2 &&
    realIdx.length === 2 &&
    Math.abs(evs[0][0] - evs[1][0]) <= 1e-9 * Math.max(1, Math.abs(evs[0][0]));
  const plain = evs.length === 1 || double;
  const out: RealEigenvector[] = [];
  for (const i of realIdx) {
    const v = vecs[i];
    if ((ims[i] ?? null) !== null || !usable(v, dim)) continue;
    const value = evs[i][0];
    const growth = kind === "map" ? Math.abs(value) - 1 : value;
    const flow: EigenFlow = growth < -NEUTRAL_TOL ? -1 : growth > NEUTRAL_TOL ? 1 : 0;
    out.push({ v, value, flow, label: plain ? sym : `${sym}${sub(ranked.indexOf(i) + 1)}` });
  }
  return out;
}

/** Does any fixed point other than `selected` have a real eigenvector to draw? */
export function othersHaveRealEigenvectors(
  fps: FixedPointOut[],
  selected: number,
  kind: SystemKind | undefined,
): boolean {
  return fps.some((o, j) => j !== selected && realEigenvectors(o, kind).length > 0);
}

/** A real eigenvector and its projection onto the plot axes (state units). */
export interface EigenDirection extends RealEigenvector {
  p: [number, number];
}

/**
 * The real eigenvectors of `fp` that show in the current axes: none with time
 * on an axis, and none seen nearly end-on (a projection shorter than 0.05 of
 * the unit vector, e.g. the Lorenz origin's z direction in the x–y view).
 */
export function eigenDirections(scene: SceneAxes, fp: FixedPointOut): EigenDirection[] {
  if (scene.xAxis.kind === "time" || scene.yAxis.kind === "time") return [];
  const xi = scene.xAxis.index;
  const yi = scene.yAxis.index;
  return realEigenvectors(fp, scene.kind).flatMap((e) => {
    const p: [number, number] = [e.v[xi] ?? 0, e.v[yi] ?? 0];
    return Math.hypot(p[0], p[1]) < 0.05 ? [] : [{ ...e, p }];
  });
}
