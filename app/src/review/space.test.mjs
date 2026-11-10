// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports resolve to .ts through a module hook.

import test from "node:test";
import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { OrthographicCamera, Vector3 } from "three";

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
const { SECTION_MISFIT, SPACE_MISFIT } = await import("../api/ipc.generated.ts");
const space = await import("../canvas/space.ts");
const { projectFromState, stateFromProject } = await import("../project/serialize.ts");

const st = (name) => ({ name, latex: name, derived: false });
const flow = (names) => ({
  ok: true,
  systemId: 7,
  kind: "ode",
  hamiltonian: false,
  autonomous: true,
  states: names.map(st),
  params: [],
  equationsLatex: [],
  diagnostics: [],
});
const lorenz = flow(["x", "y", "z"]);
const S = (i) => ({ kind: "state", index: i });

/** A seeded uniform generator (mulberry32), so a failure reruns the same points. */
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function threeCameraFor(camera, w, h) {
  const t = space.threeCamera(camera, w, h);
  const cam = new OrthographicCamera();
  cam.position.set(...t.position);
  cam.up.set(...t.up);
  cam.lookAt(0, 0, 0);
  Object.assign(cam, { left: t.left, right: t.right, top: t.top, bottom: t.bottom, near: t.near, far: t.far });
  cam.updateProjectionMatrix();
  cam.updateMatrixWorld();
  return cam;
}

test("three.js projects the WebGL lines exactly where the overlay draws: 200 points, 10 cameras", () => {
  const rand = rng(20261105);
  const [w, h] = [913, 641];
  const box = { xAxis: S(2), yAxis: S(0), zAxis: S(3), xRange: [-3, 7], yRange: [100, 100.5], zRange: [-1e3, 2e3] };
  const cameras = [
    space.DEFAULT_CAMERA,
    { yaw: 0, pitch: 0, zoom: 1 },
    { yaw: Math.PI / 2, pitch: space.PITCH_LIMIT, zoom: 0.25 },
    { yaw: -Math.PI, pitch: -space.PITCH_LIMIT, zoom: 8 },
    ...Array.from({ length: 6 }, () =>
      space.clampCamera({ yaw: (rand() * 2 - 1) * Math.PI, pitch: (rand() * 2 - 1) * 1.5, zoom: 0.25 + rand() * 7.75 }),
    ),
  ];
  const points = Array.from({ length: 200 }, () => [
    100 + rand() - 0.25,
    rand() * 50 - 20,
    rand() * 14 - 5,
    rand() * 4e3 - 1.5e3,
  ]);
  let worst = 0;
  for (const camera of cameras) {
    const view = { ...box, camera };
    const project = space.spaceProjector(view, w, h);
    const toBox = space.boxCoords(view);
    const cam = threeCameraFor(camera, w, h);
    for (const p of points) {
      const ndc = new Vector3(...toBox(p)).project(cam);
      const three = [((ndc.x + 1) / 2) * w, ((1 - ndc.y) / 2) * h, ndc.z];
      const ours = project(p);
      for (let k = 0; k < 3; k++) worst = Math.max(worst, Math.abs(three[k] - ours[k]));
    }
  }
  assert.ok(worst < 1e-9, `largest difference ${worst}`);
  const [, , near] = space.spaceProjector({ ...box, camera: space.DEFAULT_CAMERA }, w, h)([100.5, 0, 7, 2e3]);
  const [, , far] = space.spaceProjector({ ...box, camera: space.DEFAULT_CAMERA }, w, h)([100, 0, -3, -1e3]);
  assert.ok(near < far, `${near} < ${far}`);
});

