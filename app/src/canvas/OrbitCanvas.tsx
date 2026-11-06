import { useEffect, useMemo, useSyncExternalStore, type ReactNode } from "react";
import { Tex } from "../math/Tex";
import { IconInfo } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { isOrbit, lyapunovStart, orbitColumnAt, orbitColumns, orbitFits } from "../state/model";
import { axisLines } from "./draw";
import { canvasX, drawOrbit } from "./orbit";
import { usePanZoom, usePlotChrome } from "./usePlotChrome";
import { makeViewport } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  toolbar?: ReactNode;
}

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
  const { wrapRef, canvasRef, probeRef, size, th, toolbarBox, nameBoxes } = usePlotChrome();

  const { compiled, view } = state;
  const orbit = isOrbit(view) ? view : null;
  const run = orbitFits(compiled, state.shapeId, view, state.orbit) ? state.orbit : null;
  const param = orbit ? state.params[orbit.param] : undefined;
  const vp = useMemo(() => makeViewport(size.w, size.h, view.xRange, view.yRange), [size.w, size.h, view.xRange, view.yRange]);

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

  const setParamAt = (clientX: number) => {
    const canvas = canvasRef.current;
    if (!canvas || !param || columns < 2) return;
    const x = canvasX(clientX, canvas.getBoundingClientRect().left, window.devicePixelRatio || 1);
    actions.setParam(param.name, orbitColumnAt(view.xRange, columns, (x - vp.left) / vp.width).r);
  };

  const { panning, handlers } = usePanZoom(canvasRef, view, vp, actions, actions.resetView, setParamAt);

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
          {...handlers}
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
          Each column is the orbit at one {param?.name ?? "parameter"} value after the transient, more opaque where it
          returns more often. Click to set {param?.name ?? "the parameter"}, drag to pan, scroll to zoom, double-click to reset.
        </p>
      </div>
    </>
  );
}

export default OrbitCanvas;
