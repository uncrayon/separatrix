//! Semantic pass: from parsed lines to a resolved first-order system.
//!
//! 1. **Classify** each line by its left-hand side: `\dot{x}` / `x'` /
//!    `\frac{dx}{dt}` (flow), `x_{n+1}` (map), `H(q, p)` (Hamiltonian) or a
//!    plain symbol (parameter default). Leibniz derivatives anywhere in the
//!    line are first rewritten as `\dot{x}`-style derivatives. Lines such as
//!    `x'' + x = 0` or `x = -\ddot{x}` are solved for their highest
//!    derivative when it appears linearly.
//! 2. **States** are collected in equation order; an equation of order `k`
//!    for `x` adds `x, x', …, x^(k-1)`.
//! 3. **Resolve** right-hand sides: every name becomes a state, time, a
//!    constant or a parameter (in order of first appearance).
//! 4. Build [`SystemInfo`], the display LaTeX and the expressions to compile.

use std::collections::HashMap;

use dyn_core::SystemKind;

use crate::ast::{Ast, AstKind, BinOp, Func, Node, FUNCTION_WORDS};
use crate::diag::Diagnostic;
use crate::latex::{to_latex, Names};
use crate::parser::Line;
use crate::program::call1;
use crate::token::{is_greek, sym_from_word, Span, SymName};
use crate::{InputFormat, ParamInfo, SymbolInfo, SystemInfo};

/// Result of a successful analysis.
pub(crate) struct Analysis {
    pub info: SystemInfo,
    /// Direct systems: one expression per state (the field). Hamiltonian
    /// systems: a single expression, `H`.
    pub roots: Vec<Node>,
    /// `Some(k)` for a Hamiltonian with `k` degrees of freedom.
    pub hamiltonian_dof: Option<usize>,
}

/// What a line defines.
enum Target {
    /// `k`-th derivative of a symbol.
    Ode(SymName, u32),
    /// Next iterate of a symbol.
    Map(SymName),
    /// Hamiltonian with coordinates then momenta.
    Ham(Vec<SymName>),
    /// Default value of a parameter.
    Default(SymName),
}

struct Eqn {
    target: Target,
    rhs: Ast,
    lhs_span: Span,
}

fn error(msg: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error(msg, Some(span))
}

/// How to show a name in a message, in the user's own notation.
fn show(s: &SymName, text: bool) -> String {
    if text {
        s.name.clone()
    } else {
        s.latex()
    }
}

fn show_deriv(s: &SymName, k: u32, text: bool) -> String {
    if text {
        s.derived_name(k)
    } else {
        s.dotted(k)
    }
}

fn examples(text: bool) -> &'static str {
    if text {
        "x' = …, x'' = …, x_{n+1} = … or a parameter value like a = 2"
    } else {
        "\\dot{x} = …, \\ddot{x} = …, x_{n+1} = … or a parameter value like a = 2"
    }
}

// ----- classification ----------------------------------------------------

fn classify(line: Line, text: bool) -> Result<Eqn, Diagnostic> {
    let Line { lhs, rhs, span } = line;
    let lhs = leibniz_to_deriv(lhs);
    let rhs = leibniz_to_deriv(rhs);
    let lhs_span = lhs.span;
    let target = match &lhs.kind {
        AstKind::Deriv(s, k) => Target::Ode(s.clone(), *k),
        AstKind::Next(s) => Target::Map(s.clone()),
        AstKind::Apply(s, args) if s.name == "H" => {
            Target::Ham(hamiltonian_args(args, lhs_span, text)?)
        }
        // `x = -\ddot{x}` is an equation for x'', not a value for x.
        AstKind::Sym(_) if has_deriv(&rhs) => return rearrange(&lhs, &rhs, span, text),
        AstKind::Sym(s) => Target::Default(s.clone()),
        _ if has_deriv(&lhs) || has_deriv(&rhs) => return rearrange(&lhs, &rhs, span, text),
        _ => {
            let msg = other_variable_message(&lhs, text).unwrap_or_else(|| {
                format!("Can't tell what this line defines. Start it with {}", examples(text))
            });
            return Err(error(msg, lhs_span));
        }
    };
    Ok(Eqn { target, rhs, lhs_span })
}

