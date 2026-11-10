//! Curves of folds, Hopf points, flips and Neimark–Sacker points in two
//! parameters, by pseudo-arclength continuation of a minimally extended
//! defining system (W. Govaerts, *Numerical Methods for Bifurcations of
//! Dynamical Equilibria*, SIAM 2000, the systems MatCont follows).

use dyn_core::{SystemKind, VectorField};

use crate::continuation::{
    correct_along, cos, diff, dist, dot, identity, matvec, newton, norm2, norm_inf, null_vector,
    tangent, Fail, Power, Underdetermined, FAST_NEWTON_ITERS, SLOW_NEWTON_ITERS, STEP_GROWTH, TOL,
};
use crate::linalg::{eigenvalues, lu_solve};

/// The bifurcation a curve follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurveKind {
    /// Folds: an eigenvalue `0` of a flow, a multiplier `+1` of a map.
    Fold,
    /// Hopf points of a flow: two eigenvalues with sum `0`.
    Hopf,
    /// Flips of a map: a multiplier `−1`.
    Flip,
    /// Neimark–Sacker points of a map: two multipliers with product `1`.
    NeimarkSacker,
}

/// Where a curve starts: a bifurcation of the kind followed, as a branch
/// finds it.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveSeed {
    /// The bifurcation followed.
    pub kind: CurveKind,
    /// The state; for a map, the first point of the cycle.
    pub x: Vec<f64>,
    /// `k` of `f^k` for a map; ignored (taken as 1) for a flow.
    pub period: usize,
}

/// How to follow a curve.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveOptions {
    /// The two free parameters: `p₁`, drawn across, then `p₂`, drawn up.
    pub free: (usize, usize),
    /// The box the curve stays in: the range of `p₁`, then that of `p₂`,
    /// each with two different ends, in either order.
    pub ranges: ((f64, f64), (f64, f64)),
    /// Signed first step in arclength in scaled coordinates (see
    /// [`continue_curve`]); its sign picks the direction.
    pub ds: f64,
    /// Smallest step; a step halved below it ends the curve.
    pub ds_min: f64,
    /// Largest step.
    pub ds_max: f64,
    /// Most points on the curve, the seed included.
    pub max_points: usize,
    /// Largest angle in radians between successive unit tangents.
    pub max_angle: f64,
}

/// What the eigenvalues at a point of a curve make of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Nature {
    /// A bifurcation of the curve's kind.
    Bifurcation {
        /// Hopf: the frequency `ω`. Neimark–Sacker: the angle `θ = arg μ`
        /// of the critical multiplier with `Im μ ≥ 0`. `None` on a fold or
        /// flip curve.
        omega: Option<f64>,
    },
    /// A neutral saddle, where the defining system holds without a
    /// bifurcation: real eigenvalues `±κ` on a Hopf curve, real multipliers
    /// `μ` and `1/μ` on a Neimark–Sacker curve.
    Neutral,
}

/// One point of a curve.
#[derive(Debug, Clone, PartialEq)]
pub struct CurvePoint {
    /// Arclength from the seed in scaled coordinates `(x, u₁, u₂)`.
    pub s: f64,
    /// The parameters `(p₁, p₂)`.
    pub p: (f64, f64),
    /// The state; for a map, the first point of the cycle.
    pub x: Vec<f64>,
    /// Unit tangent `(ẋ, u̇₁, u̇₂)` in scaled coordinates, `dim + 2` values,
    /// oriented in the direction of continuation.
    pub tangent: Vec<f64>,
    /// Eigenvalues `(re, im)` of `J` (flow) or `D f^k` (map).
    pub eigenvalues: Vec<(f64, f64)>,
    /// A bifurcation or a neutral saddle.
    pub nature: Nature,
}

/// Why a curve stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveEnd {
    /// It reached an edge of the box; the last point lies on it.
    Range,
    /// It has `max_points` points.
    MaxPoints,
    /// A step failed with the step at `ds_min`.
    StepTooSmall,
    /// It came back to the seed; the last point repeats the first.
    Closed,
}

/// A curve of bifurcations in two parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    /// The bifurcation followed.
    pub kind: CurveKind,
    /// `k` of `f^k` for a map, 1 for a flow.
    pub period: usize,
    /// The points in order of arclength, the corrected seed first.
    pub points: Vec<CurvePoint>,
    /// Why the curve stopped.
    pub end: CurveEnd,
}

