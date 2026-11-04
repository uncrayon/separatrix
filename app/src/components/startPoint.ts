/** `v` with a fixed number of decimals, a typographic minus and no "−0.00". */
export function dec(v: number, places: number): string {
  if (!Number.isFinite(v)) return Number.isNaN(v) ? "NaN" : v > 0 ? "∞" : "−∞";
  const s = v.toFixed(places);
  return (Number(s) === 0 ? (0).toFixed(places) : s).replace(/^-/, "−");
}

/**
 * A start point as "(x_0, y_0) =" (LaTeX) and "(1.00, 2.00)", with the start
 * time first when `t0` is given.
 */
export function startPointParts(
  stateLatex: string[],
  x0: number[],
  t0: number | null,
): { nameLatex: string; value: string } {
  const names = [...(t0 !== null ? ["t"] : []), ...stateLatex].map((n) => `${n}_0`);
  const values = [...(t0 !== null ? [t0] : []), ...x0].map((v) => dec(v, 2));
  if (names.length === 1) return { nameLatex: `${names[0]} =`, value: values[0] ?? "" };
  return { nameLatex: `(${names.join(", ")}) =`, value: `(${values.join(", ")})` };
}
