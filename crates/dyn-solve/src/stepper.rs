use dyn_core::VectorField;

use crate::{all_finite, blew_up, Method, SolverOptions, Stats, Status};

/// Dormand–Prince 5(4) coefficients, as in Hairer's DOPRI5.
mod dormand_prince {
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
    pub const A71: f64 = 35.0 / 384.0;
    pub const A73: f64 = 500.0 / 1113.0;
    pub const A74: f64 = 125.0 / 192.0;
    pub const A75: f64 = -2187.0 / 6784.0;
    pub const A76: f64 = 11.0 / 84.0;

    /// Fifth-order weights. The method is FSAL: the last stage row is the
    /// solution row.
    pub const B1: f64 = A71;
    pub const B3: f64 = A73;
    pub const B4: f64 = A74;
    pub const B5: f64 = A75;
    pub const B6: f64 = A76;

    pub const ERR_1: f64 = 71.0 / 57600.0;
    pub const ERR_3: f64 = -71.0 / 16695.0;
    pub const ERR_4: f64 = 71.0 / 1920.0;
    pub const ERR_5: f64 = -17253.0 / 339200.0;
    pub const ERR_6: f64 = 22.0 / 525.0;
    pub const ERR_7: f64 = -1.0 / 40.0;

    pub const DENSE_1: f64 = -12715105075.0 / 11282082432.0;
    pub const DENSE_3: f64 = 87487479700.0 / 32700410799.0;
    pub const DENSE_4: f64 = -10690763975.0 / 1880347072.0;
    pub const DENSE_5: f64 = 701980252875.0 / 199316789632.0;
    pub const DENSE_6: f64 = -1453857185.0 / 822651844.0;
    pub const DENSE_7: f64 = 69997945.0 / 29380423.0;
}

/// Step-size controller constants (Hairer's DOPRI5 defaults).
mod ctrl {
    pub const SAFETY: f64 = 0.9;
    pub const MAX_SHRINK: f64 = 0.2;
    pub const MAX_GROWTH: f64 = 10.0;
    pub const PI_BETA: f64 = 0.04;
    pub const EXPO1: f64 = 0.2 - PI_BETA * 0.75;
}

const MIN_REL_STEP: f64 = 1e-14;

/// A fixed step this close to the remaining span is stretched to land on
/// `t_end` rather than leave a sliver.
const END_STRETCH_TOL: f64 = 1e-9;

/// An adaptive step within this factor of the remaining span is stretched
/// to land on `t_end`.
const END_STRETCH: f64 = 1.01;

/// Radius of the circle inside the Fritsch–Carlson monotonicity region
/// that overshooting slopes are scaled back onto.
const FRITSCH_CARLSON_RADIUS: f64 = 3.0;

/// Limit the end slopes of one cubic Hermite piece so that it cannot
/// overshoot (Fritsch & Carlson, *Monotone piecewise cubic interpolation*,
/// SIAM J. Numer. Anal. 17, 1980).
fn limit_hermite_slopes(m0: f64, m1: f64, secant: f64) -> (f64, f64) {
    if secant == 0.0 {
        return (m0, m1);
    }
    let (alpha, beta) = (m0 / secant, m1 / secant);
    if !(alpha >= 0.0 && beta >= 0.0) || hermite_is_monotone(alpha, beta) {
        return (m0, m1);
    }
    let tau = FRITSCH_CARLSON_RADIUS / alpha.hypot(beta);
    (tau * m0, tau * m1)
}

fn hermite_is_monotone(alpha: f64, beta: f64) -> bool {
    alpha + beta <= 2.0
        || 2.0 * alpha + beta <= 3.0
        || alpha + 2.0 * beta <= 3.0
        || alpha - (2.0 * alpha + beta - 3.0).powi(2) / (3.0 * (alpha + beta - 2.0)) >= 0.0
}

fn ratio_or_zero(v: f64, w: f64) -> f64 {
    if v == 0.0 {
        0.0
    } else {
        v / w
    }
}

