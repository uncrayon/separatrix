//! Converting equations between LaTeX and text (`dyn_expr::convert`).

mod common;

use std::path::Path;

use common::*;
use dyn_core::VectorField;
use dyn_expr::{compile, convert, CompiledSystem, ConvertedLine, InputFormat, Severity};

const LATEX: InputFormat = InputFormat::Latex;
const TEXT: InputFormat = InputFormat::Text;

const TEMPLATES_JSON: &str = include_str!("../../../app/src/templates/templates.json");
/// The text form of every template, which the UI loads in text mode.
const TEMPLATE_TEXT: &str = "../../app/src/templates/templates.text.generated.json";
const UPDATE: &str = "UPDATE_TEMPLATE_TEXT";
/// `BLANK_TEMPLATE.equations` in `app/src/templates/index.ts`.
const BLANK: [&str; 2] = ["\\dot{x}=y", "\\dot{y}=-x"];

fn owned(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| l.to_string()).collect()
}

fn conv(lines: &[&str], from: InputFormat, to: InputFormat) -> Vec<ConvertedLine> {
    let out = convert(&owned(lines), from, to);
    assert_eq!(out.len(), lines.len());
    out
}

/// Every line converts; the converted lines.
fn all(lines: &[&str], from: InputFormat, to: InputFormat) -> Vec<String> {
    conv(lines, from, to)
        .into_iter()
        .zip(lines)
        .map(|(c, l)| {
            assert!(c.converted && c.note.is_none(), "{l:?} -> {c:?}");
            c.source
        })
        .collect()
}

fn l2t(line: &str) -> String {
    all(&[line], LATEX, TEXT).remove(0)
}

fn t2l(line: &str) -> String {
    all(&[line], TEXT, LATEX).remove(0)
}

/// The line is kept as typed; its note.
fn kept(line: &str, from: InputFormat, to: InputFormat) -> String {
    let c = conv(&[line], from, to).remove(0);
    assert!(!c.converted, "{line:?} -> {c:?}");
    assert_eq!(c.source, line);
    c.note.expect("a kept line has a note")
}

fn system(lines: &[String], f: InputFormat) -> CompiledSystem {
    let src: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|l| !l.trim().is_empty())
        .collect();
    ok(&src.join("\n"), f)
}

/// Two systems are the same: names, kinds, flags and defaults, and values
/// and Jacobians bit for bit at random points.
fn assert_same_system(a: &CompiledSystem, b: &CompiledSystem, r: &mut Rng, what: &str) {
    let (ia, ib) = (&a.info, &b.info);
    assert_eq!(ia.kind, ib.kind, "{what}");
    assert_eq!(ia.hamiltonian, ib.hamiltonian, "{what}");
    assert_eq!(ia.autonomous, ib.autonomous, "{what}");
    let states = |i: &dyn_expr::SystemInfo| -> Vec<(String, bool)> {
        i.states
            .iter()
            .map(|s| (s.name.clone(), s.derived))
            .collect()
    };
    assert_eq!(states(ia), states(ib), "{what}");
    let params = |i: &dyn_expr::SystemInfo| -> Vec<(String, u64)> {
        i.params
            .iter()
            .map(|p| (p.name.clone(), p.default.to_bits()))
            .collect()
    };
    assert_eq!(params(ia), params(ib), "{what}");
    let bits = |v: &[f64]| -> Vec<u64> { v.iter().map(|x| x.to_bits()).collect() };
    for _ in 0..20 {
        let t = r.uniform(-2.0, 2.0);
        let x: Vec<f64> = (0..a.dim()).map(|_| r.uniform(-2.0, 2.0)).collect();
        let p: Vec<f64> = (0..a.n_params()).map(|_| r.uniform(0.5, 2.0)).collect();
        assert_eq!(
            bits(&a.eval_alloc(t, &x, &p)),
            bits(&b.eval_alloc(t, &x, &p)),
            "{what}"
        );
        assert_eq!(bits(&jac(a, t, &x, &p)), bits(&jac(b, t, &x, &p)), "{what}");
    }
}

