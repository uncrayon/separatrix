// Static typeset maths (read-only), via MathLive's convertLatexToMarkup.

import { memo } from "react";
import { convertLatexToMarkup } from "mathlive";

const cache = new Map<string, string>();

/** LaTeX → HTML markup, cached (the same labels are rendered many times). */
export function texToMarkup(latex: string, display = false): string {
  const key = (display ? "D" : "T") + latex;
  let html = cache.get(key);
  if (html === undefined) {
    try {
      html = convertLatexToMarkup(latex, { defaultMode: display ? "math" : "inline-math" });
    } catch {
      html = escapeHtml(latex);
    }
    if (cache.size > 2000) cache.clear();
    cache.set(key, html);
  }
  return html;
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

interface TexProps {
  /** LaTeX source, e.g. `\dot{x} = \sigma(y - x)`. */
  latex: string;
  /** Larger, display-style layout (fractions full size). */
  display?: boolean;
  className?: string;
  title?: string;
}

/** Inline typeset LaTeX. Use for every symbol the user typed. */
export const Tex = memo(function Tex({ latex, display, className, title }: TexProps) {
  return (
    <span
      className={"tex" + (className ? " " + className : "")}
      title={title}
      // MathLive produces trusted markup from our own LaTeX strings.
      dangerouslySetInnerHTML={{ __html: texToMarkup(latex, display) }}
    />
  );
});
