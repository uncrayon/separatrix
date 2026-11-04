use std::cell::RefCell;
use std::collections::HashMap;

use crate::ad::{Dual, Scalar};
use crate::ast::{BinOp, Func, Node};

type Reg = u32;

/// An `f64` compared and hashed by its bits, so instructions can be
/// memoised by derived `Eq`/`Hash`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Bits(u64);

impl Bits {
    pub(crate) fn of(v: f64) -> Bits {
        Bits(v.to_bits())
    }

    pub(crate) fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// An instruction that reads an input rather than registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Load {
    Const(Bits),
    Time,
    State(u32),
    Param(u32),
}

/// An instruction computed from earlier registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Op {
    Neg(Reg),
    Add(Reg, Reg),
    Sub(Reg, Reg),
    Mul(Reg, Reg),
    Div(Reg, Reg),
    PowI(Reg, i32),
    PowC(Reg, Bits),
    Pow(Reg, Reg),
    Call1(Func, Reg),
    Call2(Func, Reg, Reg),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Inst {
    Load(Load),
    Op(Op),
}

impl Op {
    fn operands(self) -> [Option<Reg>; 2] {
        use Op::*;
        match self {
            Neg(a) | PowI(a, _) | PowC(a, _) | Call1(_, a) => [Some(a), None],
            Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | Call2(_, a, b) => {
                [Some(a), Some(b)]
            }
        }
    }

    fn remap(self, f: impl Fn(Reg) -> Reg) -> Op {
        use Op::*;
        match self {
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

    /// The same operation with commutative operands in a fixed order, so
    /// `a + b` and `b + a` share one register.
    fn normalised(self) -> Op {
        use Op::*;
        let sorted = |a: Reg, b: Reg| (a.min(b), a.max(b));
        match self {
            Add(a, b) => {
                let (x, y) = sorted(a, b);
                Add(x, y)
            }
            Mul(a, b) => {
                let (x, y) = sorted(a, b);
                Mul(x, y)
            }
            Call2(f, a, b) if f.is_commutative() => {
                let (x, y) = sorted(a, b);
                Call2(f, x, y)
            }
            op => op,
        }
    }
}

impl Inst {
    fn operands(self) -> [Option<Reg>; 2] {
        match self {
            Inst::Load(_) => [None, None],
            Inst::Op(op) => op.operands(),
        }
    }

    fn remap(self, f: impl Fn(Reg) -> Reg) -> Inst {
        match self {
            Inst::Load(_) => self,
            Inst::Op(op) => Inst::Op(op.remap(f)),
        }
    }

    fn normalised(self) -> Inst {
        match self {
            Inst::Load(_) => self,
            Inst::Op(op) => Inst::Op(op.normalised()),
        }
    }
}

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

#[inline]
fn compute<T: Scalar>(op: Op, r: impl Fn(Reg) -> T) -> T {
    use Op::*;
    match op {
        Neg(a) => -r(a),
        Add(a, b) => r(a) + r(b),
        Sub(a, b) => r(a) - r(b),
        Mul(a, b) => r(a) * r(b),
        Div(a, b) => r(a) / r(b),
        PowI(a, n) => r(a).powi(n),
        PowC(a, c) => r(a).powc(c.get()),
        Pow(a, b) => r(a).pow(r(b)),
        Call1(f, a) => call1(f, r(a)),
        Call2(Func::Min, a, b) => r(a).min(r(b)),
        Call2(_, a, b) => r(a).max(r(b)),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Program {
    pub insts: Vec<Inst>,
    pub outputs: Vec<Reg>,
}

impl Program {
    pub(crate) fn compile(roots: &[Node]) -> Program {
        let mut b = Builder::default();
        let outputs: Vec<Reg> = roots.iter().map(|n| b.lower(n)).collect();
        Program {
            insts: b.insts,
            outputs,
        }
        .without_dead_code()
    }

    pub(crate) fn len(&self) -> usize {
        self.insts.len()
    }

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
                Inst::Load(Load::Const(c)) => T::cst(c.get()),
                Inst::Load(Load::Time) => t,
                Inst::Load(Load::State(k)) => x(k as usize),
                Inst::Load(Load::Param(k)) => p(k as usize),
                Inst::Op(op) => compute(op, |a| regs[a as usize]),
            };
            regs[i] = v;
        }
    }

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
        let outputs = self
            .outputs
            .iter()
            .map(|&o| new_index[o as usize])
            .collect();
        Program { insts, outputs }
    }
}

