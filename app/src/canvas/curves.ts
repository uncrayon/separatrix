import type { CurveKindOut, CurveOut, HomoclinicOut } from "../api/types";
import { CODIM2_KINDS, CURVE_KIND_ORDER, type PlanePointRow } from "../state/curves";
import { frameOnly } from "./branch";
import { FP_HALO_R, clampPx, drawFixedPoint, drawScene, overlaps, tickLabelBoxes, type Box, type DrawContext } from "./draw";
import type { PlotTheme } from "./theme";
import { sx, sy, type Viewport } from "./viewport";

/**
 * How each curve kind is drawn: its colour token and its dash pattern in
 * units of the line scale `s` (empty is solid). The one place to change a
 * curve's look; the canvas, the legend and the Curves list all read it.
 */
export type CurveStyleKey = CurveKindOut | "homoclinic";
export const CURVE_STYLES: Record<CurveStyleKey, { token: string; dash: number[] }> = {
  fold: { token: "--curve-fold", dash: [] },
  hopf: { token: "--curve-hopf", dash: [8, 3] },
  branchPoint: { token: "--curve-bp", dash: [8, 2.5, 1.5, 2.5, 1.5, 2.5] },
  flip: { token: "--curve-flip", dash: [3, 3] },
  neimarkSacker: { token: "--curve-ns", dash: [8, 3] },
  homoclinic: { token: "--curve-hom", dash: [8, 3, 1.5, 3] },
};
export const CURVE_WIDTH = 1.75;
/** A stretch where the curve is no bifurcation: its kind's colour, faded and dotted. */
export const NEUTRAL_STYLE = { alpha: 0.55, width: 1.5, dash: [0, 3.5] };

/** The colour of each curve kind, read from the design tokens now (they change with the theme). */
export function readCurveColors(): Record<CurveStyleKey, string> {
  const css = getComputedStyle(document.documentElement);
  return Object.fromEntries(
    [...CURVE_KIND_ORDER, "homoclinic" as const].map((k) => [k, css.getPropertyValue(CURVE_STYLES[k].token).trim() || "#888"]),
  ) as Record<CurveStyleKey, string>;
}

/** The runs of segments `i → i+1` that are neutral (both ends neutral) or not, as [from, to, neutral]. */
export function segmentRuns(neutral: readonly boolean[]): [number, number, boolean][] {
  const runs: [number, number, boolean][] = [];
  for (let i = 0; i + 1 < neutral.length; i++) {
    const n = neutral[i] && neutral[i + 1];
    const last = runs[runs.length - 1];
    if (last && last[2] === n) last[1] = i + 1;
    else runs.push([i, i + 1, n]);
  }
  return runs;
}

export interface CurvesDrawOptions {
  colors: Record<CurveStyleKey, string>;
  points: readonly PlanePointRow[];
  /** The current parameters (across, up). */
  crosshair: [number, number] | null;
  /** The Branch view's range, drawn at the current up value. */
  slice: { range: [number, number]; at: number } | null;
  avoid?: Box[];
}

