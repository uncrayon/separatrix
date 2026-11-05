//! The geometry of a cobweb plot (Strogatz §10.1), one definition shared with
//! the screen (`app/src/canvas/cobweb.ts`; the shared vectors are
//! `tests/vectors/cobweb.json`).
//!
//! - [`staircase`]: from the orbit `v_0 … v_m`, the vertices `(v_0, v_0)`,
//!   then for each `k = 1 … m` the pair `(v_{k−1}, v_k)`, `(v_k, v_k)`:
//!   `2m + 1` vertices that start and end on the diagonal and alternate a
//!   vertical step (to the graph of `f`) and a horizontal one (back to the
//!   diagonal). The staircase is cut at the first non-finite value.
//! - [`diagonal`]: the part of `y = x` inside the view.
//! - [`drop_retraced`]: segments already drawn are left out (a converged
//!   `p`-cycle costs `2p` segments however many iterates there are).

use std::collections::HashSet;

/// The staircase of `orbit` (`v_k = x_{T+k}`), cut at the first non-finite
/// value: `2m + 1` vertices for `m + 1` finite values, none for none.
pub fn staircase(orbit: &[f64]) -> Vec<(f64, f64)> {
    let m = orbit
        .iter()
        .position(|v| !v.is_finite())
        .unwrap_or(orbit.len());
    let orbit = &orbit[..m];
    let Some(&first) = orbit.first() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(2 * m - 1);
    out.push((first, first));
    for w in orbit.windows(2) {
        out.push((w[0], w[1]));
        out.push((w[1], w[1]));
    }
    out
}

/// The diagonal `y = x` inside the view: from `(a, a)` to `(b, b)` with
/// `a = max(x0, y0)` and `b = min(x1, y1)`, or `None` when `a ≥ b`.
pub fn diagonal(x_range: (f64, f64), y_range: (f64, f64)) -> Option<((f64, f64), (f64, f64))> {
    let a = x_range.0.max(y_range.0);
    let b = x_range.1.min(y_range.1);
    (a < b).then_some(((a, a), (b, b)))
}

/// The staircase without the segments already drawn, as pieces in drawing
/// order (a new piece starts after every skipped segment). Two segments are
/// the same when their ends have the same `key` in either order; `key`
/// quantizes a point (the figure's rounding, or the screen's half pixel).
pub fn drop_retraced<K: Eq + std::hash::Hash + Clone>(
    vertices: &[(f64, f64)],
    key: impl Fn((f64, f64)) -> K,
) -> Vec<Vec<(f64, f64)>> {
    let mut seen: HashSet<(K, K)> = HashSet::new();
    let mut pieces: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for w in vertices.windows(2) {
        let (p, q) = (w[0], w[1]);
        let (kp, kq) = (key(p), key(q));
        if kp == kq {
            // A step of length 0 (on a fixed point) draws nothing.
            continue;
        }
        let drawn = seen.contains(&(kq.clone(), kp.clone()));
        if !drawn && seen.insert((kp, kq)) {
            if cur.is_empty() {
                cur.push(p);
            }
            cur.push(q);
        } else if !cur.is_empty() {
            pieces.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        pieces.push(cur);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_staircase_alternates_and_ends_on_the_diagonal() {
        let v = staircase(&[0.1, 0.288, 0.65618]);
        assert_eq!(
            v,
            vec![
                (0.1, 0.1),
                (0.1, 0.288),
                (0.288, 0.288),
                (0.288, 0.65618),
                (0.65618, 0.65618)
            ]
        );
        assert_eq!(staircase(&[0.5]), vec![(0.5, 0.5)]);
        assert!(staircase(&[]).is_empty());
        assert_eq!(staircase(&[0.2, 0.3, f64::NAN, 0.4]).len(), 3);
        assert!(staircase(&[f64::INFINITY]).is_empty());
    }

    #[test]
    fn the_diagonal_is_clipped_to_the_view() {
        assert_eq!(
            diagonal((-0.05, 1.05), (-0.05, 1.05)),
            Some(((-0.05, -0.05), (1.05, 1.05)))
        );
        assert_eq!(
            diagonal((0.0, 2.0), (1.0, 3.0)),
            Some(((1.0, 1.0), (2.0, 2.0)))
        );
        // Touching at a corner only, or missing the view.
        assert_eq!(diagonal((0.0, 1.0), (1.0, 2.0)), None);
        assert_eq!(diagonal((0.0, 1.0), (2.0, 3.0)), None);
    }

    #[test]
    fn retraced_segments_are_dropped_in_order() {
        let key = |(x, y): (f64, f64)| ((x * 1e6).round() as i64, (y * 1e6).round() as i64);
        // A 2-cycle {0.2, 0.8} retraced three times.
        let mut orbit = vec![0.5];
        for _ in 0..3 {
            orbit.extend([0.2, 0.8]);
        }
        let v = staircase(&orbit);
        let pieces = drop_retraced(&v, key);
        let segments: usize = pieces.iter().map(|p| p.len() - 1).sum();
        // 0.5 → 0.2 (2 segments), then the cycle's 4.
        assert_eq!(segments, 6, "{pieces:?}");
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0][0], (0.5, 0.5));
        // A segment met backwards counts as drawn.
        let back = [(0.0, 0.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0)];
        let pieces = drop_retraced(&back, key);
        assert_eq!(
            pieces,
            vec![vec![(0.0, 0.0), (0.0, 1.0)], vec![(0.0, 0.0), (1.0, 0.0)]]
        );
    }
}
