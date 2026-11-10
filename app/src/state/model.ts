import type {
  AxisRef,
  CompileResponse,
  ContinuationDirection,
  ContinuationRequest,
  ContinuationResponse,
  ContinuationStart,
  ConvertResponse,
  Direction,
  FixedPointOut,
  InputFormat,
  LyapunovRequest,
  LyapunovResponse,
  Method,
  OrbitDiagramRequest,
  OrbitDiagramResponse,
  ParamInfo,
  PortraitRequest,
  PortraitResponse,
  SectionSpec,
  SolverOptions,
  SpecialKindOut,
  SpecialPointOut,
  SwitchPoint,
  StateInfo,
  SystemKind,
  TrajectoryRequest,
} from "../api/types";
import {
  DEFAULT_CAMERA,
  LYAPUNOV_MISFIT,
  MAX_CONTINUATION_POINTS,
  MAX_ORBIT_COLUMNS,
  MAX_LYAPUNOV_ITERATES,
  MAX_RENORMALIZATIONS,
  ORBIT_MAX_KEPT,
  ORBIT_MAX_TRANSIENT,
  ORBIT_MISFIT,
  SAME_BIFURCATION_TOL,
  SECTION_MISFIT,
  SPACE_MISFIT,
} from "../api/ipc.generated.ts";
import { DEFAULT_METHOD, METHODS } from "./methods.ts";
import type { Job, PortraitJob } from "../api/client";
import type { Template } from "../templates";
import type { LoadedAxis, LoadedSeed, LoadedView, ProjectLoad } from "../project/serialize";
import { curvesAvailability, openPlane, planeSeed, withBranch, type CurvesView, type PlaneSeed } from "./curves.ts";

/** One equation line, exactly as the user typed it, in the system's input format. */
export interface EquationLine {
  id: number;
  source: string;
}

export interface InitialCondition {
  id: number;
  /**
   * The id it has in a project file (`initial_conditions[].id`), stable across
   * saves. Optional so start points built outside the studio (thumbnails,
   * tests) need none; such a point is saved as `ic-<id>`.
   */
  uid?: string;
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

export interface Camera {
  yaw: number;
  pitch: number;
  zoom: number;
}

export const PITCH_LIMIT = Math.PI / 2 - 0.01;
export const ZOOM_MIN = 0.25;
export const ZOOM_MAX = 8;
export { DEFAULT_CAMERA };

const finite = (v: unknown, fallback: number) => (typeof v === "number" && Number.isFinite(v) ? v : fallback);

export function clampCamera(c: Partial<Camera>): Camera {
  const yaw = finite(c.yaw, DEFAULT_CAMERA.yaw);
  const turn = 2 * Math.PI;
  return {
    yaw: yaw - turn * Math.floor((yaw + Math.PI) / turn),
    pitch: Math.min(PITCH_LIMIT, Math.max(-PITCH_LIMIT, finite(c.pitch, DEFAULT_CAMERA.pitch))),
    zoom: Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, finite(c.zoom, DEFAULT_CAMERA.zoom))),
  };
}

/**
 * How a view draws its two axes. `plane` is the phase plane and the time
 * series (the axes tell them apart); `cobweb` is the cobweb plot of a 1-D
 * map x_{n+1} = f(x_n), whose two axes are both state 0 (x_n across, x_{n+1}
 * up) and whose ranges are kept at equal scales; `section` is a flow's
 * Poincaré section, its crossings plotted in two states, or as the return
 * map x_k -> x_{k+1} when both axes are one state (the cobweb's convention);
 * `space` is the 3D view of a flow, three states in a box seen from `camera`.
 */
interface ViewBase {
  xAxis: AxisRef;
  yAxis: AxisRef;
  xRange: [number, number];
  yRange: [number, number];
}

export type SectionView = ViewBase & { kind: "section"; section: SectionSpec };
export type StateAxis = Extract<AxisRef, { kind: "state" }>;
export type SpaceView = Omit<ViewBase, "xAxis" | "yAxis"> & {
  kind: "space";
  xAxis: StateAxis;
  yAxis: StateAxis;
  zAxis: StateAxis;
  zRange: [number, number];
  camera: Camera;
};
export type TimeAxis = Extract<AxisRef, { kind: "time" }>;

export interface LyapunovSettings {
  transient: number;
  total: number;
  interval: number;
  blocks: number;
}

export type LyapunovView = ViewBase & {
  kind: "lyapunov";
  xAxis: TimeAxis;
  yAxis: TimeAxis;
  lyapunov: LyapunovSettings;
};

export interface OrbitSettings {
  transient: number;
  kept: number;
}

export type OrbitView = ViewBase & {
  kind: "orbit";
  xAxis: TimeAxis;
  yAxis: StateAxis;
  param: number;
  orbit: OrbitSettings;
};
export type NormAxis = { kind: "norm" };

export type ContinuationSeed =
  | { from: "point"; x: number[]; param: number; period: number; direction: ContinuationDirection }
  | { from: "switch"; at: SwitchPoint; direction: ContinuationDirection };

export interface ContinuationSettings {
  range: [number, number];
  ds: number;
  maxPoints: number;
  seeds: ContinuationSeed[];
}

export type ContinuationView = Omit<ViewBase, "yAxis"> & {
  kind: "continuation";
  xAxis: TimeAxis;
  yAxis: StateAxis | NormAxis;
  param: number;
  continuation: ContinuationSettings;
};

export type View =
  | (ViewBase & { kind?: "plane" })
  | (ViewBase & { kind: "cobweb" })
  | SectionView
  | SpaceView
  | LyapunovView
  | OrbitView
  | ContinuationView
  | CurvesView;
export type ViewKind = NonNullable<View["kind"]>;
export type PlotView = Exclude<View, ContinuationView | CurvesView>;

export const viewKind = (v: Pick<View, "kind">): ViewKind => v.kind ?? "plane";
export const isCobweb = (v: Pick<View, "kind">): boolean => v.kind === "cobweb";
export const isSection = (v: View): v is SectionView => v.kind === "section";
export const isSpace = (v: View): v is SpaceView => v.kind === "space";
export const isLyapunov = (v: View): v is LyapunovView => v.kind === "lyapunov";
export const isOrbit = (v: View): v is OrbitView => v.kind === "orbit";
export const isContinuation = (v: View): v is ContinuationView => v.kind === "continuation";
export const isCurves = (v: Pick<View, "kind">): v is CurvesView => v.kind === "curves";
export const isAnalysis = (v: View): v is LyapunovView | OrbitView | ContinuationView | CurvesView =>
  isLyapunov(v) || isOrbit(v) || isContinuation(v) || isCurves(v);
/** The Branch view and the Parameter plane: parameters on the axes, no portrait and no start points. */
export const isParameterView = (v: View): v is ContinuationView | CurvesView => isContinuation(v) || isCurves(v);
export const isReturnMap = (v: View): v is SectionView =>
  v.kind === "section" && v.xAxis.kind === "state" && v.yAxis.kind === "state" && v.xAxis.index === v.yAxis.index;

/** Every option of the View control, in the order shown. */
export type ViewChoice =
  | "phase"
  | "series"
  | "cobweb"
  | "section"
  | "space"
  | "lyapunov"
  | "orbit"
  | "continuation"
  | "curves";
export const VIEW_CHOICES: readonly ViewChoice[] = [
  "phase",
  "series",
  "cobweb",
  "section",
  "space",
  "lyapunov",
  "orbit",
  "continuation",
  "curves",
];

export const VIEW_CHOICES_BY_KIND: Record<SystemKind, readonly ViewChoice[]> = {
  ode: ["phase", "series", "section", "space", "lyapunov", "continuation", "curves"],
  map: ["phase", "series", "cobweb", "lyapunov", "orbit", "continuation", "curves"],
};

export const viewChoicesFor = (sys: Pick<CompileResponse, "kind"> | null | undefined): readonly ViewChoice[] =>
  VIEW_CHOICES_BY_KIND[sys?.kind ?? "ode"];

/** Which of the View control's options `v` is. */
export function viewChoice(v: View): ViewChoice {
  if (isCobweb(v)) return "cobweb";
  if (isSection(v)) return "section";
  if (isSpace(v)) return "space";
  if (isLyapunov(v)) return "lyapunov";
  if (isOrbit(v)) return "orbit";
  if (isContinuation(v)) return "continuation";
  if (isCurves(v)) return "curves";
  return v.xAxis.kind === "time" || v.yAxis.kind === "time" ? "series" : "phase";
}

export type Availability = { ok: true } | { ok: false; reason: string };

type SystemShape = Pick<CompileResponse, "kind" | "autonomous" | "states"> & Partial<Pick<CompileResponse, "params">>;

/**
 * The cobweb view can show `sys`: a one-dimensional autonomous map. `name`
 * is the system's title, for the reason ("Hénon map has 2 states"). The
 * reasons are the core's (`cobweb_fits` in service.rs), with the system's
 * name and the maths between `$…$` to typeset.
 */
export function cobwebAvailability(sys: SystemShape | null | undefined, name = "this system"): Availability {
  const needs = "The cobweb view needs a one-dimensional map $x_{n+1} = f(x_n)$";
  if (!sys || sys.kind !== "map") return { ok: false, reason: `${needs}; ${name} is a flow.` };
  const dim = sys.states.length;
  if (dim !== 1) return { ok: false, reason: `${needs}; ${name} has ${dim} states.` };
  if (!sys.autonomous) return { ok: false, reason: "This map changes with n, so f has no single graph to cobweb." };
  return { ok: true };
}

/** The phase plane can show `sys`: it has two states or more. */
export function phasePlaneAvailability(sys: SystemShape | null | undefined, name = "This system"): Availability {
  const dim = sys?.states.length ?? 0;
  return dim >= 2
    ? { ok: true }
    : { ok: false, reason: `A phase plane needs at least 2 states. ${name} has ${dim}.` };
}

export function sectionAvailability(sys: SystemShape | null | undefined, name = "this system"): Availability {
  return sys && sys.kind !== "map" ? { ok: true } : { ok: false, reason: `A Poincaré section needs a flow; ${name} is a map.` };
}

export function planeSectionAvailability(sys: SystemShape | null | undefined, name = "This system"): Availability {
  const dim = sys?.states.length ?? 0;
  return dim >= 2
    ? { ok: true }
    : { ok: false, reason: `A plane section needs at least 2 states. ${name} has ${dim}; take a stroboscopic section.` };
}

export function orbitAvailability(sys: SystemShape | null | undefined, name = "this system"): Availability {
  const needs = "The orbit diagram needs a map with a parameter";
  if (!sys || sys.kind !== "map") return { ok: false, reason: `${needs}; ${name} is a flow.` };
  return sys.params?.length ? { ok: true } : { ok: false, reason: `${needs}; ${name} has none.` };
}

