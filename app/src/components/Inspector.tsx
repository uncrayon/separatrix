import { Fragment, useEffect, useId, useRef, useState, type ReactNode } from "react";
import type {
  AxisRef,
  ContinuationDirection,
  Direction,
  FixedPointOut,
  Method,
  SectionSpec,
  SpecialPointOut,
  SystemKind,
  TrajectoryOut,
} from "../api/types";
import { MAX_SECTION_POINTS } from "../api/ipc.generated";
import { Tex } from "../math/Tex";
import { MathProse } from "../math/MathProse";
import {
  classificationExplain,
  classificationTitle,
  COMPUTED_ZERO,
  fmt,
  latexToPlain,
  mapMultiplier,
  plainSubscripted,
  mapStabilityHint,
  refusalText,
  sameAxis,
  viewAxisLabels,
  withSubscript,
  texMinus,
} from "../math/format";
import { METHODS, STIFF_HINT, STIFF_METHOD, energyDriftText, methodFits, symplecticRefusal } from "../state/methods";
import type { StudioActions, StudioState } from "../state/useStudio";
import {
  COBWEB_FILL_ITERATES,
  DEFAULT_FIXED_STEP,
  MAX_CONTINUATION_POINTS,
  SPECIAL_KINDS,
  continuationAvailability,
  continuationSettingsError,
  fitBranchRanges,
  isContinuation,
  isCurves,
  isParameterView,
  pointSeed,
  shownBranches,
  specialRows,
  switchSeed,
  switchedBranch,
  type ContinuationSeed,
  MAX_ITERATES,
  PALETTE_SIZE,
  VIEW_CHOICE_LABELS,
  VIEW_CHOICE_SHORT,
  countText,
  doublingRows,
  endText,
  findsDoublings,
  isCobweb,
  isLyapunov,
  isOrbit,
  orbitFits,
  orbitSettingsError,
  stateAxis,
  type OrbitSettings,
  isReturnMap,
  isSection,
  isSpace,
  lyapunovFits,
  lyapunovSettingsError,
  lyapunovSolver,
  lyapunovStart,
  lyapunovValueText,
  renormalizations,
  scientificText,
  type LyapunovSettings,
  spaceStates,
  planeSectionAvailability,
  viewChoice,
  viewChoiceAvailability,
  viewChoicesFor,
  portraitFits,
  searchedFixedPoints,
  selectFixedPoint,
  stateExtent,
  selectedFixedPointIndex,
  trajectorySpans,
  type InitialCondition,
} from "../state/model";
import {
  Button,
  IconButton,
  NumberField,
  SegmentedControl,
  Select,
  Switch,
  withMinus,
  type SegmentOption,
  type SelectOption,
} from "./controls";
import { systemNameOf } from "./systemName";
import { PanelSection, useSections } from "./PanelSection";
import { focusAfterTrajectoryDelete } from "./focus";
import { dec, startPointParts } from "./startPoint";
import { landedOnSource, looksStiff, STATUS_WORDS, stopText } from "./runStatus";
import { fixedPointGlyph } from "../canvas/draw";
import { COBWEB_DEDUPE_ITERATES, distinctSegments, orbitValues } from "../canvas/cobweb";
import { FixedPointIcon } from "../canvas/PortraitCanvas";
import { CurveSample } from "../canvas/CurvesCanvas";
import { FollowHomoclinic, HomoclinicRowItems } from "./HomoclinicRows";
import {
  CODIM2_KINDS,
  CURVE_KINDS,
  buildCurveJobs,
  curveKindOf,
  curvesAvailability,
  curvesSettingsError,
  drawnCurves,
  locatedText,
  mergedText,
  neutralText,
  nextParam,
  onCurvesText,
  planePointRows,
  retryStep,
  stallText,
  type CurvesView,
} from "../state/curves";
import {
  hasComplexEigenvalue,
  numberedEigenvalues,
  othersHaveRealEigenvectors,
  realEigenvectors,
  setShowEigenvectors,
  subscriptDigits,
  useShowEigenvectors,
} from "../canvas/eigen";
import {
  IconClose,
  IconEye,
  IconEyeOff,
  IconField,
  IconFixedPoint,
  IconFollow,
  IconSearch,
  IconNullcline,
  IconTrajectoryAdd,
  IconTrash,
  IconWarning,
} from "./icons";

interface Props {
  state: StudioState;
  actions: StudioActions;
}

const cx = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(" ");

const short = (v: number, digits = 4) => withMinus(fmt(v, digits, COMPUTED_ZERO));

const atTime = (v: number) => (Math.abs(v - Math.round(v)) < 1e-9 ? withMinus(String(Math.round(v))) : dec(v, 2));


const plainName = (latex: string) => latexToPlain(latex);

const DOTS: Record<string, string> = { dot: "dot", ddot: "double dot", dddot: "triple dot" };

/** `\dot{x}_{1}` as "x_1 dot": the dotted base, then any subscript after it. */
function optionName(latex: string): string {
  const m = /^\\(d{0,2}dot)\{([^{}]*)\}(.*)$/.exec(latex);
  if (m) return `${latexToPlain(m[2] + m[3])} ${DOTS[m[1]]}`;
  return latexToPlain(latex);
}

type SectionKey =
  | "layers"
  | "axes"
  | "section"
  | "lyapunov"
  | "orbit"
  | "continuation"
  | "curves"
  | "planePoints"
  | "solver"
  | "fixedPoints"
  | "trajectories"
  | "selected";
const SECTIONS_KEY = "separatrix.inspector.sections";

export function Inspector({ state, actions }: Props) {
  const sys = state.compiled;
  const [open, setOpen] = useSections<SectionKey>(SECTIONS_KEY, {
    layers: true,
    axes: true,
    section: true,
    lyapunov: true,
    orbit: true,
    continuation: true,
    curves: true,
    planePoints: true,
    solver: true,
    fixedPoints: true,
    trajectories: state.ics.length <= 5,
    selected: true,
  });

  const lyapunov = isLyapunov(state.view);
  const orbit = isOrbit(state.view);
  const branch = isContinuation(state.view);
  const plane = isCurves(state.view);
  const sel = state.selection;
  const [direction, setDirection] = useState<ContinuationDirection>("both");
  const selKind = sel?.kind ?? null;
  const selId = sel ? (sel.kind === "ic" ? sel.id : sel.key) : null;
  useEffect(() => {
    if (selKind === "fp") setOpen("fixedPoints", true);
    if (selKind === "ic") {
      setOpen("trajectories", true);
      setOpen("selected", true);
    }
  }, [selKind, selId, setOpen]);

  return (
    <aside
      className="panel panel-right ds-panel-section-panel ds-panel-section-panel--right inspector"
      aria-label="Inspector"
    >
      {!sys ? (
        <p className="inspector-empty body ds-muted">Pick a template or type an equation to see its settings here.</p>
      ) : (
        <>
          {!lyapunov && !orbit && !branch && !plane && (
            <LayersSection state={state} actions={actions} open={open.layers} onToggle={(v) => setOpen("layers", v)} />
          )}
          <ViewSection state={state} actions={actions} open={open.axes} onToggle={(v) => setOpen("axes", v)} />
          {lyapunov && (
            <LyapunovSection
              state={state}
              actions={actions}
              open={open.lyapunov}
              onToggle={(v) => setOpen("lyapunov", v)}
            />
          )}
          {orbit && (
            <OrbitSection state={state} actions={actions} open={open.orbit} onToggle={(v) => setOpen("orbit", v)} />
          )}
          {branch && (
            <ContinuationSection
              state={state}
              actions={actions}
              open={open.continuation}
              onToggle={(v) => setOpen("continuation", v)}
              direction={direction}
              onDirection={setDirection}
            />
          )}
          {plane && (
            <>
              <CurvesSection state={state} actions={actions} open={open.curves} onToggle={(v) => setOpen("curves", v)} />
              <PlanePointsSection
                state={state}
                actions={actions}
                open={open.planePoints}
                onToggle={(v) => setOpen("planePoints", v)}
              />
            </>
          )}
          {isSection(state.view) && (
            <PoincareSection
              state={state}
              actions={actions}
              open={open.section}
              onToggle={(v) => setOpen("section", v)}
            />
          )}
          {!(lyapunov && sys.kind === "map") && !orbit && !branch && !plane && (
            <SolverSection state={state} actions={actions} open={open.solver} onToggle={(v) => setOpen("solver", v)} />
          )}
          {!lyapunov && !orbit && !branch && !plane && (
            <FixedPointsSection
              state={state}
              actions={actions}
              open={open.fixedPoints}
              onToggle={(v) => setOpen("fixedPoints", v)}
              direction={direction}
            />
          )}
          {!branch && !plane && (
            <TrajectoriesSection
              state={state}
              actions={actions}
              open={open.trajectories}
              onToggle={(v) => setOpen("trajectories", v)}
            />
          )}
          {sel?.kind === "ic" && !plane && (
            <SelectedTrajectorySection
              state={state}
              actions={actions}
              id={sel.id}
              open={open.selected}
              onToggle={(v) => setOpen("selected", v)}
            />
          )}
        </>
      )}
    </aside>
  );
}

interface SectionProps extends Props {
  open: boolean;
  onToggle: (open: boolean) => void;
}

function FieldRow({ label, tip, children }: { label: ReactNode; tip?: string; children: ReactNode }) {
  return (
    <div className="ds-panel-section-kv-row">
      <dt className="ds-panel-section-key body" data-tooltip={tip}>
        <span>{label}</span>
      </dt>
      <dd className="ds-panel-section-value ds-panel-section-value--field">{children}</dd>
    </div>
  );
}

function KvRow({ label, children, text }: { label: ReactNode; children: ReactNode; text?: boolean }) {
  return (
    <div className="ds-panel-section-kv-row">
      <dt className="ds-panel-section-key body">
        <span>{label}</span>
      </dt>
      <dd className={cx("ds-panel-section-value", text ? "ds-panel-section-value--text body" : "numeric")}>
        {children}
      </dd>
    </div>
  );
}

function LayersSection({ state, actions, open, onToggle }: SectionProps) {
  const isMap = state.compiled?.kind === "map";
  const section = isSection(state.view);
  const space = isSpace(state.view);
  const L = state.layers;
  const mapHint = useId();
  const noField = isMap || section || space;
  return (
    <PanelSection title="Layers" open={open} onToggle={onToggle} flush>
      <Switch
        row
        icon={<IconField />}
        label="Direction field"
        checked={L.field && !noField}
        disabled={noField}
        describedBy={noField ? mapHint : undefined}
        title="Direction field (arrows of normalized length)"
        onChange={(field) => actions.setLayers({ field })}
      />
      <Switch
        row
        icon={<IconNullcline />}
        label="Nullclines"
        checked={L.nullclines && !noField}
        disabled={noField}
        describedBy={noField ? mapHint : undefined}
        title="Nullclines (where one component of the field vanishes)"
        onChange={(nullclines) => actions.setLayers({ nullclines })}
      />
      <Switch
        row
        icon={<IconFixedPoint />}
        label="Fixed points"
        checked={L.fixedPoints && !section}
        disabled={section}
        describedBy={section ? mapHint : undefined}
        title="Fixed points (filled when stable, open when unstable)"
        onChange={(fixedPoints) => actions.setLayers({ fixedPoints })}
      />
      <div className="inspector-inset">
        {section ? (
          <p className="ds-panel-section-hint caption" id={mapHint}>
            A Poincaré section shows only the crossings: no field, nullclines or fixed points.
          </p>
        ) : space ? (
          <p className="ds-panel-section-hint caption" id={mapHint}>
            The 3D view draws trajectories and fixed points: no direction field or nullclines.
          </p>
        ) : isMap ? (
          <p className="ds-panel-section-hint caption" id={mapHint}>
            Maps have no direction field or nullclines.
          </p>
        ) : (
          <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
            <FieldRow label="Field grid" tip="Arrows per side of the direction field">
              <NumberField
                size="sm"
                value={L.fieldGrid}
                onCommit={(g) => actions.setLayers({ fieldGrid: Math.round(g) })}
                validate={(g) => (g < 4 || g > 60 ? "Must be from 4 to 60" : null)}
                step={1}
                digits={3}
                unit="per side"
                disabled={!L.field}
                ariaLabel="Arrows per side of the direction field"
              />
            </FieldRow>
          </dl>
        )}
      </div>
    </PanelSection>
  );
}

