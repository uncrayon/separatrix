//! Flat register program: the compiled form of the right-hand sides.
//!
//! Each instruction writes one register, and its index *is* the register
//! number (static single assignment), so evaluation is one loop over a
//! slice. While building, identical instructions are shared (common
//! subexpression elimination), constant sub-trees are folded, and
//! instructions that no output needs are removed.
//!
//! Integer powers become multiplications (`x^2` → `x·x`) so they are fast
//! and differentiate exactly at any sign of `x`.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::ad::{Dual, Scalar};
use crate::ast::{BinOp, Func, Node};

/// Register index.
type Reg = u32;

/// One instruction; the result goes to the register with the same index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Inst {
    Const(f64),
    /// `t` (or `n` for maps).
    Time,
    State(u32),
    Param(u32),
    Neg(Reg),
    Add(Reg, Reg),
    Sub(Reg, Reg),
    Mul(Reg, Reg),
    Div(Reg, Reg),
    PowI(Reg, i32),
    PowC(Reg, f64),
    Pow(Reg, Reg),
    Call1(Func, Reg),
    Call2(Func, Reg, Reg),
}

impl Inst {
    /// Registers this instruction reads.
    fn operands(&self) -> [Option<Reg>; 2] {
        use Inst::*;
        match *self {
            Const(_) | Time | State(_) | Param(_) => [None, None],
            Neg(a) | PowI(a, _) | PowC(a, _) | Call1(_, a) => [Some(a), None],
            Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | Call2(_, a, b) => {
                [Some(a), Some(b)]
            }
        }
    }

    /// Same instruction with operands renamed by `f`.
    fn remap(self, f: impl Fn(Reg) -> Reg) -> Inst {
        use Inst::*;
        match self {
            Const(_) | Time | State(_) | Param(_) => self,
            Neg(a) => Neg(f(a)),
            Add(a, b) => Add(f(a), f(b)),
            Sub(a, b) => Sub(f(a), f(b)),
            Mul(a, b) => Mul(f(a), f(b)),
            Div(a, b) => Div(f(a), f(b)),
            PowI(a, n) => PowI(f(a), n),
            PowC(a, c) => PowC(f(a), c),
            Pow(a, b) => Pow(f(a), f(b)),
            Call1(g, a) => Call1(g, f(a)),
            Call2(g, a, b) => Call2(g, f(a), f(b)),
        }
    }

    /// Hashable identity for sharing. Commutative operands are sorted.
    fn key(&self) -> (u8, u64, u64, u64) {
        use Inst::*;
        let sorted = |a: Reg, b: Reg| (u64::from(a.min(b)), u64::from(a.max(b)));
        match *self {
            Const(c) => (0, c.to_bits(), 0, 0),
            Time => (1, 0, 0, 0),
            State(k) => (2, k.into(), 0, 0),
            Param(k) => (3, k.into(), 0, 0),
            Neg(a) => (4, a.into(), 0, 0),
            Add(a, b) => {
                let (x, y) = sorted(a, b);
                (5, x, y, 0)
            }
            Sub(a, b) => (6, a.into(), b.into(), 0),
            Mul(a, b) => {
                let (x, y) = sorted(a, b);
                (7, x, y, 0)
            }
            Div(a, b) => (8, a.into(), b.into(), 0),
            PowI(a, n) => (9, a.into(), n as u64, 0),
            PowC(a, c) => (10, a.into(), c.to_bits(), 0),
            Pow(a, b) => (11, a.into(), b.into(), 0),
            Call1(f, a) => (12, f as u64, a.into(), 0),
            Call2(f, a, b) => {
                let (x, y) = sorted(a, b); // min and max are commutative
                (13, f as u64, x, y)
            }
        }
    }
}

/// Apply a unary built-in function.
#[inline]
pub(crate) fn call1<T: Scalar>(f: Func, a: T) -> T {
    match f {
        Func::Sin => a.sin(),
        Func::Cos => a.cos(),
        Func::Tan => a.tan(),
        Func::Exp => a.exp(),
        Func::Ln => a.ln(),
        Func::Sqrt => a.sqrt(),
        Func::Cbrt => a.cbrt(),
        Func::Abs => a.abs(),
        Func::Sign => a.sign(),
        Func::Sinh => a.sinh(),
        Func::Cosh => a.cosh(),
        Func::Tanh => a.tanh(),
        Func::Asin => a.asin(),
        Func::Acos => a.acos(),
        Func::Atan => a.atan(),
        Func::Min | Func::Max => unreachable!("binary function in Call1"),
    }
}

