//! The Strogatz gate: each gallery template from chapters 2 to 8 reproduces
//! a quantitative result of *Nonlinear Dynamics and Chaos*, 2nd edition
//! (Westview Press, 2015, ISBN 9780813349107). Every expected value comes
//! from the book's formula, except the relaxation oscillator's period at
//! μ = 10: the book gives only its asymptote, so the gate checks the known
//! value 19.0784 and that DOPRI5 and RK4 agree on it.
//!
//! The section titles in [`STROGATZ_2E`] were checked against the 2nd
//! edition's contents pages, as scanned by the Czech library cover service:
//! <https://obalkyknih.cz/file/toc/452162/pdf>.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

use dyn_analysis::FIXED_POINT_MERGE_REL;
use dyn_expr::CompiledSystem;
use dyn_solve::Sampling;
use separatrix_lib::ipc::{
    AxisRef, Classification, CompileRequest, CompileResponse, Direction, FixedPointOut,
    InputFormat, Method, PortraitRequest, SectionSpec, Severity, SolverOptions, Status,
    TrajectoryOut, TrajectoryRequest,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

const STROGATZ_2E: &[(&str, &str)] = &[
    ("2.1", "A Geometric Way of Thinking"),
    ("2.2", "Fixed Points and Stability"),
    ("2.3", "Population Growth"),
    ("2.4", "Linear Stability Analysis"),
    ("2.5", "Existence and Uniqueness"),
    ("2.6", "Impossibility of Oscillations"),
    ("2.7", "Potentials"),
    ("2.8", "Solving Equations on the Computer"),
    ("3.1", "Saddle-Node Bifurcation"),
    ("3.2", "Transcritical Bifurcation"),
    ("3.3", "Laser Threshold"),
    ("3.4", "Pitchfork Bifurcation"),
    ("3.5", "Overdamped Bead on a Rotating Hoop"),
    ("3.6", "Imperfect Bifurcations and Catastrophes"),
    ("3.7", "Insect Outbreak"),
    ("4.1", "Examples and Definitions"),
    ("4.2", "Uniform Oscillator"),
    ("4.3", "Nonuniform Oscillator"),
    ("4.4", "Overdamped Pendulum"),
    ("4.5", "Fireflies"),
    ("4.6", "Superconducting Josephson Junctions"),
    ("5.1", "Definitions and Examples"),
    ("5.2", "Classification of Linear Systems"),
    ("5.3", "Love Affairs"),
    ("6.1", "Phase Portraits"),
    ("6.2", "Existence, Uniqueness, and Topological Consequences"),
    ("6.3", "Fixed Points and Linearization"),
    ("6.4", "Rabbits versus Sheep"),
    ("6.5", "Conservative Systems"),
    ("6.6", "Reversible Systems"),
    ("6.7", "Pendulum"),
    ("6.8", "Index Theory"),
    ("7.1", "Examples"),
    ("7.2", "Ruling Out Closed Orbits"),
    ("7.3", "Poincaré−Bendixson Theorem"),
    ("7.4", "Liénard Systems"),
    ("7.5", "Relaxation Oscillations"),
    ("7.6", "Weakly Nonlinear Oscillators"),
    (
        "8.1",
        "Saddle-Node, Transcritical, and Pitchfork Bifurcations",
    ),
    ("8.2", "Hopf Bifurcations"),
    ("8.3", "Oscillating Chemical Reactions"),
    ("8.4", "Global Bifurcations of Cycles"),
    (
        "8.5",
        "Hysteresis in the Driven Pendulum and Josephson Junction",
    ),
    ("8.6", "Coupled Oscillators and Quasiperiodicity"),
    ("8.7", "Poincaré Maps"),
    ("9.1", "A Chaotic Waterwheel"),
    ("9.2", "Simple Properties of the Lorenz Equations"),
    ("9.3", "Chaos on a Strange Attractor"),
    ("9.4", "Lorenz Map"),
    ("9.5", "Exploring Parameter Space"),
    ("9.6", "Using Chaos to Send Secret Messages"),
    ("10.1", "Fixed Points and Cobwebs"),
    ("10.2", "Logistic Map: Numerics"),
    ("10.3", "Logistic Map: Analysis"),
    ("10.4", "Periodic Windows"),
    ("10.5", "Liapunov Exponent"),
    ("10.6", "Universality and Experiments"),
    ("10.7", "Renormalization"),
    ("11.1", "Countable and Uncountable Sets"),
    ("11.2", "Cantor Set"),
    ("11.3", "Dimension of Self-Similar Fractals"),
    ("11.4", "Box Dimension"),
    ("11.5", "Pointwise and Correlation Dimensions"),
    ("12.1", "The Simplest Examples"),
    ("12.2", "Hénon Map"),
    ("12.3", "Rössler System"),
    ("12.4", "Chemical Chaos and Attractor Reconstruction"),
    ("12.5", "Forced Double-Well Oscillator"),
];

const BISECTION_WIDTH: f64 = 1e-9;

/// The transcritical template's view is `x ∈ [−2, 2]`, so its branches
/// `x* = 0` and `x* = r` are found as one for `|r|` below
/// `FIXED_POINT_MERGE_REL` times this.
const TRANSCRITICAL_SEARCH_DIAGONAL: f64 = 4.0;

struct Check {
    template: &'static str,
    section: &'static str,
    book: &'static str,
    claim: Claim,
}

enum Claim {
    FixedPoints {
        params: &'static [(&'static str, f64)],
        expect: &'static [(&'static [f64], Classification)],
        tol: f64,
    },
    Bifurcation {
        param: &'static str,
        bracket: (f64, f64),
        observable: Observable,
        book: f64,
        tol: f64,
    },
    EnergyDrift {
        params: &'static [(&'static str, f64)],
        x0: &'static [f64],
        energy: fn(&[f64]) -> f64,
        t_end: f64,
        rtol: f64,
        atol: f64,
        bound: Option<f64>,
    },
    Period {
        run: SectionRun,
        book: f64,
        tol: Option<f64>,
    },
    PeriodsAgree {
        runs: [SectionRun; 2],
        tol: f64,
    },
    FirstCrossing {
        run: SectionRun,
        book: f64,
        tol: f64,
    },
    MeanFrequency {
        run: SectionRun,
        states: &'static [usize],
        book: f64,
        tol: f64,
    },
    StroboscopicFixedPoint {
        run: SectionRun,
        book: f64,
        tol: f64,
    },
    Amplitude {
        run: SectionRun,
        state: usize,
        book: f64,
        tol: f64,
    },
}

