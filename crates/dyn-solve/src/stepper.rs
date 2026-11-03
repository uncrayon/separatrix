//! One-step-at-a-time integrator with dense output.
//!
//! [`Stepper`] holds the current point `(t, x)`, the derivative there and
//! the next step size. Each call to [`Stepper::step`] advances by one
//! accepted step (an adaptive method may internally reject and retry) and
//! leaves behind an interpolant for that step, evaluated with
//! [`Stepper::interpolate`].
//!
//! Every interpolant is stored in the same five-vector form as Hairer's
//! `CONTD5`:
//!
//! ```text
//! x(t_prev + θh) = c0 + θ (c1 + (1-θ) (c2 + θ (c3 + (1-θ) c4)))
//! ```
//!
//! For Dopri5 this is the standard 4th-order continuous extension. For the
//! fixed-step methods `c4 = 0` and the formula is the cubic Hermite
//! interpolant through both end points with slopes `f` at both ends, with a
//! monotonicity limiter for badly resolved steps (see
//! [`limit_hermite_slopes`]).

use dyn_core::VectorField;

use crate::{all_finite, blew_up, Method, SolverOptions, Stats, Status};

/// Dormand–Prince 5(4) coefficients, as in Hairer's DOPRI5.
mod dp {
    pub const C2: f64 = 1.0 / 5.0;
    pub const C3: f64 = 3.0 / 10.0;
    pub const C4: f64 = 4.0 / 5.0;
    pub const C5: f64 = 8.0 / 9.0;

    pub const A21: f64 = 1.0 / 5.0;
    pub const A31: f64 = 3.0 / 40.0;
    pub const A32: f64 = 9.0 / 40.0;
    pub const A41: f64 = 44.0 / 45.0;
    pub const A42: f64 = -56.0 / 15.0;
    pub const A43: f64 = 32.0 / 9.0;
    pub const A51: f64 = 19372.0 / 6561.0;
    pub const A52: f64 = -25360.0 / 2187.0;
    pub const A53: f64 = 64448.0 / 6561.0;
    pub const A54: f64 = -212.0 / 729.0;
    pub const A61: f64 = 9017.0 / 3168.0;
    pub const A62: f64 = -355.0 / 33.0;
    pub const A63: f64 = 46732.0 / 5247.0;
    pub const A64: f64 = 49.0 / 176.0;
    pub const A65: f64 = -5103.0 / 18656.0;
    // The last row doubles as the 5th-order weights b_i (b2 = 0, b7 = 0).
    pub const A71: f64 = 35.0 / 384.0;
    pub const A73: f64 = 500.0 / 1113.0;
    pub const A74: f64 = 125.0 / 192.0;
    pub const A75: f64 = -2187.0 / 6784.0;
    pub const A76: f64 = 11.0 / 84.0;

    // Error weights e_i = b_i - b*_i (difference between orders 5 and 4).
    pub const E1: f64 = 71.0 / 57600.0;
    pub const E3: f64 = -71.0 / 16695.0;
    pub const E4: f64 = 71.0 / 1920.0;
    pub const E5: f64 = -17253.0 / 339200.0;
    pub const E6: f64 = 22.0 / 525.0;
    pub const E7: f64 = -1.0 / 40.0;

    // Dense output (4th-order continuous extension).
    pub const D1: f64 = -12715105075.0 / 11282082432.0;
    pub const D3: f64 = 87487479700.0 / 32700410799.0;
    pub const D4: f64 = -10690763975.0 / 1880347072.0;
    pub const D5: f64 = 701980252875.0 / 199316789632.0;
    pub const D6: f64 = -1453857185.0 / 822651844.0;
    pub const D7: f64 = 69997945.0 / 29380423.0;
}

/// Step-size controller constants (Hairer's DOPRI5 defaults).
mod ctrl {
    /// Safety factor applied to the optimal step.
    pub const SAFETY: f64 = 0.9;
    /// The step never shrinks by more than this factor at once.
    pub const FAC_MIN: f64 = 0.2;
    /// The step never grows by more than this factor at once.
    pub const FAC_MAX: f64 = 10.0;
    /// PI ("Lund") stabilisation exponent β.
    pub const BETA: f64 = 0.04;
    /// Proportional exponent: 1/5 - 0.75 β.
    pub const EXPO1: f64 = 0.2 - BETA * 0.75;
}

/// Steps with `|h| < MIN_REL_STEP * max(1, |t|)` are considered too small.
const MIN_REL_STEP: f64 = 1e-14;

