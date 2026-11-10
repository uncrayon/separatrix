import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { Tex } from "../math/Tex";
import { plainSubscripted } from "../math/format";
import { Button } from "../components/controls";
import { IconFollow } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import {
  CURVE_KINDS,
  CURVE_KIND_ORDER,
  HOMOCLINIC_LEGEND,
  HOMOCLINIC_NAME,
  NEUTRAL_LEGEND,
  drawnCurves,
  planePointRows,
  ticksText,
} from "../state/curves";
import { drawnHomoclinic } from "../state/homoclinic";
import { isCurves } from "../state/model";
import { axisLines } from "./draw";
import {
  CURVE_STYLES,
  CURVE_WIDTH,
  NEUTRAL_STYLE,
  cornerBoxes,
  drawCurves,
  emptiestCorner,
  readCurveColors,
  segmentRuns,
  type CurveStyleKey,
} from "./curves";
import { INSET_CAPTION_H, INSET_MARGIN, PlaneInset, insetWidth } from "./PlaneInset";
import { INSET_ASPECT } from "../state/inset";
import { usePanZoom, usePlotChrome } from "./usePlotChrome";
import { makeViewport, sx, sy, toPlot } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  toolbar?: ReactNode;
}

/** A legend or list sample of a curve kind, in its colour and pattern. */
export function CurveSample({ kind, neutral }: { kind: CurveStyleKey; neutral?: boolean }) {
  const dash = neutral ? NEUTRAL_STYLE.dash : CURVE_STYLES[kind].dash;
  return (
    <svg viewBox="0 0 24 16" aria-hidden="true" className="curve-sample">
      <path
        d="M2 8H22"
        fill="none"
        style={{
          stroke: `var(${CURVE_STYLES[kind].token})`,
          strokeWidth: `calc(${neutral ? NEUTRAL_STYLE.width : CURVE_WIDTH}px * var(--line-scale))`,
          strokeLinecap: neutral ? "round" : "butt",
          strokeDasharray: dash.length ? dash.map((d) => `calc(${d}px * var(--line-scale))`).join(" ") : undefined,
          opacity: neutral ? NEUTRAL_STYLE.alpha : undefined,
        }}
      />
    </svg>
  );
}

const SliceSample = () => (
  <svg viewBox="0 0 24 16" aria-hidden="true">
    <path d="M3 8H21M3 5V11M21 5V11" fill="none" style={{ stroke: "var(--plot-tick)", strokeWidth: "calc(1px * var(--line-scale))" }} />
  </svg>
);

const FALLBACK_RANGES: [[number, number], [number, number]] = [
  [0, 1],
  [0, 1],
];