/// Converting `lines` from `from` converts every line, and compiles to the
/// same system; converting back and forth again gives the same lines.
fn round_trip(lines: &[&str], from: InputFormat, r: &mut Rng) -> Vec<String> {
    let to = match from {
        LATEX => TEXT,
        TEXT => LATEX,
    };
    let there = all(lines, from, to);
    assert_same_system(
        &system(&owned(lines), from),
        &system(&there, to),
        r,
        &format!("{lines:?} -> {there:?}"),
    );
    let refs: Vec<&str> = there.iter().map(String::as_str).collect();
    let back = all(&refs, to, from);
    let back_refs: Vec<&str> = back.iter().map(String::as_str).collect();
    assert_eq!(
        all(&back_refs, from, to),
        there,
        "{lines:?} is not a fixed point"
    );
    there
}

fn templates() -> Vec<(String, Vec<String>)> {
    let v: serde_json::Value = serde_json::from_str(TEMPLATES_JSON).expect("templates.json parses");
    let mut out: Vec<(String, Vec<String>)> = v
        .as_array()
        .expect("an array")
        .iter()
        .map(|t| {
            assert_eq!(t["format"], "latex");
            let eqs = t["equations"].as_array().expect("equations");
            (
                t["id"].as_str().expect("id").to_string(),
                eqs.iter()
                    .map(|e| e.as_str().expect("a string").to_string())
                    .collect(),
            )
        })
        .collect();
    out.push(("blank".to_string(), owned(&BLANK)));
    out
}

#[test]
fn every_template_converts_to_text_and_means_the_same() {
    let mut r = Rng(0x5EED_0001);
    let mut generated = String::from("{\n");
    let list = templates();
    for (i, (id, eqs)) in list.iter().enumerate() {
        let refs: Vec<&str> = eqs.iter().map(String::as_str).collect();
        let text = round_trip(&refs, LATEX, &mut r);
        let back = all(
            &text.iter().map(String::as_str).collect::<Vec<_>>(),
            TEXT,
            LATEX,
        );
        assert_same_system(&system(eqs, LATEX), &system(&back, LATEX), &mut r, id);
        let quoted: Vec<String> = text
            .iter()
            .map(|l| serde_json::to_string(l).unwrap())
            .collect();
        let comma = if i + 1 < list.len() { "," } else { "" };
        generated.push_str(&format!(
            "  {}: [{}]{comma}\n",
            serde_json::to_string(id).unwrap(),
            quoted.join(", ")
        ));
    }
    generated.push_str("}\n");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(TEMPLATE_TEXT);
    if std::env::var_os(UPDATE).is_some() {
        std::fs::write(&path, &generated).expect("write the template text");
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        have == generated,
        "app/src/templates/templates.text.generated.json is out of date; run {UPDATE}=1 cargo test \
         -p dyn-expr --test convert"
    );
}

#[test]
fn template_text_reads_naturally() {
    let text: Vec<(String, Vec<String>)> = templates()
        .into_iter()
        .map(|(id, eqs)| {
            let refs: Vec<&str> = eqs.iter().map(String::as_str).collect();
            (id, all(&refs, LATEX, TEXT))
        })
        .collect();
    let get = |id: &str| text.iter().find(|(i, _)| i == id).expect(id).1.clone();
    assert_eq!(
        get("rabbits-vs-sheep"),
        ["x' = x*(3 - x - 2*y)", "y' = y*(2 - x - y)"]
    );
    assert_eq!(get("pendulum"), ["theta'' = -sin(theta)"]);
    assert_eq!(
        get("damped-pendulum"),
        ["theta'' + b*theta' + sin(theta) = 0"]
    );
    assert_eq!(get("logistic-map"), ["x_{n+1} = r*x_n*(1 - x_n)"]);
    assert_eq!(get("hamiltonian-pendulum"), ["H(q, p) = p^2/2 - cos(q)"]);
    assert_eq!(get("lorenz")[0], "x' = sigma*(y - x)");
}

fn random_systems(seed: u64, cases: usize, build: impl Fn(&E, &E) -> (Vec<String>, InputFormat)) {
    let mut r = Rng(seed);
    for case in 0..cases {
        let f = gen(&mut r, 4);
        let g = gen(&mut r, 4);
        let (lines, from) = build(&f, &g);
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let to = if from == LATEX { TEXT } else { LATEX };
        let there = all(&refs, from, to);
        assert_same_system(
            &system(&lines, from),
            &system(&there, to),
            &mut r,
            &format!("case {case}: {lines:?} -> {there:?}"),
        );
    }
}

#[test]
fn random_latex_systems_convert_to_the_same_text_systems() {
    random_systems(0xA11CE, 2000, |f, g| {
        (
            vec![
                format!("\\dot{{x}}={}+0ya", to_latex(f)),
                format!("\\dot{{y}}={}+0x", to_latex(g)),
            ],
            LATEX,
        )
    });
}