/// Limit the end slopes of one cubic Hermite piece so that it cannot
/// overshoot (Fritsch & Carlson, *Monotone piecewise cubic interpolation*,
/// SIAM J. Numer. Anal. 17, 1980).
///
/// `m0`, `m1` are the derivatives at the two ends and `secant` is
/// `(x_new - x) / h`. Write `α = m0 / secant`, `β = m1 / secant`. When both
/// are positive (the data are monotone and so is the true solution), the
/// cubic is monotone exactly when `(α, β)` lies in the Fritsch–Carlson
/// region; if it does not, both slopes are scaled by `3 / |(α, β)|`, which
/// moves `(α, β)` onto the circle of radius 3 inside that region.
///
/// For a well-resolved step `α` and `β` are within `O(h)` of 1, so nothing
/// changes and the interpolant keeps its order. The limiter acts on steps
/// where the solution changes by orders of magnitude, most importantly the
/// last steps of a finite-time blow-up (`x' = x^2`): there `f(x_new)` is
/// far larger than the secant and the plain cubic swings to huge values of
/// the wrong sign inside the step.
///
/// A turning point inside the step (an end slope of the opposite sign to
/// the secant) or a zero secant is left alone.
fn limit_hermite_slopes(m0: f64, m1: f64, secant: f64) -> (f64, f64) {
    if secant == 0.0 {
        return (m0, m1);
    }
    let (alpha, beta) = (m0 / secant, m1 / secant);
    if !(alpha >= 0.0 && beta >= 0.0) || hermite_is_monotone(alpha, beta) {
        return (m0, m1);
    }
    let tau = 3.0 / alpha.hypot(beta);
    (tau * m0, tau * m1)
}

/// The Fritsch–Carlson monotonicity region for `α, β >= 0`. Returns `false`
/// when the test overflows to NaN (only for absurdly large `α` or `β`, which
/// are far outside the region anyway).
fn hermite_is_monotone(alpha: f64, beta: f64) -> bool {
    alpha + beta <= 2.0
        || 2.0 * alpha + beta <= 3.0
        || alpha + 2.0 * beta <= 3.0
        || alpha - (2.0 * alpha + beta - 3.0).powi(2) / (3.0 * (alpha + beta - 2.0)) >= 0.0
}

/// `v / w`, but `0` when `v == 0` (also when `w == 0`).
///
/// Used for the scaled error norm: with `atol = 0` a state component that
/// is exactly zero has weight `w = 0`, and its (exactly zero) error must
/// count as zero instead of `0 / 0 = NaN`.
fn scaled(v: f64, w: f64) -> f64 {
    if v == 0.0 {
        0.0
    } else {
        v / w
    }
}

/// Result of one call to [`Stepper::step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// A step was accepted. [`Stepper::t`] / [`Stepper::x`] are the new
    /// point and [`Stepper::interpolate`] covers `[t_prev, t]`.
    Accepted,
    /// A step was accepted, but the new state exceeds
    /// [`BLOW_UP_THRESHOLD`](crate::BLOW_UP_THRESHOLD). Stop integrating.
    BlewUp,
    /// Already at `t_end`; nothing was done.
    AtEnd,
    /// No step could be taken. The state is unchanged. The status is
    /// [`Status::StepTooSmall`], [`Status::NonFinite`], or
    /// [`Status::BlewUp`] when the adaptive step size collapsed because the
    /// solution runs off to infinity faster than double precision can
    /// follow (see [`Stepper::step`]).
    Failed(Status),
}

/// An integrator that advances one step at a time.
///
/// ```
/// use dyn_core::{FnField, SystemKind};
/// use dyn_solve::{SolverOptions, StepOutcome, Stepper};
///
/// let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -x[0]);
/// let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 1.0, &SolverOptions::default());
/// while s.step() == StepOutcome::Accepted {
///     // look at s.t(), s.x(), or s.interpolate(..) inside the step
/// }
/// assert_eq!(s.t(), 1.0);
/// assert!((s.x()[0] - (-1.0f64).exp()).abs() < 1e-6);
/// ```
pub struct Stepper<'a> {
    f: &'a dyn VectorField,
    p: &'a [f64],
    method: Method,
    rtol: f64,
    atol: f64,
    /// Largest |h| allowed (`INFINITY` if unlimited).
    h_max: f64,

    /// +1 forwards in time, -1 backwards.
    dir: f64,
    t_end: f64,

    /// Current point and the derivative there.
    t: f64,
    x: Vec<f64>,
    fx: Vec<f64>,
    /// Signed size of the next step to try.
    h: f64,

    /// The last accepted step went from `t_prev` to `t` with size `h_last`.
    t_prev: f64,
    h_last: f64,
    /// Interpolation coefficients `c0..c4`, each of length `dim`, back to back.
    cont: Vec<f64>,
    has_dense: bool,

    /// Adaptive controller memory.
    err_old: f64,
    last_rejected: bool,

    /// State and derivative at the start of the last accepted step (at
    /// `t_prev`). Used to tell a finite-time blow-up from other
    /// singularities when the step size collapses.
    x_prev: Vec<f64>,
    f_prev: Vec<f64>,

    /// Scratch space.
    k: [Vec<f64>; 5],
    x_stage: Vec<f64>,
    x_new: Vec<f64>,
    f_new: Vec<f64>,

    stats: Stats,
}

/// Call `f` and count the evaluation.
fn eval(f: &dyn VectorField, p: &[f64], stats: &mut Stats, t: f64, x: &[f64], out: &mut [f64]) {
    stats.evals += 1;
    f.eval(t, x, p, out);
}

