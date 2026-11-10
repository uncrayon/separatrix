//! `compute_curve` through the service layer, seeded the way the Parameter
//! plane seeds it: the gallery's CO oxidation model and predator–prey map
//! against docs/codim2-reference.md and closed forms, and against the
//! browser preview's mock curves.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use dyn_expr::CompiledSystem;
use separatrix_lib::ipc::{
    BranchEndOut, BranchOut, BranchStop, Codim2KindOut, CompileRequest, ContinuationDirection,
    ContinuationRequest, ContinuationStart, CurveKindOut, CurveOut, CurveRequest, CurveSeed,
    InputFormat, Severity, SpecialKindOut, SpecialPointOut, MAX_CONTINUATION_POINTS,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");
const MOCK_CURVES_JSON: &str = include_str!("../../src/api/mockCurves.generated.json");

/// The Parameter plane's defaults (app/src/state/curves.ts): `ds`,
/// `dsMax = 2·ds` and `maxPoints`.
const UI_DS: f64 = 0.01;
const UI_MAX_POINTS: usize = 2000;

/// The digits docs/codim2-reference.md gives, as crates/dyn-analysis/tests/curves.rs
/// checks them.
const REFERENCE_TOL: f64 = 1e-8;

type Box2 = ((f64, f64), (f64, f64));

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    id: String,
    format: InputFormat,
    equations: Vec<String>,
    #[serde(default)]
    params: HashMap<String, f64>,
    #[serde(default)]
    param_ranges: HashMap<String, (f64, f64)>,
}

fn store() -> &'static SystemStore {
    static STORE: OnceLock<SystemStore> = OnceLock::new();
    STORE.get_or_init(SystemStore::new)
}

struct System {
    compiled: Arc<CompiledSystem>,
    id: u32,
    names: Vec<String>,
    /// The template's values, else the defaults.
    values: Vec<f64>,
    /// The template's slider ranges, which the Parameter plane opens on.
    ranges: Vec<(f64, f64)>,
}

fn compile(source: &str) -> System {
    compile_as(source.to_string(), InputFormat::Text)
}

fn compile_as(source: String, format: InputFormat) -> System {
    let res = service::compile_system(store(), &CompileRequest { source, format });
    assert!(
        res.ok
            && !res
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
        "{:?}",
        res.diagnostics
    );
    let id = res.system_id.expect("compiled");
    System {
        compiled: store().get(id).expect("stored"),
        id,
        names: res.params.iter().map(|p| p.name.clone()).collect(),
        values: res.params.iter().map(|p| p.default).collect(),
        ranges: Vec::new(),
    }
}

fn template(id: &str) -> System {
    let templates: Vec<Template> = serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
    let t = templates
        .into_iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("no template {id:?}"));
    let mut sys = compile_as(t.equations.join("\n"), t.format);
    sys.values = sys.names.iter().map(|n| t.params[n]).collect();
    sys.ranges = sys.names.iter().map(|n| t.param_ranges[n]).collect();
    sys
}

impl System {
    fn param(&self, name: &str) -> usize {
        self.names
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("no parameter {name} in {:?}", self.names))
    }

    fn plane(&self, free: (usize, usize)) -> Box2 {
        (self.ranges[free.0], self.ranges[free.1])
    }

    /// The Branch view's run in `params[param]` over its slider, both ways
    /// from `x0`, with that view's defaults.
    fn branch(&self, param: usize, x0: &[f64]) -> BranchOut {
        let range = self.ranges[param];
        let ds = (range.1 - range.0) / 100.0;
        let req = ContinuationRequest {
            request_id: 1,
            system_id: self.id,
            params: self.values.clone(),
            param,
            range,
            start: ContinuationStart::Point {
                x0: x0.to_vec(),
                period: 1,
            },
            direction: ContinuationDirection::Both,
            ds,
            ds_max: 2.0 * ds,
            max_points: 500,
        };
        service::compute_continuation(&self.compiled, &req)
            .expect("the branch")
            .branch
    }

    /// The request the Parameter plane sends for `seed` at `params`.
    fn request(
        &self,
        params: Vec<f64>,
        free: (usize, usize),
        ranges: Box2,
        seed: CurveSeed,
    ) -> CurveRequest {
        CurveRequest {
            request_id: 1,
            system_id: self.id,
            params,
            free,
            ranges,
            seed,
            direction: ContinuationDirection::Both,
            ds: UI_DS,
            ds_max: 2.0 * UI_DS,
            max_points: UI_MAX_POINTS,
        }
    }

    /// The seed `planeSeed` (app/src/state/curves.ts) makes of a special
    /// point of a branch in `params[param]`, with its parameters.
    fn plane_seed(
        &self,
        at: &SpecialPointOut,
        kind: CurveKindOut,
        param: usize,
    ) -> (Vec<f64>, CurveSeed) {
        let mut params = self.values.clone();
        params[param] = at.p;
        let seed = CurveSeed {
            kind,
            x: at.x.clone(),
            period: at.period,
        };
        (params, seed)
    }

    fn curve(&self, req: &CurveRequest) -> CurveOut {
        service::compute_curve(&self.compiled, req)
            .unwrap_or_else(|e| panic!("{e}"))
            .curve
    }

    fn refusal(&self, req: &CurveRequest) -> String {
        match service::compute_curve(&self.compiled, req) {
            Ok(res) => panic!("not refused: {:?}", res.curve.end),
            Err(e) => e,
        }
    }
}

