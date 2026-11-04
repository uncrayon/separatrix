mod common;

use common::*;
use dyn_core::{fd_jacobian, SystemKind, VectorField};
use dyn_expr::{compile, InputFormat, Severity};

fn eval1(src: &str, t: f64, x: &[f64], p: &[f64]) -> Vec<f64> {
    latex(src).eval_alloc(t, x, p)
}

fn first_error_message(src: &str) -> String {
    let out = compile(src, InputFormat::Latex);
    out.diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .unwrap_or_else(|| panic!("expected an error for {src:?}"))
}

#[test]
fn precedence_of_minus_powers_and_implicit_products() {
    assert_eq!(eval1(r"\dot{x}=-x^2", 0.0, &[3.0], &[]), [-9.0]);
    assert_eq!(eval1(r"\dot{x}=2x^2", 0.0, &[3.0], &[]), [18.0]);
    assert_eq!(
        eval1(r"\dot{x}=-\left(x\right)^{2}", 0.0, &[3.0], &[]),
        [-9.0]
    );
    assert_close(
        &eval1(r"\dot{x}=\sin^2x", 0.0, &[0.7], &[]),
        &[0.7f64.sin().powi(2)],
        1e-15,
    );
    assert_close(
        &eval1(r"\dot{x}=\sin x^{2}", 0.0, &[0.7], &[]),
        &[0.49f64.sin()],
        1e-15,
    );
    let sys = latex(r"\dot{x}=\sin x y\\\dot{y}=0");
    assert_close(
        &sys.eval_alloc(0.0, &[0.5, 0.75], &[]),
        &[0.375f64.sin(), 0.0],
        1e-15,
    );
    assert_close(
        &eval1(r"\dot{x}=\cos\omega t", 0.3, &[0.0], &[2.0]),
        &[0.6f64.cos()],
        1e-15,
    );
    let sys = latex(r"\dot{x}=\sin\theta\cos\phi\\\dot{\theta}=1\\\dot{\phi}=2");
    let f = sys.eval_alloc(0.0, &[0.0, 0.4, 0.9], &[]);
    assert_close(&f[..1], &[0.4f64.sin() * 0.9f64.cos()], 1e-15);
}

#[test]
fn constants_e_exp_pi_and_abs() {
    let v = (-0.7f64).exp();
    assert_close(&eval1(r"\dot{x}=e^{-x}", 0.0, &[0.7], &[]), &[v], 1e-15);
    assert_close(
        &eval1(r"\dot{x}=\exponentialE^{-x}", 0.0, &[0.7], &[]),
        &[v],
        1e-15,
    );
    assert_close(
        &eval1(r"\dot{x}=\exp\left(-x\right)", 0.0, &[0.7], &[]),
        &[v],
        1e-15,
    );
    assert_close(
        &eval1(r"\dot{x}=\mathrm{e}^{-x}", 0.0, &[0.7], &[]),
        &[v],
        1e-15,
    );
    assert_close(
        &eval1(r"\dot{x}=2\pi x", 0.0, &[1.0], &[]),
        &[std::f64::consts::TAU],
        1e-15,
    );
    assert_eq!(eval1(r"\dot{x}=\left|x\right|", 0.0, &[-2.0], &[]), [2.0]);
    assert_eq!(
        eval1(r"\dot{x}=\left\vert x\right\vert", 0.0, &[-2.0], &[]),
        [2.0]
    );
    assert_eq!(
        eval1(
            r"\dot{x}=|x|y|z|\\\dot{y}=0\\\dot{z}=0",
            0.0,
            &[-2.0, 3.0, -4.0],
            &[]
        ),
        [24.0, 0.0, 0.0]
    );
}

#[test]
fn nested_fractions() {
    let f = eval1(r"\dot{x}=\frac{1}{1+\frac{1}{x}}", 0.0, &[1.0], &[]);
    assert_eq!(f, [0.5]);
    let f = eval1(
        r"\dot{x}=\frac{\frac{1}{x}}{\frac{1}{2}+\frac{x}{1+\frac{1}{x}}}",
        0.0,
        &[0.5],
        &[],
    );
    assert_close(&f, &[3.0], 1e-15);
}

