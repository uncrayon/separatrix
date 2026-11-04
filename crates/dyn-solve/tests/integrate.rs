use std::f64::consts::{FRAC_PI_2, PI, TAU};

use dyn_core::{FnField, SystemKind};
use dyn_solve::*;

fn decay() -> FnField {
    FnField::new(1, 1, SystemKind::Ode, |_t, x, p, out| out[0] = -p[0] * x[0])
}

fn oscillator() -> FnField {
    FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[1];
        out[1] = -x[0];
    })
}

fn lorenz() -> FnField {
    FnField::new(3, 3, SystemKind::Ode, |_t, x, p, o| {
        o[0] = p[0] * (x[1] - x[0]);
        o[1] = x[0] * (p[1] - x[2]) - x[1];
        o[2] = x[0] * x[1] - p[2] * x[2];
    })
}
const LORENZ_P: [f64; 3] = [10.0, 28.0, 8.0 / 3.0];

fn fixed(method: Method, h: f64) -> SolverOptions {
    SolverOptions {
        method,
        h,
        max_steps: 10_000_000,
        ..SolverOptions::default()
    }
}

fn dopri(rtol: f64, atol: f64) -> SolverOptions {
    SolverOptions {
        method: Method::Dopri5,
        rtol,
        atol,
        max_steps: 10_000_000,
        ..SolverOptions::default()
    }
}

fn final_state(sol: &Solution) -> Vec<f64> {
    sol.last().unwrap().1.to_vec()
}

fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

fn x_squared() -> FnField {
    FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0] * x[0])
}

fn exp_growth() -> FnField {
    FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0].exp())
}

fn forced_decay() -> FnField {
    FnField::new(1, 0, SystemKind::Ode, |t, x, _p, out| {
        out[0] = -x[0] + t.sin()
    })
    .non_autonomous()
}

/// Solution of `x' = -x + sin t` with `x(0) = 2`.
fn forced_decay_exact(t: f64) -> f64 {
    0.5 * (t.sin() - t.cos()) + 2.5 * (-t).exp()
}

/// `x' = -x^{1/2} - 1`, which reaches 0 and then takes the root of a
/// negative number.
fn sqrt_into_negative() -> FnField {
    FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = -x[0].sqrt() - 1.0
    })
}

fn pole_at_two(m: i32) -> FnField {
    FnField::new(1, 0, SystemKind::Ode, move |_t, x, _p, out| {
        out[0] = 1.0 / (2.0 - x[0]).powi(m)
    })
}

#[test]
fn exponential_decay_matches_exact_solution() {
    let f = decay();
    let cases = [
        (fixed(Method::Euler, 1e-4), 1e-4),
        (fixed(Method::Heun, 1e-3), 1e-6),
        (fixed(Method::Rk4, 1e-2), 1e-9),
        (dopri(1e-8, 1e-10), 1e-7),
    ];
    for (opts, tol) in cases {
        let sol = integrate(&f, &[0.7], 0.0, &[2.0], 3.0, &opts, Sampling::Steps);
        assert_eq!(sol.status, Status::Completed);
        for (i, &t) in sol.t.iter().enumerate() {
            let exact = 2.0 * (-0.7 * t).exp();
            assert!(
                (sol.state(i)[0] - exact).abs() < tol,
                "{:?} at t={t}",
                opts.method
            );
        }
        assert_eq!(*sol.t.last().unwrap(), 3.0);
    }
}

#[test]
fn harmonic_oscillator_returns_after_one_period() {
    let f = oscillator();
    let cases = [
        (fixed(Method::Euler, 1e-5), 1e-3),
        (fixed(Method::Heun, 1e-3), 1e-5),
        (fixed(Method::Rk4, 1e-2), 1e-8),
        (dopri(1e-10, 1e-12), 1e-8),
    ];
    for (opts, tol) in cases {
        let sol = integrate(&f, &[], 0.0, &[1.0, 0.0], TAU, &opts, Sampling::Steps);
        assert_eq!(sol.status, Status::Completed);
        let err = max_abs_diff(&final_state(&sol), &[1.0, 0.0]);
        assert!(err < tol, "{:?}: error {err}", opts.method);
    }
}

