// A DrawContext that records the canvas calls drawScene makes as SVG, so an
// SVG export is the same drawing as the screen, never a raster.
//
// - Every stroke(), fill() and fillRect() becomes one <path> or <rect>;
//   strokeText() and fillText() one <text> each (text stays text).
// - clip() opens a <g clip-path> that the matching restore() closes.
// - Text is always written on the alphabetic baseline: `textBaseline` is
//   resolved into a y offset from the font's ascent and descent, because
//   `dominant-baseline` is handled inconsistently by Word, PowerPoint and
//   Inkscape.
// - Maths labels (canvas/mathText.ts) come in through `mathText` as one
//   <text> whose sub- and superscripts are <tspan dy font-size="70%">.

import type { DrawContext } from "./draw";
import type { MathLayout, MathTextSink } from "./mathText";

export interface TextMetricsLite {
  width: number;
  /** The font's ascent above the alphabetic baseline, px. */
  ascent: number;
  /** The font's descent below it, px. */
  descent: number;
}

/** Measures `text` in the CSS `font`. */
export type TextMeasure = (font: string, text: string) => TextMetricsLite;

interface GState {
  fillStyle: string;
  strokeStyle: string;
  lineWidth: number;
  lineCap: CanvasLineCap;
  lineJoin: CanvasLineJoin;
  font: string;
  textAlign: CanvasTextAlign;
  textBaseline: CanvasTextBaseline;
  globalAlpha: number;
  dash: number[];
  /** <g clip-path> elements opened at this save level. */
  groups: number;
}

export interface SvgDocumentOptions {
  /** viewBox size in CSS px. */
  width: number;
  height: number;
  /** The `width` and `height` attributes (e.g. "8cm"); the px size by default. */
  widthAttr?: string;
  heightAttr?: string;
  title?: string;
  desc?: string;
}

const TAU = Math.PI * 2;

/** A coordinate or length, rounded to 1/100 px. */
export function num(v: number): string {
  const r = Math.round(v * 100) / 100;
  return Object.is(r, -0) || r === 0 ? "0" : String(r);
}

export function escapeXml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

/** A canvas colour as an SVG paint and its alpha (rgba() is not SVG 1.1). */
export function svgPaint(c: unknown): { color: string; alpha: number } {
  if (typeof c !== "string") return { color: "#000000", alpha: 1 };
  const m = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+%?))?\s*\)$/i.exec(c.trim());
  if (!m) return { color: c.trim(), alpha: 1 };
  const a = m[4] === undefined ? 1 : m[4].endsWith("%") ? parseFloat(m[4]) / 100 : parseFloat(m[4]);
  return { color: `rgb(${m[1]},${m[2]},${m[3]})`, alpha: Number.isFinite(a) ? a : 1 };
}

export interface ParsedFont {
  style: string;
  weight: string;
  size: number;
  family: string;
}

