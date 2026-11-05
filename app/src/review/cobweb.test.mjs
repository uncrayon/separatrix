// Cobweb plots of 1-D maps (Strogatz §10.1): the geometry in canvas/cobweb.ts,
// the drawing through draw.ts's DrawContext, the studio model's cobweb view,
// and the cobweb.json vectors that the Rust export crate checks its own
// staircase and diagonal against (crates/export/tests/vectors.rs).
//
// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports resolve to .ts through a module hook.
// Regenerate cobweb.json with UPDATE_VECTORS=1.

import test from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import tokens from "../styles/ds/tokens.json" with { type: "json" };
import templates from "../templates/templates.json" with { type: "json" };

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

const cob = await import("../canvas/cobweb.ts");
const draw = await import("../canvas/draw.ts");
const { makeViewport, plotViewport, sx, sy } = await import("../canvas/viewport.ts");
const { plotThemeFromTokens } = await import("../export/theme.ts");
const { buildFigureJob, exportScene, exportGraphSamples, axisNames } = await import("../export/job.ts");
const model = await import("../state/model.ts");
const { viewAxisLabels, mapStabilityHint } = await import("../math/format.ts");

const dark = plotThemeFromTokens(tokens, "dark");
const projector = { ...plotThemeFromTokens(tokens, "light"), lineScale: 1.75, glyphScale: 1.3 };
const logisticT = templates.find((t) => t.id === "logistic-map");
const henonT = templates.find((t) => t.id === "henon-map");

const st = (name, latex = name) => ({ name, latex, derived: false });
const logisticSys = { ok: true, systemId: 1, kind: "map", hamiltonian: false, autonomous: true, states: [st("x")], params: [{ name: "r", latex: "r", default: 3.2 }], equationsLatex: [], diagnostics: [] };
const henonSys = { ...logisticSys, states: [st("x"), st("y")], params: [] };
const nonAutonomous = { ...logisticSys, autonomous: false };
const flow2 = { ...henonSys, kind: "ode" };

const logistic = (r, x0, n, skip = 0) => {
  let x = x0;
  for (let k = 0; k < skip; k++) x = r * x * (1 - x);
  const out = [x];
  for (let k = 0; k < n; k++) {
    x = r * x * (1 - x);
    out.push(x);
  }
  return out;
};

// --- geometry ----------------------------------------------------------------------

test("staircase: 2m + 1 vertices, starting and ending on the diagonal, alternating vertical and horizontal", () => {
  const v = logistic(3.2, 0.1, 9);
  const p = cob.cobwebVertices(v);
  const m = v.length - 1;
  assert.equal(p.length / 2, 2 * m + 1);
  assert.equal(p[0], p[1], "starts on the diagonal");
  assert.equal(p.at(-2), p.at(-1), "ends on the diagonal");
  for (let k = 1; k < p.length / 2; k++) {
    const [ax, ay, bx, by] = p.slice(2 * k - 2, 2 * k + 2);
    if (k % 2 === 1) assert.equal(ax, bx, `segment ${k} is vertical, to the graph`);
    else assert.equal(ay, by, `segment ${k} is horizontal, to the diagonal`);
  }
  // Each vertical lands on the graph: (v_{k−1}, v_k) with v_k = f(v_{k−1}).
  for (let k = 1; k <= m; k++) assert.deepEqual(p.slice(4 * k - 2, 4 * k), [v[k - 1], v[k]]);
});

test("staircase: cut at the first non-finite value; nothing from a non-finite start", () => {
  assert.deepEqual(cob.cobwebVertices([0.5, 2, NaN, 3]), [0.5, 0.5, 0.5, 2, 2, 2]);
  assert.deepEqual(cob.cobwebVertices([Infinity, 1]), []);
  assert.deepEqual(cob.cobwebVertices([]), []);
  assert.deepEqual(cob.cobwebVertices([0.3]), [0.3, 0.3]);
});

test("diagonal: the part of y = x in view, or none", () => {
  assert.deepEqual(cob.diagonalSegment([-0.05, 1.05], [-0.05, 1.05]), [-0.05, -0.05, 1.05, 1.05]);
  assert.deepEqual(cob.diagonalSegment([0, 2], [1, 3]), [1, 1, 2, 2]);
  assert.equal(cob.diagonalSegment([0, 1], [2, 3]), null, "misses the view");
  assert.equal(cob.diagonalSegment([0, 1], [1, 2]), null, "touches only a corner");
  assert.deepEqual(cob.diagonalSegment([-1, 1], [-10, 10]), [-1, -1, 1, 1]);
});

