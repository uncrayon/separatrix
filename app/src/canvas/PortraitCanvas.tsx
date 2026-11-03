// The centre of the app: the phase portrait canvas and its overlays
// (typeset axis labels, fixed-point tooltip, legend, slice sliders, hints).
//
// Interactions: click = add an initial condition, drag a dot = move it (live
// re-solve), drag empty space = pan, wheel = zoom around the cursor,
// double-click = reset the view.
//
// Keyboard (the canvas is focusable): arrow keys move a cursor ring (Shift =
// bigger steps), Enter/Space adds a trajectory there or selects the mark under
// it, F / Shift+F step through fixed points, + / − zoom around the cursor,
// 0 resets the view, Delete removes the selected trajectory, Escape deselects.

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { AxisRef, FixedPointOut, SymbolInfo, SystemKind } from "../api/types";
import { Tex } from "../math/Tex";
import { axisLatex, classificationTitle, complexTex, fmtTex, sameAxis } from "../math/format";
import { IconChevron } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { stateExtent } from "../state/model";
import { NumberField, Slider } from "../components/controls";
import {
  drawScene,
  fixedPointPx,
  fixedPointShape,
  icPx,
  trajectoryPx,
  type FixedPointShape,
  type Scene,
} from "./draw";
import { readPlotTheme, type PlotTheme } from "./theme";
import { insidePlot, makeViewport, sx, sy, toPlot, unitsPerPx, zoomRange } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  theme: "dark" | "light";
}

type Drag =
  | { mode: "ic"; id: number }
  | {
      mode: "pan";
      startX: number;
      startY: number;
      xRange: [number, number];
      yRange: [number, number];
      moved: boolean;
    };

const HIT_RADIUS = 10;
/** Below this plot width (CSS px) the slice card starts folded. */
const SLICE_CARD_MIN_PLOT_W = 640;
/** Keyboard cursor step, as a fraction of the visible range (Shift: ×5). */
const KEY_STEP = 1 / 40;

/** The LaTeX of `d/dt` applied to a state: x → \dot{x}, \dot{θ} → \ddot{θ}. */
export function dotted(latex: string): string {
  const m = /^\\dot\{(.*)\}$/.exec(latex);
  return m ? `\\ddot{${m[1]}}` : `\\dot{${latex}}`;
}

/** Axis label: the user's name, with `_n` for map values plotted against n. */
function axisLabel(
  axis: AxisRef,
  other: AxisRef,
  states: SymbolInfo[],
  kind: SystemKind | undefined,
): string {
  const base = axisLatex(axis, states, kind);
  return kind === "map" && axis.kind === "state" && other.kind === "time" ? `${base}_n` : base;
}

