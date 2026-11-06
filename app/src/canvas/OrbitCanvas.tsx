import { useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { Tex } from "../math/Tex";
import { IconInfo } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { isOrbit, lyapunovStart, orbitColumnAt, orbitColumns, orbitFits } from "../state/model";
import type { Box } from "./draw";
import { axisLines } from "./draw";
import { drawOrbit } from "./orbit";
import { readPlotTheme, type PlotTheme } from "./theme";
import { makeViewport, toPlot, unitsPerPx, zoomRange } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  toolbar?: ReactNode;
}

const KEY_STEP = 0.05;
const CLICK_SLOP = 3;

const sameBox = (a: Box | null, b: Box | null) => (!a || !b ? a === b : a.every((v, i) => Math.abs(v - b[i]) < 0.5));

let drawMs: number | null = null;
const drawListeners = new Set<() => void>();
function setDrawMs(ms: number | null) {
  drawMs = ms;
  drawListeners.forEach((l) => l());
}
export function useOrbitDrawMs(): number | null {
  return useSyncExternalStore(
    (l) => {
      drawListeners.add(l);
      return () => drawListeners.delete(l);
    },
    () => drawMs,
  );
}

export function OrbitCanvas({ state, actions, toolbar }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const probeRef = useRef<HTMLSpanElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [th, setTh] = useState<PlotTheme | null>(null);
  const [toolbarBox, setToolbarBox] = useState<Box | null>(null);
  const [nameBoxes, setNameBoxes] = useState<Box[]>([]);
  const [panning, setPanning] = useState(false);
  const drag = useRef<{ x: number; y: number; xRange: [number, number]; yRange: [number, number]; upp: [number, number] } | null>(null);

  const { compiled, view } = state;
  const orbit = isOrbit(view) ? view : null;
  const run = orbitFits(compiled, state.shapeId, view, state.orbit) ? state.orbit : null;
  const param = orbit ? state.params[orbit.param] : undefined;
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

  const columns = size.w > 0 ? orbitColumns(size.w, window.devicePixelRatio || 1) : 0;
  useEffect(() => {
    if (columns > 0) actions.setOrbitColumns(columns);
  }, [columns, actions]);

  const marker = param?.value ?? null;
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
    const started = performance.now();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = th.bg;
    ctx.fillRect(0, 0, size.w, size.h);
    drawOrbit(ctx, vp, run?.res ?? null, th, {
      pixelRatio: dpr,
      marker,
      avoid: [...(toolbarBox ? [toolbarBox] : []), ...nameBoxes],
    });
    setDrawMs(run ? performance.now() - started : null);
  }, [th, size.w, size.h, vp, run, marker, toolbarBox, nameBoxes]);

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

  const setParamAt = (clientX: number) => {
    const canvas = canvasRef.current;
    if (!canvas || !param || columns < 2) return;
    const [x] = toPlot(vp, clientX - canvas.getBoundingClientRect().left, 0);
    actions.setParam(param.name, orbitColumnAt(view.xRange, columns, x).r);
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, xRange: view.xRange, yRange: view.yRange, upp: unitsPerPx(vp) };
  };
  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d) return;
    if (!panning && Math.hypot(e.clientX - d.x, e.clientY - d.y) <= CLICK_SLOP) return;
    if (!panning) setPanning(true);
    const dx = (e.clientX - d.x) * d.upp[0];
    const dy = (e.clientY - d.y) * d.upp[1];
    actions.setView({
      ...view,
      xRange: [d.xRange[0] - dx, d.xRange[1] - dx],
      yRange: [d.yRange[0] + dy, d.yRange[1] + dy],
    });
  };
  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    drag.current = null;
    if (d && !panning && e.type === "pointerup") setParamAt(e.clientX);
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
      actions.resetView();
    }
  };

  const dim = compiled?.states.length ?? 0;
  const start = lyapunovStart(state.ics, state.selection, dim);
  const axes = axisLines(vp);
  const right = vp.left + vp.width;
  const xNameTop = vp.top + vp.height - axes.Y > 28 ? axes.Y + 4 : axes.Y - 24;
  const yNameTop = toolbarBox && axes.X + 8 < toolbarBox[2] ? toolbarBox[3] + 6 : vp.top + 6;
  const stateLatex = orbit && compiled ? compiled.states[orbit.yAxis.index]?.latex ?? "x" : "x";
  const label = param
    ? `Orbit diagram of ${param.name} from ${view.xRange[0].toPrecision(4)} to ${view.xRange[1].toPrecision(4)}; ${param.name} = ${param.value.toPrecision(4)}. Click to set ${param.name}.`
    : "Orbit diagram";

  return (
    <>
      <div className="plot-wrap" ref={wrapRef}>
        <canvas
          ref={canvasRef}
          className="plot-canvas orbit-canvas"
          style={{ cursor: panning ? "grabbing" : "crosshair" }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onDoubleClick={actions.resetView}
          onKeyDown={onKeyDown}
          tabIndex={0}
          role="application"
          aria-roledescription="Orbit diagram"
          aria-label={label}
          aria-describedby="orbit-keys"
          data-columns={columns || undefined}
        />
        {toolbar}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="orbit-keys" className="ds-sr">
          Arrow keys pan, Shift for bigger steps. Plus and minus zoom, 0 resets the view.
        </span>
        {compiled && param && size.w > 0 && (
          <>
            <div className="axis-label axis-label-x math" style={{ right: size.w - right + 8, top: xNameTop }} aria-hidden="true">
              <Tex latex={param.latex} />
            </div>
            <div className="axis-label axis-label-y math" style={{ left: axes.X + 8, top: yNameTop }} aria-hidden="true">
              <Tex latex={stateLatex} />
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
                  Add a start point in the Trajectories panel or another view: every column of the diagram starts there.
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
          Each column is the orbit at one {param?.name ?? "parameter"} value after the transient, darker where it returns
          more often. Click to set {param?.name ?? "the parameter"}, drag to pan, scroll to zoom, double-click to reset.
        </p>
      </div>
    </>
  );
}

export default OrbitCanvas;
