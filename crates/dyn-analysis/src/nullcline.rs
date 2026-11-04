//! Nullclines by marching squares.
//!
//! For every state variable `i` shown on an axis we trace the zero set of
//! `g = f_i` over the plane: the curves where that variable momentarily stops
//! changing. With a time axis and a 1D system this is the zero-slope
//! isocline `f(t, x) = 0`.
//!
//! # Algorithm
//!
//! 1. Evaluate `f` on a `(grid + 1) x (grid + 1)` lattice of points covering
//!    the plane (corners included).
//! 2. For every lattice edge whose end values have opposite signs, place a
//!    crossing point by linear interpolation, refined by one regula falsi
//!    step (one extra evaluation). Values that are exactly `0` count as
//!    positive, which keeps the case table consistent.
//! 3. For every cell, connect its crossing points (marching squares). Cells
//!    with four crossings (the ambiguous "saddle" case) are resolved with the
//!    average of the four corner values, which approximates the value at the
//!    cell centre.
//! 4. Join the segments into polylines through their shared edges.
//!
//! # Rejecting poles and jumps
//!
//! A sign change does not always mean a zero: `1/y` changes sign across
//! `y = 0` by jumping through infinity, and `tan x` does the same at `π/2`.
//! A crossing on an edge from `v0` to `v1` is treated as spurious, and every
//! segment that uses it is dropped, when **both** of these hold:
//!
//! - *Big jump*: `|v1 - v0|` is more than [`JUMP_FACTOR`] times the median
//!   of `|Δg|` over all lattice edges (the typical variation of `g` between
//!   neighbouring lattice points);
//! - *Grows inwards*: `g` at the edge midpoint is larger in magnitude than
//!   the end value on the same side of the sign change. A continuous function
//!   crossing zero shrinks towards its zero, whereas near a pole it grows.
//!
//! Requiring both keeps steep but continuous crossings (like `tanh(50 y)`,
//! which has a big jump but shrinks inwards) and crossings near the
//! intersection of two branches of the same nullcline (which grow inwards
//! but have small jumps). The midpoint, or any lattice value, being
//! non-finite also marks the edge as spurious.

use std::collections::HashMap;

use dyn_core::{SystemKind, VectorField};
use serde::{Deserialize, Serialize};

use crate::plane::{Axis, Plane, Sampler};

/// A crossing on an edge is a pole/jump candidate if the jump across the
/// edge exceeds this multiple of the median edge variation.
pub const JUMP_FACTOR: f64 = 10.0;

/// The zero set of one component of the field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nullcline {
    /// Which equation vanishes along these curves (`f_state = 0`).
    pub state: usize,
    /// Curves in plot coordinates. A closed curve repeats its first point at
    /// the end.
    pub polylines: Vec<Vec<(f64, f64)>>,
}

/// Computes the nullclines of every state variable shown on an axis.
///
/// Uses a `(grid + 1) x (grid + 1)` lattice over the plane (see the module
/// docs for the algorithm and the pole heuristic). One [`Nullcline`] is
/// returned per state on an axis (x axis first), possibly with no
/// polylines. Returns an empty `Vec` for maps, for `grid == 0` and for axes
/// that refer to a non-existent state.
pub fn nullclines(f: &dyn VectorField, p: &[f64], plane: &Plane, grid: usize) -> Vec<Nullcline> {
    if f.kind() == SystemKind::Map || grid == 0 {
        return Vec::new();
    }
    let Some(mut sampler) = Sampler::new(f, plane) else {
        return Vec::new();
    };

    let mut states = Vec::new();
    for axis in [plane.x, plane.y] {
        if let Axis::State(i) = axis {
            if !states.contains(&i) {
                states.push(i);
            }
        }
    }
    if states.is_empty() {
        return Vec::new();
    }

    let lattice = Lattice::new(plane, grid);
    // values[k][j * n + i] = f_{states[k]} at lattice point (i, j).
    let n = grid + 1;
    let mut values = vec![vec![0.0; n * n]; states.len()];
    for j in 0..n {
        for i in 0..n {
            let (px, py) = lattice.point(i, j);
            sampler.eval(f, p, px, py);
            for (k, &s) in states.iter().enumerate() {
                values[k][j * n + i] = sampler.out[s];
            }
        }
    }

    states
        .iter()
        .zip(&values)
        .map(|(&state, g)| {
            let mut eval_g = |px: f64, py: f64| {
                sampler.eval(f, p, px, py);
                sampler.out[state]
            };
            Nullcline {
                state,
                polylines: trace(&lattice, g, &mut eval_g),
            }
        })
        .collect()
}

