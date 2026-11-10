//! The codimension-2 points of a curve: one row per kind of curve, kind of
//! system and kind of point, with a test function that changes sign at the
//! point and the points where it applies. The weak resonances of a
//! Neimark–Sacker curve are rows of the same table.

use std::f64::consts::PI;

use super::{
    critical_pair, shifted, Codim2Kind, CurveKind, CurvePoint, Defining, Nature, NormalForm,
};
use crate::continuation::{cos, diff, dist, dot, locate, norm2, OnCurve};
use crate::linalg::lu_det;
use crate::normal_form::{self, first_lyapunov, Taylor};
use Codim2Kind::{
    Bautin, BogdanovTakens, Chenciner, Cusp, FoldFlip, GeneralizedFlip, HopfHopf, Resonance1,
    Resonance2, Resonance3, Resonance4, ZeroHopf,
};
use CurveKind::{BranchPoint, Flip, Fold, Hopf, NeimarkSacker};

/// A point of a curve as the bisection and the test functions read it: the
/// unknown `z` and the point.
#[derive(Clone)]
pub(super) struct Site {
    pub(super) z: Vec<f64>,
    pub(super) point: CurvePoint,
}

impl Site {
    pub(super) fn new(z: Vec<f64>, point: CurvePoint) -> Self {
        Site { z, point }
    }

    fn eigenvalues(&self) -> &[(f64, f64)] {
        &self.point.eigenvalues
    }
}

impl OnCurve for Site {
    fn z(&self) -> Vec<f64> {
        self.z.clone()
    }
    fn tangent(&self) -> &[f64] {
        &self.point.tangent
    }
    /// `(u₁, u₂)`.
    fn parameters(&self) -> &[f64] {
        let n = self.point.x.len();
        &self.z[n..n + 2]
    }
}

type Test = fn(&Defining, &Site) -> Option<f64>;

/// What a row locates.
#[derive(Clone, Copy)]
pub(super) enum Mark {
    /// A codimension-2 point, inserted into the curve's points.
    Point(Codim2Kind),
    /// The weak resonance `1:q` of a Neimark–Sacker curve, marked on the
    /// curve but not among its points.
    Tick(usize),
}

/// One kind of codimension-2 point, or one weak resonance, on one kind of
/// curve.
struct Row {
    curve: CurveKind,
    map: bool,
    mark: Mark,
    /// Zero at the point, continuous between accepted points.
    test: Test,
    /// Where the test is read; a sign change counts only between two points
    /// where it applies.
    applies: fn(&CurvePoint) -> bool,
}

impl Row {
    fn value(&self, sys: &Defining, at: &Site) -> Option<f64> {
        (self.applies)(&at.point)
            .then(|| (self.test)(sys, at))
            .flatten()
    }

    /// The test is a normal-form coefficient whose sign the point's
    /// sentence reads.
    fn signed(&self) -> bool {
        matches!(self.mark, Mark::Point(Bautin | Chenciner | GeneralizedFlip))
    }
}

/// A Hopf or Neimark–Sacker point, not a neutral saddle. On a neutral
/// stretch only the points where it begins are reported: Bogdanov–Takens
/// (`κ = 0`) and the 1:1 and 1:2 resonances (`c = ±1`), whose tests change
/// sign there and nowhere else on the stretch. Every other point of a Hopf
/// or Neimark–Sacker curve needs a true bifurcation at both ends of the
/// step: the Bautin and Chenciner coefficients and the eigenvalues outside
/// the critical pair mean nothing at a neutral saddle.
fn critical(pt: &CurvePoint) -> bool {
    pt.nature != Nature::Neutral
}

fn anywhere(_: &CurvePoint) -> bool {
    true
}

const fn row(
    curve: CurveKind,
    map: bool,
    kind: Codim2Kind,
    test: Test,
    applies: fn(&CurvePoint) -> bool,
) -> Row {
    Row {
        curve,
        map,
        mark: Mark::Point(kind),
        test,
        applies,
    }
}

