use dyn_core::{FnField, SystemKind};
use dyn_solve::*;

fn x_squared() -> FnField {
    FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0] * x[0])
}

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
