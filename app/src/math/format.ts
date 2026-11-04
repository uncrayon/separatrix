import type { AxisRef, Classification, StateInfo, SystemKind } from "../api/types";

export function fmt(v: number, digits = 3): string {
  if (!Number.isFinite(v)) return Number.isNaN(v) ? "NaN" : v > 0 ? "∞" : "−∞";
  if (Math.abs(v) < 1e-12) return "0";
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

export function fmtTex(v: number, digits = 3): string {
  const s = fmt(v, digits);
  const m = /^(-?[\d.]+)e(-?\d+)$/.exec(s);
  if (m) return `${m[1]}\\times 10^{${m[2]}}`;
  if (s === "∞") return "\\infty";
  if (s === "−∞") return "-\\infty";
  return s;
}

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

/** The Greek letters the parser accepts, by command name (`varrho`, `Upsilon`, …). */
export const GREEK_NAMES: ReadonlySet<string> = new Set(Object.keys(GREEK));

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

export function diagnosticText(message: string): string {
  return latexToPlain(message).replace(/'([^'\s]{1,12})'/g, "‘$1’");
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
  return kind === "map" && axis.kind === "state" && other.kind === "time" ? `${base}_n` : base;
}

export function stateLatexForKind(s: StateInfo, kind: SystemKind | undefined): string {
  return kind === "map" ? `${s.latex}_n` : s.latex;
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
