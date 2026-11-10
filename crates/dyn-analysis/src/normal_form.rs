//! Normal-form coefficients of codimension-1 bifurcations (Yu. A.
//! Kuznetsov, *Elements of Applied Bifurcation Theory*, 3rd ed., §3.5,
//! §5.4, §8.7). Where one vanishes the bifurcation degenerates, so along a
//! curve of bifurcations each is the test function of a codimension-2
//! point: a cusp on a fold curve, a Bautin point on a Hopf curve, a
//! generalized flip on a flip curve, a Chenciner point on a Neimark–Sacker
//! curve.
//!
//! Inner products are `⟨u, v⟩ = ūᵀv`. A complex left vector is kept as
//! `p̄`, so that `⟨p, v⟩` is the plain sum `p̄ · v`.

use dyn_core::{SystemKind, VectorField};

use crate::continuation::{dot, identity, matvec, norm2, norm_inf, Power};
use crate::eigvec::eigenvectors;
use crate::linalg::lu_solve;

/// `A`, `B` and `C` of `f` (of `f^k` for a map) at `x`. `B(u, v) = D(u)·v`,
/// where `D(u)` is the derivative of the Jacobian along `u`, exact where
/// `jacobian_derivative` is. `C(u, u, v)` is a difference of `D(u)` along
/// `u`, with a step of `2⁻¹⁷ · max(|x|∞, 1)`.
pub(crate) struct Taylor<'a> {
    power: &'a Power<'a>,
    x: &'a [f64],
    p: &'a [f64],
    a: Vec<f64>,
}

impl<'a> Taylor<'a> {
    pub(crate) fn new(power: &'a Power<'a>, x: &'a [f64], p: &'a [f64]) -> Self {
        let n = x.len();
        let mut a = vec![0.0; n * n];
        power.jacobian(0.0, x, p, &mut a);
        Taylor { power, x, p, a }
    }

    fn dim(&self) -> usize {
        self.x.len()
    }

    fn derivative(&self, x: &[f64], u: &[f64]) -> Vec<f64> {
        let n = self.dim();
        let mut out = vec![0.0; n * n];
        let still = vec![0.0; self.p.len()];
        self.power
            .jacobian_derivative(0.0, x, self.p, u, &still, &mut out);
        out
    }

    fn b(&self, u: &[f64], v: &[f64]) -> Vec<f64> {
        matvec(&self.derivative(self.x, u), v, self.dim())
    }

    fn b_complex(&self, u: &CVector, v: &CVector) -> CVector {
        let n = self.dim();
        let (du_re, du_im) = (
            self.derivative(self.x, &u.re),
            self.derivative(self.x, &u.im),
        );
        let product = |d: &[f64], w: &[f64]| matvec(d, w, n);
        CVector {
            re: sum(
                &product(&du_re, &v.re),
                &scaled(&product(&du_im, &v.im), -1.0),
            ),
            im: sum(&product(&du_re, &v.im), &product(&du_im, &v.re)),
        }
    }

    /// The matrix of `C(u, u, ·)`, the derivative of `D(u)` along `u` by
    /// the five-point stencil, whose error is `O(h⁴)`: the three-point one
    /// moved the predator–prey Chenciner point by `6e-7` in `ε`.
    fn c(&self, u: &[f64]) -> Vec<f64> {
        let len = norm2(u);
        if len == 0.0 {
            return vec![0.0; self.a.len()];
        }
        let h = norm_inf(self.x).max(1.0) / 131_072.0 / len;
        let at = |steps: f64| {
            let x: Vec<f64> = self
                .x
                .iter()
                .zip(u)
                .map(|(x, u)| x + steps * h * u)
                .collect();
            self.derivative(&x, u)
        };
        let (p1, m1, p2, m2) = (at(1.0), at(-1.0), at(2.0), at(-2.0));
        (0..p1.len())
            .map(|i| (8.0 * (p1[i] - m1[i]) - (p2[i] - m2[i])) / (12.0 * h))
            .collect()
    }

    /// `I − A`.
    fn identity_minus_a(&self) -> Vec<f64> {
        sum(&identity(self.dim()), &scaled(&self.a, -1.0))
    }
}

