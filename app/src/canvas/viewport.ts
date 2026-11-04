import type { SystemKind } from "../api/types";
import { equalizeRanges, isFlowPhasePlane, type View } from "../state/model";

export { equalizeRanges, isFlowPhasePlane } from "../state/model";

export interface Viewport {
  left: number;
  top: number;
  width: number;
  height: number;
  xRange: [number, number];
  yRange: [number, number];
}

export function makeViewport(
  w: number,
  h: number,
  xRange: [number, number],
  yRange: [number, number],
  equal = false,
): Viewport {
  const width = Math.max(10, w);
  const height = Math.max(10, h);
  const r = equal ? equalizeRanges(xRange, yRange, width / height) : { xRange, yRange };
  return { left: 0, top: 0, width, height, xRange: r.xRange, yRange: r.yRange };
}

/** The viewport of the main plot: equal scales on a flow's phase plane. */
export function plotViewport(w: number, h: number, view: View, kind: SystemKind | undefined): Viewport {
  return makeViewport(w, h, view.xRange, view.yRange, isFlowPhasePlane(view.xAxis, view.yAxis, kind));
}

export function sx(vp: Viewport, x: number): number {
  return vp.left + ((x - vp.xRange[0]) / (vp.xRange[1] - vp.xRange[0])) * vp.width;
}

export function sy(vp: Viewport, y: number): number {
  return vp.top + (1 - (y - vp.yRange[0]) / (vp.yRange[1] - vp.yRange[0])) * vp.height;
}

export function toPlot(vp: Viewport, px: number, py: number): [number, number] {
  const x = vp.xRange[0] + ((px - vp.left) / vp.width) * (vp.xRange[1] - vp.xRange[0]);
  const y = vp.yRange[0] + (1 - (py - vp.top) / vp.height) * (vp.yRange[1] - vp.yRange[0]);
  return [x, y];
}

export function unitsPerPx(vp: Viewport): [number, number] {
  return [(vp.xRange[1] - vp.xRange[0]) / vp.width, (vp.yRange[1] - vp.yRange[0]) / vp.height];
}

export function insidePlot(vp: Viewport, px: number, py: number): boolean {
  return px >= vp.left && px <= vp.left + vp.width && py >= vp.top && py <= vp.top + vp.height;
}

export function niceTicks(lo: number, hi: number, target: number): { ticks: number[]; step: number } {
  const span = hi - lo;
  if (!(span > 0) || !Number.isFinite(span)) return { ticks: [], step: 1 };
  const raw = span / Math.max(1, target);
  const p = Math.pow(10, Math.floor(Math.log10(raw)));
  const m = raw / p;
  const step = (m < 1.5 ? 1 : m < 3.5 ? 2 : m < 7.5 ? 5 : 10) * p;
  const ticks: number[] = [];
  // Counted, not accumulated: deep in a zoom `v + step` rounds back to `v`.
  const first = Math.ceil(lo / step);
  const count = Math.floor(hi / step + 1e-9) - first;
  for (let k = 0; k <= count; k++) {
    const v = (first + k) * step;
    ticks.push(Math.abs(v) < step * 1e-9 ? 0 : v);
  }
  return { ticks, step };
}

/** Most digits a tick label shows after the point: enough down to the narrowest span {@link zoomRange} allows. */
const MAX_LABEL_DECIMALS = 20;

export function tickLabel(v: number, step: number): string {
  // Neighbouring ticks differ in the step's digit, so a label shows down to it (1.005e7, not 1.0e7 for every tick).
  const stepExp = Math.floor(Math.log10(step) + 1e-9);
  const abs = Math.abs(v);
  if (abs >= 1e5 || (abs > 0 && abs < 1e-4)) {
    const digits = Math.max(1, Math.min(15, Math.floor(Math.log10(abs) + 1e-9) - stepExp));
    return v.toExponential(digits).replace("+", "");
  }
  const decimals = Math.max(0, Math.min(MAX_LABEL_DECIMALS, -stepExp));
  const text = v.toFixed(decimals);
  return (text.includes(".") ? text.replace(/\.?0+$/, "") : text).replace("-", "−");
}

/** Narrower than this fraction of its coordinates, a span has too few doubles across it to draw (it rounds to a point). */
const MIN_REL_SPAN = 1e-12;
/** Around 0 a span shrinks without that limit; below this a grid of cells across it underflows. */
const MIN_SPAN = 1e-250;
/** Wider than this, a span is far past any state the solvers follow (blow-up stops at 1e12) and heads for overflow. */
const MAX_SPAN = 1e15;

/**
 * `range` zoomed by `factor` about `anchor`, or null when zooming in would
 * leave it too narrow, or zooming out too wide, to show. A view already past
 * either bound can always be zoomed back.
 */
export function zoomRange(range: [number, number], anchor: number, factor: number): [number, number] | null {
  const next: [number, number] = [anchor + (range[0] - anchor) * factor, anchor + (range[1] - anchor) * factor];
  const span = next[1] - next[0];
  const size = Math.max(Math.abs(next[0]), Math.abs(next[1]));
  const tooNarrow = factor < 1 && (span < MIN_REL_SPAN * size || span < MIN_SPAN);
  const tooWide = factor > 1 && !(span <= MAX_SPAN);
  return span > 0 && !tooNarrow && !tooWide ? next : null;
}

/**
 * Points this many times farther out than the size of the axis's start points
 * and fixed points (at least 1) belong to a blow-up and are left out of a fit.
 */
const BLOWUP_CUTOFF = 1e6;

/**
 * Ranges that fit the points `(xs[i], ys[i])`, padded. `scale` is the size of
 * the start points and fixed points on each axis (Infinity for a time axis,
 * which has no blow-ups), so a system whose states are large (a population
 * of 1e7) is fitted, not taken for a blow-up.
 */
export function fitRanges(
  xs: ArrayLike<number>,
  ys: ArrayLike<number>,
  padTop = 0,
  scale: [number, number] = [1, 1],
): { xRange: [number, number]; yRange: [number, number] } | null {
  const bounds = (vs: ArrayLike<number>, size: number): [number, number] | null => {
    const cutoff = BLOWUP_CUTOFF * Math.max(1, size);
    let lo = Infinity;
    let hi = -Infinity;
    for (let i = 0; i < vs.length; i++) {
      const v = vs[i];
      if (!Number.isFinite(v) || Math.abs(v) > cutoff) continue;
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    return lo <= hi ? [lo, hi] : null;
  };
  const bx = bounds(xs, scale[0]);
  const by = bounds(ys, scale[1]);
  if (!bx || !by) return null;
  const grow = ([lo, hi]: [number, number], below: number, above: number): [number, number] => {
    if (!(hi > lo)) {
      const span = Math.max(1, Math.abs(lo));
      return [lo - span * below, hi + span * above];
    }
    // Points that nearly coincide (a start at an equilibrium) fit no narrower than zoomRange lets a view get.
    const floor = Math.max(MIN_REL_SPAN * Math.max(Math.abs(lo), Math.abs(hi)), MIN_SPAN);
    const half = Math.max(hi - lo, floor) / 2;
    const c = lo + (hi - lo) / 2;
    const [a, b] = hi - lo < floor ? [c - half, c + half] : [lo, hi];
    return [a - (b - a) * below, b + (b - a) * above];
  };
  return { xRange: grow(bx, 0.08, 0.08), yRange: grow(by, 0.08, 0.08 + padTop) };
}
