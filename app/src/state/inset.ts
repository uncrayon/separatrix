// The Parameter plane's live inset: a small Phase view at the crosshair, the
// current parameters. Flows show the two states the Phase view shows; maps
// show the orbit after a transient.

import type { CompileResponse, FixedPointOut, PortraitResponse, SystemKind, TrajectoryOut } from "../api/types";
import type { PortraitJob } from "../api/client";
import { dec, plural } from "../math/format.ts";
import {
  buildPortraitJob,
  clampTransient,
  defaultView,
  viewChoice,
  type InitialCondition,
  type PlotView,
  type StudioSnapshot,
  type View,
} from "./model.ts";

/** Width over height of the inset's plot. */
export const INSET_ASPECT = 232 / 168;
/** Iterates of a map the inset always hides first, so the orbit shows where it settles. */
export const INSET_TRANSIENT = 1000;
/** Most iterates a settled orbit's period is looked for in. */
const MAX_PERIOD = 16;

export interface InsetMeta {
  kind: SystemKind;
  dim: number;
  /** The inset's view, with the ranges the request used. */
  view: PlotView;
  /** The palette index of each trajectory. */
  colors: number[];
  /** Iterates kept per orbit, after the transient (maps). */
  kept: number;
  /** The parameters it was computed at. */
  params: number[];
}

export interface InsetState {
  res: PortraitResponse;
  meta: InsetMeta;
}

/** The view the inset draws: the Phase view last shown, else the system's default phase plane; null where there is no phase plane. */
export function insetView(remembered: View | undefined, sys: CompileResponse): PlotView | null {
  if (sys.states.length < 2) return null;
  if (remembered && viewChoice(remembered) === "phase") return remembered as PlotView;
  const fallback = defaultView(sys);
  return viewChoice(fallback) === "phase" ? fallback : null;
}

/**
 * Where the inset starts trajectories when the Phase view has none: the
 * centre of the view and the middles of its four quarters, the other states
 * at the slice.
 */
export function insetStarts(view: PlotView, dim: number, baseState: readonly number[]): number[][] {
  const at = (fx: number, fy: number) => {
    const x0 = Array.from({ length: dim }, (_, i) => baseState[i] ?? 0);
    if (view.xAxis.kind === "state") x0[view.xAxis.index] = view.xRange[0] + fx * (view.xRange[1] - view.xRange[0]);
    if (view.yAxis.kind === "state") x0[view.yAxis.index] = view.yRange[0] + fy * (view.yRange[1] - view.yRange[0]);
    return x0;
  };
  return [at(0.5, 0.5), at(0.25, 0.25), at(0.75, 0.25), at(0.25, 0.75), at(0.75, 0.75)];
}

/**
 * The inset's request: the Phase view's axes, ranges, start points and
 * solver at the current parameters, with fixed points, without the field,
 * nullclines or backward halves; a map hides at least
 * {@link INSET_TRANSIENT} iterates.
 */
export function buildInsetJob(s: StudioSnapshot, view: PlotView): PortraitJob<InsetMeta> | null {
  const sys = s.compiled;
  if (!sys?.ok) return null;
  const dim = sys.states.length;
  const map = sys.kind === "map";
  const visible = s.ics.filter((ic) => !ic.hidden && ic.x0.length === dim);
  const ics: InitialCondition[] = visible.length
    ? visible
    : insetStarts(view, dim, s.baseState).map((x0, k) => ({ id: -1 - k, x0, t0: 0, color: k }));
  const solver = map ? { ...s.solver, transient: clampTransient(Math.max(s.solver.transient, INSET_TRANSIENT), s.solver.tEnd) } : s.solver;
  const job = buildPortraitJob({
    ...s,
    view,
    ics,
    solver,
    plotAspect: INSET_ASPECT,
    layers: { ...s.layers, field: false, nullclines: false, fixedPoints: true, backward: false },
  });
  if (!job) return null;
  const color = new Map(ics.map((ic) => [ic.id, ic.color]));
  return {
    body: job.body,
    meta: {
      kind: sys.kind ?? "ode",
      dim,
      view: job.meta.view as PlotView,
      colors: job.meta.trajectories.map((t) => color.get(t.icId) ?? 0),
      kept: Math.max(1, Math.round(s.solver.tEnd)),
      params: job.body.params,
    },
  };
}

