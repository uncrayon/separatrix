//! Classification of fixed points from their linearisation.
//!
//! # Rules
//!
//! **2D flows** follow the trace–determinant plane of Strogatz (§5.2), with
//! `τ = tr J`, `Δ = det J`:
//!
//! | condition | class |
//! | --- | --- |
//! | `Δ ≈ 0` | non-isolated |
//! | `Δ < 0` | saddle |
//! | `Δ > 0`, `τ ≈ 0` | center |
//! | `Δ > 0`, `τ² - 4Δ ≈ 0` | star if `J` is a multiple of `I`, else degenerate node |
//! | `Δ > 0`, `τ² - 4Δ > 0` | node |
//! | `Δ > 0`, `τ² - 4Δ < 0` | spiral |
//!
//! Nodes, spirals, stars and degenerate nodes are *stable* when `τ < 0`.
//!
//! **Other dimensions of flows**: all `Re λ < 0` sink, all `> 0` source,
//! mixed signs `saddle (n-D)`, any `Re λ ≈ 0` non-hyperbolic.
//!
//! **Maps** (any dimension) use the multipliers `μ` (eigenvalues of `Df`):
//! all `|μ| < 1` sink, all `> 1` source, mixed `saddle (n-D)`, any
//! `|μ| ≈ 1` non-hyperbolic.
//!
//! # Tolerances
//!
//! "≈" is relative to the size of the linearisation, `s`:
//! for 2D flows `s = max(|τ|, sqrt|Δ|)` (roughly the largest |λ|) and
//! `Δ ≈ 0` means `|Δ| ≤ REL_TOL·s²`, `τ ≈ 0` means `|τ| ≤ REL_TOL·s`,
//! `τ² - 4Δ ≈ 0` means `|τ² - 4Δ| ≤ REL_TOL·s²` (so eigenvalues closer than
//! about `δ = sqrt(REL_TOL)·s ≈ 3e-5·s` count as repeated: repeated
//! eigenvalues are very sensitive to rounding). "Multiple of `I`" is decided
//! at the same resolution: every entry of `N = J - (τ/2)·I` is `≤ δ`. With
//! the same `δ` for both decisions a diagonalisable matrix (two independent
//! eigenvectors, e.g. any diagonal or symmetric `J`) whose eigenvalues count
//! as repeated is always a star, never a degenerate node: its `N` has
//! entries of at most half the eigenvalue gap.
//! For other flows `Re λ ≈ 0` means `|Re λ| ≤ max(REL_TOL·max|λ|, ABS_TOL)`.
//! For maps `|μ| ≈ 1` means `||μ| - 1| ≤ REL_TOL`.

use dyn_core::SystemKind;
use serde::{Deserialize, Serialize};

use crate::linalg;

/// Relative tolerance used for all "≈ 0" decisions (see the module docs).
pub const REL_TOL: f64 = 1e-9;
/// Absolute tolerance below which a real part counts as zero for flows that
/// are not 2D (needed in 1D, where there is nothing to be relative to).
pub const ABS_TOL: f64 = 1e-12;

/// Type of a fixed point. Serialises to the strings of the UI's
/// `Classification` type (e.g. `"stable spiral"`, `"saddle (n-D)"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Classification {
    /// 2D flow, `Δ < 0`.
    #[serde(rename = "saddle")]
    Saddle,
    /// 2D flow, two distinct negative real eigenvalues.
    #[serde(rename = "stable node")]
    StableNode,
    /// 2D flow, two distinct positive real eigenvalues.
    #[serde(rename = "unstable node")]
    UnstableNode,
    /// 2D flow, complex eigenvalues with negative real part.
    #[serde(rename = "stable spiral")]
    StableSpiral,
    /// 2D flow, complex eigenvalues with positive real part.
    #[serde(rename = "unstable spiral")]
    UnstableSpiral,
    /// 2D flow, purely imaginary eigenvalues.
    #[serde(rename = "center")]
    Centre,
    /// 2D flow, `J = λI` with `λ < 0`.
    #[serde(rename = "stable star")]
    StableStar,
    /// 2D flow, `J = λI` with `λ > 0`.
    #[serde(rename = "unstable star")]
    UnstableStar,
    /// 2D flow, repeated negative eigenvalue, one eigenvector.
    #[serde(rename = "stable degenerate node")]
    StableDegenerateNode,
    /// 2D flow, repeated positive eigenvalue, one eigenvector.
    #[serde(rename = "unstable degenerate node")]
    UnstableDegenerateNode,
    /// 2D flow, `Δ = 0`: a zero eigenvalue (a line or plane of fixed points
    /// in the linear system).
    #[serde(rename = "non-isolated")]
    NonIsolated,
    /// Every direction attracts (flow: all `Re λ < 0`; map: all `|μ| < 1`).
    #[serde(rename = "sink")]
    Sink,
    /// Every direction repels.
    #[serde(rename = "source")]
    Source,
    /// Some directions attract and some repel (not a 2D flow).
    #[serde(rename = "saddle (n-D)")]
    SaddleND,
    /// Some eigenvalue on the stability boundary (`Re λ = 0` or `|μ| = 1`).
    #[serde(rename = "non-hyperbolic")]
    NonHyperbolic,
}

