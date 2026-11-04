//! Tokenizers for the two input formats.
//!
//! * **LaTeX** (what MathLive produces): every plain letter is its own
//!   symbol (`xy` is `x·y`), commands such as `\sigma` or `\sin` are single
//!   tokens, and purely visual commands (`\,`, `\left`, `\displaystyle`, …)
//!   are dropped here so the parser never sees them.
//! * **Text**: identifiers are whole words (`sigma`, `x_1`), `**` is a power
//!   and scientific numbers like `1.5e-3` are allowed.
//!
//! Problems found while tokenizing become [`Tok::Bad`] tokens; the parser
//! reports them for the line they are on, so one bad line does not hide
//! errors on other lines.

use crate::ast::{Func, FUNCTION_WORDS};
use crate::InputFormat;

/// Byte range `(start, end)` into the source string.
pub(crate) type Span = (usize, usize);

/// A symbol name together with how to display it.
///
/// `name` is the identity used everywhere ("sigma", "x_1", "theta");
/// the LaTeX form is kept in two parts so that decorations such as dots can
/// go on the base: `\dot{x}_{1}` rather than `\dot{x_{1}}`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SymName {
    pub name: String,
    pub base_latex: String,
    pub sub_latex: Option<String>,
}

impl SymName {
    /// A symbol without subscript.
    pub(crate) fn simple(name: &str, base_latex: &str) -> SymName {
        SymName {
            name: name.to_string(),
            base_latex: base_latex.to_string(),
            sub_latex: None,
        }
    }

    fn with_sub(&self, base: String) -> String {
        match &self.sub_latex {
            Some(s) => format!("{base}_{{{s}}}"),
            None => base,
        }
    }

    /// Display form, e.g. `\theta_{1}`.
    pub(crate) fn latex(&self) -> String {
        self.with_sub(self.base_latex.clone())
    }

    /// Display form of the `k`-th time derivative: `x`, `\dot{x}`,
    /// `\ddot{x}`, `\dddot{x}`, `x^{(4)}`, ...
    pub(crate) fn dotted(&self, k: u32) -> String {
        let b = &self.base_latex;
        match k {
            0 => self.latex(),
            1 => self.with_sub(format!("\\dot{{{b}}}")),
            2 => self.with_sub(format!("\\ddot{{{b}}}")),
            3 => self.with_sub(format!("\\dddot{{{b}}}")),
            _ => format!("{}^{{({k})}}", self.latex()),
        }
    }

    /// Display form with an iterate index: `x_{n}`, `x_{n+1}`, `x_{1,n}`.
    pub(crate) fn map_latex(&self, index: &str) -> String {
        match &self.sub_latex {
            Some(s) => format!("{}_{{{s},{index}}}", self.base_latex),
            None => format!("{}_{{{index}}}", self.base_latex),
        }
    }

    /// Name of the `k`-th derivative state: `x`, `x'`, `x''`, ...
    pub(crate) fn derived_name(&self, k: u32) -> String {
        format!("{}{}", self.name, "'".repeat(k as usize))
    }
}

/// Greek letters accepted as `\name` (LaTeX) or `name` (text).
const GREEK: &[&str] = &[
    "alpha",
    "beta",
    "gamma",
    "delta",
    "epsilon",
    "varepsilon",
    "zeta",
    "eta",
    "theta",
    "vartheta",
    "iota",
    "kappa",
    "lambda",
    "mu",
    "nu",
    "xi",
    "pi",
    "varpi",
    "rho",
    "varrho",
    "sigma",
    "varsigma",
    "tau",
    "upsilon",
    "phi",
    "varphi",
    "chi",
    "psi",
    "omega",
    "Gamma",
    "Delta",
    "Theta",
    "Lambda",
    "Xi",
    "Pi",
    "Sigma",
    "Upsilon",
    "Phi",
    "Psi",
    "Omega",
];

pub(crate) fn is_greek(word: &str) -> bool {
    GREEK.contains(&word)
}

/// LaTeX for one word: Greek gets a backslash, one letter stays as is,
/// longer words are set upright.
fn latex_word(w: &str) -> String {
    if is_greek(w) {
        format!("\\{w}")
    } else if w.len() == 1 || w.bytes().all(|b| b.is_ascii_digit()) {
        w.to_string()
    } else {
        format!("\\mathrm{{{w}}}")
    }
}

