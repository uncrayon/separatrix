import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  ContinuationFigureRequest,
  ContinuationLatex,
  CsvFileOut,
  FigureLatexRequest,
  FixedPointOut,
  LyapunovResponse,
  OrbitDiagramRequest,
  OrbitDiagramResponse,
  OrbitFigureRequest,
  PortraitResponse,
  SpecialPointOut,
  SystemKind,
} from "../api/types";
import { computeOrbitDiagram } from "../api/client";
import { drawAnalysisFigure, drawFigure, type AnalysisFigure } from "../canvas/figure";
import { lambdaName } from "../canvas/lyapunov";
import { makeViewport } from "../canvas/viewport";
import { Button, IconButton, NumberField, SegmentedControl, withMinus } from "../components/controls";
import { IconCheck, IconClose, IconCopy, IconError, IconInfo } from "../components/icons";
import { Tex } from "../math/Tex";
import { axisLatex, classificationTitle, fmt, latexToPlain, plural } from "../math/format";
import {
  hasNullclineLayer,
  isCobweb,
  isReturnMap,
  isSection,
  isSpace,
  lyapunovCurrent,
  lyapunovValueText,
  orbitColumns,
  scientificText,
  shownBranches,
  specialRows,
  systemTitle,
  SPECIAL_KINDS,
  isContinuation,
  type SpaceView,
} from "../state/model";
import { spaceNames } from "../canvas/space";
import type { StudioState } from "../state/useStudio";
import { templateRef, templateById } from "../templates";
import {
  clipboardWritePng,
  clipboardWriteText,
  computeExportPortrait,
  errorText,
  exportContinuationCsv,
  exportContinuationLatex,
  exportCsv,
  exportFigureLatex,
  exportFixedPointsLatex,
  exportLyapunovCsv,
  exportLyapunovLatex,
  exportOrbitCsv,
  exportOrbitLatex,
  exportSystemLatex,
  saveContinuationLatex,
  saveCsv,
  saveCsvFile,
  saveFigureLatex,
  saveOrbitLatex,
  savePngFile,
  saveTextFile,
} from "./commands";
import { ExportPreview } from "./ExportPreview";
import {
  analysisKind,
  axisNames,
  buildContinuationExport,
  buildCsvRequest,
  buildFigureJob,
  buildLyapunovExport,
  buildOrbitExport,
  exportPortraitRequest,
  exportScene,
  exportSpaceFigure,
  icProjectId,
  orbitFigureRatio,
  screenPortrait,
  type ExportSource,
  type FigureJob,
} from "./job";
import { SPACE_LATEX_REFUSAL } from "../api/ipc.generated";
import {
  CSV_MAX_SAMPLES,
  fileStem,
  figurePx,
  mergeExportSettings,
  pngSizeError,
  readExportOptions,
  sizeError,
  type ExportFormat,
  type ExportOptions,
  type LatexContent,
  type Notation,
  type PngScale,
} from "./options";
import { pngRaster, renderPng, spacePngDraw } from "./png";
import { canvasMeasure, figureSvg } from "./svg";
import type { ExportPalette } from "./theme";
import { EXPORT_FONTS, exportTheme } from "./tokens";
import "../styles/areas/export.css";

interface Props {
  state: StudioState;
  /** The system's name ("Rabbits vs sheep"); from the template by default. */
  title?: string;
  /** Where it comes from ("Strogatz §6.4"); from the template by default. */
  sourceRef?: string;
  /**
   * The stem of the files Save… suggests: the project file's ("hw" for
   * hw.sepx), else a slug of the system's name.
   */
  fileStem?: string;
  /** The project's `export_settings`: the options last used for this file. */
  settings?: unknown;
  /**
   * Called with the new `export_settings` whenever the options change: the
   * options merged onto `settings`, so keys this version doesn't know survive.
   */
  onSettingsChange?: (settings: Record<string, unknown>) => void;
  onClose: () => void;
}

const FORMATS: { value: ExportFormat; label: string }[] = [
  { value: "latex", label: "LaTeX (TikZ/pgfplots)" },
  { value: "svg", label: "SVG" },
  { value: "png", label: "PNG" },
  { value: "csv", label: "CSV" },
];

const CONTENT_LABELS: Record<LatexContent, string> = {
  figure: "Figure",
  spectrum: "Spectrum",
  specialPoints: "Special points",
  equations: "Equations",
  fixedPoints: "Fixed points",
};

const PALETTES: { value: ExportPalette; label: string }[] = [
  { value: "light", label: "Projector" },
  { value: "dark", label: "Dark" },
];

const NOTATIONS: { value: Notation; label: string; title: string; ariaLabel: string }[] = [
  { value: "dot", label: "ẋ", title: "Dot notation (Strogatz)", ariaLabel: "Dot notation" },
  { value: "prime", label: "x′", title: "Prime notation", ariaLabel: "Prime notation" },
  { value: "leibniz", label: "dx/dt", title: "Leibniz notation", ariaLabel: "Leibniz notation" },
];

const SCALES: { value: `${PngScale}`; label: string }[] = [
  { value: "2", label: "2×" },
  { value: "3", label: "3×" },
  { value: "4", label: "4×" },
];

/** How long a Copy button reads "Copied". */
const COPIED_MS = 2000;
/** Options settle for this long before the core is asked again. */
const DEBOUNCE_MS = 120;

function tabbables(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'button:not([disabled]):not([tabindex="-1"]), input:not([disabled]), [tabindex="0"]',
    ),
  );
}

/** The snippet with its comment lines dimmed. */
function Snippet({ text, labelledBy }: { text: string; labelledBy: string }) {
  const lines = text.replace(/\n$/, "").split("\n");
  return (
    <pre className="ds-export-snippet" tabIndex={0} aria-labelledby={labelledBy}>
      <code>
        {lines.map((l, i) => (
          <span key={i}>
            {/^\s*%/.test(l) ? <span className="ds-export-cm">{l}</span> : l}
            {i < lines.length - 1 ? "\n" : ""}
          </span>
        ))}
      </code>
    </pre>
  );
}

function Check({
  checked,
  onChange,
  label,
  reason,
  describedBy,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  reason?: string | null;
  describedBy?: string;
}) {
  return (
    <label className="ds-checkbox">
      <input
        type="checkbox"
        className="ds-checkbox-input"
        checked={checked && !reason}
        disabled={!!reason}
        aria-describedby={describedBy}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{reason ? `${label} (${reason})` : label}</span>
    </label>
  );
}

/** A message in the dialog. Not a live region: the dialog's one `.ds-sr` region says each outcome once. */
function Notice({ kind, children }: { kind: "error" | "info"; children: ReactNode }) {
  return (
    <ul className="ds-diagnostic-list ds-diagnostic-list--block">
      <li className={`ds-diagnostic ds-diagnostic--block ds-diagnostic--${kind}`}>
        {kind === "error" ? <IconError className="ds-icon ds-diagnostic-icon" /> : <IconInfo className="ds-icon ds-diagnostic-icon" />}
        <span className="ds-diagnostic-body">
          <span className="ds-diagnostic-word">{kind === "error" ? "Error" : "Note"}</span>
          <span className="ds-diagnostic-msg">{children}</span>
        </span>
      </li>
    </ul>
  );
}

type Snip = { key: string; tex: string; notes: string[]; previewLatex?: string } | null;

const num = (v: number, digits: number) => withMinus(fmt(v, digits, 1e-12));

