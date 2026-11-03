//! Follow-up checks for the fixes made after the adversarial review
//! (`review_parser.rs`): variants of each finding in both input formats.

mod common;

use common::*;
use dyn_expr::{compile, InputFormat, Severity};

fn error_message(src: &str, f: InputFormat) -> String {
    fails(src, f).into_iter().find(|d| d.severity == Severity::Error).unwrap().message
}

#[test]
fn jacobian_is_finite_where_a_root_argument_is_zero() {
    // Polar-style radius in a 3D system, at the origin.
    let sys = latex(r"\dot{x}=x\sqrt{x^2+y^2}\\\dot{y}=-y\\\dot{z}=\sqrt{x^2+y^2}-z");
    let j = jac(&sys, 0.0, &[0.0, 0.0, 0.0], &[]);
    assert!(j.iter().all(|v| v.is_finite()), "{j:?}");
    assert_eq!(j, [0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0]);
    // Hamiltonian (second derivatives through nested duals) with the root
    // of a parameter that is 0.
    let sys = latex(r"H(q,p)=\frac{p^2}{2}+\cos q+\sqrt{x^2}\\x=0");
    let j = jac(&sys, 0.0, &[0.3, 0.0], &[0.0]);
    assert_close(&j, &[0.0, 1.0, 0.3f64.cos(), 0.0], 1e-15);
    // Away from the singular point, still exact.
    let sys = latex(r"\dot{x}=x\left(1-\sqrt{x^2+y^2}\right)-y\\\dot{y}=y\left(1-\sqrt{x^2+y^2}\right)+x");
    check_derivatives(&sys, 0.0, &[0.3, -0.4], &[], 1e-6);
}

#[test]
fn function_argument_stops_before_any_derivative() {
    let x = [0.5, 0.75];
    let expect = [0.75, -0.5f64.cos() * 0.75];
    for src in [
        r"\ddot{\theta}=-\cos\theta\dot{\theta}",
        r"\theta''=-\cos\theta\theta'",
        r"\theta^{\prime\prime}=-\cos\theta\theta^{\prime}",
    ] {
        assert_close(&latex(src).eval_alloc(0.0, &x, &[]), &expect, 1e-15);
    }
    // Subscripted.
    let sys = latex(r"\ddot{x}_1=-\sin x_1\dot{x}_1");
    assert_close(&sys.eval_alloc(0.0, &x, &[]), &[0.75, -0.5f64.sin() * 0.75], 1e-15);
    // Ordinary factors are still part of the argument.
    let sys = latex(r"\dot{x}=\sin x y\\\dot{y}=0");
    assert_close(&sys.eval_alloc(0.0, &x, &[]), &[0.375f64.sin(), 0.0], 1e-15);
}

#[test]
fn equations_with_a_bare_symbol_on_the_left() {
    let sys = text("x = -x''");
    assert_eq!(names(&sys).0, ["x", "x'"]);
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 0.0], &[]), [0.0, -2.0]);
    // a = x' is x' = a.
    let sys = latex(r"a=\dot{x}");
    assert_eq!(names(&sys), (vec!["x".into()], vec!["a".into()]));
    // Plain values are still parameter defaults.
    let sys = latex(r"\dot{x}=-kx\\k=2");
    assert_eq!(sys.default_params(), [2.0]);
}

#[test]
fn leibniz_notation_anywhere() {
    let p = [0.3, 4.0];
    let x = [0.5, -2.0];
    let expect = [-2.0, -0.3 * -2.0 - 4.0 * 0.5];
    for (src, f) in [
        (r"\frac{d^2x}{dt^2}+c\frac{dx}{dt}+kx=0", InputFormat::Latex),
        (r"\ddot{x}=-c\frac{dx}{dt}-kx", InputFormat::Latex),
        (r"\frac{\mathrm{d}^2x}{\mathrm{d}t^2}=-c\dot{x}-kx", InputFormat::Latex),
        ("x'' + c*(dx/dt) + k*x = 0", InputFormat::Text),
    ] {
        let sys = ok(src, f);
        assert_eq!(names(&sys), (vec!["x".into(), "x'".into()], vec!["c".into(), "k".into()]), "{src}");
        assert_close(&sys.eval_alloc(0.0, &x, &p), &expect, 1e-15);
    }
}

#[test]
fn derivative_with_respect_to_another_variable_is_explained() {
    let m = error_message(r"\frac{dy}{dx}=x-y", InputFormat::Latex);
    assert!(m.contains("time t, not x"), "{m}");
    assert!(m.contains(r"\frac{dy}{dt}"), "{m}");
    assert!(m.contains(r"\dot{x} = 1"), "{m}");
    let m = error_message("dy/dx = x - y", InputFormat::Text);
    assert!(m.contains("dy/dt") && m.contains("x' = 1"), "{m}");
    let m = error_message(r"\frac{d^2y}{dx^2}=-y", InputFormat::Latex);
    assert!(m.contains("time t, not x"), "{m}");
}

