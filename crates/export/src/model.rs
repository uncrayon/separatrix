//! What an export is built from: one computed portrait, the names to label
//! it with, and the figure's size and colours.
//!
//! The shapes mirror the IPC types (`PortraitResponse` and its request) so
//! the app crate can convert one into the other field by field; this crate
//! does not depend on the app.

use serde::{Deserialize, Serialize};

pub use dyn_analysis::Classification;
pub use dyn_core::SystemKind;
pub use dyn_solve::{Direction, Status};

use crate::num::shortest;

/// What a plot axis shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Axis {
    /// State variable number `index`.
    State {
        /// Index into the state vector.
        index: usize,
    },
    /// Time `t` (flows) or the iterate `n` (maps).
    Time,
}

impl Axis {
    /// The value this axis shows for sample `k` of a trajectory.
    pub fn value(self, t: &[f64], x: &[f64], dim: usize, k: usize) -> f64 {
        match self {
            Axis::Time => t[k],
            Axis::State { index } => x[k * dim + index],
        }
    }
}

/// Where plot coordinates land in the figure, in CSS pixels (y down), as
/// `app/src/canvas/viewport.ts` maps them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    /// Left edge of the plot box.
    pub left: f64,
    /// Top edge of the plot box.
    pub top: f64,
    /// Width of the plot box.
    pub width: f64,
    /// Height of the plot box.
    pub height: f64,
    /// Horizontal range shown.
    pub x_range: (f64, f64),
    /// Vertical range shown.
    pub y_range: (f64, f64),
}

impl Viewport {
    /// A plot box at the origin.
    pub fn new(width: f64, height: f64, x_range: (f64, f64), y_range: (f64, f64)) -> Viewport {
        Viewport {
            left: 0.0,
            top: 0.0,
            width,
            height,
            x_range,
            y_range,
        }
    }

    /// `sx`: horizontal pixel of `x`.
    pub fn sx(&self, x: f64) -> f64 {
        self.left + (x - self.x_range.0) / (self.x_range.1 - self.x_range.0) * self.width
    }

    /// `sy`: vertical pixel of `y` (down).
    pub fn sy(&self, y: f64) -> f64 {
        self.top + (1.0 - (y - self.y_range.0) / (self.y_range.1 - self.y_range.0)) * self.height
    }

    /// Plot units per pixel on each axis.
    pub fn units_per_px(&self) -> (f64, f64) {
        (
            (self.x_range.1 - self.x_range.0) / self.width,
            (self.y_range.1 - self.y_range.0) / self.height,
        )
    }

    /// Right edge.
    pub fn right(&self) -> f64 {
        self.left + self.width
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f64 {
        self.top + self.height
    }
}

/// One arrow of the direction field (raw field components).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Arrow {
    /// Position.
    pub x: f64,
    /// Position.
    pub y: f64,
    /// Horizontal component.
    pub u: f64,
    /// Vertical component.
    pub v: f64,
}

/// The curves where one equation's right-hand side vanishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nullcline {
    /// Which equation.
    pub state: usize,
    /// Polylines in plot coordinates.
    pub polylines: Vec<Vec<(f64, f64)>>,
}

/// A fixed point and its linearization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixedPoint {
    /// Full state vector.
    pub x: Vec<f64>,
    /// Eigenvalues (multipliers for maps) as `(re, im)`.
    pub eigenvalues: Vec<(f64, f64)>,
    /// Strogatz's name for it.
    pub classification: Classification,
    /// Asymptotically stable.
    pub stable: bool,
}

/// Which initial condition a trajectory belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryTag {
    /// The initial condition's project id (`"ic-3"`).
    pub ic_id: String,
    /// Integrated backward in time from the start point.
    pub backward: bool,
    /// Palette index (taken mod 7).
    pub color: u8,
}

/// One computed run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trajectory {
    /// Which initial condition and direction.
    pub tag: TrajectoryTag,
    /// The start point (full state vector).
    pub x0: Vec<f64>,
    /// The start time (0 for maps).
    pub t0: f64,
    /// Times (iterate numbers for maps), in integration order.
    pub t: Vec<f64>,
    /// States, row-major, `t.len() * dim` values.
    pub x: Vec<f64>,
    /// Why the run stopped.
    pub status: Status,
}

/// One computed portrait, at the figure's aspect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Portrait {
    /// Flow or map.
    pub kind: SystemKind,
    /// Number of states.
    pub dim: usize,
    /// Horizontal axis.
    pub x_axis: Axis,
    /// Vertical axis.
    pub y_axis: Axis,
    /// Horizontal range drawn (already equalized for the figure's aspect).
    pub x_range: (f64, f64),
    /// Vertical range drawn.
    pub y_range: (f64, f64),
    /// Arrows per side of the direction field (0 = none).
    pub field_grid: usize,
    /// The direction field.
    pub arrows: Vec<Arrow>,
    /// Nullclines.
    pub nullclines: Vec<Nullcline>,
    /// Fixed points.
    pub fixed_points: Vec<FixedPoint>,
    /// Trajectories, any order (backward runs are drawn first).
    pub trajectories: Vec<Trajectory>,
    /// A cobweb plot of a one-dimensional map (both axes are state 0): each
    /// map trajectory is drawn as a staircase, with the diagonal and the
    /// graph of `f` (see [`crate::cobweb`]).
    #[serde(default)]
    pub cobweb: bool,
    /// The graph of `f` in a cobweb plot: polylines of `(x, f(x))`.
    #[serde(default)]
    pub graph: Vec<Vec<(f64, f64)>>,
    /// Map iterates computed but not drawn before each orbit (its first
    /// sample is `x_T`), or, with a `section`, the crossings skipped before
    /// each orbit's first.
    #[serde(default)]
    pub transient: usize,
    /// A Poincaré section of a flow: each trajectory is the orbit's
    /// crossings, drawn as dots; with both axes one state, the return map
    /// `(x_k, x_{k+1})` and the diagonal.
    #[serde(default)]
    pub section: Option<SectionSpec>,
}