test("graph to px keeps the breaks between polylines", () => {
  const vp = makeViewport(200, 200, [-1, 1], [-3, 3]);
  const graph = [
    [[-1, -1], [-0.5, -2], [-0.34, -3]],
    [[0.34, 3], [0.5, 2], [1, 1]],
    [[0.9, 0]],
  ];
  const px = cob.graphPx(vp, graph);
  assert.equal(px.length, 2, "two polylines; a lone point draws nothing");
  assert.equal(px[0].length, 6);
  assert.equal(px[0][0], sx(vp, -1));
  assert.equal(px[1][1], sy(vp, 3));
});

test("dedupe: a long converged orbit draws each segment of its cycle once, in order", () => {
  const vp = makeViewport(600, 600, [-0.05, 1.05], [-0.05, 1.05]);
  const short = cob.cobwebPx(vp, logistic(3.2, 0.1, 1000));
  assert.equal(short.length, 1, "below 2,000 iterates nothing is dropped");
  assert.equal(short[0].length / 2, 2 * 1000 + 1);
  const long = cob.cobwebPx(vp, logistic(3.2, 0.1, 5000, 200));
  const segments = long.reduce((n, p) => n + p.length / 2 - 1, 0);
  assert.ok(segments <= 8, `the 2-cycle box costs a few segments, not 10,000 (${segments})`);
  assert.equal(long[0][0], sx(vp, logistic(3.2, 0.1, 0, 200)[0]), "starts where the orbit starts");
  // The 4-cycle of r = 3.5.
  const four = cob.cobwebPx(vp, logistic(3.5, 0.1, 5000, 500));
  assert.ok(four.reduce((n, p) => n + p.length / 2 - 1, 0) <= 16);
});

test("hit distance and the zoom-to-fit square", () => {
  assert.equal(cob.pathsDist2(5, 0, [Float64Array.of(0, 0, 10, 0)]), 0);
  assert.equal(cob.pathsDist2(5, 3, [Float64Array.of(0, 0, 10, 0)]), 9);
  const r = cob.cobwebFit([0.2, 0.8, NaN]);
  assert.ok(Math.abs(r[0] - 0.17) < 1e-12 && Math.abs(r[1] - 0.83) < 1e-12, "padded by 5% on each side");
  assert.equal(cob.cobwebFit([]), null);
});

// --- cobweb.json ---------------------------------------------------------------------

const vectorsPath = fileURLToPath(new URL("../../../crates/export/tests/vectors/cobweb.json", import.meta.url));

function vectorCases() {
  const view = { xRange: [-0.05, 1.05], yRange: [-0.05, 1.05] };
  const raw = [
    { x: [0.1, 0.288, 0.65618], ...view },
    { x: logistic(3.2, 0.1, 12).map((v) => Number(v.toPrecision(12))), ...view },
    { x: [0.4], xRange: [0, 2], yRange: [1, 3] },
    { x: [0.6, 0.6, 0.6], xRange: [0, 1], yRange: [2, 3] },
    { x: [-2, 3, -1], xRange: [-1, 1], yRange: [-10, 10] },
  ];
  return raw.map((c) => ({
    ...c,
    staircase: cob.cobwebVertices(c.x),
    diagonal: cob.diagonalSegment(c.xRange, c.yRange),
  }));
}

test("cobweb.json: the staircase and diagonal match the parity vectors", () => {
  const cases = vectorCases();
  assert.deepEqual(cases[0].staircase, [0.1, 0.1, 0.1, 0.288, 0.288, 0.288, 0.288, 0.65618, 0.65618, 0.65618]);
  assert.deepEqual(cases[0].diagonal, [-0.05, -0.05, 1.05, 1.05]);
  assert.equal(cases[3].diagonal, null);
  const doc = {
    generator: "app/src/review/cobweb.test.mjs (UPDATE_VECTORS=1 to rewrite)",
    source: "cobwebVertices and diagonalSegment in app/src/canvas/cobweb.ts",
    cases,
  };
  if (process.env.UPDATE_VECTORS === "1" || !existsSync(vectorsPath)) {
    const body = cases.map((c) => `  ${JSON.stringify(c)}`).join(",\n");
    const head = `{\n "generator": ${JSON.stringify(doc.generator)},\n "source": ${JSON.stringify(doc.source)},\n "cases": [\n`;
    writeFileSync(vectorsPath, `${head}${body}\n ]\n}\n`);
  }
  const saved = JSON.parse(readFileSync(vectorsPath, "utf8"));
  assert.deepEqual(saved.cases, JSON.parse(JSON.stringify(cases)), "cobweb.json is out of date: run with UPDATE_VECTORS=1");
});

