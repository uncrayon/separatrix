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

export function tickLabel(v: number, step: number): string {
  const decimals = Math.max(0, Math.min(8, -Math.floor(Math.log10(step) + 1e-9)));
  const abs = Math.abs(v);
  if (abs >= 1e5 || (abs > 0 && abs < 1e-4)) return v.toExponential(1).replace("+", "");
  const text = v.toFixed(decimals);
  return (text.includes(".") ? text.replace(/\.?0+$/, "") : text).replace("-", "−");
}

export function zoomRange(range: [number, number], anchor: number, factor: number): [number, number] {
  return [anchor + (range[0] - anchor) * factor, anchor + (range[1] - anchor) * factor];
}

/** Points beyond this size belong to a blow-up and are left out of a fit. */
const BLOWUP_CUTOFF = 1e6;

export function fitRanges(
  xs: ArrayLike<number>,
  ys: ArrayLike<number>,
  padTop = 0,
): { xRange: [number, number]; yRange: [number, number] } | null {
  const bounds = (vs: ArrayLike<number>): [number, number] | null => {
    let lo = Infinity;
    let hi = -Infinity;
    for (let i = 0; i < vs.length; i++) {
      const v = vs[i];
      if (!Number.isFinite(v) || Math.abs(v) > BLOWUP_CUTOFF) continue;
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    return lo <= hi ? [lo, hi] : null;
  };
  const bx = bounds(xs);
  const by = bounds(ys);
  if (!bx || !by) return null;
  const grow = ([lo, hi]: [number, number], below: number, above: number): [number, number] => {
    const span = hi - lo || Math.max(1, Math.abs(lo));
    return [lo - span * below, hi + span * above];
  };
  return { xRange: grow(bx, 0.08, 0.08), yRange: grow(by, 0.08, 0.08 + padTop) };
}
