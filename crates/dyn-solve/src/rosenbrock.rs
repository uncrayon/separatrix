//! Rosenbrock 2(3): the L-stable linearly implicit scheme of MATLAB's
//! `ode23s` (L. F. Shampine & M. W. Reichelt, *The MATLAB ODE suite*, SIAM
//! J. Sci. Comput. 18, 1997, §3.1), with `W = I − h·d·J`:
//!
//! ```text
//! k1 = W⁻¹ (f(t, x) + h·d·∂f/∂t)
//! k2 = W⁻¹ (f(t + h/2, x + h/2·k1) − k1) + k1
//! x₁ = x + h·k2                                   (order 2)
//! k3 = W⁻¹ (f(t + h, x₁) − e₃₂ (k2 − F1) − 2 (k1 − F0) + h·d·∂f/∂t)
//! err = h/6 (k1 − 2 k2 + k3)                      (order 3)
//! ```
//!
//! `J` is the field's own Jacobian, exact for compiled systems. `∂f/∂t` is
//! a central difference, taken only when the field reads `t`. `J` and
//! `∂f/∂t` are evaluated once per accepted step; a rejected step only
//! refactors `W` for the smaller `h`.

use dyn_core::VectorField;

use crate::lu::{Lu, Singular};

/// `d = 1/(2 + √2)`, written out so every platform steps with the same bits.
pub(crate) const D: f64 = 0.292_893_218_813_452_5;
/// `e₃₂ = 6 + √2`.
const E32: f64 = 7.414_213_562_373_095;

/// The Jacobian at the start of the step, the factored `W` and the stages.
pub(crate) struct Rosenbrock {
    jac: Vec<f64>,
    dfdt: Vec<f64>,
    lu: Lu,
    k1: Vec<f64>,
    k2: Vec<f64>,
    k3: Vec<f64>,
    f1: Vec<f64>,
    scratch: Vec<f64>,
}

impl Rosenbrock {
    pub(crate) fn new(n: usize) -> Rosenbrock {
        Rosenbrock {
            jac: vec![0.0; n * n],
            dfdt: vec![0.0; n],
            lu: Lu::new(n),
            k1: vec![0.0; n],
            k2: vec![0.0; n],
            k3: vec![0.0; n],
            f1: vec![0.0; n],
            scratch: vec![0.0; n],
        }
    }

    /// `J = ∂f/∂x` and `∂f/∂t` at `(t, x)`, for every try of the next step.
    pub(crate) fn linearize(
        &mut self,
        f: &dyn VectorField,
        p: &[f64],
        t: f64,
        x: &[f64],
        mut eval: impl FnMut(f64, &[f64], &mut [f64]),
    ) {
        f.jacobian(t, x, p, &mut self.jac);
        if f.is_autonomous() {
            self.dfdt.fill(0.0);
            return;
        }
        let delta = f64::EPSILON.cbrt() * t.abs().max(1.0);
        let (lo, hi) = (t - delta, t + delta);
        eval(hi, x, &mut self.dfdt);
        eval(lo, x, &mut self.scratch);
        let span = hi - lo;
        for (d, lo) in self.dfdt.iter_mut().zip(&self.scratch) {
            *d = (*d - lo) / span;
        }
    }

    /// One try of the step `h` from `(t, x)` with `fx = f(t, x)`: the new
    /// point in `x_new` and `f(t + h, x_new)` in `f_new`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn stages(
        &mut self,
        h: f64,
        t: f64,
        x: &[f64],
        fx: &[f64],
        t_new: f64,
        x_new: &mut [f64],
        f_new: &mut [f64],
        mut eval: impl FnMut(f64, &[f64], &mut [f64]),
    ) -> Result<(), Singular> {
        let Rosenbrock {
            jac,
            dfdt,
            lu,
            k1,
            k2,
            k3,
            f1,
            scratch,
        } = self;
        let n = x.len();
        let hd = h * D;
        lu.factor_shifted(hd, jac)?;

        for i in 0..n {
            k1[i] = fx[i] + hd * dfdt[i];
        }
        lu.solve(k1);
        for i in 0..n {
            scratch[i] = x[i] + 0.5 * h * k1[i];
        }
        eval(t + 0.5 * h, scratch, f1);
        for i in 0..n {
            k2[i] = f1[i] - k1[i];
        }
        lu.solve(k2);
        for i in 0..n {
            k2[i] += k1[i];
            x_new[i] = x[i] + h * k2[i];
        }
        eval(t_new, x_new, f_new);
        for i in 0..n {
            k3[i] = f_new[i] - E32 * (k2[i] - f1[i]) - 2.0 * (k1[i] - fx[i]) + hd * dfdt[i];
        }
        lu.solve(k3);
        Ok(())
    }

    /// Component `i` of the error estimate of the last try.
    pub(crate) fn error(&self, h: f64, i: usize) -> f64 {
        h / 6.0 * (self.k1[i] - 2.0 * self.k2[i] + self.k3[i])
    }

    /// The scheme's continuous extension, `x + h·(θ(1 − θ)/(1 − 2d)·k1 +
    /// θ(θ − 2d)/(1 − 2d)·k2)` (`ntrp23s`), as `x + θ·(x₁ − x) + θ(1 − θ)·c2`.
    pub(crate) fn dense_c2(&self, h: f64, i: usize) -> f64 {
        h * (self.k1[i] - self.k2[i]) / (1.0 - 2.0 * D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_the_rounded_closed_forms() {
        let sqrt2 = std::f64::consts::SQRT_2;
        assert!((D - 1.0 / (2.0 + sqrt2)).abs() <= f64::EPSILON * D);
        assert!((E32 - (6.0 + sqrt2)).abs() <= f64::EPSILON * E32);
    }
}