// --- drawScene ---------------------------------------------------------------------

/** A DrawContext that records each stroke and fill with its path and pen. */
function recorder() {
  const ops = [];
  let path = [];
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
    save() {},
    restore() {},
    beginPath: () => (path = []),
    closePath() {},
    moveTo: (x, y) => path.push(["M", x, y]),
    lineTo: (x, y) => path.push(["L", x, y]),
    arc: (x, y, r) => path.push(["A", x, y, r]),
    rect: (...a) => path.push(["R", ...a]),
    clip() {},
    fill: () => ops.push({ op: "fill", style: ctx.fillStyle, alpha: ctx.globalAlpha, path: [...path] }),
    stroke: () =>
      ops.push({
        op: "stroke",
        style: ctx.strokeStyle,
        width: ctx.lineWidth,
        join: ctx.lineJoin,
        cap: ctx.lineCap,
        alpha: ctx.globalAlpha,
        path: [...path],
      }),
    fillRect: (...a) => ops.push({ op: "fillRect", style: ctx.fillStyle, a }),
    setLineDash() {},
    fillText() {},
    strokeText() {},
    measureText: (t) => ({ width: 6 * [...t].length }),
  };
  return { ctx, ops };
}

const FP_UNSTABLE = { x: [0.6875], eigenvalues: [[-1.2, 0]], eigenvectors: [[1]], eigenvectorsIm: [null], classification: "source", stable: false };

function cobwebScene(orbit, overrides = {}) {
  const graph = [Array.from({ length: 201 }, (_, i) => {
    const x = -0.05 + (1.1 * i) / 200;
    return [x, 3.2 * x * (1 - x)];
  })];
  return {
    kind: "map",
    dim: 1,
    xAxis: { kind: "state", index: 0 },
    yAxis: { kind: "state", index: 0 },
    arrows: [],
    fieldGrid: 0,
    nullclines: [],
    fixedPoints: [FP_UNSTABLE],
    trajectories: [
      { out: { t: orbit.map((_, k) => k), x: orbit, status: "completed", stats: { accepted: 0, rejected: 0, evals: 0 } }, meta: { icId: 1, direction: "forward" }, color: 0 },
    ],
    ics: [{ id: 1, x0: [orbit[0]], t0: 0, color: 0 }],
    selectedIc: null,
    selectedFp: null,
    hoverIc: null,
    hoverFp: null,
    eigenvectors: false,
    stale: false,
    avoid: { labels: [], field: [] },
    cobweb: { graph },
    ...overrides,
  };
}

function drawCobwebScene(th, scene) {
  const vp = makeViewport(500, 500, [-0.05, 1.05], [-0.05, 1.05], true);
  const { ctx, ops } = recorder();
  draw.drawScene(ctx, 500, 500, vp, scene, th);
  return { vp, ops };
}

const segmentsOf = (path) => {
  const out = [];
  for (let i = 1; i < path.length; i++) if (path[i][0] === "L") out.push([path[i - 1][1], path[i - 1][2], path[i][1], path[i][2]]);
  return out;
};

