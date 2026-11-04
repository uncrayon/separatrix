import { useEffect, useId, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import type { AxisRef } from "../api/types";
import { fitRanges } from "../canvas/viewport";
import { Tex } from "../math/Tex";
import { axisLabelLatex, axisLatex, latexToPlain, sameAxis } from "../math/format";
import type { StudioActions, StudioState } from "../state/useStudio";
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
  const timeOnAxis = state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time";

  const zoomToFit = () => {
    const fit = fitView(state);
    if (fit) actions.setView({ ...state.view, ...fit });
    else actions.resetView();
  };

  const view: IconButtonSpec[] = [
    {
      key: "fit",
      label: "Zoom to fit",
      tip: "Zoom to fit the trajectories",
      icon: <IconZoomFit />,
      onClick: zoomToFit,
    },
    {
      key: "reset",
      label: "Reset view",
      tip: "Reset view (or double-click the plot)",
      icon: <IconResetView />,
      onClick: actions.resetView,
    },
  ];
  const layers: IconButtonSpec[] = [
    ...(!isMap
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
    ...(!isMap && (!timeOnAxis || sys.states.length === 1)
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

  const keys = [
    ...TOOLS.map((t) => t.key as string),
    ...view.map((b) => b.key),
    ...layers.map((b) => b.key),
    "axes",
  ];
  const tabStop = keys.includes(stop) && !view.some((b) => b.key === stop) ? stop : tool;

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
      <div className="ds-canvas-toolbar-group" role="group" aria-label="Tool">
        {TOOLS.map((t) =>
          button({
            ...t,
            tool: true,
            pressed: tool === t.key,
            onClick: () => setCanvasTool(t.key),
          }),
        )}
      </div>
      <span className="ds-canvas-toolbar-divider toolbar-view" role="separator" aria-orientation="vertical" />
      <div className="ds-canvas-toolbar-group toolbar-view" role="group" aria-label="View">
        {view.map(button)}
      </div>
      <span className="ds-canvas-toolbar-divider" role="separator" aria-orientation="vertical" />
      <div className="ds-canvas-toolbar-group" role="group" aria-label="Layers">
        {layers.map(button)}
      </div>
      <span className="ds-canvas-toolbar-divider" role="separator" aria-orientation="vertical" />
      <AxisPicker
        state={state}
        actions={actions}
        tabIndex={tabStop === "axes" ? 0 : -1}
        onFocus={() => setStop("axes")}
      />
    </div>
  );
}

function fitView(state: StudioState): { xRange: [number, number]; yRange: [number, number] } | null {
  const sys = state.compiled;
  if (!sys) return null;
  const dim = sys.states.length;
  const { xAxis, yAxis } = state.view;
  const xs: number[] = [];
  const ys: number[] = [];
  const p = state.portrait;
  const valid = !!p && p.meta.systemId === sys.systemId;
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
  const core = fitRanges(xs, ys);
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
  return fitRanges(xs, ys, 0.1);
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

  const options: { key: string; value: AxisRef; latex: string; hint?: string }[] = [
    ...sys.states.map((_, i) => ({
      key: `s${i}`,
      value: { kind: "state", index: i } as AxisRef,
      latex: axisLatex({ kind: "state", index: i }, sys.states, sys.kind),
    })),
    {
      key: "time",
      value: { kind: "time" } as AxisRef,
      latex: isMap ? "n" : "t",
      hint: isMap ? "iterate" : "time",
    },
  ];
  const xLatex = axisLabelLatex(state.view.xAxis, state.view.yAxis, sys.states, sys.kind);
  const yLatex = axisLabelLatex(state.view.yAxis, state.view.xAxis, sys.states, sys.kind);
  const xPlain = latexToPlain(axisLatex(state.view.xAxis, sys.states, sys.kind));
  const yPlain = latexToPlain(axisLatex(state.view.yAxis, sys.states, sys.kind));
  const items = () => menuItems(menuRef.current);

  useEffect(() => {
    if (!open) return;
    const all = menuItems(menuRef.current);
    const where = focusOnOpen.current;
    const current = all.findIndex((el) => el.getAttribute("aria-checked") === "true");
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
        aria-label={`Axes: ${xPlain} horizontal, ${yPlain} vertical`}
        data-tooltip="Choose the axes"
        data-tooltip-desc="Each state, or time, on either axis."
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
        <IconChevronDown />
      </button>
      {open && (
        <div
          role="menu"
          id={menuId}
          aria-label="Axes"
          className="axes-menu"
          ref={menuRef}
          onKeyDown={onMenuKey}
        >
          {group("x")}
          <div role="separator" className="axes-menu-sep" />
          {group("y")}
        </div>
      )}
    </div>
  );
}
