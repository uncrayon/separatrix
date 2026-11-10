import { useEffect, useRef, useState, type KeyboardEvent, type SyntheticEvent } from "react";
import { isTauri } from "../api/client";
import type { AxisRef, StateInfo, SystemKind } from "../api/types";
import { insidePlot, plotViewport, toPlot, unitsPerPx } from "../canvas/viewport";
import { useOrbitDrawMs } from "../canvas/OrbitCanvas";
import { axisLabelLatex, fmt, viewAxisLabels, plural } from "../math/format";
import { Tex } from "../math/Tex";
import { METHODS, STIFF_HINT } from "../state/methods";
import {
  continuationSummary,
  countText,
  isContinuation,
  isParameterView,
  isAnalysis,
  isCurves,
  isLyapunov,
  isOrbit,
  isReturnMap,
  lyapunovCurrent,
  orbitCurrent,
  lyapunovSolver,
  renormalizations,
  searchedFixedPoints,
  shownBranches,
  systemTitle,
  type LyapunovSettings,
  type OrbitSettings,
  type SolverSettings,
} from "../state/model";
import type { StudioActions, StudioState } from "../state/useStudio";
import { buildCurveJobs, curvesSummary } from "../state/curves";
import { templateRef, templateById } from "../templates";
import { isMacPlatform, shortcutLabel } from "../project/shortcuts";
import {
  IconCheck,
  IconError,
  IconExport,
  IconGallery,
  IconLogo,
  IconMoon,
  IconNewDoc,
  IconOpen,
  IconProjector,
  IconSave,
  IconSaveAs,
  IconWarning,
} from "./icons";
import { diagnosticLine } from "./diagnostics";
import { looksStiff, sectionCounts } from "./runStatus";
import { UpdateButton } from "../update/UpdateButton";

type Theme = "dark" | "light";

/** Escape hides an open IconButton tooltip without moving focus (WCAG 1.4.13). */
export const tipDismiss = {
  onKeyDown: (e: KeyboardEvent<HTMLElement>) => {
    if (e.key === "Escape") e.currentTarget.classList.add("is-tip-dismissed");
  },
  onPointerLeave: (e: SyntheticEvent<HTMLElement>) => e.currentTarget.classList.remove("is-tip-dismissed"),
  onBlur: (e: SyntheticEvent<HTMLElement>) => e.currentTarget.classList.remove("is-tip-dismissed"),
};

const minus = (s: string) => s.replace(/-/g, "−");


/** The open document as the top bar shows it (from useProjectDocument). */
export interface TopBarDocument {
  /** The file's stem once it has one, otherwise the system's title. */
  name: string;
  /** The document has a file (opened or saved). */
  hasFile: boolean;
  /** Unsaved changes. */
  dirty: boolean;
}

/** The document actions; each may ask first (in-page) when there are unsaved changes. */
export interface TopBarActions {
  onNew: () => void;
  onOpen: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  onExport: () => void;
  /** Export… is unavailable (nothing compiled yet). */
  exportDisabled?: boolean;
  /** Why export is off, for its tooltip. */
  exportReason?: string;
}

interface TopBarProps extends TopBarActions {
  state: StudioState;
  doc: TopBarDocument;
  theme: Theme;
  onToggleTheme: () => void;
  onOpenGallery: () => void;
}

const MAC = isMacPlatform(typeof navigator === "undefined" ? undefined : navigator);
const keys = (letter: string, shift = false) => shortcutLabel(letter, { mac: MAC, shift });