for (const [name, th] of [
  ["screen", dark],
  ["projector", projector],
]) {
  test(`drawScene draws a cobweb: diagonal, graph, staircase, glyph on the diagonal, no dots (${name})`, () => {
    const s = th.lineScale;
    const orbit = logistic(3.2, 0.1, 30);
    const { vp, ops } = drawCobwebScene(th, cobwebScene(orbit));
    const strokes = ops.filter((o) => o.op === "stroke");

    const diag = strokes.filter((o) => o.style === th.axis && o.path.length === 2);
    assert.equal(diag.length, 1, "one 2-point stroke in the axis colour");
    const [[, x0, y0], [, x1, y1]] = diag[0].path;
    assert.ok(Math.abs(Math.abs(x1 - x0) - Math.abs(y1 - y0)) < 1e-9, "at 45° in px");
    assert.ok(y1 < y0 && x1 > x0, "rising to the right");
    assert.equal(diag[0].width, s);
    assert.equal(diag[0].cap, "butt");

    const graph = strokes.filter((o) => o.style === th.fp && o.path.length > 100);
    assert.equal(graph.length, 1, "the graph of f in plot-fp");
    assert.equal(graph[0].width, 1.5 * s);
    assert.equal(graph[0].join, "round");
    assert.equal(graph[0].cap, "round");

    const stair = strokes.filter((o) => o.style === th.traj[0]);
    assert.equal(stair.length, 1, "one staircase");
    assert.equal(stair[0].width, s, "thinner than a flow's trajectory");
    assert.equal(stair[0].join, "miter");
    assert.equal(stair[0].cap, "butt");
    assert.equal(stair[0].alpha, 1, "opaque");
    const segs = segmentsOf(stair[0].path);
    assert.equal(segs.length, 2 * (orbit.length - 1));
    for (const [ax, ay, bx, by] of segs) assert.ok(ax === bx || ay === by, "only vertical and horizontal segments");

    // The z-order: diagonal, graph, staircase, then the fixed point.
    const at = (o) => ops.indexOf(o);
    assert.ok(at(diag[0]) < at(graph[0]) && at(graph[0]) < at(stair[0]));
    const glyph = ops.find((o) => o.op === "stroke" && o.style === th.fp && o.path.length === 1 && o.path[0][0] === "A");
    assert.ok(glyph && at(glyph) > at(stair[0]), "the fixed point is drawn over the staircase");
    const [, gx, gy, gr] = glyph.path[0];
    assert.ok(Math.abs(gx - sx(vp, 0.6875)) < 1e-9 && Math.abs(gy - sy(vp, 0.6875)) < 1e-9, "centred on the diagonal at (x*, x*)");
    assert.equal(gr, 5 * th.glyphScale);

    // No map dots: no fill in the orbit's colour except the start handle (r = 3.5 g).
    const trajFills = ops.filter((o) => (o.op === "fill" || o.op === "fillRect") && o.style === th.traj[0]);
    for (const f of trajFills) {
      assert.ok(f.path.length === 1 && Math.abs(f.path[0][3] - 3.5 * th.glyphScale) < 1e-9, "only the start handle");
      assert.ok(Math.abs(f.path[0][1] - sx(vp, 0.1)) < 1e-9 && Math.abs(f.path[0][2] - sy(vp, 0.1)) < 1e-9, "on the diagonal at (x0, x0)");
    }
  });
}

test("drawScene: the selected staircase is drawn last and a little heavier; an early stop gets an ×", () => {
  const scene = cobwebScene(logistic(3.2, 0.1, 10));
  scene.trajectories.push({
    out: { t: [0, 1, 2], x: [0.5, 2, 1e13], status: "blewUp", stats: { accepted: 0, rejected: 0, evals: 0 } },
    meta: { icId: 2, direction: "forward" },
    color: 1,
  });
  const { ops } = drawCobwebScene(dark, { ...scene, selectedIc: 1 });
  const strokes = ops.filter((o) => o.op === "stroke" && dark.traj.slice(0, 2).includes(o.style));
  assert.equal(strokes.at(-1).style, dark.traj[0], "the selected one last");
  assert.equal(strokes.at(-1).width, 1.5);
  const cross = strokes.find((o) => o.style === dark.traj[1] && o.path.length === 4 && o.path[0][0] === "M" && o.path[2][0] === "M");
  assert.ok(cross, "an × where the blown-up run stops");
});

test("drawScene: a stale cobweb is drawn at 55%, the diagonal at full strength; no graph until one arrives", () => {
  const { ops } = drawCobwebScene(dark, cobwebScene(logistic(3.2, 0.1, 10), { stale: true, cobweb: { graph: [] } }));
  const strokes = ops.filter((o) => o.op === "stroke");
  assert.equal(strokes.find((o) => o.style === dark.traj[0]).alpha, 0.55);
  assert.equal(strokes.find((o) => o.style === dark.axis && o.path.length === 2).alpha, 1);
  assert.ok(!strokes.some((o) => o.style === dark.fp && o.path.length > 100), "no graph drawn");
});

