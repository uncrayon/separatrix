import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import type { RecentProject } from "../api/types";
import { Tex } from "../math/Tex";
import { isMacPlatform, shortcutLabel } from "../project/shortcuts";
import { TEMPLATES, strogatzRef, templateChapters, type Template } from "../templates";
import { tipDismiss } from "./Chrome";
import { Button } from "./controls";
import { IconClose, IconOpen, IconPlus } from "./icons";
import { TemplateThumb, compileTemplate } from "./TemplateThumb";

interface Props {
  onPick: (t: Template) => void;
  onBlank: () => void;
  onClose: () => void;
  /** Open… (the file dialog, after the unsaved-changes confirm). */
  onOpen: () => void;
  /** Projects opened or saved lately, most recent first. */
  recent: RecentProject[];
  onOpenRecent: (path: string) => void;
  canClose: boolean;
  /** Shown under the heading: why a file passed at launch didn't open, say. */
  notice?: ReactNode;
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

const CHAPTERS = templateChapters(TEMPLATES);

const MIN_MATH_SCALE = 0.6;

function useFitLines(ref: RefObject<HTMLElement | null>) {
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const fit = () => {
      for (const line of Array.from(el.children) as HTMLElement[]) {
        line.style.fontSize = "";
        const over = line.scrollWidth / Math.max(1, line.clientWidth);
        if (over > 1) line.style.fontSize = `${Math.max(MIN_MATH_SCALE, 1 / over)}em`;
      }
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    let live = true;
    document.fonts?.ready.then(() => live && fit());
    return () => {
      live = false;
      ro.disconnect();
    };
  }, [ref]);
}

function TemplateCardButton({ template: t, onPick }: { template: Template; onPick: (t: Template) => void }) {
  const id = `tpl-${t.id}`;
  const math = useRef<HTMLSpanElement>(null);
  useFitLines(math);
  return (
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
        <span className="ds-template-card-math" id={`${id}-eq`} ref={math}>
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
  );
}

const MAC = isMacPlatform(typeof navigator === "undefined" ? undefined : navigator);
const OPEN_KEYS = shortcutLabel("o", { mac: MAC });

function RecentProjects({ recent, onOpenRecent }: { recent: RecentProject[]; onOpenRecent: (path: string) => void }) {
  return (
    <section className="gallery-chapter" aria-labelledby="gallery-recent">
      <ChapterHeading id="gallery-recent" chapter="Recent" title="" />
      <ul className="gallery-recent">
        {recent.map((r) => (
          <li key={r.path}>
            <button type="button" className="gallery-recent-row" title={r.path} onClick={() => onOpenRecent(r.path)}>
              <span className="body-strong gallery-recent-name">{r.name}</span>
              <span className="caption gallery-recent-folder">{r.folder}</span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

function ChapterHeading({ id, chapter, title }: { id: string; chapter: string; title: string }) {
  return (
    <h2 className="heading gallery-chapter-head" id={id}>
      <span>{chapter}</span>
      {title && <span className="body gallery-chapter-title">{title}</span>}
    </h2>
  );
}

export function TemplateGallery({ onPick, onBlank, onClose, onOpen, recent, onOpenRecent, canClose, notice }: Props) {
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
        data-shortcuts="open"
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
          <div className="gallery-head-actions">
            <Button
              variant="secondary"
              icon={<IconOpen />}
              aria-keyshortcuts={MAC ? "Meta+O" : "Control+O"}
              data-tooltip={`Open a project (${OPEN_KEYS})`}
              data-tooltip-desc="A .sepx file you or a colleague saved."
              onClick={onOpen}
            >
              Open…
            </Button>
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
          </div>
        </header>
        {notice && <div className="gallery-notice">{notice}</div>}
        <div className="gallery-grid">
          {recent.length > 0 && <RecentProjects recent={recent} onOpenRecent={onOpenRecent} />}
          {CHAPTERS.map((ch) => (
            <section className="gallery-chapter" key={ch.chapter} aria-labelledby={`gallery-ch-${ch.number}`}>
              <ChapterHeading id={`gallery-ch-${ch.number}`} chapter={`Chapter ${ch.number}`} title={ch.title} />
              <ul className="ds-template-grid template-grid">
                {ch.templates.map((t) => (
                  <li key={t.id}>
                    <TemplateCardButton template={t} onPick={onPick} />
                  </li>
                ))}
              </ul>
            </section>
          ))}
          <section className="gallery-chapter" aria-labelledby="gallery-own">
            <ChapterHeading id="gallery-own" chapter="Your own system" title="" />
            <ul className="ds-template-grid template-grid">
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
          </section>
        </div>
      </div>
    </div>
  );
}