/// The weak resonance `1:Q`, where `cos θ = cos(2π/Q)` between two true
/// Neimark–Sacker points.
const fn tick<const Q: usize>() -> Row {
    Row {
        curve: NeimarkSacker,
        map: MAP,
        mark: Mark::Tick(Q),
        test: weak::<Q>,
        applies: critical,
    }
}

const FLOW: bool = false;
const MAP: bool = true;

/// The test functions per curve (brief §5 of the special-bifurcations plan).
///
/// Branch-point curves have the rectangular `M = [g_x | g_{u₁}]`, whose
/// bordered systems give a kernel of dimension two, not the null vectors
/// `q` and `w` of the square `g_x`. So the tests that read `wᵀq` are left
/// out there: Bogdanov–Takens on a flow and the 1:1 resonance on a map. So
/// is the cusp, as a transcritical or pitchfork has no fold coefficient
/// that vanishes generically. What remains reads only the eigenvalues: on a
/// map the fold-flip, where the other multiplier passes `−1`
/// (`det(A + I) = 0`), and on a flow the zero-Hopf, where a complex pair of
/// the other eigenvalues crosses the imaginary axis, as on a fold curve.
const ROWS: &[Row] = &[
    row(Fold, FLOW, BogdanovTakens, null_vectors_meet, anywhere),
    row(Fold, FLOW, Cusp, fold_coefficient, anywhere),
    row(Fold, FLOW, ZeroHopf, hopf_test, anywhere),
    row(Hopf, FLOW, BogdanovTakens, kappa, anywhere),
    row(Hopf, FLOW, ZeroHopf, product_outside_pair, critical),
    row(Hopf, FLOW, Bautin, lyapunov, critical),
    row(Hopf, FLOW, HopfHopf, hopf_test_outside_pair, critical),
    row(BranchPoint, FLOW, ZeroHopf, hopf_test, anywhere),
    row(Fold, MAP, Resonance1, null_vectors_meet, anywhere),
    row(Fold, MAP, Cusp, fold_coefficient, anywhere),
    row(Fold, MAP, FoldFlip, det_plus_identity, anywhere),
    row(Flip, MAP, Resonance2, null_vectors_meet, anywhere),
    row(Flip, MAP, FoldFlip, det_minus_identity, anywhere),
    row(Flip, MAP, GeneralizedFlip, flip_coefficient, anywhere),
    row(NeimarkSacker, MAP, Resonance1, cosine_minus_one, anywhere),
    row(NeimarkSacker, MAP, Resonance2, cosine_plus_one, anywhere),
    row(NeimarkSacker, MAP, Resonance3, cosine_plus_half, critical),
    row(NeimarkSacker, MAP, Resonance4, cosine, critical),
    row(NeimarkSacker, MAP, Chenciner, ns_coefficient, critical),
    tick::<5>(),
    tick::<6>(),
    tick::<7>(),
    tick::<8>(),
    tick::<9>(),
    row(BranchPoint, MAP, FoldFlip, det_plus_identity, anywhere),
];

/// How far a test must shrink during the bisection, from its value at the
/// nearer end of the step, for its sign change to be a zero. A test that
/// crosses a pole grows instead (ℓ₁ at a Bogdanov–Takens point, the
/// Neimark–Sacker coefficient at a 1:3 resonance), and rounding noise in a
/// test that vanishes identically changes sign without shrinking that far:
/// ℓ₁ of `ẍ + ω²x + x³ = 0` stays within a decade of `1e-31`. The zeros of
/// the reference systems shrink to between `1e-12` and `3e-8` of it, the
/// most for ℓ₁, from central differences.
const SHRINK: f64 = 1e-4;

