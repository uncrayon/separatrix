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
    pub x_axis: ProjectAxis,
    pub y_axis: ProjectAxis,
    /// The user's ranges (not the ones equalized for the plot's aspect).
    pub x_range: [f64; 2],
    pub y_range: [f64; 2],
    #[serde(default)]
    pub slice: ProjectSlice,
    #[serde(default)]
    pub layers: ProjectLayers,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub extra: Map<String, Value>,
}

/// An axis: `{ "kind": "state", "name": "x" }` or `{ "kind": "time" }`.
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
