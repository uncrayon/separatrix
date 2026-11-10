//! The JSON shapes exchanged with the webview.
//!
//! This file is the source of the UI's IPC types: the `bindings` test
//! renders every type here (with ts-rs) into `app/src/api/ipc.generated.ts`,
//! which `app/src/api/types.ts` re-exports, so the UI compiles against these
//! exact shapes and CI fails when the generated file is stale (see also
//! `docs/contracts.md`). Field names are camelCase on the wire.
//!
//! Small enums whose wire names are already fixed (and tested) in the maths
//! crates are reused directly, and their TypeScript comes from the same
//! derive (the crates' `ts` feature): [`SystemKind`] (`"ode"`/`"map"`),
//! [`InputFormat`] (`"latex"`/`"text"`), [`Severity`], [`Method`]
//! (`"dopri5"`, ...), [`Status`] (`"blewUp"`, ...), [`Stats`],
//! [`Direction`] (`"rising"`, ...) and [`Classification`]
//! (`"stable spiral"`, ...).
//!
//! The project file types ([`ProjectFile`] and its parts) come from
//! `separatrix-project` (its `ts` feature) and keep the file's snake_case
//! keys on the wire: the one documented exception to camelCase, so the UI
//! edits exactly what is saved (docs/project-format.md).

use serde::{Deserialize, Serialize};

pub use dyn_analysis::Classification;
pub use dyn_core::SystemKind;
pub use dyn_expr::{InputFormat, Severity};
pub use dyn_solve::{Direction, Method, Stats, Status};

/// Request of the `compile_system` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CompileRequest {
    /// The equations, one per line (the UI joins its non-empty lines with `"\n"`).
    pub source: String,
    /// LaTeX (from MathLive) or plain text.
    pub format: InputFormat,
}

/// Request of the `convert_source` command: rewrite the equations from one
/// input format to the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    /// The UI's lines, as typed (one entry per line; empty lines allowed).
    pub lines: Vec<String>,
    /// The format the lines are in.
    pub from: InputFormat,
    /// The format to write them in.
    pub to: InputFormat,
}

/// One line of a [`ConvertResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ConvertedLineOut {
    /// The line in the target format, or exactly as typed when `converted`
    /// is false.
    pub source: String,
    /// False when the line was kept as typed (it has an error, or the
    /// target format can't write it exactly).
    pub converted: bool,
    /// True when the line was kept because it has an error in its own
    /// format. A line kept with `error` false is valid but can't be written
    /// exactly in the target format; it would mean something else (or
    /// nothing) there, so the UI doesn't switch.
    pub error: bool,
    /// Why the line was kept as typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub note: Option<String>,
}

/// Response of the `convert_source` command: one entry per requested line,
/// in order. A converted line means exactly the same and keeps every name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ConvertResponse {
    /// The converted lines.
    pub lines: Vec<ConvertedLineOut>,
}

/// A message about the user's input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// Error, warning or info.
    pub severity: Severity,
    /// Plain-language explanation.
    pub message: String,
    /// Byte offsets `[start, end]` into `CompileRequest::source`; absent when
    /// the message is about the system as a whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub span: Option<(usize, usize)>,
}

/// A state variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StateInfo {
    /// Identity, e.g. `"theta'"`.
    pub name: String,
    /// Display form, e.g. `"\\dot{\\theta}"`.
    pub latex: String,
    /// A lower derivative introduced by reducing a higher-order equation to
    /// first order (`theta'` for `theta'' = ...`).
    pub derived: bool,
}

/// A parameter and its default value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamInfo {
    /// Identity, e.g. `"sigma"`.
    pub name: String,
    /// Display form, e.g. `"\\sigma"`.
    pub latex: String,
    /// Value used when the user has not moved the slider.
    pub default: f64,
}

/// Response of the `compile_system` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CompileResponse {
    /// `true` when the system compiled (there may still be warnings).
    pub ok: bool,
    /// Handle for `compute_portrait`; present when `ok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub system_id: Option<u32>,
    /// Flow or map; present when `ok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub kind: Option<SystemKind>,
    /// Entered as `H(q, p) = ...`.
    pub hamiltonian: bool,
    /// A Hamiltonian that splits as `T(p) + V(q)` and does not read `t`,
    /// so the symplectic methods (`verlet`, `yoshida4`) can integrate it.
    pub separable: bool,
    /// `t` (or `n`) does not appear on a right-hand side.
    pub autonomous: bool,
    /// State variables in order (empty when not `ok`).
    pub states: Vec<StateInfo>,
    /// Parameters in order (empty when not `ok`).
    pub params: Vec<ParamInfo>,
    /// The first-order system for display, e.g. `"\\dot{x} = y"`.
    pub equations_latex: Vec<String>,
    /// Errors and warnings, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
}

/// What is shown on one axis of the canvas.
///
/// On the wire: `{ "kind": "state", "index": 1 }` or `{ "kind": "time" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AxisRef {
    /// State variable number `index` (in `CompileResponse::states` order).
    State {
        /// Index into the state vector.
        index: usize,
    },
    /// Time `t` for flows, iterate number `n` for maps.
    Time,
}

/// Solver settings sent by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SolverOptions {
    /// Integration method.
    pub method: Method,
    /// Relative tolerance (dopri5 and rosenbrock23 only).
    pub rtol: f64,
    /// Absolute tolerance (dopri5 and rosenbrock23 only).
    pub atol: f64,
    /// Fixed step for euler/heun/rk4/verlet/yoshida4; initial step for
    /// dopri5 and rosenbrock23 (0 = automatic).
    pub h: f64,
    /// Largest allowed step for dopri5 and rosenbrock23; absent = no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub h_max: Option<f64>,
    /// Step limit per trajectory.
    pub max_steps: usize,
}

/// One trajectory to compute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryRequest {
    /// Full initial state vector.
    pub x0: Vec<f64>,
    /// Start time (ODE); ignored for maps, which always start at `n = 0`.
    pub t0: f64,
    /// End time (ODE; may be smaller than `t0` to go backwards in time) or
    /// number of iterates (map).
    pub t1: f64,
}