fn special(branch: &BranchOut, kind: SpecialKindOut) -> Vec<&SpecialPointOut> {
    branch.special.iter().filter(|sp| sp.kind == kind).collect()
}

fn nearest_special(branch: &BranchOut, kind: SpecialKindOut, p: f64) -> &SpecialPointOut {
    special(branch, kind)
        .into_iter()
        .min_by(|a, b| (a.p - p).abs().total_cmp(&(b.p - p).abs()))
        .unwrap_or_else(|| panic!("no {kind:?} near {p}"))
}

fn state(c: &CurveOut, i: usize) -> &[f64] {
    let dim = c.x.len() / c.p1.len();
    &c.x[i * dim..(i + 1) * dim]
}

fn on_edge(c: &CurveOut, i: usize, ((lo1, hi1), (lo2, hi2)): Box2) -> bool {
    [lo1, hi1].contains(&c.p1[i]) || [lo2, hi2].contains(&c.p2[i])
}

const BOTH_IN_RANGE: BranchEndOut = BranchEndOut {
    first: Some(BranchStop::Range),
    last: BranchStop::Range,
};

#[test]
fn co_oxidation_curves_cross_k_0_4_at_the_reference_folds_and_hopf_points() {
    let co = template("co-oxidation");
    let (k2, k) = (co.param("k_2"), co.param("K"));
    let branch = co.branch(k2, &[0.02, 0.5, 0.33]);
    let cases = [
        (
            SpecialKindOut::Fold,
            CurveKindOut::Fold,
            [1.04204851, 1.05220029],
        ),
        (
            SpecialKindOut::Hopf,
            CurveKindOut::Hopf,
            [1.04099157, 1.05155746],
        ),
    ];
    let below = (co.ranges[k2], (co.ranges[k].0, 0.4));
    let mut largest = 0.0f64;
    for (found, kind, reference) in cases {
        let seeds = special(&branch, found);
        assert_eq!(seeds.len(), 2, "{found:?} at K = 0.4");
        for at in seeds {
            let (params, seed) = co.plane_seed(at, kind, k2);
            // With K = 0.4 the top of the box, the far crossing is a point
            // corrected on that edge, not one interpolated between steps.
            let c = co.curve(&co.request(params.clone(), (k2, k), below, seed.clone()));
            assert_eq!(c.end, BOTH_IN_RANGE, "{kind:?} from k₂ = {}", at.p);
            let mut crossings: Vec<f64> = (0..c.p1.len())
                .filter(|&i| c.p2[i] == 0.4)
                .map(|i| c.p1[i])
                .collect();
            crossings.sort_by(f64::total_cmp);
            assert_eq!(crossings.len(), 2, "{kind:?} at K = 0.4: {crossings:?}");
            for (got, want) in crossings.iter().zip(reference) {
                largest = largest.max((got - want).abs());
                assert!(
                    (got - want).abs() <= REFERENCE_TOL,
                    "{kind:?} from k₂ = {}: crosses K = 0.4 at {got}, reference {want}",
                    at.p
                );
            }

            let plane = co.plane((k2, k));
            let c = co.curve(&co.request(params, (k2, k), plane, seed));
            let n = c.p1.len();
            assert_eq!(c.end, BOTH_IN_RANGE, "{kind:?} from k₂ = {}", at.p);
            assert!(on_edge(&c, 0, plane) && on_edge(&c, n - 1, plane));
            let seed_at = (0..n).find(|&i| c.p2[i] == 0.4).expect("the seed");
            let switches = c.neutral.windows(2).filter(|w| w[0] != w[1]).count();
            if kind == CurveKindOut::Hopf {
                assert!(
                    c.neutral[0] && c.neutral[n - 1] && !c.neutral[seed_at] && switches == 2,
                    "a neutral stretch past each Bogdanov–Takens point"
                );
            } else {
                assert!(c.neutral.iter().all(|v| !v), "no neutral fold");
            }
        }
    }
    println!("CO oxidation: worst |k₂ − reference| at K = 0.4 is {largest:.1e}");
}

