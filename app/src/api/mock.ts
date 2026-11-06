import type {
  Method,
  Arrow,
  AxisRef,
  ChosenSavePath,
  Classification,
  ClipboardTextRequest,
  CompileRequest,
  CompileResponse,
  ConvertRequest,
  ConvertResponse,
  CsvFileOut,
  CsvRequest,
  CsvResponse,
  DocumentStateRequest,
  ExportPortraitRequest,
  FigureLatexRequest,
  FigureLatexResponse,
  FixedPointOut,
  FixedPointsLatexRequest,
  LyapunovExportRequest,
  LyapunovRequest,
  LyapunovResponse,
  NullclineOut,
  OpenedProject,
  OrbitDiagramRequest,
  OrbitDiagramResponse,
  OrbitExportRequest,
  OrbitFigureRequest,
  ParamInfo,
  PortraitRequest,
  PortraitResponse,
  ProjectChooseSaveRequest,
  SectionSpec,
  ProjectFile,
  ProjectReadRequest,
  ProjectWriteRequest,
  SaveCsvFileRequest,
  SaveCsvRequest,
  SavedFiles,
  SavedProject,
  SaveFigureRequest,
  SaveOrbitFigureRequest,
  SaveTextRequest,
  StateInfo,
  SystemKind,
  SystemLatexRequest,
  SystemLatexResponse,
  TrajectoryOut,
  TrajectoryRequest,
} from "./types";
import { BLANK_TEMPLATE, TEMPLATES, type Template } from "../templates";
import { mockConvertLines, mockTemplateIndex } from "./mockLookup";
import { MAX_SECTION_POINTS } from "./ipc.generated";
import { METHODS } from "../state/methods";
import { roundWithUncertainty } from "../state/model";

type Rhs = (t: number, x: number[], p: number[]) => number[];

interface MockSystem {
  kind: SystemKind;
  hamiltonian?: boolean;
  /** H(q, p) of a Hamiltonian system that splits as T(p) + V(q). */
  energy?: (x: number[], p: number[]) => number;
  /** Whether the core's Dormand–Prince stiffness test fires on the template's runs at these parameters. */
  stiffAt?: (p: number[]) => boolean;
  autonomous?: boolean;
  states: StateInfo[];
  params: ParamInfo[];
  equationsLatex: string[];
  f: Rhs;
}

const s = (name: string, latex = name, derived = false): StateInfo => ({ name, latex, derived });
const par = (name: string, latex = name): ParamInfo => ({ name, latex, default: 1 });
const xy = [s("x"), s("y")];
const xyz = [s("x"), s("y"), s("z")];
const theta = [s("theta", "\\theta"), s("theta'", "\\dot{\\theta}", true)];

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
    energy: ([q, p]) => (p * p) / 2 - Math.cos(q),
  },
  "sin-x": {
    kind: "ode",
    states: [s("x")],
    params: [],
    equationsLatex: ["\\dot{x} = \\sin x"],
    f: (_t, [x]) => [Math.sin(x)],
  },
  "logistic-growth": {
    kind: "ode",
    states: [s("N")],
    params: [par("r"), par("K")],
    equationsLatex: ["\\dot{N} = r N \\left(1 - \\frac{N}{K}\\right)"],
    f: (_t, [n], [r, k]) => [r * n * (1 - n / k)],
  },
  transcritical: {
    kind: "ode",
    states: [s("x")],
    params: [par("r")],
    equationsLatex: ["\\dot{x} = r x - x^{2}"],
    f: (_t, [x], [r]) => [r * x - x * x],
  },
  pitchfork: {
    kind: "ode",
    states: [s("x")],
    params: [par("r")],
    equationsLatex: ["\\dot{x} = r x - x^{3}"],
    f: (_t, [x], [r]) => [r * x - x * x * x],
  },
  "bead-on-hoop": {
    kind: "ode",
    states: [s("phi", "\\phi")],
    params: [par("gamma", "\\gamma")],
    equationsLatex: ["\\dot{\\phi} = \\left(\\gamma \\cos\\phi - 1\\right) \\sin\\phi"],
    f: (_t, [ph], [g]) => [(g * Math.cos(ph) - 1) * Math.sin(ph)],
  },
  "imperfect-bifurcation": {
    kind: "ode",
    states: [s("x")],
    params: [par("h"), par("r")],
    equationsLatex: ["\\dot{x} = h + r x - x^{3}"],
    f: (_t, [x], [h, r]) => [h + r * x - x * x * x],
  },
  "nonuniform-oscillator": {
    kind: "ode",
    states: [s("theta", "\\theta")],
    params: [par("omega", "\\omega"), par("a")],
    equationsLatex: ["\\dot{\\theta} = \\omega - a \\sin\\theta"],
    f: (_t, [th], [w, a]) => [w - a * Math.sin(th)],
  },
  "romeo-juliet": {
    kind: "ode",
    states: [s("R"), s("J")],
    params: [par("a"), par("b")],
    equationsLatex: ["\\dot{R} = a J", "\\dot{J} = -b R"],
    f: (_t, [r, j], [a, b]) => [a * j, -b * r],
  },
  "double-well": {
    kind: "ode",
    states: [s("x"), s("x'", "\\dot{x}", true)],
    params: [],
    equationsLatex: ["\\dot{x} = \\dot{x}", "\\ddot{x} = x - x^{3}"],
    f: (_t, [x, v]) => [v, x - x * x * x],
  },
  reversible: {
    kind: "ode",
    states: xy,
    params: [],
    equationsLatex: ["\\dot{x} = y - y^{3}", "\\dot{y} = -x - y^{2}"],
    f: (_t, [x, y]) => [y - y * y * y, -x - y * y],
  },
  "relaxation-oscillator": {
    kind: "ode",
    states: xy,
    params: [par("mu", "\\mu")],
    equationsLatex: [
      "\\dot{x} = \\mu \\left(y - \\frac{x^{3}}{3} + x\\right)",
      "\\dot{y} = -\\frac{x}{\\mu}",
    ],
    f: (_t, [x, y], [mu]) => [mu * (y - (x * x * x) / 3 + x), -x / mu],
    // Measured with the core from the template's three start points over t ≤ 40: μ = 30 fires, μ = 20 does not.
    stiffAt: ([mu]) => mu >= 30,
  },
  "saddle-node-2d": {
    kind: "ode",
    states: xy,
    params: [par("mu", "\\mu")],
    equationsLatex: ["\\dot{x} = \\mu - x^{2}", "\\dot{y} = -y"],
    f: (_t, [x, y], [mu]) => [mu - x * x, -y],
  },
  "hopf-supercritical": {
    kind: "ode",
    states: xy,
    params: [par("mu", "\\mu")],
    equationsLatex: [
      "\\dot{x} = \\mu x - y - x \\left(x^{2} + y^{2}\\right)",
      "\\dot{y} = x + \\mu y - y \\left(x^{2} + y^{2}\\right)",
    ],
    f: (_t, [x, y], [mu]) => {
      const r2 = x * x + y * y;
      return [mu * x - y - x * r2, x + mu * y - y * r2];
    },
  },
  cdima: {
    kind: "ode",
    states: xy,
    params: [par("a"), par("b")],
    equationsLatex: [
      "\\dot{x} = a - x - \\frac{4 x y}{1 + x^{2}}",
      "\\dot{y} = b x \\left(1 - \\frac{y}{1 + x^{2}}\\right)",
    ],
    f: (_t, [x, y], [a, b]) => [a - x - (4 * x * y) / (1 + x * x), b * x * (1 - y / (1 + x * x))],
  },
  "driven-pendulum": {
    kind: "ode",
    states: [s("phi", "\\phi"), s("phi'", "\\dot{\\phi}", true)],
    params: [par("alpha", "\\alpha"), par("I")],
    equationsLatex: ["\\dot{\\phi} = \\dot{\\phi}", "\\ddot{\\phi} = -\\alpha \\dot{\\phi} - \\sin\\phi + I"],
    f: (_t, [ph, w], [al, i]) => [w, -al * w - Math.sin(ph) + i],
  },
  "coupled-oscillators": {
    kind: "ode",
    states: [s("theta_1", "\\theta_{1}"), s("theta_2", "\\theta_{2}")],
    params: [par("omega_1", "\\omega_{1}"), par("K_1", "K_{1}"), par("omega_2", "\\omega_{2}"), par("K_2", "K_{2}")],
    equationsLatex: [
      "\\dot{\\theta}_{1} = \\omega_{1} + K_{1} \\sin\\left(\\theta_{2} - \\theta_{1}\\right)",
      "\\dot{\\theta}_{2} = \\omega_{2} + K_{2} \\sin\\left(\\theta_{1} - \\theta_{2}\\right)",
    ],
    f: (_t, [t1, t2], [w1, k1, w2, k2]) => [w1 + k1 * Math.sin(t2 - t1), w2 + k2 * Math.sin(t1 - t2)],
  },
  "poincare-map-forced": {
    kind: "ode",
    autonomous: false,
    states: [s("x")],
    params: [par("A")],
    equationsLatex: ["\\dot{x} = -x + A \\sin t"],
    f: (t, [x], [a]) => [-x + a * Math.sin(t)],
  },
  "forced-double-well": {
    kind: "ode",
    autonomous: false,
    states: [s("x"), s("x'", "\\dot{x}", true)],
    params: [par("delta", "\\delta"), par("F"), par("omega", "\\omega")],
    equationsLatex: [
      "\\dot{x} = \\dot{x}",
      "\\ddot{x} = -\\delta \\dot{x} + x - x^{3} + F \\cos\\left(\\omega t\\right)",
    ],
    f: (t, [x, v], [d, f, w]) => [v, -d * v + x - x * x * x + f * Math.cos(w * t)],
  },
  blank: {
    kind: "ode",
    states: xy,
    params: [],
    equationsLatex: ["\\dot{x} = y", "\\dot{y} = -x"],
    f: (_t, [x, y]) => [y, -x],
  },
};