/// The rows of one curve.
pub(super) struct Detectors(Vec<&'static Row>);

/// A codimension-2 point or a weak resonance located between two accepted
/// points.
pub(super) struct Found {
    pub(super) mark: Mark,
    pub(super) site: Site,
    pub(super) bracket: f64,
    /// For a test that is a coefficient, `(Δp₁, Δp₂)` from the point to the
    /// end of the step where the test is positive.
    pub(super) positive_toward: Option<(f64, f64)>,
}

impl Detectors {
    pub(super) fn of(sys: &Defining) -> Self {
        Detectors(
            ROWS.iter()
                .filter(|r| r.curve == sys.kind && r.map == sys.map)
                .collect(),
        )
    }

    pub(super) fn tests(&self, sys: &Defining, at: &Site) -> Vec<Option<f64>> {
        self.0.iter().map(|r| r.value(sys, at)).collect()
    }

    /// The points between `a` and `b` where a test changes sign, by
    /// bisection along the chord from `a` to `b`, in order of distance
    /// from `a`. A point counts where the bracket closed and the test
    /// shrank to [`SHRINK`] times its value at the nearer end.
    pub(super) fn locate(
        &self,
        sys: &Defining,
        (a, tests_a): (&Site, &[Option<f64>]),
        (b, tests_b): (&Site, &[Option<f64>]),
    ) -> Vec<Found> {
        let chord = diff(&b.z, &a.z);
        let len = norm2(&chord);
        let dir: Vec<f64> = chord.iter().map(|c| c / len).collect();
        let mut found: Vec<Found> = self
            .0
            .iter()
            .zip(tests_a.iter().zip(tests_b))
            .filter_map(|(row, (ta, tb))| {
                let (ta, tb) = ((*ta)?, (*tb)?);
                if !(ta < 0.0 && tb > 0.0 || ta > 0.0 && tb < 0.0) {
                    return None;
                }
                let bisection = locate(
                    sys,
                    (a, ta),
                    b,
                    &dir,
                    |at| row.value(sys, at),
                    |z, t| Site::new(z.to_vec(), sys.point(0.0, z, t)),
                );
                let shrank = bisection.test <= SHRINK * ta.abs().min(tb.abs());
                let at = bisection.point.point.p;
                let toward = |end: &Site| (end.point.p.0 - at.0, end.point.p.1 - at.1);
                (bisection.converged && shrank).then(|| Found {
                    mark: row.mark,
                    positive_toward: row.signed().then(|| toward(if tb > 0.0 { b } else { a })),
                    site: bisection.point,
                    bracket: bisection.width,
                })
            })
            .collect();
        found.sort_by(|f, g| dist(&a.z, &f.site.z).total_cmp(&dist(&a.z, &g.site.z)));
        found
    }
}

/// The normal form of a point of `kind` located at `at`: Bogdanov–Takens on
/// a fold or Hopf curve of a flow, the cusp on a fold curve.
pub(super) fn normal_form(sys: &Defining, kind: Codim2Kind, at: &Site) -> Option<NormalForm> {
    let p = sys.params_at(&at.z);
    let taylor = Taylor::new(&sys.power, &at.point.x, &p);
    match kind {
        BogdanovTakens => {
            let (a, b) = normal_form::bogdanov_takens(&taylor)?;
            Some(NormalForm::BogdanovTakens { a, b })
        }
        Cusp => {
            let (q, w) = null_vectors(sys, at)?;
            let c = normal_form::cusp(&taylor, &q, &w)?;
            Some(NormalForm::Cusp { c })
        }
        _ => None,
    }
}

/// On a Neimark–Sacker curve, `θ = acos c` of the critical pair, with `c`
/// clamped to `[−1, 1]`.
pub(super) fn angle(sys: &Defining, at: &Site) -> Option<f64> {
    if sys.kind != NeimarkSacker {
        return None;
    }
    Some(cosine(sys, at)?.clamp(-1.0, 1.0).acos())
}

/// `A` at `at`.
fn jacobian(sys: &Defining, at: &Site) -> Vec<f64> {
    sys.jacobian(&at.point.x, &sys.params_at(&at.z))
}

/// The null vectors `q` of `M` and `w` of `Mᵀ` from the bordered systems,
/// on a fold or flip curve, where `M` is square.
fn null_vectors(sys: &Defining, at: &Site) -> Option<(Vec<f64>, Vec<f64>)> {
    let solved = sys.bordered_solve(&sys.square(&jacobian(sys, at)))?;
    Some((solved.q.into_iter().next()?, solved.w))
}

/// `wᵀq / (‖w‖‖q‖)`, zero where the kernel of `M` turns into a Jordan
/// block: a Bogdanov–Takens point (`M = A`), a 1:1 resonance (`A − I`), a
/// 1:2 resonance (`A + I`). The borders carried from the last accepted
/// point keep `q` and `w` oriented, so the sign is continuous.
fn null_vectors_meet(sys: &Defining, at: &Site) -> Option<f64> {
    let (q, w) = null_vectors(sys, at)?;
    Some(dot(&w, &q) / (norm2(&w) * norm2(&q)))
}

fn fold_coefficient(sys: &Defining, at: &Site) -> Option<f64> {
    let (q, w) = null_vectors(sys, at)?;
    let p = sys.params_at(&at.z);
    normal_form::fold(&Taylor::new(&sys.power, &at.point.x, &p), &q, &w)
}

fn flip_coefficient(sys: &Defining, at: &Site) -> Option<f64> {
    let (q, w) = null_vectors(sys, at)?;
    let p = sys.params_at(&at.z);
    normal_form::flip(&Taylor::new(&sys.power, &at.point.x, &p), &q, &w)
}

fn det_plus_identity(sys: &Defining, at: &Site) -> Option<f64> {
    Some(lu_det(&shifted(&jacobian(sys, at), at.point.x.len(), 1.0)))
}

fn det_minus_identity(sys: &Defining, at: &Site) -> Option<f64> {
    Some(lu_det(&shifted(&jacobian(sys, at), at.point.x.len(), -1.0)))
}

/// The largest real part among the complex pairs, the Hopf test of a
/// branch: on a fold curve, whose zero eigenvalue is real, it is the test
/// of the other eigenvalues.
fn hopf_test(_: &Defining, at: &Site) -> Option<f64> {
    upper(at.eigenvalues().iter().copied()).reduce(f64::max)
}

/// `κ = Re(λᵢλⱼ)` of the critical pair: positive at a Hopf point, negative
/// at a neutral saddle.
fn kappa(_: &Defining, at: &Site) -> Option<f64> {
    let ev = at.eigenvalues();
    let (i, j) = critical_pair(Hopf, ev)?;
    Some(ev[i].0 * ev[j].0 - ev[i].1 * ev[j].1)
}

/// The eigenvalues but the critical pair.
fn outside_pair(at: &Site) -> Option<impl Iterator<Item = (f64, f64)> + '_> {
    let (i, j) = critical_pair(Hopf, at.eigenvalues())?;
    Some(
        at.eigenvalues()
            .iter()
            .enumerate()
            .filter(move |&(k, _)| k != i && k != j)
            .map(|(_, &l)| l),
    )
}

