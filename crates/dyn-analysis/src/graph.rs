//! The graph `y = f(x)` of a one-dimensional map, for the cobweb view
//! (Strogatz §10.1).

use dyn_core::{SystemKind, VectorField};

use crate::nullcline::JUMP_FACTOR;

/// Most samples [`map_graph`] takes across the view.
pub const MAX_GRAPH_SAMPLES: usize = 4096;

/// Bisection levels used to classify a steep interval or to find the edge
/// of `f`'s domain: the edge is found to within 1/4096 of a cell.
const LEVELS: usize = 12;

/// A steep interval whose rise shrank by more than this over the [`LEVELS`]
/// bisections is continuous (halfway, on a log scale, between a smooth
/// curve's 4096 and a jump's 1).
const CONVERGED: f64 = 64.0;

/// The smallest rise that is steep, as a fraction of the view's height
/// (`y1 − y0`). Low, so that a pole is caught in a zoomed-out view, where
/// the samples either side of it are close to 0: a steep but continuous
/// curve is told apart by its shrinking rise, not by this floor.
const FLOOR: f64 = 0.02;

/// A half whose rise is at most this fraction of its parent's shrinks as a
/// continuous curve's does (a smooth one's halves, about 1/2 each).
const SHRINK: f64 = 0.9;

/// Refinement evaluations allowed for one examined interval (two full
/// bisection chains, for a jump on each side of an exact midpoint value,
/// and the rest for the steep halves beside them).
const INTERVAL_BUDGET: usize = 4 * LEVELS;

/// The graph of a one-dimensional map's `f` (evaluated at `n = 0`), sampled
/// at `samples` evenly spaced `x` in `x_range`, both ends included
/// (`samples` is clamped to `2..=`[`MAX_GRAPH_SAMPLES`]).
///
/// Empty unless `f.kind() == Map` and `f.dim() == 1`, or when a range is not
/// finite with `lo < hi`. Each polyline has `x` strictly increasing inside
/// `x_range`; a new one starts at every gap:
///
/// - a non-finite value (outside `f`'s domain): the edge of the domain is
///   found by bisection, so `√x` starts within `cell/4096` of 0;
/// - a pole or a jump. An interval is examined when its rise is more than
///   the threshold `max(JUMP_FACTOR · median rise, (y1 − y0)/50)`, when its
///   rise is more than `JUMP_FACTOR · median rise` (or its step is against
///   both neighbours' and larger than theirs, as at a pole of `tan x` far
///   out) and `f` changes sign across it or its midpoint is larger in
///   magnitude than both ends, or when it is next to a steep one and its midpoint stands out above both
///   ends by the threshold (an even pole midway between two samples). It is
///   bisected up to 12 levels, the more suspicious half first (the one with
///   the larger rise or, when the midpoint is larger in magnitude than both
///   ends, the one towards the larger end, so an even pole like `1/x²` is
///   followed) and then the other. A half is continuous once its rise is
///   below the threshold *and* has shrunk on two successive halvings, as a
///   continuous curve's does (one drop is not enough: a midpoint that lands
///   exactly on a jump, at an intermediate value like `sign(0) = 0`, halves
///   the rise once). A half still rising after 12 levels by more than 1/64
///   of the interval's rise is split there (a jump's rise stays, a pole's
///   grows), and so is one with a non-finite midpoint; the refined points
///   are kept on each side, so the curve reaches towards the pole.
///
/// The values are then clipped to the band `[y0 − Δy, y1 + Δy]`
/// (`Δy = y1 − y0`), with a curve's ends on the band's edge interpolated
/// linearly. Empty when `f` is undefined or outside the band everywhere in
/// view.
///
/// Cost: `samples` evaluations, at most one more per interval to test its
/// midpoint, and at most 48 per examined interval or domain edge, with at
/// most `12 · samples` of those in all.
///
/// ```
/// use dyn_analysis::map_graph;
/// use dyn_core::{FnField, SystemKind};
///
/// let logistic = FnField::new(1, 1, SystemKind::Map, |_n, x, p, out| {
///     out[0] = p[0] * x[0] * (1.0 - x[0])
/// });
/// let g = map_graph(&logistic, &[3.2], (0.0, 1.0), (0.0, 1.0), 101);
/// assert_eq!(g.len(), 1);
/// assert_eq!(g[0].len(), 101);
/// assert_eq!(g[0][50], (0.5, 0.8));
/// ```
pub fn map_graph(
    f: &dyn VectorField,
    p: &[f64],
    x_range: (f64, f64),
    y_range: (f64, f64),
    samples: usize,
) -> Vec<Vec<(f64, f64)>> {
    let Some(raw) = unclipped(f, p, x_range, y_range, samples) else {
        return Vec::new();
    };
    let span = y_range.1 - y_range.0;
    let band = (y_range.0 - span, y_range.1 + span);
    raw.iter()
        .flat_map(|line| clip_to_band(line, band))
        .collect()
}

