//! What the Tauri commands actually do, without any Tauri types.
//!
//! Everything here is plain Rust so it can be unit-tested without a window:
//! [`SystemStore`] keeps compiled systems between calls, [`compile_system`]
//! answers `compile_system` and [`compute_portrait`] answers
//! `compute_portrait`. `lib.rs` only wires these to the webview.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use dyn_analysis::{Axis, Plane};
use dyn_core::{SystemKind, VectorField};
use dyn_expr::CompiledSystem;
use dyn_solve::Sampling;

use crate::ipc::{
    Arrow, AxisRef, CompileRequest, CompileResponse, Diagnostic, FixedPointOut, NullclineOut,
    ParamInfo, PortraitRequest, PortraitResponse, SymbolInfo, TrajectoryOut, TrajectoryRequest,
};

/// How many compiled systems are kept. The UI only ever uses the newest one,
/// but a portrait request for the previous one may still be in flight while
/// the user types, so a few older ones are kept too.
pub const MAX_STORED_SYSTEMS: usize = 32;

/// Marching-squares lattice size for nullclines (cells per side).
pub const NULLCLINE_GRID: usize = 160;

/// Upper limits that protect the app from absurd requests (a typo such as
/// a field grid of 100000 would otherwise freeze the computation).
pub const MAX_FIELD_GRID: usize = 200;
/// Largest number of dense samples per trajectory.
pub const MAX_SAMPLES: usize = 200_000;
/// Largest number of map iterates per trajectory.
pub const MAX_ITERATES: usize = 1_000_000;

/// Compiled systems, by the id handed to the UI.
///
/// Ids start at 1 and only ever increase. Systems are shared as
/// `Arc<CompiledSystem>` so a long computation can keep using one after it
/// has been evicted.
#[derive(Default)]
pub struct SystemStore {
    inner: Mutex<StoreInner>,
}

#[derive(Default)]
struct StoreInner {
    last_id: u32,
    systems: HashMap<u32, Arc<CompiledSystem>>,
}

impl SystemStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `system` and returns its new id. Drops the oldest systems
    /// beyond [`MAX_STORED_SYSTEMS`].
    pub fn insert(&self, system: CompiledSystem) -> u32 {
        let mut inner = self.lock();
        inner.last_id += 1;
        let id = inner.last_id;
        inner.systems.insert(id, Arc::new(system));
        let oldest_kept = id.saturating_sub(MAX_STORED_SYSTEMS as u32 - 1);
        inner.systems.retain(|&k, _| k >= oldest_kept);
        id
    }

    /// The system with this id, if it is still stored.
    pub fn get(&self, id: u32) -> Option<Arc<CompiledSystem>> {
        self.lock().systems.get(&id).cloned()
    }

    /// Number of systems currently stored.
    pub fn len(&self) -> usize {
        self.lock().systems.len()
    }

    /// `true` if no system is stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StoreInner> {
        // A panic while holding the lock cannot leave the map half-updated
        // in a harmful way, so a poisoned lock is simply reused.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// ------------------------------------------------------------------ compile

/// Compiles the user's equations and, on success, stores the system.
pub fn compile_system(store: &SystemStore, req: &CompileRequest) -> CompileResponse {
    let out = dyn_expr::compile(&req.source, req.format);
    let diagnostics = out
        .diagnostics
        .into_iter()
        .map(|d| Diagnostic {
            severity: d.severity,
            message: d.message,
            span: d.span,
        })
        .collect();
    match out.system {
        Some(system) => {
            let info = system.info.clone();
            let id = store.insert(system);
            CompileResponse {
                ok: true,
                system_id: Some(id),
                kind: Some(info.kind),
                hamiltonian: info.hamiltonian,
                autonomous: info.autonomous,
                states: info
                    .states
                    .into_iter()
                    .map(|s| SymbolInfo {
                        name: s.name,
                        latex: s.latex,
                    })
                    .collect(),
                params: info
                    .params
                    .into_iter()
                    .map(|p| ParamInfo {
                        name: p.name,
                        latex: p.latex,
                        default: p.default,
                    })
                    .collect(),
                equations_latex: info.equations_latex,
                diagnostics,
            }
        }
        None => CompileResponse {
            ok: false,
            system_id: None,
            kind: None,
            hamiltonian: false,
            autonomous: true,
            states: Vec::new(),
            params: Vec::new(),
            equations_latex: Vec::new(),
            diagnostics,
        },
    }
}

