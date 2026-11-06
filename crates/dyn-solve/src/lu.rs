//! Dense LU factorization with partial pivoting, for the small linear
//! systems `(I − h·d·J)·k = b` of the Rosenbrock method.

/// `P·A = L·U` of a square matrix, `L` unit lower triangular, stored in one
/// row-major array.
pub(crate) struct Lu {
    n: usize,
    a: Vec<f64>,
    piv: Vec<usize>,
}

/// A zero (or non-finite) pivot: the matrix is singular to working precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Singular;

impl Lu {
    pub(crate) fn new(n: usize) -> Lu {
        Lu {
            n,
            a: vec![0.0; n * n],
            piv: vec![0; n],
        }
    }

    /// Factor `I − c·J`, `J` row-major `n × n`.
    pub(crate) fn factor_shifted(&mut self, c: f64, jac: &[f64]) -> Result<(), Singular> {
        let n = self.n;
        for i in 0..n {
            for j in 0..n {
                let id = if i == j { 1.0 } else { 0.0 };
                self.a[i * n + j] = id - c * jac[i * n + j];
            }
        }
        self.factor()
    }

    fn factor(&mut self) -> Result<(), Singular> {
        let n = self.n;
        let a = &mut self.a;
        for k in 0..n {
            let mut p = k;
            for i in k + 1..n {
                if a[i * n + k].abs() > a[p * n + k].abs() {
                    p = i;
                }
            }
            self.piv[k] = p;
            let pivot = a[p * n + k];
            if pivot == 0.0 || !pivot.is_finite() {
                return Err(Singular);
            }
            if p != k {
                for j in 0..n {
                    a.swap(k * n + j, p * n + j);
                }
            }
            for i in k + 1..n {
                let l = a[i * n + k] / pivot;
                a[i * n + k] = l;
                if l != 0.0 {
                    for j in k + 1..n {
                        a[i * n + j] -= l * a[k * n + j];
                    }
                }
            }
        }
        Ok(())
    }

    /// Overwrite `b` with the solution of `A·x = b`.
    pub(crate) fn solve(&self, b: &mut [f64]) {
        let n = self.n;
        let a = &self.a;
        for k in 0..n {
            b.swap(k, self.piv[k]);
        }
        for i in 1..n {
            let mut s = b[i];
            for j in 0..i {
                s -= a[i * n + j] * b[j];
            }
            b[i] = s;
        }
        for i in (0..n).rev() {
            let mut s = b[i];
            for j in i + 1..n {
                s -= a[i * n + j] * b[j];
            }
            b[i] = s / a[i * n + i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solve_shifted(c: f64, jac: &[f64], b: &[f64]) -> Result<Vec<f64>, Singular> {
        let mut lu = Lu::new(b.len());
        lu.factor_shifted(c, jac)?;
        let mut x = b.to_vec();
        lu.solve(&mut x);
        Ok(x)
    }

    #[test]
    fn solves_a_system_that_needs_a_row_swap() {
        // I − J = [[0, 2], [3, 1]]: the first pivot is zero without pivoting.
        let jac = [1.0, -2.0, -3.0, 0.0];
        let x = solve_shifted(1.0, &jac, &[4.0, 5.0]).unwrap();
        assert_eq!(x, vec![1.0, 2.0]);
    }

    #[test]
    fn solves_a_3x3_system_to_rounding() {
        // A = I − J = [[2, 1, 1], [4, -6, 0], [-2, 7, 2]], x = (1, 2, 3).
        let a = [2.0, 1.0, 1.0, 4.0, -6.0, 0.0, -2.0, 7.0, 2.0];
        let jac: Vec<f64> = (0..9)
            .map(|k| if k % 4 == 0 { 1.0 - a[k] } else { -a[k] })
            .collect();
        let x = solve_shifted(1.0, &jac, &[7.0, -8.0, 18.0]).unwrap();
        for (xi, want) in x.iter().zip([1.0, 2.0, 3.0]) {
            assert!((xi - want).abs() < 1e-14, "{x:?}");
        }
    }

    #[test]
    fn a_singular_matrix_is_reported() {
        // I − J = [[1, 2], [2, 4]].
        let jac = [0.0, -2.0, -2.0, -3.0];
        assert_eq!(solve_shifted(1.0, &jac, &[1.0, 1.0]), Err(Singular));
        assert_eq!(solve_shifted(1.0, &[f64::NAN], &[1.0]), Err(Singular));
    }
}
