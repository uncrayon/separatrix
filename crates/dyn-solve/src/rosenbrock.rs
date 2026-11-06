//! Rosenbrock 2(3), the L-stable linearly implicit scheme of MATLAB's
//! `ode23s` (L. F. Shampine & M. W. Reichelt, *The MATLAB ODE suite*, SIAM
//! J. Sci. Comput. 18, 1997, §3.1).

use std::f64::consts::SQRT_2;

use dyn_core::VectorField;

use crate::lu::{Lu, Singular};

pub(crate) const D: f64 = 1.0 / (2.0 + SQRT_2);
const E32: f64 = 6.0 + SQRT_2;

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

    pub(crate) fn error(&self, h: f64, i: usize) -> f64 {
        h / 6.0 * (self.k1[i] - 2.0 * self.k2[i] + self.k3[i])
    }

    /// The continuous extension of MATLAB's `ntrp23s`.
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
