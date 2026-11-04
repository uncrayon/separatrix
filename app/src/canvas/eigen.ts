// Eigenvector directions through fixed points ("Show eigenvectors", design
// system: FixedPoint `.ds-fp-eigen`).
//
// The switch sits in the Inspector's fixed-point detail and the plot draws the
// lines, so the choice is shared through this tiny store, as the canvas tool
// is (one plot per window). It is a view preference, like the tool: it is not
// saved with the system. The pure eigenvector helpers live in eigenvectors.ts
// (no React, so node --test can import them) and are re-exported here.

import { useSyncExternalStore } from "react";

let showEigen = false;
const listeners = new Set<() => void>();

/** Turn the eigenvector lines on or off for every fixed point. */
export function setShowEigenvectors(on: boolean): void {
  if (on === showEigen) return;
  showEigen = on;
  listeners.forEach((l) => l());
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

/** Whether "Show eigenvectors" is on (off by default). */
export function useShowEigenvectors(): boolean {
  return useSyncExternalStore(subscribe, () => showEigen);
}

export {
  displayEigenvalues,
  hasComplexEigenvalue,
  numberedEigenvalues,
  othersHaveRealEigenvectors,
  realEigenvectors,
  realMask,
  type EigenFlow,
  type NumberedEigenvalue,
  type RealEigenvector,
} from "./eigenvectors";
