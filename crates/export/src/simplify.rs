//! Curve simplification for exports (Ramer–Douglas–Peucker in figure
//! pixels).
//!
//! A run is first cut at its first sample whose shown coordinates are not
//! finite, as `trajectoryPx` does on screen (everything after it is never
//! drawn). The rest is simplified in figure pixel space, each axis mapped
//! separately, so the tolerance is a distance on the page whatever the
//! units; the kept samples keep their time and every state, so a simplified
//! run is still a run.

use crate::model::{Axis, Portrait, SystemKind, Viewport};

/// Tolerance used for every exported curve, in CSS pixels of the figure.
pub const EXPORT_TOLERANCE_PX: f64 = 0.25;

/// Number of leading points of `pts` before the first non-finite one.
pub fn finite_prefix(pts: &[(f64, f64)]) -> usize {
    pts.iter()
        .position(|(a, b)| !a.is_finite() || !b.is_finite())
        .unwrap_or(pts.len())
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Indices of the points Ramer–Douglas–Peucker keeps at tolerance `tol`
/// (increasing; the first and last are always kept). Every dropped point
/// lies within `tol` of the kept polyline. Iterative, with an explicit
/// stack, so long runs cannot overflow the call stack. The input must be
/// finite (see [`finite_prefix`]).
pub fn rdp_indices(pts: &[(f64, f64)], tol: f64) -> Vec<usize> {
    let n = pts.len();
    if n <= 2 {
        return (0..n).collect();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let mut worst = 0.0;
        let mut at = a;
        for (i, &p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = segment_distance(p, pts[a], pts[b]);
            if d > worst {
                worst = d;
                at = i;
            }
        }
        if worst > tol {
            keep[at] = true;
            stack.push((at, b));
            stack.push((a, at));
        }
    }
    (0..n).filter(|&i| keep[i]).collect()
}

fn to_px(vp: &Viewport, p: (f64, f64)) -> (f64, f64) {
    (vp.sx(p.0), vp.sy(p.1))
}

/// A polyline in plot coordinates, cut at its first non-finite point and
/// simplified at `tol` figure pixels.
pub fn simplify_polyline(vp: &Viewport, poly: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    let poly = &poly[..finite_prefix(poly)];
    let px: Vec<(f64, f64)> = poly.iter().map(|&p| to_px(vp, p)).collect();
    rdp_indices(&px, tol).into_iter().map(|i| poly[i]).collect()
}

/// A run (`t`, row-major `x` with `dim` columns) cut at its first sample
/// whose shown coordinates are not finite and, for a flow, simplified at
/// `tol` figure pixels. A map's iterates are dots, so they are only cut.
/// Returns the kept `t` and `x`.
#[allow(clippy::too_many_arguments)]
pub fn simplify_run(
    vp: &Viewport,
    kind: SystemKind,
    x_axis: Axis,
    y_axis: Axis,
    dim: usize,
    t: &[f64],
    x: &[f64],
    tol: f64,
) -> (Vec<f64>, Vec<f64>) {
    let n = t.len().min(x.len().checked_div(dim).unwrap_or(0));
    let shown: Vec<(f64, f64)> = (0..n)
        .map(|k| (x_axis.value(t, x, dim, k), y_axis.value(t, x, dim, k)))
        .collect();
    let m = finite_prefix(&shown);
    let keep: Vec<usize> = if kind == SystemKind::Map {
        (0..m).collect()
    } else {
        let px: Vec<(f64, f64)> = shown[..m].iter().map(|&p| to_px(vp, p)).collect();
        rdp_indices(&px, tol)
    };
    let mut tt = Vec::with_capacity(keep.len());
    let mut xx = Vec::with_capacity(keep.len() * dim);
    for k in keep {
        tt.push(t[k]);
        xx.extend_from_slice(&x[k * dim..(k + 1) * dim]);
    }
    (tt, xx)
}

/// Every curve of `p` cut and simplified at `tol` pixels of a
/// `width` × `height` figure (what `compute_export_portrait` returns).
pub fn simplify_portrait(p: &mut Portrait, width: f64, height: f64, tol: f64) {
    let vp = Viewport::new(width, height, p.x_range, p.y_range);
    for nc in &mut p.nullclines {
        for line in &mut nc.polylines {
            *line = simplify_polyline(&vp, line, tol);
        }
    }
    for tr in &mut p.trajectories {
        let (t, x) = simplify_run(&vp, p.kind, p.x_axis, p.y_axis, p.dim, &tr.t, &tr.x, tol);
        tr.t = t;
        tr.x = x;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_deviation(pts: &[(f64, f64)], keep: &[usize]) -> f64 {
        let mut worst: f64 = 0.0;
        for w in keep.windows(2) {
            for p in &pts[w[0]..=w[1]] {
                worst = worst.max(segment_distance(*p, pts[w[0]], pts[w[1]]));
            }
        }
        worst
    }

    #[test]
    fn rdp_keeps_ends_and_stays_within_tolerance() {
        let pts: Vec<(f64, f64)> = (0..2000)
            .map(|i| {
                let a = i as f64 * 0.01;
                (100.0 * a.cos() * (1.0 + 0.1 * a), 100.0 * a.sin())
            })
            .collect();
        for tol in [0.25, 0.5, 1.0, 2.0] {
            let keep = rdp_indices(&pts, tol);
            assert_eq!(keep[0], 0);
            assert_eq!(*keep.last().unwrap(), pts.len() - 1);
            assert!(keep.windows(2).all(|w| w[0] < w[1]));
            assert!(max_deviation(&pts, &keep) <= tol, "{tol}");
            assert!(keep.len() < pts.len() / 4, "{tol}: {}", keep.len());
        }
        // A straight line keeps only its ends; a closed loop keeps its far side.
        let line: Vec<(f64, f64)> = (0..100).map(|i| (i as f64, 2.0 * i as f64)).collect();
        assert_eq!(rdp_indices(&line, 0.25), [0, 99]);
        let loop_: Vec<(f64, f64)> = (0..=100)
            .map(|i| {
                let a = i as f64 / 100.0 * std::f64::consts::TAU;
                (50.0 * a.cos(), 50.0 * a.sin())
            })
            .collect();
        assert!(rdp_indices(&loop_, 0.25).len() > 10);
        assert_eq!(rdp_indices(&[], 1.0), Vec::<usize>::new());
        assert_eq!(rdp_indices(&[(0.0, 0.0)], 1.0), [0]);
    }

    #[test]
    fn rdp_handles_long_runs_without_recursion() {
        // A spiral of 300,000 samples: deep splits, no call-stack recursion.
        let pts: Vec<(f64, f64)> = (0..300_000)
            .map(|i| {
                let a = i as f64 * 1e-3;
                (2.0 * a * a.cos(), 2.0 * a * a.sin())
            })
            .collect();
        let keep = rdp_indices(&pts, 0.25);
        assert!(max_deviation(&pts, &keep) <= 0.25);
    }

    #[test]
    fn runs_are_cut_at_the_first_non_finite_sample() {
        let vp = Viewport::new(100.0, 100.0, (0.0, 10.0), (0.0, 10.0));
        let t = [0.0, 1.0, 2.0, 3.0, 4.0];
        let x = [0.0, 0.0, 1.0, 1.0, f64::NAN, 2.0, 3.0, 3.0, 4.0, 4.0];
        let sx = Axis::State { index: 0 };
        let sy = Axis::State { index: 1 };
        let (tt, xx) = simplify_run(&vp, SystemKind::Ode, sx, sy, 2, &t, &x, 0.25);
        assert_eq!(tt, [0.0, 1.0]);
        assert_eq!(xx, [0.0, 0.0, 1.0, 1.0]);
        // Points after the break are never drawn, even where they are finite again.
        let (tt, _) = simplify_run(&vp, SystemKind::Map, sx, sy, 2, &t, &x, 0.25);
        assert_eq!(tt.len(), 2);
        // A hidden coordinate that is not finite does not cut what is shown.
        let x3 = [0.0, 0.0, f64::NAN, 1.0, 1.0, f64::NAN];
        let (tt, xx) = simplify_run(&vp, SystemKind::Map, sx, sy, 3, &t[..2], &x3, 0.25);
        assert_eq!(tt.len(), 2);
        assert!(xx[2].is_nan());
        assert_eq!(
            simplify_polyline(
                &vp,
                &[(0.0, 0.0), (1.0, 1.0), (f64::INFINITY, 0.0), (3.0, 3.0)],
                0.25
            ),
            [(0.0, 0.0), (1.0, 1.0)]
        );
    }

    #[test]
    fn maps_are_never_thinned() {
        let vp = Viewport::new(100.0, 100.0, (0.0, 10.0), (0.0, 10.0));
        let t: Vec<f64> = (0..50).map(f64::from).collect();
        let x: Vec<f64> = (0..50).map(|i| 5.0 + 1e-6 * f64::from(i)).collect();
        let (tt, _) = simplify_run(
            &vp,
            SystemKind::Map,
            Axis::Time,
            Axis::State { index: 0 },
            1,
            &t,
            &x,
            0.25,
        );
        assert_eq!(tt.len(), 50);
        let (tt, _) = simplify_run(
            &vp,
            SystemKind::Ode,
            Axis::Time,
            Axis::State { index: 0 },
            1,
            &t,
            &x,
            0.25,
        );
        assert_eq!(tt.len(), 2);
    }
}
