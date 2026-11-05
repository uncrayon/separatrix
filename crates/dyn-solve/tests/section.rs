use std::f64::consts::TAU;

use dyn_core::{FnField, SystemKind};
use dyn_solve::{section, Direction, Section, SolverOptions, Status};

fn tight() -> SolverOptions {
    SolverOptions {
        rtol: 1e-11,
        atol: 1e-13,
        max_steps: 1_000_000,
        ..SolverOptions::default()
    }
}

fn harmonic() -> FnField {
    FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[1];
        out[1] = -x[0];
    })
}

const Y_RISING: Section = Section::Plane {
    state: 1,
    value: 0.0,
    direction: Direction::Rising,
};

#[test]
fn hopf_normal_form_crossings_sit_on_the_limit_cycle() {
    let mu = 0.25;
    let f = FnField::new(2, 1, SystemKind::Ode, |_t, x, p, out| {
        let r2 = x[0] * x[0] + x[1] * x[1];
        out[0] = p[0] * x[0] - x[1] - x[0] * r2;
        out[1] = x[0] + p[0] * x[1] - x[1] * r2;
    });
    let sol = section(
        &f,
        &[mu],
        0.0,
        &[0.1, 0.0],
        60.0 * TAU,
        &tight(),
        &Y_RISING,
        20,
        30,
    );
    assert_eq!(sol.status, Status::Completed);
    assert_eq!(sol.len(), 30);
    let radius = mu.sqrt();
    let dx = (0..sol.len())
        .map(|i| (sol.state(i)[0] - radius).abs())
        .fold(0.0, f64::max);
    let dt = sol
        .t
        .windows(2)
        .map(|w| (w[1] - w[0] - TAU).abs())
        .fold(0.0, f64::max);
    println!("hopf mu = 0.25: max |x − √μ| = {dx:.3e}, max |Δt − 2π| = {dt:.3e}");
    assert!(dx < 1e-7, "max |x − √μ| = {dx:e}");
    assert!(dt < 1e-7, "max |Δt − 2π| = {dt:e}");
    assert!(
        (sol.t[0] - 21.0 * TAU).abs() < 1e-7,
        "the 21st crossing is the first kept: {}",
        sol.t[0]
    );
}

#[test]
fn stroboscopic_samples_converge_to_the_forced_response() {
    let f = FnField::new(1, 0, SystemKind::Ode, |t, x, _p, out| {
        out[0] = -x[0] + t.sin()
    });
    let strobe = Section::Stroboscopic {
        period: TAU,
        phase: 0.0,
    };
    let sol = section(&f, &[], 0.0, &[1.0], 12.0 * TAU, &tight(), &strobe, 0, 1000);
    assert_eq!(sol.status, Status::Completed);
    assert_eq!(sol.len(), 13, "t = 0, 2π, …, 24π");
    for (k, &t) in sol.t.iter().enumerate() {
        assert_eq!(t, k as f64 * TAU, "t_{k}");
    }
    assert_eq!(sol.state(0), &[1.0]);
    let worst = (5..sol.len())
        .map(|k| (sol.state(k)[0] + 0.5).abs())
        .fold(0.0, f64::max);
    println!("stroboscopic: max |x_k + 1/2| for k ≥ 5 = {worst:.3e}");
    assert!(worst < 1e-8, "{worst:e}");
}

#[test]
fn both_directions_count_twice_the_rising_crossings() {
    let f = harmonic();
    let opts = tight();
    let count = |direction| {
        let plane = Section::Plane {
            state: 1,
            value: 0.3,
            direction,
        };
        section(&f, &[], 0.0, &[1.0, 0.0], 100.0, &opts, &plane, 0, 10_000).len()
    };
    let (rising, falling, both) = (
        count(Direction::Rising),
        count(Direction::Falling),
        count(Direction::Both),
    );
    assert!(rising > 10, "{rising}");
    assert_eq!(both, rising + falling);
    assert!(both.abs_diff(2 * rising) <= 1, "{both} vs 2 × {rising}");
}

#[test]
fn skip_and_max_points_are_exact() {
    let f = harmonic();
    let opts = tight();
    let all = section(&f, &[], 0.0, &[1.0, 0.0], 1000.0, &opts, &Y_RISING, 0, 10);
    let some = section(&f, &[], 0.0, &[1.0, 0.0], 1000.0, &opts, &Y_RISING, 3, 7);
    assert_eq!(all.len(), 10);
    assert_eq!(some.len(), 7);
    assert_eq!(some.t, all.t[3..]);
    assert_eq!(some.x, all.x[6..]);
    assert_eq!(some.status, Status::Completed);
    let to_end = section(
        &f,
        &[],
        0.0,
        &[1.0, 0.0],
        1000.0,
        &opts,
        &Y_RISING,
        0,
        1_000_000,
    );
    assert_eq!(to_end.len(), (1000.0 / TAU) as usize);
    assert!(
        all.stats.accepted * 10 < to_end.stats.accepted,
        "the run stops at the last wanted crossing: {} vs {} steps",
        all.stats.accepted,
        to_end.stats.accepted
    );
    let none = section(&f, &[], 0.0, &[1.0, 0.0], 1000.0, &opts, &Y_RISING, 4, 0);
    assert!(none.is_empty());
    assert_eq!(none.status, Status::Completed);
}