const MOCK_TEMPLATES: Template[] = [...TEMPLATES, BLANK_TEMPLATE];

const byId = new Map<number, MockSystem>();
MOCK_TEMPLATES.forEach((t, i) => {
  const sys = SYSTEMS[t.id];
  if (sys) byId.set(i + 1, sys);
});

/** A template's system, by its LaTeX or text source: both give the same `systemId`. */
function lookup(src: string): { id: number; sys: MockSystem } | null {
  const index = mockTemplateIndex(MOCK_TEMPLATES, src);
  const sys = index >= 0 ? SYSTEMS[MOCK_TEMPLATES[index].id] : undefined;
  return sys ? { id: index + 1, sys } : null;
}

export async function mockConvert(req: ConvertRequest): Promise<ConvertResponse> {
  return mockConvertLines(MOCK_TEMPLATES, req);
}

export async function mockCompile(req: CompileRequest): Promise<CompileResponse> {
  const src = req.source.trim();
  const empty: CompileResponse = {
    ok: false,
    hamiltonian: false,
    separable: false,
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
        {
          severity: "info",
          message: `Type an equation, for example ${req.format === "text" ? "x' = y" : "\\dot{x} = y"}, or pick a template.`,
        },
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
    separable: sys.energy !== undefined,
    autonomous: sys.autonomous ?? true,
    states: sys.states,
    params: sys.params,
    equationsLatex: sys.equationsLatex,
    diagnostics: [],
  };
}

/** `compute_portrait`; the request and response are kept in `__sepxMock.lastPortrait` for the e2e tests. */
export async function mockPortrait(req: PortraitRequest): Promise<PortraitResponse> {
  const res = await portrait(req);
  sepxMock().lastPortrait = { req, res };
  return res;
}

/** `compute_lyapunov`; the request and response are kept in `__sepxMock.lastLyapunov` for the e2e tests. */
export async function mockLyapunov(req: LyapunovRequest): Promise<LyapunovResponse> {
  const started = performance.now();
  const sys = byId.get(req.systemId);
  if (!sys) throw new Error("Unknown system; compile it first.");
  const method = req.solver.method;
  if (sys.kind !== "map" && METHODS[method].needs !== "any")
    throw new Error(
      `A Lyapunov run integrates the variational equations, which ${METHODS[method].label} cannot step; choose Runge–Kutta 4 or Dormand–Prince 5(4).`,
    );
  if (sys.kind !== "map" && method === "rosenbrock23")
    throw new Error(
      "A Lyapunov run integrates the variational equations, which have no exact Jacobian for Rosenbrock 2(3) to use; choose Runge–Kutta 4 or Dormand–Prince 5(4).",
    );
  const res = { ...lyapunovSpectrum(sys, req), elapsedMs: performance.now() - started };
  sepxMock().lastLyapunov = { req, res };
  return res;
}

/**
 * `compute_orbit_diagram`, as `dyn_analysis::orbit_diagram` and
 * `period_doublings` compute it (the multiplier from a central difference);
 * the request and response are kept in `__sepxMock.lastOrbit` for the e2e tests.
 */
export async function mockOrbitDiagram(req: OrbitDiagramRequest): Promise<OrbitDiagramResponse> {
  const started = performance.now();
  const sys = byId.get(req.systemId);
  if (!sys) throw new Error("Unknown system; compile it first.");
  if (sys.kind !== "map") throw new Error("Orbit diagrams are for maps; this system is a flow.");
  const [lo, hi] = req.range;
  const step = (hi - lo) / (req.columns - 1);
  const r = Array.from({ length: req.columns }, (_, j) => lo + j * step);
  const x: (number | null)[] = new Array(req.columns * req.kept).fill(null);
  const p = req.params.slice();
  const end = req.transient + req.kept;
  r.forEach((rj, j) => {
    p[req.param] = rj;
    let state = req.x0.slice();
    for (let k = 0; k < end; k++) {
      if (k >= req.transient) x[j * req.kept + k - req.transient] = state[req.state];
      if (k + 1 === end) break;
      const next = sys.f(k, state, p);
      if (!next.every((v) => Math.abs(v) <= BLOW_UP)) break;
      state = next;
    }
  });
  const doublings = req.doublings ? mockDoublings(sys, req.params, req.param, r, x, req.kept) : undefined;
  const res: OrbitDiagramResponse = {
    requestId: req.requestId,
    r,
    x,
    ...(doublings ? { doublings } : {}),
    elapsedMs: performance.now() - started,
  };
  sepxMock().lastOrbit = { req, res };
  return res;
}

/** `BLOW_UP_THRESHOLD` in dyn-core. */
const BLOW_UP = 1e12;

function mockDoublings(
  sys: MockSystem,
  params: number[],
  param: number,
  r: number[],
  x: (number | null)[],
  kept: number,
): { r: number; period: number }[] {
  const p = params.slice();
  const f = (v: number) => sys.f(0, [v], p)[0];
  const orbitOf = (v: number, period: number): [number, number] => {
    let mu = 1;
    for (let i = 0; i < period; i++) {
      const h = 1e-6 * (1 + Math.abs(v));
      mu *= (f(v + h) - f(v - h)) / (2 * h);
      v = f(v);
    }
    return [v, mu];
  };
  const cycle = (rr: number, seed: number, period: number): [number, number] | null => {
    p[param] = rr;
    let v = seed;
    for (let i = 0; i < 50; i++) {
      const [image, mu] = orbitOf(v, period);
      const dx = (image - v) / (mu - 1);
      v -= dx;
      if (!Number.isFinite(v)) return null;
      if (Math.abs(dx) <= 1e-14 * (1 + Math.abs(v))) return [v, orbitOf(v, period)[1]];
    }
    return null;
  };
  const column = (j: number) => x.slice(j * kept, (j + 1) * kept);
  const periodOf = (c: (number | null)[]): number | null => {
    for (let period = 1; 2 * period <= c.length; period *= 2) {
      const repeats = c.every(
        (a, n) => n + period >= c.length || (a !== null && c[n + period] !== null && Math.abs(c[n + period]! - a) <= 1e-7 * (1 + Math.abs(a))),
      );
      if (repeats) return period;
    }
    return null;
  };
  const found: { r: number; period: number }[] = [];
  let previous: [number, number] | null = null;
  for (let j = 0; j < r.length; j++) {
    const period = periodOf(column(j));
    if (period === null) continue;
    if (previous && period === 2 * previous[1]) {
      const [a, before] = previous;
      let seed = column(a)[kept - 1]!;
      let stable: [number, number] | null = null;
      let bracket: [number, number, number] | null = null;
      for (const rr of r.slice(Math.max(0, a - 1))) {
        const c = cycle(rr, seed, before);
        if (!c) break;
        seed = c[0];
        if (c[1] + 1 > 0) stable = [rr, c[0]];
        else {
          bracket = stable ? [stable[0], rr, stable[1]] : null;
          break;
        }
      }
      if (bracket) {
        let [blo, bhi, bx] = bracket;
        let ok = true;
        while (ok && bhi - blo > 1e-12) {
          const mid = 0.5 * (blo + bhi);
          const c = cycle(mid, bx, before);
          if (!c) ok = false;
          else if (c[1] + 1 > 0) [blo, bx] = [mid, c[0]];
          else [bhi, bx] = [mid, c[0]];
        }
        if (ok) found.push({ r: 0.5 * (blo + bhi), period: before });
      }
    }
    previous = [j, period];
  }
  return found;
}

/** The core's history length (`LYAPUNOV_HISTORY` in service.rs). */
const LYAPUNOV_HISTORY = 2000;

