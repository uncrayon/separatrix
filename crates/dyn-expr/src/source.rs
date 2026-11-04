//! Converting equations between the two input formats.
//!
//! Each line is parsed in its own format, the few constructs that the two
//! formats read differently are rewritten so the tree means the same in
//! both (`x(t)`, Leibniz fractions), and the tree is written out again node
//! for node in the other format. Nothing is prettified into a different
//! tree: `\exp x` stays `exp(x)` and `e^x` stays `e^x`. The result is then
//! checked by the same parser and analysis that compile it: every line must
//! parse back to the same tree, and the whole system (when it compiles) to
//! the same analysis. A line that fails is kept as typed, with a note.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::ast::{Ast, AstKind, BinOp, Func};
use crate::latex::product_separator;
use crate::parser::{parse_lines, Line};
use crate::sema::{leibniz_to_deriv, same_meaning};
use crate::token::SymName;
use crate::{analyze_source, Diagnostic, InputFormat};

/// One line of [`convert`]'s result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConvertedLine {
    /// The line in the target format, or exactly as typed when it could not
    /// be converted.
    pub source: String,
    /// `false` when the line was kept as typed.
    pub converted: bool,
    /// `true` when the line was kept because it has an error in its own
    /// format: it was broken before the switch, so switching can go ahead.
    /// A line kept although it is valid (`converted` and `error` both
    /// false: the target can't write it exactly) would mean something else,
    /// or nothing, read in the other format, so the UI must not switch.
    #[serde(default)]
    pub error: bool,
    /// Why the line was kept as typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub note: Option<String>,
}

/// Longest number written out digit by digit in LaTeX (which has no
/// exponent notation that reads back as one number).
const MAX_LATEX_NUMBER_LEN: usize = 32;

const HAS_ERROR: &str = "This line has an error, so it was kept as typed";
const NOT_EXACT: &str = "This line can't be converted exactly";
const SYSTEM_CHANGED: &str = "Converting would change what the system means";

/// Rewrite each of `lines` (one UI line each, possibly holding several
/// equations separated by `;`) from `from` to `to`.
///
/// The result has one entry per line. A converted line means exactly what
/// the original meant and keeps every name: it parses to the same tree,
/// and a system that compiled compiles to the same states, parameters,
/// defaults and right-hand sides (the parameters possibly listed in another
/// order: LaTeX `\sin^{a} b` names `a` first, text `sin(b)^a` names `b`
/// first). A line that can't be converted (it has an error, or uses
/// something the target format can't write) is returned as typed with
/// `converted: false` and a note; `error` says which of the two it was,
/// since only a line with an error may be left in the old format by a
/// switch. Empty lines pass through, and a line holding no equation (only
/// spacing such as `\quad`) becomes an empty one.
pub fn convert(lines: &[String], from: InputFormat, to: InputFormat) -> Vec<ConvertedLine> {
    if from == to {
        return lines.iter().map(|l| same(l)).collect();
    }
    let source = join_nonempty(lines.iter().map(String::as_str));
    let mut diags = Vec::new();
    let analysis = analyze_source(&source, from, &mut diags).filter(|_| no_errors(&diags));
    let scope = match &analysis {
        Some(a) => Scope {
            states: a.info.states.iter().map(|s| s.name.clone()).collect(),
            ode: a.info.kind == dyn_core::SystemKind::Ode,
        },
        None => Scope::from_targets(lines, from),
    };
    let mut writer = Writer {
        to,
        names: HashMap::new(),
    };
    let out: Vec<ConvertedLine> = lines
        .iter()
        .map(|l| match convert_line(l, from, &scope, &mut writer) {
            Ok(Some(s)) => ConvertedLine {
                source: s,
                converted: true,
                error: false,
                note: None,
            },
            Ok(None) => same(l),
            Err(f) => kept(l, matches!(f, Fail::Error), f.note(to)),
        })
        .collect();
    if let Some(a) = &analysis {
        if out.iter().all(|l| l.converted) {
            let mut d2 = Vec::new();
            let target = join_nonempty(out.iter().map(|l| l.source.as_str()));
            let b = analyze_source(&target, to, &mut d2).filter(|_| no_errors(&d2));
            let same = b.is_some_and(|b| same_meaning(a, &b));
            debug_assert!(
                same,
                "converting {source:?} gave {target:?}, which means something else"
            );
            if !same {
                return lines
                    .iter()
                    .map(|l| kept(l, false, SYSTEM_CHANGED.to_string()))
                    .collect();
            }
        }
    }
    out
}

