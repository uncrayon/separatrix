import { useSyncExternalStore } from "react";

let showEigen = false;
const listeners = new Set<() => void>();

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
