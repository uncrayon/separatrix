// The centre of the app: the phase portrait canvas and its overlays
// (typeset axis names, fixed-point tooltip, slice card, hints) and the legend.
//
// What the pointer does depends on the canvas tool (CanvasToolbar):
// - Select (default): click = add an initial condition, drag a start point =
//   move it (live re-solve), click a fixed point or a curve = select it, drag
//   empty space = pan;
// - Pan: drag anywhere = pan;
// - Add trajectory: click anywhere (also on a curve or a fixed point) = add a
//   start point there, keep dragging to adjust it.
// In every tool: wheel = zoom around the cursor, double-click = reset the view.
//
// Keyboard (the canvas is focusable): arrow keys move a cursor ring (Shift =
// bigger steps), Enter/Space adds a trajectory there or selects the mark under
// it, F / Shift+F step through fixed points, + / − zoom around the cursor,
// 0 resets the view, Delete removes the selected trajectory, Escape deselects.

import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { AxisRef, FixedPointOut, SymbolInfo, SystemKind } from "../api/types";
import { Tex } from "../math/Tex";
import { axisLabelLatex, axisLatex, classificationTitle, fmtTex, latexToPlain, sameAxis } from "../math/format";
import { useCanvasTool } from "../components/CanvasToolbar";
import { IconChevronDown, IconError, IconWarning } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { stateExtent } from "../state/model";
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
import { makeViewport, sx, sy, toPlot, unitsPerPx, wantsEqualScales, zoomRange } from "./viewport";

interface Props {
  state: StudioState;
  actions: StudioActions;
  theme: "dark" | "light";
  /** The floating CanvasToolbar, rendered inside the plot well (CanvasToolbar README). */
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
      /** Plot units per px when the drag started (the view on screen). */
      upp: [number, number];
      moved: boolean;
      /** Select tool: a click (no move) adds a start point. */
      clickAdds: boolean;
    };

