//! Where a trajectory's arrow heads go: an exact port of `drawHeads`
//! (`app/src/canvas/draw.ts`), so the figure puts them where the screen does.
//!
//! - No head when the length of the run inside the view is under `3·L`.
//! - One head up to 600 px in view, then one per 300 px, at most
//!   `max_heads` (12, or 1 when the system has 3 or more states).
//! - Each head starts in the middle of its share of the run and may slide
//!   by a sixth of that share to a straighter spot (bend measured over
//!   ±12 px), but not within `min_gap` of a head already placed.
//!
//! `tests/vectors/heads.json` (generated from the TypeScript through a
//! recording context) holds cases both test suites check.

use crate::model::Viewport;

/// Pixels of trajectory per arrow head once a trajectory is long enough for several.
const HEAD_SPACING_PX: f64 = 300.0;
/// Trajectories shorter than this (in view) get a single head.
const SINGLE_HEAD_MAX_PX: f64 = 600.0;
/// Half-length of the stretch over which a head's bend is measured.
const BEND_WINDOW_PX: f64 = 12.0;
/// A spot must be straighter by this many radians to move a head off the middle.
const STRAIGHTNESS_TOL: f64 = 0.05;
/// A head may slide this fraction of its share of the trajectory either way.
const HEAD_SLIDE_FRACTION: f64 = 1.0 / 6.0;

/// Head length of an unselected trajectory at glyph scale 1, in px.
pub const HEAD_LENGTH_PX: f64 = 8.0;
/// Closest two heads may be, at glyph scale 1, in px.
pub const MIN_GAP_PX: f64 = 24.0;

/// Most heads per trajectory for a system of `dim` states.
pub fn max_heads(dim: usize) -> usize {
    if dim >= 3 {
        1
    } else {
        12
    }
}

/// One arrow head: the point it sits on and the unit direction it points
/// (pixels, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Head {
    /// Horizontal pixel of the head's centre.
    pub x: f64,
    /// Vertical pixel of the head's centre.
    pub y: f64,
    /// Unit direction, horizontal.
    pub ux: f64,
    /// Unit direction, vertical (down).
    pub uy: f64,
}

impl Head {
    /// The head's triangle for length `l`: tip, then the two base corners.
    pub fn triangle(&self, l: f64) -> [(f64, f64); 3] {
        let hw = 0.45 * l;
        let (x, y, ux, uy) = (self.x, self.y, self.ux, self.uy);
        [
            (x + ux * l / 2.0, y + uy * l / 2.0),
            (x - ux * l / 2.0 - uy * hw, y - uy * l / 2.0 + ux * hw),
            (x - ux * l / 2.0 + uy * hw, y - uy * l / 2.0 - ux * hw),
        ]
    }
}

