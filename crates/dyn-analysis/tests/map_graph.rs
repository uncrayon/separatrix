//! The graph of a one-dimensional map, for the cobweb view.

use dyn_analysis::{map_graph, MAX_GRAPH_SAMPLES};
use dyn_core::{FnField, SystemKind};

type Lines = Vec<Vec<(f64, f64)>>;

fn map1(g: fn(f64, &[f64]) -> f64, n_params: usize) -> FnField {
    FnField::new(1, n_params, SystemKind::Map, move |_n, x, p, out| {
        out[0] = g(x[0], p)
    })
}

fn graph(g: fn(f64) -> f64, x: (f64, f64), y: (f64, f64), samples: usize) -> Lines {
    let f = FnField::new(1, 0, SystemKind::Map, move |_n, x, _p, out| {
        out[0] = g(x[0])
    });
    map_graph(&f, &[], x, y, samples)
}

/// Every polyline: at least 2 points, x strictly increasing inside the x
/// range, y finite inside the band.
fn check_shape(lines: &Lines, x: (f64, f64), y: (f64, f64)) {
    let span = y.1 - y.0;
    let band = (y.0 - span, y.1 + span);
    for line in lines {
        assert!(line.len() >= 2, "{line:?}");
        for w in line.windows(2) {
            assert!(w[1].0 > w[0].0, "x not increasing: {w:?}");
        }
        for &(px, py) in line {
            assert!(px >= x.0 && px <= x.1, "x {px} outside {x:?}");
            assert!(
                py.is_finite() && py >= band.0 && py <= band.1,
                "y {py} outside {band:?}"
            );
        }
    }
}

/// No segment of any polyline spans `x = c`.
fn no_segment_spans(lines: &Lines, c: f64) {
    for line in lines {
        for w in line.windows(2) {
            assert!(!(w[0].0 < c && w[1].0 > c), "segment {w:?} spans {c}");
        }
    }
}

#[test]
fn logistic_is_one_polyline_of_exact_samples() {
    let f = map1(|x, p| p[0] * x * (1.0 - x), 1);
    let samples = 2048;
    let g = map_graph(&f, &[3.2], (0.0, 1.0), (0.0, 1.0), samples);
    assert_eq!(g.len(), 1);
    let line = &g[0];
    assert_eq!(line.len(), samples);
    assert_eq!(line[0].0, 0.0);
    assert_eq!(line[samples - 1].0, 1.0);
    for &(x, y) in line {
        assert_eq!(y.to_bits(), (3.2 * x * (1.0 - x)).to_bits(), "at {x}");
    }
    check_shape(&g, (0.0, 1.0), (0.0, 1.0));
}

#[test]
fn one_over_x_splits_at_its_pole_and_reaches_the_band_edge() {
    let (x, y) = ((-1.0, 1.0), (-1.0, 1.0));
    let g = graph(|x| 1.0 / x, x, y, 2048);
    assert_eq!(g.len(), 2, "{g:?}");
    check_shape(&g, x, y);
    no_segment_spans(&g, 0.0);
    // The band is [-3, 3]: the left branch leaves it at (-1/3, -3), the
    // right branch enters it at (1/3, 3), interpolated on the edge.
    let end = *g[0].last().unwrap();
    assert_eq!(end.1, -3.0);
    assert!((end.0 + 1.0 / 3.0).abs() < 1e-6, "{end:?}");
    let start = g[1][0];
    assert_eq!(start.1, 3.0);
    assert!((start.0 - 1.0 / 3.0).abs() < 1e-6, "{start:?}");
}

#[test]
fn an_even_pole_splits_into_two() {
    let (x, y) = ((-1.0, 1.0), (-1.0, 4.0));
    let g = graph(|x| 1.0 / (x * x), x, y, 2048);
    assert_eq!(g.len(), 2, "{g:?}");
    check_shape(&g, x, y);
    no_segment_spans(&g, 0.0);
    // The same with samples that don't straddle the pole symmetrically.
    let g = graph(|x| 1.0 / ((x - 0.1234) * (x - 0.1234)), x, y, 1000);
    assert_eq!(g.len(), 2);
    no_segment_spans(&g, 0.1234);
}

