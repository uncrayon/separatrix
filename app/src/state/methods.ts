import type { CompileResponse, Method } from "../api/types";

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

/** The method a new studio starts with, and what an unknown saved method falls back to. */
export const DEFAULT_METHOD: Method = "dopri5";

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
  verlet: {
    label: "Störmer–Verlet",
    short: "Störmer–Verlet",
    about: "Störmer–Verlet (leapfrog): symplectic, second order, fixed step. The energy error stays bounded.",
    adaptive: false,
    needs: "separableHamiltonian",
  },
  yoshida4: {
    label: "Yoshida 4",
    short: "Yoshida 4",
    about: "Yoshida's composition of three leapfrog steps: symplectic, fourth order, fixed step.",
    adaptive: false,
    needs: "separableHamiltonian",
  },
  rosenbrock23: {
    label: "Rosenbrock 2(3)",
    short: "Rosenbrock 2(3)",
    about: "Rosenbrock 2(3), as MATLAB's ode23s: implicit and adaptive, for stiff systems. Uses the exact Jacobian.",
    adaptive: true,
    needs: "any",
  },
};

/** The method the stiffness hint offers. */
export const STIFF_METHOD: Method = "rosenbrock23";

/** What the inspector and the status bar say when Dormand–Prince's steps look stiff (`TrajectoryOut.stiff`). */
export const STIFF_HINT = "These steps look stiff: Dormand–Prince is limited by stability, not accuracy.";

/**
 * Why the symplectic methods can't run `sys`, in the core's words; null when
 * they can. They step H(q, p) = T(p) + V(q) as kicks and drifts.
 */
export function symplecticRefusal(sys: Pick<CompileResponse, "kind" | "hamiltonian" | "separable">): string | null {
  if (sys.kind === "map") return "A map is iterated exactly, so it has no integration method.";
  if (!sys.hamiltonian) {
    return "Störmer–Verlet and Yoshida 4 need a Hamiltonian H(q, p); this system is given by its equations of motion.";
  }
  if (!sys.separable) return "Störmer–Verlet and Yoshida 4 need H(q, p) to split as T(p) + V(q).";
  return null;
}

/** Whether `method` can integrate `sys`. */
export function methodFits(
  method: Method,
  sys: Pick<CompileResponse, "kind" | "hamiltonian" | "separable">,
): boolean {
  return METHODS[method].needs === "any" || symplecticRefusal(sys) === null;
}

const SUPERSCRIPT: Record<string, string> = {
  "-": "⁻",
  "0": "⁰",
  "1": "¹",
  "2": "²",
  "3": "³",
  "4": "⁴",
  "5": "⁵",
  "6": "⁶",
  "7": "⁷",
  "8": "⁸",
  "9": "⁹",
};

/** The energy readout, `|ΔH| max = 3.1 × 10⁻⁷`: two significant digits. */
export function energyDriftText(maxAbsDrift: number): string {
  if (maxAbsDrift === 0) return "|ΔH| max = 0";
  const [m, e] = maxAbsDrift.toExponential(1).split("e");
  const exp = String(Number(e)).replace(/./g, (c) => SUPERSCRIPT[c] ?? c);
  return `|ΔH| max = ${m} × 10${exp}`;
}
