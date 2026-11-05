import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  BufferAttribute,
  BufferGeometry,
  Line,
  LineBasicMaterial,
  OrthographicCamera,
  Scene,
  WebGLRenderer,
  type Material,
  type Object3D,
} from "three";
import { Line2 } from "three/addons/lines/Line2.js";
import { LineGeometry } from "three/addons/lines/LineGeometry.js";
import { LineMaterial } from "three/addons/lines/LineMaterial.js";
import { latexToPlain } from "../math/format";
import { IconInfo } from "../components/icons";
import type { StudioActions, StudioState } from "../state/useStudio";
import { isSection, portraitFits, selectedFixedPointIndex, stateAxis, type SpaceView } from "../state/model";
import {
  clampCamera,
  DEFAULT_CAMERA,
  drawSpaceLines,
  drawSpaceOverlay,
  spaceAxes,
  spaceNames,
  threeCamera,
  trajectoryBoxCoords,
  trajectoryWidth,
  type Camera,
  type SpaceBox,
  type SpaceFigure,
  type SpaceTrajectory,
} from "./space";
import { readPlotTheme, type PlotTheme } from "./theme";

interface Props {
  state: StudioState;
  actions: StudioActions;
  toolbar?: ReactNode;
}

const TURN_PER_PX = 0.01;
const KEY_TURN = Math.PI / 36;
const KEY_ZOOM = 1.25;
const WHEEL_SETTLE_MS = 250;
/**
 * In software WebGL (SwiftShader, measured) a wide line costs about 20 µs a
 * segment, so 8 orbits of 20,000 samples take seconds a frame, and a thin
 * one about 2 µs a vertex over a floor of about 5 ms.
 */
const TURNING_VERTICES = 4000;
const FAST_FRAME_MS = 8;

interface Gl {
  renderer: WebGLRenderer;
  scene: Scene;
  camera: OrthographicCamera;
  wide: Object3D[];
  thin: Object3D[];
  disposables: { dispose(): void }[];
  restMs: number;
  settle: number;
}

function makeGl(canvas: HTMLCanvasElement): Gl | null {
  try {
    // No multisampling: in software WebGL it doubled a turning frame (54 ms against 24, measured).
    const renderer = new WebGLRenderer({ canvas, antialias: false });
    return { renderer, scene: new Scene(), camera: new OrthographicCamera(), wide: [], thin: [], disposables: [], restMs: Infinity, settle: 0 };
  } catch {
    return null;
  }
}

function clearLines(gl: Gl) {
  for (const o of [...gl.wide, ...gl.thin]) gl.scene.remove(o);
  for (const d of gl.disposables) d.dispose();
  gl.wide = [];
  gl.thin = [];
  gl.disposables = [];
}

function thinned(q: Float64Array, max: number): Float32Array {
  const n = q.length / 3;
  const k = Math.max(1, Math.ceil(n / max));
  const keep = Math.ceil((n - 1) / k) + 1;
  const out = new Float32Array(keep * 3);
  for (let j = 0; j < keep; j++) {
    const i = Math.min(n - 1, j * k);
    out.set(q.subarray(3 * i, 3 * i + 3), 3 * j);
  }
  return out;
}

function wideLine(q: Float64Array, tr: SpaceTrajectory, th: PlotTheme, w: number, h: number) {
  const geometry = new LineGeometry();
  geometry.setPositions(Float32Array.from(q));
  const material = new LineMaterial({
    color: th.traj[tr.color % th.traj.length],
    linewidth: trajectoryWidth(tr, th),
    transparent: tr.backward,
    opacity: tr.backward ? 0.5 : 1,
  });
  material.resolution.set(Math.max(1, w), Math.max(1, h));
  const line = new Line2(geometry, material);
  line.renderOrder = tr.selected ? 2 : tr.backward ? 0 : 1;
  return { line, disposables: [geometry, material] };
}

