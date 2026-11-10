//! Homoclinic orbits and their curves against docs/codim2-reference.md §4,
//! the Freire circuit of AUTO's HomCont demo `cir`.

use std::cell::Cell;
use std::time::{Duration, Instant};

use dyn_analysis::{
    continue_homoclinic, homoclinic_at, homoclinic_from_bt, CurveOptions, Half, Homoclinic,
    HomoclinicEnd, HomoclinicError, HomoclinicOrbit,
};
use dyn_expr::{compile, CompiledSystem, InputFormat};

struct Freire {
    sys: CompiledSystem,
    nu: usize,
    beta: usize,
}

fn freire() -> Freire {
    let sys = compile(
        "x' = (-(beta + nu)*x + beta*y - 0.328578*x^3 + 0.933578*(y - x)^3)/0.6\n\
         y' = beta*x - beta*y - z - 0.933578*(y - x)^3\n\
         z' = y",
        InputFormat::Text,
    )
    .system
    .expect("the Freire circuit compiles");
    let index = |name: &str| {
        sys.info
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| panic!("parameter {name}"))
    };
    let (nu, beta) = (index("nu"), index("beta"));
    Freire { sys, nu, beta }
}

/// The Bogdanov–Takens point `ν = −β = −√0.6`.
const BT: (f64, f64) = (-0.774_596_669_241_483_4, 0.774_596_669_241_483_4);

impl Freire {
    fn params(&self, nu: f64, beta: f64) -> Vec<f64> {
        let mut p = vec![0.0; 2];
        p[self.nu] = nu;
        p[self.beta] = beta;
        p
    }

    /// The primary homoclinic orbit at `beta` on the `+v` half: the one of
    /// shortest return time with `ν` scanned over `range`.
    fn at(&self, beta: f64, range: (f64, f64)) -> Vec<HomoclinicOrbit> {
        homoclinic_at(
            &self.sys,
            &self.params(range.0, beta),
            &[0.0; 3],
            self.nu,
            range,
            41,
            &|| false,
        )
        .expect("the scan runs")
        .into_iter()
        .filter(|o| o.half == Half::Plus)
        .collect()
    }

