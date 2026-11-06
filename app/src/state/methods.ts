import type { Method } from "../api/types";

/** What a method needs from the system before the core will run it. */
export type MethodNeeds = "any" | "separableHamiltonian";

export interface MethodInfo {
  /** The full name: the method picker, tooltips and notices. */
  label: string;
  /** The status bar's name. */
  short: string;
  /** One sentence under the method picker. */
  about: string;
  /** Chooses its own step from rtol and atol; the others take the step h. */
  adaptive: boolean;
  needs: MethodNeeds;
}

/** Every `dyn_solve::Method`, in the order the method picker lists them. */
export const METHODS: Record<Method, MethodInfo> = {
  dopri5: {
    label: "Dormand–Prince 5(4)",
    short: "DOPRI5",
    about: "Dormand–Prince 5(4): adaptive step with error control.",
    adaptive: true,
    needs: "any",
  },
  rk4: {
    label: "Runge–Kutta 4",
    short: "RK4",
    about: "Classical Runge–Kutta: fourth order, fixed step.",
    adaptive: false,
    needs: "any",
  },
  heun: {
    label: "Heun",
    short: "Heun",
    about: "Heun (improved Euler): second order, fixed step.",
    adaptive: false,
    needs: "any",
  },
  euler: {
    label: "Euler",
    short: "Euler",
    about: "Forward Euler: first order, fixed step.",
    adaptive: false,
    needs: "any",
  },
};