#[test]
fn a_seed_outside_the_box_is_that_point_alone() {
    let co = template("co-oxidation");
    let (k2, k) = (co.param("k_2"), co.param("K"));
    let branch = co.branch(k2, &[0.02, 0.5, 0.33]);
    let at = nearest_special(&branch, SpecialKindOut::Fold, 1.042);
    let (params, seed) = co.plane_seed(at, CurveKindOut::Fold, k2);
    let above = (co.ranges[k2], (0.5, co.ranges[k].1));
    let c = co.curve(&co.request(params, (k2, k), above, seed));
    assert_eq!(c.end, BOTH_IN_RANGE);
    assert_eq!(c.p2, vec![0.4]);
    assert!(
        (c.p1[0] - 1.04204851).abs() <= REFERENCE_TOL,
        "k₂ = {}",
        c.p1[0]
    );
}

/// F3 of the predator–prey map, reference system 3.
fn f3(epsilon: f64, a: f64) -> [f64; 2] {
    let x = 1.0 / (3.5 - epsilon);
    [x, 3.5 * x * (a * (1.0 - x) - 1.0) / 3.0]
}

fn worst(c: &CurveOut, error: impl Fn(f64, f64, &[f64]) -> f64) -> f64 {
    (0..c.p1.len())
        .map(|i| error(c.p1[i], c.p2[i], state(c, i)))
        .fold(0.0, f64::max)
}

#[test]
fn predator_prey_neimark_sacker_curve_is_its_closed_form_and_neutral_past_r2() {
    let pp = template("predator-prey-map");
    let (a, e) = (pp.param("a"), pp.param("varepsilon"));
    let branch = pp.branch(a, &f3(1.0, 5.3));
    let at = nearest_special(&branch, SpecialKindOut::NeimarkSacker, 5.0);
    assert!((at.p - 5.0).abs() <= REFERENCE_TOL, "NS at a = {}", at.p);
    let (params, seed) = pp.plane_seed(at, CurveKindOut::NeimarkSacker, a);
    let c = pp.curve(&pp.request(params, (a, e), pp.plane((a, e)), seed));
    assert_eq!(c.end, BOTH_IN_RANGE);
    let error = worst(&c, |a, epsilon, x| {
        let [x0, y0] = f3(epsilon, a);
        let want = (2.0 * epsilon - 7.0) / (2.0 * epsilon - 3.0);
        (a - want)
            .abs()
            .max((x[0] - x0).abs())
            .max((x[1] - y0).abs())
    });
    println!("NS curve: worst |a − (2ε − 7)/(2ε − 3)|, |x − F3| is {error:.1e}");
    assert!(error <= REFERENCE_TOL, "{error:e}");
    let r2 = 35.0 / 26.0;
    for i in 0..c.p1.len() {
        assert_eq!(c.neutral[i], c.p2[i] > r2, "at ε = {}", c.p2[i]);
    }
}

/// F3 crosses F2 = ((a − 1)/a, 0) on the invariant line `y = 0` where
/// `a = (2ε − 7)/(2ε − 5)`; there the multipliers are 1 and `2 − a`, and the
/// second is −1 at the fold-flip `(a, ε) = (3, 2)`.
#[test]
fn predator_prey_branch_point_curve_is_where_f3_crosses_f2_through_the_fold_flip() {
    let pp = template("predator-prey-map");
    let (a, e) = (pp.param("a"), pp.param("varepsilon"));
    let branch = pp.branch(a, &[1.0 - 1.0 / 5.3, 0.0]);
    let at = nearest_special(&branch, SpecialKindOut::BranchPoint, 5.0 / 3.0);
    assert!(
        (at.p - 5.0 / 3.0).abs() <= REFERENCE_TOL,
        "BP at a = {}",
        at.p
    );
    let (params, seed) = pp.plane_seed(at, CurveKindOut::BranchPoint, a);
    let plane = pp.plane((a, e));
    let c = pp.curve(&pp.request(params.clone(), (a, e), plane, seed.clone()));
    assert_eq!(c.end, BOTH_IN_RANGE);
    let error = worst(&c, |a, epsilon, x| {
        let want = (2.0 * epsilon - 7.0) / (2.0 * epsilon - 5.0);
        let x0 = 1.0 / (3.5 - epsilon);
        (a - want).abs().max((x[0] - x0).abs()).max(x[1].abs())
    });
    println!("BP curve: worst |a − (2ε − 7)/(2ε − 5)|, |x − (1/(3.5 − ε), 0)| is {error:.1e}");
    assert!(error <= REFERENCE_TOL, "{error:e}");
    let flip = (1..c.p1.len())
        .find(|&i| (3.0 - c.p1[i - 1]) * (3.0 - c.p1[i]) < 0.0)
        .expect("the multiplier 2 − a passes −1");
    let (below, above) = (
        c.p2[flip - 1].min(c.p2[flip]),
        c.p2[flip - 1].max(c.p2[flip]),
    );
    assert!(
        below < 2.0 && 2.0 < above,
        "the fold-flip at ε = 2 is not between ε = {below} and {above}"
    );

    let fold = CurveSeed {
        kind: CurveKindOut::Fold,
        ..seed
    };
    assert_eq!(
        pp.refusal(&pp.request(params, (a, e), plane, fold)),
        "The curve of folds has no unique direction at the seed, so it can't be followed from \
         there."
    );
}

