use dyn_core::SystemKind;
use nalgebra::{Complex, DMatrix};

use crate::classify::{is_repeated_2d, repeat_resolution_2d, ABS_TOL, REL_TOL};

/// Eigenvectors matching a list of eigenvalues, one entry per eigenvalue.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Eigenvectors {
    /// For a real eigenvalue its unit eigenvector; for a complex one the
    /// real part of its eigenvector. `None` when the eigenvalue is
    /// defective and has no further independent eigenvector.
    pub re: Vec<Option<Vec<f64>>>,
    /// For a complex eigenvalue the imaginary part of its eigenvector;
    /// `None` for real eigenvalues (including those that only rounding
    /// made slightly complex) and missing vectors.
    pub im: Vec<Option<Vec<f64>>>,
}

const TIE_TOL: f64 = 1e-9;
const CIRCLE_TOL: f64 = 1e-8;
/// Relative distance below which eigenvalues count as identical rather
/// than merely close. It must sit below the repeat resolution
/// `sqrt(REL_TOL)`, or close eigenvalues would never be told apart.
const IDENT_TOL: f64 = 1e-8;
const _: () = assert!(IDENT_TOL * IDENT_TOL < REL_TOL);

/// Eigenvectors of the real `n x n` row-major matrix `a` for its
/// `eigenvalues` `(re, im)` (as returned by [`crate::eigenvalues`]).
///
/// `kind` says whether `a` is the Jacobian of a flow or the derivative of
/// a map: it decides which eigenvalues may share a repeated group (never
/// two of different stability).
///
/// Never produces NaN or infinity: if `a` or the eigenvalues are not
/// finite, or the list does not have `n` entries, every entry is `None`.
/// The zero matrix gets the standard basis.
///
/// # Normalisation
///
/// * **Real** `λ`: unit length, sign chosen so that the largest-magnitude
///   component is positive (the first one, if several are equal to within
///   `1e-9`). For identical repeated eigenvalues with several vectors
///   (stars) the vectors are orthonormal; any basis of the eigenspace is
///   valid.
/// * **Complex** `λ = α + iω`: the complex eigenvector `v = a + ib` with
///   `A v = λ v` is returned as its real part `a` ([`Eigenvectors::re`]) and
///   imaginary part `b` ([`Eigenvectors::im`]). Its free complex phase is
///   fixed so that `|a|² + |b|² = 1`, `a ⊥ b` and `|a| ≥ |b|`, so the
///   linearised orbits are ellipses with semi-axes along `a` (major) and
///   `b` (minor). The sign is fixed like a real vector's, by `a`. If the
///   ellipse is a circle (`|a| = |b|`, `a·b = 0` within `1e-8`) the
///   largest-magnitude component of `v` is made real and positive. The
///   conjugate eigenvalue gets the conjugate vector (`a`, `-b`).
pub fn eigenvectors(
    kind: SystemKind,
    n: usize,
    a: &[f64],
    eigenvalues: &[(f64, f64)],
) -> Eigenvectors {
    let len = eigenvalues.len();
    let mut out = Eigenvectors {
        re: vec![None; len],
        im: vec![None; len],
    };
    let finite = a.iter().all(|v| v.is_finite())
        && eigenvalues
            .iter()
            .all(|(re, im)| re.is_finite() && im.is_finite());
    if a.len() != n * n || len != n || n == 0 || !finite {
        return out;
    }
    let max_entry = a.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if max_entry == 0.0 {
        for (k, slot) in out.re.iter_mut().enumerate() {
            let mut e = vec![0.0; n];
            e[k] = 1.0;
            *slot = Some(e);
        }
        return out;
    }
    let flow_2d = kind == SystemKind::Ode && n == 2;
    let (tau, delta) = if n == 2 {
        (a[0] + a[3], a[0] * a[3] - a[1] * a[2])
    } else {
        (0.0, 0.0)
    };
    let s = eigenvalues
        .iter()
        .map(|&(re, im)| re.hypot(im))
        .fold(0.0, f64::max);
    let repeat_resolution = repeat_resolution(flow_2d, tau, delta, s, max_entry);
    let identity_tol = IDENT_TOL * if s > 0.0 { s } else { max_entry };
    let stability = Stability::new(kind, s);

    for group in groups(
        flow_2d,
        eigenvalues,
        repeat_resolution,
        tau,
        delta,
        &stability,
    ) {
        let m = group.len();
        let mean_re = if m == n {
            (0..n).map(|i| a[i * n + i]).sum::<f64>() / n as f64
        } else {
            group.iter().map(|&k| eigenvalues[k].0).sum::<f64>() / m as f64
        };
        let mean_im = group.iter().map(|&k| eigenvalues[k].1).sum::<f64>() / m as f64;
        let real = group
            .iter()
            .all(|&k| eigenvalues[k].1.abs() <= 0.5 * repeat_resolution);
        let vectors = GroupVectors {
            group: &group,
            ev: eigenvalues,
            identity_tol,
        };
        if real {
            let shifted = shift_real(n, a, mean_re);
            let is_star = m == n && shifted.iter().all(|v| v.abs() <= repeat_resolution);
            let Some(null) = real_null_space(n, &shifted) else {
                continue;
            };
            let g = if is_star {
                n
            } else {
                count_small(&null, repeat_resolution).clamp(1, m)
            };
            let assigned = vectors.assign(g, null, |cluster| {
                let lambda =
                    cluster.iter().map(|&k| eigenvalues[k].0).sum::<f64>() / cluster.len() as f64;
                real_null_space(n, &shift_real(n, a, lambda))
            });
            for (slot, v) in assigned {
                out.re[slot] = finite_vec(sign_normalised(v));
            }
        } else {
            let lower_half_plane = mean_im < 0.0;
            let mu = Complex::new(mean_re, mean_im.abs());
            let Some(null) = complex_null_space(n, a, mu) else {
                continue;
            };
            let g = count_small(&null, repeat_resolution).clamp(1, m);
            let assigned = vectors.assign(g, null, |cluster| {
                let c = cluster.len() as f64;
                let lambda = Complex::new(
                    cluster.iter().map(|&k| eigenvalues[k].0).sum::<f64>() / c,
                    cluster.iter().map(|&k| eigenvalues[k].1.abs()).sum::<f64>() / c,
                );
                complex_null_space(n, a, lambda)
            });
            for (slot, v) in assigned {
                let (re, mut im) = phase_normalised(v);
                if lower_half_plane {
                    im.iter_mut().for_each(|b| *b = -*b);
                }
                if let (Some(re), Some(im)) = (finite_vec(re), finite_vec(im)) {
                    out.re[slot] = Some(re);
                    out.im[slot] = Some(im);
                }
            }
        }
    }
    out
}

