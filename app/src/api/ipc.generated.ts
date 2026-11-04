// Generated from app/src-tauri/src/ipc.rs by `UPDATE_IPC_BINDINGS=1 cargo test -p separatrix typescript_bindings`.

export type CompileRequest = { 
/**
 * The equations, one per line (the UI joins its non-empty lines with `"\n"`).
 */
source: string, 
/**
 * LaTeX (from MathLive) or plain text.
 */
format: import("./types").InputFormat, };

export type Diagnostic = { 
/**
 * Error, warning or info.
 */
severity: import("./types").Severity, 
/**
 * Plain-language explanation.
 */
message: string, 
/**
 * Byte offsets `[start, end]` into `CompileRequest::source`; absent when
 * the message is about the system as a whole.
 */
span?: [number, number] | null, };

export type StateInfo = { 
/**
 * Identity, e.g. `"theta'"`.
 */
name: string, 
/**
 * Display form, e.g. `"\\dot{\\theta}"`.
 */
latex: string, };

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
systemId?: number | null, 
/**
 * Flow or map; present when `ok`.
 */
kind?: import("./types").SystemKind, 
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

export type AxisRef = { "kind": "state", 
/**
 * Index into the state vector.
 */
index: number, } | { "kind": "time" };

export type SolverOptions = { 
/**
 * Integration method.
 */
method: import("./types").Method, 
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
hMax?: number | null, 
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
classification: import("./types").Classification, 
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
status: import("./types").Status, 
/**
 * Work counters.
 */
stats: import("./types").TrajectoryOut["stats"], };

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
