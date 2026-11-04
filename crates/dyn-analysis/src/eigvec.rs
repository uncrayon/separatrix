//! Eigenvectors of a fixed point's linearisation.
//!
//! [`eigenvectors`] returns one vector per eigenvalue, in the order of the
//! eigenvalues it is given (normally those of [`crate::eigenvalues`]).
//!
//! # Method
//!
//! 1. **Group** eigenvalues that count as repeated, at the same resolution
//!    as the classification (see [`crate::classify`]): for a 2D flow when
//!    `τ² - 4Δ ≈ 0`, otherwise (maps of any dimension, flows of other
//!    dimensions) when two eigenvalues of the same kind (real, `Im > 0`,
//!    `Im < 0`) are within `δ = sqrt(REL_TOL)·s` of each other,
//!    `s = max|λ|`. An eigenvalue with `|Im λ| ≤ δ/2` counts as real.
//!    Two eigenvalues whose *stability* differs are never grouped, however
//!    close: for flows the sign of `Re λ` beyond `max(REL_TOL·s, ABS_TOL)`,
//!    for maps the sign of `|μ| - 1` beyond `REL_TOL` (the rules of the
//!    classification). So the stable and unstable directions of a saddle
//!    are always told apart, e.g. for a map with multipliers `1 ± 1e-5`, or
//!    a 3D flow with `λ = ±1e-4` next to `λ = 10`.
//! 2. For a group with mean `μ` and `m` members, the eigenvectors span the
//!    null space of `A - μI`: take the right singular vectors (SVD, from
//!    `nalgebra`; complex arithmetic for complex `μ`) of the `g` smallest
//!    singular values, where `g` is the number of singular values `≤ δ`,
//!    clamped to `1..=m`. A simple eigenvalue therefore always gets its
//!    vector. When the group is the whole spectrum and real, `g = n`
//!    exactly when every entry of `A - μI` is `≤ δ` (`A ≈ λI`, the 2D
//!    *star* test of the classification), so stars get a full basis and
//!    degenerate nodes get a single vector, consistently with their class.
//! 3. **Assign** the vectors to the members of the group:
//!    * members that are identical (all within `IDENT_TOL·s = 1e-8·s`,
//!      well above the rounding of a finite-difference Jacobian), such as
//!      a true star `A = λI`: any basis of the eigenspace is valid, so the
//!      `g` orthonormal singular vectors, in order;
//!    * a diagonalisable group (`g = m`) whose members differ: each member
//!      `λ_k` gets the eigenvector of *its own* eigenvalue, the smallest
//!      right singular vector of `A - λ_k I` (members identical to each
//!      other share an orthonormal basis of their eigenspace). So a nearly
//!      repeated but distinct pair such as `diag(-1, -1 - 1e-5)`, which
//!      the classification calls a star, still gets each vector on its
//!      own eigenvalue;
//!    * a defective group (`g < m`, whose members differ only by the
//!      rounding of the repeated eigenvalue): the first `g` members get
//!      the vectors and the other `m - g` get `None` (a defective
//!      eigenvalue has fewer eigenvectors than its multiplicity: the
//!      degenerate node's missing direction).
//!
//! A repeated real group whose eigenvalues came out of rounding as a
//! slightly complex pair (`|Im λ| ≤ δ/2`, e.g. a degenerate node computed
//! as `-1 ± 1e-6 i`) is treated as real: its vector is real and its
//! [`Eigenvectors::im`] entry is `None`, although `Im λ ≠ 0`.
//!
//! # Normalisation
//!
//! * **Real** `λ`: unit length, sign chosen so that the largest-magnitude
//!   component is positive (the first one, if several are equal to within
//!   `1e-9`). For identical repeated eigenvalues with several vectors
//!   (stars) the vectors are orthonormal; any basis of the eigenspace is
//!   valid.
//! * **Complex** `λ = α + iω`: the complex eigenvector `v = a + ib` with
//!   `A v = λ v` is returned as its real part `a` ([`Eigenvectors::re`]) and
//!   imaginary part `b` ([`Eigenvectors::im`]). Its free complex phase is
//!   fixed so that `|a|² + |b|² = 1`, `a ⊥ b` and `|a| ≥ |b|`: then the
//!   linearised orbits `Re(c e^{λt} v)` are (spiralling) ellipses with
//!   semi-axes along `a` (major) and `b` (minor), which is what a plot
//!   needs. The sign is fixed like a real vector's, by `a`. If the ellipse
//!   is a circle (`|a| = |b|`, `a·b = 0` within `1e-8`) every phase works,
//!   and the largest-magnitude component of `v` is made real and positive.
//!   The conjugate eigenvalue gets the conjugate vector (`a`, `-b`).
//!
//! The plane spanned by `a` and `b` is the invariant plane of the pair.

