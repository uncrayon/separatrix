import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { AxisRef, FixedPointOut, StateInfo, SystemKind } from "../api/types";
import { Tex } from "../math/Tex";
import { axisLabelLatex, axisLatex, classificationTitle, COMPUTED_ZERO, fmtTex, latexToPlain, sameAxis } from "../math/format";
import { useCanvasTool } from "../components/CanvasToolbar";
import { IconChevronDown, IconError, IconWarning } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { selectFixedPoint, selectedFixedPointIndex, stateExtent } from "../state/model";
import { NumberField, Slider } from "../components/controls";
import { numberedEigenvalues, useShowEigenvectors, type NumberedEigenvalue } from "./eigen";
import {
  axisLines,
  drawScene,
  eigenDirections,
  fixedPointGlyph,
  fixedPointPx,
  icPx,
  trajectoryPx,
  type Box,
  type FixedPointGlyphKind,
  type Scene,
} from "./draw";
import { readPlotTheme, type PlotTheme } from "./theme";
import { plotViewport, sx, sy, toPlot, unitsPerPx, zoomRange } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  theme: "dark" | "light";
  toolbar?: ReactNode;
}

type Drag =
  | { mode: "ic"; id: number }
  | {
      mode: "pan";
      startX: number;
      startY: number;
      xRange: [number, number];
      yRange: [number, number];
      upp: [number, number];
      moved: boolean;
      clickAdds: boolean;
    };

const HIT_RADIUS = 10;
const SLICE_CARD_MIN_PLOT_W = 640;
const KEY_STEP = 1 / 40;

export function dotted(latex: string): string {
  const m = /^\\dot\{(.*)\}$/.exec(latex);
  return m ? `\\ddot{${m[1]}}` : `\\dot{${latex}}`;
}

const sameBox = (a: Box | null, b: Box | null) =>
  a === b || (!!a && !!b && a.every((v, i) => Math.abs(v - b[i]) < 0.5));