#[test]
fn realistic_mathlive_and_student_systems() {
    let sys = latex(r"\dot{x}=-x+\sin\left(t\right)");
    assert!(!sys.info.autonomous);
    assert_close(
        &sys.eval_alloc(1.0, &[0.5], &[]),
        &[-0.5 + 1f64.sin()],
        1e-15,
    );

    let sys = latex(r"x^{\prime}=y\\y^{\prime}=-x");
    assert_eq!(names(&sys).0, ["x", "y"]);

    let sys = latex(r"\frac{\differentialD x}{\differentialD t}=x");
    assert_eq!(sys.eval_alloc(0.0, &[2.0], &[]), [2.0]);

    let sys = latex(r"\theta^{\prime\prime}=-\frac{g}{L}\sin\theta");
    assert_eq!(
        names(&sys),
        (
            vec!["theta".into(), "theta'".into()],
            vec!["g".into(), "L".into()]
        )
    );
    assert_close(
        &sys.eval_alloc(0.0, &[0.5, 0.75], &[9.8, 2.0]),
        &[0.75, -4.9 * 0.5f64.sin()],
        1e-15,
    );

    let sys = latex(r"x_{n+1}=rx_n(1-x_n)");
    assert_eq!(sys.kind(), SystemKind::Map);
    assert_close(
        &sys.eval_alloc(0.0, &[0.2], &[3.7]),
        &[3.7 * 0.2 * 0.8],
        1e-15,
    );

    let sys = latex(r"H(q,p)=\frac{p^2}{2m}+\frac{1}{2}kq^2");
    assert!(sys.info.hamiltonian);
    assert_eq!(names(&sys).1, ["m", "k"]);
    assert_close(
        &sys.eval_alloc(0.0, &[0.5, 0.75], &[2.0, 3.0]),
        &[0.375, -1.5],
        1e-15,
    );

    let sys = latex(r"\dot{x}=\mu x-x^3");
    assert_close(&sys.eval_alloc(0.0, &[0.5], &[2.0]), &[0.875], 1e-15);
    let sys = latex(r"\dot{x}=xy\\\dot{y}=-y");
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 3.0], &[]), [6.0, -3.0]);

    let sys = latex(r"mr\ddot{\phi}=-b\dot{\phi}-mg\sin\phi+mr\omega^2\sin\phi\cos\phi");
    let (phi, w) = (0.4f64, 0.3);
    let (m, r, b, g, om) = (1.5, 0.7, 0.2, 9.8, 2.0);
    let expect = (-b * w - m * g * phi.sin() + m * r * om * om * phi.sin() * phi.cos()) / (m * r);
    let p: Vec<f64> = names(&sys)
        .1
        .iter()
        .map(|n| match n.as_str() {
            "m" => m,
            "r" => r,
            "b" => b,
            "g" => g,
            "omega" => om,
            other => panic!("unexpected parameter {other}"),
        })
        .collect();
    assert_close(&sys.eval_alloc(0.0, &[phi, w], &p), &[w, expect], 1e-14);
}

#[test]
fn degenerate_inputs_give_located_errors() {
    for src in [
        "",
        "=",
        "=x",
        "x",
        r"\dot{x}",
        r"\dot{x}=",
        r"\dot{}=x",
        "x'=x'=x",
        r"\left(",
        "^",
        "_",
        "\\",
        "''",
    ] {
        let out = compile(src, InputFormat::Latex);
        assert!(out.system.is_none(), "{src:?}");
        for d in &out.diagnostics {
            assert!(!d.message.is_empty());
            if let Some((a, b)) = d.span {
                assert!(a <= b && b <= src.len(), "{src:?}: {d:?}");
            }
        }
    }
}

#[test]
fn exact_jacobians_match_central_differences() {
    let cases: &[(&str, &[f64], f64)] = &[
        (r"H(q,p)=\frac{p^2}{2m}+\frac{1}{2}kq^2", &[0.3, -0.7], 0.0),
        (
            r"H(q_1,q_2,p_1,p_2)=\frac{p_1^2+p_2^2}{2}+\frac{1}{2}\left(q_1^2+q_2^2\right)+q_1^2q_2-\frac{q_2^3}{3}",
            &[0.3, -0.2, 0.1, 0.4],
            0.0,
        ),
        (
            r"H(\theta,p)=\frac{p^2}{2mL^2}-mgL\cos\theta",
            &[0.9, 0.2],
            0.0,
        ),
        (
            r"H(q,p)=\frac{p^2}{2}+\frac{q^2}{2}\cos t",
            &[0.9, 0.2],
            0.4,
        ),
        (r"H(q,p)=\frac{p^2}{2}+aq^{n}", &[0.9, 0.2], 0.0),
        (
            r"\theta^{\prime\prime}=-\frac{g}{L}\sin\theta",
            &[0.4, 0.3],
            0.0,
        ),
        (
            r"\ddot{x}+2\zeta\omega\dot{x}+\omega^2x=F\cos\left(\Omega t\right)",
            &[0.4, 0.3],
            0.5,
        ),
        (
            r"mr\ddot{\phi}=-b\dot{\phi}-mg\sin\phi+mr\omega^2\sin\phi\cos\phi",
            &[0.4, 0.3],
            0.0,
        ),
        (
            r"\ddot{x}+\mu\left(x^2-1\right)\dot{x}+x=0",
            &[0.4, 0.3],
            0.0,
        ),
        (r"\dddot{x}=-a\ddot{x}-\dot{x}^2+x", &[0.4, 0.3, -0.2], 0.0),
        (r"x_{n+1}=rx_n(1-x_n)", &[0.3], 0.0),
        (r"x_{n+1}=1-ax_n^2+y_n\\y_{n+1}=bx_n", &[0.3, 0.1], 0.0),
        (r"\dot{x}=\frac{x^n}{1+x^n}", &[0.3], 0.0),
        (r"\dot{x}=e^{-x^2}\sin\left(\omega t\right)", &[0.3], 0.7),
    ];
    for &(src, x, t) in cases {
        let sys = latex(src);
        let p: Vec<f64> = (0..sys.n_params()).map(|k| 1.3 + 0.2 * k as f64).collect();
        check_derivatives(&sys, t, x, &p, 1e-6);
    }
}

