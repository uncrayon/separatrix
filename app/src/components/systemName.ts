import { UNTITLED_SYSTEM, systemTitle } from "../state/model";
import { templateById } from "../templates";
import type { StudioState } from "../state/useStudio";

/**
 * The system's name for a sentence about it ("Hénon map has 2 states"): its
 * template's title while the equations are unedited, otherwise undefined
 * (the sentence then says "This map").
 */
export function systemNameOf(state: Pick<StudioState, "sourceTemplateId" | "equations" | "format">): string | undefined {
  const tpl = state.sourceTemplateId ? templateById(state.sourceTemplateId) : undefined;
  const t = systemTitle(
    tpl,
    state.equations.map((l) => l.source),
    state.format,
  );
  return t.edited || t.name === UNTITLED_SYSTEM ? undefined : t.name;
}
