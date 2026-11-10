// The Parameter plane: bifurcation curves followed in two parameters. The
// types and pure functions here sit beside the Branch view's in model.ts,
// which imports them.

import type {
  CompileResponse,
  Codim2KindOut,
  ContinuationDirection,
  CurveKindOut,
  CurveOut,
  CurveRequest,
  CurveResponse,
  SpecialKindOut,
  SpecialPointOut,
  SystemKind,
} from "../api/types";
import { MAX_CONTINUATION_POINTS } from "../api/ipc.generated.ts";
import type { Job } from "../api/client";
import type { HomoclinicSeed } from "./homoclinic.ts";
import {
  followParamIndex,
  isCurves,
  scientificText,
  type Availability,
  type ContinuationView,
  type ParamState,
  type TimeAxis,
  type View,
} from "./model.ts";

interface CurveKindInfo {
  /** The one-parameter bifurcation's label, as in the Branch view. */
  label: string;
  /** The row title. */
  name: string;
  /** The legend key. */
  legend: string;
  /** How "On the … curve" names it. */
  on: string;
  /** How "Follow this … in two parameters" names the point it starts from. */
  point: string;
  /** The special point of the Branch view it is followed from. */
  from: SpecialKindOut;
}

/** Every curve kind, in the order the legend and the lists use. */
export const CURVE_KINDS: Record<CurveKindOut, CurveKindInfo> = {
  fold: { label: "LP", name: "Fold curve", legend: "Fold (LP)", on: "fold", point: "fold", from: "fold" },
  hopf: { label: "H", name: "Hopf curve", legend: "Hopf (H)", on: "Hopf", point: "Hopf point", from: "hopf" },
  branchPoint: {
    label: "BP",
    name: "Branch-point curve",
    legend: "Branch point (BP)",
    on: "branch-point",
    point: "branch point",
    from: "branchPoint",
  },
  flip: { label: "PD", name: "Flip curve", legend: "Flip (PD)", on: "flip", point: "flip", from: "flip" },
  neimarkSacker: {
    label: "NS",
    name: "Neimark–Sacker curve",
    legend: "Neimark–Sacker (NS)",
    on: "NS",
    point: "Neimark–Sacker point",
    from: "neimarkSacker",
  },
};
export const CURVE_KIND_ORDER = Object.keys(CURVE_KINDS) as CurveKindOut[];

/** The curve a special point of the Branch view is followed along, or null when no curve kind starts from it. */
export const curveKindOf = (kind: SpecialKindOut): CurveKindOut | null =>
  CURVE_KIND_ORDER.find((k) => CURVE_KINDS[k].from === kind) ?? null;

export const NEUTRAL_LEGEND = "No bifurcation (neutral saddle)";

/** Every codimension-2 point, in the order the Special points list uses. */
export const CODIM2_KINDS: Record<Codim2KindOut, { label: string; name: string }> = {
  bogdanovTakens: { label: "BT", name: "Bogdanov–Takens" },
  cusp: { label: "CP", name: "Cusp" },
  bautin: { label: "GH", name: "Bautin" },
  zeroHopf: { label: "ZH", name: "Zero-Hopf" },
  hopfHopf: { label: "HH", name: "Hopf–Hopf" },
  resonance1: { label: "R1", name: "1:1 resonance" },
  resonance2: { label: "R2", name: "1:2 resonance" },
  resonance3: { label: "R3", name: "1:3 resonance" },
  resonance4: { label: "R4", name: "1:4 resonance" },
  chenciner: { label: "CH", name: "Chenciner" },
  foldFlip: { label: "LPPD", name: "Fold-flip" },
  generalizedFlip: { label: "GPD", name: "Generalized flip" },
};
const CODIM2_ORDER = Object.keys(CODIM2_KINDS) as Codim2KindOut[];

/**
 * A bifurcation to follow: its kind, the state there, and every parameter's
 * value there, so it keeps its meaning whichever two parameters are free.
 */
export interface PlaneSeed {
  kind: CurveKindOut;
  x: number[];
  period: number;
  params: number[];
}

export interface CurvesSettings {
  /** The box the curves stay in: across, then up. */
  ranges: [[number, number], [number, number]];
  ds: number;
  maxPoints: number;
  direction: ContinuationDirection;
  seeds: PlaneSeed[];
  /** The Bogdanov–Takens points a homoclinic curve is followed from (flows). */
  homoclinic: HomoclinicSeed[];
}

