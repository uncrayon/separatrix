import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  AxisRef,
  CompileResponse,
  ContinuationDirection,
  Diagnostic,
  InputFormat,
  SectionSpec,
  SpecialPointOut,
} from "../api/types";
import {
  compileSystem,
  computeContinuation,
  cancelJob,
  computeCurve,
  computeHomoclinic,
  computeLyapunov,
  computeOrbitDiagram,
  computePortrait,
  convertSource,
  LatestRequestScheduler,
} from "../api/client";
import { sameAxis } from "../math/format";
import { DEFAULT_METHOD, METHODS, methodFits } from "./methods";
import { BLANK_TEMPLATE, templateById, type Template } from "../templates";
import type { ProjectLoad } from "../project/serialize";
import {
  buildCurveJobs,
  curvesAvailability,
  drawnCurves,
  fitCurveRanges,
  followPlaneParams,
  openPlane,
  planeSeed,
  shownCurves,
  sliderRange,
  type CurveRow,
  type CurveRun,
  type CurvesSettings,
  type PlaneSeed,
  type CurvesView,
} from "./curves";
import {
  buildHomoclinicJobs,
  homoclinicRequestId,
  homoclinicRows,
  homoclinicSeed,
  settleHomoclinic,
  withHomoclinicSeed,
  type HomoclinicProgressState,
  type HomoclinicRow,
  type HomoclinicRun,
} from "./homoclinic";
import {
  DEFAULT_LAYERS,
  DEFAULT_SOLVER,
  adaptBaseState,
  applyProjectLoad,
  adaptInitialConditions,
  adaptView,
  branchSeeds,
  buildContinuationJobs,
  buildLyapunovJob,
  buildOrbitJob,
  buildPortraitJob,
  clampCamera,
  clampTransient,
  cobwebAvailability,
  continuationAvailability,
  continuationView,
  continuationXRange,
  continuationStep,
  defaultView,
  fitBranchRanges,
  followOrbitParam,
  isCobweb,
  isContinuation,
  isCurves,
  isLyapunov,
  isParameterView,
  isOrbit,
  isSection,
  isSpace,
  knownStateRange,
  viewChoice,
  viewRequestKey,
  withSpaceAxis,
  withSection,
  liftPlotPoint,
  switchView,
  newIcColor,
  pointSeed,
  withSeed,
  conversionOutcome,
  planModeSwitch,
  resolveParam,
  seriesRange,
  shiftTimeAxes,
  shownBranches,
  stateExtent,
  templateLines,
  templateView,
  trackSelection,
  type BranchRun,
  type Camera,
  type ContinuationSeed,
  type ContinuationSettings,
  type ContinuationView,
  type EquationLine,
  type InitialCondition,
  type Layers,
  type LyapunovMeta,
  type LyapunovSettings,
  type LyapunovState,
  type ModeSnapshot,
  type OrbitMeta,
  type OrbitSettings,
  type OrbitState,
  type ParamState,
  type PortraitMeta,
  type PortraitState,
  type Selection,
  type SolverSettings,
  type StudioSnapshot,
  type View,
  type ViewChoice,
  type ViewMemory,
} from "./model";

const COMPILE_DEBOUNCE_MS = 160;

/** The one input mode of the whole system, remembered across sessions. */
const FORMAT_KEY = "separatrix.inputMode";

function readFormat(): InputFormat {
  try {
    return localStorage.getItem(FORMAT_KEY) === "text" ? "text" : "latex";
  } catch {
    return "latex";
  }
}

function saveFormat(format: InputFormat) {
  try {
    localStorage.setItem(FORMAT_KEY, format);
  } catch {
    // Storage may be unavailable (private mode); the mode then lasts this session only.
  }
}

export interface StudioState extends StudioSnapshot {
  /** The input format of every line in `equations`. */
  format: InputFormat;
  /** The lines exactly as typed, in `format`. */
  equations: EquationLine[];
  /** Why a line was kept as typed by the last mode switch, by line id. */
  conversionNotes: Record<number, string>;
  /** A mode switch is waiting for the core. */
  converting: boolean;
  diagnostics: Diagnostic[];
  compileOk: boolean;
  homeView: View;
  selection: Selection;
  portrait: PortraitState | null;
  lyapunov: LyapunovState | null;
  orbit: OrbitState | null;
  orbitColumns: number;
  branches: Record<string, BranchRun>;
  /** The Parameter plane's curves, by request key (see `buildCurveJobs`). */
  curveRuns: Record<string, CurveRun>;
  /** The rows of the plane's Curves list; none in another view. */
  planeRows: CurveRow[];
  /** The Parameter plane's homoclinic curves, by request key (see `buildHomoclinicJobs`). */
  homoclinicRuns: Record<string, HomoclinicRun>;
  /** The homoclinic job in flight and the latest point it reported. */
  homoclinicProgress: HomoclinicProgressState | null;
  /** The homoclinic rows of the plane's Curves list; none in another view. */
  homoclinicRows: HomoclinicRow[];
  sourceTemplateId: string | null;
  computeError: string | null;
  /**
   * A project file was opened whose lines haven't compiled yet: its
   * parameters, start points, view and solver are applied by the first
   * compile that succeeds (after the user fixes a line), and until then a
   * save writes them exactly as read (project/serialize.ts).
   */
  pendingProject: ProjectLoad | null;
  /**
   * Counts the loads that have settled: a template, Blank or a project file
   * whose first compile came back (successful or not, so a load left
   * pending counts). Dirty tracking takes its baseline when it changes.
   */
  docEpoch: number;
  /**
   * Counts the view changes the plot made by itself ({@link
   * StudioActions.autoAdjustView}), which leave a clean document clean.
   */
  autoAdjustCount: number;
  /** Notices from the last project load, for the open banner. */
  loadWarnings: string[];
  /**
   * The last compile replaced a method the new system cannot use (a
   * symplectic method without a separable H(q, p)); says which, until the
   * method is chosen again.
   */
  methodNote: string | null;
}

export type Studio = ReturnType<typeof useStudio>;

/** A usable plot aspect rounded to 3 significant digits, or null. */
function roundAspect(aspect: number | null): number | null {
  return aspect && Number.isFinite(aspect) && aspect > 0 ? Number(aspect.toPrecision(3)) : null;
}

