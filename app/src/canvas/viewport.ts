// Plot-space ↔ screen-space mapping, tick placement, zoom and pan.
//
// "Plot" coordinates are the maths values on the axes; "screen" coordinates
// are CSS pixels inside the canvas element (y grows downwards).

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

/** Space reserved around the plot for tick labels (CSS px, before scaling). */
export const MARGIN = { left: 48, right: 14, top: 14, bottom: 30 };

/** Build the viewport for a canvas of `w × h` CSS px. `scale` grows margins in projector mode. */
export function makeViewport(
  w: number,
  h: number,
  xRange: [number, number],
  yRange: [number, number],
  scale = 1,
): Viewport {
  const left = MARGIN.left * scale;
  const top = MARGIN.top * scale;
  return {
    left,
    top,
    width: Math.max(10, w - left - MARGIN.right * scale),
    height: Math.max(10, h - top - MARGIN.bottom * scale),
    xRange,
    yRange,
  };
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

/** Label a tick with just enough decimals for its step. */
export function tickLabel(v: number, step: number): string {
  const decimals = Math.max(0, Math.min(8, -Math.floor(Math.log10(step) + 1e-9)));
  const abs = Math.abs(v);
  if (abs >= 1e5 || (abs > 0 && abs < 1e-4)) return v.toExponential(1).replace("+", "");
  return v.toFixed(decimals).replace("-", "−");
}

/** Zoom a range by `factor` (> 1 zooms out) keeping `anchor` fixed. */
export function zoomRange(range: [number, number], anchor: number, factor: number): [number, number] {
  return [anchor + (range[0] - anchor) * factor, anchor + (range[1] - anchor) * factor];
}
