import { useEffect, useId, useRef, type ReactNode } from "react";
import type { AxisRef, Direction, FixedPointOut, Method, SectionSpec, SystemKind, TrajectoryOut } from "../api/types";
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
  mapStabilityHint,
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
  MAX_ITERATES,
  PALETTE_SIZE,
  VIEW_CHOICE_LABELS,
  VIEW_CHOICE_SHORT,
  isCobweb,
  isReturnMap,
  isSection,
  isSpace,
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
import { landedOnSource, looksStiff, stopText } from "./runStatus";
import { fixedPointGlyph } from "../canvas/draw";
import { COBWEB_DEDUPE_ITERATES, distinctSegments, orbitValues } from "../canvas/cobweb";
import { FixedPointIcon } from "../canvas/PortraitCanvas";
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

type SectionKey = "layers" | "axes" | "section" | "solver" | "fixedPoints" | "trajectories" | "selected";
const SECTIONS_KEY = "separatrix.inspector.sections";

export function Inspector({ state, actions }: Props) {
  const sys = state.compiled;
  const [open, setOpen] = useSections<SectionKey>(SECTIONS_KEY, {
    layers: true,
    axes: true,
    section: true,
    solver: true,
    fixedPoints: true,
    trajectories: state.ics.length <= 5,
    selected: true,
  });

  const sel = state.selection;
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
          <LayersSection state={state} actions={actions} open={open.layers} onToggle={(v) => setOpen("layers", v)} />
          <ViewSection state={state} actions={actions} open={open.axes} onToggle={(v) => setOpen("axes", v)} />
          {isSection(state.view) && (
            <PoincareSection
              state={state}
              actions={actions}
              open={open.section}
              onToggle={(v) => setOpen("section", v)}
            />
          )}
          <SolverSection state={state} actions={actions} open={open.solver} onToggle={(v) => setOpen("solver", v)} />
          <FixedPointsSection
            state={state}
            actions={actions}
            open={open.fixedPoints}
            onToggle={(v) => setOpen("fixedPoints", v)}
          />
          <TrajectoriesSection
            state={state}
            actions={actions}
            open={open.trajectories}
            onToggle={(v) => setOpen("trajectories", v)}
          />
          {sel?.kind === "ic" && (
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
  const view = state.view;
  const cobweb = isCobweb(view);
  const section = isSection(view);
  const space = isSpace(view) ? view : null;
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
  const names = viewAxisLabels(view, sys.states, kind);
  const options: SelectOption<string>[] = [
    ...sys.states.map((s, i) => ({ value: `s${i}`, label: optionName(s.latex) })),
    ...(section ? [] : [{ value: "time", label: kind === "map" ? "n" : "t" }]),
  ];
  // A return map's axes are one state at successive crossings, named xₖ and xₖ₊₁ as on the plot.
  const named = (axis: AxisRef, step: string) =>
    isReturnMap(view) ? options.map((o) => (o.value === axisValue(axis) ? { ...o, label: `${o.label}${step}` } : o)) : options;
  const sliceStates =
    kind === "ode" && !section && !space
      ? sys.states
          .map((s, i) => ({ s, i }))
          .filter(({ i }) => !sameAxis(view.xAxis, { kind: "state", index: i }) && !sameAxis(view.yAxis, { kind: "state", index: i }))
      : [];
  const timeSlice =
    kind === "ode" && !section && !space && !sys.autonomous && view.xAxis.kind !== "time" && view.yAxis.kind !== "time";
  const hId = useId();
  const vId = useId();
  return (
    <PanelSection title="View" open={open} onToggle={onToggle}>
      <div className="inspector-view-control">
        <SegmentedControl
          size="sm"
          fill
          ariaLabel="View"
          value={viewChoice(view)}
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
      {space ? (
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

/** The largest energy drift over the drawn trajectories of a Hamiltonian flow; null when there is none to show. */
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
  const refusal = symplecticRefusal(sys);
  const methodOptions: SelectOption<Method>[] = (Object.keys(METHODS) as Method[]).map((value) => ({
    value,
    label: METHODS[value].label,
    disabled: !methodFits(value, sys),
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
                // The button goes with the hint; keep focus on the method it chose.
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
        {timeOnAxis ? (
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
    </PanelSection>
  );
}

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

function FixedPointsSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
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
