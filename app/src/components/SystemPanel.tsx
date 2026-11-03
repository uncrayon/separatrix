// Left panel: the system as the user wrote it, what we inferred from it,
// its parameters and its initial conditions.

import type { Diagnostic } from "../api/types";
import { MathField } from "../math/MathField";
import { Tex } from "../math/Tex";
import { firstOrderDisplay, fmtTex, latexToPlain } from "../math/format";
import type { StudioActions, StudioState } from "../state/useStudio";
import type { InitialCondition, ParamState } from "../state/model";
import { NumberField, Slider } from "./controls";
import { IconClose, IconError, IconInfo, IconPlus, IconWarning } from "./icons";

interface Props {
  state: StudioState;
  actions: StudioActions;
}

export function SystemPanel({ state, actions }: Props) {
  return (
    <aside className="panel panel-left" aria-label="System">
      <EquationsSection state={state} actions={actions} />
      <InferenceSection state={state} />
      <ParamsSection params={state.params} actions={actions} />
      <IcSection state={state} actions={actions} />
    </aside>
  );
}

/**
 * Which equation line a diagnostic points at. Spans are byte offsets into the
 * compiled source: the non-empty lines joined with "\n". Returns -1 if unknown.
 */
function diagnosticLine(d: Diagnostic, lines: string[]): number {
  if (!d.span) return -1;
  const enc = new TextEncoder();
  let offset = 0;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim() === "") continue;
    const end = offset + enc.encode(lines[i]).length;
    if (d.span[0] <= end) return i;
    offset = end + 1;
  }
  return -1;
}

function EquationsSection({ state, actions }: Props) {
  const latexes = state.equations.map((l) => l.latex);
  const located = state.diagnostics.map((d) => ({ d, line: diagnosticLine(d, latexes) }));
  const general = located.filter((x) => x.line < 0).map((x) => x.d);
  return (
    <section className="section">
      <header className="section-head">
        <h2>Equations</h2>
        <span className="section-hint">one per line</span>
      </header>
      <div className="equations">
        {state.equations.map((line, i) => {
          const here = located.filter((x) => x.line === i).map((x) => x.d);
          return (
            <div className="equation-block" key={line.id}>
              <div className="equation-row">
                <MathField
                  value={line.latex}
                  onChange={(latex) => actions.setEquation(line.id, latex)}
                  onEnter={() => actions.addEquation(line.id)}
                  placeholder={i === 0 ? "\\dot{x}=" : ""}
                  ariaLabel={`Equation ${i + 1}`}
                  autoFocus={line.latex === ""}
                  invalid={here.some((d) => d.severity === "error")}
                />
                {state.equations.length > 1 && (
                  <button
                    type="button"
                    className="icon-btn"
                    onClick={() => actions.removeEquation(line.id)}
                    aria-label={`Remove equation ${i + 1}`}
                    title="Remove this equation"
                  >
                    <IconClose size={14} />
                  </button>
                )}
              </div>
              <Diagnostics list={here} />
            </div>
          );
        })}
      </div>
      <button type="button" className="btn ghost small add-line" onClick={() => actions.addEquation()}>
        <IconPlus size={14} /> Add equation
      </button>
      <Diagnostics list={general} />
    </section>
  );
}

function Diagnostics({ list }: { list: Diagnostic[] }) {
  if (!list.length) return null;
  return (
    <ul className="diagnostics" aria-live="polite">
      {list.map((d, i) => (
        <li key={i} className={"diag " + d.severity}>
          {d.severity === "error" ? (
            <IconError size={14} />
          ) : d.severity === "warning" ? (
            <IconWarning size={14} />
          ) : (
            <IconInfo size={14} />
          )}
          <span>{latexToPlain(d.message)}</span>
        </li>
      ))}
    </ul>
  );
}

