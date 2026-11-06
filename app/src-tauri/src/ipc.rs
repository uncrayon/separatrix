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
    /// Relative tolerance (dopri5 only).
    pub rtol: f64,
    /// Absolute tolerance (dopri5 only).
    pub atol: f64,
    /// Fixed step for euler/heun/rk4; initial step for dopri5 (0 = automatic).
    pub h: f64,
    /// Largest allowed step for dopri5; absent = no limit.
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
    /// eigenvalue, sign-normalised so its largest-magnitude component is
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

// ---------------------------------------------------------------------
// Project files (`.sepx`). The project itself is `ProjectFile` from
// `separatrix-project`, whose keys are snake_case on the wire (the one
// documented exception to camelCase; see docs/project-format.md).
// ---------------------------------------------------------------------

pub use separatrix_project::{
    ProjectAxis, ProjectCamera, ProjectFile, ProjectInitialCondition, ProjectLayers, ProjectParam,
    ProjectSection, ProjectSlice, ProjectSolver, ProjectSystem, ProjectView,
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

/// Colour theme of an exported figure.
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
            PortraitResponse::decl(&cfg),
            LyapunovRequest::decl(&cfg),
            LyapunovResponse::decl(&cfg),
            OrbitDiagramRequest::decl(&cfg),
            DoublingOut::decl(&cfg),
            OrbitDiagramResponse::decl(&cfg),
            ProjectFile::decl(&cfg),
            ProjectSystem::decl(&cfg),
            ProjectParam::decl(&cfg),
            ProjectInitialCondition::decl(&cfg),
            ProjectView::decl(&cfg),
            ProjectSection::decl(&cfg),
            ProjectCamera::decl(&cfg),
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
            DocumentStateRequest::decl(&cfg),
            ProjectOpenRequested::decl(&cfg),
            CloseRequested::decl(&cfg),
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
            ("MAX_ORBIT_COLUMNS", MAX_ORBIT_COLUMNS.to_string()),
            ("MAX_ORBIT_POINTS", MAX_ORBIT_POINTS.to_string()),
            ("SECTION_MISFIT", text(separatrix_project::SECTION_MISFIT)),
            ("SPACE_MISFIT", text(separatrix_project::SPACE_MISFIT)),
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
