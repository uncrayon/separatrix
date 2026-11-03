//! Fixed points: multi-start damped Newton, deduplication, classification.
//!
//! For a flow we solve `F(x) = f(x) = 0`; for a map `F(x) = f(x) - x = 0`.
//!
//! 1. **Seeds**: a regular grid of `seeds_per_axis^dim` cell centres of the
//!    search box when that is at most [`MAX_SEEDS`], otherwise the first
//!    [`MAX_SEEDS`] points of a Halton sequence (a well-spread
//!    quasi-random sequence) in the box.
//! 2. **Newton**: from each seed, solve `DF(x) δ = -F(x)` by LU with
//!    partial pivoting, then *backtrack*: halve the step until `‖F‖`
//!    decreases enough (Armijo condition). Stop when the step is tiny
//!    relative to `‖x‖`. Accept the result when, at the final point `x`,
//!    either every `|F_i(x)|` is small compared with the size of the terms
//!    of `F_i` there, or the Newton correction is negligible (see
//!    [`RESIDUAL_TOL`] and [`NEWTON_STEP_TOL`]). Neither test depends on the
//!    seed or the search box, so zooming out cannot create fixed points.
//! 3. **Filter**: keep points inside the box enlarged by 5% per side, and
//!    merge points closer than `1e-6` of the box diagonal (keeping the one
//!    with the smallest residual).
//! 4. **Classify** with the eigenvalues of `J = Df` (see [`crate::classify`]).

use dyn_core::{SystemKind, VectorField};
use serde::{Deserialize, Serialize};

use crate::classify::{classify_jacobian, Classification};
use crate::linalg::lu_solve;

/// Maximum number of Newton starting points.
pub const MAX_SEEDS: usize = 4096;
/// Maximum Newton iterations per seed.
const MAX_ITER: usize = 100;
/// Newton stops when `‖δ‖∞ ≤ STEP_TOL · (1 + ‖x‖∞)`.
const STEP_TOL: f64 = 1e-14;
/// Residual test: a point `x` is accepted when for every component
/// `|F_i(x)| ≤ RESIDUAL_TOL · max(1, s_i)`, where
/// `s_i = Σ_j |∂f_i/∂x_j| |x_j|` (plus `|x_i|` for maps) estimates the size
/// of the terms that cancel in `F_i` at `x` (exactly so for linear and
/// homogeneous terms).
const RESIDUAL_TOL: f64 = 1e-9;
/// Convergence test: a point `x` is also accepted when the full Newton
/// correction there is negligible, `‖DF(x)⁻¹ F(x)‖∞ ≤ NEWTON_STEP_TOL ·
/// (1 + ‖x‖∞)`. This catches roots where rounding keeps `|F|` above the
/// residual test, e.g. a double root of a function with large values. Near
/// a minimum of `|F|` that is not a root (the "ghost" of a saddle-node,
/// `x' = r + x²` with `r > 0`) the correction is at least `√r`, so ghosts
/// with `r` above about `1e-12` are rejected.
const NEWTON_STEP_TOL: f64 = 1e-6;
/// Points closer than this fraction of the box diagonal are the same point.
const DEDUP_REL: f64 = 1e-6;
/// Fraction of the box width by which points may lie outside the box.
const BOX_MARGIN: f64 = 0.05;
/// Newton gives up when an iterate is this many box widths from the box.
const ESCAPE_WIDTHS: f64 = 10.0;

/// A fixed point with its linearisation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixedPoint {
    /// Location (full state vector).
    pub x: Vec<f64>,
    /// Eigenvalues `(re, im)` of the Jacobian (flow) or of `Df` (map).
    ///
    /// Empty when the Jacobian at the point is not finite (e.g. `x' = ∛x`
    /// at 0, where the slope is infinite): there is no linearisation, and
    /// the point is classified [`Classification::NonHyperbolic`] (the
    /// linearisation cannot decide its type). See [`classify_jacobian`].
    pub eigenvalues: Vec<(f64, f64)>,
    /// Type of the fixed point.
    pub classification: Classification,
    /// `true` if the point is asymptotically stable (sink-like).
    pub stable: bool,
}

