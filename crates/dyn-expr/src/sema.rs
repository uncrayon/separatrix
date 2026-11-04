use std::collections::HashMap;

use dyn_core::SystemKind;

use crate::ast::{Ast, AstKind, BinOp, Func, Node, FUNCTION_WORDS};
use crate::diag::Diagnostic;
use crate::latex::{to_latex, Names};
use crate::parser::Line;
use crate::program::call1;
use crate::token::{is_greek, sym_from_word, Span, SymName};
use crate::{InputFormat, ParamInfo, SymbolInfo, SystemInfo};

/// The right-hand sides of a system, by how they define the field.
pub(crate) enum Form {
    /// One expression per state.
    Direct(Vec<Node>),
    /// A single Hamiltonian `H(q, p)` with `dof` coordinates and momenta.
    Hamiltonian { h: Node, dof: usize },
}

impl Form {
    fn roots(&self) -> &[Node] {
        match self {
            Form::Direct(roots) => roots,
            Form::Hamiltonian { h, .. } => std::slice::from_ref(h),
        }
    }
}

pub(crate) struct Analysis {
    pub info: SystemInfo,
    pub form: Form,
}

enum Target {
    Ode(SymName, u32),
    Map(SymName),
    Ham {
        coords: Vec<SymName>,
        momenta: Vec<SymName>,
    },
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

fn classify(line: Line, text: bool) -> Result<Eqn, Diagnostic> {
    let Line { lhs, rhs, span } = line;
    let lhs = leibniz_to_deriv(lhs);
    let rhs = leibniz_to_deriv(rhs);
    let lhs_span = lhs.span;
    let target = match (&lhs.kind, lhs.plain_symbol()) {
        (AstKind::Deriv(s, k), _) => Target::Ode(s.clone(), *k),
        (AstKind::Next(s), _) => Target::Map(s.clone()),
        (AstKind::Apply(s, args), _) if s.name == "H" => hamiltonian_args(args, lhs_span, text)?,
        (_, Some(_)) if has_deriv(&rhs) => return rearrange(&lhs, &rhs, span, text),
        (_, Some(s)) => Target::Default(s),
        _ if has_deriv(&lhs) || has_deriv(&rhs) => return rearrange(&lhs, &rhs, span, text),
        _ => {
            let msg = other_variable_message(&lhs, text).unwrap_or_else(|| {
                format!(
                    "Can't tell what this line defines. Start it with {}",
                    examples(text)
                )
            });
            return Err(error(msg, lhs_span));
        }
    };
    Ok(Eqn {
        target,
        rhs,
        lhs_span,
    })
}

fn hamiltonian_args(args: &[Ast], span: Span, text: bool) -> Result<Target, Diagnostic> {
    let mut out: Vec<SymName> = Vec::new();
    for a in args {
        match a.plain_symbol() {
            Some(s) => {
                if out.iter().any(|o| o.name == s.name) {
                    return Err(error(
                        format!("{} is listed twice in H(…)", show(&s, text)),
                        a.span,
                    ));
                }
                out.push(s);
            }
            None => {
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
    let momenta = out.split_off(out.len() / 2);
    Ok(Target::Ham {
        coords: out,
        momenta,
    })
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

fn d_power(a: &Ast) -> Option<u32> {
    match &a.kind {
        AstKind::Sym(s) if s.name == "d" => Some(1),
        AstKind::Bin(BinOp::Pow, b, e) if is_sym(b, "d") => whole_number(e),
        _ => None,
    }
}

/// The variable of a differential written as one word, as text mode reads
/// `dx` and `dx_1`. A Greek letter that starts with d (`\delta`, the word
/// `delta`) is that letter, not `d` times `elta`.
fn one_word_differential(name: &str) -> Option<SymName> {
    let rest = name.strip_prefix('d')?;
    let base = name.split('_').next().unwrap_or(name);
    (rest.starts_with(|c: char| c.is_ascii_alphabetic()) && !is_greek(base))
        .then(|| sym_from_word(rest))
}

/// Text mode writes `dt` as one word: the variable it differentiates by.
fn text_d_word(s: &SymName) -> Option<SymName> {
    s.sub_latex
        .is_none()
        .then(|| one_word_differential(&s.name))
        .flatten()
}

fn d_var_power(a: &Ast) -> Option<(u32, SymName)> {
    match &a.kind {
        AstKind::Sym(s) => Some((1, text_d_word(s)?)),
        AstKind::Bin(BinOp::Pow, b, e) => match &b.kind {
            AstKind::Sym(s) => Some((whole_number(e)?, text_d_word(s)?)),
            _ => None,
        },
        AstKind::Bin(BinOp::Mul, d, v) if d_power(d) == Some(1) => match &v.kind {
            AstKind::Bin(BinOp::Pow, b, e) => Some((whole_number(e)?, b.plain_symbol()?)),
            _ => Some((1, v.plain_symbol()?)),
        },
        _ => None,
    }
}

/// `d^k x / d var^k` (or `d^k/d var^k x`) as `(x, k, var)`. The variables are
/// read as plain names, so `x_n` is the variable `x_n`, as in `x_n'`.
fn leibniz_wrt(a: &Ast) -> Option<(SymName, u32, SymName)> {
    match &a.kind {
        AstKind::Bin(BinOp::Div, num, den) => {
            let (k, var) = d_var_power(den)?;
            match &num.kind {
                AstKind::Bin(BinOp::Mul, d, x) if d_power(d) == Some(k) => {
                    Some((x.plain_symbol()?, k, var))
                }
                _ if k == 1 => Some((one_word_differential(&num.plain_symbol()?.name)?, 1, var)),
                _ => None,
            }
        }
        AstKind::Bin(BinOp::Mul, op, x) => {
            let AstKind::Bin(BinOp::Div, d, den) = &op.kind else {
                return None;
            };
            let (k, var) = d_var_power(den)?;
            if d_power(d) != Some(k) {
                return None;
            }
            Some((x.plain_symbol()?, k, var))
        }
        _ => None,
    }
}

fn leibniz(a: &Ast) -> Option<(SymName, u32)> {
    let (s, k, var) = leibniz_wrt(a)?;
    (var.name == "t").then_some((s, k))
}

fn leibniz_to_deriv(a: Ast) -> Ast {
    if let Some((s, k)) = leibniz(&a) {
        return Ast::new(AstKind::Deriv(s, k), a.span);
    }
    let kind = match a.kind {
        AstKind::Neg(x) => AstKind::Neg(Box::new(leibniz_to_deriv(*x))),
        AstKind::Bin(op, x, y) => AstKind::Bin(
            op,
            Box::new(leibniz_to_deriv(*x)),
            Box::new(leibniz_to_deriv(*y)),
        ),
        AstKind::Call(f, args) => {
            AstKind::Call(f, args.into_iter().map(leibniz_to_deriv).collect())
        }
        AstKind::Apply(s, args) => {
            AstKind::Apply(s, args.into_iter().map(leibniz_to_deriv).collect())
        }
        leaf => leaf,
    };
    Ast::new(kind, a.span)
}

fn other_variable_message(lhs: &Ast, text: bool) -> Option<String> {
    let mut found = None;
    lhs.walk(&mut |n| {
        if found.is_none() {
            found = leibniz_wrt(n).filter(|(_, _, v)| v.name != "t");
        }
    });
    let (y, k, x) = found?;
    let (yd, xd, yt) = if text {
        (
            show_deriv(&y, 1, true),
            show_deriv(&x, 1, true),
            format!("d{}/dt", y.name),
        )
    } else {
        (
            show_deriv(&y, 1, false),
            show_deriv(&x, 1, false),
            format!("\\frac{{d{}}}{{dt}}", y.latex()),
        )
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

/// The target derivative `D` as it sits in one term: `sign · num · D / den`.
struct TargetTerm {
    sign: f64,
    num: Option<Ast>,
    den: Option<Ast>,
}

/// Read `a` as a multiple of the target derivative, if it is one.
fn as_target_multiple(a: &Ast, name: &str, k: u32) -> Option<TargetTerm> {
    match &a.kind {
        AstKind::Deriv(s, j) if s.name == name && *j == k => Some(TargetTerm {
            sign: 1.0,
            num: None,
            den: None,
        }),
        AstKind::Neg(x) => {
            as_target_multiple(x, name, k).map(|t| TargetTerm { sign: -t.sign, ..t })
        }
        AstKind::Bin(BinOp::Mul, x, y) => {
            let (inner, other) = match (contains_target(x, name, k), contains_target(y, name, k)) {
                (true, false) => (x, y),
                (false, true) => (y, x),
                _ => return None,
            };
            let t = as_target_multiple(inner, name, k)?;
            Some(TargetTerm {
                num: Some(mul_opt((**other).clone(), t.num)),
                ..t
            })
        }
        AstKind::Bin(BinOp::Div, x, y)
            if contains_target(x, name, k) && !contains_target(y, name, k) =>
        {
            let t = as_target_multiple(x, name, k)?;
            Some(TargetTerm {
                den: Some(mul_opt((**y).clone(), t.den)),
                ..t
            })
        }
        _ => None,
    }
}

/// Solve `sign · num · D / den + Σ s_i t_i = 0` for `D`:
/// `D = den · Σ(-sign · s_i · t_i) / num`.
fn solve_for_target(term: TargetTerm, others: Vec<(f64, Ast)>, span: Span) -> Ast {
    let mut sum: Option<Ast> = None;
    for (s, t) in others {
        if matches!(t.kind, AstKind::Num(v) if v == 0.0) {
            continue;
        }
        let positive = -term.sign * s > 0.0;
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
    if let Some(d) = term.den {
        out = Ast::bin(BinOp::Mul, d, out);
    }
    if let Some(n) = term.num {
        out = Ast::bin(BinOp::Div, out, n);
    }
    out
}

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
                tops.iter()
                    .map(|s| show_deriv(s, top, text))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            span,
        ));
    }
    let target = tops[0].clone();
    let dname = show_deriv(&target, top, text);
    let mut terms = Vec::new();
    flatten(lhs, 1.0, &mut terms);
    flatten(rhs, -1.0, &mut terms);
    let (with, without): (Vec<_>, Vec<_>) = terms
        .into_iter()
        .partition(|(_, t)| contains_target(t, &target.name, top));
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
    let Some(term) = as_target_multiple(term, &target.name, top) else {
        return Err(error(
            format!("Can't solve this line for {dname}; write it as {dname} = …"),
            span,
        ));
    };
    let term = TargetTerm {
        sign: s0 * term.sign,
        ..term
    };
    Ok(Eqn {
        target: Target::Ode(target, top),
        rhs: solve_for_target(term, without, span),
        lhs_span: span,
    })
}

#[derive(Default)]
struct StateTable {
    syms: Vec<SymName>,
    orders: Vec<u32>,
    by_name: HashMap<String, Vec<usize>>,
}

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
                format!(
                    "{} has more than one equation; keep only one",
                    show(s, text)
                ),
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
            derived: self.orders[i] > 0,
        }
    }
}

enum ConstErr {
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

fn const_eval(a: &Ast) -> Result<f64, ConstErr> {
    Ok(match &a.kind {
        AstKind::Num(v) => *v,
        AstKind::Sym(s) if s.name == "pi" => std::f64::consts::PI,
        AstKind::Sym(s) if s.name == "e" => std::f64::consts::E,
        AstKind::Sym(s) | AstKind::Deriv(s, _) | AstKind::Next(s) | AstKind::Apply(s, _) => {
            return Err(ConstErr::Symbol(s.name.clone()))
        }
        AstKind::Current(s) => return Err(ConstErr::Symbol(s.at_index_n().name)),
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

/// The function a multi-letter name looks like, followed by a plausible
/// argument (`cost` → `\cos t`), as `(function, rest)`.
fn function_lookalike<'n>(name: &'n str, table: &StateTable) -> Option<(&'static str, &'n str)> {
    FUNCTION_WORDS.iter().find_map(|&f| {
        let rest = name.strip_prefix(f).filter(|r| !r.is_empty())?;
        let plausible = rest.len() <= 2 || is_greek(rest) || table.by_name.contains_key(rest);
        let word = rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        (plausible && word).then_some((f, rest))
    })
}

/// How the function a name looks like would be written: `cos(t)` or `\cos t`.
fn function_fix(f: &str, rest: &str, text: bool) -> String {
    if text {
        format!("{f}({rest})")
    } else {
        format!("\\{f} {rest}")
    }
}

/// What a plain name means in a system.
enum Meaning {
    State(usize),
    Time,
    Pi,
    E,
    Param,
}

/// The names a system defines: what its right-hand sides can refer to.
struct Scope<'a> {
    kind: SystemKind,
    text: bool,
    table: &'a StateTable,
}

impl Scope<'_> {
    fn meaning(&self, s: &SymName) -> Meaning {
        if let Some(idx) = self.table.by_name.get(&s.name) {
            return Meaning::State(idx[0]);
        }
        match (s.name.as_str(), self.kind) {
            ("t", SystemKind::Ode) | ("n", SystemKind::Map) => return Meaning::Time,
            ("pi", _) => return Meaning::Pi,
            ("e", _) => return Meaning::E,
            _ => {}
        }
        // A multi-letter name only reaches here when it was spelled as one
        // name (`\mathrm{cost}`, or the word `cost` in text mode), so it is a
        // parameter even if it looks like a function; `warn_function_lookalikes`
        // points out the possible typo.
        Meaning::Param
    }

    /// The state that `s_n` reads, in a map that has a state `s`.
    fn current_state(&self, s: &SymName) -> Option<usize> {
        if self.kind != SystemKind::Map {
            return None;
        }
        self.table.by_name.get(&s.name).map(|idx| idx[0])
    }

    /// The plain name a node reads, if any: a variable, a current iterate
    /// that is not a state's, or the name in front of `x(…)`.
    fn name_read(&self, a: &Ast) -> Option<SymName> {
        match &a.kind {
            AstKind::Sym(s) | AstKind::Apply(s, _) => Some(s.clone()),
            AstKind::Current(s) if self.current_state(s).is_none() => Some(s.at_index_n()),
            _ => None,
        }
    }
}

/// The parameters the accepted right-hand sides use, in order of first
/// appearance in the source, each with the span of that first use.
fn parameters_by_first_appearance(
    scope: &Scope,
    eqns: &[Eqn],
    accepted: &[bool],
) -> Vec<(SymName, Span)> {
    let mut uses: Vec<(SymName, Span)> = Vec::new();
    for (e, _) in eqns.iter().zip(accepted).filter(|(_, &ok)| ok) {
        e.rhs.walk(&mut |n| {
            if let Some(s) = scope.name_read(n) {
                if matches!(scope.meaning(&s), Meaning::Param) {
                    uses.push((s, n.span));
                }
            }
        });
    }
    uses.sort_by_key(|(_, span)| span.0);
    let mut params: Vec<(SymName, Span)> = Vec::new();
    for (s, span) in uses {
        if !params.iter().any(|(p, _)| p.name == s.name) {
            params.push((s, span));
        }
    }
    params
}

struct Resolver<'a> {
    scope: &'a Scope<'a>,
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
            AstKind::Sym(s) => self.symbol(s),
            AstKind::Current(s) => match self.scope.current_state(s) {
                Some(i) => Node::State(i),
                None => self.symbol(&s.at_index_n()),
            },
            AstKind::Deriv(s, k) => self.derivative(s, *k, a.span),
            AstKind::Next(s) => self.fail(
                format!("{} can only appear on the left of =", s.map_latex("n+1")),
                a.span,
            ),
            AstKind::Neg(x) => Node::Neg(Box::new(self.resolve(x))),
            AstKind::Bin(op, x, y) => {
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

    fn symbol(&self, s: &SymName) -> Node {
        match self.scope.meaning(s) {
            Meaning::State(i) => Node::State(i),
            Meaning::Time => Node::Time,
            Meaning::Pi => Node::Pi,
            Meaning::E => Node::E,
            Meaning::Param => Node::Param(self.index[&s.name]),
        }
    }

    fn derivative(&mut self, s: &SymName, k: u32, span: Span) -> Node {
        let text = self.scope.text;
        let d = show_deriv(s, k, text);
        let x = show(s, text);
        if self.scope.kind == SystemKind::Map {
            return self.fail(format!("Derivatives like {d} can't appear in a map"), span);
        }
        match self.scope.table.by_name.get(&s.name) {
            Some(idx) if (k as usize) < idx.len() => Node::State(idx[k as usize]),
            Some(idx) => {
                let order = idx.len() as u32;
                self.fail(
                    format!(
                        "{d} can't appear on the right: {x} has an order-{order} equation, so \
                         {d} isn't one of the variables. Only lower derivatives of a higher-order \
                         equation can (like {} in {} = …)",
                        show_deriv(s, 1, text),
                        show_deriv(s, 2, text)
                    ),
                    span,
                )
            }
            None => self.fail(
                format!("{d} appears on the right, but {x} has no equation"),
                span,
            ),
        }
    }

    fn apply(&mut self, s: &SymName, args: &[Ast], span: Span) -> Node {
        let text = self.scope.text;
        let arg_is_t = args.len() == 1 && is_sym(&args[0], "t");
        if arg_is_t && self.scope.kind == SystemKind::Ode {
            if let Some(idx) = self.scope.table.by_name.get(&s.name) {
                return Node::State(idx[0]);
            }
        }
        if !text && args.len() == 1 {
            if arg_is_t {
                let x = show(s, false);
                self.diags.push(Diagnostic::warning(
                    format!("{x}(t) is read as {x}·t, since {x} has no equation"),
                    Some(span),
                ));
            }
            let l = self.symbol(s);
            let r = self.resolve(&args[0]);
            return Node::bin(BinOp::Mul, l, r);
        }
        if !text {
            return self.fail(
                format!(
                    "{}(…) can only be used on the left, as in H(q, p) = …",
                    s.name
                ),
                span,
            );
        }
        if let Some((f, rest)) = function_lookalike(&s.name, self.scope.table) {
            return self.fail(
                format!(
                    "{0} is not a function; did you mean {1}? To multiply, write {0}*(…)",
                    s.name,
                    function_fix(f, rest, true)
                ),
                span,
            );
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

const STATE_LIKE: &[&str] = &["x", "y", "z", "u", "v", "w"];

fn classify_lines(lines: Vec<Line>, text: bool, diags: &mut Vec<Diagnostic>) -> Vec<Eqn> {
    let mut eqns = Vec::new();
    for line in lines {
        match classify(line, text) {
            Ok(e) => eqns.push(e),
            Err(d) => diags.push(d),
        }
    }
    eqns
}

fn system_kind(eqns: &[Eqn], text: bool, diags: &mut Vec<Diagnostic>) -> Option<SystemKind> {
    let is_flow = |e: &Eqn| matches!(e.target, Target::Ode(..) | Target::Ham { .. });
    let first_flow = eqns.iter().position(is_flow);
    let first_map = eqns.iter().position(|e| matches!(e.target, Target::Map(_)));
    match (first_flow, first_map) {
        (Some(a), Some(b)) => {
            let (ode, map) = if text {
                ("x' = …", "x_{n+1} = …")
            } else {
                ("\\dot{x} = …", "x_{n+1} = …")
            };
            diags.push(error(
                format!(
                    "This mixes differential equations ({ode}) and maps ({map}); \
                     use only one kind"
                ),
                eqns[a.max(b)].lhs_span,
            ));
            None
        }
        (Some(_), None) => Some(SystemKind::Ode),
        (None, Some(_)) => Some(SystemKind::Map),
        (None, None) => {
            if !diags.iter().any(Diagnostic::is_error) {
                let span = eqns.first().map(|e| e.lhs_span);
                let ex = if text { "x' = -x" } else { "\\dot{x} = -x" };
                diags.push(Diagnostic::error(
                    format!("No equations found, only parameter values. Add one such as {ex}"),
                    span,
                ));
            }
            None
        }
    }
}

/// The Hamiltonian's degrees of freedom, if the system is one. A
/// Hamiltonian must be the system's only equation.
fn check_hamiltonian_alone(eqns: &[Eqn]) -> Result<Option<usize>, Diagnostic> {
    let mut hams = eqns.iter().filter_map(|e| match &e.target {
        Target::Ham { coords, .. } => Some((e, coords.len())),
        _ => None,
    });
    let Some((_, dof)) = hams.next() else {
        return Ok(None);
    };
    if let Some((second, _)) = hams.next() {
        return Err(error(
            "Only one Hamiltonian H(…) = … is allowed",
            second.lhs_span,
        ));
    }
    if let Some(ode) = eqns.iter().find(|e| matches!(e.target, Target::Ode(..))) {
        return Err(error(
            "A Hamiltonian H(q, p) = … already defines the whole system; remove the other \
             equations, or write every equation out instead of H",
            ode.lhs_span,
        ));
    }
    Ok(Some(dof))
}

/// The state table, and for each equation whether its states were accepted.
fn collect_states(
    eqns: &[Eqn],
    kind: SystemKind,
    text: bool,
    diags: &mut Vec<Diagnostic>,
) -> (StateTable, Vec<bool>) {
    let mut table = StateTable::default();
    let accepted = eqns
        .iter()
        .map(|e| match &e.target {
            Target::Ode(s, k) => table.add(s, *k, kind, e.lhs_span, text, diags),
            Target::Map(s) => table.add(s, 1, kind, e.lhs_span, text, diags),
            Target::Ham { coords, momenta } => coords
                .iter()
                .chain(momenta)
                .all(|a| table.add(a, 1, kind, e.lhs_span, text, diags)),
            Target::Default(_) => false,
        })
        .collect();
    (table, accepted)
}

/// Values set by lines like `a = 2`, and the lines that set them.
struct Defaults {
    values: HashMap<String, f64>,
    lines: Vec<(SymName, Span)>,
}

fn parameter_defaults(
    eqns: &[Eqn],
    table: &StateTable,
    kind: SystemKind,
    text: bool,
    diags: &mut Vec<Diagnostic>,
) -> Defaults {
    let mut d = Defaults {
        values: HashMap::new(),
        lines: Vec::new(),
    };
    for e in eqns {
        let Target::Default(s) = &e.target else {
            continue;
        };
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
                if d.values.insert(s.name.clone(), v).is_some() {
                    diags.push(error(format!("{a} is given a value twice"), e.lhs_span));
                } else {
                    d.lines.push((s.clone(), e.lhs_span));
                }
            }
            Ok(_) => diags.push(error(
                format!("The value of {a} isn't a finite number"),
                e.rhs.span,
            )),
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
    d
}

/// One right-hand side per state: an order-`k` equation contributes the
/// trivial rows `x' = x'`, …, then its own right-hand side.
fn resolve_rhs(r: &mut Resolver, table: &StateTable, eqns: &[Eqn], accepted: &[bool]) -> Vec<Node> {
    let mut roots = Vec::new();
    for (e, _) in eqns.iter().zip(accepted).filter(|(_, &ok)| ok) {
        let node = r.resolve(&e.rhs);
        if let Target::Ode(s, k) = &e.target {
            let idx = &table.by_name[&s.name];
            roots.extend(idx[1..*k as usize].iter().map(|&j| Node::State(j)));
        }
        roots.push(node);
    }
    roots
}

fn warn_parameter_lookalikes(
    params: &[(SymName, Span)],
    defaults: &Defaults,
    table: &StateTable,
    text: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for (s, span) in params {
        if defaults.values.contains_key(&s.name) {
            continue;
        }
        let base = s.name.split('_').next().unwrap_or("");
        let sibling = table
            .syms
            .iter()
            .any(|t| t.sub_latex.is_some() && t.name.starts_with(&format!("{base}_")));
        if STATE_LIKE.contains(&s.name.as_str()) || sibling {
            diags.push(Diagnostic::warning(
                format!(
                    "{} has no equation; treating it as a parameter",
                    show(s, text)
                ),
                Some(*span),
            ));
        }
    }
}

/// A parameter whose name starts with a function name (`\mathrm{cost}`,
/// `signal`) compiles as that parameter, but may be a missing space or
/// bracket, so say how the function would be written. A parameter given a
/// value on its own line is clearly meant as a name, and a name already
/// reported as an error where it first appears (`cost(x)` in text mode) needs
/// no second note.
fn warn_function_lookalikes(
    params: &[(SymName, Span)],
    defaults: &Defaults,
    table: &StateTable,
    text: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for (s, span) in params {
        let reported = diags
            .iter()
            .any(|d| d.is_error() && d.span.is_some_and(|(a, _)| a == span.0));
        if reported || defaults.values.contains_key(&s.name) {
            continue;
        }
        if let Some((f, rest)) = function_lookalike(&s.name, table) {
            diags.push(Diagnostic::warning(
                format!(
                    "{} is read as a parameter named {}; did you mean {}?",
                    show(s, text),
                    s.name,
                    function_fix(f, rest, text)
                ),
                Some(*span),
            ));
        }
    }
}

fn display_latex(form: &Form, kind: SystemKind, table: &StateTable, names: &Names) -> Vec<String> {
    match (form, kind) {
        (Form::Hamiltonian { dof, .. }, _) => {
            let dof = *dof;
            let (q, p) = table.syms.split_at(dof);
            let flows = q.iter().zip(p).map(|(q, p)| {
                format!(
                    "{} = \\frac{{\\partial H}}{{\\partial {}}}",
                    q.dotted(1),
                    p.latex()
                )
            });
            let forces = q.iter().zip(p).map(|(q, p)| {
                format!(
                    "{} = -\\frac{{\\partial H}}{{\\partial {}}}",
                    p.dotted(1),
                    q.latex()
                )
            });
            flows.chain(forces).collect()
        }
        (Form::Direct(roots), SystemKind::Ode) => (0..table.len())
            .map(|i| {
                let lhs = table.syms[i].dotted(table.orders[i] + 1);
                format!("{lhs} = {}", to_latex(&roots[i], names))
            })
            .collect(),
        (Form::Direct(roots), SystemKind::Map) => (0..table.len())
            .map(|i| {
                format!(
                    "{} = {}",
                    table.syms[i].map_latex("n+1"),
                    to_latex(&roots[i], names)
                )
            })
            .collect(),
    }
}

pub(crate) fn analyze(
    lines: Vec<Line>,
    format: InputFormat,
    diags: &mut Vec<Diagnostic>,
) -> Option<Analysis> {
    let text = format == InputFormat::Text;
    let eqns = classify_lines(lines, text, diags);
    let kind = system_kind(&eqns, text, diags)?;
    let hamiltonian_dof = match check_hamiltonian_alone(&eqns) {
        Ok(dof) => dof,
        Err(d) => {
            diags.push(d);
            return None;
        }
    };
    let (table, accepted) = collect_states(&eqns, kind, text, diags);
    let defaults = parameter_defaults(&eqns, &table, kind, text, diags);

    let scope = Scope {
        kind,
        text,
        table: &table,
    };
    let params = parameters_by_first_appearance(&scope, &eqns, &accepted);
    let index: HashMap<String, usize> = params
        .iter()
        .enumerate()
        .map(|(i, (s, _))| (s.name.clone(), i))
        .collect();
    let mut resolver = Resolver {
        scope: &scope,
        index,
        diags: &mut *diags,
    };
    let mut roots = resolve_rhs(&mut resolver, &table, &eqns, &accepted);
    let index = resolver.index;
    warn_function_lookalikes(&params, &defaults, &table, text, diags);

    for (s, span) in &defaults.lines {
        if !index.contains_key(&s.name) {
            diags.push(Diagnostic::warning(
                format!(
                    "{} is given a value but isn't used in any equation",
                    show(s, text)
                ),
                Some(*span),
            ));
        }
    }
    if diags.iter().any(Diagnostic::is_error) {
        return None;
    }
    warn_parameter_lookalikes(&params, &defaults, &table, text, diags);

    let form = match hamiltonian_dof {
        Some(dof) => {
            let h = roots
                .pop()
                .expect("a Hamiltonian is the system's only equation");
            Form::Hamiltonian { h, dof }
        }
        None => Form::Direct(roots),
    };
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
            default: defaults.values.get(&s.name).copied().unwrap_or(1.0),
        })
        .collect();
    let param_disp: Vec<String> = params.iter().map(|p| p.latex.clone()).collect();
    let names = Names {
        states: &state_disp,
        params: &param_disp,
        time: if kind == SystemKind::Map { "n" } else { "t" },
    };
    let equations_latex = display_latex(&form, kind, &table, &names);
    let autonomous = !form.roots().iter().any(Node::uses_time);
    Some(Analysis {
        info: SystemInfo {
            kind,
            hamiltonian: matches!(form, Form::Hamiltonian { .. }),
            autonomous,
            states,
            params,
            equations_latex,
        },
        form,
    })
}
