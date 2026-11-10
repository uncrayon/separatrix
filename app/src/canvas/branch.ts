import type { BranchOut } from "../api/types";
import { SPECIAL_KINDS, branchValue, type NormAxis, type StateAxis } from "../state/model";
import { FP_HALO_R, drawFixedPoint, drawScene, tickLabelBoxes, type Box, type DrawContext, type Scene } from "./draw";
import type { PlotTheme } from "./theme";
import { sx, sy, type Viewport } from "./viewport";

const CLAMP = 1e5;
const clampPx = (v: number) => (v > CLAMP ? CLAMP : v < -CLAMP ? -CLAMP : v);
const overlaps = (a: Box, b: Box) => a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1];

export interface ShownBranch {
  branch: Pick<BranchOut, "p" | "x" | "period" | "stable" | "unstable" | "special">;
  color: number;
}

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

function vertexPx(vp: Viewport, b: ShownBranch["branch"], dim: number, axis: StateAxis | NormAxis, i: number, m: number): [number, number] {
  return [clampPx(sx(vp, b.p[i])), clampPx(sy(vp, branchValue(b, dim, axis, i, m)))];
}

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

export function frameOnly(avoid: Box[], yAxis: Scene["yAxis"] = { kind: "state", index: 0 }): Scene {
  return {
    kind: "ode",
    dim: 0,
    xAxis: { kind: "time" },
    yAxis,
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
  marker?: number | null;
  avoid?: Box[];
}

export function drawBranch(
  ctx: DrawContext,
  vp: Viewport,
  branches: readonly ShownBranch[],
  dim: number,
  axis: StateAxis | NormAxis,
  th: PlotTheme,
  opts: BranchDrawOptions = {},
): void {
  const frame = frameOnly(opts.avoid ?? []);
  drawScene(ctx, vp.left + vp.width, vp.top + vp.height, vp, frame, th, { background: false });
  const s = th.lineScale;
  ctx.save();
  ctx.beginPath();
  ctx.rect(vp.left, vp.top, vp.width, vp.height);
  ctx.clip();
  const ticks = tickLabelBoxes(ctx, vp, th, opts.avoid ?? [], frame);
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
  const g = th.glyphScale;
  const r = FP_HALO_R * g;
  const glyphs = branches.flatMap(({ branch: b }) =>
    b.special.flatMap((sp) =>
      Array.from({ length: b.period }, (_, m) => {
        const [x, y] = vertexPx(vp, b, dim, axis, sp.index, m);
        return { sp, x, y, box: [x - r, y - r, x + r, y + r] as Box };
      }),
    ),
  );
  for (const { x, y } of glyphs) drawFixedPoint(ctx, x, y, "half", th);
  const same = (a: { kind: string; p: number }, b: { kind: string; p: number }) =>
    a.kind === b.kind && Math.abs(a.p - b.p) <= 1e-9 * (1 + Math.abs(a.p));
  const groups: (typeof glyphs)[] = [];
  for (const gl of glyphs) {
    const group = groups.find((gr) => same(gr[0].sp, gl.sp));
    if (group) group.push(gl);
    else groups.push([gl]);
  }
  const midline = vp.top + vp.height / 2;
  const segments = branches.flatMap(({ branch: b }) =>
    Array.from({ length: b.period }, (_, m) =>
      b.p.slice(1).map((_, i) => [vertexPx(vp, b, dim, axis, i, m), vertexPx(vp, b, dim, axis, i + 1, m)] as const),
    ).flat(),
  );
  const crossesBranch = (box: Box) => segments.some(([a, b]) => segmentHitsBox(a, b, box));
  const labels: Box[] = [];
  ctx.lineJoin = "round";
  ctx.lineWidth = 3;
  ctx.strokeStyle = th.halo;
  ctx.fillStyle = th.fp;
  for (const group of groups) {
    const label = SPECIAL_KINDS[group[0].sp.kind].label;
    const w = ctx.measureText(label).width;
    const fits = (b: Box) =>
      b[0] >= vp.left &&
      b[2] <= vp.left + vp.width &&
      b[1] >= vp.top &&
      ![...(opts.avoid ?? []), ...labels].some((t) => overlaps(b, t)) &&
      !glyphs.some((o) => !group.includes(o) && overlaps(b, o.box));
    const candidates = [...group]
      .sort((a, b) => Math.abs(a.y - midline) - Math.abs(b.y - midline))
      .flatMap(({ x, y }): Box[] => [
        [x + 7 * g, y - 5 * g - th.tickSize, x + 7 * g + w, y - 5 * g],
        [x - 7 * g - w, y - 5 * g - th.tickSize, x - 7 * g, y - 5 * g],
      ])
      .filter(fits);
    const box = candidates.find((b) => !crossesBranch(b)) ?? candidates[0];
    if (!box) continue;
    labels.push(box);
    ctx.strokeText(label, box[0], box[3]);
    ctx.fillText(label, box[0], box[3]);
  }
  const mk = opts.marker;
  if (mk !== null && mk !== undefined && Number.isFinite(mk) && mk >= vp.xRange[0] && mk <= vp.xRange[1]) {
    const x = Math.round(sx(vp, mk)) + 0.5;
    const gaps = [...ticks, ...labels]
      .filter((b) => b[0] <= x && x <= b[2])
      .map((b) => [b[1], b[3]] as const)
      .sort((a, b) => a[0] - b[0]);
    ctx.strokeStyle = th.accent;
    ctx.lineWidth = th.lineScale;
    ctx.beginPath();
    let y = vp.top;
    for (const [top, bottom] of gaps) {
      if (top > y) {
        ctx.moveTo(x, y);
        ctx.lineTo(x, top);
      }
      y = Math.max(y, bottom);
    }
    if (y < vp.top + vp.height) {
      ctx.moveTo(x, y);
      ctx.lineTo(x, vp.top + vp.height);
    }
    ctx.stroke();
  }
  ctx.restore();
}

/** Whether the segment from `a` to `b` meets `box` (Liang–Barsky clipping). */
function segmentHitsBox(a: readonly [number, number], b: readonly [number, number], [l, t, r, btm]: Box): boolean {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  let lo = 0;
  let hi = 1;
  for (const [p, q] of [
    [-dx, a[0] - l],
    [dx, r - a[0]],
    [-dy, a[1] - t],
    [dy, btm - a[1]],
  ]) {
    if (p === 0) {
      if (q < 0) return false;
    } else {
      const u = q / p;
      if (p < 0) lo = Math.max(lo, u);
      else hi = Math.min(hi, u);
      if (lo > hi) return false;
    }
  }
  return true;
}
