//! Homoclinic orbits of a flow to a saddle with a one-dimensional unstable
//! (or, in reversed time, stable) manifold, located by shooting along that
//! manifold, and their curves in two parameters.
//!
//! The shot starts at `x* ± ε·v`, `v` the unit eigenvector of the
//! one-dimensional eigenvalue `λ` and `l` its unit left eigenvector, and is
//! read in the saddle's own coordinates `x − x* = y·v + w`, with
//! `y = l·(x − x*)/(l·v)` and `w` in the invariant subspace of the other
//! eigenvalues. The orbit leaves, makes its excursion to a largest distance
//! `Dmax = max √(y² + |w|²)`, and comes back; the split function is
//! `y/(RHO_U·Dmax)` where it first enters the ellipsoid
//! `(y/(RHO_U·Dmax))² + (|w|/(RHO·Dmax))² < 1`, and a homoclinic orbit is a
//! zero of it. The entry time is bisected on the dense output, and `Dmax` at
//! the turn of the orbit, so the split is smooth in the parameters.
//!
//! These coordinates matter near a Bogdanov–Takens point, where `v` and the
//! stable eigenvector become parallel: there a Euclidean ball lets orbits in
//! on one side of the stable manifold only, and its root moves with its
//! radius. The ellipsoid is long along `v`, which widens the window of
//! parameters where the orbit comes back without moving the root, since
//! `y ≈ 0` at a root.

use dyn_core::{SystemKind, VectorField};
use dyn_solve::{Method, SolverOptions, StepOutcome, Stepper};

use std::cell::{Cell, RefCell};

use crate::continuation::{
    correct_along, cos, diff, dist, dot, newton, norm2, null_vector, tangent, Underdetermined,
    FAST_NEWTON_ITERS, SLOW_NEWTON_ITERS, STEP_GROWTH,
};
use crate::curve::{correct_fixed, first_edge, unit, CurveOptions};
use crate::linalg::eigenvalues;

/// Distance from the saddle along its one-dimensional eigenvector where the
/// shot starts.
pub const EPSILON: f64 = 1e-6;

/// The ellipsoid's radius across the one-dimensional eigenvector, as a
/// fraction of `Dmax` (see the module documentation). Halving it moves `ν`
/// of the Freire circuit at `β = 0.6` by 4e-7, towards the converged
/// −0.7213093; the window where the split is defined shrinks with it.
pub const RHO: f64 = 0.05;
/// The ellipsoid's radius along the eigenvector, as a fraction of `Dmax`.
/// The root does not depend on it (to 1e-12 from 0.05 to 0.5 on the Freire
/// circuit); the domain of the split grows with it.
pub const RHO_U: f64 = 0.5;

const RTOL: f64 = 1e-11;
const ATOL: f64 = 1e-14;

/// The integration gives up after this many time constants `1/|λ|` of the
/// one-dimensional eigenvalue.
const MAX_TIME_CONSTANTS: f64 = 200.0;
/// Ten times the most steps a returning shot took on the Freire circuit's
/// curve, 4820 at return time 20606 next to its Bogdanov–Takens point.
const MAX_STEPS: usize = 50_000;
/// No shot is tried when the one-dimensional eigenvalue is below this
/// fraction of the largest real part in modulus: with steps limited by the
/// fastest decay, leaving the saddle alone takes about `4/LAMBDA_FLOOR`
/// steps, beyond [`MAX_STEPS`].
const LAMBDA_FLOOR: f64 = 1e-4;
/// A shot has settled on an attractor once its speed is below this fraction
/// of `|λ|·Dmax`; entering the ellipsoid takes at least `|λₛ|·RHO·Dmax`.
const SETTLED: f64 = 1e-8;
/// Integration steps between two calls of the cancel predicate.
const CANCEL_STEPS: usize = 256;

/// A bisected sign change of the split is a root only where the split
/// went to zero.
const ROOT_SPLIT: f64 = 1e-6;
const MAX_ROOT_BISECTION: usize = 100;
/// The split's gradient is taken by forward differences over the step that
/// moves the split by this much, from the gradient at the last point.
const FD_SPLIT: f64 = 1e-5;
/// The first gradient, at the start, uses this step in the scaled
/// parameters.
const FD_FIRST: f64 = 1e-10;
/// See [`HomoclinicEnd::Shrunk`].
const SHRUNK: f64 = 1e-3;

/// An equilibrium with a one-dimensional unstable or stable manifold, and
/// what its eigenvalues say about homoclinic orbits to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Saddle {
    /// The equilibrium.
    pub x: Vec<f64>,
    /// The eigenvalues `(re, im)` of the Jacobian there.
    pub eigenvalues: Vec<(f64, f64)>,
    /// The saddle quantity: the one-dimensional manifold's eigenvalue plus
    /// the real part of the leading eigenvalue on the other side, the one
    /// nearest the imaginary axis.
    pub sigma: f64,
    /// The leading eigenvalues on the side of the manifold of dimension two
    /// or more are a complex pair: a saddle-focus.
    pub focus: bool,
    /// A saddle-focus whose leading pair is weaker than the real eigenvalue
    /// (`σ > 0` when the unstable manifold is the one-dimensional one):
    /// Shilnikov's condition for chaos near a homoclinic orbit.
    pub chaotic: bool,
}

/// Which branch of the one-dimensional manifold: the one leaving the
/// saddle `x*` along `+v` or along `−v`, `v` its unit eigenvector with its
/// largest component positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    /// Along `+v`.
    Plus,
    /// Along `−v`.
    Minus,
}