use dyn_core::SystemKind;
use nalgebra::{Complex, DMatrix};

use crate::classify::{is_repeated_2d, repeat_resolution_2d, ABS_TOL, REL_TOL};

/// Eigenvectors matching a list of eigenvalues, one entry per eigenvalue
/// (see the [module docs](self) for the method and normalisation).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Eigenvectors {
    /// For a real eigenvalue its unit eigenvector; for a complex one the
    /// real part of its eigenvector. `None` when the eigenvalue is
    /// defective and has no further independent eigenvector.
    pub re: Vec<Option<Vec<f64>>>,
    /// For a complex eigenvalue the imaginary part of its eigenvector;
    /// `None` for real eigenvalues (including those that only rounding
    /// made slightly complex, see the [module docs](self)) and missing
    /// vectors.
    pub im: Vec<Option<Vec<f64>>>,
}

/// Two components count as equally large for the sign convention when
/// they agree to this relative tolerance.
const TIE_TOL: f64 = 1e-9;
/// Relative tolerance below which the ellipse of a complex eigenvector is a
/// circle (its axes are then undefined).
const CIRCLE_TOL: f64 = 1e-8;
/// Eigenvalues of a group closer than `IDENT_TOL·s` are the same eigenvalue
/// (step 3 of the module docs). Far above rounding and the error of a
/// central-difference Jacobian (about `1e-10` relative), far below the
/// grouping resolution `sqrt(REL_TOL) ≈ 3e-5`.
const IDENT_TOL: f64 = 1e-8;

