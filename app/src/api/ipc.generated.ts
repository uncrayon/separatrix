// Generated from app/src-tauri/src/ipc.rs by `UPDATE_IPC_BINDINGS=1 cargo test -p separatrix typescript_bindings`.

export type InputFormat = "latex" | "text";

export type SystemKind = "ode" | "map";

export type Severity = "error" | "warning" | "info";

export type Method = "euler" | "heun" | "rk4" | "dopri5" | "verlet" | "yoshida4" | "rosenbrock23";

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

export type Direction = "rising" | "falling" | "both";

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
 * A Hamiltonian that splits as `T(p) + V(q)` and does not read `t`,
 * so the symplectic methods (`verlet`, `yoshida4`) can integrate it.
 */
separable: boolean, 
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
 * Relative tolerance (dopri5 and rosenbrock23 only).
 */
rtol: number, 
/**
 * Absolute tolerance (dopri5 and rosenbrock23 only).
 */
atol: number, 
/**
 * Fixed step for euler/heun/rk4/verlet/yoshida4; initial step for
 * dopri5 and rosenbrock23 (0 = automatic).
 */
h: number, 
/**
 * Largest allowed step for dopri5 and rosenbrock23; absent = no limit.
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
samples: number, 
/**
 * Sample the graph `y = f(x)` of a one-dimensional autonomous map across
 * `x_range` (the cobweb view; both axes are then state 0). Absent = no
 * graph. Clamped to `2..=MAX_GRAPH_SAMPLES` (4096).
 */
graphSamples?: number, 
/**
 * Iterates computed but not returned at the start of each orbit:
 * map iterates before the orbit's `n` (clamped so that
 * `transient + n <= MAX_ITERATES`), or, for a flow with a `section`,
 * crossings of the section (iterates of the Poincaré map). Ignored for
 * a flow without one. Absent = 0.
 */
transient?: number, 
/**
 * A Poincaré section of a flow: each trajectory in the response is then
 * the orbit's crossings of it (at most `MAX_SECTION_POINTS` after the
 * transient), not its samples. Absent = whole trajectories.
 */
section?: SectionSpec, 
/**
 * The third axis of a 3D view: a state plotted along the box's depth.
 * The fixed-point search treats it as a visible axis over `z_range`, as
 * it does `x_axis` and `y_axis`. Absent = a plane view.
 */
zAxis?: AxisRef, 
/**
 * The range of `z_axis` in view `[min, max]`; required with `z_axis`.
 */
zRange?: [number, number], };

export type SectionSpec = { "kind": "plane", 
/**
 * Index of the state held fixed.
 */
state: number, 
/**
 * Where the plane cuts that state's axis.
 */
value: number, 
/**
 * Which crossings count.
 */
direction: Direction, } | { "kind": "stroboscopic", 
/**
 * Forcing period, `> 0`.
 */
period: number, 
/**
 * The sample times modulo `period`, in absolute time (not from `t0`).
 */
phase: number, };

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
 * eigenvalue, sign-normalized so its largest-magnitude component is
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
stats: Stats, 
/**
 * How well a Hamiltonian flow kept its energy; absent for other systems.
 */
energy?: EnergyOut, 
/**
 * `true` when Dormand–Prince's steps looked stiff (Hairer's test): its
 * step was held back by stability, not accuracy, and `rosenbrock23`
 * would take far fewer. Absent otherwise, and for every other method.
 */
stiff?: boolean, };

export type EnergyOut = { 
/**
 * `H` at the start point.
 */
h0: number, 
/**
 * The largest `|H − h0|` over the returned samples.
 */
maxAbsDrift: number, };

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
 * The graph of `f` (cobweb view): present exactly when `graphSamples`
 * was sent. Polylines of `[x, f(x)]`, `x` strictly increasing inside
 * `xRange`, split at every gap (a non-finite value, a pole or a jump)
 * and clipped to the vertical range grown by its span on each side.
 * Empty when `f` is undefined or outside that band everywhere in view.
 */
