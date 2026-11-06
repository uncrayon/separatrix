use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use dyn_analysis::{Axis, Plane};
use dyn_core::{SystemKind, VectorField};
use dyn_expr::CompiledSystem;
use dyn_solve::Sampling;
use separatrix_project::ORBIT_TRANSIENT_MAX;

use crate::ipc::{
    Arrow, AxisRef, BranchEndOut, BranchOut, BranchStop, CompileRequest, CompileResponse,
    ContinuationDirection, ContinuationRequest, ContinuationResponse, ContinuationStart,
    ConvertRequest, ConvertResponse, ConvertedLineOut, Diagnostic, DoublingOut, EnergyOut,
    FixedPointOut, LyapunovRequest, LyapunovResponse, Method, NullclineOut, OrbitDiagramRequest,
    OrbitDiagramResponse, ParamInfo, PortraitRequest, PortraitResponse, SectionSpec, SolverOptions,
    SpecialKindOut, SpecialPointOut, StateInfo, TrajectoryOut, TrajectoryRequest,
    MAX_CONTINUATION_POINTS, MAX_ORBIT_COLUMNS, MAX_ORBIT_POINTS,
};

pub const RETAINED_SYSTEMS_FOR_IN_FLIGHT_REQUESTS: usize = 32;

pub const NULLCLINE_CELLS_PER_SIDE: usize = 160;

