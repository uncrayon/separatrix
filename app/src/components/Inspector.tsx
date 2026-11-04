import { useEffect, useId, useRef, type ReactNode } from "react";
import type { AxisRef, FixedPointOut, Method, Status, SystemKind, TrajectoryOut } from "../api/types";
import { Tex } from "../math/Tex";
import {
  classificationExplain,
  classificationTitle,
  COMPUTED_ZERO,
  fmt,
  latexToPlain,
  sameAxis,
  withSubscript,
} from "../math/format";
import type { StudioActions, StudioState } from "../state/useStudio";
import {
  DEFAULT_FIXED_STEP,
  PALETTE_SIZE,
  selectFixedPoint,
  selectedFixedPointIndex,
  trajectorySpans,
  type InitialCondition,
} from "../state/model";
import { Button, IconButton, NumberField, Select, Switch, withMinus, type SelectOption } from "./controls";
import { PanelSection, useSections } from "./PanelSection";
import { focusAfterTrajectoryDelete } from "./focus";
import { dec, startPointParts } from "./startPoint";
import { fixedPointGlyph } from "../canvas/draw";
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

type SectionKey = "layers" | "axes" | "solver" | "fixedPoints" | "trajectories" | "selected";
const SECTIONS_KEY = "separatrix.inspector.sections";

