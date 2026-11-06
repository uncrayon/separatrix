// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports resolve to .ts through a module hook.

import test from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";
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

const model = await import("../state/model.ts");
const { LYAPUNOV_MISFIT, MAX_RENORMALIZATIONS } = await import("../api/ipc.generated.ts");
const { projectFromState, stateFromProject } = await import("../project/serialize.ts");
const { drawLyapunov, labelRows } = await import("../canvas/lyapunov.ts");
const { SvgContext } = await import("../canvas/svgContext.ts");
const { makeViewport, sy } = await import("../canvas/viewport.ts");
const { plotThemeFromTokens } = await import("../export/theme.ts");

const st = (name) => ({ name, latex: name, derived: false });
const system = (kind, names, systemId = 7) => ({
  ok: true,
  systemId,
  kind,
  hamiltonian: false,
  separable: false,
  autonomous: true,
  states: names.map(st),
  params: [
    { name: "sigma", latex: "\\sigma", default: 10 },
    { name: "r", latex: "r", default: 28 },
  ],
  equationsLatex: [],
  diagnostics: [],
});
const lorenz = system("ode", ["x", "y", "z"]);
const henon = system("map", ["x", "y"], 8);
const ic = (id, x0, t0 = 0) => ({ id, uid: `ic-${id}`, x0, t0, color: id % 7 });

function snapshot(over = {}) {
  const sys = over.compiled ?? lorenz;
  return {
    compiled: sys,
    params: sys.params.map((p) => ({ name: p.name, latex: p.latex, value: p.default, min: 0, max: 50 })),
    view: model.lyapunovView(sys.kind),
    baseState: sys.states.map(() => 0),
    fieldTime: 0,
    ics: [ic(1, sys.states.map((_, i) => i + 1)), ic(2, sys.states.map((_, i) => 10 * (i + 1)), 0.5)],
    layers: model.DEFAULT_LAYERS,
    solver: { ...model.DEFAULT_SOLVER, method: "rk4", h: 0.01 },
    plotAspect: 1.5,
    shapeId: 3,
    selection: null,
    ...over,
  };
}

test("the View control offers the Lyapunov spectrum to flows and to maps", () => {
  assert.ok(model.VIEW_CHOICES_BY_KIND.ode.includes("lyapunov"));
  assert.ok(model.VIEW_CHOICES_BY_KIND.map.includes("lyapunov"));
  assert.deepEqual(model.viewChoiceAvailability(lorenz).lyapunov, { ok: true });
  assert.deepEqual(model.viewChoiceAvailability(system("map", ["x"])).lyapunov, { ok: true });
  assert.equal(model.VIEW_CHOICE_LABELS.lyapunov, "Lyapunov spectrum");
  assert.equal(model.viewChoice(model.lyapunovView("ode")), "lyapunov");
});

test("switching to the Lyapunov view opens it with the defaults for the system's kind", () => {
  const flow = model.switchView("lyapunov", { compiled: lorenz, view: model.defaultView(lorenz), solver: model.DEFAULT_SOLVER }, {});
  assert.equal(flow.kind, "lyapunov");
  assert.deepEqual(flow.lyapunov, { transient: 100, total: 1000, interval: 0.1, blocks: 10 });
  assert.deepEqual(flow.xRange, [100, 1100], "t from the transient to its end");
  assert.equal(flow.xAxis.kind, "time");
  const map = model.switchView("lyapunov", { compiled: henon, view: model.defaultView(henon), solver: model.DEFAULT_SOLVER }, {});
  assert.deepEqual(map.lyapunov, { transient: 1000, total: 100000, interval: 1, blocks: 10 });
  assert.equal(model.shiftTimeAxes(map, 50), map);
});

