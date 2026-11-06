// The studio state <-> a project file (.sepx, docs/project-format.md).
//
// Saving merges the state onto the file as it was opened, at every level, so
// keys this version doesn't know (a v2 `assignment`, a second view, an
// analysis) are written back as they were read. Opening turns the file into a
// `ProjectLoad` by name (states, parameters, axes), which the studio applies
// once the lines have compiled (`applyProjectLoad` in state/model.ts).

import type {
  AxisRef,
  InputFormat,
  Method,
  ProjectAxis,
  ProjectCamera,
  ProjectContinuation,
  ProjectFile,
  ProjectInitialCondition,
  ProjectLyapunov,
  ProjectOrbit,
  ProjectParam,
  ProjectSection,
  ProjectSeed,
  ProjectView,
  SectionSpec,
  SystemKind,
} from "../api/types";
import { DEFAULT_METHOD, METHODS } from "../state/methods.ts";
import type {
  Camera,
  ContinuationSeed,
  ContinuationSettings,
  EquationLine,
  InitialCondition,
  Layers,
  LyapunovSettings,
  OrbitSettings,
  ParamState,
  SolverSettings,
  View,
} from "../state/model";


/** Keys this version writes; anything else in a loaded object is kept as read. */
type Obj = Record<string, unknown>;

/** The method names `dyn_solve::Method` knows (mirrors `KNOWN_METHODS` in crates/project). */
export const KNOWN_METHODS = Object.keys(METHODS) as readonly Method[];

/**
 * A saved axis, by name. `next` is the state one iterate later (a cobweb's
 * x_{n+1}); `unknown` is a kind this version can't show (a v2 parameter axis).
 */
export type LoadedAxis =
  | { kind: "time" }
  | { kind: "state"; name: string }
  | { kind: "next"; name: string }
  | { kind: "exponent" }
  | { kind: "param"; name: string }
  | { kind: "norm" }
  | { kind: "unknown"; kindName: string };

export interface LoadedSeed {
  from?: string;
  x?: number[];
  param?: number;
  period?: number;
  direction?: string;
  kind?: string;
  tangent?: number[];
}

export interface LoadedContinuation {
  range?: [number, number] | null;
  ds?: number;
  maxPoints?: number;
  seeds: LoadedSeed[];
}

/** The most transient iterates a file may ask for (validate.rs clamps to this too). */
export const MAX_TRANSIENT = 1_000_000;

export interface LoadedSection {
  kind: string;
  state?: string;
  value?: number;
  direction?: string;
  period?: number;
  phase?: number;
}

