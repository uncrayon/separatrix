import type {
  AxisRef,
  CompileResponse,
  ConvertResponse,
  FixedPointOut,
  InputFormat,
  Method,
  PortraitRequest,
  PortraitResponse,
  StateInfo,
  SystemKind,
  TrajectoryRequest,
} from "../api/types";
import type { PortraitJob } from "../api/client";
import type { Template } from "../templates";
import type { LoadedAxis, ProjectLoad } from "../project/serialize";

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

/**
 * How a view draws its two axes. `plane` is the phase plane and the time
 * series (the axes tell them apart); `cobweb` is the cobweb plot of a 1-D
 * map x_{n+1} = f(x_n), whose two axes are both state 0 (x_n across, x_{n+1}
 * up) and whose ranges are kept at equal scales.
 */
export type ViewKind = "plane" | "cobweb";

export interface View {
  /** Absent = `plane`. */
  kind?: ViewKind;
  xAxis: AxisRef;
  yAxis: AxisRef;
  xRange: [number, number];
  yRange: [number, number];
}

export const viewKind = (v: Pick<View, "kind">): ViewKind => v.kind ?? "plane";
export const isCobweb = (v: Pick<View, "kind">): boolean => v.kind === "cobweb";

/** What the View control offers: the phase plane, the time series and the cobweb, in that order for every system. */
export type ViewChoice = "phase" | "series" | "cobweb";
export const VIEW_CHOICES: readonly ViewChoice[] = ["phase", "series", "cobweb"];

/** Which of the View control's options `v` is. */
export function viewChoice(v: View): ViewChoice {
  if (isCobweb(v)) return "cobweb";
  return v.xAxis.kind === "time" || v.yAxis.kind === "time" ? "series" : "phase";
}

export type Availability = { ok: true } | { ok: false; reason: string };

type SystemShape = Pick<CompileResponse, "kind" | "autonomous" | "states">;

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

export const VIEW_CHOICE_LABELS: Record<ViewChoice, string> = {
  phase: "Phase plane",
  series: "Time series",
  cobweb: "Cobweb",
};

/**
 * The View control's visible labels, short enough for three equal segments
 * of a 1,024 px window's inspector in projector mode; the full names stay
 * in each segment's title and accessible name.
 */
export const VIEW_CHOICE_SHORT: Record<ViewChoice, string> = {
  phase: "Phase",
  series: "Series",
  cobweb: "Cobweb",
};

