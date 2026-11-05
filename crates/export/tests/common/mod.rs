//! Portraits for the export tests, computed the way the app's
//! `compute_portrait` does (same analyses, solver and sampling), from the
//! gallery templates.
#![allow(dead_code)]

use std::collections::HashMap;

use dyn_analysis::{Axis as PlaneAxis, Plane};
use dyn_core::{SystemKind, VectorField};
use dyn_expr::{compile, CompiledSystem, InputFormat};
use dyn_solve::{Method, Sampling, SolverOptions};
use separatrix_export::{
    Arrow, Axis, FixedPoint, Labels, Nullcline, Palette, Portrait, Theme, Trajectory, TrajectoryTag,
};
use serde::Deserialize;

pub const TEMPLATES_JSON: &str = include_str!("../../../../app/src/templates/templates.json");
pub const TOKENS_JSON: &str = include_str!("../../../../app/src/styles/ds/tokens.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub section: Option<String>,
    pub format: InputFormat,
    pub equations: Vec<String>,
    pub params: HashMap<String, f64>,
    pub view: TemplateView,
    pub initial_conditions: Vec<Vec<f64>>,
    pub t_end: f64,
    #[serde(default)]
    pub backward: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateView {
    pub x: String,
    pub y: String,
    pub x_range: (f64, f64),
    pub y_range: (f64, f64),
}

pub fn templates() -> Vec<Template> {
    serde_json::from_str(TEMPLATES_JSON).expect("templates.json parses")
}

pub fn template(id: &str) -> Template {
    templates()
        .into_iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("no template {id}"))
}

pub fn palette(theme: Theme) -> Palette {
    Palette::from_tokens(TOKENS_JSON, theme).expect("tokens.json has the plot colours")
}

/// `equalizeRanges` (app/src/state/model.ts).
pub fn equalize(x: (f64, f64), y: (f64, f64), aspect: f64) -> ((f64, f64), (f64, f64)) {
    let sx = x.1 - x.0;
    let sy = y.1 - y.0;
    let xm = sy * aspect;
    if (xm - sx).abs() <= 1e-9 * sx.max(xm) {
        return (x, y);
    }
    if xm > sx {
        let c = (x.0 + x.1) / 2.0;
        return ((c - xm / 2.0, c + xm / 2.0), y);
    }
    let c = (y.0 + y.1) / 2.0;
    let span = sx / aspect;
    (x, (c - span / 2.0, c + span / 2.0))
}

/// A computed template: the portrait (at the figure's aspect), the names,
/// the lines and the compiled system.
pub struct Case {
    pub template: Template,
    pub system: CompiledSystem,
    pub portrait: Portrait,
    pub labels: Labels,
    pub params: Vec<f64>,
    pub search: Vec<(f64, f64)>,
}

pub struct Options {
    pub width_cm: f64,
    pub height_cm: f64,
    pub field_grid: usize,
    pub samples: usize,
    pub t_end: Option<f64>,
    pub backward: bool,
    /// The view's ranges as they are, instead of the template's (equalised).
    pub view: Option<((f64, f64), (f64, f64))>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            width_cm: 8.0,
            height_cm: 6.0,
            field_grid: 21,
            samples: 2000,
            t_end: None,
            backward: true,
            view: None,
        }
    }
}

fn resolve_axis(name: &str, states: &[String], kind: SystemKind) -> Axis {
    if name == "t" || name == "n" {
        return Axis::Time;
    }
    let index = states
        .iter()
        .position(|s| s == name)
        .unwrap_or_else(|| panic!("{name} is not a state of {states:?} ({kind:?})"));
    Axis::State { index }
}

pub fn case(id: &str, o: &Options) -> Case {
    case_of(template(id), o)
}