export function PortraitCanvas({ state, actions, theme }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [hover, setHover] = useState<{ fp: number | null; ic: number | null }>({ fp: null, ic: null });
  const [dragging, setDragging] = useState<Drag["mode"] | null>(null);
  const drag = useRef<Drag | null>(null);
  const recentAdds = useRef<{ id: number; at: number }[]>([]);
  const themeCache = useRef<{ key: string; th: PlotTheme } | null>(null);
  /** Keyboard cursor in plot coordinates; null when not using the keyboard. */
  const [kbd, setKbd] = useState<[number, number] | null>(null);
  /** Last keyboard action, read out by screen readers. */
  const [announce, setAnnounce] = useState("");
  /**
   * The slice card can be folded into one line so it never hides the plot.
   * Until the user chooses (null), it starts folded on narrow plots, where the
   * 236 px card would cover a large part of the picture.
   */
  const [sliceChoice, setSliceOpen] = useState<boolean | null>(null);
  const sliceOpen = sliceChoice ?? size.w >= SLICE_CARD_MIN_PLOT_W;

  const { compiled, view } = state;
  const scale = theme === "light" ? 1.25 : 1;
  const vp = makeViewport(size.w, size.h, view.xRange, view.yRange, scale);

  // ---- size (CSS px), kept in sync with the layout
  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  // ---- what to draw
  const scene = useMemo<Scene | null>(() => {
    if (!compiled || !compiled.ok) return null;
    const p = state.portrait;
    const dim = compiled.states.length;
    const valid = !!p && p.meta.systemId === compiled.systemId;
    const sameAxes =
      valid && sameAxis(p.meta.view.xAxis, view.xAxis) && sameAxis(p.meta.view.yAxis, view.yAxis);
    const colorOf = new Map(state.ics.map((ic) => [ic.id, ic.color]));
    const trajectories = valid
      ? p.res.trajectories.flatMap((out, i) => {
          const meta = p.meta.trajectories[i];
          const color = meta ? colorOf.get(meta.icId) : undefined;
          return meta && color !== undefined ? [{ out, meta, color }] : [];
        })
      : [];
    return {
      kind: compiled.kind ?? "ode",
      dim,
      xAxis: view.xAxis,
      yAxis: view.yAxis,
      arrows: sameAxes && state.layers.field ? p.res.arrows : [],
      fieldGrid: state.layers.fieldGrid,
      nullclines: sameAxes && state.layers.nullclines ? p.res.nullclines : [],
      fixedPoints: valid && state.layers.fixedPoints ? p.res.fixedPoints : [],
      trajectories,
      ics: state.ics,
      selectedIc: state.selection?.kind === "ic" ? state.selection.id : null,
      selectedFp: state.selection?.kind === "fp" ? state.selection.index : null,
      hoverIc: hover.ic,
      hoverFp: hover.fp,
      stale: !state.compileOk,
    };
  }, [
    compiled,
    state.portrait,
    state.ics,
    state.layers,
    state.selection,
    state.compileOk,
    view.xAxis,
    view.yAxis,
    hover,
  ]);

  // ---- draw (after layout effects, so the theme attribute is already applied)
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || size.w === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(size.w * dpr);
    const H = Math.round(size.h * dpr);
    if (canvas.width !== W || canvas.height !== H) {
      canvas.width = W;
      canvas.height = H;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    if (themeCache.current?.key !== theme) themeCache.current = { key: theme, th: readPlotTheme() };
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawScene(ctx, size.w, size.h, vp, scene, themeCache.current.th);
  });

  // ---- hit testing
  const localPoint = (e: { clientX: number; clientY: number }): [number, number] => {
    const r = canvasRef.current!.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  /**
   * The start dot or fixed point under (px, py): the NEAREST mark within the
   * hit radius wins, so a fixed point stays clickable when a start dot sits a
   * few px away (Van der Pol's origin and its (0.1, 0) start point). On an
   * exact tie the start dot wins, so a dot placed on a fixed point can still
   * be dragged off it.
   */
  const hitTest = (px: number, py: number): { ic: number | null; fp: number | null } => {
    const none = { ic: null, fp: null };
    if (!scene) return none;
    let best: { ic: number | null; fp: number | null } = none;
    let bestD = HIT_RADIUS * HIT_RADIUS * scale * scale;
    for (let i = 0; i < scene.fixedPoints.length; i++) {
      const [x, y] = fixedPointPx(vp, scene, scene.fixedPoints[i]);
      const d = (x - px) ** 2 + (y - py) ** 2;
      if (d <= bestD) {
        bestD = d;
        best = { ic: null, fp: i };
      }
    }
    // Start dots after fixed points, oldest to newest, with `<=`: ties go to
    // the dot, and between dots to the newest (the one drawn on top).
    for (let i = 0; i < scene.ics.length; i++) {
      const ic = scene.ics[i];
      if (ic.x0.length !== scene.dim) continue;
      const [x, y] = icPx(vp, scene, ic);
      const d = (x - px) ** 2 + (y - py) ** 2;
      if (d <= bestD) {
        bestD = d;
        best = { ic: ic.id, fp: null };
      }
    }
    return best;
  };

  /** The initial condition whose trajectory passes within a few px, if any. */
  const hitTrajectory = (px: number, py: number): number | null => {
    if (!scene) return null;
    const tol = 5 * scale;
    let best: number | null = null;
    let bestD = tol * tol;
    for (const tr of scene.trajectories) {
      const pts = trajectoryPx(vp, scene, tr.out);
      for (let k = 2; k < pts.length; k += 2) {
        const d = segDist2(px, py, pts[k - 2], pts[k - 1], pts[k], pts[k + 1]);
        if (d < bestD) {
          bestD = d;
          best = tr.meta.icId;
        }
      }
    }
    return best;
  };

  // ---- wheel zoom (needs a non-passive listener to prevent page scroll)
  const latestRef = useRef({ view, vp, actions });
  latestRef.current = { view, vp, actions };
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const { view: v, vp: port, actions: act } = latestRef.current;
      const r = canvas.getBoundingClientRect();
      const [ax, ay] = toPlot(port, e.clientX - r.left, e.clientY - r.top);
      const delta = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
      const f = Math.exp(Math.max(-0.5, Math.min(0.5, delta * 0.0015)));
      act.setView({ ...v, xRange: zoomRange(v.xRange, ax, f), yRange: zoomRange(v.yRange, ay, f) });
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });
    return () => canvas.removeEventListener("wheel", onWheel);
  }, []);

  // ---- keyboard
  const centre = (): [number, number] => [
    (view.xRange[0] + view.xRange[1]) / 2,
    (view.yRange[0] + view.yRange[1]) / 2,
  ];
  const inView = ([x, y]: [number, number]) =>
    x >= Math.min(...view.xRange) &&
    x <= Math.max(...view.xRange) &&
    y >= Math.min(...view.yRange) &&
    y <= Math.max(...view.yRange);
  const fmtPoint = ([x, y]: [number, number]) => `${fmtPlain(x)}, ${fmtPlain(y)}`;

  const onKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    if (!scene || e.altKey || e.ctrlKey || e.metaKey) return;
    const cur = kbd && inView(kbd) ? kbd : centre();
    const big = e.shiftKey ? 5 : 1;
    const dx = (view.xRange[1] - view.xRange[0]) * KEY_STEP * big;
    const dy = (view.yRange[1] - view.yRange[0]) * KEY_STEP * big;
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [-dx, 0],
      ArrowRight: [dx, 0],
      ArrowUp: [0, dy],
      ArrowDown: [0, -dy],
    };
    const handled = () => e.preventDefault();
    if (e.key in moves) {
      handled();
      const [mx, my] = moves[e.key];
      const next: [number, number] = [
        clampTo(cur[0] + mx, view.xRange),
        clampTo(cur[1] + my, view.yRange),
      ];
      setKbd(next);
      setAnnounce(fmtPoint(next));
    } else if (e.key === "Enter" || e.key === " ") {
      handled();
      const hit = hitTest(sx(vp, cur[0]), sy(vp, cur[1]));
      setKbd(cur);
      if (hit.ic !== null) {
        actions.select({ kind: "ic", id: hit.ic });
        setAnnounce("Selected trajectory");
      } else if (hit.fp !== null) {
        actions.select({ kind: "fp", index: hit.fp });
        setAnnounce("Selected fixed point");
      } else {
        actions.addIc(cur[0], cur[1]);
        setAnnounce(`Added a trajectory at ${fmtPoint(cur)}`);
      }
    } else if (e.key === "f" || e.key === "F") {
      handled();
      const n = scene.fixedPoints.length;
      if (n === 0) {
        setAnnounce("No fixed points in view");
        return;
      }
      const now = state.selection?.kind === "fp" ? state.selection.index : e.shiftKey ? 0 : -1;
      const i = (((now + (e.shiftKey ? -1 : 1)) % n) + n) % n;
      const [px, py] = fixedPointPx(vp, scene, scene.fixedPoints[i]);
      setKbd(toPlot(vp, px, py));
      actions.select({ kind: "fp", index: i });
      setAnnounce(`Fixed point ${i + 1} of ${n}`);
    } else if (e.key === "+" || e.key === "=" || e.key === "-" || e.key === "_") {
      handled();
      const f = e.key === "+" || e.key === "=" ? 1 / 1.25 : 1.25;
      actions.setView({ ...view, xRange: zoomRange(view.xRange, cur[0], f), yRange: zoomRange(view.yRange, cur[1], f) });
      setKbd(cur);
    } else if (e.key === "0") {
      handled();
      actions.resetView();
      setKbd(null);
    } else if ((e.key === "Delete" || e.key === "Backspace") && state.selection?.kind === "ic") {
      handled();
      actions.removeIcs([state.selection.id]);
      setAnnounce("Removed the trajectory");
    } else if (e.key === "Escape" && state.selection) {
      handled();
      actions.select(null);
    }
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0 || !scene) return;
    setKbd(null);
    const [px, py] = localPoint(e);
    const hit = hitTest(px, py);
    e.currentTarget.setPointerCapture(e.pointerId);
    if (hit.ic !== null) {
      actions.select({ kind: "ic", id: hit.ic });
      drag.current = { mode: "ic", id: hit.ic };
      setDragging("ic");
    } else if (hit.fp !== null) {
      actions.select({ kind: "fp", index: hit.fp });
      drag.current = null;
    } else {
      drag.current = {
        mode: "pan",
        startX: px,
        startY: py,
        xRange: view.xRange,
        yRange: view.yRange,
        moved: false,
      };
    }
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const [px, py] = localPoint(e);
    const d = drag.current;
    if (d?.mode === "ic") {
      const [x, y] = toPlot(vp, px, py);
      actions.moveIc(d.id, x, y);
      return;
    }
    if (d?.mode === "pan") {
      const dx = px - d.startX;
      const dy = py - d.startY;
      if (!d.moved && Math.hypot(dx, dy) > 3) {
        d.moved = true;
        setDragging("pan");
      }
      if (d.moved) {
        const [ux, uy] = unitsPerPx({ ...vp, xRange: d.xRange, yRange: d.yRange });
        actions.setView({
          ...view,
          xRange: [d.xRange[0] - dx * ux, d.xRange[1] - dx * ux],
          yRange: [d.yRange[0] + dy * uy, d.yRange[1] + dy * uy],
        });
      }
      return;
    }
    const hit = hitTest(px, py);
    if (hit.fp !== hover.fp || hit.ic !== hover.ic) setHover(hit);
  };

  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    drag.current = null;
    setDragging(null);
    if (d?.mode !== "pan" || d.moved) return;
    const [px, py] = localPoint(e);
    if (!insidePlot(vp, px, py)) return;
    const traj = hitTrajectory(px, py);
    if (traj !== null) {
      actions.select({ kind: "ic", id: traj });
      return;
    }
    const [x, y] = toPlot(vp, px, py);
    const id = actions.addIc(x, y);
    recentAdds.current = [
      ...recentAdds.current.filter((a) => performance.now() - a.at < 600),
      { id, at: performance.now() },
    ];
  };

  const onDoubleClick = () => {
    // The two clicks of a double-click each added a point; take them back.
    const now = performance.now();
    const ids = recentAdds.current.filter((a) => now - a.at < 600).map((a) => a.id);
    recentAdds.current = [];
    if (ids.length) actions.removeIcs(ids);
    actions.resetView();
  };

  const cursor =
    dragging === "pan"
      ? "grabbing"
      : dragging === "ic" || hover.ic !== null
        ? "grab"
        : hover.fp !== null
          ? "pointer"
          : "crosshair";

  // ---- overlays
  const states = compiled?.states ?? [];
  const kind = compiled?.kind;
  const dim = states.length;
  const hoveredFp = scene && hover.fp !== null ? scene.fixedPoints[hover.fp] : undefined;
  const hiddenStates =
    kind === "ode"
      ? states
          .map((s, i) => ({ s, i }))
          .filter(
            ({ i }) =>
              !sameAxis(view.xAxis, { kind: "state", index: i }) &&
              !sameAxis(view.yAxis, { kind: "state", index: i }),
          )
      : [];
  const showTimeSlice =
    kind === "ode" &&
    compiled &&
    !compiled.autonomous &&
    view.xAxis.kind !== "time" &&
    view.yAxis.kind !== "time";

  return (
    <>
      <div className="plot-wrap" ref={wrapRef}>
        <canvas
          ref={canvasRef}
          className="plot-canvas"
          style={{ cursor }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerLeave={() => !drag.current && setHover({ fp: null, ic: null })}
          onDoubleClick={onDoubleClick}
          onKeyDown={onKeyDown}
          onFocus={(e) => {
            // Show the keyboard cursor only for keyboard focus, not for clicks.
            if (e.currentTarget.matches(":focus-visible")) setKbd((k) => (k && inView(k) ? k : centre()));
          }}
          onBlur={() => setKbd(null)}
          tabIndex={0}
          role="application"
          aria-roledescription="phase portrait"
          aria-label="Phase portrait"
          aria-describedby="plot-keys"
        />
        <span id="plot-keys" className="sr-only">
          Click, or press Enter, to add a trajectory. Arrow keys move the cursor, Shift for bigger steps. F and
          Shift+F step through fixed points. Plus and minus zoom, 0 resets the view. Delete removes the selected
          trajectory.
        </span>
        <span className="sr-only" aria-live="polite">
          {announce}
        </span>
        {kbd && inView(kbd) && (
          <div className="kbd-cursor" style={{ left: sx(vp, kbd[0]), top: sy(vp, kbd[1]) }} aria-hidden="true" />
        )}

        {compiled && size.w > 0 && (
          <>
            <div
              className="axis-label axis-label-x"
              style={{ right: size.w - vp.left - vp.width + 8, top: vp.top + vp.height - 8 }}
            >
              <Tex latex={axisLabel(view.xAxis, view.yAxis, states, kind)} />
            </div>
            <div className="axis-label axis-label-y" style={{ left: vp.left + 8, top: vp.top + 6 }}>
              <Tex latex={axisLabel(view.yAxis, view.xAxis, states, kind)} />
            </div>
          </>
        )}

        {(hiddenStates.length > 0 || showTimeSlice) && !sliceOpen && (
          <button
            type="button"
            className="slice-card slice-folded"
            style={{ top: vp.top + 10, right: size.w - vp.left - vp.width + 10 }}
            onClick={() => setSliceOpen(true)}
            aria-expanded={false}
            title="Show the slice controls"
          >
            <span className="slice-label">Field at</span>
            <Tex
              latex={[
                ...hiddenStates.map(({ s, i }) => `${s.latex}=${fmtTex(state.baseState[i] ?? 0, 3)}`),
                ...(showTimeSlice ? [`t=${fmtTex(state.fieldTime, 3)}`] : []),
              ].join(",\ ")}
            />
            <IconChevron size={14} />
          </button>
        )}
        {(hiddenStates.length > 0 || showTimeSlice) && sliceOpen && (
          <div className="slice-card" style={{ top: vp.top + 10, right: size.w - vp.left - vp.width + 10 }}>
            <div className="slice-head">
              <span>Field slice</span>
              <button
                type="button"
                className="icon-btn slice-fold"
                onClick={() => setSliceOpen(false)}
                aria-expanded={true}
                aria-label="Fold the slice controls"
                title="Fold to one line"
              >
                <IconChevron size={14} />
              </button>
            </div>
            {hiddenStates.map(({ s, i }) => {
              const ext = stateExtent(state.portrait?.res, dim, i);
              const lo = Math.min(ext ? Math.floor(ext[0]) : -10, state.baseState[i] ?? 0);
              const hi = Math.max(ext ? Math.ceil(ext[1]) : 10, state.baseState[i] ?? 0);
              return (
                <SliceRow
                  key={s.name}
                  latex={s.latex}
                  value={state.baseState[i] ?? 0}
                  min={lo}
                  max={hi}
                  onChange={(v) => actions.setSlice(i, v)}
                />
              );
            })}
            {showTimeSlice && (
              <SliceRow
                latex="t"
                value={state.fieldTime}
                min={0}
                max={Math.max(10, state.solver.tEnd)}
                onChange={actions.setFieldTime}
              />
            )}
          </div>
        )}

        {hoveredFp && scene && (
          <FixedPointTooltip
            fp={hoveredFp}
            states={states}
            kind={kind}
            pos={fixedPointPx(vp, scene, hoveredFp)}
            width={size.w}
          />
        )}

        {compiled && !state.compileOk && (
          <div className="plot-hint warn">
            Showing the last system that worked. Fix the equation on the left to update.
          </div>
        )}
        {compiled && state.compileOk && state.ics.length === 0 && (
          <div className="plot-hint">Click anywhere on the plot to start a trajectory</div>
        )}
        {state.computeError && <div className="plot-hint error">Could not compute: {state.computeError}</div>}
      </div>
      <LegendBar scene={scene} states={states} kind={kind} />
    </>
  );
}

