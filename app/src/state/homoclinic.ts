// Homoclinic curves started from Bogdanov–Takens points in the Parameter
// plane. Unlike an ordinary curve, one is a long job: it reports progress
// while it runs and can be cancelled, and a cancelled job keeps nothing.

import type { CompileResponse, HomoclinicRequest, HomoclinicResponse } from "../api/types";
import type { Job } from "../api/client";
import { SENTENCES, planeContext, runsOf } from "./curves.ts";
import type { View } from "./model.ts";

/**
 * A Bogdanov–Takens point to follow a homoclinic curve from: the
 * equilibrium there and every parameter's value there, so it keeps its
 * meaning whichever two parameters are free (as `PlaneSeed` does). The job
 * takes all of them from here, not from the sliders: a BT point is isolated,
 * so moving a parameter that is not free would leave it.
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
export const HOMOCLINIC_MAX_POINTS = 500;

export function homoclinicSeed(x: readonly number[], values: readonly number[], free: readonly [number, number], at: readonly [number, number]): HomoclinicSeed {
  const params = values.slice();
  params[free[0]] = at[0];
  params[free[1]] = at[1];
  return { x: x.slice(), params };
}

export const hasHomoclinicSeed = (seeds: readonly HomoclinicSeed[], seed: HomoclinicSeed) => {
  const key = JSON.stringify(seed);
  return seeds.some((s) => JSON.stringify(s) === key);
};

/** `seeds` with `add` appended unless an identical seed is there. */
export function withHomoclinicSeed(seeds: readonly HomoclinicSeed[], add: HomoclinicSeed): HomoclinicSeed[] {
  return hasHomoclinicSeed(seeds, add) ? seeds.slice() : [...seeds, add];
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

/** One request per seed, on a flow, keyed by its JSON (property order matters); every parameter is the seed's. */
export function buildHomoclinicJobs(s: { compiled: CompileResponse | null; view: View }): HomoclinicJob[] {
  const ctx = planeContext(s);
  if (!ctx || ctx.sys.kind === "map") return [];
  const { systemId, sys, v, c, n } = ctx;
  return c.homoclinic.flatMap((seed, i): HomoclinicJob[] => {
    if (seed.x.length !== sys.states.length || seed.params.length !== n) return [];
    const body: HomoclinicJob["body"] = {
      systemId,
      params: seed.params.slice(),
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

/**
 * The runs after a job ends: those still wanted stay, and so does the job's
 * own result when it is wanted. A cancelled job (null) keeps nothing, so a key
 * wanted again reruns. Outside the plane no job is wanted, which says nothing
 * about the finished runs, so they all stay.
 */
export function settleHomoclinic(
  runs: Readonly<Record<string, HomoclinicRun>>,
  wanted: ReadonlySet<string>,
  key: string,
  run: HomoclinicRun | null,
  inPlane: boolean,
): Record<string, HomoclinicRun> {
  const next: Record<string, HomoclinicRun> = {};
  for (const [k, v] of Object.entries(runs)) if (!inPlane || wanted.has(k)) next[k] = v;
  if (run && wanted.has(key)) next[key] = run;
  return next;
}

/** An id no earlier job of this or a previous run of the app had: the shell keeps a cancel flag per id. */
let idCounter = 0;
export function homoclinicRequestId(now: number = Date.now()): number {
  idCounter = (idCounter + 1) % 1000;
  return now * 1000 + idCounter;
}

type Curve = Pick<HomoclinicResponse["curve"], "p1" | "p2" | "sigma" | "chaotic" | "reversed" | "end">;

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

/** The lines under a done row: size and ends, σ, chaos, and the time direction when reversed. */
export function homoclinicLines(c: Curve, names: readonly [string, string]): string[] {
  const say = SENTENCES.homoclinic;
  const at = (i: number): [number, number] => [c.p1[i], c.p2[i]];
  const chaos = runsOf(c.chaotic).map(([a, b]) => (a === b ? ([at(a)] as const) : ([at(a), at(b)] as const)));
  return [
    say.extent(c.p1.length, c.end),
    say.sigma(sigmaCrossings(c), Math.sign(c.sigma.find((v) => v !== 0) ?? 0), names),
    say.chaos(chaos, names),
    ...(c.reversed ? [say.reversed] : []),
  ];
}
