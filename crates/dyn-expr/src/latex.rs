use crate::ast::{BinOp, Func, Node};

pub(crate) struct Names<'a> {
    pub states: &'a [String],
    pub params: &'a [String],
    pub time: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Sum = 1,
    Product = 2,
    Frac = 3,
    Power = 4,
    Atom = 5,
}

struct Out {
    s: String,
    prec: Prec,
    ends_in_bracketless_arg: bool,
}

impl Out {
    fn new(s: String, prec: Prec) -> Out {
        Out {
            s,
            prec,
            ends_in_bracketless_arg: false,
        }
    }
}

pub(crate) fn to_latex(n: &Node, names: &Names) -> String {
    go(n, names).s
}

fn paren(s: &str) -> String {
    format!("\\left({s}\\right)")
}

fn at_least(o: &Out, p: Prec) -> String {
    if o.prec >= p {
        o.s.clone()
    } else {
        paren(&o.s)
    }
}

pub(crate) fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return if v.is_nan() {
            "\\mathrm{NaN}".into()
        } else {
            "\\infty".into()
        };
    }
    let a = v.abs();
    if v == v.trunc() && a < 1e15 {
        format!("{}", v as i64)
    } else if (1e-4..1e15).contains(&a) {
        format!("{v}")
    } else {
        let s = format!("{v:e}");
        match s.split_once('e') {
            Some((m, e)) => format!("{m} \\times 10^{{{e}}}"),
            None => s,
        }
    }
}

fn go(n: &Node, names: &Names) -> Out {
    match n {
        Node::Num(v) => Out::new(fmt_num(*v), if *v < 0.0 { Prec::Sum } else { Prec::Atom }),
        Node::Pi => Out::new("\\pi".into(), Prec::Atom),
        Node::E => Out::new("e".into(), Prec::Atom),
        Node::Time => Out::new(names.time.into(), Prec::Atom),
        Node::State(i) => Out::new(names.states[*i].clone(), Prec::Atom),
        Node::Param(i) => Out::new(names.params[*i].clone(), Prec::Atom),
        Node::Neg(a) => {
            let a = go(a, names);
            Out {
                s: format!("-{}", at_least(&a, Prec::Product)),
                prec: Prec::Sum,
                ends_in_bracketless_arg: false,
            }
        }
        Node::Bin(op, a, b) => bin(*op, a, b, names),
        Node::Call(f, args) => call(*f, args, names),
    }
}

fn bin(op: BinOp, a: &Node, b: &Node, names: &Names) -> Out {
    match op {
        BinOp::Add => {
            let l = at_least(&go(a, names), Prec::Sum);
            match b {
                Node::Neg(inner) => {
                    let r = at_least(&go(inner, names), Prec::Product);
                    Out::new(format!("{l} - {r}"), Prec::Sum)
                }
                _ => {
                    let r = at_least(&go(b, names), Prec::Sum);
                    Out::new(format!("{l} + {r}"), Prec::Sum)
                }
            }
        }
        BinOp::Sub => {
            let l = at_least(&go(a, names), Prec::Sum);
            let r = at_least(&go(b, names), Prec::Product);
            Out::new(format!("{l} - {r}"), Prec::Sum)
        }
        BinOp::Mul => match a {
            Node::Neg(inner) => neg_product(inner, b, names),
            _ => product(a, b, names),
        },
        BinOp::Div => {
            let num = go(a, names).s;
            let den = go(b, names).s;
            Out::new(format!("\\frac{{{num}}}{{{den}}}"), Prec::Frac)
        }
        BinOp::Pow => {
            let e = go(b, names).s;
            if matches!(a, Node::E) {
                return Out::new(format!("e^{{{e}}}"), Prec::Power);
            }
            let base = at_least(&go(a, names), Prec::Atom);
            Out::new(format!("{base}^{{{e}}}"), Prec::Power)
        }
    }
}

/// `(-a) · b`, written `-a b`.
fn neg_product(inner: &Node, b: &Node, names: &Names) -> Out {
    let o = bin(BinOp::Mul, inner, b, names);
    Out {
        s: format!("-{}", o.s),
        prec: Prec::Sum,
        ends_in_bracketless_arg: o.ends_in_bracketless_arg,
    }
}

fn product(a: &Node, b: &Node, names: &Names) -> Out {
    let lo = go(a, names);
    let ro = go(b, names);
    let l = at_least(&lo, Prec::Product);
    let r = at_least(&ro, Prec::Product);
    let left_open = lo.ends_in_bracketless_arg && lo.prec >= Prec::Product;
    let digit_next = r.starts_with(|c: char| c.is_ascii_digit());
    let mixed_number = l.ends_with(|c: char| c.is_ascii_digit()) && r.starts_with("\\frac");
    let sep = if left_open || digit_next || mixed_number {
        " \\cdot "
    } else {
        " "
    };
    Out {
        s: format!("{l}{sep}{r}"),
        prec: Prec::Product,
        ends_in_bracketless_arg: ro.ends_in_bracketless_arg && ro.prec >= Prec::Product,
    }
}

