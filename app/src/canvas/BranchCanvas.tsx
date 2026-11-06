import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { Tex } from "../math/Tex";
import { IconInfo } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { isContinuation, shownBranches } from "../state/model";
import { axisLines } from "./draw";
import { drawBranch, nearestBranchVertex, type ShownBranch } from "./branch";
import { usePanZoom, usePlotChrome } from "./usePlotChrome";
import { makeViewport } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  toolbar?: ReactNode;
}

export function BranchCanvas({ state, actions, toolbar }: Props) {
  const { wrapRef, canvasRef, probeRef, size, th, toolbarBox, nameBoxes } = usePlotChrome();
  const { compiled, view } = state;
  const cont = isContinuation(view) ? view : null;
  const param = cont ? state.params[cont.param] : undefined;
  const dim = compiled?.states.length ?? 0;
  const vp = useMemo(() => makeViewport(size.w, size.h, view.xRange, view.yRange), [size.w, size.h, view.xRange, view.yRange]);

  const shown = shownBranches(state);
  const branches: ShownBranch[] = shown.flatMap(({ job, run }) =>
    run?.ok ? [{ branch: run.res.branch, color: job.meta.seed }] : [],
  );

  const axis = cont?.yAxis ?? null;
  const marker = param?.value ?? null;
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !th || size.w === 0 || !axis) return;
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
    drawBranch(ctx, vp, branches, dim, axis, th, { marker, avoid: [...(toolbarBox ? [toolbarBox] : []), ...nameBoxes] });
  }, [th, size.w, size.h, vp, branches, dim, axis, marker, toolbarBox, nameBoxes]);

  const upY = useRef(0);
  const setParamAt = (clientX: number) => {
    const canvas = canvasRef.current;
    if (!canvas || !param || !axis) return;
    const r = canvas.getBoundingClientRect();
    const hit = nearestBranchVertex(vp, branches, dim, axis, clientX - r.left, upY.current - r.top);
    if (hit) actions.setParam(param.name, hit.p);
  };
  const { panning, handlers } = usePanZoom(canvasRef, view, vp, actions, actions.resetView, setParamAt);
  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    upY.current = e.clientY;
    handlers.onPointerUp(e);
  };

  const axes = axisLines(vp);
  const right = vp.left + vp.width;
  const xNameTop = vp.top + vp.height - axes.Y > 28 ? axes.Y + 4 : axes.Y - 24;
  const yNameTop = toolbarBox && axes.X + 8 < toolbarBox[2] ? toolbarBox[3] + 6 : vp.top + 6;
  const yLatex = axis?.kind === "state" ? (compiled?.states[axis.index]?.latex ?? "x") : "\\lVert x \\rVert";
  const label = param
    ? `Branch plot over ${param.name} from ${view.xRange[0].toPrecision(4)} to ${view.xRange[1].toPrecision(4)}; ${param.name} = ${param.value.toPrecision(4)}. Click a branch to set ${param.name}.`
    : "Branch plot";

  return (
    <>
      <div className="plot-wrap" ref={wrapRef}>
        <canvas
          ref={canvasRef}
          className="plot-canvas branch-canvas"
          style={{ cursor: panning ? "grabbing" : "crosshair" }}
          {...handlers}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          tabIndex={0}
          role="application"
          aria-roledescription="Branch plot"
          aria-label={label}
          aria-describedby="branch-keys"
        />
        {toolbar}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="branch-keys" className="ds-sr">
          Arrow keys pan, Shift for bigger steps. Plus and minus zoom, 0 resets the view.
        </span>
        {compiled && param && size.w > 0 && (
          <>
            <div className="axis-label axis-label-x math" style={{ right: size.w - right + 8, top: xNameTop }} aria-hidden="true">
              <Tex latex={param.latex} />
            </div>
            <div className="axis-label axis-label-y math" style={{ left: axes.X + 8, top: yNameTop }} aria-hidden="true">
              <Tex latex={yLatex} />
            </div>
          </>
        )}
        {compiled && cont && cont.continuation.seeds.length === 0 && (
          <div className="plot-hint">
            <div className="ds-diagnostic ds-diagnostic--block ds-diagnostic--info" role="status">
              <IconInfo className="ds-diagnostic-icon" />
              <span className="ds-diagnostic-body">
                <span className="ds-diagnostic-word">Note</span>
                <span className="ds-diagnostic-msg">
                  Select a fixed point in another view, then continue from it in the Continuation panel.
                </span>
              </span>
            </div>
          </div>
        )}
      </div>
      <div className="legend-bar">
        <p className="legend-help caption">
          Each branch is a fixed point followed as {param?.name ?? "the parameter"} changes: solid where it is stable, dashed
          where it is unstable. Click a branch to set {param?.name ?? "the parameter"}, drag to pan, scroll to zoom,
          double-click to reset.
        </p>
      </div>
    </>
  );
}

export default BranchCanvas;
