use dyn_core::{SystemKind, VectorField};
use serde::{Deserialize, Serialize};

use crate::classify::{classify_jacobian, Classification};
use crate::eigvec::{eigenvectors, Eigenvectors};
use crate::linalg::lu_solve;

/// Maximum number of Newton starting points.
pub const MAX_SEEDS: usize = 4096;
const MAX_ITER: usize = 100;
const STEP_TOL: f64 = 1e-14;
const RESIDUAL_TOL: f64 = 1e-9;
const NEWTON_STEP_TOL: f64 = 1e-6;
/// Sufficient-decrease constant of the backtracking line search.
const ARMIJO_C1: f64 = 1e-4;
/// The line search gives up below this fraction of the Newton step.
const MIN_STEP_FRACTION: f64 = 1e-6;
/// Relative damping of the Levenberg–Marquardt fallback step.
const LM_DAMPING: f64 = 1e-12;
const DEDUP_REL: f64 = 1e-6;
const BOX_MARGIN: f64 = 0.05;
const ESCAPE_WIDTHS: f64 = 10.0;

/// A fixed point with its linearisation.
///
/// Serialises with camelCase field names, like the UI's `FixedPointOut`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    /// One entry per eigenvalue, in the same order: the unit eigenvector of
    /// a real eigenvalue (largest-magnitude component positive), or the
    /// real part of the eigenvector of a complex one. `None` for the
    /// missing directions of a defective eigenvalue (a degenerate node has
    /// one vector and one `None`). Stars get an orthonormal basis. See
    /// [`crate::eigenvectors`].
    #[serde(default)]
    pub eigenvectors: Vec<Option<Vec<f64>>>,
    /// Imaginary parts of the eigenvectors of complex eigenvalues (`None`
    /// for real ones). The phase is chosen so that the real and imaginary
    /// parts are the major and minor semi-axes of the linearised orbits'
    /// ellipses (orthogonal, `|re|² + |im|² = 1`).
    #[serde(default)]
    pub eigenvectors_im: Vec<Option<Vec<f64>>>,
    /// Type of the fixed point.
    pub classification: Classification,
    /// `true` if the point is asymptotically stable (sink-like).
    pub stable: bool,
}

/// Finds the fixed points of `f` in the box `search` (one `(min, max)` per
/// state variable) by multi-start Newton.
///
/// Flows: `f(x) = 0`; maps: `f(x) = x`. Both are evaluated at `t = 0`.
/// Returns an empty `Vec` for non-autonomous systems (their fixed points
/// would move with time), when `search.len() != f.dim()`, or when
/// `seeds_per_axis == 0`. Results are sorted by coordinates.
///
/// Newton starts from a regular grid of `seeds_per_axis^dim` cell centres
/// of the box when that is at most [`MAX_SEEDS`], otherwise from the first
/// [`MAX_SEEDS`] points of a Halton sequence in the box. Points outside the
/// box enlarged by 5% per side are dropped, and points closer than `1e-6`
/// of the box diagonal are merged.
///
/// A whole curve of fixed points (e.g. `x' = 0`) yields one point per seed
/// that lands on it.
pub fn fixed_points(
    f: &dyn VectorField,
    p: &[f64],
    search: &[(f64, f64)],
    seeds_per_axis: usize,
) -> Vec<FixedPoint> {
    fixed_points_with_budget(f, p, search, seeds_per_axis, MAX_SEEDS)
}

