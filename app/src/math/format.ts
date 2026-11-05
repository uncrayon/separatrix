import type { AxisRef, Classification, StateInfo, SystemKind } from "../api/types";

export const COMPUTED_ZERO = 1e-12;

export function fmt(v: number, digits = 3, zeroBelow = 0): string {
  if (!Number.isFinite(v)) return Number.isNaN(v) ? "NaN" : v > 0 ? "∞" : "−∞";
  if (Math.abs(v) < zeroBelow || v === 0) return "0";
  const abs = Math.abs(v);
  if (abs >= 1e5 || abs < 1e-4) {
    const [m, e] = v.toExponential(digits - 1).split("e");
    return `${trimZeros(m)}e${e.replace("+", "")}`;
  }
  const decimals = Math.max(0, digits - 1 - Math.floor(Math.log10(abs)));
  return trimZeros(v.toFixed(Math.min(decimals, 10)));
}

function trimZeros(s: string): string {
  return s.includes(".") ? s.replace(/\.?0+$/, "") : s;
}

export function fmtTex(v: number, digits = 3, zeroBelow = 0): string {
  const s = fmt(v, digits, zeroBelow);
  const m = /^(-?[\d.]+)e(-?\d+)$/.exec(s);
  if (m) return `${m[1]}\\times 10^{${m[2]}}`;
  if (s === "∞") return "\\infty";
  if (s === "−∞") return "-\\infty";
  return s;
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

/** The Greek letters the parser accepts, by command name (`varrho`, `Upsilon`, …). */
export const GREEK_NAMES: ReadonlySet<string> = new Set(Object.keys(GREEK));

/** LaTeX as plain text, keeping every hyphen as typed. */
function latexToPlainText(s: string): string {
  let out = s;
  out = out.replace(/\\ddot\{([^{}]*)\}/g, (_, a: string) => `${latexToPlainText(a)}̈`);
  out = out.replace(/\\dot\{([^{}]*)\}/g, (_, a: string) => `${latexToPlainText(a)}̇`);
  out = out.replace(/\\(?:operatorname|mathrm|text)\{([^{}]*)\}/g, "$1");
  out = out.replace(/\\frac\{([^{}]*)\}\{([^{}]*)\}/g, "($1)/($2)");
  out = out.replace(/\\left|\\right/g, "");
  out = out.replace(/\\cdot/g, "·").replace(/\\times/g, "×");
  out = out.replace(/\\([A-Za-z]+)/g, (m, name: string) => GREEK[name] ?? (name.length > 1 ? name : m));
  out = out.replace(/_\{([^{}]*)\}/g, "_$1").replace(/\^\{([^{}]*)\}/g, "^$1");
  return out;
}

/** A LaTeX expression (a name, an axis, an equation) as plain text, with true minus signs. */
export function latexToPlain(s: string): string {
  return latexToPlainText(s).replace(/-/g, "−");
}

/**
 * A diagnostic from the core for display. Messages are prose that quote
 * maths (`\dot{x} = -x`, `'*'`), so LaTeX is spelled out but hyphens stay
 * hyphens ("right-hand side"). A token in single quotes gets curly quotes;
 * a prime is not a quote (`x' + y'` stays as it is).
 */
export function diagnosticText(message: string): string {
  return latexToPlainText(message).replace(/(^|[\s(])'([^'\s]{1,12})'(?=$|[\s,.;:)])/g, "$1‘$2’");
}

export function axisLatex(axis: AxisRef, states: StateInfo[], kind: SystemKind | undefined): string {
  if (axis.kind === "time") return kind === "map" ? "n" : "t";
  return states[axis.index]?.latex ?? "?";
}

export function axisLabelLatex(
  axis: AxisRef,
  other: AxisRef,
  states: StateInfo[],
  kind: SystemKind | undefined,
): string {
  const base = axisLatex(axis, states, kind);
  return kind === "map" && axis.kind === "state" && other.kind === "time" ? withSubscript(base, "n") : base;
}

export function viewAxisLabels(
  view: { kind?: string; xAxis: AxisRef; yAxis: AxisRef },
  states: StateInfo[],
  kind: SystemKind | undefined,
): { x: string; y: string } {
  if (view.kind === "cobweb") {
    const base = axisLatex(view.xAxis, states, kind);
    return { x: withSubscript(base, "n"), y: withSubscript(base, "n+1") };
  }
  if (view.kind === "section" && sameAxis(view.xAxis, view.yAxis)) {
    const base = axisLatex(view.xAxis, states, kind);
    return { x: withSubscript(base, "k"), y: withSubscript(base, "k+1") };
  }
  return {
    x: axisLabelLatex(view.xAxis, view.yAxis, states, kind),
    y: axisLabelLatex(view.yAxis, view.xAxis, states, kind),
  };
}

/** The multiplier λ = f′(x*) of a 1-D map's fixed point: its only eigenvalue. */
export function mapMultiplier(fp: { eigenvalues: [number, number][] }): number {
  return fp.eigenvalues[0]?.[0] ?? NaN;
}

/** A multiplier this close to 0 is λ = 0 (superstable). */
const SUPERSTABLE_TOL = 1e-12;

/**
 * Why a 1-D map's fixed point is stable or not, in Strogatz §10.1's words:
 * |λ| against 1, then what the sign of λ says about nearby orbits. `name`
 * is the state's plain name (`x`).
 */
export function mapStabilityHint(fp: { eigenvalues: [number, number][]; classification: Classification }, name = "x"): string {
  const lambda = mapMultiplier(fp);
  const star = `${name}*`;
  if (fp.classification === "non-isolated")
    return `f(${name}) = ${name} for every ${name} between the two non-isolated points: ${star} is not isolated.`;
  if (fp.eigenvalues.length === 0)
    return `f is not differentiable at ${star} (its slopes on the two sides differ), so there is no λ = f′(${star}) and linearization does not decide.`;
  const parts: string[] = [];
  if (fp.classification === "non-hyperbolic") parts.push("|λ| = 1: linearization does not decide.");
  else if (fp.classification === "sink" || fp.classification.startsWith("stable")) parts.push(`|λ| < 1, so ${star} is stable.`);
  else parts.push(`|λ| > 1, so ${star} is unstable.`);
  if (Math.abs(lambda) < SUPERSTABLE_TOL) parts.push("λ = 0: superstable.");
  else if (lambda < 0) parts.push(`λ < 0: nearby orbits alternate sides of ${star}.`);
  return parts.join(" ");
}

/** Where the `{…}` group that ends `latex` opens, or -1. */
function lastGroupStart(latex: string): number {
  if (!latex.endsWith("}")) return -1;
  let depth = 0;
  for (let i = latex.length - 1; i >= 0; i--) {
    if (latex[i] === "}") depth++;
    else if (latex[i] === "{" && --depth === 0) return i;
  }
  return -1;
}

/**
 * `latex` with `index` added to its subscript: `x_{0}`, and `x_{1,0}` for
 * `x_{1}` (as the core writes a map's `x_{1,n}`), not the double subscript
 * `x_{1}_0`, which is drawn as x₁₀. A superscript after the subscript stays
 * after it: `x_{1}^{(4)}` (the core's fourth derivative) is `x_{1,0}^{(4)}`.
 */
export function withSubscript(latex: string, index: string): string {
  const open = lastGroupStart(latex);
  if (open > 0 && latex[open - 1] === "_") return `${latex.slice(0, -1)},${index}}`;
  if (open > 0 && latex[open - 1] === "^") {
    const base = latex.slice(0, open - 1);
    const sub = lastGroupStart(base);
    if (sub > 0 && base[sub - 1] === "_") return `${base.slice(0, -1)},${index}}${latex.slice(open - 1)}`;
  }
  return `${latex}_{${index}}`;
}

export function sameAxis(a: AxisRef, b: AxisRef): boolean {
  return a.kind === b.kind && (a.kind === "time" || a.index === (b as { index: number }).index);
}

export function classificationTitle(c: Classification, dim = 2): string {
  if (dim === 1) {
    // f′(x*) = 0 decides nothing: x′ = x² is half-stable at 0, x′ = −x³ stable, x′ = x³ unstable.
    if (c === "sink" || c.startsWith("stable")) return "Stable";
    if (c === "source" || c.startsWith("unstable")) return "Unstable";
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

export function classificationExplain(c: Classification, kind: SystemKind | undefined, dim = 2): string {
  if (dim === 1) {
    const stable = c === "sink" || c.startsWith("stable");
    const unstable = c === "source" || c.startsWith("unstable");
    if (c === "non-isolated")
      return kind === "map"
        ? "f(x) = x for every x between the two non-isolated points: the fixed points are not isolated."
        : "f(x) = 0 for every x between the two non-isolated points: the fixed points are not isolated.";
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

export function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

export function firstOrderDisplay(
  equationsLatex: string[],
  states: StateInfo[],
  kind: SystemKind | undefined,
): string[] {
  const derived = states.some((s) => s.derived);
  if (kind === "map" || !derived || equationsLatex.length !== states.length) return equationsLatex;
  return equationsLatex.map((eq, i) => {
    const k = eq.indexOf("=");
    if (k < 0) return eq;
    return `\\frac{d${states[i].latex}}{dt} =${eq.slice(k + 1)}`;
  });
}