test("the request follows the selected start point, else the first, with the studio's solver", () => {
  const first = model.buildLyapunovJob(snapshot());
  assert.deepEqual(first.body.x0, [1, 2, 3]);
  assert.equal(first.meta.icId, 1);
  assert.deepEqual(
    { transient: first.body.transient, total: first.body.total, interval: first.body.interval, blocks: first.body.blocks },
    { transient: 100, total: 1000, interval: 0.1, blocks: 10 },
  );
  assert.equal(first.body.solver.method, "rk4");
  assert.equal(first.body.solver.h, 0.01);
  assert.deepEqual(first.body.params, [10, 28]);
  const picked = model.buildLyapunovJob(snapshot({ selection: { kind: "ic", id: 2 } }));
  assert.deepEqual(picked.body.x0, [10, 20, 30]);
  assert.equal(picked.body.t0, 0.5, "a flow starts at its start point's t₀");
  assert.notEqual(picked.meta.key, first.meta.key, "another start point is another run");
  const map = model.buildLyapunovJob(snapshot({ compiled: henon, ics: [ic(4, [0, 0], 7)] }));
  assert.equal(map.body.t0, 0, "a map starts at n = 0");
});

test("the step limit covers the whole run, transient included", () => {
  const long = snapshot();
  long.view = { ...long.view, lyapunov: { ...long.view.lyapunov, total: 1e4 } };
  const { solver } = model.buildLyapunovJob(long).body;
  assert.ok(solver.maxSteps >= (100 + 1e4) / 0.01, `${solver.maxSteps} steps of 0.01 reach t = 10,100`);
  const roomy = model.buildLyapunovJob(snapshot({ solver: { ...model.DEFAULT_SOLVER, method: "rk4", h: 0.01, maxSteps: 5e6 } }));
  assert.equal(roomy.body.solver.maxSteps, 5e6, "a larger limit of the studio's is kept");
});

test("nothing is asked for without a start point, outside the view, or with settings the core refuses", () => {
  assert.equal(model.buildLyapunovJob(snapshot({ ics: [] })), null);
  assert.equal(model.buildLyapunovJob(snapshot({ view: model.defaultView(lorenz) })), null);
  const bad = (patch, kind = "ode") => {
    const s = snapshot(kind === "map" ? { compiled: henon } : {});
    s.view = { ...s.view, lyapunov: { ...s.view.lyapunov, ...patch } };
    return model.buildLyapunovJob(s);
  };
  assert.equal(bad({ blocks: 1 }), null, "the ± needs two blocks");
  assert.equal(bad({ total: 0.5 }), null, "10 blocks need 10 QR steps; 0.5 / 0.1 gives 5");
  assert.equal(bad({ interval: 0 }), null);
  assert.equal(bad({ transient: -1 }), null);
  assert.equal(bad({ interval: 1e-5, total: 1e3 }), null, `more than ${MAX_RENORMALIZATIONS} QR steps`);
  assert.equal(bad({ interval: 2.5 }, "map"), null, "a map renormalizes after whole iterates");
  assert.equal(model.lyapunovSettingsError({ transient: 0, total: 10, interval: 1, blocks: 10 }, "map"), null);
  assert.equal(model.lyapunovSettingsError({ transient: 0, total: 10, interval: 1, blocks: 11 }, "map").field, "blocks");
});

test("the Lyapunov view asks the core for no portrait", () => {
  assert.equal(model.buildPortraitJob(snapshot()), null);
  assert.ok(model.buildPortraitJob(snapshot({ view: model.defaultView(lorenz) })));
});

test("a run on hand is current only for the request the view makes now", () => {
  const s = snapshot();
  const job = model.buildLyapunovJob(s);
  const run = { res: { exponents: [1, 0, -1] }, meta: job.meta, roundTripMs: 1 };
  assert.equal(model.lyapunovCurrent({ ...s, lyapunov: run }), true);
  assert.equal(model.lyapunovCurrent({ ...s, selection: { kind: "ic", id: 2 }, lyapunov: run }), false);
  assert.equal(model.lyapunovFits(lorenz, 3, run), true);
  assert.equal(model.lyapunovFits(lorenz, 4, run), false, "another shape of system");
});