function setCamera(gl: Pick<Gl, "camera">, c: Camera, w: number, h: number) {
  const t = threeCamera(c, w, h);
  const cam = gl.camera;
  cam.position.set(...t.position);
  cam.up.set(...t.up);
  cam.lookAt(0, 0, 0);
  cam.left = t.left;
  cam.right = t.right;
  cam.top = t.top;
  cam.bottom = t.bottom;
  cam.near = t.near;
  cam.far = t.far;
  cam.updateProjectionMatrix();
  cam.updateMatrixWorld();
}

export function renderSpaceFigure(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  scale: number,
  fig: SpaceFigure,
  th: PlotTheme,
  background = true,
): void {
  if (background) {
    ctx.fillStyle = th.bg;
    ctx.fillRect(0, 0, w, h);
  }
  if (!drawWebGlLines(ctx, w, h, scale, fig, th)) drawSpaceLines(ctx, fig.view, w, h, fig.trajectories, fig.marks.dim, th);
  drawSpaceOverlay(ctx, fig.view, w, h, fig.marks, th);
}

function drawWebGlLines(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  scale: number,
  fig: SpaceFigure,
  th: PlotTheme,
): boolean {
  const canvas = document.createElement("canvas");
  let renderer: WebGLRenderer;
  try {
    renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
  } catch {
    return false;
  }
  const disposables: { dispose(): void }[] = [];
  try {
    renderer.setPixelRatio(scale);
    renderer.setSize(w, h, false);
    renderer.setClearColor(th.bg, 0);
    const scene = new Scene();
    for (const tr of fig.trajectories) {
      const q = trajectoryBoxCoords(fig.view, tr.out, fig.marks.dim);
      if (q.length < 6) continue;
      const made = wideLine(q, tr, th, w, h);
      scene.add(made.line);
      disposables.push(...made.disposables);
    }
    const camera = new OrthographicCamera();
    setCamera({ camera }, fig.view.camera, w, h);
    renderer.render(scene, camera);
    ctx.drawImage(canvas, 0, 0, w, h);
    return true;
  } finally {
    for (const d of disposables) d.dispose();
    renderer.dispose();
    renderer.forceContextLoss();
  }
}

