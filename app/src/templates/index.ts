// Typed access to the template gallery.
//
// templates.json is plain JSON on purpose: the Rust tests read the same file
// to check that every template compiles. Optional fields below are UI hints
// that Rust can ignore.

import raw from "./templates.json";

/** An axis in a template view: a state name (as dyn-expr names it, e.g.
 * "theta'" for the derived state of θ̈), or "t" / "n" for time / iterate. */
export type TemplateAxis = string;

export interface Template {
  id: string;
  title: string;
  /** Strogatz chapter, e.g. "Ch. 6". */
  chapter: string;
  /** One line shown in the gallery. */
  description: string;
  format: "latex";
  /** One equation per entry, exactly as MathLive serialises it. */
  equations: string[];
  /** Parameter values by dyn-expr name (e.g. "sigma", not "\\sigma"). */
  params: Record<string, number>;
  /** Optional slider ranges by parameter name. */
  paramRanges?: Record<string, [number, number]>;
  view: {
    x: TemplateAxis;
    y: TemplateAxis;
    xRange: [number, number];
    yRange: [number, number];
  };
  /** Full state vectors at t = 0 (or n = 0). */
  initialConditions: number[][];
  /** Integration span T for ODEs, or number of iterates for maps. */
  tEnd: number;
  /** Also integrate backwards in time from each initial condition (default true). */
  backward?: boolean;
}

/** All gallery templates, in gallery order. */
export const TEMPLATES: Template[] = raw as Template[];

/** Look up a template by id. */
export function templateById(id: string): Template | undefined {
  return TEMPLATES.find((t) => t.id === id);
}

/** The source text sent to the compiler: one equation per line. */
export function templateSource(t: Template): string {
  return t.equations.join("\n");
}
