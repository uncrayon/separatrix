// Input modes: one format per system, conversion by the core, and the
// browser preview's stand-in for it. Node (>= 22.18) strips the types from
// the .ts modules; the JSON files are read with import attributes.

import test from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_LAYERS,
  DEFAULT_SOLVER,
  buildPortraitJob,
  conversionOutcome,
  hasNullclineLayer,
  planModeSwitch,
  sameEquations,
  systemTitle,
  templateLines,
} from "../state/model.ts";
import { MOCK_CONVERT_NOTE, mockConvertLines, mockTemplateIndex } from "../api/mockLookup.ts";
import { diagnosticText, latexToPlain } from "../math/format.ts";
import raw from "../templates/templates.json" with { type: "json" };
import generated from "../templates/templates.text.generated.json" with { type: "json" };

const templates = raw.map((t) => ({ ...t, text: generated[t.id] }));
const vdp = templates.find((t) => t.id === "van-der-pol");

test("templates: every template has generated text, one line per equation", () => {
  for (const t of templates) {
    assert.ok(Array.isArray(t.text), `${t.id} has no text in templates.text.generated.json`);
    assert.equal(t.text.length, t.equations.length, t.id);
    assert.ok(t.text.every((l) => typeof l === "string" && l.trim() !== ""), t.id);
  }
  assert.deepEqual(Object.keys(generated).sort(), [...raw.map((t) => t.id), "blank"].sort());
});

test("templates: templateLines picks the format's lines", () => {
  assert.deepEqual(templateLines(vdp, "latex"), vdp.equations);
  assert.deepEqual(templateLines(vdp, "text"), vdp.text);
});

test("text mode: only spacing between non-word characters is ignored", () => {
  assert.equal(sameEquations(["x' = x*(3 - x - 2*y)"], ["x'=x*(3-x-2*y)"], "text"), true);
  assert.equal(sameEquations(["x' = y", ""], ["  x' =  y  "], "text"), true);
  assert.equal(sameEquations(["x' = a b"], ["x' = ab"], "text"), false);
  assert.equal(sameEquations(["x' = x 2"], ["x' = x2"], "text"), false);
  assert.equal(sameEquations(["x' = sin(x)"], ["x' = sin(x)^2"], "text"), false);
  // No LaTeX normalisation in text mode.
  assert.equal(sameEquations(["x' = x^{2}"], ["x' = x^2"], "text"), false);
});

test("text mode: a template's own text is not an edit, any change is", () => {
  for (const t of templates) assert.equal(systemTitle(t, t.text, "text").edited, false, t.id);
  assert.equal(systemTitle(vdp, vdp.equations, "text").edited, true, "LaTeX lines in text mode are an edit");
  const edited = vdp.text.map((l, i) => (i === 0 ? l + "1" : l));
  assert.deepEqual(systemTitle(vdp, edited, "text"), { name: vdp.title, edited: true });
  assert.equal(systemTitle(vdp, vdp.text.map((l) => l.replace(/ /g, "")), "text").edited, false);
});

test("mode switch: straight back with nothing edited restores the lines exactly", () => {
  const snapshot = { from: "latex", before: ["\\dot{x}=y", "\\dot{y}=-x"], after: ["x' = y", "y' = -x"] };
  assert.deepEqual(planModeSwitch(["x' = y", "y' = -x"], snapshot, "latex"), { restore: snapshot.before });
});

test("mode switch: an edit, an added line or a removed line makes the core convert", () => {
  const snapshot = { from: "latex", before: ["\\dot{x}=y", "\\dot{y}=-x"], after: ["x' = y", "y' = -x"] };
  for (const lines of [["x' = y", "y' = -x + 1"], ["x' = y", "y' = -x", ""], ["x' = y"]]) {
    assert.deepEqual(planModeSwitch(lines, snapshot, "latex"), { convert: lines }, JSON.stringify(lines));
  }
  assert.deepEqual(planModeSwitch(["x' = y"], null, "latex"), { convert: ["x' = y"] });
  // The snapshot is only for switching back to the format it came from.
  assert.deepEqual(planModeSwitch(snapshot.after, snapshot, "text"), { convert: snapshot.after });
});

