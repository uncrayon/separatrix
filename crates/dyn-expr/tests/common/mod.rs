#![allow(dead_code)]

use dyn_core::{fd_jacobian, fd_param_jacobian, VectorField};
use dyn_expr::{compile, CompiledSystem, Diagnostic, InputFormat, Severity};

pub fn ok(src: &str, f: InputFormat) -> CompiledSystem {
    let out = compile(src, f);
    let errors: Vec<&Diagnostic> = out
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{src:?} failed: {errors:#?}");
    out.system.expect("system present when there are no errors")
}

pub fn latex(src: &str) -> CompiledSystem {
    ok(src, InputFormat::Latex)
}

pub fn text(src: &str) -> CompiledSystem {
    ok(src, InputFormat::Text)
}

pub fn fails(src: &str, f: InputFormat) -> Vec<Diagnostic> {
    let out = compile(src, f);
    assert!(out.system.is_none(), "{src:?} should fail");
    assert!(out
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error));
    out.diagnostics
}

pub fn names(sys: &CompiledSystem) -> (Vec<String>, Vec<String>) {
    (
        sys.info.states.iter().map(|s| s.name.clone()).collect(),
        sys.info.params.iter().map(|p| p.name.clone()).collect(),
    )
}

pub fn assert_close(a: &[f64], b: &[f64], tol: f64) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!(
            (x - y).abs() <= tol * (1.0 + y.abs()),
            "entry {i}: {x} vs {y}\n{a:?}\n{b:?}"
        );
    }
}

pub fn jac(sys: &CompiledSystem, t: f64, x: &[f64], p: &[f64]) -> Vec<f64> {
    let mut j = vec![0.0; sys.dim() * sys.dim()];
    sys.jacobian(t, x, p, &mut j);
    j
}

pub fn check_derivatives(sys: &CompiledSystem, t: f64, x: &[f64], p: &[f64], tol: f64) {
    let n = sys.dim();
    let mut fd = vec![0.0; n * n];
    fd_jacobian(sys, t, x, p, &mut fd);
    assert_close(&jac(sys, t, x, p), &fd, tol);
    for k in 0..sys.n_params() {
        let mut ad = vec![0.0; n];
        let mut fdp = vec![0.0; n];
        sys.param_jacobian(t, x, p, k, &mut ad);
        fd_param_jacobian(sys, t, x, p, k, &mut fdp);
        assert_close(&ad, &fdp, tol);
    }
}

/// xorshift64* generator.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A random expression in `x`, `y` and the parameter `a`, defined for every
/// real input.
#[derive(Debug, Clone)]
pub enum E {
    X,
    Y,
    A,
    Num(f64),
    Add(Box<E>, Box<E>),
    Sub(Box<E>, Box<E>),
    Mul(Box<E>, Box<E>),
    SafeDiv(Box<E>, Box<E>),
    PowI(Box<E>, i32),
    Neg(Box<E>),
    Sin(Box<E>),
    Cos(Box<E>),
    Tanh(Box<E>),
    Atan(Box<E>),
    ExpSin(Box<E>),
    LogSq(Box<E>),
    SqrtSq(Box<E>),
}

pub fn gen(r: &mut Rng, depth: u32) -> E {
    let leaf = depth == 0 || r.below(4) == 0;
    if leaf {
        return match r.below(5) {
            0 | 1 => E::X,
            2 => E::Y,
            3 => E::A,
            _ => E::Num((r.uniform(0.1, 3.0) * 4.0).round() / 4.0),
        };
    }
    let mut sub = || Box::new(gen(r, depth - 1));
    let (a, b) = (sub(), sub());
    match r.below(15) {
        0 | 1 => E::Add(a, b),
        2 => E::Sub(a, b),
        3 | 4 => E::Mul(a, b),
        5 => E::SafeDiv(a, b),
        6 => E::PowI(a, 2 + (r.below(3) as i32)),
        7 => E::Neg(a),
        8 => E::Sin(a),
        9 => E::Cos(a),
        10 => E::Tanh(a),
        11 => E::Atan(a),
        12 => E::ExpSin(a),
        13 => E::LogSq(a),
        _ => E::SqrtSq(a),
    }
}