impl<'a> Stepper<'a> {
    /// Start at `(t0, x0)`, integrating towards `t_end` (which may be
    /// smaller than `t0`). Evaluates `f(t0, x0)` once, plus once more for
    /// Dopri5 with automatic initial step (`opts.h == 0`).
    ///
    /// # Panics
    /// If `x0.len() != f.dim()`.
    pub fn new(
        f: &'a dyn VectorField,
        p: &'a [f64],
        t0: f64,
        x0: &[f64],
        t_end: f64,
        opts: &SolverOptions,
    ) -> Self {
        let n = f.dim();
        assert_eq!(
            x0.len(),
            n,
            "x0 has length {} but the system has dimension {}",
            x0.len(),
            n
        );
        let h_max = if opts.h_max > 0.0 && opts.h_max.is_finite() {
            opts.h_max
        } else {
            f64::INFINITY
        };
        let dir = if t_end < t0 { -1.0 } else { 1.0 };

        let mut s = Self {
            f,
            p,
            method: opts.method,
            rtol: opts.rtol.abs(),
            atol: opts.atol.abs(),
            h_max,
            dir,
            t_end,
            t: t0,
            x: x0.to_vec(),
            fx: vec![0.0; n],
            h: 0.0,
            t_prev: t0,
            h_last: 0.0,
            cont: vec![0.0; 5 * n],
            has_dense: false,
            err_old: 1e-4,
            last_rejected: false,
            x_prev: x0.to_vec(),
            f_prev: vec![0.0; n],
            k: std::array::from_fn(|_| vec![0.0; n]),
            x_stage: vec![0.0; n],
            x_new: vec![0.0; n],
            f_new: vec![0.0; n],
            stats: Stats::default(),
        };
        eval(f, p, &mut s.stats, t0, &s.x, &mut s.fx);

        let user_h = opts.h.abs();
        let h = if user_h > 0.0 && user_h.is_finite() {
            user_h
        } else if s.method == Method::Dopri5 {
            if s.is_done() {
                0.0
            } else {
                s.initial_step()
            }
        } else {
            let span = (t_end - t0).abs();
            if span > 0.0 && span.is_finite() {
                span / 1000.0
            } else {
                1e-3
            }
        };
        s.h = dir * h.min(h_max);
        s
    }

    /// Current time.
    pub fn t(&self) -> f64 {
        self.t
    }

    /// Current state.
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// `f(t, x)` at the current point.
    pub fn dxdt(&self) -> &[f64] {
        &self.fx
    }

    /// Start of the last accepted step (equals [`t`](Self::t) before the first step).
    pub fn t_prev(&self) -> f64 {
        self.t_prev
    }

    /// Signed size of the last accepted step (0 before the first step).
    pub fn h_last(&self) -> f64 {
        self.h_last
    }

    /// Signed size of the next step that will be tried.
    pub fn h_next(&self) -> f64 {
        self.h
    }

    /// Where integration stops.
    pub fn t_end(&self) -> f64 {
        self.t_end
    }

    /// The method in use.
    pub fn method(&self) -> Method {
        self.method
    }

    /// Dimension of the state.
    pub fn dim(&self) -> usize {
        self.x.len()
    }

    /// Work counters so far.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// `true` once `t` has reached `t_end`.
    pub fn is_done(&self) -> bool {
        (self.t_end - self.t) * self.dir <= 0.0
    }

    /// Move the end point (for example to keep integrating). If the new end
    /// lies on the other side of `t`, the direction is flipped.
    pub fn set_t_end(&mut self, t_end: f64) {
        self.t_end = t_end;
        if t_end != self.t {
            self.dir = if t_end < self.t { -1.0 } else { 1.0 };
            self.h = self.dir * self.h.abs();
        }
    }

    /// Replace the current point, e.g. to renormalise a perturbation when
    /// computing Lyapunov exponents. Keeps the step size, re-evaluates `f`
    /// (one evaluation) and forgets the dense output of the last step.
    ///
    /// # Panics
    /// If `x.len() != self.dim()`.
    pub fn set_state(&mut self, t: f64, x: &[f64]) {
        assert_eq!(x.len(), self.dim());
        self.t = t;
        self.t_prev = t;
        self.h_last = 0.0;
        self.x.copy_from_slice(x);
        eval(self.f, self.p, &mut self.stats, t, &self.x, &mut self.fx);
        self.has_dense = false;
        self.last_rejected = false;
        self.x_prev.copy_from_slice(x);
        self.f_prev.copy_from_slice(&self.fx);
    }

    /// Evaluate the interpolant of the last accepted step at time `t`,
    /// writing the state into `out`.
    ///
    /// Accurate for `t` between [`t_prev`](Self::t_prev) and
    /// [`t`](Self::t); outside that interval it extrapolates. Before any
    /// step (or after [`set_state`](Self::set_state)) it returns the
    /// current state.
    pub fn interpolate(&self, t: f64, out: &mut [f64]) {
        let n = self.dim();
        if !self.has_dense || self.h_last == 0.0 {
            out.copy_from_slice(&self.x);
            return;
        }
        let theta = (t - self.t_prev) / self.h_last;
        let theta1 = 1.0 - theta;
        let c = &self.cont;
        for i in 0..n {
            out[i] = c[i]
                + theta
                    * (c[n + i]
                        + theta1 * (c[2 * n + i] + theta * (c[3 * n + i] + theta1 * c[4 * n + i])));
        }
    }

