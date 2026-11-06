//! `CompiledSystem::is_separable`: which Hamiltonians split as T(p) + V(q),
//! so a symplectic split-step method may integrate them.

mod common;

use common::*;

fn separable(src: &str) -> bool {
    text(src).is_separable()
}

#[test]
fn kinetic_plus_potential_is_separable() {
    assert!(separable("H(q, p) = p^2/2 - cos(q)"));
    assert!(latex(r"H(q,p)=\frac{p^2}{2}-\cos q").is_separable());
    assert!(separable("H(q, p) = p^2/(2*m) + q^4"));
    assert!(separable(
        "H(q_1, q_2, p_1, p_2) = (p_1^2 + p_2^2)/2 + (q_1^2 + q_2^2)/2 + lambda*(q_1^2*q_2 - q_2^3/3)"
    ));
    assert!(separable("H(q, p) = -(q^2/2 - p^2/2)"));
}

#[test]
fn a_constant_factor_over_both_halves_still_splits() {
    assert!(separable("H(q, p) = (p^2 + q^2)/2"));
    assert!(separable("H(q, p) = k*(p^2 + q^2)"));
}

#[test]
fn a_term_reading_both_halves_is_not_separable() {
    assert!(!separable("H(q, p) = q*p"));
    assert!(!separable("H(q, p) = (p^2 + q^2)^2"));
    assert!(!separable("H(q, p) = p^2*q"));
    assert!(!separable("H(q, p) = p^2/2 + q^2/2 + q*p"));
    assert!(!separable("H(q, p) = p^2/(2*q)"));
}

#[test]
fn a_hamiltonian_that_reads_t_is_not_separable() {
    assert!(!separable("H(q, p) = p^2/2 + q^2/2*cos(t)"));
    assert!(!separable("H(q, p) = p^2/2 + t"));
}

#[test]
fn equations_of_motion_are_not_a_separable_hamiltonian() {
    assert!(!separable("x' = y; y' = -sin(x)"));
    assert!(!separable("x'' = -x"));
}
