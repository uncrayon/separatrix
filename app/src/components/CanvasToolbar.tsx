// The strip above the canvas: what is on each axis, which layers are drawn,
// and the time span — all smart defaults, all one click to change.

import type { AxisRef } from "../api/types";
import { Tex } from "../math/Tex";
import { axisLatex } from "../math/format";
import type { StudioActions, StudioState } from "../state/useStudio";
import { MenuButton, NumberField, ToggleChip } from "./controls";
import { IconReset } from "./icons";

interface Props {
  state: StudioState;
  actions: StudioActions;
}

const axisKey = (a: AxisRef) => (a.kind === "time" ? "time" : `s${a.index}`);

export function CanvasToolbar({ state, actions }: Props) {
  const sys = state.compiled;
  if (!sys) return <div className="canvas-toolbar" />;
  const isMap = sys.kind === "map";
  const timeName = isMap ? "n" : "t";
  const options: { key: string; value: AxisRef; label: React.ReactNode; hint?: React.ReactNode }[] = [
    ...sys.states.map((s, i) => ({
      key: `s${i}`,
      value: { kind: "state", index: i } as AxisRef,
      label: <Tex latex={s.latex} />,
    })),
    {
      key: "time",
      value: { kind: "time" },
      label: <Tex latex={timeName} />,
      hint: isMap ? "iterate" : "time",
    },
  ];
  const picker = (which: "x" | "y") => {
    const axis = which === "x" ? state.view.xAxis : state.view.yAxis;
    return (
      <MenuButton
        ariaLabel={which === "x" ? "Horizontal axis" : "Vertical axis"}
        label={
          <>
            <span className="axis-dir">{which === "x" ? "→" : "↑"}</span>
            <Tex latex={axisLatex(axis, sys.states, sys.kind)} />
          </>
        }
        options={options}
        selectedKey={axisKey(axis)}
        onSelect={(a) => actions.setAxis(which, a)}
      />
    );
  };
  const timeOnAxis = state.view.xAxis.kind === "time" || state.view.yAxis.kind === "time";

  return (
    <div className="canvas-toolbar">
      <div className="toolbar-group" aria-label="Axes">
        <span className="toolbar-label">Axes</span>
        {picker("x")}
        {picker("y")}
      </div>
      <div className="toolbar-group" aria-label="Layers">
        {!isMap && (
          <ToggleChip
            on={state.layers.field}
            onChange={(field) => actions.setLayers({ field })}
            title="Direction field arrows (normalised length)"
          >
            Field
          </ToggleChip>
        )}
        {!isMap && (
          <ToggleChip
            on={state.layers.nullclines}
            onChange={(nullclines) => actions.setLayers({ nullclines })}
            title="Curves where one component of the vector field vanishes"
          >
            Nullclines
          </ToggleChip>
        )}
        <ToggleChip
          on={state.layers.fixedPoints}
          onChange={(fixedPoints) => actions.setLayers({ fixedPoints })}
        >
          Fixed points
        </ToggleChip>
        {!timeOnAxis || isMap ? (
          <label
            className="chip edit"
            title={isMap ? "Number of iterates" : "Integrate for this long from each start point"}
          >
            <Tex latex={isMap ? "N =" : "T ="} />
            <NumberField
              value={state.solver.tEnd}
              onCommit={(tEnd) => tEnd > 0 && actions.setSolver({ tEnd })}
              digits={4}
              step={isMap ? 10 : 1}
              ariaLabel={isMap ? "Number of iterates" : "Time span"}
            />
          </label>
        ) : (
          <span
            className="chip static"
            title="With time on an axis, trajectories cover exactly the visible time window"
          >
            <Tex latex="t" /> follows the view
          </span>
        )}
      </div>
      <div className="toolbar-spacer" />
      <button
        type="button"
        className="icon-btn"
        onClick={actions.resetView}
        title="Reset view (or double-click the plot)"
        aria-label="Reset view"
      >
        <IconReset size={16} />
      </button>
    </div>
  );
}