export function PortraitCanvas({ state, actions, theme, toolbar }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const probeRef = useRef<HTMLSpanElement>(null);
  const sliceRef = useRef<HTMLElement | null>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [hover, setHover] = useState<{ fp: number | null; ic: number | null }>({ fp: null, ic: null });
  const [dragging, setDragging] = useState<Drag["mode"] | null>(null);
  const drag = useRef<Drag | null>(null);
  const recentAdds = useRef<{ id: number; at: number }[]>([]);
  const themeCache = useRef<{ key: string; th: PlotTheme } | null>(null);
  const [themeVersion, setThemeVersion] = useState(0);
  const [toolbarBox, setToolbarBox] = useState<Box | null>(null);
  const [sliceBelow, setSliceBelow] = useState(false);
  const [xNameSize, setXNameSize] = useState<[number, number]>([12, 20]);
  const nameBoxes = useRef<Box[]>([]);
  const [kbd, setKbd] = useState<[number, number] | null>(null);
  const [announce, setAnnounce] = useState("");
  const [sliceChoice, setSliceOpen] = useState<boolean | null>(null);
  const sliceOpen = sliceChoice ?? size.w >= SLICE_CARD_MIN_PLOT_W;
  const tool = useCanvasTool();
  const showEigen = useShowEigenvectors();

  const { compiled, view } = state;
  const plotTheme = (): PlotTheme => {
    const probe = probeRef.current;
    const key = `${document.documentElement.dataset.theme ?? theme}:${themeVersion}:${probe ? "probe" : "root"}`;
    if (themeCache.current?.key !== key) themeCache.current = { key, th: readPlotTheme(probe) };
    return themeCache.current.th;
  };
  const glyphScale = plotTheme().glyphScale;
  const vp = useMemo(() => plotViewport(size.w, size.h, view, compiled?.kind), [size.w, size.h, view, compiled?.kind]);

  useEffect(() => {
    actions.setPlotAspect(size.w > 0 && size.h > 0 ? size.w / size.h : null);
  }, [actions, size.w, size.h]);

  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const bump = () => setThemeVersion((v) => v + 1);
    const mo = new MutationObserver(bump);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    let live = true;
    document.fonts?.ready.then(() => live && bump());
    return () => {
      live = false;
      mo.disconnect();
    };
  }, []);

  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const wr = wrap.getBoundingClientRect();
    const ox = wr.left + wrap.clientLeft;
    const oy = wr.top + wrap.clientTop;
    const rel = (r: DOMRect): Box => [r.left - ox, r.top - oy, r.right - ox, r.bottom - oy];
    const bar = wrap.querySelector<HTMLElement>(".plot-toolbar");
    const tb = bar ? rel(bar.getBoundingClientRect()) : null;
    if (!sameBox(tb, toolbarBox)) setToolbarBox(tb);
    nameBoxes.current = [...wrap.querySelectorAll<HTMLElement>(".axis-label")].map((el) =>
      rel(el.getBoundingClientRect()),
    );
    const xName = wrap.querySelector<HTMLElement>(".axis-label-x");
    if (xName) {
      const w = xName.offsetWidth;
      const h = xName.offsetHeight;
      if (Math.abs(w - xNameSize[0]) > 0.5 || Math.abs(h - xNameSize[1]) > 0.5) setXNameSize([w, h]);
    }
    const card = sliceRef.current;
    if (card && tb) {
      const cr = rel(card.getBoundingClientRect());
      const below = cr[0] < tb[2] + 8;
      if (below !== sliceBelow) setSliceBelow(below);
    }
  });

  const scene = useMemo<Omit<Scene, "avoid"> | null>(() => {
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
          const hidden = state.ics.some((ic) => ic.id === meta?.icId && ic.hidden);
          return meta && color !== undefined && !hidden ? [{ out, meta, color }] : [];
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
      selectedFp:
        valid && state.layers.fixedPoints
          ? selectedFixedPointIndex(state.selection, p.res.fixedPoints, p.meta.view)
          : null,
      hoverIc: hover.ic,
      hoverFp: hover.fp,
      eigenvectors: showEigen,
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
    showEigen,
  ]);

  useEffect(() => {
    if (!scene || !toolbarBox || size.w === 0 || view !== state.homeView) return;
    const [l, , r, b] = toolbarBox;
    const pad = 12 * glyphScale;
    const marks: [number, number][] = [
      ...scene.ics.filter((ic) => ic.x0.length === scene.dim).map((ic) => icPx(vp, scene, ic)),
      ...scene.fixedPoints.map((fp) => fixedPointPx(vp, scene, fp)),
    ];
    const target = b + pad;
    const H = vp.height;
    if (target >= H * 0.6) return;
    const [y0, y1] = vp.yRange;
    let top = y1;
    for (const [px, py] of marks) {
      if (px < l - pad || px > r + pad || py >= target || py < vp.top) continue;
      const y = y1 - ((py - vp.top) / H) * (y1 - y0);
      const f = (target - vp.top) / H;
      top = Math.max(top, (y - f * y0) / (1 - f));
    }
    if (top > y1 + 1e-9 * Math.abs(y1 - y0)) actions.setView({ ...view, xRange: vp.xRange, yRange: [y0, top] });
  }, [scene, toolbarBox, size.w, vp, glyphScale, view, state.homeView, actions]);

  // Every input of the last paint (the canvas holds its own size). A render
  // that changes none of them (a keystroke in the equation editor) needs no repaint.
  const painted = useRef<{ scene: typeof scene; vp: typeof vp; th: PlotTheme; boxes: string } | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || size.w === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(size.w * dpr);
    const H = Math.round(size.h * dpr);
    const th = plotTheme();
    const labels = [...(toolbarBox ? [toolbarBox] : []), ...nameBoxes.current];
    const boxes = labels.join(";");
    const last = painted.current;
    const sized = canvas.width === W && canvas.height === H;
    if (sized && last && last.scene === scene && last.vp === vp && last.th === th && last.boxes === boxes) return;
    if (!sized) {
      canvas.width = W;
      canvas.height = H;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    painted.current = { scene, vp, th, boxes };
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const avoid = {
      labels,
      field: nameBoxes.current.map(([l, t, r, b]): Box => [l - 3, t - 3, r + 3, b + 3]),
    };
    drawScene(ctx, size.w, size.h, vp, scene && { ...scene, avoid }, th);
  });

  const localPoint = (e: { clientX: number; clientY: number }): [number, number] => {
    const r = canvasRef.current!.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  /** The mark under the pointer. Start dots win ties over fixed points, and newer dots over older ones. */
  const hitTest = (px: number, py: number): { ic: number | null; fp: number | null } => {
    const none = { ic: null, fp: null };
    if (!scene) return none;
    type Hit = { d: number; rank: number; hit: { ic: number | null; fp: number | null } };
    const dist = ([x, y]: [number, number]) => (x - px) ** 2 + (y - py) ** 2;
    const candidates: Hit[] = [
      ...scene.fixedPoints.map((fp, i) => ({ d: dist(fixedPointPx(vp, scene, fp)), rank: 0, hit: { ic: null, fp: i } })),
      ...scene.ics.flatMap((ic, i) =>
        ic.x0.length === scene.dim ? [{ d: dist(icPx(vp, scene, ic)), rank: 1 + i, hit: { ic: ic.id, fp: null } }] : [],
      ),
    ];
    let best: Hit | null = null;
    for (const c of candidates) {
      if (c.d > (HIT_RADIUS * glyphScale) ** 2) continue;
      if (!best || c.d < best.d || (c.d === best.d && c.rank > best.rank)) best = c;
    }
    return best ? best.hit : none;
  };

  const hitTrajectory = (px: number, py: number): number | null => {
    if (!scene) return null;
    const tol = 5 * glyphScale;
    // A map's orbit off a time axis is drawn as dots only, so only the dots can be hit.
    const dotsOnly = scene.kind === "map" && scene.xAxis.kind !== "time" && scene.yAxis.kind !== "time";
    let best: number | null = null;
    let bestD = tol * tol;
    for (const tr of scene.trajectories) {
      const pts = trajectoryPx(vp, scene, tr.out);
      for (let k = dotsOnly ? 0 : 2; k < pts.length; k += 2) {
        const d = dotsOnly
          ? (px - pts[k]) ** 2 + (py - pts[k + 1]) ** 2
          : segDist2(px, py, pts[k - 2], pts[k - 1], pts[k], pts[k + 1]);
        if (d < bestD) {
          bestD = d;
          best = tr.meta.icId;
        }
      }
    }
    return best;
  };

  // Wheel zoom needs a non-passive listener to prevent page scroll.
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

  const centre = (): [number, number] => [
    (vp.xRange[0] + vp.xRange[1]) / 2,
    (vp.yRange[0] + vp.yRange[1]) / 2,
  ];
  const inView = ([x, y]: [number, number]) =>
    x >= Math.min(...vp.xRange) &&
    x <= Math.max(...vp.xRange) &&
    y >= Math.min(...vp.yRange) &&
    y <= Math.max(...vp.yRange);
  const fmtPoint = ([x, y]: [number, number]) => `${fmtPlain(x)}, ${fmtPlain(y)}`;

  const onKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    if (!scene || e.altKey || e.ctrlKey || e.metaKey) return;
    const cur = kbd && inView(kbd) ? kbd : centre();
    const big = e.shiftKey ? 5 : 1;
    const dx = (vp.xRange[1] - vp.xRange[0]) * KEY_STEP * big;
    const dy = (vp.yRange[1] - vp.yRange[0]) * KEY_STEP * big;
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
      const next: [number, number] = [clampTo(cur[0] + mx, vp.xRange), clampTo(cur[1] + my, vp.yRange)];
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
        actions.select(selectFixedPoint(scene.fixedPoints[hit.fp].x));
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
      const now = scene.selectedFp ?? (e.shiftKey ? 0 : -1);
      const i = (((now + (e.shiftKey ? -1 : 1)) % n) + n) % n;
      const [px, py] = fixedPointPx(vp, scene, scene.fixedPoints[i]);
      setKbd(toPlot(vp, px, py));
      actions.select(selectFixedPoint(scene.fixedPoints[i].x));
      setAnnounce(`Fixed point ${i + 1} of ${n}`);
    } else if (e.key === "+" || e.key === "=" || e.key === "-" || e.key === "_") {
      handled();
      const f = e.key === "+" || e.key === "=" ? 1 / 1.25 : 1.25;
      actions.setView({
        ...view,
        xRange: zoomRange(view.xRange, cur[0], f),
        yRange: zoomRange(view.yRange, cur[1], f),
      });
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

  const addAt = (px: number, py: number) => {
    const [x, y] = toPlot(vp, px, py);
    const id = actions.addIc(x, y);
    recentAdds.current = [
      ...recentAdds.current.filter((a) => performance.now() - a.at < 600),
      { id, at: performance.now() },
    ];
    return id;
  };

  const startPan = (px: number, py: number, clickAdds: boolean) => {
    drag.current = {
      mode: "pan",
      startX: px,
      startY: py,
      xRange: view.xRange,
      yRange: view.yRange,
      upp: unitsPerPx(vp),
      moved: false,
      clickAdds,
    };
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0 || !scene) return;
    setKbd(null);
    const [px, py] = localPoint(e);
    e.currentTarget.setPointerCapture(e.pointerId);
    if (tool === "pan") {
      startPan(px, py, false);
      return;
    }
    if (tool === "add") {
      const id = addAt(px, py);
      drag.current = { mode: "ic", id };
      setDragging("ic");
      return;
    }
    const hit = hitTest(px, py);
    if (hit.ic !== null) {
      actions.select({ kind: "ic", id: hit.ic });
      drag.current = { mode: "ic", id: hit.ic };
      setDragging("ic");
    } else if (hit.fp !== null) {
      actions.select(selectFixedPoint(scene.fixedPoints[hit.fp].x));
      drag.current = null;
    } else {
      startPan(px, py, true);
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
        const [ux, uy] = d.upp;
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
    if (d?.mode !== "pan" || d.moved || !d.clickAdds) return;
    const [px, py] = localPoint(e);
    const traj = hitTrajectory(px, py);
    if (traj !== null) {
      actions.select({ kind: "ic", id: traj });
      return;
    }
    addAt(px, py);
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
      : tool === "pan"
        ? "grab"
        : tool === "add"
          ? "crosshair"
          : dragging === "ic" || hover.ic !== null
            ? "grab"
            : hover.fp !== null
              ? "pointer"
              : "crosshair";

  const states = compiled?.states ?? [];
  const kind = compiled?.kind;
  const dim = states.length;
  const hoveredFp = scene && hover.fp !== null && !dragging ? scene.fixedPoints[hover.fp] : undefined;
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
  const hasSlice = hiddenStates.length > 0 || !!showTimeSlice;

  const axes = axisLines(vp);
  const right = vp.left + vp.width;
  const xNameAbove = axes.Y - vp.top > 40;
  const [xNameW, xNameH] = xNameSize;
  let xNameTop = xNameAbove ? axes.Y - 4 - xNameH : axes.Y + 4;
  if (
    toolbarBox &&
    right - 8 - xNameW < toolbarBox[2] &&
    right - 8 > toolbarBox[0] &&
    xNameTop < toolbarBox[3] &&
    xNameTop + xNameH > toolbarBox[1]
  )
    xNameTop = toolbarBox[3] + 6;
  const yNameRight = right - axes.X > 48;
  const yNameX = yNameRight ? axes.X + 8 : axes.X - 8;
  const yNameTop =
    toolbarBox && (yNameRight ? yNameX < toolbarBox[2] : yNameX > toolbarBox[0])
      ? toolbarBox[3] + 6
      : vp.top + 6;
  const sliceStyle = {
    top: sliceBelow && toolbarBox ? toolbarBox[3] + 8 : undefined,
  };

  const ariaLabel = scene
    ? describePlot(scene, states, kind, vp.xRange, vp.yRange, view.xAxis, view.yAxis)
    : "Phase portrait";

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
            if (e.currentTarget.matches(":focus-visible")) setKbd((k) => (k && inView(k) ? k : centre()));
          }}
          onBlur={() => setKbd(null)}
          tabIndex={0}
          role="application"
          aria-roledescription="phase plane"
          aria-label={ariaLabel}
          aria-describedby="plot-keys"
        />
        {toolbar}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="plot-keys" className="ds-sr">
          Click, or press Enter, to add a trajectory. Arrow keys move the cursor, Shift for bigger steps. F
          and Shift+F step through fixed points. Plus and minus zoom, 0 resets the view. Delete removes the
          selected trajectory.
        </span>
        <span className="ds-sr" aria-live="polite" aria-atomic="true">
          {announce}
        </span>
        {kbd && inView(kbd) && (
          <div
            className="kbd-cursor"
            style={{ left: sx(vp, kbd[0]), top: sy(vp, kbd[1]) }}
            aria-hidden="true"
          />
        )}

        {compiled && size.w > 0 && (
          <>
            <div
              className="axis-label axis-label-x math"
              style={{ right: size.w - right + 8, top: xNameTop }}
              aria-hidden="true"
            >
              <Tex latex={axisLabelLatex(view.xAxis, view.yAxis, states, kind)} />
            </div>
            <div
              className={"axis-label axis-label-y math" + (yNameRight ? "" : " is-left")}
              style={yNameRight ? { left: yNameX, top: yNameTop } : { right: size.w - yNameX, top: yNameTop }}
              aria-hidden="true"
            >
              <Tex latex={axisLabelLatex(view.yAxis, view.xAxis, states, kind)} />
            </div>
          </>
        )}

        {hasSlice && !sliceOpen && (
          <button
            ref={(el) => {
              sliceRef.current = el;
            }}
            type="button"
            className="slice-card slice-folded"
            style={sliceStyle}
            onClick={() => setSliceOpen(true)}
            aria-expanded={false}
            data-tooltip="Show the slice controls"
          >
            <span className="slice-label caption">Field at</span>
            <span className="math">
              <Tex
                latex={[
                  ...hiddenStates.map(({ s, i }) => `${s.latex}=${fmtTex(state.baseState[i] ?? 0, 3)}`),
                  ...(showTimeSlice ? [`t=${fmtTex(state.fieldTime, 3)}`] : []),
                ].join(",\\ ")}
              />
            </span>
            <IconChevronDown className="slice-folded-chevron" />
            <span className="ds-sr">. Show the slice controls</span>
          </button>
        )}
        {hasSlice && sliceOpen && (
          <div
            ref={(el) => {
              sliceRef.current = el;
            }}
            className="slice-card"
            style={sliceStyle}
          >
            <div className="slice-head">
              <span className="label">Field slice</span>
              <button
                type="button"
                className="ds-icon-button ds-icon-button--sm slice-fold"
                onClick={() => setSliceOpen(false)}
                aria-expanded={true}
                aria-label="Fold the slice controls"
                data-tip="Fold to one line"
                data-tip-align="end"
              >
                <IconChevronDown className="slice-fold-icon" />
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
            height={size.h}
            glyphScale={glyphScale}
          />
        )}

        {compiled && !state.compileOk && (
          <div className="plot-hint">
            <div className="ds-diagnostic ds-diagnostic--block ds-diagnostic--warning" role="status">
              <IconWarning className="ds-diagnostic-icon" />
              <span className="ds-diagnostic-body">
                <span className="ds-diagnostic-word">Warning</span>
                <span className="ds-diagnostic-msg">
                  Showing the last system that worked. Fix the equation on the left to update.
                </span>
              </span>
            </div>
          </div>
        )}
        {compiled && state.compileOk && state.ics.length === 0 && (
          <div className="plot-hint">
            <p className="plot-hint-note caption">
              {tool === "pan"
                ? "Choose Select or Add trajectory, then click the plot to start a trajectory."
                : "Click anywhere on the plot to start a trajectory."}
            </p>
          </div>
        )}
        {state.computeError && (
          <div className="plot-hint">
            <div className="ds-diagnostic ds-diagnostic--block ds-diagnostic--error" role="alert">
              <IconError className="ds-diagnostic-icon" />
              <span className="ds-diagnostic-body">
                <span className="ds-diagnostic-word">Error</span>
                <span className="ds-diagnostic-msg">Could not compute: {state.computeError}</span>
              </span>
            </div>
          </div>
        )}
      </div>
      <Legend scene={scene} states={states} kind={kind} tool={tool} />
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
  const name = latexToPlain(props.latex);
  return (
    <div className="slice-row">
      <NumberField
        size="sm"
        lead={<Tex latex={props.latex} />}
        value={props.value}
        onCommit={props.onChange}
        digits={3}
        ariaLabel={`Field slice at ${name}`}
        className="slice-num"
      />
      <Slider
        value={props.value}
        min={props.min}
        max={props.max}
        onChange={props.onChange}
        ariaLabel={`Field slice at ${name}`}
        className="slice-slider"
      />
    </div>
  );
}

