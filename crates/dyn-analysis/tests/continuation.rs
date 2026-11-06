use dyn_analysis::{continue_branch, Branch, BranchEnd, ContinuationOptions, SpecialKind};
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

fn opts(param: usize, range: (f64, f64), ds: f64) -> ContinuationOptions {
    ContinuationOptions {
        param,
        period: 1,
        range,
        ds,
        ds_min: 1e-8,
        ds_max: 0.1,
        max_points: 1000,
        max_angle: 0.3,
    }
}

fn is_special(b: &Branch, pt: &dyn_analysis::BranchPoint) -> bool {
    b.special
        .iter()
        .any(|sp| std::ptr::eq(&b.points[sp.index], pt))
}

fn run(sys: &CompiledSystem, params: &[f64], x0: &[f64], o: &ContinuationOptions) -> Branch {
    continue_branch(sys, params, x0, o).expect("the branch starts")
}

#[test]
fn saddle_node_branch_passes_the_fold() {
    let sys = system("x' = r + x^2");
    let b = run(&sys, &[-1.0], &[-1.0], &opts(0, (-1.0, 1.0), 0.05));
    assert_eq!(b.end, BranchEnd::Range);
    for pt in &b.points {
        let (x, r) = (pt.x[0], pt.p);
        assert!(
            (r + x * x).abs() <= 1e-10,
            "r + x² = {} at x = {x}",
            r + x * x
        );
        if !is_special(&b, pt) {
            assert_eq!(pt.stable, x < 0.0, "λ = 2x = {} at x = {x}", 2.0 * x);
        }
    }
    let xs: Vec<f64> = b.points.iter().map(|pt| pt.x[0]).collect();
    assert!(xs.iter().any(|&x| x < -0.99), "{xs:?}");
    assert!(xs.iter().any(|&x| x > 0.99), "{xs:?}");
}

#[test]
fn transcritical_trivial_branch_stays_on_zero() {
    let sys = system("x' = r*x - x^2");
    let b = run(&sys, &[-1.0], &[0.0], &opts(0, (-1.0, 1.0), 0.05));
    assert_eq!(b.end, BranchEnd::Range);
    assert_eq!(b.points.last().unwrap().p, 1.0);
    for pt in &b.points {
        assert!(pt.x[0].abs() <= 1e-12, "x = {} at r = {}", pt.x[0], pt.p);
        if !is_special(&b, pt) {
            assert_eq!(pt.stable, pt.p < 0.0, "λ = r = {}", pt.p);
        }
    }
}

#[test]
fn selkov_branch_matches_the_formula() {
    let sys = system("x' = -x + a*y + x^2*y\ny' = b - a*y - x^2*y");
    let (ia, ib) = (param(&sys, "a"), param(&sys, "b"));
    let a = 0.08;
    let mut params = vec![0.0; 2];
    params[ia] = a;
    params[ib] = 0.05;
    let b = run(&sys, &params, &[0.06, 0.6], &opts(ib, (0.05, 1.5), 0.05));
    assert_eq!(b.end, BranchEnd::Range);
    assert_eq!(b.points[0].p, 0.05);
    assert_eq!(b.points.last().unwrap().p, 1.5);
    for pt in &b.points {
        let bb = pt.p;
        let y = bb / (a + bb * bb);
        assert!((pt.x[0] - bb).abs() <= 1e-10, "x = {} at b = {bb}", pt.x[0]);
        assert!(
            (pt.x[1] - y).abs() <= 1e-10,
            "y = {} vs {y} at b = {bb}",
            pt.x[1]
        );
    }
}

