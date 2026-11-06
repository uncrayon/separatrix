use dyn_analysis::{lyapunov_spectrum, LyapunovOptions, LyapunovSpectrum};
use dyn_core::{FnField, SystemKind};
use dyn_solve::{Method, SolverOptions, Status};

const RK4_001: SolverOptions = SolverOptions {
    method: Method::Rk4,
    rtol: 1e-6,
    atol: 1e-9,
    h: 0.01,
    h_max: 0.0,
    max_steps: 10_000_000,
};

fn opts(transient: f64, total: f64, interval: f64) -> LyapunovOptions {
    LyapunovOptions {
        transient,
        total,
        interval,
        blocks: 10,
        history: 500,
    }
}

fn lorenz() -> FnField {
    FnField::new(3, 3, SystemKind::Ode, |_t, x, p, o| {
        o[0] = p[0] * (x[1] - x[0]);
        o[1] = x[0] * (p[1] - x[2]) - x[1];
        o[2] = x[0] * x[1] - p[2] * x[2];
    })
    .with_jacobian(|_t, x, p, j| {
        j.copy_from_slice(&[
            -p[0],
            p[0],
            0.0,
            p[1] - x[2],
            -1.0,
            -x[0],
            x[1],
            x[0],
            -p[2],
        ]);
    })
}

const LORENZ: [f64; 3] = [10.0, 28.0, 8.0 / 3.0];

fn lorenz_run() -> LyapunovSpectrum {
    lyapunov_spectrum(
        &lorenz(),
        &LORENZ,
        0.0,
        &[0.0, 1.0, 0.0],
        &RK4_001,
        &opts(100.0, 1e4, 0.1),
    )
}

#[test]
fn linear_flow_has_exponent_a() {
    for a in [0.3, -0.7] {
        let f = FnField::new(1, 1, SystemKind::Ode, |_t, x, p, o| o[0] = p[0] * x[0]);
        let s = lyapunov_spectrum(&f, &[a], 0.0, &[0.0], &RK4_001, &opts(0.0, 50.0, 0.5));
        assert_eq!(s.status, Status::Completed);
        assert_eq!(s.renormalizations, 100);
        assert!((s.exponents[0] - a).abs() < 1e-9, "{a}: {:?}", s.exponents);
        assert!((s.trace_average - a).abs() < 1e-9);
        assert!(s.uncertainty[0] < 1e-9);
    }
}

#[test]
fn diagonal_map_has_exponents_ln2_and_minus_ln2() {
    let f = FnField::new(2, 0, SystemKind::Map, |_n, x, _p, o| {
        o[0] = 2.0 * x[0];
        o[1] = 0.5 * x[1];
    });
    let s = lyapunov_spectrum(
        &f,
        &[],
        0.0,
        &[0.0, 0.0],
        &RK4_001,
        &opts(10.0, 1000.0, 1.0),
    );
    assert_eq!(s.status, Status::Completed);
    assert_eq!(s.renormalizations, 1000);
    let ln2 = 2f64.ln();
    assert!((s.exponents[0] - ln2).abs() < 1e-9, "{:?}", s.exponents);
    assert!((s.exponents[1] + ln2).abs() < 1e-9, "{:?}", s.exponents);
    assert!(s.sum.abs() < 1e-9);
    assert!(s.trace_average.abs() < 1e-9);
}

#[test]
fn several_iterates_per_qr_step_measure_the_same_map() {
    let f = FnField::new(2, 0, SystemKind::Map, |_n, x, _p, o| {
        o[0] = 1.5 * x[0] + x[1];
        o[1] = 0.25 * x[1];
    });
    let s = lyapunov_spectrum(&f, &[], 0.0, &[0.0, 0.0], &RK4_001, &opts(0.0, 999.0, 7.0));
    assert_eq!(s.status, Status::Completed);
    assert_eq!(s.renormalizations, 143);
    assert!(
        (s.exponents[0] - 1.5f64.ln()).abs() < 1e-9,
        "{:?}",
        s.exponents
    );
    assert!(
        (s.exponents[1] - 0.25f64.ln()).abs() < 1e-9,
        "{:?}",
        s.exponents
    );
}

#[test]
fn lorenz_reproduces_sprott() {
    let s = lorenz_run();
    assert_eq!(s.status, Status::Completed);
    assert_eq!(s.renormalizations, 100_000);
    let published = [0.9056, 0.0, -14.5723];
    let tol = [0.01, 0.01, 0.02];
    for i in 0..3 {
        assert!(
            (s.exponents[i] - published[i]).abs() < tol[i],
            "λ{} = {} (published {})",
            i + 1,
            s.exponents[i],
            published[i]
        );
        assert!(s.uncertainty[i] > 0.0 && s.uncertainty[i] < tol[i]);
    }
}

#[test]
fn lorenz_sum_matches_the_trace() {
    let trace = -(10.0 + 1.0 + 8.0 / 3.0);
    let gap = |h: f64| {
        let solver = SolverOptions { h, ..RK4_001 };
        let s = lyapunov_spectrum(
            &lorenz(),
            &LORENZ,
            0.0,
            &[0.0, 1.0, 0.0],
            &solver,
            &opts(10.0, 100.0, 0.1),
        );
        assert_eq!(s.status, Status::Completed);
        assert!(
            (s.trace_average - trace).abs() < 1e-9,
            "{}",
            s.trace_average
        );
        (s.sum - s.trace_average).abs()
    };
    let (coarse, half, fine) = (gap(0.01), gap(0.005), gap(0.001));
    assert!(
        fine < 1e-6,
        "Σλ is {fine:e} from the mean trace at h = 0.001"
    );
    let order = (coarse / half).log2();
    assert!(
        (order - 4.0).abs() < 0.5,
        "the gap shrinks like h^{order}, not h^4"
    );
}

#[test]
fn running_estimates_end_at_the_spectrum() {
    let s = lorenz_run();
    let samples = s.history_t.len();
    assert!(samples > 0 && samples <= 500, "{samples}");
    assert_eq!(s.history.len(), samples * 3);
    assert!(s.history_t.windows(2).all(|w| w[0] < w[1]));
    assert!((s.history_t[samples - 1] - 1e4 - 100.0).abs() < 1e-6);
    assert_eq!(&s.history[(samples - 1) * 3..], &s.exponents[..]);
}

#[test]
fn a_map_leaving_the_threshold_blows_up() {
    let f = FnField::new(2, 0, SystemKind::Map, |_n, x, _p, o| {
        o[0] = 2.0 * x[0];
        o[1] = 0.5 * x[1];
    });
    let s = lyapunov_spectrum(
        &f,
        &[],
        0.0,
        &[1e-3, 1.0],
        &RK4_001,
        &opts(0.0, 1000.0, 1.0),
    );
    assert_eq!(s.status, Status::BlewUp);
    assert!(s.renormalizations < 60, "{}", s.renormalizations);
}

#[test]
fn a_flow_out_of_steps_says_so() {
    let solver = SolverOptions {
        max_steps: 500,
        ..RK4_001
    };
    let s = lyapunov_spectrum(
        &lorenz(),
        &LORENZ,
        0.0,
        &[0.0, 1.0, 0.0],
        &solver,
        &opts(1.0, 100.0, 0.1),
    );
    assert_eq!(s.status, Status::MaxSteps);
    assert_eq!(s.renormalizations, 40);
}