/// Compute an arithmetic instruction given a way to read registers.
/// Loads (`Const`, `Time`, `State`, `Param`) are handled by the caller.
#[inline]
fn compute<T: Scalar>(inst: Inst, r: impl Fn(Reg) -> T) -> T {
    use Inst::*;
    match inst {
        Neg(a) => -r(a),
        Add(a, b) => r(a) + r(b),
        Sub(a, b) => r(a) - r(b),
        Mul(a, b) => r(a) * r(b),
        Div(a, b) => r(a) / r(b),
        PowI(a, n) => r(a).powi(n),
        PowC(a, c) => r(a).powc(c),
        Pow(a, b) => r(a).pow(r(b)),
        Call1(f, a) => call1(f, r(a)),
        Call2(Func::Min, a, b) => r(a).min(r(b)),
        Call2(_, a, b) => r(a).max(r(b)),
        Const(c) => T::cst(c),
        Time | State(_) | Param(_) => unreachable!("loads are handled by the caller"),
    }
}

/// A compiled set of expressions sharing one instruction list.
#[derive(Debug, Clone)]
pub(crate) struct Program {
    pub insts: Vec<Inst>,
    /// Register holding each requested expression, in order.
    pub outputs: Vec<Reg>,
}

impl Program {
    /// Compile the expressions `roots` into one program.
    pub(crate) fn compile(roots: &[Node]) -> Program {
        let mut b = Builder::default();
        let outputs: Vec<Reg> = roots.iter().map(|n| b.lower(n)).collect();
        Program { insts: b.insts, outputs }.without_dead_code()
    }

    /// Number of registers needed.
    pub(crate) fn len(&self) -> usize {
        self.insts.len()
    }

    /// Evaluate every instruction into `regs` (length [`Program::len`]).
    /// `x(k)` and `p(k)` give the k-th state and parameter.
    #[inline]
    pub(crate) fn run<T: Scalar>(
        &self,
        regs: &mut [T],
        t: T,
        x: impl Fn(usize) -> T,
        p: impl Fn(usize) -> T,
    ) {
        debug_assert!(regs.len() >= self.insts.len());
        for (i, &inst) in self.insts.iter().enumerate() {
            let v = match inst {
                Inst::Const(c) => T::cst(c),
                Inst::Time => t,
                Inst::State(k) => x(k as usize),
                Inst::Param(k) => p(k as usize),
                // Operands always refer to earlier registers.
                op => compute(op, |a| regs[a as usize]),
            };
            regs[i] = v;
        }
    }

    /// Drop instructions that no output depends on, and renumber.
    fn without_dead_code(self) -> Program {
        let n = self.insts.len();
        let mut live = vec![false; n];
        for &o in &self.outputs {
            live[o as usize] = true;
        }
        for i in (0..n).rev() {
            if live[i] {
                for a in self.insts[i].operands().into_iter().flatten() {
                    live[a as usize] = true;
                }
            }
        }
        let mut new_index = vec![u32::MAX; n];
        let mut insts = Vec::with_capacity(n);
        for (i, inst) in self.insts.iter().enumerate() {
            if live[i] {
                new_index[i] = insts.len() as u32;
                insts.push(inst.remap(|a| new_index[a as usize]));
            }
        }
        let outputs = self.outputs.iter().map(|&o| new_index[o as usize]).collect();
        Program { insts, outputs }
    }
}

/// Builds a [`Program`] with sharing and constant folding.
#[derive(Default)]
struct Builder {
    insts: Vec<Inst>,
    memo: HashMap<(u8, u64, u64, u64), Reg>,
}

impl Builder {
    fn constant(&self, r: Reg) -> Option<f64> {
        match self.insts[r as usize] {
            Inst::Const(c) => Some(c),
            _ => None,
        }
    }

