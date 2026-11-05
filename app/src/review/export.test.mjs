// Exports drawn in TypeScript: the DrawContext abstraction (draw.ts), the SVG
// recorder, maths labels, axis names, the PNG's pHYs and scale handling, the
// export options and job, and the heads.json parity vectors that the Rust
// export crate checks its `heads::place` against.
//
// Node (>= 22.18) strips the types from the .ts modules. The app's modules
// import each other without extensions (for Vite and tsc), so this file
// resolves those to .ts with a module hook before loading them.
// Regenerate heads.json with UPDATE_VECTORS=1.

import test from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { deflateSync, crc32 as zcrc32 } from "node:zlib";
import tokens from "../styles/ds/tokens.json" with { type: "json" };

registerHooks({
  resolve(specifier, context, next) {
    try {
      return next(specifier, context);
    } catch (err) {
      if (/^\.\.?\//.test(specifier) && !/\.[cm]?[jt]sx?$/.test(specifier)) return next(`${specifier}.ts`, context);
      throw err;
    }
  },
});

const draw = await import("../canvas/draw.ts");
const { SvgContext, parseFont, svgPaint, num } = await import("../canvas/svgContext.ts");
const { mathRuns, layoutMath, drawMath } = await import("../canvas/mathText.ts");
const { axisNamePlacement, axisNameBoxes, drawFigure } = await import("../canvas/figure.ts");
const { makeViewport } = await import("../canvas/viewport.ts");
const { plotThemeFromTokens } = await import("../export/theme.ts");
const { readExportOptions, DEFAULT_EXPORT_OPTIONS, sizeError, fileStem, figurePx } = await import("../export/options.ts");
const { pngRaster, withPhys, pngChunks, physDpi, crc32, pixelsPerMetre, PNG_MAX_PIXELS } = await import("../export/png.ts");
const { buildFigureJob, buildCsvRequest, exportSamples, exportScene, icProjectId } = await import("../export/job.ts");
const { figureSvg } = await import("../export/svg.ts");
const { DEFAULT_LAYERS, DEFAULT_SOLVER } = await import("../state/model.ts");

const light = plotThemeFromTokens(tokens, "light");
const dark = plotThemeFromTokens(tokens, "dark");

// --- helpers -----------------------------------------------------------------

/** Font size in px from a CSS font. */
const pxOf = (font) => parseFloat(/([\d.]+)px/.exec(font)?.[1] ?? "10");
const measure = (font, text) => {
  const s = pxOf(font);
  return { width: 0.6 * s * [...text].length, ascent: 0.8 * s, descent: 0.2 * s };
};

/** A DrawContext that logs every call, as a canvas would receive them. */
function recorder() {
  const log = [];
  let pathOps = 0;
  const ctx = {
    globalAlpha: 1,
    fillStyle: "#000",
    strokeStyle: "#000",
    lineWidth: 1,
    lineCap: "butt",
    lineJoin: "miter",
    font: "10px sans-serif",
    textAlign: "start",
    textBaseline: "alphabetic",
    save: () => log.push(["save"]),
    restore: () => log.push(["restore"]),
    beginPath: () => {
      pathOps = 0;
      log.push(["beginPath"]);
    },
    closePath: () => log.push(["closePath"]),
    moveTo: (x, y) => {
      pathOps++;
      log.push(["moveTo", x, y]);
    },
    lineTo: (x, y) => {
      pathOps++;
      log.push(["lineTo", x, y]);
    },
    arc: (...a) => {
      pathOps++;
      log.push(["arc", ...a]);
    },
    rect: (...a) => {
      pathOps++;
      log.push(["rect", ...a]);
    },
    clip: () => log.push(["clip"]),
    fill: () => log.push(["fill", pathOps > 0, ctx.fillStyle, ctx.globalAlpha]),
    stroke: () => log.push(["stroke", pathOps > 0, ctx.strokeStyle, ctx.globalAlpha]),
    fillRect: (...a) => log.push(["fillRect", ...a]),
    setLineDash: (d) => log.push(["setLineDash", [...d]]),
    fillText: (t, x, y) => log.push(["fillText", t, x, y, ctx.textBaseline]),
    strokeText: (t, x, y) => log.push(["strokeText", t, x, y, ctx.textBaseline]),
    measureText: (t) => ({ width: measure(ctx.font, t).width }),
  };
  return { ctx, log };
}

/** A small well-formedness check: balanced tags, quoted attributes, known entities. */
function assertWellFormedXml(xml) {
  let s = xml.replace(/^<\?xml[^?]*\?>\s*/, "");
  const stack = [];
  const re = /<(\/?)([A-Za-z][\w:-]*)((?:\s+[\w:-]+="[^"<]*")*)\s*(\/?)>|([^<]+)|(<)/g;
  let m;
  let roots = 0;
  while ((m = re.exec(s))) {
    if (m[6]) assert.fail(`stray < at ${m.index}: ${s.slice(m.index, m.index + 40)}`);
    if (m[5] !== undefined) {
      const text = m[5];
      assert.ok(!/&(?!(amp|lt|gt|quot|apos|#\d+|#x[0-9a-f]+);)/i.test(text), `bad entity in ${JSON.stringify(text)}`);
      if (stack.length === 0) assert.match(text, /^\s*$/, "text outside the root");
      continue;
    }
    const [, close, name, attrs, self] = m;
    for (const a of attrs.matchAll(/="([^"]*)"/g)) {
      assert.ok(!/&(?!(amp|lt|gt|quot|apos|#\d+);)/.test(a[1]), `bad entity in attribute ${a[1]}`);
    }
    const names = [...attrs.matchAll(/([\w:-]+)=/g)].map((x) => x[1]);
    assert.equal(new Set(names).size, names.length, `duplicate attribute on <${name}>`);
    if (close) {
      assert.equal(stack.pop(), name, `</${name}> closes the wrong element`);
    } else if (!self) {
      if (stack.length === 0) roots++;
      stack.push(name);
    } else if (stack.length === 0) roots++;
  }
  assert.equal(stack.length, 0, `unclosed: ${stack.join(", ")}`);
  assert.equal(roots, 1, "one root element");
}

const fp = (x, classification, stable) => ({
  x,
  eigenvalues: [
    [-1, 0],
    [-2, 0],
  ],
  eigenvectors: [
    [1, 0],
    [0, 1],
  ],
  eigenvectorsIm: [null, null],
  classification,
  stable,
});

function spiral(n, cx, cy, r0, turns) {
  const t = [];
  const x = [];
  for (let k = 0; k < n; k++) {
    const a = (k / (n - 1)) * turns * 2 * Math.PI;
    const r = r0 * (1 - (0.8 * k) / n);
    t.push(k * 0.01);
    x.push(cx + r * Math.cos(a), cy + r * Math.sin(a));
  }
  return { t, x, status: "completed" };
}

/** A scene with every kind of mark: field, nullclines, both runs, an unfinished run, all three glyphs. */
function sampleScene() {
  const arrows = [];
  for (let i = 0; i < 6; i++)
    for (let j = 0; j < 5; j++) arrows.push({ x: -3 + i * 1.2, y: -2 + j, u: 1 + j * 0.1, v: i - 2.5 });
  const unfinished = spiral(200, 1, 0.5, 1.5, 1.2);
  unfinished.status = "blew-up";
  return {
    kind: "ode",
    dim: 2,
    xAxis: { kind: "state", index: 0 },
    yAxis: { kind: "state", index: 1 },
    arrows,
    fieldGrid: 15,
    nullclines: [
      { state: 0, polylines: [[[-4, -1], [0, 0.2], [4, 1.5]]] },
      { state: 1, polylines: [[[-1, -3], [-0.5, 3]], [[2, -3], [2.2, 3]]] },
    ],
    fixedPoints: [fp([0, 0], "saddle", false), fp([1, 1], "stable node", true), fp([-2, 1.5], "center", false)],
    trajectories: [
      { out: spiral(400, 0, 0, 2.5, 2), meta: { icId: 1, direction: "forward" }, color: 0 },
      { out: spiral(300, 0.5, -0.5, 1.8, 1.5), meta: { icId: 1, direction: "backward" }, color: 0 },
      { out: unfinished, meta: { icId: 2, direction: "forward" }, color: 8 },
    ],
    ics: [],
    selectedIc: null,
    selectedFp: null,
    hoverIc: null,
    hoverFp: null,
    eigenvectors: false,
    stale: false,
    avoid: { labels: [], field: [] },
  };
}

const W = 480;
const H = 360;
const vp = () => makeViewport(W, H, [-4, 4], [-3, 3]);

function countSvg(svg) {
  return {
    strokes: (svg.match(/<path [^>]*fill="none"/g) ?? []).length,
    fills: (svg.match(/<path (?![^>]*fill="none")[^>]*fill="/g) ?? []).length,
    rects: (svg.match(/<rect /g) ?? []).length,
    texts: (svg.match(/<text /g) ?? []).length,
  };
}

// --- draw.ts -------------------------------------------------------------------

test("draw.ts imports in node and exports the DrawContext pieces", () => {
  for (const name of ["drawScene", "drawHeads", "axisLines", "trajectoryPx", "drawFixedPoint", "fixedPointGlyph"]) {
    assert.equal(typeof draw[name], "function", name);
  }
});

test("drawScene draws no background when asked not to, and fills it by default", () => {
  const a = recorder();
  draw.drawScene(a.ctx, W, H, vp(), sampleScene(), light);
  assert.deepEqual(a.log.find((c) => c[0] === "fillRect"), ["fillRect", 0, 0, W, H]);
  const b = recorder();
  draw.drawScene(b.ctx, W, H, vp(), sampleScene(), light, { background: false });
  assert.equal(b.log.filter((c) => c[0] === "fillRect").length, 0);
});

test("eigenvector lines are traced twice into the context (halo, then line), with no Path2D", () => {
  const scene = { ...sampleScene(), eigenvectors: true, trajectories: [], arrows: [], nullclines: [] };
  const { ctx, log } = recorder();
  draw.drawScene(ctx, W, H, vp(), scene, light);
  const strokes = log.filter((c) => c[0] === "stroke" && (c[2] === light.halo || c[2] === light.fp));
  assert.ok(strokes.length >= 2, "halo and line strokes");
  const halo = log.findIndex((c) => c[0] === "stroke" && c[2] === light.halo);
  const line = log.findIndex((c, i) => i > halo && c[0] === "stroke" && c[2] === light.fp);
  assert.ok(halo >= 0 && line > halo, "the line follows its halo");
  const segs = (from, to) => log.slice(from, to).filter((c) => c[0] === "moveTo").length;
  const begin1 = log.lastIndexOf(log.slice(0, halo).findLast((c) => c[0] === "beginPath"));
  const begin2 = log.lastIndexOf(log.slice(0, line).findLast((c) => c[0] === "beginPath"));
  assert.equal(segs(begin1, halo), segs(begin2, line), "the same path both times");
});

// --- SVG recorder ----------------------------------------------------------------

test("SVG: the recorder writes exactly the primitives the canvas receives", () => {
  const rec = recorder();
  draw.drawScene(rec.ctx, W, H, vp(), sampleScene(), light);
  const svgCtx = new SvgContext(measure);
  draw.drawScene(svgCtx, W, H, vp(), sampleScene(), light);
  const svg = svgCtx.toSvg({ width: W, height: H });
  const c = countSvg(svg);
  const drawn = (op) => rec.log.filter((x) => x[0] === op && x[1] && x[3] > 0).length;
  assert.equal(c.strokes, drawn("stroke"), "strokes");
  assert.equal(c.fills, drawn("fill"), "fills");
  assert.equal(c.rects, rec.log.filter((x) => x[0] === "fillRect").length, "rects");
  assert.equal(c.texts, rec.log.filter((x) => x[0] === "fillText" || x[0] === "strokeText").length, "texts");
  assert.ok(c.strokes > 5 && c.fills > 5 && c.texts > 4, "a real drawing");
  // The same colours, in the same order.
  const svgColors = [...svg.matchAll(/<path [^>]*?(?:fill="none"[^>]*stroke="([^"]+)"|fill="([^"]+)")/g)].map((m) => m[1] ?? m[2]);
  const canvasColors = rec.log.filter((x) => (x[0] === "stroke" || x[0] === "fill") && x[1] && x[3] > 0).map((x) => x[2]);
  assert.deepEqual(svgColors, canvasColors);
});

test("SVG: valid XML, one clip group closed, text escaped", () => {
  const ctx = new SvgContext(measure);
  drawFigure(ctx, W, H, vp(), sampleScene(), light, { background: true, names: { x: "x_{1}", y: "\\dot{y}" } });
  ctx.font = "400 11px 'A & B', monospace";
  ctx.fillStyle = "#000";
  ctx.fillText("a < b & \"c\"", 10, 10);
  const svg = ctx.toSvg({ width: W, height: H, widthAttr: "8cm", heightAttr: "6cm", title: "Rabbits & sheep" });
  assertWellFormedXml(svg);
  assert.match(svg, /^<\?xml version="1.0" encoding="UTF-8"\?>\n<svg xmlns="http:\/\/www.w3.org\/2000\/svg"/);
  assert.match(svg, /width="8cm" height="6cm" viewBox="0 0 480 360"/);
  assert.match(svg, /<title>Rabbits &amp; sheep<\/title>/);
  assert.match(svg, /a &lt; b &amp; &quot;c&quot;/);
  assert.equal((svg.match(/<clipPath /g) ?? []).length, 1);
  assert.equal((svg.match(/<g /g) ?? []).length, (svg.match(/<\/g>/g) ?? []).length);
});

test("SVG: maths labels are <tspan dy> runs, never baseline-shift or dominant-baseline", () => {
  const ctx = new SvgContext(measure);
  drawFigure(ctx, W, H, vp(), sampleScene(), light, { background: true, names: { x: "x_{1,n}", y: "\\theta^{2}" } });
  const svg = ctx.toSvg({ width: W, height: H });
  assert.doesNotMatch(svg, /baseline-shift|dominant-baseline|alignment-baseline/);
  const sub = /<text [^>]*><tspan font-style="italic">x<\/tspan><tspan dy="([\d.]+)" font-size="70%" font-style="normal">1,<\/tspan><tspan font-size="70%" font-style="italic">n<\/tspan><\/text>/.exec(svg);
  assert.ok(sub, "x with the subscript 1,n");
  assert.equal(Number(sub[1]), 4, "a subscript drops a quarter of 16 px");
  const sup = /<tspan font-style="italic">θ<\/tspan><tspan dy="(-[\d.]+)" font-size="70%" font-style="normal">2<\/tspan>/.exec(svg);
  assert.ok(sup, "θ with a superscript");
  // Two copies of each name: the halo stroke, then the fill.
  assert.equal((svg.match(/<text [^>]*fill="none"[^>]*><tspan/g) ?? []).length, 2);
});

test("SVG: a run that returns to the baseline resets dy", () => {
  const ctx = new SvgContext(measure);
  const layout = layoutMath(ctx, mathRuns("x_{1}y"), "serif", 20);
  ctx.mathText(layout, 0, 20, "fill");
  const svg = ctx.toSvg({ width: 10, height: 10 });
  assert.match(svg, /<tspan dy="5" font-size="70%" font-style="normal">1<\/tspan><tspan dy="-5" font-style="italic">y<\/tspan>/);
});

test("SVG: text sits on the alphabetic baseline, resolved from textBaseline", () => {
  const ctx = new SvgContext(measure);
  ctx.font = "400 10px mono";
  ctx.fillStyle = "#123456";
  const ys = {};
  for (const b of ["top", "middle", "bottom", "alphabetic"]) {
    ctx.textBaseline = b;
    ctx.fillText(b, 0, 100);
  }
  for (const m of ctx.toSvg({ width: 1, height: 1 }).matchAll(/<text x="0" y="([\d.]+)"[^>]*>(\w+)</g)) ys[m[2]] = Number(m[1]);
  assert.deepEqual(ys, { top: 108, middle: 103, bottom: 98, alphabetic: 100 });
});

test("SVG: arcs become SVG arcs: full circles in two halves, half discs closed", () => {
  const ctx = new SvgContext(measure);
  ctx.beginPath();
  ctx.arc(10, 10, 5, 0, Math.PI * 2);
  ctx.fill();
  ctx.beginPath();
  ctx.arc(10, 10, 5, Math.PI / 2, (3 * Math.PI) / 2);
  ctx.closePath();
  ctx.fill();
  ctx.beginPath();
  ctx.moveTo(20, 10);
  ctx.arc(15, 10, 5, 0, Math.PI * 2);
  ctx.stroke();
  const svg = ctx.toSvg({ width: 30, height: 20 });
  assert.match(svg, /d="M15 10A5 5 0 0 1 5 10A5 5 0 0 1 15 10"/);
  assert.match(svg, /d="M10 15A5 5 0 0 1 10 5Z"/);
  assert.match(svg, /d="M20 10A5 5 0 0 1 10 10A5 5 0 0 1 20 10"/, "no extra segment when the arc starts at the pen");
});

test("SVG: dashes, caps, joins, alpha and colours map to SVG attributes", () => {
  const ctx = new SvgContext(measure);
  ctx.strokeStyle = "rgba(10, 20, 30, 0.5)";
  ctx.globalAlpha = 0.5;
  ctx.lineWidth = 1.5;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.setLineDash([6, 5]);
  ctx.beginPath();
  ctx.moveTo(0, 0);
  ctx.lineTo(10, -0.004);
  ctx.stroke();
  const svg = ctx.toSvg({ width: 10, height: 10 });
  assert.match(
    svg,
    /<path d="M0 0L10 0" fill="none" stroke="rgb\(10,20,30\)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="6 5" stroke-opacity="0.25"\/>/,
  );
  assert.deepEqual(svgPaint("#abc"), { color: "#abc", alpha: 1 });
  assert.equal(num(-0.001), "0");
  assert.deepEqual(parseFont('italic 400 16px "KaTeX_Main", serif'), {
    style: "italic",
    weight: "400",
    size: 16,
    family: "'KaTeX_Main', serif",
  });
});

test("SVG: a figure from figureSvg has the theme's background and no screen-only marks", () => {
  const svg = figureSvg(
    {
      widthPx: W,
      heightPx: H,
      widthCm: 12.7,
      heightCm: 9.525,
      xRange: [-4, 4],
      yRange: [-3, 3],
      scene: sampleScene(),
      theme: dark,
      names: { x: "x", y: "y" },
      transparent: false,
    },
    measure,
  );
  assertWellFormedXml(svg);
  assert.match(svg, new RegExp(`<rect x="0" y="0" width="480" height="360" fill="${dark.bg}"/>`));
  assert.match(svg, /width="12.7cm" height="9.525cm"/);
  assert.ok(!svg.includes(dark.accent), "no selection ring");
  const clear = figureSvg({ ...{ widthPx: W, heightPx: H, xRange: [-4, 4], yRange: [-3, 3] }, scene: sampleScene(), theme: dark, names: null, transparent: true }, measure);
  assert.doesNotMatch(clear, /<rect /, "transparent: no background");
});

// --- maths labels and axis names --------------------------------------------------

test("mathRuns: names become base, sub and superscript runs", () => {
  assert.deepEqual(mathRuns("x_{1}"), [
    { text: "x", script: "base", italic: true },
    { text: "1", script: "sub", italic: false },
  ]);
  assert.deepEqual(mathRuns("\\dot{x}"), [{ text: "ẋ", script: "base", italic: true }]);
  assert.deepEqual(mathRuns("\\ddot{\\theta}"), [{ text: "θ̈", script: "base", italic: true }]);
  assert.deepEqual(mathRuns("\\Omega"), [{ text: "Ω", script: "base", italic: false }]);
  assert.deepEqual(mathRuns("x_{1,n}"), [
    { text: "x", script: "base", italic: true },
    { text: "1,", script: "sub", italic: false },
    { text: "n", script: "sub", italic: true },
  ]);
  assert.deepEqual(mathRuns("x_{1}^{(4)}").map((r) => [r.text, r.script]), [
    ["x", "base"],
    ["1", "sub"],
    ["(4)", "sup"],
  ]);
  assert.deepEqual(mathRuns("\\theta'").map((r) => r.text), ["θ", "′"]);
  assert.deepEqual(mathRuns("\\mathrm{pop}"), [{ text: "pop", script: "base", italic: false }]);
});

test("drawMath on a canvas draws each run at its offset, halo first", () => {
  const { ctx, log } = recorder();
  const layout = layoutMath(ctx, mathRuns("x_{1}"), "serif", 20);
  drawMath(ctx, layout, 100, 50, "#111", "#fff");
  const texts = log.filter((c) => c[0] === "strokeText" || c[0] === "fillText");
  assert.deepEqual(
    texts.map((c) => [c[0], c[1], c[3]]),
    [
      ["strokeText", "x", 50],
      ["strokeText", "1", 55],
      ["fillText", "x", 50],
      ["fillText", "1", 55],
    ],
  );
  assert.ok(texts[1][2] > texts[0][2], "the subscript follows the base");
});

test("axisNamePlacement: the screen's rule, as the LaTeX figure ports it", () => {
  const v = makeViewport(400, 300, [-1, 3], [-1, 2]);
  const axes = draw.axisLines(v);
  const p = axisNamePlacement(v, axes);
  assert.equal(p.xRight, 392);
  assert.equal(p.xAbove, axes.Y - v.top > 40);
  assert.equal(p.yRight, 400 - axes.X > 48);
  assert.equal(p.yX, axes.X + 8);
  assert.equal(p.yTop, 6);
  const near = axisNamePlacement(v, { X: 380, Y: 20 });
  assert.equal(near.xAbove, false, "x axis near the top: name below it");
  assert.equal(near.yRight, false, "y axis near the right edge: name left of it");
  const b = axisNameBoxes(near, [10, 20], [12, 20]);
  assert.deepEqual(b.x, [382, 24, 392, 44]);
  assert.deepEqual(b.y, [360, 6, 372, 26]);
});

test("axisLines: an axis exactly on the edge counts as on the left/bottom edge", () => {
  const v = makeViewport(400, 300, [0, 3], [0, 3]);
  const a = draw.axisLines(v);
  assert.equal(a.xAxisInView, false);
  assert.equal(a.yAxisInView, false);
  assert.equal(a.X, v.left, "y axis on the left edge");
  assert.equal(a.Y, v.top + v.height, "x axis on the bottom edge");
  const p = axisNamePlacement(v, a);
  assert.equal(p.yRight, true, "the y name sits right of the y axis, away from the x name");
});

test("drawFigure keeps tick labels and field arrows clear of the axis names", () => {
  const arrows = [];
  for (let x = -4; x <= 4; x += 0.25) for (let y = -3; y <= 3; y += 0.25) arrows.push({ x, y, u: 1, v: 0.3 });
  const scene = { ...sampleScene(), arrows };
  const plain = recorder();
  draw.drawScene(plain.ctx, W, H, vp(), scene, light);
  const named = recorder();
  drawFigure(named.ctx, W, H, vp(), scene, light, { background: true, names: { x: "x", y: "y" } });
  const count = (log) => log.filter((c) => c[0] === "lineTo").length;
  assert.ok(count(named.log) < count(plain.log), "some arrows give way to the names");
  assert.ok(named.log.some((c) => c[0] === "fillText" && c[1] === "x"));
});

// --- theme -----------------------------------------------------------------------

test("plotThemeFromTokens: either palette from tokens.json, line and glyph scale 1, 11 px ticks", () => {
  assert.equal(light.bg, "#ffffff");
  assert.equal(dark.bg, "#0a0e10");
  assert.equal(light.traj.length, 7);
  assert.equal(light.traj[0], "#0072b2");
  assert.equal(dark.traj[0], "#56b4e9");
  assert.equal(light.nullcline.length, 4);
  assert.equal(light.lineScale, 1);
  assert.equal(light.glyphScale, 1);
  assert.equal(light.tickSize, 11);
  assert.match(light.tickFont, /^400 11px "Atkinson Hyperlegible Mono"/);
});

// --- PNG ---------------------------------------------------------------------------

function tinyPng(extra = []) {
  const chunk = (type, data) => {
    const out = new Uint8Array(12 + data.length);
    const dv = new DataView(out.buffer);
    dv.setUint32(0, data.length);
    out.set([...type].map((c) => c.charCodeAt(0)), 4);
    out.set(data, 8);
    dv.setUint32(8 + data.length, zcrc32(out.subarray(4, 8 + data.length)));
    return out;
  };
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, 2);
  dv.setUint32(4, 1);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const idat = deflateSync(Buffer.from([0, 255, 0, 0, 255, 0, 0, 255, 255]));
  const parts = [
    Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", ihdr),
    ...extra.map(([t, d]) => chunk(t, d)),
    chunk("IDAT", new Uint8Array(idat)),
    chunk("IEND", new Uint8Array(0)),
  ];
  return new Uint8Array(Buffer.concat(parts));
}

test("PNG: scale 2×, 3× and 4× multiply the pixels; the physical size stays the CSS size", () => {
  for (const k of [2, 3, 4]) {
    const r = pngRaster(800, 640, k);
    assert.deepEqual(r, { width: 800 * k, height: 640 * k, scale: k, reduced: false });
  }
  const odd = pngRaster(301.5, 241, 3);
  assert.deepEqual([odd.width, odd.height], [905, 723]);
});

test("PNG: above 16.7 megapixels the scale is reduced to fit, never the figure", () => {
  const r = pngRaster(2000, 1600, 4);
  assert.equal(r.reduced, true);
  assert.ok(r.width * r.height <= PNG_MAX_PIXELS);
  assert.ok(r.width * r.height > PNG_MAX_PIXELS * 0.99, "as large as fits");
  assert.ok(Math.abs(r.width / r.height - 2000 / 1600) < 1e-3, "same aspect");
  assert.ok(r.scale < 4 && r.scale > 2);
  assert.equal(pngRaster(4096, 4096, 1).reduced, false, "exactly at the cap");
});

test("PNG: pHYs at 96·scale dpi goes right after IHDR, with a valid CRC, replacing any earlier one", () => {
  const src = tinyPng([["pHYs", new Uint8Array(9)]]);
  const out = withPhys(src, 96 * 2);
  const chunks = pngChunks(out);
  assert.deepEqual(chunks.map((c) => c.type), ["IHDR", "pHYs", "IDAT", "IEND"]);
  const p = chunks[1];
  const dv = new DataView(out.buffer, out.byteOffset);
  assert.equal(dv.getUint32(p.offset + 8), pixelsPerMetre(192));
  assert.equal(dv.getUint32(p.offset + 12), 7559);
  assert.equal(out[p.offset + 16], 1, "unit: metre");
  for (const c of chunks) {
    const want = dv.getUint32(c.offset + 8 + c.length);
    assert.equal(crc32(out, c.offset + 4, c.offset + 8 + c.length), want, `${c.type} CRC`);
    assert.equal(zcrc32(out.subarray(c.offset + 4, c.offset + 8 + c.length)), want);
  }
  assert.equal(physDpi(out), 192);
  assert.equal(physDpi(withPhys(tinyPng(), 288)), 288);
  assert.equal(physDpi(tinyPng()), null);
  assert.throws(() => withPhys(new Uint8Array([1, 2, 3]), 96), /Not a PNG/);
});

// --- options -------------------------------------------------------------------------

test("readExportOptions: anything unknown or out of range falls back field by field", () => {
  assert.deepEqual(readExportOptions(undefined), DEFAULT_EXPORT_OPTIONS);
  assert.deepEqual(readExportOptions([1, 2]), DEFAULT_EXPORT_OPTIONS);
  const o = readExportOptions({
    format: "png",
    latexContent: "lyapunov",
    widthCm: 50,
    heightCm: 5,
    pngScale: 5,
    palette: "dark",
    field: "yes",
    digits: 6.4,
    csvIcIds: ["ic-1", 2],
    future: { x: 1 },
  });
  assert.equal(o.format, "png");
  assert.equal(o.latexContent, "figure");
  assert.equal(o.widthCm, DEFAULT_EXPORT_OPTIONS.widthCm);
  assert.equal(o.heightCm, 5);
  assert.equal(o.pngScale, 2);
  assert.equal(o.palette, "dark");
  assert.equal(o.field, true);
  assert.equal(o.digits, 6);
  assert.equal(o.csvIcIds, null);
  assert.ok(!("future" in o));
  assert.equal(DEFAULT_EXPORT_OPTIONS.palette, "light", "figures default to the projector palette");
  assert.equal(sizeError(0.5, "width"), "Use a width between 1 and 40 cm.");
  assert.equal(sizeError(40, "height"), null);
  assert.equal(fileStem("Rabbits vs sheep"), "rabbits-vs-sheep");
  assert.equal(fileStem("Lorenz — σ, ρ, β"), "lorenz");
  assert.deepEqual(figurePx({ ...DEFAULT_EXPORT_OPTIONS, widthCm: 2.54, heightCm: 1.27 }), { width: 96, height: 48 });
});

// --- job ---------------------------------------------------------------------------

const compiled = {
  ok: true,
  systemId: 7,
  kind: "ode",
  autonomous: true,
  states: [
    { name: "x", latex: "x", derived: false },
    { name: "y", latex: "y", derived: false },
  ],
  params: [],
  diagnostics: [],
};

function snapshot(over = {}) {
  return {
    compiled,
    params: [],
    view: { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [-1, 1], yRange: [-1, 1] },
    baseState: [0, 0],
    fieldTime: 0,
    ics: [
      { id: 1, x0: [0.3, 0.2], t0: 0, color: 2 },
      { id: 2, x0: [0.1, 0.1], t0: 0, color: 3, hidden: true },
      { id: 3, x0: [0.5, 0.5], t0: 0, color: 4, uid: "ic-from-file" },
    ],
    layers: { ...DEFAULT_LAYERS, backward: true },
    solver: { ...DEFAULT_SOLVER },
    plotAspect: 1,
    shapeId: 1,
    ...over,
  };
}

test("buildFigureJob: the figure's aspect sets the ranges, samples are clamped, hidden runs left out", () => {
  const job = buildFigureJob(snapshot(), { widthPx: 400, heightPx: 200 }, { field: false, nullclines: true, fixedPoints: true });
  assert.ok(job);
  assert.deepEqual(job.portrait.xRange, [-2, 2], "equal scales at 2:1");
  assert.deepEqual(job.portrait.yRange, [-1, 1]);
  assert.equal(job.portrait.fieldGrid, 0);
  assert.equal(job.portrait.nullclines, true);
  assert.equal(job.portrait.requestId, 0);
  assert.ok(job.portrait.samples >= 2000 && job.portrait.samples <= 20000);
  assert.deepEqual(
    job.tags,
    [
      { icId: "ic-1", backward: false, color: 2 },
      { icId: "ic-1", backward: true, color: 2 },
      { icId: "ic-from-file", backward: false, color: 4 },
      { icId: "ic-from-file", backward: true, color: 4 },
    ],
  );
  assert.equal(job.portrait.trajectories.length, job.tags.length);
  assert.equal(exportSamples(400), 2000);
  assert.equal(exportSamples(5000), 5000);
  assert.equal(exportSamples(80000), 20000);
  assert.equal(icProjectId({ id: 4, x0: [], t0: 0, color: 0 }), "ic-4");
  assert.equal(buildFigureJob(snapshot({ compiled: null }), { widthPx: 1, heightPx: 1 }, {}), null);
});

test("exportScene: no selection, hover, handles, eigenvectors or fade", () => {
  const s = snapshot({ selection: { kind: "ic", id: 1 } });
  const job = buildFigureJob(s, { widthPx: 300, heightPx: 300 }, { field: true, nullclines: true, fixedPoints: false });
  const res = {
    requestId: 0,
    arrows: [],
    nullclines: [],
    fixedPoints: [fp([0, 0], "saddle", false)],
    trajectories: job.portrait.trajectories.map(() => ({ t: [0, 1], x: [0, 0, 1, 1], status: "completed" })),
    stats: {},
  };
  const scene = exportScene(job, res, s.ics);
  assert.equal(scene.selectedIc, null);
  assert.equal(scene.hoverFp, null);
  assert.deepEqual(scene.ics, []);
  assert.equal(scene.eigenvectors, false);
  assert.equal(scene.stale, false);
  assert.deepEqual(scene.fixedPoints, [], "fixed points were not asked for");
  assert.deepEqual(scene.trajectories.map((t) => t.color), [2, 2, 4, 4]);
});

test("buildCsvRequest: the screen's runs, the CSV's own samples, the chosen initial conditions", () => {
  const req = buildCsvRequest(snapshot(), { icIds: ["ic-from-file", "gone"], samples: 1234 }, { lines: ["a"], format: "latex", title: "T" });
  assert.equal(req.portrait.samples, 1234);
  assert.deepEqual(req.icIds, ["ic-from-file"]);
  assert.equal(req.tags.length, req.portrait.trajectories.length);
  assert.equal(req.portrait.fieldGrid, 0);
  const all = buildCsvRequest(snapshot(), { icIds: null, samples: 0 }, { lines: [], format: "text", title: "T" });
  assert.deepEqual(all.icIds, []);
  assert.ok(all.portrait.samples > 0);
});

// --- heads.json ---------------------------------------------------------------------

const headsPath = fileURLToPath(new URL("../../../crates/export/tests/vectors/heads.json", import.meta.url));

/** A polyline in pixels as a flat list. */
function poly(f, n) {
  const out = [];
  for (let k = 0; k < n; k++) out.push(...f(k / (n - 1)));
  return out;
}

function headsCases() {
  const vpBox = [0, 0, 600, 400];
  const base = { vp: vpBox, L: 8, maxHeads: 12, minGap: 24, backward: false, placed: [] };
  return [
    { name: "straight, single head", ...base, pts: poly((s) => [50 + 400 * s, 200], 81) },
    { name: "short: no head", ...base, pts: poly((s) => [100 + 20 * s, 100], 5) },
    { name: "long: one head per 300 px", ...base, pts: poly((s) => [20 + 560 * s, 30 + 340 * Math.abs(Math.sin(3 * Math.PI * s))], 900) },
    { name: "spiral", ...base, pts: poly((s) => [300 + 160 * (1 - 0.7 * s) * Math.cos(12 * s), 200 + 160 * (1 - 0.7 * s) * Math.sin(12 * s)], 600) },
    { name: "backward", ...base, backward: true, pts: poly((s) => [80 + 300 * s, 80 + 200 * s * s], 120) },
    { name: "3D: one head", ...base, maxHeads: 1, pts: poly((s) => [20 + 560 * s, 200 + 150 * Math.sin(6 * s)], 700) },
    { name: "crowded by an earlier head", ...base, placed: [250, 200], pts: poly((s) => [50 + 400 * s, 200], 81) },
    { name: "partly off view", ...base, pts: poly((s) => [-300 + 700 * s, 100 + 250 * s], 200) },
    { name: "kinked: slides to the straight part", ...base, pts: [...poly((s) => [100 + 200 * s, 200], 40), ...poly((s) => [300, 200 - 150 * s], 40).slice(2)] },
    { name: "selected head length", ...base, L: 9, pts: poly((s) => [40 + 520 * s, 380 - 360 * s], 300) },
    { name: "repeated points", ...base, pts: [100, 100, 100, 100, ...poly((s) => [100 + 100 * s, 100 + 30 * s], 30), 200, 130, 200, 130] },
  ];
}

function runHeads(c) {
  const { ctx, log } = recorder();
  const v = { left: c.vp[0], top: c.vp[1], width: c.vp[2], height: c.vp[3], xRange: [0, 1], yRange: [0, 1] };
  const placed = [...c.placed];
  draw.drawHeads(ctx, v, Float64Array.from(c.pts), c.backward, c.L, c.maxHeads, placed, c.minGap);
  const tris = [];
  for (let i = 0; i < log.length; i++) {
    if (log[i][0] !== "moveTo") continue;
    const [a, b, d] = [log[i], log[i + 1], log[i + 2]];
    assert.equal(b[0], "lineTo");
    assert.equal(d[0], "lineTo");
    assert.equal(log[i + 3][0], "closePath");
    tris.push([a[1], a[2], b[1], b[2], d[1], d[2]]);
  }
  return { triangles: tris, placedAfter: placed };
}

test("heads.json: drawHeads through a recording context matches the parity vectors", () => {
  const cases = headsCases().map((c) => {
    const { name, ...rest } = c;
    return { name, ...rest, ...runHeads(c) };
  });
  assert.ok(cases.some((c) => c.triangles.length === 0), "a case with no head");
  assert.ok(cases.some((c) => c.triangles.length > 1), "a case with several heads");
  assert.equal(cases.find((c) => c.name === "3D: one head").triangles.length, 1);
  assert.equal(cases.find((c) => c.name === "crowded by an earlier head").triangles.length, 0);
  const doc = {
    generator: "app/src/review/export.test.mjs (UPDATE_VECTORS=1 to rewrite)",
    source: "drawHeads in app/src/canvas/draw.ts",
    cases,
  };
  if (process.env.UPDATE_VECTORS === "1" || !existsSync(headsPath)) {
    const body = cases.map((c) => `  ${JSON.stringify(c)}`).join(",\n");
    const head = `{\n "generator": ${JSON.stringify(doc.generator)},\n "source": ${JSON.stringify(doc.source)},\n "cases": [\n`;
    writeFileSync(headsPath, `${head}${body}\n ]\n}\n`);
  }
  const saved = JSON.parse(readFileSync(headsPath, "utf8"));
  assert.deepEqual(saved.cases, JSON.parse(JSON.stringify(cases)), "heads.json is out of date: run with UPDATE_VECTORS=1");
});