/// A line that needs no conversion (an empty one, or the same format).
fn same(line: &str) -> ConvertedLine {
    ConvertedLine {
        source: line.to_string(),
        converted: true,
        error: false,
        note: None,
    }
}

/// A line kept as typed, with why.
fn kept(line: &str, error: bool, note: String) -> ConvertedLine {
    ConvertedLine {
        source: line.to_string(),
        converted: false,
        error,
        note: Some(note),
    }
}

fn no_errors(diags: &[Diagnostic]) -> bool {
    !diags.iter().any(Diagnostic::is_error)
}

/// The source the UI compiles: its non-empty lines joined with newlines.
fn join_nonempty<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    lines
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Why a line was kept as typed.
#[derive(Debug)]
enum Fail {
    /// It doesn't parse, or uses something that is an error in its format.
    Error,
    /// The target format can't write this name so that it reads back as
    /// the same name.
    Name(String),
    /// A number too long to write out in LaTeX.
    Number(f64),
    /// The written line didn't read back as the same tree.
    NotExact,
}

impl Fail {
    fn note(&self, to: InputFormat) -> String {
        let mode = match to {
            InputFormat::Latex => "LaTeX",
            InputFormat::Text => "Text mode",
        };
        match self {
            Fail::Error => HAS_ERROR.to_string(),
            Fail::Name(n) => format!("{mode} can't write the name {n}"),
            Fail::Number(v) => format!("{mode} can't write the number {v:e} exactly"),
            Fail::NotExact => NOT_EXACT.to_string(),
        }
    }
}

/// What the rewrite needs to know about the system: which names are
/// states (`x(t)` is the state `x`), and whether it is a flow.
struct Scope {
    states: HashSet<String>,
    ode: bool,
}

impl Scope {
    /// For a system that doesn't compile: the variables the lines that
    /// parse give equations for.
    fn from_targets(lines: &[String], from: InputFormat) -> Scope {
        let mut states = HashSet::new();
        let (mut flow, mut map) = (false, false);
        for l in lines {
            let mut d = Vec::new();
            for line in parse_lines(l, from, &mut d) {
                let lhs = leibniz_to_deriv(line.lhs);
                let rhs = leibniz_to_deriv(line.rhs);
                match &lhs.kind {
                    AstKind::Next(s) => {
                        map = true;
                        states.insert(s.name.clone());
                    }
                    AstKind::Apply(s, args) if s.name == "H" => {
                        flow = true;
                        states.extend(args.iter().filter_map(|a| a.plain_symbol()).map(|s| s.name));
                    }
                    _ => {}
                }
                for side in [&lhs, &rhs] {
                    side.walk(&mut |n| {
                        if let AstKind::Deriv(s, _) = &n.kind {
                            flow = true;
                            states.insert(s.name.clone());
                        }
                    });
                }
            }
        }
        Scope {
            states,
            ode: flow || !map,
        }
    }
}

/// The equations of one UI line in `to`, `None` for a blank line. A line
/// with no equation in it, only spacing or separators (`\quad`, `\\`),
/// becomes an empty line: kept as typed it would be an error in text mode.
fn convert_line(
    line: &str,
    from: InputFormat,
    scope: &Scope,
    w: &mut Writer,
) -> Result<Option<String>, Fail> {
    if line.trim().is_empty() {
        return Ok(None);
    }
    let mut diags = Vec::new();
    let parsed = parse_lines(line, from, &mut diags);
    if !diags.is_empty() {
        return Err(Fail::Error);
    }
    if parsed.is_empty() {
        return Ok(Some(String::new()));
    }
    let eqs = parsed
        .into_iter()
        .map(|l| normalize(l, scope, from))
        .collect::<Result<Vec<_>, _>>()?;
    let mut parts = Vec::with_capacity(eqs.len());
    for (lhs, rhs) in &eqs {
        parts.push(format!("{} = {}", w.write(lhs)?.s, w.write(rhs)?.s));
    }
    let out = parts.join("; ");
    if !reads_back(&out, &eqs, scope, w.to) {
        return Err(Fail::NotExact);
    }
    Ok(Some(out))
}

