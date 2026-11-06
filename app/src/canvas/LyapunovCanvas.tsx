import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { Tex } from "../math/Tex";
import { IconInfo } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { fitLyapunovRanges, isLyapunov, lyapunovFits, lyapunovStart, type LyapunovState } from "../state/model";
import { axisLines } from "./draw";
import { drawLyapunov, lambdaName } from "./lyapunov";
import { usePanZoom, usePlotChrome } from "./usePlotChrome";
import { makeViewport } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  toolbar?: ReactNode;
}

export function LyapunovCanvas({ state, actions, toolbar }: Props) {
  const { wrapRef, canvasRef, probeRef, size, th, toolbarBox, nameBoxes } = usePlotChrome();
  const fitted = useRef<LyapunovState | null>(null);

  const { compiled, view } = state;
  const kind = compiled?.kind;
  const run = lyapunovFits(compiled, state.shapeId, state.lyapunov) ? state.lyapunov : null;
  const vp = useMemo(() => makeViewport(size.w, size.h, view.xRange, view.yRange), [size.w, size.h, view.xRange, view.yRange]);

  useEffect(() => {
    if (!run || fitted.current === run) return;
    fitted.current = run;
    if (run.framedByFile) return;
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

  const refit = () => {
    const fit = run ? fitLyapunovRanges(run.res) : null;
    if (fit) actions.setView({ ...view, ...fit });
    else actions.resetView();
  };
  const { panning, handlers } = usePanZoom(canvasRef, view, vp, actions, refit);

  const dim = compiled?.states.length ?? 0;
  const start = lyapunovStart(state.ics, state.selection, dim);
  const time = kind === "map" ? "n" : "t";
  const axes = axisLines(vp);
  const right = vp.left + vp.width;
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
          {...handlers}
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