fn oscillator_error(method: Method, h: f64) -> f64 {
    let sol = integrate(
        &oscillator(),
        &[],
        0.0,
        &[1.0, 0.0],
        1.0,
        &fixed(method, h),
        Sampling::Steps,
    );
    max_abs_diff(&final_state(&sol), &[1f64.cos(), -1f64.sin()])
}

#[test]
fn empirical_convergence_orders() {
    for (method, order, h0) in [
        (Method::Euler, 1.0, 0.01),
        (Method::Heun, 2.0, 0.01),
        (Method::Rk4, 4.0, 0.1),
    ] {
        let errors: Vec<f64> = (0..4)
            .map(|k| oscillator_error(method, h0 / 2f64.powi(k)))
            .collect();
        for w in errors.windows(2) {
            let slope = (w[0] / w[1]).log2();
            assert!(
                (slope - order).abs() < 0.3,
                "{method:?}: observed order {slope}, errors {errors:?}"
            );
        }
    }
}

#[test]
fn dopri5_global_error_scales_with_tolerance() {
    let f = oscillator();
    let exact = [10f64.cos(), -10f64.sin()];
    let mut prev = f64::INFINITY;
    for k in [4, 6, 8, 10] {
        let tol = 10f64.powi(-k);
        let sol = integrate(
            &f,
            &[],
            0.0,
            &[1.0, 0.0],
            10.0,
            &dopri(tol, tol),
            Sampling::Steps,
        );
        let err = max_abs_diff(&final_state(&sol), &exact);
        assert!(
            err < 100.0 * tol && err > 1e-4 * tol,
            "tol {tol}: err {err}"
        );
        assert!(err < prev / 10.0, "tol {tol}: err {err}, previous {prev}");
        prev = err;
    }
}

#[test]
fn dopri5_statistics_are_sensible() {
    let f = decay();
    let sol = integrate(
        &f,
        &[1.0],
        0.0,
        &[1.0],
        10.0,
        &dopri(1e-6, 1e-9),
        Sampling::Steps,
    );
    let s = sol.stats;
    assert_eq!(sol.status, Status::Completed);
    assert_eq!(sol.len(), s.accepted + 1);
    assert!(s.accepted > 5 && s.accepted < 200, "{s:?}");
    assert!(s.rejected <= s.accepted / 4 + 2, "{s:?}");
    assert_eq!(s.evals, 6 * (s.accepted + s.rejected) + 2, "{s:?}");
}

#[test]
fn dopri5_tighter_tolerance_takes_more_steps() {
    let f = decay();
    let run = |rtol, atol| {
        integrate(
            &f,
            &[1.0],
            0.0,
            &[1.0],
            10.0,
            &dopri(rtol, atol),
            Sampling::Steps,
        )
        .stats
    };
    let (loose, tight) = (run(1e-6, 1e-9), run(1e-10, 1e-13));
    assert!(
        tight.accepted > 3 * loose.accepted,
        "{tight:?} vs {loose:?}"
    );
}

#[test]
fn dopri5_too_large_user_step_is_rejected_first() {
    let f = decay();
    let sol = integrate(
        &f,
        &[50.0],
        0.0,
        &[1.0],
        1.0,
        &SolverOptions {
            h: 1.0,
            ..dopri(1e-6, 1e-9)
        },
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::Completed);
    assert!(sol.stats.rejected >= 1);
    assert_eq!(
        sol.stats.evals,
        6 * (sol.stats.accepted + sol.stats.rejected) + 1
    );
}

#[test]
fn fixed_step_statistics() {
    let f = decay();
    for (method, per_step) in [(Method::Euler, 1), (Method::Heun, 2), (Method::Rk4, 4)] {
        let sol = integrate(
            &f,
            &[1.0],
            0.0,
            &[1.0],
            1.0,
            &fixed(method, 0.1),
            Sampling::Steps,
        );
        assert_eq!(sol.stats.accepted, 10, "{method:?}");
        assert_eq!(sol.stats.rejected, 0);
        assert_eq!(sol.stats.evals, 1 + per_step * 10, "{method:?}");
    }
}