export function continuationAvailability(sys: SystemShape | null | undefined, name = "this system"): Availability {
  const needs = "Continuation follows a fixed point as a parameter changes";
  if (!sys?.params?.length) return { ok: false, reason: `${needs}; ${name} has no parameter.` };
  if (!sys.autonomous)
    return { ok: false, reason: `${needs}; ${name} changes with ${sys.kind === "map" ? "$n$" : "$t$"}, so it has none.` };
  return { ok: true };
}

export function spaceAvailability(sys: SystemShape | null | undefined, name = "This system"): Availability {
  const needs = "The 3D view needs a flow with at least 3 states.";
  if (sys?.kind === "map") return { ok: false, reason: `${needs} ${name} is a map.` };
  const dim = sys?.states.length ?? 0;
  return dim >= 3 ? { ok: true } : { ok: false, reason: `${needs} ${name} has ${dim}.` };
}

export const VIEW_CHOICE_LABELS: Record<ViewChoice, string> = {
  phase: "Phase plane",
  series: "Time series",
  cobweb: "Cobweb",
  section: "Poincaré section",
  space: "3D view",
  lyapunov: "Lyapunov spectrum",
  orbit: "Orbit diagram",
  continuation: "Continuation",
  curves: "Parameter plane",
};

/**
 * The View control's visible labels, short enough for three equal segments
 * of a 1,024 px window's inspector in projector mode; the full names stay
 * in each segment's title and accessible name. The Parameter plane comes
 * last and spans a row of its own, so it keeps its full name.
 */
export const VIEW_CHOICE_SHORT: Record<ViewChoice, string> = {
  phase: "Phase",
  series: "Series",
  cobweb: "Cobweb",
  section: "Section",
  space: "3D",
  lyapunov: "λ",
  orbit: "Orbit",
  continuation: "Branch",
  curves: "Parameter plane",
};

/** Which of the View control's options `sys` can show, with the reason for each it can't. */
export function viewChoiceAvailability(
  sys: SystemShape | null | undefined,
  name?: string,
): Record<ViewChoice, Availability> {
  return {
    phase: phasePlaneAvailability(sys, name),
    series: { ok: true },
    cobweb: cobwebAvailability(sys, name),
    section: sectionAvailability(sys, name),
    space: spaceAvailability(sys, name),
    lyapunov: { ok: true },
    orbit: orbitAvailability(sys, name),
    continuation: continuationAvailability(sys, name),
    curves: curvesAvailability(sys, name),
  };
}

export function defaultSection(sys: Pick<CompileResponse, "autonomous" | "states">, middle: number): SectionSpec {
  return !sys.autonomous || sys.states.length < 2
    ? { kind: "stroboscopic", period: 2 * Math.PI, phase: 0 }
    : { kind: "plane", state: 0, value: middle, direction: "rising" };
}

export function sectionView(
  sys: Pick<CompileResponse, "states">,
  section: SectionSpec,
  rangeOf: (i: number) => [number, number],
): SectionView {
  const free = sys.states.map((_, i) => i).filter((i) => section.kind !== "plane" || i !== section.state);
  const x = free[0] ?? 0;
  const y = free[1] ?? x;
  const xr = rangeOf(x);
  const yr = rangeOf(y);
  return { kind: "section", section, xAxis: stateAxis(x), yAxis: stateAxis(y), xRange: [xr[0], xr[1]], yRange: [yr[0], yr[1]] };
}

const isSingleValue = ([lo, hi]: readonly [number, number]) => lo === hi;

export function knownStateRange(
  i: number,
  views: readonly (View | undefined)[],
  res: PortraitResponse | undefined,
  dim: number,
): [number, number] {
  for (const v of views) {
    if (v?.xAxis.kind === "state" && v.xAxis.index === i) return v.xRange;
    if (v?.yAxis.kind === "state" && v.yAxis.index === i) return v.yRange;
    if (v && isSpace(v) && v.zAxis.kind === "state" && v.zAxis.index === i) return v.zRange;
  }
  const ext = stateExtent(res, dim, i);
  if (!ext || isSingleValue(ext)) return [-5, 5];
  const pad = Math.max(0.1 * (ext[1] - ext[0]), 0.5);
  return [ext[0] - pad, ext[1] + pad];
}

export function withSection(
  view: SectionView,
  sys: Pick<CompileResponse, "states">,
  section: SectionSpec,
  rangeOf: (i: number) => [number, number],
): SectionView {
  const onPlane = (a: AxisRef) => section.kind === "plane" && a.kind === "state" && a.index === section.state;
  return onPlane(view.xAxis) || onPlane(view.yAxis) ? sectionView(sys, section, rangeOf) : { ...view, section };
}

export function spaceView(
  axes: [number, number, number],
  rangeOf: (i: number) => [number, number],
  camera: Camera = DEFAULT_CAMERA,
): SpaceView {
  const r = axes.map((i) => rangeOf(i));
  return {
    kind: "space",
    xAxis: stateAxis(axes[0]),
    yAxis: stateAxis(axes[1]),
    zAxis: stateAxis(axes[2]),
    xRange: [r[0][0], r[0][1]],
    yRange: [r[1][0], r[1][1]],
    zRange: [r[2][0], r[2][1]],
    camera: clampCamera(camera),
  };
}

export const spaceStates = (v: Pick<SpaceView, "xAxis" | "yAxis" | "zAxis">): [number, number, number] => [
  v.xAxis.index,
  v.yAxis.index,
  v.zAxis.index,
];

export function withSpaceAxis(
  view: SpaceView,
  slot: 0 | 1 | 2,
  index: number,
  rangeOf: (i: number) => [number, number],
): SpaceView {
  const axes = spaceStates(view);
  const ranges: [number, number][] = [view.xRange, view.yRange, view.zRange];
  const from = axes.indexOf(index);
  if (from === slot) return view;
  if (from >= 0) {
    [axes[from], axes[slot]] = [axes[slot], axes[from]];
    [ranges[from], ranges[slot]] = [ranges[slot], ranges[from]];
  } else {
    axes[slot] = index;
    ranges[slot] = rangeOf(index);
  }
  return spaceView(axes, (i) => ranges[axes.indexOf(i)], view.camera);
}

export function viewRequestKey(v: View): string {
  if (!isSpace(v)) return JSON.stringify(v);
  const { camera: _camera, ...rest } = v;
  return JSON.stringify(rest);
}

/** Graph samples of f a cobweb asks for on screen: at least one per CSS px up to 2,048 px. */
export const COBWEB_GRAPH_SAMPLES = 2048;
/** The most graph samples the core takes (`dyn_analysis::MAX_GRAPH_SAMPLES`). */
export const MAX_GRAPH_SAMPLES = 4096;
/** The most iterates the core computes per orbit, transient included (`MAX_ITERATES` in service.rs). */
export const MAX_ITERATES = 1_000_000;
/** Above this many iterates the Iterates field says a cobweb fills in. */
export const COBWEB_FILL_ITERATES = 10_000;

/** The fields a portrait request carries for maps: f's graph (the cobweb view) and the transient. */
export type MapRequestFields = Pick<PortraitRequest, "graphSamples" | "transient">;

/** The graph of f a cobweb portrait came back with, or undefined when none was asked for. */
export function portraitGraph(res: PortraitResponse): [number, number][][] | undefined {
  return res.graph ?? undefined;
}

/** The map's view a cobweb shows: both axes state 0, the same range on each. */
export function cobwebView(range: [number, number]): PlotView {
  return {
    kind: "cobweb",
    xAxis: stateAxis(0),
    yAxis: stateAxis(0),
    xRange: [range[0], range[1]],
    yRange: [range[0], range[1]],
  };
}

export const LYAPUNOV_DEFAULTS: Record<SystemKind, LyapunovSettings> = {
  ode: { transient: 100, total: 1000, interval: 0.1, blocks: 10 },
  map: { transient: 1000, total: 100_000, interval: 1, blocks: 10 },
};

export function lyapunovView(kind: SystemKind, settings: LyapunovSettings = LYAPUNOV_DEFAULTS[kind]): LyapunovView {
  const { transient, total } = settings;
  return {
    kind: "lyapunov",
    xAxis: TIME_AXIS,
    yAxis: TIME_AXIS,
    xRange: [transient, transient + total],
    yRange: [-1, 1],
    lyapunov: { ...settings },
  };
}

export function renormalizations(s: Pick<LyapunovSettings, "total" | "interval">, kind: SystemKind | undefined): number {
  if (kind === "map") return Math.ceil(Math.round(s.total) / Math.max(1, Math.round(s.interval)));
  const r = s.total / s.interval;
  const k = Math.round(r);
  return Math.abs(r - k) <= 1e-9 * Math.max(1, r) ? k : Math.ceil(r);
}

export function lyapunovSettingsError(
  s: LyapunovSettings,
  kind: SystemKind | undefined,
): { field: keyof LyapunovSettings; message: string } | null {
  const map = kind === "map";
  const whole = (v: number) => !map || Number.isInteger(v);
  if (!(Number.isFinite(s.transient) && s.transient >= 0 && whole(s.transient)))
    return { field: "transient", message: map ? "Must be a whole number, 0 or more" : "Must be 0 or more" };
  if (!(Number.isFinite(s.total) && s.total > 0 && whole(s.total)))
    return { field: "total", message: map ? "Must be a whole number above 0" : "Must be above 0" };
  if (map && s.transient + s.total > MAX_LYAPUNOV_ITERATES)
    return { field: "transient", message: `Transient and iterates together at most ${MAX_LYAPUNOV_ITERATES.toLocaleString("en-US")}` };
  if (!(Number.isFinite(s.interval) && s.interval > 0 && whole(s.interval)))
    return { field: "interval", message: map ? "Must be a whole number above 0" : "Must be above 0" };
  const k = renormalizations(s, kind);
  if (k > MAX_RENORMALIZATIONS)
    return { field: "interval", message: `${k.toLocaleString("en-US")} QR steps; at most ${MAX_RENORMALIZATIONS.toLocaleString("en-US")} fit in one run` };
  if (!(Number.isInteger(s.blocks) && s.blocks >= 2 && s.blocks <= k))
    return { field: "blocks", message: `Must be a whole number from 2 to ${k.toLocaleString("en-US")}, the QR steps` };
  return null;
}

export function lyapunovStart(
  ics: readonly InitialCondition[],
  selection: Selection,
  dim: number,
): InitialCondition | null {
  const fits = (ic: InitialCondition) => ic.x0.length === dim;
  const picked = selection?.kind === "ic" ? ics.find((ic) => ic.id === selection.id && fits(ic)) : undefined;
  return picked ?? ics.find(fits) ?? null;
}

const LYAPUNOV_MAX_STEPS = 1e9;
const ADAPTIVE_STEPS_PER_INTERVAL = 100;

