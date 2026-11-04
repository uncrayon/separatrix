// Number and notation helpers shared by the panels and the canvas.

import type { AxisRef, Classification, SymbolInfo, SystemKind } from "../api/types";

/**
 * Format a number for display with `digits` significant digits, dropping
 * trailing zeros ("2.50" → "2.5") and avoiding "-0".
 */
export function fmt(v: number, digits = 3): string {
  if (!Number.isFinite(v)) return Number.isNaN(v) ? "NaN" : v > 0 ? "∞" : "−∞";
  if (Math.abs(v) < 1e-12) return "0";
  const abs = Math.abs(v);
  if (abs >= 1e5 || abs < 1e-4) {
    const [m, e] = v.toExponential(digits - 1).split("e");
    return `${trimZeros(m)}e${e.replace("+", "")}`;
  }
  // Fixed notation with `digits` significant digits, at least 0 decimals.
  const decimals = Math.max(0, digits - 1 - Math.floor(Math.log10(abs)));
  return trimZeros(v.toFixed(Math.min(decimals, 10)));
}

function trimZeros(s: string): string {
  return s.includes(".") ? s.replace(/\.?0+$/, "") : s;
}

/** Like {@link fmt} but as LaTeX (scientific notation as `\times 10^{k}`). */
export function fmtTex(v: number, digits = 3): string {
  const s = fmt(v, digits);
  const m = /^(-?[\d.]+)e(-?\d+)$/.exec(s);
  if (m) return `${m[1]}\\times 10^{${m[2]}}`;
  if (s === "∞") return "\\infty";
  if (s === "−∞") return "-\\infty";
  return s;
}

/** A complex eigenvalue as LaTeX, e.g. `-0.5 + 1.32i`. */
export function complexTex([re, im]: [number, number], digits = 3): string {
  if (Math.abs(im) < 1e-12) return fmtTex(re, digits);
  const sign = im < 0 ? "-" : "+";
  const reS = Math.abs(re) < 1e-12 ? "" : `${fmtTex(re, digits)} ${sign} `;
  const imS = `${fmtTex(Math.abs(im), digits)}i`;
  return reS ? reS + imS : (im < 0 ? "-" : "") + imS;
}

const GREEK: Record<string, string> = {
  alpha: "α",
  beta: "β",
  gamma: "γ",
  delta: "δ",
  epsilon: "ϵ",
  varepsilon: "ε",
  zeta: "ζ",
  eta: "η",
  theta: "θ",
  vartheta: "ϑ",
  iota: "ι",
  kappa: "κ",
  lambda: "λ",
  mu: "μ",
  nu: "ν",
  xi: "ξ",
  pi: "π",
  varpi: "ϖ",
  rho: "ρ",
  varrho: "ϱ",
  sigma: "σ",
  varsigma: "ς",
  tau: "τ",
  upsilon: "υ",
  phi: "ϕ",
  varphi: "φ",
  chi: "χ",
  psi: "ψ",
  omega: "ω",
  Gamma: "Γ",
  Delta: "Δ",
  Theta: "Θ",
  Lambda: "Λ",
  Xi: "Ξ",
  Pi: "Π",
  Sigma: "Σ",
  Upsilon: "Υ",
  Phi: "Φ",
  Psi: "Ψ",
  Omega: "Ω",
};

/**
 * Turn short LaTeX (names, dotted names, simple expressions) into readable
 * plain Unicode. Used where typesetting is not possible: diagnostics text,
 * accessible names and canvas tick labels. `\dot{\theta}` → "θ̇".
 */
export function latexToPlain(s: string): string {
  let out = s;
  out = out.replace(/\\ddot\{([^{}]*)\}/g, (_, a: string) => `${latexToPlain(a)}̈`);
  out = out.replace(/\\dot\{([^{}]*)\}/g, (_, a: string) => `${latexToPlain(a)}̇`);
  out = out.replace(/\\(?:operatorname|mathrm|text)\{([^{}]*)\}/g, "$1");
  out = out.replace(/\\frac\{([^{}]*)\}\{([^{}]*)\}/g, "($1)/($2)");
  out = out.replace(/\\left|\\right/g, "");
  out = out.replace(/\\cdot/g, "·").replace(/\\times/g, "×");
  out = out.replace(/\\([A-Za-z]+)/g, (m, name: string) => GREEK[name] ?? (name.length > 1 ? name : m));
  out = out.replace(/_\{([^{}]*)\}/g, "_$1").replace(/\^\{([^{}]*)\}/g, "^$1");
  out = out.replace(/-/g, "−");
  return out;
}

/**
 * A core diagnostic as the panel shows it: plain maths, and a quoted token in curly quotes
 * ("Missing right-hand side after ‘=’", as in the brand book). Apostrophes ("can't", "x'") stay.
 */
export function diagnosticText(message: string): string {
  return latexToPlain(message).replace(/'([^'\s]{1,12})'/g, "‘$1’");
}

/** The LaTeX used to label an axis: the user's state name, or t / n. */
export function axisLatex(axis: AxisRef, states: SymbolInfo[], kind: SystemKind | undefined): string {
  if (axis.kind === "time") return kind === "map" ? "n" : "t";
  return states[axis.index]?.latex ?? "?";
}

/** An axis name as the plot labels it: {@link axisLatex}, with `_n` for a map value plotted against n. */
export function axisLabelLatex(
  axis: AxisRef,
  other: AxisRef,
  states: SymbolInfo[],
  kind: SystemKind | undefined,
): string {
  const base = axisLatex(axis, states, kind);
  return kind === "map" && axis.kind === "state" && other.kind === "time" ? `${base}_n` : base;
}