fn hamiltonian_args(args: &[Ast], span: Span, text: bool) -> Result<Vec<SymName>, Diagnostic> {
    let mut out: Vec<SymName> = Vec::new();
    for a in args {
        match &a.kind {
            AstKind::Sym(s) => {
                if out.iter().any(|o| o.name == s.name) {
                    return Err(error(format!("{} is listed twice in H(…)", show(s, text)), a.span));
                }
                out.push(s.clone());
            }
            _ => {
                return Err(error(
                    "H(…) must list plain variable names, like H(q, p) or H(q_1, q_2, p_1, p_2)",
                    a.span,
                ))
            }
        }
    }
    if out.len() % 2 == 1 {
        return Err(error(
            "H needs as many momenta as coordinates: coordinates first, then momenta, \
             like H(q, p) or H(q_1, q_2, p_1, p_2)",
            span,
        ));
    }
    Ok(out)
}

fn is_sym(a: &Ast, name: &str) -> bool {
    matches!(&a.kind, AstKind::Sym(s) if s.name == name)
}

fn whole_number(a: &Ast) -> Option<u32> {
    match a.kind {
        AstKind::Num(k) if k >= 1.0 && k.fract() == 0.0 && k <= 16.0 => Some(k as u32),
        _ => None,
    }
}

/// `d` → 1, `d^k` → k.
fn d_power(a: &Ast) -> Option<u32> {
    match &a.kind {
        AstKind::Sym(s) if s.name == "d" => Some(1),
        AstKind::Bin(BinOp::Pow, b, e) if is_sym(b, "d") => whole_number(e),
        _ => None,
    }
}

