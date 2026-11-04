import { useEffect, useRef, useState } from "react";
import { Tex } from "../math/Tex";
import { TEMPLATES, strogatzRef, type Template } from "../templates";
import { tipDismiss } from "./Chrome";
import { IconClose, IconPlus } from "./icons";
import { TemplateThumb, compileTemplate } from "./TemplateThumb";

interface Props {
  onPick: (t: Template) => void;
  onBlank: () => void;
  onClose: () => void;
  canClose: boolean;
}

function tabbables(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), [tabindex="0"]'),
  );
}

/** "2D" and "ODE" (or "Map") as the compiler understood the template. */
function TemplateTags({ template }: { template: Template }) {
  const [tags, setTags] = useState<[string, string] | null>(null);
  useEffect(() => {
    let live = true;
    compileTemplate(template)
      .then((sys) => {
        if (live && sys.ok) setTags([`${sys.states.length}D`, sys.kind === "map" ? "Map" : "ODE"]);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [template]);
  return (
    <span className="ds-template-card-tags">
      {tags?.map((tag) => (
        <span className="ds-template-card-tag" key={tag}>
          {tag}
        </span>
      ))}
    </span>
  );
}

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
        aria-describedby="gallery-sub"
      >
        <header className="gallery-head">
          <div className="gallery-head-text">
            <h1 id="gallery-title" className="display">
              Start with a system
            </h1>
            <p id="gallery-sub" className="body gallery-sub">
              Each one opens with trajectories already placed. Edit the equations or drag anything afterwards.
            </p>
          </div>
          {canClose && (
            <button
              type="button"
              className="ds-icon-button"
              aria-label="Close gallery"
              data-tip="Close gallery"
              data-tip-align="end"
              onClick={onClose}
              {...tipDismiss}
            >
              <IconClose />
            </button>
          )}
        </header>
        <div className="gallery-grid">
          <ul className="ds-template-grid template-grid" aria-label="Templates">
            {TEMPLATES.map((t) => {
              const id = `tpl-${t.id}`;
              return (
                <li key={t.id}>
                  <button
                    type="button"
                    className="ds-template-card card"
                    aria-labelledby={`${id}-name`}
                    aria-describedby={`${id}-eq ${id}-desc`}
                    onClick={() => onPick(t)}
                  >
                    <TemplateThumb template={t} />
                    <span className="ds-template-card-text">
                      <span className="ds-template-card-name" id={`${id}-name`}>
                        {t.title}
                      </span>
                      <span className="ds-template-card-math" id={`${id}-eq`}>
                        {t.equations.map((e, k) => (
                          <span key={k}>
                            <Tex latex={e} />
                          </span>
                        ))}
                      </span>
                    </span>
                    <span className="ds-template-card-desc" id={`${id}-desc`}>
                      {t.description}
                    </span>
                    <span className="ds-template-card-meta">
                      <TemplateTags template={t} />
                      {strogatzRef(t) && <span className="ds-template-card-ref">{strogatzRef(t)}</span>}
                    </span>
                  </button>
                </li>
              );
            })}
            <li>
              <button
                type="button"
                className="ds-template-card ds-template-card--blank card blank"
                aria-labelledby="tpl-blank-name"
                aria-describedby="tpl-blank-eq tpl-blank-foot"
                onClick={onBlank}
              >
                <span className="ds-template-card-blank" aria-hidden="true">
                  <IconPlus />
                </span>
                <span className="ds-template-card-text">
                  <span className="ds-template-card-name" id="tpl-blank-name">
                    Blank system
                  </span>
                  <span className="ds-template-card-math" id="tpl-blank-eq">
                    <span>
                      <Tex latex="\dot{x}=y" />
                    </span>
                    <span>
                      <Tex latex="\dot{y}=-x" />
                    </span>
                  </span>
                </span>
                <span className="ds-template-card-meta">
                  <span className="ds-template-card-ref" id="tpl-blank-foot">
                    Type your own equations
                  </span>
                </span>
              </button>
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}
