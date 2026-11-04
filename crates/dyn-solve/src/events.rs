use serde::{Deserialize, Serialize};

use crate::Stepper;

/// Type of an event function `g(t, x)`.
pub type EventFn = dyn Fn(f64, &[f64]) -> f64 + Send + Sync;

/// An event function `g(t, x)` whose zero crossings should be reported.
///
/// ```
/// use dyn_solve::{Direction, Event};
/// let e = Event { g: Box::new(|_t, x| x[0]), direction: Direction::Rising };
/// assert_eq!((e.g)(0.0, &[2.0]), 2.0);
/// ```
pub struct Event {
    /// The function whose sign changes are detected.
    pub g: Box<EventFn>,
    /// Which crossings to report.
    pub direction: Direction,
}

/// Which sign changes of `g` count as an event.
///
/// "Rising" means `g` goes from negative to non-negative *in the direction
/// of integration* (so, backwards in time when integrating backwards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// `g` goes from `< 0` to `>= 0`.
    Rising,
    /// `g` goes from `> 0` to `<= 0`.
    Falling,
    /// Either.
    Both,
}

/// A located zero crossing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventHit {
    /// Index into the `events` slice.
    pub event: usize,
    /// Crossing time.
    pub t: f64,
    /// State at the crossing (from the dense output).
    pub x: Vec<f64>,
    /// `true` for a [`Direction::Rising`] crossing.
    pub rising: bool,
}

pub(crate) struct EventTracker<'e> {
    events: &'e [Event],
    g_prev: Vec<f64>,
    buf: Vec<f64>,
    step_hits: Vec<EventHit>,
}

impl<'e> EventTracker<'e> {
    pub(crate) fn new(events: &'e [Event], t0: f64, x0: &[f64]) -> Self {
        let g_prev = events.iter().map(|e| (e.g)(t0, x0)).collect();
        Self {
            events,
            g_prev,
            buf: vec![0.0; x0.len()],
            step_hits: Vec::new(),
        }
    }

    pub(crate) fn after_step(&mut self, s: &Stepper, hits: &mut Vec<EventHit>) {
        let (ta, tb) = (s.t_prev(), s.t());
        for (i, event) in self.events.iter().enumerate() {
            let ga = self.g_prev[i];
            let gb = (event.g)(tb, s.x());
            self.g_prev[i] = gb;

            let rising = ga < 0.0 && gb >= 0.0;
            let falling = ga > 0.0 && gb <= 0.0;
            let wanted = match event.direction {
                Direction::Rising => rising,
                Direction::Falling => falling,
                Direction::Both => rising || falling,
            };
            if !wanted {
                continue;
            }

            let (t, x) = if gb == 0.0 {
                (tb, s.x().to_vec())
            } else {
                let buf = &mut self.buf;
                let mut g_dense = |t: f64| {
                    s.interpolate(t, buf);
                    (event.g)(t, buf)
                };
                let tol = 1e-12 * ta.abs().max(tb.abs()).max(1.0);
                let bracket = Bracket::new(ta, tb, ga, gb).expect("a wanted crossing changes sign");
                let t = brent(&mut g_dense, bracket, tol);
                let mut x = vec![0.0; s.dim()];
                s.interpolate(t, &mut x);
                (t, x)
            };
            self.step_hits.push(EventHit {
                event: i,
                t,
                x,
                rising,
            });
        }
        if self.step_hits.len() > 1 {
            let dir = (tb - ta).signum();
            self.step_hits
                .sort_by(|a, b| ((a.t - ta) * dir).total_cmp(&((b.t - ta) * dir)));
        }
        hits.append(&mut self.step_hits);
    }
}

/// An interval whose end values have opposite signs (or one is zero), so
/// it holds a root of a continuous function.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bracket {
    a: f64,
    b: f64,
    fa: f64,
    fb: f64,
}

impl Bracket {
    /// `None` unless `fa` and `fb` have opposite signs or one is zero.
    pub(crate) fn new(a: f64, b: f64, fa: f64, fb: f64) -> Option<Bracket> {
        let opposite = (fa <= 0.0 && fb >= 0.0) || (fa >= 0.0 && fb <= 0.0);
        opposite.then_some(Bracket { a, b, fa, fb })
    }
}

/// Brent's iterate: `b` is the best estimate, `c` the end of the bracket
/// opposite `b`, `a` the previous `b`; `d` is the last step and `e` the
/// one before.
struct Brent {
    a: f64,
    fa: f64,
    b: f64,
    fb: f64,
    c: f64,
    fc: f64,
    d: f64,
    e: f64,
}

impl Brent {
    fn start(br: Bracket) -> Brent {
        Brent {
            a: br.a,
            fa: br.fa,
            b: br.b,
            fb: br.fb,
            c: br.b,
            fc: br.fb,
            d: br.b - br.a,
            e: br.b - br.a,
        }
    }

