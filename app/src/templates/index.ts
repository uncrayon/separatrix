import raw from "./templates.json";
import generatedText from "./templates.text.generated.json";
import { UNTITLED_SYSTEM } from "../state/model";
import type { Direction } from "../api/types";

export { templateLines } from "../state/model";

export type TemplateSection =
  | { kind: "plane"; state: string; value: number; direction: Direction }
  | { kind: "stroboscopic"; period: number; phase: number };

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
    kind?: "cobweb" | "space" | "section";
    x: string;
    y: string;
    xRange: [number, number];
    yRange: [number, number];
    z?: string;
    zRange?: [number, number];
    section?: TemplateSection;
  };
  initialConditions: number[][];
  tEnd: number;
  transient?: number;
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

const CHAPTER_TITLES: Record<string, string> = {
  "Ch. 2": "Flows on the line",
  "Ch. 3": "Bifurcations",
  "Ch. 4": "Flows on the circle",
  "Ch. 5": "Linear systems",
  "Ch. 6": "Phase plane",
  "Ch. 7": "Limit cycles",
  "Ch. 8": "Bifurcations revisited",
  "Ch. 9": "Lorenz equations",
  "Ch. 10": "One-dimensional maps",
  "Ch. 11": "Fractals",
  "Ch. 12": "Strange attractors",
};

export interface TemplateChapter {
  chapter: string;
  number: number;
  title: string;
  templates: Template[];
}

const bookOrder = (s: string | undefined): [number, number] => {
  const [a, b] = (s ?? "").replace(/^Ch\. /, "").split(".").map(Number);
  return [Number.isFinite(a) ? a : Infinity, Number.isFinite(b) ? b : Infinity];
};

export function templateChapters(templates: readonly Template[]): TemplateChapter[] {
  const byChapter = new Map<string, Template[]>();
  for (const t of templates) byChapter.set(t.chapter, [...(byChapter.get(t.chapter) ?? []), t]);
  const sectionKey = (t: Template) => (t.section ? bookOrder(t.section)[1] : -1);
  return [...byChapter.entries()]
    .map(([chapter, list]) => ({
      chapter,
      number: bookOrder(chapter)[0],
      title: CHAPTER_TITLES[chapter] ?? "",
      templates: [...list].sort((a, b) => sectionKey(a) - sectionKey(b)),
    }))
    .sort((a, b) => a.number - b.number);
}

export function strogatzRef(t: Template): string {
  if (t.section) return `Strogatz §${t.section}`;
  return t.chapter ? `Strogatz ${t.chapter}` : "";
}