/// Why no curve could start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveError {
    /// The field depends explicitly on `t` (or `n`).
    NotAutonomous,
    /// No such curve: a Hopf curve of a map, a flip or Neimark–Sacker
    /// curve of a flow, a Hopf or Neimark–Sacker curve in one dimension,
    /// or the same parameter freed twice.
    Unsupported,
    /// Newton's method from the seed with `p₂` held did not converge to a
    /// point of the curve.
    NotABifurcation,
    /// The defining system is singular at the seed, so the curve has no
    /// unique direction.
    Singular,
    /// `p₂` does not move the bifurcation: neither `g` nor `ψ` depends on
    /// it at the seed.
    NoEffect,
}

/// Follow the curve of `seed.kind` bifurcations through `seed` as
/// `params[opts.free.0]` and `params[opts.free.1]` vary.
///
/// The unknown is `z = (x, u₁, u₂)`, with `u = (p − lo)/(hi − lo)` on each
/// side of the box, so that a step is a fraction of the box whatever the
/// parameters' scales. The equations are `g(x, p) = 0` (`f` for a flow,
/// `f^k − x` for a map) and `ψ(x, p) = 0`, where `ψ` is the `s` of
/// `[M w₀; v₀ᵀ 0][q; s] = [0; 1]` and `M` is singular on the curve: `g_x`
/// for a fold, `D f^k + I` for a flip, `2A ⊙ I` for Hopf and `A ⊙ A − I`
/// for Neimark–Sacker, bialternate products of `A = J` or `D f^k`.
///
/// The seed is first corrected with `p₂` held. `ds > 0` then follows the
/// curve the way `p₁` increases at the seed, or the way `p₂` increases
/// where the tangent has no `u₁` component; `ds < 0` goes the other way. A
/// corrected seed outside the box is the curve alone, ending with
/// [`CurveEnd::Range`].
///
/// [`CurveError::NoEffect`] needs `g_{p₂}` and `ψ_{p₂}` to vanish at the
/// seed. They count as zero when the first-order move of `(x, u₁)` they
/// cause over the whole height of the box, `R⁻¹(g_{u₂}; ψ_{u₂})` with `R`
/// the Jacobian in `(x, u₁)`, is within Newton's tolerance
/// `1e-10 · (1 + |z|∞)`: the corrector could not tell the curve from the
/// vertical line through the seed. The move is exactly zero when `p₂` does
/// not enter the field, and unlike the derivatives themselves it does not
/// depend on how `g` and `ψ` are scaled.
pub fn continue_curve(
    f: &dyn VectorField,
    params: &[f64],
    seed: &CurveSeed,
    opts: &CurveOptions,
) -> Result<Curve, CurveError> {
    if !f.is_autonomous() {
        return Err(CurveError::NotAutonomous);
    }
    let n = f.dim();
    let map = f.kind() == SystemKind::Map;
    let supported = match seed.kind {
        CurveKind::Fold => true,
        CurveKind::Hopf => !map && n >= 2,
        CurveKind::Flip => map,
        CurveKind::NeimarkSacker => map && n >= 2,
    };
    if !supported || opts.free.0 == opts.free.1 {
        return Err(CurveError::Unsupported);
    }
    let mut sys = Defining::new(f, params, seed, opts);
    let z0 = sys.scaled(&seed.x);
    sys.first_borders(&z0).ok_or(CurveError::Singular)?;
    let (z, _) = correct_fixed(&sys, &z0, n + 1, z0[n + 1]).map_err(|e| match e {
        Fail::Singular => CurveError::Singular,
        Fail::NoConvergence => CurveError::NotABifurcation,
    })?;
    sys.renew_borders(&z);
    let mut up = vec![0.0; n + 2];
    up[n + 1] = 1.0;
    let tau = tangent(&sys, &z, &up).ok_or(CurveError::Singular)?;
    if norm_inf(&tau[..=n]) <= TOL * (1.0 + norm_inf(&z)) * tau[n + 1].abs() {
        return Err(CurveError::NoEffect);
    }
    let lead = if tau[n] != 0.0 { tau[n] } else { tau[n + 1] };
    let sign = if (lead < 0.0) == (opts.ds < 0.0) {
        1.0
    } else {
        -1.0
    };
    let tau: Vec<f64> = tau.iter().map(|t| sign * t).collect();
    let start = sys.point(0.0, &z, tau.clone());
    if !z[n..].iter().all(|u| (0.0..=1.0).contains(u)) {
        return Ok(sys.curve(vec![start], CurveEnd::Range));
    }
    let h = opts.ds.abs().min(opts.ds_max).max(opts.ds_min);
    Ok(sys.follow(start, z, tau, h, opts))
}

