//! Zero-crossing detection ("events") during integration.

use serde::{Deserialize, Serialize};

use crate::Stepper;

/// Type of an event function `g(t, x)`.
pub type EventFn = dyn Fn(f64, &[f64]) -> f64 + Send + Sync;

/// An event function `g(t, x)` whose zero crossings should be reported.
///
/// ```
/// use dyn_solve::{Direction, Event};
/// // Poincaré section x_0 = 0, crossed upwards.
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

/// Remembers the previous value of each `g` and checks each new step.
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

    /// Look for crossings in the step the stepper just accepted and append
    /// them, in integration order, to `hits`.
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
                let t = brent(&mut g_dense, ta, tb, ga, gb, tol);
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

/// Brent's root finder (as in *Numerical Recipes*, `zbrent`).
///
/// Requires `f(a) = fa` and `f(b) = fb` of opposite signs. Combines
/// bisection (always safe) with secant and inverse quadratic interpolation
/// (fast). Returns `t` with the root bracketed to within about `tol`.
pub(crate) fn brent(
    f: &mut impl FnMut(f64) -> f64,
    a: f64,
    b: f64,
    fa: f64,
    fb: f64,
    tol: f64,
) -> f64 {
    const MAX_ITER: usize = 100;
    let (mut a, mut b, mut fa, mut fb) = (a, b, fa, fb);
    let (mut c, mut fc) = (b, fb);
    let (mut d, mut e) = (b - a, b - a);

    for _ in 0..MAX_ITER {
        if (fb > 0.0 && fc > 0.0) || (fb < 0.0 && fc < 0.0) {
            // Root is between a and b: rename so that it is between b and c.
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            // Make b the best guess so far.
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let tol1 = 2.0 * f64::EPSILON * b.abs() + 0.5 * tol;
        let xm = 0.5 * (c - b);
        if xm.abs() <= tol1 || fb == 0.0 {
            return b;
        }
        if e.abs() >= tol1 && fa.abs() > fb.abs() {
            // Try interpolation.
            let s = fb / fa;
            let (mut p, mut q);
            if a == c {
                // Secant.
                p = 2.0 * xm * s;
                q = 1.0 - s;
            } else {
                // Inverse quadratic.
                let qq = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * xm * qq * (qq - r) - (b - a) * (r - 1.0));
                q = (qq - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            }
            p = p.abs();
            let min1 = 3.0 * xm * q - (tol1 * q).abs();
            let min2 = (e * q).abs();
            if 2.0 * p < min1.min(min2) {
                e = d;
                d = p / q;
            } else {
                d = xm;
                e = d;
            }
        } else {
            // Bisection.
            d = xm;
            e = d;
        }
        a = b;
        fa = fb;
        b += if d.abs() > tol1 { d } else { tol1.copysign(xm) };
        fb = f(b);
    }
    b
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
        let r = brent(&mut f, 0.0, 2.0, -2.0, 2.0, 1e-14);
        assert!((r - 2f64.sqrt()).abs() < 1e-13, "{r}");
        assert!(calls < 20, "{calls} calls");

        // Reversed bracket (as in backward integration).
        let mut g = |x: f64| x.cos();
        let r = brent(&mut g, 3.0, 1.0, 3f64.cos(), 1f64.cos(), 1e-14);
        assert!((r - std::f64::consts::FRAC_PI_2).abs() < 1e-13, "{r}");
    }

    #[test]
    fn brent_handles_flat_and_steep_functions() {
        let mut f = |x: f64| (x - 0.3).powi(3);
        let (fa, fb) = (f(0.0), f(1.0));
        let r = brent(&mut f, 0.0, 1.0, fa, fb, 1e-12);
        assert!((r - 0.3).abs() < 1e-4, "{r}"); // cubic root: g is ~1e-12 within 1e-4
        let mut h = |x: f64| (50.0 * (x - 0.7)).tanh();
        let (ha, hb) = (h(0.0), h(1.0));
        let r = brent(&mut h, 0.0, 1.0, ha, hb, 1e-12);
        assert!((r - 0.7).abs() < 1e-11, "{r}");
    }
}
