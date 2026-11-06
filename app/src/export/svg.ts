// The SVG export: the figure drawn through the SVG recorder, so it is the
// same scene as the screen and the PNG, with text kept as text.

import type { Scene } from "../canvas/draw";
import { drawAnalysisFigure, drawFigure, type AnalysisFigure, type FigureFonts } from "../canvas/figure";
import { drawSpaceFigure, type SpaceFigure } from "../canvas/space";
import { createSvgContext, type TextMeasure } from "../canvas/svgContext";
import type { PlotTheme } from "../canvas/theme";
import { makeViewport } from "../canvas/viewport";

export interface SvgFigure {
  /** Figure size in CSS px (the viewBox). */
  widthPx: number;
  heightPx: number;
  /** Physical size for the width and height attributes, cm. */
  widthCm?: number;
  heightCm?: number;
  xRange: [number, number];
  yRange: [number, number];
  scene: Scene | null;
  theme: PlotTheme;
  names: { x: string; y: string } | null;
  transparent: boolean;
  fonts?: FigureFonts;
  title?: string;
  desc?: string;
  space?: SpaceFigure | null;
  analysis?: AnalysisFigure | null;
}

const cm = (v: number) => `${Math.round(v * 1000) / 1000}cm`;

export function figureSvg(fig: SvgFigure, measure: TextMeasure): string {
  const ctx = createSvgContext(measure);
  const opts = { background: !fig.transparent, names: fig.names, fonts: fig.fonts };
  if (fig.space) drawSpaceFigure(ctx, fig.widthPx, fig.heightPx, fig.space, fig.theme, !fig.transparent);
  else {
    const vp = makeViewport(fig.widthPx, fig.heightPx, fig.xRange, fig.yRange);
    if (fig.analysis) drawAnalysisFigure(ctx, fig.widthPx, fig.heightPx, vp, fig.analysis, fig.theme, opts);
    else drawFigure(ctx, fig.widthPx, fig.heightPx, vp, fig.scene, fig.theme, opts);
  }
  return ctx.toSvg({
    width: fig.widthPx,
    height: fig.heightPx,
    widthAttr: fig.widthCm ? cm(fig.widthCm) : undefined,
    heightAttr: fig.heightCm ? cm(fig.heightCm) : undefined,
    title: fig.title,
    desc: fig.desc,
  });
}

/**
 * A text measure from a 2D canvas (the browser): the advance width, and the
 * font's ascent and descent (the em box when the browser reports it).
 */
export function canvasMeasure(ctx: CanvasRenderingContext2D): TextMeasure {
  const cache = new Map<string, { ascent: number; descent: number }>();
  return (font, text) => {
    ctx.font = font;
    const m = ctx.measureText(text);
    let v = cache.get(font);
    if (!v) {
      const probe = ctx.measureText("Hg");
      const size = parseFloat(/([\d.]+)px/.exec(font)?.[1] ?? "10");
      v = {
        ascent: probe.fontBoundingBoxAscent ?? probe.actualBoundingBoxAscent ?? size * 0.8,
        descent: probe.fontBoundingBoxDescent ?? probe.actualBoundingBoxDescent ?? size * 0.2,
      };
      cache.set(font, v);
    }
    return { width: m.width, ascent: v.ascent, descent: v.descent };
  };
}
