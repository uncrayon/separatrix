// The UI's data model and the pure functions that turn it into requests.
//
// Nothing here touches React; useStudio.ts holds the state and calls these.

import type {
  AxisRef,
  CompileResponse,
  Method,
  PortraitResponse,
  SymbolInfo,
  TrajectoryRequest,
} from "../api/types";
import type { PortraitJob } from "../api/client";
import type { Template } from "../templates";

/** One editable equation line in the left panel. */
export interface EquationLine {
  id: number;
  latex: string;
}

/** A user-placed starting point. `x0` is always the full state vector. */
export interface InitialCondition {
  id: number;
  x0: number[];
  /** Start time (ODE). Maps always start at n = 0. */
  t0: number;
  /** Index into the trajectory palette, fixed when the point is created. */
  color: number;
  /** Hidden from the plot: not integrated, its start point is drawn hollow. */
  hidden?: boolean;
}

/** A parameter with its current value and slider range. */
export interface ParamState {
  name: string;
  latex: string;
  value: number;
  min: number;
  max: number;
}

/** What is on the canvas. */
export interface View {
  xAxis: AxisRef;
  yAxis: AxisRef;
  xRange: [number, number];
  yRange: [number, number];
}

/** Toggleable layers and their densities ("smart defaults" shown as chips). */
export interface Layers {
  field: boolean;
  /** Arrows per side of the direction-field grid. */
  fieldGrid: number;
  nullclines: boolean;
  fixedPoints: boolean;
  /** Also integrate each initial condition backwards in time. */
  backward: boolean;
}

/** Solver settings shown in the inspector. */
export interface SolverSettings {
  method: Method;
  rtol: number;
  atol: number;
  /** Fixed step (euler/heun/rk4) or initial step for dopri5; 0 = automatic. */
  h: number;
  maxSteps: number;
  /** Time span T for ODEs, number of iterates N for maps. */
  tEnd: number;
  /** Dense samples per trajectory; 0 = automatic. */
  samples: number;
}

/** What the inspector shows. */
export type Selection = { kind: "ic"; id: number } | { kind: "fp"; index: number } | null;

/** Which initial condition (and which direction) a returned trajectory belongs to. */
export interface TrajectoryMeta {
  icId: number;
  direction: "forward" | "backward";
}

/** Extra data returned with each portrait response. */
export interface PortraitMeta {
  systemId: number;
  trajectories: TrajectoryMeta[];
  view: View;
}

/** The latest portrait the canvas can draw. */
export interface PortraitState {
  res: PortraitResponse;
  meta: PortraitMeta;
  roundTripMs: number;
}

/** Number of colours in the trajectory palette (tokens --traj-1 … --traj-7). */
export const PALETTE_SIZE = 7;

/**
 * The palette index for a new trajectory, given the ones already in the list
 * (hidden ones included: their start point is still drawn in their colour).
 *
 * The first colour no trajectory uses, so an empty list starts again at
 * traj-1 and deleting one trajectory frees its colour for the next. When all
 * seven are taken, the least-used colour, the first one on a tie: the cycle
 * carries on as traj-1, traj-2, … for the eighth and later trajectories.
 */
export function nextIcColor(ics: readonly Pick<InitialCondition, "color">[]): number {
  const count = new Array<number>(PALETTE_SIZE).fill(0);
  for (const ic of ics) {
    const c = ((ic.color % PALETTE_SIZE) + PALETTE_SIZE) % PALETTE_SIZE;
    if (Number.isInteger(c)) count[c]++;
  }
  let best = 0;
  for (let c = 1; c < PALETTE_SIZE; c++) if (count[c] < count[best]) best = c;
  return best;
}

/** The name shown for a system that does not come from a template. */
export const UNTITLED_SYSTEM = "Untitled system";

/**
 * One equation line in a canonical form, for comparing systems as typed. Only
 * spellings that mean the same thing are merged, so the text-mode round trip
 * of a template (`x(3-x)`, `x^{2}`) still matches it (`x\left(3-x\right)`, `x^2`):
 *
 * - `\left` and `\right` are dropped (they only size the delimiter);
 * - a one-character script loses its braces (`x^{2}` is `x^2`);
 * - blank space is dropped next to anything that is not a letter (`x = y` is
 *   `x=y`), and runs of it become one space between letters, where it still
 *   ends a control word (`\alpha x` is not `\alphax`).
 */
