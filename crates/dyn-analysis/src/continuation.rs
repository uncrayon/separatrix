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
//!
//! Special points come from one table of test functions, [`DETECTORS`],
//! each of which changes sign at its bifurcation, as in BifurcationKit.jl's
//! event detection. A step is accepted only if its change in the unstable
//! count is the jump of the one test that changed sign, or zero when none
//! did; the sign change is then located by bisection in arclength.

use std::f64::consts::PI;

use dyn_core::{SystemKind, VectorField};

use crate::eigvec::eigenvectors;
use crate::linalg::{eigenvalues, lu_det, lu_solve};

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
/// A step may reach this multiple of the predicted arclength `s*` to the
/// next crossing of the stability boundary. Up to 2 it never steps over
/// both crossings of a dip `d(s) = c (s − s₁)(s − s₂)`: from `L` before
/// `s₁`, `s* = L (L + Δ) / (2L + Δ)` and `2 s* < L + Δ`. Above 1 the step
/// lands past a near crossing rather than creeping up to it in halving
/// steps, which a factor of 1/2 does.
const PROXIMITY: f64 = 1.5;
/// The farthest the first point of a switched branch may land from the
/// switch point, as a multiple of the offset `h`: the second branch crosses
/// the hyperplane `h` along `d` at `h / cos θ`, `θ` its angle to `d`, so this
/// takes branches up to 60° off `d`, and a point farther away is on another
/// branch through the same region (another point of the old cycle).
const SWITCH_REACH: f64 = 2.0;
/// Below this residual of `f^k − x` (relative to `1 + |z|∞`) the first point
/// after a flip of `f^k` is a point of a `k`-cycle, not of the new `2k`-cycle.
const SWITCH_OLD_TOL: f64 = 1e-6;
/// Bisection steps allowed to locate a special point.
const MAX_BISECTION: usize = 200;

/// How to follow a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuationOptions {
    /// Index of the continuation parameter.
    pub param: usize,
    /// `k` for fixed points of `f^k` of a map; ignored (taken as 1) for
    /// flows, and by [`switch_branch`], which takes the switch point's.
    pub period: usize,
    /// The parameter interval the branch stays in.
    pub range: (f64, f64),
    /// Signed first step in arclength: its sign is the direction in `p`,
    /// or for [`switch_branch`] the half of the new branch.
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
    /// Eigenvalues with `Re λ > 0` (flow) or `|μ| > 1` (map). At a special
    /// point the critical eigenvalues count as neutral, so it is the smaller
    /// count of its two sides.
    pub unstable: usize,
    /// Every eigenvalue has `Re λ < 0` (flow) or `|μ| < 1` (map). `false`
    /// at a special point.
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
    /// The points in order of arclength, the corrected start first. Each
    /// special point is one of them.
    pub points: Vec<BranchPoint>,
    /// The special points in order of arclength.
    pub special: Vec<SpecialPoint>,
    /// Why the branch stopped.
    pub end: BranchEnd,
}

/// The bifurcations located on a branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialKind {
    /// A limit point: the branch turns back in `p` (an eigenvalue `0` of a
    /// flow, a multiplier `+1` of a map).
    Fold,
    /// A complex pair of eigenvalues of a flow crosses the imaginary axis.
    Hopf,
    /// Another branch crosses this one (pitchfork, transcritical).
    BranchPoint,
    /// A multiplier of a map passes through `−1` (period doubling).
    Flip,
    /// A complex pair of multipliers of a map crosses the unit circle.
    NeimarkSacker,
}

impl SpecialKind {
    /// The standard abbreviation: `LP`, `H`, `BP`, `PD`, `NS`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Fold => "LP",
            Self::Hopf => "H",
            Self::BranchPoint => "BP",
            Self::Flip => "PD",
            Self::NeimarkSacker => "NS",
        }
    }

    /// The name in prose.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fold => "fold",
            Self::Hopf => "Hopf",
            Self::BranchPoint => "branch point",
            Self::Flip => "flip",
            Self::NeimarkSacker => "Neimark–Sacker",
        }
    }
}