// ----------------------------------------------------------------- portrait

/// Computes everything the canvas draws for one view.
///
/// - **Direction field**: `fieldGrid × fieldGrid` arrows (flows only).
/// - **Nullclines**: only when requested, flows only.
/// - **Fixed points**: only when requested, for autonomous flows and maps.
///   Axis variables are searched over the visible range; hidden variables
///   over `baseState ± span` (see [`fixed_point_search_box`]).
/// - **Trajectories**: maps are iterated `t1` times from `x0` (the time axis
///   is `n`); flows are integrated from `t0` to `t1` (backwards if
///   `t1 < t0`) with `samples` evenly spaced output points.
///
/// Returns an error message (shown to the user) when the request does not
/// fit the system, e.g. the wrong number of parameters.
pub fn compute_portrait(
    system: &CompiledSystem,
    req: &PortraitRequest,
) -> Result<PortraitResponse, String> {
    let started = Instant::now();
    validate(system, req)?;
    let f: &dyn VectorField = system;
    let p = req.params.as_slice();
    let dim = f.dim();
    let is_map = f.kind() == SystemKind::Map;

    let mut base_state = req.base_state.clone();
    base_state.resize(dim, 0.0);
    let plane = Plane {
        x: to_axis(req.x_axis),
        y: to_axis(req.y_axis),
        x_range: req.x_range,
        y_range: req.y_range,
        base_state,
        t: req.field_time,
    };

    let arrows = if req.field_grid > 0 && !is_map {
        dyn_analysis::slope_field(f, p, &plane, req.field_grid.min(MAX_FIELD_GRID))
            .into_iter()
            .map(|a| Arrow {
                x: a.x,
                y: a.y,
                u: a.u,
                v: a.v,
            })
            .collect()
    } else {
        Vec::new()
    };

    let nullclines = if req.nullclines && !is_map {
        dyn_analysis::nullclines(f, p, &plane, NULLCLINE_GRID)
            .into_iter()
            .map(|n| NullclineOut {
                state: n.state,
                polylines: n.polylines,
            })
            .collect()
    } else {
        Vec::new()
    };

    let fixed_points = if req.fixed_points && f.is_autonomous() {
        let search = fixed_point_search_box(req, dim);
        dyn_analysis::fixed_points(f, p, &search, seeds_per_axis(dim))
            .into_iter()
            .filter(|fp| {
                fp.x.iter().all(|v| v.is_finite())
                    && fp
                        .eigenvalues
                        .iter()
                        .all(|(re, im)| re.is_finite() && im.is_finite())
            })
            .map(|fp| FixedPointOut {
                x: fp.x,
                eigenvalues: fp.eigenvalues,
                classification: fp.classification,
                stable: fp.stable,
            })
            .collect()
    } else {
        Vec::new()
    };

    let trajectories = req
        .trajectories
        .iter()
        .map(|tr| trajectory(f, p, tr, req))
        .collect();

    Ok(PortraitResponse {
        request_id: req.request_id,
        arrows,
        nullclines,
        fixed_points,
        trajectories,
        elapsed_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

/// Checks that the request fits the system, with messages a student can act on.
fn validate(f: &dyn VectorField, req: &PortraitRequest) -> Result<(), String> {
    let dim = f.dim();
    if req.params.len() != f.n_params() {
        return Err(format!(
            "Expected {} parameter value(s) but got {}.",
            f.n_params(),
            req.params.len()
        ));
    }
    for (name, axis) in [("horizontal", req.x_axis), ("vertical", req.y_axis)] {
        if let AxisRef::State { index } = axis {
            if index >= dim {
                return Err(format!(
                    "The {name} axis shows state {} but the system only has {dim}.",
                    index + 1
                ));
            }
        }
    }
    for (name, (lo, hi)) in [("horizontal", req.x_range), ("vertical", req.y_range)] {
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(format!(
                "The {name} range [{lo}, {hi}] is not a valid interval."
            ));
        }
    }
    for (k, tr) in req.trajectories.iter().enumerate() {
        if tr.x0.len() != dim {
            return Err(format!(
                "Starting point {} has {} coordinate(s) but the system has {dim}.",
                k + 1,
                tr.x0.len()
            ));
        }
    }
    Ok(())
}

fn to_axis(a: AxisRef) -> Axis {
    match a {
        AxisRef::State { index } => Axis::State(index),
        AxisRef::Time => Axis::Time,
    }
}

/// Where to look for fixed points, one `(min, max)` per state.
///
/// A variable on an axis is searched over its visible range. A hidden
/// variable (e.g. `y` in the x–z view of the Lorenz system) is searched
/// over `baseState[i] ± span`, where `span` is half the larger visible
/// range of the state axes (10 if no state is on an axis). Fixed points
/// usually live at a similar scale to the visible ones, so this finds e.g.
/// both Lorenz C± points even though `y` is not shown.
pub fn fixed_point_search_box(req: &PortraitRequest, dim: usize) -> Vec<(f64, f64)> {
    let visible = [(req.x_axis, req.x_range), (req.y_axis, req.y_range)];
    let span = visible
        .iter()
        .filter(|(a, _)| matches!(a, AxisRef::State { .. }))
        .map(|(_, (lo, hi))| (hi - lo).abs() / 2.0)
        .fold(0.0_f64, f64::max);
    let span = if span > 0.0 { span } else { 10.0 };
    (0..dim)
        .map(|i| {
            let on_axis = visible
                .iter()
                .find(|(a, _)| *a == AxisRef::State { index: i })
                .map(|(_, r)| *r);
            on_axis.unwrap_or_else(|| {
                let c = req
                    .base_state
                    .get(i)
                    .copied()
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0);
                (c - span, c + span)
            })
        })
        .collect()
}

/// Newton starting points per axis: a dense grid in 1D and 2D, coarser in
/// higher dimensions so the total stays around a thousand seeds.
fn seeds_per_axis(dim: usize) -> usize {
    match dim {
        0 => 0,
        1 => 40,
        2 => 14,
        3 => 9,
        d => ((1000.0_f64).powf(1.0 / d as f64).floor() as usize).max(2),
    }
}

/// One trajectory: map iteration or ODE integration.
fn trajectory(
    f: &dyn VectorField,
    p: &[f64],
    tr: &TrajectoryRequest,
    req: &PortraitRequest,
) -> TrajectoryOut {
    let sol = if f.kind() == SystemKind::Map {
        let n = if tr.t1.is_finite() {
            tr.t1.round().clamp(0.0, MAX_ITERATES as f64)
        } else {
            0.0
        };
        dyn_solve::iterate_map(f, p, &tr.x0, n as usize, 0)
    } else {
        let s = req.solver;
        let opts = dyn_solve::SolverOptions {
            method: s.method,
            rtol: s.rtol,
            atol: s.atol,
            h: s.h,
            h_max: s.h_max,
            max_steps: s.max_steps,
        };
        let sampling = match req.samples {
            0 | 1 => Sampling::Steps,
            n => Sampling::Uniform(n.min(MAX_SAMPLES)),
        };
        dyn_solve::integrate(f, p, tr.t0, &tr.x0, tr.t1, &opts, sampling)
    };
    TrajectoryOut {
        t: sol.t,
        x: sol.x,
        status: sol.status,
        stats: sol.stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{Classification, InputFormat, Method, Severity, SolverOptions, Status};
    use serde::Deserialize;

    const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

    /// The fields of `templates.json` these tests need (extra fields ignored).
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Template {
        id: String,
        format: InputFormat,
        equations: Vec<String>,
        params: HashMap<String, f64>,
        view: TemplateView,
        initial_conditions: Vec<Vec<f64>>,
        t_end: f64,
        #[serde(default)]
        backward: Option<bool>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TemplateView {
        x: String,
        y: String,
        x_range: (f64, f64),
        y_range: (f64, f64),
    }

    fn templates() -> Vec<Template> {
        serde_json::from_str(TEMPLATES_JSON).expect("templates.json parses")
    }

    fn template(id: &str) -> Template {
        templates()
            .into_iter()
            .find(|t| t.id == id)
            .expect("template exists")
    }

    /// Same rule as `resolveAxis` in `app/src/state/model.ts`.
    fn resolve_axis(name: &str, states: &[SymbolInfo]) -> AxisRef {
        if name == "t" || name == "n" {
            return AxisRef::Time;
        }
        let index = states
            .iter()
            .position(|s| s.name == name || s.latex == name)
            .unwrap_or_else(|| panic!("axis {name:?} is not one of {states:?}"));
        AxisRef::State { index }
    }

    fn compile_ok(store: &SystemStore, t: &Template) -> CompileResponse {
        let source = t.equations.join("\n");
        let res = compile_system(
            store,
            &CompileRequest {
                source,
                format: t.format,
            },
        );
        let errors: Vec<_> = res
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(
            res.ok && errors.is_empty(),
            "template {} failed: {errors:?}",
            t.id
        );
        res
    }

    /// Builds the request the UI sends right after opening a template
    /// (mirrors `trajectorySpans` and `buildPortraitJob` in `model.ts`).
    fn template_request(t: &Template, res: &CompileResponse) -> PortraitRequest {
        let dim = res.states.len();
        let is_map = res.kind == Some(SystemKind::Map);
        let x_axis = resolve_axis(&t.view.x, &res.states);
        let y_axis = resolve_axis(&t.view.y, &res.states);
        let params = res
            .params
            .iter()
            .map(|p| t.params.get(&p.name).copied().unwrap_or(p.default))
            .collect();
        let window = match (x_axis, y_axis) {
            (AxisRef::Time, _) => Some(t.view.x_range),
            (_, AxisRef::Time) => Some(t.view.y_range),
            _ => None,
        };
        let mut trajectories = Vec::new();
        for ic in &t.initial_conditions {
            let x0: Vec<f64> = (0..dim)
                .map(|i| ic.get(i).copied().unwrap_or(0.0))
                .collect();
            if is_map {
                trajectories.push(TrajectoryRequest {
                    x0,
                    t0: 0.0,
                    t1: t.t_end.round().max(1.0),
                });
            } else if let Some((lo, hi)) = window {
                if hi > 0.0 {
                    trajectories.push(TrajectoryRequest {
                        x0: x0.clone(),
                        t0: 0.0,
                        t1: hi,
                    });
                }
                if lo < 0.0 {
                    trajectories.push(TrajectoryRequest {
                        x0,
                        t0: 0.0,
                        t1: lo,
                    });
                }
            } else {
                trajectories.push(TrajectoryRequest {
                    x0: x0.clone(),
                    t0: 0.0,
                    t1: t.t_end,
                });
                if t.backward != Some(false) {
                    trajectories.push(TrajectoryRequest {
                        x0,
                        t0: 0.0,
                        t1: -t.t_end,
                    });
                }
            }
        }
        let max_span = trajectories
            .iter()
            .map(|r| (r.t1 - r.t0).abs())
            .fold(0.0, f64::max);
        PortraitRequest {
            request_id: 7,
            system_id: res.system_id.unwrap(),
            params,
            x_axis,
            y_axis,
            x_range: t.view.x_range,
            y_range: t.view.y_range,
            base_state: vec![0.0; dim],
            field_time: 0.0,
            field_grid: if is_map { 0 } else { 21 },
            nullclines: !is_map,
            fixed_points: true,
            trajectories,
            solver: SolverOptions {
                method: Method::Dopri5,
                rtol: 1e-6,
                atol: 1e-9,
                h: 0.0,
                h_max: 0.0,
                max_steps: 200_000,
            },
            samples: (max_span * 120.0).clamp(400.0, 8000.0).round() as usize,
        }
    }

    fn all_finite(v: &[f64]) -> bool {
        v.iter().all(|x| x.is_finite())
    }

    fn assert_finite_output(id: &str, res: &PortraitResponse, dim: usize) {
        for a in &res.arrows {
            assert!(all_finite(&[a.x, a.y, a.u, a.v]), "{id}: arrow {a:?}");
        }
        for n in &res.nullclines {
            assert!(n.state < dim, "{id}: nullcline state {}", n.state);
            for line in &n.polylines {
                assert!(line.len() >= 2, "{id}: degenerate polyline");
                assert!(
                    line.iter().all(|(x, y)| x.is_finite() && y.is_finite()),
                    "{id}"
                );
            }
        }
        for fp in &res.fixed_points {
            assert_eq!(fp.x.len(), dim, "{id}");
            assert_eq!(fp.eigenvalues.len(), dim, "{id}");
            assert!(all_finite(&fp.x), "{id}: {fp:?}");
        }
        for tr in &res.trajectories {
            assert!(!tr.t.is_empty(), "{id}: empty trajectory");
            assert_eq!(tr.x.len(), tr.t.len() * dim, "{id}: x is not t.len() * dim");
            assert!(
                all_finite(&tr.t) && all_finite(&tr.x),
                "{id}: non-finite trajectory"
            );
        }
        // The JSON must not contain `null` where the UI expects numbers.
        let json = serde_json::to_string(res).unwrap();
        for bad in [":null", "[null", ",null"] {
            assert!(!json.contains(bad), "{id}: JSON has null");
        }
        assert!(res.elapsed_ms.is_finite() && res.elapsed_ms >= 0.0);
    }

    #[test]
    fn every_template_compiles_and_draws() {
        let store = SystemStore::new();
        let all = templates();
        assert_eq!(all.len(), 13, "the UI promises 13 templates");
        for t in &all {
            let res = compile_ok(&store, t);
            let dim = res.states.len();
            for name in t.params.keys() {
                assert!(
                    res.params.iter().any(|p| &p.name == name),
                    "{}: template parameter {name} is not in the system {:?}",
                    t.id,
                    res.params
                );
            }
            let req = template_request(t, &res);
            let system = store.get(req.system_id).unwrap();
            let out = compute_portrait(&system, &req).unwrap_or_else(|e| panic!("{}: {e}", t.id));
            assert_eq!(out.request_id, 7);
            eprintln!(
                "{:<24} {:>7.2} ms  {} arrows, {} fixed points, {} trajectories",
                t.id,
                out.elapsed_ms,
                out.arrows.len(),
                out.fixed_points.len(),
                out.trajectories.len()
            );
            assert_eq!(out.trajectories.len(), req.trajectories.len(), "{}", t.id);
            assert_finite_output(&t.id, &out, dim);

            let is_map = res.kind == Some(SystemKind::Map);
            if is_map {
                assert!(out.arrows.is_empty() && out.nullclines.is_empty());
                for tr in &out.trajectories {
                    // Iterates 0..=N, unless the orbit escaped.
                    if tr.status == Status::Completed {
                        assert_eq!(tr.t.len() as f64, t.t_end + 1.0, "{}", t.id);
                    }
                }
            } else {
                assert_eq!(out.arrows.len(), 21 * 21, "{}: arrows", t.id);
                assert!(!out.nullclines.is_empty(), "{}: nullclines", t.id);
                for (tr, rq) in out.trajectories.iter().zip(&req.trajectories) {
                    assert_eq!(tr.t[0], rq.t0, "{}", t.id);
                    if tr.status == Status::Completed {
                        assert!((tr.t.last().unwrap() - rq.t1).abs() < 1e-9, "{}", t.id);
                        assert_eq!(tr.t.len(), req.samples, "{}", t.id);
                    }
                }
            }
            if !res.autonomous {
                assert!(out.fixed_points.is_empty(), "{}: non-autonomous", t.id);
            }
        }
    }

    fn find_fp<'a>(out: &'a PortraitResponse, x: &[f64]) -> &'a FixedPointOut {
        out.fixed_points
            .iter()
            .find(|fp| fp.x.iter().zip(x).all(|(a, b)| (a - b).abs() < 1e-6))
            .unwrap_or_else(|| panic!("no fixed point at {x:?} in {:?}", out.fixed_points))
    }

    fn template_portrait(id: &str) -> (CompileResponse, PortraitResponse) {
        let store = SystemStore::new();
        let t = template(id);
        let res = compile_ok(&store, &t);
        let req = template_request(&t, &res);
        let out = compute_portrait(&store.get(req.system_id).unwrap(), &req).unwrap();
        (res, out)
    }

    #[test]
    fn rabbits_vs_sheep_has_the_four_strogatz_fixed_points() {
        // Strogatz §6.4: (0,0) unstable node, (0,2) and (3,0) stable nodes,
        // (1,1) saddle.
        let (_, out) = template_portrait("rabbits-vs-sheep");
        assert_eq!(out.fixed_points.len(), 4, "{:?}", out.fixed_points);
        let expect = [
            ([0.0, 0.0], Classification::UnstableNode, false),
            ([0.0, 2.0], Classification::StableNode, true),
            ([3.0, 0.0], Classification::StableNode, true),
            ([1.0, 1.0], Classification::Saddle, false),
        ];
        for (x, class, stable) in expect {
            let fp = find_fp(&out, &x);
            assert_eq!(fp.classification, class, "at {x:?}");
            assert_eq!(fp.stable, stable, "at {x:?}");
        }
        // Two nullclines (one per equation), each with curves.
        assert_eq!(out.nullclines.len(), 2);
        assert!(out.nullclines.iter().all(|n| !n.polylines.is_empty()));
    }

    #[test]
    fn pendulum_has_centres_and_saddles() {
        let (res, out) = template_portrait("pendulum");
        assert_eq!(res.states.len(), 2);
        let pi = std::f64::consts::PI;
        assert_eq!(
            find_fp(&out, &[0.0, 0.0]).classification,
            Classification::Centre
        );
        assert_eq!(
            find_fp(&out, &[pi, 0.0]).classification,
            Classification::Saddle
        );
        assert_eq!(
            find_fp(&out, &[-pi, 0.0]).classification,
            Classification::Saddle
        );
        assert_eq!(
            find_fp(&out, &[2.0 * pi, 0.0]).classification,
            Classification::Centre
        );
    }

    #[test]
    fn lorenz_finds_fixed_points_off_the_visible_plane() {
        // The x–z view hides y; C± = (±√(b(r-1)), ±√(b(r-1)), r-1) must
        // still be found through the hidden-variable search range.
        let (_, out) = template_portrait("lorenz");
        let c = (8.0 / 3.0 * 27.0_f64).sqrt();
        for x in [[0.0, 0.0, 0.0], [c, c, 27.0], [-c, -c, 27.0]] {
            let fp = find_fp(&out, &x);
            assert_eq!(fp.classification, Classification::SaddleND, "at {x:?}");
            assert!(!fp.stable);
        }
        assert_eq!(out.fixed_points.len(), 3);
        // The attractor stays bounded.
        assert!(out
            .trajectories
            .iter()
            .all(|t| t.status == Status::Completed));
    }

    #[test]
    fn logistic_map_iterates_from_x0_on_the_n_axis() {
        let (_, out) = template_portrait("logistic-map");
        let tr = &out.trajectories[0];
        assert_eq!(tr.t.len(), 61);
        assert_eq!(tr.t[0], 0.0);
        assert_eq!(tr.t[60], 60.0);
        assert_eq!(tr.x[0], 0.2);
        assert!((tr.x[1] - 3.7 * 0.2 * 0.8).abs() < 1e-15);
        // Fixed points 0 and 1 - 1/r, both unstable at r = 3.7.
        assert_eq!(out.fixed_points.len(), 2, "{:?}", out.fixed_points);
        find_fp(&out, &[0.0]);
        let fp = find_fp(&out, &[1.0 - 1.0 / 3.7]);
        assert!(!fp.stable);
    }

    #[test]
    fn non_autonomous_slope_field_has_no_fixed_points_and_unit_time_component() {
        let (res, out) = template_portrait("slope-field-x2-minus-t");
        assert!(!res.autonomous);
        assert!(out.fixed_points.is_empty());
        assert!(out.arrows.iter().all(|a| a.u == 1.0));
        // Some solutions of x' = x² - t blow up in finite time.
        assert!(out.trajectories.iter().any(|t| t.status == Status::BlewUp));
    }

    fn simple_request(system_id: u32, dim: usize) -> PortraitRequest {
        PortraitRequest {
            request_id: 1,
            system_id,
            params: vec![],
            x_axis: AxisRef::State { index: 0 },
            y_axis: AxisRef::State {
                index: 1.min(dim - 1),
            },
            x_range: (-1.0, 1.0),
            y_range: (-1.0, 1.0),
            base_state: vec![0.0; dim],
            field_time: 0.0,
            field_grid: 5,
            nullclines: false,
            fixed_points: false,
            trajectories: vec![],
            solver: SolverOptions {
                method: Method::Rk4,
                rtol: 1e-6,
                atol: 1e-9,
                h: 0.01,
                h_max: 0.0,
                max_steps: 1000,
            },
            samples: 11,
        }
    }

    fn compile_src(store: &SystemStore, src: &str) -> u32 {
        let res = compile_system(
            store,
            &CompileRequest {
                source: src.into(),
                format: InputFormat::Latex,
            },
        );
        assert!(res.ok, "{:?}", res.diagnostics);
        res.system_id.unwrap()
    }

    #[test]
    fn layers_are_only_computed_when_requested() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=y\n\\dot{y}=-x");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 2);
        req.field_grid = 0;
        let out = compute_portrait(&sys, &req).unwrap();
        assert!(out.arrows.is_empty() && out.nullclines.is_empty() && out.fixed_points.is_empty());
        req.field_grid = 5;
        req.nullclines = true;
        req.fixed_points = true;
        let out = compute_portrait(&sys, &req).unwrap();
        assert_eq!(out.arrows.len(), 25);
        assert_eq!(out.nullclines.len(), 2);
        assert_eq!(out.fixed_points.len(), 1);
        assert_eq!(out.fixed_points[0].classification, Classification::Centre);
    }

    #[test]
    fn trajectories_go_forward_and_backward_with_uniform_samples() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=-x");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 1);
        req.x_axis = AxisRef::Time;
        req.trajectories = vec![
            TrajectoryRequest {
                x0: vec![1.0],
                t0: 0.0,
                t1: 1.0,
            },
            TrajectoryRequest {
                x0: vec![1.0],
                t0: 0.0,
                t1: -1.0,
            },
        ];
        let out = compute_portrait(&sys, &req).unwrap();
        let (fwd, bwd) = (&out.trajectories[0], &out.trajectories[1]);
        assert_eq!(fwd.t.len(), 11);
        assert_eq!(bwd.t.len(), 11);
        assert!((fwd.t[5] - 0.5).abs() < 1e-12 && (bwd.t[5] + 0.5).abs() < 1e-12);
        assert!((fwd.x[10] - (-1.0_f64).exp()).abs() < 1e-8);
        assert!((bwd.x[10] - 1.0_f64.exp()).abs() < 1e-7);
        // With samples = 0 the raw steps come back: RK4 with h = 0.01.
        req.samples = 0;
        let out = compute_portrait(&sys, &req).unwrap();
        assert_eq!(out.trajectories[0].t.len(), 101);
    }

    #[test]
    fn hidden_variables_are_searched_around_the_slice() {
        let mut req = simple_request(1, 3);
        req.x_axis = AxisRef::State { index: 0 };
        req.y_axis = AxisRef::State { index: 2 };
        req.x_range = (-25.0, 25.0);
        req.y_range = (-2.0, 54.0);
        req.base_state = vec![0.0, 5.0, 0.0];
        let b = fixed_point_search_box(&req, 3);
        assert_eq!(
            b,
            vec![(-25.0, 25.0), (5.0 - 28.0, 5.0 + 28.0), (-2.0, 54.0)]
        );
        // A time axis does not set the scale for hidden variables.
        req.x_axis = AxisRef::Time;
        req.x_range = (0.0, 1000.0);
        let b = fixed_point_search_box(&req, 3);
        assert_eq!(b[0], (-28.0, 28.0));
    }

    #[test]
    fn bad_requests_get_readable_errors() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=ax\n\\dot{y}=-y");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 2);
        let err = compute_portrait(&sys, &req).unwrap_err();
        assert!(err.contains("1 parameter"), "{err}");
        req.params = vec![1.0];
        req.y_axis = AxisRef::State { index: 2 };
        assert!(compute_portrait(&sys, &req)
            .unwrap_err()
            .contains("vertical axis"));
        req.y_axis = AxisRef::State { index: 1 };
        req.x_range = (1.0, 1.0);
        assert!(compute_portrait(&sys, &req)
            .unwrap_err()
            .contains("horizontal range"));
        req.x_range = (-1.0, 1.0);
        req.trajectories = vec![TrajectoryRequest {
            x0: vec![1.0],
            t0: 0.0,
            t1: 1.0,
        }];
        assert!(compute_portrait(&sys, &req)
            .unwrap_err()
            .contains("Starting point 1"));
        req.trajectories.clear();
        assert!(compute_portrait(&sys, &req).is_ok());
    }

    #[test]
    fn compile_errors_come_back_with_spans_and_no_id() {
        let store = SystemStore::new();
        let res = compile_system(
            &store,
            &CompileRequest {
                source: "\\dot{x}=y+\n\\dot{y}=-x".into(),
                format: InputFormat::Latex,
            },
        );
        assert!(!res.ok);
        assert!(res.system_id.is_none() && res.kind.is_none());
        assert!(res.states.is_empty());
        assert!(res
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.span.is_some()));
        assert!(store.is_empty());
        let json = serde_json::to_value(&res).unwrap();
        assert!(json.get("systemId").is_none());
        assert_eq!(json["ok"], false);
    }

    #[test]
    fn store_ids_increase_and_old_systems_are_evicted() {
        let store = SystemStore::new();
        let ids: Vec<u32> = (0..MAX_STORED_SYSTEMS + 5)
            .map(|_| compile_src(&store, "\\dot{x}=-x"))
            .collect();
        assert!(ids.windows(2).all(|w| w[1] == w[0] + 1));
        assert_eq!(ids[0], 1);
        assert_eq!(store.len(), MAX_STORED_SYSTEMS);
        assert!(store.get(ids[4]).is_none());
        assert!(store.get(ids[5]).is_some());
        assert!(store.get(*ids.last().unwrap()).is_some());
    }

    #[test]
    fn json_matches_the_typescript_contract() {
        let req: PortraitRequest = serde_json::from_value(serde_json::json!({
            "requestId": 3, "systemId": 1, "params": [],
            "xAxis": { "kind": "time" }, "yAxis": { "kind": "state", "index": 0 },
            "xRange": [0, 10], "yRange": [-3, 3], "baseState": [0], "fieldTime": 0,
            "fieldGrid": 21, "nullclines": true, "fixedPoints": true,
            "trajectories": [{ "x0": [1], "t0": 0, "t1": 10 }],
            "solver": { "method": "dopri5", "rtol": 1e-6, "atol": 1e-9, "h": 0, "maxSteps": 200000 },
            "samples": 400
        }))
        .expect("the request the UI sends deserializes");
        assert_eq!(req.x_axis, AxisRef::Time);
        assert_eq!(req.y_axis, AxisRef::State { index: 0 });
        assert_eq!(req.solver.h_max, 0.0);

        let store = SystemStore::new();
        let res = compile_system(
            &store,
            &CompileRequest {
                source: "\\dot{x}=-x".into(),
                format: InputFormat::Latex,
            },
        );
        let json = serde_json::to_value(&res).unwrap();
        for key in [
            "ok",
            "systemId",
            "kind",
            "hamiltonian",
            "autonomous",
            "states",
            "params",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert!(json.get("equationsLatex").is_some() && json.get("diagnostics").is_some());
        assert_eq!(json["kind"], "ode");

        let out = compute_portrait(&store.get(1).unwrap(), &req).unwrap();
        let json = serde_json::to_value(&out).unwrap();
        for key in [
            "requestId",
            "arrows",
            "nullclines",
            "fixedPoints",
            "trajectories",
            "elapsedMs",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["requestId"], 3);
        // 1D flows use sink/source (Strogatz ch. 2 "stable"/"unstable").
        assert_eq!(json["fixedPoints"][0]["classification"], "sink");
        assert!(json["fixedPoints"][0]["eigenvalues"][0].is_array());
        assert_eq!(json["trajectories"][0]["status"], "completed");
        assert!(json["trajectories"][0]["stats"]["evals"].is_u64());
        assert!(json["nullclines"][0]["polylines"][0][0].is_array());
    }
}