#[test]
fn tangent_has_three_branches() {
    let (x, y) = ((-3.0, 3.0), (-3.0, 3.0));
    let g = graph(f64::tan, x, y, 2048);
    assert_eq!(
        g.len(),
        3,
        "{:?}",
        g.iter().map(Vec::len).collect::<Vec<_>>()
    );
    check_shape(&g, x, y);
    let half_pi = std::f64::consts::FRAC_PI_2;
    no_segment_spans(&g, -half_pi);
    no_segment_spans(&g, half_pi);
}

#[test]
fn square_root_starts_at_the_edge_of_its_domain() {
    let (x, y) = ((-1.0, 1.0), (-1.0, 1.0));
    let samples = 2048;
    let g = graph(f64::sqrt, x, y, samples);
    assert_eq!(g.len(), 1);
    check_shape(&g, x, y);
    let cell = 2.0 / (samples - 1) as f64;
    let x0 = g[0][0].0;
    assert!(x0 >= 0.0 && x0 <= cell / 4096.0, "starts at {x0}");
    assert_eq!(g[0].last().unwrap(), &(1.0, 1.0));
    // An odd number of samples puts one on 0 itself.
    let g = graph(f64::sqrt, x, y, 2047);
    assert_eq!(g[0][0], (0.0, 0.0));
}

#[test]
fn the_tent_map_does_not_split_at_its_kink() {
    let f = map1(|x, p| p[0] / 2.0 * (1.0 - (2.0 * x - 1.0).abs()), 1);
    let g = map_graph(&f, &[2.0], (-0.05, 1.05), (-0.05, 1.05), 2048);
    assert_eq!(g.len(), 1);
    check_shape(&g, (-0.05, 1.05), (-0.05, 1.05));
    let peak = g[0].iter().map(|p| p.1).fold(f64::MIN, f64::max);
    assert!(peak > 0.999, "{peak}");
}

#[test]
fn a_fast_exponential_is_clipped_to_the_band() {
    let (x, y) = ((-0.05, 1.05), (-0.05, 1.05));
    let g = graph(|x| (50.0 * x).exp(), x, y, 2048);
    assert_eq!(g.len(), 1);
    check_shape(&g, x, y);
    let end = *g[0].last().unwrap();
    let top = 1.05 + 1.1;
    assert!((end.1 - top).abs() < 1e-12, "{end:?}");
    assert!((end.0 - top.ln() / 50.0).abs() < 1e-4, "{end:?}");
}

#[test]
fn a_step_splits_at_its_jump() {
    let (x, y) = ((0.0, 1.0), (0.0, 1.0));
    let g = graph(|x| if x < 0.5 { 0.2 } else { 0.8 }, x, y, 100);
    assert_eq!(g.len(), 2);
    no_segment_spans(&g, 0.5);
    let cell = 1.0 / 99.0;
    assert!(0.5 - g[0].last().unwrap().0 < cell / 2048.0);
    assert!(g[1][0].0 - 0.5 < cell / 2048.0);
}

#[test]
fn only_one_dimensional_maps_have_a_graph() {
    let henon = FnField::new(2, 2, SystemKind::Map, |_n, x, p, out| {
        out[0] = 1.0 - p[0] * x[0] * x[0] + x[1];
        out[1] = p[1] * x[0];
    });
    assert!(map_graph(&henon, &[1.4, 0.3], (-1.5, 1.5), (-1.5, 1.5), 100).is_empty());
    let flow = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, out| out[0] = -x[0]);
    assert!(map_graph(&flow, &[], (-1.0, 1.0), (-1.0, 1.0), 100).is_empty());
}

#[test]
fn nothing_is_drawn_where_f_is_undefined_or_out_of_view() {
    assert!(graph(|_| f64::NAN, (0.0, 1.0), (0.0, 1.0), 500).is_empty());
    assert!(graph(|x| 100.0 + x, (0.0, 1.0), (0.0, 1.0), 500).is_empty());
    assert!(graph(|x| x, (1.0, 1.0), (0.0, 1.0), 500).is_empty());
    assert!(graph(|x| x, (0.0, 1.0), (0.0, f64::NAN), 500).is_empty());
}