/// `G(z) = (g(x, p); ψ(x, p))`, `dim + 1` equations in `z = (x, u₁, u₂)`.
/// The borders `v₀`, `w₀` stay fixed while a point is corrected.
struct Defining<'a> {
    power: Power<'a>,
    map: bool,
    kind: CurveKind,
    params: Vec<f64>,
    free: [usize; 2],
    edges: [(f64, f64); 2],
    v0: Vec<f64>,
    w0: Vec<f64>,
}

impl<'a> Defining<'a> {
    fn new(f: &'a dyn VectorField, params: &[f64], seed: &CurveSeed, opts: &CurveOptions) -> Self {
        let map = f.kind() == SystemKind::Map;
        let sorted = |(a, b): (f64, f64)| (a.min(b), a.max(b));
        Defining {
            power: Power {
                f,
                k: if map { seed.period.max(1) } else { 1 },
            },
            map,
            kind: seed.kind,
            params: params.to_vec(),
            free: [opts.free.0, opts.free.1],
            edges: [sorted(opts.ranges.0), sorted(opts.ranges.1)],
            v0: Vec::new(),
            w0: Vec::new(),
        }
    }

    fn dim(&self) -> usize {
        self.power.f.dim()
    }

    /// Rows and columns of `M`.
    fn size(&self) -> usize {
        let n = self.dim();
        match self.kind {
            CurveKind::Fold | CurveKind::Flip => n,
            CurveKind::Hopf | CurveKind::NeimarkSacker => n * (n - 1) / 2,
        }
    }

    fn scaled(&self, x: &[f64]) -> Vec<f64> {
        let u = (0..2).map(|k| {
            let (lo, hi) = self.edges[k];
            (self.params[self.free[k]] - lo) / (hi - lo)
        });
        x.iter().copied().chain(u).collect()
    }

    /// `p = (1 − u)·lo + u·hi`, which is `lo` and `hi` exactly on the edges.
    fn params_at(&self, z: &[f64]) -> Vec<f64> {
        let n = self.dim();
        let mut p = self.params.clone();
        for k in 0..2 {
            let ((lo, hi), u) = (self.edges[k], z[n + k]);
            p[self.free[k]] = (1.0 - u) * lo + u * hi;
        }
        p
    }

    fn jacobian(&self, x: &[f64], p: &[f64]) -> Vec<f64> {
        let n = x.len();
        let mut a = vec![0.0; n * n];
        self.power.jacobian(0.0, x, p, &mut a);
        a
    }

    /// `M` from `A`.
    fn singular(&self, a: &[f64]) -> Vec<f64> {
        let n = self.dim();
        match self.kind {
            CurveKind::Fold if self.map => shifted(a, n, -1.0),
            CurveKind::Fold => a.to_vec(),
            CurveKind::Flip => shifted(a, n, 1.0),
            CurveKind::Hopf => {
                let eye = identity(n);
                sum(&cross(a, &eye, n), &cross(&eye, a, n))
            }
            CurveKind::NeimarkSacker => shifted(&cross(a, a, n), self.size(), -1.0),
        }
    }

    /// The derivative of `M` where `A` moves by `da`.
    fn singular_derivative(&self, a: &[f64], da: &[f64]) -> Vec<f64> {
        let n = self.dim();
        match self.kind {
            CurveKind::Fold | CurveKind::Flip => da.to_vec(),
            CurveKind::Hopf => {
                let eye = identity(n);
                sum(&cross(da, &eye, n), &cross(&eye, da, n))
            }
            CurveKind::NeimarkSacker => sum(&cross(da, a, n), &cross(a, da, n)),
        }
    }