/// Geometry of the evaluation lattice.
struct Lattice {
    /// Number of cells per side.
    grid: usize,
    x0: f64,
    y0: f64,
    dx: f64,
    dy: f64,
}

impl Lattice {
    fn new(plane: &Plane, grid: usize) -> Self {
        let (x0, x1) = plane.x_range;
        let (y0, y1) = plane.y_range;
        Self {
            grid,
            x0,
            y0,
            dx: (x1 - x0) / grid as f64,
            dy: (y1 - y0) / grid as f64,
        }
    }

    /// Plot coordinates of lattice point `(i, j)`, `0 <= i, j <= grid`.
    fn point(&self, i: usize, j: usize) -> (f64, f64) {
        (self.x0 + i as f64 * self.dx, self.y0 + j as f64 * self.dy)
    }
}

/// A lattice edge: `H(i, j)` joins points `(i, j)`–`(i+1, j)`,
/// `V(i, j)` joins `(i, j)`–`(i, j+1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Edge {
    H(usize, usize),
    V(usize, usize),
}

impl Edge {
    /// The two lattice points of the edge.
    fn ends(self) -> ((usize, usize), (usize, usize)) {
        match self {
            Edge::H(i, j) => ((i, j), (i + 1, j)),
            Edge::V(i, j) => ((i, j), (i, j + 1)),
        }
    }
}

/// What we know about an edge with a sign change.
#[derive(Clone, Copy)]
enum Crossing {
    /// A genuine zero at this point.
    Point((f64, f64)),
    /// A pole or jump; segments using this edge are dropped.
    Spurious,
}

/// Traces the zero set of `g` (sampled on the lattice) into polylines.
fn trace(
    lat: &Lattice,
    g: &[f64],
    eval_g: &mut dyn FnMut(f64, f64) -> f64,
) -> Vec<Vec<(f64, f64)>> {
    let n = lat.grid + 1;
    let val = |(i, j): (usize, usize)| g[j * n + i];
    let positive = |v: f64| v >= 0.0;
    let scale = median_edge_variation(g, n);

    let mut crossings: HashMap<Edge, Crossing> = HashMap::new();
    let mut crossing = |e: Edge| -> Crossing {
        *crossings.entry(e).or_insert_with(|| {
            let (a, b) = e.ends();
            let (v0, v1) = (val(a), val(b));
            let (p0, p1) = (lat.point(a.0, a.1), lat.point(b.0, b.1));
            classify_crossing(v0, v1, p0, p1, scale, eval_g)
        })
    };

    // Segments as pairs of edges; their points are looked up in `crossings`.
    let mut segments: Vec<(Edge, Edge)> = Vec::new();
    for j in 0..lat.grid {
        for i in 0..lat.grid {
            let corners = [
                val((i, j)),
                val((i + 1, j)),
                val((i + 1, j + 1)),
                val((i, j + 1)),
            ];
            if corners.iter().any(|v| !v.is_finite()) {
                continue;
            }
            // Edges in counter-clockwise order: bottom, right, top, left.
            // Edge k joins corner k and corner (k + 1) % 4.
            let edges = [
                Edge::H(i, j),
                Edge::V(i + 1, j),
                Edge::H(i, j + 1),
                Edge::V(i, j),
            ];
            let crossed: Vec<usize> = (0..4)
                .filter(|&k| positive(corners[k]) != positive(corners[(k + 1) % 4]))
                .collect();
            let pairs: Vec<(usize, usize)> = match crossed.len() {
                2 => vec![(crossed[0], crossed[1])],
                4 => {
                    // Saddle cell: corners 0 and 2 share a sign, as do 1 and 3.
                    // If the centre has the sign of corner 0, corners 0 and 2
                    // are connected through the middle, so cut off 1 and 3.
                    let centre = corners.iter().sum::<f64>() / 4.0;
                    if positive(centre) == positive(corners[0]) {
                        vec![(0, 1), (2, 3)] // around corners 1 and 3
                    } else {
                        vec![(3, 0), (1, 2)] // around corners 0 and 2
                    }
                }
                _ => Vec::new(),
            };
            for (ka, kb) in pairs {
                let (ea, eb) = (edges[ka], edges[kb]);
                if matches!(crossing(ea), Crossing::Point(_))
                    && matches!(crossing(eb), Crossing::Point(_))
                {
                    segments.push((ea, eb));
                }
            }
        }
    }

    let point_of = |e: &Edge| match crossings[e] {
        Crossing::Point(pt) => pt,
        Crossing::Spurious => unreachable!("spurious edges are never part of a segment"),
    };
    join_segments(&segments)
        .into_iter()
        .map(|chain| chain.iter().map(point_of).collect())
        .collect()
}

