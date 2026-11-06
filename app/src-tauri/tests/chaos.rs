//! Published values: Lorenz and Hénon exponents from Sprott, *Chaos and
//! Time-Series Analysis*, Oxford 2003, appendix A; the logistic map's
//! λ = ln 2 at r = 4 (Strogatz §10.5) and its doublings r1 = 3, r2 = 1 + √6
//! (Strogatz §10.3); Strogatz Table 10.2.1 gives r3 and r4 to fewer digits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use dyn_expr::CompiledSystem;
use separatrix_lib::ipc::{
    AxisRef, CompileRequest, InputFormat, LyapunovRequest, LyapunovResponse, Method,
    OrbitDiagramRequest, OrbitDiagramResponse, PortraitRequest, Severity, SolverOptions, Status,
    TrajectoryOut, TrajectoryRequest,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

const BLOCKS: usize = 10;

const fn rk4(h: f64) -> SolverOptions {
    SolverOptions {
        method: Method::Rk4,
        rtol: 1e-6,
        atol: 1e-9,
        h,
        h_max: None,
        max_steps: 2_000_000,
    }
}

struct Run {
    label: &'static str,
    template: &'static str,
    params: &'static [(&'static str, f64)],
    x0: &'static [f64],
    transient: f64,
    total: f64,
    interval: f64,
    solver: SolverOptions,
}

const LORENZ: Run = Run {
    label: "Lorenz, RK4 h = 0.01, T = 1e4, τ = 0.1",
    template: "lorenz",
    params: &[("sigma", 10.0), ("r", 28.0), ("b", 8.0 / 3.0)],
    x0: &[0.0, 1.0, 0.0],
    transient: 100.0,
    total: 1e4,
    interval: 0.1,
    solver: rk4(0.01),
};

const LORENZ_FINE: Run = Run {
    label: "Lorenz, RK4 h = 0.001, T = 1e3, τ = 0.1",
    total: 1e3,
    solver: rk4(0.001),
    ..LORENZ
};

const HENON: Run = Run {
    label: "Hénon, N = 1e6",
    template: "henon-map",
    params: &[("a", 1.4), ("b", 0.3)],
    x0: &[0.0, 0.0],
    transient: 1e3,
    total: 1e6,
    interval: 1.0,
    solver: rk4(0.01),
};

const LOGISTIC: Run = Run {
    label: "logistic r = 4, N = 1e6",
    template: "logistic-map",
    params: &[("r", 4.0)],
    x0: &[0.1],
    transient: 0.0,
    total: 1e6,
    interval: 1.0,
    solver: rk4(0.01),
};

struct Diagram {
    label: &'static str,
    template: &'static str,
    param: &'static str,
    range: (f64, f64),
    columns: usize,
    x0: &'static [f64],
    transient: usize,
    kept: usize,
}

const LOGISTIC_DIAGRAM: Diagram = Diagram {
    label: "logistic orbit diagram, r ∈ [2.8, 3.57], 2000 columns",
    template: "logistic-map",
    param: "r",
    range: (2.8, 3.57),
    columns: 2000,
    x0: &[0.1],
    transient: 2000,
    kept: 64,
};

enum Claim {
    Exponent {
        run: &'static Run,
        index: usize,
    },
    Sum {
        run: &'static Run,
    },
    Doubling {
        k: usize,
    },
    EnergyError {
        run: &'static EnergyRun,
    },
    EnergyGrowth {
        run: &'static EnergyRun,
    },
    StiffEnd {
        run: &'static StiffRun,
        index: usize,
    },
    StepRatio {
        implicit: &'static StiffRun,
        explicit: &'static StiffRun,
    },
    Steps {
        run: &'static StiffRun,
    },
    StiffHint {
        run: &'static StiffRun,
    },
}

enum Expect {
    Near { value: f64, tol: f64 },
    AtMost { bound: f64 },
    Relative { value: f64, tol: f64 },
    Yes,
}

struct Check {
    claim: Claim,
    text: &'static str,
    expect: Expect,
}

struct EnergyRun {
    label: &'static str,
    method: Method,
    h: f64,
}

const PERIODS: f64 = 1e5;
const WINDOW_PERIODS: f64 = 1e3;

const VERLET: EnergyRun = EnergyRun {
    label: "pendulum, Störmer–Verlet h = 0.05, 1e5 periods",
    method: Method::Verlet,
    h: 0.05,
};