export function FixedPointIcon({ kind }: { kind: FixedPointGlyphKind }) {
  return (
    <svg className="ds-fp-icon" viewBox="-8 -8 16 16" aria-hidden="true">
      <g className={`ds-fp ds-fp--${kind}`}>
        <g className="ds-fp-mark">
          <circle className="ds-fp-body" r="5" />
          {kind === "half" && <path className="ds-fp-half" d="M0 -4.25A4.25 4.25 0 0 0 0 4.25Z" />}
        </g>
      </g>
    </svg>
  );
}

const GLYPH_ORDER: FixedPointGlyphKind[] = ["stable", "unstable", "half"];

function Legend({
  scene,
  states,
  kind,
  tool,
}: {
  scene: Omit<Scene, "avoid"> | null;
  states: StateInfo[];
  kind?: SystemKind;
  tool: "select" | "pan" | "add";
}) {
  const nulls = scene ? scene.nullclines.filter((n) => n.polylines.length > 0) : [];
  const glyphs = new Map<FixedPointGlyphKind, Set<string>>();
  for (const fp of scene?.fixedPoints ?? []) {
    const k = fixedPointGlyph(fp.classification, fp.stable);
    if (!glyphs.has(k)) glyphs.set(k, new Set());
    glyphs.get(k)!.add(classificationTitle(fp.classification, states.length));
  }
  const glyphName = (k: FixedPointGlyphKind, names: Set<string>) => {
    const list = [...names].sort((p, q) => Number(/saddle/i.test(p)) - Number(/saddle/i.test(q)));
    if (list.length === 1) return list[0];
    if (list.length === 2) return `${list[0]} or ${list[1].charAt(0).toLowerCase()}${list[1].slice(1)}`;
    return k === "stable"
      ? "Stable fixed point"
      : k === "unstable"
        ? "Unstable fixed point"
        : "Non-hyperbolic fixed point";
  };
  const isMap = kind === "map";
  const trajs = scene?.trajectories ?? [];
  const hasForward = trajs.some((t) => t.meta.direction === "forward");
  const hasBackward = trajs.some((t) => t.meta.direction === "backward");
  const hasStarts = (scene?.ics.length ?? 0) > 0;
  const help =
    tool === "pan"
      ? "Drag to pan, scroll to zoom, double-click to reset the view."
      : tool === "add"
        ? "Click to place a start point and drag to adjust it. Scroll to zoom."
        : "Click to add a trajectory, drag a start point to move it, drag the plot to pan, scroll to zoom, double-click to reset.";

  const forwardSample = (
    <svg viewBox="0 0 24 16" aria-hidden="true">
      <path className="ds-phase-mark ds-phase-mark--traj" d="M2 8H22" />
      <path className="ds-phase-mark ds-phase-mark--head" d="M15.5 8L8.5 4.85V11.15Z" />
    </svg>
  );
  const data: { key: string; sample: ReactNode; faint?: boolean; text: ReactNode }[] = [
    ...(hasForward && !isMap ? [{ key: "fwd", sample: forwardSample, text: "Trajectory, forward in time" }] : []),
    ...(hasForward && isMap
      ? [
          {
            key: "orbit",
            sample: (
              <svg viewBox="0 0 24 16" aria-hidden="true">
                <circle className="ds-phase-mark ds-phase-mark--handle" cx="6" cy="8" r="2" />
                <circle className="ds-phase-mark ds-phase-mark--handle" cx="12" cy="8" r="2" />
                <circle className="ds-phase-mark ds-phase-mark--handle" cx="18" cy="8" r="2" />
              </svg>
            ),
            text: "Orbit, one dot per iterate",
          },
        ]
      : []),
    ...(hasBackward ? [{ key: "back", sample: forwardSample, faint: true, text: "Trajectory, backward in time" }] : []),
    ...(hasStarts
      ? [
          {
            key: "start",
            sample: (
              <svg viewBox="0 0 24 16" aria-hidden="true">
                <circle className="ds-phase-mark ds-phase-mark--halo" cx="12" cy="8" r="7.5" />
                <circle className="ds-phase-mark ds-phase-mark--ring" cx="12" cy="8" r="6.75" />
                <circle className="ds-phase-mark ds-phase-mark--handle" cx="12" cy="8" r="3.5" />
              </svg>
            ),
            text: "Start point, drag to move",
          },
        ]
      : []),
    ...GLYPH_ORDER.filter((k) => glyphs.has(k)).map((k) => ({
      key: `fp-${k}`,
      sample: <FixedPointIcon kind={k} />,
      text: glyphName(k, glyphs.get(k)!),
    })),
  ];
  const flows = new Set<number>();
  if (scene?.eigenvectors) {
    for (const fp of scene.fixedPoints) for (const e of eigenDirections(scene, fp)) flows.add(e.flow);
  }
  const eigenSample = (flow: number) => (
    <svg viewBox="0 0 24 16" aria-hidden="true">
      <path
        className="ds-phase-mark ds-fp-eigen"
        d={
          flow > 0
            ? "M2 8H22M5.5 5.5L2 8L5.5 10.5M18.5 5.5L22 8L18.5 10.5"
            : flow < 0
              ? "M2 8H22M4.5 5.5L8 8L4.5 10.5M19.5 5.5L16 8L19.5 10.5"
              : "M2 8H22"
        }
      />
      <circle className="legend-eigen-point" cx="12" cy="8" r="2" />
    </svg>
  );
  const eigenKeys = [
    { flow: 1, text: "Unstable eigenvector" },
    { flow: -1, text: "Stable eigenvector" },
    { flow: 0, text: isMap ? "Eigenvector, |μ| = 1" : "Eigenvector, λ = 0" },
  ]
    .filter((k) => flows.has(k.flow))
    .map((k) => ({ key: `eigen${k.flow}`, sample: eigenSample(k.flow), text: k.text }));
  const background: { key: string; sample: ReactNode; text: ReactNode }[] = [
    ...(scene && scene.arrows.length > 0
      ? [
          {
            key: "field",
            sample: (
              <svg viewBox="0 0 24 16" aria-hidden="true">
                <path className="ds-phase-mark ds-phase-mark--field" d="M7 11.5L17 4.5M13.7 5L17 4.5L15.4 7.4" />
              </svg>
            ),
            text: "Direction field",
          },
        ]
      : []),
    ...nulls.map((n) => ({
      key: `n${n.state}`,
      sample: (
        <svg viewBox="0 0 24 16" aria-hidden="true">
          <path className={`ds-phase-mark ds-phase-mark--null ds-phase-mark--null-${(n.state % 4) + 1}`} d="M0 8H24" />
        </svg>
      ),
      text: (
        <span>
          Nullcline{" "}
          <Tex latex={`${isMap ? states[n.state]?.latex : dotted(states[n.state]?.latex ?? "x")}=0`} />
        </span>
      ),
    })),
    ...eigenKeys,
  ];
  const keys: typeof data = interleave(data, background);

  return (
    <div className="legend-bar">
      {keys.length > 0 && (
        <ul className="ds-phase-legend caption" aria-label="Legend">
          {keys.map((k) => (
            <li className="ds-phase-key" key={k.key}>
              <span className={"ds-phase-sample" + (k.faint ? " legend-faint" : "")}>{k.sample}</span>
              {k.text}
            </li>
          ))}
        </ul>
      )}
      {scene && <p className="legend-help caption">{help}</p>}
    </div>
  );
}

