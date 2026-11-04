//! Randomized tests: generate expressions, print them in both input
//! formats, and check that
//! - LaTeX and text compile to the same values,
//! - the values match a direct Rust evaluation of the generated tree,
//! - AD Jacobians (state and parameter) match central differences,
//! - Hamiltonian fields match finite differences of `H`.
//!
//! A tiny deterministic PRNG keeps the test reproducible without extra
//! dependencies.

mod common;

use common::*;
use dyn_core::VectorField;
use dyn_expr::CompiledSystem;

/// xorshift64* generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Generated expression over states `x`, `y` and parameter `a`.
/// Every node is smooth on the sampled domain (denominators and log
/// arguments are kept ≥ 1).
#[derive(Debug, Clone)]
enum E {
    X,
    Y,
    A,
    Num(f64),
    Add(Box<E>, Box<E>),
    Sub(Box<E>, Box<E>),
    Mul(Box<E>, Box<E>),
    /// a / (1 + b^2)
    SafeDiv(Box<E>, Box<E>),
    /// a^k, k in 2..=4
    PowI(Box<E>, i32),
    Neg(Box<E>),
    Sin(Box<E>),
    Cos(Box<E>),
    Tanh(Box<E>),
    Atan(Box<E>),
    /// exp(sin a)
    ExpSin(Box<E>),
    /// ln(1 + a^2)
    LogSq(Box<E>),
    /// sqrt(1 + a^2)
    SqrtSq(Box<E>),
}

