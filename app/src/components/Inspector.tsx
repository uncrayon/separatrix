// Right panel: details of whatever is selected, and the solver settings.

import { Fragment } from "react";
import type { Method, Status, TrajectoryOut } from "../api/types";
import { Tex } from "../math/Tex";
import { classificationExplain, classificationTitle, complexTex, fmt, fmtTex } from "../math/format";
import type { StudioActions, StudioState } from "../state/useStudio";
import { DEFAULT_FIXED_STEP, type InitialCondition } from "../state/model";
import { fixedPointShape } from "../canvas/draw";
import { FixedPointGlyph } from "../canvas/PortraitCanvas";
import { NumberField, Segmented, ToggleChip } from "./controls";
import { IconClose } from "./icons";

interface Props {
  state: StudioState;
  actions: StudioActions;
}

export function Inspector({ state, actions }: Props) {
  return (
    <aside className="panel panel-right" aria-label="Inspector">
      <SelectionSection state={state} actions={actions} />
      <SolverSection state={state} actions={actions} />
    </aside>
  );
}

function SelectionSection({ state, actions }: Props) {
  const sel = state.selection;
  if (sel?.kind === "ic") {
    const ic = state.ics.find((i) => i.id === sel.id);
    if (ic) return <TrajectoryDetails ic={ic} state={state} actions={actions} />;
  }
  if (sel?.kind === "fp") {
    const fp = state.portrait?.res.fixedPoints[sel.index];
    if (fp) return <FixedPointDetails index={sel.index} state={state} actions={actions} />;
  }
  return (
    <section className="section">
      <header className="section-head">
        <h2>Inspector</h2>
      </header>
      <p className="empty-hint">
        Select a trajectory (its start dot) or a fixed point on the plot to see its numbers here.
      </p>
    </section>
  );
}

const STATUS_TEXT: Record<Status, string> = {
  completed: "Completed",
  blewUp: "Escaped to infinity",
  maxSteps: "Stopped: step limit reached",
  stepTooSmall: "Stopped: step size became too small",
  nonFinite: "Stopped: value became NaN or ∞",
};

function TrajectoryDetails({ ic, state, actions }: Props & { ic: InitialCondition }) {
  const sys = state.compiled!;
  const isMap = sys.kind === "map";
  const index = state.ics.indexOf(ic) + 1;
  const p = state.portrait;
  const runs: { out: TrajectoryOut; dir: string }[] = [];
  if (p && p.meta.systemId === sys.systemId) {
    p.meta.trajectories.forEach((m, i) => {
      const out = p.res.trajectories[i];
      if (m.icId === ic.id && out) runs.push({ out, dir: m.direction });
    });
  }
  const dim = sys.states.length;
  const showT0 = !isMap;
  return (
    <section className="section">
      <header className="section-head">
        <h2>
          <span className="swatch" style={{ background: `var(--traj-${(ic.color % 7) + 1})` }} /> Trajectory{" "}
          {index}
        </h2>
        <button type="button" className="icon-btn" aria-label="Deselect" onClick={() => actions.select(null)}>
          <IconClose size={14} />
        </button>
      </header>
      <div className="kv-grid">
        {showT0 && (
          <>
            <Tex latex="t_0" className="kv-key" />
            <NumberField
              value={ic.t0}
              onCommit={(t0) => actions.updateIc(ic.id, ic.x0, t0)}
              step={0.1}
              ariaLabel="Start time"
              digits={5}
            />
          </>
        )}
        {sys.states.map((s, i) => (
          <Fragment key={s.name}>
            <Tex latex={`${s.latex}_0`} className="kv-key" />
            <NumberField
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
          </Fragment>
        ))}
      </div>
      {runs.map(({ out, dir }) => {
        const n = out.t.length;
        const last = n ? out.x.slice((n - 1) * dim, n * dim) : [];
        const tEnd = n ? out.t[n - 1] : NaN;
        return (
          <div className="run" key={dir}>
            <div className="run-head">
              <span>{isMap ? "Orbit" : dir === "forward" ? "Forward in time" : "Backward in time"}</span>
              <span
                className={
                  "badge " +
                  (out.status === "completed" ? "ok" : out.status === "blewUp" ? "neutral" : "warn")
                }
                title={
                  out.status === "blewUp"
                    ? "The solution left |x| < 10¹² — usually the maths, not the solver"
                    : undefined
                }
              >
                {STATUS_TEXT[out.status]}
              </span>
            </div>
            <div className="stats">
              <span>{isMap ? "Iterates" : "Accepted steps"}</span>
              <b>{out.stats.accepted.toLocaleString()}</b>
              {!isMap && (
                <>
                  <span>Rejected steps</span>
                  <b>{out.stats.rejected.toLocaleString()}</b>
                </>
              )}
              <span>Function evaluations</span>
              <b>{out.stats.evals.toLocaleString()}</b>
              <span>{isMap ? "Last iterate" : "Ends at"}</span>
              <b>
                <Tex latex={`${isMap ? "n" : "t"} = ${fmtTex(tEnd, 4)}`} />
              </b>
            </div>
            {last.length > 0 && (
              <div className="end-state">
                <Tex
                  latex={
                    dim === 1
                      ? `${sys.states[0].latex} = ${fmtTex(last[0], 5)}`
                      : `(${sys.states.map((s) => s.latex).join(", ")}) = (${last.map((v) => fmtTex(v, 4)).join(", ")})`
                  }
                />
              </div>
            )}
          </div>
        );
      })}
      <button type="button" className="btn ghost small danger" onClick={() => actions.removeIcs([ic.id])}>
        Delete trajectory
      </button>
    </section>
  );
}