#[test]
fn random_snippets_never_panic_and_spans_are_valid() {
    const FRAG: &[&str] = &[
        "x",
        "y",
        "t",
        "n",
        "H",
        "q",
        "p",
        "d",
        "e",
        "2",
        "1",
        "0",
        ".",
        "10",
        "=",
        "=",
        "+",
        "-",
        "^",
        "_",
        "'",
        "(",
        ")",
        "{",
        "}",
        "[",
        "]",
        "|",
        ",",
        ";",
        "\n",
        "\\\\",
        "\\dot",
        "\\ddot",
        "\\frac",
        "\\sqrt",
        "\\sin",
        "\\exp",
        "\\ln",
        "\\left(",
        "\\right)",
        "\\left|",
        "\\right|",
        "\\prime",
        "\\pi",
        "\\theta",
        "\\mathrm{d}",
        "\\cdot",
        "\\max",
        "x_{n+1}",
        "x_n",
        "\\placeholder{}",
        " ",
        "θ",
        "\\operatorname{",
        "\\begin{cases}",
        "\\end{cases}",
        "&",
        "\\sqrt[",
        "\\left.",
        "{,}",
        "\\differentialD",
        "\\",
        "\\sin^{-1}",
        "^{\\prime}",
        "*",
        "/",
    ];
    let mut s = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move || {
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        s.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    for _ in 0..20_000 {
        let len = 1 + next() % 14;
        let src: String = (0..len)
            .map(|_| FRAG[(next() % FRAG.len() as u64) as usize])
            .collect();
        for fmt in [InputFormat::Latex, InputFormat::Text] {
            let out = compile(&src, fmt);
            for d in &out.diagnostics {
                if let Some((a, b)) = d.span {
                    assert!(
                        a <= b
                            && b <= src.len()
                            && src.is_char_boundary(a)
                            && src.is_char_boundary(b),
                        "{src:?}: {d:?}"
                    );
                }
            }
            if let Some(sys) = out.system {
                let x = vec![0.3; sys.dim()];
                let mut j = vec![0.0; sys.dim() * sys.dim()];
                sys.jacobian(0.1, &x, &sys.default_params(), &mut j);
            }
        }
    }
}

#[test]
fn jacobian_at_origin_of_cartesian_limit_cycle_is_finite() {
    let sys = latex(
        r"\dot{x}=x\left(1-\sqrt{x^2+y^2}\right)-y\\\dot{y}=y\left(1-\sqrt{x^2+y^2}\right)+x",
    );
    let j = jac(&sys, 0.0, &[0.0, 0.0], &[]);
    assert_close(&j, &[1.0, -1.0, 1.0, 1.0], 1e-9);
}

#[test]
fn unrelated_jacobian_entry_is_zero_at_sqrt_of_zero() {
    for src in [
        r"\dot{x}=\sqrt{x}\\\dot{y}=y",
        r"\dot{x}=\sqrt[3]{x}\\\dot{y}=y",
        r"\dot{x}=x^{\frac{1}{3}}\\\dot{y}=y",
        r"\dot{x}=x^{y}\\\dot{y}=y",
    ] {
        let sys = latex(src);
        let x = [0.0, 2.0];
        let j = jac(&sys, 0.0, &x, &[]);
        let mut fd = [0.0; 4];
        fd_jacobian(&sys, 0.0, &x, &[], &mut fd);
        assert_eq!(fd[1], 0.0, "{src}");
        assert_eq!(j[1], 0.0, "{src}: AD gives {:?}", j);
    }
    let sys = latex(r"\dot{x}=\arcsin x\\\dot{y}=y");
    let j = jac(&sys, 0.0, &[1.0, 0.5], &[]);
    assert_eq!(j[1], 0.0, "arcsin at 1: {j:?}");
}

#[test]
fn hill_exponent_parameter_derivative_at_zero_is_zero() {
    let sys = latex(r"\dot{x}=\frac{x^n}{1+x^n}");
    let mut d = [0.0];
    sys.param_jacobian(0.0, &[0.0], &[2.0], 0, &mut d);
    assert_eq!(d, [0.0]);
}

#[test]
fn bare_symbol_left_side_with_second_derivative_is_solved() {
    let sys = latex(r"x=-\ddot{x}");
    assert_eq!(names(&sys).0, ["x", "x'"]);
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 0.0], &[]), [0.0, -2.0]);
}