export function drawCurves(
  ctx: DrawContext,
  vp: Viewport,
  curves: readonly CurveOut[],
  th: PlotTheme,
  opts: CurvesDrawOptions,
  homoclinic: readonly Pick<HomoclinicOut, "p1" | "p2">[] = [],
): void {
  const frame = frameOnly(opts.avoid ?? [], { kind: "time" });
  drawScene(ctx, vp.left + vp.width, vp.top + vp.height, vp, frame, th, { background: false });
  const s = th.lineScale;
  const g = th.glyphScale;
  const px = (a: number, b: number): [number, number] => [clampPx(sx(vp, a)), clampPx(sy(vp, b))];
  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();
  const ticks = tickLabelBoxes(ctx, vp, th, opts.avoid ?? [], frame);

  if (opts.slice) {
    const [lo, hi] = opts.slice.range;
    const [x0, y] = px(lo, opts.slice.at);
    const [x1] = px(hi, opts.slice.at);
    const t = 3 * g;
    ctx.strokeStyle = th.tick;
    ctx.lineWidth = s;
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.moveTo(x0, y);
    ctx.lineTo(x1, y);
    ctx.moveTo(x0, y - t);
    ctx.lineTo(x0, y + t);
    ctx.moveTo(x1, y - t);
    ctx.lineTo(x1, y + t);
    ctx.stroke();
  }

  const path = (c: Pick<CurveOut, "p1" | "p2">, from: number, to: number) => {
    ctx.beginPath();
    for (let i = from; i <= to; i++) {
      const [x, y] = px(c.p1[i], c.p2[i]);
      if (i === from) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
  };
  const ordered = [...curves].sort((a, b) => CURVE_KIND_ORDER.indexOf(a.kind) - CURVE_KIND_ORDER.indexOf(b.kind));
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.globalAlpha = NEUTRAL_STYLE.alpha;
  ctx.lineWidth = NEUTRAL_STYLE.width * s;
  ctx.setLineDash(NEUTRAL_STYLE.dash.map((d) => d * s));
  for (const c of ordered) {
    ctx.strokeStyle = opts.colors[c.kind];
    for (const [from, to, neutral] of segmentRuns(c.neutral)) if (neutral) path(c, from, to);
  }
  ctx.globalAlpha = 1;
  ctx.lineCap = "butt";
  ctx.lineWidth = CURVE_WIDTH * s;
  for (const c of ordered) {
    ctx.strokeStyle = opts.colors[c.kind];
    ctx.setLineDash(CURVE_STYLES[c.kind].dash.map((d) => d * s));
    for (const [from, to, neutral] of segmentRuns(c.neutral)) if (!neutral) path(c, from, to);
  }
  ctx.strokeStyle = opts.colors.homoclinic;
  ctx.setLineDash(CURVE_STYLES.homoclinic.dash.map((d) => d * s));
  for (const h of homoclinic) path(h, 0, h.p1.length - 1);
  ctx.setLineDash([]);

  if (opts.crosshair) {
    const [x, y] = opts.crosshair.map((v, k) => Math.round(k === 0 ? sx(vp, v) : sy(vp, v)) + 0.5);
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(x, vp.top);
    ctx.lineTo(x, vp.top + vp.height);
    ctx.moveTo(vp.left, y);
    ctx.lineTo(vp.left + vp.width, y);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(x, y, 6 * g, 0, 2 * Math.PI);
    ctx.stroke();
  }

  const r = FP_HALO_R * g;
  const glyphs = opts.points.map((pt) => {
    const [x, y] = px(pt.p[0], pt.p[1]);
    return { pt, x, y, box: [x - r, y - r, x + r, y + r] as Box };
  });
  for (const { x, y } of glyphs) drawFixedPoint(ctx, x, y, "half", th);

  ctx.font = th.tickFont;
  ctx.textAlign = "left";
  ctx.textBaseline = "bottom";
  ctx.lineJoin = "round";
  ctx.lineWidth = 3;
  ctx.strokeStyle = th.halo;
  ctx.fillStyle = th.fp;
  const labels: Box[] = [];
  for (const gl of glyphs) {
    const label = CODIM2_KINDS[gl.pt.kind].label;
    const w = ctx.measureText(label).width;
    const { x, y } = gl;
    const fits = (b: Box) =>
      b[0] >= vp.left &&
      b[2] <= vp.left + vp.width &&
      b[1] >= vp.top &&
      ![...(opts.avoid ?? []), ...ticks, ...labels].some((t) => overlaps(b, t)) &&
      !glyphs.some((o) => o !== gl && overlaps(b, o.box));
    const box = (
      [
        [x + 7 * g, y - 5 * g - th.tickSize, x + 7 * g + w, y - 5 * g],
        [x - 7 * g - w, y - 5 * g - th.tickSize, x - 7 * g, y - 5 * g],
        [x + 7 * g, y + 5 * g, x + 7 * g + w, y + 5 * g + th.tickSize],
        [x - 7 * g - w, y + 5 * g, x - 7 * g, y + 5 * g + th.tickSize],
      ] as Box[]
    ).find(fits);
    if (!box) continue;
    labels.push(box);
    ctx.strokeText(label, box[0], box[3]);
    ctx.fillText(label, box[0], box[3]);
  }
  ctx.restore();
}
