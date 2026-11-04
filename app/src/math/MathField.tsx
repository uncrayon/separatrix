import { useEffect, useRef } from "react";
import { MathfieldElement } from "mathlive";
import { GREEK_NAMES } from "./format";

interface MathFieldProps {
  value: string;
  onChange: (latex: string) => void;
  onEnter?: () => void;
  onFocusChange?: (focused: boolean) => void;
  placeholder?: string;
  ariaLabel?: string;
  ariaDescribedBy?: string;
  autoFocus?: boolean;
  invalid?: boolean;
}

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
  const onChangeRef = useRef(onChange);
  const onEnterRef = useRef(onEnter);
  const onFocusRef = useRef(onFocusChange);
  onChangeRef.current = onChange;
  onEnterRef.current = onEnter;
  onFocusRef.current = onFocusChange;

  useEffect(() => {
    const mf = new MathfieldElement();
    mf.className = "math";
    mf.mathVirtualKeyboardPolicy = "manual";
    mf.smartFence = true;
    mf.popoverPolicy = "off";
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
  }, []);

  useEffect(() => {
    const mf = fieldRef.current;
    if (mf && mf.value !== value) mf.value = value;
  }, [value]);

  useEffect(() => {
    const mf = fieldRef.current;
    if (!mf) return;
    setAttr(mf, "aria-label", ariaLabel);
    setAttr(mf, "aria-describedby", ariaDescribedBy);
    setAttr(mf, "aria-invalid", invalid ? "true" : undefined);
    mf.placeholder = placeholder ?? "";
  }, [ariaLabel, ariaDescribedBy, invalid, placeholder]);

  return <div ref={hostRef} className="ds-equation-editor-field mathfield-host" />;
}

function setAttr(el: Element, name: string, value: string | undefined) {
  if (value) el.setAttribute(name, value);
  else el.removeAttribute(name);
}

const LATEX_FUNCS = new Set([
  "sin", "cos", "tan", "sec", "csc", "cot", "sinh", "cosh", "tanh", "coth",
  "arcsin", "arccos", "arctan", "exp", "ln", "log", "min", "max",
]);
const TEXT_ONLY_FUNCS = new Set(["sqrt", "cbrt", "abs", "sign", "sgn", "asin", "acos", "atan", "floor", "ceil"]);
const isFunc = (w: string) => LATEX_FUNCS.has(w) || TEXT_ONLY_FUNCS.has(w);

const PRIME_COUNTS: Record<string, number> = { "'": 1, "\\prime": 1, "\\doubleprime": 2, "\\tripleprime": 3 };

/** How many primes a superscript made only of primes stands for (MathLive writes x'' as `x^{\doubleprime}`), else 0. */
function primeCount(sup: string): number {
  let n = 0;
  for (const part of sup.match(/\\[A-Za-z]+|\S/g) ?? []) {
    const k = PRIME_COUNTS[part];
    if (!k) return 0;
    n += k;
  }
  return n;
}

type Kind = "num" | "name" | "func" | "open" | "close" | "op" | "eq" | "comma";
interface TextTok {
  text: string;
  startKind: Kind;
  endKind: Kind;
}

const tok = (text: string, k: Kind, endKind: Kind = k): TextTok => ({ text, startKind: k, endKind });

const LEFT_PAREN = "\\left(";
const RIGHT = "\\right";

export function latexToText(latex: string): string {
  return joinText(latexToks(latex));
}

