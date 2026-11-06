//! Pseudo-arclength continuation of equilibria of flows and of fixed points
//! of `f^k` for maps.
//!
//! Modelled on the natural and pseudo-arclength continuation of
//! BifurcationKit.jl (R. Veltz, MIT license,
//! <https://github.com/bifurcationkit/BifurcationKit.jl>). Everything reads
//! one residual `g(x, p)` and its Jacobian `[g_x | g_p]`: `g = f` for a flow
//! and `g = f^k − x` for a map. A step predicts along the unit tangent (the
//! first step) or the secant (later steps) and corrects by Newton's method on
//! the bordered system
//!
//! ```text
//! [ g_x  g_p ] Δz = − [ g(z)              ]
//! [    τᵀ    ]        [ τ · (z − z_pred)  ]
//! ```
//!
//! with the exact Jacobians of the field ([`VectorField::jacobian`] and
//! [`VectorField::param_jacobian`]).

use std::f64::consts::PI;

use dyn_core::{SystemKind, VectorField};

use crate::linalg::{eigenvalues, lu_solve};

/// Newton iterations allowed per correction.
const MAX_NEWTON: usize = 10;
/// Newton converges when its last step and the residual are both below
/// `TOL · (1 + |z|∞)`.
const TOL: f64 = 1e-10;
/// A correction in this many iterations or fewer grows the next step.
const FAST: usize = 3;
/// A correction in more iterations than this halves the next step.
const SLOW: usize = 6;
const GROW: f64 = 1.5;

/// How to follow a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuationOptions {
    /// Index of the continuation parameter.
    pub param: usize,
    /// `k` for fixed points of `f^k` of a map; ignored (taken as 1) for flows.
    pub period: usize,
    /// The parameter interval the branch stays in.
    pub range: (f64, f64),
    /// Signed first step in arclength: its sign is the direction in `p`.
    pub ds: f64,
    /// Smallest step; a step halved below it ends the branch.
    pub ds_min: f64,
    /// Largest step.
    pub ds_max: f64,
    /// Most points on the branch, the start included.
    pub max_points: usize,
    /// Largest angle in radians between successive unit tangents.
    pub max_angle: f64,
}

/// One point of a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchPoint {
    /// Arclength from the start in `(x, p)`.
    pub s: f64,
    /// The parameter.
    pub p: f64,
    /// The state, a zero of `g`.
    pub x: Vec<f64>,
    /// `x, f(x), …, f^{k−1}(x)` (`period · dim` values) for a map; `x` for
    /// a flow.
    pub cycle: Vec<f64>,
    /// Unit tangent `(ẋ, ṗ)` of the branch, `dim + 1` values, oriented in
    /// the direction of continuation.
    pub tangent: Vec<f64>,
    /// Eigenvalues `(re, im)` of `J` (flow) or `D f^k` (map).
    pub eigenvalues: Vec<(f64, f64)>,
    /// Eigenvalues with `Re λ > 0` (flow) or `|μ| > 1` (map).
    pub unstable: usize,
    /// Every eigenvalue has `Re λ < 0` (flow) or `|μ| < 1` (map).
    pub stable: bool,
}

/// Why a branch stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchEnd {
    /// It reached an edge of the parameter range; the last point lies on it.
    Range,
    /// It has `max_points` points.
    MaxPoints,
    /// A step failed with the step at `ds_min`.
    StepTooSmall,
}

/// A branch of equilibria or of fixed points of `f^period`.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    /// `k` of `f^k` for a map, 1 for a flow.
    pub period: usize,
    /// The points in order of arclength, the corrected start first.
    pub points: Vec<BranchPoint>,
    /// Why the branch stopped.
    pub end: BranchEnd,
}

/// Why no branch could start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationError {
    /// Newton's method from the start did not converge to a zero of `g`.
    NotAFixedPoint,
    /// `g_x` or the bordered Jacobian is singular at the start (a fold or a
    /// branch point), so the branch has no unique direction.
    Singular,
    /// The field depends explicitly on `t` (or `n`).
    NotAutonomous,
}

