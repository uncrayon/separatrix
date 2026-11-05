// The studio state <-> a project file (.sepx, docs/project-format.md).
//
// Saving merges the state onto the file as it was opened, at every level, so
// keys this version doesn't know (a v2 `assignment`, a second view, an
// analysis) are written back as they were read. Opening turns the file into a
// `ProjectLoad` by name (states, parameters, axes), which the studio applies
// once the lines have compiled (`applyProjectLoad` in state/model.ts).
//
// Node's unit tests import this module directly, so it has only type imports.

import type {
  AxisRef,
  InputFormat,
  Method,
  ProjectAxis,
  ProjectFile,
  ProjectInitialCondition,
  ProjectParam,
  ProjectView,
  SystemKind,
} from "../api/types";
import type { EquationLine, InitialCondition, Layers, ParamState, SolverSettings, View } from "../state/model";

/** Keys this version writes; anything else in a loaded object is kept as read. */
type Obj = Record<string, unknown>;

/** The method names `dyn_solve::Method` knows (mirrors `KNOWN_METHODS` in crates/project). */
export const KNOWN_METHODS: readonly Method[] = ["euler", "heun", "rk4", "dopri5"];

const METHOD_NAMES: Record<Method, string> = {
  euler: "Euler",
  heun: "Heun",
  rk4: "Runge–Kutta 4",
  dopri5: "Dormand–Prince 5(4)",
};

/** A saved axis, by name. `unknown` is a kind this version can't show (a v2 parameter axis). */
export type LoadedAxis = { kind: "time" } | { kind: "state"; name: string } | { kind: "unknown"; kindName: string };

export interface LoadedView {
  id: string;
  xAxis: LoadedAxis;
  yAxis: LoadedAxis;
  /** Null when the saved range isn't finite with lo < hi. */
  xRange: [number, number] | null;
  yRange: [number, number] | null;
  /** Slice values by state name. */
  slice: Record<string, number>;
  t: number;
  layers: Partial<Layers>;
}

export interface LoadedIc {
  uid: string;
  /** In the order of {@link ProjectLoad.states}. */
  x0: number[];
  t0: number;
  color: number;
  hidden: boolean;
}

export interface LoadedParam {
  name: string;
  value: number;
  min: number;
  max: number;
}

/** A project file as the studio loads it: everything by name, applied after the compile. */
export interface ProjectLoad {
  format: InputFormat;
  /** Exactly as saved, blank lines included. */
  lines: string[];
  templateId: string | null;
  /** The state names the `x0` arrays were written in. */
  states: string[];
  params: LoadedParam[];
  ics: LoadedIc[];
  /** The primary view: id `main`, otherwise the first. */
  view: LoadedView | null;
  solver: Partial<SolverSettings>;
  /** Sentences for the open banner. */
  warnings: string[];
}

/** What of the studio state a project file holds. */
export interface DocumentState {
  format: InputFormat;
  equations: readonly EquationLine[];
  compiled: { kind?: SystemKind; states: readonly { name: string }[] } | null;
  params: readonly ParamState[];
  view: View;
  baseState: readonly number[];
  fieldTime: number;
  ics: readonly InitialCondition[];
  layers: Layers;
  solver: SolverSettings;
  sourceTemplateId: string | null;
}

export interface SerializeOptions {
  /** The file as opened (or last saved): the base every section is merged onto. */
  loaded?: ProjectFile | null;
  /**
   * The opened file's system hasn't compiled yet (`StudioState.pendingProject`):
   * its parameters, start points, views and solver are written exactly as
   * read, and only the lines and input format come from the editor, so a
   * system that doesn't compile can never wipe the file's data.
   */
  pending?: unknown;
  /** The export dialog's options for this file; `undefined` keeps the loaded ones. */
  exportSettings?: unknown;
  /** The running app's version (Rust stamps its own on write). */
  appVersion?: string;
}

const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const num = (v: unknown): number | undefined => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
const bool = (v: unknown): boolean | undefined => (typeof v === "boolean" ? v : undefined);
const str = (v: unknown): string | undefined => (typeof v === "string" ? v : undefined);

/** The primary view of a file: id `main`, otherwise the first. */
export function primaryViewIndex(views: readonly { id?: unknown }[] | undefined): number {
  if (!views || views.length === 0) return -1;
  const main = views.findIndex((v) => isObj(v) && v.id === "main");
  return main >= 0 ? main : 0;
}

/** The id a start point is saved under. */
export const icUid = (ic: Pick<InitialCondition, "id" | "uid">): string => ic.uid ?? `ic-${ic.id}`;

/** Unique ids in order: a repeat gets `-2`, `-3`, … (as validation does on open). */
function uniqueIds(ids: string[]): string[] {
  const used = new Set<string>();
  return ids.map((id) => {
    let out = id || "ic";
    for (let k = 2; used.has(out); k++) out = `${id}-${k}`;
    used.add(out);
    return out;
  });
}