/// `out`, read in `to`, gives the equations `eqs`.
fn reads_back(out: &str, eqs: &[(Ast, Ast)], scope: &Scope, to: InputFormat) -> bool {
    let mut diags = Vec::new();
    let lines = parse_lines(out, to, &mut diags);
    diags.is_empty()
        && lines.len() == eqs.len()
        && lines.into_iter().zip(eqs).all(|(l, (a, b))| {
            normalize(l, scope, to).is_ok_and(|(x, y)| same_shape(&x, a) && same_shape(&y, b))
        })
}

/// One equation, with every construct the two formats read differently
/// rewritten to what it means in `fmt`.
fn normalize(line: Line, scope: &Scope, fmt: InputFormat) -> Result<(Ast, Ast), Fail> {
    let lhs = leibniz_to_deriv(line.lhs);
    let lhs = match lhs.kind {
        // `H(q, p) = …` names a Hamiltonian's variables; it is the same in both.
        AstKind::Apply(s, args) if s.name == "H" => {
            let args = args
                .into_iter()
                .map(|a| rewrite(a, scope, fmt))
                .collect::<Result<_, _>>()?;
            Ast::new(AstKind::Apply(s, args), lhs.span)
        }
        kind => rewrite(Ast::new(kind, lhs.span), scope, fmt)?,
    };
    let rhs = rewrite(leibniz_to_deriv(line.rhs), scope, fmt)?;
    Ok((lhs, rhs))
}

/// `x(t)` is the state `x` in a flow that has one; otherwise LaTeX reads
/// `x(…)` as a product and text mode as an unknown function (an error).
fn rewrite(a: Ast, scope: &Scope, fmt: InputFormat) -> Result<Ast, Fail> {
    let span = a.span;
    let kind = match a.kind {
        AstKind::Apply(s, mut args) => {
            let is_t = args.len() == 1 && matches!(&args[0].kind, AstKind::Sym(t) if t.name == "t");
            if is_t && scope.ode && scope.states.contains(&s.name) {
                AstKind::Sym(s)
            } else if fmt == InputFormat::Latex && args.len() == 1 {
                let arg = rewrite(args.pop().expect("one argument"), scope, fmt)?;
                let name = Ast::new(AstKind::Sym(s), (span.0, span.0));
                AstKind::Bin(BinOp::Mul, Box::new(name), Box::new(arg))
            } else {
                return Err(Fail::Error);
            }
        }
        AstKind::Neg(x) => AstKind::Neg(Box::new(rewrite(*x, scope, fmt)?)),
        AstKind::Bin(op, x, y) => AstKind::Bin(
            op,
            Box::new(rewrite(*x, scope, fmt)?),
            Box::new(rewrite(*y, scope, fmt)?),
        ),
        AstKind::Call(f, args) => AstKind::Call(
            f,
            args.into_iter()
                .map(|x| rewrite(x, scope, fmt))
                .collect::<Result<_, _>>()?,
        ),
        leaf => leaf,
    };
    Ok(Ast::new(kind, span))
}