/// The fold's quadratic coefficient `a = ½⟨p, B(q, q)⟩` with `‖q‖ = 1` and
/// `⟨p, q⟩ = 1`, from null vectors `q` of `M` and `w` of `Mᵀ`, where
/// `M = A` for a flow and `A − I` for a map. It vanishes at a cusp, flips
/// sign with `q`, and has a pole where `wᵀq = 0`, at a Bogdanov–Takens
/// point or a 1:1 resonance.
pub(crate) fn fold(t: &Taylor, q: &[f64], w: &[f64]) -> Option<f64> {
    let (q, p) = adjoint_pair(q, w);
    finite(0.5 * dot(&p, &t.b(&q, &q)))
}

/// The flip's cubic coefficient
/// `c = ⅙⟨p, C(q, q, q) + 3B(q, (I − A)⁻¹B(q, q))⟩` with `‖q‖ = 1` and
/// `⟨p, q⟩ = 1`, from null vectors `q` of `A + I` and `w` of its
/// transpose. It vanishes at a generalized flip and does not depend on the
/// orientation of `q`. For a scalar map it is `f″²/4 + f‴/6`.
pub(crate) fn flip(t: &Taylor, q: &[f64], w: &[f64]) -> Option<f64> {
    let n = t.dim();
    let (q, p) = adjoint_pair(q, w);
    let h = lu_solve(&t.identity_minus_a(), &t.b(&q, &q))?;
    let terms = sum(&matvec(&t.c(&q), &q, n), &scaled(&t.b(&q, &h), 3.0));
    finite(dot(&p, &terms) / 6.0)
}

/// The Neimark–Sacker coefficient `d = Re(e^{−iθ} c₁)` at the multiplier
/// `μ = e^{iθ} = eigenvalues[k]`, `Im μ > 0`, with
/// `c₁ = ½⟨p, C(q, q, q̄) + 2B(q, (I − A)⁻¹B(q, q̄)) + B(q̄, (μ²I − A)⁻¹B(q, q))⟩`,
/// `A q = μq`, `Aᵀp = μ̄p` and `⟨q, q⟩ = ⟨p, q⟩ = 1`, the scaling of
/// MatContM. It vanishes at a Chenciner point. It has poles at the 1:1 and
/// 1:3 resonances, where `I − A` or `μ²I − A` is singular, and at 1:2.
pub(crate) fn neimark_sacker(t: &Taylor, eigenvalues: &[(f64, f64)], k: usize) -> Option<f64> {
    let n = t.dim();
    let mu = eigenvalues[k];
    let (q, w) = eigenpair(SystemKind::Map, &t.a, eigenvalues, k)?;
    let q = q.scale(1.0 / q.dot_conj(&q).0.sqrt());
    let w = w.div(w.dot(&q));
    let h11 = lu_solve(&t.identity_minus_a(), &t.b_complex(&q, &q.conj()).re)?;
    let mu2 = (mu.0 * mu.0 - mu.1 * mu.1, 2.0 * mu.0 * mu.1);
    let shifted = sum(&scaled(&identity(n), mu2.0), &scaled(&t.a, -1.0));
    let h20 = complex_solve(&shifted, mu2.1, &t.b_complex(&q, &q))?;
    let e = sum(&t.c(&q.re), &t.c(&q.im));
    let cqqq = CVector::new(matvec(&e, &q.re, n), matvec(&e, &q.im, n));
    let (b1, b2) = (
        t.b_complex(&q, &CVector::real(h11)),
        t.b_complex(&q.conj(), &h20),
    );
    let terms = CVector::new(
        (0..n)
            .map(|i| cqqq.re[i] + 2.0 * b1.re[i] + b2.re[i])
            .collect(),
        (0..n)
            .map(|i| cqqq.im[i] + 2.0 * b1.im[i] + b2.im[i])
            .collect(),
    );
    let c1 = w.dot(&terms);
    finite(0.5 * (mu.0 * c1.0 + mu.1 * c1.1))
}