/** a[0], b[0], a[1], b[1], …, then the rest of the longer list. */
function interleave<T>(a: readonly T[], b: readonly T[]): T[] {
  const out: T[] = [];
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (i < a.length) out.push(a[i]);
    if (i < b.length) out.push(b[i]);
  }
  return out;
}

/** v, or exactly 0 when it is rounding noise next to the plot span. */
function snapNearZero(v: number, span: number): number {
  return Math.abs(v) < 1e-9 * span ? 0 : v;
}

function eigenvalueTex(ev: NumberedEigenvalue, sym: string): string {
  const label = ev.numbers.length ? `${sym}_{${ev.numbers.join(",")}}` : sym;
  const im = `${fmtTex(ev.im, 3, COMPUTED_ZERO)}i`;
  const value =
    ev.im === 0 ? fmtTex(ev.re, 3, COMPUTED_ZERO) : Math.abs(ev.re) < 5e-4 ? `\\pm ${im}` : `${fmtTex(ev.re, 3, COMPUTED_ZERO)} \\pm ${im}`;
  return `${label} = ${value}${ev.double ? "\\ (\\text{double})" : ""}`;
}

function FixedPointTooltip(props: {
  fp: FixedPointOut;
  states: StateInfo[];
  kind?: SystemKind;
  pos: [number, number];
  width: number;
  height: number;
  glyphScale: number;
}) {
  const { fp, states, kind, pos, glyphScale } = props;
  const names = states.map((s) => s.latex).join(", ");
  const values = fp.x.map((v) => fmtTex(v, 3, COMPUTED_ZERO)).join(", ");
  const point = states.length === 1 ? `${names}^* = ${values}` : `(${names}) = (${values})`;
  const sym = kind === "map" ? "\\mu" : "\\lambda";
  const glyph = fixedPointGlyph(fp.classification, fp.stable);
  const title = classificationTitle(fp.classification, states.length);
  const word = glyph !== "half" && !/stable/i.test(title) ? (fp.stable ? "stable" : "unstable") : null;
  const off = 12 * glyphScale;
  const flipX = pos[0] > props.width - 260;
  const flipY = pos[1] > props.height - 140;
  return (
    <div
      className="ds-tooltip fp-tooltip"
      role="tooltip"
      style={{
        left: pos[0] + (flipX ? -off : off),
        top: pos[1] + (flipY ? -off : off),
        transform: `translate(${flipX ? "-100%" : "0"}, ${flipY ? "-100%" : "0"})`,
      }}
    >
      <div className="ds-tooltip-row">
        <span className="fp-tooltip-glyph">
          <FixedPointIcon kind={glyph} />
        </span>
        <span className="ds-tooltip-label label">
          {title}
          {word && <span className="fp-tooltip-word">, {word}</span>}
        </span>
      </div>
      <div className="fp-tooltip-math math">
        <Tex latex={point} />
      </div>
      {numberedEigenvalues(fp).map((ev, i) => (
        <div className="fp-tooltip-math math" key={i}>
          <Tex latex={eigenvalueTex(ev, sym)} />
        </div>
      ))}
      <p className="ds-tooltip-desc caption">Click for details in the inspector.</p>
    </div>
  );
}