#[test]
fn random_text_systems_convert_to_the_same_latex_systems() {
    random_systems(0xB0B, 2000, |f, g| {
        (
            vec![
                format!("x' = {} + 0*y*a", to_text(f)),
                format!("y' = {} + 0*x", to_text(g)),
            ],
            TEXT,
        )
    });
}

#[test]
fn random_maps_defaults_and_second_order_systems_convert() {
    random_systems(0xC0FFEE, 300, |f, g| {
        (
            vec![
                format!("x_{{n+1}}={}+0y_na", to_latex(f)),
                String::new(),
                format!("y_{{n+1}}={}", to_latex(g)),
                "a=0.75".to_string(),
            ],
            LATEX,
        )
    });
    random_systems(0xD00D, 300, |f, g| {
        (
            vec![
                format!("x'' = {} + 0*a; y' = {} + 0*x'", to_text(f), to_text(g)),
                "a = 2.5e-3".to_string(),
            ],
            TEXT,
        )
    });
}

// ---- where the two formats read the same characters differently ----

#[test]
fn x_of_t_is_the_state_or_a_product() {
    assert_eq!(all(&["\\dot{x}=-x(t)"], LATEX, TEXT), ["x' = -x"]);
    let out = all(&["\\dot{y}=x(t)", "\\dot{z}=z"], LATEX, TEXT);
    assert_eq!(out[0], "y' = x*t");
    assert_eq!(l2t("\\dot{x}=p(x+1)"), "x' = p*(x + 1)");
    let mut r = Rng(1);
    round_trip(&["\\dot{x}=-x(t)+y(t)", "\\dot{y}=x"], LATEX, &mut r);
    round_trip(&["x' = -x(t)"], TEXT, &mut r);
}

#[test]
fn functions_keep_their_reach() {
    assert_eq!(l2t("\\dot{x}=\\sin x y"), "x' = sin(x*y)");
    assert_eq!(l2t("\\dot{x}=\\sin x\\cdot y"), "x' = sin(x)*y");
    assert_eq!(l2t("\\dot{x}=\\sin^2 x"), "x' = sin(x)^2");
    assert_eq!(l2t("\\dot{x}=\\sin^{-1} x"), "x' = arcsin(x)");
    assert_eq!(t2l("x' = sin(x)*y"), "\\dot{x} = \\sin x \\cdot y");
    assert_eq!(t2l("x' = sin(x*y)"), "\\dot{x} = \\sin\\left(x y\\right)");
    assert_eq!(t2l("x' = -sin(x)*y"), "\\dot{x} = -\\sin x \\cdot y");
    let mut r = Rng(2);
    for src in [
        "\\dot{x}=\\sin x y",
        "\\dot{x}=\\sin^2 x",
        "\\dot{x}=\\sin^{-1} x",
        "\\dot{x}=2\\cos x\\sin y",
    ] {
        round_trip(&[src], LATEX, &mut r);
    }
}

#[test]
fn e_to_the_x_and_exp_stay_different() {
    assert_eq!(l2t("\\dot{x}=e^{x}"), "x' = e^x");
    assert_eq!(l2t("\\dot{x}=\\exp x"), "x' = exp(x)");
    assert_eq!(t2l("x' = exp(-x)"), "\\dot{x} = \\exp\\left(-x\\right)");
    assert_eq!(t2l("x' = e^(-x)"), "\\dot{x} = e^{-x}");
}

#[test]
fn roots() {
    assert_eq!(l2t("\\dot{x}=\\sqrt{x}"), "x' = sqrt(x)");
    assert_eq!(l2t("\\dot{x}=\\sqrt[3]{x}"), "x' = cbrt(x)");
    assert_eq!(l2t("\\dot{x}=\\sqrt[4]{x}"), "x' = x^(1/4)");
    assert_eq!(t2l("x' = x^(1/4)"), "\\dot{x} = \\sqrt[4]{x}");
    // Real for x < 0, unlike x^(1/5); text mode writes it root(x, 5).
    assert_eq!(l2t("\\dot{x}=\\sqrt[5]{x}"), "x' = root(x, 5)");
    assert_eq!(t2l("x' = root(x, 5)"), "\\dot{x} = \\sqrt[5]{x}");
    let mut r = Rng(3);
    for src in [
        "\\dot{x}=\\sqrt[5]{x}",
        "\\dot{x}=\\sqrt[4]{x}+\\sqrt[3]{-x}",
        "\\dot{x}=\\sqrt[7]{\\sqrt[5]{x}}",
    ] {
        round_trip(&[src], LATEX, &mut r);
    }
}

