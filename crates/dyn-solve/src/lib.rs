//! Numerical integration of flows and iteration of maps.
//!
//! This crate turns a [`VectorField`] (built by `dyn-expr` from the user's
//! equations, or by hand with [`dyn_core::FnField`]) into trajectories.
//!
//! * [`integrate`] solves `dx/dt = f(t, x; p)` from `t0` to `t1` (backwards
//!   when `t1 < t0`) with one of seven [`Method`]s: two of them symplectic
//!   for separable Hamiltonians, one implicit for stiff systems.
//! * [`integrate_with_events`] does the same and also reports when given
//!   functions `g(t, x)` cross zero (used for Poincaré sections, turning
//!   points, ...).
//! * [`section()`] returns only the points where a flow meets a Poincaré
//!   section: a hyperplane `x_i = c`, or the stroboscopic times `t mod T`.
//! * [`iterate_map`] iterates `x_{n+1} = f(n, x_n; p)`.
//! * [`Stepper`] is the building block underneath: it advances the solution
//!   one step at a time and can interpolate inside the last step. Use it
//!   directly when you need full control (Lyapunov exponents, custom
//!   stopping rules).
//!
//! The methods and the adaptive step-size control follow Hairer, Nørsett &
//! Wanner, *Solving Ordinary Differential Equations I* (the DOPRI5 code);
//! [`Method::Rosenbrock23`] is Shampine & Reichelt's `ode23s`.
//!
//! # Example
//!
//! ```
//! use dyn_core::{FnField, SystemKind};
//! use dyn_solve::{integrate, Sampling, SolverOptions, Status};
//!
//! // Exponential decay x' = -k x with k = 0.5.
//! let f = FnField::new(1, 1, SystemKind::Ode, |_t, x, p, out| out[0] = -p[0] * x[0]);
//! let sol = integrate(&f, &[0.5], 0.0, &[1.0], 2.0, &SolverOptions::default(), Sampling::Uniform(5));
//!
//! assert_eq!(sol.status, Status::Completed);
//! assert_eq!(sol.t, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
//! assert!((sol.state(4)[0] - (-1.0f64).exp()).abs() < 1e-6);
//! ```

#![warn(missing_docs)]

use dyn_core::VectorField;
use serde::{Deserialize, Serialize};

mod events;
mod lu;
mod map;
mod rosenbrock;
mod sampling;
mod section;
mod stepper;
mod symplectic;

pub use events::{Direction, Event, EventFn, EventHit};
pub use map::iterate_map;
pub use section::{section, Section};
pub use stepper::{StepOutcome, Stepper};

use events::EventTracker;
use sampling::Recorder;

/// A trajectory is stopped with [`Status::BlewUp`] as soon as any component
/// satisfies `|x_i| > BLOW_UP_THRESHOLD` (or when a blow-up is too fast to
/// reach it: an adaptive step collapses, or a fixed step overflows; see
/// [`Stepper::step`]).
pub const BLOW_UP_THRESHOLD: f64 = 1e12;

/// Integration method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// Forward Euler, order 1, fixed step. (Strogatz §2.8.)
    Euler,
    /// Improved Euler / Heun, order 2, fixed step. (Strogatz §2.8.)
    Heun,
    /// Classical fourth-order Runge–Kutta, fixed step. (Strogatz §2.8.)
    Rk4,
    /// Dormand–Prince 5(4) with adaptive step size (the default; what
    /// MATLAB's `ode45` and SciPy's `RK45` use).
    Dopri5,
    /// Störmer–Verlet (leapfrog), symplectic, order 2, fixed step. For a
    /// separable Hamiltonian only; see [`Method::is_symplectic`].
    Verlet,
    /// Yoshida's fourth-order symplectic composition of three Verlet steps,
    /// fixed step. For a separable Hamiltonian only.
    Yoshida4,
    /// Rosenbrock 2(3), Shampine & Reichelt's L-stable linearly implicit
    /// method (MATLAB's `ode23s`), adaptive, for stiff systems. Each step
    /// takes the field's Jacobian ([`VectorField::jacobian`]) and solves
    /// with `I − h·d·J`; its step is limited by accuracy, never by the
    /// fast decaying modes that hold back the explicit methods.
    Rosenbrock23,
}