/// The same tree, ignoring positions: names compared by name, numbers bit
/// for bit.
fn same_shape(a: &Ast, b: &Ast) -> bool {
    use AstKind::*;
    let all = |xs: &[Ast], ys: &[Ast]| {
        xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| same_shape(x, y))
    };
    match (&a.kind, &b.kind) {
        (Num(x), Num(y)) => x.to_bits() == y.to_bits(),
        (Sym(x), Sym(y)) | (Current(x), Current(y)) | (Next(x), Next(y)) => x.name == y.name,
        (Deriv(x, j), Deriv(y, k)) => x.name == y.name && j == k,
        (Neg(x), Neg(y)) => same_shape(x, y),
        (Bin(o, x1, x2), Bin(p, y1, y2)) => o == p && same_shape(x1, y1) && same_shape(x2, y2),
        (Call(f, xs), Call(g, ys)) => f == g && all(xs, ys),
        (Apply(s, xs), Apply(t, ys)) => s.name == t.name && all(xs, ys),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Sum = 1,
    Product = 2,
    Unary = 3,
    Power = 4,
    Atom = 5,
}

/// A written subexpression.
struct Out {
    s: String,
    prec: Prec,
    /// LaTeX: ends in a function argument written without brackets
    /// (`\sin x`), so a factor after it must be set off with `\cdot`.
    open: bool,
}

impl Out {
    fn new(s: String, prec: Prec) -> Out {
        Out {
            s,
            prec,
            open: false,
        }
    }
}

/// A name as LaTeX writes it: base and optional subscript.
#[derive(Clone)]
struct LatexName {
    base: String,
    sub: Option<String>,
}

impl LatexName {
    fn plain(&self) -> String {
        self.with_sub(&self.base)
    }

    fn with_sub(&self, base: &str) -> String {
        match &self.sub {
            Some(s) => format!("{base}_{{{s}}}"),
            None => base.to_string(),
        }
    }

    fn indexed(&self, index: &str) -> String {
        match &self.sub {
            Some(s) => format!("{}_{{{s},{index}}}", self.base),
            None => format!("{}_{{{index}}}", self.base),
        }
    }
}

struct Writer {
    to: InputFormat,
    /// LaTeX spelling of each name, `None` when it has none.
    names: HashMap<String, Option<LatexName>>,
}

impl Writer {
    fn write(&mut self, a: &Ast) -> Result<Out, Fail> {
        match self.to {
            InputFormat::Text => self.text(a),
            InputFormat::Latex => self.latex(a),
        }
    }

    // ---- text ----

    fn text(&mut self, a: &Ast) -> Result<Out, Fail> {
        Ok(match &a.kind {
            AstKind::Num(v) => Out::new(text_number(*v)?, Prec::Atom),
            AstKind::Sym(s) => Out::new(s.name.clone(), Prec::Atom),
            AstKind::Current(s) => Out::new(format!("{}_n", s.name), Prec::Atom),
            AstKind::Next(s) => Out::new(format!("{}_{{n+1}}", s.name), Prec::Atom),
            AstKind::Deriv(s, k) => Out::new(s.derived_name(*k), Prec::Atom),
            AstKind::Neg(x) => {
                let x = self.text(x)?;
                Out::new(format!("-{}", text_at_least(&x, Prec::Power)), Prec::Unary)
            }
            AstKind::Bin(op, x, y) => {
                let (l, r) = (self.text(x)?, self.text(y)?);
                match op {
                    BinOp::Add | BinOp::Sub => {
                        let sign = if *op == BinOp::Add { "+" } else { "-" };
                        let l = text_at_least(&l, Prec::Sum);
                        let r = text_right(&r, Prec::Product);
                        Out::new(format!("{l} {sign} {r}"), Prec::Sum)
                    }
                    BinOp::Mul | BinOp::Div => {
                        let sign = if *op == BinOp::Mul { "*" } else { "/" };
                        let l = text_at_least(&l, Prec::Product);
                        let r = text_right(&r, Prec::Power);
                        Out::new(format!("{l}{sign}{r}"), Prec::Product)
                    }
                    BinOp::Pow => {
                        let base = text_at_least(&l, Prec::Atom);
                        let simple = r.prec == Prec::Atom
                            && !(matches!(y.kind, AstKind::Num(_)) && r.s.contains('e'));
                        let e = if simple { r.s } else { format!("({})", r.s) };
                        Out::new(format!("{base}^{e}"), Prec::Power)
                    }
                }
            }
            AstKind::Call(f, args) => {
                let args = args
                    .iter()
                    .map(|x| self.text(x).map(|o| o.s))
                    .collect::<Result<Vec<_>, _>>()?;
                let s = match f {
                    Func::OddRoot(n) => format!("root({}, {n})", args[0]),
                    f => format!("{}({})", f.text_name(), args.join(", ")),
                };
                Out::new(s, Prec::Atom)
            }
            AstKind::Apply(s, args) => {
                let args = args
                    .iter()
                    .map(|x| self.text(x).map(|o| o.s))
                    .collect::<Result<Vec<_>, _>>()?;
                Out::new(format!("{}({})", s.name, args.join(", ")), Prec::Atom)
            }
        })
    }