/// Where a fixed-step method evaluates `f` inside a step, in order: stage `j`
/// computes `k[j]` at `t + c·h`, `x + c·h·slope`, where `slope` is `fx`
/// (`None`) or an earlier stage's `k[i]` (`Some(i)`).
fn fixed_stage_points(method: Method) -> &'static [(f64, Option<usize>)] {
    match method {
        Method::Heun => &[(1.0, None)],
        Method::Rk4 => &[(0.5, None), (0.5, Some(0)), (1.0, Some(1))],
        Method::Euler | Method::Dopri5 => &[],
    }
}

/// Did component `x0 → x1` move away from 0 while `|f|` grew (`f0 → f1`)?
fn moves_outward_with_growing_slope(x0: f64, x1: f64, f0: f64, f1: f64) -> bool {
    x0 * x1 > 0.0 && x1.abs() > x0.abs() && f1 > f0 && f0 > 0.0
}

/// The PI controller's step-size divisor for error `err`, given the error
/// of the last accepted step: `(err^EXPO1, clamped divisor)`.
fn pi_step_factor(err: f64, err_old: f64) -> (f64, f64) {
    use ctrl::*;
    let fac11 = err.powf(EXPO1);
    let fac = fac11 / err_old.powf(PI_BETA);
    (
        fac11,
        (fac / SAFETY).clamp(1.0 / MAX_GROWTH, 1.0 / MAX_SHRINK),
    )
}

/// A step cut short to land on `t_end` says nothing about the step size:
/// the next proposal only replaces the planned one if it is larger.
fn adopts_proposal(lands_on_t_end: bool, h_new: f64, h_planned: f64) -> bool {
    !lands_on_t_end || h_new.abs() > h_planned.abs()
}

/// Which way time runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimeDirection {
    Forward,
    Backward,
}

impl TimeDirection {
    /// The direction from `t0` to `t1` (forward when they are equal).
    pub(crate) fn from_to(t0: f64, t1: f64) -> TimeDirection {
        if t1 < t0 {
            TimeDirection::Backward
        } else {
            TimeDirection::Forward
        }
    }

    pub(crate) fn sign(self) -> f64 {
        match self {
            TimeDirection::Forward => 1.0,
            TimeDirection::Backward => -1.0,
        }
    }
}

/// Dense output of one step, per component:
/// `x(t_prev + θh) = c0 + θ(c1 + (1-θ)(c2 + θ(c3 + (1-θ)c4)))`.
struct DenseCoeffs {
    c0: Vec<f64>,
    c1: Vec<f64>,
    c2: Vec<f64>,
    c3: Vec<f64>,
    c4: Vec<f64>,
}

impl DenseCoeffs {
    fn new(n: usize) -> DenseCoeffs {
        DenseCoeffs {
            c0: vec![0.0; n],
            c1: vec![0.0; n],
            c2: vec![0.0; n],
            c3: vec![0.0; n],
            c4: vec![0.0; n],
        }
    }

    fn eval(&self, theta: f64, out: &mut [f64]) {
        let theta1 = 1.0 - theta;
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.c0[i]
                + theta
                    * (self.c1[i]
                        + theta1 * (self.c2[i] + theta * (self.c3[i] + theta1 * self.c4[i])));
        }
    }

    /// The straight line from `x` to `x_new`.
    fn linear(&mut self, x: &[f64], x_new: &[f64]) {
        for i in 0..x.len() {
            self.c0[i] = x[i];
            self.c1[i] = x_new[i] - x[i];
            self.c2[i] = 0.0;
            self.c3[i] = 0.0;
            self.c4[i] = 0.0;
        }
    }

    /// Monotonicity-limited cubic Hermite through `(x, f)` and `(x_new, f_new)`.
    fn hermite(&mut self, h: f64, x: &[f64], x_new: &[f64], f: &[f64], f_new: &[f64]) {
        for i in 0..x.len() {
            let ydiff = x_new[i] - x[i];
            let (m0, m1) = limit_hermite_slopes(f[i], f_new[i], ydiff / h);
            let bspl = h * m0 - ydiff;
            self.c0[i] = x[i];
            self.c1[i] = ydiff;
            self.c2[i] = bspl;
            self.c3[i] = ydiff - h * m1 - bspl;
            self.c4[i] = 0.0;
        }
    }

    /// Dormand–Prince's fourth-order continuous extension.
    fn dopri5(
        &mut self,
        h: f64,
        x: &[f64],
        x_new: &[f64],
        k1: &[f64],
        k: &[Vec<f64>; 5],
        k7: &[f64],
    ) {
        use dormand_prince::*;
        let [_, k3, k4, k5, k6] = k;
        for i in 0..x.len() {
            let ydiff = x_new[i] - x[i];
            let bspl = h * k1[i] - ydiff;
            self.c0[i] = x[i];
            self.c1[i] = ydiff;
            self.c2[i] = bspl;
            self.c3[i] = ydiff - h * k7[i] - bspl;
            self.c4[i] = h
                * (DENSE_1 * k1[i]
                    + DENSE_3 * k3[i]
                    + DENSE_4 * k4[i]
                    + DENSE_5 * k5[i]
                    + DENSE_6 * k6[i]
                    + DENSE_7 * k7[i]);
        }
    }
}

