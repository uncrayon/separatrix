import raw from "./templates.json";
import { UNTITLED_SYSTEM } from "../state/model";

export interface Template {
  id: string;
  title: string;
  chapter: string;
  section?: string;
  description: string;
  format: "latex";
  equations: string[];
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

export const TEMPLATES: Template[] = raw as Template[];

export const BLANK_TEMPLATE: Template = {
  id: "blank",
  title: UNTITLED_SYSTEM,
  chapter: "",
  description: "",
  format: "latex",
  equations: ["\\dot{x}=y", "\\dot{y}=-x"],
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
