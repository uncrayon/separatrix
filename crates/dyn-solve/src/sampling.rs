use crate::stepper::TimeDirection;
use crate::{Sampling, Solution, Status, Stepper};

struct Grid {
    t0: f64,
    t1: f64,
    n: usize,
    next: usize,
}

impl Grid {
    /// The last sample sits exactly on `t1`, whatever the formula rounds to.
    fn is_last(&self, k: usize) -> bool {
        k + 1 == self.n
    }

    fn time(&self, k: usize) -> f64 {
        if k == 0 {
            self.t0
        } else if self.is_last(k) {
            self.t1
        } else {
            self.t0 + (self.t1 - self.t0) * (k as f64 / (self.n - 1) as f64)
        }
    }
}

/// Where the recorder takes its samples, mirroring [`Sampling`].
enum Output {
    Steps,
    Uniform(Grid),
}

pub(crate) struct Recorder {
    dir: TimeDirection,
    t: Vec<f64>,
    x: Vec<f64>,
    output: Output,
    buf: Vec<f64>,
}

impl Recorder {
    pub(crate) fn new(sampling: Sampling, t0: f64, t1: f64, x0: &[f64]) -> Self {
        let dir = TimeDirection::from_to(t0, t1);
        let mut r = Self {
            dir,
            t: Vec::new(),
            x: Vec::new(),
            output: Output::Steps,
            buf: vec![0.0; x0.len()],
        };
        match sampling {
            Sampling::Steps => r.push(t0, x0),
            Sampling::Uniform(n) => {
                r.t.reserve(n);
                r.x.reserve(n * x0.len());
                let mut grid = Grid { t0, t1, n, next: 0 };
                while grid.next < n && (grid.time(grid.next) - t0) * dir.sign() <= 0.0 {
                    r.push(grid.time(grid.next), x0);
                    grid.next += 1;
                }
                r.output = Output::Uniform(grid);
            }
        }
        r
    }

    fn push(&mut self, t: f64, x: &[f64]) {
        self.t.push(t);
        self.x.extend_from_slice(x);
    }

    pub(crate) fn record_step(&mut self, s: &Stepper) {
        let Output::Uniform(grid) = &mut self.output else {
            self.t.push(s.t());
            self.x.extend_from_slice(s.x());
            return;
        };
        while grid.next < grid.n {
            let tk = grid.time(grid.next);
            if (tk - s.t()) * self.dir.sign() > 0.0 {
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

    pub(crate) fn finish(mut self, s: &Stepper, status: Status) -> Solution {
        let wants_points = match &self.output {
            Output::Uniform(g) => g.n > 0,
            Output::Steps => true,
        };
        if status != Status::Completed && wants_points && self.t.last() != Some(&s.t()) {
            self.push(s.t(), s.x());
        }
        if let (Status::Completed, Output::Uniform(g)) = (status, &self.output) {
            debug_assert_eq!(g.next, g.n, "a completed run must fill the whole grid");
        }
        Solution {
            t: self.t,
            x: self.x,
            status,
            stats: s.stats(),
            stiff: s.looks_stiff(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_uniform_sample_is_exactly_t1() {
        let grid = Grid {
            t0: -0.1,
            t1: 0.2,
            n: 7,
            next: 0,
        };
        let by_formula = grid.t0 + (grid.t1 - grid.t0) * (6.0 / 6.0);
        assert_ne!(by_formula, grid.t1, "pick endpoints whose formula rounds");
        assert_eq!(grid.time(6), grid.t1);
    }
}
