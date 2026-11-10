import { useEffect, useRef, useState } from "react";
import { compileSystem, computePortrait } from "../api/client";
import type { CompileResponse, PortraitResponse, SystemKind } from "../api/types";
import { fixedPointGlyph, trajectoryPx } from "../canvas/draw";
import { readPlotTheme } from "../canvas/theme";
import { makeViewport, sx, sy, hasEqualScales, type Viewport } from "../canvas/viewport";
import { cobwebPx, diagonalSegment, graphPx, orbitValues } from "../canvas/cobweb";
import { drawSpaceLines } from "../canvas/space";
import {
  DEFAULT_LAYERS,
  DEFAULT_SOLVER,
  autoSamples,
  isCobweb,
  isReturnMap,
  isSection,
  isSpace,
  portraitGraph,
  templateView,
  trajectorySpans,
  type MapRequestFields,
  type PlotView,
} from "../state/model";
import { templateSource, type Template } from "../templates";

const ASPECT = 1.6;
const STEP = 0.01;
const THUMB_FIELD_GRID = 13;
/** Graph samples of f for a cobweb thumbnail. */
const THUMB_GRAPH_SAMPLES = 256;

/** What a thumbnail draws: a portrait response in its view, each trajectory's palette index in `icOf`. */
export interface Thumb {
  kind: SystemKind;
  dim: number;
  view: PlotView;
  res: PortraitResponse;
  icOf: number[];
}

const cache = new Map<string, Promise<Thumb | null>>();
const compiled = new Map<string, Promise<CompileResponse>>();
let nextRequestId = 1_000_000_000;

/** The template compiled once, shared by its thumbnail and its gallery tags. A failed call is not kept, so the next one tries again. */
export function compileTemplate(t: Template): Promise<CompileResponse> {
  let p = compiled.get(t.id);
  if (!p) {
    const call = compileSystem({ source: templateSource(t), format: "latex" });
    call.catch(() => {
      if (compiled.get(t.id) === call) compiled.delete(t.id);
    });
    compiled.set(t.id, call);
    p = call;
  }
  return p;
}

/**
 * The thumbnail, computed once; null when the template does not compile. A
 * failed computation is not kept, and neither is the compile it used: its
 * system may have been evicted from the core's store by the time of the
 * retry, which must then compile afresh to get a live id.
 */
function computeThumb(t: Template): Promise<Thumb | null> {
  let p = cache.get(t.id);
  if (!p) {
    let sysCall: Promise<CompileResponse> | undefined;
    const thumb: Promise<Thumb | null> = (async () => {
      sysCall = compileTemplate(t);
      const sys = await sysCall;
      if (!sys.ok || sys.systemId === undefined) return null;
      const kind = sys.kind ?? "ode";
      const dim = sys.states.length;
      const view = templateView(t, sys);
      const solver = { ...DEFAULT_SOLVER, method: "rk4" as const, h: STEP, tEnd: t.tEnd };
      const layers = { ...DEFAULT_LAYERS, backward: t.backward ?? true };
      const icOf: number[] = [];
      const trajectories = t.initialConditions.flatMap((x0, k) => {
        const ic = { id: k, x0: Array.from({ length: dim }, (_, i) => x0[i] ?? 0), t0: 0, color: k };
        return trajectorySpans(kind, view, ic, solver, layers).map((s) => {
          icOf.push(k);
          return s.req;
        });
      });
      const maxSpan = trajectories.reduce((m, r) => Math.max(m, Math.abs(r.t1 - r.t0)), 0);
      const shown = makeViewport(ASPECT * 100, 100, view.xRange, view.yRange, hasEqualScales(view, kind));
      const section = isSection(view) ? view.section : null;
      const planar = kind === "ode" && dim <= 2 && !section;
      const cobweb: MapRequestFields = isCobweb(view) ? { graphSamples: THUMB_GRAPH_SAMPLES, transient: 0 } : {};
      const crossings = section ? { section, transient: t.transient ?? 0 } : {};
      const res = await computePortrait({
        requestId: nextRequestId++,
        systemId: sys.systemId,
        params: sys.params.map((q) => t.params[q.name] ?? q.default),
        xAxis: view.xAxis,
        yAxis: view.yAxis,
        xRange: shown.xRange,
        yRange: shown.yRange,
        baseState: new Array(dim).fill(0),
        fieldTime: 0,
        fieldGrid: planar ? THUMB_FIELD_GRID : 0,
        nullclines: false,
        fixedPoints: (planar && dim === 2) || isCobweb(view),
        trajectories,
        solver: { method: "rk4", rtol: solver.rtol, atol: solver.atol, h: STEP, maxSteps: solver.maxSteps },
        samples: autoSamples(maxSpan),
        ...cobweb,
        ...crossings,
      });
      return { kind, dim, view, res, icOf };
    })().catch(() => {
      if (cache.get(t.id) === thumb) cache.delete(t.id);
      if (sysCall && compiled.get(t.id) === sysCall) compiled.delete(t.id);
      return null;
    });
    cache.set(t.id, thumb);
    p = thumb;
  }
  return p;
}

