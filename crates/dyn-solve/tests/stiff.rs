//! Rosenbrock 2(3) on stiff problems, and Dormand–Prince's stiffness test.
//!
//! Van der Pol at μ = 1000 is written in the Liénard plane of the
//! relaxation-oscillator template, `x' = μ(y − x³/3 + x)`, `y' = −x/μ`.
//! Robertson's reaction (Hairer & Wanner, *Solving ODEs II*, §IV.1) is
//! `y1' = −0.04y1 + 1e4y2y3`, `y2' = 0.04y1 − 1e4y2y3 − 3e7y2²`,
//! `y3' = 3e7y2²` from `(1, 0, 0)`.

use dyn_core::{FnField, SystemKind};
use dyn_solve::{
    integrate, section, Direction, Method, Sampling, Section, SolverOptions, Status, StepOutcome,
    Stepper,
};

/// The relaxation oscillator with `p = [μ]`.
fn lienard() -> FnField {
    FnField::new(2, 1, SystemKind::Ode, |_t, x, p, out| {
        out[0] = p[0] * (x[1] - x[0] * x[0] * x[0] / 3.0 + x[0]);
        out[1] = -x[0] / p[0];
    })
    .with_jacobian(|_t, x, p, out| {
        out[0] = p[0] * (1.0 - x[0] * x[0]);
        out[1] = p[0];
        out[2] = -1.0 / p[0];
        out[3] = 0.0;
    })
}

fn robertson() -> FnField {
    FnField::new(3, 0, SystemKind::Ode, |_t, y, _p, out| {
        out[0] = -0.04 * y[0] + 1e4 * y[1] * y[2];
        out[1] = 0.04 * y[0] - 1e4 * y[1] * y[2] - 3e7 * y[1] * y[1];
        out[2] = 3e7 * y[1] * y[1];
    })
    .with_jacobian(|_t, y, _p, out| {
        out.copy_from_slice(&[
            -0.04,
            1e4 * y[2],
            1e4 * y[1],
            0.04,
            -1e4 * y[2] - 6e7 * y[1],
            -1e4 * y[1],
            0.0,
            6e7 * y[1],
            0.0,
        ]);
    })
}

/// Robertson at t = 40 (SUNDIALS `cvRoberts_dns`; Hairer & Wanner).
const ROBERTSON_40: [f64; 3] = [7.158271e-1, 9.185535e-6, 2.841637e-1];

fn adaptive(method: Method, rtol: f64, atol: f64) -> SolverOptions {
    SolverOptions {
        method,
        rtol,
        atol,
        max_steps: 100_000_000,
        ..SolverOptions::default()
    }
}

/// Steps of exactly `h`: the tolerances are so loose that every try is
/// accepted, and `h_max` stops the step from growing.
fn fixed(h: f64) -> SolverOptions {
    SolverOptions {
        method: Method::Rosenbrock23,
        rtol: 1e30,
        atol: 1e30,
        h,
        h_max: h,
        max_steps: 100_000_000,
    }
}

struct Counts {
    accepted: usize,
    rejected: usize,
    looks_stiff: bool,
    end: Vec<f64>,
}

fn run(f: &FnField, p: &[f64], x0: &[f64], t1: f64, opts: &SolverOptions) -> Counts {
    let sol = integrate(f, p, 0.0, x0, t1, opts, Sampling::Uniform(2));
    assert_eq!(sol.status, Status::Completed, "{:?}", opts.method);
    assert_eq!(sol.t.last(), Some(&t1));
    Counts {
        accepted: sol.stats.accepted,
        rejected: sol.stats.rejected,
        looks_stiff: sol.stiff,
        end: sol.state(sol.len() - 1).to_vec(),
    }
}

/// One problem for Dormand–Prince and Rosenbrock 2(3) at the same tolerances.
struct Case {
    name: &'static str,
    f: FnField,
    p: Vec<f64>,
    x0: Vec<f64>,
    t1: f64,
    rtol: f64,
    atol: f64,
    stiff: bool,
}

