mod common;

use common::*;
use dyn_core::{SystemKind, VectorField};
use dyn_expr::{compile, InputFormat};

const LORENZ: &str = "\\dot{x}=\\sigma\\left(y-x\\right)\\\\\\dot{y}=x\\left(\\rho-z\\right)-y\\\\\\dot{z}=xy-\\beta z\n\\sigma=10\n\\rho=28\n\\beta=\\frac{8}{3}";

#[test]
fn lorenz_latex_values_and_jacobian() {
    let sys = latex(LORENZ);
    assert_eq!(sys.kind(), SystemKind::Ode);
    assert!(sys.is_autonomous());
    assert!(!sys.info.hamiltonian);
    let (s, p) = names(&sys);
    assert_eq!(s, ["x", "y", "z"]);
    assert_eq!(p, ["sigma", "rho", "beta"]);
    let defaults = sys.default_params();
    assert_close(&defaults, &[10.0, 28.0, 8.0 / 3.0], 0.0);
    assert_eq!(sys.info.params[0].latex, "\\sigma");

    let x = [1.0, 2.0, 3.0];
    let f = sys.eval_alloc(0.0, &x, &defaults);
    assert_close(&f, &[10.0, 1.0 * (28.0 - 3.0) - 2.0, 2.0 - 8.0], 1e-15);

    let exact = [
        -10.0,
        10.0,
        0.0,
        28.0 - 3.0,
        -1.0,
        -1.0,
        2.0,
        1.0,
        -8.0 / 3.0,
    ];
    assert_close(&jac(&sys, 0.0, &x, &defaults), &exact, 1e-15);
    check_derivatives(&sys, 0.0, &[-3.2, 4.1, 17.0], &defaults, 1e-7);

    let mut dp = [0.0; 3];
    sys.param_jacobian(0.0, &x, &defaults, 1, &mut dp);
    assert_eq!(dp, [0.0, 1.0, 0.0]);

    assert_eq!(
        sys.info.equations_latex,
        [
            "\\dot{x} = \\sigma \\left(y - x\\right)",
            "\\dot{y} = x \\left(\\rho - z\\right) - y",
            "\\dot{z} = x y - \\beta z",
        ]
    );
    assert!(sys.instruction_count() <= 14, "{}", sys.instruction_count());
}

#[test]
fn lorenz_in_cases_environment_and_text_mode_agree() {
    let cases = "\\begin{cases}\\dot{x}&=\\sigma\\left(y-x\\right)\\\\\\dot{y}&=x\\left(\\rho-z\\right)-y\\\\\\dot{z}&=xy-\\beta z\\end{cases}";
    let a = latex(cases);
    let b = text("x' = sigma*(y - x)\ny' = x*(rho - z) - y\nz' = x*y - beta*z");
    assert_eq!(names(&a), names(&b));
    let (x, p) = ([0.3, -1.2, 2.5], [10.0, 28.0, 2.5]);
    assert_eq!(a.eval_alloc(0.0, &x, &p), b.eval_alloc(0.0, &x, &p));
    assert_eq!(jac(&a, 0.0, &x, &p), jac(&b, 0.0, &x, &p));
    assert_eq!(a.default_params(), [1.0, 1.0, 1.0]);

    let aligned = "\\begin{aligned}\\dot{x}&=\\sigma\\mleft(y-x\\mright)\\\\\\dot{y}&=x\\left(\\rho-z\\right)-y\\\\\\dot{z}&=xy-\\beta z\\end{aligned}";
    let c = latex(aligned);
    assert_eq!(c.eval_alloc(0.0, &x, &p), b.eval_alloc(0.0, &x, &p));
}

#[test]
fn text_mode_single_equation() {
    let sys = text("x' = sigma*(y - x)");
    let (s, p) = names(&sys);
    assert_eq!(s, ["x"]);
    assert_eq!(p, ["sigma", "y"]);
    assert_eq!(sys.eval_alloc(0.0, &[1.0], &[2.0, 5.0]), [8.0]);
    assert_eq!(
        sys.info.equations_latex,
        ["\\dot{x} = \\sigma \\left(y - x\\right)"]
    );
}

