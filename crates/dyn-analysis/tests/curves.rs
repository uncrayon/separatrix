//! Two-parameter curves against docs/codim2-reference.md and closed forms.

use dyn_analysis::{
    continue_branch, continue_curve, Codim2Kind, Codim2Point, ContinuationOptions, Curve, CurveEnd,
    CurveError, CurveKind, CurveOptions, CurvePoint, CurveSeed, Nature, SpecialKind,
};
use dyn_expr::{compile, CompiledSystem, InputFormat};

fn system(source: &str) -> CompiledSystem {
    compile(source, InputFormat::Text)
        .system
        .unwrap_or_else(|| panic!("{source} compiles"))
}

fn param(sys: &CompiledSystem, name: &str) -> usize {
    sys.info
        .params
        .iter()
        .position(|p| p.name == name)
        .unwrap_or_else(|| panic!("parameter {name}"))
}

fn opts(free: (usize, usize), ranges: ((f64, f64), (f64, f64)), ds: f64) -> CurveOptions {
    CurveOptions {
        free,
        ranges,
        ds,
        ds_min: 1e-8,
        ds_max: 0.05,
        max_points: 3000,
        max_angle: 0.3,
    }
}

fn seed(kind: CurveKind, x: &[f64]) -> CurveSeed {
    CurveSeed {
        kind,
        x: x.to_vec(),
        period: 1,
    }
}

fn run(sys: &CompiledSystem, params: &[f64], seed: &CurveSeed, o: &CurveOptions) -> Curve {
    continue_curve(sys, params, seed, o).expect("the curve starts")
}

fn close(got: &[f64], want: &[f64], tol: f64) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(g, w)| (g - w).abs() <= tol)
}

/// Bykov–Yablonskii–Kim, reference system 1, with its parameters `k2` and
/// `K` and the branch at `K = 0.4` from the reference start.
struct Co {
    sys: CompiledSystem,
    k2: usize,
    k: usize,
    params: Vec<f64>,
    branch: dyn_analysis::Branch,
}

fn co() -> Co {
    let sys = system(
        "x' = 2*2.5*(1 - x - y - s)^2 - 2*x^2 - 10*x*y\n\
         y' = k2*(1 - x - y - s) - 0.1*y - 10*x*y\n\
         s' = 0.0675*((1 - x - y - s) - K*s)",
    );
    let (k2, k) = (param(&sys, "k2"), param(&sys, "K"));
    let mut params = vec![0.0; 2];
    params[k2] = 2.204678;
    params[k] = 0.4;
    let o = ContinuationOptions {
        param: k2,
        period: 1,
        range: (0.8, 2.3),
        ds: -0.01,
        ds_min: 1e-8,
        ds_max: 0.05,
        max_points: 2000,
        max_angle: 0.3,
    };
    let branch = continue_branch(&sys, &params, &[0.001137, 0.891483, 0.062345], &o)
        .expect("the branch at K = 0.4");
    Co {
        sys,
        k2,
        k,
        params,
        branch,
    }
}

impl Co {
    /// The curve from the branch's special point of `kind` nearest `k2`.
    fn curve(
        &self,
        kind: SpecialKind,
        k2: f64,
        ranges: ((f64, f64), (f64, f64)),
        ds: f64,
    ) -> Curve {
        let at = self
            .branch
            .special
            .iter()
            .filter(|sp| sp.kind == kind)
            .min_by(|a, b| (a.p - k2).abs().total_cmp(&(b.p - k2).abs()))
            .unwrap_or_else(|| panic!("{kind:?} near k2 = {k2}"));
        let mut params = self.params.clone();
        params[self.k2] = at.p;
        let kind = match kind {
            SpecialKind::Fold => CurveKind::Fold,
            _ => CurveKind::Hopf,
        };
        run(
            &self.sys,
            &params,
            &seed(kind, &at.x),
            &opts((self.k2, self.k), ranges, ds),
        )
    }
}

#[test]
fn co_oxidation_fold_curve_joins_the_two_folds_at_k_0_4_through_the_cusp() {
    let co = co();
    let c = co.curve(SpecialKind::Fold, 1.042, ((0.9, 1.2), (0.3, 0.4)), -0.01);
    assert_eq!(c.end, CurveEnd::Range);
    let (first, last) = (&c.points[0], c.points.last().unwrap());
    assert_eq!(first.p.1, 0.4);
    assert!(
        (first.p.0 - 1.04204851).abs() <= 1e-8,
        "seed k2 = {}",
        first.p.0
    );
    assert!(
        close(&first.x, &[0.02471644, 0.45026004, 0.37501680], 1e-8),
        "{:?}",
        first.x
    );
    assert_eq!(last.p.1, 0.4);
    assert!(
        (last.p.0 - 1.05220029).abs() <= 1e-8,
        "lands at k2 = {}",
        last.p.0
    );
    assert!(
        close(&last.x, &[0.05402942, 0.30224144, 0.45980653], 1e-8),
        "{:?}",
        last.x
    );
    let lowest = c
        .points
        .iter()
        .map(|pt| pt.p.1)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (0.35599127 - 1e-8..0.357).contains(&lowest),
        "the cusp is at K = 0.35599127, the curve's lowest K is {lowest}"
    );
    assert!(c
        .points
        .iter()
        .all(|pt| pt.nature == Nature::Bifurcation { omega: None }));
}

