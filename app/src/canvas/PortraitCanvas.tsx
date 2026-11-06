import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { AxisRef, FixedPointOut, StateInfo, SystemKind } from "../api/types";
import { Tex } from "../math/Tex";
import {
  axisLatex,
  classificationTitle,
  COMPUTED_ZERO,
  fmtTex,
  latexToPlain,
  mapMultiplier,
  mapStabilityHint,
  sameAxis,
  viewAxisLabels,
  withSubscript,
  texMinus,
} from "../math/format";
import { useCanvasTool } from "../components/CanvasToolbar";
import { IconChevronDown, IconError, IconWarning } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import {
  canAddOnPlot,
  isCobweb,
  isReturnMap,
  isSection,
  portraitFits,
  portraitGraph,
  selectFixedPoint,
  selectedFixedPointIndex,
  stateExtent,
  type View,
} from "../state/model";
import { cobwebPx, diagonalSegment, orbitValues, pathsDist2, segDist2 } from "./cobweb";
import { NumberField, Slider } from "../components/controls";
import { dec } from "../components/startPoint";
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
  const cobweb = isCobweb(view);
  const section = isSection(view);
  const returnMap = isReturnMap(view);
  const canAdd = canAddOnPlot(view);
  const viewSection = sectionKey(view);
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
    const valid = portraitFits(compiled, state.shapeId, p);
    const sameAxes =
      valid && sameAxis(p.meta.view.xAxis, view.xAxis) && sameAxis(p.meta.view.yAxis, view.yAxis);
    const colorOf = new Map(state.ics.map((ic) => [ic.id, ic.color]));
    // The staircase comes from any portrait of this map (it is the same
    // orbit); the graph of f only from one that was asked for it.
    const graph = valid ? portraitGraph(p.res) : undefined;
    const sameSection = valid && sectionKey(p.meta.view) === viewSection;
    const trajectories = sameSection
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
      ics: returnMap ? [] : state.ics,
      selectedIc: state.selection?.kind === "ic" ? state.selection.id : null,
      selectedFp:
        valid && state.layers.fixedPoints
          ? selectedFixedPointIndex(state.selection, p.res.fixedPoints, p.meta.view)
          : null,
      hoverIc: hover.ic,
      hoverFp: hover.fp,
      eigenvectors: showEigen,
      // An earlier compile's portrait, kept up because this one's could not be computed, is faded too.
      stale: !state.compileOk || (valid && p.meta.systemId !== compiled.systemId && !!state.computeError),
      cobweb: cobweb ? { graph: graph ?? [] } : null,
      section: section ? { returnMap } : null,
    };
  }, [
    compiled,
    state.shapeId,
    state.portrait,
    state.ics,
    state.layers,
    state.selection,
    state.compileOk,
    state.computeError,
    view.xAxis,
    view.yAxis,
    viewSection,
    cobweb,
    section,
    returnMap,
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
    if (top > y1 + 1e-9 * Math.abs(y1 - y0)) actions.autoAdjustView({ ...view, xRange: vp.xRange, yRange: [y0, top] });
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
    if (scene.cobweb) {
      // A cobweb's orbit is its staircase.
      let best: number | null = null;
      let bestD = tol * tol;
      for (const tr of scene.trajectories) {
        const d = pathsDist2(px, py, cobwebPx(vp, orbitValues(tr.out, scene.dim, 0), tr.out));
        if (d < bestD) {
          bestD = d;
          best = tr.meta.icId;
        }
      }
      return best;
    }
    const dotsOnly =
      !!scene.section || (scene.kind === "map" && scene.xAxis.kind !== "time" && scene.yAxis.kind !== "time");
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
      // A sideways scroll (deltaY 0) is no zoom: a new view would recompute the whole portrait.
      if (f === 1) return;
      const xRange = zoomRange(v.xRange, ax, f);
      const yRange = zoomRange(v.yRange, ay, f);
      if (xRange && yRange) act.setView({ ...v, xRange, yRange });
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });
    return () => canvas.removeEventListener("wheel", onWheel);
  }, []);

  // A cobweb's keyboard cursor moves along the diagonal, where start points are.
  const diag = cobweb ? diagonalSegment(vp.xRange, vp.yRange) : null;
  const centre = (): [number, number] => {
    const c: [number, number] = [(vp.xRange[0] + vp.xRange[1]) / 2, (vp.yRange[0] + vp.yRange[1]) / 2];
    if (!diag) return c;
    const v = clampTo((c[0] + c[1]) / 2, [diag[0], diag[2]]);
    return [v, v];
  };
  const inView = ([x, y]: [number, number]) =>
    x >= Math.min(...vp.xRange) &&
    x <= Math.max(...vp.xRange) &&
    y >= Math.min(...vp.yRange) &&
    y <= Math.max(...vp.yRange);
  const fmtPoint = ([x, y]: [number, number]) => (cobweb ? fmtPlain(x) : `${fmtPlain(x)}, ${fmtPlain(y)}`);

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
    if (e.key in moves && diag) {
      // Along the diagonal: Left and Down decrease, Right and Up increase.
      handled();
      const d = (diag[2] - diag[0]) * KEY_STEP * big;
      const sign = e.key === "ArrowLeft" || e.key === "ArrowDown" ? -1 : 1;
      const v = clampTo(cur[0] + sign * d, [diag[0], diag[2]]);
      setKbd([v, v]);
      setAnnounce(fmtPoint([v, v]));
    } else if (e.key in moves) {
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
      } else if (canAdd) {
        actions.addIc(cur[0], cur[1]);
        setAnnounce(cobweb ? `Added an orbit at x₀ = ${fmtPlain(cur[0])}` : `Added a trajectory at ${fmtPoint(cur)}`);
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
      const xRange = zoomRange(view.xRange, cur[0], f);
      const yRange = zoomRange(view.yRange, cur[1], f);
      if (xRange && yRange) actions.setView({ ...view, xRange, yRange });
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
    if (tool === "add" && canAdd) {
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
    if (canAdd) addAt(px, py);
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
  const hasSlice = !section && (hiddenStates.length > 0 || !!showTimeSlice);

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
    ? scene.cobweb
      ? describeCobweb(scene, states, vp.xRange, vp.yRange, state.solver.transient)
      : describePlot(scene, states, kind, vp.xRange, vp.yRange, view.xAxis, view.yAxis)
    : "Phase portrait";
  const names = viewAxisLabels(view, states, kind);

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
          aria-roledescription={cobweb ? "cobweb plot" : section ? "Poincaré section" : "phase plane"}
          aria-label={ariaLabel}
          aria-describedby="plot-keys"
        />
        {toolbar}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="plot-keys" className="ds-sr">
          {cobweb
            ? "Click, or press Enter, to start an orbit at that x₀. Arrow keys move the cursor along the diagonal, Shift for bigger steps. F and Shift+F step through fixed points. Plus and minus zoom, 0 resets the view. Delete removes the selected orbit."
            : returnMap
              ? "Click a dot to select its trajectory. Plus and minus zoom, 0 resets the view. Delete removes the selected trajectory."
              : "Click, or press Enter, to add a trajectory. Arrow keys move the cursor, Shift for bigger steps. F and Shift+F step through fixed points. Plus and minus zoom, 0 resets the view. Delete removes the selected trajectory."}
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
              <Tex latex={names.x} />
            </div>
            <div
              className={"axis-label axis-label-y math" + (yNameRight ? "" : " is-left")}
              style={yNameRight ? { left: yNameX, top: yNameTop } : { right: size.w - yNameX, top: yNameTop }}
              aria-hidden="true"
            >
              <Tex latex={names.y} />
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
              // Runs that all keep this state at one whole number (an invariant plane z = 0) give no span to slide over.
              const spread = ext && Math.ceil(ext[1]) > Math.floor(ext[0]);
              const lo = Math.min(spread ? Math.floor(ext[0]) : -10, state.baseState[i] ?? 0);
              const hi = Math.max(spread ? Math.ceil(ext[1]) : 10, state.baseState[i] ?? 0);
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
                ? `Choose Select or Add trajectory, then click the plot to start ${cobweb ? "an orbit" : "a trajectory"}.`
                : cobweb
                  ? "Click the plot to start an orbit at that x₀."
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
      <Legend
        scene={scene}
        states={states}
        equations={compiled?.equationsLatex ?? []}
        kind={kind}
        tool={tool}
        names={names}
      />
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
const GLYPH_NAMES: Record<FixedPointGlyphKind, string> = {
  stable: "Stable fixed point",
  unstable: "Unstable fixed point",
  half: "Non-hyperbolic fixed point",
};

function Legend({
  scene,
  states,
  equations,
  kind,
  tool,
  names,
}: {
  scene: Omit<Scene, "avoid"> | null;
  states: StateInfo[];
  /** The core's first-order system, one `\dot{x} = …` per state: its left sides spell each rate (`\dddot{x}`). */
  equations: string[];
  kind?: SystemKind;
  tool: "select" | "pan" | "add";
  /** The axis names (LaTeX): a cobweb's x_n and x_{n+1}. */
  names: { x: string; y: string };
}) {
  const rateLatex = (i: number) => {
    const eq = equations.length === states.length ? equations[i] : undefined;
    const k = eq ? eq.indexOf(" = ") : -1;
    return eq && k > 0 ? eq.slice(0, k) : dotted(states[i]?.latex ?? "x");
  };
  const nulls = scene ? scene.nullclines.filter((n) => n.polylines.length > 0) : [];
  const glyphs = new Map<FixedPointGlyphKind, Set<string>>();
  for (const fp of scene?.fixedPoints ?? []) {
    const k = fixedPointGlyph(fp.classification, fp.stable);
    if (!glyphs.has(k)) glyphs.set(k, new Set());
    glyphs.get(k)!.add(classificationTitle(fp.classification, states.length));
  }
  const cobweb = !!scene?.cobweb;
  const glyphName = (k: FixedPointGlyphKind, titles: Set<string>) => {
    if (cobweb) return GLYPH_NAMES[k];
    const list = [...titles].sort((p, q) => Number(/saddle/i.test(p)) - Number(/saddle/i.test(q)));
    if (list.length === 1) return list[0];
    if (list.length === 2) return `${list[0]} or ${list[1].charAt(0).toLowerCase()}${list[1].slice(1)}`;
    return GLYPH_NAMES[k];
  };
  const isMap = kind === "map";
  const sectionDots = !!scene?.section;
  const trajs = scene?.trajectories ?? [];
  const hasForward = trajs.some((t) => t.meta.direction === "forward");
  const hasBackward = trajs.some((t) => t.meta.direction === "backward");
  const hasStarts = (scene?.ics.length ?? 0) > 0;
  // A transient: the staircases start at x_T, marked by an open square.
  const transientShown = cobweb && trajs.some((t) => t.meta.direction === "forward" && t.out.t.length > 0 && t.out.t[0] > 0);
  const stateLatex = states[0]?.latex ?? "x";
  const help =
    tool === "pan"
      ? "Drag to pan, scroll to zoom, double-click to reset the view."
      : scene?.section?.returnMap
        ? "Click a dot to select its trajectory, drag the plot to pan, scroll to zoom, double-click to reset."
        : cobweb
        ? tool === "add"
          ? "Click to start an orbit at that x₀ and drag along the diagonal to adjust it. Scroll to zoom."
          : "Click to start an orbit at that x₀, drag a start point along the diagonal, drag the plot to pan, scroll to zoom, double-click to reset."
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
    ...(hasForward && !isMap && !sectionDots ? [{ key: "fwd", sample: forwardSample, text: "Trajectory, forward in time" }] : []),
    ...(hasForward && cobweb
      ? [
          {
            key: "cobweb",
            sample: (
              <svg viewBox="0 0 24 16" aria-hidden="true">
                <path className="ds-phase-mark ds-phase-mark--cobweb" d="M4 13V5H12V10H8V7H11" />
              </svg>
            ),
            text: "Orbit, cobweb",
          },
          ...(transientShown
            ? [
                {
                  key: "cobweb-from",
                  sample: (
                    <svg viewBox="0 0 24 16" aria-hidden="true">
                      <rect className="ds-phase-mark ds-phase-mark--from" x="9" y="5" width="6" height="6" />
                    </svg>
                  ),
                  text: (
                    <span>
                      Orbit drawn from <Tex latex={withSubscript(stateLatex, "T")} />, after the transient
                    </span>
                  ),
                },
              ]
            : []),
        ]
      : []),
    ...(hasForward && sectionDots
      ? [
          {
            key: "crossings",
            sample: (
              <svg viewBox="0 0 24 16" aria-hidden="true">
                <circle className="ds-phase-mark ds-phase-mark--handle" cx="6" cy="8" r="2" />
                <circle className="ds-phase-mark ds-phase-mark--handle" cx="12" cy="8" r="2" />
                <circle className="ds-phase-mark ds-phase-mark--handle" cx="18" cy="8" r="2" />
              </svg>
            ),
            text: scene?.section?.returnMap ? "A crossing and the next, one dot per pair" : "Orbit, one dot per crossing",
          },
        ]
      : []),
    ...(hasForward && isMap && !cobweb
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
            text: cobweb ? "Start point, drag along the diagonal" : "Start point, drag to move",
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
  const graphShown = (scene?.cobweb?.graph ?? []).some((l) => l.length > 1);
  const background: { key: string; sample: ReactNode; text: ReactNode }[] = [
    ...(cobweb && graphShown
      ? [
          {
            key: "graph",
            sample: (
              <svg viewBox="0 0 24 16" aria-hidden="true">
                <path className="ds-phase-mark ds-phase-mark--graph" d="M2 14Q12 -2 22 14" />
              </svg>
            ),
            text: (
              <span>
                Graph <Tex latex={`${names.y} = f(${names.x})`} />
              </span>
            ),
          },
        ]
      : []),
    ...(cobweb
      ? [
          {
            key: "diagonal",
            sample: (
              <svg viewBox="0 0 24 16" aria-hidden="true">
                <path className="ds-phase-mark ds-phase-mark--diagonal" d="M5 14L17 2" />
              </svg>
            ),
            text: (
              <span>
                Diagonal <Tex latex={`${names.y} = ${names.x}`} />
              </span>
            ),
          },
        ]
      : []),
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
          <Tex latex={`${isMap ? states[n.state]?.latex : rateLatex(n.state)}=0`} />
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
  // A 1-D map's one multiplier is λ = f′(x*) (Strogatz §10.1).
  const map1 = kind === "map" && states.length === 1;
  const lambda = map1 ? mapMultiplier(fp) : NaN;
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
      {map1 ? (
        <>
          {Number.isFinite(lambda) ? (
            <>
              <div className="fp-tooltip-math math">
                <Tex latex={`\\lambda = f'(${names}^*) = ${texDec(lambda)}`} />
              </div>
              <div className="fp-tooltip-math math">
                <Tex latex={`|\\lambda| = ${texDec(Math.abs(lambda))}`} />
              </div>
            </>
          ) : (
            <div className="fp-tooltip-math math">
              <Tex latex={`f'(${names}^*)\\text{ does not exist}`} />
            </div>
          )}
          <p className="ds-tooltip-desc caption">{mapStabilityHint(fp, latexToPlain(states[0].latex)).replace(/-/g, "−")}</p>
        </>
      ) : (
        numberedEigenvalues(fp).map((ev, i) => (
          <div className="fp-tooltip-math math" key={i}>
            <Tex latex={eigenvalueTex(ev, sym)} />
          </div>
        ))
      )}
      <p className="ds-tooltip-desc caption">Click for details in the inspector.</p>
    </div>
  );
}

/** Three decimals, as the inspector's row ("−1.200"), for LaTeX. */
const texDec = (v: number) => texMinus(dec(v, 3));

/** Unicode subscripts for the digits, signs and n of a cobweb's axis names. */
const SUB: Record<string, string> = { n: "ₙ", "+": "₊", "-": "₋", "0": "₀", "1": "₁", "2": "₂", "3": "₃", "4": "₄", "5": "₅", "6": "₆", "7": "₇", "8": "₈", "9": "₉" };
const sub = (text: string) => [...text].map((c) => SUB[c] ?? c).join("");

/**
 * The cobweb for a screen reader: "Cobweb plot of xₙ₊₁ = f(xₙ): xₙ from 0
 * to 1 across, xₙ₊₁ from 0 to 1 up. 1 orbit, starting at x₀ = 0.1. Fixed
 * points: unstable at x* = 0 (λ = 3.2); …" (with ", first T iterates
 * hidden" after the start when there is a transient).
 */
function describeCobweb(
  scene: Omit<Scene, "avoid">,
  states: StateInfo[],
  xRange: [number, number],
  yRange: [number, number],
  transient: number,
): string {
  const x = latexToPlain(states[0]?.latex ?? "x");
  const num = (v: number) => fmtPlain(v).replace("-", "−");
  const r = ([a, b]: [number, number]) => `${num(a)} to ${num(b)}`;
  const span = Math.max(Math.abs(xRange[1] - xRange[0]), Math.abs(yRange[1] - yRange[0]));
  const xn = `${x}${sub("n")}`;
  const xn1 = `${x}${sub("n+1")}`;
  const parts = [`Cobweb plot of ${xn1} = f(${xn}): ${xn} from ${r(xRange)} across, ${xn1} from ${r(yRange)} up.`];
  const ics = scene.ics.filter((ic) => ic.x0.length === scene.dim);
  if (ics.length > 0) {
    const sel = ics.findIndex((ic) => ic.id === scene.selectedIc);
    parts.push(
      `${ics.length} orbit${ics.length === 1 ? "" : "s"}, starting at ${x}${sub("0")} = ${ics.map((ic) => num(ic.x0[0])).join(", ")}` +
        (transient > 0 ? `, first ${transient.toLocaleString("en-US")} iterate${transient === 1 ? "" : "s"} hidden` : "") +
        (sel >= 0 ? `; orbit ${sel + 1} is selected.` : "."),
    );
  }
  if (scene.fixedPoints.length > 0) {
    parts.push(
      "Fixed points: " +
        scene.fixedPoints
          .map((fp) => {
            const word = fixedPointGlyph(fp.classification, fp.stable) === "half" ? "non-hyperbolic" : fp.stable ? "stable" : "unstable";
            const lambda = mapMultiplier(fp);
            const why = Number.isFinite(lambda) ? `λ = ${num(snapNearZero(lambda, 1))}` : "no λ: f is not differentiable there";
            return `${word} at ${x}* = ${num(snapNearZero(fp.x[0], span))} (${why})`;
          })
          .join("; ") +
        ".",
    );
  }
  return parts.join(" ");
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
  const what = scene.section ? "Poincaré section" : kind === "map" ? "Orbit plot" : "Phase plane";
  const [xName, yName] = scene.section?.returnMap
    ? [`${name(xAxis)} k`, `${name(yAxis)} k+1`]
    : [name(xAxis), name(yAxis)];
  const parts = [`${what}: ${xName} from ${r(xRange)} across, ${yName} from ${r(yRange)} up.`];
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

function sectionKey(v: View): string {
  return isSection(v) ? JSON.stringify(v.section) : "";
}

function clampTo(v: number, [a, b]: [number, number]): number {
  return Math.min(Math.max(a, b), Math.max(Math.min(a, b), v));
}

function fmtPlain(v: number): string {
  return Number(v.toPrecision(3)).toString();
}