impl Classification {
    /// `true` for the asymptotically stable ("sink-like") classes: stable
    /// node, spiral, star, degenerate node, and sink. Centres and
    /// non-hyperbolic points are reported as not stable.
    pub fn is_stable(self) -> bool {
        use Classification::*;
        matches!(self, StableNode | StableSpiral | StableStar | StableDegenerateNode | Sink)
    }

    /// The display string, identical to the serialised form.
    pub fn as_str(self) -> &'static str {
        use Classification::*;
        match self {
            Saddle => "saddle",
            StableNode => "stable node",
            UnstableNode => "unstable node",
            StableSpiral => "stable spiral",
            UnstableSpiral => "unstable spiral",
            Centre => "center",
            StableStar => "stable star",
            UnstableStar => "unstable star",
            StableDegenerateNode => "stable degenerate node",
            UnstableDegenerateNode => "unstable degenerate node",
            NonIsolated => "non-isolated",
            Sink => "sink",
            Source => "source",
            SaddleND => "saddle (n-D)",
            NonHyperbolic => "non-hyperbolic",
        }
    }
}

impl std::fmt::Display for Classification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classifies a fixed point from the eigenvalues `(re, im)` of its
/// linearisation (`J` for flows, `Df` for maps). See the module docs.
///
/// Eigenvalues alone cannot tell a star from a degenerate node (both have a
/// repeated eigenvalue), so a repeated eigenvalue of a 2D flow is reported
/// as a degenerate node. Use [`classify_jacobian`] when the matrix is known.
pub fn classify(kind: SystemKind, eigenvalues: &[(f64, f64)]) -> Classification {
    match (kind, eigenvalues) {
        (SystemKind::Ode, [(a, b), (c, d)]) => {
            let tau = a + c;
            let delta = a * c - b * d; // Re(λ1 λ2)
            classify_2d(tau, delta, false)
        }
        (SystemKind::Ode, ev) => classify_flow_nd(ev),
        (SystemKind::Map, ev) => classify_map(ev),
    }
}

/// Eigenvalues and classification of a fixed point from its `n x n`
/// row-major linearisation `jac` (`J` for flows, `Df` for maps).
///
/// Unlike [`classify`], this distinguishes stars (`J = λI`) from degenerate
/// nodes.
///
/// If `jac` has a non-finite entry (an infinite slope, as for `x' = ∛x` at
/// 0), or its eigenvalues overflow, there is no usable linearisation: the
/// result is `(vec![], NonHyperbolic)`, so that no NaN or infinity reaches
/// the UI (JSON would turn it into `null`).
pub fn classify_jacobian(kind: SystemKind, n: usize, jac: &[f64]) -> (Vec<(f64, f64)>, Classification) {
    let no_linearisation = (Vec::new(), Classification::NonHyperbolic);
    if !jac.iter().all(|v| v.is_finite()) {
        return no_linearisation;
    }
    let ev = linalg::eigenvalues(n, jac);
    if !ev.iter().all(|(re, im)| re.is_finite() && im.is_finite()) {
        return no_linearisation;
    }
    let class = if kind == SystemKind::Ode && n == 2 {
        let tau = jac[0] + jac[3];
        let delta = jac[0] * jac[3] - jac[1] * jac[2];
        // J = λI up to the resolution at which eigenvalues count as repeated.
        let resolution = REL_TOL.sqrt() * scale_2d(tau, delta);
        let scalar = jac[1].abs() <= resolution
            && jac[2].abs() <= resolution
            && 0.5 * (jac[0] - jac[3]).abs() <= resolution;
        classify_2d(tau, delta, scalar)
    } else {
        classify(kind, &ev)
    };
    (ev, class)
}

