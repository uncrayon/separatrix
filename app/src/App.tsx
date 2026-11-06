import { lazy, Suspense, useCallback, useLayoutEffect, useRef, useState } from "react";
import { isTauri } from "./api/client";
import { BranchCanvas } from "./canvas/BranchCanvas";
import { LyapunovCanvas } from "./canvas/LyapunovCanvas";
import { OrbitCanvas } from "./canvas/OrbitCanvas";
import { PortraitCanvas } from "./canvas/PortraitCanvas";
import { CanvasToolbar } from "./components/CanvasToolbar";
import { StatusBar, TopBar } from "./components/Chrome";
import { Inspector } from "./components/Inspector";
import { SystemPanel } from "./components/SystemPanel";
import { TemplateGallery } from "./components/TemplateGallery";
import { TooltipLayer } from "./components/Tooltip";
import { ExportDialog } from "./export/ExportDialog";
import { ConfirmDialog, ProjectAnnouncer, ProjectBanner } from "./project/ConfirmDialog";
import { projectStem } from "./project/names";
import { useProjectDocument } from "./project/useProjectDocument";
import { useShortcuts } from "./project/useShortcuts";
import { useStudio } from "./state/useStudio";
import { isContinuation, isLyapunov, isOrbit, isSpace } from "./state/model";
import type { Template } from "./templates";

type Theme = "dark" | "light";

const SpaceCanvas = lazy(() => import("./canvas/SpaceCanvas"));

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
  }
}

const activeElement = () => (document.activeElement instanceof HTMLElement ? document.activeElement : null);
const focusPlot = () => document.querySelector<HTMLElement>(".plot-canvas")?.focus();

