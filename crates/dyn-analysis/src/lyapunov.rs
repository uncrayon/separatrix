//! The Lyapunov spectrum by the Benettin (QR) method: Benettin, Galgani,
//! Giorgilli & Strelcyn, *Meccanica* 15, 1980.

use std::cell::RefCell;

use dyn_core::{SystemKind, VectorField};
use dyn_solve::{SolverOptions, Stats, Status, StepOutcome, Stepper, BLOW_UP_THRESHOLD};

/// How long to run and how often to renormalize.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LyapunovOptions {
    /// Time (flow) or iterates (map, rounded) run before measuring, so the
    /// orbit settles onto the attractor.
    pub transient: f64,
    /// Time (flow) or iterates (map, rounded) measured after the transient.
    pub total: f64,
    /// Flow: time `τ` between two QR steps. Map: iterates per QR step
    /// (rounded, at least 1). The last interval is shortened to end at
    /// `total`.
    pub interval: f64,
    /// Consecutive blocks the renormalizations are split into; the spread
    /// of the per-block exponents gives the uncertainty. At least 2.
    pub blocks: usize,
    /// Most running-estimate samples kept: every `k`-th renormalization,
    /// with `k` the smallest stride that fits, and always the last one.
    pub history: usize,
}

/// The measured spectrum.
#[derive(Debug, Clone, PartialEq)]
pub struct LyapunovSpectrum {
    /// `λ1 … λn` in Gram–Schmidt column order (per unit time, or per
    /// iterate for a map); the first is the largest. Empty when no
    /// renormalization was completed.
    pub exponents: Vec<f64>,
    /// Standard error of each exponent over the blocks reached: the sample
    /// standard deviation of the per-block exponents divided by `√B`.
    /// Empty when fewer than two blocks were reached.
    pub uncertainty: Vec<f64>,
    /// `Σ λ_i`.
    pub sum: f64,
    /// What `Σ λ_i` must equal: flow, the time average of `tr J` along the
    /// orbit (one extra state `s' = tr J`); map, the mean of `ln |det J|`.
    pub trace_average: f64,
    /// Time (flow) or iterate index (map) of each running-estimate sample.
    pub history_t: Vec<f64>,
    /// Running estimates of every exponent, row-major:
    /// sample `k` is `history[k·n .. (k + 1)·n]`.
    pub history: Vec<f64>,
    /// QR steps completed.
    pub renormalizations: usize,
    /// Why the run stopped. The estimates cover what was completed.
    pub status: Status,
    /// Work counters, transient included.
    pub stats: Stats,
}

/// Measure the Lyapunov spectrum of `f` from `(t0, x0)`.
///
/// For a flow, `solver` integrates the orbit together with its tangent
/// vectors (landing exactly on every renormalization time) and its
/// `max_steps` bounds the accepted steps of the whole run, transient
/// included. A map ignores `solver`, and `t0` (iterate indices start at 0).
///
/// ```
/// use dyn_analysis::{lyapunov_spectrum, LyapunovOptions};
/// use dyn_core::{FnField, SystemKind};
/// use dyn_solve::{SolverOptions, Status};
///
/// // The logistic map at r = 4 has λ = ln 2.
/// let f = FnField::new(1, 1, SystemKind::Map, |_n, x, p, out| out[0] = p[0] * x[0] * (1.0 - x[0]));
/// let opts = LyapunovOptions { transient: 100.0, total: 1e5, interval: 1.0, blocks: 10, history: 0 };
/// let s = lyapunov_spectrum(&f, &[4.0], 0.0, &[0.1], &SolverOptions::default(), &opts);
/// assert_eq!(s.status, Status::Completed);
/// assert!((s.exponents[0] - 2f64.ln()).abs() < 1e-2);
/// ```
///
/// # Panics
/// If `x0.len() != f.dim()`.
pub fn lyapunov_spectrum(
    f: &dyn VectorField,
    p: &[f64],
    t0: f64,
    x0: &[f64],
    solver: &SolverOptions,
    opts: &LyapunovOptions,
) -> LyapunovSpectrum {
    assert_eq!(
        x0.len(),
        f.dim(),
        "x0 has length {} but the system has dimension {}",
        x0.len(),
        f.dim()
    );
    match f.kind() {
        SystemKind::Ode => flow_spectrum(f, p, t0, x0, solver, opts),
        SystemKind::Map => map_spectrum(f, p, x0, opts),
    }
}

