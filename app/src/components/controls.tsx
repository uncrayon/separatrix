import {
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

export const withMinus = (s: string) => s.replace(/-/g, "−");

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "ghost-danger";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: "md" | "sm";
  icon?: ReactNode;
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

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> {
  label: string;
  tip?: string | false;
  tipAlign?: "end";
  pressed?: boolean;
  tool?: boolean;
  size?: "md" | "sm";
  outline?: boolean;
  children: ReactNode;
}

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

interface NumberFieldProps {
  value: number;
  onCommit: (v: number) => void;
  digits?: number;
  step?: number;
  ariaLabel: string;
  className?: string;
  live?: boolean;
  format?: (v: number) => string;
  lead?: ReactNode;
  unit?: ReactNode;
  steppers?: boolean;
  size?: "md" | "sm";
  disabled?: boolean;
  validate?: (v: number) => string | null;
  hint?: ReactNode;
}

/** A number as typed: a typographic minus, `pi` or `π` are understood; NaN when it is not a number. */
export const parseNumber = (s: string): number => {
  const t = s.trim().replace(/−/g, "-").toLowerCase();
  if (t === "pi" || t === "π") return Math.PI;
  if (t === "-pi" || t === "-π") return -Math.PI;
  return t === "" ? NaN : Number(t);
};

/** A number field's text, as {@link parseNumber}, where "auto" is 0 (the samples field shows 0 as "auto"). */
const parseFieldNumber = (s: string): number => (s.trim().toLowerCase() === "auto" ? 0 : parseNumber(s));

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
  const reverting = useRef(false);
  const shown = draft ?? withMinus(format ? format(value) : fmt(value, digits));
  const focusText = withMinus(fmt(value, 8));

  const commit = (s: string, quiet = false): boolean => {
    if (s === focusText) {
      setError(null);
      return true;
    }
    const v = parseFieldNumber(s);
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
    const cur = parseFieldNumber(shown);
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
            if (draft === null) setDraft(focusText);
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

interface SliderProps {
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
  ariaLabel: string;
  resolution?: number;
  ariaValueText?: string;
  className?: string;
  id?: string;
}

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

interface SwitchProps {
  checked: boolean;
  onChange: (on: boolean) => void;
  label: ReactNode;
  icon?: ReactNode;
  row?: boolean;
  disabled?: boolean;
  ariaLabel?: string;
  describedBy?: string;
  title?: string;
  className?: string;
}

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

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
}

interface SelectProps<T extends string> {
  value: T;
  options: SelectOption<T>[];
  onChange: (v: T) => void;
  ariaLabel?: string;
  id?: string;
  size?: "md" | "sm";
  math?: boolean;
  auto?: boolean;
  disabled?: boolean;
  describedBy?: string;
  className?: string;
}

/**
 * WebKitGTK sends no pointerup for the mouse press that opens a <select>'s
 * popup, so it still counts that button as down: the next press brings a
 * mousedown but no pointerdown, and everything that starts on pointerdown (the
 * plots, closing a menu or a tooltip) misses it. After a mouse press on a
 * Select, the first mousedown that comes without its pointerdown gets one
 * dispatched ahead of it. A pointerdown or pointerup first means the browser
 * kept the pointer's state, and nothing is dispatched.
 */
let selectPressId: number | null = null;
let disarmPressRepair: (() => void) | null = null;

function repairNextPress(pointerId: number) {
  disarmPressRepair?.();
  const stop = () => disarmPressRepair?.();
  const onMouseDown = (e: MouseEvent) => {
    stop();
    e.target?.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        cancelable: true,
        composed: true,
        view: window,
        pointerId,
        pointerType: "mouse",
        isPrimary: true,
        width: 1,
        height: 1,
        pressure: 0.5,
        detail: e.detail,
        button: e.button,
        buttons: e.buttons,
        clientX: e.clientX,
        clientY: e.clientY,
        screenX: e.screenX,
        screenY: e.screenY,
        ctrlKey: e.ctrlKey,
        shiftKey: e.shiftKey,
        altKey: e.altKey,
        metaKey: e.metaKey,
      }),
    );
  };
  window.addEventListener("mousedown", onMouseDown, true);
  window.addEventListener("pointerdown", stop, true);
  window.addEventListener("pointerup", stop, true);
  disarmPressRepair = () => {
    window.removeEventListener("mousedown", onMouseDown, true);
    window.removeEventListener("pointerdown", stop, true);
    window.removeEventListener("pointerup", stop, true);
    disarmPressRepair = null;
  };
}

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
      onPointerDown={(e) => (selectPressId = e.pointerType === "mouse" ? e.pointerId : null)}
      onMouseDown={() => selectPressId !== null && repairNextPress(selectPressId)}
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

export interface SegmentOption<T extends string> {
  value: T;
  label: ReactNode;
  title?: string;
  /** The accessible name when the visible label is a bare glyph (`ẋ`). */
  ariaLabel?: string;
  disabled?: boolean;
  /** The id of this option's own reason for being disabled (else the group's `describedBy`). */
  describedBy?: string;
  icon?: ReactNode;
}

interface SegmentedProps<T extends string> {
  value: T;
  options: SegmentOption<T>[];
  onChange: (v: T) => void;
  /** The group's name; with `labelledBy`, a visible label names it instead. */
  ariaLabel?: string;
  /** The id of a visible label for the group. */
  labelledBy?: string;
  size?: "md" | "sm";
  fill?: boolean;
  /** Lay the options out in rows of this many, each row ruled off from the one above. */
  columns?: number;
  iconOnly?: boolean;
  describedBy?: string;
  disabled?: boolean;
  className?: string;
}

export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  labelledBy,
  size = "md",
  fill,
  columns,
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
        columns !== undefined && "segmented-grid",
        disabled && "is-disabled",
        className,
      )}
      role="radiogroup"
      aria-label={labelledBy ? undefined : ariaLabel}
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      aria-disabled={disabled || undefined}
      style={columns !== undefined ? { gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` } : undefined}
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
            aria-describedby={o.disabled ? (o.describedBy ?? describedBy) : undefined}
            aria-label={o.ariaLabel ?? (iconOnly && typeof o.label === "string" ? o.label : undefined)}
            disabled={disabled}
            tabIndex={tabbable ? 0 : -1}
            className={cx(
              "ds-segmented-item",
              iconOnly && "ds-segmented-item--icon",
              columns !== undefined && i % columns === 0 && "segmented-row-start",
              columns !== undefined && i >= columns && "segmented-lower-row",
              columns !== undefined && selectedIndex >= 0 && i === selectedIndex + columns && "segmented-below-checked",
            )}
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
