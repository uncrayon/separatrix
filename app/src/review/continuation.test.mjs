// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports resolve to .ts through a module hook.

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { registerHooks } from "node:module";

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
const { projectFromState, stateFromProject } = await import("../project/serialize.ts");
const { nearestBranchVertex, stabilityRuns } = await import("../canvas/branch.ts");
const { makeViewport, sx, sy } = await import("../canvas/viewport.ts");

const here = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE = JSON.parse(
  fs.readFileSync(path.join(here, "../../../crates/project/tests/fixtures/continuation-view.sepx"), "utf8"),
);

const st = (name) => ({ name, latex: name, derived: false });
const system = (kind, states, params, extra = {}) => ({
  ok: true,
  systemId: 7,
  kind,
  hamiltonian: false,
  separable: false,
  autonomous: true,
  states: states.map(st),
  params: params.map((name) => ({ name, latex: name, default: 1 })),
  equationsLatex: [],
  diagnostics: [],
  ...extra,
});
const logistic = system("map", ["x"], ["r"]);
const selkov = system("ode", ["x", "y"], ["a", "b"]);
const sliders = (sys, values) => sys.params.map((p, i) => ({ name: p.name, latex: p.latex, value: values[i], min: 0, max: 4 }));

test("the Branch view is the last choice of a flow and a map, offered with a parameter to vary", () => {
  assert.equal(model.viewChoicesFor(selkov).at(-1), "continuation");
  assert.equal(model.viewChoicesFor(logistic).at(-1), "continuation");
  assert.equal(model.VIEW_CHOICE_LABELS.continuation, "Continuation");
  assert.equal(model.VIEW_CHOICE_SHORT.continuation, "Branch");
  assert.deepEqual(model.continuationAvailability(logistic), { ok: true });
  assert.deepEqual(model.continuationAvailability(system("ode", ["x"], []), "Pendulum"), {
    ok: false,
    reason: "Continuation follows a fixed point as a parameter changes; Pendulum has no parameter.",
  });
  assert.equal(model.continuationAvailability({ ...selkov, autonomous: false }).ok, false);
  assert.equal(model.viewChoice(model.continuationView([{ min: 0, max: 4 }], [0, 1])), "continuation");
});

test("one job per seed; the swept slider is not in any key, the other sliders are", () => {
  const seeds = [model.pointSeed([0.6, 1.36], 0.6, "both"), model.pointSeed([0.1, 1], 0.2, "forward")];
  const view = model.continuationView(sliders(selkov, [0.08, 0.6]), [0, 3], 1, seeds);
  const snap = (values) => ({ compiled: selkov, params: sliders(selkov, values), view });
  const jobs = model.buildContinuationJobs(snap([0.08, 0.6]));
  assert.equal(jobs.length, 2);
  assert.deepEqual(jobs[0].body.params, [0.08, 0.6], "the swept b is the seed's own");
  assert.deepEqual(jobs[1].body.params, [0.08, 0.2]);
  assert.deepEqual(jobs[0].body.start, { kind: "point", x0: [0.6, 1.36], period: 1 });
  assert.equal(jobs[0].body.dsMax, 2 * jobs[0].body.ds);
  const keys = (values) => model.buildContinuationJobs(snap(values)).map((j) => j.meta.key);
  assert.deepEqual(keys([0.08, 1.2]), keys([0.08, 0.6]), "moving b asks for nothing");
  assert.notDeepEqual(keys([0.09, 0.6]), keys([0.08, 0.6]), "moving a asks again");
  assert.deepEqual(model.buildContinuationJobs({ ...snap([0.08, 0.6]), compiled: logistic }), [], "a seed of another system");
  const bad = { ...view, continuation: { ...view.continuation, ds: 0 } };
  assert.deepEqual(model.buildContinuationJobs({ ...snap([0.08, 0.6]), view: bad }), [], "Δs must be positive");
});

