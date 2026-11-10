//! Curves of folds, Hopf points, branch points, flips and Neimark–Sacker
//! points in two parameters, by pseudo-arclength continuation of a minimally
//! extended defining system (W. Govaerts, *Numerical Methods for
//! Bifurcations of Dynamical Equilibria*, SIAM 2000, the systems MatCont
//! follows).

mod codim2;

use dyn_core::{SystemKind, VectorField};

use crate::continuation::{
    correct_along, cos, diff, dist, dot, identity, matvec, newton, norm2, norm_inf, null_vector,
    tangent, Fail, Power, Underdetermined, FAST_NEWTON_ITERS, SLOW_NEWTON_ITERS, STEP_GROWTH, TOL,
};
use crate::linalg::{eigenvalues, lu_solve};
use codim2::{Detectors, Mark, Site};

/// The bifurcation a curve follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurveKind {
    /// Folds: an eigenvalue `0` of a flow, a multiplier `+1` of a map.
    Fold,
    /// Hopf points of a flow: two eigenvalues with sum `0`.
    Hopf,
    /// Branch points: another branch of equilibria or fixed points crosses
    /// (transcritical, pitchfork).
    BranchPoint,
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
    /// On a branch-point curve `p₁` is the parameter the branch point was
    /// found along, the one in which the other branch crosses.
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
    /// Most points on the curve, the seed and the codimension-2 points
    /// included.
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
        /// of the critical multiplier with `Im μ ≥ 0`. `None` on a fold,
        /// branch-point or flip curve.
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
    /// Arclength from the seed in the scaled unknowns `z` (see
    /// [`continue_curve`]).
    pub s: f64,
    /// The parameters `(p₁, p₂)`.
    pub p: (f64, f64),
    /// The state; for a map, the first point of the cycle.
    pub x: Vec<f64>,
    /// Unit tangent `ż`, oriented in the direction of continuation:
    /// `(ẋ, u̇₁, u̇₂)`, `dim + 2` values, or `(ẋ, u̇₁, u̇₂, β̇)`, `dim + 3`, on
    /// a branch-point curve.
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
    /// A corrected point of a branch-point curve needed `β ≠ 0`, with
    /// `|β| > 1e-10 · (1 + |z|∞)`, Newton's tolerance: the parameters no
    /// longer keep the branch point. That point is dropped, so the curve
    /// ends at its last branch point.
    Unfolded,
}

/// A curve of bifurcations in two parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    /// The bifurcation followed.
    pub kind: CurveKind,
    /// `k` of `f^k` for a map, 1 for a flow.
    pub period: usize,
    /// The points in order of arclength, the corrected seed first. Each
    /// codimension-2 point is one of them.
    pub points: Vec<CurvePoint>,
    /// The codimension-2 points in order of arclength.
    pub special: Vec<Codim2Point>,
    /// On a Neimark–Sacker curve, the weak resonances 1:5 to 1:9 in order of
    /// arclength, located as the codimension-2 points are but not among the
    /// points; empty on other curves.
    pub ticks: Vec<ResonanceTick>,
    /// Why the curve stopped.
    pub end: CurveEnd,
}

/// The codimension-2 bifurcations located on a curve, where its
/// bifurcation degenerates or meets another (Yu. A. Kuznetsov, *Elements of
/// Applied Bifurcation Theory*, 3rd ed., chapters 8 and 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codim2Kind {
    /// Bogdanov–Takens: a double eigenvalue `0`, where a fold curve and a
    /// Hopf curve meet.
    BogdanovTakens,
    /// Cusp: the fold's quadratic coefficient is `0`; two fold curves meet.
    Cusp,
    /// Bautin (generalized Hopf): the first Lyapunov coefficient is `0`.
    Bautin,
    /// Zero-Hopf: an eigenvalue `0` and a pair `±iω`.
    ZeroHopf,
    /// Hopf–Hopf: two pairs `±iω₁` and `±iω₂`.
    HopfHopf,
    /// 1:1 resonance: a double multiplier `+1`.
    Resonance1,
    /// 1:2 resonance: a double multiplier `−1`.
    Resonance2,
    /// 1:3 resonance: multipliers `e^{±2πi/3}`.
    Resonance3,
    /// 1:4 resonance: multipliers `±i`.
    Resonance4,
    /// Chenciner: the Neimark–Sacker coefficient is `0`.
    Chenciner,
    /// Fold-flip: multipliers `+1` and `−1`.
    FoldFlip,
    /// Generalized flip: the flip's cubic coefficient is `0`.
    GeneralizedFlip,
}