graph?: Array<Array<[number, number]>>, 
/**
 * Wall time spent in Rust, in milliseconds.
 */
elapsedMs: number, };

export type LyapunovRequest = { 
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
 * Full initial state vector.
 */
x0: Array<number>, 
/**
 * Start time (flow); ignored for maps, which start at `n = 0`.
 */
t0: number, 
/**
 * Time (flow) or iterates (map) run before measuring, `>= 0`.
 */
transient: number, 
/**
 * Time (flow) or iterates (map) measured after the transient, `> 0`.
 * For a map, `transient + total` is at most `MAX_LYAPUNOV_ITERATES`.
 */
total: number, 
/**
 * Flow: time `τ` between QR steps. Map: iterates per QR step. `> 0`,
 * and `total / interval` at most `MAX_RENORMALIZATIONS`.
 */
interval: number, 
/**
 * Blocks for the uncertainty, `2..=` the number of QR steps.
 */
blocks: number, 
/**
 * Integrator for a flow (ignored for maps). `maxSteps` bounds the
 * accepted steps of the whole run, transient included.
 */
solver: SolverOptions, };

export type LyapunovResponse = { 
/**
 * Copied from the request.
 */
requestId: number, 
/**
 * `λ1 … λn`, largest first (per unit time, or per iterate for a map).
 * Empty when no QR step was completed.
 */
exponents: Array<number>, 
/**
 * Standard error of each exponent over the blocks reached (sample
 * standard deviation of the per-block exponents over `√B`). Empty when
 * fewer than two blocks were reached.
 */
uncertainty: Array<number>, 
/**
 * `Σ λ_i`.
 */
sum: number, 
/**
 * What `sum` must equal: the time average of `tr J` (flow) or the mean
 * of `ln |det J|` (map) along the orbit. The gap measures the
 * integrator's error on the tangent vectors.
 */
traceAverage: number, 
/**
 * Time (flow) or iterate index (map) of each running-estimate sample.
 */
historyT: Array<number>, 
/**
 * Running estimates, row-major: sample `k` is `history[k·n .. (k+1)·n]`.
 */
history: Array<number>, 
/**
 * QR steps completed.
 */
renormalizations: number, 
/**
 * Why the run stopped; the estimates cover what was completed.
 */
status: Status, 
/**
 * Work counters, transient included.
 */
stats: Stats, 
/**
 * Wall time spent in Rust, in milliseconds.
 */
elapsedMs: number, };

export type OrbitDiagramRequest = { 
/**
 * Echoed back so the UI can drop stale responses.
 */
requestId: number, 
/**
 * Handle from `compile_system`.
 */
systemId: number, 
/**
 * Parameter values, in `CompileResponse::params` order; the swept one
 * is replaced column by column.
 */
params: Array<number>, 
/**
 * Index of the swept parameter.
 */
param: number, 
/**
 * First and last parameter value, both included, `lo < hi`.
 */
range: [number, number], 
/**
 * Parameter values across `range`, `2..=MAX_ORBIT_COLUMNS`.
 */
columns: number, 
/**
 * Index of the state recorded.
 */
state: number, 
/**
 * Full initial state vector, the same for every column.
 */
x0: Array<number>, 
/**
 * Iterates discarded in each column.
 */
transient: number, 
/**
 * Iterates kept in each column, `>= 1`, with `columns · kept` at most
 * `MAX_ORBIT_POINTS`.
 */
kept: number, 
/**
 * Also locate the period-doubling points (1-D maps only).
 */
doublings: boolean, };

export type DoublingOut = { 
/**
 * Value of the swept parameter.
 */
r: number, 
/**
 * Period of the cycle that loses stability.
 */
period: number, };

