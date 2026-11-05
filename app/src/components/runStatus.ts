import type { FixedPointOut, PortraitResponse, Status, TrajectoryOut } from "../api/types";
import { dec } from "./startPoint";

export const STATUS_WORDS: Record<Exclude<Status, "completed">, { words: string; stopped: boolean }> = {
  blewUp: { words: "Blew up at", stopped: false },
  maxSteps: { words: "Step limit at", stopped: true },
  stepTooSmall: { words: "Step too small at", stopped: true },
  nonFinite: { words: "NaN at", stopped: true },
};

/**
 * Why a run stopped, worded for its status line, and the n (or t) it names;
 * null when it completed. A map's NaN is x_{k+1}, one past the last iterate
 * returned (n = k); a map that blows up or hits NaN inside the transient
 * returns no iterates, and `iterate_map`'s accepted count says where.
 */
export function stopText(out: TrajectoryOut, isMap: boolean): { words: string; at: number; stopped: boolean } | null {
  if (out.status === "completed") return null;
  const info = STATUS_WORDS[out.status];
  const len = out.t.length;
  if (!isMap) return { ...info, at: len ? out.t[len - 1] : NaN };
  const { accepted, evals } = out.stats;
  const at = out.status === "nonFinite" ? (evals === 0 ? 0 : accepted + 1) : len ? out.t[len - 1] : accepted;
  const during = len === 0 && at > 0;
  return { ...info, words: during ? `${info.words.replace(/ at$/, "")} during the transient, at` : info.words, at };
}

/**
 * A map orbit that reaches an unstable fixed point exactly (x_{k+1} = x_k
 * bit for bit, to the end) and stays: a floating-point artefact (the tent
 * map at r = 2 doubles every step, so the 53 bits of a double run out).
 * The first iterate on it and the point; null otherwise.
 */
export function landedOnSource(
  out: TrajectoryOut,
  dim: number,
  fps: readonly FixedPointOut[],
  name = "x",
): { n: number; at: string } | null {
  const len = dim > 0 ? Math.min(out.t.length, Math.floor(out.x.length / dim)) : 0;
  if (out.status !== "completed" || len < 3) return null;
  const same = (a: number, b: number) => {
    for (let i = 0; i < dim; i++) if (out.x[a * dim + i] !== out.x[b * dim + i]) return false;
    return true;
  };
  if (!same(len - 1, len - 2)) return null;
  let k = len - 2;
  while (k > 0 && same(k - 1, len - 1)) k--;
  const x = out.x.slice((len - 1) * dim, len * dim);
  const fp = fps.find(
    (f) => f.x.length === dim && !f.stable && f.x.every((v, i) => Math.abs(v - x[i]) <= 1e-9 * Math.max(1, Math.abs(v))),
  );
  if (!fp || !fp.eigenvalues.some(([re, im]) => Math.hypot(re, im) > 1)) return null;
  const at = dim === 1 ? `${name}* = ${dec(x[0], 3)}` : `(${x.map((v) => dec(v, 3)).join(", ")})`;
  return { n: out.t[k], at };
}

export function sectionCounts(res: PortraitResponse, transient: number): { crossings: number; skipped: number } {
  const crossings = res.trajectories.reduce((n, tr) => n + tr.t.length, 0);
  return { crossings, skipped: Math.max(0, Math.round(transient)) };
}