#[test]
fn circle_branch_turns_without_jumping() {
    let sys = system("x' = x^2 + r^2 - 1");
    let max_angle = 0.15;
    let o = ContinuationOptions {
        ds_max: 0.5,
        max_points: 120,
        max_angle,
        ..opts(0, (-2.0, 2.0), 0.4)
    };
    let b = run(&sys, &[0.0], &[-1.0], &o);
    assert_eq!(b.end, BranchEnd::MaxPoints);
    for pt in &b.points {
        let (x, r) = (pt.x[0], pt.p);
        assert!(
            (x * x + r * r - 1.0).abs() <= 1e-10,
            "off the circle at ({x}, {r})"
        );
    }
    for w in b.points.windows(2) {
        let cos: f64 = w[0]
            .tangent
            .iter()
            .zip(&w[1].tangent)
            .map(|(u, v)| u * v)
            .sum();
        assert!(
            cos >= max_angle.cos() - 1e-12,
            "turned by {} rad",
            cos.acos()
        );
    }
    let turns = b
        .points
        .windows(2)
        .filter(|w| w[0].tangent[1].signum() != w[1].tangent[1].signum())
        .count();
    assert!(turns >= 2, "ṗ changes sign {turns} times");
    assert!(b.points.iter().any(|pt| pt.p > 0.99));
    assert!(b.points.iter().any(|pt| pt.p < -0.99));
}

#[test]
fn power_map_matches_closed_form() {
    let sys = system("x_{n+1} = r*x_n*(1 - x_n)");
    let r: f64 = 3.3;
    let o = ContinuationOptions {
        period: 2,
        max_points: 1,
        ..opts(0, (3.3, 3.4), 0.01)
    };
    let b = run(&sys, &[r], &[0.8], &o);
    assert_eq!(b.period, 2);
    let pt = &b.points[0];
    let s = ((r - 3.0) * (r + 1.0)).sqrt();
    let (hi, lo) = ((r + 1.0 + s) / (2.0 * r), (r + 1.0 - s) / (2.0 * r));
    assert!((pt.x[0] - hi).abs() <= 1e-12, "x = {} vs {hi}", pt.x[0]);
    assert_eq!(pt.cycle.len(), 2);
    assert!((pt.cycle[0] - hi).abs() <= 1e-12 && (pt.cycle[1] - lo).abs() <= 1e-12);
    let mu = 4.0 + 2.0 * r - r * r;
    assert_eq!(pt.eigenvalues.len(), 1);
    assert!(
        (pt.eigenvalues[0].0 - mu).abs() <= 1e-12,
        "{:?} vs {mu}",
        pt.eigenvalues
    );
    assert!(pt.stable && pt.unstable == 0);
    let dhi_dr = (2.0 * r * (r - 1.0) / s - 2.0 - 2.0 * s) / (4.0 * r * r);
    let slope = pt.tangent[0] / pt.tangent[1];
    assert!(
        (slope - dhi_dr).abs() <= 1e-12,
        "dx/dr = {slope} vs {dhi_dr}"
    );
}

#[test]
fn map_stability_uses_the_modulus_of_complex_multipliers() {
    let sys = system("x_{n+1} = r*x_n*(1 - y_n)\ny_{n+1} = x_n");
    let x0 = 1.0 - 1.0 / 1.5;
    let b = run(&sys, &[1.5], &[x0, x0], &opts(0, (1.5, 2.5), 0.05));
    assert_eq!(b.end, BranchEnd::Range);
    assert!(b.points.iter().any(|pt| pt.p > 2.0));
    for pt in &b.points {
        let r = pt.p;
        assert!((pt.x[0] - (1.0 - 1.0 / r)).abs() <= 1e-12);
        for &(re, im) in &pt.eigenvalues {
            assert!(
                (re * re + im * im - (r - 1.0)).abs() <= 1e-12,
                "|μ|² at r = {r}"
            );
        }
        if (r - 2.0).abs() > 1e-9 {
            assert_eq!(pt.stable, r < 2.0, "r = {r}");
            assert_eq!(pt.unstable, if r > 2.0 { 2 } else { 0 }, "r = {r}");
        }
    }
}

