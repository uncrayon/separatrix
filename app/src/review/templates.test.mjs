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

const { TEMPLATES } = await import("../templates/index.ts");
const { mockCompile } = await import("../api/mock.ts");
const { templateView, viewChoice } = await import("../state/model.ts");
const generated = (await import("../templates/templates.text.generated.json", { with: { type: "json" } })).default;

const compile = (t, format = "latex") =>
  mockCompile({ source: (format === "latex" ? t.equations : t.text).join("\n"), format });

test("templates: every template has generated text and a mock system that compiles in both formats", async () => {
  for (const t of TEMPLATES) {
    assert.ok(Array.isArray(generated[t.id]), `${t.id}: no lines in templates.text.generated.json`);
    for (const format of ["latex", "text"]) {
      const sys = await compile(t, format);
      assert.ok(sys.ok, `${t.id} (${format}): ${sys.diagnostics.map((d) => d.message).join("; ")}`);
    }
  }
});

test("templates: every parameter a template sets is one its system has", async () => {
  for (const t of TEMPLATES) {
    const sys = await compile(t);
    const names = sys.params.map((p) => p.name);
    for (const p of [...Object.keys(t.params), ...Object.keys(t.paramRanges ?? {})]) {
      assert.ok(names.includes(p), `${t.id}: no parameter ${p} in [${names}]`);
    }
    for (const x0 of t.initialConditions) assert.equal(x0.length, sys.states.length, `${t.id}: start ${x0}`);
  }
});

test("templates: every template opens in the view it names", async () => {
  for (const t of TEMPLATES) {
    const sys = await compile(t);
    const view = templateView(t, sys);
    const want = t.view.kind ?? (t.view.x === "t" || t.view.y === "t" ? "series" : "phase");
    assert.equal(viewChoice(view), want, t.id);
    assert.deepEqual(view.xRange, t.view.xRange, t.id);
    assert.deepEqual(view.yRange, t.view.yRange, t.id);
    if (t.view.kind === "section") {
      assert.equal(view.section.kind, t.view.section.kind, t.id);
      if (view.section.kind === "plane") {
        assert.equal(sys.states[view.section.state].name, t.view.section.state, t.id);
      }
    }
  }
});

test("templates: a template cites a section of its own chapter", () => {
  const cited = TEMPLATES.filter((t) => t.section);
  for (const t of cited) assert.equal(t.chapter, `Ch. ${t.section.split(".")[0]}`, t.id);
  for (const ch of [2, 3, 4, 5, 6, 7, 8]) {
    assert.ok(
      cited.some((t) => t.chapter === `Ch. ${ch}`),
      `no template cites a section of chapter ${ch}`,
    );
  }
});