/// Finds the fixed points of `f` in the box `search` (one `(min, max)` per
/// state variable) by multi-start Newton. See the module docs.
///
/// Flows: `f(x) = 0`; maps: `f(x) = x`. Both are evaluated at `t = 0`.
/// Returns an empty `Vec` for non-autonomous systems (their fixed points
/// would move with time), when `search.len() != f.dim()`, or when
/// `seeds_per_axis == 0`. Results are sorted by coordinates.
///
/// A whole curve of fixed points (e.g. `x' = 0`) yields one point per seed
/// that lands on it.
pub fn fixed_points(
    f: &dyn VectorField,
    p: &[f64],
    search: &[(f64, f64)],
    seeds_per_axis: usize,
) -> Vec<FixedPoint> {
    let n = f.dim();
    if !f.is_autonomous() || n == 0 || search.len() != n || seeds_per_axis == 0 {
        return Vec::new();
    }
    let kind = f.kind();
    let mut newton = Newton::new(f, p);

    // Accepted roots with their residuals.
    let mut found: Vec<(Vec<f64>, f64)> = Vec::new();
    let diag = search.iter().map(|(a, b)| (b - a) * (b - a)).sum::<f64>().sqrt();
    let dedup_tol = (DEDUP_REL * diag).max(1e-12);
    let mut seed = vec![0.0; n];

    for k in 0..seed_count(n, seeds_per_axis) {
        seed_point(k, search, seeds_per_axis, &mut seed);
        let Some((x, res)) = newton.solve(&seed, search) else {
            continue;
        };
        if !in_box(&x, search) {
            continue;
        }
        match found.iter_mut().find(|(y, _)| distance(&x, y) <= dedup_tol) {
            Some(existing) => {
                if res < existing.1 {
                    *existing = (x, res);
                }
            }
            None => found.push((x, res)),
        }
    }

    let mut jac = vec![0.0; n * n];
    let mut points: Vec<FixedPoint> = found
        .into_iter()
        .map(|(x, _)| {
            f.jacobian(0.0, &x, p, &mut jac);
            let (eigenvalues, classification) = classify_jacobian(kind, n, &jac);
            FixedPoint { x, eigenvalues, classification, stable: classification.is_stable() }
        })
        .collect();
    points.sort_by(|a, b| {
        a.x.iter()
            .zip(&b.x)
            .map(|(u, v)| u.total_cmp(v))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    points
}

/// Number of seeds: the full grid if small enough, else [`MAX_SEEDS`].
fn seed_count(n: usize, per_axis: usize) -> usize {
    grid_size(n, per_axis).unwrap_or(MAX_SEEDS)
}

/// `per_axis^n` if it is at most [`MAX_SEEDS`].
fn grid_size(n: usize, per_axis: usize) -> Option<usize> {
    let mut total: usize = 1;
    for _ in 0..n {
        total = total.checked_mul(per_axis).filter(|&t| t <= MAX_SEEDS)?;
    }
    Some(total)
}

/// Writes seed number `k` into `out`: grid cell centre `k` (in row-major
/// order, last coordinate fastest) or Halton point `k + 1`.
fn seed_point(k: usize, search: &[(f64, f64)], per_axis: usize, out: &mut [f64]) {
    let n = search.len();
    if grid_size(n, per_axis).is_some() {
        let mut rest = k;
        for d in (0..n).rev() {
            let i = rest % per_axis;
            rest /= per_axis;
            let (a, b) = search[d];
            out[d] = a + (i as f64 + 0.5) / per_axis as f64 * (b - a);
        }
    } else {
        for d in 0..n {
            let (a, b) = search[d];
            out[d] = a + radical_inverse(k + 1, nth_prime(d)) * (b - a);
        }
    }
}

/// Van der Corput radical inverse of `i` in `base`, in `[0, 1)`: the digits
/// of `i` mirrored about the decimal point. Halton point `i` is
/// `(radical_inverse(i, 2), radical_inverse(i, 3), radical_inverse(i, 5), …)`.
fn radical_inverse(mut i: usize, base: usize) -> f64 {
    let (mut result, mut f) = (0.0, 1.0 / base as f64);
    while i > 0 {
        result += (i % base) as f64 * f;
        i /= base;
        f /= base as f64;
    }
    result
}

/// The `d`-th prime (0-based: 2, 3, 5, 7, …).
fn nth_prime(d: usize) -> usize {
    (2..)
        .filter(|&c: &usize| (2..).take_while(|q| q * q <= c).all(|q| c % q != 0))
        .nth(d)
        .expect("there are infinitely many primes")
}

/// Is `x` inside the box enlarged by [`BOX_MARGIN`] on each side?
fn in_box(x: &[f64], search: &[(f64, f64)]) -> bool {
    x.iter().zip(search).all(|(&v, &(a, b))| {
        let (lo, hi) = (a.min(b), a.max(b));
        let m = BOX_MARGIN * (hi - lo);
        v >= lo - m && v <= hi + m
    })
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(u, v)| (u - v) * (u - v)).sum::<f64>().sqrt()
}

fn max_norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}