test("an exponent is printed to its uncertainty's first significant digit", () => {
  assert.equal(model.lyapunovValueText(0.906, 0.004), "0.906 ± 0.004");
  assert.equal(model.lyapunovValueText(-14.57316, 0.0068), "−14.573 ± 0.007");
  assert.equal(model.lyapunovValueText(-0.0022, 0.0044), "−0.002 ± 0.004");
  assert.equal(model.lyapunovValueText(0.90685, 0.0011), "0.9069 ± 0.0011", "two digits when the first is 1");
  assert.equal(model.lyapunovValueText(0.41943, 0.0096), "0.419 ± 0.010", "0.0096 rounds up a place");
  assert.equal(model.lyapunovValueText(-0.00001, 0.004), "0.000 ± 0.004", "no minus on a rounded zero");
  assert.equal(model.lyapunovValueText(-0.6931471805599453, undefined), "−0.6931", "four significant digits without an error");
  assert.deepEqual(model.roundWithUncertainty(-14.57316, 0.0068), { value: "-14.573", error: "0.007" });
});

test("counts and small numbers read as prose", () => {
  assert.equal(model.countText(1e5), "10⁵");
  assert.equal(model.countText(10000), "10⁴");
  assert.equal(model.countText(12500), "12,500");
  assert.equal(model.countText(100), "100");
  assert.equal(model.scientificText(1.04e-4), "1.0 × 10⁻⁴");
  assert.equal(model.scientificText(-2.5e-13), "−2.5 × 10⁻¹³");
  assert.equal(model.renormalizations({ total: 1000, interval: 0.1 }, "ode"), 10000);
  assert.equal(model.renormalizations({ total: 10, interval: 3 }, "map"), 4);
});

test("the fit spans the measured t and every settled estimate, with λ = 0 in view", () => {
  const res = {
    exponents: [0.9, -14.6],
    historyT: [100.1, 200, 600, 1100],
    history: [40, -90, 0.95, -14.4, 0.91, -14.55, 0.9, -14.6],
  };
  const fit = model.fitLyapunovRanges(res);
  assert.equal(fit.xRange[0], 100.1);
  assert.ok(fit.xRange[1] >= 1100);
  assert.ok(fit.yRange[0] < -14.6 && fit.yRange[1] > 0.95, `${fit.yRange} holds the settled estimates`);
  assert.ok(fit.yRange[1] < 40 && fit.yRange[0] > -90, "the first QR step's swing is left out");
  const positive = model.fitLyapunovRanges({ exponents: [0.69], historyT: [1, 2], history: [0.7, 0.69] });
  assert.ok(positive.yRange[0] < 0, "λ = 0 stays in view");
});

test("drawLyapunov draws each estimate in its own trajectory colour, its final value dashed, and λᵢ", () => {
  const th = plotThemeFromTokens(tokens, "light");
  const measure = (font, text) => {
    const s = parseFloat(/([\d.]+)px/.exec(font)?.[1] ?? "10");
    return { width: 0.6 * s * [...text].length, ascent: 0.8 * s, descent: 0.2 * s };
  };
  const ctx = new SvgContext(measure);
  const vp = makeViewport(600, 400, [100, 1100], [-16, 2]);
  const res = {
    exponents: [0.9, 0.0, -14.57],
    uncertainty: [],
    historyT: [100.1, 600, 1100],
    history: [1.2, 0.1, -14, 0.95, 0.01, -14.5, 0.9, 0.0, -14.57],
  };
  drawLyapunov(ctx, vp, res, th);
  const svg = ctx.toSvg({ width: 600, height: 400 });
  for (let i = 0; i < 3; i++) {
    const strokes = [...svg.matchAll(new RegExp(`stroke="${th.traj[i]}"[^>]*`, "g"))].map((m) => m[0]);
    assert.ok(strokes.some((s) => /stroke-dasharray/.test(s)), `λ${i + 1}'s final value is dashed in traj-${i + 1}`);
    assert.ok(strokes.some((s) => !/stroke-dasharray/.test(s)), `λ${i + 1}'s estimate is solid in traj-${i + 1}`);
  }
  assert.equal((svg.match(/>λ</g) ?? []).length, 6, "λ₁, λ₂, λ₃, each with its halo");
  for (const d of ["1", "2", "3"]) assert.ok(svg.includes(`>${d}</tspan>`), `the subscript ${d}`);
  const rows = labelRows([sy(vp, 0.9), sy(vp, 0)], 18, 0, 400);
  assert.ok(rows[0] < sy(vp, 0.9) && rows[1] < sy(vp, 0));
  assert.ok(Math.abs(rows[1] - rows[0]) >= 18);
});

