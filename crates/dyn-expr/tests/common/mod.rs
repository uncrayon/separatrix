//! Helpers shared by the integration tests.
#![allow(dead_code)]

use dyn_core::{fd_jacobian, fd_param_jacobian, VectorField};
use dyn_expr::{compile, CompiledSystem, Diagnostic, InputFormat, Severity};

/// Compile and insist on success (warnings allowed).
pub fn ok(src: &str, f: InputFormat) -> CompiledSystem {
    let out = compile(src, f);
    let errors: Vec<&Diagnostic> =
        out.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "{src:?} failed: {errors:#?}");
    out.system.expect("system present when there are no errors")
}

pub fn latex(src: &str) -> CompiledSystem {
    ok(src, InputFormat::Latex)
}

pub fn text(src: &str) -> CompiledSystem {
    ok(src, InputFormat::Text)
}

/// Compile and return the diagnostics, insisting there is no system.
pub fn fails(src: &str, f: InputFormat) -> Vec<Diagnostic> {
    let out = compile(src, f);
    assert!(out.system.is_none(), "{src:?} should fail");
    assert!(out.diagnostics.iter().any(|d| d.severity == Severity::Error));
    out.diagnostics
}

pub fn names(sys: &CompiledSystem) -> (Vec<String>, Vec<String>) {
    (
        sys.info.states.iter().map(|s| s.name.clone()).collect(),
        sys.info.params.iter().map(|p| p.name.clone()).collect(),
    )
}

pub fn assert_close(a: &[f64], b: &[f64], tol: f64) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!((x - y).abs() <= tol * (1.0 + y.abs()), "entry {i}: {x} vs {y}\n{a:?}\n{b:?}");
    }
}

pub fn jac(sys: &CompiledSystem, t: f64, x: &[f64], p: &[f64]) -> Vec<f64> {
    let mut j = vec![0.0; sys.dim() * sys.dim()];
    sys.jacobian(t, x, p, &mut j);
    j
}

/// AD Jacobian and parameter Jacobians agree with central differences.
pub fn check_derivatives(sys: &CompiledSystem, t: f64, x: &[f64], p: &[f64], tol: f64) {
    let n = sys.dim();
    let mut fd = vec![0.0; n * n];
    fd_jacobian(sys, t, x, p, &mut fd);
    assert_close(&jac(sys, t, x, p), &fd, tol);
    for k in 0..sys.n_params() {
        let mut ad = vec![0.0; n];
        let mut fdp = vec![0.0; n];
        sys.param_jacobian(t, x, p, k, &mut ad);
        fd_param_jacobian(sys, t, x, p, k, &mut fdp);
        assert_close(&ad, &fdp, tol);
    }
}