fn gen(r: &mut Rng, depth: u32) -> E {
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

fn eval(e: &E, x: f64, y: f64, a: f64) -> f64 {
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

/// Fully bracketed text-mode form.
fn to_text(e: &E) -> String {
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

/// MathLive-style LaTeX, using implicit multiplication, `\frac`,
/// `\left( \right)`, `e^{…}` and `\sqrt{…}`.
fn to_latex(e: &E) -> String {
    let g = |s: String| format!("\\left({s}\\right)");
    match e {
        E::X => "x".into(),
        E::Y => "y".into(),
        E::A => "a".into(),
        E::Num(v) => format!("{v}"),
        E::Add(p, q) => g(format!("{}+{}", to_latex(p), to_latex(q))),
        E::Sub(p, q) => g(format!("{}-{}", to_latex(p), to_latex(q))),
        // Juxtaposition of two bracketed groups is a product.
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

/// Derivative of `f` at `x0` by central differences with Richardson
/// extrapolation, plus an error estimate. Plain central differences are not
/// accurate enough for the wilder generated expressions (e.g. sin(a^12)).
fn richardson(f: impl Fn(f64) -> Vec<f64>, x0: f64) -> (Vec<f64>, Vec<f64>) {
    let scale = x0.abs().max(1.0);
    let d = |h: f64| -> Vec<f64> {
        let (a, b) = (f(x0 + h), f(x0 - h));
        a.iter().zip(&b).map(|(a, b)| (a - b) / (2.0 * h)).collect()
    };
    // Try a few base steps and keep, per entry, the estimate with the
    // smallest error bound.
    let mut best: Option<(Vec<f64>, Vec<f64>)> = None;
    for h in [1e-2, 1e-3, 1e-4].map(|h| h * scale) {
        let (d1, d2, d3) = (d(h), d(h / 2.0), d(h / 4.0));
        let mut est = Vec::new();
        let mut err = Vec::new();
        for i in 0..d1.len() {
            let r1 = (4.0 * d2[i] - d1[i]) / 3.0;
            let r2 = (4.0 * d3[i] - d2[i]) / 3.0;
            let r = (16.0 * r2 - r1) / 15.0;
            est.push(r);
            err.push((r - r2).abs());
        }
        best = Some(match best {
            None => (est, err),
            Some((mut be, mut br)) => {
                for i in 0..est.len() {
                    if err[i] < br[i] {
                        be[i] = est[i];
                        br[i] = err[i];
                    }
                }
                (be, br)
            }
        });
    }
    best.expect("at least one step")
}

fn assert_matches_fd(ad: &[f64], (est, err): (Vec<f64>, Vec<f64>), what: &str) {
    for i in 0..ad.len() {
        let tol = 10.0 * err[i] + 1e-7 * (1.0 + est[i].abs());
        assert!(
            (ad[i] - est[i]).abs() <= tol,
            "{what}[{i}]: AD {} vs FD {} (±{})",
            ad[i],
            est[i],
            err[i]
        );
    }
}

/// Exact Jacobian columns and parameter derivatives vs extrapolated FD.
fn check_ad(sys: &CompiledSystem, x: &[f64], p: &[f64]) {
    let n = sys.dim();
    let j = jac(sys, 0.0, x, p);
    for c in 0..n {
        let fd = richardson(
            |v| {
                let mut xx = x.to_vec();
                xx[c] = v;
                sys.eval_alloc(0.0, &xx, p)
            },
            x[c],
        );
        let col: Vec<f64> = (0..n).map(|r| j[r * n + c]).collect();
        assert_matches_fd(&col, fd, &format!("column {c}"));
    }
    for k in 0..sys.n_params() {
        let mut ad = vec![0.0; n];
        sys.param_jacobian(0.0, x, p, k, &mut ad);
        let fd = richardson(
            |v| {
                let mut pp = p.to_vec();
                pp[k] = v;
                sys.eval_alloc(0.0, x, &pp)
            },
            p[k],
        );
        assert_matches_fd(&ad, fd, &format!("param {k}"));
    }
}

#[test]
fn random_expressions_ad_matches_finite_differences() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut checked = 0;
    for case in 0..300 {
        let f = gen(&mut r, 4);
        let g = gen(&mut r, 4);
        // Always mention x, y and a so the system has dim 2 and 1 parameter.
        let txt = format!("x' = {} + 0*y*a\ny' = {} + 0*x", to_text(&f), to_text(&g));
        let ltx = format!(
            "\\dot{{x}}={}+0ya\\\\\\dot{{y}}={}+0x",
            to_latex(&f),
            to_latex(&g)
        );
        let st = text(&txt);
        let sl = latex(&ltx);
        assert_eq!(names(&st), names(&sl), "case {case}");
        assert_eq!(names(&st).1, ["a"], "case {case}");
        for _ in 0..3 {
            let x = [r.uniform(-1.5, 1.5), r.uniform(-1.5, 1.5)];
            let p = [r.uniform(0.5, 1.5)];
            let want = [eval(&f, x[0], x[1], p[0]), eval(&g, x[0], x[1], p[0])];
            let vt = st.eval_alloc(0.0, &x, &p);
            let vl = sl.eval_alloc(0.0, &x, &p);
            assert_close(&vt, &want, 1e-12);
            assert_close(&vl, &want, 1e-12);
            check_ad(&st, &x, &p);
            assert_close(&jac(&sl, 0.0, &x, &p), &jac(&st, 0.0, &x, &p), 1e-12);
            checked += 1;
        }
    }
    assert_eq!(checked, 900);
}

#[test]
fn random_hamiltonians_match_finite_differences() {
    let mut r = Rng(0xD1B5_4A32_D192_ED03);
    for case in 0..100 {
        // H(x, y) with x = q, y = p.
        let h = gen(&mut r, 3);
        let src = format!("H(x, y) = {} + 0*x*y*a", to_text(&h));
        let sys = text(&src);
        for _ in 0..3 {
            let x = [r.uniform(-1.2, 1.2), r.uniform(-1.2, 1.2)];
            let p = [r.uniform(0.5, 1.5)];
            let hv = |q: f64, m: f64| eval(&h, q, m, p[0]);
            assert_close(
                &[sys.hamiltonian_value(0.0, &x, &p).unwrap()],
                &[hv(x[0], x[1])],
                1e-12,
            );
            // Field = (∂H/∂p, -∂H/∂q).
            let f = sys.eval_alloc(0.0, &x, &p);
            assert_matches_fd(&[f[0]], richardson(|m| vec![hv(x[0], m)], x[1]), "dH/dp");
            assert_matches_fd(&[-f[1]], richardson(|q| vec![hv(q, x[1])], x[0]), "dH/dq");
            check_ad(&sys, &x, &p);
            // Hamiltonian flows are divergence-free: trace(J) = 0.
            let j = jac(&sys, 0.0, &x, &p);
            assert!(
                (j[0] + j[3]).abs() < 1e-9 * (1.0 + j[0].abs()),
                "case {case}: {j:?}"
            );
        }
    }
}
