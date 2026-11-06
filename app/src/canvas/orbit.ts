import type { OrbitDiagramResponse } from "../api/types";
import { drawScene, type Box, type DrawContext, type Scene } from "./draw";
import type { PlotTheme } from "./theme";
import { sx, type Viewport } from "./viewport";

export interface RasterContext extends DrawContext {
  imageSmoothingEnabled: boolean;
  drawImage(image: CanvasImageSource, dx: number, dy: number, dw: number, dh: number): void;
}

/**
 * The canvas x (CSS pixels) under `clientX`. The browser puts the canvas's
 * first device pixel at the rounded left edge, so a fractional
 * `rectLeft` must be rounded the same way or a click lands one column off
 * the density drawn under it.
 */
export function canvasX(clientX: number, rectLeft: number, pixelRatio: number): number {
  return clientX - Math.round(rectLeft * pixelRatio) / pixelRatio;
}

/** The center (CSS pixels) of the device-pixel column that holds parameter `r`. */
export function markerX(vp: Viewport, r: number, pixelRatio: number): number {
  return vp.left + (Math.floor((sx(vp, r) - vp.left) * pixelRatio) + 0.5) / pixelRatio;
}

export interface OrbitHistogram {
  width: number;
  height: number;
  counts: Uint32Array;
  max: number;
}

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

const MIN_ALPHA = 0.3;

export function densityAlpha(count: number, max: number): number {
  if (count <= 0) return 0;
  if (max <= 1) return 1;
  return MIN_ALPHA + (1 - MIN_ALPHA) * (Math.log(count) / Math.log(max));
}

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

function cssRgb(color: string): [number, number, number] {
  const probe = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  if (!probe) return [128, 128, 128];
  probe.fillStyle = color;
  probe.fillRect(0, 0, 1, 1);
  const [r, g, b] = probe.getImageData(0, 0, 1, 1).data;
  return [r, g, b];
}

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
  pixelRatio?: number;
  marker?: number | null;
  avoid?: Box[];
}

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
    const x = markerX(vp, m, opts.pixelRatio ?? 1);
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = th.lineScale;
    ctx.beginPath();
    ctx.moveTo(x, vp.top);
    ctx.lineTo(x, vp.top + vp.height);
    ctx.stroke();
  }
  ctx.restore();
}
