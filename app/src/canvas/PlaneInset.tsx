import { useEffect, useMemo, useRef } from "react";
import { drawThumb } from "../components/TemplateThumb";
import { INSET_ASPECT, insetCaption, insetParamsText, type InsetState } from "../state/inset";

import type { Corner } from "./curves";

/** Width of the inset as a fraction of the plot's, within these bounds in CSS px. */
const INSET_FRACTION = 0.3;
const INSET_MIN = 180;
const INSET_MAX = 280;
/** The inset's caption and padding below its plot, for placing it, in CSS px. */
export const INSET_CAPTION_H = 64;
/** The inset's distance from the plot's edges, `--space-3`, in CSS px. */
export const INSET_MARGIN = 12;

export const insetWidth = (plotWidth: number) => Math.round(Math.min(INSET_MAX, Math.max(INSET_MIN, INSET_FRACTION * plotWidth)));

interface Props {
  inset: InsetState;
  /** The plain names of the across and up parameters. */
  names: readonly [string, string];
  /** The across and up parameters' indices. */
  shown: readonly [number, number];
  /** Computed at other parameters than the current ones: drawn faded until the next result arrives. */
  stale: boolean;
  corner: Corner;
  width: number;
  /** Distance from the plot's top edge, clear of the toolbar, when the inset sits at the top. */
  top: number;
}

/** The live inset: a small Phase view (flows) or orbit (maps) at the crosshair, with a caption. */
export function PlaneInset({ inset, names, shown, stale, corner, width, top }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  const { res, meta } = inset;
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const thumb = { kind: meta.kind, dim: meta.dim, view: meta.view, res, icOf: meta.colors };
    const redraw = () => drawThumb(canvas, thumb);
    redraw();
    const mo = new MutationObserver(redraw);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => mo.disconnect();
  }, [res, meta, width]);

  const paramsText = insetParamsText(
    names,
    shown.map((i) => meta.params[i]),
  );
  const caption = useMemo(() => insetCaption(res, meta), [res, meta]);
  const map = meta.kind === "map";
  return (
    <figure
      className={`plane-inset${stale ? " is-stale" : ""}`}
      style={{ width, ["--inset-top" as string]: `${top}px` }}
      data-corner={corner}
      data-plane-inset=""
      aria-label={map ? "Live inset: orbit at the crosshair" : "Live inset: phase portrait at the crosshair"}
    >
      <canvas
        ref={ref}
        className="plane-inset-canvas"
        style={{ aspectRatio: INSET_ASPECT }}
        role="img"
        aria-label={`${map ? "Orbit" : "Phase portrait"} at ${paramsText}. ${caption}`}
      />
      <figcaption className="caption plane-inset-caption">
        <span className="numeric-sm plane-inset-params">{paramsText}</span>
        <br />
        <span data-inset-caption="">{caption}</span>
      </figcaption>
    </figure>
  );
}
