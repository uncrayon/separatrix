// All of the app's state, and every action that changes it, in one hook.
//
// Flow: equations → compile (debounced while typing) → params/view/ICs are
// adapted → any change schedules a portrait recompute (coalesced per frame).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AxisRef, CompileResponse, Diagnostic } from "../api/types";
import { compileSystem, PortraitScheduler } from "../api/client";
import type { Template } from "../templates";
import {
  DEFAULT_LAYERS,
  DEFAULT_SOLVER,
  PALETTE_SIZE,
  adaptInitialConditions,
  axisValid,
  buildPortraitJob,
  defaultParamRange,
  defaultView,
  stateExtent,
  templateView,
  type EquationLine,
  type InitialCondition,
  type Layers,
  type ParamState,
  type PortraitMeta,
  type PortraitState,
  type Selection,
  type SolverSettings,
  type StudioSnapshot,
  type View,
} from "./model";

const COMPILE_DEBOUNCE_MS = 160;

/** A blank starting point: the harmonic oscillator, ready to edit. */
const BLANK_TEMPLATE: Template = {
  id: "blank",
  title: "Untitled system",
  chapter: "",
  description: "",
  format: "latex",
  equations: ["\\dot{x}=y", "\\dot{y}=-x"],
  params: {},
  view: { x: "x", y: "y", xRange: [-4, 4], yRange: [-3, 3] },
  initialConditions: [],
  tEnd: 20,
};

export interface StudioState extends StudioSnapshot {
  equations: EquationLine[];
  diagnostics: Diagnostic[];
  /** False while the latest edit does not compile (the last good system stays on screen). */
  compileOk: boolean;
  homeView: View;
  selection: Selection;
  portrait: PortraitState | null;
  templateId: string | null;
  computeError: string | null;
}

export type Studio = ReturnType<typeof useStudio>;

