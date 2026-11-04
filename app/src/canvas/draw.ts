import type {
  Arrow,
  AxisRef,
  Classification,
  FixedPointOut,
  NullclineOut,
  SystemKind,
  TrajectoryOut,
} from "../api/types";
import type { InitialCondition, TrajectoryMeta } from "../state/model";
import { eigenDirections } from "./eigenvectors";
import type { PlotTheme } from "./theme";
import { niceTicks, sx, sy, tickLabel, type Viewport } from "./viewport";

export type Box = [number, number, number, number];

export interface SceneTrajectory {
  out: TrajectoryOut;
  meta: TrajectoryMeta;
  color: number;
}

export interface Scene {
  kind: SystemKind;
  dim: number;
  xAxis: AxisRef;
  yAxis: AxisRef;
  arrows: Arrow[];
  fieldGrid: number;
  nullclines: NullclineOut[];
  fixedPoints: FixedPointOut[];
  trajectories: SceneTrajectory[];
  ics: InitialCondition[];
  selectedIc: number | null;
  selectedFp: number | null;
  hoverIc: number | null;
  hoverFp: number | null;
  eigenvectors: boolean;
  stale: boolean;
  avoid: { labels: Box[]; field: Box[] };
}

export type SceneAxes = Pick<Scene, "kind" | "dim" | "xAxis" | "yAxis">;

export type FixedPointGlyphKind = "stable" | "unstable" | "half";

export function fixedPointGlyph(c: Classification, stable: boolean): FixedPointGlyphKind {
  if (c === "center" || c === "non-hyperbolic" || c === "non-isolated") return "half";
  return stable ? "stable" : "unstable";
}

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);
const TAU = Math.PI * 2;

export function fixedPointPx(vp: Viewport, scene: SceneAxes, fp: FixedPointOut): [number, number] {
  const px = scene.xAxis.kind === "time" ? vp.left + 12 : sx(vp, fp.x[scene.xAxis.index]);
  const py = scene.yAxis.kind === "time" ? vp.top + vp.height - 12 : sy(vp, fp.x[scene.yAxis.index]);
  return [px, py];
}

export { eigenDirections, type EigenDirection } from "./eigenvectors";

export function icPx(vp: Viewport, scene: SceneAxes, ic: InitialCondition): [number, number] {
  const t0 = scene.kind === "map" ? 0 : ic.t0;
  const px = sx(vp, scene.xAxis.kind === "time" ? t0 : ic.x0[scene.xAxis.index]);
  const py = sy(vp, scene.yAxis.kind === "time" ? t0 : ic.x0[scene.yAxis.index]);
  return [px, py];
}

export function trajectoryPx(vp: Viewport, scene: SceneAxes, out: TrajectoryOut): Float64Array {
  const n = out.t.length;
  const pts = new Float64Array(n * 2);
  let m = 0;
  for (let k = 0; k < n; k++) {
    const a = scene.xAxis.kind === "time" ? out.t[k] : out.x[k * scene.dim + scene.xAxis.index];
    const b = scene.yAxis.kind === "time" ? out.t[k] : out.x[k * scene.dim + scene.yAxis.index];
    if (!Number.isFinite(a) || !Number.isFinite(b)) break;
    pts[m++] = clampPx(sx(vp, a));
    pts[m++] = clampPx(sy(vp, b));
  }
  return pts.subarray(0, m);
}

export function axisLines(vp: Viewport): { X: number; Y: number; xAxisInView: boolean; yAxisInView: boolean } {
  const right = vp.left + vp.width;
  const bottom = vp.top + vp.height;
  const [x0, x1] = vp.xRange;
  const [y0, y1] = vp.yRange;
  const yAxisInView = Math.min(x0, x1) < 0 && Math.max(x0, x1) > 0;
  const xAxisInView = Math.min(y0, y1) < 0 && Math.max(y0, y1) > 0;
  const X = yAxisInView ? Math.round(sx(vp, 0)) + 0.5 : sx(vp, 0) < vp.left ? vp.left : right;
  const Y = xAxisInView ? Math.round(sy(vp, 0)) + 0.5 : sy(vp, 0) > bottom ? bottom : vp.top;
  return { X, Y, xAxisInView, yAxisInView };
}