/// A located codimension-2 point on a curve.
#[derive(Debug, Clone, PartialEq)]
pub struct Codim2Point {
    /// What it is.
    pub kind: Codim2Kind,
    /// Its own entry in [`Curve::points`].
    pub index: usize,
    /// The parameters `(p₁, p₂)`.
    pub p: (f64, f64),
    /// The state; for a map, the first point of the cycle.
    pub x: Vec<f64>,
    /// The width of the last bracket of the bisection that located it, in
    /// the arclength of [`CurvePoint::s`].
    pub bracket: f64,
    /// Its normal form: Bogdanov–Takens on fold and Hopf curves of a flow,
    /// the cusp on fold curves; `None` for other points.
    pub normal_form: Option<NormalForm>,
    /// On a Neimark–Sacker curve, `θ ∈ [0, π]` of the critical multipliers
    /// `e^{±iθ}`: `0` at a 1:1 resonance, `π/2` at 1:4, `2π/3` at 1:3, `π`
    /// at 1:2. `None` on other curves.
    pub angle: Option<f64>,
    /// For a Bautin, Chenciner or generalized flip point, `(Δp₁, Δp₂)` from
    /// the point to the accepted point of the curve beside it where its
    /// coefficient is positive: `ℓ₁` (a subcritical Hopf), the
    /// Neimark–Sacker `d` (a subcritical Neimark–Sacker point) or the flip
    /// `c` (a supercritical flip). In parameter units, not of unit length.
    /// `None` for other points.
    pub positive_toward: Option<(f64, f64)>,
}

/// The normal-form coefficients of a codimension-2 point (Yu. A. Kuznetsov,
/// *Elements of Applied Bifurcation Theory*, 3rd ed., §8.2, §8.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NormalForm {
    /// `ẇ₀ = w₁, ẇ₁ = β₁ + β₂w₀ + a w₀² + b w₀w₁`, with `‖q₀‖ = 1` and the
    /// orientation of `q₀` that makes `a ≥ 0`; the sign of `b` is that of
    /// `ab`, which decides on which side of the Hopf curve the cycles are.
    BogdanovTakens {
        /// The quadratic coefficient, `≥ 0`.
        a: f64,
        /// The mixed coefficient.
        b: f64,
    },
    /// `ẇ = β₁ + β₂w + c w³`, with `‖q‖ = 1`: `c < 0` where the two folds
    /// bound a region of three equilibria, one of them stable. It does not
    /// depend on the orientation of `q`.
    Cusp {
        /// The cubic coefficient.
        c: f64,
    },
}