/// Build a symbol from a text-mode word: `sigma` → `\sigma`, `x_1` → `x_{1}`,
/// `omega0` → `\omega_{0}`, `k_on` → `k_{\mathrm{on}}`.
pub(crate) fn sym_from_word(word: &str) -> SymName {
    let (base, sub) = match word.find('_') {
        Some(i) => (&word[..i], Some(&word[i + 1..])),
        None => {
            let digits = word.bytes().rev().take_while(u8::is_ascii_digit).count();
            if digits > 0 && digits < word.len() {
                let i = word.len() - digits;
                (&word[..i], Some(&word[i..]))
            } else {
                (word, None)
            }
        }
    };
    SymName {
        name: word.to_string(),
        base_latex: latex_word(base),
        sub_latex: sub.map(|s| s.split('_').map(latex_word).collect::<Vec<_>>().join(",")),
    }
}

/// Delimiter kinds for grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Delim {
    Paren,
    Bracket,
    Brace,
}

impl Delim {
    pub(crate) fn open_str(self) -> &'static str {
        match self {
            Delim::Paren => "(",
            Delim::Bracket => "[",
            Delim::Brace => "{",
        }
    }
    pub(crate) fn close_str(self) -> &'static str {
        match self {
            Delim::Paren => ")",
            Delim::Bracket => "]",
            Delim::Brace => "}",
        }
    }
}

/// Token kinds shared by both formats.
#[derive(Debug, Clone)]
pub(crate) enum Tok {
    Num(f64),
    Ident(SymName),
    Func(Func),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Underscore,
    /// One or more primes: `'`, `\prime`, `\doubleprime`.
    Prime(u32),
    Open(Delim),
    Close(Delim),
    /// A bare `|`: opens or closes an absolute value depending on context.
    Pipe,
    /// `\left|`
    AbsOpen,
    /// `\right|`
    AbsClose,
    Comma,
    Eq,
    /// Equation separator: newline, `;` or `\\`.
    Sep,
    Frac,
    Sqrt,
    /// `\dot` (1), `\ddot` (2), `\dddot` (3).
    Dot(u32),
    Placeholder,
    /// Something we could not read; the message explains why.
    Bad(String),
}

/// A token and its byte span in the source.
#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub tok: Tok,
    pub span: Span,
}

/// Split `src` into tokens.
pub(crate) fn tokenize(src: &str, format: InputFormat) -> Vec<Token> {
    let mut lx = Lexer {
        src,
        pos: 0,
        out: Vec::new(),
        text: format == InputFormat::Text,
    };
    lx.run();
    lx.out
}

const DECIMAL_COMMA: &str = "Decimal commas aren't supported: write 1.5 instead of 1,5";

struct Lexer<'s> {
    src: &'s str,
    pos: usize,
    out: Vec<Token>,
    text: bool,
}