#[test]
fn co_oxidation_hopf_curve_joins_the_two_hopf_points_at_k_0_4_below_the_bautin_points() {
    let co = co();
    let c = co.curve(SpecialKind::Hopf, 1.0516, ((0.5, 1.2), (0.1, 0.4)), -0.01);
    assert_eq!(c.end, CurveEnd::Range);
    let (first, last) = (&c.points[0], c.points.last().unwrap());
    assert_eq!(first.p.1, 0.4);
    assert!(
        (first.p.0 - 1.05155746).abs() <= 1e-8,
        "seed k2 = {}",
        first.p.0
    );
    assert!(
        close(&first.x, &[0.01635754, 0.52397118, 0.32833662], 1e-8),
        "{:?}",
        first.x
    );
    assert_eq!(last.p.1, 0.4);
    assert!(
        (last.p.0 - 1.04099157).abs() <= 1e-8,
        "lands at k2 = {}",
        last.p.0
    );
    assert!(
        close(&last.x, &[0.07792777, 0.23306493, 0.49214807], 1e-8),
        "{:?}",
        last.x
    );
    let lowest = c
        .points
        .iter()
        .map(|pt| pt.p.1)
        .fold(f64::INFINITY, f64::min);
    assert!(
        lowest < 0.23248735,
        "lowest K {lowest} is above the Bautin points"
    );
    for pt in &c.points {
        assert!(
            matches!(pt.nature, Nature::Bifurcation { omega: Some(w) } if w > 0.0),
            "{:?} at {:?}",
            pt.nature,
            pt.p
        );
    }
}

/// The index of the first neutral point; every point before it is a Hopf
/// point and every point from it on is neutral.
fn neutral_from(c: &Curve) -> usize {
    let at = c
        .points
        .iter()
        .position(|pt| pt.nature == Nature::Neutral)
        .expect("a neutral stretch");
    for (i, pt) in c.points.iter().enumerate() {
        assert_eq!(
            pt.nature == Nature::Neutral,
            i >= at,
            "{:?} at {:?}",
            pt.nature,
            pt.p
        );
    }
    at
}

#[test]
fn co_oxidation_hopf_curve_turns_neutral_past_each_bogdanov_takens_point() {
    let co = co();
    let ranges = ((0.5, 2.0), (0.4, 2.0));
    let cases = [
        (1.0410, (1.41762764, 0.97139757), 1.6402536400),
        (1.0516, (1.16119871, 0.72233907), 1.2711072431),
    ];
    for (k2, bt, at_k_2) in cases {
        let c = co.curve(SpecialKind::Hopf, k2, ranges, 0.01);
        assert_eq!(c.end, CurveEnd::Range, "from k2 = {k2}");
        let i = neutral_from(&c);
        let [at] = kinds_at(&c, Codim2Kind::BogdanovTakens)[..] else {
            panic!("one BT on {:?}", c.special);
        };
        assert!(
            at.index == i - 1 || at.index == i,
            "turns neutral at {:?}, not at BT {:?}",
            c.points[i].p,
            at.p
        );
        assert_near(at.p, bt, 1e-6);
        let last = c.points.last().unwrap();
        assert_eq!(last.p.1, 2.0);
        assert!(
            (last.p.0 - at_k_2).abs() <= 1e-8,
            "neutral at k2 = {}",
            last.p.0
        );
    }
}

const PREDATOR_PREY: &str = "x_{n+1} = a*x_n*(1 - x_n) - 3*x_n*y_n/(1 + epsilon*x_n)\n\
                             y_{n+1} = 3.5*x_n*y_n/(1 + epsilon*x_n)";

/// The fixed point `F3` of the predator–prey map, reference system 3.
fn f3(epsilon: f64, a: f64) -> Vec<f64> {
    let x = 1.0 / (3.5 - epsilon);
    vec![x, 3.5 * (a * (1.0 - x) - 1.0) / (3.0 * (3.5 - epsilon))]
}

/// Both halves from `(ε0, a(ε0))` with `ε` across and `a` up, so that the
/// seed is corrected in `ε` at its `a`.
fn predator_prey(
    kind: CurveKind,
    epsilon: f64,
    a_of: fn(f64) -> f64,
    ranges: ((f64, f64), (f64, f64)),
) -> [Curve; 2] {
    let sys = system(PREDATOR_PREY);
    let (ie, ia) = (param(&sys, "epsilon"), param(&sys, "a"));
    let mut params = vec![0.0; 2];
    params[ie] = epsilon;
    params[ia] = a_of(epsilon);
    let s = seed(kind, &f3(epsilon, params[ia]));
    [0.01, -0.01].map(|ds| run(&sys, &params, &s, &opts((ie, ia), ranges, ds)))
}

fn assert_on(c: &Curve, a_of: fn(f64) -> f64) {
    assert_eq!(c.end, CurveEnd::Range);
    for pt in &c.points {
        let (epsilon, a) = pt.p;
        assert!(
            (a - a_of(epsilon)).abs() <= 1e-8,
            "a = {a} vs {} at ε = {epsilon}",
            a_of(epsilon)
        );
        assert!(
            close(&pt.x, &f3(epsilon, a), 1e-8),
            "{:?} at ε = {epsilon}",
            pt.x
        );
    }
}

#[test]
fn predator_prey_neimark_sacker_curve_is_neutral_past_the_1_2_resonance() {
    let ns: fn(f64) -> f64 = |e| (2.0 * e - 7.0) / (2.0 * e - 3.0);
    let [forward, backward] = predator_prey(
        CurveKind::NeimarkSacker,
        1.0,
        ns,
        ((-5.0, 1.45), (0.0, 45.0)),
    );
    for c in [&forward, &backward] {
        assert_on(c, ns);
    }
    assert_eq!(forward.points.last().unwrap().p.0, 1.45);
    assert_eq!(backward.points.last().unwrap().p.0, -5.0);
    let r2 = 35.0 / 26.0;
    let i = neutral_from(&forward);
    assert!(forward.points[i - 1].p.0 < r2 && r2 < forward.points[i].p.0);
    assert!(backward
        .points
        .iter()
        .all(|pt| pt.nature != Nature::Neutral));
}

