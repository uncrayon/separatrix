// Browser-only stand-in for the Rust core, used by `npm run dev` outside Tauri.
//
// It knows the gallery templates (and the Blank system's starting equations)
// only: each has a hand-written right-hand side below. Anything else gets a friendly compile diagnostic. The numerics are
// deliberately simple (fixed-step RK4, Newton from a grid of seeds, marching
// squares) — good enough for a preview, not a reference.

import type {
  Arrow,
  AxisRef,
  Classification,
  CompileRequest,
  CompileResponse,
  FixedPointOut,
  NullclineOut,
  ParamInfo,
  PortraitRequest,
  PortraitResponse,
  SymbolInfo,
  SystemKind,
  TrajectoryOut,
  TrajectoryRequest,
} from "./types";
import { TEMPLATES } from "../templates";

/** Right-hand side: ODE → dx/dt, map → x_{n+1}. */
type Rhs = (t: number, x: number[], p: number[]) => number[];

interface MockSystem {
  kind: SystemKind;
  hamiltonian?: boolean;
  autonomous?: boolean;
  states: SymbolInfo[];
  params: ParamInfo[];
  equationsLatex: string[];
  f: Rhs;
}

const s = (name: string, latex = name): SymbolInfo => ({ name, latex });
const par = (name: string, latex = name): ParamInfo => ({ name, latex, default: 1 });
const xy = [s("x"), s("y")];
const xyz = [s("x"), s("y"), s("z")];
const theta = [s("theta", "\\theta"), s("theta'", "\\dot{\\theta}")];

/** Hand-written systems, keyed by template id. Mirrors what dyn-expr infers. */
const SYSTEMS: Record<string, MockSystem> = {
  "slope-field-x2-minus-t": {
    kind: "ode",
    autonomous: false,
    states: [s("x")],
    params: [],
    equationsLatex: ["\\dot{x} = x^{2} - t"],
    f: (t, [x]) => [x * x - t],
  },
  "saddle-node": {
    kind: "ode",
    states: [s("x")],
    params: [par("r")],
    equationsLatex: ["\\dot{x} = r + x^{2}"],
    f: (_t, [x], [r]) => [r + x * x],
  },
  "rabbits-vs-sheep": {
    kind: "ode",
    states: xy,
    params: [],
    equationsLatex: ["\\dot{x} = x \\left(3 - x - 2 y\\right)", "\\dot{y} = y \\left(2 - x - y\\right)"],
    f: (_t, [x, y]) => [x * (3 - x - 2 * y), y * (2 - x - y)],
  },
  pendulum: {
    kind: "ode",
    states: theta,
    params: [],
    equationsLatex: ["\\dot{\\theta} = \\dot{\\theta}", "\\ddot{\\theta} = -\\sin\\theta"],
    f: (_t, [th, w]) => [w, -Math.sin(th)],
  },
  "damped-pendulum": {
    kind: "ode",
    states: theta,
    params: [par("b")],
    equationsLatex: ["\\dot{\\theta} = \\dot{\\theta}", "\\ddot{\\theta} = -b \\dot{\\theta} - \\sin\\theta"],
    f: (_t, [th, w], [b]) => [w, -b * w - Math.sin(th)],
  },
  "van-der-pol": {
    kind: "ode",
    states: xy,
    params: [par("mu", "\\mu")],
    equationsLatex: ["\\dot{x} = y", "\\dot{y} = -x - \\mu \\left(x^{2} - 1\\right) y"],
    f: (_t, [x, y], [mu]) => [y, -x - mu * (x * x - 1) * y],
  },
  "selkov-glycolysis": {
    kind: "ode",
    states: xy,
    params: [par("a"), par("b")],
    equationsLatex: ["\\dot{x} = -x + a y + x^{2} y", "\\dot{y} = b - a y - x^{2} y"],
    f: (_t, [x, y], [a, b]) => [-x + a * y + x * x * y, b - a * y - x * x * y],
  },
  "linear-2d": {
    kind: "ode",
    states: xy,
    params: [par("a"), par("b"), par("c"), par("d")],
    equationsLatex: ["\\dot{x} = a x + b y", "\\dot{y} = c x + d y"],
    f: (_t, [x, y], [a, b, c, d]) => [a * x + b * y, c * x + d * y],
  },
  lorenz: {
    kind: "ode",
    states: xyz,
    params: [par("sigma", "\\sigma"), par("r"), par("b")],
    equationsLatex: [
      "\\dot{x} = \\sigma \\left(y - x\\right)",
      "\\dot{y} = r x - y - x z",
      "\\dot{z} = x y - b z",
    ],
    f: (_t, [x, y, z], [sg, r, b]) => [sg * (y - x), r * x - y - x * z, x * y - b * z],
  },
  rossler: {
    kind: "ode",
    states: xyz,
    params: [par("a"), par("b"), par("c")],
    equationsLatex: ["\\dot{x} = -y - z", "\\dot{y} = x + a y", "\\dot{z} = b + z \\left(x - c\\right)"],
    f: (_t, [x, y, z], [a, b, c]) => [-y - z, x + a * y, b + z * (x - c)],
  },
  "logistic-map": {
    kind: "map",
    states: [s("x")],
    params: [par("r")],
    equationsLatex: ["x_{n+1} = r x_{n} \\left(1 - x_{n}\\right)"],
    f: (_n, [x], [r]) => [r * x * (1 - x)],
  },
  "henon-map": {
    kind: "map",
    states: xy,
    params: [par("a"), par("b")],
    equationsLatex: ["x_{n+1} = y_{n} + 1 - a x_{n}^{2}", "y_{n+1} = b x_{n}"],
    f: (_n, [x, y], [a, b]) => [y + 1 - a * x * x, b * x],
  },
  "hamiltonian-pendulum": {
    kind: "ode",
    hamiltonian: true,
    states: [s("q"), s("p")],
    params: [],
    equationsLatex: [
      "\\dot{q} = \\frac{\\partial H}{\\partial p}",
      "\\dot{p} = -\\frac{\\partial H}{\\partial q}",
    ],
    f: (_t, [q, p]) => [p, -Math.sin(q)],
  },
};