/// A located bifurcation on a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecialPoint {
    /// What it is.
    pub kind: SpecialKind,
    /// Its own entry in [`Branch::points`].
    pub index: usize,
    /// The parameter.
    pub p: f64,
    /// The state.
    pub x: Vec<f64>,
    /// `k` of `f^k`, as [`Branch::period`].
    pub period: usize,
    /// The unit tangent of the branch there.
    pub tangent: Vec<f64>,
    /// The eigenvalues there, the critical ones on the stability boundary.
    pub eigenvalues: Vec<(f64, f64)>,
    /// Hopf: the frequency `ω = Im λ`. Neimark–Sacker: the angle
    /// `θ = arg μ` of the critical multiplier, in `(0, π)`.
    pub omega: Option<f64>,
    /// Hopf: the first Lyapunov coefficient `ℓ₁`, negative when the
    /// bifurcation is supercritical. Kuznetsov's formula (Elements of
    /// Applied Bifurcation Theory, §3.5) with `⟨p, q⟩ = 1` and
    /// `⟨q, q⟩ = 1/2`, the scaling at which the normal form
    /// `ṙ = μr + a r³, θ̇ = ω` has `ℓ₁ = a/ω`. `None` where a solve fails.
    pub lyapunov: Option<f64>,
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
    /// [`switch_branch`] at a kind of point with no second branch to switch
    /// onto (anything but a branch point, or a flip of a map).
    CannotSwitch,
    /// [`switch_branch`] found no point of a second branch near the switch
    /// point, or its kernel there is not two-dimensional.
    NoSecondBranch,
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
    let g = Residual::new(f, params, opts.param, opts.period);
    let n = f.dim();
    let p0 = params[opts.param];
    let (z, _) = g.correct_at_fixed_p(x0, p0).map_err(|e| match e {
        Fail::Singular => ContinuationError::Singular,
        Fail::NoConvergence => ContinuationError::NotAFixedPoint,
    })?;
    let mut seed = vec![0.0; n + 1];
    seed[n] = if opts.ds < 0.0 { -1.0 } else { 1.0 };
    let tau = g.tangent(&z, &seed).ok_or(ContinuationError::Singular)?;
    let start = g.point(0.0, &z, tau.clone());
    if !in_range(opts, p0) {
        return Ok(g.branch(vec![start], Vec::new(), BranchEnd::Range));
    }
    let h = opts.ds.abs().min(opts.ds_max).max(opts.ds_min);
    Ok(g.follow(vec![start], tau, h, opts))
}

/// Halvings of the offset allowed when no second branch is found at it.
const SWITCH_HALVINGS: usize = 8;