/// Singular values up to this size count as zero when deciding how many
/// eigenvectors a repeated group has. A nilpotent matrix (all eigenvalues
/// zero) has no eigenvalue scale, so it is measured against its entries.
fn repeat_resolution(flow_2d: bool, tau: f64, delta: f64, s: f64, max_entry: f64) -> f64 {
    let res = if flow_2d {
        repeat_resolution_2d(tau, delta)
    } else {
        REL_TOL.sqrt() * s
    };
    if res == 0.0 {
        REL_TOL.sqrt() * max_entry
    } else {
        res
    }
}

/// A group of repeated eigenvalues, ready to be given eigenvectors.
struct GroupVectors<'g> {
    group: &'g [usize],
    ev: &'g [(f64, f64)],
    identity_tol: f64,
}

impl GroupVectors<'_> {
    /// Pair members with eigenvectors, given the `g`-dimensional null space
    /// `null` of the group's shifted matrix.
    ///
    /// If every member has its own vector but the eigenvalues are only
    /// close, not identical, each cluster of identical ones gets the null
    /// space at its own value (`null_space_at`). Otherwise the members
    /// share the first `g` vectors of `null`, and the rest get none.
    fn assign<V>(
        &self,
        g: usize,
        null: Vec<(f64, V)>,
        null_space_at: impl Fn(&[usize]) -> Option<Vec<(f64, V)>>,
    ) -> Vec<(usize, V)> {
        let m = self.group.len();
        let per_member_vectors =
            g == m && m >= 2 && !identical(self.group, self.ev, self.identity_tol);
        if !per_member_vectors {
            return self
                .group
                .iter()
                .copied()
                .zip(null.into_iter().take(g).map(|(_, v)| v))
                .collect();
        }
        let mut out = Vec::with_capacity(m);
        for cluster in clusters(self.group, self.ev, self.identity_tol) {
            let Some(null) = null_space_at(&cluster) else {
                continue;
            };
            out.extend(cluster.into_iter().zip(null.into_iter().map(|(_, v)| v)));
        }
        out
    }
}

fn shift_real(n: usize, a: &[f64], mu: f64) -> Vec<f64> {
    let mut shifted = a.to_vec();
    for i in 0..n {
        shifted[i * n + i] -= mu;
    }
    shifted
}

fn identical(group: &[usize], ev: &[(f64, f64)], ident: f64) -> bool {
    let first = ev[group[0]];
    group
        .iter()
        .all(|&k| (ev[k].0 - first.0).hypot(ev[k].1 - first.1) <= ident)
}