const squash = (src: string) => src.replace(/\s+/g, "");

/**
 * Systems that are not gallery templates but that the UI itself offers:
 * the "Blank system" card starts from the harmonic oscillator (BLANK_TEMPLATE
 * in useStudio.ts). Keyed by the whitespace-free source.
 */
const EXTRA_SYSTEMS: { source: string; sys: MockSystem }[] = [
  {
    source: squash("\\dot{x}=y\n\\dot{y}=-x"),
    sys: {
      kind: "ode",
      states: xy,
      params: [],
      equationsLatex: ["\\dot{x} = y", "\\dot{y} = -x"],
      f: (_t, [x, y]) => [y, -x],
    },
  },
];

/** systemId → system. Ids are 1-based template indices, then the extras. */
const byId = new Map<number, MockSystem>();
TEMPLATES.forEach((t, i) => {
  const sys = SYSTEMS[t.id];
  if (sys) byId.set(i + 1, sys);
});
EXTRA_SYSTEMS.forEach((e, k) => byId.set(TEMPLATES.length + 1 + k, e.sys));

/** The id and system for a source, if the preview knows it. */
function lookup(src: string): { id: number; sys: MockSystem } | null {
  const index = TEMPLATES.findIndex((t) => squash(t.equations.join("\n")) === src);
  const sys = index >= 0 ? SYSTEMS[TEMPLATES[index].id] : undefined;
  if (sys) return { id: index + 1, sys };
  const k = EXTRA_SYSTEMS.findIndex((e) => e.source === src);
  return k >= 0 ? { id: TEMPLATES.length + 1 + k, sys: EXTRA_SYSTEMS[k].sys } : null;
}

