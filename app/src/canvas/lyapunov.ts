import type { LyapunovResponse } from "../api/types";
import { drawScene, type Box, type DrawContext, type Scene } from "./draw";
import { DEFAULT_FIGURE_FONTS, type FigureFonts } from "./figure";
import { drawMath, layoutMath, mathRuns } from "./mathText";
import type { PlotTheme } from "./theme";
import { sx, sy, type Viewport } from "./viewport";

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);

export const lambdaName = (i: number): string =>
  `λ${String(i + 1).replace(/\d/g, (d) => "₀₁₂₃₄₅₆₇₈₉"[Number(d)])}`;

export function lyapunovPx(vp: Viewport, res: Pick<LyapunovResponse, "historyT" | "history" | "exponents">, i: number): Float64Array {
  const n = res.exponents.length;
  const m = res.historyT.length;
  const pts = new Float64Array(2 * m);
  let k = 0;
  for (let j = 0; j < m; j++) {
    const v = res.history[j * n + i];
    if (!Number.isFinite(v)) continue;
    pts[k++] = clampPx(sx(vp, res.historyT[j]));
    pts[k++] = clampPx(sy(vp, v));
  }
  return pts.subarray(0, k);
}

export function labelRows(finals: readonly number[], height: number, top: number, bottom: number): number[] {
  const order = finals.map((y, i) => ({ y, i })).sort((a, b) => a.y - b.y);
  const rows = new Array<number>(finals.length);
  let last = -Infinity;
  for (const { y, i } of order) {
    const r = Math.max(Math.min(Math.max(y - 4, top + height), bottom - 2), last + height);
    rows[i] = r;
    last = r;
  }
  return rows;
}

function frameOnly(avoid: Box[]): Scene {
  return {
    kind: "ode",
    dim: 0,
    xAxis: { kind: "time" },
    yAxis: { kind: "state", index: 0 },
    arrows: [],
    fieldGrid: 0,
    nullclines: [],
    fixedPoints: [],
    trajectories: [],
    ics: [],
    selectedIc: null,
    selectedFp: null,
    hoverIc: null,
    hoverFp: null,
    eigenvectors: false,
    stale: false,
    avoid: { labels: avoid, field: [] },
  };
}

export function drawLyapunov(
  ctx: DrawContext,
  vp: Viewport,
  res: LyapunovResponse | null,
  th: PlotTheme,
  fonts: FigureFonts = DEFAULT_FIGURE_FONTS,
  avoid: Box[] = [],
): void {
  drawScene(ctx, vp.left + vp.width, vp.top + vp.height, vp, frameOnly(avoid), th, { background: false });
  if (!res || res.exponents.length === 0) return;
  const n = res.exponents.length;
  const s = th.lineScale;
  const right = vp.left + vp.width;
  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();
  ctx.lineCap = "butt";
  ctx.lineJoin = "round";
  for (let i = 0; i < n; i++) {
    const y = Math.round(sy(vp, res.exponents[i])) + 0.5;
    if (!Number.isFinite(y)) continue;
    ctx.globalAlpha = 0.8;
    ctx.strokeStyle = th.traj[i % th.traj.length];
    ctx.lineWidth = s;
    ctx.setLineDash([6 * s, 4 * s]);
    ctx.beginPath();
    ctx.moveTo(vp.left, clampPx(y));
    ctx.lineTo(right, clampPx(y));
    ctx.stroke();
  }
  ctx.setLineDash([]);
  ctx.globalAlpha = 1;
  for (let i = 0; i < n; i++) {
    const pts = lyapunovPx(vp, res, i);
    if (pts.length < 2) continue;
    ctx.strokeStyle = th.traj[i % th.traj.length];
    ctx.lineWidth = 1.5 * s;
    ctx.beginPath();
    ctx.moveTo(pts[0], pts[1]);
    for (let k = 2; k < pts.length; k += 2) ctx.lineTo(pts[k], pts[k + 1]);
    if (pts.length === 2) ctx.lineTo(pts[0] + 0.01, pts[1]);
    ctx.stroke();
  }
  const size = Math.round(fonts.mathSize * 0.875);
  const layouts = res.exponents.map((_, i) => layoutMath(ctx, mathRuns(`\\lambda_{${i + 1}}`), fonts.mathFamily, size));
  const rows = labelRows(
    res.exponents.map((v) => sy(vp, v)),
    size * 1.25,
    vp.top,
    vp.top + vp.height,
  );
  layouts.forEach((layout, i) => {
    drawMath(ctx, layout, right - 8 - layout.width, rows[i], th.traj[i % th.traj.length], th.halo);
  });
  ctx.restore();
}
