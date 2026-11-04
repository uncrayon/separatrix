// App layout (Figma-like): top bar · system panel | canvas | inspector · status bar.

import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { PortraitCanvas } from "./canvas/PortraitCanvas";
import { CanvasToolbar } from "./components/CanvasToolbar";
import { StatusBar, TopBar } from "./components/Chrome";
import { Inspector } from "./components/Inspector";
import { SystemPanel } from "./components/SystemPanel";
import { TemplateGallery } from "./components/TemplateGallery";
import { TooltipLayer } from "./components/Tooltip";
import { useStudio } from "./state/useStudio";
import type { Template } from "./templates";

type Theme = "dark" | "light";

// Small per-viewer conveniences; the app works the same without storage.
const THEME_KEY = "separatrix.theme";
function readStored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function writeStored(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* storage unavailable: ignore */
  }
}

export default function App() {
  const { state, actions } = useStudio();
  const [theme, setTheme] = useState<Theme>(() => (readStored(THEME_KEY) === "light" ? "light" : "dark"));
  // First run: open the gallery so the next step is obvious.
  const [galleryOpen, setGalleryOpen] = useState(true);
  /** What had focus when the gallery opened, to give it back on close. */
  const opener = useRef<HTMLElement | null>(null);
  /** Where focus goes once the gallery is gone (after `inert` is lifted). */
  const focusAfterClose = useRef<() => void>(() => {});

  useLayoutEffect(() => {
    if (galleryOpen) return;
    // The app was inert while the dialog was open; focus it only now.
    const f = focusAfterClose.current;
    focusAfterClose.current = () => {};
    f();
  }, [galleryOpen]);

  const openGallery = useCallback(() => {
    opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setGalleryOpen(true);
  }, []);
  /** Close the gallery; a loaded system sends focus to the plot, Escape back to the opener. */
  const closeGalleryTo = useCallback((target: "plot" | "opener") => {
    focusAfterClose.current = () => {
      const back = opener.current;
      if (target === "opener" && back && back.isConnected && back !== document.body) back.focus();
      else document.querySelector<HTMLElement>(".plot-canvas")?.focus();
    };
    setGalleryOpen(false);
  }, []);

  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme;
    writeStored(THEME_KEY, theme);
  }, [theme]);

  const pick = useCallback(
    (t: Template) => {
      actions.loadTemplate(t);
      closeGalleryTo("plot");
    },
    [actions, closeGalleryTo],
  );
  const blank = useCallback(() => {
    actions.newBlank();
    closeGalleryTo("plot");
  }, [actions, closeGalleryTo]);
  const closeGallery = useCallback(() => closeGalleryTo("opener"), [closeGalleryTo]);

  return (
    <>
      {/* `inert` while the gallery is open: no focus, clicks or screen-reader
          access to what is behind the modal backdrop. */}
      <div className="app" inert={galleryOpen}>
        <TopBar
          state={state}
          theme={theme}
          onToggleTheme={() => setTheme((t) => (t === "dark" ? "light" : "dark"))}
          onOpenGallery={openGallery}
        />
        <div className="workspace">
          <SystemPanel state={state} actions={actions} />
          <main className="canvas-area">
            <PortraitCanvas
              state={state}
              actions={actions}
              theme={theme}
              toolbar={<CanvasToolbar state={state} actions={actions} />}
            />
          </main>
          <Inspector state={state} actions={actions} />
        </div>
        <StatusBar state={state} actions={actions} theme={theme} />
      </div>
      {galleryOpen && (
        <TemplateGallery
          onPick={pick}
          onBlank={blank}
          onClose={closeGallery}
          canClose={state.compiled !== null}
        />
      )}
      <TooltipLayer />
    </>
  );
}