export function TopBar({
  state,
  doc,
  theme,
  onToggleTheme,
  onOpenGallery,
  onNew,
  onOpen,
  onSave,
  onSaveAs,
  onExport,
  exportDisabled,
  exportReason,
}: TopBarProps) {
  const tpl = state.sourceTemplateId ? templateById(state.sourceTemplateId) : undefined;
  const title = systemTitle(
    tpl,
    state.equations.map((l) => l.source),
    state.format,
  );
  const projector = theme === "light";
  // New has a shortcut in the desktop app only: the browser keeps Ctrl+N for a new window.
  const newTip = isTauri() ? `New system (${keys("n")})` : "New system";
  return (
    <header className="topbar">
      <div className="brand">
        <IconLogo />
        <span className="title brand-name">Separatrix</span>
      </div>
      <div className="doc-title" aria-live="polite">
        {(tpl || state.compiled || doc.hasFile) && (
          <span className="body-strong doc-name">{doc.name}</span>
        )}
        {doc.dirty && (
          <>
            <span className="caption doc-dirty" aria-hidden="true" data-tooltip="Unsaved changes">
              ●
            </span>
            <span className="ds-sr">Unsaved changes</span>
          </>
        )}
        {!doc.hasFile && title.edited && <span className="ds-template-card-tag doc-edited">Edited</span>}
        {tpl && templateRef(tpl) && <span className="caption doc-chapter">{templateRef(tpl)}</span>}
      </div>
      <div className="topbar-actions">
        <div className="topbar-file" role="group" aria-label="File">
          <button type="button" className="ds-icon-button" aria-label="New system" data-tip={newTip} onClick={onNew} {...tipDismiss}>
            <IconNewDoc />
          </button>
          <button
            type="button"
            className="ds-icon-button"
            aria-label="Open…"
            data-tip={`Open a project (${keys("o")})`}
            onClick={onOpen}
            {...tipDismiss}
          >
            <IconOpen />
          </button>
          <button
            type="button"
            className="ds-icon-button"
            aria-label="Save"
            data-tip={`Save (${keys("s")})`}
            onClick={onSave}
            {...tipDismiss}
          >
            <IconSave />
          </button>
          <button
            type="button"
            className="ds-icon-button"
            aria-label="Save as…"
            data-tip={`Save as… (${keys("s", true)})`}
            onClick={onSaveAs}
            {...tipDismiss}
          >
            <IconSaveAs />
          </button>
        </div>
        <button type="button" className="ds-button" onClick={onOpenGallery}>
          <IconGallery />
          <span>Templates…</span>
        </button>
        <button
          type="button"
          className="ds-button"
          onClick={onExport}
          disabled={exportDisabled}
          aria-keyshortcuts={MAC ? "Meta+E" : "Control+E"}
          data-tooltip={`Export (${keys("e")})`}
          data-tooltip-desc={exportReason ?? "The figure as LaTeX, SVG or PNG, the equations, or the data as CSV."}
        >
          <IconExport />
          <span>Export…</span>
        </button>
        <button
          type="button"
          className="ds-icon-button"
          aria-label="Projector mode"
          aria-pressed={projector}
          data-tip={
            projector
              ? "Projector mode (click for the dark chalkboard theme)"
              : "Projector mode (light theme, larger type, thicker lines)"
          }
          data-tip-align="end"
          onClick={onToggleTheme}
          {...tipDismiss}
        >
          {projector ? <IconMoon /> : <IconProjector />}
        </button>
      </div>
    </header>
  );
}

function timing(ms: number): { num: string; unit: string } {
  if (ms >= 1000) return { num: (ms / 1000).toFixed(1), unit: "s" };
  if (ms >= 10) return { num: String(Math.round(ms)), unit: "ms" };
  return { num: ms.toFixed(1), unit: "ms" };
}

function solverSummary(s: SolverSettings, kind: SystemKind | undefined) {
  if (kind === "map") {
    const n = Math.max(1, Math.round(s.tEnd));
    return { lead: "", num: String(n), tail: n === 1 ? " iterate" : " iterates", tip: `Iterating the map ${plural(n, "time")}` };
  }
  const m = METHODS[s.method];
  if (m.adaptive) {
    const v = minus(fmt(s.rtol, 2));
    return { lead: `${m.short} · rtol `, num: v, tail: "", tip: `${m.label}, relative tolerance ${v}` };
  }
  const v = minus(fmt(s.h, 3));
  return { lead: `${m.short} · h `, num: v, tail: "", tip: `${m.label}, step ${v}` };
}

function lyapunovSummary(l: LyapunovSettings, s: SolverSettings, kind: SystemKind | undefined) {
  const k = renormalizations(l, kind);
  const count = `${countText(k)} ${k === 1 ? "renormalization" : "renormalizations"}`;
  if (kind === "map") return { lead: "", num: count, tail: "", tip: `Lyapunov run of ${k.toLocaleString("en-US")} QR steps` };
  const m = solverSummary({ ...s, h: lyapunovSolver(s, l, kind).h }, kind);
  return { ...m, lead: `${count} · ${m.lead}`, tip: `Lyapunov run of ${k.toLocaleString("en-US")} QR steps, ${m.tip}` };
}