/// Median of `|Δg|` over all lattice edges with finite, non-zero variation.
/// Returns `0` if there is none (e.g. `g` is constant).
fn median_edge_variation(g: &[f64], n: usize) -> f64 {
    let mut diffs = Vec::with_capacity(2 * n * n);
    for j in 0..n {
        for i in 0..n {
            let v = g[j * n + i];
            if i + 1 < n {
                diffs.push((g[j * n + i + 1] - v).abs());
            }
            if j + 1 < n {
                diffs.push((g[(j + 1) * n + i] - v).abs());
            }
        }
    }
    diffs.retain(|d| d.is_finite() && *d > 0.0);
    if diffs.is_empty() {
        return 0.0;
    }
    let mid = diffs.len() / 2;
    *diffs.select_nth_unstable_by(mid, f64::total_cmp).1
}

/// Decides whether the sign change from `v0` (at `p0`) to `v1` (at `p1`) is a
/// zero (and where) or a pole/jump. See the module docs.
fn classify_crossing(
    v0: f64,
    v1: f64,
    p0: (f64, f64),
    p1: (f64, f64),
    scale: f64,
    eval_g: &mut dyn FnMut(f64, f64) -> f64,
) -> Crossing {
    let lerp = |s: f64| (p0.0 + s * (p1.0 - p0.0), p0.1 + s * (p1.1 - p0.1));
    let big_jump = (v1 - v0).abs() > JUMP_FACTOR * scale;
    if big_jump {
        let mid = lerp(0.5);
        let gm = eval_g(mid.0, mid.1);
        if !gm.is_finite() {
            return Crossing::Spurious;
        }
        // End value on the same side of the sign change as the midpoint.
        let same_side = if (gm >= 0.0) == (v0 >= 0.0) { v0 } else { v1 };
        if gm.abs() > same_side.abs() {
            return Crossing::Spurious;
        }
    }
    // v0 and v1 have different signs, so v0 - v1 != 0.
    let s = v0 / (v0 - v1);
    // One regula falsi refinement: evaluate at the linear estimate and
    // interpolate again on the part of the edge that still has the sign
    // change. This makes curved nullclines markedly more accurate for the
    // price of one evaluation per crossing.
    let (px, py) = lerp(s);
    let gs = eval_g(px, py);
    let refined = if !gs.is_finite() || gs == 0.0 || s <= 0.0 || s >= 1.0 {
        s
    } else if (gs >= 0.0) == (v0 >= 0.0) {
        s + (1.0 - s) * gs / (gs - v1) // sign change between s and 1
    } else {
        s * v0 / (v0 - gs) // sign change between 0 and s
    };
    Crossing::Point(lerp(refined))
}