// --- the model ---------------------------------------------------------------------

const snapshot = (over = {}) => ({
  compiled: logisticSys,
  params: [{ name: "r", latex: "r", value: 3.5, min: 0, max: 4 }],
  view: model.templateView(logisticT, logisticSys),
  baseState: [0],
  fieldTime: 0,
  ics: [{ id: 1, x0: [0.1], t0: 0, color: 0 }],
  layers: { ...model.DEFAULT_LAYERS },
  solver: { ...model.DEFAULT_SOLVER, tEnd: 60, transient: 200 },
  plotAspect: 1.6,
  shapeId: 1,
  ...over,
});

test("the logistic map opens as a cobweb: default view and template view", () => {
  assert.equal(logisticT.view.kind, "cobweb");
  const v = model.templateView(logisticT, logisticSys);
  assert.deepEqual(v, { kind: "cobweb", xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 0 }, xRange: [-0.05, 1.05], yRange: [-0.05, 1.05] });
  assert.equal(model.defaultView(logisticSys).kind, "cobweb");
  assert.deepEqual(model.defaultView(logisticSys).xRange, [-0.1, 1.1]);
  // A map that changes with n has no single graph: it opens as a time series.
  assert.equal(model.defaultView(nonAutonomous).kind, undefined);
  assert.equal(model.defaultView(nonAutonomous).xAxis.kind, "time");
  assert.equal(model.templateView(logisticT, nonAutonomous).xAxis.kind, "time");
  assert.equal(model.defaultView(henonSys).kind, undefined);
  assert.equal(model.templateView(henonT, henonSys).kind, undefined);
});

test("availability: the cobweb is for 1-D autonomous maps, the phase plane for 2 states or more", () => {
  assert.deepEqual(model.cobwebAvailability(logisticSys), { ok: true });
  assert.deepEqual(model.cobwebAvailability(henonSys, "Hénon map"), { ok: false, reason: "The cobweb view needs a one-dimensional map $x_{n+1} = f(x_n)$; Hénon map has 2 states." });
  assert.deepEqual(model.cobwebAvailability(flow2, "Van der Pol"), { ok: false, reason: "The cobweb view needs a one-dimensional map $x_{n+1} = f(x_n)$; Van der Pol is a flow." });
  // The core's sentence for the same condition (service.rs cobweb_fits), with the maths typeset.
  assert.equal(model.cobwebAvailability(flow2).reason.replaceAll("$", ""), "The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system is a flow.");
  assert.deepEqual(model.cobwebAvailability(nonAutonomous), { ok: false, reason: "This map changes with n, so f has no single graph to cobweb." });
  assert.deepEqual(model.phasePlaneAvailability(logisticSys, "Logistic map"), { ok: false, reason: "A phase plane needs at least 2 states. Logistic map has 1." });
  const all = model.viewChoiceAvailability(henonSys);
  assert.deepEqual([all.phase.ok, all.series.ok, all.cobweb.ok], [true, true, false]);
});

test("buildPortraitJob: a cobweb asks for state 0 on both axes, f's graph and the transient, no field or nullclines", () => {
  const job = model.buildPortraitJob(snapshot());
  const b = job.body;
  assert.deepEqual(b.xAxis, { kind: "state", index: 0 });
  assert.deepEqual(b.yAxis, { kind: "state", index: 0 });
  assert.equal(b.graphSamples, model.COBWEB_GRAPH_SAMPLES);
  assert.equal(b.graphSamples, 2048);
  assert.equal(b.transient, 200);
  assert.equal(b.fieldGrid, 0);
  assert.equal(b.nullclines, false);
  assert.equal(b.fixedPoints, true);
  assert.deepEqual(b.trajectories, [{ x0: [0.1], t0: 0, t1: 60 }]);
  // The time series of the same map hides the same transient, with no graph.
  const series = model.buildPortraitJob(snapshot({ view: { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 60], yRange: [0, 1] } })).body;
  assert.equal(series.graphSamples, undefined);
  assert.equal(series.transient, 200);
  // A flow sends neither.
  const flow = model.buildPortraitJob(snapshot({ compiled: flow2, ics: [], view: model.defaultView(flow2) })).body;
  assert.ok(!("graphSamples" in flow) && !("transient" in flow));
});

