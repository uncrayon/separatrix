import { useEffect, useRef } from "react";
import { MathfieldElement } from "mathlive";

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
