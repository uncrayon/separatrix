// Generated from app/src-tauri/src/ipc.rs by `UPDATE_IPC_BINDINGS=1 cargo test -p separatrix typescript_bindings`.

export type InputFormat = "latex" | "text";

export type SystemKind = "ode" | "map";

export type Severity = "error" | "warning" | "info";

export type Method = "euler" | "heun" | "rk4" | "dopri5";

export type Status = "completed" | "blewUp" | "maxSteps" | "stepTooSmall" | "nonFinite";

export type Stats = { 
/**
 * Accepted steps (map iterations for [`iterate_map`]).
 */
accepted: number, 
/**
 * Rejected steps (adaptive methods only).
 */
rejected: number, 
/**
 * Calls to [`VectorField::eval`].
 */
evals: number, };

export type Classification = "saddle" | "stable node" | "unstable node" | "stable spiral" | "unstable spiral" | "center" | "stable star" | "unstable star" | "stable degenerate node" | "unstable degenerate node" | "non-isolated" | "sink" | "source" | "saddle (n-D)" | "non-hyperbolic";

export type CompileRequest = { 
/**
 * The equations, one per line (the UI joins its non-empty lines with `"\n"`).
 */
source: string, 
/**
 * LaTeX (from MathLive) or plain text.
 */
format: InputFormat, };

export type Diagnostic = { 
/**
 * Error, warning or info.
 */
severity: Severity, 
/**
 * Plain-language explanation.
 */
message: string, 
/**
 * Byte offsets `[start, end]` into `CompileRequest::source`; absent when
 * the message is about the system as a whole.
 */
span?: [number, number], };

export type StateInfo = { 
/**
 * Identity, e.g. `"theta'"`.
 */
name: string, 
/**
 * Display form, e.g. `"\\dot{\\theta}"`.
 */
latex: string, 
/**
 * A lower derivative introduced by reducing a higher-order equation to
 * first order (`theta'` for `theta'' = ...`).
 */
derived: boolean, };

export type ParamInfo = { 
/**
 * Identity, e.g. `"sigma"`.
 */
name: string, 
/**
 * Display form, e.g. `"\\sigma"`.
 */
latex: string, 
/**
 * Value used when the user has not moved the slider.
 */
default: number, };

export type CompileResponse = { 
/**
 * `true` when the system compiled (there may still be warnings).
 */
ok: boolean, 
/**
 * Handle for `compute_portrait`; present when `ok`.
 */
systemId?: number, 
/**
 * Flow or map; present when `ok`.
 */
kind?: SystemKind, 
/**
 * Entered as `H(q, p) = ...`.
 */
hamiltonian: boolean, 
/**
 * `t` (or `n`) does not appear on a right-hand side.
 */
autonomous: boolean, 
/**
 * State variables in order (empty when not `ok`).
 */
states: Array<StateInfo>, 
/**
 * Parameters in order (empty when not `ok`).
 */
params: Array<ParamInfo>, 
/**
 * The first-order system for display, e.g. `"\\dot{x} = y"`.
 */
equationsLatex: Array<string>, 
/**
 * Errors and warnings, sorted by position.
 */
diagnostics: Array<Diagnostic>, };

export type ConvertRequest = { 
/**
 * The UI's lines, as typed (one entry per line; empty lines allowed).
 */
lines: Array<string>, 
/**
 * The format the lines are in.
 */
from: InputFormat, 
/**
 * The format to write them in.
 */
to: InputFormat, };

export type ConvertedLineOut = { 
/**
 * The line in the target format, or exactly as typed when `converted`
 * is false.
 */
source: string, 
/**
 * False when the line was kept as typed (it has an error, or the
 * target format can't write it exactly).
 */
converted: boolean, 
/**
 * True when the line was kept because it has an error in its own
 * format. A line kept with `error` false is valid but can't be written
 * exactly in the target format; it would mean something else (or
 * nothing) there, so the UI doesn't switch.
 */
error: boolean, 
/**
 * Why the line was kept as typed.
 */
note?: string, };

export type ConvertResponse = { 
/**
 * The converted lines.
 */
lines: Array<ConvertedLineOut>, };

export type AxisRef = { "kind": "state", 
/**
 * Index into the state vector.
 */
index: number, } | { "kind": "time" };

export type SolverOptions = { 
/**
 * Integration method.
 */
method: Method, 
/**
 * Relative tolerance (dopri5 only).
 */
rtol: number, 
/**
 * Absolute tolerance (dopri5 only).
 */
atol: number, 
/**
 * Fixed step for euler/heun/rk4; initial step for dopri5 (0 = automatic).
 */
h: number, 
/**
 * Largest allowed step for dopri5; absent = no limit.
 */
hMax?: number, 
/**
 * Step limit per trajectory.
 */
maxSteps: number, };

