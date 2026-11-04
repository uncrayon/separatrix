// Plot-space ↔ screen-space mapping, tick placement, zoom and pan.
//
// "Plot" coordinates are the maths values on the axes; "screen" coordinates
// are CSS pixels inside the canvas element (y grows downwards).

import { equalizeRanges } from "../state/model";

export { equalizeRanges, wantsEqualScales } from "../state/model";

/** The drawing rectangle and the ranges shown in it. */
export interface Viewport {
  /** Plot rectangle in CSS px, relative to the canvas. */
  left: number;
  top: number;
  width: number;
  height: number;
  xRange: [number, number];
  yRange: [number, number];
}

/**
 * Space reserved around the plot (CSS px, before scaling). The plot fills the
 * well edge to edge: tick labels sit beside the axes on a halo (PhasePlane
 * README), so nothing needs a margin of its own.
 */
export const MARGIN = { left: 0, right: 0, top: 0, bottom: 0 };

/**
 * Build the viewport for a canvas of `w × h` CSS px. `scale` grows margins in
 * projector mode. With `equal`, the ranges are widened so both axes have the
 * same pixels per unit (see {@link equalizeRanges}).
 */
export function makeViewport(
  w: number,
  h: number,
  xRange: [number, number],
  yRange: [number, number],
  scale = 1,
  equal = false,
): Viewport {
  const left = MARGIN.left * scale;
  const top = MARGIN.top * scale;
  const width = Math.max(10, w - left - MARGIN.right * scale);
  const height = Math.max(10, h - top - MARGIN.bottom * scale);
  const r = equal ? equalizeRanges(xRange, yRange, width / height) : { xRange, yRange };
  return { left, top, width, height, xRange: r.xRange, yRange: r.yRange };
}

/** Plot → screen x. */
export function sx(vp: Viewport, x: number): number {
  return vp.left + ((x - vp.xRange[0]) / (vp.xRange[1] - vp.xRange[0])) * vp.width;
}

/** Plot → screen y. */
export function sy(vp: Viewport, y: number): number {
  return vp.top + (1 - (y - vp.yRange[0]) / (vp.yRange[1] - vp.yRange[0])) * vp.height;
}

/** Screen → plot. */
export function toPlot(vp: Viewport, px: number, py: number): [number, number] {
  const x = vp.xRange[0] + ((px - vp.left) / vp.width) * (vp.xRange[1] - vp.xRange[0]);
  const y = vp.yRange[0] + (1 - (py - vp.top) / vp.height) * (vp.yRange[1] - vp.yRange[0]);
  return [x, y];
}

/** Plot units per screen pixel, horizontally and vertically. */
export function unitsPerPx(vp: Viewport): [number, number] {
  return [(vp.xRange[1] - vp.xRange[0]) / vp.width, (vp.yRange[1] - vp.yRange[0]) / vp.height];
}

/** Is a screen point inside the plot rectangle? */
export function insidePlot(vp: Viewport, px: number, py: number): boolean {
  return px >= vp.left && px <= vp.left + vp.width && py >= vp.top && py <= vp.top + vp.height;
}

/**
 * "Nice" tick positions (1, 2, 5 × 10^k) covering [lo, hi] with roughly
 * `target` ticks. Returns the ticks and the step (for label precision).
 */
export function niceTicks(lo: number, hi: number, target: number): { ticks: number[]; step: number } {
  const span = hi - lo;
  if (!(span > 0) || !Number.isFinite(span)) return { ticks: [], step: 1 };
  const raw = span / Math.max(1, target);
  const p = Math.pow(10, Math.floor(Math.log10(raw)));
  const m = raw / p;
  const step = (m < 1.5 ? 1 : m < 3.5 ? 2 : m < 7.5 ? 5 : 10) * p;
  const ticks: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) {
    ticks.push(Math.abs(v) < step * 1e-9 ? 0 : v);
  }
  return { ticks, step };
}

/** Label a tick with at most the decimals its step needs, trailing zeros dropped. */
export function tickLabel(v: number, step: number): string {
  const decimals = Math.max(0, Math.min(8, -Math.floor(Math.log10(step) + 1e-9)));
  const abs = Math.abs(v);
  if (abs >= 1e5 || (abs > 0 && abs < 1e-4)) return v.toExponential(1).replace("+", "");
  // Just enough decimals for this value: 0.5, 1, 1.5 rather than 0.5, 1.0, 1.5.
  const text = v.toFixed(decimals);
  return (text.includes(".") ? text.replace(/\.?0+$/, "") : text).replace("-", "−");
}

/** Zoom a range by `factor` (> 1 zooms out) keeping `anchor` fixed. */
export function zoomRange(range: [number, number], anchor: number, factor: number): [number, number] {
  return [anchor + (range[0] - anchor) * factor, anchor + (range[1] - anchor) * factor];
}

/**
 * Ranges that show every value in `xs` and `ys` with a margin, for "Zoom to
 * fit". `padTop` is extra room (a fraction of the span) above the data, so the
 * floating canvas toolbar does not cover what it just framed. Returns null
 * when there is nothing finite to fit.
 */
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
      // Ignore blow-ups: a trajectory that escapes to 1e300 would flatten the rest.
      if (!Number.isFinite(v) || Math.abs(v) > 1e6) continue;
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