    /// Steps of at most 0.002 keep the chords between points within 1e-6
    /// of the curve, whose curvature is about 1.5 near `β = 0.6`.
    fn opts(&self, ds: f64, max_points: usize) -> CurveOptions {
        CurveOptions {
            free: (self.nu, self.beta),
            ranges: ((-1.2, -0.4), (0.1, 1.0)),
            ds,
            ds_min: 1e-8,
            ds_max: 0.002,
            max_points,
            max_angle: 0.3,
        }
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The distance from `target` to the polyline through the points of `curve`.
fn distance_to(curve: &Homoclinic, target: (f64, f64)) -> f64 {
    curve
        .points
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0].p, w[1].p);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((target.0 - a.0) * dx + (target.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            distance((a.0 + t * dx, a.1 + t * dy), target)
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn freire_orbit_at_beta_0_6_to_five_digits_without_the_false_root() {
    let f = freire();
    let found = f.at(0.6, (-0.7235, -0.7195));
    let nu: Vec<f64> = found.iter().map(|o| o.params[f.nu]).collect();
    assert_eq!(nu.len(), 1, "one orbit, not the jump near −0.72201: {nu:?}");
    let o = &found[0];
    assert!((nu[0] - -0.7213093).abs() <= 5e-6, "ν = {}", nu[0]);
    assert!(
        (o.saddle.sigma - -0.01701).abs() <= 5e-5,
        "σ = {}",
        o.saddle.sigma
    );
    assert!(o.saddle.focus);
    assert!(!o.saddle.chaotic);
    assert!(!o.reversed);
}

#[test]
fn freire_orbit_at_beta_0_4535645_is_shilnikov_chaotic() {
    let f = freire();
    let found = f.at(0.4535645, (-0.7275, -0.7245));
    let o = &found[0];
    let nu = o.params[f.nu];
    assert!((nu - -0.7256926).abs() <= 5e-6, "ν = {nu}");
    assert!(
        (o.saddle.sigma - 0.28883).abs() <= 5e-5,
        "σ = {}",
        o.saddle.sigma
    );
    assert!(o.saddle.focus);
    assert!(o.saddle.chaotic);
    // The subsidiary orbit the brief finds at −0.72695630 is seen, after the
    // primary one.
    let next = &found[1];
    assert!(
        (next.params[f.nu] - -0.7269563).abs() <= 1e-6,
        "ν = {}",
        next.params[f.nu]
    );
    assert!(next.return_time > o.return_time);
}

#[test]
fn freire_curve_from_bt_passes_both_points_and_shrinks_onto_bt() {
    let f = freire();
    let bt = f.params(BT.0, BT.1);
    let start = homoclinic_from_bt(&f.sys, &bt, &[0.0; 3], &f.opts(0.002, 300), &|| false)
        .expect("an orbit next to the BT point");
    let halves: Vec<Homoclinic> = [0.002, -0.002]
        .into_iter()
        .map(|ds| {
            continue_homoclinic(&f.sys, &start, &f.opts(ds, 300), &|| false, &mut |_| {})
                .expect("the curve continues")
        })
        .collect();
    for target in [(-0.7213093, 0.6), (-0.7256926, 0.4535645)] {
        let d = halves
            .iter()
            .map(|c| distance_to(c, target))
            .fold(f64::INFINITY, f64::min);
        assert!(d <= 1e-5, "{target:?} is {d} from the curve");
    }
    let shrunk = halves
        .iter()
        .find(|c| c.end == HomoclinicEnd::Shrunk)
        .expect("one half ends at the BT point");
    let end = shrunk.points.last().expect("points").p;
    // `l·v ≈ 0.686 √d` along this curve, measured from d = 5.6e-7 to 8.2e-6,
    // so `l·v < 1e-3` holds within d = 2.12e-6 of the BT point.
    let d = distance(end, BT);
    assert!(d <= 2.2e-6, "ends {d} from the BT point at {end:?}");
}

#[test]
fn cancelling_returns_cancelled_promptly_and_keeps_nothing() {
    let f = freire();
    let start = f.at(0.6, (-0.7215, -0.7211)).remove(0);
    let opts = f.opts(0.01, 12);
    let reference = continue_homoclinic(&f.sys, &start, &opts, &|| false, &mut |_| {})
        .expect("the uncancelled curve");
    assert_eq!(reference.points.len(), 12);

    let accepted = Cell::new(0);
    let since: Cell<Option<Instant>> = Cell::new(None);
    let cancelled = || accepted.get() >= 3;
    let result = continue_homoclinic(&f.sys, &start, &opts, &cancelled, &mut |_| {
        accepted.set(accepted.get() + 1);
        if accepted.get() == 3 {
            since.set(Some(Instant::now()));
        }
    });
    let latency = since.get().expect("three points were accepted").elapsed();
    assert_eq!(result, Err(HomoclinicError::Cancelled));
    assert_eq!(accepted.get(), 3);
    assert!(latency < Duration::from_millis(50), "latency {latency:?}");

    let again =
        continue_homoclinic(&f.sys, &start, &opts, &|| false, &mut |_| {}).expect("a fresh run");
    assert_eq!(again, reference);
}

/// From a generic BT point the curve ends with [`HomoclinicEnd::Shrunk`]
/// once the ellipsoid's radius `RHO · Dmax` falls below `ε = 1e-6`.
fn shrinks_from_generic_bt(
    source: &str,
    names: [&str; 2],
    bt: (f64, f64),
    x: &[f64],
    ranges: ((f64, f64), (f64, f64)),
) -> (HomoclinicOrbit, Homoclinic) {
    let sys = compile(source, InputFormat::Text)
        .system
        .expect("the system compiles");
    let index = |name: &str| {
        sys.info
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| panic!("parameter {name}"))
    };
    let free = (index(names[0]), index(names[1]));
    let mut params = vec![0.0; sys.info.params.len()];
    params[free.0] = bt.0;
    params[free.1] = bt.1;
    let opts = |ds: f64| CurveOptions {
        free,
        ranges,
        ds,
        ds_min: 1e-8,
        ds_max: 0.02,
        max_points: 100,
        max_angle: 0.3,
    };
    let start = homoclinic_from_bt(&sys, &params, x, &opts(0.01), &|| false)
        .expect("an orbit next to the BT point");
    let towards = [0.01, -0.01]
        .into_iter()
        .map(|ds| {
            continue_homoclinic(&sys, &start, &opts(ds), &|| false, &mut |_| {})
                .expect("the curve continues")
        })
        .find(|c| c.end == HomoclinicEnd::Shrunk)
        .expect("one half ends at the BT point");
    (start, towards)
}

#[test]
fn co_oxidation_curve_from_its_first_bt_point_shrinks_onto_it() {
    let bt = (1.41762764, 0.97139757);
    let (start, curve) = shrinks_from_generic_bt(
        "x' = 2*2.5*(1 - x - y - s)^2 - 2*x^2 - 10*x*y\n\
         y' = k2*(1 - x - y - s) - 0.1*y - 10*x*y\n\
         s' = 0.0675*((1 - x - y - s) - K*s)",
        ["k2", "K"],
        bt,
        &[0.11590872, 0.31546728, 0.28843700],
        ((0.9, 1.8), (0.3, 1.5)),
    );
    assert!(!start.reversed);
    assert!(!start.saddle.focus);
    // `Dmax ≥ 0.155·d` over the last points, so `RHO · Dmax < 1e-6` holds
    // within d = 1.29e-4 of the BT point.
    let d = distance(curve.points.last().expect("points").p, bt);
    assert!(d <= 1.3e-4, "ends {d} from the BT point");
}

#[test]
fn lorenz_84_curve_from_its_bt_point_is_followed_in_reversed_time() {
    let bt = (1.4467167, 0.02094017);
    let (start, curve) = shrinks_from_generic_bt(
        "X' = -Y^2 - Z^2 - 0.25*X + 0.25*F - 0.987*U^2\n\
         Y' = X*Y - X*Z - Y + 0.25\n\
         Z' = X*Y + X*Z - Z\n\
         U' = -1.04*U + 0.987*U*X + T",
        ["F", "T"],
        bt,
        &[1.2256405, -0.03632079, 0.19728832, -0.12339001],
        ((1.0, 3.0), (-0.01, 0.06)),
    );
    assert!(start.reversed);
    // `Dmax ≥ 3.09·d` over the last points, so `RHO · Dmax < 1e-6` holds
    // within d = 6.5e-6 of the BT point.
    let d = distance(curve.points.last().expect("points").p, bt);
    assert!(d <= 6.5e-6, "ends {d} from the BT point");
}
