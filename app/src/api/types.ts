// IPC contract between the webview and the Rust core.
// Mirrors app/src-tauri/src/ipc.rs. See docs/contracts.md.

export type InputFormat = "latex" | "text";
export type SystemKind = "ode" | "map";
export type Severity = "error" | "warning" | "info";

export interface Diagnostic {
  severity: Severity;
  message: string;
  /** Byte offsets into the source string. */
  span?: [number, number];
}

export interface SymbolInfo {
  name: string;
  latex: string;
  /** A lower derivative introduced by order reduction (x' for x''). */
  derived?: boolean;
}

export interface ParamInfo extends SymbolInfo {
  default: number;
}

export interface CompileRequest {
  source: string;
  format: InputFormat;
}

export interface CompileResponse {
  ok: boolean;
  /** Handle for later requests; present when ok. */
  systemId?: number;
  kind?: SystemKind;
  hamiltonian: boolean;
  autonomous: boolean;
  states: SymbolInfo[];
  params: ParamInfo[];
  /** First-order system for display, e.g. "\\dot{x} = y". */
  equationsLatex: string[];
  diagnostics: Diagnostic[];
}

export type AxisRef = { kind: "state"; index: number } | { kind: "time" };

export type Method = "euler" | "heun" | "rk4" | "dopri5";

export interface SolverOptions {
  method: Method;
  rtol: number;
  atol: number;
  /** Fixed step for euler/heun/rk4; initial step for dopri5 (0 = automatic). */
  h: number;
  /** Largest allowed step for dopri5 (0 or absent = no limit). */
  hMax?: number;
  maxSteps: number;
}

export interface TrajectoryRequest {
  /** Full state vector. */
  x0: number[];
  /** Start time (ODE) or 0 (map). */
  t0: number;
  /** End time (ODE, may be < t0 for backward) or number of iterates (map). */
  t1: number;
}

export interface PortraitRequest {
  requestId: number;
  systemId: number;
  params: number[];
  xAxis: AxisRef;
  yAxis: AxisRef;
  xRange: [number, number];
  yRange: [number, number];
  /** Values used for state variables not on an axis (the slice). Length = dim. */
  baseState: number[];
  /** Time at which a non-autonomous field is drawn. */
  fieldTime: number;
  /** Grid size for the direction field; 0 = no field. */
  fieldGrid: number;
  nullclines: boolean;
  fixedPoints: boolean;
  trajectories: TrajectoryRequest[];
  solver: SolverOptions;
  /** Dense samples per trajectory. */
  samples: number;
}

export interface Arrow {
  x: number;
  y: number;
  /** Raw field components in plot coordinates (time axis component = 1). */
  u: number;
  v: number;
}

export interface NullclineOut {
  /** Which equation vanishes along these curves. */
  state: number;
  polylines: [number, number][][];
}

export type Classification =
  | "saddle"
  | "stable node"
  | "unstable node"
  | "stable spiral"
  | "unstable spiral"
  | "center"
  | "stable star"
  | "unstable star"
  | "stable degenerate node"
  | "unstable degenerate node"
  | "non-isolated"
  | "sink"
  | "source"
  | "saddle (n-D)"
  | "non-hyperbolic";

export interface FixedPointOut {
  x: number[];
  /** [re, im] pairs. */
  eigenvalues: [number, number][];
  /**
   * One entry per eigenvalue, same order. Real eigenvalue: its unit
   * eigenvector, sign-normalised so the largest-magnitude component is
   * positive. Complex eigenvalue: the real part of its eigenvector. `null`
   * for the missing direction of a defective eigenvalue (a degenerate node
   * has one vector and one `null`). Stars get an orthonormal basis.
   */
  eigenvectors: (number[] | null)[];
  /**
   * Imaginary parts of the eigenvectors of complex eigenvalues; `null` for
   * real ones. For a pair α ± iω, re and im are orthogonal with
   * |re|² + |im|² = 1 and |re| ≥ |im|: the major and minor semi-axes of the
   * linearised orbits' ellipses (the conjugate has im negated).
   */
  eigenvectorsIm: (number[] | null)[];
  classification: Classification;
  stable: boolean;
}

export type Status = "completed" | "blewUp" | "maxSteps" | "stepTooSmall" | "nonFinite";

export interface TrajectoryOut {
  t: number[];
  /** Row-major, length t.length * dim. */
  x: number[];
  status: Status;
  stats: { accepted: number; rejected: number; evals: number };
}

export interface PortraitResponse {
  requestId: number;
  arrows: Arrow[];
  nullclines: NullclineOut[];
  fixedPoints: FixedPointOut[];
  trajectories: TrajectoryOut[];
  /** Wall time spent in Rust. */
  elapsedMs: number;
}