/// The first Lyapunov coefficient at a Hopf point with critical eigenvalue
/// `λ = (re, ω)` of `power` at `x` and `params`, by Kuznetsov's formula
/// (Elements of Applied Bifurcation Theory, 3rd ed., §3.5, eq. 3.20):
///
/// ```text
/// ℓ₁ = Re[⟨p, C(q,q,q̄)⟩ − 2⟨p, B(q, A⁻¹B(q,q̄))⟩ + ⟨p, B(q̄, (2iωI − A)⁻¹B(q,q))⟩] / 2ω
/// ```
///
/// with `A q = iω q`, `Aᵀ p = −iω p`, `⟨p, q⟩ = 1`, `⟨q, q⟩ = 1/2`. `B` and
/// `C` are central differences of the exact Jacobian; the complex solve goes
/// through the real `2n` system.
pub(crate) fn first_lyapunov(
    power: &Power,
    params: &[f64],
    x: &[f64],
    eigenvalues: &[(f64, f64)],
    lambda: (f64, f64),
) -> Option<f64> {
    let n = x.len();
    let omega = lambda.1;
    let jac = |x: &[f64]| {
        let mut j = vec![0.0; n * n];
        power.jacobian(0.0, x, params, &mut j);
        j
    };
    let a = jac(x);
    let k = eigenvalues.iter().position(|&l| l == lambda)?;
    let (q, w) = eigenpair(SystemKind::Ode, &a, eigenvalues, k)?;
    let q = q.scale(1.0 / (2.0 * q.dot_conj(&q).0).sqrt());
    let w = w.div(w.dot(&q));

    let scale = 1.0 + norm_inf(x);
    let around = |c: &[f64], eps: f64| {
        let len = norm2(c);
        let at = |sign: f64| {
            let x: Vec<f64> = (0..n).map(|i| x[i] + sign * eps * c[i] / len).collect();
            jac(&x)
        };
        (at(1.0), at(-1.0), len)
    };
    let b_real = |u: &[f64], c: &[f64]| -> Vec<f64> {
        if c.iter().all(|v| *v == 0.0) {
            return vec![0.0; n];
        }
        let eps = 1e-5 * scale;
        let (jp, jm, len) = around(c, eps);
        let d: Vec<f64> = (0..n * n)
            .map(|i| (jp[i] - jm[i]) * len / (2.0 * eps))
            .collect();
        matvec(&d, u, n)
    };
    let b = |u: &CVector, v: &CVector| -> CVector {
        let rr = b_real(&u.re, &v.re);
        let ii = b_real(&u.im, &v.im);
        let ri = b_real(&u.re, &v.im);
        let ir = b_real(&u.im, &v.re);
        CVector {
            re: (0..n).map(|i| rr[i] - ii[i]).collect(),
            im: (0..n).map(|i| ri[i] + ir[i]).collect(),
        }
    };
    let d2j = |c: &[f64]| -> Vec<f64> {
        if c.iter().all(|v| *v == 0.0) {
            return vec![0.0; n * n];
        }
        let eps = 1e-4 * scale;
        let (jp, jm, len) = around(c, eps);
        (0..n * n)
            .map(|i| (jp[i] - 2.0 * a[i] + jm[i]) * len * len / (eps * eps))
            .collect()
    };
    let m: Vec<f64> = d2j(&q.re)
        .iter()
        .zip(d2j(&q.im))
        .map(|(u, v)| u + v)
        .collect();
    let cqqq = CVector {
        re: matvec(&m, &q.re, n),
        im: matvec(&m, &q.im, n),
    };
    let h11 = lu_solve(&a, &b(&q, &q.conj()).re)?;
    let minus_a: Vec<f64> = a.iter().map(|v| -v).collect();
    let h20 = complex_solve(&minus_a, 2.0 * omega, &b(&q, &q))?;
    let total =
        w.dot(&cqqq).0 - 2.0 * w.dot(&b(&q, &CVector::real(h11))).0 + w.dot(&b(&q.conj(), &h20)).0;
    let l1 = total / (2.0 * omega);
    l1.is_finite().then_some(l1)
}

/// The complex eigenvector `q` of `a` for `eigenvalues[k]` and the one of
/// `aᵀ` for the same eigenvalue, which is `p̄`.
fn eigenpair(
    kind: SystemKind,
    a: &[f64],
    eigenvalues: &[(f64, f64)],
    k: usize,
) -> Option<(CVector, CVector)> {
    let n = eigenvalues.len();
    let vector = |m: &[f64]| {
        let v = eigenvectors(kind, n, m, eigenvalues);
        Some(CVector::new(v.re[k].clone()?, v.im[k].clone()?))
    };
    let transpose: Vec<f64> = (0..n * n).map(|i| a[i % n * n + i / n]).collect();
    Some((vector(a)?, vector(&transpose)?))
}

