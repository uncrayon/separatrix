// Node (>= 22.18) strips the types from the .ts modules.
// templates.json is read with an import attribute, because templates/index.ts
// imports it the Vite way, which plain Node does not accept.

import test from "node:test";
import assert from "node:assert/strict";
import {
  PALETTE_SIZE,
  UNTITLED_SYSTEM,
  leastUsedPaletteIndex,
  resolveParam,
  sameEquations,
  systemTitle,
} from "../state/model.ts";
import templates from "../templates/templates.json" with { type: "json" };

const ics = (...colors) => colors.map((color, i) => ({ id: i + 1, x0: [0, 0], t0: 0, color }));

function addMany(list, n) {
  const out = [];
  for (let k = 0; k < n; k++) {
    const color = leastUsedPaletteIndex(list);
    list = [...list, { id: 100 + k, x0: [0, 0], t0: 0, color }];
    out.push(color);
  }
  return { list, colors: out };
}

test("colours: the first trajectory is traj-1", () => {
  assert.equal(leastUsedPaletteIndex([]), 0);
});

test("colours: after clearing every trajectory, new ones start again at traj-1", () => {
  const first = addMany([], 3);
  assert.deepEqual(first.colors, [0, 1, 2]);
  const cleared = first.list.filter(() => false);
  assert.deepEqual(addMany(cleared, 3).colors, [0, 1, 2]);
});

test("colours: removing one trajectory frees its colour for the next one", () => {
  assert.equal(leastUsedPaletteIndex(ics(0, 2, 3)), 1);
  assert.equal(leastUsedPaletteIndex(ics(1, 2)), 0);
  assert.equal(leastUsedPaletteIndex(ics(0, 1, 2, 3, 4, 6)), 5);
});

test("colours: hidden trajectories keep their colour in use", () => {
  const list = ics(0, 1).map((ic, i) => ({ ...ic, hidden: i === 0 }));
  assert.equal(leastUsedPaletteIndex(list), 2);
});

test("colours: with every colour taken, the cycle carries on from traj-1", () => {
  const { colors } = addMany([], 2 * PALETTE_SIZE + 2);
  assert.deepEqual(colors, [0, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 6, 0, 1]);
  assert.equal(leastUsedPaletteIndex(ics(0, 1, 2, 2, 3, 4, 5, 6)), 0);
  assert.equal(leastUsedPaletteIndex(ics(0, 0, 1, 2, 3, 4, 5, 6)), 1);
});

test("colours: every result is a palette index", () => {
  for (const list of [ics(), ics(0, 1, 2, 3, 4, 5, 6), ics(7, 8, -1)]) {
    const c = leastUsedPaletteIndex(list);
    assert.ok(Number.isInteger(c) && c >= 0 && c < PALETTE_SIZE, `${c} for ${JSON.stringify(list)}`);
  }
});

const sheep = templates.find((t) => t.title.toLowerCase().includes("sheep")) ?? templates[0];

test("name: a template as loaded shows its title, not marked edited", () => {
  assert.deepEqual(systemTitle(sheep, sheep.equations, "latex"), { name: sheep.title, edited: false });
});

test("name: editing a template's equations keeps its title and marks it edited", () => {
  const lines = sheep.equations.slice();
  lines[0] = lines[0] + "+1";
  const title = systemTitle(sheep, lines, "latex");
  assert.equal(title.name, sheep.title);
  assert.notEqual(title.name, UNTITLED_SYSTEM);
  assert.equal(title.edited, true);
});

test("name: adding or removing an equation also counts as an edit", () => {
  assert.equal(systemTitle(sheep, [...sheep.equations, "\\dot{z}=z"], "latex").edited, true);
  assert.equal(systemTitle(sheep, sheep.equations.slice(1), "latex").edited, true);
});

test("name: undoing back to the template's text drops the mark", () => {
  const edited = sheep.equations.map((l, i) => (i === 0 ? l + "x" : l));
  assert.equal(systemTitle(sheep, edited, "latex").edited, true);
  assert.equal(systemTitle(sheep, edited.map((l, i) => (i === 0 ? sheep.equations[0] : l)), "latex").edited, false);
});

test("name: blank lines and spacing alone are not edits", () => {
  assert.equal(systemTitle(sheep, [...sheep.equations, "", "  "], "latex").edited, false);
  const spaced = sheep.equations.map((l) => " " + l.replace(/=/g, " = ") + " ");
  assert.equal(systemTitle(sheep, spaced, "latex").edited, false);
});

test("name: LaTeX spelling that cannot change the meaning is not an edit", () => {
  assert.equal(sameEquations(["\\dot{x}=x\\left(3-x-2y\\right)"], ["\\dot{x}=x(3-x-2y)"], "latex"), true);
  assert.equal(sameEquations(["x_{n+1}=y_n+1-ax_n^2"], ["x_{n+1}=y_n+1-ax_n^{2}"], "latex"), true);
  assert.equal(sameEquations(["\\dot{x}=x^2"], ["\\dot{x}=x^{3}"], "latex"), false);
  assert.equal(sameEquations(["\\dot{x}=x^{\\alpha}y"], ["\\dot{x}=x^\\alphay"], "latex"), false);
  assert.equal(sameEquations(["\\dot{x}=e^{2t}"], ["\\dot{x}=e^2t"], "latex"), false);
});

test("name: a space that ends a control word still matters", () => {
  assert.equal(sameEquations(["\\dot{x}=\\alpha x"], ["\\dot{x} = \\alpha  x"], "latex"), true);
  assert.equal(sameEquations(["\\dot{x}=\\alpha x"], ["\\dot{x}=\\alphax"], "latex"), false);
});

test("name: a blank system stays Untitled system, edited or not", () => {
  assert.deepEqual(systemTitle(undefined, ["\\dot{x}=y", "\\dot{y}=-x"], "latex"), { name: "Untitled system", edited: false });
  assert.deepEqual(systemTitle(undefined, ["\\dot{x}=y", "\\dot{y}=-x-y"], "latex"), { name: "Untitled system", edited: false });
});

test("name: every gallery template matches its own equations", () => {
  for (const t of templates) assert.equal(systemTitle(t, t.equations, "latex").edited, false, t.id);
});

const lorenz = templates.find((t) => t.id === "lorenz");
const param = (name, value, min = 0, max = 20) => ({ name, latex: name, value, min, max });
const info = (name, def) => ({ name, latex: name, default: def });

test("parameters: a default line added to a template follows, while the slider is where the template put it", () => {
  const p = resolveParam(info("sigma", 15), param("sigma", lorenz.params.sigma), 1, undefined, lorenz);
  assert.equal(p.value, 15);
});

test("parameters: a template's value survives an edit that leaves the default alone", () => {
  const p = resolveParam(info("sigma", 1), param("sigma", lorenz.params.sigma), 1, undefined, lorenz);
  assert.equal(p.value, lorenz.params.sigma);
});

test("parameters: a slider the user moved keeps its value when the default line changes", () => {
  assert.equal(resolveParam(info("sigma", 15), param("sigma", 12), 1, undefined, lorenz).value, 12);
  assert.equal(resolveParam(info("a", 3), param("a", 5), 2, undefined, undefined).value, 5);
});

test("parameters: a slider still at the old default follows the edited default line", () => {
  assert.equal(resolveParam(info("a", 3), param("a", 2), 2, undefined, undefined).value, 3);
});
