//! The chaos gate: the gallery's Lorenz, Hénon and logistic templates,
//! compiled from source and measured through `service::compute_lyapunov`
//! (the path the UI calls), reproduce published Lyapunov exponents.
//!
//! - Lorenz (σ = 10, r = 28, b = 8/3): λ ≈ 0.9056, 0, −14.5723 (Sprott,
//!   *Chaos and Time-Series Analysis*, Oxford 2003, appendix A), and
//!   Σλ = −(σ + 1 + b) = −41/3 exactly, since `tr J` is constant.
//! - Hénon (a = 1.4, b = 0.3): λ ≈ 0.41922, −1.62319 (Sprott, appendix A),
//!   and Σλ = ln |det J| = ln 0.3 exactly.
//! - Logistic map at r = 4: λ = ln 2 (Strogatz, §10.5).
//!
//! The Σ row of Lorenz uses its own run with RK4 at h = 0.001: RK4's
//! truncation error on `ln det Φ` puts Σλ about 1e−4 from −41/3 at
//! h = 0.01, shrinking like h⁴ (see `crates/dyn-analysis/tests/lyapunov.rs`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use dyn_expr::CompiledSystem;
use separatrix_lib::ipc::{
    CompileRequest, InputFormat, LyapunovRequest, LyapunovResponse, Method, Severity,
    SolverOptions, Status,
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

enum Claim {
    Exponent { index: usize },
    Sum,
}

struct Check {
    run: &'static Run,
    claim: Claim,
    text: &'static str,
    published: f64,
    tol: f64,
}

const LN_0_3: f64 = -1.203_972_804_325_936_1;
const LN_2: f64 = std::f64::consts::LN_2;

const CHECKS: &[Check] = &[
    Check {
        run: &LORENZ,
        claim: Claim::Exponent { index: 0 },
        text: "Lorenz λ1",
        published: 0.9056,
        tol: 0.01,
    },
    Check {
        run: &LORENZ,
        claim: Claim::Exponent { index: 1 },
        text: "Lorenz λ2",
        published: 0.0,
        tol: 0.01,
    },
    Check {
        run: &LORENZ,
        claim: Claim::Exponent { index: 2 },
        text: "Lorenz λ3",
        published: -14.5723,
        tol: 0.02,
    },
    Check {
        run: &LORENZ_FINE,
        claim: Claim::Sum,
        text: "Lorenz Σλ = −(σ + 1 + b) = −41/3",
        published: -41.0 / 3.0,
        tol: 1e-6,
    },
    Check {
        run: &HENON,
        claim: Claim::Exponent { index: 0 },
        text: "Hénon λ1",
        published: 0.41922,
        tol: 0.005,
    },
    Check {
        run: &HENON,
        claim: Claim::Exponent { index: 1 },
        text: "Hénon λ2",
        published: -1.62319,
        tol: 0.005,
    },
    Check {
        run: &HENON,
        claim: Claim::Sum,
        text: "Hénon Σλ = ln 0.3",
        published: LN_0_3,
        tol: 1e-9,
    },
    Check {
        run: &LOGISTIC,
        claim: Claim::Exponent { index: 0 },
        text: "logistic r = 4 λ = ln 2",
        published: LN_2,
        tol: 5e-3,
    },
];

const WRONG_CHECKS: &[Check] = &[
    Check {
        run: &LORENZ,
        claim: Claim::Exponent { index: 0 },
        text: "wrong: Lorenz λ1 = 1.0",
        published: 1.0,
        tol: 0.01,
    },
    Check {
        run: &LORENZ_FINE,
        claim: Claim::Sum,
        text: "wrong: Lorenz Σλ = −13",
        published: -13.0,
        tol: 1e-6,
    },
    Check {
        run: &HENON,
        claim: Claim::Exponent { index: 0 },
        text: "wrong: Hénon λ1 = 0.5",
        published: 0.5,
        tol: 0.005,
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

/// Compiles a gallery template through `service::compile_system`, as the UI
/// does when the student opens it.
fn compile(run: &Run) -> (u32, Arc<CompiledSystem>, Vec<f64>) {
    let templates: Vec<Template> = serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
    let template = templates
        .iter()
        .find(|t| t.id == run.template)
        .unwrap_or_else(|| panic!("no template {:?}", run.template));
    let res = service::compile_system(
        store(),
        &CompileRequest {
            source: template.equations.join("\n"),
            format: template.format,
        },
    );
    assert!(
        res.ok
            && !res
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
        "{} doesn't compile: {:?}",
        run.template,
        res.diagnostics
    );
    let params = res
        .params
        .iter()
        .map(|p| {
            run.params
                .iter()
                .find(|(name, _)| *name == p.name)
                .map(|&(_, v)| v)
                .unwrap_or_else(|| panic!("{}: no value for {}", run.label, p.name))
        })
        .collect();
    let id = res.system_id.expect("compiled");
    (id, store().get(id).expect("stored"), params)
}

/// Each run is measured once per test binary, whichever test asks first.
fn spectrum(run: &'static Run) -> Arc<LyapunovResponse> {
    static RUNS: OnceLock<Mutex<HashMap<&'static str, Arc<LyapunovResponse>>>> = OnceLock::new();
    let runs = RUNS.get_or_init(Default::default);
    if let Some(done) = runs.lock().unwrap().get(run.label) {
        return done.clone();
    }
    let (system_id, system, params) = compile(run);
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Verdict {
    Pass,
    Fail,
}

struct Row {
    claim: &'static str,
    published: f64,
    measured: f64,
    uncertainty: Option<f64>,
    delta: f64,
    tol: f64,
    verdict: Verdict,
}

fn evaluate(check: &Check) -> Row {
    let s = spectrum(check.run);
    let (measured, uncertainty) = match check.claim {
        Claim::Exponent { index } => (
            s.exponents.get(index).copied().unwrap_or(f64::NAN),
            s.uncertainty.get(index).copied(),
        ),
        Claim::Sum => (s.sum, None),
    };
    let delta = (measured - check.published).abs();
    let ok = s.status == Status::Completed && delta <= check.tol;
    Row {
        claim: check.text,
        published: check.published,
        measured,
        uncertainty,
        delta,
        tol: check.tol,
        verdict: if ok { Verdict::Pass } else { Verdict::Fail },
    }
}

fn print_line([claim, published, measured, pm, delta, tol, verdict, within]: [&str; 8]) {
    println!(
        "{claim:<34} {published:>12} {measured:>14} {pm:>9} {delta:>8} {tol:>6} {verdict:<6} {within}"
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
        let (pm, within) = match r.uncertainty {
            Some(u) => (
                format!("{u:.1e}"),
                if r.delta <= 3.0 * u { "yes" } else { "no" }.to_string(),
            ),
            None => ("—".to_string(), "—".to_string()),
        };
        print_line([
            r.claim,
            &format!("{:.6}", r.published),
            &format!("{:.8}", r.measured),
            &pm,
            &format!("{:.1e}", r.delta),
            &format!("{:.0e}", r.tol),
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