function SliceRow(props: {
  latex: string;
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="slice-row">
      <span className="slice-label">
        Field at <Tex latex={`${props.latex} =`} />
      </span>
      <NumberField
        value={props.value}
        onCommit={props.onChange}
        digits={3}
        ariaLabel={`Slice value`}
        className="slice-num"
      />
      <Slider
        value={props.value}
        min={props.min}
        max={props.max}
        onChange={props.onChange}
        ariaLabel="Slice value"
      />
    </div>
  );
}

/** Small SVG copy of a canvas fixed-point marker, for the legend and inspector. */
export function FixedPointGlyph({
  shape,
  filled,
  size = 14,
}: {
  shape: FixedPointShape;
  filled: boolean;
  size?: number;
}) {
  const c = size / 2;
  const r = size * 0.32;
  const fill = filled ? "var(--plot-fp)" : "var(--plot-bg)";
  const common = { fill, stroke: "var(--plot-fp)", strokeWidth: 1.4 };
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden="true" className="fp-glyph">
      {shape === "diamond" && (
        <path
          d={`M${c} ${c - r * 1.2}L${c + r * 1.2} ${c}L${c} ${c + r * 1.2}L${c - r * 1.2} ${c}Z`}
          {...common}
        />
      )}
      {shape === "triangle" && (
        <path
          d={`M${c} ${c - r * 1.15}L${c + r * 1.1} ${c + r * 0.8}L${c - r * 1.1} ${c + r * 0.8}Z`}
          {...common}
        />
      )}
      {(shape === "circle" || shape === "centre") && <circle cx={c} cy={c} r={r} {...common} />}
      {shape === "centre" && <circle cx={c} cy={c} r={r * 0.4} fill="var(--plot-fp)" />}
      {shape === "spiral" && (
        <>
          <circle cx={c} cy={c} r={r * 0.62} {...common} />
          <path
            d={`M${c + r * 1.25 * Math.cos(-0.15 * Math.PI)} ${c + r * 1.25 * Math.sin(-0.15 * Math.PI)}A${r * 1.25} ${r * 1.25} 0 1 1 ${c + r * 1.25 * Math.cos(1.25 * Math.PI)} ${c + r * 1.25 * Math.sin(1.25 * Math.PI)}`}
            fill="none"
            stroke="var(--plot-fp)"
            strokeWidth={1.2}
          />
        </>
      )}
    </svg>
  );
}