export function lyapunovSolver(solver: SolverSettings, settings: LyapunovSettings, kind: SystemKind | undefined): SolverOptions {
  const fixed = !METHODS[solver.method].adaptive;
  const h = fixed && solver.h <= 0 ? DEFAULT_FIXED_STEP : solver.h;
  const k = renormalizations(settings, kind);
  const need = fixed
    ? Math.ceil((settings.transient + settings.total) / h) + k + 2
    : ADAPTIVE_STEPS_PER_INTERVAL * (k + 1);
  return {
    method: solver.method,
    rtol: solver.rtol,
    atol: solver.atol,
    h,
    maxSteps: Math.min(LYAPUNOV_MAX_STEPS, Math.max(solver.maxSteps, need)),
  };
}

export interface LyapunovMeta {
  key: string;
  systemId: number;
  shapeId: number;
  kind: SystemKind;
  icId: number;
  settings: LyapunovSettings;
  solver: SolverOptions;
}

export interface LyapunovState {
  res: LyapunovResponse;
  meta: LyapunovMeta;
  roundTripMs: number;
  /** The first run after a project file opened: the plot keeps the file's framing instead of fitting this run. */
  framedByFile?: boolean;
}

export type LyapunovJob = Job<Omit<LyapunovRequest, "requestId">, LyapunovMeta>;

export function buildLyapunovJob(
  s: Pick<StudioSnapshot, "compiled" | "params" | "view" | "ics" | "solver" | "shapeId"> & { selection: Selection },
): LyapunovJob | null {
  const sys = s.compiled;
  if (!sys || !sys.ok || sys.systemId === undefined || !sys.kind || !isLyapunov(s.view)) return null;
  const settings = s.view.lyapunov;
  if (lyapunovSettingsError(settings, sys.kind)) return null;
  const ic = lyapunovStart(s.ics, s.selection, sys.states.length);
  if (!ic) return null;
  const solver = lyapunovSolver(s.solver, settings, sys.kind);
  const body: LyapunovJob["body"] = {
    systemId: sys.systemId,
    params: s.params.map((p) => p.value),
    x0: ic.x0.slice(),
    t0: sys.kind === "map" ? 0 : ic.t0,
    transient: settings.transient,
    total: settings.total,
    interval: settings.interval,
    blocks: settings.blocks,
    solver,
  };
  return {
    body,
    meta: {
      key: JSON.stringify(body),
      systemId: sys.systemId,
      shapeId: s.shapeId,
      kind: sys.kind,
      icId: ic.id,
      settings: { ...settings },
      solver,
    },
  };
}

export function lyapunovCurrent(
  s: Parameters<typeof buildLyapunovJob>[0] & { lyapunov: LyapunovState | null },
): boolean {
  const job = buildLyapunovJob(s);
  return !!job && s.lyapunov?.meta.key === job.meta.key;
}

export function lyapunovFits(
  sys: CompileResponse | null,
  shapeId: number,
  l: LyapunovState | null,
): l is LyapunovState {
  return !!sys && !!l && l.meta.shapeId === shapeId && l.res.exponents.length === sys.states.length;
}

export function fitLyapunovRanges(res: Pick<LyapunovResponse, "historyT" | "history" | "exponents">): {
  xRange: [number, number];
  yRange: [number, number];
} | null {
  const n = res.exponents.length;
  const m = res.historyT.length;
  if (n === 0 || m === 0) return null;
  const t0 = res.historyT[0];
  const t1 = res.historyT[m - 1];
  const settled = t0 + 0.1 * (t1 - t0);
  let lo = 0;
  let hi = 0;
  const take = (v: number) => {
    if (!Number.isFinite(v)) return;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  };
  res.exponents.forEach(take);
  for (let k = 0; k < m; k++) {
    if (res.historyT[k] < settled) continue;
    for (let i = 0; i < n; i++) take(res.history[k * n + i]);
  }
  const span = hi - lo || 1;
  const xSpan = t1 - t0 || 1;
  return {
    xRange: [t0, t1 + 0.02 * xSpan],
    yRange: [lo - 0.08 * span, hi + 0.14 * span],
  };
}

const MINUS = "\u2212";
const withRealMinus = (text: string) => (Number(text) === 0 ? text.replace(/^-/, "") : text).replace(/^-/, MINUS);

export function roundWithUncertainty(v: number, u: number | undefined): { value: string; error: string | null } {
  if (!Number.isFinite(v)) return { value: String(v), error: null };
  if (u === undefined || !(u > 0) || !Number.isFinite(u)) {
    const text = Number(v.toPrecision(4)).toString();
    return { value: text, error: null };
  }
  let d = -Math.floor(Math.log10(u));
  const carriesIntoNextDecade = Math.round(u * 10 ** d) >= 10;
  if (carriesIntoNextDecade) d -= 1;
  const leadsWithOne = Math.round(u * 10 ** d) === 1;
  if (leadsWithOne) d += 1;
  const places = Math.max(0, Math.min(20, d));
  return { value: v.toFixed(places), error: u.toFixed(places) };
}

export function lyapunovValueText(v: number, u: number | undefined): string {
  const r = roundWithUncertainty(v, u);
  return r.error === null ? withRealMinus(r.value) : `${withRealMinus(r.value)} ± ${r.error}`;
}

const SUPERSCRIPTS: Record<string, string> = {
  "-": "⁻",
  "0": "⁰",
  "1": "¹",
  "2": "²",
  "3": "³",
  "4": "⁴",
  "5": "⁵",
  "6": "⁶",
  "7": "⁷",
  "8": "⁸",
  "9": "⁹",
};
const superscript = (e: number) => String(e).replace(/./g, (c) => SUPERSCRIPTS[c] ?? c);

export function countText(n: number): string {
  const e = Math.round(Math.log10(n));
  return n >= 1000 && 10 ** e === n ? `10${superscript(e)}` : n.toLocaleString("en-US");
}

export function scientificText(v: number): string {
  if (v === 0) return "0";
  if (!Number.isFinite(v)) return String(v);
  const [m, e] = v.toExponential(1).split("e");
  return `${withRealMinus(m)} × 10${superscript(Number(e))}`;
}

export const ORBIT_DEFAULTS: OrbitSettings = { transient: 1000, kept: 200 };
export { ORBIT_MAX_KEPT, ORBIT_MAX_TRANSIENT };

export function orbitView(
  params: readonly Pick<ParamState, "min" | "max">[],
  yRange: [number, number],
  param = 0,
  state = 0,
  settings: OrbitSettings = ORBIT_DEFAULTS,
): OrbitView {
  const p = params[param];
  return {
    kind: "orbit",
    xAxis: TIME_AXIS,
    yAxis: stateAxis(state),
    xRange: p && p.min < p.max ? [p.min, p.max] : [0, 1],
    yRange: [yRange[0], yRange[1]],
    param,
    orbit: { ...settings },
  };
}

export function orbitColumns(cssWidth: number, pixelRatio: number): number {
  const n = Math.round(cssWidth * pixelRatio);
  return Number.isFinite(n) ? Math.min(MAX_ORBIT_COLUMNS, Math.max(2, n)) : 2;
}

export function orbitRange(xRange: [number, number], columns: number): [number, number] {
  const half = (xRange[1] - xRange[0]) / (2 * columns);
  return [xRange[0] + half, xRange[1] - half];
}

/**
 * The column at fraction `f` of the plot's width (0 at the left edge). A
 * fraction rather than an r, so that a click on a pixel's left edge, where
 * every integer clientX lands, does not round into the column before it.
 */
export function orbitColumnAt(xRange: [number, number], columns: number, f: number): { column: number; r: number } {
  const [lo, hi] = orbitRange(xRange, columns);
  const column = Math.min(columns - 1, Math.max(0, Math.floor(f * columns)));
  return { column, r: lo + column * ((hi - lo) / (columns - 1)) };
}

export function orbitSettingsError(s: OrbitSettings): { field: keyof OrbitSettings; message: string } | null {
  if (!(Number.isInteger(s.transient) && s.transient >= 0 && s.transient <= ORBIT_MAX_TRANSIENT))
    return { field: "transient", message: `Must be a whole number from 0 to ${ORBIT_MAX_TRANSIENT.toLocaleString("en-US")}` };
  if (!(Number.isInteger(s.kept) && s.kept >= 1 && s.kept <= ORBIT_MAX_KEPT))
    return { field: "kept", message: `Must be a whole number from 1 to ${ORBIT_MAX_KEPT.toLocaleString("en-US")}` };
  return null;
}

export const findsDoublings = (sys: Pick<CompileResponse, "states" | "autonomous">): boolean =>
  sys.states.length === 1 && sys.autonomous;

export interface OrbitMeta {
  key: string;
  systemId: number;
  shapeId: number;
  param: number;
  state: number;
}

export interface OrbitState {
  res: OrbitDiagramResponse;
  meta: OrbitMeta;
  roundTripMs: number;
}

export type OrbitJob = Job<Omit<OrbitDiagramRequest, "requestId">, OrbitMeta>;

export function buildOrbitJob(
  s: Pick<StudioSnapshot, "compiled" | "params" | "view" | "ics" | "shapeId"> & {
    selection: Selection;
    orbitColumns: number;
  },
): OrbitJob | null {
  const sys = s.compiled;
  if (!sys || !sys.ok || sys.systemId === undefined || sys.kind !== "map" || !isOrbit(s.view)) return null;
  const v = s.view;
  if (s.orbitColumns < 2 || v.param >= sys.params.length || v.yAxis.index >= sys.states.length) return null;
  if (orbitSettingsError(v.orbit)) return null;
  const ic = lyapunovStart(s.ics, s.selection, sys.states.length);
  if (!ic) return null;
  const range = orbitRange(v.xRange, s.orbitColumns);
  const body: OrbitJob["body"] = {
    systemId: sys.systemId,
    params: s.params.map((p, i) => (i === v.param ? range[0] : p.value)),
    param: v.param,
    range,
    columns: s.orbitColumns,
    state: v.yAxis.index,
    x0: ic.x0.slice(),
    transient: v.orbit.transient,
    kept: v.orbit.kept,
    doublings: findsDoublings(sys),
  };
  return {
    body,
    meta: { key: JSON.stringify(body), systemId: sys.systemId, shapeId: s.shapeId, param: v.param, state: v.yAxis.index },
  };
}

export function orbitFits(
  sys: CompileResponse | null,
  shapeId: number,
  view: View,
  o: OrbitState | null,
): o is OrbitState {
  return !!sys && !!o && isOrbit(view) && o.meta.shapeId === shapeId && o.meta.param === view.param && o.meta.state === view.yAxis.index;
}

export function orbitCurrent(s: Parameters<typeof buildOrbitJob>[0] & { orbit: OrbitState | null }): boolean {
  const job = buildOrbitJob(s);
  return !!job && s.orbit?.meta.key === job.meta.key;
}

