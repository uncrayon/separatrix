use dyn_analysis::{continue_branch, Branch, BranchEnd, ContinuationOptions};
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
        assert_eq!(pt.stable, x < 0.0, "λ = 2x = {} at x = {x}", 2.0 * x);
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
        assert_eq!(pt.stable, pt.p < 0.0, "λ = r = {}", pt.p);
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