export type OrbitDiagramResponse = { 
/**
 * Copied from the request.
 */
requestId: number, 
/**
 * The parameter values, `r_j = lo + j·(hi − lo)/(columns − 1)`.
 */
r: Array<number>, 
/**
 * `columns · kept` iterates, column by column: column `j` is
 * `x[j·kept .. (j + 1)·kept]`. `null` from the iterate where the orbit
 * left `BLOW_UP_THRESHOLD` (or stopped being finite) on.
 */
x: Array<number | null>, 
/**
 * The period-doubling points in increasing `r`: present exactly when
 * `doublings` was requested.
 */
doublings?: Array<DoublingOut>, 
/**
 * Wall time spent in Rust, in milliseconds.
 */
elapsedMs: number, };

export type ProjectFile = { 
/**
 * Always `"separatrix"`.
 */
format: string, 
/**
 * The schema version the file was written in.
 */
format_version: number, 
/**
 * The oldest schema version that can read the file.
 */
min_reader_version: number, 
/**
 * The Separatrix version that saved the file (informational).
 */
app_version: string, system: ProjectSystem, initial_conditions: Array<ProjectInitialCondition>, views: Array<ProjectView>, 
/**
 * Reserved; v1 keeps the entries verbatim.
 */
analyses: unknown[], solver: ProjectSolver, 
/**
 * Owned by the export dialog; opaque to Rust.
 */
export_settings?: unknown, };

export type ProjectSystem = { 
/**
 * `"ode"`, `"map"`, … Informational: recomputed on open.
 */
kind: string, 
/**
 * `"latex"` or `"text"`.
 */
input_format: string, 
/**
 * The source of truth: every line exactly as typed, blank lines included.
 */
lines: Array<string>, 
/**
 * State names, in the order `x0` arrays are written in.
 */
states: Array<string>, params: Array<ProjectParam>, 
/**
 * The template the system came from; `null` for untitled work.
 */
template: string | null, };

export type ProjectParam = { id: string, value: number, 
/**
 * A missing bound reads as NaN and is repaired by validation.
 */
min: number, max: number, };

export type ProjectInitialCondition = { 
/**
 * A missing or empty id is repaired by validation.
 */
id: string, 
/**
 * One value per entry of `system.states`, in that order.
 */
x0: Array<number>, t0: number, 
/**
 * Index into the trajectory palette (normalized mod 7).
 */
color: number, hidden: boolean, };

export type ProjectView = { 
/**
 * A missing or empty id is repaired by validation.
 */
id: string, 
/**
 * `"cobweb"` for a cobweb plot of a one-dimensional map (`x_axis` is
 * state `s`, `y_axis` is `{ "kind": "next", "name": s }`); `"section"`
 * for a Poincaré section of a flow (see `section`); `"space"` for a 3D
 * view of a flow (three states: `x_axis`, `y_axis`, `z_axis`; see
 * `camera`); `"lyapunov"` for the Lyapunov spectrum of a flow or a map
 * (see `lyapunov`); `"orbit"` for the orbit diagram of a map (`x_axis`
 * is `{ "kind": "param", "name": p }`, `y_axis` a state; see `orbit`);
 * absent for the phase plane and the time series, which the axes tell
 * apart.
 */
kind?: string, 
/**
 * The section of a `"section"` view. Its crossings are plotted in two
 * states (`x_axis`, `y_axis`), or as the return map when `y_axis` is
 * `{ "kind": "next", "name": s }` of the state `s` across.
 */
section?: ProjectSection, x_axis: ProjectAxis, y_axis: ProjectAxis, 
/**
 * The third axis of a `"space"` view (a 3D view of a flow): a state, as
 * `x_axis` and `y_axis` are.
 */
z_axis?: ProjectAxis, 
/**
 * The user's ranges (not the ones equalized for the plot's aspect).
 */
x_range: [number, number], y_range: [number, number], 
/**
 * The range of `z_axis` in a `"space"` view.
 */
z_range?: [number, number], 
/**
 * Where a `"space"` view is seen from.
 */
camera?: ProjectCamera, 
/**
 * The run of a `"lyapunov"` view (the running estimates of the
 * Lyapunov spectrum: `x_axis` is time, `y_axis` is
 * `{ "kind": "exponent" }`).
 */
lyapunov?: ProjectLyapunov, 
/**
 * The columns of an `"orbit"` view (the orbit diagram of a map over
 * the range of the parameter across).
 */
orbit?: ProjectOrbit, slice: ProjectSlice, layers: ProjectLayers, };