/// Damped Newton solver with reusable scratch buffers.
struct Newton<'a> {
    f: &'a dyn VectorField,
    p: &'a [f64],
    is_map: bool,
    fx: Vec<f64>,
    jac: Vec<f64>,
    /// Copy of the Jacobian (LU overwrites `jac`).
    jac_copy: Vec<f64>,
    step: Vec<f64>,
    trial: Vec<f64>,
    f_trial: Vec<f64>,
}

impl<'a> Newton<'a> {
    fn new(f: &'a dyn VectorField, p: &'a [f64]) -> Self {
        let n = f.dim();
        Self {
            f,
            p,
            is_map: f.kind() == SystemKind::Map,
            fx: vec![0.0; n],
            jac: vec![0.0; n * n],
            jac_copy: vec![0.0; n * n],
            step: vec![0.0; n],
            trial: vec![0.0; n],
            f_trial: vec![0.0; n],
        }
    }

    /// `out = F(x)`: `f(x)` for flows, `f(x) - x` for maps.
    fn residual(&self, x: &[f64], out: &mut [f64]) {
        self.f.eval(0.0, x, self.p, out);
        if self.is_map {
            for (o, xi) in out.iter_mut().zip(x) {
                *o -= xi;
            }
        }
    }

    /// Runs Newton from `seed`. Returns the root and its residual `‖F‖∞`,
    /// or `None` if it did not converge to an acceptable root.
    fn solve(&mut self, seed: &[f64], search: &[(f64, f64)]) -> Option<(Vec<f64>, f64)> {
        let n = seed.len();
        let mut x = seed.to_vec();
        let mut fx = std::mem::take(&mut self.fx);
        self.residual(&x, &mut fx);
        let mut norm2: f64 = fx.iter().map(|v| v * v).sum();

        let result = 'iterate: {
            for _ in 0..MAX_ITER {
                if !norm2.is_finite() {
                    break 'iterate None;
                }
                if norm2 == 0.0 {
                    break;
                }
                // Newton direction: DF δ = -F, with DF = Df (- I for maps).
                self.f.jacobian(0.0, &x, self.p, &mut self.jac);
                if self.is_map {
                    for i in 0..n {
                        self.jac[i * n + i] -= 1.0;
                    }
                }
                self.jac_copy.copy_from_slice(&self.jac);
                for (s, v) in self.step.iter_mut().zip(&fx) {
                    *s = -v;
                }
                if !lu_solve(&mut self.jac, &mut self.step)
                    && !regularised_step(&self.jac_copy, &fx, &mut self.step)
                {
                    break; // DF is zero: no direction to move in
                }

                // Backtracking line search on ‖F‖².
                let mut alpha = 1.0;
                let accepted = loop {
                    for ((t, xi), s) in self.trial.iter_mut().zip(&x).zip(&self.step) {
                        *t = xi + alpha * s;
                    }
                    let mut ft = std::mem::take(&mut self.f_trial);
                    self.residual(&self.trial, &mut ft);
                    self.f_trial = ft;
                    let trial2: f64 = self.f_trial.iter().map(|v| v * v).sum();
                    if trial2.is_finite() && trial2 <= (1.0 - 1e-4 * alpha) * norm2 {
                        break Some(trial2);
                    }
                    alpha *= 0.5;
                    if alpha < 1e-6 {
                        break None;
                    }
                };
                let Some(trial2) = accepted else {
                    break; // no progress possible from here
                };
                x.copy_from_slice(&self.trial);
                fx.copy_from_slice(&self.f_trial);
                norm2 = trial2;

                let step_size = alpha * max_norm(&self.step);
                if step_size <= STEP_TOL * (1.0 + max_norm(&x)) {
                    break;
                }
                if escaped(&x, search) {
                    break 'iterate None;
                }
            }
            let res = max_norm(&fx);
            self.is_root(&x, &fx).then_some((x, res))
        };
        self.fx = fx;
        result
    }

    /// Final acceptance test for the point `x` with residual `fx = F(x)`:
    /// the residual test ([`RESIDUAL_TOL`]) or the convergence test
    /// ([`NEWTON_STEP_TOL`]). Costs one Jacobian evaluation.
    fn is_root(&mut self, x: &[f64], fx: &[f64]) -> bool {
        let n = x.len();
        if !x.iter().chain(fx).all(|v| v.is_finite()) {
            return false;
        }
        self.f.jacobian(0.0, x, self.p, &mut self.jac);

        // Residual test against the size of the terms of each F_i.
        let small_residual = (0..n).all(|i| {
            let mut terms: f64 = (0..n).map(|j| (self.jac[i * n + j] * x[j]).abs()).sum();
            if self.is_map {
                terms += x[i].abs(); // F_i = f_i(x) - x_i
            }
            // An infinite slope says nothing about the size of F_i.
            let scale = if terms.is_finite() { terms.max(1.0) } else { 1.0 };
            fx[i].abs() <= RESIDUAL_TOL * scale
        });
        if small_residual {
            return true;
        }

        // Convergence test: size of the full Newton correction DF⁻¹ F.
        if self.is_map {
            for i in 0..n {
                self.jac[i * n + i] -= 1.0;
            }
        }
        self.step.copy_from_slice(fx);
        lu_solve(&mut self.jac, &mut self.step)
            && max_norm(&self.step) <= NEWTON_STEP_TOL * (1.0 + max_norm(x))
    }
}