/// Follow the other branch through the branch point or flip `at`, found on
/// a branch of `f` with the same `params` and `opts.param`.
///
/// At a branch point the kernel of `[g_x | g_p]` is two-dimensional; its
/// unit vector `d` orthogonal to `at.tangent` is the new direction. At a
/// flip of `f^k` the same holds for `g = f^{2k} − x`, whose kernel there is
/// spanned by the old tangent and `(v, 0)`, `v` the eigenvector of the
/// multiplier `−1`, so the new branch is the `2k`-cycle born there. The
/// first new point is corrected on the hyperplane through `z* + h d`
/// orthogonal to `d`, which is parallel to the old tangent and so misses
/// the old branch to first order; `h = |ds|` is halved up to
/// [`SWITCH_HALVINGS`] times when Newton's method fails there or the point
/// lies outside `opts.range`. `d` is oriented so that its largest
/// component is positive for `ds > 0` and negative for `ds < 0`, which
/// picks one half of the new branch.
///
/// The branch starts at `at` itself, neutral, so the halves for `±ds`
/// share their first point. A start outside `opts.range`, or a first new
/// point outside it at every offset, gives the start alone, ending with
/// [`BranchEnd::Range`]. Only [`SpecialKind::BranchPoint`] and, on a map,
/// [`SpecialKind::Flip`] can be switched at.
pub fn switch_branch(
    f: &dyn VectorField,
    params: &[f64],
    at: &SpecialPoint,
    opts: &ContinuationOptions,
) -> Result<Branch, ContinuationError> {
    if !f.is_autonomous() {
        return Err(ContinuationError::NotAutonomous);
    }
    let period = match (at.kind, f.kind()) {
        (SpecialKind::BranchPoint, _) => at.period,
        (SpecialKind::Flip, SystemKind::Map) => 2 * at.period.max(1),
        _ => return Err(ContinuationError::CannotSwitch),
    };
    let g = Residual::new(f, params, opts.param, period);
    // At a flip every point of the old cycle is also a fixed point of
    // `f^{2k}`: a first point that `f^k` already fixes is on one of those.
    let old = (period != at.period).then(|| Residual::new(f, params, opts.param, at.period));
    let on_old = |z: &[f64]| {
        old.as_ref()
            .is_some_and(|o| norm_inf(&o.linear(z).0) <= SWITCH_OLD_TOL * (1.0 + norm_inf(z)))
    };
    let z_star: Vec<f64> = at.x.iter().copied().chain([at.p]).collect();
    let len = norm2(&at.tangent);
    let tau: Vec<f64> = at.tangent.iter().map(|t| t / len).collect();
    let mut d = g
        .second_direction(&z_star, &tau)
        .ok_or(ContinuationError::NoSecondBranch)?;
    let largest = d
        .iter()
        .copied()
        .reduce(|a, b| if b.abs() > a.abs() { b } else { a })
        .unwrap_or(1.0);
    let sign = if (largest < 0.0) == (opts.ds < 0.0) {
        1.0
    } else {
        -1.0
    };
    d.iter_mut().for_each(|c| *c *= sign);
    let mut start = g.point(0.0, &z_star, d.clone());
    start.stable = false;
    if !in_range(opts, at.p) {
        return Ok(g.branch(vec![start], Vec::new(), BranchEnd::Range));
    }
    if opts.max_points < 2 {
        return Ok(g.branch(vec![start], Vec::new(), BranchEnd::MaxPoints));
    }
    let mut h = opts.ds.abs().min(opts.ds_max).max(opts.ds_min);
    let mut outside = false;
    for _ in 0..=SWITCH_HALVINGS {
        let pred: Vec<f64> = z_star.iter().zip(&d).map(|(z, c)| z + h * c).collect();
        let found = g.correct_along(&pred, &d).ok().and_then(|(z1, _)| {
            let w = diff(&z1, &z_star);
            let len = norm2(&w);
            if len > SWITCH_REACH * h || on_old(&z1) {
                return None;
            }
            let dir: Vec<f64> = w.iter().map(|c| c / len).collect();
            let t1 = g.tangent(&z1, &dir)?;
            Some((z1, len, dir, t1))
        });
        outside = found
            .as_ref()
            .is_some_and(|(z1, ..)| !in_range(opts, z1[z1.len() - 1]));
        let Some((z1, len, dir, t1)) = found.filter(|_| !outside) else {
            h /= 2.0;
            continue;
        };
        let first = g.point(len, &z1, t1);
        start.unstable = start.unstable.min(first.unstable);
        return Ok(g.follow(vec![start, first], dir, h.max(opts.ds_min), opts));
    }
    if outside {
        return Ok(g.branch(vec![start], Vec::new(), BranchEnd::Range));
    }
    Err(ContinuationError::NoSecondBranch)
}

fn in_range(opts: &ContinuationOptions, p: f64) -> bool {
    let (a, b) = opts.range;
    (a.min(b)..=a.max(b)).contains(&p)
}

