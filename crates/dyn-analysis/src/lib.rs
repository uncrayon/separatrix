//! Phase-portrait analyses for the slope-field visualiser.
//!
//! Everything works on a [`dyn_core::VectorField`] and a [`Plane`] (the 2D
//! window the UI shows):
//!
//! - [`slope_field`]: arrows of the direction field on a grid;
//! - [`nullclines`]: curves where one component of the field vanishes
//!   (marching squares, with a heuristic that ignores sign changes across
//!   poles such as `1/x`);
//! - [`fixed_points`]: equilibria of flows (`f(x) = 0`) and of maps
//!   (`f(x) = x`) by multi-start Newton, with eigenvalues and type;
//! - [`classify`] / [`classify_jacobian`]: the type of a fixed point from
//!   its linearisation, following Strogatz, *Nonlinear Dynamics and Chaos*.
//!
//! ```
//! use dyn_analysis::{fixed_points, Classification};
//! use dyn_core::{FnField, SystemKind};
//!
//! // Damped oscillator x' = y, y' = -x - y/2: a stable spiral at the origin.
//! let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, out| {
//!     out[0] = x[1];
//!     out[1] = -x[0] - 0.5 * x[1];
//! });
//! let fps = fixed_points(&f, &[], &[(-1.0, 1.0), (-1.0, 1.0)], 4);
//! assert_eq!(fps.len(), 1);
//! assert_eq!(fps[0].classification, Classification::StableSpiral);
//! assert!(fps[0].stable);
//! ```

#![warn(missing_docs)]

mod classify;
mod fixed;
mod linalg;
mod nullcline;
mod plane;
mod slope;

pub use classify::{classify, classify_jacobian, Classification, ABS_TOL, REL_TOL};
pub use fixed::{fixed_points, FixedPoint, MAX_SEEDS};
pub use linalg::eigenvalues;
pub use nullcline::{nullclines, Nullcline, JUMP_FACTOR};
pub use plane::{Axis, Plane};
pub use slope::{slope_field, Arrow};