/**
 * Benettin's method as `dyn_analysis::lyapunov_spectrum` runs it: Q (column
 * major) follows J(x)Q, re-orthonormalized by modified Gram–Schmidt (twice)
 * every interval, ln R_ii summed per block. The preview's flows step with
 * RK4 at h (0.01 for an adaptive method) and a central-difference Jacobian.
 */
function lyapunovSpectrum(sys: MockSystem, req: LyapunovRequest): Omit<LyapunovResponse, "elapsedMs"> {
  const n = sys.states.length;
  const p = req.params;
  const map = sys.kind === "map";
  const jac = (t: number, x: number[]) => jacobian((v) => sys.f(t, v, p), x, n);
  const h = map ? 1 : !METHODS[req.solver.method].adaptive && req.solver.h > 0 ? req.solver.h : 0.01;
  const per = map ? Math.max(1, Math.round(req.interval)) : req.interval;
  const total = map ? Math.round(req.total) : req.total;
  const ratio = total / per;
  const steps = Math.abs(ratio - Math.round(ratio)) <= 1e-9 * Math.max(1, ratio) ? Math.round(ratio) : Math.ceil(ratio);
  const blocks = Math.max(2, Math.min(req.blocks, steps));
  const every = Math.max(1, Math.ceil(steps / LYAPUNOV_HISTORY));
  const sums = new Array<number>(n).fill(0);
  const blockSums = Array.from({ length: blocks }, () => new Array<number>(n).fill(0));
  const blockSpan = new Array<number>(blocks).fill(0);
  const historyT: number[] = [];
  const history: number[] = [];
  let q: number[] = Array.from({ length: n * n }, (_, k) => (k % (n + 1) === 0 ? 1 : 0));
  let x = req.x0.slice(0, n);
  let t = map ? 0 : req.t0;
  let trace = 0;
  let measured = 0;
  let evals = 0;
  let accepted = 0;
  const bad = (v: number[]) => v.some((c) => !Number.isFinite(c) || Math.abs(c) > 1e12);
  // Tangent flow (x, Φ, ∫tr J): RK4 on the n + n² + 1 variables.
  const tangent = (tt: number, y: number[]): number[] => {
    const xs = y.slice(0, n);
    const j = jac(tt, xs);
    evals += 1 + 2 * n;
    const out = sys.f(tt, xs, p).slice();
    for (let c = 0; c < n; c++)
      for (let r = 0; r < n; r++) {
        let acc = 0;
        for (let k = 0; k < n; k++) acc += j[r * n + k] * y[n + c * n + k];
        out.push(acc);
      }
    let tr = 0;
    for (let i = 0; i < n; i++) tr += j[i * n + i];
    out.push(tr);
    return out;
  };
  const flowFor = (span: number, withQ: boolean): boolean => {
    let y = [...x, ...(withQ ? q : []), 0];
    const f = withQ ? tangent : (tt: number, v: number[]) => [...sys.f(tt, v.slice(0, n), p), 0];
    let left = span;
    while (left > 1e-12 * Math.max(1, span)) {
      const dt = Math.min(h, left);
      const k1 = f(t, y);
      const k2 = f(t + dt / 2, y.map((v, i) => v + (dt / 2) * k1[i]));
      const k3 = f(t + dt / 2, y.map((v, i) => v + (dt / 2) * k2[i]));
      const k4 = f(t + dt, y.map((v, i) => v + dt * k3[i]));
      if (!withQ) evals += 4;
      y = y.map((v, i) => v + (dt / 6) * (k1[i] + 2 * k2[i] + 2 * k3[i] + k4[i]));
      t += dt;
      left -= dt;
      accepted++;
      if (bad(y.slice(0, n))) return false;
    }
    x = y.slice(0, n);
    if (withQ) {
      q = y.slice(n, n + n * n);
      trace += y[n + n * n];
    }
    return true;
  };
  const iterate = (count: number, withQ: boolean): boolean => {
    for (let k = 0; k < count; k++) {
      if (withQ) {
        const j = jac(t, x);
        const next = new Array<number>(n * n).fill(0);
        for (let c = 0; c < n; c++)
          for (let r = 0; r < n; r++) {
            let acc = 0;
            for (let m = 0; m < n; m++) acc += j[r * n + m] * q[c * n + m];
            next[c * n + r] = acc;
          }
        q = next;
        trace += Math.log(Math.abs(determinant(j, n)));
      }
      x = sys.f(t, x, p);
      t += 1;
      evals++;
      accepted++;
      if (bad(x)) return false;
    }
    return true;
  };
  const done = (status: LyapunovResponse["status"], k: number): Omit<LyapunovResponse, "elapsedMs"> => {
    const exponents = measured > 0 ? sums.map((v) => v / measured) : [];
    const reached = blockSpan.filter((v) => v > 0).length;
    const uncertainty =
      reached >= 2
        ? exponents.map((_, i) => {
            const per = blockSums.slice(0, reached).map((b, m) => b[i] / blockSpan[m]);
            const mean = per.reduce((a, v) => a + v, 0) / reached;
            const sd = Math.sqrt(per.reduce((a, v) => a + (v - mean) ** 2, 0) / (reached - 1));
            return sd / Math.sqrt(reached);
          })
        : [];
    return {
      requestId: req.requestId,
      exponents,
      uncertainty,
      sum: exponents.reduce((a, v) => a + v, 0),
      traceAverage: measured > 0 ? trace / measured : 0,
      historyT,
      history,
      renormalizations: k,
      status,
      stats: { accepted, rejected: 0, evals },
    };
  };
  const transient = map ? Math.round(req.transient) : req.transient;
  if (transient > 0 && !(map ? iterate(transient, false) : flowFor(transient, false))) return done("blewUp", 0);
  for (let k = 0; k < steps; k++) {
    const span = Math.min(per, total - k * per);
    if (!(map ? iterate(span, true) : flowFor(span, true))) return done("blewUp", k);
    const block = Math.min(blocks - 1, Math.floor((k * blocks) / steps));
    for (let c = 0; c < n; c++) {
      for (let pass = 0; pass < 2; pass++) {
        for (let b = 0; b < c; b++) {
          let dot = 0;
          for (let r = 0; r < n; r++) dot += q[b * n + r] * q[c * n + r];
          for (let r = 0; r < n; r++) q[c * n + r] -= dot * q[b * n + r];
        }
      }
      let norm = 0;
      for (let r = 0; r < n; r++) norm += q[c * n + r] ** 2;
      norm = Math.sqrt(norm);
      for (let r = 0; r < n; r++) q[c * n + r] /= norm;
      const ln = Math.log(norm);
      sums[c] += ln;
      blockSums[block][c] += ln;
    }
    measured += span;
    blockSpan[block] += span;
    if ((k + 1) % every === 0 || k === steps - 1) {
      historyT.push(t);
      for (let c = 0; c < n; c++) history.push(sums[c] / measured);
    }
  }
  return done("completed", steps);
}

function determinant(a: number[], n: number): number {
  const m = a.slice();
  let det = 1;
  for (let c = 0; c < n; c++) {
    let piv = c;
    for (let r = c + 1; r < n; r++) if (Math.abs(m[r * n + c]) > Math.abs(m[piv * n + c])) piv = r;
    if (m[piv * n + c] === 0) return 0;
    if (piv !== c) {
      for (let k = 0; k < n; k++) [m[c * n + k], m[piv * n + k]] = [m[piv * n + k], m[c * n + k]];
      det = -det;
    }
    det *= m[c * n + c];
    for (let r = c + 1; r < n; r++) {
      const f = m[r * n + c] / m[c * n + c];
      for (let k = c; k < n; k++) m[r * n + k] -= f * m[c * n + k];
    }
  }
  return det;
}

/** The checks `cobweb_fits` makes in service.rs, with the same messages. */
function cobwebError(sys: MockSystem, xAxis: AxisRef): string | null {
  const dim = sys.states.length;
  if (sys.kind !== "map") return "The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system is a flow.";
  if (dim !== 1) return `The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system has ${dim} states.`;
  if (sys.autonomous === false) return "This map changes with n, so f has no single graph to cobweb.";
  if (xAxis.kind !== "state" || xAxis.index !== 0) return "The cobweb view needs x_n on the horizontal axis.";
  return null;
}

function thirdAxisError(dim: number, req: PortraitRequest): string | null {
  const { zAxis, zRange } = req;
  if (!zAxis && !zRange) return null;
  if (zAxis?.kind === "time") return "The third axis of a 3D view must be a state.";
  if (!zAxis || !zRange) return "A 3D view needs both a third axis and its range.";
  if (zAxis.index >= dim) return `The third axis shows state ${zAxis.index + 1} but the system only has ${dim}.`;
  const [lo, hi] = zRange;
  if (!(Number.isFinite(lo) && Number.isFinite(hi) && lo < hi)) return `The third axis range [${lo}, ${hi}] is not a valid interval.`;
  return null;
}