/// A test function that changes sign at one kind of special point.
struct Detector {
    kind: SpecialKind,
    applies_to: &'static [SystemKind],
    /// The change of the unstable count across it, `±jump`.
    jump: usize,
    /// The signed test at a point; `None` where it is undefined (no
    /// complex pair, for Hopf and Neimark–Sacker).
    test: fn(&Residual, &BranchPoint) -> Option<f64>,
    /// [`SpecialPoint::omega`] and [`SpecialPoint::lyapunov`] of the
    /// located point.
    extras: fn(&Residual, &BranchPoint) -> OmegaLyapunov,
}

type OmegaLyapunov = (Option<f64>, Option<f64>);

/// Every special point a branch can carry. A fold or branch point of a map
/// is one of `g = f^k − x` (a multiplier `+1`), so those two rows serve
/// both kinds of system.
const DETECTORS: &[Detector] = &[
    Detector {
        kind: SpecialKind::Fold,
        applies_to: &[SystemKind::Ode, SystemKind::Map],
        jump: 1,
        test: |_, pt| pt.tangent.last().copied(),
        extras: |_, _| (None, None),
    },
    Detector {
        kind: SpecialKind::BranchPoint,
        applies_to: &[SystemKind::Ode, SystemKind::Map],
        jump: 1,
        test: |g, pt| {
            let (_, a) = g.linear(&z_of(pt));
            Some(lu_det(&bordered(a, &pt.tangent)))
        },
        extras: |_, _| (None, None),
    },
    Detector {
        kind: SpecialKind::Hopf,
        applies_to: &[SystemKind::Ode],
        jump: 2,
        test: |_, pt| upper_pairs(pt).map(|(re, _)| re).reduce(f64::max),
        extras: |g, pt| {
            let pair = critical_pair(pt, |re, _| re.abs());
            (
                pair.map(|(_, im)| im),
                pair.and_then(|l| first_lyapunov(g, pt, l)),
            )
        },
    },
    Detector {
        kind: SpecialKind::Flip,
        applies_to: &[SystemKind::Map],
        jump: 1,
        // det(D f^k + I) = Π (μ + 1): a complex pair adds |μ + 1|² > 0 and
        // a pair that splits at μ < −1 two negative factors, so only a
        // multiplier through −1 changes its sign.
        test: |g, pt| {
            let n = pt.x.len();
            let (_, a) = g.linear(&z_of(pt));
            let shifted: Vec<f64> = (0..n * n)
                .map(|k| a[k / n * (n + 1) + k % n] + if k / n == k % n { 2.0 } else { 0.0 })
                .collect();
            Some(lu_det(&shifted))
        },
        extras: |_, _| (None, None),
    },
    Detector {
        kind: SpecialKind::NeimarkSacker,
        applies_to: &[SystemKind::Map],
        jump: 2,
        test: |_, pt| {
            upper_pairs(pt)
                .map(|(re, im)| (re * re + im * im).sqrt() - 1.0)
                .reduce(f64::max)
        },
        extras: |_, pt| {
            let pair = critical_pair(pt, |re, im| ((re * re + im * im).sqrt() - 1.0).abs());
            (pair.map(|(re, im)| im.atan2(re)), None)
        },
    },
];

/// One eigenvalue `(re, im)` with `im > 0` per complex pair.
fn upper_pairs(pt: &BranchPoint) -> impl Iterator<Item = (f64, f64)> + '_ {
    pt.eigenvalues.iter().copied().filter(|&(_, im)| im > 0.0)
}

/// The pair (`im > 0`) nearest the stability boundary by `distance`.
fn critical_pair(pt: &BranchPoint, distance: impl Fn(f64, f64) -> f64) -> Option<(f64, f64)> {
    upper_pairs(pt).min_by(|a, b| distance(a.0, a.1).total_cmp(&distance(b.0, b.1)))
}

