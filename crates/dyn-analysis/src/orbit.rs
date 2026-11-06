use dyn_core::{SystemKind, VectorField};
use dyn_solve::BLOW_UP_THRESHOLD;

/// What to sweep and what to keep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitDiagramOptions {
    /// Index of the swept parameter in `p`.
    pub param: usize,
    /// `(lo, hi)`: the first and last parameter value, both included.
    pub range: (f64, f64),
    /// Parameter values (columns) across `range`.
    pub columns: usize,
    /// Index of the state component recorded.
    pub state: usize,
    /// Iterates discarded before recording.
    pub transient: usize,
    /// Iterates recorded per column.
    pub kept: usize,
}

/// An orbit diagram: `kept` iterates of one state for each parameter value.
#[derive(Debug, Clone, PartialEq)]
pub struct OrbitDiagram {
    /// The parameter values, `r_j = lo + j·step` with
    /// `step = (hi − lo)/(columns − 1)`: one multiplication and one addition,
    /// so the same bits on every platform. One column is `[lo]`.
    pub r: Vec<f64>,
    /// `columns × kept` values, column by column: column `j` is
    /// `x[j·kept .. (j + 1)·kept]`, the iterates `x_T … x_{T+kept−1}` (with
    /// `x_0 = x0` and `T` the transient). NaN from the first iterate that
    /// leaves `|x_i| ≤ BLOW_UP_THRESHOLD` (or is not finite) on.
    pub x: Vec<f64>,
    /// Iterates per column.
    pub kept: usize,
}

impl OrbitDiagram {
    /// The kept iterates of column `j`.
    pub fn column(&self, j: usize) -> &[f64] {
        &self.x[j * self.kept..(j + 1) * self.kept]
    }
}

/// A period-doubling bifurcation: the `period`-cycle loses stability at `r`
/// (its multiplier crosses −1) and a `2·period`-cycle is born.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Doubling {
    /// Parameter value of the bifurcation.
    pub r: f64,
    /// Period of the cycle that loses stability.
    pub period: usize,
}

/// Iterates `f` (a map, `x_{n+1} = f(n, x_n)`) from `x0` at every parameter
/// value of `opts.range`, sequentially, with `p[opts.param]` replaced by
/// the column's value and every other parameter as in `p`. The caller
/// checks that `f` is a map and that the indices and `x0` fit it.
///
/// ```
/// use dyn_analysis::{orbit_diagram, OrbitDiagramOptions};
/// use dyn_core::{FnField, SystemKind};
///
/// let logistic = FnField::new(1, 1, SystemKind::Map, |_n, x, p, out| {
///     out[0] = p[0] * x[0] * (1.0 - x[0])
/// });
/// let opts = OrbitDiagramOptions { param: 0, range: (2.5, 2.5), columns: 1, state: 0,
///                                  transient: 200, kept: 4 };
/// let d = orbit_diagram(&logistic, &[0.0], &[0.1], &opts);
/// assert!(d.column(0).iter().all(|x| (x - 0.6).abs() < 1e-12)); // x* = 1 − 1/r
/// ```
pub fn orbit_diagram(
    f: &dyn VectorField,
    p: &[f64],
    x0: &[f64],
    opts: &OrbitDiagramOptions,
) -> OrbitDiagram {
    let (lo, hi) = opts.range;
    let step = if opts.columns > 1 {
        (hi - lo) / (opts.columns - 1) as f64
    } else {
        0.0
    };
    let r: Vec<f64> = (0..opts.columns).map(|j| lo + j as f64 * step).collect();
    let kept = opts.kept;
    let mut x = vec![f64::NAN; opts.columns * kept];
    let mut params = p.to_vec();
    let mut state = x0.to_vec();
    let mut next = vec![0.0; x0.len()];
    let end = opts.transient + kept;
    for (&rj, out) in r.iter().zip(x.chunks_exact_mut(kept.max(1))) {
        params[opts.param] = rj;
        state.copy_from_slice(x0);
        for k in 0..end {
            if k >= opts.transient {
                out[k - opts.transient] = state[opts.state];
            }
            if k + 1 == end {
                break;
            }
            f.eval(k as f64, &state, &params, &mut next);
            if !next.iter().all(|v| v.abs() <= BLOW_UP_THRESHOLD) {
                break;
            }
            std::mem::swap(&mut state, &mut next);
        }
    }
    OrbitDiagram { r, x, kept }
}

const PERIOD_TOL: f64 = 1e-7;

const R_TOL: f64 = 1e-12;

const X_TOL: f64 = 1e-14;

const NEWTON_ITERS: usize = 50;

