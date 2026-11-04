// Editable equation input: a thin React wrapper around MathLive's <math-field>,
// plus the converters behind the equation editor's plain-text mode.
//
// The element is created imperatively (not through JSX) so React never fights
// MathLive over its DOM. The `value` prop is pushed into the field only when it
// differs from what the field already shows, so typing never jumps the caret.
// The wrapper carries the design system's `ds-equation-editor-field` box (a
// flex row); the <math-field> inside it is a block that fills the box. On the
// element itself the DS class made MathLive's inner container a flex item that
// shrank to the maths, so a click on the empty part of the line hit the bare
// host and never started editing.

import { useEffect, useRef } from "react";
import { MathfieldElement } from "mathlive";

interface MathFieldProps {
  value: string;
  onChange: (latex: string) => void;
  /** Called on Enter: the parent usually adds a new equation line. */
  onEnter?: () => void;
  /** Called when the field gains or loses focus (the line shows its editing state). */
  onFocusChange?: (focused: boolean) => void;
  placeholder?: string;
  ariaLabel?: string;
  /** Id(s) of the diagnostics that describe this field. */
  ariaDescribedBy?: string;
  autoFocus?: boolean;
  invalid?: boolean;
}

/** One MathLive input line. */
export function MathField({
  value,
  onChange,
  onEnter,
  onFocusChange,
  placeholder,
  ariaLabel,
  ariaDescribedBy,
  autoFocus,
  invalid,
}: MathFieldProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const fieldRef = useRef<MathfieldElement | null>(null);
  // Keep the latest callbacks without re-creating the element.
  const onChangeRef = useRef(onChange);
  const onEnterRef = useRef(onEnter);
  const onFocusRef = useRef(onFocusChange);
  onChangeRef.current = onChange;
  onEnterRef.current = onEnter;
  onFocusRef.current = onFocusChange;

  useEffect(() => {
    const mf = new MathfieldElement();
    mf.className = "math";
    // The virtual keyboard opens from the field's own toggle (and on touch).
    mf.mathVirtualKeyboardPolicy = "manual";
    mf.smartFence = true;
    mf.popoverPolicy = "off";
    if (placeholder) mf.placeholder = placeholder;
    mf.value = value;
    mf.addEventListener("input", () => onChangeRef.current(mf.value));
    mf.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter" && !ev.shiftKey) {
        ev.preventDefault();
        onEnterRef.current?.();
      }
    });
    mf.addEventListener("focusin", () => onFocusRef.current?.(true));
    mf.addEventListener("focusout", () => onFocusRef.current?.(false));
    const host = hostRef.current;
    host?.appendChild(mf);
    // A click on the box's padding, outside the field, edits the line too, with
    // the caret at the end.
    const onHostDown = (ev: PointerEvent) => {
      if (ev.target !== host || ev.button !== 0) return;
      ev.preventDefault();
      mf.focus();
      mf.executeCommand("moveToMathfieldEnd");
    };
    host?.addEventListener("pointerdown", onHostDown);
    fieldRef.current = mf;
    if (autoFocus) requestAnimationFrame(() => mf.focus());
    return () => {
      host?.removeEventListener("pointerdown", onHostDown);
      mf.remove();
      fieldRef.current = null;
    };
    // The element is created once; later prop changes are synced below.
  }, []);

  useEffect(() => {
    const mf = fieldRef.current;
    if (mf && mf.value !== value) mf.value = value;
  }, [value]);

  // Accessible name and state follow the line (numbers shift when a line is removed).
  useEffect(() => {
    const mf = fieldRef.current;
    if (!mf) return;
    setAttr(mf, "aria-label", ariaLabel);
    setAttr(mf, "aria-describedby", ariaDescribedBy);
    setAttr(mf, "aria-invalid", invalid ? "true" : undefined);
  }, [ariaLabel, ariaDescribedBy, invalid]);

  return <div ref={hostRef} className="ds-equation-editor-field mathfield-host" />;
}