/// The product of the eigenvalues outside the critical pair, real as they
/// come in conjugate pairs; `1` where there are none.
fn product_outside_pair(_: &Defining, at: &Site) -> Option<f64> {
    let product = outside_pair(at)?.fold((1.0, 0.0), |(re, im), (a, b)| {
        (re * a - im * b, re * b + im * a)
    });
    Some(product.0)
}

/// The Hopf test of the eigenvalues outside the critical pair.
fn hopf_test_outside_pair(_: &Defining, at: &Site) -> Option<f64> {
    upper(outside_pair(at)?).reduce(f64::max)
}

fn upper(eigenvalues: impl Iterator<Item = (f64, f64)>) -> impl Iterator<Item = f64> {
    eigenvalues.filter(|&(_, im)| im > 0.0).map(|(re, _)| re)
}

/// `ℓ₁` of the critical eigenvalue `iω`, `ω > 0`.
fn lyapunov(sys: &Defining, at: &Site) -> Option<f64> {
    let ev = at.eigenvalues();
    let (i, j) = critical_pair(Hopf, ev)?;
    let lambda = if ev[i].1 > 0.0 { ev[i] } else { ev[j] };
    first_lyapunov(&sys.power, &sys.params_at(&at.z), &at.point.x, ev, lambda)
}

