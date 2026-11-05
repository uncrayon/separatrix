// Regenerates ticks.json and numbers.json from the UI's own code:
//   node crates/export/tests/vectors/generate.mjs
// `niceTicks` and `tickLabel` are taken verbatim from
// app/src/canvas/viewport.ts (copied to a temporary .ts file, so this runs
// whatever that file imports), and numbers are JavaScript's String(v).
// heads.json is written by the UI's tests (see docs/exports.md).
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const src = readFileSync(join(here, "../../../../app/src/canvas/viewport.ts"), "utf8");

function grab(name) {
  const i = src.indexOf(`export function ${name}(`);
  const line = src.slice(i, src.indexOf("\n", i));
  let p = i + line.lastIndexOf("{");
  let depth = 0;
  for (; p < src.length; p++) {
    if (src[p] === "{") depth++;
    else if (src[p] === "}" && --depth === 0) return src.slice(i, p + 1);
  }
  throw new Error(`no ${name} in viewport.ts`);
}
const maxDecimals = src.slice(src.indexOf("const MAX_LABEL_DECIMALS"), src.indexOf("\n", src.indexOf("const MAX_LABEL_DECIMALS")));
const dir = mkdtempSync(join(tmpdir(), "sepx-vectors-"));
const file = join(dir, "ticks.ts");
writeFileSync(file, `${grab("niceTicks")}\n${maxDecimals}\n${grab("tickLabel")}\n`);
const { niceTicks, tickLabel } = await import(pathToFileURL(file).href);
rmSync(dir, { recursive: true, force: true });

const ranges = [
  [-0.25, 3.5, 2], [-0.25, 3.5, 4], [-25, 25, 3], [-2, 64, 2], [-1.6, 1.6, 2], [-0.5, 0.5, 2],
  [-0.3, 6, 2], [-2.5, 2.5, 2], [-2, 62, 4], [-0.05, 1.05, 2], [1e-9, 1.0000000001e-9, 3],
  [123456, 123457, 4], [-1e7, 3e7, 5], [-7, 7, 6], [-3.5, 3.5, 3], [0.1, 0.30000000000000004, 3],
  [-1.234e-6, 5.678e-6, 4], [99990, 100010, 4], [-1e-3, 1e-3, 7], [0, 1, 1], [0, 1, 10],
  [2.5, 3.75, 3], [-0.0001, 0.0001, 3], [1e20, 3e20, 3], [-12, 14, 5], [-13, 10, 4],
];
const ticks = ranges.map(([lo, hi, target]) => {
  const t = niceTicks(lo, hi, target);
  return { lo, hi, target, step: t.step, ticks: t.ticks, labels: t.ticks.map((v) => tickLabel(v, t.step)) };
});
const extra = [
  [0.125, 0.01], [-0.125, 0.01], [2.5, 1], [1.005, 0.01], [0.0005, 0.001], [1.5e-7, 1e-7], [-1.5e-7, 1e-8],
  [250000, 50000], [1.25e5, 1e4], [9.995, 0.01], [-0, 1], [0.35, 0.1], [1e-5, 1e-6],
];
const labels = extra.map(([v, step]) => ({ v, step, label: tickLabel(v, step) }));
writeFileSync(
  join(here, "ticks.json"),
  JSON.stringify({ generator: "app/src/canvas/viewport.ts niceTicks/tickLabel", ticks, labels }, null, 1) + "\n",
);
const nums = [
  0, -0, 1, -1.5, 0.1, 0.1 + 0.2, 1e-6, 1.5e-6, 9.99e-7, 1e-7, -1.5e-7, 1e20, 1e21, 1.5e21, 123456789012345680000,
  5e-324, 1.7976931348623157e308, 2.6666666666666665, 1 / 3, -2 / 3, 1e-300, 4.35, 0.000001234, 1e15, 1e16,
  12345678.9, 2 ** 53, 2 ** 53 + 2, -1e-6, 3.14159, 100, 1e100, Number.MIN_VALUE * 3, 0.1 * 3,
];
writeFileSync(
  join(here, "numbers.json"),
  JSON.stringify({ generator: "JavaScript String(v)", cases: nums.map((v) => ({ v, s: String(v) })) }, null, 1) + "\n",
);