function describePlot(
  scene: Omit<Scene, "avoid">,
  states: StateInfo[],
  kind: SystemKind | undefined,
  xRange: [number, number],
  yRange: [number, number],
  xAxis: AxisRef,
  yAxis: AxisRef,
): string {
  const name = (a: AxisRef) => latexToPlain(axisLatex(a, states, kind));
  const num = (v: number) => fmtPlain(v).replace("-", "−");
  const r = ([a, b]: [number, number]) => `${num(a)} to ${num(b)}`;
  const span = Math.max(Math.abs(xRange[1] - xRange[0]), Math.abs(yRange[1] - yRange[0]));
  const coord = (v: number) => num(snapNearZero(v, span));
  const parts = [
    `${kind === "map" ? "Orbit plot" : "Phase plane"}: ${name(xAxis)} from ${r(xRange)} across, ${name(yAxis)} from ${r(yRange)} up.`,
  ];
  const n = scene.ics.length;
  if (n > 0) {
    const sel = scene.ics.findIndex((ic) => ic.id === scene.selectedIc);
    parts.push(
      `${n} trajector${n === 1 ? "y" : "ies"}` +
        (sel >= 0
          ? `; trajectory ${sel + 1} is selected, starting at (${scene.ics[sel].x0.map(num).join(", ")}).`
          : "."),
    );
  }
  if (scene.fixedPoints.length > 0) {
    parts.push(
      "Fixed points: " +
        scene.fixedPoints
          .map(
            (fp) =>
              `${classificationTitle(fp.classification, states.length).toLowerCase()}${states.length === 1 ? " fixed point" : ""} at (${fp.x.map(coord).join(", ")})`,
          )
          .join("; ") +
        ".",
    );
  }
  if (scene.eigenvectors && scene.fixedPoints.some((fp) => eigenDirections(scene, fp).length > 0)) {
    parts.push("Eigenvector directions are drawn through the fixed points.");
  }
  return parts.join(" ");
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

function clampTo(v: number, [a, b]: [number, number]): number {
  return Math.min(Math.max(a, b), Math.max(Math.min(a, b), v));
}

function fmtPlain(v: number): string {
  return Number(v.toPrecision(3)).toString();
}
