// The template gallery: opens on first run, one click loads a full portrait.

import { useEffect, useRef } from "react";
import { Tex } from "../math/Tex";
import { TEMPLATES, type Template } from "../templates";
import { IconClose, IconPlus } from "./icons";

interface Props {
  onPick: (t: Template) => void;
  onBlank: () => void;
  onClose: () => void;
  /** Hide the close button when there is nothing behind the dialog yet. */
  canClose: boolean;
}

/** Everything inside `root` that Tab can reach, in document order. */
function tabbables(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), [tabindex="0"]'),
  );
}

/**
 * The gallery is a modal dialog: the app behind it is made `inert` by App.tsx,
 * and Tab / Shift+Tab wrap around inside the dialog here, so keyboard focus
 * can never land on controls hidden behind the backdrop.
 */
export function TemplateGallery({ onPick, onBlank, onClose, canClose }: Props) {
  const dialog = useRef<HTMLDivElement>(null);
  useEffect(() => {
    dialog.current?.focus();
  }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && canClose) onClose();
      const root = dialog.current;
      if (e.key !== "Tab" || !root) return;
      const items = tabbables(root);
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      const outside = !root.contains(active) || active === root;
      if (e.shiftKey && (active === first || outside)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !root.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [canClose, onClose]);

  return (
    <div
      className="modal-backdrop"
      onPointerDown={(e) => e.target === e.currentTarget && canClose && onClose()}
    >
      <div
        className="modal gallery"
        ref={dialog}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="gallery-title"
      >
        <header className="gallery-head">
          <div>
            <h1 id="gallery-title">Start from a classic system</h1>
            <p>
              Each one opens with trajectories already placed. Edit the equations or drag anything afterwards.
            </p>
          </div>
          {canClose && (
            <button type="button" className="icon-btn" aria-label="Close gallery" onClick={onClose}>
              <IconClose />
            </button>
          )}
        </header>
        <div className="gallery-grid">
          {TEMPLATES.map((t) => (
            <button key={t.id} type="button" className="card" onClick={() => onPick(t)}>
              <div className="card-top">
                <span className="card-title">{t.title}</span>
                <span className="card-chapter">{t.chapter}</span>
              </div>
              <div className="card-math">
                {t.equations.map((e, k) => (
                  <div key={k}>
                    <Tex latex={e} />
                  </div>
                ))}
              </div>
              <p className="card-desc">{t.description}</p>
            </button>
          ))}
          <button type="button" className="card blank" onClick={onBlank}>
            <IconPlus size={20} />
            <span className="card-title">Blank system</span>
            <p className="card-desc">Start from ẋ = y, ẏ = −x and type your own.</p>
          </button>
        </div>
      </div>
    </div>
  );
}