export type CurvesView = {
  kind: "curves";
  xAxis: TimeAxis;
  yAxis: TimeAxis;
  xRange: [number, number];
  yRange: [number, number];
  /** The parameter drawn across, and the one drawn up. */
  across: number;
  up: number;
  curves: CurvesSettings;
  /** The Branch view as it was when the plane opened: its range is the slice. */
  branch: ContinuationView | null;
  /**
   * Every parameter's value when the plane opened from the Branch view, which
   * its branches were computed at: the slice is drawn at the up parameter's.
   */
  branchParams: number[] | null;
  /**
   * What a project file saves while the plane is shown, as the file format
   * has no plane yet: the Branch view, else the default view.
   */
  home: Exclude<View, CurvesView>;
};

const CURVES_DS = 0.01;
const CURVES_MAX_POINTS = 2000;

export function curvesAvailability(
  sys: Pick<CompileResponse, "autonomous" | "kind"> & Partial<Pick<CompileResponse, "params">> | null | undefined,
  name = "this system",
): Availability {
  const n = sys?.params?.length ?? 0;
  if (!sys || n < 2) return { ok: false, reason: `The parameter plane needs two parameters; ${name} has ${n}.` };
  if (!sys.autonomous)
    return {
      ok: false,
      reason: `The parameter plane follows bifurcations of fixed points; ${name} changes with ${sys.kind === "map" ? "$n$" : "$t$"}, so it has none.`,
    };
  return { ok: true };
}

export const sliderRange = (p: Pick<ParamState, "min" | "max"> | undefined): [number, number] =>
  p && p.min < p.max ? [p.min, p.max] : [0, 1];

function padRange([lo, hi]: readonly [number, number], f = 0.04): [number, number] {
  const pad = f * (hi - lo || Math.max(1, Math.abs(lo)));
  return [lo - pad, hi + pad];
}

/** The Parameter plane with `across` and `up` free over their sliders' ranges, and no curve yet. */
export function curvesView(
  params: readonly Pick<ParamState, "min" | "max">[],
  across: number,
  up: number,
  branch: ContinuationView | null,
  home: CurvesView["home"],
): CurvesView {
  const ranges: CurvesSettings["ranges"] = [sliderRange(params[across]), sliderRange(params[up])];
  return {
    kind: "curves",
    xAxis: { kind: "time" },
    yAxis: { kind: "time" },
    xRange: padRange(ranges[0]),
    yRange: padRange(ranges[1]),
    across,
    up,
    curves: { ranges, ds: CURVES_DS, maxPoints: CURVES_MAX_POINTS, direction: "both", seeds: [], homoclinic: [] },
    branch,
    branchParams: null,
    home,
  };
}

/** The parameter offered up first: the next one after `across` in declaration order. */
export const nextParam = (across: number, n: number): number => (n > 0 ? (across + 1) % n : 0);

export function planeSeed(at: SpecialPointOut, values: readonly number[], param: number): PlaneSeed | null {
  const kind = curveKindOf(at.kind);
  if (!kind) return null;
  const params = values.slice();
  params[param] = at.p;
  return { kind, x: at.x.slice(), period: at.period, params };
}

function withPlaneSeeds(seeds: readonly PlaneSeed[], add: readonly PlaneSeed[]): PlaneSeed[] {
  const out = seeds.slice();
  const keys = new Set(out.map((s) => JSON.stringify(s)));
  for (const seed of add) {
    const key = JSON.stringify(seed);
    if (keys.has(key)) continue;
    keys.add(key);
    out.push(seed);
  }
  return out;
}

/**
 * The plane an entry action opens, with `seeds` added: across is the Branch
 * view's parameter (else parameter 0) and up is `up`, else the remembered
 * plane's when it has the same across, else the next parameter. The
 * remembered plane is kept, curves and all, when it has the same two.
 */
export function openPlane(
  kept: CurvesView | null,
  branch: ContinuationView | null,
  home: CurvesView["home"],
  params: readonly PlaneParam[],
  seeds: readonly PlaneSeed[],
  up?: number,
): CurvesView {
  const across = branch?.param ?? kept?.across ?? 0;
  const u = up ?? (kept && kept.across === across ? kept.up : nextParam(across, params.length));
  const base = kept && kept.across === across && kept.up === u ? kept : curvesView(params, across, u, branch, home);
  return { ...withBranch(base, branch, params), home, curves: { ...base.curves, seeds: withPlaneSeeds(base.curves.seeds, seeds) } };
}

type PlaneParam = Pick<ParamState, "min" | "max"> & Partial<Pick<ParamState, "value">>;

