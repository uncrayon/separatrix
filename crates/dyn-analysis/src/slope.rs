use dyn_core::{SystemKind, VectorField};
use serde::{Deserialize, Serialize};

use crate::plane::{Plane, Sampler};

/// One arrow of the direction field, in plot coordinates.
///
/// `(u, v)` are the *raw* field components (not normalised): the UI decides
/// how to scale them. On a time axis the component is `1`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Arrow {
    /// Horizontal position of the arrow's base point.
    pub x: f64,
    /// Vertical position of the arrow's base point.
    pub y: f64,
    /// Horizontal component of the field.
    pub u: f64,
    /// Vertical component of the field.
    pub v: f64,
}

/// Samples the direction field on a `grid x grid` lattice of cell centres.
///
/// For each point the full state is `plane.base_state` with the axis
/// variables replaced by the point's coordinates, and `f` is evaluated at
/// time `plane.t` (or at the point's coordinate when an axis is
/// [`Axis::Time`](crate::Axis::Time)).
///
/// Arrows are returned row by row, starting at the bottom row
/// (`y_range.0`), with `x` increasing fastest. Points where a component is
/// not finite (e.g. on a pole of `1/x`) are left out.
///
/// Returns an empty `Vec` for maps (a direction field is not meaningful for
/// a discrete system), for `grid == 0`, or if an axis refers to a state
/// index that does not exist.
pub fn slope_field(f: &dyn VectorField, p: &[f64], plane: &Plane, grid: usize) -> Vec<Arrow> {
    if f.kind() == SystemKind::Map || grid == 0 {
        return Vec::new();
    }
    let Some(mut sampler) = Sampler::new(f, plane) else {
        return Vec::new();
    };
    let (x0, x1) = plane.x_range;
    let (y0, y1) = plane.y_range;
    let (dx, dy) = ((x1 - x0) / grid as f64, (y1 - y0) / grid as f64);

    let mut arrows = Vec::with_capacity(grid * grid);
    for j in 0..grid {
        let y = y0 + (j as f64 + 0.5) * dy;
        for i in 0..grid {
            let x = x0 + (i as f64 + 0.5) * dx;
            sampler.eval(f, p, x, y);
            let u = sampler.component(plane.x);
            let v = sampler.component(plane.y);
            if u.is_finite() && v.is_finite() {
                arrows.push(Arrow { x, y, u, v });
            }
        }
    }
    arrows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Axis;
    use dyn_core::FnField;

    fn plane(x: Axis, y: Axis) -> Plane {
        Plane {
            x,
            y,
            x_range: (-1.0, 1.0),
            y_range: (0.0, 4.0),
            base_state: vec![0.5, 0.25, 7.0],
            t: 2.0,
        }
    }

    fn field() -> FnField {
        FnField::new(3, 0, SystemKind::Ode, |t, x, _p, o| {
            o[0] = x[1] + t;
            o[1] = -x[0];
            o[2] = x[2];
        })
        .non_autonomous()
    }

    #[test]
    fn grid_positions_are_cell_centres() {
        let a = slope_field(&field(), &[], &plane(Axis::State(0), Axis::State(1)), 2);
        assert_eq!(a.len(), 4);
        let pos: Vec<_> = a.iter().map(|a| (a.x, a.y)).collect();
        assert_eq!(pos, vec![(-0.5, 1.0), (0.5, 1.0), (-0.5, 3.0), (0.5, 3.0)]);
    }

    #[test]
    fn state_components_use_plane_time() {
        let a = slope_field(&field(), &[], &plane(Axis::State(0), Axis::State(1)), 4);
        for arrow in &a {
            assert_eq!(arrow.u, arrow.y + 2.0);
            assert_eq!(arrow.v, -arrow.x);
        }
    }

    #[test]
    fn hidden_variables_come_from_base_state() {
        let a = slope_field(&field(), &[], &plane(Axis::State(0), Axis::State(2)), 3);
        for arrow in &a {
            assert_eq!(arrow.u, 0.25 + 2.0);
            assert_eq!(arrow.v, arrow.y);
        }
    }

    #[test]
    fn time_axis_component_is_one_and_sets_t() {
        let a = slope_field(&field(), &[], &plane(Axis::Time, Axis::State(0)), 5);
        assert_eq!(a.len(), 25);
        for arrow in &a {
            assert_eq!(arrow.u, 1.0);
            assert!((arrow.v - (0.25 + arrow.x)).abs() < 1e-15);
        }
        let a = slope_field(&field(), &[], &plane(Axis::State(1), Axis::Time), 5);
        for arrow in &a {
            assert_eq!(arrow.v, 1.0);
            assert_eq!(arrow.u, -0.5);
        }
    }

    #[test]
    fn one_dimensional_with_time_axis() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] * (1.0 - x[0])
        });
        let pl = Plane {
            x: Axis::Time,
            y: Axis::State(0),
            x_range: (0.0, 10.0),
            y_range: (-1.0, 2.0),
            base_state: vec![0.0],
            t: 0.0,
        };
        let a = slope_field(&f, &[], &pl, 6);
        assert_eq!(a.len(), 36);
        for arrow in a {
            assert_eq!(arrow.u, 1.0);
            assert_eq!(arrow.v, arrow.y * (1.0 - arrow.y));
        }
    }

    #[test]
    fn maps_bad_axes_and_zero_grid_give_nothing() {
        let map = FnField::new(1, 0, SystemKind::Map, |_t, x, _p, o| o[0] = x[0]);
        let pl = Plane {
            x: Axis::Time,
            y: Axis::State(0),
            x_range: (0.0, 1.0),
            y_range: (0.0, 1.0),
            base_state: vec![0.0],
            t: 0.0,
        };
        assert!(slope_field(&map, &[], &pl, 10).is_empty());
        assert!(slope_field(&field(), &[], &plane(Axis::State(0), Axis::State(3)), 10).is_empty());
        assert!(slope_field(&field(), &[], &plane(Axis::State(0), Axis::State(1)), 0).is_empty());
    }

    #[test]
    fn non_finite_points_are_skipped() {
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = 1.0;
            o[1] = 1.0 / x[0];
        });
        let pl = Plane {
            x: Axis::State(0),
            y: Axis::State(1),
            x_range: (-1.5, 1.5),
            y_range: (-1.5, 1.5),
            base_state: vec![0.0, 0.0],
            t: 0.0,
        };
        let a = slope_field(&f, &[], &pl, 3);
        assert_eq!(a.len(), 6);
        assert!(a.iter().all(|a| a.x != 0.0));
    }
}
