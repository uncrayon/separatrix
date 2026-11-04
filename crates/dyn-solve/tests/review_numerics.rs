//! Regression tests written during the numerics review (milestone 1).
//!
//! Each test demonstrates a defect found in review and is expected to FAIL
//! until the corresponding fix lands. See the comment on each test for the
//! failing scenario and the code it points at.

use dyn_core::{FnField, SystemKind};
use dyn_solve::*;

/// x' = x², x(0) = 1: x = 1/(1 - t), increasing and positive until it blows
/// up at t = 1 (Strogatz §2.5).
fn x_squared() -> FnField {
    FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0] * x[0])
}

/// Fixed-step methods build a cubic Hermite interpolant from the slopes at
/// both ends of each step (`stepper.rs`, `hermite_dense`). On the step that
/// triggers `BlewUp`, `f(x_new)` is astronomically larger than the secant
/// `(x_new - x) / h`, so the cubic swings far below zero inside the step.
/// With `Sampling::Uniform` (what the app uses for ODE trajectories) those
/// samples are returned: Euler gives a sample of about -7e25, Heun -9e44,
/// RK4 -2e22, although both the exact and the numerical solution are
/// monotone and >= 1. The UI then draws a spike to -infinity.
#[test]
fn fixed_step_blow_up_step_does_not_produce_wrong_sign_samples() {
    let f = x_squared();
    for method in [Method::Euler, Method::Heun, Method::Rk4] {
        let opts = SolverOptions {
            method,
            h: 0.1,
            ..SolverOptions::default()
        };
        let sol = integrate(&f, &[], 0.0, &[1.0], 30.0, &opts, Sampling::Uniform(3001));
        assert_eq!(sol.status, Status::BlewUp, "{method:?}");
        let min = sol.x.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(
            min >= 1.0,
            "{method:?}: a sample of x' = x^2 from x0 = 1 is {min:e} (must stay >= 1)"
        );
        assert!(
            sol.x.windows(2).all(|w| w[1] >= w[0]),
            "{method:?}: samples of an increasing solution are not monotone"
        );
    }
}

/// Finite-time blow-up faster than x' = x² never reaches the 1e12
/// threshold in double precision: x' = x³ from x0 = 1 blows up at t = 1/2,
/// but |x| = 1e12 would need t within 5e-25 of 1/2. Dopri5 instead stops
/// with `StepTooSmall` at x ≈ 3e6 (`stepper.rs`, `too_small`, together
/// with `BLOW_UP_THRESHOLD` in `lib.rs`). The same happens for x' = e^x
/// (stops at x ≈ 31). The UI therefore reports "step too small" instead of
/// "blew up" for textbook blow-up examples.
#[test]
fn fast_finite_time_blow_up_is_reported_as_blew_up() {
    let cubic = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
        out[0] = x[0].powi(3)
    });
    let exp = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0].exp());
    for (name, f, x0) in [("x^3", &cubic, 1.0), ("e^x", &exp, 0.0)] {
        let sol = integrate(
            f,
            &[],
            0.0,
            &[x0],
            5.0,
            &SolverOptions::default(),
            Sampling::Steps,
        );
        let (t, x) = sol.last().unwrap();
        assert_eq!(
            sol.status,
            Status::BlewUp,
            "x' = {name}: stopped at t = {t}, x = {x:?}"
        );
    }
}

/// With `atol = 0` (pure relative tolerance, accepted by the API) a state
/// component that stays exactly 0 gives `err_i / sk_i = 0 / 0 = NaN` in the
/// error norm (`stepper.rs`, `dopri5_stages`). Every step is then rejected
/// as if the right-hand side had produced NaN and integration ends with
/// `Status::NonFinite` at t0. Example: rabbits vs sheep (Strogatz §6.4)
/// started on the invariant axis y = 0.
#[test]
fn zero_atol_with_an_identically_zero_component_completes() {
    let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
        o[0] = x[0] * (3.0 - x[0] - 2.0 * x[1]);
        o[1] = x[1] * (2.0 - x[0] - x[1]);
    });
    let opts = SolverOptions {
        rtol: 1e-6,
        atol: 0.0,
        ..SolverOptions::default()
    };
    let sol = integrate(&f, &[], 0.0, &[1.0, 0.0], 5.0, &opts, Sampling::Steps);
    assert_eq!(sol.status, Status::Completed, "stopped at {:?}", sol.last());
}