impl Method {
    /// Chooses its own step size from `rtol` and `atol` (Dopri5,
    /// Rosenbrock23); the others take the step `h` as given.
    pub fn is_adaptive(self) -> bool {
        matches!(self, Method::Dopri5 | Method::Rosenbrock23)
    }

    /// Preserves the symplectic form of a Hamiltonian flow, so the energy
    /// error stays bounded over long times instead of drifting. Such a
    /// method needs a separable `H(q, p) = T(p) + V(q)` with the state laid
    /// out as `[q; p]` (`dyn_expr::CompiledSystem::is_separable`). The
    /// [`Stepper`] trusts that; the caller must check it.
    pub fn is_symplectic(self) -> bool {
        matches!(self, Method::Verlet | Method::Yoshida4)
    }
}

/// Solver settings.
///
/// Serialized in camelCase to match `SolverOptions` in `app/src/api/types.ts`;
/// missing fields take their [`Default`] value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SolverOptions {
    /// Which integrator to use.
    pub method: Method,
    /// Relative tolerance (adaptive methods only).
    pub rtol: f64,
    /// Absolute tolerance (adaptive methods only).
    pub atol: f64,
    /// Step size for the fixed-step methods; initial step size for the
    /// adaptive ones. `0` means "choose automatically": Hairer's
    /// starting-step heuristic for those, `|t1 - t0| / 1000` for the
    /// fixed-step methods. The sign is ignored (the direction comes from
    /// `t1 - t0`).
    pub h: f64,
    /// Largest step size allowed. `0`, negative or non-finite means no limit.
    pub h_max: f64,
    /// Maximum number of accepted steps before giving up with
    /// [`Status::MaxSteps`].
    pub max_steps: usize,
}

impl Default for SolverOptions {
    fn default() -> Self {
        Self {
            method: Method::Dopri5,
            rtol: 1e-6,
            atol: 1e-9,
            h: 0.0,
            h_max: 0.0,
            max_steps: 100_000,
        }
    }
}

/// Which points of the trajectory to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sampling {
    /// The initial point and every accepted step (uneven spacing for Dopri5).
    Steps,
    /// `N` points evenly spaced in time from `t0` to `t1` inclusive,
    /// computed from the dense output. If integration stops early, only the
    /// grid points reached are returned, followed by the last computed point.
    Uniform(usize),
}

/// Why an integration or iteration stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Reached `t1` (or performed all iterations).
    Completed,
    /// Some `|x_i|` exceeded [`BLOW_UP_THRESHOLD`] (finite-time blow-up or
    /// escape to infinity). The offending point is included.
    ///
    /// Also reported by [`Method::Dopri5`] when the step size collapses
    /// while the solution is running off to infinity too fast to ever reach
    /// the threshold in double precision (e.g. `x' = x^3`, `x' = e^x`); the
    /// last point is then the last one computed. See [`Stepper::step`].
    ///
    /// The fixed-step methods report it too when a step overflows (`f` or a
    /// stage becomes ±∞) on a component that is growing and `f` is still
    /// ±∞ just short of the overflowing point, e.g. Euler on `x' = e^x`
    /// reaching a finite `x ≈ 1e9` where `e^x = ∞`. The last finite state
    /// computed is included. A step landing exactly on a pole at a finite
    /// point (`x' = 1/(2 - x)^3` at 2), where `f` is finite just short of
    /// it, is [`Status::NonFinite`] instead.
    BlewUp,
    /// `max_steps` accepted steps were taken before reaching `t1`.
    MaxSteps,
    /// The adaptive step size became negligibly small compared with `t`
    /// (typically a singularity where `x` stays bounded but `f` does not,
    /// such as `x' = -1/x` reaching 0; fast blow-ups are [`Status::BlewUp`]).
    StepTooSmall,
    /// The right-hand side produced NaN or infinity (e.g. `sqrt` of a
    /// negative number, division by zero) other than by a blow-up (see
    /// [`Status::BlewUp`]). The bad point is *not* included.
    NonFinite,
}

