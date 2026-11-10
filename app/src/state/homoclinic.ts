// Homoclinic curves started from Bogdanov–Takens points in the Parameter
// plane. Unlike an ordinary curve, one is a long job: it reports progress
// while it runs and can be cancelled, and a cancelled job keeps nothing.

import type {
  CompileResponse,
  HomoclinicProgress,
  HomoclinicRequest,
  HomoclinicResponse,
  HomoclinicStop,
} from "../api/types";
import { MAX_HOMOCLINIC_POINTS } from "../api/ipc.generated.ts";
import type { Job } from "../api/client";
import { CODIM2_KINDS, curvesAvailability, curvesSettingsError, dec4 } from "./curves.ts";
import { isCurves, type ParamState, type View } from "./model.ts";

/**
 * A Bogdanov–Takens point to follow a homoclinic curve from: the
 * equilibrium there and every parameter's value there, so it keeps its
 * meaning whichever two parameters are free (as `PlaneSeed` does).
 */
export interface HomoclinicSeed {
  x: number[];
  params: number[];
}

/**
 * Most points each way. Every point is a shooting, so a curve of the
 * default 2,000 would run for minutes; 500 reaches across a plane of the
 * reference systems and stays within `MAX_HOMOCLINIC_POINTS`.
 */
export const HOMOCLINIC_MAX_POINTS = Math.min(500, MAX_HOMOCLINIC_POINTS);

export const HOMOCLINIC_NAME = "Homoclinic curve";
export const HOMOCLINIC_LEGEND = "Homoclinic (Hom)";
export const HOMOCLINIC_LABEL = "Hom";

export function homoclinicSeed(x: readonly number[], values: readonly number[], free: readonly [number, number], at: readonly [number, number]): HomoclinicSeed {
  const params = values.slice();
  params[free[0]] = at[0];
  params[free[1]] = at[1];
  return { x: x.slice(), params };
}

/** `seeds` with `add` appended unless an identical seed is there. */
export function withHomoclinicSeed(seeds: readonly HomoclinicSeed[], add: HomoclinicSeed): HomoclinicSeed[] {
  const key = JSON.stringify(add);
  return seeds.some((s) => JSON.stringify(s) === key) ? seeds.slice() : [...seeds, add];
}

interface HomoclinicMeta {
  key: string;
  seed: number;
}

export type HomoclinicJob = Job<Omit<HomoclinicRequest, "requestId">, HomoclinicMeta>;

export type HomoclinicRun = { ok: true; res: HomoclinicResponse; roundTripMs: number } | { ok: false; error: string };

/** The job in flight and the latest point it reported; `points` is 0 before the first message. */
export interface HomoclinicProgressState {
  key: string;
  points: number;
  p: [number, number] | null;
}

/** One request per seed, on a flow, keyed by its JSON (property order matters). */
export function buildHomoclinicJobs(s: {
  compiled: CompileResponse | null;
  params: readonly Pick<ParamState, "value">[];
  view: View;
}): HomoclinicJob[] {
  const sys = s.compiled;
  if (!sys || !sys.ok || sys.systemId === undefined || sys.kind === "map" || !isCurves(s.view) || !curvesAvailability(sys).ok) return [];
  const v = s.view;
  const c = v.curves;
  const n = sys.params.length;
  if (v.across >= n || v.up >= n || v.across === v.up || s.params.length !== n || curvesSettingsError(c)) return [];
  const systemId = sys.systemId;
  return c.homoclinic.flatMap((seed, i): HomoclinicJob[] => {
    if (seed.x.length !== sys.states.length || seed.params.length !== n) return [];
    const body: HomoclinicJob["body"] = {
      systemId,
      params: s.params.map((q, j) => (j === v.across || j === v.up ? seed.params[j] : q.value)),
      free: [v.across, v.up],
      ranges: [
        [c.ranges[0][0], c.ranges[0][1]],
        [c.ranges[1][0], c.ranges[1][1]],
      ],
      x: seed.x.slice(),
      direction: c.direction,
      ds: c.ds,
      dsMax: 2 * c.ds,
      maxPoints: HOMOCLINIC_MAX_POINTS,
    };
    return [{ body, meta: { key: JSON.stringify(body), seed: i } }];
  });
}

/**
 * One row of the Curves list: the job running (with the latest progress, or
 * none before the first message and while it waits for its turn), or what
 * it came to.
 */
export type HomoclinicRow =
  | { status: "computing"; job: HomoclinicJob; progress: { points: number; p: [number, number] } | null }
  | { status: "done"; job: HomoclinicJob; res: HomoclinicResponse; roundTripMs: number }
  | { status: "failed"; job: HomoclinicJob; error: string };

export function homoclinicRows(
  jobs: readonly HomoclinicJob[],
  runs: Readonly<Record<string, HomoclinicRun>>,
  progress: HomoclinicProgressState | null,
): HomoclinicRow[] {
  return jobs.map((job): HomoclinicRow => {
    const run = runs[job.meta.key];
    if (!run) {
      const latest = progress && progress.key === job.meta.key && progress.p ? { points: progress.points, p: progress.p } : null;
      return { status: "computing", job, progress: latest };
    }
    return run.ok
      ? { status: "done", job, res: run.res, roundTripMs: run.roundTripMs }
      : { status: "failed", job, error: run.error };
  });
}

