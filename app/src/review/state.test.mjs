// Regression tests for the trajectory colour cycle and the system name shown
// in the top bar (pure helpers in app/src/state/model.ts).
//
// Plain JavaScript for the same reason as model.review.test.mjs: tsc never
// sees it, and Node (>= 22.18) strips the types from the .ts modules.
// templates.json is read with an import attribute, because templates/index.ts
// imports it the Vite way, which plain Node does not accept.
//
// Run from app/:   node --test src/review/

import test from "node:test";
import assert from "node:assert/strict";
import { PALETTE_SIZE, UNTITLED_SYSTEM, nextIcColor, sameEquations, systemTitle } from "../state/model.ts";
import templates from "../templates/templates.json" with { type: "json" };

const ics = (...colors) => colors.map((color, i) => ({ id: i + 1, x0: [0, 0], t0: 0, color }));

/** Add `n` trajectories one at a time, as clicks on the plot do; returns the new colours. */
function addMany(list, n) {
  const out = [];
  for (let k = 0; k < n; k++) {
    const color = nextIcColor(list);
    list = [...list, { id: 100 + k, x0: [0, 0], t0: 0, color }];
    out.push(color);
  }
  return { list, colors: out };
}

// ------------------------------------------------------------ colour cycle

test("colours: the first trajectory is traj-1", () => {
  assert.equal(nextIcColor([]), 0);
});

test("colours: after clearing every trajectory, new ones start again at traj-1", () => {
  // Three clicks, Clear all, three more clicks: traj-1, traj-2, traj-3 again
  // (it used to carry on with traj-4, traj-5, traj-6).
  const first = addMany([], 3);
  assert.deepEqual(first.colors, [0, 1, 2]);
  const cleared = first.list.filter(() => false);
  assert.deepEqual(addMany(cleared, 3).colors, [0, 1, 2]);
});

test("colours: removing one trajectory frees its colour for the next one", () => {
  assert.equal(nextIcColor(ics(0, 2, 3)), 1);
  assert.equal(nextIcColor(ics(1, 2)), 0);
  assert.equal(nextIcColor(ics(0, 1, 2, 3, 4, 6)), 5);
});

test("colours: hidden trajectories keep their colour in use", () => {
  const list = ics(0, 1).map((ic, i) => ({ ...ic, hidden: i === 0 }));
  assert.equal(nextIcColor(list), 2);
});

test("colours: with every colour taken, the cycle carries on from traj-1", () => {
  const { colors } = addMany([], 2 * PALETTE_SIZE + 2);
  assert.deepEqual(colors, [0, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 6, 0, 1]);
  // All in use once, traj-3 twice: the least-used colour, the first on a tie.
  assert.equal(nextIcColor(ics(0, 1, 2, 2, 3, 4, 5, 6)), 0);
  assert.equal(nextIcColor(ics(0, 0, 1, 2, 3, 4, 5, 6)), 1);
});

test("colours: every result is a palette index", () => {
  for (const list of [ics(), ics(0, 1, 2, 3, 4, 5, 6), ics(7, 8, -1)]) {
    const c = nextIcColor(list);
    assert.ok(Number.isInteger(c) && c >= 0 && c < PALETTE_SIZE, `${c} for ${JSON.stringify(list)}`);
  }
});

// ------------------------------------------------------------ system name

const sheep = templates.find((t) => t.title.toLowerCase().includes("sheep")) ?? templates[0];

test("name: a template as loaded shows its title, not marked edited", () => {
  assert.deepEqual(systemTitle(sheep, sheep.equations), { name: sheep.title, edited: false });
});

test("name: editing a template's equations keeps its title and marks it edited", () => {
  const lines = sheep.equations.slice();
  lines[0] = lines[0] + "+1";
  const title = systemTitle(sheep, lines);
  assert.equal(title.name, sheep.title);
  assert.notEqual(title.name, UNTITLED_SYSTEM);
  assert.equal(title.edited, true);
});

test("name: adding or removing an equation also counts as an edit", () => {
  assert.equal(systemTitle(sheep, [...sheep.equations, "\\dot{z}=z"]).edited, true);
  assert.equal(systemTitle(sheep, sheep.equations.slice(1)).edited, true);
});

test("name: undoing back to the template's text drops the mark", () => {
  const edited = sheep.equations.map((l, i) => (i === 0 ? l + "x" : l));
  assert.equal(systemTitle(sheep, edited).edited, true);
  assert.equal(systemTitle(sheep, edited.map((l, i) => (i === 0 ? sheep.equations[0] : l))).edited, false);
});

test("name: blank lines and spacing alone are not edits", () => {
  assert.equal(systemTitle(sheep, [...sheep.equations, "", "  "]).edited, false);
  const spaced = sheep.equations.map((l) => " " + l.replace(/=/g, " = ") + " ");
  assert.equal(systemTitle(sheep, spaced).edited, false);
});

test("name: retyping a template in text mode is not an edit", () => {
  // What the Text input produces for the template's lines (textToLatex of latexToText).
  assert.equal(sameEquations(["\\dot{x}=x\\left(3-x-2y\\right)"], ["\\dot{x}=x(3-x-2y)"]), true);
  assert.equal(sameEquations(["\\dot{y}=-x-\\mu\\left(x^2-1\\right)y"], ["\\dot{y}=-x-\\mu (x^{2}-1)y"]), true);
  assert.equal(sameEquations(["x_{n+1}=y_n+1-ax_n^2"], ["x_{n+1}=y_n+1-ax_n^{2}"]), true);
  // …but a different exponent, or a brace group of more than one token, still differs.
  assert.equal(sameEquations(["\\dot{x}=x^2"], ["\\dot{x}=x^{3}"]), false);
  assert.equal(sameEquations(["\\dot{x}=x^{\\alpha}y"], ["\\dot{x}=x^\\alphay"]), false);
  assert.equal(sameEquations(["\\dot{x}=e^{2t}"], ["\\dot{x}=e^2t"]), false);
});

test("name: a space that ends a control word still matters", () => {
  assert.equal(sameEquations(["\\dot{x}=\\alpha x"], ["\\dot{x} = \\alpha  x"]), true);
  assert.equal(sameEquations(["\\dot{x}=\\alpha x"], ["\\dot{x}=\\alphax"]), false);
});

test("name: a blank system stays Untitled system, edited or not", () => {
  assert.deepEqual(systemTitle(undefined, ["\\dot{x}=y", "\\dot{y}=-x"]), { name: "Untitled system", edited: false });
  assert.deepEqual(systemTitle(undefined, ["\\dot{x}=y", "\\dot{y}=-x-y"]), { name: "Untitled system", edited: false });
});

test("name: every gallery template matches its own equations", () => {
  for (const t of templates) assert.equal(systemTitle(t, t.equations).edited, false, t.id);
});