/// Work counters, useful to compare methods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Accepted steps (map iterations for [`iterate_map`]).
    pub accepted: usize,
    /// Rejected steps (adaptive methods only).
    pub rejected: usize,
    /// Calls to [`VectorField::eval`].
    pub evals: usize,
}

/// A computed trajectory.
///
/// Serialized like `TrajectoryOut` in `app/src/api/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Times (iterate index `n` for maps).
    pub t: Vec<f64>,
    /// States, row-major: point `i` is `x[i * dim .. (i + 1) * dim]`.
    pub x: Vec<f64>,
    /// Why the computation stopped. The points computed so far are always
    /// returned, whatever the status.
    pub status: Status,
    /// Work counters.
    pub stats: Stats,
    /// Dormand–Prince's steps looked stiff ([`Stepper::looks_stiff`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stiff: bool,
}

impl Solution {
    /// Number of points.
    pub fn len(&self) -> usize {
        self.t.len()
    }

    /// `true` if there are no points.
    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }

    /// Dimension of the state (0 if there are no points).
    pub fn dim(&self) -> usize {
        if self.t.is_empty() {
            0
        } else {
            self.x.len() / self.t.len()
        }
    }

    /// State at point `i`.
    ///
    /// # Panics
    /// If `i >= self.len()`.
    pub fn state(&self, i: usize) -> &[f64] {
        let d = self.dim();
        &self.x[i * d..(i + 1) * d]
    }

    /// Last point `(t, x)`, if any.
    pub fn last(&self) -> Option<(f64, &[f64])> {
        let n = self.len();
        (n > 0).then(|| (self.t[n - 1], self.state(n - 1)))
    }
}

pub(crate) fn all_finite(v: &[f64]) -> bool {
    v.iter().all(|a| a.is_finite())
}

pub(crate) fn blew_up(v: &[f64]) -> bool {
    v.iter().any(|a| a.abs() > BLOW_UP_THRESHOLD)
}

/// Integrate `dx/dt = f(t, x; p)` from `(t0, x0)` to `t1`.
///
/// `t1 < t0` integrates backwards in time. Whatever happens, the points
/// computed so far are returned and [`Solution::status`] says why the
/// integration stopped.
///
/// # Panics
/// If `x0.len() != f.dim()`.
pub fn integrate(
    f: &dyn VectorField,
    p: &[f64],
    t0: f64,
    x0: &[f64],
    t1: f64,
    opts: &SolverOptions,
    sampling: Sampling,
) -> Solution {
    integrate_with_events(f, p, t0, x0, t1, opts, sampling, &[]).0
}

