//! Forward-mode automatic differentiation.
//!
//! The compiled program is evaluated generically over [`Scalar`]. With
//! `f64` it computes values; with [`Dual<f64>`] it also computes one
//! directional derivative exactly (no finite-difference error); with
//! `Dual<Dual<f64>>` it computes second derivatives (used for the Jacobian
//! of Hamiltonian systems, whose field is itself a gradient).
//!
//! ```
//! use dyn_expr::ad::{Dual, Scalar};
//! // d/dx sin(x^2) at x = 1.5 is 2x cos(x^2).
//! let x = Dual::variable(1.5);
//! let y = (x * x).sin();
//! assert!((y.eps - 3.0 * (2.25f64).cos()).abs() < 1e-15);
//! ```

use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Number-like type the evaluator runs on: `f64` or a (nested) [`Dual`].
///
/// Every operation the instruction set needs is here. `min`, `max`, `abs`
/// and `sign` branch on the plain value ([`Scalar::re`]), so their
/// derivatives are the one-sided ones of the branch taken.
pub trait Scalar:
    Copy
    + Debug
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    /// A constant (zero derivative).
    fn cst(v: f64) -> Self;
    /// The plain `f64` value, without derivative parts.
    fn re(self) -> f64;
    /// True if the value and all derivative parts are exactly zero.
    fn is_zero(self) -> bool;
    /// Sine.
    fn sin(self) -> Self;
    /// Cosine.
    fn cos(self) -> Self;
    /// Tangent.
    fn tan(self) -> Self;
    /// Exponential `e^x`.
    fn exp(self) -> Self;
    /// Natural logarithm.
    fn ln(self) -> Self;
    /// Square root.
    fn sqrt(self) -> Self;
    /// Real cube root.
    fn cbrt(self) -> Self;
    /// Absolute value (derivative `sign(x)`, 0 at 0).
    fn abs(self) -> Self;
    /// `-1`, `0` or `1` (derivative zero).
    fn sign(self) -> Self;
    /// Hyperbolic sine.
    fn sinh(self) -> Self;
    /// Hyperbolic cosine.
    fn cosh(self) -> Self;
    /// Hyperbolic tangent.
    fn tanh(self) -> Self;
    /// Inverse sine.
    fn asin(self) -> Self;
    /// Inverse cosine.
    fn acos(self) -> Self;
    /// Inverse tangent.
    fn atan(self) -> Self;
    /// Integer power.
    fn powi(self, n: i32) -> Self;
    /// Power with a constant real exponent.
    fn powc(self, c: f64) -> Self;
    /// General power `self^e`.
    fn pow(self, e: Self) -> Self;
    /// Product `self · seed` for the chain rule, where an exactly-zero
    /// `seed` wins: `∞ · 0 = 0` and `NaN · 0 = 0` (for duals, part by part).
    ///
    /// A zero seed means the input does not change in the direction being
    /// differentiated, so the output does not change either, even where
    /// the function's own derivative is infinite (`sqrt` at 0, `arcsin` at
    /// 1). Plain `*` would give NaN there and poison the whole Jacobian.
    fn mul_seed(self, seed: Self) -> Self;
    /// Smaller of the two (by plain value).
    fn min(self, o: Self) -> Self {
        if o.re() < self.re() {
            o
        } else {
            self
        }
    }
    /// Larger of the two (by plain value).
    fn max(self, o: Self) -> Self {
        if o.re() > self.re() {
            o
        } else {
            self
        }
    }
}

/// Sign with `sign(0) = 0` (unlike `f64::signum`).
fn sign_f64(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        v // 0 or NaN
    }
}

impl Scalar for f64 {
    #[inline]
    fn cst(v: f64) -> Self {
        v
    }
    #[inline]
    fn re(self) -> f64 {
        self
    }
    #[inline]
    fn is_zero(self) -> bool {
        self == 0.0
    }
    #[inline]
    fn sin(self) -> Self {
        f64::sin(self)
    }
    #[inline]
    fn cos(self) -> Self {
        f64::cos(self)
    }
    #[inline]
    fn tan(self) -> Self {
        f64::tan(self)
    }
    #[inline]
    fn exp(self) -> Self {
        f64::exp(self)
    }
    #[inline]
    fn ln(self) -> Self {
        f64::ln(self)
    }
    #[inline]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    #[inline]
    fn cbrt(self) -> Self {
        f64::cbrt(self)
    }
    #[inline]
    fn abs(self) -> Self {
        f64::abs(self)
    }
    #[inline]
    fn sign(self) -> Self {
        sign_f64(self)
    }
    #[inline]
    fn sinh(self) -> Self {
        f64::sinh(self)
    }
    #[inline]
    fn cosh(self) -> Self {
        f64::cosh(self)
    }
    #[inline]
    fn tanh(self) -> Self {
        f64::tanh(self)
    }
    #[inline]
    fn asin(self) -> Self {
        f64::asin(self)
    }
    #[inline]
    fn acos(self) -> Self {
        f64::acos(self)
    }
    #[inline]
    fn atan(self) -> Self {
        f64::atan(self)
    }
    #[inline]
    fn powi(self, n: i32) -> Self {
        f64::powi(self, n)
    }
    #[inline]
    fn powc(self, c: f64) -> Self {
        f64::powf(self, c)
    }
    #[inline]
    fn pow(self, e: Self) -> Self {
        f64::powf(self, e)
    }
    #[inline]
    fn mul_seed(self, seed: Self) -> Self {
        if seed == 0.0 {
            0.0
        } else {
            self * seed
        }
    }
}