export default function App() {
  const studio = useStudio();
  const { state, actions } = studio;
  const [theme, setTheme] = useState<Theme>(() => (readStored(THEME_KEY) === "light" ? "light" : "dark"));
  const [galleryOpen, setGalleryOpen] = useState(true);
  const [exportOpen, setExportOpen] = useState(false);
  const opener = useRef<HTMLElement | null>(null);
  const exportOpener = useRef<HTMLElement | null>(null);
  const focusAfterClose = useRef<() => void>(() => {});
  const galleryOpenRef = useRef(galleryOpen);
  galleryOpenRef.current = galleryOpen;

  useLayoutEffect(() => {
    if (galleryOpen) return;
    // The app was inert while the dialog was open; focus it only now.
    const f = focusAfterClose.current;
    focusAfterClose.current = () => {};
    f();
  }, [galleryOpen]);

  // The export dialog hands focus back itself, but `.app` is still inert at
  // that moment (its cleanup runs before the attribute changes); do it here.
  useLayoutEffect(() => {
    if (exportOpen) return;
    const back = exportOpener.current;
    exportOpener.current = null;
    if (!back || galleryOpenRef.current) return;
    if (back.isConnected && back !== document.body) back.focus();
    // Some elements (a MathLive field) don't take focus this way: the plot then.
    if (document.activeElement !== back) focusPlot();
  }, [exportOpen]);

  const openGallery = useCallback(() => {
    opener.current = activeElement();
    setGalleryOpen(true);
  }, []);
  const closeGalleryTo = useCallback((target: "plot" | "opener") => {
    focusAfterClose.current = () => {
      const back = opener.current;
      if (target === "opener" && back && back.isConnected && back !== document.body) back.focus();
      else focusPlot();
    };
    setGalleryOpen(false);
  }, []);

  const openExport = useCallback(() => {
    if (galleryOpenRef.current) return;
    exportOpener.current = activeElement();
    setExportOpen(true);
  }, []);
  const closeExport = useCallback(() => setExportOpen(false), []);

  // Before a file opens (and before asking about unsaved changes): the
  // export dialog closes.
  const beforeAsk = useCallback(() => {
    // A file replacing the document while exporting: focus goes to the plot.
    if (exportOpener.current) exportOpener.current = document.body;
    setExportOpen(false);
  }, []);
  // Once the document is really replaced (the question answered, the file
  // read): the gallery closes, as a pick from it does. A file that can't be
  // read leaves it open, with the reason.
  const replaced = useCallback(() => {
    if (galleryOpenRef.current) closeGalleryTo("plot");
  }, [closeGalleryTo]);

  const doc = useProjectDocument(studio, { onBeforeAsk: beforeAsk, onReplaced: replaced });
  const exportOpenRef = useRef(exportOpen);
  exportOpenRef.current = exportOpen;

  // The in-page confirm can't give focus back itself: `.app` is inert until
  // it has gone. Back to what had focus when it was asked, else the plot.
  const confirmOpen = doc.confirm !== null;
  const takeConfirmOpener = doc.takeConfirmOpener;
  useLayoutEffect(() => {
    if (confirmOpen) return;
    const back = takeConfirmOpener();
    // Over the gallery or the export dialog, those keep focus themselves.
    if (galleryOpenRef.current || exportOpenRef.current) return;
    if (document.activeElement && document.activeElement !== document.body) return;
    if (back && back.isConnected && !back.closest("[inert]")) back.focus();
    if (back && document.activeElement !== back) focusPlot();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [confirmOpen]);

  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme;
    writeStored(THEME_KEY, theme);
  }, [theme]);

  // Gallery picks and Blank go through the document, which asks first when
  // there are unsaved changes; on Cancel the gallery stays open.
  const pick = useCallback((t: Template) => void doc.newFromTemplate(t), [doc.newFromTemplate]);
  const blank = useCallback(() => void doc.newBlank(), [doc.newBlank]);
  const closeGallery = useCallback(() => closeGalleryTo("opener"), [closeGalleryTo]);

  const newSystem = useCallback(() => {
    void doc.newBlank().then((ok) => {
      if (ok) requestAnimationFrame(focusPlot);
    });
  }, [doc.newBlank]);
  const openFile = useCallback(() => void doc.open(), [doc.open]);
  const save = useCallback(() => void doc.save(), [doc.save]);
  const saveAs = useCallback(() => void doc.saveAs(), [doc.saveAs]);

  const canExport = state.compiled !== null;
  useShortcuts(
    {
      new: newSystem,
      open: openFile,
      save,
      saveAs,
      export: () => {
        if (canExport) openExport();
      },
    },
    { tauri: isTauri() },
  );

  const modal = galleryOpen || exportOpen || doc.confirm !== null;

  return (
    <>
      <div className="app" inert={modal}>
        <TopBar
          state={state}
          doc={{ name: doc.name, hasFile: doc.fileName !== null, dirty: doc.dirty }}
          theme={theme}
          onToggleTheme={() => setTheme((t) => (t === "dark" ? "light" : "dark"))}
          onOpenGallery={openGallery}
          onNew={newSystem}
          onOpen={openFile}
          onSave={save}
          onSaveAs={saveAs}
          onExport={openExport}
          exportDisabled={!canExport}
        />
        <div className="workspace">
          <SystemPanel state={state} actions={actions} />
          <main className="canvas-area">
            {isSpace(state.view) ? (
              <Suspense fallback={<div className="plot-wrap" />}>
                <SpaceCanvas
                  state={state}
                  actions={actions}
                  toolbar={<CanvasToolbar state={state} actions={actions} />}
                />
              </Suspense>
            ) : isLyapunov(state.view) ? (
              <LyapunovCanvas state={state} actions={actions} toolbar={<CanvasToolbar state={state} actions={actions} />} />
            ) : isOrbit(state.view) ? (
              <OrbitCanvas state={state} actions={actions} toolbar={<CanvasToolbar state={state} actions={actions} />} />
            ) : isContinuation(state.view) ? (
              <BranchCanvas state={state} actions={actions} toolbar={<CanvasToolbar state={state} actions={actions} />} />
            ) : (
              <PortraitCanvas
                state={state}
                view={state.view}
                actions={actions}
                theme={theme}
                toolbar={<CanvasToolbar state={state} actions={actions} />}
              />
            )}
            {doc.banner.length > 0 && !galleryOpen && (
              <div className="project-banner-dock">
                <ProjectBanner messages={doc.banner} onDismiss={doc.dismissBanner} />
              </div>
            )}
          </main>
          <Inspector state={state} actions={actions} />
        </div>
        <StatusBar state={state} actions={actions} />
      </div>
      {/* Outside `.app`, which is inert under a dialog: always there, so a
          new message is announced (errors assertively). */}
      <ProjectAnnouncer messages={doc.banner} />
      {galleryOpen && (
        <TemplateGallery
          onPick={pick}
          onBlank={blank}
          onClose={closeGallery}
          canClose={state.compiled !== null}
          notice={
            doc.banner.length > 0 ? <ProjectBanner messages={doc.banner} onDismiss={doc.dismissBanner} /> : null
          }
        />
      )}
      {exportOpen && !galleryOpen && (
        <ExportDialog
          state={state}
          fileStem={doc.fileName ? projectStem(doc.fileName) : undefined}
          settings={doc.exportSettings}
          onSettingsChange={doc.setExportSettings}
          onClose={closeExport}
        />
      )}
      {doc.confirm && <ConfirmDialog request={doc.confirm} />}
      <TooltipLayer />
    </>
  );
}