#[test]
fn dense_output_is_accurate_at_midpoints() {
    let f = oscillator();
    let o = dopri(1e-9, 1e-12);
    let mut s = Stepper::new(&f, &[], 0.0, &[1.0, 0.0], 20.0, &o);
    let mut out = [0.0; 2];
    let mut worst: f64 = 0.0;
    while s.step() == StepOutcome::Accepted {
        for frac in [0.25, 0.5, 0.75] {
            let t = s.t_prev() + frac * s.h_last();
            s.interpolate(t, &mut out);
            worst = worst.max(max_abs_diff(&out, &[t.cos(), -t.sin()]));
        }
    }
    assert!(worst < 1e-7, "dopri5 dense error {worst}");

    let mut s = Stepper::new(&f, &[], 0.0, &[1.0, 0.0], 5.0, &fixed(Method::Rk4, 0.05));
    let mut worst: f64 = 0.0;
    while s.step() == StepOutcome::Accepted {
        let t = s.t_prev() + 0.5 * s.h_last();
        s.interpolate(t, &mut out);
        worst = worst.max(max_abs_diff(&out, &[t.cos(), -t.sin()]));
    }
    assert!(worst < 1e-6, "rk4 + hermite dense error {worst}");
}

#[test]
fn uniform_sampling_count_and_endpoints() {
    let f = oscillator();
    for opts in [
        dopri(1e-8, 1e-10),
        fixed(Method::Rk4, 0.013),
        fixed(Method::Euler, 0.001),
    ] {
        for n in [2, 7, 101, 1000] {
            let sol = integrate(
                &f,
                &[],
                1.0,
                &[1f64.cos(), -1f64.sin()],
                4.0,
                &opts,
                Sampling::Uniform(n),
            );
            assert_eq!(sol.status, Status::Completed);
            assert_eq!(sol.len(), n);
            assert_eq!(sol.x.len(), 2 * n);
            assert_eq!(sol.t[0], 1.0);
            assert_eq!(sol.t[n - 1], 4.0);
            for (k, &t) in sol.t.iter().enumerate() {
                let expected = 1.0 + 3.0 * k as f64 / (n - 1) as f64;
                assert!((t - expected).abs() < 1e-14);
            }
            if opts.method == Method::Dopri5 {
                for i in 0..n {
                    let t = sol.t[i];
                    assert!(max_abs_diff(sol.state(i), &[t.cos(), -t.sin()]) < 1e-6);
                }
            }
        }
    }

    let sol = integrate(
        &f,
        &[],
        0.0,
        &[1.0, 0.0],
        1.0,
        &dopri(1e-6, 1e-9),
        Sampling::Uniform(1),
    );
    assert_eq!(sol.t, vec![0.0]);
    let sol = integrate(
        &f,
        &[],
        0.0,
        &[1.0, 0.0],
        1.0,
        &dopri(1e-6, 1e-9),
        Sampling::Uniform(0),
    );
    assert!(sol.is_empty());
    let sol = integrate(
        &f,
        &[],
        2.0,
        &[1.0, 0.0],
        2.0,
        &dopri(1e-6, 1e-9),
        Sampling::Uniform(3),
    );
    assert_eq!(sol.t, vec![2.0, 2.0, 2.0]);
    assert_eq!(sol.status, Status::Completed);
}

#[test]
fn backward_integration_round_trip() {
    let f = lorenz();
    let x0 = [1.0, 1.0, 1.0];
    let o = dopri(1e-11, 1e-12);
    let fwd = integrate(&f, &LORENZ_P, 0.0, &x0, 0.5, &o, Sampling::Uniform(11));
    let bwd = integrate(
        &f,
        &LORENZ_P,
        0.5,
        &final_state(&fwd),
        0.0,
        &o,
        Sampling::Uniform(11),
    );
    assert_eq!(bwd.status, Status::Completed);
    assert_eq!(bwd.t[0], 0.5);
    assert_eq!(bwd.t[10], 0.0);
    assert!(bwd.t.windows(2).all(|w| w[1] < w[0]));
    assert!(
        max_abs_diff(&final_state(&bwd), &x0) < 1e-6,
        "{:?}",
        final_state(&bwd)
    );

    let sol = integrate(
        &decay(),
        &[1.0],
        1.0,
        &[1.0],
        0.0,
        &fixed(Method::Rk4, 0.01),
        Sampling::Steps,
    );
    assert_eq!(sol.len(), 101);
    assert!((final_state(&sol)[0] - 1f64.exp()).abs() < 1e-9);
}

