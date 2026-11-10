import type { CurveKindOut, CurveOut } from "../api/types";
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
export const CURVE_STYLES: Record<CurveKindOut, { token: string; dash: number[] }> = {
  fold: { token: "--curve-fold", dash: [] },
  hopf: { token: "--curve-hopf", dash: [8, 3] },
  branchPoint: { token: "--curve-bp", dash: [8, 2.5, 1.5, 2.5, 1.5, 2.5] },
  flip: { token: "--curve-flip", dash: [3, 3] },
  neimarkSacker: { token: "--curve-ns", dash: [8, 3] },
};
export const CURVE_WIDTH = 1.75;
/** A stretch where the curve is no bifurcation: its kind's colour, faded and dotted. */
export const NEUTRAL_STYLE = { alpha: 0.55, width: 1.5, dash: [0, 3.5] };

/** The colour of each curve kind, read from the design tokens now (they change with the theme). */
export function readCurveColors(): Record<CurveKindOut, string> {
  const css = getComputedStyle(document.documentElement);
  return Object.fromEntries(
    CURVE_KIND_ORDER.map((k) => [k, css.getPropertyValue(CURVE_STYLES[k].token).trim() || "#888"]),
  ) as Record<CurveKindOut, string>;
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
  colors: Record<CurveKindOut, string>;
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

  const path = (c: CurveOut, from: number, to: number) => {
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
  ctx.setLineDash([]);
  const tickLabels = drawResonanceTicks(ctx, vp, curves, th, opts.colors.neimarkSacker);

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
  const labels: Box[] = [...tickLabels];
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

/** Half the length of a resonance tick, across the curve, in units of the glyph scale. */
const TICK_HALF = 7;

/**
 * The weak resonances 1:q of each Neimark–Sacker curve: a short stroke across
 * the curve and its label `1:q` on the side below it. Returns the labels'
 * boxes, which the special points' labels keep clear of.
 */
function drawResonanceTicks(ctx: DrawContext, vp: Viewport, curves: readonly CurveOut[], th: PlotTheme, color: string): Box[] {
  const g = th.glyphScale;
  const boxes: Box[] = [];
  ctx.font = th.tickFont;
  ctx.textBaseline = "middle";
  for (const c of curves) {
    for (const tick of c.ticks) {
      const at = nearestSegment(vp, c, tick.p);
      if (!at) continue;
      const { x, y, nx, ny } = at;
      const h = TICK_HALF * g;
      ctx.strokeStyle = color;
      ctx.lineWidth = th.lineScale;
      ctx.beginPath();
      ctx.moveTo(x - nx * h, y - ny * h);
      ctx.lineTo(x + nx * h, y + ny * h);
      ctx.stroke();
      const label = `1:${tick.q}`;
      const w = ctx.measureText(label).width;
      const lx = x + nx * (h + 3);
      const ly = y + ny * (h + 3) + th.tickSize * 0.3;
      const left = nx >= 0 ? lx : lx - w;
      ctx.textAlign = "left";
      ctx.lineJoin = "round";
      ctx.lineWidth = 3;
      ctx.strokeStyle = th.halo;
      ctx.fillStyle = th.tick;
      ctx.strokeText(label, left, ly);
      ctx.fillText(label, left, ly);
      boxes.push([left, ly - th.tickSize / 2, left + w, ly + th.tickSize / 2]);
    }
  }
  return boxes;
}

/** The point of `c` nearest `p` on screen and the unit normal there, turned to point down the screen. */
function nearestSegment(vp: Viewport, c: CurveOut, p: readonly [number, number]) {
  const [tx, ty] = [sx(vp, p[0]), sy(vp, p[1])];
  let best: { d: number; x: number; y: number; nx: number; ny: number } | null = null;
  for (let i = 0; i + 1 < c.p1.length; i++) {
    const [ax, ay, bx, by] = [sx(vp, c.p1[i]), sy(vp, c.p2[i]), sx(vp, c.p1[i + 1]), sy(vp, c.p2[i + 1])];
    const [dx, dy] = [bx - ax, by - ay];
    const len = Math.hypot(dx, dy);
    if (!(len > 0)) continue;
    const t = Math.min(1, Math.max(0, ((tx - ax) * dx + (ty - ay) * dy) / (len * len)));
    const [x, y] = [ax + t * dx, ay + t * dy];
    const d = Math.hypot(x - tx, y - ty);
    if (best && d >= best.d) continue;
    const flip = dx < 0 ? -1 : 1;
    best = { d, x, y, nx: (-dy / len) * flip, ny: (dx / len) * flip };
  }
  return best;
}

export type Corner = "top-left" | "top-right" | "bottom-left" | "bottom-right";
/** Ties go in this order. */
const CORNERS: Corner[] = ["bottom-right", "top-right", "bottom-left", "top-left"];

export interface CornerBox {
  corner: Corner;
  box: Box;
  /** The curve points it would cover. */
  covers: number;
}

/** Where a `w` × `h` box would sit in each corner of the plot, and how many curve points it would cover. Top corners start at `top`, below the toolbar. */
export function cornerBoxes(
  vp: Viewport,
  curves: readonly Pick<CurveOut, "p1" | "p2">[],
  w: number,
  h: number,
  top: number,
  margin: number,
): CornerBox[] {
  return CORNERS.map((corner) => {
    const x0 = corner.endsWith("left") ? vp.left + margin : vp.left + vp.width - margin - w;
    const y0 = corner.startsWith("top") ? top : vp.top + vp.height - margin - h;
    const box: Box = [x0, y0, x0 + w, y0 + h];
    let covers = 0;
    for (const c of curves) for (let i = 0; i < c.p1.length; i++) if (inside(box, sx(vp, c.p1[i]), sy(vp, c.p2[i]))) covers++;
    return { corner, box, covers };
  });
}

const inside = (b: Box, x: number, y: number) => x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3];

/** The corner covering the fewest curve points, and never the crosshair at screen point `at` while another corner is clear of it. */
export function emptiestCorner(boxes: readonly CornerBox[], at: readonly [number, number] | null): Corner {
  let best: { corner: Corner; score: number } | null = null;
  for (const b of boxes) {
    const score = (at && inside(b.box, at[0], at[1]) ? Infinity : 0) + b.covers;
    if (!best || score < best.score) best = { corner: b.corner, score };
  }
  return best?.corner ?? CORNERS[0];
}