#[test]
fn pendulum_second_order() {
    let sys = latex("\\ddot{\\theta}=-\\sin\\theta");
    assert_eq!(sys.dim(), 2);
    assert_eq!(sys.info.states[0].name, "theta");
    assert_eq!(sys.info.states[0].latex, "\\theta");
    assert_eq!(sys.info.states[1].name, "theta'");
    assert_eq!(sys.info.states[1].latex, "\\dot{\\theta}");
    assert_eq!(sys.n_params(), 0);
    let x = [0.7, -0.4];
    assert_close(
        &sys.eval_alloc(0.0, &x, &[]),
        &[-0.4, -(0.7f64.sin())],
        1e-15,
    );
    assert_close(
        &jac(&sys, 0.0, &x, &[]),
        &[0.0, 1.0, -(0.7f64.cos()), 0.0],
        1e-15,
    );
    assert_eq!(
        sys.info.equations_latex,
        [
            "\\dot{\\theta} = \\dot{\\theta}",
            "\\ddot{\\theta} = -\\sin\\theta"
        ]
    );
}

#[test]
fn damped_oscillator_implicit_form() {
    let forms = [
        ("x'' + c*x' + k*x = 0", InputFormat::Text),
        ("x'' = -c*x' - k*x", InputFormat::Text),
        ("\\ddot{x}+c\\dot{x}+kx=0", InputFormat::Latex),
        ("x^{\\prime\\prime}=-cx^{\\prime}-kx", InputFormat::Latex),
        (
            "\\frac{d^2x}{dt^2}+c\\frac{dx}{dt}\\cdot 1+kx=0",
            InputFormat::Latex,
        ),
    ];
    let x = [0.5, -2.0];
    let p = [0.3, 4.0];
    let expect = [-2.0, -0.3 * -2.0 - 4.0 * 0.5];
    for (src, f) in &forms {
        let sys = ok(src, *f);
        let (s, pn) = names(&sys);
        assert_eq!(s, ["x", "x'"], "{src}");
        assert_eq!(pn, ["c", "k"], "{src}");
        assert_close(&sys.eval_alloc(0.0, &x, &p), &expect, 1e-15);
        assert_close(&jac(&sys, 0.0, &x, &p), &[0.0, 1.0, -4.0, -0.3], 1e-15);
    }

    let sys = latex("m\\ddot{x}+\\gamma\\dot{x}+kx=F\\cos\\left(\\omega t\\right)");
    assert!(!sys.is_autonomous());
    let (_, pn) = names(&sys);
    assert_eq!(pn, ["m", "gamma", "k", "F", "omega"]);
    let p = [2.0, 0.5, 3.0, 1.0, 1.2];
    let f = sys.eval_alloc(1.0, &x, &p);
    let want = (1.0 * (1.2f64).cos() - 0.5 * -2.0 - 3.0 * 0.5) / 2.0;
    assert_close(&f, &[-2.0, want], 1e-14);
    check_derivatives(&sys, 1.0, &x, &p, 1e-7);
}

#[test]
fn third_order_equation() {
    let sys = text("x''' = -x'' - x' - x + t");
    assert_eq!(names(&sys).0, ["x", "x'", "x''"]);
    assert_eq!(sys.info.states[2].latex, "\\ddot{x}");
    assert_eq!(sys.eval_alloc(2.0, &[1.0, 2.0, 3.0], &[]), [2.0, 3.0, -4.0]);
    assert_eq!(
        sys.info.equations_latex[2],
        "\\dddot{x} = -\\ddot{x} - \\dot{x} - x + t"
    );
}

#[test]
fn logistic_map() {
    let sys = latex("x_{n+1}=rx_n\\left(1-x_n\\right)\nr=3.7");
    assert_eq!(sys.kind(), SystemKind::Map);
    assert!(sys.is_autonomous());
    assert_eq!(names(&sys), (vec!["x".to_string()], vec!["r".to_string()]));
    assert_eq!(sys.default_params(), [3.7]);
    assert_close(
        &sys.eval_alloc(0.0, &[0.2], &[3.7]),
        &[3.7 * 0.2 * 0.8],
        1e-15,
    );
    assert_close(&jac(&sys, 0.0, &[0.2], &[3.7]), &[3.7 * (1.0 - 0.4)], 1e-15);
    assert_eq!(
        sys.info.equations_latex,
        ["x_{n+1} = r x_{n} \\left(1 - x_{n}\\right)"]
    );

    let bare = text("x_{n+1} = r*x*(1 - x)");
    assert_eq!(
        bare.eval_alloc(0.0, &[0.2], &[3.7]),
        sys.eval_alloc(0.0, &[0.2], &[3.7])
    );
}