/** `view` tied to the Branch view `branch`, whose branches were computed at the values of `params`. */
export function withBranch(view: CurvesView, branch: ContinuationView | null, params: readonly PlaneParam[]): CurvesView {
  const values = params.map((p) => p.value);
  const known = values.every((v): v is number => v !== undefined);
  return { ...view, branch, branchParams: branch && known ? (values as number[]) : null };
}

export function curvesSettingsError(
  s: Omit<CurvesSettings, "seeds" | "direction" | "homoclinic">,
): { field: "across" | "up" | "ds" | "maxPoints"; message: string } | null {
  const bad = ([lo, hi]: readonly [number, number]) => !(Number.isFinite(lo) && Number.isFinite(hi) && lo < hi);
  if (bad(s.ranges[0])) return { field: "across", message: "The lower end must be below the upper" };
  if (bad(s.ranges[1])) return { field: "up", message: "The lower end must be below the upper" };
  if (!(Number.isFinite(s.ds) && s.ds > 0)) return { field: "ds", message: "Must be positive" };
  if (!(Number.isInteger(s.maxPoints) && s.maxPoints >= 2 && s.maxPoints <= MAX_CONTINUATION_POINTS))
    return { field: "maxPoints", message: `Must be a whole number from 2 to ${MAX_CONTINUATION_POINTS.toLocaleString("en-US")}` };
  return null;
}

interface CurveMeta {
  key: string;
  seed: number;
}

type CurveJob = Job<Omit<CurveRequest, "requestId">, CurveMeta>;

export type CurveRun = { ok: true; res: CurveResponse; roundTripMs: number } | { ok: false; error: string };

/** One request per seed, keyed by its JSON, every parameter but the free two from its slider. */
export function buildCurveJobs(s: {
  compiled: CompileResponse | null;
  params: readonly Pick<ParamState, "value">[];
  view: View;
}): CurveJob[] {
  const sys = s.compiled;
  if (!sys || !sys.ok || sys.systemId === undefined || !isCurves(s.view) || !curvesAvailability(sys).ok) return [];
  const v = s.view;
  const c = v.curves;
  const n = sys.params.length;
  if (v.across >= n || v.up >= n || v.across === v.up || s.params.length !== n || curvesSettingsError(c)) return [];
  const systemId = sys.systemId;
  return c.seeds.flatMap((seed, i): CurveJob[] => {
    if (seed.x.length !== sys.states.length || seed.params.length !== n) return [];
    const body: CurveJob["body"] = {
      systemId,
      params: s.params.map((q, j) => (j === v.across || j === v.up ? seed.params[j] : q.value)),
      free: [v.across, v.up],
      ranges: [
        [c.ranges[0][0], c.ranges[0][1]],
        [c.ranges[1][0], c.ranges[1][1]],
      ],
      seed: { kind: seed.kind, x: seed.x.slice(), period: seed.period },
      direction: c.direction,
      ds: c.ds,
      dsMax: 2 * c.ds,
      maxPoints: c.maxPoints,
    };
    return [{ body, meta: { key: JSON.stringify(body), seed: i } }];
  });
}

/** A seed whose curve repeats one already drawn lies on it within this fraction of the box. */
const SAME_CURVE_TOL = 1e-3;
/** Two codimension-2 points within this fraction of the box are one. */
export const SAME_POINT_TOL = 1e-6;

/** Distance from (a, b) to the polyline of `c`, each parameter scaled by its range. */
function distanceToCurve(c: Pick<CurveOut, "p1" | "p2">, a: number, b: number, w1: number, w2: number): number {
  let best = Infinity;
  const n = c.p1.length;
  for (let i = 0; i < n; i++) {
    const x0 = (c.p1[i] - a) / w1;
    const y0 = (c.p2[i] - b) / w2;
    if (i + 1 >= n) {
      best = Math.min(best, Math.hypot(x0, y0));
      break;
    }
    const dx = (c.p1[i + 1] - c.p1[i]) / w1;
    const dy = (c.p2[i + 1] - c.p2[i]) / w2;
    const len2 = dx * dx + dy * dy;
    const t = len2 > 0 ? Math.min(1, Math.max(0, -(x0 * dx + y0 * dy) / len2)) : 0;
    best = Math.min(best, Math.hypot(x0 + t * dx, y0 + t * dy));
  }
  return best;
}

