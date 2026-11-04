import { Fragment, useCallback, useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { Diagnostic } from "../api/types";
import { MathField, latexToText, textToLatex } from "../math/MathField";
import { Tex } from "../math/Tex";
import { firstOrderDisplay, fmt, diagnosticText } from "../math/format";
import type { StudioActions, StudioState } from "../state/useStudio";
import type { InitialCondition, ParamState } from "../state/model";
import { IconClose, IconError, IconInfo, IconPlus, IconTrajectoryAdd, IconTrash, IconWarning } from "./icons";
import { tipDismiss } from "./Chrome";
import { IconButton } from "./controls";
import { focusAfterTrajectoryDelete, focusLater, neighbourAfterDelete } from "./focus";
import { PanelSection, useSections } from "./PanelSection";
import { diagnosticLine } from "./diagnostics";
import { startPointParts } from "./startPoint";

interface Props {
  state: StudioState;
  actions: StudioActions;
}

export function SystemPanel({ state, actions }: Props) {
  const [currentParam, setCurrentParam] = useState<string | null>(null);
  const [open, setOpen] = useSections<"params" | "ics">(SECTIONS_KEY, {
    params: true,
    ics: state.ics.length <= 5,
  });
  const selIc = state.selection?.kind === "ic" ? state.selection.id : null;
  useEffect(() => {
    if (selIc !== null) setOpen("ics", true);
  }, [selIc, setOpen]);
  return (
    <aside className="panel panel-left ds-panel-section-panel ds-panel-section-panel--left" aria-label="System">
      <section className="ds-panel-section sys-top" aria-label="Equations">
        <EquationEditor state={state} actions={actions} />
        <InferenceChips state={state} currentParam={currentParam} />
      </section>
      <ParamsSection
        params={state.params}
        actions={actions}
        onCurrent={setCurrentParam}
        open={open.params}
        onToggle={(v) => setOpen("params", v)}
      />
      <IcSection state={state} actions={actions} open={open.ics} onToggle={(v) => setOpen("ics", v)} />
    </aside>
  );
}

const SECTIONS_KEY = "separatrix.system.sections";

function fmtNum(v: number, digits = 4): string {
  return fmt(v, digits).replace(/^-/, "−");
}

function parseNum(s: string): number {
  const t = s.trim().replace(/−/g, "-").toLowerCase();
  if (t === "pi" || t === "π") return Math.PI;
  if (t === "-pi" || t === "-π") return -Math.PI;
  return t === "" ? NaN : Number(t);
}

const SEVERITY_WORD: Record<Diagnostic["severity"], string> = {
  error: "Error",
  warning: "Warning",
  info: "Note",
};

function SeverityIcon({ severity }: { severity: Diagnostic["severity"] }) {
  const cls = "ds-diagnostic-icon";
  if (severity === "error") return <IconError className={cls} />;
  if (severity === "warning") return <IconWarning className={cls} />;
  return <IconInfo className={cls} />;
}

function DiagnosticItem({
  d,
  variant,
  id,
  meta,
}: {
  d: Diagnostic;
  variant: "compact" | "block";
  id?: string;
  meta?: ReactNode;
}) {
  return (
    <li
      id={id}
      className={`ds-diagnostic ds-diagnostic--${variant} ds-diagnostic--${d.severity}`}
    >
      <SeverityIcon severity={d.severity} />
      <span className="ds-diagnostic-body">
        <span className="ds-diagnostic-word">{SEVERITY_WORD[d.severity]}</span>
        <span className="ds-diagnostic-msg">{diagnosticText(d.message)}</span>
        {meta && <span className="ds-diagnostic-meta">{meta}</span>}
      </span>
    </li>
  );
}

type InputMode = "latex" | "text";
const MODE_KEY = "separatrix.inputMode";

function readMode(): InputMode {
  try {
    return localStorage.getItem(MODE_KEY) === "text" ? "text" : "latex";
  } catch {
    return "latex";
  }
}

const MAX_PER_LINE = 2;

function EquationEditor({ state, actions }: Props) {
  const [mode, setModeState] = useState<InputMode>(readMode);
  const [focusedLine, setFocusedLine] = useState<number | null>(null);
  const [drafts, setDrafts] = useState<Record<number, { text: string; latex: string }>>({});
  const focusNext = useRef<number | null>(null);
  const textRefs = useRef(new Map<number, HTMLInputElement>());

  const setMode = (m: InputMode) => {
    setModeState(m);
    setDrafts({});
    try {
      localStorage.setItem(MODE_KEY, m);
    } catch {}
  };

  const addLine = (afterId?: number) => {
    focusNext.current = actions.addEquation(afterId);
  };

  useEffect(() => {
    const id = focusNext.current;
    if (id === null) return;
    if (mode === "latex") {
      focusNext.current = null;
      return;
    }
    const el = textRefs.current.get(id);
    if (el) {
      el.focus();
      focusNext.current = null;
    }
  });

  const latexes = state.equations.map((l) => l.latex);
  const located = state.diagnostics.map((d) => ({ d, line: diagnosticLine(d, latexes) }));
  const general: { d: Diagnostic; line: number }[] = located.filter((x) => x.line < 0);
  const single = state.equations.length === 1;

  const lines = state.equations.map((line, i) => {
    const all = located.filter((x) => x.line === i).map((x) => x.d);
    const here = all.slice(0, MAX_PER_LINE);
    for (const d of all.slice(MAX_PER_LINE)) general.push({ d, line: i });
    const hasError = all.some((d) => d.severity === "error");
    const diagIds = here.map((_, k) => `eq-${line.id}-d${k}`);
    const label = `Equation ${i + 1}`;
    const describedBy = diagIds.length ? diagIds.join(" ") : undefined;
    const draft = drafts[line.id];
    const text = draft && draft.latex === line.latex ? draft.text : latexToText(line.latex);
    const focused = focusedLine === line.id;
    return (
      <li
        key={line.id}
        className={
          "ds-equation-editor-line" + (focused ? " is-focus" : "") + (hasError ? " is-error" : "")
        }
      >
        <span className="ds-equation-editor-gutter" aria-hidden="true">
          {i + 1}
        </span>
        {mode === "latex" ? (
          <MathField
            value={line.latex}
            onChange={(latex) => actions.setEquation(line.id, latex)}
            onEnter={() => addLine(line.id)}
            onFocusChange={(f) => setFocusedLine((cur) => (f ? line.id : cur === line.id ? null : cur))}
            placeholder={i === 0 ? "\\dot{x}=" : ""}
            ariaLabel={label}
            ariaDescribedBy={describedBy}
            autoFocus={line.latex === ""}
            invalid={hasError}
          />
        ) : (
          <input
            ref={(el) => {
              if (el) textRefs.current.set(line.id, el);
              else textRefs.current.delete(line.id);
            }}
            className="ds-equation-editor-field eq-text-input"
            aria-label={label}
            aria-describedby={describedBy}
            aria-invalid={hasError || undefined}
            spellCheck={false}
            autoComplete="off"
            autoCapitalize="off"
            placeholder={i === 0 ? "x' =" : undefined}
            value={text}
            onChange={(e) => {
              const t = e.target.value;
              const latex = textToLatex(t);
              setDrafts((d) => ({ ...d, [line.id]: { text: t, latex } }));
              actions.setEquation(line.id, latex);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                addLine(line.id);
              }
            }}
            onFocus={() => setFocusedLine(line.id)}
            onBlur={() => setFocusedLine((cur) => (cur === line.id ? null : cur))}
          />
        )}
        {single ? (
          <span aria-hidden="true" />
        ) : (
          <button
            type="button"
            className="ds-icon-button ds-icon-button--sm ds-equation-editor-remove"
            onClick={() => {
              const ids = state.equations.map((l) => l.id);
              const next = neighbourAfterDelete(ids, i);
              actions.removeEquation(line.id);
              const at = next === null ? -1 : ids.filter((id) => id !== line.id).indexOf(next);
              focusLater(() => {
                const box = document.querySelectorAll<HTMLElement>(".panel-left .ds-equation-editor-field")[at];
                return box?.matches("input") ? box : box?.querySelector<HTMLElement>("math-field");
              });
            }}
            aria-label={`Remove equation ${i + 1}`}
            data-tip="Remove equation"
            data-tip-align="end"
            {...tipDismiss}
          >
            <IconClose />
          </button>
        )}
        {here.length > 0 && (
          <ul className="ds-equation-editor-diags" aria-live="polite">
            {here.map((d, k) => (
              <DiagnosticItem key={k} d={d} variant="compact" id={diagIds[k]} />
            ))}
          </ul>
        )}
      </li>
    );
  });

  return (
    <div className={"ds-equation-editor" + (mode === "text" ? " ds-equation-editor--text" : "")}>
      <header className="ds-equation-editor-head">
        <h2 className="ds-equation-editor-title heading">Equations</h2>
        <ModeSwitch mode={mode} onChange={setMode} />
      </header>
      <ol className="ds-equation-editor-lines">{lines}</ol>
      <div className="ds-equation-editor-foot">
        <button type="button" className="ds-button ds-button--ghost ds-button--sm" onClick={() => addLine()}>
          <IconPlus /> Add equation
        </button>
        <span className="caption ds-equation-editor-hint">
          {mode === "text" ? "Write x*y, not xy" : "Enter adds a line"}
        </span>
      </div>
      {general.length > 0 && (
        <ul className="ds-diagnostic-list ds-diagnostic-list--block" aria-live="polite">
          {general.map(({ d, line }, k) => (
            <DiagnosticItem key={k} d={d} variant="block" meta={line >= 0 ? `Line ${line + 1}` : undefined} />
          ))}
        </ul>
      )}
    </div>
  );
}

