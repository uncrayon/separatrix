import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { isTauri } from "../api/client";
import type { Diagnostic, OpenedProject, ProjectFile } from "../api/types";
import { systemTitle } from "../state/model";
import type { Studio, StudioState } from "../state/useStudio";
import { templateById, type Template } from "../templates";
import {
  chooseOpenPath,
  chooseSavePath,
  closeWindow,
  errorText,
  launchReady,
  listenDocumentEvents,
  readProject,
  setDocumentState,
  writeProject,
} from "./ipc";
import { baseName, documentName, quoted, suggestedProjectName, windowTitle } from "./names";
import { dirtyKey, projectFromState, stateFromProject } from "./serialize";

/** One button of a {@link ConfirmRequest}. */
export interface ConfirmChoice {
  value: string;
  label: string;
  variant: "primary" | "secondary" | "ghost" | "danger";
  /** Focused when the dialog opens. */
  initial?: boolean;
}

/** An in-page question (never `window.confirm`), answered through {@link ConfirmDialog}. */
export interface ConfirmRequest {
  id: number;
  title: string;
  message: string;
  choices: ConfirmChoice[];
  /** The answer for Escape and a click outside. */
  cancel: string;
  resolve: (value: string) => void;
}

export interface ProjectDocumentOptions {
  /**
   * Called before the document may be replaced, ahead of any question: a
   * file is about to open (close the export dialog).
   */
  onBeforeAsk?: () => void;
  /**
   * Called once the document is really being replaced: the unsaved-changes
   * question was answered and, for a file, it was read (close the gallery).
   * A file that can't be read, or Cancel, leaves the gallery as it was.
   */
  onReplaced?: () => void;
  /** Called after a file was read and handed to the studio. */
  onOpened?: (opened: OpenedProject) => void;
}

export type ProjectDocument = ReturnType<typeof useProjectDocument>;

const UNSAVED = (name: string): Omit<ConfirmRequest, "id" | "resolve"> => ({
  title: `Save changes to ${quoted(name)}?`,
  message: "Your changes will be lost if you don't save them.",
  choices: [
    { value: "discard", label: "Don't save", variant: "ghost" },
    { value: "cancel", label: "Cancel", variant: "secondary" },
    { value: "save", label: "Save", variant: "primary", initial: true },
  ],
  cancel: "cancel",
});

const REPLACE = (fileName: string): Omit<ConfirmRequest, "id" | "resolve"> => ({
  title: `${quoted(fileName)} already exists. Replace it?`,
  message: "Replacing it overwrites what is saved there.",
  choices: [
    { value: "cancel", label: "Cancel", variant: "secondary", initial: true },
    { value: "replace", label: "Replace", variant: "danger" },
  ],
  cancel: "cancel",
});

/** The document's name for the title: the file's, else the system's. */
function nameOf(state: StudioState, fileName: string | null): string {
  const template = state.sourceTemplateId ? templateById(state.sourceTemplateId) : undefined;
  const lines = state.equations.map((l) => l.source);
  return documentName(fileName, systemTitle(template, lines, state.format).name);
}

/**
 * The open document: its file, whether it has unsaved changes, and New /
 * Open… / Save / Save as… / close with an in-page confirm before anything
 * unsaved is dropped. Dirty tracking compares what a save would write with
 * the same after the last load or save (`dirtyKey`), so undoing an edit by
 * hand makes the document clean again.
 */