#[derive(Default)]
struct Builder {
    insts: Vec<Inst>,
    memo: HashMap<Inst, Reg>,
}

/// Integer powers up to this size become products (or `PowI`) rather than
/// a general `powf`.
const MAX_UNROLLED_INT_POW: f64 = 1024.0;

fn is_small_integer_power(c: f64) -> bool {
    c.fract() == 0.0 && c.abs() <= MAX_UNROLLED_INT_POW
}

impl Builder {
    fn constant(&self, r: Reg) -> Option<f64> {
        match self.insts[r as usize] {
            Inst::Load(Load::Const(c)) => Some(c.get()),
            _ => None,
        }
    }

    fn konst(&mut self, v: f64) -> Reg {
        self.memoised(Inst::Load(Load::Const(Bits::of(v))))
    }

    fn load(&mut self, l: Load) -> Reg {
        self.memoised(Inst::Load(l))
    }

    fn push(&mut self, op: Op) -> Reg {
        if op
            .operands()
            .iter()
            .flatten()
            .all(|&a| self.constant(a).is_some())
        {
            let v = compute::<f64>(op, |a| self.constant(a).expect("checked"));
            return self.konst(v);
        }
        self.memoised(Inst::Op(op))
    }

    fn memoised(&mut self, inst: Inst) -> Reg {
        let key = inst.normalised();
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
            Node::Num(v) => self.konst(*v),
            Node::Pi => self.konst(std::f64::consts::PI),
            Node::E => self.konst(std::f64::consts::E),
            Node::Time => self.load(Load::Time),
            Node::State(k) => self.load(Load::State(*k as u32)),
            Node::Param(k) => self.load(Load::Param(*k as u32)),
            Node::Neg(a) => {
                let a = self.lower(a);
                if let Inst::Op(Op::Neg(inner)) = self.insts[a as usize] {
                    return inner;
                }
                self.push(Op::Neg(a))
            }
            Node::Bin(BinOp::Pow, base, exp) => self.lower_pow(base, exp),
            Node::Bin(op, a, b) => {
                let a = self.lower(a);
                let b = self.lower(b);
                match op {
                    BinOp::Add if self.is_const(b, 0.0) => a,
                    BinOp::Add if self.is_const(a, 0.0) => b,
                    BinOp::Add => self.push(Op::Add(a, b)),
                    BinOp::Sub if self.is_const(b, 0.0) => a,
                    BinOp::Sub => self.push(Op::Sub(a, b)),
                    BinOp::Mul if self.is_const(b, 1.0) => a,
                    BinOp::Mul if self.is_const(a, 1.0) => b,
                    BinOp::Mul => self.push(Op::Mul(a, b)),
                    BinOp::Div if self.is_const(b, 1.0) => a,
                    BinOp::Div => self.push(Op::Div(a, b)),
                    BinOp::Pow => unreachable!("handled above"),
                }
            }
            Node::Call(f, args) => {
                let a = self.lower(&args[0]);
                if f.arity() == 2 {
                    let b = self.lower(&args[1]);
                    self.push(Op::Call2(*f, a, b))
                } else {
                    self.push(Op::Call1(*f, a))
                }
            }
        }
    }

    fn lower_pow(&mut self, base: &Node, exp: &Node) -> Reg {
        let e = self.lower(exp);
        if matches!(base, Node::E) {
            return self.push(Op::Call1(Func::Exp, e));
        }
        let b = self.lower(base);
        match self.constant(e) {
            Some(c) if is_small_integer_power(c) => self.powi(b, c as i32),
            Some(0.5) => self.push(Op::Call1(Func::Sqrt, b)),
            Some(c) => self.push(Op::PowC(b, Bits::of(c))),
            None => self.push(Op::Pow(b, e)),
        }
    }

    fn powi(&mut self, b: Reg, k: i32) -> Reg {
        match k {
            0 => self.konst(1.0),
            1 => b,
            2 => self.push(Op::Mul(b, b)),
            3 => {
                let sq = self.push(Op::Mul(b, b));
                self.push(Op::Mul(sq, b))
            }
            4 => {
                let sq = self.push(Op::Mul(b, b));
                self.push(Op::Mul(sq, sq))
            }
            -1 => {
                let one = self.konst(1.0);
                self.push(Op::Div(one, b))
            }
            _ => self.push(Op::PowI(b, k)),
        }
    }
}