/// Follow the branch through `x0` at `params` as `params[opts.param]`
/// varies.
///
/// `x0` is first corrected by Newton's method at fixed `p`. If `p` already
/// lies outside `opts.range` the branch is that point alone, ending with
/// [`BranchEnd::Range`].
pub fn continue_branch(
    f: &dyn VectorField,
    params: &[f64],
    x0: &[f64],
    opts: &ContinuationOptions,
) -> Result<Branch, ContinuationError> {
    if !f.is_autonomous() {
        return Err(ContinuationError::NotAutonomous);
    }
    let map = f.kind() == SystemKind::Map;
    let period = if map { opts.period.max(1) } else { 1 };
    let g = Residual {
        power: Power { f, k: period },
        map,
        params: params.to_vec(),
        param: opts.param,
    };
    let n = f.dim();
    let (lo, hi) = (
        opts.range.0.min(opts.range.1),
        opts.range.0.max(opts.range.1),
    );

    let p0 = params[opts.param];
    let (mut z, _) = g.correct_at_fixed_p(x0, p0).map_err(|e| match e {
        Fail::Singular => ContinuationError::Singular,
        Fail::NoConvergence => ContinuationError::NotAFixedPoint,
    })?;
    let mut seed = vec![0.0; n + 1];
    seed[n] = if opts.ds < 0.0 { -1.0 } else { 1.0 };
    let mut tau = g.tangent(&z, &seed).ok_or(ContinuationError::Singular)?;
    let mut points = vec![g.point(0.0, &z, tau.clone())];
    if !(lo..=hi).contains(&p0) {
        return Ok(Branch {
            period,
            points,
            end: BranchEnd::Range,
        });
    }

    let cos_max = cos(opts.max_angle);
    let mut h = opts.ds.abs().min(opts.ds_max).max(opts.ds_min);
    let mut dir = tau.clone();
    let mut s = 0.0;
    let end = loop {
        if points.len() >= opts.max_points {
            break BranchEnd::MaxPoints;
        }
        if h < opts.ds_min {
            break BranchEnd::StepTooSmall;
        }
        let pred: Vec<f64> = z.iter().zip(&dir).map(|(a, d)| a + h * d).collect();
        let edge = if pred[n] > hi {
            Some(hi)
        } else if pred[n] < lo {
            Some(lo)
        } else {
            None
        };
        let corrected = match edge {
            Some(e) if z[n] == e => break BranchEnd::Range,
            Some(e) => {
                let along = (e - z[n]) / dir[n];
                let guess: Vec<f64> = (0..n).map(|i| z[i] + along * dir[i]).collect();
                g.correct_at_fixed_p(&guess, e)
            }
            None => g.correct_along(&pred, &dir),
        };
        let accepted = corrected.ok().and_then(|(z_new, iters)| {
            let t = g.tangent(&z_new, &tau)?;
            (dot(&t, &tau) >= cos_max).then_some((z_new, iters, t))
        });
        let Some((z_new, iters, t_new)) = accepted else {
            h /= 2.0;
            continue;
        };
        let chord: Vec<f64> = z_new.iter().zip(&z).map(|(a, b)| a - b).collect();
        let len = norm2(&chord);
        s += len;
        points.push(g.point(s, &z_new, t_new.clone()));
        if edge.is_some() {
            break BranchEnd::Range;
        }
        dir = chord.iter().map(|c| c / len).collect();
        z = z_new;
        tau = t_new;
        if iters <= FAST {
            h = (h * GROW).min(opts.ds_max);
        } else if iters > SLOW {
            h = (h / 2.0).max(opts.ds_min);
        }
    };
    Ok(Branch {
        period,
        points,
        end,
    })
}

/// `f^k` as a field. For `k = 1` it is `f` itself, so flows and maps share
/// one residual.
pub(crate) struct Power<'a> {
    pub(crate) f: &'a dyn VectorField,
    pub(crate) k: usize,
}

impl Power<'_> {
    /// Iterate `f` `k` times from `x`, calling `visit` on each `x_j`
    /// (`j = 0..k`) before it is mapped. Returns `x_k`.
    fn walk(&self, x: &[f64], p: &[f64], mut visit: impl FnMut(&[f64])) -> Vec<f64> {
        let mut xj = x.to_vec();
        let mut next = vec![0.0; x.len()];
        for _ in 0..self.k {
            visit(&xj);
            self.f.eval(0.0, &xj, p, &mut next);
            std::mem::swap(&mut xj, &mut next);
        }
        xj
    }

    /// `x, f(x), …, f^{k−1}(x)`, `k · dim` values.
    pub(crate) fn cycle(&self, x: &[f64], p: &[f64]) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.k * x.len());
        self.walk(x, p, |xj| out.extend_from_slice(xj));
        out
    }
}