    // ---- LaTeX ----

    /// How LaTeX writes `s` so that it reads back as the same name: its
    /// usual form (`x_{1}`, `\omega`, `k_{\mathrm{on}}`) when that reads
    /// back as `s`, else `\mathrm{name}` (text mode's `x1` and `omega0`,
    /// which would otherwise read back as `x_1` and `omega_0`).
    fn latex_name(&mut self, s: &SymName) -> Result<LatexName, Fail> {
        let found = self
            .names
            .entry(s.name.clone())
            .or_insert_with(|| {
                spellings(s)
                    .into_iter()
                    .find(|n| latex_reads_as(&n.plain(), &s.name))
            })
            .clone();
        found.ok_or_else(|| Fail::Name(s.name.clone()))
    }

    fn latex(&mut self, a: &Ast) -> Result<Out, Fail> {
        Ok(match &a.kind {
            AstKind::Num(v) => Out::new(latex_number(*v)?, Prec::Atom),
            AstKind::Sym(s) => Out::new(self.latex_name(s)?.plain(), Prec::Atom),
            AstKind::Current(s) => Out::new(self.latex_name(s)?.indexed("n"), Prec::Atom),
            AstKind::Next(s) => Out::new(self.latex_name(s)?.indexed("n+1"), Prec::Atom),
            AstKind::Deriv(s, k) => Out::new(latex_deriv(s, *k)?, Prec::Atom),
            AstKind::Neg(x) => {
                let x = self.latex(x)?;
                Out {
                    s: format!("-{}", latex_at_least(&x, Prec::Power)),
                    prec: Prec::Unary,
                    open: x.open,
                }
            }
            AstKind::Bin(op, x, y) => self.latex_bin(*op, x, y)?,
            AstKind::Call(f, args) => self.latex_call(*f, args)?,
            AstKind::Apply(s, args) => {
                let name = self.latex_name(s)?.plain();
                let args = args
                    .iter()
                    .map(|x| self.latex(x).map(|o| o.s))
                    .collect::<Result<Vec<_>, _>>()?;
                Out::new(
                    format!("{name}\\left({}\\right)", args.join(", ")),
                    Prec::Atom,
                )
            }
        })
    }

    fn latex_bin(&mut self, op: BinOp, x: &Ast, y: &Ast) -> Result<Out, Fail> {
        if op == BinOp::Pow {
            if let Some(n) = even_root_index(y) {
                let a = self.latex(x)?;
                return Ok(Out::new(format!("\\sqrt[{n}]{{{}}}", a.s), Prec::Atom));
            }
        }
        let (l, r) = (self.latex(x)?, self.latex(y)?);
        Ok(match op {
            BinOp::Add | BinOp::Sub => {
                let sign = if op == BinOp::Add { "+" } else { "-" };
                let ls = latex_at_least(&l, Prec::Sum);
                let rs = latex_right(&r, Prec::Product);
                Out::new(format!("{ls} {sign} {rs}"), Prec::Sum)
            }
            BinOp::Mul => {
                let ls = latex_at_least(&l, Prec::Product);
                let rs = latex_right(&r, Prec::Power);
                let sep = product_separator(&ls, &rs, l.open && l.prec >= Prec::Product);
                let open = r.open && rs == r.s;
                Out {
                    s: format!("{ls}{sep}{rs}"),
                    prec: Prec::Product,
                    open,
                }
            }
            BinOp::Div => Out::new(format!("\\frac{{{}}}{{{}}}", l.s, r.s), Prec::Power),
            BinOp::Pow => {
                let base = latex_at_least(&l, Prec::Atom);
                Out::new(format!("{base}^{{{}}}", r.s), Prec::Power)
            }
        })
    }

