import type {
  AxisRef,
  CompileResponse,
  FixedPointOut,
  Method,
  PortraitResponse,
  StateInfo,
  SystemKind,
  TrajectoryRequest,
} from "../api/types";
import type { PortraitJob } from "../api/client";
import type { Template } from "../templates";

export interface EquationLine {
  id: number;
  latex: string;
}

export interface InitialCondition {
  id: number;
  x0: number[];
  t0: number;
  color: number;
  hidden?: boolean;
}

export interface ParamState {
  name: string;
  latex: string;
  value: number;
  min: number;
  max: number;
}

export interface View {
  xAxis: AxisRef;
  yAxis: AxisRef;
  xRange: [number, number];
  yRange: [number, number];
}

export interface Layers {
  field: boolean;
  fieldGrid: number;
  nullclines: boolean;
  fixedPoints: boolean;
  backward: boolean;
}

export interface SolverSettings {
  method: Method;
  rtol: number;
  atol: number;
  h: number;
  maxSteps: number;
  tEnd: number;
  samples: number;
}

/**
 * What the user selected. A trajectory by its id; a fixed point by where it
 * is (`x`, its full state vector), because the list of fixed points is
 * recomputed and re-sorted on every change, so a list index would jump to
 * another point. `key` tells one selection apart from the next (it stays
 * the same while the point is tracked across recomputes).
 */
export type Selection = { kind: "ic"; id: number } | { kind: "fp"; x: number[]; key: number } | null;

let nextFixedPointKey = 1;

/** A new selection of the fixed point at `x`. */
export function selectFixedPoint(x: readonly number[]): Selection {
  return { kind: "fp", x: x.slice(), key: nextFixedPointKey++ };
}

/**
 * After a recompute, the selected fixed point is the nearest one within this
 * fraction of the view's larger state-axis span (Euclidean distance over the
 * full state vector); farther than that, it is taken to have disappeared.
 * Generous, because one slider move can carry a point a long way (near a
 * saddle-node a point moves like the square root of the parameter); the
 * mutual-nearest test in [`trackSelection`] keeps it from jumping to a
 * neighbour.
 */
export const FIXED_POINT_MATCH_FRACTION = 0.1;

/** Distance within which a recomputed fixed point is the selected one, in state units. */
export function fixedPointMatchTolerance(view: View): number {
  const spans = (axes: [AxisRef, [number, number]][]) =>
    axes.map(([, [lo, hi]]) => Math.abs(hi - lo)).filter((v) => Number.isFinite(v) && v > 0);
  const all: [AxisRef, [number, number]][] = [
    [view.xAxis, view.xRange],
    [view.yAxis, view.yRange],
  ];
  const onState = spans(all.filter(([a]) => a.kind === "state"));
  const pick = onState.length ? onState : spans(all);
  return FIXED_POINT_MATCH_FRACTION * (pick.length ? Math.max(...pick) : 1);
}

/** Index of the fixed point nearest `x` within `tol`, or -1. */
export function nearestFixedPoint(
  fps: readonly Pick<FixedPointOut, "x">[],
  x: readonly number[],
  tol: number,
): number {
  let best = -1;
  let bestD = Infinity;
  fps.forEach((fp, i) => {
    if (fp.x.length !== x.length) return;
    const d = Math.hypot(...fp.x.map((v, k) => v - x[k]));
    if (d <= tol && d < bestD) {
      best = i;
      bestD = d;
    }
  });
  return best;
}

/** Index in `fps` of the selected fixed point, or null. */
export function selectedFixedPointIndex(
  sel: Selection,
  fps: readonly Pick<FixedPointOut, "x">[],
  view: View,
): number | null {
  if (sel?.kind !== "fp") return null;
  const i = nearestFixedPoint(fps, sel.x, fixedPointMatchTolerance(view));
  return i >= 0 ? i : null;
}

/**
 * The selection after fixed points were recomputed for `view`: a selected
 * fixed point follows the nearest new one (so it keeps up as a parameter
 * moves it) and is cleared when none is near; other selections are kept.
 * Given the points it was selected among (`prev`), the new point must also
 * be nearer to the selected one than to any other old point: when the
 * selected point vanishes into a neighbour (a pitchfork or saddle-node), the
 * neighbour that is left is not taken for it.
 */
export function trackSelection(
  sel: Selection,
  fps: readonly Pick<FixedPointOut, "x">[],
  view: View,
  prev: readonly Pick<FixedPointOut, "x">[] = [],
): Selection {
  if (sel?.kind !== "fp") return sel;
  const i = selectedFixedPointIndex(sel, fps, view);
  if (i === null) return null;
  const own = nearestFixedPoint(prev, sel.x, 0);
  if (own >= 0) {
    const dist = (a: readonly number[], b: readonly number[]) => Math.hypot(...a.map((v, k) => v - b[k]));
    const d = dist(fps[i].x, sel.x);
    if (prev.some((q, k) => k !== own && q.x.length === sel.x.length && dist(fps[i].x, q.x) < d)) return null;
  }
  const x = fps[i].x;
  return x.every((v, k) => v === sel.x[k]) ? sel : { ...sel, x: x.slice() };
}

