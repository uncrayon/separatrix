// Canvas2D renderer for the phase portrait. Pure drawing: no state, no events.
//
// Follows the PhasePlane conventions of the design system:
// - z-order, bottom to top: plot-bg, grid, axes and tick marks, direction field,
//   tick labels (on a halo), nullclines, trajectories (the selected one last),
//   fixed points, start points (the selected one last);
// - two scales from the theme: s = --line-scale for line widths, and the gentler
//   g = 0.6 + 0.4 s for glyphs, heads and handles;
// - every colour comes from the plot tokens, read at draw time (theme.ts).

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
import type { PlotTheme } from "./theme";
import { niceTicks, sx, sy, tickLabel, type Viewport } from "./viewport";

/** A screen rectangle in CSS px: [left, top, right, bottom]. */
export type Box = [number, number, number, number];

/** A trajectory ready to draw. */
export interface SceneTrajectory {
  out: TrajectoryOut;
  meta: TrajectoryMeta;
  color: number;
}

/** Everything the renderer needs for one frame. */
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
  /** True while the drawn data belongs to an older view or system (drawn dimmer). */
  stale: boolean;
  /**
   * Boxes covered by chrome over the plot (the canvas toolbar, the typeset axis
   * names): tick labels inside them are left out, and field marks keep clear of
   * the axis names.
   */
  avoid: { labels: Box[]; field: Box[] };
}

/** What the coordinate helpers need from a scene. */
export type SceneAxes = Pick<Scene, "kind" | "dim" | "xAxis" | "yAxis">;

/** Strogatz's fixed-point glyphs: the fill carries stability, never the shape. */
export type FixedPointGlyphKind = "stable" | "unstable" | "half";

/** Filled when stable, open when unstable or a saddle, half-filled when linearization does not decide. */
export function fixedPointGlyph(c: Classification, stable: boolean): FixedPointGlyphKind {
  if (c === "center" || c === "non-hyperbolic" || c === "non-isolated") return "half";
  return stable ? "stable" : "unstable";
}

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);
const TAU = Math.PI * 2;

/** Screen position of a fixed point. On a time axis it sits at the left/bottom edge. */
export function fixedPointPx(vp: Viewport, scene: SceneAxes, fp: FixedPointOut): [number, number] {
  const px = scene.xAxis.kind === "time" ? vp.left + 12 : sx(vp, fp.x[scene.xAxis.index]);
  const py = scene.yAxis.kind === "time" ? vp.top + vp.height - 12 : sy(vp, fp.x[scene.yAxis.index]);
  return [px, py];
}

/** Screen position of an initial-condition handle. */
export function icPx(vp: Viewport, scene: SceneAxes, ic: InitialCondition): [number, number] {
  const t0 = scene.kind === "map" ? 0 : ic.t0;
  const px = sx(vp, scene.xAxis.kind === "time" ? t0 : ic.x0[scene.xAxis.index]);
  const py = sy(vp, scene.yAxis.kind === "time" ? t0 : ic.x0[scene.yAxis.index]);
  return [px, py];
}

/** Screen-space polyline of a trajectory (NaN-free, clamped). */
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

/**
 * Where the axes run on screen. An axis through the origin is `inView` when the
 * origin's coordinate is visible; otherwise it is pinned to the nearest edge,
 * where its tick labels stay (the grid still marks the values).
 */
export function axisLines(vp: Viewport): { X: number; Y: number; xIn: boolean; yIn: boolean } {
  const right = vp.left + vp.width;
  const bottom = vp.top + vp.height;
  const [x0, x1] = vp.xRange;
  const [y0, y1] = vp.yRange;
  const yIn = Math.min(x0, x1) < 0 && Math.max(x0, x1) > 0; // the y-axis (x = 0)
  const xIn = Math.min(y0, y1) < 0 && Math.max(y0, y1) > 0; // the x-axis (y = 0)
  const X = yIn ? Math.round(sx(vp, 0)) + 0.5 : sx(vp, 0) < vp.left ? vp.left : right;
  const Y = xIn ? Math.round(sy(vp, 0)) + 0.5 : sy(vp, 0) > bottom ? bottom : vp.top;
  return { X, Y, xIn, yIn };
}

