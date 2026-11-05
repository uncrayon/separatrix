// The 3D view of a flow with three or more states (Strogatz ch. 9's Lorenz
// pictures): a camera that orbits the box of the three state ranges, the
// orthographic projection of that box, and the 2D drawing every surface
// shares (the overlay on screen, the fallback without WebGL, exports).
//
// The box is normalised to [−1, 1]³: box coordinate k of a state vector p is
// (p[axis k] − mid_k) / half_k. The camera turns about the box's third axis
// (yaw) and tilts towards it (pitch); with both 0 the first axis points
// right, the third up and the second away from the viewer. three.js draws
// only the trajectory lines, through an OrthographicCamera set from
// {@link threeCamera}, so the overlay drawn from {@link spaceProjector} sits
// exactly on them.
//
// The camera itself (its limits and default) is state, in state/model.ts,
// whose unit tests run in plain Node; it is re-exported here. Node's unit
// tests import this module too, so it imports no three.js.

import type { FixedPointOut, TrajectoryOut } from "../api/types";
import type { Camera, InitialCondition, SpaceView } from "../state/model";
import { drawFixedPoint, fixedPointGlyph, type DrawContext } from "./draw";
import { DEFAULT_FIGURE_FONTS } from "./figure";
import { drawMath, layoutMath, mathRuns } from "./mathText";
import type { PlotTheme } from "./theme";
import { niceTicks, tickLabel } from "./viewport";

export { clampCamera, DEFAULT_CAMERA, PITCH_LIMIT, ZOOM_MAX, ZOOM_MIN, type Camera } from "../state/model";

export type Vec3 = [number, number, number];

/** What the projection needs of a 3D view. */
export type SpaceBox = Pick<SpaceView, "xAxis" | "yAxis" | "zAxis" | "xRange" | "yRange" | "zRange" | "camera">;

/** The camera's axes in box coordinates and the screen it draws on. */
export interface SpaceFrame {
  right: Vec3;
  up: Vec3;
  /** From the box centre towards the viewer. */
  toward: Vec3;
  /** Pixels per box unit. */
  scale: number;
  w: number;
  h: number;
}

/** At zoom 1 a box unit is this fraction of the plot's shorter side, so the box (diagonal 2√3) fits at any angle. */
export const BOX_FILL = 0.27;

export function spaceFrame(c: Camera, w: number, h: number): SpaceFrame {
  const cy = Math.cos(c.yaw);
  const sy = Math.sin(c.yaw);
  const cp = Math.cos(c.pitch);
  const sp = Math.sin(c.pitch);
  // A larger yaw carries the side of the box facing the viewer to the right.
  return {
    right: [cy, -sy, 0],
    up: [sp * sy, sp * cy, cp],
    toward: [-sy * cp, -cy * cp, sp],
    scale: c.zoom * BOX_FILL * Math.min(w, h),
    w,
    h,
  };
}

/** The state indices on the box's three axes. */
export function spaceAxes(v: Pick<SpaceView, "xAxis" | "yAxis" | "zAxis">): Vec3 {
  const at = (a: SpaceView["xAxis"]) => (a.kind === "state" ? a.index : 0);
  return [at(v.xAxis), at(v.yAxis), at(v.zAxis)];
}

/** A state vector's box coordinates. */
export function boxCoords(v: SpaceBox): (p: readonly number[]) => Vec3 {
  const [i, j, k] = spaceAxes(v);
  const mid = (r: [number, number]) => (r[0] + r[1]) / 2;
  const half = (r: [number, number]) => (r[1] - r[0]) / 2;
  const [mx, my, mz] = [mid(v.xRange), mid(v.yRange), mid(v.zRange)];
  const [hx, hy, hz] = [half(v.xRange), half(v.yRange), half(v.zRange)];
  return (p) => [(p[i] - mx) / hx, (p[j] - my) / hy, (p[k] - mz) / hz];
}

const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

/**
 * A box point on screen: CSS px from the plot's top left, and its depth as
 * three.js's normalised device z (−1 nearest, +1 farthest; the box spans
 * about ±0.87).
 */
export function projectBox(f: SpaceFrame, q: Vec3): [sx: number, sy: number, depth: number] {
  return [f.w / 2 + f.scale * dot(f.right, q), f.h / 2 - f.scale * dot(f.up, q), -dot(f.toward, q) / 2];
}

/** The orthographic projection of a state vector in the view `v` on a w × h plot. */
export function spaceProjector(
  v: SpaceBox,
  w: number,
  h: number,
): (p: readonly number[]) => [sx: number, sy: number, depth: number] {
  const box = boxCoords(v);
  const f = spaceFrame(v.camera, w, h);
  return (p) => projectBox(f, box(p));
}

/** How far the three.js camera sits from the box centre; the near and far planes are 2 either side of it. */
export const CAMERA_DISTANCE = 4;

/**
 * The settings of a three.js OrthographicCamera that projects box
 * coordinates exactly as {@link projectBox} does: set `position` and `up`,
 * look at the origin, then set the frustum and update its projection.
 */
