//! Shared contract between the expression compiler (`dyn-expr`), the solvers
//! (`dyn-solve`) and the analyses (`dyn-analysis`).
//!
//! Everything numeric in the app goes through [`VectorField`]. The expression
//! compiler produces one from the user's equations; solvers and analyses only
//! ever see this trait, so they can be tested with hand-written closures
//! ([`FnField`]) and never depend on the parser.

use serde::{Deserialize, Serialize};

/// Whether the right-hand side is a flow (ODE) or an iterated map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemKind {
    /// `dx/dt = f(t, x; p)`.
    Ode,
    /// `x_{n+1} = f(n, x_n; p)`.
    Map,
}

/// A (possibly non-autonomous, parameterised) right-hand side.
///
/// Conventions:
/// - `x.len() == dim()`, `p.len() == n_params()`, `out.len() == dim()`.
/// - Jacobians are dense, **row-major**: `out[i * dim + j] = ∂f_i/∂x_j`.
/// - For maps `t` is the iteration index `n` as `f64`.
/// - Implementations must be cheap to call and must not allocate in `eval`.
pub trait VectorField: Send + Sync {
    fn dim(&self) -> usize;
    fn n_params(&self) -> usize;
    fn kind(&self) -> SystemKind;

    /// ODE: `out = dx/dt`. Map: `out = x_{n+1}`.
    fn eval(&self, t: f64, x: &[f64], p: &[f64], out: &mut [f64]);

    /// `out[i*dim + j] = ∂f_i/∂x_j`. The default uses central finite
    /// differences; the expression compiler overrides it with exact
    /// forward-mode automatic differentiation.
    fn jacobian(&self, t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        fd_jacobian(self, t, x, p, out);
    }

    /// `out[i] = ∂f_i/∂p_k`. Default: central finite differences.
    fn param_jacobian(&self, t: f64, x: &[f64], p: &[f64], k: usize, out: &mut [f64]) {
        fd_param_jacobian(self, t, x, p, k, out);
    }

    /// `true` if `t` (or `n`) appears explicitly on the right-hand side.
    fn is_autonomous(&self) -> bool {
        true
    }
}

fn fd_step(v: f64) -> f64 {
    // Optimal step for central differences ~ eps^(1/3) * scale.
    6.055_454_452_393_343e-6 * v.abs().max(1.0)
}

/// Central-difference Jacobian `∂f/∂x`, row-major.
pub fn fd_jacobian<F: VectorField + ?Sized>(f: &F, t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
    let n = f.dim();
    debug_assert_eq!(out.len(), n * n);
    let mut xp = x.to_vec();
    let mut fp = vec![0.0; n];
    let mut fm = vec![0.0; n];
    for j in 0..n {
        let h = fd_step(x[j]);
        xp[j] = x[j] + h;
        f.eval(t, &xp, p, &mut fp);
        xp[j] = x[j] - h;
        f.eval(t, &xp, p, &mut fm);
        xp[j] = x[j];
        for i in 0..n {
            out[i * n + j] = (fp[i] - fm[i]) / (2.0 * h);
        }
    }
}

/// Central-difference `∂f/∂p_k`.
pub fn fd_param_jacobian<F: VectorField + ?Sized>(
    f: &F,
    t: f64,
    x: &[f64],
    p: &[f64],
    k: usize,
    out: &mut [f64],
) {
    let n = f.dim();
    let mut pp = p.to_vec();
    let mut fm = vec![0.0; n];
    let h = fd_step(p[k]);
    pp[k] = p[k] + h;
    f.eval(t, x, &pp, out);
    pp[k] = p[k] - h;
    f.eval(t, x, &pp, &mut fm);
    for i in 0..n {
        out[i] = (out[i] - fm[i]) / (2.0 * h);
    }
}

type EvalFn = dyn Fn(f64, &[f64], &[f64], &mut [f64]) + Send + Sync;

/// A [`VectorField`] built from closures. Used in tests and examples so that
/// solvers and analyses never need the parser.
///
/// ```
/// use dyn_core::{FnField, SystemKind, VectorField};
/// // Harmonic oscillator x' = v, v' = -w^2 x
/// let f = FnField::new(2, 1, SystemKind::Ode, |_t, x, p, out| {
///     out[0] = x[1];
///     out[1] = -p[0] * p[0] * x[0];
/// });
/// let mut out = [0.0; 2];
/// f.eval(0.0, &[1.0, 0.0], &[2.0], &mut out);
/// assert_eq!(out, [0.0, -4.0]);
/// ```
pub struct FnField {
    dim: usize,
    n_params: usize,
    kind: SystemKind,
    autonomous: bool,
    f: Box<EvalFn>,
    jac: Option<Box<EvalFn>>,
}

impl FnField {
    pub fn new(
        dim: usize,
        n_params: usize,
        kind: SystemKind,
        f: impl Fn(f64, &[f64], &[f64], &mut [f64]) + Send + Sync + 'static,
    ) -> Self {
        Self { dim, n_params, kind, autonomous: true, f: Box::new(f), jac: None }
    }

    /// Supply an exact Jacobian (row-major) instead of finite differences.
    pub fn with_jacobian(
        mut self,
        jac: impl Fn(f64, &[f64], &[f64], &mut [f64]) + Send + Sync + 'static,
    ) -> Self {
        self.jac = Some(Box::new(jac));
        self
    }

    /// Mark the field as non-autonomous (depends explicitly on `t`).
    pub fn non_autonomous(mut self) -> Self {
        self.autonomous = false;
        self
    }
}

impl VectorField for FnField {
    fn dim(&self) -> usize {
        self.dim
    }
    fn n_params(&self) -> usize {
        self.n_params
    }
    fn kind(&self) -> SystemKind {
        self.kind
    }
    fn eval(&self, t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        (self.f)(t, x, p, out)
    }
    fn jacobian(&self, t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        match &self.jac {
            Some(j) => j(t, x, p, out),
            None => fd_jacobian(self, t, x, p, out),
        }
    }
    fn is_autonomous(&self) -> bool {
        self.autonomous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fd_jacobian_matches_exact() {
        // Lorenz
        let f = FnField::new(3, 3, SystemKind::Ode, |_t, x, p, o| {
            o[0] = p[0] * (x[1] - x[0]);
            o[1] = x[0] * (p[1] - x[2]) - x[1];
            o[2] = x[0] * x[1] - p[2] * x[2];
        });
        let (x, p) = ([1.0, 2.0, 3.0], [10.0, 28.0, 8.0 / 3.0]);
        let mut j = [0.0; 9];
        f.jacobian(0.0, &x, &p, &mut j);
        let exact = [-10.0, 10.0, 0.0, 28.0 - 3.0, -1.0, -1.0, 2.0, 1.0, -8.0 / 3.0];
        for (a, b) in j.iter().zip(exact) {
            assert!((a - b).abs() < 1e-7, "{a} vs {b}");
        }
    }
}