    /// `(q, s, w)` from `[M w₀; v₀ᵀ 0][q; s] = [0; 1]` and
    /// `[Mᵀ v₀; w₀ᵀ 0][w; s] = [0; 1]`.
    fn bordered_solve(&self, m: &[f64]) -> Option<(Vec<f64>, f64, Vec<f64>)> {
        let size = self.v0.len();
        let k = size + 1;
        let mut b = vec![0.0; k * k];
        let mut bt = vec![0.0; k * k];
        for i in 0..size {
            for j in 0..size {
                b[i * k + j] = m[i * size + j];
                bt[i * k + j] = m[j * size + i];
            }
            b[i * k + size] = self.w0[i];
            b[size * k + i] = self.v0[i];
            bt[i * k + size] = self.v0[i];
            bt[size * k + i] = self.w0[i];
        }
        let mut rhs = vec![0.0; k];
        rhs[size] = 1.0;
        let qs = lu_solve(&b, &rhs)?;
        let w = lu_solve(&bt, &rhs)?;
        Some((qs[..size].to_vec(), qs[size], w[..size].to_vec()))
    }

    fn singular_at(&self, z: &[f64]) -> Vec<f64> {
        let n = self.dim();
        self.singular(&self.jacobian(&z[..n], &self.params_at(z)))
    }

    /// Borders for a seed, where no `q` is known yet: approximate null
    /// vectors of `M` and `Mᵀ`.
    fn first_borders(&mut self, z: &[f64]) -> Option<()> {
        let (m, size) = (self.singular_at(z), self.size());
        let transposed: Vec<f64> = (0..size * size)
            .map(|i| m[i % size * size + i / size])
            .collect();
        self.v0 = unit(null_vector(&m, size)?)?;
        self.w0 = unit(null_vector(&transposed, size)?)?;
        Some(())
    }

    /// The normalized `q` and `w` at an accepted point become the borders.
    fn renew_borders(&mut self, z: &[f64]) {
        let found = self.bordered_solve(&self.singular_at(z));
        if let Some((Some(v), Some(w))) = found.map(|(q, _, w)| (unit(q), unit(w))) {
            (self.v0, self.w0) = (v, w);
        }
    }

    fn point(&self, s: f64, z: &[f64], tangent: Vec<f64>) -> CurvePoint {
        let n = self.dim();
        let p = self.params_at(z);
        let eigenvalues = eigenvalues(n, &self.jacobian(&z[..n], &p));
        CurvePoint {
            s,
            p: (p[self.free[0]], p[self.free[1]]),
            x: z[..n].to_vec(),
            tangent,
            nature: nature(self.kind, &eigenvalues),
            eigenvalues,
        }
    }

    fn curve(&self, points: Vec<CurvePoint>, end: CurveEnd) -> Curve {
        Curve {
            kind: self.kind,
            period: self.power.k,
            points,
            end,
        }
    }

    /// The step loop of a branch, without detection, in the box `[0, 1]²`
    /// of `(u₁, u₂)`, which also stops where the curve closes.
    fn follow(
        mut self,
        start: CurvePoint,
        mut z: Vec<f64>,
        mut dir: Vec<f64>,
        mut h: f64,
        opts: &CurveOptions,
    ) -> Curve {
        let n = self.dim();
        let cos_max = cos(opts.max_angle);
        let z0 = z.clone();
        let mut points = vec![start];
        let mut away = false;
        let end = loop {
            if points.len() >= opts.max_points {
                break CurveEnd::MaxPoints;
            }
            if h < opts.ds_min {
                break CurveEnd::StepTooSmall;
            }
            let last = points.last().expect("the seed");
            let pred: Vec<f64> = z.iter().zip(&dir).map(|(a, d)| a + h * d).collect();
            let edge = first_edge(&z, &pred, &dir, n);
            let corrected = match edge {
                Some((k, e)) if z[k] == e => break CurveEnd::Range,
                Some((k, e)) => {
                    let along = (e - z[k]) / dir[k];
                    let guess: Vec<f64> = z.iter().zip(&dir).map(|(a, d)| a + along * d).collect();
                    correct_fixed(&self, &guess, k, e)
                }
                None => correct_along(&self, &pred, &dir),
            };
            let accepted = corrected.ok().and_then(|(z_new, iters)| {
                let t = tangent(&self, &z_new, &last.tangent)?;
                (dot(&t, &last.tangent) >= cos_max).then_some((z_new, iters, t))
            });
            let Some((z_new, iters, t_new)) = accepted else {
                h /= 2.0;
                continue;
            };
            let len = dist(&z, &z_new);
            let next = self.point(last.s + len, &z_new, t_new);
            self.renew_borders(&z_new);
            points.push(next);
            if edge.is_some() {
                break CurveEnd::Range;
            }
            dir = diff(&z_new, &z).iter().map(|c| c / len).collect();
            z = z_new;
            if iters <= FAST_NEWTON_ITERS {
                h = (h * STEP_GROWTH).min(opts.ds_max);
            } else if iters > SLOW_NEWTON_ITERS {
                h = (h / 2.0).max(opts.ds_min);
            }
            let back: f64 = diff(&z, &z0).iter().map(|c| c * c).sum();
            let (first, last) = (&points[0], &points[points.len() - 1]);
            let returns = back <= h * h && dot(&last.tangent, &first.tangent) > 0.0;
            if away && returns && points.len() < opts.max_points {
                let again = CurvePoint {
                    s: last.s + back.sqrt(),
                    ..first.clone()
                };
                points.push(again);
                break CurveEnd::Closed;
            }
            away |= back > h * h;
        };
        self.curve(points, end)
    }
}