fn clusters(group: &[usize], ev: &[(f64, f64)], ident: f64) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    for &k in group {
        let joined = out.iter_mut().find(|c| {
            let first = ev[c[0]];
            (ev[k].0 - first.0).hypot(ev[k].1 - first.1) <= ident
        });
        match joined {
            Some(c) => c.push(k),
            None => out.push(vec![k]),
        }
    }
    out
}

/// Where an eigenvalue sits relative to the stability boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StabilityClass {
    Attracting,
    Boundary,
    Repelling,
}

struct Stability {
    kind: SystemKind,
    flow_zero_tol: f64,
}

impl Stability {
    fn new(kind: SystemKind, s: f64) -> Self {
        Self {
            kind,
            flow_zero_tol: (REL_TOL * s).max(ABS_TOL),
        }
    }

    fn of(&self, (re, im): (f64, f64)) -> StabilityClass {
        let (v, zero) = match self.kind {
            SystemKind::Ode => (re, self.flow_zero_tol),
            SystemKind::Map => (re.hypot(im) - 1.0, REL_TOL),
        };
        if v.abs() <= zero {
            StabilityClass::Boundary
        } else if v < 0.0 {
            StabilityClass::Attracting
        } else {
            StabilityClass::Repelling
        }
    }
}

/// Which half of the complex plane an eigenvalue is in, with near-real
/// values counted as real.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HalfPlane {
    Real,
    Upper,
    Lower,
}

impl HalfPlane {
    fn of(im: f64, repeat_resolution: f64) -> HalfPlane {
        if im.abs() <= 0.5 * repeat_resolution {
            HalfPlane::Real
        } else if im > 0.0 {
            HalfPlane::Upper
        } else {
            HalfPlane::Lower
        }
    }
}

/// Indices of `ev` grouped into repeated eigenvalues. A 2D flow groups
/// exactly when its classification calls the eigenvalue repeated.
fn groups(
    flow_2d: bool,
    ev: &[(f64, f64)],
    repeat_resolution: f64,
    tau: f64,
    delta: f64,
    stability: &Stability,
) -> Vec<Vec<usize>> {
    if flow_2d {
        return if is_repeated_2d(tau, delta) {
            vec![vec![0, 1]]
        } else {
            vec![vec![0], vec![1]]
        };
    }
    let half = |&(_, im): &(f64, f64)| HalfPlane::of(im, repeat_resolution);
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for k in 0..ev.len() {
        let joined = groups.iter_mut().find(|g| {
            let first = ev[g[0]];
            half(&first) == half(&ev[k]) && stability.of(first) == stability.of(ev[k]) && {
                let im_gap = if half(&first) == HalfPlane::Real {
                    0.0
                } else {
                    first.1 - ev[k].1
                };
                (first.0 - ev[k].0).hypot(im_gap) <= repeat_resolution
            }
        });
        match joined {
            Some(g) => g.push(k),
            None => groups.push(vec![k]),
        }
    }
    groups
}

fn real_null_space(n: usize, m: &[f64]) -> Option<Vec<(f64, Vec<f64>)>> {
    let mat = DMatrix::from_row_slice(n, n, m);
    let svd = nalgebra::linalg::SVD::try_new(mat, false, true, f64::EPSILON, 1000 * n.max(10))?;
    let v_t = svd.v_t?;
    let mut pairs: Vec<(f64, Vec<f64>)> = (0..n)
        .map(|i| (svd.singular_values[i], v_t.row(i).iter().copied().collect()))
        .collect();
    pairs.sort_by(|p, q| p.0.total_cmp(&q.0));
    Some(pairs)
}

fn complex_null_space(
    n: usize,
    a: &[f64],
    mu: Complex<f64>,
) -> Option<Vec<(f64, Vec<Complex<f64>>)>> {
    let mat = DMatrix::from_fn(n, n, |i, j| {
        let v = Complex::new(a[i * n + j], 0.0);
        if i == j {
            v - mu
        } else {
            v
        }
    });
    let svd = nalgebra::linalg::SVD::try_new(mat, false, true, f64::EPSILON, 1000 * n.max(10))?;
    let v_t = svd.v_t?;
    // Rows of V^H are the conjugates of the columns of V.
    let mut pairs: Vec<(f64, Vec<Complex<f64>>)> = (0..n)
        .map(|i| {
            let v = v_t.row(i).iter().map(|z| z.conj()).collect();
            (svd.singular_values[i], v)
        })
        .collect();
    pairs.sort_by(|p, q| p.0.total_cmp(&q.0));
    Some(pairs)
}

fn count_small<V>(pairs: &[(f64, V)], res: f64) -> usize {
    pairs.iter().filter(|(s, _)| *s <= res).count()
}

fn dominant(mags: impl Iterator<Item = f64> + Clone) -> usize {
    let max = mags.clone().fold(0.0, f64::max);
    mags.into_iter()
        .position(|m| m >= (1.0 - TIE_TOL) * max)
        .unwrap_or(0)
}