export interface LoadedView {
  id: string;
  /** The saved `kind`; absent for a plane view (phase plane or time series). */
  kind?: string;
  section?: LoadedSection;
  xAxis: LoadedAxis;
  yAxis: LoadedAxis;
  zAxis?: LoadedAxis;
  /** Null when the saved range isn't finite with lo < hi. */
  xRange: [number, number] | null;
  yRange: [number, number] | null;
  zRange?: [number, number] | null;
  camera?: Partial<Camera>;
  lyapunov?: Partial<LyapunovSettings>;
  orbit?: Partial<OrbitSettings>;
  continuation?: LoadedContinuation;
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

function axisOut(prev: ProjectAxis | undefined, a: AxisRef, states: readonly string[], next = false): ProjectAxis {
  const fresh: ProjectAxis =
    a.kind === "time" ? { kind: "time" } : { kind: next ? "next" : "state", name: states[a.index] };
  if (fresh.kind !== "time" && fresh.name === undefined) delete fresh.name;
  // A sub-object whose kind changed is written fresh: no stale `name` on a time axis.
  if (!prev || prev.kind !== fresh.kind) return fresh;
  const out: ProjectAxis = { ...prev, ...fresh };
  if (fresh.kind === "time") delete out.name;
  return out;
}

function sectionOut(prev: ProjectSection | undefined, s: SectionSpec, states: readonly string[]): ProjectSection {
  const fresh: ProjectSection =
    s.kind === "plane"
      ? { kind: "plane", state: states[s.state], value: s.value, direction: s.direction }
      : { kind: "stroboscopic", period: s.period, phase: s.phase };
  if (fresh.state === undefined) delete fresh.state;
  return prev && isObj(prev) && prev.kind === fresh.kind ? { ...prev, ...fresh } : fresh;
}

const SEED_FROMS: readonly string[] = ["point", "switch"] satisfies ContinuationSeed["from"][];

function continuationOut(prev: ProjectContinuation | undefined, c: ContinuationSettings): ProjectContinuation {
  const saved = isObj(prev) && Array.isArray(prev.seeds) ? prev.seeds : [];
  const known = saved.filter((p) => isObj(p) && SEED_FROMS.includes(String(p.from)));
  const unknown = saved.filter((p) => !known.includes(p));
  const seeds = c.seeds.map((seed, i): ProjectSeed => {
    const fresh: ProjectSeed =
      seed.from === "point"
        ? { from: "point", x: [...seed.x], param: seed.param, period: seed.period, direction: seed.direction }
        : {
            from: "switch",
            x: [...seed.at.x],
            param: seed.at.p,
            period: seed.at.period,
            direction: seed.direction,
            kind: seed.at.kind,
            tangent: [...seed.at.tangent],
          };
    const old = known[i];
    if (!old || old.from !== fresh.from) return fresh;
    const out: ProjectSeed = { ...old, ...fresh };
    if (fresh.from === "point") {
      delete out.kind;
      delete out.tangent;
    }
    return out;
  });
  return {
    ...(isObj(prev) ? prev : {}),
    range: [c.range[0], c.range[1]],
    ds: c.ds,
    max_points: c.maxPoints,
    seeds: [...seeds, ...unknown],
  };
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
    const lv: ProjectView | undefined = at >= 0 ? lviews[at] : undefined;
    const cobweb = s.view.kind === "cobweb";
    const section = s.view.kind === "section" ? s.view.section : null;
    const space = s.view.kind === "space" ? s.view : null;
    const lyapunov = s.view.kind === "lyapunov" ? s.view.lyapunov : null;
    const orbit = s.view.kind === "orbit" ? s.view : null;
    const continuation = s.view.kind === "continuation" ? s.view : null;
    const swept = orbit ?? continuation;
    const next = cobweb || (!!section && sameStateAxes(s.view.xAxis, s.view.yAxis));
    const exponent: ProjectAxis = { kind: "exponent" };
    const paramAxis = (name: string | undefined): ProjectAxis => {
      const fresh: ProjectAxis = name === undefined ? { kind: "param" } : { kind: "param", name };
      return lv?.x_axis?.kind === "param" ? { ...lv.x_axis, ...fresh } : fresh;
    };
    const primary: ProjectView = {
      ...lv,
      id: lv?.id || "main",
      ...(cobweb || section || space || lyapunov || swept ? { kind: s.view.kind } : {}),
      ...(section ? { section: sectionOut(lv?.section, section, states) } : {}),
      ...(lyapunov
        ? {
            lyapunov: {
              ...(isObj(lv?.lyapunov) ? lv.lyapunov : {}),
              transient: lyapunov.transient,
              total: lyapunov.total,
              interval: lyapunov.interval,
              blocks: lyapunov.blocks,
            } as ProjectLyapunov,
          }
        : {}),
      ...(orbit
        ? {
            orbit: {
              ...(isObj(lv?.orbit) ? lv.orbit : {}),
              transient: orbit.orbit.transient,
              kept: orbit.orbit.kept,
            } as ProjectOrbit,
          }
        : {}),
      ...(continuation ? { continuation: continuationOut(lv?.continuation, continuation.continuation) } : {}),
      x_axis: swept ? paramAxis(s.params[swept.param]?.name) : axisOut(lv?.x_axis, s.view.xAxis, states),
      y_axis: lyapunov
        ? lv?.y_axis?.kind === "exponent"
          ? { ...lv.y_axis, ...exponent }
          : exponent
        : s.view.yAxis.kind === "norm"
          ? { ...(lv?.y_axis?.kind === "norm" ? lv.y_axis : {}), kind: "norm" }
          : axisOut(lv?.y_axis, s.view.yAxis, states, next),
      ...(space ? { z_axis: axisOut(lv?.z_axis, space.zAxis, states) } : {}),
      x_range: [s.view.xRange[0], s.view.xRange[1]],
      y_range: [s.view.yRange[0], s.view.yRange[1]],
      ...(space
        ? {
            z_range: [space.zRange[0], space.zRange[1]] as [number, number],
            camera: { ...(isObj(lv?.camera) ? lv.camera : {}), ...space.camera } as ProjectCamera,
          }
        : {}),
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
    if (!cobweb && !section && !space && !lyapunov && !swept) delete primary.kind;
    if (!section) delete primary.section;
    if (!lyapunov) delete primary.lyapunov;
    if (!orbit) delete primary.orbit;
    if (!continuation) delete primary.continuation;
    if (!space) {
      delete primary.z_axis;
      delete primary.z_range;
      delete primary.camera;
    }
    views = at >= 0 ? lviews.map((v, k) => (k === at ? primary : v)) : [primary];

    const transient = Math.round(s.solver.transient ?? 0);
    const sv: ProjectFile["solver"] = {
      ...loaded?.solver,
      method: s.solver.method,
      rtol: s.solver.rtol,
      atol: s.solver.atol,
      h: s.solver.h,
      max_steps: s.solver.maxSteps,
      t_end: s.solver.tEnd,
      ...(transient > 0 ? { transient } : {}),
      samples: s.solver.samples,
    };
    // Written only when some iterates are hidden.
    if (!(transient > 0)) delete sv.transient;
    solver = sv;
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

const sameStateAxes = (a: { kind: string; index?: number }, b: { kind: string; index?: number }) =>
  a.kind === "state" && b.kind === "state" && a.index === b.index;

function loadSection(s: unknown): LoadedSection | undefined {
  if (!isObj(s)) return undefined;
  return {
    kind: str(s.kind) ?? "",
    ...definedOnly({
      state: str(s.state),
      value: num(s.value),
      direction: str(s.direction),
      period: num(s.period),
      phase: num(s.phase),
    }),
  };
}

function loadAxis(a: unknown): LoadedAxis {
  const kind = isObj(a) ? str(a.kind) : undefined;
  if (kind === "time") return { kind: "time" };
  if (kind === "state" && isObj(a) && typeof a.name === "string") return { kind: "state", name: a.name };
  if (kind === "next" && isObj(a) && typeof a.name === "string") return { kind: "next", name: a.name };
  if (kind === "exponent") return { kind: "exponent" };
  if (kind === "param" && isObj(a) && typeof a.name === "string") return { kind: "param", name: a.name };
  if (kind === "norm") return { kind: "norm" };
  return { kind: "unknown", kindName: kind ?? "" };
}

function loadRange(r: unknown): [number, number] | null {
  if (!Array.isArray(r) || r.length !== 2) return null;
  const [lo, hi] = [num(r[0]), num(r[1])];
  return lo !== undefined && hi !== undefined && lo < hi ? [lo, hi] : null;
}

/** A saved transient: a whole number (a whole float such as 200.0 too) clamped to 0–{@link MAX_TRANSIENT}. */
function wholeTransient(v: unknown): number | undefined {
  const n = num(v);
  if (n === undefined) return undefined;
  return Math.min(MAX_TRANSIENT, Math.max(0, Math.round(n)));
}

const numbers = (v: unknown): number[] | undefined =>
  Array.isArray(v) && v.every((x) => num(x) !== undefined) ? (v as number[]) : undefined;

function loadContinuation(c: Obj): LoadedContinuation {
  return {
    ...definedOnly({ range: c.range !== undefined ? loadRange(c.range) : undefined, ds: num(c.ds), maxPoints: num(c.max_points) }),
    seeds: (Array.isArray(c.seeds) ? c.seeds : []).map((seed) =>
      isObj(seed)
        ? definedOnly({
            from: str(seed.from),
            x: numbers(seed.x),
            param: num(seed.param),
            period: num(seed.period),
            direction: str(seed.direction),
            kind: str(seed.kind),
            tangent: numbers(seed.tangent),
          })
        : {},
    ),
  };
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
    const kind = str(v.kind);
    const section = loadSection(v.section);
    const cam: Obj = isObj(v.camera) ? v.camera : {};
    const lyap: Obj = isObj(v.lyapunov) ? v.lyapunov : {};
    const orb: Obj = isObj(v.orbit) ? v.orbit : {};
    view = {
      id: str(v.id) || "main",
      ...(kind !== undefined ? { kind } : {}),
      ...(section ? { section } : {}),
      xAxis: loadAxis(v.x_axis),
      yAxis: loadAxis(v.y_axis),
      ...(v.z_axis !== undefined ? { zAxis: loadAxis(v.z_axis) } : {}),
      xRange: loadRange(v.x_range),
      yRange: loadRange(v.y_range),
      ...(v.z_range !== undefined ? { zRange: loadRange(v.z_range) } : {}),
      ...(isObj(v.camera) ? { camera: definedOnly({ yaw: num(cam.yaw), pitch: num(cam.pitch), zoom: num(cam.zoom) }) } : {}),
      ...(isObj(v.lyapunov)
        ? {
            lyapunov: definedOnly({
              transient: num(lyap.transient),
              total: num(lyap.total),
              interval: num(lyap.interval),
              blocks: num(lyap.blocks),
            }),
          }
        : {}),
      ...(isObj(v.orbit) ? { orbit: definedOnly({ transient: num(orb.transient), kept: num(orb.kept) }) } : {}),
      ...(isObj(v.continuation) ? { continuation: loadContinuation(v.continuation) } : {}),
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
      method = DEFAULT_METHOD;
      warnings.push(
        `The solver method “${sv.method}” isn't available in this version, so ${METHODS[DEFAULT_METHOD].label} is used.`,
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
    transient: wholeTransient(sv.transient),
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