/// Request of the `compute_portrait` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PortraitRequest {
    /// Echoed back so the UI can drop stale responses.
    pub request_id: u64,
    /// Handle from `compile_system`.
    pub system_id: u32,
    /// Parameter values, in `CompileResponse::params` order.
    pub params: Vec<f64>,
    /// Horizontal axis.
    pub x_axis: AxisRef,
    /// Vertical axis.
    pub y_axis: AxisRef,
    /// Visible horizontal range `[min, max]`.
    pub x_range: (f64, f64),
    /// Visible vertical range `[min, max]`.
    pub y_range: (f64, f64),
    /// Values used for state variables not on an axis (the slice).
    pub base_state: Vec<f64>,
    /// Time at which a non-autonomous field is drawn.
    pub field_time: f64,
    /// Arrows per side of the direction field; 0 = no field.
    pub field_grid: usize,
    /// Compute nullclines.
    pub nullclines: bool,
    /// Compute fixed points.
    pub fixed_points: bool,
    /// Trajectories to integrate or iterate.
    pub trajectories: Vec<TrajectoryRequest>,
    /// Solver settings for every trajectory.
    pub solver: SolverOptions,
    /// Evenly spaced samples per ODE trajectory (0 = the raw solver steps).
    pub samples: usize,
    /// Sample the graph `y = f(x)` of a one-dimensional autonomous map across
    /// `x_range` (the cobweb view; both axes are then state 0). Absent = no
    /// graph. Clamped to `2..=MAX_GRAPH_SAMPLES` (4096).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub graph_samples: Option<usize>,
    /// Iterates computed but not returned at the start of each orbit:
    /// map iterates before the orbit's `n` (clamped so that
    /// `transient + n <= MAX_ITERATES`), or, for a flow with a `section`,
    /// crossings of the section (iterates of the Poincaré map). Ignored for
    /// a flow without one. Absent = 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub transient: Option<usize>,
    /// A Poincaré section of a flow: each trajectory in the response is then
    /// the orbit's crossings of it (at most `MAX_SECTION_POINTS` after the
    /// transient), not its samples. Absent = whole trajectories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub section: Option<SectionSpec>,
    /// The third axis of a 3D view: a state plotted along the box's depth.
    /// The fixed-point search treats it as a visible axis over `z_range`, as
    /// it does `x_axis` and `y_axis`. Absent = a plane view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub z_axis: Option<AxisRef>,
    /// The range of `z_axis` in view `[min, max]`; required with `z_axis`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub z_range: Option<(f64, f64)>,
}

/// Where a Poincaré section samples a flow (`dyn_solve::Section`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SectionSpec {
    /// The hyperplane `x[state] = value`, crossed in `direction` (judged in
    /// the direction of integration, so backwards in time when `t1 < t0`).
    Plane {
        /// Index of the state held fixed.
        state: usize,
        /// Where the plane cuts that state's axis.
        value: f64,
        /// Which crossings count.
        direction: Direction,
    },
    /// The state at `t_k = phase + k·period` (a periodically forced
    /// flow's stroboscopic map: `t mod period = phase`), for every `t_k`
    /// from each trajectory's `t0` to its `t1`.
    Stroboscopic {
        /// Forcing period, `> 0`.
        period: f64,
        /// The sample times modulo `period`, in absolute time (not from `t0`).
        phase: f64,
    },
}

/// One arrow of the direction field.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Arrow {
    /// Position (plot coordinates).
    pub x: f64,
    /// Position (plot coordinates).
    pub y: f64,
    /// Raw horizontal field component (1 on a time axis).
    pub u: f64,
    /// Raw vertical field component (1 on a time axis).
    pub v: f64,
}

/// The curves where one equation's right-hand side vanishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct NullclineOut {
    /// Which equation vanishes along these curves.
    pub state: usize,
    /// Polylines of `[x, y]` points in plot coordinates.
    pub polylines: Vec<Vec<(f64, f64)>>,
}

/// A fixed point and its linearisation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FixedPointOut {
    /// Full state vector.
    pub x: Vec<f64>,
    /// Eigenvalues as `[re, im]` pairs.
    pub eigenvalues: Vec<(f64, f64)>,
    /// One entry per eigenvalue (same order): the unit eigenvector of a real
    /// eigenvalue, sign-normalized so its largest-magnitude component is
    /// positive, or the real part of the eigenvector of a complex one.
    /// `null` for the missing direction of a defective eigenvalue (a
    /// degenerate node has one vector and one `null`); stars get an
    /// orthonormal basis.
    pub eigenvectors: Vec<Option<Vec<f64>>>,
    /// Imaginary parts of the eigenvectors of complex eigenvalues, `null`
    /// for real ones. For a pair `α ± iω` the real and imaginary parts are
    /// orthogonal, `|re|² + |im|² = 1`, `|re| ≥ |im|`: the major and minor
    /// semi-axes of the linearised orbits' ellipses.
    pub eigenvectors_im: Vec<Option<Vec<f64>>>,
    /// Strogatz's name for the point.
    pub classification: Classification,
    /// Asymptotically stable.
    pub stable: bool,
}

/// One computed trajectory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryOut {
    /// Times (iterate index for maps).
    pub t: Vec<f64>,
    /// States, row-major, length `t.len() * dim`.
    pub x: Vec<f64>,
    /// Why the computation stopped.
    pub status: Status,
    /// Work counters.
    pub stats: Stats,
    /// How well a Hamiltonian flow kept its energy; absent for other systems.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub energy: Option<EnergyOut>,
    /// `true` when Dormand–Prince's steps looked stiff (Hairer's test): its
    /// step was held back by stability, not accuracy, and `rosenbrock23`
    /// would take far fewer. Absent otherwise, and for every other method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub stiff: Option<bool>,
}

/// The energy `H` along one trajectory of a Hamiltonian flow.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EnergyOut {
    /// `H` at the start point.
    pub h0: f64,
    /// The largest `|H − h0|` over the returned samples.
    pub max_abs_drift: f64,
}

/// Response of the `compute_portrait` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PortraitResponse {
    /// Copied from the request.
    pub request_id: u64,
    /// Direction field (empty for maps or when `fieldGrid == 0`).
    pub arrows: Vec<Arrow>,
    /// Nullclines (empty unless requested; never for maps).
    pub nullclines: Vec<NullclineOut>,
    /// Fixed points (empty unless requested; never for non-autonomous systems).
    pub fixed_points: Vec<FixedPointOut>,
    /// One entry per requested trajectory, in request order.
    pub trajectories: Vec<TrajectoryOut>,
    /// The graph of `f` (cobweb view): present exactly when `graphSamples`
    /// was sent. Polylines of `[x, f(x)]`, `x` strictly increasing inside
    /// `xRange`, split at every gap (a non-finite value, a pole or a jump)
    /// and clipped to the vertical range grown by its span on each side.
    /// Empty when `f` is undefined or outside that band everywhere in view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub graph: Option<Vec<Vec<(f64, f64)>>>,
    /// Wall time spent in Rust, in milliseconds.
    pub elapsed_ms: f64,
}

/// Request of the `compute_lyapunov` command: the Lyapunov spectrum of the
/// orbit from `x0` by Benettin's QR method (`dyn_analysis::lyapunov_spectrum`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LyapunovRequest {
    /// Echoed back so the UI can drop stale responses.
    pub request_id: u64,
    /// Handle from `compile_system`.
    pub system_id: u32,
    /// Parameter values, in `CompileResponse::params` order.
    pub params: Vec<f64>,
    /// Full initial state vector.
    pub x0: Vec<f64>,
    /// Start time (flow); ignored for maps, which start at `n = 0`.
    pub t0: f64,
    /// Time (flow) or iterates (map) run before measuring, `>= 0`.
    pub transient: f64,
    /// Time (flow) or iterates (map) measured after the transient, `> 0`.
    /// For a map, `transient + total` is at most `MAX_LYAPUNOV_ITERATES`.
    pub total: f64,
    /// Flow: time `τ` between QR steps. Map: iterates per QR step. `> 0`,
    /// and `total / interval` at most `MAX_RENORMALIZATIONS`.
    pub interval: f64,
    /// Blocks for the uncertainty, `2..=` the number of QR steps.
    pub blocks: usize,
    /// Integrator for a flow (ignored for maps). `maxSteps` bounds the
    /// accepted steps of the whole run, transient included.
    pub solver: SolverOptions,
}

