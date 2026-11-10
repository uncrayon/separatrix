//! The homoclinic gate: the Freire circuit of docs/codim2-reference.md §4,
//! and the Bogdanov–Takens points of systems 1 and 2 there. Each curve
//! starts from a BT row, as "Follow homoclinic" in the Parameter plane
//! starts it, and is followed through `homoclinic_service::compute_homoclinic`
//! with the request the plane sends (app/src/state/homoclinic.ts).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use dyn_analysis::HomoclinicOrbit;
use separatrix_lib::homoclinic_service;
use separatrix_lib::ipc::{
    CompileRequest, ContinuationDirection, HomoclinicOut, HomoclinicRequest, HomoclinicStop,
    InputFormat, Severity,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

/// The plane's request: `ds` and `dsMax = 2·ds` of the Curves settings,
/// `HOMOCLINIC_MAX_POINTS` in each direction, direction `both`.
const UI_DS: f64 = 0.01;
const UI_MAX_POINTS: usize = 500;

type Box2 = ((f64, f64), (f64, f64));

enum Source {
    Template(&'static str),
    Text(&'static str),
}

/// A curve of homoclinic orbits from the BT point `bt`, state `x`, with
/// `across` and `up` free in `ranges`.
struct Run {
    label: &'static str,
    source: Source,
    across: &'static str,
    up: &'static str,
    bt: (f64, f64),
    x: &'static [f64],
    ranges: Box2,
}

/// The BT point `ν = −β = −√0.6` at the origin (docs/codim2-reference.md
/// §4), in the box of the engine's tests.
const FREIRE: Run = Run {
    label: "Freire",
    source: Source::Text(
        "x' = (-(beta + nu)*x + beta*y - 0.328578*x^3 + 0.933578*(y - x)^3)/0.6\n\
         y' = beta*x - beta*y - z - 0.933578*(y - x)^3\n\
         z' = y",
    ),
    across: "nu",
    up: "beta",
    bt: (-0.774_596_669_241_483_4, 0.774_596_669_241_483_4),
    x: &[0.0, 0.0, 0.0],
    ranges: ((-1.2, -0.4), (0.1, 1.0)),
};

/// BT₁ as the codimension-2 gate locates it on the fold and Hopf curves, in
/// the box of the plane's sliders.
const CO: Run = Run {
    label: "CO BT₁",
    source: Source::Template("co-oxidation"),
    across: "k_2",
    up: "K",
    bt: (1.4176276376, 0.9713975735),
    x: &[0.11590872, 0.31546728, 0.28843700],
    ranges: ((0.5, 2.5), (0.0, 2.0)),
};

/// The BT point as the codimension-2 gate locates it, in its box.
const LORENZ: Run = Run {
    label: "Lorenz-84 BT",
    source: Source::Text(
        "X' = -Y^2 - Z^2 - 0.25*X + 0.25*F - 0.987*U^2\n\
         Y' = X*Y - X*Z - Y + 0.25\n\
         Z' = X*Y + X*Z - Z\n\
         U' = -1.04*U + 0.987*U*X + T",
    ),
    across: "F",
    up: "T",
    bt: (1.4467167011, 0.0209401697),
    x: &[1.2256405, -0.03632079, 0.19728832, -0.12339001],
    ranges: ((0.0, 10.0), (-0.1, 0.1)),
};

const RUNS: &[&Run] = &[&FREIRE, &CO, &LORENZ];

/// CO's other BT point, as the codimension-2 gate locates it.
const CO_BT2: (f64, f64) = (1.1611987088, 0.7223390725);

/// A curve ends at a BT point when its `shrunk` end is within this fraction
/// of the box of it on both axes: a tenth of a pixel on a plot 1,000 pixels
/// wide. Where the `Shrunk` test stops depends on the system and on where
/// the last step lands, so no tighter bound holds for every system.
const AT_BT: f64 = 1e-4;

/// `ν` scanned this far either side of the curve's `ν` at a given `β`, on
/// 41 samples, the spacing of the engine's tests. The window holds the
/// subsidiary orbit at `β = 0.4535645`, 1.3e-3 off, and the false root near
/// `ν = −0.72201` at `β = 0.6`, so the claims also check that the primary
/// orbit is the one read.
const NU_WINDOW: f64 = 2e-3;
const NU_SAMPLES: usize = 41;

/// Five digits of the converged shooting and AUTO.
const NU_TOL: f64 = 5e-6;
const SIGMA_TOL: f64 = 5e-5;

/// What a check reads.
enum Measure {
    /// `ν` of the primary orbit at `β` (Freire).
    Nu(f64),
    /// `ν` of the orbit there of the next return time, past the primary
    /// orbit's two halves, whose return times are equal.
    Subsidiary(f64),
    /// Its saddle quantity.
    Sigma(f64),
    /// 1 if that is positive.
    SigmaPositive(f64),
    /// 1 if its saddle has Shilnikov's chaos.
    Chaotic(f64),
    /// The distance from the curve's nearest `shrunk` end to `to`, the
    /// larger of the two parameters' over their box sides; missing when
    /// neither end is.
    Shrunk { run: &'static Run, to: (f64, f64) },
    /// How many of the curve's ends are `shrunk`.
    ShrunkEnds(&'static Run),
    /// 1 if the curve is followed in reversed time.
    Reversed(&'static Run),
    /// How far the drawn curve passes from the primary orbit at `β`: the
    /// gap in `ν` between them over the chord's own bound there, the
    /// sagitta `κℓ²/8` of the bracketing segment seen along `ν`.
    CurveGap(f64),
}

enum Expect {
    Near { value: f64, tol: f64 },
    AtMost { bound: f64 },
    Exactly { count: usize },
}

struct Check {
    text: &'static str,
    measure: Measure,
    expect: Expect,
}

const fn near(value: f64, tol: f64) -> Expect {
    Expect::Near { value, tol }
}

const fn shrunk(text: &'static str, run: &'static Run, to: (f64, f64)) -> Check {
    Check {
        text,
        measure: Measure::Shrunk { run, to },
        expect: Expect::AtMost { bound: AT_BT },
    }
}

const fn exactly(text: &'static str, measure: Measure, count: usize) -> Check {
    Check {
        text,
        measure,
        expect: Expect::Exactly { count },
    }
}

const CHECKS: &[Check] = &[
    Check {
        text: "Freire: ν at β = 0.6",
        measure: Measure::Nu(0.6),
        expect: near(-0.7213093, NU_TOL),
    },
    Check {
        text: "Freire: ν at β = 0.4535645",
        measure: Measure::Nu(0.4535645),
        expect: near(-0.7256926, NU_TOL),
    },
    Check {
        text: "Freire: subsidiary orbit seen at β = 0.4535645",
        measure: Measure::Subsidiary(0.4535645),
        expect: near(-0.7269563, NU_TOL),
    },
    Check {
        text: "Freire: σ at β = 0.6",
        measure: Measure::Sigma(0.6),
        expect: near(-0.01701, SIGMA_TOL),
    },
    Check {
        text: "Freire: σ at β = 0.4535645",
        measure: Measure::Sigma(0.4535645),
        expect: near(0.28883, SIGMA_TOL),
    },
    Check {
        text: "Freire: curve at β = 0.6, gap over sagitta",
        measure: Measure::CurveGap(0.6),
        expect: Expect::AtMost { bound: 1.0 },
    },
    Check {
        text: "Freire: curve at β = 0.4535645, gap over sagitta",
        measure: Measure::CurveGap(0.4535645),
        expect: Expect::AtMost { bound: 1.0 },
    },
    exactly(
        "Freire: chaotic at β = 0.6 (1 = yes)",
        Measure::Chaotic(0.6),
        0,
    ),
    exactly(
        "Freire: chaotic at β = 0.4535645 (1 = yes)",
        Measure::Chaotic(0.4535645),
        1,
    ),
    shrunk(
        "Freire: shrunk end to BT, fraction of box",
        &FREIRE,
        FREIRE.bt,
    ),
    exactly("Freire: shrunk ends", Measure::ShrunkEnds(&FREIRE), 1),
    exactly(
        "Freire: reversed time (1 = yes)",
        Measure::Reversed(&FREIRE),
        0,
    ),
    shrunk(
        "Lorenz-84: shrunk end to BT, fraction of box",
        &LORENZ,
        LORENZ.bt,
    ),
    // From BT to its mirror at −T, both inside the box.
    exactly("Lorenz-84: shrunk ends", Measure::ShrunkEnds(&LORENZ), 2),
    exactly(
        "Lorenz-84: reversed time (1 = yes)",
        Measure::Reversed(&LORENZ),
        1,
    ),
    shrunk("CO: shrunk end to BT₁, fraction of box", &CO, CO.bt),
];

const WRONG_CHECKS: &[Check] = &[
    Check {
        text: "wrong: Freire ν at β = 0.6 is −0.72132 (brief)",
        measure: Measure::Nu(0.6),
        expect: near(-0.72132, NU_TOL),
    },
    Check {
        text: "wrong: Freire ν at β = 0.4535645 is the subsidiary",
        measure: Measure::Nu(0.4535645),
        expect: near(-0.7269563, NU_TOL),
    },
    exactly(
        "wrong: Freire σ > 0 at β = 0.6 (1 = yes)",
        Measure::SigmaPositive(0.6),
        1,
    ),
    exactly(
        "wrong: Freire chaotic at β = 0.6 (1 = yes)",
        Measure::Chaotic(0.6),
        1,
    ),
    shrunk("wrong: CO's curve from BT₁ ends at BT₂", &CO, CO_BT2),
    exactly(
        "wrong: Freire shrinks at both ends",
        Measure::ShrunkEnds(&FREIRE),
        2,
    ),
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

struct Followed {
    system: Arc<dyn_expr::CompiledSystem>,
    req: HomoclinicRequest,
    curve: HomoclinicOut,
    elapsed_ms: f64,
}

fn compile(run: &Run) -> Arc<dyn_expr::CompiledSystem> {
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
    store()
        .get(res.system_id.expect("compiled"))
        .expect("stored")
}

fn curve(run: &'static Run) -> Arc<Followed> {
    static CURVES: OnceLock<Mutex<HashMap<&'static str, Arc<Followed>>>> = OnceLock::new();
    let curves = CURVES.get_or_init(Default::default);
    if let Some(done) = curves.lock().unwrap().get(run.label) {
        return done.clone();
    }
    let system = compile(run);
    let index = |name: &str| {
        system
            .info
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| panic!("{}: no parameter {name}", run.label))
    };
    let free = (index(run.across), index(run.up));
    let mut params: Vec<f64> = system.info.params.iter().map(|p| p.default).collect();
    params[free.0] = run.bt.0;
    params[free.1] = run.bt.1;
    let req = HomoclinicRequest {
        request_id: 1,
        system_id: 0,
        params,
        free,
        ranges: run.ranges,
        x: run.x.to_vec(),
        direction: ContinuationDirection::Both,
        ds: UI_DS,
        ds_max: 2.0 * UI_DS,
        max_points: UI_MAX_POINTS,
    };
    let res = homoclinic_service::compute_homoclinic(&system, &req, &|| false, &mut |_| {})
        .unwrap_or_else(|e| panic!("{}: {e}", run.label))
        .expect("not cancelled");
    let out = Arc::new(Followed {
        system,
        req,
        curve: res.curve,
        elapsed_ms: res.elapsed_ms,
    });
    curves.lock().unwrap().insert(run.label, out.clone());
    out
}

/// The Freire circuit's homoclinic orbits at `beta` in order of return
/// time, with `ν` scanned within [`NU_WINDOW`] of the curve's `ν` between
/// its two points that bracket `beta`, from the saddle there; none if no two
/// points bracket it.
fn orbits_at(beta: f64) -> Arc<Vec<HomoclinicOrbit>> {
    static ORBITS: OnceLock<Mutex<HashMap<u64, Arc<Vec<HomoclinicOrbit>>>>> = OnceLock::new();
    let orbits = ORBITS.get_or_init(Default::default);
    if let Some(done) = orbits.lock().unwrap().get(&beta.to_bits()) {
        return done.clone();
    }
    let c = curve(&FREIRE);
    let (p1, p2) = (&c.curve.p1, &c.curve.p2);
    let found = bracket(p2, beta).map_or_else(Vec::new, |i| {
        let t = (beta - p2[i]) / (p2[i + 1] - p2[i]);
        let nu = p1[i] + t * (p1[i + 1] - p1[i]);
        let mut params = c.req.params.clone();
        params[c.req.free.0] = nu - NU_WINDOW;
        params[c.req.free.1] = beta;
        dyn_analysis::homoclinic_at(
            &*c.system,
            &params,
            &c.curve.x[3 * i..3 * i + 3],
            c.req.free.0,
            (nu - NU_WINDOW, nu + NU_WINDOW),
            NU_SAMPLES,
            &|| false,
        )
        .expect("the scan runs")
    });
    let found = Arc::new(found);
    orbits.lock().unwrap().insert(beta.to_bits(), found.clone());
    found
}

/// The primary orbit at `beta`: the shortest return time.
fn primary(beta: f64) -> Option<HomoclinicOrbit> {
    orbits_at(beta).first().cloned()
}

fn measure(check: &Check) -> f64 {
    let missing = f64::NAN;
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let nu = curve(&FREIRE).req.free.0;
    match check.measure {
        Measure::Nu(beta) => primary(beta).map_or(missing, |o| o.params[nu]),
        Measure::Subsidiary(beta) => {
            let found = orbits_at(beta);
            found
                .iter()
                .find(|o| o.return_time > 1.01 * found[0].return_time)
                .map_or(missing, |o| o.params[nu])
        }
        Measure::Sigma(beta) => primary(beta).map_or(missing, |o| o.saddle.sigma),
        Measure::SigmaPositive(beta) => {
            primary(beta).map_or(missing, |o| flag(o.saddle.sigma > 0.0))
        }
        Measure::Chaotic(beta) => primary(beta).map_or(missing, |o| flag(o.saddle.chaotic)),
        Measure::Shrunk { run, to } => {
            let c = &curve(run).curve;
            let n = c.p1.len();
            [(c.end.first, 0), (Some(c.end.last), n - 1)]
                .into_iter()
                .filter(|(stop, _)| *stop == Some(HomoclinicStop::Shrunk))
                .map(|(_, i)| {
                    let ((lo1, hi1), (lo2, hi2)) = run.ranges;
                    ((c.p1[i] - to.0) / (hi1 - lo1))
                        .abs()
                        .max(((c.p2[i] - to.1) / (hi2 - lo2)).abs())
                })
                .reduce(f64::min)
                .unwrap_or(missing)
        }
        Measure::Reversed(run) => flag(curve(run).curve.reversed),
        Measure::ShrunkEnds(run) => {
            let end = &curve(run).curve.end;
            [end.first, Some(end.last)]
                .iter()
                .filter(|stop| **stop == Some(HomoclinicStop::Shrunk))
                .count() as f64
        }
        Measure::CurveGap(beta) => {
            let c = &curve(&FREIRE).curve;
            match (bracket(&c.p2, beta), primary(beta)) {
                (Some(i), Some(o)) if i > 0 && i + 2 < c.p1.len() => {
                    let pt = |j: usize| (c.p1[j], c.p2[j]);
                    let t = (beta - c.p2[i]) / (c.p2[i + 1] - c.p2[i]);
                    let drawn = c.p1[i] + t * (c.p1[i + 1] - c.p1[i]);
                    let kappa = curvature(pt(i - 1), pt(i), pt(i + 1)).max(curvature(
                        pt(i),
                        pt(i + 1),
                        pt(i + 2),
                    ));
                    let (d1, d2) = (c.p1[i + 1] - c.p1[i], c.p2[i + 1] - c.p2[i]);
                    let len = d1.hypot(d2);
                    let sagitta = kappa * len * len / 8.0 * len / d2.abs();
                    (drawn - o.params[nu]).abs() / sagitta
                }
                _ => missing,
            }
        }
    }
}

/// The first segment `i → i + 1` of `p2` whose ends bracket `at`.
fn bracket(p2: &[f64], at: f64) -> Option<usize> {
    (0..p2.len() - 1).find(|&i| (p2[i] - at) * (p2[i + 1] - at) <= 0.0)
}

/// The curvature of the circle through three points.
fn curvature(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let side = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1);
    2.0 * cross.abs() / (side(a, b) * side(b, c) * side(a, c))
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
    let pad = " ".repeat(54usize.saturating_sub(claim.chars().count()));
    println!("{claim}{pad} {reference:>16} {measured:>16} {delta:>8} {tol:>8} {verdict}");
}

fn print_table(rows: &[Row]) {
    println!();
    print_line(["claim", "reference", "measured", "Δ", "tol", "result"]);
    for r in rows {
        let (reference, measured, tol) = match *r.expect {
            Expect::Near { value, tol } => (
                format!("{value:.10}"),
                format!("{:.10}", r.measured),
                format!("{tol:.0e}"),
            ),
            Expect::AtMost { bound } => (
                "0".to_string(),
                format!("{:.3e}", r.measured),
                format!("≤ {bound:.1e}"),
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

/// Each run's start, box and curve, as the table's claims use them.
fn print_runs() {
    println!(
        "\nEach curve: Δs = {UI_DS}, Δs max = {}, at most {UI_MAX_POINTS} points each way, both \
         directions, as the Parameter plane sends them.",
        2.0 * UI_DS
    );
    for run in RUNS {
        let c = curve(run);
        let ((lo1, hi1), (lo2, hi2)) = run.ranges;
        let (p1, p2) = (&c.curve.p1, &c.curve.p2);
        let n = p1.len();
        println!(
            "  {:<13} from BT ({}, {}) = ({}, {}); box {} ∈ [{lo1}, {hi1}], {} ∈ [{lo2}, {hi2}]; \
             {n} points in {:.1} s, ends {:?} at ({:.8}, {:.8}) and {:?} at ({:.8}, {:.8})",
            run.label,
            run.across,
            run.up,
            run.bt.0,
            run.bt.1,
            run.across,
            run.up,
            c.elapsed_ms / 1e3,
            c.curve.end.first,
            p1[0],
            p2[0],
            c.curve.end.last,
            p1[n - 1],
            p2[n - 1],
        );
    }
}

#[test]
fn homoclinic_claims_reproduce() {
    let started = Instant::now();
    let rows: Vec<Row> = CHECKS.iter().map(evaluate).collect();
    print_runs();
    print_table(&rows);
    println!(
        "{} rows, wall time {:.2} s",
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
fn wrong_homoclinic_claims_fail() {
    let rows: Vec<Row> = WRONG_CHECKS.iter().map(evaluate).collect();
    print_table(&rows);
    let passed: Vec<&str> = rows.iter().filter(|r| r.passes()).map(|r| r.text).collect();
    assert!(passed.is_empty(), "wrong claims not failed: {passed:?}");
}