#[test]
fn blow_up_of_x_squared() {
    let f = x_squared();
    let sol = integrate(
        &f,
        &[],
        0.0,
        &[1.0],
        2.0,
        &SolverOptions::default(),
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::BlewUp, "{:?}", sol.stats);
    let (t, x) = sol.last().unwrap();
    assert!((t - 1.0).abs() < 1e-5, "stopped at {t}");
    assert!(x[0] > BLOW_UP_THRESHOLD);
}

#[test]
fn blow_up_of_x_squared_on_a_uniform_grid_keeps_finite_samples() {
    let f = x_squared();
    let sol = integrate(
        &f,
        &[],
        0.0,
        &[1.0],
        2.0,
        &SolverOptions::default(),
        Sampling::Uniform(21),
    );
    assert_eq!(sol.status, Status::BlewUp);
    assert!(sol.len() == 11 || sol.len() == 12, "{:?}", sol.t);
    assert!(sol.x.iter().all(|v| v.is_finite()));
    assert!(sol.t.windows(2).all(|w| w[0] < w[1]));
    assert!((sol.state(9)[0] - 10.0).abs() < 1e-4);
}

#[test]
fn blow_up_of_x_squared_with_rk4() {
    let f = x_squared();
    let sol = integrate(
        &f,
        &[],
        0.0,
        &[1.0],
        2.0,
        &fixed(Method::Rk4, 0.01),
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::BlewUp);
    assert!((sol.last().unwrap().0 - 1.0).abs() < 0.05);
}

#[test]
fn lorenz_dopri5_agrees_with_fine_rk4() {
    let f = lorenz();
    let x0 = [1.0, 1.0, 1.0];
    let a = integrate(
        &f,
        &LORENZ_P,
        0.0,
        &x0,
        1.0,
        &dopri(1e-10, 1e-12),
        Sampling::Steps,
    );
    let b = integrate(
        &f,
        &LORENZ_P,
        0.0,
        &x0,
        1.0,
        &fixed(Method::Rk4, 1e-4),
        Sampling::Uniform(2),
    );
    assert_eq!(a.status, Status::Completed);
    assert_eq!(b.status, Status::Completed);
    let d = max_abs_diff(&final_state(&a), &final_state(&b));
    assert!(d < 1e-7, "difference {d}");
}

#[test]
fn non_autonomous_forced_decay() {
    let f = forced_decay();
    let exact = forced_decay_exact;
    for opts in [
        dopri(1e-9, 1e-12),
        fixed(Method::Rk4, 0.01),
        fixed(Method::Heun, 0.001),
    ] {
        let sol = integrate(&f, &[], 0.0, &[2.0], 15.0, &opts, Sampling::Uniform(151));
        assert_eq!(sol.status, Status::Completed);
        for (i, &t) in sol.t.iter().enumerate() {
            assert!(
                (sol.state(i)[0] - exact(t)).abs() < 1e-6,
                "{:?} at t = {t}",
                opts.method
            );
        }
    }
}

#[test]
fn non_autonomous_forced_decay_backwards_from_15_to_12() {
    let f = forced_decay();
    let exact = forced_decay_exact;
    let sol = integrate(
        &f,
        &[],
        15.0,
        &[exact(15.0)],
        12.0,
        &dopri(1e-10, 1e-12),
        Sampling::Steps,
    );
    assert!((final_state(&sol)[0] - exact(12.0)).abs() < 1e-7);
}

#[test]
fn max_steps_returns_partial_solution() {
    let o = SolverOptions {
        max_steps: 25,
        ..fixed(Method::Rk4, 0.01)
    };
    let sol = integrate(
        &oscillator(),
        &[],
        0.0,
        &[1.0, 0.0],
        10.0,
        &o,
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::MaxSteps);
    assert_eq!(sol.len(), 26);
    assert!((sol.last().unwrap().0 - 0.25).abs() < 1e-12);

    let sol = integrate(
        &oscillator(),
        &[],
        0.0,
        &[1.0, 0.0],
        10.0,
        &o,
        Sampling::Uniform(101),
    );
    assert_eq!(sol.status, Status::MaxSteps);
    assert_eq!(sol.len(), 4);

    let o = SolverOptions {
        max_steps: 100,
        ..fixed(Method::Rk4, 0.01)
    };
    let sol = integrate(
        &oscillator(),
        &[],
        0.0,
        &[1.0, 0.0],
        1.0,
        &o,
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::Completed);
}