/** Mock of the `compile_system` command: recognises template sources only. */
export async function mockCompile(req: CompileRequest): Promise<CompileResponse> {
  const src = squash(req.source);
  const empty: CompileResponse = {
    ok: false,
    hamiltonian: false,
    autonomous: true,
    states: [],
    params: [],
    equationsLatex: [],
    diagnostics: [],
  };
  if (!src) {
    return {
      ...empty,
      diagnostics: [
        { severity: "info", message: "Type an equation, for example \\dot{x} = y, or pick a template." },
      ],
    };
  }
  const found = lookup(src);
  if (!found) {
    return {
      ...empty,
      diagnostics: [
        {
          severity: "error",
          message:
            "This browser preview only runs the gallery templates. Open the desktop app (npm run tauri dev) to compile your own equations.",
        },
      ],
    };
  }
  const { id, sys } = found;
  return {
    ok: true,
    systemId: id,
    kind: sys.kind,
    hamiltonian: sys.hamiltonian ?? false,
    autonomous: sys.autonomous ?? true,
    states: sys.states,
    params: sys.params,
    equationsLatex: sys.equationsLatex,
    diagnostics: [],
  };
}

/** Mock of the `compute_portrait` command. */
export async function mockPortrait(req: PortraitRequest): Promise<PortraitResponse> {
  const start = performance.now();
  const sys = byId.get(req.systemId);
  if (!sys) throw new Error(`Unknown system ${req.systemId}`);
  const dim = sys.states.length;
  const p = req.params;

  /** The full state and time for a point of the drawing plane. */
  const lift = (px: number, py: number): [number, number[]] => {
    const x = req.baseState.slice(0, dim);
    while (x.length < dim) x.push(0);
    let t = req.fieldTime;
    const put = (axis: AxisRef, v: number) => {
      if (axis.kind === "time") t = v;
      else x[axis.index] = v;
    };
    put(req.xAxis, px);
    put(req.yAxis, py);
    return [t, x];
  };
  const comp = (axis: AxisRef, f: number[]) => (axis.kind === "time" ? 1 : f[axis.index]);

  const arrows: Arrow[] = [];
  if (sys.kind === "ode" && req.fieldGrid > 0) {
    const g = req.fieldGrid;
    const [x0, x1] = req.xRange;
    const [y0, y1] = req.yRange;
    for (let i = 0; i < g; i++) {
      for (let j = 0; j < g; j++) {
        const px = x0 + ((i + 0.5) / g) * (x1 - x0);
        const py = y0 + ((j + 0.5) / g) * (y1 - y0);
        const [t, x] = lift(px, py);
        const f = sys.f(t, x, p);
        arrows.push({ x: px, y: py, u: comp(req.xAxis, f), v: comp(req.yAxis, f) });
      }
    }
  }

  const nullclines: NullclineOut[] = [];
  if (sys.kind === "ode" && req.nullclines) {
    const shown = new Set<number>();
    for (const a of [req.xAxis, req.yAxis]) if (a.kind === "state") shown.add(a.index);
    if (dim === 1) shown.add(0);
    for (const k of [...shown].sort()) {
      const polylines = contour(
        (px, py) => {
          const [t, x] = lift(px, py);
          return sys.f(t, x, p)[k];
        },
        req.xRange,
        req.yRange,
        100,
      );
      nullclines.push({ state: k, polylines });
    }
  }

  const fixedPoints = req.fixedPoints && (sys.autonomous ?? true) ? findFixedPoints(sys, req, lift) : [];
  const trajectories = req.trajectories.map((tr) => solve(sys, p, tr, req.samples));

  return {
    requestId: req.requestId,
    arrows,
    nullclines,
    fixedPoints,
    trajectories,
    elapsedMs: performance.now() - start,
  };
}

// ---------------------------------------------------------------- trajectories