fn loop_request(sys: &System, params: Vec<f64>) -> CurveRequest {
    let seed = CurveSeed {
        kind: CurveKindOut::Fold,
        x: vec![0.0, 0.0],
        period: 1,
    };
    sys.request(params, (0, 1), ((-2.0, 2.0), (-2.0, 2.0)), seed)
}

/// Ends `closed` with the first point repeated, every point on `on = 0`,
/// once around the origin.
fn assert_closes_once(c: &CurveOut, on: impl Fn(f64, f64) -> f64) {
    let n = c.p1.len();
    assert_eq!(
        c.end,
        BranchEndOut {
            first: None,
            last: BranchStop::Closed
        }
    );
    assert_eq!(
        (c.p1[n - 1], c.p2[n - 1], state(c, n - 1)),
        (c.p1[0], c.p2[0], state(c, 0))
    );
    let tau = std::f64::consts::TAU;
    let turn: f64 = (1..n)
        .map(|i| {
            let d = c.p2[i].atan2(c.p1[i]) - c.p2[i - 1].atan2(c.p1[i - 1]);
            d - tau * (d / tau).round()
        })
        .sum();
    assert!(
        (turn.abs() - tau).abs() <= 1e-9,
        "turns {turn} around the origin"
    );
    for i in 0..n {
        let off = on(c.p1[i], c.p2[i]);
        assert!(off.abs() <= 1e-10, "{off:e} off the loop at {i}");
    }
}

#[test]
fn a_closed_curve_comes_back_once_without_a_backward_half() {
    let sys = compile("x' = a^2 + b^2 - 1 + x^2\ny' = -y");
    let c = sys.curve(&loop_request(&sys, vec![0.8, 0.6]));
    assert_closes_once(&c, |a, b| a * a + b * b - 1.0);
}

/// A loop whose halves take different steps: with `max_points` between
/// their lengths only the backward half closes. It is the whole curve, run
/// the forward way, from the seed with `a` increasing.
#[test]
fn a_backward_half_that_closes_is_the_whole_curve() {
    let sys = compile("x' = a^2 + 4*b^2 + a*b^3 - 1 + x^2\ny' = -y");
    let on = |a: f64, b: f64| a * a + 4.0 * b * b + a * b * b * b - 1.0;
    let mut b: f64 = 0.4;
    for _ in 0..50 {
        b -= on(0.6, b) / (8.0 * b + 1.8 * b * b);
    }
    let req = CurveRequest {
        ds: 0.003,
        ds_max: 0.05,
        ..loop_request(&sys, vec![0.6, b])
    };
    let length = |direction| {
        let half = CurveRequest {
            direction,
            max_points: MAX_CONTINUATION_POINTS,
            ..req.clone()
        };
        sys.curve(&half).p1.len()
    };
    let forward = length(ContinuationDirection::Forward);
    let backward = length(ContinuationDirection::Backward);
    assert!(
        backward < forward,
        "halves of {forward} and {backward} points"
    );
    let c = sys.curve(&CurveRequest {
        max_points: backward,
        ..req
    });
    assert_eq!(c.p1.len(), backward);
    assert!(c.p1[1] > c.p1[0], "runs the forward way");
    assert_closes_once(&c, on);
}