export function fitOrbitRange(res: Pick<OrbitDiagramResponse, "x">): [number, number] | null {
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of res.x) {
    if (v === null || !Number.isFinite(v)) continue;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  if (!(lo <= hi)) return null;
  const pad = 0.08 * (hi - lo || Math.max(1, Math.abs(lo)));
  return [lo - pad, hi + pad];
}

export function doublingRows(
  doublings: readonly { r: number; period: number }[],
  name = "r",
): { label: string; r: number; period: number }[] {
  let chain = true;
  return doublings.map((d, i) => {
    const n = Math.log2(d.period);
    chain = chain && Number.isInteger(n) && (i === 0 || d.period === 2 * doublings[i - 1].period);
    return { label: chain ? `${name}${subscriptDigits(n + 1)}` : `${d.period} → ${2 * d.period}`, r: d.r, period: d.period };
  });
}

const subscriptDigits = (n: number) => String(n).replace(/\d/g, (d) => "₀₁₂₃₄₅₆₇₈₉"[Number(d)]);

export interface SpecialKindInfo {
  label: string;
  name: string;
  omega: "ω" | "θ" | null;
  switches: boolean;
}

export const SPECIAL_KINDS: Record<SpecialKindOut, SpecialKindInfo> = {
  fold: { label: "LP", name: "Fold (limit point)", omega: null, switches: false },
  hopf: { label: "H", name: "Hopf bifurcation", omega: "ω", switches: false },
  branchPoint: { label: "BP", name: "Branch point", omega: null, switches: true },
  flip: { label: "PD", name: "Period doubling (flip)", omega: null, switches: true },
  neimarkSacker: { label: "NS", name: "Neimark–Sacker bifurcation", omega: "θ", switches: false },
};

export const CONTINUATION_MAX_POINTS = 500;
export { MAX_CONTINUATION_POINTS };

export const continuationStep = ([lo, hi]: readonly [number, number]): number => Number(((hi - lo) / 100).toPrecision(1));

export function continuationXRange([lo, hi]: readonly [number, number]): [number, number] {
  const pad = 0.04 * (hi - lo);
  return [lo - pad, hi + pad];
}

export function continuationView(
  params: readonly Pick<ParamState, "min" | "max">[],
  yRange: [number, number],
  param = 0,
  seeds: ContinuationSeed[] = [],
): ContinuationView {
  const p = params[param];
  const range: [number, number] = p && p.min < p.max ? [p.min, p.max] : [0, 1];
  return {
    kind: "continuation",
    xAxis: TIME_AXIS,
    yAxis: stateAxis(0),
    xRange: continuationXRange(range),
    yRange: [yRange[0], yRange[1]],
    param,
    continuation: { range, ds: continuationStep(range), maxPoints: CONTINUATION_MAX_POINTS, seeds },
  };
}

export function continuationSettingsError(
  s: Omit<ContinuationSettings, "seeds">,
): { field: "range" | "ds" | "maxPoints"; message: string } | null {
  const [lo, hi] = s.range;
  if (!(Number.isFinite(lo) && Number.isFinite(hi) && lo < hi)) return { field: "range", message: "From must be below To" };
  if (!(Number.isFinite(s.ds) && s.ds > 0 && s.ds <= hi - lo))
    return { field: "ds", message: "Must be positive and at most the range" };
  if (!(Number.isInteger(s.maxPoints) && s.maxPoints >= 2 && s.maxPoints <= MAX_CONTINUATION_POINTS))
    return { field: "maxPoints", message: `Must be a whole number from 2 to ${MAX_CONTINUATION_POINTS.toLocaleString("en-US")}` };
  return null;
}

export const pointSeed = (x: readonly number[], param: number, direction: ContinuationDirection): ContinuationSeed => ({
  from: "point",
  x: x.slice(),
  param,
  period: 1,
  direction,
});
export const switchSeed = ({ kind, p, x, period, tangent }: SwitchPoint): ContinuationSeed => ({
  from: "switch",
  at: { kind, p, x: x.slice(), period, tangent: tangent.slice() },
  direction: "both",
});

/** The number (from 1) of the branch already switched onto at `at`, if any. */
export function switchedBranch(seeds: readonly ContinuationSeed[], at: SwitchPoint): number | null {
  const i = seeds.findIndex((s) => s.from === "switch" && s.at.kind === at.kind && s.at.period === at.period && s.at.p === at.p);
  return i < 0 ? null : i + 1;
}

export function withSeed(seeds: readonly ContinuationSeed[], seed: ContinuationSeed): ContinuationSeed[] {
  const key = JSON.stringify(seed);
  return seeds.some((s) => JSON.stringify(s) === key) ? seeds.slice() : [...seeds, seed];
}

export interface ContinuationMeta {
  key: string;
  systemId: number;
  seed: number;
}

export type ContinuationJob = Job<Omit<ContinuationRequest, "requestId">, ContinuationMeta>;

export type BranchRun = { ok: true; res: ContinuationResponse; roundTripMs: number } | { ok: false; error: string };

export function buildContinuationJobs(s: Pick<StudioSnapshot, "compiled" | "params" | "view">): ContinuationJob[] {
  const sys = s.compiled;
  if (!sys || !sys.ok || sys.systemId === undefined || !isContinuation(s.view) || !continuationAvailability(sys).ok)
    return [];
  const v = s.view;
  const c = v.continuation;
  if (v.param >= sys.params.length || s.params.length !== sys.params.length || continuationSettingsError(c)) return [];
  const systemId = sys.systemId;
  const dim = sys.states.length;
  return c.seeds.flatMap((seed, i): ContinuationJob[] => {
    const [x, p, start]: [number[], number, ContinuationStart] =
      seed.from === "point"
        ? [seed.x, seed.param, { kind: "point", x0: seed.x.slice(), period: seed.period }]
        : [seed.at.x, seed.at.p, { kind: "switch", at: seed.at }];
    if (x.length !== dim) return [];
    const body: ContinuationJob["body"] = {
      systemId,
      params: s.params.map((q, j) => (j === v.param ? p : q.value)),
      param: v.param,
      range: [c.range[0], c.range[1]],
      start,
      direction: seed.direction,
      ds: c.ds,
      dsMax: 2 * c.ds,
      maxPoints: c.maxPoints,
    };
    return [{ body, meta: { key: JSON.stringify(body), systemId, seed: i } }];
  });
}

export function shownBranches(
  s: Parameters<typeof buildContinuationJobs>[0] & { branches: Readonly<Record<string, BranchRun>> },
): { job: ContinuationJob; run: BranchRun | undefined }[] {
  return buildContinuationJobs(s).map((job) => ({ job, run: s.branches[job.meta.key] }));
}

const isSameBifurcation = (a: SpecialPointOut, b: SpecialPointOut): boolean =>
  a.kind === b.kind && a.period === b.period && Math.abs(a.p - b.p) <= SAME_BIFURCATION_TOL * (1 + Math.abs(b.p));

export function specialRows(runs: readonly (BranchRun | undefined)[]): { at: SpecialPointOut; branch: number }[] {
  const rows: { at: SpecialPointOut; branch: number }[] = [];
  runs.forEach((run, branch) => {
    if (!run?.ok) return;
    for (const at of run.res.branch.special) {
      if (!rows.some((r) => isSameBifurcation(r.at, at))) rows.push({ at, branch });
    }
  });
  return rows.sort((a, b) => a.at.p - b.at.p);
}

/**
 * The bifurcations the Branch view `branch` found that a curve can be
 * followed from (LP, H, PD, NS), each with every parameter's value there.
 */
export function branchSeeds(
  sys: CompileResponse,
  params: readonly Partial<Pick<ParamState, "value">>[],
  branch: ContinuationView | null,
  branches: Readonly<Record<string, BranchRun>>,
): PlaneSeed[] {
  if (!branch) return [];
  const values = params.map((p) => p.value ?? 0);
  const runs = shownBranches({ compiled: sys, params: values.map((value) => ({ value }) as ParamState), view: branch, branches });
  return specialRows(runs.map((b) => b.run)).flatMap(({ at }) => planeSeed(at, values, branch.param) ?? []);
}

export function continuationSummary(
  runs: readonly (BranchRun | undefined)[],
): { elapsedMs: number; roundTripMs: number; points: number; special: number } {
  let elapsedMs = 0;
  let roundTripMs = 0;
  let points = 0;
  for (const run of runs) {
    if (!run?.ok) continue;
    elapsedMs += run.res.elapsedMs;
    roundTripMs += run.roundTripMs;
    points += run.res.branch.p.length;
  }
  return { elapsedMs, roundTripMs, points, special: specialRows(runs).length };
}

export function branchValue(
  b: Pick<ContinuationResponse["branch"], "x" | "period">,
  dim: number,
  axis: StateAxis | NormAxis,
  i: number,
  m: number,
): number {
  const at = (i * b.period + m) * dim;
  if (axis.kind === "state") return b.x[at + axis.index];
  let sum = 0;
  for (let k = 0; k < dim; k++) sum += b.x[at + k] * b.x[at + k];
  return Math.sqrt(sum);
}