#[test]
fn range_and_max_points_bound_the_branch() {
    let sys = system("x' = r - x");
    for (ds, edge) in [(0.1, 0.7_f64), (-0.1, -0.3)] {
        let b = run(&sys, &[0.0], &[0.0], &opts(0, (-0.3, 0.7), ds));
        assert_eq!(b.end, BranchEnd::Range);
        let last = b.points.last().unwrap();
        assert_eq!(last.p.to_bits(), edge.to_bits(), "{} vs {edge}", last.p);
        assert!((last.x[0] - edge).abs() <= 1e-12);
        assert!(b.points.windows(2).all(|w| (w[1].p - w[0].p) * ds > 0.0));
    }
    let o = ContinuationOptions {
        max_points: 5,
        ..opts(0, (-10.0, 10.0), 0.1)
    };
    let b = run(&sys, &[0.0], &[0.0], &o);
    assert_eq!(b.end, BranchEnd::MaxPoints);
    assert_eq!(b.points.len(), 5);
}

/// The branch of `ẋ = r + x²` through the fold at `r = 0`.
fn saddle_node() -> Branch {
    let sys = system("x' = r + x^2");
    run(&sys, &[-1.0], &[-1.0], &opts(0, (-1.0, 1.0), 0.05))
}

fn trivial_branch(source: &str) -> Branch {
    let sys = system(source);
    run(&sys, &[-1.0], &[0.0], &opts(0, (-1.0, 1.0), 0.05))
}

fn hopf(sign: &str) -> Branch {
    let sys = system(&format!(
        "x' = mu*x - y {sign} x*(x^2 + y^2)\ny' = x + mu*y {sign} y*(x^2 + y^2)"
    ));
    run(&sys, &[-1.0], &[0.0, 0.0], &opts(0, (-1.0, 1.0), 0.05))
}

/// `ẋ = μx − y + x² + xy, ẏ = x + μy + x²`: quadratic terms only, so `ℓ₁`
/// comes from the `B` terms. Guckenheimer and Holmes's formula (3.4.11) for
/// `ẋ = −ωy + f, ẏ = ωx + g` gives the radial coefficient
/// `a = [f_xy (f_xx + f_yy) − g_xy (g_xx + g_yy) − f_xx g_xx + f_yy g_yy] / 16ω
/// = (1·2 − 0 − 2·2 + 0) / 16 = −1/8`, and `ℓ₁ = a/ω`.
fn hopf_quadratic() -> Branch {
    let sys = system("x' = mu*x - y + x^2 + x*y\ny' = x + mu*y + x^2");
    run(&sys, &[-1.0], &[0.0, 0.0], &opts(0, (-1.0, 1.0), 0.05))
}

/// `Re λ = 0.01 − μ²`: two Hopf points 0.2 apart, which a first step of 0.5
/// growing to 1 would cross together and miss without the proximity limit.
fn hopf_dip() -> Branch {
    let sys = system("x' = (0.01 - mu^2)*x - y\ny' = x + (0.01 - mu^2)*y");
    let o = ContinuationOptions {
        ds_max: 1.0,
        ..opts(0, (-1.0, 1.0), 0.5)
    };
    run(&sys, &[-1.0], &[0.0, 0.0], &o)
}

/// `λ = (μ ± √(μ² − 0.04))/2`: the pair is complex only for `|μ| < 0.2`, so
/// a long step goes from two real eigenvalues straight to an unstable
/// spiral, and only the unstable count sees the Hopf point inside it.
fn hopf_from_a_node() -> Branch {
    let sys = system("x' = y\ny' = -0.01*x + mu*y");
    let o = ContinuationOptions {
        ds_max: 0.5,
        ..opts(0, (-1.0, 1.0), 0.05)
    };
    run(&sys, &[-1.0], &[0.0, 0.0], &o)
}

fn neutral_saddle() -> Branch {
    let sys = system("x' = y\ny' = x + mu*y");
    run(&sys, &[-1.0], &[0.0, 0.0], &opts(0, (-1.0, 1.0), 0.05))
}

const SELKOV_A: f64 = 0.08;

