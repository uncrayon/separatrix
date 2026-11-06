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
//! - Logistic map period doublings, located from its orbit diagram through
//!   `service::compute_orbit_diagram`: r1 = 3, r2 = 1 + √6 (Strogatz §10.3),
//!   r3 ≈ 3.5440903 and r4 ≈ 3.5644073 (Strogatz Table 10.2.1 gives them to
//!   fewer digits). The ratio (r3 − r2)/(r4 − r3) is printed next to
//!   Feigenbaum's δ = 4.669… for information only: three doublings are too
//!   few for it to converge.
//!
//! The Σ row of Lorenz uses its own run with RK4 at h = 0.001: RK4's
//! truncation error on `ln det Φ` puts Σλ about 1e−4 from −41/3 at
//! h = 0.01, shrinking like h⁴ (see `crates/dyn-analysis/tests/lyapunov.rs`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use dyn_expr::CompiledSystem;
use separatrix_lib::ipc::{
    CompileRequest, InputFormat, LyapunovRequest, LyapunovResponse, Method, OrbitDiagramRequest,
    OrbitDiagramResponse, Severity, SolverOptions, Status,
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
    /// The `k`-th period doubling (from 1) of `LOGISTIC_DIAGRAM`.
    Doubling {
        k: usize,
    },
}

struct Check {
    claim: Claim,
    text: &'static str,
    published: f64,
    tol: f64,
}

const LN_0_3: f64 = -1.203_972_804_325_936_1;
const LN_2: f64 = std::f64::consts::LN_2;
const R2: f64 = 3.449_489_742_783_178; // 1 + √6
const FEIGENBAUM_DELTA: f64 = 4.669_201_609_102_99;

const CHECKS: &[Check] = &[
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 0,
        },
        text: "Lorenz λ1",
        published: 0.9056,
        tol: 0.01,
    },
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 1,
        },
        text: "Lorenz λ2",
        published: 0.0,
        tol: 0.01,
    },
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 2,
        },
        text: "Lorenz λ3",
        published: -14.5723,
        tol: 0.02,
    },
    Check {
        claim: Claim::Sum { run: &LORENZ_FINE },
        text: "Lorenz Σλ = −(σ + 1 + b) = −41/3",
        published: -41.0 / 3.0,
        tol: 1e-6,
    },
    Check {
        claim: Claim::Exponent {
            run: &HENON,
            index: 0,
        },
        text: "Hénon λ1",
        published: 0.41922,
        tol: 0.005,
    },
    Check {
        claim: Claim::Exponent {
            run: &HENON,
            index: 1,
        },
        text: "Hénon λ2",
        published: -1.62319,
        tol: 0.005,
    },
    Check {
        claim: Claim::Sum { run: &HENON },
        text: "Hénon Σλ = ln 0.3",
        published: LN_0_3,
        tol: 1e-9,
    },
    Check {
        claim: Claim::Exponent {
            run: &LOGISTIC,
            index: 0,
        },
        text: "logistic r = 4 λ = ln 2",
        published: LN_2,
        tol: 5e-3,
    },
    Check {
        claim: Claim::Doubling { k: 1 },
        text: "logistic r1 = 3",
        published: 3.0,
        tol: 1e-6,
    },
    Check {
        claim: Claim::Doubling { k: 2 },
        text: "logistic r2 = 1 + √6",
        published: R2,
        tol: 1e-6,
    },
    Check {
        claim: Claim::Doubling { k: 3 },
        text: "logistic r3",
        published: 3.544_090_3,
        tol: 1e-6,
    },
    Check {
        claim: Claim::Doubling { k: 4 },
        text: "logistic r4",
        published: 3.564_407_3,
        tol: 1e-6,
    },
];

const WRONG_CHECKS: &[Check] = &[
    Check {
        claim: Claim::Exponent {
            run: &LORENZ,
            index: 0,
        },
        text: "wrong: Lorenz λ1 = 1.0",
        published: 1.0,
        tol: 0.01,
    },
    Check {
        claim: Claim::Sum { run: &LORENZ_FINE },
        text: "wrong: Lorenz Σλ = −13",
        published: -13.0,
        tol: 1e-6,
    },
    Check {
        claim: Claim::Exponent {
            run: &HENON,
            index: 0,
        },
        text: "wrong: Hénon λ1 = 0.5",
        published: 0.5,
        tol: 0.005,
    },
    Check {
        claim: Claim::Doubling { k: 2 },
        text: "wrong: logistic r2 = 3.45",
        published: 3.45,
        tol: 1e-6,
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
/// does when the student opens it, with the parameters in `values`. Also
/// returns the parameter names, in the order of the values.
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
    let res = service::compile_system(
        store(),
        &CompileRequest {
            source: found.equations.join("\n"),
            format: found.format,
        },
    );
    assert!(
        res.ok
            && !res
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
        "{template} doesn't compile: {:?}",
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

/// Each run is measured once per test binary, whichever test asks first.
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

/// `LOGISTIC_DIAGRAM`, computed once per test binary, whichever test asks first.
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
    };
    let delta = (measured - check.published).abs();
    let ok = completed && delta <= check.tol;
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
        let (pm, within) = match r.uncertainty {
            Some(u) => (
                format!("{u:.1e}"),
                if r.delta <= 3.0 * u { "yes" } else { "no" }.to_string(),
            ),
            None => ("—".to_string(), "—".to_string()),
        };
        print_line([
            r.claim,
            &format!("{:.7}", r.published),
            &format!("{:.9}", r.measured),
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