test("a switch seed keeps what the switch needs of the special point, followed both ways, and is added once", () => {
  const at = { kind: "flip", index: 12, p: 3, x: [2 / 3], period: 1, tangent: [0.1, 0.99], eigenvalues: [[-1, 0]], omega: null, lyapunov: null };
  const seeds = model.withSeed([], model.switchSeed(at));
  assert.deepEqual(model.withSeed(seeds, model.switchSeed(at)), seeds);
  assert.equal(model.switchedBranch(seeds, at), 1);
  assert.equal(model.switchedBranch(seeds, { ...at, p: 3.449 }), null);
  const [job] = model.buildContinuationJobs({
    compiled: logistic,
    params: sliders(logistic, [3.7]),
    view: model.continuationView([{ min: 0, max: 4 }], [0, 1], 0, seeds),
  });
  assert.deepEqual(job.body.start, { kind: "switch", at: { kind: "flip", p: 3, x: [2 / 3], period: 1, tangent: [0.1, 0.99] } });
  assert.equal(job.body.direction, "both");
  assert.deepEqual(job.body.params, [3]);
});

test("nearest-vertex picking returns the vertex's exact p, any cycle member", () => {
  const vp = makeViewport(800, 600, [2.8, 3.6], [0.3, 1]);
  const branch = {
    p: [3.1, 3.2360679774997896, 3.3],
    x: [0.5, 0.8, 0.513, 0.799, 0.52, 0.81],
    period: 2,
    stable: [true, true, true],
    unstable: [0, 0, 0],
    special: [],
  };
  const shown = [{ branch: { ...branch, p: [2.9, 3.0], x: [0.65, 0.65, 0.66, 0.66], stable: [true, false], unstable: [0, 1] }, color: 0 }, { branch, color: 1 }];
  const axis = { kind: "state", index: 0 };
  const hit = nearestBranchVertex(vp, shown, 1, axis, sx(vp, 3.2360679774997896) + 2, sy(vp, 0.799) - 3);
  assert.deepEqual(hit, { branch: 1, index: 1, p: 3.2360679774997896 }, "the second cycle member of branch 2");
  assert.equal(nearestBranchVertex(vp, shown, 1, axis, sx(vp, 3.45), sy(vp, 0.4)), null, "nothing within 12 px");
});

test("stable stretches are solid and unstable dashed, split at every special point", () => {
  const run = (stable, unstable, special) => stabilityRuns({ stable, unstable, special: special.map((index) => ({ index })) });
  assert.deepEqual(run([true, true, false, false, false], [0, 0, 0, 1, 1], [2]), [
    { from: 0, to: 2, stable: true },
    { from: 2, to: 4, stable: false },
  ]);
  assert.deepEqual(
    run([true, false, true, true], [0, 0, 0, 0], [1]),
    [
      { from: 0, to: 1, stable: true },
      { from: 1, to: 3, stable: true },
    ],
    "a special point between two stable stretches still splits them",
  );
  assert.deepEqual(
    run([false, false, true, true], [1, 0, 0, 0], []),
    [
      { from: 0, to: 1, stable: false },
      { from: 1, to: 3, stable: true },
    ],
    "a neutral switch start takes its neighbour's stability",
  );
});

test("the special-point table drives every label; each bifurcation is listed once, by p", () => {
  assert.deepEqual(
    Object.values(model.SPECIAL_KINDS).map((k) => k.label),
    ["LP", "H", "BP", "PD", "NS"],
  );
  assert.deepEqual(
    Object.entries(model.SPECIAL_KINDS).filter(([, k]) => k.switches).map(([kind]) => kind),
    ["branchPoint", "flip"],
  );
  const sp = (kind, p, period = 1) => ({ kind, index: 0, p, x: [0], period, tangent: [0, 1], eigenvalues: [], omega: null, lyapunov: null });
  const branch = (special) => ({ ok: true, roundTripMs: 1, res: { requestId: 1, elapsedMs: 2, branch: { period: 1, p: [0, 1], x: [0, 0], stable: [], unstable: [], special, end: { first: null, last: "range" } } } });
  const rows = model.specialRows([
    branch([sp("flip", 3), sp("branchPoint", 1)]),
    undefined,
    branch([sp("flip", 3.449489742783178, 2), sp("flip", 3.449489742783179, 2), sp("branchPoint", 1 + 1e-12)]),
  ]);
  assert.deepEqual(rows.map((r) => [r.at.kind, r.at.p]), [["branchPoint", 1], ["flip", 3], ["flip", 3.449489742783178]]);
  assert.deepEqual(model.continuationSummary([branch([]), { ok: false, error: "x" }]), { elapsedMs: 2, roundTripMs: 1, points: 2, special: 0 });
});