impl Underdetermined for Defining<'_> {
    /// Row `i < dim` is `[g_x | g_{p₁}·(hi₁ − lo₁) | g_{p₂}·(hi₂ − lo₂)]`,
    /// the last row `ψ_z` with `ψ' = −wᵀ M' q` along each unknown. For a
    /// fold or flip the symmetry of the second derivative gives all of
    /// `ψ_x = −wᵀ D(q, 0)` from one derivative of the Jacobian.
    fn linear(&self, z: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.dim();
        let (x, p) = (&z[..n], self.params_at(z));
        let a = self.jacobian(x, &p);
        let Some((q, psi, w)) = self.bordered_solve(&self.singular(&a)) else {
            return (vec![f64::NAN; n + 1], vec![f64::NAN; (n + 1) * (n + 2)]);
        };
        let width = self.edges.map(|(lo, hi)| hi - lo);
        let unit_vector = |len: usize, i: usize| {
            let mut e = vec![0.0; len];
            e[i] = 1.0;
            e
        };
        let derivative = |dx: &[f64], dp: &[f64]| {
            let mut da = vec![0.0; n * n];
            self.power.jacobian_derivative(0.0, x, &p, dx, dp, &mut da);
            da
        };
        let along = |da: &[f64]| -dot(&w, &matvec(&self.singular_derivative(&a, da), &q, q.len()));
        let (still_x, still_p) = (vec![0.0; n], vec![0.0; p.len()]);
        let mut psi_z: Vec<f64> = match self.kind {
            CurveKind::Fold | CurveKind::Flip => {
                let d = derivative(&q, &still_p);
                (0..n)
                    .map(|j| -(0..n).map(|i| w[i] * d[i * n + j]).sum::<f64>())
                    .collect()
            }
            CurveKind::Hopf | CurveKind::NeimarkSacker => (0..n)
                .map(|j| along(&derivative(&unit_vector(n, j), &still_p)))
                .collect(),
        };
        psi_z.extend(
            (0..2).map(|k| {
                along(&derivative(&still_x, &unit_vector(p.len(), self.free[k]))) * width[k]
            }),
        );
        let mut g = vec![0.0; n];
        self.power.eval(0.0, x, &p, &mut g);
        let gp = self.free.map(|free| {
            let mut column = vec![0.0; n];
            self.power.param_jacobian(0.0, x, &p, free, &mut column);
            column
        });
        let mut dg = Vec::with_capacity((n + 1) * (n + 2));
        for i in 0..n {
            dg.extend_from_slice(&a[i * n..(i + 1) * n]);
            if self.map {
                g[i] -= x[i];
                dg[i * (n + 2) + i] -= 1.0;
            }
            dg.extend((0..2).map(|k| gp[k][i] * width[k]));
        }
        dg.extend(psi_z);
        g.push(psi);
        (g, dg)
    }
}

/// `z` with `z[k] = e` held and the other unknowns corrected by Newton's
/// method on `G = 0`, a square system.
fn correct_fixed(
    g: &impl Underdetermined,
    z: &[f64],
    k: usize,
    e: f64,
) -> Result<(Vec<f64>, usize), Fail> {
    let with = |y: &[f64]| {
        let mut z = y.to_vec();
        z.insert(k, e);
        z
    };
    let mut y = z.to_vec();
    y.remove(k);
    let (y, iters) = newton(y, |y| {
        let (r, a) = g.linear(&with(y));
        let cols = y.len() + 1;
        let square = a
            .iter()
            .enumerate()
            .filter(|(i, _)| i % cols != k)
            .map(|(_, v)| *v)
            .collect();
        (r, square)
    })?;
    Ok((with(&y), iters))
}

