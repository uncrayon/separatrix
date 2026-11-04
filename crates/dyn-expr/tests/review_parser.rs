//! Adversarial review of the parser, the semantic pass and the AD
//! evaluator, using realistic MathLive output and student input.
//!
//! The first group of tests documents behaviour that is correct today and
//! must stay so (precedence, MathLive forms, exact Jacobians). The second
//! group, under "Known bugs", FAILS on purpose: each test shows one real
//! problem found in the review and passes once it is fixed.

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

// ===== Behaviour that is correct and must stay so =========================

#[test]
fn precedence_of_minus_powers_and_implicit_products() {
    // -x^2 is -(x^2), 2x^2 is 2·(x^2), not (2x)^2.
    assert_eq!(eval1(r"\dot{x}=-x^2", 0.0, &[3.0], &[]), [-9.0]);
    assert_eq!(eval1(r"\dot{x}=2x^2", 0.0, &[3.0], &[]), [18.0]);
    assert_eq!(eval1(r"\dot{x}=-\left(x\right)^{2}", 0.0, &[3.0], &[]), [-9.0]);
    // \sin^2 x is (sin x)^2; \sin x^2 is sin(x^2).
    assert_close(&eval1(r"\dot{x}=\sin^2x", 0.0, &[0.7], &[]), &[0.7f64.sin().powi(2)], 1e-15);
    assert_close(&eval1(r"\dot{x}=\sin x^{2}", 0.0, &[0.7], &[]), &[0.49f64.sin()], 1e-15);
    // Documented rule: an unbracketed function takes the following product.
    let sys = latex(r"\dot{x}=\sin x y\\\dot{y}=0");
    assert_close(&sys.eval_alloc(0.0, &[0.5, 0.75], &[]), &[0.375f64.sin(), 0.0], 1e-15);
    assert_close(
        &eval1(r"\dot{x}=\cos\omega t", 0.3, &[0.0], &[2.0]),
        &[0.6f64.cos()],
        1e-15,
    );
    // A function stops the product: \sin x \cos y = sin(x)·cos(y).
    let sys = latex(r"\dot{x}=\sin\theta\cos\phi\\\dot{\theta}=1\\\dot{\phi}=2");
    let f = sys.eval_alloc(0.0, &[0.0, 0.4, 0.9], &[]);
    assert_close(&f[..1], &[0.4f64.sin() * 0.9f64.cos()], 1e-15);
}

#[test]
fn constants_e_exp_pi_and_abs() {
    let v = (-0.7f64).exp();
    assert_close(&eval1(r"\dot{x}=e^{-x}", 0.0, &[0.7], &[]), &[v], 1e-15);
    assert_close(&eval1(r"\dot{x}=\exponentialE^{-x}", 0.0, &[0.7], &[]), &[v], 1e-15);
    assert_close(&eval1(r"\dot{x}=\exp\left(-x\right)", 0.0, &[0.7], &[]), &[v], 1e-15);
    assert_close(&eval1(r"\dot{x}=\mathrm{e}^{-x}", 0.0, &[0.7], &[]), &[v], 1e-15);
    assert_close(&eval1(r"\dot{x}=2\pi x", 0.0, &[1.0], &[]), &[std::f64::consts::TAU], 1e-15);
    assert_eq!(eval1(r"\dot{x}=\left|x\right|", 0.0, &[-2.0], &[]), [2.0]);
    assert_eq!(eval1(r"\dot{x}=\left\vert x\right\vert", 0.0, &[-2.0], &[]), [2.0]);
    assert_eq!(eval1(r"\dot{x}=|x|y|z|\\\dot{y}=0\\\dot{z}=0", 0.0, &[-2.0, 3.0, -4.0], &[]), [
        24.0, 0.0, 0.0
    ]);
}

