import type { CompileResponse, Method } from "../api/types";

export type MethodNeeds = "any" | "separableHamiltonian";

export interface MethodInfo {
  label: string;
  short: string;
  about: string;
  adaptive: boolean;
  needs: MethodNeeds;
}

export const DEFAULT_METHOD: Method = "dopri5";

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

export const STIFF_METHOD: Method = "rosenbrock23";

export const STIFF_HINT = "These steps look stiff: Dormand–Prince is limited by stability, not accuracy.";

export function symplecticRefusal(sys: Pick<CompileResponse, "kind" | "hamiltonian" | "separable">): string | null {
  if (sys.kind === "map") return "A map is iterated exactly, so it has no integration method.";
  if (!sys.hamiltonian) {
    return "Störmer–Verlet and Yoshida 4 need a Hamiltonian H(q, p); this system is given by its equations of motion.";
  }
  if (!sys.separable) return "Störmer–Verlet and Yoshida 4 need H(q, p) to split as T(p) + V(q).";
  return null;
}

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

export function energyDriftText(maxAbsDrift: number): string {
  if (maxAbsDrift === 0) return "|ΔH| max = 0";
  const [m, e] = maxAbsDrift.toExponential(1).split("e");
  const exp = String(Number(e)).replace(/./g, (c) => SUPERSCRIPT[c] ?? c);
  return `|ΔH| max = ${m} × 10${exp}`;
}
