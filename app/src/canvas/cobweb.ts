// The cobweb plot of a 1-D map x_{n+1} = f(x_n) (Strogatz §10.1): the
// staircase of an orbit between the graph of f and the diagonal y = x. The
// same geometry as `separatrix_export::cobweb` (the LaTeX figure), checked
// against it through crates/export/tests/vectors/cobweb.json.

import type { TrajectoryOut } from "../api/types";
import { sx, sy, type Viewport } from "./viewport";

/** Above this many iterates a staircase drops the segments it has already drawn. */
export const COBWEB_DEDUPE_ITERATES = 2000;

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);

/** The orbit's values of state `index` (a 1-D map's x_T … x_{T+N}). */
export function orbitValues(out: Pick<TrajectoryOut, "t" | "x">, dim = 1, index = 0): number[] {
  const n = dim > 0 ? Math.min(out.t.length, Math.floor(out.x.length / dim)) : 0;
  const v: number[] = [];
  for (let k = 0; k < n; k++) v.push(out.x[k * dim + index]);
  return v;
}

/**
 * The staircase of the orbit v_0 … v_m, as flat `[x, y, …]`: (v_0, v_0),
 * then for each k = 1..m the pair (v_{k−1}, v_k), (v_k, v_k). That is 2m + 1
 * vertices, starting and ending on the diagonal, alternating vertical
 * (up or down to f) and horizontal (across to the diagonal) segments. Cut at
 * the first non-finite value.
 */
export function cobwebVertices(v: ArrayLike<number>): number[] {
  const out: number[] = [];
  if (v.length === 0 || !Number.isFinite(v[0])) return out;
  out.push(v[0], v[0]);
  for (let k = 1; k < v.length; k++) {
    if (!Number.isFinite(v[k])) break;
    out.push(v[k - 1], v[k], v[k], v[k]);
  }
  return out;
}

/**
 * The part of the diagonal y = x inside the view, as `[a, a, b, b]` with
 * a = max(x0, y0) and b = min(x1, y1); null when it misses the view.
 */
export function diagonalSegment(
  xRange: readonly [number, number],
  yRange: readonly [number, number],
): [number, number, number, number] | null {
  const a = Math.max(Math.min(xRange[0], xRange[1]), Math.min(yRange[0], yRange[1]));
  const b = Math.min(Math.max(xRange[0], xRange[1]), Math.max(yRange[0], yRange[1]));
  return a < b ? [a, a, b, b] : null;
}

/** Quantize a pixel coordinate to 0.5 px for the retrace test. */
const q = (v: number) => Math.round(v * 2);

const cache = new WeakMap<object, { key: string; paths: Float64Array[] }>();

/**
 * The staircase in pixels, as subpaths. Above {@link COBWEB_DEDUPE_ITERATES}
 * iterates a segment already drawn (both ends the same to 0.5 px, either
 * way round) is dropped and the next one starts a new subpath, so a
 * converged p-cycle costs 2p segments whatever N is; the order is kept.
 * Cached per orbit and viewport, as hover and selection repaint often.
 */
export function cobwebPx(vp: Viewport, values: ArrayLike<number>, owner?: object): Float64Array[] {
  const key = `${vp.left},${vp.top},${vp.width},${vp.height},${vp.xRange},${vp.yRange}`;
  if (owner) {
    const hit = cache.get(owner);
    if (hit && hit.key === key) return hit.paths;
  }
  const verts = cobwebVertices(values);
  const n = verts.length / 2;
  const px = new Float64Array(verts.length);
  for (let k = 0; k < n; k++) {
    px[2 * k] = clampPx(sx(vp, verts[2 * k]));
    px[2 * k + 1] = clampPx(sy(vp, verts[2 * k + 1]));
  }
  let paths: Float64Array[];
  if (n === 0) paths = [];
  else if ((n - 1) / 2 <= COBWEB_DEDUPE_ITERATES) paths = [px];
  else {
    paths = [];
    const seen = new Set<string>();
    let cur: number[] = [];
    for (let k = 1; k < n; k++) {
      const ax = px[2 * k - 2];
      const ay = px[2 * k - 1];
      const bx = px[2 * k];
      const by = px[2 * k + 1];
      const a = `${q(ax)},${q(ay)}`;
      const b = `${q(bx)},${q(by)}`;
      const seg = a < b ? `${a};${b}` : `${b};${a}`;
      if (seen.has(seg)) {
        if (cur.length >= 4) paths.push(Float64Array.from(cur));
        cur = [];
        continue;
      }
      seen.add(seg);
      if (cur.length === 0) cur.push(ax, ay);
      cur.push(bx, by);
    }
    if (cur.length >= 4) paths.push(Float64Array.from(cur));
  }
  if (owner) cache.set(owner, { key, paths });
  return paths;
}

/**
 * How many different segments the staircase of `values` has, telling two
 * apart only when an end differs by more than 1/`cells` of the view's span
 * (about a pixel), up to `limit`: a converged p-cycle has 2p whatever N
 * is; a chaotic orbit's grow with N until the plot fills in.
 */
export function distinctSegments(
  values: ArrayLike<number>,
  xRange: readonly [number, number],
  yRange: readonly [number, number],
  limit: number,
  cells = 1000,
): number {
  const span = Math.max(Math.abs(xRange[1] - xRange[0]), Math.abs(yRange[1] - yRange[0])) || 1;
  const cell = (v: number) => Math.round((v / span) * cells);
  const verts = cobwebVertices(values);
  const seen = new Set<string>();
  for (let k = 2; k + 1 < verts.length && seen.size < limit; k += 2) {
    const a = `${cell(verts[k - 2])},${cell(verts[k - 1])}`;
    const b = `${cell(verts[k])},${cell(verts[k + 1])}`;
    seen.add(a < b ? `${a};${b}` : `${b};${a}`);
  }
  return seen.size;
}

/** The graph of f in pixels, one subpath per polyline (its breaks kept). */
export function graphPx(vp: Viewport, graph: readonly (readonly [number, number])[][]): Float64Array[] {
  const out: Float64Array[] = [];
  for (const line of graph) {
    if (line.length < 2) continue;
    const pts = new Float64Array(line.length * 2);
    line.forEach(([x, y], i) => {
      pts[2 * i] = clampPx(sx(vp, x));
      pts[2 * i + 1] = clampPx(sy(vp, y));
    });
    out.push(pts);
  }
  return out;
}

/** Squared distance from (px, py) to the segment a–b. */
export function segDist2(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  const t = len2 > 0 ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len2)) : 0;
  const x = ax + t * dx - px;
  const y = ay + t * dy - py;
  return x * x + y * y;
}

/** The least squared distance from (px, py) to any segment of `paths`. */
export function pathsDist2(px: number, py: number, paths: readonly Float64Array[]): number {
  let best = Infinity;
  for (const p of paths) {
    for (let k = 2; k < p.length; k += 2) {
      const d = segDist2(px, py, p[k - 2], p[k - 1], p[k], p[k + 1]);
      if (d < best) best = d;
    }
  }
  return best;
}

/**
 * A square range that fits the orbits and fixed points of a cobweb, padded
 * by 5%: the same on both axes, so the diagonal stays at 45°.
 */
export function cobwebFit(values: readonly number[]): [number, number] | null {
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of values) {
    if (!Number.isFinite(v) || Math.abs(v) > 1e12) continue;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  if (!(lo <= hi)) return null;
  const span = hi > lo ? hi - lo : Math.max(1, Math.abs(lo));
  const c = hi > lo ? (lo + hi) / 2 : lo;
  const half = (span * 1.1) / 2;
  return [c - half, c + half];
}