impl Half {
    fn sign(self) -> f64 {
        match self {
            Half::Plus => 1.0,
            Half::Minus => -1.0,
        }
    }
}

/// A homoclinic orbit located at fixed parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct HomoclinicOrbit {
    /// All the parameters.
    pub params: Vec<f64>,
    /// The saddle the orbit leaves and returns to.
    pub saddle: Saddle,
    /// The one-dimensional manifold is the stable one, followed in reversed
    /// time.
    pub reversed: bool,
    /// The branch of the one-dimensional manifold that returns.
    pub half: Half,
    /// The time from `ε` off the saddle to the entry into the ellipsoid,
    /// positive in either direction of time.
    pub return_time: f64,
}

/// Why no homoclinic orbit was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomoclinicError {
    /// The field depends explicitly on `t`.
    NotAutonomous,
    /// Not a flow, or the same parameter freed twice.
    Unsupported,
    /// No equilibrium near the given state has a one-dimensional unstable or
    /// stable manifold.
    NoSaddle,
    /// No sign change of the split function that goes to zero.
    NotFound,
    /// The cancel predicate turned true.
    Cancelled,
}

/// The saddle's linearization as the shooting uses it.
#[derive(Debug, Clone)]
struct Linear {
    saddle: Saddle,
    reversed: bool,
    /// The one-dimensional manifold's eigenvalue.
    lambda: f64,
    /// Its unit right eigenvector, oriented as asked.
    v: Vec<f64>,
    /// Its unit left eigenvector, with `l·v > 0`.
    l: Vec<f64>,
}

/// What the shot along one half of the manifold found.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shot {
    /// `y/(RHO_U·Dmax)` where the orbit enters the ellipsoid.
    split: f64,
    return_time: f64,
    dmax: f64,
}

struct Cancelled;

/// The equilibrium by Newton's method from `guess`.
fn equilibrium(f: &dyn VectorField, p: &[f64], guess: &[f64]) -> Option<Vec<f64>> {
    let n = f.dim();
    let (x, _) = newton(guess.to_vec(), |x| {
        let mut r = vec![0.0; n];
        let mut a = vec![0.0; n * n];
        f.eval(0.0, x, p, &mut r);
        f.jacobian(0.0, x, p, &mut a);
        (r, a)
    })
    .ok()?;
    Some(x)
}

/// Which side the one-dimensional manifold is on, from the signs of the
/// real parts: `Some(false)` for exactly one eigenvalue with a positive real
/// part, real; `Some(true)` for exactly one with a negative real part, real.
/// With both (a planar saddle) the unstable manifold is taken. Anything
/// non-hyperbolic is `None`.
fn manifold_side(eigenvalues: &[(f64, f64)]) -> Option<(bool, usize)> {
    if eigenvalues.iter().any(|e| e.0 == 0.0 || !e.0.is_finite()) {
        return None;
    }
    let only = |positive: bool| {
        let mut side = eigenvalues
            .iter()
            .enumerate()
            .filter(|(_, e)| (e.0 > 0.0) == positive);
        match (side.next(), side.next()) {
            (Some((i, e)), None) if e.1 == 0.0 => Some(i),
            _ => None,
        }
    };
    only(true)
        .map(|i| (false, i))
        .or_else(|| only(false).map(|i| (true, i)))
}

fn classify(x: Vec<f64>, eigenvalues: Vec<(f64, f64)>, reversed: bool, one: usize) -> Saddle {
    let lambda = eigenvalues[one].0;
    let leading = eigenvalues
        .iter()
        .filter(|e| (e.0 > 0.0) == reversed)
        .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
        .copied()
        .unwrap_or((f64::NAN, 0.0));
    let focus = leading.1 != 0.0;
    Saddle {
        x,
        sigma: lambda + leading.0,
        focus,
        chaotic: focus && leading.0.abs() < lambda.abs(),
        eigenvalues,
    }
}

/// The saddle at `p` by Newton from `guess`, with its one-dimensional
/// manifold on the side `reversed` if given. `orient` picks the sign of `v`:
/// `v·orient > 0`, or by default its largest component positive.
fn linearize(
    f: &dyn VectorField,
    p: &[f64],
    guess: &[f64],
    reversed: Option<bool>,
    orient: Option<&[f64]>,
) -> Option<Linear> {
    let n = f.dim();
    let x = equilibrium(f, p, guess)?;
    let mut a = vec![0.0; n * n];
    f.jacobian(0.0, &x, p, &mut a);
    let eig = eigenvalues(n, &a);
    if eig.len() != n {
        return None;
    }
    let (rev, one) = manifold_side(&eig)?;
    if reversed.is_some_and(|r| r != rev) {
        return None;
    }
    let lambda = eig[one].0;
    let mut shifted = a.clone();
    for i in 0..n {
        shifted[i * n + i] -= lambda;
    }
    let transposed: Vec<f64> = (0..n * n).map(|i| shifted[i % n * n + i / n]).collect();
    let mut v = unit(null_vector(&shifted, n)?)?;
    let mut l = unit(null_vector(&transposed, n)?)?;
    let flip = match orient {
        Some(o) => dot(&v, o) < 0.0,
        None => {
            let big = v
                .iter()
                .fold(0.0_f64, |m, c| if c.abs() > m.abs() { *c } else { m });
            big < 0.0
        }
    };
    if flip {
        v.iter_mut().for_each(|c| *c = -*c);
    }
    if dot(&l, &v) < 0.0 {
        l.iter_mut().for_each(|c| *c = -*c);
    }
    Some(Linear {
        saddle: classify(x, eig, rev, one),
        reversed: rev,
        lambda,
        v,
        l,
    })
}