/// Where a Poincaré section samples a flow, as the IPC's `SectionSpec`
/// (a stroboscopic phase is absolute time modulo the period).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SectionSpec {
    /// The plane `x[state] = value`, crossed in `direction`.
    Plane {
        /// Index of the state held fixed.
        state: usize,
        /// Where the plane cuts that state's axis.
        value: f64,
        /// Which crossings count.
        direction: Direction,
    },
    /// The state at every `t` with `t mod period = phase`.
    Stroboscopic {
        /// Forcing period.
        period: f64,
        /// Sample time modulo `period`.
        phase: f64,
    },
}

impl SectionSpec {
    /// The section in words, for the CSV metadata and the figure's header:
    /// `plane z = 27 (increasing)` or `stroboscopic, period 6.28, phase 0`.
    pub fn describe(&self, labels: &Labels) -> String {
        match *self {
            SectionSpec::Plane {
                state,
                value,
                direction,
            } => {
                let name = labels.states.get(state).map_or("?", |s| s.name.as_str());
                let way = match direction {
                    Direction::Rising => "increasing",
                    Direction::Falling => "decreasing",
                    Direction::Both => "both directions",
                };
                format!("plane {name} = {} ({way})", shortest(value))
            }
            SectionSpec::Stroboscopic { period, phase } => format!(
                "stroboscopic, period {}, phase {}",
                shortest(period),
                shortest(phase)
            ),
        }
    }
}

/// A state's names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateName {
    /// Identity (`"theta'"`), used in CSV headers.
    pub name: String,
    /// Display form (`"\\dot{\\theta}"`).
    pub latex: String,
}

/// A parameter's names and its current value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamValue {
    /// Identity (`"sigma"`).
    pub name: String,
    /// Display form (`"\\sigma"`).
    pub latex: String,
    /// The slider's value.
    pub value: f64,
}

/// The names a system's exports are labelled with, from the compiled
/// system, plus the parameter values in use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Labels {
    /// States in order.
    pub states: Vec<StateName>,
    /// Parameters in order, with their values.
    pub params: Vec<ParamValue>,
}

impl Labels {
    /// From a compiled system's info and the parameter values in use (in
    /// the system's parameter order; missing values are the defaults).
    pub fn from_info(info: &dyn_expr::SystemInfo, values: &[f64]) -> Labels {
        Labels {
            states: info
                .states
                .iter()
                .map(|s| StateName {
                    name: s.name.clone(),
                    latex: s.latex.clone(),
                })
                .collect(),
            params: info
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| ParamValue {
                    name: p.name.clone(),
                    latex: p.latex.clone(),
                    value: values.get(i).copied().unwrap_or(p.default),
                })
                .collect(),
        }
    }
}

/// The export's colour theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Projector (whiteboard), the default for exports.
    Light,
    /// Chalkboard.
    Dark,
}

/// The plot colours of one theme, as `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    /// `plot-bg`.
    pub bg: String,
    /// `plot-grid`.
    pub grid: String,
    /// `plot-axis`.
    pub axis: String,
    /// `plot-tick`.
    pub tick: String,
    /// `plot-field`.
    pub field: String,
    /// `plot-fp`.
    pub fp: String,
    /// `plot-halo`.
    pub halo: String,
    /// `traj-1` … `traj-7`.
    pub traj: Vec<String>,
    /// `null-1` … `null-4`.
    pub nullcline: Vec<String>,
}

impl Palette {
    /// The palette of `theme` from the design system's `tokens.json`
    /// (`app/src/styles/ds/tokens.json`, which the app embeds).
    pub fn from_tokens(tokens_json: &str, theme: Theme) -> Result<Palette, String> {
        let doc: serde_json::Value =
            serde_json::from_str(tokens_json).map_err(|e| format!("tokens.json: {e}"))?;
        let key = match theme {
            Theme::Light => "light",
            Theme::Dark => "dark",
        };
        let tokens = doc["color"]["tokens"]
            .as_array()
            .ok_or("tokens.json has no color.tokens")?;
        let get = |name: &str| -> Result<String, String> {
            let v = tokens
                .iter()
                .find(|t| t["name"] == name)
                .and_then(|t| t["value"][key].as_str())
                .ok_or_else(|| format!("tokens.json has no {key} value for {name}"))?;
            let ok =
                v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|b| b.is_ascii_hexdigit());
            if ok {
                Ok(v.to_ascii_lowercase())
            } else {
                Err(format!("{name} is {v}, not a #rrggbb color"))
            }
        };
        Ok(Palette {
            bg: get("plot-bg")?,
            grid: get("plot-grid")?,
            axis: get("plot-axis")?,
            tick: get("plot-tick")?,
            field: get("plot-field")?,
            fp: get("plot-fp")?,
            halo: get("plot-halo")?,
            traj: (1..=7)
                .map(|i| get(&format!("traj-{i}")))
                .collect::<Result<_, _>>()?,
            nullcline: (1..=4)
                .map(|i| get(&format!("null-{i}")))
                .collect::<Result<_, _>>()?,
        })
    }
}

/// Number of trajectory colours.
pub const PALETTE_SIZE: usize = 7;