/** The strip under the plot: what each mark means, and how to interact. */
function LegendBar({
  scene,
  states,
  kind,
}: {
  scene: Scene | null;
  states: SymbolInfo[];
  kind?: SystemKind;
}) {
  const nulls = scene ? scene.nullclines.filter((n) => n.polylines.length > 0) : [];
  const kinds = new Map<string, { shape: FixedPointShape; filled: boolean; label: string }>();
  for (const fp of scene?.fixedPoints ?? []) {
    const label = classificationTitle(fp.classification, states.length);
    kinds.set(label, { shape: fixedPointShape(fp.classification), filled: fp.stable, label });
  }
  const hasBackward = scene?.trajectories.some((t) => t.meta.direction === "backward") ?? false;
  return (
    <div className="legend-bar">
      {nulls.map((n) => (
        <span className="legend-item" key={`n${n.state}`}>
          <span className="legend-dash" style={{ borderColor: `var(--null-${(n.state % 4) + 1})` }} />
          <Tex
            latex={`${kind === "map" ? states[n.state]?.latex : dotted(states[n.state]?.latex ?? "x")}=0`}
          />
        </span>
      ))}
      {[...kinds.values()].map((k) => (
        <span className="legend-item" key={k.label}>
          <FixedPointGlyph shape={k.shape} filled={k.filled} />
          {k.label}
        </span>
      ))}
      {hasBackward && (
        <span className="legend-item">
          <span className="legend-faint" />
          backward in time
        </span>
      )}
      <span className="legend-spacer" />
      {scene && (
        <span className="legend-help">
          Click to add · drag a dot to move · drag to pan · scroll to zoom · double-click to reset
        </span>
      )}
    </div>
  );
}

