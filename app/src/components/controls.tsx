// Shared controls, drawn with the design system's ds-* markup (styles/ds/components.css).
//
// Used by the system panel, the inspector and the canvas. Keep the props stable:
// NumberField, Slider, ToggleChip, Segmented and MenuButton predate the design
// system and keep their original props; Button, IconButton, Select, Switch and
// SegmentedControl are the design-system components.

import {
  useEffect,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { fmt } from "../math/format";
import { IconChevronDown, IconError, IconPlus } from "./icons";

const cx = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(" ");

/** Display a formatted number with the true minus sign (U+2212). */
export const withMinus = (s: string) => s.replace(/-/g, "−");

/* ------------------------------------------------------------------ Button */

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "ghost-danger";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: "md" | "sm";
  /** Leading 16px icon (the same icon the action uses elsewhere). */
  icon?: ReactNode;
  /** Busy: keeps the width, shows a spinner and `busyLabel`, ignores clicks. */
  loading?: boolean;
  busyLabel?: string;
}

const VARIANT_CLASS: Record<ButtonVariant, string> = {
  primary: "ds-button--primary",
  secondary: "ds-button--secondary",
  ghost: "ds-button--ghost",
  danger: "ds-button--danger",
  "ghost-danger": "ds-button--ghost ds-button--danger",
};

/** A text button (DS Button): primary, secondary, ghost or danger, two sizes. */
export function Button({
  variant = "secondary",
  size = "md",
  icon,
  loading,
  busyLabel = "Computing…",
  className,
  children,
  onClick,
  ...rest
}: ButtonProps) {
  const cls = cx("ds-button", VARIANT_CLASS[variant], size === "sm" && "ds-button--sm", loading && "is-loading", className);
  if (loading !== undefined) {
    return (
      <button
        type="button"
        className={cls}
        aria-disabled={loading || undefined}
        aria-busy={loading || undefined}
        onClick={(e) => {
          if (!loading) onClick?.(e);
        }}
        {...rest}
      >
        <span className="ds-button-stack">
          <span className="ds-button-label">
            {icon}
            <span>{children}</span>
          </span>
          <span className="ds-button-busy">
            <svg
              className="ds-icon ds-button-spinner"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth={1.5}
              strokeLinecap="round"
              aria-hidden="true"
            >
              <path d="M8 2.5A5.5 5.5 0 1 1 2.5 8" />
            </svg>
            <span>{busyLabel}</span>
          </span>
        </span>
      </button>
    );
  }
  return (
    <button type="button" className={cls} onClick={onClick} {...rest}>
      {icon}
      {icon ? <span>{children}</span> : children}
    </button>
  );
}

/* -------------------------------------------------------------- IconButton */

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> {
  /** Accessible name (required: the button has no visible text). */
  label: string;
  /** Tooltip text (`data-tip`); starts with the name. Defaults to `label`. */
  tip?: string | false;
  /** Put the tooltip's right edge on the button's (near a container's right edge). */
  tipAlign?: "end";
  /** Toggle state (`aria-pressed`). Leave undefined for a plain action. */
  pressed?: boolean;
  /** A canvas tool: chosen = accent-soft fill. */
  tool?: boolean;
  size?: "md" | "sm";
  /** Raised fill and edge outline, for a standalone button in a panel. */
  outline?: boolean;
  children: ReactNode;
}

/** A square icon-only button (DS IconButton) with a tooltip. Escape hides the tooltip. */
export function IconButton({
  label,
  tip,
  tipAlign,
  pressed,
  tool,
  size = "md",
  outline,
  className,
  children,
  onKeyDown,
  onPointerLeave,
  onBlur,
  ...rest
}: IconButtonProps) {
  const [dismissed, setDismissed] = useState(false);
  const tipText = tip === false ? undefined : (tip ?? label);
  return (
    <button
      type="button"
      className={cx(
        "ds-icon-button",
        size === "sm" && "ds-icon-button--sm",
        tool && "ds-icon-button--tool",
        outline && "ds-icon-button--outline",
        dismissed && "is-tip-dismissed",
        className,
      )}
      aria-label={label}
      aria-pressed={pressed}
      data-tip={tipText}
      data-tip-align={tipAlign}
      onKeyDown={(e) => {
        if (e.key === "Escape" && tipText && !dismissed) setDismissed(true);
        onKeyDown?.(e);
      }}
      onPointerLeave={(e) => {
        setDismissed(false);
        onPointerLeave?.(e);
      }}
      onBlur={(e) => {
        setDismissed(false);
        onBlur?.(e);
      }}
      {...rest}
    >
      {children}
    </button>
  );
}

/* ------------------------------------------------------------- NumberField */