pub fn eval(e: &E, x: f64, y: f64, a: f64) -> f64 {
    let ev = |e: &E| eval(e, x, y, a);
    match e {
        E::X => x,
        E::Y => y,
        E::A => a,
        E::Num(v) => *v,
        E::Add(p, q) => ev(p) + ev(q),
        E::Sub(p, q) => ev(p) - ev(q),
        E::Mul(p, q) => ev(p) * ev(q),
        E::SafeDiv(p, q) => ev(p) / (1.0 + ev(q).powi(2)),
        E::PowI(p, k) => ev(p).powi(*k),
        E::Neg(p) => -ev(p),
        E::Sin(p) => ev(p).sin(),
        E::Cos(p) => ev(p).cos(),
        E::Tanh(p) => ev(p).tanh(),
        E::Atan(p) => ev(p).atan(),
        E::ExpSin(p) => ev(p).sin().exp(),
        E::LogSq(p) => (1.0 + ev(p).powi(2)).ln(),
        E::SqrtSq(p) => (1.0 + ev(p).powi(2)).sqrt(),
    }
}

pub fn to_text(e: &E) -> String {
    match e {
        E::X => "x".into(),
        E::Y => "y".into(),
        E::A => "a".into(),
        E::Num(v) => format!("{v}"),
        E::Add(p, q) => format!("({} + {})", to_text(p), to_text(q)),
        E::Sub(p, q) => format!("({} - {})", to_text(p), to_text(q)),
        E::Mul(p, q) => format!("({} * {})", to_text(p), to_text(q)),
        E::SafeDiv(p, q) => format!("({} / (1 + ({})^2))", to_text(p), to_text(q)),
        E::PowI(p, k) => format!("({})^{k}", to_text(p)),
        E::Neg(p) => format!("(-{})", to_text(p)),
        E::Sin(p) => format!("sin({})", to_text(p)),
        E::Cos(p) => format!("cos({})", to_text(p)),
        E::Tanh(p) => format!("tanh({})", to_text(p)),
        E::Atan(p) => format!("arctan({})", to_text(p)),
        E::ExpSin(p) => format!("exp(sin({}))", to_text(p)),
        E::LogSq(p) => format!("ln(1 + ({})**2)", to_text(p)),
        E::SqrtSq(p) => format!("sqrt(1 + ({})^2)", to_text(p)),
    }
}

pub fn to_latex(e: &E) -> String {
    let g = |s: String| format!("\\left({s}\\right)");
    match e {
        E::X => "x".into(),
        E::Y => "y".into(),
        E::A => "a".into(),
        E::Num(v) => format!("{v}"),
        E::Add(p, q) => g(format!("{}+{}", to_latex(p), to_latex(q))),
        E::Sub(p, q) => g(format!("{}-{}", to_latex(p), to_latex(q))),
        E::Mul(p, q) => g(format!("{}{}", g(to_latex(p)), g(to_latex(q)))),
        E::SafeDiv(p, q) => format!("\\frac{{{}}}{{1+{}^{{2}}}}", to_latex(p), g(to_latex(q))),
        E::PowI(p, k) => format!("{}^{{{k}}}", g(to_latex(p))),
        E::Neg(p) => g(format!("-{}", to_latex(p))),
        E::Sin(p) => format!("\\sin{}", g(to_latex(p))),
        E::Cos(p) => format!("\\cos{}", g(to_latex(p))),
        E::Tanh(p) => format!("\\tanh{}", g(to_latex(p))),
        E::Atan(p) => format!("\\arctan{}", g(to_latex(p))),
        E::ExpSin(p) => format!("e^{{\\sin{}}}", g(to_latex(p))),
        E::LogSq(p) => format!("\\ln{}", g(format!("1+{}^2", g(to_latex(p))))),
        E::SqrtSq(p) => format!("\\sqrt{{1+{}^{{2}}}}", g(to_latex(p))),
    }
}
