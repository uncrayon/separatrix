use dyn_analysis::*;
use dyn_core::{FnField, SystemKind};

#[test]
fn fixed_point_eigenvalues_are_finite_or_point_is_flagged() {
    let one_d = FnField::new(1, 0, SystemKind::Ode, |_t, x, _p, o| o[0] = x[0].cbrt())
        .with_jacobian(|_t, x, _p, j| j[0] = 1.0 / (3.0 * x[0].cbrt() * x[0].cbrt()));
    let three_d = FnField::new(3, 0, SystemKind::Ode, |_t, x, _p, o| {
        o[0] = x[0].cbrt();
        o[1] = -x[1];
        o[2] = -x[2];
    })
    .with_jacobian(|_t, x, _p, j| {
        let d = 1.0 / (3.0 * x[0].cbrt() * x[0].cbrt());
        j.copy_from_slice(&[d, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0]);
    });
    let with_null = |f: &FnField, search: &[(f64, f64)], seeds: usize| -> Vec<String> {
        fixed_points(f, &[], search, seeds)
            .iter()
            .map(|fp| serde_json::to_string(fp).unwrap())
            .filter(|json| json.contains("null"))
            .collect()
    };
    let bad_1d = with_null(&one_d, &[(-1.0, 1.0)], 3);
    let bad_3d = with_null(&three_d, &[(-1.0, 1.0); 3], 9);
    assert!(
        bad_1d.is_empty() && bad_3d.is_empty(),
        "fixed points serialise with null eigenvalues:\n1D: {bad_1d:?}\n3D: {bad_3d:?}"
    );
}

#[test]
fn diagonal_jacobian_is_never_a_degenerate_node() {
    for a in [-1.00001, -1.000001, -0.99999] {
        let (_, class) = classify_jacobian(SystemKind::Ode, 2, &[a, 0.0, 0.0, -1.0]);
        assert!(
            matches!(
                class,
                Classification::StableNode | Classification::StableStar
            ),
            "diag({a}, -1) classified as {class}"
        );
    }
}

#[test]
fn ghost_of_saddle_node_is_not_a_fixed_point_when_zoomed_out() {
    let f = FnField::new(2, 1, SystemKind::Ode, |_t, x, p, o| {
        o[0] = p[0] + x[0] * x[0];
        o[1] = -x[1];
    })
    .with_jacobian(|_t, x, _p, j| j.copy_from_slice(&[2.0 * x[0], 0.0, 0.0, -1.0]));
    let r = 1e-6;
    let near = fixed_points(&f, &[r], &[(-2.0, 2.0), (-2.0, 2.0)], 14);
    assert!(near.is_empty(), "box ±2: {near:?}");
    let far = fixed_points(&f, &[r], &[(-100.0, 100.0), (-100.0, 100.0)], 14);
    assert!(
        far.is_empty(),
        "box ±100 reports a fixed point that does not exist: {far:?}"
    );
}