#[test]
fn non_finite_values_stop_the_integration() {
    let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = -0.5 / x[0].sqrt()
    });
    let sol = integrate(
        &f,
        &[],
        0.0,
        &[1.0],
        2.0,
        &SolverOptions::default(),
        Sampling::Steps,
    );
    assert!(
        matches!(sol.status, Status::NonFinite | Status::StepTooSmall),
        "{:?}",
        sol.status
    );
    assert!(sol.x.iter().all(|v| v.is_finite()));
    let t_end = sol.last().unwrap().0;
    assert!((t_end - 4.0 / 3.0).abs() < 1e-4, "{t_end}");
}

#[test]
fn rk4_root_of_negative_is_non_finite() {
    let h = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[0].sqrt() - 1.0
    });
    let sol = integrate(
        &h,
        &[],
        0.0,
        &[0.5],
        5.0,
        &fixed(Method::Rk4, 0.1),
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::NonFinite);
    assert!(sol.x.iter().all(|v| v.is_finite()));
}

#[test]
fn nan_initial_state_is_non_finite_and_empty() {
    let sol = integrate(
        &decay(),
        &[1.0],
        0.0,
        &[f64::NAN],
        1.0,
        &SolverOptions::default(),
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::NonFinite);
    assert!(sol.is_empty());
}

#[test]
fn nan_field_keeps_only_the_start_sample() {
    let g = FnField::new(1, 0, SystemKind::Ode, |_t, _x, _p, out| out[0] = f64::NAN);
    let sol = integrate(
        &g,
        &[],
        0.0,
        &[1.0],
        1.0,
        &SolverOptions::default(),
        Sampling::Uniform(10),
    );
    assert_eq!(sol.status, Status::NonFinite);
    assert_eq!(sol.t, vec![0.0]);
}

#[test]
fn step_too_small_near_a_singularity() {
    let f = FnField::new(1, 0, SystemKind::Ode, |t, _x, _p, out| {
        out[0] = 1.0 / (1.0 - t).sqrt()
    })
    .non_autonomous();
    let sol = integrate(
        &f,
        &[],
        0.0,
        &[0.0],
        2.0,
        &dopri(1e-8, 1e-10),
        Sampling::Steps,
    );
    assert!(
        matches!(sol.status, Status::StepTooSmall | Status::NonFinite),
        "{:?} at t = {}",
        sol.status,
        sol.last().unwrap().0
    );
    assert!((sol.last().unwrap().0 - 1.0).abs() < 1e-6);
    assert!((sol.last().unwrap().1[0] - 2.0).abs() < 1e-3);
}

#[test]
fn logarithmic_blow_up_is_blew_up() {
    let f = FnField::new(1, 0, SystemKind::Ode, |t, _x, _p, out| {
        out[0] = 1.0 / (1.0 - t)
    })
    .non_autonomous();
    let sol = integrate(
        &f,
        &[],
        0.0,
        &[0.0],
        2.0,
        &dopri(1e-8, 1e-10),
        Sampling::Steps,
    );
    assert_eq!(
        sol.status,
        Status::BlewUp,
        "at t = {}",
        sol.last().unwrap().0
    );
    assert!((sol.last().unwrap().0 - 1.0).abs() < 1e-6);
}

fn zero_of_x(direction: Direction) -> Event {
    Event {
        g: Box::new(|_t, x| x[0]),
        direction,
    }
}

#[test]
fn events_locate_zeros_of_the_oscillator() {
    let f = oscillator();
    let o = dopri(1e-12, 1e-12);
    let events = [zero_of_x(Direction::Both)];
    let (sol, hits) = integrate_with_events(
        &f,
        &[],
        0.0,
        &[1.0, 0.0],
        20.0,
        &o,
        Sampling::Uniform(50),
        &events,
    );
    assert_eq!(sol.status, Status::Completed);
    assert_eq!(sol.len(), 50);
    assert_eq!(hits.len(), 6);
    for (k, hit) in hits.iter().enumerate() {
        let exact = FRAC_PI_2 + k as f64 * PI;
        assert!(
            (hit.t - exact).abs() < 1e-9,
            "hit {k}: {} vs {exact}",
            hit.t
        );
        assert!(hit.x[0].abs() < 1e-9);
        assert_eq!(hit.event, 0);
        assert_eq!(hit.rising, k % 2 == 1);
        assert!((hit.x[1].abs() - 1.0).abs() < 1e-9);
    }
}