function setAttr(el: Element, name: string, value: string | undefined) {
  if (value) el.setAttribute(name, value);
  else el.removeAttribute(name);
}

// ------------------------------------------------------------------ text mode
//
// The editor's Text mode shows each line in the core's plain-text syntax
// (`x' = x*(3 - x - 2y)`) and turns what the user types back into LaTeX, so the
// system is always stored and compiled as LaTeX. Switching modes converts the
// lines; it never discards them.

const GREEK_WORDS = new Set([
  "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta", "theta", "vartheta",
  "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "varpi", "rho", "varrho", "sigma",
  "varsigma", "tau", "upsilon", "phi", "varphi", "chi", "psi", "omega", "Gamma", "Delta",
  "Theta", "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi", "Omega",
]);

/** Functions LaTeX has a command for (`\sin`); others become `\operatorname{…}`. */
const LATEX_FUNCS = new Set([
  "sin", "cos", "tan", "sec", "csc", "cot", "sinh", "cosh", "tanh", "coth",
  "arcsin", "arccos", "arctan", "exp", "ln", "log", "min", "max",
]);
const TEXT_ONLY_FUNCS = new Set(["sqrt", "abs", "sign", "sgn", "asin", "acos", "atan", "floor", "ceil"]);
const isFunc = (w: string) => LATEX_FUNCS.has(w) || TEXT_ONLY_FUNCS.has(w);

/** What a text token starts and ends with, for deciding where `*` is needed. */
type Kind = "num" | "name" | "func" | "open" | "close" | "op" | "eq" | "comma";
interface TextTok {
  t: string;
  s: Kind;
  e: Kind;
}

const tok = (t: string, k: Kind, e: Kind = k): TextTok => ({ t, s: k, e });

/** LaTeX as stored (`\dot{x}=x\left(3-x-2y\right)`) → plain text (`x' = x*(3 - x - 2y)`). */
export function latexToText(latex: string): string {
  return joinText(latexToks(latex));
}