#[test]
fn predator_prey_flip_curve_matches_its_closed_form() {
    let flip: fn(f64) -> f64 = |e| (4.0 * e * e - 56.0 * e + 147.0) / (4.0 * e * e + 4.0 * e - 7.0);
    let [forward, backward] = predator_prey(CurveKind::Flip, 1.5, flip, ((1.2, 1.9), (0.0, 30.0)));
    for c in [&forward, &backward] {
        assert_on(c, flip);
    }
    assert_eq!(forward.points.last().unwrap().p.0, 1.9);
    assert_eq!(backward.points.last().unwrap().p.0, 1.2);
}

/// The predator–prey map's multiplier +1 curve is where F3 crosses
/// F2 = ((a − 1)/a, 0). The line y = 0 is invariant, so the row of y in
/// `[g_x | g_ε | g_a]` vanishes all along it and the fold system is
/// singular there: a branch point, not a fold. Hénon's fold curve
/// `a = −(1 − b)²/4`, at `x = 2/(1 − b)`, is the fold of a map instead.
#[test]
fn henon_fold_curve_matches_its_closed_form() {
    let sys = system("x_{n+1} = 1 - a*x_n^2 + y_n\ny_{n+1} = b*x_n");
    let (ia, ib) = (param(&sys, "a"), param(&sys, "b"));
    let mut params = vec![0.0; 2];
    params[ia] = -0.1225;
    params[ib] = 0.3;
    let s = seed(CurveKind::Fold, &[2.0 / 0.7, 0.6 / 0.7]);
    for (ds, edge) in [(0.01, 0.5), (-0.01, -0.8)] {
        let c = run(
            &sys,
            &params,
            &s,
            &opts((ib, ia), ((-0.8, 0.5), (-1.0, 0.0)), ds),
        );
        assert_eq!(c.end, CurveEnd::Range);
        assert_eq!(c.points.last().unwrap().p.0, edge);
        for pt in &c.points {
            let (b, a) = pt.p;
            let x = 2.0 / (1.0 - b);
            assert!(
                (a + (1.0 - b) * (1.0 - b) / 4.0).abs() <= 1e-10,
                "a = {a} at b = {b}"
            );
            assert!(close(&pt.x, &[x, b * x], 1e-9), "{:?} at b = {b}", pt.x);
        }
    }
}

#[test]
fn hopf_curve_through_a_bogdanov_takens_point_is_neutral_exactly_where_det_is_negative() {
    let sys = system("x' = y\ny' = a*x + (b - a^2)*y - x^3");
    let (ia, ib) = (param(&sys, "a"), param(&sys, "b"));
    let mut params = vec![0.0; 2];
    params[ia] = -1.0;
    params[ib] = 1.0;
    for ds in [0.01, -0.01] {
        let c = run(
            &sys,
            &params,
            &seed(CurveKind::Hopf, &[0.0, 0.0]),
            &opts((ia, ib), ((-1.5, 1.5), (-0.5, 2.0)), ds),
        );
        assert_eq!(c.end, CurveEnd::Range);
        let last = c.points.last().unwrap();
        assert_eq!(last.p.1, 2.0);
        assert!(
            (last.p.0.abs() - 2f64.sqrt()).abs() <= 1e-10,
            "lands at a = {}",
            last.p.0
        );
        for pt in &c.points {
            let (a, b) = pt.p;
            assert!((b - a * a).abs() <= 1e-10, "b = {b} at a = {a}");
            assert!(close(&pt.x, &[0.0, 0.0], 1e-12), "{:?}", pt.x);
            match pt.nature {
                Nature::Bifurcation { omega: Some(w) } => {
                    assert!(
                        a < 0.0 && (w - (-a).sqrt()).abs() <= 1e-10,
                        "ω = {w} at a = {a}"
                    )
                }
                Nature::Neutral => assert!(a > 0.0, "neutral at a = {a}"),
                other => panic!("{other:?} at a = {a}"),
            }
        }
        let neutral = c.points.iter().filter(|pt| pt.nature == Nature::Neutral);
        assert_eq!(neutral.count() > 0, ds > 0.0, "ds = {ds}");
    }
}

#[test]
fn a_parameter_that_does_not_move_the_fold_is_refused() {
    let opts = opts((0, 1), ((-1.0, 1.0), (0.5, 2.0)), 0.01);
    let fold = seed(CurveKind::Fold, &[0.0, 0.0]);
    let still = system("x' = r + x^2\ny' = -c*y");
    assert_eq!(
        continue_curve(&still, &[0.0, 1.0], &fold, &opts),
        Err(CurveError::NoEffect)
    );
    let moved = system("x' = r + c + x^2\ny' = -c*y");
    let c = run(&moved, &[-1.0, 1.0], &fold, &opts);
    assert_eq!(c.end, CurveEnd::Range);
    assert!(c.points.len() > 2);
    for pt in &c.points {
        assert!(
            (pt.p.0 + pt.p.1).abs() <= 1e-10,
            "r = {} at c = {}",
            pt.p.0,
            pt.p.1
        );
    }
}

#[test]
fn a_closed_fold_curve_closes_on_its_seed() {
    let sys = system("x' = a^2 + b^2 - 1 + x^2\ny' = -y");
    for ds in [0.01, -0.01] {
        let c = run(
            &sys,
            &[0.8, 0.6],
            &seed(CurveKind::Fold, &[0.0, 0.0]),
            &opts((0, 1), ((-2.0, 2.0), (-2.0, 2.0)), ds),
        );
        assert_eq!(c.end, CurveEnd::Closed);
        let (first, last) = (&c.points[0], c.points.last().unwrap());
        assert_eq!((last.p, &last.x), (first.p, &first.x));
        let gap = {
            let before = &c.points[c.points.len() - 2];
            let (da, db) = ((before.p.0 - last.p.0) / 4.0, (before.p.1 - last.p.1) / 4.0);
            (da * da + db * db).sqrt()
        };
        assert!(gap <= 0.05, "closing step {gap}");
        let mut quadrants = [false; 4];
        for pt in &c.points {
            let (a, b) = pt.p;
            assert!(
                (a * a + b * b - 1.0).abs() <= 1e-10,
                "off the circle at ({a}, {b})"
            );
            assert!(close(&pt.x, &[0.0, 0.0], 1e-10), "{:?}", pt.x);
            quadrants[usize::from(a < 0.0) + 2 * usize::from(b < 0.0)] = true;
        }
        assert_eq!(quadrants, [true; 4]);
    }
}