/** Label steps for the visible ranges; grid lines also fall on the half steps. */
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

/** Draw a full frame. `w × h` is the canvas size in CSS px. */
export function drawScene(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  vp: Viewport,
  scene: Scene | null,
  th: PlotTheme,
): void {
  // 1. plot-bg
  ctx.globalAlpha = 1;
  ctx.fillStyle = th.bg;
  ctx.fillRect(0, 0, w, h);

  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();

  const ticks = ticksFor(vp);
  // 2. grid, 3. axes and tick marks
  drawGrid(ctx, vp, ticks, th);
  const axes = axisLines(vp);
  drawAxes(ctx, vp, ticks, axes, th);
  if (!scene) {
    drawLabels(ctx, layoutLabels(ctx, vp, ticks, axes, th, []), th);
    ctx.restore();
    return;
  }

  // Tick labels are laid out first so the field can leave them clear. With time
  // on an axis, fixed points sit at the edge of the other axis's labels: put
  // those labels on the far side of the axis.
  const labels = layoutLabels(ctx, vp, ticks, axes, th, scene.avoid.labels, {
    yRight: scene.xAxis.kind === "time",
    xAbove: scene.yAxis.kind === "time",
  });
  const fade = scene.stale ? 0.55 : 1;
  // 4. direction field
  ctx.globalAlpha = fade;
  drawField(ctx, vp, scene, th, [...labels.map((l) => l.box), ...scene.avoid.field]);
  // 5. tick labels on a halo, above the field
  ctx.globalAlpha = 1;
  drawLabels(ctx, labels, th);
  // 6. nullclines, 7. trajectories
  ctx.globalAlpha = fade;
  drawNullclines(ctx, vp, scene, th);
  drawTrajectories(ctx, vp, scene, th, fade);
  // 8. fixed points, the selected one last
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
  // 9. start points (drag handles), the selected one last
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
    for (let v = Math.ceil(a / s) * s; v <= b + s * 1e-9; v += s) out.push(v);
    return out;
  };
  // 1px, not scaled, pixel-snapped.
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

