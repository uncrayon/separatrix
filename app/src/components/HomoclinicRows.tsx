import { CurveSample } from "../canvas/CurvesCanvas";
import { dec, refusalText, plainSubscripted } from "../math/format";
import { CODIM2_KINDS, type PlanePointRow } from "../state/curves";
import {
  HOMOCLINIC_LABEL,
  HOMOCLINIC_NAME,
  computingText,
  homoclinicLines,
  hasHomoclinicSeed,
  homoclinicSeed,
} from "../state/homoclinic";
import { isCurves } from "../state/model";
import type { StudioActions, StudioState } from "../state/useStudio";
import { Button, IconButton } from "./controls";
import { IconFollow, IconTrash } from "./icons";

interface Props {
  state: StudioState;
  actions: StudioActions;
}

const BT = CODIM2_KINDS.bogdanovTakens.label;

/** The homoclinic rows of the Curves list, after the ordinary curves'. */
export function HomoclinicRowItems({ state, actions }: Props) {
  const sys = state.compiled;
  const view = state.view;
  if (!sys || !isCurves(view)) return null;
  const names: [string, string] = [plainSubscripted(sys.params[view.across].latex), plainSubscripted(sys.params[view.up].latex)];
  return (
    <>
      {state.homoclinicRows.map((row) => {
        const seed = view.curves.homoclinic[row.job.meta.seed];
        const from = `From ${BT} at ${names[0]} = ${dec(seed.params[view.across], 4)}, ${names[1]} = ${dec(seed.params[view.up], 4)}`;
        const lines = row.status === "done" ? homoclinicLines(row.res.curve, names) : [];
        return (
          <li
            key={row.job.meta.key}
            className="ds-traj-row inspector-curve-row"
            data-homoclinic-status={row.status}
          >
            <span className="ds-traj-select inspector-branch">
              <span className="ds-traj-line">
                <span className="inspector-curve-swatch">
                  <CurveSample kind="homoclinic" />
                </span>
                <span className="body-strong">{HOMOCLINIC_NAME}</span>
              </span>
              <span className="ds-traj-meta">
                <span className="numeric-sm">{from}</span>
                {row.status === "computing" && (
                  <span className="caption" role="status" data-homoclinic-progress="">
                    {computingText(row.progress, names)}
                  </span>
                )}
                {row.status === "failed" && <span className="caption inspector-branch-error">{refusalText(row.error)}</span>}
                {lines.map((line) => (
                  <span className="caption" key={line}>
                    {line}
                  </span>
                ))}
              </span>
            </span>
            <span className="ds-traj-actions">
              {row.status === "computing" ? (
                <Button
                  size="sm"
                  aria-label={`Cancel the ${HOMOCLINIC_NAME.toLowerCase()} from ${BT}`}
                  onClick={() => actions.removeHomoclinic(row.job.meta.seed)}
                >
                  Cancel
                </Button>
              ) : (
                <IconButton
                  size="sm"
                  label={`Remove the ${HOMOCLINIC_NAME.toLowerCase()} from ${BT}`}
                  tipAlign="end"
                  onClick={() => actions.removeHomoclinic(row.job.meta.seed)}
                >
                  <IconTrash />
                </IconButton>
              )}
            </span>
          </li>
        );
      })}
    </>
  );
}

/**
 * "Follow homoclinic" on a Bogdanov–Takens row of a flow, under the row's
 * sentence that a homoclinic curve starts there; nothing on any other row.
 */
export function FollowHomoclinic({ state, actions, point }: Props & { point: PlanePointRow }) {
  const sys = state.compiled;
  const view = state.view;
  if (!sys || sys.kind === "map" || point.kind !== "bogdanovTakens" || !isCurves(view)) return null;
  const seed = homoclinicSeed(point.x, state.params.map((p) => p.value), [view.across, view.up], point.p);
  const followed = hasHomoclinicSeed(view.curves.homoclinic, seed);
  return followed ? (
    <span className="caption" data-homoclinic-followed="">
      Followed as a homoclinic curve ({HOMOCLINIC_LABEL})
    </span>
  ) : (
    <Button size="sm" icon={<IconFollow />} onClick={() => actions.followHomoclinic(point.x, point.p)}>
      Follow homoclinic
    </Button>
  );
}