function ticksFor(vp: Viewport) {
  return {
    x: niceTicks(vp.xRange[0], vp.xRange[1], Math.max(2, Math.round(vp.width / 140))),
    y: niceTicks(vp.yRange[0], vp.yRange[1], Math.max(2, Math.round(vp.height / 120))),
  };
}

interface Label {
  text: string;
  x: number;
  y: number;
  align: CanvasTextAlign;
  base: CanvasTextBaseline;
  box: Box;
}

export function drawScene(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  vp: Viewport,
  scene: Scene | null,
  th: PlotTheme,
): void {
  ctx.globalAlpha = 1;
  ctx.fillStyle = th.bg;
  ctx.fillRect(0, 0, w, h);

  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();

  const ticks = ticksFor(vp);
  drawGrid(ctx, vp, ticks, th);
  const axes = axisLines(vp);
  drawAxes(ctx, vp, ticks, axes, th);
  if (!scene) {
    drawLabels(ctx, layoutLabels(ctx, vp, ticks, axes, th, []), th);
    ctx.restore();
    return;
  }

  const labels = layoutLabels(ctx, vp, ticks, axes, th, scene.avoid.labels, {
    yRight: scene.xAxis.kind === "time",
    xAbove: scene.yAxis.kind === "time",
  });
  const fade = scene.stale ? 0.55 : 1;
  ctx.globalAlpha = fade;
  drawField(ctx, vp, scene, th, [...labels.map((l) => l.box), ...scene.avoid.field]);
  drawNullclines(ctx, vp, scene, th);
  drawTrajectories(ctx, vp, scene, th, fade);
  ctx.globalAlpha = fade;
  const eigenLabels = scene.eigenvectors
    ? drawEigenvectors(ctx, vp, scene, th, [...labels.map((l) => l.box), ...scene.avoid.labels])
    : [];
  ctx.globalAlpha = fade;
  const order = scene.fixedPoints.map((_, i) => i).filter((i) => i !== scene.selectedFp);
  if (scene.selectedFp !== null && scene.selectedFp < scene.fixedPoints.length) order.push(scene.selectedFp);
  for (const i of order) {
    const fp = scene.fixedPoints[i];
    const [px, py] = fixedPointPx(vp, scene, fp);
    drawFixedPoint(ctx, px, py, fixedPointGlyph(fp.classification, fp.stable), th, {
      hover: scene.hoverFp === i,
      selected: scene.selectedFp === i,
    });
  }
  ctx.globalAlpha = 1;
  drawLabels(ctx, labels, th);
  ctx.globalAlpha = fade;
  drawLabels(ctx, eigenLabels, th);
  ctx.globalAlpha = 1;
  drawHandles(ctx, vp, scene, th);
  ctx.restore();
}

function drawGrid(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  ticks: ReturnType<typeof ticksFor>,
  th: PlotTheme,
) {
  const bottom = vp.top + vp.height;
  const right = vp.left + vp.width;
  const half = (lo: number, hi: number, step: number) => {
    const out: number[] = [];
    const s = step / 2;
    const a = Math.min(lo, hi);
    const b = Math.max(lo, hi);
    const first = Math.ceil(a / s);
    const count = Math.floor(b / s + 1e-9) - first;
    for (let k = 0; k <= count; k++) out.push((first + k) * s);
    return out;
  };
  ctx.lineWidth = 1;
  ctx.strokeStyle = th.grid;
  ctx.beginPath();
  for (const v of half(vp.xRange[0], vp.xRange[1], ticks.x.step)) {
    const x = Math.round(sx(vp, v)) + 0.5;
    ctx.moveTo(x, vp.top);
    ctx.lineTo(x, bottom);
  }
  for (const v of half(vp.yRange[0], vp.yRange[1], ticks.y.step)) {
    const y = Math.round(sy(vp, v)) + 0.5;
    ctx.moveTo(vp.left, y);
    ctx.lineTo(right, y);
  }
  ctx.stroke();
}

/** Axis tick length at glyph scale 1. */
const TICK_LEN = 4;
/** Space between a tick and its label. */
const TICK_LABEL_PAD = 3;
/** Radius of the halo behind a fixed-point glyph at glyph scale 1. */
const FP_HALO_R = 7.5;