/// The longest next step: [`PROXIMITY`] times the arclength at which the
/// nearest approaching eigenvalue reaches the stability boundary, its
/// distance `d_i` (`|Re λ|`, or `||μ| − 1|`) extrapolated linearly along
/// the secant `a → b` of length `len`, so that two crossings never fall
/// inside one step and cancel in the count.
fn proximity_limit(map: bool, a: &BranchPoint, b: &BranchPoint, len: f64) -> f64 {
    let distances = |pt: &BranchPoint| {
        let mut d: Vec<f64> = pt
            .eigenvalues
            .iter()
            .map(|&(re, im)| {
                if map {
                    ((re * re + im * im).sqrt() - 1.0).abs()
                } else {
                    re.abs()
                }
            })
            .collect();
        d.sort_by(f64::total_cmp);
        d
    };
    distances(a)
        .iter()
        .zip(distances(b))
        .filter_map(|(da, db)| {
            let rate = (db - da) / len;
            (rate < 0.0 && db > TOL).then(|| PROXIMITY * db / -rate)
        })
        .fold(f64::INFINITY, f64::min)
}

/// The first Lyapunov coefficient at a Hopf point with critical eigenvalue
/// `λ = (re, ω)`, by Kuznetsov's formula (Elements of Applied Bifurcation
/// Theory, 3rd ed., §3.5, eq. 3.20):
///
/// ```text
/// ℓ₁ = Re[⟨p, C(q,q,q̄)⟩ − 2⟨p, B(q, A⁻¹B(q,q̄))⟩ + ⟨p, B(q̄, (2iωI − A)⁻¹B(q,q))⟩] / 2ω
/// ```
///
/// with `A q = iω q`, `Aᵀ p = −iω p`, `⟨p, q⟩ = 1`, `⟨q, q⟩ = 1/2`. `B` and
/// `C` are central differences of the exact Jacobian; the complex solve goes
/// through the real `2n` system.
fn first_lyapunov(g: &Residual, pt: &BranchPoint, lambda: (f64, f64)) -> Option<f64> {
    let n = pt.x.len();
    let omega = lambda.1;
    let params = g.params_at(pt.p);
    let jac = |x: &[f64]| {
        let mut j = vec![0.0; n * n];
        g.power.jacobian(0.0, x, &params, &mut j);
        j
    };
    let a = jac(&pt.x);
    let k = pt.eigenvalues.iter().position(|&l| l == lambda)?;
    let vector = |m: &[f64]| {
        let v = eigenvectors(SystemKind::Ode, n, m, &pt.eigenvalues);
        Some(C::new(v.re[k].clone()?, v.im[k].clone()?))
    };
    let transpose: Vec<f64> = (0..n * n).map(|i| a[i % n * n + i / n]).collect();
    let q = vector(&a)?;
    let q = q.scale(1.0 / (2.0 * q.dot_conj(&q).0).sqrt());
    // w = p̄, so ⟨p, v⟩ = Σ wᵢ vᵢ.
    let w = vector(&transpose)?;
    let w = w.div(w.dot(&q));

    let scale = 1.0 + norm_inf(&pt.x);
    // J(x ± εĉ) with ĉ = c/|c|, and |c|.
    let around = |c: &[f64], eps: f64| {
        let len = norm2(c);
        let at = |sign: f64| {
            let x: Vec<f64> = (0..n).map(|i| pt.x[i] + sign * eps * c[i] / len).collect();
            jac(&x)
        };
        (at(1.0), at(-1.0), len)
    };
    // DJ[c] u ≈ (J(x + εĉ) − J(x − εĉ)) u |c| / 2ε.
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
    let b = |u: &C, v: &C| -> C {
        let rr = b_real(&u.re, &v.re);
        let ii = b_real(&u.im, &v.im);
        let ri = b_real(&u.re, &v.im);
        let ir = b_real(&u.im, &v.re);
        C {
            re: (0..n).map(|i| rr[i] - ii[i]).collect(),
            im: (0..n).map(|i| ri[i] + ir[i]).collect(),
        }
    };
    // D²J[c, c] ≈ (J(x + εĉ) − 2J(x) + J(x − εĉ)) |c|² / ε².
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
    // C(q, q, q̄) = D²J[q, q̄] q, and D²J[q, q̄] = D²J[Re q, Re q] + D²J[Im q, Im q].
    let m: Vec<f64> = d2j(&q.re)
        .iter()
        .zip(d2j(&q.im))
        .map(|(u, v)| u + v)
        .collect();
    let cqqq = C {
        re: matvec(&m, &q.re, n),
        im: matvec(&m, &q.im, n),
    };
    let h11 = lu_solve(&a, &b(&q, &q.conj()).re)?;
    let minus_a: Vec<f64> = a.iter().map(|v| -v).collect();
    let h20 = complex_solve(&minus_a, 2.0 * omega, &b(&q, &q))?;
    let total =
        w.dot(&cqqq).0 - 2.0 * w.dot(&b(&q, &C::real(h11))).0 + w.dot(&b(&q.conj(), &h20)).0;
    let l1 = total / (2.0 * omega);
    l1.is_finite().then_some(l1)
}

