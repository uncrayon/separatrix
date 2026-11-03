// Canvas2D renderer for the phase portrait. Pure drawing: no state, no events.

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
}

/** Marker shapes for fixed points; fill encodes stability. */
export type FixedPointShape = "circle" | "spiral" | "centre" | "diamond" | "triangle";

/** Shape by classification (see the legend). */
export function fixedPointShape(c: Classification): FixedPointShape {
  switch (c) {
    case "saddle":
    case "saddle (n-D)":
      return "diamond";
    case "stable spiral":
    case "unstable spiral":
      return "spiral";
    case "center":
      return "centre";
    case "non-isolated":
    case "non-hyperbolic":
      return "triangle";
    default:
      return "circle";
  }
}

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);

/** Screen position of a fixed point. On a time axis it sits at the left/bottom edge. */
export function fixedPointPx(vp: Viewport, scene: Scene, fp: FixedPointOut): [number, number] {
  const px = scene.xAxis.kind === "time" ? vp.left + 12 : sx(vp, fp.x[scene.xAxis.index]);
  const py = scene.yAxis.kind === "time" ? vp.top + vp.height - 12 : sy(vp, fp.x[scene.yAxis.index]);
  return [px, py];
}

/** Screen position of an initial-condition handle. */
export function icPx(vp: Viewport, scene: Scene, ic: InitialCondition): [number, number] {
  const t0 = scene.kind === "map" ? 0 : ic.t0;
  const px = sx(vp, scene.xAxis.kind === "time" ? t0 : ic.x0[scene.xAxis.index]);
  const py = sy(vp, scene.yAxis.kind === "time" ? t0 : ic.x0[scene.yAxis.index]);
  return [px, py];
}