/// `c = Re(μᵢ + μⱼ)/2` of the critical pair, `cos θ` at a Neimark–Sacker
/// point: `1`, `−1`, `−½` and `0` at the 1:1 to 1:4 resonances.
fn cosine(_: &Defining, at: &Site) -> Option<f64> {
    let ev = at.eigenvalues();
    let (i, j) = critical_pair(NeimarkSacker, ev)?;
    Some((ev[i].0 + ev[j].0) / 2.0)
}

fn cosine_minus_one(sys: &Defining, at: &Site) -> Option<f64> {
    Some(cosine(sys, at)? - 1.0)
}

fn cosine_plus_one(sys: &Defining, at: &Site) -> Option<f64> {
    Some(cosine(sys, at)? + 1.0)
}

fn cosine_plus_half(sys: &Defining, at: &Site) -> Option<f64> {
    Some(cosine(sys, at)? + 0.5)
}

/// `c − cos(2π/Q)`, zero at the weak resonance `1:Q`.
fn weak<const Q: usize>(sys: &Defining, at: &Site) -> Option<f64> {
    Some(cosine(sys, at)? - cos(2.0 * PI / Q as f64))
}

fn ns_coefficient(sys: &Defining, at: &Site) -> Option<f64> {
    let ev = at.eigenvalues();
    let (i, j) = critical_pair(NeimarkSacker, ev)?;
    let k = if ev[i].1 > 0.0 { i } else { j };
    let p = sys.params_at(&at.z);
    normal_form::neimark_sacker(&Taylor::new(&sys.power, &at.point.x, &p), ev, k)
}

#[cfg(test)]
mod tests {
    use super::super::{continue_curve, CurveOptions, CurveSeed};
    use super::*;
    use crate::continuation::{null_vector, Power};
    use crate::normal_form::{first_lyapunov, flip, neimark_sacker};
    use dyn_core::VectorField;
    use dyn_expr::{compile, CompiledSystem, InputFormat};

    fn system(source: &str) -> CompiledSystem {
        compile(source, InputFormat::Text)
            .system
            .unwrap_or_else(|| panic!("{source} compiles"))
    }

    /// Every parameter, the two free ones by name.
    fn params(sys: &CompiledSystem, free: [(&str, f64); 2]) -> (Vec<f64>, (usize, usize)) {
        let index = |name: &str| {
            sys.info
                .params
                .iter()
                .position(|q| q.name == name)
                .unwrap_or_else(|| panic!("{name}"))
        };
        let mut p = vec![0.0; sys.info.params.len()];
        let (i, j) = (index(free[0].0), index(free[1].0));
        (p[i], p[j]) = (free[0].1, free[1].1);
        (p, (i, j))
    }

    /// The coefficient a point's sign is read from, at a point of the curve.
    fn coefficient(sys: &CompiledSystem, kind: CurveKind, params: &[f64], pt: &CurvePoint) -> f64 {
        let power = Power { f: sys, k: 1 };
        let t = Taylor::new(&power, &pt.x, params);
        let ev = &pt.eigenvalues;
        let n = pt.x.len();
        let value = match kind {
            Hopf => {
                let (i, j) = critical_pair(Hopf, ev).expect("a pair");
                let lambda = if ev[i].1 > 0.0 { ev[i] } else { ev[j] };
                first_lyapunov(&power, params, &pt.x, ev, lambda)
            }
            NeimarkSacker => {
                let (i, j) = critical_pair(NeimarkSacker, ev).expect("a pair");
                neimark_sacker(&t, ev, if ev[i].1 > 0.0 { i } else { j })
            }
            _ => {
                let mut a = vec![0.0; n * n];
                power.jacobian(0.0, &pt.x, params, &mut a);
                let m = shifted(&a, n, 1.0);
                let mt: Vec<f64> = (0..n * n).map(|k| m[k % n * n + k / n]).collect();
                flip(
                    &t,
                    &null_vector(&m, n).expect("q"),
                    &null_vector(&mt, n).expect("w"),
                )
            }
        };
        value.expect("a coefficient")
    }