    /// Add an instruction (or reuse an identical one). Instructions whose
    /// operands are all constants are evaluated now.
    fn push(&mut self, inst: Inst) -> Reg {
        let ops = inst.operands();
        let has_ops = ops[0].is_some();
        if has_ops && ops.iter().flatten().all(|&a| self.constant(a).is_some()) {
            let v = compute::<f64>(inst, |a| self.constant(a).expect("checked"));
            return self.push(Inst::Const(v));
        }
        let key = inst.key();
        if let Some(&r) = self.memo.get(&key) {
            return r;
        }
        let r = self.insts.len() as Reg;
        self.insts.push(inst);
        self.memo.insert(key, r);
        r
    }

    fn is_const(&self, r: Reg, v: f64) -> bool {
        self.constant(r) == Some(v)
    }

    fn lower(&mut self, n: &Node) -> Reg {
        match n {
            Node::Num(v) => self.push(Inst::Const(*v)),
            Node::Pi => self.push(Inst::Const(std::f64::consts::PI)),
            Node::E => self.push(Inst::Const(std::f64::consts::E)),
            Node::Time => self.push(Inst::Time),
            Node::State(k) => self.push(Inst::State(*k as u32)),
            Node::Param(k) => self.push(Inst::Param(*k as u32)),
            Node::Neg(a) => {
                let a = self.lower(a);
                if let Inst::Neg(inner) = self.insts[a as usize] {
                    return inner;
                }
                self.push(Inst::Neg(a))
            }
            Node::Bin(BinOp::Pow, base, exp) => self.lower_pow(base, exp),
            Node::Bin(op, a, b) => {
                let a = self.lower(a);
                let b = self.lower(b);
                match op {
                    BinOp::Add if self.is_const(b, 0.0) => a,
                    BinOp::Add if self.is_const(a, 0.0) => b,
                    BinOp::Add => self.push(Inst::Add(a, b)),
                    BinOp::Sub if self.is_const(b, 0.0) => a,
                    BinOp::Sub => self.push(Inst::Sub(a, b)),
                    BinOp::Mul if self.is_const(b, 1.0) => a,
                    BinOp::Mul if self.is_const(a, 1.0) => b,
                    BinOp::Mul => self.push(Inst::Mul(a, b)),
                    BinOp::Div if self.is_const(b, 1.0) => a,
                    BinOp::Div => self.push(Inst::Div(a, b)),
                    BinOp::Pow => unreachable!("handled above"),
                }
            }
            Node::Call(f, args) => {
                let a = self.lower(&args[0]);
                if f.arity() == 2 {
                    let b = self.lower(&args[1]);
                    self.push(Inst::Call2(*f, a, b))
                } else {
                    self.push(Inst::Call1(*f, a))
                }
            }
        }
    }

    fn lower_pow(&mut self, base: &Node, exp: &Node) -> Reg {
        let e = self.lower(exp);
        if matches!(base, Node::E) {
            return self.push(Inst::Call1(Func::Exp, e));
        }
        let b = self.lower(base);
        match self.constant(e) {
            Some(c) if c.fract() == 0.0 && c.abs() <= 1024.0 => self.powi(b, c as i32),
            Some(0.5) => self.push(Inst::Call1(Func::Sqrt, b)),
            Some(c) => self.push(Inst::PowC(b, c)),
            None => self.push(Inst::Pow(b, e)),
        }
    }

    /// `b^k` for an integer `k`, using multiplications for small `k`.
    fn powi(&mut self, b: Reg, k: i32) -> Reg {
        match k {
            0 => self.push(Inst::Const(1.0)),
            1 => b,
            2 => self.push(Inst::Mul(b, b)),
            3 => {
                let sq = self.push(Inst::Mul(b, b));
                self.push(Inst::Mul(sq, b))
            }
            4 => {
                let sq = self.push(Inst::Mul(b, b));
                self.push(Inst::Mul(sq, sq))
            }
            -1 => {
                let one = self.push(Inst::Const(1.0));
                self.push(Inst::Div(one, b))
            }
            _ => self.push(Inst::PowI(b, k)),
        }
    }
}

/// Scalar types with a per-thread register buffer, so evaluation does not
/// allocate on every call.
pub(crate) trait Scratch: Scalar {
    /// Run `f` with a register slice of length `len`.
    fn with_regs<R>(len: usize, f: impl FnOnce(&mut [Self]) -> R) -> R;
}