impl VectorField for Power<'_> {
    fn dim(&self) -> usize {
        self.f.dim()
    }
    fn n_params(&self) -> usize {
        self.f.n_params()
    }
    fn kind(&self) -> SystemKind {
        self.f.kind()
    }
    fn eval(&self, _t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        out.copy_from_slice(&self.walk(x, p, |_| {}));
    }

    /// `D f^k(x) = J(x_{k−1}) ⋯ J(x_0)`.
    fn jacobian(&self, _t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        let n = x.len();
        let mut m = identity(n);
        let mut j = vec![0.0; n * n];
        self.walk(x, p, |xj| {
            self.f.jacobian(0.0, xj, p, &mut j);
            m = matmul(&j, &m, n);
        });
        out.copy_from_slice(&m);
    }

    /// `∂f^j/∂p = J(x_{j−1}) · ∂f^{j−1}/∂p + f_p(x_{j−1})`.
    fn param_jacobian(&self, _t: f64, x: &[f64], p: &[f64], k: usize, out: &mut [f64]) {
        let n = x.len();
        let mut v = vec![0.0; n];
        let mut j = vec![0.0; n * n];
        let mut fp = vec![0.0; n];
        self.walk(x, p, |xj| {
            self.f.jacobian(0.0, xj, p, &mut j);
            self.f.param_jacobian(0.0, xj, p, k, &mut fp);
            v = (0..n)
                .map(|r| fp[r] + (0..n).map(|c| j[r * n + c] * v[c]).sum::<f64>())
                .collect();
        });
        out.copy_from_slice(&v);
    }
}

/// `g(x, p) = f^k(x) − x` (map) or `f(x)` (flow) in `z = (x, p)`.
struct Residual<'a> {
    power: Power<'a>,
    map: bool,
    params: Vec<f64>,
    param: usize,
}

enum Fail {
    Singular,
    NoConvergence,
}

impl Residual<'_> {
    fn params_at(&self, p: f64) -> Vec<f64> {
        let mut params = self.params.clone();
        params[self.param] = p;
        params
    }

    /// `g(z)` and `[g_x | g_p]`, `n × (n + 1)` row-major.
    fn linear(&self, z: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = z.len() - 1;
        let (x, p) = (&z[..n], self.params_at(z[n]));
        let mut g = vec![0.0; n];
        let mut gx = vec![0.0; n * n];
        let mut gp = vec![0.0; n];
        self.power.eval(0.0, x, &p, &mut g);
        self.power.jacobian(0.0, x, &p, &mut gx);
        self.power.param_jacobian(0.0, x, &p, self.param, &mut gp);
        if self.map {
            for i in 0..n {
                g[i] -= x[i];
                gx[i * n + i] -= 1.0;
            }
        }
        let a = (0..n)
            .flat_map(|i| gx[i * n..(i + 1) * n].iter().copied().chain([gp[i]]))
            .collect();
        (g, a)
    }

    /// Newton on `g(x, p) = 0` with `p` held exactly.
    fn correct_at_fixed_p(&self, x: &[f64], p: f64) -> Result<(Vec<f64>, usize), Fail> {
        let n = x.len();
        let (x, iters) = newton(x.to_vec(), |x| {
            let z: Vec<f64> = x.iter().copied().chain([p]).collect();
            let (g, a) = self.linear(&z);
            let gx = (0..n).flat_map(|i| a[i * (n + 1)..i * (n + 1) + n].to_vec());
            (g, gx.collect())
        })?;
        Ok((x.into_iter().chain([p]).collect(), iters))
    }

    /// Newton on `g(z) = 0, dir · (z − pred) = 0` from `pred`.
    fn correct_along(&self, pred: &[f64], dir: &[f64]) -> Result<(Vec<f64>, usize), Fail> {
        newton(pred.to_vec(), |z| {
            let (mut g, a) = self.linear(z);
            g.push(
                z.iter()
                    .zip(pred)
                    .zip(dir)
                    .map(|((a, b), d)| d * (a - b))
                    .sum(),
            );
            (g, bordered(a, dir))
        })
    }

    /// The unit kernel vector `τ` of `[g_x | g_p]` at `z` with
    /// `border · τ > 0`, or `None` where the bordered Jacobian is singular.
    fn tangent(&self, z: &[f64], border: &[f64]) -> Option<Vec<f64>> {
        let (_, a) = self.linear(z);
        let mut rhs = vec![0.0; z.len()];
        rhs[z.len() - 1] = 1.0;
        let t = lu_solve(&bordered(a, border), &rhs)?;
        let len = norm2(&t);
        len.is_finite().then(|| t.iter().map(|v| v / len).collect())
    }

    fn point(&self, s: f64, z: &[f64], tangent: Vec<f64>) -> BranchPoint {
        let n = z.len() - 1;
        let (x, p) = (&z[..n], self.params_at(z[n]));
        let mut jac = vec![0.0; n * n];
        self.power.jacobian(0.0, x, &p, &mut jac);
        let eigenvalues = eigenvalues(n, &jac);
        let growth: Vec<f64> = if self.map {
            eigenvalues
                .iter()
                .map(|(re, im)| (re * re + im * im).sqrt() - 1.0)
                .collect()
        } else {
            eigenvalues.iter().map(|(re, _)| *re).collect()
        };
        BranchPoint {
            s,
            p: z[n],
            x: x.to_vec(),
            cycle: if self.map {
                self.power.cycle(x, &p)
            } else {
                x.to_vec()
            },
            tangent,
            unstable: growth.iter().filter(|g| **g > 0.0).count(),
            stable: !growth.is_empty() && growth.iter().all(|g| *g < 0.0),
            eigenvalues,
        }
    }
}