/// The zero of `g` in `[lo, hi]`, positive at `lo` and not at `hi`, by
/// bisection to the resolution of `t`.
fn bisect_time(mut lo: f64, mut hi: f64, mut g: impl FnMut(f64) -> f64) -> f64 {
    loop {
        let mid = 0.5 * (lo + hi);
        if mid == lo || mid == hi {
            return mid;
        }
        if g(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
}

/// Shoot from `x* ± ε·v` along `half` and return the split at the first
/// entry into the ellipsoid around the saddle, or `None` if the orbit does
/// not come back before the time limit, settles, or the integration fails.
///
/// The ellipsoid is measured in the saddle's own coordinates
/// `x − x* = y·v + w`, `y = l·(x − x*)/(l·v)` and `w` in the stable subspace
/// (unstable in reversed time): `(y/(RHO_U·Dmax))² + (|w|/(RHO·Dmax))² < 1`,
/// `Dmax` the largest `√(y² + |w|²)` of the excursion. The split is
/// `y/(RHO_U·Dmax)` there.
fn shoot(
    f: &dyn VectorField,
    p: &[f64],
    lin: &Linear,
    half: Half,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Shot>, Cancelled> {
    let fastest = lin
        .saddle
        .eigenvalues
        .iter()
        .fold(0.0_f64, |m, e| m.max(e.0.abs()));
    if lin.lambda.abs() < LAMBDA_FLOOR * fastest {
        return Ok(None);
    }
    let n = f.dim();
    let xs = &lin.saddle.x;
    let x0: Vec<f64> = xs
        .iter()
        .zip(&lin.v)
        .map(|(x, v)| x + half.sign() * EPSILON * v)
        .collect();
    let dir = if lin.reversed { -1.0 } else { 1.0 };
    let opts = SolverOptions {
        method: Method::Dopri5,
        rtol: RTOL,
        atol: ATOL,
        h: 0.0,
        h_max: 0.0,
        max_steps: MAX_STEPS,
    };
    let t_end = dir * MAX_TIME_CONSTANTS / lin.lambda.abs();
    let mut stepper = Stepper::new(f, p, 0.0, &x0, t_end, &opts);
    let mut buf = vec![0.0; n];
    let mut fx = vec![0.0; n];
    let lv = dot(&lin.l, &lin.v);
    let coordinates = |x: &[f64]| {
        let offset: Vec<f64> = x.iter().zip(xs).map(|(a, b)| a - b).collect();
        let y = dot(&lin.l, &offset) / lv;
        let w: Vec<f64> = offset.iter().zip(&lin.v).map(|(o, v)| o - y * v).collect();
        (y, w)
    };
    let distance = |x: &[f64]| {
        let (y, w) = coordinates(x);
        (y * y + dot(&w, &w)).sqrt()
    };
    // Half the rate of change of the squared distance in the direction of
    // integration; `fx` gets `f(x)`.
    let outward = |x: &[f64], fx: &mut [f64]| {
        f.eval(0.0, x, p, fx);
        let (y, w) = coordinates(x);
        let dy = dot(&lin.l, fx) / lv;
        let dw: Vec<f64> = fx.iter().zip(&lin.v).map(|(f, v)| f - dy * v).collect();
        dir * (y * dy + dot(&w, &dw))
    };
    let mut dmax = EPSILON;
    let mut d_prev = EPSILON;
    let mut was_outward = true;
    for steps in 1..=MAX_STEPS {
        if steps % CANCEL_STEPS == 0 && cancelled() {
            return Err(Cancelled);
        }
        if stepper.step() != StepOutcome::Accepted {
            return Ok(None);
        }
        let (t0, t1) = (stepper.t_prev(), stepper.t());
        let x1 = stepper.x().to_vec();
        let now_outward = outward(&x1, &mut fx) > 0.0;
        let d1 = distance(&x1);
        // Within a step the distance exceeds its ends by at most the way
        // travelled, so only a turn that can raise `Dmax` is refined.
        let reach = d_prev.max(d1) + 2.0 * (t1 - t0).abs() * norm2(&fx) * (1.0 + 2.0 / lv);
        if was_outward && !now_outward && reach > dmax {
            let tm = bisect_time(t0, t1, |t| {
                stepper.interpolate(t, &mut buf);
                outward(&buf, &mut fx)
            });
            stepper.interpolate(tm, &mut buf);
            dmax = dmax.max(distance(&buf));
        }
        was_outward = now_outward;
        d_prev = d1;
        dmax = dmax.max(d1);
        if dmax > 10.0 * EPSILON && norm2(&fx) < SETTLED * lin.lambda.abs() * dmax {
            return Ok(None);
        }
        let (r, ru) = (RHO * dmax, RHO_U * dmax);
        let inside = |x: &[f64]| {
            let (y, w) = coordinates(x);
            (y / ru).powi(2) + dot(&w, &w) / (r * r) - 1.0
        };
        if inside(&x1) < 0.0 {
            let tc = bisect_time(t0, t1, |t| {
                stepper.interpolate(t, &mut buf);
                inside(&buf)
            });
            stepper.interpolate(tc, &mut buf);
            return Ok(Some(Shot {
                split: coordinates(&buf).0 / ru,
                return_time: tc.abs(),
                dmax,
            }));
        }
    }
    Ok(None)
}

/// A point of a scan along a path `t ↦ p(t)` in parameter space where the
/// saddle exists, and the shot from it if the orbit came back.
#[derive(Clone)]
struct Sample {
    t: f64,
    params: Vec<f64>,
    lin: Linear,
    shot: Option<Shot>,
}

impl Sample {
    fn split(&self) -> Option<f64> {
        self.shot.map(|s| s.split)
    }
}

/// Shots along one half of the one-dimensional manifold.
struct Shooter<'a> {
    f: &'a dyn VectorField,
    half: Half,
    cancelled: &'a dyn Fn() -> bool,
}

impl Shooter<'_> {
    fn sample(&self, t: f64, params: Vec<f64>, lin: Linear) -> Result<Sample, HomoclinicError> {
        if (self.cancelled)() {
            return Err(HomoclinicError::Cancelled);
        }
        let shot = shoot(self.f, &params, &lin, self.half, self.cancelled)
            .map_err(|Cancelled| HomoclinicError::Cancelled)?;
        Ok(Sample {
            t,
            params,
            lin,
            shot,
        })
    }

    /// The homoclinic orbit between two neighbouring samples of `path`: a
    /// sign change of the split, or the edge of the set where it is defined,
    /// bisected in `t` until the parameters agree to `1e-13`. It is a root
    /// only if the split went to zero; a jump of the split, or an edge where
    /// the orbit grazes the ellipsoid, is not.
    fn root(
        &self,
        path: &dyn Fn(f64) -> Vec<f64>,
        a: &Sample,
        b: &Sample,
    ) -> Result<Option<HomoclinicOrbit>, HomoclinicError> {
        let (mut lo, mut hi) = match (a.split(), b.split()) {
            (Some(sa), Some(sb)) if (sa > 0.0) != (sb > 0.0) => (a.clone(), b.clone()),
            (Some(_), None) => (a.clone(), b.clone()),
            (None, Some(_)) => (b.clone(), a.clone()),
            _ => return Ok(None),
        };
        let positive = lo.split().is_some_and(|s| s > 0.0);
        for _ in 0..MAX_ROOT_BISECTION {
            let pinned = lo
                .params
                .iter()
                .zip(&hi.params)
                .all(|(l, h)| (h - l).abs() <= 1e-13 * (1.0 + l.abs()));
            if pinned {
                break;
            }
            let t = 0.5 * (lo.t + hi.t);
            let params = path(t);
            let Some(lin) = linearize(
                self.f,
                &params,
                &lo.lin.saddle.x,
                Some(lo.lin.reversed),
                Some(&lo.lin.v),
            ) else {
                return Ok(None);
            };
            let mid = self.sample(t, params, lin)?;
            match mid.split() {
                Some(s) if (s > 0.0) == positive => lo = mid,
                _ => hi = mid,
            }
        }
        let best = [lo, hi]
            .into_iter()
            .filter_map(|s| Some((s.split()?.abs(), s)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        Ok(best
            .filter(|(split, _)| *split <= ROOT_SPLIT)
            .map(|(_, s)| HomoclinicOrbit {
                return_time: s.shot.expect("a sample with a split").return_time,
                params: s.params,
                saddle: s.lin.saddle,
                reversed: s.lin.reversed,
                half: self.half,
            }))
    }
}

/// The homoclinic orbits to the saddle near `x` as `params[param]` runs
/// over `range`, from `samples` evenly spaced values, in order of return
/// time, the shortest first, with one entry per half of the manifold.
///
/// The saddle is followed from `x` by Newton's method from one sample to
/// the next, and its one-dimensional manifold keeps the side it has at the
/// first sample where it exists. Between two neighbours where the split
/// changes sign, or where it is defined at one and the orbit does not come
/// back at the other, the bracket is bisected; it is an orbit only if the
/// split went to zero there. So a sign change where the split jumps,
/// because the first entry moves to another loop of the orbit, is dropped,
/// and so is a multi-loop orbit whose earlier loops enter the ellipsoid, as
/// only the first entry is tested. Multi-loop orbits whose earlier loops
/// pass outside it are found too, after the primary orbit they shadow,
/// which has the shortest return time.
pub fn homoclinic_at(
    f: &dyn VectorField,
    params: &[f64],
    x: &[f64],
    param: usize,
    range: (f64, f64),
    samples: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<HomoclinicOrbit>, HomoclinicError> {
    if !f.is_autonomous() {
        return Err(HomoclinicError::NotAutonomous);
    }
    if f.kind() != SystemKind::Ode || samples < 2 || param >= params.len() {
        return Err(HomoclinicError::Unsupported);
    }
    let path = |t: f64| {
        let mut p = params.to_vec();
        p[param] = t;
        p
    };
    let mut found = Vec::new();
    for half in [Half::Plus, Half::Minus] {
        let shooter = Shooter { f, half, cancelled };
        let mut guess: (Vec<f64>, Option<bool>, Option<Vec<f64>>) = (x.to_vec(), None, None);
        let mut prev: Option<Sample> = None;
        for i in 0..samples {
            let t = range.0 + (range.1 - range.0) * i as f64 / (samples - 1) as f64;
            let p = path(t);
            let Some(lin) = linearize(f, &p, &guess.0, guess.1, guess.2.as_deref()) else {
                prev = None;
                continue;
            };
            guess = (
                lin.saddle.x.clone(),
                Some(lin.reversed),
                Some(lin.v.clone()),
            );
            let here = shooter.sample(t, p, lin)?;
            if let Some(a) = &prev {
                found.extend(shooter.root(&path, a, &here)?);
            }
            prev = Some(here);
        }
    }
    found.sort_by(|a, b| a.return_time.total_cmp(&b.return_time));
    Ok(found)
}

/// Half the side, in the scaled parameters, of the square around a
/// Bogdanov–Takens point where [`homoclinic_from_bt`] looks for the orbit.
///
/// Near a generic BT point the homoclinic, Hopf and fold curves are
/// tangent, and the picture they make is local: CO oxidation's second BT
/// point has no orbit near a square of `0.01` or `0.03` of the box used in
/// the tests, and one at `0.001` and `0.003`. The Freire circuit, CO
/// oxidation's first BT point and Lorenz-84's start at `0.001` too.
pub const BT_OFFSET: f64 = 0.001;
/// Points sampled around that square for the saddle.
const BT_SAMPLES: usize = 120;
/// The square's perimeter in units of [`BT_OFFSET`].
const PERIMETER: f64 = 8.0;
/// Newton seeds `x + c·(1 + |x|∞)·q` for the saddle near the BT state `x`,
/// `q` the kernel of the Jacobian there: the two equilibria born at the BT
/// point move apart along `q`, at a rate that depends on the system.
const BT_SEEDS: [f64; 9] = [0.0, 1e-4, -1e-4, 1e-3, -1e-3, 1e-2, -1e-2, 1e-1, -1e-1];
/// The march from an end of a stretch where the saddle exists starts this
/// far into it, in units of [`BT_OFFSET`], and grows by `MARCH_GROWTH` per
/// sample.
const MARCH_FIRST: f64 = 1e-8;
const MARCH_GROWTH: f64 = 1.5;

/// The saddle at `p` among the equilibria from the [`BT_SEEDS`]: one whose
/// two eigenvalues nearest zero, the pair at the BT point, are real with
/// opposite signs, and whose one-dimensional manifold exists; of several,
/// the nearest to `near`.
fn bt_saddle(f: &dyn VectorField, p: &[f64], x: &[f64], q: &[f64], near: &[f64]) -> Option<Linear> {
    let n = f.dim();
    let scale = 1.0 + x.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    BT_SEEDS
        .iter()
        .filter_map(|c| {
            let seed: Vec<f64> = x.iter().zip(q).map(|(x, q)| x + c * scale * q).collect();
            let lin = linearize(f, p, &seed, None, None)?;
            let mut by_size = lin.saddle.eigenvalues.clone();
            by_size.sort_by(|a, b| a.0.hypot(a.1).total_cmp(&b.0.hypot(b.1)));
            let pair = n >= 2 && by_size[0].1 == 0.0 && by_size[1].1 == 0.0;
            (pair && (by_size[0].0 > 0.0) != (by_size[1].0 > 0.0)).then_some(lin)
        })
        .min_by(|a, b| {
            let d = |l: &Linear| {
                l.saddle
                    .x
                    .iter()
                    .zip(near)
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum::<f64>()
            };
            d(a).total_cmp(&d(b))
        })
}

/// A homoclinic orbit near the Bogdanov–Takens point at `params`, state
/// `x`, of the curve of homoclinic orbits that ends there, with
/// `params[opts.free.0]` and `params[opts.free.1]` free in the box
/// `opts.ranges`.
///
/// The orbit is looked for on the square of half-side [`BT_OFFSET`] around
/// the BT point in the scaled parameters `u = (p − lo)/(hi − lo)`, each side
/// holding one parameter at that offset and scanning the other, to the
/// saddle of the pair of equilibria born there: the one whose eigenvalues
/// near zero are real with opposite signs. Near a generic BT point that
/// saddle exists on a stretch of the square that ends on the fold curve,
/// and the curve of homoclinic orbits, tangent to the fold curve, crosses
/// the square within `O(BT_OFFSET²)` of an end of the stretch, where the
/// split is defined on a window of the same order. So the split is first
/// sampled from each end into the stretch at distances growing
/// geometrically from `1e-8 · BT_OFFSET`, then at 120 evenly spaced points,
/// which find a curve that leaves the BT point at an angle, as it does when
/// a symmetry keeps the saddle on both sides of the BT point. The first
/// orbit found, by the bisection of [`homoclinic_at`], is returned.
pub fn homoclinic_from_bt(
    f: &dyn VectorField,
    params: &[f64],
    x: &[f64],
    opts: &CurveOptions,
    cancelled: &dyn Fn() -> bool,
) -> Result<HomoclinicOrbit, HomoclinicError> {
    if !f.is_autonomous() {
        return Err(HomoclinicError::NotAutonomous);
    }
    if f.kind() != SystemKind::Ode || f.dim() < 2 || opts.free.0 == opts.free.1 {
        return Err(HomoclinicError::Unsupported);
    }
    let n = f.dim();
    let mut a = vec![0.0; n * n];
    f.jacobian(0.0, x, params, &mut a);
    let q = null_vector(&a, n)
        .and_then(unit)
        .ok_or(HomoclinicError::NoSaddle)?;
    let free = [opts.free.0, opts.free.1];
    let edges = [opts.ranges.0, opts.ranges.1].map(|(a, b)| (a.min(b), a.max(b)));
    let centre = [0, 1].map(|k| (params[free[k]] - edges[k].0) / (edges[k].1 - edges[k].0));
    let path = |t: f64| {
        let mut p = params.to_vec();
        for (k, offset) in square(t).into_iter().enumerate() {
            let (lo, hi) = edges[k];
            let u = centre[k] + BT_OFFSET * offset;
            p[free[k]] = (1.0 - u) * lo + u * hi;
        }
        p
    };
    let saddle_at = |t: f64, near: &[f64]| bt_saddle(f, &path(t), x, &q, near);
    let full = PERIMETER;
    let along: Vec<f64> = (0..BT_SAMPLES)
        .map(|k| full * k as f64 / BT_SAMPLES as f64)
        .collect();
    let mut coarse = Vec::with_capacity(BT_SAMPLES);
    for &t in &along {
        if cancelled() {
            return Err(HomoclinicError::Cancelled);
        }
        coarse.push(saddle_at(t, x));
    }
    if coarse.iter().all(Option::is_none) {
        return Err(HomoclinicError::NoSaddle);
    }
    let mut ends = Vec::new();
    for k in 0..BT_SAMPLES {
        let next = (k + 1) % BT_SAMPLES;
        if coarse[k].is_some() == coarse[next].is_some() {
            continue;
        }
        let (mut inside, mut outside) = if coarse[k].is_some() {
            (along[k], along[k] + full / BT_SAMPLES as f64)
        } else {
            (along[next] + if next == 0 { full } else { 0.0 }, along[k])
        };
        while (inside - outside).abs() > 1e-12 {
            let mid = 0.5 * (inside + outside);
            if saddle_at(mid, x).is_some() {
                inside = mid;
            } else {
                outside = mid;
            }
        }
        ends.push((inside, if inside > outside { 1.0 } else { -1.0 }));
    }
    for half in [Half::Plus, Half::Minus] {
        let shooter = Shooter { f, half, cancelled };
        for &(end, dir) in &ends {
            let mut prev: Option<Sample> = None;
            let mut step = MARCH_FIRST;
            while step < full {
                let t = end + dir * step;
                step *= MARCH_GROWTH;
                let near = prev.as_ref().map_or(x, |s| &s.lin.saddle.x);
                let Some(lin) = saddle_at(t, near) else {
                    break;
                };
                let here = shooter.sample(t, path(t), lin)?;
                if let Some(a) = &prev {
                    if let Some(orbit) = shooter.root(&path, a, &here)? {
                        return Ok(orbit);
                    }
                }
                prev = Some(here);
            }
        }
    }
    for half in [Half::Plus, Half::Minus] {
        let shooter = Shooter { f, half, cancelled };
        let mut prev: Option<Sample> = None;
        for (k, lin) in coarse.iter().enumerate().chain([(0, &coarse[0])]) {
            let Some(lin) = lin else {
                prev = None;
                continue;
            };
            let t = along[k] + if prev.is_some() && k == 0 { full } else { 0.0 };
            let here = shooter.sample(t, path(t), lin.clone())?;
            if let Some(a) = &prev {
                if let Some(orbit) = shooter.root(&path, a, &here)? {
                    return Ok(orbit);
                }
            }
            prev = Some(here);
        }
    }
    Err(HomoclinicError::NotFound)
}

/// The point at `t` along the perimeter of the square `[−1, 1]²`, from
/// `(1, −1)` anticlockwise, with period [`PERIMETER`].
fn square(t: f64) -> [f64; 2] {
    let t = t.rem_euclid(PERIMETER);
    let along = t % 2.0 - 1.0;
    match (t / 2.0) as usize {
        0 => [1.0, along],
        1 => [-along, 1.0],
        2 => [-1.0, -along],
        _ => [along, -1.0],
    }
}

/// One point of a curve of homoclinic orbits.
#[derive(Debug, Clone, PartialEq)]
pub struct HomoclinicPoint {
    /// Arclength from the start in the scaled parameters `u = (p − lo)/(hi − lo)`.
    pub s: f64,
    /// The parameters `(p₁, p₂)`.
    pub p: (f64, f64),
    /// The saddle.
    pub saddle: Saddle,
    /// The time from `ε` off the saddle to the entry into the ellipsoid.
    pub return_time: f64,
}

/// Why a curve of homoclinic orbits stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomoclinicEnd {
    /// It reached an edge of the box; the last point lies on it.
    Range,
    /// It has `max_points` points.
    MaxPoints,
    /// A step failed with the step at `ds_min`.
    StepTooSmall,
    /// The orbit shrank onto the saddle, as it does at a Bogdanov–Takens
    /// point: at the last point `l·v < 1e-3`, the one-dimensional eigenvalue
    /// about to meet another at zero, or `RHO · Dmax < ε`, the ellipsoid
    /// narrower than the shot's offset from the saddle. The first holds
    /// first where the loop shrinks like `l·v` (the Freire circuit, whose
    /// symmetry keeps the saddle at the origin), the second where it shrinks
    /// like `(l·v)²` (CO oxidation, Lorenz-84).
    Shrunk,
}

/// A curve of homoclinic orbits in two parameters, one direction from its
/// start.
#[derive(Debug, Clone, PartialEq)]
pub struct Homoclinic {
    /// The one-dimensional manifold is the stable one.
    pub reversed: bool,
    /// The branch of the one-dimensional manifold that returns.
    pub half: Half,
    /// The points in order of arclength, the corrected start first.
    pub points: Vec<HomoclinicPoint>,
    /// Why the curve stopped.
    pub end: HomoclinicEnd,
}

/// The split function of the scaled parameters `z = (u₁, u₂)`, one
/// equation in two unknowns, with the saddle followed by Newton's method
/// from the last accepted point's.
struct Split<'a> {
    f: &'a dyn VectorField,
    params: Vec<f64>,
    free: [usize; 2],
    edges: [(f64, f64); 2],
    half: Half,
    reversed: bool,
    saddle: Vec<f64>,
    v: Vec<f64>,
    cancelled: &'a dyn Fn() -> bool,
    stopped: Cell<bool>,
    last: RefCell<Option<Evaluated>>,
    /// `1/|∇split|` at the last accepted point: the equation is the split
    /// times this, a distance to the curve in `u`.
    width: f64,
}

#[derive(Clone)]
struct Evaluated {
    z: Vec<f64>,
    lin: Linear,
    shot: Shot,
    gradient: [f64; 2],
}

impl Split<'_> {
    fn params_at(&self, z: &[f64]) -> Vec<f64> {
        let mut p = self.params.clone();
        for k in 0..2 {
            let ((lo, hi), u) = (self.edges[k], z[k]);
            p[self.free[k]] = (1.0 - u) * lo + u * hi;
        }
        p
    }

    fn scaled(&self, params: &[f64]) -> Vec<f64> {
        (0..2)
            .map(|k| {
                let (lo, hi) = self.edges[k];
                (params[self.free[k]] - lo) / (hi - lo)
            })
            .collect()
    }

    fn shot(&self, z: &[f64]) -> Option<(Linear, Shot)> {
        if self.stopped.get() || (self.cancelled)() {
            self.stopped.set(true);
            return None;
        }
        let p = self.params_at(z);
        let lin = linearize(self.f, &p, &self.saddle, Some(self.reversed), Some(&self.v))?;
        match shoot(self.f, &p, &lin, self.half, self.cancelled) {
            Ok(shot) => Some((lin, shot?)),
            Err(Cancelled) => {
                self.stopped.set(true);
                None
            }
        }
    }

    fn evaluate(&self, z: &[f64]) -> Option<Evaluated> {
        if let Some(e) = self.last.borrow().as_ref().filter(|e| e.z == z) {
            return Some(e.clone());
        }
        let (lin, shot) = self.shot(z)?;
        let mut gradient = [0.0; 2];
        for (k, g) in gradient.iter_mut().enumerate() {
            let mut moved = z.to_vec();
            moved[k] += FD_SPLIT * self.width;
            *g = (self.shot(&moved)?.1.split - shot.split) / (moved[k] - z[k]);
        }
        let e = Evaluated {
            z: z.to_vec(),
            lin,
            shot,
            gradient,
        };
        *self.last.borrow_mut() = Some(e.clone());
        Some(e)
    }

    /// The new `width` from the gradient at an accepted point. The cached
    /// evaluation used the old difference step, so it is dropped.
    fn renew_width(&mut self, e: &Evaluated) {
        let len = norm2(&e.gradient);
        if len.is_finite() && len > 0.0 {
            self.width = 1.0 / len;
        }
        *self.last.get_mut() = None;
    }

    fn point(&self, s: f64, e: &Evaluated) -> HomoclinicPoint {
        let p = self.params_at(&e.z);
        HomoclinicPoint {
            s,
            p: (p[self.free[0]], p[self.free[1]]),
            saddle: e.lin.saddle.clone(),
            return_time: e.shot.return_time,
        }
    }
}

impl Underdetermined for Split<'_> {
    fn linear(&self, z: &[f64]) -> (Vec<f64>, Vec<f64>) {
        match self.evaluate(z) {
            Some(e) => (
                vec![e.shot.split * self.width],
                e.gradient.iter().map(|g| g * self.width).collect(),
            ),
            None => (vec![f64::NAN], vec![f64::NAN; 2]),
        }
    }
}