export type ProjectSection = { kind: string, 
/**
 * The plane's state, by name.
 */
state?: string, value?: number, 
/**
 * `"rising"`, `"falling"` or `"both"`.
 */
direction?: string, period?: number, phase?: number, };

export type ProjectCamera = { yaw: number, pitch: number, zoom: number, };

export type ProjectLyapunov = { transient?: number, total?: number, interval?: number, blocks?: number, };

export type ProjectOrbit = { transient?: number, kept?: number, };

export type ProjectAxis = { kind: string, name?: string, };

export type ProjectSlice = { states: { [key in string]: number }, t: number, };

export type ProjectLayers = { field: boolean, 
/**
 * Arrows per side, clamped to 2..=200.
 */
field_grid: number, nullclines: boolean, fixed_points: boolean, backward: boolean, };

export type ProjectSolver = { method: string, rtol: number, atol: number, 
/**
 * Fixed step size; 0 means "the method's default".
 */
h: number, max_steps: number, t_end: number, 
/**
 * Map iterates computed but not drawn before each orbit; absent = 0.
 * Written only when above 0. Validation clamps it to
 * `0..=TRANSIENT_MAX`.
 */
transient?: number, 
/**
 * Samples per trajectory; 0 means automatic.
 */
samples: number, };

export type ProjectReadRequest = { 
/**
 * A path from `project_choose_open_path`, `launch_ready` or
 * `project-open-requested`; any other path is refused.
 */
path: string, };

export type OpenedProject = { 
/**
 * The path that was read.
 */
path: string, 
/**
 * Its file name, e.g. `"rabbits-vs-sheep.sepx"`.
 */
name: string, 
/**
 * The project, repaired where it could be (see `warnings`).
 */
project: ProjectFile, 
/**
 * One sentence per repair or notice, for the UI's banner.
 */
warnings: Array<string>, 
/**
 * The format version the file was migrated from.
 */
migratedFrom?: number, 
/**
 * The file's (newer, still readable) format version.
 */
newerFormat?: number, };

export type ProjectChooseSaveRequest = { 
/**
 * File name offered in the dialog (`.sepx` is added when missing).
 */
suggestedName: string, 
/**
 * The document's current path; the dialog opens in its folder.
 */
currentPath?: string, };

export type ChosenSavePath = { 
/**
 * Where to save, always ending in `.sepx`.
 */
path: string, 
/**
 * `.sepx` was added to the chosen name and that file already exists:
 * the native dialog never asked about it, so the UI asks
 * "“x.sepx” already exists. Replace it?".
 */
appendedExists: boolean, };

export type ProjectWriteRequest = { 
/**
 * A granted `.sepx` path.
 */
path: string, 
/**
 * The project as the UI built it (checked with path-precise messages
 * before anything is written).
 */
project: unknown, };

export type SavedProject = { 
/**
 * The path written.
 */
path: string, 
/**
 * Its file name.
 */
name: string, };

export type DocumentStateRequest = { 
/**
 * Window title, e.g. `"● rabbits-vs-sheep — Separatrix"`.
 */
title: string, 
/**
 * Unsaved changes: closing the window asks first.
 */
dirty: boolean, };

export type ProjectOpenRequested = { 
/**
 * A granted path, for `project_read`.
 */
path: string, };

export type CloseRequested = Record<symbol, never>;

export type ExportPortraitRequest = { 
/**
 * What to compute.
 */
portrait: PortraitRequest, 
/**
 * Figure width in CSS px (the simplification tolerance is in these).
 */
widthPx: number, 
/**
 * Figure height in CSS px.
 */
heightPx: number, };