function canonicalLine(latex: string): string {
  return latex
    .replace(/\\(?:left|right)(?![A-Za-z])/g, "")
    .replace(/([_^])\s*\{\s*([A-Za-z0-9])\s*\}/g, "$1$2")
    .trim()
    .replace(/\s+/g, " ")
    .replace(/ ?([^A-Za-z ]) ?/g, "$1");
}

/** Do two lists of equation lines describe the same system text? Blank lines are ignored, as the compiler ignores them. */
export function sameEquations(a: readonly string[], b: readonly string[]): boolean {
  const norm = (lines: readonly string[]) => lines.map(canonicalLine).filter((l) => l !== "");
  const x = norm(a);
  const y = norm(b);
  return x.length === y.length && x.every((l, i) => l === y[i]);
}

/**
 * The name of the open system for the top bar. A template keeps its title
 * when its equations are edited and is marked `edited` until the text is the
 * template's again; anything else (a blank system) is "Untitled system".
 */
export function systemTitle(
  template: Template | undefined,
  equations: readonly string[],
): { name: string; edited: boolean } {
  if (!template) return { name: UNTITLED_SYSTEM, edited: false };
  return { name: template.title, edited: !sameEquations(template.equations, equations) };
}

export const DEFAULT_LAYERS: Layers = {
  field: true,
  fieldGrid: 21,
  nullclines: true,
  fixedPoints: true,
  backward: true,
};

export const DEFAULT_SOLVER: SolverSettings = {
  method: "dopri5",
  rtol: 1e-6,
  atol: 1e-9,
  h: 0,
  maxSteps: 200_000,
  tEnd: 20,
  samples: 0,
};

/** Default fixed step when switching to a fixed-step method. */
export const DEFAULT_FIXED_STEP = 0.01;

export const TIME_AXIS: AxisRef = { kind: "time" };
export const stateAxis = (index: number): AxisRef => ({ kind: "state", index });

/** A reasonable first view for a freshly compiled system. */
export function defaultView(sys: CompileResponse): View {
  const dim = sys.states.length;
  if (dim <= 1) {
    const map = sys.kind === "map";
    return {
      xAxis: TIME_AXIS,
      yAxis: stateAxis(0),
      xRange: map ? [0, 50] : [0, 10],
      yRange: map ? [-0.1, 1.1] : [-3, 3],
    };
  }
  return { xAxis: stateAxis(0), yAxis: stateAxis(1), xRange: [-5, 5], yRange: [-5, 5] };
}

/** Resolve a template axis name ("t", "n" or a state name) to an axis. */
export function resolveAxis(name: string, states: SymbolInfo[]): AxisRef | null {
  if (name === "t" || name === "n") return TIME_AXIS;
  const i = states.findIndex((s) => s.name === name || s.latex === name);
  return i >= 0 ? stateAxis(i) : null;
}

/** The view a template asks for, or the default when its axes don't resolve. */
export function templateView(t: Template, sys: CompileResponse): View {
  const xAxis = resolveAxis(t.view.x, sys.states);
  const yAxis = resolveAxis(t.view.y, sys.states);
  if (!xAxis || !yAxis) return defaultView(sys);
  return { xAxis, yAxis, xRange: [...t.view.xRange], yRange: [...t.view.yRange] };
}

/** Is this axis still meaningful for a system with `dim` states? */
export function axisValid(a: AxisRef, dim: number): boolean {
  return a.kind === "time" || a.index < dim;
}

/** A slider range that comfortably contains `v`. */
export function defaultParamRange(v: number): [number, number] {
  if (v === 0) return [-1, 1];
  const span = niceCeil(Math.abs(v) * 2);
  return v > 0 ? [0, span] : [-span, 0];
}

/** Smallest 1/2/5 × 10^k that is ≥ v. */
export function niceCeil(v: number): number {
  if (v <= 0) return 1;
  const p = Math.pow(10, Math.floor(Math.log10(v)));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * p >= v - 1e-12) return m * p;
  return 10 * p;
}

/**
 * Carry initial conditions over to a recompiled system: values are matched by
 * state name, so renaming nothing keeps every point where it was.
 */