test("the transient is clamped so transient + N stays within the core's iterate limit", () => {
  assert.equal(model.clampTransient(200, 60), 200);
  assert.equal(model.clampTransient(-5, 60), 0);
  assert.equal(model.clampTransient(10.6, 60), 11);
  assert.equal(model.clampTransient(2_000_000, 60), 1_000_000 - 60);
  assert.equal(model.clampTransient(NaN, 60), 0);
});

test("a cobweb has equal scales, so the diagonal is at 45°", () => {
  const v = model.cobwebView([0, 1]);
  const r = model.displayRanges(v, "map", 2);
  assert.deepEqual(r.yRange, [0, 1]);
  assert.deepEqual(r.xRange, [-0.5, 1.5]);
  const series = { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 50], yRange: [0, 1] };
  assert.deepEqual(model.displayRanges(series, "map", 2), { xRange: [0, 50], yRange: [0, 1] });
  const vp = plotViewport(800, 400, v, "map");
  assert.equal(vp.width / (vp.xRange[1] - vp.xRange[0]), vp.height / (vp.yRange[1] - vp.yRange[0]));
});

test("adaptView keeps a cobweb only while the system stays a 1-D autonomous map", () => {
  const v = model.cobwebView([0, 1]);
  assert.equal(model.adaptView(v, [st("x")], [st("x"), st("y")], henonSys), null, "to Hénon");
  assert.equal(model.adaptView(v, [st("x")], [st("x")], nonAutonomous), null, "now changing with n");
  assert.deepEqual(model.adaptView(v, [st("x")], [st("u")], logisticSys), v, "a renamed state keeps it");
});

test("liftPlotPoint: a cobweb's start point takes only the pointer's x; it lands on the diagonal", () => {
  const v = model.cobwebView([0, 1]);
  assert.deepEqual(model.liftPlotPoint(logisticSys, v, [0], 0, 0.3, 0.9), { x0: [0.3], t0: 0 });
  assert.deepEqual(model.liftPlotPoint(logisticSys, v, [0], 0, 0.42, -7, { x0: [0.3], t0: 0 }), { x0: [0.42], t0: 0 });
  const series = { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 50], yRange: [0, 1] };
  assert.deepEqual(model.liftPlotPoint(logisticSys, series, [0], 0, 12, 0.9), { x0: [0.9], t0: 0 });
});

test("axis names: x_n across and x_{n+1} up, from the user's own state LaTeX", () => {
  const v = model.cobwebView([0, 1]);
  assert.deepEqual(viewAxisLabels(v, [st("x")], "map"), { x: "x_{n}", y: "x_{n+1}" });
  assert.deepEqual(viewAxisLabels(v, [st("p", "p_{1}")], "map"), { x: "p_{1,n}", y: "p_{1,n+1}" });
  const series = { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 50], yRange: [0, 1] };
  assert.deepEqual(viewAxisLabels(series, [st("x")], "map"), { x: "n", y: "x_{n}" });
  assert.deepEqual(axisNames(snapshot()), { x: "x_{n}", y: "x_{n+1}" });
});

test("switchView: Cobweb → Time series → Cobweb restores the ranges; impossible views are refused", () => {
  const memory = {};
  const cob0 = { ...model.cobwebView([0.2, 0.9]) };
  const s = { compiled: logisticSys, view: cob0, solver: { ...model.DEFAULT_SOLVER, tEnd: 60 } };
  const series = model.switchView("series", s, memory);
  assert.deepEqual(series, { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [-0.04 * 60, 60], yRange: [0.2, 0.9] });
  const zoomed = { ...series, yRange: [0.3, 0.8] };
  assert.equal(model.switchView("cobweb", { ...s, view: zoomed }, memory), cob0, "the cobweb as it was left");
  assert.equal(model.switchView("series", { ...s, view: cob0 }, memory), zoomed, "and the time series as it was left");
  assert.equal(model.switchView("phase", s, {}), null, "a 1-D map has no phase plane");
  assert.equal(model.switchView("cobweb", s, {}), null, "already shown");
  // Without a remembered one, a cobweb takes the time series' state range.
  const fresh = model.switchView("cobweb", { ...s, view: zoomed }, {});
  assert.deepEqual([fresh.kind, fresh.xRange, fresh.yRange], ["cobweb", [0.3, 0.8], [0.3, 0.8]]);
  // Hénon: no cobweb; a time series of x over n from its phase plane.
  const hs = { compiled: henonSys, view: { xAxis: { kind: "state", index: 0 }, yAxis: { kind: "state", index: 1 }, xRange: [-1.6, 1.6], yRange: [-0.5, 0.5] }, solver: { ...model.DEFAULT_SOLVER, tEnd: 100 } };
  assert.equal(model.switchView("cobweb", hs, {}), null);
  assert.deepEqual(model.switchView("series", hs, {}).yRange, [-1.6, 1.6]);
});

