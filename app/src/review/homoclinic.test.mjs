// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports (a file or a folder's index.ts) and its JSON imports
// resolve through module hooks.

import test from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";

registerHooks({
  resolve(specifier, context, next) {
    try {
      return next(specifier, context);
    } catch (err) {
      if (!/^\.\.?\//.test(specifier) || /\.[cm]?[jt]sx?$|\.json$/.test(specifier)) throw err;
      try {
        return next(`${specifier}.ts`, context);
      } catch {
        return next(`${specifier}/index.ts`, context);
      }
    }
  },
  load(url, context, next) {
    if (url.endsWith(".json")) return next(url, { ...context, importAttributes: { type: "json" } });
    return next(url, context);
  },
});


const model = await import("../state/model.ts");
const curves = await import("../state/curves.ts");
const hom = await import("../state/homoclinic.ts");
const { mockHomoclinic, mockCancelJob } = await import("../api/mock.ts");

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
const freire = system("ode", ["x", "y", "z"], ["nu", "beta"]);
const map = system("map", ["x"], ["nu", "beta"]);
const sliders = [
  { name: "nu", latex: "nu", value: -0.7, min: -1, max: 0 },
  { name: "beta", latex: "beta", value: 0.5, min: 0, max: 1 },
];
const names = ["ν", "β"];

const planeWith = (homoclinic, patch = {}) => {
  const base = curves.curvesView(sliders, 0, 1, null, model.defaultView(freire));
  return { ...base, curves: { ...base.curves, homoclinic, ...patch } };
};
const BT = hom.homoclinicSeed([0, 0, 0], [-0.7745967, 0.7745967], [0, 1], [-0.7745967, 0.7745967]);
const jobsOf = (view, compiled = freire, params = sliders) => hom.buildHomoclinicJobs({ compiled, params, view });

test("request ids differ in a row and are safe integers, even within one millisecond", () => {
  const a = hom.homoclinicRequestId();
  const b = hom.homoclinicRequestId();
  assert.notEqual(a, b);
  assert.ok(Number.isSafeInteger(a) && Number.isSafeInteger(b));
  const same = [hom.homoclinicRequestId(1.8e12), hom.homoclinicRequestId(1.8e12), hom.homoclinicRequestId(1.8e12)];
  assert.equal(new Set(same).size, 3);
  assert.ok(Number.isSafeInteger(hom.homoclinicRequestId(9e12 - 1)));
});

test("a seed keeps every parameter's value at its BT point, whichever two are free", () => {
  assert.deepEqual(hom.homoclinicSeed([1, 2, 3], [0.1, 0.2, 0.3], [0, 2], [5, 6]), { x: [1, 2, 3], params: [5, 0.2, 6] });
  const seeds = hom.withHomoclinicSeed([BT], BT);
  assert.equal(seeds.length, 1, "the same point twice is one seed");
});

test("one job per seed on a flow, keyed by the body in a fixed property order", () => {
  const jobs = jobsOf(planeWith([BT]));
  assert.equal(jobs.length, 1);
  const { body, meta } = jobs[0];
  assert.deepEqual(Object.keys(body), ["systemId", "params", "free", "ranges", "x", "direction", "ds", "dsMax", "maxPoints"]);
  assert.equal(meta.key, JSON.stringify(body));
  assert.deepEqual(body.params, [-0.7745967, 0.7745967]);
  assert.deepEqual(body.free, [0, 1]);
  assert.deepEqual(body.ranges, [[-1, 0], [0, 1]]);
  assert.equal(body.direction, "both");
  assert.equal(body.ds, 0.01);
  assert.equal(body.dsMax, 0.02);
  assert.equal(body.maxPoints, 500);
  assert.ok(body.maxPoints <= 2000);
  assert.deepEqual(jobsOf(planeWith([BT, BT]))[0].meta.key, meta.key);
});

test("the request changes, and so does the key, with the box, the step and the sliders it does not free", () => {
  const key = (view, params) => jobsOf(view, freire, params)[0].meta.key;
  const base = key(planeWith([BT]), sliders);
  assert.notEqual(base, key(planeWith([BT], { ranges: [[-1, 0.1], [0, 1]] }), sliders));
  assert.notEqual(base, key(planeWith([BT], { ds: 0.005 }), sliders));
  const three = system("ode", ["x"], ["a", "b", "c"]);
  const v3 = curves.curvesView([{ min: 0, max: 1 }, { min: 0, max: 1 }, { min: 0, max: 1 }], 0, 1, null, model.defaultView(three));
  const view3 = { ...v3, curves: { ...v3.curves, homoclinic: [{ x: [0], params: [0.1, 0.2, 0.3] }] } };
  const at = (c) => [{ value: 9 }, { value: 9 }, { value: c }];
  const k1 = hom.buildHomoclinicJobs({ compiled: three, params: at(0.3), view: view3 })[0];
  const k2 = hom.buildHomoclinicJobs({ compiled: three, params: at(0.4), view: view3 })[0];
  assert.deepEqual(k1.body.params, [0.1, 0.2, 0.3]);
  assert.deepEqual(k2.body.params, [0.1, 0.2, 0.4]);
  assert.notEqual(k1.meta.key, k2.meta.key);
});

test("no job for a map, for another view, or for a seed of another shape", () => {
  assert.deepEqual(jobsOf(planeWith([BT]), map), []);
  assert.deepEqual(jobsOf(model.defaultView(freire)), []);
  assert.deepEqual(jobsOf(planeWith([{ x: [0], params: [1, 2] }])), []);
});

test("rows are computing with progress, done, or failed, each by its own key", () => {
  const view = planeWith([BT, hom.homoclinicSeed([0, 0, 0], [-0.75, 0.75], [0, 1], [-0.75, 0.75])]);
  const jobs = jobsOf(view);
  assert.equal(jobs.length, 2);
  const res = { requestId: 1, curve: { p1: [0, 1], p2: [0, 1] }, elapsedMs: 3 };
  const runs = { [jobs[0].meta.key]: { ok: true, res, roundTripMs: 4 } };
  const progress = { key: jobs[1].meta.key, points: 37, p: [-0.7213, 0.6] };
  let rows = hom.homoclinicRows(jobs, runs, progress);
  assert.deepEqual(rows.map((r) => r.status), ["done", "computing"]);
  assert.deepEqual(rows[1].progress, { points: 37, p: [-0.7213, 0.6] });
  assert.equal(rows[0].res, res);
  rows = hom.homoclinicRows(jobs, runs, { key: "other", points: 3, p: [0, 0] });
  assert.equal(rows[1].progress, null, "another job's progress is not this row's");
  rows = hom.homoclinicRows(jobs, runs, { key: jobs[1].meta.key, points: 0, p: null });
  assert.equal(rows[1].progress, null, "before the first message");
  rows = hom.homoclinicRows(jobs, { ...runs, [jobs[1].meta.key]: { ok: false, error: "No saddle." } }, null);
  assert.deepEqual(rows.map((r) => r.status), ["done", "failed"]);
  assert.equal(rows[1].error, "No saddle.");
  assert.deepEqual(hom.drawnHomoclinic(rows), [res.curve], "only the done row is drawn");
});

test("removing a seed leaves no job, no row and nothing to draw", () => {
  const jobs = jobsOf(planeWith([BT]));
  const runs = { [jobs[0].meta.key]: { ok: true, res: { curve: { p1: [], p2: [] } }, roundTripMs: 1 } };
  const after = planeWith([]);
  const rows = hom.homoclinicRows(jobsOf(after), runs, { key: jobs[0].meta.key, points: 5, p: [0, 0] });
  assert.deepEqual(rows, []);
  assert.deepEqual(hom.drawnHomoclinic(rows), []);
});

test("the running sentence names the points and the parameters", () => {
  assert.equal(hom.computingText(null, names), "Computing the homoclinic curve…");
  assert.equal(hom.computingText({ points: 37, p: [-0.72131, 0.6] }, names), "Computing the homoclinic curve: 37 points, at ν = −0.7213, β = 0.6000");
  assert.equal(hom.computingText({ points: 1, p: [0, 1] }, names), "Computing the homoclinic curve: 1 point, at ν = 0.0000, β = 1.0000");
});

test("the end sentences say what each end did", () => {
  const t = (first, last) => hom.homoclinicEndText({ first, last });
  assert.equal(t("shrunk", "range"), "one end shrinks onto the saddle at BT, the other runs to the edge of the box");
  assert.equal(t(null, "shrunk"), "the end shrinks onto the saddle at BT");
  assert.equal(t("range", "range"), "both ends run to the edge of the box");
  assert.equal(t("maxPoints", "stepTooSmall"), "one end stops at Max points, the other stops at a step too small to go on");
});

const curve = (extra = {}) => ({
  p1: [-0.8, -0.78, -0.76, -0.74],
  p2: [0.6, 0.62, 0.64, 0.66],
  sigma: [-0.2, -0.1, 0.3, 0.1],
  focus: [false, false, true, true],
  chaotic: [false, false, true, true],
  reversed: false,
  end: { first: "shrunk", last: "range" },
  ...extra,
});

test("σ changes sign where the linear interpolation puts it", () => {
  const c = curve();
  const [[a, b]] = hom.sigmaCrossings(c);
  assert.ok(Math.abs(a - -0.775) < 1e-12 && Math.abs(b - 0.625) < 1e-12, `${a} ${b}`);
  assert.equal(hom.sigmaText(c, names), "σ changes sign at ν = −0.7750, β = 0.6250");
  assert.equal(hom.sigmaText(curve({ sigma: [0.1, 0.2, 0.3, 0.4] }), names), "σ stays positive along the curve");
  assert.equal(hom.sigmaText(curve({ sigma: [-0.1, -0.2, -0.3, -0.4] }), names), "σ stays negative along the curve");
  const twice = curve({ sigma: [-1, 1, 1, -1] });
  assert.equal(hom.sigmaCrossings(twice).length, 2);
});

test("Shilnikov chaos is told as the runs of points that have it", () => {
  assert.equal(hom.chaosText(curve(), names), "Shilnikov chaos from ν = −0.7600, β = 0.6400 to ν = −0.7400, β = 0.6600");
  assert.equal(hom.chaosText(curve({ chaotic: [false, true, false, false] }), names), "Shilnikov chaos at ν = −0.7800, β = 0.6200");
  assert.equal(hom.chaosText(curve({ chaotic: [false, false, false, false] }), names), "No Shilnikov chaos along it");
});

test("a done row reads in four lines, the last only when time is reversed", () => {
  assert.deepEqual(hom.homoclinicLines(curve(), names), [
    "4 points · one end shrinks onto the saddle at BT, the other runs to the edge of the box",
    "σ changes sign at ν = −0.7750, β = 0.6250",
    "Shilnikov chaos from ν = −0.7600, β = 0.6400 to ν = −0.7400, β = 0.6600",
  ]);
  assert.equal(hom.homoclinicLines(curve({ reversed: true }), names).at(-1), "Followed in reversed time");
});

test("the preview refuses an unknown system with a sentence, and a cancel is idempotent", async () => {
  await assert.rejects(
    mockHomoclinic(
      { requestId: 1, systemId: 1_000_000, params: [0, 0], free: [0, 1], ranges: [[0, 1], [0, 1]], x: [0], direction: "both", ds: 0.01, dsMax: 0.02, maxPoints: 500 },
      () => {},
    ),
    /Unknown system/,
  );
  await mockCancelJob(1);
});