/// Joins segments that share an edge into chains of edges. Open chains come
/// first; closed loops repeat their first edge at the end.
fn join_segments(segments: &[(Edge, Edge)]) -> Vec<Vec<Edge>> {
    // Each edge belongs to at most two cells, hence to at most two segments.
    let mut touching: HashMap<Edge, Vec<usize>> = HashMap::new();
    for (k, (a, b)) in segments.iter().enumerate() {
        touching.entry(*a).or_default().push(k);
        touching.entry(*b).or_default().push(k);
    }
    let mut used = vec![false; segments.len()];

    // Walks from `start` (an end of segment `first`) as far as possible.
    let walk = |first: usize, start: Edge, used: &mut Vec<bool>| -> Vec<Edge> {
        let mut chain = vec![start];
        let (mut seg, mut at) = (first, start);
        loop {
            used[seg] = true;
            let (a, b) = segments[seg];
            let next_edge = if a == at { b } else { a };
            chain.push(next_edge);
            at = next_edge;
            match touching[&at].iter().find(|&&s| !used[s]) {
                Some(&s) => seg = s,
                None => break,
            }
        }
        chain
    };

    let mut chains = Vec::new();
    // Open chains start at an edge used by only one segment.
    let mut ends: Vec<(Edge, usize)> = segments
        .iter()
        .enumerate()
        .flat_map(|(k, (a, b))| [(*a, k), (*b, k)])
        .filter(|(e, _)| touching[e].len() == 1)
        .collect();
    // Deterministic output regardless of hash order.
    ends.sort_by_key(|&(_, k)| k);
    for (e, k) in ends {
        if !used[k] {
            chains.push(walk(k, e, &mut used));
        }
    }
    // Whatever is left forms closed loops.
    for k in 0..segments.len() {
        if !used[k] {
            chains.push(walk(k, segments[k].0, &mut used));
        }
    }
    chains
}

#[cfg(test)]
mod tests {
    use super::*;
    use dyn_core::FnField;

    fn xy_plane(x_range: (f64, f64), y_range: (f64, f64)) -> Plane {
        Plane {
            x: Axis::State(0),
            y: Axis::State(1),
            x_range,
            y_range,
            base_state: vec![0.0, 0.0],
            t: 0.0,
        }
    }

