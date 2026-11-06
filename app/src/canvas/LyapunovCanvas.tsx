import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Tex } from "../math/Tex";
import { IconInfo } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { fitLyapunovRanges, isLyapunov, lyapunovFits, lyapunovStart, type LyapunovState } from "../state/model";
import type { Box } from "./draw";
import { axisLines } from "./draw";
import { drawLyapunov, lambdaName } from "./lyapunov";
import { readPlotTheme, type PlotTheme } from "./theme";
import { makeViewport, toPlot, unitsPerPx, zoomRange } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  toolbar?: ReactNode;
}

const KEY_STEP = 0.05;

const sameBox = (a: Box | null, b: Box | null) => (!a || !b ? a === b : a.every((v, i) => Math.abs(v - b[i]) < 0.5));

/** The running estimates of the Lyapunov spectrum against t (or n): zoom and pan as on the phase plane. */
export function LyapunovCanvas({ state, actions, toolbar }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const probeRef = useRef<HTMLSpanElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [th, setTh] = useState<PlotTheme | null>(null);
  const [toolbarBox, setToolbarBox] = useState<Box | null>(null);
  const [nameBoxes, setNameBoxes] = useState<Box[]>([]);
  const [panning, setPanning] = useState(false);
  const drag = useRef<{ x: number; y: number; xRange: [number, number]; yRange: [number, number]; upp: [number, number] } | null>(null);
  const fitted = useRef<LyapunovState | null>(null);

  const { compiled, view } = state;
  const kind = compiled?.kind;
  const run = lyapunovFits(compiled, state.shapeId, state.lyapunov) ? state.lyapunov : null;
  const vp = useMemo(() => makeViewport(size.w, size.h, view.xRange, view.yRange), [size.w, size.h, view.xRange, view.yRange]);

  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  useLayoutEffect(() => {
    const read = () => setTh(readPlotTheme(probeRef.current));
    read();
    const mo = new MutationObserver(read);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    let alive = true;
    document.fonts?.ready.then(() => alive && read());
    return () => {
      alive = false;
      mo.disconnect();
    };
  }, []);

  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    const bar = wrap?.querySelector<HTMLElement>(".plot-toolbar");
    if (!wrap) return;
    const wr = wrap.getBoundingClientRect();
    const rel = (r: DOMRect): Box => [r.left - wr.left, r.top - wr.top, r.right - wr.left, r.bottom - wr.top];
    const tb = bar ? rel(bar.getBoundingClientRect()) : null;
    if (!sameBox(tb, toolbarBox)) setToolbarBox(tb);
    const names = [...wrap.querySelectorAll<HTMLElement>(".axis-label")].map((el) => rel(el.getBoundingClientRect()));
    if (names.length !== nameBoxes.length || names.some((b, i) => !sameBox(b, nameBoxes[i]))) setNameBoxes(names);
  });

  // Each new run is fitted once; zooming and panning after that are the user's.
  useEffect(() => {
    if (!run || fitted.current === run) return;
    fitted.current = run;
    const fit = fitLyapunovRanges(run.res);
    if (fit && isLyapunov(view)) actions.autoAdjustView({ ...view, ...fit });
  }, [run, view, actions]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !th || size.w === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(size.w * dpr);
    const H = Math.round(size.h * dpr);
    if (canvas.width !== W || canvas.height !== H) {
      canvas.width = W;
      canvas.height = H;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = th.bg;
    ctx.fillRect(0, 0, size.w, size.h);
    drawLyapunov(ctx, vp, run?.res ?? null, th, undefined, [...(toolbarBox ? [toolbarBox] : []), ...nameBoxes]);
  }, [th, size.w, size.h, vp, run, toolbarBox, nameBoxes]);

  const latest = useRef({ view, vp, actions });
  latest.current = { view, vp, actions };
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const { view: v, vp: port, actions: act } = latest.current;
      const r = canvas.getBoundingClientRect();
      const [ax, ay] = toPlot(port, e.clientX - r.left, e.clientY - r.top);
      const delta = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
      const f = Math.exp(Math.max(-0.5, Math.min(0.5, delta * 0.0015)));
      if (f === 1) return;
      const xRange = zoomRange(v.xRange, ax, f);
      const yRange = zoomRange(v.yRange, ay, f);
      if (xRange && yRange) act.setView({ ...v, xRange, yRange });
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });
    return () => canvas.removeEventListener("wheel", onWheel);
  }, []);

  const refit = () => {
    const fit = run ? fitLyapunovRanges(run.res) : null;
    if (fit) actions.setView({ ...view, ...fit });
    else actions.resetView();
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, xRange: view.xRange, yRange: view.yRange, upp: unitsPerPx(vp) };
  };
  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d) return;
    const dx = (e.clientX - d.x) * d.upp[0];
    const dy = (e.clientY - d.y) * d.upp[1];
    if (!panning && Math.hypot(e.clientX - d.x, e.clientY - d.y) > 3) setPanning(true);
    actions.setView({
      ...view,
      xRange: [d.xRange[0] - dx, d.xRange[1] - dx],
      yRange: [d.yRange[0] + dy, d.yRange[1] + dy],
    });
  };
  const onPointerUp = () => {
    drag.current = null;
    setPanning(false);
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const sx = (view.xRange[1] - view.xRange[0]) * KEY_STEP * (e.shiftKey ? 5 : 1);
    const sy = (view.yRange[1] - view.yRange[0]) * KEY_STEP * (e.shiftKey ? 5 : 1);
    const pan: Record<string, [number, number]> = { ArrowLeft: [-sx, 0], ArrowRight: [sx, 0], ArrowUp: [0, sy], ArrowDown: [0, -sy] };
    const c: [number, number] = [(view.xRange[0] + view.xRange[1]) / 2, (view.yRange[0] + view.yRange[1]) / 2];
    if (e.key in pan) {
      e.preventDefault();
      const [mx, my] = pan[e.key];
      actions.setView({ ...view, xRange: [view.xRange[0] + mx, view.xRange[1] + mx], yRange: [view.yRange[0] + my, view.yRange[1] + my] });
    } else if (e.key === "+" || e.key === "=" || e.key === "-" || e.key === "_" || e.key === "−") {
      e.preventDefault();
      const f = e.key === "+" || e.key === "=" ? 1 / 1.25 : 1.25;
      const xRange = zoomRange(view.xRange, c[0], f);
      const yRange = zoomRange(view.yRange, c[1], f);
      if (xRange && yRange) actions.setView({ ...view, xRange, yRange });
    } else if (e.key === "0") {
      e.preventDefault();
      refit();
    }
  };

  const dim = compiled?.states.length ?? 0;
  const start = lyapunovStart(state.ics, state.selection, dim);
  const time = kind === "map" ? "n" : "t";
  const axes = axisLines(vp);
  const right = vp.left + vp.width;
  // Below the axis: each λᵢ's name sits above its dashed line at the right edge, λ = 0 among them.
  const xNameTop = vp.top + vp.height - axes.Y > 28 ? axes.Y + 4 : axes.Y - 24;
  const yNameTop = toolbarBox && axes.X + 8 < toolbarBox[2] ? toolbarBox[3] + 6 : vp.top + 6;
  const res = run?.res;
  const label = res?.exponents.length
    ? `Lyapunov spectrum against ${time}: ${res.exponents.map((v, i) => `${lambdaName(i)} ≈ ${v.toPrecision(3).replace("-", "−")}`).join(", ")}.`
    : "Lyapunov spectrum";

  return (
    <>
      <div className="plot-wrap" ref={wrapRef}>
        <canvas
          ref={canvasRef}
          className="plot-canvas lyapunov-canvas"
          style={{ cursor: panning ? "grabbing" : "grab" }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onDoubleClick={refit}
          onKeyDown={onKeyDown}
          tabIndex={0}
          role="application"
          aria-roledescription="Lyapunov spectrum"
          aria-label={label}
          aria-describedby="lyapunov-keys"
        />
        {toolbar}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="lyapunov-keys" className="ds-sr">
          Arrow keys pan, Shift for bigger steps. Plus and minus zoom, 0 fits the run.
        </span>
        {compiled && size.w > 0 && (
          <>
            <div className="axis-label axis-label-x math" style={{ right: size.w - right + 8, top: xNameTop }} aria-hidden="true">
              <Tex latex={time} />
            </div>
            <div className="axis-label axis-label-y math" style={{ left: axes.X + 8, top: yNameTop }} aria-hidden="true">
              <Tex latex="\lambda" />
            </div>
          </>
        )}
        {compiled && !start && (
          <div className="plot-hint">
            <div className="ds-diagnostic ds-diagnostic--block ds-diagnostic--info" role="status">
              <IconInfo className="ds-diagnostic-icon" />
              <span className="ds-diagnostic-body">
                <span className="ds-diagnostic-word">Note</span>
                <span className="ds-diagnostic-msg">
                  Add a start point in the Trajectories panel or another view to measure its Lyapunov spectrum.
                </span>
              </span>
            </div>
          </div>
        )}
        {state.computeError && (
          <div className="plot-hint">
            <div className="ds-diagnostic ds-diagnostic--block ds-diagnostic--error" role="alert">
              <span className="ds-diagnostic-body">
                <span className="ds-diagnostic-word">Error</span>
                <span className="ds-diagnostic-msg">Could not compute: {state.computeError}</span>
              </span>
            </div>
          </div>
        )}
      </div>
      <div className="legend-bar">
        <p className="legend-help caption">
          Running estimates of each exponent; dashed, the final value. Drag to pan, scroll to zoom, double-click to fit.
        </p>
      </div>
    </>
  );
}

export default LyapunovCanvas;