function FixedPointDetails({ index, state, actions }: Props & { index: number }) {
  const sys = state.compiled!;
  const fp = state.portrait!.res.fixedPoints[index];
  const isMap = sys.kind === "map";
  const sym = isMap ? "\\mu" : "\\lambda";
  const coords =
    sys.states.length === 1
      ? `${sys.states[0].latex}^* = ${fmtTex(fp.x[0], 6)}`
      : `(${sys.states.map((s) => s.latex).join(", ")})^* = (${fp.x.map((v) => fmtTex(v, 5)).join(", ")})`;
  // Trace and determinant (Strogatz §5.2) for planar flows.
  let tauDelta: { tau: number; delta: number } | null = null;
  if (!isMap && fp.eigenvalues.length === 2) {
    const [[a, b], [c, d]] = fp.eigenvalues;
    tauDelta = { tau: a + c, delta: a * c - b * d };
  }
  return (
    <section className="section">
      <header className="section-head">
        <h2>
          <FixedPointGlyph shape={fixedPointShape(fp.classification)} filled={fp.stable} /> Fixed point
        </h2>
        <button type="button" className="icon-btn" aria-label="Deselect" onClick={() => actions.select(null)}>
          <IconClose size={14} />
        </button>
      </header>
      <div className="fp-class">
        <span className="fp-class-title">{classificationTitle(fp.classification, sys.states.length)}</span>
        <span className={"badge " + (fp.stable ? "ok" : "neutral")}>{fp.stable ? "stable" : "unstable"}</span>
      </div>
      <p className="explain">{classificationExplain(fp.classification, sys.kind, sys.states.length)}</p>
      <div className="math-block">
        <Tex latex={coords} />
      </div>
      <div className="subhead">{isMap ? "Multipliers" : "Eigenvalues of the Jacobian"}</div>
      <div className="math-block">
        {fp.eigenvalues.map((ev, i) => (
          <div key={i}>
            <Tex latex={`${sym}_{${i + 1}} = ${complexTex(ev, 5)}`} />
            {isMap && <span className="muted"> &nbsp;|μ| = {fmt(Math.hypot(ev[0], ev[1]), 4)}</span>}
          </div>
        ))}
      </div>
      {tauDelta && (
        <>
          <div className="subhead">Trace and determinant</div>
          <div className="math-block">
            <div>
              <Tex
                latex={`\\tau = ${fmtTex(tauDelta.tau, 4)},\\quad \\Delta = ${fmtTex(tauDelta.delta, 4)}`}
              />
            </div>
            <div>
              <Tex latex={`\\tau^2 - 4\\Delta = ${fmtTex(tauDelta.tau ** 2 - 4 * tauDelta.delta, 4)}`} />
            </div>
          </div>
        </>
      )}
    </section>
  );
}

