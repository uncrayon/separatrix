// MathLive's visual markup is a stack of glyph spans that a screen reader
// reads as "x ˙ = x 2 − t". So the markup is aria-hidden and a spoken copy
// ("x dot equals x squared minus t") sits next to it for assistive technology.

import { memo } from "react";
import { convertLatexToMarkup } from "mathlive";

/** HTML that is safe to inject: MathLive's markup, or escaped source text. */
export type SanitizedMarkup = string & { readonly __brand: "SanitizedMarkup" };

const cache = new Map<string, SanitizedMarkup>();
const speechCache = new Map<string, string>();

export function texToMarkup(latex: string, display = false): SanitizedMarkup {
  const key = (display ? "D" : "T") + latex;
  let html = cache.get(key);
  if (html === undefined) {
    try {
      html = convertLatexToMarkup(latex, { defaultMode: display ? "math" : "inline-math" }) as SanitizedMarkup;
    } catch {
      html = escapeHtml(latex);
    }
    if (cache.size > 2000) cache.clear();
    cache.set(key, html);
  }
  return html;
}

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

/** How deeply nested braces (\\frac{\\dot{x}}{…}) are spoken. */
const MAX_GROUP_DEPTH = 4;

/** Every phrase speak() and power() say. */
const SPOKEN = {
  dot: "dot",
  doubleDot: "double dot",
  over: "over",
  sub: "sub",
  times: "times",
  plusOrMinus: "plus or minus",
  partial: "partial",
  infinity: "infinity",
  equals: "equals",
  plus: "plus",
  minus: "minus",
  openParen: "open paren",
  closeParen: "close paren",
  squared: "squared",
  cubed: "cubed",
  toThe: "to the",
} as const;

const FUNCS: Record<string, string> = { sin: "sine", cos: "cosine", tan: "tangent", exp: "exp", ln: "log", log: "log" };

const WORDS = new Set([...Object.values(SPOKEN), ...Object.values(FUNCS)].flatMap((phrase) => phrase.split(" ")));

function speak(src: string): string {
  let s = src;
  const group = String.raw`\{((?:[^{}]|\{[^{}]*\})*)\}`;
  const re = (p: string) => new RegExp(p.replace(/G/g, group), "g");
  for (let k = 0; k < MAX_GROUP_DEPTH; k++) {
    s = s
      .replace(re(String.raw`\\ddot\s*G`), ` $1 ${SPOKEN.doubleDot} `)
      .replace(re(String.raw`\\dot\s*G`), ` $1 ${SPOKEN.dot} `)
      .replace(re(String.raw`\\frac\s*GG`), ` $1 ${SPOKEN.over} $2 `)
      .replace(re(String.raw`\\(?:operatorname|mathrm|text)\s*G`), " $1 ")
      .replace(re(String.raw`\^\s*G`), (_, e: string) => power(e))
      .replace(re(String.raw`_\s*G`), ` ${SPOKEN.sub} $1 `);
  }
  s = s
    .replace(/\^\s*([0-9A-Za-z])/g, (_, e: string) => power(e))
    .replace(/_\s*([0-9A-Za-z])/g, ` ${SPOKEN.sub} $1 `)
    .replace(/\\left|\\right/g, "")
    .replace(/\\(sin|cos|tan|exp|ln|log)\b/g, (_, f: string) => ` ${FUNCS[f] ?? f} `)
    .replace(/\\(cdot|times)\b/g, ` ${SPOKEN.times} `)
    .replace(/\\pm\b/g, ` ${SPOKEN.plusOrMinus} `)
    .replace(/\\partial\b/g, ` ${SPOKEN.partial} `)
    .replace(/\\infty\b/g, ` ${SPOKEN.infinity} `)
    .replace(/\\([A-Za-z]+)/g, (m, name: string) => (GREEK.has(name) ? ` ${name} ` : m))
    .replace(/\\[,;:! ]/g, " ")
    .replace(/[{}]/g, " ")
    .replace(/=/g, ` ${SPOKEN.equals} `)
    .replace(/\+/g, ` ${SPOKEN.plus} `)
    .replace(/[-−]/g, ` ${SPOKEN.minus} `)
    .replace(/\(/g, ` ${SPOKEN.openParen} `)
    .replace(/\)/g, ` ${SPOKEN.closeParen} `);
  return s
    .split(/\s+/)
    .filter(Boolean)
    .map(spellImplicitProduct)
    .join(" ")
    .replace(/\s+,/g, ",");
}

/** "2xy" is spoken "2 x y": split letter runs and digit-letter joins, except known words. */
function spellImplicitProduct(word: string): string {
  if (!/^[0-9.]*[A-Za-z]+$/.test(word) || WORDS.has(word) || GREEK.has(word)) return word;
  return word.split(/(?<=[0-9.])(?=[A-Za-z])|(?<=[A-Za-z])(?=[A-Za-z])/).join(" ");
}

function power(e: string): string {
  const t = e.trim();
  if (t === "2") return ` ${SPOKEN.squared} `;
  if (t === "3") return ` ${SPOKEN.cubed} `;
  return ` ${SPOKEN.toThe} ${t} `;
}

function escapeHtml(s: string): SanitizedMarkup {
  return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`) as SanitizedMarkup;
}

const innerHtml = (markup: SanitizedMarkup) => ({ __html: markup });

interface TexProps {
  latex: string;
  display?: boolean;
  className?: string;
}

export const Tex = memo(function Tex({ latex, display, className }: TexProps) {
  return (
    <span className={"tex" + (className ? " " + className : "")}>
      <span
        className="tex-glyphs"
        aria-hidden="true"
        dangerouslySetInnerHTML={innerHtml(texToMarkup(latex, display))}
      />
      <span className="ds-sr">{texToSpeech(latex)}</span>
    </span>
  );
});