function FixedPointTooltip(props: {
  fp: FixedPointOut;
  states: SymbolInfo[];
  kind?: SystemKind;
  pos: [number, number];
  width: number;
}) {
  const { fp, states, kind, pos } = props;
  const names = states.map((s) => s.latex).join(", ");
  const values = fp.x.map((v) => fmtTex(v, 3)).join(", ");
  const point = states.length === 1 ? `${names}^* = ${values}` : `(${names}) = (${values})`;
  const sym = kind === "map" ? "\\mu" : "\\lambda";
  const flip = pos[0] > props.width - 260;
  return (
    <div
      className="tooltip"
      style={{
        left: pos[0] + (flip ? -14 : 14),
        top: pos[1] + 14,
        transform: flip ? "translateX(-100%)" : undefined,
      }}
    >
      <div className="tooltip-title">
        {classificationTitle(fp.classification, states.length)}
        <span className={"badge " + (fp.stable ? "ok" : "neutral")}>{fp.stable ? "stable" : "unstable"}</span>
      </div>
      <div className="tooltip-row">
        <Tex latex={point} />
      </div>
      {fp.eigenvalues.map((ev, i) => (
        <div className="tooltip-row" key={i}>
          <Tex latex={`${sym}_{${i + 1}} = ${complexTex(ev)}`} />
        </div>
      ))}
      <div className="tooltip-foot">Click for details</div>
    </div>
  );
}

function segDist2(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  const t = len2 > 0 ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len2)) : 0;
  const x = ax + t * dx - px;
  const y = ay + t * dy - py;
  return x * x + y * y;
}

/** Clamp `v` into a range given in either order. */
function clampTo(v: number, [a, b]: [number, number]): number {
  return Math.min(Math.max(a, b), Math.max(Math.min(a, b), v));
}

/** A coordinate for screen-reader announcements (3 significant digits). */
function fmtPlain(v: number): string {
  return Number(v.toPrecision(3)).toString();
}
