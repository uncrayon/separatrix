//! The typed v1 schema.
//!
//! Every struct keeps the keys it doesn't know in `extra` (a flattened
//! `serde_json::Map`), so a file written by a newer Separatrix, or edited by
//! hand, keeps them on save. Preservation is value-level: unknown keys come
//! back with equal values, sorted by key (the crate doesn't enable serde_json's
//! `preserve_order`). Known keys are written in struct order, then the extras.
//!
//! Keys are snake_case: this is the one documented exception to the
//! camelCase IPC (the file format is meant to be read and edited by people).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{CURRENT_VERSION, FORMAT, MIN_READER_VERSION};

/// A whole `.sepx` file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectFile {
    /// Always `"separatrix"`.
    #[serde(default = "default_format")]
    pub format: String,
    /// The schema version the file was written in.
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    /// The oldest schema version that can read the file.
    #[serde(default = "default_min_reader_version")]
    pub min_reader_version: u32,
    /// The Separatrix version that saved the file (informational).
    #[serde(default)]
    pub app_version: String,
    pub system: ProjectSystem,
    #[serde(default)]
    pub initial_conditions: Vec<ProjectInitialCondition>,
    #[serde(default)]
    pub views: Vec<ProjectView>,
    /// Reserved; v1 keeps the entries verbatim.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "unknown[]"))]
    pub analyses: Vec<Value>,
    #[serde(default)]
    pub solver: ProjectSolver,
    /// Owned by the export dialog; opaque to Rust.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "unknown", optional))]
    pub export_settings: Option<Value>,
    /// Unknown keys, kept on save. This is where the v2 `assignment` key
    /// lives (v1 never reads or writes it; `null` survives too).
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// The equations, as typed, and their parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectSystem {
    /// `"ode"`, `"map"`, … Informational: recomputed on open.
    #[serde(default)]
    pub kind: String,
    /// `"latex"` or `"text"`.
    pub input_format: String,
    /// The source of truth: every line exactly as typed, blank lines included.
    pub lines: Vec<String>,
    /// State names, in the order `x0` arrays are written in.
    #[serde(default)]
    pub states: Vec<String>,
    #[serde(default)]
    pub params: Vec<ProjectParam>,
    /// The template the system came from; `null` for untitled work.
    #[serde(default)]
    pub template: Option<String>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// A parameter slider. `id` is the parameter's name (its path is `params.<id>`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectParam {
    pub id: String,
    pub value: f64,
    /// A missing bound reads as NaN and is repaired by validation.
    #[serde(default = "nan")]
    pub min: f64,
    #[serde(default = "nan")]
    pub max: f64,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// A trajectory's start point.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectInitialCondition {
    /// A missing or empty id is repaired by validation.
    #[serde(default)]
    pub id: String,
    /// One value per entry of `system.states`, in that order.
    pub x0: Vec<f64>,
    #[serde(default)]
    pub t0: f64,
    /// Index into the trajectory palette (normalized mod 7).
    #[serde(default)]
    pub color: i32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// A plot view. v1 edits only the primary view (id `main`, otherwise the