function solve(sys: MockSystem, p: number[], tr: TrajectoryRequest, samples: number): TrajectoryOut {
  const dim = sys.states.length;
  let x = tr.x0.slice(0, dim);
  const ts: number[] = [];
  const xs: number[] = [];
  const push = (t: number, v: number[]) => {
    ts.push(t);
    xs.push(...v);
  };
  const tooBig = (v: number[]) => v.some((c) => !Number.isFinite(c) || Math.abs(c) > 1e12);

  if (sys.kind === "map") {
    const n = Math.max(0, Math.round(tr.t1));
    push(0, x);
    for (let k = 0; k < n; k++) {
      x = sys.f(k, x, p);
      if (tooBig(x))
        return { t: ts, x: xs, status: "blewUp", stats: { accepted: k, rejected: 0, evals: k + 1 } };
      push(k + 1, x);
    }
    return { t: ts, x: xs, status: "completed", stats: { accepted: n, rejected: 0, evals: n } };
  }

  const span = tr.t1 - tr.t0;
  const hMax = 0.01;
  const steps = Math.max(1, Math.ceil(Math.abs(span) / hMax));
  const h = span / steps;
  const every = Math.max(1, Math.floor(steps / Math.max(2, samples)));
  let t = tr.t0;
  push(t, x);
  for (let k = 1; k <= steps; k++) {
    const k1 = sys.f(t, x, p);
    const k2 = sys.f(
      t + h / 2,
      x.map((v, i) => v + (h / 2) * k1[i]),
      p,
    );
    const k3 = sys.f(
      t + h / 2,
      x.map((v, i) => v + (h / 2) * k2[i]),
      p,
    );
    const k4 = sys.f(
      t + h,
      x.map((v, i) => v + h * k3[i]),
      p,
    );
    x = x.map((v, i) => v + (h / 6) * (k1[i] + 2 * k2[i] + 2 * k3[i] + k4[i]));
    t = tr.t0 + k * h;
    if (tooBig(x)) {
      return { t: ts, x: xs, status: "blewUp", stats: { accepted: k, rejected: 0, evals: 4 * k } };
    }
    if (k % every === 0 || k === steps) push(t, x);
  }
  return { t: ts, x: xs, status: "completed", stats: { accepted: steps, rejected: 0, evals: 4 * steps } };
}

// ---------------------------------------------------------------- fixed points

function findFixedPoints(
  sys: MockSystem,
  req: PortraitRequest,
  lift: (px: number, py: number) => [number, number[]],
): FixedPointOut[] {
  const dim = sys.states.length;
  const g = (x: number[]) => {
    const f = sys.f(0, x, req.params);
    return sys.kind === "map" ? f.map((v, i) => v - x[i]) : f;
  };
  const found: number[][] = [];
  const seeds = 9;
  for (let i = 0; i < seeds; i++) {
    for (let j = 0; j < seeds; j++) {
      const px = req.xRange[0] + ((i + 0.5) / seeds) * (req.xRange[1] - req.xRange[0]);
      const py = req.yRange[0] + ((j + 0.5) / seeds) * (req.yRange[1] - req.yRange[0]);
      const root = newton(g, lift(px, py)[1], dim);
      if (!root) continue;
      const scale = 1e-6 * (1 + Math.hypot(...root));
      if (!found.some((q) => Math.hypot(...q.map((v, k) => v - root[k])) < scale)) found.push(root);
    }
  }
  return found.map((x) => {
    const jac = jacobian((y) => sys.f(0, y, req.params), x, dim);
    const eig = eigenvalues(jac, dim);
    const { re, im } = eigenvectors(jac, eig, dim);
    return {
      x,
      eigenvalues: eig,
      eigenvectors: re,
      eigenvectorsIm: im,
      ...classifyMock(sys.kind, jac, eig, dim),
    };
  });
}