#[test]
fn events_direction_filtering_and_multiple_events() {
    let f = oscillator();
    let o = dopri(1e-12, 1e-12);
    let events = [
        zero_of_x(Direction::Rising),
        zero_of_x(Direction::Falling),
        Event {
            g: Box::new(|_t, x| x[1]),
            direction: Direction::Both,
        },
    ];
    let (_, hits) = integrate_with_events(
        &f,
        &[],
        0.0,
        &[1.0, 0.0],
        10.0,
        &o,
        Sampling::Steps,
        &events,
    );

    let times = |e: usize| {
        hits.iter()
            .filter(|h| h.event == e)
            .map(|h| h.t)
            .collect::<Vec<_>>()
    };
    let rising = times(0);
    let falling = times(1);
    let v_zero = times(2);
    assert_eq!(rising.len(), 1);
    assert!((rising[0] - 1.5 * PI).abs() < 1e-9);
    assert_eq!(falling.len(), 2);
    assert!((falling[0] - FRAC_PI_2).abs() < 1e-9);
    assert!((falling[1] - 2.5 * PI).abs() < 1e-9);
    assert!(hits.iter().filter(|h| h.event == 0).all(|h| h.rising));
    assert!(hits.iter().filter(|h| h.event == 1).all(|h| !h.rising));
    assert_eq!(v_zero.len(), 3);
    for (k, t) in v_zero.iter().enumerate() {
        assert!((t - (k + 1) as f64 * PI).abs() < 1e-9);
    }
    assert!(hits.windows(2).all(|w| w[0].t <= w[1].t));
}

#[test]
fn events_with_fixed_step() {
    let f = oscillator();
    let events = [zero_of_x(Direction::Both)];
    let (_, hits) = integrate_with_events(
        &f,
        &[],
        0.0,
        &[1.0, 0.0],
        5.0,
        &fixed(Method::Rk4, 0.01),
        Sampling::Steps,
        &events,
    );
    assert_eq!(hits.len(), 2);
    assert!((hits[0].t - FRAC_PI_2).abs() < 1e-8);
    assert!((hits[1].t - 1.5 * PI).abs() < 1e-8);
}

#[test]
fn events_backwards_with_dopri5() {
    let f = oscillator();
    let events = [zero_of_x(Direction::Both)];
    let (_, hits) = integrate_with_events(
        &f,
        &[],
        0.0,
        &[1.0, 0.0],
        -5.0,
        &dopri(1e-12, 1e-12),
        Sampling::Steps,
        &events,
    );
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert!((hits[0].t + FRAC_PI_2).abs() < 1e-9);
    assert!(!hits[0].rising);
    assert!((hits[1].t + 1.5 * PI).abs() < 1e-9);
    assert!(hits[1].rising);
}

#[test]
fn integrate_and_integrate_with_no_events_agree() {
    let f = lorenz();
    let o = SolverOptions::default();
    let a = integrate(
        &f,
        &LORENZ_P,
        0.0,
        &[1.0, 2.0, 3.0],
        3.0,
        &o,
        Sampling::Uniform(30),
    );
    let (b, hits) = integrate_with_events(
        &f,
        &LORENZ_P,
        0.0,
        &[1.0, 2.0, 3.0],
        3.0,
        &o,
        Sampling::Uniform(30),
        &[],
    );
    assert_eq!(a, b);
    assert!(hits.is_empty());
}

#[test]
fn fixed_step_exponential_blow_up_is_blew_up() {
    let f = exp_growth();
    for method in [Method::Euler, Method::Heun, Method::Rk4] {
        for h in [0.1, 0.01, 0.001] {
            for sampling in [Sampling::Steps, Sampling::Uniform(501)] {
                let sol = integrate(&f, &[], 0.0, &[0.0], 5.0, &fixed(method, h), sampling);
                let (t, x) = sol.last().unwrap();
                assert_eq!(
                    sol.status,
                    Status::BlewUp,
                    "{method:?}, h = {h}: stopped at t = {t}, x = {x:?}"
                );
                assert!(sol.x.iter().all(|v| v.is_finite()), "{method:?}, h = {h}");
                assert!(t > 0.9 && t < 1.5, "{method:?}, h = {h}: t = {t}");
                if sampling == Sampling::Steps {
                    assert!(x[0] > 5.0, "{method:?}, h = {h}: x = {x:?}");
                }
            }
        }
    }
}