const YOSHIDA: EnergyRun = EnergyRun {
    label: "pendulum, Yoshida 4 h = 0.1, 1e5 periods",
    method: Method::Yoshida4,
    h: 0.1,
};

const PENDULUM_RK4: EnergyRun = EnergyRun {
    label: "pendulum, RK4 h = 0.1, 1e5 periods",
    method: Method::Rk4,
    h: 0.1,
};

enum Source {
    Template {
        id: &'static str,
        params: &'static [(&'static str, f64)],
    },
    Text(&'static str),
}

struct StiffRun {
    label: &'static str,
    source: Source,
    x0: &'static [f64],
    t1: f64,
    method: Method,
    rtol: f64,
    atol: f64,
}

const ROBERTSON_SOURCE: &str =
    "x' = -0.04*x + 1e4*y*z\ny' = 0.04*x - 1e4*y*z - 3e7*y^2\nz' = 3e7*y^2";
/// Robertson's reaction (Hairer & Wanner, *Solving ODEs II*, §IV.1) at
/// t = 40: SUNDIALS `cvRoberts_dns`.
const ROBERTSON_40: [f64; 3] = [7.158_271e-1, 9.185_535e-6, 2.841_637e-1];

const ROBERTSON: StiffRun = StiffRun {
    label: "Robertson, Rosenbrock 2(3) rtol 1e-8 atol 1e-14, t = 40",
    source: Source::Text(ROBERTSON_SOURCE),
    x0: &[1.0, 0.0, 0.0],
    t1: 40.0,
    method: Method::Rosenbrock23,
    rtol: 1e-8,
    atol: 1e-14,
};

const ROBERTSON_REFERENCE: StiffRun = StiffRun {
    label: "Robertson, Dormand–Prince rtol 1e-11 atol 1e-14, t = 40",
    method: Method::Dopri5,
    rtol: 1e-11,
    ..ROBERTSON
};

const ROBERTSON_DOPRI5: StiffRun = StiffRun {
    label: "Robertson, Dormand–Prince rtol 1e-8 atol 1e-14, t = 40",
    method: Method::Dopri5,
    ..ROBERTSON
};

const RELAXATION_DOPRI5: StiffRun = StiffRun {
    label: "relaxation oscillator μ = 1000, Dormand–Prince rtol 1e-6, t = 3000",
    source: Source::Template {
        id: "relaxation-oscillator",
        params: &[("mu", 1000.0)],
    },
    x0: &[2.0, 0.0],
    t1: 3000.0,
    method: Method::Dopri5,
    rtol: 1e-6,
    atol: 1e-9,
};

const RELAXATION_ROSENBROCK: StiffRun = StiffRun {
    label: "relaxation oscillator μ = 1000, Rosenbrock 2(3) rtol 1e-6, t = 3000",
    method: Method::Rosenbrock23,
    ..RELAXATION_DOPRI5
};

const STIFF_MAX_STEPS: usize = 100_000_000;

const LN_0_3: f64 = -1.203_972_804_325_936_1;
const LN_2: f64 = std::f64::consts::LN_2;
const ONE_PLUS_SQRT_6: f64 = 3.449_489_742_783_178;
const FEIGENBAUM_DELTA: f64 = 4.669_201_609_102_99;