function newton(g: (x: number[]) => number[], start: number[], dim: number): number[] | null {
  let x = start.slice();
  for (let it = 0; it < 40; it++) {
    const r = g(x);
    if (r.some((v) => !Number.isFinite(v))) return null;
    if (Math.hypot(...r) < 1e-11) return x;
    const dx = solveLinear(
      jacobian(g, x, dim),
      r.map((v) => -v),
      dim,
    );
    if (!dx) return null;
    x = x.map((v, i) => v + dx[i]);
    if (x.some((v) => Math.abs(v) > 1e6)) return null;
  }
  return Math.hypot(...g(x)) < 1e-9 ? x : null;
}

/** Central-difference Jacobian, row-major. */
function jacobian(f: (x: number[]) => number[], x: number[], dim: number): number[] {
  const out = new Array<number>(dim * dim).fill(0);
  for (let j = 0; j < dim; j++) {
    const h = 1e-6 * Math.max(1, Math.abs(x[j]));
    const xp = x.slice();
    const xm = x.slice();
    xp[j] += h;
    xm[j] -= h;
    const fp = f(xp);
    const fm = f(xm);
    for (let i = 0; i < dim; i++) out[i * dim + j] = (fp[i] - fm[i]) / (2 * h);
  }
  return out;
}

/** Gaussian elimination with partial pivoting; null if singular. */
function solveLinear(a: number[], b: number[], n: number): number[] | null {
  const m = a.slice();
  const r = b.slice();
  for (let c = 0; c < n; c++) {
    let piv = c;
    for (let i = c + 1; i < n; i++) if (Math.abs(m[i * n + c]) > Math.abs(m[piv * n + c])) piv = i;
    if (Math.abs(m[piv * n + c]) < 1e-14) return null;
    for (let k = 0; k < n; k++) [m[c * n + k], m[piv * n + k]] = [m[piv * n + k], m[c * n + k]];
    [r[c], r[piv]] = [r[piv], r[c]];
    for (let i = c + 1; i < n; i++) {
      const f = m[i * n + c] / m[c * n + c];
      for (let k = c; k < n; k++) m[i * n + k] -= f * m[c * n + k];
      r[i] -= f * r[c];
    }
  }
  const x = new Array<number>(n).fill(0);
  for (let i = n - 1; i >= 0; i--) {
    let acc = r[i];
    for (let k = i + 1; k < n; k++) acc -= m[i * n + k] * x[k];
    x[i] = acc / m[i * n + i];
  }
  return x;
}

/** Eigenvalues of a 1×1, 2×2 or 3×3 matrix as [re, im] pairs. */
function eigenvalues(j: number[], n: number): [number, number][] {
  if (n === 1) return [[j[0], 0]];
  if (n === 2) {
    const tr = j[0] + j[3];
    const det = j[0] * j[3] - j[1] * j[2];
    return quadratic(1, -tr, det);
  }
  if (n === 3) {
    // Characteristic polynomial λ³ − c2 λ² + c1 λ − c0.
    const c2 = j[0] + j[4] + j[8];
    const c1 = j[0] * j[4] - j[1] * j[3] + j[0] * j[8] - j[2] * j[6] + j[4] * j[8] - j[5] * j[7];
    const c0 =
      j[0] * (j[4] * j[8] - j[5] * j[7]) -
      j[1] * (j[3] * j[8] - j[5] * j[6]) +
      j[2] * (j[3] * j[7] - j[4] * j[6]);
    const p = (l: number) => ((l - c2) * l + c1) * l - c0;
    // One real root by bisection on a bracket that surely contains it.
    const bound = 1 + Math.max(Math.abs(c2), Math.abs(c1), Math.abs(c0));
    let lo = -bound;
    let hi = bound;
    for (let k = 0; k < 200; k++) {
      const mid = (lo + hi) / 2;
      if (Math.sign(p(mid)) === Math.sign(p(lo))) lo = mid;
      else hi = mid;
    }
    const r = (lo + hi) / 2;
    // Deflate: λ³ − c2 λ² + c1 λ − c0 = (λ − r)(λ² + bλ + c).
    const b = r - c2;
    const c = c1 + r * b;
    return [[r, 0], ...quadratic(1, b, c)];
  }
  return [];
}

