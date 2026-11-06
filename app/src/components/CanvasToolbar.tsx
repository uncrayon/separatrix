import { Fragment, useEffect, useId, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import type { AxisRef } from "../api/types";
import { fitRanges } from "../canvas/viewport";
import { Tex } from "../math/Tex";
import { MathProse } from "../math/MathProse";
import { axisLatex, latexToPlain, sameAxis, viewAxisLabels } from "../math/format";
import type { StudioActions, StudioState } from "../state/useStudio";
import {
  VIEW_CHOICE_LABELS,
  fitLyapunovRanges,
  fitOrbitRange,
  hasNullclineLayer,
  isCobweb,
  isLyapunov,
  isOrbit,
  lyapunovFits,
  orbitFits,
  isReturnMap,
  isSection,
  isSpace,
  portraitFits,
  spaceStates,
  stateExtent,
  type SpaceView,
  viewChoice,
  viewChoiceAvailability,
  viewChoicesFor,
} from "../state/model";
import { cobwebFit, orbitValues } from "../canvas/cobweb";
import { Select } from "./controls";
import { systemNameOf } from "./systemName";
import {
  IconCheck,
  IconChevronDown,
  IconField,
  IconFixedPoint,
  IconNullcline,
  IconPan,
  IconResetView,
  IconSelect,
  IconTrajectoryAdd,
  IconZoomFit,
} from "./icons";

export type CanvasTool = "select" | "pan" | "add";

let currentTool: CanvasTool = "select";
const toolListeners = new Set<() => void>();
function setCanvasTool(t: CanvasTool) {
  if (t === currentTool) return;
  currentTool = t;
  toolListeners.forEach((l) => l());
}
function subscribeTool(l: () => void) {
  toolListeners.add(l);
  return () => toolListeners.delete(l);
}
export function useCanvasTool(): CanvasTool {
  return useSyncExternalStore(subscribeTool, () => currentTool);
}

interface Props {
  state: StudioState;
  actions: StudioActions;
}

interface IconButtonSpec {
  key: string;
  label: string;
  tip: string;
  icon: ReactNode;
  pressed?: boolean;
  tool?: boolean;
  onClick: () => void;
}

const TOOLS: { key: CanvasTool; label: string; tip: string; icon: ReactNode }[] = [
  { key: "select", label: "Select", tip: "Select (drag a start point to move it)", icon: <IconSelect /> },
  { key: "pan", label: "Pan", tip: "Pan (drag the plot; scroll to zoom)", icon: <IconPan /> },
  {
    key: "add",
    label: "Add trajectory",
    tip: "Add trajectory (click the plot to place its start)",
    icon: <IconTrajectoryAdd />,
  },
];

export function CanvasToolbar({ state, actions }: Props) {
  const tool = useCanvasTool();
  const rootRef = useRef<HTMLDivElement>(null);
  const [stop, setStop] = useState<string>(tool);
  const [dismissed, setDismissed] = useState<string | null>(null);

  const sys = state.compiled;
  if (!sys) return null;
  const isMap = sys.kind === "map";
  const space = isSpace(state.view) ? state.view : null;
  const lyapunov = isLyapunov(state.view);
  const orbit = isOrbit(state.view);
  const analysis = lyapunov || orbit;

  const zoomToFit = () => {
    const v = state.view;
    const fit = isSpace(v)
      ? fitSpace(state, v)
      : isLyapunov(v)
        ? fitLyapunov(state)
        : isOrbit(v)
          ? fitOrbit(state)
          : fitView(state);
    if (fit) actions.setView({ ...v, ...fit } as typeof v);
    else actions.resetView();
  };

  const view: IconButtonSpec[] = [
    {
      key: "fit",
      label: "Zoom to fit",
      tip: orbit
        ? "Zoom to fit the diagram over the slider's range"
        : lyapunov
          ? "Zoom to fit the run (or double-click the plot)"
          : "Zoom to fit the trajectories",
      icon: <IconZoomFit />,
      onClick: zoomToFit,
    },
    // The Lyapunov view has no home framing to go back to: fitting the run is its reset.
    ...(lyapunov
      ? []
      : [
          {
            key: "reset",
            label: "Reset view",
            tip: "Reset view (or double-click the plot)",
            icon: <IconResetView />,
            onClick: actions.resetView,
          },
        ]),
  ];
  const layers: IconButtonSpec[] = isSection(state.view) || analysis ? [] : [
    ...(!isMap && !space
      ? [
          {
            key: "field",
            label: "Direction field",
            tip: "Direction field (arrows of normalized length)",
            icon: <IconField />,
            pressed: state.layers.field,
            onClick: () => actions.setLayers({ field: !state.layers.field }),
          },
        ]
      : []),
    // Shown whenever the layer is requested and drawn, time on an axis or not.
    ...(hasNullclineLayer(sys.kind) && !space
      ? [
          {
            key: "nullclines",
            label: "Nullclines",
            tip: "Nullclines (where one component of the field vanishes)",
            icon: <IconNullcline />,
            pressed: state.layers.nullclines,
            onClick: () => actions.setLayers({ nullclines: !state.layers.nullclines }),
          },
        ]
      : []),
    {
      key: "fixed",
      label: "Fixed points",
      tip: "Fixed points (filled is stable, open is unstable)",
      icon: <IconFixedPoint />,
      pressed: state.layers.fixedPoints,
      onClick: () => actions.setLayers({ fixedPoints: !state.layers.fixedPoints }),
    },
  ];

  const tools = space || analysis ? [] : TOOLS;
  const keys = [
    ...tools.map((t) => t.key as string),
    ...view.map((b) => b.key),
    ...layers.map((b) => b.key),
    "axes",
  ];
  const tabStop = keys.includes(stop) && !view.some((b) => b.key === stop) ? stop : space || analysis ? "axes" : tool;

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest("[role=menu]")) return;
    const buttons = [
      ...(rootRef.current?.querySelectorAll<HTMLButtonElement>("button[data-key]") ?? []),
    ].filter((b) => b.offsetParent !== null);
    const i = buttons.indexOf(target as HTMLButtonElement);
    if (i < 0) return;
    let next = -1;
    if (e.key === "ArrowRight") next = (i + 1) % buttons.length;
    else if (e.key === "ArrowLeft") next = (i - 1 + buttons.length) % buttons.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = buttons.length - 1;
    else if (e.key === "Escape" && target.dataset.tip) {
      setDismissed(target.dataset.key ?? null);
      return;
    }
    if (next < 0) return;
    e.preventDefault();
    buttons[next].focus();
  };

  const button = (b: IconButtonSpec) => (
    <button
      key={b.key}
      type="button"
      data-key={b.key}
      className={
        "ds-icon-button" +
        (b.tool ? " ds-icon-button--tool" : "") +
        (dismissed === b.key ? " is-tip-dismissed" : "")
      }
      aria-label={b.label}
      aria-pressed={b.pressed}
      data-tip={b.tip}
      tabIndex={tabStop === b.key ? 0 : -1}
      onFocus={() => setStop(b.key)}
      onBlur={() => dismissed === b.key && setDismissed(null)}
      onPointerLeave={() => dismissed === b.key && setDismissed(null)}
      onClick={b.onClick}
    >
      {b.icon}
    </button>
  );

  return (
    <div
      ref={rootRef}
      className="ds-canvas-toolbar canvas-toolbar plot-toolbar"
      role="toolbar"
      aria-label="Plot tools"
      onKeyDown={onKeyDown}
    >
      {tools.length > 0 && (
      <div className="ds-canvas-toolbar-group" role="group" aria-label="Tool">
        {tools.map((t) =>
          button({
            ...t,
            ...(t.key === "add" && isCobweb(state.view)
              ? { label: "Add orbit", tip: "Add orbit (click the plot to start an orbit at that x₀)" }
              : {}),
            tool: true,
            pressed: tool === t.key,
            onClick: () => setCanvasTool(t.key),
          }),
        )}
      </div>
      )}
      {tools.length > 0 && (
        <span className="ds-canvas-toolbar-divider toolbar-view" role="separator" aria-orientation="vertical" />
      )}
      <div className="ds-canvas-toolbar-group toolbar-view" role="group" aria-label="View">
        {view.map(button)}
      </div>
      {layers.length > 0 && (
        <>
          <span className="ds-canvas-toolbar-divider" role="separator" aria-orientation="vertical" />
          <div className="ds-canvas-toolbar-group" role="group" aria-label="Layers">
            {layers.map(button)}
          </div>
        </>
      )}
      <span className="ds-canvas-toolbar-divider" role="separator" aria-orientation="vertical" />
      <AxisPicker
        state={state}
        actions={actions}
        tabIndex={tabStop === "axes" ? 0 : -1}
        onFocus={() => setStop("axes")}
      />
      {space && <SpaceAxes view={space} state={state} actions={actions} />}
    </div>
  );
}

