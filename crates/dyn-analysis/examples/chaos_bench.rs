use std::hint::black_box;
use std::time::Instant;

use dyn_analysis::{lyapunov_spectrum, orbit_diagram, LyapunovOptions, OrbitDiagramOptions};
use dyn_core::{FnField, SystemKind, VectorField};
use dyn_expr::{compile, CompiledSystem, InputFormat};
use dyn_solve::{Method, SolverOptions, Status};

const TEMPLATES_JSON: &str = include_str!("../../../app/src/templates/templates.json");

const HISTORY: usize = 2000;

fn template(id: &str) -> CompiledSystem {
    let all: serde_json::Value = serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
    let found = all
        .as_array()
        .expect("a list")
        .iter()
        .find(|t| t["id"] == id)
        .unwrap_or_else(|| panic!("no template {id}"));
    let source: Vec<&str> = found["equations"]
        .as_array()
        .expect("equations")
        .iter()
        .map(|e| e.as_str().expect("an equation"))
        .collect();
    compile(&source.join("\n"), InputFormat::Latex)
        .system
        .unwrap_or_else(|| panic!("{id} compiles"))
}

fn params(sys: &CompiledSystem, values: &[(&str, f64)]) -> Vec<f64> {
    assert_eq!(sys.info.params.len(), values.len());
    sys.info
        .params
        .iter()
        .map(|p| {
            values
                .iter()
                .find(|(name, _)| *name == p.name)
                .unwrap_or_else(|| panic!("no value for {}", p.name))
                .1
        })
        .collect()
}

fn lorenz_native() -> FnField {
    FnField::new(3, 3, SystemKind::Ode, |_t, x, p, out| {
        let (s, r, b) = (p[0], p[1], p[2]);
        out[0] = s * (x[1] - x[0]);
        out[1] = r * x[0] - x[1] - x[0] * x[2];
        out[2] = x[0] * x[1] - b * x[2];
    })
    .with_jacobian(|_t, x, p, j| {
        let (s, r, b) = (p[0], p[1], p[2]);
        j.copy_from_slice(&[-s, s, 0.0, r - x[2], -1.0, -x[0], x[1], x[0], -b]);
    })
}

fn logistic_native() -> FnField {
    FnField::new(1, 1, SystemKind::Map, |_n, x, p, out| {
        out[0] = p[0] * x[0] * (1.0 - x[0])
    })
    .with_jacobian(|_n, x, p, j| j[0] = p[0] - 2.0 * p[0] * x[0])
}

fn henon_native() -> FnField {
    FnField::new(2, 2, SystemKind::Map, |_n, x, p, out| {
        out[0] = x[1] + 1.0 - p[0] * (x[0] * x[0]);
        out[1] = p[1] * x[0];
    })
    .with_jacobian(|_n, x, p, j| j.copy_from_slice(&[-2.0 * p[0] * x[0], 1.0, p[1], 0.0]))
}

struct Outcome {
    check: f64,
    evals: usize,
}

struct Side {
    name: &'static str,
    field: Box<dyn VectorField>,
    params: Vec<f64>,
}

struct Workload {
    label: &'static str,
    sides: [Side; 2],
    run: fn(&dyn VectorField, &[f64]) -> Outcome,
}

const GATE_RK4: SolverOptions = SolverOptions {
    method: Method::Rk4,
    rtol: 1e-6,
    atol: 1e-9,
    h: 0.01,
    h_max: 0.0,
    max_steps: 2_000_000,
};

fn lorenz_run(f: &dyn VectorField, p: &[f64], total: f64) -> Outcome {
    let opts = LyapunovOptions {
        transient: 100.0,
        total,
        interval: 0.1,
        blocks: 10,
        history: HISTORY,
    };
    let s = lyapunov_spectrum(f, p, 0.0, &[0.0, 1.0, 0.0], &GATE_RK4, &opts);
    assert_eq!(s.status, Status::Completed, "Lorenz T = {total}");
    Outcome {
        check: s.exponents[0],
        evals: s.stats.evals,
    }
}

fn lorenz_gate(f: &dyn VectorField, p: &[f64]) -> Outcome {
    lorenz_run(f, p, 1e4)
}

fn lorenz_view(f: &dyn VectorField, p: &[f64]) -> Outcome {
    lorenz_run(f, p, 1e3)
}

fn logistic_orbit(f: &dyn VectorField, p: &[f64]) -> Outcome {
    let opts = OrbitDiagramOptions {
        param: 0,
        range: (2.8, 4.0),
        columns: 1200,
        state: 0,
        transient: 1000,
        kept: 300,
    };
    let d = orbit_diagram(f, p, &[0.1], &opts);
    assert!(d.x.iter().all(|x| x.is_finite()), "no column escaped");
    Outcome {
        check: d.x.iter().sum(),
        evals: opts.columns * (opts.transient + opts.kept - 1),
    }
}