fn finite(v: &[f64]) -> bool {
    v.iter().all(|a| a.is_finite())
}

fn escaped(v: &[f64]) -> bool {
    v.iter().any(|a| a.abs() > BLOW_UP_THRESHOLD)
}

fn add_stats(a: Stats, b: Stats) -> Stats {
    Stats {
        accepted: a.accepted + b.accepted,
        rejected: a.rejected + b.rejected,
        evals: a.evals + b.evals,
    }
}

#[derive(Clone, Copy)]
struct RowMajor<'a> {
    n: usize,
    a: &'a [f64],
}

impl RowMajor<'_> {
    fn row(&self, i: usize) -> &[f64] {
        &self.a[i * self.n..(i + 1) * self.n]
    }

    fn trace(&self) -> f64 {
        (0..self.n).map(|i| self.a[i * self.n + i]).sum()
    }
}

#[derive(Clone, Copy)]
struct ColMajor<'a> {
    n: usize,
    a: &'a [f64],
}

impl ColMajor<'_> {
    fn col(&self, j: usize) -> &[f64] {
        &self.a[j * self.n..(j + 1) * self.n]
    }
}

struct ColMajorMut<'a> {
    n: usize,
    a: &'a mut [f64],
}

impl ColMajorMut<'_> {
    fn col_mut(&mut self, j: usize) -> &mut [f64] {
        &mut self.a[j * self.n..(j + 1) * self.n]
    }

    fn set_identity(&mut self) {
        self.a.fill(0.0);
        for j in 0..self.n {
            self.a[j * self.n + j] = 1.0;
        }
    }
}

const WHOLE_RATIO_SLACK: f64 = 1e-9;

fn planned_steps(total: f64, interval: f64) -> usize {
    let ratio = total / interval;
    if ratio.is_finite() && ratio > 0.0 {
        ((ratio - WHOLE_RATIO_SLACK).ceil() as usize).max(1)
    } else {
        1
    }
}

struct Accumulator {
    n: usize,
    planned: usize,
    blocks: usize,
    stride: Option<usize>,
    done: usize,
    elapsed: f64,
    trace: f64,
    log_r: Vec<f64>,
    block_log_r: Vec<f64>,
    block_elapsed: Vec<f64>,
    history_t: Vec<f64>,
    history: Vec<f64>,
}

impl Accumulator {
    fn new(n: usize, planned: usize, blocks: usize, history: usize) -> Self {
        let blocks = blocks.max(1);
        let stride = (history > 0).then(|| planned.div_ceil(history).max(1));
        let samples = planned.min(history);
        Self {
            n,
            planned,
            blocks,
            stride,
            done: 0,
            elapsed: 0.0,
            trace: 0.0,
            log_r: vec![0.0; n],
            block_log_r: vec![0.0; blocks * n],
            block_elapsed: vec![0.0; blocks],
            history_t: Vec::with_capacity(samples),
            history: Vec::with_capacity(samples * n),
        }
    }

