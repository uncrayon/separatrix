import raw from "./templates.json";
import generatedText from "./templates.text.generated.json";
import { UNTITLED_SYSTEM } from "../state/model";

export { templateLines } from "../state/model";

export interface Template {
  id: string;
  title: string;
  chapter: string;
  section?: string;
  description: string;
  format: "latex";
  /** The equations as authored, in LaTeX. */
  equations: string[];
  /**
   * The same equations in text mode, one per line, written by the core's
   * converter (templates.text.generated.json; regenerate with
   * `UPDATE_TEMPLATE_TEXT=1 cargo test -p dyn-expr --test convert`).
   */
  text: string[];
  params: Record<string, number>;
  paramRanges?: Record<string, [number, number]>;
  view: {
    x: string;
    y: string;
    xRange: [number, number];
    yRange: [number, number];
  };
  initialConditions: number[][];
  tEnd: number;
  backward?: boolean;
}

const TEXT: Record<string, string[] | undefined> = generatedText;

/** Text-mode lines of the template `id`; a template without them starts empty. */
const textOf = (id: string): string[] => TEXT[id] ?? [""];

export const TEMPLATES: Template[] = (raw as Omit<Template, "text">[]).map((t) => ({ ...t, text: textOf(t.id) }));

export const BLANK_TEMPLATE: Template = {
  id: "blank",
  title: UNTITLED_SYSTEM,
  chapter: "",
  description: "",
  format: "latex",
  equations: ["\\dot{x}=y", "\\dot{y}=-x"],
  text: textOf("blank"),
  params: {},
  view: { x: "x", y: "y", xRange: [-4, 4], yRange: [-3, 3] },
  initialConditions: [],
  tEnd: 20,
};

export function templateById(id: string): Template | undefined {
  return TEMPLATES.find((t) => t.id === id);
}

export function templateSource(t: Template): string {
  return t.equations.join("\n");
}

export function strogatzRef(t: Template): string {
  if (t.section) return `Strogatz §${t.section}`;
  return t.chapter ? `Strogatz ${t.chapter}` : "";
}