function quadratic(a: number, b: number, c: number): [number, number][] {
  const disc = b * b - 4 * a * c;
  if (disc >= 0) {
    const sq = Math.sqrt(disc);
    return [
      [(-b - sq) / (2 * a), 0],
      [(-b + sq) / (2 * a), 0],
    ];
  }
  const sq = Math.sqrt(-disc);
  return [
    [-b / (2 * a), -sq / (2 * a)],
    [-b / (2 * a), sq / (2 * a)],
  ];
}

// Eigenvectors, following the conventions of dyn-analysis (eigvec.rs): unit
// length, largest-magnitude component positive; complex pairs as real and
// imaginary parts with the phase making them the ellipse's semi-axes; `null`
// for the missing direction of a defective (repeated) eigenvalue.

type C = [number, number];
const cmul = (a: C, b: C): C => [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]];
const csub = (a: C, b: C): C => [a[0] - b[0], a[1] - b[1]];
const cnorm = (v: C[]) => Math.sqrt(v.reduce((acc, z) => acc + z[0] * z[0] + z[1] * z[1], 0));

/** Bilinear cross product: orthogonal (without conjugation) to both rows. */
function ccross(a: C[], b: C[]): C[] {
  return [
    csub(cmul(a[1], b[2]), cmul(a[2], b[1])),
    csub(cmul(a[2], b[0]), cmul(a[0], b[2])),
    csub(cmul(a[0], b[1]), cmul(a[1], b[0])),
  ];
}

/** Basis of the null space of the n×n complex matrix `m` (n ≤ 3), best effort. */
function nullSpace(m: C[][], n: number, tol: number): C[][] {
  const e = (k: number): C[] => Array.from({ length: n }, (_, i): C => [i === k ? 1 : 0, 0]);
  const big = (v: C[]) => cnorm(v) > tol;
  if (!m.some((row) => big(row))) return Array.from({ length: n }, (_, k) => e(k));
  if (n === 1) return [];
  if (n === 2) {
    const cands: C[][] = [
      [[-m[0][1][0], -m[0][1][1]], m[0][0]],
      [[-m[1][1][0], -m[1][1][1]], m[1][0]],
    ];
    return [cands.reduce((a, b) => (cnorm(b) > cnorm(a) ? b : a))];
  }
  const crosses = [ccross(m[0], m[1]), ccross(m[0], m[2]), ccross(m[1], m[2])];
  const best = crosses.reduce((a, b) => (cnorm(b) > cnorm(a) ? b : a));
  if (cnorm(best) > tol * tol) return [best];
  // Rank one: two vectors orthogonal to the largest row.
  const r = m.reduce((a, b) => (cnorm(b) > cnorm(a) ? b : a));
  const u = [0, 1, 2].map((k) => ccross(r, e(k))).reduce((a, b) => (cnorm(b) > cnorm(a) ? b : a));
  return [u, ccross(r, u)];
}

function dominantIndex(mags: number[]): number {
  const max = Math.max(...mags);
  return mags.findIndex((v) => v >= (1 - 1e-9) * max);
}

function normaliseVector(v: C[], complex: boolean): { re: number[]; im: number[] | null } {
  const len = cnorm(v);
  let w = v.map(([a, b]): C => [a / len, b / len]);
  if (complex) {
    const aa = w.reduce((acc, z) => acc + z[0] * z[0], 0);
    const bb = w.reduce((acc, z) => acc + z[1] * z[1], 0);
    const ab = w.reduce((acc, z) => acc + z[0] * z[1], 0);
    const p = aa - bb;
    const q = 2 * ab;
    let theta: number;
    if (Math.hypot(p, q) <= 1e-8 * (aa + bb)) {
      const k = dominantIndex(w.map(([a, b]) => Math.hypot(a, b)));
      theta = -Math.atan2(w[k][1], w[k][0]);
    } else {
      theta = 0.5 * Math.atan2(-q, p);
    }
    const rot: C = [Math.cos(theta), Math.sin(theta)];
    w = w.map((z) => cmul(z, rot));
  }
  const k = dominantIndex(w.map(([a]) => Math.abs(a)));
  if (w[k][0] < 0) w = w.map(([a, b]): C => [-a, -b]);
  return { re: w.map(([a]) => a), im: complex ? w.map(([, b]) => b) : null };
}

