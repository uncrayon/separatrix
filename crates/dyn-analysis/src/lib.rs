//! Phase-portrait analyses for Separatrix.
//!
//! Everything works on a [`dyn_core::VectorField`] and a [`Plane`] (the 2D
//! window the UI shows):
//!
//! - [`slope_field`]: arrows of the direction field on a grid;
//! - [`nullclines`]: curves where one component of the field vanishes
//!   (marching squares, with a heuristic that ignores sign changes across
//!   poles such as `1/x`);
//! - [`fixed_points`]: equilibria of flows (`f(x) = 0`) and of maps
//!   (`f(x) = x`) by multi-start Newton, with eigenvalues, eigenvectors
//!   (see [`eigenvectors`]) and type;
//! - [`map_graph`]: the graph `y = f(x)` of a one-dimensional map, split at
//!   poles, jumps and the edges of its domain (the cobweb view);
//! - [`lyapunov_spectrum`]: the Lyapunov exponents of a flow or a map by
//!   Benettin's QR method, with a block standard error and the trace check;
//! - [`orbit_diagram`]: the orbit (bifurcation) diagram of a map over a
//!   parameter range, and [`period_doublings`]: the period-doubling points
//!   of a one-dimensional map located from it;
//! - [`continue_branch`]: a branch of equilibria of a flow, or of fixed
//!   points of `f^k` for a map, followed through folds as one parameter
//!   varies by pseudo-arclength continuation, with the eigenvalues and
//!   stability of each point and its located folds, Hopf, branch, flip and
//!   Neimark–Sacker points ([`SpecialPoint`]), and [`switch_branch`]: the
//!   second branch through a branch point, or the `2k`-cycle born at a flip;
//! - [`continue_curve`]: a curve of folds, Hopf points, flips or
//!   Neimark–Sacker points followed as two parameters vary, with the
//!   stretches where a Hopf or Neimark–Sacker condition holds at a neutral
//!   saddle marked ([`Nature`]);
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
mod continuation;
mod curve;
mod eigvec;
mod fixed;
mod graph;
mod linalg;
mod lyapunov;
mod nullcline;
mod orbit;
mod plane;
mod slope;

pub use classify::{classify, classify_jacobian, Classification, ABS_TOL, REL_TOL};
pub use continuation::{
    continue_branch, switch_branch, Branch, BranchEnd, BranchPoint, ContinuationError,
    ContinuationOptions, SpecialKind, SpecialPoint,
};
pub use curve::{
    continue_curve, Curve, CurveEnd, CurveError, CurveKind, CurveOptions, CurvePoint, CurveSeed,
    Nature,
};
pub use eigvec::{eigenvectors, Eigenvectors};
pub use fixed::{fixed_points, fixed_points_with_budget, seed_count, FixedPoint, MAX_SEEDS};
pub use graph::{map_graph, MAX_GRAPH_SAMPLES};
pub use linalg::eigenvalues;
pub use lyapunov::{lyapunov_spectrum, LyapunovOptions, LyapunovSpectrum};
pub use nullcline::{nullclines, Nullcline, JUMP_FACTOR};
pub use orbit::{orbit_diagram, period_doublings, Doubling, OrbitDiagram, OrbitDiagramOptions};
pub use plane::{Axis, Plane};
pub use slope::{slope_field, Arrow};
