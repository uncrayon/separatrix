mod common;

use common::*;
use dyn_expr::{compile, InputFormat, Severity};

fn error_message(src: &str, f: InputFormat) -> String {
    fails(src, f)
        .into_iter()
        .find(|d| d.severity == Severity::Error)
        .unwrap()
        .message
}

#[test]
fn jacobian_is_finite_where_a_root_argument_is_zero() {
    let sys = latex(r"\dot{x}=x\sqrt{x^2+y^2}\\\dot{y}=-y\\\dot{z}=\sqrt{x^2+y^2}-z");
    let j = jac(&sys, 0.0, &[0.0, 0.0, 0.0], &[]);
    assert!(j.iter().all(|v| v.is_finite()), "{j:?}");
    assert_eq!(j, [0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0]);
    let sys = latex(r"H(q,p)=\frac{p^2}{2}+\cos q+\sqrt{x^2}\\x=0");
    let j = jac(&sys, 0.0, &[0.3, 0.0], &[0.0]);
    assert_close(&j, &[0.0, 1.0, 0.3f64.cos(), 0.0], 1e-15);
    let sys = latex(
        r"\dot{x}=x\left(1-\sqrt{x^2+y^2}\right)-y\\\dot{y}=y\left(1-\sqrt{x^2+y^2}\right)+x",
    );
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
    let sys = latex(r"\ddot{x}_1=-\sin x_1\dot{x}_1");
    assert_close(
        &sys.eval_alloc(0.0, &x, &[]),
        &[0.75, -0.5f64.sin() * 0.75],
        1e-15,
    );
    let sys = latex(r"\dot{x}=\sin x y\\\dot{y}=0");
    assert_close(&sys.eval_alloc(0.0, &x, &[]), &[0.375f64.sin(), 0.0], 1e-15);
}

#[test]
fn equations_with_a_bare_symbol_on_the_left() {
    let sys = text("x = -x''");
    assert_eq!(names(&sys).0, ["x", "x'"]);
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 0.0], &[]), [0.0, -2.0]);
    let sys = latex(r"a=\dot{x}");
    assert_eq!(names(&sys), (vec!["x".into()], vec!["a".into()]));
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
        (
            r"\frac{\mathrm{d}^2x}{\mathrm{d}t^2}=-c\dot{x}-kx",
            InputFormat::Latex,
        ),
        ("x'' + c*(dx/dt) + k*x = 0", InputFormat::Text),
    ] {
        let sys = ok(src, f);
        assert_eq!(
            names(&sys),
            (vec!["x".into(), "x'".into()], vec!["c".into(), "k".into()]),
            "{src}"
        );
        assert_close(&sys.eval_alloc(0.0, &x, &p), &expect, 1e-15);
    }
}

#[test]
fn a_greek_delta_over_dt_is_a_quotient_not_a_derivative() {
    for (src, f, params, value) in [
        (
            r"\dot{x}=\frac{\delta}{dt}",
            InputFormat::Latex,
            ["delta", "d"],
            2.0 / 15.0,
        ),
        (
            "x' = delta/(d*t)",
            InputFormat::Text,
            ["delta", "d"],
            2.0 / 15.0,
        ),
        ("x' = delta_1/dt", InputFormat::Text, ["delta_1", "dt"], 0.4),
    ] {
        let sys = ok(src, f);
        assert_eq!(
            names(&sys),
            (vec!["x".to_string()], params.map(String::from).to_vec()),
            "{src}"
        );
        assert_close(&sys.eval_alloc(3.0, &[1.0], &[2.0, 5.0]), &[value], 1e-15);
    }
    let sys = text("dx_1/dt = -x_1");
    assert_eq!(names(&sys).0, vec!["x_1".to_string()]);
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
    for src in [
        r"\dot{x}=\frac{x}{1-1}",
        r"\dot{x}=\frac{x}{\sin0}",
        r"\dot{x}=x/(2\pi-2\pi)",
    ] {
        let out = compile(src, InputFormat::Latex);
        let d = out.diagnostics.iter().find(|d| d.is_error()).expect(src);
        assert_eq!(d.message, "Division by zero", "{src}");
    }
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
    assert!(compile("x' = 1.5e-3.2*x", InputFormat::Text)
        .system
        .is_none());
    assert_eq!(text("x' = 2.*x").eval_alloc(0.0, &[1.5], &[]), [3.0]);
}