export function adaptInitialConditions(
  ics: InitialCondition[],
  oldStates: SymbolInfo[],
  newStates: SymbolInfo[],
): InitialCondition[] {
  return ics.map((ic) => ({
    ...ic,
    x0: newStates.map((s) => {
      const k = oldStates.findIndex((o) => o.name === s.name);
      return k >= 0 ? (ic.x0[k] ?? 0) : 0;
    }),
  }));
}

/** The time window covered by an axis on screen, if time is on an axis. */
export function visibleTimeRange(view: View): [number, number] | null {
  if (view.xAxis.kind === "time") return view.xRange;
  if (view.yAxis.kind === "time") return view.yRange;
  return null;
}

/**
 * The time spans to integrate for one initial condition.
 *
 * - Maps: iterate `tEnd` steps forward.
 * - ODE with time on an axis: forward up to the right/top edge of the visible
 *   time window, and (only when `layers.backward` is on) backward down to the
 *   left/bottom edge. The time span T is not used: the view sets the span.
 * - ODE phase plane: forward for T, and backward for T when `layers.backward`.
 */
export function trajectorySpans(
  kind: CompileResponse["kind"],
  view: View,
  ic: InitialCondition,
  solver: SolverSettings,
  layers: Layers,
): { req: TrajectoryRequest; direction: TrajectoryMeta["direction"] }[] {
  if (kind === "map") {
    return [{ req: { x0: ic.x0, t0: 0, t1: Math.max(1, Math.round(solver.tEnd)) }, direction: "forward" }];
  }
  const out: { req: TrajectoryRequest; direction: TrajectoryMeta["direction"] }[] = [];
  const window = visibleTimeRange(view);
  if (window) {
    // Time is on screen: cover exactly what is visible. Going backwards is
    // still the user's choice: many systems (Lorenz, any dissipative flow)
    // blow up in backward time and would only produce a solver failure.
    const [lo, hi] = window;
    if (hi > ic.t0) out.push({ req: { x0: ic.x0, t0: ic.t0, t1: hi }, direction: "forward" });
    if (layers.backward && lo < ic.t0)
      out.push({ req: { x0: ic.x0, t0: ic.t0, t1: lo }, direction: "backward" });
    return out;
  }
  out.push({ req: { x0: ic.x0, t0: ic.t0, t1: ic.t0 + solver.tEnd }, direction: "forward" });
  if (layers.backward)
    out.push({ req: { x0: ic.x0, t0: ic.t0, t1: ic.t0 - solver.tEnd }, direction: "backward" });
  return out;
}

/** Automatic sample count: dense enough for smooth curves on long spans. */
export function autoSamples(maxSpan: number): number {
  return Math.round(Math.min(8000, Math.max(400, maxSpan * 120)));
}

/** Everything needed to build a portrait request. */
export interface StudioSnapshot {
  compiled: CompileResponse | null;
  params: ParamState[];
  view: View;
  baseState: number[];
  fieldTime: number;
  ics: InitialCondition[];
  layers: Layers;
  solver: SolverSettings;
  /** Width / height of the plot on screen (null before it is laid out). */
  plotAspect: number | null;
}

/**
 * Phase planes use equal scales on both axes, so angles in the direction
 * field and the shapes of orbits are true (PhasePlane README): the shorter of
 * the two ranges is widened about its centre to match the plot's aspect
 * (`aspect` = plot width / height). Nothing is ever cut off.
 */
export function equalizeRanges(
  xRange: [number, number],
  yRange: [number, number],
  aspect: number | null | undefined,
): { xRange: [number, number]; yRange: [number, number] } {
  const sx = xRange[1] - xRange[0];
  const sy = yRange[1] - yRange[0];
  if (!aspect || !(aspect > 0) || !Number.isFinite(aspect) || !(sx > 0) || !(sy > 0)) return { xRange, yRange };
  const want = sy * aspect; // the x span that matches y's scale
  if (Math.abs(want - sx) <= 1e-9 * Math.max(sx, want)) return { xRange, yRange };
  if (want > sx) {
    const c = (xRange[0] + xRange[1]) / 2;
    return { xRange: [c - want / 2, c + want / 2], yRange };
  }
  const c = (yRange[0] + yRange[1]) / 2;
  const span = sx / aspect;
  return { xRange, yRange: [c - span / 2, c + span / 2] };
}