/// Programs up to this many instructions keep their registers on the stack.
const SMALL_PROGRAM: usize = 32;

macro_rules! impl_scratch {
    ($t:ty, $cell:ident) => {
        thread_local! {
            static $cell: RefCell<Vec<$t>> = const { RefCell::new(Vec::new()) };
        }
        impl Scratch for $t {
            #[inline]
            fn with_regs<R>(len: usize, f: impl FnOnce(&mut [Self]) -> R) -> R {
                // Small programs (the common case) use a stack array.
                if len <= SMALL_PROGRAM {
                    let mut buf = [<$t as Scalar>::cst(0.0); SMALL_PROGRAM];
                    return f(&mut buf[..len]);
                }
                $cell.with(|cell| match cell.try_borrow_mut() {
                    Ok(mut buf) => {
                        if buf.len() < len {
                            buf.resize(len, <$t as Scalar>::cst(0.0));
                        }
                        f(&mut buf[..len])
                    }
                    // Re-entrant call (should not happen): fall back to a
                    // fresh buffer rather than panic.
                    Err(_) => f(&mut vec![<$t as Scalar>::cst(0.0); len]),
                })
            }
        }
    };
}

impl_scratch!(f64, SCRATCH_F64);
impl_scratch!(Dual<f64>, SCRATCH_DUAL);
impl_scratch!(Dual<Dual<f64>>, SCRATCH_DUAL2);

#[cfg(test)]
mod tests {
    use super::*;

    fn st(k: usize) -> Node {
        Node::State(k)
    }

    fn eval(p: &Program, x: &[f64]) -> Vec<f64> {
        let mut regs = vec![0.0; p.len()];
        p.run(&mut regs, 0.0, |k| x[k], |_| 0.0);
        p.outputs.iter().map(|&o| regs[o as usize]).collect()
    }

    #[test]
    fn shares_common_subexpressions() {
        // (x+y)^2 and sin(x+y) share x+y; y+x is the same as x+y.
        let s = Node::bin(BinOp::Add, st(0), st(1));
        let s2 = Node::bin(BinOp::Add, st(1), st(0));
        let a = Node::bin(BinOp::Pow, s.clone(), Node::Num(2.0));
        let b = Node::Call(Func::Sin, vec![s2]);
        let p = Program::compile(&[a, b]);
        // x, y, x+y, (x+y)^2 via mul, sin
        assert_eq!(p.len(), 5, "{:?}", p.insts);
        let v = eval(&p, &[1.0, 2.0]);
        assert_eq!(v, vec![9.0, 3f64.sin()]);
    }

    #[test]
    fn folds_constants_and_drops_dead_code() {
        // x * (8/3) + 0 ; the exponent 2/2 folds to 1.
        let c = Node::bin(BinOp::Div, Node::Num(8.0), Node::Num(3.0));
        let e = Node::bin(BinOp::Div, Node::Num(2.0), Node::Num(2.0));
        let n = Node::bin(
            BinOp::Add,
            Node::bin(BinOp::Mul, Node::bin(BinOp::Pow, st(0), e), c),
            Node::Num(0.0),
        );
        let p = Program::compile(&[n]);
        assert_eq!(p.insts, vec![Inst::State(0), Inst::Const(8.0 / 3.0), Inst::Mul(0, 1)]);
    }

    #[test]
    fn integer_powers_are_products() {
        let p = Program::compile(&[Node::bin(BinOp::Pow, st(0), Node::Num(3.0))]);
        assert!(p.insts.iter().all(|i| !matches!(i, Inst::PowI(..) | Inst::PowC(..))));
        assert_eq!(eval(&p, &[-2.0]), vec![-8.0]);
        let p = Program::compile(&[Node::bin(BinOp::Pow, st(0), Node::Num(-3.0))]);
        assert_eq!(eval(&p, &[2.0]), vec![0.125]);
    }

    #[test]
    fn e_power_is_exp() {
        let p = Program::compile(&[Node::bin(BinOp::Pow, Node::E, st(0))]);
        assert_eq!(p.insts.last(), Some(&Inst::Call1(Func::Exp, 0)));
    }
}