#[test]
fn henon_map() {
    let sys = latex("x_{n+1}=1-ax_n^2+y_n\\\\y_{n+1}=bx_n\\\\a=1.4\\\\b=0.3");
    assert_eq!(
        names(&sys),
        (vec!["x".into(), "y".into()], vec!["a".into(), "b".into()])
    );
    let p = sys.default_params();
    assert_eq!(p, [1.4, 0.3]);
    let x = [0.5, -0.2];
    assert_close(
        &sys.eval_alloc(0.0, &x, &p),
        &[1.0 - 1.4 * 0.25 - 0.2, 0.15],
        1e-15,
    );
    assert_close(
        &jac(&sys, 0.0, &x, &p),
        &[-2.0 * 1.4 * 0.5, 1.0, 0.3, 0.0],
        1e-15,
    );
    check_derivatives(&sys, 0.0, &x, &p, 1e-7);
}

#[test]
fn non_autonomous_map_uses_n() {
    let sys = text("x_{n+1} = x_n + n");
    assert!(!sys.is_autonomous());
    assert_eq!(sys.eval_alloc(3.0, &[1.0], &[]), [4.0]);
    let ode = text("x' = n*x");
    assert!(ode.is_autonomous(), "n is an ordinary parameter in a flow");
    assert_eq!(names(&ode).1, ["n"]);
}

#[test]
fn hamiltonian_pendulum() {
    let sys = latex("H(q,p)=\\frac{p^2}{2}-\\cos q");
    assert!(sys.info.hamiltonian);
    assert_eq!(names(&sys).0, ["q", "p"]);
    let x = [0.8, 1.3];
    assert_close(
        &sys.eval_alloc(0.0, &x, &[]),
        &[1.3, -(0.8f64.sin())],
        1e-15,
    );
    assert_close(
        &jac(&sys, 0.0, &x, &[]),
        &[0.0, 1.0, -(0.8f64.cos()), 0.0],
        1e-15,
    );
    assert_close(
        &[sys.hamiltonian_value(0.0, &x, &[]).unwrap()],
        &[0.845 - 0.8f64.cos()],
        1e-15,
    );
    assert_eq!(
        sys.info.equations_latex,
        [
            "\\dot{q} = \\frac{\\partial H}{\\partial p}",
            "\\dot{p} = -\\frac{\\partial H}{\\partial q}"
        ]
    );
}

#[test]
fn hamiltonian_two_degrees_of_freedom_with_parameters() {
    let sys = text(
        "H(q_1, q_2, p_1, p_2) = (p_1^2 + p_2^2)/2 + (q_1^2 + q_2^2)/2 + lambda*(q_1^2*q_2 - q_2^3/3)",
    );
    assert_eq!(
        names(&sys),
        (
            vec!["q_1".into(), "q_2".into(), "p_1".into(), "p_2".into()],
            vec!["lambda".into()],
        )
    );
    let (x, p) = ([0.1, -0.3, 0.25, 0.4], [1.0]);
    let (q1, q2) = (x[0], x[1]);
    let f = sys.eval_alloc(0.0, &x, &p);
    let want = [0.25, 0.4, -(q1 + 2.0 * q1 * q2), -(q2 + q1 * q1 - q2 * q2)];
    assert_close(&f, &want, 1e-15);
    check_derivatives(&sys, 0.0, &x, &[0.7], 1e-7);
    let j = jac(&sys, 0.0, &x, &p);
    assert_close(&j[0..4], &[0.0, 0.0, 1.0, 0.0], 0.0);
    assert_close(&j[8..12], &[-(1.0 + 2.0 * q2), -2.0 * q1, 0.0, 0.0], 1e-15);
}

#[test]
fn non_autonomous_1d() {
    let sys = latex("\\dot{x}=x^2-t");
    assert!(!sys.is_autonomous());
    assert_eq!(sys.dim(), 1);
    assert_eq!(sys.eval_alloc(3.0, &[2.0], &[]), [1.0]);
    assert_eq!(jac(&sys, 3.0, &[2.0], &[]), [4.0]);
}