/// The polylines before the band clip (`None` for a system or range that has
/// no graph). Split out for the tests.
pub(crate) fn unclipped(
    f: &dyn VectorField,
    p: &[f64],
    (x0, x1): (f64, f64),
    (y0, y1): (f64, f64),
    samples: usize,
) -> Option<Vec<Vec<(f64, f64)>>> {
    let valid = |lo: f64, hi: f64| lo.is_finite() && hi.is_finite() && lo < hi;
    if f.kind() != SystemKind::Map || f.dim() != 1 || !valid(x0, x1) || !valid(y0, y1) {
        return None;
    }
    let n = samples.clamp(2, MAX_GRAPH_SAMPLES);
    let mut out = [0.0];
    let mut eval = |x: f64| {
        f.eval(0.0, &[x], p, &mut out);
        out[0]
    };
    let xs: Vec<f64> = (0..n)
        .map(|i| {
            if i == n - 1 {
                x1
            } else {
                x0 + (x1 - x0) * (i as f64 / (n - 1) as f64)
            }
        })
        .collect();
    let ys: Vec<f64> = xs.iter().map(|&x| eval(x)).collect();
    let finite = |i: usize| ys[i].is_finite();

    let rise = |i: usize| (ys[i + 1] - ys[i]).abs();
    let mut rises: Vec<f64> = (0..n - 1)
        .filter(|&i| finite(i) && finite(i + 1))
        .map(rise)
        .collect();
    let median = if rises.is_empty() {
        0.0
    } else {
        let mid = rises.len() / 2;
        *rises.select_nth_unstable_by(mid, f64::total_cmp).1
    };
    let unusual = JUMP_FACTOR * median;
    let threshold = unusual.max(FLOOR * (y1 - y0));
    let steep: Vec<bool> = (0..n - 1)
        .map(|i| finite(i) && finite(i + 1) && rise(i) > threshold)
        .collect();

    let mut refiner = Refiner {
        f: &mut eval,
        budget: LEVELS * n,
        local: 0,
        threshold,
        start: 0.0,
    };
    let mut items: Vec<Item> = Vec::with_capacity(n + 16);
    for i in 0..n {
        items.push(if finite(i) {
            Item::Point(xs[i], ys[i])
        } else {
            Item::Break
        });
        if i + 1 == n {
            break;
        }
        let (a, b, fa, fb) = (xs[i], xs[i + 1], ys[i], ys[i + 1]);
        match (finite(i), finite(i + 1)) {
            (true, true) => {
                let near_steep = (i > 0 && steep[i - 1]) || steep.get(i + 1) == Some(&true);
                // A step against both neighbours' that is larger than
                // theirs: an odd pole in an undersampled view (tan x far out).
                let step = |j: usize| ys[j + 1] - ys[j];
                let against = |j: usize| {
                    j + 1 < n
                        && finite(j)
                        && finite(j + 1)
                        && step(j) * step(i) < 0.0
                        && rise(j) < rise(i)
                };
                let odd = rise(i) > unusual || (i > 0 && against(i - 1) && against(i + 1));
                let examine = steep[i]
                    || (odd && fa * fb < 0.0)
                    || ((near_steep || odd) && {
                        let fm = (refiner.f)(0.5 * (a + b));
                        let above = fm.abs() - fa.abs().max(fb.abs());
                        !fm.is_finite() || (near_steep && above > threshold) || (odd && above > 0.0)
                    });
                if examine {
                    items.extend(refiner.examine((a, fa), (b, fb)));
                }
            }
            (true, false) => {
                refiner.local = INTERVAL_BUDGET;
                push_all(&mut items, &refiner.domain_edge((a, fa), b, LEVELS));
            }
            (false, true) => {
                refiner.local = INTERVAL_BUDGET;
                push_all(&mut items, &refiner.domain_edge((b, fb), a, LEVELS));
            }
            (false, false) => {}
        }
    }

    let mut lines = Vec::new();
    let mut line: Vec<(f64, f64)> = Vec::new();
    for item in items {
        match item {
            Item::Point(x, y) => {
                if line.last().is_none_or(|&(lx, _)| x > lx) {
                    line.push((x, y));
                }
            }
            Item::Break => {
                if line.len() >= 2 {
                    lines.push(std::mem::take(&mut line));
                } else {
                    line.clear();
                }
            }
        }
    }
    if line.len() >= 2 {
        lines.push(line);
    }
    Some(lines)
}