/**
 * Should a view use equal scales? Only a flow's phase plane: both axes are
 * states and there is a direction field whose angles should be true. Time
 * series stay stretched, and so do maps (no field; the Hénon attractor would
 * shrink to a sliver).
 */
export function wantsEqualScales(
  xAxis: AxisRef,
  yAxis: AxisRef,
  kind: string | undefined,
): boolean {
  return kind !== "map" && xAxis.kind === "state" && yAxis.kind === "state";
}

/** The ranges actually on screen: the view, widened to equal scales on a phase plane. */
export function displayRanges(
  view: View,
  kind: CompileResponse["kind"] | undefined,
  aspect: number | null,
): { xRange: [number, number]; yRange: [number, number] } {
  return wantsEqualScales(view.xAxis, view.yAxis, kind)
    ? equalizeRanges(view.xRange, view.yRange, aspect)
    : { xRange: view.xRange, yRange: view.yRange };
}

/** Turn the current UI state into a `compute_portrait` request. */
export function buildPortraitJob(s: StudioSnapshot): PortraitJob<PortraitMeta> | null {
  const sys = s.compiled;
  if (!sys || !sys.ok || sys.systemId === undefined) return null;
  const dim = sys.states.length;
  const isOde = sys.kind !== "map";
  const trajectories: TrajectoryRequest[] = [];
  const metas: TrajectoryMeta[] = [];
  for (const ic of s.ics) {
    if (ic.x0.length !== dim) continue;
    if (ic.hidden) continue;
    for (const span of trajectorySpans(sys.kind, s.view, ic, s.solver, s.layers)) {
      trajectories.push(span.req);
      metas.push({ icId: ic.id, direction: span.direction });
    }
  }
  const maxSpan = trajectories.reduce((m, r) => Math.max(m, Math.abs(r.t1 - r.t0)), 0);
  const fixedStep = s.solver.method !== "dopri5";
  // The field, nullclines and fixed points cover what is on screen.
  const shown = displayRanges(s.view, sys.kind, s.plotAspect);
  return {
    body: {
      systemId: sys.systemId,
      params: s.params.map((p) => p.value),
      xAxis: s.view.xAxis,
      yAxis: s.view.yAxis,
      xRange: shown.xRange,
      yRange: shown.yRange,
      baseState: Array.from({ length: dim }, (_, i) => s.baseState[i] ?? 0),
      fieldTime: s.fieldTime,
      fieldGrid: isOde && s.layers.field ? s.layers.fieldGrid : 0,
      nullclines: isOde && s.layers.nullclines,
      fixedPoints: s.layers.fixedPoints,
      trajectories,
      solver: {
        method: s.solver.method,
        rtol: s.solver.rtol,
        atol: s.solver.atol,
        h: fixedStep && s.solver.h <= 0 ? DEFAULT_FIXED_STEP : s.solver.h,
        maxSteps: s.solver.maxSteps,
      },
      samples: s.solver.samples > 0 ? s.solver.samples : autoSamples(maxSpan),
    },
    meta: { systemId: sys.systemId, trajectories: metas, view: s.view },
  };
}

/** Value of axis `a` at sample `k` of a trajectory. */
export function sampleOnAxis(a: AxisRef, t: number[], x: number[], dim: number, k: number): number {
  return a.kind === "time" ? t[k] : x[k * dim + a.index];
}

/**
 * Min and max of one state over the trajectories that completed (used for
 * slice-slider ranges and for the range of a newly chosen axis).
 *
 * Runs the solver stopped early (`maxSteps`, `blewUp`, …) are skipped: they
 * typically end far outside the region of interest (a Lorenz orbit run
 * backwards reaches |x| ~ 1e5 before the step limit) and would stretch the
 * range until the interesting part is a dot. Returns null when no completed
 * run has a finite sample of that state.
 */
export function stateExtent(
  res: PortraitResponse | undefined,
  dim: number,
  index: number,
): [number, number] | null {
  if (!res) return null;
  let lo = Infinity;
  let hi = -Infinity;
  for (const tr of res.trajectories) {
    if (tr.status !== "completed") continue;
    for (let k = 0; k < tr.t.length; k++) {
      const v = tr.x[k * dim + index];
      if (Number.isFinite(v) && Math.abs(v) < 1e6) {
        if (v < lo) lo = v;
        if (v > hi) hi = v;
      }
    }
  }
  return lo <= hi ? [lo, hi] : null;
}
