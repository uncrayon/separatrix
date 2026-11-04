//! [`CompiledSystem`]: the compiled program behind the [`VectorField`] trait.

use dyn_core::{SystemKind, VectorField};

use crate::ad::Dual;
use crate::program::{Program, Scratch};
use crate::sema::Analysis;
use crate::SystemInfo;

type D1 = Dual<f64>;
type D2 = Dual<Dual<f64>>;

/// Direction of differentiation.
#[derive(Debug, Clone, Copy)]
enum Seed {
    State(usize),
    Param(usize),
}

/// How the program's outputs relate to the vector field.
#[derive(Debug, Clone, Copy)]
enum Form {
    /// One output per state: the field itself.
    Direct,
    /// One output, `H(q_1..q_k, p_1..p_k)`. The field is
    /// `q_i' = ∂H/∂p_i`, `p_i' = -∂H/∂q_i`, computed by automatic
    /// differentiation (second derivatives for the Jacobian).
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
    form: Form,
}

impl CompiledSystem {
    pub(crate) fn new(a: Analysis) -> CompiledSystem {
        let program = Program::compile(&a.roots);
        let form = match a.hamiltonian_dof {
            Some(dof) => Form::Hamiltonian { dof },
            None => Form::Direct,
        };
        CompiledSystem {
            info: a.info,
            program,
            form,
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
        match self.form {
            Form::Hamiltonian { .. } => Some(f64::with_regs(self.program.len(), |regs| {
                self.program.run(regs, t, |k| x[k], |k| p[k]);
                regs[self.program.outputs[0] as usize]
            })),
            Form::Direct => None,
        }
    }

    /// Number of instructions in the compiled program (after sharing
    /// common subexpressions). Useful to gauge evaluation cost.
    pub fn instruction_count(&self) -> usize {
        self.program.len()
    }

    /// Run the program over dual numbers where state `j` (if `Some`) or
    /// parameter `k` (if `Some`) is seeded; call `read` with the registers.
    fn run_dual1<R>(
        &self,
        t: f64,
        x: &[f64],
        p: &[f64],
        seed_x: Option<usize>,
        seed_p: Option<usize>,
        read: impl FnOnce(&[D1]) -> R,
    ) -> R {
        D1::with_regs(self.program.len(), |regs| {
            self.program.run(
                regs,
                D1::constant(t),
                |i| D1::new(x[i], if Some(i) == seed_x { 1.0 } else { 0.0 }),
                |i| D1::new(p[i], if Some(i) == seed_p { 1.0 } else { 0.0 }),
            );
            read(regs)
        })
    }

    /// Hamiltonian helper: the second derivative `∂²H / ∂x_a ∂(inner)`,
    /// where `inner` is a state or a parameter. `regs` is scratch space.
    fn second_derivative(
        &self,
        (t, x, p): (f64, &[f64], &[f64]),
        a: usize,
        inner: Seed,
        regs: &mut [D2],
    ) -> f64 {
        let (inner_x, inner_p) = match inner {
            Seed::State(b) => (Some(b), None),
            Seed::Param(k) => (None, Some(k)),
        };
        let one = |b: bool| if b { 1.0 } else { 0.0 };
        self.program.run(
            regs,
            D2::constant(D1::constant(t)),
            |i| {
                D2::new(
                    D1::new(x[i], one(Some(i) == inner_x)),
                    D1::new(one(i == a), 0.0),
                )
            },
            |i| D2::constant(D1::new(p[i], one(Some(i) == inner_p))),
        );
        regs[self.program.outputs[0] as usize].eps.eps
    }

    /// Reorder rows of the Hessian-like block `m` (rows: ∂/∂q.., ∂/∂p..)
    /// into the field's rows: `q_i' ← +∂/∂p_i`, `p_i' ← −∂/∂q_i`.
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
        match self.form {
            Form::Direct => f64::with_regs(self.program.len(), |regs| {
                self.program.run(regs, t, |k| x[k], |k| p[k]);
                for (o, &r) in out.iter_mut().zip(&self.program.outputs) {
                    *o = regs[r as usize];
                }
            }),
            Form::Hamiltonian { dof } => {
                for j in 0..2 * dof {
                    let g = self.run_dual1(t, x, p, Some(j), None, |regs| {
                        regs[self.program.outputs[0] as usize].eps
                    });
                    if j < dof {
                        out[dof + j] = -g; // p_j' = -∂H/∂q_j
                    } else {
                        out[j - dof] = g; // q_j' = ∂H/∂p_j
                    }
                }
            }
        }
    }

    fn jacobian(&self, t: f64, x: &[f64], p: &[f64], out: &mut [f64]) {
        let n = self.dim();
        match self.form {
            Form::Direct => {
                for j in 0..n {
                    self.run_dual1(t, x, p, Some(j), None, |regs| {
                        for (i, &r) in self.program.outputs.iter().enumerate() {
                            out[i * n + j] = regs[r as usize].eps;
                        }
                    });
                }
            }
            Form::Hamiltonian { dof } => {
                // Hessian of H (symmetric: compute the upper triangle).
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
        match self.form {
            Form::Direct => self.run_dual1(t, x, p, None, Some(k), |regs| {
                for (o, &r) in out.iter_mut().zip(&self.program.outputs) {
                    *o = regs[r as usize].eps;
                }
            }),
            Form::Hamiltonian { dof } => {
                D2::with_regs(self.program.len(), |regs| {
                    for (a, o) in out.iter_mut().enumerate().take(2 * dof) {
                        *o = self.second_derivative((t, x, p), a, Seed::Param(k), regs);
                    }
                });
                Self::symplectic_rows(dof, 1, out);
            }
        }
    }

    fn is_autonomous(&self) -> bool {
        self.info.autonomous
    }
}
