// The method table and the symplectic methods' rules in the UI.

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { METHODS, energyDriftText, methodFits, symplecticRefusal } from "../state/methods.ts";
import { KNOWN_METHODS, stateFromProject } from "../project/serialize.ts";

const hamiltonian = { kind: "ode", hamiltonian: true, separable: true };
const coupled = { kind: "ode", hamiltonian: true, separable: false };
const flow = { kind: "ode", hamiltonian: false, separable: false };

test("the table lists every method the core knows, and only those", () => {
  const rust = readFileSync(new URL("../../../crates/project/src/lib.rs", import.meta.url), "utf8");
  const known = JSON.parse(/pub const KNOWN_METHODS: \[&str; \d+\] = (\[[^\]]*\]);/.exec(rust)[1].replace(/,\s*\]$/, "]"));
  assert.deepEqual(Object.keys(METHODS).sort(), [...known].sort());
  assert.deepEqual([...KNOWN_METHODS].sort(), [...known].sort());
  assert.deepEqual(
    Object.entries(METHODS)
      .filter(([, m]) => m.adaptive)
      .map(([k]) => k),
    ["dopri5", "rosenbrock23"],
  );
  assert.equal(METHODS.verlet.short, "Störmer–Verlet");
  assert.equal(METHODS.yoshida4.label, "Yoshida 4");
});

test("the symplectic methods need a separable Hamiltonian; the others run anything", () => {
  for (const m of ["verlet", "yoshida4"]) {
    assert.equal(methodFits(m, hamiltonian), true, m);
    assert.equal(methodFits(m, coupled), false, m);
    assert.equal(methodFits(m, flow), false, m);
  }
  for (const m of ["euler", "heun", "rk4", "dopri5", "rosenbrock23"]) {
    for (const sys of [hamiltonian, coupled, flow]) assert.equal(methodFits(m, sys), true, m);
  }
  assert.equal(symplecticRefusal(hamiltonian), null);
  assert.match(symplecticRefusal(coupled), /split as T\(p\) \+ V\(q\)/);
  assert.match(symplecticRefusal(flow), /need a Hamiltonian H\(q, p\); this system is given by its equations of motion/);
});

test("the energy readout has two significant digits and superscript powers", () => {
  assert.equal(energyDriftText(3.1e-7), "|ΔH| max = 3.1 × 10⁻⁷");
  assert.equal(energyDriftText(2.653e-4), "|ΔH| max = 2.7 × 10⁻⁴");
  assert.equal(energyDriftText(0.0123), "|ΔH| max = 1.2 × 10⁻²");
  assert.equal(energyDriftText(12), "|ΔH| max = 1.2 × 10¹");
  assert.equal(energyDriftText(0), "|ΔH| max = 0");
  assert.doesNotMatch(energyDriftText(4.4e-12), /-/);
});

test("a project saved with a symplectic method opens with it and no notice", () => {
  for (const method of ["verlet", "yoshida4"]) {
    const load = stateFromProject({
      format: "separatrix",
      format_version: 1,
      min_reader_version: 1,
      system: { format: "text", lines: ["H(q, p) = p^2/2 - cos(q)"], states: ["q", "p"] },
      solver: { method, h: 0.05 },
    });
    assert.deepEqual(load.warnings, [], method);
    assert.equal(load.solver.method, method);
    assert.equal(load.solver.h, 0.05);
  }
});
