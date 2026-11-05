//! Clipping polylines to a rectangle (Liang–Barsky), so a figure never
//! hands TeX a coordinate far outside the plot ("Dimension too large").
//!
//! The rectangle is the view grown by a margin, so line caps and joins at
//! the edge look the same as on screen. Each segment is clipped on its own;
//! where the curve leaves the rectangle the polyline is split, so a curve
//! that leaves and comes back is drawn as two pieces with no chord across
//! the outside.

/// A rectangle `[x0, x1] × [y0, y1]` in plot coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left.
    pub x0: f64,
    /// Right.
    pub x1: f64,
    /// Bottom.
    pub y0: f64,
    /// Top.
    pub y1: f64,
}

impl Rect {
    /// Whether `p` is inside (edges included).
    pub fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.x0 && p.0 <= self.x1 && p.1 >= self.y0 && p.1 <= self.y1
    }
}

/// The part of segment `a`–`b` inside `r`, or `None`.
pub fn clip_segment(r: &Rect, a: (f64, f64), b: (f64, f64)) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    // Which edge each end was cut at: 0 left, 1 right, 2 bottom, 3 top.
    let mut e0: Option<usize> = None;
    let mut e1: Option<usize> = None;
    for (edge, (p, q)) in [
        (-dx, a.0 - r.x0),
        (dx, r.x1 - a.0),
        (-dy, a.1 - r.y0),
        (dy, r.y1 - a.1),
    ]
    .into_iter()
    .enumerate()
    {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t0 {
                    t0 = t;
                    e0 = Some(edge);
                }
            } else if t < t1 {
                t1 = t;
                e1 = Some(edge);
            }
        }
        if t0 > t1 {
            return None;
        }
    }
    // The crossing, put exactly on the edge it crosses (with far-away
    // points the interpolation alone can land off it).
    let at = |t: f64, edge: Option<usize>, end: (f64, f64)| {
        let Some(edge) = edge else {
            return end;
        };
        let mut p = (
            (a.0 + t * dx).clamp(r.x0, r.x1),
            (a.1 + t * dy).clamp(r.y0, r.y1),
        );
        match edge {
            0 => p.0 = r.x0,
            1 => p.0 = r.x1,
            2 => p.1 = r.y0,
            _ => p.1 = r.y1,
        }
        p
    };
    Some((at(t0, e0, a), at(t1, e1, b)))
}

/// The pieces of `poly` inside `r`. Points inside are kept exactly; a
/// point where the curve crosses the edge is added at the crossing.
pub fn clip_polyline(r: &Rect, poly: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
    let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    if poly.len() == 1 {
        if r.contains(poly[0]) {
            out.push(vec![poly[0]]);
        }
        return out;
    }
    for w in poly.windows(2) {
        match clip_segment(r, w[0], w[1]) {
            Some((p, q)) => {
                if cur.last() != Some(&p) {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.push(p);
                }
                if q != p || cur.len() == 1 {
                    cur.push(q);
                }
                if q != w[1] {
                    // Left the rectangle.
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.retain(|p| p.len() >= 2);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect {
        x0: 0.0,
        x1: 10.0,
        y0: 0.0,
        y1: 10.0,
    };

    #[test]
    fn inside_points_are_kept_and_crossings_added() {
        let poly = [
            (1.0, 1.0),
            (5.0, 5.0),
            (15.0, 5.0),
            (20.0, 20.0),
            (15.0, 5.0),
            (5.0, 9.0),
        ];
        let pieces = clip_polyline(&R, &poly);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0], [(1.0, 1.0), (5.0, 5.0), (10.0, 5.0)]);
        assert_eq!(pieces[1][0].0, 10.0);
        assert_eq!(*pieces[1].last().unwrap(), (5.0, 9.0));
        for p in pieces.iter().flatten() {
            assert!(R.contains(*p));
        }
    }

    #[test]
    fn a_segment_crossing_the_whole_box_is_cut_at_both_edges() {
        let pieces = clip_polyline(&R, &[(-1e300, 5.0), (1e300, 5.0)]);
        assert_eq!(pieces, [vec![(0.0, 5.0), (10.0, 5.0)]]);
        assert!(clip_polyline(&R, &[(-5.0, -5.0), (-1.0, 20.0)]).is_empty());
        let all_in = [(1.0, 1.0), (2.0, 2.0), (3.0, 1.0)];
        assert_eq!(clip_polyline(&R, &all_in), [all_in.to_vec()]);
    }
}