/// first) and keeps the others verbatim.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectView {
    /// A missing or empty id is repaired by validation.
    #[serde(default)]
    pub id: String,
    /// `"cobweb"` for a cobweb plot of a one-dimensional map (`x_axis` is
    /// state `s`, `y_axis` is `{ "kind": "next", "name": s }`); `"section"`
    /// for a Poincaré section of a flow (see `section`); `"space"` for a 3D
    /// view of a flow (three states: `x_axis`, `y_axis`, `z_axis`; see
    /// `camera`); `"lyapunov"` for the Lyapunov spectrum of a flow or a map
    /// (see `lyapunov`); `"orbit"` for the orbit diagram of a map (`x_axis`
    /// is `{ "kind": "param", "name": p }`, `y_axis` a state; see `orbit`);
    /// `"continuation"` for a branch plot of fixed points against a
    /// parameter (see `continuation`); `"curves"` for the Parameter plane,
    /// bifurcation curves in two parameters (`x_axis` and `y_axis` are
    /// `{ "kind": "param", "name": p }`; see `curves`); absent for the phase
    /// plane and the time series, which the axes tell apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<String>,
    /// The section of a `"section"` view. Its crossings are plotted in two
    /// states (`x_axis`, `y_axis`), or as the return map when `y_axis` is
    /// `{ "kind": "next", "name": s }` of the state `s` across.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub section: Option<ProjectSection>,
    pub x_axis: ProjectAxis,
    pub y_axis: ProjectAxis,
    /// The third axis of a `"space"` view (a 3D view of a flow): a state, as
    /// `x_axis` and `y_axis` are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub z_axis: Option<ProjectAxis>,
    /// The user's ranges (not the ones equalized for the plot's aspect).
    pub x_range: [f64; 2],
    pub y_range: [f64; 2],
    /// The range of `z_axis` in a `"space"` view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub z_range: Option<[f64; 2]>,
    /// Where a `"space"` view is seen from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub camera: Option<ProjectCamera>,
    /// The run of a `"lyapunov"` view (the running estimates of the
    /// Lyapunov spectrum: `x_axis` is time, `y_axis` is
    /// `{ "kind": "exponent" }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub lyapunov: Option<ProjectLyapunov>,
    /// The columns of an `"orbit"` view (the orbit diagram of a map over
    /// the range of the parameter across).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub orbit: Option<ProjectOrbit>,
    /// The requests of a `"continuation"` view (a branch plot: `x_axis` is
    /// `{ "kind": "param", "name": p }`, `y_axis` a state or
    /// `{ "kind": "norm" }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub continuation: Option<ProjectContinuation>,
    /// The requests of a `"curves"` view (the Parameter plane: `x_axis` is
    /// the parameter across, `y_axis` the one up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub curves: Option<ProjectCurves>,
    #[serde(default)]
    pub slice: ProjectSlice,
    #[serde(default)]
    pub layers: ProjectLayers,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// A Poincaré section: `{ "kind": "plane", "state": "z", "value": 27,
/// "direction": "rising" }` (the plane `z = 27`, crossed upwards; `"falling"`
/// or `"both"` otherwise) or `{ "kind": "stroboscopic", "period": 6.28…,
/// "phase": 0 }` (the state at `t = phase + k·period`).
///
/// A lenient struct like [`ProjectAxis`], so a section kind from a newer
/// version is a warning, not an unreadable file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectSection {
    pub kind: String,
    /// The plane's state, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<f64>,
    /// `"rising"`, `"falling"` or `"both"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub period: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub phase: Option<f64>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// A Lyapunov run: `{ "transient": 100, "total": 1000, "interval": 0.1,
/// "blocks": 10 }`. A flow's are times (`interval` is τ, the time between QR
/// steps); a map's are iterates (whole numbers). `blocks` splits the run for
/// the ± spread.
///
/// Lenient like [`ProjectSection`]: a missing value makes the view a
/// warning (the default view is shown), not an unreadable file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectLyapunov {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub transient: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub total: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub interval: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub blocks: Option<u32>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// The columns of an orbit diagram: `{ "transient": 1000, "kept": 200 }`,
/// iterates dropped and then drawn in every column, each a whole number.
///
/// Lenient like [`ProjectLyapunov`]: a missing value makes the view a
/// warning (the default view is shown), not an unreadable file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectOrbit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub transient: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kept: Option<u32>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// The requests of a branch plot: the parameter interval, the first step in
/// arclength, the most points per direction, and the seeds the branches are
/// followed from. Seeds are saved, never the computed branches, so opening
/// the file replays the same requests.
///
/// Lenient like [`ProjectOrbit`]: a missing value takes the app's default.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectContinuation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub range: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_points: Option<u32>,
    #[serde(default)]
    pub seeds: Vec<ProjectSeed>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// Where one branch starts: `from` is `"point"` (a fixed point, corrected by
