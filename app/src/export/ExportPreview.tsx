import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { Scene } from "../canvas/draw";
import { drawFigure, type FigureFonts } from "../canvas/figure";
import type { PlotTheme } from "../canvas/theme";
import { makeViewport } from "../canvas/viewport";

interface Props {
  /** The figure's size in CSS px; the preview draws it scaled to fit. */
  widthPx: number;
  heightPx: number;
  xRange: [number, number] | null;
  yRange: [number, number] | null;
  scene: Scene | null;
  theme: PlotTheme;
  names: { x: string; y: string } | null;
  fonts: FigureFonts;
  transparent: boolean;
  label: string;
  caption: ReactNode;
  busy: boolean;
}

/**
 * The exported figure, drawn by the same code as the PNG and SVG (drawFigure)
 * at the figure's own size and scaled into the well, so line weights and
 * type sit as they will in the file.
 */
export function ExportPreview({
  widthPx,
  heightPx,
  xRange,
  yRange,
  scene,
  theme,
  names,
  fonts,
  transparent,
  label,
  caption,
  busy,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [box, setBox] = useState({ w: 0, h: 0 });

  useLayoutEffect(() => {
    const el = canvasRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setBox({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || box.w === 0 || box.h === 0 || !xRange || !yRange) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(box.w * dpr);
    const H = Math.round(box.h * dpr);
    if (canvas.width !== W || canvas.height !== H) {
      canvas.width = W;
      canvas.height = H;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, W, H);
    // One scale for both directions: the well has the figure's aspect, so this fills it.
    const k = Math.min(box.w / widthPx, box.h / heightPx) * dpr;
    ctx.setTransform(k, 0, 0, k, 0, 0);
    const vp = makeViewport(widthPx, heightPx, xRange, yRange);
    drawFigure(ctx, widthPx, heightPx, vp, scene, theme, { background: !transparent, names, fonts });
  }, [box, widthPx, heightPx, xRange, yRange, scene, theme, names, fonts, transparent]);

  return (
    <figure className="ds-export-preview">
      <span
        className={"ds-export-preview-well export-preview-well" + (transparent ? " is-transparent" : "")}
        style={{
          ["--ds-export-aspect" as string]: `${widthPx} / ${heightPx}`,
          ["--export-ratio" as string]: String(widthPx / heightPx),
        }}
        aria-busy={busy || undefined}
      >
        <canvas ref={canvasRef} role="img" aria-label={label} />
      </span>
      <figcaption className="ds-export-preview-cap">{caption}</figcaption>
    </figure>
  );
}