function drawAxes(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  ticks: ReturnType<typeof ticksFor>,
  axes: ReturnType<typeof axisLines>,
  th: PlotTheme,
) {
  const s = th.lineScale;
  const tick = TICK_LEN * th.glyphScale;
  ctx.strokeStyle = th.axis;
  ctx.lineWidth = s;
  ctx.lineCap = "butt";
  ctx.beginPath();
  if (axes.yAxisInView) {
    ctx.moveTo(axes.X, vp.top);
    ctx.lineTo(axes.X, vp.top + vp.height);
    for (const v of ticks.y.ticks) {
      if (v === 0) continue;
      const y = Math.round(sy(vp, v)) + 0.5;
      ctx.moveTo(axes.X, y);
      ctx.lineTo(axes.X - tick, y);
    }
  }
  if (axes.xAxisInView) {
    ctx.moveTo(vp.left, axes.Y);
    ctx.lineTo(vp.left + vp.width, axes.Y);
    for (const v of ticks.x.ticks) {
      if (v === 0) continue;
      const x = Math.round(sx(vp, v)) + 0.5;
      ctx.moveTo(x, axes.Y);
      ctx.lineTo(x, axes.Y + tick);
    }
  }
  ctx.stroke();
}

const overlaps = (a: Box, b: Box) => a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1];

function layoutLabels(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  ticks: ReturnType<typeof ticksFor>,
  axes: ReturnType<typeof axisLines>,
  th: PlotTheme,
  avoid: Box[],
  prefer: { yRight?: boolean; xAbove?: boolean } = {},
): Label[] {
  const g = th.glyphScale;
  const gap = Math.max(TICK_LEN * g + TICK_LABEL_PAD, FP_HALO_R * g + 2);
  const right = vp.left + vp.width;
  const bottom = vp.top + vp.height;
  const hgt = th.tickSize;
  ctx.font = th.tickFont;
  const both = axes.xAxisInView && axes.yAxisInView;
  const out: Label[] = [];
  const make = (text: string, x: number, y: number, align: CanvasTextAlign, base: CanvasTextBaseline) =>
    makeLabel(ctx, hgt, text, x, y, align, base);
  const keep = (lab: Label) => {
    const [l, t, r, b] = lab.box;
    if (l < vp.left || r > right || t < vp.top || b > bottom) return;
    if (avoid.some((a) => overlaps(lab.box, a))) return;
    if (out.some((o) => overlaps(lab.box, o.box))) return;
    out.push(lab);
  };

  const xBelow = prefer.xAbove ? axes.Y - gap - hgt - 3 < vp.top : axes.Y + gap + hgt + 3 <= bottom;
  const xy = xBelow ? axes.Y + gap : axes.Y - gap;
  for (const v of ticks.x.ticks) {
    if (v === 0 && both) continue;
    keep(make(tickLabel(v, ticks.x.step), sx(vp, v), xy, "center", xBelow ? "top" : "bottom"));
  }
  const yWidest = Math.max(0, ...ticks.y.ticks.map((v) => ctx.measureText(tickLabel(v, ticks.y.step)).width));
  const yLeft = prefer.yRight ? axes.X + gap + yWidest + 3 > right : axes.X - gap - yWidest - 3 >= vp.left;
  const yx = yLeft ? axes.X - gap : axes.X + gap;
  for (const v of ticks.y.ticks) {
    if (v === 0 && both) continue;
    keep(make(tickLabel(v, ticks.y.step), yx, sy(vp, v), yLeft ? "right" : "left", "middle"));
  }
  if (both) {
    keep(
      make(
        "0",
        yLeft ? axes.X - gap : axes.X + gap,
        xBelow ? axes.Y + gap : axes.Y - gap,
        yLeft ? "right" : "left",
        xBelow ? "top" : "bottom",
      ),
    );
  }
  return out;
}

function makeLabel(
  ctx: CanvasRenderingContext2D,
  hgt: number,
  text: string,
  x: number,
  y: number,
  align: CanvasTextAlign,
  base: CanvasTextBaseline,
): Label {
  const wid = ctx.measureText(text).width;
  const l = align === "center" ? x - wid / 2 : align === "right" ? x - wid : x;
  const t = base === "top" ? y : base === "middle" ? y - hgt / 2 : y - hgt;
  return { text, x, y, align, base, box: [l - 3, t - 3, l + wid + 3, t + hgt + 3] };
}

