// Node (>= 22.18) strips the types from the .ts modules; the app's
// extensionless imports resolve to .ts through a module hook.

import test from "node:test";
import assert from "node:assert/strict";
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

const { METHODS, STIFF_HINT, STIFF_METHOD, methodFits } = await import("../state/methods.ts");
const { looksStiff } = await import("../components/runStatus.ts");
const { KNOWN_METHODS, stateFromProject } = await import("../project/serialize.ts");

const traj = (extra = {}) => ({
  t: [0, 1],
  x: [1, 0, 0.5, 0.1],
  status: "completed",
  stats: { accepted: 10, rejected: 0, evals: 70 },
  ...extra,
});
const portrait = (...trajectories) => ({ trajectories });

test("Rosenbrock 2(3) is an adaptive method that runs any flow", () => {
  assert.deepEqual(
    { label: METHODS.rosenbrock23.label, adaptive: METHODS.rosenbrock23.adaptive, needs: METHODS.rosenbrock23.needs },
    { label: "Rosenbrock 2(3)", adaptive: true, needs: "any" },
  );
  assert.equal(KNOWN_METHODS.length, 7);
  assert.ok(KNOWN_METHODS.includes("rosenbrock23"));
  for (const sys of [
    { kind: "ode", hamiltonian: false, separable: false },
    { kind: "ode", hamiltonian: true, separable: true },
  ]) {
    assert.equal(methodFits("rosenbrock23", sys), true);
  }
  assert.equal(STIFF_METHOD, "rosenbrock23");
  assert.equal(STIFF_HINT, "These steps look stiff: Dormand–Prince is limited by stability, not accuracy.");
});

test("the hint shows when a trajectory came back stiff, until Rosenbrock 2(3) is chosen", () => {
  assert.equal(looksStiff(portrait(traj(), traj({ stiff: true })), "dopri5"), true);
  assert.equal(looksStiff(portrait(traj(), traj()), "dopri5"), false);
  assert.equal(looksStiff(portrait(), "dopri5"), false);
  assert.equal(
    looksStiff(portrait(traj({ stiff: true })), "rosenbrock23"),
    false,
    "the stale Dormand–Prince portrait while Rosenbrock's is on its way",
  );
});

test("a project saved with Rosenbrock 2(3) opens with it and no notice", () => {
  const load = stateFromProject({
    format: "separatrix",
    format_version: 1,
    min_reader_version: 1,
    system: { format: "text", lines: ["x' = mu*(y - x^3/3 + x)", "y' = -x/mu"], states: ["x", "y"] },
    solver: { method: "rosenbrock23", rtol: 1e-6, atol: 1e-9 },
  });
  assert.deepEqual(load.warnings, []);
  assert.equal(load.solver.method, "rosenbrock23");
});