/// What goes into the polylines, in increasing `x`.
#[derive(Debug, Clone, Copy)]
enum Item {
    Point(f64, f64),
    Break,
}

/// Bisects the intervals [`unclipped`] examines, within its budgets.
struct Refiner<'e, E> {
    f: &'e mut E,
    /// Evaluations left for the whole graph.
    budget: usize,
    /// Evaluations left for the current interval.
    local: usize,
    /// The graph's steepness threshold.
    threshold: f64,
    /// The current interval's largest rise on its first level.
    start: f64,
}

impl<E: FnMut(f64) -> f64> Refiner<'_, E> {
    /// `f(x)`, or `None` once a budget is spent.
    fn eval(&mut self, x: f64) -> Option<f64> {
        if self.budget == 0 || self.local == 0 {
            return None;
        }
        self.budget -= 1;
        self.local -= 1;
        Some((self.f)(x))
    }

    /// Classifies the interval `(a, b)` and returns its refined points in
    /// order, with a break where the curve is split.
    fn examine(&mut self, a: (f64, f64), b: (f64, f64)) -> Vec<Item> {
        self.local = INTERVAL_BUDGET;
        self.start = (b.1 - a.1).abs();
        self.refine(a, b, 0, f64::INFINITY, self.threshold)
    }

    /// The points strictly inside `(a, b)` (whose parent interval rose by
    /// `parent`), at bisection level `depth`, with `t` the rise below which
    /// a shrinking half is continuous.
    fn refine(
        &mut self,
        (a, fa): (f64, f64),
        (b, fb): (f64, f64),
        depth: usize,
        parent: f64,
        t: f64,
    ) -> Vec<Item> {
        let rise = (fb - fa).abs();
        let small = depth > 0 && rise <= t && rise <= SHRINK * parent;
        if small && rise <= 0.25 * t {
            return Vec::new();
        }
        let m = 0.5 * (a + b);
        if depth == LEVELS || !(a < m && m < b) {
            // Still rising at 1/4096 of a cell. A continuous curve's rise
            // shrinks with the interval (by about 4096 when it is smooth
            // there, as next to a pole), a jump's stays and a pole's grows:
            // split unless it has shrunk by more than CONVERGED.
            return if !small && rise * CONVERGED > self.start {
                vec![Item::Break]
            } else {
                Vec::new()
            };
        }
        let Some(fm) = self.eval(m) else {
            return Vec::new();
        };
        if !fm.is_finite() {
            // A gap inside the interval: find the domain's edge on each side
            // with the levels that are left, then split.
            let rest = LEVELS - depth - 1;
            let mut out = Vec::new();
            push_all(&mut out, &self.domain_edge((a, fa), m, rest));
            out.push(Item::Break);
            push_all(&mut out, &self.domain_edge((b, fb), m, rest));
            return out;
        }
        let (rise_l, rise_r) = ((fm - fa).abs(), (fb - fm).abs());
        let t = if depth == 0 {
            // A pole-like midpoint can rise more than the interval does.
            self.start = rise.max(rise_l).max(rise_r);
            t.min(0.5 * self.start)
        } else {
            t
        };
        if small && rise_l.max(rise_r) <= SHRINK * rise {
            // Only steep: shrinking on two successive halvings.
            return vec![Item::Point(m, fm)];
        }
        let pole_like = fm.abs() > fa.abs().max(fb.abs());
        let left_first = if pole_like && fa.abs() != fb.abs() {
            fa.abs() > fb.abs()
        } else {
            rise_l >= rise_r
        };
        let (left, right) = if left_first {
            let left = self.refine((a, fa), (m, fm), depth + 1, rise, t);
            (left, self.refine((m, fm), (b, fb), depth + 1, rise, t))
        } else {
            let right = self.refine((m, fm), (b, fb), depth + 1, rise, t);
            (self.refine((a, fa), (m, fm), depth + 1, rise, t), right)
        };
        let mut out = left;
        out.push(Item::Point(m, fm));
        out.extend(right);
        out
    }

    /// Bisects between the finite point `inside` and the non-finite `x = out`
    /// for `levels` levels; returns the finite midpoints in increasing `x`.
    fn domain_edge(&mut self, inside: (f64, f64), out: f64, levels: usize) -> Vec<(f64, f64)> {
        let (mut fin, mut bad) = (inside.0, out);
        let mut found = Vec::new();
        for _ in 0..levels {
            let m = 0.5 * (fin + bad);
            if m == fin || m == bad {
                break;
            }
            let Some(fm) = self.eval(m) else { break };
            if fm.is_finite() {
                found.push((m, fm));
                fin = m;
            } else {
                bad = m;
            }
        }
        // Moving towards `out`: increasing x when out is to the right.
        if out < inside.0 {
            found.reverse();
        }
        found
    }
}