export function threeCamera(c: Camera, w: number, h: number) {
  const f = spaceFrame(c, w, h);
  const d = CAMERA_DISTANCE;
  return {
    position: [f.toward[0] * d, f.toward[1] * d, f.toward[2] * d] as Vec3,
    up: [0, 0, 1] as Vec3,
    left: -w / (2 * f.scale),
    right: w / (2 * f.scale),
    top: h / (2 * f.scale),
    bottom: -h / (2 * f.scale),
    near: d - 2,
    far: d + 2,
  };
}

/** A trajectory as the 3D view draws it. */
export interface SpaceTrajectory {
  out: TrajectoryOut;
  color: number;
  backward: boolean;
  selected: boolean;
}

/** Line width of a trajectory at line scale 1, CSS px (2.25 when selected), as in the phase plane. */
export const trajectoryWidth = (t: Pick<SpaceTrajectory, "selected">, th: Pick<PlotTheme, "lineScale">) =>
  (t.selected ? 2.25 : 1.5) * th.lineScale;

/**
 * Box coordinates of a trajectory's samples, x y z after each other, up to
 * the first sample that isn't finite (what three.js draws).
 */
export function trajectoryBoxCoords(v: SpaceBox, out: TrajectoryOut, dim: number): Float64Array {
  const box = boxCoords(v);
  const n = Math.min(out.t.length, Math.floor(out.x.length / Math.max(1, dim)));
  const pts = new Float64Array(n * 3);
  const p = new Array<number>(dim);
  let m = 0;
  for (let k = 0; k < n; k++) {
    for (let i = 0; i < dim; i++) p[i] = out.x[k * dim + i];
    const q = box(p);
    if (!Number.isFinite(q[0]) || !Number.isFinite(q[1]) || !Number.isFinite(q[2])) break;
    pts[m++] = q[0];
    pts[m++] = q[1];
    pts[m++] = q[2];
  }
  return pts.subarray(0, m);
}

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);

