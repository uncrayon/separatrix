import { dec, withSubscript } from "../math/format";

export { dec };

/**
 * A start point as "(x_0, y_0) =" (LaTeX) and "(1.00, 2.00)", with the start
 * time first when `t0` is given.
 */
export function startPointParts(
  stateLatex: string[],
  x0: number[],
  t0: number | null,
): { nameLatex: string; value: string } {
  const names = [...(t0 !== null ? ["t"] : []), ...stateLatex].map((n) => withSubscript(n, "0"));
  const values = [...(t0 !== null ? [t0] : []), ...x0].map((v) => dec(v, 2));
  if (names.length === 1) return { nameLatex: `${names[0]} =`, value: values[0] ?? "" };
  return { nameLatex: `(${names.join(", ")}) =`, value: `(${values.join(", ")})` };
}
