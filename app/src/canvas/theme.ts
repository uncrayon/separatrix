// Reads the design tokens the canvas needs from CSS custom properties, so the
// plot follows the design system (styles/ds/tokens.css) and Projector mode
// without duplicating colours. Read again on every theme change: never cache
// colours across one (PhasePlane README, "Read every colour at draw time").

/** Colours, scales and fonts used by the canvas renderer. */
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
  /** `s`: multiplier for every line width (1 on the chalkboard, 1.75 in projector mode). */
  lineScale: number;
  /** `g` = 0.6 + 0.4 × s: the gentler scale for glyphs, heads and handles (1 and 1.3). */
  glyphScale: number;
  /** Tick labels: the `numeric-sm` type style (Atkinson Hyperlegible Mono). */
  tickFont: string;
  /** Line height of the tick font in CSS px (for label boxes). */
  tickSize: number;
}

/**
 * Snapshot the current tokens. `probe` is an element carrying the `numeric-sm`
 * class: its computed font is the tick font, so projector mode (115% root)
 * grows the labels with the rest of the type.
 */
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
    // numeric-sm is 0.6875rem in the mono face; rem follows the root size.
    tickSize = 0.6875 * (parseFloat(css.fontSize) || 16);
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