/// A weak resonance on a Neimark–Sacker curve, where the critical
/// multipliers are `e^{±2πi/q}` and a tongue of `q`-cycles is rooted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResonanceTick {
    /// `q`, from 5 to 9.
    pub q: usize,
    /// The parameters `(p₁, p₂)`.
    pub p: (f64, f64),
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
    /// point of the curve, or on a branch-point curve only to one with
    /// `β ≠ 0` (see [`CurveEnd::Unfolded`]).
    NotABifurcation,
    /// The defining system is singular at the seed, so the curve has no
    /// unique direction.
    Singular,
    /// `p₂` does not move the bifurcation: no equation depends on it at the
    /// seed.
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
/// A branch point is the rank defect of a wider `M = [g_x | g_{u₁}]`, of
/// rank `dim − 1` where `g_{p₁}` lies in the range of a singular `g_x`.
/// That is two conditions, `ψ = (ψ₁, ψ₂)`, the bottom row of
/// `[M w₀; V₀ᵀ 0][Q; ψ] = [0; I₂]` with two columns in `V₀`. As `g = 0`
/// alone has rank `dim − 1` there, `z` gains a last unknown `β` and the
/// first equations become `g + β w₀ = 0`. On a curve of branch points
/// `β = 0`; a point that needs `β ≠ 0` ends it with [`CurveEnd::Unfolded`].
///
/// The seed is first corrected with `p₂` held. `ds > 0` then follows the
/// curve the way `p₁` increases at the seed, or the way `p₂` increases
/// where the tangent has no `u₁` component; `ds < 0` goes the other way. A
/// corrected seed outside the box is the curve alone, ending with
/// [`CurveEnd::Range`].
///
/// At every accepted point each test function of the curve's kind is
/// evaluated (see the table in `curve/codim2.rs`); where one changes sign
/// between two points it is bisected in arclength, and the point it
/// vanishes at is inserted into [`Curve::points`] and listed in
/// [`Curve::special`]. A sign change through a pole, where the test grows
/// instead of shrinking, is dropped. The points do not change the steps,
/// so two zeros of one test within one step cancel and neither is found:
/// `ds_max` below the distance between two such points finds both.
///
/// [`CurveError::NoEffect`] needs `g_{p₂}` and `ψ_{p₂}` to vanish at the
/// seed. They count as zero when the first-order move of the other unknowns
/// they cause over the whole height of the box, `R⁻¹(g_{u₂}; ψ_{u₂})` with
/// `R` the Jacobian in those unknowns, is within Newton's tolerance
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
        CurveKind::Fold | CurveKind::BranchPoint => true,
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
    if sys.unfolded(&z) {
        return Err(CurveError::NotABifurcation);
    }
    sys.renew_borders(&z);
    let mut up = vec![0.0; z.len()];
    up[n + 1] = 1.0;
    let tau = tangent(&sys, &z, &up).ok_or(CurveError::Singular)?;
    let across = norm_inf(&tau[..=n]).max(norm_inf(&tau[n + 2..]));
    if across <= TOL * (1.0 + norm_inf(&z)) * tau[n + 1].abs() {
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
    if !z[n..n + 2].iter().all(|u| (0.0..=1.0).contains(u)) {
        return Ok(sys.curve(vec![start], Vec::new(), Vec::new(), CurveEnd::Range));
    }
    let h = opts.ds.abs().min(opts.ds_max).max(opts.ds_min);
    Ok(sys.follow(start, z, tau, h, opts))
}

