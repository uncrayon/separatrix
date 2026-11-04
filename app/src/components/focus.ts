export function focusLater(find: () => HTMLElement | null | undefined): void {
  requestAnimationFrame(() => find()?.focus());
}

export function neighbourAfterDelete<T>(ids: T[], index: number): T | null {
  return ids[index + 1] ?? ids[index - 1] ?? null;
}

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