/// The same loop as a Hopf curve whose first Lyapunov coefficient is `b`,
/// so it has Bautin points at `(a, b) = (1, 0)` and `(−1, 0)`, met in that
/// order the forward way. The backward half closes first again; reversed,
/// it keeps each point at its own vertex and in arclength order.
#[test]
fn a_backward_half_that_closes_keeps_its_points_at_their_vertices() {
    let sys = compile(
        "x' = (a^2 + 4*b^2 + a*b^3 - 1)*x - y + b*x*(x^2 + y^2)\n\
         y' = x + (a^2 + 4*b^2 + a*b^3 - 1)*y + b*y*(x^2 + y^2)",
    );
    let on = |a: f64, b: f64| a * a + 4.0 * b * b + a * b * b * b - 1.0;
    let mut b: f64 = 0.4;
    for _ in 0..50 {
        b -= on(0.6, b) / (8.0 * b + 1.8 * b * b);
    }
    let req = CurveRequest {
        seed: CurveSeed {
            kind: CurveKindOut::Hopf,
            x: vec![0.0, 0.0],
            period: 1,
        },
        ds: 0.003,
        ds_max: 0.05,
        ..loop_request(&sys, vec![0.6, b])
    };
    let length = |direction| {
        let half = CurveRequest {
            direction,
            max_points: MAX_CONTINUATION_POINTS,
            ..req.clone()
        };
        sys.curve(&half).p1.len()
    };
    let forward = length(ContinuationDirection::Forward);
    let backward = length(ContinuationDirection::Backward);
    assert!(
        backward < forward,
        "halves of {forward} and {backward} points"
    );
    let c = sys.curve(&CurveRequest {
        max_points: backward,
        ..req
    });
    assert_closes_once(&c, on);
    let found: Vec<(Codim2KindOut, (f64, f64))> =
        c.special.iter().map(|sp| (sp.kind, sp.p)).collect();
    assert_eq!(found.len(), 2, "{found:?}");
    for (sp, want) in c.special.iter().zip([(1.0, 0.0), (-1.0, 0.0)]) {
        assert_eq!(sp.kind, Codim2KindOut::Bautin);
        assert_eq!((c.p1[sp.index], c.p2[sp.index]), sp.p, "at its vertex");
        assert!(
            (sp.p.0 - want.0).abs() <= 1e-9 && sp.p.1.abs() <= 1e-9,
            "{:?}, not {want:?}",
            sp.p
        );
    }
    assert!(c.special[0].index < c.special[1].index);
}

#[test]
fn bad_requests_get_their_sentences() {
    let co = template("co-oxidation");
    let (k2, k) = (co.param("k_2"), co.param("K"));
    let branch = co.branch(k2, &[0.02, 0.5, 0.33]);
    let at = nearest_special(&branch, SpecialKindOut::Fold, 1.042);
    let (params, seed) = co.plane_seed(at, CurveKindOut::Fold, k2);
    let good = co.request(params, (k2, k), co.plane((k2, k)), seed);
    co.curve(&good);
    let with = |change: fn(&mut CurveRequest)| {
        let mut req = good.clone();
        change(&mut req);
        req
    };
    let range = "Both parameter ranges must be finite, each with its start below its end.";
    let points = "A curve needs 2 to 20000 points in each direction.";
    let bad = [
        (
            with(|r| r.free = (r.free.0, r.free.0)),
            "The two free parameters must be different.",
        ),
        (
            with(|r| r.free = (r.free.0, 2)),
            "Parameter 2 doesn't exist; the system has 2.",
        ),
        (with(|r| r.ranges.1 = (0.4, 0.4)), range),
        (with(|r| r.ranges.0 = (2.5, 0.5)), range),
        (with(|r| r.ranges.1 = (0.0, f64::INFINITY)), range),
        (with(|r| r.max_points = 1), points),
        (with(|r| r.max_points = MAX_CONTINUATION_POINTS + 1), points),
        (
            with(|r| r.seed.x.truncate(2)),
            "The seed has 2 coordinate(s) but the system has 3.",
        ),
        (
            with(|r| r.params.truncate(1)),
            "Expected 2 parameter value(s) but got 1.",
        ),
        (
            with(|r| r.params[0] = f64::NAN),
            "The seed and the parameters must be finite.",
        ),
        (
            with(|r| r.seed.period = 0),
            "The period must be at least 1.",
        ),
        (
            with(|r| r.seed.period = 2),
            "A flow's equilibria have no period; use period 1.",
        ),
        (
            with(|r| r.ds = 0.0),
            "The step ds must be a positive number.",
        ),
        (
            with(|r| r.ds_max = 0.5 * r.ds),
            "The largest step must be a number at least ds.",
        ),
    ];
    for (req, sentence) in bad {
        assert_eq!(co.refusal(&req), sentence);
    }
}