function axisOut(prev: ProjectAxis | undefined, a: AxisRef, states: readonly string[]): ProjectAxis {
  const fresh: ProjectAxis = a.kind === "time" ? { kind: "time" } : { kind: "state", name: states[a.index] };
  if (fresh.kind === "state" && fresh.name === undefined) delete fresh.name;
  // A sub-object whose kind changed is written fresh: no stale `name` on a time axis.
  if (!prev || prev.kind !== fresh.kind) return fresh;
  const out: ProjectAxis = { ...prev, ...fresh };
  if (fresh.kind === "time") delete out.name;
  return out;
}

function byKey<T>(list: readonly T[] | undefined, key: (t: T) => unknown): Map<unknown, T> {
  const out = new Map<unknown, T>();
  for (const t of list ?? []) if (isObj(t) && !out.has(key(t))) out.set(key(t), t);
  return out;
}

/**
 * The project file for `s`, merged onto `loaded` at every level: known keys
 * from the state, every other key as it was read. Rust checks it and stamps
 * the version fields when it writes.
 */
export function projectFromState(s: DocumentState, opts: SerializeOptions = {}): ProjectFile {
  const loaded = opts.loaded ?? null;
  const pending = !!opts.pending && !!loaded;
  const lsys = loaded?.system;
  const states = pending ? (lsys?.states ?? []) : (s.compiled?.states.map((st) => st.name) ?? lsys?.states ?? []);

  const loadedParams = byKey<ProjectParam>(lsys?.params, (p) => p.id);
  const params: ProjectParam[] = pending
    ? (lsys?.params ?? [])
    : s.params.map((p) => ({ ...loadedParams.get(p.name), id: p.name, value: p.value, min: p.min, max: p.max }));

  const system = {
    ...lsys,
    kind: pending ? (lsys?.kind ?? "ode") : (s.compiled?.kind ?? lsys?.kind ?? "ode"),
    input_format: s.format,
    lines: s.equations.map((l) => l.source),
    states: [...states],
    params,
    template: s.sourceTemplateId,
  };

  let initialConditions: ProjectInitialCondition[];
  let views: ProjectView[];
  let solver: ProjectFile["solver"];
  if (pending && loaded) {
    initialConditions = loaded.initial_conditions ?? [];
    views = loaded.views ?? [];
    solver = loaded.solver;
  } else {
    const loadedIcs = byKey<ProjectInitialCondition>(loaded?.initial_conditions, (ic) => ic.id);
    const ids = uniqueIds(s.ics.map(icUid));
    initialConditions = s.ics.map((ic, k) => ({
      ...loadedIcs.get(icUid(ic)),
      id: ids[k],
      x0: [...ic.x0],
      t0: ic.t0,
      color: ic.color,
      hidden: !!ic.hidden,
    }));

    const lviews = loaded?.views ?? [];
    const at = primaryViewIndex(lviews);
    const lv = at >= 0 ? lviews[at] : undefined;
    const primary: ProjectView = {
      ...lv,
      id: lv?.id || "main",
      x_axis: axisOut(lv?.x_axis, s.view.xAxis, states),
      y_axis: axisOut(lv?.y_axis, s.view.yAxis, states),
      x_range: [s.view.xRange[0], s.view.xRange[1]],
      y_range: [s.view.yRange[0], s.view.yRange[1]],
      slice: {
        ...lv?.slice,
        states: Object.fromEntries(states.map((name, i) => [name, s.baseState[i] ?? 0])),
        t: s.fieldTime,
      },
      layers: {
        ...lv?.layers,
        field: s.layers.field,
        field_grid: s.layers.fieldGrid,
        nullclines: s.layers.nullclines,
        fixed_points: s.layers.fixedPoints,
        backward: s.layers.backward,
      },
    };
    views = at >= 0 ? lviews.map((v, k) => (k === at ? primary : v)) : [primary];

    solver = {
      ...loaded?.solver,
      method: s.solver.method,
      rtol: s.solver.rtol,
      atol: s.solver.atol,
      h: s.solver.h,
      max_steps: s.solver.maxSteps,
      t_end: s.solver.tEnd,
      samples: s.solver.samples,
    };
  }

  const out: ProjectFile & Obj = {
    ...loaded,
    format: "separatrix",
    format_version: loaded?.format_version ?? 1,
    min_reader_version: loaded?.min_reader_version ?? 1,
    app_version: opts.appVersion ?? loaded?.app_version ?? "",
    system,
    initial_conditions: initialConditions,
    views,
    analyses: loaded?.analyses ?? [],
    solver,
  };
  const exportSettings = opts.exportSettings !== undefined ? opts.exportSettings : loaded?.export_settings;
  if (exportSettings === undefined) delete out.export_settings;
  else out.export_settings = exportSettings;
  return out;
}