/** Which of the View control's three options `sys` can show, with the reason for each it can't. */
export function viewChoiceAvailability(
  sys: SystemShape | null | undefined,
  name?: string,
): Record<ViewChoice, Availability> {
  return {
    phase: phasePlaneAvailability(sys, name),
    series: { ok: true },
    cobweb: cobwebAvailability(sys, name),
  };
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
export function cobwebView(range: [number, number]): View {
  return {
    kind: "cobweb",
    xAxis: stateAxis(0),
    yAxis: stateAxis(0),
    xRange: [range[0], range[1]],
    yRange: [range[0], range[1]],
  };
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
  /** The {@link StudioSnapshot.shapeId} it was asked for under. */
  shapeId: number;
  trajectories: TrajectoryMeta[];
  /** The view it was asked for, with the ranges as shown (equal scales on a flow's phase plane). */
  view: View;
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
  method: "dopri5",
  rtol: 1e-6,
  atol: 1e-9,
  h: 0,
  maxSteps: 200_000,
  tEnd: 20,
  samples: 0,
  transient: 0,
};

export const DEFAULT_FIXED_STEP = 0.01;

export const TIME_AXIS: AxisRef = { kind: "time" };
export const stateAxis = (index: number): AxisRef => ({ kind: "state", index });

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
  if (dt === 0 || isCobweb(v)) return v;
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
export function defaultView(sys: CompileResponse, solver?: Pick<SolverSettings, "tEnd" | "transient">): View {
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

export function templateView(t: Template, sys: CompileResponse): View {
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
  if (isCobweb(view)) {
    // A cobweb stays while the system is still a one-dimensional autonomous map.
    const ok = newStates.length === 1 && (!next || cobwebAvailability({ ...next, states: newStates }).ok);
    return ok ? { ...view, xAxis: stateAxis(0), yAxis: stateAxis(0) } : null;
  }
  const follow = (a: AxisRef): AxisRef | null => {
    if (a.kind === "time") return a;
    const k = newStates.findIndex((s) => s.name === oldStates[a.index]?.name);
    if (k >= 0) return stateAxis(k);
    return renamedSlot(a.index, oldStates, newStates) ? a : null;
  };
  const xAxis = follow(view.xAxis);
  const yAxis = follow(view.yAxis);
  if (!xAxis || !yAxis) return null;
  if (xAxis.kind === "state" && yAxis.kind === "state" && xAxis.index === yAxis.index) return null;
  return { ...view, xAxis, yAxis };
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
  const cobweb = isCobweb(s.view);
  // A map hides the same transient in every view; a cobweb also asks for f's graph.
  const mapFields: MapRequestFields = isOde
    ? {}
    : {
        ...(cobweb ? { graphSamples: COBWEB_GRAPH_SAMPLES } : {}),
        transient: clampTransient(s.solver.transient, s.solver.tEnd),
      };
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
      fieldGrid: isOde && s.layers.field && !cobweb ? s.layers.fieldGrid : 0,
      nullclines: hasNullclineLayer(sys.kind) && s.layers.nullclines && !cobweb,
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
      ...mapFields,
    },
    meta: {
      systemId: sys.systemId,
      shapeId: s.shapeId,
      trajectories: metas,
      // The visible spans, as the selection's match tolerance is a fraction of them.
      view: { ...s.view, xRange: shown.xRange, yRange: shown.yRange },
      fixedPoints: s.layers.fixedPoints,
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
 * the horizontal value counts. A map always starts at n = 0.
 */
export function liftPlotPoint(
  sys: Pick<CompileResponse, "kind" | "states"> | null,
  view: View,
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
  return { x0, t0 };
}

/** The last view of each kind, for switching back (useStudio keeps it for the session). */
export type ViewMemory = Partial<Record<ViewChoice, View>>;

/**
 * The view the View control's `choice` shows, or null when it can't (the
 * system can't show it, or it is shown already). The view being left is
 * remembered in `memory`, and the remembered view of `choice` comes back;
 * without one it is made from what is shown now: a cobweb over the state's
 * range now (else [−0.1, 1.1]), a time series over n (or t) in [T − 0.04N, T + N]
 * with the state's range, a phase plane of states 0 and 1 over their ranges
 * (else their extents in the portrait, as `setAxis` does).
 */
export function switchView(
  choice: ViewChoice,
  s: Pick<StudioSnapshot, "compiled" | "view" | "solver"> & { portrait?: PortraitState | null },
  memory: ViewMemory,
): View | null {
  const sys = s.compiled;
  if (!sys || !sys.ok) return null;
  const v = s.view;
  const from = viewChoice(v);
  if (choice === from) return null;
  if (!viewChoiceAvailability(sys)[choice].ok) return null;
  memory[from] = v;
  const kept = memory[choice];
  if (kept) return kept;
  const dim = sys.states.length;
  const shownRange = (i: number): [number, number] | null => {
    if (v.xAxis.kind === "state" && v.xAxis.index === i) return v.xRange;
    if (v.yAxis.kind === "state" && v.yAxis.index === i) return v.yRange;
    return null;
  };
  const fromExtent = (i: number): [number, number] => {
    const ext = stateExtent(s.portrait?.res, dim, i);
    if (!ext) return [-5, 5];
    const pad = Math.max(0.1 * (ext[1] - ext[0]), 0.5);
    return [ext[0] - pad, ext[1] + pad];
  };
  if (choice === "cobweb") return cobwebView(shownRange(0) ?? [-0.1, 1.1]);
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
  if (!v) return null;
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

/** The view a project saved for `sys`, or null when it does not fit (an axis it can't show, a bad range). */
export function projectView(load: ProjectLoad, sys: CompileResponse): View | null {
  const v = load.view;
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
    if (load.view)
      warnings.push(
        savedCobwebFits(load, sys) === false
          ? COBWEB_VIEW_WARNING
          : "The saved view doesn't fit this system, so the default view is shown.",
      );
  }
  const slice = load.view?.slice ?? {};
  const baseState = sys.states.map((s) => (Number.isFinite(slice[s.name]) ? slice[s.name] : 0));
  const t = load.view?.t;
  const fieldTime = t !== undefined && Number.isFinite(t) ? t : 0;
  const layers: Layers = { ...DEFAULT_LAYERS, ...load.view?.layers };
  return { params, ics, view, baseState, fieldTime, layers, solver, warnings };
}
