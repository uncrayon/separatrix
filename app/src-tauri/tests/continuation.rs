use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use separatrix_lib::ipc::{
    BranchOut, CompileRequest, ContinuationDirection, ContinuationRequest, ContinuationStart,
    InputFormat, OrbitDiagramRequest, Severity, SpecialKindOut,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

enum Source {
    Template(&'static str),
    Text(&'static str),
}

struct Run {
    label: &'static str,
    source: Source,
    params: &'static [(&'static str, f64)],
    param: &'static str,
    range: (f64, f64),
    start: Start,
    direction: ContinuationDirection,
    ds: f64,
    ds_max: f64,
}

enum Start {
    Point(&'static [f64]),
    Switch {
        parent: &'static Run,
        kind: SpecialKindOut,
    },
}

const DEFAULT: Run = Run {
    label: "",
    source: Source::Text(""),
    params: &[],
    param: "",
    range: (-1.0, 1.0),
    start: Start::Point(&[0.0]),
    direction: ContinuationDirection::Forward,
    ds: 0.05,
    ds_max: 0.1,
};

const SADDLE_NODE: Run = Run {
    label: "saddle-node",
    source: Source::Template("saddle-node"),
    params: &[("r", -1.0)],
    param: "r",
    start: Start::Point(&[-1.0]),
    ..DEFAULT
};

const TRANSCRITICAL: Run = Run {
    label: "transcritical",
    source: Source::Template("transcritical"),
    params: &[("r", -1.0)],
    param: "r",
    ..DEFAULT
};

const PITCHFORK: Run = Run {
    label: "pitchfork",
    source: Source::Template("pitchfork"),
    params: &[("r", -1.0)],
    param: "r",
    ..DEFAULT
};

const HOPF: Run = Run {
    label: "Hopf normal form",
    source: Source::Template("hopf-supercritical"),
    params: &[("mu", -1.0)],
    param: "mu",
    start: Start::Point(&[0.0, 0.0]),
    ..DEFAULT
};

const SUBCRITICAL: Run = Run {
    label: "subcritical Hopf",
    source: Source::Text("x' = mu*x - y + x*(x^2 + y^2)\ny' = x + mu*y + y*(x^2 + y^2)"),
    ..HOPF
};

const NEUTRAL_SADDLE: Run = Run {
    label: "neutral saddle",
    source: Source::Text("x' = y\ny' = x + mu*y"),
    ..HOPF
};

const SELKOV_A: f64 = 0.08;

const SELKOV: Run = Run {
    label: "Sel'kov",
    source: Source::Template("selkov-glycolysis"),
    params: &[("a", SELKOV_A), ("b", 0.05)],
    param: "b",
    range: (0.05, 1.5),
    start: Start::Point(&[0.06, 0.6]),
    ..DEFAULT
};

const SELKOV_LONG: Run = Run {
    label: "Sel'kov, ds = 1, ds_max = 2",
    ds: 1.0,
    ds_max: 2.0,
    ..SELKOV
};

const BRUSSELATOR: Run = Run {
    label: "Brusselator",
    source: Source::Text("x' = a - (b + 1)*x + x^2*y\ny' = b*x - x^2*y"),
    params: &[("a", 2.0), ("b", 1.0)],
    param: "b",
    range: (1.0, 8.0),
    start: Start::Point(&[2.0, 0.5]),
    ..DEFAULT
};

const LOGISTIC: Run = Run {
    label: "logistic map",
    source: Source::Template("logistic-map"),
    params: &[("r", 2.5)],
    param: "r",
    range: (2.5, 3.5),
    start: Start::Point(&[0.6]),
    ..DEFAULT
};

const HENON_B: f64 = 0.3;

const HENON: Run = Run {
    label: "Hénon map, both directions",
    source: Source::Template("henon-map"),
    params: &[("a", 0.2), ("b", HENON_B)],
    param: "a",
    range: (-0.5, 1.0),
    start: Start::Point(&[1.09, 0.327]),
    direction: ContinuationDirection::Both,
    ..DEFAULT
};

const PITCHFORK_SWITCH: Run = Run {
    label: "pitchfork, switched at BP, both halves",
    start: Start::Switch {
        parent: &PITCHFORK,
        kind: BranchPoint,
    },
    direction: ContinuationDirection::Both,
    ..PITCHFORK
};

const TRANSCRITICAL_SWITCH: Run = Run {
    label: "transcritical, switched at BP, both halves",
    start: Start::Switch {
        parent: &TRANSCRITICAL,
        kind: BranchPoint,
    },
    direction: ContinuationDirection::Both,
    ..TRANSCRITICAL
};

const CASCADE_1: Run = Run {
    label: "logistic map, period 1, r ∈ [2.8, 3.57]",
    params: &[("r", 2.8)],
    range: (2.8, 3.57),
    start: Start::Point(&[0.642_857_142_857_142_9]),
    ..LOGISTIC
};

const CASCADE_2: Run = Run {
    label: "logistic map, period 2, switched at r1",
    start: Start::Switch {
        parent: &CASCADE_1,
        kind: Flip,
    },
    ..CASCADE_1
};

const CASCADE_4: Run = Run {
    label: "logistic map, period 4, switched at r2",
    start: Start::Switch {
        parent: &CASCADE_2,
        kind: Flip,
    },
    ..CASCADE_1
};

const CASCADE_8: Run = Run {
    label: "logistic map, period 8, switched at r3",
    start: Start::Switch {
        parent: &CASCADE_4,
        kind: Flip,
    },
    ..CASCADE_1
};

const CASCADE: [&Run; 4] = [&CASCADE_1, &CASCADE_2, &CASCADE_4, &CASCADE_8];

const DELAYED_LOGISTIC: Run = Run {
    label: "delayed logistic map",
    source: Source::Text("x_{n+1} = r*x_n*(1 - y_n)\ny_{n+1} = x_n"),
    params: &[("r", 1.5)],
    param: "r",
    range: (1.5, 2.5),
    start: Start::Point(&[0.333, 0.333]),
    ..DEFAULT
};

enum Measure {
    P(SpecialKindOut, usize),
    Omega(SpecialKindOut, usize),
    Lyapunov(usize),
    Count(SpecialKindOut),
    MaxAbsX,
    MaxResidual {
        residual: fn(f64, f64) -> f64,
        both_signs: bool,
    },
    CascadeAgreement,
}

enum Expect {
    Near { value: f64, tol: f64 },
    AtMost { bound: f64 },
    Exactly { count: usize },
}

struct Check {
    text: &'static str,
    run: &'static Run,
    measure: Measure,
    expect: Expect,
}

const TOL: f64 = 1e-9;
const SELKOV_TOL: f64 = 1e-8;
const LYAPUNOV_TOL: f64 = 1e-6;
const DOUBLING_TOL: f64 = 1e-6;
const ON_BRANCH_TOL: f64 = 1e-10;
const AGREEMENT_TOL: f64 = 1e-9;
const FEIGENBAUM_DELTA: f64 = 4.669_201_609_102_99;

const fn near(value: f64, tol: f64) -> Expect {
    Expect::Near { value, tol }
}

use SpecialKindOut::{BranchPoint, Flip, Fold, Hopf, NeimarkSacker};

const CHECKS: &[Check] = &[
    Check {
        text: "saddle-node ẋ = r + x²: LP r",
        run: &SADDLE_NODE,
        measure: Measure::P(Fold, 0),
        expect: near(0.0, TOL),
    },
    Check {
        text: "transcritical ẋ = rx − x²: BP r",
        run: &TRANSCRITICAL,
        measure: Measure::P(BranchPoint, 0),
        expect: near(0.0, TOL),
    },
    Check {
        text: "transcritical: max |x| on x = 0",
        run: &TRANSCRITICAL,
        measure: Measure::MaxAbsX,
        expect: Expect::AtMost { bound: 1e-12 },
    },
    Check {
        text: "pitchfork ẋ = rx − x³: BP r",
        run: &PITCHFORK,
        measure: Measure::P(BranchPoint, 0),
        expect: near(0.0, TOL),
    },
    Check {
        text: "Hopf normal form: H μ",
        run: &HOPF,
        measure: Measure::P(Hopf, 0),
        expect: near(0.0, TOL),
    },
    Check {
        text: "Hopf normal form: ω",
        run: &HOPF,
        measure: Measure::Omega(Hopf, 0),
        expect: near(1.0, TOL),
    },
    Check {
        text: "Hopf normal form: ℓ₁ (supercritical)",
        run: &HOPF,
        measure: Measure::Lyapunov(0),
        expect: near(-1.0, LYAPUNOV_TOL),
    },
    Check {
        text: "subcritical Hopf: ℓ₁",
        run: &SUBCRITICAL,
        measure: Measure::Lyapunov(0),
        expect: near(1.0, LYAPUNOV_TOL),
    },
    Check {
        text: "neutral saddle ẋ = y, ẏ = x + μy: H count",
        run: &NEUTRAL_SADDLE,
        measure: Measure::Count(Hopf),
        expect: Expect::Exactly { count: 0 },
    },
    Check {
        text: "Sel'kov a = 0.08: first H b = √0.12",
        run: &SELKOV,
        measure: Measure::P(Hopf, 0),
        expect: near(0.346_410_161_513_775_4, SELKOV_TOL),
    },
    Check {
        text: "Sel'kov: first H ω = √0.2",
        run: &SELKOV,
        measure: Measure::Omega(Hopf, 0),
        expect: near(0.447_213_595_499_958, SELKOV_TOL),
    },
    Check {
        text: "Sel'kov: second H b = √0.72",
        run: &SELKOV,
        measure: Measure::P(Hopf, 1),
        expect: near(0.848_528_137_423_857, SELKOV_TOL),
    },
    Check {
        text: "Sel'kov: second H ω = √0.8",
        run: &SELKOV,
        measure: Measure::Omega(Hopf, 1),
        expect: near(0.894_427_190_999_916, SELKOV_TOL),
    },
    Check {
        text: "Sel'kov, ds = 1, ds_max = 2: H count",
        run: &SELKOV_LONG,
        measure: Measure::Count(Hopf),
        expect: Expect::Exactly { count: 2 },
    },
    Check {
        text: "Brusselator a = 2: H b = 1 + a²",
        run: &BRUSSELATOR,
        measure: Measure::P(Hopf, 0),
        expect: near(5.0, TOL),
    },
    Check {
        text: "Brusselator a = 2: ω = a",
        run: &BRUSSELATOR,
        measure: Measure::Omega(Hopf, 0),
        expect: near(2.0, TOL),
    },
    Check {
        text: "logistic map: PD r1",
        run: &LOGISTIC,
        measure: Measure::P(Flip, 0),
        expect: near(3.0, DOUBLING_TOL),
    },
    Check {
        text: "Hénon b = 0.3: LP a0 = −(1 − b)²/4",
        run: &HENON,
        measure: Measure::P(Fold, 0),
        expect: near(-0.1225, TOL),
    },
    Check {
        text: "Hénon b = 0.3: PD a1 = 3(1 − b)²/4",
        run: &HENON,
        measure: Measure::P(Flip, 0),
        expect: near(0.3675, TOL),
    },
    Check {
        text: "delayed logistic map: NS r",
        run: &DELAYED_LOGISTIC,
        measure: Measure::P(NeimarkSacker, 0),
        expect: near(2.0, TOL),
    },
    Check {
        text: "delayed logistic map: NS θ = π/3",
        run: &DELAYED_LOGISTIC,
        measure: Measure::Omega(NeimarkSacker, 0),
        expect: near(std::f64::consts::FRAC_PI_3, TOL),
    },
    Check {
        text: "pitchfork switched at BP: max |r − x²|, ±x",
        run: &PITCHFORK_SWITCH,
        measure: Measure::MaxResidual {
            residual: |x, r| r - x * x,
            both_signs: true,
        },
        expect: Expect::AtMost {
            bound: ON_BRANCH_TOL,
        },
    },
    Check {
        text: "transcritical switched at BP: max |x − r|",
        run: &TRANSCRITICAL_SWITCH,
        measure: Measure::MaxResidual {
            residual: |x, r| x - r,
            both_signs: true,
        },
        expect: Expect::AtMost {
            bound: ON_BRANCH_TOL,
        },
    },
    Check {
        text: "logistic, period 2 from r1: PD r2 = 1 + √6",
        run: &CASCADE_2,
        measure: Measure::P(Flip, 0),
        expect: near(3.449_489_742_783_178, DOUBLING_TOL),
    },
    Check {
        text: "logistic, period 4 from r2: PD r3",
        run: &CASCADE_4,
        measure: Measure::P(Flip, 0),
        expect: near(3.544_090_360, DOUBLING_TOL),
    },
    Check {
        text: "logistic, period 8 from r3: PD r4",
        run: &CASCADE_8,
        measure: Measure::P(Flip, 0),
        expect: near(3.564_407_266, DOUBLING_TOL),
    },
    Check {
        text: "r1..r4 vs period_doublings: max |Δ|",
        run: &CASCADE_1,
        measure: Measure::CascadeAgreement,
        expect: Expect::AtMost {
            bound: AGREEMENT_TOL,
        },
    },
];

const WRONG_CHECKS: &[Check] = &[
    Check {
        text: "wrong: pitchfork BP r = 1e−4",
        run: &PITCHFORK,
        measure: Measure::P(BranchPoint, 0),
        expect: near(1e-4, TOL),
    },
    Check {
        text: "wrong: Sel'kov first H b = 0.35",
        run: &SELKOV,
        measure: Measure::P(Hopf, 0),
        expect: near(0.35, SELKOV_TOL),
    },
    Check {
        text: "wrong: Hopf normal form ω = 1.001",
        run: &HOPF,
        measure: Measure::Omega(Hopf, 0),
        expect: near(1.001, TOL),
    },
    Check {
        text: "wrong: neutral saddle has 1 H",
        run: &NEUTRAL_SADDLE,
        measure: Measure::Count(Hopf),
        expect: Expect::Exactly { count: 1 },
    },
    Check {
        text: "wrong: logistic r1 = 3.00001",
        run: &LOGISTIC,
        measure: Measure::P(Flip, 0),
        expect: near(3.00001, DOUBLING_TOL),
    },
    Check {
        text: "wrong: logistic r3 = 3.5441",
        run: &CASCADE_4,
        measure: Measure::P(Flip, 0),
        expect: near(3.5441, DOUBLING_TOL),
    },
    Check {
        text: "wrong: switched pitchfork branch is x = r",
        run: &PITCHFORK_SWITCH,
        measure: Measure::MaxResidual {
            residual: |x, r| x - r,
            both_signs: false,
        },
        expect: Expect::AtMost {
            bound: ON_BRANCH_TOL,
        },
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

fn branch(run: &'static Run) -> Arc<BranchOut> {
    static RUNS: OnceLock<Mutex<HashMap<&'static str, Arc<BranchOut>>>> = OnceLock::new();
    let runs = RUNS.get_or_init(Default::default);
    if let Some(done) = runs.lock().unwrap().get(run.label) {
        return done.clone();
    }
    let (source, format) = match run.source {
        Source::Template(id) => {
            let templates: Vec<Template> =
                serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
            let t = templates
                .into_iter()
                .find(|t| t.id == id)
                .unwrap_or_else(|| panic!("no template {id:?}"));
            (t.equations.join("\n"), t.format)
        }
        Source::Text(text) => (text.to_string(), InputFormat::Text),
    };
    let res = service::compile_system(store(), &CompileRequest { source, format });
    assert!(
        res.ok
            && !res
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
        "{} doesn't compile: {:?}",
        run.label,
        res.diagnostics
    );
    let value = |name: &str| {
        run.params
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, v)| v)
            .unwrap_or_else(|| panic!("{}: no value for {name}", run.label))
    };
    let params = res.params.iter().map(|p| value(&p.name)).collect();
    let param = res
        .params
        .iter()
        .position(|p| p.name == run.param)
        .unwrap_or_else(|| panic!("{}: no parameter {}", run.label, run.param));
    let system_id = res.system_id.expect("compiled");
    let start = match run.start {
        Start::Point(x0) => ContinuationStart::Point {
            x0: x0.to_vec(),
            period: 1,
        },
        Start::Switch { parent, kind } => {
            let at = branch(parent)
                .special
                .iter()
                .find(|sp| sp.kind == kind)
                .unwrap_or_else(|| panic!("{}: no {kind:?} on {}", run.label, parent.label))
                .clone();
            ContinuationStart::Switch { at }
        }
    };
    let req = ContinuationRequest {
        request_id: 1,
        system_id,
        params,
        param,
        range: run.range,
        start,
        direction: run.direction,
        ds: run.ds,
        ds_max: run.ds_max,
        max_points: 1000,
    };
    let system = store().get(system_id).expect("stored");
    let out = service::compute_continuation(&system, &req)
        .unwrap_or_else(|e| panic!("{}: {e}", run.label));
    let out = Arc::new(out.branch);
    runs.lock().unwrap().insert(run.label, out.clone());
    out
}

fn measure(check: &Check) -> f64 {
    let b = branch(check.run);
    let nth = |kind: SpecialKindOut, n: usize| b.special.iter().filter(|sp| sp.kind == kind).nth(n);
    let missing = f64::NAN;
    match check.measure {
        Measure::P(kind, n) => nth(kind, n).map_or(missing, |sp| sp.p),
        Measure::Omega(kind, n) => nth(kind, n).and_then(|sp| sp.omega).unwrap_or(missing),
        Measure::Lyapunov(n) => nth(Hopf, n).and_then(|sp| sp.lyapunov).unwrap_or(missing),
        Measure::Count(kind) => b.special.iter().filter(|sp| sp.kind == kind).count() as f64,
        Measure::MaxAbsX => b.x.iter().fold(0.0, |m, x| x.abs().max(m)),
        Measure::MaxResidual {
            residual,
            both_signs,
        } => {
            let signs = b.x.iter().any(|x| *x > 0.0) && b.x.iter().any(|x| *x < 0.0);
            if both_signs && !signs {
                return missing;
            }
            b.x.iter()
                .zip(&b.p)
                .fold(0.0, |m, (x, p)| residual(*x, *p).abs().max(m))
        }
        Measure::CascadeAgreement => cascade()
            .iter()
            .zip(period_doublings())
            .fold(0.0, |m, (r, d)| (r - d).abs().max(m)),
    }
}

fn cascade() -> Vec<f64> {
    CASCADE
        .iter()
        .map(|run| {
            branch(run)
                .special
                .iter()
                .find(|sp| sp.kind == Flip)
                .map_or(f64::NAN, |sp| sp.p)
        })
        .collect()
}

fn period_doublings() -> Vec<f64> {
    let res = service::compile_system(
        store(),
        &CompileRequest {
            source: "x_{n+1} = r*x_n*(1 - x_n)".into(),
            format: InputFormat::Text,
        },
    );
    let system_id = res.system_id.expect("the logistic map compiles");
    let req = OrbitDiagramRequest {
        request_id: 1,
        system_id,
        params: vec![2.8],
        param: 0,
        range: (2.8, 3.57),
        columns: 2000,
        state: 0,
        x0: vec![0.1],
        transient: 2000,
        kept: 64,
        doublings: true,
    };
    let system = store().get(system_id).expect("stored");
    let out = service::compute_orbit_diagram(&system, &req).expect("the orbit diagram");
    let found = out.doublings.unwrap_or_default();
    (0..CASCADE.len())
        .map(|k| found.get(k).map_or(f64::NAN, |d| d.r))
        .collect()
}

struct Row {
    text: &'static str,
    expect: &'static Expect,
    measured: f64,
}

impl Row {
    fn delta(&self) -> Option<f64> {
        match *self.expect {
            Expect::Near { value, .. } => Some((self.measured - value).abs()),
            Expect::Exactly { count } => Some((self.measured - count as f64).abs()),
            Expect::AtMost { .. } => None,
        }
        .filter(|d| d.is_finite())
    }

    fn passes(&self) -> bool {
        match *self.expect {
            Expect::Near { tol, .. } => self.delta().is_some_and(|d| d <= tol),
            Expect::Exactly { .. } => self.delta() == Some(0.0),
            Expect::AtMost { bound } => self.measured <= bound,
        }
    }
}

fn print_line([claim, reference, measured, delta, tol, verdict]: [&str; 6]) {
    // The dot of ẋ and ẏ is a combining character, which takes no column.
    let columns = claim
        .chars()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .count();
    let pad = " ".repeat(44usize.saturating_sub(columns));
    println!("{claim}{pad} {reference:>16} {measured:>16} {delta:>8} {tol:>8} {verdict}");
}

/// Twelve decimals, without the sign of a value that rounds to zero.
fn fixed(v: f64) -> String {
    let s = format!("{v:.12}");
    match s.strip_prefix('-') {
        Some(digits) if digits.bytes().all(|c| c == b'0' || c == b'.') => digits.to_string(),
        _ => s,
    }
}

fn print_table(rows: &[Row]) {
    println!();
    print_line(["claim", "reference", "measured", "Δ", "tol", "result"]);
    for r in rows {
        let (reference, measured, tol) = match *r.expect {
            Expect::Near { value, tol } => (fixed(value), fixed(r.measured), format!("{tol:.0e}")),
            Expect::AtMost { bound } => (
                "0".to_string(),
                format!("{:.3e}", r.measured),
                format!("≤ {bound:.0e}"),
            ),
            Expect::Exactly { count } => (
                count.to_string(),
                format!("{}", r.measured),
                "exact".to_string(),
            ),
        };
        print_line([
            r.text,
            &reference,
            &measured,
            &r.delta().map_or("—".to_string(), |d| format!("{d:.1e}")),
            &tol,
            if r.passes() { "PASS" } else { "FAIL" },
        ]);
    }
}

fn evaluate(check: &'static Check) -> Row {
    Row {
        text: check.text,
        expect: &check.expect,
        measured: measure(check),
    }
}

#[test]
fn continuation_claims_reproduce() {
    let started = Instant::now();
    let rows: Vec<Row> = CHECKS.iter().map(evaluate).collect();
    print_table(&rows);
    println!(
        "\nℓ₁ is Kuznetsov's (§3.5) with ⟨p, q⟩ = 1 and ⟨q, q⟩ = 1/2, so ṙ = μr + a r³, \
         θ̇ = ω has ℓ₁ = a/ω."
    );
    let r = cascade();
    let ratio = (r[2] - r[1]) / (r[3] - r[2]);
    print_line([
        "info: Feigenbaum δ ≈ (r3 − r2)/(r4 − r3)",
        &format!("{FEIGENBAUM_DELTA:.12}"),
        &format!("{ratio:.12}"),
        &format!("{:.1e}", (ratio - FEIGENBAUM_DELTA).abs()),
        "—",
        "info",
    ]);
    println!(
        "{} rows, wall time {:.2} s (budget 10 s)",
        rows.len(),
        started.elapsed().as_secs_f64()
    );
    let failed: Vec<&str> = rows
        .iter()
        .filter(|r| !r.passes())
        .map(|r| r.text)
        .collect();
    assert!(failed.is_empty(), "failed claims: {failed:?}");
}

#[test]
fn wrong_claims_fail() {
    let rows: Vec<Row> = WRONG_CHECKS.iter().map(evaluate).collect();
    print_table(&rows);
    let passed: Vec<&str> = rows.iter().filter(|r| r.passes()).map(|r| r.text).collect();
    assert!(passed.is_empty(), "wrong claims not failed: {passed:?}");
}