/// Response of the `compute_lyapunov` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LyapunovResponse {
    /// Copied from the request.
    pub request_id: u64,
    /// `λ1 … λn`, largest first (per unit time, or per iterate for a map).
    /// Empty when no QR step was completed.
    pub exponents: Vec<f64>,
    /// Standard error of each exponent over the blocks reached (sample
    /// standard deviation of the per-block exponents over `√B`). Empty when
    /// fewer than two blocks were reached.
    pub uncertainty: Vec<f64>,
    /// `Σ λ_i`.
    pub sum: f64,
    /// What `sum` must equal: the time average of `tr J` (flow) or the mean
    /// of `ln |det J|` (map) along the orbit. The gap measures the
    /// integrator's error on the tangent vectors.
    pub trace_average: f64,
    /// Time (flow) or iterate index (map) of each running-estimate sample.
    pub history_t: Vec<f64>,
    /// Running estimates, row-major: sample `k` is `history[k·n .. (k+1)·n]`.
    pub history: Vec<f64>,
    /// QR steps completed.
    pub renormalizations: usize,
    /// Why the run stopped; the estimates cover what was completed.
    pub status: Status,
    /// Work counters, transient included.
    pub stats: Stats,
    /// Wall time spent in Rust, in milliseconds.
    pub elapsed_ms: f64,
}

/// Most parameter values (columns) one `compute_orbit_diagram` request may ask for.
pub const MAX_ORBIT_COLUMNS: usize = 4096;
/// Most kept iterates (`columns · kept`) one `compute_orbit_diagram` request may ask for.
pub const MAX_ORBIT_POINTS: usize = 4_000_000;

/// Request of the `compute_orbit_diagram` command: the orbit diagram of a
/// map over a range of one parameter (`dyn_analysis::orbit_diagram`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OrbitDiagramRequest {
    /// Echoed back so the UI can drop stale responses.
    pub request_id: u64,
    /// Handle from `compile_system`.
    pub system_id: u32,
    /// Parameter values, in `CompileResponse::params` order; the swept one
    /// is replaced column by column.
    pub params: Vec<f64>,
    /// Index of the swept parameter.
    pub param: usize,
    /// First and last parameter value, both included, `lo < hi`.
    pub range: (f64, f64),
    /// Parameter values across `range`, `2..=MAX_ORBIT_COLUMNS`.
    pub columns: usize,
    /// Index of the state recorded.
    pub state: usize,
    /// Full initial state vector, the same for every column.
    pub x0: Vec<f64>,
    /// Iterates discarded in each column.
    pub transient: usize,
    /// Iterates kept in each column, `>= 1`, with `columns · kept` at most
    /// `MAX_ORBIT_POINTS`.
    pub kept: usize,
    /// Also locate the period-doubling points (1-D maps only).
    pub doublings: bool,
}

/// A period-doubling point: the `period`-cycle loses stability at `r`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DoublingOut {
    /// Value of the swept parameter.
    pub r: f64,
    /// Period of the cycle that loses stability.
    pub period: usize,
}

/// Response of the `compute_orbit_diagram` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OrbitDiagramResponse {
    /// Copied from the request.
    pub request_id: u64,
    /// The parameter values, `r_j = lo + j·(hi − lo)/(columns − 1)`.
    pub r: Vec<f64>,
    /// `columns · kept` iterates, column by column: column `j` is
    /// `x[j·kept .. (j + 1)·kept]`. `null` from the iterate where the orbit
    /// left `BLOW_UP_THRESHOLD` (or stopped being finite) on.
    #[cfg_attr(test, ts(type = "Array<number | null>"))]
    pub x: Vec<f64>,
    /// The period-doubling points in increasing `r`: present exactly when
    /// `doublings` was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub doublings: Option<Vec<DoublingOut>>,
    /// Wall time spent in Rust, in milliseconds.
    pub elapsed_ms: f64,
}

/// Most points one `compute_continuation` request may ask for in each
/// direction.
pub const MAX_CONTINUATION_POINTS: usize = 20_000;

/// Request of the `compute_continuation` command: follow a branch of
/// equilibria of a flow, or of fixed points of `f^period` of a map, as one
/// parameter varies (`dyn_analysis::continue_branch`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ContinuationRequest {
    /// Echoed back so the UI can drop stale responses.
    pub request_id: u64,
    /// Handle from `compile_system`.
    pub system_id: u32,
    /// Parameter values, in `CompileResponse::params` order; the branch
    /// starts at `params[param]`.
    pub params: Vec<f64>,
    /// Index of the continuation parameter.
    pub param: usize,
    /// The parameter interval the branch stays in, `lo < hi`.
    pub range: (f64, f64),
    /// Where the branch starts.
    pub start: ContinuationStart,
    /// Which way to follow it from the start.
    pub direction: ContinuationDirection,
    /// First step in arclength, `> 0`.
    pub ds: f64,
    /// Largest step, `>= ds`.
    pub ds_max: f64,
    /// Most points in each direction, the start included,
    /// `2..=MAX_CONTINUATION_POINTS`.
    pub max_points: usize,
}

/// Where a branch starts.
///
/// On the wire: `{ "kind": "point", "x0": [0.5], "period": 1 }` or
/// `{ "kind": "switch", "at": { "kind": "branchPoint", … } }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContinuationStart {
    /// Near a fixed point: `x0` is corrected by Newton's method first.
    Point {
        /// Full state vector.
        x0: Vec<f64>,
        /// `k` of `f^k` for a map, 1 for a flow.
        period: usize,
    },
    /// Onto the other branch through a branch point, or onto the
    /// `2·period`-cycle born at a flip (`dyn_analysis::switch_branch`).
    /// The point is self-contained, so the branch it came from is not
    /// needed.
    Switch {
        /// A special point of kind `branchPoint` or `flip`.
        at: SwitchPoint,
    },
}

/// What [`ContinuationStart::Switch`] needs of a special point: the fields
/// of a [`SpecialPointOut`] that `dyn_analysis::switch_branch` reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SwitchPoint {
    /// `branchPoint` or `flip`.
    pub kind: SpecialKindOut,
    /// The parameter.
    pub p: f64,
    /// The state (`dim` values).
    pub x: Vec<f64>,
    /// `k` of `f^k`, as `BranchOut::period`.
    pub period: usize,
    /// Unit tangent `(ẋ, ṗ)` of the branch there, `dim + 1` values.
    pub tangent: Vec<f64>,
}

