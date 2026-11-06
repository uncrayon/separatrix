use dyn_core::VectorField;

use crate::events::EventTracker;
use crate::stepper::TimeDirection;
use crate::{
    all_finite, blew_up, Direction, Event, EventHit, Solution, SolverOptions, Stats, Status,
    StepOutcome, Stepper,
};

/// Where a Poincaré section samples a flow (Strogatz §8.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Section {
    /// The hyperplane `x[state] = value`. A crossing is a sign change of
    /// `g = x[state] − value` in `direction`, judged in the direction of
    /// integration (see [`Direction`]); starting on the plane is not one.
    Plane {
        /// Index of the state held fixed.
        state: usize,
        /// Where the plane cuts that state's axis.
        value: f64,
        /// Which crossings count.
        direction: Direction,
    },
    /// The stroboscopic map of a flow forced with period `period`: the state
    /// at `t_k = phase + k·period` for every integer `k` with `t_k` in
    /// the integration interval, `t0` and `t1` included.
    Stroboscopic {
        /// Forcing period, `> 0`.
        period: f64,
        /// One sample time, in absolute time (not from `t0`).
        phase: f64,
    },
}

/// Integrate from `(t0, x0)` towards `t1` like [`integrate`](crate::integrate)
/// and return only the points where the orbit meets `section`, in the order
/// the solver meets them.
///
/// [`Solution::t`] holds the crossing times and [`Solution::x`] the crossing
/// states (from the dense output; plane crossings are located with Brent's
/// method as in [`integrate_with_events`](crate::integrate_with_events), and
/// their held state is exactly the plane's `value`).
/// The first `skip` crossings are dropped as transient. The run stops after
/// `skip + max_points` crossings with [`Status::Completed`], or at `t1`, or
/// for any reason [`integrate`](crate::integrate) stops, with the same
/// [`Status`] and [`Stats`]. Unlike `integrate`, the last computed point is
/// never appended: every point is a crossing.
///
/// Stroboscopic times are `phase + k as f64 * period`, one multiply
/// and one add with no library function, so the same samples are kept on
/// every platform.
///
/// # Panics
/// If `x0.len() != f.dim()`, a plane's `state >= f.dim()`, or a
/// stroboscopic `period` is not a positive finite number or `phase` is not
/// finite.
#[allow(clippy::too_many_arguments)]
pub fn section(
    f: &dyn VectorField,
    p: &[f64],
    t0: f64,
    x0: &[f64],
    t1: f64,
    opts: &SolverOptions,
    section: &Section,
    skip: usize,
    max_points: usize,
) -> Solution {
    assert_eq!(
        x0.len(),
        f.dim(),
        "x0 has length {} but the system has dimension {}",
        x0.len(),
        f.dim()
    );
    let mut out = Crossings::new(skip, max_points);
    if !all_finite(x0) || !t0.is_finite() || !t1.is_finite() {
        return out.finish(Status::NonFinite, Stats::default());
    }
    if blew_up(x0) {
        return out.finish(Status::BlewUp, Stats::default());
    }

    let events: Vec<Event> = match *section {
        Section::Plane {
            state,
            value,
            direction,
        } => {
            assert!(
                state < f.dim(),
                "the section's state {state} is out of range for dimension {}",
                f.dim()
            );
            vec![Event {
                g: Box::new(move |_t, x| x[state] - value),
                direction,
            }]
        }
        Section::Stroboscopic { .. } => Vec::new(),
    };
    let mut sampler = match *section {
        Section::Plane { state, value, .. } => Sampler::Plane {
            tracker: EventTracker::new(&events, t0, x0),
            hits: Vec::new(),
            state,
            value,
        },
        Section::Stroboscopic { period, phase } => {
            assert!(
                period > 0.0 && period.is_finite() && phase.is_finite(),
                "a stroboscopic section needs a positive finite period and a finite phase"
            );
            Sampler::Stroboscopic(Strobe::new(t0, t1, period, phase, x0.len()))
        }
    };

    let mut stepper = Stepper::new(f, p, t0, x0, t1, opts);
    sampler.take(&stepper, &mut out);
    let status = loop {
        if out.is_full() {
            break Status::Completed;
        }
        if stepper.stats().accepted >= opts.max_steps && !stepper.is_done() {
            break Status::MaxSteps;
        }
        match stepper.step() {
            StepOutcome::AtEnd => break Status::Completed,
            StepOutcome::Accepted => sampler.take(&stepper, &mut out),
            StepOutcome::BlewUp => {
                sampler.take(&stepper, &mut out);
                break Status::BlewUp;
            }
            StepOutcome::Failed(status) => break status,
        }
    };
    out.finish(status, stepper.stats())
}