#[test]
fn curves_the_engine_cannot_follow_get_their_sentences() {
    let pp = template("predator-prey-map");
    let (a, e) = (pp.param("a"), pp.param("varepsilon"));
    let mut params = pp.values.clone();
    params[a] = 5.0;
    let hopf = CurveSeed {
        kind: CurveKindOut::Hopf,
        x: f3(1.0, 5.0).to_vec(),
        period: 1,
    };
    assert_eq!(
        pp.refusal(&pp.request(params.clone(), (a, e), pp.plane((a, e)), hopf)),
        "Curves in two parameters follow folds, branch points and Hopf points of a flow, and \
         folds, branch points, flips and Neimark–Sacker points of a map; Hopf and \
         Neimark–Sacker points need two or more states."
    );
    // F2 = ((a − 1)/a, 0) flips where its multiplier 2 − a is −1, at a = 3
    // whatever ε is. The sentence names ε by its LaTeX, for the interface to
    // spell out.
    params[a] = 3.0;
    let flip = CurveSeed {
        kind: CurveKindOut::Flip,
        x: vec![2.0 / 3.0, 0.0],
        period: 1,
    };
    assert_eq!(
        pp.refusal(&pp.request(params, (a, e), pp.plane((a, e)), flip)),
        "\\varepsilon doesn't move this flip; choose another parameter to follow it in."
    );

    let at_origin = |source: &str, params: Vec<f64>, kind| {
        let sys = compile(source);
        let seed = CurveSeed {
            kind,
            x: vec![0.0; sys.compiled.info.states.len()],
            period: 1,
        };
        sys.refusal(&sys.request(params, (0, 1), ((-1.0, 1.0), (-1.0, 1.0)), seed))
    };
    assert_eq!(
        at_origin(
            "x' = r + x^2\ny' = -c*y",
            vec![0.0, 1.0],
            CurveKindOut::Fold
        ),
        "c doesn't move this fold; choose another parameter to follow it in."
    );
    assert_eq!(
        at_origin(
            "x' = p*x - x^2 + q",
            vec![0.0, 0.5],
            CurveKindOut::BranchPoint
        ),
        "Newton's method found no branch point of an equilibrium near the seed with q held; \
         start from the special point the Branch view found."
    );
    assert_eq!(
        at_origin(
            "x' = r + x^2 + t\ny' = -c*y",
            vec![0.0, 1.0],
            CurveKindOut::Fold
        ),
        "Continuation follows fixed points, which a system that depends on t or n doesn't have."
    );
    assert_eq!(
        at_origin("x' = r + x^2", vec![0.0], CurveKindOut::Fold),
        "A curve needs two parameters to follow; this system has 1."
    );
}

#[derive(Deserialize)]
struct MockSystem {
    free: (usize, usize),
    curves: Vec<MockCurve>,
}

#[derive(Deserialize)]
struct MockCurve {
    kind: CurveKindOut,
    period: usize,
    p1: Vec<f64>,
    p2: Vec<f64>,
    x: Vec<f64>,
    neutral: Vec<bool>,
    special: Vec<MockPoint>,
    stops: [String; 2],
}

#[derive(Deserialize)]
struct MockPoint {
    /// As `Codim2KindOut` serializes it.
    kind: String,
    p: (f64, f64),
    x: Vec<f64>,
}

/// A polyline in the coordinates both followers measure arclength in: each
/// parameter as a fraction of its side of the box, then the state.
fn scaled(p1: &[f64], p2: &[f64], x: &[f64], ((lo1, hi1), (lo2, hi2)): Box2) -> Vec<Vec<f64>> {
    let dim = x.len() / p1.len();
    (0..p1.len())
        .map(|i| {
            [p1[i] / (hi1 - lo1), p2[i] / (hi2 - lo2)]
                .into_iter()
                .chain(x[i * dim..(i + 1) * dim].iter().copied())
                .collect()
        })
        .collect()
}

fn sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(a, b)| a - b).collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

fn chords(poly: &[Vec<f64>]) -> impl Iterator<Item = f64> + '_ {
    poly.windows(2).map(|w| norm(&sub(&w[1], &w[0])))
}

/// The point of the polyline nearest `q`: its distance, its segment and its
/// arclength along the polyline.
fn nearest(poly: &[Vec<f64>], q: &[f64]) -> (f64, usize, f64) {
    let mut best = (norm(&sub(q, &poly[0])), 0, 0.0);
    let mut s = 0.0;
    for (j, w) in poly.windows(2).enumerate() {
        let d = sub(&w[1], &w[0]);
        let len = norm(&d);
        let t = if len > 0.0 {
            (dot(&sub(q, &w[0]), &d) / (len * len)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let at: Vec<f64> = w[0].iter().zip(&d).map(|(a, d)| a + t * d).collect();
        let dist = norm(&sub(q, &at));
        if dist < best.0 {
            best = (dist, j, s + t * len);
        }
        s += len;
    }
    best
}

/// The largest turning angle per unit length at a vertex: the polyline's
/// discrete curvature κ.
fn curvature(poly: &[Vec<f64>]) -> f64 {
    poly.windows(3)
        .map(|w| {
            let (u, v) = (sub(&w[1], &w[0]), sub(&w[2], &w[1]));
            let (lu, lv) = (norm(&u), norm(&v));
            (dot(&u, &v) / (lu * lv)).clamp(-1.0, 1.0).acos() / (0.5 * (lu + lv))
        })
        .fold(0.0, f64::max)
}

/// How far a vertex `v` moves when the reference script writes the mock to
/// 8 significant digits.
fn rounding(v: &[f64]) -> f64 {
    5e-8 * norm(v)
}

/// The reference script resamples each mock curve at about this arclength;
/// the engine's parity run takes no longer steps.
const MOCK_H: f64 = 5e-3;

/// The distance, of a set, that comes nearest its bound or past it.
#[derive(Default)]
struct Worst {
    ratio: f64,
    dist: f64,
    bound: f64,
    at: (f64, f64),
}

impl Worst {
    fn add(&mut self, dist: f64, bound: f64, at: (f64, f64)) {
        if dist / bound > self.ratio {
            *self = Worst {
                ratio: dist / bound,
                dist,
                bound,
                at,
            };
        }
    }
}

impl std::fmt::Display for Worst {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{:.2e} of a bound {:.2e} at ({:.6}, {:.6})",
            self.dist, self.bound, self.at.0, self.at.1
        )
    }
}