/// The branch point on the branch through `x0` as `params[param]` varies
/// over `range`, as a seed, with the parameters there.
fn branch_point(
    sys: &CompiledSystem,
    params: &[f64],
    x0: &[f64],
    param: usize,
    range: (f64, f64),
) -> (CurveSeed, Vec<f64>) {
    let o = ContinuationOptions {
        param,
        period: 1,
        range,
        ds: 0.01,
        ds_min: 1e-8,
        ds_max: 0.05,
        max_points: 2000,
        max_angle: 0.3,
    };
    let branch = continue_branch(sys, params, x0, &o).expect("the branch starts");
    let bp = branch
        .special
        .iter()
        .find(|sp| sp.kind == SpecialKind::BranchPoint)
        .expect("a branch point");
    let mut at = params.to_vec();
    at[param] = bp.p;
    let seed = CurveSeed {
        kind: CurveKind::BranchPoint,
        x: bp.x.clone(),
        period: bp.period,
    };
    (seed, at)
}

/// F3 of the predator–prey map crosses F2 = ((a − 1)/a, 0) where the
/// multiplier `3.5x/(1 + εx)` of F2 is 1, on the invariant line `y = 0`:
/// `a = (2ε − 7)/(2ε − 5)`, `x = 1/(3.5 − ε)`, with multipliers 1 and
/// `2 − a`. The second is −1 at the fold-flip `(a, ε) = (3, 2)`.
#[test]
fn predator_prey_branch_point_curve_is_where_f3_crosses_f2() {
    let sys = system(PREDATOR_PREY);
    let (ia, ie) = (param(&sys, "a"), param(&sys, "epsilon"));
    let mut params = vec![0.0; 2];
    params[ia] = 1.5;
    params[ie] = 1.0;
    let (s, params) = branch_point(&sys, &params, &[1.0 / 3.0, 0.0], ia, (1.2, 2.5));
    let o = |ds| opts((ia, ie), ((1.0, 4.0), (-2.0, 2.4)), ds);
    let [right, left] = [0.01, -0.01].map(|ds| run(&sys, &params, &s, &o(ds)));
    let multipliers = |pt: &CurvePoint| {
        let mut mu = pt.eigenvalues.clone();
        mu.sort_by(|p, q| p.0.total_cmp(&q.0));
        [mu[0].0, mu[0].1, mu[1].0, mu[1].1]
    };
    for c in [&right, &left] {
        assert_eq!(c.end, CurveEnd::Range);
        for pt in &c.points {
            let (a, epsilon) = pt.p;
            let want = (2.0 * epsilon - 7.0) / (2.0 * epsilon - 5.0);
            assert!(
                (a - want).abs() <= 1e-8,
                "a = {a} vs {want} at ε = {epsilon}"
            );
            assert!(
                close(&pt.x, &[1.0 / (3.5 - epsilon), 0.0], 1e-8),
                "{:?} at ε = {epsilon}",
                pt.x
            );
            assert!(
                close(&multipliers(pt), &[2.0 - a, 0.0, 1.0, 0.0], 1e-8),
                "{:?} at a = {a}",
                pt.eigenvalues
            );
            assert_eq!(pt.nature, Nature::Bifurcation { omega: None });
        }
    }
    let (east, south) = (right.points.last().unwrap(), left.points.last().unwrap());
    assert_eq!(east.p.0, 4.0);
    assert!(
        (east.p.1 - 13.0 / 6.0).abs() <= 1e-8,
        "lands at ε = {}",
        east.p.1
    );
    assert_eq!(south.p.1, -2.0);
    assert!(
        (south.p.0 - 11.0 / 9.0).abs() <= 1e-8,
        "lands at a = {}",
        south.p.0
    );
    let flip = right
        .points
        .windows(2)
        .find(|w| (1.0 + multipliers(&w[0])[0]) * (1.0 + multipliers(&w[1])[0]) < 0.0)
        .expect("a multiplier passes −1");
    assert!(
        flip[0].p.1 < 2.0 && 2.0 < flip[1].p.1,
        "the fold-flip at ε = 2 is not between {:?} and {:?}",
        flip[0].p,
        flip[1].p
    );
    let fold = CurveSeed {
        kind: CurveKind::Fold,
        ..s.clone()
    };
    assert_eq!(
        continue_curve(&sys, &params, &fold, &o(0.01)),
        Err(CurveError::Singular)
    );
}

/// Competition `x' = x(1 − x − ay)`, `y' = y(b − y − x)`: the semi-trivial
/// equilibrium `(0, b)` has the eigenvalue `1 − ab` along the invariant line
/// `x = 0`, where the coexistence branch crosses it.
#[test]
fn competition_branch_point_curve_is_the_hyperbola_ab_1() {
    let sys = system("x' = x*(1 - x - a*y)\ny' = y*(b - y - x)");
    let (ia, ib) = (param(&sys, "a"), param(&sys, "b"));
    let mut params = vec![0.0; 2];
    params[ia] = 0.3;
    params[ib] = 2.0;
    let (s, params) = branch_point(&sys, &params, &[0.0, 2.0], ia, (0.2, 0.8));
    let o = |ds| opts((ia, ib), ((0.2, 1.25), (0.6, 3.0)), ds);
    let [right, left] = [0.01, -0.01].map(|ds| run(&sys, &params, &s, &o(ds)));
    for c in [&right, &left] {
        assert_eq!(c.end, CurveEnd::Range);
        for pt in &c.points {
            let (a, b) = pt.p;
            assert!((a * b - 1.0).abs() <= 1e-8, "ab = {} at b = {b}", a * b);
            assert!(close(&pt.x, &[0.0, b], 1e-8), "{:?} at b = {b}", pt.x);
        }
    }
    let (east, north) = (right.points.last().unwrap(), left.points.last().unwrap());
    assert_eq!(east.p.0, 1.25);
    assert!((east.p.1 - 0.8).abs() <= 1e-8, "lands at b = {}", east.p.1);
    assert_eq!(north.p.1, 3.0);
    assert!(
        (north.p.0 - 1.0 / 3.0).abs() <= 1e-8,
        "lands at a = {}",
        north.p.0
    );
}