/// Follow the curve of homoclinic orbits through `orbit` as
/// `params[opts.free.0]` and `params[opts.free.1]` vary, in one direction:
/// `opts.ds > 0` the way `p₁` increases at the start, or `p₂` where the
/// curve is vertical there.
///
/// The curve is the zero set of the split function in the scaled
/// parameters `u = (p − lo)/(hi − lo)`, followed by pseudo-arclength
/// continuation with the step control of [`crate::continue_curve`] and the
/// split's gradient by forward differences. A step where the split is not
/// defined (no saddle of the start's kind, or no return) fails and is
/// halved.
///
/// `cancelled` is called before each evaluation of the split and every 256
/// steps of its integration; once it returns `true` the call returns
/// [`HomoclinicError::Cancelled`]. `progress` is called with each point as
/// it is accepted, the start included.
pub fn continue_homoclinic(
    f: &dyn VectorField,
    orbit: &HomoclinicOrbit,
    opts: &CurveOptions,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(&HomoclinicPoint),
) -> Result<Homoclinic, HomoclinicError> {
    if !f.is_autonomous() {
        return Err(HomoclinicError::NotAutonomous);
    }
    if f.kind() != SystemKind::Ode || opts.free.0 == opts.free.1 {
        return Err(HomoclinicError::Unsupported);
    }
    let sorted = |(a, b): (f64, f64)| (a.min(b), a.max(b));
    let lin0 = linearize(
        f,
        &orbit.params,
        &orbit.saddle.x,
        Some(orbit.reversed),
        None,
    )
    .ok_or(HomoclinicError::NoSaddle)?;
    let mut sys = Split {
        f,
        params: orbit.params.clone(),
        free: [opts.free.0, opts.free.1],
        edges: [sorted(opts.ranges.0), sorted(opts.ranges.1)],
        half: orbit.half,
        reversed: orbit.reversed,
        saddle: lin0.saddle.x.clone(),
        v: lin0.v.clone(),
        cancelled,
        stopped: Cell::new(false),
        last: RefCell::new(None),
        width: FD_FIRST / FD_SPLIT,
    };
    let stopped = |sys: &Split| {
        if sys.stopped.get() {
            Err(HomoclinicError::Cancelled)
        } else {
            Ok(())
        }
    };
    let z0 = sys.scaled(&orbit.params);
    let e0 = sys.evaluate(&z0);
    stopped(&sys)?;
    let e0 = e0.ok_or(HomoclinicError::NotFound)?;
    sys.renew_width(&e0);
    let e0 = sys.evaluate(&z0);
    stopped(&sys)?;
    let e0 = e0.ok_or(HomoclinicError::NotFound)?;
    let across = unit(vec![-e0.gradient[1], e0.gradient[0]]).ok_or(HomoclinicError::NotFound)?;
    let corrected = correct_along(&sys, &z0, &across);
    stopped(&sys)?;
    let (z, _) = corrected.map_err(|_| HomoclinicError::NotFound)?;
    let tau = tangent(&sys, &z, &across);
    stopped(&sys)?;
    let tau = tau.ok_or(HomoclinicError::NotFound)?;
    let lead = if tau[0] != 0.0 { tau[0] } else { tau[1] };
    let sign = if (lead < 0.0) == (opts.ds < 0.0) {
        1.0
    } else {
        -1.0
    };
    let tau: Vec<f64> = tau.iter().map(|t| sign * t).collect();
    let e = sys.evaluate(&z).ok_or(HomoclinicError::NotFound)?;
    sys.saddle = e.lin.saddle.x.clone();
    sys.v = e.lin.v.clone();
    sys.renew_width(&e);
    let start = sys.point(0.0, &e);
    progress(&start);
    let mut curve = Homoclinic {
        reversed: orbit.reversed,
        half: orbit.half,
        points: vec![start],
        end: HomoclinicEnd::Range,
    };
    if !z.iter().all(|u| (0.0..=1.0).contains(u)) {
        return Ok(curve);
    }
    let h = opts.ds.abs().min(opts.ds_max).max(opts.ds_min);
    curve.end = follow(&mut sys, z, tau, h, opts, &mut curve.points, progress)?;
    Ok(curve)
}