/// A complex vector as its real and imaginary parts.
#[derive(Clone)]
struct C {
    re: Vec<f64>,
    im: Vec<f64>,
}

impl C {
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
    /// `self / (re + i im)`.
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
    /// `Σ selfᵢ vᵢ`, without conjugation.
    fn dot(&self, v: &C) -> (f64, f64) {
        (0..self.re.len()).fold((0.0, 0.0), |(r, i), k| {
            (
                r + self.re[k] * v.re[k] - self.im[k] * v.im[k],
                i + self.re[k] * v.im[k] + self.im[k] * v.re[k],
            )
        })
    }
    /// `Σ conj(selfᵢ) vᵢ`.
    fn dot_conj(&self, v: &C) -> (f64, f64) {
        self.conj().dot(v)
    }
}

/// Solve `(m + i·s·I) z = r` through the real `2n` system
/// `[m, −sI; sI, m] [Re z; Im z] = [Re r; Im r]`.
fn complex_solve(m: &[f64], s: f64, r: &C) -> Option<C> {
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
    Some(C::new(z[..n].to_vec(), z[n..].to_vec()))
}

fn matvec(m: &[f64], v: &[f64], n: usize) -> Vec<f64> {
    (0..n)
        .map(|r| (0..n).map(|c| m[r * n + c] * v[c]).sum())
        .collect()
}

fn diff(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(u, v)| u - v).collect()
}

/// `(x, p)` of a point.
fn z_of(pt: &BranchPoint) -> Vec<f64> {
    pt.x.iter().copied().chain([pt.p]).collect()
}

fn dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(u, v)| (u - v) * (u - v))
        .sum::<f64>()
        .sqrt()
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

impl<'a> Residual<'a> {
    /// The residual of `f` (a flow; `period` is then taken as 1) or of
    /// `f^period − x` (a map).
    fn new(f: &'a dyn VectorField, params: &[f64], param: usize, period: usize) -> Self {
        let map = f.kind() == SystemKind::Map;
        Residual {
            power: Power {
                f,
                k: if map { period.max(1) } else { 1 },
            },
            map,
            params: params.to_vec(),
            param,
        }
    }

    fn branch(
        &self,
        points: Vec<BranchPoint>,
        special: Vec<SpecialPoint>,
        end: BranchEnd,
    ) -> Branch {
        Branch {
            period: self.power.k,
            points,
            special,
            end,
        }
    }