    fn latex_call(&mut self, f: Func, args: &[Ast]) -> Result<Out, Fail> {
        let a = self.latex(&args[0])?;
        let paren = |s: &str| format!("\\left({s}\\right)");
        Ok(match f {
            Func::Sqrt => Out::new(format!("\\sqrt{{{}}}", a.s), Prec::Atom),
            Func::Cbrt => Out::new(format!("\\sqrt[3]{{{}}}", a.s), Prec::Atom),
            Func::OddRoot(n) => Out::new(format!("\\sqrt[{n}]{{{}}}", a.s), Prec::Atom),
            Func::Abs => Out::new(format!("\\left|{}\\right|", a.s), Prec::Atom),
            Func::Exp => Out::new(format!("\\exp{}", paren(&a.s)), Prec::Atom),
            Func::Sign => Out::new(format!("\\operatorname{{sign}}{}", paren(&a.s)), Prec::Atom),
            Func::Min | Func::Max => {
                let b = self.latex(&args[1])?;
                Out::new(
                    format!("{}{}", f.latex_name(), paren(&format!("{}, {}", a.s, b.s))),
                    Prec::Atom,
                )
            }
            _ => {
                let name = f.latex_name();
                if a.prec == Prec::Atom && !a.s.starts_with("\\left") {
                    let sep = if a.s.starts_with('\\') { "" } else { " " };
                    Out {
                        s: format!("{name}{sep}{}", a.s),
                        prec: Prec::Power,
                        open: true,
                    }
                } else {
                    Out::new(format!("{name}{}", paren(&a.s)), Prec::Atom)
                }
            }
        })
    }
}

/// `n` when `e` is `1/n` for an even whole `n >= 4`: `\sqrt[n]{x}` reads
/// as `x^(1/n)`.
fn even_root_index(e: &Ast) -> Option<u64> {
    let AstKind::Bin(BinOp::Div, one, n) = &e.kind else {
        return None;
    };
    match (&one.kind, &n.kind) {
        (AstKind::Num(o), AstKind::Num(n))
            if *o == 1.0 && *n >= 4.0 && n % 2.0 == 0.0 && *n < 1e15 =>
        {
            Some(*n as u64)
        }
        _ => None,
    }
}

fn text_at_least(o: &Out, p: Prec) -> String {
    if o.prec >= p {
        o.s.clone()
    } else {
        format!("({})", o.s)
    }
}

/// A right operand: also bracketed when it starts with a minus sign, which
/// reads the same but is easy to misread (`a - -b`).
fn text_right(o: &Out, p: Prec) -> String {
    if o.prec >= p && !o.s.starts_with('-') {
        o.s.clone()
    } else {
        format!("({})", o.s)
    }
}

fn latex_at_least(o: &Out, p: Prec) -> String {
    if o.prec >= p {
        o.s.clone()
    } else {
        format!("\\left({}\\right)", o.s)
    }
}

fn latex_right(o: &Out, p: Prec) -> String {
    if o.prec >= p && !o.s.starts_with('-') {
        o.s.clone()
    } else {
        format!("\\left({}\\right)", o.s)
    }
}

/// A number as text mode reads it back exactly: plain decimals, or
/// exponent notation when that is clearly shorter (`1e-7`, `6.674e-11`).
fn text_number(v: f64) -> Result<String, Fail> {
    if !v.is_finite() || v < 0.0 {
        return Err(Fail::Number(v));
    }
    let plain = format!("{v}");
    let exp = format!("{v:e}");
    Ok(if exp.len() + 3 <= plain.len() {
        exp
    } else {
        plain
    })
}

/// A number as LaTeX reads it back exactly: digits only, since `10^{k}`
/// would be a power, not a number.
fn latex_number(v: f64) -> Result<String, Fail> {
    let s = format!("{v}");
    if !v.is_finite() || v < 0.0 || s.len() > MAX_LATEX_NUMBER_LEN {
        return Err(Fail::Number(v));
    }
    Ok(s)
}