struct Crossings {
    skip: usize,
    wanted: usize,
    seen: usize,
    t: Vec<f64>,
    x: Vec<f64>,
}

impl Crossings {
    fn new(skip: usize, max_points: usize) -> Self {
        Self {
            skip,
            wanted: skip.saturating_add(max_points),
            seen: 0,
            t: Vec::new(),
            x: Vec::new(),
        }
    }

    fn is_full(&self) -> bool {
        self.seen >= self.wanted
    }

    fn push(&mut self, t: f64, x: &[f64]) {
        if self.seen >= self.skip {
            self.t.push(t);
            self.x.extend_from_slice(x);
        }
        self.seen += 1;
    }

    fn finish(self, status: Status, stats: Stats) -> Solution {
        Solution {
            t: self.t,
            x: self.x,
            status,
            stats,
        }
    }
}

enum Sampler<'e> {
    Plane {
        tracker: EventTracker<'e>,
        hits: Vec<EventHit>,
        state: usize,
        value: f64,
    },
    Stroboscopic(Strobe),
}

impl Sampler<'_> {
    fn take(&mut self, s: &Stepper, out: &mut Crossings) {
        match self {
            Sampler::Plane {
                tracker,
                hits,
                state,
                value,
            } => {
                tracker.after_step(s, hits);
                for mut hit in hits.drain(..) {
                    if out.is_full() {
                        break;
                    }
                    hit.x[*state] = *value;
                    out.push(hit.t, &hit.x);
                }
            }
            Sampler::Stroboscopic(strobe) => strobe.take(s, out),
        }
    }
}

struct Strobe {
    base: f64,
    period: f64,
    k: i64,
    step: i64,
    dir: TimeDirection,
    buf: Vec<f64>,
}

impl Strobe {
    fn new(t0: f64, t1: f64, period: f64, phase: f64, dim: usize) -> Self {
        let dir = TimeDirection::from_to(t0, t1);
        let step = match dir {
            TimeDirection::Forward => 1,
            TimeDirection::Backward => -1,
        };
        let guess = (t0 - phase) / period;
        let k = match dir {
            TimeDirection::Forward => guess.ceil(),
            TimeDirection::Backward => guess.floor(),
        } as i64;
        let mut strobe = Self {
            base: phase,
            period,
            k,
            step,
            dir,
            buf: vec![0.0; dim],
        };
        let from_start = |s: &Self, k: i64| (s.time(k) - t0) * dir.sign() >= 0.0;
        if !from_start(&strobe, strobe.k) {
            strobe.k += step;
        } else if from_start(&strobe, strobe.k - step) {
            strobe.k -= step;
        }
        strobe
    }

    fn time(&self, k: i64) -> f64 {
        self.base + k as f64 * self.period
    }

    fn take(&mut self, s: &Stepper, out: &mut Crossings) {
        while !out.is_full() {
            let tk = self.time(self.k);
            if (tk - s.t()) * self.dir.sign() > 0.0 {
                break;
            }
            if tk == s.t() {
                out.push(tk, s.x());
            } else {
                s.interpolate(tk, &mut self.buf);
                out.push(tk, &self.buf);
            }
            self.k += self.step;
        }
    }
}
