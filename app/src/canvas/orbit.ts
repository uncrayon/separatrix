// The orbit diagram (Strogatz §10.2, Figure 10.2.7): for each parameter value
// across, the iterates of one state after the transient, binned per device
// pixel and drawn as a density, darker where the orbit visits more often,
// with a vertical marker at the parameter's current value. Drawn through any
// DrawContext that can also draw an image.

import type { OrbitDiagramResponse } from "../api/types";
import { drawScene, type Box, type DrawContext, type Scene } from "./draw";
import type { PlotTheme } from "./theme";
import { sx, type Viewport } from "./viewport";

/** A DrawContext that can paint a raster, as a canvas's 2D context does. */
export interface RasterContext extends DrawContext {
  imageSmoothingEnabled: boolean;
  drawImage(image: CanvasImageSource, dx: number, dy: number, dw: number, dh: number): void;
}

export interface OrbitHistogram {
  width: number;
  height: number;
  /** Iterates per pixel, row by row from the top. */
  counts: Uint32Array;
  max: number;
}

/**
 * The diagram's iterates binned into a `width × height` grid over `vp`'s
 * ranges: column by r, row by x. Iterates outside the ranges, and the null
 * (escaped) ones, are left out.
 */
export function orbitHistogram(
  res: Pick<OrbitDiagramResponse, "r" | "x">,
  vp: Pick<Viewport, "xRange" | "yRange">,
  width: number,
  height: number,
): OrbitHistogram {
  const counts = new Uint32Array(width * height);
  const columns = res.r.length;
  const kept = columns > 0 ? res.x.length / columns : 0;
  const [x0, x1] = vp.xRange;
  const [y0, y1] = vp.yRange;
  const kx = width / (x1 - x0);
  const ky = height / (y1 - y0);
  let max = 0;
  for (let j = 0; j < columns; j++) {
    const bx = Math.floor((res.r[j] - x0) * kx);
    if (!(bx >= 0 && bx < width)) continue;
    for (let k = j * kept, end = k + kept; k < end; k++) {
      const v = res.x[k];
      if (v === null) continue;
      const by = Math.floor((y1 - v) * ky);
      if (!(by >= 0 && by < height)) continue;
      const i = by * width + bx;
      const c = ++counts[i];
      if (c > max) max = c;
    }
  }
  return { width, height, counts, max };
}

/** The faintest pixel's opacity: one visit in a column still reads on both themes. */
const MIN_ALPHA = 0.3;

/** A pixel visited `count` times, of `max` at most: transparent when 0, then log-scaled from {@link MIN_ALPHA} to 1. */
export function densityAlpha(count: number, max: number): number {
  if (count <= 0) return 0;
  if (max <= 1) return 1;
  return MIN_ALPHA + (1 - MIN_ALPHA) * (Math.log(count) / Math.log(max));
}

/** RGBA pixels of `h` in the colour `rgb`, opacity from {@link densityAlpha}. */
export function densityPixels(h: OrbitHistogram, rgb: readonly [number, number, number]): Uint8ClampedArray<ArrayBuffer> {
  const out = new Uint8ClampedArray(h.counts.length * 4);
  for (let i = 0; i < h.counts.length; i++) {
    const c = h.counts[i];
    if (c === 0) continue;
    out[4 * i] = rgb[0];
    out[4 * i + 1] = rgb[1];
    out[4 * i + 2] = rgb[2];
    out[4 * i + 3] = Math.round(255 * densityAlpha(c, h.max));
  }
  return out;
}

/** A CSS colour as RGB, read back from a 1-pixel canvas (any syntax the browser takes). */
function cssRgb(color: string): [number, number, number] {
  const probe = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  if (!probe) return [128, 128, 128];
  probe.fillStyle = color;
  probe.fillRect(0, 0, 1, 1);
  const [r, g, b] = probe.getImageData(0, 0, 1, 1).data;
  return [r, g, b];
}

/** A scene with nothing in it but the frame: grid, axes and ticks, the labels clear of `avoid`. */
function frameOnly(avoid: Box[]): Scene {
  return {
    kind: "map",
    dim: 0,
    xAxis: { kind: "state", index: 0 },
    yAxis: { kind: "state", index: 0 },
    arrows: [],
    fieldGrid: 0,
    nullclines: [],
    fixedPoints: [],
    trajectories: [],
    ics: [],
    selectedIc: null,
    selectedFp: null,
    hoverIc: null,
    hoverFp: null,
    eigenvectors: false,
    stale: false,
    avoid: { labels: avoid, field: [] },
  };
}

export interface OrbitDrawOptions {
  /** Device pixels per CSS pixel: the density is binned per device pixel. */
  pixelRatio?: number;
  /** The swept parameter's current value, marked by a vertical line. */
  marker?: number | null;
  /** Boxes the tick labels keep clear of (the axis names, a toolbar). */
  avoid?: Box[];
}

/**
 * Draws the grid, axes and ticks of `vp`, then the diagram `res` as a
 * density in `plot-fp` over them (the plot's z-order puts data above the
 * frame), then the marker at the parameter's value in `accent`. The caller
 * fills the background.
 */
export function drawOrbit(
  ctx: RasterContext,
  vp: Viewport,
  res: Pick<OrbitDiagramResponse, "r" | "x"> | null,
  th: PlotTheme,
  opts: OrbitDrawOptions = {},
): void {
  drawScene(ctx, vp.left + vp.width, vp.top + vp.height, vp, frameOnly(opts.avoid ?? []), th, { background: false });
  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();
  if (res && res.r.length > 0) {
    const ratio = opts.pixelRatio ?? 1;
    const w = Math.max(1, Math.round(vp.width * ratio));
    const h = Math.max(1, Math.round(vp.height * ratio));
    const hist = orbitHistogram(res, vp, w, h);
    const image = document.createElement("canvas");
    image.width = w;
    image.height = h;
    image.getContext("2d")?.putImageData(new ImageData(densityPixels(hist, cssRgb(th.fp)), w, h), 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(image, vp.left, vp.top, vp.width, vp.height);
  }
  const m = opts.marker;
  if (m !== null && m !== undefined && Number.isFinite(m) && m >= vp.xRange[0] && m <= vp.xRange[1]) {
    const x = Math.round(sx(vp, m)) + 0.5;
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = th.lineScale;
    ctx.beginPath();
    ctx.moveTo(x, vp.top);
    ctx.lineTo(x, vp.top + vp.height);
    ctx.stroke();
  }
  ctx.restore();
}