/// `x' = px − x² + q` has a transcritical point at `x = p = 0` for `q = 0`
/// only: any other `q` unfolds it, into two folds for `q < 0` and two
/// branches that never meet for `q > 0`.
#[test]
fn a_parameter_that_unfolds_the_branch_point_ends_the_curve_at_its_seed() {
    let sys = system("x' = p*x - x^2 + q");
    let (ip, iq) = (param(&sys, "p"), param(&sys, "q"));
    let s = seed(CurveKind::BranchPoint, &[0.0]);
    for ds in [0.01, -0.01] {
        let c = run(
            &sys,
            &[0.0, 0.0],
            &s,
            &opts((ip, iq), ((-1.0, 1.0), (-1.0, 1.0)), ds),
        );
        assert_eq!(c.end, CurveEnd::Unfolded, "ds = {ds}");
        assert_eq!(c.points.len(), 1, "ds = {ds}");
        assert_eq!((c.points[0].p, &c.points[0].x), ((0.0, 0.0), &vec![0.0]));
    }
}

fn kinds_at(c: &Curve, kind: Codim2Kind) -> Vec<&Codim2Point> {
    c.special.iter().filter(|sp| sp.kind == kind).collect()
}

fn kinds(c: &Curve) -> Vec<Codim2Kind> {
    c.special.iter().map(|sp| sp.kind).collect()
}

fn assert_near(got: (f64, f64), want: (f64, f64), tol: f64) {
    assert!(
        (got.0 - want.0).abs() <= tol && (got.1 - want.1).abs() <= tol,
        "{got:?} vs {want:?}, tolerance {tol:e}"
    );
}

/// The one point of `kind` on the two halves `c`, nearest `want`.
fn one(halves: &[Curve], kind: Codim2Kind, want: (f64, f64)) -> &Codim2Point {
    let d = |sp: &&Codim2Point| (sp.p.0 - want.0).abs().max((sp.p.1 - want.1).abs());
    halves
        .iter()
        .flat_map(|c| kinds_at(c, kind))
        .min_by(|a, b| d(a).total_cmp(&d(b)))
        .unwrap_or_else(|| panic!("no {kind:?} near {want:?}"))
}

impl Co {
    /// Both halves from the branch's special point of `kind` nearest `k2`,
    /// in a box that holds every codimension-2 point of the reference page.
    fn halves(&self, kind: SpecialKind, k2: f64) -> [Curve; 2] {
        [0.01, -0.01].map(|ds| self.curve(kind, k2, ((0.5, 2.0), (0.1, 2.0)), ds))
    }
}

const CO_BT: [(f64, f64); 2] = [(1.41762764, 0.97139757), (1.16119871, 0.72233907)];
const CO_BT_STATE: [[f64; 3]; 2] = [
    [0.11590872, 0.31546728, 0.28843700],
    [0.01633704, 0.63840988, 0.20045593],
];

#[test]
fn co_oxidation_fold_curve_has_the_cusp_and_both_bogdanov_takens_points() {
    let co = co();
    let fold = co.halves(SpecialKind::Fold, 1.042);
    let mut found: Vec<Codim2Kind> = fold.iter().flat_map(kinds).collect();
    found.sort_by_key(|k| *k as u8);
    assert_eq!(
        found,
        [
            Codim2Kind::BogdanovTakens,
            Codim2Kind::BogdanovTakens,
            Codim2Kind::Cusp
        ]
    );
    let cusp = one(&fold, Codim2Kind::Cusp, (1.00640833, 0.35599127));
    assert_near(cusp.p, (1.00640833, 0.35599127), 1e-5);
    assert!(close(&cusp.x, &[0.03594064, 0.35200543, 0.45137011], 1e-5));
    for (bt, x) in CO_BT.iter().zip(CO_BT_STATE) {
        let at = one(&fold, Codim2Kind::BogdanovTakens, *bt);
        assert_near(at.p, *bt, 1e-6);
        assert!(close(&at.x, &x, 1e-6), "{:?}", at.x);
        assert!(at.bracket < 1e-10, "bracket {}", at.bracket);
    }
}

#[test]
fn co_oxidation_hopf_curve_has_both_bautin_and_bogdanov_takens_points() {
    let co = co();
    let hopf = co.halves(SpecialKind::Hopf, 1.0516);
    let mut found: Vec<Codim2Kind> = hopf.iter().flat_map(kinds).collect();
    found.sort_by_key(|k| *k as u8);
    assert_eq!(
        found,
        [
            Codim2Kind::BogdanovTakens,
            Codim2Kind::BogdanovTakens,
            Codim2Kind::Bautin,
            Codim2Kind::Bautin
        ]
    );
    let bautin = [
        (
            (0.89131898, 0.23248735),
            [0.01802242, 0.36823794, 0.49796831],
        ),
        (
            (0.92425502, 0.30587927),
            [0.06431133, 0.21109514, 0.55487023],
        ),
    ];
    for (gh, x) in bautin {
        let at = one(&hopf, Codim2Kind::Bautin, gh);
        assert_near(at.p, gh, 1e-6);
        assert!(close(&at.x, &x, 1e-6), "{:?}", at.x);
    }
    let fold = co.halves(SpecialKind::Fold, 1.042);
    for (bt, x) in CO_BT.iter().zip(CO_BT_STATE) {
        let (on_hopf, on_fold) = (
            one(&hopf, Codim2Kind::BogdanovTakens, *bt),
            one(&fold, Codim2Kind::BogdanovTakens, *bt),
        );
        assert_near(on_hopf.p, *bt, 1e-6);
        assert!(close(&on_hopf.x, &x, 1e-6), "{:?}", on_hopf.x);
        // The fold curve finds BT where wᵀq = 0, the Hopf curve where
        // κ = 0: two test functions on two defining systems, which agree
        // to 1e-13.
        assert_near(on_hopf.p, on_fold.p, 1e-11);
        assert!(close(&on_hopf.x, &on_fold.x, 1e-11));
    }
}