function latexToks(src: string): TextTok[] {
  const out: TextTok[] = [];
  let i = 0;

  const skipSpaces = () => {
    while (i < src.length && /\s/.test(src[i])) i++;
  };
  /** The next argument: a `{…}` group's contents, a command, or one character. */
  const readArg = (): string => {
    skipSpaces();
    if (src[i] === "{") {
      let depth = 0;
      const start = i;
      for (; i < src.length; i++) {
        if (src[i] === "{") depth++;
        else if (src[i] === "}" && --depth === 0) break;
      }
      i++;
      return src.slice(start + 1, i - 1);
    }
    if (src[i] === "\\") {
      const m = /^\\([A-Za-z]+|.)/.exec(src.slice(i));
      i += m ? m[0].length : 1;
      return m ? m[0] : "\\";
    }
    return src[i++] ?? "";
  };
  const asText = (latex: string) => joinText(latexToks(latex));
  /** Wrap a sub-expression in brackets unless it is a single token. */
  const wrapped = (latex: string): TextTok => {
    const toks = latexToks(latex);
    if (toks.length === 1) return toks[0];
    return tok(`(${joinText(toks)})`, "open", "close");
  };
  const appendToLast = (suffix: string, e?: Kind) => {
    const last = out[out.length - 1];
    if (!last) return;
    last.t += suffix;
    if (e) last.e = e;
  };

  /** Convert one item at `i`; returns false at the end of input. */
  const step = (): boolean => {
    skipSpaces();
    if (i >= src.length) return false;
    const c = src[i];
    if (/[0-9.]/.test(c)) {
      const m = /^[0-9]*\.?[0-9]+|^[0-9]+\.?/.exec(src.slice(i))!;
      i += m[0].length;
      out.push(tok(m[0], "num"));
    } else if (/[A-Za-z]/.test(c)) {
      i++;
      out.push(tok(c, "name"));
    } else if (c === "\\") {
      const m = /^\\([A-Za-z]+|.)/.exec(src.slice(i));
      const name = m ? m[1] : "";
      i += m ? m[0].length : 1;
      command(name);
    } else if (c === "_") {
      i++;
      const sub = asText(readArg());
      appendToLast(/^[A-Za-z0-9]+$/.test(sub) && sub.length === 1 ? `_${sub}` : `_{${sub.replace(/\s+/g, "")}}`);
    } else if (c === "^") {
      i++;
      const sup = asText(readArg());
      appendToLast(/^[A-Za-z0-9.]+$/.test(sup) ? `^${sup}` : `^(${sup})`);
    } else if (c === "'") {
      i++;
      appendToLast("'");
    } else if (c === "(" || c === "[") {
      i++;
      out.push(tok("(", "open"));
    } else if (c === ")" || c === "]") {
      i++;
      out.push(tok(")", "close"));
    } else if (c === "{") {
      for (const t of latexToks(readArg())) out.push(t);
    } else if (c === "=") {
      i++;
      out.push(tok("=", "eq"));
    } else if (c === ",") {
      i++;
      out.push(tok(",", "comma"));
    } else if (c === "*") {
      i++;
      out.push(tok("*", "op"));
    } else {
      i++;
      out.push(tok(c === "−" ? "-" : c, "op"));
    }
    return true;
  };

  const command = (name: string) => {
    switch (name) {
      case "left":
      case "right": {
        skipSpaces();
        const d = src[i] === "\\" ? readArg() : src[i++];
        if (d === "(" || d === "[" || d === "\\{" || d === "\\lbrace") out.push(tok("(", "open"));
        else if (d === ")" || d === "]" || d === "\\}" || d === "\\rbrace") out.push(tok(")", "close"));
        else if (d === "|" || d === "\\vert") out.push(tok("|", name === "left" ? "open" : "close"));
        return;
      }
      case ",":
      case ";":
      case ":":
      case "!":
      case " ":
      case "quad":
      case "qquad":
      case "displaystyle":
        return;
      case "dot":
      case "ddot": {
        const inner = asText(readArg());
        out.push(tok(inner + (name === "dot" ? "'" : "''"), "name"));
        return;
      }
      case "frac":
      case "dfrac":
      case "tfrac": {
        const a = wrapped(readArg());
        const b = wrapped(readArg());
        out.push({ t: `${a.t}/${b.t}`, s: a.s, e: b.e });
        return;
      }
      case "sqrt": {
        skipSpaces();
        let index: string | null = null;
        if (src[i] === "[") {
          const end = src.indexOf("]", i);
          index = src.slice(i + 1, end < 0 ? src.length : end);
          i = end < 0 ? src.length : end + 1;
        }
        const arg = asText(readArg());
        out.push(
          index
            ? tok(`(${arg})^(1/${asText(index)})`, "open", "close")
            : tok(`sqrt(${arg})`, "func", "close"),
        );
        return;
      }
      case "cdot":
      case "times":
      case "ast":
        out.push(tok("*", "op"));
        return;
      case "operatorname":
      case "mathrm":
      case "text":
      case "textrm":
      case "mathit": {
        const word = readArg().replace(/\s+/g, "");
        if (isFunc(word)) funcCall(word);
        else out.push(tok(word, "name"));
        return;
      }
      case "infty":
        out.push(tok("inf", "name"));
        return;
      case "prime":
        appendToLast("'");
        return;
      case "doubleprime":
        appendToLast("''");
        return;
      case "{":
      case "lbrace":
        out.push(tok("(", "open"));
        return;
      case "}":
      case "rbrace":
        out.push(tok(")", "close"));
        return;
      case "vert":
      case "|":
        out.push(tok("|", "op"));
        return;
    }
    if (isFunc(name)) funcCall(name);
    else out.push(tok(name, "name"));
  };

  /** A function: text mode needs brackets, `\sin\theta` → `sin(theta)`. */
  const funcCall = (name: string) => {
    skipSpaces();
    if (src[i] === "(" || src.startsWith("\\left(", i)) {
      out.push(tok(name, "func"));
      return;
    }
    // `\sin^2 x` = sin(x)^2.
    let power = "";
    if (src[i] === "^") {
      i++;
      const sup = asText(readArg());
      power = /^[A-Za-z0-9.]+$/.test(sup) ? `^${sup}` : `^(${sup})`;
      skipSpaces();
      if (src[i] === "(" || src.startsWith("\\left(", i)) {
        // `\sin^2(x)` or `\sin^2\left(x\right)`: take the bracket's contents.
        const open = src[i] === "(" ? 1 : 6;
        let depth = 0;
        let k = i;
        for (; k < src.length; k++) {
          if (src[k] === "(") depth++;
          else if (src[k] === ")" && --depth === 0) break;
        }
        const close = src.slice(0, k).endsWith("\\right") ? 6 : 0;
        const inner = src.slice(i + open, k - close);
        i = Math.min(src.length, k + 1);
        out.push(tok(`${name}(${asText(inner)})${power}`, "func", "close"));
        return;
      }
    }
    // Without brackets the argument is the following product (`\sin 2x` = sin(2x)),
    // up to the next operator, function or time derivative (as the core reads it).
    const n = out.length;
    if (i < src.length) step();
    for (skipSpaces(); i < src.length && !endsFuncArg(); skipSpaces()) step();
    const arg = out.splice(n);
    out.push(tok(`${name}(${joinText(arg)})${power}`, "func", "close"));
  };

  const endsFuncArg = (): boolean => {
    const c = src[i];
    if ("+-=,)]}−".includes(c)) return true;
    if (c !== "\\") return false;
    const m = /^\\([A-Za-z]+)/.exec(src.slice(i));
    if (!m) return false;
    const name = m[1];
    return (
      name === "right" || name === "dot" || name === "ddot" || name === "cdot" || name === "times" || isFunc(name) ||
      ((name === "operatorname" || name === "mathrm") && isFunc(/^\\[A-Za-z]+\{([^}]*)\}/.exec(src.slice(i))?.[1] ?? ""))
    );
  };

  while (step());
  return out;
}