/// Dual number `re + eps·ε` with `ε² = 0`.
///
/// If the inputs carry derivatives in `eps` (a "seed", e.g. `eps = 1` on
/// the variable being differentiated), every operation propagates them by
/// the chain rule, so the output's `eps` is the exact derivative.
/// `T` may itself be a `Dual`, giving second derivatives.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dual<T> {
    /// Value.
    pub re: T,
    /// Derivative along the seeded direction.
    pub eps: T,
}

impl<T: Scalar> Dual<T> {
    /// Build from value and derivative.
    #[inline]
    pub fn new(re: T, eps: T) -> Self {
        Dual { re, eps }
    }

    /// A constant: derivative zero.
    #[inline]
    pub fn constant(re: T) -> Self {
        Dual { re, eps: T::cst(0.0) }
    }

    /// The variable being differentiated: derivative one.
    #[inline]
    pub fn variable(re: T) -> Self {
        Dual { re, eps: T::cst(1.0) }
    }

    /// Value `f(re)` with derivative `f'(re)·eps`. A zero `eps` gives a
    /// zero derivative even where `f'` is infinite (see
    /// [`Scalar::mul_seed`]).
    #[inline]
    fn chain(self, value: T, deriv: T) -> Self {
        Dual { re: value, eps: deriv.mul_seed(self.eps) }
    }
}

impl<T: Scalar> Add for Dual<T> {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        Dual { re: self.re + o.re, eps: self.eps + o.eps }
    }
}

impl<T: Scalar> Sub for Dual<T> {
    type Output = Self;
    #[inline]
    fn sub(self, o: Self) -> Self {
        Dual { re: self.re - o.re, eps: self.eps - o.eps }
    }
}

impl<T: Scalar> Mul for Dual<T> {
    type Output = Self;
    #[inline]
    fn mul(self, o: Self) -> Self {
        Dual { re: self.re * o.re, eps: self.eps * o.re + self.re * o.eps }
    }
}

impl<T: Scalar> Div for Dual<T> {
    type Output = Self;
    #[inline]
    fn div(self, o: Self) -> Self {
        let q = self.re / o.re;
        Dual { re: q, eps: (self.eps - q * o.eps) / o.re }
    }
}

impl<T: Scalar> Neg for Dual<T> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Dual { re: -self.re, eps: -self.eps }
    }
}