/// Denominator of a Leibniz derivative: `dt` → (1, t), `dt^k` → (k, t),
/// and likewise for any other variable (`dx` → (1, x)). LaTeX writes
/// `d t` (two symbols), text mode the word `dt`.
fn d_var_power(a: &Ast) -> Option<(u32, SymName)> {
    // Text-mode word `dt` → t.
    let word = |s: &SymName| {
        (s.name.len() > 1 && s.name.starts_with('d') && s.sub_latex.is_none())
            .then(|| sym_from_word(&s.name[1..]))
    };
    match &a.kind {
        AstKind::Sym(s) => Some((1, word(s)?)),
        AstKind::Bin(BinOp::Pow, b, e) => match &b.kind {
            AstKind::Sym(s) => Some((whole_number(e)?, word(s)?)),
            _ => None,
        },
        AstKind::Bin(BinOp::Mul, d, v) if d_power(d) == Some(1) => match &v.kind {
            AstKind::Sym(s) => Some((1, s.clone())),
            AstKind::Bin(BinOp::Pow, b, e) => match &b.kind {
                AstKind::Sym(s) => Some((whole_number(e)?, s.clone())),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// A Leibniz derivative `\frac{dx}{dv}`, `\frac{d^2x}{dv^2}`,
/// `\frac{d}{dv}x` or text `dx/dv`: returns `(x, order, v)`.
fn leibniz_wrt(a: &Ast) -> Option<(SymName, u32, SymName)> {
    match &a.kind {
        AstKind::Bin(BinOp::Div, num, den) => {
            let (k, var) = d_var_power(den)?;
            match &num.kind {
                AstKind::Bin(BinOp::Mul, d, x) => match &x.kind {
                    AstKind::Sym(s) if d_power(d) == Some(k) => Some((s.clone(), k, var)),
                    _ => None,
                },
                AstKind::Sym(s) if k == 1 && s.name.len() > 1 && s.name.starts_with('d') => {
                    Some((sym_from_word(&s.name[1..]), 1, var))
                }
                _ => None,
            }
        }
        AstKind::Bin(BinOp::Mul, op, x) => {
            let AstKind::Bin(BinOp::Div, d, den) = &op.kind else { return None };
            let (k, var) = d_var_power(den)?;
            match &x.kind {
                AstKind::Sym(s) if d_power(d) == Some(k) => Some((s.clone(), k, var)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Leibniz notation for a time derivative: `\frac{dx}{dt}`,
/// `\frac{d^2x}{dt^2}`, `\frac{d}{dt}x`, text `dx/dt`.
fn leibniz(a: &Ast) -> Option<(SymName, u32)> {
    let (s, k, var) = leibniz_wrt(a)?;
    (var.name == "t").then_some((s, k))
}

/// Replace every Leibniz time derivative in the tree by a
/// [`AstKind::Deriv`] node, so `\frac{d^2x}{dt^2} + x = 0` is handled
/// like `\ddot{x} + x = 0`.
fn leibniz_to_deriv(a: Ast) -> Ast {
    if let Some((s, k)) = leibniz(&a) {
        return Ast::new(AstKind::Deriv(s, k), a.span);
    }
    let kind = match a.kind {
        AstKind::Neg(x) => AstKind::Neg(Box::new(leibniz_to_deriv(*x))),
        AstKind::Bin(op, x, y) => {
            AstKind::Bin(op, Box::new(leibniz_to_deriv(*x)), Box::new(leibniz_to_deriv(*y)))
        }
        AstKind::Call(f, args) => AstKind::Call(f, args.into_iter().map(leibniz_to_deriv).collect()),
        AstKind::Apply(s, args) => {
            AstKind::Apply(s, args.into_iter().map(leibniz_to_deriv).collect())
        }
        leaf => leaf,
    };
    Ast::new(kind, a.span)
}

/// For a line like `\frac{dy}{dx} = x - y`: explain that derivatives are
/// with respect to time, and how to get a slope field of dy/dx.
fn other_variable_message(lhs: &Ast, text: bool) -> Option<String> {
    let mut found = None;
    lhs.walk(&mut |n| {
        if found.is_none() {
            found = leibniz_wrt(n).filter(|(_, _, v)| v.name != "t");
        }
    });
    let (y, k, x) = found?;
    let (yd, xd, yt) = if text {
        (show_deriv(&y, 1, true), show_deriv(&x, 1, true), format!("d{}/dt", y.name))
    } else {
        (show_deriv(&y, 1, false), show_deriv(&x, 1, false), format!("\\frac{{d{}}}{{dt}}", y.latex()))
    };
    let (yn, xn) = (show(&y, text), show(&x, text));
    let mut msg = format!(
        "Derivatives are taken with respect to the time t, not {xn}: write {yt} (or {yd}) = …"
    );
    if k == 1 {
        msg.push_str(&format!(
            ". For the slope field of d{yn}/d{xn} = f({xn}, {yn}), enter {xd} = 1 and \
             {yd} = f({xn}, {yn})"
        ));
    }
    Some(msg)
}

fn has_deriv(a: &Ast) -> bool {
    let mut found = false;
    a.walk(&mut |n| found |= matches!(n.kind, AstKind::Deriv(..)));
    found
}

fn contains_target(a: &Ast, name: &str, k: u32) -> bool {
    let mut found = false;
    a.walk(&mut |n| {
        found |= matches!(&n.kind, AstKind::Deriv(s, j) if s.name == name && *j == k);
    });
    found
}

/// Split a sum into signed terms.
fn flatten(a: &Ast, sign: f64, out: &mut Vec<(f64, Ast)>) {
    match &a.kind {
        AstKind::Bin(BinOp::Add, x, y) => {
            flatten(x, sign, out);
            flatten(y, sign, out);
        }
        AstKind::Bin(BinOp::Sub, x, y) => {
            flatten(x, sign, out);
            flatten(y, -sign, out);
        }
        AstKind::Neg(x) => flatten(x, -sign, out),
        _ => out.push((sign, a.clone())),
    }
}

fn mul_opt(a: Ast, b: Option<Ast>) -> Ast {
    match b {
        None => a,
        Some(b) => Ast::bin(BinOp::Mul, a, b),
    }
}

/// Write `term` as `sign · num · D / den` where `D` is the target
/// derivative and `num`, `den` don't contain it (`None` means 1).
fn extract(a: &Ast, name: &str, k: u32) -> Option<(f64, Option<Ast>, Option<Ast>)> {
    match &a.kind {
        AstKind::Deriv(s, j) if s.name == name && *j == k => Some((1.0, None, None)),
        AstKind::Neg(x) => extract(x, name, k).map(|(s, n, d)| (-s, n, d)),
        AstKind::Bin(BinOp::Mul, x, y) => {
            let (inner, other) = match (contains_target(x, name, k), contains_target(y, name, k)) {
                (true, false) => (x, y),
                (false, true) => (y, x),
                _ => return None,
            };
            let (s, n, d) = extract(inner, name, k)?;
            Some((s, Some(mul_opt((**other).clone(), n)), d))
        }
        AstKind::Bin(BinOp::Div, x, y)
            if contains_target(x, name, k) && !contains_target(y, name, k) =>
        {
            let (s, n, d) = extract(x, name, k)?;
            Some((s, n, Some(mul_opt((**y).clone(), d))))
        }
        _ => None,
    }
}

/// Solve `lhs = rhs` for its highest derivative, e.g.
/// `x'' + c x' + x = 0` → `x'' = -c x' - x`.
fn rearrange(lhs: &Ast, rhs: &Ast, span: Span, text: bool) -> Result<Eqn, Diagnostic> {
    let mut derivs: Vec<(SymName, u32)> = Vec::new();
    let mut collect = |n: &Ast| {
        if let AstKind::Deriv(s, k) = &n.kind {
            derivs.push((s.clone(), *k));
        }
    };
    lhs.walk(&mut collect);
    rhs.walk(&mut collect);
    let top = derivs.iter().map(|d| d.1).max().unwrap_or(1);
    let mut tops: Vec<&SymName> = Vec::new();
    for (s, k) in &derivs {
        if *k == top && !tops.iter().any(|t| t.name == s.name) {
            tops.push(s);
        }
    }
    if tops.len() > 1 {
        return Err(error(
            format!(
                "Several highest derivatives ({}) appear in one line; write one equation per \
                 variable, each starting with its derivative",
                tops.iter().map(|s| show_deriv(s, top, text)).collect::<Vec<_>>().join(", ")
            ),
            span,
        ));
    }
    let target = tops[0].clone();
    let dname = show_deriv(&target, top, text);
    let mut terms = Vec::new();
    flatten(lhs, 1.0, &mut terms);
    flatten(rhs, -1.0, &mut terms);
    let (with, without): (Vec<_>, Vec<_>) =
        terms.into_iter().partition(|(_, t)| contains_target(t, &target.name, top));
    if with.len() != 1 {
        return Err(error(
            format!(
                "{dname} appears in more than one term; combine them or write the line as \
                 {dname} = …"
            ),
            span,
        ));
    }
    let (s0, term) = &with[0];
    let Some((s1, num, den)) = extract(term, &target.name, top) else {
        return Err(error(format!("Can't solve this line for {dname}; write it as {dname} = …"), span));
    };
    let sigma = s0 * s1;
    // 0 = σ·num·D/den + Σ sᵢ tᵢ  ⇒  D = den · Σ(−σ sᵢ tᵢ) / num   (σ = ±1).
    let mut sum: Option<Ast> = None;
    for (s, t) in without {
        if matches!(t.kind, AstKind::Num(v) if v == 0.0) {
            continue;
        }
        let positive = -sigma * s > 0.0;
        sum = Some(match sum {
            None if positive => t,
            None => {
                let sp = t.span;
                Ast::new(AstKind::Neg(Box::new(t)), sp)
            }
            Some(acc) => Ast::bin(if positive { BinOp::Add } else { BinOp::Sub }, acc, t),
        });
    }
    let mut out = sum.unwrap_or_else(|| Ast::new(AstKind::Num(0.0), span));
    if let Some(d) = den {
        out = Ast::bin(BinOp::Mul, d, out);
    }
    if let Some(n) = num {
        out = Ast::bin(BinOp::Div, out, n);
    }
    Ok(Eqn { target: Target::Ode(target, top), rhs: out, lhs_span: span })
}

// ----- states ------------------------------------------------------------

/// The state variables, in order.
#[derive(Default)]
struct StateTable {
    /// Base symbol of each state.
    syms: Vec<SymName>,
    /// Derivative order of each state (`x'` has order 1).
    orders: Vec<u32>,
    /// Base name → indices of `x, x', x'', …`.
    by_name: HashMap<String, Vec<usize>>,
}

/// Names that can't be states or parameter defaults.
fn reserved_message(name: &str, kind: SystemKind) -> Option<&'static str> {
    match name {
        "t" if kind == SystemKind::Ode => {
            Some("t is the time variable, so it can't have its own equation or value")
        }
        "n" if kind == SystemKind::Map => {
            Some("n is the iteration counter, so it can't have its own equation or value")
        }
        "e" => Some("e is the constant 2.718…; choose another name"),
        "pi" => Some("π is the constant 3.14…; choose another name"),
        _ => None,
    }
}

impl StateTable {
    fn add(
        &mut self,
        s: &SymName,
        count: u32,
        kind: SystemKind,
        span: Span,
        text: bool,
        diags: &mut Vec<Diagnostic>,
    ) -> bool {
        if let Some(msg) = reserved_message(&s.name, kind) {
            diags.push(error(msg, span));
            return false;
        }
        if self.by_name.contains_key(&s.name) {
            diags.push(error(
                format!("{} has more than one equation; keep only one", show(s, text)),
                span,
            ));
            return false;
        }
        let idx = (0..count)
            .map(|j| {
                self.syms.push(s.clone());
                self.orders.push(j);
                self.syms.len() - 1
            })
            .collect();
        self.by_name.insert(s.name.clone(), idx);
        true
    }

    fn len(&self) -> usize {
        self.syms.len()
    }

    fn info(&self, i: usize) -> SymbolInfo {
        SymbolInfo {
            name: self.syms[i].derived_name(self.orders[i]),
            latex: self.syms[i].dotted(self.orders[i]),
        }
    }
}

// ----- constant expressions ---------------------------------------------

enum ConstErr {
    /// Uses a name that isn't a constant.
    Symbol(String),
    Other(String, Span),
}

fn eval_func(f: Func, v: &[f64]) -> f64 {
    match f {
        Func::Min => v[0].min(v[1]),
        Func::Max => v[0].max(v[1]),
        _ => call1(f, v[0]),
    }
}

/// Value of an expression made of numbers, `\pi`, `e` and functions.
fn const_eval(a: &Ast) -> Result<f64, ConstErr> {
    Ok(match &a.kind {
        AstKind::Num(v) => *v,
        AstKind::Sym(s) if s.name == "pi" => std::f64::consts::PI,
        AstKind::Sym(s) if s.name == "e" => std::f64::consts::E,
        AstKind::Sym(s) | AstKind::Deriv(s, _) | AstKind::Next(s) | AstKind::Apply(s, _) => {
            return Err(ConstErr::Symbol(s.name.clone()))
        }
        AstKind::Neg(x) => -const_eval(x)?,
        AstKind::Bin(op, x, y) => {
            let l = const_eval(x)?;
            let r = const_eval(y)?;
            match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div if r == 0.0 => {
                    return Err(ConstErr::Other("Division by zero".into(), y.span))
                }
                BinOp::Div => l / r,
                BinOp::Pow => l.powf(r),
            }
        }
        AstKind::Call(f, args) => {
            let v = args.iter().map(const_eval).collect::<Result<Vec<_>, _>>()?;
            eval_func(*f, &v)
        }
    })
}

// ----- name resolution ---------------------------------------------------

/// `sinx` → "did you mean sin(x)?" for multi-letter names.
fn function_typo(name: &str, text: bool, table: &StateTable) -> Option<String> {
    for f in FUNCTION_WORDS {
        if name.len() > f.len() && name.starts_with(f) {
            let rest = &name[f.len()..];
            let plausible =
                rest.len() <= 2 || is_greek(rest) || table.by_name.contains_key(rest);
            if plausible && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let fix = if text { format!("{f}({rest})") } else { format!("\\{f} {rest}") };
                return Some(format!("{name} is not a function; did you mean {fix}?"));
            }
        }
    }
    None
}

struct Resolver<'a> {
    kind: SystemKind,
    text: bool,
    table: &'a StateTable,
    /// Parameters in order of first appearance, with that first span.
    params: Vec<(SymName, Span)>,
    index: HashMap<String, usize>,
    diags: &'a mut Vec<Diagnostic>,
}

impl Resolver<'_> {
    fn fail(&mut self, msg: impl Into<String>, span: Span) -> Node {
        self.diags.push(error(msg, span));
        Node::Num(f64::NAN)
    }

    fn resolve(&mut self, a: &Ast) -> Node {
        match &a.kind {
            AstKind::Num(v) => Node::Num(*v),
            AstKind::Sym(s) => self.symbol(s, a.span),
            AstKind::Deriv(s, k) => self.derivative(s, *k, a.span),
            AstKind::Next(s) => self.fail(
                format!("{} can only appear on the left of =", s.map_latex("n+1")),
                a.span,
            ),
            AstKind::Neg(x) => Node::Neg(Box::new(self.resolve(x))),
            AstKind::Bin(op, x, y) => {
                // A denominator that is always 0, literally (`/0`) or after
                // folding constants (`\frac{x}{1-1}`).
                if *op == BinOp::Div && matches!(const_eval(y), Ok(v) if v == 0.0) {
                    return self.fail("Division by zero", y.span);
                }
                let l = self.resolve(x);
                let r = self.resolve(y);
                Node::bin(*op, l, r)
            }
            AstKind::Call(f, args) => {
                let args = args.iter().map(|x| self.resolve(x)).collect();
                Node::Call(*f, args)
            }
            AstKind::Apply(s, args) => self.apply(s, args, a.span),
        }
    }

    fn symbol(&mut self, s: &SymName, span: Span) -> Node {
        if let Some(idx) = self.table.by_name.get(&s.name) {
            return Node::State(idx[0]);
        }
        if self.kind == SystemKind::Map {
            // x_n is the current iterate of x.
            if let Some(idx) = s.name.strip_suffix("_n").and_then(|b| self.table.by_name.get(b)) {
                return Node::State(idx[0]);
            }
        }
        match (s.name.as_str(), self.kind) {
            ("t", SystemKind::Ode) | ("n", SystemKind::Map) => return Node::Time,
            ("pi", _) => return Node::Pi,
            ("e", _) => return Node::E,
            _ => {}
        }
        if let Some(msg) = function_typo(&s.name, self.text, self.table) {
            return self.fail(msg, span);
        }
        if let Some(&i) = self.index.get(&s.name) {
            // Keep the earliest occurrence in the source: a rearranged line
            // is visited out of source order (`m\ddot{x} = -kx + mg` visits
            // the left-hand m last).
            if span.0 < self.params[i].1 .0 {
                self.params[i].1 = span;
            }
            return Node::Param(i);
        }
        let i = self.params.len();
        self.params.push((s.clone(), span));
        self.index.insert(s.name.clone(), i);
        Node::Param(i)
    }

    fn derivative(&mut self, s: &SymName, k: u32, span: Span) -> Node {
        let d = show_deriv(s, k, self.text);
        let x = show(s, self.text);
        if self.kind == SystemKind::Map {
            return self.fail(format!("Derivatives like {d} can't appear in a map"), span);
        }
        match self.table.by_name.get(&s.name) {
            Some(idx) if (k as usize) < idx.len() => Node::State(idx[k as usize]),
            Some(idx) => {
                let order = idx.len() as u32;
                self.fail(
                    format!(
                        "{d} can't appear on the right: {x} has an order-{order} equation, so \
                         {d} isn't one of the variables. Only lower derivatives of a higher-order \
                         equation can (like {} in {} = …)",
                        show_deriv(s, 1, self.text),
                        show_deriv(s, 2, self.text)
                    ),
                    span,
                )
            }
            None => self.fail(format!("{d} appears on the right, but {x} has no equation"), span),
        }
    }

    fn apply(&mut self, s: &SymName, args: &[Ast], span: Span) -> Node {
        let arg_is_t = args.len() == 1 && is_sym(&args[0], "t");
        // x(t): a state written as a function of time.
        if arg_is_t && self.kind == SystemKind::Ode {
            if let Some(idx) = self.table.by_name.get(&s.name) {
                return Node::State(idx[0]);
            }
        }
        if !self.text && args.len() == 1 {
            // LaTeX a(t) or H(x): ordinary multiplication.
            if arg_is_t {
                let x = show(s, false);
                self.diags.push(Diagnostic::warning(
                    format!("{x}(t) is read as {x}·t, since {x} has no equation"),
                    Some(span),
                ));
            }
            let l = self.symbol(s, span);
            let r = self.resolve(&args[0]);
            return Node::bin(BinOp::Mul, l, r);
        }
        if !self.text {
            return self.fail(
                format!("{}(…) can only be used on the left, as in H(q, p) = …", s.name),
                span,
            );
        }
        if let Some(msg) = function_typo(&s.name, true, self.table) {
            return self.fail(msg, span);
        }
        self.fail(
            format!(
                "{0} is not a known function. To multiply, write {0}*(…). Known functions: \
                 sin, cos, tan, exp, ln, log, sqrt, abs, sign, sinh, cosh, tanh, arcsin, arccos, \
                 arctan, min, max",
                s.name
            ),
            span,
        )
    }
}

/// Replace `Param(k)` by `Param(map[k])` everywhere.
fn renumber_params(n: &mut Node, map: &[usize]) {
    match n {
        Node::Param(k) => *k = map[*k],
        Node::Neg(a) => renumber_params(a, map),
        Node::Bin(_, a, b) => {
            renumber_params(a, map);
            renumber_params(b, map);
        }
        Node::Call(_, args) => args.iter_mut().for_each(|a| renumber_params(a, map)),
        Node::Num(_) | Node::Pi | Node::E | Node::Time | Node::State(_) => {}
    }
}

// ----- driver ------------------------------------------------------------

/// Names that are usually state variables; a parameter with one of these
/// names probably means a missing equation.
const STATE_LIKE: &[&str] = &["x", "y", "z", "u", "v", "w"];

/// Run the semantic pass. Problems go to `diags`; returns `None` if there
/// are errors (including ones already in `diags` from parsing).
pub(crate) fn analyze(
    lines: Vec<Line>,
    format: InputFormat,
    diags: &mut Vec<Diagnostic>,
) -> Option<Analysis> {
    let text = format == InputFormat::Text;
    let mut eqns = Vec::new();
    for line in lines {
        match classify(line, text) {
            Ok(e) => eqns.push(e),
            Err(d) => diags.push(d),
        }
    }

    // Kind of system.
    let is_flow = |e: &Eqn| matches!(e.target, Target::Ode(..) | Target::Ham(_));
    let first_flow = eqns.iter().position(is_flow);
    let first_map = eqns.iter().position(|e| matches!(e.target, Target::Map(_)));
    let kind = match (first_flow, first_map) {
        (Some(a), Some(b)) => {
            let (ode, map) = if text { ("x' = …", "x_{n+1} = …") } else { ("\\dot{x} = …", "x_{n+1} = …") };
            diags.push(error(
                format!(
                    "This mixes differential equations ({ode}) and maps ({map}); \
                     use only one kind"
                ),
                eqns[a.max(b)].lhs_span,
            ));
            return None;
        }
        (Some(_), None) => SystemKind::Ode,
        (None, Some(_)) => SystemKind::Map,
        (None, None) => {
            if !diags.iter().any(Diagnostic::is_error) {
                let span = eqns.first().map(|e| e.lhs_span);
                let ex = if text { "x' = -x" } else { "\\dot{x} = -x" };
                diags.push(Diagnostic::error(
                    format!("No equations found, only parameter values. Add one such as {ex}"),
                    span,
                ));
            }
            return None;
        }
    };

    // A Hamiltonian must stand alone.
    let hams: Vec<&Eqn> = eqns.iter().filter(|e| matches!(e.target, Target::Ham(_))).collect();
    let mut hamiltonian_dof = None;
    if let Some(first) = hams.first() {
        if let Some(second) = hams.get(1) {
            diags.push(error("Only one Hamiltonian H(…) = … is allowed", second.lhs_span));
            return None;
        }
        if let Some(ode) = eqns.iter().find(|e| matches!(e.target, Target::Ode(..))) {
            diags.push(error(
                "A Hamiltonian H(q, p) = … already defines the whole system; remove the other \
                 equations, or write every equation out instead of H",
                ode.lhs_span,
            ));
            return None;
        }
        if let Target::Ham(args) = &first.target {
            hamiltonian_dof = Some(args.len() / 2);
        }
    }

    // States, in equation order.
    let mut table = StateTable::default();
    let mut accepted = vec![false; eqns.len()];
    for (i, e) in eqns.iter().enumerate() {
        accepted[i] = match &e.target {
            Target::Ode(s, k) => table.add(s, *k, kind, e.lhs_span, text, diags),
            Target::Map(s) => table.add(s, 1, kind, e.lhs_span, text, diags),
            Target::Ham(args) => {
                args.iter().all(|a| table.add(a, 1, kind, e.lhs_span, text, diags))
            }
            Target::Default(_) => false,
        };
    }

    // Parameter defaults.
    let mut defaults: HashMap<String, f64> = HashMap::new();
    let mut default_lines: Vec<(SymName, Span)> = Vec::new();
    for e in &eqns {
        let Target::Default(s) = &e.target else { continue };
        let a = show(s, text);
        if table.by_name.contains_key(&s.name) {
            diags.push(error(
                format!(
                    "{a} is a state variable, so it can't be given a fixed value here. \
                     Set initial values in the panel"
                ),
                e.lhs_span,
            ));
            continue;
        }
        if let Some(msg) = reserved_message(&s.name, kind) {
            diags.push(error(msg, e.lhs_span));
            continue;
        }
        match const_eval(&e.rhs) {
            Ok(v) if v.is_finite() => {
                if defaults.insert(s.name.clone(), v).is_some() {
                    diags.push(error(format!("{a} is given a value twice"), e.lhs_span));
                } else {
                    default_lines.push((s.clone(), e.lhs_span));
                }
            }
            Ok(_) => diags.push(error(format!("The value of {a} isn't a finite number"), e.rhs.span)),
            Err(ConstErr::Other(msg, span)) => diags.push(error(msg, span)),
            Err(ConstErr::Symbol(used)) => {
                let msg = if s.name == "H" {
                    "To enter a Hamiltonian, list its variables: H(q, p) = …".to_string()
                } else {
                    format!(
                        "{a} = … must be a plain number to set the default value of {a}, but it \
                         uses {used}. To give {a} an equation write {} = …",
                        show_deriv(s, 1, text)
                    )
                };
                diags.push(error(msg, (e.lhs_span.0, e.rhs.span.1)));
            }
        }
    }

    // Right-hand sides.
    let mut r = Resolver {
        kind,
        text,
        table: &table,
        params: Vec::new(),
        index: HashMap::new(),
        diags: &mut *diags,
    };
    let mut roots = Vec::new();
    for (i, e) in eqns.iter().enumerate() {
        if !accepted[i] {
            continue;
        }
        let node = r.resolve(&e.rhs);
        if let Target::Ode(s, k) = &e.target {
            // x' = x', ..., x^(k-1)' = x^(k-1)', x^(k-1)' = rhs
            let idx = &table.by_name[&s.name];
            for &j in &idx[1..*k as usize] {
                roots.push(Node::State(j));
            }
        }
        roots.push(node);
    }
    let params = std::mem::take(&mut r.params);
    let param_index = std::mem::take(&mut r.index);
    // Order parameters by where they first appear in the source (this
    // matters for rearranged lines such as `m x'' + k x = 0`, whose
    // resolved form visits `m` last; `symbol` keeps the earliest span).
    let mut order: Vec<usize> = (0..params.len()).collect();
    order.sort_by_key(|&i| params[i].1 .0);
    let mut new_of_old = vec![0; params.len()];
    for (new, &old) in order.iter().enumerate() {
        new_of_old[old] = new;
    }
    for root in &mut roots {
        renumber_params(root, &new_of_old);
    }
    let params: Vec<(SymName, Span)> = order.iter().map(|&i| params[i].clone()).collect();

    for (s, span) in &default_lines {
        if !param_index.contains_key(&s.name) {
            diags.push(Diagnostic::warning(
                format!("{} is given a value but isn't used in any equation", show(s, text)),
                Some(*span),
            ));
        }
    }
    if diags.iter().any(Diagnostic::is_error) {
        return None;
    }
    for (s, span) in &params {
        if defaults.contains_key(&s.name) {
            continue;
        }
        let base = s.name.split('_').next().unwrap_or("");
        let sibling = table.syms.iter().any(|t| t.sub_latex.is_some() && t.name.starts_with(&format!("{base}_")));
        if STATE_LIKE.contains(&s.name.as_str()) || sibling {
            diags.push(Diagnostic::warning(
                format!("{} has no equation; treating it as a parameter", show(s, text)),
                Some(*span),
            ));
        }
    }

    // Display.
    let states: Vec<SymbolInfo> = (0..table.len()).map(|i| table.info(i)).collect();
    let state_disp: Vec<String> = match kind {
        SystemKind::Ode => states.iter().map(|s| s.latex.clone()).collect(),
        SystemKind::Map => table.syms.iter().map(|s| s.map_latex("n")).collect(),
    };
    let params: Vec<ParamInfo> = params
        .iter()
        .map(|(s, _)| ParamInfo {
            name: s.name.clone(),
            latex: s.latex(),
            default: defaults.get(&s.name).copied().unwrap_or(1.0),
        })
        .collect();
    let param_disp: Vec<String> = params.iter().map(|p| p.latex.clone()).collect();
    let names = Names {
        states: &state_disp,
        params: &param_disp,
        time: if kind == SystemKind::Map { "n" } else { "t" },
    };
    let equations_latex: Vec<String> = match (kind, hamiltonian_dof) {
        (_, Some(dof)) => {
            let mut v = Vec::new();
            for i in 0..dof {
                v.push(format!(
                    "{} = \\frac{{\\partial H}}{{\\partial {}}}",
                    table.syms[i].dotted(1),
                    table.syms[dof + i].latex()
                ));
            }
            for i in 0..dof {
                v.push(format!(
                    "{} = -\\frac{{\\partial H}}{{\\partial {}}}",
                    table.syms[dof + i].dotted(1),
                    table.syms[i].latex()
                ));
            }
            v
        }
        (SystemKind::Ode, None) => (0..table.len())
            .map(|i| {
                let lhs = table.syms[i].dotted(table.orders[i] + 1);
                format!("{lhs} = {}", to_latex(&roots[i], &names))
            })
            .collect(),
        (SystemKind::Map, None) => (0..table.len())
            .map(|i| format!("{} = {}", table.syms[i].map_latex("n+1"), to_latex(&roots[i], &names)))
            .collect(),
    };

    let autonomous = !roots.iter().any(Node::uses_time);
    Some(Analysis {
        info: SystemInfo {
            kind,
            hamiltonian: hamiltonian_dof.is_some(),
            autonomous,
            states,
            params,
            equations_latex,
        },
        roots,
        hamiltonian_dof,
    })
}
