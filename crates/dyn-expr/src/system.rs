use dyn_core::{fd_jacobian_derivative, SystemKind, VectorField};

use crate::ad::Dual;
use crate::program::{Inst, Op, Program, Reads, Scratch};
use crate::sema::{Analysis, Form};
use crate::SystemInfo;

type D1 = Dual<f64>;
type D2 = Dual<Dual<f64>>;

/// The one input whose derivative a dual-number run carries.
#[derive(Debug, Clone, Copy)]
enum Seed {
    State(usize),
    Param(usize),
}

impl Seed {
    fn on_state(self, i: usize) -> f64 {
        if matches!(self, Seed::State(j) if j == i) {
            1.0
        } else {
            0.0
        }
    }

    fn on_param(self, k: usize) -> f64 {
        if matches!(self, Seed::Param(j) if j == k) {
            1.0
        } else {
            0.0
        }
    }
}

/// How the program's outputs become the field.
#[derive(Debug, Clone, Copy)]
enum Layout {
    /// One output per state.
    Direct,
    /// One output, `H`; the field is its symplectic gradient.
    Hamiltonian { dof: usize },
}

/// A system of equations compiled from user input. Implements
/// [`VectorField`] with exact (automatic-differentiation) Jacobians.
///
/// Evaluation does not allocate: registers live in a per-thread buffer.
/// The type is `Send + Sync` and can be shared between threads.
///
/// ```
/// use dyn_core::VectorField;
/// use dyn_expr::{compile, InputFormat};
///
/// let out = compile("x' = -k*x", InputFormat::Text);
/// let sys = out.system.unwrap();
/// assert_eq!(sys.info.params[0].name, "k");
/// assert_eq!(sys.eval_alloc(0.0, &[2.0], &[3.0]), vec![-6.0]);
/// let mut j = [0.0];
/// sys.jacobian(0.0, &[2.0], &[3.0], &mut j);
/// assert_eq!(j, [-3.0]);
/// ```
#[derive(Debug, Clone)]
pub struct CompiledSystem {
    /// What was understood: kind, states, parameters, display equations.
    pub info: SystemInfo,
    program: Program,
    layout: Layout,
}

impl CompiledSystem {
    pub(crate) fn new(a: Analysis) -> CompiledSystem {
        let (program, layout) = match a.form {
            Form::Direct(roots) => (Program::compile(&roots), Layout::Direct),
            Form::Hamiltonian { h, dof } => (Program::compile(&[h]), Layout::Hamiltonian { dof }),
        };
        CompiledSystem {
            info: a.info,
            program,
            layout,
        }
    }

    /// Evaluate the field into a new vector (convenience for tests and
    /// one-off calls; use [`VectorField::eval`] in loops).
    pub fn eval_alloc(&self, t: f64, x: &[f64], p: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; self.dim()];
        self.eval(t, x, p, &mut out);
        out
    }

    /// Default parameter values, in parameter order.
    pub fn default_params(&self) -> Vec<f64> {
        self.info.params.iter().map(|p| p.default).collect()
    }

    /// Value of the Hamiltonian `H(q, p)` (the energy) for Hamiltonian
    /// systems; `None` otherwise.
    pub fn hamiltonian_value(&self, t: f64, x: &[f64], p: &[f64]) -> Option<f64> {
        match self.layout {
            Layout::Hamiltonian { .. } => Some(f64::with_regs(self.program.len(), |regs| {
                self.program.run(regs, t, |k| x[k], |k| p[k]);
                regs[self.program.outputs[0] as usize]
            })),
            Layout::Direct => None,
        }
    }

    /// `true` for a Hamiltonian that splits as `H = T(p) + V(q)` and does
    /// not read `t`, so a symplectic split-step method can integrate it.
    ///
    /// The check is structural. Below the sums, differences and negations at
    /// the top of `H`, and its products with or quotients by factors that
    /// read no state, every term must read only coordinates or only momenta.
    /// So `p²/2 − cos q` and `(p² + q²)/2` are separable, and `q p`, `p² q`
    /// and `(p² + q²)²` are not.
    pub fn is_separable(&self) -> bool {
        let Layout::Hamiltonian { dof } = self.layout else {
            return false;
        };
        let reads = self.program.reads(dof);
        let root = self.program.outputs[0];
        !reads[root as usize].time && self.splits(root, &reads)
    }

    fn splits(&self, r: u32, reads: &[Reads]) -> bool {
        let here = reads[r as usize];
        if !(here.low && here.high) {
            return true;
        }
        let free = |a: u32| reads[a as usize].no_state();
        match self.program.insts[r as usize] {
            Inst::Op(Op::Add(a, b) | Op::Sub(a, b)) => {
                self.splits(a, reads) && self.splits(b, reads)
            }
            Inst::Op(Op::Neg(a)) => self.splits(a, reads),
            Inst::Op(Op::Mul(a, b)) if free(a) => self.splits(b, reads),
            Inst::Op(Op::Mul(a, b) | Op::Div(a, b)) if free(b) => self.splits(a, reads),
            _ => false,
        }
    }

    /// Number of instructions in the compiled program (after sharing
    /// common subexpressions). Useful to gauge evaluation cost.
    pub fn instruction_count(&self) -> usize {
        self.program.len()
    }

    fn run_dual1<R>(
        &self,
        t: f64,
        x: &[f64],
        p: &[f64],
        seed: Seed,
        read: impl FnOnce(&[D1]) -> R,
    ) -> R {
        D1::with_regs(self.program.len(), |regs| {
            self.program.run(
                regs,
                D1::constant(t),
                |i| D1::new(x[i], seed.on_state(i)),
                |i| D1::new(p[i], seed.on_param(i)),
            );
            read(regs)
        })
    }

    /// One nested-dual run: the outer seed on state `a`, the inner one
    /// along `(dx, dp)`. Each output's `.eps.eps` is then its second
    /// derivative in `x_a` and along `(dx, dp)`.
    fn run_dual2(
        &self,
        (t, x, p): (f64, &[f64], &[f64]),
        a: usize,
        (dx, dp): (impl Fn(usize) -> f64, impl Fn(usize) -> f64),
        regs: &mut [D2],
    ) {
        let outer = Seed::State(a);
        self.program.run(
            regs,
            D2::constant(D1::constant(t)),
            |i| D2::new(D1::new(x[i], dx(i)), D1::new(outer.on_state(i), 0.0)),
            |i| D2::constant(D1::new(p[i], dp(i))),
        );
    }

    fn second_derivative(
        &self,
        txp: (f64, &[f64], &[f64]),
        a: usize,
        inner: Seed,
        regs: &mut [D2],
    ) -> f64 {
        let along = (|i| inner.on_state(i), |k| inner.on_param(k));
        self.run_dual2(txp, a, along, regs);
        regs[self.program.outputs[0] as usize].eps.eps
    }

    fn symplectic_rows(dof: usize, cols: usize, m: &mut [f64]) {
        for i in 0..dof {
            for c in 0..cols {
                let dq = m[i * cols + c];
                let dp = m[(dof + i) * cols + c];
                m[i * cols + c] = dp;
                m[(dof + i) * cols + c] = -dq;
            }
        }
    }
}