#[test]
fn absolute_values_and_derivatives() {
    assert_eq!(
        l2t("\\dot{x}=|x|-\\left|x-1\\right|"),
        "x' = abs(x) - abs(x - 1)"
    );
    assert_eq!(t2l("x' = abs(x)"), "\\dot{x} = \\left|x\\right|");
    assert_eq!(l2t("\\frac{dx}{dt}=-x"), "x' = -x");
    assert_eq!(l2t("\\frac{\\mathrm{d}x}{\\mathrm{d}t}=-x"), "x' = -x");
    assert_eq!(l2t("\\frac{d^2x}{dt^2}=-x"), "x'' = -x");
    assert_eq!(t2l("dx/dt = -x"), "\\dot{x} = -x");
    assert_eq!(l2t("x'''=-x"), "x''' = -x");
    assert_eq!(t2l("x''' = -x"), "x''' = -x");
    assert_eq!(t2l("x'' = -x'"), "\\ddot{x} = -\\dot{x}");
    let mut r = Rng(4);
    round_trip(&["\\frac{d^2x}{dt^2}=-x-\\frac{dx}{dt}"], LATEX, &mut r);
    round_trip(&["x''' = -x - x' - x''"], TEXT, &mut r);
}

#[test]
fn maps_and_hamiltonians() {
    assert_eq!(l2t("x_{n+1}=rx_n"), "x_{n+1} = r*x_n");
    let out = all(&["x_{1,n+1}=x_{2,n}", "x_{2,n+1}=-x_{1,n}"], LATEX, TEXT);
    assert_eq!(out, ["x_1_{n+1} = x_2_n", "x_2_{n+1} = -x_1_n"]);
    let back = all(&["x_1_{n+1} = x_2_n", "x_2_{n+1} = -x_1_n"], TEXT, LATEX);
    assert_eq!(back, ["x_{1,n+1} = x_{2,n}", "x_{2,n+1} = -x_{1,n}"]);
    assert_eq!(
        l2t("H\\left(q,p\\right)=\\frac{p^2}{2}+q^4"),
        "H(q, p) = p^2/2 + q^4"
    );
    assert_eq!(
        t2l("H(q, p) = p^2/2 + q^4"),
        "H\\left(q, p\\right) = \\frac{p^{2}}{2} + q^{4}"
    );
    let mut r = Rng(5);
    round_trip(
        &["x_{1,n+1}=x_{2,n}", "x_{2,n+1}=-x_{1,n}+ax_{2,n}"],
        LATEX,
        &mut r,
    );
    round_trip(
        &["H\\left(q_1,q_2,p_1,p_2\\right)=\\frac{p_1^2+p_2^2}{2}+q_1q_2"],
        LATEX,
        &mut r,
    );
}

#[test]
fn a_function_lookalike_keeps_its_warning() {
    let text = l2t("\\dot{x}=\\mathrm{cost}");
    assert_eq!(text, "x' = cost");
    let out = compile(&text, TEXT);
    assert!(out.system.is_some());
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("cos(t)")),
        "{:?}",
        out.diagnostics
    );
    assert_eq!(t2l("x' = cost"), "\\dot{x} = \\mathrm{cost}");
}

#[test]
fn names_keep_their_identity() {
    assert_eq!(l2t("\\dot{x_1}=-k_{12}x_1"), "x_1' = -k_12*x_1");
    assert_eq!(t2l("x_1' = -k_12*x_1"), "\\dot{x}_{1} = -k_{12} x_{1}");
    assert_eq!(l2t("\\dot{x}=-k_{\\mathrm{on}}x"), "x' = -k_on*x");
    assert_eq!(t2l("x' = -k_on*x"), "\\dot{x} = -k_{\\mathrm{on}} x");
    assert_eq!(l2t("\\dot{x}=-\\omega_0^2x"), "x' = -omega_0^2*x");
    // Text mode's x1 and omega0 are not x_1 and omega_0.
    assert_eq!(
        t2l("x1' = -omega0*x1"),
        "\\dot{\\mathrm{x1}} = -\\mathrm{omega0} \\mathrm{x1}"
    );
    let mut r = Rng(6);
    round_trip(&["x1' = -omega0*x1 + x_1", "x_1' = x1"], TEXT, &mut r);
    let note = kept("x' = -a_b_c*x", TEXT, LATEX);
    assert!(note.contains("a_b_c"), "{note}");
}

