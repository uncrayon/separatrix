// Keep the keyboard user's place when the element they used goes away.

/** Focus what `find` returns once React has committed and the browser laid out. */
export function focusLater(find: () => HTMLElement | null | undefined): void {
  requestAnimationFrame(() => find()?.focus());
}

/**
 * Where focus goes after deleting row `index` of `ids`: the next row, else the
 * previous one. Returns the id to focus, or null when the list will be empty.
 */
export function neighbourAfterDelete<T>(ids: T[], index: number): T | null {
  return ids[index + 1] ?? ids[index - 1] ?? null;
}

/**
 * After deleting a trajectory in `panel`, focus the next row's select button,
 * else the previous row's, else the section's "Add trajectory" action (or the
 * plot, in a panel without one).
 */
export function focusAfterTrajectoryDelete(panel: ".panel-left" | ".panel-right", ids: number[], id: number): void {
  const next = neighbourAfterDelete(ids, ids.indexOf(id));
  focusLater(
    () =>
      (next !== null
        ? document.querySelector<HTMLElement>(`${panel} [data-ic-id="${next}"] .ds-traj-select`)
        : null) ??
      document.querySelector<HTMLElement>(`${panel} [data-traj-add]`) ??
      document.querySelector<HTMLElement>(".plot-canvas"),
  );
}