/// Eigenvectors of the real `n x n` row-major matrix `a` for its
/// `eigenvalues` `(re, im)` (as returned by [`crate::eigenvalues`]).
///
/// `kind` says whether `a` is the Jacobian of a flow or the derivative of
/// a map: it decides which eigenvalues may share a repeated group (never
/// two of different stability; see the [module docs](self)).
///
/// Never produces NaN or infinity: if `a` or the eigenvalues are not
/// finite, or the list does not have `n` entries, every entry is `None`.
/// The zero matrix gets the standard basis.
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
    // Resolution at which eigenvalues count as repeated.
    let mut res = if flow_2d {
        repeat_resolution_2d(tau, delta)
    } else {
        REL_TOL.sqrt() * s
    };
    if res == 0.0 {
        // Nilpotent, e.g. [[0, 1], [0, 0]]: measure against the entries.
        res = REL_TOL.sqrt() * max_entry;
    }
    // Members closer than this are the same eigenvalue.
    let ident = IDENT_TOL * if s > 0.0 { s } else { max_entry };
    let stability = Stability::new(kind, s);

    for group in groups(flow_2d, eigenvalues, res, tau, delta, &stability) {
        let m = group.len();
        let mean_re = if m == n {
            (0..n).map(|i| a[i * n + i]).sum::<f64>() / n as f64
        } else {
            group.iter().map(|&k| eigenvalues[k].0).sum::<f64>() / m as f64
        };
        let mean_im = group.iter().map(|&k| eigenvalues[k].1).sum::<f64>() / m as f64;
        let real = group.iter().all(|&k| eigenvalues[k].1.abs() <= 0.5 * res);
        if real {
            let shifted = shift_real(n, a, mean_re);
            // The whole spectrum is one group and `A ≈ μI` (a star).
            let whole_scalar = m == n && shifted.iter().all(|v| v.abs() <= res);
            let Some(null) = real_null_space(n, &shifted) else {
                continue;
            };
            let g = if whole_scalar {
                n
            } else {
                count_small(&null, res).clamp(1, m)
            };
            let own = g == m && m >= 2 && !identical(&group, eigenvalues, ident);
            let assigned: Vec<(usize, Vec<f64>)> = if own {
                // Each member gets the vector of its own eigenvalue.
                let mut v = Vec::with_capacity(m);
                for cluster in clusters(&group, eigenvalues, ident) {
                    let lambda = cluster.iter().map(|&k| eigenvalues[k].0).sum::<f64>()
                        / cluster.len() as f64;
                    let Some(null) = real_null_space(n, &shift_real(n, a, lambda)) else {
                        continue;
                    };
                    v.extend(cluster.into_iter().zip(null.into_iter().map(|(_, v)| v)));
                }
                v
            } else {
                group
                    .iter()
                    .copied()
                    .zip(null.into_iter().take(g).map(|(_, v)| v))
                    .collect()
            };
            for (slot, v) in assigned {
                out.re[slot] = finite_vec(sign_normalised(v));
            }
        } else {
            // Work with the member of the conjugate pair with Im > 0 so
            // that conjugate groups get exactly conjugate vectors.
            let conj = mean_im < 0.0;
            let mu = Complex::new(mean_re, mean_im.abs());
            let Some(null) = complex_null_space(n, a, mu) else {
                continue;
            };
            let g = count_small(&null, res).clamp(1, m);
            let own = g == m && m >= 2 && !identical(&group, eigenvalues, ident);
            let assigned: Vec<(usize, Vec<Complex<f64>>)> = if own {
                let mut v = Vec::with_capacity(m);
                for cluster in clusters(&group, eigenvalues, ident) {
                    let c = cluster.len() as f64;
                    let lambda = Complex::new(
                        cluster.iter().map(|&k| eigenvalues[k].0).sum::<f64>() / c,
                        cluster.iter().map(|&k| eigenvalues[k].1.abs()).sum::<f64>() / c,
                    );
                    let Some(null) = complex_null_space(n, a, lambda) else {
                        continue;
                    };
                    v.extend(cluster.into_iter().zip(null.into_iter().map(|(_, v)| v)));
                }
                v
            } else {
                group
                    .iter()
                    .copied()
                    .zip(null.into_iter().take(g).map(|(_, v)| v))
                    .collect()
            };
            for (slot, v) in assigned {
                let (re, mut im) = phase_normalised(v);
                if conj {
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

/// `A - μI` for real `μ`.
fn shift_real(n: usize, a: &[f64], mu: f64) -> Vec<f64> {
    let mut shifted = a.to_vec();
    for i in 0..n {
        shifted[i * n + i] -= mu;
    }
    shifted
}

/// Are all members of `group` within `ident` of its first one?
fn identical(group: &[usize], ev: &[(f64, f64)], ident: f64) -> bool {
    let first = ev[group[0]];
    group
        .iter()
        .all(|&k| (ev[k].0 - first.0).hypot(ev[k].1 - first.1) <= ident)
}

/// Splits `group` into clusters of identical members (within `ident` of
/// the cluster's first member), in increasing index order.
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

/// The stability of an eigenvalue by the rules of the classification:
/// `-1` attracting, `+1` repelling, `0` on the boundary (see
/// [`crate::classify`]).
struct Stability {
    kind: SystemKind,
    /// Flows: `|Re λ|` at most this is zero.
    zero: f64,
}

impl Stability {
    fn new(kind: SystemKind, s: f64) -> Self {
        Self {
            kind,
            zero: (REL_TOL * s).max(ABS_TOL),
        }
    }

    fn of(&self, (re, im): (f64, f64)) -> i8 {
        let (v, zero) = match self.kind {
            SystemKind::Ode => (re, self.zero),
            SystemKind::Map => (re.hypot(im) - 1.0, REL_TOL),
        };
        if v.abs() <= zero {
            0
        } else if v < 0.0 {
            -1
        } else {
            1
        }
    }
}

/// Indices of the eigenvalues grouped into repeated clusters (step 1 of
/// the module docs), each group in increasing index order.
fn groups(
    flow_2d: bool,
    ev: &[(f64, f64)],
    res: f64,
    tau: f64,
    delta: f64,
    stability: &Stability,
) -> Vec<Vec<usize>> {
    if flow_2d {
        // Repeated eigenvalues of a 2D flow (Δ ≈ τ²/4 ≥ 0) always have
        // the same stability; the check is only a safeguard.
        let same = stability.of(ev[0]) == stability.of(ev[1]);
        return if same && is_repeated_2d(tau, delta) {
            vec![vec![0, 1]]
        } else {
            vec![vec![0], vec![1]]
        };
    }
    // Kind: 0 real, 1 upper half plane, 2 lower half plane.
    let kind = |&(_, im): &(f64, f64)| {
        if im.abs() <= 0.5 * res {
            0
        } else if im > 0.0 {
            1
        } else {
            2
        }
    };
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for k in 0..ev.len() {
        let joined = groups.iter_mut().find(|g| {
            let first = ev[g[0]];
            kind(&first) == kind(&ev[k]) && stability.of(first) == stability.of(ev[k]) && {
                let im_gap = if kind(&first) == 0 {
                    0.0
                } else {
                    first.1 - ev[k].1
                };
                (first.0 - ev[k].0).hypot(im_gap) <= res
            }
        });
        match joined {
            Some(g) => g.push(k),
            None => groups.push(vec![k]),
        }
    }
    groups
}

/// Right singular vectors of the real `n x n` matrix `m`, with their
/// singular values, smallest singular value first.
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

/// Right singular vectors of `A - μI` for complex `μ`, smallest singular
/// value first.
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

/// Number of singular values `≤ res`.
fn count_small<V>(pairs: &[(f64, V)], res: f64) -> usize {
    pairs.iter().filter(|(s, _)| *s <= res).count()
}

/// Index of the largest-magnitude entry of `mags` (the first one within
/// [`TIE_TOL`] of the maximum).
fn dominant(mags: impl Iterator<Item = f64> + Clone) -> usize {
    let max = mags.clone().fold(0.0, f64::max);
    mags.into_iter()
        .position(|m| m >= (1.0 - TIE_TOL) * max)
        .unwrap_or(0)
}

/// Scales `v` to unit length with its largest-magnitude component positive.
fn sign_normalised(mut v: Vec<f64>) -> Vec<f64> {
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let k = dominant(v.iter().map(|x| x.abs()));
    let sign = if v[k] < 0.0 { -1.0 } else { 1.0 };
    let s = sign / norm;
    v.iter_mut().for_each(|x| *x *= s);
    v
}

/// Real and imaginary parts of the complex vector `v`, with the phase fixed
/// as in the module docs.
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
        // Circle: make the largest component real and positive.
        let k = dominant(v.iter().map(|z| z.norm()));
        -v[k].arg()
    } else {
        // Maximise |Re(e^{iθ} v)|, which also makes Re ⊥ Im.
        0.5 * (-q).atan2(p)
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

/// `Some(v)` if every entry is finite.
fn finite_vec(v: Vec<f64>) -> Option<Vec<f64>> {
    v.iter().all(|x| x.is_finite()).then_some(v)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::eigenvalues;
    use dyn_core::SystemKind::{Map, Ode};

    /// `‖A v - λ v‖∞` for every returned vector of a flow's `A`, relative
    /// to `max(1, |λ|)`.
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
            // A real vector for a slightly complex λ: rounding of a
            // repeated real eigenvalue (|Im λ| ≤ δ/2, see the module docs).
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
            // A(a + ib) = (λr + iλi)(a + ib).
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
            // Sign convention.
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
        // λ = -b has eigenvector e_z; the others lie in the (x, y) plane.
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
                // (A - λ)v = 0, first row: v_y = (λ + σ)/σ · v_x.
                assert!((v[1] - (ev[k].0 + s) / s * v[0]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn rabbits_sheep_saddle() {
        // J at (1, 1) = [[-1, -2], [-1, -1]]: λ = -1 ± √2 with
        // eigenvectors (√2, ∓1)/√3.
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
        // Rounding-level coupling is still a star (as classified).
        let a = [-2.0, 1e-12, -1e-12, -2.0 + 1e-12];
        let vecs = eigenvectors(Ode, 2, &a, &eigenvalues(2, &a));
        assert!(vecs.re.iter().all(Option::is_some), "{vecs:?}");
        // 3D: -I has a full orthonormal basis.
        let a = [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0];
        let (_, vecs) = check(3, &a, 1e-12);
        assert!(vecs.re.iter().all(Option::is_some));
    }

    #[test]
    fn degenerate_nodes_have_one_vector() {
        for a in [
            [-1.0, 1.0, 0.0, -1.0],
            [1.0, 0.0, 5.0, 1.0],
            // Perturbed by rounding: eigenvalues -1 ± 1e-6 count as repeated.
            [-1.0, 1.0, 1e-12, -1.0],
        ] {
            let (_, vecs) = check(2, &a, 1e-5);
            let found: Vec<_> = vecs.re.iter().flatten().collect();
            assert_eq!(found.len(), 1, "{a:?}: {vecs:?}");
            assert_eq!(vecs.re[1], None);
            let expect = if a[1] != 0.0 { [1.0, 0.0] } else { [0.0, 1.0] };
            assert!(close(found[0], &expect, 1e-5), "{a:?}: {found:?}");
        }
        // The class agrees.
        use crate::{classify_jacobian, Classification};
        let (_, c) = classify_jacobian(dyn_core::SystemKind::Ode, 2, &[-1.0, 1.0, 1e-12, -1.0]);
        assert_eq!(c, Classification::StableDegenerateNode);
        // A 3x3 Jordan block (λ = 2): one vector, two missing.
        let a = [2.0, 1.0, 0.0, 0.0, 2.0, 1.0, 0.0, 0.0, 2.0];
        let vecs = eigenvectors(Ode, 3, &a, &[(2.0, 0.0); 3]);
        assert!(close(vecs.re[0].as_ref().unwrap(), &[1.0, 0.0, 0.0], 1e-12));
        assert_eq!(vecs.re[1..], [None, None]);
    }

    #[test]
    fn complex_pairs_give_ellipse_axes() {
        // Centre x' = y, y' = -4x: λ = ±2i, orbits 4x² + y² = c with the
        // major axis along y.
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
        // The conjugate gets the conjugate vector.
        assert_eq!(vecs.re[1], vecs.re[0]);
        assert!(close(vecs.im[1].as_ref().unwrap(), &[1.0 / s5, 0.0], 1e-15));
        // Spirals: Re ⊥ Im and |Re| ≥ |Im|.
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
        // Circular case: the largest component is real and positive.
        let (_, vecs) = check(2, &[-0.1, 1.0, -1.0, -0.1], 1e-12);
        let (re, im) = (vecs.re[0].as_ref().unwrap(), vecs.im[0].as_ref().unwrap());
        assert!(close(re, &[0.5f64.sqrt(), 0.0], 1e-15), "{re:?}");
        assert!(close(im, &[0.0, 0.5f64.sqrt()], 1e-15), "{im:?}");
        // 3D: rotation block plus a real direction.
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
        // Zero matrix: every vector is an eigenvector.
        let vecs = eigenvectors(Ode, 2, &[0.0; 4], &[(0.0, 0.0); 2]);
        assert_eq!(vecs.re, vec![Some(vec![1.0, 0.0]), Some(vec![0.0, 1.0])]);
        // Nilpotent: one vector.
        let (_, vecs) = check(2, &[0.0, 1.0, 0.0, 0.0], 1e-12);
        assert_eq!(vecs.re, vec![Some(vec![1.0, 0.0]), None]);
        // Non-isolated with τ ≠ 0: both directions.
        let (_, vecs) = check(2, &[1.0, 2.0, 2.0, 4.0], 1e-12);
        assert!(vecs.re.iter().all(Option::is_some));
        // Not finite / wrong length: all missing, never NaN.
        let vecs = eigenvectors(Ode, 1, &[f64::INFINITY], &[(f64::INFINITY, 0.0)]);
        assert_eq!(vecs.re, vec![None]);
        let vecs = eigenvectors(Ode, 2, &[1.0, 0.0, 0.0, 1.0], &[]);
        assert!(vecs.re.is_empty());
        assert_eq!(eigenvectors(Ode, 0, &[], &[]), Eigenvectors::default());
        assert_eq!(eigenvectors(Map, 0, &[], &[]), Eigenvectors::default());
    }

    #[test]
    fn random_matrices() {
        // A small LCG: deterministic pseudo-random entries in [-5, 5).
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
        // x' = y, y' = x: λ = 1 with (1, 1)/√2, λ = -1 with (1, -1)/√2.
        let (ev, vecs) = check(2, &[0.0, 1.0, 1.0, 0.0], 1e-15);
        let h = 0.5f64.sqrt();
        for (k, &(l, _)) in ev.iter().enumerate() {
            let expect = if l > 0.0 { [h, h] } else { [h, -h] };
            assert!(close(vecs.re[k].as_ref().unwrap(), &expect, 1e-15));
        }
    }

    /// `Q diag(d) Qᵀ` for an orthogonal `q` (rows), row-major `n x n`.
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

    /// `|u · v|` for unit vectors: 1 when they are parallel (either sign).
    fn parallel(u: &[f64], v: &[f64]) -> f64 {
        u.iter().zip(v).map(|(a, b)| a * b).sum::<f64>().abs()
    }

    /// The vector of the eigenvalue closest to `lambda`.
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
        // μ = 1 ± 1e-5: classified as a saddle (|μ| - 1 = ±1e-5 is far
        // beyond REL_TOL), so the vectors must not be mixed up, although
        // the multipliers are closer than the grouping resolution.
        let (up, down) = (1.0 + 1e-5, 1.0 - 1e-5);
        let a = [up, 0.0, 0.0, down];
        assert_eq!(classify_jacobian(Map, 2, &a).1, Classification::SaddleND);
        let (ev, vecs) = check_kind(Map, 2, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, up), vec![1.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, down), vec![0.0, 1.0]);
        // The same map rotated by 30°: the unstable direction is
        // (cos 30°, sin 30°), the stable one (-sin 30°, cos 30°).
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
        // In 3D, next to a strongly contracting direction.
        let a = [up, 0.0, 0.0, 0.0, down, 0.0, 0.0, 0.0, 0.5];
        let (ev, vecs) = check_kind(Map, 3, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, up), vec![1.0, 0.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, down), vec![0.0, 1.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, 0.5), vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn close_eigenvalues_of_an_nd_flow_saddle_are_not_swapped() {
        use crate::{classify_jacobian, Classification};
        // λ = ±1e-4 next to λ = 10, in a basis rotated by about 53° in the
        // (x, y) plane: (0.6, -0.8, 0) is unstable, (0.8, 0.6, 0) stable.
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
        // On the axes too.
        let a = [10.0, 0.0, 0.0, 0.0, 1e-4, 0.0, 0.0, 0.0, -1e-4];
        let (ev, vecs) = check(3, &a, 1e-9);
        assert_eq!(vector_of(&ev, &vecs, 1e-4), vec![0.0, 1.0, 0.0]);
        assert_eq!(vector_of(&ev, &vecs, -1e-4), vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn nearly_repeated_distinct_eigenvalues_get_their_own_vectors() {
        use crate::{classify_jacobian, Classification};
        // A 2D flow with λ = -1 and -1 - 1e-5 (within the star resolution):
        // classified as a star, but each vector still belongs to its own
        // eigenvalue, in the axes and rotated.
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
        // 3D: a repeated pair next to a close but distinct eigenvalue. The
        // repeated pair keeps an orthonormal basis of its plane.
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
        // Two close complex pairs of the same stability (4D, block
        // diagonal): each pair gets the vector of its own plane.
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
        // The affine map x ↦ J x + c with J the rotated multipliers 1 ± 1e-5
        // (its fixed point is far out, about 3e4 from the origin) and a
        // central-difference Jacobian.
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
        // J = [[-1, 1], [-1e-12, -1]]: λ = -1 ± 1e-6 i by rounding of a
        // repeated eigenvalue. The class is a degenerate node, and the
        // vector is real (Im None) although Im λ ≠ 0.
        let a = [-1.0, 1.0, -1e-12, -1.0];
        let (ev, c) = classify_jacobian(Ode, 2, &a);
        assert_eq!(c, Classification::StableDegenerateNode);
        assert!(ev[0].1 != 0.0);
        let (_, vecs) = check(2, &a, 1e-5);
        assert_eq!(vecs.re, vec![Some(vec![1.0, 0.0]), None]);
        assert_eq!(vecs.im, vec![None, None]);
        // The same in 3D.
        let a = [-1.0, 1.0, 0.0, -1e-12, -1.0, 0.0, 0.0, 0.0, 3.0];
        let (ev, vecs) = check(3, &a, 1e-5);
        let k = ev.iter().position(|e| e.0 < 0.0).unwrap();
        assert!(close(vecs.re[k].as_ref().unwrap(), &[1.0, 0.0, 0.0], 1e-5));
        assert!(vecs.im.iter().all(Option::is_none));
    }

    #[test]
    fn groups_never_mix_stability() {
        // Random saddles with an unstable/stable pair straddling the
        // boundary by less than the grouping resolution: every vector is an
        // eigenvector of its own eigenvalue (a residual far below the gap,
        // which a swapped vector would show), for flows
        // (pair ±g around 0) and maps (pair 1 ± g).
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        for n in 2..=4 {
            for _ in 0..50 {
                // A random rotation-ish orthogonal basis by Gram–Schmidt.
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
                // Columns of the basis: transpose the rows.
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