/// Fallback step when `DF` is singular (e.g. on a line of fixed points):
/// the Levenberg–Marquardt step `(DFᵀDF + μI) δ = -DFᵀF` with a tiny `μ`,
/// which is close to the least-squares (pseudo-inverse) Newton step.
/// Returns `false` if even that system cannot be solved.
fn regularised_step(jac: &[f64], fx: &[f64], step: &mut [f64]) -> bool {
    let n = fx.len();
    let mut normal = vec![0.0; n * n];
    for r in 0..n {
        for c in 0..n {
            normal[r * n + c] = (0..n).map(|k| jac[k * n + r] * jac[k * n + c]).sum();
        }
        step[r] = -(0..n).map(|k| jac[k * n + r] * fx[k]).sum::<f64>();
    }
    let size = normal.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if size.is_nan() || size <= 0.0 {
        return false;
    }
    for i in 0..n {
        normal[i * n + i] += 1e-12 * size;
    }
    lu_solve(&mut normal, step)
}

/// Has Newton wandered far from the search box (e.g. towards infinity,
/// where `1/x` also looks like a root)?
fn escaped(x: &[f64], search: &[(f64, f64)]) -> bool {
    x.iter().zip(search).any(|(&v, &(a, b))| {
        let (lo, hi) = (a.min(b), a.max(b));
        let w = ESCAPE_WIDTHS * (hi - lo).max(1.0);
        v < lo - w || v > hi + w
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Classification::*;
    use dyn_core::FnField;
    use std::f64::consts::PI;

    /// Asserts that `points` contains exactly the `expected` locations (in
    /// any order), each with the expected class, and returns them in the
    /// order of `expected`.
    fn expect_points<'a>(
        points: &'a [FixedPoint],
        expected: &[(&[f64], Classification)],
        tol: f64,
    ) -> Vec<&'a FixedPoint> {
        assert_eq!(points.len(), expected.len(), "got {points:#?}");
        expected
            .iter()
            .map(|(x, class)| {
                let fp = points
                    .iter()
                    .find(|fp| distance(&fp.x, x) < tol)
                    .unwrap_or_else(|| panic!("no fixed point near {x:?} in {points:#?}"));
                assert_eq!(fp.classification, *class, "at {x:?}");
                assert_eq!(fp.stable, class.is_stable());
                fp
            })
            .collect()
    }

    fn linear(a: [f64; 4]) -> FnField {
        FnField::new(2, 0, SystemKind::Ode, move |_t, x, _p, o| {
            o[0] = a[0] * x[0] + a[1] * x[1];
            o[1] = a[2] * x[0] + a[3] * x[1];
        })
        .with_jacobian(move |_t, _x, _p, j| j.copy_from_slice(&a))
    }

    #[test]
    fn linear_systems_have_origin_with_right_class() {
        let cases = [
            ([1.0, 0.0, 0.0, -1.0], Saddle),
            ([-1.0, 0.0, 0.0, -2.0], StableNode),
            ([1.0, 0.5, 0.0, 2.0], UnstableNode),
            ([-0.2, 1.0, -1.0, -0.2], StableSpiral),
            ([0.2, 1.0, -1.0, 0.2], UnstableSpiral),
            ([0.0, 1.0, -1.0, 0.0], Centre),
            ([-1.0, 0.0, 0.0, -1.0], StableStar),
            ([2.0, 0.0, 0.0, 2.0], UnstableStar),
            ([-1.0, 1.0, 0.0, -1.0], StableDegenerateNode),
            ([1.0, 1.0, 0.0, 1.0], UnstableDegenerateNode),
        ];
        let search = [(-1.0, 1.0), (-1.0, 1.0)];
        for (a, class) in cases {
            let fps = fixed_points(&linear(a), &[], &search, 5);
            let fp = &expect_points(&fps, &[(&[0.0, 0.0], class)], 1e-12)[0];
            assert!(fp.x.iter().all(|v| v.abs() < 1e-14), "{:?}", fp.x);
        }
    }

    #[test]
    fn non_isolated_line_of_fixed_points() {
        // x' = y, y' = 0: the whole line y = 0 consists of fixed points.
        let a = [0.0, 1.0, 0.0, 0.0];
        let fps = fixed_points(&linear(a), &[], &[(-1.0, 1.0), (-1.0, 1.0)], 4);
        assert!(!fps.is_empty());
        for fp in &fps {
            assert!(fp.x[1].abs() < 1e-12);
            assert_eq!(fp.classification, NonIsolated);
            assert!(!fp.stable);
        }
    }

    fn rabbits_sheep() -> FnField {
        FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] * (3.0 - x[0] - 2.0 * x[1]);
            o[1] = x[1] * (2.0 - x[0] - x[1]);
        })
        .with_jacobian(|_t, x, _p, j| {
            j[0] = 3.0 - 2.0 * x[0] - 2.0 * x[1];
            j[1] = -2.0 * x[0];
            j[2] = -x[1];
            j[3] = 2.0 - x[0] - 2.0 * x[1];
        })
    }

    #[test]
    fn rabbits_versus_sheep() {
        let fps = fixed_points(&rabbits_sheep(), &[], &[(-0.5, 3.5), (-0.5, 3.5)], 12);
        let pts = expect_points(
            &fps,
            &[
                (&[0.0, 0.0], UnstableNode),
                (&[0.0, 2.0], StableNode),
                (&[3.0, 0.0], StableNode),
                (&[1.0, 1.0], Saddle),
            ],
            1e-10,
        );
        // Strogatz 6.4: eigenvalues at (0,0) are 3, 2; at (1,1) -1 ± √2.
        let mut ev: Vec<f64> = pts[0].eigenvalues.iter().map(|e| e.0).collect();
        ev.sort_by(f64::total_cmp);
        assert_eq!(ev, vec![2.0, 3.0]);
        let mut ev: Vec<f64> = pts[3].eigenvalues.iter().map(|e| e.0).collect();
        ev.sort_by(f64::total_cmp);
        assert!((ev[0] - (-1.0 - 2f64.sqrt())).abs() < 1e-12);
        assert!((ev[1] - (-1.0 + 2f64.sqrt())).abs() < 1e-12);
    }

    #[test]
    fn rabbits_versus_sheep_is_fast() {
        let f = rabbits_sheep();
        let search = [(-0.5, 3.5), (-0.5, 3.5)];
        fixed_points(&f, &[], &search, 12); // warm up
        let start = std::time::Instant::now();
        let runs = 20;
        for _ in 0..runs {
            assert_eq!(fixed_points(&f, &[], &search, 12).len(), 4);
        }
        let per_run = start.elapsed() / runs;
        eprintln!("fixed_points(rabbits vs sheep, 12 seeds/axis): {per_run:?}");
        // Budget is 30 ms; debug builds are slower, so only assert loosely.
        assert!(per_run.as_millis() < 30, "took {per_run:?} per run");
    }

    #[test]
    fn pendulum_centres_and_saddles() {
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[1];
            o[1] = -x[0].sin();
        })
        .with_jacobian(|_t, x, _p, j| {
            j.copy_from_slice(&[0.0, 1.0, -x[0].cos(), 0.0]);
        });
        let fps = fixed_points(&f, &[], &[(-4.0, 4.0), (-3.0, 3.0)], 12);
        expect_points(
            &fps,
            &[(&[-PI, 0.0], Saddle), (&[0.0, 0.0], Centre), (&[PI, 0.0], Saddle)],
            1e-12,
        );
        // Sorted by x.
        assert!(fps[0].x[0] < fps[1].x[0] && fps[1].x[0] < fps[2].x[0]);
    }

    #[test]
    fn lorenz_fixed_points() {
        let (sigma, r, b) = (10.0, 28.0, 8.0 / 3.0);
        let f = FnField::new(3, 3, SystemKind::Ode, |_t, x, p, o| {
            o[0] = p[0] * (x[1] - x[0]);
            o[1] = x[0] * (p[1] - x[2]) - x[1];
            o[2] = x[0] * x[1] - p[2] * x[2];
        })
        .with_jacobian(|_t, x, p, j| {
            j.copy_from_slice(&[-p[0], p[0], 0.0, p[1] - x[2], -1.0, -x[0], x[1], x[0], -p[2]]);
        });
        let search = [(-20.0, 20.0), (-25.0, 25.0), (0.0, 50.0)];
        let fps = fixed_points(&f, &[sigma, r, b], &search, 10);
        let c = (b * (r - 1.0)).sqrt();
        let pts = expect_points(
            &fps,
            &[
                (&[0.0, 0.0, 0.0], SaddleND),
                (&[c, c, r - 1.0], SaddleND),
                (&[-c, -c, r - 1.0], SaddleND),
            ],
            1e-9,
        );
        // C± have one real negative eigenvalue and a complex pair with
        // positive real part at r = 28 (r > r_H ≈ 24.74).
        let ev = &pts[1].eigenvalues;
        assert_eq!(ev.iter().filter(|e| e.1 != 0.0).count(), 2);
        assert!(ev.iter().filter(|e| e.1 != 0.0).all(|e| e.0 > 0.0));
        // Origin: λ = -b and (-(σ+1) ± sqrt((σ+1)² - 4σ(1-r)))/2.
        let mut ev0: Vec<f64> = pts[0].eigenvalues.iter().map(|e| e.0).collect();
        ev0.sort_by(f64::total_cmp);
        let d = ((sigma + 1.0) * (sigma + 1.0) - 4.0 * sigma * (1.0 - r)).sqrt();
        let expect = [(-(sigma + 1.0) - d) / 2.0, -b, (-(sigma + 1.0) + d) / 2.0];
        for (u, v) in ev0.iter().zip(expect) {
            assert!((u - v).abs() < 1e-9, "{ev0:?}");
        }
    }

    #[test]
    fn logistic_map_fixed_points_and_multipliers() {
        let f = FnField::new(1, 1, SystemKind::Map, |_t, x, p, o| o[0] = p[0] * x[0] * (1.0 - x[0]))
            .with_jacobian(|_t, x, p, j| j[0] = p[0] * (1.0 - 2.0 * x[0]));
        for (r, c0, c1) in [(2.5, Source, Sink), (3.2, Source, Source), (0.5, Sink, Source)] {
            let fps = fixed_points(&f, &[r], &[(-1.5, 1.5)], 20);
            let star = 1.0 - 1.0 / r;
            let pts = expect_points(&fps, &[(&[0.0], c0), (&[star], c1)], 1e-12);
            assert!((pts[0].eigenvalues[0].0 - r).abs() < 1e-12);
            assert!((pts[1].eigenvalues[0].0 - (2.0 - r)).abs() < 1e-12);
        }
        // r = 3: x* = 2/3 has multiplier -1 (period-doubling): non-hyperbolic.
        let fps = fixed_points(&f, &[3.0], &[(-0.5, 1.5)], 20);
        expect_points(&fps, &[(&[0.0], Source), (&[2.0 / 3.0], NonHyperbolic)], 1e-12);
    }

    #[test]
    fn one_dimensional_flow() {
        // x' = x - x³: fixed points -1, 1 (sinks), 0 (source).
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = x[0] - x[0].powi(3))
            .with_jacobian(|_t, x, _p, j| j[0] = 1.0 - 3.0 * x[0] * x[0]);
        let fps = fixed_points(&f, &[], &[(-2.0, 2.0)], 10);
        expect_points(&fps, &[(&[-1.0], Sink), (&[0.0], Source), (&[1.0], Sink)], 1e-12);
        // x' = x²: a double root, non-hyperbolic.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = x[0] * x[0])
            .with_jacobian(|_t, x, _p, j| j[0] = 2.0 * x[0]);
        let fps = fixed_points(&f, &[], &[(-2.0, 2.0)], 10);
        expect_points(&fps, &[(&[0.0], NonHyperbolic)], 1e-6);
    }

    #[test]
    fn halton_seeds_for_large_grids() {
        // 8^5 = 32768 > 4096 seeds: Halton is used. Linear sink in 5D.
        let f = FnField::new(5, 0, SystemKind::Ode, |_t, x, _p, o| {
            for i in 0..5 {
                o[i] = -(i as f64 + 1.0) * (x[i] - 0.25);
            }
        });
        let fps = fixed_points(&f, &[], &[(-1.0, 1.0); 5], 8);
        expect_points(&fps, &[(&[0.25; 5], Sink)], 1e-9);
        assert_eq!(grid_size(5, 8), None);
        assert_eq!(seed_count(5, 8), MAX_SEEDS);
        assert_eq!(seed_count(2, 12), 144);
        assert_eq!(seed_count(12, 2), MAX_SEEDS); // 2^12 = 4096 is still a grid
        assert_eq!(grid_size(12, 2), Some(4096));
        assert_eq!(grid_size(13, 2), None);
    }

    #[test]
    fn halton_sequence_values() {
        assert_eq!(radical_inverse(1, 2), 0.5);
        assert_eq!(radical_inverse(2, 2), 0.25);
        assert_eq!(radical_inverse(3, 2), 0.75);
        assert!((radical_inverse(1, 3) - 1.0 / 3.0).abs() < 1e-15);
        assert!((radical_inverse(5, 3) - (2.0 / 3.0 + 1.0 / 9.0)).abs() < 1e-15);
        let primes: Vec<_> = (0..6).map(nth_prime).collect();
        assert_eq!(primes, vec![2, 3, 5, 7, 11, 13]);
    }

    #[test]
    fn poles_and_far_roots_are_rejected() {
        // x' = 1/x has no root, but Newton drifts to infinity where |F| → 0.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = 1.0 / x[0]);
        assert!(fixed_points(&f, &[], &[(-1.0, 1.0)], 20).is_empty());
        // Root at 1.08 is inside the 5% margin of [0, 1]; 1.2 is outside.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = (x[0] - 1.04) * (x[0] - 1.2)
        });
        let fps = fixed_points(&f, &[], &[(0.0, 1.0)], 10);
        assert_eq!(fps.len(), 1);
        assert!((fps[0].x[0] - 1.04).abs() < 1e-12);
    }

    #[test]
    fn default_finite_difference_jacobian_works() {
        // No exact Jacobian supplied: uses dyn_core's central differences.
        let f = FnField::new(2, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] * (3.0 - x[0] - 2.0 * x[1]);
            o[1] = x[1] * (2.0 - x[0] - x[1]);
        });
        let fps = fixed_points(&f, &[], &[(-0.5, 3.5), (-0.5, 3.5)], 12);
        expect_points(
            &fps,
            &[
                (&[0.0, 0.0], UnstableNode),
                (&[0.0, 2.0], StableNode),
                (&[3.0, 0.0], StableNode),
                (&[1.0, 1.0], Saddle),
            ],
            1e-8,
        );
    }

    #[test]
    fn henon_map() {
        // x' = 1 - a x² + y, y' = b x with a = 1.4, b = 0.3: two saddles.
        let f = FnField::new(2, 2, SystemKind::Map, |_t, x, p, o| {
            o[0] = 1.0 - p[0] * x[0] * x[0] + x[1];
            o[1] = p[1] * x[0];
        })
        .with_jacobian(|_t, x, p, j| j.copy_from_slice(&[-2.0 * p[0] * x[0], 1.0, p[1], 0.0]));
        let (a, b) = (1.4, 0.3);
        let fps = fixed_points(&f, &[a, b], &[(-2.0, 2.0), (-1.0, 1.0)], 12);
        let disc = ((b - 1.0) * (b - 1.0) + 4.0 * a).sqrt();
        let xp = ((b - 1.0) + disc) / (2.0 * a);
        let xm = ((b - 1.0) - disc) / (2.0 * a);
        expect_points(&fps, &[(&[xm, b * xm], SaddleND), (&[xp, b * xp], SaddleND)], 1e-10);
    }

    #[test]
    fn rejected_inputs() {
        let f = rabbits_sheep();
        assert!(fixed_points(&f, &[], &[(0.0, 1.0)], 5).is_empty()); // wrong box length
        assert!(fixed_points(&f, &[], &[(0.0, 1.0), (0.0, 1.0)], 0).is_empty());
        let g = FnField::new(1, 0, SystemKind::Ode, |t, x, _p, o| o[0] = x[0] - t).non_autonomous();
        assert!(fixed_points(&g, &[], &[(-1.0, 1.0)], 5).is_empty());
    }

    #[test]
    fn serialises_like_the_ui_type() {
        let fp = FixedPoint {
            x: vec![1.0, 2.0],
            eigenvalues: vec![(-1.0, 0.5), (-1.0, -0.5)],
            classification: StableSpiral,
            stable: true,
        };
        assert_eq!(
            serde_json::to_string(&fp).unwrap(),
            r#"{"x":[1.0,2.0],"eigenvalues":[[-1.0,0.5],[-1.0,-0.5]],"classification":"stable spiral","stable":true}"#
        );
    }

    #[test]
    fn saddle_node_ghost_is_never_a_fixed_point() {
        // x' = r + x², y' = -y: no fixed point for r > 0, two for r < 0
        // (Strogatz §3.1), whatever the zoom.
        let f = FnField::new(2, 1, SystemKind::Ode, |_t, x, p, o| {
            o[0] = p[0] + x[0] * x[0];
            o[1] = -x[1];
        })
        .with_jacobian(|_t, x, _p, j| j.copy_from_slice(&[2.0 * x[0], 0.0, 0.0, -1.0]));
        for half_width in [1e-2, 1.0, 2.0, 100.0, 1e4, 1e6] {
            let search = [(-half_width, half_width); 2];
            for seeds in [7, 14, 30] {
                for r in [1e-8, 1e-6, 1e-3] {
                    let fps = fixed_points(&f, &[r], &search, seeds);
                    assert!(fps.is_empty(), "r = {r}, box ±{half_width}, {seeds}: {fps:?}");
                }
            }
        }
        let fps = fixed_points(&f, &[-1e-4], &[(-1.0, 1.0); 2], 14);
        expect_points(&fps, &[(&[-1e-2, 0.0], StableNode), (&[1e-2, 0.0], Saddle)], 1e-12);
        let fps = fixed_points(&f, &[-1e-4], &[(-100.0, 100.0); 2], 14);
        assert_eq!(fps.len(), 2, "{fps:?}");
    }

    #[test]
    fn roots_of_functions_with_large_values_are_found() {
        // Simple roots, terms of size 1e12: rounding leaves |F| ≈ 1e-4.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = 1e12 * (x[0] * x[0] - 2.0))
            .with_jacobian(|_t, x, _p, j| j[0] = 2e12 * x[0]);
        let fps = fixed_points(&f, &[], &[(-3.0, 3.0)], 10);
        let r2 = 2f64.sqrt();
        expect_points(&fps, &[(&[-r2], Sink), (&[r2], Source)], 1e-12);
        // A double root: Newton converges only linearly and stalls with
        // |F| far above 1e-9; the convergence test still accepts it.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = 1e12 * (1.0 + x[0].sin()))
            .with_jacobian(|_t, x, _p, j| j[0] = 1e12 * x[0].cos());
        let fps = fixed_points(&f, &[], &[(-3.0, 0.0)], 10);
        assert!(!fps.is_empty());
        for fp in &fps {
            assert!((fp.x[0] + PI / 2.0).abs() < 1e-6, "{fp:?}");
        }
    }

    #[test]
    fn infinite_slope_at_a_fixed_point() {
        // x' = cbrt(x): the fixed point 0 is found (the seed lands on it) but
        // has no linearisation.
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = x[0].cbrt())
            .with_jacobian(|_t, x, _p, j| j[0] = 1.0 / (3.0 * x[0].cbrt() * x[0].cbrt()));
        let fps = fixed_points(&f, &[], &[(-1.0, 1.0)], 3);
        assert_eq!(fps.len(), 1, "{fps:?}");
        assert_eq!(fps[0].x, vec![0.0]);
        assert!(fps[0].eigenvalues.is_empty());
        assert_eq!(fps[0].classification, NonHyperbolic);
        assert!(!fps[0].stable);
        assert_eq!(
            serde_json::to_string(&fps[0]).unwrap(),
            r#"{"x":[0.0],"eigenvalues":[],"classification":"non-hyperbolic","stable":false}"#
        );
    }
}