    fn rabbits_sheep() -> FnField {
        FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] * (3.0 - x[0] - 2.0 * x[1]);
            o[1] = x[1] * (2.0 - x[0] - x[1]);
        })
    }

    fn all_points(nc: &Nullcline) -> Vec<(f64, f64)> {
        nc.polylines.iter().flatten().copied().collect()
    }

    #[test]
    fn rabbits_vs_sheep_nullclines_lie_on_true_lines() {
        let f = rabbits_sheep();
        let plane = xy_plane((-0.37, 3.6), (-0.41, 3.3));
        let ncs = nullclines(&f, &[], &plane, 60);
        assert_eq!(ncs.len(), 2);
        assert_eq!((ncs[0].state, ncs[1].state), (0, 1));
        let h = 4.0 / 60.0;
        for nc in &ncs {
            let pts = all_points(nc);
            assert!(pts.len() > 100, "too few points: {}", pts.len());
            for (x, y) in pts {
                // Distance to the nearest true nullcline line.
                let d = if nc.state == 0 {
                    x.abs().min((3.0 - x - 2.0 * y).abs() / 5f64.sqrt())
                } else {
                    y.abs().min((2.0 - x - y).abs() / 2f64.sqrt())
                };
                // Where the two lines of one nullcline cross, g is locally a
                // product of two small factors and interpolation is less
                // accurate; elsewhere points are very close to the lines.
                let crossing = if nc.state == 0 {
                    (0.0, 1.5)
                } else {
                    (2.0, 0.0)
                };
                let near_crossing = (x - crossing.0).hypot(y - crossing.1) < 4.0 * h;
                let tol = if near_crossing { 0.25 * h } else { 5e-3 * h };
                assert!(d < tol, "state {} point ({x}, {y}) off by {d}", nc.state);
                let mut o = [0.0; 2];
                f.eval(0.0, &[x, y], &[], &mut o);
                // Linear interpolation of a quadratic: error O(h²).
                assert!(o[nc.state].abs() < h * h, "|f| = {}", o[nc.state].abs());
            }
        }
        // Both lines of each nullcline are found: x = 0 and x + 2y = 3.
        let pts = all_points(&ncs[0]);
        assert!(pts.iter().any(|&(x, y)| x.abs() < 1e-3 && y > 3.0));
        assert!(pts.iter().any(|&(x, y)| x > 3.0 && y.abs() < 0.1));
    }

    #[test]
    fn straight_line_is_one_polyline() {
        // x' = y - x: the nullcline is the diagonal, traced as a single chain.
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[1] - x[0] + 0.01;
            o[1] = 1.0;
        });
        let ncs = nullclines(&f, &[], &xy_plane((-1.0, 1.0), (-1.0, 1.0)), 20);
        assert_eq!(ncs[0].polylines.len(), 1);
        assert!(ncs[1].polylines.is_empty()); // y' = 1 never vanishes
        let line = &ncs[0].polylines[0];
        assert!(line.len() > 20);
        for &(x, y) in line {
            assert!((y - x + 0.01).abs() < 1e-12);
        }
    }

    #[test]
    fn circle_is_closed_loop() {
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] * x[0] + x[1] * x[1] - 1.0;
            o[1] = 1.0;
        });
        let ncs = nullclines(&f, &[], &xy_plane((-2.0, 2.0), (-2.0, 2.0)), 40);
        assert_eq!(ncs[0].polylines.len(), 1);
        let loop_ = &ncs[0].polylines[0];
        assert_eq!(loop_.first(), loop_.last());
        for &(x, y) in loop_ {
            assert!(((x * x + y * y).sqrt() - 1.0).abs() < 5e-3);
        }
    }

    #[test]
    fn pole_does_not_produce_a_nullcline() {
        // x' = 1/y, y' = 1: no zero anywhere, but a sign change at y = 0.
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = 1.0 / x[1];
            o[1] = 1.0;
        });
        for grid in [10, 37, 40, 100] {
            for y_range in [(-3.0, 3.0), (-2.9, 3.3), (-1.0, 0.5)] {
                let ncs = nullclines(&f, &[], &xy_plane((-3.0, 3.0), y_range), grid);
                assert!(
                    ncs[0].polylines.is_empty(),
                    "grid {grid} range {y_range:?}: {:?}",
                    ncs[0].polylines
                );
            }
        }
    }

    #[test]
    fn tan_keeps_zeros_but_drops_poles() {
        // x' = tan(x): zeros at 0, ±π; poles at ±π/2.
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0].tan();
            o[1] = 1.0;
        });
        let ncs = nullclines(&f, &[], &xy_plane((-3.5, 3.5), (-1.0, 1.0)), 50);
        let xs: Vec<f64> = ncs[0].polylines.iter().map(|l| l[0].0).collect();
        assert_eq!(xs.len(), 3, "{xs:?}");
        for x in xs {
            let d = [-std::f64::consts::PI, 0.0, std::f64::consts::PI]
                .iter()
                .map(|z| (x - z).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(d < 1e-2, "unexpected nullcline at x = {x}");
        }
    }

    #[test]
    fn steep_continuous_crossing_is_kept() {
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = (50.0 * (x[1] - 0.123)).tanh();
            o[1] = 1.0;
        });
        let ncs = nullclines(&f, &[], &xy_plane((-1.0, 1.0), (-1.0, 1.0)), 30);
        assert_eq!(ncs[0].polylines.len(), 1);
        for &(_, y) in &ncs[0].polylines[0] {
            assert!((y - 0.123).abs() < 0.03);
        }
    }

    #[test]
    fn saddle_cells_give_crossing_lines() {
        // x' = x y: nullcline is both axes, crossing at the origin, which is
        // the centre of a cell with this grid.
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] * x[1];
            o[1] = 1.0;
        });
        let ncs = nullclines(&f, &[], &xy_plane((-1.05, 0.95), (-1.05, 0.95)), 20);
        for &(x, y) in ncs[0].polylines.iter().flatten() {
            assert!(x.abs().min(y.abs()) < 0.06, "({x}, {y})");
        }
        let pts: Vec<_> = ncs[0].polylines.iter().flatten().collect();
        assert!(pts.iter().any(|p| p.0 < -1.0) && pts.iter().any(|p| p.0 > 0.9));
        assert!(pts.iter().any(|p| p.1 < -1.0) && pts.iter().any(|p| p.1 > 0.9));
    }

    #[test]
    fn one_dimensional_time_axis_isocline() {
        // x' = x - t: zero-slope isocline x = t in the (t, x) plane.
        let f = FnField::new(1, 0, SystemKind::Ode, |t, x, _p, o| o[0] = x[0] - t).non_autonomous();
        let plane = Plane {
            x: Axis::Time,
            y: Axis::State(0),
            x_range: (0.0, 2.0),
            y_range: (-1.0, 3.0),
            base_state: vec![0.0],
            t: 0.0,
        };
        let ncs = nullclines(&f, &[], &plane, 25);
        assert_eq!(ncs.len(), 1);
        assert_eq!(ncs[0].polylines.len(), 1);
        for &(t, x) in &ncs[0].polylines[0] {
            assert!((x - t).abs() < 1e-12);
        }
    }

    #[test]
    fn hidden_state_is_not_traced() {
        // Plane (x, z) of a 3D system: nullclines of f_0 and f_2 only.
        let f = FnField::new(3, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] - x[1];
            o[1] = 0.0;
            o[2] = x[2] + x[1];
        });
        let plane = Plane {
            x: Axis::State(0),
            y: Axis::State(2),
            x_range: (-1.0, 1.0),
            y_range: (-1.0, 1.0),
            base_state: vec![0.0, 0.3, 0.0],
            t: 0.0,
        };
        let ncs = nullclines(&f, &[], &plane, 10);
        assert_eq!(ncs.iter().map(|n| n.state).collect::<Vec<_>>(), vec![0, 2]);
        for &(x, _) in ncs[0].polylines.iter().flatten() {
            assert!((x - 0.3).abs() < 1e-12);
        }
        for &(_, z) in ncs[1].polylines.iter().flatten() {
            assert!((z + 0.3).abs() < 1e-12);
        }
    }

    #[test]
    fn maps_and_degenerate_input_give_nothing() {
        let map = FnField::new(2, 0, SystemKind::Map, |_t, x, _p, o| o.copy_from_slice(x));
        assert!(nullclines(&map, &[], &xy_plane((0.0, 1.0), (0.0, 1.0)), 10).is_empty());
        let f = rabbits_sheep();
        assert!(nullclines(&f, &[], &xy_plane((0.0, 1.0), (0.0, 1.0)), 0).is_empty());
        let mut pl = xy_plane((0.0, 1.0), (0.0, 1.0));
        pl.y = Axis::State(5);
        assert!(nullclines(&f, &[], &pl, 10).is_empty());
    }

    #[test]
    fn serialises_as_point_arrays() {
        let nc = Nullcline {
            state: 1,
            polylines: vec![vec![(0.0, 1.0), (2.0, 3.5)]],
        };
        let json = serde_json::to_string(&nc).unwrap();
        assert_eq!(json, r#"{"state":1,"polylines":[[[0.0,1.0],[2.0,3.5]]]}"#);
    }
}
