use crate::token::{Span, SymName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Func {
    Sin,
    Cos,
    Tan,
    Exp,
    Ln,
    Sqrt,
    Cbrt,
    Abs,
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

pub(crate) const FUNCTION_WORDS: &[&str] = &[
    "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "sqrt", "cbrt", "sign", "asin", "acos",
    "atan", "sin", "cos", "tan", "exp", "log", "abs", "sgn", "min", "max", "ln",
];

impl Func {
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

    pub(crate) fn arity(self) -> usize {
        match self {
            Func::Min | Func::Max => 2,
            _ => 1,
        }
    }

    /// `f(a, b) = f(b, a)`, so operand order can be normalised.
    pub(crate) fn is_commutative(self) -> bool {
        matches!(self, Func::Min | Func::Max)
    }

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

    pub(crate) fn latex_name(self) -> String {
        match self {
            Func::Sign => "\\operatorname{sign}".to_string(),
            Func::Cbrt => "\\operatorname{cbrt}".to_string(),
            Func::Abs => "\\operatorname{abs}".to_string(),
            f => format!("\\{}", f.text_name()),
        }
    }

    pub(crate) fn inverse(self) -> Option<Func> {
        match self {
            Func::Sin => Some(Func::Asin),
            Func::Cos => Some(Func::Acos),
            Func::Tan => Some(Func::Atan),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone)]
pub(crate) struct Ast {
    pub kind: AstKind,
    pub span: Span,
    pub height: u32,
}

pub(crate) const MAX_HEIGHT: u32 = 256;

#[derive(Debug, Clone)]
pub(crate) enum AstKind {
    Num(f64),
    Sym(SymName),
    /// `x_n` or `x_{1,n}`: the current iterate of the state named here
    /// (`x`, `x_1`). Outside a map it is the plain symbol `x_n`.
    Current(SymName),
    Deriv(SymName, u32),
    Next(SymName),
    Neg(Box<Ast>),
    Bin(BinOp, Box<Ast>, Box<Ast>),
    Call(Func, Vec<Ast>),
    Apply(SymName, Vec<Ast>),
}

impl Ast {
    pub(crate) fn new(kind: AstKind, span: Span) -> Ast {
        let below = match &kind {
            AstKind::Num(_)
            | AstKind::Sym(_)
            | AstKind::Current(_)
            | AstKind::Deriv(..)
            | AstKind::Next(_) => 0,
            AstKind::Neg(a) => a.height,
            AstKind::Bin(_, a, b) => a.height.max(b.height),
            AstKind::Call(_, args) | AstKind::Apply(_, args) => {
                args.iter().map(|a| a.height).max().unwrap_or(0)
            }
        };
        Ast {
            kind,
            span,
            height: below + 1,
        }
    }

    pub(crate) fn bin(op: BinOp, a: Ast, b: Ast) -> Ast {
        let span = (a.span.0.min(b.span.0), a.span.1.max(b.span.1));
        Ast::new(AstKind::Bin(op, Box::new(a), Box::new(b)), span)
    }

    /// The name this node spells when read as a plain variable: `x` for
    /// `x`, and `x_n` for the current iterate `x_n`.
    pub(crate) fn plain_symbol(&self) -> Option<SymName> {
        match &self.kind {
            AstKind::Sym(s) => Some(s.clone()),
            AstKind::Current(s) => Some(s.at_index_n()),
            _ => None,
        }
    }

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
            AstKind::Num(_)
            | AstKind::Sym(_)
            | AstKind::Current(_)
            | AstKind::Deriv(..)
            | AstKind::Next(_) => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    Num(f64),
    Pi,
    E,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_words_are_longest_first_so_prefix_matching_picks_the_full_name() {
        assert!(FUNCTION_WORDS.windows(2).all(|w| w[0].len() >= w[1].len()));
    }

    #[test]
    fn every_function_word_names_a_function() {
        assert!(FUNCTION_WORDS.iter().all(|w| Func::from_name(w).is_some()));
    }
}