/** "k₂ = 1.180, K = 0.600". */
export const insetParamsText = (names: readonly string[], values: readonly number[]): string =>
  names.map((n, i) => `${n} = ${dec(values[i], 3)}`).join(", ");

/**
 * What the fixed points in view are: "3 fixed points in view: 2 stable, 1
 * saddle.", "One fixed point in view, a stable spiral.", "No fixed point in
 * view." Two or more stable ones add that the start decides.
 */
export function fixedPointsText(fps: readonly Pick<FixedPointOut, "classification" | "stable">[]): string {
  if (fps.length === 0) return "No fixed point in view.";
  if (fps.length === 1) return `One fixed point in view, ${/^[aeiou]/.test(fps[0].classification) ? "an" : "a"} ${fps[0].classification}.`;
  const stable = fps.filter((fp) => fp.stable).length;
  const saddles = fps.filter((fp) => !fp.stable && fp.classification.startsWith("saddle")).length;
  const unstable = fps.filter((fp) => !fp.stable && (fp.classification.startsWith("unstable") || fp.classification === "source")).length;
  const other = fps.length - stable - saddles - unstable;
  const parts = [
    stable && `${stable} stable`,
    saddles && plural(saddles, "saddle"),
    unstable && `${unstable} unstable`,
    other && `${other} non-hyperbolic`,
  ].filter(Boolean);
  const choice = stable >= 2 ? " Where you start decides which stable state you reach." : "";
  return `${fps.length} fixed points in view: ${parts.join(", ")}.${choice}`;
}

/** How a map's orbit ends: the least period its last iterates repeat with, "escaped", or "wanders". */
export function orbitFate(tr: Pick<TrajectoryOut, "x" | "status">, dim: number, scale: number): number | "escaped" | "wanders" {
  const n = tr.x.length / dim;
  if (tr.status !== "completed" || tr.x.some((v) => !Number.isFinite(v))) return "escaped";
  const tol = 1e-6 * scale;
  const tail = Math.min(n - MAX_PERIOD, 4 * MAX_PERIOD);
  if (tail < 1) return "wanders";
  for (let p = 1; p <= MAX_PERIOD; p++) {
    let repeats = true;
    for (let k = n - tail; k < n && repeats; k++)
      for (let i = 0; i < dim; i++) if (Math.abs(tr.x[k * dim + i] - tr.x[(k - p) * dim + i]) > tol) repeats = false;
    if (repeats) return p;
  }
  return "wanders";
}

/** How orbits ended, for one orbit or for several. */
function fateText(fate: number | "escaped" | "wanders", kept: number, many: boolean): string {
  if (fate === "escaped") return many ? "blow up" : "blows up";
  if (fate === "wanders") return `${many ? "do" : "does"} not repeat within ${kept.toLocaleString("en-US")} iterates`;
  const settle = many ? "settle" : "settles";
  return fate === 1 ? `${settle} on a fixed point` : `${settle} on a period-${fate} cycle`;
}

/** The orbits' line: "The orbit settles on a period-4 cycle.", "The orbits do not repeat within 3,000 iterates.", or that they end differently. */
export function orbitsText(res: Pick<PortraitResponse, "trajectories">, meta: Pick<InsetMeta, "dim" | "view" | "kept">): string {
  if (res.trajectories.length === 0) return "";
  const scale = Math.max(meta.view.xRange[1] - meta.view.xRange[0], meta.view.yRange[1] - meta.view.yRange[0]);
  const fates = res.trajectories.map((tr) => orbitFate(tr, meta.dim, scale));
  const one = fates.every((f) => f === fates[0]);
  if (!one) return "The orbits end differently: where you start decides.";
  const many = fates.length > 1;
  return `The ${many ? "orbits" : "orbit"} ${fateText(fates[0], meta.kept, many)}.`;
}

/** The inset's caption under its parameters: the fixed points in view, and for a map how the orbits end. */
export function insetCaption(res: PortraitResponse, meta: InsetMeta): string {
  const fps = fixedPointsText(res.fixedPoints);
  return meta.kind === "map" ? `${fps} ${orbitsText(res, meta)}`.trim() : fps;
}