impl From<&SpecialPointOut> for SwitchPoint {
    fn from(sp: &SpecialPointOut) -> Self {
        SwitchPoint {
            kind: sp.kind,
            p: sp.p,
            x: sp.x.clone(),
            period: sp.period,
            tangent: sp.tangent.clone(),
        }
    }
}

/// Which way a branch is followed from its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ContinuationDirection {
    /// The parameter increases from the start.
    Forward,
    /// The parameter decreases from the start.
    Backward,
    /// Both ways, joined into one branch through the start.
    Both,
}

/// Response of the `compute_continuation` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ContinuationResponse {
    /// Copied from the request.
    pub request_id: u64,
    /// The branch.
    pub branch: BranchOut,
    /// Wall time spent in Rust, in milliseconds.
    pub elapsed_ms: f64,
}

/// A branch as one polyline in arclength order. With direction `both` the
/// backward half comes first, reversed, so the start is in the middle.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BranchOut {
    /// `k` of `f^k` for a map, 1 for a flow.
    pub period: usize,
    /// The parameter at each point.
    pub p: Vec<f64>,
    /// `points · period · dim` values: point `i` is the cycle
    /// `x, f(x), …, f^{k−1}(x)` at `x[i·period·dim .. (i + 1)·period·dim]`.
    pub x: Vec<f64>,
    /// Every eigenvalue has `Re λ < 0` (flow) or `|μ| < 1` (map); `false`
    /// at a special point.
    pub stable: Vec<bool>,
    /// Eigenvalues with `Re λ > 0` (flow) or `|μ| > 1` (map) at each point.
    pub unstable: Vec<u32>,
    /// The located bifurcations in arclength order.
    pub special: Vec<SpecialPointOut>,
    /// Why each end of the branch stopped.
    pub end: BranchEndOut,
}

/// Why each end of a branch stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BranchEndOut {
    /// The first point's, with direction `both`; `null` when the first
    /// point is the start.
    pub first: Option<BranchStop>,
    /// The last point's.
    pub last: BranchStop,
}

/// Why a branch stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum BranchStop {
    /// It reached an edge of the parameter range; the point lies on it.
    Range,
    /// It has `max_points` points in that direction.
    MaxPoints,
    /// A step failed at the smallest step size.
    StepTooSmall,
    /// A curve came back to its start: the last point repeats the first.
    /// Never on a branch, and never the first end of a curve.
    Closed,
    /// A branch-point curve reached parameters that no longer keep the
    /// branch point (a second parameter breaks the invariance that makes
    /// it). Never on a branch.
    Unfolded,
}

/// A located bifurcation on a branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SpecialPointOut {
    /// What it is.
    pub kind: SpecialKindOut,
    /// Its own point in `BranchOut::p`.
    pub index: usize,
    /// The parameter.
    pub p: f64,
    /// The state (`dim` values; the cycle's first point for a map).
    pub x: Vec<f64>,
    /// `k` of `f^k`, as `BranchOut::period`.
    pub period: usize,
    /// Unit tangent `(ẋ, ṗ)` of the branch there, `dim + 1` values,
    /// oriented as the polyline.
    pub tangent: Vec<f64>,
    /// Eigenvalues of `J` (flow) or `D f^k` (map) as `[re, im]` pairs, the
    /// critical ones on the stability boundary.
    pub eigenvalues: Vec<(f64, f64)>,
    /// Hopf: the frequency `ω`. Neimark–Sacker: the angle `θ = arg μ` of
    /// the critical multiplier, in `(0, π)`. `null` otherwise.
    pub omega: Option<f64>,
    /// Hopf: the first Lyapunov coefficient `ℓ₁` (negative when
    /// supercritical), scaled so that `ṙ = μr + a r³, θ̇ = ω` has
    /// `ℓ₁ = a/ω`. `null` otherwise, or where it could not be computed.
    pub lyapunov: Option<f64>,
}

/// The kinds of special point; their labels are `LP`, `H`, `BP`, `PD`, `NS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SpecialKindOut {
    /// A limit point: the branch turns back in the parameter.
    Fold,
    /// A complex pair of eigenvalues of a flow crosses the imaginary axis.
    Hopf,
    /// Another branch crosses this one (pitchfork, transcritical).
    BranchPoint,
    /// A multiplier of a map passes through `−1` (period doubling).
    Flip,
    /// A complex pair of multipliers of a map crosses the unit circle.
    NeimarkSacker,
}

/// Request of the `compute_curve` command: follow a fold, Hopf, branch
/// point, flip or Neimark–Sacker bifurcation as two parameters vary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CurveRequest {
    /// Echoed back so the UI can drop stale responses.
    pub request_id: u64,
    /// Handle from `compile_system`.
    pub system_id: u32,
    /// Parameter values, in `CompileResponse::params` order; the curve
    /// starts at `(params[free.0], params[free.1])`.
    pub params: Vec<f64>,
    /// The two free parameters: across (the Branch view's), then up.
    pub free: (usize, usize),
    /// The box the curve stays in: the across range, then the up range,
    /// each `lo < hi`.
    pub ranges: ((f64, f64), (f64, f64)),
    /// The bifurcation the curve starts from.
    pub seed: CurveSeed,
    /// Which way to follow it from the seed.
    pub direction: ContinuationDirection,
    /// First step in arclength, `> 0`.
    pub ds: f64,
    /// Largest step, `>= ds`.
    pub ds_max: f64,
    /// Most points in each direction, the seed included,
    /// `2..=MAX_CONTINUATION_POINTS`.
    pub max_points: usize,
}

/// The one-parameter bifurcation a curve starts from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CurveSeed {
    /// Which bifurcation is followed.
    pub kind: CurveKindOut,
    /// The state (`dim` values; the cycle's first point for a map).
    pub x: Vec<f64>,
    /// `k` of `f^k` for a map, 1 for a flow.
    pub period: usize,
}

/// The kinds of bifurcation curve; their labels are `LP`, `H`, `BP`, `PD`,
/// `NS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum CurveKindOut {
    /// Folds (limit points): an eigenvalue `0` (flow) or a multiplier `+1`
    /// (map).
    Fold,
    /// Hopf points of a flow.
    Hopf,
    /// Branch points: another branch of equilibria or fixed points crosses
    /// (transcritical, pitchfork), as on an invariant line.
    BranchPoint,
    /// Flips (period doublings) of a map.
    Flip,
    /// Neimark–Sacker points of a map.
    NeimarkSacker,
}

/// Response of the `compute_curve` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CurveResponse {
    /// Copied from the request.
    pub request_id: u64,
    /// The curve.
    pub curve: CurveOut,
    /// Wall time spent in Rust, in milliseconds.
    pub elapsed_ms: f64,
}

