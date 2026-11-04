mod common;

use common::*;
use dyn_expr::{compile, Diagnostic, InputFormat, Severity};

fn latex_err(src: &str) -> Diagnostic {
    first_error(src, InputFormat::Latex)
}

fn text_err(src: &str) -> Diagnostic {
    first_error(src, InputFormat::Text)
}

fn first_error(src: &str, f: InputFormat) -> Diagnostic {
    fails(src, f)
        .into_iter()
        .find(|d| d.severity == Severity::Error)
        .unwrap()
}

fn spanned<'a>(src: &'a str, d: &Diagnostic) -> &'a str {
    let (a, b) = d.span.expect("diagnostic has a span");
    &src[a..b]
}

#[test]
fn empty_input() {
    for src in ["", "   ", "\n\n", "\\\\", ";"] {
        let d = latex_err(src);
        assert!(
            d.message.contains("Enter at least one equation"),
            "{src:?}: {d:?}"
        );
        assert_eq!(d.span, None);
    }
}

fn warnings(src: &str, f: InputFormat) -> Vec<Diagnostic> {
    compile(src, f)
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect()
}

#[test]
fn unknown_function_in_text_mode() {
    // A whole word is one name: `sinx` is a parameter, with a hint.
    let src = "x' = sinx";
    let sys = text(src);
    assert_eq!(names(&sys).1, ["sinx"]);
    let w = warnings(src, InputFormat::Text);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(
        w[0].message,
        "sinx is read as a parameter named sinx; did you mean sin(x)?"
    );
    assert_eq!(spanned(src, &w[0]), "sinx");
    let d = text_err("x' = f(x)");
    assert!(d.message.contains("f is not a known function"), "{d:?}");
    let d = text_err("x' = costheta(x)");
    assert!(d.message.contains("did you mean cos(theta)"), "{d:?}");
    assert!(d.message.contains("costheta*(…)"), "{d:?}");
    // The error already names the fix; no second note on the same name.
    assert!(warnings("x' = costheta(x)", InputFormat::Text).is_empty());
}

/// Names spelled as one identifier that begin with a function name compile
/// as parameters; the possible typo is a warning, not an error.
#[test]
fn names_that_start_with_a_function_compile_with_a_warning() {
    let cases = [
        (
            r"\dot{x}=-\mathrm{cost}\,x",
            InputFormat::Latex,
            "cost",
            r"\cos t",
        ),
        (
            r"\dot{x}=\mathrm{tank}-x",
            InputFormat::Latex,
            "tank",
            r"\tan k",
        ),
        (
            r"\dot{x}=\mathrm{signal}-x",
            InputFormat::Latex,
            "signal",
            r"\sign al",
        ),
        (
            r"\dot{x}=\text{cost}x",
            InputFormat::Latex,
            "cost",
            r"\cos t",
        ),
        ("x' = -cost*x", InputFormat::Text, "cost", "cos(t)"),
        ("x' = signal - x", InputFormat::Text, "signal", "sign(al)"),
    ];
    for (src, f, name, fix) in cases {
        let out = compile(src, f);
        let sys = out
            .system
            .unwrap_or_else(|| panic!("{src:?} should compile: {:?}", out.diagnostics));
        assert_eq!(names(&sys).1, [name], "{src:?}");
        assert!(!out.diagnostics.iter().any(Diagnostic::is_error), "{src:?}");
        let w: Vec<_> = out
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert_eq!(w.len(), 1, "{src:?}: {w:?}");
        assert!(
            w[0].message.contains(&format!("did you mean {fix}?")),
            "{src:?}: {w:?}"
        );
    }
    // The parameter is a real parameter: 2·cost at cost = 3 is 6.
    let sys = latex(r"\dot{x}=2\mathrm{cost}\\\mathrm{cost}=3");
    assert_eq!(sys.default_params(), [3.0]);
    assert_eq!(sys.eval_alloc(0.0, &[0.0], &[3.0]), [6.0]);
    // Given a value on its own line, the name is clearly meant: no warning.
    assert!(warnings(
        r"\dot{x}=\mathrm{cost}\\\mathrm{cost}=3",
        InputFormat::Latex
    )
    .is_empty());
}

#[test]
fn spelled_out_function_in_latex() {
    let src = "\\dot{x}=sinx";
    let d = latex_err(src);
    assert!(d.message.contains("did you mean \\sin"), "{d:?}");
    assert_eq!(spanned(src, &d), "sin");
}

#[test]
fn unbalanced_brackets() {
    let src = "\\dot{x}=\\sigma\\left(y-x";
    let d = latex_err(src);
    assert_eq!(d.message, "This ( is never closed");
    assert_eq!(spanned(src, &d), "\\left(");

    let src = "x' = (y - x))";
    let d = text_err(src);
    assert_eq!(d.message, "This ) has no matching (");
    assert_eq!(spanned(src, &d), ")");
    assert_eq!(d.span, Some((12, 13)));

    let d = latex_err("\\dot{x}=\\frac{1}{x");
    assert_eq!(d.message, "This { is never closed");

    let d = latex_err("\\dot{x}=(x]");
    assert!(d.message.contains("don't match"), "{d:?}");
}

