// Editable equation input: a thin React wrapper around MathLive's <math-field>.
//
// The element is created imperatively (not through JSX) so React never fights
// MathLive over its DOM. The `value` prop is pushed into the field only when it
// differs from what the field already shows, so typing never jumps the caret.

import { useEffect, useRef } from "react";
import { MathfieldElement } from "mathlive";

interface MathFieldProps {
  value: string;
  onChange: (latex: string) => void;
  /** Called on Enter: the parent usually adds a new equation line. */
  onEnter?: () => void;
  placeholder?: string;
  ariaLabel?: string;
  autoFocus?: boolean;
  invalid?: boolean;
}

/** One MathLive input line. */
export function MathField({
  value,
  onChange,
  onEnter,
  placeholder,
  ariaLabel,
  autoFocus,
  invalid,
}: MathFieldProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const fieldRef = useRef<MathfieldElement | null>(null);
  // Keep the latest callbacks without re-creating the element.
  const onChangeRef = useRef(onChange);
  const onEnterRef = useRef(onEnter);
  onChangeRef.current = onChange;
  onEnterRef.current = onEnter;

  useEffect(() => {
    const mf = new MathfieldElement();
    mf.mathVirtualKeyboardPolicy = "manual";
    mf.smartFence = true;
    mf.popoverPolicy = "off";
    if (placeholder) mf.placeholder = placeholder;
    if (ariaLabel) mf.setAttribute("aria-label", ariaLabel);
    mf.value = value;
    mf.addEventListener("input", () => onChangeRef.current(mf.value));
    mf.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter" && !ev.shiftKey) {
        ev.preventDefault();
        onEnterRef.current?.();
      }
    });
    hostRef.current?.appendChild(mf);
    fieldRef.current = mf;
    if (autoFocus) requestAnimationFrame(() => mf.focus());
    return () => {
      mf.remove();
      fieldRef.current = null;
    };
    // The element is created once; later prop changes are synced below.
  }, []);

  useEffect(() => {
    const mf = fieldRef.current;
    if (mf && mf.value !== value) mf.value = value;
  }, [value]);

  return <div ref={hostRef} className={"mathfield-host" + (invalid ? " invalid" : "")} />;
}