test("the camera stays within its limits", () => {
  const lim = Math.PI / 2 - 0.01;
  assert.equal(space.clampCamera({ yaw: 0, pitch: 3, zoom: 1 }).pitch, lim);
  assert.equal(space.clampCamera({ yaw: 0, pitch: -3, zoom: 1 }).pitch, -lim);
  assert.equal(space.clampCamera({ yaw: 0, pitch: 0.4, zoom: 1 }).pitch, 0.4);
  assert.equal(space.clampCamera({ yaw: 0, pitch: 0, zoom: 100 }).zoom, 8);
  assert.equal(space.clampCamera({ yaw: 0, pitch: 0, zoom: 0.01 }).zoom, 0.25);
  assert.equal(space.clampCamera({ yaw: 0, pitch: 0, zoom: 2 }).zoom, 2);
  assert.equal(space.clampCamera({ yaw: -Math.PI / 4, pitch: 0, zoom: 1 }).yaw, -Math.PI / 4);
  assert.ok(Math.abs(space.clampCamera({ yaw: 2 * Math.PI + 0.5, pitch: 0, zoom: 1 }).yaw - 0.5) < 1e-12);
  assert.deepEqual(space.clampCamera({ yaw: NaN, pitch: Infinity }), space.DEFAULT_CAMERA);
  assert.deepEqual(space.DEFAULT_CAMERA, { yaw: -Math.PI / 4, pitch: Math.PI / 6, zoom: 1 });
});

test("a flow of three states or more offers the 3D view; others say why not", () => {
  assert.deepEqual(model.viewChoicesFor(lorenz), ["phase", "series", "section", "space", "lyapunov", "continuation", "curves"]);
  assert.equal(model.VIEW_CHOICE_SHORT.space, "3D");
  assert.deepEqual(model.spaceAvailability(lorenz), { ok: true });
  assert.deepEqual(model.viewChoiceAvailability(flow(["x", "y"]), "Van der Pol oscillator").space, {
    ok: false,
    reason: "The 3D view needs a flow with at least 3 states. Van der Pol oscillator has 2.",
  });
  assert.deepEqual(model.spaceAvailability({ ...lorenz, kind: "map" }, "Hénon map").ok, false);
});

test("switching to 3D shows states 0, 1 and 2 over their extents, and the camera asks the core for nothing", () => {
  const portrait = {
    res: { trajectories: [{ t: [0, 1], x: [-10, -20, 5, 10, 20, 45], status: "completed" }], fixedPoints: [] },
    meta: { systemId: 7, shapeId: 1, trajectories: [], view: model.defaultView(lorenz), fixedPoints: true },
  };
  const memory = {};
  const v = model.switchView("space", { compiled: lorenz, view: model.defaultView(lorenz), solver: model.DEFAULT_SOLVER, portrait }, memory);
  assert.equal(v.kind, "space");
  assert.deepEqual([v.xAxis, v.yAxis, v.zAxis], [S(0), S(1), S(2)]);
  assert.deepEqual([v.xRange, v.yRange, v.zRange], [[-12, 12], [-24, 24], [1, 49]]);
  assert.deepEqual(v.camera, space.DEFAULT_CAMERA);
  assert.equal(model.switchView("space", { compiled: flow(["x", "y"]), view: model.defaultView(flow(["x", "y"])), solver: model.DEFAULT_SOLVER }, {}), null);

  const snapshot = (view) => ({
    compiled: lorenz,
    params: [],
    view,
    baseState: [0, 0, 0],
    fieldTime: 0,
    ics: [{ id: 1, x0: [0, 1, 0], t0: 0, color: 0 }],
    layers: model.DEFAULT_LAYERS,
    solver: { ...model.DEFAULT_SOLVER, tEnd: 50 },
    plotAspect: 1.6,
    shapeId: 1,
  });
  const job = model.buildPortraitJob(snapshot(v));
  assert.deepEqual(job.body.zAxis, S(2));
  assert.deepEqual(job.body.zRange, [1, 49]);
  assert.deepEqual([job.body.xRange, job.body.yRange], [[-12, 12], [-24, 24]]);
  assert.equal(job.body.fieldGrid, 0);
  assert.equal(job.body.nullclines, false);
  assert.equal(job.body.fixedPoints, true);
  assert.deepEqual(job.body.trajectories, [
    { x0: [0, 1, 0], t0: 0, t1: 50 },
    { x0: [0, 1, 0], t0: 0, t1: -50 },
  ]);
  const turned = { ...v, camera: { yaw: 1, pitch: 0.2, zoom: 3 } };
  assert.deepEqual(model.buildPortraitJob(snapshot(turned)).body, job.body);
  assert.equal(model.viewRequestKey(turned), model.viewRequestKey(v));
  assert.notEqual(model.viewRequestKey({ ...v, zRange: [0, 50] }), model.viewRequestKey(v));
  assert.equal(model.canAddOnPlot(v), false);
});