function drawLabels(ctx: CanvasRenderingContext2D, labels: Label[], th: PlotTheme) {
  ctx.font = th.tickFont;
  ctx.lineJoin = "round";
  ctx.lineWidth = 3;
  ctx.strokeStyle = th.halo;
  ctx.fillStyle = th.tick;
  for (const l of labels) {
    ctx.textAlign = l.align;
    ctx.textBaseline = l.base;
    ctx.strokeText(l.text, l.x, l.y);
    ctx.fillText(l.text, l.x, l.y);
  }
}

function drawField(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme, clear: Box[]) {
  if (!scene.arrows.length || scene.fieldGrid <= 0) return;
  const s = th.lineScale;
  const cell = Math.min(vp.width, vp.height) / scene.fieldGrid;
  const len = 0.6 * cell;
  const head = 0.25 * len;
  const spread = Math.tan(Math.PI / 7);
  const r = len / 2 + s;
  const sxu = vp.width / (vp.xRange[1] - vp.xRange[0]);
  const syu = vp.height / (vp.yRange[1] - vp.yRange[0]);
  ctx.strokeStyle = th.field;
  ctx.lineWidth = s;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.beginPath();
  for (const a of scene.arrows) {
    const dx = a.u * sxu;
    const dy = -a.v * syu;
    const m = Math.hypot(dx, dy);
    if (!Number.isFinite(m) || m < 1e-12) continue;
    const cx = sx(vp, a.x);
    const cy = sy(vp, a.y);
    const box: Box = [cx - r, cy - r, cx + r, cy + r];
    if (clear.some((c) => overlaps(box, c))) continue;
    const ux = dx / m;
    const uy = dy / m;
    const tx = cx + (ux * len) / 2;
    const ty = cy + (uy * len) / 2;
    ctx.moveTo(cx - (ux * len) / 2, cy - (uy * len) / 2);
    ctx.lineTo(tx, ty);
    ctx.moveTo(tx - ux * head - uy * head * spread, ty - uy * head + ux * head * spread);
    ctx.lineTo(tx, ty);
    ctx.lineTo(tx - ux * head + uy * head * spread, ty - uy * head - ux * head * spread);
  }
  ctx.stroke();
}

function drawNullclines(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme) {
  const s = th.lineScale;
  ctx.lineWidth = 1.5 * s;
  ctx.setLineDash([6 * s, 5 * s]);
  ctx.lineCap = "butt";
  ctx.lineJoin = "round";
  for (const nc of scene.nullclines) {
    ctx.strokeStyle = th.nullcline[nc.state % th.nullcline.length];
    ctx.beginPath();
    for (const line of nc.polylines) {
      line.forEach(([x, y], i) => {
        const px = clampPx(sx(vp, x));
        const py = clampPx(sy(vp, y));
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      });
    }
    ctx.stroke();
  }
  ctx.setLineDash([]);
}