#[test]
fn implicit_multiplication() {
    let sys = latex("\\dot{x}=2xy\\\\\\dot{y}=-x");
    assert_eq!(sys.eval_alloc(0.0, &[3.0, 5.0], &[]), [30.0, -3.0]);
    assert_eq!(jac(&sys, 0.0, &[3.0, 5.0], &[]), [10.0, 6.0, -1.0, 0.0]);
    assert_eq!(sys.info.equations_latex[0], "\\dot{x} = 2 x y");
}

#[test]
fn leibniz_notation() {
    for src in [
        "\\frac{\\mathrm{d}x}{\\mathrm{d}t}=-x",
        "\\frac{dx}{dt}=-x",
        "\\frac{\\differentialD x}{\\differentialD t}=-x",
        "\\frac{d}{dt}x=-x",
    ] {
        let sys = latex(src);
        assert_eq!(sys.eval_alloc(0.0, &[2.0], &[]), [-2.0], "{src}");
    }
    let sys = latex("\\frac{d^2\\theta}{dt^2}=-\\theta");
    assert_eq!(names(&sys).0, ["theta", "theta'"]);
    let sys = text("dx/dt = -x");
    assert_eq!(sys.eval_alloc(0.0, &[2.0], &[]), [-2.0]);
}

#[test]
fn leibniz_notation_for_a_variable_with_subscript_n() {
    for sys in [
        latex("\\frac{dx_n}{dt}=-x_n"),
        latex("\\frac{\\mathrm{d}x_{n}}{\\mathrm{d}t}=-x_n"),
        latex("\\frac{d}{dt}x_n=-x_n"),
        text("dx_n/dt = -x_n"),
    ] {
        assert_eq!(names(&sys).0, ["x_n"]);
        assert_eq!(sys.eval_alloc(0.0, &[2.0], &[]), [-2.0]);
    }
    let sys = latex("\\frac{d^2x_n}{dt^2}=-x_n");
    assert_eq!(names(&sys).0, ["x_n", "x_n'"]);
}

#[test]
fn odd_roots_are_real_and_display_as_roots() {
    let sys = latex("\\dot{x}=\\sqrt[5]{-16x}");
    assert_close(&sys.eval_alloc(0.0, &[2.0], &[]), &[-2.0], 1e-14);
    let want = -16.0 / 5.0 * 32f64.powf(-0.8);
    assert_close(&jac(&sys, 0.0, &[2.0], &[]), &[want], 1e-12);
    assert_eq!(sys.info.equations_latex[0], "\\dot{x} = \\sqrt[5]{-16 x}");
    let sys = latex("\\dot{x}=\\sqrt[7]{x}");
    assert_close(&sys.eval_alloc(0.0, &[-128.0], &[]), &[-2.0], 1e-14);
}

#[test]
fn odd_roots_have_an_infinite_slope_at_zero_like_the_cube_root() {
    for src in [
        r"\dot{x}=\sqrt[3]{x}",
        r"\dot{x}=\sqrt[5]{x}",
        r"\dot{x}=-\sqrt[7]{x}",
    ] {
        let j = jac(&latex(src), 0.0, &[0.0], &[]);
        assert!(j[0].is_infinite(), "{src}: {j:?}");
    }
    let sys = latex(r"\dot{x}=\sqrt[5]{y}\\\dot{y}=-y");
    assert_eq!(
        jac(&sys, 0.0, &[0.3, 0.0], &[]),
        [0.0, f64::INFINITY, 0.0, -1.0]
    );
    let sys = latex(r"H(q,p)=\frac{p^2}{2}+q\sqrt[5]{q}");
    check_derivatives(&sys, 0.0, &[-0.7, 0.4], &[], 1e-6);
}

#[test]
fn nested_odd_roots_compile() {
    // Each root used to copy its argument, so 30 levels meant 2^30 nodes.
    let src = format!(r"\dot{{x}}={}x{}", r"\sqrt[5]{".repeat(30), "}".repeat(30));
    let sys = latex(&src);
    assert_eq!(sys.instruction_count(), 31);
    assert_close(&sys.eval_alloc(0.0, &[-1.0], &[]), &[-1.0], 1e-14);
}