/// Newton's method first) or `"switch"` (onto the other branch at a special
/// point of `kind` `"branchPoint"` or `"flip"`, whose `tangent` is the old
/// branch's there, `x` and then the parameter). `direction` is `"forward"`,
/// `"backward"` or `"both"`; `period` is `k` of `f^k` for a map, 1 for a
/// flow.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectSeed {
    pub from: String,
    /// One value per entry of `system.states`, in that order.
    pub x: Vec<f64>,
    /// The value of the view's parameter.
    pub param: f64,
    #[serde(default = "one")]
    pub period: u32,
    pub direction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tangent: Option<Vec<f64>>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// The requests of the Parameter plane: the box the curves stay in (the
/// parameter across, then the one up), the first step in arclength, the most
/// points per direction, which way each curve is followed, the bifurcations
/// the curves are followed from, the Bogdanov–Takens points the homoclinic
/// curves are followed from, and the Branch view the plane came from. Seeds
/// are saved, never the computed curves, so opening the file replays the same
/// requests.
///
/// Lenient like [`ProjectContinuation`]: a missing value makes the view a
/// warning, not an unreadable file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectCurves {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub across_range: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub up_range: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub ds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_points: Option<u32>,
    /// `"forward"`, `"backward"` or `"both"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub direction: Option<String>,
    #[serde(default)]
    pub seeds: Vec<ProjectCurveSeed>,
    #[serde(default)]
    pub homoclinic: Vec<ProjectHomoclinicSeed>,
    /// The Branch view the plane came from: its range is the slice drawn
    /// across the plane, and the Branch view shows it again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub branch: Option<ProjectBranch>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// Where one curve of the Parameter plane starts: a bifurcation of `kind`
/// `"fold"`, `"hopf"`, `"branchPoint"`, `"flip"` or `"neimarkSacker"`, the
/// state there, `period` (`k` of `f^k` for a map, 1 for a flow), and every
/// parameter's value there by name, so the seed keeps its meaning whichever
/// two parameters are free.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectCurveSeed {
    pub kind: String,
    /// One value per entry of `system.states`, in that order.
    pub x: Vec<f64>,
    #[serde(default = "one")]
    pub period: u32,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// Where a homoclinic curve of a flow starts: a Bogdanov–Takens point, its
/// equilibrium and every parameter's value there by name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectHomoclinicSeed {
    /// One value per entry of `system.states`, in that order.
    pub x: Vec<f64>,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// The Branch view a Parameter plane came from, with the keys of a