function eigenvectors(
  j: number[],
  eig: [number, number][],
  n: number,
): { re: (number[] | null)[]; im: (number[] | null)[] } {
  const re: (number[] | null)[] = [];
  const im: (number[] | null)[] = [];
  const scale = Math.max(...eig.map(([a, b]) => Math.hypot(a, b)), ...j.map(Math.abs));
  const tol = 1e-6 * Math.max(scale, 1e-300);
  // Repeated eigenvalues (the mock's 3×3 roots are only accurate to ~1e-5
  // when repeated) are replaced by the mean of their cluster.
  const same = (a: [number, number], b: [number, number]) =>
    Math.hypot(a[0] - b[0], a[1] - b[1]) <= 1e-4 * Math.max(scale, 1e-300);
  for (let k = 0; k < eig.length; k++) {
    const cluster = eig.filter((e) => same(e, eig[k]));
    const occurrence = eig.slice(0, k).filter((e) => same(e, eig[k])).length;
    const lr = cluster.reduce((acc, e) => acc + e[0], 0) / cluster.length;
    const li = cluster.reduce((acc, e) => acc + e[1], 0) / cluster.length;
    const complex = Math.abs(li) > tol;
    const m: C[][] = Array.from({ length: n }, (_, r) =>
      Array.from({ length: n }, (_, c): C => (r === c ? [j[r * n + c] - lr, complex ? -li : 0] : [j[r * n + c], 0])),
    );
    const basis = scale === 0 ? nullSpace(m, n, 0) : nullSpace(m, n, tol);
    const v = basis[occurrence];
    if (!v || !(cnorm(v) > 0)) {
      re.push(null);
      im.push(null);
      continue;
    }
    const out = normaliseVector(v, complex);
    const ok = out.re.every(Number.isFinite) && (out.im ?? []).every(Number.isFinite);
    re.push(ok ? out.re : null);
    im.push(ok ? out.im : null);
  }
  return { re, im };
}

function classifyMock(
  kind: SystemKind,
  jac: number[],
  eig: [number, number][],
  dim: number,
): { classification: Classification; stable: boolean } {
  const eps = 1e-8;
  if (kind === "map") {
    const mods = eig.map(([re, im]) => Math.hypot(re, im));
    const stable = mods.every((m) => m < 1 - eps);
    if (mods.some((m) => Math.abs(m - 1) < eps)) return { classification: "non-hyperbolic", stable: false };
    if (stable) return { classification: "sink", stable };
    if (mods.every((m) => m > 1)) return { classification: "source", stable };
    // Maps of any dimension: "saddle (n-D)", as dyn-analysis (Strogatz's
    // trace–determinant classes are for 2D flows only).
    return { classification: "saddle (n-D)", stable };
  }
  const stable = eig.every(([re]) => re < -eps);
  if (dim === 1) {
    const l = eig[0][0];
    if (Math.abs(l) < eps) return { classification: "non-hyperbolic", stable: false };
    // As dyn-analysis classify.rs: 1D flows are sink / source.
    return { classification: l < 0 ? "sink" : "source", stable };
  }
  if (dim === 2) {
    const tau = jac[0] + jac[3];
    const delta = jac[0] * jac[3] - jac[1] * jac[2];
    const disc = tau * tau - 4 * delta;
    if (delta < -eps) return { classification: "saddle", stable: false };
    if (Math.abs(delta) <= eps) return { classification: "non-isolated", stable: false };
    if (Math.abs(tau) <= eps) return { classification: "center", stable: false };
    const s = tau < 0 ? "stable" : "unstable";
    if (Math.abs(disc) <= eps * Math.max(1, tau * tau)) {
      const scalar = Math.abs(jac[1]) < eps && Math.abs(jac[2]) < eps;
      return { classification: `${s} ${scalar ? "star" : "degenerate node"}` as Classification, stable };
    }
    return { classification: `${s} ${disc > 0 ? "node" : "spiral"}` as Classification, stable };
  }
  if (eig.some(([re]) => Math.abs(re) < eps)) return { classification: "non-hyperbolic", stable: false };
  if (stable) return { classification: "sink", stable };
  if (eig.every(([re]) => re > 0)) return { classification: "source", stable };
  return { classification: "saddle (n-D)", stable };
}