#[test]
fn mathlive_quirks() {
    let cases: &[(&str, f64)] = &[
        ("x^{\\prime}=\\exponentialE^{x}", 2f64.exp()),
        ("x^\\prime=\\operatorname{sin}\\left(x\\right)", 2f64.sin()),
        ("\\dot{x}=\\sin x", 2f64.sin()),
        ("\\dot{x}=\\left|1-x\\right|", 1.0),
        ("\\dot{x}=|1-x|", 1.0),
        ("\\dot{x}=\\sqrt[3]{-8x}", -(16f64.cbrt())),
        ("\\dot{x}=\\sqrt{x}", 2f64.sqrt()),
        ("\\dot{x}=x\\cdot x\\times x", 8.0),
        ("\\dot{x}=\\frac12x", 1.0),
        ("\\dot{x}=x^{2}+x^2", 8.0),
        ("\\dot{x}=\\,x\\;\\!\\quad+\\qquad1", 3.0),
        ("\\dot{x}={x}^{{2}}", 4.0),
        ("\\dot{x}=e^{-x}", (-2f64).exp()),
        ("\\dot{x}=\\pi x", 2.0 * std::f64::consts::PI),
        ("\\dot{x}(t)=-x(t)", -2.0),
        ("\\dot{x}=\\mathrm{e}^{x}", 2f64.exp()),
        ("\\dot{x}=\\left[x+1\\right]^2", 9.0),
        ("\\dot{x}=\\ln\\left(x\\right)+\\log x", 2.0 * 2f64.ln()),
        ("\\dot{x}=\\displaystyle\\dfrac{x}{4}", 0.5),
    ];
    for &(src, want) in cases {
        let sys = latex(src);
        let got = sys.eval_alloc(0.0, &[2.0], &[]);
        assert_close(&got, &[want], 1e-14);
        let _ = src;
    }
}

#[test]
fn names_greek_subscripts_and_multiletter() {
    let sys = latex(
        "\\dot{\\varphi}=\\omega_0\\\\\\dot{\\omega_0}=-k_{12}\\sin\\varphi-\\mathrm{gain}\\,\\omega_0",
    );
    let (s, p) = names(&sys);
    assert_eq!(s, ["varphi", "omega_0"]);
    assert_eq!(p, ["k_12", "gain"]);
    assert_eq!(sys.info.states[1].latex, "\\omega_{0}");
    assert_eq!(sys.info.params[0].latex, "k_{12}");
    assert_eq!(sys.info.params[1].latex, "\\mathrm{gain}");
    assert_eq!(
        sys.info.equations_latex[1],
        "\\dot{\\omega}_{0} = -k_{12} \\sin\\varphi - \\mathrm{gain} \\omega_{0}"
    );
}

#[test]
fn all_functions_evaluate_and_differentiate() {
    let funcs = [
        ("sin", f64::sin as fn(f64) -> f64),
        ("cos", f64::cos),
        ("tan", f64::tan),
        ("exp", f64::exp),
        ("ln", f64::ln),
        ("log", f64::ln),
        ("sqrt", f64::sqrt),
        ("abs", f64::abs),
        ("sinh", f64::sinh),
        ("cosh", f64::cosh),
        ("tanh", f64::tanh),
        ("arcsin", f64::asin),
        ("arccos", f64::acos),
        ("arctan", f64::atan),
    ];
    for (name, f) in funcs {
        let sys = text(&format!("x' = {name}(x)"));
        let x = [0.4];
        assert_close(&sys.eval_alloc(0.0, &x, &[]), &[f(0.4)], 1e-15);
        check_derivatives(&sys, 0.0, &x, &[], 1e-7);
        let lsys = latex(&format!("\\dot{{x}}=\\{name}\\left(x\\right)"));
        assert_eq!(
            lsys.eval_alloc(0.0, &x, &[]),
            sys.eval_alloc(0.0, &x, &[]),
            "{name}"
        );
    }
    let sys = text("x' = sign(x) + min(x, a) + max(x, a)");
    assert_eq!(sys.eval_alloc(0.0, &[-2.0], &[3.0]), [-1.0 - 2.0 + 3.0]);
    assert_eq!(jac(&sys, 0.0, &[-2.0], &[3.0]), [1.0]);
    let sys = text("x' = x^a");
    check_derivatives(&sys, 0.0, &[1.7], &[2.5], 1e-7);
    let sys = text("x' = x^0.5 + x^1.5 + x^(-2) + 2^x");
    check_derivatives(&sys, 0.0, &[1.7], &[], 1e-7);
}

