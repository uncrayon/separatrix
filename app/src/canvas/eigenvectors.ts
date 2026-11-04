import type { FixedPointOut, SystemKind } from "../api/types";
import type { SceneAxes } from "./draw";

export type EigenFlow = -1 | 0 | 1;

export interface RealEigenvector {
  v: number[];
  value: number;
  flow: EigenFlow;
  label: string;
}

const SUBSCRIPT_DIGITS = "₀₁₂₃₄₅₆₇₈₉";

/** `n` in Unicode subscript digits, for labels such as λ₁₂. */
export function subscriptDigits(n: number): string {
  return String(n).replace(/\d/g, (d) => SUBSCRIPT_DIGITS[Number(d)]);
}

const IM_TOL = 1e-9;
const NEUTRAL_TOL = 1e-9;
/** REL_TOL in crates/dyn-analysis/src/classify.rs: the core groups eigenvalues within sqrt(REL_TOL). */
const CORE_REL_TOL = 1e-9;
const GROUP_REACH = 2 * Math.sqrt(CORE_REL_TOL);

const usable = (v: number[] | null | undefined, dim: number): v is number[] =>
  !!v && v.length === dim && v.every(Number.isFinite);

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

export function displayEigenvalues(fp: FixedPointOut): [number, number][] {
  const real = realMask(fp);
  return fp.eigenvalues.map(([re, im], i) => [re, real[i] ? 0 : im]);
}

export interface NumberedEigenvalue {
  numbers: number[];
  re: number;
  im: number;
  double: boolean;
}

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

export function hasComplexEigenvalue(fp: FixedPointOut): boolean {
  return realMask(fp).some((r) => !r);
}

export function realEigenvectors(fp: FixedPointOut, kind: SystemKind | undefined): RealEigenvector[] {
  const evs = displayEigenvalues(fp);
  const vecs = fp.eigenvectors ?? [];
  const ims = fp.eigenvectorsIm ?? [];
  const dim = fp.x.length;
  const real = realMask(fp);
  const realIdx = evs.map((_, i) => i).filter((i) => real[i]);
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
    out.push({ v, value, flow, label: plain ? sym : `${sym}${subscriptDigits(ranked.indexOf(i) + 1)}` });
  }
  return out;
}

export function othersHaveRealEigenvectors(
  fps: FixedPointOut[],
  selected: number,
  kind: SystemKind | undefined,
): boolean {
  return fps.some((o, j) => j !== selected && realEigenvectors(o, kind).length > 0);
}

export interface EigenDirection extends RealEigenvector {
  p: [number, number];
}

export function eigenDirections(scene: SceneAxes, fp: FixedPointOut): EigenDirection[] {
  if (scene.xAxis.kind === "time" || scene.yAxis.kind === "time") return [];
  const xi = scene.xAxis.index;
  const yi = scene.yAxis.index;
  return realEigenvectors(fp, scene.kind).flatMap((e) => {
    const p: [number, number] = [e.v[xi] ?? 0, e.v[yi] ?? 0];
    return Math.hypot(p[0], p[1]) < 0.05 ? [] : [{ ...e, p }];
  });
}