function orbitSummary(o: OrbitSettings, columns: number) {
  const n = (v: number) => v.toLocaleString("en-US");
  return {
    lead: "",
    num: n(columns),
    tail: ` columns · ${n(o.kept)} ${o.kept === 1 ? "iterate" : "iterates"} each`,
    tip: `Orbit diagram of ${n(columns)} parameter values, one per pixel: ${n(o.transient)} iterates dropped, then ${n(o.kept)} drawn in each`,
  };
}

function branchSummary(s: { points: number; special: number }, branches: number) {
  const n = (v: number) => v.toLocaleString("en-US");
  return {
    lead: "",
    num: n(s.points),
    tail: ` ${s.points === 1 ? "point" : "points"} · ${n(s.special)} special`,
    tip: `${n(branches)} ${branches === 1 ? "branch" : "branches"} of ${n(s.points)} points, with ${n(s.special)} special ${s.special === 1 ? "point" : "points"}`,
  };
}

function curvesStatus(s: { points: number; special: number; curves: number }) {
  const n = (v: number) => v.toLocaleString("en-US");
  return {
    lead: "",
    num: n(s.curves),
    tail: ` ${s.curves === 1 ? "curve" : "curves"} · ${n(s.points)} points · ${n(s.special)} special`,
    tip: `${n(s.curves)} ${s.curves === 1 ? "curve" : "curves"} of ${n(s.points)} points, with ${n(s.special)} special ${s.special === 1 ? "point" : "points"}`,
  };
}

function focusEquationLine(index: number) {
  const boxes = document.querySelectorAll<HTMLElement>(".panel-left .ds-equation-editor-field");
  const box = boxes[Math.max(0, index)] ?? boxes[0];
  const target = box?.matches("input, math-field") ? box : box?.querySelector<HTMLElement>("math-field");
  target?.scrollIntoView({ block: "nearest" });
  target?.focus();
}

function focusSolverSettings(id = "solver-settings") {
  const section = document.querySelector<HTMLElement>(`.panel-right #${id}`);
  if (!section) return;
  const toggle = section.querySelector<HTMLButtonElement>("button[aria-expanded]");
  if (toggle && toggle.getAttribute("aria-expanded") === "false") toggle.click();
  section.scrollIntoView({ block: "nearest" });
  const target =
    toggle ??
    section.querySelector<HTMLElement>("button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex='0']");
  target?.focus();
}

function axisName(axis: AxisRef, other: AxisRef, states: StateInfo[], kind: SystemKind | undefined): string {
  if (axis.kind === "state" && !states[axis.index]) return axis.index === 0 ? "x" : axis.index === 1 ? "y" : "?";
  return axisLabelLatex(axis, other, states, kind);
}

interface PlotPointer {
  x: number;
  y: number;
  digits: [number, number];
}

function usePlotPointer(state: StudioState): PlotPointer | null {
  const [pos, setPos] = useState<PlotPointer | null>(null);
  const viewRef = useRef(state.view);
  viewRef.current = state.view;
  const kindRef = useRef(state.compiled?.kind);
  kindRef.current = state.compiled?.kind;
  const last = useRef<{ el: HTMLElement; cx: number; cy: number } | null>(null);
  const schedule = useRef<() => void>(() => {});

  useEffect(() => {
    let frame = 0;
    const compute = () => {
      frame = 0;
      const l = last.current;
      if (!l || !l.el.isConnected) {
        setPos(null);
        return;
      }
      const v = viewRef.current;
      const r = l.el.getBoundingClientRect();
      const vp = plotViewport(l.el.clientWidth, l.el.clientHeight, v, kindRef.current);
      const px = l.cx - r.left;
      const py = l.cy - r.top;
      if (!insidePlot(vp, px, py)) {
        setPos(null);
        return;
      }
      const [x, y] = toPlot(vp, px, py);
      const [ux, uy] = unitsPerPx(vp);
      const dec = (u: number) => Math.min(8, Math.max(2, Math.ceil(-Math.log10(Math.abs(u) || 1)) - 1));
      setPos({ x, y, digits: [dec(ux), dec(uy)] });
    };
    schedule.current = () => {
      if (!frame) frame = requestAnimationFrame(compute);
    };
    const onMove = (e: PointerEvent) => {
      const el = e.target instanceof Element ? e.target.closest<HTMLElement>(".plot-canvas") : null;
      last.current = el ? { el, cx: e.clientX, cy: e.clientY } : null;
      schedule.current();
    };
    const onLeave = () => {
      last.current = null;
      schedule.current();
    };
    document.addEventListener("pointermove", onMove, { passive: true });
    document.documentElement.addEventListener("pointerleave", onLeave);
    return () => {
      document.removeEventListener("pointermove", onMove);
      document.documentElement.removeEventListener("pointerleave", onLeave);
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  useEffect(() => {
    if (last.current) schedule.current();
  }, [state.view]);

  return pos;
}

function useDelayed(on: boolean, delay: number): boolean {
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!on) {
      setShown(false);
      return;
    }
    const t = window.setTimeout(() => setShown(true), delay);
    return () => window.clearTimeout(t);
  }, [on, delay]);
  return on && shown;
}