/**
 * What dirty tracking compares: the file `s` would save, without the parts
 * that don't make a document "changed" (the export options, the app version).
 */
export function dirtyKey(s: DocumentState, opts: Omit<SerializeOptions, "exportSettings" | "appVersion"> = {}): string {
  const p: Obj = { ...projectFromState(s, opts) };
  delete p.export_settings;
  delete p.app_version;
  return JSON.stringify(p);
}

function loadAxis(a: unknown): LoadedAxis {
  const kind = isObj(a) ? str(a.kind) : undefined;
  if (kind === "time") return { kind: "time" };
  if (kind === "state" && isObj(a) && typeof a.name === "string") return { kind: "state", name: a.name };
  return { kind: "unknown", kindName: kind ?? "" };
}

function loadRange(r: unknown): [number, number] | null {
  if (!Array.isArray(r) || r.length !== 2) return null;
  const [lo, hi] = [num(r[0]), num(r[1])];
  return lo !== undefined && hi !== undefined && lo < hi ? [lo, hi] : null;
}

function definedOnly<T extends Obj>(o: T): Partial<T> {
  return Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as Partial<T>;
}

/**
 * A project file as the studio loads it. Rust has already checked and
 * repaired it (`project_read`); this reads defensively all the same, as the
 * browser preview's files are not checked.
 */
export function stateFromProject(doc: ProjectFile): ProjectLoad {
  const warnings: string[] = [];
  const sys = isObj(doc.system) ? doc.system : ({} as Partial<ProjectFile["system"]>);
  const format: InputFormat = sys.input_format === "text" ? "text" : "latex";
  const lines = Array.isArray(sys.lines) ? sys.lines.map((l) => (typeof l === "string" ? l : String(l))) : [];
  const states = Array.isArray(sys.states) ? sys.states.filter((n): n is string => typeof n === "string") : [];

  const params: LoadedParam[] = (Array.isArray(sys.params) ? sys.params : []).flatMap((p) => {
    if (!isObj(p) || typeof p.id !== "string") return [];
    const value = num(p.value);
    if (value === undefined) return [];
    return [{ name: p.id, value, min: num(p.min) ?? NaN, max: num(p.max) ?? NaN }];
  });

  const ics: LoadedIc[] = (Array.isArray(doc.initial_conditions) ? doc.initial_conditions : []).flatMap((ic, k) => {
    if (!isObj(ic) || !Array.isArray(ic.x0)) return [];
    return [
      {
        uid: typeof ic.id === "string" && ic.id ? ic.id : `ic-${k + 1}`,
        x0: ic.x0.map((v) => num(v) ?? 0),
        t0: num(ic.t0) ?? 0,
        color: num(ic.color) ?? 0,
        hidden: bool(ic.hidden) ?? false,
      },
    ];
  });

  const views = Array.isArray(doc.views) ? doc.views : [];
  const at = primaryViewIndex(views);
  const v = at >= 0 && isObj(views[at]) ? (views[at] as ProjectView & Obj) : null;
  let view: LoadedView | null = null;
  if (v) {
    const slice: Record<string, number> = {};
    const sliceStates: Obj = isObj(v.slice) && isObj(v.slice.states) ? v.slice.states : {};
    for (const [name, value] of Object.entries(sliceStates)) {
      const n = num(value);
      if (n !== undefined) slice[name] = n;
    }
    const layers: Obj = isObj(v.layers) ? v.layers : {};
    view = {
      id: str(v.id) || "main",
      xAxis: loadAxis(v.x_axis),
      yAxis: loadAxis(v.y_axis),
      xRange: loadRange(v.x_range),
      yRange: loadRange(v.y_range),
      slice,
      t: (isObj(v.slice) ? num(v.slice.t) : undefined) ?? 0,
      layers: definedOnly({
        field: bool(layers.field),
        fieldGrid: num(layers.field_grid),
        nullclines: bool(layers.nullclines),
        fixedPoints: bool(layers.fixed_points),
        backward: bool(layers.backward),
      }),
    };
  }

  const sv: Obj = isObj(doc.solver) ? doc.solver : {};
  let method: Method | undefined;
  if (typeof sv.method === "string") {
    if ((KNOWN_METHODS as readonly string[]).includes(sv.method)) method = sv.method as Method;
    else {
      method = "dopri5";
      warnings.push(
        `The solver method “${sv.method}” isn't available in this version, so ${METHOD_NAMES.dopri5} is used.`,
      );
    }
  }
  const solver = definedOnly({
    method,
    rtol: num(sv.rtol),
    atol: num(sv.atol),
    h: num(sv.h),
    maxSteps: num(sv.max_steps),
    tEnd: num(sv.t_end),
    samples: num(sv.samples),
  }) as Partial<SolverSettings>;

  return {
    format,
    lines,
    templateId: typeof sys.template === "string" ? sys.template : null,
    states,
    params,
    ics,
    view,
    solver,
    warnings,
  };
}