fn sign_normalised(mut v: Vec<f64>) -> Vec<f64> {
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let k = dominant(v.iter().map(|x| x.abs()));
    let sign = if v[k] < 0.0 { -1.0 } else { 1.0 };
    let s = sign / norm;
    v.iter_mut().for_each(|x| *x *= s);
    v
}

fn phase_normalised(v: Vec<Complex<f64>>) -> (Vec<f64>, Vec<f64>) {
    let norm = v.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt();
    let v: Vec<Complex<f64>> = v.into_iter().map(|z| z / norm).collect();
    let dot = |p: &[Complex<f64>], f: fn(&Complex<f64>) -> f64, g: fn(&Complex<f64>) -> f64| {
        p.iter().map(|z| f(z) * g(z)).sum::<f64>()
    };
    let aa = dot(&v, |z| z.re, |z| z.re);
    let bb = dot(&v, |z| z.im, |z| z.im);
    let ab = dot(&v, |z| z.re, |z| z.im);
    let (p, q) = (aa - bb, 2.0 * ab);
    let theta = if p.hypot(q) <= CIRCLE_TOL * (aa + bb) {
        circle_phase(&v)
    } else {
        major_axis_phase(p, q)
    };
    let rot = Complex::from_polar(1.0, theta);
    let mut v: Vec<Complex<f64>> = v.into_iter().map(|z| z * rot).collect();
    let k = dominant(v.iter().map(|z| z.re.abs()));
    if v[k].re < 0.0 {
        v.iter_mut().for_each(|z| *z = -*z);
    }
    (
        v.iter().map(|z| z.re).collect(),
        v.iter().map(|z| z.im).collect(),
    )
}

/// For a circle: the rotation that makes the largest component real and
/// positive.
fn circle_phase(v: &[Complex<f64>]) -> f64 {
    let k = dominant(v.iter().map(|z| z.norm()));
    -v[k].arg()
}

/// The rotation that maximises `|Re v|`, given `p = |a|² - |b|²` and
/// `q = 2 a·b`.
fn major_axis_phase(p: f64, q: f64) -> f64 {
    0.5 * (-q).atan2(p)
}

