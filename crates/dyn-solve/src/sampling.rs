//! Collects the output points of an integration according to [`Sampling`].

use crate::{Sampling, Solution, Status, Stepper};

/// Evenly spaced output times `t0 + (t1 - t0) * k / (n - 1)`, `k = 0..n`.
struct Grid {
    t0: f64,
    t1: f64,
    n: usize,
    /// Index of the next grid time not yet written.
    next: usize,
}

impl Grid {
    fn time(&self, k: usize) -> f64 {
        if k == 0 {
            self.t0
        } else if k + 1 == self.n {
            self.t1 // exact, no rounding
        } else {
            self.t0 + (self.t1 - self.t0) * (k as f64 / (self.n - 1) as f64)
        }
    }
}

/// Accumulates `t` and row-major `x` while a [`Stepper`] runs.
pub(crate) struct Recorder {
    dir: f64,
    t: Vec<f64>,
    x: Vec<f64>,
    /// `None` for [`Sampling::Steps`].
    grid: Option<Grid>,
    buf: Vec<f64>,
}

impl Recorder {
    /// Set up the recorder and write the initial point(s).
    pub(crate) fn new(sampling: Sampling, t0: f64, t1: f64, x0: &[f64]) -> Self {
        let dir = if t1 < t0 { -1.0 } else { 1.0 };
        let mut r = Self {
            dir,
            t: Vec::new(),
            x: Vec::new(),
            grid: None,
            buf: vec![0.0; x0.len()],
        };
        match sampling {
            Sampling::Steps => r.push(t0, x0),
            Sampling::Uniform(n) => {
                r.t.reserve(n);
                r.x.reserve(n * x0.len());
                let mut grid = Grid { t0, t1, n, next: 0 };
                // Usually only k = 0; all of them when t0 == t1.
                while grid.next < n && (grid.time(grid.next) - t0) * dir <= 0.0 {
                    r.push(grid.time(grid.next), x0);
                    grid.next += 1;
                }
                r.grid = Some(grid);
            }
        }
        r
    }

    fn push(&mut self, t: f64, x: &[f64]) {
        self.t.push(t);
        self.x.extend_from_slice(x);
    }

    /// Record whatever falls inside the step the stepper just accepted.
    pub(crate) fn record_step(&mut self, s: &Stepper) {
        let Some(grid) = self.grid.as_mut() else {
            self.t.push(s.t());
            self.x.extend_from_slice(s.x());
            return;
        };
        while grid.next < grid.n {
            let tk = grid.time(grid.next);
            if (tk - s.t()) * self.dir > 0.0 {
                break;
            }
            if tk == s.t() {
                self.x.extend_from_slice(s.x());
            } else {
                s.interpolate(tk, &mut self.buf);
                self.x.extend_from_slice(&self.buf);
            }
            self.t.push(tk);
            grid.next += 1;
        }
    }

    /// Build the [`Solution`]. If integration stopped early, the last
    /// computed point is appended (unless it is already the last sample).
    pub(crate) fn finish(mut self, s: &Stepper, status: Status) -> Solution {
        let wants_points = match &self.grid {
            Some(g) => g.n > 0,
            None => true,
        };
        if status != Status::Completed && wants_points && self.t.last() != Some(&s.t()) {
            self.push(s.t(), s.x());
        }
        if let (Status::Completed, Some(g)) = (status, &self.grid) {
            debug_assert_eq!(g.next, g.n, "a completed run must fill the whole grid");
        }
        Solution {
            t: self.t,
            x: self.x,
            status,
            stats: s.stats(),
        }
    }
}