const axisValue = (a: AxisRef) => (a.kind === "time" ? "time" : `s${a.index}`);
const axisFromValue = (v: string): AxisRef => (v === "time" ? { kind: "time" } : { kind: "state", index: Number(v.slice(1)) });

/** Subscript digits for a caption: x₁₀₀. */
const subDigits = (n: number) => String(n).replace(/\d/g, (d) => "₀₁₂₃₄₅₆₇₈₉"[Number(d)]);

function ViewSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const kind = sys.kind;
  const shown = state.view;
  const view = isParameterView(shown) ? null : shown;
  const plane = isCurves(shown) ? shown : null;
  const cobweb = !!view && isCobweb(view);
  const section = !!view && isSection(view);
  const space = view && isSpace(view) ? view : null;
  const reasonBase = useId();
  const avail = viewChoiceAvailability(sys, systemNameOf(state));
  const choices = viewChoicesFor(sys);
  const viewOptions: SegmentOption<(typeof choices)[number]>[] = choices.map((c) => {
    const a = avail[c];
    return {
      value: c,
      label: VIEW_CHOICE_SHORT[c],
      title: VIEW_CHOICE_LABELS[c],
      ariaLabel: VIEW_CHOICE_LABELS[c],
      ...(a.ok ? {} : { disabled: true, describedBy: `${reasonBase}-${c}` }),
    };
  });
  // The group is described by every reason too: arrow keys skip the disabled options, so it is
  // what a screen reader announces on entering the control.
  const reasonIds = choices.filter((c) => !avail[c].ok)
    .map((c) => `${reasonBase}-${c}`)
    .join(" ");
  const names = view ? viewAxisLabels(view, sys.states, kind) : { x: "", y: "" };
  const options: SelectOption<string>[] = [
    ...sys.states.map((s, i) => ({ value: `s${i}`, label: optionName(s.latex) })),
    ...(section ? [] : [{ value: "time", label: kind === "map" ? "n" : "t" }]),
  ];
  // A return map's axes are one state at successive crossings, named xₖ and xₖ₊₁ as on the plot.
  const named = (axis: AxisRef, step: string) =>
    view && isReturnMap(view) ? options.map((o) => (o.value === axisValue(axis) ? { ...o, label: `${o.label}${step}` } : o)) : options;
  const sliceStates =
    view && kind === "ode" && !section && !space && !isLyapunov(view)
      ? sys.states
          .map((s, i) => ({ s, i }))
          .filter(({ i }) => !sameAxis(view.xAxis, { kind: "state", index: i }) && !sameAxis(view.yAxis, { kind: "state", index: i }))
      : [];
  const timeSlice =
    !!view && kind === "ode" && !section && !space && !sys.autonomous && view.xAxis.kind !== "time" && view.yAxis.kind !== "time";
  const hId = useId();
  const vId = useId();
  return (
    <PanelSection title="View" open={open} onToggle={onToggle}>
      <div className="inspector-view-control">
        <SegmentedControl
          size="sm"
          fill
          columns={3}
          ariaLabel="View"
          value={viewChoice(shown)}
          options={viewOptions}
          onChange={actions.setViewKind}
          describedBy={reasonIds || undefined}
        />
        {choices.map((c) => {
          const a = avail[c];
          return a.ok ? null : (
            <p key={c} className="ds-panel-section-hint caption" id={`${reasonBase}-${c}`}>
              <MathProse text={a.reason} />
            </p>
          );
        })}
      </div>
      {plane ? (
        <p className="ds-panel-section-hint caption" data-plane-caption="">
          {sliceCaption(plane, sys.params.map((q) => plainSubscripted(q.latex)))}
        </p>
      ) : !view ? (
        <p className="ds-panel-section-hint caption">
          The parameter across and the measure up are set in the Continuation panel.
        </p>
      ) : isOrbit(view) ? (
        <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
          <div className="ds-panel-section-kv-row">
            <dt className="ds-panel-section-key body">
              <label className="inspector-key-label" htmlFor={hId}>
                Horizontal
              </label>
            </dt>
            <dd className="ds-panel-section-value ds-panel-section-value--field">
              <Select
                id={hId}
                size="sm"
                math
                value={`p${view.param}`}
                options={sys.params.map((p, i) => ({ value: `p${i}`, label: optionName(p.latex) }))}
                onChange={(v) => actions.setOrbitParam(Number(v.slice(1)))}
              />
            </dd>
          </div>
          <div className="ds-panel-section-kv-row">
            <dt className="ds-panel-section-key body">
              <label className="inspector-key-label" htmlFor={vId}>
                Vertical
              </label>
            </dt>
            <dd className="ds-panel-section-value ds-panel-section-value--field">
              <Select
                id={vId}
                size="sm"
                math
                value={`s${view.yAxis.index}`}
                options={sys.states.map((s, i) => ({ value: `s${i}`, label: optionName(s.latex) }))}
                onChange={(v) => {
                  const i = Number(v.slice(1));
                  const ext = stateExtent(state.portrait?.res, sys.states.length, i);
                  const pad = ext ? Math.max(0.1 * (ext[1] - ext[0]), 0.05) : 0;
                  const yRange: [number, number] = ext ? [ext[0] - pad, ext[1] + pad] : view.yRange;
                  actions.setView({ ...view, yAxis: stateAxis(i), yRange });
                }}
              />
            </dd>
          </div>
        </dl>
      ) : isLyapunov(view) ? (
        <>
          <dl className="ds-panel-section-kv">
            <KvRow label="Horizontal" text>
              <span className="math">
                <Tex latex={kind === "map" ? "n" : "t"} />
              </span>
            </KvRow>
            <KvRow label="Vertical" text>
              <span className="math">
                <Tex latex="\lambda" />
              </span>
            </KvRow>
          </dl>
          <p className="ds-panel-section-hint caption">
            The running estimate of each exponent as the orbit is followed; it settles as {kind === "map" ? "n" : "t"} grows.
          </p>
        </>
      ) : space ? (
        <>
          <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
            {spaceStates(space).map((index, slot) => (
              <FieldRow key={slot} label={`${["First", "Second", "Third"][slot]} axis`}>
                <Select
                  size="sm"
                  math
                  value={`s${index}`}
                  options={sys.states.map((s, i) => ({ value: `s${i}`, label: optionName(s.latex) }))}
                  onChange={(v) => actions.setSpaceAxis(slot as 0 | 1 | 2, Number(v.slice(1)))}
                  ariaLabel={`${["First", "Second", "Third"][slot]} axis of the 3D view`}
                />
              </FieldRow>
            ))}
          </dl>
          <p className="ds-panel-section-hint caption">
            Drag the plot to turn the box and scroll to zoom. A state already on another axis swaps with it.
          </p>
        </>
      ) : cobweb ? (
        <>
          <dl className="ds-panel-section-kv">
            <KvRow label="Horizontal" text>
              <span className="math">
                <Tex latex={names.x} />
              </span>
            </KvRow>
            <KvRow label="Vertical" text>
              <span className="math">
                <Tex latex={names.y} />
              </span>
            </KvRow>
          </dl>
          <p className="ds-panel-section-hint caption">
            Equal scales: the diagonal is at 45°, so a slope steeper than it means |f′| &gt; 1.
          </p>
        </>
      ) : (
        <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
          <div className="ds-panel-section-kv-row">
            <dt className="ds-panel-section-key body">
              <label className="inspector-key-label" htmlFor={hId}>
                Horizontal
              </label>
            </dt>
            <dd className="ds-panel-section-value ds-panel-section-value--field">
              <Select
                id={hId}
                size="sm"
                math
                value={axisValue(view.xAxis)}
                options={named(view.xAxis, "ₖ")}
                onChange={(v) => actions.setAxis("x", axisFromValue(v))}
              />
            </dd>
          </div>
          <div className="ds-panel-section-kv-row">
            <dt className="ds-panel-section-key body">
              <label className="inspector-key-label" htmlFor={vId}>
                Vertical
              </label>
            </dt>
            <dd className="ds-panel-section-value ds-panel-section-value--field">
              <Select
                id={vId}
                size="sm"
                math
                value={axisValue(view.yAxis)}
                options={named(view.yAxis, "ₖ₊₁")}
                onChange={(v) => actions.setAxis("y", axisFromValue(v))}
              />
            </dd>
          </div>
        </dl>
      )}
      {(sliceStates.length > 0 || timeSlice) && (
        <>
          <h3 className="ds-panel-section-subhead label">Field slice</h3>
          <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
            {sliceStates.map(({ s, i }) => (
              <FieldRow key={s.name} label={<>Held at</>}>
                <NumberField
                  size="sm"
                  lead={<Tex latex={`${s.latex} =`} />}
                  value={state.baseState[i] ?? 0}
                  onCommit={(v) => actions.setSlice(i, v)}
                  step={0.1}
                  digits={4}
                  ariaLabel={`Slice value of ${s.name}`}
                />
              </FieldRow>
            ))}
            {timeSlice && (
              <FieldRow label="Field at time">
                <NumberField
                  size="sm"
                  lead={<Tex latex="t =" />}
                  value={state.fieldTime}
                  onCommit={actions.setFieldTime}
                  step={0.1}
                  digits={4}
                  ariaLabel="Time at which the field is drawn"
                />
              </FieldRow>
            )}
          </dl>
          <p className="ds-panel-section-hint caption">
            The field and nullclines are drawn with {timeSlice && sliceStates.length === 0 ? "time" : "these values"}{" "}
            held fixed.
          </p>
        </>
      )}
    </PanelSection>
  );
}

const CROSSINGS: SelectOption<Direction>[] = [
  { value: "rising", label: "Increasing" },
  { value: "falling", label: "Decreasing" },
  { value: "both", label: "Both" },
];

const CROSSING_WORDS: Record<Direction, string> = { rising: "upward", falling: "downward", both: "either way" };


function PoincareSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const view = state.view;
  const kindHint = useId();
  const stateId = useId();
  const crossingId = useId();
  if (!isSection(view)) return null;
  const sec = view.section;
  const plane = planeSectionAvailability(sys, systemNameOf(state));
  const skip = state.solver.transient ?? 0;
  const latexOf = (i: number) => sys.states[i]?.latex ?? "x";
  const set = (spec: SectionSpec) => actions.setSection(spec);
  const toKind = (k: SectionSpec["kind"]) => {
    if (k === sec.kind) return;
    if (k === "stroboscopic") set({ kind: "stroboscopic", period: 2 * Math.PI, phase: 0 });
    else {
      const on = (a: AxisRef, i: number) => a.kind === "state" && a.index === i;
      const free = sys.states.findIndex((_, i) => !on(view.xAxis, i) && !on(view.yAxis, i));
      const i = Math.max(0, free);
      const r = on(view.xAxis, i)
        ? view.xRange
        : on(view.yAxis, i)
          ? view.yRange
          : (stateExtent(state.portrait?.res, sys.states.length, i) ?? [-1, 1]);
      set({ kind: "plane", state: i, value: (r[0] + r[1]) / 2, direction: "rising" });
    }
  };
  const what =
    sec.kind === "plane"
      ? `Dots where an orbit crosses ${plainName(latexOf(sec.state))} = ${withMinus(fmt(sec.value, 4))} ${CROSSING_WORDS[sec.direction]}`
      : `Dots at t = ${withMinus(fmt(sec.phase, 4))} + k·${withMinus(fmt(sec.period, 4))}, k = 0, 1, 2, …`;
  const skipped = skip === 0 ? "." : `, after the first ${skip.toLocaleString("en-US")} ${skip === 1 ? "one" : "ones"}.`;
  return (
    <PanelSection title="Section" open={open} onToggle={onToggle}>
      <SegmentedControl
        size="sm"
        fill
        ariaLabel="Section kind"
        value={sec.kind}
        options={[
          {
            value: "plane",
            label: "Plane",
            title: "Where orbits cross a plane: one state held at a value",
            ...(plane.ok ? {} : { disabled: true, describedBy: kindHint }),
          },
          { value: "stroboscopic", label: "Stroboscopic", title: "The state once per forcing period" },
        ]}
        onChange={toKind}
      />
      {!plane.ok && (
        <p className="ds-panel-section-hint caption" id={kindHint}>
          {plane.reason}
        </p>
      )}
      <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
        {sec.kind === "plane" ? (
          <>
            <div className="ds-panel-section-kv-row">
              <dt className="ds-panel-section-key body">
                <label className="inspector-key-label" htmlFor={stateId}>
                  State
                </label>
              </dt>
              <dd className="ds-panel-section-value ds-panel-section-value--field">
                <Select
                  id={stateId}
                  size="sm"
                  math
                  value={`s${sec.state}`}
                  options={sys.states.map((s, i) => ({ value: `s${i}`, label: optionName(s.latex) }))}
                  onChange={(v) => set({ ...sec, state: Number(v.slice(1)) })}
                />
              </dd>
            </div>
            <FieldRow label="Value" tip="Where the plane cuts the state's axis">
              <NumberField
                size="sm"
                lead={<Tex latex={`${latexOf(sec.state)} =`} />}
                value={sec.value}
                onCommit={(value) => set({ ...sec, value })}
                step={0.1}
                digits={4}
                ariaLabel="Value of the section plane"
              />
            </FieldRow>
            <div className="ds-panel-section-kv-row">
              <dt className="ds-panel-section-key body">
                <label className="inspector-key-label" htmlFor={crossingId}>
                  Crossing
                </label>
              </dt>
              <dd className="ds-panel-section-value ds-panel-section-value--field">
                <Select
                  id={crossingId}
                  size="sm"
                  value={sec.direction}
                  options={CROSSINGS}
                  onChange={(direction) => set({ ...sec, direction })}
                />
              </dd>
            </div>
          </>
        ) : (
          <>
            <FieldRow label="Period" tip="The forcing period: one sample per period">
              <NumberField
                size="sm"
                lead={<Tex latex="T =" />}
                value={sec.period}
                onCommit={(period) => set({ ...sec, period })}
                validate={(v) => (v > 0 ? null : "Must be above 0")}
                step={0.1}
                digits={4}
                ariaLabel="Stroboscopic period"
              />
            </FieldRow>
            <FieldRow label="Phase" tip="The sample times modulo the period">
              <NumberField
                size="sm"
                lead={<Tex latex="\varphi =" />}
                value={sec.phase}
                onCommit={(phase) => set({ ...sec, phase })}
                step={0.1}
                digits={4}
                ariaLabel="Stroboscopic phase"
              />
            </FieldRow>
          </>
        )}
        <FieldRow label="Skip crossings" tip="Crossings computed but not drawn, so the attractor stands out">
          <NumberField
            size="sm"
            value={skip}
            onCommit={(v) => actions.setSolver({ transient: Math.round(v) })}
            validate={(v) =>
              Number.isInteger(v) && v >= 0 && v <= MAX_SECTION_POINTS
                ? null
                : `Must be a whole number from 0 to ${MAX_SECTION_POINTS.toLocaleString("en-US")}`
            }
            step={1}
            steppers
            format={(v) => String(v)}
            ariaLabel="Crossings skipped at the start of each orbit"
          />
        </FieldRow>
      </dl>
      <p className="ds-panel-section-hint caption" aria-live="polite">
        {what}
        {skipped}
        {isReturnMap(view) ? " Each dot pairs a crossing with the next." : ""}
      </p>
    </PanelSection>
  );
}

const LYAPUNOV_FIELDS: Record<keyof LyapunovSettings, string> = {
  transient: "transient",
  total: "measured span",
  interval: "renormalization interval",
  blocks: "blocks",
};

const ORBIT_FIELDS: Record<keyof OrbitSettings, string> = { transient: "transient", kept: "iterates drawn" };

function OrbitSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const view = state.view;
  if (!isOrbit(view)) return null;
  const settings = view.orbit;
  const validate = (field: keyof OrbitSettings) => (v: number) => {
    const err = orbitSettingsError({ ...settings, [field]: v });
    if (!err) return null;
    return err.field === field ? err.message : `Then the ${ORBIT_FIELDS[err.field]}: ${err.message.toLowerCase()}`;
  };
  const set = (field: keyof OrbitSettings) => (v: number) => actions.setOrbit({ [field]: Math.round(v) });
  const whole = (v: number) => v.toLocaleString("en-US");
  const p = sys.params[view.param];
  const start = lyapunovStart(state.ics, state.selection, sys.states.length);
  const startText = start
    ? sys.states.length === 1
      ? `${plainName(sys.states[0].latex)}${sub(0)} = ${dec(start.x0[0], 3)}`
      : `(${sys.states.map((st) => plainName(st.latex) + sub(0)).join(", ")}) = (${start.x0.map((v) => dec(v, 3)).join(", ")})`
    : null;
  const which = state.selection?.kind === "ic" && start?.id === state.selection.id ? "the selected start point" : "the first start point";
  const run = orbitFits(sys, state.shapeId, view, state.orbit) ? state.orbit : null;
  const pName = p ? plainName(p.latex) : "the parameter";
  const rows = run?.res.doublings ? doublingRows(run.res.doublings, p ? pName : "r") : null;
  return (
    <PanelSection title="Orbit diagram" id="orbit-settings" open={open} onToggle={onToggle}>
      <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
        <FieldRow label="From" tip={`The first value of ${pName} across`}>
          <NumberField
            size="sm"
            lead={<Tex latex={`${p?.latex ?? "r"} =`} />}
            value={view.xRange[0]}
            onCommit={(v) => actions.setOrbitRange(v, view.xRange[1])}
            validate={(v) => (v < view.xRange[1] ? null : `Must be below ${dec(view.xRange[1], 6)}`)}
            digits={6}
            ariaLabel={`First value of ${pName}`}
          />
        </FieldRow>
        <FieldRow label="To" tip={`The last value of ${pName} across`}>
          <NumberField
            size="sm"
            lead={<Tex latex={`${p?.latex ?? "r"} =`} />}
            value={view.xRange[1]}
            onCommit={(v) => actions.setOrbitRange(view.xRange[0], v)}
            validate={(v) => (v > view.xRange[0] ? null : `Must be above ${dec(view.xRange[0], 6)}`)}
            digits={6}
            ariaLabel={`Last value of ${pName}`}
          />
        </FieldRow>
        <FieldRow label="Transient" tip="Iterates dropped in every column, so the orbit settles first">
          <NumberField
            size="sm"
            lead={<Tex latex="N_0" />}
            value={settings.transient}
            onCommit={set("transient")}
            validate={validate("transient")}
            format={whole}
            ariaLabel="Transient iterates in every column"
          />
        </FieldRow>
        <FieldRow label="Drawn" tip="Iterates drawn in every column, after the transient">
          <NumberField
            size="sm"
            lead={<Tex latex="N" />}
            value={settings.kept}
            onCommit={set("kept")}
            validate={validate("kept")}
            format={whole}
            ariaLabel="Iterates drawn in every column"
          />
        </FieldRow>
      </dl>
      <p className="ds-panel-section-hint caption" data-orbit-start="">
        {startText
          ? `Every column starts at ${which}, ${startText}, one column per pixel of ${pName}.`
          : "Add a start point: every column of the diagram starts there."}
      </p>
      {findsDoublings(sys) && (
        <>
          <h3 className="ds-panel-section-subhead label">Period doublings</h3>
          {rows && rows.length > 0 ? (
            <>
              <dl className="ds-panel-section-kv" data-orbit-doublings="">
                {rows.map((d) => (
                  <KvRow key={d.r} label={d.label}>
                    {withMinus(d.r.toFixed(6))}
                  </KvRow>
                ))}
              </dl>
              <p className="ds-panel-section-hint caption">
                Where the cycle of period 2ⁿ⁻¹ loses stability and one of twice the period is born, as its multiplier
                passes −1. Found in the columns shown, then refined to 10⁻¹².
              </p>
            </>
          ) : (
            <p className="ds-panel-section-hint caption" data-orbit-doublings="">
              {run ? `None between ${pName} = ${dec(view.xRange[0], 4)} and ${dec(view.xRange[1], 4)}.` : "Computing…"}
            </p>
          )}
        </>
      )}
    </PanelSection>
  );
}

const sub = (n: number | string) => String(n).replace(/\d/g, (d) => "₀₁₂₃₄₅₆₇₈₉"[Number(d)]);

const DIRECTIONS: SegmentOption<ContinuationDirection>[] = [
  { value: "forward", label: "Forward", title: "With the parameter increasing" },
  { value: "backward", label: "Backward", title: "With the parameter decreasing" },
  { value: "both", label: "Both", title: "Both ways from the start, joined into one branch" },
];

function stateText(names: readonly string[], x: readonly number[]): string {
  return names.length === 1 ? `${names[0]} = ${dec(x[0], 4)}` : `(${names.join(", ")}) = (${x.map((v) => dec(v, 4)).join(", ")})`;
}

function specialParts(at: SpecialPointOut, pName: string, names: readonly string[]): string[] {
  const info = SPECIAL_KINDS[at.kind];
  const parts = [`${pName} = ${dec(at.p, 6)}`, stateText(names, at.x)];
  if (at.period > 1) parts.push(`period ${at.period}`);
  if (info.omega && at.omega !== null) parts.push(`${info.omega} = ${dec(at.omega, 6)}`);
  if (at.lyapunov !== null)
    parts.push(`ℓ₁ = ${dec(at.lyapunov, 6)} (${at.lyapunov < 0 ? "supercritical" : "subcritical"})`);
  return parts;
}

const seedText = (seed: ContinuationSeed, pName: string, names: readonly string[]) =>
  seed.from === "point"
    ? `From ${stateText(names, seed.x)} at ${pName} = ${dec(seed.param, 4)}`
    : `Switched at ${SPECIAL_KINDS[seed.at.kind].label} ${pName} = ${dec(seed.at.p, 6)}`;