/// A bifurcation curve as one polyline in arclength order. With direction
/// `both` the backward half comes first, reversed, so the seed is in the
/// middle.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CurveOut {
    /// Which bifurcation it is.
    pub kind: CurveKindOut,
    /// `k` of `f^k` for a map, 1 for a flow.
    pub period: usize,
    /// The across parameter at each point.
    pub p1: Vec<f64>,
    /// The up parameter at each point.
    pub p2: Vec<f64>,
    /// `points · dim` values: the state at each point (the cycle's first
    /// point for a map).
    pub x: Vec<f64>,
    /// `true` where the curve is no bifurcation: a neutral saddle, whose
    /// eigenvalues `±κ` (Hopf) or multipliers `μ, 1/μ` (Neimark–Sacker)
    /// are real.
    pub neutral: Vec<bool>,
    /// The located codimension-2 points in arclength order.
    pub special: Vec<Codim2PointOut>,
    /// Why each end of the curve stopped.
    pub end: BranchEndOut,
}

/// A located codimension-2 point on a curve.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Codim2PointOut {
    /// What it is.
    pub kind: Codim2KindOut,
    /// Its nearest point in `CurveOut::p1`.
    pub index: usize,
    /// The across and up parameters.
    pub p: (f64, f64),
    /// The state (`dim` values; the cycle's first point for a map).
    pub x: Vec<f64>,
    /// `k` of `f^k`, as `CurveOut::period`.
    pub period: usize,
    /// The width of the last bracket of the bisection that located it, in
    /// the curve's scaled arclength (each parameter as a fraction of its
    /// range, the state as it is); `null` where no bisection ran, as in the
    /// browser preview.
    pub bracket: Option<f64>,
}

/// The kinds of codimension-2 point; their labels are `BT`, `CP`, `GH`,
/// `ZH`, `HH`, `R1` to `R4`, `CH`, `LPPD`, `GPD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Codim2KindOut {
    /// Bogdanov–Takens: a double zero eigenvalue, where a fold and a Hopf
    /// curve meet.
    BogdanovTakens,
    /// Cusp: two fold curves meet, the fold's quadratic coefficient is 0.
    Cusp,
    /// Bautin (generalized Hopf): the first Lyapunov coefficient is 0.
    Bautin,
    /// Zero-Hopf: an eigenvalue 0 and a pair `±iω`.
    ZeroHopf,
    /// Hopf–Hopf: two pairs `±iω₁`, `±iω₂`.
    HopfHopf,
    /// 1:1 resonance: a double multiplier `+1`.
    Resonance1,
    /// 1:2 resonance: a double multiplier `−1`.
    Resonance2,
    /// 1:3 resonance: multipliers `e^{±2πi/3}`.
    Resonance3,
    /// 1:4 resonance: multipliers `±i`.
    Resonance4,
    /// Chenciner: the Neimark–Sacker coefficient is 0.
    Chenciner,
    /// Fold-flip: multipliers `+1` and `−1`.
    FoldFlip,
    /// Generalized flip: the flip's normal-form coefficient is 0.
    GeneralizedFlip,
}

// ---------------------------------------------------------------------
// Project files (`.sepx`). The project itself is `ProjectFile` from
// `separatrix-project`, whose keys are snake_case on the wire (the one
// documented exception to camelCase; see docs/project-format.md).
// ---------------------------------------------------------------------

pub use separatrix_project::{
    ProjectAxis, ProjectCamera, ProjectContinuation, ProjectFile, ProjectInitialCondition,
    ProjectLayers, ProjectLyapunov, ProjectOrbit, ProjectParam, ProjectSection, ProjectSeed,
    ProjectSlice, ProjectSolver, ProjectSystem, ProjectView,
};

/// Request of `project_read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectReadRequest {
    /// A path from `project_choose_open_path`, `launch_ready` or
    /// `project-open-requested`; any other path is refused.
    pub path: String,
}

/// Response of `project_read`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OpenedProject {
    /// The path that was read.
    pub path: String,
    /// Its file name, e.g. `"rabbits-vs-sheep.sepx"`.
    pub name: String,
    /// The project, repaired where it could be (see `warnings`).
    pub project: ProjectFile,
    /// One sentence per repair or notice, for the UI's banner.
    pub warnings: Vec<String>,
    /// The format version the file was migrated from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub migrated_from: Option<u32>,
    /// The file's (newer, still readable) format version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub newer_format: Option<u32>,
}

/// Request of `project_choose_save_path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectChooseSaveRequest {
    /// File name offered in the dialog (`.sepx` is added when missing).
    pub suggested_name: String,
    /// The document's current path; the dialog opens in its folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub current_path: Option<String>,
}

/// Response of `project_choose_save_path` (`null` when cancelled).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ChosenSavePath {
    /// Where to save, always ending in `.sepx`.
    pub path: String,
    /// `.sepx` was added to the chosen name and that file already exists:
    /// the native dialog never asked about it, so the UI asks
    /// "“x.sepx” already exists. Replace it?".
    pub appended_exists: bool,
}

/// Request of `project_write`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteRequest {
    /// A granted `.sepx` path.
    pub path: String,
    /// The project as the UI built it (checked with path-precise messages
    /// before anything is written).
    #[cfg_attr(test, ts(type = "unknown"))]
    pub project: serde_json::Value,
}

/// Response of `project_write`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SavedProject {
    /// The path written.
    pub path: String,
    /// Its file name.
    pub name: String,
}

/// One entry of `recent_projects`: a project opened or saved lately that
/// still exists, granted for `project_read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    /// The path, for `project_read`.
    pub path: String,
    /// Its file name, e.g. `"rabbits-vs-sheep.sepx"`.
    pub name: String,
    /// The folder it is in, as the system writes it.
    pub folder: String,
}

/// Request of `set_document_state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DocumentStateRequest {
    /// Window title, e.g. `"● rabbits-vs-sheep — Separatrix"`.
    pub title: String,
    /// Unsaved changes: closing the window asks first.
    pub dirty: bool,
}

/// Payload of the `project-open-requested` event (a file from the OS: a
/// double-click, a second instance or a drop on the window).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenRequested {
    /// A granted path, for `project_read`.
    pub path: String,
}

/// Payload of the `close-requested` event: the window's close was stopped
/// because the document is dirty; the UI confirms, then calls
/// `close_window`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CloseRequested {}

/// Response of `check_for_update`, and payload of the `update-available`
/// event (always `available` there).
///
/// On the wire: `{ "kind": "off" }`, `{ "kind": "upToDate", "version":
/// "1.0.0" }`, `{ "kind": "available", "version": "1.0.1", "notes": …,
/// "date": … }` or `{ "kind": "failed", "message": … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateCheck {
    /// This build has no updater key (docs/updater.md).
    Off,
    /// The running version is the newest.
    UpToDate {
        /// The running version, e.g. `"1.0.0"`.
        version: String,
    },
    /// A newer release is on GitHub Releases.
    Available {
        /// Its version, e.g. `"1.0.1"`.
        version: String,
        /// Its release notes, from `latest.json`.
        notes: Option<String>,
        /// Its publication date (RFC 3339), from `latest.json`.
        date: Option<String>,
    },
    /// The check failed (offline, or the server answered badly).
    Failed {
        /// The reason, for the log.
        message: String,
    },
}