#[test]
fn integer_powers_are_exact_for_negative_bases() {
    let sys = latex("\\dot{x}=x^3+x^{-2}");
    assert_eq!(sys.eval_alloc(0.0, &[-2.0], &[]), [-8.0 + 0.25]);
    assert_eq!(jac(&sys, 0.0, &[-2.0], &[]), [12.0 + 0.25]);
}

#[test]
fn parameter_defaults_and_order() {
    let sys = text("a = 2\nx' = b*x + a\nb = -0.5e1\nc = 3*pi");
    assert_eq!(names(&sys).1, ["b", "a"]);
    assert_eq!(sys.default_params(), [-5.0, 2.0]);
    let out = compile(
        "a = 2\nx' = b*x + a\nb = -0.5e1\nc = 3*pi",
        InputFormat::Text,
    );
    let w = &out.diagnostics;
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].message.contains("isn't used"), "{w:?}");
}

#[test]
fn compiled_system_is_shareable_across_threads() {
    let sys = std::sync::Arc::new(latex(LORENZ));
    let p = sys.default_params();
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let sys = sys.clone();
            let p = p.clone();
            std::thread::spawn(move || {
                let x = [i as f64, 1.0, 2.0];
                let mut acc = 0.0;
                for _ in 0..1000 {
                    acc += sys.eval_alloc(0.0, &x, &p)[0] + jac(&sys, 0.0, &x, &p)[0];
                }
                acc
            })
        })
        .collect();
    for (i, h) in handles.into_iter().enumerate() {
        let want = 1000.0 * (10.0 * (1.0 - i as f64) - 10.0);
        assert_close(&[h.join().unwrap()], &[want], 1e-12);
    }
}

#[test]
fn info_serializes_in_camel_case() {
    let sys = latex("\\dot{x}=-x");
    let json = serde_json::to_string(&sys.info).unwrap();
    assert!(json.contains("\"equationsLatex\""), "{json}");
    assert!(json.contains("\"kind\":\"ode\""), "{json}");
    let out = compile("x' = y", InputFormat::Text);
    let d = serde_json::to_string(&out.diagnostics).unwrap();
    assert!(d.contains("\"severity\":\"warning\""), "{d}");
    assert!(d.contains("\"span\":[5,6]"), "{d}");
}

#[test]
fn states_introduced_by_order_reduction_are_marked_derived() {
    let derived = |src: &str, f: InputFormat| -> Vec<(String, bool)> {
        let out = compile(src, f);
        let sys = out
            .system
            .unwrap_or_else(|| panic!("{src}: {:?}", out.diagnostics));
        sys.info
            .states
            .iter()
            .map(|s| (s.name.clone(), s.derived))
            .collect()
    };
    let own = |v: &[(&str, bool)]| -> Vec<(String, bool)> {
        v.iter().map(|(n, d)| (n.to_string(), *d)).collect()
    };
    assert_eq!(
        derived("\\ddot{\\theta}=-\\sin\\theta", InputFormat::Latex),
        own(&[("theta", false), ("theta'", true)])
    );
    assert_eq!(
        derived("x''' = -x\ny' = x - y", InputFormat::Text),
        own(&[("x", false), ("x'", true), ("x''", true), ("y", false)])
    );
    // First-order states, Hamiltonian momenta and maps are never derived,
    // even when their display form is dotted.
    assert_eq!(
        derived("\\dot{x}=y\\\\\\dot{y}=-x", InputFormat::Latex),
        own(&[("x", false), ("y", false)])
    );
    assert_eq!(
        derived("H(q,p)=\\frac{p^2}{2}-\\cos q", InputFormat::Latex),
        own(&[("q", false), ("p", false)])
    );
    assert_eq!(
        derived("x_{n+1}=rx_n\\left(1-x_n\\right)", InputFormat::Latex),
        own(&[("x", false)])
    );
    let json = serde_json::to_value(&latex("\\ddot{x}=-x").info).unwrap();
    assert_eq!(json["states"][1]["derived"], true);
    assert_eq!(json["states"][0]["derived"], false);
}
