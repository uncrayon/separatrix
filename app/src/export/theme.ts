// The plot theme of an exported figure, read from the design tokens
// (styles/ds/tokens.json) rather than from the page's CSS, so an export can
// use either palette whatever the app shows. Line weight and glyph size are
// fixed at 1 (the screen's `--line-scale` is 1 too), and the tick labels use
// the `numeric-sm` type token at 100%: projector type scaling is not applied.

import type { PlotTheme } from "../canvas/theme";

export type ExportPalette = "light" | "dark";

/** The parts of tokens.json the plot theme reads. */
export interface DesignTokens {
  color: { tokens: { name: string; value: string | Record<string, string> }[] };
  type: {
    families: Record<string, string>;
    groups: { name: string; family: string; styles: { name: string; fontSize: string; fontWeight?: number }[] }[];
  };
}

/** Root font size the rem tokens are relative to. */
const ROOT_PX = 16;

function remPx(v: string | undefined, fallback: number): number {
  if (!v) return fallback;
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return fallback;
  return v.trim().endsWith("rem") ? n * ROOT_PX : n;
}

export function plotThemeFromTokens(tokens: DesignTokens, mode: ExportPalette): PlotTheme {
  const byName = new Map(tokens.color.tokens.map((t) => [t.name, t.value]));
  const color = (name: string, fallback: string): string => {
    const v = byName.get(name);
    if (typeof v === "string") return v;
    return v?.[mode] ?? fallback;
  };
  const list = (prefix: string, n: number, fallback: string) =>
    Array.from({ length: n }, (_, i) => color(`${prefix}${i + 1}`, fallback));
  let tickSize = 11;
  let weight = 400;
  for (const g of tokens.type.groups) {
    const s = g.styles.find((x) => x.name === "numeric-sm");
    if (s) {
      tickSize = remPx(s.fontSize, 11);
      weight = s.fontWeight ?? 400;
    }
  }
  const mono = tokens.type.families.mono ?? "monospace";
  return {
    bg: color("plot-bg", mode === "light" ? "#ffffff" : "#0a0e10"),
    grid: color("plot-grid", "#edf0f0"),
    axis: color("plot-axis", "#7d888b"),
    tick: color("plot-tick", "#3a4649"),
    field: color("plot-field", "#8a9497"),
    fp: color("plot-fp", "#0b1214"),
    halo: color("plot-halo", "#ffffff"),
    accent: color("accent", "#1d55d0"),
    traj: list("traj-", 7, "#0072b2"),
    nullcline: list("null-", 4, "#6d28d9"),
    lineScale: 1,
    glyphScale: 1,
    tickFont: `${weight} ${tickSize}px ${mono}`,
    tickSize,
  };
}

/** The maths font family from the tokens (axis names). */
export function mathFamily(tokens: DesignTokens): string {
  return tokens.type.families.math ?? "serif";
}
