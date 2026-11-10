//! The codimension-2 points of a curve: one row per kind of curve, kind of
//! system and kind of point, with a test function that changes sign at the
//! point and the points where it applies.

use super::{critical_pair, shifted, Codim2Kind, CurveKind, CurvePoint, Defining, Nature};
use crate::continuation::{diff, dist, dot, locate, norm2, OnCurve};
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

/// One kind of codimension-2 point on one kind of curve.
struct Row {
    curve: CurveKind,
    map: bool,
    kind: Codim2Kind,
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
        kind,
        test,
        applies,
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

/// A codimension-2 point located between two accepted points.
pub(super) struct Found {
    pub(super) kind: Codim2Kind,
    pub(super) site: Site,
    pub(super) bracket: f64,
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
                (bisection.converged && shrank).then_some(Found {
                    kind: row.kind,
                    site: bisection.point,
                    bracket: bisection.width,
                })
            })
            .collect();
        found.sort_by(|f, g| dist(&a.z, &f.site.z).total_cmp(&dist(&a.z, &g.site.z)));
        found
    }
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

fn ns_coefficient(sys: &Defining, at: &Site) -> Option<f64> {
    let ev = at.eigenvalues();
    let (i, j) = critical_pair(NeimarkSacker, ev)?;
    let k = if ev[i].1 > 0.0 { i } else { j };
    let p = sys.params_at(&at.z);
    normal_form::neimark_sacker(&Taylor::new(&sys.power, &at.point.x, &p), ev, k)
}