#[test]
fn backward_integration_meets_the_section_in_reverse_time() {
    let f = harmonic();
    let opts = tight();
    let sol = section(&f, &[], 0.0, &[1.0, 0.0], -20.0, &opts, &Y_RISING, 0, 100);
    assert_eq!(sol.status, Status::Completed);
    assert_eq!(sol.len(), 3, "{:?}", sol.t);
    for (i, &t) in sol.t.iter().enumerate() {
        let k = (i + 1) as f64;
        assert!((t + k * TAU).abs() < 1e-9, "t = {t}");
        assert!((sol.state(i)[0] - 1.0).abs() < 1e-9);
    }

    let strobe = Section::Stroboscopic {
        period: TAU,
        phase: 0.0,
    };
    let sol = section(&f, &[], 0.0, &[1.0, 0.0], -20.0, &opts, &strobe, 0, 100);
    assert_eq!(sol.t, vec![0.0, -TAU, -2.0 * TAU, -3.0 * TAU]);
    for i in 0..sol.len() {
        assert!((sol.state(i)[0] - 1.0).abs() < 1e-9 && sol.state(i)[1].abs() < 1e-9);
    }
}

#[test]
fn a_phase_moves_every_stroboscopic_sample() {
    let f = harmonic();
    let strobe = Section::Stroboscopic {
        period: 2.0,
        phase: -2.5,
    };
    let sol = section(&f, &[], 1.0, &[1.0, 0.0], 8.0, &tight(), &strobe, 0, 100);
    let want: Vec<f64> = (2..=5).map(|k| -2.5 + k as f64 * 2.0).collect();
    assert_eq!(sol.t, want, "t_k = 1.5, 3.5, 5.5, 7.5");
    for (i, &t) in sol.t.iter().enumerate() {
        assert!((sol.state(i)[0] - (t - 1.0).cos()).abs() < 1e-9);
    }
}

#[test]
fn the_first_stroboscopic_sample_is_the_first_on_or_after_t0() {
    let f = harmonic();
    let first = |t0: f64, phase: f64| {
        let strobe = Section::Stroboscopic { period: 0.2, phase };
        section(&f, &[], t0, &[1.0, 0.0], t0 + 1.0, &tight(), &strobe, 0, 1).t[0]
    };
    let (t0, phase): (f64, f64) = (0.063, 0.863);
    assert_eq!(((t0 - phase) / 0.2).ceil(), -4.0);
    assert!(
        phase + -4.0 * 0.2 < t0,
        "ceil((t0 − phase)/period) = −4 rounds below t0"
    );
    assert_eq!(first(t0, phase), phase + -3.0 * 0.2);
    let (t0, phase): (f64, f64) = (4.727, 3.927);
    assert_eq!(((t0 - phase) / 0.2).ceil(), 5.0);
    assert_eq!(
        phase + 4.0 * 0.2,
        t0,
        "ceil((t0 − phase)/period) = 5 skips t0"
    );
    assert_eq!(first(t0, phase), t0);
}

#[test]
fn an_orbit_into_a_stable_node_never_crosses() {
    let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = -x[0];
        out[1] = -2.0 * x[1];
    });
    let plane = Section::Plane {
        state: 1,
        value: 0.0,
        direction: Direction::Both,
    };
    let sol = section(&f, &[], 0.0, &[1.0, 1.0], 50.0, &tight(), &plane, 0, 100);
    assert_eq!(sol.status, Status::Completed);
    assert!(sol.is_empty());
}

#[test]
fn a_blow_up_stops_the_section() {
    let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0] * x[0]);
    let plane = Section::Plane {
        state: 0,
        value: 2.0,
        direction: Direction::Rising,
    };
    let sol = section(
        &f,
        &[],
        0.0,
        &[1.0],
        2.0,
        &SolverOptions::default(),
        &plane,
        0,
        100,
    );
    assert_eq!(sol.status, Status::BlewUp);
    assert_eq!(sol.len(), 1, "x = 1/(1 − t) crosses 2 once, at t = 1/2");
    assert!((sol.t[0] - 0.5).abs() < 1e-6, "{}", sol.t[0]);
    assert!((sol.state(0)[0] - 2.0).abs() < 1e-9);
}