/** The single source of truth for the UI. */
export function useStudio() {
  const [equations, setEquations] = useState<EquationLine[]>([]);
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
  const [templateId, setTemplateId] = useState<string | null>(null);
  const [computeError, setComputeError] = useState<string | null>(null);
  const [plotAspect, setPlotAspectState] = useState<number | null>(null);

  const state: StudioState = {
    equations,
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
    templateId,
    computeError,
    plotAspect,
  };
  // Async callbacks (compile results, the scheduler) read the newest state here.
  const latest = useRef(state);
  latest.current = state;

  const nextLineId = useRef(1);
  const nextIcId = useRef(1);
  const nextColor = useRef(0);
  const compileSeq = useRef(0);
  const compileTimer = useRef<number | undefined>(undefined);

  const makeIc = useCallback((x0: number[], t0: number): InitialCondition => {
    const ic = { id: nextIcId.current++, x0, t0, color: nextColor.current % PALETTE_SIZE };
    nextColor.current++;
    return ic;
  }, []);

  // ------------------------------------------------------------ portrait

  const scheduler = useMemo(
    () =>
      new PortraitScheduler<PortraitMeta>(
        () => buildPortraitJob(latest.current),
        (res, meta, roundTripMs) => {
          setPortrait({ res, meta, roundTripMs });
          setComputeError(null);
        },
        (err) => setComputeError(err instanceof Error ? err.message : String(err)),
      ),
    [],
  );
  useEffect(() => () => scheduler.dispose(), [scheduler]);

  useEffect(() => {
    scheduler.request();
  }, [scheduler, compiled, params, view, baseState, fieldTime, ics, layers, solver, plotAspect]);

  // ------------------------------------------------------------ compile

  /** Compile `lines`; when `template` is given, apply its params, view and ICs. */
  const runCompile = useCallback(
    async (lines: string[], template?: Template) => {
      const seq = ++compileSeq.current;
      const source = lines.filter((l) => l.trim() !== "").join("\n");
      let res: CompileResponse;
      try {
        res = await compileSystem({ source, format: "latex" });
      } catch (err) {
        if (seq !== compileSeq.current) return;
        setDiagnostics([{ severity: "error", message: `The maths core did not answer: ${String(err)}` }]);
        setCompileOk(false);
        return;
      }
      if (seq !== compileSeq.current) return; // a newer edit is already compiling
      setDiagnostics(res.diagnostics);
      setCompileOk(res.ok);
      if (!res.ok || res.systemId === undefined) return;

      const cur = latest.current;
      const old = cur.compiled;
      const dim = res.states.length;

      // Parameters: template value, else the current slider value, else the default.
      setParams(
        res.params.map((p) => {
          const prev = cur.params.find((q) => q.name === p.name);
          const value = template?.params[p.name] ?? (template ? undefined : prev?.value) ?? p.default;
          const range =
            template?.paramRanges?.[p.name] ??
            (prev && !template ? [prev.min, prev.max] : defaultParamRange(value));
          return {
            name: p.name,
            latex: p.latex,
            value,
            min: Math.min(range[0], value),
            max: Math.max(range[1], value),
          };
        }),
      );

      if (template) {
        const v = templateView(template, res);
        setViewState(v);
        setHomeView(v);
        setBaseState(new Array(dim).fill(0));
        setFieldTimeState(0);
        nextColor.current = 0;
        setIcs(
          template.initialConditions.map((x0) =>
            makeIc(
              Array.from({ length: dim }, (_, i) => x0[i] ?? 0),
              0,
            ),
          ),
        );
        setSolverState((s) => ({ ...s, tEnd: template.tEnd }));
        setLayersState((l) => ({
          ...l,
          backward: template.backward ?? true,
          // Nullclines get busy in projections of 3D flows; start with them off.
          nullclines: dim <= 2,
        }));
        setSelection(null);
      } else {
        const sameShape =
          old &&
          old.kind === res.kind &&
          old.states.map((s) => s.name).join() === res.states.map((s) => s.name).join();
        if (!sameShape) {
          const valid =
            axisValid(cur.view.xAxis, dim) && axisValid(cur.view.yAxis, dim) && old?.kind === res.kind;
          const v = valid ? cur.view : defaultView(res);
          setViewState(v);
          setHomeView(v);
          setBaseState((b) => Array.from({ length: dim }, (_, i) => b[i] ?? 0));
          setIcs((list) => adaptInitialConditions(list, old?.states ?? [], res.states));
          if (old?.kind !== res.kind) setSolverState((s) => ({ ...s, tEnd: res.kind === "map" ? 50 : 20 }));
        }
      }
      setCompiled(res);
    },
    [makeIc],
  );

  const scheduleCompile = useCallback(
    (lines: EquationLine[]) => {
      window.clearTimeout(compileTimer.current);
      compileTimer.current = window.setTimeout(
        () => void runCompile(lines.map((l) => l.latex)),
        COMPILE_DEBOUNCE_MS,
      );
    },
    [runCompile],
  );

  // ------------------------------------------------------------ actions

  const toLines = (latexes: string[]) => latexes.map((latex) => ({ id: nextLineId.current++, latex }));

  const loadTemplate = useCallback(
    (t: Template) => {
      window.clearTimeout(compileTimer.current);
      setEquations(toLines(t.equations));
      setTemplateId(t.id);
      void runCompile(t.equations, t);
    },
    [runCompile],
  );

  const newBlank = useCallback(() => {
    loadTemplate(BLANK_TEMPLATE);
    setTemplateId(null);
  }, [loadTemplate]);

  const setEquation = useCallback(
    (id: number, latex: string) => {
      const lines = latest.current.equations.map((l) => (l.id === id ? { ...l, latex } : l));
      setEquations(lines);
      setTemplateId(null);
      scheduleCompile(lines);
    },
    [scheduleCompile],
  );

  /** Add an empty line after `afterId` (or at the end). Returns its id. */
  const addEquation = useCallback((afterId?: number): number => {
    const line = { id: nextLineId.current++, latex: "" };
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
      setTemplateId(null);
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
  /** The plot's width / height, so a phase plane can use equal scales (rounded: no churn while resizing). */
  const setPlotAspect = useCallback((aspect: number | null) => {
    const a = aspect && Number.isFinite(aspect) && aspect > 0 ? Number(aspect.toPrecision(3)) : null;
    setPlotAspectState((old) => (old === a ? old : a));
  }, []);
  /** Show or hide one trajectory; a hidden one is not integrated, its start point stays. */
  const setIcHidden = useCallback((id: number, hidden: boolean) => {
    setIcs((list) => list.map((ic) => (ic.id === id ? { ...ic, hidden } : ic)));
  }, []);
  const resetView = useCallback(() => setViewState(latest.current.homeView), []);

  /** Put `axis` on the horizontal or vertical axis (swapping if it is already on the other). */
  const setAxis = useCallback((which: "x" | "y", axis: AxisRef) => {
    const cur = latest.current;
    const v = cur.view;
    const other = which === "x" ? v.yAxis : v.xAxis;
    const same = (a: AxisRef, b: AxisRef) =>
      a.kind === b.kind && (a.kind === "time" || (b.kind === "state" && a.index === b.index));
    const rangeFor = (a: AxisRef): [number, number] => {
      if (same(a, v.xAxis)) return v.xRange;
      if (same(a, v.yAxis)) return v.yRange;
      if (a.kind === "time") return [-0.04 * cur.solver.tEnd, cur.solver.tEnd];
      const ext = stateExtent(cur.portrait?.res, cur.compiled?.states.length ?? 0, a.index);
      if (ext) {
        const pad = Math.max(0.1 * (ext[1] - ext[0]), 0.5);
        return [ext[0] - pad, ext[1] + pad];
      }
      return [-5, 5];
    };
    let next: View;
    if (same(axis, other)) {
      next = { xAxis: v.yAxis, yAxis: v.xAxis, xRange: v.yRange, yRange: v.xRange };
    } else if (which === "x") {
      next = { ...v, xAxis: axis, xRange: rangeFor(axis) };
    } else {
      next = { ...v, yAxis: axis, yRange: rangeFor(axis) };
    }
    setViewState(next);
    setHomeView(next);
  }, []);

  const setSlice = useCallback((index: number, value: number) => {
    setBaseState((b) => {
      const copy = b.slice();
      copy[index] = value;
      return copy;
    });
  }, []);

  const setFieldTime = useCallback((t: number) => setFieldTimeState(t), []);

  /** The full state (and start time) for a point clicked on the canvas. */
  const liftPoint = useCallback((px: number, py: number, base?: InitialCondition) => {
    const cur = latest.current;
    const dim = cur.compiled?.states.length ?? 0;
    const x0 = Array.from({ length: dim }, (_, i) => base?.x0[i] ?? cur.baseState[i] ?? 0);
    let t0 = base?.t0 ?? (cur.compiled?.kind === "map" ? 0 : cur.fieldTime);
    const put = (a: AxisRef, v: number) => {
      if (a.kind === "time") t0 = cur.compiled?.kind === "map" ? 0 : v;
      else x0[a.index] = v;
    };
    put(cur.view.xAxis, px);
    put(cur.view.yAxis, py);
    return { x0, t0 };
  }, []);

  const addIc = useCallback(
    (px: number, py: number): number => {
      const { x0, t0 } = liftPoint(px, py);
      const ic = makeIc(x0, t0);
      setIcs((list) => [...list, ic]);
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
  const setSolver = useCallback(
    (patch: Partial<SolverSettings>) => setSolverState((s) => ({ ...s, ...patch })),
    [],
  );

  const actions = useMemo(
    () => ({
      loadTemplate,
      newBlank,
      setEquation,
      addEquation,
      removeEquation,
      setParam,
      setParamRange,
      setView,
      resetView,
      setPlotAspect,
      setAxis,
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
      setEquation,
      addEquation,
      removeEquation,
      setParam,
      setParamRange,
      setView,
      resetView,
      setPlotAspect,
      setAxis,
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