/** Hit radius for start points and fixed points, × g (PhasePlane README). */
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
const axisLabel = axisLabelLatex;

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
  /** Bumped when the theme attribute changes or web fonts finish loading: redraw with fresh tokens. */
  const [themeVersion, setThemeVersion] = useState(0);
  /** The floating toolbar's box, relative to the plot well (null when absent). */
  const [toolbarBox, setToolbarBox] = useState<Box | null>(null);
  /** The slice card moves below the toolbar when both would share the top row. */
  const [sliceBelow, setSliceBelow] = useState(false);
  /** Size of the typeset x-axis name, measured after layout, to keep it clear of the toolbar. */
  const [xNameSize, setXNameSize] = useState<[number, number]>([12, 20]);
  /** Boxes of the typeset axis names, measured after layout, for the renderer. */
  const nameBoxes = useRef<Box[]>([]);
  /** Keyboard cursor in plot coordinates; null when not using the keyboard. */
  const [kbd, setKbd] = useState<[number, number] | null>(null);
  /** Last keyboard action, read out by screen readers. */
  const [announce, setAnnounce] = useState("");
  /**
   * The slice card can be folded into one line so it never hides the plot.
   * Until the user chooses (null), it starts folded on narrow plots, where the
   * open card would cover a large part of the picture.
   */
  const [sliceChoice, setSliceOpen] = useState<boolean | null>(null);
  const sliceOpen = sliceChoice ?? size.w >= SLICE_CARD_MIN_PLOT_W;
  const tool = useCanvasTool();
  const showEigen = useShowEigenvectors();

  const { compiled, view } = state;
  /** Glyph scale g: hit radii grow with the glyphs in projector mode. */
  const g = theme === "light" ? 1.3 : 1;
  // Phase planes of flows use equal scales on both axes (PhasePlane README).
  const vp = makeViewport(
    size.w,
    size.h,
    view.xRange,
    view.yRange,
    g,
    wantsEqualScales(view.xAxis, view.yAxis, compiled?.kind),
  );

  // The plot's aspect goes to the state, so the field is computed for exactly
  // what is on screen.
  useEffect(() => {
    actions.setPlotAspect(size.w > 0 && size.h > 0 ? size.w / size.h : null);
  }, [actions, size.w, size.h]);

  // ---- size (CSS px), kept in sync with the layout
  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  // ---- redraw on theme change (any writer of html[data-theme]) and when fonts load
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

  // ---- where the chrome over the plot sits (every commit; cheap, and the
  // toolbar changes with the same state that renders this component)
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

  // ---- what to draw
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
      selectedFp: state.selection?.kind === "fp" ? state.selection.index : null,
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

  // ---- keep the default view clear of the toolbar (CanvasToolbar README: "keep
  // the plot's default view padded so fixed points and starts are not under
  // it"). Only while the view is the home view, as loaded or reset: once the
  // user pans or zooms, the view is theirs.
  useEffect(() => {
    if (!scene || !toolbarBox || size.w === 0 || view !== state.homeView) return;
    const [l, , r, b] = toolbarBox;
    const pad = 12 * g;
    const marks: [number, number][] = [
      ...scene.ics.filter((ic) => ic.x0.length === scene.dim).map((ic) => icPx(vp, scene, ic)),
      ...scene.fixedPoints.map((fp) => fixedPointPx(vp, scene, fp)),
    ];
    const target = b + pad;
    const H = vp.height;
    if (target >= H * 0.6) return; // a tiny plot: nothing sensible to do
    const [y0, y1] = vp.yRange;
    let top = y1;
    for (const [px, py] of marks) {
      if (px < l - pad || px > r + pad || py >= target || py < vp.top) continue;
      // The y value of this mark, and the top that puts it `target` px down.
      const y = y1 - ((py - vp.top) / H) * (y1 - y0);
      const f = (target - vp.top) / H;
      top = Math.max(top, (y - f * y0) / (1 - f));
    }
    if (top > y1 + 1e-9 * Math.abs(y1 - y0)) actions.setView({ ...view, xRange: vp.xRange, yRange: [y0, top] });
    // vp and scene follow from these.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scene, toolbarBox, size.w, size.h, view, state.homeView]);

  // ---- draw (after layout effects, so the theme attribute is already applied
  // and the chrome over the plot has been measured)
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
    const key = `${document.documentElement.dataset.theme ?? theme}:${themeVersion}`;
    if (themeCache.current?.key !== key) themeCache.current = { key, th: readPlotTheme(probeRef.current) };
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const avoid = {
      labels: [...(toolbarBox ? [toolbarBox] : []), ...nameBoxes.current],
      field: nameBoxes.current.map(([l, t, r, b]): Box => [l - 3, t - 3, r + 3, b + 3]),
    };
    drawScene(ctx, size.w, size.h, vp, scene && { ...scene, avoid }, themeCache.current.th);
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
    let bestD = (HIT_RADIUS * g) ** 2;
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
    const tol = 5 * g;
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
  // The ranges on screen (vp), which a phase plane widens to equal scales.
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
      // Place the start point now; keep dragging to adjust it.
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
      actions.select({ kind: "fp", index: hit.fp });
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

  // ---- overlays
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

  // Axis names: x at the right end above the x-axis, y at the top right of the
  // y-axis (both pinned to an edge when the axis is out of view). Either name
  // drops below the toolbar rather than hide under it.
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
            // Show the keyboard cursor only for keyboard focus, not for clicks.
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
        {/* Probe for the tick-label font: the numeric-sm type style, as computed. */}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="plot-keys" className="sr-only">
          Click, or press Enter, to add a trajectory. Arrow keys move the cursor, Shift for bigger steps. F
          and Shift+F step through fixed points. Plus and minus zoom, 0 resets the view. Delete removes the
          selected trajectory.
        </span>
        <span className="sr-only" aria-live="polite" aria-atomic="true">
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
              <Tex latex={axisLabel(view.xAxis, view.yAxis, states, kind)} />
            </div>
            <div
              className={"axis-label axis-label-y math" + (yNameRight ? "" : " is-left")}
              style={yNameRight ? { left: yNameX, top: yNameTop } : { right: size.w - yNameX, top: yNameTop }}
              aria-hidden="true"
            >
              <Tex latex={axisLabel(view.yAxis, view.xAxis, states, kind)} />
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
            g={g}
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

/** A fixed-point glyph as a 16px icon (design system: FixedPoint, `.ds-fp-icon`). */
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

/**
 * The legend under the plot (design system: PhasePlane `.ds-phase-legend`):
 * HTML, one key per kind of mark on the plot, each sample on a plot-bg chip.
 */
function Legend({
  scene,
  states,
  kind,
  tool,
}: {
  scene: Omit<Scene, "avoid"> | null;
  states: SymbolInfo[];
  kind?: SystemKind;
  tool: "select" | "pan" | "add";
}) {
  const nulls = scene ? scene.nullclines.filter((n) => n.polylines.length > 0) : [];
  // Fixed points by glyph, named by the classifications on the plot.
  const glyphs = new Map<FixedPointGlyphKind, Set<string>>();
  for (const fp of scene?.fixedPoints ?? []) {
    const k = fixedPointGlyph(fp.classification, fp.stable);
    if (!glyphs.has(k)) glyphs.set(k, new Set());
    glyphs.get(k)!.add(classificationTitle(fp.classification, states.length));
  }
  const glyphName = (k: FixedPointGlyphKind, names: Set<string>) => {
    // Saddles last: "Unstable node or saddle", as the PhasePlane legend writes it.
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
  // Two kinds of key (PhasePlane README): what the user drags or reads off,
  // and the background marks. Listed alternately, so in a two-column legend
  // each kind lines up in its own column.
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
  // Eigenvector directions, one key per kind of head on the plot.
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
  const keys: typeof data = [];
  for (let i = 0; i < Math.max(data.length, background.length); i++) {
    if (data[i]) keys.push(data[i]);
    if (background[i]) keys.push(background[i]);
  }

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

/** The tooltip line for one eigenvalue or pair ("λ₂,₃ = 0.094 ± 10.2i" as TeX), numbered as the Inspector row. */
function eigenvalueTex(ev: NumberedEigenvalue, sym: string): string {
  const label = ev.numbers.length ? `${sym}_{${ev.numbers.join(",")}}` : sym;
  const im = `${fmtTex(ev.im, 3)}i`;
  const value =
    ev.im === 0 ? fmtTex(ev.re, 3) : Math.abs(ev.re) < 5e-4 ? `\\pm ${im}` : `${fmtTex(ev.re, 3)} \\pm ${im}`;
  return `${label} = ${value}${ev.double ? "\\ (\\text{double})" : ""}`;
}

function FixedPointTooltip(props: {
  fp: FixedPointOut;
  states: SymbolInfo[];
  kind?: SystemKind;
  pos: [number, number];
  width: number;
  height: number;
  g: number;
}) {
  const { fp, states, kind, pos, g } = props;
  const names = states.map((s) => s.latex).join(", ");
  const values = fp.x.map((v) => fmtTex(v, 3)).join(", ");
  const point = states.length === 1 ? `${names}^* = ${values}` : `(${names}) = (${values})`;
  const sym = kind === "map" ? "\\mu" : "\\lambda";
  const glyph = fixedPointGlyph(fp.classification, fp.stable);
  const title = classificationTitle(fp.classification, states.length);
  // The word "stable" or "unstable" when the name does not already say it.
  const word = glyph !== "half" && !/stable/i.test(title) ? (fp.stable ? "stable" : "unstable") : null;
  const off = 12 * g;
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

/** The canvas's accessible name: the view, the trajectories and the fixed points. */
function describePlot(
  scene: Omit<Scene, "avoid">,
  states: SymbolInfo[],
  kind: SystemKind | undefined,
  xRange: [number, number],
  yRange: [number, number],
  xAxis: AxisRef,
  yAxis: AxisRef,
): string {
  const name = (a: AxisRef) => latexToPlain(axisLatex(a, states, kind));
  const num = (v: number) => fmtPlain(v).replace("-", "−");
  const r = ([a, b]: [number, number]) => `${num(a)} to ${num(b)}`;
  // Fixed-point coordinates that are zero up to Newton's rounding (−6.58e-14 at
  // the origin) read as 0, as the Inspector rows show them.
  const span = Math.max(Math.abs(xRange[1] - xRange[0]), Math.abs(yRange[1] - yRange[0]));
  const coord = (v: number) => num(Math.abs(v) < 1e-9 * span ? 0 : v);
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

/** Clamp `v` into a range given in either order. */
function clampTo(v: number, [a, b]: [number, number]): number {
  return Math.min(Math.max(a, b), Math.max(Math.min(a, b), v));
}

/** A coordinate for screen-reader announcements (3 significant digits). */
function fmtPlain(v: number): string {
  return Number(v.toPrecision(3)).toString();
}