    /// Take one step towards `t_end`. See [`StepOutcome`].
    ///
    /// When the Dopri5 step size collapses (below `1e-14 · max(1, |t|)`),
    /// the outcome is normally `Failed(StepTooSmall)`. It is
    /// `Failed(BlewUp)` instead when the last accepted steps look like a
    /// finite-time blow-up (see [`Stepper::is_blowing_up`]): solutions such
    /// as `x' = x^3` or `x' = e^x` reach infinity so fast that `|x|` cannot
    /// get anywhere near [`BLOW_UP_THRESHOLD`](crate::BLOW_UP_THRESHOLD)
    /// before the steps become too small for double precision.
    pub fn step(&mut self) -> StepOutcome {
        if self.is_done() {
            return StepOutcome::AtEnd;
        }
        if !all_finite(&self.x) || !all_finite(&self.fx) {
            return StepOutcome::Failed(Status::NonFinite);
        }
        match self.method {
            Method::Dopri5 => self.step_adaptive(),
            _ => self.step_fixed(),
        }
    }

    fn too_small(&self, h: f64) -> bool {
        h.is_nan() || h.abs() < MIN_REL_STEP * self.t.abs().max(1.0)
    }

    /// Does the last accepted step look like a finite-time blow-up?
    ///
    /// Meant to be called when the step size has collapsed, i.e. the
    /// solution runs into a singularity at some time `t*`. There are two
    /// kinds, and the last step tells them apart:
    ///
    /// * **blow-up**: `|x| → ∞`. Then `|f|` grows like a modest power of
    ///   `|x|` (`x' = x^3`: `|f| = |x|^3`; `x' = e^x`: `ln|f| = x ≈ 31` when
    ///   the steps collapse).
    /// * **finite limit**: `x → c` with `|f| → ∞` (`x' = 1/(2 - x)^3`
    ///   reaching 2, `x' = -1/x` reaching 0). Then `x` hardly changes while
    ///   `|f|` explodes.
    ///
    /// So for each component that moved away from 0 in the last step while
    /// its derivative grew, compute the *elasticity*
    /// `E = Δ ln|f_i| / Δ ln|x_i|` over the step. It is a blow-up if some
    /// such component has `E <= MAX_BLOW_UP_ELASTICITY` (1000); a finite
    /// limit `c` gives `E ≈ m |c| / |c - x|`, typically `10^4` or more.
    ///
    /// Always `false` before the first step.
    pub fn is_blowing_up(&self) -> bool {
        /// Largest elasticity `d ln|f| / d ln|x|` still counted as blow-up.
        const MAX_BLOW_UP_ELASTICITY: f64 = 1000.0;
        if !self.has_dense {
            return false;
        }
        (0..self.dim()).any(|i| {
            let (x0, x1) = (self.x_prev[i], self.x[i]);
            let (f0, f1) = (self.f_prev[i].abs(), self.fx[i].abs());
            // Moving away from zero (without crossing it), derivative growing.
            let outward = x0 * x1 > 0.0 && x1.abs() > x0.abs() && f1 > f0 && f0 > 0.0;
            if !outward {
                return false;
            }
            let elasticity = (f1 / f0).ln() / (x1 / x0).ln();
            elasticity <= MAX_BLOW_UP_ELASTICITY
        })
    }

    /// Hairer's `HINIT`: guess a starting step from the size of `x`, `f`
    /// and a finite-difference estimate of the second derivative.
    fn initial_step(&mut self) -> f64 {
        const ORDER: f64 = 5.0;
        let n = self.dim();
        let sk = |xi: f64, atol: f64, rtol: f64| atol + rtol * xi.abs();

        let (mut dnf, mut dny) = (0.0, 0.0);
        for i in 0..n {
            let w = sk(self.x[i], self.atol, self.rtol);
            dnf += scaled(self.fx[i], w).powi(2);
            dny += scaled(self.x[i], w).powi(2);
        }
        let mut h = if dnf <= 1e-10 || dny <= 1e-10 {
            1e-6
        } else {
            (dny / dnf).sqrt() * 0.01
        };
        h = h.min(self.h_max);

        // One explicit Euler step to estimate the second derivative.
        let hs = self.dir * h;
        for i in 0..n {
            self.x_stage[i] = self.x[i] + hs * self.fx[i];
        }
        eval(
            self.f,
            self.p,
            &mut self.stats,
            self.t + hs,
            &self.x_stage,
            &mut self.f_new,
        );
        let mut der2 = 0.0;
        for i in 0..n {
            let w = sk(self.x[i], self.atol, self.rtol);
            der2 += scaled(self.f_new[i] - self.fx[i], w).powi(2);
        }
        let der2 = der2.sqrt() / h;
        let der12 = der2.max(dnf.sqrt());
        let h1 = if der12 <= 1e-15 {
            (h * 1e-3).max(1e-6)
        } else {
            (0.01 / der12).powf(1.0 / ORDER)
        };
        let h = (100.0 * h).min(h1).min(self.h_max);
        if h.is_finite() && h > 0.0 {
            h
        } else {
            1e-6
        }
    }

    /// Commit `x_new` / `f_new` as the new point after an accepted step of size `h`.
    fn accept(&mut self, t_new: f64, h: f64) -> StepOutcome {
        self.x_prev.copy_from_slice(&self.x);
        self.f_prev.copy_from_slice(&self.fx);
        self.t_prev = self.t;
        self.t = t_new;
        self.h_last = h;
        std::mem::swap(&mut self.x, &mut self.x_new);
        std::mem::swap(&mut self.fx, &mut self.f_new);
        self.has_dense = true;
        self.stats.accepted += 1;
        if blew_up(&self.x) {
            StepOutcome::BlewUp
        } else {
            StepOutcome::Accepted
        }
    }

