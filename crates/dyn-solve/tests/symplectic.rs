use dyn_core::{FnField, SystemKind};
use dyn_solve::{
    integrate, section, Method, Sampling, Section, SolverOptions, Status, StepOutcome, Stepper,
};

fn oscillator() -> FnField {
    FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[1];
        out[1] = -x[0];
    })
}

fn pendulum() -> FnField {
    FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[1];
        out[1] = -x[0].sin();
    })
}

fn pendulum_energy(x: &[f64]) -> f64 {
    x[1] * x[1] / 2.0 - x[0].cos()
}

fn fixed(method: Method, h: f64) -> SolverOptions {
    SolverOptions {
        method,
        h,
        max_steps: 10_000_000,
        ..SolverOptions::default()
    }
}

fn end_state(f: &FnField, method: Method, h: f64, x0: &[f64], t0: f64, t1: f64) -> Vec<f64> {
    let sol = integrate(f, &[], t0, x0, t1, &fixed(method, h), Sampling::Steps);
    assert_eq!(sol.status, Status::Completed);
    let (t, x) = sol.last().unwrap();
    assert_eq!(t, t1);
    x.to_vec()
}

fn oscillator_error(method: Method, h: f64) -> f64 {
    const T: f64 = 10.0;
    let x = end_state(&oscillator(), method, h, &[1.0, 0.0], 0.0, T);
    (x[0] - T.cos()).hypot(x[1] + T.sin())
}

fn order_ratio(method: Method, h: f64) -> f64 {
    let (coarse, fine) = (
        oscillator_error(method, h),
        oscillator_error(method, h / 2.0),
    );
    let ratio = coarse / fine;
    println!(
        "{method:?}: error {coarse:.3e} at h = {h}, {fine:.3e} at h = {}, ratio {ratio:.3}",
        h / 2.0
    );
    ratio
}

#[test]
fn halving_h_divides_the_verlet_error_by_4() {
    let ratio = order_ratio(Method::Verlet, 0.1);
    assert!((ratio - 4.0).abs() <= 0.4, "{ratio}");
}

#[test]
fn halving_h_divides_the_yoshida_error_by_16() {
    let ratio = order_ratio(Method::Yoshida4, 0.2);
    assert!((ratio - 16.0).abs() <= 2.0, "{ratio}");
}

#[test]
fn forward_then_backward_returns_to_the_start() {
    let f = pendulum();
    let x0 = [1.0, 0.0];
    for (method, h) in [(Method::Verlet, 0.05), (Method::Yoshida4, 0.1)] {
        let there = end_state(&f, method, h, &x0, 0.0, 100.0);
        let back = end_state(&f, method, h, &there, 100.0, 0.0);
        let gap = (back[0] - x0[0]).abs().max((back[1] - x0[1]).abs());
        println!("{method:?}: back at t = 0 within {gap:.1e}");
        assert!(gap <= 1e-10, "{method:?}: {back:?}");
    }
}

fn max_energy_error(method: Method, h: f64, t1: f64) -> f64 {
    let f = pendulum();
    let x0 = [1.0, 0.0];
    let h0 = pendulum_energy(&x0);
    let opts = fixed(method, h);
    let mut s = Stepper::new(&f, &[], 0.0, &x0, t1, &opts);
    let mut worst: f64 = 0.0;
    while s.step() == StepOutcome::Accepted {
        worst = worst.max((pendulum_energy(s.x()) - h0).abs());
    }
    assert_eq!(s.t(), t1);
    worst
}

#[test]
fn pendulum_energy_stays_bounded() {
    for (method, h, bound) in [(Method::Verlet, 0.05, 1e-3), (Method::Yoshida4, 0.1, 1e-4)] {
        let early = max_energy_error(method, h, 1e3);
        let all = max_energy_error(method, h, 1e4);
        println!("{method:?} h = {h}: max |ΔH| {early:.3e} to t = 1e3, {all:.3e} to t = 1e4");
        assert!(all <= bound, "{method:?}: {all}");
        assert!(all <= 2.0 * early, "{method:?}: {early} then {all}");
    }
}

#[test]
fn energy_error_shrinks_with_the_order() {
    let verlet = max_energy_error(Method::Verlet, 0.1, 100.0)
        / max_energy_error(Method::Verlet, 0.05, 100.0);
    let yoshida = max_energy_error(Method::Yoshida4, 0.2, 100.0)
        / max_energy_error(Method::Yoshida4, 0.1, 100.0);
    println!("max |ΔH| ratio on halving h: Verlet {verlet:.3}, Yoshida 4 {yoshida:.3}");
    assert!((verlet - 4.0).abs() <= 0.4, "{verlet}");
    assert!((yoshida - 16.0).abs() <= 2.0, "{yoshida}");
}

#[test]
fn a_poincare_section_reads_the_dense_output() {
    let opts = fixed(Method::Yoshida4, 0.05);
    let hits = section(
        &oscillator(),
        &[],
        0.0,
        &[1.0, 0.0],
        20.0,
        &opts,
        &Section::Plane {
            state: 0,
            value: 0.0,
            direction: dyn_solve::Direction::Both,
        },
        0,
        100,
    );
    assert_eq!(hits.status, Status::Completed);
    let expect = [-1.0, 1.0, -1.0, 1.0, -1.0, 1.0];
    assert_eq!(hits.len(), expect.len());
    for (k, &p) in expect.iter().enumerate() {
        let t = std::f64::consts::FRAC_PI_2 * (2 * k + 1) as f64;
        assert!((hits.t[k] - t).abs() < 1e-4, "{k}: t = {}", hits.t[k]);
        assert!(
            (hits.state(k)[1] - p).abs() < 1e-4,
            "{k}: p = {}",
            hits.state(k)[1]
        );
    }
}

#[test]
fn an_automatic_step_is_the_span_over_1000() {
    let f = oscillator();
    let s = Stepper::new(&f, &[], 0.0, &[1.0, 0.0], 5.0, &fixed(Method::Verlet, 0.0));
    assert_eq!(s.h_next(), 0.005);
}

#[test]
fn verlet_costs_three_evaluations_a_step_and_yoshida_seven() {
    for (method, per_step) in [(Method::Verlet, 3), (Method::Yoshida4, 7)] {
        let sol = integrate(
            &oscillator(),
            &[],
            0.0,
            &[1.0, 0.0],
            1.0,
            &fixed(method, 0.1),
            Sampling::Steps,
        );
        assert_eq!(sol.stats.accepted, 10);
        assert_eq!(sol.stats.evals, 1 + per_step * 10, "{method:?}");
    }
}