impl<'s> Lexer<'s> {
    fn bytes(&self) -> &'s [u8] {
        self.src.as_bytes()
    }

    fn peek_byte(&self) -> Option<u8> {
        self.bytes().get(self.pos).copied()
    }

    fn push(&mut self, tok: Tok, start: usize) {
        self.out.push(Token {
            tok,
            span: (start, self.pos),
        });
    }

    fn bad(&mut self, msg: impl Into<String>, start: usize) {
        self.push(Tok::Bad(msg.into()), start);
    }

    fn run(&mut self) {
        while let Some(c) = self.peek_byte() {
            let start = self.pos;
            match c {
                b' ' | b'\t' | b'\r' => self.pos += 1,
                b'&' | b'~' if !self.text => self.pos += 1,
                b'\n' | b';' => {
                    self.pos += 1;
                    self.push(Tok::Sep, start);
                }
                b'\\' if !self.text => self.command(),
                b'\\' => {
                    self.pos += 1;
                    self.bad(
                        "Backslash commands are LaTeX. Switch the input to LaTeX, or write \
                         sin(x), sqrt(x), sigma, … in text mode",
                        start,
                    );
                }
                b'0'..=b'9' | b'.' => self.number(),
                b'a'..=b'z' | b'A'..=b'Z' => {
                    if self.text {
                        self.word();
                    } else {
                        self.letter();
                    }
                }
                b'*' if self.text && self.src[start..].starts_with("**") => {
                    self.pos += 2;
                    self.push(Tok::Caret, start);
                }
                b'{' if self.src[start..].starts_with("{,}") => {
                    self.pos += 3;
                    self.bad(DECIMAL_COMMA, start);
                }
                _ if c.is_ascii() => {
                    self.pos += 1;
                    let tok = match c {
                        b'\'' => Tok::Prime(1),
                        b'+' => Tok::Plus,
                        b'-' => Tok::Minus,
                        b'*' => Tok::Star,
                        b'/' => Tok::Slash,
                        b'^' => Tok::Caret,
                        b'_' => Tok::Underscore,
                        b'(' => Tok::Open(Delim::Paren),
                        b')' => Tok::Close(Delim::Paren),
                        b'[' => Tok::Open(Delim::Bracket),
                        b']' => Tok::Close(Delim::Bracket),
                        b'{' => Tok::Open(Delim::Brace),
                        b'}' => Tok::Close(Delim::Brace),
                        b'|' => Tok::Pipe,
                        b',' => Tok::Comma,
                        b'=' => Tok::Eq,
                        b'<' | b'>' => Tok::Bad("Inequalities aren't supported; use =".into()),
                        _ => Tok::Bad(format!("Unexpected character '{}'", c as char)),
                    };
                    self.push(tok, start);
                }
                _ => {
                    let ch = self.src[start..].chars().next().unwrap_or('?');
                    self.pos += ch.len_utf8();
                    let tok = unicode_token(ch)
                        .unwrap_or_else(|| Tok::Bad(format!("Unexpected character '{ch}'")));
                    self.push(tok, start);
                }
            }
        }
    }

    /// Number: `12`, `1.5`, `.5`; in text mode also `1.5e-3`.
    fn number(&mut self) {
        let start = self.pos;
        let b = self.bytes();
        let mut i = start;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        if self.text && i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            if j < b.len() && b[j].is_ascii_digit() {
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
            }
        }
        if i < b.len() && b[i] == b'.' {
            // `1.2.3`: take the whole run so the error covers it.
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            self.pos = i;
            self.bad("A number can have only one decimal point", start);
            return;
        }
        self.pos = i;
        match self.src[start..i].parse::<f64>() {
            Ok(v) => self.push(Tok::Num(v), start),
            Err(_) => self.bad("A lone '.' isn't a number", start),
        }
    }

    /// LaTeX: a single letter. Runs of letters that spell a function name
    /// (`sin` without backslash) are almost always a typo, so flag them.
    fn letter(&mut self) {
        let start = self.pos;
        let rest = &self.src[start..];
        for f in FUNCTION_WORDS {
            if f.len() >= 3 && rest.starts_with(f) && !matches!(*f, "min" | "max") {
                self.pos += f.len();
                let dotted: Vec<String> = f.chars().map(String::from).collect();
                self.bad(
                    format!(
                        "\"{f}\" here is read as {}; did you mean \\{f} x?",
                        dotted.join("·")
                    ),
                    start,
                );
                return;
            }
        }
        self.pos += 1;
        let s = &self.src[start..self.pos];
        self.push(Tok::Ident(SymName::simple(s, s)), start);
    }

    /// Text mode: a whole word, `[A-Za-z][A-Za-z0-9]*` with `_suffix` parts.
    fn word(&mut self) {
        let start = self.pos;
        let b = self.bytes();
        let mut i = start;
        while i < b.len() && b[i].is_ascii_alphanumeric() {
            i += 1;
        }
        while i + 1 < b.len() && b[i] == b'_' && b[i + 1].is_ascii_alphanumeric() {
            i += 1;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
        }
        self.pos = i;
        let w = &self.src[start..i];
        let tok = match Func::from_name(w) {
            Some(f) => Tok::Func(f),
            None => Tok::Ident(sym_from_word(w)),
        };
        self.push(tok, start);
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\t' | b'\r')) {
            self.pos += 1;
        }
    }

    /// Read `{...}` (with nesting) and return the inside, or `None` if the
    /// next non-space character is not `{`.
    fn read_group(&mut self) -> Option<&'s str> {
        let save = self.pos;
        self.skip_spaces();
        if self.peek_byte() != Some(b'{') {
            self.pos = save;
            return None;
        }
        let open = self.pos + 1;
        let mut depth = 0usize;
        let b = self.bytes();
        let mut i = self.pos;
        while i < b.len() {
            match b[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        self.pos = i + 1;
                        return Some(&self.src[open..i]);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        self.pos = save;
        None
    }

    /// Read the letters of a command name after the backslash.
    fn command_name(&mut self) -> &'s str {
        let start = self.pos;
        while matches!(self.peek_byte(), Some(c) if c.is_ascii_alphabetic()) {
            self.pos += 1;
        }
        &self.src[start..self.pos]
    }

    /// A backslash command.
    fn command(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let Some(c) = self.peek_byte() else {
            self.bad("A backslash must be followed by a command name", start);
            return;
        };
        if !c.is_ascii_alphabetic() {
            let ch = self.src[self.pos..].chars().next().unwrap_or(' ');
            self.pos += ch.len_utf8();
            match ch {
                '\\' => self.push(Tok::Sep, start),
                ',' | ';' | ':' | '!' | ' ' | '>' | '{' | '}' => {}
                '|' => self.push(Tok::Pipe, start),
                _ => self.bad(format!("\\{ch} isn't supported"), start),
            }
            return;
        }
        let name = self.command_name();
        let tok = match name {
            "left" | "mleft" => return self.left_right(true, start),
            "right" | "mright" => return self.left_right(false, start),
            "lparen" => Tok::Open(Delim::Paren),
            "rparen" => Tok::Close(Delim::Paren),
            "lbrack" => Tok::Open(Delim::Bracket),
            "rbrack" => Tok::Close(Delim::Bracket),
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" | "biggl"
            | "biggr" | "Biggl" | "Biggr" | "quad" | "qquad" | "enspace" | "thinspace"
            | "medspace" | "thickspace" | "negthinspace" | "negmedspace" | "negthickspace"
            | "displaystyle" | "textstyle" | "limits" | "nolimits" | "mathstrut" | "lbrace"
            | "rbrace" => return,
            "cdot" | "times" | "ast" => Tok::Star,
            "div" => Tok::Slash,
            "frac" | "dfrac" | "tfrac" | "cfrac" => Tok::Frac,
            "sqrt" => Tok::Sqrt,
            "dot" => Tok::Dot(1),
            "ddot" => Tok::Dot(2),
            "dddot" => Tok::Dot(3),
            "prime" => Tok::Prime(1),
            "doubleprime" => Tok::Prime(2),
            "tripleprime" => Tok::Prime(3),
            "exponentialE" => Tok::Ident(SymName::simple("e", "e")),
            "differentialD" => Tok::Ident(SymName::simple("d", "d")),
            "placeholder" => {
                // \placeholder[id]{content}
                self.skip_spaces();
                if self.peek_byte() == Some(b'[') {
                    while let Some(c) = self.peek_byte() {
                        self.pos += 1;
                        if c == b']' {
                            break;
                        }
                    }
                }
                let _ = self.read_group();
                Tok::Placeholder
            }
            "mathrm" | "operatorname" | "text" | "textrm" | "mathit" | "mathcal" | "mathsf"
            | "mathbf" | "boldsymbol" | "mathnormal" | "textit" => {
                if self.peek_byte() == Some(b'*') {
                    self.pos += 1;
                }
                match self.read_group() {
                    Some(content) => named_group(content),
                    None => Tok::Bad(format!("\\{name} must be followed by {{…}}")),
                }
            }
            "begin" | "end" => return self.environment(name == "begin", start),
            "vert" | "mid" => Tok::Pipe,
            "lvert" => Tok::AbsOpen,
            "rvert" => Tok::AbsClose,
            "infty" => Tok::Bad("Infinity can't be used in an equation".into()),
            "imaginaryI" => Tok::Bad("Complex numbers aren't supported".into()),
            "partial" => Tok::Bad(
                "Partial derivatives aren't supported; write time derivatives as \\dot{x} or x'"
                    .into(),
            ),
            "lt" | "gt" | "le" | "ge" | "leq" | "geq" | "neq" | "ne" | "approx" => {
                Tok::Bad("Only = is supported, not inequalities".into())
            }
            _ if is_greek(name) => Tok::Ident(SymName::simple(name, &format!("\\{name}"))),
            _ => match Func::from_name(name) {
                Some(f) => Tok::Func(f),
                None => Tok::Bad(format!("\\{name} isn't supported")),
            },
        };
        self.push(tok, start);
    }

    /// `\left(`, `\right)`, `\left|`, `\left.` ...
    fn left_right(&mut self, left: bool, start: usize) {
        self.skip_spaces();
        let Some(c) = self.peek_byte() else {
            self.bad("\\left / \\right must be followed by a bracket", start);
            return;
        };
        let delim: &str = if c == b'\\' {
            self.pos += 1;
            match self.peek_byte() {
                Some(c) if c.is_ascii_alphabetic() => self.command_name(),
                Some(_) => {
                    let s = self.pos;
                    self.pos += 1;
                    &self.src[s..self.pos]
                }
                None => "",
            }
        } else {
            let ch = self.src[self.pos..].chars().next().unwrap_or(' ');
            self.pos += ch.len_utf8();
            &self.src[self.pos - ch.len_utf8()..self.pos]
        };
        let tok = match (left, delim) {
            (true, "(") => Tok::Open(Delim::Paren),
            (false, ")") => Tok::Close(Delim::Paren),
            (true, "[") => Tok::Open(Delim::Bracket),
            (false, "]") => Tok::Close(Delim::Bracket),
            (true, "|" | "vert" | "lvert") => Tok::AbsOpen,
            (false, "|" | "vert" | "rvert") => Tok::AbsClose,
            // Invisible or decorative delimiters (e.g. around a cases block).
            (_, "." | "{" | "}" | "lbrace" | "rbrace") => return,
            _ => Tok::Bad(format!(
                "\\{}{} isn't supported",
                if left { "left" } else { "right" },
                delim
            )),
        };
        self.push(tok, start);
    }

    /// `\begin{cases}` / `\end{aligned}` etc. are layout only.
    fn environment(&mut self, begin: bool, start: usize) {
        let Some(env) = self.read_group() else {
            self.bad(
                "\\begin / \\end must name an environment, like \\begin{cases}",
                start,
            );
            return;
        };
        match env.trim().trim_end_matches('*') {
            "cases" | "dcases" | "rcases" | "aligned" | "align" | "alignat" | "gathered"
            | "gather" | "split" | "array" => {
                if begin && matches!(env.trim(), "array" | "alignat") {
                    let _ = self.read_group(); // column spec
                }
            }
            other => self.bad(format!("The {other} environment isn't supported"), start),
        }
    }
}