// ---------------------------------------------------------------------
// Exports (docs/exports.md)
// ---------------------------------------------------------------------

/// Request of `compute_export_portrait`: the screen's request at the
/// figure's size (ranges already equalized for its aspect).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ExportPortraitRequest {
    /// What to compute.
    pub portrait: PortraitRequest,
    /// Figure width in CSS px (the simplification tolerance is in these).
    pub width_px: f64,
    /// Figure height in CSS px.
    pub height_px: f64,
}

/// Which initial condition a requested trajectory belongs to; one per entry
/// of `PortraitRequest::trajectories`, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryTag {
    /// The initial condition's project id (`"ic-3"`).
    pub ic_id: String,
    /// Integrated backward in time.
    pub backward: bool,
    /// Palette index (taken mod 7).
    pub color: u8,
}

/// Color theme of an exported figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ExportTheme {
    /// Projector (whiteboard): the default for exports.
    Light,
    /// Chalkboard.
    Dark,
}

/// How derivatives are written in the LaTeX system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Notation {
    /// `\dot{x}` (Strogatz).
    Dot,
    /// `x'`.
    Prime,
    /// `\frac{dx}{dt}`.
    Leibniz,
}

/// Request of `export_figure_latex` (and the `figure` of
/// `save_figure_latex`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FigureLatexRequest {
    /// The portrait at the figure's aspect.
    pub portrait: PortraitRequest,
    /// One tag per requested trajectory.
    pub tags: Vec<TrajectoryTag>,
    /// Plot width in cm.
    pub width_cm: f64,
    /// Plot height in cm.
    pub height_cm: f64,
    /// Palette.
    pub theme: ExportTheme,
    /// The system's title (comments, file names).
    pub title: String,
    /// Where the system comes from, e.g. "Strogatz §6.4".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub source_ref: Option<String>,
    /// The lines as typed (comments).
    pub lines: Vec<String>,
    /// Their format.
    pub format: InputFormat,
    /// Wrap in a `standalone` document.
    pub standalone: bool,
}

/// Response of `export_figure_latex` (inline data, for Copy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FigureLatexResponse {
    /// The snippet.
    pub tex: String,
    /// Data rows written.
    pub points: usize,
    /// Map iterates were thinned to stay under 5,000 points.
    pub thinned: bool,
    /// What the user should know, one sentence each.
    pub notes: Vec<String>,
}

/// Request of `save_figure_latex`: the `.tex` and its `.dat` (and `.png`)
/// files, written next to each other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SaveFigureRequest {
    /// The figure.
    pub figure: FigureLatexRequest,
    /// File name offered in the dialog.
    pub suggested_name: String,
}

/// Response of the `save_*` export commands (`null` when cancelled).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SavedFiles {
    /// The file names written (no folders), the main file first.
    pub names: Vec<String>,
}

/// Request of `export_system_latex`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SystemLatexRequest {
    /// Handle from `compile_system`.
    pub system_id: u32,
    /// The lines as typed.
    pub lines: Vec<String>,
    /// Their format.
    pub format: InputFormat,
    /// Parameter values, in `CompileResponse::params` order.
    pub params: Vec<f64>,
    /// Left-hand-side notation.
    pub notation: Notation,
    /// `align` with labels, or `align*`.
    pub numbered: bool,
    /// The system's title (labels, comments).
    pub title: String,
    /// Where the system comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub source_ref: Option<String>,
}

/// Response of `export_system_latex`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SystemLatexResponse {
    /// The snippet.
    pub tex: String,
    /// The normalized equations as one `aligned` block, for the dialog's
    /// typeset preview (never the raw file text).
    pub preview_latex: String,
}

/// Request of `export_fixed_points_latex`: the screen's portrait request
/// (its fixed points are recomputed with the same search box).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FixedPointsLatexRequest {
    /// The screen's request.
    pub portrait: PortraitRequest,
    /// The system's title (caption, label).
    pub title: String,
    /// Where the system comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub source_ref: Option<String>,
    /// The lines as typed (comments).
    pub lines: Vec<String>,
    /// Their format.
    pub format: InputFormat,
    /// Significant digits, 2 to 8.
    pub digits: usize,
}

/// Request of `export_csv` (and the `csv` of `save_csv`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CsvRequest {
    /// The trajectories to write (its `samples` is the CSV's own).
    pub portrait: PortraitRequest,
    /// One tag per requested trajectory.
    pub tags: Vec<TrajectoryTag>,
    /// Which initial conditions to write, in order; empty for all.
    pub ic_ids: Vec<String>,
    /// The lines as typed (metadata).
    pub lines: Vec<String>,
    /// Their format.
    pub format: InputFormat,
    /// The system's title (file names).
    pub title: String,
}

/// One initial condition's CSV and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CsvFileOut {
    /// `<stem>.csv` and `<stem>.meta.json`.
    pub stem: String,
    /// The CSV (RFC 4180 with LF line endings).
    pub csv: String,
    /// The `.meta.json` contents.
    pub meta: String,
    /// A branch plot's special points, written beside it as
    /// `<stem>-special.csv`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub special: Option<String>,
}

/// Response of `export_csv`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CsvResponse {
    /// One entry per initial condition.
    pub files: Vec<CsvFileOut>,
}

/// Request of `save_csv`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SaveCsvRequest {
    /// What to write.
    pub csv: CsvRequest,
    /// File name offered in the dialog.
    pub suggested_name: String,
}

/// Request of `export_lyapunov_latex` and `export_lyapunov_csv`: the
/// Lyapunov view's run (the core measures it again, or reuses the last
/// export's run of the same request) and the names to label it with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LyapunovExportRequest {
    /// The view's own request.
    pub run: LyapunovRequest,
    /// The system's title (caption, label, file names).
    pub title: String,
    /// Where the system comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub source_ref: Option<String>,
    /// The lines as typed (comments, metadata).
    pub lines: Vec<String>,
    /// Their format.
    pub format: InputFormat,
}

/// Request of `export_orbit_csv`: an orbit diagram (the core computes it
/// again) and the names to label it with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OrbitExportRequest {
    /// The diagram's request: the screen's for the CSV, one column per
    /// pixel of the PNG layer for a figure.
    pub diagram: OrbitDiagramRequest,
    /// The system's title (comments, file names).
    pub title: String,
    /// Where the system comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub source_ref: Option<String>,
    /// The lines as typed (comments, metadata).
    pub lines: Vec<String>,
    /// Their format.
    pub format: InputFormat,
}

/// Request of `export_orbit_latex` (and the `figure` of
/// `save_orbit_latex`): the diagram drawn in the view's ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OrbitFigureRequest {
    /// The diagram and its names.
    pub orbit: OrbitExportRequest,
    /// The view's parameter range (across).
    pub x_range: (f64, f64),
    /// The view's state range (up).
    pub y_range: (f64, f64),
    /// Plot width in cm.
    pub width_cm: f64,
    /// Plot height in cm.
    pub height_cm: f64,
    /// Palette.
    pub theme: ExportTheme,
    /// Wrap in a `standalone` document.
    pub standalone: bool,
}