const CHECKS: &[Check] = &[
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 0,
        },
        text: "Lorenz λ1",
        expect: Expect::Near {
            value: 0.9056,
            tol: 0.01,
        },
    },
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 1,
        },
        text: "Lorenz λ2",
        expect: Expect::Near {
            value: 0.0,
            tol: 0.01,
        },
    },
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 2,
        },
        text: "Lorenz λ3",
        expect: Expect::Near {
            value: -14.5723,
            tol: 0.02,
        },
    },
    Check {
        claim: Claim::Sum { run: &LORENZ_FINE },
        text: "Lorenz Σλ = −(σ + 1 + b) = −41/3",
        expect: Expect::Near {
            value: -41.0 / 3.0,
            tol: 1e-6,
        },
    },
    Check {
        claim: Claim::Exponent {
            run: &HENON,
            index: 0,
        },
        text: "Hénon λ1",
        expect: Expect::Near {
            value: 0.41922,
            tol: 0.005,
        },
    },
    Check {
        claim: Claim::Exponent {
            run: &HENON,
            index: 1,
        },
        text: "Hénon λ2",
        expect: Expect::Near {
            value: -1.62319,
            tol: 0.005,
        },
    },
    Check {
        claim: Claim::Sum { run: &HENON },
        text: "Hénon Σλ = ln 0.3",
        expect: Expect::Near {
            value: LN_0_3,
            tol: 1e-9,
        },
    },
    Check {
        claim: Claim::Exponent {
            run: &LOGISTIC,
            index: 0,
        },
        text: "logistic r = 4 λ = ln 2",
        expect: Expect::Near {
            value: LN_2,
            tol: 5e-3,
        },
    },
    Check {
        claim: Claim::Doubling { k: 1 },
        text: "logistic r1 = 3",
        expect: Expect::Near {
            value: 3.0,
            tol: 1e-6,
        },
    },
    Check {
        claim: Claim::Doubling { k: 2 },
        text: "logistic r2 = 1 + √6",
        expect: Expect::Near {
            value: ONE_PLUS_SQRT_6,
            tol: 1e-6,
        },
    },
    Check {
        claim: Claim::Doubling { k: 3 },
        text: "logistic r3",
        expect: Expect::Near {
            value: 3.544_090_3,
            tol: 1e-6,
        },
    },
    Check {
        claim: Claim::Doubling { k: 4 },
        text: "logistic r4",
        expect: Expect::Near {
            value: 3.564_407_3,
            tol: 1e-6,
        },
    },
    Check {
        claim: Claim::EnergyError { run: &VERLET },
        text: "Verlet h = 0.05 max |ΔH|",
        expect: Expect::AtMost { bound: 1e-3 },
    },
    Check {
        claim: Claim::EnergyGrowth { run: &VERLET },
        text: "Verlet last/first 1e3 periods",
        expect: Expect::AtMost { bound: 2.0 },
    },
    Check {
        claim: Claim::EnergyError { run: &YOSHIDA },
        text: "Yoshida 4 h = 0.1 max |ΔH|",
        expect: Expect::AtMost { bound: 1e-4 },
    },
    Check {
        claim: Claim::EnergyGrowth { run: &YOSHIDA },
        text: "Yoshida 4 last/first 1e3 periods",
        expect: Expect::AtMost { bound: 2.0 },
    },
    Check {
        claim: Claim::StiffEnd {
            run: &ROBERTSON_REFERENCE,
            index: 0,
        },
        text: "Robertson y1(40), Dopri5 1e-11",
        expect: Expect::Relative {
            value: ROBERTSON_40[0],
            tol: 1e-5,
        },
    },
    Check {
        claim: Claim::StiffEnd {
            run: &ROBERTSON_REFERENCE,
            index: 1,
        },
        text: "Robertson y2(40), Dopri5 1e-11",
        expect: Expect::Relative {
            value: ROBERTSON_40[1],
            tol: 1e-5,
        },
    },
    Check {
        claim: Claim::StiffEnd {
            run: &ROBERTSON_REFERENCE,
            index: 2,
        },
        text: "Robertson y3(40), Dopri5 1e-11",
        expect: Expect::Relative {
            value: ROBERTSON_40[2],
            tol: 1e-5,
        },
    },
    Check {
        claim: Claim::StiffEnd {
            run: &ROBERTSON,
            index: 0,
        },
        text: "Robertson y1(40), Rosenbrock 2(3)",
        expect: Expect::Relative {
            value: ROBERTSON_40[0],
            tol: 1e-5,
        },
    },
    Check {
        claim: Claim::StiffEnd {
            run: &ROBERTSON,
            index: 1,
        },
        text: "Robertson y2(40), Rosenbrock 2(3)",
        expect: Expect::Relative {
            value: ROBERTSON_40[1],
            tol: 1e-5,
        },
    },
    Check {
        claim: Claim::StiffEnd {
            run: &ROBERTSON,
            index: 2,
        },
        text: "Robertson y3(40), Rosenbrock 2(3)",
        expect: Expect::Relative {
            value: ROBERTSON_40[2],
            tol: 1e-5,
        },
    },
    Check {
        claim: Claim::StiffHint {
            run: &ROBERTSON_DOPRI5,
        },
        text: "Robertson, Dopri5 1e-8 looks stiff",
        expect: Expect::Yes,
    },
    Check {
        claim: Claim::StepRatio {
            implicit: &RELAXATION_ROSENBROCK,
            explicit: &RELAXATION_DOPRI5,
        },
        text: "VdP μ = 1000 steps Ros23/Dopri5",
        expect: Expect::AtMost { bound: 0.1 },
    },
    Check {
        claim: Claim::StiffHint {
            run: &RELAXATION_DOPRI5,
        },
        text: "VdP μ = 1000, Dopri5 looks stiff",
        expect: Expect::Yes,
    },
];