    /// Both halves from `x`, and for each point of `kind` the curve's points
    /// three steps to either side: the coefficient has opposite signs there,
    /// and `positive_toward` points to the positive one.
    fn check(
        source: &str,
        curve: CurveKind,
        point: Codim2Kind,
        free: [(&str, f64); 2],
        x: &[f64],
        ranges: ((f64, f64), (f64, f64)),
        count: usize,
    ) {
        let sys = system(source);
        let (base, free) = params(&sys, free);
        let seed = CurveSeed {
            kind: curve,
            x: x.to_vec(),
            period: 1,
        };
        let mut seen = 0;
        for ds in [0.01, -0.01] {
            let opts = CurveOptions {
                free,
                ranges,
                ds,
                ds_min: 1e-8,
                ds_max: 0.05,
                max_points: 3000,
                max_angle: 0.3,
            };
            let c = continue_curve(&sys, &base, &seed, &opts).expect("the curve starts");
            for sp in c.special.iter().filter(|sp| sp.kind == point) {
                let toward = sp.positive_toward.expect("a direction");
                let side = |i: usize| {
                    let pt = &c.points[i];
                    let mut p = base.clone();
                    (p[free.0], p[free.1]) = pt.p;
                    let v = coefficient(&sys, curve, &p, pt);
                    let along = (pt.p.0 - sp.p.0) * toward.0 + (pt.p.1 - sp.p.1) * toward.1;
                    (v, along)
                };
                let (before, after) = (side(sp.index - 3), side(sp.index + 3));
                assert!(
                    before.0 * after.0 < 0.0,
                    "{point:?} at {:?}: {before:?}, {after:?}",
                    sp.p
                );
                for (v, along) in [before, after] {
                    assert_eq!(
                        v > 0.0,
                        along > 0.0,
                        "{point:?} at {:?}: {v} at {along}",
                        sp.p
                    );
                }
                assert_eq!(sp.normal_form, None);
                seen += 1;
            }
        }
        assert_eq!(seen, count, "{point:?}");
    }

    #[test]
    fn co_oxidation_bautin_points_point_to_positive_l1() {
        check(
            "x' = 2*2.5*(1 - x - y - s)^2 - 2*x^2 - 10*x*y\n\
             y' = k2*(1 - x - y - s) - 0.1*y - 10*x*y\n\
             s' = 0.0675*((1 - x - y - s) - K*s)",
            Hopf,
            Bautin,
            [("k2", 1.05155746), ("K", 0.4)],
            &[0.01635754, 0.52397118, 0.32833662],
            ((0.5, 2.0), (0.1, 2.0)),
            2,
        );
    }

    const PREDATOR_PREY: &str = "x_{n+1} = a*x_n*(1 - x_n) - 3*x_n*y_n/(1 + epsilon*x_n)\n\
                                 y_{n+1} = 3.5*x_n*y_n/(1 + epsilon*x_n)";

    #[test]
    fn predator_prey_chenciner_point_points_to_positive_d() {
        check(
            PREDATOR_PREY,
            NeimarkSacker,
            Chenciner,
            [("epsilon", 1.0), ("a", 5.0)],
            &[0.4, 3.5 * (5.0 * 0.6 - 1.0) / 7.5],
            ((-6.0, 1.45), (0.0, 45.0)),
            1,
        );
    }

    /// F2 = (x, 0) on its flip curve `ε = −3.5 − 1/x`, `a = 1/(1 − x)`.
    #[test]
    fn predator_prey_generalized_flip_points_to_positive_c() {
        let x = -1.0 / 1.7;
        check(
            PREDATOR_PREY,
            Flip,
            GeneralizedFlip,
            [("epsilon", -1.8), ("a", 1.0 / (1.0 - x))],
            &[x, 0.0],
            ((-2.0, -1.5), (0.5, 1.0)),
            1,
        );
    }
}