// ---------------------------------------------------------------- nullclines

/** Zero set of `g` on a grid by marching squares, stitched into polylines. */
function contour(
  g: (x: number, y: number) => number,
  xr: [number, number],
  yr: [number, number],
  n: number,
): [number, number][][] {
  const dx = (xr[1] - xr[0]) / n;
  const dy = (yr[1] - yr[0]) / n;
  const v: number[] = [];
  for (let j = 0; j <= n; j++) for (let i = 0; i <= n; i++) v.push(g(xr[0] + i * dx, yr[0] + j * dy));
  const at = (i: number, j: number) => v[j * (n + 1) + i];
  const segs: [[number, number], [number, number]][] = [];
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      const c = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
      const pts: [number, number][] = [];
      // Corners in order: (i,j) (i+1,j) (i+1,j+1) (i,j+1).
      const cx = [i, i + 1, i + 1, i];
      const cy = [j, j, j + 1, j + 1];
      for (let e = 0; e < 4; e++) {
        const a = c[e];
        const b = c[(e + 1) % 4];
        if (!Number.isFinite(a) || !Number.isFinite(b)) continue;
        if ((a <= 0 && b > 0) || (a > 0 && b <= 0)) {
          const s = a / (a - b);
          const x = cx[e] + s * (cx[(e + 1) % 4] - cx[e]);
          const y = cy[e] + s * (cy[(e + 1) % 4] - cy[e]);
          pts.push([xr[0] + x * dx, yr[0] + y * dy]);
        }
      }
      if (pts.length === 2) segs.push([pts[0], pts[1]]);
      else if (pts.length === 4) {
        segs.push([pts[0], pts[1]]);
        segs.push([pts[2], pts[3]]);
      }
    }
  }
  return stitch(segs, Math.min(dx, dy) * 1e-6);
}

/** Join segments that share endpoints into longer polylines (so dashes flow). */
function stitch(segs: [[number, number], [number, number]][], tol: number): [number, number][][] {
  const key = ([x, y]: [number, number]) => `${Math.round(x / tol)},${Math.round(y / tol)}`;
  const ends = new Map<string, number[]>();
  segs.forEach((s, i) => {
    for (const p of s) {
      const k = key(p);
      const list = ends.get(k);
      if (list) list.push(i);
      else ends.set(k, [i]);
    }
  });
  const used = new Array<boolean>(segs.length).fill(false);
  const out: [number, number][][] = [];
  const grow = (line: [number, number][], atEnd: boolean) => {
    for (;;) {
      const tip = atEnd ? line[line.length - 1] : line[0];
      const next = (ends.get(key(tip)) ?? []).find((i) => !used[i]);
      if (next === undefined) return;
      used[next] = true;
      const [a, b] = segs[next];
      const other = key(a) === key(tip) ? b : a;
      if (atEnd) line.push(other);
      else line.unshift(other);
    }
  };
  segs.forEach((s, i) => {
    if (used[i]) return;
    used[i] = true;
    const line: [number, number][] = [s[0], s[1]];
    grow(line, true);
    grow(line, false);
    out.push(line);
  });
  return out;
}