const WRONG_CHECKS: &[Check] = &[
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 0,
        },
        text: "wrong: Lorenz λ1 = 1.0",
        expect: Expect::Near {
            value: 1.0,
            tol: 0.01,
        },
    },
    Check {
        claim: Claim::Sum { run: &LORENZ_FINE },
        text: "wrong: Lorenz Σλ = −13",
        expect: Expect::Near {
            value: -13.0,
            tol: 1e-6,
        },
    },
    Check {
        claim: Claim::Exponent {
            run: &HENON,
            index: 0,
        },
        text: "wrong: Hénon λ1 = 0.5",
        expect: Expect::Near {
            value: 0.5,
            tol: 0.005,
        },
    },
    Check {
        claim: Claim::Doubling { k: 2 },
        text: "wrong: logistic r2 = 3.45",
        expect: Expect::Near {
            value: 3.45,
            tol: 1e-6,
        },
    },
    Check {
        claim: Claim::EnergyGrowth { run: &PENDULUM_RK4 },
        text: "wrong: RK4 h = 0.1 stays bounded",
        expect: Expect::AtMost { bound: 2.0 },
    },
    Check {
        claim: Claim::Steps {
            run: &ROBERTSON_DOPRI5,
        },
        text: "wrong: Dopri5 1e-8 within 2,000 steps",
        expect: Expect::AtMost { bound: 2000.0 },
    },
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    id: String,
    format: InputFormat,
    equations: Vec<String>,
}

fn store() -> &'static SystemStore {
    static STORE: OnceLock<SystemStore> = OnceLock::new();
    STORE.get_or_init(SystemStore::new)
}

fn compile(
    label: &str,
    template: &str,
    values: &[(&str, f64)],
) -> (u32, Arc<CompiledSystem>, Vec<f64>, Vec<String>) {
    let templates: Vec<Template> = serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
    let found = templates
        .iter()
        .find(|t| t.id == template)
        .unwrap_or_else(|| panic!("no template {template:?}"));
    compile_source(label, &found.equations.join("\n"), found.format, values)
}

fn compile_source(
    label: &str,
    source: &str,
    format: InputFormat,
    values: &[(&str, f64)],
) -> (u32, Arc<CompiledSystem>, Vec<f64>, Vec<String>) {
    let res = service::compile_system(
        store(),
        &CompileRequest {
            source: source.into(),
            format,
        },
    );
    assert!(
        res.ok
            && !res
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
        "{label} doesn't compile: {:?}",
        res.diagnostics
    );
    let params = res
        .params
        .iter()
        .map(|p| {
            values
                .iter()
                .find(|(name, _)| *name == p.name)
                .map(|&(_, v)| v)
                .unwrap_or_else(|| panic!("{label}: no value for {}", p.name))
        })
        .collect();
    let names = res.params.iter().map(|p| p.name.clone()).collect();
    let id = res.system_id.expect("compiled");
    (id, store().get(id).expect("stored"), params, names)
}

fn spectrum(run: &'static Run) -> Arc<LyapunovResponse> {
    static RUNS: OnceLock<Mutex<HashMap<&'static str, Arc<LyapunovResponse>>>> = OnceLock::new();
    let runs = RUNS.get_or_init(Default::default);
    if let Some(done) = runs.lock().unwrap().get(run.label) {
        return done.clone();
    }
    let (system_id, system, params, _) = compile(run.label, run.template, run.params);
    let req = LyapunovRequest {
        request_id: 1,
        system_id,
        params,
        x0: run.x0.to_vec(),
        t0: 0.0,
        transient: run.transient,
        total: run.total,
        interval: run.interval,
        blocks: BLOCKS,
        solver: run.solver,
    };
    let out = Arc::new(
        service::compute_lyapunov(&system, &req).unwrap_or_else(|e| panic!("{}: {e}", run.label)),
    );
    println!(
        "{}: {} renormalizations, {:?}, {:.0} ms",
        run.label, out.renormalizations, out.status, out.elapsed_ms
    );
    runs.lock().unwrap().insert(run.label, out.clone());
    out
}