    /// Fill `cont[0..4n]` with the cubic Hermite interpolant between
    /// `(x, fx)` and `(x_new, f_new)`, and zero `cont[4n..]`.
    ///
    /// The end slopes go through [`limit_hermite_slopes`] so that a badly
    /// resolved step (such as the last step of a blow-up) cannot produce
    /// samples far outside the range of its end points.
    fn hermite_dense(&mut self, h: f64) {
        let n = self.dim();
        let f_new_ok = all_finite(&self.f_new);
        let c = &mut self.cont;
        for i in 0..n {
            let ydiff = self.x_new[i] - self.x[i];
            c[i] = self.x[i];
            c[n + i] = ydiff;
            if f_new_ok {
                let (m0, m1) = limit_hermite_slopes(self.fx[i], self.f_new[i], ydiff / h);
                let bspl = h * m0 - ydiff;
                c[2 * n + i] = bspl;
                c[3 * n + i] = ydiff - h * m1 - bspl;
            } else {
                // Only happens on the final step of a blow-up: fall back to
                // linear interpolation rather than producing NaN.
                c[2 * n + i] = 0.0;
                c[3 * n + i] = 0.0;
            }
            c[4 * n + i] = 0.0;
        }
    }

    // ----------------------------------------------------------------- fixed

    fn step_fixed(&mut self) -> StepOutcome {
        let mut h = self.h;
        if self.too_small(h) {
            return StepOutcome::Failed(Status::StepTooSmall);
        }
        // Shorten (or very slightly stretch) the last step to land exactly on t_end.
        let remaining = self.t_end - self.t;
        let last = remaining.abs() <= h.abs() * (1.0 + 1e-9);
        if last {
            h = remaining;
        }
        let t_new = if last { self.t_end } else { self.t + h };

        self.fixed_stages(h);
        if !all_finite(&self.x_new) {
            return StepOutcome::Failed(Status::NonFinite);
        }
        eval(
            self.f,
            self.p,
            &mut self.stats,
            t_new,
            &self.x_new,
            &mut self.f_new,
        );
        if !all_finite(&self.f_new) && !blew_up(&self.x_new) {
            return StepOutcome::Failed(Status::NonFinite);
        }
        self.hermite_dense(h);
        self.accept(t_new, h)
    }