/// Each special point with both of its neighbours on the curve neutral.
fn inside_neutral_stretches(c: &Curve) -> Vec<&Codim2Point> {
    let neutral = |i: usize| c.points.get(i).map(|pt| pt.nature) == Some(Nature::Neutral);
    c.special
        .iter()
        .filter(|sp| sp.index > 0 && neutral(sp.index - 1) && neutral(sp.index + 1))
        .collect()
}

#[test]
fn no_special_point_lies_inside_a_neutral_stretch() {
    let co = co();
    let ns: fn(f64) -> f64 = |e| (2.0 * e - 7.0) / (2.0 * e - 3.0);
    let curves: Vec<Curve> = co
        .halves(SpecialKind::Hopf, 1.0516)
        .into_iter()
        .chain(co.halves(SpecialKind::Hopf, 1.041))
        .chain(predator_prey(
            CurveKind::NeimarkSacker,
            1.0,
            ns,
            ((-5.0, 1.45), (0.0, 45.0)),
        ))
        .collect();
    let mut boundaries = 0;
    for c in &curves {
        assert_eq!(inside_neutral_stretches(c), Vec::<&Codim2Point>::new());
        for sp in &c.special {
            let neutral = [sp.index - 1, sp.index + 1]
                .map(|i| c.points[i].nature == Nature::Neutral)
                .iter()
                .filter(|n| **n)
                .count();
            let begins = matches!(
                sp.kind,
                Codim2Kind::BogdanovTakens | Codim2Kind::Resonance1 | Codim2Kind::Resonance2
            );
            assert_eq!(neutral, usize::from(begins), "{:?} at {:?}", sp.kind, sp.p);
            boundaries += neutral;
        }
    }
    assert_eq!(boundaries, 5, "four BT points and one 1:2 resonance");
}

/// The Bogdanov–Takens normal form `ẋ = y, ẏ = b − a + ax + x² − xy` with
/// `ż = a + c + z²`: a Hopf curve `b = a` at `x = y = 0`, `z² = −a − c`,
/// with the eigenvalues `±√a` and `2z`. Where `a < 0` it is a Hopf curve,
/// with a Bogdanov–Takens point at `a = 0` and a neutral saddle beyond. At
/// `z = 0` the third eigenvalue, the product of those outside the critical
/// pair, changes sign and the curve turns back in `a`: a zero-Hopf point
/// at `a = −c` if `c > 0`, but if `c < 0` a fold of the neutral saddle, no
/// bifurcation.
#[test]
fn a_zero_eigenvalue_beside_a_neutral_saddle_is_not_a_zero_hopf_point() {
    let sys = system("x' = y\ny' = b - a + a*x + x^2 - x*y\nz' = a + c + z^2");
    let (ia, ib, ic) = (param(&sys, "a"), param(&sys, "b"), param(&sys, "c"));
    let curve = |c: f64| {
        let mut params = vec![0.0; 3];
        (params[ia], params[ib], params[ic]) = (-1.0, -1.0, c);
        run(
            &sys,
            &params,
            &seed(CurveKind::Hopf, &[0.0, 0.0, -(1.0 - c).sqrt()]),
            &opts((ia, ib), ((-1.5, 1.5), (-1.5, 1.5)), 0.01),
        )
    };
    let hopf_side = curve(0.5);
    assert_eq!(kinds(&hopf_side), [Codim2Kind::ZeroHopf]);
    assert_near(hopf_side.special[0].p, (-0.5, -0.5), 1e-10);

    let neutral_side = curve(-0.5);
    let crossing = neutral_side
        .points
        .windows(2)
        .find(|w| w[0].x[2] * w[1].x[2] < 0.0)
        .expect("the third eigenvalue 2z changes sign");
    assert!(
        crossing.iter().all(|pt| pt.nature == Nature::Neutral),
        "{:?} to {:?}",
        crossing[0].p,
        crossing[1].p
    );
    assert_eq!(
        kinds(&neutral_side),
        [Codim2Kind::BogdanovTakens, Codim2Kind::BogdanovTakens]
    );
    for bt in &neutral_side.special {
        assert_near(bt.p, (0.0, 0.0), 1e-10);
    }
}

#[test]
fn predator_prey_neimark_sacker_curve_has_the_strong_resonances_and_the_chenciner_point() {
    let ns: fn(f64) -> f64 = |e| (2.0 * e - 7.0) / (2.0 * e - 3.0);
    let [forward, backward] = predator_prey(
        CurveKind::NeimarkSacker,
        1.0,
        ns,
        ((-5.0, 1.45), (0.0, 45.0)),
    );
    assert_eq!(
        kinds(&forward),
        [
            Codim2Kind::Resonance4,
            Codim2Kind::Resonance3,
            Codim2Kind::Resonance2
        ]
    );
    let want = [(7.0 / 6.0, 7.0), (49.0 / 38.0, 10.5), (35.0 / 26.0, 14.0)];
    for (sp, want) in forward.special.iter().zip(want) {
        assert_near(sp.p, want, 1e-6);
        assert!(close(&sp.x, &f3(want.0, want.1), 1e-6), "{:?}", sp.x);
    }
    assert_eq!(kinds(&backward), [Codim2Kind::Chenciner]);
    assert_near(backward.special[0].p, (-4.6400549446, 1.3257299842), 1e-6);
    assert!(close(
        &backward.special[0].x,
        &[0.1228492936, 0.0233424908],
        1e-6
    ));
}