function drawTrajectories(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  scene: Scene,
  th: PlotTheme,
  fade: number,
) {
  const s = th.lineScale;
  const g = th.glyphScale;
  const rank = (t: SceneTrajectory) =>
    (scene.selectedIc === t.meta.icId ? 2 : 0) + (t.meta.direction === "forward" ? 1 : 0);
  const ordered = [...scene.trajectories].sort((a, b) => rank(a) - rank(b));
  const isMap = scene.kind === "map";
  const timeSeries = scene.xAxis.kind === "time" || scene.yAxis.kind === "time";
  const dense = scene.dim >= 3;
  const placed: number[] = [];

  for (const tr of ordered) {
    const pts = trajectoryPx(vp, scene, tr.out);
    if (pts.length < 2) continue;
    const color = th.traj[tr.color % th.traj.length];
    const selected = scene.selectedIc === tr.meta.icId;
    const backward = tr.meta.direction === "backward";
    ctx.strokeStyle = color;
    ctx.fillStyle = color;

    if (isMap) {
      const n = pts.length / 2;
      if (timeSeries) {
        ctx.globalAlpha = fade * 0.4;
        ctx.lineWidth = s;
        ctx.lineJoin = "round";
        strokePolyline(ctx, pts);
      }
      ctx.globalAlpha = fade;
      const r = (n > 2000 ? 0.75 : n > 300 ? 1.3 : selected ? 3 : 2.6) * g;
      if (n > 2000) {
        for (let k = 0; k < pts.length; k += 2) ctx.fillRect(pts[k] - r, pts[k + 1] - r, 2 * r, 2 * r);
      } else {
        ctx.beginPath();
        for (let k = 0; k < pts.length; k += 2) {
          ctx.moveTo(pts[k] + r, pts[k + 1]);
          ctx.arc(pts[k], pts[k + 1], r, 0, TAU);
        }
        ctx.fill();
      }
      continue;
    }

    ctx.globalAlpha = fade * (backward ? 0.5 : 1);
    ctx.lineWidth = (selected ? 2.25 : 1.5) * s;
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    strokePolyline(ctx, pts);
    drawHeads(ctx, vp, pts, backward, (selected ? 9 : 8) * g, dense ? 1 : 12, placed, 24 * g);

    if (tr.out.status !== "completed") {
      const x = pts[pts.length - 2];
      const y = pts[pts.length - 1];
      const c = 4 * g;
      ctx.globalAlpha = fade;
      ctx.lineWidth = 1.5 * s;
      ctx.beginPath();
      ctx.moveTo(x - c, y - c);
      ctx.lineTo(x + c, y + c);
      ctx.moveTo(x + c, y - c);
      ctx.lineTo(x - c, y + c);
      ctx.stroke();
    }
  }
  ctx.globalAlpha = 1;
}

function strokePolyline(ctx: CanvasRenderingContext2D, pts: Float64Array) {
  ctx.beginPath();
  ctx.moveTo(pts[0], pts[1]);
  for (let k = 2; k < pts.length; k += 2) ctx.lineTo(pts[k], pts[k + 1]);
  ctx.stroke();
}

/** Pixels of trajectory per arrow head once a trajectory is long enough for several. */
const HEAD_SPACING_PX = 300;
/** Trajectories shorter than this (in view) get a single head. */
const SINGLE_HEAD_MAX_PX = 600;
/** Half-length of the stretch over which a head's bend is measured. */
const BEND_WINDOW_PX = 12;
/** A spot must be straighter by this many radians to move a head off the middle. */
const STRAIGHTNESS_TOL = 0.05;
/** A head may slide this fraction of its share of the trajectory either way. */
const HEAD_SLIDE_FRACTION = 1 / 6;

