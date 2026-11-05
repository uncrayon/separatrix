import { useEffect, useId, useRef } from "react";
import type { Diagnostic } from "../api/types";
import { IconClose, IconError, IconInfo, IconWarning } from "../components/icons";
import type { ConfirmRequest } from "./useProjectDocument";
import "../styles/areas/project.css";

function tabbables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>("button:not([disabled])"));
}

/**
 * An in-page question with its answers as buttons (the app never calls
 * `window.confirm`). It sits above any other modal, such as the gallery:
 * Escape, Tab and clicks outside are its own while it is open.
 *
 * TODO(R-int): move onto the modal stack of components/useModal.ts once it
 * exists; until then it takes Escape and Tab in the capture phase on
 * `window`, before the gallery's own listener sees them.
 */
export function ConfirmDialog({ request }: { request: ConfirmRequest }) {
  const dialog = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const msgId = useId();

  useEffect(() => {
    // Over the gallery (not inert), what had focus there gets it back. Over
    // the app, which is inert by now, the app gives focus back itself once
    // the question has gone (App.tsx, `takeConfirmOpener`).
    const active = document.activeElement;
    const before = active instanceof HTMLElement && active !== document.body && !active.closest("[inert]") ? active : null;
    const root = dialog.current;
    const initial = root?.querySelector<HTMLElement>("[data-initial]") ?? (root ? tabbables(root)[0] : null);
    (initial ?? root)?.focus();
    return () => {
      if (before && before.isConnected && !before.closest("[inert]")) before.focus();
    };
  }, [request.id]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const root = dialog.current;
      if (!root) return;
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        request.resolve(request.cancel);
        return;
      }
      if (e.key !== "Tab") return;
      e.stopPropagation();
      const items = tabbables(root);
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      const inside = root.contains(active) && active !== root;
      if (e.shiftKey && (active === first || !inside)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !inside)) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [request]);

  return (
    <div
      className="modal-backdrop confirm-backdrop"
      onPointerDown={(e) => e.target === e.currentTarget && request.resolve(request.cancel)}
    >
      <div
        className="modal confirm"
        ref={dialog}
        tabIndex={-1}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={msgId}
      >
        <h2 className="title confirm-title" id={titleId}>
          {request.title}
        </h2>
        <p className="body confirm-message" id={msgId}>
          {request.message}
        </p>
        <div className="confirm-actions">
          {request.choices.map((c) => (
            <button
              key={c.value}
              type="button"
              className={`ds-button ds-button--${c.variant}`}
              data-initial={c.initial ? "" : undefined}
              onClick={() => request.resolve(c.value)}
            >
              {c.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

const WORD: Record<Diagnostic["severity"], string> = { error: "Error", warning: "Warning", info: "Note" };

function SeverityIcon({ severity }: { severity: Diagnostic["severity"] }) {
  const cls = "ds-diagnostic-icon";
  if (severity === "error") return <IconError className={cls} />;
  if (severity === "warning") return <IconWarning className={cls} />;
  return <IconInfo className={cls} />;
}

/**
 * The banner's messages for screen readers. Always mounted (a live region
 * that appears with its text already inside is often not read), outside the
 * app so a dialog's `inert` doesn't silence it: errors assertively, notes
 * and warnings politely, each once.
 */
export function ProjectAnnouncer({ messages }: { messages: Diagnostic[] }) {
  const said = (pick: (d: Diagnostic) => boolean) =>
    messages
      .filter(pick)
      .map((d) => `${WORD[d.severity]}: ${d.message}`)
      .join(" ");
  return (
    <>
      <span className="ds-sr" role="alert">
        {said((d) => d.severity === "error")}
      </span>
      <span className="ds-sr" role="status" aria-live="polite" aria-atomic="true">
        {said((d) => d.severity !== "error")}
      </span>
    </>
  );
}

/**
 * Open and save errors, and the notices about an opened file, as block
 * Diagnostics. Not a live region itself: {@link ProjectAnnouncer} says them.
 */
export function ProjectBanner({ messages, onDismiss }: { messages: Diagnostic[]; onDismiss: () => void }) {
  if (messages.length === 0) return null;
  return (
    <div className="project-banner">
      <ul className="ds-diagnostic-list ds-diagnostic-list--block">
        {messages.map((d, k) => (
          <li key={k} className={`ds-diagnostic ds-diagnostic--block ds-diagnostic--${d.severity}`}>
            <SeverityIcon severity={d.severity} />
            <span className="ds-diagnostic-body">
              <span className="ds-diagnostic-word">{WORD[d.severity]}</span>
              <span className="ds-diagnostic-msg">{d.message}</span>
            </span>
          </li>
        ))}
      </ul>
      <button type="button" className="ds-icon-button" aria-label="Dismiss" onClick={onDismiss}>
        <IconClose />
      </button>
    </div>
  );
}