/// Newton's method on `system(z) = (residual, Jacobian)`. Returns the zero
/// and the number of linear solves.
fn newton(
    mut z: Vec<f64>,
    mut system: impl FnMut(&[f64]) -> (Vec<f64>, Vec<f64>),
) -> Result<(Vec<f64>, usize), Fail> {
    let mut step = f64::INFINITY;
    for iters in 0..=MAX_NEWTON {
        let (r, jac) = system(&z);
        if !r.iter().all(|v| v.is_finite()) {
            return Err(Fail::NoConvergence);
        }
        let tol = TOL * (1.0 + norm_inf(&z));
        if step <= tol && norm_inf(&r) <= tol {
            return Ok((z, iters));
        }
        if iters == MAX_NEWTON {
            break;
        }
        let rhs: Vec<f64> = r.iter().map(|v| -v).collect();
        let dz = lu_solve(&jac, &rhs).ok_or(Fail::Singular)?;
        step = norm_inf(&dz);
        for (zi, d) in z.iter_mut().zip(&dz) {
            *zi += d;
        }
        if !step.is_finite() {
            return Err(Fail::NoConvergence);
        }
    }
    Err(Fail::NoConvergence)
}

/// `[a; borderᵀ]`: `a` is `n × (n + 1)` row-major, the result square.
fn bordered(mut a: Vec<f64>, border: &[f64]) -> Vec<f64> {
    a.extend_from_slice(border);
    a
}

/// `cos θ` for `θ` clamped to `[0, π]` from its Taylor series. Only `+`, `×`
/// and `÷`, so the angle limit keeps the same points on every platform,
/// where a libm `cos` may differ in the last bit.
fn cos(theta: f64) -> f64 {
    let t2 = theta.clamp(0.0, PI).powi(2);
    let (mut term, mut sum, mut k) = (1.0_f64, 1.0, 0.0);
    while term.abs() > 1e-18 {
        k += 2.0;
        term *= -t2 / ((k - 1.0) * k);
        sum += term;
    }
    sum
}

fn identity(n: usize) -> Vec<f64> {
    (0..n * n)
        .map(|i| if i / n == i % n { 1.0 } else { 0.0 })
        .collect()
}

fn matmul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    (0..n * n)
        .map(|i| (0..n).map(|l| a[i / n * n + l] * b[l * n + i % n]).sum())
        .collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(u, v)| u * v).sum()
}

fn norm2(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

fn norm_inf(a: &[f64]) -> f64 {
    a.iter().fold(0.0, |m, v| m.max(v.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dyn_expr::{compile, InputFormat};

    #[test]
    fn power_of_the_logistic_map() {
        let sys = compile("x_{n+1} = r*x_n*(1 - x_n)", InputFormat::Text)
            .system
            .expect("compiles");
        let f2 = Power { f: &sys, k: 2 };
        let (r, x) = (3.3, 0.3);
        let fx = r * x * (1.0 - x);
        let mut out = [0.0];
        f2.eval(0.0, &[x], &[r], &mut out);
        assert!((out[0] - r * fx * (1.0 - fx)).abs() < 1e-12, "{out:?}");
        f2.jacobian(0.0, &[x], &[r], &mut out);
        let dx = r * (1.0 - 2.0 * fx) * r * (1.0 - 2.0 * x);
        assert!((out[0] - dx).abs() < 1e-12, "{out:?} vs {dx}");
        f2.param_jacobian(0.0, &[x], &[r], 0, &mut out);
        let dr = fx * (1.0 - fx) + r * (1.0 - 2.0 * fx) * x * (1.0 - x);
        assert!((out[0] - dr).abs() < 1e-12, "{out:?} vs {dr}");
        assert_eq!(f2.cycle(&[x], &[r]), [x, fx]);
    }

    #[test]
    fn cos_series_matches_known_values() {
        assert_eq!(cos(0.0), 1.0);
        assert!((cos(PI / 3.0) - 0.5).abs() < 1e-15);
        assert!((cos(PI) + 1.0).abs() < 1e-15);
        assert!((cos(4.0) + 1.0).abs() < 1e-15, "clamped to π");
    }
}