test("a time series of a map with a transient T shows n from T to T + N", () => {
  const solver = { ...model.DEFAULT_SOLVER, tEnd: 60, transient: 200 };
  assert.deepEqual(model.seriesRange(solver, "map"), [200 - 0.04 * 60, 260]);
  assert.deepEqual(model.seriesRange(solver, "ode"), [-0.04 * 60, 60], "a flow has no transient");
  const s = { compiled: logisticSys, view: model.cobwebView([0, 1]), solver };
  const series = model.switchView("series", s, {});
  assert.ok(series.xRange[0] <= 200 && series.xRange[1] >= 260, `${series.xRange}`);
  // A map that changes with n opens as a time series from n = T.
  const v = model.defaultView(nonAutonomous, solver);
  assert.ok(v.xRange[0] <= 200 && v.xRange[1] > 200, `${v.xRange}`);
  assert.deepEqual(model.defaultView(nonAutonomous).xRange, [0, 50]);
  // A changed transient moves the time axis with the orbit; a cobweb stays.
  const shifted = model.shiftTimeAxes(series, -150);
  assert.deepEqual(shifted.xRange, [series.xRange[0] - 150, series.xRange[1] - 150]);
  assert.deepEqual(shifted.yRange, series.yRange);
  const cob = model.cobwebView([0, 1]);
  assert.equal(model.shiftTimeAxes(cob, 10), cob);
});

test("export: a cobweb figure asks for f's graph at two samples per px, from 2,048 to 4,096, and draws it", () => {
  assert.equal(exportGraphSamples(500), 2048);
  assert.equal(exportGraphSamples(1500), 3000);
  assert.equal(exportGraphSamples(5000), 4096);
  const job = buildFigureJob(snapshot(), { widthPx: 1500, heightPx: 1000 }, { field: false, nullclines: false, fixedPoints: true });
  assert.equal(job.cobweb, true);
  assert.equal(job.portrait.graphSamples, 3000);
  assert.equal(job.portrait.transient, 200);
  const r = job.portrait;
  assert.ok(Math.abs((r.xRange[1] - r.xRange[0]) / (r.yRange[1] - r.yRange[0]) - 1.5) < 1e-12, "equal scales at the figure's aspect");
  const res = { requestId: 0, arrows: [], nullclines: [], fixedPoints: [], trajectories: [{ t: [200, 201], x: [0.5, 0.875], status: "completed", stats: { accepted: 1, rejected: 0, evals: 1 } }], graph: [[[0, 0], [1, 0]]], elapsedMs: 0 };
  const scene = exportScene(job, res, snapshot().ics);
  assert.deepEqual(scene.cobweb, { graph: [[[0, 0], [1, 0]]] });
  const plane = buildFigureJob(snapshot({ view: { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 60], yRange: [0, 1] } }), { widthPx: 600, heightPx: 400 }, { field: false, nullclines: false, fixedPoints: true });
  assert.equal(plane.cobweb, false);
  assert.equal(plane.portrait.graphSamples, undefined);
  assert.equal(exportScene(plane, res, snapshot().ics).cobweb, null);
});