#[test]
fn leibniz_second_derivative_in_implicit_equation_is_solved() {
    let sys = latex(r"\frac{d^2x}{dt^2}+\omega^2x=0");
    assert_eq!(names(&sys).0, ["x", "x'"]);
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 0.0], &[3.0]), [0.0, -18.0]);
}

#[test]
fn function_argument_stops_before_dotted_derivative() {
    let sys = latex(r"\ddot{\theta}=-\sin\theta\dot{\theta}");
    let (th, w) = (0.5f64, 0.75);
    assert_close(
        &sys.eval_alloc(0.0, &[th, w], &[]),
        &[w, -th.sin() * w],
        1e-15,
    );
}

#[test]
fn parameter_order_is_first_appearance_in_rearranged_line() {
    let sys = latex(r"mr\ddot{\phi}=-b\dot{\phi}-mg\sin\phi+mr\omega^2\sin\phi\cos\phi");
    assert_eq!(names(&sys).1, ["m", "r", "b", "g", "omega"]);
}

#[test]
fn empty_braces_are_not_reported_as_unmatched() {
    for src in [
        r"\dot{x}=\frac{}{}",
        r"\dot{x}=\frac{1}{}",
        r"\dot{x}=x^{}",
        r"\dot{x}=\sqrt{}",
    ] {
        let m = first_error_message(src);
        assert!(!m.contains("no matching"), "{src}: {m}");
    }
}

#[test]
fn dy_dx_message_points_to_time_derivatives() {
    let m = first_error_message(r"\frac{dy}{dx}=x-y");
    assert!(m.contains("time") || m.contains("dt"), "{m}");
}

#[test]
fn division_by_constant_zero_expression_is_rejected() {
    let out = compile(r"\dot{x}=\frac{x}{1-1}", InputFormat::Latex);
    assert!(
        out.system.is_none(),
        "evaluates to {:?}",
        out.system.map(|s| s.eval_alloc(0.0, &[1.0], &[]))
    );
}

#[test]
fn number_with_two_decimal_points_is_rejected() {
    let out = compile(r"\dot{x}=1.2.3x", InputFormat::Latex);
    assert!(
        out.system.is_none(),
        "{:?}",
        out.system.map(|s| s.info.equations_latex)
    );
}

#[test]
fn map_with_subscripted_states_compiles() {
    let sys = latex(r"x_{1,n+1}=x_{2,n}\\x_{2,n+1}=-x_{1,n}");
    assert_eq!(names(&sys).0, ["x_1", "x_2"]);
    assert_eq!(sys.eval_alloc(0.0, &[1.0, 2.0], &[]), [2.0, -1.0]);
}

#[test]
fn hamiltonian_duplicate_argument_message_uses_display_name() {
    let m = first_error_message(r"H(\theta,\theta)=1");
    assert!(m.contains("\\theta"), "{m}");
}

/// A stack overflow cannot be caught, so the check runs in a child process.
#[test]
fn deep_or_long_input_does_not_overflow_the_stack() {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "deep_input_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "compile crashed the process ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .last()
            .unwrap_or("")
    );
}

#[test]
#[ignore = "child process of deep_or_long_input_does_not_overflow_the_stack"]
fn deep_input_child() {
    let nested = format!(
        r"\dot{{x}}={}x{}",
        r"\left(".repeat(3000),
        r"\right)".repeat(3000)
    );
    let long = format!(r"\dot{{x}}=x{}", "+x".repeat(20_000));
    let h = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(move || {
            let _ = compile(&nested, InputFormat::Latex);
            let _ = compile(&long, InputFormat::Latex);
        })
        .unwrap();
    h.join().unwrap();
}
