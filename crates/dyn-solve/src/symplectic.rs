//! Symplectic split-step methods for a separable `H(q, p) = T(p) + V(q)`.
//!
//! The state is laid out `[q; p]` with `dof = dim / 2`, as `dyn-expr`
//! builds a Hamiltonian field: `f = (∂H/∂p, −∂H/∂q)`. A step alternates
//! kicks `p += c·h·ṗ(q)` and drifts `q += d·h·q̇(p)`. For a separable `H`
//! the `ṗ` half of `f(q, ·)` does not depend on `p` and the `q̇` half of
//! `f(·, p)` does not depend on `q`, so each half can be read from one
//! evaluation of the whole field. The caller checks that `H` separates;
//! these functions do not.

use crate::Method;

/// A symmetric composition `kick c₀, drift d₀, kick c₁, …, drift d_{m−1},
/// kick c_m`, the coefficients in units of the step `h`.
pub(crate) struct Splitting {
    kicks: &'static [f64],
    drifts: &'static [f64],
}

/// Yoshida's `w₁ = 1/(2 − 2^{1/3})`, written out so that every platform
/// steps with the same bits (no `cbrt` at run time).
const W1: f64 = 1.3512071919596575;
/// `w₀ = 1 − 2w₁`.
const W0: f64 = -1.7024143839193153;
/// `w₁/2`.
const OUTER_KICK: f64 = 0.6756035959798288;
/// `(w₀ + w₁)/2`.
const INNER_KICK: f64 = -0.17560359597982883;

/// Störmer–Verlet (leapfrog), kick–drift–kick: order 2.
const VERLET: Splitting = Splitting {
    kicks: &[0.5, 0.5],
    drifts: &[1.0],
};

/// Yoshida's fourth-order composition of three Verlet steps of sizes
/// `w₁h, w₀h, w₁h` (H. Yoshida, *Construction of higher order symplectic
/// integrators*, Phys. Lett. A 150, 1990; E. Forest & R. D. Ruth, Physica D
/// 43, 1990).
const YOSHIDA4: Splitting = Splitting {
    kicks: &[OUTER_KICK, INNER_KICK, INNER_KICK, OUTER_KICK],
    drifts: &[W1, W0, W1],
};

/// The splitting of a symplectic method; `None` for the others.
pub(crate) fn splitting(method: Method) -> Option<&'static Splitting> {
    match method {
        Method::Verlet => Some(&VERLET),
        Method::Yoshida4 => Some(&YOSHIDA4),
        Method::Euler | Method::Heun | Method::Rk4 | Method::Dopri5 => None,
    }
}

impl Splitting {
    /// One step of size `h` from `(t, x)`, where `fx = f(t, x)`, into
    /// `x_new`. Calls `eval(t, x, out)` twice per drift; `scratch` holds
    /// its output.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step(
        &self,
        t: f64,
        h: f64,
        x: &[f64],
        fx: &[f64],
        scratch: &mut [f64],
        x_new: &mut [f64],
        mut eval: impl FnMut(f64, &[f64], &mut [f64]),
    ) {
        let dof = x.len() / 2;
        x_new.copy_from_slice(x);
        for i in dof..2 * dof {
            x_new[i] += self.kicks[0] * h * fx[i];
        }
        let mut tau = 0.0;
        for (&d, &c) in self.drifts.iter().zip(&self.kicks[1..]) {
            eval(t + tau * h, x_new, scratch);
            for i in 0..dof {
                x_new[i] += d * h * scratch[i];
            }
            tau += d;
            eval(t + tau * h, x_new, scratch);
            for i in dof..2 * dof {
                x_new[i] += c * h * scratch[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficients_are_yoshidas() {
        let close = |a: f64, b: f64| assert!((a - b).abs() <= 4.0 * f64::EPSILON, "{a} vs {b}");
        close(W1, 1.0 / (2.0 - 2f64.powf(1.0 / 3.0)));
        close(W0, 1.0 - 2.0 * W1);
        close(OUTER_KICK, W1 / 2.0);
        close(INNER_KICK, (W0 + W1) / 2.0);
        for s in [&VERLET, &YOSHIDA4] {
            close(s.kicks.iter().sum(), 1.0);
            close(s.drifts.iter().sum(), 1.0);
            assert_eq!(s.kicks.len(), s.drifts.len() + 1);
        }
    }
}