function drawHeads(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  pts: Float64Array,
  backward: boolean,
  L: number,
  maxHeads: number,
  placed: number[],
  minGap: number,
) {
  const n = pts.length / 2;
  if (n < 2) return;
  const right = vp.left + vp.width;
  const bottom = vp.top + vp.height;
  const acc = new Float64Array(n);
  let total = 0;
  for (let k = 1; k < n; k++) {
    const x = pts[2 * k];
    const y = pts[2 * k + 1];
    if (x >= vp.left && x <= right && y >= vp.top && y <= bottom) {
      total += Math.hypot(x - pts[2 * k - 2], y - pts[2 * k - 1]);
    }
    acc[k] = total;
  }
  if (total < 3 * L) return;
  const at = (t: number) => {
    let lo = 0;
    let hi = n - 1;
    if (t <= 0) return 0;
    if (t >= total) return n - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (acc[mid] < t) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  };
  const turn = (k: number) => {
    const a = at(acc[k] - BEND_WINDOW_PX);
    const b = at(acc[k] + BEND_WINDOW_PX);
    if (a >= k || b <= k) return Infinity;
    const t1 = Math.atan2(pts[2 * k + 1] - pts[2 * a + 1], pts[2 * k] - pts[2 * a]);
    const t2 = Math.atan2(pts[2 * b + 1] - pts[2 * k + 1], pts[2 * b] - pts[2 * k]);
    return Math.abs(Math.atan2(Math.sin(t2 - t1), Math.cos(t2 - t1)));
  };
  const count = Math.min(maxHeads, total > SINGLE_HEAD_MAX_PX ? Math.floor(total / HEAD_SPACING_PX) : 1);
  const stretch = total / count;
  const gap2 = minGap * minGap;
  const hw = 0.45 * L;
  ctx.beginPath();
  for (let h = 0; h < count; h++) {
    const mid = (h + 0.5) * stretch;
    let k = at(mid);
    let best = turn(k);
    const slide = stretch * HEAD_SLIDE_FRACTION;
    for (let t = mid - slide; t <= mid + slide; t += 4) {
      const c = at(t);
      const v = turn(c) + 1e-3 * Math.abs(t - mid);
      if (v < best - STRAIGHTNESS_TOL) {
        best = v;
        k = c;
      }
    }
    if (k < 1) continue;
    const x = pts[2 * k];
    const y = pts[2 * k + 1];
    if (x < vp.left || x > right || y < vp.top || y > bottom) continue;
    let crowded = false;
    for (let j = 0; j < placed.length && !crowded; j += 2) {
      crowded = (placed[j] - x) ** 2 + (placed[j + 1] - y) ** 2 < gap2;
    }
    if (crowded) continue;
    let j = k - 1;
    while (j > 0 && Math.hypot(x - pts[2 * j], y - pts[2 * j + 1]) < 1) j--;
    const dx = x - pts[2 * j];
    const dy = y - pts[2 * j + 1];
    const m = Math.hypot(dx, dy);
    if (m === 0) continue;
    placed.push(x, y);
    const sign = backward ? -1 : 1;
    const ux = (sign * dx) / m;
    const uy = (sign * dy) / m;
    ctx.moveTo(x + (ux * L) / 2, y + (uy * L) / 2);
    ctx.lineTo(x - (ux * L) / 2 - uy * hw, y - (uy * L) / 2 + ux * hw);
    ctx.lineTo(x - (ux * L) / 2 + uy * hw, y - (uy * L) / 2 - ux * hw);
    ctx.closePath();
  }
  ctx.fill();
}

/** Half-length of an eigenvector line, as a fraction of the plot diagonal. */
const EIGEN_HALF_LEN_FRAC = 0.06;
const HEAD_HALF_ANGLE = Math.PI / 5;
/** A direction with |x| below this is labelled above or below rather than beside. */
const LABEL_SIDE_MIN_X = 0.3;
/** Direction components beyond this anchor the label on that side. */
const LABEL_ANCHOR_THRESHOLD = 0.38;

function drawEigenvectors(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  scene: Scene,
  th: PlotTheme,
  clear: Box[],
): Label[] {
  const s = th.lineScale;
  const g = th.glyphScale;
  const half = Math.max(20 * g, EIGEN_HALF_LEN_FRAC * Math.hypot(vp.width, vp.height));
  const head = 6 * g;
  const spread = Math.tan(HEAD_HALF_ANGLE);
  const sxu = vp.width / (vp.xRange[1] - vp.xRange[0]);
  const syu = vp.height / (vp.yRange[1] - vp.yRange[0]);
  const right = vp.left + vp.width;
  const bottom = vp.top + vp.height;
  const labels: Label[] = [];
  ctx.font = th.tickFont;
  const path = new Path2D();
  const vee = (x: number, y: number, ux: number, uy: number) => {
    path.moveTo(x - ux * head - uy * head * spread, y - uy * head + ux * head * spread);
    path.lineTo(x, y);
    path.lineTo(x - ux * head + uy * head * spread, y - uy * head - ux * head * spread);
  };
  scene.fixedPoints.forEach((fp, i) => {
    const [cx, cy] = fixedPointPx(vp, scene, fp);
    for (const e of eigenDirections(scene, fp)) {
      const dx = e.p[0] * sxu;
      const dy = -e.p[1] * syu;
      const m = Math.hypot(dx, dy);
      if (!Number.isFinite(m) || m === 0) continue;
      const ux = dx / m;
      const uy = dy / m;
      path.moveTo(cx - ux * half, cy - uy * half);
      path.lineTo(cx + ux * half, cy + uy * half);
      for (const sg of [1, -1]) {
        if (e.flow > 0) vee(cx + sg * ux * half, cy + sg * uy * half, sg * ux, sg * uy);
        else if (e.flow < 0) vee(cx + sg * ux * half * 0.5, cy + sg * uy * half * 0.5, -sg * ux, -sg * uy);
      }
      if (scene.selectedFp !== i) continue;
      const sg = Math.abs(ux) > LABEL_SIDE_MIN_X ? Math.sign(ux) : -Math.sign(uy) || 1;
      const ex = sg * ux;
      const ey = sg * uy;
      const off = half + 4 * g + 2;
      const lab = makeLabel(
        ctx,
        th.tickSize,
        e.label,
        cx + ex * off,
        cy + ey * off,
        ex > LABEL_ANCHOR_THRESHOLD ? "left" : ex < -LABEL_ANCHOR_THRESHOLD ? "right" : "center",
        ey > LABEL_ANCHOR_THRESHOLD ? "top" : ey < -LABEL_ANCHOR_THRESHOLD ? "bottom" : "middle",
      );
      const [l, t, r, b] = lab.box;
      if (l < vp.left || r > right || t < vp.top || b > bottom) continue;
      if ([...clear, ...labels.map((o) => o.box)].some((c) => overlaps(lab.box, c))) continue;
      labels.push(lab);
    }
  });
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.setLineDash([]);
  ctx.strokeStyle = th.halo;
  ctx.lineWidth = s + 3;
  ctx.stroke(path);
  ctx.strokeStyle = th.fp;
  ctx.lineWidth = s;
  ctx.stroke(path);
  return labels;
}