/// The edge of `[0, 1]²` in `(u₁, u₂)` that the step from `z` to `pred`
/// along `dir` crosses first: the index of the coordinate in `z`, and its
/// value on the edge.
fn first_edge(z: &[f64], pred: &[f64], dir: &[f64], n: usize) -> Option<(usize, f64)> {
    [n, n + 1]
        .into_iter()
        .filter_map(|k| {
            let e = if pred[k] > 1.0 {
                1.0
            } else if pred[k] < 0.0 {
                0.0
            } else {
                return None;
            };
            Some((k, e, (e - z[k]) / dir[k]))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(k, e, _)| (k, e))
}

/// A bifurcation or a neutral saddle, from the pair of eigenvalues that
/// meets the curve's condition best. Hopf: the pair with the smallest
/// `|λᵢ + λⱼ|` is a Hopf pair with `ω = √κ` where `κ = Re(λᵢλⱼ) > 0`.
/// Neimark–Sacker: the pair with the smallest `|μᵢμⱼ − 1|` is critical
/// where `c = Re(μᵢ + μⱼ)/2` has `|c| < 1`.
fn nature(kind: CurveKind, eigenvalues: &[(f64, f64)]) -> Nature {
    let closest = |distance: fn((f64, f64), (f64, f64)) -> f64| {
        let len = eigenvalues.len();
        (0..len)
            .flat_map(|i| (i + 1..len).map(move |j| (eigenvalues[i], eigenvalues[j])))
            .min_by(|a, b| distance(a.0, a.1).total_cmp(&distance(b.0, b.1)))
    };
    let omega = match kind {
        CurveKind::Fold | CurveKind::Flip => None,
        CurveKind::Hopf => {
            let pair = closest(|a, b| (a.0 + b.0) * (a.0 + b.0) + (a.1 + b.1) * (a.1 + b.1));
            match pair.map(|(a, b)| a.0 * b.0 - a.1 * b.1) {
                Some(kappa) if kappa > 0.0 => Some(kappa.sqrt()),
                Some(_) => return Nature::Neutral,
                None => None,
            }
        }
        CurveKind::NeimarkSacker => {
            let pair = closest(|a, b| {
                let (re, im) = (a.0 * b.0 - a.1 * b.1 - 1.0, a.0 * b.1 + a.1 * b.0);
                re * re + im * im
            });
            match pair {
                Some((a, b)) if ((a.0 + b.0) / 2.0).abs() < 1.0 => Some(a.1.abs().atan2(a.0)),
                Some(_) => return Nature::Neutral,
                None => None,
            }
        }
    };
    Nature::Bifurcation { omega }
}

/// `[(p, q), (r, s)] ↦ a_pr·b_qs − a_ps·b_qr` over the pairs `p > q`,
/// `r > s`. The bialternate products are `2A ⊙ I = cross(A, I) + cross(I, A)`
/// and `A ⊙ A = cross(A, A)`; the eigenvalues of the first are the sums
/// `λᵢ + λⱼ`, those of the second the products `λᵢλⱼ`, `i < j`.
fn cross(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    let pairs: Vec<(usize, usize)> = (1..n).flat_map(|p| (0..p).map(move |q| (p, q))).collect();
    pairs
        .iter()
        .flat_map(|&(p, q)| {
            pairs
                .iter()
                .map(move |&(r, s)| a[p * n + r] * b[q * n + s] - a[p * n + s] * b[q * n + r])
        })
        .collect()
}

fn sum(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(u, v)| u + v).collect()
}

fn shifted(a: &[f64], n: usize, c: f64) -> Vec<f64> {
    let mut a = a.to_vec();
    for i in 0..n {
        a[i * n + i] += c;
    }
    a
}