/// Like [`integrate`], but also reports zero crossings of event functions.
///
/// After every accepted step each event function `g` is evaluated at the new
/// point. If its sign changed (in the requested [`Direction`], judged in the
/// order the solver moves, i.e. backwards in time when `t1 < t0`), the
/// crossing time is located with Brent's method on the dense output to about
/// `4 * f64::EPSILON * max(1, |t|)`. Integration then continues normally. Hits are
/// returned in the order they occur along the integration.
///
/// Limitation: two crossings of the same `g` inside one step cancel out and
/// are missed. Lower `h_max` if `g` oscillates quickly.
///
/// # Panics
/// If `x0.len() != f.dim()`.
#[allow(clippy::too_many_arguments)]
pub fn integrate_with_events(
    f: &dyn VectorField,
    p: &[f64],
    t0: f64,
    x0: &[f64],
    t1: f64,
    opts: &SolverOptions,
    sampling: Sampling,
    events: &[Event],
) -> (Solution, Vec<EventHit>) {
    assert_eq!(
        x0.len(),
        f.dim(),
        "x0 has length {} but the system has dimension {}",
        x0.len(),
        f.dim()
    );

    let mut hits = Vec::new();
    if !all_finite(x0) || !t0.is_finite() || !t1.is_finite() {
        let empty = Solution {
            t: vec![],
            x: vec![],
            status: Status::NonFinite,
            stats: Stats::default(),
            stiff: false,
        };
        return (empty, hits);
    }
    if blew_up(x0) {
        let sol = Solution {
            t: vec![t0],
            x: x0.to_vec(),
            status: Status::BlewUp,
            stats: Stats::default(),
            stiff: false,
        };
        return (sol, hits);
    }

    let mut stepper = Stepper::new(f, p, t0, x0, t1, opts);
    let mut recorder = Recorder::new(sampling, t0, t1, x0);
    let mut tracker = EventTracker::new(events, t0, x0);

    let status = loop {
        if stepper.stats().accepted >= opts.max_steps && !stepper.is_done() {
            break Status::MaxSteps;
        }
        match stepper.step() {
            StepOutcome::AtEnd => break Status::Completed,
            StepOutcome::Accepted => {
                recorder.record_step(&stepper);
                tracker.after_step(&stepper, &mut hits);
            }
            StepOutcome::BlewUp => {
                recorder.record_step(&stepper);
                tracker.after_step(&stepper, &mut hits);
                break Status::BlewUp;
            }
            StepOutcome::Failed(status) => break status,
        }
    };

    (recorder.finish(&stepper, status), hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_names_match_the_typescript_types() {
        let opts = SolverOptions {
            method: Method::Rk4,
            ..SolverOptions::default()
        };
        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains("\"method\":\"rk4\""), "{json}");
        assert!(json.contains("\"maxSteps\":100000"), "{json}");
        assert!(json.contains("\"hMax\""), "{json}");
        assert_eq!(
            serde_json::to_string(&Method::Dopri5).unwrap(),
            "\"dopri5\""
        );
        assert_eq!(
            serde_json::to_string(&Method::Verlet).unwrap(),
            "\"verlet\""
        );
        assert_eq!(
            serde_json::to_string(&Method::Yoshida4).unwrap(),
            "\"yoshida4\""
        );
        assert_eq!(
            serde_json::to_string(&Method::Rosenbrock23).unwrap(),
            "\"rosenbrock23\""
        );
        assert_eq!(
            serde_json::to_string(&Status::BlewUp).unwrap(),
            "\"blewUp\""
        );
        assert_eq!(
            serde_json::to_string(&Status::StepTooSmall).unwrap(),
            "\"stepTooSmall\""
        );
    }

    #[test]
    fn missing_h_max_takes_default() {
        let parsed: SolverOptions = serde_json::from_str(
            r#"{"method":"heun","rtol":1e-3,"atol":1e-6,"h":0.1,"maxSteps":10}"#,
        )
        .unwrap();
        assert_eq!(parsed.method, Method::Heun);
        assert_eq!(parsed.h_max, 0.0);
        assert_eq!(parsed.max_steps, 10);
    }

    #[test]
    fn solution_accessors() {
        let s = Solution {
            t: vec![0.0, 1.0],
            x: vec![1.0, 2.0, 3.0, 4.0],
            status: Status::Completed,
            stats: Stats::default(),
            stiff: false,
        };
        assert_eq!(s.len(), 2);
        assert_eq!(s.dim(), 2);
        assert_eq!(s.state(1), &[3.0, 4.0]);
        assert_eq!(s.last(), Some((1.0, &[3.0, 4.0][..])));
    }
}