struct SectionRun {
    params: &'static [(&'static str, f64)],
    x0: &'static [f64],
    t_end: f64,
    section: SectionSpec,
    skip: usize,
    solver: SolverOptions,
}

const APP_SOLVER: SolverOptions = SolverOptions {
    method: Method::Dopri5,
    rtol: 1e-6,
    atol: 1e-9,
    h: 0.0,
    h_max: None,
    max_steps: 200_000,
};

enum Observable {
    FixedPointCount { search: &'static [(f64, f64)] },
    StabilityOf { near: &'static [f64] },
}

fn pendulum_energy(x: &[f64]) -> f64 {
    x[1] * x[1] / 2.0 - x[0].cos()
}

/// `E = ẋ²/2 + V(x)` with the double well `V = −x²/2 + x⁴/4` (§6.5).
fn double_well_energy(x: &[f64]) -> f64 {
    let x2 = x[0] * x[0];
    x[1] * x[1] / 2.0 - x2 / 2.0 + x2 * x2 / 4.0
}

const PENDULUM_POINTS: &[(&[f64], Classification)] = &[
    (&[0.0, 0.0], Classification::Centre),
    (&[PI, 0.0], Classification::Saddle),
    (&[-PI, 0.0], Classification::Saddle),
    (&[2.0 * PI, 0.0], Classification::Centre),
    (&[-2.0 * PI, 0.0], Classification::Centre),
];

const VAN_DER_POL_TURNS: SectionRun = SectionRun {
    params: &[("mu", 1.0)],
    x0: &[0.1, 0.0],
    t_end: 200.0,
    section: SectionSpec::Plane {
        state: 1,
        value: 0.0,
        direction: Direction::Falling,
    },
    skip: 10,
    solver: APP_SOLVER,
};

const fn relaxation_turns(t_end: f64, solver: SolverOptions) -> SectionRun {
    SectionRun {
        params: &[("mu", 10.0)],
        x0: &[0.5, 0.0],
        t_end,
        section: SectionSpec::Plane {
            state: 1,
            value: 0.0,
            direction: Direction::Falling,
        },
        skip: 2,
        solver,
    }
}

const HOPF_TURNS: SectionRun = SectionRun {
    params: &[("mu", 0.25)],
    x0: &[0.05, 0.0],
    t_end: 200.0,
    section: SectionSpec::Plane {
        state: 1,
        value: 0.0,
        direction: Direction::Rising,
    },
    skip: 20,
    solver: APP_SOLVER,
};

const CHECKS: &[Check] = &[
    Check {
        template: "saddle-node",
        section: "3.1",
        book: "r = −1: x* = −1 stable, x* = +1 unstable",
        claim: Claim::FixedPoints {
            params: &[("r", -1.0)],
            expect: &[
                (&[-1.0], Classification::Sink),
                (&[1.0], Classification::Source),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "saddle-node",
        section: "3.1",
        book: "two fixed points merge and vanish at r_c = 0",
        claim: Claim::Bifurcation {
            param: "r",
            bracket: (-1.0, 1.0),
            observable: Observable::FixedPointCount {
                search: &[(-2.5, 2.5)],
            },
            book: 0.0,
            tol: 1e-6,
        },
    },
    Check {
        template: "linear-2d",
        section: "5.2",
        book: "(a,b,c,d) = (1,0,0,−1): Δ < 0, saddle",
        claim: Claim::FixedPoints {
            params: &[("a", 1.0), ("b", 0.0), ("c", 0.0), ("d", -1.0)],
            expect: &[(&[0.0, 0.0], Classification::Saddle)],
            tol: 1e-6,
        },
    },
    Check {
        template: "linear-2d",
        section: "5.2",
        book: "(−2,0,0,−1): τ < 0, τ² > 4Δ, stable node",
        claim: Claim::FixedPoints {
            params: &[("a", -2.0), ("b", 0.0), ("c", 0.0), ("d", -1.0)],
            expect: &[(&[0.0, 0.0], Classification::StableNode)],
            tol: 1e-6,
        },
    },
    Check {
        template: "linear-2d",
        section: "5.2",
        book: "(1,−2,2,1): τ > 0, τ² < 4Δ, unstable spiral",
        claim: Claim::FixedPoints {
            params: &[("a", 1.0), ("b", -2.0), ("c", 2.0), ("d", 1.0)],
            expect: &[(&[0.0, 0.0], Classification::UnstableSpiral)],
            tol: 1e-6,
        },
    },
    Check {
        template: "linear-2d",
        section: "5.2",
        book: "(0,1,−1,0): τ = 0, Δ > 0, center",
        claim: Claim::FixedPoints {
            params: &[("a", 0.0), ("b", 1.0), ("c", -1.0), ("d", 0.0)],
            expect: &[(&[0.0, 0.0], Classification::Centre)],
            tol: 1e-6,
        },
    },
    Check {
        template: "linear-2d",
        section: "5.2",
        book: "(−1,0,0,−1): A = −I, stable star",
        claim: Claim::FixedPoints {
            params: &[("a", -1.0), ("b", 0.0), ("c", 0.0), ("d", -1.0)],
            expect: &[(&[0.0, 0.0], Classification::StableStar)],
            tol: 1e-6,
        },
    },
    Check {
        template: "linear-2d",
        section: "5.2",
        book: "(−1,1,0,−1): τ² = 4Δ, one eigenvector, degenerate",
        claim: Claim::FixedPoints {
            params: &[("a", -1.0), ("b", 1.0), ("c", 0.0), ("d", -1.0)],
            expect: &[(&[0.0, 0.0], Classification::StableDegenerateNode)],
            tol: 1e-6,
        },
    },
    Check {
        template: "rabbits-vs-sheep",
        section: "6.4",
        book: "(0,0) unstable, (0,2) (3,0) stable nodes, (1,1) saddle",
        claim: Claim::FixedPoints {
            params: &[],
            expect: &[
                (&[0.0, 0.0], Classification::UnstableNode),
                (&[0.0, 2.0], Classification::StableNode),
                (&[3.0, 0.0], Classification::StableNode),
                (&[1.0, 1.0], Classification::Saddle),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "hamiltonian-pendulum",
        section: "6.5",
        book: "centers at q = 2kπ, saddles at q = (2k+1)π",
        claim: Claim::FixedPoints {
            params: &[],
            expect: PENDULUM_POINTS,
            tol: 1e-6,
        },
    },
    Check {
        template: "pendulum",
        section: "6.7",
        book: "centers at θ = 2kπ, saddles at θ = (2k+1)π",
        claim: Claim::FixedPoints {
            params: &[],
            expect: PENDULUM_POINTS,
            tol: 1e-6,
        },
    },
    Check {
        template: "pendulum",
        section: "6.7",
        book: "E = ω²/2 − cos θ conserved, rtol 1e−8 atol 1e−10",
        claim: Claim::EnergyDrift {
            params: &[],
            x0: &[1.0, 0.0],
            energy: pendulum_energy,
            t_end: 100.0,
            rtol: 1e-8,
            atol: 1e-10,
            bound: Some(1e-6),
        },
    },
    Check {
        template: "pendulum",
        section: "6.7",
        book: "E = ω²/2 − cos θ, app default rtol 1e−6 atol 1e−9",
        claim: Claim::EnergyDrift {
            params: &[],
            x0: &[1.0, 0.0],
            energy: pendulum_energy,
            t_end: 100.0,
            rtol: 1e-6,
            atol: 1e-9,
            bound: None,
        },
    },
    Check {
        template: "damped-pendulum",
        section: "6.7",
        book: "b > 0: stable spirals at 2kπ, saddles at (2k+1)π",
        claim: Claim::FixedPoints {
            params: &[("b", 0.25)],
            expect: &[
                (&[0.0, 0.0], Classification::StableSpiral),
                (&[PI, 0.0], Classification::Saddle),
                (&[-PI, 0.0], Classification::Saddle),
                (&[2.0 * PI, 0.0], Classification::StableSpiral),
                (&[-2.0 * PI, 0.0], Classification::StableSpiral),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "van-der-pol",
        section: "7.1",
        book: "μ = 1: origin τ = μ > 0, unstable spiral",
        claim: Claim::FixedPoints {
            params: &[("mu", 1.0)],
            expect: &[(&[0.0, 0.0], Classification::UnstableSpiral)],
            tol: 1e-6,
        },
    },
    Check {
        template: "van-der-pol",
        section: "7.1",
        book: "μ = 1: limit cycle period T = 6.66329",
        claim: Claim::Period {
            run: VAN_DER_POL_TURNS,
            book: 6.663_286_859,
            tol: Some(1e-3),
        },
    },
    Check {
        template: "van-der-pol",
        section: "7.1",
        book: "μ = 1: limit cycle amplitude max x = 2.00862",
        claim: Claim::Amplitude {
            run: VAN_DER_POL_TURNS,
            state: 0,
            book: 2.008_619_8,
            tol: 1e-3,
        },
    },
    Check {
        template: "selkov-glycolysis",
        section: "7.3",
        book: "a = 0.08, b = 0.6: x* = b, y* = b/(a+b²), unstable",
        claim: Claim::FixedPoints {
            params: &[("a", 0.08), ("b", 0.6)],
            expect: &[(&[0.6, 0.6 / (0.08 + 0.36)], Classification::UnstableSpiral)],
            tol: 1e-6,
        },
    },
    Check {
        template: "selkov-glycolysis",
        section: "7.3",
        book: "a = 0.08: stability changes at b² = 0.12",
        claim: Claim::Bifurcation {
            param: "b",
            bracket: (0.2, 0.6),
            observable: Observable::StabilityOf { near: &[0.5, 1.5] },
            book: 0.346_410_161_513_775_46,
            tol: 1e-6,
        },
    },
    Check {
        template: "selkov-glycolysis",
        section: "7.3",
        book: "a = 0.08: stability changes at b² = 0.72",
        claim: Claim::Bifurcation {
            param: "b",
            bracket: (0.6, 1.2),
            observable: Observable::StabilityOf { near: &[0.9, 1.0] },
            book: 0.848_528_137_423_857,
            tol: 1e-6,
        },
    },
    Check {
        template: "sin-x",
        section: "2.1",
        book: "x* = 0 unstable, x* = ±π stable",
        claim: Claim::FixedPoints {
            params: &[],
            expect: &[
                (&[-PI], Classification::Sink),
                (&[0.0], Classification::Source),
                (&[PI], Classification::Sink),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "logistic-growth",
        section: "2.3",
        book: "K = 2: N* = 0 unstable, N* = K stable",
        claim: Claim::FixedPoints {
            params: &[("r", 1.0), ("K", 2.0)],
            expect: &[
                (&[0.0], Classification::Source),
                (&[2.0], Classification::Sink),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "transcritical",
        section: "3.2",
        book: "r = −1: x* = 0 stable, x* = r unstable",
        claim: Claim::FixedPoints {
            params: &[("r", -1.0)],
            expect: &[
                (&[0.0], Classification::Sink),
                (&[-1.0], Classification::Source),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "transcritical",
        section: "3.2",
        book: "r = 1: x* = 0 unstable, x* = r stable",
        claim: Claim::FixedPoints {
            params: &[("r", 1.0)],
            expect: &[
                (&[0.0], Classification::Source),
                (&[1.0], Classification::Sink),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "transcritical",
        section: "3.2",
        book: "x* = 0 exchanges stability at r_c = 0",
        claim: Claim::Bifurcation {
            param: "r",
            bracket: (-1.0, 1.0),
            observable: Observable::StabilityOf { near: &[0.0] },
            book: 0.0,
            tol: 2.0 * FIXED_POINT_MERGE_REL * TRANSCRITICAL_SEARCH_DIAGONAL,
        },
    },
    Check {
        template: "pitchfork",
        section: "3.4",
        book: "r = 1: x* = ±√r stable, x* = 0 unstable",
        claim: Claim::FixedPoints {
            params: &[("r", 1.0)],
            expect: &[
                (&[-1.0], Classification::Sink),
                (&[0.0], Classification::Source),
                (&[1.0], Classification::Sink),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "pitchfork",
        section: "3.4",
        book: "one fixed point becomes three at r_c = 0",
        claim: Claim::Bifurcation {
            param: "r",
            bracket: (-1.0, 1.0),
            observable: Observable::FixedPointCount {
                search: &[(-2.0, 2.0)],
            },
            book: 0.0,
            tol: 1e-6,
        },
    },
    Check {
        template: "bead-on-hoop",
        section: "3.5",
        book: "γ = 2: φ* = ±arccos(1/γ) = ±π/3 stable, 0 and ±π not",
        claim: Claim::FixedPoints {
            params: &[("gamma", 2.0)],
            expect: &[
                (&[-PI], Classification::Source),
                (&[-PI / 3.0], Classification::Sink),
                (&[0.0], Classification::Source),
                (&[PI / 3.0], Classification::Sink),
                (&[PI], Classification::Source),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "bead-on-hoop",
        section: "3.5",
        book: "supercritical pitchfork at γ = 1",
        claim: Claim::Bifurcation {
            param: "gamma",
            bracket: (0.5, 2.0),
            observable: Observable::FixedPointCount {
                search: &[(-2.5, 2.5)],
            },
            book: 1.0,
            tol: 1e-6,
        },
    },
    Check {
        template: "imperfect-bifurcation",
        section: "3.6",
        book: "r = 1: two fixed points vanish at h_c = 2/(3√3)",
        claim: Claim::Bifurcation {
            param: "h",
            bracket: (0.0, 1.0),
            observable: Observable::FixedPointCount {
                search: &[(-2.0, 2.0)],
            },
            book: 0.384_900_179_459_750_5,
            tol: 1e-6,
        },
    },
    Check {
        template: "nonuniform-oscillator",
        section: "4.3",
        book: "ω = 1, a = 0.5: period T = 2π/√(ω² − a²)",
        claim: Claim::FirstCrossing {
            run: SectionRun {
                params: &[("omega", 1.0), ("a", 0.5)],
                x0: &[0.0],
                t_end: 20.0,
                section: SectionSpec::Plane {
                    state: 0,
                    value: 2.0 * PI,
                    direction: Direction::Rising,
                },
                skip: 0,
                solver: APP_SOLVER,
            },
            book: 7.255_197_456_936_871,
            tol: 1e-4,
        },
    },
    Check {
        template: "nonuniform-oscillator",
        section: "4.3",
        book: "a = 2 > ω: θ* = π/6 stable, θ* = 5π/6 unstable",
        claim: Claim::FixedPoints {
            params: &[("omega", 1.0), ("a", 2.0)],
            expect: &[
                (&[PI / 6.0], Classification::Sink),
                (&[5.0 * PI / 6.0], Classification::Source),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "nonuniform-oscillator",
        section: "4.3",
        book: "ω = 1: saddle-node bifurcation at a_c = ω",
        claim: Claim::Bifurcation {
            param: "a",
            bracket: (0.5, 2.0),
            observable: Observable::FixedPointCount {
                search: &[(-0.5, 6.4)],
            },
            book: 1.0,
            tol: 1e-6,
        },
    },
    Check {
        template: "romeo-juliet",
        section: "5.3",
        book: "a, b > 0: τ = 0, Δ = ab > 0, center",
        claim: Claim::FixedPoints {
            params: &[("a", 1.0), ("b", 2.0)],
            expect: &[(&[0.0, 0.0], Classification::Centre)],
            tol: 1e-6,
        },
    },
    Check {
        template: "romeo-juliet",
        section: "5.3",
        book: "a = 1, b = 2: period 2π/√(ab)",
        claim: Claim::Period {
            run: SectionRun {
                params: &[("a", 1.0), ("b", 2.0)],
                x0: &[1.0, 0.5],
                t_end: 30.0,
                section: SectionSpec::Plane {
                    state: 1,
                    value: 0.0,
                    direction: Direction::Falling,
                },
                skip: 0,
                solver: APP_SOLVER,
            },
            book: 4.442_882_938_158_366,
            tol: Some(1e-4),
        },
    },
    Check {
        template: "double-well",
        section: "6.5",
        book: "saddle at x = 0, centers at x = ±1",
        claim: Claim::FixedPoints {
            params: &[],
            expect: &[
                (&[-1.0, 0.0], Classification::Centre),
                (&[0.0, 0.0], Classification::Saddle),
                (&[1.0, 0.0], Classification::Centre),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "double-well",
        section: "6.5",
        book: "E = ẋ²/2 − x²/2 + x⁴/4 conserved, rtol 1e−8",
        claim: Claim::EnergyDrift {
            params: &[],
            x0: &[1.5, 0.0],
            energy: double_well_energy,
            t_end: 100.0,
            rtol: 1e-8,
            atol: 1e-10,
            bound: Some(1e-6),
        },
    },
    Check {
        template: "reversible",
        section: "6.6",
        book: "nonlinear center at 0, saddles at (−1, ±1)",
        claim: Claim::FixedPoints {
            params: &[],
            expect: &[
                (&[0.0, 0.0], Classification::Centre),
                (&[-1.0, 1.0], Classification::Saddle),
                (&[-1.0, -1.0], Classification::Saddle),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "relaxation-oscillator",
        section: "7.5",
        book: "μ = 10: period T = 19.0784",
        claim: Claim::Period {
            run: relaxation_turns(200.0, APP_SOLVER),
            book: 19.0784,
            tol: Some(1e-2),
        },
    },
    Check {
        template: "relaxation-oscillator",
        section: "7.5",
        book: "DOPRI5 rtol 1e−10 and RK4 h = 1e−4 agree on T",
        claim: Claim::PeriodsAgree {
            runs: [
                relaxation_turns(
                    120.0,
                    SolverOptions {
                        method: Method::Dopri5,
                        rtol: 1e-10,
                        atol: 1e-12,
                        h: 0.0,
                        h_max: None,
                        max_steps: 1_000_000,
                    },
                ),
                relaxation_turns(
                    120.0,
                    SolverOptions {
                        method: Method::Rk4,
                        rtol: 0.0,
                        atol: 0.0,
                        h: 1e-4,
                        h_max: None,
                        max_steps: 1_300_000,
                    },
                ),
            ],
            tol: 1e-5,
        },
    },
    Check {
        template: "relaxation-oscillator",
        section: "7.5",
        book: "μ ≫ 1 asymptote T ≈ μ(3 − 2 ln 2) = 16.14",
        claim: Claim::Period {
            run: relaxation_turns(200.0, APP_SOLVER),
            book: 16.137_056_388_801_094,
            tol: None,
        },
    },
    Check {
        template: "saddle-node-2d",
        section: "8.1",
        book: "μ = 1: (1, 0) stable node, (−1, 0) saddle",
        claim: Claim::FixedPoints {
            params: &[("mu", 1.0)],
            expect: &[
                (&[1.0, 0.0], Classification::StableNode),
                (&[-1.0, 0.0], Classification::Saddle),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "saddle-node-2d",
        section: "8.1",
        book: "node and saddle collide and vanish at μ_c = 0",
        claim: Claim::Bifurcation {
            param: "mu",
            bracket: (-1.0, 1.0),
            observable: Observable::FixedPointCount {
                search: &[(-2.0, 2.0), (-2.0, 2.0)],
            },
            book: 0.0,
            tol: 1e-6,
        },
    },
    Check {
        template: "hopf-supercritical",
        section: "8.2",
        book: "the origin loses stability at μ_c = 0",
        claim: Claim::Bifurcation {
            param: "mu",
            bracket: (-0.5, 0.5),
            observable: Observable::StabilityOf { near: &[0.0, 0.0] },
            book: 0.0,
            tol: 1e-6,
        },
    },
    Check {
        template: "hopf-supercritical",
        section: "8.2",
        book: "μ = 0.25: limit cycle radius √μ",
        claim: Claim::Amplitude {
            run: HOPF_TURNS,
            state: 0,
            book: 0.5,
            tol: 1e-4,
        },
    },
    Check {
        template: "hopf-supercritical",
        section: "8.2",
        book: "ω = 1, b = 0: limit cycle period 2π",
        claim: Claim::Period {
            run: HOPF_TURNS,
            book: 2.0 * PI,
            tol: Some(1e-4),
        },
    },
    Check {
        template: "cdima",
        section: "8.3",
        book: "a = 10, b = 2: x* = a/5, y* = 1 + (a/5)², unstable",
        claim: Claim::FixedPoints {
            params: &[("a", 10.0), ("b", 2.0)],
            expect: &[(&[2.0, 5.0], Classification::UnstableSpiral)],
            tol: 1e-6,
        },
    },
    Check {
        template: "cdima",
        section: "8.3",
        book: "a = 10: Hopf at b_c = 3a/5 − 25/a",
        claim: Claim::Bifurcation {
            param: "b",
            bracket: (2.0, 5.0),
            observable: Observable::StabilityOf { near: &[2.0, 5.0] },
            book: 3.5,
            tol: 1e-6,
        },
    },
    Check {
        template: "driven-pendulum",
        section: "8.5",
        book: "I = 1/2: φ* = arcsin I stable, π − arcsin I saddles",
        claim: Claim::FixedPoints {
            params: &[("alpha", 0.2), ("I", 0.5)],
            expect: &[
                (&[PI / 6.0, 0.0], Classification::StableSpiral),
                (&[5.0 * PI / 6.0, 0.0], Classification::Saddle),
                (&[PI / 6.0 + 2.0 * PI, 0.0], Classification::StableSpiral),
                (&[5.0 * PI / 6.0 + 2.0 * PI, 0.0], Classification::Saddle),
            ],
            tol: 1e-6,
        },
    },
    Check {
        template: "driven-pendulum",
        section: "8.5",
        book: "fixed points vanish in a saddle-node at I = 1",
        claim: Claim::Bifurcation {
            param: "I",
            bracket: (0.5, 1.5),
            observable: Observable::FixedPointCount {
                search: &[(-2.0, 5.0), (-2.0, 4.0)],
            },
            book: 1.0,
            tol: 1e-6,
        },
    },
    Check {
        template: "coupled-oscillators",
        section: "8.6",
        book: "phase-locked at (K₂ω₁ + K₁ω₂)/(K₁ + K₂)",
        claim: Claim::MeanFrequency {
            run: SectionRun {
                params: &[
                    ("omega_1", 1.2),
                    ("omega_2", 1.0),
                    ("K_1", 0.25),
                    ("K_2", 0.25),
                ],
                x0: &[0.0, 3.0],
                t_end: 200.0,
                section: SectionSpec::Stroboscopic {
                    period: 1.0,
                    phase: 0.0,
                },
                skip: 50,
                solver: APP_SOLVER,
            },
            states: &[0, 1],
            book: 1.1,
            tol: 1e-6,
        },
    },
    Check {
        template: "poincare-map-forced",
        section: "8.7",
        book: "A = 1: the Poincaré map's fixed point x* = −A/2",
        claim: Claim::StroboscopicFixedPoint {
            run: SectionRun {
                params: &[("A", 1.0)],
                x0: &[2.0],
                t_end: 40.0,
                section: SectionSpec::Stroboscopic {
                    period: 2.0 * PI,
                    phase: 0.0,
                },
                skip: 0,
                solver: APP_SOLVER,
            },
            book: -0.5,
            tol: 1e-5,
        },
    },
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    id: String,
    #[serde(default)]
    chapter: Option<String>,
    #[serde(default)]
    section: Option<String>,
    format: InputFormat,
    equations: Vec<String>,
    params: HashMap<String, f64>,
    view: View,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct View {
    x: String,
    y: String,
    x_range: (f64, f64),
    y_range: (f64, f64),
}

fn templates() -> Vec<Template> {
    serde_json::from_str(TEMPLATES_JSON).expect("templates.json")
}

fn chapter_of(section: &str) -> &str {
    section.split('.').next().unwrap_or(section)
}

fn in_second_edition(section: &str) -> bool {
    STROGATZ_2E.iter().any(|(s, _)| *s == section)
}

struct Lab<'a> {
    template: &'a Template,
    res: CompileResponse,
    system: Arc<CompiledSystem>,
}

impl<'a> Lab<'a> {
    fn new(store: &SystemStore, template: &'a Template) -> Self {
        let res = service::compile_system(
            store,
            &CompileRequest {
                source: template.equations.join("\n"),
                format: template.format,
            },
        );
        assert!(
            res.ok
                && !res
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == Severity::Error),
            "template {} doesn't compile: {:?}",
            template.id,
            res.diagnostics
        );
        let system = store.get(res.system_id.unwrap()).unwrap();
        Lab {
            template,
            res,
            system,
        }
    }

    fn params(&self, overrides: &[(&str, f64)]) -> Vec<f64> {
        for (name, _) in overrides {
            assert!(
                self.res.params.iter().any(|p| p.name == *name),
                "{} has no parameter {name:?}",
                self.template.id
            );
        }
        self.res
            .params
            .iter()
            .map(|p| {
                overrides
                    .iter()
                    .find(|(n, _)| *n == p.name)
                    .map(|(_, v)| *v)
                    .or_else(|| self.template.params.get(&p.name).copied())
                    .unwrap_or(p.default)
            })
            .collect()
    }

    fn axis(&self, name: &str) -> AxisRef {
        if name == "t" {
            return AxisRef::Time;
        }
        let index = self
            .res
            .states
            .iter()
            .position(|s| s.name == name || s.latex == name)
            .unwrap_or_else(|| panic!("axis {name:?} is not a state"));
        AxisRef::State { index }
    }

    fn request(&self, params: Vec<f64>) -> PortraitRequest {
        PortraitRequest {
            request_id: 1,
            system_id: self.res.system_id.unwrap(),
            params,
            x_axis: self.axis(&self.template.view.x),
            y_axis: self.axis(&self.template.view.y),
            x_range: self.template.view.x_range,
            y_range: self.template.view.y_range,
            base_state: vec![0.0; self.res.states.len()],
            field_time: 0.0,
            field_grid: 0,
            nullclines: false,
            fixed_points: false,
            trajectories: Vec::new(),
            solver: APP_SOLVER,
            samples: 0,
            graph_samples: None,
            transient: None,
            section: None,
            z_axis: None,
            z_range: None,
        }
    }

    fn fixed_points(&self, params: Vec<f64>, search: Option<&[(f64, f64)]>) -> Vec<FixedPointOut> {
        let mut req = self.request(params);
        let range = |axis: AxisRef, view: (f64, f64)| match (axis, search) {
            (AxisRef::State { index }, Some(s)) => s[index],
            _ => view,
        };
        req.x_range = range(req.x_axis, req.x_range);
        req.y_range = range(req.y_axis, req.y_range);
        req.fixed_points = true;
        service::compute_portrait(&self.system, &req)
            .unwrap_or_else(|e| panic!("{}: {e}", self.template.id))
            .fixed_points
    }

    fn section(&self, run: &SectionRun) -> TrajectoryOut {
        let mut req = self.request(self.params(run.params));
        req.trajectories = vec![TrajectoryRequest {
            x0: run.x0.to_vec(),
            t0: 0.0,
            t1: run.t_end,
        }];
        req.section = Some(run.section);
        req.transient = Some(run.skip);
        req.solver = run.solver;
        let mut out = service::compute_portrait(&self.system, &req)
            .unwrap_or_else(|e| panic!("{}: {e}", self.template.id));
        out.trajectories.remove(0)
    }
}

fn all_near(
    values: &[f64],
    status: Status,
    book: f64,
    tol: Option<f64>,
) -> (String, String, f64, f64, Verdict) {
    let delta = values.iter().map(|v| (v - book).abs()).fold(0.0, f64::max);
    let measured = if values.is_empty() {
        format!("no crossings, {status:?}")
    } else {
        format!("{:.6} over {}", mean(values), values.len())
    };
    let ran = status == Status::Completed && !values.is_empty();
    let pass = match tol {
        Some(tol) => verdict(ran && delta <= tol),
        None if ran => Verdict::Info,
        None => Verdict::Fail,
    };
    (
        format!("{book:.6}"),
        measured,
        delta,
        tol.unwrap_or(f64::NAN),
        pass,
    )
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn periods(sol: &TrajectoryOut) -> Vec<f64> {
    sol.t.windows(2).map(|w| w[1] - w[0]).collect()
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

fn points(n: usize) -> String {
    if n == 1 {
        "1 point".into()
    } else {
        format!("{n} points")
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Verdict {
    Pass,
    Fail,
    Info,
}

struct Row {
    chapter: String,
    section: &'static str,
    template: &'static str,
    claim: &'static str,
    book: String,
    measured: String,
    delta: f64,
    tol: f64,
    pass: Verdict,
}

fn verdict(ok: bool) -> Verdict {
    if ok {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

fn evaluate(lab: &Lab, claim: &Claim) -> (String, String, f64, f64, Verdict) {
    match claim {
        Claim::FixedPoints {
            params,
            expect,
            tol,
        } => {
            let found = lab.fixed_points(lab.params(params), None);
            let mut delta = 0.0_f64;
            let mut wrong = Vec::new();
            for (x, class) in *expect {
                match found
                    .iter()
                    .min_by(|a, b| distance(&a.x, x).total_cmp(&distance(&b.x, x)))
                {
                    Some(fp) => {
                        delta = delta.max(distance(&fp.x, x));
                        if fp.classification != *class {
                            wrong.push(format!("{x:?} {}", fp.classification));
                        }
                    }
                    None => delta = f64::INFINITY,
                }
            }
            let measured = if wrong.is_empty() {
                format!("{}, classes ok", points(found.len()))
            } else {
                format!("{}, {}", points(found.len()), wrong.join("; "))
            };
            let ok = found.len() == expect.len() && wrong.is_empty() && delta <= *tol;
            (points(expect.len()), measured, delta, *tol, verdict(ok))
        }
        Claim::Bifurcation {
            param,
            bracket,
            observable,
            book,
            tol,
        } => {
            let observe = |value: f64| -> Option<usize> {
                let found = |search| lab.fixed_points(lab.params(&[(param, value)]), search);
                match observable {
                    Observable::FixedPointCount { search } => Some(found(Some(search)).len()),
                    Observable::StabilityOf { near } => found(None)
                        .iter()
                        .min_by(|a, b| distance(&a.x, near).total_cmp(&distance(&b.x, near)))
                        .map(|fp| usize::from(fp.stable)),
                }
            };
            let (mut lo, mut hi) = *bracket;
            let at_lo = observe(lo);
            if at_lo.is_none() || at_lo == observe(hi) {
                return (
                    format!("{book:.9}"),
                    "no change in bracket".into(),
                    f64::INFINITY,
                    *tol,
                    Verdict::Fail,
                );
            }
            while hi - lo > BISECTION_WIDTH {
                let mid = 0.5 * (lo + hi);
                if observe(mid) == at_lo {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let measured = 0.5 * (lo + hi);
            let delta = (measured - book).abs();
            (
                format!("{book:.9}"),
                format!("{measured:.9}"),
                delta,
                *tol,
                verdict(delta <= *tol),
            )
        }
        Claim::Period { run, book, tol } => {
            let sol = lab.section(run);
            all_near(&periods(&sol), sol.status, *book, *tol)
        }
        Claim::PeriodsAgree { runs, tol } => {
            let [a, b] = runs.each_ref().map(|r| lab.section(r));
            let (pa, pb) = (periods(&a), periods(&b));
            let ran = [&a, &b].iter().all(|s| s.status == Status::Completed)
                && !pa.is_empty()
                && !pb.is_empty();
            let delta = if ran {
                (mean(&pa) - mean(&pb)).abs()
            } else {
                f64::INFINITY
            };
            (
                "agree".into(),
                format!("{:.6} vs {:.6}", mean(&pa), mean(&pb)),
                delta,
                *tol,
                verdict(ran && delta <= *tol),
            )
        }
        Claim::FirstCrossing { run, book, tol } => {
            let sol = lab.section(run);
            let first: Vec<f64> = sol.t.first().copied().into_iter().collect();
            all_near(&first, sol.status, *book, Some(*tol))
        }
        Claim::MeanFrequency {
            run,
            states,
            book,
            tol,
        } => {
            let sol = lab.section(run);
            let dim = lab.res.states.len();
            let n = sol.t.len();
            let rates: Vec<f64> = if n < 2 {
                Vec::new()
            } else {
                let span = sol.t[n - 1] - sol.t[0];
                states
                    .iter()
                    .map(|&i| (sol.x[(n - 1) * dim + i] - sol.x[i]) / span)
                    .collect()
            };
            all_near(&rates, sol.status, *book, Some(*tol))
        }
        Claim::StroboscopicFixedPoint { run, book, tol } => {
            let sol = lab.section(run);
            assert_eq!(lab.res.states.len(), 1, "a one-state return map");
            let x = &sol.x;
            let values: Vec<f64> = if x.len() < 3 {
                Vec::new()
            } else {
                let slope = (x[2] - x[1]) / (x[1] - x[0]);
                let secant = (x[1] - slope * x[0]) / (1.0 - slope);
                vec![secant, x[x.len() - 1]]
            };
            all_near(&values, sol.status, *book, Some(*tol))
        }
        Claim::Amplitude {
            run,
            state,
            book,
            tol,
        } => {
            let sol = lab.section(run);
            let dim = lab.res.states.len();
            let values: Vec<f64> = sol.x.chunks(dim).map(|x| x[*state]).collect();
            all_near(&values, sol.status, *book, Some(*tol))
        }
        Claim::EnergyDrift {
            params,
            x0,
            energy,
            t_end,
            rtol,
            atol,
            bound,
        } => {
            let opts = dyn_solve::SolverOptions {
                method: Method::Dopri5,
                rtol: *rtol,
                atol: *atol,
                h: 0.0,
                h_max: 0.0,
                max_steps: 200_000,
            };
            let sol = dyn_solve::integrate(
                lab.system.as_ref(),
                &lab.params(params),
                0.0,
                x0,
                *t_end,
                &opts,
                Sampling::Steps,
            );
            let e0 = energy(x0);
            let drift = sol
                .x
                .chunks(x0.len())
                .map(|x| (energy(x) - e0).abs())
                .fold(0.0, f64::max);
            let completed = sol.status == Status::Completed;
            let pass = match bound {
                Some(b) => verdict(completed && drift <= *b),
                None if completed => Verdict::Info,
                None => Verdict::Fail,
            };
            (
                "conserved".into(),
                format!("{drift:.2e}"),
                drift,
                bound.unwrap_or(f64::NAN),
                pass,
            )
        }
    }
}

fn print_line([ch, sec, tpl, claim, book, meas, d, tol, res]: [&str; 9]) {
    println!("{ch:<3} {sec:<4} {tpl:<22} {claim:<54} {book:<12} {meas:<26} {d:>8} {tol:>7} {res}");
}

fn print_table(rows: &[Row]) {
    println!();
    print_line([
        "ch", "§", "template", "claim", "book", "measured", "delta", "tol", "result",
    ]);
    for r in rows {
        let tol = if r.tol.is_nan() {
            "—".to_string()
        } else {
            format!("{:.0e}", r.tol)
        };
        let result = match r.pass {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Info => "info",
        };
        print_line([
            &r.chapter,
            r.section,
            r.template,
            r.claim,
            &r.book,
            &r.measured,
            &format!("{:.1e}", r.delta),
            &tol,
            result,
        ]);
    }
}

#[test]
fn template_sections_are_in_the_second_edition() {
    let mut bad = Vec::new();
    for t in templates() {
        let Some(section) = &t.section else {
            continue;
        };
        if !in_second_edition(section) {
            bad.push(format!("{}: §{section} is not in the 2nd edition", t.id));
        }
        let chapter = format!("Ch. {}", chapter_of(section));
        if t.chapter.as_deref() != Some(chapter.as_str()) {
            bad.push(format!(
                "{}: chapter {:?} but §{section} is in {chapter}",
                t.id, t.chapter
            ));
        }
    }
    for c in CHECKS {
        if !in_second_edition(c.section) {
            bad.push(format!(
                "check on {}: §{} is not in the 2nd edition",
                c.template, c.section
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

fn section_key(section: &str) -> (u32, u32) {
    let mut parts = section.split('.').map(|p| p.parse().unwrap_or(u32::MAX));
    (
        parts.next().unwrap_or(u32::MAX),
        parts.next().unwrap_or(u32::MAX),
    )
}

fn gate_rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let templates = templates();
        let store = SystemStore::new();
        let mut rows: Vec<Row> = CHECKS
            .iter()
            .map(|c| {
                let template = templates
                    .iter()
                    .find(|t| t.id == c.template)
                    .unwrap_or_else(|| panic!("no template {:?}", c.template));
                let lab = Lab::new(&store, template);
                let (book, measured, delta, tol, pass) = evaluate(&lab, &c.claim);
                Row {
                    chapter: chapter_of(c.section).to_string(),
                    section: c.section,
                    template: c.template,
                    claim: c.book,
                    book,
                    measured,
                    delta,
                    tol,
                    pass,
                }
            })
            .collect();
        rows.sort_by_key(|r| section_key(r.section));
        rows
    })
}

#[test]
fn strogatz_claims_reproduce() {
    let rows = gate_rows();
    print_table(rows);
    let failed: Vec<String> = rows
        .iter()
        .filter(|r| r.pass == Verdict::Fail)
        .map(|r| format!("§{} {}: {}", r.section, r.template, r.claim))
        .collect();
    assert!(failed.is_empty(), "failed claims:\n{}", failed.join("\n"));
}

#[test]
fn every_chapter_2_to_8_is_covered() {
    let rows = gate_rows();
    let missing: Vec<u32> = (2..=8)
        .filter(|ch| {
            !rows
                .iter()
                .any(|r| r.chapter == ch.to_string() && r.pass == Verdict::Pass)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "no passing claim in chapter(s) {missing:?}"
    );
}

#[test]
fn every_ch2_8_template_has_a_claim() {
    let unchecked: Vec<String> = templates()
        .into_iter()
        .filter(|t| {
            t.section
                .as_deref()
                .is_some_and(|s| (2..=8).contains(&section_key(s).0))
        })
        .filter(|t| !CHECKS.iter().any(|c| c.template == t.id))
        .map(|t| format!("{} (§{})", t.id, t.section.unwrap_or_default()))
        .collect();
    assert!(
        unchecked.is_empty(),
        "templates with no claim: {}",
        unchecked.join(", ")
    );
}