export function Inspector({ state, actions }: Props) {
  const sys = state.compiled;
  const [open, setOpen] = useSections<SectionKey>(SECTIONS_KEY, {
    layers: true,
    axes: true,
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
          <AxesSection state={state} actions={actions} open={open.axes} onToggle={(v) => setOpen("axes", v)} />
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
  const L = state.layers;
  const mapHint = useId();
  return (
    <PanelSection title="Layers" open={open} onToggle={onToggle} flush>
      <Switch
        row
        icon={<IconField />}
        label="Direction field"
        checked={L.field && !isMap}
        disabled={isMap}
        describedBy={isMap ? mapHint : undefined}
        title="Direction field (arrows of normalized length)"
        onChange={(field) => actions.setLayers({ field })}
      />
      <Switch
        row
        icon={<IconNullcline />}
        label="Nullclines"
        checked={L.nullclines && !isMap}
        disabled={isMap}
        describedBy={isMap ? mapHint : undefined}
        title="Nullclines (where one component of the field vanishes)"
        onChange={(nullclines) => actions.setLayers({ nullclines })}
      />
      <Switch
        row
        icon={<IconFixedPoint />}
        label="Fixed points"
        checked={L.fixedPoints}
        title="Fixed points (filled when stable, open when unstable)"
        onChange={(fixedPoints) => actions.setLayers({ fixedPoints })}
      />
      <div className="inspector-inset">
        {isMap ? (
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

function AxesSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const kind = sys.kind;
  const view = state.view;
  const options: SelectOption<string>[] = [
    ...sys.states.map((s, i) => ({ value: `s${i}`, label: optionName(s.latex) })),
    { value: "time", label: kind === "map" ? "n" : "t" },
  ];
  const sliceStates =
    kind === "ode"
      ? sys.states
          .map((s, i) => ({ s, i }))
          .filter(({ i }) => !sameAxis(view.xAxis, { kind: "state", index: i }) && !sameAxis(view.yAxis, { kind: "state", index: i }))
      : [];
  const timeSlice = kind === "ode" && !sys.autonomous && view.xAxis.kind !== "time" && view.yAxis.kind !== "time";
  const hId = useId();
  const vId = useId();
  return (
    <PanelSection title="Axes" open={open} onToggle={onToggle}>
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
              options={options}
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
              options={options}
              onChange={(v) => actions.setAxis("y", axisFromValue(v))}
            />
          </dd>
        </div>
      </dl>
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

const METHODS: (SelectOption<Method> & { about: string })[] = [
  {
    value: "dopri5",
    label: "Dormand–Prince 5(4)",
    about: "Dormand–Prince 5(4): adaptive step with error control.",
  },
  { value: "rk4", label: "RK4 · fixed step", about: "Classical Runge–Kutta: fourth order, fixed step." },
  { value: "heun", label: "Heun", about: "Heun (improved Euler): second order, fixed step." },
  { value: "euler", label: "Euler", about: "Forward Euler: first order, fixed step." },
];

const positive = (v: number) => (v > 0 ? null : "Must be positive");

function SolverSection({ state, actions, open, onToggle }: SectionProps) {
  const sys = state.compiled!;
  const methodHint = useId();
  const backHint = useId();
  if (sys.kind === "map") {
    return (
      <PanelSection title="Iteration" id="solver-settings" open={open} onToggle={onToggle}>
        <dl className="ds-panel-section-kv ds-panel-section-kv--fields">
          <FieldRow label="Iterates" tip="Number of iterates per orbit">
            <NumberField
              size="sm"
              lead={<Tex latex="N" />}
              value={state.solver.tEnd}
              onCommit={(v) => actions.setSolver({ tEnd: Math.round(v) })}
              validate={(v) => (v >= 1 ? null : "Must be at least 1")}
              step={10}
              ariaLabel="Number of iterates"
            />
          </FieldRow>
        </dl>
        <p className="ds-panel-section-hint caption">Maps are iterated exactly; there is no step size to choose.</p>
      </PanelSection>
    );
  }
  const s = state.solver;
  const adaptive = s.method === "dopri5";
  const timeOnAxis = state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time";
  return (
    <PanelSection title="Solver" id="solver-settings" open={open} onToggle={onToggle}>
      <Select
        ariaLabel="Integration method"
        describedBy={methodHint}
        value={s.method}
        options={METHODS}
        onChange={(method) =>
          actions.setSolver({ method, h: method === "dopri5" ? 0 : s.h > 0 ? s.h : DEFAULT_FIXED_STEP })
        }
      />
      <p className="ds-panel-section-hint caption" id={methodHint}>
        {METHODS.find((m) => m.value === s.method)?.about}
      </p>
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
            validate={(v) => (v >= 0 ? null : "Must be 0 (auto) or more")}
            digits={5}
            format={(v) => (v > 0 ? String(v) : "auto")}
            ariaLabel="Samples per trajectory (0 = automatic)"
          />
        </FieldRow>
      </dl>
      <div className="inspector-switch-row">
        <Switch
          row
          label="Backward time"
          checked={state.layers.backward}
          describedBy={backHint}
          onChange={(backward) => actions.setLayers({ backward })}
        />
      </div>
      <p className="ds-panel-section-hint caption" id={backHint}>
        Also follow each start point backward in time, drawn fainter. Step limit{" "}
        {s.maxSteps.toLocaleString("en-US")} per run.
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
  // A portrait asked for while the layer was off searched nothing: an empty list there means "not yet", not "none".
  const searched = !!p && p.meta.systemId === sys.systemId && p.meta.fixedPoints;
  const fps = searched ? p.res.fixedPoints : [];
  const sym = sys.kind === "map" ? "μ" : "λ";
  const selected = p && fps.length ? selectedFixedPointIndex(state.selection, fps, p.meta.view) : null;
  const coordsText = (fp: FixedPointOut) =>
    dim === 1 ? `${plainName(sys.states[0].latex)}* = ${dec(fp.x[0], 3)}` : `(${fp.x.map((v) => dec(v, 3)).join(", ")})`;

  let empty: string | null = null;
  if (!state.layers.fixedPoints) empty = "Turn on Fixed points in Layers to find them.";
  else if (!sys.autonomous)
    empty =
      sys.kind === "map"
        ? "Not searched: the map changes with n, so fixed points would move."
        : "Not searched: the field changes with t, so fixed points would move.";
  else if (!searched) empty = "Searching…";
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
                      <span className="ds-fp-eig numeric-sm">{eigenText(fp, sym)}</span>
                    </span>
                  </button>
                  {isSel && (
                    <div className="ds-fp-detail">
                      <FpHint fp={fp} kind={sys.kind} dim={dim} />
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

const STATUS_WORDS: Record<Exclude<Status, "completed">, { words: string; stopped: boolean }> = {
  blewUp: { words: "Blew up at", stopped: false },
  maxSteps: { words: "Step limit at", stopped: true },
  stepTooSmall: { words: "Step too small at", stopped: true },
  nonFinite: { words: "NaN at", stopped: true },
};

interface Run {
  out: TrajectoryOut;
  direction: "forward" | "backward";
}

function runsOf(state: StudioState, ic: InitialCondition): Run[] {
  const sys = state.compiled;
  const p = state.portrait;
  const runs: Run[] = [];
  if (!sys || !p || p.meta.systemId !== sys.systemId) return runs;
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
        const info = STATUS_WORDS[out.status as Exclude<Status, "completed">];
        const tEnd = out.t.length ? out.t[out.t.length - 1] : NaN;
        return (
          <span
            key={direction}
            className={cx("ds-traj-status caption", info.stopped && "ds-traj-status--stopped")}
          >
            {info.stopped && <IconWarning />}
            {info.words} <span className="ds-num">{`${timeName} = ${atTime(tEnd)}`}</span>
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
            const lo = ends.length ? Math.min(...ends) : ic.t0;
            const hi = ends.length ? Math.max(...ends) : ic.t0;
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
        const info = out.status === "completed" ? null : STATUS_WORDS[out.status];
        return (
          <div className="inspector-run" key={direction}>
            <h3 className="ds-panel-section-subhead label">
              {isMap ? "Orbit" : direction === "forward" ? "Forward in time" : "Backward in time"}
            </h3>
            <dl className="ds-panel-section-kv">
              <KvRow label={isMap ? "Iterates" : "Accepted steps"}>{out.stats.accepted.toLocaleString("en-US")}</KvRow>
              {!isMap && <KvRow label="Rejected steps">{out.stats.rejected.toLocaleString("en-US")}</KvRow>}
              <KvRow label="Function evaluations">{out.stats.evals.toLocaleString("en-US")}</KvRow>
              <KvRow label="Ends at" text>
                <Tex latex={`${isMap ? "n" : "t"} = ${atTime(tEnd).replace("−", "-")}`} />
              </KvRow>
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
                    {info.words} <span className="ds-num">{`${isMap ? "n" : "t"} = ${atTime(tEnd)}`}</span>
                    {out.status === "blewUp" && <span className="ds-sr">: |x| passed 10¹², usually the maths, not the solver</span>}
                  </span>
                ) : (
                  "Completed"
                )}
              </KvRow>
            </dl>
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