/** The parts of a CSS font shorthand ("italic 400 16px KaTeX_Main, serif"). */
export function parseFont(font: string): ParsedFont {
  const m = /^\s*((?:\S+\s+)*?)([\d.]+)px(?:\/\S+)?\s+(.+?)\s*$/.exec(font);
  if (!m) return { style: "normal", weight: "400", size: 10, family: "sans-serif" };
  let style = "normal";
  let weight = "400";
  for (const w of m[1].trim().split(/\s+/).filter(Boolean)) {
    if (w === "italic" || w === "oblique") style = w;
    else if (/^\d{3}$/.test(w) || w === "bold" || w === "lighter" || w === "bolder") weight = w === "bold" ? "700" : w;
  }
  // Single quotes inside the attribute keep it readable in editors.
  return { style, weight, size: parseFloat(m[2]), family: m[3].replace(/"/g, "'") };
}

function fontAttrs(f: ParsedFont): string {
  let a = ` font-family="${escapeXml(f.family).replace(/&apos;/g, "'")}" font-size="${num(f.size)}"`;
  if (f.weight !== "400") a += ` font-weight="${f.weight}"`;
  if (f.style !== "normal") a += ` font-style="${f.style}"`;
  return a;
}

export class SvgContext implements DrawContext, MathTextSink {
  private st: GState = {
    fillStyle: "#000000",
    strokeStyle: "#000000",
    lineWidth: 1,
    lineCap: "butt",
    lineJoin: "miter",
    font: "10px sans-serif",
    textAlign: "start",
    textBaseline: "alphabetic",
    globalAlpha: 1,
    dash: [],
    groups: 0,
  };
  private stack: GState[] = [];
  private d: string[] = [];
  private cur: [number, number] | null = null;
  private start: [number, number] | null = null;
  private body: string[] = [];
  private defs: string[] = [];
  private clips = 0;
  private readonly measure: TextMeasure;

  constructor(measure: TextMeasure) {
    this.measure = measure;
  }

  get fillStyle(): string {
    return this.st.fillStyle;
  }
  set fillStyle(v: string | CanvasGradient | CanvasPattern) {
    if (typeof v === "string") this.st.fillStyle = v;
  }
  get strokeStyle(): string {
    return this.st.strokeStyle;
  }
  set strokeStyle(v: string | CanvasGradient | CanvasPattern) {
    if (typeof v === "string") this.st.strokeStyle = v;
  }
  get lineWidth(): number {
    return this.st.lineWidth;
  }
  set lineWidth(v: number) {
    if (Number.isFinite(v) && v > 0) this.st.lineWidth = v;
  }
  get lineCap(): CanvasLineCap {
    return this.st.lineCap;
  }
  set lineCap(v: CanvasLineCap) {
    this.st.lineCap = v;
  }
  get lineJoin(): CanvasLineJoin {
    return this.st.lineJoin;
  }
  set lineJoin(v: CanvasLineJoin) {
    this.st.lineJoin = v;
  }
  get font(): string {
    return this.st.font;
  }
  set font(v: string) {
    this.st.font = v;
  }
  get textAlign(): CanvasTextAlign {
    return this.st.textAlign;
  }
  set textAlign(v: CanvasTextAlign) {
    this.st.textAlign = v;
  }
  get textBaseline(): CanvasTextBaseline {
    return this.st.textBaseline;
  }
  set textBaseline(v: CanvasTextBaseline) {
    this.st.textBaseline = v;
  }
  get globalAlpha(): number {
    return this.st.globalAlpha;
  }
  set globalAlpha(v: number) {
    if (Number.isFinite(v) && v >= 0 && v <= 1) this.st.globalAlpha = v;
  }

  save(): void {
    this.stack.push(this.st);
    this.st = { ...this.st, dash: [...this.st.dash], groups: 0 };
  }

  restore(): void {
    const prev = this.stack.pop();
    if (!prev) return;
    for (let i = 0; i < this.st.groups; i++) this.body.push("</g>");
    this.st = prev;
  }

  beginPath(): void {
    this.d = [];
    this.cur = null;
    this.start = null;
  }

  closePath(): void {
    if (!this.cur) return;
    this.d.push("Z");
    this.cur = this.start;
  }

  moveTo(x: number, y: number): void {
    this.d.push(`M${num(x)} ${num(y)}`);
    this.cur = [x, y];
    this.start = [x, y];
  }

  lineTo(x: number, y: number): void {
    if (!this.cur) {
      this.moveTo(x, y);
      return;
    }
    this.d.push(`L${num(x)} ${num(y)}`);
    this.cur = [x, y];
  }

  arc(x: number, y: number, r: number, a0: number, a1: number, ccw = false): void {
    if (!(r >= 0)) return;
    const raw = ccw ? a0 - a1 : a1 - a0;
    const full = raw >= TAU - 1e-12;
    const delta = full ? TAU : ((raw % TAU) + TAU) % TAU;
    const ps: [number, number] = [x + r * Math.cos(a0), y + r * Math.sin(a0)];
    if (!this.cur) {
      this.d.push(`M${num(ps[0])} ${num(ps[1])}`);
      this.start = ps;
    } else if (Math.abs(this.cur[0] - ps[0]) > 1e-9 || Math.abs(this.cur[1] - ps[1]) > 1e-9) {
      this.d.push(`L${num(ps[0])} ${num(ps[1])}`);
    }
    this.cur = ps;
    if (delta === 0 || r === 0) return;
    const sweep = ccw ? 0 : 1;
    const rr = num(r);
    if (full) {
      const mid = a0 + (ccw ? -Math.PI : Math.PI);
      const pm: [number, number] = [x + r * Math.cos(mid), y + r * Math.sin(mid)];
      this.d.push(`A${rr} ${rr} 0 0 ${sweep} ${num(pm[0])} ${num(pm[1])}`);
      this.d.push(`A${rr} ${rr} 0 0 ${sweep} ${num(ps[0])} ${num(ps[1])}`);
      return;
    }
    const end = a0 + (ccw ? -delta : delta);
    const pe: [number, number] = [x + r * Math.cos(end), y + r * Math.sin(end)];
    this.d.push(`A${rr} ${rr} 0 ${delta > Math.PI ? 1 : 0} ${sweep} ${num(pe[0])} ${num(pe[1])}`);
    this.cur = pe;
  }

  rect(x: number, y: number, w: number, h: number): void {
    this.d.push(`M${num(x)} ${num(y)}h${num(w)}v${num(h)}h${num(-w)}Z`);
    this.cur = [x, y];
    this.start = [x, y];
  }

  clip(): void {
    const id = `sepx-clip-${++this.clips}`;
    this.defs.push(`<clipPath id="${id}"><path d="${this.d.join("")}"/></clipPath>`);
    this.body.push(`<g clip-path="url(#${id})">`);
    this.st.groups++;
  }

  private opacity(attr: string, alpha: number): string {
    return alpha < 1 ? ` ${attr}="${num(alpha)}"` : "";
  }

  fill(): void {
    if (!this.d.length) return;
    const p = svgPaint(this.st.fillStyle);
    const a = p.alpha * this.st.globalAlpha;
    if (a <= 0) return;
    this.body.push(`<path d="${this.d.join("")}" fill="${escapeXml(p.color)}"${this.opacity("fill-opacity", a)}/>`);
  }

  private strokeAttrs(): string {
    const p = svgPaint(this.st.strokeStyle);
    const a = p.alpha * this.st.globalAlpha;
    let s = ` stroke="${escapeXml(p.color)}" stroke-width="${num(this.st.lineWidth)}"`;
    if (this.st.lineCap !== "butt") s += ` stroke-linecap="${this.st.lineCap}"`;
    // Canvas's miter limit is 10, SVG's default 4.
    s += this.st.lineJoin !== "miter" ? ` stroke-linejoin="${this.st.lineJoin}"` : ` stroke-miterlimit="10"`;
    if (this.st.dash.length) s += ` stroke-dasharray="${this.st.dash.map(num).join(" ")}"`;
    return s + this.opacity("stroke-opacity", a);
  }

  stroke(): void {
    if (!this.d.length) return;
    if (svgPaint(this.st.strokeStyle).alpha * this.st.globalAlpha <= 0) return;
    this.body.push(`<path d="${this.d.join("")}" fill="none"${this.strokeAttrs()}/>`);
  }

  fillRect(x: number, y: number, w: number, h: number): void {
    const p = svgPaint(this.st.fillStyle);
    const a = p.alpha * this.st.globalAlpha;
    if (a <= 0) return;
    this.body.push(
      `<rect x="${num(x)}" y="${num(y)}" width="${num(w)}" height="${num(h)}" fill="${escapeXml(p.color)}"${this.opacity("fill-opacity", a)}/>`,
    );
  }

  setLineDash(segments: number[]): void {
    const s = [...segments].filter((v) => Number.isFinite(v) && v >= 0);
    // An odd list repeats, as canvas does.
    this.st.dash = s.length % 2 ? [...s, ...s] : s;
  }

  measureText(text: string): { width: number } {
    return { width: this.measure(this.st.font, text).width };
  }

  /** The y of the alphabetic baseline for text drawn at `y` with the current `textBaseline`. */
  private baselineY(text: string, y: number): number {
    const m = this.measure(this.st.font, text);
    switch (this.st.textBaseline) {
      case "top":
      case "hanging":
        return y + m.ascent;
      case "middle":
        return y + (m.ascent - m.descent) / 2;
      case "bottom":
      case "ideographic":
        return y - m.descent;
      default:
        return y;
    }
  }

  private anchor(): string {
    const a = this.st.textAlign;
    if (a === "center") return ` text-anchor="middle"`;
    if (a === "right" || a === "end") return ` text-anchor="end"`;
    return "";
  }

  private text(text: string, x: number, y: number, mode: "fill" | "stroke"): void {
    const f = parseFont(this.st.font);
    const paint = mode === "fill" ? this.fillAttrs() : ` fill="none"${this.strokeAttrs()}`;
    if (!paint) return;
    this.body.push(
      `<text x="${num(x)}" y="${num(this.baselineY(text, y))}"${fontAttrs(f)}${this.anchor()}${paint}>${escapeXml(text)}</text>`,
    );
  }

  private fillAttrs(): string {
    const p = svgPaint(this.st.fillStyle);
    const a = p.alpha * this.st.globalAlpha;
    return a <= 0 ? "" : ` fill="${escapeXml(p.color)}"${this.opacity("fill-opacity", a)}`;
  }

  fillText(text: string, x: number, y: number): void {
    this.text(text, x, y, "fill");
  }

  strokeText(text: string, x: number, y: number): void {
    this.text(text, x, y, "stroke");
  }

  mathText(layout: MathLayout, x: number, baseline: number, mode: "fill" | "stroke"): void {
    const paint = mode === "fill" ? this.fillAttrs() : ` fill="none"${this.strokeAttrs()}`;
    if (!paint || !layout.pieces.length) return;
    const family = escapeXml(layout.family.replace(/"/g, "'")).replace(/&apos;/g, "'");
    let dy = 0;
    const spans = layout.pieces.map((p) => {
      const shift = p.dy - dy;
      dy = p.dy;
      let a = "";
      if (shift !== 0) a += ` dy="${num(shift)}"`;
      if (p.script !== "base") a += ` font-size="70%"`;
      a += ` font-style="${p.italic ? "italic" : "normal"}"`;
      return `<tspan${a}>${escapeXml(p.text)}</tspan>`;
    });
    this.body.push(
      `<text x="${num(x)}" y="${num(baseline)}" font-family="${family}" font-size="${num(layout.size)}"${paint}>${spans.join("")}</text>`,
    );
  }

  imageSmoothingEnabled = true;

  drawImage(image: CanvasImageSource, dx: number, dy: number, dw: number, dh: number): void {
    if (!(typeof HTMLCanvasElement !== "undefined" && image instanceof HTMLCanvasElement)) return;
    const a = this.st.globalAlpha;
    const rendering = this.imageSmoothingEnabled ? "" : ` style="image-rendering:pixelated"`;
    this.body.push(
      `<image x="${num(dx)}" y="${num(dy)}" width="${num(dw)}" height="${num(dh)}" preserveAspectRatio="none"${rendering}${this.opacity("opacity", a)} href="${image.toDataURL("image/png")}"/>`,
    );
  }

  /** The recorded drawing as a standalone SVG document. */
  toSvg(opts: SvgDocumentOptions): string {
    let open = this.st.groups;
    for (const s of this.stack) open += s.groups;
    const close = Array.from({ length: open }, () => "</g>");
    const w = opts.widthAttr ?? num(opts.width);
    const h = opts.heightAttr ?? num(opts.height);
    return [
      `<?xml version="1.0" encoding="UTF-8"?>`,
      `<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="${escapeXml(w)}" height="${escapeXml(h)}" viewBox="0 0 ${num(opts.width)} ${num(opts.height)}">`,
      ...(opts.title ? [`<title>${escapeXml(opts.title)}</title>`] : []),
      ...(opts.desc ? [`<desc>${escapeXml(opts.desc)}</desc>`] : []),
      ...(this.defs.length ? ["<defs>", ...this.defs, "</defs>"] : []),
      ...this.body,
      ...close,
      "</svg>",
      "",
    ].join("\n");
  }
}

export function createSvgContext(measure: TextMeasure): SvgContext {
  return new SvgContext(measure);
}