export function drawFixedPoint(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  kind: FixedPointGlyphKind,
  th: PlotTheme,
  state: { hover?: boolean; selected?: boolean } = {},
) {
  const g = th.glyphScale * (state.hover ? 1.2 : 1);
  if (state.selected) {
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = 2 * th.glyphScale;
    ctx.beginPath();
    ctx.arc(x, y, 9.5 * g, 0, TAU);
    ctx.stroke();
  }
  ctx.fillStyle = th.halo;
  ctx.beginPath();
  ctx.arc(x, y, FP_HALO_R * g, 0, TAU);
  ctx.fill();

  ctx.beginPath();
  ctx.arc(x, y, 5 * g, 0, TAU);
  ctx.fillStyle = kind === "stable" ? th.fp : th.bg;
  ctx.fill();
  if (kind === "half") {
    ctx.fillStyle = th.fp;
    ctx.beginPath();
    ctx.arc(x, y, 5 * g, Math.PI / 2, (3 * Math.PI) / 2);
    ctx.closePath();
    ctx.fill();
  }
  ctx.strokeStyle = th.fp;
  ctx.lineWidth = 1.5 * g;
  ctx.beginPath();
  ctx.arc(x, y, 5 * g, 0, TAU);
  ctx.stroke();
}

function drawHandles(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme) {
  const g = th.glyphScale;
  let sel: { x: number; y: number; color: string } | null = null;
  for (const ic of scene.ics) {
    if (ic.x0.length !== scene.dim) continue;
    const [x, y] = icPx(vp, scene, ic);
    const color = th.traj[ic.color % th.traj.length];
    if (scene.selectedIc === ic.id) {
      sel = { x, y, color };
      continue;
    }
    const r = (scene.hoverIc === ic.id ? 4.5 : 3.5) * g;
    if (ic.hidden) {
      ctx.strokeStyle = th.halo;
      ctx.lineWidth = 3 * g;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, TAU);
      ctx.stroke();
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5 * g;
      ctx.stroke();
      continue;
    }
    ctx.fillStyle = color;
    ctx.strokeStyle = th.halo;
    ctx.lineWidth = 1.5 * g;
    ctx.beginPath();
    ctx.arc(x, y, r, 0, TAU);
    ctx.fill();
    ctx.stroke();
  }
  if (sel) {
    ctx.fillStyle = th.halo;
    ctx.beginPath();
    ctx.arc(sel.x, sel.y, 9.5 * g, 0, TAU);
    ctx.fill();
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = 2 * g;
    ctx.beginPath();
    ctx.arc(sel.x, sel.y, 8 * g, 0, TAU);
    ctx.stroke();
    ctx.fillStyle = sel.color;
    ctx.beginPath();
    ctx.arc(sel.x, sel.y, 4.5 * g, 0, TAU);
    ctx.fill();
  }
}