test("picking a state already on another 3D axis swaps the two, ranges and all", () => {
  const four = flow(["x", "y", "z", "w"]);
  const v = model.spaceView([0, 1, 2], (i) => [[-1, 1], [-2, 2], [0, 50]][i]);
  const swapped = model.withSpaceAxis(v, 0, 2, () => assert.fail("a state on the view keeps its range"));
  assert.deepEqual([swapped.xAxis, swapped.yAxis, swapped.zAxis], [S(2), S(1), S(0)]);
  assert.deepEqual([swapped.xRange, swapped.yRange, swapped.zRange], [[0, 50], [-2, 2], [-1, 1]]);
  const fresh = model.withSpaceAxis(v, 1, 3, (i) => (i === 3 ? [5, 6] : assert.fail()));
  assert.deepEqual([fresh.xAxis, fresh.yAxis, fresh.zAxis], [S(0), S(3), S(2)]);
  assert.deepEqual(fresh.yRange, [5, 6]);
  assert.equal(model.withSpaceAxis(v, 2, 2, () => [0, 1]), v);
  assert.deepEqual(model.adaptView(fresh, four.states, [st("w"), ...lorenz.states], four).yAxis, S(0));
  assert.equal(model.adaptView(v, lorenz.states, [st("x"), st("y")], flow(["x", "y"])), null);
});

function documentOf(sys, view) {
  return {
    format: "latex",
    equations: sys.states.map((s, i) => ({ id: i + 1, source: `\\dot{${s.name}}=0` })),
    compiled: sys,
    params: [],
    view,
    baseState: sys.states.map(() => 0),
    fieldTime: 0,
    ics: [{ id: 1, uid: "ic-1", x0: sys.states.map((_, i) => i + 0.5), t0: 0, color: 0 }],
    layers: model.DEFAULT_LAYERS,
    solver: { ...model.DEFAULT_SOLVER, tEnd: 50 },
    sourceTemplateId: null,
  };
}

test("a 3D view round-trips through .sepx identically, camera included", () => {
  const view = model.spaceView([2, 0, 1], (i) => [[-20, 20], [-25, 25], [0, 50]][i], { yaw: 0.3, pitch: -0.2, zoom: 2.5 });
  const doc = JSON.parse(JSON.stringify(projectFromState(documentOf(lorenz, view))));
  const saved = doc.views[0];
  assert.equal(saved.kind, "space");
  assert.deepEqual([saved.x_axis, saved.y_axis, saved.z_axis], [
    { kind: "state", name: "z" },
    { kind: "state", name: "x" },
    { kind: "state", name: "y" },
  ]);
  assert.deepEqual(saved.z_range, [-25, 25]);
  assert.deepEqual(saved.camera, { yaw: 0.3, pitch: -0.2, zoom: 2.5 });
  let id = 1;
  const applied = model.applyProjectLoad(stateFromProject(doc), lorenz, () => id++);
  assert.deepEqual(applied.warnings, []);
  assert.deepEqual(applied.view, view);
  const again = projectFromState({ ...documentOf(lorenz, applied.view), ics: applied.ics }, { loaded: doc });
  assert.equal(JSON.stringify(again), JSON.stringify(doc));

  const back = projectFromState(documentOf(lorenz, model.defaultView(lorenz)), { loaded: doc });
  for (const k of ["kind", "z_axis", "z_range", "camera"]) assert.equal(k in back.views[0], false, k);

  const open = (edit, sys = lorenz) => {
    const d = JSON.parse(JSON.stringify(doc));
    edit(d.views[0]);
    return model.applyProjectLoad(stateFromProject(d), sys, () => 1);
  };
  assert.deepEqual(open((v) => (v.camera = { yaw: 0, pitch: 9, zoom: 99 })).view.camera, { yaw: 0, pitch: Math.PI / 2 - 0.01, zoom: 8 });
  assert.deepEqual(open((v) => delete v.camera).view.camera, space.DEFAULT_CAMERA);
  const two = flow(["x", "y"]);
  const misfit = open(() => {}, two);
  assert.deepEqual(misfit.warnings, ["The saved 3D view doesn't fit this system, so the default view is shown."]);
  assert.deepEqual(misfit.view, model.defaultView(two));
  assert.deepEqual(open((v) => (v.z_axis = { kind: "state", name: "x" })).warnings, [SPACE_MISFIT]);
});