interface NumberFieldProps {
  value: number;
  onCommit: (v: number) => void;
  /** Significant digits shown when not editing. */
  digits?: number;
  /** Step for ArrowUp/ArrowDown (Shift = ×10) and the − / + steppers. */
  step?: number;
  ariaLabel: string;
  /** Extra class on the field box (`.ds-numfield`). */
  className?: string;
  /** Commit on every keystroke that parses (live), instead of on Enter/blur. */
  live?: boolean;
  /** Custom display when not editing, e.g. 0 → "auto". */
  format?: (v: number) => string;
  /** Leading symbol (typeset): t₀, rtol, σ. */
  lead?: ReactNode;
  /** Short suffix in ink-3: rad, samples. */
  unit?: ReactNode;
  /** Show − / + stepper buttons (needs `step`). */
  steppers?: boolean;
  /** Dense rows: control-h-sm tall, numeric-sm digits. */
  size?: "md" | "sm";
  disabled?: boolean;
  /** Rule check before committing: return a message ("Must be positive") or null. */
  validate?: (v: number) => string | null;
  /** A hint shown under the field in ink-3 when there is no error. */
  hint?: ReactNode;
}

const parseNumber = (s: string): number => {
  const t = s.trim().replace(/−/g, "-").replace(/×/g, "*").toLowerCase();
  if (t === "pi" || t === "π") return Math.PI;
  if (t === "-pi" || t === "-π") return -Math.PI;
  if (t === "auto") return 0;
  return t === "" ? NaN : Number(t);
};

/**
 * A one-line numeric input (DS NumberField): mono, right-aligned, optional
 * leading symbol, unit and steppers. Shows a rounded value at rest and full
 * precision while editing. Accepts "1e-6", "−2" and "pi". A bad value stays in
 * the field with a message under it until it is fixed or Escape reverts it.
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
  lead,
  unit,
  steppers,
  size = "md",
  disabled,
  validate,
  hint,
}: NumberFieldProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const msgId = useId();
  /** Set while Escape blurs the input, so the blur does not commit the reverted text. */
  const reverting = useRef(false);
  const shown = draft ?? withMinus(format ? format(value) : fmt(value, digits));

  /** Parse and check `s`; commits and returns true when it is good. */
  const commit = (s: string, quiet = false): boolean => {
    const v = parseNumber(s);
    const msg = !Number.isFinite(v) ? "Not a number" : (validate?.(v) ?? null);
    if (msg) {
      if (!quiet) setError(msg);
      return false;
    }
    setError(null);
    onCommit(v);
    return true;
  };
  const nudge = (dir: 1 | -1, big: boolean) => {
    if (!step) return;
    const cur = parseNumber(shown);
    const base = Number.isFinite(cur) ? cur : value;
    const next = Number((base + dir * step * (big ? 10 : 1)).toPrecision(12));
    const msg = validate?.(next) ?? null;
    if (msg) return;
    setError(null);
    if (draft !== null) setDraft(withMinus(String(next)));
    onCommit(next);
  };

  const invalid = error !== null;
  return (
    <>
      <div
        className={cx("ds-numfield", size === "sm" && "ds-numfield--sm", className)}
        onPointerDown={(e) => {
          // Clicking the symbol or unit focuses the input, like a label.
          if (e.target === e.currentTarget || (e.target as HTMLElement).closest(".ds-numfield-lead, .ds-numfield-unit")) {
            e.preventDefault();
            (e.currentTarget.querySelector("input") as HTMLInputElement | null)?.focus();
          }
        }}
      >
        {lead !== undefined && <span className="ds-numfield-lead" aria-hidden="true">{lead}</span>}
        <input
          className="ds-numfield-input"
          type="text"
          aria-label={ariaLabel}
          inputMode="decimal"
          spellCheck={false}
          autoComplete="off"
          disabled={disabled}
          value={shown}
          role={steppers ? "spinbutton" : undefined}
          aria-valuenow={steppers ? value : undefined}
          aria-invalid={invalid || undefined}
          aria-describedby={invalid || hint ? msgId : undefined}
          onFocus={(e) => {
            if (draft === null) setDraft(withMinus(fmt(value, 8)));
            const el = e.target;
            requestAnimationFrame(() => el.select());
          }}
          onChange={(e) => {
            setDraft(e.target.value);
            if (live) commit(e.target.value, true);
          }}
          onBlur={(e) => {
            if (reverting.current) {
              reverting.current = false;
              return;
            }
            if (commit(e.target.value)) setDraft(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              if (commit((e.target as HTMLInputElement).value)) {
                setDraft(null);
                (e.target as HTMLInputElement).blur();
              }
            } else if (e.key === "Escape") {
              e.preventDefault();
              setDraft(null);
              setError(null);
              reverting.current = true;
              (e.target as HTMLInputElement).blur();
              reverting.current = false;
            } else if (step && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
              e.preventDefault();
              nudge(e.key === "ArrowUp" ? 1 : -1, e.shiftKey);
            }
          }}
        />
        {unit !== undefined && <span className="ds-numfield-unit">{unit}</span>}
        {steppers && step && (
          <>
            <button
              className="ds-numfield-step"
              type="button"
              tabIndex={-1}
              aria-label={`Decrease ${ariaLabel}`}
              disabled={disabled}
              onClick={() => nudge(-1, false)}
            >
              <svg
                className="ds-icon"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                strokeWidth={1.5}
                strokeLinecap="round"
                aria-hidden="true"
              >
                <path d="M3.5 8h9" />
              </svg>
            </button>
            <button
              className="ds-numfield-step"
              type="button"
              tabIndex={-1}
              aria-label={`Increase ${ariaLabel}`}
              disabled={disabled}
              onClick={() => nudge(1, false)}
            >
              <IconPlus />
            </button>
          </>
        )}
      </div>
      {invalid ? (
        <p className="ds-numfield-msg caption" id={msgId}>
          <IconError />
          <span>{error}</span>
        </p>
      ) : (
        hint && (
          <p className="ds-numfield-msg ds-numfield-msg--hint caption" id={msgId}>
            {hint}
          </p>
        )
      )}
    </>
  );
}