/**
 * One row of the Curves list. `computing` is the request in flight (the
 * first seed without a result); seeds after it are not shown until their
 * turn. `merged` is a curve that repeats an earlier row's, drawn once.
 */
export type CurveRow =
  | { status: "done"; job: CurveJob; res: CurveResponse; roundTripMs: number }
  | { status: "merged"; job: CurveJob; res: CurveResponse; into: number }
  | { status: "failed"; job: CurveJob; error: string }
  | { status: "computing"; job: CurveJob };

export function curveRows(
  shown: readonly { job: CurveJob; run: CurveRun | undefined }[],
  ranges: CurvesSettings["ranges"],
): CurveRow[] {
  const w1 = ranges[0][1] - ranges[0][0];
  const w2 = ranges[1][1] - ranges[1][0];
  const rows: CurveRow[] = [];
  let computing = false;
  for (const { job, run } of shown) {
    if (!run) {
      if (!computing) rows.push({ status: "computing", job });
      computing = true;
      continue;
    }
    if (!run.ok) {
      rows.push({ status: "failed", job, error: run.error });
      continue;
    }
    const c = run.res.curve;
    const [a, b] = job.body.free.map((i) => job.body.params[i]);
    const into = rows.findIndex(
      (r) =>
        r.status === "done" &&
        r.res.curve.kind === c.kind &&
        r.res.curve.period === c.period &&
        distanceToCurve(r.res.curve, a, b, w1, w2) <= SAME_CURVE_TOL,
    );
    rows.push(into >= 0 ? { status: "merged", job, res: run.res, into } : { status: "done", job, res: run.res, roundTripMs: run.roundTripMs });
  }
  return rows;
}

/** The rows of the plane's Curves list, or none when the view is not the plane. */
export function shownCurves(s: {
  compiled: CompileResponse | null;
  params: readonly Pick<ParamState, "value">[];
  view: View;
  curveRuns: Readonly<Record<string, CurveRun>>;
}): CurveRow[] {
  if (!isCurves(s.view)) return [];
  return curveRows(
    buildCurveJobs(s).map((job) => ({ job, run: s.curveRuns[job.meta.key] })),
    s.view.curves.ranges,
  );
}

export const drawnCurves = (rows: readonly CurveRow[]) =>
  rows.flatMap((r) => (r.status === "done" ? [r.res.curve] : []));

export interface PlanePointRow {
  kind: Codim2KindOut;
  p: [number, number];
  x: number[];
  period: number;
  /** The kinds of the curves it was found on, in kind order. */
  on: CurveKindOut[];
  /** The widest of its bisection brackets on those curves, or null where none was bisected (the preview). */
  bracket: number | null;
}

/** The Special points list: each point once, with every curve kind it lies on; by kind, then across. */
export function planePointRows(curves: readonly CurveOut[], ranges: CurvesSettings["ranges"]): PlanePointRow[] {
  const w1 = ranges[0][1] - ranges[0][0];
  const w2 = ranges[1][1] - ranges[1][0];
  const rows: PlanePointRow[] = [];
  for (const c of curves) {
    for (const sp of c.special) {
      const same = rows.find(
        (r) =>
          r.kind === sp.kind &&
          r.period === sp.period &&
          Math.abs(r.p[0] - sp.p[0]) <= SAME_POINT_TOL * w1 &&
          Math.abs(r.p[1] - sp.p[1]) <= SAME_POINT_TOL * w2,
      );
      if (same) {
        if (!same.on.includes(c.kind)) same.on.push(c.kind);
        if (sp.bracket !== null) same.bracket = Math.max(same.bracket ?? 0, sp.bracket);
      } else
        rows.push({ kind: sp.kind, p: [sp.p[0], sp.p[1]], x: sp.x.slice(), period: sp.period, on: [c.kind], bracket: sp.bracket });
    }
  }
  for (const r of rows) r.on.sort((a, b) => CURVE_KIND_ORDER.indexOf(a) - CURVE_KIND_ORDER.indexOf(b));
  return rows.sort((a, b) => CODIM2_ORDER.indexOf(a.kind) - CODIM2_ORDER.indexOf(b.kind) || a.p[0] - b.p[0]);
}

/** "located to 1.5 × 10⁻¹³", or null for a point no bisection located. */
export const locatedText = (bracket: number | null): string | null => (bracket === null ? null : `located to ${scientificText(bracket)}`);

/** "On the fold and Hopf curves". */
export function onCurvesText(on: readonly CurveKindOut[]): string {
  const names = on.map((k) => CURVE_KINDS[k].on);
  const list = names.length > 1 ? `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}` : names.join("");
  return `On the ${list} ${names.length > 1 ? "curves" : "curve"}`;
}

