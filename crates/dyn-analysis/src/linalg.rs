//! Tiny dense linear algebra: LU solve for Newton steps and eigenvalues.
//!
//! Matrices are row-major `n x n` slices, like the Jacobians of
//! [`dyn_core::VectorField`].

/// Solves `A x = b` in place by Gaussian elimination with partial pivoting.
///
/// On entry `a` holds `A` (row-major, `n x n`) and `b` holds the right-hand
/// side; on success `b` holds `x` and `a` is overwritten by its LU factors.
/// Returns `false` (leaving garbage in `a` and `b`) if `A` is singular to
/// working precision, i.e. a pivot is below `1e-14 * max|A_ij|`, or if `A`
/// contains non-finite entries.
pub(crate) fn lu_solve(a: &mut [f64], b: &mut [f64]) -> bool {
    let n = b.len();
    debug_assert_eq!(a.len(), n * n);
    let scale = a.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if !scale.is_finite() || scale == 0.0 {
        return false;
    }
    let tiny = 1e-14 * scale;
    for k in 0..n {
        // Pick the largest pivot in column k at or below the diagonal.
        let (piv, piv_abs) = (k..n)
            .map(|r| (r, a[r * n + k].abs()))
            .fold((k, -1.0), |best, cur| if cur.1 > best.1 { cur } else { best });
        if piv_abs <= tiny {
            return false;
        }
        if piv != k {
            for c in 0..n {
                a.swap(k * n + c, piv * n + c);
            }
            b.swap(k, piv);
        }
        // Eliminate below the pivot.
        for r in k + 1..n {
            let m = a[r * n + k] / a[k * n + k];
            if m != 0.0 {
                for c in k..n {
                    a[r * n + c] -= m * a[k * n + c];
                }
                b[r] -= m * b[k];
            }
        }
    }
    // Back substitution.
    for k in (0..n).rev() {
        let s: f64 = (k + 1..n).map(|c| a[k * n + c] * b[c]).sum();
        b[k] = (b[k] - s) / a[k * n + k];
    }
    true
}

/// Eigenvalues `(re, im)` of a real `n x n` row-major matrix.
///
/// 1x1 and 2x2 use closed forms (exact and cheap); larger matrices use the
/// real Schur decomposition from `nalgebra`. Complex pairs are returned with
/// the positive imaginary part first. The order is otherwise unspecified.
pub fn eigenvalues(n: usize, a: &[f64]) -> Vec<(f64, f64)> {
    assert_eq!(a.len(), n * n, "matrix must be n x n");
    match n {
        0 => Vec::new(),
        1 => vec![(a[0], 0.0)],
        2 => {
            let tau = a[0] + a[3];
            let delta = a[0] * a[3] - a[1] * a[2];
            eig2(tau, delta)
        }
        _ => {
            let m = nalgebra::DMatrix::from_row_slice(n, n, a);
            let mut ev: Vec<(f64, f64)> =
                m.complex_eigenvalues().iter().map(|z| (z.re, z.im)).collect();
            // Schur gives each conjugate pair; put +im first for consistency.
            for k in 0..ev.len().saturating_sub(1) {
                if ev[k].1 < 0.0 && ev[k + 1].1 > 0.0 && ev[k].0 == ev[k + 1].0 {
                    ev.swap(k, k + 1);
                }
            }
            ev
        }
    }
}

/// Eigenvalues of a 2x2 matrix with trace `tau` and determinant `delta`:
/// `λ = (τ ± sqrt(τ² - 4Δ)) / 2`.
pub(crate) fn eig2(tau: f64, delta: f64) -> Vec<(f64, f64)> {
    let disc = tau * tau - 4.0 * delta;
    if disc >= 0.0 {
        let s = disc.sqrt();
        // Avoid cancellation: compute the larger-magnitude root first.
        let big = 0.5 * (tau + tau.signum() * s);
        if big == 0.0 {
            return vec![(0.0, 0.0), (0.0, 0.0)];
        }
        let small = delta / big;
        let (l1, l2) = if big >= small { (big, small) } else { (small, big) };
        vec![(l1, 0.0), (l2, 0.0)]
    } else {
        let im = 0.5 * (-disc).sqrt();
        vec![(0.5 * tau, im), (0.5 * tau, -im)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lu_solves_with_pivoting() {
        // First pivot is zero: needs a row swap.
        let mut a = [0.0, 2.0, 1.0, 1.0, 1.0, 0.0, 2.0, 0.0, 3.0];
        let x = [1.0, -2.0, 0.5];
        let mut b = [2.0 * -2.0 + 0.5, 1.0 - 2.0, 2.0 + 1.5];
        assert!(lu_solve(&mut a, &mut b));
        for (u, v) in b.iter().zip(x) {
            assert!((u - v).abs() < 1e-14);
        }
    }

    #[test]
    fn lu_detects_singular() {
        let mut a = [1.0, 2.0, 2.0, 4.0];
        let mut b = [1.0, 2.0];
        assert!(!lu_solve(&mut a, &mut b));
        let mut a = [f64::NAN, 0.0, 0.0, 1.0];
        assert!(!lu_solve(&mut a, &mut b));
    }

    #[test]
    fn eigenvalues_small_closed_forms() {
        assert_eq!(eigenvalues(1, &[-3.0]), vec![(-3.0, 0.0)]);
        assert_eq!(eigenvalues(2, &[2.0, 0.0, 0.0, -1.0]), vec![(2.0, 0.0), (-1.0, 0.0)]);
        let ev = eigenvalues(2, &[0.0, 1.0, -4.0, 0.0]);
        assert_eq!(ev, vec![(0.0, 2.0), (0.0, -2.0)]);
        // Large spread: the small root is computed without cancellation.
        let ev = eigenvalues(2, &[1e8, 0.0, 0.0, 1e-8]);
        assert!((ev[1].0 - 1e-8).abs() < 1e-22);
    }

    #[test]
    fn eigenvalues_three_by_three() {
        // Block diag(rotation-ish 2x2 with eigenvalues -1 ± 2i, 3).
        let a = [-1.0, 2.0, 0.0, -2.0, -1.0, 0.0, 0.0, 0.0, 3.0];
        let mut ev = eigenvalues(3, &a);
        ev.sort_by(|p, q| p.0.total_cmp(&q.0).then(q.1.total_cmp(&p.1)));
        let expect = [(-1.0, 2.0), (-1.0, -2.0), (3.0, 0.0)];
        for (u, v) in ev.iter().zip(expect) {
            assert!((u.0 - v.0).abs() < 1e-12 && (u.1 - v.1).abs() < 1e-12, "{ev:?}");
        }
    }
}