function drawAxes(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  ticks: ReturnType<typeof ticksFor>,
  axes: ReturnType<typeof axisLines>,
  th: PlotTheme,
) {
  const s = th.lineScale;
  const tick = 4 * th.glyphScale;
  ctx.strokeStyle = th.axis;
  ctx.lineWidth = s;
  ctx.lineCap = "butt";
  ctx.beginPath();
  if (axes.yIn) {
    ctx.moveTo(axes.X, vp.top);
    ctx.lineTo(axes.X, vp.top + vp.height);
    // Tick marks point outward, towards the labels (left of the y-axis).
    for (const v of ticks.y.ticks) {
      if (v === 0) continue;
      const y = Math.round(sy(vp, v)) + 0.5;
      ctx.moveTo(axes.X, y);
      ctx.lineTo(axes.X - tick, y);
    }
  }
  if (axes.xIn) {
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

/**
 * Tick labels beside the axes: whole steps only, one "0" at the origin (below
 * and left), max(4g + 3, 7.5g + 2) px from the axis so a fixed point on the
 * axis never covers them. A label that would leave the plot or sit under chrome
 * is left out.
 */
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
  const gap = Math.max(4 * g + 3, 7.5 * g + 2);
  const right = vp.left + vp.width;
  const bottom = vp.top + vp.height;
  const hgt = th.tickSize;
  ctx.font = th.tickFont;
  const both = axes.xIn && axes.yIn;
  const out: Label[] = [];
  const make = (
    text: string,
    x: number,
    y: number,
    align: CanvasTextAlign,
    base: CanvasTextBaseline,
  ): Label => {
    const wid = ctx.measureText(text).width;
    const l = align === "center" ? x - wid / 2 : align === "right" ? x - wid : x;
    const t = base === "top" ? y : base === "middle" ? y - hgt / 2 : y - hgt;
    return { text, x, y, align, base, box: [l - 3, t - 3, l + wid + 3, t + hgt + 3] };
  };
  const keep = (lab: Label) => {
    const [l, t, r, b] = lab.box;
    if (l < vp.left || r > right || t < vp.top || b > bottom) return;
    if (avoid.some((a) => overlaps(lab.box, a))) return;
    if (out.some((o) => overlaps(lab.box, o.box))) return;
    out.push(lab);
  };

  // x labels below the x-axis, or above it when it is pinned to the bottom edge.
  const xBelow = prefer.xAbove ? axes.Y - gap - hgt - 3 < vp.top : axes.Y + gap + hgt + 3 <= bottom;
  const xy = xBelow ? axes.Y + gap : axes.Y - gap;
  for (const v of ticks.x.ticks) {
    if (v === 0 && both) continue;
    keep(make(tickLabel(v, ticks.x.step), sx(vp, v), xy, "center", xBelow ? "top" : "bottom"));
  }
  // y labels left of the y-axis, or right of it when it is pinned to the left edge.
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

/**
 * One centred segment per cell, all the same length (the field shows direction,
 * never speed), with a small open V head at the tip. Zero vectors are skipped,
 * and so is any mark whose box would touch a label.
 */
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
    // Normalise in screen space so marks look the same length at any aspect.
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

/** Dashed, in null-1 for the first equation, null-2 for the second, … */
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
  // Backward halves first so forward time sits on top; the selected curve last.
  const rank = (t: SceneTrajectory) =>
    (scene.selectedIc === t.meta.icId ? 2 : 0) + (t.meta.direction === "forward" ? 1 : 0);
  const ordered = [...scene.trajectories].sort((a, b) => rank(a) - rank(b));
  const isMap = scene.kind === "map";
  const timeSeries = scene.xAxis.kind === "time" || scene.yAxis.kind === "time";
  const dense = scene.dim >= 3;
  // Heads already placed (screen px), shared by all curves, so orbits that run
  // together (a limit cycle, curves converging on a fixed point) stay readable.
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
      // Mark where the solver stopped (blew up, step limit): a small cross.
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

/**
 * Filled triangular heads pointing in forward time: one at half the visible arc
 * length (one per 300px on curves longer than 600px), each slid within the
 * middle third of its stretch to the straightest spot (least turning over
 * ±12px), so a head never sits on a bend. Backward halves point the way time
 * runs, not the way they were integrated.
 */
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
  // Cumulative visible arc length at each point.
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
  /** First point index whose arc length reaches `t`. */
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
    const a = at(acc[k] - 12);
    const b = at(acc[k] + 12);
    if (a >= k || b <= k) return Infinity;
    const t1 = Math.atan2(pts[2 * k + 1] - pts[2 * a + 1], pts[2 * k] - pts[2 * a]);
    const t2 = Math.atan2(pts[2 * b + 1] - pts[2 * k + 1], pts[2 * b] - pts[2 * k]);
    return Math.abs(Math.atan2(Math.sin(t2 - t1), Math.cos(t2 - t1)));
  };
  const count = Math.min(maxHeads, total > 600 ? Math.floor(total / 300) : 1);
  const stretch = total / count;
  const gap2 = minGap * minGap;
  const hw = 0.45 * L;
  ctx.beginPath();
  for (let h = 0; h < count; h++) {
    const mid = (h + 0.5) * stretch;
    let k = at(mid);
    let best = turn(k);
    for (let t = mid - stretch / 6; t <= mid + stretch / 6; t += 4) {
      const c = at(t);
      const v = turn(c) + 1e-3 * Math.abs(t - mid);
      if (v < best - 0.05) {
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
    // Direction: back along the curve until the step has some length.
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

/**
 * One fixed point in Strogatz's glyph, on a plot-halo disc so it reads over any
 * line: filled when stable, open when unstable or a saddle, left half filled
 * when linearization does not decide. Selection adds an accent ring outside the
 * halo; hover grows the mark 1.2×.
 */
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
  ctx.arc(x, y, 7.5 * g, 0, TAU);
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

/**
 * Start points: a dot in the trajectory's colour with a plot-halo edge; bigger
 * on hover. The selected one sits on a plot-halo disc inside an accent ring
 * (the only place accent appears on the plot) and is drawn last.
 */
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
      // A hidden trajectory keeps its start point, drawn hollow.
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