impl<T: Scalar> Scalar for Dual<T> {
    #[inline]
    fn cst(v: f64) -> Self {
        Dual::constant(T::cst(v))
    }
    #[inline]
    fn re(self) -> f64 {
        self.re.re()
    }
    #[inline]
    fn is_zero(self) -> bool {
        self.re.is_zero() && self.eps.is_zero()
    }
    #[inline]
    fn sin(self) -> Self {
        self.chain(self.re.sin(), self.re.cos())
    }
    #[inline]
    fn cos(self) -> Self {
        self.chain(self.re.cos(), -self.re.sin())
    }
    #[inline]
    fn tan(self) -> Self {
        let t = self.re.tan();
        self.chain(t, T::cst(1.0) + t * t)
    }
    #[inline]
    fn exp(self) -> Self {
        let e = self.re.exp();
        self.chain(e, e)
    }
    #[inline]
    fn ln(self) -> Self {
        self.chain(self.re.ln(), T::cst(1.0) / self.re)
    }
    #[inline]
    fn sqrt(self) -> Self {
        let s = self.re.sqrt();
        self.chain(s, T::cst(0.5) / s)
    }
    #[inline]
    fn cbrt(self) -> Self {
        let c = self.re.cbrt();
        self.chain(c, T::cst(1.0) / (T::cst(3.0) * c * c))
    }
    #[inline]
    fn abs(self) -> Self {
        self.chain(self.re.abs(), T::cst(sign_f64(self.re.re())))
    }
    #[inline]
    fn sign(self) -> Self {
        Dual::constant(self.re.sign())
    }
    #[inline]
    fn sinh(self) -> Self {
        self.chain(self.re.sinh(), self.re.cosh())
    }
    #[inline]
    fn cosh(self) -> Self {
        self.chain(self.re.cosh(), self.re.sinh())
    }
    #[inline]
    fn tanh(self) -> Self {
        let t = self.re.tanh();
        self.chain(t, T::cst(1.0) - t * t)
    }
    #[inline]
    fn asin(self) -> Self {
        let d = (T::cst(1.0) - self.re * self.re).sqrt();
        self.chain(self.re.asin(), T::cst(1.0) / d)
    }
    #[inline]
    fn acos(self) -> Self {
        let d = (T::cst(1.0) - self.re * self.re).sqrt();
        self.chain(self.re.acos(), T::cst(-1.0) / d)
    }
    #[inline]
    fn atan(self) -> Self {
        self.chain(self.re.atan(), T::cst(1.0) / (T::cst(1.0) + self.re * self.re))
    }
    #[inline]
    fn powi(self, n: i32) -> Self {
        if n == 0 {
            return Self::cst(1.0);
        }
        self.chain(self.re.powi(n), T::cst(f64::from(n)) * self.re.powi(n - 1))
    }
    #[inline]
    fn powc(self, c: f64) -> Self {
        self.chain(self.re.powc(c), T::cst(c) * self.re.powc(c - 1.0))
    }
    fn pow(self, e: Self) -> Self {
        let (a, b) = (self.re, e.re);
        let v = a.pow(b);
        // d(a^b) = b a^(b-1) da + a^b ln(a) db. Each term is skipped when
        // its seed is zero, so e.g. (-2)^b with constant b stays finite.
        let mut d = (b * a.pow(b - T::cst(1.0))).mul_seed(self.eps);
        // At a = 0 with b > 0, a^b = 0 for every b near this one, so the
        // b-derivative is 0 (the limit of a^b ln a), not 0 · (-∞) = NaN.
        if !(a.re() == 0.0 && b.re() > 0.0) {
            d = d + (v * a.ln()).mul_seed(e.eps);
        }
        Dual { re: v, eps: d }
    }
    #[inline]
    fn mul_seed(self, seed: Self) -> Self {
        // (a + a'ε)(b + b'ε) = ab + (a'b + ab')ε, each product zero-safe.
        Dual {
            re: self.re.mul_seed(seed.re),
            eps: self.eps.mul_seed(seed.re) + self.re.mul_seed(seed.eps),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type D = Dual<f64>;
    type D2 = Dual<Dual<f64>>;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * (1.0 + b.abs())
    }

    /// Check f'(x) by AD against central differences for a list of unary
    /// operations.
    #[test]
    fn unary_derivatives_match_finite_differences() {
        type F = fn(D) -> D;
        type G = fn(f64) -> f64;
        let cases: &[(&str, F, G, f64)] = &[
            ("sin", D::sin, f64::sin, 0.7),
            ("cos", D::cos, f64::cos, 0.7),
            ("tan", D::tan, f64::tan, 0.7),
            ("exp", D::exp, f64::exp, 0.7),
            ("ln", D::ln, f64::ln, 0.7),
            ("sqrt", D::sqrt, f64::sqrt, 0.7),
            ("cbrt", D::cbrt, f64::cbrt, -0.7),
            ("abs", D::abs, f64::abs, -0.7),
            ("sinh", D::sinh, f64::sinh, 0.7),
            ("cosh", D::cosh, f64::cosh, 0.7),
            ("tanh", D::tanh, f64::tanh, 0.7),
            ("asin", D::asin, f64::asin, 0.3),
            ("acos", D::acos, f64::acos, 0.3),
            ("atan", D::atan, f64::atan, 0.3),
        ];
        for &(name, f, g, x) in cases {
            let ad = f(D::variable(x)).eps;
            let h = 1e-6;
            let fd = (g(x + h) - g(x - h)) / (2.0 * h);
            assert!((ad - fd).abs() < 1e-7, "{name}: {ad} vs {fd}");
            assert!(close(f(D::variable(x)).re, g(x)), "{name} value");
        }
    }

    #[test]
    fn powers() {
        let x = D::variable(-1.3);
        assert!(close(x.powi(3).eps, 3.0 * 1.69));
        assert!(close(x.powi(0).eps, 0.0));
        assert!(close(D::variable(2.0).powc(0.5).eps, 0.5 / 2f64.sqrt()));
        // x^y with both seeded: d/dx + d/dy at (2, 3) = 3·4 + 8 ln 2.
        let v = D::new(2.0, 1.0).pow(D::new(3.0, 1.0));
        assert!(close(v.eps, 12.0 + 8.0 * 2f64.ln()));
        // Negative base, constant exponent: no NaN from ln.
        let w = D::variable(-2.0).pow(D::constant(2.0));
        assert!(close(w.eps, -4.0), "{w:?}");
    }

    #[test]
    fn second_derivative_with_nested_duals() {
        // f(x) = x^3 sin x; f'' = 6x sin x + 6x^2 cos x - x^3 sin x.
        let x0 = 0.9;
        let x = D2::new(D::new(x0, 1.0), D::new(1.0, 0.0));
        let f = x.powi(3) * x.sin();
        let exact = 6.0 * x0 * x0.sin() + 6.0 * x0 * x0 * x0.cos() - x0.powi(3) * x0.sin();
        assert!(close(f.eps.eps, exact), "{} vs {exact}", f.eps.eps);
        assert!(close(f.eps.re, f.re.eps));
    }

    /// Where a function's own derivative is infinite (sqrt, cbrt and
    /// fractional powers at 0, arcsin at ±1), a zero seed still gives a
    /// zero derivative instead of NaN; a non-zero seed gives ±∞.
    #[test]
    fn zero_seed_beats_infinite_derivative() {
        let still = D::constant(0.0);
        assert_eq!(still.sqrt().eps, 0.0);
        assert_eq!(still.cbrt().eps, 0.0);
        assert_eq!(still.powc(1.0 / 3.0).eps, 0.0);
        assert_eq!(D::constant(1.0).asin().eps, 0.0);
        assert_eq!(D::constant(-1.0).acos().eps, 0.0);
        assert_eq!(still.ln().eps, 0.0);
        assert_eq!(D::variable(0.0).sqrt().eps, f64::INFINITY);
        // Second order: the inner duals are all zero too.
        let still2 = D2::constant(D::constant(0.0));
        let s = still2.sqrt();
        assert_eq!((s.eps.re, s.eps.eps), (0.0, 0.0));
    }

    /// sqrt(x² + y²) at the origin, differentiated through x·(1 − r): the
    /// inner sum has zero derivative, so r contributes nothing.
    #[test]
    fn radius_at_origin_has_finite_product_derivative() {
        let x = D::variable(0.0);
        let y = D::constant(0.0);
        let r = (x * x + y * y).sqrt();
        let f = x * (D::cst(1.0) - r);
        assert_eq!(f.eps, 1.0);
    }

    /// d/db a^b at a = 0 (b > 0) is the limit of a^b ln a, which is 0.
    #[test]
    fn power_exponent_derivative_at_zero_base() {
        let v = D::constant(0.0).pow(D::variable(2.0));
        assert_eq!((v.re, v.eps), (0.0, 0.0));
        let v = D::constant(0.0).pow(D::variable(0.5));
        assert_eq!(v.eps, 0.0);
        // Base derivative at 0 is still exact for b ≥ 1.
        let v = D::variable(0.0).pow(D::constant(2.0));
        assert_eq!(v.eps, 0.0);
        let v = D::variable(0.0).pow(D::constant(1.0));
        assert_eq!(v.eps, 1.0);
    }

    #[test]
    fn mul_seed() {
        assert_eq!(f64::INFINITY.mul_seed(0.0), 0.0);
        assert_eq!(f64::NAN.mul_seed(0.0), 0.0);
        assert_eq!(2.0f64.mul_seed(3.0), 6.0);
        assert!(f64::INFINITY.mul_seed(1.0).is_infinite());
        let a = D::new(f64::INFINITY, 2.0);
        let b = D::new(0.0, 3.0);
        // (∞ + 2ε)(0 + 3ε) = 0 + (2·0 + ∞·3)ε
        let p = a.mul_seed(b);
        assert_eq!(p.re, 0.0);
        assert!(p.eps.is_infinite());
        assert_eq!(D::new(f64::NAN, f64::NAN).mul_seed(D::constant(0.0)), D::constant(0.0));
    }

    #[test]
    fn min_max_sign() {
        let a = D::new(1.0, 2.0);
        let b = D::new(3.0, 5.0);
        assert_eq!(a.min(b), a);
        assert_eq!(a.max(b), b);
        assert_eq!(D::variable(0.0).sign(), D::constant(0.0));
        assert_eq!(D::variable(-4.0).sign(), D::constant(-1.0));
    }
}
