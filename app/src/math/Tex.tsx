// Static typeset maths (read-only), via MathLive's convertLatexToMarkup.
//
// MathLive's visual markup is a stack of glyph spans that a screen reader
// reads as "x ˙ = x 2 − t". So the markup is aria-hidden and a spoken copy
// ("x dot equals x squared minus t") sits next to it for assistive technology.

import { memo } from "react";
import { convertLatexToMarkup } from "mathlive";

const cache = new Map<string, string>();
const speechCache = new Map<string, string>();

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

/** LaTeX → words for screen readers, cached ("x dot equals x squared minus t"). */
export function texToSpeech(latex: string): string {
  let text = speechCache.get(latex);
  if (text === undefined) {
    text = speak(latex);
    if (speechCache.size > 2000) speechCache.clear();
    speechCache.set(latex, text);
  }
  return text;
}

const GREEK = new Set([
  "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta", "theta", "vartheta", "iota",
  "kappa", "lambda", "mu", "nu", "xi", "pi", "rho", "sigma", "tau", "upsilon", "phi", "varphi", "chi",
  "psi", "omega", "Gamma", "Delta", "Theta", "Lambda", "Xi", "Pi", "Sigma", "Phi", "Psi", "Omega",
]);

/**
 * Short LaTeX (the app's labels and equations) in words, the way a teacher
 * reads it aloud: "θ double dot equals minus sine θ". Not a general LaTeX
 * reader; anything it does not know is read as written.
 */
function speak(src: string): string {
  let s = src;
  const group = String.raw`\{((?:[^{}]|\{[^{}]*\})*)\}`;
  const re = (p: string) => new RegExp(p.replace(/G/g, group), "g");
  // Innermost first, so nested groups unwind.
  for (let k = 0; k < 4; k++) {
    s = s
      .replace(re(String.raw`\\ddot\s*G`), " $1 double dot ")
      .replace(re(String.raw`\\dot\s*G`), " $1 dot ")
      .replace(re(String.raw`\\frac\s*GG`), " $1 over $2 ")
      .replace(re(String.raw`\\(?:operatorname|mathrm|text)\s*G`), " $1 ")
      .replace(re(String.raw`\^\s*G`), (_, e: string) => power(e))
      .replace(re(String.raw`_\s*G`), " sub $1 ");
  }
  s = s
    .replace(/\^\s*([0-9A-Za-z])/g, (_, e: string) => power(e))
    .replace(/_\s*([0-9A-Za-z])/g, " sub $1 ")
    .replace(/\\left|\\right/g, "")
    .replace(/\\(sin|cos|tan|exp|ln|log)\b/g, (_, f: string) => ` ${FUNCS[f] ?? f} `)
    .replace(/\\(cdot|times)\b/g, " times ")
    .replace(/\\pm\b/g, " plus or minus ")
    .replace(/\\partial\b/g, " partial ")
    .replace(/\\infty\b/g, " infinity ")
    .replace(/\\([A-Za-z]+)/g, (m, name: string) => (GREEK.has(name) ? ` ${name} ` : m))
    .replace(/\\[,;:! ]/g, " ")
    .replace(/[{}]/g, " ")
    .replace(/=/g, " equals ")
    .replace(/\+/g, " plus ")
    .replace(/[-−]/g, " minus ")
    .replace(/\(/g, " open paren ")
    .replace(/\)/g, " close paren ");
  // Implicit products are read letter by letter: "a y", "2 y", not the word "ay".
  return s
    .split(/\s+/)
    .filter(Boolean)
    .map((w) => (/^[0-9.]*[A-Za-z]+$/.test(w) && !WORDS.has(w) && !GREEK.has(w) ? w.split(/(?<=[0-9.])(?=[A-Za-z])|(?<=[A-Za-z])(?=[A-Za-z])/).join(" ") : w))
    .join(" ")
    .replace(/\s+,/g, ",");
}

/** Words speak() itself writes; everything else made of letters is a product of symbols. */
const WORDS = new Set([
  "dot", "double", "equals", "plus", "minus", "open", "close", "paren", "squared", "cubed", "to", "the",
  "over", "or", "sub", "sine", "cosine", "tangent", "exp", "log", "times", "partial", "infinity",
]);

const FUNCS: Record<string, string> = { sin: "sine", cos: "cosine", tan: "tangent", exp: "exp", ln: "log", log: "log" };

function power(e: string): string {
  const t = e.trim();
  if (t === "2") return " squared ";
  if (t === "3") return " cubed ";
  return ` to the ${t} `;
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
}

/** Inline typeset LaTeX. Use for every symbol the user typed. */
export const Tex = memo(function Tex({ latex, display, className }: TexProps) {
  return (
    <span className={"tex" + (className ? " " + className : "")}>
      <span
        className="tex-glyphs"
        aria-hidden="true"
        // MathLive produces trusted markup from our own LaTeX strings.
        dangerouslySetInnerHTML={{ __html: texToMarkup(latex, display) }}
      />
      <span className="ds-sr">{texToSpeech(latex)}</span>
    </span>
  );
});