/// Size of a 2x2 linearisation, roughly its largest |eigenvalue|.
fn scale_2d(tau: f64, delta: f64) -> f64 {
    tau.abs().max(delta.abs().sqrt())
}

/// Strogatz trace–determinant classification. `scalar` says whether `J` is
/// a multiple of the identity (only matters for repeated eigenvalues).
fn classify_2d(tau: f64, delta: f64, scalar: bool) -> Classification {
    use Classification::*;
    if !tau.is_finite() || !delta.is_finite() {
        return NonHyperbolic;
    }
    let s = scale_2d(tau, delta);
    if s == 0.0 || delta.abs() <= REL_TOL * s * s {
        return NonIsolated;
    }
    if delta < 0.0 {
        return Saddle;
    }
    if tau.abs() <= REL_TOL * s {
        return Centre;
    }
    let stable = tau < 0.0;
    let disc = tau * tau - 4.0 * delta;
    let pick = |st, un| if stable { st } else { un };
    if disc.abs() <= REL_TOL * s * s {
        if scalar {
            pick(StableStar, UnstableStar)
        } else {
            pick(StableDegenerateNode, UnstableDegenerateNode)
        }
    } else if disc > 0.0 {
        pick(StableNode, UnstableNode)
    } else {
        pick(StableSpiral, UnstableSpiral)
    }
}

/// Sink/source/saddle by the signs of the real parts (flows, `n != 2`).
fn classify_flow_nd(ev: &[(f64, f64)]) -> Classification {
    let s = ev.iter().map(|&(re, im)| re.hypot(im)).fold(0.0, f64::max);
    let zero = (REL_TOL * s).max(ABS_TOL);
    by_sign(ev.iter().map(|&(re, _)| re), zero)
}

/// Sink/source/saddle by `|μ| - 1` (maps).
fn classify_map(ev: &[(f64, f64)]) -> Classification {
    by_sign(ev.iter().map(|&(re, im)| re.hypot(im) - 1.0), REL_TOL)
}