/// [`fixed_points`] with at most `max_seeds` Newton starting points in
/// total, whatever the dimension: the grid of `seeds_per_axis^dim` cell
/// centres when it has at most `max_seeds` points, otherwise the first
/// `max_seeds` points of a Halton sequence in the box. `max_seeds` is
/// clamped to [`MAX_SEEDS`]; 0 finds nothing.
pub fn fixed_points_with_budget(
    f: &dyn VectorField,
    p: &[f64],
    search: &[(f64, f64)],
    seeds_per_axis: usize,
    max_seeds: usize,
) -> Vec<FixedPoint> {
    let max_seeds = max_seeds.min(MAX_SEEDS);
    if max_seeds == 0 {
        return Vec::new();
    }
    let n = f.dim();
    if !f.is_autonomous() || n == 0 || search.len() != n || seeds_per_axis == 0 {
        return Vec::new();
    }
    let kind = f.kind();
    let mut newton = Newton::new(f, p);

    let mut found: Vec<Root> = Vec::new();
    let diag = search
        .iter()
        .map(|(a, b)| (b - a) * (b - a))
        .sum::<f64>()
        .sqrt();
    let dedup_tol = (DEDUP_REL * diag).max(1e-12);
    let mut seed = vec![0.0; n];

    for k in 0..seed_count(n, seeds_per_axis, max_seeds) {
        seed_point(k, search, seeds_per_axis, max_seeds, &mut seed);
        let Some(root) = newton.solve(&seed, search) else {
            continue;
        };
        if !in_box(&root.x, search) {
            continue;
        }
        match found
            .iter_mut()
            .find(|r| distance(&root.x, &r.x) <= dedup_tol)
        {
            Some(existing) => {
                if root.residual < existing.residual {
                    *existing = root;
                }
            }
            None => found.push(root),
        }
    }

    let mut jac = vec![0.0; n * n];
    let mut points: Vec<FixedPoint> = found
        .into_iter()
        .map(|Root { x, .. }| {
            f.jacobian(0.0, &x, p, &mut jac);
            let (eigenvalues, classification) = classify_jacobian(kind, n, &jac);
            let vecs = if eigenvalues.is_empty() {
                Eigenvectors::default()
            } else {
                eigenvectors(kind, n, &jac, &eigenvalues)
            };
            FixedPoint {
                x,
                eigenvalues,
                eigenvectors: vecs.re,
                eigenvectors_im: vecs.im,
                classification,
                stable: classification.is_stable(),
            }
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

/// How many Newton starting points [`fixed_points_with_budget`] uses in
/// dimension `n`: `per_axis^n` if that is at most `max_seeds` (itself
/// clamped to [`MAX_SEEDS`]), otherwise `max_seeds`.
pub fn seed_count(n: usize, per_axis: usize, max_seeds: usize) -> usize {
    let max_seeds = max_seeds.min(MAX_SEEDS);
    grid_size(n, per_axis, max_seeds).unwrap_or(max_seeds)
}

fn grid_size(n: usize, per_axis: usize, max_seeds: usize) -> Option<usize> {
    let mut total: usize = 1;
    for _ in 0..n {
        total = total.checked_mul(per_axis).filter(|&t| t <= max_seeds)?;
    }
    Some(total)
}

fn seed_point(k: usize, search: &[(f64, f64)], per_axis: usize, max_seeds: usize, out: &mut [f64]) {
    let n = search.len();
    if grid_size(n, per_axis, max_seeds).is_some() {
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

fn radical_inverse(mut i: usize, base: usize) -> f64 {
    let (mut result, mut f) = (0.0, 1.0 / base as f64);
    while i > 0 {
        result += (i % base) as f64 * f;
        i /= base;
        f /= base as f64;
    }
    result
}

fn nth_prime(d: usize) -> usize {
    (2..)
        .filter(|&c: &usize| (2..).take_while(|q| q * q <= c).all(|q| c % q != 0))
        .nth(d)
        .expect("there are infinitely many primes")
}

fn in_box(x: &[f64], search: &[(f64, f64)]) -> bool {
    x.iter().zip(search).all(|(&v, &(a, b))| {
        let (lo, hi) = (a.min(b), a.max(b));
        let m = BOX_MARGIN * (hi - lo);
        v >= lo - m && v <= hi + m
    })
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(u, v)| (u - v) * (u - v))
        .sum::<f64>()
        .sqrt()
}

fn max_norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}

/// A converged Newton solution and its residual (max norm).
struct Root {
    x: Vec<f64>,
    residual: f64,
}

/// The function whose roots are the fixed points: `f(x)` for flows,
/// `f(x) - x` for maps.
struct Residual<'a> {
    f: &'a dyn VectorField,
    p: &'a [f64],
    is_map: bool,
}

impl Residual<'_> {
    fn eval(&self, x: &[f64], out: &mut [f64]) {
        self.f.eval(0.0, x, self.p, out);
        if self.is_map {
            for (o, xi) in out.iter_mut().zip(x) {
                *o -= xi;
            }
        }
    }

    /// `Df(x)`, the Jacobian of the field itself.
    fn df(&self, x: &[f64], jac: &mut [f64]) {
        self.f.jacobian(0.0, x, self.p, jac);
    }

    /// Turn `Df` into the residual's Jacobian: `Df - I` for maps.
    fn shift_to_residual_jacobian(&self, jac: &mut [f64]) {
        if self.is_map {
            let n = self.f.dim();
            for i in 0..n {
                jac[i * n + i] -= 1.0;
            }
        }
    }

    fn residual_jacobian(&self, x: &[f64], jac: &mut [f64]) {
        self.df(x, jac);
        self.shift_to_residual_jacobian(jac);
    }

    /// Size of the terms that make up row `i` of the residual at `x`:
    /// `Σ |Df_ij x_j|`, plus `|x_i|` for maps; 1 where that is not finite
    /// (an infinite slope) or below 1. `jac` holds `Df(x)`.
    fn term_scale(&self, i: usize, x: &[f64], jac: &[f64]) -> f64 {
        let n = x.len();
        let mut terms: f64 = (0..n).map(|j| (jac[i * n + j] * x[j]).abs()).sum();
        if self.is_map {
            terms += x[i].abs();
        }
        if terms.is_finite() {
            terms.max(1.0)
        } else {
            1.0
        }
    }
}

struct Newton<'a> {
    g: Residual<'a>,
    fx: Vec<f64>,
    jac: Vec<f64>,
    trial: Vec<f64>,
    f_trial: Vec<f64>,
}