function documentOf(load, sys) {
  const applied = model.applyProjectLoad(load, sys, (() => {
    let id = 1;
    return () => id++;
  })());
  return {
    applied,
    doc: {
      format: load.format,
      equations: load.lines.map((source, id) => ({ id, source })),
      compiled: sys,
      params: applied.params,
      view: applied.view,
      baseState: applied.baseState,
      fieldTime: applied.fieldTime,
      ics: applied.ics,
      layers: applied.layers,
      solver: applied.solver,
      sourceTemplateId: load.templateId,
    },
  };
}

test("the seeds of continuation-view.sepx round-trip through the studio", () => {
  const { applied, doc } = documentOf(stateFromProject(FIXTURE), logistic);
  assert.deepEqual(applied.warnings, []);
  const view = applied.view;
  assert.equal(view.kind, "continuation");
  assert.deepEqual(view.continuation.range, [2.8, 3.57]);
  assert.deepEqual(view.continuation.seeds.map((s) => s.from), ["point", "switch", "switch", "switch"]);
  assert.deepEqual(view.continuation.seeds.slice(1).map((s) => s.at.period), [1, 2, 4]);
  const loaded = view.continuation.seeds[1];
  const special = { ...loaded.at, index: 7, eigenvalues: [[-1, 0]], omega: null, lyapunov: null };
  assert.deepEqual(model.withSeed(view.continuation.seeds, model.switchSeed(special)), view.continuation.seeds, "a loaded switch is the switch at that point");
  const { slice: _slice, ...saved } = projectFromState(doc, { loaded: FIXTURE }).views[0];
  const { slice: _fileSlice, ...file } = FIXTURE.views[0];
  assert.deepEqual(saved, file, "every key but the slice, which the studio fills in for x");

  const norm = projectFromState({ ...doc, view: { ...view, yAxis: { kind: "norm" } } }, { loaded: FIXTURE }).views[0];
  assert.deepEqual(norm.y_axis, { kind: "norm" });
  assert.deepEqual(stateFromProject({ ...FIXTURE, views: [norm] }).view.yAxis, { kind: "norm" });
});

test("a seed that doesn't fit is left out; a view that can't run gives the default view", () => {
  const withView = (patch) => {
    const v = structuredClone(FIXTURE.views[0]);
    patch(v);
    return model.applyProjectLoad(stateFromProject({ ...FIXTURE, views: [v] }), logistic, () => 1);
  };
  const short = withView((v) => (v.continuation.seeds[2].tangent = [1]));
  assert.deepEqual(short.warnings, ["The saved continuation's branch 3 doesn't fit this system, so it was left out."]);
  assert.equal(short.view.continuation.seeds.length, 3);
  const newer = withView((v) => (v.continuation.seeds[0].from = "cycle"));
  assert.deepEqual(newer.warnings, [model.continuationSeedMisfit(1)]);
  const saved = projectFromState(
    { ...documentOf(stateFromProject(FIXTURE), logistic).doc, view: newer.view },
    { loaded: { ...FIXTURE, views: [{ ...FIXTURE.views[0], continuation: { ...FIXTURE.views[0].continuation, seeds: [{ ...FIXTURE.views[0].continuation.seeds[0], from: "cycle" }, ...FIXTURE.views[0].continuation.seeds.slice(1)] } }] } },
  );
  assert.equal(saved.views[0].continuation.seeds.at(-1).from, "cycle", "a seed this version can't replay is kept, last");

  for (const [why, patch] of [
    ["a parameter it lacks", (v) => (v.x_axis.name = "mu")],
    ["an empty range", (v) => (v.continuation.range = [3, 3])],
    ["Δs not positive", (v) => (v.continuation.ds = 0)],
    ["max points too many", (v) => (v.continuation.max_points = 10 ** 6)],
  ]) {
    const applied = withView(patch);
    assert.deepEqual(applied.warnings, [model.CONTINUATION_MISFIT], why);
    assert.equal(model.CONTINUATION_MISFIT, "The saved continuation can't be run as saved, so the default view is shown.");
    assert.notEqual(applied.view.kind, "continuation", why);
  }
});