/// Shared rule: all values negative → sink, all positive → source, mixed →
/// saddle, any within `zero` of 0 (or non-finite) → non-hyperbolic.
fn by_sign(values: impl Iterator<Item = f64>, zero: f64) -> Classification {
    let (mut neg, mut pos) = (0, 0);
    for v in values {
        if !v.is_finite() || v.abs() <= zero {
            return Classification::NonHyperbolic;
        }
        if v < 0.0 {
            neg += 1;
        } else {
            pos += 1;
        }
    }
    match (neg, pos) {
        (0, 0) => Classification::NonHyperbolic,
        (_, 0) => Classification::Sink,
        (0, _) => Classification::Source,
        _ => Classification::SaddleND,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Classification::*;

    fn ode2(j: [f64; 4]) -> Classification {
        classify_jacobian(SystemKind::Ode, 2, &j).1
    }

    #[test]
    fn linear_2d_classes() {
        assert_eq!(ode2([1.0, 0.0, 0.0, -2.0]), Saddle);
        assert_eq!(ode2([0.0, 1.0, 1.0, 0.0]), Saddle);
        assert_eq!(ode2([-1.0, 0.0, 0.0, -3.0]), StableNode);
        assert_eq!(ode2([2.0, 1.0, 0.0, 3.0]), UnstableNode);
        assert_eq!(ode2([-0.1, 1.0, -1.0, -0.1]), StableSpiral);
        assert_eq!(ode2([0.5, -2.0, 2.0, 0.5]), UnstableSpiral);
        assert_eq!(ode2([0.0, 1.0, -4.0, 0.0]), Centre);
        assert_eq!(ode2([-2.0, 0.0, 0.0, -2.0]), StableStar);
        assert_eq!(ode2([3.0, 0.0, 0.0, 3.0]), UnstableStar);
        assert_eq!(ode2([-1.0, 1.0, 0.0, -1.0]), StableDegenerateNode);
        assert_eq!(ode2([1.0, 0.0, 5.0, 1.0]), UnstableDegenerateNode);
        assert_eq!(ode2([0.0, 1.0, 0.0, -1.0]), NonIsolated); // Δ = 0, τ ≠ 0
        assert_eq!(ode2([1.0, 2.0, 2.0, 4.0]), NonIsolated);
        assert_eq!(ode2([0.0, 0.0, 0.0, 0.0]), NonIsolated);
        assert_eq!(ode2([0.0, 1.0, 0.0, 0.0]), NonIsolated);
    }

    #[test]
    fn tolerances_are_relative() {
        // Same shapes scaled by 1e-6 and 1e6 classify the same.
        for k in [1e-6, 1.0, 1e6] {
            assert_eq!(ode2([0.0, k, -k, 0.0]), Centre);
            assert_eq!(ode2([-0.1 * k, k, -k, -0.1 * k]), StableSpiral);
            assert_eq!(ode2([-k, k, 0.0, -k]), StableDegenerateNode);
            assert_eq!(ode2([-k, 0.0, 0.0, -k]), StableStar);
        }
        // Rounding-level perturbations do not change the class.
        assert_eq!(ode2([1e-15, 1.0, -1.0, 0.0]), Centre);
        assert_eq!(ode2([-1.0, 1e-14, 0.0, -1.0 + 1e-14]), StableStar);
        assert_eq!(ode2([1e-13, 1.0, 0.0, 0.0]), NonIsolated);
        // Genuinely distinct eigenvalues stay a node.
        assert_eq!(ode2([-1.0, 0.0, 0.0, -1.01]), StableNode);
    }

    #[test]
    fn stability_flag() {
        for c in [StableNode, StableSpiral, StableStar, StableDegenerateNode, Sink] {
            assert!(c.is_stable(), "{c}");
        }
        for c in [Saddle, UnstableNode, Centre, NonIsolated, Source, SaddleND, NonHyperbolic] {
            assert!(!c.is_stable(), "{c}");
        }
    }

    #[test]
    fn classify_from_eigenvalues_2d() {
        let k = SystemKind::Ode;
        assert_eq!(classify(k, &[(1.0, 0.0), (-1.0, 0.0)]), Saddle);
        assert_eq!(classify(k, &[(-1.0, 2.0), (-1.0, -2.0)]), StableSpiral);
        assert_eq!(classify(k, &[(0.0, 2.0), (0.0, -2.0)]), Centre);
        assert_eq!(classify(k, &[(-2.0, 0.0), (-2.0, 0.0)]), StableDegenerateNode);
        assert_eq!(classify(k, &[(0.0, 0.0), (-2.0, 0.0)]), NonIsolated);
        assert_eq!(classify(k, &[(3.0, 0.0), (1.0, 0.0)]), UnstableNode);
    }

    #[test]
    fn flows_in_other_dimensions() {
        let k = SystemKind::Ode;
        assert_eq!(classify(k, &[(-1.0, 0.0)]), Sink);
        assert_eq!(classify(k, &[(2.0, 0.0)]), Source);
        assert_eq!(classify(k, &[(0.0, 0.0)]), NonHyperbolic);
        assert_eq!(classify(k, &[(1e-13, 0.0)]), NonHyperbolic);
        assert_eq!(classify(k, &[(-1.0, 1.0), (-1.0, -1.0), (-3.0, 0.0)]), Sink);
        assert_eq!(classify(k, &[(1.0, 1.0), (1.0, -1.0), (3.0, 0.0)]), Source);
        assert_eq!(classify(k, &[(-1.0, 1.0), (-1.0, -1.0), (3.0, 0.0)]), SaddleND);
        assert_eq!(classify(k, &[(0.0, 1.0), (0.0, -1.0), (-3.0, 0.0)]), NonHyperbolic);
        assert_eq!(classify(k, &[]), NonHyperbolic);
    }

    #[test]
    fn maps_by_modulus() {
        let k = SystemKind::Map;
        assert_eq!(classify(k, &[(0.5, 0.0)]), Sink);
        assert_eq!(classify(k, &[(-0.99, 0.0)]), Sink);
        assert_eq!(classify(k, &[(-1.5, 0.0)]), Source);
        assert_eq!(classify(k, &[(1.0, 0.0)]), NonHyperbolic);
        assert_eq!(classify(k, &[(-1.0, 0.0)]), NonHyperbolic);
        assert_eq!(classify(k, &[(0.6, 0.8), (0.6, -0.8)]), NonHyperbolic); // |μ| = 1
        assert_eq!(classify(k, &[(0.3, 0.4), (0.3, -0.4)]), Sink);
        assert_eq!(classify(k, &[(2.0, 0.0), (0.1, 0.0)]), SaddleND);
        assert_eq!(classify(k, &[(0.0, 0.0), (0.0, 0.0)]), Sink); // superstable
    }

    #[test]
    fn serialises_to_ui_strings() {
        let all = [
            Saddle,
            StableNode,
            UnstableNode,
            StableSpiral,
            UnstableSpiral,
            Centre,
            StableStar,
            UnstableStar,
            StableDegenerateNode,
            UnstableDegenerateNode,
            NonIsolated,
            Sink,
            Source,
            SaddleND,
            NonHyperbolic,
        ];
        for c in all {
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(json, format!("\"{}\"", c.as_str()));
            let back: Classification = serde_json::from_str(&json).unwrap();
            assert_eq!(back, c);
        }
        assert_eq!(serde_json::to_string(&Centre).unwrap(), "\"center\"");
        assert_eq!(serde_json::to_string(&SaddleND).unwrap(), "\"saddle (n-D)\"");
    }

    #[test]
    fn diagonalisable_matrices_are_never_degenerate_nodes() {
        // Diagonal and symmetric J have two independent eigenvectors, so a
        // (nearly) repeated eigenvalue must give a star, never a degenerate
        // node; well-separated eigenvalues give a node.
        for k in [1e-6, 1.0, 1e6] {
            for gap in [0.0, 1e-12, 1e-9, 1e-7, 1e-6, 1e-5, 3e-5, 6e-5, 1e-4, 1e-3] {
                for sign in [-1.0, 1.0] {
                    let (a, d) = (sign * k, sign * k * (1.0 + gap));
                    for j in [[a, 0.0, 0.0, d], [a, 0.5 * k * gap, 0.5 * k * gap, d]] {
                        let class = ode2(j);
                        assert!(
                            !matches!(class, StableDegenerateNode | UnstableDegenerateNode),
                            "{j:?}: {class}"
                        );
                    }
                }
            }
        }
        // A genuine Jordan block stays degenerate, even a fairly mild one.
        assert_eq!(ode2([-1.0, 1e-3, 0.0, -1.0]), StableDegenerateNode);
        assert_eq!(ode2([2.0, 0.0, 0.1, 2.0]), UnstableDegenerateNode);
    }

    #[test]
    fn non_finite_jacobian_has_no_eigenvalues() {
        let (ev, class) = classify_jacobian(SystemKind::Ode, 1, &[f64::INFINITY]);
        assert!(ev.is_empty());
        assert_eq!(class, NonHyperbolic);
        let (ev, class) = classify_jacobian(SystemKind::Ode, 2, &[f64::NAN, 0.0, 0.0, -1.0]);
        assert!(ev.is_empty());
        assert_eq!(class, NonHyperbolic);
        // Finite entries whose eigenvalue computation overflows.
        let (ev, class) = classify_jacobian(SystemKind::Ode, 2, &[1e200, 1e200, -1e200, 1e200]);
        assert!(ev.is_empty());
        assert_eq!(class, NonHyperbolic);
        let (ev, class) = classify_jacobian(SystemKind::Map, 3, &[f64::INFINITY; 9]);
        assert!(ev.is_empty());
        assert_eq!(class, NonHyperbolic);
    }
}