impl<'a> Newton<'a> {
    fn new(f: &'a dyn VectorField, p: &'a [f64]) -> Self {
        let n = f.dim();
        Self {
            g: Residual {
                f,
                p,
                is_map: f.kind() == SystemKind::Map,
            },
            fx: vec![0.0; n],
            jac: vec![0.0; n * n],
            trial: vec![0.0; n],
            f_trial: vec![0.0; n],
        }
    }

    fn solve(&mut self, seed: &[f64], search: &[(f64, f64)]) -> Option<Root> {
        let mut x = seed.to_vec();
        let mut fx = std::mem::take(&mut self.fx);
        self.g.eval(&x, &mut fx);
        let mut norm2: f64 = fx.iter().map(|v| v * v).sum();

        let result = 'iterate: {
            for _ in 0..MAX_ITER {
                if !norm2.is_finite() {
                    break 'iterate None;
                }
                if norm2 == 0.0 {
                    break;
                }
                self.g.residual_jacobian(&x, &mut self.jac);
                let neg_fx: Vec<f64> = fx.iter().map(|v| -v).collect();
                let Some(step) = lu_solve(&self.jac, &neg_fx)
                    .or_else(|| levenberg_marquardt_step(&self.jac, &fx))
                else {
                    break;
                };
                let Some((alpha, trial2)) = self.line_search(&x, &step, norm2) else {
                    break;
                };
                x.copy_from_slice(&self.trial);
                fx.copy_from_slice(&self.f_trial);
                norm2 = trial2;

                let step_size = alpha * max_norm(&step);
                if step_size <= STEP_TOL * (1.0 + max_norm(&x)) {
                    break;
                }
                if escaped(&x, search) {
                    break 'iterate None;
                }
            }
            let residual = max_norm(&fx);
            self.is_root(&x, &fx).then_some(Root { x, residual })
        };
        self.fx = fx;
        result
    }

    /// Backtrack along `step` from `x` (`alpha = 1, 1/2, …`) to the first
    /// point whose squared residual satisfies the Armijo condition against
    /// `norm2`. Leaves that point in `trial` and its residual in `f_trial`,
    /// and returns `(alpha, |residual|²)`.
    fn line_search(&mut self, x: &[f64], step: &[f64], norm2: f64) -> Option<(f64, f64)> {
        let mut alpha = 1.0;
        loop {
            for ((t, xi), s) in self.trial.iter_mut().zip(x).zip(step) {
                *t = xi + alpha * s;
            }
            self.g.eval(&self.trial, &mut self.f_trial);
            let trial2: f64 = self.f_trial.iter().map(|v| v * v).sum();
            if trial2.is_finite() && trial2 <= (1.0 - ARMIJO_C1 * alpha) * norm2 {
                return Some((alpha, trial2));
            }
            alpha *= 0.5;
            if alpha < MIN_STEP_FRACTION {
                return None;
            }
        }
    }

    fn is_root(&mut self, x: &[f64], fx: &[f64]) -> bool {
        if !x.iter().chain(fx).all(|v| v.is_finite()) {
            return false;
        }
        self.g.df(x, &mut self.jac);
        if self.residual_small_against_term_scale(x, fx) {
            return true;
        }
        self.g.shift_to_residual_jacobian(&mut self.jac);
        self.newton_correction_negligible(x, fx)
    }

    /// Every residual component is tiny next to the terms it is made of.
    /// `jac` holds `Df(x)`.
    fn residual_small_against_term_scale(&self, x: &[f64], fx: &[f64]) -> bool {
        (0..x.len()).all(|i| fx[i].abs() <= RESIDUAL_TOL * self.g.term_scale(i, x, &self.jac))
    }

    /// One more Newton step would barely move `x`. `jac` holds the
    /// residual's Jacobian.
    fn newton_correction_negligible(&self, x: &[f64], fx: &[f64]) -> bool {
        lu_solve(&self.jac, fx)
            .is_some_and(|step| max_norm(&step) <= NEWTON_STEP_TOL * (1.0 + max_norm(x)))
    }
}