#[test]
fn samples_are_clamped() {
    let line = |n: usize| graph(|x| x, (0.0, 1.0), (0.0, 1.0), n)[0].len();
    assert_eq!(line(0), 2);
    assert_eq!(line(1), 2);
    assert_eq!(line(2), 2);
    assert_eq!(line(MAX_GRAPH_SAMPLES), MAX_GRAPH_SAMPLES);
    assert_eq!(line(MAX_GRAPH_SAMPLES + 1), MAX_GRAPH_SAMPLES);
    assert_eq!(line(usize::MAX), MAX_GRAPH_SAMPLES);
}

#[test]
fn the_cost_is_bounded() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static EVALS: AtomicUsize = AtomicUsize::new(0);
    // sin(1/x) jumps between every pair of samples near 0.
    let f = FnField::new(1, 0, SystemKind::Map, |_n, x, _p, out| {
        EVALS.fetch_add(1, Ordering::Relaxed);
        out[0] = (1.0 / x[0]).sin();
    });
    let samples = 1000;
    let g = map_graph(&f, &[], (-1.0, 1.0), (-1.0, 1.0), samples);
    check_shape(&g, (-1.0, 1.0), (-1.0, 1.0));
    let evals = EVALS.load(Ordering::Relaxed);
    assert!(evals <= samples * (1 + 12 + 1), "{evals} evaluations");
}

#[test]
fn a_pole_in_a_zoomed_out_view_splits() {
    // The samples either side of the pole are about ±20 in a view 200
    // high, under the old Δy/4 floor; screen (2048) and export (4096)
    // samples agree.
    let (x, y) = ((-100.0, 100.0), (-100.0, 100.0));
    for samples in [2048, 4096] {
        let g = graph(|x| 1.0 / (x - 30.0), x, y, samples);
        assert_eq!(g.len(), 2, "{samples} samples");
        check_shape(&g, x, y);
        no_segment_spans(&g, 30.0);
    }
    for (x, y) in [
        ((-100.0, 100.0), (-100.0, 100.0)),
        ((-300.0, 300.0), (-300.0, 300.0)),
    ] {
        let g = graph(f64::tan, x, y, 2048);
        check_shape(&g, x, y);
        let half_pi = std::f64::consts::FRAC_PI_2;
        let poles: Vec<f64> = (-200..200)
            .map(|k| half_pi + k as f64 * std::f64::consts::PI)
            .filter(|p| *p > x.0 && *p < x.1)
            .collect();
        for &p in &poles {
            no_segment_spans(&g, p);
        }
        assert_eq!(g.len(), poles.len() + 1, "tan on {x:?}");
    }
}

#[test]
fn a_jump_splits_even_with_a_midpoint_value_on_it() {
    // A view symmetric about the jump puts a bisection midpoint on 0, where
    // sign(0) = 0 halves the rise.
    let (x, y) = ((-2.0, 2.0), (-2.0, 2.0));
    let g = graph(f64::signum_or_zero, x, y, 2048);
    assert_eq!(g.len(), 2, "{g:?}");
    no_segment_spans(&g, 0.0);
    assert!(g[0].iter().all(|&(_, y)| y == -1.0), "{:?}", g[0].last());
    assert!(g[1].iter().all(|&(_, y)| y == 1.0), "{:?}", g[1].first());
    // Jumps under a quarter of the view's height.
    let x = (-2.0, 2.1);
    let g = graph(|x| 0.3 * x.signum_or_zero(), x, y, 2048);
    assert_eq!(g.len(), 2);
    no_segment_spans(&g, 0.0);
    let g = graph(|x| x + 0.3 * (x - 0.4).signum_or_zero(), x, y, 2048);
    assert_eq!(g.len(), 2);
    no_segment_spans(&g, 0.4);
}

trait SignumOrZero {
    fn signum_or_zero(self) -> f64;
}

impl SignumOrZero for f64 {
    /// `sign` as the expression language has it: 0 at 0.
    fn signum_or_zero(self) -> f64 {
        if self == 0.0 {
            0.0
        } else {
            self.signum()
        }
    }
}
