//! The 2D window ("plane") onto a possibly higher-dimensional system.
//!
//! The UI shows two axes. Each axis is either one of the state variables or
//! time. Every state variable that is *not* on an axis is held fixed at its
//! value in [`Plane::base_state`] (a "slice" through state space).

use dyn_core::VectorField;

/// What a plot axis shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// State variable number `i` (index into the state vector).
    State(usize),
    /// Time `t` (ODE) or the iterate `n` (map).
    Time,
}

/// A rectangular window onto the system, as drawn by the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Plane {
    /// Horizontal axis.
    pub x: Axis,
    /// Vertical axis.
    pub y: Axis,
    /// `(min, max)` of the horizontal axis.
    pub x_range: (f64, f64),
    /// `(min, max)` of the vertical axis.
    pub y_range: (f64, f64),
    /// Values for the state variables that are not on an axis. Should have
    /// length `dim`; missing entries are treated as `0`.
    pub base_state: Vec<f64>,
    /// Time at which the field is evaluated when no axis is [`Axis::Time`].
    pub t: f64,
}

/// Helper that turns plot coordinates `(px, py)` into a full `(t, x)` pair
/// for evaluating the field. Shared by the slope field and the nullclines.
pub(crate) struct Sampler<'a> {
    plane: &'a Plane,
    /// Scratch state vector: `base_state` with the axis entries overwritten.
    pub state: Vec<f64>,
    /// Scratch output of `f`.
    pub out: Vec<f64>,
}

impl<'a> Sampler<'a> {
    /// Returns `None` if an axis refers to a state index `>= f.dim()`.
    pub fn new(f: &dyn VectorField, plane: &'a Plane) -> Option<Self> {
        let dim = f.dim();
        for axis in [plane.x, plane.y] {
            if let Axis::State(i) = axis {
                if i >= dim {
                    return None;
                }
            }
        }
        let mut state = vec![0.0; dim];
        for (s, b) in state.iter_mut().zip(&plane.base_state) {
            *s = *b;
        }
        Some(Self { plane, state, out: vec![0.0; dim] })
    }

    /// Evaluates `f` at plot coordinates `(px, py)`; the result is in
    /// `self.out`. Returns the time used, so callers can reuse it.
    pub fn eval(&mut self, f: &dyn VectorField, p: &[f64], px: f64, py: f64) -> f64 {
        let mut t = self.plane.t;
        for (axis, value) in [(self.plane.x, px), (self.plane.y, py)] {
            match axis {
                Axis::State(i) => self.state[i] = value,
                Axis::Time => t = value,
            }
        }
        f.eval(t, &self.state, p, &mut self.out);
        t
    }

    /// The component of the field along `axis` after a call to [`Self::eval`]:
    /// `1` on a time axis (time advances at unit speed), `f_i` on `State(i)`.
    pub fn component(&self, axis: Axis) -> f64 {
        match axis {
            Axis::State(i) => self.out[i],
            Axis::Time => 1.0,
        }
    }
}