function latexToks(src: string): TextTok[] {
  const out: TextTok[] = [];
  let i = 0;

  const skipSpaces = () => {
    while (i < src.length && /\s/.test(src[i])) i++;
  };
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
  const wrapped = (latex: string): TextTok => {
    const toks = latexToks(latex);
    if (toks.length === 1) return toks[0];
    return tok(`(${joinText(toks)})`, "open", "close");
  };
  const appendToLast = (suffix: string, e?: Kind) => {
    const last = out[out.length - 1];
    if (!last) return;
    last.text += suffix;
    if (e) last.endKind = e;
  };

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
      const arg = readArg();
      const primes = primeCount(arg);
      if (primes > 0) {
        appendToLast("'".repeat(primes));
      } else {
        const sup = asText(arg);
        appendToLast(/^[A-Za-z0-9.]+$/.test(sup) ? `^${sup}` : `^(${sup})`);
      }
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
        out.push({ text: `${a.text}/${b.text}`, startKind: a.startKind, endKind: b.endKind });
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
        const n = index === null ? "2" : asText(index);
        // A cube root is real for x < 0, unlike x^(1/3), so it keeps its own function.
        out.push(
          n === "2"
            ? tok(`sqrt(${arg})`, "func", "close")
            : n === "3"
              ? tok(`cbrt(${arg})`, "func", "close")
              : tok(`(${arg})^(1/${n})`, "open", "close"),
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

  const funcCall = (name: string) => {
    skipSpaces();
    if (src[i] === "(" || src.startsWith(LEFT_PAREN, i)) {
      out.push(tok(name, "func"));
      return;
    }
    let power = "";
    if (src[i] === "^") {
      i++;
      const sup = asText(readArg());
      power = /^[A-Za-z0-9.]+$/.test(sup) ? `^${sup}` : `^(${sup})`;
      skipSpaces();
      if (src[i] === "(" || src.startsWith(LEFT_PAREN, i)) {
        const open = src[i] === "(" ? 1 : LEFT_PAREN.length;
        let depth = 0;
        let k = i;
        for (; k < src.length; k++) {
          if (src[k] === "(") depth++;
          else if (src[k] === ")" && --depth === 0) break;
        }
        const close = src.slice(0, k).endsWith(RIGHT) ? RIGHT.length : 0;
        const inner = src.slice(i + open, k - close);
        i = Math.min(src.length, k + 1);
        out.push(tok(`${name}(${asText(inner)})${power}`, "func", "close"));
        return;
      }
    }
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

function joinText(toks: TextTok[]): string {
  let s = "";
  let seenEq = false;
  for (let k = 0; k < toks.length; k++) {
    const a = toks[k - 1];
    const b = toks[k];
    if (a && needsExplicitStar(a, b, seenEq)) s += "*";
    if (b.startKind === "eq") {
      seenEq = true;
      s += " = ";
    } else if (b.startKind === "comma") {
      s += ", ";
    } else if (b.text === "+" || b.text === "-") {
      const unary =
        !a || a.endKind === "op" || a.endKind === "eq" || a.endKind === "open" || a.endKind === "comma";
      s += unary ? b.text : ` ${b.text} `;
    } else {
      s += b.text;
    }
  }
  return s.trim();
}

function needsExplicitStar(a: TextTok, b: TextTok, seenEq: boolean): boolean {
  return (
    (a.endKind === "name" &&
      (b.startKind === "name" || b.startKind === "num" || b.startKind === "func" || (b.startKind === "open" && seenEq))) ||
    (a.endKind === "num" && b.startKind === "num")
  );
}

export function textToLatex(text: string): string {
  let out = "";
  let i = 0;
  const src = text.replace(/−/g, "-").replace(/[·×]/g, "*");
  let last: "num" | "name" | "close" | "other" = "other";
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
    if (GREEK_NAMES.has(w)) return `\\${w}`;
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
      // One factor, so that x/1e3 stays x/(1×10³) and does not become (x/1)×10³.
      out += exp !== undefined ? `\\left(${mant}\\times10^{${exp}}\\right)` : mant;
      i += all.length;
      last = "num";
      continue;
    }
    if ((m = /^[A-Za-z][A-Za-z0-9]*/.exec(rest))) {
      const w = m[0];
      i += w.length;
      if (isFunc(w)) {
        const cmd = LATEX_FUNCS.has(w) ? `\\${w}` : null;
        if (src[i] === "(" && (w === "sqrt" || w === "cbrt" || w === "abs")) {
          const [inner, next] = balancedGroup(i);
          i = next;
          const arg = textToLatex(inner);
          out += w === "sqrt" ? `\\sqrt{${arg}}` : w === "cbrt" ? `\\sqrt[3]{${arg}}` : `\\left|${arg}\\right|`;
          last = "close";
        } else {
          out += cmd ?? `\\operatorname{${w}}`;
          last = "other";
        }
        continue;
      }
      lastNameAt = out.length;
      out += w === "pi" ? "\\pi" : word(w);
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
        const em = /^-?(?:[0-9.]+(?:[eE][+-]?[0-9]+)?|[A-Za-z][A-Za-z0-9]*)/.exec(src.slice(i));
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
      const next = src[i] ?? "";
      // Juxtaposed, `x*(t)` would read as x at time t, not as x times t.
      const timeArgument = /^\(\s*t\s*\)/.test(src.slice(i));
      const implicit = /[A-Za-z(]/.test(next) && last !== "other" && !timeArgument;
      // A space keeps letters apart: `\sigma x`, not `\sigmax`, and `a b s` (a·b·s), not `abs`,
      // which LaTeX reads as a misspelt \abs.
      out += implicit ? (/[A-Za-z]$/.test(out) ? " " : "") : "\\cdot ";
      last = "other";
      continue;
    }
    i++;
    out += c;
    last = c === ")" ? "close" : "other";
  }
  return out;
}
