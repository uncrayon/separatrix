use dyn_core::VectorField;

use crate::{all_finite, blew_up, Solution, Stats, Status};

/// Iterate the map `x_{n+1} = f(n, x_n; p)` starting from `x_0 = x0`.
///
/// The first `transient` iterates are computed but not stored. The result
/// holds `x_k` for `k = transient, transient + 1, ..., transient + n`
/// (that is `n + 1` points when nothing goes wrong), and `t[i]` is the
/// index `k` as `f64`. With `transient = 0` the first point is `x0`
/// itself, which is what a cobweb diagram needs.
///
/// Stops early with [`Status::BlewUp`] (the large iterate is kept) or
/// [`Status::NonFinite`] (the bad iterate is dropped). `stats.accepted`
/// and `stats.evals` both count the iterations performed.
///
/// ```
/// use dyn_core::{FnField, SystemKind};
/// use dyn_solve::iterate_map;
///
/// // Logistic map x -> r x (1 - x).
/// let f = FnField::new(1, 1, SystemKind::Map, |_n, x, p, out| out[0] = p[0] * x[0] * (1.0 - x[0]));
/// let sol = iterate_map(&f, &[2.0], &[0.1], 3, 0);
/// assert_eq!(sol.t, vec![0.0, 1.0, 2.0, 3.0]);
/// assert!((sol.x[1] - 0.18).abs() < 1e-15);
/// ```
///
/// # Panics
/// If `x0.len() != f.dim()`.
pub fn iterate_map(
    f: &dyn VectorField,
    p: &[f64],
    x0: &[f64],
    n: usize,
    transient: usize,
) -> Solution {
    let dim = f.dim();
    assert_eq!(
        x0.len(),
        dim,
        "x0 has length {} but the system has dimension {}",
        x0.len(),
        dim
    );

    let mut sol = Solution {
        t: Vec::new(),
        x: Vec::new(),
        status: Status::Completed,
        stats: Stats::default(),
    };
    if !all_finite(x0) {
        sol.status = Status::NonFinite;
        return sol;
    }
    sol.t.reserve(n.saturating_add(1));
    sol.x.reserve(n.saturating_add(1).saturating_mul(dim));

    let mut x = x0.to_vec();
    let mut next = vec![0.0; dim];
    let record = |sol: &mut Solution, k: usize, x: &[f64]| {
        if k >= transient {
            sol.t.push(k as f64);
            sol.x.extend_from_slice(x);
        }
    };

    record(&mut sol, 0, &x);
    if blew_up(&x) {
        sol.status = Status::BlewUp;
        return sol;
    }
    let total = transient.saturating_add(n);
    for k in 0..total {
        f.eval(k as f64, &x, p, &mut next);
        sol.stats.evals += 1;
        if !all_finite(&next) {
            sol.status = Status::NonFinite;
            break;
        }
        std::mem::swap(&mut x, &mut next);
        sol.stats.accepted += 1;
        record(&mut sol, k + 1, &x);
        if blew_up(&x) {
            sol.status = Status::BlewUp;
            break;
        }
    }
    sol
}

#[cfg(test)]
mod tests {
    use super::*;
    use dyn_core::{FnField, SystemKind};

    fn logistic() -> FnField {
        FnField::new(1, 1, SystemKind::Map, |_n, x, p, out| {
            out[0] = p[0] * x[0] * (1.0 - x[0])
        })
    }

    #[test]
    fn logistic_converges_to_fixed_point() {
        let sol = iterate_map(&logistic(), &[2.5], &[0.1], 50, 200);
        assert_eq!(sol.status, Status::Completed);
        assert_eq!(sol.len(), 51);
        assert_eq!(sol.t[0], 200.0);
        assert_eq!(sol.t[50], 250.0);
        assert!(sol.x.iter().all(|&x| (x - 0.6).abs() < 1e-12));
        assert_eq!(sol.stats.accepted, 250);
        assert_eq!(sol.stats.evals, 250);
    }

    #[test]
    fn index_is_passed_as_time() {
        let f =
            FnField::new(1, 0, SystemKind::Map, |n, x, _p, out| out[0] = x[0] + n).non_autonomous();
        let sol = iterate_map(&f, &[], &[0.0], 10, 0);
        for (k, &x) in sol.x.iter().enumerate() {
            assert_eq!(x, (k * (k.max(1) - 1) / 2) as f64);
        }
    }

    #[test]
    fn escape_and_nan() {
        let sq = FnField::new(1, 0, SystemKind::Map, |_n, x, _p, out| out[0] = x[0] * x[0]);
        let sol = iterate_map(&sq, &[], &[2.0], 100, 0);
        assert_eq!(sol.status, Status::BlewUp);
        assert_eq!(sol.len(), 7);
        assert!(sol.last().unwrap().1[0] > 1e12);

        let ln = FnField::new(1, 0, SystemKind::Map, |_n, x, _p, out| out[0] = x[0].ln());
        let sol = iterate_map(&ln, &[], &[0.5], 10, 0);
        assert_eq!(sol.status, Status::NonFinite);
        assert_eq!(sol.len(), 2);
        assert!(all_finite(&sol.x));

        let sol = iterate_map(&ln, &[], &[f64::NAN], 10, 0);
        assert_eq!(sol.status, Status::NonFinite);
        assert!(sol.is_empty());
    }

    #[test]
    fn blow_up_during_transient_returns_no_points() {
        let sq = FnField::new(1, 0, SystemKind::Map, |_n, x, _p, out| out[0] = x[0] * x[0]);
        let sol = iterate_map(&sq, &[], &[2.0], 10, 50);
        assert_eq!(sol.status, Status::BlewUp);
        assert!(sol.is_empty());
    }

    #[test]
    fn two_dimensional_henon() {
        let f = FnField::new(2, 2, SystemKind::Map, |_n, x, p, out| {
            out[0] = x[1] + 1.0 - p[0] * x[0] * x[0];
            out[1] = p[1] * x[0];
        });
        let sol = iterate_map(&f, &[1.4, 0.3], &[0.0, 0.0], 1000, 100);
        assert_eq!(sol.status, Status::Completed);
        assert_eq!(sol.dim(), 2);
        assert!(sol.x.iter().all(|v| v.abs() < 2.0));
    }
}