/// `"continuation"` view, and every parameter's value by name when the plane
/// opened from it (its branches were computed there).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectBranch {
    pub x_axis: ProjectAxis,
    pub y_axis: ProjectAxis,
    pub x_range: [f64; 2],
    pub y_range: [f64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub continuation: Option<ProjectContinuation>,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// The camera of a 3D view: `{ "yaw": -0.785…, "pitch": 0.523…, "zoom": 1 }`.
/// `yaw` turns the box about its third axis and `pitch` tilts it towards the
/// viewer, both in radians; `zoom` scales it. The app clamps `pitch` to
/// ±(π/2 − 0.01) and `zoom` to 0.25–8 when it opens the file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectCamera {
    #[serde(default = "default_yaw")]
    pub yaw: f64,
    #[serde(default = "default_pitch")]
    pub pitch: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// An axis: `{ "kind": "state", "name": "x" }`, `{ "kind": "time" }`, in a
/// cobweb view `{ "kind": "next", "name": "x" }` (the state one iterate
/// later, `x_{n+1}`), in a Lyapunov view `{ "kind": "exponent" }` (λ), or in
/// an orbit diagram or a branch plot `{ "kind": "param", "name": "r" }` (a
/// parameter), or in a branch plot `{ "kind": "norm" }` (‖x‖₂).
///
/// A lenient struct rather than a tagged enum, so an axis kind from a newer
/// version is kept instead of making the whole file unreadable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectAxis {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub name: Option<String>,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

impl ProjectAxis {
    pub fn state(name: &str) -> Self {
        ProjectAxis {
            kind: "state".into(),
            name: Some(name.into()),
            extra: Map::new(),
        }
    }

    /// The state `name` one iterate later (a cobweb view's vertical axis).
    pub fn next(name: &str) -> Self {
        ProjectAxis {
            kind: "next".into(),
            name: Some(name.into()),
            extra: Map::new(),
        }
    }

    /// The Lyapunov exponents' axis (a Lyapunov view's vertical axis).
    pub fn exponent() -> Self {
        ProjectAxis {
            kind: "exponent".into(),
            name: None,
            extra: Map::new(),
        }
    }

    /// The parameter `name` (an orbit diagram's horizontal axis).
    pub fn param(name: &str) -> Self {
        ProjectAxis {
            kind: "param".into(),
            name: Some(name.into()),
            extra: Map::new(),
        }
    }

    pub fn time() -> Self {
        ProjectAxis {
            kind: "time".into(),
            name: None,
            extra: Map::new(),
        }
    }
}

/// Where the field is sliced: the values of the states that aren't on an
/// axis, and the time for non-autonomous systems.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectSlice {
    #[serde(default)]
    pub states: BTreeMap<String, f64>,
    #[serde(default)]
    pub t: f64,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// Which layers the view draws.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectLayers {
    #[serde(default = "yes")]
    pub field: bool,
    /// Arrows per side, clamped to 2..=200.
    #[serde(default = "default_field_grid")]
    pub field_grid: u32,
    #[serde(default = "yes")]
    pub nullclines: bool,
    #[serde(default = "yes")]
    pub fixed_points: bool,
    #[serde(default = "yes")]
    pub backward: bool,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

impl Default for ProjectLayers {
    fn default() -> Self {
        ProjectLayers {
            field: true,
            field_grid: default_field_grid(),
            nullclines: true,
            fixed_points: true,
            backward: true,
            extra: Map::new(),
        }
    }
}

/// Solver settings. `method` is a string so a method from a newer version
/// is a warning (it falls back to `dopri5`), not a hard error.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectSolver {
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_rtol")]
    pub rtol: f64,
    #[serde(default = "default_atol")]
    pub atol: f64,
    /// Fixed step size; 0 means "the method's default".
    #[serde(default)]
    pub h: f64,
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    #[serde(default = "default_t_end")]
    pub t_end: f64,
    /// Map iterates computed but not drawn before each orbit; absent = 0.
    /// Written only when above 0. Validation clamps it to
    /// `0..=TRANSIENT_MAX`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub transient: Option<i64>,
    /// Samples per trajectory; 0 means automatic.
    #[serde(default)]
    pub samples: u32,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

impl Default for ProjectSolver {
    fn default() -> Self {
        ProjectSolver {
            method: default_method(),
            rtol: default_rtol(),
            atol: default_atol(),
            h: 0.0,
            max_steps: default_max_steps(),
            t_end: default_t_end(),
            transient: None,
            samples: 0,
            extra: Map::new(),
        }
    }
}

fn default_format() -> String {
    FORMAT.to_string()
}
fn default_format_version() -> u32 {
    CURRENT_VERSION
}
fn default_min_reader_version() -> u32 {
    MIN_READER_VERSION
}
fn nan() -> f64 {
    f64::NAN
}
fn yes() -> bool {
    true
}
fn one() -> u32 {
    1
}
/// The yaw of a camera that gives none.
pub const DEFAULT_YAW: f64 = -std::f64::consts::FRAC_PI_4;
/// The pitch of a camera that gives none.
pub const DEFAULT_PITCH: f64 = std::f64::consts::PI / 6.0;
/// The zoom of a camera that gives none.
pub const DEFAULT_ZOOM: f64 = 1.0;

fn default_yaw() -> f64 {
    DEFAULT_YAW
}
fn default_pitch() -> f64 {
    DEFAULT_PITCH
}
fn default_zoom() -> f64 {
    DEFAULT_ZOOM
}
pub(crate) fn default_field_grid() -> u32 {
    21
}
pub(crate) fn default_method() -> String {
    "dopri5".to_string()
}
pub(crate) fn default_rtol() -> f64 {
    1e-6
}
pub(crate) fn default_atol() -> f64 {
    1e-9
}
pub(crate) fn default_max_steps() -> u32 {
    200_000
}
pub(crate) fn default_t_end() -> f64 {
    20.0
}