/// `q / ‖q‖` and `w` scaled so that `⟨w, q⟩ = 1`.
fn adjoint_pair(q: &[f64], w: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let q = scaled(q, 1.0 / norm2(q));
    let along = dot(w, &q);
    (q, scaled(w, 1.0 / along))
}

fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

fn sum(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(u, v)| u + v).collect()
}

fn scaled(a: &[f64], s: f64) -> Vec<f64> {
    a.iter().map(|v| v * s).collect()
}

#[derive(Clone)]
struct CVector {
    re: Vec<f64>,
    im: Vec<f64>,
}

impl CVector {
    fn new(re: Vec<f64>, im: Vec<f64>) -> Self {
        Self { re, im }
    }
    fn real(re: Vec<f64>) -> Self {
        let im = vec![0.0; re.len()];
        Self { re, im }
    }
    fn conj(&self) -> Self {
        Self::new(self.re.clone(), self.im.iter().map(|v| -v).collect())
    }
    fn scale(&self, s: f64) -> Self {
        Self::new(
            self.re.iter().map(|v| v * s).collect(),
            self.im.iter().map(|v| v * s).collect(),
        )
    }
    fn div(&self, (re, im): (f64, f64)) -> Self {
        let d = re * re + im * im;
        let (cr, ci) = (re / d, -im / d);
        Self::new(
            (0..self.re.len())
                .map(|i| self.re[i] * cr - self.im[i] * ci)
                .collect(),
            (0..self.re.len())
                .map(|i| self.re[i] * ci + self.im[i] * cr)
                .collect(),
        )
    }
    fn dot(&self, v: &CVector) -> (f64, f64) {
        (0..self.re.len()).fold((0.0, 0.0), |(r, i), k| {
            (
                r + self.re[k] * v.re[k] - self.im[k] * v.im[k],
                i + self.re[k] * v.im[k] + self.im[k] * v.re[k],
            )
        })
    }
    fn dot_conj(&self, v: &CVector) -> (f64, f64) {
        self.conj().dot(v)
    }
}