/** The runs of consecutive neutral points, as [first, last] indices. */
function neutralRuns(neutral: readonly boolean[]): [number, number][] {
  const runs: [number, number][] = [];
  neutral.forEach((n, i) => {
    if (!n) return;
    const last = runs[runs.length - 1];
    if (last && last[1] === i - 1) last[1] = i;
    else runs.push([i, i]);
  });
  return runs;
}

/**
 * Where a curve is no bifurcation, for its row: "neutral past the BT
 * points" when every neutral run starts at a special point of one label,
 * else "neutral where dotted"; "" when it has none.
 */
export function neutralText(c: Pick<CurveOut, "neutral" | "special">): string {
  const runs = neutralRuns(c.neutral);
  if (!runs.length) return "";
  const near = (i: number) => c.special.find((sp) => Math.abs(sp.index - i) <= 1);
  const labels = runs.map(([a, b]) => {
    const sp = near(a) ?? near(b);
    return sp ? CODIM2_KINDS[sp.kind].label : null;
  });
  const label = labels[0];
  if (label === null || labels.some((l) => l !== label)) return "neutral where dotted";
  return runs.length > 1 ? `neutral past the ${label} points` : `neutral past ${label}`;
}

export const dec4 = (v: number) => v.toFixed(4).replace(/^-/, "−");

/**
 * The warning of a curve that stopped because the step became too small,
 * or null: where, and the likely reason, which differs for a flow and a map.
 */
export function stallText(c: Pick<CurveOut, "p1" | "p2" | "end">, names: readonly [string, string], kind: SystemKind | undefined): string | null {
  const i = c.end.first === "stepTooSmall" ? 0 : c.end.last === "stepTooSmall" ? c.p1.length - 1 : -1;
  if (i < 0) return null;
  const hint =
    kind === "map"
      ? "The map may be singular there, or a special point may be near. Lower Δs."
      : "A cusp or Bogdanov–Takens point may be there. Lower Δs.";
  return `The step became too small near ${names[0]} = ${dec4(c.p1[i])}, ${names[1]} = ${dec4(c.p2[i])}. ${hint}`;
}

export const retryStep = (ds: number): number => Number((ds / 2).toPrecision(6));

const ORDINALS = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];
/** "Same curve as the first: merged". */
export const mergedText = (into: number): string => `Same curve as the ${ORDINALS[into] ?? `curve ${into + 1}`}: merged`;

export function curvesSummary(rows: readonly CurveRow[], ranges: CurvesSettings["ranges"]) {
  let elapsedMs = 0;
  let roundTripMs = 0;
  let points = 0;
  let curves = 0;
  for (const r of rows) {
    if (r.status !== "done") continue;
    curves += 1;
    elapsedMs += r.res.elapsedMs;
    roundTripMs += r.roundTripMs;
    points += r.res.curve.p1.length;
  }
  return { elapsedMs, roundTripMs, points, curves, special: planePointRows(drawnCurves(rows), ranges).length };
}

/** The view's ranges fitted to the curves drawn, or null when none is. */
export function fitCurveRanges(curves: readonly Pick<CurveOut, "p1" | "p2">[]): { xRange: [number, number]; yRange: [number, number] } | null {
  let x0 = Infinity;
  let x1 = -Infinity;
  let y0 = Infinity;
  let y1 = -Infinity;
  for (const c of curves) {
    c.p1.forEach((a, i) => {
      const b = c.p2[i];
      if (!Number.isFinite(a) || !Number.isFinite(b)) return;
      x0 = Math.min(x0, a);
      x1 = Math.max(x1, a);
      y0 = Math.min(y0, b);
      y1 = Math.max(y1, b);
    });
  }
  return x0 <= x1 && y0 <= y1 ? { xRange: padRange([x0, x1], 0.06), yRange: padRange([y0, y1], 0.06) } : null;
}

/** The plane after a recompile: across and up follow their parameters by name; null when either is gone. */
export function followPlaneParams(
  view: CurvesView,
  oldParams: readonly { name: string }[],
  newParams: readonly { name: string }[],
): CurvesView | null {
  const across = followParamIndex(oldParams, newParams, view.across);
  const up = followParamIndex(oldParams, newParams, view.up);
  if (across < 0 || up < 0 || across === up) return null;
  return across === view.across && up === view.up ? view : { ...view, across, up, branchParams: null };
}