/** Same as {@link axisLatex}, for a map state on the value axis: `x_n`. */
export function stateLatexForKind(s: SymbolInfo, kind: SystemKind | undefined): string {
  return kind === "map" ? `${s.latex}_n` : s.latex;
}

/** Stable comparison of two axis references. */
export function sameAxis(a: AxisRef, b: AxisRef): boolean {
  return a.kind === b.kind && (a.kind === "time" || a.index === (b as { index: number }).index);
}

/**
 * Classification in Strogatz's words, capitalised for headings (US spelling,
 * as the brand book asks: "Center", "Saddle"). In 1D Strogatz just says
 * "stable" or "unstable" (Ch. 2), and "half-stable" when f′(x*) = 0.
 */
export function classificationTitle(c: Classification, dim = 2): string {
  if (dim === 1) {
    if (c === "non-hyperbolic" || c === "non-isolated") return "Half-stable";
    const stable = c === "sink" || c.startsWith("stable");
    return stable ? "Stable" : "Unstable";
  }
  const words: Record<Classification, string> = {
    saddle: "Saddle",
    "stable node": "Stable node",
    "unstable node": "Unstable node",
    "stable spiral": "Stable spiral",
    "unstable spiral": "Unstable spiral",
    center: "Center",
    "stable star": "Stable star",
    "unstable star": "Unstable star",
    "stable degenerate node": "Stable degenerate node",
    "unstable degenerate node": "Unstable degenerate node",
    "non-isolated": "Non-isolated",
    sink: "Sink",
    source: "Source",
    "saddle (n-D)": "Saddle",
    "non-hyperbolic": "Non-hyperbolic",
  };
  return words[c];
}

/** One sentence explaining what the classification means for nearby trajectories. */
export function classificationExplain(c: Classification, kind: SystemKind | undefined, dim = 2): string {
  if (dim === 1) {
    const stable = c === "sink" || c.startsWith("stable");
    const unstable = c === "source" || c.startsWith("unstable");
    if (kind === "map") {
      if (stable) return "|f′(x*)| < 1: nearby orbits are pulled in.";
      if (unstable) return "|f′(x*)| > 1: nearby orbits are pushed away.";
      return "|f′(x*)| = 1: linearization cannot decide stability.";
    }
    if (stable) return "f′(x*) < 0: nearby solutions flow toward it.";
    if (unstable) return "f′(x*) > 0: nearby solutions flow away from it.";
    return "f′(x*) = 0: linearization cannot decide; look at the sign of f on each side.";
  }
  if (kind === "map") {
    switch (c) {
      case "sink":
      case "stable node":
      case "stable spiral":
        return "Every multiplier has |μ| < 1, so nearby orbits are pulled in.";
      case "source":
      case "unstable node":
      case "unstable spiral":
        return "Every multiplier has |μ| > 1, so nearby orbits are pushed away.";
      case "saddle":
      case "saddle (n-D)":
        return "Some multipliers have |μ| > 1 and some |μ| < 1: orbits come in along one direction and leave along another.";
      default:
        return "A multiplier has |μ| = 1, so linearization cannot decide stability.";
    }
  }
  switch (c) {
    case "saddle":
    case "saddle (n-D)":
      return "Eigenvalues of opposite sign: trajectories approach along the stable manifold and leave along the unstable one.";
    case "stable node":
      return "Real negative eigenvalues: nearby trajectories approach without circling.";
    case "unstable node":
      return "Real positive eigenvalues: nearby trajectories move away without circling.";
    case "stable spiral":
      return "Complex eigenvalues with negative real part: trajectories spiral in.";
    case "unstable spiral":
      return "Complex eigenvalues with positive real part: trajectories spiral out.";
    case "center":
      return "Purely imaginary eigenvalues: in the linearization, trajectories are closed orbits.";
    case "stable star":
    case "unstable star":
      return "Two equal eigenvalues with two independent eigenvectors: trajectories are straight lines.";
    case "stable degenerate node":
    case "unstable degenerate node":
      return "Repeated eigenvalue with only one eigenvector: trajectories become parallel to it.";
    case "non-isolated":
      return "A zero eigenvalue: this point sits on a line or plane of fixed points.";
    case "sink":
      return "Every eigenvalue has negative real part: all nearby trajectories approach.";
    case "source":
      return "Every eigenvalue has positive real part: all nearby trajectories leave.";
    case "non-hyperbolic":
      return "An eigenvalue on the imaginary axis: linearization cannot decide stability.";
  }
}

/** Clamp `v` to `[lo, hi]`. */
export function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

/**
 * The first-order system for display.
 *
 * When a second-order equation was rewritten, the core names the new state
 * `\dot{θ}` and returns `\dot{θ} = \dot{θ}` for the first row, which reads as
 * a tautology. In that case every left-hand side is written as d/dt of its
 * state instead (Strogatz's x' = y, y' = … with y = θ̇):
 *   dθ/dt = θ̇,   dθ̇/dt = −sin θ.
 * Systems without derived states (and maps) are returned unchanged.
 */
export function firstOrderDisplay(
  equationsLatex: string[],
  states: SymbolInfo[],
  kind: SystemKind | undefined,
): string[] {
  const derived = states.some((s) => /^\\dot\{/.test(s.latex));
  if (kind === "map" || !derived || equationsLatex.length !== states.length) return equationsLatex;
  return equationsLatex.map((eq, i) => {
    const k = eq.indexOf("=");
    if (k < 0) return eq;
    return `\\frac{d${states[i].latex}}{dt} =${eq.slice(k + 1)}`;
  });
}