/** Screen-space polyline of a trajectory (NaN-free, clamped). */
export function trajectoryPx(vp: Viewport, scene: Scene, out: TrajectoryOut): Float64Array {
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

/** Draw a full frame. `w × h` is the canvas size in CSS px. */
export function drawScene(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  vp: Viewport,
  scene: Scene | null,
  th: PlotTheme,
): void {
  const ls = th.lineScale;
  ctx.fillStyle = th.bg;
  ctx.fillRect(0, 0, w, h);
  drawGrid(ctx, vp, th);
  if (!scene) return;

  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();
  if (scene.stale) ctx.globalAlpha = 0.55;

  drawField(ctx, vp, scene, th);
  drawNullclines(ctx, vp, scene, th);
  drawFixedPointGuides(ctx, vp, scene, th);
  drawTrajectories(ctx, vp, scene, th);
  ctx.globalAlpha = 1;
  scene.fixedPoints.forEach((fp, i) => {
    const [px, py] = fixedPointPx(vp, scene, fp);
    const r = (scene.hoverFp === i || scene.selectedFp === i ? 7.5 : 6) * Math.sqrt(ls);
    drawFixedPointMarker(ctx, px, py, r, fp, th, scene.selectedFp === i);
  });
  drawHandles(ctx, vp, scene, th);
  ctx.restore();
}

function drawGrid(ctx: CanvasRenderingContext2D, vp: Viewport, th: PlotTheme) {
  const ls = th.lineScale;
  const xt = niceTicks(vp.xRange[0], vp.xRange[1], Math.max(2, Math.round(vp.width / 90)));
  const yt = niceTicks(vp.yRange[0], vp.yRange[1], Math.max(2, Math.round(vp.height / 70)));
  const bottom = vp.top + vp.height;
  const right = vp.left + vp.width;

  ctx.lineWidth = 1;
  ctx.strokeStyle = th.grid;
  ctx.beginPath();
  for (const v of xt.ticks) {
    const x = Math.round(sx(vp, v)) + 0.5;
    ctx.moveTo(x, vp.top);
    ctx.lineTo(x, bottom);
  }
  for (const v of yt.ticks) {
    const y = Math.round(sy(vp, v)) + 0.5;
    ctx.moveTo(vp.left, y);
    ctx.lineTo(right, y);
  }
  ctx.stroke();

  // Axes through the origin, when visible.
  ctx.strokeStyle = th.axis;
  ctx.lineWidth = Math.max(1, ls);
  ctx.beginPath();
  if (vp.xRange[0] < 0 && vp.xRange[1] > 0) {
    const x = Math.round(sx(vp, 0)) + 0.5;
    ctx.moveTo(x, vp.top);
    ctx.lineTo(x, bottom);
  }
  if (vp.yRange[0] < 0 && vp.yRange[1] > 0) {
    const y = Math.round(sy(vp, 0)) + 0.5;
    ctx.moveTo(vp.left, y);
    ctx.lineTo(right, y);
  }
  ctx.stroke();

  // Frame baseline on the left and bottom, where the ticks are.
  ctx.strokeStyle = th.axis;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(vp.left + 0.5, vp.top);
  ctx.lineTo(vp.left + 0.5, bottom + 0.5);
  ctx.lineTo(right, bottom + 0.5);
  ctx.stroke();

  ctx.fillStyle = th.tickText;
  ctx.font = th.tickFont;
  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  ctx.beginPath();
  for (const v of xt.ticks) {
    const x = sx(vp, v);
    if (x < vp.left + 4 || x > right - 4) continue;
    ctx.moveTo(Math.round(x) + 0.5, bottom);
    ctx.lineTo(Math.round(x) + 0.5, bottom + 4);
    ctx.fillText(tickLabel(v, xt.step), x, bottom + 7);
  }
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";
  for (const v of yt.ticks) {
    const y = sy(vp, v);
    if (y < vp.top + 4 || y > bottom - 4) continue;
    ctx.moveTo(vp.left - 4, Math.round(y) + 0.5);
    ctx.lineTo(vp.left, Math.round(y) + 0.5);
    ctx.fillText(tickLabel(v, yt.step), vp.left - 7, y);
  }
  ctx.stroke();
}

function drawField(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme) {
  if (!scene.arrows.length || scene.fieldGrid <= 0) return;
  const ls = th.lineScale;
  const cell = Math.min(vp.width, vp.height) / scene.fieldGrid;
  const len = Math.min(cell * 0.55, 24);
  const head = Math.min(4 * Math.sqrt(ls), len * 0.35);
  const sxu = vp.width / (vp.xRange[1] - vp.xRange[0]);
  const syu = vp.height / (vp.yRange[1] - vp.yRange[0]);
  ctx.strokeStyle = th.field;
  ctx.lineWidth = Math.sqrt(ls);
  ctx.lineCap = "round";
  ctx.beginPath();
  for (const a of scene.arrows) {
    // Normalise in screen space so arrows look the same length at any aspect.
    const dx = a.u * sxu;
    const dy = -a.v * syu;
    const m = Math.hypot(dx, dy);
    const cx = sx(vp, a.x);
    const cy = sy(vp, a.y);
    if (!Number.isFinite(m)) continue;
    if (m < 1e-12) {
      ctx.moveTo(cx + 1, cy);
      ctx.arc(cx, cy, 1, 0, Math.PI * 2);
      continue;
    }
    const ux = dx / m;
    const uy = dy / m;
    const x0 = cx - (ux * len) / 2;
    const y0 = cy - (uy * len) / 2;
    const x1 = cx + (ux * len) / 2;
    const y1 = cy + (uy * len) / 2;
    ctx.moveTo(x0, y0);
    ctx.lineTo(x1, y1);
    ctx.moveTo(x1 - ux * head - uy * head * 0.6, y1 - uy * head + ux * head * 0.6);
    ctx.lineTo(x1, y1);
    ctx.lineTo(x1 - ux * head + uy * head * 0.6, y1 - uy * head - ux * head * 0.6);
  }
  ctx.stroke();
}

function drawNullclines(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme) {
  const ls = th.lineScale;
  ctx.lineWidth = 1.6 * ls;
  ctx.setLineDash([7 * ls, 5 * ls]);
  ctx.lineCap = "butt";
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

/** On a time axis, a fixed point is a constant solution: draw it as a faint line. */
function drawFixedPointGuides(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme) {
  const timeX = scene.xAxis.kind === "time";
  const timeY = scene.yAxis.kind === "time";
  if (!timeX && !timeY) return;
  ctx.strokeStyle = th.fp;
  ctx.globalAlpha = scene.stale ? 0.2 : 0.35;
  ctx.lineWidth = 1 * th.lineScale;
  ctx.setLineDash([2 * th.lineScale, 4 * th.lineScale]);
  ctx.beginPath();
  for (const fp of scene.fixedPoints) {
    const [px, py] = fixedPointPx(vp, scene, fp);
    if (timeX) {
      ctx.moveTo(px, py);
      ctx.lineTo(vp.left + vp.width, py);
    } else {
      ctx.moveTo(px, py);
      ctx.lineTo(px, vp.top);
    }
  }
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.globalAlpha = scene.stale ? 0.55 : 1;
}

function drawTrajectories(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme) {
  const ls = th.lineScale;
  const baseAlpha = scene.stale ? 0.55 : 1;
  // Backward halves first, so forward time sits on top.
  const ordered = [...scene.trajectories].sort(
    (a, b) => Number(a.meta.direction === "forward") - Number(b.meta.direction === "forward"),
  );
  const isMap = scene.kind === "map";
  const timeSeries = scene.xAxis.kind === "time" || scene.yAxis.kind === "time";
  const dense = scene.dim >= 3;
  // Chevrons already placed (screen px), shared by all curves so overlapping
  // orbits (limit cycles, curves converging onto a fixed point) stay readable.
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
        ctx.globalAlpha = baseAlpha * 0.4;
        ctx.lineWidth = 1 * ls;
        strokePolyline(ctx, pts);
      }
      ctx.globalAlpha = baseAlpha;
      const r = n > 2000 ? 0.75 * ls : n > 300 ? 1.3 * ls : 2.6 * ls;
      if (n > 2000) {
        for (let k = 0; k < pts.length; k += 2) ctx.fillRect(pts[k] - r, pts[k + 1] - r, 2 * r, 2 * r);
      } else {
        ctx.beginPath();
        for (let k = 0; k < pts.length; k += 2) {
          ctx.moveTo(pts[k] + r, pts[k + 1]);
          ctx.arc(pts[k], pts[k + 1], r, 0, Math.PI * 2);
        }
        ctx.fill();
      }
      continue;
    }

    ctx.globalAlpha = baseAlpha * (backward ? 0.5 : 1);
    ctx.lineWidth = (selected ? 2.6 : dense ? 1.1 : 1.8) * ls;
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    strokePolyline(ctx, pts);
    // Long chaotic orbits would be covered in chevrons: only mark their start.
    const arrowLimit = dense ? 1600 : Infinity;
    drawDirectionArrows(
      ctx,
      pts,
      backward,
      130 * Math.sqrt(ls),
      5.5 * Math.sqrt(ls) * (selected ? 1.2 : 1),
      arrowLimit,
      placed,
    );

    if (tr.out.status !== "completed") {
      // Mark where the solver stopped.
      const x = pts[pts.length - 2];
      const y = pts[pts.length - 1];
      const s = 4 * Math.sqrt(ls);
      ctx.globalAlpha = baseAlpha;
      ctx.lineWidth = 1.6 * ls;
      ctx.beginPath();
      ctx.moveTo(x - s, y - s);
      ctx.lineTo(x + s, y + s);
      ctx.moveTo(x + s, y - s);
      ctx.lineTo(x - s, y + s);
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

/** Chevrons every `spacing` px along the curve, pointing in the direction of increasing time. */
function drawDirectionArrows(
  ctx: CanvasRenderingContext2D,
  pts: Float64Array,
  reversed: boolean,
  spacing: number,
  size: number,
  maxLength = Infinity,
  placed: number[] = [],
) {
  const minGap2 = (spacing * 0.35) ** 2;
  let acc = spacing * 0.45;
  let travelled = 0;
  ctx.beginPath();
  for (let k = 2; k < pts.length && travelled < maxLength; k += 2) {
    const dx = pts[k] - pts[k - 2];
    const dy = pts[k + 1] - pts[k - 1];
    const seg = Math.hypot(dx, dy);
    if (seg === 0) continue;
    travelled += seg;
    acc -= seg;
    if (acc > 0) continue;
    const x = pts[k];
    const y = pts[k + 1];
    let crowded = false;
    for (let j = 0; j < placed.length && !crowded; j += 2) {
      crowded = (placed[j] - x) ** 2 + (placed[j + 1] - y) ** 2 < minGap2;
    }
    if (crowded) continue; // try again at the next sample
    placed.push(x, y);
    acc = spacing;
    let ux = dx / seg;
    let uy = dy / seg;
    if (reversed) {
      ux = -ux;
      uy = -uy;
    }
    ctx.moveTo(x - ux * size - uy * size * 0.65, y - uy * size + ux * size * 0.65);
    ctx.lineTo(x, y);
    ctx.lineTo(x - ux * size + uy * size * 0.65, y - uy * size - ux * size * 0.65);
  }
  ctx.stroke();
}

/** One fixed-point marker. Also used by the legend (via an offscreen canvas). */
export function drawFixedPointMarker(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  r: number,
  fp: Pick<FixedPointOut, "classification" | "stable">,
  th: PlotTheme,
  selected = false,
) {
  const shape = fixedPointShape(fp.classification);
  const filled = fp.stable;
  const lw = 1.6 * Math.sqrt(th.lineScale);
  ctx.lineWidth = lw;
  ctx.strokeStyle = th.fp;
  ctx.fillStyle = filled ? th.fp : th.bg;
  ctx.globalAlpha = 1;

  if (selected) {
    ctx.save();
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = 2 * lw;
    ctx.beginPath();
    ctx.arc(x, y, r + 5, 0, Math.PI * 2);
    ctx.stroke();
    ctx.restore();
  }

  ctx.beginPath();
  switch (shape) {
    case "diamond":
      ctx.moveTo(x, y - r * 1.2);
      ctx.lineTo(x + r * 1.2, y);
      ctx.lineTo(x, y + r * 1.2);
      ctx.lineTo(x - r * 1.2, y);
      ctx.closePath();
      break;
    case "triangle":
      ctx.moveTo(x, y - r * 1.15);
      ctx.lineTo(x + r * 1.1, y + r * 0.8);
      ctx.lineTo(x - r * 1.1, y + r * 0.8);
      ctx.closePath();
      break;
    default:
      ctx.arc(x, y, shape === "spiral" ? r * 0.75 : r, 0, Math.PI * 2);
  }
  ctx.fill();
  ctx.stroke();

  if (shape === "spiral") {
    // An open arc around the marker: "things turn around here".
    ctx.strokeStyle = th.fp;
    ctx.lineWidth = lw * 0.9;
    ctx.beginPath();
    ctx.arc(x, y, r * 1.45, -Math.PI * 0.15, Math.PI * 1.25);
    ctx.stroke();
  } else if (shape === "centre") {
    ctx.fillStyle = th.fp;
    ctx.beginPath();
    ctx.arc(x, y, r * 0.38, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawHandles(ctx: CanvasRenderingContext2D, vp: Viewport, scene: Scene, th: PlotTheme) {
  const ls = Math.sqrt(th.lineScale);
  for (const ic of scene.ics) {
    if (ic.x0.length !== scene.dim) continue;
    const [x, y] = icPx(vp, scene, ic);
    const color = th.traj[ic.color % th.traj.length];
    const active = scene.selectedIc === ic.id || scene.hoverIc === ic.id;
    const r = (active ? 6.5 : 5) * ls;
    if (scene.selectedIc === ic.id) {
      ctx.strokeStyle = color;
      ctx.globalAlpha = 0.35;
      ctx.lineWidth = 6 * ls;
      ctx.beginPath();
      ctx.arc(x, y, r + 4 * ls, 0, Math.PI * 2);
      ctx.stroke();
      ctx.globalAlpha = 1;
    }
    ctx.fillStyle = color;
    ctx.strokeStyle = th.handleRing;
    ctx.lineWidth = 2 * ls;
    ctx.beginPath();
    ctx.arc(x, y, r, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
  }
}
