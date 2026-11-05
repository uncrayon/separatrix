// Axis names (`x`, `\theta`, `x_{1}`, `x_{1,n}`, `\dot{x}`) as runs of
// plain text for an exported figure. The screen typesets them with MathLive
// in HTML; a canvas or an SVG file has only text, so a name becomes base,
// subscript and superscript runs drawn in the maths font.
//
// The SVG recorder writes the runs as one <text> with <tspan dy> children
// (never `baseline-shift`, which Firefox and several editors ignore). The
// dot accents are combining marks (U+0307, U+0308), which some fonts place
// a little off-centre.

import type { DrawContext } from "./draw";

export type MathScript = "base" | "sub" | "sup";

export interface MathRun {
  text: string;
  script: MathScript;
  italic: boolean;
}

/** A run laid out: where its baseline sits relative to the name's, and its font. */
export interface MathPiece {
  text: string;
  /** Advance from the start of the name, px. */
  dx: number;
  /** Baseline offset from the name's baseline, px (down is positive). */
  dy: number;
  /** Font size, px. */
  size: number;
  italic: boolean;
  script: MathScript;
}

export interface MathLayout {
  pieces: MathPiece[];
  width: number;
  /** Font size of the base runs, px. */
  size: number;
  family: string;
}

/** A recorder that writes a laid-out name as structured text (the SVG context). */
export interface MathTextSink {
  mathText(layout: MathLayout, x: number, baseline: number, mode: "fill" | "stroke"): void;
}

/** Size of a sub- or superscript relative to the base. */
export const SCRIPT_SCALE = 0.7;
/** How far a subscript's baseline drops, as a fraction of the base size. */
export const SUB_DROP = 0.25;
/** How far a superscript's baseline rises, as a fraction of the base size. */
export const SUP_RISE = 0.4;

const GREEK: Record<string, string> = {
  alpha: "α", beta: "β", gamma: "γ", delta: "δ", epsilon: "ϵ", varepsilon: "ε", zeta: "ζ", eta: "η",
  theta: "θ", vartheta: "ϑ", iota: "ι", kappa: "κ", lambda: "λ", mu: "μ", nu: "ν", xi: "ξ", pi: "π",
  varpi: "ϖ", rho: "ρ", varrho: "ϱ", sigma: "σ", varsigma: "ς", tau: "τ", upsilon: "υ", phi: "ϕ",
  varphi: "φ", chi: "χ", psi: "ψ", omega: "ω", Gamma: "Γ", Delta: "Δ", Theta: "Θ", Lambda: "Λ",
  Xi: "Ξ", Pi: "Π", Sigma: "Σ", Upsilon: "Υ", Phi: "Φ", Psi: "Ψ", Omega: "Ω",
};

const ACCENT: Record<string, string> = { dot: "̇", ddot: "̈", dddot: "⃛" };
const UPRIGHT_GROUP = new Set(["mathrm", "operatorname", "text", "textrm"]);

/** Is `ch` set in italic in maths (Latin letters and lowercase Greek, as TeX does)? */
function italicChar(ch: string): boolean {
  return /^[A-Za-z]$/.test(ch) || /^[α-ωϵϑϖϱςϕ]$/.test(ch);
}

/** The `{…}` group starting at `i` (or the single token there): its content and the index after it. */
function readArg(s: string, i: number): [string, number] {
  while (s[i] === " ") i++;
  if (s[i] === "{") {
    let depth = 0;
    for (let j = i; j < s.length; j++) {
      if (s[j] === "{") depth++;
      else if (s[j] === "}" && --depth === 0) return [s.slice(i + 1, j), j + 1];
    }
    return [s.slice(i + 1), s.length];
  }
  if (s[i] === "\\") {
    const m = /^\\([A-Za-z]+|.)/.exec(s.slice(i));
    const tok = m ? m[0] : "\\";
    return [tok, i + tok.length];
  }
  return [s[i] ?? "", i + 1];
}