pub(crate) trait Scratch: Scalar {
    fn with_regs<R>(len: usize, f: impl FnOnce(&mut [Self]) -> R) -> R;
}

/// Programs up to this many registers evaluate in a stack buffer; longer
/// ones use a per-thread buffer.
pub const SMALL_PROGRAM: usize = 32;

macro_rules! impl_scratch {
    ($t:ty, $cell:ident) => {
        thread_local! {
            static $cell: RefCell<Vec<$t>> = const { RefCell::new(Vec::new()) };
        }
        impl Scratch for $t {
            #[inline]
            fn with_regs<R>(len: usize, f: impl FnOnce(&mut [Self]) -> R) -> R {
                if len <= SMALL_PROGRAM {
                    let mut buf = [<$t as Scalar>::cst(0.0); SMALL_PROGRAM];
                    return f(&mut buf[..len]);
                }
                $cell.with(|cell| {
                    let mut buf = cell.borrow_mut();
                    if buf.len() < len {
                        buf.resize(len, <$t as Scalar>::cst(0.0));
                    }
                    f(&mut buf[..len])
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
        let s = Node::bin(BinOp::Add, st(0), st(1));
        let s2 = Node::bin(BinOp::Add, st(1), st(0));
        let a = Node::bin(BinOp::Pow, s.clone(), Node::Num(2.0));
        let b = Node::Call(Func::Sin, vec![s2]);
        let p = Program::compile(&[a, b]);
        assert_eq!(
            p.insts,
            vec![
                Inst::Load(Load::State(0)),
                Inst::Load(Load::State(1)),
                Inst::Op(Op::Add(0, 1)),
                Inst::Op(Op::Mul(2, 2)),
                Inst::Op(Op::Call1(Func::Sin, 2)),
            ]
        );
        let v = eval(&p, &[1.0, 2.0]);
        assert_eq!(v, vec![9.0, 3f64.sin()]);
    }

    #[test]
    fn folds_constants_and_drops_dead_code() {
        let c = Node::bin(BinOp::Div, Node::Num(8.0), Node::Num(3.0));
        let e = Node::bin(BinOp::Div, Node::Num(2.0), Node::Num(2.0));
        let n = Node::bin(
            BinOp::Add,
            Node::bin(BinOp::Mul, Node::bin(BinOp::Pow, st(0), e), c),
            Node::Num(0.0),
        );
        let p = Program::compile(&[n]);
        assert_eq!(
            p.insts,
            vec![
                Inst::Load(Load::State(0)),
                Inst::Load(Load::Const(Bits::of(8.0 / 3.0))),
                Inst::Op(Op::Mul(0, 1)),
            ]
        );
    }

    #[test]
    fn integer_powers_are_products() {
        let p = Program::compile(&[Node::bin(BinOp::Pow, st(0), Node::Num(3.0))]);
        assert!(p
            .insts
            .iter()
            .all(|i| !matches!(i, Inst::Op(Op::PowI(..) | Op::PowC(..)))));
        assert_eq!(eval(&p, &[-2.0]), vec![-8.0]);
        let p = Program::compile(&[Node::bin(BinOp::Pow, st(0), Node::Num(-3.0))]);
        assert_eq!(eval(&p, &[2.0]), vec![0.125]);
    }

    #[test]
    fn e_power_is_exp() {
        let p = Program::compile(&[Node::bin(BinOp::Pow, Node::E, st(0))]);
        assert_eq!(p.insts.last(), Some(&Inst::Op(Op::Call1(Func::Exp, 0))));
    }
}
