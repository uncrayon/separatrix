// Small reusable controls. They only use token-based classes from app.css.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { fmt } from "../math/format";
import { IconChevron } from "./icons";

interface NumberFieldProps {
  value: number;
  onCommit: (v: number) => void;
  /** Significant digits shown when not editing. */
  digits?: number;
  /** Step for ArrowUp/ArrowDown (Shift = ×10). */
  step?: number;
  ariaLabel: string;
  className?: string;
  /** Commit on every keystroke that parses (live), instead of on Enter/blur. */
  live?: boolean;
  /** Custom display when not editing, e.g. 0 → "auto". */
  format?: (v: number) => string;
}

/**
 * A compact numeric input. Shows a rounded value; while focused shows what
 * the user types. Accepts "1e-6", "−2" and "pi" style input.
 */
export function NumberField({
  value,
  onCommit,
  digits = 4,
  step,
  ariaLabel,
  className,
  live,
  format,
}: NumberFieldProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const shown = draft ?? (format ? format(value) : fmt(value, digits));
  const parse = (s: string): number => {
    const t = s.trim().replace(/−/g, "-").toLowerCase();
    if (t === "pi" || t === "π") return Math.PI;
    if (t === "-pi" || t === "-π") return -Math.PI;
    if (t === "auto") return 0;
    return t === "" ? NaN : Number(t);
  };
  const commit = (s: string) => {
    const v = parse(s);
    if (Number.isFinite(v)) onCommit(v);
  };
  return (
    <input
      className={"num-field" + (className ? " " + className : "")}
      aria-label={ariaLabel}
      inputMode="decimal"
      spellCheck={false}
      value={shown}
      onFocus={(e) => {
        setDraft(fmt(value, 8));
        requestAnimationFrame(() => e.target.select());
      }}
      onChange={(e) => {
        setDraft(e.target.value);
        if (live) commit(e.target.value);
      }}
      onBlur={(e) => {
        commit(e.target.value);
        setDraft(null);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") {
          setDraft(null);
          (e.target as HTMLInputElement).blur();
        }
        if (step && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
          e.preventDefault();
          const base = Number.isFinite(parse(shown)) ? parse(shown) : value;
          const next = base + (e.key === "ArrowUp" ? 1 : -1) * step * (e.shiftKey ? 10 : 1);
          const rounded = Number(next.toPrecision(12));
          setDraft(String(rounded));
          onCommit(rounded);
        }
      }}
    />
  );
}

interface SliderProps {
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
  ariaLabel: string;
  /** Number of slider steps across the range. */
  resolution?: number;
}

/** A range slider whose fill shows the current value. */
export function Slider({ value, min, max, onChange, ariaLabel, resolution = 1000 }: SliderProps) {
  const step = (max - min) / resolution;
  const frac = max > min ? (value - min) / (max - min) : 0;
  return (
    <input
      type="range"
      className="slider"
      aria-label={ariaLabel}
      min={min}
      max={max}
      step={step || "any"}
      value={value}
      style={{ ["--fill" as string]: `${Math.max(0, Math.min(1, frac)) * 100}%` }}
      onChange={(e) => onChange(Number(e.target.value))}
    />
  );
}

interface ToggleChipProps {
  on: boolean;
  onChange: (on: boolean) => void;
  children: ReactNode;
  title?: string;
  /** Optional colour swatch (CSS colour or token reference). */
  swatch?: string;
}

/** A pill that toggles a layer or option. */
export function ToggleChip({ on, onChange, children, title, swatch }: ToggleChipProps) {
  return (
    <button
      type="button"
      className={"chip toggle" + (on ? " on" : "")}
      aria-pressed={on}
      title={title}
      onClick={() => onChange(!on)}
    >
      {swatch && <span className="chip-swatch" style={{ background: swatch }} />}
      {children}
    </button>
  );
}

interface SegmentedProps<T extends string> {
  value: T;
  options: { value: T; label: ReactNode; title?: string }[];
  onChange: (v: T) => void;
  ariaLabel: string;
}

/** A row of mutually exclusive buttons. */
export function Segmented<T extends string>({ value, options, onChange, ariaLabel }: SegmentedProps<T>) {
  return (
    <div className="segmented" role="radiogroup" aria-label={ariaLabel}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          className={o.value === value ? "on" : ""}
          title={o.title}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

interface MenuButtonProps<T> {
  label: ReactNode;
  options: { key: string; value: T; label: ReactNode; hint?: ReactNode }[];
  selectedKey: string;
  onSelect: (v: T) => void;
  ariaLabel: string;
  className?: string;
}

/**
 * A button that opens a small list of choices (used for axis pickers).
 *
 * Keyboard (WAI-ARIA menu button pattern): Enter, Space or ArrowDown/ArrowUp on
 * the button opens the menu with the current choice focused; ArrowUp/ArrowDown
 * move (wrapping), Home/End jump, Enter/Space choose, Escape or Tab close.
 * Focus returns to the button after choosing or Escape.
 */
export function MenuButton<T>({
  label,
  options,
  selectedKey,
  onSelect,
  ariaLabel,
  className,
}: MenuButtonProps<T>) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);
  /** Which item to focus when the menu opens (set by the key that opened it). */
  const focusOnOpen = useRef<"selected" | "first" | "last">("selected");

  const focusItem = (i: number) => {
    const n = options.length;
    if (n > 0) itemRefs.current[((i % n) + n) % n]?.focus();
  };
  const closeAndReturn = () => {
    setOpen(false);
    buttonRef.current?.focus();
  };

  useEffect(() => {
    if (!open) return;
    const sel = options.findIndex((o) => o.key === selectedKey);
    const where = focusOnOpen.current;
    focusItem(where === "first" ? 0 : where === "last" ? options.length - 1 : Math.max(0, sel));
    const close = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
    // Runs only when the menu opens: re-focusing on every option change would steal focus.
  }, [open]);

  const openWith = (where: "selected" | "first" | "last") => {
    focusOnOpen.current = where;
    setOpen(true);
  };

  const onMenuKey = (e: React.KeyboardEvent) => {
    const i = itemRefs.current.findIndex((el) => el === document.activeElement);
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        focusItem(i + 1);
        break;
      case "ArrowUp":
        e.preventDefault();
        focusItem(i - 1);
        break;
      case "Home":
        e.preventDefault();
        focusItem(0);
        break;
      case "End":
        e.preventDefault();
        focusItem(options.length - 1);
        break;
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        closeAndReturn();
        break;
      case "Tab":
        setOpen(false);
        break;
    }
  };

  return (
    <div className={"menu-wrap" + (className ? " " + className : "")} ref={ref}>
      <button
        ref={buttonRef}
        type="button"
        className="menu-button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel}
        onClick={() => (open ? setOpen(false) : openWith("selected"))}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            openWith(e.key === "ArrowDown" ? "selected" : "last");
          }
        }}
      >
        {label}
        <IconChevron size={14} />
      </button>
      {open && (
        <div className="menu" role="menu" aria-label={ariaLabel} onKeyDown={onMenuKey}>
          {options.map((o, i) => (
            <button
              key={o.key}
              ref={(el) => {
                itemRefs.current[i] = el;
              }}
              type="button"
              role="menuitemradio"
              aria-checked={o.key === selectedKey}
              tabIndex={-1}
              className={"menu-item" + (o.key === selectedKey ? " on" : "")}
              onClick={() => {
                onSelect(o.value);
                closeAndReturn();
              }}
            >
              <span className="menu-item-label">{o.label}</span>
              {o.hint && <span className="menu-item-hint">{o.hint}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