/// F3's flip curve passes the 1:2 resonance of its Neimark–Sacker curve and
/// meets the branch-point curve at the fold-flip `(ε, a) = (2, 3)`, where
/// F3 = F2 = (2/3, 0). The flip coefficient has poles at both, and no zero.
#[test]
fn predator_prey_flip_curve_has_the_1_2_resonance_and_the_fold_flip() {
    let flip: fn(f64) -> f64 = |e| (4.0 * e * e - 56.0 * e + 147.0) / (4.0 * e * e + 4.0 * e - 7.0);
    let halves = predator_prey(CurveKind::Flip, 1.5, flip, ((1.2, 2.5), (0.0, 30.0)));
    let found: Vec<Codim2Kind> = halves.iter().flat_map(kinds).collect();
    assert_eq!(found, [Codim2Kind::FoldFlip, Codim2Kind::Resonance2]);
    assert_near(
        one(&halves, Codim2Kind::Resonance2, (0.0, 0.0)).p,
        (35.0 / 26.0, 14.0),
        1e-6,
    );
    let lppd = one(&halves, Codim2Kind::FoldFlip, (0.0, 0.0));
    assert_near(lppd.p, (2.0, 3.0), 1e-6);
    assert!(close(&lppd.x, &[2.0 / 3.0, 0.0], 1e-6), "{:?}", lppd.x);
}

#[test]
fn predator_prey_branch_point_curve_has_the_fold_flip() {
    let sys = system(PREDATOR_PREY);
    let (ia, ie) = (param(&sys, "a"), param(&sys, "epsilon"));
    let mut params = vec![0.0; 2];
    params[ia] = 1.5;
    params[ie] = 1.0;
    let (s, params) = branch_point(&sys, &params, &[1.0 / 3.0, 0.0], ia, (1.2, 2.5));
    let halves = [0.01, -0.01].map(|ds| {
        run(
            &sys,
            &params,
            &s,
            &opts((ia, ie), ((1.0, 4.0), (-2.0, 2.4)), ds),
        )
    });
    let found: Vec<Codim2Kind> = halves.iter().flat_map(kinds).collect();
    assert_eq!(found, [Codim2Kind::FoldFlip]);
    let lppd = one(&halves, Codim2Kind::FoldFlip, (3.0, 2.0));
    assert_near(lppd.p, (3.0, 2.0), 1e-6);
    assert!(close(&lppd.x, &[2.0 / 3.0, 0.0], 1e-6), "{:?}", lppd.x);
}

/// Extended Lorenz-84, reference system 2, with `F` across and `T` up, and
/// the branch at `T = 0.04` from the reference start.
struct Lorenz {
    sys: CompiledSystem,
    f: usize,
    t: usize,
    params: Vec<f64>,
    branch: dyn_analysis::Branch,
}

fn lorenz() -> Lorenz {
    let sys = system(
        "x' = -y^2 - z^2 - 0.25*x + 0.25*F - 0.987*u^2\n\
         y' = x*y - x*z - y + 0.25\n\
         z' = x*y + x*z - z\n\
         u' = -1.04*u + 0.987*u*x + T",
    );
    let (f, t) = (param(&sys, "F"), param(&sys, "T"));
    let mut params = vec![0.0; 2];
    params[f] = 3.0;
    params[t] = 0.04;
    let o = ContinuationOptions {
        param: f,
        period: 1,
        range: (1.0, 3.0),
        ds: -0.01,
        ds_min: 1e-8,
        ds_max: 0.05,
        max_points: 2000,
        max_angle: 0.3,
    };
    let start = [
        2.9787004394953343,
        -0.03868302503393752,
        0.058232737694740085,
        -0.02105288273117459,
    ];
    let branch = continue_branch(&sys, &params, &start, &o).expect("the branch at T = 0.04");
    Lorenz {
        sys,
        f,
        t,
        params,
        branch,
    }
}

impl Lorenz {
    fn halves(&self, kind: SpecialKind, f: f64) -> [Curve; 2] {
        let at = self
            .branch
            .special
            .iter()
            .filter(|sp| sp.kind == kind)
            .min_by(|a, b| (a.p - f).abs().total_cmp(&(b.p - f).abs()))
            .unwrap_or_else(|| panic!("{kind:?} near F = {f}"));
        let mut params = self.params.clone();
        params[self.f] = at.p;
        let curve = match kind {
            SpecialKind::Fold => CurveKind::Fold,
            _ => CurveKind::Hopf,
        };
        // The zero-Hopf points at T = ±0.000127 are 0.042 apart in z, so a
        // step longer than that can hold both, and their sign changes cancel.
        let o = |ds| CurveOptions {
            ds_max: 0.01,
            ..opts((self.f, self.t), ((0.5, 3.5), (-0.03, 0.08)), ds)
        };
        [0.01, -0.01].map(|ds| run(&self.sys, &params, &seed(curve, &at.x), &o(ds)))
    }
}

const LORENZ_BT: (f64, f64) = (1.4467167, 0.02094017);
const LORENZ_ZH: (f64, f64) = (1.2834192961, 0.0001265408);
const LORENZ_HH: (f64, f64) = (2.5332211070, 0.0262739430);