/* ------------------------------------------------------------------ Slider */

interface SliderProps {
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
  ariaLabel: string;
  /** Number of slider steps across the range. */
  resolution?: number;
  /** Spoken value when the plain number is unclear ("2.667 (8/3)"). */
  ariaValueText?: string;
  className?: string;
  id?: string;
}

/** A range slider (the ParamSlider track and thumb); the fill shows the value. */
export function Slider({
  value,
  min,
  max,
  onChange,
  ariaLabel,
  resolution = 1000,
  ariaValueText,
  className,
  id,
}: SliderProps) {
  const step = (max - min) / resolution;
  const frac = max > min ? (value - min) / (max - min) : 0;
  return (
    <input
      type="range"
      id={id}
      className={cx("ds-param-slider", className)}
      aria-label={ariaLabel}
      aria-valuetext={ariaValueText}
      min={min}
      max={max}
      step={step || "any"}
      value={value}
      style={{ ["--frac" as string]: String(Math.max(0, Math.min(1, frac))) }}
      onChange={(e) => onChange(Number(e.target.value))}
    />
  );
}

/* ------------------------------------------------------------------ Switch */

interface SwitchProps {
  checked: boolean;
  onChange: (on: boolean) => void;
  /** Visible label: the thing that appears ("Nullclines"), never "Enable …". */
  label: ReactNode;
  /** Optional leading icon in the label (ink-2). */
  icon?: ReactNode;
  /** Label left, track right, full width: layer lists and settings rows. */
  row?: boolean;
  disabled?: boolean;
  /** Only when the label is not text. */
  ariaLabel?: string;
  /** Id of a caption that explains the option. */
  describedBy?: string;
  title?: string;
  className?: string;
}

/** An on/off switch (DS Switch) that applies at once. */
export function Switch({
  checked,
  onChange,
  label,
  icon,
  row,
  disabled,
  ariaLabel,
  describedBy,
  title,
  className,
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      aria-describedby={describedBy}
      data-tooltip={title}
      disabled={disabled}
      className={cx("ds-switch", row && "ds-switch--row", className)}
      onClick={() => onChange(!checked)}
    >
      <span className="ds-switch-track">
        <span className="ds-switch-thumb" />
      </span>
      <span className="ds-switch-label">
        {icon}
        <span>{label}</span>
      </span>
    </button>
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

/** A layer or option toggle. Kept for older call sites; drawn as a DS Switch. */
export function ToggleChip({ on, onChange, children, title, swatch }: ToggleChipProps) {
  return (
    <Switch
      checked={on}
      onChange={onChange}
      title={title}
      label={children}
      icon={
        swatch ? (
          <span className="ds-traj-swatch" style={{ ["--traj" as string]: swatch }} aria-hidden="true" />
        ) : undefined
      }
    />
  );
}

/* ------------------------------------------------------------------ Select */

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
}

interface SelectProps<T extends string> {
  value: T;
  options: SelectOption<T>[];
  onChange: (v: T) => void;
  /** Use either a visible <label htmlFor={id}> or this, not both. */
  ariaLabel?: string;
  id?: string;
  size?: "md" | "sm";
  /** Italic math font, for options that are variables (x, y, t, θ). */
  math?: boolean;
  /** As wide as the selected text (toolbars only). */
  auto?: boolean;
  disabled?: boolean;
  describedBy?: string;
  className?: string;
}

/** A native select with the chevron-down icon on top (DS Select). */
export function Select<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  id,
  size = "md",
  math,
  auto,
  disabled,
  describedBy,
  className,
}: SelectProps<T>) {
  return (
    <div
      className={cx(
        "ds-select",
        size === "sm" && "ds-select--sm",
        math && "ds-select--math",
        auto && "ds-select--auto",
        className,
      )}
    >
      <select
        id={id}
        className="ds-select-input"
        aria-label={ariaLabel}
        aria-describedby={describedBy}
        disabled={disabled}
        value={value}
        onChange={(e) => onChange(e.target.value as T)}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value} disabled={o.disabled}>
            {o.label}
          </option>
        ))}
      </select>
      <IconChevronDown className="ds-select-chevron" />
    </div>
  );
}