/** Eigenvalues as the table writes them: a complex pair once, as a ± bi. */
function eigenCell(ev: [number, number][], digits: number): string {
  const out: string[] = [];
  const used = ev.map(() => false);
  ev.forEach(([re, im], i) => {
    if (used[i]) return;
    used[i] = true;
    if (Math.abs(im) < 1e-12) {
      out.push(num(re, digits));
      return;
    }
    const j = ev.findIndex(([r2, i2], k) => !used[k] && k > i && Math.abs(r2 - re) < 1e-12 && Math.abs(i2 + im) < 1e-12);
    if (j >= 0) used[j] = true;
    const head = Math.abs(re) < 1e-12 ? "" : `${num(re, digits)} `;
    out.push(j >= 0 ? `${head}± ${num(Math.abs(im), digits)}i` : `${head}${im < 0 ? "−" : "+"} ${num(Math.abs(im), digits)}i`);
  });
  return out.join(", ");
}

/** The fixed-points table as it will be written: the screen's points at the chosen digits. */
function FixedPointsPreview({
  fps,
  kind,
  dim,
  digits,
}: {
  fps: FixedPointOut[];
  kind: SystemKind;
  dim: number;
  digits: number;
}) {
  const plane = kind === "ode" && dim === 2;
  const map = kind === "map";
  if (fps.length === 0) return <p className="ds-export-hint export-hint-flush">No fixed points found in the view.</p>;
  return (
    <table className="export-fp-table numeric-sm">
      <thead>
        <tr>
          <th scope="col">Fixed point</th>
          <th scope="col">{map ? (dim === 1 ? "λ = f′(x*)" : "Multipliers μ") : dim === 1 ? "λ" : "Eigenvalues λ"}</th>
          {map && <th scope="col">{dim === 1 ? "|λ|" : "|μ|"}</th>}
          {plane && <th scope="col">τ</th>}
          {plane && <th scope="col">Δ</th>}
          <th scope="col">Type</th>
        </tr>
      </thead>
      <tbody>
        {fps.map((fp, k) => {
          const ev = fp.eigenvalues;
          const tau = ev.reduce((a, [re]) => a + re, 0);
          const delta = ev.length === 2 ? ev[0][0] * ev[1][0] - ev[0][1] * ev[1][1] : NaN;
          return (
            <tr key={k}>
              <td>{fp.x.length === 1 ? num(fp.x[0], digits) : `(${fp.x.map((v) => num(v, digits)).join(", ")})`}</td>
              <td>{ev.length ? eigenCell(ev, digits) : "–"}</td>
              {map && <td>{ev.length ? ev.map(([re, im]) => num(Math.hypot(re, im), digits)).join(", ") : "–"}</td>}
              {plane && <td>{ev.length === 2 ? num(tau, digits) : "–"}</td>}
              {plane && <td>{ev.length === 2 ? num(delta, digits) : "–"}</td>}
              <td className="body">{classificationTitle(fp.classification, dim)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function SpecialPointsPreview({ rows, param, map }: { rows: SpecialPointOut[]; param: string; map: boolean }) {
  if (rows.length === 0) return <p className="ds-export-hint export-hint-flush">No special point was found on the branches.</p>;
  return (
    <table className="export-fp-table numeric-sm">
      <thead>
        <tr>
          <th scope="col">Type</th>
          <th scope="col">{param}</th>
          <th scope="col">State</th>
          {map && <th scope="col">Period</th>}
          <th scope="col">ω or θ</th>
          <th scope="col">ℓ₁</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r, k) => (
          <tr key={k}>
            <td>{SPECIAL_KINDS[r.kind].label}</td>
            <td>{withMinus(r.p.toFixed(6))}</td>
            <td>{r.x.length === 1 ? num(r.x[0], 6) : `(${r.x.map((v) => num(v, 6)).join(", ")})`}</td>
            {map && <td>{r.period}</td>}
            <td>{r.omega === null ? "–" : withMinus(r.omega.toFixed(6))}</td>
            <td>{r.lyapunov === null ? "–" : num(r.lyapunov, 6)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SpectrumPreview({ res, kind }: { res: LyapunovResponse; kind: SystemKind }) {
  const gap = res.sum - res.traceAverage;
  return (
    <table className="export-fp-table numeric-sm">
      <thead>
        <tr>
          <th scope="col">Exponent</th>
          <th scope="col">Value</th>
        </tr>
      </thead>
      <tbody>
        {res.exponents.map((v, i) => (
          <tr key={i}>
            <td>{lambdaName(i)}</td>
            <td>{lyapunovValueText(v, res.uncertainty[i])}</td>
          </tr>
        ))}
        <tr>
          <td>Σλ</td>
          <td>{withMinus(fmt(res.sum, 6))}</td>
        </tr>
        <tr>
          <td>{kind === "map" ? "⟨ln |det J|⟩" : "⟨tr J⟩"}</td>
          <td>{withMinus(fmt(res.traceAverage, 6))}</td>
        </tr>
        <tr>
          <td className="body">Difference</td>
          <td>{scientificText(gap)}</td>
        </tr>
      </tbody>
    </table>
  );
}

function spaceAxesPlain(v: SpaceView, states: readonly { latex: string }[]): string {
  const [a, b, c] = spaceNames(v, states).map(latexToPlain);
  return `${a}, ${b} and ${c}`;
}

const trajectories = (n: number) => plural(n, "trajectory", "trajectories");

export function ExportDialog({ state, title, sourceRef, fileStem: stemProp, settings, onSettingsChange, onClose }: Props) {
  const ids = useId();
  const titleId = `${ids}-title`;
  const subId = `${ids}-sub`;
  const codeLabelId = `${ids}-code`;
  const standaloneHint = `${ids}-standalone`;
  const transparentHint = `${ids}-transparent`;
  const numberedHint = `${ids}-numbered`;
  const clipboardHint = `${ids}-clipboard`;
  const reasonId = `${ids}-reason`;
  const dialogRef = useRef<HTMLDivElement>(null);

  const [opts, setOpts] = useState<ExportOptions>(() => readExportOptions(settings));
  const update = (patch: Partial<ExportOptions>) => setOpts((o) => ({ ...o, ...patch }));
  const reported = useRef(true);
  const stored = useRef(settings);
  stored.current = settings;
  useEffect(() => {
    // The options read from the file are not a change.
    if (reported.current) {
      reported.current = false;
      return;
    }
    onSettingsChange?.(mergeExportSettings(stored.current, opts));
  }, [opts, onSettingsChange]);

  const tpl = state.sourceTemplateId ? templateById(state.sourceTemplateId) : undefined;
  const lines = useMemo(() => state.equations.map((l) => l.source), [state.equations]);
  const name = title ?? systemTitle(tpl, lines, state.format).name;
  const ref = sourceRef ?? (tpl ? templateRef(tpl) || undefined : undefined);
  const stem = fileStem(stemProp ?? name);

  const sys = state.compiled && state.compiled.ok ? state.compiled : null;
  const kind = sys?.kind;
  const cobweb = !!sys && isCobweb(state.view);
  const section = !!sys && isSection(state.view);
  const space = !!sys && isSpace(state.view);
  const returnMap = section && isReturnMap(state.view);
  const compiles = !!sys && state.compileOk;
  const timeDependent = !!sys && !sys.autonomous;
  const chaos = sys ? analysisKind(state) : null;
  const chaosWords = chaos === "lyapunov" ? "the Lyapunov view" : chaos === "orbit" ? "the orbit diagram" : "the branch plot";
  const reasons = {
    field: kind === "map" ? "none: a map" : section ? "none: a Poincaré section" : space ? "none: a 3D view" : null,
    nullclines:
      sys && !hasNullclineLayer(kind)
        ? "none: a map"
        : section
          ? "none: a Poincaré section"
          : space
            ? "none: a 3D view"
            : null,
    fixedPoints: timeDependent
      ? "none: the field changes with time"
      : section
        ? "none: a Poincaré section shows only the crossings"
        : null,
  };
  const contentReason: Record<LatexContent, string | null> = {
    figure: !sys
      ? "the equations have not compiled"
      : space
        ? SPACE_LATEX_REFUSAL
        : chaos === "lyapunov"
          ? "LaTeX gives the spectrum as a table; export the running estimates as PNG or SVG"
          : null,
    spectrum: chaos === "lyapunov" ? null : "only the Lyapunov view has a spectrum",
    specialPoints: chaos === "continuation" ? null : "only the branch plot has special points",
    equations: compiles ? null : "the equations don't compile",
    fixedPoints: !compiles
      ? "the equations don't compile"
      : chaos
        ? `${chaosWords} shows no fixed points`
        : timeDependent
          ? "the field changes with time"
          : null,
  };
  const contents: LatexContent[] =
    chaos === "lyapunov"
      ? ["spectrum", "figure", "equations", "fixedPoints"]
      : chaos === "continuation"
        ? ["figure", "specialPoints", "equations", "fixedPoints"]
        : ["figure", "equations", "fixedPoints"];
  const content: LatexContent = [opts.latexContent, ...contents].find((c) => contents.includes(c) && !contentReason[c]) ?? contents[0];
  const format = opts.format;
  // Equations and Fixed points are not figures: no export portrait for them.
  const wantsFigure = format === "svg" || format === "png" || (format === "latex" && content === "figure");

  const size = figurePx(opts);
  const layers = {
    field: opts.field && !reasons.field,
    nullclines: opts.nullclines && !reasons.nullclines,
    fixedPoints: opts.fixedPoints && !reasons.fixedPoints,
  };
  const job = useMemo<FigureJob | null>(
    () => buildFigureJob(state, { widthPx: size.width, heightPx: size.height }, layers),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      state.compiled,
      state.params,
      state.view,
      state.baseState,
      state.fieldTime,
      state.ics,
      state.layers,
      state.solver,
      state.shapeId,
      size.width,
      size.height,
      layers.field,
      layers.nullclines,
      layers.fixedPoints,
    ],
  );
  const jobKey = job ? JSON.stringify(exportPortraitRequest(job)) : "";

  const raster = pngRaster(opts.pngWidthPx, opts.pngHeightPx, opts.pngScale);
  const source: ExportSource = { title: name, ...(ref ? { sourceRef: ref } : {}), lines, format: state.format };
  const lyapunovReq = chaos === "lyapunov" ? buildLyapunovExport(state, source) : null;
  const lyapunovRun = chaos === "lyapunov" && state.lyapunov && lyapunovCurrent(state) ? state.lyapunov.res : null;
  const orbitRatio = orbitFigureRatio(format, raster.scale);
  const orbitFigReq = chaos === "orbit" ? buildOrbitExport(state, source, orbitColumns(size.width, orbitRatio)) : null;
  const orbitCsvReq = chaos === "orbit" ? buildOrbitExport(state, source, state.orbitColumns) : null;
  const orbitKey = orbitFigReq ? JSON.stringify(orbitFigReq.diagram) : "";
  const [orbitFig, setOrbitFig] = useState<{ key: string; res: OrbitDiagramResponse } | null>(null);
  const [orbitError, setOrbitError] = useState<string | null>(null);
  useEffect(() => {
    if (!orbitKey || !wantsFigure) return;
    let live = true;
    const key = orbitKey;
    const timer = window.setTimeout(() => {
      computeOrbitDiagram(JSON.parse(key) as OrbitDiagramRequest)
        .then((res) => {
          if (!live) return;
          setOrbitFig({ key, res });
          setOrbitError(null);
        })
        .catch((err) => live && setOrbitError(errorText(err)));
    }, DEBOUNCE_MS);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [orbitKey, wantsFigure]);
  const continuationReq = chaos === "continuation" ? buildContinuationExport(state, source) : null;
  const branchRuns = chaos === "continuation" ? shownBranches(state).map((b) => b.run) : [];
  const { compiled, params, view, branches: runs } = state;
  const branches = useMemo(
    () =>
      chaos === "continuation"
        ? shownBranches({ compiled, params, view, branches: runs }).flatMap(({ job, run }) =>
            run?.ok ? [{ branch: run.res.branch, color: job.meta.seed }] : [],
          )
        : [],
    [chaos, compiled, params, view, runs],
  );
  const branchAxis = isContinuation(state.view) ? state.view.yAxis : null;
  const analysis: AnalysisFigure | null =
    chaos === "lyapunov"
      ? lyapunovRun && { kind: "lyapunov", res: lyapunovRun }
      : chaos === "orbit" && orbitFig && orbitFig.key === orbitKey
        ? { kind: "orbit", res: orbitFig.res, pixelRatio: orbitRatio }
        : chaos === "continuation" && branchAxis && branches.length > 0
          ? { kind: "branch", branches, dim: sys?.states.length ?? 0, axis: branchAxis }
          : null;
  const continuationLatexReq = (which: ContinuationLatex): ContinuationFigureRequest | null =>
    continuationReq && {
      continuation: continuationReq,
      content: which,
      xRange: state.view.xRange,
      yRange: state.view.yRange,
      widthCm: opts.widthCm,
      heightCm: opts.heightCm,
      theme: opts.palette,
      standalone: opts.standalone,
    };
  const branchLatex: ContinuationLatex | null =
    chaos === "continuation" && (content === "figure" || content === "specialPoints") ? content : null;
  const orbitLatexReq = (): OrbitFigureRequest | null =>
    orbitFigReq && {
      orbit: orbitFigReq,
      xRange: state.view.xRange,
      yRange: state.view.yRange,
      widthCm: opts.widthCm,
      heightCm: opts.heightCm,
      theme: opts.palette,
      standalone: opts.standalone,
    };

  // The figure's portrait, recomputed by the core at the figure's aspect.
  const [fig, setFig] = useState<{ job: FigureJob; res: PortraitResponse } | null>(null);
  const [figError, setFigError] = useState<string | null>(null);
  const [figBusy, setFigBusy] = useState(false);
  useEffect(() => {
    if (!job || !wantsFigure) return;
    let live = true;
    setFigBusy(true);
    const timer = window.setTimeout(() => {
      computeExportPortrait(exportPortraitRequest(job))
        .then((res) => {
          if (!live) return;
          setFig({ job, res });
          setFigError(null);
        })
        .catch((err) => live && setFigError(errorText(err)))
        .finally(() => live && setFigBusy(false));
    }, DEBOUNCE_MS);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jobKey, wantsFigure]);
  const scene = useMemo(() => (fig ? exportScene(fig.job, fig.res, state.ics) : null), [fig, state.ics]);
  const theme = exportTheme(opts.palette);
  const names = useMemo(() => axisNames(state), [state]);
  const figReady = chaos ? !!analysis : !!fig && !!job && fig.job === job;
  const spaceFig = useMemo(
    () => (fig && scene && sys ? exportSpaceFigure(fig.job, scene, sys.states, EXPORT_FONTS) : null),
    [fig, scene, sys],
  );

  // The LaTeX snippet (or the CSV), from the core.
  const figureReq = (): FigureLatexRequest | null =>
    job && {
      portrait: job.portrait,
      tags: job.tags,
      widthCm: opts.widthCm,
      heightCm: opts.heightCm,
      theme: opts.palette,
      title: name,
      ...(ref ? { sourceRef: ref } : {}),
      lines,
      format: state.format,
      standalone: opts.standalone,
    };
  const csvReq = () =>
    buildCsvRequest(state, { icIds: opts.csvIcIds, samples: opts.csvSamples }, { lines, format: state.format, title: name });
  const snippetKey = JSON.stringify([
    format,
    content,
    format === "latex" && content === "figure" ? [jobKey, opts.widthCm, opts.heightCm, opts.palette, opts.standalone] : null,
    format === "latex" && content === "equations"
      ? [sys?.systemId, state.params.map((p) => p.value), opts.notation, opts.numbered]
      : null,
    format === "latex" && content === "fixedPoints" ? [JSON.stringify(screenPortrait(state)), opts.digits] : null,
    format === "csv" && !chaos ? [JSON.stringify(csvReq()), opts.csvIcIds, opts.csvSamples] : null,
    chaos === "lyapunov" ? JSON.stringify(lyapunovReq) : null,
    chaos === "orbit" && format === "csv" ? JSON.stringify(orbitCsvReq) : null,
    chaos === "orbit" && format === "latex" ? JSON.stringify(orbitLatexReq()) : null,
    chaos === "continuation" && format === "csv" ? JSON.stringify(continuationReq) : null,
    format === "latex" && branchLatex ? JSON.stringify(continuationLatexReq(branchLatex)) : null,
    name,
    ref,
    lines,
    state.format,
  ]);
  const [snip, setSnip] = useState<Snip>(null);
  const [csvFiles, setCsvFiles] = useState<{ key: string; files: CsvFileOut[] } | null>(null);
  const [snipError, setSnipError] = useState<string | null>(null);
  useEffect(() => {
    if (format !== "latex" && format !== "csv") return;
    let live = true;
    const key = snippetKey;
    const timer = window.setTimeout(async () => {
      try {
        if (format === "csv" && chaos) {
          const file =
            chaos === "lyapunov"
              ? lyapunovReq && (await exportLyapunovCsv(lyapunovReq))
              : chaos === "orbit"
                ? orbitCsvReq && (await exportOrbitCsv(orbitCsvReq))
                : continuationReq && (await exportContinuationCsv(continuationReq));
          if (!file) return;
          if (live) setCsvFiles({ key, files: [file] });
        } else if (format === "csv") {
          const req = csvReq();
          if (!req) return;
          const res = await exportCsv(req);
          if (live) setCsvFiles({ key, files: res.files });
        } else if (content === "spectrum") {
          if (!lyapunovReq) return;
          const tex = await exportLyapunovLatex(lyapunovReq);
          if (live) setSnip({ key, tex, notes: [] });
        } else if (content === "figure" && chaos === "orbit") {
          const req = orbitLatexReq();
          if (!req) return;
          const res = await exportOrbitLatex(req);
          if (live) setSnip({ key, tex: res.tex, notes: res.notes });
        } else if (branchLatex) {
          const req = continuationLatexReq(branchLatex);
          if (!req) return;
          const res = await exportContinuationLatex(req);
          if (live) setSnip({ key, tex: res.tex, notes: res.notes });
        } else if (content === "figure") {
          const req = figureReq();
          if (!req) return;
          const res = await exportFigureLatex(req);
          if (live) setSnip({ key, tex: res.tex, notes: res.notes });
        } else if (content === "equations" && sys?.systemId !== undefined) {
          const res = await exportSystemLatex({
            systemId: sys.systemId,
            lines,
            format: state.format,
            params: state.params.map((p) => p.value),
            notation: opts.notation,
            numbered: opts.numbered,
            title: name,
            ...(ref ? { sourceRef: ref } : {}),
          });
          if (live) setSnip({ key, tex: res.tex, notes: [], previewLatex: res.previewLatex });
        } else if (content === "fixedPoints") {
          const portrait = screenPortrait(state);
          if (!portrait) return;
          const tex = await exportFixedPointsLatex({
            portrait,
            title: name,
            ...(ref ? { sourceRef: ref } : {}),
            lines,
            format: state.format,
            digits: opts.digits,
          });
          if (live) setSnip({ key, tex, notes: [] });
        }
        if (live) setSnipError(null);
      } catch (err) {
        if (live) setSnipError(errorText(err));
      }
    }, DEBOUNCE_MS);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snippetKey]);
  const snipReady = !!snip && snip.key === snippetKey;
  const csvReady = !!csvFiles && csvFiles.key === snippetKey;

  // Copy feedback and the polite announcement.
  const [copied, setCopied] = useState<string | null>(null);
  const [announce, setAnnounce] = useState("");
  const [notice, setNotice] = useState<{ kind: "error" | "info"; text: string } | null>(null);
  const [working, setWorking] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const t = window.setTimeout(() => setCopied(null), COPIED_MS);
    return () => window.clearTimeout(t);
  }, [copied]);
  useEffect(() => setNotice(null), [format, content]);

  // Focus: the format control on open, back to the opener on close; Escape and Tab stay here.
  useLayoutEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialogRef.current?.querySelector<HTMLElement>('.export-format [role="radio"][aria-checked="true"]')?.focus();
    return () => {
      if (opener && opener.isConnected) opener.focus();
    };
  }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const root = dialogRef.current;
      if (!root) return;
      if (e.key === "Escape" && !e.defaultPrevented) {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key !== "Tab") return;
      const items = tabbables(root);
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !root.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !root.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const xRange = chaos ? state.view.xRange : job?.portrait.xRange ?? null;
  const yRange = chaos ? state.view.yRange : job?.portrait.yRange ?? null;
  const transparent = (format === "svg" || format === "png") && opts.transparent;

  const drawTo = (ctx: CanvasRenderingContext2D) => {
    if (analysis && xRange && yRange) {
      const vp = makeViewport(size.width, size.height, xRange, yRange);
      drawAnalysisFigure(ctx, size.width, size.height, vp, analysis, theme, {
        background: !opts.transparent,
        names,
        fonts: EXPORT_FONTS,
      });
      return;
    }
    if (!fig) return;
    const vp = makeViewport(fig.job.widthPx, fig.job.heightPx, fig.job.portrait.xRange, fig.job.portrait.yRange);
    drawFigure(ctx, fig.job.widthPx, fig.job.heightPx, vp, scene, theme, {
      background: !opts.transparent,
      names,
      fonts: EXPORT_FONTS,
    });
  };
  const svgText = (): string | null => {
    const probe = document.createElement("canvas").getContext("2d");
    if (!probe) return null;
    if (analysis && xRange && yRange)
      return figureSvg(
        {
          widthPx: size.width,
          heightPx: size.height,
          widthCm: opts.widthCm,
          heightCm: opts.heightCm,
          xRange,
          yRange,
          scene: null,
          theme,
          names,
          transparent: opts.transparent,
          fonts: EXPORT_FONTS,
          title: name,
          desc: figureDesc,
          analysis,
        },
        canvasMeasure(probe),
      );
    if (!fig) return null;
    return figureSvg(
      {
        widthPx: fig.job.widthPx,
        heightPx: fig.job.heightPx,
        widthCm: opts.widthCm,
        heightCm: opts.heightCm,
        xRange: fig.job.portrait.xRange,
        yRange: fig.job.portrait.yRange,
        scene,
        theme,
        names,
        transparent: opts.transparent,
        fonts: EXPORT_FONTS,
        title: name,
        desc: figureDesc,
        space: spaceFig,
      },
      canvasMeasure(probe),
    );
  };
  const pngDraw = async () =>
    fig && spaceFig
      ? spacePngDraw(spaceFig, fig.job.widthPx, fig.job.heightPx, raster.scale, theme, !opts.transparent)
      : drawTo;

  const run = async (what: () => Promise<void>) => {
    if (working) return;
    setWorking(true);
    try {
      await what();
    } catch (err) {
      const text = errorText(err);
      setNotice({ kind: "error", text });
      setAnnounce("");
      window.setTimeout(() => setAnnounce(text), 30);
    } finally {
      setWorking(false);
    }
  };
  const copyDone = (key: string, what: string) => {
    setCopied(key);
    setNotice(null);
    setAnnounce("");
    window.setTimeout(() => setAnnounce(`${what} copied`), 30);
  };
  const saveDone = (names: string[] | undefined) => {
    if (!names) return;
    const [first, ...rest] = names;
    const text = rest.length ? `Saved ${first} and ${plural(rest.length, "more file")}.` : `Saved ${first}.`;
    setNotice({ kind: "info", text });
    setAnnounce(text);
  };

  const csvSelectable = state.ics.filter((ic) => !ic.hidden && ic.x0.length === (sys?.states.length ?? -1));
  const csvSelected = (id: string) => opts.csvIcIds === null || opts.csvIcIds.includes(id);
  const csvCount = csvSelectable.filter((ic) => csvSelected(icProjectId(ic))).length;

  /** `act` gets the copy key of the button that was used, for its "Copied". */
  type Action = { label: string; icon?: ReactNode; ready: boolean; act: (key?: string) => void; copyKey?: string };
  let primary: Action | null = null;
  let secondary: Action | null = null;
  let footNote: ReactNode = null;
  const chaosCsv = chaos
    ? `${stem}-${chaos === "lyapunov" ? "lyapunov" : chaos === "orbit" ? "orbit-diagram" : "continuation"}.csv`
    : null;
  if (format === "latex") {
    const file =
      content === "figure"
        ? `${stem}.tex`
        : content === "spectrum"
          ? `${stem}-lyapunov.tex`
          : content === "specialPoints"
            ? `${stem}-special-points.tex`
          : content === "equations"
            ? `${stem}-equations.tex`
            : `${stem}-fixed-points.tex`;
    const withFiles = content === "figure";
    primary = {
      label: "Copy LaTeX",
      copyKey: "latex",
      ready: snipReady,
      act: (key = "latex") =>
        run(async () => {
          if (!snip) return;
          await clipboardWriteText(snip.tex);
          copyDone(key, "LaTeX");
        }),
    };
    secondary = {
      label: "Save…",
      ready: branchLatex ? !!continuationReq : content === "figure" ? (chaos === "orbit" ? !!orbitFigReq : !!job) : snipReady,
      act: () =>
        run(async () => {
          if (branchLatex) {
            const req = continuationLatexReq(branchLatex);
            if (req) saveDone((await saveContinuationLatex(req, file))?.names);
          } else if (content === "figure" && chaos === "orbit") {
            const req = orbitLatexReq();
            if (req) saveDone((await saveOrbitLatex(req, file))?.names);
          } else if (content === "figure") {
            const req = figureReq();
            if (req) saveDone((await saveFigureLatex(req, file))?.names);
          } else if (snip) saveDone((await saveTextFile(file, "tex", snip.tex))?.names);
        }),
    };
    footNote =
      withFiles && chaos === "orbit" ? (
        <>
          Save… writes <span className="ds-num">{file}</span> and its data file, a PNG above 5,000 points
        </>
      ) : withFiles ? (
        <>
          Save… writes <span className="ds-num">{file}</span> and its data files
        </>
      ) : (
        <>
          Save… writes <span className="ds-num">{file}</span>
        </>
      );
  } else if (format === "svg") {
    primary = {
      label: "Save…",
      ready: figReady,
      act: () =>
        run(async () => {
          const svg = svgText();
          if (svg) saveDone((await saveTextFile(`${stem}.svg`, "svg", svg))?.names);
        }),
    };
    secondary = {
      label: "Copy SVG",
      copyKey: "svg",
      ready: figReady,
      act: () =>
        run(async () => {
          const svg = svgText();
          if (!svg) return;
          await clipboardWriteText(svg);
          copyDone("svg", "SVG");
        }),
    };
    footNote = (
      <>
        Save… writes <span className="ds-num">{stem}.svg</span>
      </>
    );
  } else if (format === "png") {
    primary = {
      label: "Save…",
      ready: figReady,
      act: () =>
        run(async () => {
          const png = await renderPng(raster, await pngDraw());
          saveDone((await savePngFile(png, `${stem}.png`))?.names);
        }),
    };
    secondary = {
      label: "Copy image",
      copyKey: "png",
      ready: figReady,
      act: () =>
        run(async () => {
          await clipboardWritePng(await renderPng(raster, await pngDraw(), false));
          copyDone("png", "Image");
        }),
    };
    footNote = (
      <>
        Save… writes <span className="ds-num">{stem}.png</span>
      </>
    );
  } else if (chaosCsv) {
    const file = chaosCsv;
    const ready = csvReady && !!csvFiles?.files.length;
    primary = {
      label: "Save…",
      ready,
      act: () =>
        run(async () => {
          const f = csvFiles?.files[0];
          if (f) saveDone((await saveCsvFile(f, name, file))?.names);
        }),
    };
    secondary = {
      label: "Copy CSV",
      copyKey: "csv",
      ready,
      act: () =>
        run(async () => {
          const f = csvFiles?.files[0];
          if (!f) return;
          await clipboardWriteText(f.csv);
          copyDone("csv", "CSV");
        }),
    };
    footNote =
      chaos === "continuation" ? (
        <>
          Save… writes <span className="ds-num">{file}</span>, its .meta.json and{" "}
          <span className="ds-num">{file.replace(/\.csv$/, "-special.csv")}</span>
        </>
      ) : (
        <>
          Save… writes <span className="ds-num">{file}</span> and its .meta.json
        </>
      );
  } else {
    const any = csvCount > 0;
    primary = {
      label: "Save…",
      ready: csvReady && any,
      act: () =>
        run(async () => {
          const req = csvReq();
          if (req) saveDone((await saveCsv(req, `${stem}.csv`))?.names);
        }),
    };
    secondary = {
      label: "Copy CSV",
      copyKey: "csv",
      ready: csvReady && any && !!csvFiles?.files.length,
      act: () =>
        run(async () => {
          const f = csvFiles?.files[0];
          if (!f) return;
          await clipboardWriteText(f.csv);
          copyDone("csv", "CSV");
        }),
    };
    footNote = (
      <>
        {csvCount > 1 ? (
          <>
            Save… writes <span className="ds-num">{stem}-trajectory-1.csv</span>, … and a .meta.json for each
          </>
        ) : (
          <>
            Save… writes <span className="ds-num">{stem}.csv</span> and its .meta.json
          </>
        )}
      </>
    );
  }

  const trajectoriesShown = scene ? new Set(scene.trajectories.map((t) => t.meta.icId)).size : 0;
  const crossingsShown = scene
    ? scene.trajectories.reduce((n, t) => n + Math.max(0, t.out.t.length - (returnMap ? 1 : 0)), 0)
    : 0;
  const branchSpecial = chaos === "continuation" ? specialRows(branchRuns) : [];
  const runWords = chaos
    ? chaos === "lyapunov"
      ? plural(sys?.states.length ?? 0, "running estimate")
      : chaos === "orbit"
        ? plural(orbitFigReq?.diagram.columns ?? 0, "parameter value")
        : plural(branches.length, "branch", "branches")
    : cobweb
    ? plural(trajectoriesShown, "orbit")
    : returnMap
      ? plural(crossingsShown, "pair of crossings", "pairs of crossings")
      : section
        ? plural(crossingsShown, "crossing")
        : trajectories(trajectoriesShown);
  const layerWords = chaos
    ? chaos === "continuation"
      ? [runWords, plural(branchSpecial.length, "special point")]
      : [runWords]
    : [
    layers.field && "direction field",
    layers.nullclines && "nullclines",
    cobweb && "the graph of f",
    (cobweb || returnMap) && "the diagonal",
    runWords,
    layers.fixedPoints && plural(scene?.fixedPoints.length ?? 0, "fixed point"),
  ].filter(Boolean) as string[];
  // What the figure shows: the SVG's <desc>, and the preview's label after "Preview: ".
  const figureDesc = `${name} with ${layerWords.slice(0, -1).join(", ")}${layerWords.length > 1 ? " and " : ""}${layerWords[layerWords.length - 1]}`;
  const previewLabel = `Preview: ${figureDesc}`;
  const axisPlain = (i: 0 | 1) => {
    if (!sys) return "";
    const a = i === 0 ? state.view.xAxis : state.view.yAxis;
    return a.kind === "norm" ? "‖x‖" : latexToPlain(axisLatex(a, sys.states, kind));
  };
  const timeView = state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time";
  const viewWords = sys
    ? chaos === "lyapunov"
      ? "Lyapunov spectrum"
      : chaos === "orbit"
        ? `orbit diagram of ${names ? latexToPlain(names.y) : "x"} against ${names ? latexToPlain(names.x) : "r"}`
        : chaos === "continuation"
          ? `branch plot of ${names ? latexToPlain(names.y) : "x"} against ${names ? latexToPlain(names.x) : "r"}`
          : cobweb
      ? `cobweb of ${axisPlain(0)}`
      : space
        ? `3D view of ${spaceAxesPlain(state.view as SpaceView, sys.states)}`
        : returnMap
          ? `return map of ${axisPlain(0)}`
          : section
            ? `Poincaré section ${axisPlain(0)}–${axisPlain(1)}`
            : `${timeView ? "time series" : kind === "map" ? "orbit" : "phase plane"} ${axisPlain(0)}–${axisPlain(1)}`
    : "no plot yet";
  const sizeCaption =
    format === "png" ? (
      <span>
        <span className="ds-num">
          {raster.width} × {raster.height}
        </span>{" "}
        px, {Math.round(96 * raster.scale)} dpi
      </span>
    ) : (
      <span>
        <span className="ds-num">
          {withMinus(fmt(opts.widthCm, 3))} × {withMinus(fmt(opts.heightCm, 3))}
        </span>{" "}
        cm, {runWords}
      </span>
    );

  const actionButton = (a: Action, variant: "primary" | "secondary" | "ghost", small = false, describedBy?: string) => {
    const isCopied = a.copyKey !== undefined && copied === a.copyKey;
    const icon = a.copyKey ? isCopied ? <IconCheck /> : <IconCopy /> : undefined;
    return (
      <Button
        variant={variant}
        size={small ? "sm" : "md"}
        icon={icon}
        disabled={!a.ready || working || !sys}
        onClick={() => a.act(a.copyKey)}
        data-export-action={a.copyKey ?? "save"}
        aria-describedby={describedBy}
      >
        {a.copyKey ? (
          // The other label, hidden, keeps the button's width: the buttons
          // beside it don't move under the pointer for "Copied".
          <span className="export-swap" data-alt={isCopied ? a.label : "Copied"}>
            {isCopied ? "Copied" : a.label}
          </span>
        ) : (
          a.label
        )}
      </Button>
    );
  };

  const sizeFields =
    format === "png" ? (
      <fieldset className="ds-export-group">
        <legend className="ds-export-label">Size</legend>
        <div className="ds-export-size">
          <label className="ds-export-field">
            <span className="ds-export-label">Width</span>
            <NumberField
              value={opts.pngWidthPx}
              ariaLabel="Width in px"
              unit="px"
              digits={6}
              live
              validate={(v) => pngSizeError(v, "width")}
              onCommit={(v) => update({ pngWidthPx: v })}
            />
          </label>
          <label className="ds-export-field">
            <span className="ds-export-label">Height</span>
            <NumberField
              value={opts.pngHeightPx}
              ariaLabel="Height in px"
              unit="px"
              digits={6}
              live
              validate={(v) => pngSizeError(v, "height")}
              onCommit={(v) => update({ pngHeightPx: v })}
            />
          </label>
        </div>
        <div className="ds-export-field">
          <span className="ds-export-label" id={`${ids}-scale`}>
            Scale
          </span>
          <SegmentedControl
            size="sm"
            fill
            labelledBy={`${ids}-scale`}
            value={`${opts.pngScale}` as `${PngScale}`}
            options={SCALES}
            onChange={(v) => update({ pngScale: Number(v) as PngScale })}
          />
        </div>
        {raster.reduced && (
          <p className="ds-export-hint export-hint-flush">
            At {opts.pngScale}× this image would be over 16.7 megapixels, so it is drawn at{" "}
            {raster.scale.toFixed(2)}×.
          </p>
        )}
      </fieldset>
    ) : (
      <fieldset className="ds-export-group">
        <legend className="ds-export-label">Size</legend>
        <div className="ds-export-size">
          <label className="ds-export-field">
            <span className="ds-export-label">Width</span>
            <NumberField
              value={opts.widthCm}
              ariaLabel="Width in cm"
              unit="cm"
              digits={3}
              live
              validate={(v) => sizeError(v, "width")}
              onCommit={(v) => update({ widthCm: v })}
            />
          </label>
          <label className="ds-export-field">
            <span className="ds-export-label">Height</span>
            <NumberField
              value={opts.heightCm}
              ariaLabel="Height in cm"
              unit="cm"
              digits={3}
              live
              validate={(v) => sizeError(v, "height")}
              onCommit={(v) => update({ heightCm: v })}
            />
          </label>
        </div>
      </fieldset>
    );

  const includeGroup = (
    <fieldset className="ds-export-group">
      <legend className="ds-export-label">Include</legend>
      {!cobweb && !section && !space && !chaos && (
        <>
          <Check
            checked={opts.field}
            reason={reasons.field}
            label="Include direction field"
            onChange={(v) => update({ field: v })}
          />
          <Check
            checked={opts.nullclines}
            reason={reasons.nullclines}
            label="Include nullclines"
            onChange={(v) => update({ nullclines: v })}
          />
        </>
      )}
      {!chaos && (
        <Check
          checked={opts.fixedPoints}
          reason={reasons.fixedPoints}
          label="Include fixed points"
          onChange={(v) => update({ fixedPoints: v })}
        />
      )}
      {format === "latex" && (
        <>
          <Check
            checked={opts.standalone}
            label="Standalone document"
            describedBy={standaloneHint}
            onChange={(v) => update({ standalone: v })}
          />
          <p className="ds-export-hint export-hint-tight" id={standaloneHint}>
            Wraps the figure in <code>standalone</code> so it compiles on its own.
          </p>
        </>
      )}
      {(format === "svg" || format === "png") && (
        <>
          <Check
            checked={opts.transparent}
            label="Transparent background"
            describedBy={transparentHint}
            onChange={(v) => update({ transparent: v })}
          />
          <p className="ds-export-hint export-hint-tight" id={transparentHint}>
            Leaves the plot background out, for slides with their own.
          </p>
        </>
      )}
    </fieldset>
  );

  const paletteGroup = (
    <fieldset className="ds-export-group">
      <legend className="ds-export-label">Colors</legend>
      <SegmentedControl
        size="sm"
        fill
        ariaLabel="Colors"
        value={opts.palette}
        options={PALETTES}
        onChange={(v) => update({ palette: v })}
      />
    </fieldset>
  );

  const preview = (
    <ExportPreview
      widthPx={size.width}
      heightPx={size.height}
      xRange={xRange}
      yRange={yRange}
      scene={scene}
      theme={theme}
      names={names}
      fonts={EXPORT_FONTS}
      transparent={transparent}
      label={previewLabel}
      busy={chaos ? !analysis : figBusy}
      space={spaceFig}
      analysis={analysis}
      caption={
        <>
          <span>Preview</span>
          {sizeCaption}
        </>
      }
    />
  );

  let split: ReactNode;
  if (format === "latex" && content === "equations") {
    split = (
      <div className="ds-export-split">
        <figure className="ds-export-preview">
          <div className="export-equations" aria-busy={!snipReady || undefined}>
            {snip?.previewLatex ? <Tex latex={snip.previewLatex} display /> : null}
          </div>
          <figcaption className="ds-export-preview-cap">
            <span>Preview</span>
            <span>{plural(sys?.states.length ?? 0, "equation")}</span>
          </figcaption>
        </figure>
        <div className="ds-export-options">
          <fieldset className="ds-export-group">
            <legend className="ds-export-label">Notation</legend>
            <SegmentedControl
              size="sm"
              fill
              ariaLabel="Notation"
              value={opts.notation}
              options={NOTATIONS}
              onChange={(v) => update({ notation: v })}
            />
          </fieldset>
          <fieldset className="ds-export-group">
            <legend className="ds-export-label">Layout</legend>
            <Check
              checked={opts.numbered}
              label="Number the equations"
              describedBy={numberedHint}
              onChange={(v) => update({ numbered: v })}
            />
            <p className="ds-export-hint" id={numberedHint}>
              <code>align</code> with a label on each line; off, <code>align*</code>.
            </p>
          </fieldset>
        </div>
      </div>
    );
  } else if (format === "latex" && content === "spectrum") {
    split = (
      <div className="ds-export-split">
        <figure className="ds-export-preview">
          <div className="export-fp-preview">
            {lyapunovRun && sys?.kind ? (
              <SpectrumPreview res={lyapunovRun} kind={sys.kind} />
            ) : (
              <p className="ds-export-hint export-hint-flush">The Lyapunov run has not finished yet.</p>
            )}
          </div>
          <figcaption className="ds-export-preview-cap">
            <span>Preview</span>
            <span>{plural(sys?.states.length ?? 0, "exponent")}</span>
          </figcaption>
        </figure>
        <div className="ds-export-options">
          <p className="ds-export-hint export-hint-flush">
            Each exponent with its standard error over the blocks, as <code>\num{"{0.906 +- 0.004}"}</code> for siunitx;
            the caption says how the run was made.
          </p>
        </div>
      </div>
    );
  } else if (format === "latex" && content === "specialPoints") {
    split = (
      <div className="ds-export-split">
        <figure className="ds-export-preview">
          <div className="export-fp-preview">
            <SpecialPointsPreview rows={branchSpecial.map((r) => r.at)} param={names ? latexToPlain(names.x) : "r"} map={kind === "map"} />
          </div>
          <figcaption className="ds-export-preview-cap">
            <span>Preview</span>
            <span>{plural(branchSpecial.length, "special point")}</span>
          </figcaption>
        </figure>
        <div className="ds-export-options">
          <p className="ds-export-hint export-hint-flush">
            Each special point once, in increasing {names ? latexToPlain(names.x) : "r"}, as <code>\num{"{…}"}</code> for
            siunitx; the caption names the types.
          </p>
        </div>
      </div>
    );
  } else if (format === "latex" && content === "fixedPoints") {
    const fps = state.portrait?.res.fixedPoints ?? [];
    split = (
      <div className="ds-export-split">
        <figure className="ds-export-preview">
          <div className="export-fp-preview">
            {sys && <FixedPointsPreview fps={fps} kind={sys.kind ?? "ode"} dim={sys.states.length} digits={opts.digits} />}
          </div>
          <figcaption className="ds-export-preview-cap">
            <span>Preview</span>
            <span>{plural(fps.length, "fixed point")}</span>
          </figcaption>
        </figure>
        <div className="ds-export-options">
          <fieldset className="ds-export-group">
            <legend className="ds-export-label">Digits</legend>
            <NumberField
              value={opts.digits}
              ariaLabel="Significant digits"
              digits={1}
              step={1}
              steppers
              validate={(v) => (Number.isInteger(v) && v >= 2 && v <= 8 ? null : "Use 2 to 8 digits.")}
              onCommit={(v) => update({ digits: v })}
            />
          </fieldset>
        </div>
      </div>
    );
  } else if (format === "csv" && chaos) {
    const columns =
      chaos === "lyapunov"
        ? `${kind === "map" ? "n" : "t"}, ${(sys?.states ?? []).map((_, i) => `lambda_${i + 1}`).join(", ")}`
        : chaos === "orbit"
          ? `${names ? latexToPlain(names.x) : "r"}, n, ${names ? latexToPlain(names.y) : "x"}`
          : ["branch", "period", "point", names ? latexToPlain(names.x) : "r", ...(sys?.states ?? []).map((s) => s.name), "stable", "unstable"].join(", ");
    split = (
      <div className="ds-export-options export-csv-options">
        <p className="ds-export-hint export-hint-flush">
          Columns {columns};{" "}
          {chaos === "lyapunov"
            ? "one row per sampled renormalization, the last row the final estimate."
            : chaos === "orbit"
              ? `one row per kept iterate at each of the ${orbitCsvReq?.diagram.columns ?? 0} parameter values on screen.`
              : "one row per point of each branch (a k-cycle's point is k rows); the special points go to a second file."}
        </p>
      </div>
    );
  } else if (format === "csv") {
    split = (
      <div className="ds-export-options export-csv-options">
        <fieldset className="ds-export-group">
          <legend className="ds-export-label">Trajectories</legend>
          {csvSelectable.length === 0 && <p className="ds-export-hint export-hint-flush">There are no trajectories to write.</p>}
          {csvSelectable.map((ic, k) => {
            const id = icProjectId(ic);
            return (
              <Check
                key={ic.id}
                checked={csvSelected(id)}
                label={`Trajectory ${k + 1}, from (${ic.x0.map((v) => withMinus(fmt(v, 3))).join(", ")})`}
                onChange={(on) => {
                  const all = csvSelectable.map(icProjectId);
                  const cur = opts.csvIcIds ?? all;
                  const next = on ? all.filter((x) => x === id || cur.includes(x)) : cur.filter((x) => x !== id);
                  update({ csvIcIds: next.length === all.length ? null : next });
                }}
              />
            );
          })}
          <p className="ds-export-hint export-hint-flush">
            Columns {sys ? [kind === "map" ? "n" : "t", ...sys.states.map((s) => s.name)].join(", ") : "t"}; one file
            per trajectory, {section ? "one row per crossing after the skipped ones." : "backward run first."}
          </p>
        </fieldset>
        {!section && (
          <fieldset className="ds-export-group export-csv-samples">
            <legend className="ds-export-label">Samples per run</legend>
            <NumberField
              value={opts.csvSamples}
              ariaLabel="Samples per run"
              format={(v) => (v === 0 ? "auto" : String(Math.round(v)))}
              validate={(v) =>
                v === 0 || (Number.isInteger(v) && v >= 2 && v <= CSV_MAX_SAMPLES)
                  ? null
                  : `Use a whole number from 2 to ${CSV_MAX_SAMPLES.toLocaleString("en-US")}, or auto.`
              }
              hint="auto: as many as the plot uses"
              onCommit={(v) => update({ csvSamples: v })}
            />
          </fieldset>
        )}
      </div>
    );
  } else {
    split = (
      <div className="ds-export-split">
        {preview}
        <div className="ds-export-options">
          {sizeFields}
          {includeGroup}
          {paletteGroup}
          {format === "png" && (
            <p className="ds-export-hint export-hint-flush" id={clipboardHint}>
              Copy image pastes at screen size; Save… keeps the physical size.
            </p>
          )}
        </div>
      </div>
    );
  }

  const showCode = format === "latex" || format === "csv";
  const contentHint =
    [
      space && contentReason.figure,
      chaos === "lyapunov" && `Figure: ${contentReason.figure}.`,
      chaos && compiles && `Fixed points: ${contentReason.fixedPoints}.`,
      contentReason.equations
        ? "Equations and Fixed points need equations that compile."
        : !chaos && contentReason.fixedPoints && "Fixed points: none, because the field changes with time.",
    ]
      .filter(Boolean)
      .join(" ") || null;
  const figureContent = format === "svg" || format === "png" || (format === "latex" && content === "figure");
  const csvWords =
    chaos === "lyapunov"
      ? "running estimates"
      : chaos === "orbit"
        ? "orbit diagram"
        : chaos === "continuation"
          ? "branches"
          : section
            ? "crossings"
            : "trajectories";
  const codeText =
    format === "csv"
      ? csvFiles && csvReady
        ? csvFiles.files.length
          ? csvFiles.files[0].csv.split("\n").slice(0, 13).join("\n")
          : ""
        : ""
      : snipReady && snip
        ? snip.tex
        : snip?.tex ?? "";
  const codeLabel =
    format === "csv" ? (
      csvFiles?.files.length ? (
        <>
          First rows of <code>{chaosCsv ?? `${csvFiles.files[0].stem}.csv`}</code>
          {csvFiles.files.length > 1 ? ` (1 of ${csvFiles.files.length} files)` : ""}
        </>
      ) : (
        "CSV"
      )
    ) : content === "spectrum" ? (
      <>
        LaTeX, a <code>table</code> ready to paste
      </>
    ) : content === "figure" ? (
      opts.standalone ? (
        "LaTeX, a standalone document"
      ) : (
        <>
          LaTeX, ready to paste inside a <code>figure</code> environment
        </>
      )
    ) : content === "equations" ? (
      "LaTeX, ready to paste into your text"
    ) : (
      <>
        LaTeX, a <code>table</code> ready to paste
      </>
    );

  return (
    <div className="ds-export-scrim export-scrim" onPointerDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        className="ds-export export-dialog"
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={subId}
      >
        <header className="ds-export-head">
          <div>
            <h2 className="ds-export-title" id={titleId}>
              {figureContent ? "Export figure" : "Export"}
            </h2>
            <p className="ds-export-sub" id={subId}>
              {format === "latex" && content === "equations"
                ? `${name}, the equations as typed`
                : format === "latex" && content === "fixedPoints"
                  ? `${name}, the fixed points in view`
                  : format === "latex" && content === "spectrum"
                    ? `${name}, the Lyapunov spectrum`
                    : format === "latex" && content === "specialPoints"
                    ? `${name}, the special points of the branches`
                    : format === "csv"
                    ? `${name}, the ${csvWords} as you see ${chaos === "orbit" ? "it" : "them"} now`
                    : `${name}, ${viewWords}, as you see it now`}
            </p>
          </div>
          <IconButton label="Close" tip={false} onClick={onClose}>
            <IconClose />
          </IconButton>
        </header>

        <div className="ds-export-body">
          <div className="ds-export-format export-format">
            <span className="ds-export-label" id={`${ids}-format`}>
              Format
            </span>
            <SegmentedControl fill ariaLabel="Format" value={format} options={FORMATS} onChange={(v) => update({ format: v })} />
          </div>
          {format === "latex" && (
            <div className="ds-export-format">
              <span className="ds-export-label">Content</span>
              <SegmentedControl
                size="sm"
                fill
                ariaLabel="Content"
                value={content}
                describedBy={contentHint ? reasonId : undefined}
                options={contents.map((c) => ({
                  value: c,
                  label: CONTENT_LABELS[c],
                  disabled: !!contentReason[c],
                  title: contentReason[c] ?? undefined,
                }))}
                onChange={(v) => update({ latexContent: v })}
              />
              {contentHint && (
                <p className="ds-export-hint export-hint-flush" id={reasonId}>
                  {contentHint}
                </p>
              )}
            </div>
          )}

          {!sys && (
            <Notice kind="error">
              Nothing to export yet: the equations haven't compiled. Fix the errors in the Equations panel first.
            </Notice>
          )}

          {split}

          {wantsFigure && !chaos && figError && <Notice kind="error">{figError}</Notice>}
          {wantsFigure && chaos === "orbit" && orbitError && <Notice kind="error">{orbitError}</Notice>}
          {wantsFigure && chaos === "continuation" && !analysis && (
            <Notice kind="info">
              There are no branches to export yet. Continue from a fixed point in the Continuation panel first.
            </Notice>
          )}
          {snipError && showCode && <Notice kind="error">{snipError}</Notice>}
          {notice && <Notice kind={notice.kind}>{notice.text}</Notice>}

          {showCode && (
            <div className="ds-export-code">
              <div className="ds-export-code-head">
                <span className="ds-export-label" id={codeLabelId}>
                  {codeLabel}
                </span>
                {secondary &&
                  format === "latex" &&
                  primary &&
                  actionButton({ ...primary, label: "Copy", copyKey: "latex-code" }, "ghost", true, codeLabelId)}
              </div>
              <Snippet text={codeText} labelledBy={codeLabelId} />
              {format === "latex" && snipReady && snip && snip.notes.length > 0 && (
                <Notice kind="info">{snip.notes.join(" ")}</Notice>
              )}
            </div>
          )}
        </div>

        <footer className="ds-export-foot">
          <p className="ds-export-foot-note">{footNote}</p>
          <div className="ds-export-actions">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            {secondary && actionButton(secondary, "secondary")}
            {primary && actionButton(primary, "primary")}
          </div>
        </footer>
        <span className="ds-sr" aria-live="polite" aria-atomic="true">
          {announce}
        </span>
      </div>
    </div>
  );
}