function sectionError(sys: MockSystem, section: SectionSpec): string | null {
  const dim = sys.states.length;
  if (sys.kind === "map") return "A Poincaré section needs a flow; this system is a map.";
  if (section.kind === "plane") {
    if (section.state >= dim) return `The section's state is out of range: this system has ${dim} ${dim === 1 ? "state" : "states"}.`;
    if (!Number.isFinite(section.value)) return "The section's value must be a finite number.";
  } else {
    if (!(section.period > 0 && Number.isFinite(section.period))) return "The stroboscopic period must be a positive number.";
    if (!Number.isFinite(section.phase)) return "The stroboscopic phase must be a finite number.";
  }
  return null;
}


/** MAX_GRAPH_SAMPLES in dyn-analysis. */
const MAX_GRAPH_SAMPLES = 4096;

/**
 * A simple port of `dyn_analysis::map_graph`: `samples` evenly spaced x (both
 * ends), split at a non-finite value and where a rise of more than a quarter
 * of the view has a midpoint that is non-finite or larger than both ends (a
 * pole), then clipped to the view grown by its height above and below.
 */
function mockMapGraph(
  f: (x: number) => number,
  [x0, x1]: [number, number],
  [y0, y1]: [number, number],
  samples: number,
): [number, number][][] {
  const n = Math.min(MAX_GRAPH_SAMPLES, Math.max(2, Math.round(samples)));
  const span = y1 - y0;
  const lines: [number, number][][] = [];
  let line: [number, number][] = [];
  const cut = () => {
    if (line.length >= 2) lines.push(line);
    line = [];
  };
  let prev: [number, number] | null = null;
  for (let i = 0; i < n; i++) {
    const x = i === n - 1 ? x1 : x0 + (x1 - x0) * (i / (n - 1));
    const y = f(x);
    if (!Number.isFinite(y)) {
      cut();
      prev = null;
      continue;
    }
    if (prev && Math.abs(y - prev[1]) > 0.25 * span) {
      const m = f((prev[0] + x) / 2);
      if (!Number.isFinite(m) || Math.abs(m) > Math.max(Math.abs(y), Math.abs(prev[1]))) cut();
    }
    if (!line.length || x > line[line.length - 1][0]) line.push([x, y]);
    prev = [x, y];
  }
  cut();
  return lines.flatMap((l) => clipToBand(l, y0 - span, y1 + span));
}

/** Port of `clip_to_band` in dyn-analysis: the parts of a polyline with lo ≤ y ≤ hi, crossings interpolated. */
function clipToBand(line: [number, number][], lo: number, hi: number): [number, number][][] {
  const inside = (y: number) => y >= lo && y <= hi;
  const edge = (y: number) => (y > hi ? hi : lo);
  const cross = (p: [number, number], q: [number, number], y: number): [number, number] => [
    p[0] + ((y - p[1]) / (q[1] - p[1])) * (q[0] - p[0]),
    y,
  ];
  const out: [number, number][][] = [];
  let cur: [number, number][] = [];
  const push = (pt: [number, number]) => {
    if (!cur.length || pt[0] > cur[cur.length - 1][0]) cur.push(pt);
  };
  const finish = () => {
    if (cur.length >= 2) out.push(cur);
    cur = [];
  };
  if (line.length && inside(line[0][1])) cur.push(line[0]);
  for (let k = 1; k < line.length; k++) {
    const p = line[k - 1];
    const q = line[k];
    const [pin, qin] = [inside(p[1]), inside(q[1])];
    if (pin && qin) push(q);
    else if (pin) {
      push(cross(p, q, edge(q[1])));
      finish();
    } else if (qin) {
      push(cross(p, q, edge(p[1])));
      push(q);
    } else if (p[1] > hi !== q[1] > hi) {
      push(cross(p, q, edge(p[1])));
      push(cross(p, q, edge(q[1])));
      finish();
    }
  }
  finish();
  return out;
}

async function portrait(req: PortraitRequest): Promise<PortraitResponse> {
  const start = performance.now();
  const sys = byId.get(req.systemId);
  if (!sys) throw new Error(`Unknown system ${req.systemId}`);
  const dim = sys.states.length;
  const p = req.params;
  if (req.graphSamples !== undefined) {
    const err = cobwebError(sys, req.xAxis);
    if (err) throw err;
  }
  if (req.section) {
    const err = sectionError(sys, req.section);
    if (err) throw err;
  }
  const zErr = thirdAxisError(dim, req);
  if (zErr) throw zErr;
  const methodErr = methodError(sys, req.solver.method);
  if (methodErr) throw methodErr;

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
  const transient = req.transient ?? 0;
  const section = sys.kind === "ode" ? req.section : undefined;
  // The preview has no Dormand–Prince to test, so it reports what the core measured (`stiffAt`).
  const stiff = sys.kind === "ode" && req.solver.method === "dopri5" && (sys.stiffAt?.(p) ?? false);
  const trajectories = req.trajectories.map((tr) => {
    const out = withEnergy(
      sys,
      p,
      tr,
      section
        ? crossings(sys, p, tr, req.solver.maxSteps, section, transient)
        : solve(sys, p, tr, req.samples, req.solver.maxSteps, transient),
    );
    return stiff ? { ...out, stiff: true } : out;
  });
  const graph =
    req.graphSamples === undefined
      ? undefined
      : mockMapGraph((x) => sys.f(0, [x], p)[0], req.xRange, req.yRange, req.graphSamples);

  return {
    requestId: req.requestId,
    arrows,
    nullclines,
    fixedPoints,
    trajectories,
    ...(graph ? { graph } : {}),
    elapsedMs: performance.now() - start,
  };
}

/** `method_fits` in service.rs, with its messages. The preview integrates every method with RK4. */
function methodError(sys: MockSystem, method: Method): string | null {
  if (METHODS[method].needs === "any" || sys.kind === "map") return null;
  if (!sys.hamiltonian) {
    return `${METHODS[method].label} needs a Hamiltonian H(q, p); this system is given by its equations of motion.`;
  }
  if (!sys.energy) return "H(q, p) must split as T(p) + V(q) for a symplectic method.";
  return null;
}

/** `energy` in service.rs: H at the start and its largest drift over the samples. */
function withEnergy(sys: MockSystem, p: number[], tr: TrajectoryRequest, out: TrajectoryOut): TrajectoryOut {
  if (!sys.energy) return out;
  const dim = sys.states.length;
  const h0 = sys.energy(tr.x0, p);
  let maxAbsDrift = 0;
  for (let i = 0; i < out.t.length; i++) {
    maxAbsDrift = Math.max(maxAbsDrift, Math.abs(sys.energy(out.x.slice(i * dim, (i + 1) * dim), p) - h0));
  }
  return Number.isFinite(h0) && Number.isFinite(maxAbsDrift) ? { ...out, energy: { h0, maxAbsDrift } } : out;
}

/** MAX_ITERATES in app/src-tauri/src/service.rs. */
const MAX_ITERATES = 1_000_000;

function solve(
  sys: MockSystem,
  p: number[],
  tr: TrajectoryRequest,
  samples: number,
  maxSteps: number,
  transient: number,
): TrajectoryOut {
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
    // As `iterate_map`: x_T … x_{T+N}, the first T iterates computed but not returned.
    const n = Number.isFinite(tr.t1) ? Math.min(MAX_ITERATES, Math.max(0, Math.round(tr.t1))) : 0;
    const skip = Math.min(Math.max(0, Math.round(transient)), MAX_ITERATES - n);
    // As the core counts: an evaluation per step tried, an accepted step per finite iterate.
    const stats = (k: number, evals = k) => ({ accepted: k, rejected: 0, evals });
    if (!x.every(Number.isFinite)) return { t: [], x: [], status: "nonFinite", stats: stats(0) };
    if (skip === 0) push(0, x);
    for (let k = 0; k < skip + n; k++) {
      const next = sys.f(k, x, p);
      if (!next.every(Number.isFinite)) return { t: ts, x: xs, status: "nonFinite", stats: stats(k, k + 1) };
      x = next;
      if (k + 1 >= skip) push(k + 1, x);
      if (tooBig(x)) return { t: ts, x: xs, status: "blewUp", stats: stats(k + 1) };
    }
    return { t: ts, x: xs, status: "completed", stats: stats(skip + n) };
  }

  const span = tr.t1 - tr.t0;
  const hMax = 0.01;
  // Steps of at most hMax cover the span in `needed`; like the core, a run stops at the step limit.
  const needed = Math.max(1, Math.ceil(Math.abs(span) / hMax));
  const steps = Math.min(needed, Math.max(1, maxSteps));
  const h = span / needed;
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
  const status = steps < needed ? "maxSteps" : "completed";
  return { t: ts, x: xs, status, stats: { accepted: steps, rejected: 0, evals: 4 * steps } };
}