/// Request of `save_orbit_latex`: the `.tex` and its `.dat` or `.png` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SaveOrbitFigureRequest {
    /// The figure.
    pub figure: OrbitFigureRequest,
    /// File name offered in the dialog.
    pub suggested_name: String,
}

/// What the vertical axis of a branch plot shows.
///
/// On the wire: `{ "kind": "state", "index": 1 }` or `{ "kind": "norm" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BranchAxis {
    /// State variable number `index`.
    State {
        /// Index into the state vector.
        index: usize,
    },
    /// The norm ‖x‖₂ of the state.
    Norm,
}

/// One branch of a branch plot: its request and its color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BranchExport {
    /// The view's request for the branch.
    pub request: ContinuationRequest,
    /// Palette index (taken mod 7), as the plot colors the branch.
    pub color: u8,
}

/// Request of `export_continuation_csv` (and the `continuation` of a
/// figure): the branch plot's requests, which the core follows again (or
/// the last export's run of the same requests is reused), and the names to
/// label them with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ContinuationExportRequest {
    /// Handle from `compile_system`.
    pub system_id: u32,
    /// The sliders' parameter values (the swept one's is each branch's own).
    pub params: Vec<f64>,
    /// Index of the swept parameter.
    pub param: usize,
    /// The parameter interval the branches are followed in.
    pub range: (f64, f64),
    /// The vertical axis.
    pub y_axis: BranchAxis,
    /// The branches, in the view's order.
    pub branches: Vec<BranchExport>,
    /// The system's title (caption, label, file names).
    pub title: String,
    /// Where the system comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub source_ref: Option<String>,
    /// The lines as typed (comments, metadata).
    pub lines: Vec<String>,
    /// Their format.
    pub format: InputFormat,
}

/// Which LaTeX a branch plot exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ContinuationLatex {
    /// The pgfplots figure.
    Figure,
    /// The booktabs table of the special points.
    SpecialPoints,
}

/// Request of `export_continuation_latex` (and the `figure` of
/// `save_continuation_latex`): the branch plot drawn in the view's ranges,
/// or its special points as a table (which ignores the size and colors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ContinuationFigureRequest {
    /// The branches and their names.
    pub continuation: ContinuationExportRequest,
    /// The figure or the table.
    pub content: ContinuationLatex,
    /// The view's parameter range (across).
    pub x_range: (f64, f64),
    /// The view's range up.
    pub y_range: (f64, f64),
    /// Plot width in cm.
    pub width_cm: f64,
    /// Plot height in cm.
    pub height_cm: f64,
    /// Palette.
    pub theme: ExportTheme,
    /// Wrap in a `standalone` document.
    pub standalone: bool,
}

/// Request of `save_continuation_latex`: the `.tex` and, for the figure,
/// its `.dat` files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SaveContinuationFigureRequest {
    /// The figure or table.
    pub figure: ContinuationFigureRequest,
    /// File name offered in the dialog.
    pub suggested_name: String,
}

/// Request of `save_csv_file`: one CSV with its `.meta.json`, as an export
/// command returned it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SaveCsvFileRequest {
    /// The file and its metadata.
    pub file: CsvFileOut,
    /// The system's title (as the export command was given it).
    pub title: String,
    /// File name offered in the dialog.
    pub suggested_name: String,
}

/// The kinds of text file `save_text_file` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum TextExtension {
    /// LaTeX.
    Tex,
    /// SVG.
    Svg,
    /// CSV.
    Csv,
}

/// Request of `save_text_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SaveTextRequest {
    /// File name offered in the dialog.
    pub suggested_name: String,
    /// The file type (dialog filter, and the extension added when missing).
    pub extension: TextExtension,
    /// The contents (UTF-8).
    pub contents: String,
}

/// Request of `clipboard_write_text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipboardTextRequest {
    /// The text to copy.
    pub text: String,
}

#[cfg(test)]
mod bindings {
    use std::path::Path;

    use ts_rs::{Config, TS};

    use super::*;

    const GENERATED: &str = "../src/api/ipc.generated.ts";
    const UI_TYPES: &str = "../src/api/types.ts";
    const UPDATE: &str = "UPDATE_IPC_BINDINGS";