export type TrajectoryTag = { 
/**
 * The initial condition's project id (`"ic-3"`).
 */
icId: string, 
/**
 * Integrated backward in time.
 */
backward: boolean, 
/**
 * Palette index (taken mod 7).
 */
color: number, };

export type ExportTheme = "light" | "dark";

export type Notation = "dot" | "prime" | "leibniz";

export type FigureLatexRequest = { 
/**
 * The portrait at the figure's aspect.
 */
portrait: PortraitRequest, 
/**
 * One tag per requested trajectory.
 */
tags: Array<TrajectoryTag>, 
/**
 * Plot width in cm.
 */
widthCm: number, 
/**
 * Plot height in cm.
 */
heightCm: number, 
/**
 * Palette.
 */
theme: ExportTheme, 
/**
 * The system's title (comments, file names).
 */
title: string, 
/**
 * Where the system comes from, e.g. "Strogatz §6.4".
 */
sourceRef?: string, 
/**
 * The lines as typed (comments).
 */
lines: Array<string>, 
/**
 * Their format.
 */
format: InputFormat, 
/**
 * Wrap in a `standalone` document.
 */
standalone: boolean, };

export type FigureLatexResponse = { 
/**
 * The snippet.
 */
tex: string, 
/**
 * Data rows written.
 */
points: number, 
/**
 * Map iterates were thinned to stay under 5,000 points.
 */
thinned: boolean, 
/**
 * What the user should know, one sentence each.
 */
notes: Array<string>, };

export type SaveFigureRequest = { 
/**
 * The figure.
 */
figure: FigureLatexRequest, 
/**
 * File name offered in the dialog.
 */
suggestedName: string, };

export type SavedFiles = { 
/**
 * The file names written (no folders), the main file first.
 */
names: Array<string>, };

export type SystemLatexRequest = { 
/**
 * Handle from `compile_system`.
 */
systemId: number, 
/**
 * The lines as typed.
 */
lines: Array<string>, 
/**
 * Their format.
 */
format: InputFormat, 
/**
 * Parameter values, in `CompileResponse::params` order.
 */
params: Array<number>, 
/**
 * Left-hand-side notation.
 */
notation: Notation, 
/**
 * `align` with labels, or `align*`.
 */
numbered: boolean, 
/**
 * The system's title (labels, comments).
 */
title: string, 
/**
 * Where the system comes from.
 */
sourceRef?: string, };

export type SystemLatexResponse = { 
/**
 * The snippet.
 */
tex: string, 
/**
 * The normalized equations as one `aligned` block, for the dialog's
 * typeset preview (never the raw file text).
 */
previewLatex: string, };

export type FixedPointsLatexRequest = { 
/**
 * The screen's request.
 */
portrait: PortraitRequest, 
/**
 * The system's title (caption, label).
 */
title: string, 
/**
 * Where the system comes from.
 */
sourceRef?: string, 
/**
 * The lines as typed (comments).
 */
lines: Array<string>, 
/**
 * Their format.
 */
format: InputFormat, 
/**
 * Significant digits, 2 to 8.
 */
digits: number, };

export type CsvRequest = { 
/**
 * The trajectories to write (its `samples` is the CSV's own).
 */
portrait: PortraitRequest, 
/**
 * One tag per requested trajectory.
 */
tags: Array<TrajectoryTag>, 
/**
 * Which initial conditions to write, in order; empty for all.
 */
icIds: Array<string>, 
/**
 * The lines as typed (metadata).
 */
lines: Array<string>, 
/**
 * Their format.
 */
format: InputFormat, 
/**
 * The system's title (file names).
 */
title: string, };

export type CsvFileOut = { 
/**
 * `<stem>.csv` and `<stem>.meta.json`.
 */
stem: string, 
/**
 * The CSV (RFC 4180 with LF line endings).
 */
csv: string, 
/**
 * The `.meta.json` contents.
 */
meta: string, };