/// `G(z) = (g(x, p) + β w₀; ψ(x, p))`, `dim + c` equations in
/// `z = (x, u₁, u₂, β)`, where `c` is the dimension of the kernel of `M` on
/// the curve and `β` has `c − 1` entries: none, or one on a branch-point
/// curve. The borders `V₀`, `w₀` stay fixed while a point is corrected.
struct Defining<'a> {
    power: Power<'a>,
    map: bool,
    kind: CurveKind,
    params: Vec<f64>,
    free: [usize; 2],
    edges: [(f64, f64); 2],
    /// The `c` columns of `V₀`, each of the length of a row of `M`.
    v0: Vec<Vec<f64>>,
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

    /// A branch-point curve has the square part of a fold's `M` and one
    /// more column, `g_{u₁}`.
    fn branch(&self) -> bool {
        self.kind == CurveKind::BranchPoint
    }

    /// The kind whose `M` is the square part of this one's.
    fn base(&self) -> CurveKind {
        if self.branch() {
            CurveKind::Fold
        } else {
            self.kind
        }
    }

    /// Rows of `M`, and its columns but for `g_{u₁}` on a branch-point curve.
    fn size(&self) -> usize {
        let n = self.dim();
        match self.base() {
            CurveKind::Hopf | CurveKind::NeimarkSacker => n * (n - 1) / 2,
            _ => n,
        }
    }

    /// `c`: `M` has `size + c − 1` columns and a kernel of dimension `c` on
    /// the curve.
    fn corank(&self) -> usize {
        1 + usize::from(self.branch())
    }

    /// `z` at `x` and `params`, with `β = 0`.
    fn scaled(&self, x: &[f64]) -> Vec<f64> {
        let u = (0..2).map(|k| {
            let (lo, hi) = self.edges[k];
            (self.params[self.free[k]] - lo) / (hi - lo)
        });
        let beta = vec![0.0; self.corank() - 1];
        x.iter().copied().chain(u).chain(beta).collect()
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

    /// `g_p` along `params[free]`.
    fn param_column(&self, x: &[f64], p: &[f64], free: usize) -> Vec<f64> {
        let mut column = vec![0.0; x.len()];
        self.power.param_jacobian(0.0, x, p, free, &mut column);
        column
    }

    /// `wᵀ g_{p₁ p_k}` by a central difference of `wᵀ g_{p₁}` in `p_k`, with
    /// a step of `2⁻¹⁷ · max(|p_k|, 1)`: arithmetic only, as no libm function
    /// may decide which points are kept.
    fn w_second_param_derivative(&self, x: &[f64], p: &[f64], w: &[f64], k: usize) -> f64 {
        let free = self.free[k];
        let h = p[free].abs().max(1.0) / 131_072.0;
        let (up, down) = (p[free] + h, p[free] - h);
        let mut moved = p.to_vec();
        let mut at = |v: f64| {
            moved[free] = v;
            dot(w, &self.param_column(x, &moved, self.free[0]))
        };
        (at(up) - at(down)) / (up - down)
    }

    /// The first `size` columns of `M`, from `A`.
    fn square(&self, a: &[f64]) -> Vec<f64> {
        let n = self.dim();
        match self.base() {
            CurveKind::Fold if self.map => shifted(a, n, -1.0),
            CurveKind::Fold | CurveKind::BranchPoint => a.to_vec(),
            CurveKind::Flip => shifted(a, n, 1.0),
            CurveKind::Hopf => {
                let eye = identity(n);
                sum(&cross(a, &eye, n), &cross(&eye, a, n))
            }
            CurveKind::NeimarkSacker => shifted(&cross(a, a, n), self.size(), -1.0),
        }
    }

    /// The derivative of those columns where `A` moves by `da`.
    fn square_derivative(&self, a: &[f64], da: &[f64]) -> Vec<f64> {
        let n = self.dim();
        match self.base() {
            CurveKind::Fold | CurveKind::BranchPoint | CurveKind::Flip => da.to_vec(),
            CurveKind::Hopf => {
                let eye = identity(n);
                sum(&cross(da, &eye, n), &cross(&eye, da, n))
            }
            CurveKind::NeimarkSacker => sum(&cross(da, a, n), &cross(a, da, n)),
        }
    }

    /// `M` from `A`: the square part, followed on a branch-point curve by the
    /// column `g_{u₁}`, from `g_{p₁}` asked for only then.
    fn singular(&self, a: &[f64], g_p1: impl FnOnce() -> Vec<f64>) -> Vec<f64> {
        let square = self.square(a);
        if !self.branch() {
            return square;
        }
        let (lo, hi) = self.edges[0];
        square
            .chunks(self.dim())
            .zip(g_p1())
            .flat_map(|(row, g)| row.iter().copied().chain([g * (hi - lo)]))
            .collect()
    }

    /// The solutions of the bordered systems of `M` and `Mᵀ`.
    fn bordered_solve(&self, m: &[f64]) -> Option<Bordered> {
        let (rows, c) = (self.w0.len(), self.v0.len());
        let (cols, k) = (rows + c - 1, rows + c);
        let mut b = vec![0.0; k * k];
        for i in 0..rows {
            b[i * k..i * k + cols].copy_from_slice(&m[i * cols..(i + 1) * cols]);
            b[i * k + cols] = self.w0[i];
        }
        for (i, v) in (rows..k).zip(&self.v0) {
            b[i * k..i * k + cols].copy_from_slice(v);
        }
        let bt: Vec<f64> = (0..k * k).map(|i| b[i % k * k + i / k]).collect();
        let solved = (rows..k)
            .map(|i| lu_solve(&b, &unit_vector(k, i)))
            .collect::<Option<Vec<_>>>()?;
        let w = lu_solve(&bt, &unit_vector(k, k - 1))?;
        Some(Bordered {
            q: solved.iter().map(|s| s[..cols].to_vec()).collect(),
            psi: solved.iter().map(|s| s[cols]).collect(),
            w: w[..rows].to_vec(),
        })
    }

    /// Borders for a seed, where no `Q` is known yet: approximate null
    /// vectors `q` of the square part `S` of `M` and `w₀` of `Sᵀ`. On a
    /// branch-point curve `V₀ = [(q, 0), e_{u₁}]`, whose `V₀ᵀ` is invertible
    /// on the kernel of `M`, spanned by `(q, 0)` and some `(v, 1)`.
    fn first_borders(&mut self, z: &[f64]) -> Option<()> {
        let (n, size) = (self.dim(), self.size());
        let s = self.square(&self.jacobian(&z[..n], &self.params_at(z)));
        let transposed: Vec<f64> = (0..size * size)
            .map(|i| s[i % size * size + i / size])
            .collect();
        let cols = size + self.corank() - 1;
        let mut q = unit(null_vector(&s, size)?)?;
        q.resize(cols, 0.0);
        self.v0 = std::iter::once(q)
            .chain((size..cols).map(|i| unit_vector(cols, i)))
            .collect();
        self.w0 = unit(null_vector(&transposed, size)?)?;
        Some(())
    }

    /// The orthonormalized `Q` and the normalized `w` at an accepted point
    /// become the borders.
    fn renew_borders(&mut self, z: &[f64]) {
        let n = self.dim();
        let (x, p) = (&z[..n], self.params_at(z));
        let m = self.singular(&self.jacobian(x, &p), || {
            self.param_column(x, &p, self.free[0])
        });
        let found = self.bordered_solve(&m);
        if let Some((Some(v), Some(w))) = found.map(|b| (orthonormal(b.q), unit(b.w))) {
            (self.v0, self.w0) = (v, w);
        }
    }

    /// `|β|`, the size of the residual `g` once `β` is dropped, is beyond
    /// Newton's tolerance.
    fn unfolded(&self, z: &[f64]) -> bool {
        let tol = TOL * (1.0 + norm_inf(z));
        z[self.dim() + 2..].iter().any(|beta| beta.abs() > tol)
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
            nature: nature(self.base(), &eigenvalues),
            eigenvalues,
        }
    }

    fn curve(
        &self,
        points: Vec<CurvePoint>,
        special: Vec<Codim2Point>,
        ticks: Vec<ResonanceTick>,
        end: CurveEnd,
    ) -> Curve {
        Curve {
            kind: self.kind,
            period: self.power.k,
            points,
            special,
            ticks,
            end,
        }
    }

    /// The step loop of a branch in the box `[0, 1]²` of `(u₁, u₂)`, which
    /// also stops where the curve closes. The codimension-2 points between
    /// two accepted points are located with the borders of the first, and
    /// they do not change the steps.
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
        let detectors = Detectors::of(&self);
        let mut site = Site::new(z.clone(), start.clone());
        let mut tests = detectors.tests(&self, &site);
        let mut points = vec![start];
        let mut special = Vec::new();
        let mut ticks = Vec::new();
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
            if self.unfolded(&z_new) {
                break CurveEnd::Unfolded;
            }
            let len = dist(&z, &z_new);
            let next = Site::new(z_new.clone(), self.point(last.s + len, &z_new, t_new));
            let next_tests = detectors.tests(&self, &next);
            let found = detectors.locate(&self, (&site, &tests), (&next, &next_tests));
            let inserted = found
                .iter()
                .filter(|f| matches!(f.mark, Mark::Point(_)))
                .count();
            if points.len() + 1 + inserted > opts.max_points {
                break CurveEnd::MaxPoints;
            }
            let from = last.s;
            for f in found {
                let kind = match f.mark {
                    Mark::Point(kind) => kind,
                    Mark::Tick(q) => {
                        ticks.push(ResonanceTick {
                            q,
                            p: f.site.point.p,
                        });
                        continue;
                    }
                };
                special.push(Codim2Point {
                    kind,
                    index: points.len(),
                    p: f.site.point.p,
                    x: f.site.point.x.clone(),
                    bracket: f.bracket,
                    normal_form: codim2::normal_form(&self, kind, &f.site),
                    angle: codim2::angle(&self, &f.site),
                    positive_toward: f.positive_toward,
                });
                points.push(CurvePoint {
                    s: from + dist(&z, &f.site.z),
                    ..f.site.point
                });
            }
            self.renew_borders(&z_new);
            points.push(next.point.clone());
            (site, tests) = (next, next_tests);
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
        self.curve(points, special, ticks, end)
    }
}