fn finite_vec(v: Vec<f64>) -> Option<Vec<f64>> {
    v.iter().all(|x| x.is_finite()).then_some(v)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::eigenvalues;
    use dyn_core::SystemKind::{Map, Ode};

    fn check(n: usize, a: &[f64], tol: f64) -> (Vec<(f64, f64)>, Eigenvectors) {
        check_kind(Ode, n, a, tol)
    }

    fn check_kind(
        kind: SystemKind,
        n: usize,
        a: &[f64],
        tol: f64,
    ) -> (Vec<(f64, f64)>, Eigenvectors) {
        let ev = eigenvalues(n, a);
        let vecs = eigenvectors(kind, n, a, &ev);
        assert_residuals(n, a, &ev, &vecs, tol);
        (ev, vecs)
    }

    pub(crate) fn assert_residuals(
        n: usize,
        a: &[f64],
        ev: &[(f64, f64)],
        vecs: &Eigenvectors,
        tol: f64,
    ) {
        assert_eq!(vecs.re.len(), ev.len());
        assert_eq!(vecs.im.len(), ev.len());
        let mul = |v: &[f64]| -> Vec<f64> {
            (0..n)
                .map(|i| (0..n).map(|j| a[i * n + j] * v[j]).sum())
                .collect()
        };
        for (k, &(lr, li)) in ev.iter().enumerate() {
            let Some(re) = &vecs.re[k] else {
                assert!(vecs.im[k].is_none());
                continue;
            };
            let im = vecs.im[k].clone().unwrap_or_else(|| vec![0.0; n]);
            let rounded_real = vecs.im[k].is_none()
                && li != 0.0
                && li.abs() <= 0.5 * REL_TOL.sqrt() * 1f64.max(lr.hypot(li));
            assert!(
                vecs.im[k].is_some() == (li != 0.0) || rounded_real,
                "λ = {lr} + {li}i"
            );
            let li = if rounded_real { 0.0 } else { li };
            let norm: f64 = re.iter().chain(&im).map(|x| x * x).sum();
            assert!((norm - 1.0).abs() < 1e-12, "not unit: {re:?} {im:?}");
            let (ar, ai) = (mul(re), mul(&im));
            let scale = 1f64.max(lr.hypot(li));
            for i in 0..n {
                let r1 = ar[i] - (lr * re[i] - li * im[i]);
                let r2 = ai[i] - (li * re[i] + lr * im[i]);
                assert!(
                    r1.abs() <= tol * scale && r2.abs() <= tol * scale,
                    "A v != λ v for λ = {lr} + {li}i, v = {re:?} + i{im:?}: {r1:e}, {r2:e}"
                );
            }
            let k = dominant(re.iter().map(|x| x.abs()));
            assert!(re[k] > 0.0, "{re:?}");
        }
    }

    fn close(u: &[f64], v: &[f64], tol: f64) -> bool {
        u.len() == v.len() && u.iter().zip(v).all(|(a, b)| (a - b).abs() <= tol)
    }

    #[test]
    fn lorenz_origin() {
        let (s, r, b) = (10.0, 28.0, 8.0 / 3.0);
        let a = [-s, s, 0.0, r, -1.0, 0.0, 0.0, 0.0, -b];
        let (ev, vecs) = check(3, &a, 1e-9);
        let kz = ev.iter().position(|e| (e.0 + b).abs() < 1e-9).unwrap();
        assert!(close(
            vecs.re[kz].as_ref().unwrap(),
            &[0.0, 0.0, 1.0],
            1e-12
        ));
        for (k, v) in vecs.re.iter().enumerate() {
            let v = v.as_ref().unwrap();
            if k != kz {
                assert!(v[2].abs() < 1e-12);
                assert!((v[1] - (ev[k].0 + s) / s * v[0]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn rabbits_sheep_saddle() {
        let a = [-1.0, -2.0, -1.0, -1.0];
        let (ev, vecs) = check(2, &a, 1e-12);
        let r2 = 2f64.sqrt();
        let r3 = 3f64.sqrt();
        for (k, &(l, _)) in ev.iter().enumerate() {
            let v = vecs.re[k].as_ref().unwrap();
            let expect = if l > 0.0 {
                assert!((l - (r2 - 1.0)).abs() < 1e-14);
                [r2 / r3, -1.0 / r3]
            } else {
                assert!((l + 1.0 + r2).abs() < 1e-14);
                [r2 / r3, 1.0 / r3]
            };
            assert!(close(v, &expect, 1e-14), "{l}: {v:?}");
        }
    }

    #[test]
    fn stars_get_a_basis() {
        for a in [[-2.0, 0.0, 0.0, -2.0], [3.0, 0.0, 0.0, 3.0]] {
            let (_, vecs) = check(2, &a, 1e-12);
            let u = vecs.re[0].as_ref().unwrap();
            let v = vecs.re[1].as_ref().unwrap();
            assert!((u[0] * v[0] + u[1] * v[1]).abs() < 1e-15, "{u:?} {v:?}");
            assert!(vecs.im.iter().all(Option::is_none));
        }
        let a = [-2.0, 1e-12, -1e-12, -2.0 + 1e-12];
        let vecs = eigenvectors(Ode, 2, &a, &eigenvalues(2, &a));
        assert!(vecs.re.iter().all(Option::is_some), "{vecs:?}");
        let a = [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0];
        let (_, vecs) = check(3, &a, 1e-12);
        assert!(vecs.re.iter().all(Option::is_some));
    }

    #[test]
    fn degenerate_nodes_have_one_vector() {
        for a in [
            [-1.0, 1.0, 0.0, -1.0],
            [1.0, 0.0, 5.0, 1.0],
            [-1.0, 1.0, 1e-12, -1.0],
        ] {
            let (_, vecs) = check(2, &a, 1e-5);
            let found: Vec<_> = vecs.re.iter().flatten().collect();
            assert_eq!(found.len(), 1, "{a:?}: {vecs:?}");
            assert_eq!(vecs.re[1], None);
            let expect = if a[1] != 0.0 { [1.0, 0.0] } else { [0.0, 1.0] };
            assert!(close(found[0], &expect, 1e-5), "{a:?}: {found:?}");
        }
        use crate::{classify_jacobian, Classification};
        let (_, c) = classify_jacobian(dyn_core::SystemKind::Ode, 2, &[-1.0, 1.0, 1e-12, -1.0]);
        assert_eq!(c, Classification::StableDegenerateNode);
        let a = [2.0, 1.0, 0.0, 0.0, 2.0, 1.0, 0.0, 0.0, 2.0];
        let vecs = eigenvectors(Ode, 3, &a, &[(2.0, 0.0); 3]);
        assert!(close(vecs.re[0].as_ref().unwrap(), &[1.0, 0.0, 0.0], 1e-12));
        assert_eq!(vecs.re[1..], [None, None]);
    }

    #[test]
    fn tiny_jordan_block_straddling_the_absolute_tolerance_has_one_vector() {
        use crate::{classify_jacobian, Classification};
        let a = [-1e-12 * (1.0 + 1e-5), 1.0, 0.0, -1e-12 * (1.0 - 1e-5)];
        let (ev, c) = classify_jacobian(Ode, 2, &a);
        assert_eq!(c, Classification::StableDegenerateNode);
        let vecs = eigenvectors(Ode, 2, &a, &ev);
        assert_eq!(vecs.re.iter().flatten().count(), 1, "{vecs:?}");
    }

    #[test]
    fn complex_pairs_give_ellipse_axes() {
        let a = [0.0, 1.0, -4.0, 0.0];
        let (ev, vecs) = check(2, &a, 1e-12);
        assert_eq!(ev[0], (0.0, 2.0));
        let s5 = 5f64.sqrt();
        assert!(close(vecs.re[0].as_ref().unwrap(), &[0.0, 2.0 / s5], 1e-15));
        assert!(close(
            vecs.im[0].as_ref().unwrap(),
            &[-1.0 / s5, 0.0],
            1e-15
        ));
        assert_eq!(vecs.re[1], vecs.re[0]);
        assert!(close(vecs.im[1].as_ref().unwrap(), &[1.0 / s5, 0.0], 1e-15));
        for a in [
            [-0.1, 1.0, -1.0, -0.1],
            [0.5, -2.0, 3.0, 0.5],
            [1.0, 2.0, -5.0, -1.0],
        ] {
            let (_, vecs) = check(2, &a, 1e-12);
            let (re, im) = (vecs.re[0].as_ref().unwrap(), vecs.im[0].as_ref().unwrap());
            let dot = re[0] * im[0] + re[1] * im[1];
            assert!(dot.abs() < 1e-12, "{a:?}");
            assert!(re[0].hypot(re[1]) >= im[0].hypot(im[1]) - 1e-12);
        }
        let (_, vecs) = check(2, &[-0.1, 1.0, -1.0, -0.1], 1e-12);
        let (re, im) = (vecs.re[0].as_ref().unwrap(), vecs.im[0].as_ref().unwrap());
        assert!(close(re, &[0.5f64.sqrt(), 0.0], 1e-15), "{re:?}");
        assert!(close(im, &[0.0, 0.5f64.sqrt()], 1e-15), "{im:?}");
        let a = [-1.0, 2.0, 0.0, -2.0, -1.0, 0.0, 0.0, 0.0, 3.0];
        let (ev, vecs) = check(3, &a, 1e-12);
        let k3 = ev.iter().position(|e| e.1 == 0.0).unwrap();
        assert!(close(
            vecs.re[k3].as_ref().unwrap(),
            &[0.0, 0.0, 1.0],
            1e-12
        ));
    }

    #[test]
    fn one_dimensional_and_degenerate_inputs() {
        let (_, vecs) = check(1, &[-3.0], 1e-15);
        assert_eq!(vecs.re, vec![Some(vec![1.0])]);
        let vecs = eigenvectors(Ode, 1, &[0.0], &[(0.0, 0.0)]);
        assert_eq!(vecs.re, vec![Some(vec![1.0])]);
        let vecs = eigenvectors(Ode, 2, &[0.0; 4], &[(0.0, 0.0); 2]);
        assert_eq!(vecs.re, vec![Some(vec![1.0, 0.0]), Some(vec![0.0, 1.0])]);
        let (_, vecs) = check(2, &[0.0, 1.0, 0.0, 0.0], 1e-12);
        assert_eq!(vecs.re, vec![Some(vec![1.0, 0.0]), None]);
        let (_, vecs) = check(2, &[1.0, 2.0, 2.0, 4.0], 1e-12);
        assert!(vecs.re.iter().all(Option::is_some));
        let vecs = eigenvectors(Ode, 1, &[f64::INFINITY], &[(f64::INFINITY, 0.0)]);
        assert_eq!(vecs.re, vec![None]);
        let vecs = eigenvectors(Ode, 2, &[1.0, 0.0, 0.0, 1.0], &[]);
        assert!(vecs.re.is_empty());
        assert_eq!(eigenvectors(Ode, 0, &[], &[]), Eigenvectors::default());
        assert_eq!(eigenvectors(Map, 0, &[], &[]), Eigenvectors::default());
    }

    #[test]
    fn random_matrices() {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64 * 10.0 - 5.0
        };
        for n in 1..=6 {
            for _ in 0..200 {
                let a: Vec<f64> = (0..n * n).map(|_| next()).collect();
                let (_, vecs) = check(n, &a, 1e-9);
                assert!(vecs.re.iter().all(Option::is_some), "{a:?}");
            }
        }
    }

    #[test]
    fn ties_pick_the_first_component() {
        let (ev, vecs) = check(2, &[0.0, 1.0, 1.0, 0.0], 1e-15);
        let h = 0.5f64.sqrt();
        for (k, &(l, _)) in ev.iter().enumerate() {
            let expect = if l > 0.0 { [h, h] } else { [h, -h] };
            assert!(close(vecs.re[k].as_ref().unwrap(), &expect, 1e-15));
        }
    }

    fn conjugated(q: &[Vec<f64>], d: &[f64]) -> Vec<f64> {
        let n = d.len();
        let mut m = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                m[i * n + j] = (0..n).map(|k| q[i][k] * d[k] * q[j][k]).sum();
            }
        }
        m
    }

    fn parallel(u: &[f64], v: &[f64]) -> f64 {
        u.iter().zip(v).map(|(a, b)| a * b).sum::<f64>().abs()
    }

    fn vector_of(ev: &[(f64, f64)], vecs: &Eigenvectors, lambda: f64) -> Vec<f64> {
        let k = (0..ev.len())
            .min_by(|&i, &j| {
                (ev[i].0 - lambda)
                    .abs()
                    .total_cmp(&(ev[j].0 - lambda).abs())
            })
            .unwrap();
        vecs.re[k].clone().expect("a vector")
    }

    #[test]
    fn close_multipliers_of_a_map_saddle_are_not_swapped() {
        use crate::{classify_jacobian, Classification};
        let (up, down) = (1.0 + 1e-5, 1.0 - 1e-5);
        let a = [up, 0.0, 0.0, down];
        assert_eq!(classify_jacobian(Map, 2, &a).1, Classification::SaddleND);
        let (ev, vecs) = check_kind(Map, 2, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, up), vec![1.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, down), vec![0.0, 1.0]);
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let a = conjugated(&[vec![c, -s], vec![s, c]], &[up, down]);
        assert_eq!(classify_jacobian(Map, 2, &a).1, Classification::SaddleND);
        let (ev, vecs) = check_kind(Map, 2, &a, 1e-9);
        assert!(
            close(&vector_of(&ev, &vecs, up), &[c, s], 1e-10),
            "{vecs:?}"
        );
        assert!(
            close(&vector_of(&ev, &vecs, down), &[-s, c], 1e-10),
            "{vecs:?}"
        );
        let a = [up, 0.0, 0.0, 0.0, down, 0.0, 0.0, 0.0, 0.5];
        let (ev, vecs) = check_kind(Map, 3, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, up), vec![1.0, 0.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, down), vec![0.0, 1.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, 0.5), vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn close_eigenvalues_of_an_nd_flow_saddle_are_not_swapped() {
        use crate::{classify_jacobian, Classification};
        let q = [
            vec![0.6, 0.8, 0.0],
            vec![-0.8, 0.6, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        for gap in [1e-4, 1e-5, 1e-7] {
            let a = conjugated(&q, &[gap, -gap, 10.0]);
            assert_eq!(classify_jacobian(Ode, 3, &a).1, Classification::SaddleND);
            let (ev, vecs) = check(3, &a, 1e-12);
            let unstable = vector_of(&ev, &vecs, gap);
            let stable = vector_of(&ev, &vecs, -gap);
            assert!(
                parallel(&unstable, &[0.6, -0.8, 0.0]) > 1.0 - 1e-9,
                "{gap}: {unstable:?}"
            );
            assert!(
                parallel(&stable, &[0.8, 0.6, 0.0]) > 1.0 - 1e-9,
                "{gap}: {stable:?}"
            );
            assert!(close(&vector_of(&ev, &vecs, 10.0), &[0.0, 0.0, 1.0], 1e-12));
        }
        let a = [10.0, 0.0, 0.0, 0.0, 1e-4, 0.0, 0.0, 0.0, -1e-4];
        let (ev, vecs) = check(3, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, 1e-4), vec![0.0, 1.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, -1e-4), vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn nearly_repeated_distinct_eigenvalues_get_their_own_vectors() {
        use crate::{classify_jacobian, Classification};
        let a = [-1.0, 0.0, 0.0, -1.0 - 1e-5];
        assert_eq!(classify_jacobian(Ode, 2, &a).1, Classification::StableStar);
        let (ev, vecs) = check(2, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, -1.0), vec![1.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, -1.0 - 1e-5), vec![0.0, 1.0]);
        let (c, s) = (0.6, 0.8);
        let a = conjugated(&[vec![c, -s], vec![s, c]], &[-1.0, -1.0 - 1e-5]);
        let (ev, vecs) = check(2, &a, 1e-9);
        assert!(parallel(&vector_of(&ev, &vecs, -1.0), &[c, s]) > 1.0 - 1e-9);
        assert!(parallel(&vector_of(&ev, &vecs, -1.0 - 1e-5), &[-s, c]) > 1.0 - 1e-9);
        let a = [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0 - 1e-5];
        let (ev, vecs) = check(3, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, -1.0 - 1e-5), vec![0.0, 0.0, 1.0]);
        let plane: Vec<_> = (0..3)
            .filter(|&k| ev[k].0 == -1.0)
            .map(|k| vecs.re[k].clone().unwrap())
            .collect();
        assert_eq!(plane.len(), 2);
        assert!(plane.iter().all(|v| v[2] == 0.0));
        assert!(parallel(&plane[0], &plane[1]) < 1e-12);
        let w = 1.0 + 1e-6;
        let a = [
            -0.1, 1.0, 0.0, 0.0, -1.0, -0.1, 0.0, 0.0, 0.0, 0.0, -0.1, w, 0.0, 0.0, -w, -0.1,
        ];
        let (ev, vecs) = check(4, &a, 1e-12);
        for k in 0..4 {
            let (re, im) = (vecs.re[k].as_ref().unwrap(), vecs.im[k].as_ref().unwrap());
            let in_first = ev[k].1.abs() < 1.0 + 5e-7;
            let off: f64 = if in_first {
                re[2].hypot(re[3]).hypot(im[2].hypot(im[3]))
            } else {
                re[0].hypot(re[1]).hypot(im[0].hypot(im[1]))
            };
            assert!(off < 1e-9, "{k}: {ev:?} {vecs:?}");
        }
    }

    #[test]
    fn finite_difference_map_saddle_through_fixed_points() {
        use crate::{fixed_points, Classification};
        use dyn_core::FnField;
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let (up, down) = (1.0 + 1e-5, 1.0 - 1e-5);
        let j = conjugated(&[vec![c, -s], vec![s, c]], &[up, down]);
        let f = FnField::new(2, 0, Map, move |_t, x, _p, o| {
            o[0] = j[0] * x[0] + j[1] * x[1] + 0.3;
            o[1] = j[2] * x[0] + j[3] * x[1] - 0.2;
        });
        let fps = fixed_points(&f, &[], &[(-1e5, 1e5), (-1e5, 1e5)], 5);
        assert_eq!(fps.len(), 1, "{fps:?}");
        let fp = &fps[0];
        assert_eq!(fp.classification, Classification::SaddleND);
        let vecs = Eigenvectors {
            re: fp.eigenvectors.clone(),
            im: fp.eigenvectors_im.clone(),
        };
        assert!(
            close(&vector_of(&fp.eigenvalues, &vecs, up), &[c, s], 1e-4),
            "{fp:?}"
        );
        assert!(
            close(&vector_of(&fp.eigenvalues, &vecs, down), &[-s, c], 1e-4),
            "{fp:?}"
        );
    }

    #[test]
    fn rounded_complex_degenerate_node_gets_a_real_vector() {
        use crate::{classify_jacobian, Classification};
        let a = [-1.0, 1.0, -1e-12, -1.0];
        let (ev, c) = classify_jacobian(Ode, 2, &a);
        assert_eq!(c, Classification::StableDegenerateNode);
        assert!(ev[0].1 != 0.0);
        let (_, vecs) = check(2, &a, 1e-5);
        assert_eq!(vecs.re, vec![Some(vec![1.0, 0.0]), None]);
        assert_eq!(vecs.im, vec![None, None]);
        let a = [-1.0, 1.0, 0.0, -1e-12, -1.0, 0.0, 0.0, 0.0, 3.0];
        let (ev, vecs) = check(3, &a, 1e-5);
        let k = ev.iter().position(|e| e.0 < 0.0).unwrap();
        assert!(close(vecs.re[k].as_ref().unwrap(), &[1.0, 0.0, 0.0], 1e-5));
        assert!(vecs.im.iter().all(Option::is_none));
    }

    #[test]
    fn groups_never_mix_stability() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        for n in 2..=4 {
            for _ in 0..50 {
                let mut q: Vec<Vec<f64>> = Vec::new();
                while q.len() < n {
                    let mut v: Vec<f64> = (0..n).map(|_| next()).collect();
                    for u in &q {
                        let d: f64 = u.iter().zip(&v).map(|(a, b)| a * b).sum();
                        v.iter_mut().zip(u).for_each(|(x, y)| *x -= d * y);
                    }
                    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
                    if norm > 0.1 {
                        q.push(v.into_iter().map(|x| x / norm).collect());
                    }
                }
                let qt: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|k| q[k][i]).collect()).collect();
                let gap = 1e-5 * (1.5 + next());
                for (kind, centre, rest) in [(Ode, 0.0, 5.0), (Map, 1.0, 0.25)] {
                    let mut d = vec![centre + gap, centre - gap];
                    d.extend((2..n).map(|i| rest * (i as f64)));
                    let a = conjugated(&qt, &d);
                    let (_, vecs) = check_kind(kind, n, &a, 1e-9);
                    assert!(vecs.re.iter().all(Option::is_some), "{kind:?} {a:?}");
                }
            }
        }
    }
}