    fn renormalize(&mut self, q: ColMajorMut, dt: f64, trace: f64, t: f64) {
        let n = self.n;
        let q = q.a;
        let block = self.done * self.blocks / self.planned.max(1);
        let block = block.min(self.blocks - 1);
        for j in 0..n {
            let (done, rest) = q.split_at_mut(j * n);
            let v = &mut rest[..n];
            for _ in 0..2 {
                for i in 0..j {
                    let qi = &done[i * n..(i + 1) * n];
                    let r: f64 = qi.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
                    for (vk, qk) in v.iter_mut().zip(qi) {
                        *vk -= r * qk;
                    }
                }
            }
            let norm = v.iter().map(|a| a * a).sum::<f64>().sqrt();
            for vk in v.iter_mut() {
                *vk /= norm;
            }
            let ln = norm.ln();
            self.log_r[j] += ln;
            self.block_log_r[block * n + j] += ln;
        }
        self.block_elapsed[block] += dt;
        self.elapsed += dt;
        self.trace += trace;
        self.done += 1;
        let sample = self
            .stride
            .is_some_and(|k| self.done.is_multiple_of(k) || self.done == self.planned);
        if sample {
            self.history_t.push(t);
            self.history
                .extend(self.log_r.iter().map(|l| l / self.elapsed));
        }
    }

    fn finish(self, status: Status, stats: Stats) -> LyapunovSpectrum {
        let n = self.n;
        let measured = self.done > 0 && self.elapsed > 0.0;
        let exponents: Vec<f64> = if measured {
            self.log_r.iter().map(|l| l / self.elapsed).collect()
        } else {
            Vec::new()
        };
        let reached: Vec<usize> = (0..self.blocks)
            .filter(|&b| self.block_elapsed[b] > 0.0)
            .collect();
        let uncertainty = if reached.len() >= 2 {
            let b = reached.len() as f64;
            (0..n)
                .map(|i| {
                    let per_block = || {
                        reached
                            .iter()
                            .map(|&k| self.block_log_r[k * n + i] / self.block_elapsed[k])
                    };
                    let mean = per_block().sum::<f64>() / b;
                    let var = per_block().map(|l| (l - mean) * (l - mean)).sum::<f64>() / (b - 1.0);
                    (var / b).sqrt()
                })
                .collect()
        } else {
            Vec::new()
        };
        LyapunovSpectrum {
            sum: exponents.iter().sum(),
            trace_average: if measured {
                self.trace / self.elapsed
            } else {
                0.0
            },
            exponents,
            uncertainty,
            history_t: self.history_t,
            history: self.history,
            renormalizations: self.done,
            status,
            stats,
        }
    }
}

fn advance(stepper: &mut Stepper, max_steps: usize) -> Result<(), Status> {
    loop {
        if stepper.stats().accepted >= max_steps && !stepper.is_done() {
            return Err(Status::MaxSteps);
        }
        match stepper.step() {
            StepOutcome::AtEnd => return Ok(()),
            StepOutcome::Accepted => {}
            StepOutcome::BlewUp => return Err(Status::BlewUp),
            StepOutcome::Failed(status) => return Err(status),
        }
    }
}

fn flow_spectrum(
    f: &dyn VectorField,
    p: &[f64],
    t0: f64,
    x0: &[f64],
    solver: &SolverOptions,
    opts: &LyapunovOptions,
) -> LyapunovSpectrum {
    let n = f.dim();
    let planned = planned_steps(opts.total, opts.interval);
    let mut acc = Accumulator::new(n, planned, opts.blocks, opts.history);
    if !finite(x0) || !t0.is_finite() {
        return acc.finish(Status::NonFinite, Stats::default());
    }
    if escaped(x0) {
        return acc.finish(Status::BlewUp, Stats::default());
    }

    let mut x = x0.to_vec();
    let mut t = t0;
    let mut stats = Stats::default();
    if opts.transient > 0.0 {
        let mut settle = Stepper::new(f, p, t0, x0, t0 + opts.transient, solver);
        let outcome = advance(&mut settle, solver.max_steps);
        stats = settle.stats();
        if let Err(status) = outcome {
            return acc.finish(status, stats);
        }
        x.copy_from_slice(settle.x());
        t = settle.t();
    }

    let layout = TangentLayout { n };
    let tangent = Tangent { f, layout };
    let mut z = vec![0.0; layout.len()];
    let (z_x, mut phi, _) = layout.split_mut(&mut z);
    z_x.copy_from_slice(&x);
    phi.set_identity();
    let t_start = t;
    let budget = solver.max_steps.saturating_sub(stats.accepted);
    let first_end = t_start + opts.interval.min(opts.total);
    let mut stepper = Stepper::new(&tangent, p, t_start, &z, first_end, solver);
    let mut status = Status::Completed;
    let mut t_last = t_start;
    for k in 1..=planned {
        let t_end = if k == planned {
            t_start + opts.total
        } else {
            t_start + k as f64 * opts.interval
        };
        stepper.set_t_end(t_end);
        if let Err(s) = advance(&mut stepper, budget) {
            status = s;
            break;
        }
        z.copy_from_slice(stepper.x());
        let t_now = stepper.t();
        let (_, phi, s) = layout.split_mut(&mut z);
        acc.renormalize(phi, t_now - t_last, *s, t_now);
        *s = 0.0;
        if !finite(&z) {
            status = Status::NonFinite;
            break;
        }
        stepper.set_state(t_now, &z);
        t_last = t_now;
    }
    acc.finish(status, add_stats(stats, stepper.stats()))
}