fn selkov(ds: f64, ds_max: f64) -> Branch {
    let sys = system("x' = -x + a*y + x^2*y\ny' = b - a*y - x^2*y");
    let (ia, ib) = (param(&sys, "a"), param(&sys, "b"));
    let mut params = vec![0.0; 2];
    params[ia] = SELKOV_A;
    params[ib] = 0.05;
    let o = ContinuationOptions {
        ds_max,
        ..opts(ib, (0.05, 1.5), ds)
    };
    run(&sys, &params, &[0.06, 0.6], &o)
}

fn brusselator() -> Branch {
    let sys = system("x' = a - (b + 1)*x + x^2*y\ny' = b*x - x^2*y");
    let (ia, ib) = (param(&sys, "a"), param(&sys, "b"));
    let mut params = vec![0.0; 2];
    params[ia] = 2.0;
    params[ib] = 1.0;
    run(&sys, &params, &[2.0, 0.5], &opts(ib, (1.0, 8.0), 0.05))
}

fn logistic(x0: f64, range: (f64, f64)) -> Branch {
    let sys = system("x_{n+1} = r*x_n*(1 - x_n)");
    run(&sys, &[range.0], &[x0], &opts(0, range, 0.05))
}

const HENON_B: f64 = 0.3;

/// Hénon's map from its fixed point `x₊` at `a = 0.2`, followed with `ds`.
fn henon(ds: f64) -> Branch {
    let sys = system("x_{n+1} = 1 - a*x_n^2 + y_n\ny_{n+1} = b*x_n");
    let (ia, ib) = (param(&sys, "a"), param(&sys, "b"));
    let (a, b) = (0.2, HENON_B);
    let mut params = vec![0.0; 2];
    params[ia] = a;
    params[ib] = b;
    let x = (-(1.0 - b) + ((1.0 - b) * (1.0 - b) + 4.0 * a).sqrt()) / (2.0 * a);
    let o = ContinuationOptions {
        max_points: 300,
        ..opts(ia, (-0.5, 1.0), ds)
    };
    run(&sys, &params, &[x, b * x], &o)
}

fn delayed_logistic() -> Branch {
    let sys = system("x_{n+1} = r*x_n*(1 - y_n)\ny_{n+1} = x_n");
    let x0 = 1.0 - 1.0 / 1.5;
    run(&sys, &[1.5], &[x0, x0], &opts(0, (1.5, 2.5), 0.05))
}

fn kinds(b: &Branch) -> Vec<SpecialKind> {
    b.special.iter().map(|sp| sp.kind).collect()
}

/// The one special point of `kind` on `b`.
fn only(b: &Branch, kind: SpecialKind) -> &dyn_analysis::SpecialPoint {
    let mut of_kind = b.special.iter().filter(|sp| sp.kind == kind);
    let sp = of_kind
        .next()
        .unwrap_or_else(|| panic!("no {} in {:?}", kind.label(), kinds(b)));
    assert!(
        of_kind.next().is_none(),
        "two {} in {:?}",
        kind.label(),
        kinds(b)
    );
    sp
}

#[test]
fn fold_of_saddle_node_at_r_0() {
    let b = saddle_node();
    assert_eq!(kinds(&b), [SpecialKind::Fold]);
    let lp = &b.special[0];
    assert!(lp.p.abs() <= 1e-10, "r = {}", lp.p);
    assert!(lp.x[0].abs() <= 1e-9, "x = {}", lp.x[0]);
    assert_eq!((lp.omega, lp.lyapunov), (None, None));
}

#[test]
fn branch_points_at_r_0() {
    for source in ["x' = r*x - x^3", "x' = r*x - x^2"] {
        let b = trivial_branch(source);
        assert_eq!(kinds(&b), [SpecialKind::BranchPoint], "{source}");
        let bp = &b.special[0];
        assert!(bp.p.abs() <= 1e-10, "{source}: r = {}", bp.p);
        assert_eq!(bp.x, b.points[bp.index].x);
    }
}