/// The damped least-squares step `-(JᵀJ + μI)⁻¹ Jᵀ f` for a singular `J`.
fn levenberg_marquardt_step(jac: &[f64], fx: &[f64]) -> Option<Vec<f64>> {
    let n = fx.len();
    let mut normal = vec![0.0; n * n];
    let mut rhs = vec![0.0; n];
    for r in 0..n {
        for c in 0..n {
            normal[r * n + c] = (0..n).map(|k| jac[k * n + r] * jac[k * n + c]).sum();
        }
        rhs[r] = -(0..n).map(|k| jac[k * n + r] * fx[k]).sum::<f64>();
    }
    let size = normal.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if size.is_nan() || size <= 0.0 {
        return None;
    }
    for i in 0..n {
        normal[i * n + i] += LM_DAMPING * size;
    }
    lu_solve(&normal, &rhs)
}

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

    fn check_eigenpairs(f: &dyn VectorField, p: &[f64], fp: &FixedPoint, tol: f64) -> Vec<f64> {
        let n = fp.x.len();
        let mut jac = vec![0.0; n * n];
        f.jacobian(0.0, &fp.x, p, &mut jac);
        let vecs = Eigenvectors {
            re: fp.eigenvectors.clone(),
            im: fp.eigenvectors_im.clone(),
        };
        crate::eigvec::tests::assert_residuals(n, &jac, &fp.eigenvalues, &vecs, tol);
        jac
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
        let mut ev: Vec<f64> = pts[0].eigenvalues.iter().map(|e| e.0).collect();
        ev.sort_by(f64::total_cmp);
        assert_eq!(ev, vec![2.0, 3.0]);
        let mut ev: Vec<f64> = pts[3].eigenvalues.iter().map(|e| e.0).collect();
        ev.sort_by(f64::total_cmp);
        assert!((ev[0] - (-1.0 - 2f64.sqrt())).abs() < 1e-12);
        assert!((ev[1] - (-1.0 + 2f64.sqrt())).abs() < 1e-12);
        let f = rabbits_sheep();
        for fp in &fps {
            check_eigenpairs(&f, &[], fp, 1e-9);
            assert!(fp.eigenvectors_im.iter().all(Option::is_none));
        }
        let (r2, r3) = (2f64.sqrt(), 3f64.sqrt());
        for (k, &(l, _)) in pts[3].eigenvalues.iter().enumerate() {
            let v = pts[3].eigenvectors[k].as_ref().unwrap();
            let expect = if l > 0.0 {
                [r2 / r3, -1.0 / r3]
            } else {
                [r2 / r3, 1.0 / r3]
            };
            assert!(distance(v, &expect) < 1e-12, "λ = {l}: {v:?}");
        }
        for (k, &(l, _)) in pts[0].eigenvalues.iter().enumerate() {
            let v = pts[0].eigenvectors[k].as_ref().unwrap();
            let expect = if l == 3.0 { [1.0, 0.0] } else { [0.0, 1.0] };
            assert_eq!(v, &expect, "λ = {l}");
        }
    }

    #[test]
    fn rabbits_versus_sheep_is_fast() {
        let f = rabbits_sheep();
        let search = [(-0.5, 3.5), (-0.5, 3.5)];
        fixed_points(&f, &[], &search, 12);
        let start = std::time::Instant::now();
        let runs = 20;
        for _ in 0..runs {
            assert_eq!(fixed_points(&f, &[], &search, 12).len(), 4);
        }
        let per_run = start.elapsed() / runs;
        eprintln!("fixed_points(rabbits vs sheep, 12 seeds/axis): {per_run:?}");
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
            &[
                (&[-PI, 0.0], Saddle),
                (&[0.0, 0.0], Centre),
                (&[PI, 0.0], Saddle),
            ],
            1e-12,
        );
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
            j.copy_from_slice(&[
                -p[0],
                p[0],
                0.0,
                p[1] - x[2],
                -1.0,
                -x[0],
                x[1],
                x[0],
                -p[2],
            ]);
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
        let ev = &pts[1].eigenvalues;
        assert_eq!(ev.iter().filter(|e| e.1 != 0.0).count(), 2);
        assert!(ev.iter().filter(|e| e.1 != 0.0).all(|e| e.0 > 0.0));
        let mut ev0: Vec<f64> = pts[0].eigenvalues.iter().map(|e| e.0).collect();
        ev0.sort_by(f64::total_cmp);
        let d = ((sigma + 1.0) * (sigma + 1.0) - 4.0 * sigma * (1.0 - r)).sqrt();
        let expect = [(-(sigma + 1.0) - d) / 2.0, -b, (-(sigma + 1.0) + d) / 2.0];
        for (u, v) in ev0.iter().zip(expect) {
            assert!((u - v).abs() < 1e-9, "{ev0:?}");
        }
        for fp in &fps {
            check_eigenpairs(&f, &[sigma, r, b], fp, 1e-9);
        }
        let complex = pts[1].eigenvectors_im.iter().flatten().count();
        assert_eq!(complex, 2);
        let kz = pts[0]
            .eigenvalues
            .iter()
            .position(|e| (e.0 + b).abs() < 1e-9)
            .unwrap();
        assert_eq!(
            pts[0].eigenvectors[kz].as_deref(),
            Some(&[0.0, 0.0, 1.0][..])
        );
    }

    #[test]
    fn logistic_map_fixed_points_and_multipliers() {
        let f = FnField::new(1, 1, SystemKind::Map, |_t, x, p, o| {
            o[0] = p[0] * x[0] * (1.0 - x[0])
        })
        .with_jacobian(|_t, x, p, j| j[0] = p[0] * (1.0 - 2.0 * x[0]));
        for (r, c0, c1) in [
            (2.5, Source, Sink),
            (3.2, Source, Source),
            (0.5, Sink, Source),
        ] {
            let fps = fixed_points(&f, &[r], &[(-1.5, 1.5)], 20);
            let star = 1.0 - 1.0 / r;
            let pts = expect_points(&fps, &[(&[0.0], c0), (&[star], c1)], 1e-12);
            assert!((pts[0].eigenvalues[0].0 - r).abs() < 1e-12);
            assert!((pts[1].eigenvalues[0].0 - (2.0 - r)).abs() < 1e-12);
        }
        let fps = fixed_points(&f, &[3.0], &[(-0.5, 1.5)], 20);
        expect_points(
            &fps,
            &[(&[0.0], Source), (&[2.0 / 3.0], NonHyperbolic)],
            1e-12,
        );
    }

    #[test]
    fn one_dimensional_flow() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = x[0] - x[0].powi(3)
        })
        .with_jacobian(|_t, x, _p, j| j[0] = 1.0 - 3.0 * x[0] * x[0]);
        let fps = fixed_points(&f, &[], &[(-2.0, 2.0)], 10);
        expect_points(
            &fps,
            &[(&[-1.0], Sink), (&[0.0], Source), (&[1.0], Sink)],
            1e-12,
        );
        for fp in &fps {
            check_eigenpairs(&f, &[], fp, 1e-12);
            assert_eq!(fp.eigenvectors, vec![Some(vec![1.0])]);
            assert_eq!(fp.eigenvectors_im, vec![None]);
        }
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = x[0] * x[0])
            .with_jacobian(|_t, x, _p, j| j[0] = 2.0 * x[0]);
        let fps = fixed_points(&f, &[], &[(-2.0, 2.0)], 10);
        expect_points(&fps, &[(&[0.0], NonHyperbolic)], 1e-6);
    }

    #[test]
    fn halton_seeds_for_large_grids() {
        let f = FnField::new(5, 0, SystemKind::Ode, |_t, x, _p, o| {
            for i in 0..5 {
                o[i] = -(i as f64 + 1.0) * (x[i] - 0.25);
            }
        });
        let fps = fixed_points(&f, &[], &[(-1.0, 1.0); 5], 8);
        expect_points(&fps, &[(&[0.25; 5], Sink)], 1e-9);
        assert_eq!(grid_size(5, 8, MAX_SEEDS), None);
        assert_eq!(seed_count(5, 8, MAX_SEEDS), MAX_SEEDS);
        assert_eq!(seed_count(2, 12, MAX_SEEDS), 144);
        assert_eq!(seed_count(12, 2, MAX_SEEDS), MAX_SEEDS);
        assert_eq!(grid_size(12, 2, MAX_SEEDS), Some(4096));
        assert_eq!(grid_size(13, 2, MAX_SEEDS), None);
    }

    /// A field that vanishes everywhere has a root at every seed, so the
    /// number of points found is the number of Newton starts.
    fn zero_field(n: usize) -> FnField {
        FnField::new(n, 0, SystemKind::Ode, |_t, _x, _p, o| o.fill(0.0))
    }

    #[test]
    fn the_seed_budget_is_a_hard_cap_in_every_dimension() {
        // 2^10 = 1024 grid points do not fit in 1000: Halton instead.
        assert_eq!(seed_count(10, 2, 1000), 1000);
        assert_eq!(grid_size(10, 2, 1000), None);
        assert_eq!(seed_count(3, 9, 1000), 729);
        assert_eq!(seed_count(3, 10, 1000), 1000);
        assert_eq!(seed_count(3, 11, 1000), 1000);
        // The budget never exceeds MAX_SEEDS.
        assert_eq!(seed_count(2, 100, usize::MAX), MAX_SEEDS);
        for (n, per_axis, budget, want) in [
            (1, 40, 1000, 40),
            (2, 14, 1000, 196),
            (10, 2, 1000, 1000),
            (12, 2, 1000, 1000),
            (12, 2, 37, 37),
        ] {
            let f = zero_field(n);
            let fps = fixed_points_with_budget(&f, &[], &vec![(-1.0, 1.0); n], per_axis, budget);
            assert_eq!(
                fps.len(),
                want,
                "n = {n}, {per_axis} per axis, budget {budget}"
            );
        }
        assert!(fixed_points_with_budget(&zero_field(2), &[], &[(-1.0, 1.0); 2], 4, 0).is_empty());
        // Without a budget the old limit applies: 2^12 = 4096 grid points.
        assert_eq!(
            fixed_points(&zero_field(12), &[], &[(-1.0, 1.0); 12], 2).len(),
            4096
        );
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
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = 1.0 / x[0]);
        assert!(fixed_points(&f, &[], &[(-1.0, 1.0)], 20).is_empty());
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = (x[0] - 1.04) * (x[0] - 1.2)
        });
        let fps = fixed_points(&f, &[], &[(0.0, 1.0)], 10);
        assert_eq!(fps.len(), 1);
        assert!((fps[0].x[0] - 1.04).abs() < 1e-12);
    }

    #[test]
    fn default_finite_difference_jacobian_works() {
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
        expect_points(
            &fps,
            &[(&[xm, b * xm], SaddleND), (&[xp, b * xp], SaddleND)],
            1e-10,
        );
        for fp in &fps {
            check_eigenpairs(&f, &[a, b], fp, 1e-9);
            assert!(fp.eigenvectors.iter().all(Option::is_some));
        }
        let g = FnField::new(1, 1, SystemKind::Map, |_t, x, p, o| {
            o[0] = p[0] * x[0] * (1.0 - x[0])
        })
        .with_jacobian(|_t, x, p, j| j[0] = p[0] * (1.0 - 2.0 * x[0]));
        for fp in fixed_points(&g, &[2.5], &[(-1.5, 1.5)], 20) {
            check_eigenpairs(&g, &[2.5], &fp, 1e-12);
            assert_eq!(fp.eigenvectors, vec![Some(vec![1.0])]);
        }
    }

    #[test]
    fn eigenvectors_of_stars_degenerate_nodes_and_centres() {
        let fps = fixed_points(&linear([-2.0, 0.0, 0.0, -2.0]), &[], &[(-1.0, 1.0); 2], 3);
        let fp = &expect_points(&fps, &[(&[0.0, 0.0], StableStar)], 1e-12)[0];
        check_eigenpairs(&linear([-2.0, 0.0, 0.0, -2.0]), &[], fp, 1e-12);
        assert!(fp.eigenvectors.iter().all(Option::is_some));
        let f = linear([-1.0, 1.0, 0.0, -1.0]);
        let fps = fixed_points(&f, &[], &[(-1.0, 1.0); 2], 3);
        let fp = &expect_points(&fps, &[(&[0.0, 0.0], StableDegenerateNode)], 1e-12)[0];
        check_eigenpairs(&f, &[], fp, 1e-12);
        assert_eq!(fp.eigenvectors, vec![Some(vec![1.0, 0.0]), None]);
        let f = linear([0.0, 1.0, -4.0, 0.0]);
        let fps = fixed_points(&f, &[], &[(-1.0, 1.0); 2], 3);
        let fp = &expect_points(&fps, &[(&[0.0, 0.0], Centre)], 1e-12)[0];
        check_eigenpairs(&f, &[], fp, 1e-12);
        assert!(fp.eigenvectors_im.iter().all(Option::is_some));
        let f = linear([0.0, 0.0, 0.0, -1.0]);
        for fp in fixed_points(&f, &[], &[(-1.0, 1.0); 2], 3) {
            check_eigenpairs(&f, &[], &fp, 1e-12);
            assert!(fp.eigenvectors.iter().all(Option::is_some));
        }
    }

    #[test]
    fn rejected_inputs() {
        let f = rabbits_sheep();
        assert!(fixed_points(&f, &[], &[(0.0, 1.0)], 5).is_empty());
        assert!(fixed_points(&f, &[], &[(0.0, 1.0), (0.0, 1.0)], 0).is_empty());
        let g = FnField::new(1, 0, SystemKind::Ode, |t, x, _p, o| o[0] = x[0] - t).non_autonomous();
        assert!(fixed_points(&g, &[], &[(-1.0, 1.0)], 5).is_empty());
    }

    #[test]
    fn serialises_like_the_ui_type() {
        let fp = FixedPoint {
            x: vec![1.0, 2.0],
            eigenvalues: vec![(-1.0, 0.5), (-1.0, -0.5)],
            eigenvectors: vec![Some(vec![1.0, 0.0]), Some(vec![1.0, 0.0])],
            eigenvectors_im: vec![Some(vec![0.0, 0.5]), Some(vec![0.0, -0.5])],
            classification: StableSpiral,
            stable: true,
        };
        let json = serde_json::to_string(&fp).unwrap();
        assert_eq!(
            json,
            r#"{"x":[1.0,2.0],"eigenvalues":[[-1.0,0.5],[-1.0,-0.5]],"eigenvectors":[[1.0,0.0],[1.0,0.0]],"eigenvectorsIm":[[0.0,0.5],[0.0,-0.5]],"classification":"stable spiral","stable":true}"#
        );
        assert_eq!(serde_json::from_str::<FixedPoint>(&json).unwrap(), fp);
        let fp = FixedPoint {
            x: vec![0.0, 0.0],
            eigenvalues: vec![(-1.0, 0.0), (-1.0, 0.0)],
            eigenvectors: vec![Some(vec![1.0, 0.0]), None],
            eigenvectors_im: vec![None, None],
            classification: StableDegenerateNode,
            stable: true,
        };
        assert_eq!(
            serde_json::to_string(&fp).unwrap(),
            r#"{"x":[0.0,0.0],"eigenvalues":[[-1.0,0.0],[-1.0,0.0]],"eigenvectors":[[1.0,0.0],null],"eigenvectorsIm":[null,null],"classification":"stable degenerate node","stable":true}"#
        );
    }

    #[test]
    fn saddle_node_ghost_is_never_a_fixed_point() {
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
                    assert!(
                        fps.is_empty(),
                        "r = {r}, box ±{half_width}, {seeds}: {fps:?}"
                    );
                }
            }
        }
        let fps = fixed_points(&f, &[-1e-4], &[(-1.0, 1.0); 2], 14);
        expect_points(
            &fps,
            &[(&[-1e-2, 0.0], StableNode), (&[1e-2, 0.0], Saddle)],
            1e-12,
        );
        let fps = fixed_points(&f, &[-1e-4], &[(-100.0, 100.0); 2], 14);
        assert_eq!(fps.len(), 2, "{fps:?}");
    }

    #[test]
    fn roots_of_functions_with_large_values_are_found() {
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = 1e12 * (x[0] * x[0] - 2.0)
        })
        .with_jacobian(|_t, x, _p, j| j[0] = 2e12 * x[0]);
        let fps = fixed_points(&f, &[], &[(-3.0, 3.0)], 10);
        let r2 = 2f64.sqrt();
        expect_points(&fps, &[(&[-r2], Sink), (&[r2], Source)], 1e-12);
        let f = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| {
            o[0] = 1e12 * (1.0 + x[0].sin())
        })
        .with_jacobian(|_t, x, _p, j| j[0] = 1e12 * x[0].cos());
        let fps = fixed_points(&f, &[], &[(-3.0, 0.0)], 10);
        assert!(!fps.is_empty());
        for fp in &fps {
            assert!((fp.x[0] + PI / 2.0).abs() < 1e-6, "{fp:?}");
        }
    }

    #[test]
    fn infinite_slope_at_a_fixed_point() {
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
            r#"{"x":[0.0],"eigenvalues":[],"eigenvectors":[],"eigenvectorsIm":[],"classification":"non-hyperbolic","stable":false}"#
        );
    }
}