test("browser preview: template lines convert both ways, other lines are kept with a note", () => {
  for (const t of templates) {
    const toText = mockConvertLines(templates, { lines: t.equations, from: "latex", to: "text" });
    assert.deepEqual(
      toText.lines.map((l) => l.source),
      t.text,
      t.id,
    );
    assert.ok(toText.lines.every((l) => l.converted && !l.note), t.id);
    const back = mockConvertLines(templates, { lines: t.text, from: "text", to: "latex" });
    assert.deepEqual(
      back.lines.map((l) => l.source),
      t.equations,
      t.id,
    );
  }
  const res = mockConvertLines(templates, { lines: ["x' = y", "", "x' = 42*x"], from: "text", to: "latex" });
  assert.deepEqual(res.lines, [
    { source: "\\dot{x}=y", converted: true, error: false },
    { source: "", converted: true, error: false },
    { source: "x' = 42*x", converted: false, error: true, note: MOCK_CONVERT_NOTE },
  ]);
  // The preview's kept lines let the switch go ahead, as an error line does.
  assert.equal(conversionOutcome(res).switch, true);
});

test("mode switch: a valid line the core couldn't convert calls the switch off", () => {
  const note = "LaTeX can't write the number 9.109e-31 exactly";
  const res = {
    lines: [
      { source: "\\dot{x} = y", converted: true, error: false },
      { source: "y' = -x/9.109e-31", converted: false, error: false, note },
      { source: "", converted: true, error: false },
    ],
  };
  assert.deepEqual(conversionOutcome(res), {
    switch: false,
    notes: [undefined, `${note}, so the input mode was not switched`, undefined],
  });
});

test("mode switch: lines with an error are kept as typed and the switch goes ahead", () => {
  const note = "This line has an error, so it was kept as typed";
  const res = {
    lines: [
      { source: "\\dot{x} = y", converted: true, error: false },
      { source: "y' = (x", converted: false, error: true, note },
    ],
  };
  assert.deepEqual(conversionOutcome(res), {
    switch: true,
    lines: ["\\dot{x} = y", "y' = (x"],
    notes: [undefined, note],
  });
});

test("browser preview: a template's LaTeX and text compile to the same system", () => {
  templates.forEach((t, i) => {
    assert.equal(mockTemplateIndex(templates, t.equations.join("\n")), i, t.id);
    assert.equal(mockTemplateIndex(templates, t.text.join("\n")), i, t.id);
  });
  assert.equal(mockTemplateIndex(templates, "x' = 42*x"), -1);
  assert.equal(mockTemplateIndex(templates, ""), -1);
});

test("diagnostics: prose keeps its hyphens and primes are not quotes", () => {
  assert.equal(diagnosticText("Missing right-hand side after '='"), "Missing right-hand side after ‘=’");
  assert.equal(diagnosticText("Missing '*' between x and y"), "Missing ‘*’ between x and y");
  const prime = "x' appears in more than one term; combine them or write the line as x'+x' = …";
  assert.equal(diagnosticText(prime), prime);
  assert.equal(diagnosticText("Several highest derivatives (x'', y'') appear"), "Several highest derivatives (x'', y'') appear");
  assert.equal(diagnosticText("for example \\dot{x} = -x"), "for example x\u0307 = -x");
  // Maths alone still gets true minus signs.
  assert.equal(latexToPlain("-\\sigma x"), "−σ x");
});

test("nullclines: requested in a time-series view of a 2-D flow, so the toggle is shown there", () => {
  assert.equal(hasNullclineLayer("ode"), true);
  assert.equal(hasNullclineLayer("map"), false);
  const compiled = {
    ok: true,
    systemId: 1,
    kind: "ode",
    hamiltonian: false,
    autonomous: true,
    states: [
      { name: "x", latex: "x", derived: false },
      { name: "y", latex: "y", derived: false },
    ],
    params: [],
    equationsLatex: [],
    diagnostics: [],
  };
  const job = buildPortraitJob({
    compiled,
    params: [],
    view: { xAxis: { kind: "time" }, yAxis: { kind: "state", index: 0 }, xRange: [0, 10], yRange: [-3, 3] },
    baseState: [0, 0],
    fieldTime: 0,
    ics: [],
    layers: { ...DEFAULT_LAYERS, nullclines: true },
    solver: DEFAULT_SOLVER,
    plotAspect: null,
  });
  assert.equal(job.body.nullclines, true);
});