fn logistic_diagram() -> &'static OrbitDiagramResponse {
    static DIAGRAM: OnceLock<OrbitDiagramResponse> = OnceLock::new();
    DIAGRAM.get_or_init(|| {
        let d = &LOGISTIC_DIAGRAM;
        let (system_id, system, params, names) =
            compile(d.label, d.template, &[(d.param, d.range.0)]);
        let param = names
            .iter()
            .position(|name| name == d.param)
            .expect("the swept parameter");
        let req = OrbitDiagramRequest {
            request_id: 1,
            system_id,
            params,
            param,
            range: d.range,
            columns: d.columns,
            state: 0,
            x0: d.x0.to_vec(),
            transient: d.transient,
            kept: d.kept,
            doublings: true,
        };
        let out = service::compute_orbit_diagram(&system, &req)
            .unwrap_or_else(|e| panic!("{}: {e}", d.label));
        println!(
            "{}: {} doublings, {:.0} ms",
            d.label,
            out.doublings.as_ref().map_or(0, Vec::len),
            out.elapsed_ms
        );
        out
    })
}

fn doubling(k: usize) -> f64 {
    logistic_diagram()
        .doublings
        .as_ref()
        .and_then(|found| found.get(k - 1))
        .map_or(f64::NAN, |d| d.r)
}

fn pendulum_period(q0: f64) -> f64 {
    let (mut a, mut b) = (1.0f64, (q0 / 2.0).cos());
    while (a - b).abs() > 1e-15 {
        (a, b) = ((a + b) / 2.0, (a * b).sqrt());
    }
    2.0 * std::f64::consts::PI / a
}

struct EnergyErrors {
    all: f64,
    first: f64,
    last: f64,
    completed: bool,
}

fn energy_errors(run: &'static EnergyRun) -> Arc<EnergyErrors> {
    static RUNS: OnceLock<Mutex<HashMap<&'static str, Arc<EnergyErrors>>>> = OnceLock::new();
    let runs = RUNS.get_or_init(Default::default);
    if let Some(done) = runs.lock().unwrap().get(run.label) {
        return done.clone();
    }
    let started = Instant::now();
    let (_, system, params, _) = compile(run.label, "hamiltonian-pendulum", &[]);
    let x0 = [1.0, 0.0];
    let period = pendulum_period(x0[0]);
    let (t_end, window) = (PERIODS * period, WINDOW_PERIODS * period);
    let h0 = system.hamiltonian_value(0.0, &x0, &params).expect("H");
    let opts = dyn_solve::SolverOptions {
        method: run.method,
        h: run.h,
        max_steps: usize::MAX,
        ..dyn_solve::SolverOptions::default()
    };
    let mut stepper = dyn_solve::Stepper::new(&*system, &params, 0.0, &x0, t_end, &opts);
    let mut e = EnergyErrors {
        all: 0.0,
        first: 0.0,
        last: 0.0,
        completed: false,
    };
    loop {
        match stepper.step() {
            dyn_solve::StepOutcome::Accepted => {}
            dyn_solve::StepOutcome::AtEnd => {
                e.completed = true;
                break;
            }
            _ => break,
        }
        let t = stepper.t();
        let drift = (system
            .hamiltonian_value(t, stepper.x(), &params)
            .expect("H")
            - h0)
            .abs();
        e.all = e.all.max(drift);
        if t <= window {
            e.first = e.first.max(drift);
        }
        if t >= t_end - window {
            e.last = e.last.max(drift);
        }
    }
    println!(
        "{}: period {period:.6}, {} steps, {} evaluations, {:.0} ms",
        run.label,
        stepper.stats().accepted,
        stepper.stats().evals,
        started.elapsed().as_secs_f64() * 1e3
    );
    let e = Arc::new(e);
    runs.lock().unwrap().insert(run.label, e.clone());
    e
}

