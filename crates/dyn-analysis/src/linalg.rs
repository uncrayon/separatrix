/// Solve `a x = b` for a row-major `n x n` matrix `a` by LU decomposition
/// with partial pivoting. `None` if `a` is singular (a pivot below `1e-14`
/// of its largest entry) or not finite.
pub(crate) fn lu_solve(a: &[f64], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    debug_assert_eq!(a.len(), n * n);
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    let scale = a.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if !scale.is_finite() || scale == 0.0 {
        return None;
    }
    let tiny = 1e-14 * scale;
    for k in 0..n {
        let (piv, piv_abs) =
            (k..n)
                .map(|r| (r, a[r * n + k].abs()))
                .fold(
                    (k, -1.0),
                    |best, cur| if cur.1 > best.1 { cur } else { best },
                );
        if piv_abs <= tiny {
            return None;
        }
        if piv != k {
            for c in 0..n {
                a.swap(k * n + c, piv * n + c);
            }
            b.swap(k, piv);
        }
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
    for k in (0..n).rev() {
        let s: f64 = (k + 1..n).map(|c| a[k * n + c] * b[c]).sum();
        b[k] = (b[k] - s) / a[k * n + k];
    }
    Some(b)
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
            let mut ev: Vec<(f64, f64)> = m
                .complex_eigenvalues()
                .iter()
                .map(|z| (z.re, z.im))
                .collect();
            for k in 0..ev.len().saturating_sub(1) {
                if ev[k].1 < 0.0 && ev[k + 1].1 > 0.0 && ev[k].0 == ev[k + 1].0 {
                    ev.swap(k, k + 1);
                }
            }
            ev
        }
    }
}

/// Real roots of `λ² - τλ + δ = 0` (discriminant `disc >= 0`) without
/// cancellation: the larger-magnitude root first, the other as `δ / big`.
fn stable_quadratic_roots(tau: f64, delta: f64, disc: f64) -> (f64, f64) {
    let big = 0.5 * (tau + tau.signum() * disc.sqrt());
    if big == 0.0 {
        return (0.0, 0.0);
    }
    (big, delta / big)
}

pub(crate) fn eig2(tau: f64, delta: f64) -> Vec<(f64, f64)> {
    let disc = tau * tau - 4.0 * delta;
    if disc >= 0.0 {
        let (r1, r2) = stable_quadratic_roots(tau, delta, disc);
        let (l1, l2) = if r1 >= r2 { (r1, r2) } else { (r2, r1) };
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
        let a = [0.0, 2.0, 1.0, 1.0, 1.0, 0.0, 2.0, 0.0, 3.0];
        let x = [1.0, -2.0, 0.5];
        let b = [2.0 * -2.0 + 0.5, 1.0 - 2.0, 2.0 + 1.5];
        let sol = lu_solve(&a, &b).expect("regular");
        for (u, v) in sol.iter().zip(x) {
            assert!((u - v).abs() < 1e-14);
        }
    }

    #[test]
    fn lu_detects_singular() {
        let b = [1.0, 2.0];
        assert_eq!(lu_solve(&[1.0, 2.0, 2.0, 4.0], &b), None);
        assert_eq!(lu_solve(&[f64::NAN, 0.0, 0.0, 1.0], &b), None);
    }

    #[test]
    fn eigenvalues_small_closed_forms() {
        assert_eq!(eigenvalues(1, &[-3.0]), vec![(-3.0, 0.0)]);
        assert_eq!(
            eigenvalues(2, &[2.0, 0.0, 0.0, -1.0]),
            vec![(2.0, 0.0), (-1.0, 0.0)]
        );
        let ev = eigenvalues(2, &[0.0, 1.0, -4.0, 0.0]);
        assert_eq!(ev, vec![(0.0, 2.0), (0.0, -2.0)]);
        let ev = eigenvalues(2, &[1e8, 0.0, 0.0, 1e-8]);
        assert!((ev[1].0 - 1e-8).abs() < 1e-22);
    }

    #[test]
    fn eigenvalues_three_by_three() {
        let a = [-1.0, 2.0, 0.0, -2.0, -1.0, 0.0, 0.0, 0.0, 3.0];
        let mut ev = eigenvalues(3, &a);
        ev.sort_by(|p, q| p.0.total_cmp(&q.0).then(q.1.total_cmp(&p.1)));
        let expect = [(-1.0, 2.0), (-1.0, -2.0), (3.0, 0.0)];
        for (u, v) in ev.iter().zip(expect) {
            assert!(
                (u.0 - v.0).abs() < 1e-12 && (u.1 - v.1).abs() < 1e-12,
                "{ev:?}"
            );
        }
    }
}