#[test]
fn nested_fractions() {
    let f = eval1(r"\dot{x}=\frac{1}{1+\frac{1}{x}}", 0.0, &[1.0], &[]);
    assert_eq!(f, [0.5]);
    let f = eval1(r"\dot{x}=\frac{\frac{1}{x}}{\frac{1}{2}+\frac{x}{1+\frac{1}{x}}}", 0.0, &[0.5], &[]);
    assert_close(&f, &[3.0], 1e-15);
}

#[test]
fn realistic_mathlive_and_student_systems() {
    // Non-autonomous 1D flow.
    let sys = latex(r"\dot{x}=-x+\sin\left(t\right)");
    assert!(!sys.info.autonomous);
    assert_close(&sys.eval_alloc(1.0, &[0.5], &[]), &[-0.5 + 1f64.sin()], 1e-15);

    // Prime as MathLive writes it.
    let sys = latex(r"x^{\prime}=y\\y^{\prime}=-x");
    assert_eq!(names(&sys).0, ["x", "y"]);

    // Leibniz with \differentialD.
    let sys = latex(r"\frac{\differentialD x}{\differentialD t}=x");
    assert_eq!(sys.eval_alloc(0.0, &[2.0], &[]), [2.0]);

    // Pendulum, order reduction.
    let sys = latex(r"\theta^{\prime\prime}=-\frac{g}{L}\sin\theta");
    assert_eq!(names(&sys), (vec!["theta".into(), "theta'".into()], vec!["g".into(), "L".into()]));
    assert_close(&sys.eval_alloc(0.0, &[0.5, 0.75], &[9.8, 2.0]), &[0.75, -4.9 * 0.5f64.sin()], 1e-15);

    // Logistic map.
    let sys = latex(r"x_{n+1}=rx_n(1-x_n)");
    assert_eq!(sys.kind(), SystemKind::Map);
    assert_close(&sys.eval_alloc(0.0, &[0.2], &[3.7]), &[3.7 * 0.2 * 0.8], 1e-15);

    // Hamiltonian harmonic oscillator.
    let sys = latex(r"H(q,p)=\frac{p^2}{2m}+\frac{1}{2}kq^2");
    assert!(sys.info.hamiltonian);
    assert_eq!(names(&sys).1, ["m", "k"]);
    assert_close(&sys.eval_alloc(0.0, &[0.5, 0.75], &[2.0, 3.0]), &[0.375, -1.5], 1e-15);

    // Pitchfork normal form, product of two states.
    let sys = latex(r"\dot{x}=\mu x-x^3");
    assert_close(&sys.eval_alloc(0.0, &[0.5], &[2.0]), &[0.875], 1e-15);
    let sys = latex(r"\dot{x}=xy\\\dot{y}=-y");
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 3.0], &[]), [6.0, -3.0]);

    // Bead on a rotating hoop (Strogatz 3.5), solved for \ddot{\phi}.
    let sys = latex(r"mr\ddot{\phi}=-b\dot{\phi}-mg\sin\phi+mr\omega^2\sin\phi\cos\phi");
    let (phi, w) = (0.4f64, 0.3);
    let (m, r, b, g, om) = (1.5, 0.7, 0.2, 9.8, 2.0);
    let expect = (-b * w - m * g * phi.sin() + m * r * om * om * phi.sin() * phi.cos()) / (m * r);
    // Parameter order is checked separately (see the bug test below).
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
    for src in ["", "=", "=x", "x", r"\dot{x}", r"\dot{x}=", r"\dot{}=x", "x'=x'=x", r"\left(", "^", "_", "\\", "''"] {
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

/// AD Jacobians (state and parameter) equal central differences for
/// Hamiltonians (second derivatives), order-reduced systems and maps.
#[test]
fn exact_jacobians_match_central_differences() {
    let cases: &[(&str, &[f64], f64)] = &[
        (r"H(q,p)=\frac{p^2}{2m}+\frac{1}{2}kq^2", &[0.3, -0.7], 0.0),
        (
            r"H(q_1,q_2,p_1,p_2)=\frac{p_1^2+p_2^2}{2}+\frac{1}{2}\left(q_1^2+q_2^2\right)+q_1^2q_2-\frac{q_2^3}{3}",
            &[0.3, -0.2, 0.1, 0.4],
            0.0,
        ),
        (r"H(\theta,p)=\frac{p^2}{2mL^2}-mgL\cos\theta", &[0.9, 0.2], 0.0),
        (r"H(q,p)=\frac{p^2}{2}+\frac{q^2}{2}\cos t", &[0.9, 0.2], 0.4),
        (r"H(q,p)=\frac{p^2}{2}+aq^{n}", &[0.9, 0.2], 0.0),
        (r"\theta^{\prime\prime}=-\frac{g}{L}\sin\theta", &[0.4, 0.3], 0.0),
        (r"\ddot{x}+2\zeta\omega\dot{x}+\omega^2x=F\cos\left(\Omega t\right)", &[0.4, 0.3], 0.5),
        (r"mr\ddot{\phi}=-b\dot{\phi}-mg\sin\phi+mr\omega^2\sin\phi\cos\phi", &[0.4, 0.3], 0.0),
        (r"\ddot{x}+\mu\left(x^2-1\right)\dot{x}+x=0", &[0.4, 0.3], 0.0),
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

/// Random snippets never panic, and every span is a valid range.
#[test]
fn random_snippets_never_panic_and_spans_are_valid() {
    const FRAG: &[&str] = &[
        "x", "y", "t", "n", "H", "q", "p", "d", "e", "2", "1", "0", ".", "10", "=", "=", "+", "-",
        "^", "_", "'", "(", ")", "{", "}", "[", "]", "|", ",", ";", "\n", "\\\\", "\\dot",
        "\\ddot", "\\frac", "\\sqrt", "\\sin", "\\exp", "\\ln", "\\left(", "\\right)", "\\left|",
        "\\right|", "\\prime", "\\pi", "\\theta", "\\mathrm{d}", "\\cdot", "\\max", "x_{n+1}",
        "x_n", "\\placeholder{}", " ", "θ", "\\operatorname{", "\\begin{cases}", "\\end{cases}",
        "&", "\\sqrt[", "\\left.", "{,}", "\\differentialD", "\\", "\\sin^{-1}", "^{\\prime}",
        "*", "/",
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
        let src: String = (0..len).map(|_| FRAG[(next() % FRAG.len() as u64) as usize]).collect();
        for fmt in [InputFormat::Latex, InputFormat::Text] {
            let out = compile(&src, fmt);
            for d in &out.diagnostics {
                if let Some((a, b)) = d.span {
                    assert!(
                        a <= b && b <= src.len() && src.is_char_boundary(a) && src.is_char_boundary(b),
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

// ===== Known bugs: these tests fail until the bug is fixed ================

/// Strogatz Example 7.1.1 written in Cartesian form: the origin is a fixed
/// point (an unstable spiral, J = [[1, -1], [1, 1]]). The field is
/// differentiable there, but the AD Jacobian is all NaN because
/// `d sqrt(u)` with `u = 0` gives `0 / 0` and `x · NaN` spreads it.
#[test]
fn bug_jacobian_at_origin_of_cartesian_limit_cycle_is_nan() {
    let sys = latex(
        r"\dot{x}=x\left(1-\sqrt{x^2+y^2}\right)-y\\\dot{y}=y\left(1-\sqrt{x^2+y^2}\right)+x",
    );
    let j = jac(&sys, 0.0, &[0.0, 0.0], &[]);
    assert_close(&j, &[1.0, -1.0, 1.0, 1.0], 1e-9);
}

/// A partial derivative with respect to a variable the expression does not
/// depend on must be exactly 0, but sqrt / cbrt / fractional powers /
/// arcsin evaluated where their own derivative is infinite turn the
/// `0 · ∞` of the chain rule into NaN, poisoning the whole Jacobian.
#[test]
fn bug_unrelated_jacobian_entry_is_nan_at_sqrt_of_zero() {
    for src in [
        r"\dot{x}=\sqrt{x}\\\dot{y}=y",
        r"\dot{x}=\sqrt[3]{x}\\\dot{y}=y",
        r"\dot{x}=x^{\frac{1}{3}}\\\dot{y}=y",
        r"\dot{x}=x^{y}\\\dot{y}=y",
    ] {
        let sys = latex(src);
        let x = [0.0, 2.0];
        let j = jac(&sys, 0.0, &x, &[]);
        // Row 0 does not depend on y for the first three; for x^y at x = 0
        // the y-derivative x^y ln x tends to 0 as well (central differences
        // agree).
        let mut fd = [0.0; 4];
        fd_jacobian(&sys, 0.0, &x, &[], &mut fd);
        assert_eq!(fd[1], 0.0, "{src}");
        assert_eq!(j[1], 0.0, "{src}: AD gives {:?}", j);
    }
    let sys = latex(r"\dot{x}=\arcsin x\\\dot{y}=y");
    let j = jac(&sys, 0.0, &[1.0, 0.5], &[]);
    assert_eq!(j[1], 0.0, "arcsin at 1: {j:?}");
}

/// Hill function x^n / (1 + x^n): the derivative with respect to the
/// exponent n at x = 0 is 0 (x^n = 0 for all n > 0), but AD computes
/// `0 · ln 0 = NaN`.
#[test]
fn bug_hill_exponent_parameter_derivative_at_zero_is_nan() {
    let sys = latex(r"\dot{x}=\frac{x^n}{1+x^n}");
    let mut d = [0.0];
    sys.param_jacobian(0.0, &[0.0], &[2.0], 0, &mut d);
    assert_eq!(d, [0.0]);
}

/// `x = -\ddot{x}` is a perfectly clear harmonic oscillator, and
/// `\omega^2 x = -\ddot{x}` already works, but a bare symbol on the left
/// makes the line a "parameter value", so the user is told "No equations
/// found, only parameter values".
#[test]
fn bug_bare_symbol_left_side_with_second_derivative_is_rejected() {
    let sys = latex(r"x=-\ddot{x}");
    assert_eq!(names(&sys).0, ["x", "x'"]);
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 0.0], &[]), [0.0, -2.0]);
}

/// Leibniz notation works alone (`\frac{d^2x}{dt^2}=-x`) and implicit
/// equations work with dots (`\ddot{x}+x=0`), but not both together.
#[test]
fn bug_leibniz_second_derivative_in_implicit_equation() {
    let sys = latex(r"\frac{d^2x}{dt^2}+\omega^2x=0");
    assert_eq!(names(&sys).0, ["x", "x'"]);
    assert_eq!(sys.eval_alloc(0.0, &[2.0, 0.0], &[3.0]), [0.0, -18.0]);
}

/// The unbracketed-function rule swallows a dotted derivative:
/// `-\sin\theta\dot{\theta}` becomes -sin(θ·θ̇). Nobody writes a time
/// derivative inside a function argument without brackets; physics texts
/// write chain-rule results exactly like this (ẋ = L cos θ θ̇).
#[test]
fn bug_function_argument_swallows_dotted_derivative() {
    let sys = latex(r"\ddot{\theta}=-\sin\theta\dot{\theta}");
    let (th, w) = (0.5f64, 0.75);
    assert_close(&sys.eval_alloc(0.0, &[th, w], &[]), &[w, -th.sin() * w], 1e-15);
}

/// Parameters must be listed "in order of first appearance" (contract).
/// For a rearranged line the order follows where each name is first
/// *visited* after solving, so a coefficient on the left (m, r) that also
/// appears later on the right is placed after b.
#[test]
fn bug_parameter_order_is_not_first_appearance_in_rearranged_line() {
    let sys = latex(r"mr\ddot{\phi}=-b\dot{\phi}-mg\sin\phi+mr\omega^2\sin\phi\cos\phi");
    assert_eq!(names(&sys).1, ["m", "r", "b", "g", "omega"]);
}

/// Empty braces (a cleared \frac or exponent box) are reported as an
/// unmatched brace, which is false and confusing.
#[test]
fn bug_empty_braces_are_reported_as_unmatched() {
    for src in [r"\dot{x}=\frac{}{}", r"\dot{x}=\frac{1}{}", r"\dot{x}=x^{}", r"\dot{x}=\sqrt{}"] {
        let m = first_error_message(src);
        assert!(!m.contains("no matching"), "{src}: {m}");
    }
}

/// A slope-field user will type dy/dx; they get the generic "Can't tell
/// what this line defines" instead of being told that the independent
/// variable is t.
#[test]
fn bug_dy_dx_gets_a_generic_message() {
    let m = first_error_message(r"\frac{dy}{dx}=x-y");
    assert!(m.contains("time") || m.contains("dt"), "{m}");
}

/// A literal division by zero is an error, but a constant expression that
/// folds to zero silently compiles to a field that is infinite everywhere.
#[test]
fn bug_division_by_constant_zero_expression_compiles() {
    let out = compile(r"\dot{x}=\frac{x}{1-1}", InputFormat::Latex);
    assert!(out.system.is_none(), "evaluates to {:?}", out.system.map(|s| s.eval_alloc(0.0, &[1.0], &[])));
}

/// `1.2.3` is silently read as 1.2 · 0.3.
#[test]
fn bug_number_with_two_decimal_points_is_accepted() {
    let out = compile(r"\dot{x}=1.2.3x", InputFormat::Latex);
    assert!(out.system.is_none(), "{:?}", out.system.map(|s| s.info.equations_latex));
}

/// Maps of subscripted states cannot be entered, although the display form
/// for them (`x_{1,n}`) exists.
#[test]
fn bug_map_with_subscripted_states_cannot_be_entered() {
    let sys = latex(r"x_{1,n+1}=x_{2,n}\\x_{2,n+1}=-x_{1,n}");
    assert_eq!(names(&sys).0, ["x_1", "x_2"]);
    assert_eq!(sys.eval_alloc(0.0, &[1.0, 2.0], &[]), [2.0, -1.0]);
}

/// The duplicate-argument message shows the internal name ("theta"), not
/// the LaTeX the user typed, unlike every other LaTeX-mode message.
#[test]
fn bug_hamiltonian_duplicate_argument_message_uses_internal_name() {
    let m = first_error_message(r"H(\theta,\theta)=1");
    assert!(m.contains("\\theta"), "{m}");
}

/// Deeply nested or very long input aborts the whole process with a stack
/// overflow (recursive descent, recursive lowering and drop). 2000 nested
/// `\left(` or a 20 000-term sum is enough on an 8 MiB main thread in
/// release. A stack overflow cannot be caught, so the check runs in a
/// child process.
#[test]
fn bug_deep_or_long_input_overflows_the_stack() {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "deep_input_child", "--nocapture", "--test-threads=1"])
        .env("DYN_EXPR_DEEP_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "compile crashed the process ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
    );
}

/// Helper for [`bug_deep_or_long_input_overflows_the_stack`]: does nothing
/// unless run as the child process.
#[test]
fn deep_input_child() {
    if std::env::var_os("DYN_EXPR_DEEP_CHILD").is_none() {
        return;
    }
    let nested = format!(r"\dot{{x}}={}x{}", r"\left(".repeat(3000), r"\right)".repeat(3000));
    let long = format!(r"\dot{{x}}=x{}", "+x".repeat(20_000));
    let h = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(move || {
            // Either an error ("too deeply nested") or a system is fine;
            // crashing is not.
            let _ = compile(&nested, InputFormat::Latex);
            let _ = compile(&long, InputFormat::Latex);
        })
        .unwrap();
    h.join().unwrap();
}