fn push_all(items: &mut Vec<Item>, pts: &[(f64, f64)]) {
    items.extend(pts.iter().map(|&(x, y)| Item::Point(x, y)));
}

/// Clips a polyline to `lo <= y <= hi`, interpolating the crossings; the
/// curve splits where it leaves the band and comes back.
fn clip_to_band(line: &[(f64, f64)], (lo, hi): (f64, f64)) -> Vec<Vec<(f64, f64)>> {
    let inside = |y: f64| (lo..=hi).contains(&y);
    let cross = |p: (f64, f64), q: (f64, f64), y: f64| {
        let t = (y - p.1) / (q.1 - p.1);
        (p.0 + t * (q.0 - p.0), y)
    };
    let edge = |y: f64| if y > hi { hi } else { lo };
    let mut out = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let push = |cur: &mut Vec<(f64, f64)>, pt: (f64, f64)| {
        if cur.last().is_none_or(|&(x, _)| pt.0 > x) {
            cur.push(pt);
        }
    };
    let finish = |cur: &mut Vec<(f64, f64)>, out: &mut Vec<Vec<(f64, f64)>>| {
        if cur.len() >= 2 {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    if let Some(&first) = line.first() {
        if inside(first.1) {
            cur.push(first);
        }
    }
    for w in line.windows(2) {
        let (p, q) = (w[0], w[1]);
        match (inside(p.1), inside(q.1)) {
            (true, true) => push(&mut cur, q),
            (true, false) => {
                push(&mut cur, cross(p, q, edge(q.1)));
                finish(&mut cur, &mut out);
            }
            (false, true) => {
                push(&mut cur, cross(p, q, edge(p.1)));
                push(&mut cur, q);
            }
            (false, false) => {
                if (p.1 > hi) != (q.1 > hi) {
                    // Straight through the band.
                    push(&mut cur, cross(p, q, edge(p.1)));
                    push(&mut cur, cross(p, q, edge(q.1)));
                    finish(&mut cur, &mut out);
                }
            }
        }
    }
    finish(&mut cur, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dyn_core::FnField;

    fn map(g: fn(f64) -> f64) -> FnField {
        FnField::new(1, 0, SystemKind::Map, move |_n, x, _p, out| {
            out[0] = g(x[0])
        })
    }

    #[test]
    fn the_band_clip_interpolates_and_splits() {
        let line = [(0.0, 0.0), (1.0, 2.0), (2.0, 0.0), (3.0, -4.0), (4.0, 4.0)];
        let got = clip_to_band(&line, (-1.0, 1.0));
        assert_eq!(
            got,
            vec![
                vec![(0.0, 0.0), (0.5, 1.0)],
                vec![(1.5, 1.0), (2.0, 0.0), (2.25, -1.0)],
                vec![(3.375, -1.0), (3.625, 1.0)],
            ]
        );
        assert!(clip_to_band(&[(0.0, 5.0), (1.0, 6.0)], (-1.0, 1.0)).is_empty());
        // A point on the edge counts as inside and is not repeated.
        assert_eq!(
            clip_to_band(&[(0.0, 1.0), (1.0, 3.0)], (-1.0, 1.0)),
            Vec::<Vec<(f64, f64)>>::new()
        );
    }

    #[test]
    fn one_over_x_reaches_towards_its_pole_before_the_clip() {
        let f = map(|x| 1.0 / x);
        let raw = unclipped(&f, &[], (-1.0, 1.0), (-1.0, 1.0), 2048).unwrap();
        assert_eq!(
            raw.len(),
            2,
            "{:?}",
            raw.iter().map(Vec::len).collect::<Vec<_>>()
        );
        let cell = 2.0 / 2047.0;
        let left_end = raw[0].last().unwrap().0;
        let right_start = raw[1][0].0;
        assert!(
            left_end < 0.0 && -left_end <= cell / 4096.0 * 1.0001,
            "{left_end}"
        );
        assert!(
            right_start > 0.0 && right_start <= cell / 4096.0 * 1.0001,
            "{right_start}"
        );
    }

    #[test]
    fn an_even_pole_between_two_samples_splits() {
        // Samples at ±1/2047 straddle the pole with equal values.
        let f = map(|x| 1.0 / (x * x));
        let raw = unclipped(&f, &[], (-1.0, 1.0), (-10.0, 10.0), 2048).unwrap();
        assert_eq!(raw.len(), 2);
        assert!(raw[0].iter().all(|&(x, _)| x < 0.0));
        assert!(raw[1].iter().all(|&(x, _)| x > 0.0));
    }

    #[test]
    fn a_jump_splits_but_a_steep_curve_does_not() {
        let step = map(|x| if x < 0.3 { 0.0 } else { 1.0 });
        let raw = unclipped(&step, &[], (0.0, 1.0), (0.0, 1.0), 100).unwrap();
        assert_eq!(raw.len(), 2);
        assert!(raw[0].last().unwrap().0 > 0.3 - 1.0 / 99.0 / 4000.0);
        let steep = map(|x| (1000.0 * (x - 0.3)).tanh());
        let raw = unclipped(&steep, &[], (0.0, 1.0), (-1.0, 1.0), 100).unwrap();
        assert_eq!(raw.len(), 1);
        assert!(raw[0].len() > 100, "the refined points are kept");
        assert!(raw[0].windows(2).all(|w| w[1].0 > w[0].0));
    }

    #[test]
    fn a_smooth_peak_narrower_than_a_cell_is_not_a_pole() {
        let f = map(|x| 1.0 / ((x - 0.5) * (x - 0.5) + 1e-6));
        let raw = unclipped(&f, &[], (0.0, 1.0), (0.0, 2e6), 101).unwrap();
        assert_eq!(raw.len(), 1);
    }
}