#[test]
fn hopf_normal_form() {
    for (sign, l1) in [("-", -1.0), ("+", 1.0)] {
        let b = hopf(sign);
        assert_eq!(kinds(&b), [SpecialKind::Hopf], "{sign}");
        let h = &b.special[0];
        assert!(h.p.abs() <= 1e-10, "μ = {}", h.p);
        let omega = h.omega.expect("ω");
        assert!((omega - 1.0).abs() <= 1e-9, "ω = {omega}");
        let l = h.lyapunov.expect("ℓ₁");
        assert!((l - l1).abs() <= 1e-6, "ℓ₁ = {l} vs {l1}");
    }
}

#[test]
fn hopf_lyapunov_coefficient_from_quadratic_terms() {
    let b = hopf_quadratic();
    let h = only(&b, SpecialKind::Hopf);
    assert!(h.p.abs() <= 1e-10, "μ = {}", h.p);
    let l = h.lyapunov.expect("ℓ₁");
    assert!((l + 0.125).abs() <= 1e-6, "ℓ₁ = {l}");
}

#[test]
fn close_hopf_points_are_not_stepped_over() {
    let b = hopf_dip();
    assert_eq!(kinds(&b), [SpecialKind::Hopf, SpecialKind::Hopf]);
    for (h, mu) in b.special.iter().zip([-0.1, 0.1]) {
        assert!((h.p - mu).abs() <= 1e-10, "μ = {} vs {mu}", h.p);
    }
}

#[test]
fn hopf_inside_a_step_from_a_node() {
    let b = hopf_from_a_node();
    assert_eq!(kinds(&b), [SpecialKind::Hopf]);
    let h = &b.special[0];
    assert!(h.p.abs() <= 1e-10, "μ = {}", h.p);
    let omega = h.omega.expect("ω");
    assert!((omega - 0.1).abs() <= 1e-9, "ω = {omega}");
}

#[test]
fn neutral_saddle_is_not_hopf() {
    let b = neutral_saddle();
    assert_eq!(b.end, BranchEnd::Range);
    assert!(b.special.is_empty(), "{:?}", kinds(&b));
}

#[test]
fn selkov_two_hopf_points() {
    for (ds, ds_max) in [(0.05, 0.1), (1.0, 2.0)] {
        let b = selkov(ds, ds_max);
        assert_eq!(b.end, BranchEnd::Range, "ds = {ds}");
        assert_eq!(
            kinds(&b),
            [SpecialKind::Hopf, SpecialKind::Hopf],
            "ds = {ds}"
        );
        for (h, b2) in b.special.iter().zip([0.12_f64, 0.72]) {
            assert!(
                (h.p - b2.sqrt()).abs() <= 1e-9,
                "ds = {ds}: b = {} vs √{b2}",
                h.p
            );
            let omega = (SELKOV_A + b2).sqrt();
            let got = h.omega.expect("ω");
            assert!(
                (got - omega).abs() <= 1e-9,
                "ds = {ds}: ω = {got} vs {omega}"
            );
        }
    }
}

#[test]
fn brusselator_hopf() {
    let b = brusselator();
    let h = only(&b, SpecialKind::Hopf);
    assert!((h.p - 5.0).abs() <= 1e-9, "b = {}", h.p);
    let omega = h.omega.expect("ω");
    assert!((omega - 2.0).abs() <= 1e-9, "ω = {omega}");
    assert_eq!(kinds(&b), [SpecialKind::Hopf]);
}

#[test]
fn logistic_flip_at_3() {
    let b = logistic(0.6, (2.5, 3.5));
    assert_eq!(kinds(&b), [SpecialKind::Flip]);
    let pd = &b.special[0];
    assert!((pd.p - 3.0).abs() <= 1e-10, "r = {}", pd.p);
    assert!(
        (pd.eigenvalues[0].0 + 1.0).abs() <= 1e-9,
        "{:?}",
        pd.eigenvalues
    );
}

