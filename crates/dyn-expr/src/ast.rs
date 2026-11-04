//! Syntax tree produced by the parser, the resolved expression tree produced
//! by the semantic pass, and the table of built-in functions.

use crate::token::{Span, SymName};

/// A built-in function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Func {
    Sin,
    Cos,
    Tan,
    Exp,
    /// Natural logarithm (`\ln`, `\log`, `ln`, `log`).
    Ln,
    Sqrt,
    /// Real cube root (also valid for negative arguments).
    Cbrt,
    Abs,
    /// `-1`, `0` or `1`.
    Sign,
    Sinh,
    Cosh,
    Tanh,
    Asin,
    Acos,
    Atan,
    Min,
    Max,
}

/// Function names (text mode words and LaTeX command names), longest first
/// so that prefix checks such as "sinhx" find `sinh` before `sin`.
pub(crate) const FUNCTION_WORDS: &[&str] = &[
    "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "sqrt", "cbrt", "sign", "asin", "acos",
    "atan", "sin", "cos", "tan", "exp", "log", "abs", "sgn", "min", "max", "ln",
];

impl Func {
    /// Look up a function by its text-mode / LaTeX name (without backslash).
    pub(crate) fn from_name(name: &str) -> Option<Func> {
        use Func::*;
        Some(match name {
            "sin" => Sin,
            "cos" => Cos,
            "tan" => Tan,
            "exp" => Exp,
            "ln" | "log" => Ln,
            "sqrt" => Sqrt,
            "cbrt" => Cbrt,
            "abs" => Abs,
            "sign" | "sgn" => Sign,
            "sinh" => Sinh,
            "cosh" => Cosh,
            "tanh" => Tanh,
            "arcsin" | "asin" => Asin,
            "arccos" | "acos" => Acos,
            "arctan" | "atan" => Atan,
            "min" => Min,
            "max" => Max,
            _ => return None,
        })
    }

    /// Number of arguments.
    pub(crate) fn arity(self) -> usize {
        match self {
            Func::Min | Func::Max => 2,
            _ => 1,
        }
    }

    /// Name used in text mode and in messages.
    pub(crate) fn text_name(self) -> &'static str {
        use Func::*;
        match self {
            Sin => "sin",
            Cos => "cos",
            Tan => "tan",
            Exp => "exp",
            Ln => "ln",
            Sqrt => "sqrt",
            Cbrt => "cbrt",
            Abs => "abs",
            Sign => "sign",
            Sinh => "sinh",
            Cosh => "cosh",
            Tanh => "tanh",
            Asin => "arcsin",
            Acos => "arccos",
            Atan => "arctan",
            Min => "min",
            Max => "max",
        }
    }

    /// LaTeX command for functions written as `\name x` (operator style).
    pub(crate) fn latex_name(self) -> String {
        match self {
            Func::Sign => "\\operatorname{sign}".to_string(),
            Func::Cbrt => "\\operatorname{cbrt}".to_string(),
            Func::Abs => "\\operatorname{abs}".to_string(),
            f => format!("\\{}", f.text_name()),
        }
    }

    /// `\sin^{-1}` means `\arcsin`, etc.
    pub(crate) fn inverse(self) -> Option<Func> {
        match self {
            Func::Sin => Some(Func::Asin),
            Func::Cos => Some(Func::Acos),
            Func::Tan => Some(Func::Atan),
            _ => None,
        }
    }
}

/// Binary operators, shared by the syntax tree and the resolved tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// A node of the syntax tree, with the byte span it came from.
#[derive(Debug, Clone)]
pub(crate) struct Ast {
    pub kind: AstKind,
    pub span: Span,
    /// Number of levels in this tree (1 for a leaf). Every later pass
    /// recurses over the tree, so the parser keeps this bounded (see
    /// [`MAX_HEIGHT`]).
    pub height: u32,
}

/// Largest tree height the parser accepts. The passes after parsing
/// (semantic analysis, display, compilation, and dropping the tree) are
/// recursive, so this bounds their stack use. 256 levels is far beyond
/// any equation typed by hand (a sum of 250 terms, or brackets nested 50
/// deep) while keeping stack use under 512 KiB in release builds.
pub(crate) const MAX_HEIGHT: u32 = 256;

/// What a syntax-tree node is. Names are not resolved yet: `Sym("x")` may
/// later become a state, a parameter, time or a constant.
#[derive(Debug, Clone)]
pub(crate) enum AstKind {
    Num(f64),
    Sym(SymName),
    /// `\dot{x}` (order 1), `x''` (order 2), ...
    Deriv(SymName, u32),
    /// `x_{n+1}` (left-hand side of a map).
    Next(SymName),
    Neg(Box<Ast>),
    Bin(BinOp, Box<Ast>, Box<Ast>),
    Call(Func, Vec<Ast>),
    /// `name(args)` where `name` is not a built-in function: `H(q, p)`,
    /// `x(t)`, or a text-mode call to an unknown function. The semantic pass
    /// decides what it means.
    Apply(SymName, Vec<Ast>),
}

impl Ast {
    pub(crate) fn new(kind: AstKind, span: Span) -> Ast {
        let below = match &kind {
            AstKind::Num(_) | AstKind::Sym(_) | AstKind::Deriv(..) | AstKind::Next(_) => 0,
            AstKind::Neg(a) => a.height,
            AstKind::Bin(_, a, b) => a.height.max(b.height),
            AstKind::Call(_, args) | AstKind::Apply(_, args) => {
                args.iter().map(|a| a.height).max().unwrap_or(0)
            }
        };
        Ast { kind, span, height: below + 1 }
    }

    pub(crate) fn bin(op: BinOp, a: Ast, b: Ast) -> Ast {
        let span = (a.span.0.min(b.span.0), a.span.1.max(b.span.1));
        Ast::new(AstKind::Bin(op, Box::new(a), Box::new(b)), span)
    }

    /// Calls `f` on this node and every descendant (pre-order).
    pub(crate) fn walk(&self, f: &mut impl FnMut(&Ast)) {
        f(self);
        match &self.kind {
            AstKind::Neg(a) => a.walk(f),
            AstKind::Bin(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            AstKind::Call(_, args) | AstKind::Apply(_, args) => {
                for a in args {
                    a.walk(f);
                }
            }
            AstKind::Num(_) | AstKind::Sym(_) | AstKind::Deriv(..) | AstKind::Next(_) => {}
        }
    }
}

/// Resolved expression: every name is now a state, a parameter, time or a
/// constant. This is what gets printed back as LaTeX and compiled.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    Num(f64),
    Pi,
    E,
    /// `t` for flows, `n` for maps.
    Time,
    State(usize),
    Param(usize),
    Neg(Box<Node>),
    Bin(BinOp, Box<Node>, Box<Node>),
    Call(Func, Vec<Node>),
}

impl Node {
    pub(crate) fn bin(op: BinOp, a: Node, b: Node) -> Node {
        Node::Bin(op, Box::new(a), Box::new(b))
    }

    /// True if [`Node::Time`] occurs anywhere.
    pub(crate) fn uses_time(&self) -> bool {
        match self {
            Node::Time => true,
            Node::Neg(a) => a.uses_time(),
            Node::Bin(_, a, b) => a.uses_time() || b.uses_time(),
            Node::Call(_, args) => args.iter().any(Node::uses_time),
            _ => false,
        }
    }
}