/// How the dense output of an accepted step is built.
#[derive(Debug, Clone, Copy)]
enum Dense {
    /// Hermite from the end-point slopes; linear if `f` at the new point
    /// is not finite.
    Hermite,
    /// Dormand–Prince's own interpolant from the stages.
    Dopri5,
}

/// What a fixed step evaluated before it overflowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endpoint {
    /// Only the stages; `f_new` holds stale data.
    StagesOnly,
    /// The stages and `f` at the new point.
    FEvaluated,
}

/// One evaluation of `f` made during a step.
struct StageEval<'s> {
    t: f64,
    x: Vec<f64>,
    f: &'s [f64],
}

/// Result of one call to [`Stepper::step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// A step was accepted. [`Stepper::t`] / [`Stepper::x`] are the new
    /// point and [`Stepper::interpolate`] covers `[t_prev, t]`.
    Accepted,
    /// A step was accepted, but the new state exceeds
    /// [`BLOW_UP_THRESHOLD`](crate::BLOW_UP_THRESHOLD), or (fixed-step
    /// methods) `f` overflows to ±∞ there while the state grows. Stop
    /// integrating.
    BlewUp,
    /// Already at `t_end`; nothing was done.
    AtEnd,
    /// No step could be taken. The state is unchanged. The status is
    /// [`Status::StepTooSmall`], [`Status::NonFinite`], or
    /// [`Status::BlewUp`] when the adaptive step size collapsed because the
    /// solution runs off to infinity faster than double precision can
    /// follow, or a fixed step overflowed to a non-finite state while
    /// growing (see [`Stepper::step`]).
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
    h_max: f64,

    dir: TimeDirection,
    t_end: f64,

    t: f64,
    x: Vec<f64>,
    fx: Vec<f64>,
    h_next: f64,

    t_prev: f64,
    h_last: f64,
    cont: DenseCoeffs,
    has_dense: bool,

    err_old: f64,
    last_rejected: bool,

    x_prev: Vec<f64>,
    f_prev: Vec<f64>,

    k: [Vec<f64>; 5],
    x_stage: Vec<f64>,
    x_new: Vec<f64>,
    f_new: Vec<f64>,

    stats: Stats,
}