export default function SpaceCanvas({ state, actions, toolbar }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const glCanvasRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const probeRef = useRef<HTMLSpanElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [gl, setGl] = useState<Gl | null | undefined>(undefined);
  const [th, setTh] = useState<PlotTheme>(() => readPlotTheme());
  const live = useRef<Camera | null>(null);
  const drag = useRef<{ x: number; y: number; from: Camera } | null>(null);
  const wheelTimer = useRef(0);

  const view = state.view as SpaceView;
  const { compiled } = state;
  const dim = compiled?.states.length ?? 0;

  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  useLayoutEffect(() => {
    const made = glCanvasRef.current ? makeGl(glCanvasRef.current) : null;
    setGl(made);
    return () => {
      if (!made) return;
      cancelAnimationFrame(made.settle);
      clearLines(made);
      made.renderer.dispose();
    };
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

  const marks = useMemo(() => {
    const p = state.portrait;
    const valid = !!compiled && portraitFits(compiled, state.shapeId, p);
    const colorOf = new Map(state.ics.map((ic) => [ic.id, ic]));
    const selectedIc = state.selection?.kind === "ic" ? state.selection.id : null;
    const trajectories: SpaceTrajectory[] =
      valid && !isSection(p.meta.view)
        ? p.res.trajectories.flatMap((out, i) => {
            const meta = p.meta.trajectories[i];
            const ic = meta ? colorOf.get(meta.icId) : undefined;
            if (!meta || !ic || ic.hidden) return [];
            return [{ out, color: ic.color, backward: meta.direction === "backward", selected: ic.id === selectedIc }];
          })
        : [];
    const fixedPoints = valid && state.layers.fixedPoints ? p.res.fixedPoints : [];
    const names = spaceNames(view, compiled?.states ?? []);
    return {
      trajectories,
      overlay: {
        dim,
        names,
        fixedPoints,
        ics: state.ics,
        selectedIc,
        selectedFp: valid && state.layers.fixedPoints ? selectedFixedPointIndex(state.selection, fixedPoints, p.meta.view) : null,
      },
    };
  }, [compiled, dim, state.portrait, state.shapeId, state.ics, state.layers.fixedPoints, state.selection, view.xAxis, view.yAxis, view.zAxis]);

  const [ix, iy, iz] = spaceAxes(view);
  const [x0, x1] = view.xRange;
  const [y0, y1] = view.yRange;
  const [z0, z1] = view.zRange;
  const box = useMemo<Omit<SpaceBox, "camera">>(
    () => ({
      xAxis: stateAxis(ix),
      yAxis: stateAxis(iy),
      zAxis: stateAxis(iz),
      xRange: [x0, x1],
      yRange: [y0, y1],
      zRange: [z0, z1],
    }),
    [ix, iy, iz, x0, x1, y0, y1, z0, z1],
  );

  useLayoutEffect(() => {
    if (!gl) return;
    clearLines(gl);
    const turning = Math.max(200, Math.floor(TURNING_VERTICES / Math.max(1, marks.trajectories.length)));
    for (const tr of marks.trajectories) {
      const q = trajectoryBoxCoords(box, tr.out, dim);
      if (q.length < 6) continue;
      const color = th.traj[tr.color % th.traj.length];
      const { line: wide, disposables } = wideLine(q, tr, th, size.w, size.h);
      const thinGeometry = new BufferGeometry();
      thinGeometry.setAttribute("position", new BufferAttribute(thinned(q, turning), 3));
      const thinMaterial: Material = new LineBasicMaterial({ color, transparent: tr.backward, opacity: tr.backward ? 0.5 : 1 });
      const thin = new Line(thinGeometry, thinMaterial);
      thin.renderOrder = wide.renderOrder;
      thin.visible = false;
      gl.scene.add(wide, thin);
      gl.wide.push(wide);
      gl.thin.push(thin);
      gl.disposables.push(...disposables, thinGeometry, thinMaterial);
    }
  }, [gl, marks.trajectories, box, th, size.w, size.h, dim]);

  const draw = useCallback((camera: Camera, lines: boolean) => {
    const overlay = overlayRef.current;
    const w = size.w;
    const h = size.h;
    if (!overlay || w === 0 || h === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(w * dpr);
    const H = Math.round(h * dpr);
    if (overlay.width !== W || overlay.height !== H) {
      overlay.width = W;
      overlay.height = H;
    }
    const v = { ...box, camera };
    if (gl) {
      const wide = lines || gl.restMs < FAST_FRAME_MS;
      for (const o of gl.wide) o.visible = wide;
      for (const o of gl.thin) o.visible = !wide;
      // Resizing reallocates the drawing buffer (setting a canvas's width clears it), so only on a change.
      const glCanvas = gl.renderer.domElement;
      if (glCanvas.width !== W || glCanvas.height !== H || gl.renderer.getPixelRatio() !== dpr) {
        gl.renderer.setPixelRatio(dpr);
        gl.renderer.setSize(w, h, false);
      }
      gl.renderer.setClearColor(th.bg, 1);
      setCamera(gl, camera, w, h);
      const started = performance.now();
      gl.renderer.render(gl.scene, gl.camera);
      if (lines) {
        // Reading a pixel back waits for the frame, so this is what the wide lines cost.
        const gc = gl.renderer.getContext();
        gc.readPixels(0, 0, 1, 1, gc.RGBA, gc.UNSIGNED_BYTE, new Uint8Array(4));
        gl.restMs = performance.now() - started;
        // WebKitGTK (seen on Mesa's llvmpipe) can leave the frame before this one on screen
        // until something else repaints; drawing it again two frames later puts it there.
        const again = gl;
        cancelAnimationFrame(again.settle);
        again.settle = requestAnimationFrame(() => {
          again.settle = requestAnimationFrame(() => again.renderer.render(again.scene, again.camera));
        });
      } else {
        cancelAnimationFrame(gl.settle);
      }
    }
    const ctx = overlay.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    if (!gl) {
      ctx.fillStyle = th.bg;
      ctx.fillRect(0, 0, w, h);
      if (lines) drawSpaceLines(ctx, v, w, h, marks.trajectories, dim, th);
    }
    drawSpaceOverlay(ctx, v, w, h, marks.overlay, th);
  }, [size.w, size.h, gl, box, th, marks, dim]);

  useLayoutEffect(() => {
    if (gl !== undefined) draw(live.current ?? view.camera, true);
  }, [draw, gl, view.camera]);

  const commit = (c: Camera) => {
    live.current = null;
    actions.setCamera(c);
  };
  const current = () => live.current ?? view.camera;
  const preview = (c: Camera) => {
    live.current = c;
    draw(c, false);
  };

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const c = current();
    preview(clampCamera({ ...c, zoom: c.zoom * Math.exp(-e.deltaY * 0.0015) }));
    window.clearTimeout(wheelTimer.current);
    wheelTimer.current = window.setTimeout(() => live.current && commit(live.current), WHEEL_SETTLE_MS);
  };
  const wheelRef = useRef(onWheel);
  wheelRef.current = onWheel;
  useEffect(() => {
    const el = overlayRef.current;
    if (!el) return;
    const listener = (e: WheelEvent) => wheelRef.current(e);
    el.addEventListener("wheel", listener, { passive: false });
    return () => {
      el.removeEventListener("wheel", listener);
      window.clearTimeout(wheelTimer.current);
    };
  }, []);

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, from: current() };
  };
  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    preview(clampCamera({ ...d.from, yaw: d.from.yaw + dx * TURN_PER_PX, pitch: d.from.pitch + dy * TURN_PER_PX }));
  };
  const onPointerUp = () => {
    drag.current = null;
    if (live.current) commit(live.current);
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    const c = current();
    const turn = e.shiftKey ? 3 * KEY_TURN : KEY_TURN;
    const next: Partial<Record<string, Camera>> = {
      ArrowLeft: { ...c, yaw: c.yaw - turn },
      ArrowRight: { ...c, yaw: c.yaw + turn },
      ArrowUp: { ...c, pitch: c.pitch - turn },
      ArrowDown: { ...c, pitch: c.pitch + turn },
      "+": { ...c, zoom: c.zoom * KEY_ZOOM },
      "=": { ...c, zoom: c.zoom * KEY_ZOOM },
      "-": { ...c, zoom: c.zoom / KEY_ZOOM },
      "−": { ...c, zoom: c.zoom / KEY_ZOOM },
      "0": DEFAULT_CAMERA,
    };
    const to = next[e.key];
    if (!to) return;
    e.preventDefault();
    commit(clampCamera(to));
  };

  const states = compiled?.states ?? [];
  const [nx, ny, nz] = marks.overlay.names.map(latexToPlain);
  const label = `3D view of ${nx}, ${ny} and ${nz}: ${marks.trajectories.length} trajectories, ${marks.overlay.fixedPoints.length} fixed points.`;

  return (
    <>
      <div className="plot-wrap space-wrap" ref={wrapRef}>
        <canvas ref={glCanvasRef} className="space-gl" aria-hidden="true" hidden={gl === null} />
        <canvas
          ref={overlayRef}
          className="plot-canvas space-overlay"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onDoubleClick={() => commit(DEFAULT_CAMERA)}
          onKeyDown={onKeyDown}
          tabIndex={0}
          role="application"
          aria-roledescription="3D view"
          aria-label={label}
          aria-describedby="space-keys"
        />
        {toolbar}
        <span ref={probeRef} className="numeric-sm plot-probe" aria-hidden="true" />
        <span id="space-keys" className="ds-sr">
          Arrow keys turn the box, Shift for bigger steps. Plus and minus zoom, 0 resets the camera.
        </span>
        {gl === null && (
          <div className="plot-hint">
            <div className="ds-diagnostic ds-diagnostic--block ds-diagnostic--info" role="status">
              <IconInfo className="ds-diagnostic-icon" />
              <span className="ds-diagnostic-body">
                <span className="ds-diagnostic-word">Note</span>
                <span className="ds-diagnostic-msg">
                  This 3D view is drawn without WebGL, so the trajectories are redrawn when you stop dragging.
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
        {states.length > 0 && (
          <p className="legend-help caption">
            Drag to turn the box, scroll to zoom, double-click to reset. Add start points in the Trajectories panel or
            another view.
          </p>
        )}
      </div>
    </>
  );
}