#[test]
fn logistic_transcritical_at_1_is_a_branch_point() {
    let b = logistic(0.0, (0.5, 1.5));
    assert_eq!(b.end, BranchEnd::Range);
    assert_eq!(kinds(&b), [SpecialKind::BranchPoint]);
    assert!(
        (b.special[0].p - 1.0).abs() <= 1e-10,
        "r = {}",
        b.special[0].p
    );
}

#[test]
fn henon_fold_and_flip() {
    let c = (1.0 - HENON_B) * (1.0 - HENON_B);
    let lp = henon(-0.05);
    let fold = only(&lp, SpecialKind::Fold);
    assert!((fold.p + c / 4.0).abs() <= 1e-9, "a = {}", fold.p);
    let pd = henon(0.05);
    let flip = only(&pd, SpecialKind::Flip);
    assert!((flip.p - 3.0 * c / 4.0).abs() <= 1e-9, "a = {}", flip.p);
}

#[test]
fn delayed_logistic_neimark_sacker() {
    let b = delayed_logistic();
    assert_eq!(kinds(&b), [SpecialKind::NeimarkSacker]);
    let ns = &b.special[0];
    assert!((ns.p - 2.0).abs() <= 1e-9, "r = {}", ns.p);
    let theta = ns.omega.expect("θ");
    let third = std::f64::consts::PI / 3.0;
    assert!((theta - third).abs() <= 1e-9, "θ = {theta}");
}

/// The jump of the unstable count across a special point of `kind`.
fn jump(kind: SpecialKind) -> usize {
    match kind {
        SpecialKind::Fold | SpecialKind::BranchPoint | SpecialKind::Flip => 1,
        SpecialKind::Hopf | SpecialKind::NeimarkSacker => 2,
    }
}

#[test]
fn unstable_count_is_explained() {
    let branches = [
        ("saddle-node", saddle_node()),
        ("pitchfork", trivial_branch("x' = r*x - x^3")),
        ("transcritical", trivial_branch("x' = r*x - x^2")),
        ("Hopf −", hopf("-")),
        ("Hopf +", hopf("+")),
        ("Hopf quadratic", hopf_quadratic()),
        ("Hopf dip", hopf_dip()),
        ("Hopf from a node", hopf_from_a_node()),
        ("neutral saddle", neutral_saddle()),
        ("Sel'kov", selkov(0.05, 0.1)),
        ("Sel'kov ds 1", selkov(1.0, 2.0)),
        ("Brusselator", brusselator()),
        ("logistic", logistic(0.6, (2.5, 3.5))),
        ("logistic x = 0", logistic(0.0, (0.5, 1.5))),
        ("Hénon back", henon(-0.05)),
        ("Hénon forward", henon(0.05)),
        ("delayed logistic", delayed_logistic()),
    ];
    for (name, b) in &branches {
        let special: Vec<usize> = b.special.iter().map(|sp| sp.index).collect();
        assert!(
            b.special.windows(2).all(|w| w[0].index < w[1].index),
            "{name}"
        );
        for (i, w) in b.points.windows(2).enumerate() {
            if special.contains(&i) || special.contains(&(i + 1)) {
                continue;
            }
            assert_eq!(
                w[0].unstable, w[1].unstable,
                "{name}: unstable changes between r = {} and {} with no special point",
                w[0].p, w[1].p
            );
        }
        for sp in &b.special {
            let i = sp.index;
            assert!(
                i > 0 && i + 1 < b.points.len(),
                "{name}: {} at an end",
                sp.kind.label()
            );
            let (before, at, after) = (&b.points[i - 1], &b.points[i], &b.points[i + 1]);
            assert_eq!(
                before.unstable.abs_diff(after.unstable),
                jump(sp.kind),
                "{name}: {} at {}",
                sp.kind.label(),
                sp.p
            );
            assert_eq!(at.unstable, before.unstable.min(after.unstable), "{name}");
            assert!(!at.stable, "{name}: {} is neutral", sp.kind.label());
            assert_eq!((at.p, &at.x), (sp.p, &sp.x), "{name}");
        }
    }
}