#[test]
fn empty_boxes_name_what_is_empty() {
    let m = error_message(r"\dot{x}=\frac{}{2}", InputFormat::Latex);
    assert_eq!(m, r"The box after \frac is empty; fill it in");
    let m = error_message(r"\dot{x}=x^{}", InputFormat::Latex);
    assert_eq!(m, "The box after ^ is empty; fill it in");
    let m = error_message(r"\dot{x}=\sqrt{}", InputFormat::Latex);
    assert_eq!(m, r"The box after \sqrt is empty; fill it in");
    let m = error_message("x' = ()", InputFormat::Text);
    assert_eq!(m, "Nothing between ( and ); fill it in");
    let m = error_message(r"\dot{x}=\sin\left(\right)", InputFormat::Latex);
    assert_eq!(m, "Nothing between ( and ); fill it in");
}

#[test]
fn constant_zero_denominators() {
    for src in [r"\dot{x}=\frac{x}{1-1}", r"\dot{x}=\frac{x}{\sin0}", r"\dot{x}=x/(2\pi-2\pi)"] {
        let out = compile(src, InputFormat::Latex);
        let d = out.diagnostics.iter().find(|d| d.is_error()).expect(src);
        assert_eq!(d.message, "Division by zero", "{src}");
    }
    // Not constant, or not zero: fine.
    latex(r"\dot{x}=\frac{x}{1-a}");
    latex(r"\dot{x}=\frac{x}{1-0.5}");
}

#[test]
fn numbers_with_two_decimal_points() {
    let src = r"\dot{x}=1.2.3x";
    let out = compile(src, InputFormat::Latex);
    let d = out.diagnostics.iter().find(|d| d.is_error()).unwrap();
    assert_eq!(d.message, "A number can have only one decimal point");
    assert_eq!(&src[d.span.unwrap().0..d.span.unwrap().1], "1.2.3");
    assert!(compile("x' = 1..5*x", InputFormat::Text).system.is_none());
    assert!(compile("x' = 1.5e-3.2*x", InputFormat::Text).system.is_none());
    // A trailing point alone is still a number.
    assert_eq!(text("x' = 2.*x").eval_alloc(0.0, &[1.5], &[]), [3.0]);
}

#[test]
fn maps_of_subscripted_states() {
    // Hénon map with indexed states.
    let sys = latex(r"x_{1,n+1}=1-ax_{1,n}^2+x_{2,n}\\x_{2,n+1}=bx_{1,n}");
    assert_eq!(names(&sys), (vec!["x_1".into(), "x_2".into()], vec!["a".into(), "b".into()]));
    assert_eq!(sys.info.states[0].latex, "x_{1}");
    assert_eq!(sys.info.equations_latex[1], "x_{2,n+1} = b x_{1,n}");
    assert_close(&sys.eval_alloc(0.0, &[0.5, 0.2], &[1.4, 0.3]), &[1.0 - 0.35 + 0.2, 0.15], 1e-15);
    check_derivatives(&sys, 0.0, &[0.5, 0.2], &[1.4, 0.3], 1e-6);
    // Bare x_1 on the right also means x_{1,n}, as x means x_n.
    let sys = latex(r"x_{1,n+1}=x_2\\x_{2,n+1}=-x_1");
    assert_eq!(sys.eval_alloc(0.0, &[1.0, 2.0], &[]), [2.0, -1.0]);
    // Text mode spelling.
    let sys = text("x_1_{n+1} = x_2_n\nx_2_{n+1} = -x_1_n");
    assert_eq!(sys.eval_alloc(0.0, &[1.0, 2.0], &[]), [2.0, -1.0]);
    // Other uses of a comma in a subscript are explained.
    let m = error_message(r"\dot{x}=k_{1,5}x", InputFormat::Latex);
    assert!(m.contains("x_{1,n}"), "{m}");
}

#[test]
fn hamiltonian_duplicate_argument_in_users_notation() {
    let m = error_message(r"H(q_1,q_1)=1", InputFormat::Latex);
    assert_eq!(m, "q_{1} is listed twice in H(…)");
    let m = error_message("H(theta, theta) = 1", InputFormat::Text);
    assert_eq!(m, "theta is listed twice in H(…)");
}

#[test]
fn parameters_in_order_of_first_appearance() {
    let sys = latex(r"m\ddot{x}+kx=mg");
    assert_eq!(names(&sys).1, ["m", "k", "g"]);
    let sys = text("c*x'' = -k*x - c*b*x'");
    assert_eq!(names(&sys).1, ["c", "k", "b"]);
    // Across lines too.
    let sys = latex(r"\dot{x}=ay\\b\ddot{y}=-a y");
    assert_eq!(names(&sys).1, ["a", "b"]);
}
