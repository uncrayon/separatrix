use crate::ast::{Ast, AstKind, BinOp, Func, MAX_HEIGHT};
use crate::diag::Diagnostic;
use crate::token::{sym_from_word, tokenize, Delim, Span, SymName, Tok, Token};
use crate::InputFormat;

#[derive(Debug, Clone)]
pub(crate) struct Line {
    pub lhs: Ast,
    pub rhs: Ast,
    pub span: Span,
}

type PResult<T> = Result<T, Diagnostic>;

const MAX_BRACKET_DEPTH: u32 = 50;
const LEVELS_PER_BRACKET: u32 = 2;
const MAX_NESTING: u32 = MAX_BRACKET_DEPTH * LEVELS_PER_BRACKET;

fn err<T>(msg: impl Into<String>, span: Span) -> PResult<T> {
    Err(Diagnostic::error(msg, Some(span)))
}

pub(crate) fn parse_lines(
    src: &str,
    format: InputFormat,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Line> {
    let toks = tokenize(src, format);
    let mut lines = Vec::new();
    for group in toks.split(|t| matches!(t.tok, Tok::Sep)) {
        if group.is_empty() {
            continue;
        }
        if let Some((msg, span)) = group.iter().find_map(|t| match &t.tok {
            Tok::Bad(m) => Some((m.clone(), t.span)),
            _ => None,
        }) {
            diags.push(Diagnostic::error(msg, Some(span)));
            continue;
        }
        let mut p = Parser {
            src,
            toks: group.to_vec(),
            pos: 0,
            text: format == InputFormat::Text,
            bar_closes_abs: false,
            depth: 0,
        };
        match p.parse_line() {
            Ok(line) => lines.push(line),
            Err(d) => diags.push(d),
        }
    }
    lines
}

struct Parser<'s> {
    src: &'s str,
    toks: Vec<Token>,
    pos: usize,
    text: bool,
    bar_closes_abs: bool,
    depth: u32,
}

fn is_n_token(t: &Token) -> bool {
    matches!(&t.tok, Tok::Ident(s) if s.name == "n")
}

fn is_next_index(parts: &[Token]) -> bool {
    let is_one = |t: &Token| matches!(t.tok, Tok::Num(v) if v == 1.0);
    parts.len() == 3
        && matches!(parts[1].tok, Tok::Plus)
        && ((is_n_token(&parts[0]) && is_one(&parts[2]))
            || (is_one(&parts[0]) && is_n_token(&parts[2])))
}

fn example(text: bool) -> &'static str {
    if text {
        "x' = -x"
    } else {
        "\\dot{x} = -x"
    }
}