test("the multiplier's words, in Strogatz §10.1's terms", () => {
  const fp = (lambda, classification) => ({ eigenvalues: [[lambda, 0]], classification });
  assert.equal(mapStabilityHint(fp(-1.2, "source")), "|λ| > 1, so x* is unstable. λ < 0: nearby orbits alternate sides of x*.");
  assert.equal(mapStabilityHint(fp(3.2, "source")), "|λ| > 1, so x* is unstable.");
  assert.equal(mapStabilityHint(fp(-0.5, "sink")), "|λ| < 1, so x* is stable. λ < 0: nearby orbits alternate sides of x*.");
  assert.equal(mapStabilityHint(fp(0, "sink")), "|λ| < 1, so x* is stable. λ = 0: superstable.");
  assert.equal(mapStabilityHint(fp(-1, "non-hyperbolic")), "|λ| = 1: linearization does not decide. λ < 0: nearby orbits alternate sides of x*.");
  assert.equal(draw.fixedPointGlyph("non-hyperbolic", false), "half");
  // A kink (the tent map's peak at r = 1, |x| at 0): no multiplier, never "superstable".
  const kink = mapStabilityHint({ eigenvalues: [], classification: "non-hyperbolic" });
  assert.match(kink, /^f is not differentiable at x\*/);
  assert.doesNotMatch(kink, /superstable|\|λ\| [<>=]/);
  assert.match(mapStabilityHint({ eigenvalues: [[1, 0]], classification: "non-isolated" }), /^f\(x\) = x for every x between/);
});

test("new cobweb orbits skip the palette slots too close to the graph of f's ink (traj-4 on projector)", () => {
  const hex = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  const lum = (h) => {
    const [r, g, b] = hex(h).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const contrast = (a, b) => {
    const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
    return (x + 0.05) / (y + 0.05);
  };
  const low = [];
  for (let k = 0; k < model.PALETTE_SIZE; k++) if (contrast(projector.traj[k], projector.fp) < 1.5) low.push(k);
  assert.deepEqual(low, [...model.COBWEB_SKIPPED_COLORS]);
  // The fourth orbit of a cobweb is traj-5, not black; a phase plane keeps every slot.
  const three = [{ color: 0 }, { color: 1 }, { color: 2 }];
  assert.equal(model.newIcColor(three, model.cobwebView([0, 1])), 4);
  assert.equal(model.newIcColor(three, { kind: undefined }), 3);
});

test("a map run's status names the right iterate, inside the transient too", async () => {
  const { stopText, landedOnSource } = await import("../components/runStatus.ts");
  const run = (t, x, status, accepted, evals = accepted) => ({ t, x, status, stats: { accepted, rejected: 0, evals } });
  // x² + 1 from 0.5 blows up at n = 8; with a transient of 10 nothing is returned.
  assert.deepEqual(stopText(run([0, 1, 2, 3, 4, 5, 6, 7, 8], [], "blewUp", 8), true), { words: "Blew up at", stopped: false, at: 8 });
  assert.deepEqual(stopText(run([], [], "blewUp", 8), true), { words: "Blew up during the transient, at", stopped: false, at: 8 });
  // ln x from 0.5: x₁ = −0.693 is returned, x₂ = ln(−0.693) is the NaN.
  assert.deepEqual(stopText(run([0, 1], [0.5, -0.693], "nonFinite", 1, 2), true), { words: "NaN at", stopped: true, at: 2 });
  assert.deepEqual(stopText(run([], [], "nonFinite", 4, 5), true), { words: "NaN during the transient, at", stopped: true, at: 5 });
  assert.deepEqual(stopText(run([], [], "nonFinite", 0, 0), true), { words: "NaN at", stopped: true, at: 0 }, "x₀ itself");
  assert.equal(stopText(run([0, 1], [0, 0], "completed", 1), true), null);
  // A flow's t is the last time reached.
  assert.equal(stopText(run([0, 0.5], [1, 2], "blewUp", 50), false).at, 0.5);

  // The tent map at r = 2 lands exactly on its unstable 0 after 53 steps (doubling uses up the bits).
  let x = 0.1234567;
  const xs = [x];
  for (let k = 0; k < 60; k++) xs.push((x = 1 - Math.abs(2 * x - 1)));
  const tent = run(xs.map((_, k) => k), xs, "completed", 60);
  const source = { x: [0], eigenvalues: [[2, 0]], classification: "source", stable: false };
  const landed = landedOnSource(tent, 1, [source]);
  assert.ok(landed && xs[landed.n] === 0 && xs[landed.n - 1] !== 0, JSON.stringify(landed));
  assert.equal(landed.at, "x* = 0.000");
  // A sink reached exactly is no artefact; nor is an orbit still moving.
  assert.equal(landedOnSource(tent, 1, [{ ...source, eigenvalues: [[0.5, 0]], classification: "sink", stable: true }]), null);
  assert.equal(landedOnSource(run([0, 1, 2], [0.1, 0.2, 0.3], "completed", 2), 1, [source]), null);
});
