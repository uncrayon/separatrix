// What an export asks the core for, built from the studio's state the same
// way the screen's portrait is (state/model.ts `buildPortraitJob`), with the
// figure's own aspect and the dialog's layers.
//
// - `plotAspect` is the figure's width over height, so a flow's phase plane
//   gets equal scales at the figure's shape (the ranges, the field and the
//   nullclines are recomputed for it).
// - `samples` is the screen's (or the automatic count), at least 2,000 and
//   at most 20,000; Rust simplifies the curves at 0.25 px afterwards.
// - Hidden initial conditions are left out, as on screen.

import type {
  CsvRequest,
  ExportPortraitRequest,
  InputFormat,
  PortraitRequest,
  PortraitResponse,
  TrajectoryTag,
} from "../api/types";
import type { Scene } from "../canvas/draw";
import { viewAxisLabels } from "../math/format";
import {
  COBWEB_GRAPH_SAMPLES,
  MAX_GRAPH_SAMPLES,
  buildPortraitJob,
  isCobweb,
  portraitGraph,
  type InitialCondition,
  type MapRequestFields,
  type StudioSnapshot,
  type TrajectoryMeta,
  type View,
} from "../state/model";

export const EXPORT_MIN_SAMPLES = 2000;
export const EXPORT_MAX_SAMPLES = 20000;

/** Samples per run for a figure, from the screen's count. */
export function exportSamples(screen: number): number {
  return Math.min(Math.max(screen, EXPORT_MIN_SAMPLES), EXPORT_MAX_SAMPLES);
}

/** Graph samples of f for a cobweb figure `widthPx` wide: two per px, from 2,048 to 4,096. */
export function exportGraphSamples(widthPx: number): number {
  return Math.min(Math.max(Math.round(2 * widthPx), COBWEB_GRAPH_SAMPLES), MAX_GRAPH_SAMPLES);
}

/** An initial condition's id in a project file: its `uid`, else `ic-<id>`. */
export function icProjectId(ic: InitialCondition): string {
  const uid = (ic as InitialCondition & { uid?: string }).uid;
  return typeof uid === "string" && uid !== "" ? uid : `ic-${ic.id}`;
}

export interface FigureLayers {
  field: boolean;
  nullclines: boolean;
  fixedPoints: boolean;
}

export interface FigureJob {
  /** For `compute_export_portrait` and `export_figure_latex`. */
  portrait: PortraitRequest;
  /** One per entry of `portrait.trajectories`. */
  tags: TrajectoryTag[];
  metas: TrajectoryMeta[];
  widthPx: number;
  heightPx: number;
  /** The view with the ranges the figure shows (equal scales applied). */
  view: View;
  kind: "ode" | "map";
  dim: number;
  /** A cobweb plot: the graph of f, the diagonal and the staircases. */
  cobweb: boolean;
}

/** The figure's portrait at `widthPx` × `heightPx` with `layers`, or null without a compiled system. */
export function buildFigureJob(
  s: StudioSnapshot,
  size: { widthPx: number; heightPx: number },
  layers: FigureLayers,
): FigureJob | null {
  const sys = s.compiled;
  if (!sys || !sys.ok || !(size.widthPx > 0) || !(size.heightPx > 0)) return null;
  const job = buildPortraitJob({
    ...s,
    plotAspect: size.widthPx / size.heightPx,
    layers: { ...s.layers, field: layers.field, nullclines: layers.nullclines, fixedPoints: layers.fixedPoints },
  });
  if (!job) return null;
  const colorOf = new Map(s.ics.map((ic) => [ic.id, ic]));
  const tags = job.meta.trajectories.map((m): TrajectoryTag => {
    const ic = colorOf.get(m.icId);
    return { icId: ic ? icProjectId(ic) : `ic-${m.icId}`, backward: m.direction === "backward", color: ic?.color ?? 0 };
  });
  const cobweb = isCobweb(s.view);
  const graph: MapRequestFields = cobweb ? { graphSamples: exportGraphSamples(size.widthPx) } : {};
  return {
    portrait: { ...job.body, requestId: 0, samples: exportSamples(job.body.samples), ...graph },
    tags,
    metas: job.meta.trajectories,
    widthPx: size.widthPx,
    heightPx: size.heightPx,
    view: job.meta.view,
    kind: sys.kind === "map" ? "map" : "ode",
    dim: sys.states.length,
    cobweb,
  };
}

export function exportPortraitRequest(job: FigureJob): ExportPortraitRequest {
  return { portrait: job.portrait, widthPx: job.widthPx, heightPx: job.heightPx };
}

/**
 * The scene of an exported figure: the screen's layers and colours, with no
 * selection, hover, eigenvectors, start-point handles or stale fade.
 */
export function exportScene(job: FigureJob, res: PortraitResponse, ics: InitialCondition[]): Scene {
  const colorOf = new Map(ics.map((ic) => [ic.id, ic.color]));
  return {
    kind: job.kind,
    dim: job.dim,
    xAxis: job.view.xAxis,
    yAxis: job.view.yAxis,
    arrows: res.arrows,
    fieldGrid: job.portrait.fieldGrid,
    nullclines: res.nullclines,
    fixedPoints: job.portrait.fixedPoints ? res.fixedPoints : [],
    trajectories: res.trajectories.flatMap((out, i) => {
      const meta = job.metas[i];
      const color = meta ? colorOf.get(meta.icId) : undefined;
      return meta && color !== undefined ? [{ out, meta, color }] : [];
    }),
    ics: [],
    selectedIc: null,
    selectedFp: null,
    hoverIc: null,
    hoverFp: null,
    eigenvectors: false,
    stale: false,
    avoid: { labels: [], field: [] },
    cobweb: job.cobweb ? { graph: portraitGraph(res) ?? [] } : null,
  };
}

/** The axis names as the screen shows them (LaTeX). */
export function axisNames(s: StudioSnapshot): { x: string; y: string } | null {
  const sys = s.compiled;
  if (!sys || !sys.ok) return null;
  return viewAxisLabels(s.view, sys.states, sys.kind);
}

/** The screen's own request, with fixed points on (the fixed-point table). */
export function screenPortrait(s: StudioSnapshot): PortraitRequest | null {
  const job = buildPortraitJob({ ...s, layers: { ...s.layers, fixedPoints: true } });
  return job ? { ...job.body, requestId: 0 } : null;
}

/** The CSV request: the screen's runs at the CSV's own samples (0: the screen's). */
export function buildCsvRequest(
  s: StudioSnapshot,
  opts: { icIds: string[] | null; samples: number },
  source: { lines: string[]; format: InputFormat; title: string },
): CsvRequest | null {
  const job = buildPortraitJob({ ...s, layers: { ...s.layers, field: false, nullclines: false, fixedPoints: false } });
  if (!job) return null;
  const byId = new Map(s.ics.map((ic) => [ic.id, ic]));
  const tags = job.meta.trajectories.map((m): TrajectoryTag => {
    const ic = byId.get(m.icId);
    return { icId: ic ? icProjectId(ic) : `ic-${m.icId}`, backward: m.direction === "backward", color: ic?.color ?? 0 };
  });
  const known = new Set(tags.map((t) => t.icId));
  const icIds = opts.icIds === null ? [] : opts.icIds.filter((id) => known.has(id));
  return {
    portrait: { ...job.body, requestId: 0, samples: opts.samples > 0 ? Math.round(opts.samples) : job.body.samples },
    tags,
    icIds,
    lines: source.lines,
    format: source.format,
    title: source.title,
  };
}