    fn render() -> String {
        let cfg = Config::new().with_large_int("number");
        let decls = [
            InputFormat::decl(&cfg),
            SystemKind::decl(&cfg),
            Severity::decl(&cfg),
            Method::decl(&cfg),
            Status::decl(&cfg),
            Stats::decl(&cfg),
            Direction::decl(&cfg),
            Classification::decl(&cfg),
            CompileRequest::decl(&cfg),
            Diagnostic::decl(&cfg),
            StateInfo::decl(&cfg),
            ParamInfo::decl(&cfg),
            CompileResponse::decl(&cfg),
            ConvertRequest::decl(&cfg),
            ConvertedLineOut::decl(&cfg),
            ConvertResponse::decl(&cfg),
            AxisRef::decl(&cfg),
            SolverOptions::decl(&cfg),
            TrajectoryRequest::decl(&cfg),
            PortraitRequest::decl(&cfg),
            SectionSpec::decl(&cfg),
            Arrow::decl(&cfg),
            NullclineOut::decl(&cfg),
            FixedPointOut::decl(&cfg),
            TrajectoryOut::decl(&cfg),
            EnergyOut::decl(&cfg),
            PortraitResponse::decl(&cfg),
            LyapunovRequest::decl(&cfg),
            LyapunovResponse::decl(&cfg),
            OrbitDiagramRequest::decl(&cfg),
            DoublingOut::decl(&cfg),
            OrbitDiagramResponse::decl(&cfg),
            ContinuationRequest::decl(&cfg),
            ContinuationStart::decl(&cfg),
            ContinuationDirection::decl(&cfg),
            ContinuationResponse::decl(&cfg),
            BranchOut::decl(&cfg),
            BranchEndOut::decl(&cfg),
            BranchStop::decl(&cfg),
            SpecialPointOut::decl(&cfg),
            SwitchPoint::decl(&cfg),
            SpecialKindOut::decl(&cfg),
            CurveRequest::decl(&cfg),
            CurveSeed::decl(&cfg),
            CurveKindOut::decl(&cfg),
            CurveResponse::decl(&cfg),
            CurveOut::decl(&cfg),
            Codim2PointOut::decl(&cfg),
            Codim2KindOut::decl(&cfg),
            ProjectFile::decl(&cfg),
            ProjectSystem::decl(&cfg),
            ProjectParam::decl(&cfg),
            ProjectInitialCondition::decl(&cfg),
            ProjectView::decl(&cfg),
            ProjectSection::decl(&cfg),
            ProjectCamera::decl(&cfg),
            ProjectLyapunov::decl(&cfg),
            ProjectOrbit::decl(&cfg),
            ProjectContinuation::decl(&cfg),
            ProjectSeed::decl(&cfg),
            ProjectAxis::decl(&cfg),
            ProjectSlice::decl(&cfg),
            ProjectLayers::decl(&cfg),
            ProjectSolver::decl(&cfg),
            ProjectReadRequest::decl(&cfg),
            OpenedProject::decl(&cfg),
            ProjectChooseSaveRequest::decl(&cfg),
            ChosenSavePath::decl(&cfg),
            ProjectWriteRequest::decl(&cfg),
            SavedProject::decl(&cfg),
            RecentProject::decl(&cfg),
            DocumentStateRequest::decl(&cfg),
            ProjectOpenRequested::decl(&cfg),
            CloseRequested::decl(&cfg),
            UpdateCheck::decl(&cfg),
            ExportPortraitRequest::decl(&cfg),
            TrajectoryTag::decl(&cfg),
            ExportTheme::decl(&cfg),
            Notation::decl(&cfg),
            FigureLatexRequest::decl(&cfg),
            FigureLatexResponse::decl(&cfg),
            SaveFigureRequest::decl(&cfg),
            SavedFiles::decl(&cfg),
            SystemLatexRequest::decl(&cfg),
            SystemLatexResponse::decl(&cfg),
            FixedPointsLatexRequest::decl(&cfg),
            CsvRequest::decl(&cfg),
            CsvFileOut::decl(&cfg),
            CsvResponse::decl(&cfg),
            SaveCsvRequest::decl(&cfg),
            TextExtension::decl(&cfg),
            LyapunovExportRequest::decl(&cfg),
            OrbitExportRequest::decl(&cfg),
            OrbitFigureRequest::decl(&cfg),
            SaveOrbitFigureRequest::decl(&cfg),
            BranchAxis::decl(&cfg),
            BranchExport::decl(&cfg),
            ContinuationExportRequest::decl(&cfg),
            ContinuationLatex::decl(&cfg),
            ContinuationFigureRequest::decl(&cfg),
            SaveContinuationFigureRequest::decl(&cfg),
            SaveCsvFileRequest::decl(&cfg),
            SaveTextRequest::decl(&cfg),
            ClipboardTextRequest::decl(&cfg),
        ];
        let mut out = format!(
            "// Generated from app/src-tauri/src/ipc.rs by `{UPDATE}=1 cargo test -p separatrix \
             typescript_bindings`.\n"
        );
        for d in decls {
            out.push_str(&format!("\nexport {d}\n"));
        }
        let text = |s: &str| serde_json::to_string(s).expect("a string is JSON");
        let consts = [
            (
                "MAX_SECTION_POINTS",
                crate::service::MAX_SECTION_POINTS.to_string(),
            ),
            (
                "MAX_RENORMALIZATIONS",
                crate::service::MAX_RENORMALIZATIONS.to_string(),
            ),
            (
                "MAX_LYAPUNOV_ITERATES",
                crate::service::MAX_LYAPUNOV_ITERATES.to_string(),
            ),
            ("MAX_ORBIT_COLUMNS", MAX_ORBIT_COLUMNS.to_string()),
            (
                "BLOW_UP_THRESHOLD",
                format!("{:e}", dyn_solve::BLOW_UP_THRESHOLD),
            ),
            (
                "LYAPUNOV_HISTORY",
                crate::service::LYAPUNOV_HISTORY.to_string(),
            ),
            ("MAX_ORBIT_POINTS", MAX_ORBIT_POINTS.to_string()),
            (
                "MAX_CONTINUATION_POINTS",
                MAX_CONTINUATION_POINTS.to_string(),
            ),
            (
                "SAME_BIFURCATION_TOL",
                format!("{:e}", separatrix_export::model::SAME_BIFURCATION_TOL),
            ),
            ("SECTION_MISFIT", text(separatrix_project::SECTION_MISFIT)),
            ("SPACE_MISFIT", text(separatrix_project::SPACE_MISFIT)),
            ("LYAPUNOV_MISFIT", text(separatrix_project::LYAPUNOV_MISFIT)),
            ("ORBIT_MISFIT", text(separatrix_project::ORBIT_MISFIT)),
            (
                "ORBIT_MAX_TRANSIENT",
                separatrix_project::ORBIT_TRANSIENT_MAX.to_string(),
            ),
            (
                "ORBIT_MAX_KEPT",
                separatrix_project::ORBIT_KEPT_MAX.to_string(),
            ),
            (
                "SPACE_LATEX_REFUSAL",
                text(crate::export_service::SPACE_LATEX_REFUSAL),
            ),
            (
                "DEFAULT_CAMERA",
                format!(
                    "{{ yaw: {:?}, pitch: {:?}, zoom: {:?} }}",
                    separatrix_project::DEFAULT_YAW,
                    separatrix_project::DEFAULT_PITCH,
                    separatrix_project::DEFAULT_ZOOM
                ),
            ),
        ];
        for (name, value) in consts {
            out.push_str(&format!("\nexport const {name} = {value};\n"));
        }
        out
    }

    #[test]
    fn typescript_bindings_are_up_to_date() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GENERATED);
        let want = render();
        if std::env::var_os(UPDATE).is_some() {
            std::fs::write(&path, want).expect("write the generated bindings");
            return;
        }
        let have = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            have == want,
            "app/src/api/ipc.generated.ts is out of date; run {UPDATE}=1 cargo test -p separatrix \
             typescript_bindings"
        );
        ui_types_are_the_generated_ones(&want);
    }

    #[test]
    fn a_saved_orbit_diagram_fits_the_widest_plot() {
        let kept = separatrix_project::ORBIT_KEPT_MAX as usize;
        assert!(kept * MAX_ORBIT_COLUMNS <= MAX_ORBIT_POINTS);
        assert!((kept + 1) * MAX_ORBIT_COLUMNS > MAX_ORBIT_POINTS);
    }

    /// The UI imports its IPC types from `types.ts`; it must re-export every
    /// generated type rather than declare its own, or a stale hand-written
    /// copy would compile while the generated file goes unused.
    fn ui_types_are_the_generated_ones(generated: &str) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(UI_TYPES);
        let ui = std::fs::read_to_string(&path).expect("read app/src/api/types.ts");
        let block = ui
            .split("export type {")
            .nth(1)
            .and_then(|rest| rest.split_once("} from \"./ipc.generated\";"))
            .map(|(names, _)| names)
            .expect("types.ts re-exports from ./ipc.generated");
        let reexported: Vec<&str> = block
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let names: Vec<&str> = generated
            .lines()
            .filter_map(|l| l.strip_prefix("export type "))
            .filter_map(|l| l.split([' ', '=']).next())
            .collect();
        assert!(!names.is_empty());
        for name in &names {
            assert!(
                reexported.contains(name),
                "types.ts does not re-export {name} from ipc.generated.ts"
            );
            for own in [
                format!("interface {name} "),
                format!("interface {name}<"),
                format!("type {name} ="),
                format!("type {name}<"),
            ] {
                assert!(
                    !ui.contains(&own),
                    "types.ts declares its own {name}; re-export the generated one"
                );
            }
        }
    }
}