fn call(f: Func, args: &[Node], names: &Names) -> Out {
    let a = go(&args[0], names);
    match f {
        Func::Sqrt => Out::new(format!("\\sqrt{{{}}}", a.s), Prec::Atom),
        Func::Cbrt => Out::new(format!("\\sqrt[3]{{{}}}", a.s), Prec::Atom),
        Func::OddRoot(n) => Out::new(format!("\\sqrt[{n}]{{{}}}", a.s), Prec::Atom),
        Func::Abs => Out::new(format!("\\left|{}\\right|", a.s), Prec::Atom),
        Func::Exp => Out::new(format!("e^{{{}}}", a.s), Prec::Power),
        Func::Min | Func::Max => {
            let b = go(&args[1], names);
            Out::new(
                format!("{}\\left({}, {}\\right)", f.latex_name(), a.s, b.s),
                Prec::Atom,
            )
        }
        Func::Sign => Out::new(format!("{}{}", f.latex_name(), paren(&a.s)), Prec::Atom),
        _ => {
            let name = f.latex_name();
            if a.prec == Prec::Atom && !a.s.starts_with("\\left") {
                let sep = if a.s.starts_with('\\') { "" } else { " " };
                Out {
                    s: format!("{name}{sep}{}", a.s),
                    prec: Prec::Product,
                    ends_in_bracketless_arg: true,
                }
            } else {
                Out::new(format!("{name}{}", paren(&a.s)), Prec::Atom)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(k: usize) -> Node {
        Node::State(k)
    }
    fn p(k: usize) -> Node {
        Node::Param(k)
    }
    fn b(op: BinOp, x: Node, y: Node) -> Node {
        Node::bin(op, x, y)
    }

    fn show(n: &Node) -> String {
        let states = ["x".to_string(), "y".to_string(), "\\theta".to_string()];
        let params = ["\\sigma".to_string(), "r".to_string()];
        to_latex(
            n,
            &Names {
                states: &states,
                params: &params,
                time: "t",
            },
        )
    }

    #[test]
    fn products_and_sums() {
        let lorenz_x = b(BinOp::Mul, p(0), b(BinOp::Sub, s(1), s(0)));
        assert_eq!(show(&lorenz_x), "\\sigma \\left(y - x\\right)");
        assert_eq!(show(&b(BinOp::Mul, Node::Num(2.0), s(0))), "2 x");
        assert_eq!(show(&b(BinOp::Mul, s(0), Node::Num(2.0))), "x \\cdot 2");
        assert_eq!(
            show(&b(BinOp::Sub, s(0), b(BinOp::Sub, s(1), s(0)))),
            "x - \\left(y - x\\right)"
        );
        assert_eq!(
            show(&b(BinOp::Add, s(0), Node::Neg(Box::new(s(1))))),
            "x - y"
        );
        assert_eq!(
            show(&b(
                BinOp::Mul,
                Node::Num(2.0),
                b(BinOp::Div, s(0), Node::Num(3.0))
            )),
            "2 \\cdot \\frac{x}{3}"
        );
    }

    #[test]
    fn functions_and_powers() {
        assert_eq!(
            show(&Node::Neg(Box::new(Node::Call(Func::Sin, vec![s(2)])))),
            "-\\sin\\theta"
        );
        assert_eq!(
            show(&Node::Call(Func::Cos, vec![b(BinOp::Add, s(0), s(1))])),
            "\\cos\\left(x + y\\right)"
        );
        let sin_x_y = b(BinOp::Mul, Node::Call(Func::Sin, vec![s(0)]), s(1));
        assert_eq!(show(&sin_x_y), "\\sin x \\cdot y");
        let sq = b(
            BinOp::Pow,
            Node::Call(Func::Sin, vec![s(0)]),
            Node::Num(2.0),
        );
        assert_eq!(show(&sq), "\\left(\\sin x\\right)^{2}");
        assert_eq!(
            show(&b(BinOp::Pow, Node::E, Node::Neg(Box::new(s(0))))),
            "e^{-x}"
        );
        assert_eq!(
            show(&b(BinOp::Pow, b(BinOp::Add, s(0), s(1)), Node::Num(2.0))),
            "\\left(x + y\\right)^{2}"
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(fmt_num(10.0), "10");
        assert_eq!(fmt_num(0.25), "0.25");
        assert_eq!(fmt_num(1.5e-7), "1.5 \\times 10^{-7}");
    }
}
