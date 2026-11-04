// The design system's PanelSection, shared by both side panels: a header row
// that toggles (h2 > button with aria-expanded), an optional count and up to
// two icon-button actions, and a body that hides when folded. Each panel
// remembers which of its sections are open (a per-viewer convenience).

import { useId, useState, type ReactNode } from "react";
import { IconChevronDown } from "./icons";

const cx = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(" ");

function readSections<K extends string>(storageKey: string): Partial<Record<K, boolean>> {
  try {
    const raw = localStorage.getItem(storageKey);
    return raw ? (JSON.parse(raw) as Partial<Record<K, boolean>>) : {};
  } catch {
    return {};
  }
}

/** Each section's open state, remembered across sessions under `storageKey`. */
export function useSections<K extends string>(storageKey: string, defaults: Record<K, boolean>) {
  const [open, setOpen] = useState<Record<K, boolean>>(() => ({ ...defaults, ...readSections<K>(storageKey) }));
  const set = (key: K, value: boolean) =>
    setOpen((o) => {
      if (o[key] === value) return o;
      const next = { ...o, [key]: value };
      try {
        localStorage.setItem(storageKey, JSON.stringify(next));
      } catch {
        /* storage unavailable: the state still works for this session */
      }
      return next;
    });
  return [open, set] as const;
}

/** One collapsible DS PanelSection: the whole header row toggles; up to two actions. */
export function PanelSection({
  title,
  lead,
  count,
  countSr,
  actions,
  open,
  onToggle,
  flush,
  className,
  id,
  children,
}: {
  title: string;
  /** A glyph or swatch before the title (aria-hidden). */
  lead?: ReactNode;
  count?: number;
  /** Words read after the count ("found"). */
  countSr?: string;
  actions?: ReactNode;
  open: boolean;
  onToggle: (open: boolean) => void;
  /** Switch rows and lists: rows bring their own padding. */
  flush?: boolean;
  className?: string;
  id?: string;
  children: ReactNode;
}) {
  const bodyId = useId();
  return (
    <section className={cx("ds-panel-section", className)} id={id}>
      <h2 className="ds-panel-section-head">
        <button
          type="button"
          className="ds-panel-section-toggle"
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={() => onToggle(!open)}
        >
          {lead}
          <span className="ds-panel-section-title heading">{title}</span>
          {count !== undefined && count > 0 && (
            <span className="ds-panel-section-count numeric-sm">
              {count}
              {countSr && <span className="ds-sr"> {countSr}</span>}
            </span>
          )}
          <IconChevronDown className="ds-panel-section-chevron" />
        </button>
        {actions && <span className="ds-panel-section-actions">{actions}</span>}
      </h2>
      <div
        className={cx("ds-panel-section-body", flush && "ds-panel-section-body--flush")}
        id={bodyId}
        hidden={!open}
      >
        {children}
      </div>
    </section>
  );
}