fn follow(
    sys: &mut Split,
    mut z: Vec<f64>,
    mut dir: Vec<f64>,
    mut h: f64,
    opts: &CurveOptions,
    points: &mut Vec<HomoclinicPoint>,
    progress: &mut dyn FnMut(&HomoclinicPoint),
) -> Result<HomoclinicEnd, HomoclinicError> {
    let cos_max = cos(opts.max_angle);
    let mut last_tangent = dir.clone();
    loop {
        if points.len() >= opts.max_points {
            return Ok(HomoclinicEnd::MaxPoints);
        }
        if h < opts.ds_min {
            return Ok(HomoclinicEnd::StepTooSmall);
        }
        let pred: Vec<f64> = z.iter().zip(&dir).map(|(a, d)| a + h * d).collect();
        let edge = first_edge(&z, &pred, &dir, 0);
        let corrected = match edge {
            Some((k, e)) if z[k] == e => return Ok(HomoclinicEnd::Range),
            Some((k, e)) => {
                let along = (e - z[k]) / dir[k];
                let guess: Vec<f64> = z.iter().zip(&dir).map(|(a, d)| a + along * d).collect();
                correct_fixed(&*sys, &guess, k, e)
            }
            None => correct_along(&*sys, &pred, &dir),
        };
        let accepted = corrected.ok().and_then(|(z_new, iters)| {
            let t = tangent(&*sys, &z_new, &last_tangent)?;
            (dot(&t, &last_tangent) >= cos_max).then_some((z_new, iters, t))
        });
        if sys.stopped.get() {
            return Err(HomoclinicError::Cancelled);
        }
        let Some((z_new, iters, t_new)) = accepted else {
            h /= 2.0;
            continue;
        };
        let e = sys
            .evaluate(&z_new)
            .expect("an accepted point was evaluated");
        let s = points.last().expect("the start").s + dist(&z, &z_new);
        let point = sys.point(s, &e);
        progress(&point);
        points.push(point);
        sys.saddle = e.lin.saddle.x.clone();
        sys.v = e.lin.v.clone();
        sys.renew_width(&e);
        if edge.is_some() {
            return Ok(HomoclinicEnd::Range);
        }
        if dot(&e.lin.l, &e.lin.v) < SHRUNK || RHO * e.shot.dmax < EPSILON {
            return Ok(HomoclinicEnd::Shrunk);
        }
        let len = dist(&z, &z_new);
        dir = diff(&z_new, &z).iter().map(|c| c / len).collect();
        last_tangent = t_new;
        z = z_new;
        if iters <= FAST_NEWTON_ITERS {
            h = (h * STEP_GROWTH).min(opts.ds_max);
        } else if iters > SLOW_NEWTON_ITERS {
            h = (h / 2.0).max(opts.ds_min);
        }
    }
}