impl VectorField for CompiledSystem {
    fn dim(&self) -> usize {
        self.info.states.len()
    }

    fn n_params(&self) -> usize {
        self.info.params.len()
    }

    fn kind(&self) -> SystemKind {
        self.info.kind
    }

    fn eval(&self, t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        match self.layout {
            Layout::Direct => f64::with_regs(self.program.len(), |regs| {
                self.program.run(regs, t, |k| x[k], |k| p[k]);
                for (o, &r) in out.iter_mut().zip(&self.program.outputs) {
                    *o = regs[r as usize];
                }
            }),
            Layout::Hamiltonian { dof } => {
                for (j, o) in out.iter_mut().enumerate().take(2 * dof) {
                    *o = self.run_dual1(t, x, p, Seed::State(j), |regs| {
                        regs[self.program.outputs[0] as usize].eps
                    });
                }
                Self::symplectic_rows(dof, 1, out);
            }
        }
    }

    fn jacobian(&self, t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        let n = self.dim();
        match self.layout {
            Layout::Direct => {
                for j in 0..n {
                    self.run_dual1(t, x, p, Seed::State(j), |regs| {
                        for (i, &r) in self.program.outputs.iter().enumerate() {
                            out[i * n + j] = regs[r as usize].eps;
                        }
                    });
                }
            }
            Layout::Hamiltonian { dof } => {
                D2::with_regs(self.program.len(), |regs| {
                    for a in 0..n {
                        for b in a..n {
                            let h = self.second_derivative((t, x, p), a, Seed::State(b), regs);
                            out[a * n + b] = h;
                            out[b * n + a] = h;
                        }
                    }
                });
                Self::symplectic_rows(dof, n, out);
            }
        }
    }

    fn param_jacobian(&self, t: f64, x: &[f64], p: &[f64], k: usize, out: &mut [f64]) {
        match self.layout {
            Layout::Direct => self.run_dual1(t, x, p, Seed::Param(k), |regs| {
                for (o, &r) in out.iter_mut().zip(&self.program.outputs) {
                    *o = regs[r as usize].eps;
                }
            }),
            Layout::Hamiltonian { dof } => {
                D2::with_regs(self.program.len(), |regs| {
                    for (a, o) in out.iter_mut().enumerate().take(2 * dof) {
                        *o = self.second_derivative((t, x, p), a, Seed::Param(k), regs);
                    }
                });
                Self::symplectic_rows(dof, 1, out);
            }
        }
    }

    fn jacobian_derivative(
        &self,
        t: f64,
        x: &[f64],
        p: &[f64],
        dx: &[f64],
        dp: &[f64],
        out: &mut [f64],
    ) {
        let Layout::Direct = self.layout else {
            return fd_jacobian_derivative(self, t, x, p, dx, dp, out);
        };
        let n = self.dim();
        D2::with_regs(self.program.len(), |regs| {
            for j in 0..n {
                self.run_dual2((t, x, p), j, (|i| dx[i], |k| dp[k]), regs);
                for (i, &r) in self.program.outputs.iter().enumerate() {
                    out[i * n + j] = regs[r as usize].eps.eps;
                }
            }
        });
    }

    fn is_autonomous(&self) -> bool {
        self.info.autonomous
    }
}