impl<'s> Parser<'s> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }

    fn peek_at(&self, k: usize) -> Option<&Tok> {
        self.toks.get(self.pos + k).map(|t| &t.tok)
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }

    fn prev_end(&self) -> usize {
        self.toks[self.pos - 1].span.1
    }

    fn line_end(&self) -> usize {
        self.toks.last().map_or(0, |t| t.span.1)
    }

    fn text_of(&self, span: Span) -> &'s str {
        &self.src[span.0..span.1]
    }

    #[inline(never)]
    fn unexpected(&self) -> Diagnostic {
        let Some(t) = self.toks.get(self.pos) else {
            return self.incomplete();
        };
        let s = self.text_of(t.span);
        let msg = match &t.tok {
            Tok::Close(d) => format!("This {} has no matching {}", d.close_str(), d.open_str()),
            Tok::AbsClose => "This \\right| has no matching \\left|".to_string(),
            Tok::Comma if self.is_decimal_comma() => {
                "Decimal commas aren't supported: write 1.5 instead of 1,5".to_string()
            }
            Tok::Comma => {
                "Unexpected comma. Commas only separate function arguments, as in max(a, b)"
                    .to_string()
            }
            Tok::Eq => "Only one '=' per equation. Put each equation on its own line".to_string(),
            Tok::Caret => "Unexpected ^. For a power of a power use braces: x^{2^{3}}".to_string(),
            Tok::Underscore => "A subscript (_) must follow a variable name".to_string(),
            Tok::Prime(_) => "A prime (') must follow a variable name, as in x'".to_string(),
            _ => format!("Unexpected '{s}' here"),
        };
        Diagnostic::error(msg, Some(t.span))
    }

    fn is_decimal_comma(&self) -> bool {
        let (Some(prev), Some(next)) = (
            self.pos.checked_sub(1).and_then(|i| self.toks.get(i)),
            self.toks.get(self.pos + 1),
        ) else {
            return false;
        };
        let comma = &self.toks[self.pos];
        matches!(prev.tok, Tok::Num(_))
            && matches!(next.tok, Tok::Num(_))
            && prev.span.1 == comma.span.0
            && comma.span.1 == next.span.0
    }

    #[inline(never)]
    fn incomplete(&self) -> Diagnostic {
        let end = self.line_end();
        match self.toks.last() {
            Some(t) => {
                let s = self.text_of(t.span);
                let msg = match t.tok {
                    Tok::Eq => "Missing right-hand side after '='".to_string(),
                    Tok::Open(d) => format!("This {} is never closed", d.open_str()),
                    _ => format!("Something is missing after '{s}'"),
                };
                Diagnostic::error(msg, Some(t.span))
            }
            None => Diagnostic::error("Empty equation", Some((end, end))),
        }
    }

    fn nested(&mut self, f: impl FnOnce(&mut Self) -> PResult<Ast>) -> PResult<Ast> {
        if self.depth >= MAX_NESTING {
            let span = self
                .toks
                .get(self.pos)
                .map_or((self.line_end(), self.line_end()), |t| t.span);
            return err(
                format!(
                    "This expression is nested too deeply (brackets, fractions or functions \
                     inside each other about {MAX_BRACKET_DEPTH} levels deep). Simplify it"
                ),
                span,
            );
        }
        self.depth += 1;
        let r = f(self);
        self.depth -= 1;
        self.check_height(r?)
    }

    fn check_height(&self, a: Ast) -> PResult<Ast> {
        if a.height > MAX_HEIGHT {
            return err(
                "This expression is too long. Split it up, for example by naming parts of it \
                 as parameters",
                a.span,
            );
        }
        Ok(a)
    }

    fn parse_line(&mut self) -> PResult<Line> {
        let span = (self.toks[0].span.0, self.line_end());
        if matches!(self.peek(), Some(Tok::Eq)) {
            return err("Missing left-hand side before '='", self.toks[0].span);
        }
        let lhs = self.parse_expr()?;
        match self.peek() {
            Some(Tok::Eq) => {}
            None => {
                return err(
                    format!(
                        "This line has no '=' sign. Write an equation such as {}",
                        example(self.text)
                    ),
                    span,
                )
            }
            Some(_) => return Err(self.unexpected()),
        }
        let eq = self.bump();
        if self.peek().is_none() {
            return err("Missing right-hand side after '='", eq.span);
        }
        let rhs = self.parse_expr()?;
        if self.peek().is_some() {
            return Err(self.unexpected());
        }
        Ok(Line { lhs, rhs, span })
    }

    fn parse_expr(&mut self) -> PResult<Ast> {
        let mut lhs = self.parse_term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_term()?;
            lhs = self.check_height(Ast::bin(op, lhs, rhs))?;
        }
        Ok(lhs)
    }

    fn parse_term(&mut self) -> PResult<Ast> {
        let mut lhs = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(Tok::Star) => {
                    self.bump();
                    let rhs = self.parse_unary()?;
                    lhs = self.check_height(Ast::bin(BinOp::Mul, lhs, rhs))?;
                }
                Some(Tok::Slash) => {
                    self.bump();
                    let rhs = self.parse_unary()?;
                    lhs = self.check_height(Ast::bin(BinOp::Div, lhs, rhs))?;
                }
                _ if self.implicit_next()? => {
                    let rhs = self.parse_power()?;
                    lhs = self.check_height(Ast::bin(BinOp::Mul, lhs, rhs))?;
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn can_start_factor(&self, t: &Tok) -> bool {
        match t {
            Tok::Num(_)
            | Tok::Ident(_)
            | Tok::Func(_)
            | Tok::Open(_)
            | Tok::Frac
            | Tok::Sqrt
            | Tok::Root
            | Tok::Dot(_)
            | Tok::Placeholder
            | Tok::AbsOpen => true,
            Tok::Pipe => !self.bar_closes_abs,
            _ => false,
        }
    }

    #[inline(never)]
    fn implicit_next(&self) -> PResult<bool> {
        let Some(next) = self.peek() else {
            return Ok(false);
        };
        if !self.can_start_factor(next) {
            return Ok(false);
        }
        if !self.text {
            return Ok(true);
        }
        let prev = &self.toks[self.pos - 1];
        let next_tok = &self.toks[self.pos];
        let span = (prev.span.0, next_tok.span.1);
        match (&prev.tok, next) {
            (Tok::Num(_), Tok::Num(_)) => {
                err("Two numbers in a row; put an operator between them", span)
            }
            (Tok::Num(_) | Tok::Close(_), _) => Ok(true),
            _ => err(
                format!(
                    "Missing '*' between {} and {}. In text mode write {}*{}",
                    self.text_of(prev.span),
                    self.text_of(next_tok.span),
                    self.text_of(prev.span),
                    self.text_of(next_tok.span)
                ),
                span,
            ),
        }
    }

    fn parse_unary(&mut self) -> PResult<Ast> {
        self.nested(|p| match p.peek() {
            Some(Tok::Minus) => {
                let m = p.bump();
                let e = p.parse_unary()?;
                let span = (m.span.0, e.span.1);
                Ok(Ast::new(AstKind::Neg(Box::new(e)), span))
            }
            Some(Tok::Plus) => {
                p.bump();
                p.parse_unary()
            }
            _ => p.parse_power(),
        })
    }

    fn parse_power(&mut self) -> PResult<Ast> {
        let base = self.parse_postfix()?;
        if !matches!(self.peek(), Some(Tok::Caret)) {
            return Ok(base);
        }
        self.bump();
        if self.peek().is_none() {
            return Err(self.incomplete());
        }
        let exp = if self.text {
            self.parse_unary()?
        } else {
            self.parse_latex_arg(true, "^")?
        };
        Ok(Ast::bin(BinOp::Pow, base, exp))
    }

    fn caret_primes(&self) -> Option<(u32, usize)> {
        match self.peek_at(1)? {
            Tok::Prime(k) => Some((*k, 2)),
            Tok::Open(Delim::Brace) => {
                let mut order = 0;
                let mut i = 2;
                loop {
                    match self.peek_at(i)? {
                        Tok::Prime(k) => order += k,
                        Tok::Close(Delim::Brace) if order > 0 => return Some((order, i + 1)),
                        _ => return None,
                    }
                    i += 1;
                }
            }
            _ => None,
        }
    }

    fn add_primes(&self, e: Ast, k: u32, end: usize) -> PResult<Ast> {
        let span = (e.span.0, end);
        if let Some(s) = e.plain_symbol() {
            return Ok(Ast::new(AstKind::Deriv(s, k), span));
        }
        match e.kind {
            AstKind::Deriv(s, j) => Ok(Ast::new(AstKind::Deriv(s, j + k), span)),
            _ => err(
                "A prime (') can only follow a variable name, as in x'",
                span,
            ),
        }
    }

    fn paren_t_follows(&self) -> bool {
        matches!(self.peek(), Some(Tok::Open(Delim::Paren)))
            && matches!(self.peek_at(1), Some(Tok::Ident(s)) if s.name == "t")
            && matches!(self.peek_at(2), Some(Tok::Close(Delim::Paren)))
    }

    fn parse_postfix(&mut self) -> PResult<Ast> {
        let mut e = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Tok::Prime(k)) => {
                    let k = *k;
                    let t = self.bump();
                    e = self.add_primes(e, k, t.span.1)?;
                }
                Some(Tok::Caret) => match self.caret_primes() {
                    Some((k, skip)) => {
                        self.pos += skip;
                        e = self.add_primes(e, k, self.prev_end())?;
                    }
                    None => break,
                },
                _ => break,
            }
        }
        if self.paren_t_follows() {
            match (&e.kind, e.plain_symbol()) {
                (AstKind::Deriv(..), _) => {
                    self.pos += 3;
                    e.span.1 = self.prev_end();
                }
                (_, Some(s)) if !self.text => {
                    let t_tok = self.toks[self.pos + 1].clone();
                    self.pos += 3;
                    let arg = match t_tok.tok {
                        Tok::Ident(t) => Ast::new(AstKind::Sym(t), t_tok.span),
                        _ => unreachable!("checked by paren_t_follows"),
                    };
                    e = Ast::new(AstKind::Apply(s, vec![arg]), (e.span.0, self.prev_end()));
                }
                _ => {}
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> PResult<Ast> {
        self.nested(Self::parse_primary_inner)
    }

    fn parse_primary_inner(&mut self) -> PResult<Ast> {
        let Some(tok) = self.peek().cloned() else {
            return Err(self.incomplete());
        };
        match tok {
            Tok::Num(v) => {
                let t = self.bump();
                Ok(Ast::new(AstKind::Num(v), t.span))
            }
            Tok::Ident(s) => {
                let t = self.bump();
                self.parse_symbol(s, t.span)
            }
            Tok::Func(f) => {
                let t = self.bump();
                self.parse_function(f, t.span)
            }
            Tok::Open(d) => {
                let open = self.bump();
                let mut args = self.parse_args(d, open.span, false)?;
                let mut e = args.pop().expect("parse_args returns at least one");
                e.span = (open.span.0, self.prev_end());
                Ok(e)
            }
            Tok::Pipe | Tok::AbsOpen => self.parse_abs(),
            Tok::Frac => {
                let t = self.bump();
                let a = self.parse_latex_arg(false, "\\frac")?;
                let b = self.parse_latex_arg(false, "\\frac")?;
                let span = (t.span.0, self.prev_end());
                Ok(Ast::new(
                    AstKind::Bin(BinOp::Div, Box::new(a), Box::new(b)),
                    span,
                ))
            }
            Tok::Sqrt => self.parse_sqrt(),
            Tok::Root => self.parse_root(),
            Tok::Dot(k) => self.parse_dot(k),
            Tok::Placeholder => err("Fill in the empty box", self.toks[self.pos].span),
            _ => Err(self.unexpected()),
        }
    }

    #[inline(never)]
    fn parse_symbol(&mut self, s: SymName, span: Span) -> PResult<Ast> {
        let e = match self.text_current_iterate(&s) {
            Some(state) => Ast::new(AstKind::Current(state), span),
            None => self.parse_subscript(s, span)?,
        };
        if let Some(s) = e.plain_symbol() {
            if (self.text || s.name == "H") && matches!(self.peek(), Some(Tok::Open(Delim::Paren)))
            {
                let open = self.bump();
                let args = self.parse_args(Delim::Paren, open.span, true)?;
                return Ok(Ast::new(AstKind::Apply(s, args), (span.0, self.prev_end())));
            }
        }
        Ok(e)
    }

    /// Text mode reads a word such as `x_n` (or `x_1_n`) as one token, so
    /// messages quote it whole; it is the current iterate of `x` (`x_1`),
    /// as `x_n` is in LaTeX. A function name keeps the word (`sin_n`).
    fn text_current_iterate(&self, s: &SymName) -> Option<SymName> {
        let base = s
            .name
            .strip_suffix("_n")
            .filter(|b| self.text && !b.is_empty())?;
        Func::from_name(base).is_none().then(|| sym_from_word(base))
    }

    #[inline(never)]
    fn parse_subscript(&mut self, s: SymName, span: Span) -> PResult<Ast> {
        if !matches!(self.peek(), Some(Tok::Underscore)) {
            return Ok(Ast::new(AstKind::Sym(s), span));
        }
        let us = self.bump();
        let parts: Vec<Token> = match self.peek() {
            Some(Tok::Open(Delim::Brace)) => {
                let open = self.bump();
                let mut inner = Vec::new();
                loop {
                    match self.peek() {
                        None => return err("This { is never closed", open.span),
                        Some(Tok::Close(Delim::Brace)) => {
                            self.bump();
                            break;
                        }
                        Some(_) => inner.push(self.bump()),
                    }
                }
                inner
            }
            Some(Tok::Num(_) | Tok::Ident(_)) => vec![self.bump()],
            _ => return err("A subscript needs something after _, as in x_1", us.span),
        };
        let full = (span.0, self.prev_end());
        if is_next_index(&parts) {
            return Ok(Ast::new(AstKind::Next(s), full));
        }
        if let [index] = parts.as_slice() {
            if is_n_token(index) {
                return Ok(Ast::new(AstKind::Current(s), full));
            }
        }
        if s.sub_latex.is_some() {
            return err("Only one subscript per name is allowed", full);
        }
        if let Some(comma) = parts.iter().position(|t| matches!(t.tok, Tok::Comma)) {
            return self.indexed_iterate(s, &parts[..comma], &parts[comma + 1..], full);
        }
        let (name, latex) = self.subscript_name(&parts, full)?;
        let sym = SymName {
            name: format!("{}_{}", s.name, name),
            base_latex: s.base_latex.clone(),
            sub_latex: Some(latex),
        };
        Ok(Ast::new(AstKind::Sym(sym), full))
    }

    /// `x_{1,n}` (the current iterate of `x_1`) or `x_{1,n+1}` (its next).
    fn indexed_iterate(
        &self,
        s: SymName,
        head: &[Token],
        index: &[Token],
        full: Span,
    ) -> PResult<Ast> {
        let is_n = index.len() == 1 && is_n_token(&index[0]);
        if head.is_empty() || !(is_n || is_next_index(index)) {
            return err(
                "A subscript with a comma is for maps of indexed variables: x_{1,n} or \
                 x_{1,n+1}. Otherwise use a name or number, as in x_1 or k_{12}",
                full,
            );
        }
        let (name, latex) = self.subscript_name(head, full)?;
        let state = SymName {
            name: format!("{}_{name}", s.name),
            base_latex: s.base_latex,
            sub_latex: Some(latex),
        };
        let kind = if is_n {
            AstKind::Current(state)
        } else {
            AstKind::Next(state)
        };
        Ok(Ast::new(kind, full))
    }

    fn subscript_name(&self, parts: &[Token], full: Span) -> PResult<(String, String)> {
        let mut name = String::new();
        let mut latex = String::new();
        for p in parts {
            let (n, l) = match &p.tok {
                Tok::Ident(id) => (id.name.clone(), id.latex()),
                Tok::Num(_) => {
                    let t = self.text_of(p.span).to_string();
                    (t.clone(), t)
                }
                _ => {
                    return err(
                        "This subscript can't be read. Use a name or number (x_1, k_{12}), \
                         or n+1 for maps (x_{n+1})",
                        full,
                    )
                }
            };
            if latex.starts_with('\\') && l.starts_with(|c: char| c.is_ascii_alphabetic()) {
                latex.push(' ');
            }
            name.push_str(&n);
            latex.push_str(&l);
        }
        if name.is_empty() {
            return err("This subscript is empty", full);
        }
        Ok((name, latex))
    }

    fn parse_args(&mut self, d: Delim, open: Span, allow_comma: bool) -> PResult<Vec<Ast>> {
        if matches!(self.peek(), Some(Tok::Close(c)) if *c == d) {
            let close = self.toks[self.pos].span;
            return err(
                format!(
                    "Nothing between {} and {}; fill it in",
                    d.open_str(),
                    d.close_str()
                ),
                (open.0, close.1),
            );
        }
        let saved = self.bar_closes_abs;
        self.bar_closes_abs = false;
        let mut args = vec![self.parse_expr()?];
        while allow_comma && matches!(self.peek(), Some(Tok::Comma)) {
            self.bump();
            args.push(self.parse_expr()?);
        }
        self.bar_closes_abs = saved;
        self.expect_close(d, open)?;
        Ok(args)
    }

    fn expect_close(&mut self, d: Delim, open: Span) -> PResult<()> {
        match self.peek() {
            Some(Tok::Close(c)) if *c == d => {
                self.bump();
                Ok(())
            }
            Some(Tok::Close(c)) => {
                let c = *c;
                let close = self.toks[self.pos].span;
                err(
                    format!(
                        "This {} is closed by {}; they don't match",
                        d.open_str(),
                        c.close_str()
                    ),
                    (open.0, close.1),
                )
            }
            None => err(format!("This {} is never closed", d.open_str()), open),
            Some(_) => Err(self.unexpected()),
        }
    }

    #[inline(never)]
    fn parse_abs(&mut self) -> PResult<Ast> {
        let open = self.bump();
        let explicit = matches!(open.tok, Tok::AbsOpen);
        let saved = self.bar_closes_abs;
        self.bar_closes_abs = !explicit;
        let inner = self.parse_expr()?;
        self.bar_closes_abs = saved;
        let closed = match self.peek() {
            Some(Tok::Pipe) if !explicit => true,
            Some(Tok::AbsClose) if explicit => true,
            None => false,
            Some(_) => return Err(self.unexpected()),
        };
        if !closed {
            return err("This | is never closed", open.span);
        }
        self.bump();
        let span = (open.span.0, self.prev_end());
        Ok(Ast::new(AstKind::Call(Func::Abs, vec![inner]), span))
    }

    fn parse_latex_arg(&mut self, allow_minus: bool, ctx: &str) -> PResult<Ast> {
        self.nested(|p| p.parse_latex_arg_inner(allow_minus, ctx))
    }

    fn parse_latex_arg_inner(&mut self, allow_minus: bool, ctx: &str) -> PResult<Ast> {
        let here = self
            .toks
            .get(self.pos)
            .map_or((self.line_end(), self.line_end()), |t| t.span);
        match self.peek().cloned() {
            Some(Tok::Open(d)) => {
                if matches!(self.peek_at(1), Some(Tok::Close(c)) if *c == d) {
                    let close = self.toks[self.pos + 1].span;
                    return err(
                        format!("The box after {ctx} is empty; fill it in"),
                        (here.0, close.1),
                    );
                }
                let open = self.bump();
                let mut args = self.parse_args(d, open.span, false)?;
                let mut e = args.pop().expect("one argument");
                e.span = (open.span.0, self.prev_end());
                Ok(e)
            }
            Some(Tok::Minus) if allow_minus => {
                let m = self.bump();
                let a = self.parse_latex_arg(false, ctx)?;
                let span = (m.span.0, a.span.1);
                Ok(Ast::new(AstKind::Neg(Box::new(a)), span))
            }
            Some(Tok::Num(v)) => {
                let span = here;
                let text = self.text_of(span);
                if text.len() > 1 && text.bytes().all(|b| b.is_ascii_digit()) {
                    // LaTeX takes one digit: \frac12 = 1/2, x^23 = x^2·3.
                    let first = f64::from(text.as_bytes()[0] - b'0');
                    let rest: f64 = text[1..].parse().unwrap_or(0.0);
                    self.toks[self.pos] = Token {
                        tok: Tok::Num(rest),
                        span: (span.0 + 1, span.1),
                    };
                    Ok(Ast::new(AstKind::Num(first), (span.0, span.0 + 1)))
                } else {
                    self.bump();
                    Ok(Ast::new(AstKind::Num(v), span))
                }
            }
            Some(Tok::Ident(s)) => {
                self.bump();
                Ok(Ast::new(AstKind::Sym(s), here))
            }
            Some(Tok::Placeholder) => err("Fill in the empty box", here),
            Some(Tok::Dot(k)) => self.parse_dot(k),
            Some(Tok::Frac | Tok::Sqrt) => self.parse_primary(),
            None => Err(self.incomplete()),
            Some(_) => err(
                format!("{ctx} needs an argument in braces here, for example {ctx}{{x}}"),
                here,
            ),
        }
    }

    #[inline(never)]
    fn parse_sqrt(&mut self) -> PResult<Ast> {
        let t = self.bump();
        let mut index = 2.0;
        if matches!(self.peek(), Some(Tok::Open(Delim::Bracket))) {
            let open = self.bump();
            let e = self
                .parse_args(Delim::Bracket, open.span, false)?
                .pop()
                .expect("one");
            match e.kind {
                AstKind::Num(v) if v >= 2.0 && v.fract() == 0.0 => index = v,
                _ => {
                    return err(
                        "Only whole-number roots such as \\sqrt[3]{x} are supported",
                        (open.span.0, self.prev_end()),
                    )
                }
            }
        }
        let arg = self.parse_latex_arg(false, "\\sqrt")?;
        let span = (t.span.0, self.prev_end());
        Ok(Ast::new(root_kind(index, arg, span), span))
    }

    /// Text mode's `root(x, n)`: the same tree as `\sqrt[n]{x}`.
    #[inline(never)]
    fn parse_root(&mut self) -> PResult<Ast> {
        let t = self.bump();
        if !matches!(self.peek(), Some(Tok::Open(Delim::Paren))) {
            return err(
                "Write root(x, n) with parentheses, as in root(x, 5) for the fifth root of x",
                t.span,
            );
        }
        let open = self.bump();
        let mut args = self.parse_args(Delim::Paren, open.span, true)?;
        let span = (t.span.0, self.prev_end());
        let (Some(n), Some(arg), None) = (args.pop(), args.pop(), args.pop()) else {
            return err(
                "root takes two arguments: root(x, n) is the n-th root of x, as in root(x, 5)",
                span,
            );
        };
        match n.kind {
            AstKind::Num(v) if v >= 2.0 && v.fract() == 0.0 => {
                Ok(Ast::new(root_kind(v, arg, span), span))
            }
            _ => err(
                "The n in root(x, n) must be a whole number of at least 2, as in root(x, 5)",
                n.span,
            ),
        }
    }

    #[inline(never)]
    fn parse_dot(&mut self, k: u32) -> PResult<Ast> {
        let t = self.bump();
        let need =
            || Diagnostic::error("\\dot needs a variable name, as in \\dot{x}", Some(t.span));
        let sym = match self.peek().cloned() {
            Some(Tok::Open(Delim::Brace)) => {
                let open = self.bump();
                let Some(Tok::Ident(s)) = self.peek().cloned() else {
                    return Err(need());
                };
                let id = self.bump();
                let inner = self.parse_subscript(s, id.span)?;
                self.expect_close(Delim::Brace, open.span)?;
                match inner.plain_symbol() {
                    Some(s) => s,
                    None => return Err(need()),
                }
            }
            Some(Tok::Ident(s)) => {
                self.bump();
                s
            }
            _ => return Err(need()),
        };
        let after = self.parse_subscript(sym, t.span)?;
        let span = (t.span.0, self.prev_end());
        match after.plain_symbol() {
            Some(s) => Ok(Ast::new(AstKind::Deriv(s, k), span)),
            None => err("A derivative can't have the subscript n+1", span),
        }
    }

    #[inline(never)]
    fn parse_function(&mut self, f: Func, fspan: Span) -> PResult<Ast> {
        let fname = if self.text {
            f.text_name().to_string()
        } else {
            f.latex_name()
        };
        let mut fpow = None;
        if !self.text && matches!(self.peek(), Some(Tok::Caret)) {
            self.bump();
            fpow = Some(self.parse_latex_arg(true, "^")?);
        }
        if matches!(self.peek(), Some(Tok::Underscore)) {
            let msg = if f == Func::Ln {
                "Only the natural logarithm is supported: write \\ln x (or \\log x)".to_string()
            } else {
                format!("{fname} can't have a subscript")
            };
            return err(msg, self.toks[self.pos].span);
        }
        let args = match self.peek() {
            Some(Tok::Open(d)) if !(self.text && *d == Delim::Brace) => {
                let d = *d;
                let open = self.bump();
                self.parse_args(d, open.span, true)?
            }
            _ if self.text => {
                return err(
                    format!("Write {fname}(…) with parentheses in text mode"),
                    fspan,
                )
            }
            _ if f.arity() == 2 => {
                return err(
                    format!("{fname} needs two arguments in parentheses: {fname}(a, b)"),
                    fspan,
                )
            }
            None => return err(format!("{fname} needs an argument"), fspan),
            _ => vec![self.parse_func_chain()?],
        };
        let span = (fspan.0, self.prev_end());
        if args.len() != f.arity() {
            let msg = if f.arity() == 1 {
                format!(
                    "{fname} takes one argument. If you meant a decimal number, use a point: 1.5"
                )
            } else {
                format!("{fname} takes two arguments: {fname}(a, b)")
            };
            return err(msg, span);
        }
        let is_minus_one = |a: &Ast| match &a.kind {
            AstKind::Num(v) => *v == -1.0,
            AstKind::Neg(inner) => matches!(inner.kind, AstKind::Num(v) if v == 1.0),
            _ => false,
        };
        let call = match fpow {
            Some(p) if is_minus_one(&p) && f.inverse().is_some() => {
                Ast::new(AstKind::Call(f.inverse().expect("checked"), args), span)
            }
            Some(p) => Ast::bin(BinOp::Pow, Ast::new(AstKind::Call(f, args), span), p),
            None => Ast::new(AstKind::Call(f, args), span),
        };
        Ok(call)
    }

    fn parse_func_chain(&mut self) -> PResult<Ast> {
        self.nested(Self::parse_func_chain_inner)
    }

    fn parse_func_chain_inner(&mut self) -> PResult<Ast> {
        if matches!(self.peek(), Some(Tok::Minus)) {
            let m = self.bump();
            let inner = self.parse_func_chain()?;
            let span = (m.span.0, inner.span.1);
            return Ok(Ast::new(AstKind::Neg(Box::new(inner)), span));
        }
        let mut e = self.parse_power()?;
        while let Some(t) = self.peek() {
            if matches!(t, Tok::Func(_)) || !self.can_start_factor(t) || self.derivative_follows() {
                break;
            }
            let r = self.parse_power()?;
            e = self.check_height(Ast::bin(BinOp::Mul, e, r))?;
        }
        Ok(e)
    }

    /// The offset just past an optional `_x`, `_1` or `_{...}` starting at `i`.
    fn skip_subscript(&self, mut i: usize) -> usize {
        if !matches!(self.peek_at(i), Some(Tok::Underscore)) {
            return i;
        }
        i += 1;
        if matches!(self.peek_at(i), Some(Tok::Open(Delim::Brace))) {
            while !matches!(self.peek_at(i), Some(Tok::Close(Delim::Brace)) | None) {
                i += 1;
            }
        }
        i + 1
    }

    fn derivative_follows(&self) -> bool {
        match self.peek() {
            Some(Tok::Dot(_)) => true,
            Some(Tok::Ident(_)) => {
                let i = self.skip_subscript(1);
                match self.peek_at(i) {
                    Some(Tok::Prime(_)) => true,
                    Some(Tok::Caret) => matches!(
                        (self.peek_at(i + 1), self.peek_at(i + 2)),
                        (Some(Tok::Prime(_)), _)
                            | (Some(Tok::Open(Delim::Brace)), Some(Tok::Prime(_)))
                    ),
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

/// The `index`-th root of `arg`, a whole number of at least 2: `\sqrt`,
/// `\sqrt[3]` and odd roots are real for negative `arg`, even roots are
/// `arg^(1/index)`.
fn root_kind(index: f64, arg: Ast, span: Span) -> AstKind {
    if index == 2.0 {
        AstKind::Call(Func::Sqrt, vec![arg])
    } else if index == 3.0 {
        AstKind::Call(Func::Cbrt, vec![arg])
    } else if index % 2.0 == 1.0 {
        // Odd indices are below 2^53, so the conversion is exact.
        AstKind::Call(Func::OddRoot(index as u64), vec![arg])
    } else {
        let one = Ast::new(AstKind::Num(1.0), span);
        let n = Ast::new(AstKind::Num(index), span);
        AstKind::Bin(
            BinOp::Pow,
            Box::new(arg),
            Box::new(Ast::bin(BinOp::Div, one, n)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sexpr(a: &Ast) -> String {
        match &a.kind {
            AstKind::Num(v) => format!("{v}"),
            AstKind::Sym(s) => s.name.clone(),
            AstKind::Current(s) => format!("{}[n]", s.name),
            AstKind::Deriv(s, k) => format!("{}{}", s.name, "'".repeat(*k as usize)),
            AstKind::Next(s) => format!("{}[n+1]", s.name),
            AstKind::Neg(x) => format!("(- {})", sexpr(x)),
            AstKind::Bin(op, x, y) => {
                let o = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Pow => "^",
                };
                format!("({o} {} {})", sexpr(x), sexpr(y))
            }
            AstKind::Call(f, args) => {
                let inner: Vec<String> = args.iter().map(sexpr).collect();
                let name = match f {
                    Func::OddRoot(n) => format!("root{n}"),
                    f => f.text_name().to_string(),
                };
                format!("({name} {})", inner.join(" "))
            }
            AstKind::Apply(s, args) => {
                let inner: Vec<String> = args.iter().map(sexpr).collect();
                format!("(apply {} {})", s.name, inner.join(" "))
            }
        }
    }

    fn node_count(a: &Ast) -> usize {
        let mut n = 0;
        a.walk(&mut |_| n += 1);
        n
    }

    fn parse_one(src: &str, f: InputFormat) -> (String, String) {
        let mut d = Vec::new();
        let lines = parse_lines(src, f, &mut d);
        assert!(d.is_empty(), "{src}: {d:?}");
        assert_eq!(lines.len(), 1);
        (sexpr(&lines[0].lhs), sexpr(&lines[0].rhs))
    }

    fn rhs(src: &str) -> String {
        parse_one(&format!("y={src}"), InputFormat::Latex).1
    }

    fn rhs_text(src: &str) -> String {
        parse_one(&format!("y={src}"), InputFormat::Text).1
    }

    fn first_error(src: &str, f: InputFormat) -> Diagnostic {
        let mut d = Vec::new();
        parse_lines(src, f, &mut d);
        d.into_iter()
            .next()
            .unwrap_or_else(|| panic!("expected an error for {src}"))
    }

    #[test]
    fn implicit_multiplication_latex() {
        assert_eq!(rhs("2xy"), "(* (* 2 x) y)");
        assert_eq!(rhs("\\sigma\\left(y-x\\right)"), "(* sigma (- y x))");
        assert_eq!(rhs("rx_n\\left(1-x_n\\right)"), "(* (* r x[n]) (- 1 x[n]))");
    }

    #[test]
    fn precedence() {
        assert_eq!(rhs("-x^2"), "(- (^ x 2))");
        assert_eq!(rhs("a-b-c"), "(- (- a b) c)");
        assert_eq!(rhs("a/b\\cdot c"), "(* (/ a b) c)");
        assert_eq!(rhs_text("2^3^2"), "(^ 2 (^ 3 2))");
        assert_eq!(rhs_text("x^-2"), "(^ x (- 2))");
        assert_eq!(rhs("x^{-1}"), "(^ x (- 1))");
        assert_eq!(rhs("x^23"), "(* (^ x 2) 3)");
    }

    #[test]
    fn fractions_and_roots() {
        assert_eq!(rhs("\\frac12"), "(/ 1 2)");
        assert_eq!(rhs("\\frac{p^2}{2}"), "(/ (^ p 2) 2)");
        assert_eq!(rhs("\\frac{1}{2}x"), "(* (/ 1 2) x)");
        assert_eq!(rhs("\\sqrt{x}"), "(sqrt x)");
        assert_eq!(rhs("\\sqrt[3]{x}"), "(cbrt x)");
        assert_eq!(rhs("\\sqrt[4]{x}"), "(^ x (/ 1 4))");
        assert_eq!(rhs("\\sqrt[5]{x}"), "(root5 x)");
        assert_eq!(rhs_text("root(x, 5)"), "(root5 x)");
        assert_eq!(rhs_text("root(x, 3)"), "(cbrt x)");
        assert_eq!(rhs_text("root(x, 4)"), "(^ x (/ 1 4))");
        assert_eq!(rhs_text("2root(x+1, 2)"), "(* 2 (sqrt (+ x 1)))");
        for bad in [
            "root x",
            "root(x)",
            "root(x, 1)",
            "root(x, n)",
            "root(x, 2.5)",
        ] {
            let e = first_error(&format!("y={bad}"), InputFormat::Text);
            assert!(e.message.contains("root"), "{bad}: {e:?}");
        }
    }

    #[test]
    fn nested_odd_roots_stay_linear_in_size() {
        let k = 30;
        let src = format!("y={}x{}", r"\sqrt[5]{".repeat(k), "}".repeat(k));
        let mut d = Vec::new();
        let lines = parse_lines(&src, InputFormat::Latex, &mut d);
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(node_count(&lines[0].rhs), k + 1);
    }

    #[test]
    fn functions_latex() {
        assert_eq!(rhs("\\sin\\theta"), "(sin theta)");
        assert_eq!(rhs("\\sin 2x"), "(sin (* 2 x))");
        assert_eq!(rhs("\\sin x\\cos y"), "(* (sin x) (cos y))");
        assert_eq!(rhs("\\sin^2x"), "(^ (sin x) 2)");
        assert_eq!(rhs("\\sin^{-1}x"), "(arcsin x)");
        assert_eq!(rhs("\\operatorname{sin}\\left(x\\right)"), "(sin x)");
        assert_eq!(rhs("\\max\\left(x,y\\right)"), "(max x y)");
        assert_eq!(rhs("\\sin -x"), "(sin (- x))");
        assert_eq!(rhs("\\exp\\left(-x\\right)+1"), "(+ (exp (- x)) 1)");
    }

    #[test]
    fn absolute_values() {
        assert_eq!(rhs("|x|"), "(abs x)");
        assert_eq!(rhs("|x||y|"), "(* (abs x) (abs y))");
        assert_eq!(rhs("\\left|x-1\\right|"), "(abs (- x 1))");
        assert_eq!(rhs("2|x-y|z"), "(* (* 2 (abs (- x y))) z)");
    }

    #[test]
    fn derivatives_and_subscripts() {
        let lhs = |s: &str| parse_one(&format!("{s}=0"), InputFormat::Latex).0;
        assert_eq!(lhs("\\dot{x}"), "x'");
        assert_eq!(lhs("\\ddot{\\theta}"), "theta''");
        assert_eq!(lhs("x'"), "x'");
        assert_eq!(lhs("x''"), "x''");
        assert_eq!(lhs("x^{\\prime}"), "x'");
        assert_eq!(lhs("x^{\\prime\\prime}"), "x''");
        assert_eq!(lhs("x^\\prime"), "x'");
        assert_eq!(lhs("x^{\\doubleprime}"), "x''");
        assert_eq!(lhs("\\dot{x}_1"), "x_1'");
        assert_eq!(lhs("\\dot{x_{12}}"), "x_12'");
        assert_eq!(lhs("x_{n+1}"), "x[n+1]");
        assert_eq!(lhs("x'(t)"), "x'");
        assert_eq!(
            lhs("\\frac{\\mathrm{d}x}{\\mathrm{d}t}"),
            "(/ (* d x) (* d t))"
        );
        assert_eq!(lhs("H(q,p)"), "(apply H q p)");
    }

    #[test]
    fn text_mode() {
        assert_eq!(rhs_text("sigma*(y - x)"), "(* sigma (- y x))");
        assert_eq!(rhs_text("2x"), "(* 2 x)");
        assert_eq!(rhs_text("2 sin(x)"), "(* 2 (sin x))");
        assert_eq!(rhs_text("(x+1)(x-1)"), "(* (+ x 1) (- x 1))");
        assert_eq!(rhs_text("1.5e-3*x"), "(* 0.0015 x)");
        assert_eq!(rhs_text("f(x)"), "(apply f x)");
        let (l, _) = parse_one("x_{n+1} = r*x_n", InputFormat::Text);
        assert_eq!(l, "x[n+1]");
    }

    #[test]
    fn errors() {
        let e = first_error("\\dot{x}=\\sigma\\left(y-x", InputFormat::Latex);
        assert!(e.message.contains("never closed"), "{e:?}");
        assert_eq!(e.span, Some((14, 20)));
        let e = first_error("x' = x)", InputFormat::Text);
        assert!(e.message.contains("no matching"), "{e:?}");
        let e = first_error("x' = 1,5", InputFormat::Text);
        assert!(e.message.contains("Decimal comma"), "{e:?}");
        let e = first_error("\\dot{x}=1{,}5", InputFormat::Latex);
        assert!(e.message.contains("Decimal comma"), "{e:?}");
        let e = first_error("x' = x y", InputFormat::Text);
        assert!(e.message.contains("Missing '*'"), "{e:?}");
        let e = first_error("x' = sin x", InputFormat::Text);
        assert!(e.message.contains("parentheses"), "{e:?}");
        let e = first_error("\\dot{x}=\\placeholder{}", InputFormat::Latex);
        assert_eq!(e.message, "Fill in the empty box");
        assert_eq!(e.span, Some((8, 22)));
        let e = first_error("x' = ", InputFormat::Text);
        assert!(e.message.contains("right-hand side"), "{e:?}");
        let e = first_error("x' + 1", InputFormat::Text);
        assert!(e.message.contains("no '='"), "{e:?}");
        let e = first_error("|x", InputFormat::Latex);
        assert!(e.message.contains("never closed"), "{e:?}");
        let e = first_error("y=(x]", InputFormat::Latex);
        assert!(e.message.contains("don't match"), "{e:?}");
    }

    #[test]
    fn separators() {
        let mut d = Vec::new();
        let lines = parse_lines(
            "\\begin{cases}\\dot{x}=y\\\\\\dot{y}=-x\\end{cases}\n;a=1",
            InputFormat::Latex,
            &mut d,
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(lines.len(), 3);
    }
}