/// Classify the inside of `\mathrm{…}` / `\operatorname{…}`.
fn named_group(content: &str) -> Tok {
    let c: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    if c.is_empty() {
        return Tok::Bad("This \\mathrm{} / \\operatorname{} is empty".into());
    }
    if let Some(f) = Func::from_name(&c) {
        return Tok::Func(f);
    }
    let ok = c.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic())
        && c.chars().all(|ch| ch.is_ascii_alphanumeric());
    if !ok {
        return Tok::Bad(format!(
            "Can't read the name \"{content}\"; use letters and digits only"
        ));
    }
    if c.len() == 1 {
        Tok::Ident(SymName::simple(&c, &c))
    } else if is_greek(&c) {
        Tok::Ident(SymName::simple(&c, &format!("\\{c}")))
    } else {
        Tok::Ident(SymName::simple(&c, &format!("\\mathrm{{{c}}}")))
    }
}

/// Unicode characters people paste: Greek letters, `·`, `×`, `−`, `′`.
fn unicode_token(c: char) -> Option<Tok> {
    let greek = match c {
        'α' => "alpha",
        'β' => "beta",
        'γ' => "gamma",
        'δ' => "delta",
        'ε' => "epsilon",
        'ζ' => "zeta",
        'η' => "eta",
        'θ' => "theta",
        'ι' => "iota",
        'κ' => "kappa",
        'λ' => "lambda",
        'μ' => "mu",
        'ν' => "nu",
        'ξ' => "xi",
        'π' => "pi",
        'ρ' => "rho",
        'σ' => "sigma",
        'τ' => "tau",
        'υ' => "upsilon",
        'φ' | 'ϕ' => "phi",
        'χ' => "chi",
        'ψ' => "psi",
        'ω' => "omega",
        'Γ' => "Gamma",
        'Δ' => "Delta",
        'Θ' => "Theta",
        'Λ' => "Lambda",
        'Ξ' => "Xi",
        'Π' => "Pi",
        'Σ' => "Sigma",
        'Φ' => "Phi",
        'Ψ' => "Psi",
        'Ω' => "Omega",
        _ => "",
    };
    if !greek.is_empty() {
        return Some(Tok::Ident(SymName::simple(greek, &format!("\\{greek}"))));
    }
    Some(match c {
        '·' | '×' | '⋅' | '∗' => Tok::Star,
        '−' | '–' => Tok::Minus,
        '÷' => Tok::Slash,
        '′' => Tok::Prime(1),
        '″' => Tok::Prime(2),
        '‴' => Tok::Prime(3),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str, f: InputFormat) -> Vec<String> {
        tokenize(src, f)
            .into_iter()
            .map(|t| match t.tok {
                Tok::Ident(s) => format!("id:{}", s.name),
                Tok::Num(v) => format!("num:{v}"),
                Tok::Func(f) => format!("fn:{}", f.text_name()),
                Tok::Bad(m) => format!("bad:{m}"),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn latex_basic() {
        let k = kinds("\\dot{x}=\\sigma\\left(y-x\\right)", InputFormat::Latex);
        assert_eq!(
            k,
            [
                "Dot(1)",
                "Open(Brace)",
                "id:x",
                "Close(Brace)",
                "Eq",
                "id:sigma",
                "Open(Paren)",
                "id:y",
                "Minus",
                "id:x",
                "Close(Paren)"
            ]
        );
    }

    #[test]
    fn latex_ignores_spacing_and_alignment() {
        let k = kinds(
            "\\begin{cases}x\\,&\\;y\\quad\\!\\end{cases}",
            InputFormat::Latex,
        );
        assert_eq!(k, ["id:x", "id:y"]);
    }

    #[test]
    fn latex_mathrm_and_operatorname() {
        let k = kinds(
            "\\mathrm{d}\\operatorname{sin}\\mathrm{foo}",
            InputFormat::Latex,
        );
        assert_eq!(k, ["id:d", "fn:sin", "id:foo"]);
    }

    #[test]
    fn latex_spans_are_bytes() {
        let t = tokenize("θ+\\theta", InputFormat::Latex);
        assert_eq!(t[0].span, (0, 2));
        assert_eq!(t[1].span, (2, 3));
        assert_eq!(t[2].span, (3, 9));
    }

    #[test]
    fn latex_spelled_function_is_flagged() {
        let k = kinds("sinx", InputFormat::Latex);
        assert!(k[0].starts_with("bad:"), "{k:?}");
    }

    #[test]
    fn text_words_and_scientific() {
        let k = kinds("sigma*x_1 + 1.5e-3 - 2e x**2", InputFormat::Text);
        assert_eq!(
            k,
            [
                "id:sigma",
                "Star",
                "id:x_1",
                "Plus",
                "num:0.0015",
                "Minus",
                "num:2",
                "id:e",
                "id:x",
                "Caret",
                "num:2"
            ]
        );
    }

    #[test]
    fn word_latex_forms() {
        assert_eq!(sym_from_word("omega0").latex(), "\\omega_{0}");
        assert_eq!(sym_from_word("x_1").latex(), "x_{1}");
        assert_eq!(sym_from_word("k_on").latex(), "k_{\\mathrm{on}}");
        assert_eq!(sym_from_word("gain").latex(), "\\mathrm{gain}");
        assert_eq!(sym_from_word("theta").dotted(2), "\\ddot{\\theta}");
        assert_eq!(sym_from_word("x_1").dotted(1), "\\dot{x}_{1}");
    }
}