/** Join text tokens with the spacing and explicit `*` the text syntax wants. */
function joinText(toks: TextTok[]): string {
  let s = "";
  let seenEq = false;
  for (let k = 0; k < toks.length; k++) {
    const a = toks[k - 1];
    const b = toks[k];
    if (a) {
      // Text mode multiplies implicitly only after a number or a closing bracket.
      const needStar =
        (a.e === "name" && (b.s === "name" || b.s === "num" || b.s === "func" || (b.s === "open" && seenEq))) ||
        (a.e === "num" && b.s === "num");
      if (needStar) s += "*";
    }
    if (b.s === "eq") {
      seenEq = true;
      s += " = ";
    } else if (b.s === "comma") {
      s += ", ";
    } else if (b.t === "+" || b.t === "-") {
      const unary = !a || a.e === "op" || a.e === "eq" || a.e === "open" || a.e === "comma";
      s += unary ? b.t : ` ${b.t} `;
    } else {
      s += b.t;
    }
  }
  return s.trim();
}

/** Plain text (`x' = sigma*(y - x)`) → LaTeX (`\dot{x}=\sigma(y-x)`). */
export function textToLatex(text: string): string {
  let out = "";
  let i = 0;
  const src = text.replace(/−/g, "-").replace(/[·×]/g, "*");
  /** What the last emitted piece ends with, for implicit multiplication. */
  let last: "num" | "name" | "close" | "other" = "other";
  /** Start index in `out` of the last name, so primes can wrap it in \dot. */
  let lastNameAt = -1;

  const balancedGroup = (from: number): [string, number] => {
    let depth = 0;
    for (let k = from; k < src.length; k++) {
      if (src[k] === "(") depth++;
      else if (src[k] === ")" && --depth === 0) return [src.slice(from + 1, k), k + 1];
    }
    return [src.slice(from + 1), src.length];
  };
  const word = (w: string): string => {
    if (GREEK_WORDS.has(w)) return `\\${w}`;
    if (w.length === 1) return w;
    const m = /^([A-Za-z])([0-9]+)$/.exec(w);
    if (m) return `${m[1]}_{${m[2]}}`;
    return `\\mathrm{${w}}`;
  };

  while (i < src.length) {
    const c = src[i];
    const rest = src.slice(i);
    if (/\s/.test(c)) {
      i++;
      continue;
    }
    let m: RegExpExecArray | null;
    if ((m = /^(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE]([+-]?[0-9]+))?/.exec(rest))) {
      const [all, exp] = m;
      const mant = exp !== undefined ? all.slice(0, all.search(/[eE]/)) : all;
      out += exp !== undefined ? `${mant}\\times10^{${exp}}` : mant;
      i += all.length;
      last = "num";
      continue;
    }
    if ((m = /^[A-Za-z][A-Za-z0-9]*/.exec(rest))) {
      const w = m[0];
      i += w.length;
      if (isFunc(w)) {
        const cmd = LATEX_FUNCS.has(w) ? `\\${w}` : null;
        if (src[i] === "(" && (w === "sqrt" || w === "abs")) {
          const [inner, next] = balancedGroup(i);
          i = next;
          const arg = textToLatex(inner);
          out += w === "sqrt" ? `\\sqrt{${arg}}` : `\\left|${arg}\\right|`;
          last = "close";
        } else {
          out += cmd ?? `\\operatorname{${w}}`;
          last = "other";
        }
        continue;
      }
      lastNameAt = out.length;
      out += w === "pi" ? "\\pi" : word(w);
      // Subscript: x_1, x_n, x_{n+1}, x_(n+1)
      if (src[i] === "_") {
        i++;
        if (src[i] === "{" || src[i] === "(") {
          const close = src[i] === "{" ? "}" : ")";
          const end = src.indexOf(close, i);
          const inner = src.slice(i + 1, end < 0 ? src.length : end);
          i = end < 0 ? src.length : end + 1;
          out += `_{${textToLatex(inner)}}`;
        } else {
          const sm = /^[A-Za-z0-9]+/.exec(src.slice(i));
          const sub = sm ? sm[0] : "";
          i += sub.length;
          out += sub.length === 1 ? `_${sub}` : `_{${sub}}`;
        }
      }
      last = "name";
      continue;
    }
    if (c === "'") {
      let n = 0;
      while (src[i] === "'") {
        n++;
        i++;
      }
      if (last === "name" && lastNameAt >= 0 && n <= 2) {
        const base = out.slice(lastNameAt);
        out = out.slice(0, lastNameAt) + `\\${n === 1 ? "dot" : "ddot"}{${base}}`;
      } else {
        out += "'".repeat(n);
      }
      continue;
    }
    if (rest.startsWith("**") || c === "^") {
      i += c === "^" ? 1 : 2;
      while (src[i] === " ") i++;
      let exp: string;
      if (src[i] === "(") {
        const [inner, next] = balancedGroup(i);
        i = next;
        exp = textToLatex(inner);
      } else {
        const em = /^-?(?:[0-9.]+|[A-Za-z][A-Za-z0-9]*)/.exec(src.slice(i));
        exp = em ? textToLatex(em[0]) : "";
        i += em ? em[0].length : 0;
      }
      out += `^{${exp}}`;
      last = "close";
      continue;
    }
    if (c === "*") {
      i++;
      while (src[i] === " ") i++;
      // Drop the star where LaTeX multiplies implicitly (x*y → xy, 2*x → 2x).
      const next = src[i] ?? "";
      const implicit = /[A-Za-z(]/.test(next) && !(last === "other");
      // After a command (`\sigma`) a following letter needs a space: `\sigma x`, not `\sigmax`.
      out += implicit ? (/\\[A-Za-z]+$/.test(out) ? " " : "") : "\\cdot ";
      last = "other";
      continue;
    }
    i++;
    out += c;
    last = c === ")" ? "close" : "other";
  }
  return out;
}