#[test]
fn numbers_are_exact() {
    assert_eq!(l2t("\\dot{x}=0.1x"), "x' = 0.1*x");
    assert_eq!(t2l("x' = 1e-7*x"), "\\dot{x} = 0.0000001 x");
    assert_eq!(t2l("x' = 6.674e-11*x"), "\\dot{x} = 0.00000000006674 x");
    assert_eq!(l2t("\\dot{x}=0.00000000006674x"), "x' = 6.674e-11*x");
    assert_eq!(t2l("x' = 2(x+1)"), "\\dot{x} = 2 \\left(x + 1\\right)");
    let note = kept("x' = 1e300*x", TEXT, LATEX);
    assert!(note.contains("number"), "{note}");
    let mut r = Rng(7);
    round_trip(
        &[
            "x' = 1e-7*x + 6.674e-11 + 1e21*y",
            "y' = 0.30000000000000004",
        ],
        TEXT,
        &mut r,
    );
}

#[test]
fn precedence_is_kept() {
    let mut r = Rng(8);
    let cases = [
        ("-x^2", "-x^{2}"),
        ("x^-1", "x^{-1}"),
        ("a/b*c", "\\frac{a}{b} c"),
        ("a/(b*c)", "\\frac{a}{b c}"),
        ("x-(y-z)", "x - \\left(y - z\\right)"),
        ("-(x-y)", "-\\left(x - y\\right)"),
        ("(-x)^2", "\\left(-x\\right)^{2}"),
        ("-x*y", "-x y"),
        ("x*(-y)", "x \\left(-y\\right)"),
        ("2^3^2", "2^{3^{2}}"),
        ("(x^2)^3", "\\left(x^{2}\\right)^{3}"),
        ("x*3", "x \\cdot 3"),
        ("2*(1/2)", "2 \\cdot \\frac{1}{2}"),
        ("(a/b)^2", "\\left(\\frac{a}{b}\\right)^{2}"),
    ];
    for (text, latex) in cases {
        let line = format!("x' = {text} + 0*y*z*a*b*c");
        let out = t2l(&line);
        assert!(
            out.starts_with(&format!("\\dot{{x}} = {latex} + ")),
            "{text}: {out}"
        );
        round_trip(&[&line, "y' = 0", "z' = 0"], TEXT, &mut r);
    }
}

#[test]
fn implicit_equations_separators_empty_lines_and_constants() {
    let mut r = Rng(9);
    round_trip(
        &["\\ddot{\\theta}+b\\dot{\\theta}+\\sin\\theta=0"],
        LATEX,
        &mut r,
    );
    assert_eq!(
        all(&["\\dot{x}=y;\\dot{y}=-x"], LATEX, TEXT),
        ["x' = y; y' = -x"]
    );
    assert_eq!(
        all(&["x' = y; y' = -x"], TEXT, LATEX),
        ["\\dot{x} = y; \\dot{y} = -x"]
    );
    assert_eq!(
        all(&["\\dot{x}=y", "", "  ", "\\dot{y}=-x"], LATEX, TEXT),
        ["x' = y", "", "  ", "y' = -x"]
    );
    assert_eq!(l2t("\\dot{x}=\\pi e\\varepsilon"), "x' = pi*e*varepsilon");
    assert_eq!(
        t2l("x' = pi*e*varepsilon"),
        "\\dot{x} = \\pi e \\varepsilon"
    );
    round_trip(
        &["\\dot{x}=\\pi e^{\\varepsilon t}", "\\varepsilon=0.5"],
        LATEX,
        &mut r,
    );
}

#[test]
fn a_line_with_an_error_is_kept_and_the_others_convert() {
    let out = conv(
        &["\\dot{x}=y", "\\dot{y}=\\left(x", "\\dot{z}=z"],
        LATEX,
        TEXT,
    );
    assert_eq!(out[0].source, "x' = y");
    assert!(!out[1].converted);
    assert_eq!(out[1].source, "\\dot{y}=\\left(x");
    assert!(out[1].note.as_deref().unwrap().contains("error"));
    assert_eq!(out[2].source, "z' = z");
    // A text-mode error that LaTeX would read differently is kept too.
    let note = kept("x' = f(x)", TEXT, LATEX);
    assert!(note.contains("error"), "{note}");
}