fn unit(v: Vec<f64>) -> Option<Vec<f64>> {
    let len = norm2(&v);
    (len.is_finite() && len > 0.0).then(|| v.iter().map(|c| c / len).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dyn_expr::{compile, CompiledSystem, InputFormat};

    fn system(source: &str) -> CompiledSystem {
        compile(source, InputFormat::Text)
            .system
            .unwrap_or_else(|| panic!("{source} compiles"))
    }

    #[test]
    fn defining_system_derivatives_match_central_differences() {
        let flow =
            system("x' = a*x - y + x*z^2 + b*y^2\ny' = x + b*y - x*y*z\nz' = -z + a*x^2 + y*z");
        let map = system(
            "x_{n+1} = a*x_n*(1 - x_n) + 0.1*y_n*z_n\n\
             y_{n+1} = b*x_n + 0.3*z_n\n\
             z_{n+1} = 0.5*y_n - 0.2*x_n*z_n + a*b",
        );
        let opts = CurveOptions {
            free: (1, 0),
            ranges: ((2.5, 0.5), (-1.0, 3.0)),
            ds: 0.1,
            ds_min: 1e-8,
            ds_max: 0.1,
            max_points: 10,
            max_angle: 0.3,
        };
        let cases = [
            (&flow, CurveKind::Fold, 1),
            (&flow, CurveKind::Hopf, 1),
            (&map, CurveKind::Fold, 2),
            (&map, CurveKind::Flip, 2),
            (&map, CurveKind::NeimarkSacker, 2),
        ];
        for (f, kind, period) in cases {
            let x = [0.3, -0.4, 0.5];
            let seed = CurveSeed {
                kind,
                x: x.to_vec(),
                period,
            };
            let mut sys = Defining::new(f, &[1.3, 0.7], &seed, &opts);
            let z = sys.scaled(&x);
            sys.first_borders(&z).expect("borders");
            let (g, dg) = sys.linear(&z);
            let (rows, cols) = (g.len(), z.len());
            assert!(g[rows - 1].abs() > 1e-3, "{kind:?}: ψ = {}", g[rows - 1]);
            let psi_z = &dg[(rows - 1) * cols..];
            assert!(
                psi_z.iter().all(|v| v.abs() > 1e-4),
                "{kind:?}: ψ_z = {psi_z:?}"
            );
            let h = 1e-6;
            for j in 0..cols {
                let at = |sign: f64| {
                    let mut z = z.clone();
                    z[j] += sign * h;
                    sys.linear(&z).0
                };
                let (plus, minus) = (at(1.0), at(-1.0));
                for i in 0..rows {
                    let fd = (plus[i] - minus[i]) / (2.0 * h);
                    let exact = dg[i * cols + j];
                    assert!(
                        (fd - exact).abs() <= 1e-7 * (1.0 + exact.abs()),
                        "{kind:?} ∂G{i}/∂z{j}: {exact} vs {fd}"
                    );
                }
            }
        }
    }

    fn assert_same_values(mut got: Vec<(f64, f64)>, want: &[(f64, f64)]) {
        assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
        for w in want {
            let d = |v: &(f64, f64)| (v.0 - w.0).abs().max((v.1 - w.1).abs());
            let k = (0..got.len())
                .min_by(|&i, &j| d(&got[i]).total_cmp(&d(&got[j])))
                .expect("left");
            assert!(d(&got[k]) <= 1e-10, "{w:?} not in {got:?}");
            got.remove(k);
        }
    }

    #[test]
    fn bialternate_products_have_the_sums_and_products_of_eigenvalues() {
        let n = 4;
        let a = [
            1.0, 2.0, 0.0, 1.0, //
            -3.0, 0.5, 1.0, 0.0, //
            0.0, 1.0, -1.0, 2.0, //
            1.0, 0.0, -2.0, 0.3,
        ];
        let lambda = eigenvalues(n, &a);
        let pairs: Vec<((f64, f64), (f64, f64))> = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .map(|(i, j)| (lambda[i], lambda[j]))
            .collect();
        let sums: Vec<(f64, f64)> = pairs.iter().map(|(l, m)| (l.0 + m.0, l.1 + m.1)).collect();
        let products: Vec<(f64, f64)> = pairs
            .iter()
            .map(|(l, m)| (l.0 * m.0 - l.1 * m.1, l.0 * m.1 + l.1 * m.0))
            .collect();
        assert!(lambda.iter().any(|l| l.1 != 0.0), "{lambda:?}");
        let eye = identity(n);
        let hopf = sum(&cross(&a, &eye, n), &cross(&eye, &a, n));
        assert_same_values(eigenvalues(6, &hopf), &sums);
        assert_same_values(eigenvalues(6, &cross(&a, &a, n)), &products);

        let b = [1.0, 2.0, 3.0, 4.0];
        let eye = identity(2);
        assert_eq!(sum(&cross(&b, &eye, 2), &cross(&eye, &b, 2)), [5.0]);
        assert_eq!(cross(&b, &b, 2), [-2.0]);
    }
}