function rk4(sys: MockSystem, p: number[], t: number, x: number[], h: number): number[] {
  const k1 = sys.f(t, x, p);
  const k2 = sys.f(t + h / 2, x.map((v, i) => v + (h / 2) * k1[i]), p);
  const k3 = sys.f(t + h / 2, x.map((v, i) => v + (h / 2) * k2[i]), p);
  const k4 = sys.f(t + h, x.map((v, i) => v + h * k3[i]), p);
  return x.map((v, i) => v + (h / 6) * (k1[i] + 2 * k2[i] + 2 * k3[i] + k4[i]));
}

function crossings(
  sys: MockSystem,
  p: number[],
  tr: TrajectoryRequest,
  maxSteps: number,
  section: SectionSpec,
  transient: number,
): TrajectoryOut {
  const dim = sys.states.length;
  const ts: number[] = [];
  const xs: number[] = [];
  let seen = 0;
  const skip = Math.max(0, Math.round(transient));
  const full = () => seen >= skip + MAX_SECTION_POINTS;
  const take = (t: number, v: number[]) => {
    if (seen++ >= skip) {
      ts.push(t);
      xs.push(...v);
    }
  };
  const lerp = (a: number[], b: number[], u: number) => a.map((v, i) => v + u * (b[i] - v));
  const span = tr.t1 - tr.t0;
  const needed = Math.max(1, Math.ceil(Math.abs(span) / 0.01));
  const steps = Math.min(needed, Math.max(1, maxSteps));
  const h = span / needed;
  let t = tr.t0;
  let x = tr.x0.slice(0, dim);
  const g = (v: number[]) => (section.kind === "plane" ? v[section.state] - section.value : 0);
  let k = section.kind === "stroboscopic" ? Math.ceil((tr.t0 - section.phase) / section.period) : 0;
  const strobe = (n: number) => (section.kind === "stroboscopic" ? section.phase + n * section.period : NaN);
  if (section.kind === "stroboscopic" && strobe(k) === t) {
    take(t, x);
    k++;
  }
  for (let s = 1; s <= steps && !full(); s++) {
    const next = rk4(sys, p, t, x, h);
    const tn = tr.t0 + s * h;
    if (next.some((c) => !Number.isFinite(c) || Math.abs(c) > 1e12))
      return { t: ts, x: xs, status: "blewUp", stats: { accepted: s, rejected: 0, evals: 4 * s } };
    if (section.kind === "plane") {
      const [a, b] = [g(x), g(next)];
      const up = a < 0 && b >= 0;
      const down = a > 0 && b <= 0;
      const dir = section.direction;
      if ((up && dir !== "falling") || (down && dir !== "rising")) {
        const u = a / (a - b);
        take(t + u * h, lerp(x, next, u));
      }
    } else {
      while (strobe(k) <= tn && !full()) {
        const u = (strobe(k) - t) / h;
        take(strobe(k), lerp(x, next, u));
        k++;
      }
    }
    t = tn;
    x = next;
  }
  const status = steps < needed && !full() ? "maxSteps" : "completed";
  return { t: ts, x: xs, status, stats: { accepted: steps, rejected: 0, evals: 4 * steps } };
}

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
  const z = req.zAxis?.kind === "state" && req.zRange ? { index: req.zAxis.index, range: req.zRange } : null;
  const zSeeds = z ? 5 : 1;
  for (let i = 0; i < seeds; i++) {
    for (let j = 0; j < seeds; j++) {
      for (let l = 0; l < zSeeds; l++) {
        const px = req.xRange[0] + ((i + 0.5) / seeds) * (req.xRange[1] - req.xRange[0]);
        const py = req.yRange[0] + ((j + 0.5) / seeds) * (req.yRange[1] - req.yRange[0]);
        const start = lift(px, py)[1];
        if (z) start[z.index] = z.range[0] + ((l + 0.5) / zSeeds) * (z.range[1] - z.range[0]);
        const root = newton(g, start, dim);
        if (!root) continue;
        const scale = 1e-6 * (1 + Math.hypot(...root));
        if (!found.some((q) => Math.hypot(...q.map((v, k) => v - root[k])) < scale)) found.push(root);
      }
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

function eigenvalues(j: number[], n: number): [number, number][] {
  if (n === 1) return [[j[0], 0]];
  if (n === 2) {
    const tr = j[0] + j[3];
    const det = j[0] * j[3] - j[1] * j[2];
    return quadratic(1, -tr, det);
  }
  if (n === 3) {
    const [c2, c1, c0] = charPolyCoefficients(j);
    const p = (l: number) => ((l - c2) * l + c1) * l - c0;
    const r = realRootByBisection(p, 1 + Math.max(Math.abs(c2), Math.abs(c1), Math.abs(c0)));
    const [b, c] = deflate(r, c2, c1);
    return [[r, 0], ...quadratic(1, b, c)];
  }
  return [];
}

/** [c2, c1, c0] with det(λI - J) = λ³ - c2 λ² + c1 λ - c0 for a 3x3 J. */
function charPolyCoefficients(j: number[]): [number, number, number] {
  const c2 = j[0] + j[4] + j[8];
  const c1 = j[0] * j[4] - j[1] * j[3] + j[0] * j[8] - j[2] * j[6] + j[4] * j[8] - j[5] * j[7];
  const c0 =
    j[0] * (j[4] * j[8] - j[5] * j[7]) - j[1] * (j[3] * j[8] - j[5] * j[6]) + j[2] * (j[3] * j[7] - j[4] * j[6]);
  return [c2, c1, c0];
}

/** A real root of the cubic p in [-bound, bound], where p changes sign. */
function realRootByBisection(p: (l: number) => number, bound: number): number {
  let lo = -bound;
  let hi = bound;
  for (let k = 0; k < 200; k++) {
    const mid = (lo + hi) / 2;
    if (Math.sign(p(mid)) === Math.sign(p(lo))) lo = mid;
    else hi = mid;
  }
  return (lo + hi) / 2;
}

/** [b, c] with λ³ - c2 λ² + c1 λ - c0 = (λ - r)(λ² + b λ + c). */
function deflate(r: number, c2: number, c1: number): [number, number] {
  const b = r - c2;
  return [b, c1 + r * b];
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

type C = [number, number];
const cmul = (a: C, b: C): C => [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]];
const csub = (a: C, b: C): C => [a[0] - b[0], a[1] - b[1]];
const cnorm = (v: C[]) => Math.sqrt(v.reduce((acc, z) => acc + z[0] * z[0] + z[1] * z[1], 0));

function ccross(a: C[], b: C[]): C[] {
  return [
    csub(cmul(a[1], b[2]), cmul(a[2], b[1])),
    csub(cmul(a[2], b[0]), cmul(a[0], b[2])),
    csub(cmul(a[0], b[1]), cmul(a[1], b[0])),
  ];
}

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
  const r = m.reduce((a, b) => (cnorm(b) > cnorm(a) ? b : a));
  const u = [0, 1, 2].map((k) => ccross(r, e(k))).reduce((a, b) => (cnorm(b) > cnorm(a) ? b : a));
  return [u, ccross(r, u)];
}

function dominantIndex(mags: number[]): number {
  const max = Math.max(...mags);
  return mags.findIndex((v) => v >= (1 - 1e-9) * max);
}

/**
 * Rotate the complex phase of w so its real and imaginary parts are the
 * semi-axes of the ellipse (major first); a circle gets its largest
 * component made real.
 */
function rotateToSemiAxes(w: C[]): C[] {
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
  return w.map((z) => cmul(z, rot));
}

function normaliseVector(v: C[], complex: boolean): { re: number[]; im: number[] | null } {
  const len = cnorm(v);
  let w = v.map(([a, b]): C => [a / len, b / len]);
  if (complex) w = rotateToSemiAxes(w);
  const k = dominantIndex(w.map(([a]) => Math.abs(a)));
  if (w[k][0] < 0) w = w.map(([a, b]): C => [-a, -b]);
  return { re: w.map(([a]) => a), im: complex ? w.map(([, b]) => b) : null };
}

/**
 * Bisection plus deflation finds a repeated 3x3 root only to about 1e-5
 * (a triple root converges like eps^(1/3)), so roots this close (relative)
 * are one repeated eigenvalue.
 */
const REPEATED_ROOT_TOL = 1e-4;

function clusterMean(cluster: [number, number][]): [number, number] {
  const n = cluster.length;
  return [cluster.reduce((acc, e) => acc + e[0], 0) / n, cluster.reduce((acc, e) => acc + e[1], 0) / n];
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
  const same = (a: [number, number], b: [number, number]) =>
    Math.hypot(a[0] - b[0], a[1] - b[1]) <= REPEATED_ROOT_TOL * Math.max(scale, 1e-300);
  for (let k = 0; k < eig.length; k++) {
    const occurrence = eig.slice(0, k).filter((e) => same(e, eig[k])).length;
    const [lr, li] = clusterMean(eig.filter((e) => same(e, eig[k])));
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
    return { classification: "saddle (n-D)", stable };
  }
  const stable = eig.every(([re]) => re < -eps);
  if (dim === 1) {
    const l = eig[0][0];
    if (Math.abs(l) < eps) return { classification: "non-hyperbolic", stable: false };
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

// ---------------------------------------------------------------------
// Project files, exports and the clipboard in the browser preview.
//
// There is no file system here: "files" live in memory, in
// `window.__sepxMock` (which the e2e tests drive), and the dialogs return
// what it says. Exports return placeholder LaTeX built from the preview's
// own portrait, so the dialog can be exercised; the real snippets are
// built in Rust (docs/exports.md).
// ---------------------------------------------------------------------

/** The browser preview's stand-in for the file system, dialogs and clipboard. */
export interface SepxMock {
  /** Project files by path (their text). */
  files: Map<string, string>;
  /** What the next Open dialog returns (`null`: cancelled); cleared after use. */
  nextOpenPath?: string | null;
  /** What the next Save dialog returns (`null`: cancelled); cleared after use. */
  nextSavePath?: string | null;
  /** Paths "passed on the command line", returned once by `launch_ready`. */
  launchFiles: string[];
  /** The last text copied. */
  clipboardText?: string;
  /** Size of the last image copied, in bytes. */
  clipboardPngBytes?: number;
  /** Every export saved, in order. */
  saved: { name: string; contents: string | Uint8Array }[];
  /** The last `set_document_state`. */
  document: DocumentStateRequest;
  /** The last `compute_portrait` (the canvas's or a gallery thumbnail's; never the export dialog's). */
  lastPortrait?: { req: PortraitRequest; res: PortraitResponse };
  /** The last `compute_lyapunov`. */
  lastLyapunov?: { req: LyapunovRequest; res: LyapunovResponse };
  /** The last `compute_orbit_diagram`. */
  lastOrbit?: { req: OrbitDiagramRequest; res: OrbitDiagramResponse };
}

function sepxMock(): SepxMock {
  const g = globalThis as unknown as { __sepxMock?: SepxMock };
  if (!g.__sepxMock) {
    g.__sepxMock = {
      files: new Map(),
      launchFiles: [],
      saved: [],
      document: { title: "Separatrix", dirty: false },
    };
  }
  return g.__sepxMock;
}

if (typeof window !== "undefined") sepxMock();

const fileName = (path: string) => path.split(/[\\/]/).pop() || path;
const q = (name: string) => `“${name}”`;

function takeDialog(key: "nextOpenPath" | "nextSavePath", fallback: string | null): string | null {
  const m = sepxMock();
  const v = m[key];
  delete m[key];
  return v === undefined ? fallback : v;
}

function withExt(name: string, ext: string): { path: string; appended: boolean } {
  return name.toLowerCase().endsWith(`.${ext}`) ? { path: name, appended: false } : { path: `${name}.${ext}`, appended: true };
}

export async function mockProjectChooseOpenPath(): Promise<string | null> {
  const first = sepxMock().files.keys().next();
  return takeDialog("nextOpenPath", first.done ? null : first.value);
}

export async function mockProjectRead(req: ProjectReadRequest): Promise<OpenedProject> {
  const name = fileName(req.path);
  const text = sepxMock().files.get(req.path);
  if (text === undefined) throw `${q(name)} can't be found. It may have been moved or deleted.`;
  let doc: unknown;
  try {
    doc = JSON.parse(text);
  } catch {
    throw `${q(name)} isn't a Separatrix project: it isn't valid JSON.`;
  }
  const d = doc as Partial<ProjectFile> | null;
  if (!d || typeof d !== "object" || d.format !== "separatrix") {
    throw `${q(name)} isn't a Separatrix project.`;
  }
  if ((d.min_reader_version ?? d.format_version ?? 1) > 1) {
    throw `${q(name)} needs a newer version of Separatrix to open. Update Separatrix, then open it again.`;
  }
  const newer = (d.format_version ?? 1) > 1 ? d.format_version : undefined;
  return {
    path: req.path,
    name,
    project: d as ProjectFile,
    warnings: newer ? [`${q(name)} was saved in a newer format. It opens, and saving keeps the parts this version doesn't use.`] : [],
    ...(newer ? { newerFormat: newer } : {}),
  };
}

export async function mockProjectChooseSavePath(req: ProjectChooseSaveRequest): Promise<ChosenSavePath | null> {
  const dir = req.currentPath ? req.currentPath.replace(/[^\\/]*$/, "") : "/mock/";
  const suggested = dir + withExt(fileName(req.suggestedName) || "untitled", "sepx").path;
  const chosen = takeDialog("nextSavePath", suggested);
  if (chosen === null) return null;
  const { path, appended } = withExt(chosen, "sepx");
  return { path, appendedExists: appended && sepxMock().files.has(path) };
}

export async function mockProjectWrite(req: ProjectWriteRequest): Promise<SavedProject> {
  const name = fileName(req.path);
  if (!req.path.toLowerCase().endsWith(".sepx")) throw `Projects are saved as .sepx files, not ${q(name)}.`;
  const p = req.project as Record<string, unknown> | null;
  if (!p || typeof p !== "object") throw "The project can't be saved: it isn't an object.";
  const stamped = { ...p, format: "separatrix", format_version: Math.max(1, Number(p.format_version) || 1), min_reader_version: 1, app_version: "0.2.0" };
  sepxMock().files.set(req.path, JSON.stringify(stamped, null, 2) + "\n");
  return { path: req.path, name };
}

export async function mockLaunchReady(): Promise<string[]> {
  const m = sepxMock();
  const out = m.launchFiles;
  m.launchFiles = [];
  return out;
}

export async function mockSetDocumentState(req: DocumentStateRequest): Promise<void> {
  sepxMock().document = { ...req };
  if (typeof document !== "undefined") document.title = req.title;
}

/** The browser can't close its own tab; the preview does nothing. */
export async function mockCloseWindow(): Promise<void> {}

export async function mockComputeExportPortrait(req: ExportPortraitRequest): Promise<PortraitResponse> {
  return portrait(req.portrait);
}

const PREVIEW_NOTE = "% Browser preview: placeholder LaTeX. The desktop app writes the real snippet.";

function mockSystemOf(systemId: number): MockSystem {
  const sys = byId.get(systemId);
  if (!sys) throw `System ${systemId} is no longer available; edit the equations to recompile.`;
  return sys;
}

function mockAlignRows(sys: MockSystem): string[] {
  return sys.equationsLatex.map((e) => {
    const k = e.indexOf("=");
    return k < 0 ? e : `${e.slice(0, k).trim()} &= ${e.slice(k + 1).trim()}`;
  });
}

export async function mockExportSystemLatex(req: SystemLatexRequest): Promise<SystemLatexResponse> {
  const sys = mockSystemOf(req.systemId);
  const rows = mockAlignRows(sys);
  const env = req.numbered ? "align" : "align*";
  const tex = [
    `% ${req.title}${req.sourceRef ? ` (${req.sourceRef})` : ""}`,
    PREVIEW_NOTE,
    "% Needs: \\usepackage{amsmath}",
    `\\begin{${env}}`,
    rows.map((r) => `  ${r}`).join(" \\\\\n"),
    `\\end{${env}}`,
    "",
  ].join("\n");
  return { tex, previewLatex: `\\begin{aligned}${rows.join("\\\\")}\\end{aligned}` };
}

export async function mockExportFixedPointsLatex(req: FixedPointsLatexRequest): Promise<string> {
  const sys = mockSystemOf(req.portrait.systemId);
  const res = await portrait({
    ...req.portrait,
    fixedPoints: true,
    trajectories: [],
    fieldGrid: 0,
    nullclines: false,
    graphSamples: undefined,
  });
  const digits = Math.min(8, Math.max(2, req.digits));
  const num = (v: number) => `\\num{${Number(v.toPrecision(digits))}}`;
  const rows = res.fixedPoints.map((fp) => {
    const point = `$(${fp.x.map(num).join(", ")})$`;
    const eig = fp.eigenvalues.map(([re, im]) => `$${num(re)}${im ? ` ${im < 0 ? "-" : "+"} ${num(Math.abs(im))}\\,i` : ""}$`).join(", ");
    return `    ${point} & ${eig} & ${fp.classification} \\\\`;
  });
  return [
    `% ${req.title}: fixed points`,
    PREVIEW_NOTE,
    "% Needs: \\usepackage{amsmath} \\usepackage{booktabs} \\usepackage{siunitx}",
    "\\begin{table}",
    "  \\centering",
    "  \\begin{tabular}{@{}lll@{}}",
    "    \\toprule",
    `    Point & Eigenvalues & Type \\\\`,
    "    \\midrule",
    ...rows,
    "    \\bottomrule",
    "  \\end{tabular}",
    `  \\caption{Fixed points of the system (${sys.states.length}D).}`,
    "\\end{table}",
    "",
  ].join("\n");
}

function mockAxisValue(axis: AxisRef, t: number[], x: number[], dim: number, k: number): number {
  return axis.kind === "time" ? t[k] : x[k * dim + axis.index];
}

export async function mockExportFigureLatex(req: FigureLatexRequest): Promise<FigureLatexResponse> {
  const sys = mockSystemOf(req.portrait.systemId);
  const dim = sys.states.length;
  const res = await portrait(req.portrait);
  let points = 0;
  if (req.portrait.graphSamples !== undefined) {
    // A cobweb, with the real figure's comment markers (docs/exports.md).
    const [x0, x1] = req.portrait.xRange;
    const [y0, y1] = req.portrait.yRange;
    const pt = (a: number, b: number) => `(${String(a)},${String(b)})`;
    const plots: string[] = [];
    const [a, b] = [Math.max(x0, y0), Math.min(x1, y1)];
    if (a < b) {
      plots.push("% Diagonal x_{n+1} = x_n", `  \\addplot[no marks] coordinates {${pt(a, a)} ${pt(b, b)}};`);
      points += 2;
    }
    plots.push("% Graph of f");
    for (const line of res.graph ?? []) {
      plots.push(`  \\addplot[no marks] coordinates {${line.map(([gx, gy]) => pt(gx, gy)).join(" ")}};`);
      points += line.length;
    }
    res.trajectories.forEach((tr, k) => {
      const v = tr.x.slice(0, tr.t.length * dim);
      const cut = v.findIndex((c) => !Number.isFinite(c));
      const orbit = cut < 0 ? v : v.slice(0, cut);
      if (!orbit.length) return;
      const coords = [pt(orbit[0], orbit[0])];
      for (let i = 1; i < orbit.length; i++) coords.push(pt(orbit[i - 1], orbit[i]), pt(orbit[i], orbit[i]));
      points += coords.length;
      plots.push(`% Cobweb of ${req.tags[k]?.icId ?? `ic-${k + 1}`}`);
      plots.push(`  \\addplot[no marks] coordinates {${coords.join(" ")}};`);
    });
    return { tex: mockFigureTex(req, plots), points, thinned: false, notes: [] };
  }
  const plots = res.trajectories.map((tr) => {
    const coords: string[] = [];
    const step = Math.max(1, Math.floor(tr.t.length / 200));
    for (let k = 0; k < tr.t.length; k += step) {
      const a = mockAxisValue(req.portrait.xAxis, tr.t, tr.x, dim, k);
      const b = mockAxisValue(req.portrait.yAxis, tr.t, tr.x, dim, k);
      if (Number.isFinite(a) && Number.isFinite(b)) coords.push(`(${String(a)},${String(b)})`);
    }
    points += coords.length;
    return `  \\addplot[no marks] coordinates {${coords.join(" ")}};`;
  });
  return { tex: mockFigureTex(req, plots), points, thinned: false, notes: [] };
}

/** The placeholder figure around `plots`. */
function mockFigureTex(req: FigureLatexRequest, plots: string[]): string {
  const [x0, x1] = req.portrait.xRange;
  const [y0, y1] = req.portrait.yRange;
  const axis = [
    "\\begin{tikzpicture}",
    `\\begin{axis}[width=${req.widthCm}cm, height=${req.heightCm}cm, scale only axis, xmin=${x0}, xmax=${x1}, ymin=${y0}, ymax=${y1}]`,
    ...plots,
    "\\end{axis}",
    "\\end{tikzpicture}",
  ];
  const preamble = "\\usepackage{pgfplots} \\pgfplotsset{compat=1.18}";
  return [
    `% ${req.title}: figure`,
    PREVIEW_NOTE,
    `% Needs: ${preamble}`,
    ...(req.standalone
      ? ["\\documentclass[border=2mm]{standalone}", ...preamble.split(" "), "\\begin{document}", ...axis, "\\end{document}"]
      : axis),
    "",
  ].join("\n");
}

export async function mockExportCsv(req: CsvRequest): Promise<CsvResponse> {
  const sys = mockSystemOf(req.portrait.systemId);
  if (req.tags.length !== req.portrait.trajectories.length) {
    throw `Expected one tag per trajectory (${req.portrait.trajectories.length}) but got ${req.tags.length}.`;
  }
  const dim = sys.states.length;
  const res = await portrait({ ...req.portrait, fixedPoints: false, fieldGrid: 0, nullclines: false, graphSamples: undefined });
  const ids = req.icIds.length ? req.icIds : [...new Set(req.tags.map((t) => t.icId))];
  const slug = (s: string) =>
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "system";
  const header = [sys.kind === "map" ? "n" : "t", ...sys.states.map((s) => s.name)];
  const files: CsvFileOut[] = [];
  // Numbered as the dialog lists them, by first appearance among the runs (as Rust does).
  const order = [...new Set(req.tags.map((t) => t.icId))];
  for (const id of ids) {
    const run = (backward: boolean) => {
      const k = req.tags.findIndex((t) => t.icId === id && t.backward === backward);
      return k < 0 ? null : res.trajectories[k];
    };
    const back = run(true);
    const fwd = run(false);
    if (!back && !fwd) continue;
    const rows: string[] = [];
    const row = (tr: TrajectoryOut, k: number) =>
      [tr.t[k], ...tr.x.slice(k * dim, (k + 1) * dim)].map((v) => String(v)).join(",");
    if (back) for (let k = back.t.length - 1; k >= 0; k--) rows.push(row(back, k));
    if (fwd) for (let k = back && fwd.t.length ? 1 : 0; k < fwd.t.length; k++) rows.push(row(fwd, k));
    const stem = `${slug(req.title)}-trajectory-${order.indexOf(id) + 1}`;
    const meta = {
      format: "separatrix-trajectory",
      format_version: 1,
      app_version: "0.2.0",
      title: req.title,
      csv: `${stem}.csv`,
      ic_id: id,
      system: { kind: sys.kind, input_format: req.format, lines: req.lines },
      columns: header,
      ...(sys.kind === "map" ? { transient: Math.max(0, Math.round(req.portrait.transient ?? 0)) } : {}),
      note: "Browser preview: computed by the preview's own solver.",
    };
    files.push({ stem, csv: [header.join(","), ...rows].join("\n") + "\n", meta: JSON.stringify(meta, null, 2) + "\n" });
  }
  return { files };
}

const mockSlug = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || "system";

/** `export_lyapunov_latex`: the real table's rows and caption words, from the preview's own run. */
export async function mockExportLyapunovLatex(req: LyapunovExportRequest): Promise<string> {
  const sys = mockSystemOf(req.run.systemId);
  const res = await mockLyapunov(req.run);
  const cell = (v: number, u: number | undefined) => {
    const r = roundWithUncertainty(v, u);
    return r.error === null ? `\\num{${r.value}}` : `\\num{${r.value} +- ${r.error}}`;
  };
  const o = req.run.solver;
  const how =
    sys.kind === "map"
      ? `the map iterated $${req.run.total}$ times after a transient of $${req.run.transient}$ iterates`
      : `${METHODS[o.method].label.replace("–", "--")} with ${
          METHODS[o.method].adaptive
            ? `relative tolerance $${o.rtol}$ and absolute tolerance $${o.atol}$`
            : `step $h = ${o.h}$`
        }, total time $T = ${req.run.total}$ after a transient of $${req.run.transient}$`;
  return [
    `% Separatrix Lyapunov spectrum: ${req.title}`,
    PREVIEW_NOTE,
    "% Needs: \\usepackage{amsmath} \\usepackage{booktabs} \\usepackage{siunitx}",
    "\\begin{table}",
    "\\centering",
    "\\sisetup{uncertainty-mode = separate}",
    `\\caption{Lyapunov spectrum of ${req.title}: ${how}, \\num{${res.renormalizations}} renormalizations every $\\tau = ${req.run.interval}$.}`,
    "\\begin{tabular}{@{}ll@{}}",
    "\\toprule",
    "Exponent & Value \\\\",
    "\\midrule",
    ...res.exponents.map((v, i) => `$\\lambda_{${i + 1}}$ & ${cell(v, res.uncertainty[i])} \\\\`),
    "\\midrule",
    `$\\sum_i \\lambda_i$ & \\num{${Number(res.sum.toPrecision(6))}} \\\\`,
    "\\bottomrule",
    "\\end{tabular}",
    "\\end{table}",
    "",
  ].join("\n");
}

/** `export_lyapunov_csv`: `t,lambda_1,…` from the preview's own run. */
export async function mockExportLyapunovCsv(req: LyapunovExportRequest): Promise<CsvFileOut> {
  const sys = mockSystemOf(req.run.systemId);
  const res = await mockLyapunov(req.run);
  const n = res.exponents.length;
  const header = [sys.kind === "map" ? "n" : "t", ...res.exponents.map((_, i) => `lambda_${i + 1}`)];
  const rows = res.historyT.map((t, k) => [t, ...res.history.slice(k * n, (k + 1) * n)].map(String).join(","));
  const stem = `${mockSlug(req.title)}-lyapunov`;
  const meta = { format: "separatrix-lyapunov", format_version: 1, title: req.title, csv: `${stem}.csv`, columns: header, note: "Browser preview." };
  return { stem, csv: [header.join(","), ...rows].join("\n") + "\n", meta: JSON.stringify(meta, null, 2) + "\n" };
}

/** The finite iterates of a diagram as `[r, n, x]`. */
function mockOrbitRows(req: OrbitExportRequest, res: OrbitDiagramResponse): [number, number, number][] {
  const out: [number, number, number][] = [];
  const kept = req.diagram.kept;
  res.r.forEach((r, j) => {
    for (let k = 0; k < kept; k++) {
      const x = res.x[j * kept + k];
      if (x !== null && Number.isFinite(x)) out.push([r, req.diagram.transient + k, x]);
    }
  });
  return out;
}

/** `export_orbit_csv`. */
export async function mockExportOrbitCsv(req: OrbitExportRequest): Promise<CsvFileOut> {
  const sys = mockSystemOf(req.diagram.systemId);
  const res = await mockOrbitDiagram(req.diagram);
  const header = [sys.params[req.diagram.param]?.name ?? "r", "n", sys.states[req.diagram.state]?.name ?? "x"];
  const stem = `${mockSlug(req.title)}-orbit-diagram`;
  const rows = mockOrbitRows(req, res).map((row) => row.map(String).join(","));
  const meta = { format: "separatrix-orbit-diagram", format_version: 1, title: req.title, csv: `${stem}.csv`, columns: header, note: "Browser preview." };
  return { stem, csv: [header.join(","), ...rows].join("\n") + "\n", meta: JSON.stringify(meta, null, 2) + "\n" };
}

/** `export_orbit_latex` (inline) or, with a `base`, the saved figure: the 5,000-point rule as Rust applies it. */
async function mockOrbitFigure(
  req: OrbitFigureRequest,
  base: string | null,
): Promise<FigureLatexResponse & { files: string[] }> {
  const res = await mockOrbitDiagram(req.orbit.diagram);
  const [x0, x1] = req.xRange;
  const [y0, y1] = req.yRange;
  let pts = mockOrbitRows(req.orbit, res).filter(([r, , x]) => r >= x0 && r <= x1 && x >= y0 && x <= y1);
  const notes: string[] = [];
  const files: string[] = [];
  let thinned = false;
  let graphics = "";
  const total = pts.length;
  if (total > 5000) {
    if (base !== null) {
      files.push(`${base}-points.png`);
      graphics = `\\addplot graphics[xmin=${x0}, xmax=${x1}, ymin=${y0}, ymax=${y1}] {\\sepxdir ${base}-points.png};`;
      notes.push(`The iterates (${total} points) are drawn as a density PNG under vector axes.`);
      pts = [];
    } else {
      const stride = Math.max(2, Math.ceil(total / 5000));
      pts = pts.filter((_, i) => i % stride === 0);
      thinned = true;
      notes.push(`One iterate in ${stride} is kept so the copied figure stays under 5,000 points; Save… draws them all as an image.`);
    }
  }
  const tex = [
    `% Separatrix orbit diagram: ${req.orbit.title}`,
    PREVIEW_NOTE,
    "% Needs: \\usepackage{amsmath} \\usepackage{pgfplots} \\pgfplotsset{compat=1.18} \\usetikzlibrary{calc}",
    ...notes.map((n) => `% ${n}`),
    "\\begin{tikzpicture}",
    `\\begin{axis}[xmin=${x0}, xmax=${x1}, ymin=${y0}, ymax=${y1}]`,
    ...(graphics ? [graphics] : []),
    ...(pts.length ? [`\\addplot[only marks] coordinates {${pts.map(([r, , x]) => `(${r},${x})`).join(" ")}};`] : []),
    "\\end{axis}",
    "\\end{tikzpicture}",
    "",
  ].join("\n");
  return { tex, points: pts.length, thinned, notes, files };
}

export async function mockExportOrbitLatex(req: OrbitFigureRequest): Promise<FigureLatexResponse> {
  const { tex, points, thinned, notes } = await mockOrbitFigure(req, null);
  return { tex, points, thinned, notes };
}

export async function mockSaveOrbitLatex(req: SaveOrbitFigureRequest): Promise<SavedFiles | null> {
  const chosen = takeDialog("nextSavePath", withExt(fileName(req.suggestedName) || "figure", "tex").path);
  if (chosen === null) return null;
  const name = withExt(fileName(chosen), "tex").path;
  const fig = await mockOrbitFigure(req.figure, mockSlug(name.replace(/\.tex$/i, "")));
  mockSaved(name, fig.tex);
  for (const f of fig.files) mockSaved(f, new Uint8Array([137, 80, 78, 71]));
  return { names: [name, ...fig.files] };
}

export async function mockSaveCsvFile(req: SaveCsvFileRequest): Promise<SavedFiles | null> {
  const chosen = takeDialog("nextSavePath", withExt(fileName(req.suggestedName) || "data", "csv").path);
  if (chosen === null) return null;
  const stem = withExt(fileName(chosen), "csv").path.replace(/\.csv$/i, "");
  mockSaved(`${stem}.csv`, req.file.csv);
  mockSaved(`${stem}.meta.json`, req.file.meta);
  return { names: [`${stem}.csv`, `${stem}.meta.json`] };
}

function mockSaved(name: string, contents: string | Uint8Array): void {
  sepxMock().saved.push({ name, contents });
}

export async function mockSaveFigureLatex(req: SaveFigureRequest): Promise<SavedFiles | null> {
  const chosen = takeDialog("nextSavePath", withExt(fileName(req.suggestedName) || "figure", "tex").path);
  if (chosen === null) return null;
  const name = withExt(fileName(chosen), "tex").path;
  const fig = await mockExportFigureLatex(req.figure);
  mockSaved(name, fig.tex);
  return { names: [name] };
}

export async function mockSaveCsv(req: SaveCsvRequest): Promise<SavedFiles | null> {
  const chosen = takeDialog("nextSavePath", withExt(fileName(req.suggestedName) || "trajectory", "csv").path);
  if (chosen === null) return null;
  const stem = withExt(fileName(chosen), "csv").path.replace(/\.csv$/i, "");
  const { files } = await mockExportCsv(req.csv);
  const names: string[] = [];
  const prefix = `${req.csv.title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || "system"}-`;
  files.forEach((f) => {
    const s = files.length === 1 ? stem : `${stem}-${f.stem.startsWith(prefix) ? f.stem.slice(prefix.length) : f.stem}`;
    mockSaved(`${s}.csv`, f.csv);
    mockSaved(`${s}.meta.json`, f.meta);
    names.push(`${s}.csv`, `${s}.meta.json`);
  });
  return { names };
}

export async function mockSaveTextFile(req: SaveTextRequest): Promise<SavedFiles | null> {
  const chosen = takeDialog("nextSavePath", withExt(fileName(req.suggestedName) || "untitled", req.extension).path);
  if (chosen === null) return null;
  const name = withExt(fileName(chosen), req.extension).path;
  mockSaved(name, req.contents);
  return { names: [name] };
}

export async function mockSavePngFile(png: Uint8Array, suggestedName: string): Promise<SavedFiles | null> {
  const chosen = takeDialog("nextSavePath", withExt(fileName(suggestedName) || "figure", "png").path);
  if (chosen === null) return null;
  const name = withExt(fileName(chosen), "png").path;
  mockSaved(name, png);
  return { names: [name] };
}

export async function mockClipboardWriteText(req: ClipboardTextRequest): Promise<void> {
  sepxMock().clipboardText = req.text;
  try {
    await navigator.clipboard?.writeText(req.text);
  } catch {
    // The preview keeps the text in `__sepxMock` when the page may not copy.
  }
}

export async function mockClipboardWritePng(png: Uint8Array): Promise<void> {
  sepxMock().clipboardPngBytes = png.byteLength;
  try {
    const ClipboardItemCtor = (globalThis as { ClipboardItem?: typeof ClipboardItem }).ClipboardItem;
    if (ClipboardItemCtor && navigator.clipboard?.write) {
      const blob = new Blob([png.slice().buffer], { type: "image/png" });
      await navigator.clipboard.write([new ClipboardItemCtor({ "image/png": blob })]);
    }
  } catch {
    // As above: the size is recorded in `__sepxMock`.
  }
}