/// A computed case from a template built in the test.
pub fn case_of(t: Template, o: &Options) -> Case {
    let out = compile(&t.equations.join("\n"), t.format);
    let system = out
        .system
        .unwrap_or_else(|| panic!("{}: {:?}", t.id, out.diagnostics));
    let info = &system.info;
    let names: Vec<String> = info.states.iter().map(|s| s.name.clone()).collect();
    let kind = info.kind;
    let params: Vec<f64> = info
        .params
        .iter()
        .map(|p| t.params.get(&p.name).copied().unwrap_or(p.default))
        .collect();
    let x_axis = resolve_axis(&t.view.x, &names, kind);
    let y_axis = resolve_axis(&t.view.y, &names, kind);
    let aspect = o.width_cm / o.height_cm;
    let phase_plane = kind == SystemKind::Ode
        && matches!(x_axis, Axis::State { .. })
        && matches!(y_axis, Axis::State { .. });
    let (x_range, y_range) = if let Some(v) = o.view {
        v
    } else if phase_plane {
        equalize(t.view.x_range, t.view.y_range, aspect)
    } else {
        (t.view.x_range, t.view.y_range)
    };
    let dim = system.dim();
    let to_plane = |a: Axis| match a {
        Axis::Time => PlaneAxis::Time,
        Axis::State { index } => PlaneAxis::State(index),
    };
    let plane = Plane {
        x: to_plane(x_axis),
        y: to_plane(y_axis),
        x_range,
        y_range,
        base_state: vec![0.0; dim],
        t: 0.0,
    };
    let f: &dyn VectorField = &system;
    let is_map = kind == SystemKind::Map;
    let arrows = if is_map || o.field_grid == 0 {
        Vec::new()
    } else {
        dyn_analysis::slope_field(f, &params, &plane, o.field_grid)
            .into_iter()
            .map(|a| Arrow {
                x: a.x,
                y: a.y,
                u: a.u,
                v: a.v,
            })
            .collect()
    };
    let nullclines = if is_map {
        Vec::new()
    } else {
        dyn_analysis::nullclines(f, &params, &plane, 160)
            .into_iter()
            .map(|n| Nullcline {
                state: n.state,
                polylines: n.polylines,
            })
            .collect()
    };
    let span = [(x_axis, x_range), (y_axis, y_range)]
        .iter()
        .filter(|(a, _)| matches!(a, Axis::State { .. }))
        .map(|(_, (lo, hi))| (hi - lo).abs() / 2.0)
        .fold(0.0_f64, f64::max);
    let span = if span > 0.0 { span } else { 10.0 };
    let search: Vec<(f64, f64)> = (0..dim)
        .map(|i| {
            [(x_axis, x_range), (y_axis, y_range)]
                .iter()
                .find(|(a, _)| *a == Axis::State { index: i })
                .map(|(_, r)| *r)
                .unwrap_or((-span, span))
        })
        .collect();
    let seeds = match dim {
        1 => 40,
        2 => 14,
        _ => 9,
    };
    let fixed_points = if f.is_autonomous() {
        dyn_analysis::fixed_points_with_budget(f, &params, &search, seeds, 1000)
            .into_iter()
            .filter(|fp| fp.x.iter().all(|v| v.is_finite()))
            .map(|fp| FixedPoint {
                x: fp.x,
                eigenvalues: fp.eigenvalues,
                classification: fp.classification,
                stable: fp.stable,
            })
            .collect()
    } else {
        Vec::new()
    };
    let opts = SolverOptions {
        method: Method::Dopri5,
        rtol: 1e-6,
        atol: 1e-9,
        h: 0.0,
        h_max: 0.0,
        max_steps: 200_000,
    };
    let t_end = o.t_end.unwrap_or(t.t_end);
    let backward = o.backward && t.backward.unwrap_or(true) && !is_map;
    let time_axis = x_axis == Axis::Time || y_axis == Axis::Time;
    let mut trajectories = Vec::new();
    for (k, x0) in t.initial_conditions.iter().enumerate() {
        let tag = |backward: bool| TrajectoryTag {
            ic_id: format!("ic-{}", k + 1),
            backward,
            color: (k % 7) as u8,
        };
        if is_map {
            let n = t_end.round() as usize;
            let sol = dyn_solve::iterate_map(f, &params, x0, n, 0);
            trajectories.push(Trajectory {
                tag: tag(false),
                x0: x0.clone(),
                t0: 0.0,
                t: sol.t,
                x: sol.x,
                status: sol.status,
            });
            continue;
        }
        let (t0, t1, tb) = if time_axis {
            // visibleWindowSpans: the visible time window from t0 = 0.
            let tr = if x_axis == Axis::Time {
                x_range
            } else {
                y_range
            };
            (0.0, tr.1.max(0.0), tr.0.min(0.0))
        } else {
            (0.0, t_end, -t_end)
        };
        let mut runs = vec![(false, t1)];
        if backward && tb < t0 {
            runs.push((true, tb));
        }
        for (back, end) in runs {
            let sol =
                dyn_solve::integrate(f, &params, t0, x0, end, &opts, Sampling::Uniform(o.samples));
            trajectories.push(Trajectory {
                tag: tag(back),
                x0: x0.clone(),
                t0,
                t: sol.t,
                x: sol.x,
                status: sol.status,
            });
        }
    }
    let portrait = Portrait {
        kind,
        dim,
        x_axis,
        y_axis,
        x_range,
        y_range,
        field_grid: if is_map { 0 } else { o.field_grid },
        arrows,
        nullclines,
        fixed_points,
        trajectories,
    };
    let labels = Labels::from_info(info, &params);
    Case {
        template: t,
        system,
        portrait,
        labels,
        params,
        search,
    }
}
