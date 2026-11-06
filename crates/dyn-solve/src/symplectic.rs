use crate::Method;

pub(crate) struct Splitting {
    kicks: &'static [f64],
    drifts: &'static [f64],
}

/// Yoshida's `w₁ = 1/(2 − 2^{1/3})`, written out so that every platform
/// steps with the same bits (no `cbrt` at run time).
const W1: f64 = 1.3512071919596575;
const W0: f64 = 1.0 - 2.0 * W1;
const OUTER_KICK: f64 = W1 / 2.0;
const INNER_KICK: f64 = (W0 + W1) / 2.0;

const VERLET: Splitting = Splitting {
    kicks: &[0.5, 0.5],
    drifts: &[1.0],
};

/// H. Yoshida, *Construction of higher order symplectic integrators*,
/// Phys. Lett. A 150, 1990; E. Forest & R. D. Ruth, Physica D 43, 1990.
const YOSHIDA4: Splitting = Splitting {
    kicks: &[OUTER_KICK, INNER_KICK, INNER_KICK, OUTER_KICK],
    drifts: &[W1, W0, W1],
};

pub(crate) fn splitting(method: Method) -> Option<&'static Splitting> {
    match method {
        Method::Verlet => Some(&VERLET),
        Method::Yoshida4 => Some(&YOSHIDA4),
        Method::Euler | Method::Heun | Method::Rk4 | Method::Dopri5 | Method::Rosenbrock23 => None,
    }
}

impl Splitting {
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