test("a Lyapunov view is saved as kind lyapunov with its run, and opens as it was", () => {
  const sys = lorenz;
  const view = { ...model.lyapunovView("ode", { transient: 50, total: 10000, interval: 0.2, blocks: 20 }), yRange: [-16, 2] };
  const state = {
    format: "latex",
    equations: [{ id: 1, source: "\\dot{x}=y" }],
    compiled: sys,
    params: [],
    view,
    baseState: [0, 0, 0],
    fieldTime: 0,
    ics: [ic(1, [0, 1, 0])],
    layers: model.DEFAULT_LAYERS,
    solver: model.DEFAULT_SOLVER,
    sourceTemplateId: "lorenz",
  };
  const file = projectFromState(state);
  const v = file.views[0];
  assert.equal(v.kind, "lyapunov");
  assert.deepEqual(v.lyapunov, { transient: 50, total: 10000, interval: 0.2, blocks: 20 });
  assert.deepEqual(v.x_axis, { kind: "time" });
  assert.deepEqual(v.y_axis, { kind: "exponent" });
  assert.deepEqual(v.x_range, [50, 10050]);
  assert.deepEqual(v.y_range, [-16, 2]);
  assert.equal(v.camera, undefined);
  assert.equal(v.section, undefined);

  const load = stateFromProject(JSON.parse(JSON.stringify(file)));
  const applied = model.applyProjectLoad(load, sys, () => 1);
  assert.deepEqual(applied.warnings, []);
  assert.deepEqual(applied.view, view);
  const reopened = { ...file, views: [{ ...v, lyapunov: { ...v.lyapunov, note: "kept" } }] };
  const again = projectFromState({ ...state, view: applied.view }, { loaded: reopened });
  assert.deepEqual(again.views[0], reopened.views[0]);

  const plane = projectFromState({ ...state, view: model.defaultView(sys) }, { loaded: reopened });
  assert.equal(plane.views[0].lyapunov, undefined);
  assert.equal(plane.views[0].kind, undefined);
});

test("a saved Lyapunov view whose run can't be run opens the default view with one warning", () => {
  const base = {
    id: "main",
    kind: "lyapunov",
    lyapunov: { transient: 1000, total: 100000, interval: 1, blocks: 10 },
    x_axis: { kind: "time" },
    y_axis: { kind: "exponent" },
    x_range: [1000, 101000],
    y_range: [-2, 1],
    slice: { states: {}, t: 0 },
    layers: {},
  };
  const file = (view) => ({
    format: "separatrix",
    format_version: 1,
    min_reader_version: 1,
    app_version: "",
    system: { kind: "map", input_format: "latex", lines: [], states: ["x", "y"], params: [], template: null },
    initial_conditions: [],
    views: [view],
    analyses: [],
    solver: {},
  });
  const open = (view) => model.applyProjectLoad(stateFromProject(file(view)), henon, () => 1);
  assert.deepEqual(open(base).warnings, []);
  assert.equal(open(base).view.kind, "lyapunov");
  for (const [why, view] of [
    ["no run", { ...base, lyapunov: undefined }],
    ["one block", { ...base, lyapunov: { ...base.lyapunov, blocks: 1 } }],
    ["half an iterate", { ...base, lyapunov: { ...base.lyapunov, interval: 0.5 } }],
    ["a state up the side", { ...base, y_axis: { kind: "state", name: "x" } }],
  ]) {
    const applied = open(view);
    assert.deepEqual(applied.warnings, [LYAPUNOV_MISFIT], why);
    assert.notEqual(applied.view.kind, "lyapunov", why);
  }
});
