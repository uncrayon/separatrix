//! Regression tests written during the numerics review (milestone 1).
//!
//! Each test demonstrates a defect found in review and is expected to FAIL
//! until the corresponding fix lands. See the comment on each test for the
//! failing scenario and the code it points at.

use dyn_analysis::*;
use dyn_core::{FnField, SystemKind};

/// A fixed point whose Jacobian is not finite (here x' = cbrt(x) with its
/// exact derivative 1 / (3 cbrt(x)²), as dyn-expr's automatic
/// differentiation produces, Strogatz Example 2.5.1) is returned with
/// non-finite eigenvalues: `[(inf, 0)]` in 1D, NaN in 2D/3D
/// (`fixed.rs`, `fixed_points`, and `linalg.rs`, `eigenvalues`). serde_json
/// writes those as `null`, which the UI's `[number, number]` type does not
/// allow. The root is found whenever a seed lands exactly on it (an odd
/// number of seeds per axis on a symmetric box, e.g. the app's 9 seeds per
/// axis in 3D).
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
    // JSON of every fixed point that contains `null`.
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

/// Eigenvalues closer than about sqrt(REL_TOL)·s ≈ 3e-5·s count as
/// "repeated" (`classify.rs`, `classify_2d`, `disc.abs() <= REL_TOL * s * s`),
/// but the star test uses the much tighter REL_TOL·s on the matrix entries
/// (`classify_jacobian`). So J = diag(-1, -1.00001), a stable node with two
/// independent eigendirections (Strogatz Example 5.1.1, x' = ax, y' = -y
/// with a = -1.00001), is labelled "stable degenerate node", a class that by
/// definition has only one eigendirection. It should be a stable node (or,
/// if it is treated as repeated, a star); never degenerate.
#[test]
fn diagonal_jacobian_is_never_a_degenerate_node() {
    for a in [-1.00001, -1.000001, -0.99999] {
        let (_, class) = classify_jacobian(SystemKind::Ode, 2, &[a, 0.0, 0.0, -1.0]);
        assert!(
            matches!(class, Classification::StableNode | Classification::StableStar),
            "diag({a}, -1) classified as {class}"
        );
    }
}

/// Newton accepts a point when `‖F‖∞ ≤ 1e-9 · max(1, ‖F(seed)‖∞)`
/// (`fixed.rs`, `RESIDUAL_TOL` / `f_scale`). The tolerance grows with the
/// residual at the *seed*, i.e. with the size of the search box, so whether
/// a fixed point is reported depends on how far the user has zoomed out.
/// x' = r + x², y' = -y with r = 1e-6 > 0 has no fixed point (Strogatz §3.1,
/// past the saddle-node bifurcation). With the box ±2 none is found, but with
/// the box ±100 Newton stalls at the bottleneck x ≈ 0 with residual
/// r = 1e-6 ≤ 1e-9 · |F(seed)| ≈ 8.6e-6 and a spurious fixed point is
/// reported.
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
    assert!(far.is_empty(), "box ±100 reports a fixed point that does not exist: {far:?}");
}