export type TrajectoryRequest = { 
/**
 * Full initial state vector.
 */
x0: Array<number>, 
/**
 * Start time (ODE); ignored for maps, which always start at `n = 0`.
 */
t0: number, 
/**
 * End time (ODE; may be smaller than `t0` to go backwards in time) or
 * number of iterates (map).
 */
t1: number, };

export type PortraitRequest = { 
/**
 * Echoed back so the UI can drop stale responses.
 */
requestId: number, 
/**
 * Handle from `compile_system`.
 */
systemId: number, 
/**
 * Parameter values, in `CompileResponse::params` order.
 */
params: Array<number>, 
/**
 * Horizontal axis.
 */
xAxis: AxisRef, 
/**
 * Vertical axis.
 */
yAxis: AxisRef, 
/**
 * Visible horizontal range `[min, max]`.
 */
xRange: [number, number], 
/**
 * Visible vertical range `[min, max]`.
 */
yRange: [number, number], 
/**
 * Values used for state variables not on an axis (the slice).
 */
baseState: Array<number>, 
/**
 * Time at which a non-autonomous field is drawn.
 */
fieldTime: number, 
/**
 * Arrows per side of the direction field; 0 = no field.
 */
fieldGrid: number, 
/**
 * Compute nullclines.
 */
nullclines: boolean, 
/**
 * Compute fixed points.
 */
fixedPoints: boolean, 
/**
 * Trajectories to integrate or iterate.
 */
trajectories: Array<TrajectoryRequest>, 
/**
 * Solver settings for every trajectory.
 */
solver: SolverOptions, 
/**
 * Evenly spaced samples per ODE trajectory (0 = the raw solver steps).
 */
samples: number, };

export type Arrow = { 
/**
 * Position (plot coordinates).
 */
x: number, 
/**
 * Position (plot coordinates).
 */
y: number, 
/**
 * Raw horizontal field component (1 on a time axis).
 */
u: number, 
/**
 * Raw vertical field component (1 on a time axis).
 */
v: number, };

export type NullclineOut = { 
/**
 * Which equation vanishes along these curves.
 */
state: number, 
/**
 * Polylines of `[x, y]` points in plot coordinates.
 */
polylines: Array<Array<[number, number]>>, };

export type FixedPointOut = { 
/**
 * Full state vector.
 */
x: Array<number>, 
/**
 * Eigenvalues as `[re, im]` pairs.
 */
eigenvalues: Array<[number, number]>, 
/**
 * One entry per eigenvalue (same order): the unit eigenvector of a real
 * eigenvalue, sign-normalised so its largest-magnitude component is
 * positive, or the real part of the eigenvector of a complex one.
 * `null` for the missing direction of a defective eigenvalue (a
 * degenerate node has one vector and one `null`); stars get an
 * orthonormal basis.
 */
eigenvectors: Array<Array<number> | null>, 
/**
 * Imaginary parts of the eigenvectors of complex eigenvalues, `null`
 * for real ones. For a pair `α ± iω` the real and imaginary parts are
 * orthogonal, `|re|² + |im|² = 1`, `|re| ≥ |im|`: the major and minor
 * semi-axes of the linearised orbits' ellipses.
 */
eigenvectorsIm: Array<Array<number> | null>, 
/**
 * Strogatz's name for the point.
 */
classification: Classification, 
/**
 * Asymptotically stable.
 */
stable: boolean, };

export type TrajectoryOut = { 
/**
 * Times (iterate index for maps).
 */
t: Array<number>, 
/**
 * States, row-major, length `t.len() * dim`.
 */
x: Array<number>, 
/**
 * Why the computation stopped.
 */
status: Status, 
/**
 * Work counters.
 */
stats: Stats, };

export type PortraitResponse = { 
/**
 * Copied from the request.
 */
requestId: number, 
/**
 * Direction field (empty for maps or when `fieldGrid == 0`).
 */
arrows: Array<Arrow>, 
/**
 * Nullclines (empty unless requested; never for maps).
 */
nullclines: Array<NullclineOut>, 
/**
 * Fixed points (empty unless requested; never for non-autonomous systems).
 */
fixedPoints: Array<FixedPointOut>, 
/**
 * One entry per requested trajectory, in request order.
 */
trajectories: Array<TrajectoryOut>, 
/**
 * Wall time spent in Rust, in milliseconds.
 */
elapsedMs: number, };