interface StatusBarProps {
  state: StudioState;
  actions: StudioActions;
  /** Asks about unsaved changes before an update restarts the app. */
  beforeUpdate: () => Promise<boolean>;
}

export function StatusBar({ state, actions, beforeUpdate }: StatusBarProps) {
  const lyapunov = isLyapunov(state.view) ? state.view : null;
  const orbit = isOrbit(state.view) ? state.view : null;
  const branch = isContinuation(state.view) ? state.view : null;
  const plane = isCurves(state.view) ? state.view : null;
  const plot = isParameterView(state.view) ? null : state.view;
  const p = isAnalysis(state.view) ? null : state.portrait;
  const branches = branch ? shownBranches(state) : [];
  const branchRuns = branches.map((b) => b.run);
  const curveJobs = plane ? buildCurveJobs(state) : [];
  const curvesDone = curveJobs.filter((j) => j.meta.key in state.curveRuns).length;
  const sys = state.compiled;
  const kind = sys?.kind;
  const states = sys?.states ?? [];
  const current = lyapunov
    ? lyapunovCurrent(state)
    : orbit
      ? orbitCurrent(state)
      : branch
        ? branches.length > 0 && branchRuns.every((r) => r !== undefined)
        : plane
          ? curveJobs.length > 0 && curvesDone === curveJobs.length
          : !!p && !!sys && p.meta.systemId === sys.systemId;
  const run = lyapunov && current ? state.lyapunov : null;
  const diagram = orbit && current ? state.orbit : null;
  const followed = branch && current ? continuationSummary(branchRuns) : null;
  const traced = plane && current ? curvesSummary(state.planeRows, plane.curves.ranges) : null;
  const drawMs = useOrbitDrawMs();
  const nFp =
    state.layers.fixedPoints && !isAnalysis(state.view) ? (searchedFixedPoints(sys, p, state.shapeId)?.length ?? null) : null;
  const crossings =
    p && current && p.meta.view.kind === "section" && state.view.kind === "section"
      ? sectionCounts(p.res, state.solver.transient)
      : null;

  const solverGaveUpIcIds = p
    ? p.res.trajectories.flatMap((t, i) =>
        t.status !== "completed" && t.status !== "blewUp" && p.meta.trajectories[i] ? [p.meta.trajectories[i].icId] : [],
      )
    : [];
  const solverGaveUpIcs = [...new Set(solverGaveUpIcIds)].filter((id) => state.ics.some((ic) => ic.id === id));
  const stiff = !!p && current && looksStiff(p.res, state.solver.method);

  const errors = state.diagnostics.filter((d) => d.severity === "error");
  const lines = state.equations.map((l) => l.source);
  const firstBad = errors.length ? diagnosticLine(errors[0], lines) : -1;

  const waiting = branch
    ? !current && branches.length > 0
    : plane
      ? !current && curveJobs.length > 0
    : lyapunov || orbit
      ? !current && state.ics.some((ic) => ic.x0.length === states.length)
      : !current;
  const computing = useDelayed(!!sys?.ok && waiting && !state.computeError, 150);
  const pointer = usePlotPointer(state);

  const swept = orbit ?? branch;
  const cobwebNames =
    plot && (plot.kind === "cobweb" || isReturnMap(plot)) && states[0] ? viewAxisLabels(plot, states, kind) : null;
  const xName = lyapunov
    ? kind === "map"
      ? "n"
      : "t"
    : swept
      ? (sys?.params[swept.param]?.latex ?? "")
      : plane
        ? (sys?.params[plane.across]?.latex ?? "")
      : plot
        ? (cobwebNames?.x ?? axisName(plot.xAxis, plot.yAxis, states, kind))
        : "";
  const yName = lyapunov
    ? "\\lambda"
    : orbit
      ? (states[orbit.yAxis.index]?.latex ?? "")
      : branch
        ? branch.yAxis.kind === "state"
          ? (states[branch.yAxis.index]?.latex ?? "")
          : "\\lVert x \\rVert"
        : plane
          ? (sys?.params[plane.up]?.latex ?? "")
        : plot
          ? (cobwebNames?.y ?? axisName(plot.yAxis, plot.xAxis, states, kind))
          : "";
  const solver = lyapunov
    ? lyapunovSummary(lyapunov.lyapunov, state.solver, kind)
    : orbit
      ? orbitSummary(orbit.orbit, state.orbitColumns)
      : branch
        ? branchSummary(continuationSummary(branchRuns), branches.length)
        : plane
          ? curvesStatus(curvesSummary(state.planeRows, plane.curves.ranges))
          : solverSummary(state.solver, kind);
  const settingsId = orbit ? "orbit-settings" : branch ? "continuation-settings" : plane ? "curves-settings" : "solver-settings";
  const settingsTip = orbit
    ? "Open orbit diagram settings"
    : branch
      ? "Open continuation settings"
      : plane
        ? "Open the curves' settings"
        : "Open solver settings";
  const summed = followed ?? traced;
  const done = run ?? diagram ?? (summed ? { res: summed, roundTripMs: summed.roundTripMs } : null) ?? p;
  const t = done ? timing(done.res.elapsedMs) : null;
  const drawn = diagram && drawMs !== null ? ` Drawn in ${fmt(drawMs, 2)} ms.` : "";
  const timingDesc = done
    ? `${fmt(done.roundTripMs, 2)} ms including transfer.${drawn} ` +
      (isTauri() ? "Rust core." : "Browser preview: a small JavaScript stand-in computes the templates only.")
    : undefined;
  const timingLabel = done ? `Computed in ${fmt(done.res.elapsedMs, 2)} ms` : undefined;
  const lineTip = firstBad >= 0 ? `Go to line ${firstBad + 1}` : "Go to the equations";
  const num = (v: number, digits: number) => <span className="ds-statusbar-num">{minus(v.toFixed(digits))}</span>;

  return (
    <footer className="statusbar ds-statusbar" aria-label="Status">
      <div className="ds-statusbar-zone" role="status">
        {errors.length > 0 ? (
          <button
            type="button"
            className="ds-statusbar-item ds-statusbar-item--button ds-statusbar-item--danger"
            data-tooltip={lineTip}
            onClick={() => focusEquationLine(firstBad)}
          >
            <IconError />
            <span className="ds-statusbar-text">{plural(errors.length, "error")}</span>
            <span className="ds-sr">. {lineTip}</span>
          </button>
        ) : sys?.ok && state.compileOk ? (
          <>
            <span className="ds-statusbar-item ds-statusbar-item--ok">
              <IconCheck />
              <span className="ds-statusbar-text ds-statusbar-strong">Compiled</span>
            </span>
            <span className="ds-statusbar-item ds-statusbar-item--detail">
              <span className="ds-statusbar-text">
                · {plural(states.length, "state")}, {plural(sys.params.length, "parameter")}
              </span>
            </span>
          </>
        ) : (
          <span className="ds-statusbar-item">
            <span className="ds-statusbar-text">
              {state.equations.some((l) => l.source.trim() !== "") ? "Not compiled" : "No equations"}
            </span>
          </span>
        )}
      </div>

      {pointer ? (
        <span className="ds-statusbar-item ds-statusbar-coords">
          <span className="ds-sr">Cursor at </span>
          <Tex latex={xName} /> = {num(pointer.x, pointer.digits[0])}, <Tex latex={yName} /> ={" "}
          {num(pointer.y, pointer.digits[1])}
        </span>
      ) : (
        <span className="ds-statusbar-item ds-statusbar-coords is-empty">
          <span className="ds-sr">Cursor outside the plot</span>
          <span aria-hidden="true">
            <Tex latex={xName} /> = –, <Tex latex={yName} /> = –
          </span>
        </span>
      )}

      <div className="ds-statusbar-zone ds-statusbar-zone--end">
        {solverGaveUpIcs.length > 0 && (
          <button
            type="button"
            className="ds-statusbar-item ds-statusbar-item--button ds-statusbar-item--warn"
            data-tooltip="Select the trajectory"
            data-tooltip-desc="The inspector says why it stopped."
            onClick={() => actions.select({ kind: "ic", id: solverGaveUpIcs[0] })}
          >
            <IconWarning />
            <span className="ds-statusbar-text">{plural(solverGaveUpIcs.length, "run")} stopped</span>
            <span className="ds-sr">. Select the trajectory to see why it stopped</span>
          </button>
        )}
        {stiff && (
          <button
            type="button"
            className="ds-statusbar-item ds-statusbar-item--button ds-statusbar-item--warn"
            data-tooltip="Open solver settings"
            data-tooltip-desc={STIFF_HINT}
            data-stiff-status=""
            onClick={() => focusSolverSettings()}
          >
            <IconWarning />
            <span className="ds-statusbar-text">Steps look stiff</span>
            <span className="ds-sr">. {STIFF_HINT} Open solver settings</span>
          </button>
        )}
        {state.computeError && (
          <span
            className="ds-statusbar-item ds-statusbar-item--danger"
            data-tooltip="Computation failed"
            data-tooltip-desc={state.computeError}
          >
            <IconError />
            <span className="ds-statusbar-text">Computation failed</span>
            <span className="ds-sr">: {state.computeError}</span>
          </span>
        )}
        {computing ? (
          <span className="ds-statusbar-item">
            <svg
              className="ds-icon ds-statusbar-spinner"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              aria-hidden="true"
            >
              <circle cx="8" cy="8" r="5.75" opacity="0.3" />
              <path d="M8 2.25a5.75 5.75 0 0 1 5.75 5.75" />
            </svg>
            <span className="ds-statusbar-text">
              {plane ? (
                <>
                  Curves <span className="ds-statusbar-num">{curvesDone}</span> of{" "}
                  <span className="ds-statusbar-num">{curveJobs.length}</span> · searching
                </>
              ) : (
                "Computing…"
              )}
            </span>
          </span>
        ) : (
          t && (
            <span className="ds-statusbar-item" data-tooltip={timingLabel} data-tooltip-desc={timingDesc}>
              <span className="ds-statusbar-text">
                <span className="ds-statusbar-wide">
                  {run ? "Lyapunov " : diagram ? "Orbit " : followed ? "Continuation " : traced ? "Curves " : "Portrait "}
                </span>
                <span className="ds-statusbar-num">{t.num}</span> {t.unit}
              </span>
              <span className="ds-sr">
                . {timingLabel}, {timingDesc}
              </span>
            </span>
          )
        )}
        {crossings && (
          <span className="ds-statusbar-group">
            <span className="ds-statusbar-sep" aria-hidden="true" />
            <span
              className="ds-statusbar-item"
              data-tooltip="Section crossings drawn"
              data-tooltip-desc={
                crossings.skipPerOrbit
                  ? `The first ${crossings.skipPerOrbit.toLocaleString("en-US")} crossings of each orbit are skipped.`
                  : "No crossings are skipped."
              }
              data-section-count=""
            >
              <span className="ds-statusbar-text">
                <span className="ds-statusbar-num">{crossings.crossings.toLocaleString("en-US")}</span>{" "}
                {crossings.crossings === 1 ? "crossing" : "crossings"}
                {crossings.skipPerOrbit > 0 && (
                  <>
                    , first <span className="ds-statusbar-num">{crossings.skipPerOrbit.toLocaleString("en-US")}</span> per
                    orbit skipped
                  </>
                )}
              </span>
            </span>
          </span>
        )}
        {nFp !== null && (
          <span className="ds-statusbar-group">
            <span className="ds-statusbar-sep" aria-hidden="true" />
            <span
              className="ds-statusbar-item"
              data-tooltip="Fixed points in view"
              data-tooltip-desc="They are searched for in the visible region only."
            >
              <span className="ds-statusbar-text">
                <span className="ds-statusbar-num">{nFp}</span> {nFp === 1 ? "fixed point" : "fixed points"} in view
              </span>
            </span>
          </span>
        )}
        {sys?.ok && (
          <span className="ds-statusbar-group">
            <span className="ds-statusbar-sep" aria-hidden="true" />
            <button
              type="button"
              className="ds-statusbar-item ds-statusbar-item--button"
              data-tooltip={settingsTip}
              data-tooltip-desc={`${solver.tip}.`}
              onClick={() => focusSolverSettings(settingsId)}
            >
              <span className="ds-statusbar-text">
                {solver.lead}
                <span className="ds-statusbar-num">{solver.num}</span>
                {solver.tail}
              </span>
              <span className="ds-sr">. {solver.tip}. {settingsTip}</span>
            </button>
          </span>
        )}
        <UpdateButton beforeInstall={beforeUpdate} />
      </div>
    </footer>
  );
}