/// The engine's curve from the mock's middle vertex inside the box,
/// compared both ways with the mock's polyline: a line for the report, and
/// every disagreement.
fn parity(sys: &System, free: (usize, usize), mock: &MockCurve) -> (String, Vec<String>) {
    let plane = sys.plane(free);
    let ((lo1, hi1), (lo2, hi2)) = plane;
    let dim = mock.x.len() / mock.p1.len();
    let inside: Vec<bool> = (0..mock.p1.len())
        .map(|i| (lo1..=hi1).contains(&mock.p1[i]) && (lo2..=hi2).contains(&mock.p2[i]))
        .collect();
    let within: Vec<usize> = (0..inside.len()).filter(|&i| inside[i]).collect();
    let mid = within[within.len() / 2];
    let mut params = sys.values.clone();
    params[free.0] = mock.p1[mid];
    params[free.1] = mock.p2[mid];
    let seed = CurveSeed {
        kind: mock.kind,
        x: mock.x[mid * dim..(mid + 1) * dim].to_vec(),
        period: mock.period,
    };
    let req = CurveRequest {
        ds: MOCK_H / 2.0,
        ds_max: MOCK_H,
        max_points: MAX_CONTINUATION_POINTS,
        ..sys.request(params, free, plane, seed)
    };
    let c = match service::compute_curve(&sys.compiled, &req) {
        Ok(res) => res.curve,
        Err(e) => {
            let seeded = format!("({}, {})", mock.p1[mid], mock.p2[mid]);
            return (
                "refused".into(),
                vec![format!("refused from {seeded}: {e}")],
            );
        }
    };
    let n = c.p1.len();
    let m = scaled(&mock.p1, &mock.p2, &mock.x, plane);
    let e = scaled(&c.p1, &c.p2, &c.x, plane);
    let mut problems = Vec::new();

    // Both polylines are inscribed in the same curve, so a vertex of one
    // lies within the sagitta κℓ²/8 of the other's chord of length ℓ, plus
    // the mock's rounding. κ is the larger of the two polylines' discrete
    // curvatures, as either can sample past the peak of a sharp turn: at the
    // CO oxidation Hopf curve's lowest point the mock gives 95, the engine
    // 106. A vertex at the middle of the other's chord attains the bound.
    let kappa = curvature(&m).max(curvature(&e));
    let sagitta = |len: f64| kappa * len * len / 8.0;
    let mock_chords: Vec<f64> = chords(&m).collect();
    let mut to_mock = Worst::default();
    let (mut compared, mut differ) = (0, Vec::new());
    for (i, q) in e.iter().enumerate() {
        let (dist, j, _) = nearest(&m, q);
        let bound = sagitta(mock_chords[j]) + rounding(&m[j]).max(rounding(&m[j + 1]));
        to_mock.add(dist, bound, (c.p1[i], c.p2[i]));
        let around = &mock.neutral[j.saturating_sub(1)..(j + 3).min(mock.neutral.len())];
        if around.iter().all(|&v| v == around[0]) {
            compared += 1;
            if c.neutral[i] != around[0] {
                differ.push((c.p1[i], c.p2[i]));
            }
        }
    }
    if to_mock.ratio > 1.0 {
        problems.push(format!("an engine point is {to_mock} from the mock"));
    }
    if compared <= n / 2 || !differ.is_empty() {
        problems.push(format!(
            "{} of {compared} neutral flags compared differ, first at {:?}",
            differ.len(),
            differ.first()
        ));
    }

    // The engine's ends as arclengths along the mock, the mock's first end
    // first. Past an end where the mock stopped short of the box, only the
    // stretch both cover is compared.
    let along: Vec<f64> = std::iter::once(0.0)
        .chain(mock_chords.iter().scan(0.0, |s, l| {
            *s += l;
            Some(*s)
        }))
        .collect();
    let (s_first, s_last) = (nearest(&m, &e[0]).2, nearest(&m, &e[n - 1]).2);
    let engine_ends = [
        (c.end.first.unwrap_or(BranchStop::Range), 0, s_first),
        (c.end.last, n - 1, s_last),
    ];
    let ends = if s_first <= s_last {
        engine_ends
    } else {
        [engine_ends[1], engine_ends[0]]
    };
    let range = mock.stops.each_ref().map(|s| s == "range");
    for k in 0..2 {
        let (stop, i, _) = ends[k];
        if range[k] && !(stop == BranchStop::Range && on_edge(&c, i, plane)) {
            problems.push(format!(
                "the mock's {} end reaches the edge; the engine's stops with {stop:?} at ({}, {})",
                ["first", "last"][k],
                c.p1[i],
                c.p2[i]
            ));
        }
    }
    let covered = (
        if range[0] { 0.0 } else { ends[0].2 },
        if range[1] { f64::INFINITY } else { ends[1].2 },
    );
    let step = chords(&e).fold(0.0, f64::max);
    let mut to_engine = Worst::default();
    for (i, v) in m.iter().enumerate() {
        if inside[i] && (covered.0..=covered.1).contains(&along[i]) {
            let bound = sagitta(step) + rounding(v);
            to_engine.add(nearest(&e, v).0, bound, (mock.p1[i], mock.p2[i]));
        }
    }
    if to_engine.ratio > 1.0 {
        problems.push(format!("a mock vertex is {to_engine} from the engine"));
    }

    // Each codimension-2 point of one has a point of the same kind on the
    // other. The mock's are the reference script's to 8 significant digits,
    // and the engine's are within Newton's tolerance 1e-10 · (1 + |z|∞).
    let point = |p: (f64, f64), x: &[f64]| scaled(&[p.0], &[p.1], x, plane).remove(0);
    let engine_points: Vec<(String, Vec<f64>)> = c
        .special
        .iter()
        .map(|sp| (kind_name(sp.kind), point(sp.p, &sp.x)))
        .collect();
    let mock_points: Vec<(String, Vec<f64>)> = mock
        .special
        .iter()
        .map(|sp| (sp.kind.clone(), point(sp.p, &sp.x)))
        .collect();
    let mut points = Worst::default();
    for (from, to, which) in [
        (&mock_points, &engine_points, "the engine"),
        (&engine_points, &mock_points, "the mock"),
    ] {
        for (kind, q) in from {
            let nearest = to
                .iter()
                .filter(|(k, _)| k == kind)
                .map(|(_, v)| norm(&sub(v, q)))
                .min_by(f64::total_cmp);
            let bound = rounding(q) + 1e-10 * (1.0 + q.iter().fold(0.0, |m, v| v.abs().max(m)));
            let at = (q[0] * (hi1 - lo1), q[1] * (hi2 - lo2));
            match nearest {
                Some(d) => points.add(d, bound, at),
                None => problems.push(format!(
                    "{which} has no {kind} point; the other has one at {at:?}"
                )),
            }
        }
    }
    if points.ratio > 1.0 {
        problems.push(format!("a codimension-2 point is {points} from its match"));
    }

    let at = |i: usize| format!("({:.6}, {:.6})", c.p1[i], c.p2[i]);
    let kinds = |pts: &[(String, Vec<f64>)]| {
        pts.iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let row = format!(
        "engine → mock {to_mock} ({:.2}); mock → engine {to_engine} ({:.2}); κ = {kappa:.1}; \
         points: mock [{}], engine [{}], worst {points} ({:.2}); \
         mock stops {:?}; engine {n} points from {} ({:?}) to {} ({:?})",
        to_mock.ratio,
        to_engine.ratio,
        kinds(&mock_points),
        kinds(&engine_points),
        points.ratio,
        mock.stops,
        at(0),
        c.end.first,
        at(n - 1),
        c.end.last
    );
    (row, problems)
}

fn kind_name(kind: Codim2KindOut) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .expect("a unit variant")
}

#[test]
fn the_preview_mock_agrees_with_the_engine() {
    let mocks: HashMap<String, MockSystem> =
        serde_json::from_str(MOCK_CURVES_JSON).expect("mockCurves.generated.json");
    let mut ids: Vec<&String> = mocks.keys().collect();
    ids.sort();
    let mut problems = Vec::new();
    println!();
    for id in ids {
        let sys = template(id);
        let mock = &mocks[id];
        for (k, curve) in mock.curves.iter().enumerate() {
            let label = format!("{id} #{k} {:?}", curve.kind);
            let (row, found) = parity(&sys, mock.free, curve);
            println!("{label}: {row}");
            problems.extend(found.into_iter().map(|p| format!("{label}: {p}")));
        }
    }
    assert!(
        problems.is_empty(),
        "the mock and the engine disagree:\n{}",
        problems.join("\n")
    );
}