function InferenceSection({ state }: { state: StudioState }) {
  const sys = state.compiled;
  if (!sys) return null;
  const type = sys.hamiltonian ? "Hamiltonian" : sys.kind === "map" ? "Map" : "ODE";
  const typeTitle = sys.hamiltonian
    ? "Hamiltonian flow: q̇ = ∂H/∂p, ṗ = −∂H/∂q"
    : sys.kind === "map"
      ? "Discrete map: iterated x ↦ f(x)"
      : "Ordinary differential equation (a flow)";
  return (
    <section className="section">
      <header className="section-head">
        <h2>We read this as</h2>
      </header>
      <div className="chips">
        <span className="chip badge-type" title={typeTitle}>
          {type}
        </span>
        <span className="chip static">{sys.states.length}D</span>
        {sys.kind !== "map" && (
          <span
            className="chip static"
            title={
              sys.autonomous
                ? "Time t does not appear on the right-hand side"
                : "Time t appears explicitly on the right-hand side"
            }
          >
            {sys.autonomous ? "autonomous" : "non-autonomous"}
          </span>
        )}
      </div>
      <dl className="inference">
        <dt>States</dt>
        <dd className="chips">
          {sys.states.map((s) => (
            <span key={s.name} className="chip math">
              <Tex latex={s.latex} />
            </span>
          ))}
        </dd>
        <dt>Parameters</dt>
        <dd className="chips">
          {state.params.length === 0 && <span className="muted">none</span>}
          {state.params.map((p) => (
            <a key={p.name} className="chip math link" href={`#param-${p.name}`}>
              <Tex latex={`${p.latex}=${fmtTex(p.value, 3)}`} />
            </a>
          ))}
        </dd>
      </dl>
      {(sys.hamiltonian ||
        sys.equationsLatex.length !== state.equations.filter((e) => e.latex.trim()).length) && (
        <div className="first-order" title="The first-order system that is actually solved">
          <div className="first-order-label">Solved as the first-order system</div>
          {firstOrderDisplay(sys.equationsLatex, sys.states, sys.kind).map((e, i) => (
            <div key={i}>
              <Tex latex={e} />
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function ParamsSection({ params, actions }: { params: ParamState[]; actions: StudioActions }) {
  if (!params.length) return null;
  return (
    <section className="section">
      <header className="section-head">
        <h2>Parameters</h2>
        <span className="section-hint">drag to explore</span>
      </header>
      <div className="params">
        {params.map((p) => (
          <div className="param" key={p.name} id={`param-${p.name}`}>
            <div className="param-top">
              <Tex latex={p.latex} className="param-name" />
              <Slider
                value={p.value}
                min={p.min}
                max={p.max}
                onChange={(v) => actions.setParam(p.name, v)}
                ariaLabel={`Parameter ${p.name}`}
              />
              <NumberField
                value={p.value}
                onCommit={(v) => actions.setParam(p.name, v)}
                digits={4}
                step={(p.max - p.min) / 100}
                ariaLabel={`Value of ${p.name}`}
                className="param-value"
              />
            </div>
            <div className="param-range">
              <NumberField
                value={p.min}
                onCommit={(v) => actions.setParamRange(p.name, v, p.max)}
                digits={3}
                ariaLabel={`Minimum of ${p.name}`}
                className="range-num"
              />
              <NumberField
                value={p.max}
                onCommit={(v) => actions.setParamRange(p.name, p.min, v)}
                digits={3}
                ariaLabel={`Maximum of ${p.name}`}
                className="range-num right"
              />
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function icLatex(ic: InitialCondition, state: StudioState): string {
  const sys = state.compiled;
  if (!sys) return "";
  const names = sys.states.map((s) => s.latex);
  const vals = ic.x0.map((v) => fmtTex(v, 3));
  const withTime =
    sys.kind === "ode" &&
    (!sys.autonomous || state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time");
  const sub = (n: string) => `${n}_0`;
  if (names.length === 1 && !withTime) return `${sub(names[0])} = ${vals[0]}`;
  const allNames = [...(withTime ? ["t_0"] : []), ...names.map(sub)];
  const allVals = [...(withTime ? [fmtTex(ic.t0, 3)] : []), ...vals];
  return `(${allNames.join(", ")}) = (${allVals.join(", ")})`;
}

function IcSection({ state, actions }: Props) {
  if (!state.compiled) return null;
  const selectedId = state.selection?.kind === "ic" ? state.selection.id : null;
  return (
    <section className="section grow">
      <header className="section-head">
        <h2>Initial conditions</h2>
        {state.ics.length > 0 && (
          <button type="button" className="link-btn" onClick={actions.clearIcs}>
            Clear all
          </button>
        )}
      </header>
      {state.ics.length === 0 ? (
        <p className="empty-hint">Click on the plot to place one. Drag it to move it.</p>
      ) : (
        <ul className="ic-list">
          {state.ics.map((ic) => (
            <li key={ic.id} className={"ic-row" + (ic.id === selectedId ? " selected" : "")}>
              <button
                type="button"
                className="ic-main"
                onClick={() => actions.select({ kind: "ic", id: ic.id })}
              >
                <span className="swatch" style={{ background: `var(--traj-${(ic.color % 7) + 1})` }} />
                <Tex latex={icLatex(ic, state)} />
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-label="Delete initial condition"
                title="Delete"
                onClick={() => actions.removeIcs([ic.id])}
              >
                <IconClose size={14} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