fn stiff_run(run: &'static StiffRun) -> Arc<TrajectoryOut> {
    static RUNS: OnceLock<Mutex<HashMap<&'static str, Arc<TrajectoryOut>>>> = OnceLock::new();
    let runs = RUNS.get_or_init(Default::default);
    if let Some(done) = runs.lock().unwrap().get(run.label) {
        return done.clone();
    }
    let (system_id, system, params, _) = match run.source {
        Source::Template { id, params } => compile(run.label, id, params),
        Source::Text(src) => compile_source(run.label, src, InputFormat::Text, &[]),
    };
    let dim = run.x0.len();
    let req = PortraitRequest {
        request_id: 1,
        system_id,
        params,
        x_axis: AxisRef::State { index: 0 },
        y_axis: AxisRef::State { index: 1 },
        x_range: (-1.0, 1.0),
        y_range: (-1.0, 1.0),
        base_state: vec![0.0; dim],
        field_time: 0.0,
        field_grid: 0,
        nullclines: false,
        fixed_points: false,
        trajectories: vec![TrajectoryRequest {
            x0: run.x0.to_vec(),
            t0: 0.0,
            t1: run.t1,
        }],
        solver: SolverOptions {
            method: run.method,
            rtol: run.rtol,
            atol: run.atol,
            h: 0.0,
            h_max: None,
            max_steps: STIFF_MAX_STEPS,
        },
        samples: 2,
        graph_samples: None,
        transient: None,
        section: None,
        z_axis: None,
        z_range: None,
    };
    let out =
        service::compute_portrait(&system, &req).unwrap_or_else(|e| panic!("{}: {e}", run.label));
    let tr = out.trajectories.into_iter().next().expect("one trajectory");
    let end: Vec<String> = tr.x[tr.x.len() - dim..]
        .iter()
        .map(|v| format!("{v:.9e}"))
        .collect();
    let end = end.join(", ");
    println!(
        "{}: {:?} at t = {}, {} steps ({} rejected), stiff {:?}, x = ({end}), {:.0} ms",
        run.label,
        tr.status,
        tr.t.last().copied().unwrap_or(f64::NAN),
        tr.stats.accepted,
        tr.stats.rejected,
        tr.stiff,
        out.elapsed_ms
    );
    let tr = Arc::new(tr);
    runs.lock().unwrap().insert(run.label, tr.clone());
    tr
}