export interface TrajectoryMeta {
  icId: number;
  direction: "forward" | "backward";
}

export interface PortraitMeta {
  systemId: number;
  trajectories: TrajectoryMeta[];
  view: View;
  /** Fixed points were asked for, so an empty list means there are none (not that the layer is hidden). */
  fixedPoints: boolean;
}

export interface PortraitState {
  res: PortraitResponse;
  meta: PortraitMeta;
  roundTripMs: number;
}

export const PALETTE_SIZE = 7;

export function leastUsedPaletteIndex(ics: readonly Pick<InitialCondition, "color">[]): number {
  const count = new Array<number>(PALETTE_SIZE).fill(0);
  for (const ic of ics) {
    const c = ((ic.color % PALETTE_SIZE) + PALETTE_SIZE) % PALETTE_SIZE;
    if (Number.isInteger(c)) count[c]++;
  }
  let best = 0;
  for (let c = 1; c < PALETTE_SIZE; c++) if (count[c] < count[best]) best = c;
  return best;
}

export const UNTITLED_SYSTEM = "Untitled system";

const dropDelimiterSizing = (latex: string) => latex.replace(/\\(?:left|right)(?![A-Za-z])/g, "");

const unbraceSingleScript = (latex: string) => latex.replace(/([_^])\s*\{\s*([A-Za-z0-9])\s*\}/g, "$1$2");

const collapseSpaces = (latex: string) =>
  latex
    .trim()
    .replace(/\s+/g, " ")
    .replace(/ ?([^A-Za-z ]) ?/g, "$1");

function canonicalLine(latex: string): string {
  return collapseSpaces(unbraceSingleScript(dropDelimiterSizing(latex)));
}

export function sameEquations(a: readonly string[], b: readonly string[]): boolean {
  const norm = (lines: readonly string[]) => lines.map(canonicalLine).filter((l) => l !== "");
  const x = norm(a);
  const y = norm(b);
  return x.length === y.length && x.every((l, i) => l === y[i]);
}

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

export const DEFAULT_FIXED_STEP = 0.01;

export const TIME_AXIS: AxisRef = { kind: "time" };
export const stateAxis = (index: number): AxisRef => ({ kind: "state", index });

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

export function resolveAxis(name: string, states: StateInfo[]): AxisRef | null {
  if (name === "t" || name === "n") return TIME_AXIS;
  const i = states.findIndex((s) => s.name === name || s.latex === name);
  return i >= 0 ? stateAxis(i) : null;
}

export function templateView(t: Template, sys: CompileResponse): View {
  const xAxis = resolveAxis(t.view.x, sys.states);
  const yAxis = resolveAxis(t.view.y, sys.states);
  if (!xAxis || !yAxis) return defaultView(sys);
  return { xAxis, yAxis, xRange: [...t.view.xRange], yRange: [...t.view.yRange] };
}

export function axisValid(a: AxisRef, dim: number): boolean {
  return a.kind === "time" || a.index < dim;
}

export function defaultParamRange(v: number): [number, number] {
  if (v === 0) return [-1, 1];
  const span = niceCeil(Math.abs(v) * 2);
  return v > 0 ? [0, span] : [-span, 0];
}

const NICE_MANTISSAS = [1, 2, 2.5, 5, 10];

export function niceCeil(v: number): number {
  if (v <= 0) return 1;
  const p = Math.pow(10, Math.floor(Math.log10(v)));
  for (const m of NICE_MANTISSAS) if (m * p >= v - 1e-12) return m * p;
  return 10 * p;
}

export function adaptInitialConditions(
  ics: InitialCondition[],
  oldStates: StateInfo[],
  newStates: StateInfo[],
): InitialCondition[] {
  return ics.map((ic) => ({
    ...ic,
    x0: newStates.map((s) => {
      const k = oldStates.findIndex((o) => o.name === s.name);
      return k >= 0 ? (ic.x0[k] ?? 0) : 0;
    }),
  }));
}

export function visibleTimeRange(view: View): [number, number] | null {
  if (view.xAxis.kind === "time") return view.xRange;
  if (view.yAxis.kind === "time") return view.yRange;
  return null;
}

type TrajectorySpan = { req: TrajectoryRequest; direction: TrajectoryMeta["direction"] };

function mapSpan(ic: InitialCondition, solver: SolverSettings): TrajectorySpan {
  return { req: { x0: ic.x0, t0: 0, t1: Math.max(1, Math.round(solver.tEnd)) }, direction: "forward" };
}