#[test]
fn dormand_prince_looks_stiff_where_rosenbrock_does_not_need_to() {
    let vdp = |mu: f64, t1: f64, stiff: bool, name| Case {
        name,
        f: lienard(),
        p: vec![mu],
        x0: vec![2.0, 0.0],
        t1,
        rtol: 1e-6,
        atol: 1e-9,
        stiff,
    };
    let cases = [
        vdp(
            1000.0,
            3000.0,
            true,
            "Van der Pol μ = 1000, Liénard, t = 3000",
        ),
        Case {
            name: "Robertson, t = 40",
            f: robertson(),
            p: vec![],
            x0: vec![1.0, 0.0, 0.0],
            t1: 40.0,
            rtol: 1e-8,
            atol: 1e-14,
            stiff: true,
        },
        vdp(10.0, 40.0, false, "Van der Pol μ = 10, Liénard, t = 40"),
        vdp(1.0, 40.0, false, "Van der Pol μ = 1, Liénard, t = 40"),
    ];
    println!(
        "{:<40} {:<13} {:>9} {:>8} {:>9} {:>11}",
        "problem", "method", "accepted", "rejected", "rej/acc", "looks_stiff"
    );
    for c in &cases {
        let name = c.name;
        let dopri = run(
            &c.f,
            &c.p,
            &c.x0,
            c.t1,
            &adaptive(Method::Dopri5, c.rtol, c.atol),
        );
        let ros = run(
            &c.f,
            &c.p,
            &c.x0,
            c.t1,
            &adaptive(Method::Rosenbrock23, c.rtol, c.atol),
        );
        for (label, n) in [("Dopri5", &dopri), ("Rosenbrock23", &ros)] {
            println!(
                "{name:<40} {label:<13} {:>9} {:>8} {:>9.4} {:>11}",
                n.accepted,
                n.rejected,
                n.rejected as f64 / n.accepted as f64,
                n.looks_stiff
            );
        }
        assert_eq!(dopri.looks_stiff, c.stiff, "{name}");
        assert!(
            !ros.looks_stiff,
            "{name}: only Dormand–Prince runs the test"
        );
        if c.stiff {
            assert!(
                ros.accepted * 4 <= dopri.accepted,
                "{name}: {} vs {}",
                ros.accepted,
                dopri.accepted
            );
        }
    }
}

fn decay_error(h: f64) -> f64 {
    let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -x[0])
        .with_jacobian(|_t, _x, _p, out| out[0] = -1.0);
    let c = run(&f, &[], &[1.0], 1.0, &fixed(h));
    assert_eq!(c.accepted, (1.0 / h).round() as usize, "fixed steps of {h}");
    (c.end[0] - (-1.0f64).exp()).abs()
}

#[test]
fn rosenbrock_converges_at_order_2() {
    let (coarse, fine) = (decay_error(0.1), decay_error(0.05));
    let ratio = coarse / fine;
    println!(
        "x' = −x to t = 1: error {coarse:.4e} at h = 0.1, {fine:.4e} at h = 0.05, ratio {ratio:.4}"
    );
    assert!((ratio - 4.0).abs() <= 0.5, "{ratio}");
}

/// `y' = −1e6 (y − cos t)` stepped with a fixed `h` from `y(0) = y0`: the
/// largest `|y − cos t|` after the first step, and over `t ≤ 10`.
fn stiff_tracking(h: f64, y0: f64) -> (f64, f64) {
    let f = FnField::new(1, 0, SystemKind::Ode, |t, y, _p, out| {
        out[0] = -1e6 * (y[0] - t.cos())
    })
    .with_jacobian(|_t, _y, _p, out| out[0] = -1e6)
    .non_autonomous();
    let mut s = Stepper::new(&f, &[], 0.0, &[y0], 10.0, &fixed(h));
    let (mut first, mut worst) = (None, 0.0f64);
    let mut steps = 0;
    while s.step() == StepOutcome::Accepted {
        steps += 1;
        let e = (s.x()[0] - s.t().cos()).abs();
        first.get_or_insert(e);
        worst = worst.max(e);
    }
    assert_eq!(s.t(), 10.0);
    assert_eq!(steps, (10.0 / h).round() as usize);
    let first = first.unwrap();
    println!("y' = −1e6 (y − cos t), y(0) = {y0}, h = {h}: |y − cos t| = {first:.3e} after one step, max {worst:.3e}");
    (first, worst)
}