/** `latex` as runs, merged where the script and slant agree. */
export function mathRuns(latex: string): MathRun[] {
  const out: MathRun[] = [];
  const push = (text: string, script: MathScript, italic: boolean) => {
    if (!text) return;
    const last = out[out.length - 1];
    if (last && last.script === script && last.italic === italic) last.text += text;
    else out.push({ text, script, italic });
  };
  const walk = (s: string, script: MathScript, upright: boolean) => {
    let i = 0;
    while (i < s.length) {
      const c = s[i];
      if (c === " " || c === "{" || c === "}") {
        if (c === "{") {
          const [inner, next] = readArg(s, i);
          walk(inner, script, upright);
          i = next;
        } else i++;
        continue;
      }
      if (c === "_" || c === "^") {
        const [arg, next] = readArg(s, i + 1);
        // Scripts nest one level only: a sub inside a sup stays at the sup's size.
        walk(arg, script === "base" ? (c === "_" ? "sub" : "sup") : script, upright);
        i = next;
        continue;
      }
      if (c === "'") {
        push("′", script, false);
        i++;
        continue;
      }
      if (c === "\\") {
        const m = /^\\([A-Za-z]+|.)/.exec(s.slice(i));
        const name = m ? m[1] : "";
        i += m ? m[0].length : 1;
        if (name in ACCENT) {
          const [arg, next] = readArg(s, i);
          i = next;
          const before = out.length;
          walk(arg, script, upright);
          // The accent goes on the last character written for the argument.
          const last = out[out.length - 1];
          if (last && out.length >= before) last.text += ACCENT[name];
          continue;
        }
        if (UPRIGHT_GROUP.has(name)) {
          const [arg, next] = readArg(s, i);
          i = next;
          walk(arg, script, true);
          continue;
        }
        if (name === "left" || name === "right" || name === ",") continue;
        if (name === "prime") {
          push("′", script, false);
          continue;
        }
        const g = GREEK[name];
        if (g) push(g, script, !upright && italicChar(g));
        // Any other command word (`\sin`) is written upright, as its name.
        else push(name, script, false);
        continue;
      }
      const ch = c === "-" ? "−" : c;
      push(ch, script, !upright && italicChar(ch));
      i++;
    }
  };
  walk(latex, "base", false);
  return out;
}

/** The CSS font for a run of `size` px. */
export function mathFont(family: string, size: number, italic: boolean): string {
  return `${italic ? "italic " : ""}400 ${round(size)}px ${family}`;
}

const round = (v: number) => Math.round(v * 100) / 100;

/** Lays `runs` out from x = 0 on the base baseline, measuring with `ctx`. */
export function layoutMath(
  ctx: Pick<DrawContext, "font" | "measureText">,
  runs: MathRun[],
  family: string,
  size: number,
): MathLayout {
  const pieces: MathPiece[] = [];
  let x = 0;
  for (const r of runs) {
    const s = r.script === "base" ? size : size * SCRIPT_SCALE;
    const dy = r.script === "sub" ? size * SUB_DROP : r.script === "sup" ? -size * SUP_RISE : 0;
    ctx.font = mathFont(family, s, r.italic);
    const w = ctx.measureText(r.text).width;
    pieces.push({ text: r.text, dx: x, dy, size: s, italic: r.italic, script: r.script });
    // A little italic correction after a slanted base letter before a script.
    x += w + (r.italic && r.script === "base" ? size * 0.05 : 0);
  }
  return { pieces, width: x, size, family };
}

function isSink(ctx: DrawContext): ctx is DrawContext & MathTextSink {
  return typeof (ctx as Partial<MathTextSink>).mathText === "function";
}

/**
 * Draws a laid-out name with its left end at `x` and base baseline at `y`,
 * with a halo behind it as the tick labels have.
 */
export function drawMath(
  ctx: DrawContext,
  layout: MathLayout,
  x: number,
  y: number,
  color: string,
  halo: string,
): void {
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  ctx.lineJoin = "round";
  ctx.lineWidth = 3;
  ctx.strokeStyle = halo;
  ctx.fillStyle = color;
  if (isSink(ctx)) {
    ctx.mathText(layout, x, y, "stroke");
    ctx.mathText(layout, x, y, "fill");
    return;
  }
  for (const mode of ["stroke", "fill"] as const) {
    for (const p of layout.pieces) {
      ctx.font = mathFont(layout.family, p.size, p.italic);
      if (mode === "stroke") ctx.strokeText(p.text, x + p.dx, y + p.dy);
      else ctx.fillText(p.text, x + p.dx, y + p.dy);
    }
  }
}