    /// Make `[b, c]` a bracket again after `b` moved to `c`'s side.
    fn restore_bracket(&mut self) {
        if (self.fb > 0.0 && self.fc > 0.0) || (self.fb < 0.0 && self.fc < 0.0) {
            self.c = self.a;
            self.fc = self.fa;
            self.d = self.b - self.a;
            self.e = self.d;
        }
    }

    /// Make `b` the end of the bracket with the smaller `|f|`.
    fn take_best_guess(&mut self) {
        if self.fc.abs() < self.fb.abs() {
            self.a = self.b;
            self.b = self.c;
            self.c = self.a;
            self.fa = self.fb;
            self.fb = self.fc;
            self.fc = self.fa;
        }
    }

    /// The interpolation step from `b` as `p / q` with `p >= 0`: secant
    /// through `a` and `b` when `a == c`, inverse quadratic otherwise.
    fn interpolation(&self, xm: f64) -> (f64, f64) {
        let s = self.fb / self.fa;
        let (p, q) = if self.a == self.c {
            (2.0 * xm * s, 1.0 - s)
        } else {
            let qq = self.fa / self.fc;
            let r = self.fb / self.fc;
            (
                s * (2.0 * xm * qq * (qq - r) - (self.b - self.a) * (r - 1.0)),
                (qq - 1.0) * (r - 1.0) * (s - 1.0),
            )
        };
        if p > 0.0 {
            (p, -q)
        } else {
            (-p, q)
        }
    }

    fn bisect(&mut self, xm: f64) {
        self.d = xm;
        self.e = self.d;
    }
}

/// Brent's root finder (as in *Numerical Recipes*, `zbrent`).
pub(crate) fn brent(f: &mut impl FnMut(f64) -> f64, bracket: Bracket, tol: f64) -> f64 {
    const MAX_ITER: usize = 100;
    let mut s = Brent::start(bracket);
    for _ in 0..MAX_ITER {
        s.restore_bracket();
        s.take_best_guess();
        let tol1 = 2.0 * f64::EPSILON * s.b.abs() + 0.5 * tol;
        let xm = 0.5 * (s.c - s.b);
        if xm.abs() <= tol1 || s.fb == 0.0 {
            return s.b;
        }
        if s.e.abs() >= tol1 && s.fa.abs() > s.fb.abs() {
            let (p, q) = s.interpolation(xm);
            let min1 = 3.0 * xm * q - (tol1 * q).abs();
            let min2 = (s.e * q).abs();
            if 2.0 * p < min1.min(min2) {
                s.e = s.d;
                s.d = p / q;
            } else {
                s.bisect(xm);
            }
        } else {
            s.bisect(xm);
        }
        s.a = s.b;
        s.fa = s.fb;
        s.b += if s.d.abs() > tol1 {
            s.d
        } else {
            tol1.copysign(xm)
        };
        s.fb = f(s.b);
    }
    s.b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brent_finds_simple_roots() {
        let mut calls = 0;
        let mut f = |x: f64| {
            calls += 1;
            x * x - 2.0
        };
        let r = brent(&mut f, Bracket::new(0.0, 2.0, -2.0, 2.0).unwrap(), 1e-14);
        assert!((r - 2f64.sqrt()).abs() < 1e-13, "{r}");
        assert!(calls < 20, "{calls} calls");

        let mut g = |x: f64| x.cos();
        let bracket = Bracket::new(3.0, 1.0, 3f64.cos(), 1f64.cos()).unwrap();
        let r = brent(&mut g, bracket, 1e-14);
        assert!((r - std::f64::consts::FRAC_PI_2).abs() < 1e-13, "{r}");
    }

    #[test]
    fn brent_handles_flat_and_steep_functions() {
        let mut f = |x: f64| (x - 0.3).powi(3);
        let (fa, fb) = (f(0.0), f(1.0));
        let r = brent(&mut f, Bracket::new(0.0, 1.0, fa, fb).unwrap(), 1e-12);
        assert!((r - 0.3).abs() < 1e-4, "{r}");
        let mut h = |x: f64| (50.0 * (x - 0.7)).tanh();
        let (ha, hb) = (h(0.0), h(1.0));
        let r = brent(&mut h, Bracket::new(0.0, 1.0, ha, hb).unwrap(), 1e-12);
        assert!((r - 0.7).abs() < 1e-11, "{r}");
    }

    #[test]
    fn bracket_needs_a_sign_change() {
        assert!(Bracket::new(0.0, 1.0, -1.0, 1.0).is_some());
        assert!(Bracket::new(0.0, 1.0, 0.0, 1.0).is_some());
        assert!(Bracket::new(0.0, 1.0, 1.0, 2.0).is_none());
        assert!(Bracket::new(0.0, 1.0, f64::NAN, 1.0).is_none());
    }
}
