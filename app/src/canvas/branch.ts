import type { BranchOut } from "../api/types";
import { SPECIAL_KINDS, branchValue, type NormAxis, type StateAxis } from "../state/model";
import { drawFixedPoint, drawScene, type Box, type DrawContext, type Scene } from "./draw";
import type { PlotTheme } from "./theme";
import { sx, sy, type Viewport } from "./viewport";

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);

/** A branch as the plot draws it: its points, and its palette slot. */
export interface ShownBranch {
  branch: Pick<BranchOut, "p" | "x" | "period" | "stable" | "unstable" | "special">;
  color: number;
}

/**
 * The stretches of a branch with one stability, as point index ranges
 * `[from, to]` that share their end points, split at every special point. A
 * neutral point (a special point, or the start of a switched branch, which
 * is neither stable nor has an unstable direction) takes its stability from
 * its neighbour, so it never makes a stretch of its own.
 */
export function stabilityRuns(
  b: Pick<BranchOut, "stable" | "unstable"> & { special: readonly { index: number }[] },
): { from: number; to: number; stable: boolean }[] {
  const at = new Set(b.special.map((sp) => sp.index));
  const neutral = (i: number) => at.has(i) || (!b.stable[i] && b.unstable[i] === 0);
  const runs: { from: number; to: number; stable: boolean }[] = [];
  for (let i = 0; i + 1 < b.stable.length; i++) {
    const s = neutral(i) ? b.stable[i + 1] : b.stable[i];
    const last = runs[runs.length - 1];
    if (last && last.stable === s && !at.has(i)) last.to = i + 1;
    else runs.push({ from: i, to: i + 1, stable: s });
  }
  return runs;
}

/** Point `i` of cycle member `m` on the screen. */
function vertexPx(vp: Viewport, b: ShownBranch["branch"], dim: number, axis: StateAxis | NormAxis, i: number, m: number): [number, number] {
  return [clampPx(sx(vp, b.p[i])), clampPx(sy(vp, branchValue(b, dim, axis, i, m)))];
}

/**
 * The branch vertex nearest to (`x`, `y`) on the screen, any cycle member,
 * within `slop` pixels; null when none is that near.
 */
export function nearestBranchVertex(
  vp: Viewport,
  branches: readonly ShownBranch[],
  dim: number,
  axis: StateAxis | NormAxis,
  x: number,
  y: number,
  slop = 12,
): { branch: number; index: number; p: number } | null {
  let best: { branch: number; index: number; p: number } | null = null;
  let bestD = slop * slop;
  branches.forEach(({ branch: b }, k) => {
    for (let i = 0; i < b.p.length; i++) {
      for (let m = 0; m < b.period; m++) {
        const [px, py] = vertexPx(vp, b, dim, axis, i, m);
        const d = (px - x) * (px - x) + (py - y) * (py - y);
        if (d <= bestD) {
          bestD = d;
          best = { branch: k, index: i, p: b.p[i] };
        }
      }
    }
  });
  return best;
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

export interface BranchDrawOptions {
  /** The swept parameter's value, marked with a line in accent. */
  marker?: number | null;
  avoid?: Box[];
}

/**
 * A branch plot: the frame, then each branch in `traj-(i mod 7 + 1)`, stable
 * stretches solid and unstable ones dashed, every member of a cycle; then
 * the special points as fixed-point glyphs on a halo with their labels (LP,
 * H, BP, PD, NS); the marker on top, as the orbit diagram orders them.
 */
export function drawBranch(
  ctx: DrawContext,
  vp: Viewport,
  branches: readonly ShownBranch[],
  dim: number,
  axis: StateAxis | NormAxis,
  th: PlotTheme,
  opts: BranchDrawOptions = {},
): void {
  drawScene(ctx, vp.left + vp.width, vp.top + vp.height, vp, frameOnly(opts.avoid ?? []), th, { background: false });
  const s = th.lineScale;
  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();
  ctx.lineCap = "butt";
  ctx.lineJoin = "round";
  ctx.lineWidth = 1.75 * s;
  for (const { branch: b, color } of branches) {
    ctx.strokeStyle = th.traj[color % th.traj.length];
    const runs = stabilityRuns(b);
    for (const run of runs) {
      ctx.setLineDash(run.stable ? [] : [6 * s, 4 * s]);
      ctx.beginPath();
      for (let m = 0; m < b.period; m++) {
        for (let i = run.from; i <= run.to; i++) {
          const [x, y] = vertexPx(vp, b, dim, axis, i, m);
          if (i === run.from) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
      }
      ctx.stroke();
    }
  }
  ctx.setLineDash([]);
  ctx.font = th.tickFont;
  ctx.textAlign = "left";
  ctx.textBaseline = "bottom";
  for (const { branch: b } of branches) {
    for (const sp of b.special) {
      for (let m = 0; m < b.period; m++) {
        const [x, y] = vertexPx(vp, b, dim, axis, sp.index, m);
        drawFixedPoint(ctx, x, y, "half", th);
        if (m > 0) continue;
        const label = SPECIAL_KINDS[sp.kind].label;
        ctx.lineJoin = "round";
        ctx.lineWidth = 3;
        ctx.strokeStyle = th.halo;
        ctx.fillStyle = th.fp;
        ctx.strokeText(label, x + 7 * th.glyphScale, y - 5 * th.glyphScale);
        ctx.fillText(label, x + 7 * th.glyphScale, y - 5 * th.glyphScale);
      }
    }
  }
  const mk = opts.marker;
  if (mk !== null && mk !== undefined && Number.isFinite(mk) && mk >= vp.xRange[0] && mk <= vp.xRange[1]) {
    const x = Math.round(sx(vp, mk)) + 0.5;
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = th.lineScale;
    ctx.beginPath();
    ctx.moveTo(x, vp.top);
    ctx.lineTo(x, vp.top + vp.height);
    ctx.stroke();
  }
  ctx.restore();
}