fn counted_eval(
    f: &dyn VectorField,
    p: &[f64],
    stats: &mut Stats,
    t: f64,
    x: &[f64],
    out: &mut [f64],
) {
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
        let dir = TimeDirection::from_to(t0, t_end);

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
            h_next: 0.0,
            t_prev: t0,
            h_last: 0.0,
            cont: DenseCoeffs::new(n),
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
        counted_eval(f, p, &mut s.stats, t0, &s.x, &mut s.fx);

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
        s.h_next = dir.sign() * h.min(h_max);
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
        self.h_next
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
        (self.t_end - self.t) * self.dir.sign() <= 0.0
    }

    /// Move the end point. If the new end lies on the other side of `t`, the
    /// direction is flipped.
    pub fn set_t_end(&mut self, t_end: f64) {
        self.t_end = t_end;
        if t_end != self.t {
            self.dir = TimeDirection::from_to(self.t, t_end);
            self.h_next = self.dir.sign() * self.h_next.abs();
        }
    }

    /// Replace the current point. Keeps the step size, re-evaluates `f`
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
        counted_eval(self.f, self.p, &mut self.stats, t, &self.x, &mut self.fx);
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
        if !self.has_dense || self.h_last == 0.0 {
            out.copy_from_slice(&self.x);
            return;
        }
        self.cont.eval((t - self.t_prev) / self.h_last, out);
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
    ///
    /// The fixed-step methods meet the same blow-ups as an overflow: `f` or
    /// a stage of the step becomes ±∞ at a finite state (`e^x` for
    /// `x > 709.8`). If the overflowing component is growing and `f` is
    /// still ±∞ just short of the point where it overflowed, the outcome is
    /// `BlewUp` (the new state is finite and kept) or `Failed(BlewUp)` (the
    /// new state is not finite and is dropped). Otherwise it is
    /// `Failed(NonFinite)`: NaN without overflow (`sqrt` of a negative
    /// number), or a step or stage landing exactly on a pole at a finite
    /// point (`x' = 1/(2 - x)^3` at 2, where `f` is finite just short of
    /// it; Dopri5 reports such a finite limit as `StepTooSmall`).
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
    /// `true` if some component moved away from 0 in the last step while its
    /// derivative grew, with elasticity `E = Δ ln|f_i| / Δ ln|x_i|` over the
    /// step at most 1000.
    ///
    /// Always `false` before the first step.
    pub fn is_blowing_up(&self) -> bool {
        const MAX_BLOW_UP_ELASTICITY: f64 = 1000.0;
        if !self.has_dense {
            return false;
        }
        (0..self.dim()).any(|i| {
            let (x0, x1) = (self.x_prev[i], self.x[i]);
            let (f0, f1) = (self.f_prev[i].abs(), self.fx[i].abs());
            if !moves_outward_with_growing_slope(x0, x1, f0, f1) {
                return false;
            }
            let elasticity = (f1 / f0).ln() / (x1 / x0).ln();
            elasticity <= MAX_BLOW_UP_ELASTICITY
        })
    }

    fn error_weight(&self, i: usize) -> f64 {
        self.atol + self.rtol * self.x[i].abs()
    }

    /// Hairer's `HINIT`: guess a starting step from the size of `x`, `f`
    /// and a finite-difference estimate of the second derivative.
    fn initial_step(&mut self) -> f64 {
        const ORDER: f64 = 5.0;
        let n = self.dim();

        let (mut dnf, mut dny) = (0.0, 0.0);
        for i in 0..n {
            let w = self.error_weight(i);
            dnf += ratio_or_zero(self.fx[i], w).powi(2);
            dny += ratio_or_zero(self.x[i], w).powi(2);
        }
        let mut h = if dnf <= 1e-10 || dny <= 1e-10 {
            1e-6
        } else {
            (dny / dnf).sqrt() * 0.01
        };
        h = h.min(self.h_max);

        let der2 = self.estimate_second_derivative(h);
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

    /// Weighted norm of `x''` from one explicit Euler step of size `h`.
    fn estimate_second_derivative(&mut self, h: f64) -> f64 {
        let n = self.dim();
        let hs = self.dir.sign() * h;
        for i in 0..n {
            self.x_stage[i] = self.x[i] + hs * self.fx[i];
        }
        counted_eval(
            self.f,
            self.p,
            &mut self.stats,
            self.t + hs,
            &self.x_stage,
            &mut self.f_new,
        );
        let mut der2 = 0.0;
        for i in 0..n {
            der2 += ratio_or_zero(self.f_new[i] - self.fx[i], self.error_weight(i)).powi(2);
        }
        der2.sqrt() / h
    }

    /// Accept the step to `(t_new, x_new)`: build its dense output, then make
    /// the new point current.
    fn commit(&mut self, t_new: f64, h: f64, dense: Dense) {
        match dense {
            Dense::Hermite if all_finite(&self.f_new) => {
                self.cont
                    .hermite(h, &self.x, &self.x_new, &self.fx, &self.f_new)
            }
            Dense::Hermite => self.cont.linear(&self.x, &self.x_new),
            Dense::Dopri5 => {
                self.cont
                    .dopri5(h, &self.x, &self.x_new, &self.fx, &self.k, &self.f_new)
            }
        }
        self.x_prev.copy_from_slice(&self.x);
        self.f_prev.copy_from_slice(&self.fx);
        self.t_prev = self.t;
        self.t = t_new;
        self.h_last = h;
        std::mem::swap(&mut self.x, &mut self.x_new);
        std::mem::swap(&mut self.fx, &mut self.f_new);
        self.has_dense = true;
        self.stats.accepted += 1;
    }

    fn accept(&mut self, t_new: f64, h: f64, dense: Dense) -> StepOutcome {
        self.commit(t_new, h, dense);
        if blew_up(&self.x) {
            StepOutcome::BlewUp
        } else {
            StepOutcome::Accepted
        }
    }

    /// Keep a finite new point where `f` overflowed while growing: the run
    /// stops there as a blow-up.
    fn accept_overflowed_endpoint(&mut self, t_new: f64, h: f64) -> StepOutcome {
        self.commit(t_new, h, Dense::Hermite);
        StepOutcome::BlewUp
    }

    fn step_fixed(&mut self) -> StepOutcome {
        let mut h = self.h_next;
        if self.too_small(h) {
            return StepOutcome::Failed(Status::StepTooSmall);
        }
        let remaining = self.t_end - self.t;
        let lands_on_t_end = remaining.abs() <= h.abs() * (1.0 + END_STRETCH_TOL);
        if lands_on_t_end {
            h = remaining;
        }
        let t_new = if lands_on_t_end {
            self.t_end
        } else {
            self.t + h
        };

        self.fixed_stages(h);
        if !all_finite(&self.x_new) {
            let status = if self.overflowed_while_growing(h, t_new, Endpoint::StagesOnly) {
                Status::BlewUp
            } else {
                Status::NonFinite
            };
            return StepOutcome::Failed(status);
        }
        counted_eval(
            self.f,
            self.p,
            &mut self.stats,
            t_new,
            &self.x_new,
            &mut self.f_new,
        );
        if !all_finite(&self.f_new) && !blew_up(&self.x_new) {
            if !self.overflowed_while_growing(h, t_new, Endpoint::FEvaluated) {
                return StepOutcome::Failed(Status::NonFinite);
            }
            return self.accept_overflowed_endpoint(t_new, h);
        }
        self.accept(t_new, h, Dense::Hermite)
    }

    fn overflowed_while_growing(&mut self, h: f64, t_new: f64, endpoint: Endpoint) -> bool {
        let stages = &self.k[..fixed_stage_points(self.method).len()];
        let uses_stage_point = !stages.is_empty();
        let f_evaluated = endpoint == Endpoint::FEvaluated;
        let blowing_up = self.is_blowing_up();
        let grew = (0..self.dim()).any(|i| {
            let xn = self.x_new[i];
            let overflow = xn.is_infinite()
                || (f_evaluated && self.f_new[i].is_infinite())
                || (uses_stage_point && self.x_stage[i].is_infinite())
                || stages.iter().any(|k| k[i].is_infinite());
            let growing = xn.abs() > self.x[i].abs() || (xn.is_nan() && blowing_up);
            overflow && growing
        });
        grew && self.still_overflows_near(h, t_new, endpoint)
    }

    fn still_overflows_near(&mut self, h: f64, t_new: f64, endpoint: Endpoint) -> bool {
        const PROBE_BACKOFF_FRACTION: f64 = 1e-6;
        let n = self.dim();
        let (t, x) = (self.t, &self.x);
        let along = |c: f64, k: &[f64]| -> Vec<f64> {
            x.iter().zip(k).map(|(xi, ki)| xi + c * h * ki).collect()
        };
        let stages: Vec<StageEval> = fixed_stage_points(self.method)
            .iter()
            .enumerate()
            .map(|(j, &(c, from))| StageEval {
                t: t + c * h,
                x: along(c, from.map_or(&self.fx, |i| &self.k[i])),
                f: &self.k[j],
            })
            .collect();
        let failed = stages
            .into_iter()
            .find(|s| !all_finite(s.f))
            .map(|s| (s.t, s.x))
            .or_else(|| {
                (endpoint == Endpoint::FEvaluated && !all_finite(&self.f_new))
                    .then(|| (t_new, self.x_new.clone()))
            });
        let Some((tp, xp)) = failed else {
            return true;
        };
        if !all_finite(&xp) {
            return true;
        }
        let probe: Vec<f64> = xp
            .iter()
            .zip(x)
            .map(|(p, x0)| p - PROBE_BACKOFF_FRACTION * (p - x0))
            .collect();
        let t_probe = tp - PROBE_BACKOFF_FRACTION * (tp - t);
        let mut f_probe = vec![0.0; n];
        counted_eval(
            self.f,
            self.p,
            &mut self.stats,
            t_probe,
            &probe,
            &mut f_probe,
        );
        f_probe.iter().any(|v| v.is_infinite())
    }

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
        for (j, &(c, from)) in fixed_stage_points(*method).iter().enumerate() {
            let slope: &[f64] = match from {
                None => &k1[..],
                Some(i) => &k[i][..],
            };
            for i in 0..n {
                xs[i] = x[i] + c * h * slope[i];
            }
            counted_eval(f, p, stats, t + c * h, xs, &mut k[j]);
        }
        let [k2, k3, k4, ..] = k;
        match method {
            Method::Euler => {
                for i in 0..n {
                    x_new[i] = x[i] + h * k1[i];
                }
            }
            Method::Heun => {
                for i in 0..n {
                    x_new[i] = x[i] + 0.5 * h * (k1[i] + k2[i]);
                }
            }
            Method::Rk4 => {
                for i in 0..n {
                    x_new[i] = x[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
                }
            }
            Method::Dopri5 => unreachable!("Dopri5 is handled by step_adaptive"),
        }
    }

    /// A NaN error, or a non-finite stage, means the step left the domain of
    /// `f`. An infinite error with finite stages is an ordinary rejection
    /// (a component with zero tolerance moved).
    fn stages_non_finite(&self, err: f64) -> bool {
        err.is_nan() || !all_finite(&self.x_new) || !all_finite(&self.f_new)
    }

    fn step_adaptive(&mut self) -> StepOutcome {
        use ctrl::*;
        let mut last_rejection_nonfinite = false;
        loop {
            let mut h = self.h_next;
            if self.too_small(h) {
                let status = if last_rejection_nonfinite {
                    Status::NonFinite
                } else if self.is_blowing_up() {
                    Status::BlewUp
                } else {
                    Status::StepTooSmall
                };
                return StepOutcome::Failed(status);
            }
            let lands_on_t_end = (self.t + END_STRETCH * h - self.t_end) * self.dir.sign() >= 0.0;
            if lands_on_t_end {
                h = self.t_end - self.t;
            }
            let t_new = if lands_on_t_end {
                self.t_end
            } else {
                self.t + h
            };

            self.dopri5_stages(h, t_new);
            let err = self.scaled_rms_error(h);
            if self.stages_non_finite(err) {
                self.stats.rejected += 1;
                self.last_rejected = true;
                last_rejection_nonfinite = true;
                self.h_next = h * MAX_SHRINK;
                continue;
            }
            last_rejection_nonfinite = false;

            let (fac11, fac) = pi_step_factor(err, self.err_old);
            let mut h_new = h / fac;

            if err <= 1.0 {
                self.err_old = err.max(1e-4);
                if h_new.abs() > self.h_max {
                    h_new = self.dir.sign() * self.h_max;
                }
                if self.last_rejected {
                    h_new = self.dir.sign() * h_new.abs().min(h.abs());
                }
                self.last_rejected = false;
                if adopts_proposal(lands_on_t_end, h_new, self.h_next) {
                    self.h_next = h_new;
                }
                return self.accept(t_new, h, Dense::Dopri5);
            }
            self.stats.rejected += 1;
            self.last_rejected = true;
            self.h_next = h / (1.0 / MAX_SHRINK).min(fac11 / SAFETY);
        }
    }

    /// The six stages of a Dormand–Prince step of size `h`, the new point in
    /// `x_new`, and `f` there in `f_new`, which is the next step's `k1`.
    fn dopri5_stages(&mut self, h: f64, t_new: f64) {
        use dormand_prince::*;
        let Self {
            f,
            p,
            t,
            x,
            fx: k1,
            k,
            x_stage: ys,
            x_new,
            f_new: fsal_k1,
            stats,
            ..
        } = self;
        let (f, p, t) = (*f, *p, *t);
        let n = x.len();
        let [k2, k3, k4, k5, k6] = k;

        for i in 0..n {
            ys[i] = x[i] + h * A21 * k1[i];
        }
        counted_eval(f, p, stats, t + C2 * h, ys, k2);
        for i in 0..n {
            ys[i] = x[i] + h * (A31 * k1[i] + A32 * k2[i]);
        }
        counted_eval(f, p, stats, t + C3 * h, ys, k3);
        for i in 0..n {
            ys[i] = x[i] + h * (A41 * k1[i] + A42 * k2[i] + A43 * k3[i]);
        }
        counted_eval(f, p, stats, t + C4 * h, ys, k4);
        for i in 0..n {
            ys[i] = x[i] + h * (A51 * k1[i] + A52 * k2[i] + A53 * k3[i] + A54 * k4[i]);
        }
        counted_eval(f, p, stats, t + C5 * h, ys, k5);
        for i in 0..n {
            ys[i] =
                x[i] + h * (A61 * k1[i] + A62 * k2[i] + A63 * k3[i] + A64 * k4[i] + A65 * k5[i]);
        }
        counted_eval(f, p, stats, t_new, ys, k6);
        for i in 0..n {
            x_new[i] = x[i] + h * (B1 * k1[i] + B3 * k3[i] + B4 * k4[i] + B5 * k5[i] + B6 * k6[i]);
        }
        counted_eval(f, p, stats, t_new, x_new, fsal_k1);
    }

    /// RMS of the embedded error estimate, scaled by the tolerances.
    fn scaled_rms_error(&self, h: f64) -> f64 {
        use dormand_prince::*;
        let n = self.dim();
        if n == 0 {
            return 0.0;
        }
        let (k1, k7) = (&self.fx, &self.f_new);
        let [_, k3, k4, k5, k6] = &self.k;
        let mut sum = 0.0;
        for i in 0..n {
            let e = h
                * (ERR_1 * k1[i]
                    + ERR_3 * k3[i]
                    + ERR_4 * k4[i]
                    + ERR_5 * k5[i]
                    + ERR_6 * k6[i]
                    + ERR_7 * k7[i]);
            let sk = self.atol + self.rtol * self.x[i].abs().max(self.x_new[i].abs());
            sum += ratio_or_zero(e, sk).powi(2);
        }
        (sum / n as f64).sqrt()
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
        use dormand_prince::*;
        let close = |a: f64, b: f64| assert!((a - b).abs() < 1e-15, "{a} vs {b}");
        close(A21, C2);
        close(A31 + A32, C3);
        close(A41 + A42 + A43, C4);
        close(A51 + A52 + A53 + A54, C5);
        close(A61 + A62 + A63 + A64 + A65, 1.0);
        close(B1 + B3 + B4 + B5 + B6, 1.0);
        close(ERR_1 + ERR_3 + ERR_4 + ERR_5 + ERR_6 + ERR_7, 0.0);
        close(
            DENSE_1 + DENSE_3 + DENSE_4 + DENSE_5 + DENSE_6 + DENSE_7,
            0.0,
        );
    }

    #[test]
    fn hermite_matches_cubic_exactly() {
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
        s.set_t_end(1.0);
        assert!(s.h_next() < 0.0);
        while s.step() == StepOutcome::Accepted {}
        assert!((s.x()[0] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn limiter_leaves_monotone_pieces_alone() {
        assert_eq!(limit_hermite_slopes(1.0, 1.1, 1.05), (1.0, 1.1));
        assert_eq!(limit_hermite_slopes(0.0, 3.0, 1.0), (0.0, 3.0));
    }

    #[test]
    fn limiter_leaves_turning_point_alone() {
        assert_eq!(limit_hermite_slopes(-0.5, 4.0, 1.0), (-0.5, 4.0));
    }

    #[test]
    fn limiter_leaves_flat_secant_alone() {
        assert_eq!(limit_hermite_slopes(1.0, -1.0, 0.0), (1.0, -1.0));
    }

    #[test]
    fn limiter_scales_overshooting_slopes_onto_the_circle() {
        let (m0, m1) = limit_hermite_slopes(2.0, 1e20, 1e9);
        assert!((m1 / 1e9 - FRITSCH_CARLSON_RADIUS).abs() < 1e-12 && m0 > 0.0 && m0 < 2.0);
        let (m0, m1) = limit_hermite_slopes(-2.0, -1e20, -1e9);
        assert!((m1 / -1e9 - FRITSCH_CARLSON_RADIUS).abs() < 1e-12 && m0 < 0.0);
    }

    #[test]
    fn limiter_keeps_huge_slopes_finite() {
        let (_, m1) = limit_hermite_slopes(1.0, 1e300, 1e-10);
        assert!(m1.is_finite() && m1 <= FRITSCH_CARLSON_RADIUS * 1e-10 * (1.0 + 1e-12));
    }

    #[test]
    fn monotonicity_region() {
        assert!(hermite_is_monotone(1.0, 1.0));
        assert!(hermite_is_monotone(3.0, 3.0));
        assert!(hermite_is_monotone(0.0, 3.0));
        assert!(!hermite_is_monotone(0.0, 3.5));
        assert!(!hermite_is_monotone(4.0, 4.0));
    }

    #[test]
    fn overflow_to_nan_is_not_monotone() {
        assert!(!hermite_is_monotone(0.0, f64::MAX));
    }

    #[test]
    fn fixed_step_blow_up_interpolant_stays_between_end_points() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0] * x[0]);
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 30.0, &opts(Method::Euler, 0.1));
        let mut out = [0.0];
        loop {
            let x0 = s.x()[0];
            let outcome = s.step();
            assert!(matches!(
                outcome,
                StepOutcome::Accepted | StepOutcome::BlewUp
            ));
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
    fn exponential_collapse_is_blew_up() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = x[0].exp());
        let mut s = Stepper::new(&f, &[], 0.0, &[0.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::BlewUp));
        assert!(s.is_blowing_up());
        assert!((s.t() - 1.0).abs() < 1e-5, "{}", s.t());
    }

    #[test]
    fn backwards_cubic_collapse_is_blew_up() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = -x[0].powi(3)
        });
        let mut s = Stepper::new(&f, &[], 0.0, &[-1.0], -5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::BlewUp));
        assert!((s.t() + 0.5).abs() < 1e-5, "{}", s.t());
    }

    #[test]
    fn coupled_quadratic_blow_up_crosses_the_threshold() {
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = x[1] * x[1];
            out[1] = x[0] * x[0];
        });
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0, 1.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::BlewUp);
    }

    #[test]
    fn cubic_pole_collapse_is_step_too_small() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = 1.0 / (2.0 - x[0]).powi(3)
        });
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::StepTooSmall));
        assert!(!s.is_blowing_up());
        assert!(s.x()[0] < 2.0 && s.x()[0] > 1.99);
    }

    #[test]
    fn inverse_x_collapse_is_step_too_small() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -1.0 / x[0]);
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0], 5.0, &SolverOptions::default());
        assert_eq!(run(&mut s), StepOutcome::Failed(Status::StepTooSmall));
    }

    #[test]
    fn infinite_error_with_finite_stages_is_an_ordinary_rejection() {
        let f = decay();
        let s = Stepper::new(&f, &[], 0.0, &[1.0], 1.0, &SolverOptions::default());
        assert!(!s.stages_non_finite(f64::INFINITY));
        assert!(s.stages_non_finite(f64::NAN));
    }

    #[test]
    fn no_blow_up_before_the_first_step() {
        let f = decay();
        let s = Stepper::new(&f, &[], 0.0, &[1.0], 1.0, &SolverOptions::default());
        assert!(!s.is_blowing_up());
    }

    #[test]
    fn zero_atol_with_zero_components() {
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
            out[0] = -x[0];
            out[1] = -x[1];
        });
        let o = SolverOptions {
            atol: 0.0,
            ..SolverOptions::default()
        };
        let mut s = Stepper::new(&f, &[], 0.0, &[1.0, 0.0], 1.0, &o);
        assert!(
            s.h_next().is_finite() && s.h_next() > 1e-6,
            "{}",
            s.h_next()
        );
        assert_eq!(run(&mut s), StepOutcome::AtEnd);
        assert_eq!(s.x()[1], 0.0);
        assert!((s.x()[0] - (-1.0f64).exp()).abs() < 1e-5);
        assert_eq!(ratio_or_zero(0.0, 0.0), 0.0);
        assert_eq!(ratio_or_zero(1.0, 0.0), f64::INFINITY);
    }
}
