use dyn_analysis::{orbit_diagram, period_doublings, OrbitDiagram, OrbitDiagramOptions};
use dyn_core::{FnField, SystemKind};
use dyn_solve::BLOW_UP_THRESHOLD;

fn logistic() -> FnField {
    FnField::new(1, 1, SystemKind::Map, |_n, x, p, o| {
        o[0] = p[0] * x[0] * (1.0 - x[0])
    })
    .with_jacobian(|_n, x, p, j| j[0] = p[0] * (1.0 - 2.0 * x[0]))
}

fn diagram(range: (f64, f64), columns: usize, transient: usize, kept: usize) -> OrbitDiagram {
    let opts = OrbitDiagramOptions {
        param: 0,
        range,
        columns,
        state: 0,
        transient,
        kept,
    };
    orbit_diagram(&logistic(), &[0.0], &[0.1], &opts)
}

#[test]
fn a_stable_fixed_point_is_one_value() {
    let r = 2.5;
    let d = diagram((r, r), 1, 500, 32);
    assert_eq!(d.r, [r]);
    let fixed = 1.0 - 1.0 / r;
    for x in d.column(0) {
        assert!((x - fixed).abs() < 1e-12, "{x} vs x* = {fixed}");
    }
}

#[test]
fn a_two_cycle_alternates_between_its_points() {
    let r: f64 = 3.2;
    let d = diagram((r, r), 1, 1000, 32);
    let root = ((r - 3.0) * (r + 1.0)).sqrt();
    let (p, q) = ((r + 1.0 + root) / (2.0 * r), (r + 1.0 - root) / (2.0 * r));
    let column = d.column(0);
    let start = if (column[0] - p).abs() < (column[0] - q).abs() {
        [p, q]
    } else {
        [q, p]
    };
    for (n, x) in column.iter().enumerate() {
        let want = start[n % 2];
        assert!((x - want).abs() < 1e-12, "x_{n} = {x}, want {want}");
    }
}

#[test]
fn an_escaping_orbit_is_nan_after_it_leaves() {
    let r: f64 = 4.5;
    let d = diagram((r, r), 1, 0, 50);
    let column = d.column(0);
    let escaped = column.iter().position(|x| x.is_nan()).expect("escapes");
    assert!(escaped > 3, "{column:?}");
    assert!(column[escaped..].iter().all(|x| x.is_nan()), "{column:?}");
    let mut x: f64 = 0.1;
    for (n, &kept) in column[..escaped].iter().enumerate() {
        assert_eq!(kept.to_bits(), x.to_bits(), "x_{n}");
        x = r * x * (1.0 - x);
    }
    assert!(
        x.abs() > BLOW_UP_THRESHOLD,
        "x_{escaped} = {x} was within the threshold"
    );

    let late = diagram((r, r), 1, 100, 8);
    assert!(late.column(0).iter().all(|x| x.is_nan()));
}

#[test]
fn columns_are_lo_plus_j_steps_bit_for_bit() {
    let (lo, hi, columns) = (2.8, 3.57, 2000);
    let d = diagram((lo, hi), columns, 0, 1);
    let step = (hi - lo) / (columns - 1) as f64;
    assert_eq!(d.r.len(), columns);
    for (j, r) in d.r.iter().enumerate() {
        assert_eq!(r.to_bits(), (lo + j as f64 * step).to_bits(), "r_{j}");
    }
    assert_eq!(d.x.len(), columns);
    assert_eq!(d.x[0], 0.1);
}

#[test]
fn the_same_diagram_has_the_same_bits() {
    let (lo, hi, columns, transient, kept) = (3.4, 3.9, 5, 7, 3);
    let first = diagram((lo, hi), columns, transient, kept);
    let second = diagram((lo, hi), columns, transient, kept);
    let bits =
        |d: &OrbitDiagram| -> Vec<u64> { d.r.iter().chain(&d.x).map(|v| v.to_bits()).collect() };
    assert_eq!(bits(&first), bits(&second));
    assert_eq!(first.x.len(), columns * kept);
    for j in 0..columns {
        let r = first.r[j];
        let mut x: f64 = 0.1;
        for _ in 0..transient {
            x = r * x * (1.0 - x);
        }
        for (n, kept) in first.column(j).iter().enumerate() {
            assert_eq!(kept.to_bits(), x.to_bits(), "column {j}, kept iterate {n}");
            x = r * x * (1.0 - x);
        }
    }
}

#[test]
fn the_logistic_doublings_are_r1_to_r4() {
    let d = diagram((2.8, 3.57), 2000, 2000, 64);
    let found = period_doublings(&logistic(), &[0.0], 0, &d, 4);
    let published = [
        (1, 3.0, 1e-9),
        (2, 1.0 + 6f64.sqrt(), 1e-9),
        (4, 3.544_090_3, 1e-6),
        (8, 3.564_407_3, 1e-6),
    ];
    assert_eq!(found.len(), published.len(), "{found:?}");
    for (doubling, (period, r, tol)) in found.iter().zip(published) {
        assert_eq!(doubling.period, period, "{found:?}");
        assert!(
            (doubling.r - r).abs() < tol,
            "the {period}-cycle doubles at {} (want {r} ± {tol})",
            doubling.r
        );
    }
}

#[test]
fn doublings_need_a_one_dimensional_map() {
    let henon = FnField::new(2, 2, SystemKind::Map, |_n, x, p, o| {
        o[0] = 1.0 - p[0] * x[0] * x[0] + x[1];
        o[1] = p[1] * x[0];
    });
    let opts = OrbitDiagramOptions {
        param: 0,
        range: (0.2, 1.2),
        columns: 200,
        state: 0,
        transient: 500,
        kept: 16,
    };
    let d = orbit_diagram(&henon, &[0.0, 0.3], &[0.0, 0.0], &opts);
    assert!(d.x.iter().any(|x| x.is_finite()));
    assert!(period_doublings(&henon, &[0.0, 0.3], 0, &d, 4).is_empty());
}