fn map_spectrum(
    f: &dyn VectorField,
    p: &[f64],
    x0: &[f64],
    opts: &LyapunovOptions,
) -> LyapunovSpectrum {
    let n = f.dim();
    let iterates = |v: f64| {
        if v.is_finite() {
            v.round().max(0.0) as usize
        } else {
            0
        }
    };
    let transient = iterates(opts.transient);
    let total = iterates(opts.total).max(1);
    let every = iterates(opts.interval).max(1);
    let planned = total.div_ceil(every);
    let mut acc = Accumulator::new(n, planned, opts.blocks, opts.history);
    let mut stats = Stats::default();
    if !finite(x0) {
        return acc.finish(Status::NonFinite, stats);
    }
    if escaped(x0) {
        return acc.finish(Status::BlewUp, stats);
    }

    let mut x = x0.to_vec();
    let mut next = vec![0.0; n];
    let step = |k: usize, x: &mut Vec<f64>, next: &mut Vec<f64>, stats: &mut Stats| {
        f.eval(k as f64, x, p, next);
        stats.evals += 1;
        if !finite(next) {
            return Err(Status::NonFinite);
        }
        std::mem::swap(x, next);
        stats.accepted += 1;
        if escaped(x) {
            return Err(Status::BlewUp);
        }
        Ok(())
    };
    for k in 0..transient {
        if let Err(status) = step(k, &mut x, &mut next, &mut stats) {
            return acc.finish(status, stats);
        }
    }

    let mut q = vec![0.0; n * n];
    ColMajorMut { n, a: &mut q }.set_identity();
    let mut jac = vec![0.0; n * n];
    let mut lu = vec![0.0; n * n];
    let mut jq = vec![0.0; n * n];
    let mut log_det = 0.0;
    let mut since = 0usize;
    let mut status = Status::Completed;
    for i in 0..total {
        let k = transient + i;
        f.jacobian(k as f64, &x, p, &mut jac);
        lu.copy_from_slice(&jac);
        log_det += ln_abs_det(n, &mut lu);
        multiply(
            RowMajor { n, a: &jac },
            ColMajor { n, a: &q },
            ColMajorMut { n, a: &mut jq },
        );
        std::mem::swap(&mut q, &mut jq);
        since += 1;
        if let Err(s) = step(k, &mut x, &mut next, &mut stats) {
            status = s;
            break;
        }
        if since == every || i + 1 == total {
            if !finite(&q) {
                status = Status::NonFinite;
                break;
            }
            acc.renormalize(
                ColMajorMut { n, a: &mut q },
                since as f64,
                log_det,
                (k + 1) as f64,
            );
            since = 0;
            log_det = 0.0;
        }
    }
    acc.finish(status, stats)
}