/** A small portrait in `canvas`, sized to its client box: the Template gallery's thumbnails and the Parameter plane's inset. */
export function drawThumb(canvas: HTMLCanvasElement, thumb: Thumb) {
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (w === 0 || h === 0) return;
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(h * dpr);
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const th = readPlotTheme();
  const s = th.lineScale;
  const { view, res, kind, dim, icOf } = thumb;
  const vp: Viewport = makeViewport(w, h, view.xRange, view.yRange, hasEqualScales(view, kind));

  ctx.fillStyle = th.bg;
  ctx.fillRect(0, 0, w, h);

  if (isSpace(view)) {
    const lines = res.trajectories.map((out, i) => ({ out, color: icOf[i] ?? i, backward: false, selected: false }));
    drawSpaceLines(ctx, view, w, h, lines, dim, { ...th, lineScale: 0.5 * s });
    return;
  }

  ctx.strokeStyle = th.axis;
  ctx.lineWidth = 1;
  ctx.beginPath();
  const X = Math.round(sx(vp, 0)) + 0.5;
  const Y = Math.round(sy(vp, 0)) + 0.5;
  if (X > 0 && X < w) {
    ctx.moveTo(X, 0);
    ctx.lineTo(X, h);
  }
  if (Y > 0 && Y < h) {
    ctx.moveTo(0, Y);
    ctx.lineTo(w, Y);
  }
  ctx.stroke();

  if (res.arrows.length) {
    const len = 0.55 * (Math.min(w, h) / THUMB_FIELD_GRID);
    const sxu = vp.width / (vp.xRange[1] - vp.xRange[0]);
    const syu = vp.height / (vp.yRange[1] - vp.yRange[0]);
    ctx.strokeStyle = th.field;
    ctx.lineWidth = 1;
    ctx.lineCap = "round";
    ctx.beginPath();
    for (const a of res.arrows) {
      const dx = a.u * sxu;
      const dy = -a.v * syu;
      const m = Math.hypot(dx, dy);
      if (!Number.isFinite(m) || m < 1e-12) continue;
      const cx = sx(vp, a.x);
      const cy = sy(vp, a.y);
      const ux = ((dx / m) * len) / 2;
      const uy = ((dy / m) * len) / 2;
      ctx.moveTo(cx - ux, cy - uy);
      ctx.lineTo(cx + ux, cy + uy);
    }
    ctx.stroke();
  }

  if (isCobweb(view)) {
    drawCobwebThumb(ctx, vp, thumb, th);
    return;
  }

  if (isSection(view)) {
    drawSectionThumb(ctx, vp, thumb, th);
    return;
  }

  const at = (axis: PlotView["xAxis"], tr: PortraitResponse["trajectories"][number], k: number) =>
    axis.kind === "time" ? tr.t[k] : tr.x[k * dim + axis.index];
  const isMap = kind === "map";
  const thin = dim >= 3 || isMap;
  res.trajectories.forEach((tr, i) => {
    const color = th.traj[(icOf[i] ?? i) % th.traj.length];
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = (thin ? 0.75 : 1.25) * s;
    ctx.lineJoin = "round";
    ctx.beginPath();
    let pen = false;
    const pts: [number, number][] = [];
    for (let k = 0; k < tr.t.length; k++) {
      const px = sx(vp, at(view.xAxis, tr, k));
      const py = sy(vp, at(view.yAxis, tr, k));
      if (!Number.isFinite(px) || !Number.isFinite(py) || Math.abs(px) > 1e5 || Math.abs(py) > 1e5) {
        pen = false;
        continue;
      }
      if (pen) ctx.lineTo(px, py);
      else ctx.moveTo(px, py);
      pen = true;
      if (isMap) pts.push([px, py]);
    }
    if (!isMap || view.xAxis.kind === "time" || view.yAxis.kind === "time") ctx.stroke();
    if (isMap) {
      const r = view.xAxis.kind === "time" || view.yAxis.kind === "time" ? 1.25 * s : 0.5 * s;
      for (const [px, py] of pts) {
        ctx.beginPath();
        ctx.arc(px, py, r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  });

  const coord = (axis: PlotView["xAxis"], x: number[], fallback: number) => (axis.kind === "state" ? x[axis.index] : x[fallback]);
  for (const fp of res.fixedPoints) {
    const px = sx(vp, coord(view.xAxis, fp.x, 0));
    const py = sy(vp, coord(view.yAxis, fp.x, 1));
    if (px < 0 || px > w || py < 0 || py > h) continue;
    drawThumbFixedPoint(ctx, px, py, fixedPointGlyph(fp.classification, fp.stable), th);
  }
}

/** A fixed-point glyph at thumbnail size: halo, then the body (filled, open or half). */
function drawThumbFixedPoint(
  ctx: CanvasRenderingContext2D,
  px: number,
  py: number,
  glyph: ReturnType<typeof fixedPointGlyph>,
  th: ReturnType<typeof readPlotTheme>,
) {
  const r = 2.75;
  ctx.fillStyle = th.halo;
  ctx.beginPath();
  ctx.arc(px, py, r + 1.5, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = th.fp;
  ctx.fillStyle = th.fp;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.arc(px, py, r, 0, Math.PI * 2);
  if (glyph === "stable") ctx.fill();
  ctx.stroke();
  if (glyph === "half") {
    ctx.beginPath();
    ctx.arc(px, py, r, Math.PI / 2, (Math.PI * 3) / 2);
    ctx.closePath();
    ctx.fill();
  }
}

function drawSectionThumb(ctx: CanvasRenderingContext2D, vp: Viewport, thumb: Thumb, th: ReturnType<typeof readPlotTheme>) {
  const { view, res, kind, dim, icOf } = thumb;
  if (!isSection(view)) return;
  const returnMap = isReturnMap(view);
  const d = returnMap ? diagonalSegment(vp.xRange, vp.yRange) : null;
  if (d) {
    ctx.strokeStyle = th.axis;
    ctx.lineWidth = 0.75 * th.lineScale;
    ctx.beginPath();
    ctx.moveTo(sx(vp, d[0]), sy(vp, d[1]));
    ctx.lineTo(sx(vp, d[2]), sy(vp, d[3]));
    ctx.stroke();
  }
  const scene = { kind, dim, xAxis: view.xAxis, yAxis: view.yAxis, section: { returnMap } };
  const r = 0.75 * th.lineScale;
  res.trajectories.forEach((tr, i) => {
    ctx.fillStyle = th.traj[(icOf[i] ?? i) % th.traj.length];
    const pts = trajectoryPx(vp, scene, tr);
    ctx.beginPath();
    for (let k = 0; k < pts.length; k += 2) {
      ctx.moveTo(pts[k] + r, pts[k + 1]);
      ctx.arc(pts[k], pts[k + 1], r, 0, Math.PI * 2);
    }
    ctx.fill();
  });
}

/** A cobweb thumbnail: the diagonal, the graph of f and each orbit's staircase (canvas/cobweb.ts). */
function drawCobwebThumb(ctx: CanvasRenderingContext2D, vp: Viewport, thumb: Thumb, th: ReturnType<typeof readPlotTheme>) {
  const s = th.lineScale;
  const trace = (paths: Float64Array[]) => {
    ctx.beginPath();
    for (const p of paths) {
      ctx.moveTo(p[0], p[1]);
      for (let k = 2; k < p.length; k += 2) ctx.lineTo(p[k], p[k + 1]);
    }
    ctx.stroke();
  };
  const d = diagonalSegment(vp.xRange, vp.yRange);
  if (d) {
    ctx.strokeStyle = th.axis;
    ctx.lineWidth = 0.75 * s;
    ctx.lineCap = "butt";
    trace([Float64Array.of(sx(vp, d[0]), sy(vp, d[1]), sx(vp, d[2]), sy(vp, d[3]))]);
  }
  ctx.strokeStyle = th.fp;
  ctx.lineWidth = 1.25 * s;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  trace(graphPx(vp, portraitGraph(thumb.res) ?? []));
  thumb.res.trajectories.forEach((tr, i) => {
    ctx.strokeStyle = th.traj[(thumb.icOf[i] ?? i) % th.traj.length];
    ctx.lineWidth = 0.75 * s;
    ctx.lineJoin = "miter";
    ctx.lineCap = "butt";
    trace(cobwebPx(vp, orbitValues(tr, thumb.dim, 0)));
  });
  // The fixed points on the diagonal, at (x*, x*).
  for (const fp of thumb.res.fixedPoints) {
    const px = sx(vp, fp.x[0]);
    const py = sy(vp, fp.x[0]);
    if (px < 0 || px > vp.left + vp.width || py < 0 || py > vp.top + vp.height) continue;
    drawThumbFixedPoint(ctx, px, py, fixedPointGlyph(fp.classification, fp.stable), th);
  }
}

export function TemplateThumb({ template }: { template: Template }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [thumb, setThumb] = useState<Thumb | null>(null);
  const [near, setNear] = useState(false);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || near) return;
    if (typeof IntersectionObserver === "undefined") {
      setNear(true);
      return;
    }
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && setNear(true));
    io.observe(canvas);
    return () => io.disconnect();
  }, [near]);

  useEffect(() => {
    if (!near) return;
    let live = true;
    void computeThumb(template).then((t) => live && setThumb(t));
    return () => {
      live = false;
    };
  }, [template, near]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !thumb) return;
    const redraw = () => drawThumb(canvas, thumb);
    redraw();
    const mo = new MutationObserver(redraw);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    const ro = new ResizeObserver(redraw);
    ro.observe(canvas);
    let live = true;
    document.fonts?.ready.then(() => live && redraw());
    return () => {
      live = false;
      mo.disconnect();
      ro.disconnect();
    };
  }, [thumb]);

  return (
    <span className="ds-template-card-thumb">
      <canvas ref={canvasRef} aria-hidden="true" />
    </span>
  );
}