const METHODS: { value: Method; label: string; title: string }[] = [
  { value: "euler", label: "Euler", title: "Forward Euler, fixed step (1st order)" },
  { value: "heun", label: "Heun", title: "Heun / improved Euler, fixed step (2nd order)" },
  { value: "rk4", label: "RK4", title: "Classical Runge–Kutta, fixed step (4th order)" },
  { value: "dopri5", label: "DOPRI5", title: "Dormand–Prince 5(4), adaptive step with error control" },
];

function SolverSection({ state, actions }: Props) {
  const sys = state.compiled;
  if (!sys) return null;
  if (sys.kind === "map") {
    return (
      <section className="section">
        <header className="section-head">
          <h2>Iteration</h2>
        </header>
        <div className="kv-grid">
          <span className="kv-key">Iterates</span>
          <NumberField
            value={state.solver.tEnd}
            onCommit={(v) => v >= 1 && actions.setSolver({ tEnd: Math.round(v) })}
            step={10}
            ariaLabel="Iterates"
          />
        </div>
        <p className="empty-hint">Maps are iterated exactly; there is no step size to choose.</p>
      </section>
    );
  }
  const s = state.solver;
  const adaptive = s.method === "dopri5";
  const timeOnAxis = state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time";
  return (
    <section className="section">
      <header className="section-head">
        <h2>Solver</h2>
      </header>
      <Segmented
        ariaLabel="Integration method"
        value={s.method}
        options={METHODS}
        onChange={(method) =>
          actions.setSolver({ method, h: method === "dopri5" ? 0 : s.h > 0 ? s.h : DEFAULT_FIXED_STEP })
        }
      />
      <p className="explain">{METHODS.find((m) => m.value === s.method)?.title}</p>
      <div className="kv-grid">
        {adaptive ? (
          <>
            <span className="kv-key" title="Relative tolerance">
              Rel. tolerance
            </span>
            <NumberField
              value={s.rtol}
              onCommit={(rtol) => rtol > 0 && actions.setSolver({ rtol })}
              digits={2}
              ariaLabel="Relative tolerance"
            />
            <span className="kv-key" title="Absolute tolerance">
              Abs. tolerance
            </span>
            <NumberField
              value={s.atol}
              onCommit={(atol) => atol > 0 && actions.setSolver({ atol })}
              digits={2}
              ariaLabel="Absolute tolerance"
            />
          </>
        ) : (
          <>
            <span className="kv-key">Step size h</span>
            <NumberField
              value={s.h > 0 ? s.h : DEFAULT_FIXED_STEP}
              onCommit={(h) => h > 0 && actions.setSolver({ h })}
              digits={3}
              ariaLabel="Step size"
            />
          </>
        )}
        <span className="kv-key">Time span T</span>
        {timeOnAxis ? (
          // With t on an axis the visible time window is the span (see
          // trajectorySpans); an editable T here would silently do nothing.
          <span className="kv-value muted" title="Trajectories cover exactly the visible time window">
            follows the <Tex latex="t" /> axis
          </span>
        ) : (
          <NumberField
            value={s.tEnd}
            onCommit={(tEnd) => tEnd > 0 && actions.setSolver({ tEnd })}
            step={1}
            ariaLabel="Time span"
          />
        )}
        <span className="kv-key">Samples</span>
        <NumberField
          value={s.samples}
          onCommit={(v) => actions.setSolver({ samples: Math.max(0, Math.round(v)) })}
          digits={5}
          format={(v) => (v > 0 ? String(v) : "auto")}
          ariaLabel="Samples per trajectory (0 = automatic)"
        />
      </div>
      <p className="empty-hint small">
        Type 0 for automatic sampling. Step limit {s.maxSteps.toLocaleString()} per run.
      </p>
      <div className="subhead">Display</div>
      <div className="chips">
        <ToggleChip
          title="Also follow each start point backwards in time (drawn fainter)"
          on={state.layers.backward}
          onChange={(backward) => actions.setLayers({ backward })}
        >
          Backward time
        </ToggleChip>
      </div>
      <div className="kv-grid">
        <span className="kv-key">Field grid</span>
        <NumberField
          value={state.layers.fieldGrid}
          onCommit={(g) => actions.setLayers({ fieldGrid: Math.max(4, Math.min(60, Math.round(g))) })}
          step={1}
          ariaLabel="Arrows per side"
        />
      </div>
    </section>
  );
}