/// The fold curve is symmetric under `T ↦ −T`, `u ↦ −u`, so it has each
/// point twice, and at `T = 0` a cusp: the pitchfork of the equilibria
/// with `u = 0`, where `u' = u(0.987x − 1.04)` has the eigenvalue `0`, so
/// `x = 1.04/0.987`.
#[test]
fn lorenz_84_fold_curve_has_the_bogdanov_takens_zero_hopf_and_cusp_points() {
    let l = lorenz();
    let fold = l.halves(SpecialKind::Fold, 1.5466);
    let mut found: Vec<Codim2Kind> = fold.iter().flat_map(kinds).collect();
    found.sort_by_key(|k| *k as u8);
    assert_eq!(
        found,
        [
            Codim2Kind::BogdanovTakens,
            Codim2Kind::BogdanovTakens,
            Codim2Kind::Cusp,
            Codim2Kind::ZeroHopf,
            Codim2Kind::ZeroHopf
        ]
    );
    let mirror = |(f, t): (f64, f64)| (f, -t);
    for bt in [LORENZ_BT, mirror(LORENZ_BT)] {
        let at = one(&fold, Codim2Kind::BogdanovTakens, bt);
        assert_near(at.p, bt, 1e-6);
    }
    let zh = one(&fold, Codim2Kind::ZeroHopf, LORENZ_ZH);
    assert_near(zh.p, LORENZ_ZH, 1e-6);
    assert!(
        close(
            &zh.x,
            &[1.0598439, -0.01327681, 0.23513412, -0.02086092],
            1e-6
        ),
        "{:?}",
        zh.x
    );
    let zh = one(&fold, Codim2Kind::ZeroHopf, mirror(LORENZ_ZH));
    assert_near(zh.p, mirror(LORENZ_ZH), 1e-6);
    let cusp = one(&fold, Codim2Kind::Cusp, (1.28, 0.0));
    assert!(cusp.p.1.abs() <= 1e-10, "T = {}", cusp.p.1);
    assert!((cusp.x[0] - 1.04 / 0.987).abs() <= 1e-8, "{:?}", cusp.x);
}

/// The Hopf curves from the three Hopf points at `T = 0.04`: the two of the
/// slower pair are one curve through the Bautin and Hopf–Hopf points that
/// turns neutral at the Bogdanov–Takens point and comes back as a Hopf
/// curve at its mirror; the faster pair's curve crosses it at the
/// Hopf–Hopf point and its mirror and passes the zero-Hopf points of the
/// fold curve.
#[test]
fn lorenz_84_hopf_curves_have_the_bautin_hopf_hopf_and_zero_hopf_points() {
    let l = lorenz();
    let slow = l.halves(SpecialKind::Hopf, 1.6197);
    let found: Vec<Codim2Kind> = slow.iter().flat_map(kinds).collect();
    assert_eq!(
        found,
        [
            Codim2Kind::Bautin,
            Codim2Kind::HopfHopf,
            Codim2Kind::BogdanovTakens,
            Codim2Kind::BogdanovTakens
        ]
    );
    let gh = (2.3763600882, 0.0501974320);
    let at = one(&slow, Codim2Kind::Bautin, gh);
    assert_near(at.p, gh, 1e-6);
    assert!(
        close(
            &at.x,
            &[1.15285369, -0.02825526, 0.2131069, -0.51291693],
            1e-6
        ),
        "{:?}",
        at.x
    );
    let at = one(&slow, Codim2Kind::HopfHopf, LORENZ_HH);
    assert_near(at.p, LORENZ_HH, 1e-6);
    assert!(
        close(
            &at.x,
            &[1.10143957, -0.02072803, 0.22506669, -0.55758627],
            1e-6
        ),
        "{:?}",
        at.x
    );
    for bt in [LORENZ_BT, (LORENZ_BT.0, -LORENZ_BT.1)] {
        assert_near(one(&slow, Codim2Kind::BogdanovTakens, bt).p, bt, 1e-6);
    }

    let fast = l.halves(SpecialKind::Hopf, 2.8599);
    let found: Vec<Codim2Kind> = fast.iter().flat_map(kinds).collect();
    assert_eq!(
        found,
        [
            Codim2Kind::HopfHopf,
            Codim2Kind::ZeroHopf,
            Codim2Kind::ZeroHopf,
            Codim2Kind::HopfHopf
        ]
    );
    for hh in [LORENZ_HH, (LORENZ_HH.0, -LORENZ_HH.1)] {
        assert_near(one(&fast, Codim2Kind::HopfHopf, hh).p, hh, 1e-6);
    }
    for zh in [LORENZ_ZH, (LORENZ_ZH.0, -LORENZ_ZH.1)] {
        assert_near(one(&fast, Codim2Kind::ZeroHopf, zh).p, zh, 1e-6);
    }
}

/// On the invariant line `y = 0` the predator–prey map has `F2 = (x, 0)`,
/// `x = (a − 1)/a`, whose `y` multiplier `3.5x/(1 + εx)` is `−1` along
/// `ε = −3.5 − 1/x`. The flip coefficient of that curve vanishes at
/// `x = −(2√2 + 1)/7`, so `a = 1 − √2/4` and `ε = 2√2 − 9/2` (derived by
/// solving `c = 0` symbolically).
#[test]
fn predator_prey_flip_curve_of_f2_has_a_generalized_flip() {
    let sys = system(PREDATOR_PREY);
    let (ie, ia) = (param(&sys, "epsilon"), param(&sys, "a"));
    let x = -1.0 / 1.7;
    let mut params = vec![0.0; 2];
    params[ie] = -1.8;
    params[ia] = 1.0 / (1.0 - x);
    let halves = [0.01, -0.01].map(|ds| {
        run(
            &sys,
            &params,
            &seed(CurveKind::Flip, &[x, 0.0]),
            &opts((ie, ia), ((-2.0, -1.5), (0.5, 1.0)), ds),
        )
    });
    let found: Vec<Codim2Kind> = halves.iter().flat_map(kinds).collect();
    assert_eq!(found, [Codim2Kind::GeneralizedFlip]);
    let root2 = 2f64.sqrt();
    let gpd = one(&halves, Codim2Kind::GeneralizedFlip, (0.0, 0.0));
    assert_near(gpd.p, (2.0 * root2 - 4.5, 1.0 - root2 / 4.0), 1e-9);
    assert!(
        close(&gpd.x, &[-(2.0 * root2 + 1.0) / 7.0, 0.0], 1e-9),
        "{:?}",
        gpd.x
    );
}
