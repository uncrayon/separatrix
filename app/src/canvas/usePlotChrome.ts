import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { StudioActions } from "../state/useStudio";
import type { View } from "../state/model";
import type { Box } from "./draw";
import { readPlotTheme, type PlotTheme } from "./theme";
import { toPlot, unitsPerPx, zoomRange, type Viewport } from "./viewport";

const KEY_STEP = 0.05;
const CLICK_SLOP = 3;

export const sameBox = (a: Box | null, b: Box | null) =>
  !a || !b ? a === b : a.every((v, i) => Math.abs(v - b[i]) < 0.5);

/**
 * The size and theme of a plot in its `.plot-wrap`, and the boxes of its
 * toolbar and axis names (relative to the wrap) that the frame's labels avoid.
 */
export function usePlotChrome() {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const probeRef = useRef<HTMLSpanElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [th, setTh] = useState<PlotTheme | null>(null);
  const [toolbarBox, setToolbarBox] = useState<Box | null>(null);
  const [nameBoxes, setNameBoxes] = useState<Box[]>([]);

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

  return { wrapRef, canvasRef, probeRef, size, th, toolbarBox, nameBoxes };
}

/**
 * Drag to pan, scroll or + and − to zoom, arrow keys to pan (Shift for
 * bigger steps), 0 or a double-click to `fit`. A press that ends within a
 * few pixels of where it started is a click, handed to `onClick`.
 */
export function usePanZoom(
  canvasRef: React.RefObject<HTMLCanvasElement | null>,
  view: View,
  vp: Viewport,
  actions: StudioActions,
  fit: () => void,
  onClick?: (clientX: number) => void,
) {
  const [panning, setPanning] = useState(false);
  const drag = useRef<{ x: number; y: number; xRange: [number, number]; yRange: [number, number]; upp: [number, number] } | null>(null);

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
  }, [canvasRef]);

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
    if (d && !panning && e.type === "pointerup") onClick?.(e.clientX);
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
      fit();
    }
  };

  return {
    panning,
    handlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onPointerCancel: onPointerUp,
      onDoubleClick: fit,
      onKeyDown,
    },
  };
}