fn multiply(jac: RowMajor, q: ColMajor, mut out: ColMajorMut) {
    for c in 0..q.n {
        let col = q.col(c);
        for (i, o) in out.col_mut(c).iter_mut().enumerate() {
            *o = jac.row(i).iter().zip(col).map(|(a, b)| a * b).sum();
        }
    }
}

fn ln_abs_det(n: usize, a: &mut [f64]) -> f64 {
    let mut sum = 0.0;
    for c in 0..n {
        let pivot = (c..n)
            .max_by(|&i, &j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs()))
            .unwrap_or(c);
        if pivot != c {
            for k in 0..n {
                a.swap(c * n + k, pivot * n + k);
            }
        }
        let d = a[c * n + c];
        if d == 0.0 {
            return f64::NEG_INFINITY;
        }
        sum += d.abs().ln();
        for r in c + 1..n {
            let l = a[r * n + c] / d;
            for k in c..n {
                a[r * n + k] -= l * a[c * n + k];
            }
        }
    }
    sum
}

const STACK_DIM: usize = 16;

thread_local! {
    static JACOBIAN: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
}

fn with_jacobian_scratch<R>(len: usize, run: impl FnOnce(&mut [f64]) -> R) -> R {
    if len <= STACK_DIM * STACK_DIM {
        let mut buf = [0.0; STACK_DIM * STACK_DIM];
        run(&mut buf[..len])
    } else {
        JACOBIAN.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.resize(len, 0.0);
            run(&mut buf[..len])
        })
    }
}

#[derive(Clone, Copy)]
struct TangentLayout {
    n: usize,
}

impl TangentLayout {
    fn len(self) -> usize {
        self.n + self.n * self.n + 1
    }

    fn split(self, z: &[f64]) -> (&[f64], ColMajor<'_>) {
        let (x, rest) = z.split_at(self.n);
        let phi = ColMajor {
            n: self.n,
            a: &rest[..self.n * self.n],
        };
        (x, phi)
    }

    fn split_mut(self, z: &mut [f64]) -> (&mut [f64], ColMajorMut<'_>, &mut f64) {
        let (x, rest) = z.split_at_mut(self.n);
        let (phi, s) = rest.split_at_mut(self.n * self.n);
        let phi = ColMajorMut { n: self.n, a: phi };
        (x, phi, &mut s[0])
    }
}

struct Tangent<'a> {
    f: &'a dyn VectorField,
    layout: TangentLayout,
}

impl VectorField for Tangent<'_> {
    fn dim(&self) -> usize {
        self.layout.len()
    }
    fn n_params(&self) -> usize {
        self.f.n_params()
    }
    fn kind(&self) -> SystemKind {
        SystemKind::Ode
    }
    fn is_autonomous(&self) -> bool {
        self.f.is_autonomous()
    }
    fn eval(&self, t: f64, z: &[f64], p: &[f64], out: &mut [f64]) {
        let n = self.layout.n;
        let (x, phi) = self.layout.split(z);
        let (fx, dphi, ds) = self.layout.split_mut(out);
        self.f.eval(t, x, p, fx);
        with_jacobian_scratch(n * n, |jac| {
            self.f.jacobian(t, x, p, jac);
            let jac = RowMajor { n, a: jac };
            multiply(jac, phi, dphi);
            *ds = jac.trace();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_steps_absorb_rounding() {
        assert_eq!(planned_steps(1e4, 0.1), 100_000);
        assert_eq!(planned_steps(1.05, 0.1), 11);
        assert_eq!(planned_steps(0.01, 0.1), 1);
    }

    #[test]
    fn determinant_with_pivoting() {
        let mut a = [0.0, 2.0, 3.0, 1.0];
        assert!((ln_abs_det(2, &mut a) - 6f64.ln()).abs() < 1e-15);
        let mut singular = [1.0, 2.0, 2.0, 4.0];
        assert_eq!(ln_abs_det(2, &mut singular), f64::NEG_INFINITY);
    }
}