/** The trajectories projected and stroked in their traj-n colours (the drawing without WebGL). */
export function drawSpaceLines(
  ctx: DrawContext,
  v: SpaceBox,
  w: number,
  h: number,
  trajectories: readonly SpaceTrajectory[],
  dim: number,
  th: PlotTheme,
): void {
  const f = spaceFrame(v.camera, w, h);
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  for (const tr of trajectories) {
    const q = trajectoryBoxCoords(v, tr.out, dim);
    if (q.length < 6) continue;
    ctx.globalAlpha = tr.backward ? 0.5 : 1;
    ctx.strokeStyle = th.traj[tr.color % th.traj.length];
    ctx.lineWidth = trajectoryWidth(tr, th);
    ctx.beginPath();
    for (let k = 0; k < q.length; k += 3) {
      const [x, y] = projectBox(f, [q[k], q[k + 1], q[k + 2]]);
      if (k === 0) ctx.moveTo(clampPx(x), clampPx(y));
      else ctx.lineTo(clampPx(x), clampPx(y));
    }
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
}

/** What the overlay draws besides the box: names, fixed points, start points. */
export interface SpaceMarks {
  dim: number;
  /** The three axis names, in LaTeX. */
  names: [string, string, string];
  fixedPoints: readonly FixedPointOut[];
  ics: readonly InitialCondition[];
  selectedFp?: number | null;
  selectedIc?: number | null;
}

/** The 12 edges of the box: for each axis k, the four edges along it, by the other two coordinates. */
function edge(k: number, a: number, b: number): [Vec3, Vec3] {
  const lo: Vec3 = [0, 0, 0];
  const hi: Vec3 = [0, 0, 0];
  const others = [0, 1, 2].filter((i) => i !== k);
  lo[k] = -1;
  hi[k] = 1;
  lo[others[0]] = hi[others[0]] = a;
  lo[others[1]] = hi[others[1]] = b;
  return [lo, hi];
}

const SIGNS: [number, number][] = [
  [-1, -1],
  [1, -1],
  [-1, 1],
  [1, 1],
];

/**
 * The edge that carries axis k's ticks and name: for the first two axes the
 * floor edge nearest the viewer, for the third the vertical edge furthest
 * left on screen.
 */
function labelEdge(f: SpaceFrame, k: number): [Vec3, Vec3] {
  // For the first two axes the second free coordinate is the third axis: b = −1 is the floor.
  const candidates = SIGNS.filter(([, b]) => k === 2 || b === -1).map(([a, b]) => edge(k, a, b));
  const mid = (e: [Vec3, Vec3]): Vec3 => [(e[0][0] + e[1][0]) / 2, (e[0][1] + e[1][1]) / 2, (e[0][2] + e[1][2]) / 2];
  const score = (e: [Vec3, Vec3]) => (k === 2 ? projectBox(f, mid(e))[0] : projectBox(f, mid(e))[2]);
  return candidates.reduce((best, e) => (score(e) < score(best) ? e : best));
}

/** Tick length at glyph scale 1, and the gap from a tick to its label, px. */
const TICK = 4;
const LABEL_GAP = 12;
const NAME_GAP = 34;

/**
 * The axis box with its edges, ticks and tick labels, the three axis names,
 * the fixed points (Strogatz's glyphs, farthest first) and the start points.
 * Draws no background, so it can lie over the WebGL lines.
 */
export function drawSpaceOverlay(
  ctx: DrawContext,
  v: SpaceBox,
  w: number,
  h: number,
  marks: SpaceMarks,
  th: PlotTheme,
): void {
  const f = spaceFrame(v.camera, w, h);
  const ranges = [v.xRange, v.yRange, v.zRange];
  const centre = projectBox(f, [0, 0, 0]);
  ctx.globalAlpha = 1;
  ctx.setLineDash([]);
  ctx.lineCap = "butt";

  ctx.strokeStyle = th.grid;
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let k = 0; k < 3; k++) {
    for (const [a, b] of SIGNS) {
      const [p, q] = edge(k, a, b).map((c) => projectBox(f, c));
      ctx.moveTo(p[0], p[1]);
      ctx.lineTo(q[0], q[1]);
    }
  }
  ctx.stroke();

  const tick = TICK * th.glyphScale;
  for (let k = 0; k < 3; k++) {
    const [lo, hi] = labelEdge(f, k);
    const p0 = projectBox(f, lo);
    const p1 = projectBox(f, hi);
    const mx = (p0[0] + p1[0]) / 2;
    const my = (p0[1] + p1[1]) / 2;
    let ox = mx - centre[0];
    let oy = my - centre[1];
    const len = Math.hypot(ox, oy);
    [ox, oy] = len > 1 ? [ox / len, oy / len] : [0, 1];

    ctx.strokeStyle = th.axis;
    ctx.lineWidth = th.lineScale;
    ctx.beginPath();
    ctx.moveTo(p0[0], p0[1]);
    ctx.lineTo(p1[0], p1[1]);
    const [rlo, rhi] = ranges[k];
    const edgePx = Math.hypot(p1[0] - p0[0], p1[1] - p0[1]);
    const { ticks, step } = niceTicks(rlo, rhi, Math.max(2, Math.min(6, Math.round(edgePx / 70))));
    const at = (t: number): [number, number] => {
      const s = (t - rlo) / (rhi - rlo);
      return [p0[0] + s * (p1[0] - p0[0]), p0[1] + s * (p1[1] - p0[1])];
    };
    for (const t of ticks) {
      const [x, y] = at(t);
      ctx.moveTo(x, y);
      ctx.lineTo(x + ox * tick, y + oy * tick);
    }
    ctx.stroke();

    ctx.font = th.tickFont;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.lineJoin = "round";
    for (const t of ticks) {
      const [x, y] = at(t);
      const text = tickLabel(t, step);
      const lx = x + ox * (tick + LABEL_GAP);
      const ly = y + oy * (tick + LABEL_GAP);
      ctx.strokeStyle = th.halo;
      ctx.lineWidth = 3;
      ctx.strokeText(text, lx, ly);
      ctx.fillStyle = th.tick;
      ctx.fillText(text, lx, ly);
    }

    const fonts = DEFAULT_FIGURE_FONTS;
    const layout = layoutMath(ctx, mathRuns(marks.names[k]), fonts.mathFamily, fonts.mathSize);
    const nx = mx + ox * (tick + NAME_GAP);
    const ny = my + oy * (tick + NAME_GAP);
    drawMath(ctx, layout, nx - layout.width / 2, ny + layout.size * 0.35, th.tick, th.halo);
  }

  const box = boxCoords(v);
  const project = (p: readonly number[]) => projectBox(f, box(p));
  const fps = marks.fixedPoints
    .map((fp, i) => ({ fp, i, at: project(fp.x) }))
    .filter(({ fp, at }) => fp.x.length === marks.dim && at.every(Number.isFinite))
    .sort((a, b) => b.at[2] - a.at[2]);
  for (const { fp, i, at } of fps) {
    drawFixedPoint(ctx, at[0], at[1], fixedPointGlyph(fp.classification, fp.stable), th, {
      selected: marks.selectedFp === i,
    });
  }

  const g = th.glyphScale;
  for (const ic of marks.ics) {
    if (ic.x0.length !== marks.dim) continue;
    const [x, y] = project(ic.x0);
    if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
    const color = th.traj[ic.color % th.traj.length];
    const r = (marks.selectedIc === ic.id ? 4.5 : 3.5) * g;
    ctx.beginPath();
    ctx.arc(x, y, r, 0, 2 * Math.PI);
    ctx.lineWidth = 1.5 * g;
    ctx.strokeStyle = ic.hidden ? color : th.halo;
    if (ic.hidden) {
      ctx.stroke();
      continue;
    }
    ctx.fillStyle = color;
    ctx.fill();
    ctx.stroke();
  }
}