#[test]
fn fixed_step_backwards_exponential_blow_up_is_blew_up() {
    let g = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = (-x[0]).exp()
    });
    for method in [Method::Euler, Method::Heun, Method::Rk4] {
        let sol = integrate(
            &g,
            &[],
            0.0,
            &[0.0],
            -5.0,
            &fixed(method, 0.01),
            Sampling::Steps,
        );
        assert_eq!(sol.status, Status::BlewUp, "{method:?}");
        let (t, x) = sol.last().unwrap();
        assert!(t < -0.9 && t > -1.1, "{method:?}: t = {t}");
        assert!(x[0] < -5.0, "{method:?}: x = {x:?}");
    }
}

#[test]
fn fixed_step_overflow_driven_by_another_component_is_blew_up() {
    let k = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[1].exp();
        out[1] = 1.0;
    });
    for method in [Method::Euler, Method::Heun, Method::Rk4] {
        let sol = integrate(
            &k,
            &[],
            0.0,
            &[0.0, 700.0],
            20.0,
            &fixed(method, 0.1),
            Sampling::Steps,
        );
        assert_eq!(sol.status, Status::BlewUp, "{method:?}");
    }
}

#[test]
fn fixed_step_nan_is_still_non_finite() {
    let f = sqrt_into_negative();
    for method in [Method::Euler, Method::Heun, Method::Rk4] {
        for h in [0.1, 0.01] {
            let sol = integrate(
                &f,
                &[],
                0.0,
                &[1.0],
                10.0,
                &fixed(method, h),
                Sampling::Steps,
            );
            assert_eq!(sol.status, Status::NonFinite, "{method:?}, h = {h}");
            assert!(sol.x.iter().all(|v| v.is_finite()));
        }
    }
}

#[test]
fn fixed_step_nan_in_a_growing_system_is_non_finite() {
    let g = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[0];
        out[1] = (5.0 - x[0]).sqrt();
    });
    for method in [Method::Euler, Method::Heun, Method::Rk4] {
        let sol = integrate(
            &g,
            &[],
            0.0,
            &[1.0, 0.0],
            10.0,
            &fixed(method, 0.01),
            Sampling::Steps,
        );
        assert_eq!(sol.status, Status::NonFinite, "{method:?}");
    }
}

#[test]
fn fixed_step_landing_on_the_pole_of_inverse_x_is_non_finite() {
    let k = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -1.0 / x[0]);
    let sol = integrate(
        &k,
        &[],
        0.0,
        &[1.0],
        5.0,
        &fixed(Method::Euler, 1.0),
        Sampling::Steps,
    );
    assert_eq!(sol.status, Status::NonFinite);
    assert_eq!(sol.x, vec![1.0]);
}

#[test]
fn fixed_step_on_a_finite_pole_is_not_blew_up() {
    for m in [1, 3] {
        let pole = pole_at_two(m);
        for method in [Method::Euler, Method::Heun, Method::Rk4] {
            let sol = integrate(
                &pole,
                &[],
                0.0,
                &[1.0],
                5.0,
                &fixed(method, 1.0),
                Sampling::Steps,
            );
            assert_ne!(
                sol.status,
                Status::BlewUp,
                "m = {m}, {method:?}: {:?}",
                sol.x
            );
            assert!(sol.x.iter().all(|v| v.is_finite()));
        }
    }
}

#[test]
fn fixed_step_landing_on_a_finite_pole_is_non_finite() {
    for m in [1, 3] {
        let pole = pole_at_two(m);
        let sol = integrate(
            &pole,
            &[],
            0.0,
            &[1.0],
            5.0,
            &fixed(Method::Euler, 1.0),
            Sampling::Steps,
        );
        assert_eq!(sol.status, Status::NonFinite, "m = {m}");
        assert_eq!(sol.x, vec![1.0], "m = {m}");
        let sol = integrate(
            &pole,
            &[],
            0.0,
            &[1.0],
            5.0,
            &fixed(Method::Heun, 1.0),
            Sampling::Steps,
        );
        assert_eq!(sol.status, Status::NonFinite, "m = {m}");
    }
}
