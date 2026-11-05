// The design tokens for exports, imported once for the browser bundle.
// (theme.ts stays free of JSON imports so the node tests can load it.)

import raw from "../styles/ds/tokens.json";
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