fn henon_lyapunov(f: &dyn VectorField, p: &[f64]) -> Outcome {
    let opts = LyapunovOptions {
        transient: 1e3,
        total: 1e6,
        interval: 1.0,
        blocks: 10,
        history: HISTORY,
    };
    let s = lyapunov_spectrum(f, p, 0.0, &[0.0, 0.0], &GATE_RK4, &opts);
    assert_eq!(s.status, Status::Completed, "Hénon");
    Outcome {
        check: s.exponents[0],
        evals: s.stats.evals,
    }
}

fn side(name: &'static str, field: impl VectorField + 'static, params: Vec<f64>) -> Side {
    Side {
        name,
        field: Box::new(field),
        params,
    }
}

fn workloads() -> Vec<Workload> {
    let lorenz = template("lorenz");
    let lorenz_p = params(&lorenz, &[("sigma", 10.0), ("r", 28.0), ("b", 8.0 / 3.0)]);
    let native_p = vec![10.0, 28.0, 8.0 / 3.0];
    let logistic = template("logistic-map");
    let henon = template("henon-map");
    let henon_p = params(&henon, &[("a", 1.4), ("b", 0.3)]);
    vec![
        Workload {
            label: "(a) Lorenz λ, T = 1e4 (gate)",
            sides: [
                side("compiled", lorenz.clone(), lorenz_p.clone()),
                side("native", lorenz_native(), native_p.clone()),
            ],
            run: lorenz_gate,
        },
        Workload {
            label: "(b) Lorenz λ, T = 1e3 (view)",
            sides: [
                side("compiled", lorenz, lorenz_p),
                side("native", lorenz_native(), native_p),
            ],
            run: lorenz_view,
        },
        Workload {
            label: "(c) logistic orbit 1200 × 1300",
            sides: [
                side("compiled", logistic, vec![0.0]),
                side("native", logistic_native(), vec![0.0]),
            ],
            run: logistic_orbit,
        },
        Workload {
            label: "(d) Hénon λ, N = 1e6",
            sides: [
                side("compiled", henon, henon_p),
                side("native", henon_native(), vec![1.4, 0.3]),
            ],
            run: henon_lyapunov,
        },
    ]
}

fn median(sorted: &[f64]) -> f64 {
    let m = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[m]
    } else {
        (sorted[m - 1] + sorted[m]) / 2.0
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runs: usize = args
        .get(1)
        .map(|a| a.parse().expect("RUNS is a number"))
        .unwrap_or(5);
    let only = args.get(2).map(|w| format!("({w})"));
    println!("{runs} runs per side, compiled and native alternating; times in ms\n");
    println!(
        "{:<32} | {:<8} | {:>4} | {:>9} | {:>19} | {:>10} | {:>20}",
        "workload", "side", "runs", "median ms", "min–max ms", "evals", "check value"
    );
    println!("{}", "-".repeat(122));
    let mut ratios = Vec::new();
    let chosen = workloads().into_iter().filter(|w| {
        only.as_ref()
            .is_none_or(|o| w.label.starts_with(o.as_str()))
    });
    for w in chosen {
        let mut times = [Vec::new(), Vec::new()];
        let mut seen: [Option<Outcome>; 2] = [None, None];
        for _ in 0..runs {
            for (k, s) in w.sides.iter().enumerate() {
                let start = Instant::now();
                let out = black_box((w.run)(black_box(s.field.as_ref()), &s.params));
                times[k].push(start.elapsed().as_secs_f64() * 1e3);
                if let Some(prev) = &seen[k] {
                    assert_eq!(
                        prev.check.to_bits(),
                        out.check.to_bits(),
                        "{} repeats",
                        s.name
                    );
                }
                seen[k] = Some(out);
            }
        }
        let mut medians = [0.0; 2];
        for (k, s) in w.sides.iter().enumerate() {
            times[k].sort_by(f64::total_cmp);
            medians[k] = median(&times[k]);
            let out = seen[k].as_ref().expect("ran");
            println!(
                "{:<32} | {:<8} | {:>4} | {:>9.1} | {:>8.1} – {:>8.1} | {:>10} | {:>20.12}",
                w.label,
                s.name,
                runs,
                medians[k],
                times[k][0],
                times[k][runs - 1],
                out.evals,
                out.check
            );
        }
        let [a, b] = [&seen[0], &seen[1]].map(|o| o.as_ref().expect("ran").check);
        ratios.push((w.label, medians[0] / medians[1], (a - b).abs()));
    }
    println!();
    for (label, ratio, delta) in ratios {
        println!("{label:<32}  compiled/native {ratio:5.2}×   |Δ check| {delta:.1e}");
    }
}