export const drawnHomoclinic = (rows: readonly HomoclinicRow[]) =>
  rows.flatMap((r) => (r.status === "done" ? [r.res.curve] : []));

/** An id no earlier job of this or a previous run of the app had: the shell keeps a cancel flag per id. */
let idCounter = 0;
export function homoclinicRequestId(now: number = Date.now()): number {
  idCounter = (idCounter + 1) % 1000;
  return now * 1000 + idCounter;
}

/** "Computing the homoclinic curve: 37 points, at ν = −0.7213, β = 0.6000". */
export function computingText(progress: Pick<HomoclinicProgress, "points" | "p"> | null, names: readonly [string, string]): string {
  if (!progress) return "Computing the homoclinic curve…";
  const n = progress.points;
  return `Computing the homoclinic curve: ${n} ${n === 1 ? "point" : "points"}, at ${names[0]} = ${dec4(progress.p[0])}, ${names[1]} = ${dec4(progress.p[1])}`;
}

const BT = CODIM2_KINDS.bogdanovTakens.label;
/** How an end stopped, for one end ("it …") and for both ("both ends …"). */
const ENDS: Record<HomoclinicStop, [one: string, both: string]> = {
  range: ["runs to the edge of the box", "run to the edge of the box"],
  maxPoints: ["stops at Max points", "stop at Max points"],
  stepTooSmall: ["stops at a step too small to go on", "stop at a step too small to go on"],
  shrunk: [`shrinks onto the saddle at ${BT}`, `shrink onto the saddle at ${BT}`],
};

export function homoclinicEndText(end: { first: HomoclinicStop | null; last: HomoclinicStop }): string {
  if (end.first === null) return `the end ${ENDS[end.last][0]}`;
  if (end.first === end.last) return `both ends ${ENDS[end.last][1]}`;
  return `one end ${ENDS[end.first][0]}, the other ${ENDS[end.last][0]}`;
}

type Curve = Pick<HomoclinicResponse["curve"], "p1" | "p2" | "sigma" | "chaotic" | "reversed" | "end">;

const at = (c: Pick<Curve, "p1" | "p2">, i: number, names: readonly [string, string]) =>
  `${names[0]} = ${dec4(c.p1[i])}, ${names[1]} = ${dec4(c.p2[i])}`;

/** Where σ crosses 0, each found by linear interpolation between the two points it falls between. */
export function sigmaCrossings(c: Pick<Curve, "p1" | "p2" | "sigma">): [number, number][] {
  const out: [number, number][] = [];
  for (let i = 0; i + 1 < c.sigma.length; i++) {
    const a = c.sigma[i];
    const b = c.sigma[i + 1];
    if (a === 0 || !(a < 0 !== b < 0) || b === 0) continue;
    const t = a / (a - b);
    out.push([c.p1[i] + t * (c.p1[i + 1] - c.p1[i]), c.p2[i] + t * (c.p2[i + 1] - c.p2[i])]);
  }
  return out;
}

const SHOWN = 3;
const more = (list: readonly string[]) =>
  list.length > SHOWN ? [...list.slice(0, SHOWN), `${list.length - SHOWN} more`] : list;
const joinAnd = (list: readonly string[]) =>
  list.length > 1 ? `${list.slice(0, -1).join("; ")} and ${list[list.length - 1]}` : list.join("");

export function sigmaText(c: Pick<Curve, "p1" | "p2" | "sigma">, names: readonly [string, string]): string {
  const cross = sigmaCrossings(c);
  if (cross.length === 0) {
    const s = c.sigma.find((v) => v !== 0);
    return s === undefined ? "σ is 0 along the curve" : `σ stays ${s > 0 ? "positive" : "negative"} along the curve`;
  }
  const where = more(cross.map(([a, b]) => `${names[0]} = ${dec4(a)}, ${names[1]} = ${dec4(b)}`));
  return `σ changes sign at ${joinAnd(where)}`;
}

export function chaosText(c: Pick<Curve, "p1" | "p2" | "chaotic">, names: readonly [string, string]): string {
  const runs: [number, number][] = [];
  c.chaotic.forEach((on, i) => {
    if (!on) return;
    const last = runs[runs.length - 1];
    if (last && last[1] === i - 1) last[1] = i;
    else runs.push([i, i]);
  });
  if (runs.length === 0) return "No Shilnikov chaos along it";
  const where = more(runs.map(([a, b]) => (a === b ? `at ${at(c, a, names)}` : `from ${at(c, a, names)} to ${at(c, b, names)}`)));
  return `Shilnikov chaos ${joinAnd(where)}`;
}

/** The lines under a done row: size and ends, σ, chaos, and the time direction when reversed. */
export function homoclinicLines(c: Curve, names: readonly [string, string]): string[] {
  const n = c.p1.length;
  return [
    `${n.toLocaleString("en-US")} points · ${homoclinicEndText(c.end)}`,
    sigmaText(c, names),
    chaosText(c, names),
    ...(c.reversed ? ["Followed in reversed time"] : []),
  ];
}