/// The ways LaTeX can spell the name `s`, in order of preference: its
/// usual form (`x_{1}`, `\omega`, `k_{\mathrm{on}}`), then `\mathrm{name}`.
fn spellings(s: &SymName) -> [LatexName; 2] {
    [
        LatexName {
            base: s.base_latex.clone(),
            sub: s.sub_latex.clone(),
        },
        LatexName {
            base: format!("\\mathrm{{{}}}", s.name),
            sub: None,
        },
    ]
}

/// `latex`, read as LaTeX, is the plain variable named `name`.
fn latex_reads_as(latex: &str, name: &str) -> bool {
    latex_lhs_is(
        latex,
        |a| matches!(&a.kind, AstKind::Sym(s) if s.name == name),
    )
}

/// `latex`, read as the left-hand side of a LaTeX equation, satisfies `is`.
fn latex_lhs_is(latex: &str, is: impl Fn(&Ast) -> bool) -> bool {
    let mut d = Vec::new();
    let lines = parse_lines(&format!("{latex}=0"), InputFormat::Latex, &mut d);
    d.is_empty() && lines.len() == 1 && is(&lines[0].lhs)
}

/// The `k`-th derivative of `s` as LaTeX writes it (`\dot{x}_{1}`,
/// `\ddot{\theta}`, `x'''`), in the first spelling of the name that reads
/// back as that derivative. A name's plain spelling can read as something
/// else where its derivative's doesn't: `x_{n}` is a map's `x` at step `n`,
/// but `\dot{x}_{n}` is the derivative of the state `x_n`.
fn latex_deriv(s: &SymName, k: u32) -> Result<String, Fail> {
    spellings(s)
        .iter()
        .map(|n| match k {
            1 => n.with_sub(&format!("\\dot{{{}}}", n.base)),
            2 => n.with_sub(&format!("\\ddot{{{}}}", n.base)),
            k => format!("{}{}", n.plain(), "'".repeat(k as usize)),
        })
        .find(|d| {
            latex_lhs_is(
                d,
                |a| matches!(&a.kind, AstKind::Deriv(t, j) if t.name == s.name && *j == k),
            )
        })
        .ok_or_else(|| Fail::Name(s.name.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str, from: InputFormat, to: InputFormat) -> ConvertedLine {
        convert(&[src.to_string()], from, to).remove(0)
    }

    #[test]
    fn the_same_system_may_list_its_parameters_in_another_order() {
        let analysis = |src: &str, f| analyze_source(src, f, &mut Vec::new()).expect("compiles");
        let latex = analysis("\\dot{y}=\\sin^{a}b", InputFormat::Latex);
        let text = |src: &str| analysis(src, InputFormat::Text);
        assert!(same_meaning(&latex, &text("y' = sin(b)^a")));
        assert!(!same_meaning(&latex, &text("y' = sin(a)^b")));
        assert!(!same_meaning(&latex, &text("y' = sin(b)^c")));
        assert!(!same_meaning(&latex, &text("y' = sin(b)^a; a = 2")));
    }

    #[test]
    fn same_format_is_unchanged() {
        let l = one("\\dot{x}=y", InputFormat::Latex, InputFormat::Latex);
        assert_eq!(l.source, "\\dot{x}=y");
        assert!(l.converted);
    }

    #[test]
    fn numbers_read_back_exactly() {
        for v in [
            0.1,
            1e-7,
            6.674e-11,
            1e300,
            2.5,
            1000.0,
            1e21,
            0.30000000000000004,
        ] {
            let t = text_number(v).unwrap();
            assert_eq!(t.parse::<f64>().unwrap().to_bits(), v.to_bits(), "{t}");
        }
        assert_eq!(text_number(1000.0).unwrap(), "1000");
        assert_eq!(text_number(1e-7).unwrap(), "1e-7");
        assert_eq!(latex_number(1e-7).unwrap(), "0.0000001");
        assert!(latex_number(1e300).is_err());
    }
}