export function CurvesCanvas({ state, actions, toolbar }: Props) {
  const { wrapRef, canvasRef, probeRef, size, th, toolbarBox, nameBoxes } = usePlotChrome();
  const { compiled, view } = state;
  const plane = isCurves(view) ? view : null;
  const across = plane ? state.params[plane.across] : undefined;
  const up = plane ? state.params[plane.up] : undefined;
  const aLatex = (plane && compiled?.params[plane.across]?.latex) || across?.name || "p";
  const uLatex = (plane && compiled?.params[plane.up]?.latex) || up?.name || "q";
  const vp = useMemo(() => makeViewport(size.w, size.h, view.xRange, view.yRange), [size.w, size.h, view.xRange, view.yRange]);
  const [colors, setColors] = useState<Record<CurveStyleKey, string> | null>(null);
  useEffect(() => setColors(readCurveColors()), [th]);

  const ranges = plane?.curves.ranges ?? FALLBACK_RANGES;
  const curves = useMemo(() => drawnCurves(state.planeRows), [state.planeRows]);
  const homoclinic = useMemo(() => drawnHomoclinic(state.homoclinicRows), [state.homoclinicRows]);
  const points = useMemo(() => planePointRows(curves, ranges), [curves, ranges]);
  const branch = plane?.branch ?? null;
  const sliceOn = !!plane && !!branch && branch.param === plane.across;
  const sliceAt = plane?.branchParams?.[plane.up];
  const slice = useMemo(
    () => (sliceOn && sliceAt !== undefined ? { range: branch!.continuation.range, at: sliceAt } : null),
    [sliceOn, branch, sliceAt],
  );
  const acrossValue = across?.value;
  const upValue = up?.value;
  const crosshair = useMemo<[number, number] | null>(
    () => (acrossValue !== undefined && upValue !== undefined ? [acrossValue, upValue] : null),
    [acrossValue, upValue],
  );
  const avoid = useMemo(() => [...(toolbarBox ? [toolbarBox] : []), ...nameBoxes], [toolbarBox, nameBoxes]);
  const empty = !!plane && plane.curves.seeds.length === 0 && plane.curves.homoclinic.length === 0;
  const inset = state.inset;
  const insetW = insetWidth(vp.width);
  const insetTop = (toolbarBox ? toolbarBox[3] : vp.top) + 8;
  const corners = useMemo(
    () => cornerBoxes(vp, [...curves, ...homoclinic], insetW, insetW / INSET_ASPECT + INSET_CAPTION_H, insetTop, INSET_MARGIN),
    [vp, curves, homoclinic, insetW, insetTop],
  );
  const corner = emptiestCorner(corners, crosshair && [sx(vp, crosshair[0]), sy(vp, crosshair[1])]);
  const ticks = useMemo(() => ticksText(curves.flatMap((c) => c.ticks.map((t) => t.q))), [curves]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !th || !colors || size.w === 0 || !plane) return;
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
    if (empty) return;
    drawCurves(
      ctx,
      vp,
      curves,
      th,
      {
        colors,
        points,
        crosshair,
        slice: slice && { range: [slice.range[0], slice.range[1]], at: slice.at },
        avoid,
      },
      homoclinic,
    );
  }, [canvasRef, th, colors, size.w, size.h, plane, empty, vp, curves, homoclinic, points, crosshair, slice, avoid]);

  const upY = useRef(0);
  const setAt = (clientX: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const r = canvas.getBoundingClientRect();
    const [a, b] = toPlot(vp, clientX - r.left, upY.current - r.top);
    actions.setPlaneParams(a, b);
  };
  const { panning, handlers } = usePanZoom(canvasRef, view, vp, actions, actions.resetView, setAt);
  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    upY.current = e.clientY;
    handlers.onPointerUp(e);
  };

  const axes = axisLines(vp);
  const right = vp.left + vp.width;
  const xNameTop = vp.top + vp.height - axes.Y > 28 ? axes.Y + 4 : axes.Y - 24;
  const yNameTop = toolbarBox && axes.X + 8 < toolbarBox[2] ? toolbarBox[3] + 6 : vp.top + 6;
  const aName = plainSubscripted(aLatex);
  const uName = plainSubscripted(uLatex);
  const label = plane
    ? `Parameter plane, ${aName} across from ${view.xRange[0].toPrecision(4)} to ${view.xRange[1].toPrecision(4)}, ${uName} up from ${view.yRange[0].toPrecision(4)} to ${view.yRange[1].toPrecision(4)}; ` +
      `${[...curves.map((c) => CURVE_KINDS[c.kind].name), ...homoclinic.map(() => HOMOCLINIC_NAME)].join(", ") || "no curve"}. ${aName} = ${across?.value.toPrecision(4)}, ${uName} = ${up?.value.toPrecision(4)}. Click to set both.`
    : "Parameter plane";

  const kinds = CURVE_KIND_ORDER.filter((k) => curves.some((c) => c.kind === k));
  const neutralKind = CURVE_KIND_ORDER.find((k) => curves.some((c) => c.kind === k && segmentRuns(c.neutral).some((r) => r[2])));
  const sliceText = slice ? Number(slice.at.toPrecision(4)).toString().replace(/^-/, "−") : "";

  return (
    <>
      <div className="plot-wrap" ref={wrapRef}>
        <canvas
          ref={canvasRef}
          className="plot-canvas curves-canvas"
          style={{ cursor: panning ? "grabbing" : "crosshair" }}
          {...handlers}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          tabIndex={0}
          role="application"
          aria-roledescription="Parameter plane"
          aria-label={label}
          aria-describedby="curves-keys"
        />
        {toolbar}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="curves-keys" className="ds-sr">
          Arrow keys pan, Shift for bigger steps. Plus and minus zoom, 0 resets the view.
        </span>
        {compiled && plane && !empty && size.w > 0 && (
          <>
            <div className="axis-label axis-label-x math" style={{ right: size.w - right + 8, top: xNameTop }} aria-hidden="true">
              <Tex latex={aLatex} />
            </div>
            <div className="axis-label axis-label-y math" style={{ left: axes.X + 8, top: yNameTop }} aria-hidden="true">
              <Tex latex={uLatex} />
            </div>
          </>
        )}
        {compiled && plane && !empty && inset && size.w > 0 && (
          <PlaneInset
            inset={inset}
            names={[aName, uName]}
            shown={[plane.across, plane.up]}
            stale={inset.meta.params.some((v, i) => v !== state.params[i]?.value)}
            corner={corner}
            width={insetW}
            top={insetTop}
          />
        )}
        {compiled && empty && (
          <div className="plot-empty" role="status">
            <IconFollow className="plot-empty-icon" />
            <p className="body-strong plot-empty-title">No bifurcation to follow yet</p>
            <p className="body plot-empty-body">
              Find one in the Branch view: a fold, Hopf, flip or Neimark–Sacker point. The search starts from those.
            </p>
            <Button variant="primary" size="sm" onClick={() => actions.setViewKind("continuation")}>
              Go to the Branch view
            </Button>
          </div>
        )}
      </div>
      <div className="legend-bar">
        {(kinds.length > 0 || homoclinic.length > 0 || slice) && (
          <ul className="ds-phase-legend caption" aria-label="Legend">
            {kinds.map((k) => (
              <li className="ds-phase-key" key={k}>
                <span className="ds-phase-sample">
                  <CurveSample kind={k} />
                </span>
                {CURVE_KINDS[k].legend}
              </li>
            ))}
            {homoclinic.length > 0 && (
              <li className="ds-phase-key" key="homoclinic">
                <span className="ds-phase-sample">
                  <CurveSample kind="homoclinic" />
                </span>
                {HOMOCLINIC_LEGEND}
              </li>
            )}
            {neutralKind && (
              <li className="ds-phase-key" key="neutral">
                <span className="ds-phase-sample">
                  <CurveSample kind={neutralKind} neutral />
                </span>
                {NEUTRAL_LEGEND}
              </li>
            )}
            {slice && (
              <li className="ds-phase-key" key="slice">
                <span className="ds-phase-sample">
                  <SliceSample />
                </span>
                Branch view’s range, {uName} = {sliceText}
              </li>
            )}
          </ul>
        )}
        <p className="legend-help caption">
          {ticks && `${ticks} `}
          Click anywhere to set both parameters; the sliders and every view follow. Drag to pan, scroll to zoom,
          double-click to reset.
        </p>
      </div>
    </>
  );
}

export default CurvesCanvas;
