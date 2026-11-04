//! Size limits: the largest accepted inputs compile and evaluate on a small
//! thread stack, and larger ones give an error instead of crashing.

mod common;

use common::*;
use dyn_core::VectorField;
use dyn_expr::{compile, InputFormat, Severity};

/// Stack size for the checks: 512 KiB in release builds (threads usually
/// get 2 MiB or more), 2 MiB in debug builds, whose stack frames are much
/// bigger.
const STACK_KIB: usize = if cfg!(debug_assertions) { 2048 } else { 512 };

/// Run `f` on a thread with [`STACK_KIB`] of stack (a stack overflow aborts
/// the test process, so the test fails loudly if the limits are too
/// generous).
fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(STACK_KIB << 10).spawn(f).unwrap().join().unwrap();
}

fn error_message(src: &str) -> String {
    let out = compile(src, InputFormat::Latex);
    assert!(out.system.is_none(), "{} chars compiled", src.len());
    out.diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .expect("an error")
}

/// Compile, evaluate, differentiate and print the system.
fn exercise(src: &str) {
    let sys = latex(src);
    let x = vec![0.5; sys.dim()];
    let p = sys.default_params();
    let _ = sys.eval_alloc(0.0, &x, &p);
    let _ = jac(&sys, 0.0, &x, &p);
    assert!(!sys.info.equations_latex[0].is_empty());
}

/// Inputs just below the limits (49 nested brackets, a sum of 250 terms,
/// ...).
fn largest_cases() -> Vec<String> {
    vec![
        format!(r"\dot{{x}}={}x{}", r"\left(".repeat(49), r"\right)".repeat(49)),
        format!(r"\dot{{x}}=x{}", "+x".repeat(250)),
        format!(r"\dot{{x}}=x{}", "y".repeat(250)),
        format!(r"\dot{{x}}=\sin x{}", "y".repeat(250)),
        format!(r"\dot{{x}}={}x{}", r"\frac{1}{1+".repeat(24), "}".repeat(24)),
        format!(r"\dot{{x}}={}x", r"\sin ".repeat(49)),
        format!(r"\dot{{x}}={}x", "-".repeat(95)),
        format!(r"\ddot{{x}}{}=0", "+x".repeat(250)),
        format!(r"x{}=\ddot{{x}}", "+x".repeat(250)),
        format!(r"H(q,p)=p^2{}", "+q".repeat(250)),
        format!(r"\dot{{x}}=\left({}\right){}", format!("x{}", "+x".repeat(120)), "+x".repeat(120)),
    ]
}

#[test]
fn largest_accepted_inputs_fit_in_a_small_stack() {
    for src in largest_cases() {
        on_small_stack(move || exercise(&src));
    }
}

#[test]
fn too_deep_or_too_long_input_is_an_error() {
    on_small_stack(|| {
        let m = error_message(&format!(r"\dot{{x}}={}x{}", r"\left(".repeat(3000), r"\right)".repeat(3000)));
        assert!(m.contains("nested too deeply"), "{m}");
        let m = error_message(&format!(r"\dot{{x}}={}x", "-".repeat(3000)));
        assert!(m.contains("nested too deeply"), "{m}");
        let m = error_message(&format!(r"\dot{{x}}={}x", r"\sin-".repeat(3000)));
        assert!(m.contains("nested too deeply"), "{m}");
        let m = error_message(&format!(r"\dot{{x}}=x{}", "+x".repeat(20_000)));
        assert!(m.contains("too long"), "{m}");
        let m = error_message(&format!(r"\dot{{x}}=x{}", "y".repeat(20_000)));
        assert!(m.contains("too long"), "{m}");
        let m = error_message(&format!(r"\dot{{x}}=\sin x{}", "y".repeat(20_000)));
        assert!(m.contains("too long"), "{m}");
        let out = compile(&format!("x' = x{}", "+x".repeat(20_000)), InputFormat::Text);
        assert!(out.system.is_none());
    });
}