    /// Continue from the last of `points` along `dir` with first step `h`
    /// until the range, `max_points` or `ds_min` ends the branch, locating
    /// the special points on the way.
    fn follow(
        &self,
        mut points: Vec<BranchPoint>,
        mut dir: Vec<f64>,
        mut h: f64,
        opts: &ContinuationOptions,
    ) -> Branch {
        let g = self;
        let kind = g.power.f.kind();
        let (map, period) = (g.map, g.power.k);
        let n = g.power.f.dim();
        let (lo, hi) = (
            opts.range.0.min(opts.range.1),
            opts.range.0.max(opts.range.1),
        );
        let detectors: Vec<&Detector> = DETECTORS
            .iter()
            .filter(|d| d.applies_to.contains(&kind))
            .collect();
        let test_all = |pt: &BranchPoint| -> Vec<Option<f64>> {
            detectors.iter().map(|d| (d.test)(g, pt)).collect()
        };
        let mut special = Vec::new();
        let mut z = z_of(points.last().expect("a start"));
        let mut tests = test_all(points.last().expect("a start"));
        let cos_max = cos(opts.max_angle);
        let end = loop {
            if points.len() >= opts.max_points {
                break BranchEnd::MaxPoints;
            }
            if h < opts.ds_min {
                break BranchEnd::StepTooSmall;
            }
            let last = points.last().expect("the start");
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
                let t = g.tangent(&z_new, &last.tangent)?;
                (dot(&t, &last.tangent) >= cos_max).then_some((z_new, iters, t))
            });
            let Some((z_new, iters, t_new)) = accepted else {
                h /= 2.0;
                continue;
            };
            let mut next = g.point(0.0, &z_new, t_new);
            let next_tests = test_all(&next);
            let fired: Vec<(&Detector, f64)> = (0..detectors.len())
                .filter_map(|i| match (tests[i], next_tests[i]) {
                    (Some(a), Some(b)) if a * b < 0.0 => Some((detectors[i], a)),
                    _ => None,
                })
                .collect();
            let jump = next.unstable.abs_diff(last.unstable);
            let explained = match fired[..] {
                [] => jump == 0,
                [(d, _)] => jump == d.jump,
                _ => false,
            };
            if !explained {
                h /= 2.0;
                continue;
            }
            if points.len() + 1 + fired.len() > opts.max_points {
                break BranchEnd::MaxPoints;
            }
            let len = dist(&z, &z_new);
            let limit = proximity_limit(map, last, &next, len);
            if let [(d, test_last)] = fired[..] {
                let mut at = g.locate(last, &next, &dir, d, test_last);
                at.s = last.s + dist(&z_of(last), &z_of(&at));
                at.unstable = last.unstable.min(next.unstable);
                at.stable = false;
                let (omega, lyapunov) = (d.extras)(g, &at);
                special.push(SpecialPoint {
                    kind: d.kind,
                    index: points.len(),
                    p: at.p,
                    x: at.x.clone(),
                    period,
                    tangent: at.tangent.clone(),
                    eigenvalues: at.eigenvalues.clone(),
                    omega,
                    lyapunov,
                });
                points.push(at);
            }
            let prev = points.last().expect("the start");
            next.s = prev.s + dist(&z_of(prev), &z_new);
            points.push(next);
            tests = next_tests;
            if edge.is_some() {
                break BranchEnd::Range;
            }
            dir = diff(&z_new, &z).iter().map(|c| c / len).collect();
            z = z_new;
            if iters <= FAST {
                h = (h * GROW).min(opts.ds_max);
            } else if iters > SLOW {
                h = (h / 2.0).max(opts.ds_min);
            }
            h = h.min(limit.max(opts.ds_min));
        };
        g.branch(points, special, end)
    }

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

    /// The unit vector `d ⊥ τ` in the kernel of `[g_x | g_p]` at `z`: the
    /// null vector of the bordered Jacobian `[g_x g_p; τᵀ]`, singular at a
    /// branch point. Inverse iteration on its normal matrix shifted by
    /// `σ = 1e−12 (1 + trace)`: the longest column of the inverse lies along
    /// the null vector, and one more solve refines it.
    fn second_direction(&self, z: &[f64], tau: &[f64]) -> Option<Vec<f64>> {
        let m = bordered(self.linear(z).1, tau);
        let n1 = z.len();
        let mut normal: Vec<f64> = (0..n1 * n1)
            .map(|k| {
                (0..n1)
                    .map(|r| m[r * n1 + k / n1] * m[r * n1 + k % n1])
                    .sum()
            })
            .collect();
        let trace: f64 = (0..n1).map(|i| normal[i * n1 + i]).sum();
        for i in 0..n1 {
            normal[i * n1 + i] += 1e-12 * (1.0 + trace);
        }
        let column = (0..n1)
            .filter_map(|j| {
                let mut e = vec![0.0; n1];
                e[j] = 1.0;
                lu_solve(&normal, &e)
            })
            .reduce(|a, b| if norm2(&b) > norm2(&a) { b } else { a })?;
        let u = lu_solve(&normal, &column)?;
        let along = dot(&u, tau);
        let d: Vec<f64> = u.iter().zip(tau).map(|(c, t)| c - along * t).collect();
        let len = norm2(&d);
        (len.is_finite() && len > 0.0).then(|| d.iter().map(|c| c / len).collect())
    }

    /// The point between `a` and `b` where `detector`'s test (`test_a` at
    /// `a`) changes sign, by bisection in arclength along the secant `dir`,
    /// correcting at every midpoint, until `p` is known to
    /// `1e−13 (1 + |p|)` and `z` to the Newton tolerance. Its `s` is left 0.
    fn locate(
        &self,
        a: &BranchPoint,
        b: &BranchPoint,
        dir: &[f64],
        detector: &Detector,
        test_a: f64,
    ) -> BranchPoint {
        let za = z_of(a);
        let (mut lo, mut hi) = (
            (0.0, a.clone()),
            (dot(dir, &diff(&z_of(b), &za)), b.clone()),
        );
        let mut best_test = (detector.test)(self, b).unwrap_or(f64::INFINITY).abs();
        let mut best = b.clone();
        if test_a.abs() < best_test {
            (best, best_test) = (a.clone(), test_a.abs());
        }
        for _ in 0..MAX_BISECTION {
            let (zl, zh) = (z_of(&lo.1), z_of(&hi.1));
            let p_tol = 1e-13 * (1.0 + zl[zl.len() - 1].abs());
            let z_tol = TOL * (1.0 + norm_inf(&zl));
            if (zh[zh.len() - 1] - zl[zl.len() - 1]).abs() <= p_tol
                && norm_inf(&diff(&zh, &zl)) <= z_tol
            {
                break;
            }
            // Predicted from the bracket's own end along its tangent, not on
            // the first secant: that chord stays as far off a curved branch
            // as the bracket shrinks, and next to a branch point its
            // corrector lands on the other branch, whose tests have other
            // signs.
            let half = 0.5 * (hi.0 - lo.0);
            let pred: Vec<f64> = zl
                .iter()
                .zip(&lo.1.tangent)
                .map(|(z, t)| z + half * t)
                .collect();
            let zm = match self.correct_along(&pred, &lo.1.tangent) {
                Ok((zm, _)) => zm,
                // A midpoint exactly on a branch point is a zero of `g` at
                // which the bordered Jacobian is singular.
                Err(Fail::Singular)
                    if norm_inf(&self.linear(&pred).0) <= TOL * (1.0 + norm_inf(&pred)) =>
                {
                    return self.point(0.0, &pred, lo.1.tangent.clone());
                }
                Err(_) => break,
            };
            let Some(tm) = self.tangent(&zm, &lo.1.tangent) else {
                // The bordered Jacobian is singular only at the branch point
                // itself.
                return self.point(0.0, &zm, lo.1.tangent.clone());
            };
            let sigma = dot(dir, &diff(&zm, &za));
            if !(sigma > lo.0 && sigma < hi.0) {
                break;
            }
            let mid = self.point(0.0, &zm, tm);
            let Some(v) = (detector.test)(self, &mid) else {
                break;
            };
            if v.abs() < best_test {
                (best, best_test) = (mid.clone(), v.abs());
            }
            if v == 0.0 {
                break;
            }
            if (v > 0.0) == (test_a > 0.0) {
                lo = (sigma, mid);
            } else {
                hi = (sigma, mid);
            }
        }
        best
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
