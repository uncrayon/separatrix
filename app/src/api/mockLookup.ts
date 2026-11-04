// The browser preview's stand-in for the core's template recognition and
// mode conversion. It knows the gallery templates only, in both input
// formats. Pure (no imports but types), so the unit tests can load it.

import type { ConvertRequest, ConvertResponse, InputFormat } from "./types";

export interface MockTemplateLines {
  /** As authored, in LaTeX. */
  equations: string[];
  /** The generated text-mode form, one line per equation. */
  text: string[];
}

export const squash = (src: string) => src.replace(/\s+/g, "");

const linesIn = (t: MockTemplateLines, format: InputFormat) => (format === "text" ? t.text : t.equations);

/** Joined like the UI sends a source: non-empty lines with newlines. */
const joined = (lines: readonly string[]) => squash(lines.filter((l) => l.trim() !== "").join("\n"));

/** Index of the template whose LaTeX or text equations are `source`, or -1. */
export function mockTemplateIndex(templates: readonly MockTemplateLines[], source: string): number {
  const src = squash(source);
  if (!src) return -1;
  return templates.findIndex((t) => joined(t.equations) === src || joined(t.text) === src);
}

export const MOCK_CONVERT_NOTE = "The browser preview converts template lines only";

/**
 * Maps template lines between their LaTeX and text forms; any other line is
 * kept as typed with a note, as the core does with a line it cannot convert.
 * The preview can't tell a valid line from a broken one, so it flags every
 * such line as an error line, which lets the switch go ahead.
 */
export function mockConvertLines(templates: readonly MockTemplateLines[], req: ConvertRequest): ConvertResponse {
  const lines = req.lines.map((line) => {
    if (req.from === req.to || line.trim() === "") return { source: line, converted: true, error: false };
    const want = squash(line);
    for (const t of templates) {
      const from = linesIn(t, req.from);
      const i = from.findIndex((l) => squash(l) === want);
      const to = linesIn(t, req.to)[i];
      if (i >= 0 && to !== undefined) return { source: to, converted: true, error: false };
    }
    return { source: line, converted: false, error: true, note: MOCK_CONVERT_NOTE };
  });
  return { lines };
}