function ContinuationSection({
  state,
  actions,
  open,
  onToggle,
  direction,
  onDirection,
}: SectionProps & { direction: ContinuationDirection; onDirection: (d: ContinuationDirection) => void }) {
  const sys = state.compiled!;
  const view = state.view;
  const yId = useId();
  const pId = useId();
  const freeId = useId();
  const [follow, setFollow] = useState<{ key: string; up: number } | null>(null);
  if (!isContinuation(view)) return null;
  const twoParams = curvesAvailability(sys).ok;
  const c = view.continuation;
  const [lo, hi] = c.range;
  const p = sys.params[view.param];
  const pLatex = p?.latex ?? "p";
  const pName = plainSubscripted(pLatex);
  const names = sys.states.map((st) => plainName(st.latex));
  const shown = shownBranches(state);
  const runs = shown.map((b) => b.run);
  const rows = specialRows(runs);
  const followable = twoParams && rows.some(({ at }) => curveKindOf(at.kind));
  const value = state.params[view.param]?.value;
  const fp = state.selection?.kind === "fp" && state.selection.x.length === sys.states.length ? state.selection.x : null;
  const settingError = (patch: Partial<typeof c>) => {
    const err = continuationSettingsError({ ...c, ...patch });
    return err ? err.message : null;
  };
  const whole = (v: number) => v.toLocaleString("en-US");
  const measure = view.yAxis.kind === "norm" ? "norm" : `s${view.yAxis.index}`;
  return (
    <PanelSection title="Continuation" id="continuation-settings" open={open} onToggle={onToggle}>
      <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
        <div className="ds-panel-section-kv-row">
          <dt className="ds-panel-section-key body">
            <label className="inspector-key-label" htmlFor={pId}>
              Parameter
            </label>
          </dt>
          <dd className="ds-panel-section-value ds-panel-section-value--field">
            <Select
              id={pId}
              size="sm"
              math
              value={`p${view.param}`}
              options={sys.params.map((q, i) => ({ value: `p${i}`, label: optionName(q.latex) }))}
              onChange={(v) => actions.setContinuationParam(Number(v.slice(1)))}
            />
          </dd>
        </div>
        <FieldRow label="From" tip={`The lowest ${pName} a branch reaches`}>
          <NumberField
            size="sm"
            lead={<Tex latex={`${pLatex} =`} />}
            value={lo}
            onCommit={(v) => actions.setContinuation({ range: [v, hi] })}
            validate={(v) => settingError({ range: [v, hi] })}
            digits={6}
            ariaLabel={`Lowest value of ${pName}`}
          />
        </FieldRow>
        <FieldRow label="To" tip={`The highest ${pName} a branch reaches`}>
          <NumberField
            size="sm"
            lead={<Tex latex={`${pLatex} =`} />}
            value={hi}
            onCommit={(v) => actions.setContinuation({ range: [lo, v] })}
            validate={(v) => settingError({ range: [lo, v] })}
            digits={6}
            ariaLabel={`Highest value of ${pName}`}
          />
        </FieldRow>
        <FieldRow label="Δs" tip="The first step along the branch, in arclength; steps grow to twice it where the branch is straight">
          <NumberField
            size="sm"
            lead={<Tex latex="\Delta s =" />}
            value={c.ds}
            onCommit={(v) => actions.setContinuation({ ds: v })}
            validate={(v) => settingError({ ds: v })}
            digits={6}
            ariaLabel="First step in arclength"
          />
        </FieldRow>
        <FieldRow label="Max points" tip={`The most points of a branch each way, up to ${whole(MAX_CONTINUATION_POINTS)}`}>
          <NumberField
            size="sm"
            lead={<Tex latex="N =" />}
            value={c.maxPoints}
            onCommit={(v) => actions.setContinuation({ maxPoints: Math.round(v) })}
            validate={(v) => settingError({ maxPoints: Math.round(v) })}
            format={whole}
            ariaLabel="Most points of a branch each way"
          />
        </FieldRow>
        <div className="ds-panel-section-kv-row">
          <dt className="ds-panel-section-key body">
            <label className="inspector-key-label" htmlFor={yId}>
              Vertical
            </label>
          </dt>
          <dd className="ds-panel-section-value ds-panel-section-value--field">
            <Select
              id={yId}
              size="sm"
              math
              value={measure}
              options={[
                ...sys.states.map((st, i) => ({ value: `s${i}`, label: optionName(st.latex) })),
                { value: "norm", label: "‖x‖" },
              ]}
              onChange={(v) => {
                const yAxis = v === "norm" ? ({ kind: "norm" } as const) : stateAxis(Number(v.slice(1)));
                actions.setContinuationAxis(yAxis, fitBranchRanges(runs, sys.states.length, yAxis)?.yRange ?? view.yRange);
              }}
            />
          </dd>
        </div>
      </dl>
      <div className="inspector-continue">
        <SegmentedControl
          size="sm"
          fill
          ariaLabel="Direction"
          value={direction}
          options={DIRECTIONS}
          onChange={onDirection}
        />
        <Button
          size="sm"
          disabled={!fp || value === undefined}
          onClick={() => fp && value !== undefined && actions.addSeed(pointSeed(fp, value, direction))}
        >
          Continue from selection
        </Button>
        <p className="ds-panel-section-hint caption" data-continue-from="">
          {fp && value !== undefined
            ? `From ${stateText(names, fp)} at ${pName} = ${dec(value, 4)}.`
            : "Select a fixed point in the Phase or Cobweb view, or press Continue on its row in the Fixed points panel there."}
        </p>
      </div>
      <h3 className="ds-panel-section-subhead label">Branches</h3>
      {shown.length === 0 ? (
        <p className="ds-panel-section-hint caption">None yet.</p>
      ) : (
        <ul className="ds-traj-list" aria-label="Branches" data-branches="">
          {shown.map(({ job, run }, k) => {
            const seed = c.seeds[job.meta.seed];
            const b = run?.ok ? run.res.branch : null;
            const status = !run
              ? "Computing…"
              : !run.ok
                ? run.error
                : `Period ${b!.period} · ${whole(b!.p.length)} points · ${endText(b!.end)}`;
            return (
              <li
                key={job.meta.key}
                className="ds-traj-row"
                style={{ ["--traj" as string]: `var(--traj-${(job.meta.seed % PALETTE_SIZE) + 1})` }}
                data-branch-index={k}
              >
                <span className="ds-traj-select inspector-branch">
                  <span className="ds-traj-line">
                    <span className="ds-traj-swatch" aria-hidden="true" />
                    <span className="body-strong">Branch {k + 1}</span>
                  </span>
                  <span className="ds-traj-meta">
                    <span className="numeric-sm">{seedText(seed, pName, names)}</span>
                    <span className={cx("caption", run && !run.ok && "inspector-branch-error")}>{status}</span>
                  </span>
                </span>
                <span className="ds-traj-actions">
                  <IconButton size="sm" label={`Remove branch ${k + 1}`} tipAlign="end" onClick={() => actions.removeSeed(job.meta.seed)}>
                    <IconTrash />
                  </IconButton>
                </span>
              </li>
            );
          })}
        </ul>
      )}
      <div className="inspector-subhead-row">
        <h3 className="ds-panel-section-subhead label">Special points</h3>
        {twoParams && (
          <Button
            size="sm"
            variant="ghost"
            icon={<IconSearch />}
            disabled={!followable}
            data-tooltip="Follow every fold, Hopf, branch point, flip and Neimark–Sacker point here in two parameters"
            onClick={actions.searchInPlane}
          >
            Search in two parameters
          </Button>
        )}
      </div>
      {rows.length === 0 ? (
        <p className="ds-panel-section-hint caption" data-special-points="">
          {shown.some((b) => !b.run) ? "Computing…" : "None on these branches."}
        </p>
      ) : (
        <ul className="ds-fp-list" aria-label="Special points" data-special-points="">
          {rows.map(({ at }) => {
            const info = SPECIAL_KINDS[at.kind];
            const parts = specialParts(at, pName, names);
            const text = parts.join("  ");
            const switched = info.switches ? switchedBranch(c.seeds, at) : null;
            const switchedShown = switched === null ? -1 : shown.findIndex((b) => b.job.meta.seed === switched - 1);
            const curveKind = twoParams ? curveKindOf(at.kind) : null;
            const rowKey = `${at.kind}-${at.period}-${at.p}`;
            const panel = follow?.key === rowKey ? follow : null;
            return (
              <li key={`${at.kind}-${at.period}-${at.p}`} className="ds-fp-item" data-special-kind={at.kind}>
                <button
                  type="button"
                  className="ds-fp-row"
                  aria-label={`${info.name}: ${text}. Set ${pName} to it`}
                  data-tooltip={`${info.name}: set ${pName} to ${dec(at.p, 6)}`}
                  onClick={() => p && actions.setParam(p.name, at.p)}
                >
                  <FixedPointIcon kind="half" />
                  <span className="ds-fp-row-text">
                    <span className="ds-fp-class body-strong">{info.label}</span>
                    <span className="ds-fp-eig numeric-sm inspector-special-text">
                      {parts.map((part, i) => (
                        <Fragment key={i}>
                          {i > 0 && "  "}
                          <span className={part.includes(", ") ? undefined : "nowrap"}>{part}</span>
                        </Fragment>
                      ))}
                    </span>
                  </span>
                </button>
                {(info.switches || curveKind) && (
                  <div className="ds-fp-detail">
                    {info.switches &&
                      (switched === null ? (
                        <Button size="sm" onClick={() => actions.addSeed(switchSeed(at))}>
                          Switch branch
                        </Button>
                      ) : (
                        <span className="caption" data-switched-branch={switchedShown + 1}>
                          Followed as Branch {switchedShown + 1}
                        </span>
                      ))}
                    {curveKind && !panel && (
                      <Button
                        size="sm"
                        icon={<IconFollow />}
                        aria-expanded={false}
                        onClick={() => setFollow({ key: rowKey, up: nextParam(view.param, sys.params.length) })}
                      >
                        Follow in two parameters…
                      </Button>
                    )}
                    {curveKind && panel && (
                      <div className="inspector-follow" role="group" aria-label={`Follow this ${CURVE_KINDS[curveKind].point} in two parameters`}>
                        <p className="body-strong inspector-follow-title">
                          Follow this {CURVE_KINDS[curveKind].point} in two parameters
                        </p>
                        <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
                          <div className="ds-panel-section-kv-row">
                            <dt className="ds-panel-section-key body">
                              <label className="inspector-key-label" htmlFor={freeId}>
                                Free also
                              </label>
                            </dt>
                            <dd className="ds-panel-section-value ds-panel-section-value--field">
                              <Select
                                id={freeId}
                                size="sm"
                                math
                                value={`p${panel.up}`}
                                options={sys.params.flatMap((q, i) =>
                                  i === view.param ? [] : [{ value: `p${i}`, label: optionName(q.latex) }],
                                )}
                                onChange={(v) => setFollow({ key: rowKey, up: Number(v.slice(1)) })}
                              />
                            </dd>
                          </div>
                        </dl>
                        <p className="ds-panel-section-hint caption" data-follow-note="">
                          {followNote(pName, plainSubscripted(sys.params[panel.up]?.latex ?? ""), state.params[panel.up])}
                        </p>
                        <div className="inspector-follow-actions">
                          <Button
                            size="sm"
                            variant="primary"
                            onClick={() => {
                              setFollow(null);
                              actions.followInPlane(at, panel.up);
                            }}
                          >
                            Follow
                          </Button>
                          <Button size="sm" onClick={() => setFollow(null)}>
                            Cancel
                          </Button>
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      <p className="ds-panel-section-hint caption">
        LP fold, H Hopf, BP branch point, PD period doubling (flip), NS Neimark–Sacker. Solid where the fixed point is
        stable, dashed where it is not. Click a row to set {pName} there.
        {twoParams && " Follow in two parameters frees a second parameter and draws the curve in the Parameter plane."}
      </p>
    </PanelSection>
  );
}

/** "Opens the Parameter plane with k₂ across and K up, from 0 to 2." */
function followNote(across: string, up: string, p: { min: number; max: number } | undefined): string {
  const range = p && p.min < p.max ? `, from ${short(p.min)} to ${short(p.max)}` : "";
  return `Opens the Parameter plane with ${across} across and ${up} up${range}.`;
}

/** Whether the plane's slice line is drawn, and why. */
function sliceCaption(v: CurvesView, names: readonly string[]): string {
  if (!v.branch) return "Across and up are set in the Curves panel. Open the Branch view first to see its range here too.";
  if (v.branch.param === v.across) return "Across is always the Branch view’s parameter, so the two views line up.";
  return `The Branch view’s range is hidden: it is a range of ${names[v.branch.param]}, and ${names[v.across]} is across here.`;
}

const PLANE_DIRECTIONS: SegmentOption<ContinuationDirection>[] = [
  { value: "forward", label: "Forward", title: "One way from each starting point" },
  { value: "backward", label: "Backward", title: "The other way from each starting point" },
  { value: "both", label: "Both", title: "Both ways from each starting point, joined into one curve" },
];

function CurvesSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const view = state.view;
  const aId = useId();
  const uId = useId();
  if (!isCurves(view)) return null;
  const c = view.curves;
  const names = sys.params.map((q) => plainSubscripted(q.latex));
  const latex = sys.params.map((q) => q.latex);
  const jobs = buildCurveJobs(state);
  const rows = state.planeRows;
  const done = jobs.filter((j) => j.meta.key in state.curveRuns).length;
  const searching = done < jobs.length;
  const soFar = planePointRows(drawnCurves(rows), c.ranges).length;
  const shownCount = rows.filter((r) => r.status === "done").length;
  const settingError = (patch: Partial<typeof c>) => {
    const err = curvesSettingsError({ ...c, ...patch });
    return err ? err.message : null;
  };
  const whole = (v: number) => v.toLocaleString("en-US");
  const paramOptions = sys.params.map((q, i) => ({ value: `p${i}`, label: optionName(q.latex) }));
  const rangeRow = (which: 0 | 1, id: string) => {
    const [lo, hi] = c.ranges[which];
    const i = which === 0 ? view.across : view.up;
    const set = (r: [number, number]) =>
      actions.setCurves({ ranges: which === 0 ? [r, c.ranges[1]] : [c.ranges[0], r] });
    const err = (r: [number, number]) => settingError({ ranges: which === 0 ? [r, c.ranges[1]] : [c.ranges[0], r] });
    return (
      <>
        <div className="ds-panel-section-kv-row">
          <dt className="ds-panel-section-key body">
            <label className="inspector-key-label" htmlFor={id}>
              {which === 0 ? "Across" : "Up"}
            </label>
          </dt>
          <dd className="ds-panel-section-value ds-panel-section-value--field">
            <Select
              id={id}
              size="sm"
              math
              value={`p${i}`}
              options={paramOptions}
              onChange={(v) => actions.setCurvesParam(which === 0 ? "across" : "up", Number(v.slice(1)))}
            />
          </dd>
        </div>
        <FieldRow label="Range" tip={`The ${names[i]} the curves stay between`}>
          <span className="inspector-range-pair">
            <NumberField
              size="sm"
              lead={<Tex latex={`${latex[i]} \\geq`} />}
              value={lo}
              onCommit={(v) => set([v, hi])}
              validate={(v) => err([v, hi])}
              digits={6}
              ariaLabel={`Lowest ${names[i]}`}
            />
            <NumberField
              size="sm"
              lead={<Tex latex="\leq" />}
              value={hi}
              onCommit={(v) => set([lo, v])}
              validate={(v) => err([lo, v])}
              digits={6}
              ariaLabel={`Highest ${names[i]}`}
            />
          </span>
        </FieldRow>
      </>
    );
  };
  return (
    <PanelSection title="Curves" id="curves-settings" count={shownCount} open={open} onToggle={onToggle}>
      <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
        {rangeRow(0, aId)}
        {rangeRow(1, uId)}
        <FieldRow label="Δs" tip="The first step along each curve, in arclength; steps grow to twice it where the curve is straight">
          <NumberField
            size="sm"
            lead={<Tex latex="\Delta s =" />}
            value={c.ds}
            onCommit={(v) => actions.setCurves({ ds: v })}
            validate={(v) => settingError({ ds: v })}
            digits={6}
            ariaLabel="First step in arclength"
          />
        </FieldRow>
        <FieldRow label="Max points" tip={`The most points of a curve each way, up to ${whole(MAX_CONTINUATION_POINTS)}`}>
          <NumberField
            size="sm"
            lead={<Tex latex="N =" />}
            value={c.maxPoints}
            onCommit={(v) => actions.setCurves({ maxPoints: Math.round(v) })}
            validate={(v) => settingError({ maxPoints: Math.round(v) })}
            format={whole}
            ariaLabel="Most points of a curve each way"
          />
        </FieldRow>
      </dl>
      <div className="inspector-continue">
        <SegmentedControl
          size="sm"
          fill
          ariaLabel="Direction"
          value={c.direction}
          options={PLANE_DIRECTIONS}
          onChange={(direction) => actions.setCurves({ direction })}
        />
      </div>
      {searching && (
        <div className="inspector-search" role="status" data-curves-search="">
          <span className="caption">
            <span className="numeric-sm">{done}</span> of <span className="numeric-sm">{jobs.length}</span> curves · special
            points so far: <span className="numeric-sm">{soFar}</span>
          </span>
          <Button size="sm" onClick={actions.stopCurves}>
            Stop
          </Button>
        </div>
      )}
      <h3 className="ds-panel-section-subhead label">Curves</h3>
      {rows.length === 0 && state.homoclinicRows.length === 0 ? (
        <p className="ds-panel-section-hint caption">Curves appear here, one per bifurcation followed.</p>
      ) : (
        <ul className="ds-traj-list" aria-label="Curves" data-curves="">
          {rows.map((row) => {
            const seed = c.seeds[row.job.meta.seed];
            const kind = seed.kind;
            const info = CURVE_KINDS[kind];
            const [pa, pu] = [seed.params[view.across], seed.params[view.up]];
            const from = `From ${info.label} at ${names[view.across]} = ${dec(pa, 4)}, ${names[view.up]} = ${short(pu)}`;
            const curve = row.status === "done" || row.status === "merged" ? row.res.curve : null;
            const neutral = curve ? neutralText(curve) : "";
            const status =
              row.status === "computing"
                ? null
                : row.status === "failed"
                  ? refusalText(row.error)
                  : row.status === "merged"
                    ? mergedText(row.into)
                    : `${whole(curve!.p1.length)} points · ${endText(curve!.end)}${neutral ? ` · ${neutral}` : ""}`;
            const stall = row.status === "done" ? stallText(row.res.curve, [names[view.across], names[view.up]], sys.kind) : null;
            const retry = retryStep(c.ds);
            return (
              <li
                key={row.job.meta.key}
                className="ds-traj-row inspector-curve-row"
                data-curve-kind={kind}
                data-curve-status={row.status}
              >
                <span className="ds-traj-select inspector-branch">
                  <span className="ds-traj-line">
                    <span className="inspector-curve-swatch">
                      <CurveSample kind={kind} />
                    </span>
                    <span className="body-strong">{info.name}</span>
                    {row.status === "computing" && (
                      <span className="caption inspector-curve-tag" role="status">
                        Computing…
                      </span>
                    )}
                    {stall && <span className="caption inspector-curve-tag inspector-curve-tag--warn">Stopped early</span>}
                  </span>
                  <span className="ds-traj-meta">
                    <span className="numeric-sm">{from}</span>
                    {status && <span className={cx("caption", row.status === "failed" && "inspector-branch-error")}>{status}</span>}
                  </span>
                </span>
                <span className="ds-traj-actions">
                  <IconButton
                    size="sm"
                    label={`Remove the ${info.name.toLowerCase()} from ${info.label}`}
                    tipAlign="end"
                    onClick={() => actions.removeCurveSeed(row.job.meta.seed)}
                  >
                    <IconTrash />
                  </IconButton>
                </span>
                {stall && (
                  <div className="ds-diagnostic ds-diagnostic--block ds-diagnostic--warning inspector-stall" role="status">
                    <IconWarning className="ds-diagnostic-icon" />
                    <span className="ds-diagnostic-body">
                      <span className="ds-diagnostic-word">Warning</span>
                      <span className="ds-diagnostic-msg">{stall}</span>
                      <span className="ds-diagnostic-meta caption">
                        Δs {short(c.ds)} · stopped after {whole(curve!.p1.length)} points
                      </span>
                      <span className="ds-diagnostic-actions">
                        <Button size="sm" onClick={() => actions.setCurves({ ds: retry })}>
                          Retry with Δs = {short(retry)}
                        </Button>
                      </span>
                    </span>
                  </div>
                )}
              </li>
            );
          })}
          <HomoclinicRowItems state={state} actions={actions} />
        </ul>
      )}
    </PanelSection>
  );
}

function PlanePointsSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const view = state.view;
  if (!isCurves(view)) return null;
  const names = sys.params.map((q) => plainSubscripted(q.latex));
  const stateNames = sys.states.map((st) => plainSubscripted(st.latex));
  const rows = planePointRows(drawnCurves(state.planeRows), view.curves.ranges);
  const [a, u] = [names[view.across], names[view.up]];
  return (
    <PanelSection title="Special points" count={rows.length} open={open} onToggle={onToggle}>
      {rows.length === 0 ? (
        <p className="ds-panel-section-hint caption" data-plane-points="">
          None on these curves.
        </p>
      ) : (
        <ul className="ds-fp-list" aria-label="Special points" data-plane-points="">
          {rows.map((pt) => {
            const info = CODIM2_KINDS[pt.kind];
            const where = `${a} = ${dec(pt.p[0], 6)}  ${u} = ${dec(pt.p[1], 6)}`;
            const at = stateNames.map((n, i) => `${n} = ${dec(pt.x[i], 4)}`).join(", ");
            const located = locatedText(pt.bracket);
            return (
              <li key={`${pt.kind}-${pt.p[0]}-${pt.p[1]}`} className="ds-fp-item" data-plane-point={pt.kind}>
                <button
                  type="button"
                  className="ds-fp-row"
                  aria-label={`${info.name}: ${where}. Set both parameters to it`}
                  data-tooltip={`${info.name}: set ${a} and ${u} here`}
                  onClick={() => actions.setPlaneParams(pt.p[0], pt.p[1])}
                >
                  <FixedPointIcon kind="half" />
                  <span className="ds-fp-row-text">
                    <span className="ds-fp-class">
                      <span className="body-strong">{info.label}</span> <span className="body">{info.name}</span>
                    </span>
                    <span className="ds-fp-eig numeric-sm inspector-special-text">{where}</span>
                    <span className="ds-fp-eig numeric-sm">{at}</span>
                    {located && <span className="ds-fp-eig numeric-sm">{located}</span>}
                    <span className="caption inspector-plane-on">{onCurvesText(pt.on)}</span>
                  </span>
                </button>
                <FollowHomoclinic state={state} actions={actions} point={pt} />
              </li>
            );
          })}
        </ul>
      )}
      <p className="ds-panel-section-hint caption">
        {sys.kind === "map"
          ? "R1 to R4 strong resonances, CH Chenciner, LPPD fold-flip, GPD generalized flip."
          : "BT Bogdanov–Takens, CP cusp, GH Bautin (generalized Hopf), ZH zero-Hopf, HH Hopf–Hopf."}{" "}
        {rows.some((pt) => pt.bracket !== null) && "“Located to” is the width of the last bisection step along the curve, each parameter as a fraction of its range. "}
        Click a row to set both parameters there.
      </p>
    </PanelSection>
  );
}

function LyapunovSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const view = state.view;
  if (!isLyapunov(view)) return null;
  const map = sys.kind === "map";
  const settings = view.lyapunov;
  const k = renormalizations(settings, sys.kind);
  const validate = (field: keyof LyapunovSettings) => (v: number) => {
    const err = lyapunovSettingsError({ ...settings, [field]: v }, sys.kind);
    if (!err) return null;
    return err.field === field ? err.message : `Then the ${LYAPUNOV_FIELDS[err.field]}: ${err.message.toLowerCase()}`;
  };
  const set = (field: keyof LyapunovSettings) => (v: number) =>
    actions.setLyapunov({ [field]: map || field === "blocks" ? Math.round(v) : v });
  const whole = (v: number) => v.toLocaleString("en-US");
  const run = lyapunovFits(sys, state.shapeId, state.lyapunov) ? state.lyapunov : null;
  const res = run?.res;
  const start = lyapunovStart(state.ics, state.selection, sys.states.length);
  const startText = start
    ? sys.states.length === 1
      ? `${plainName(sys.states[0].latex)}${sub(0)} = ${dec(start.x0[0], 3)}`
      : `(${sys.states.map((st) => plainName(st.latex) + sub(0)).join(", ")}) = (${start.x0.map((v) => dec(v, 3)).join(", ")})`
    : null;
  const per = map ? "per iterate" : "per unit time";
  const traceName = map ? "⟨ln |det J|⟩" : "⟨tr J⟩";
  const stopped = res && res.status !== "completed" ? STATUS_WORDS[res.status].words.replace(/ at$/, "") : null;
  return (
    <PanelSection title="Lyapunov" open={open} onToggle={onToggle}>
      <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
        <FieldRow label="Transient" tip={map ? "Iterates run before measuring" : "Time run before measuring"}>
          <NumberField
            size="sm"
            lead={<Tex latex={map ? "N_0" : "T_0"} />}
            value={settings.transient}
            onCommit={set("transient")}
            validate={validate("transient")}
            format={map ? whole : undefined}
            ariaLabel={map ? "Transient iterates before measuring" : "Transient time before measuring"}
          />
        </FieldRow>
        <FieldRow label="Measured" tip={map ? "Iterates measured after the transient" : "Time measured after the transient"}>
          <NumberField
            size="sm"
            lead={<Tex latex={map ? "N" : "T"} />}
            value={settings.total}
            onCommit={set("total")}
            validate={validate("total")}
            format={map ? whole : undefined}
            digits={6}
            ariaLabel={map ? "Iterates measured" : "Time measured"}
          />
        </FieldRow>
        <FieldRow
          label="Renormalize"
          tip={map ? "Iterates between QR steps" : "Time between QR steps (Gram–Schmidt renormalizations)"}
        >
          <NumberField
            size="sm"
            lead={<Tex latex={map ? "k" : "\\tau"} />}
            value={settings.interval}
            onCommit={set("interval")}
            validate={validate("interval")}
            format={map ? whole : undefined}
            unit={map ? (settings.interval === 1 ? "iterate" : "iterates") : undefined}
            ariaLabel={map ? "Iterates per renormalization" : "Renormalization interval tau"}
          />
        </FieldRow>
        <FieldRow label="Blocks" tip="The run is split into this many blocks; ± is the spread of their estimates">
          <NumberField
            size="sm"
            lead={<Tex latex="B" />}
            value={settings.blocks}
            onCommit={set("blocks")}
            validate={validate("blocks")}
            step={1}
            steppers
            format={whole}
            ariaLabel="Blocks for the uncertainty"
          />
        </FieldRow>
      </dl>
      <p className="ds-panel-section-hint caption" data-lyapunov-start="">
        {startText
          ? `Follows ${state.selection?.kind === "ic" && start?.id === state.selection.id ? "the selected start point" : "the first start point"}, ${startText}; ${countText(k)} renormalizations.`
          : "Add a start point to measure its spectrum."}
      </p>
      {res && res.exponents.length > 0 && (
        <>
          <h3 className="ds-panel-section-subhead label">Exponents, {per}</h3>
          <dl className="ds-panel-section-kv lyapunov-readout" data-lyapunov-readout="">
            {res.exponents.map((v, i) => (
              <KvRow key={i} label={`λ${sub(i + 1)}`}>
                {lyapunovValueText(v, res.uncertainty[i])}
              </KvRow>
            ))}
            <KvRow label="Σλ">{withMinus(res.sum.toPrecision(6))}</KvRow>
            <KvRow label={traceName}>{withMinus(res.traceAverage.toPrecision(6))}</KvRow>
            <KvRow label="Difference">{scientificText(res.sum - res.traceAverage)}</KvRow>
          </dl>
          <p className="ds-panel-section-hint caption">
            ± is the standard error over the {whole(settings.blocks)} blocks. Σλ must equal {traceName} along the orbit;
            the difference is {map ? "rounding error" : "the integrator's error"}.
          </p>
        </>
      )}
      {stopped && res && (
        <p className="ds-panel-section-hint caption" data-lyapunov-stopped="">
          {stopped} after {whole(res.renormalizations)} of {whole(k)} renormalizations; the estimates cover those.
        </p>
      )}
    </PanelSection>
  );
}

const positive = (v: number) => (v > 0 ? null : "Must be positive");

/** MAX_SAMPLES in app/src-tauri/src/service.rs: the core takes no more per trajectory. */
const MAX_SAMPLES = 200_000;

/** The cobweb's orbits still have more than {@link COBWEB_DEDUPE_ITERATES} different segments: a chaotic orbit filling the plot, not a converged cycle. */
function cobwebFillsIn(state: StudioState): boolean {
  const p = state.portrait;
  if (!portraitFits(state.compiled, state.shapeId, p)) return false;
  const { xRange, yRange } = state.view;
  const key = `${xRange},${yRange}`;
  return p.res.trajectories.some((tr) => {
    const hit = fillCache.get(tr);
    if (hit?.key === key) return hit.fills;
    const fills = distinctSegments(orbitValues(tr), xRange, yRange, COBWEB_DEDUPE_ITERATES + 1) > COBWEB_DEDUPE_ITERATES;
    fillCache.set(tr, { key, fills });
    return fills;
  });
}

const fillCache = new WeakMap<object, { key: string; fills: boolean }>();

function energyDrift(state: StudioState): number | null {
  const p = state.portrait;
  if (!state.compiled?.hamiltonian || !portraitFits(state.compiled, state.shapeId, p)) return null;
  const drifts = p.res.trajectories.flatMap((tr) => (tr.energy ? [tr.energy.maxAbsDrift] : []));
  return drifts.length ? Math.max(...drifts) : null;
}

function SolverSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const methodHint = useId();
  const refusalHint = useId();
  const stiffHint = useId();
  const methodPicker = useId();
  const backHint = useId();
  const section = isSection(state.view);
  if (sys.kind === "map") {
    const n = Math.max(1, Math.round(state.solver.tEnd));
    const T = state.solver.transient ?? 0;
    const x = plainName(sys.states[0]?.latex ?? "x");
    // A 1-D map names its iterates (x₀ … x₁₉₉); a larger one, whose start is (x₀, y₀), counts n.
    const hidden =
      sys.states.length > 1
        ? T === 0
          ? "No iterates hidden; the orbit is drawn from n = 0."
          : `Hides n = 0${T === 1 ? "" : T === 2 ? " and 1" : ` … ${(T - 1).toLocaleString("en-US")}`}; the orbit is drawn from n = ${T.toLocaleString("en-US")}.`
        : T === 0
          ? `No iterates hidden; the orbit is drawn from ${x}₀.`
          : `Hides ${x}₀${T === 1 ? "" : T === 2 ? ` and ${x}₁` : ` … ${x}${subDigits(T - 1)}`}; the orbit is drawn from ${x}${subDigits(T)}.`;
    const fills = isCobweb(state.view) && n > COBWEB_FILL_ITERATES && cobwebFillsIn(state);
    return (
      <PanelSection title="Iteration" id="solver-settings" open={open} onToggle={onToggle}>
        <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
          <FieldRow label="Iterates" tip="Number of iterates per orbit (and of cobweb steps)">
            <NumberField
              size="sm"
              lead={<Tex latex="N" />}
              value={state.solver.tEnd}
              onCommit={(v) => actions.setSolver({ tEnd: Math.round(v) })}
              validate={(v) =>
                v >= 1 && v <= MAX_ITERATES ? null : `Must be from 1 to ${MAX_ITERATES.toLocaleString("en-US")}`
              }
              step={10}
              steppers
              ariaLabel="Number of iterates"
            />
          </FieldRow>
          <FieldRow label="Transient" tip="Iterates computed but not drawn, so the attractor stands out">
            <NumberField
              size="sm"
              lead={<Tex latex="T" />}
              value={T}
              onCommit={(v) => actions.setSolver({ transient: Math.round(v) })}
              validate={(v) =>
                Number.isInteger(v) && v >= 0 && v <= MAX_ITERATES - n
                  ? null
                  : `Must be a whole number from 0 to ${(MAX_ITERATES - n).toLocaleString("en-US")}`
              }
              step={10}
              steppers
              format={(v) => String(v)}
              ariaLabel="Transient iterates, computed but not drawn"
            />
          </FieldRow>
        </dl>
        <p className="ds-panel-section-hint caption" aria-live="polite" data-transient-hint="">
          {hidden}
        </p>
        {fills && (
          <p className="ds-panel-section-hint caption">
            Above 10,000 iterates a chaotic cobweb fills in solid; set a transient to hide the early iterates instead.
          </p>
        )}
        <p className="ds-panel-section-hint caption">Maps are iterated exactly; there is no step size to choose.</p>
      </PanelSection>
    );
  }
  const s = state.solver;
  const adaptive = METHODS[s.method].adaptive;
  const lyapunov = isLyapunov(state.view) ? state.view : null;
  const refusal = lyapunov ? LYAPUNOV_METHOD_HINT : symplecticRefusal(sys);
  const methodOptions: SelectOption<Method>[] = (Object.keys(METHODS) as Method[]).map((value) => ({
    value,
    label: METHODS[value].label,
    disabled: !methodFits(value, sys) || (!!lyapunov && !LYAPUNOV_METHODS.includes(value)),
  }));
  const drift = energyDrift(state);
  const stiff = portraitFits(sys, state.shapeId, state.portrait) && looksStiff(state.portrait.res, s.method);
  const timeOnAxis = state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time";
  return (
    <PanelSection title="Solver" id="solver-settings" open={open} onToggle={onToggle}>
      <Select
        id={methodPicker}
        ariaLabel="Integration method"
        describedBy={refusal ? `${methodHint} ${refusalHint}` : methodHint}
        value={s.method}
        options={methodOptions}
        onChange={(method) =>
          actions.setSolver({ method, h: METHODS[method].adaptive ? 0 : s.h > 0 ? s.h : DEFAULT_FIXED_STEP })
        }
      />
      <p className="ds-panel-section-hint caption" id={methodHint}>
        {state.methodNote && (
          <>
            <span data-method-note="">{state.methodNote}</span>{" "}
          </>
        )}
        {METHODS[s.method].about}
      </p>
      {refusal && (
        <p className="ds-panel-section-hint caption" id={refusalHint} data-method-refusal="">
          {refusal}
        </p>
      )}
      {stiff && (
        <>
          <p className="ds-panel-section-hint caption" id={stiffHint} data-stiff-hint="">
            {STIFF_HINT}
          </p>
          <div>
            <Button
              size="sm"
              aria-describedby={stiffHint}
              onClick={() => {
                actions.setSolver({ method: STIFF_METHOD, h: 0 });
                requestAnimationFrame(() => document.getElementById(methodPicker)?.focus());
              }}
            >
              Use {METHODS[STIFF_METHOD].label}
            </Button>
          </div>
        </>
      )}
      <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
        {adaptive ? (
          <>
            <FieldRow label="Rel. tolerance" tip="Relative tolerance">
              <NumberField
                size="sm"
                lead={<Tex latex="\mathrm{rtol}" />}
                value={s.rtol}
                onCommit={(rtol) => actions.setSolver({ rtol })}
                validate={positive}
                digits={2}
                ariaLabel="Relative tolerance rtol"
              />
            </FieldRow>
            <FieldRow label="Abs. tolerance" tip="Absolute tolerance">
              <NumberField
                size="sm"
                lead={<Tex latex="\mathrm{atol}" />}
                value={s.atol}
                onCommit={(atol) => actions.setSolver({ atol })}
                validate={positive}
                digits={2}
                ariaLabel="Absolute tolerance atol"
              />
            </FieldRow>
          </>
        ) : (
          <FieldRow label="Step size">
            <NumberField
              size="sm"
              lead={<Tex latex="h" />}
              value={s.h > 0 ? s.h : DEFAULT_FIXED_STEP}
              onCommit={(h) => actions.setSolver({ h })}
              validate={positive}
              digits={3}
              ariaLabel="Step size h"
            />
          </FieldRow>
        )}
        {lyapunov ? null : timeOnAxis ? (
          <KvRow label="Time span" text>
            <span data-tooltip="Trajectories cover exactly the visible time window">
              follows the <Tex latex="t" /> axis
              <span className="ds-sr">: trajectories cover exactly the visible time window</span>
            </span>
          </KvRow>
        ) : (
          <FieldRow label="Time span" tip="Integrate for this long from each start point">
            <NumberField
              size="sm"
              lead={<Tex latex="T" />}
              value={s.tEnd}
              onCommit={(tEnd) => actions.setSolver({ tEnd })}
              validate={positive}
              step={1}
              ariaLabel="Time span T: integrate for this long from each start point"
            />
          </FieldRow>
        )}
        {!lyapunov && (
          <FieldRow label="Samples" tip="Samples per trajectory (0 = automatic)">
            <NumberField
              size="sm"
              value={s.samples}
              onCommit={(v) => actions.setSolver({ samples: Math.round(v) })}
              validate={(v) =>
                v >= 0 && v <= MAX_SAMPLES ? null : `Must be from 0 (auto) to ${MAX_SAMPLES.toLocaleString("en-US")}`
              }
              digits={5}
              format={(v) => (v > 0 ? String(v) : "auto")}
              ariaLabel="Samples per trajectory (0 = automatic)"
            />
          </FieldRow>
        )}
        {drift !== null && (
          <KvRow label="Energy drift" text>
            <span
              data-energy-drift=""
              data-tooltip="Largest |H − H₀| over the samples of every trajectory drawn"
            >
              {energyDriftText(drift)}
            </span>
          </KvRow>
        )}
      </dl>
      {lyapunov ? (
        <p className="ds-panel-section-hint caption">
          The run integrates the orbit and its tangent vectors with this method. Step limit{" "}
          {lyapunovSolver(s, lyapunov.lyapunov, sys.kind).maxSteps.toLocaleString("en-US")}, enough for the whole run.
        </p>
      ) : (
      <>
      <div className="inspector-switch-row">
        <Switch
          row
          label="Backward time"
          checked={state.layers.backward && !section}
          disabled={section}
          describedBy={backHint}
          onChange={(backward) => actions.setLayers({ backward })}
        />
      </div>
      <p className="ds-panel-section-hint caption" id={backHint}>
        {section
          ? "A Poincaré section follows each start point forward only."
          : "Also follow each start point backward in time, drawn fainter."}{" "}
        Step limit {s.maxSteps.toLocaleString("en-US")} per run.
      </p>
      </>
      )}
    </PanelSection>
  );
}

const LYAPUNOV_METHODS: readonly Method[] = ["dopri5", "rk4", "heun", "euler"];
const LYAPUNOV_METHOD_HINT =
  "A Lyapunov run integrates the variational equations: the explicit Runge–Kutta methods only, not the symplectic or implicit ones.";

const NBSP = "\u00a0";

function eigenText(fp: FixedPointOut, sym: string): string {
  const d = (v: number) => dec(v, 3);
  const eq = `${NBSP}=${NBSP}`;
  return numberedEigenvalues(fp)
    .map((ev) => {
      const label = sym + ev.numbers.map(subscriptDigits).join(",");
      if (ev.im !== 0) {
        return Math.abs(ev.re) < 5e-4
          ? `${label}${eq}±${d(ev.im)}i`
          : `${label}${eq}${d(ev.re)}${NBSP}±${NBSP}${d(ev.im)}i`;
      }
      return `${label}${eq}${d(ev.re)}${ev.double ? `${NBSP}(double)` : ""}`;
    })
    .join(", ");
}

function FpHint({ fp, kind, dim }: { fp: FixedPointOut; kind: SystemKind | undefined; dim: number }) {
  const n = (label: string, v: number) => <span className="ds-num">{`${label} = ${short(v)}`}</span>;
  if (kind !== "map" && dim === 2 && fp.eigenvalues.length === 2) {
    const [[a, b], [c, d]] = fp.eigenvalues;
    const tau = a + c;
    const delta = a * c - b * d;
    const disc = tau * tau - 4 * delta;
    const aStab = tau < 0 ? "a stable" : "an unstable";
    const cls = fp.classification;
    let tail: ReactNode;
    if (cls === "saddle") tail = ". Δ < 0, so a saddle.";
    else if (cls === "non-isolated") tail = ". Δ = 0, so the fixed point is not isolated.";
    else if (cls === "center") tail = ". τ = 0 and Δ > 0: a center of the linearization; nonlinear terms can change that.";
    else if (cls.endsWith("star") || cls.endsWith("degenerate node"))
      tail = (
        <>
          , {n("τ² − 4Δ", 0)}: {aStab} star or degenerate node.
        </>
      );
    else
      tail = (
        <>
          , {n("τ² − 4Δ", disc)} {cls.endsWith("spiral") ? `< 0: ${aStab} spiral.` : `> 0: ${aStab} node.`}
        </>
      );
    return (
      <p className="ds-fp-hint caption">
        {n("τ", tau)}, {n("Δ", delta)}
        {tail}
      </p>
    );
  }
  const text = classificationExplain(fp.classification, kind, dim);
  return <p className="ds-fp-hint caption">{withMinus(text)}</p>;
}

function EigenvectorSwitch({
  fps,
  index,
  kind,
}: {
  fps: FixedPointOut[];
  index: number;
  kind: SystemKind | undefined;
}) {
  const reasonId = useId();
  const on = useShowEigenvectors();
  const fp = fps[index];
  const sym = kind === "map" ? "μ" : "λ";
  const real = realEigenvectors(fp, kind);
  const complex = hasComplexEigenvalue(fp);
  const rule = kind === "map" ? "in where |μ| < 1, out where |μ| > 1" : "in where λ < 0, out where λ > 0";
  const others = on
    ? othersHaveRealEigenvectors(fps, index, kind)
      ? " Other fixed points show theirs."
      : " No fixed point in this system has real eigenvectors."
    : "";
  const hint =
    real.length === 0
      ? (complex ? `Complex ${sym}, so no real eigenvectors.` : "No eigenvectors could be computed at this point.") +
        others
      : `Lines along each fixed point's real eigenvectors, arrows ${rule}.` +
        (complex ? ` Complex ${sym} have none.` : "") +
        (fp.x.length > 2 ? " Directions seen end-on in this view are left out." : "");
  return (
    <>
      <Switch
        label="Show eigenvectors"
        checked={on}
        disabled={real.length === 0 && !on}
        describedBy={reasonId}
        onChange={setShowEigenvectors}
      />
      <p className="ds-fp-hint caption" id={reasonId}>
        {hint}
      </p>
    </>
  );
}

function FixedPointsSection({
  state,
  actions,
  open,
  onToggle,
  direction,
}: SectionProps & { direction: ContinuationDirection }) {
  const sys = state.compiled!;
  const continues = continuationAvailability(sys).ok;
  const dim = sys.states.length;
  const p = state.portrait;
  const found = searchedFixedPoints(sys, p, state.shapeId);
  const fps = found ?? [];
  const sym = sys.kind === "map" ? "μ" : "λ";
  // A 1-D map's one multiplier is λ = f′(x*), in Strogatz §10.1's notation.
  const map1 = sys.kind === "map" && dim === 1;
  const multiplierText = (fp: FixedPointOut) => {
    const lambda = mapMultiplier(fp);
    const x = plainName(sys.states[0].latex);
    // One clause per line at most: "|" may break after it (class BA), so each is kept whole.
    if (!Number.isFinite(lambda)) return <span className="nowrap">{`f′(${x}*) does not exist`}</span>;
    return (
      <>
        <span className="nowrap">{`λ${NBSP}=${NBSP}f′(${x}*)${NBSP}=${NBSP}${dec(lambda, 3)},`}</span>{" "}
        <span className="nowrap">{`|λ|${NBSP}=${NBSP}${dec(Math.abs(lambda), 3)}`}</span>
      </>
    );
  };
  const selected = p && fps.length ? selectedFixedPointIndex(state.selection, fps, p.meta.view) : null;
  const coordsText = (fp: FixedPointOut) =>
    dim === 1 ? `${plainName(sys.states[0].latex)}* = ${dec(fp.x[0], 3)}` : `(${fp.x.map((v) => dec(v, 3)).join(", ")})`;

  let empty: string | null = null;
  if (isSection(state.view)) empty = "Not searched: a Poincaré section shows only the crossings.";
  else if (!state.layers.fixedPoints) empty = "Turn on Fixed points in Layers to find them.";
  else if (!sys.autonomous)
    empty =
      sys.kind === "map"
        ? "Not searched: the map changes with n, so fixed points would move."
        : "Not searched: the field changes with t, so fixed points would move.";
  else if (found === null) empty = state.computeError ? "Not searched: the plot could not be computed." : "Searching…";
  else if (fps.length === 0) empty = "No fixed points in view.";

  return (
    <PanelSection
      title="Fixed points"
      count={empty ? undefined : fps.length}
      countSr="found"
      open={open}
      onToggle={onToggle}
      flush
    >
      {empty ? (
        <p className="ds-panel-section-hint caption inspector-inset-text">{empty}</p>
      ) : (
        <>
          <ul className="ds-fp-list" aria-label="Fixed points">
            {fps.map((fp, i) => {
              const isSel = selected === i;
              const label = classificationTitle(fp.classification, dim);
              const coords = coordsText(fp);
              return (
                <li key={i} className={cx("ds-fp-item", isSel && "is-selected")} data-fp-index={i}>
                  <button
                    type="button"
                    className="ds-fp-row"
                    aria-pressed={isSel}
                    aria-label={`${label} at ${coords}`}
                    onClick={() => actions.select(isSel ? null : selectFixedPoint(fp.x))}
                  >
                    <FixedPointIcon kind={fixedPointGlyph(fp.classification, fp.stable)} />
                    <span className="ds-fp-row-text">
                      <span className="ds-fp-class body-strong">{label}</span>
                      <span className="ds-fp-coords numeric">{coords}</span>
                      <span className="ds-fp-eig numeric-sm">{map1 ? multiplierText(fp) : eigenText(fp, sym)}</span>
                    </span>
                  </button>
                  {isSel && (
                    <div className="ds-fp-detail">
                      {map1 ? (
                        <p className="ds-fp-hint caption">
                          {withMinus(mapStabilityHint(fp, plainName(sys.states[0].latex)))}
                        </p>
                      ) : (
                        <FpHint fp={fp} kind={sys.kind} dim={dim} />
                      )}
                      {dim >= 2 && <EigenvectorSwitch fps={fps} index={i} kind={sys.kind} />}
                      {continues && (
                        <Button
                          size="sm"
                          data-fp-continue=""
                          data-tooltip="Follow it as a parameter changes, in the Branch view"
                          onClick={() => actions.continueFrom(fp.x, direction)}
                        >
                          Continue
                        </Button>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
          <p className="ds-panel-section-hint caption inspector-inset-text">Searched for in the visible region only.</p>
        </>
      )}
    </PanelSection>
  );
}

interface Run {
  out: TrajectoryOut;
  direction: "forward" | "backward";
}

function runsOf(state: StudioState, ic: InitialCondition): Run[] {
  const sys = state.compiled;
  const p = state.portrait;
  const runs: Run[] = [];
  if (!portraitFits(sys, state.shapeId, p)) return runs;
  p.meta.trajectories.forEach((m, i) => {
    const out = p.res.trajectories[i];
    if (m.icId === ic.id && out) runs.push({ out, direction: m.direction });
  });
  return runs;
}

const swatchStyle = (ic: InitialCondition) => ({ ["--traj" as string]: `var(--traj-${(ic.color % PALETTE_SIZE) + 1})` });

function RunStatus({ runs, timeName }: { runs: Run[]; timeName: string }) {
  if (runs.length === 0) return null;
  const bad = runs.filter((r) => r.out.status !== "completed");
  if (bad.length === 0) return <span className="ds-traj-status caption">Completed</span>;
  return (
    <>
      {bad.map(({ out, direction }) => {
        const info = stopText(out, timeName === "n")!;
        return (
          <span
            key={direction}
            className={cx("ds-traj-status caption", info.stopped && "ds-traj-status--stopped")}
          >
            {info.stopped && <IconWarning />}
            {info.words} <span className="ds-num">{`${timeName} = ${atTime(info.at)}`}</span>
          </span>
        );
      })}
    </>
  );
}

function TrajectoriesSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const isMap = sys.kind === "map";
  const timeName = isMap ? "n" : "t";
  const selectedId = state.selection?.kind === "ic" ? state.selection.id : null;
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    if (selectedId === null || !open) return;
    listRef.current
      ?.querySelector<HTMLElement>(`[data-ic-id="${selectedId}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selectedId, open]);

  const addAtCenter = () => {
    const v = state.view;
    actions.addIc((v.xRange[0] + v.xRange[1]) / 2, (v.yRange[0] + v.yRange[1]) / 2);
  };

  return (
    <PanelSection
      title="Trajectories"
      count={state.ics.length}
      open={open}
      onToggle={onToggle}
      flush
      actions={
        <>
          <IconButton
            size="sm"
            label="Add trajectory"
            tip="Add trajectory (or click the plot to place its start)"
            tipAlign="end"
            data-traj-add=""
            onClick={addAtCenter}
          >
            <IconTrajectoryAdd />
          </IconButton>
          {state.ics.length > 0 && (
            <IconButton
              size="sm"
              label="Delete all trajectories"
              tipAlign="end"
              onClick={() => {
                actions.clearIcs();
                focusAfterTrajectoryDelete(".panel-right", [], -1);
              }}
            >
              <IconTrash />
            </IconButton>
          )}
        </>
      }
    >
      {state.ics.length === 0 ? (
        <div className="ds-traj-empty">
          <IconTrajectoryAdd />
          <p className="ds-traj-empty-text body">Click the plot to start a trajectory.</p>
          <p className="ds-traj-empty-hint caption">Drag a start point to move it.</p>
        </div>
      ) : (
        <ul className="ds-traj-list" ref={listRef} aria-label="Trajectories">
          {state.ics.map((ic, k) => {
            const n = k + 1;
            const isSel = ic.id === selectedId;
            const hidden = ic.hidden === true;
            const start = startPointParts(
              (state.compiled?.states ?? []).map((st) => st.latex),
              ic.x0,
              null,
            );
            const spans = trajectorySpans(sys.kind, state.view, ic, state.solver, state.layers);
            const ends = spans.flatMap((s) => [s.req.t0, s.req.t1]);
            // A map's orbit is drawn from n = T, after the transient.
            const skip = isMap ? (state.solver.transient ?? 0) : 0;
            const lo = (ends.length ? Math.min(...ends) : ic.t0) + skip;
            const hi = (ends.length ? Math.max(...ends) : ic.t0) + skip;
            return (
              <li
                key={ic.id}
                className={cx("ds-traj-row", isSel && "is-selected", hidden && "is-hidden")}
                style={swatchStyle(ic)}
                data-ic-id={ic.id}
              >
                <button
                  type="button"
                  className="ds-traj-select"
                  aria-pressed={isSel}
                  aria-label={`Trajectory ${n}, start ${start.value}`}
                  onClick={() => actions.select(isSel ? null : { kind: "ic", id: ic.id })}
                >
                  <span className="ds-traj-line">
                    <span className="ds-traj-swatch" aria-hidden="true" />
                    <span className="ds-traj-ic numeric">
                      <span className="ds-traj-ic-name numeric-sm">
                        <Tex latex={start.nameLatex} />
                      </span>
                      <span className="ds-traj-ic-value">{start.value}</span>
                    </span>
                  </span>
                  <span className="ds-traj-meta">
                    <span className="ds-traj-span numeric-sm">{`${timeName} ∈ [${short(lo)}, ${short(hi)}]`}</span>
                    {hidden ? (
                      <span className="ds-traj-status caption">Hidden</span>
                    ) : (
                      <RunStatus runs={runsOf(state, ic)} timeName={timeName} />
                    )}
                  </span>
                </button>
                <span className="ds-traj-actions">
                  <IconButton
                    size="sm"
                    label={`Show trajectory ${n}`}
                    tip={hidden ? "Show" : "Hide"}
                    tipAlign="end"
                    pressed={!hidden}
                    onClick={() => actions.setIcHidden(ic.id, !hidden)}
                  >
                    {hidden ? <IconEyeOff /> : <IconEye />}
                  </IconButton>
                  <IconButton
                    size="sm"
                    label={`Delete trajectory ${n}`}
                    tip="Delete trajectory"
                    tipAlign="end"
                    onClick={() => {
                      focusAfterTrajectoryDelete(".panel-right", state.ics.map((i) => i.id), ic.id);
                      actions.removeIcs([ic.id]);
                    }}
                  >
                    <IconTrash />
                  </IconButton>
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </PanelSection>
  );
}

function SelectedTrajectorySection({ state, actions, id, open, onToggle }: SectionProps & { id: number }) {
  const sys = state.compiled!;
  const ic = state.ics.find((i) => i.id === id);
  if (!ic) return null;
  const isMap = sys.kind === "map";
  const n = state.ics.indexOf(ic) + 1;
  const runs = runsOf(state, ic);
  const dim = sys.states.length;
  return (
    <PanelSection
      title={`Trajectory ${n}`}
      lead={<span className="ds-traj-swatch" style={swatchStyle(ic)} aria-hidden="true" />}
      open={open}
      onToggle={onToggle}
      actions={
        <IconButton size="sm" label="Deselect" tip="Deselect (or press Escape on the plot)" tipAlign="end" onClick={() => actions.select(null)}>
          <IconClose />
        </IconButton>
      }
    >
      <h3 className="ds-panel-section-subhead label">Start point</h3>
      <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
        {!isMap && (
          <FieldRow label="Start time">
            <NumberField
              size="sm"
              lead={<Tex latex="t_0" />}
              value={ic.t0}
              onCommit={(t0) => actions.updateIc(ic.id, ic.x0, t0)}
              step={0.1}
              digits={5}
              ariaLabel="Start time t0"
            />
          </FieldRow>
        )}
        {sys.states.map((s, i) => (
          <FieldRow key={s.name} label={i === 0 ? "Start state" : ""}>
            <NumberField
              size="sm"
              lead={<Tex latex={withSubscript(s.latex, "0")} />}
              value={ic.x0[i]}
              onCommit={(v) =>
                actions.updateIc(
                  ic.id,
                  ic.x0.map((x, k) => (k === i ? v : x)),
                  ic.t0,
                )
              }
              step={0.1}
              digits={5}
              ariaLabel={`Initial ${s.name}`}
            />
          </FieldRow>
        ))}
      </dl>
      {runs.map(({ out, direction }) => {
        const len = out.t.length;
        const tEnd = len ? out.t[len - 1] : NaN;
        const last = len ? out.x.slice((len - 1) * dim, len * dim) : [];
        const info = stopText(out, isMap);
        const landed = isMap
          ? landedOnSource(out, dim, state.portrait?.res.fixedPoints ?? [], plainName(sys.states[0]?.latex ?? "x"))
          : null;
        return (
          <div className="inspector-run" key={direction}>
            <h3 className="ds-panel-section-subhead label">
              {isMap ? "Orbit" : direction === "forward" ? "Forward in time" : "Backward in time"}
            </h3>
            <dl className="ds-panel-section-kv">
              <KvRow label={isMap ? "Iterates" : "Accepted steps"}>{out.stats.accepted.toLocaleString("en-US")}</KvRow>
              {!isMap && <KvRow label="Rejected steps">{out.stats.rejected.toLocaleString("en-US")}</KvRow>}
              <KvRow label="Function evaluations">{out.stats.evals.toLocaleString("en-US")}</KvRow>
              {len > 0 && (
                <KvRow label="Ends at" text>
                  <Tex latex={`${isMap ? "n" : "t"} = ${texMinus(atTime(tEnd))}`} />
                </KvRow>
              )}
              {last.length > 0 && (
                <KvRow label="End state">
                  {last.length === 1 ? short(last[0]) : `(${last.map((v) => short(v)).join(", ")})`}
                </KvRow>
              )}
              <KvRow label="Status" text>
                {info ? (
                  <span
                    className={cx("ds-traj-status", info.stopped && "ds-traj-status--stopped")}
                    data-tooltip={out.status === "blewUp" ? "|x| passed 10¹², usually the maths, not the solver" : undefined}
                  >
                    {info.stopped && <IconWarning />}
                    {info.words} <span className="ds-num">{`${isMap ? "n" : "t"} = ${atTime(info.at)}`}</span>
                    {out.status === "blewUp" && <span className="ds-sr">: |x| passed 10¹², usually the maths, not the solver</span>}
                  </span>
                ) : (
                  "Completed"
                )}
              </KvRow>
            </dl>
            {landed && (
              <p className="ds-panel-section-hint caption" data-rounding-hint="">
                From <span className="ds-num">{`n = ${landed.n.toLocaleString("en-US")}`}</span> the orbit sits exactly on
                the unstable fixed point <span className="ds-num">{landed.at}</span>. That is rounding, not the dynamics: a
                map that stretches by 2 each step, like the tent map at r = 2, uses up a double&apos;s 53 bits in about 53
                steps, and the orbit can then land on the fixed point exactly and stay.
              </p>
            )}
          </div>
        );
      })}
      <div>
        <Button
          variant="ghost-danger"
          icon={<IconTrash />}
          onClick={() => {
            focusAfterTrajectoryDelete(".panel-right", state.ics.map((i) => i.id), ic.id);
            actions.removeIcs([ic.id]);
          }}
        >
          Delete trajectory
        </Button>
      </div>
    </PanelSection>
  );
}