/** Spans that cover the visible time window on either side of the start time. */
function visibleWindowSpans(
  [lo, hi]: [number, number],
  ic: InitialCondition,
  backward: boolean,
): TrajectorySpan[] {
  const out: TrajectorySpan[] = [];
  if (hi > ic.t0) out.push({ req: { x0: ic.x0, t0: ic.t0, t1: hi }, direction: "forward" });
  if (backward && lo < ic.t0) out.push({ req: { x0: ic.x0, t0: ic.t0, t1: lo }, direction: "backward" });
  return out;
}

/** Spans of tEnd forward (and backward) from the start time. */
function phasePlaneSpans(ic: InitialCondition, tEnd: number, backward: boolean): TrajectorySpan[] {
  const out: TrajectorySpan[] = [{ req: { x0: ic.x0, t0: ic.t0, t1: ic.t0 + tEnd }, direction: "forward" }];
  if (backward) out.push({ req: { x0: ic.x0, t0: ic.t0, t1: ic.t0 - tEnd }, direction: "backward" });
  return out;
}

export function trajectorySpans(
  kind: CompileResponse["kind"],
  view: View,
  ic: InitialCondition,
  solver: SolverSettings,
  layers: Layers,
): TrajectorySpan[] {
  if (kind === "map") return [mapSpan(ic, solver)];
  const window = visibleTimeRange(view);
  return window
    ? visibleWindowSpans(window, ic, layers.backward)
    : phasePlaneSpans(ic, solver.tEnd, layers.backward);
}

export function autoSamples(maxSpan: number): number {
  return Math.round(Math.min(8000, Math.max(400, maxSpan * 120)));
}

export interface StudioSnapshot {
  compiled: CompileResponse | null;
  params: ParamState[];
  view: View;
  baseState: number[];
  fieldTime: number;
  ics: InitialCondition[];
  layers: Layers;
  solver: SolverSettings;
  plotAspect: number | null;
}

export function equalizeRanges(
  xRange: [number, number],
  yRange: [number, number],
  aspect: number | null | undefined,
): { xRange: [number, number]; yRange: [number, number] } {
  const sx = xRange[1] - xRange[0];
  const sy = yRange[1] - yRange[0];
  if (!aspect || !(aspect > 0) || !Number.isFinite(aspect) || !(sx > 0) || !(sy > 0)) return { xRange, yRange };
  const xSpanMatchingY = sy * aspect;
  if (Math.abs(xSpanMatchingY - sx) <= 1e-9 * Math.max(sx, xSpanMatchingY)) return { xRange, yRange };
  if (xSpanMatchingY > sx) {
    const c = (xRange[0] + xRange[1]) / 2;
    return { xRange: [c - xSpanMatchingY / 2, c + xSpanMatchingY / 2], yRange };
  }
  const c = (yRange[0] + yRange[1]) / 2;
  const span = sx / aspect;
  return { xRange, yRange: [c - span / 2, c + span / 2] };
}

export function isFlowPhasePlane(xAxis: AxisRef, yAxis: AxisRef, kind: SystemKind | undefined): boolean {
  return kind !== "map" && xAxis.kind === "state" && yAxis.kind === "state";
}

export function displayRanges(
  view: View,
  kind: CompileResponse["kind"] | undefined,
  aspect: number | null,
): { xRange: [number, number]; yRange: [number, number] } {
  return isFlowPhasePlane(view.xAxis, view.yAxis, kind)
    ? equalizeRanges(view.xRange, view.yRange, aspect)
    : { xRange: view.xRange, yRange: view.yRange };
}

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
    meta: { systemId: sys.systemId, trajectories: metas, view: s.view, fixedPoints: s.layers.fixedPoints },
  };
}

export function sampleOnAxis(a: AxisRef, t: number[], x: number[], dim: number, k: number): number {
  return a.kind === "time" ? t[k] : x[k * dim + a.index];
}

/** States beyond this size are left out of extents, so one runaway run cannot squash the view. */
const OUTLIER_CAP = 1e6;

const completedRuns = (res: PortraitResponse) => res.trajectories.filter((tr) => tr.status === "completed");

export function stateExtent(
  res: PortraitResponse | undefined,
  dim: number,
  index: number,
): [number, number] | null {
  if (!res) return null;
  let lo = Infinity;
  let hi = -Infinity;
  for (const tr of completedRuns(res)) {
    for (let k = 0; k < tr.t.length; k++) {
      const v = tr.x[k * dim + index];
      if (Number.isFinite(v) && Math.abs(v) < OUTLIER_CAP) {
        if (v < lo) lo = v;
        if (v > hi) hi = v;
      }
    }
  }
  return lo <= hi ? [lo, hi] : null;
}
