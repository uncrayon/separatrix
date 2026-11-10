// The design tokens for exports, imported once for the browser bundle.
// (theme.ts stays free of JSON imports so the node tests can load it.)

import raw from "../styles/ds/tokens.json";
import { CURVE_STYLES, type CurveStyleKey } from "../canvas/curves";
import { DEFAULT_FIGURE_FONTS, type FigureFonts } from "../canvas/figure";
import type { PlotTheme } from "../canvas/theme";
import { mathFamily, plotThemeFromTokens, type DesignTokens, type ExportPalette } from "./theme";

export const TOKENS = raw as unknown as DesignTokens;

const cache = new Map<ExportPalette, PlotTheme>();

/** The plot theme for an export in `palette`. */
export function exportTheme(palette: ExportPalette): PlotTheme {
  let th = cache.get(palette);
  if (!th) {
    th = plotThemeFromTokens(TOKENS, palette);
    cache.set(palette, th);
  }
  return th;
}

export const EXPORT_FONTS: FigureFonts = { ...DEFAULT_FIGURE_FONTS, mathFamily: mathFamily(TOKENS) };

/** The parameter plane's curve colors for an export in `palette`, from the `--curve-*` tokens. */
export function exportCurveColors(palette: ExportPalette): Record<CurveStyleKey, string> {
  const byName = new Map(TOKENS.color.tokens.map((t) => [t.name, t.value]));
  const color = (token: string) => {
    const v = byName.get(token.replace(/^--/, ""));
    return (typeof v === "string" ? v : v?.[palette]) ?? "#888888";
  };
  return Object.fromEntries(Object.entries(CURVE_STYLES).map(([k, s]) => [k, color(s.token)])) as Record<CurveStyleKey, string>;
}