/// In the stiff limit `hλ → −∞` one step maps the error `e` off the slow
/// solution `g = cos t` to `R(∞)·e + h²·g''·(1/(8d) − 1/2)`, and `R(∞) = 0`
/// (L-stability). The second term, `0.0732·h²·|cos t|` at most, is the
/// scheme's order-2 error in that limit: 7.3e−4 at `h = 0.1`, 7.3e−6 at
/// `h = 0.01`.
#[test]
fn rosenbrock_is_l_stable() {
    let limit = |h: f64| h * h * (0.5 - (2.0 + std::f64::consts::SQRT_2) / 8.0);
    let (_, worst) = stiff_tracking(0.1, 1.0);
    assert!(worst <= limit(0.1) * 1.01, "{worst} vs {}", limit(0.1));
    let (_, worst) = stiff_tracking(0.01, 1.0);
    assert!(worst <= 1e-5, "{worst}");
    let (first, _) = stiff_tracking(0.1, 2.0);
    assert!(
        first <= limit(0.1) * 1.01,
        "an error of 1 off the slow solution is gone after one step: {first}"
    );
}

#[test]
fn robertson_reaches_t_40() {
    let c = run(
        &robertson(),
        &[],
        &[1.0, 0.0, 0.0],
        40.0,
        &adaptive(Method::Rosenbrock23, 1e-8, 1e-14),
    );
    println!(
        "Robertson t = 40, Rosenbrock23 rtol 1e-8 atol 1e-14, {} steps ({} rejected):",
        c.accepted, c.rejected
    );
    for (i, (y, r)) in c.end.iter().zip(ROBERTSON_40).enumerate() {
        let rel = (y - r).abs() / r.abs();
        println!(
            "  y{} = {y:.9e}  reference {r:.6e}  |Δ|/|ref| = {rel:.2e}",
            i + 1
        );
        assert!(rel <= 1e-5, "y{}: {rel}", i + 1);
    }
    let sum: f64 = c.end.iter().sum();
    assert!((sum - 1.0).abs() < 1e-12, "mass is conserved: {sum}");
}

#[test]
fn a_plane_section_of_a_stiff_run_returns_crossings() {
    let f = lienard();
    let plane = Section::Plane {
        state: 0,
        value: 0.0,
        direction: Direction::Both,
    };
    let opts = adaptive(Method::Rosenbrock23, 1e-6, 1e-9);
    let sol = section(
        &f,
        &[1000.0],
        0.0,
        &[2.0, 0.0],
        3000.0,
        &opts,
        &plane,
        0,
        100,
    );
    assert_eq!(sol.status, Status::Completed);
    println!(
        "x = 0 crossings, μ = 1000, t ≤ 3000: {:?} after {} steps",
        sol.t, sol.stats.accepted
    );
    // Two jumps per period T ≈ (3 − 2 ln 2)μ ≈ 1614, the first a half period in.
    assert!(sol.len() >= 3, "{:?}", sol.t);
    for i in 0..sol.len() {
        assert_eq!(sol.state(i)[0], 0.0);
    }
    for w in sol.t.windows(2) {
        let half = (3.0 - 2.0 * std::f64::consts::LN_2) * 1000.0 / 2.0;
        assert!((w[1] - w[0] - half).abs() < 0.05 * half, "{:?}", sol.t);
    }
}
