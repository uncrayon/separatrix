// Reads the design tokens the canvas needs from CSS custom properties, so the
// plot follows tokens.css (and Projector mode) without duplicating colours.

/** Colours and sizes used by the canvas renderer. */
export interface PlotTheme {
  bg: string;
  grid: string;
  axis: string;
  tickText: string;
  field: string;
  fp: string;
  handleRing: string;
  accent: string;
  traj: string[];
  nullcline: string[];
  /** Multiplier for every line width (1 dark, ~1.75 projector). */
  lineScale: number;
  tickFont: string;
}

/** Snapshot the current tokens from <html>. Call again after a theme change. */
export function readPlotTheme(): PlotTheme {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  const list = (prefix: string, n: number, fallback: string) =>
    Array.from({ length: n }, (_, i) => v(`${prefix}${i + 1}`, fallback));
  const tickSize = parseFloat(v("--text-xs", "11px")) || 11;
  return {
    bg: v("--plot-bg", "#0b0d11"),
    grid: v("--plot-grid", "#171b23"),
    axis: v("--plot-axis", "#3a4252"),
    tickText: v("--plot-tick-text", "#7a8191"),
    field: v("--plot-field", "#5a6375"),
    fp: v("--plot-fp", "#f2f4f8"),
    handleRing: v("--plot-handle-ring", "#0b0d11"),
    accent: v("--accent", "#8fb0ff"),
    traj: list("--traj-", 7, "#56b4e9"),
    nullcline: list("--null-", 4, "#c4b5fd"),
    lineScale: parseFloat(v("--line-scale", "1")) || 1,
    tickFont: `${tickSize}px ${v("--font-mono", "monospace")}`,
  };
}
