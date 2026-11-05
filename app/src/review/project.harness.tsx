// The e2e harness for project files (review/project.e2e.cjs): the real
// studio state (useStudio) and document (useProjectDocument) over the
// browser preview's mocks, with the confirm and banner in the DOM, and an
// API on `window.__projectHarness` that waits for loads to settle. Vite
// serves it at /src/review/project.harness.html; the build never includes it.

import { StrictMode, useLayoutEffect, useRef } from "react";
import ReactDOM from "react-dom/client";
import "../styles/ds/tokens.css";
import "../styles/ds/components.css";
import "../styles/app.css";
import { isTauri } from "../api/client";
import { ConfirmDialog, ProjectBanner } from "../project/ConfirmDialog";
import { icUid } from "../project/serialize";
import { useProjectDocument, type ProjectDocument } from "../project/useProjectDocument";
import { useShortcuts } from "../project/useShortcuts";
import { useStudio, type Studio } from "../state/useStudio";
import { BLANK_TEMPLATE, TEMPLATES, templateById } from "../templates";

interface Live {
  studio: Studio;
  doc: ProjectDocument;
}

const live: { current: Live | null } = { current: null };
const listeners = new Set<() => void>();

function waitFor(pred: () => boolean, what: string, ms = 5000): Promise<void> {
  return new Promise((resolve, reject) => {
    if (pred()) return resolve();
    const timer = window.setTimeout(() => {
      listeners.delete(check);
      reject(new Error(`timed out waiting for ${what}`));
    }, ms);
    function check() {
      if (!pred()) return;
      window.clearTimeout(timer);
      listeners.delete(check);
      resolve();
    }
    listeners.add(check);
  });
}

const cur = (): Live => {
  if (!live.current) throw new Error("harness not mounted");
  return live.current;
};

/** Runs `act` and waits until the load it starts has settled (`docEpoch` moved). */
async function settled(act: () => Promise<boolean> | boolean | void, what: string): Promise<boolean> {
  const epoch = cur().studio.state.docEpoch;
  const ok = await act();
  if (ok === false) return false;
  await waitFor(() => cur().studio.state.docEpoch > epoch, what);
  // Let layout effects (the baseline) run.
  await new Promise((r) => requestAnimationFrame(() => r(null)));
  return true;
}

const template = (id: string) => (id === "blank" ? BLANK_TEMPLATE : templateById(id)!);

const api = {
  templateIds: () => [...TEMPLATES.map((t) => t.id), "blank"],
  newFromTemplate: (id: string) =>
    settled(() => (id === "blank" ? cur().doc.newBlank() : cur().doc.newFromTemplate(template(id))), `template ${id}`),
  openPath: (path: string) => settled(() => cur().doc.openPath(path), `open ${path}`),
  save: () => cur().doc.save(),
  saveAs: () => cur().doc.saveAs(),
  /** Starts an action that may ask; the e2e answers in the DOM, then awaits `__projectHarness.last`. */
  start(name: "newBlank" | "save" | "saveAs" | "open" | "requestClose" | "newFromTemplate", arg?: string) {
    const d = cur().doc;
    const p =
      name === "newFromTemplate" ? d.newFromTemplate(template(arg!)) : (d[name] as () => Promise<boolean>)();
    (window as unknown as { __projectLast: Promise<boolean> }).__projectLast = p;
  },
  async setFormat(format: "latex" | "text") {
    await cur().studio.actions.setFormat(format);
    await waitFor(() => cur().studio.state.format === format && cur().studio.state.compileOk, `format ${format}`);
  },
  setParam: (name: string, v: number) => cur().studio.actions.setParam(name, v),
  /** The plot moving the view by itself (PortraitCanvas makes room under its toolbar). */
  autoAdjust() {
    const { state, actions } = cur().studio;
    const v = state.view;
    actions.autoAdjustView({ ...v, yRange: [v.yRange[0], v.yRange[1] + 0.5] });
  },
  /** Hides the last start point and moves the slice and field time, so a file holds more than a template. */
  editMore() {
    const { state, actions } = cur().studio;
    const last = state.ics[state.ics.length - 1];
    if (last) actions.setIcHidden(last.id, true);
    state.baseState.forEach((_, i) => actions.setSlice(i, 0.25 * (i + 1)));
    if (state.compiled?.kind !== "map") actions.setFieldTime(0.75);
  },
  async setLine(index: number, source: string) {
    const line = cur().studio.state.equations[index];
    cur().studio.actions.setEquation(line.id, source);
  },
  waitCompiled: (ok: boolean) =>
    waitFor(() => cur().studio.state.compileOk === ok && !cur().studio.state.converting, `compileOk=${ok}`),
  waitPendingApplied: () => waitFor(() => cur().studio.state.pendingProject === null, "pending load applied"),
  snapshot() {
    const { state } = cur().studio;
    return {
      format: state.format,
      lines: state.equations.map((l) => l.source),
      template: state.sourceTemplateId,
      params: state.params,
      view: state.view,
      baseState: state.baseState,
      fieldTime: state.fieldTime,
      ics: state.ics.map((ic) => ({ uid: icUid(ic), x0: ic.x0, t0: ic.t0, color: ic.color, hidden: !!ic.hidden })),
      layers: state.layers,
      solver: state.solver,
      kind: state.compiled?.kind ?? null,
    };
  },
  doc() {
    const d = cur().doc;
    return { path: d.path, fileName: d.fileName, name: d.name, title: d.title, dirty: d.dirty, pending: d.pending };
  },
  banner: () => cur().doc.banner,
};

function Harness() {
  const studio = useStudio();
  const doc = useProjectDocument(studio);
  useShortcuts(
    {
      save: () => void doc.save(),
      saveAs: () => void doc.saveAs(),
      open: () => void doc.open(),
      new: () => void doc.newBlank(),
    },
    { tauri: isTauri() },
  );
  const first = useRef(true);
  live.current = { studio, doc };
  useLayoutEffect(() => {
    if (first.current) {
      first.current = false;
      (window as unknown as { __projectHarness: typeof api }).__projectHarness = api;
    }
    for (const l of [...listeners]) l();
  });
  return (
    <main>
      <p className="harness-title">{doc.title}</p>
      <ProjectBanner messages={doc.banner} onDismiss={doc.dismissBanner} />
      {doc.confirm && <ConfirmDialog request={doc.confirm} />}
    </main>
  );
}

// StrictMode, as in main.tsx: effects mount twice in development.
ReactDOM.createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Harness />
  </StrictMode>,
);
