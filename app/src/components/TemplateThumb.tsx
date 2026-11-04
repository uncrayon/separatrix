// The small real phase portrait on a template card (design system: TemplateCard,
// "The thumbnail").
//
// Drawn from the template data so it never goes stale: the template is compiled
// by the same core as the app, and its own view window, initial conditions and
// end time are integrated with RK4 (maps are iterated). Colours are read from
// the tokens at draw time, and the thumbnail redraws when the theme changes.
// Quiet on purpose: field marks 1px with no heads, trajectories 1.25px ×
// line-scale, fixed-point glyphs radius 2.75, no tick labels and no text.

import { useEffect, useRef, useState } from "react";
import { compileSystem, computePortrait } from "../api/client";
import type { PortraitResponse, SystemKind } from "../api/types";
import { fixedPointGlyph } from "../canvas/draw";
import { readPlotTheme } from "../canvas/theme";
import { makeViewport, sx, sy, wantsEqualScales, type Viewport } from "../canvas/viewport";
import { DEFAULT_LAYERS, DEFAULT_SOLVER, autoSamples, templateView, trajectorySpans, type View } from "../state/model";
import { templateSource, type Template } from "../templates";

/** The well is 16:10 (TemplateCard README). */
const ASPECT = 1.6;
/** RK4 step: fine enough for every gallery template at thumbnail size. */
const STEP = 0.01;

interface Thumb {
  kind: SystemKind;
  dim: number;
  view: View;
  res: PortraitResponse;
  /** The start point (and so the colour) of each returned run. */
  icOf: number[];
}

/** One computation per template per session. */
const cache = new Map<string, Promise<Thumb | null>>();
let nextRequestId = 1_000_000_000;

function computeThumb(t: Template): Promise<Thumb | null> {
  let p = cache.get(t.id);
  if (!p) {
    p = (async () => {
      const sys = await compileSystem({ source: templateSource(t), format: "latex" });
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
      const shown = makeViewport(ASPECT * 100, 100, view.xRange, view.yRange, 1, wantsEqualScales(view.xAxis, view.yAxis, kind));
      const planar = kind === "ode" && dim <= 2;
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
        // A 2D (or 1D) flow gets its direction field; 3D projections and maps do not.
        fieldGrid: planar ? 13 : 0,
        nullclines: false,
        fixedPoints: planar && dim === 2,
        trajectories,
        solver: { method: "rk4", rtol: solver.rtol, atol: solver.atol, h: STEP, maxSteps: solver.maxSteps },
        samples: autoSamples(maxSpan),
      });
      return { kind, dim, view, res, icOf };
    })().catch(() => null);
    cache.set(t.id, p);
  }
  return p;
}

function draw(canvas: HTMLCanvasElement, thumb: Thumb) {
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
  const vp: Viewport = makeViewport(w, h, view.xRange, view.yRange, 1, wantsEqualScales(view.xAxis, view.yAxis, kind));

  ctx.fillStyle = th.bg;
  ctx.fillRect(0, 0, w, h);

  // Axes through the origin, where they are in view.
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

  // Direction field: 1px marks, no heads, all the same length.
  if (res.arrows.length) {
    const len = 0.55 * (Math.min(w, h) / 13);
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

  // Trajectories in traj-* order; a map's orbit is dots joined by a thin line.
  const at = (axis: View["xAxis"], tr: PortraitResponse["trajectories"][number], k: number) =>
    axis.kind === "time" ? tr.t[k] : tr.x[k * dim + axis.index];
  const isMap = kind === "map";
  const thin = dim >= 3 || isMap;
  res.trajectories.forEach((tr, i) => {
    // Forward and backward runs of one start point share its colour.
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
    // A 2D map's orbit (Hénon) is a cloud of dots, not a curve.
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

  // Fixed points: Strogatz glyphs, radius 2.75, on a plot-halo disc.
  for (const fp of res.fixedPoints) {
    const px = sx(vp, fp.x[0]);
    const py = sy(vp, fp.x[1]);
    if (px < 0 || px > w || py < 0 || py > h) continue;
    const glyph = fixedPointGlyph(fp.classification, fp.stable);
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
}

/** `<span class="ds-template-card-thumb"><canvas aria-hidden="true"/></span>` for one template. */
export function TemplateThumb({ template }: { template: Template }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [thumb, setThumb] = useState<Thumb | null>(null);

  useEffect(() => {
    let live = true;
    void computeThumb(template).then((t) => live && setThumb(t));
    return () => {
      live = false;
    };
  }, [template]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !thumb) return;
    const redraw = () => draw(canvas, thumb);
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