const SPACE_SLOTS = ["First", "Second", "Third"] as const;

function SpaceAxes({ view, state, actions }: Props & { view: SpaceView }) {
  const sys = state.compiled!;
  const axes = spaceStates(view);
  const options = sys.states.map((st, i) => ({ value: `s${i}`, label: latexToPlain(st.latex) }));
  return (
    <div className="ds-canvas-toolbar-group space-axes" role="group" aria-label="3D axes">
      {axes.map((index, slot) => (
        <Select
          key={slot}
          size="sm"
          math
          auto
          value={`s${index}`}
          options={options}
          ariaLabel={`${SPACE_SLOTS[slot]} axis of the 3D view`}
          onChange={(v) => actions.setSpaceAxis(slot as 0 | 1 | 2, Number(v.slice(1)))}
        />
      ))}
    </div>
  );
}

function fitSpace(state: StudioState, view: SpaceView): Pick<SpaceView, "xRange" | "yRange" | "zRange"> | null {
  const sys = state.compiled;
  if (!sys) return null;
  const dim = sys.states.length;
  const p = state.portrait;
  const valid = portraitFits(sys, state.shapeId, p);
  const ranges = spaceStates(view).map((i): [number, number] | null => {
    const ext = valid ? stateExtent(p.res, dim, i) : null;
    let [lo, hi] = ext ?? [Infinity, -Infinity];
    const more = [
      ...state.ics.filter((ic) => ic.x0.length === dim && !ic.hidden).map((ic) => ic.x0[i]),
      ...(valid && state.layers.fixedPoints ? p.res.fixedPoints.map((fp) => fp.x[i]) : []),
    ];
    for (const v of more) {
      if (!Number.isFinite(v)) continue;
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
    if (!(lo <= hi)) return null;
    const pad = Math.max(0.1 * (hi - lo), 0.5);
    return [lo - pad, hi + pad];
  });
  if (ranges.some((r) => !r)) return null;
  const [xRange, yRange, zRange] = ranges as [number, number][];
  return { xRange, yRange, zRange };
}

function fitLyapunov(state: StudioState): { xRange: [number, number]; yRange: [number, number] } | null {
  return lyapunovFits(state.compiled, state.shapeId, state.lyapunov) ? fitLyapunovRanges(state.lyapunov.res) : null;
}

function fitOrbit(state: StudioState): { xRange: [number, number]; yRange: [number, number] } | null {
  const v = state.view;
  if (!isOrbit(v)) return null;
  const p = state.params[v.param];
  const y = orbitFits(state.compiled, state.shapeId, v, state.orbit) ? fitOrbitRange(state.orbit.res) : null;
  return p && y ? { xRange: [p.min, p.max], yRange: y } : null;
}

function fitView(state: StudioState): { xRange: [number, number]; yRange: [number, number] } | null {
  const sys = state.compiled;
  if (!sys) return null;
  const dim = sys.states.length;
  const { xAxis: rx } = state.view;
  if (isReturnMap(state.view) && rx.kind === "state") {
    const p = state.portrait;
    const vs = portraitFits(sys, state.shapeId, p) ? p.res.trajectories.flatMap((out) => orbitValues(out, dim, rx.index)) : [];
    const r = cobwebFit(vs);
    return r ? { xRange: r, yRange: [r[0], r[1]] } : null;
  }
  if (isCobweb(state.view)) {
    // The orbits and fixed points, as one square range on both axes (the diagonal stays at 45°).
    const vs: number[] = [];
    const p = state.portrait;
    for (const ic of state.ics) if (ic.x0.length === dim && !ic.hidden) vs.push(ic.x0[0]);
    if (portraitFits(sys, state.shapeId, p)) {
      if (state.layers.fixedPoints) for (const fp of p.res.fixedPoints) vs.push(fp.x[0]);
      for (const out of p.res.trajectories) vs.push(...orbitValues(out, dim, 0));
    }
    const r = cobwebFit(vs);
    return r ? { xRange: r, yRange: [r[0], r[1]] } : null;
  }
  const { xAxis, yAxis } = state.view;
  const xs: number[] = [];
  const ys: number[] = [];
  const p = state.portrait;
  const valid = portraitFits(sys, state.shapeId, p);
  for (const ic of state.ics) {
    if (ic.x0.length !== dim) continue;
    const t0 = sys.kind === "map" ? 0 : ic.t0;
    xs.push(xAxis.kind === "time" ? t0 : ic.x0[xAxis.index]);
    ys.push(yAxis.kind === "time" ? t0 : ic.x0[yAxis.index]);
  }
  if (valid && xAxis.kind === "state" && yAxis.kind === "state" && state.layers.fixedPoints) {
    for (const fp of p.res.fixedPoints) {
      xs.push(fp.x[xAxis.index]);
      ys.push(fp.x[yAxis.index]);
    }
  }
  // The start points and fixed points set each axis's scale for telling a blow-up from large states.
  const size = (vs: number[], axis: AxisRef) =>
    axis.kind === "time" ? Infinity : vs.reduce((m, v) => (Number.isFinite(v) ? Math.max(m, Math.abs(v)) : m), 0);
  const scale: [number, number] = [size(xs, xAxis), size(ys, yAxis)];
  const core = fitRanges(xs, ys, 0, scale);
  const window = (r: [number, number] | undefined, axis: AxisRef): [number, number] => {
    if (!r || axis.kind === "time") return [-Infinity, Infinity];
    const span = Math.max(r[1] - r[0], 1);
    return [r[0] - span / 4, r[1] + span / 4];
  };
  const wx = window(core?.xRange, xAxis);
  const wy = window(core?.yRange, yAxis);
  if (valid) {
    for (const out of p.res.trajectories) {
      for (let k = 0; k < out.t.length; k++) {
        const x = xAxis.kind === "time" ? out.t[k] : out.x[k * dim + xAxis.index];
        const y = yAxis.kind === "time" ? out.t[k] : out.x[k * dim + yAxis.index];
        if (x >= wx[0] && x <= wx[1] && y >= wy[0] && y <= wy[1]) {
          xs.push(x);
          ys.push(y);
        }
      }
    }
  }
  return fitRanges(xs, ys, 0.1, scale);
}

function menuItems(menu: HTMLElement | null): HTMLButtonElement[] {
  return [...(menu?.querySelectorAll<HTMLButtonElement>("[role=menuitemradio]") ?? [])];
}

function AxisPicker({
  state,
  actions,
  tabIndex,
  onFocus,
}: Props & { tabIndex: number; onFocus: () => void }) {
  const sys = state.compiled!;
  const isMap = sys.kind === "map";
  const [open, setOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const focusOnOpen = useRef<"current" | "first" | "last">("current");
  const menuId = useId();

  const section = isSection(state.view);
  const space = isSpace(state.view);
  const options: { key: string; value: AxisRef; latex: string; hint?: string }[] = [
    ...sys.states.map((_, i) => ({
      key: `s${i}`,
      value: { kind: "state", index: i } as AxisRef,
      latex: axisLatex({ kind: "state", index: i }, sys.states, sys.kind),
    })),
    ...(section
      ? []
      : [
          {
            key: "time",
            value: { kind: "time" } as AxisRef,
            latex: isMap ? "n" : "t",
            hint: isMap ? "iterate" : "time",
          },
        ]),
  ];
  const cobweb = isCobweb(state.view);
  const lyapunov = isLyapunov(state.view);
  const orbit = isOrbit(state.view) ? state.view : null;
  const viewOnly = cobweb || space || lyapunov || !!orbit;
  const { x: xLatex, y: yLatex } = lyapunov
    ? { x: isMap ? "n" : "t", y: "\\lambda" }
    : orbit
      ? { x: sys.params[orbit.param]?.latex ?? "", y: sys.states[orbit.yAxis.index]?.latex ?? "" }
      : viewAxisLabels(state.view, sys.states, sys.kind);
  const plain = (a: AxisRef) => latexToPlain(axisLatex(a, sys.states, sys.kind));
  const step = cobweb ? "n" : isReturnMap(state.view) ? "k" : null;
  const xPlain = lyapunov ? xLatex : orbit ? latexToPlain(xLatex) : step ? `${plain(state.view.xAxis)} ${step}` : plain(state.view.xAxis);
  const yPlain = lyapunov ? "λ" : orbit ? latexToPlain(yLatex) : step ? `${plain(state.view.yAxis)} ${step}+1` : plain(state.view.yAxis);
  const avail = viewChoiceAvailability(sys, systemNameOf(state));
  const current = viewChoice(state.view);
  const items = () => menuItems(menuRef.current);

  useEffect(() => {
    if (!open) return;
    const all = menuItems(menuRef.current);
    const where = focusOnOpen.current;
    // The current horizontal axis (the View group's choice in a cobweb, which has no axis groups).
    const checked = (el: HTMLButtonElement) => el.getAttribute("aria-checked") === "true";
    const onAxis = all.findIndex((el) => checked(el) && el.dataset.viewChoice === undefined);
    const current = onAxis >= 0 ? onAxis : all.findIndex(checked);
    all[where === "first" ? 0 : where === "last" ? all.length - 1 : Math.max(0, current)]?.focus();
    const close = (e: PointerEvent) => {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [open]);

  const openWith = (where: "current" | "first" | "last") => {
    focusOnOpen.current = where;
    setOpen(true);
  };
  const closeAndReturn = () => {
    setOpen(false);
    buttonRef.current?.focus();
  };

  const onMenuKey = (e: React.KeyboardEvent) => {
    const all = items();
    const i = all.findIndex((el) => el === document.activeElement);
    const move = (j: number) => all[((j % all.length) + all.length) % all.length]?.focus();
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(i + 1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(i - 1);
        break;
      case "Home":
        e.preventDefault();
        move(0);
        break;
      case "End":
        e.preventDefault();
        move(all.length - 1);
        break;
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        closeAndReturn();
        break;
      case "Tab":
        setOpen(false);
        break;
    }
  };

  const viewGroup = () => {
    const headId = `${menuId}-view`;
    return (
      <div role="group" aria-labelledby={headId} className="axes-menu-group">
        <div className="axes-menu-head caption" id={headId}>
          View
        </div>
        {viewChoicesFor(sys).map((c) => {
          const checked = c === current;
          const a = avail[c];
          return (
            <Fragment key={c}>
              <button
                type="button"
                role="menuitemradio"
                aria-checked={checked}
                aria-disabled={!a.ok || undefined}
                aria-describedby={a.ok ? undefined : `${menuId}-why-${c}`}
                tabIndex={-1}
                className="axes-menu-item"
                data-view-choice={c}
                onClick={() => {
                  if (!a.ok) return;
                  actions.setViewKind(c);
                  closeAndReturn();
                }}
              >
                <span className="axes-menu-label">{VIEW_CHOICE_LABELS[c]}</span>
                {checked && <IconCheck className="axes-menu-check" />}
              </button>
              {/* Why it can't be picked, under it (and read through aria-describedby). */}
              {!a.ok && (
                <p className="axes-menu-why caption" id={`${menuId}-why-${c}`}>
                  <MathProse text={a.reason} />
                </p>
              )}
            </Fragment>
          );
        })}
      </div>
    );
  };

  const group = (which: "x" | "y") => {
    const axis = which === "x" ? state.view.xAxis : state.view.yAxis;
    const headId = `${menuId}-${which}`;
    return (
      <div role="group" aria-labelledby={headId} className="axes-menu-group">
        <div className="axes-menu-head caption" id={headId}>
          {which === "x" ? "Horizontal" : "Vertical"}
        </div>
        {options.map((o) => {
          const checked = sameAxis(axis, o.value);
          return (
            <button
              key={o.key}
              type="button"
              role="menuitemradio"
              aria-checked={checked}
              aria-label={`${latexToPlain(o.latex)}${o.hint ? ` (${o.hint})` : ""}`}
              tabIndex={-1}
              className="axes-menu-item"
              onClick={() => {
                actions.setAxis(which, o.value);
                closeAndReturn();
              }}
            >
              <span className="axes-menu-dir caption" aria-hidden="true">
                {which === "x" ? "→" : "↑"}
              </span>
              <span className="math">
                <Tex latex={o.latex} />
              </span>
              {o.hint && <span className="axes-menu-hint caption">{o.hint}</span>}
              {checked && <IconCheck className="axes-menu-check" />}
            </button>
          );
        })}
      </div>
    );
  };

  return (
    <div className="axes-picker" ref={wrapRef}>
      <button
        ref={buttonRef}
        type="button"
        data-key="axes"
        className="ds-canvas-toolbar-axes"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label={
          space
            ? "View: 3D"
            : `${lyapunov ? "Lyapunov spectrum" : orbit ? "Orbit diagram" : cobweb ? "Cobweb" : section ? "Section" : "Axes"}: ${xPlain} horizontal, ${yPlain} vertical`
        }
        data-tooltip={viewOnly ? "Choose the view" : "Choose the view and axes"}
        data-tooltip-desc={
          lyapunov || orbit
            ? isMap
              ? "Phase plane, time series, cobweb, Lyapunov spectrum or orbit diagram."
              : "Phase plane, time series, section, 3D or Lyapunov spectrum."
            : cobweb
            ? "Phase plane, time series, cobweb, Lyapunov spectrum or orbit diagram."
            : space
              ? "Phase plane, time series, section, 3D or Lyapunov spectrum; the three states are picked beside it."
              : section
                ? "Phase plane, time series, section, 3D or Lyapunov spectrum; a state on each axis, or one state on both for the return map."
                : isMap
                  ? "Phase plane, time series, cobweb, Lyapunov spectrum or orbit diagram; each state, or time, on either axis."
                  : "Phase plane, time series, section, 3D or Lyapunov spectrum; each state, or time, on either axis."
        }
        tabIndex={tabIndex}
        onFocus={onFocus}
        onClick={() => (open ? setOpen(false) : openWith("current"))}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            openWith("current");
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            openWith("last");
          }
        }}
      >
        {space ? (
          <span className="ds-canvas-toolbar-dir" aria-hidden="true">
            3D
          </span>
        ) : (
          <>
            <span className="ds-canvas-toolbar-dir" aria-hidden="true">
              →
            </span>
            <span className="math">
              <Tex latex={xLatex} />
            </span>
            <span className="ds-canvas-toolbar-dir" aria-hidden="true">
              ↑
            </span>
            <span className="math">
              <Tex latex={yLatex} />
            </span>
          </>
        )}
        <IconChevronDown />
      </button>
      {open && (
        <div
          role="menu"
          id={menuId}
          aria-label="View and axes"
          className="axes-menu"
          ref={menuRef}
          onKeyDown={onMenuKey}
        >
          {viewGroup()}
          {!viewOnly && (
            <>
              <div role="separator" className="axes-menu-sep" />
              {group("x")}
              <div role="separator" className="axes-menu-sep" />
              {group("y")}
            </>
          )}
        </div>
      )}
    </div>
  );
}