export function useProjectDocument(studio: Studio, opts: ProjectDocumentOptions = {}) {
  const { state, actions } = studio;
  const [path, setPath] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  /** The file as opened or last saved: the base every save merges onto. */
  const [loaded, setLoaded] = useState<ProjectFile | null>(null);
  const [exportSettings, setExportSettings] = useState<unknown>(undefined);
  const [messages, setMessages] = useState<Diagnostic[]>([]);
  const [dismissedWarnings, setDismissedWarnings] = useState<string[] | null>(null);
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);
  /**
   * What had focus when the first of a run of questions was asked, to give
   * focus back to once they are answered (the app is inert while one shows,
   * so the dialog can't do it itself). The app takes it with `takeConfirmOpener`.
   */
  const confirmOpener = useRef<HTMLElement | null>(null);
  const [baseline, setBaseline] = useState<string | null>(null);

  const key = useMemo(
    () => dirtyKey(state, { loaded, pending: state.pendingProject }),
    // Only what a file holds; the portrait and the hover don't change it.
    [
      loaded,
      state.pendingProject,
      state.format,
      state.equations,
      state.compiled,
      state.params,
      state.view,
      state.baseState,
      state.fieldTime,
      state.ics,
      state.layers,
      state.solver,
      state.sourceTemplateId,
    ],
  );
  const dirty = baseline !== null && key !== baseline;
  const name = nameOf(state, fileName);
  const title = windowTitle(name, dirty);

  // The baseline: after a load settles (template, Blank, a file, also one
  // left pending), and after the plot moves the view by itself while clean.
  const prevKey = useRef(key);
  const baselineRef = useRef(baseline);
  baselineRef.current = baseline;
  useLayoutEffect(() => {
    if (state.docEpoch > 0) setBaseline(key);
  }, [state.docEpoch]);
  useLayoutEffect(() => {
    if (state.autoAdjustCount > 0 && baselineRef.current === prevKey.current) setBaseline(key);
  }, [state.autoAdjustCount]);
  useLayoutEffect(() => {
    prevKey.current = key;
  });

  const latest = useRef({ state, loaded, path, fileName, exportSettings, dirty, name, opts });
  latest.current = { state, loaded, path, fileName, exportSettings, dirty, name, opts };

  // Rust guards closing with `dirty`; a layout effect keeps the window between an edit and the flag to one frame.
  useLayoutEffect(() => {
    void setDocumentState({ title, dirty }).catch(() => {});
  }, [title, dirty]);

  const confirmSeq = useRef(0);
  const ask = useCallback((req: Omit<ConfirmRequest, "id" | "resolve">): Promise<string> => {
    if (!confirmOpener.current && document.activeElement instanceof HTMLElement && document.activeElement !== document.body) {
      confirmOpener.current = document.activeElement;
    }
    return new Promise((resolve) => {
      setConfirm((prev) => {
        prev?.resolve(prev.cancel);
        const id = ++confirmSeq.current;
        return {
          ...req,
          id,
          resolve: (value) => {
            setConfirm((cur) => (cur?.id === id ? null : cur));
            resolve(value);
          },
        };
      });
    });
  }, []);

  const writeTo = useCallback(async (target: string): Promise<boolean> => {
    const cur = latest.current;
    const project = projectFromState(cur.state, {
      loaded: cur.loaded,
      pending: cur.state.pendingProject,
      exportSettings: cur.exportSettings,
      appVersion: __APP_VERSION__,
    });
    const savedKey = dirtyKey(cur.state, { loaded: project, pending: cur.state.pendingProject });
    try {
      const saved = await writeProject({ path: target, project });
      setPath(saved.path);
      setFileName(saved.name);
      setLoaded(project);
      setBaseline(savedKey);
      setMessages([]);
      return true;
    } catch (err) {
      setMessages([{ severity: "error", message: errorText(err) }]);
      return false;
    }
  }, []);

  /** Save as…: the dialog, then the in-page replace confirm when Rust appended `.sepx` onto an existing file. */
  const saveAs = useCallback(async (): Promise<boolean> => {
    const cur = latest.current;
    let chosen;
    try {
      chosen = await chooseSavePath({
        suggestedName: suggestedProjectName(cur.fileName, cur.state.sourceTemplateId, cur.name),
        ...(cur.path ? { currentPath: cur.path } : {}),
      });
    } catch (err) {
      setMessages([{ severity: "error", message: errorText(err) }]);
      return false;
    }
    if (!chosen) return false;
    if (chosen.appendedExists && (await ask(REPLACE(baseName(chosen.path)))) !== "replace") return false;
    return writeTo(chosen.path);
  }, [ask, writeTo]);

  const save = useCallback(async (): Promise<boolean> => {
    const target = latest.current.path;
    return target ? writeTo(target) : saveAs();
  }, [saveAs, writeTo]);

  /** True when the document may be replaced: it is clean, or the user saved or chose Don't save. */
  const guard = useCallback(async (): Promise<boolean> => {
    if (!latest.current.dirty) return true;
    const answer = await ask(UNSAVED(latest.current.name));
    if (answer === "save") return save();
    return answer === "discard";
  }, [ask, save]);

  const resetDocument = useCallback(() => {
    setPath(null);
    setFileName(null);
    setLoaded(null);
    setExportSettings(undefined);
    setMessages([]);
  }, []);

  const newFromTemplate = useCallback(
    async (t: Template): Promise<boolean> => {
      if (!(await guard())) return false;
      latest.current.opts.onBeforeAsk?.();
      latest.current.opts.onReplaced?.();
      resetDocument();
      actions.loadTemplate(t);
      return true;
    },
    [actions, guard, resetDocument],
  );

  const newBlank = useCallback(async (): Promise<boolean> => {
    if (!(await guard())) return false;
    latest.current.opts.onBeforeAsk?.();
    latest.current.opts.onReplaced?.();
    resetDocument();
    actions.newBlank();
    return true;
  }, [actions, guard, resetDocument]);

  /** Reads `target` and hands it to the studio; asks first unless `guarded` (the caller did). */
  const openPath = useCallback(
    async (target: string, guarded = false): Promise<boolean> => {
      latest.current.opts.onBeforeAsk?.();
      if (!guarded && !(await guard())) return false;
      let opened: OpenedProject;
      try {
        opened = await readProject({ path: target });
      } catch (err) {
        setMessages([{ severity: "error", message: errorText(err) }]);
        return false;
      }
      setPath(opened.path);
      setFileName(opened.name);
      setLoaded(opened.project);
      setExportSettings(opened.project.export_settings);
      setMessages(opened.warnings.map((message) => ({ severity: "warning", message })));
      setDismissedWarnings(null);
      latest.current.opts.onReplaced?.();
      actions.loadProject(stateFromProject(opened.project));
      latest.current.opts.onOpened?.(opened);
      return true;
    },
    [actions, guard],
  );

  const open = useCallback(async (): Promise<boolean> => {
    latest.current.opts.onBeforeAsk?.();
    if (!(await guard())) return false;
    let target: string | null;
    try {
      target = await chooseOpenPath();
    } catch (err) {
      setMessages([{ severity: "error", message: errorText(err) }]);
      return false;
    }
    return target ? openPath(target, true) : false;
  }, [guard, openPath]);

  /** The window was asked to close while dirty (Rust held it): ask, then close for good. */
  const requestClose = useCallback(async (): Promise<boolean> => {
    if (!(await guard())) return false;
    await closeWindow();
    return true;
  }, [guard]);

  // Files from the command line, a second instance, macOS Open or a drop.
  const handlers = useRef({ openPath, requestClose });
  handlers.current = { openPath, requestClose };
  useEffect(() => {
    let off: (() => void) | null = null;
    let live = true;
    void listenDocumentEvents({
      onOpenRequested: (p) => void handlers.current.openPath(p),
      onCloseRequested: () => void handlers.current.requestClose(),
    }).then(async (detach) => {
      if (!live) return detach();
      off = detach;
      const queued = await launchReady().catch(() => [] as string[]);
      if (live && queued.length) void handlers.current.openPath(queued[0]);
    });
    return () => {
      live = false;
      off?.();
    };
  }, []);

  // The browser preview: the tab's own close/reload prompt. Not under
  // automation, where it would hold up `page.reload()`.
  useEffect(() => {
    if (isTauri() || !dirty || (typeof navigator !== "undefined" && navigator.webdriver)) return;
    const onUnload = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", onUnload);
    return () => window.removeEventListener("beforeunload", onUnload);
  }, [dirty]);

  // The banner: open errors and Rust's warnings, the studio's notices
  // about the load, and a note while the file's system doesn't compile.
  const loadWarnings = state.loadWarnings;
  const banner: Diagnostic[] = useMemo(() => {
    const out = [...messages];
    if (loadWarnings !== dismissedWarnings) {
      // Rust and the studio can both judge a view unfit (a saved cobweb of a
      // system that isn't a 1-D map) in the same words: say it once.
      for (const message of loadWarnings)
        if (!out.some((d) => d.message === message)) out.push({ severity: "warning", message });
    }
    if (state.pendingProject && !state.compileOk) {
      out.push({
        severity: "info",
        message:
          "The equations don't compile yet. The file's parameters, start points and view come back once they do, and saving keeps them.",
      });
    }
    return out;
  }, [messages, loadWarnings, dismissedWarnings, state.pendingProject, state.compileOk]);
  const dismissBanner = useCallback(() => {
    setMessages([]);
    setDismissedWarnings(latest.current.state.loadWarnings);
  }, []);

  return {
    path,
    fileName,
    name,
    title,
    dirty,
    /** The opened file's system hasn't compiled yet. */
    pending: state.pendingProject !== null,
    banner,
    dismissBanner,
    exportSettings,
    setExportSettings,
    confirm,
    /** The element to focus once the questions are answered (see `confirmOpener`); clears it. */
    takeConfirmOpener: () => {
      const el = confirmOpener.current;
      confirmOpener.current = null;
      return el;
    },
    guard,
    newFromTemplate,
    newBlank,
    open,
    openPath,
    save,
    saveAs,
    requestClose,
  };
}