function ModeSwitch({ mode, onChange }: { mode: InputMode; onChange: (m: InputMode) => void }) {
  const refs = useRef<Record<InputMode, HTMLButtonElement | null>>({ latex: null, text: null });
  const options: { value: InputMode; label: string; title: string }[] = [
    { value: "latex", label: "LaTeX", title: "Type maths as you would write it (MathLive)" },
    { value: "text", label: "Text", title: "Plain text: x' = x*(3 - x - 2y)" },
  ];
  const onKey = (e: KeyboardEvent) => {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(e.key)) return;
    e.preventDefault();
    const next: InputMode =
      e.key === "Home" ? "latex" : e.key === "End" ? "text" : mode === "latex" ? "text" : "latex";
    onChange(next);
    refs.current[next]?.focus();
  };
  return (
    <div
      className="ds-segmented ds-segmented--sm ds-equation-editor-mode"
      role="radiogroup"
      aria-label="Input mode"
      onKeyDown={onKey}
    >
      {options.map((o) => (
        <button
          key={o.value}
          ref={(el) => {
            refs.current[o.value] = el;
          }}
          type="button"
          className="ds-segmented-item"
          role="radio"
          aria-checked={mode === o.value}
          tabIndex={mode === o.value ? 0 : -1}
          data-tooltip={o.title}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

const sliderId = (name: string) => `param-slider-${name}`;
const chipId = (name: string) => `param-chip-${name}`;

function InferenceChips({ state, currentParam }: { state: StudioState; currentParam: string | null }) {
  const sys = state.compiled;
  if (!sys || !state.compileOk) return null;
  const kind = sys.hamiltonian ? "Hamiltonian" : sys.kind === "map" ? "Map" : "ODE";
  const kindTitle = sys.hamiltonian
    ? "Hamiltonian flow: q̇ = ∂H/∂p, ṗ = −∂H/∂q"
    : sys.kind === "map"
      ? "Discrete map: iterated x ↦ f(x)"
      : "Ordinary differential equation (a flow)";
  const quals = [`${sys.states.length}D`];
  if (sys.kind !== "map" && !sys.hamiltonian) quals.push(sys.autonomous ? "autonomous" : "non-autonomous");
  const isMap = sys.kind === "map";
  const firstOrder =
    sys.hamiltonian || sys.equationsLatex.length !== state.equations.filter((e) => e.latex.trim()).length;

  const goToSlider = (name: string) => {
    const el = document.getElementById(sliderId(name));
    el?.scrollIntoView({ block: "nearest" });
    el?.focus();
  };

  return (
    <div className="sys-inference">
      <dl className="ds-inference-chips">
        <dt className="ds-inference-chips-key">Kind</dt>
        <dd className="ds-inference-chips-set">
          <span className="ds-inference-chip" data-tooltip={kindTitle}>
            <span className="ds-inference-chip-kind">{kind}</span>
            {quals.map((q) => (
              <Fragment key={q}>
                <span className="ds-inference-chip-sep" aria-hidden="true">
                  ·
                </span>
                <span className="ds-inference-chip-qual">{q}</span>
              </Fragment>
            ))}
            <span className="ds-sr">: {kindTitle}</span>
          </span>
        </dd>

        <dt className="ds-inference-chips-key">States</dt>
        <dd className="ds-inference-chips-set">
          {sys.states.map((s) => (
            <span key={s.name} className="ds-inference-chip ds-inference-chip--math">
              <Tex latex={s.latex} />
            </span>
          ))}
        </dd>

        <dt className="ds-inference-chips-key">Parameters</dt>
        <dd className="ds-inference-chips-set">
          {state.params.length === 0 && <span className="ds-inference-chips-none">none</span>}
          {state.params.map((p) => (
            <button
              key={p.name}
              id={chipId(p.name)}
              type="button"
              className="ds-inference-chip ds-inference-chip--param"
              aria-label={`${p.name} = ${fmtNum(p.value)}, go to slider`}
              aria-current={currentParam === p.name ? "true" : undefined}
              onClick={() => goToSlider(p.name)}
            >
              <Tex latex={p.latex} />
              <span className="ds-inference-chip-eq" aria-hidden="true">
                =
              </span>
              <span className="ds-inference-chip-value">{fmtNum(p.value)}</span>
            </button>
          ))}
        </dd>

        <dt className="ds-inference-chips-key">{isMap ? "Step" : "Time"}</dt>
        <dd className="ds-inference-chips-set">
          <span className="ds-inference-chip ds-inference-chip--math">
            <Tex latex={isMap ? "n" : "t"} />
          </span>
        </dd>
      </dl>
      {firstOrder && (
        <div className="sys-first-order">
          <p className="caption sys-first-order-label">Solved as the first-order system</p>
          {firstOrderDisplay(sys.equationsLatex, sys.states, sys.kind).map((e, i) => (
            <div key={i} className="sys-first-order-eq math">
              <Tex latex={e} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ParamsSection({
  params,
  actions,
  onCurrent,
  open,
  onToggle,
}: {
  params: ParamState[];
  actions: StudioActions;
  onCurrent: (name: string | null) => void;
  open: boolean;
  onToggle: (open: boolean) => void;
}) {
  if (!params.length) return null;
  return (
    <PanelSection title="Parameters" open={open} onToggle={onToggle}>
      <div className="ds-paramlist">
        {params.map((p) => (
          <ParamRow key={p.name} p={p} actions={actions} onCurrent={onCurrent} />
        ))}
      </div>
      <p className="ds-panel-section-hint caption">Drag a slider to explore.</p>
    </PanelSection>
  );
}

type Hint = { kind: "info" | "error"; text: string } | null;

function ParamRow({
  p,
  actions,
  onCurrent,
}: {
  p: ParamState;
  actions: StudioActions;
  onCurrent: (name: string | null) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [hint, setHint] = useState<Hint>(null);
  const [valueInvalid, setValueInvalid] = useState(false);
  const [bound, setBound] = useState<"min" | "max" | null>(null);
  const [boundDraft, setBoundDraft] = useState("");
  const [boundInvalid, setBoundInvalid] = useState(false);
  const valueRef = useRef<HTMLInputElement>(null);
  const boundRefs = useRef<Record<"min" | "max", HTMLButtonElement | null>>({ min: null, max: null });

  const span = p.max - p.min;
  const frac = span > 0 ? Math.max(0, Math.min(1, (p.value - p.min) / span)) : 0;
  const hintId = `param-hint-${p.name}`;

  useEffect(() => {
    if (!dragging) return;
    const up = () => setDragging(false);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    return () => {
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
  }, [dragging]);

  const commitValue = useCallback(
    (v: number, announce: boolean) => {
      const rounded = Number(v.toPrecision(12));
      if (announce && (rounded < p.min || rounded > p.max)) {
        const lo = Math.min(p.min, rounded);
        const hi = Math.max(p.max, rounded);
        setHint({ kind: "info", text: `Range widened to ${fmtNum(lo, 3)}–${fmtNum(hi, 3)} to fit ${fmtNum(rounded)}` });
      } else {
        setHint(null);
      }
      actions.setParam(p.name, rounded);
    },
    [actions, p.name, p.min, p.max],
  );

  const sliderKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const small = span / 100;
    let v: number | null = null;
    switch (e.key) {
      case "ArrowRight":
      case "ArrowUp":
        v = p.value + small;
        break;
      case "ArrowLeft":
      case "ArrowDown":
        v = p.value - small;
        break;
      case "PageUp":
        v = p.value + small * 10;
        break;
      case "PageDown":
        v = p.value - small * 10;
        break;
      case "Home":
        v = p.min;
        break;
      case "End":
        v = p.max;
        break;
      case "Escape": {
        const chip = document.getElementById(chipId(p.name));
        if (chip) {
          e.preventDefault();
          chip.focus();
        }
        return;
      }
    }
    if (v === null) return;
    e.preventDefault();
    commitValue(Math.max(p.min, Math.min(p.max, v)), false);
  };

  const commitDraft = (s: string) => {
    if (s === fmt(p.value, 10)) {
      setValueInvalid(false);
      return true;
    }
    const v = parseNum(s);
    if (!Number.isFinite(v)) {
      setValueInvalid(true);
      setHint({ kind: "error", text: "Not a number. Use a point for decimals: 0.25" });
      return false;
    }
    setValueInvalid(false);
    commitValue(v, true);
    return true;
  };

  const startBound = (which: "min" | "max") => {
    setBound(which);
    setBoundDraft(fmt(which === "min" ? p.min : p.max, 8));
    setBoundInvalid(false);
  };
  const endBound = (commit: boolean) => {
    const which = bound;
    if (!which) return;
    if (commit) {
      const v = parseNum(boundDraft);
      const lo = which === "min" ? v : p.min;
      const hi = which === "max" ? v : p.max;
      let error: string | null = null;
      if (!Number.isFinite(v)) error = "Not a number. Use a point for decimals: 0.25";
      else if (lo >= hi)
        error =
          which === "min"
            ? `Minimum must be less than ${fmtNum(p.max, 3)}`
            : `Maximum must be greater than ${fmtNum(p.min, 3)}`;
      if (error) {
        setBoundInvalid(true);
        setHint({ kind: "error", text: error });
        return;
      }
      actions.setParamRange(p.name, lo, hi);
    }
    setHint(null);
    setBound(null);
    setBoundInvalid(false);
    requestAnimationFrame(() => boundRefs.current[which]?.focus());
  };

  const describedBy = hint ? hintId : undefined;

  const boundEl = (which: "min" | "max") => {
    const v = which === "min" ? p.min : p.max;
    const word = which === "min" ? "Minimum" : "Maximum";
    if (bound === which) {
      return (
        <input
          autoFocus
          className={"ds-param-bound-input" + (which === "max" ? " ds-param-bound-input--max" : "")}
          inputMode="decimal"
          spellCheck={false}
          aria-label={`${word} of ${p.name}`}
          aria-invalid={boundInvalid || undefined}
          aria-describedby={describedBy}
          value={boundDraft}
          onFocus={(e) => e.target.select()}
          onChange={(e) => setBoundDraft(e.target.value)}
          onBlur={() => {
            if (bound === which) {
              const v2 = parseNum(boundDraft);
              const lo = which === "min" ? v2 : p.min;
              const hi = which === "max" ? v2 : p.max;
              if (Number.isFinite(v2) && lo < hi) endBound(true);
              else {
                setBound(null);
                setBoundInvalid(false);
                setHint(null);
              }
            }
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              endBound(true);
            } else if (e.key === "Escape") {
              e.preventDefault();
              setHint(null);
              endBound(false);
            }
          }}
        />
      );
    }
    return (
      <button
        ref={(el) => {
          boundRefs.current[which] = el;
        }}
        type="button"
        className="ds-param-bound"
        data-bound={which}
        aria-label={`${word} of ${p.name}: ${fmtNum(v, 3)}. Click to edit`}
        onClick={() => startBound(which)}
      >
        {fmtNum(v, 3)}
      </button>
    );
  };

  return (
    <div className={"ds-param" + (dragging ? " is-dragging" : "")} id={`param-${p.name}`}>
      <label className="ds-param-name" htmlFor={sliderId(p.name)}>
        <Tex latex={p.latex} />
      </label>
      <input
        id={sliderId(p.name)}
        className="ds-param-slider"
        type="range"
        min={p.min}
        max={p.max}
        step={span > 0 ? span / 1000 : "any"}
        value={p.value}
        style={{ ["--frac" as string]: String(frac) }}
        aria-label={`Parameter ${p.name}`}
        aria-valuetext={fmtNum(p.value)}
        aria-describedby={describedBy}
        onChange={(e) => {
          setDraft(null);
          setValueInvalid(false);
          commitValue(Number(e.target.value), false);
        }}
        onKeyDown={sliderKey}
        onPointerDown={() => setDragging(true)}
        onFocus={() => onCurrent(p.name)}
        onBlur={() => onCurrent(null)}
      />
      <input
        ref={valueRef}
        className="ds-param-value"
        inputMode="decimal"
        spellCheck={false}
        aria-label={`Value of ${p.name}`}
        aria-invalid={valueInvalid || undefined}
        aria-describedby={describedBy}
        value={draft ?? fmtNum(p.value)}
        onFocus={(e) => {
          setDraft(fmt(p.value, 10));
          const el = e.target;
          requestAnimationFrame(() => el.select());
        }}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={(e) => {
          if (commitDraft(e.target.value)) setDraft(null);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            if (commitDraft(e.currentTarget.value)) setDraft(fmt(parseNum(e.currentTarget.value), 10));
          } else if (e.key === "Escape") {
            e.preventDefault();
            setValueInvalid(false);
            setHint(null);
            setDraft(fmt(p.value, 10));
            e.currentTarget.select();
          } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            const base = Number.isFinite(parseNum(e.currentTarget.value)) ? parseNum(e.currentTarget.value) : p.value;
            const step = (span > 0 ? span / 100 : 0.01) * (e.shiftKey ? 10 : 1);
            const next = Number((base + (e.key === "ArrowUp" ? step : -step)).toPrecision(12));
            setValueInvalid(false);
            setDraft(String(next));
            commitValue(next, true);
          }
        }}
      />
      <div className="ds-param-range">
        {boundEl("min")}
        {boundEl("max")}
      </div>
      {hint && (
        <p
          id={hintId}
          className={"ds-param-hint caption" + (hint.kind === "error" ? " ds-param-hint--error" : "")}
        >
          {hint.kind === "error" ? <IconError /> : <IconInfo />}
          <span>{hint.text}</span>
        </p>
      )}
    </div>
  );
}

function icParts(ic: InitialCondition, state: StudioState): { nameLatex: string; value: string } {
  const sys = state.compiled;
  if (!sys) return { nameLatex: "", value: "" };
  const withTime =
    sys.kind === "ode" &&
    (!sys.autonomous || state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time");
  return startPointParts(
    sys.states.map((s) => s.latex),
    ic.x0,
    withTime ? ic.t0 : null,
  );
}

function IcSection({ state, actions, open, onToggle }: Props & { open: boolean; onToggle: (open: boolean) => void }) {
  if (!state.compiled) return null;
  const selectedId = state.selection?.kind === "ic" ? state.selection.id : null;
  const n = state.ics.length;
  const ids = state.ics.map((ic) => ic.id);
  return (
    <PanelSection
      title="Initial conditions"
      className="sys-ics"
      count={n}
      open={open}
      onToggle={onToggle}
      flush
      actions={
        n > 0 && (
          <IconButton
            size="sm"
            label="Delete all trajectories"
            tipAlign="end"
            onClick={() => {
              actions.clearIcs();
              focusAfterTrajectoryDelete(".panel-left", [], -1);
            }}
          >
            <IconTrash />
          </IconButton>
        )
      }
    >
      {n === 0 ? (
        <div className="ds-traj-empty">
          <IconTrajectoryAdd />
          <p className="ds-traj-empty-text body">Click the plot to start a trajectory.</p>
          <p className="ds-traj-empty-hint caption">Drag a start point to move it.</p>
        </div>
      ) : (
        <ul className="ds-traj-list" aria-label="Initial conditions">
          {state.ics.map((ic, i) => {
            const { nameLatex, value } = icParts(ic, state);
            const selected = ic.id === selectedId;
            return (
              <li
                key={ic.id}
                className={"ds-traj-row ic-row" + (selected ? " is-selected" : "")}
                style={{ ["--traj" as string]: `var(--traj-${(ic.color % 7) + 1})` }}
                data-ic-id={ic.id}
              >
                <button
                  type="button"
                  className="ds-traj-select"
                  aria-pressed={selected}
                  aria-label={`Initial condition ${i + 1}, start ${value}`}
                  onClick={() => actions.select({ kind: "ic", id: ic.id })}
                >
                  <span className="ds-traj-line">
                    <span className="ds-traj-swatch" aria-hidden="true" />
                    <span className="ds-traj-ic numeric">
                      <span className="ds-traj-ic-name numeric-sm">
                        <Tex latex={nameLatex} />
                      </span>
                      <span className="ds-traj-ic-value">{value}</span>
                    </span>
                  </span>
                </button>
                <span className="ds-traj-actions">
                  <IconButton
                    size="sm"
                    label={`Delete initial condition ${i + 1}`}
                    tip="Delete initial condition"
                    tipAlign="end"
                    onClick={() => {
                      focusAfterTrajectoryDelete(".panel-left", ids, ic.id);
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
