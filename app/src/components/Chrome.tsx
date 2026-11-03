// The app frame: top bar and bottom status bar.

import { isTauri } from "../api/client";
import { fmt } from "../math/format";
import type { StudioState } from "../state/useStudio";
import { templateById } from "../templates";
import { IconGallery, IconLogo, IconProjector } from "./icons";

interface TopBarProps {
  state: StudioState;
  theme: "dark" | "light";
  onToggleTheme: () => void;
  onOpenGallery: () => void;
}

export function TopBar({ state, theme, onToggleTheme, onOpenGallery }: TopBarProps) {
  const tpl = state.templateId ? templateById(state.templateId) : undefined;
  return (
    <header className="topbar">
      <div className="brand">
        <IconLogo />
        <span className="brand-name">Dynamics Studio</span>
      </div>
      <button type="button" className="btn" onClick={onOpenGallery}>
        <IconGallery size={14} /> Templates
      </button>
      <div className="doc-title" aria-live="polite">
        {tpl ? (
          <>
            <span className="doc-name">{tpl.title}</span>
            <span className="doc-chapter">Strogatz {tpl.chapter}</span>
          </>
        ) : (
          state.compiled && <span className="doc-name">Untitled system</span>
        )}
      </div>
      <button
        type="button"
        className={"btn" + (theme === "light" ? " on" : "")}
        aria-pressed={theme === "light"}
        onClick={onToggleTheme}
        title="Light theme with thicker lines and larger type, for projectors"
      >
        <IconProjector size={14} />
        Projector mode
      </button>
    </header>
  );
}

export function StatusBar({ state }: { state: StudioState }) {
  const p = state.portrait;
  const sys = state.compiled;
  const nTraj = state.ics.length;
  const nFp =
    p && sys && p.meta.systemId === sys.systemId && state.layers.fixedPoints
      ? p.res.fixedPoints.length
      : null;
  // Escaping to infinity is usually the maths; anything else is the solver giving up.
  const problems = p
    ? p.res.trajectories.filter((t) => t.status !== "completed" && t.status !== "blewUp").length
    : 0;
  return (
    <footer className="statusbar">
      <span
        className={"backend " + (isTauri() ? "native" : "preview")}
        title={
          isTauri()
            ? "Computed by the Rust core"
            : "Running in a browser: a small JavaScript stand-in computes the templates"
        }
      >
        <span className="dot" />
        {isTauri() ? "Rust core" : "Browser preview · templates only"}
      </span>
      <span
        className="badge neutral accuracy"
        title="Accuracy checks (comparing against a tighter reference solve) arrive in a later milestone"
      >
        Accuracy: not checked yet
      </span>
      {problems > 0 && (
        <span className="badge warn" title="Open a trajectory in the inspector to see why it stopped">
          {problems} {problems === 1 ? "run" : "runs"} stopped by the solver
        </span>
      )}
      <span className="status-spacer" />
      {sys && (
        <span title="Fixed points are searched for in the visible region only">
          {nTraj} {nTraj === 1 ? "trajectory" : "trajectories"}
          {nFp !== null && ` · ${nFp} fixed ${nFp === 1 ? "point" : "points"} in view`}
        </span>
      )}
      {p && (
        <span className="timing" title="Time spent computing / total time including transfer and drawing">
          compute {fmt(p.res.elapsedMs, 2)} ms · round trip {fmt(p.roundTripMs, 2)} ms
        </span>
      )}
    </footer>
  );
}