#[test]
fn maps_of_subscripted_states() {
    let sys = latex(r"x_{1,n+1}=1-ax_{1,n}^2+x_{2,n}\\x_{2,n+1}=bx_{1,n}");
    assert_eq!(
        names(&sys),
        (
            vec!["x_1".into(), "x_2".into()],
            vec!["a".into(), "b".into()]
        )
    );
    assert_eq!(sys.info.states[0].latex, "x_{1}");
    assert_eq!(sys.info.equations_latex[1], "x_{2,n+1} = b x_{1,n}");
    assert_close(
        &sys.eval_alloc(0.0, &[0.5, 0.2], &[1.4, 0.3]),
        &[1.0 - 0.35 + 0.2, 0.15],
        1e-15,
    );
    check_derivatives(&sys, 0.0, &[0.5, 0.2], &[1.4, 0.3], 1e-6);
    let sys = latex(r"x_{1,n+1}=x_2\\x_{2,n+1}=-x_1");
    assert_eq!(sys.eval_alloc(0.0, &[1.0, 2.0], &[]), [2.0, -1.0]);
    let sys = text("x_1_{n+1} = x_2_n\nx_2_{n+1} = -x_1_n");
    assert_eq!(sys.eval_alloc(0.0, &[1.0, 2.0], &[]), [2.0, -1.0]);
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
fn a_backslash_delimiter_before_a_non_ascii_character_is_an_error_not_a_panic() {
    for src in [r"\dot{x}=\left\θ x\right)", r"\dot{x}=\left(x\right\·"] {
        let m = error_message(src, InputFormat::Latex);
        assert!(m.contains("isn't supported"), "{src}: {m}");
        let out = dyn_expr::convert(&[src.to_string()], InputFormat::Latex, InputFormat::Text);
        assert!(out[0].error && !out[0].converted, "{src}: {out:?}");
    }
}

#[test]
fn x_of_t_in_a_map_is_not_said_to_have_no_equation() {
    let out = compile(r"x_{n+1}=x(t)", InputFormat::Latex);
    assert!(out.system.is_some());
    let w = &out.diagnostics[0].message;
    assert!(
        w.contains("x(t) is read as x·t") && !w.contains("has no equation"),
        "{w}"
    );
    let out = compile(r"\dot{y}=-y+x(t)", InputFormat::Latex);
    assert!(out.diagnostics[0]
        .message
        .contains("since x has no equation"));
}

#[test]
fn parameters_in_order_of_first_appearance() {
    let sys = latex(r"m\ddot{x}+kx=mg");
    assert_eq!(names(&sys).1, ["m", "k", "g"]);
    let sys = text("c*x'' = -k*x - c*b*x'");
    assert_eq!(names(&sys).1, ["c", "k", "b"]);
    let sys = latex(r"\dot{x}=ay\\b\ddot{y}=-a y");
    assert_eq!(names(&sys).1, ["a", "b"]);
}

#[test]
fn the_current_iterate_followed_by_t_reads_the_state_in_a_map() {
    let out = compile(r"x_{n+1}=x_n\left(t\right)", InputFormat::Latex);
    let sys = out.system.expect("compiles");
    assert_eq!(names(&sys), (vec!["x".to_string()], vec!["t".to_string()]));
    assert_close(&sys.eval_alloc(0.0, &[0.5], &[3.0]), &[1.5], 1e-15);
    let w = &out.diagnostics[0].message;
    assert!(w.contains("t is not the time in a map"), "{w}");
}

#[test]
fn a_number_too_large_for_double_precision_is_an_error() {
    for (src, f) in [
        ("x' = 2e400*x", InputFormat::Text),
        ("x' = -x/1e999", InputFormat::Text),
    ] {
        assert!(error_message(src, f).contains("too large"), "{src}");
    }
    let huge = format!(r"\dot{{x}}={}x", "9".repeat(400));
    assert!(error_message(&huge, InputFormat::Latex).contains("too large"));
}

#[test]
fn unbraced_one_digit_scripts_mean_what_mathlive_draws() {
    let sys = latex(r"\dot{x}=-x^20.5");
    assert_close(&sys.eval_alloc(0.0, &[3.0], &[]), &[-4.5], 1e-15);
    let sys = latex(r"\dot{x}_1=-x_12");
    assert_eq!(names(&sys).0, ["x_1"]);
    assert_close(&sys.eval_alloc(0.0, &[3.0], &[]), &[-6.0], 1e-15);
    let sys = latex(r"\dot{x}_{12}=-x_{12}");
    assert_eq!(names(&sys).0, ["x_12"]);
}

#[test]
fn a_greek_letter_inside_a_subscript_keeps_its_name_in_the_display() {
    let sys = latex(r"\dot{x}=-k_{a\beta c}x");
    assert_eq!(sys.info.params[0].name, "k_abetac");
    assert_eq!(sys.info.params[0].latex, r"k_{a\beta c}");
    assert_eq!(sys.info.equations_latex[0], r"\dot{x} = -k_{a\beta c} x");
}

#[test]
fn a_map_iterate_with_no_equation_is_pointed_out() {
    for (src, f) in [
        (r"x_{n+1}=y_n+1-ax_n^2", InputFormat::Latex),
        ("x_{n+1} = y_n + 1 - a*x_n^2", InputFormat::Text),
    ] {
        let out = compile(src, f);
        assert!(out.system.is_some(), "{src}");
        let warned: Vec<_> = out
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .map(|d| d.message.as_str())
            .collect();
        assert_eq!(warned.len(), 1, "{src}: {warned:?}");
        assert!(warned[0].contains("has no equation"), "{src}: {warned:?}");
    }
    let out = compile(r"\dot{x}=-\omega_n^2y\\\dot{y}=x", InputFormat::Latex);
    assert!(
        out.diagnostics.is_empty(),
        "a flow's ω_n is a name: {:?}",
        out.diagnostics
    );
    let out = compile(r"x_{n+1}=y_n\\y_{n+1}=-x_n", InputFormat::Latex);
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn a_maps_current_iterate_is_a_state_and_takes_no_value() {
    for (src, f) in [
        (
            "x_{n+1}=rx_n\\left(1-x_n\\right)\nx_n=0.5",
            InputFormat::Latex,
        ),
        ("x_{n+1} = r*x_n*(1 - x_n)\nx_n = 0.5", InputFormat::Text),
    ] {
        assert!(
            error_message(src, f).contains("is a state variable"),
            "{src}"
        );
    }
    let sys = latex("\\dot{x}=-ax\nx_n=2");
    assert_eq!(
        names(&sys).1,
        ["a"],
        "a flow's x_n is a parameter of its own"
    );
}

#[test]
fn a_text_word_that_starts_with_a_function_suggests_a_call_text_mode_can_read() {
    for (src, want) in [
        ("x' = sin_x", "did you mean sin(x)?"),
        ("x' = -cosh_y*x", "did you mean cosh(y)?"),
        ("x' = cost", "did you mean cos(t)?"),
    ] {
        let out = compile(src, InputFormat::Text);
        assert!(out.system.is_some(), "{src}: {:?}", out.diagnostics);
        let warned: Vec<_> = out.diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert!(warned.iter().any(|m| m.contains(want)), "{src}: {warned:?}");
        assert!(
            !warned.iter().any(|m| m.contains("(_")),
            "{src}: {warned:?}"
        );
    }
    let error = error_message("x' = sin_x(t)", InputFormat::Text);
    assert!(error.contains("did you mean sin(x)?"), "{error}");
}