/// The period-doubling points of a one-dimensional autonomous map found in
/// `diagram` (made by [`orbit_diagram`] with the same `f`, `p` and `param`),
/// in increasing `r`, at most `max` of them.
///
/// Each column gets the smallest period `P ∈ {1, 2, 4, …}` (with `2P` at
/// most `kept`) for which every kept iterate agrees with the one `P` later,
/// `|x_{n+P} − x_n| ≤ 1e−7·(1 + |x_n|)`; columns with none are skipped. A
/// column of period `P` followed by one of period `2P` marks a doubling of
/// the `P`-cycle. The label alone doesn't bracket it: just below the
/// bifurcation the orbit still alternates about the `P`-cycle, decaying so
/// slowly that it repeats at lag `2P` within the tolerance, so a few columns
/// before the bifurcation already read `2P`. The `P`-cycle is followed
/// instead, column by column from the one before the last `P` column, by
/// Newton's method on `f^P(x) = x` (seeded with that column's last kept
/// iterate, then with the previous column's cycle point), until its
/// multiplier `μ_P(r) = ∏ f′(x_i)` (from `VectorField::jacobian`) crosses
/// −1. The two columns either side bracket the bifurcation, found by
/// bisection on `μ_P + 1` to a bracket of 1e−12. A doubling is skipped when
/// `μ_P + 1 ≤ 0` already at the first column, when it never crosses −1, or
/// when Newton fails.
///
/// Empty unless `f` is a one-dimensional autonomous map.
pub fn period_doublings(
    f: &dyn VectorField,
    p: &[f64],
    param: usize,
    diagram: &OrbitDiagram,
    max: usize,
) -> Vec<Doubling> {
    if f.kind() != SystemKind::Map || f.dim() != 1 || !f.is_autonomous() {
        return Vec::new();
    }
    let mut params = p.to_vec();
    let mut multiplier = |r: f64, x: &mut f64, period: usize| {
        params[param] = r;
        let cycle = cycle(f, &params, *x, period)?;
        *x = cycle.point;
        Some(cycle.multiplier)
    };
    let mut found = Vec::new();
    let mut previous: Option<(usize, usize)> = None;
    for j in 0..diagram.r.len() {
        if found.len() >= max {
            break;
        }
        let Some(period) = period_of(diagram.column(j)) else {
            continue;
        };
        if let Some((a, before)) = previous.filter(|&(_, before)| period == 2 * before) {
            let first = a.saturating_sub(1);
            let mut x = diagram.column(a)[diagram.kept - 1];
            let mut bracket = None;
            let mut stable = None;
            for &r in &diagram.r[first..] {
                let Some(mu) = multiplier(r, &mut x, before) else {
                    break;
                };
                if mu + 1.0 > 0.0 {
                    stable = Some((r, x));
                } else {
                    bracket = stable.map(|(lo, x_lo)| (lo, r, x_lo));
                    break;
                }
            }
            if let Some((mut lo, mut hi, mut x)) = bracket {
                let mut ok = true;
                while ok && hi - lo > R_TOL {
                    let mid = 0.5 * (lo + hi);
                    match multiplier(mid, &mut x, before) {
                        Some(mu) if mu + 1.0 > 0.0 => lo = mid,
                        Some(_) => hi = mid,
                        None => ok = false,
                    }
                }
                if ok {
                    found.push(Doubling {
                        r: 0.5 * (lo + hi),
                        period: before,
                    });
                }
            }
        }
        previous = Some((j, period));
    }
    found
}

fn period_of(column: &[f64]) -> Option<usize> {
    let mut period = 1;
    while 2 * period <= column.len() {
        let repeats = column
            .iter()
            .zip(&column[period..])
            .all(|(a, b)| (b - a).abs() <= PERIOD_TOL * (1.0 + a.abs()));
        if repeats {
            return Some(period);
        }
        period *= 2;
    }
    None
}

struct Cycle {
    point: f64,
    multiplier: f64,
}

fn cycle(f: &dyn VectorField, p: &[f64], seed: f64, period: usize) -> Option<Cycle> {
    let mut x = seed;
    for _ in 0..NEWTON_ITERS {
        let fp = iterated(f, p, x, period);
        let dx = (fp.value - x) / (fp.derivative - 1.0);
        x -= dx;
        if !x.is_finite() {
            return None;
        }
        if dx.abs() <= X_TOL * (1.0 + x.abs()) {
            let multiplier = iterated(f, p, x, period).derivative;
            return multiplier.is_finite().then_some(Cycle {
                point: x,
                multiplier,
            });
        }
    }
    None
}

struct Iterated {
    value: f64,
    derivative: f64,
}

fn iterated(f: &dyn VectorField, p: &[f64], x: f64, period: usize) -> Iterated {
    let (mut x, mut derivative) = ([x], 1.0);
    let (mut next, mut slope) = ([0.0], [0.0]);
    for _ in 0..period {
        f.jacobian(0.0, &x, p, &mut slope);
        f.eval(0.0, &x, p, &mut next);
        derivative *= slope[0];
        x = next;
    }
    Iterated {
        value: x[0],
        derivative,
    }
}