export type CsvResponse = { 
/**
 * One entry per initial condition.
 */
files: Array<CsvFileOut>, };

export type SaveCsvRequest = { 
/**
 * What to write.
 */
csv: CsvRequest, 
/**
 * File name offered in the dialog.
 */
suggestedName: string, };

export type TextExtension = "tex" | "svg" | "csv";

export type LyapunovExportRequest = { 
/**
 * The view's own request.
 */
run: LyapunovRequest, 
/**
 * The system's title (caption, label, file names).
 */
title: string, 
/**
 * Where the system comes from.
 */
sourceRef?: string, 
/**
 * The lines as typed (comments, metadata).
 */
lines: Array<string>, 
/**
 * Their format.
 */
format: InputFormat, };

export type OrbitExportRequest = { 
/**
 * The diagram's request: the screen's for the CSV, one column per
 * pixel of the PNG layer for a figure.
 */
diagram: OrbitDiagramRequest, 
/**
 * The system's title (comments, file names).
 */
title: string, 
/**
 * Where the system comes from.
 */
sourceRef?: string, 
/**
 * The lines as typed (comments, metadata).
 */
lines: Array<string>, 
/**
 * Their format.
 */
format: InputFormat, };

export type OrbitFigureRequest = { 
/**
 * The diagram and its names.
 */
orbit: OrbitExportRequest, 
/**
 * The view's parameter range (across).
 */
xRange: [number, number], 
/**
 * The view's state range (up).
 */
yRange: [number, number], 
/**
 * Plot width in cm.
 */
widthCm: number, 
/**
 * Plot height in cm.
 */
heightCm: number, 
/**
 * Palette.
 */
theme: ExportTheme, 
/**
 * Wrap in a `standalone` document.
 */
standalone: boolean, };

export type SaveOrbitFigureRequest = { 
/**
 * The figure.
 */
figure: OrbitFigureRequest, 
/**
 * File name offered in the dialog.
 */
suggestedName: string, };

export type SaveCsvFileRequest = { 
/**
 * The file and its metadata.
 */
file: CsvFileOut, 
/**
 * The system's title (as the export command was given it).
 */
title: string, 
/**
 * File name offered in the dialog.
 */
suggestedName: string, };

export type SaveTextRequest = { 
/**
 * File name offered in the dialog.
 */
suggestedName: string, 
/**
 * The file type (dialog filter, and the extension added when missing).
 */
extension: TextExtension, 
/**
 * The contents (UTF-8).
 */
contents: string, };

export type ClipboardTextRequest = { 
/**
 * The text to copy.
 */
text: string, };

export const MAX_SECTION_POINTS = 100000;

export const MAX_RENORMALIZATIONS = 10000000;

export const MAX_LYAPUNOV_ITERATES = 100000000;

export const MAX_ORBIT_COLUMNS = 4096;

export const BLOW_UP_THRESHOLD = 1e12;

export const LYAPUNOV_HISTORY = 2000;

export const MAX_ORBIT_POINTS = 4000000;

export const SECTION_MISFIT = "The saved Poincaré section doesn't fit this system, so the default view is shown.";

export const SPACE_MISFIT = "The saved 3D view doesn't fit this system, so the default view is shown.";

export const LYAPUNOV_MISFIT = "The saved Lyapunov view can't be run as saved, so the default view is shown.";

export const ORBIT_MISFIT = "The saved orbit diagram doesn't fit this system, so the default view is shown.";

export const ORBIT_MAX_TRANSIENT = 100000;

export const ORBIT_MAX_KEPT = 976;

export const SPACE_LATEX_REFUSAL = "LaTeX figures of the 3D view aren't available yet. Export PNG or SVG, or switch to a phase plane.";

export const DEFAULT_CAMERA = { yaw: -0.7853981633974483, pitch: 0.5235987755982988, zoom: 1.0 };