export function fitBranchRanges(
  runs: readonly (BranchRun | undefined)[],
  dim: number,
  axis: StateAxis | NormAxis,
): { xRange: [number, number]; yRange: [number, number] } | null {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const run of runs) {
    if (!run?.ok) continue;
    const b = run.res.branch;
    b.p.forEach((p, i) => {
      for (let m = 0; m < b.period; m++) {
        const y = branchValue(b, dim, axis, i, m);
        if (Number.isFinite(p) && Number.isFinite(y) && Math.abs(y) < OUTLIER_CAP) {
          xs.push(p);
          ys.push(y);
        }
      }
    });
  }
  const pad = (vs: number[]): [number, number] | null => {
    if (!vs.length) return null;
    let lo = Infinity;
    let hi = -Infinity;
    for (const v of vs) {
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    const d = 0.08 * (hi - lo || Math.max(1, Math.abs(lo)));
    return [lo - d, hi + d];
  };
  const xRange = pad(xs);
  const yRange = pad(ys);
  return xRange && yRange ? { xRange, yRange } : null;
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
  /** Map iterates computed but not drawn before each orbit (maps only). */
  transient: number;
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
export function fixedPointMatchTolerance(view: PlotView): number {
  const spans = (axes: [AxisRef, [number, number]][]) =>
    axes.map(([, [lo, hi]]) => Math.abs(hi - lo)).filter((v) => Number.isFinite(v) && v > 0);
  const all: [AxisRef, [number, number]][] = [
    [view.xAxis, view.xRange],
    [view.yAxis, view.yRange],
    ...(isSpace(view) ? [[view.zAxis, view.zRange] as [AxisRef, [number, number]]] : []),
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
  view: PlotView,
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
  view: PlotView,
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
  /** The {@link StudioSnapshot.shapeId} it was asked for under. */
  shapeId: number;
  trajectories: TrajectoryMeta[];
  /** The view it was asked for, with the ranges as shown (equal scales on a flow's phase plane). */
  view: PlotView;
  /** Fixed points were asked for, so an empty list means there are none (not that the layer is hidden). */
  fixedPoints: boolean;
}

export interface PortraitState {
  res: PortraitResponse;
  meta: PortraitMeta;
  roundTripMs: number;
}

/**
 * The portrait can be drawn for `sys`: it is the portrait of `sys`, or of an
 * earlier compile with the same `shapeId` (same kind and states, not a
 * template load), which stays up until the portrait of `sys` arrives. Every
 * compile gets a new id, so keyed on the id alone the plot, the fixed-point
 * list and the legend would empty for a round trip after every edit, and the
 * legend's change of height would resize the plot and ask for two more
 * portraits.
 */
export function portraitFits(
  sys: CompileResponse | null,
  shapeId: number,
  portrait: PortraitState | null,
): portrait is PortraitState {
  return !!sys && !!portrait && (portrait.meta.systemId === sys.systemId || portrait.meta.shapeId === shapeId);
}

/**
 * The fixed points the portrait of `sys` found, or null when it searched
 * none, so an empty list means "none in view": the portrait does not fit
 * `sys` (see {@link portraitFits}) or was asked for with the layer off, or
 * the field changes with time (the core only searches autonomous systems).
 */
export function searchedFixedPoints(
  sys: CompileResponse | null,
  portrait: PortraitState | null,
  shapeId: number,
): FixedPointOut[] | null {
  if (!sys || !sys.autonomous || !portraitFits(sys, shapeId, portrait) || !portrait.meta.fixedPoints) return null;
  return portrait.res.fixedPoints;
}

export const PALETTE_SIZE = 7;

export function leastUsedPaletteIndex(
  ics: readonly Pick<InitialCondition, "color">[],
  skip: readonly number[] = [],
): number {
  const count = new Array<number>(PALETTE_SIZE).fill(0);
  for (const ic of ics) {
    const c = ((ic.color % PALETTE_SIZE) + PALETTE_SIZE) % PALETTE_SIZE;
    if (Number.isInteger(c)) count[c]++;
  }
  let best = -1;
  for (let c = 0; c < PALETTE_SIZE; c++) {
    if (skip.includes(c)) continue;
    if (best < 0 || count[c] < count[best]) best = c;
  }
  return Math.max(best, 0);
}

/**
 * Palette slots a new cobweb orbit never gets: `traj-4` is black in the
 * projector palette (the exports' default), 1.09:1 against `plot-fp`, the
 * ink of the graph of f, so its staircase and f would differ only by width.
 * (Checked against the tokens in cobweb.test.mjs.)
 */
export const COBWEB_SKIPPED_COLORS: readonly number[] = [3];

/** The palette slot for a new orbit in `view`. */
export function newIcColor(ics: readonly Pick<InitialCondition, "color">[], view: Pick<View, "kind">): number {
  return leastUsedPaletteIndex(ics, isCobweb(view) ? COBWEB_SKIPPED_COLORS : []);
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

/** Text mode: a space only matters between two word characters (`a b` is not `ab`). */
const canonicalTextLine = (text: string) =>
  text
    .trim()
    .replace(/\s+/g, " ")
    .replace(/(?<![A-Za-z0-9_]) | (?![A-Za-z0-9_])/g, "");

/**
 * The same equations, up to spelling that cannot change their meaning:
 * blank lines and spacing, and in LaTeX also `\left`/`\right` and braces
 * around a single-character sub- or superscript.
 */
export function sameEquations(a: readonly string[], b: readonly string[], format: InputFormat): boolean {
  const canon = format === "text" ? canonicalTextLine : canonicalLine;
  const norm = (lines: readonly string[]) => lines.map(canon).filter((l) => l !== "");
  const x = norm(a);
  const y = norm(b);
  return x.length === y.length && x.every((l, i) => l === y[i]);
}

/** A template's equations in `format`: as authored (LaTeX), or the generated text. */
export function templateLines(t: Pick<Template, "equations" | "text">, format: InputFormat): string[] {
  return format === "text" ? t.text : t.equations;
}

export function systemTitle(
  template: Template | undefined,
  equations: readonly string[],
  format: InputFormat,
): { name: string; edited: boolean } {
  if (!template) return { name: UNTITLED_SYSTEM, edited: false };
  return { name: template.title, edited: !sameEquations(templateLines(template, format), equations, format) };
}

/** The lines before and after the last input-mode switch, for switching straight back. */
export interface ModeSnapshot {
  /** The format before the switch. */
  from: InputFormat;
  before: string[];
  after: string[];
}

export type ModeSwitchPlan = { restore: string[] } | { convert: string[] };

/**
 * What switching the input mode to `to` does with `lines`. Straight back to
 * the format of the last switch with no line changed since, it restores the
 * lines exactly as they were before (so LaTeX → text → LaTeX gives back the
 * user's own spelling); otherwise the core converts them.
 */
export function planModeSwitch(
  lines: readonly string[],
  snapshot: ModeSnapshot | null,
  to: InputFormat,
): ModeSwitchPlan {
  const unchanged =
    snapshot !== null &&
    snapshot.from === to &&
    snapshot.after.length === lines.length &&
    snapshot.before.length === lines.length &&
    lines.every((l, i) => l === snapshot.after[i]);
  return unchanged ? { restore: snapshot.before.slice() } : { convert: lines.slice() };
}

/** What the core's conversion of every line does to a mode switch. */
export type ConversionOutcome =
  /** Switch, with these lines; a note (by line index) for each line kept as typed. */
  | { switch: true; lines: string[]; notes: (string | undefined)[] }
  /** Stay in the current format with the lines unchanged; a note on each line that stopped it. */
  | { switch: false; notes: (string | undefined)[] };

/**
 * Switch only if every line was converted or was kept because it has an
 * error (it was broken before the switch). A valid line the target format
 * can't write exactly is kept as typed by the core, and read in the other
 * format it would mean something else (text `1e-40` is `1·e - 40` in LaTeX)
 * or nothing, so the whole switch is called off.
 */
export function conversionOutcome(res: ConvertResponse): ConversionOutcome {
  const blocked = res.lines.map((l) => !l.converted && !l.error);
  if (blocked.some(Boolean)) {
    return {
      switch: false,
      notes: res.lines.map((l, i) =>
        blocked[i] ? `${l.note ?? "This line can't be converted exactly"}, so the input mode was not switched` : undefined,
      ),
    };
  }
  return { switch: true, lines: res.lines.map((l) => l.source), notes: res.lines.map((l) => l.note) };
}

export const DEFAULT_LAYERS: Layers = {
  field: true,
  fieldGrid: 21,
  nullclines: true,
  fixedPoints: true,
  backward: true,
};

export const DEFAULT_SOLVER: SolverSettings = {
  method: DEFAULT_METHOD,
  rtol: 1e-6,
  atol: 1e-9,
  h: 0,
  maxSteps: 200_000,
  tEnd: 20,
  samples: 0,
  transient: 0,
};

export const DEFAULT_FIXED_STEP = 0.01;

export const TIME_AXIS: TimeAxis = { kind: "time" };
export const stateAxis = (index: number): StateAxis => ({ kind: "state", index });

/**
 * The time axis of a time series over the N iterates (or the time `tEnd`)
 * shown: `[T − 0.04N, T + N]`, with `T` a map's transient (its orbits are
 * returned for n in [T, T + N]; 0 for a flow).
 */
export function seriesRange(
  solver: Pick<SolverSettings, "tEnd" | "transient">,
  kind: SystemKind | undefined,
): [number, number] {
  const n = solver.tEnd;
  const t = kind === "map" ? clampTransient(solver.transient ?? 0, n) : 0;
  return [t - 0.04 * n, t + n];
}

/** `v` with the range of its time axes moved by `dt` (the transient changed by `dt`); `v` itself when it has none. */
export function shiftTimeAxes(v: View, dt: number): View {
  if (dt === 0 || isCobweb(v) || isAnalysis(v)) return v;
  const shift = (r: [number, number]): [number, number] => [r[0] + dt, r[1] + dt];
  const x = v.xAxis.kind === "time";
  const y = v.yAxis.kind === "time";
  if (!x && !y) return v;
  return { ...v, xRange: x ? shift(v.xRange) : v.xRange, yRange: y ? shift(v.yRange) : v.yRange };
}

/**
 * The view a system opens in. With `solver`, a 1-D map's time series starts
 * at its transient T (its orbits begin at n = T).
 */
export function defaultView(sys: CompileResponse, solver?: Pick<SolverSettings, "tEnd" | "transient">): PlotView {
  const dim = sys.states.length;
  // Strogatz ch. 10's first picture: a 1-D map opens as its cobweb.
  if (cobwebAvailability(sys).ok) return cobwebView([-0.1, 1.1]);
  if (dim <= 1) {
    const map = sys.kind === "map";
    const t = map && solver ? clampTransient(solver.transient ?? 0, solver.tEnd) : 0;
    return {
      xAxis: TIME_AXIS,
      yAxis: stateAxis(0),
      xRange: map ? [t, t + 50] : [0, 10],
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

export function templateView(t: Template, sys: CompileResponse): PlotView {
  if (t.view.kind === "space") {
    const axes = [t.view.x, t.view.y, t.view.z ?? ""].map((n) => resolveAxis(n, sys.states));
    const idx = axes.map((a) => (a?.kind === "state" ? a.index : -1));
    const ok = spaceAvailability(sys).ok && idx.every((i) => i >= 0) && new Set(idx).size === 3 && !!t.view.zRange;
    if (!ok) return defaultView(sys);
    const ranges = [t.view.xRange, t.view.yRange, t.view.zRange!];
    return spaceView(idx as [number, number, number], (i) => [...ranges[idx.indexOf(i)]] as [number, number]);
  }
  if (t.view.kind === "section") {
    const section = templateSection(t, sys);
    const xAxis = resolveAxis(t.view.x, sys.states);
    const yAxis = resolveAxis(t.view.y, sys.states);
    if (!section || xAxis?.kind !== "state" || yAxis?.kind !== "state") return defaultView(sys);
    return { kind: "section", section, xAxis, yAxis, xRange: [...t.view.xRange], yRange: [...t.view.yRange] };
  }
  if (t.view.kind === "cobweb") {
    const axis = resolveAxis(t.view.x, sys.states);
    if (!cobwebAvailability(sys).ok || axis?.kind !== "state" || axis.index !== 0) return defaultView(sys);
    return { ...cobwebView(t.view.xRange), yRange: [...t.view.yRange] };
  }
  const xAxis = resolveAxis(t.view.x, sys.states);
  const yAxis = resolveAxis(t.view.y, sys.states);
  if (!xAxis || !yAxis) return defaultView(sys);
  return { xAxis, yAxis, xRange: [...t.view.xRange], yRange: [...t.view.yRange] };
}

function templateSection(t: Template, sys: CompileResponse): SectionSpec | null {
  const s = t.view.section;
  if (!s || !sectionAvailability(sys).ok) return null;
  if (s.kind === "stroboscopic") return { kind: "stroboscopic", period: s.period, phase: s.phase };
  const axis = resolveAxis(s.state, sys.states);
  if (axis?.kind !== "state" || !planeSectionAvailability(sys).ok) return null;
  return { kind: "plane", state: axis.index, value: s.value, direction: s.direction };
}

export function defaultParamRange(v: number): [number, number] {
  if (v === 0) return [-1, 1];
  const span = niceCeil(Math.abs(v) * 2);
  return v > 0 ? [0, span] : [-span, 0];
}

/**
 * A parameter after a compile: a template's value wins, then (only without
 * a template) the slider's previous value, then the default. A slider still
 * at the value the app gave it (the previous default `oldDefault`, or the
 * value of the template the system came from, `source`) was never moved, so
 * it follows an edited default line such as `a = 3`. The range follows the
 * same order.
 */
export function resolveParam(
  p: CompileResponse["params"][number],
  prev: ParamState | undefined,
  oldDefault: number | undefined,
  template: Template | undefined,
  source: Template | undefined,
): ParamState {
  const kept = template ? undefined : prev;
  const neverMoved = kept !== undefined && (kept.value === oldDefault || kept.value === source?.params[p.name]);
  const followsDefault = neverMoved && p.default !== oldDefault;
  const value = template?.params[p.name] ?? (followsDefault ? undefined : kept?.value) ?? p.default;
  const range = template?.paramRanges?.[p.name] ?? (kept ? [kept.min, kept.max] : defaultParamRange(value));
  return {
    name: p.name,
    latex: p.latex,
    value,
    min: Math.min(range[0], value),
    max: Math.max(range[1], value),
  };
}

const NICE_MANTISSAS = [1, 2, 2.5, 5, 10];

export function niceCeil(v: number): number {
  if (v <= 0) return 1;
  const p = Math.pow(10, Math.floor(Math.log10(v)));
  for (const m of NICE_MANTISSAS) if (m * p >= v - 1e-12) return m * p;
  return 10 * p;
}

/** Start points after a recompile that changed the states: each coordinate follows its variable as {@link adaptBaseState}. */
export function adaptInitialConditions(
  ics: InitialCondition[],
  oldStates: StateInfo[],
  newStates: StateInfo[],
): InitialCondition[] {
  return ics.map((ic) => ({ ...ic, x0: adaptBaseState(ic.x0, oldStates, newStates) }));
}

/** Slot `i` of `newStates` holds a renamed variable: a new name where the old name is gone. */
function renamedSlot(i: number, oldStates: StateInfo[], newStates: StateInfo[]): boolean {
  const before = oldStates[i]?.name;
  const after = newStates[i]?.name;
  return (
    before !== undefined &&
    after !== undefined &&
    !newStates.some((s) => s.name === before) &&
    !oldStates.some((s) => s.name === after)
  );
}

/**
 * The view after a recompile that changed the states. Each state axis
 * follows its variable by name, so adding or removing another equation
 * does not put a different variable on it; a renamed variable keeps its
 * axis. Null when an axis lost its variable or both axes would show the
 * same state.
 */
export function adaptView(
  view: View,
  oldStates: StateInfo[],
  newStates: StateInfo[],
  next?: Pick<CompileResponse, "kind" | "autonomous">,
): View | null {
  if (isLyapunov(view)) return view;
  if (isCurves(view)) {
    // The seeds are states of the old system; the view a project file saves follows like any other.
    const home = adaptView(view.home, oldStates, newStates, next);
    if (!home || isCurves(home)) return null;
    return { ...view, branch: isContinuation(home) ? home : null, home, curves: { ...view.curves, seeds: [] } };
  }
  if (isContinuation(view)) {
    const y = view.yAxis;
    const k = y.kind === "state" ? newStates.findIndex((s) => s.name === oldStates[y.index]?.name) : 0;
    if (k < 0) return null;
    const continuation = { ...view.continuation, seeds: [] };
    return { ...view, yAxis: y.kind === "state" ? stateAxis(k) : y, continuation };
  }
  if (isOrbit(view)) {
    const k = newStates.findIndex((s) => s.name === oldStates[view.yAxis.index]?.name);
    const index = k >= 0 ? k : renamedSlot(view.yAxis.index, oldStates, newStates) ? view.yAxis.index : -1;
    return index >= 0 && next?.kind !== "ode" ? { ...view, yAxis: stateAxis(index) } : null;
  }
  if (isCobweb(view)) {
    // A cobweb stays while the system is still a one-dimensional autonomous map.
    const ok = newStates.length === 1 && (!next || cobwebAvailability({ ...next, states: newStates }).ok);
    return ok ? { ...view, xAxis: stateAxis(0), yAxis: stateAxis(0) } : null;
  }
  const followState = (a: StateAxis): StateAxis | null => {
    const k = newStates.findIndex((s) => s.name === oldStates[a.index]?.name);
    if (k >= 0) return stateAxis(k);
    return renamedSlot(a.index, oldStates, newStates) ? a : null;
  };
  const follow = (a: AxisRef): AxisRef | null => (a.kind === "time" ? a : followState(a));
  if (isSpace(view)) {
    const [xAxis, yAxis, zAxis] = [view.xAxis, view.yAxis, view.zAxis].map(followState);
    if (!xAxis || !yAxis || !zAxis || newStates.length < 3 || (next && next.kind === "map")) return null;
    return { ...view, xAxis, yAxis, zAxis };
  }
  const xAxis = follow(view.xAxis);
  const yAxis = follow(view.yAxis);
  if (!xAxis || !yAxis) return null;
  if (isSection(view)) {
    const s = view.section;
    if (s.kind !== "plane") return { ...view, xAxis, yAxis };
    const held = follow(stateAxis(s.state));
    if (held?.kind !== "state" || newStates.length < 2) return null;
    return { ...view, xAxis, yAxis, section: { ...s, state: held.index } };
  }
  if (xAxis.kind === "state" && yAxis.kind === "state" && xAxis.index === yAxis.index) return null;
  return { ...view, xAxis, yAxis };
}

/** Where parameter `i` went after a recompile: by name, else by position when the count is unchanged; -1 when gone. */
export function followParamIndex(
  oldParams: readonly Pick<ParamInfo, "name">[],
  newParams: readonly Pick<ParamInfo, "name">[],
  i: number,
): number {
  const k = newParams.findIndex((p) => p.name === oldParams[i]?.name);
  return k >= 0 ? k : oldParams.length === newParams.length && i < newParams.length ? i : -1;
}

export function followOrbitParam<V extends OrbitView | ContinuationView>(
  view: V,
  oldParams: readonly Pick<ParamInfo, "name">[],
  newParams: readonly Pick<ParamInfo, "name">[],
): V | null {
  const index = followParamIndex(oldParams, newParams, view.param);
  if (index < 0) return null;
  return index === view.param ? view : { ...view, param: index };
}

/** Slice values after a recompile that changed the states: by name, as {@link adaptView}; a new variable starts at 0. */
export function adaptBaseState(base: readonly number[], oldStates: StateInfo[], newStates: StateInfo[]): number[] {
  return newStates.map((s, i) => {
    const k = oldStates.findIndex((o) => o.name === s.name);
    if (k >= 0) return base[k] ?? 0;
    return renamedSlot(i, oldStates, newStates) ? (base[i] ?? 0) : 0;
  });
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
  if (isSection(view)) return phasePlaneSpans(ic, solver.tEnd, false);
  if (isSpace(view)) return phasePlaneSpans(ic, solver.tEnd, layers.backward);
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
  /**
   * Changes when a template is loaded or a compile changes the system's kind
   * or states; recompiles that keep them share it (see {@link portraitFits}).
   */
  shapeId: number;
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

/**
 * The view is drawn at equal scales: a flow's phase plane, and a cobweb (so
 * the diagonal is at 45° and f's slope can be compared with it by eye).
 */
export function hasEqualScales(view: View, kind: SystemKind | undefined): boolean {
  if (isSpace(view) || isParameterView(view)) return false;
  return isCobweb(view) || isFlowPhasePlane(view.xAxis, view.yAxis, kind);
}

export function displayRanges(
  view: View,
  kind: CompileResponse["kind"] | undefined,
  aspect: number | null,
): { xRange: [number, number]; yRange: [number, number] } {
  return hasEqualScales(view, kind)
    ? equalizeRanges(view.xRange, view.yRange, aspect)
    : { xRange: view.xRange, yRange: view.yRange };
}

/**
 * Flows have a nullcline layer, in every view (with time on an axis it shows
 * where each plotted state's rate vanishes on the slice); maps have none.
 * The toolbar shows the Nullclines toggle exactly when the layer can be
 * requested.
 */
export function hasNullclineLayer(kind: SystemKind | undefined): boolean {
  return kind !== "map";
}

export function buildPortraitJob(s: StudioSnapshot): PortraitJob<PortraitMeta> | null {
  const sys = s.compiled;
  if (!sys || !sys.ok || sys.systemId === undefined || isAnalysis(s.view)) return null;
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
  const fixedStep = !METHODS[s.solver.method].adaptive;
  const shown = displayRanges(s.view, sys.kind, s.plotAspect);
  const cobweb = isCobweb(s.view);
  const section = isSection(s.view) ? s.view.section : null;
  const space = isSpace(s.view) ? s.view : null;
  const transient = clampTransient(s.solver.transient, s.solver.tEnd);
  // A map hides the same transient in every view; a cobweb also asks for f's graph.
  const extra: Pick<PortraitRequest, "graphSamples" | "transient" | "section" | "zAxis" | "zRange"> = !isOde
    ? { ...(cobweb ? { graphSamples: COBWEB_GRAPH_SAMPLES } : {}), transient }
    : section
      ? { section, transient }
      : space
        ? { zAxis: space.zAxis, zRange: space.zRange }
        : {};
  const fixedPoints = s.layers.fixedPoints && !section;
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
      fieldGrid: isOde && s.layers.field && !cobweb && !section && !space ? s.layers.fieldGrid : 0,
      nullclines: hasNullclineLayer(sys.kind) && s.layers.nullclines && !cobweb && !section && !space,
      fixedPoints,
      trajectories,
      solver: {
        method: s.solver.method,
        rtol: s.solver.rtol,
        atol: s.solver.atol,
        h: fixedStep && s.solver.h <= 0 ? DEFAULT_FIXED_STEP : s.solver.h,
        maxSteps: s.solver.maxSteps,
      },
      samples: s.solver.samples > 0 ? s.solver.samples : autoSamples(maxSpan),
      ...extra,
    },
    meta: {
      systemId: sys.systemId,
      shapeId: s.shapeId,
      trajectories: metas,
      // The visible spans, as the selection's match tolerance is a fraction of them.
      view: { ...s.view, xRange: shown.xRange, yRange: shown.yRange },
      fixedPoints,
    },
  };
}

/** A transient as the core takes it: a whole number, at least 0, with the N iterates after it within {@link MAX_ITERATES}. */
export function clampTransient(t: number, tEnd: number): number {
  const n = Math.max(1, Math.round(tEnd));
  if (!Number.isFinite(t)) return 0;
  return Math.min(Math.max(0, Math.round(t)), Math.max(0, MAX_ITERATES - n));
}

/**
 * The start point a click at plot point (px, py) gives (from `base`, the
 * point being dragged, else the slice): each axis's state takes the
 * pointer's value. A cobweb's start point is x_0 on the diagonal, so only
 * the horizontal value counts. A map always starts at n = 0. A section's
 * start point is on the section: the plane's state is its value, and a
 * stroboscopic start is at t_0 = the phase. (A return map has no click to
 * add: see {@link canAddOnPlot}.)
 */
export function liftPlotPoint(
  sys: Pick<CompileResponse, "kind" | "states"> | null,
  view: PlotView,
  baseState: readonly number[],
  fieldTime: number,
  px: number,
  py: number,
  base?: Pick<InitialCondition, "x0" | "t0">,
): { x0: number[]; t0: number } {
  const dim = sys?.states.length ?? 0;
  const isMap = sys?.kind === "map";
  const x0 = Array.from({ length: dim }, (_, i) => base?.x0[i] ?? baseState[i] ?? 0);
  let t0 = base?.t0 ?? (isMap ? 0 : fieldTime);
  const put = (a: AxisRef, v: number) => {
    if (a.kind === "time") t0 = isMap ? 0 : v;
    else x0[a.index] = v;
  };
  put(view.xAxis, px);
  if (!isCobweb(view)) put(view.yAxis, py);
  if (isSection(view)) {
    const s = view.section;
    if (s.kind === "plane") x0[s.state] = s.value;
    else t0 = s.phase;
  }
  return { x0, t0 };
}

export const canAddOnPlot = (view: View): boolean => !isReturnMap(view) && !isSpace(view) && !isAnalysis(view);

/** The last view of each kind, for switching back (useStudio keeps it for the session). */
export type ViewMemory = Partial<Record<ViewChoice, View>>;

/**
 * The view the View control's `choice` shows, or null when it can't (the
 * system can't show it, or it is shown already). The view being left is
 * remembered in `memory`, and the remembered view of `choice` comes back;
 * without one it is made from what is shown now: a cobweb over the state's
 * range now (else [−0.1, 1.1]), a time series over n (or t) in [T − 0.04N, T + N]
 * with the state's range, a phase plane of states 0 and 1 over their ranges
 * (else their extents in the portrait, as `setAxis` does), a 3D view of
 * states 0, 1 and 2 over their extents in the portrait, seen from
 * {@link DEFAULT_CAMERA}.
 */
export function switchView(
  choice: ViewChoice,
  s: Pick<StudioSnapshot, "compiled" | "view" | "solver"> & {
    portrait?: PortraitState | null;
    params?: readonly (Pick<ParamState, "min" | "max"> & Partial<Pick<ParamState, "value">>)[];
    selection?: Selection;
    shapeId?: number;
    branches?: Readonly<Record<string, BranchRun>>;
  },
  memory: ViewMemory,
): View | null {
  const sys = s.compiled;
  if (!sys || !sys.ok) return null;
  const v = s.view;
  const from = viewChoice(v);
  if (choice === from) return null;
  if (!viewChoiceAvailability(sys)[choice].ok) return null;
  memory[from] = v;
  if (choice === "curves") {
    const kept = memory.curves && isCurves(memory.curves) ? memory.curves : null;
    const branch = memory.continuation && isContinuation(memory.continuation) ? memory.continuation : null;
    const home = branch ?? defaultView(sys, s.solver);
    const params = s.params ?? [];
    if (kept?.curves.seeds.length) return { ...(kept.branch === branch ? kept : withBranch(kept, branch, params)), home };
    return openPlane(kept, branch, home, params, branchSeeds(sys, params, branch, s.branches ?? {}));
  }
  const kept = memory[choice];
  if (kept) return kept;
  const dim = sys.states.length;
  const shownRange = (i: number): [number, number] | null => {
    if (v.xAxis.kind === "state" && v.xAxis.index === i) return v.xRange;
    if (v.yAxis.kind === "state" && v.yAxis.index === i) return v.yRange;
    if (isSpace(v) && v.zAxis.kind === "state" && v.zAxis.index === i) return v.zRange;
    return null;
  };
  const fromExtent = (i: number): [number, number] => {
    const ext = stateExtent(s.portrait?.res, dim, i);
    if (!ext) return [-5, 5];
    const pad = Math.max(0.1 * (ext[1] - ext[0]), 0.5);
    return [ext[0] - pad, ext[1] + pad];
  };
  if (choice === "cobweb") return cobwebView(shownRange(0) ?? [-0.1, 1.1]);
  const rangeOf = (i: number) => shownRange(i) ?? fromExtent(i);
  if (choice === "section") {
    const r0 = rangeOf(0);
    return sectionView(sys, defaultSection(sys, (r0[0] + r0[1]) / 2), rangeOf);
  }
  if (choice === "space") return spaceView([0, 1, 2], fromExtent);
  if (choice === "lyapunov") return lyapunovView(sys.kind ?? "ode");
  if (choice === "orbit") return orbitView(s.params ?? [], shownRange(0) ?? fromExtent(0));
  if (choice === "continuation") {
    const value = s.params?.[0]?.value;
    const fps =
      s.selection?.kind === "fp"
        ? [s.selection.x]
        : (searchedFixedPoints(sys, s.portrait ?? null, s.shapeId ?? -1) ?? []).map((fp) => fp.x);
    const seeds = value === undefined ? [] : fps.map((x) => pointSeed(x, value, "both"));
    return continuationView(s.params ?? [], shownRange(0) ?? fromExtent(0), 0, seeds);
  }
  if (choice === "series") {
    const y = shownRange(0) ?? fromExtent(0);
    return { xAxis: TIME_AXIS, yAxis: stateAxis(0), xRange: seriesRange(s.solver, sys.kind), yRange: [y[0], y[1]] };
  }
  const x = shownRange(0) ?? fromExtent(0);
  const y = shownRange(1) ?? fromExtent(1);
  return { xAxis: stateAxis(0), yAxis: stateAxis(1), xRange: [x[0], x[1]], yRange: [y[0], y[1]] };
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
    // A response for a system with another number of states (the portrait
    // of the previous compile) is laid out with another stride.
    if (tr.x.length !== tr.t.length * dim) continue;
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

/** What a project load sets once its system has compiled (see {@link applyProjectLoad}). */
export interface AppliedProject {
  params: ParamState[];
  ics: InitialCondition[];
  view: View;
  baseState: number[];
  fieldTime: number;
  layers: Layers;
  solver: SolverSettings;
  /** Sentences for the open banner, about what could not be used as saved. */
  warnings: string[];
}

/** The state a saved axis names in `sys`, or null when it names none (another kind, or a variable it lacks). */
function loadedAxis(a: LoadedAxis, sys: CompileResponse): AxisRef | null {
  if (a.kind === "time") return TIME_AXIS;
  if (a.kind !== "state") return null;
  const i = sys.states.findIndex((s) => s.name === a.name);
  return i >= 0 ? stateAxis(i) : null;
}

/** The notice when a saved cobweb view (or a view of a kind this version lacks) can't be shown. */
export const COBWEB_VIEW_WARNING = "The saved cobweb view doesn't fit this system, so the default view is shown.";

/**
 * A saved view that is not a plane can be shown: a cobweb whose x axis is
 * state s and y axis the next iterate of s, of a system that is a 1-D
 * autonomous map. Null for a plane view; false for any other kind, or for a
 * view with an x_{n+1} axis that isn't such a cobweb (as `project_read`
 * judges it, so both say the same sentence).
 */
function savedCobwebFits(load: ProjectLoad, sys: CompileResponse): boolean | null {
  const v = load.view;
  if (!v || v.kind === "section" || v.kind === "space" || v.kind === "lyapunov" || v.kind === "orbit" || v.kind === "continuation")
    return null;
  const next = v.xAxis.kind === "next" || v.yAxis.kind === "next";
  if ((v.kind === undefined || v.kind === "plane") && !next) return null;
  if (v.kind !== "cobweb" || !v.xRange || !v.yRange) return false;
  const x = v.xAxis;
  const y = v.yAxis;
  return (
    x.kind === "state" &&
    y.kind === "next" &&
    x.name === y.name &&
    sys.states[0]?.name === x.name &&
    cobwebAvailability(sys).ok
  );
}

const VIEW_WARNING = "The saved view doesn't fit this system, so the default view is shown.";
const DIRECTIONS: readonly string[] = ["rising", "falling", "both"] satisfies Direction[];

function loadSectionView(v: LoadedView, sys: CompileResponse): SectionView | string {
  const s = v.section;
  if (!s) return SECTION_MISFIT;
  if (s.kind !== "plane" && s.kind !== "stroboscopic")
    return `The section of the view “${v.id}” is of a kind (“${s.kind}”) this version doesn't know, so the default view is used.`;
  const index = (name: string | undefined) => sys.states.findIndex((st) => st.name === name);
  let section: SectionSpec;
  if (s.kind === "plane") {
    const state = index(s.state);
    if (s.state !== undefined && state < 0)
      return `The section of the view “${v.id}” holds the state ${s.state} fixed, which the system doesn't have, so the default view is used.`;
    if (state < 0 || s.value === undefined || !s.direction || !DIRECTIONS.includes(s.direction)) return SECTION_MISFIT;
    section = { kind: "plane", state, value: s.value, direction: s.direction as Direction };
  } else {
    if (s.period === undefined || !(s.period > 0)) return SECTION_MISFIT;
    section = { kind: "stroboscopic", period: s.period, phase: s.phase ?? 0 };
  }
  if (!sectionAvailability(sys).ok || (section.kind === "plane" && !planeSectionAvailability(sys).ok))
    return SECTION_MISFIT;
  const { xAxis: x, yAxis: y } = v as { xAxis: { kind: string; name?: string }; yAxis: { kind: string; name?: string } };
  const pair = x.kind === "state" && (y.kind === "state" ? y.name !== x.name : y.kind === "next" && y.name === x.name);
  if (!pair) return SECTION_MISFIT;
  const xi = index(x.name);
  const yi = index(y.name);
  if (xi < 0 || yi < 0 || !v.xRange || !v.yRange) return VIEW_WARNING;
  return {
    kind: "section",
    section,
    xAxis: stateAxis(xi),
    yAxis: stateAxis(yi),
    xRange: [v.xRange[0], v.xRange[1]],
    yRange: [v.yRange[0], v.yRange[1]],
  };
}


function loadSpaceView(v: LoadedView, sys: CompileResponse): SpaceView | string {
  const axes = [v.xAxis, v.yAxis, v.zAxis];
  const names = axes.map((a) => (a?.kind === "state" ? a.name : undefined));
  if (!spaceAvailability(sys).ok || names.some((n) => n === undefined) || new Set(names).size !== 3 || !v.zAxis)
    return SPACE_MISFIT;
  const idx = names.map((n) => sys.states.findIndex((st) => st.name === n));
  const ranges = [v.xRange, v.yRange, v.zRange];
  if (idx.some((i) => i < 0) || ranges.some((r) => !r)) return VIEW_WARNING;
  return spaceView(idx as [number, number, number], (i) => ranges[idx.indexOf(i)]!, clampCamera(v.camera ?? {}));
}

function loadLyapunovView(v: LoadedView, sys: CompileResponse): LyapunovView | string {
  const l = v.lyapunov;
  const kind = sys.kind ?? "ode";
  const settings: LyapunovSettings | null =
    l && [l.transient, l.total, l.interval, l.blocks].every((x) => x !== undefined)
      ? { transient: l.transient!, total: l.total!, interval: l.interval!, blocks: l.blocks! }
      : null;
  if (!settings || lyapunovSettingsError(settings, kind) || v.xAxis.kind !== "time" || v.yAxis.kind !== "exponent")
    return LYAPUNOV_MISFIT;
  if (!v.xRange || !v.yRange) return VIEW_WARNING;
  return { ...lyapunovView(kind, settings), xRange: [v.xRange[0], v.xRange[1]], yRange: [v.yRange[0], v.yRange[1]] };
}

function loadOrbitView(v: LoadedView, sys: CompileResponse): OrbitView | string {
  const o = v.orbit;
  const settings: OrbitSettings | null =
    o && o.transient !== undefined && o.kept !== undefined ? { transient: o.transient, kept: o.kept } : null;
  const x = v.xAxis;
  const y = v.yAxis;
  const param = x.kind === "param" ? sys.params.findIndex((p) => p.name === x.name) : -1;
  const state = y.kind === "state" ? sys.states.findIndex((st) => st.name === y.name) : -1;
  if (!orbitAvailability(sys).ok || !settings || orbitSettingsError(settings) || param < 0 || state < 0) return ORBIT_MISFIT;
  if (!v.xRange || !v.yRange) return VIEW_WARNING;
  return { ...orbitView([], v.yRange, param, state, settings), xRange: [v.xRange[0], v.xRange[1]] };
}

export const CONTINUATION_MISFIT = "The saved continuation can't be run as saved, so the default view is shown.";
export const continuationSeedMisfit = (n: number) =>
  `The saved continuation's branch ${n} doesn't fit this system, so it was left out.`;

const CONTINUATION_DIRECTIONS: readonly string[] = ["forward", "backward", "both"] satisfies ContinuationDirection[];

function loadedSeed(seed: LoadedSeed, sys: CompileResponse): ContinuationSeed | null {
  const dim = sys.states.length;
  const { x, param, direction } = seed;
  const period = seed.period ?? 1;
  if (!x || x.length !== dim || param === undefined || !direction || !CONTINUATION_DIRECTIONS.includes(direction)) return null;
  if (!(Number.isInteger(period) && period >= 1)) return null;
  const dir = direction as ContinuationDirection;
  if (seed.from === "point") return { from: "point", x, param, period, direction: dir };
  const kind = seed.kind as SpecialKindOut | undefined;
  if (seed.from !== "switch" || !kind || !SPECIAL_KINDS[kind]?.switches || seed.tangent?.length !== dim + 1) return null;
  return { from: "switch", at: { kind, p: param, x, period, tangent: seed.tangent }, direction: dir };
}

function loadContinuationView(v: LoadedView, sys: CompileResponse): { view: ContinuationView; warnings: string[] } | string {
  const c = v.continuation;
  const x = v.xAxis;
  const y = v.yAxis;
  const param = x.kind === "param" ? sys.params.findIndex((p) => p.name === x.name) : -1;
  const state = y.kind === "state" ? sys.states.findIndex((st) => st.name === y.name) : -1;
  const settings =
    c?.range && c.ds !== undefined && c.maxPoints !== undefined ? { range: c.range, ds: c.ds, maxPoints: c.maxPoints } : null;
  if (!continuationAvailability(sys).ok || !settings || continuationSettingsError(settings) || param < 0) return CONTINUATION_MISFIT;
  if ((y.kind !== "norm" && state < 0) || !v.xRange || !v.yRange) return CONTINUATION_MISFIT;
  const warnings: string[] = [];
  const seeds = (c?.seeds ?? []).flatMap((raw, i) => {
    const seed = loadedSeed(raw, sys);
    if (!seed) warnings.push(continuationSeedMisfit(i + 1));
    return seed ? [seed] : [];
  });
  const view: ContinuationView = {
    kind: "continuation",
    xAxis: TIME_AXIS,
    yAxis: y.kind === "norm" ? { kind: "norm" } : stateAxis(state),
    xRange: [v.xRange[0], v.xRange[1]],
    yRange: [v.yRange[0], v.yRange[1]],
    param,
    continuation: { ...settings, range: [settings.range[0], settings.range[1]], seeds },
  };
  return { view, warnings };
}

/** The view a project saved for `sys`, or null when it does not fit (an axis it can't show, a bad range). */
export function projectView(load: ProjectLoad, sys: CompileResponse): View | null {
  const v = load.view;
  if (v?.kind === "continuation") {
    const fit = loadContinuationView(v, sys);
    return typeof fit === "string" ? null : fit.view;
  }
  if (v?.kind === "lyapunov") {
    const fit = loadLyapunovView(v, sys);
    return typeof fit === "string" ? null : fit;
  }
  if (v?.kind === "orbit") {
    const fit = loadOrbitView(v, sys);
    return typeof fit === "string" ? null : fit;
  }
  if (v?.kind === "section") {
    const fit = loadSectionView(v, sys);
    return typeof fit === "string" ? null : fit;
  }
  if (v?.kind === "space") {
    const fit = loadSpaceView(v, sys);
    return typeof fit === "string" ? null : fit;
  }
  if (!v || !v.xRange || !v.yRange) return null;
  const cobweb = savedCobwebFits(load, sys);
  if (cobweb === false) return null;
  if (cobweb) return { ...cobwebView(v.xRange), yRange: [v.yRange[0], v.yRange[1]] };
  const xAxis = loadedAxis(v.xAxis, sys);
  const yAxis = loadedAxis(v.yAxis, sys);
  if (!xAxis || !yAxis) return null;
  const same = xAxis.kind === "time" ? yAxis.kind === "time" : yAxis.kind === "state" && yAxis.index === xAxis.index;
  if (same) return null;
  return { xAxis, yAxis, xRange: [v.xRange[0], v.xRange[1]], yRange: [v.yRange[0], v.yRange[1]] };
}

/**
 * The studio state a project file describes, for the system its lines
 * compiled to. Everything is matched by name: a parameter by its id, a
 * start point's coordinates through the file's `system.states` (as
 * {@link adaptInitialConditions} does after a recompile), slice values by
 * state name. A parameter the file lacks gets its default; a view that
 * doesn't fit the system (or has a bad range) gives way to
 * {@link defaultView}. `newIcId` numbers the start points.
 */
export function applyProjectLoad(load: ProjectLoad, sys: CompileResponse, newIcId: () => number): AppliedProject {
  const warnings: string[] = [];
  const params = sys.params.map((p) => {
    const saved = load.params.find((q) => q.name === p.name);
    if (!saved) return resolveParam(p, undefined, undefined, undefined, undefined);
    const value = saved.value;
    const ok = Number.isFinite(saved.min) && Number.isFinite(saved.max) && saved.min < saved.max;
    const [lo, hi] = ok ? [saved.min, saved.max] : defaultParamRange(value);
    return { name: p.name, latex: p.latex, value, min: Math.min(lo, value), max: Math.max(hi, value) };
  });

  // The file's states, in the order its x0 arrays were written; without
  // them (a hand-written file), the compiled order.
  const fileStates: StateInfo[] = (load.states.length ? load.states : sys.states.map((s) => s.name)).map((name) => ({
    name,
    latex: name,
    derived: false,
  }));
  const ics: InitialCondition[] = load.ics.map((ic) => ({
    id: newIcId(),
    uid: ic.uid,
    x0: adaptBaseState(ic.x0, fileStates, sys.states),
    t0: ic.t0,
    color: ((Math.round(ic.color) % PALETTE_SIZE) + PALETTE_SIZE) % PALETTE_SIZE,
    ...(ic.hidden ? { hidden: true } : {}),
  }));

  const solver: SolverSettings = { ...DEFAULT_SOLVER, ...(sys.kind === "map" ? { tEnd: 50 } : {}), ...load.solver };
  solver.transient = clampTransient(solver.transient, solver.tEnd);
  let view = projectView(load, sys);
  if (!view) {
    view = defaultView(sys, solver);
    if (load.view?.kind === "section") warnings.push(loadSectionView(load.view, sys) as string);
    else if (load.view?.kind === "space") warnings.push(loadSpaceView(load.view, sys) as string);
    else if (load.view?.kind === "lyapunov") warnings.push(loadLyapunovView(load.view, sys) as string);
    else if (load.view?.kind === "orbit") warnings.push(loadOrbitView(load.view, sys) as string);
    else if (load.view?.kind === "continuation") warnings.push(CONTINUATION_MISFIT);
    else if (load.view) warnings.push(savedCobwebFits(load, sys) === false ? COBWEB_VIEW_WARNING : VIEW_WARNING);
  }
  const fit = isContinuation(view) && load.view ? loadContinuationView(load.view, sys) : null;
  if (fit && typeof fit !== "string") warnings.push(...fit.warnings);
  const slice = load.view?.slice ?? {};
  const baseState = sys.states.map((s) => (Number.isFinite(slice[s.name]) ? slice[s.name] : 0));
  const t = load.view?.t;
  const fieldTime = t !== undefined && Number.isFinite(t) ? t : 0;
  const layers: Layers = { ...DEFAULT_LAYERS, ...load.view?.layers };
  return { params, ics, view, baseState, fieldTime, layers, solver, warnings };
}