pub const MAX_FIELD_GRID: usize = 200;
pub const MAX_SAMPLES: usize = 200_000;
pub const MAX_ITERATES: usize = 1_000_000;
pub const MAX_SECTION_POINTS: usize = 100_000;
/// Most QR steps one `compute_lyapunov` request may ask for (`total / interval`).
pub const MAX_RENORMALIZATIONS: usize = 10_000_000;
/// Most iterates (`transient + total`) one `compute_lyapunov` request on a
/// map may ask for; a flow is bounded by `solver.max_steps` instead.
pub const MAX_LYAPUNOV_ITERATES: usize = 100_000_000;
/// Running-estimate samples returned by `compute_lyapunov`.
pub const LYAPUNOV_HISTORY: usize = 2000;
/// Smallest continuation step: a step halved below it ends the branch.
pub const CONTINUATION_DS_MIN: f64 = 1e-8;
/// Largest angle in radians between successive tangents of a branch.
pub const CONTINUATION_MAX_ANGLE: f64 = 0.3;

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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_evicting_oldest(&self, system: CompiledSystem) -> u32 {
        let mut inner = self.lock_ignoring_poison();
        inner.last_id += 1;
        let id = inner.last_id;
        inner.systems.insert(id, Arc::new(system));
        let oldest_kept = id.saturating_sub(RETAINED_SYSTEMS_FOR_IN_FLIGHT_REQUESTS as u32 - 1);
        inner.systems.retain(|&k, _| k >= oldest_kept);
        id
    }

    pub fn get(&self, id: u32) -> Option<Arc<CompiledSystem>> {
        self.lock_ignoring_poison().systems.get(&id).cloned()
    }

    pub fn len(&self) -> usize {
        self.lock_ignoring_poison().systems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock_ignoring_poison(&self) -> std::sync::MutexGuard<'_, StoreInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

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
            let separable = system.is_separable();
            let id = store.insert_evicting_oldest(system);
            CompileResponse {
                ok: true,
                system_id: Some(id),
                kind: Some(info.kind),
                hamiltonian: info.hamiltonian,
                separable,
                autonomous: info.autonomous,
                states: info
                    .states
                    .into_iter()
                    .map(|s| StateInfo {
                        name: s.name,
                        latex: s.latex,
                        derived: s.derived,
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
            separable: false,
            autonomous: true,
            states: Vec::new(),
            params: Vec::new(),
            equations_latex: Vec::new(),
            diagnostics,
        },
    }
}

/// Rewrites the UI's lines in the other input format (see
/// [`dyn_expr::convert`]): lines that can't be converted exactly come back
/// as typed, with a note.
pub fn convert_source(req: &ConvertRequest) -> ConvertResponse {
    let lines = dyn_expr::convert(&req.lines, req.from, req.to)
        .into_iter()
        .map(|l| ConvertedLineOut {
            source: l.source,
            converted: l.converted,
            error: l.error,
            note: l.note,
        })
        .collect();
    ConvertResponse { lines }
}

pub fn compute_portrait(
    system: &CompiledSystem,
    req: &PortraitRequest,
) -> Result<PortraitResponse, String> {
    let started = Instant::now();
    validate(system, req)?;
    method_fits(system, req.solver.method)?;
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
        dyn_analysis::nullclines(f, p, &plane, NULLCLINE_CELLS_PER_SIDE)
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
        dyn_analysis::fixed_points_with_budget(f, p, &search, seeds_per_axis(dim), MAX_TOTAL_SEEDS)
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
                eigenvectors: fp.eigenvectors,
                eigenvectors_im: fp.eigenvectors_im,
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
        .map(|tr| trajectory(system, p, tr, req))
        .collect();

    let graph = req
        .graph_samples
        .map(|samples| dyn_analysis::map_graph(f, p, req.x_range, req.y_range, samples));

    Ok(PortraitResponse {
        request_id: req.request_id,
        arrows,
        nullclines,
        fixed_points,
        trajectories,
        graph,
        elapsed_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

/// The Lyapunov spectrum of one orbit (see [`dyn_analysis::lyapunov_spectrum`]).
pub fn compute_lyapunov(
    system: &CompiledSystem,
    req: &LyapunovRequest,
) -> Result<LyapunovResponse, String> {
    let started = Instant::now();
    let f: &dyn VectorField = system;
    validate_lyapunov(f, req)?;
    if f.kind() == SystemKind::Ode && req.solver.method.is_symplectic() {
        return Err(format!(
            "A Lyapunov run integrates the variational equations, which {} cannot step; \
             choose Runge–Kutta 4 or Dormand–Prince 5(4).",
            method_name(req.solver.method)
        ));
    }
    if f.kind() == SystemKind::Ode && req.solver.method == Method::Rosenbrock23 {
        return Err(
            "A Lyapunov run integrates the variational equations, which have no exact \
             Jacobian for Rosenbrock 2(3) to use; choose Runge–Kutta 4 or Dormand–Prince 5(4)."
                .into(),
        );
    }
    let opts = dyn_analysis::LyapunovOptions {
        transient: req.transient,
        total: req.total,
        interval: req.interval,
        blocks: req.blocks,
        history: LYAPUNOV_HISTORY,
    };
    let s = dyn_analysis::lyapunov_spectrum(
        f,
        &req.params,
        req.t0,
        &req.x0,
        &to_solver(&req.solver),
        &opts,
    );
    Ok(LyapunovResponse {
        request_id: req.request_id,
        exponents: s.exponents,
        uncertainty: s.uncertainty,
        sum: s.sum,
        trace_average: s.trace_average,
        history_t: s.history_t,
        history: s.history,
        renormalizations: s.renormalizations,
        status: s.status,
        stats: s.stats,
        elapsed_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

/// The orbit diagram of a map over one parameter's range (see
/// [`dyn_analysis::orbit_diagram`]), with its period-doubling points when
/// asked (see [`dyn_analysis::period_doublings`]).
pub fn compute_orbit_diagram(
    system: &CompiledSystem,
    req: &OrbitDiagramRequest,
) -> Result<OrbitDiagramResponse, String> {
    let started = Instant::now();
    let f: &dyn VectorField = system;
    validate_orbit_diagram(f, req)?;
    let opts = dyn_analysis::OrbitDiagramOptions {
        param: req.param,
        range: req.range,
        columns: req.columns,
        state: req.state,
        transient: req.transient,
        kept: req.kept,
    };
    let diagram = dyn_analysis::orbit_diagram(f, &req.params, &req.x0, &opts);
    let doublings = req.doublings.then(|| {
        dyn_analysis::period_doublings(f, &req.params, req.param, &diagram, usize::MAX)
            .into_iter()
            .map(|d| DoublingOut {
                r: d.r,
                period: d.period,
            })
            .collect()
    });
    Ok(OrbitDiagramResponse {
        request_id: req.request_id,
        r: diagram.r,
        x: diagram.x,
        doublings,
        elapsed_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

fn validate_orbit_diagram(f: &dyn VectorField, req: &OrbitDiagramRequest) -> Result<(), String> {
    if f.kind() != SystemKind::Map {
        return Err("Orbit diagrams are for maps; this system is a flow.".into());
    }
    if req.params.len() != f.n_params() {
        return Err(format!(
            "Expected {} parameter value(s) but got {}.",
            f.n_params(),
            req.params.len()
        ));
    }
    if req.param >= f.n_params() {
        return Err(if f.n_params() == 0 {
            "This map has no parameter to sweep.".into()
        } else {
            format!(
                "Parameter {} doesn't exist; the map has {}.",
                req.param,
                f.n_params()
            )
        });
    }
    let (lo, hi) = req.range;
    if !(lo.is_finite() && hi.is_finite() && lo < hi) {
        return Err("The parameter range must be finite, with its start below its end.".into());
    }
    if req.state >= f.dim() {
        return Err(format!(
            "State {} doesn't exist; the map has {}.",
            req.state,
            f.dim()
        ));
    }
    if req.x0.len() != f.dim() {
        return Err(format!(
            "The starting point has {} coordinate(s) but the map has {}.",
            req.x0.len(),
            f.dim()
        ));
    }
    if !req.x0.iter().all(|v| v.is_finite()) {
        return Err("The starting point must be finite.".into());
    }
    if !(2..=MAX_ORBIT_COLUMNS).contains(&req.columns) {
        return Err(format!(
            "The diagram needs 2 to {MAX_ORBIT_COLUMNS} parameter values."
        ));
    }
    if req.transient > ORBIT_TRANSIENT_MAX as usize {
        return Err(format!(
            "The transient can be at most {ORBIT_TRANSIENT_MAX} iterates."
        ));
    }
    if req.kept == 0 {
        return Err("Keep at least one iterate per parameter value.".into());
    }
    if req.columns.saturating_mul(req.kept) > MAX_ORBIT_POINTS {
        return Err(format!(
            "That is {} points; at most {MAX_ORBIT_POINTS} fit in one diagram. \
             Keep fewer iterates or use fewer parameter values.",
            req.columns.saturating_mul(req.kept)
        ));
    }
    if req.doublings && f.dim() != 1 {
        return Err("Period doublings are located for one-dimensional maps only.".into());
    }
    if req.doublings && !f.is_autonomous() {
        return Err("Period doublings need a map that doesn't depend on n.".into());
    }
    Ok(())
}

/// A branch of equilibria or of fixed points of `f^period` as one
/// parameter varies (see [`dyn_analysis::continue_branch`]).
pub fn compute_continuation(
    system: &CompiledSystem,
    req: &ContinuationRequest,
) -> Result<ContinuationResponse, String> {
    let started = Instant::now();
    let f: &dyn VectorField = system;
    validate_continuation(f, req)?;
    let follow = |sign: f64| {
        let opts = |period| dyn_analysis::ContinuationOptions {
            param: req.param,
            period,
            range: req.range,
            ds: sign * req.ds,
            ds_min: CONTINUATION_DS_MIN,
            ds_max: req.ds_max,
            max_points: req.max_points,
            max_angle: CONTINUATION_MAX_ANGLE,
        };
        match &req.start {
            ContinuationStart::Point { x0, period } => {
                dyn_analysis::continue_branch(f, &req.params, x0, &opts(*period))
            }
            ContinuationStart::Switch { at } => {
                let at = dyn_analysis::SpecialPoint {
                    kind: kind_in(at.kind),
                    index: 0,
                    p: at.p,
                    x: at.x.clone(),
                    period: at.period,
                    tangent: at.tangent.clone(),
                    eigenvalues: at.eigenvalues.clone(),
                    omega: at.omega,
                    lyapunov: at.lyapunov,
                };
                dyn_analysis::switch_branch(f, &req.params, &at, &opts(at.period))
            }
        }
        .map_err(|e| continuation_refusal(f, e))
    };
    let branch = match req.direction {
        ContinuationDirection::Forward => join(None, follow(1.0)?),
        ContinuationDirection::Backward => join(None, follow(-1.0)?),
        ContinuationDirection::Both => join(Some(follow(-1.0)?), follow(1.0)?),
    };
    Ok(ContinuationResponse {
        request_id: req.request_id,
        branch,
        elapsed_ms: started.elapsed().as_secs_f64() * 1e3,
    })
}

fn continuation_refusal(f: &dyn VectorField, e: dyn_analysis::ContinuationError) -> String {
    let point = if f.kind() == SystemKind::Map {
        "fixed point"
    } else {
        "equilibrium"
    };
    match e {
        dyn_analysis::ContinuationError::NotAFixedPoint => format!(
            "Newton's method found no {point} near the starting point; start closer to one."
        ),
        dyn_analysis::ContinuationError::Singular => format!(
            "The starting {point} is a fold or branch point, so the branch has no unique \
             direction; start a little away from it."
        ),
        dyn_analysis::ContinuationError::NotAutonomous => NOT_AUTONOMOUS.into(),
        dyn_analysis::ContinuationError::CannotSwitch => {
            "Branches can be switched only at a branch point (BP), or at a flip (PD) of a map."
                .into()
        }
        dyn_analysis::ContinuationError::NoSecondBranch => {
            "No second branch was found near this point; try a smaller ds.".into()
        }
    }
}

const SPECIAL_KINDS: [(dyn_analysis::SpecialKind, SpecialKindOut); 5] = [
    (dyn_analysis::SpecialKind::Fold, SpecialKindOut::Fold),
    (dyn_analysis::SpecialKind::Hopf, SpecialKindOut::Hopf),
    (
        dyn_analysis::SpecialKind::BranchPoint,
        SpecialKindOut::BranchPoint,
    ),
    (dyn_analysis::SpecialKind::Flip, SpecialKindOut::Flip),
    (
        dyn_analysis::SpecialKind::NeimarkSacker,
        SpecialKindOut::NeimarkSacker,
    ),
];

fn kind_out(kind: dyn_analysis::SpecialKind) -> SpecialKindOut {
    SPECIAL_KINDS
        .iter()
        .find(|k| k.0 == kind)
        .expect("every kind")
        .1
}

fn kind_in(kind: SpecialKindOut) -> dyn_analysis::SpecialKind {
    SPECIAL_KINDS
        .iter()
        .find(|k| k.1 == kind)
        .expect("every kind")
        .0
}

const NOT_AUTONOMOUS: &str =
    "Continuation follows fixed points, which a system that depends on t or n doesn't have.";

fn join(backward: Option<dyn_analysis::Branch>, forward: dyn_analysis::Branch) -> BranchOut {
    let stop = |end| match end {
        dyn_analysis::BranchEnd::Range => BranchStop::Range,
        dyn_analysis::BranchEnd::MaxPoints => BranchStop::MaxPoints,
        dyn_analysis::BranchEnd::StepTooSmall => BranchStop::StepTooSmall,
    };
    let special_out = |sp: dyn_analysis::SpecialPoint, index: usize, sign: f64| SpecialPointOut {
        kind: kind_out(sp.kind),
        index,
        p: sp.p,
        x: sp.x,
        period: sp.period,
        tangent: sp.tangent.iter().map(|t| sign * t).collect(),
        eigenvalues: sp.eigenvalues,
        omega: sp.omega,
        lyapunov: sp.lyapunov,
    };
    let mut points = Vec::new();
    let mut special = Vec::new();
    let mut first = None;
    if let Some(b) = backward {
        let last = b.points.len() - 1;
        first = Some(stop(b.end));
        points.extend(b.points.into_iter().skip(1).rev());
        special.extend(
            b.special
                .into_iter()
                .rev()
                .map(|sp| (last - sp.index, sp))
                .map(|(index, sp)| special_out(sp, index, -1.0)),
        );
    }
    let offset = points.len();
    points.extend(forward.points);
    special.extend(
        forward
            .special
            .into_iter()
            .map(|sp| (offset + sp.index, sp))
            .map(|(index, sp)| special_out(sp, index, 1.0)),
    );
    BranchOut {
        period: forward.period,
        p: points.iter().map(|pt| pt.p).collect(),
        x: points
            .iter()
            .flat_map(|pt| pt.cycle.iter().copied())
            .collect(),
        stable: points.iter().map(|pt| pt.stable).collect(),
        unstable: points.iter().map(|pt| pt.unstable as u32).collect(),
        special,
        end: BranchEndOut {
            first,
            last: stop(forward.end),
        },
    }
}

fn validate_continuation(f: &dyn VectorField, req: &ContinuationRequest) -> Result<(), String> {
    if !f.is_autonomous() {
        return Err(NOT_AUTONOMOUS.into());
    }
    if req.params.len() != f.n_params() {
        return Err(format!(
            "Expected {} parameter value(s) but got {}.",
            f.n_params(),
            req.params.len()
        ));
    }
    if req.param >= f.n_params() {
        return Err(if f.n_params() == 0 {
            "This system has no parameter to continue in.".into()
        } else {
            format!(
                "Parameter {} doesn't exist; the system has {}.",
                req.param,
                f.n_params()
            )
        });
    }
    let (lo, hi) = req.range;
    if !(lo.is_finite() && hi.is_finite() && lo < hi) {
        return Err("The parameter range must be finite, with its start below its end.".into());
    }
    let (x0, period) = match &req.start {
        ContinuationStart::Point { x0, period } => (x0, period),
        ContinuationStart::Switch { at } => {
            let tangent = &at.tangent;
            if tangent.len() != f.dim() + 1
                || !tangent.iter().all(|v| v.is_finite())
                || tangent.iter().all(|v| *v == 0.0)
            {
                return Err(format!(
                    "The switch point's tangent needs {} finite components, not all zero.",
                    f.dim() + 1
                ));
            }
            if !at.p.is_finite() {
                return Err("The switch point's parameter must be finite.".into());
            }
            (&at.x, &at.period)
        }
    };
    if x0.len() != f.dim() {
        return Err(format!(
            "The starting point has {} coordinate(s) but the system has {}.",
            x0.len(),
            f.dim()
        ));
    }
    if !x0.iter().chain(&req.params).all(|v| v.is_finite()) {
        return Err("The starting point and the parameters must be finite.".into());
    }
    if *period == 0 {
        return Err("The period must be at least 1.".into());
    }
    if *period > 1 && f.kind() == SystemKind::Ode {
        return Err("A flow's equilibria have no period; use period 1.".into());
    }
    if !(req.ds > 0.0 && req.ds.is_finite()) {
        return Err("The step ds must be a positive number.".into());
    }
    if !(req.ds_max >= req.ds && req.ds_max.is_finite()) {
        return Err("The largest step must be a number at least ds.".into());
    }
    if !(2..=MAX_CONTINUATION_POINTS).contains(&req.max_points) {
        return Err(format!(
            "A branch needs 2 to {MAX_CONTINUATION_POINTS} points in each direction."
        ));
    }
    Ok(())
}

fn validate_lyapunov(f: &dyn VectorField, req: &LyapunovRequest) -> Result<(), String> {
    let dim = f.dim();
    if req.params.len() != f.n_params() {
        return Err(format!(
            "Expected {} parameter value(s) but got {}.",
            f.n_params(),
            req.params.len()
        ));
    }
    if req.x0.len() != dim {
        return Err(format!(
            "The starting point has {} coordinate(s) but the system has {dim}.",
            req.x0.len()
        ));
    }
    if !req.x0.iter().chain([&req.t0]).all(|v| v.is_finite()) {
        return Err("The starting point must be finite.".into());
    }
    let is_map = f.kind() == SystemKind::Map;
    let (span, interval) = if is_map {
        ("number of iterates", "iterates per renormalization")
    } else {
        ("total time", "renormalization interval τ")
    };
    if !(req.transient >= 0.0 && req.transient.is_finite()) {
        return Err("The transient must be zero or a positive number.".into());
    }
    if !(req.total > 0.0 && req.total.is_finite()) {
        return Err(format!("The {span} must be a positive number."));
    }
    if is_map && req.transient.round() + req.total.round() > MAX_LYAPUNOV_ITERATES as f64 {
        return Err(format!(
            "That is {:.0} iterates; at most {MAX_LYAPUNOV_ITERATES} fit in one run. \
             Shorten the transient or the number of iterates.",
            req.transient.round() + req.total.round()
        ));
    }
    if !(req.interval > 0.0 && req.interval.is_finite()) || (is_map && req.interval < 0.5) {
        return Err(format!("The {interval} must be a positive number."));
    }
    let steps = if is_map {
        (req.total.round() / req.interval.round()).ceil()
    } else {
        (req.total / req.interval).ceil()
    };
    if steps > MAX_RENORMALIZATIONS as f64 {
        return Err(format!(
            "That is {steps:.0} renormalizations; at most {MAX_RENORMALIZATIONS} fit in one run. \
             Lengthen the {interval} or shorten the {span}."
        ));
    }
    if req.blocks < 2 {
        return Err("The uncertainty needs at least 2 blocks.".into());
    }
    if req.blocks as f64 > steps {
        return Err(format!(
            "{} blocks need at least as many renormalizations, but this run has {steps:.0}.",
            req.blocks
        ));
    }
    Ok(())
}

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
    match (req.z_axis, req.z_range) {
        (None, None) => {}
        (Some(AxisRef::State { index }), Some((lo, hi))) => {
            if index >= dim {
                return Err(format!(
                    "The third axis shows state {} but the system only has {dim}.",
                    index + 1
                ));
            }
            if !(lo.is_finite() && hi.is_finite() && lo < hi) {
                return Err(format!(
                    "The third axis range [{lo}, {hi}] is not a valid interval."
                ));
            }
        }
        (Some(AxisRef::Time), _) => {
            return Err("The third axis of a 3D view must be a state.".into());
        }
        _ => return Err("A 3D view needs both a third axis and its range.".into()),
    }
    if req.graph_samples.is_some() {
        cobweb_fits(f, req.x_axis)?;
    }
    if let Some(section) = &req.section {
        section_fits(f, section)?;
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

/// The cobweb view (a request with `graphSamples`) needs the graph of one
/// `f`: a one-dimensional autonomous map, with `x_n` across.
fn cobweb_fits(f: &dyn VectorField, x_axis: AxisRef) -> Result<(), String> {
    let dim = f.dim();
    if f.kind() != SystemKind::Map {
        return Err(
            "The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system is a flow."
                .into(),
        );
    }
    if dim != 1 {
        return Err(format!(
            "The cobweb view needs a one-dimensional map x_{{n+1}} = f(x_n); this system has {dim} states."
        ));
    }
    if !f.is_autonomous() {
        return Err("This map changes with n, so f has no single graph to cobweb.".into());
    }
    if x_axis != (AxisRef::State { index: 0 }) {
        return Err("The cobweb view needs x_n on the horizontal axis.".into());
    }
    Ok(())
}

fn section_fits(f: &dyn VectorField, section: &SectionSpec) -> Result<(), String> {
    if f.kind() == SystemKind::Map {
        return Err("A Poincaré section needs a flow; this system is a map.".into());
    }
    match *section {
        SectionSpec::Plane { state, value, .. } => {
            let dim = f.dim();
            if state >= dim {
                let states = if dim == 1 { "state" } else { "states" };
                return Err(format!(
                    "The section's state is out of range: this system has {dim} {states}."
                ));
            }
            if !value.is_finite() {
                return Err("The section's value must be a finite number.".into());
            }
        }
        SectionSpec::Stroboscopic { period, phase } => {
            if !(period > 0.0 && period.is_finite()) {
                return Err("The stroboscopic period must be a positive number.".into());
            }
            if !phase.is_finite() {
                return Err("The stroboscopic phase must be a finite number.".into());
            }
        }
    }
    Ok(())
}

fn to_section(spec: SectionSpec) -> dyn_solve::Section {
    match spec {
        SectionSpec::Plane {
            state,
            value,
            direction,
        } => dyn_solve::Section::Plane {
            state,
            value,
            direction,
        },
        SectionSpec::Stroboscopic { period, phase } => {
            dyn_solve::Section::Stroboscopic { period, phase }
        }
    }
}

fn to_axis(a: AxisRef) -> Axis {
    match a {
        AxisRef::State { index } => Axis::State(index),
        AxisRef::Time => Axis::Time,
    }
}

/// Half-width searched around a hidden state when no state is on an axis.
const HIDDEN_STATE_FALLBACK_HALF_SPAN: f64 = 10.0;

fn visible_axes(req: &PortraitRequest) -> Vec<(AxisRef, (f64, f64))> {
    let mut axes = vec![(req.x_axis, req.x_range), (req.y_axis, req.y_range)];
    axes.extend(req.z_axis.zip(req.z_range));
    axes
}

/// Half the larger range of the state axes in view (0 if none is a state).
fn widest_visible_state_half_span(req: &PortraitRequest) -> f64 {
    visible_axes(req)
        .iter()
        .filter(|(a, _)| matches!(a, AxisRef::State { .. }))
        .map(|(_, (lo, hi))| (hi - lo).abs() / 2.0)
        .fold(0.0_f64, f64::max)
}

pub fn fixed_point_search_box(req: &PortraitRequest, dim: usize) -> Vec<(f64, f64)> {
    let visible = visible_axes(req);
    let span = widest_visible_state_half_span(req);
    let span = if span > 0.0 {
        span
    } else {
        HIDDEN_STATE_FALLBACK_HALF_SPAN
    };
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

/// Newton seeds in total, in every dimension: a hard cap on the work of the
/// fixed-point search. When the grid of [`seeds_per_axis`] points per axis
/// would be larger (2^10 = 1024 in 10D), `dyn_analysis` uses the first
/// `MAX_TOTAL_SEEDS` points of a Halton sequence instead.
pub const MAX_TOTAL_SEEDS: usize = 1000;

/// Grid points per axis: hand-tuned in 1D to 3D, otherwise the largest `k`
/// with `k^dim <= MAX_TOTAL_SEEDS` (at least 2; a grid that still does not
/// fit is replaced by a Halton sample of `MAX_TOTAL_SEEDS` points).
fn seeds_per_axis(dim: usize) -> usize {
    match dim {
        0 => 0,
        1 => 40,
        2 => 14,
        3 => 9,
        d => {
            let fits = |k: usize| {
                (0..d)
                    .try_fold(1usize, |t, _| {
                        t.checked_mul(k).filter(|&t| t <= MAX_TOTAL_SEEDS)
                    })
                    .is_some()
            };
            (2..).take_while(|&k| fits(k)).last().unwrap_or(2)
        }
    }
}

fn method_name(m: Method) -> &'static str {
    match m {
        Method::Euler => "Euler",
        Method::Heun => "Heun",
        Method::Rk4 => "Runge–Kutta 4",
        Method::Dopri5 => "Dormand–Prince 5(4)",
        Method::Verlet => "Störmer–Verlet",
        Method::Yoshida4 => "Yoshida 4",
        Method::Rosenbrock23 => "Rosenbrock 2(3)",
    }
}

fn method_fits(system: &CompiledSystem, method: Method) -> Result<(), String> {
    if !method.is_symplectic() || system.kind() == SystemKind::Map {
        return Ok(());
    }
    if !system.info.hamiltonian {
        return Err(format!(
            "{} needs a Hamiltonian H(q, p); this system is given by its equations of motion.",
            method_name(method)
        ));
    }
    if !system.is_separable() {
        return Err("H(q, p) must split as T(p) + V(q) for a symplectic method.".into());
    }
    Ok(())
}

/// `H` at the start and its largest drift over the samples, for a
/// Hamiltonian flow. Absent when `H` is not finite somewhere, which JSON
/// could not carry.
fn energy(
    system: &CompiledSystem,
    p: &[f64],
    tr: &TrajectoryRequest,
    sol: &dyn_solve::Solution,
) -> Option<EnergyOut> {
    let h0 = system
        .hamiltonian_value(tr.t0, &tr.x0, p)
        .filter(|h| h.is_finite())?;
    let dim = tr.x0.len();
    let mut max_abs_drift: f64 = 0.0;
    for (i, &t) in sol.t.iter().enumerate() {
        let h = system.hamiltonian_value(t, &sol.x[i * dim..(i + 1) * dim], p)?;
        let drift = (h - h0).abs();
        if !drift.is_finite() {
            return None;
        }
        max_abs_drift = max_abs_drift.max(drift);
    }
    Some(EnergyOut { h0, max_abs_drift })
}

fn to_solver(s: &SolverOptions) -> dyn_solve::SolverOptions {
    dyn_solve::SolverOptions {
        method: s.method,
        rtol: s.rtol,
        atol: s.atol,
        h: s.h,
        h_max: s.h_max.unwrap_or(0.0),
        max_steps: s.max_steps,
    }
}

fn trajectory(
    system: &CompiledSystem,
    p: &[f64],
    tr: &TrajectoryRequest,
    req: &PortraitRequest,
) -> TrajectoryOut {
    let f: &dyn VectorField = system;
    let sol = if f.kind() == SystemKind::Map {
        let n = if tr.t1.is_finite() {
            tr.t1.round().clamp(0.0, MAX_ITERATES as f64) as usize
        } else {
            0
        };
        let transient = req.transient.unwrap_or(0).min(MAX_ITERATES - n);
        dyn_solve::iterate_map(f, p, &tr.x0, n, transient)
    } else if let Some(spec) = req.section {
        dyn_solve::section(
            f,
            p,
            tr.t0,
            &tr.x0,
            tr.t1,
            &to_solver(&req.solver),
            &to_section(spec),
            req.transient.unwrap_or(0),
            MAX_SECTION_POINTS,
        )
    } else {
        let opts = to_solver(&req.solver);
        let sampling = match req.samples {
            0 | 1 => Sampling::Steps,
            n => Sampling::Uniform(n.min(MAX_SAMPLES)),
        };
        dyn_solve::integrate(f, p, tr.t0, &tr.x0, tr.t1, &opts, sampling)
    };
    let energy = energy(system, p, tr, &sol);
    TrajectoryOut {
        stiff: sol.stiff.then_some(true),
        t: sol.t,
        x: sol.x,
        status: sol.status,
        stats: sol.stats,
        energy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{Classification, InputFormat, Method, Severity, SolverOptions, Status};
    use serde::Deserialize;

    const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");
    const TEMPLATE_TEXT_JSON: &str =
        include_str!("../../src/templates/templates.text.generated.json");

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
        #[serde(default)]
        kind: Option<String>,
        x: String,
        y: String,
        x_range: (f64, f64),
        y_range: (f64, f64),
        #[serde(default)]
        z: Option<String>,
        #[serde(default)]
        z_range: Option<(f64, f64)>,
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

    fn resolve_axis(name: &str, states: &[StateInfo]) -> AxisRef {
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

    fn template_request(t: &Template, res: &CompileResponse) -> PortraitRequest {
        let dim = res.states.len();
        let is_map = res.kind == Some(SystemKind::Map);
        let x_axis = resolve_axis(&t.view.x, &res.states);
        let y_axis = resolve_axis(&t.view.y, &res.states);
        let z_axis = t.view.z.as_ref().map(|z| resolve_axis(z, &res.states));
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
                h_max: None,
                max_steps: 200_000,
            },
            samples: (max_span * 120.0).clamp(400.0, 8000.0).round() as usize,
            graph_samples: (t.view.kind.as_deref() == Some("cobweb")).then_some(2048),
            transient: None,
            section: None,
            z_axis,
            z_range: t.view.z_range,
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
            assert_eq!(fp.eigenvectors.len(), dim, "{id}");
            assert_eq!(fp.eigenvectors_im.len(), dim, "{id}");
            assert!(all_finite(&fp.x), "{id}: {fp:?}");
            for v in fp.eigenvectors.iter().chain(&fp.eigenvectors_im).flatten() {
                assert_eq!(v.len(), dim, "{id}");
                assert!(all_finite(v), "{id}: {fp:?}");
            }
        }
        for tr in &res.trajectories {
            assert!(!tr.t.is_empty(), "{id}: empty trajectory");
            assert_eq!(tr.x.len(), tr.t.len() * dim, "{id}: x is not t.len() * dim");
            assert!(
                all_finite(&tr.t) && all_finite(&tr.x),
                "{id}: non-finite trajectory"
            );
        }
        let mut numbers_only = res.clone();
        for fp in &mut numbers_only.fixed_points {
            fp.eigenvectors.clear();
            fp.eigenvectors_im.clear();
        }
        let json = serde_json::to_string(&numbers_only).unwrap();
        for bad in [":null", "[null", ",null"] {
            assert!(!json.contains(bad), "{id}: JSON has null");
        }
        assert!(res.elapsed_ms.is_finite() && res.elapsed_ms >= 0.0);
    }

    #[test]
    fn every_template_compiles_and_draws() {
        let store = SystemStore::new();
        let all = templates();
        assert_eq!(all.len(), 31, "the gallery shows 31 templates");
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
        let (_, out) = template_portrait("lorenz");
        let c = (8.0 / 3.0 * 27.0_f64).sqrt();
        for x in [[0.0, 0.0, 0.0], [c, c, 27.0], [-c, -c, 27.0]] {
            let fp = find_fp(&out, &x);
            assert_eq!(fp.classification, Classification::SaddleND, "at {x:?}");
            assert!(!fp.stable);
        }
        assert_eq!(out.fixed_points.len(), 3);
        assert!(out
            .trajectories
            .iter()
            .all(|t| t.status == Status::Completed));
    }

    #[test]
    fn logistic_map_opens_as_a_cobweb_from_x0() {
        let t = template("logistic-map");
        assert_eq!(t.view.kind.as_deref(), Some("cobweb"));
        let (_, out) = template_portrait("logistic-map");
        let tr = &out.trajectories[0];
        assert_eq!(tr.t.len(), 61);
        assert_eq!(tr.t[0], 0.0);
        assert_eq!(tr.t[60], 60.0);
        assert_eq!(tr.x[0], 0.1);
        assert_eq!(tr.x[1], 3.2 * 0.1 * 0.9);
        assert_eq!(out.fixed_points.len(), 2, "{:?}", out.fixed_points);
        let origin = find_fp(&out, &[0.0]);
        let inner = find_fp(&out, &[0.6875]);
        assert!(!origin.stable && !inner.stable);
        // The orbit winds out from x* = 0.6875 onto the 2-cycle {0.513, 0.799}.
        let tail = &tr.x[50..];
        assert!(
            tail.iter()
                .all(|&x| (x - 0.513).abs() < 1e-3 || (x - 0.7995).abs() < 1e-3),
            "{tail:?}"
        );
        let graph = out.graph.as_ref().expect("the cobweb asks for the graph");
        assert_eq!(graph.len(), 1);
        assert_eq!(graph[0].len(), 2048);
        assert_eq!(graph[0][0].0, -0.05);
        assert_eq!(graph[0][2047].0, 1.05);
    }

    #[test]
    fn non_autonomous_slope_field_has_no_fixed_points_and_unit_time_component() {
        let (res, out) = template_portrait("slope-field-x2-minus-t");
        assert!(!res.autonomous);
        assert!(out.fixed_points.is_empty());
        assert!(out.arrows.iter().all(|a| a.u == 1.0));
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
                h_max: None,
                max_steps: 1000,
            },
            samples: 11,
            graph_samples: None,
            transient: None,
            section: None,
            z_axis: None,
            z_range: None,
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
        req.samples = 0;
        let out = compute_portrait(&sys, &req).unwrap();
        assert_eq!(out.trajectories[0].t.len(), 101);
    }

    #[test]
    fn a_3d_view_searches_its_third_axis_for_fixed_points() {
        let store = SystemStore::new();
        let id = compile_src(
            &store,
            "\\dot{x}=10(y-x)\n\\dot{y}=28x-y-xz\n\\dot{z}=xy-\\frac{8}{3}z",
        );
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 3);
        req.field_grid = 0;
        req.fixed_points = true;
        req.x_range = (-20.0, 20.0);
        req.y_range = (-25.0, 25.0);
        req.base_state = vec![0.0, 0.0, 0.0];
        // Strogatz §9.2: C± = (±√(b(r−1)), ±√(b(r−1)), r−1).
        let c = (8.0 / 3.0 * 27.0_f64).sqrt();
        let c_pm = [[c, c, 27.0], [-c, -c, 27.0]];
        let near = |out: &PortraitResponse, x: &[f64; 3]| {
            out.fixed_points
                .iter()
                .any(|fp| fp.x.iter().zip(x).all(|(a, b)| (a - b).abs() < 1e-6))
        };

        assert_eq!(fixed_point_search_box(&req, 3)[2], (-25.0, 25.0));
        req.z_axis = Some(AxisRef::State { index: 2 });
        req.z_range = Some((0.0, 50.0));
        let out = compute_portrait(&sys, &req).unwrap();
        for x in &c_pm {
            assert!(near(&out, x), "C± = {x:?} not in {:?}", out.fixed_points);
        }
        assert!(near(&out, &[0.0, 0.0, 0.0]));
        assert_eq!(out.fixed_points.len(), 3);
        assert_eq!(
            fixed_point_search_box(&req, 3),
            vec![(-20.0, 20.0), (-25.0, 25.0), (0.0, 50.0)]
        );
    }

    #[test]
    fn bad_third_axes_get_readable_errors() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=y\n\\dot{y}=-x");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 2);
        let err = |req: &PortraitRequest| compute_portrait(&sys, req).unwrap_err();
        req.z_axis = Some(AxisRef::State { index: 2 });
        req.z_range = Some((0.0, 1.0));
        assert_eq!(
            err(&req),
            "The third axis shows state 3 but the system only has 2."
        );
        req.z_axis = Some(AxisRef::Time);
        assert_eq!(err(&req), "The third axis of a 3D view must be a state.");
        req.z_axis = Some(AxisRef::State { index: 1 });
        req.z_range = Some((1.0, 1.0));
        assert_eq!(
            err(&req),
            "The third axis range [1, 1] is not a valid interval."
        );
        req.z_range = None;
        assert_eq!(
            err(&req),
            "A 3D view needs both a third axis and its range."
        );
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

    const Y_FALLING: SectionSpec = SectionSpec::Plane {
        state: 1,
        value: 0.0,
        direction: dyn_solve::Direction::Falling,
    };

    #[test]
    fn bad_sections_get_readable_errors() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=y\n\\dot{y}=-x");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 2);
        let err = |req: &PortraitRequest| compute_portrait(&sys, req).unwrap_err();
        req.section = Some(SectionSpec::Plane {
            state: 2,
            value: 0.0,
            direction: dyn_solve::Direction::Rising,
        });
        assert_eq!(
            err(&req),
            "The section's state is out of range: this system has 2 states."
        );
        req.section = Some(SectionSpec::Plane {
            state: 0,
            value: f64::INFINITY,
            direction: dyn_solve::Direction::Rising,
        });
        assert_eq!(err(&req), "The section's value must be a finite number.");
        for period in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            req.section = Some(SectionSpec::Stroboscopic { period, phase: 0.0 });
            assert_eq!(
                err(&req),
                "The stroboscopic period must be a positive number.",
                "period {period}"
            );
        }
        req.section = Some(SectionSpec::Stroboscopic {
            period: 1.0,
            phase: f64::NAN,
        });
        assert_eq!(err(&req), "The stroboscopic phase must be a finite number.");
        req.section = Some(Y_FALLING);
        assert!(compute_portrait(&sys, &req).is_ok());

        let (store, map) = compile_map(LOGISTIC);
        let sys = store.get(map).unwrap();
        let mut req = simple_request(map, 1);
        req.params = vec![3.2];
        req.section = Some(SectionSpec::Stroboscopic {
            period: 1.0,
            phase: 0.0,
        });
        assert_eq!(
            compute_portrait(&sys, &req).unwrap_err(),
            "A Poincaré section needs a flow; this system is a map."
        );

        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=-x");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 1);
        req.section = Some(Y_FALLING);
        assert_eq!(
            compute_portrait(&sys, &req).unwrap_err(),
            "The section's state is out of range: this system has 1 state."
        );
    }

    #[test]
    fn a_stroboscopic_phase_is_absolute_for_every_start_time() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=y\n\\dot{y}=-x");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 2);
        req.trajectories = [0.0, 1.0, 0.75]
            .map(|t0| TrajectoryRequest {
                x0: vec![1.0, 0.0],
                t0,
                t1: t0 + 10.0,
            })
            .to_vec();
        req.section = Some(SectionSpec::Stroboscopic {
            period: 2.0,
            phase: 0.75,
        });
        let out = compute_portrait(&sys, &req).unwrap();
        let firsts: Vec<f64> = out.trajectories.iter().map(|tr| tr.t[0]).collect();
        assert_eq!(firsts, [0.75, 2.75, 0.75]);
        let counts: Vec<usize> = out.trajectories.iter().map(|tr| tr.t.len()).collect();
        assert_eq!(
            counts,
            [5, 5, 6],
            "t1 = 10.75 is a sample of the last orbit"
        );
        for tr in &out.trajectories {
            for &t in &tr.t {
                assert!(
                    ((t - 0.75) / 2.0 - ((t - 0.75) / 2.0).round()).abs() < 1e-12,
                    "{t}"
                );
            }
        }
    }

    #[test]
    fn a_section_returns_the_crossings_after_the_transient() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=y\n\\dot{y}=-x");
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 2);
        req.solver.method = Method::Dopri5;
        req.solver.rtol = 1e-10;
        req.solver.atol = 1e-12;
        req.solver.max_steps = 100_000;
        req.trajectories = vec![TrajectoryRequest {
            x0: vec![0.0, 1.0],
            t0: 0.0,
            t1: 10.0 * std::f64::consts::TAU,
        }];
        req.section = Some(Y_FALLING);
        req.transient = Some(3);
        let out = compute_portrait(&sys, &req).unwrap();
        let tr = &out.trajectories[0];
        assert_eq!(tr.status, Status::Completed);
        assert_eq!(tr.t.len(), 7, "10 falling crossings at x = 1, 3 skipped");
        for (k, (&t, x)) in tr.t.iter().zip(tr.x.chunks(2)).enumerate() {
            let want = std::f64::consts::FRAC_PI_2 + (k + 3) as f64 * std::f64::consts::TAU;
            assert!((t - want).abs() < 1e-8, "{t} vs {want}");
            assert!((x[0] - 1.0).abs() < 1e-8 && x[1].abs() < 1e-8, "{x:?}");
        }
    }

    #[test]
    #[ignore = "timing; run in release"]
    fn section_costs_about_as_much_as_plain_integration() {
        const RUNS: usize = 5;
        let store = SystemStore::new();
        let id = compile_src(
            &store,
            "\\dot{x}=10\\left(y-x\\right)\n\\dot{y}=28x-y-xz\n\\dot{z}=xy-\\frac{8}{3}z",
        );
        let sys = store.get(id).unwrap();
        let mut req = simple_request(id, 3);
        req.field_grid = 0;
        req.solver = SolverOptions {
            method: Method::Dopri5,
            rtol: 1e-6,
            atol: 1e-9,
            h: 0.0,
            h_max: None,
            max_steps: 1_000_000,
        };
        req.trajectories = vec![TrajectoryRequest {
            x0: vec![0.0, 1.0, 0.0],
            t0: 0.0,
            t1: 1000.0,
        }];
        req.samples = 20_000;
        let mut sectioned = req.clone();
        sectioned.section = Some(SectionSpec::Plane {
            state: 2,
            value: 27.0,
            direction: dyn_solve::Direction::Rising,
        });
        let (mut plain_ms, mut section_ms) = (Vec::new(), Vec::new());
        let (mut plain_out, mut section_out) = (None, None);
        for _ in 0..RUNS {
            let out = compute_portrait(&sys, &req).unwrap();
            plain_ms.push(out.elapsed_ms);
            plain_out = Some(out);
            let out = compute_portrait(&sys, &sectioned).unwrap();
            section_ms.push(out.elapsed_ms);
            section_out = Some(out);
        }
        let (plain, section) = (plain_out.unwrap(), section_out.unwrap());
        let (p, s) = (&plain.trajectories[0], &section.trajectories[0]);
        assert_eq!((p.status, s.status), (Status::Completed, Status::Completed));
        let median = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        let spread = |v: &[f64]| format!("{:.1}–{:.1}", v[0], v[v.len() - 1]);
        let (pm, sm) = (median(&mut plain_ms), median(&mut section_ms));
        println!(
            "{RUNS} interleaved runs each. plain: median {pm:.1} ms ({} ms), {} points, {} evals, {} steps",
            spread(&plain_ms),
            p.t.len(),
            p.stats.evals,
            p.stats.accepted
        );
        println!(
            "section z = 27 rising: median {sm:.1} ms ({} ms), {} crossings, {} evals, {} steps; ratio {:.3}",
            spread(&section_ms),
            s.t.len(),
            s.stats.evals,
            s.stats.accepted,
            sm / pm
        );
        assert!(sm <= 1.3 * pm, "section {sm:.1} ms vs plain {pm:.1} ms");
        assert!(sm < 250.0, "section {sm:.1} ms");
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
        let ids: Vec<u32> = (0..RETAINED_SYSTEMS_FOR_IN_FLIGHT_REQUESTS + 5)
            .map(|_| compile_src(&store, "\\dot{x}=-x"))
            .collect();
        assert!(ids.windows(2).all(|w| w[1] == w[0] + 1));
        assert_eq!(ids[0], 1);
        assert_eq!(store.len(), RETAINED_SYSTEMS_FOR_IN_FLIGHT_REQUESTS);
        assert!(store.get(ids[4]).is_none());
        assert!(store.get(ids[5]).is_some());
        assert!(store.get(*ids.last().unwrap()).is_some());
    }

    /// Every state's equation is `x_i' = 0`, so every Newton start is a
    /// fixed point and the number of points found is the number of starts.
    /// In 1-D a zero field is one interval of fixed points, reported by its
    /// two ends, so there `x1'` vanishes at each of the 40 seeds (the cell
    /// centers of the search box [−1, 1]) and nowhere between.
    fn zero_system_fixed_point_count(dim: usize) -> usize {
        let store = SystemStore::new();
        let src: Vec<String> = if dim == 1 {
            vec!["x1' = sin(pi*((x1 + 1)*20 - 0.5))".into()]
        } else {
            (1..=dim).map(|i| format!("x{i}' = 0")).collect()
        };
        let res = compile_system(
            &store,
            &CompileRequest {
                source: src.join("\n"),
                format: InputFormat::Text,
            },
        );
        assert!(res.ok, "{:?}", res.diagnostics);
        let sys = store.get(res.system_id.unwrap()).unwrap();
        let mut req = simple_request(res.system_id.unwrap(), dim);
        req.field_grid = 0;
        req.fixed_points = true;
        compute_portrait(&sys, &req).unwrap().fixed_points.len()
    }

    #[test]
    fn the_newton_seed_budget_is_a_hard_cap_in_every_dimension() {
        for (dim, want) in [
            (1, 40),
            (2, 14 * 14),
            (3, 9 * 9 * 9),
            (10, 1000),
            (12, 1000),
        ] {
            let per_axis = seeds_per_axis(dim);
            let starts = dyn_analysis::seed_count(dim, per_axis, MAX_TOTAL_SEEDS);
            assert_eq!(starts, want, "dim {dim}");
            assert!(starts <= MAX_TOTAL_SEEDS, "dim {dim}: {starts} seeds");
            assert_eq!(zero_system_fixed_point_count(dim), want, "dim {dim}");
        }
        for dim in 1..=20 {
            let starts = dyn_analysis::seed_count(dim, seeds_per_axis(dim), MAX_TOTAL_SEEDS);
            assert!(starts <= MAX_TOTAL_SEEDS, "dim {dim}: {starts} seeds");
        }
        assert_eq!(seeds_per_axis(4), 5);
        assert_eq!(seeds_per_axis(5), 3);
        assert_eq!(seeds_per_axis(10), 2);
    }

    fn compile_map(src: &str) -> (SystemStore, u32) {
        let store = SystemStore::new();
        let id = compile_src(&store, src);
        (store, id)
    }

    fn cobweb_request(system_id: u32, params: Vec<f64>) -> PortraitRequest {
        let mut req = simple_request(system_id, 1);
        req.params = params;
        req.x_axis = AxisRef::State { index: 0 };
        req.y_axis = AxisRef::State { index: 0 };
        req.x_range = (-0.05, 1.05);
        req.y_range = (-0.05, 1.05);
        req.field_grid = 0;
        req.graph_samples = Some(2048);
        req
    }

    const LOGISTIC: &str = "x_{n+1}=rx_n\\left(1-x_n\\right)";

    #[test]
    fn the_cobweb_view_needs_a_one_dimensional_autonomous_map() {
        let cases = [
            (
                "x_{n+1}=y_n+1-ax_n^2\ny_{n+1}=bx_n",
                2,
                "The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system has 2 states.",
            ),
            (
                "x_{n+1}=\\frac{x_n}{2}+n",
                0,
                "This map changes with n, so f has no single graph to cobweb.",
            ),
            (
                "\\dot{x}=-x",
                0,
                "The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system is a flow.",
            ),
        ];
        for (src, n_params, want) in cases {
            let (store, id) = compile_map(src);
            let sys = store.get(id).unwrap();
            let mut req = cobweb_request(id, vec![1.0; n_params]);
            req.base_state = vec![0.0; sys.dim()];
            req.y_axis = AxisRef::State {
                index: sys.dim() - 1,
            };
            assert_eq!(compute_portrait(&sys, &req).unwrap_err(), want, "{src}");
            // Without the graph, the same request is fine.
            req.graph_samples = None;
            assert!(compute_portrait(&sys, &req).is_ok(), "{src}");
        }
        let (store, id) = compile_map(LOGISTIC);
        let sys = store.get(id).unwrap();
        let mut req = cobweb_request(id, vec![3.2]);
        req.x_axis = AxisRef::Time;
        assert_eq!(
            compute_portrait(&sys, &req).unwrap_err(),
            "The cobweb view needs x_n on the horizontal axis."
        );
    }

    #[test]
    fn the_graph_comes_back_exactly_when_asked_for() {
        let (store, id) = compile_map(LOGISTIC);
        let sys = store.get(id).unwrap();
        let mut req = cobweb_request(id, vec![3.2]);
        let out = compute_portrait(&sys, &req).unwrap();
        let graph = out.graph.as_ref().unwrap();
        assert_eq!(graph.len(), 1);
        for &(x, y) in &graph[0] {
            assert_eq!(y, 3.2 * x * (1.0 - x));
        }
        let json = serde_json::to_value(&out).unwrap();
        assert!(json["graph"][0][0].is_array());
        // In view nowhere: present and empty.
        req.y_range = (50.0, 60.0);
        let out = compute_portrait(&sys, &req).unwrap();
        assert_eq!(out.graph, Some(vec![]));
        assert_eq!(
            serde_json::to_value(&out).unwrap()["graph"],
            serde_json::json!([])
        );
        req.graph_samples = None;
        let out = compute_portrait(&sys, &req).unwrap();
        assert!(out.graph.is_none());
        assert!(serde_json::to_value(&out).unwrap().get("graph").is_none());
    }

    #[test]
    fn a_transient_hides_the_first_iterates() {
        let (store, id) = compile_map(LOGISTIC);
        let sys = store.get(id).unwrap();
        let mut req = cobweb_request(id, vec![3.5]);
        req.trajectories = vec![TrajectoryRequest {
            x0: vec![0.1],
            t0: 0.0,
            t1: 60.0,
        }];
        req.transient = Some(100);
        let out = compute_portrait(&sys, &req).unwrap();
        let tr = &out.trajectories[0];
        assert_eq!(tr.status, Status::Completed);
        assert_eq!(tr.t.len(), 61);
        assert_eq!(tr.t[0], 100.0);
        assert_eq!(tr.t[60], 160.0);
        let mut x = 0.1;
        for _ in 0..100 {
            x = 3.5 * x * (1.0 - x);
        }
        assert_eq!(tr.x[0], x);
        // r = 3.5: the 4-cycle, period doubling.
        let mut tail: Vec<f64> = tr.x[52..].to_vec();
        tail.sort_by(f64::total_cmp);
        tail.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        assert_eq!(tail.len(), 4, "{tail:?}");
        // The time series of the same map hides the same iterates.
        req.graph_samples = None;
        req.x_axis = AxisRef::Time;
        req.x_range = (0.0, 200.0);
        assert_eq!(
            compute_portrait(&sys, &req).unwrap().trajectories[0].t[0],
            100.0
        );
        // Clamped so that transient + n stays within MAX_ITERATES.
        req.transient = Some(usize::MAX);
        req.trajectories[0].t1 = 10.0;
        let tr = &compute_portrait(&sys, &req).unwrap().trajectories[0];
        assert_eq!(tr.t[0], (MAX_ITERATES - 10) as f64);
        assert_eq!(*tr.t.last().unwrap(), MAX_ITERATES as f64);
        // Ignored for flows.
        let flow = compile_src(&store, "\\dot{x}=-x");
        let mut req = simple_request(flow, 1);
        req.x_axis = AxisRef::Time;
        req.transient = Some(100);
        req.trajectories = vec![TrajectoryRequest {
            x0: vec![1.0],
            t0: 0.0,
            t1: 1.0,
        }];
        let out = compute_portrait(&store.get(flow).unwrap(), &req).unwrap();
        assert_eq!(out.trajectories[0].t[0], 0.0);
    }

    /// `λ = f'(x*)` of a one-dimensional map: its only eigenvalue, from the
    /// exact (AD) Jacobian.
    #[test]
    fn one_dimensional_maps_have_the_multiplier_f_prime() {
        fn multipliers(src: &str, params: Vec<f64>) -> PortraitResponse {
            let (store, id) = compile_map(src);
            let mut req = cobweb_request(id, params);
            req.fixed_points = true;
            compute_portrait(&store.get(id).unwrap(), &req).unwrap()
        }
        let check = |out: &PortraitResponse, x: f64, lambda: f64, class: Classification| {
            let fp = find_fp(out, &[x]);
            assert_eq!(fp.eigenvalues.len(), 1);
            let (re, im) = fp.eigenvalues[0];
            assert!(
                (re - lambda).abs() < 1e-12,
                "λ = {re} at {x}, want {lambda}"
            );
            assert_eq!(im, 0.0);
            assert_eq!(fp.classification, class, "at {x}");
            assert_eq!(fp.stable, class == Classification::Sink, "at {x}");
        };
        let out = multipliers(LOGISTIC, vec![3.2]);
        assert_eq!(out.fixed_points.len(), 2);
        check(&out, 0.0, 3.2, Classification::Source);
        check(&out, 0.6875, -1.2, Classification::Source);
        check(
            &multipliers(LOGISTIC, vec![2.5]),
            0.6,
            -0.5,
            Classification::Sink,
        );
        check(
            &multipliers(LOGISTIC, vec![3.0]),
            2.0 / 3.0,
            -1.0,
            Classification::NonHyperbolic,
        );
        check(
            &multipliers(LOGISTIC, vec![2.0]),
            0.5,
            0.0,
            Classification::Sink,
        );
        let tent = "x_{n+1}=\\frac{r}{2}\\left(1-\\left|2x_n-1\\right|\\right)";
        check(
            &multipliers(tent, vec![1.5]),
            0.6,
            -1.5,
            Classification::Source,
        );
        let sine = multipliers("x_{n+1}=r\\sin\\left(\\pi x_n\\right)", vec![0.7]);
        let fp = sine
            .fixed_points
            .iter()
            .find(|fp| fp.x[0] > 0.1)
            .expect("a nonzero fixed point");
        let x = fp.x[0];
        assert!((0.7 * (std::f64::consts::PI * x).sin() - x).abs() < 1e-12);
        let lambda = std::f64::consts::PI * 0.7 * (std::f64::consts::PI * x).cos();
        assert!((fp.eigenvalues[0].0 - lambda).abs() < 1e-12);
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
        assert_eq!(req.solver.h_max, None);
        assert_eq!(req.graph_samples, None);
        assert_eq!(req.transient, None);
        let cobweb: PortraitRequest = serde_json::from_value(serde_json::json!({
            "requestId": 4, "systemId": 1, "params": [3.2],
            "xAxis": { "kind": "state", "index": 0 }, "yAxis": { "kind": "state", "index": 0 },
            "xRange": [-0.05, 1.05], "yRange": [-0.05, 1.05], "baseState": [0], "fieldTime": 0,
            "fieldGrid": 0, "nullclines": false, "fixedPoints": true,
            "trajectories": [{ "x0": [0.1], "t0": 0, "t1": 60 }],
            "solver": { "method": "dopri5", "rtol": 1e-6, "atol": 1e-9, "h": 0, "maxSteps": 200000 },
            "samples": 400, "graphSamples": 2048, "transient": 200
        }))
        .expect("a cobweb request deserializes");
        assert_eq!(cobweb.graph_samples, Some(2048));
        assert_eq!(cobweb.transient, Some(200));
        let back = serde_json::to_value(&cobweb).unwrap();
        assert_eq!(back["graphSamples"], 2048);
        assert_eq!(back["transient"], 200);

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
        assert_eq!(json["states"][0]["derived"], false);
        let second_order = compile_system(
            &SystemStore::new(),
            &CompileRequest {
                source: "\\ddot{\\theta}=-\\sin\\theta".into(),
                format: InputFormat::Latex,
            },
        );
        let states = serde_json::to_value(&second_order.states).unwrap();
        assert_eq!(
            states,
            serde_json::json!([
                { "name": "theta", "latex": "\\theta", "derived": false },
                { "name": "theta'", "latex": "\\dot{\\theta}", "derived": true }
            ])
        );

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
        assert!(
            json.get("graph").is_none(),
            "no graph unless graphSamples was sent"
        );
        assert_eq!(json["requestId"], 3);
        assert_eq!(json["fixedPoints"][0]["classification"], "sink");
        assert!(json["fixedPoints"][0]["eigenvalues"][0].is_array());
        assert_eq!(
            json["fixedPoints"][0]["eigenvectors"][0],
            serde_json::json!([1.0])
        );
        assert!(json["fixedPoints"][0]["eigenvectorsIm"][0].is_null());
        assert_eq!(json["trajectories"][0]["status"], "completed");
        assert!(json["trajectories"][0]["stats"]["evals"].is_u64());
        assert!(json["nullclines"][0]["polylines"][0][0].is_array());
    }

    #[test]
    fn every_template_converts_to_its_generated_text_over_ipc() {
        let text: HashMap<String, Vec<String>> =
            serde_json::from_str(TEMPLATE_TEXT_JSON).expect("the template text parses");
        let store = SystemStore::new();
        for t in templates() {
            let res = convert_source(&ConvertRequest {
                lines: t.equations.clone(),
                from: InputFormat::Latex,
                to: InputFormat::Text,
            });
            let lines: Vec<String> = res
                .lines
                .iter()
                .map(|l| {
                    assert!(l.converted && l.note.is_none(), "{}: {l:?}", t.id);
                    l.source.clone()
                })
                .collect();
            assert_eq!(Some(&lines), text.get(&t.id), "{}", t.id);
            let latex = compile_ok(&store, &t);
            let plain = compile_system(
                &store,
                &CompileRequest {
                    source: lines.join("\n"),
                    format: InputFormat::Text,
                },
            );
            assert!(plain.ok, "{}: {:?}", t.id, plain.diagnostics);
            assert_eq!(plain.states, latex.states, "{}", t.id);
            assert_eq!(plain.params, latex.params, "{}", t.id);
        }
    }

    #[test]
    fn convert_source_keeps_lines_it_cannot_convert_with_a_note() {
        let res = convert_source(&ConvertRequest {
            lines: vec!["x' = y".into(), "".into(), "y' = (x".into()],
            from: InputFormat::Text,
            to: InputFormat::Latex,
        });
        let json = serde_json::to_value(&res).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "lines": [
                { "source": "\\dot{x} = y", "converted": true, "error": false },
                { "source": "", "converted": true, "error": false },
                { "source": "y' = (x", "converted": false, "error": true,
                  "note": "This line has an error, so it was kept as typed" }
            ]})
        );
        let req: ConvertRequest = serde_json::from_value(serde_json::json!({
            "lines": ["\\dot{x}=-x"], "from": "latex", "to": "text"
        }))
        .expect("the request the UI sends deserializes");
        assert_eq!(convert_source(&req).lines[0].source, "x' = -x");
    }

    #[test]
    fn a_valid_line_the_target_cannot_write_is_not_an_error() {
        // Kept as typed, LaTeX would read it as -x/9.109·e - 31: the UI must
        // not switch, so the line is flagged as valid (`error: false`).
        let res = convert_source(&ConvertRequest {
            lines: vec!["x' = -x/9.109e-31".into(), "y' = (x".into()],
            from: InputFormat::Text,
            to: InputFormat::Latex,
        });
        let json = serde_json::to_value(&res).unwrap();
        assert_eq!(json["lines"][0]["converted"], false);
        assert_eq!(json["lines"][0]["error"], false);
        assert_eq!(
            json["lines"][0]["note"],
            "LaTeX can't write the number 9.109e-31 exactly"
        );
        assert_eq!(json["lines"][1]["converted"], false);
        assert_eq!(json["lines"][1]["error"], true);
    }

    #[test]
    fn orbit_diagrams_refuse_what_they_cannot_draw() {
        let store = SystemStore::new();
        let logistic = compile_src(&store, "x_{n+1}=rx_n\\left(1-x_n\\right)");
        let system = store.get(logistic).unwrap();
        let ok = OrbitDiagramRequest {
            request_id: 9,
            system_id: logistic,
            params: vec![3.2],
            param: 0,
            range: (2.5, 3.2),
            columns: 2,
            state: 0,
            x0: vec![0.1],
            transient: 1000,
            kept: 4,
            doublings: true,
        };
        let out = compute_orbit_diagram(&system, &ok).unwrap();
        assert_eq!(out.request_id, 9);
        assert_eq!(out.r, [2.5, 3.2]);
        assert_eq!(out.x.len(), 8);
        assert!(
            out.x[..4].iter().all(|x| (x - 0.6).abs() < 1e-12),
            "{:?}",
            out.x
        );
        let doublings = out.doublings.as_deref().unwrap();
        assert_eq!(doublings.len(), 1, "{doublings:?}");
        assert_eq!(doublings[0].period, 1);
        assert!((doublings[0].r - 3.0).abs() < 1e-9, "{doublings:?}");
        let json = serde_json::to_value(
            compute_orbit_diagram(
                &system,
                &OrbitDiagramRequest {
                    range: (4.5, 5.0),
                    transient: 100,
                    doublings: false,
                    ..ok.clone()
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            json["x"].as_array().unwrap().iter().all(|x| x.is_null()),
            "{json}"
        );
        assert!(json.get("doublings").is_none(), "{json}");

        let at_the_limit = OrbitDiagramRequest {
            columns: MAX_ORBIT_COLUMNS,
            kept: MAX_ORBIT_POINTS / MAX_ORBIT_COLUMNS,
            transient: 0,
            doublings: false,
            ..ok.clone()
        };
        assert!(compute_orbit_diagram(&system, &at_the_limit).is_ok());
        let refused = [
            OrbitDiagramRequest {
                param: 1,
                ..ok.clone()
            },
            OrbitDiagramRequest {
                range: (3.2, 2.5),
                ..ok.clone()
            },
            OrbitDiagramRequest {
                range: (2.5, f64::INFINITY),
                ..ok.clone()
            },
            OrbitDiagramRequest {
                state: 1,
                ..ok.clone()
            },
            OrbitDiagramRequest {
                columns: 1,
                ..ok.clone()
            },
            OrbitDiagramRequest {
                columns: MAX_ORBIT_COLUMNS + 1,
                ..ok.clone()
            },
            OrbitDiagramRequest {
                kept: 0,
                ..ok.clone()
            },
            OrbitDiagramRequest {
                transient: ORBIT_TRANSIENT_MAX as usize + 1,
                ..ok.clone()
            },
            OrbitDiagramRequest {
                transient: usize::MAX,
                ..ok.clone()
            },
            OrbitDiagramRequest {
                kept: MAX_ORBIT_POINTS / MAX_ORBIT_COLUMNS + 1,
                ..at_the_limit.clone()
            },
            OrbitDiagramRequest {
                x0: vec![f64::NAN],
                ..ok.clone()
            },
            OrbitDiagramRequest {
                params: vec![],
                ..ok.clone()
            },
        ];
        for req in &refused {
            assert!(
                compute_orbit_diagram(&system, req).is_err(),
                "{req:?} was accepted"
            );
        }

        let henon = compile_src(&store, "x_{n+1}=1-ax_n^2+y_n\ny_{n+1}=bx_n");
        let henon_req = OrbitDiagramRequest {
            system_id: henon,
            params: vec![1.4, 0.3],
            x0: vec![0.0, 0.0],
            state: 1,
            range: (1.0, 1.4),
            ..ok.clone()
        };
        let system = store.get(henon).unwrap();
        assert!(compute_orbit_diagram(&system, &henon_req).is_err());
        assert!(compute_orbit_diagram(
            &system,
            &OrbitDiagramRequest {
                doublings: false,
                ..henon_req
            }
        )
        .is_ok());

        let driven = compile_src(&store, "x_{n+1}=\\frac{x_n}{2}+an");
        let driven_req = OrbitDiagramRequest {
            system_id: driven,
            params: vec![0.5],
            ..ok.clone()
        };
        let system = store.get(driven).unwrap();
        assert!(compute_orbit_diagram(&system, &driven_req).is_err());
        assert!(compute_orbit_diagram(
            &system,
            &OrbitDiagramRequest {
                doublings: false,
                ..driven_req
            }
        )
        .is_ok());

        let flow = compile_src(&store, "\\dot{x}=ax");
        let flow_req = OrbitDiagramRequest {
            system_id: flow,
            params: vec![0.5],
            doublings: false,
            ..ok.clone()
        };
        assert!(compute_orbit_diagram(&store.get(flow).unwrap(), &flow_req).is_err());
    }

    fn symplectic(method: Method, id: u32) -> PortraitRequest {
        let mut req = simple_request(id, 2);
        req.solver.method = method;
        req.solver.h = 0.05;
        req.solver.max_steps = 100_000;
        req.trajectories = vec![TrajectoryRequest {
            x0: vec![1.0, 0.0],
            t0: 0.0,
            t1: 100.0,
        }];
        req
    }

    #[test]
    fn symplectic_methods_need_a_separable_hamiltonian() {
        let store = SystemStore::new();
        let compiled = |src: &str| {
            let res = compile_system(
                &store,
                &CompileRequest {
                    source: src.into(),
                    format: InputFormat::Text,
                },
            );
            assert!(res.ok, "{:?}", res.diagnostics);
            (res.separable, res.system_id.unwrap())
        };
        let (separable, pendulum) = compiled("H(q, p) = p^2/2 - cos(q)");
        assert!(separable);
        let (separable, coupled) = compiled("H(q, p) = p^2/2 + q^2/2 + q*p");
        assert!(!separable);
        let (separable, flow) = compiled("x' = y; y' = -sin(x)");
        assert!(!separable);
        let (_, map) = compiled("x_{n+1} = r*x_n*(1 - x_n)");

        let err = |method, id| {
            compute_portrait(&store.get(id).unwrap(), &symplectic(method, id)).unwrap_err()
        };
        assert_eq!(
            err(Method::Verlet, flow),
            "Störmer–Verlet needs a Hamiltonian H(q, p); this system is given by its equations of motion."
        );
        assert_eq!(
            err(Method::Yoshida4, flow),
            "Yoshida 4 needs a Hamiltonian H(q, p); this system is given by its equations of motion."
        );
        assert_eq!(
            err(Method::Verlet, coupled),
            "H(q, p) must split as T(p) + V(q) for a symplectic method."
        );

        let mut on_map = symplectic(Method::Verlet, map);
        on_map.params = vec![3.5];
        on_map.y_axis = AxisRef::Time;
        on_map.base_state = vec![0.0];
        on_map.trajectories[0].x0 = vec![0.2];
        assert!(compute_portrait(&store.get(map).unwrap(), &on_map).is_ok());

        for method in [Method::Verlet, Method::Yoshida4] {
            let out =
                compute_portrait(&store.get(pendulum).unwrap(), &symplectic(method, pendulum))
                    .unwrap();
            let tr = &out.trajectories[0];
            assert_eq!(tr.status, Status::Completed);
            let energy = tr
                .energy
                .expect("a Hamiltonian trajectory reports its energy");
            assert!((energy.h0 - (-(1f64.cos()))).abs() < 1e-15, "{energy:?}");
            assert!(
                energy.max_abs_drift > 0.0 && energy.max_abs_drift < 1e-3,
                "{method:?}: {energy:?}"
            );
            let worst = (0..tr.t.len())
                .map(|i| (tr.x[2 * i + 1].powi(2) / 2.0 - tr.x[2 * i].cos() - energy.h0).abs())
                .fold(0.0, f64::max);
            assert!(
                (worst - energy.max_abs_drift).abs() < 1e-15,
                "{worst} vs {energy:?}"
            );
        }
        let rk4 = compute_portrait(
            &store.get(pendulum).unwrap(),
            &symplectic(Method::Rk4, pendulum),
        )
        .unwrap();
        assert!(rk4.trajectories[0].energy.is_some());
        let flow_out =
            compute_portrait(&store.get(flow).unwrap(), &symplectic(Method::Rk4, flow)).unwrap();
        assert_eq!(flow_out.trajectories[0].energy, None);
    }

    #[test]
    fn dormand_prince_flags_stiff_steps_and_rosenbrock_takes_few() {
        let store = SystemStore::new();
        let res = compile_system(
            &store,
            &CompileRequest {
                source: "x' = mu*(y - x^3/3 + x); y' = -x/mu".into(),
                format: InputFormat::Text,
            },
        );
        assert!(res.ok, "{:?}", res.diagnostics);
        let id = res.system_id.unwrap();
        let run = |method: Method, mu: f64| {
            let mut req = simple_request(id, 2);
            req.params = vec![mu];
            req.solver = SolverOptions {
                method,
                h: 0.0,
                max_steps: 1_000_000,
                ..req.solver
            };
            req.trajectories = vec![TrajectoryRequest {
                x0: vec![2.0, 0.0],
                t0: 0.0,
                t1: 200.0,
            }];
            let out = compute_portrait(&store.get(id).unwrap(), &req).unwrap();
            out.trajectories.into_iter().next().unwrap()
        };
        let explicit = run(Method::Dopri5, 1000.0);
        assert_eq!(explicit.status, Status::Completed);
        assert_eq!(explicit.stiff, Some(true));
        let json = serde_json::to_value(&explicit).unwrap();
        assert_eq!(json["stiff"], serde_json::json!(true));

        let implicit = run(Method::Rosenbrock23, 1000.0);
        assert_eq!(implicit.status, Status::Completed);
        assert_eq!(implicit.stiff, None);
        assert!(
            implicit.stats.accepted * 10 <= explicit.stats.accepted,
            "{} vs {}",
            implicit.stats.accepted,
            explicit.stats.accepted
        );
        let (a, b) = (explicit.x.len() - 2, implicit.x.len() - 2);
        assert!(
            (explicit.x[a] - implicit.x[b]).abs() < 1e-3,
            "both end at the same point: {:?} vs {:?}",
            &explicit.x[a..],
            &implicit.x[b..]
        );

        let mild = run(Method::Dopri5, 1.0);
        assert_eq!(mild.stiff, None);
        assert!(serde_json::to_value(&mild).unwrap().get("stiff").is_none());
    }

    #[test]
    fn lyapunov_refuses_runs_it_cannot_measure() {
        let store = SystemStore::new();
        let id = compile_src(&store, "\\dot{x}=ax");
        let system = store.get(id).unwrap();
        let ok = LyapunovRequest {
            request_id: 7,
            system_id: id,
            params: vec![0.5],
            x0: vec![0.0],
            t0: 0.0,
            transient: 0.0,
            total: 10.0,
            interval: 0.1,
            blocks: 4,
            solver: SolverOptions {
                method: Method::Rk4,
                rtol: 1e-6,
                atol: 1e-9,
                h: 0.01,
                h_max: None,
                max_steps: 100_000,
            },
        };
        let out = compute_lyapunov(&system, &ok).unwrap();
        assert_eq!(out.request_id, 7);
        assert_eq!(out.status, Status::Completed);
        assert_eq!(out.renormalizations, 100);
        assert!((out.exponents[0] - 0.5).abs() < 1e-9, "{:?}", out.exponents);
        let refused = [
            LyapunovRequest {
                interval: 0.0,
                ..ok.clone()
            },
            LyapunovRequest {
                interval: -0.1,
                ..ok.clone()
            },
            LyapunovRequest {
                total: 0.0,
                ..ok.clone()
            },
            LyapunovRequest {
                blocks: 1,
                ..ok.clone()
            },
            LyapunovRequest {
                blocks: 101,
                ..ok.clone()
            },
            LyapunovRequest {
                total: 1e7,
                interval: 1e-3,
                ..ok.clone()
            },
            LyapunovRequest {
                transient: -1.0,
                ..ok.clone()
            },
            LyapunovRequest {
                x0: vec![0.0, 1.0],
                ..ok.clone()
            },
            LyapunovRequest {
                params: vec![],
                ..ok.clone()
            },
            LyapunovRequest {
                x0: vec![f64::NAN],
                ..ok.clone()
            },
        ];
        for req in &refused {
            assert!(
                compute_lyapunov(&system, req).is_err(),
                "{req:?} was accepted"
            );
        }
        let leapfrog = LyapunovRequest {
            solver: SolverOptions {
                method: Method::Verlet,
                ..ok.solver
            },
            ..ok.clone()
        };
        let henon = compile_src(&store, "x_{n+1}=1-ax_n^2+y_n\ny_{n+1}=bx_n");
        let henon = store.get(henon).unwrap();
        let map = LyapunovRequest {
            params: vec![1.4, 0.3],
            x0: vec![0.1, 0.1],
            transient: 1e3,
            total: 1e3,
            interval: 1.0,
            ..ok.clone()
        };
        assert!(compute_lyapunov(&henon, &map).is_ok());
        assert_eq!(
            compute_lyapunov(
                &henon,
                &LyapunovRequest {
                    transient: 1e12,
                    ..map.clone()
                }
            )
            .unwrap_err(),
            format!(
                "That is 1000000001000 iterates; at most {MAX_LYAPUNOV_ITERATES} fit in one run. \
                 Shorten the transient or the number of iterates."
            )
        );
        assert_eq!(
            compute_lyapunov(&system, &leapfrog).unwrap_err(),
            "A Lyapunov run integrates the variational equations, which Störmer–Verlet cannot \
             step; choose Runge–Kutta 4 or Dormand–Prince 5(4)."
        );
        let implicit = LyapunovRequest {
            solver: SolverOptions {
                method: Method::Rosenbrock23,
                ..ok.solver
            },
            ..ok.clone()
        };
        assert_eq!(
            compute_lyapunov(&system, &implicit).unwrap_err(),
            "A Lyapunov run integrates the variational equations, which have no exact Jacobian \
             for Rosenbrock 2(3) to use; choose Runge–Kutta 4 or Dormand–Prince 5(4)."
        );
        let at_the_limit = LyapunovRequest {
            total: 1e4,
            interval: 1e-3,
            ..ok.clone()
        };
        assert!(validate_lyapunov(&*system, &at_the_limit).is_ok());
    }

    fn continuation_request(system_id: u32, params: Vec<f64>, x0: Vec<f64>) -> ContinuationRequest {
        ContinuationRequest {
            request_id: 4,
            system_id,
            params,
            param: 0,
            range: (-1.0, 1.0),
            start: ContinuationStart::Point { x0, period: 1 },
            direction: ContinuationDirection::Forward,
            ds: 0.05,
            ds_max: 0.1,
            max_points: 500,
        }
    }

    #[test]
    fn continuation_refuses_what_it_cannot_follow() {
        let store = SystemStore::new();
        let pitchfork = compile_src(&store, "\\dot{x}=rx-x^3");
        let ok = continuation_request(pitchfork, vec![-1.0], vec![0.0]);
        let flow = store.get(pitchfork).unwrap();
        let out = compute_continuation(&flow, &ok).unwrap();
        assert_eq!(out.request_id, 4);
        assert_eq!(out.branch.special.len(), 1);
        let at_the_limit = ContinuationRequest {
            max_points: MAX_CONTINUATION_POINTS,
            ds_max: 0.05,
            ..ok.clone()
        };
        assert!(compute_continuation(&flow, &at_the_limit).is_ok());

        let point = |x0: Vec<f64>, period| ContinuationStart::Point { x0, period };
        let range = "The parameter range must be finite, with its start below its end.";
        let ds = "The step ds must be a positive number.";
        let points = "A branch needs 2 to 20000 points in each direction.";
        let refused = [
            (
                ContinuationRequest {
                    params: vec![],
                    ..ok.clone()
                },
                "Expected 1 parameter value(s) but got 0.",
            ),
            (
                ContinuationRequest {
                    param: 1,
                    ..ok.clone()
                },
                "Parameter 1 doesn't exist; the system has 1.",
            ),
            (
                ContinuationRequest {
                    range: (1.0, 1.0),
                    ..ok.clone()
                },
                range,
            ),
            (
                ContinuationRequest {
                    range: (1.0, -1.0),
                    ..ok.clone()
                },
                range,
            ),
            (
                ContinuationRequest {
                    range: (-1.0, f64::INFINITY),
                    ..ok.clone()
                },
                range,
            ),
            (
                ContinuationRequest {
                    range: (f64::NAN, 1.0),
                    ..ok.clone()
                },
                range,
            ),
            (
                ContinuationRequest {
                    start: point(vec![0.0, 0.0], 1),
                    ..ok.clone()
                },
                "The starting point has 2 coordinate(s) but the system has 1.",
            ),
            (
                ContinuationRequest {
                    start: point(vec![f64::NAN], 1),
                    ..ok.clone()
                },
                "The starting point and the parameters must be finite.",
            ),
            (
                ContinuationRequest {
                    params: vec![f64::INFINITY],
                    ..ok.clone()
                },
                "The starting point and the parameters must be finite.",
            ),
            (
                ContinuationRequest {
                    start: point(vec![0.0], 0),
                    ..ok.clone()
                },
                "The period must be at least 1.",
            ),
            (
                ContinuationRequest {
                    start: point(vec![0.0], 2),
                    ..ok.clone()
                },
                "A flow's equilibria have no period; use period 1.",
            ),
            (
                ContinuationRequest {
                    ds: 0.0,
                    ..ok.clone()
                },
                ds,
            ),
            (
                ContinuationRequest {
                    ds: -0.05,
                    ..ok.clone()
                },
                ds,
            ),
            (
                ContinuationRequest {
                    ds: f64::NAN,
                    ..ok.clone()
                },
                ds,
            ),
            (
                ContinuationRequest {
                    ds_max: 0.01,
                    ..ok.clone()
                },
                "The largest step must be a number at least ds.",
            ),
            (
                ContinuationRequest {
                    max_points: 1,
                    ..ok.clone()
                },
                points,
            ),
            (
                ContinuationRequest {
                    max_points: MAX_CONTINUATION_POINTS + 1,
                    ..ok.clone()
                },
                points,
            ),
            (
                ContinuationRequest {
                    params: vec![0.0],
                    ..ok.clone()
                },
                "The starting equilibrium is a fold or branch point, so the branch has no \
                 unique direction; start a little away from it.",
            ),
        ];
        for (req, message) in &refused {
            assert_eq!(
                compute_continuation(&flow, req).unwrap_err(),
                *message,
                "{req:?}"
            );
        }

        let saddle_node = compile_src(&store, "\\dot{x}=r+x^2");
        assert_eq!(
            compute_continuation(
                &store.get(saddle_node).unwrap(),
                &continuation_request(saddle_node, vec![1.0], vec![0.5]),
            )
            .unwrap_err(),
            "Newton's method found no equilibrium near the starting point; start closer to one."
        );
        let logistic = compile_src(&store, "x_{n+1}=rx_n\\left(1-x_n\\right)");
        let map = store.get(logistic).unwrap();
        assert_eq!(
            compute_continuation(&map, &continuation_request(logistic, vec![1.0], vec![0.0]),)
                .unwrap_err(),
            "The starting fixed point is a fold or branch point, so the branch has no unique \
             direction; start a little away from it."
        );
        assert!(compute_continuation(
            &map,
            &ContinuationRequest {
                start: point(vec![0.6], 2),
                ..continuation_request(logistic, vec![2.5], vec![])
            },
        )
        .is_ok());

        let constant = compile_src(&store, "\\dot{x}=-x");
        assert_eq!(
            compute_continuation(
                &store.get(constant).unwrap(),
                &continuation_request(constant, vec![], vec![0.0]),
            )
            .unwrap_err(),
            "This system has no parameter to continue in."
        );
        let driven = compile_src(&store, "\\dot{x}=-x+at");
        assert_eq!(
            compute_continuation(
                &store.get(driven).unwrap(),
                &continuation_request(driven, vec![0.5], vec![0.0]),
            )
            .unwrap_err(),
            "Continuation follows fixed points, which a system that depends on t or n doesn't \
             have."
        );
    }

    #[test]
    fn switch_start_follows_the_second_branch_from_a_saved_seed() {
        let store = SystemStore::new();
        let pitchfork = compile_src(&store, "\\dot{x}=rx-x^3");
        let flow = store.get(pitchfork).unwrap();
        let parent = continuation_request(pitchfork, vec![-1.0], vec![0.0]);
        let bp = compute_continuation(&flow, &parent).unwrap().branch.special[0].clone();
        assert_eq!(bp.kind, SpecialKindOut::BranchPoint);
        let switch = ContinuationRequest {
            start: ContinuationStart::Switch { at: bp.clone() },
            direction: ContinuationDirection::Both,
            ..parent.clone()
        };
        let saved = serde_json::to_string(&switch).unwrap();
        assert!(saved.contains(r#""start":{"kind":"switch","at":{"kind":"branchPoint""#));
        let replayed: ContinuationRequest = serde_json::from_str(&saved).unwrap();
        assert_eq!(replayed, switch);
        let b = compute_continuation(&flow, &replayed).unwrap().branch;
        assert_eq!(b, compute_continuation(&flow, &switch).unwrap().branch);

        let middle = b.p.iter().position(|r| *r == bp.p).expect("the BP once");
        assert_eq!(b.x[middle], bp.x[0]);
        assert!(b.x[..middle].iter().all(|x| *x < 0.0));
        assert!(b.x[middle + 1..].iter().all(|x| *x > 0.0));
        for (x, r) in b.x.iter().zip(&b.p) {
            assert!((r - x * x).abs() <= 1e-10, "r − x² at x = {x}");
        }
        assert_eq!((b.p[0], *b.p.last().unwrap()), (1.0, 1.0));
        assert_eq!(
            b.end,
            BranchEndOut {
                first: Some(BranchStop::Range),
                last: BranchStop::Range,
            }
        );

        let switch_at = |kind, tangent: Vec<f64>, p: f64| ContinuationRequest {
            start: ContinuationStart::Switch {
                at: SpecialPointOut {
                    kind,
                    tangent,
                    p,
                    ..bp.clone()
                },
            },
            ..parent.clone()
        };
        let refused = [
            (
                switch_at(SpecialKindOut::Hopf, bp.tangent.clone(), 0.0),
                "Branches can be switched only at a branch point (BP), or at a flip (PD) of a map.",
            ),
            (
                switch_at(SpecialKindOut::Flip, bp.tangent.clone(), 0.0),
                "Branches can be switched only at a branch point (BP), or at a flip (PD) of a map.",
            ),
            (
                switch_at(SpecialKindOut::BranchPoint, vec![1.0], 0.0),
                "The switch point's tangent needs 2 finite components, not all zero.",
            ),
            (
                switch_at(SpecialKindOut::BranchPoint, vec![0.0, 0.0], 0.0),
                "The switch point's tangent needs 2 finite components, not all zero.",
            ),
            (
                switch_at(SpecialKindOut::BranchPoint, vec![0.0, f64::NAN], 0.0),
                "The switch point's tangent needs 2 finite components, not all zero.",
            ),
            (
                switch_at(SpecialKindOut::BranchPoint, bp.tangent.clone(), f64::NAN),
                "The switch point's parameter must be finite.",
            ),
        ];
        for (req, message) in &refused {
            assert_eq!(
                compute_continuation(&flow, req).unwrap_err(),
                *message,
                "{req:?}"
            );
        }
    }

    #[test]
    fn both_directions_join_into_one_polyline() {
        let store = SystemStore::new();
        let henon = compile_src(&store, "x_{n+1}=1-ax_n^2+y_n\ny_{n+1}=bx_n");
        let map = store.get(henon).unwrap();
        let run = |direction| {
            let req = ContinuationRequest {
                direction,
                range: (-0.5, 1.0),
                ..continuation_request(henon, vec![0.2, 0.3], vec![1.09, 0.327])
            };
            compute_continuation(&map, &req).unwrap().branch
        };
        let back = run(ContinuationDirection::Backward);
        let fwd = run(ContinuationDirection::Forward);
        let both = run(ContinuationDirection::Both);
        let nb = back.p.len();

        let mut want_p: Vec<f64> = back.p.iter().rev().copied().collect();
        want_p.extend(&fwd.p[1..]);
        assert_eq!(both.p, want_p);
        assert_eq!(both.x.len(), 2 * both.p.len());
        assert_eq!(both.x[2 * (nb - 1)..2 * nb], fwd.x[..2]);
        assert_eq!(both.stable[nb - 1..], fwd.stable[..]);
        assert_eq!(both.unstable[nb - 1..], fwd.unstable[..]);
        assert!(back.p[1] < back.p[0] && fwd.p[1] > fwd.p[0]);

        let kinds: Vec<_> = both.special.iter().map(|sp| sp.kind).collect();
        assert_eq!(kinds, [SpecialKindOut::Fold, SpecialKindOut::Flip]);
        for sp in &both.special {
            assert_eq!(both.p[sp.index], sp.p);
            assert_eq!(both.x[2 * sp.index..2 * sp.index + 2], sp.x[..]);
        }
        let (lp, lp_back) = (&both.special[0], &back.special[0]);
        assert_eq!(lp.index, nb - 1 - lp_back.index);
        let negated: Vec<f64> = lp_back.tangent.iter().map(|t| -t).collect();
        assert_eq!(lp.tangent, negated);
        assert_eq!(
            both.special[1],
            SpecialPointOut {
                index: fwd.special[0].index + nb - 1,
                ..fwd.special[0].clone()
            }
        );

        assert_eq!(fwd.end.first, None);
        assert_eq!(back.end.first, None);
        assert_eq!(
            both.end,
            BranchEndOut {
                first: Some(back.end.last),
                last: fwd.end.last,
            }
        );
    }
}
