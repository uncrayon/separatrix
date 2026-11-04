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
//! (`"dopri5"`, ...), [`Status`] (`"blewUp"`, ...), [`Stats`] and
//! [`Classification`] (`"stable spiral"`, ...).

use serde::{Deserialize, Serialize};

pub use dyn_analysis::Classification;
pub use dyn_core::SystemKind;
pub use dyn_expr::{InputFormat, Severity};
pub use dyn_solve::{Method, Stats, Status};

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
    /// Wall time spent in Rust, in milliseconds.
    pub elapsed_ms: f64,
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
            Classification::decl(&cfg),
            CompileRequest::decl(&cfg),
            Diagnostic::decl(&cfg),
            StateInfo::decl(&cfg),
            ParamInfo::decl(&cfg),
            CompileResponse::decl(&cfg),
            AxisRef::decl(&cfg),
            SolverOptions::decl(&cfg),
            TrajectoryRequest::decl(&cfg),
            PortraitRequest::decl(&cfg),
            Arrow::decl(&cfg),
            NullclineOut::decl(&cfg),
            FixedPointOut::decl(&cfg),
            TrajectoryOut::decl(&cfg),
            PortraitResponse::decl(&cfg),
        ];
        let mut out = format!(
            "// Generated from app/src-tauri/src/ipc.rs by `{UPDATE}=1 cargo test -p separatrix \
             typescript_bindings`.\n"
        );
        for d in decls {
            out.push_str(&format!("\nexport {d}\n"));
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