fn completed_at_end(run: &'static StiffRun, tr: &TrajectoryOut) -> bool {
    tr.status == Status::Completed && tr.t.last() == Some(&run.t1)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Verdict {
    Pass,
    Fail,
}

struct Row {
    claim: &'static str,
    expect: &'static Expect,
    measured: f64,
    uncertainty: Option<f64>,
    verdict: Verdict,
}

impl Row {
    fn delta(&self) -> Option<f64> {
        match *self.expect {
            Expect::Near { value, .. } => Some((self.measured - value).abs()),
            Expect::Relative { value, .. } => Some((self.measured - value).abs() / value.abs()),
            Expect::AtMost { .. } | Expect::Yes => None,
        }
    }
}

fn evaluate(check: &'static Check) -> Row {
    let (measured, uncertainty, completed) = match check.claim {
        Claim::Exponent { run, index } => {
            let s = spectrum(run);
            (
                s.exponents.get(index).copied().unwrap_or(f64::NAN),
                s.uncertainty.get(index).copied(),
                s.status == Status::Completed,
            )
        }
        Claim::Sum { run } => {
            let s = spectrum(run);
            (s.sum, None, s.status == Status::Completed)
        }
        Claim::Doubling { k } => (doubling(k), None, true),
        Claim::EnergyError { run } => {
            let e = energy_errors(run);
            (e.all, None, e.completed)
        }
        Claim::EnergyGrowth { run } => {
            let e = energy_errors(run);
            (e.last / e.first, None, e.completed)
        }
        Claim::StiffEnd { run, index } => {
            let tr = stiff_run(run);
            let at = tr.x.len() - run.x0.len() + index;
            (tr.x[at], None, completed_at_end(run, &tr))
        }
        Claim::StepRatio { implicit, explicit } => {
            let (i, e) = (stiff_run(implicit), stiff_run(explicit));
            (
                i.stats.accepted as f64 / e.stats.accepted as f64,
                None,
                completed_at_end(implicit, &i) && completed_at_end(explicit, &e),
            )
        }
        Claim::Steps { run } => {
            let tr = stiff_run(run);
            (tr.stats.accepted as f64, None, completed_at_end(run, &tr))
        }
        Claim::StiffHint { run } => {
            let tr = stiff_run(run);
            let flagged = if tr.stiff == Some(true) { 1.0 } else { 0.0 };
            (flagged, None, completed_at_end(run, &tr))
        }
    };
    let holds = match check.expect {
        Expect::Near { value, tol } => (measured - value).abs() <= tol,
        Expect::AtMost { bound } => measured <= bound,
        Expect::Relative { value, tol } => (measured - value).abs() <= tol * value.abs(),
        Expect::Yes => measured == 1.0,
    };
    Row {
        claim: check.text,
        expect: &check.expect,
        measured,
        uncertainty,
        verdict: if completed && holds {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    }
}

fn print_line([claim, published, measured, pm, delta, tol, verdict, within]: [&str; 8]) {
    println!(
        "{claim:<36} {published:>13} {measured:>15} {pm:>9} {delta:>8} {tol:>6} {verdict:<6} {within}"
    );
}

fn print_table(rows: &[Row]) {
    println!();
    print_line([
        "claim",
        "published",
        "measured",
        "±",
        "delta",
        "tol",
        "result",
        "within 3·± (info)",
    ]);
    for r in rows {
        let (pm, within) = match (r.uncertainty, r.delta()) {
            (Some(u), Some(delta)) => (
                format!("{u:.1e}"),
                if delta <= 3.0 * u { "yes" } else { "no" }.to_string(),
            ),
            _ => ("—".to_string(), "—".to_string()),
        };
        let (published, measured, tol) = match *r.expect {
            Expect::Near { value, tol } => (
                format!("{value:.7}"),
                format!("{:.9}", r.measured),
                format!("{tol:.0e}"),
            ),
            Expect::Relative { value, tol } => (
                format!("{value:.6e}"),
                format!("{:.9e}", r.measured),
                format!("{tol:.0e} rel"),
            ),
            Expect::Yes => (
                "yes".to_string(),
                if r.measured == 1.0 { "yes" } else { "no" }.to_string(),
                "—".to_string(),
            ),
            Expect::AtMost { bound } if bound >= 1000.0 => (
                "—".to_string(),
                format!("{}", r.measured),
                format!("≤ {bound}"),
            ),
            Expect::AtMost { bound } if bound >= 1.0 => (
                "—".to_string(),
                format!("{:.4}", r.measured),
                format!("≤ {bound}"),
            ),
            Expect::AtMost { bound } => (
                "—".to_string(),
                format!("{:.3e}", r.measured),
                format!("≤ {bound:.0e}"),
            ),
        };
        print_line([
            r.claim,
            &published,
            &measured,
            &pm,
            &r.delta().map_or("—".to_string(), |d| format!("{d:.1e}")),
            &tol,
            match r.verdict {
                Verdict::Pass => "PASS",
                Verdict::Fail => "FAIL",
            },
            &within,
        ]);
    }
}

#[test]
fn chaos_claims_reproduce() {
    let started = Instant::now();
    let rows: Vec<Row> = CHECKS.iter().map(evaluate).collect();
    print_table(&rows);
    let ratio = (doubling(3) - doubling(2)) / (doubling(4) - doubling(3));
    print_line([
        "Feigenbaum δ ≈ (r3 − r2)/(r4 − r3)",
        &format!("{FEIGENBAUM_DELTA:.7}"),
        &format!("{ratio:.9}"),
        "—",
        &format!("{:.1e}", (ratio - FEIGENBAUM_DELTA).abs()),
        "—",
        "info",
        "not gated",
    ]);
    println!(
        "\n{} rows, wall time {:.1} s (budget 30 s)",
        rows.len(),
        started.elapsed().as_secs_f64()
    );
    let failed: Vec<&str> = rows
        .iter()
        .filter(|r| r.verdict == Verdict::Fail)
        .map(|r| r.claim)
        .collect();
    assert!(failed.is_empty(), "failed claims: {failed:?}");
}

#[test]
fn wrong_claims_fail() {
    let rows: Vec<Row> = WRONG_CHECKS.iter().map(evaluate).collect();
    print_table(&rows);
    let passed: Vec<&str> = rows
        .iter()
        .filter(|r| r.verdict != Verdict::Fail)
        .map(|r| r.claim)
        .collect();
    assert!(passed.is_empty(), "wrong claims not failed: {passed:?}");
}

#[test]
fn one_plus_sqrt_6_is_written_out_exactly() {
    assert_eq!(ONE_PLUS_SQRT_6, 1.0 + 6f64.sqrt());
}
