// The export dialog's options. They are remembered per project file, in its
// `export_settings` (opaque to Rust), so `readExportOptions` accepts any
// value and keeps only what it understands.

import type { ExportPalette } from "./theme";

export type ExportFormat = "latex" | "svg" | "png" | "csv";
export type LatexContent = "figure" | "spectrum" | "specialPoints" | "equations" | "fixedPoints";
export type Notation = "dot" | "prime" | "leibniz";
export type PngScale = 2 | 3 | 4;

export interface ExportOptions {
  format: ExportFormat;
  latexContent: LatexContent;
  /** Figure size for LaTeX and SVG, cm. */
  widthCm: number;
  heightCm: number;
  /** PNG size in CSS px (the image has `pngScale` times as many pixels). */
  pngWidthPx: number;
  pngHeightPx: number;
  pngScale: PngScale;
  /** Palette of figures; the projector (light) one by default. */
  palette: ExportPalette;
  field: boolean;
  nullclines: boolean;
  fixedPoints: boolean;
  standalone: boolean;
  /** SVG and PNG: leave the background out. */
  transparent: boolean;
  notation: Notation;
  numbered: boolean;
  /** Significant digits in the fixed-point table, 2 to 8. */
  digits: number;
  /** CSV: the initial conditions to write, by project id; null for all. */
  csvIcIds: string[] | null;
  /** CSV samples per run; 0 for the screen's. */
  csvSamples: number;
}

export const SIZE_MIN_CM = 1;
export const SIZE_MAX_CM = 40;
export const PNG_MIN_PX = 40;
export const PNG_MAX_PX = 4000;
export const CSV_MAX_SAMPLES = 200000;
export const PX_PER_CM = 96 / 2.54;

export const DEFAULT_EXPORT_OPTIONS: ExportOptions = {
  format: "latex",
  latexContent: "figure",
  widthCm: 8,
  heightCm: 6.4,
  pngWidthPx: 800,
  pngHeightPx: 640,
  pngScale: 2,
  palette: "light",
  field: true,
  nullclines: true,
  fixedPoints: true,
  standalone: false,
  transparent: false,
  notation: "dot",
  numbered: true,
  digits: 4,
  csvIcIds: null,
  csvSamples: 0,
};

/** "Use a width between 1 and 40 cm." for a size out of range, else null. */
export function sizeError(v: number, which: "width" | "height"): string | null {
  return Number.isFinite(v) && v >= SIZE_MIN_CM && v <= SIZE_MAX_CM
    ? null
    : `Use a ${which} between ${SIZE_MIN_CM} and ${SIZE_MAX_CM} cm.`;
}

export function pngSizeError(v: number, which: "width" | "height"): string | null {
  return Number.isFinite(v) && v >= PNG_MIN_PX && v <= PNG_MAX_PX && Number.isInteger(v)
    ? null
    : `Use a whole ${which} between ${PNG_MIN_PX} and ${PNG_MAX_PX} px.`;
}

function pick<T extends string>(v: unknown, allowed: readonly T[], fallback: T): T {
  return typeof v === "string" && (allowed as readonly string[]).includes(v) ? (v as T) : fallback;
}

function bool(v: unknown, fallback: boolean): boolean {
  return typeof v === "boolean" ? v : fallback;
}

function inRange(v: unknown, lo: number, hi: number, fallback: number, integer = false): number {
  if (typeof v !== "number" || !Number.isFinite(v) || v < lo || v > hi) return fallback;
  return integer ? Math.round(v) : v;
}

/** Options from a project's `export_settings`, falling back field by field to the defaults. */
export function readExportOptions(v: unknown): ExportOptions {
  const d = DEFAULT_EXPORT_OPTIONS;
  if (!v || typeof v !== "object" || Array.isArray(v)) return { ...d };
  const o = v as Record<string, unknown>;
  const scale = o.pngScale;
  const ids = Array.isArray(o.csvIcIds) && o.csvIcIds.every((x) => typeof x === "string") ? [...o.csvIcIds] : null;
  return {
    format: pick(o.format, ["latex", "svg", "png", "csv"], d.format),
    latexContent: pick(o.latexContent, ["figure", "spectrum", "specialPoints", "equations", "fixedPoints"], d.latexContent),
    widthCm: inRange(o.widthCm, SIZE_MIN_CM, SIZE_MAX_CM, d.widthCm),
    heightCm: inRange(o.heightCm, SIZE_MIN_CM, SIZE_MAX_CM, d.heightCm),
    pngWidthPx: inRange(o.pngWidthPx, PNG_MIN_PX, PNG_MAX_PX, d.pngWidthPx, true),
    pngHeightPx: inRange(o.pngHeightPx, PNG_MIN_PX, PNG_MAX_PX, d.pngHeightPx, true),
    pngScale: scale === 2 || scale === 3 || scale === 4 ? scale : d.pngScale,
    palette: pick(o.palette, ["light", "dark"], d.palette),
    field: bool(o.field, d.field),
    nullclines: bool(o.nullclines, d.nullclines),
    fixedPoints: bool(o.fixedPoints, d.fixedPoints),
    standalone: bool(o.standalone, d.standalone),
    transparent: bool(o.transparent, d.transparent),
    notation: pick(o.notation, ["dot", "prime", "leibniz"], d.notation),
    numbered: bool(o.numbered, d.numbered),
    digits: inRange(o.digits, 2, 8, d.digits, true),
    csvIcIds: ids,
    csvSamples: inRange(o.csvSamples, 0, CSV_MAX_SAMPLES, d.csvSamples, true),
  };
}

/**
 * The `export_settings` to store after the dialog's options change: the
 * options merged onto what the file had, so keys this version doesn't know
 * (a newer Separatrix's, say) survive the save.
 */
export function mergeExportSettings(stored: unknown, options: ExportOptions): Record<string, unknown> {
  const base = stored && typeof stored === "object" && !Array.isArray(stored) ? (stored as Record<string, unknown>) : {};
  return { ...base, ...options };
}

/** The figure size in CSS px for the current format (cm for LaTeX and SVG, px for PNG). */
export function figurePx(o: ExportOptions): { width: number; height: number } {
  return o.format === "png"
    ? { width: o.pngWidthPx, height: o.pngHeightPx }
    : { width: o.widthCm * PX_PER_CM, height: o.heightCm * PX_PER_CM };
}

/** A file stem from a title: "Rabbits vs sheep" → "rabbits-vs-sheep". */
export function fileStem(title: string): string {
  const s = title
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return s || "figure";
}
