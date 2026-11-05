//! The Strogatz gate: each gallery template from chapters 2 to 8 reproduces
//! a quantitative result of *Nonlinear Dynamics and Chaos*, 2nd edition
//! (Westview Press, 2015, ISBN 9780813349107), through the same service
//! functions the app calls. Every expected value comes from the book's
//! formula, never from a run.
//!
//! `scripts/strogatz-gate.sh` prints the table of every claim.
//!
//! The section titles in [`STROGATZ_2E`] were checked against the 2nd
//! edition's contents pages, as scanned by the Czech library cover service:
//! <https://obalkyknih.cz/file/toc/452162/pdf>.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::Arc;

use dyn_expr::CompiledSystem;
use dyn_solve::Sampling;
use separatrix_lib::ipc::{
    AxisRef, Classification, CompileRequest, CompileResponse, FixedPointOut, InputFormat, Method,
    PortraitRequest, Severity, SolverOptions, Status,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

/// Every section a template or a check may cite: (section, title) from the
/// 2nd edition's table of contents, chapters 2 to 12.
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

/// Bisection stops once the parameter bracket is narrower than this.
const BISECTION_WIDTH: f64 = 1e-9;

/// One result of the book, checked on one template.
struct Check {
    template: &'static str,
    section: &'static str,
    /// The book's statement, as printed in the table.
    book: &'static str,
    claim: Claim,
}

enum Claim {
    /// With `params` overriding the template's defaults, the fixed points
    /// found in the template's view are exactly `expect`, each within `tol`
    /// (max norm) and with that classification.
    FixedPoints {
        params: &'static [(&'static str, f64)],
        expect: &'static [(&'static [f64], Classification)],
        tol: f64,
    },
    /// The `observable` changes inside `bracket` of `param`, and bisection
    /// puts the change within `tol` of `book`.
    Bifurcation {
        param: &'static str,
        bracket: (f64, f64),
        observable: Observable,
        book: f64,
        tol: f64,
    },
    /// The largest `|energy(x(t)) − energy(x0)|` over the accepted DOPRI5
    /// steps to `t_end` stays within `bound`; `None` prints an info row.
    EnergyDrift {
        params: &'static [(&'static str, f64)],
        x0: &'static [f64],
        energy: fn(&[f64]) -> f64,
        t_end: f64,
        rtol: f64,
        atol: f64,
        bound: Option<f64>,
    },
}

enum Observable {
    /// The number of fixed points found in `search`, one interval per state.
    FixedPointCount { search: &'static [(f64, f64)] },
    /// Whether the fixed point nearest `near` is asymptotically stable.
    StabilityOf { near: &'static [f64] },
}

fn pendulum_energy(x: &[f64]) -> f64 {
    x[1] * x[1] / 2.0 - x[0].cos()
}

const PENDULUM_POINTS: &[(&[f64], Classification)] = &[
    (&[0.0, 0.0], Classification::Centre),
    (&[PI, 0.0], Classification::Saddle),
    (&[-PI, 0.0], Classification::Saddle),
    (&[2.0 * PI, 0.0], Classification::Centre),
    (&[-2.0 * PI, 0.0], Classification::Centre),
];

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

/// The chapter number of a section such as `"6.7"`.
fn chapter_of(section: &str) -> &str {
    section.split('.').next().unwrap_or(section)
}

fn in_second_edition(section: &str) -> bool {
    STROGATZ_2E.iter().any(|(s, _)| *s == section)
}

/// A template compiled through the app's service, with its first view.
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

    /// The template's parameter values with `overrides` applied.
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

    /// The fixed points `compute_portrait` reports in the template's view,
    /// or in `search` (one interval per state) when given.
    fn fixed_points(&self, params: Vec<f64>, search: Option<&[(f64, f64)]>) -> Vec<FixedPointOut> {
        let dim = self.res.states.len();
        let x_axis = self.axis(&self.template.view.x);
        let y_axis = self.axis(&self.template.view.y);
        let range = |axis: AxisRef, view: (f64, f64)| match (axis, search) {
            (AxisRef::State { index }, Some(s)) => s[index],
            _ => view,
        };
        let req = PortraitRequest {
            request_id: 1,
            system_id: self.res.system_id.unwrap(),
            params,
            x_axis,
            y_axis,
            x_range: range(x_axis, self.template.view.x_range),
            y_range: range(y_axis, self.template.view.y_range),
            base_state: vec![0.0; dim],
            field_time: 0.0,
            field_grid: 0,
            nullclines: false,
            fixed_points: true,
            trajectories: Vec::new(),
            solver: SolverOptions {
                method: Method::Dopri5,
                rtol: 1e-6,
                atol: 1e-9,
                h: 0.0,
                h_max: None,
                max_steps: 200_000,
            },
            samples: 0,
            graph_samples: None,
            transient: None,
        };
        service::compute_portrait(&self.system, &req)
            .unwrap_or_else(|e| panic!("{}: {e}", self.template.id))
            .fixed_points
    }
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

/// One printed line of the gate's table.
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

/// `(book, measured, delta, tol, pass)` of one claim.
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
    println!("{ch:<3} {sec:<4} {tpl:<20} {claim:<54} {book:<12} {meas:<26} {d:>8} {tol:>7} {res}");
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

#[test]
fn strogatz_claims_reproduce() {
    let templates = templates();
    let store = SystemStore::new();
    let rows: Vec<Row> = CHECKS
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
    print_table(&rows);
    let failed: Vec<String> = rows
        .iter()
        .filter(|r| r.pass == Verdict::Fail)
        .map(|r| format!("§{} {}: {}", r.section, r.template, r.claim))
        .collect();
    assert!(failed.is_empty(), "failed claims:\n{}", failed.join("\n"));
}
