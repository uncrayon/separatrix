// An exported figure: the screen's scene (drawScene) plus the axis names,
// which the screen typesets as HTML over the canvas. The same code draws the
// export preview, the PNG and the SVG, through any DrawContext.

import type { LyapunovResponse, OrbitDiagramResponse } from "../api/types";
import { axisLines, drawScene, type Box, type DrawContext, type Scene } from "./draw";
import { drawLyapunov } from "./lyapunov";
import { drawMath, layoutMath, mathRuns, type MathLayout } from "./mathText";
import { drawOrbit, type RasterContext } from "./orbit";
import type { PlotTheme } from "./theme";
import type { Viewport } from "./viewport";

export interface AxisNamePlacement {
  /** Right edge of the x name, px. */
  xRight: number;
  /** The x name sits above the x axis (else below it). */
  xAbove: boolean;
  /** The x axis's pixel row. */
  xAxisY: number;
  /** The y name's anchor: its left edge when `yRight`, else its right edge. */
  yX: number;
  /** The y name is right of the y axis (else left of it). */
  yRight: boolean;
  /** Top of the y name, px. */
  yTop: number;
}

/** Gap between the x name and the x axis line. */
const X_NAME_GAP = 4;

/**
 * Where the screen puts the axis names (PortraitCanvas, without its toolbar
 * avoidance), and where `separatrix_export::latex_figure::axis_names` puts
 * them in a LaTeX figure: the x name right-aligned 8 px in from the right
 * edge, above the x axis when that lies more than 40 px below the top; the
 * y name right of the y axis when there are more than 48 px of room.
 */
export function axisNamePlacement(vp: Viewport, axes: { X: number; Y: number }): AxisNamePlacement {
  const right = vp.left + vp.width;
  const yRight = right - axes.X > 48;
  return {
    xRight: right - 8,
    xAbove: axes.Y - vp.top > 40,
    xAxisY: axes.Y,
    yX: yRight ? axes.X + 8 : axes.X - 8,
    yRight,
    yTop: vp.top + 6,
  };
}

/** The boxes of the two names, given their sizes. */
export function axisNameBoxes(
  p: AxisNamePlacement,
  xSize: [number, number],
  ySize: [number, number],
): { x: Box; y: Box } {
  const [xw, xh] = xSize;
  const [yw, yh] = ySize;
  const xTop = p.xAbove ? p.xAxisY - X_NAME_GAP - xh : p.xAxisY + X_NAME_GAP;
  const yLeft = p.yRight ? p.yX : p.yX - yw;
  return {
    x: [p.xRight - xw, xTop, p.xRight, xTop + xh],
    y: [yLeft, p.yTop, yLeft + yw, p.yTop + yh],
  };
}

export interface FigureFonts {
  /** CSS font family of the maths labels. */
  mathFamily: string;
  /** Font size of the axis names, px (the `math` type token: 16). */
  mathSize: number;
}

export const DEFAULT_FIGURE_FONTS: FigureFonts = {
  mathFamily: '"KaTeX_Main", "Latin Modern Math", "STIX Two Math", "Cambria Math", "Times New Roman", serif',
  mathSize: 16,
};

export interface FigureOptions {
  /** Fill the background with the plot colour (off: transparent). */
  background: boolean;
  /** The axis names in LaTeX (as `axisLabelLatex` gives them); null leaves them out. */
  names: { x: string; y: string } | null;
  fonts?: FigureFonts;
}

/** Height of a name's box per px of font size (the screen's 16 px name is about 20 px tall). */
const NAME_BOX_EM = 1.25;
/** Baseline of a name below the top of its box, per px of font size. */
const NAME_BASELINE_EM = 0.95;

function nameBox(layout: MathLayout): [number, number] {
  return [layout.width, layout.size * NAME_BOX_EM];
}

/**
 * Draws the figure: the scene (no selection, hover, handles or fade: the
 * caller's scene says so) and the axis names, which the field arrows and
 * tick labels keep clear of, as on screen.
 */
export function drawFigure(
  ctx: DrawContext,
  w: number,
  h: number,
  vp: Viewport,
  scene: Scene | null,
  th: PlotTheme,
  opts: FigureOptions,
): void {
  const names = placeNames(ctx, vp, opts);
  const boxes = names.map((n) => n.box);
  const withAvoid: Scene | null = scene && {
    ...scene,
    avoid: {
      labels: [...scene.avoid.labels, ...boxes],
      field: [...scene.avoid.field, ...boxes.map(([l, t, r, b]): Box => [l - 3, t - 3, r + 3, b + 3])],
    },
  };
  drawScene(ctx, w, h, vp, withAvoid, th, { background: opts.background });
  drawNames(ctx, names, th);
}

/** An axis name laid out, and the box it takes. */
interface PlacedName {
  layout: MathLayout;
  box: Box;
}

/** The axis names where the screen puts them (none without `opts.names`). */
function placeNames(ctx: DrawContext, vp: Viewport, opts: FigureOptions): PlacedName[] {
  if (!opts.names) return [];
  const fonts = opts.fonts ?? DEFAULT_FIGURE_FONTS;
  const xl = layoutMath(ctx, mathRuns(opts.names.x), fonts.mathFamily, fonts.mathSize);
  const yl = layoutMath(ctx, mathRuns(opts.names.y), fonts.mathFamily, fonts.mathSize);
  const boxes = axisNameBoxes(axisNamePlacement(vp, axisLines(vp)), nameBox(xl), nameBox(yl));
  return [
    { layout: xl, box: boxes.x },
    { layout: yl, box: boxes.y },
  ];
}

function drawNames(ctx: DrawContext, names: PlacedName[], th: PlotTheme): void {
  ctx.globalAlpha = 1;
  for (const n of names) {
    drawMath(ctx, n.layout, n.box[0], n.box[1] + n.layout.size * NAME_BASELINE_EM, th.tick, th.halo);
  }
}

/**
 * What a Lyapunov or orbit-diagram view exports: the view's run, or a
 * diagram computed with one column per device pixel of the image
 * (`pixelRatio` image pixels per CSS px).
 */
export type AnalysisFigure =
  | { kind: "lyapunov"; res: LyapunovResponse }
  | { kind: "orbit"; res: Pick<OrbitDiagramResponse, "r" | "x">; pixelRatio: number };

/**
 * Draws an analysis view's figure as its canvas draws the screen
 * (drawLyapunov, drawOrbit, without the orbit view's parameter marker), with
 * the axis names that canvas typesets over it. The tick labels keep clear of
 * the names.
 */
export function drawAnalysisFigure(
  ctx: RasterContext,
  w: number,
  h: number,
  vp: Viewport,
  fig: AnalysisFigure,
  th: PlotTheme,
  opts: FigureOptions,
): void {
  ctx.globalAlpha = 1;
  if (opts.background) {
    ctx.fillStyle = th.bg;
    ctx.fillRect(0, 0, w, h);
  }
  const names = placeNames(ctx, vp, opts);
  const avoid = names.map((n) => n.box);
  if (fig.kind === "lyapunov") drawLyapunov(ctx, vp, fig.res, th, opts.fonts ?? DEFAULT_FIGURE_FONTS, avoid);
  else drawOrbit(ctx, vp, fig.res, th, { pixelRatio: fig.pixelRatio, marker: null, avoid });
  drawNames(ctx, names, th);
}