/* -------------------------------------------------------- SegmentedControl */

export interface SegmentOption<T extends string> {
  value: T;
  label: ReactNode;
  title?: string;
  /** Unavailable: shown, skipped by the arrow keys, not selectable. */
  disabled?: boolean;
  /** Leading icon; with `iconOnly` the label becomes the aria-label. */
  icon?: ReactNode;
}

interface SegmentedProps<T extends string> {
  value: T;
  options: SegmentOption<T>[];
  onChange: (v: T) => void;
  ariaLabel: string;
  size?: "md" | "sm";
  /** Stretch to the container with equal-width segments. */
  fill?: boolean;
  /** Icon-only keys (canvas corner only); `label` must be a string then. */
  iconOnly?: boolean;
  /** Id of a caption explaining the choice or why an option is unavailable. */
  describedBy?: string;
  disabled?: boolean;
  className?: string;
}

/**
 * 2–5 mutually exclusive options (DS SegmentedControl): role radiogroup, one
 * tab stop, arrow keys move and select (skipping unavailable options), Home/End.
 */
export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  size = "md",
  fill,
  iconOnly,
  describedBy,
  disabled,
  className,
}: SegmentedProps<T>) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const available = (i: number) => !disabled && !options[i]?.disabled;
  const move = (from: number, dir: 1 | -1 | "home" | "end") => {
    const n = options.length;
    let i = dir === "home" ? -1 : dir === "end" ? n : from;
    const d = dir === "home" ? 1 : dir === "end" ? -1 : dir;
    for (let k = 0; k < n; k++) {
      i = (((i + d) % n) + n) % n;
      if (available(i)) {
        onChange(options[i].value);
        refs.current[i]?.focus();
        return;
      }
    }
  };
  const onKey = (e: ReactKeyboardEvent, i: number) => {
    const map: Record<string, 1 | -1 | "home" | "end"> = {
      ArrowRight: 1,
      ArrowDown: 1,
      ArrowLeft: -1,
      ArrowUp: -1,
      Home: "home",
      End: "end",
    };
    const dir = map[e.key];
    if (dir === undefined) return;
    e.preventDefault();
    move(i, dir);
  };
  const selectedIndex = options.findIndex((o) => o.value === value);
  return (
    <div
      className={cx(
        "ds-segmented",
        size === "sm" && "ds-segmented--sm",
        fill && "ds-segmented--fill",
        disabled && "is-disabled",
        className,
      )}
      role="radiogroup"
      aria-label={ariaLabel}
      aria-describedby={describedBy}
      aria-disabled={disabled || undefined}
    >
      {options.map((o, i) => {
        const checked = o.value === value;
        const tabbable = checked || (selectedIndex < 0 && i === 0);
        return (
          <button
            key={o.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={checked}
            aria-disabled={o.disabled || undefined}
            aria-describedby={o.disabled ? describedBy : undefined}
            aria-label={iconOnly && typeof o.label === "string" ? o.label : undefined}
            disabled={disabled}
            tabIndex={tabbable ? 0 : -1}
            className={cx("ds-segmented-item", iconOnly && "ds-segmented-item--icon")}
            data-tooltip={o.title ?? (iconOnly && typeof o.label === "string" ? o.label : undefined)}
            onClick={() => {
              if (available(i)) onChange(o.value);
            }}
            onKeyDown={(e) => onKey(e, i)}
          >
            {o.icon}
            {iconOnly ? null : o.icon ? <span className="ds-segmented-text">{o.label}</span> : o.label}
          </button>
        );
      })}
    </div>
  );
}

/** Older name of {@link SegmentedControl}; same props. */
export const Segmented = SegmentedControl;

/* -------------------------------------------------------------- MenuButton */

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
        <IconChevronDown />
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