    /// Compute `x_new` from `(t, x, fx)` with one step of the fixed-step method.
    fn fixed_stages(&mut self, h: f64) {
        let Self {
            f,
            p,
            t,
            x,
            fx: k1,
            k,
            x_stage: xs,
            x_new,
            stats,
            method,
            ..
        } = self;
        let (f, p, t) = (*f, *p, *t);
        let n = x.len();
        let [k2, k3, k4, ..] = k;
        match method {
            Method::Euler => {
                for i in 0..n {
                    x_new[i] = x[i] + h * k1[i];
                }
            }
            Method::Heun => {
                for i in 0..n {
                    xs[i] = x[i] + h * k1[i];
                }
                eval(f, p, stats, t + h, xs, k2);
                for i in 0..n {
                    x_new[i] = x[i] + 0.5 * h * (k1[i] + k2[i]);
                }
            }
            Method::Rk4 => {
                for i in 0..n {
                    xs[i] = x[i] + 0.5 * h * k1[i];
                }
                eval(f, p, stats, t + 0.5 * h, xs, k2);
                for i in 0..n {
                    xs[i] = x[i] + 0.5 * h * k2[i];
                }
                eval(f, p, stats, t + 0.5 * h, xs, k3);
                for i in 0..n {
                    xs[i] = x[i] + h * k3[i];
                }
                eval(f, p, stats, t + h, xs, k4);
                for i in 0..n {
                    x_new[i] = x[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
                }
            }
            Method::Dopri5 => unreachable!("Dopri5 is handled by step_adaptive"),
        }
    }

    // -------------------------------------------------------------- adaptive

    fn step_adaptive(&mut self) -> StepOutcome {
        use ctrl::*;
        // Set when the last rejection was caused by NaN/inf, so that a
        // collapse of the step size is reported as NonFinite.
        let mut nonfinite = false;
        loop {
            let mut h = self.h;
            if self.too_small(h) {
                let status = if nonfinite {
                    Status::NonFinite
                } else if self.is_blowing_up() {
                    Status::BlewUp
                } else {
                    Status::StepTooSmall
                };
                return StepOutcome::Failed(status);
            }
            // If we would land within 1% of t_end, go exactly there instead
            // (avoids a tiny final step).
            let last = (self.t + 1.01 * h - self.t_end) * self.dir >= 0.0;
            if last {
                h = self.t_end - self.t;
            }
            let t_new = if last { self.t_end } else { self.t + h };

            let err = self.dopri5_stages(h, t_new);
            // NaN/inf in the stages. (An infinite `err` with finite stages,
            // e.g. a non-zero error on a component with weight 0 when
            // atol = 0, is an ordinary rejection handled below.)
            if err.is_nan() || !all_finite(&self.x_new) || !all_finite(&self.f_new) {
                self.stats.rejected += 1;
                self.last_rejected = true;
                nonfinite = true;
                self.h = h * FAC_MIN;
                continue;
            }
            nonfinite = false;

            // PI controller (Hairer's DOPRI5): fac = err^expo1 / err_old^beta.
            let fac11 = err.powf(EXPO1);
            let fac = fac11 / self.err_old.powf(BETA);
            let fac = (fac / SAFETY).clamp(1.0 / FAC_MAX, 1.0 / FAC_MIN);
            let mut h_new = h / fac;

            if err <= 1.0 {
                self.err_old = err.max(1e-4);
                if h_new.abs() > self.h_max {
                    h_new = self.dir * self.h_max;
                }
                if self.last_rejected {
                    // Do not grow right after a rejection.
                    h_new = self.dir * h_new.abs().min(h.abs());
                }
                self.last_rejected = false;
                self.dopri5_dense(h);
                // Keep the proposal of a full step even if this one was shortened to hit t_end.
                if !last || h_new.abs() > self.h.abs() {
                    self.h = h_new;
                }
                return self.accept(t_new, h);
            }
            self.stats.rejected += 1;
            self.last_rejected = true;
            self.h = h / (1.0 / FAC_MIN).min(fac11 / SAFETY);
        }
    }

    /// Compute stages k2..k7 (k7 = f at the new point, stored in `f_new`),
    /// the 5th-order solution `x_new`, and return the scaled error norm.
    fn dopri5_stages(&mut self, h: f64, t_new: f64) -> f64 {
        use dp::*;
        let Self {
            f,
            p,
            t,
            x,
            fx: k1,
            k,
            x_stage: ys,
            x_new,
            f_new: k7,
            stats,
            rtol,
            atol,
            ..
        } = self;
        let (f, p, t, rtol, atol) = (*f, *p, *t, *rtol, *atol);
        let n = x.len();
        let [k2, k3, k4, k5, k6] = k;

        for i in 0..n {
            ys[i] = x[i] + h * A21 * k1[i];
        }
        eval(f, p, stats, t + C2 * h, ys, k2);
        for i in 0..n {
            ys[i] = x[i] + h * (A31 * k1[i] + A32 * k2[i]);
        }
        eval(f, p, stats, t + C3 * h, ys, k3);
        for i in 0..n {
            ys[i] = x[i] + h * (A41 * k1[i] + A42 * k2[i] + A43 * k3[i]);
        }
        eval(f, p, stats, t + C4 * h, ys, k4);
        for i in 0..n {
            ys[i] = x[i] + h * (A51 * k1[i] + A52 * k2[i] + A53 * k3[i] + A54 * k4[i]);
        }
        eval(f, p, stats, t + C5 * h, ys, k5);
        for i in 0..n {
            ys[i] =
                x[i] + h * (A61 * k1[i] + A62 * k2[i] + A63 * k3[i] + A64 * k4[i] + A65 * k5[i]);
        }
        eval(f, p, stats, t_new, ys, k6);
        for i in 0..n {
            x_new[i] =
                x[i] + h * (A71 * k1[i] + A73 * k3[i] + A74 * k4[i] + A75 * k5[i] + A76 * k6[i]);
        }
        // FSAL: this is also k1 of the next step.
        eval(f, p, stats, t_new, x_new, k7);

        // Hairer's norm: sqrt(mean((err_i / sk_i)^2)), sk_i = atol + rtol * max(|x_i|, |x_new_i|).
        let mut sum = 0.0;
        for i in 0..n {
            let e =
                h * (E1 * k1[i] + E3 * k3[i] + E4 * k4[i] + E5 * k5[i] + E6 * k6[i] + E7 * k7[i]);
            let sk = atol + rtol * x[i].abs().max(x_new[i].abs());
            sum += scaled(e, sk).powi(2);
        }
        if n == 0 {
            0.0
        } else {
            (sum / n as f64).sqrt()
        }
    }

    /// Dense-output coefficients for the accepted Dopri5 step of size `h`
    /// (before `x_new` / `f_new` are swapped in).
    fn dopri5_dense(&mut self, h: f64) {
        use dp::*;
        let n = self.dim();
        let (x, k1, k7) = (&self.x, &self.fx, &self.f_new);
        let [_, k3, k4, k5, k6] = &self.k;
        let c = &mut self.cont;
        for i in 0..n {
            let ydiff = self.x_new[i] - x[i];
            let bspl = h * k1[i] - ydiff;
            c[i] = x[i];
            c[n + i] = ydiff;
            c[2 * n + i] = bspl;
            c[3 * n + i] = ydiff - h * k7[i] - bspl;
            c[4 * n + i] =
                h * (D1 * k1[i] + D3 * k3[i] + D4 * k4[i] + D5 * k5[i] + D6 * k6[i] + D7 * k7[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dyn_core::{FnField, SystemKind};

    fn decay() -> FnField {
        FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -x[0])
    }

    fn opts(method: Method, h: f64) -> SolverOptions {
        SolverOptions {
            method,
            h,
            ..SolverOptions::default()
        }
    }

    #[test]
    fn tableau_rows_sum_to_c() {
        use dp::*;
        let close = |a: f64, b: f64| assert!((a - b).abs() < 1e-15, "{a} vs {b}");
        close(A21, C2);
        close(A31 + A32, C3);
        close(A41 + A42 + A43, C4);
        close(A51 + A52 + A53 + A54, C5);
        close(A61 + A62 + A63 + A64 + A65, 1.0);
        close(A71 + A73 + A74 + A75 + A76, 1.0);
        close(E1 + E3 + E4 + E5 + E6 + E7, 0.0);
        close(D1 + D3 + D4 + D5 + D6 + D7, 0.0);
    }

    #[test]
    fn hermite_matches_cubic_exactly() {
        // x' = 3t^2 has the cubic x = t^3, which Hermite reproduces exactly;
        // RK4 is exact for it as well.
        let f = FnField::new(1, 0, SystemKind::Ode, |t, _x, _p, out| out[0] = 3.0 * t * t)
            .non_autonomous();
        let mut s = Stepper::new(&f, &[], 0.0, &[0.0], 1.0, &opts(Method::Rk4, 0.5));
        assert_eq!(s.step(), StepOutcome::Accepted);
        let mut out = [0.0];
        for tau in [0.0, 0.1, 0.25, 0.4, 0.5] {
            s.interpolate(tau, &mut out);
            assert!(
                (out[0] - tau * tau * tau).abs() < 1e-14,
                "{tau}: {}",
                out[0]
            );
        }
    }

    #[test]
    fn dopri5_dense_is_continuous_at_step_ends() {
        let f = decay();
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 5.0, &SolverOptions::default());
        let mut out = [0.0];
        let mut x_prev = s.x()[0];
        while s.step() == StepOutcome::Accepted {
            s.interpolate(s.t_prev(), &mut out);
            assert!((out[0] - x_prev).abs() < 1e-15);
            s.interpolate(s.t(), &mut out);
            assert!((out[0] - s.x()[0]).abs() < 1e-14);
            x_prev = s.x()[0];
        }
    }

    #[test]
    fn last_fixed_step_lands_exactly_on_end() {
        let f = decay();
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 1.05, &opts(Method::Euler, 0.1));
        let mut steps = 0;
        while s.step() == StepOutcome::Accepted {
            steps += 1;
        }
        assert_eq!(s.t(), 1.05);
        assert_eq!(steps, 11);
        assert!((s.h_last() - 0.05).abs() < 1e-12);
    }

    #[test]
    fn automatic_fixed_step_is_span_over_1000() {
        let f = decay();
        let s = Stepper::new(&f, &[], 0.0, &[1.0], -2.0, &opts(Method::Rk4, 0.0));
        assert!((s.h_next() + 0.002).abs() < 1e-15);
    }

    #[test]
    fn initial_step_is_reasonable_and_respects_h_max() {
        let f = decay();
        let s = Stepper::new(&f, &[], 0.0, &[1.0], 10.0, &SolverOptions::default());
        assert!(s.h_next() > 1e-4 && s.h_next() < 1.0, "{}", s.h_next());
        assert_eq!(s.stats().evals, 2);

        let o = SolverOptions {
            h_max: 1e-5,
            ..SolverOptions::default()
        };
        let s = Stepper::new(&f, &[], 0.0, &[1.0], -10.0, &o);
        assert_eq!(s.h_next(), -1e-5);
    }

    #[test]
    fn h_max_is_respected_by_the_controller() {
        let f = decay();
        let o = SolverOptions {
            h_max: 0.05,
            ..SolverOptions::default()
        };
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 3.0, &o);
        while s.step() == StepOutcome::Accepted {
            assert!(s.h_last().abs() <= 0.05 + 1e-15);
        }
        assert_eq!(s.t(), 3.0);
    }

    #[test]
    fn nan_in_rhs_is_reported() {
        // sqrt(x) leaves its domain when x crosses zero.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = -x[0].sqrt() - 1.0
        });
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 10.0, &SolverOptions::default());
        let outcome = loop {
            match s.step() {
                StepOutcome::Accepted => {}
                other => break other,
            }
        };
        assert_eq!(outcome, StepOutcome::Failed(Status::NonFinite));
        assert!(s.x()[0] >= 0.0 && s.x()[0] < 1e-3, "{}", s.x()[0]);

        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 10.0, &opts(Method::Rk4, 0.1));
        let outcome = loop {
            match s.step() {
                StepOutcome::Accepted => {}
                other => break other,
            }
        };
        assert_eq!(outcome, StepOutcome::Failed(Status::NonFinite));
    }

    #[test]
    fn set_state_and_set_t_end() {
        let f = decay();
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 1.0, &SolverOptions::default());
        while s.step() == StepOutcome::Accepted {}
        assert_eq!(s.step(), StepOutcome::AtEnd);
        s.set_state(1.0, &[2.0]);
        assert_eq!(s.dxdt(), &[-2.0]);
        s.set_t_end(2.0);
        while s.step() == StepOutcome::Accepted {}
        assert_eq!(s.t(), 2.0);
        assert!((s.x()[0] - 2.0 * (-1.0f64).exp()).abs() < 1e-6);
        // Turn around.
        s.set_t_end(1.0);
        assert!(s.h_next() < 0.0);
        while s.step() == StepOutcome::Accepted {}
        assert!((s.x()[0] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn hermite_limiter_only_touches_overshooting_pieces() {
        // Well resolved: slopes close to the secant are unchanged.
        assert_eq!(limit_hermite_slopes(1.0, 1.1, 1.05), (1.0, 1.1));
        // x = t^3 on [0, 1]: (α, β) = (0, 3) is on the boundary, unchanged.
        assert_eq!(limit_hermite_slopes(0.0, 3.0, 1.0), (0.0, 3.0));
        // Turning point inside the step (opposite signs): unchanged.
        assert_eq!(limit_hermite_slopes(-0.5, 4.0, 1.0), (-0.5, 4.0));
        // Zero secant: unchanged.
        assert_eq!(limit_hermite_slopes(1.0, -1.0, 0.0), (1.0, -1.0));
        // Blow-up step: the huge end slope is scaled onto |(α, β)| = 3.
        let (m0, m1) = limit_hermite_slopes(2.0, 1e20, 1e9);
        assert!((m1 / 1e9 - 3.0).abs() < 1e-12 && m0 > 0.0 && m0 < 2.0);
        // Negative direction works the same way.
        let (m0, m1) = limit_hermite_slopes(-2.0, -1e20, -1e9);
        assert!((m1 / -1e9 - 3.0).abs() < 1e-12 && m0 < 0.0);
        // Absurd ratios overflow inside the test but are still limited.
        let (_, m1) = limit_hermite_slopes(1.0, 1e300, 1e-10);
        assert!(m1.is_finite() && m1 <= 3.0 * 1e-10 * (1.0 + 1e-12));
    }

    #[test]
    fn monotonicity_region() {
        assert!(hermite_is_monotone(1.0, 1.0));
        assert!(hermite_is_monotone(3.0, 3.0)); // corner of the de Boor–Swartz box
        assert!(hermite_is_monotone(0.0, 3.0));
        assert!(!hermite_is_monotone(0.0, 3.5));
        assert!(!hermite_is_monotone(4.0, 4.0));
        assert!(!hermite_is_monotone(0.0, f64::MAX)); // NaN inside: not monotone
    }

    #[test]
    fn fixed_step_blow_up_interpolant_stays_between_end_points() {
        // x' = x^2 with Euler: the last step jumps over orders of magnitude.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0] * x[0]);
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 30.0, &opts(Method::Euler, 0.1));
        let mut out = [0.0];
        loop {
            let x0 = s.x()[0];
            let outcome = s.step();
            assert!(matches!(outcome, StepOutcome::Accepted | StepOutcome::BlewUp));
            let mut prev = x0;
            for k in 0..=100 {
                s.interpolate(s.t_prev() + s.h_last() * k as f64 / 100.0, &mut out);
                assert!(out[0] >= prev * (1.0 - 1e-15), "{} < {prev}", out[0]);
                prev = out[0];
            }
            if outcome == StepOutcome::BlewUp {
                break;
            }
        }
    }

    fn run(s: &mut Stepper) -> StepOutcome {
        loop {
            match s.step() {
                StepOutcome::Accepted => {}
                other => break other,
            }
        }
    }

    #[test]
    fn collapse_in_fast_blow_up_is_blew_up() {
        // x' = e^x from 0 blows up at t = 1 but never gets past x ≈ 32.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0].exp());
        let mut s = Stepper::new(&f, &[], 0.0, &[0.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::BlewUp));
        assert!(s.is_blowing_up());
        assert!((s.t() - 1.0).abs() < 1e-5, "{}", s.t());

        // Backwards in time: x' = -x^3 from x(0) = -1 blows up at t = -1/2.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -x[0].powi(3));
        let mut s = Stepper::new(&f, &[], 0.0, &[-1.0], -5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::BlewUp));
        assert!((s.t() + 0.5).abs() < 1e-5, "{}", s.t());

        // 2D: x' = y^2, y' = x^2 from (1, 1) (x = y, x' = x^2).
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = x[1] * x[1];
            out[1] = x[0] * x[0];
        });
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0, 1.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::BlewUp);
    }

    #[test]
    fn collapse_at_a_finite_singularity_is_step_too_small() {
        // x' = 1/(2 - x)^3 from 1: x reaches 2 with infinite slope at
        // t = 1/4. The solution stays bounded, so this is no blow-up.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = 1.0 / (2.0 - x[0]).powi(3)
        });
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::StepTooSmall));
        assert!(!s.is_blowing_up());
        assert!(s.x()[0] < 2.0 && s.x()[0] > 1.99);

        // x' = -1/x from 1: x = sqrt(1 - 2t) reaches 0 at t = 1/2.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -1.0 / x[0]);
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::StepTooSmall));
    }

    #[test]
    fn no_blow_up_before_the_first_step() {
        let f = decay();
        let s = Stepper::new(&f, &[], 0.0, &[1.0], 1.0, &SolverOptions::default());
        assert!(!s.is_blowing_up());
    }

    #[test]
    fn zero_atol_with_zero_components() {
        // Pure relative tolerance; the second component is identically 0.
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = -x[0];
            out[1] = -x[1];
        });
        let o = SolverOptions {
            atol: 0.0,
            ..SolverOptions::default()
        };
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0, 0.0], 1.0, &o);
        assert!(s.h_next().is_finite() && s.h_next() > 1e-6, "{}", s.h_next());
        assert_eq!(run(&mut s), StepOutcome::AtEnd);
        assert_eq!(s.x()[1], 0.0);
        assert!((s.x()[0] - (-1.0f64).exp()).abs() < 1e-5);
        assert_eq!(scaled(0.0, 0.0), 0.0);
        assert_eq!(scaled(1.0, 0.0), f64::INFINITY);
    }
}