export function useStudio() {
  const [format, setFormatState] = useState<InputFormat>(readFormat);
  const [equations, setEquations] = useState<EquationLine[]>([]);
  const [conversionNotes, setConversionNotes] = useState<Record<number, string>>({});
  const [converting, setConverting] = useState(false);
  const [compiled, setCompiled] = useState<CompileResponse | null>(null);
  const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
  const [compileOk, setCompileOk] = useState(true);
  const [params, setParams] = useState<ParamState[]>([]);
  const [view, setViewState] = useState<View>({
    xAxis: { kind: "state", index: 0 },
    yAxis: { kind: "state", index: 1 },
    xRange: [-5, 5],
    yRange: [-5, 5],
  });
  const [homeView, setHomeView] = useState<View>(view);
  const [baseState, setBaseState] = useState<number[]>([]);
  const [fieldTime, setFieldTimeState] = useState(0);
  const [ics, setIcs] = useState<InitialCondition[]>([]);
  const [layers, setLayersState] = useState<Layers>(DEFAULT_LAYERS);
  const [solver, setSolverState] = useState<SolverSettings>(DEFAULT_SOLVER);
  const [selection, setSelection] = useState<Selection>(null);
  const [portrait, setPortrait] = useState<PortraitState | null>(null);
  const [lyapunov, setLyapunovState] = useState<LyapunovState | null>(null);
  const [orbit, setOrbitState] = useState<OrbitState | null>(null);
  const [orbitColumns, setOrbitColumnsState] = useState(0);
  const [branches, setBranches] = useState<Record<string, BranchRun>>({});
  const [curveRuns, setCurveRuns] = useState<Record<string, CurveRun>>({});
  const [homoclinicRuns, setHomoclinicRuns] = useState<Record<string, HomoclinicRun>>({});
  const [homoclinicProgress, setHomoclinicProgress] = useState<HomoclinicProgressState | null>(null);
  const [sourceTemplateId, setSourceTemplateId] = useState<string | null>(null);
  const [computeError, setComputeError] = useState<string | null>(null);
  const [plotAspect, setPlotAspectState] = useState<number | null>(null);
  const [shapeId, setShapeId] = useState(0);
  const [pendingProject, setPendingProject] = useState<ProjectLoad | null>(null);
  const [docEpoch, setDocEpoch] = useState(0);
  const [autoAdjustCount, setAutoAdjustCount] = useState(0);
  const [loadWarnings, setLoadWarnings] = useState<string[]>([]);
  const [methodNote, setMethodNote] = useState<string | null>(null);

  const planeRows = useMemo(
    () => (isCurves(view) ? shownCurves({ compiled, params, view, curveRuns }) : []),
    [compiled, view, params, curveRuns],
  );

  const homoclinicShown = useMemo(
    () => (isCurves(view) ? homoclinicRows(buildHomoclinicJobs({ compiled, view }), homoclinicRuns, homoclinicProgress) : []),
    [compiled, view, homoclinicRuns, homoclinicProgress],
  );

  const state: StudioState = {
    format,
    equations,
    conversionNotes,
    converting,
    compiled,
    diagnostics,
    compileOk,
    params,
    view,
    homeView,
    baseState,
    fieldTime,
    ics,
    layers,
    solver,
    selection,
    portrait,
    lyapunov,
    orbit,
    orbitColumns,
    branches,
    curveRuns,
    planeRows,
    homoclinicRuns,
    homoclinicProgress,
    homoclinicRows: homoclinicShown,
    sourceTemplateId,
    computeError,
    plotAspect,
    shapeId,
    pendingProject,
    docEpoch,
    autoAdjustCount,
    loadWarnings,
    methodNote,
  };
  const latest = useRef(state);
  latest.current = state;

  const nextLineId = useRef(1);
  const nextIcId = useRef(1);
  const compileSeq = useRef(0);
  const compileTimer = useRef<number | undefined>(undefined);
  const modeSnapshot = useRef<ModeSnapshot | null>(null);
  const switching = useRef(false);
  /** The project load waiting for its lines to compile (mirrors `pendingProject`). */
  const pendingLoad = useRef<ProjectLoad | null>(null);
  /** A load was started and no compile has come back since. */
  const loadSettling = useRef(false);
  /**
   * The last view of each kind this session (phase plane, time series,
   * cobweb), so switching back restores it; forgotten when the system's
   * shape changes. Not saved.
   */
  const viewMemory = useRef<{ shapeId: number; views: ViewMemory }>({ shapeId: -1, views: {} });

  const makeIc = useCallback((x0: number[], t0: number): Omit<InitialCondition, "color"> => {
    const id = nextIcId.current++;
    return { id, uid: `ic-${id}`, x0, t0 };
  }, []);

  const scheduler = useMemo(
    () =>
      new LatestRequestScheduler(
        computePortrait,
        () => buildPortraitJob(latest.current),
        (res, meta: PortraitMeta, roundTripMs) => {
          const last = latest.current.portrait;
          const prev = last?.meta.fixedPoints ? last.res.fixedPoints : [];
          setPortrait({ res, meta, roundTripMs });
          // With the layer hidden nothing was searched, so the selection is kept for when it is shown again.
          if (meta.fixedPoints) setSelection((sel) => trackSelection(sel, res.fixedPoints, meta.view, prev));
          setComputeError(null);
        },
        (err) => setComputeError(err instanceof Error ? err.message : String(err)),
      ),
    [],
  );
  useEffect(() => () => scheduler.dispose(), [scheduler]);

  /** A project file just opened in the Lyapunov view; its next run keeps the saved framing. */
  const framedByFile = useRef(false);
  const lyapunovScheduler = useMemo(
    () =>
      new LatestRequestScheduler(
        computeLyapunov,
        () => buildLyapunovJob(latest.current),
        (res, meta: LyapunovMeta, roundTripMs) => {
          setLyapunovState({ res, meta, roundTripMs, framedByFile: framedByFile.current });
          framedByFile.current = false;
          setComputeError(null);
        },
        (err) => {
          framedByFile.current = false;
          setComputeError(err instanceof Error ? err.message : String(err));
        },
      ),
    [],
  );
  useEffect(() => () => lyapunovScheduler.dispose(), [lyapunovScheduler]);

  const orbitScheduler = useMemo(
    () =>
      new LatestRequestScheduler(
        computeOrbitDiagram,
        () => buildOrbitJob(latest.current),
        (res, meta: OrbitMeta, roundTripMs) => {
          setOrbitState({ res, meta, roundTripMs });
          setComputeError(null);
        },
        (err) => setComputeError(err instanceof Error ? err.message : String(err)),
      ),
    [],
  );
  useEffect(() => () => orbitScheduler.dispose(), [orbitScheduler]);

  const updateContinuation = useCallback((f: (v: ContinuationView) => ContinuationView) => {
    setViewState((v) => (isContinuation(v) ? f(v) : v));
    setHomeView((v) => (isContinuation(v) ? f(v) : v));
  }, []);
  const continuing = useRef(false);
  const refitBranches = useRef(false);
  const nextContinuationId = useRef(1);
  const pumpContinuation = useCallback(async () => {
    if (continuing.current) return;
    continuing.current = true;
    try {
      for (;;) {
        const cache = latest.current.branches;
        const job = buildContinuationJobs(latest.current).find((j) => !(j.meta.key in cache));
        if (!job) {
          const v = latest.current.view;
          const fit =
            refitBranches.current && isContinuation(v) && latest.current.compiled?.ok
              ? fitBranchRanges(
                  shownBranches(latest.current).map((b) => b.run),
                  latest.current.compiled.states.length,
                  v.yAxis,
                )
              : null;
          refitBranches.current = false;
          if (fit) updateContinuation((w) => ({ ...w, yRange: fit.yRange }));
          break;
        }
        const started = performance.now();
        let run: BranchRun;
        try {
          const res = await computeContinuation({ ...job.body, requestId: nextContinuationId.current++ });
          run = { ok: true, res, roundTripMs: performance.now() - started };
        } catch (err) {
          run = { ok: false, error: err instanceof Error ? err.message : String(err) };
        }
        const wanted = new Set(buildContinuationJobs(latest.current).map((j) => j.meta.key));
        const next: Record<string, BranchRun> = wanted.has(job.meta.key) ? { [job.meta.key]: run } : {};
        for (const [k, v] of Object.entries(latest.current.branches)) if (wanted.has(k)) next[k] = v;
        latest.current = { ...latest.current, branches: next };
        setBranches(next);
      }
    } finally {
      continuing.current = false;
    }
  }, [updateContinuation]);
  const updateCurves = useCallback((f: (v: CurvesView) => CurvesView) => {
    setViewState((v) => (isCurves(v) ? f(v) : v));
    setHomeView((v) => (isCurves(v) ? f(v) : v));
  }, []);
  const following = useRef(false);
  /** The next curves to arrive frame the plot, as a plane just opened or followed from the Branch view. */
  const refitCurves = useRef(false);
  const nextCurveId = useRef(1);
  const pumpCurves = useCallback(async () => {
    if (following.current) return;
    following.current = true;
    try {
      for (;;) {
        const cache = latest.current.curveRuns;
        const job = buildCurveJobs(latest.current).find((j) => !(j.meta.key in cache));
        if (!job) {
          if (refitCurves.current && isCurves(latest.current.view)) {
            const fit = fitCurveRanges(drawnCurves(shownCurves(latest.current)));
            if (fit) updateCurves((w) => ({ ...w, ...fit }));
          }
          refitCurves.current = false;
          break;
        }
        const started = performance.now();
        let run: CurveRun;
        try {
          const res = await computeCurve({ ...job.body, requestId: nextCurveId.current++ });
          run = { ok: true, res, roundTripMs: performance.now() - started };
        } catch (err) {
          run = { ok: false, error: err instanceof Error ? err.message : String(err) };
        }
        const wanted = new Set(buildCurveJobs(latest.current).map((j) => j.meta.key));
        const next: Record<string, CurveRun> = wanted.has(job.meta.key) ? { [job.meta.key]: run } : {};
        for (const [k, v] of Object.entries(latest.current.curveRuns)) if (wanted.has(k)) next[k] = v;
        latest.current = { ...latest.current, curveRuns: next };
        setCurveRuns(next);
      }
    } finally {
      following.current = false;
    }
  }, [updateCurves]);
  const curvesKey = buildCurveJobs(state)
    .map((j) => j.meta.key)
    .join("\n");
  useEffect(() => {
    if (curvesKey) void pumpCurves();
  }, [pumpCurves, curvesKey]);

  /** The homoclinic job in flight: its key and the id the shell cancels it by. */
  const homoclinicFlight = useRef<{ key: string; requestId: number } | null>(null);
  const runningHomoclinic = useRef(false);
  const pumpHomoclinic = useCallback(async () => {
    if (runningHomoclinic.current) return;
    runningHomoclinic.current = true;
    try {
      for (;;) {
        const cache = latest.current.homoclinicRuns;
        const job = buildHomoclinicJobs(latest.current).find((j) => !(j.meta.key in cache));
        if (!job) break;
        const flight = { key: job.meta.key, requestId: homoclinicRequestId() };
        homoclinicFlight.current = flight;
        setHomoclinicProgress({ key: flight.key, points: 0, p: null });
        const started = performance.now();
        let run: HomoclinicRun | null;
        try {
          const res = await computeHomoclinic({ ...job.body, requestId: flight.requestId }, (m) => {
            if (m.requestId === flight.requestId && homoclinicFlight.current === flight)
              setHomoclinicProgress({ key: flight.key, points: m.points, p: [m.p[0], m.p[1]] });
          });
          run = res && { ok: true, res, roundTripMs: performance.now() - started };
        } catch (err) {
          run = { ok: false, error: err instanceof Error ? err.message : String(err) };
        }
        homoclinicFlight.current = null;
        setHomoclinicProgress(null);
        const wanted = new Set(buildHomoclinicJobs(latest.current).map((j) => j.meta.key));
        const next = settleHomoclinic(latest.current.homoclinicRuns, wanted, job.meta.key, run, isCurves(latest.current.view));
        latest.current = { ...latest.current, homoclinicRuns: next };
        setHomoclinicRuns(next);
      }
    } finally {
      runningHomoclinic.current = false;
    }
  }, []);
  const homoclinicKey = buildHomoclinicJobs(state)
    .map((j) => j.meta.key)
    .join("\n");
  useEffect(() => {
    const flight = homoclinicFlight.current;
    if (flight && !homoclinicKey.split("\n").includes(flight.key)) void cancelJob(flight.requestId).catch(() => {});
    if (homoclinicKey) void pumpHomoclinic();
  }, [pumpHomoclinic, homoclinicKey]);

  const continuationKey = buildContinuationJobs(state)
    .map((j) => j.meta.key)
    .join("\n");
  useEffect(() => {
    if (continuationKey) void pumpContinuation();
  }, [pumpContinuation, continuationKey]);

  const viewKey = viewRequestKey(view);
  useEffect(() => {
    scheduler.request();
  }, [scheduler, compiled, params, viewKey, baseState, fieldTime, ics, layers, solver, plotAspect]);

  const lyapunovKey = buildLyapunovJob(state)?.meta.key ?? null;
  useEffect(() => {
    if (lyapunovKey !== null) lyapunovScheduler.request();
  }, [lyapunovScheduler, lyapunovKey]);

  const orbitKey = buildOrbitJob(state)?.meta.key ?? null;
  useEffect(() => {
    if (orbitKey !== null) orbitScheduler.request();
  }, [orbitScheduler, orbitKey]);

  const runCompile = useCallback(
    async (lines: string[], format: InputFormat, template?: Template) => {
      const seq = ++compileSeq.current;
      const source = lines.filter((l) => l.trim() !== "").join("\n");
      // A load (template, Blank, project file) has settled once a compile
      // after it is back, whatever it says (a file that doesn't compile
      // stays pending); a compile that supersedes the load's own settles it.
      const settle = () => {
        if (!loadSettling.current) return;
        loadSettling.current = false;
        setDocEpoch((n) => n + 1);
      };
      let res: CompileResponse;
      try {
        res = await compileSystem({ source, format });
      } catch (err) {
        if (seq !== compileSeq.current) return;
        setDiagnostics([{ severity: "error", message: `The maths core did not answer: ${String(err)}` }]);
        setCompileOk(false);
        settle();
        return;
      }
      if (seq !== compileSeq.current) return;
      setDiagnostics(res.diagnostics);
      setCompileOk(res.ok);
      if (!res.ok || res.systemId === undefined) {
        settle();
        return;
      }

      const cur = latest.current;
      const old = cur.compiled;
      const dim = res.states.length;
      const sourceTemplate = cur.sourceTemplateId ? templateById(cur.sourceTemplateId) : undefined;

      const load = template ? null : pendingLoad.current;
      let method = cur.solver.method;
      if (!load) {
        setParams(
          res.params.map((p) =>
            resolveParam(
              p,
              cur.params.find((q) => q.name === p.name),
              old?.params.find((q) => q.name === p.name)?.default,
              template,
              sourceTemplate,
            ),
          ),
        );
      }

      if (load) {
        // The project file's state, now that its lines compiled.
        pendingLoad.current = null;
        setPendingProject(null);
        const applied = applyProjectLoad(load, res, () => nextIcId.current++);
        setShapeId((n) => n + 1);
        setParams(applied.params);
        setViewState(applied.view);
        setHomeView(applied.view);
        setBaseState(applied.baseState);
        setFieldTimeState(applied.fieldTime);
        setIcs(applied.ics);
        setLayersState(applied.layers);
        setSolverState(applied.solver);
        method = applied.solver.method;
        framedByFile.current = isLyapunov(applied.view);
        setSelection(null);
        if (applied.warnings.length) setLoadWarnings((w) => [...w, ...applied.warnings]);
      } else if (template) {
        framedByFile.current = false;
        setShapeId((n) => n + 1);
        const v = templateView(template, res);
        setViewState(v);
        setHomeView(v);
        setBaseState(new Array(dim).fill(0));
        setFieldTimeState(0);
        const fresh: InitialCondition[] = [];
        for (const x0 of template.initialConditions) {
          const ic = makeIc(
            Array.from({ length: dim }, (_, i) => x0[i] ?? 0),
            0,
          );
          fresh.push({ ...ic, color: newIcColor(fresh, v) });
        }
        setIcs(fresh);
        setSolverState((s) => ({ ...s, tEnd: template.tEnd, transient: template.transient ?? 0 }));
        setLayersState((l) => ({
          ...l,
          backward: template.backward ?? true,
          nullclines: dim <= 2,
        }));
        setSelection(null);
      } else {
        const sameShape =
          old &&
          old.kind === res.kind &&
          old.states.map((s) => s.name).join() === res.states.map((s) => s.name).join();
        if (!sameShape) {
          setShapeId((n) => n + 1);
          const oldStates = old?.states ?? [];
          const adapted = old?.kind === res.kind ? adaptView(cur.view, oldStates, res.states, res) : null;
          const swept = adapted && (isOrbit(adapted) || isContinuation(adapted));
          const followed = swept
            ? followOrbitParam(adapted, old?.params ?? [], res.params)
            : adapted && isCurves(adapted)
              ? followPlaneParams(adapted, old?.params ?? [], res.params)
              : adapted;
          const unavailable =
            !!followed &&
            ((isContinuation(followed) && !continuationAvailability(res).ok) ||
              (isCurves(followed) && !curvesAvailability(res).ok));
          const kept = unavailable ? null : followed;
          const v = kept ?? defaultView(res, cur.solver);
          setViewState(v);
          setHomeView(v);
          setBaseState((b) => adaptBaseState(b, oldStates, res.states));
          setIcs((list) => adaptInitialConditions(list, oldStates, res.states));
          if (old?.kind !== res.kind) setSolverState((s) => ({ ...s, tEnd: res.kind === "map" ? 50 : 20 }));
        } else if (isOrbit(cur.view) && followOrbitParam(cur.view, old.params, res.params) !== cur.view) {
          const v = followOrbitParam(cur.view, old.params, res.params) ?? defaultView(res, cur.solver);
          setViewState(v);
          setHomeView(v);
        } else if (isContinuation(cur.view)) {
          const followed = continuationAvailability(res).ok ? followOrbitParam(cur.view, old.params, res.params) : null;
          if (followed !== cur.view) {
            const v = followed ?? defaultView(res, cur.solver);
            setViewState(v);
            setHomeView(v);
          }
        } else if (isCurves(cur.view)) {
          const followed = curvesAvailability(res).ok ? followPlaneParams(cur.view, old.params, res.params) : null;
          if (followed !== cur.view) {
            const v = followed ?? defaultView(res, cur.solver);
            setViewState(v);
            setHomeView(v);
          }
        } else if (isCobweb(cur.view) && !cobwebAvailability(res).ok) {
          // The same map, now changing with n: f has no single graph, so the default view (a time series).
          const v = defaultView(res, cur.solver);
          setViewState(v);
          setHomeView(v);
        }
      }
      if (methodFits(method, res)) {
        setMethodNote(null);
      } else {
        setSolverState((s) => ({ ...s, method: DEFAULT_METHOD, h: 0 }));
        setMethodNote(`Switched from ${METHODS[method].label} to ${METHODS[DEFAULT_METHOD].label}.`);
      }
      setCompiled(res);
      settle();
    },
    [makeIc],
  );

  const scheduleCompile = useCallback(
    (lines: EquationLine[]) => {
      const format = latest.current.format;
      window.clearTimeout(compileTimer.current);
      compileTimer.current = window.setTimeout(
        () =>
          void runCompile(
            lines.map((l) => l.source),
            format,
          ),
        COMPILE_DEBOUNCE_MS,
      );
    },
    [runCompile],
  );

  const toLines = (sources: string[]) => sources.map((source) => ({ id: nextLineId.current++, source }));

  const loadTemplate = useCallback(
    (t: Template) => {
      window.clearTimeout(compileTimer.current);
      pendingLoad.current = null;
      setPendingProject(null);
      setLoadWarnings([]);
      loadSettling.current = true;
      const format = latest.current.format;
      const lines = templateLines(t, format);
      const next = toLines(lines);
      setEquations(next);
      latest.current = { ...latest.current, equations: next };
      setConversionNotes({});
      modeSnapshot.current = null;
      setSourceTemplateId(t.id);
      void runCompile(lines, format, t);
    },
    [runCompile],
  );

  const newBlank = useCallback(() => {
    loadTemplate(BLANK_TEMPLATE);
    setSourceTemplateId(null);
  }, [loadTemplate]);

  /**
   * Opens a project file (from {@link stateFromProject}): its lines, input
   * format and template id now; everything else once the lines compile. If
   * they don't, the load waits in `pendingProject` for the first compile
   * that succeeds, so fixing the line brings the file's start points back.
   */
  const loadProject = useCallback(
    (load: ProjectLoad) => {
      window.clearTimeout(compileTimer.current);
      const next = toLines(load.lines.length ? load.lines : [""]);
      setEquations(next);
      setFormatState(load.format);
      saveFormat(load.format);
      latest.current = { ...latest.current, equations: next, format: load.format };
      setConversionNotes({});
      modeSnapshot.current = null;
      setSourceTemplateId(load.templateId);
      setSelection(null);
      setLoadWarnings(load.warnings);
      // New start points are numbered past the file's own `ic-<n>` ids.
      for (const ic of load.ics) {
        const m = /^ic-(\d+)$/.exec(ic.uid);
        if (m) nextIcId.current = Math.max(nextIcId.current, Number(m[1]) + 1);
      }
      pendingLoad.current = load;
      setPendingProject(load);
      loadSettling.current = true;
      void runCompile(load.lines, load.format);
    },
    [runCompile],
  );

  const setEquation = useCallback(
    (id: number, source: string) => {
      const lines = latest.current.equations.map((l) => (l.id === id ? { ...l, source } : l));
      setEquations(lines);
      latest.current = { ...latest.current, equations: lines };
      setConversionNotes((notes) => {
        if (!(id in notes)) return notes;
        const rest = { ...notes };
        delete rest[id];
        return rest;
      });
      scheduleCompile(lines);
    },
    [scheduleCompile],
  );

  /**
   * Switch every line to the input format `to`. Straight back after a switch
   * with nothing edited, the lines from before it are restored exactly;
   * otherwise the core converts them. A line with an error is kept as typed,
   * with a note; a valid line the core can't convert exactly calls the switch
   * off (kept as typed, it would mean something else in `to`), with a note on
   * it. A conversion that comes back after the lines have changed is dropped. Names are kept, so the recompile finds the same
   * system and keeps the sliders, view and initial conditions.
   */
  const setFormat = useCallback(
    async (to: InputFormat) => {
      const cur = latest.current;
      if (to === cur.format || switching.current) return;
      window.clearTimeout(compileTimer.current);
      const sent = cur.equations;
      const plan = planModeSwitch(
        sent.map((l) => l.source),
        modeSnapshot.current,
        to,
      );
      let next: string[];
      const notes: Record<number, string> = {};
      const noteLines = (list: (string | undefined)[]) =>
        list.forEach((note, i) => {
          if (note && sent[i]) notes[sent[i].id] = note;
        });
      if ("restore" in plan) {
        next = plan.restore;
      } else {
        switching.current = true;
        setConverting(true);
        let outcome: ReturnType<typeof conversionOutcome>;
        try {
          const res = await convertSource({ lines: plan.convert, from: cur.format, to });
          outcome = conversionOutcome(res);
        } catch (err) {
          setDiagnostics([{ severity: "error", message: `The maths core did not answer: ${String(err)}` }]);
          setCompileOk(false);
          return;
        } finally {
          switching.current = false;
          setConverting(false);
        }
        if (latest.current.equations !== sent || outcome.notes.length !== sent.length) return;
        noteLines(outcome.notes);
        if (!outcome.switch) {
          // A valid line would change meaning in `to`: stay, say which, and
          // compile the lines as they are (the switch cancelled a pending compile).
          setConversionNotes(notes);
          void runCompile(sent.map((l) => l.source), cur.format);
          return;
        }
        next = outcome.lines;
      }
      const lines = sent.map((l, i) => ({ id: l.id, source: next[i] }));
      modeSnapshot.current = { from: cur.format, before: sent.map((l) => l.source), after: next };
      setEquations(lines);
      setFormatState(to);
      latest.current = { ...latest.current, equations: lines, format: to };
      setConversionNotes(notes);
      saveFormat(to);
      void runCompile(next, to);
    },
    [runCompile],
  );

  const addEquation = useCallback((afterId?: number): number => {
    const line = { id: nextLineId.current++, source: "" };
    setEquations((list) => {
      const i = afterId === undefined ? list.length - 1 : list.findIndex((l) => l.id === afterId);
      const copy = list.slice();
      copy.splice(i + 1, 0, line);
      return copy;
    });
    return line.id;
  }, []);

  const removeEquation = useCallback(
    (id: number) => {
      const lines = latest.current.equations.filter((l) => l.id !== id);
      setEquations(lines);
      latest.current = { ...latest.current, equations: lines };
      scheduleCompile(lines);
    },
    [scheduleCompile],
  );

  const setParam = useCallback((name: string, value: number) => {
    if (!Number.isFinite(value)) return;
    setParams((ps) =>
      ps.map((p) =>
        p.name === name ? { ...p, value, min: Math.min(p.min, value), max: Math.max(p.max, value) } : p,
      ),
    );
  }, []);

  const setParamRange = useCallback((name: string, min: number, max: number) => {
    if (!Number.isFinite(min) || !Number.isFinite(max) || min >= max) return;
    setParams((ps) =>
      ps.map((p) => (p.name === name ? { ...p, min, max, value: Math.min(max, Math.max(min, p.value)) } : p)),
    );
  }, []);

  const setView = useCallback((v: View) => setViewState(v), []);
  /**
   * A view change the plot makes by itself (making room under the toolbar),
   * not the user: it leaves a clean document clean.
   */
  const autoAdjustView = useCallback((v: View) => {
    setViewState(v);
    setAutoAdjustCount((n) => n + 1);
  }, []);
  const setPlotAspect = useCallback((aspect: number | null) => {
    const a = roundAspect(aspect);
    setPlotAspectState((old) => (old === a ? old : a));
  }, []);
  const setIcHidden = useCallback((id: number, hidden: boolean) => {
    setIcs((list) => list.map((ic) => (ic.id === id ? { ...ic, hidden } : ic)));
  }, []);
  const resetView = useCallback(() => setViewState(latest.current.homeView), []);

  /**
   * Shows the phase plane, the time series or the cobweb. The last view of
   * that kind this session comes back as it was; otherwise one is made from
   * what is shown now (the same state range, as {@link setAxis} picks ranges).
   * A kind the system can't show is ignored.
   */
  const setViewKind = useCallback((choice: ViewChoice) => {
    const cur = latest.current;
    const mem = viewMemory.current;
    if (mem.shapeId !== cur.shapeId) viewMemory.current = { shapeId: cur.shapeId, views: {} };
    const next = switchView(choice, cur, viewMemory.current.views);
    if (!next) return;
    if (isCurves(next)) refitCurves.current = true;
    setViewState(next);
    setHomeView(next);
  }, []);

  const setAxis = useCallback((which: "x" | "y", axis: AxisRef) => {
    const cur = latest.current;
    if (isParameterView(cur.view)) return;
    const section = isSection(cur.view) ? cur.view : null;
    if (section && axis.kind === "time") return;
    const { xAxis, yAxis, xRange, yRange } = cur.view;
    const v: View = section ?? { xAxis, yAxis, xRange, yRange };
    const other = which === "x" ? v.yAxis : v.xAxis;
    const rangeFor = (a: AxisRef): [number, number] => {
      if (sameAxis(a, v.xAxis)) return v.xRange;
      if (sameAxis(a, v.yAxis)) return v.yRange;
      if (a.kind === "time") return seriesRange(cur.solver, cur.compiled?.kind);
      const ext = stateExtent(cur.portrait?.res, cur.compiled?.states.length ?? 0, a.index);
      if (ext) {
        const pad = Math.max(0.1 * (ext[1] - ext[0]), 0.5);
        return [ext[0] - pad, ext[1] + pad];
      }
      return [-5, 5];
    };
    let next: View;
    if (sameAxis(axis, other) && !section) {
      next = { xAxis: v.yAxis, yAxis: v.xAxis, xRange: v.yRange, yRange: v.xRange };
    } else if (which === "x") {
      next = { ...v, xAxis: axis, xRange: rangeFor(axis) };
    } else {
      next = { ...v, yAxis: axis, yRange: rangeFor(axis) };
    }
    setViewState(next);
    setHomeView(next);
  }, []);

  const setCamera = useCallback((camera: Camera) => {
    setViewState((v) => (isSpace(v) ? { ...v, camera: clampCamera(camera) } : v));
  }, []);

  const setSpaceAxis = useCallback((slot: 0 | 1 | 2, index: number) => {
    const cur = latest.current;
    const sys = cur.compiled;
    if (!sys || !isSpace(cur.view)) return;
    const v = cur.view;
    const mem = viewMemory.current.shapeId === cur.shapeId ? Object.values(viewMemory.current.views) : [];
    const rangeOf = (i: number) => knownStateRange(i, [v, ...mem], cur.portrait?.res, sys.states.length);
    const next = withSpaceAxis(v, slot, index, rangeOf);
    setViewState(next);
    setHomeView(next);
  }, []);

  const setSection = useCallback((spec: SectionSpec) => {
    const cur = latest.current;
    const sys = cur.compiled;
    if (!sys || !isSection(cur.view)) return;
    const v = cur.view;
    const mem = viewMemory.current.shapeId === cur.shapeId ? Object.values(viewMemory.current.views) : [];
    const rangeOf = (i: number) => knownStateRange(i, [v, ...mem], cur.portrait?.res, sys.states.length);
    const next = withSection(v, sys, spec, rangeOf);
    setViewState(next);
    setHomeView(next);
  }, []);

  const setLyapunov = useCallback((patch: Partial<LyapunovSettings>) => {
    setViewState((v) => (isLyapunov(v) ? { ...v, lyapunov: { ...v.lyapunov, ...patch } } : v));
    setHomeView((v) => (isLyapunov(v) ? { ...v, lyapunov: { ...v.lyapunov, ...patch } } : v));
  }, []);

  const setOrbit = useCallback((patch: Partial<OrbitSettings>) => {
    setViewState((v) => (isOrbit(v) ? { ...v, orbit: { ...v.orbit, ...patch } } : v));
    setHomeView((v) => (isOrbit(v) ? { ...v, orbit: { ...v.orbit, ...patch } } : v));
  }, []);

  const setOrbitRange = useCallback((lo: number, hi: number) => {
    if (!(Number.isFinite(lo) && Number.isFinite(hi) && lo < hi)) return;
    setViewState((v) => (isOrbit(v) ? { ...v, xRange: [lo, hi] } : v));
    setHomeView((v) => (isOrbit(v) ? { ...v, xRange: [lo, hi] } : v));
  }, []);

  const setOrbitParam = useCallback((index: number) => {
    const cur = latest.current;
    const p = cur.params[index];
    if (!isOrbit(cur.view) || !p || index === cur.view.param) return;
    const next = { ...cur.view, param: index, xRange: [p.min, p.max] as [number, number] };
    setViewState(next);
    setHomeView(next);
  }, []);


  const setContinuation = useCallback(
    (patch: Partial<Omit<ContinuationSettings, "seeds">>) => {
      if (patch.range) refitBranches.current = true;
      updateContinuation((v) => ({
        ...v,
        ...(patch.range ? { xRange: continuationXRange(patch.range) } : {}),
        continuation: { ...v.continuation, ...patch },
      }));
    },
    [updateContinuation],
  );

  const setContinuationParam = useCallback(
    (index: number) => {
      const p = latest.current.params[index];
      if (!p || !(p.min < p.max)) return;
      const range: [number, number] = [p.min, p.max];
      const pointSeedsAtValueNow = (seeds: ContinuationSeed[]) =>
        seeds
          .filter((seed) => seed.from === "point")
          .reduce<ContinuationSeed[]>((out, seed) => withSeed(out, { ...seed, param: p.value }), []);
      updateContinuation((v) =>
        v.param === index
          ? v
          : {
              ...v,
              param: index,
              xRange: continuationXRange(range),
              continuation: { ...v.continuation, range, ds: continuationStep(range), seeds: pointSeedsAtValueNow(v.continuation.seeds) },
            },
      );
    },
    [updateContinuation],
  );

  const setContinuationAxis = useCallback(
    (yAxis: ContinuationView["yAxis"], yRange: [number, number]) => updateContinuation((v) => ({ ...v, yAxis, yRange })),
    [updateContinuation],
  );

  const addSeed = useCallback(
    (seed: ContinuationSeed) =>
      updateContinuation((v) => ({ ...v, continuation: { ...v.continuation, seeds: withSeed(v.continuation.seeds, seed) } })),
    [updateContinuation],
  );

  const removeSeed = useCallback(
    (index: number) =>
      updateContinuation((v) => ({
        ...v,
        continuation: { ...v.continuation, seeds: v.continuation.seeds.filter((_, i) => i !== index) },
      })),
    [updateContinuation],
  );

  const continueFrom = useCallback((x: number[], direction: ContinuationDirection) => {
    const cur = latest.current;
    const sys = cur.compiled;
    if (!sys || !continuationAvailability(sys).ok) return;
    let v: ContinuationView;
    if (isContinuation(cur.view)) v = cur.view;
    else {
      const mem = viewMemory.current;
      if (mem.shapeId !== cur.shapeId) viewMemory.current = { shapeId: cur.shapeId, views: {} };
      const kept = viewMemory.current.views.continuation;
      viewMemory.current.views[viewChoice(cur.view)] = cur.view;
      const range = knownStateRange(0, [cur.view], cur.portrait?.res, sys.states.length);
      v = kept && isContinuation(kept) ? kept : continuationView(cur.params, range);
    }
    const value = cur.params[v.param]?.value;
    if (value === undefined) return;
    const next = { ...v, continuation: { ...v.continuation, seeds: withSeed(v.continuation.seeds, pointSeed(x, value, direction)) } };
    setViewState(next);
    setHomeView(next);
  }, []);

  const setCurves = useCallback(
    (patch: Partial<Omit<CurvesSettings, "seeds">>) => {
      if (patch.ranges) refitCurves.current = true;
      updateCurves((v) => ({ ...v, curves: { ...v.curves, ...patch } }));
    },
    [updateCurves],
  );

  /** Frees parameter `index` across or up; picking the other one's parameter swaps them. */
  const setCurvesParam = useCallback(
    (which: "across" | "up", index: number) => {
      const params = latest.current.params;
      refitCurves.current = true;
      updateCurves((v) => {
        if (v[which] === index || !params[index]) return v;
        const swap = v[which === "across" ? "up" : "across"] === index;
        const across = swap ? v.up : which === "across" ? index : v.across;
        const up = swap ? v.across : which === "up" ? index : v.up;
        const [r0, r1] = v.curves.ranges;
        const ranges: CurvesSettings["ranges"] = swap
          ? [r1, r0]
          : which === "across"
            ? [sliderRange(params[index]), r1]
            : [r0, sliderRange(params[index])];
        return { ...v, across, up, curves: { ...v.curves, ranges } };
      });
    },
    [updateCurves],
  );

  const removeCurveSeed = useCallback(
    (index: number) =>
      updateCurves((v) => ({ ...v, curves: { ...v.curves, seeds: v.curves.seeds.filter((_, i) => i !== index) } })),
    [updateCurves],
  );

  /**
   * Follows a homoclinic curve from the Bogdanov–Takens point at `at` with
   * state `x`; the point's other parameters are the sliders' now.
   */
  const followHomoclinic = useCallback(
    (x: readonly number[], at: readonly [number, number]) => {
      const cur = latest.current;
      const v = cur.view;
      if (!isCurves(v)) return;
      const seed = homoclinicSeed(x, cur.params.map((p) => p.value), [v.across, v.up], at);
      updateCurves((w) => ({ ...w, curves: { ...w.curves, homoclinic: withHomoclinicSeed(w.curves.homoclinic, seed) } }));
    },
    [updateCurves],
  );

  /**
   * Removes a homoclinic seed. The job running for it, if any, is cancelled
   * by the effect that watches the wanted keys, and nothing of it is kept.
   */
  const removeHomoclinic = useCallback(
    (index: number) =>
      updateCurves((v) => ({ ...v, curves: { ...v.curves, homoclinic: v.curves.homoclinic.filter((_, i) => i !== index) } })),
    [updateCurves],
  );

  /** Ends a search: the seeds not computed yet are dropped, the curves already drawn stay. */
  const stopCurves = useCallback(() => {
    const done = new Set(
      buildCurveJobs(latest.current)
        .filter((j) => j.meta.key in latest.current.curveRuns)
        .map((j) => j.meta.seed),
    );
    updateCurves((v) => ({ ...v, curves: { ...v.curves, seeds: v.curves.seeds.filter((_, i) => done.has(i)) } }));
  }, [updateCurves]);

  /**
   * Opens the Parameter plane from the Branch view with `seeds` added, up
   * being `up` (else the remembered plane's, else the next parameter).
   */
  const openPlaneFromBranch = useCallback((seeds: PlaneSeed[], up?: number) => {
    const cur = latest.current;
    const sys = cur.compiled;
    if (!sys || !isContinuation(cur.view) || !curvesAvailability(sys).ok) return;
    const mem = viewMemory.current;
    if (mem.shapeId !== cur.shapeId) viewMemory.current = { shapeId: cur.shapeId, views: {} };
    const views = viewMemory.current.views;
    views.continuation = cur.view;
    const kept = views.curves && isCurves(views.curves) ? views.curves : null;
    const next = openPlane(
      kept,
      cur.view,
      cur.view,
      cur.params,
      seeds,
      up,
    );
    refitCurves.current = true;
    setViewState(next);
    setHomeView(next);
  }, []);

  const followInPlane = useCallback(
    (at: SpecialPointOut, up: number) => {
      const cur = latest.current;
      if (!isContinuation(cur.view)) return;
      const seed = planeSeed(at, cur.params.map((p) => p.value), cur.view.param);
      openPlaneFromBranch(seed ? [seed] : [], up);
    },
    [openPlaneFromBranch],
  );

  const searchInPlane = useCallback(() => {
    const cur = latest.current;
    const sys = cur.compiled;
    if (!sys || !isContinuation(cur.view)) return;
    openPlaneFromBranch(branchSeeds(sys, cur.params, cur.view, cur.branches));
  }, [openPlaneFromBranch]);

  /** Sets the plane's two parameters at once, as a click on the plane does. */
  const setPlaneParams = useCallback((a: number, b: number) => {
    const v = latest.current.view;
    const ps = latest.current.params;
    if (!isCurves(v) || !Number.isFinite(a) || !Number.isFinite(b)) return;
    const names = [ps[v.across]?.name, ps[v.up]?.name];
    setParams((list) =>
      list.map((p) => {
        const value = p.name === names[0] ? a : p.name === names[1] ? b : null;
        return value === null ? p : { ...p, value, min: Math.min(p.min, value), max: Math.max(p.max, value) };
      }),
    );
  }, []);

  const setOrbitColumns = useCallback((columns: number) => {
    setOrbitColumnsState((old) => (old === columns ? old : columns));
  }, []);

  const setSlice = useCallback((index: number, value: number) => {
    setBaseState((b) => {
      const copy = b.slice();
      copy[index] = value;
      return copy;
    });
  }, []);

  const setFieldTime = useCallback((t: number) => setFieldTimeState(t), []);

  const liftPoint = useCallback((px: number, py: number, base?: InitialCondition) => {
    const cur = latest.current;
    if (isParameterView(cur.view)) return { x0: cur.baseState.slice(), t0: cur.fieldTime };
    return liftPlotPoint(cur.compiled, cur.view, cur.baseState, cur.fieldTime, px, py, base);
  }, []);

  const addIc = useCallback(
    (px: number, py: number): number => {
      const { x0, t0 } = liftPoint(px, py);
      const ic = makeIc(x0, t0);
      const view = latest.current.view;
      setIcs((list) => [...list, { ...ic, color: newIcColor(list, view) }]);
      setSelection({ kind: "ic", id: ic.id });
      return ic.id;
    },
    [liftPoint, makeIc],
  );

  const moveIc = useCallback(
    (id: number, px: number, py: number) => {
      setIcs((list) =>
        list.map((ic) => {
          if (ic.id !== id) return ic;
          const { x0, t0 } = liftPoint(px, py, ic);
          return { ...ic, x0, t0 };
        }),
      );
    },
    [liftPoint],
  );

  const updateIc = useCallback((id: number, x0: number[], t0: number) => {
    if (x0.some((v) => !Number.isFinite(v)) || !Number.isFinite(t0)) return;
    setIcs((list) => list.map((ic) => (ic.id === id ? { ...ic, x0, t0 } : ic)));
  }, []);

  const removeIcs = useCallback((ids: number[]) => {
    setIcs((list) => list.filter((ic) => !ids.includes(ic.id)));
    setSelection((s) => (s?.kind === "ic" && ids.includes(s.id) ? null : s));
  }, []);

  const clearIcs = useCallback(() => {
    setIcs([]);
    setSelection((s) => (s?.kind === "ic" ? null : s));
  }, []);

  const setLayers = useCallback((patch: Partial<Layers>) => setLayersState((l) => ({ ...l, ...patch })), []);
  const setSolver = useCallback((patch: Partial<SolverSettings>) => {
    const cur = latest.current;
    const merged = { ...cur.solver, ...patch };
    const next = { ...merged, transient: clampTransient(merged.transient, merged.tEnd) };
    latest.current = { ...cur, solver: next };
    setSolverState(next);
    if (patch.method) setMethodNote(null);
    // A map's orbits start at n = T: a time series follows the transient.
    if (cur.compiled?.kind === "map") {
      const dt = next.transient - clampTransient(cur.solver.transient, cur.solver.tEnd);
      if (dt !== 0) {
        setViewState((v) => shiftTimeAxes(v, dt));
        setHomeView((v) => shiftTimeAxes(v, dt));
        const kept = viewMemory.current.views;
        if (kept.series) kept.series = shiftTimeAxes(kept.series, dt);
      }
    }
  }, []);

  const actions = useMemo(
    () => ({
      loadTemplate,
      newBlank,
      loadProject,
      setEquation,
      setFormat,
      addEquation,
      removeEquation,
      setParam,
      setParamRange,
      setView,
      autoAdjustView,
      resetView,
      setPlotAspect,
      setAxis,
      setViewKind,
      setSection,
      setLyapunov,
      setOrbit,
      setOrbitParam,
      setOrbitRange,
      setOrbitColumns,
      setContinuation,
      setContinuationParam,
      setContinuationAxis,
      addSeed,
      removeSeed,
      continueFrom,
      setCurves,
      setCurvesParam,
      removeCurveSeed,
      stopCurves,
      followHomoclinic,
      removeHomoclinic,
      followInPlane,
      searchInPlane,
      setPlaneParams,
      setCamera,
      setSpaceAxis,
      setSlice,
      setFieldTime,
      addIc,
      moveIc,
      updateIc,
      setIcHidden,
      removeIcs,
      clearIcs,
      setLayers,
      setSolver,
      select: setSelection,
    }),
    [
      loadTemplate,
      newBlank,
      loadProject,
      setEquation,
      setFormat,
      addEquation,
      removeEquation,
      setParam,
      setParamRange,
      setView,
      autoAdjustView,
      resetView,
      setPlotAspect,
      setAxis,
      setViewKind,
      setSection,
      setLyapunov,
      setOrbit,
      setOrbitParam,
      setOrbitRange,
      setOrbitColumns,
      setContinuation,
      setContinuationParam,
      setContinuationAxis,
      addSeed,
      removeSeed,
      continueFrom,
      setCurves,
      setCurvesParam,
      removeCurveSeed,
      stopCurves,
      followHomoclinic,
      removeHomoclinic,
      followInPlane,
      searchInPlane,
      setPlaneParams,
      setCamera,
      setSpaceAxis,
      setSlice,
      setFieldTime,
      addIc,
      moveIc,
      updateIc,
      setIcHidden,
      removeIcs,
      clearIcs,
      setLayers,
      setSolver,
    ],
  );

  return { state, actions };
}

export type StudioActions = Studio["actions"];