#[test]
fn a_system_that_does_not_compile_still_converts_the_lines_that_parse() {
    // y has two equations: an error, but each line is fine on its own.
    let out = all(&["\\dot{y}=x(t)", "\\dot{x}=1", "\\dot{y}=2"], LATEX, TEXT);
    assert_eq!(out, ["y' = x", "x' = 1", "y' = 2"]);
}

#[test]
fn a_kept_line_says_whether_it_had_an_error() {
    // A valid line the target can't write: kept, and not an error, so the
    // UI calls the switch off (LaTeX would read 9.109e-31 as 9.109·e - 31).
    let c = conv(&["x' = -x/9.109e-31"], TEXT, LATEX).remove(0);
    assert!(!c.converted && !c.error, "{c:?}");
    assert_eq!(
        c.note.as_deref(),
        Some("LaTeX can't write the number 9.109e-31 exactly")
    );
    let c = conv(&["\\dot{x}=-\\mathrm{root}x"], LATEX, TEXT).remove(0);
    assert!(!c.converted && !c.error, "{c:?}");
    // A line with an error: kept, flagged, and the others still convert.
    let out = conv(&["x' = f(x)", "y' = y"], TEXT, LATEX);
    assert!(!out[0].converted && out[0].error, "{:?}", out[0]);
    assert!(out[1].converted && !out[1].error, "{:?}", out[1]);
    assert!(conv(&["\\dot{y}=\\left(x"], LATEX, TEXT)[0].error);
}

#[test]
fn a_state_named_x_n_converts_both_ways() {
    // `x_{n}` alone reads as a map's x at step n, but `\dot{x}_{n}` is the
    // derivative of the state x_n, so the derivative keeps its usual form.
    assert_eq!(l2t("\\dot{x}_n=-x_n"), "x_n' = -x_n");
    assert_eq!(t2l("x_n' = -x_n"), "\\dot{x}_{n} = -x_{n}");
    let mut r = Rng(10);
    round_trip(&["x_n' = -x_n + y", "y' = x_n"], TEXT, &mut r);
    round_trip(&["\\ddot{x}_n=-x_n-\\dot{x}_n"], LATEX, &mut r);
}

#[test]
fn a_function_power_converts_though_its_parameters_come_in_another_order() {
    // LaTeX writes the power before the argument, text after it, so the
    // parameters first appear in the other order; the system is the same.
    let bits = |v: Vec<f64>| -> Vec<u64> { v.iter().map(|x| x.to_bits()).collect() };
    let mut r = Rng(11);
    for (latex, text) in [
        ("\\dot{y}=\\sin^{a}b", "y' = sin(b)^a"),
        (
            "\\dot{x}=\\cos^{p}\\left(\\omega t\\right)",
            "x' = cos(omega*t)^p",
        ),
    ] {
        assert_eq!(l2t(latex), text);
        t2l(text);
        let (a, b) = (ok(latex, LATEX), ok(text, TEXT));
        let names = |s: &CompiledSystem| -> Vec<String> {
            s.info.params.iter().map(|p| p.name.clone()).collect()
        };
        let mut reversed = names(&a);
        reversed.reverse();
        assert_eq!(names(&b), reversed, "{latex}");
        for _ in 0..20 {
            let t = r.uniform(-2.0, 2.0);
            let x: Vec<f64> = (0..a.dim()).map(|_| r.uniform(-2.0, 2.0)).collect();
            let p: Vec<f64> = (0..a.n_params()).map(|_| r.uniform(0.5, 2.0)).collect();
            let q: Vec<f64> = p.iter().rev().copied().collect();
            assert_eq!(
                bits(a.eval_alloc(t, &x, &p)),
                bits(b.eval_alloc(t, &x, &q)),
                "{latex}"
            );
        }
    }
}

#[test]
fn a_line_with_no_equation_becomes_an_empty_line() {
    for blank in ["\\quad", "\\,", "\\\\", "\\begin{cases}\\end{cases}"] {
        let out = conv(&["\\dot{x}=y", "\\dot{y}=-x", blank], LATEX, TEXT);
        assert!(
            out.iter().all(|l| l.converted && l.note.is_none()),
            "{blank}: {out:?}"
        );
        assert_eq!(out[2].source, "", "{blank}");
        let text: Vec<String> = out.into_iter().map(|l| l.source).collect();
        assert!(compile(&text.join("\n"), TEXT).system.is_some(), "{blank}");
    }
    assert_eq!(all(&["x' = y; y' = -x", ";"], TEXT, LATEX)[1], "");
}