/// `[M w₀; V₀ᵀ 0][Q; ψ] = [0; I_c]` and `[Mᵀ V₀; w₀ᵀ 0][w; h] = [0; 1]`
/// solved at one point.
struct Bordered {
    /// The `c` columns `qᵢ` of `Q`.
    q: Vec<Vec<f64>>,
    /// The `c` values `ψᵢ`, zero on the curve.
    psi: Vec<f64>,
    w: Vec<f64>,
}

impl Underdetermined for Defining<'_> {
    /// Row `i < dim` is `[g_x | g_{p₁}·(hi₁ − lo₁) | g_{p₂}·(hi₂ − lo₂) | w₀]`,
    /// `w₀` with `β` only, and the last `c` rows are `ψᵢ_z`, with
    /// `ψᵢ' = −wᵀ M' qᵢ` along each unknown and `0` along `β`. For a fold,
    /// branch point or flip the symmetry of the second derivative gives all
    /// of `wᵀ (∂_x A) q` as `wᵀ D(q, 0)` from one derivative of the Jacobian;
    /// it also gives the branch point's column `∂_x g_{u₁}` from `∂_{p₁} A`.
    fn linear(&self, z: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.dim();
        let (rows, cols) = (z.len() - 1, z.len());
        let (x, p) = (&z[..n], self.params_at(z));
        let a = self.jacobian(x, &p);
        let gp = self.free.map(|free| self.param_column(x, &p, free));
        let Some(Bordered { q, psi, w }) =
            self.bordered_solve(&self.singular(&a, || gp[0].clone()))
        else {
            return (vec![f64::NAN; rows], vec![f64::NAN; rows * cols]);
        };
        let width = self.edges.map(|(lo, hi)| hi - lo);
        let derivative = |dx: &[f64], dp: &[f64]| {
            let mut da = vec![0.0; n * n];
            self.power.jacobian_derivative(0.0, x, &p, dx, dp, &mut da);
            da
        };
        let (still_x, still_p) = (vec![0.0; n], vec![0.0; p.len()]);
        let a_p = self
            .free
            .map(|free| derivative(&still_x, &unit_vector(p.len(), free)));
        let w_column = |d: &[f64], j: usize| (0..n).map(|i| w[i] * d[i * n + j]).sum::<f64>();
        let branch = self.branch();
        let w_dg_u1 =
            branch.then(|| {
                (0..n)
                    .map(|j| width[0] * w_column(&a_p[0], j))
                    .chain((0..2).map(|k| {
                        width[0] * width[k] * self.w_second_param_derivative(x, &p, &w, k)
                    }))
                    .collect::<Vec<f64>>()
            });
        let size = self.size();
        let m_p = a_p.each_ref().map(|da| self.square_derivative(&a, da));
        let mut psi_z = Vec::with_capacity(q.len() * cols);
        for qi in &q {
            let qa = &qi[..size];
            let along = |dm: &[f64]| -dot(&w, &matvec(dm, qa, size));
            let mut row: Vec<f64> = match self.base() {
                CurveKind::Hopf | CurveKind::NeimarkSacker => (0..n)
                    .map(|j| {
                        along(
                            &self.square_derivative(&a, &derivative(&unit_vector(n, j), &still_p)),
                        )
                    })
                    .collect(),
                _ => {
                    let d = derivative(qa, &still_p);
                    (0..n).map(|j| -w_column(&d, j)).collect()
                }
            };
            row.extend((0..2).map(|k| along(&m_p[k]) * width[k]));
            if let Some(d_u1) = &w_dg_u1 {
                let qu = qi[size];
                row.iter_mut().zip(d_u1).for_each(|(r, d)| *r -= qu * d);
                row.push(0.0);
            }
            psi_z.extend(row);
        }
        let mut g = vec![0.0; n];
        self.power.eval(0.0, x, &p, &mut g);
        let beta = branch.then(|| z[n + 2]);
        let mut dg = Vec::with_capacity(rows * cols);
        for i in 0..n {
            dg.extend_from_slice(&a[i * n..(i + 1) * n]);
            if self.map {
                g[i] -= x[i];
                dg[i * cols + i] -= 1.0;
            }
            dg.extend((0..2).map(|k| gp[k][i] * width[k]));
            if let Some(beta) = beta {
                g[i] += beta * self.w0[i];
                dg.push(self.w0[i]);
            }
        }
        dg.extend(psi_z);
        g.extend(psi);
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

/// The pair `(i, j)`, `i < j`, of eigenvalues that meets the curve's
/// condition best: the smallest `|λᵢ + λⱼ|` on a Hopf curve, the smallest
/// `|μᵢμⱼ − 1|` on a Neimark–Sacker curve. `None` on other curves and with
/// fewer than two eigenvalues.
fn critical_pair(kind: CurveKind, eigenvalues: &[(f64, f64)]) -> Option<(usize, usize)> {
    let distance: fn((f64, f64), (f64, f64)) -> f64 = match kind {
        CurveKind::Fold | CurveKind::BranchPoint | CurveKind::Flip => return None,
        CurveKind::Hopf => |a, b| (a.0 + b.0) * (a.0 + b.0) + (a.1 + b.1) * (a.1 + b.1),
        CurveKind::NeimarkSacker => |a, b| {
            let (re, im) = (a.0 * b.0 - a.1 * b.1 - 1.0, a.0 * b.1 + a.1 * b.0);
            re * re + im * im
        },
    };
    let len = eigenvalues.len();
    let d = |(i, j): (usize, usize)| distance(eigenvalues[i], eigenvalues[j]);
    (0..len)
        .flat_map(|i| (i + 1..len).map(move |j| (i, j)))
        .min_by(|&a, &b| d(a).total_cmp(&d(b)))
}

/// A bifurcation or a neutral saddle, from the [`critical_pair`]. Hopf: it
/// is a Hopf pair with `ω = √κ` where `κ = Re(λᵢλⱼ) > 0`. Neimark–Sacker:
/// it is critical where `c = Re(μᵢ + μⱼ)/2` has `|c| < 1`.
fn nature(kind: CurveKind, eigenvalues: &[(f64, f64)]) -> Nature {
    let Some((i, j)) = critical_pair(kind, eigenvalues) else {
        return Nature::Bifurcation { omega: None };
    };
    let (a, b) = (eigenvalues[i], eigenvalues[j]);
    let omega = if kind == CurveKind::Hopf {
        let kappa = a.0 * b.0 - a.1 * b.1;
        (kappa > 0.0).then(|| kappa.sqrt())
    } else {
        (((a.0 + b.0) / 2.0).abs() < 1.0).then(|| a.1.abs().atan2(a.0))
    };
    omega.map_or(Nature::Neutral, |w| Nature::Bifurcation { omega: Some(w) })
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

fn unit_vector(len: usize, i: usize) -> Vec<f64> {
    let mut e = vec![0.0; len];
    e[i] = 1.0;
    e
}

/// Gram–Schmidt on `vectors` in order; `None` if one is left zero.
fn orthonormal(vectors: Vec<Vec<f64>>) -> Option<Vec<Vec<f64>>> {
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(vectors.len());
    for v in vectors {
        let rest = basis.iter().fold(v, |r, b| {
            let along = dot(b, &r);
            r.iter().zip(b).map(|(r, b)| r - along * b).collect()
        });
        basis.push(unit(rest)?);
    }
    Some(basis)
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
        // `b` enters nonlinearly, and with `a`, so that `g_{p₁p₁}` and
        // `g_{p₁p₂}` do not vanish.
        let branch_flow =
            system("x' = a*x - y + x*z^2 + a*b*y^2\ny' = x + b^2*y - x*y*z\nz' = -z + a*x^2 + y*z");
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
            (&branch_flow, CurveKind::BranchPoint, 1),
            (&map, CurveKind::BranchPoint, 2),
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
            for i in x.len()..rows {
                assert!(g[i].abs() > 1e-3, "{kind:?}: ψ = {}", g[i]);
                let psi_z = &dg[i * cols..i * cols + x.len() + 2];
                assert!(
                    psi_z.iter().all(|v| v.abs() > 1e-4),
                    "{kind:?}: ψ_z = {psi_z:?}"
                );
            }
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
