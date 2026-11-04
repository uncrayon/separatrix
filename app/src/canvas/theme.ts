export interface PlotTheme {
  bg: string;
  grid: string;
  axis: string;
  tick: string;
  field: string;
  fp: string;
  halo: string;
  accent: string;
  traj: string[];
  nullcline: string[];
  lineScale: number;
  glyphScale: number;
  tickFont: string;
  tickSize: number;
}

/** Font size of the `numeric-sm` type token (styles/ds/tokens.json), in rem. */
const NUMERIC_SM_REM = 0.6875;

export function readPlotTheme(probe?: Element | null): PlotTheme {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  const list = (prefix: string, n: number, fallback: string) =>
    Array.from({ length: n }, (_, i) => v(`${prefix}${i + 1}`, fallback));
  const s = parseFloat(v("--line-scale", "1")) || 1;

  let tickFont: string;
  let tickSize: number;
  if (probe) {
    const p = getComputedStyle(probe);
    tickSize = parseFloat(p.fontSize) || 11;
    tickFont = `${p.fontStyle} ${p.fontWeight} ${p.fontSize} ${p.fontFamily}`;
  } else {
    tickSize = NUMERIC_SM_REM * (parseFloat(css.fontSize) || 16);
    tickFont = `400 ${tickSize}px ${v("--font-mono", "monospace")}`;
  }

  return {
    bg: v("--plot-bg", "#0a0e10"),
    grid: v("--plot-grid", "#151c1f"),
    axis: v("--plot-axis", "#5f7076"),
    tick: v("--plot-tick", "#87959a"),
    field: v("--plot-field", "#5b6b71"),
    fp: v("--plot-fp", "#f2f6f7"),
    halo: v("--plot-halo", "#0a0e10"),
    accent: v("--accent", "#7eb0ff"),
    traj: list("--traj-", 7, "#56b4e9"),
    nullcline: list("--null-", 4, "#c4b5fd"),
    lineScale: s,
    glyphScale: 0.6 + 0.4 * s,
    tickFont,
    tickSize,
  };
}
