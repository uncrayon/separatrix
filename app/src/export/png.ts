// The PNG export: the figure drawn on a canvas `scale` times the figure's
// CSS size, with a pHYs chunk at 96·scale dpi so the image keeps its
// physical size when placed in a document. Images are capped at
// 16.7 megapixels (4096 × 4096, what every browser engine can hold); a
// larger request is drawn at the largest scale that fits.
//
// The clipboard gets raw pixels without pHYs (the OS clipboard has no
// physical size), so a pasted 2× image appears at twice the size: the
// dialog says so.

import type { SpaceFigure } from "../canvas/space";
import type { PlotTheme } from "../canvas/theme";

export const PNG_MAX_PIXELS = 4096 * 4096;

export interface PngRaster {
  /** Pixel size of the image. */
  width: number;
  height: number;
  /** Pixels per CSS px actually used. */
  scale: number;
  /** The requested scale did not fit under the cap. */
  reduced: boolean;
}

/** The image size for a figure of `widthPx` × `heightPx` CSS px at `scale`. */
export function pngRaster(widthPx: number, heightPx: number, scale: number): PngRaster {
  const w = Math.max(1, widthPx);
  const h = Math.max(1, heightPx);
  let k = Math.max(0.01, scale);
  let reduced = false;
  if (Math.round(w * k) * Math.round(h * k) > PNG_MAX_PIXELS) {
    k = Math.sqrt(PNG_MAX_PIXELS / (w * h));
    // Rounding up could cross the cap again; step down until it fits.
    while (Math.round(w * k) * Math.round(h * k) > PNG_MAX_PIXELS) k *= 0.999;
    reduced = true;
  }
  return { width: Math.max(1, Math.round(w * k)), height: Math.max(1, Math.round(h * k)), scale: k, reduced };
}

/** Pixels per metre at `dpi` (pHYs unit 1). */
export function pixelsPerMetre(dpi: number): number {
  return Math.round(dpi / 0.0254);
}

let crcTable: Uint32Array | null = null;

export function crc32(bytes: Uint8Array, start = 0, end = bytes.length): number {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      crcTable[n] = c >>> 0;
    }
  }
  let c = 0xffffffff;
  for (let i = start; i < end; i++) c = crcTable[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

const SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];

export interface PngChunk {
  type: string;
  /** Offset of the chunk's length field. */
  offset: number;
  length: number;
}

/** The chunks of a PNG file, in order; throws when it is not one. */
export function pngChunks(png: Uint8Array): PngChunk[] {
  if (png.length < 8 || SIGNATURE.some((b, i) => png[i] !== b)) throw new Error("Not a PNG file.");
  const dv = new DataView(png.buffer, png.byteOffset, png.byteLength);
  const out: PngChunk[] = [];
  let p = 8;
  while (p + 12 <= png.length) {
    const length = dv.getUint32(p);
    const type = String.fromCharCode(png[p + 4], png[p + 5], png[p + 6], png[p + 7]);
    if (p + 12 + length > png.length) throw new Error("Truncated PNG chunk.");
    out.push({ type, offset: p, length });
    p += 12 + length;
    if (type === "IEND") break;
  }
  return out;
}

/** `png` with one pHYs chunk at `dpi`, right after IHDR (any earlier pHYs is replaced). */
export function withPhys(png: Uint8Array, dpi: number): Uint8Array {
  const chunks = pngChunks(png);
  if (chunks[0]?.type !== "IHDR") throw new Error("PNG without IHDR.");
  const ppm = pixelsPerMetre(dpi);
  const phys = new Uint8Array(21);
  const dv = new DataView(phys.buffer);
  dv.setUint32(0, 9);
  phys.set([0x70, 0x48, 0x59, 0x73], 4); // "pHYs"
  dv.setUint32(8, ppm);
  dv.setUint32(12, ppm);
  phys[16] = 1; // unit: metre
  dv.setUint32(17, crc32(phys, 4, 17));
  const parts: Uint8Array[] = [png.subarray(0, 8)];
  for (const c of chunks) {
    if (c.type === "pHYs") continue;
    parts.push(png.subarray(c.offset, c.offset + 12 + c.length));
    if (c.type === "IHDR") parts.push(phys);
  }
  const total = parts.reduce((n, a) => n + a.length, 0);
  const out = new Uint8Array(total);
  let o = 0;
  for (const a of parts) {
    out.set(a, o);
    o += a.length;
  }
  return out;
}

/** The pHYs resolution of `png` in dpi (rounded), or null. */
export function physDpi(png: Uint8Array): number | null {
  const c = pngChunks(png).find((x) => x.type === "pHYs");
  if (!c) return null;
  const dv = new DataView(png.buffer, png.byteOffset, png.byteLength);
  if (png[c.offset + 16] !== 1) return null;
  return Math.round(dv.getUint32(c.offset + 8) * 0.0254);
}

/**
 * Draws on a fresh canvas of `raster` pixels, scaled so `draw` works in CSS
 * px, and returns the PNG bytes with pHYs (browser only).
 */
export async function renderPng(
  raster: PngRaster,
  draw: (ctx: CanvasRenderingContext2D) => void,
  physical = true,
): Promise<Uint8Array> {
  const canvas = document.createElement("canvas");
  canvas.width = raster.width;
  canvas.height = raster.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("This window can't draw the image. Restart Separatrix, then export again; or use SVG instead.");
  ctx.setTransform(raster.scale, 0, 0, raster.scale, 0, 0);
  draw(ctx);
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) throw new Error("The PNG image couldn't be encoded. Try a smaller size or scale, or use SVG instead.");
  const bytes = new Uint8Array(await blob.arrayBuffer());
  return physical ? withPhys(bytes, 96 * raster.scale) : bytes;
}

/**
 * The PNG drawing of a 3D figure at `scale` (three.js offscreen, or 2D lines
 * without WebGL). three.js comes with the 3D view's own chunk, loaded here.
 */
export async function spacePngDraw(
  fig: SpaceFigure,
  w: number,
  h: number,
  scale: number,
  th: PlotTheme,
  background: boolean,
): Promise<(ctx: CanvasRenderingContext2D) => void> {
  const { renderSpaceFigure } = await import("../canvas/SpaceCanvas");
  return (ctx) => renderSpaceFigure(ctx, w, h, scale, fig, th, background);
}