/// The heads `drawHeads` draws on the pixel polyline `pts` (already cut at
/// its first non-finite point and clamped to ±1e5 px, as `trajectoryPx`
/// gives it). `placed` holds the heads already drawn on earlier
/// trajectories (in drawing order) and gains the new ones.
#[allow(clippy::too_many_arguments)]
pub fn place(
    pts: &[(f64, f64)],
    vp: &Viewport,
    backward: bool,
    l: f64,
    max_heads: usize,
    placed: &mut Vec<(f64, f64)>,
    min_gap: f64,
) -> Vec<Head> {
    let mut out = Vec::new();
    let n = pts.len();
    if n < 2 {
        return out;
    }
    let right = vp.left + vp.width;
    let bottom = vp.top + vp.height;
    let mut acc = vec![0.0; n];
    let mut total = 0.0;
    for k in 1..n {
        let (x, y) = pts[k];
        if x >= vp.left && x <= right && y >= vp.top && y <= bottom {
            total += crate::geom::len(x - pts[k - 1].0, y - pts[k - 1].1);
        }
        acc[k] = total;
    }
    if total < 3.0 * l {
        return out;
    }
    let at = |t: f64| -> usize {
        if t <= 0.0 {
            return 0;
        }
        if t >= total {
            return n - 1;
        }
        let (mut lo, mut hi) = (0usize, n - 1);
        while lo < hi {
            let mid = (lo + hi) >> 1;
            if acc[mid] < t {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    };
    let turn = |k: usize| -> f64 {
        let a = at(acc[k] - BEND_WINDOW_PX);
        let b = at(acc[k] + BEND_WINDOW_PX);
        if a >= k || b <= k {
            return f64::INFINITY;
        }
        let t1 = (pts[k].1 - pts[a].1).atan2(pts[k].0 - pts[a].0);
        let t2 = (pts[b].1 - pts[k].1).atan2(pts[b].0 - pts[k].0);
        (t2 - t1).sin().atan2((t2 - t1).cos()).abs()
    };
    let count = if total > SINGLE_HEAD_MAX_PX {
        (total / HEAD_SPACING_PX).floor() as usize
    } else {
        1
    }
    .min(max_heads);
    let stretch = total / count as f64;
    let gap2 = min_gap * min_gap;
    for h in 0..count {
        let mid = (h as f64 + 0.5) * stretch;
        let mut k = at(mid);
        let mut best = turn(k);
        let slide = stretch * HEAD_SLIDE_FRACTION;
        let mut t = mid - slide;
        while t <= mid + slide {
            let c = at(t);
            let v = turn(c) + 1e-3 * (t - mid).abs();
            if v < best - STRAIGHTNESS_TOL {
                best = v;
                k = c;
            }
            t += 4.0;
        }
        if k < 1 {
            continue;
        }
        let (x, y) = pts[k];
        if x < vp.left || x > right || y < vp.top || y > bottom {
            continue;
        }
        if placed
            .iter()
            .any(|&(px, py)| (px - x).powi(2) + (py - y).powi(2) < gap2)
        {
            continue;
        }
        let mut j = k - 1;
        while j > 0 && crate::geom::len(x - pts[j].0, y - pts[j].1) < 1.0 {
            j -= 1;
        }
        let dx = x - pts[j].0;
        let dy = y - pts[j].1;
        let m = crate::geom::len(dx, dy);
        if m == 0.0 {
            continue;
        }
        placed.push((x, y));
        let sign = if backward { -1.0 } else { 1.0 };
        out.push(Head {
            x,
            y,
            ux: sign * dx / m,
            uy: sign * dy / m,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp() -> Viewport {
        Viewport::new(800.0, 600.0, (0.0, 8.0), (0.0, 6.0))
    }

    fn line(n: usize, len: f64) -> Vec<(f64, f64)> {
        (0..n)
            .map(|i| (10.0 + len * i as f64 / (n - 1) as f64, 300.0))
            .collect()
    }

    #[test]
    fn one_head_in_the_middle_of_a_short_run() {
        let mut placed = Vec::new();
        let heads = place(&line(101, 400.0), &vp(), false, 8.0, 12, &mut placed, 24.0);
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].x, 210.0);
        assert_eq!((heads[0].ux, heads[0].uy), (1.0, 0.0));
        assert_eq!(placed, [(210.0, 300.0)]);
        let back = place(
            &line(101, 400.0),
            &vp(),
            true,
            8.0,
            12,
            &mut Vec::new(),
            24.0,
        );
        assert_eq!((back[0].ux, back[0].uy), (-1.0, 0.0));
    }

    #[test]
    fn no_head_under_three_lengths_in_view() {
        let heads = place(
            &line(11, 23.0),
            &vp(),
            false,
            8.0,
            12,
            &mut Vec::new(),
            24.0,
        );
        assert!(heads.is_empty());
        // Length outside the view does not count.
        let off: Vec<(f64, f64)> = (0..100).map(|i| (-500.0 + i as f64, 300.0)).collect();
        assert!(place(&off, &vp(), false, 8.0, 12, &mut Vec::new(), 24.0).is_empty());
    }

    #[test]
    fn long_runs_get_one_head_per_300_px_capped() {
        let pts = line(1001, 780.0);
        let heads = place(&pts, &vp(), false, 8.0, 12, &mut Vec::new(), 24.0);
        assert_eq!(heads.len(), 2);
        assert_eq!(heads[0].x, 205.0);
        assert_eq!(heads[1].x, 595.0);
        let one = place(&pts, &vp(), false, 8.0, max_heads(3), &mut Vec::new(), 24.0);
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn crowded_heads_are_skipped() {
        let mut placed = vec![(212.0, 302.0)];
        let heads = place(&line(101, 400.0), &vp(), false, 8.0, 12, &mut placed, 24.0);
        assert!(heads.is_empty());
        assert_eq!(placed.len(), 1);
    }

    #[test]
    fn a_head_slides_off_a_bend_to_a_straighter_spot() {
        // A right angle at the middle: the head moves to a straight part.
        let mut pts: Vec<(f64, f64)> = (0..=100).map(|i| (100.0 + 2.0 * i as f64, 100.0)).collect();
        pts.extend((1..=100).map(|i| (300.0, 100.0 + 2.0 * i as f64)));
        let heads = place(&pts, &vp(), false, 8.0, 12, &mut Vec::new(), 24.0);
        assert_eq!(heads.len(), 1);
        let h = heads[0];
        assert!(h.x != 300.0 || h.y != 100.0, "{h:?}");
        assert!((h.ux.abs() - 1.0).abs() < 1e-12 || (h.uy.abs() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn triangle_matches_the_screen() {
        let h = Head {
            x: 10.0,
            y: 20.0,
            ux: 1.0,
            uy: 0.0,
        };
        assert_eq!(h.triangle(8.0), [(14.0, 20.0), (6.0, 23.6), (6.0, 16.4)]);
    }
}