#[test]
fn state_defined_twice() {
    let src = "x' = y\ny' = -x\nx' = 1";
    let d = text_err(src);
    assert!(d.message.contains("x has more than one equation"), "{d:?}");
    assert_eq!(d.span, Some((15, 17)), "points at the second definition");

    let d = latex_err("\\dot{x}=y\\\\\\ddot{x}=1");
    assert!(d.message.contains("more than one equation"), "{d:?}");
}

#[test]
fn symbol_that_looks_like_a_state_warns() {
    let src = "x' = y";
    let out = compile(src, InputFormat::Text);
    assert!(out.system.is_some());
    assert_eq!(out.diagnostics.len(), 1);
    let d = &out.diagnostics[0];
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.message, "y has no equation; treating it as a parameter");
    assert_eq!(spanned(src, d), "y");

    let out = compile("x_1' = x_2 + a", InputFormat::Text);
    assert_eq!(out.diagnostics.len(), 1, "{:?}", out.diagnostics);
    assert!(out.diagnostics[0]
        .message
        .starts_with("x_2 has no equation"));

    let out = compile("x' = y\ny = 2", InputFormat::Text);
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn division_by_literal_zero() {
    let src = "\\dot{x}=\\frac{x}{0}";
    let d = latex_err(src);
    assert_eq!(d.message, "Division by zero");
    assert_eq!(spanned(src, &d), "{0}");
    let d = text_err("a = 1/0\nx' = a");
    assert_eq!(d.message, "Division by zero");
}

#[test]
fn placeholder_box() {
    let src = "\\dot{x}=\\placeholder{}+1";
    let d = latex_err(src);
    assert_eq!(d.message, "Fill in the empty box");
    assert_eq!(spanned(src, &d), "\\placeholder{}");
    let d = latex_err("\\dot{x}=\\frac{\\placeholder{}}{2}");
    assert_eq!(d.message, "Fill in the empty box");
}

#[test]
fn decimal_comma() {
    let d = text_err("x' = 1,5*x");
    assert!(d.message.contains("Decimal commas"), "{d:?}");
    let d = latex_err("\\dot{x}=1{,}5x");
    assert!(d.message.contains("Decimal commas"), "{d:?}");
    let d = latex_err("\\dot{x}=\\sin\\left(1,5\\right)");
    assert!(d.message.contains("decimal"), "{d:?}");
}

#[test]
fn mixing_flows_and_maps() {
    let src = "x' = y\ny_{n+1} = x";
    let d = text_err(src);
    assert!(d.message.contains("mixes differential equations"), "{d:?}");
    assert_eq!(spanned(src, &d), "y_{n+1}");
}

#[test]
fn unclassifiable_lines() {
    let d = text_err("x + y = 2");
    assert!(
        d.message.contains("Can't tell what this line defines"),
        "{d:?}"
    );
    let d = text_err("x' + y' = 2");
    assert!(d.message.contains("Several highest derivatives"), "{d:?}");
    let d = text_err("x'*x' = 2");
    assert!(d.message.contains("Can't solve"), "{d:?}");
    let d = text_err("x' = 1\ny = x");
    assert!(d.message.contains("must be a plain number"), "{d:?}");
    let d = text_err("x' = 1\nx = 3");
    assert!(d.message.contains("state variable"), "{d:?}");
    let d = text_err("a = 2");
    assert!(d.message.contains("No equations found"), "{d:?}");
    let d = text_err("H = p^2/2\nx' = H");
    assert!(d.message.contains("H(q, p)"), "{d:?}");
    let d = text_err("t' = 1");
    assert!(d.message.contains("time variable"), "{d:?}");
}

#[test]
fn derivative_misuse() {
    let d = text_err("x' = x'");
    assert!(d.message.contains("can't appear on the right"), "{d:?}");
    let d = text_err("x' = y'");
    assert!(d.message.contains("y has no equation"), "{d:?}");
    let d = text_err("x_{n+1} = x'");
    assert!(d.message.contains("map"), "{d:?}");
    let d = text_err("H(q) = q^2");
    assert!(d.message.contains("as many momenta"), "{d:?}");
    let d = text_err("H(q, p) = p^2\nq' = 1");
    assert!(
        d.message.contains("already defines the whole system"),
        "{d:?}"
    );
}

#[test]
fn text_mode_style_errors() {
    let d = text_err("x' = x y");
    assert!(d.message.contains("Missing '*' between x and y"), "{d:?}");
    let d = text_err("x' = sin x");
    assert!(d.message.contains("sin(…)"), "{d:?}");
    let d = text_err("x' = \\sin x");
    assert!(d.message.contains("LaTeX"), "{d:?}");
}

#[test]
fn errors_on_several_lines_are_all_reported() {
    let out = compile("x' = (y\ny' = 1,5\nz' = sinx(x)", InputFormat::Text);
    let errors: Vec<_> = out.diagnostics.iter().filter(|d| d.is_error()).collect();
    assert_eq!(errors.len(), 3, "{errors:#?}");
    assert!(errors
        .windows(2)
        .all(|w| w[0].span.unwrap().0 < w[1].span.unwrap().0));
}

#[test]
fn unsupported_latex_is_named() {
    let d = latex_err("\\dot{x}=\\int x");
    assert!(d.message.contains("\\int isn't supported"), "{d:?}");
    let d = latex_err("\\dot{x}=\\infty");
    assert!(d.message.contains("Infinity"), "{d:?}");
    let d = latex_err("\\dot{x}=\\log_{10}x");
    assert!(d.message.contains("natural logarithm"), "{d:?}");
}