/// `z` with `(m + i·s·I) z = r`, through the real `2n` system.
fn complex_solve(m: &[f64], s: f64, r: &CVector) -> Option<CVector> {
    let n = r.re.len();
    let big: Vec<f64> = (0..4 * n * n)
        .map(|k| {
            let (row, col) = (k / (2 * n), k % (2 * n));
            let (bi, bj) = (row / n, col / n);
            let (i, j) = (row % n, col % n);
            let eye = if i == j { s } else { 0.0 };
            match (bi, bj) {
                (0, 0) | (1, 1) => m[i * n + j],
                (0, 1) => -eye,
                _ => eye,
            }
        })
        .collect();
    let rhs: Vec<f64> = r.re.iter().chain(&r.im).copied().collect();
    let z = lu_solve(&big, &rhs)?;
    Some(CVector::new(z[..n].to_vec(), z[n..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linalg::eigenvalues;
    use dyn_expr::{compile, CompiledSystem, InputFormat};

    fn system(source: &str) -> CompiledSystem {
        compile(source, InputFormat::Text)
            .system
            .unwrap_or_else(|| panic!("{source} compiles"))
    }

    fn assert_near(got: Option<f64>, want: f64, tol: f64) {
        let got = got.expect("a coefficient");
        assert!((got - want).abs() <= tol, "{got} vs {want}");
    }

    /// `ẋ = r + x² + y, ẏ = −y + x²` at `r = 0`: the centre manifold is
    /// `y = x² + …`, on which `ẋ = r + 2x²`, so `a = 2`.
    #[test]
    fn fold_coefficient_reads_the_left_null_vector() {
        let sys = system("x' = r + x^2 + y\ny' = -y + x^2");
        let power = Power { f: &sys, k: 1 };
        let (x, p) = ([0.0, 0.0], [0.0]);
        let t = Taylor::new(&power, &x, &p);
        assert_near(fold(&t, &[1.0, 0.0], &[1.0, 1.0]), 2.0, 1e-12);
        assert_near(fold(&t, &[3.0, 0.0], &[2.0, 2.0]), 2.0, 1e-12);
        assert_near(fold(&t, &[-1.0, 0.0], &[1.0, 1.0]), -2.0, 1e-12);
    }

    /// The logistic map at `r = 3`, `x = 2/3`: `f″ = −6`, `f‴ = 0`, so
    /// `c = f″²/4 + f‴/6 = 9`, whichever way `q` points.
    #[test]
    fn flip_coefficient_of_the_logistic_map_at_3() {
        let sys = system("x_{n+1} = r*x_n*(1 - x_n)");
        let power = Power { f: &sys, k: 1 };
        let (x, p) = ([2.0 / 3.0], [3.0]);
        let t = Taylor::new(&power, &x, &p);
        assert_near(flip(&t, &[1.0], &[1.0]), 9.0, 1e-9);
        assert_near(flip(&t, &[-2.0], &[0.5]), 9.0, 1e-9);
    }

    /// `x ↦ −x − x³ + m` at `m = 0`: `f″ = 0` and `f‴ = −6`, so
    /// `c = f‴/6 = −1`, all from the difference of `jacobian_derivative`.
    #[test]
    fn flip_coefficient_from_the_third_derivative() {
        let sys = system("x_{n+1} = -x_n - x_n^3 + m");
        let power = Power { f: &sys, k: 1 };
        let (x, p) = ([0.0], [0.0]);
        let t = Taylor::new(&power, &x, &p);
        assert_near(flip(&t, &[1.0], &[1.0]), -1.0, 1e-9);
    }

    fn ns_coefficient(sys: &CompiledSystem, x: &[f64], p: &[f64]) -> Option<f64> {
        let power = Power { f: sys, k: 1 };
        let t = Taylor::new(&power, x, p);
        let ev = eigenvalues(x.len(), &t.a);
        let k = ev.iter().position(|l| l.1 > 0.0).expect("a complex pair");
        neimark_sacker(&t, &ev, k)
    }

    /// MatContM Tutorial M-I: the delayed logistic map at `r = 2` has a
    /// Neimark–Sacker point with `θ = 60°` and coefficient `−1.0`, which
    /// is `−2 Re μ` by hand.
    #[test]
    fn neimark_sacker_coefficient_of_the_delayed_logistic_map_at_2() {
        let sys = system("x_{n+1} = r*x_n*(1 - y_n)\ny_{n+1} = x_n");
        assert_near(ns_coefficient(&sys, &[0.5, 0.5], &[2.0]), -1.0, 1e-9);
    }

    /// The predator–prey map of docs/codim2-reference.md at `ε = 1`,
    /// `a = 5`, where MatContM's routine gives `d = −11.2197`.
    #[test]
    fn neimark_sacker_coefficient_of_the_predator_prey_map() {
        let sys = system(
            "x_{n+1} = a*x_n*(1 - x_n) - 3*x_n*y_n/(1 + epsilon*x_n)\n\
             y_{n+1} = 3.5*x_n*y_n/(1 + epsilon*x_n)",
        );
        let mut p = [0.0; 2];
        let index = |name: &str| sys.info.params.iter().position(|q| q.name == name);
        p[index("a").expect("a")] = 5.0;
        p[index("epsilon").expect("ε")] = 1.0;
        let x = [0.4, 3.5 * (5.0 * 0.6 - 1.0) / 7.5];
        assert_near(ns_coefficient(&sys, &x, &p), -11.2197, 5e-5);
    }

    /// `ẋ = μx − y + c·x(x² + y²), ẏ = x + μy + c·y(x² + y²)` at `μ = 0`:
    /// `ℓ₁ = c` with `ω = 1`, through `0`.
    #[test]
    fn first_lyapunov_coefficient_follows_the_cubic_term() {
        let sys = system("x' = mu*x - y + c*x*(x^2 + y^2)\ny' = x + mu*y + c*y*(x^2 + y^2)");
        let power = Power { f: &sys, k: 1 };
        let index = |name: &str| sys.info.params.iter().position(|q| q.name == name);
        let (ic, imu) = (index("c").expect("c"), index("mu").expect("μ"));
        for c in [0.3, -0.7] {
            let mut p = [0.0; 2];
            (p[ic], p[imu]) = (c, 0.0);
            let x = [0.0, 0.0];
            let mut a = [0.0; 4];
            power.jacobian(0.0, &x, &p, &mut a);
            let ev = eigenvalues(2, &a);
            let lambda = *ev.iter().find(|l| l.1 > 0.0).expect("iω");
            assert_near(first_lyapunov(&power, &p, &x, &ev, lambda), c, 1e-6);
        }
    }
}
