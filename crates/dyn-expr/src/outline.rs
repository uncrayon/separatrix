//! The typed equations, line by line, for exports.
//!
//! [`outline`] splits a LaTeX system into its equations and says what each
//! one defines (a flow, a map, a Hamiltonian or a parameter value), keeping
//! the text of both sides as the user typed it. The export writes the
//! right-hand sides exactly as typed and rebuilds the left-hand sides in the
//! notation the user picks (`\dot{x}`, `x'` or `\frac{dx}{dt}`).
//!
//! The typed text is passed through [`portable_latex`], which rewrites the
//! few control words that only MathLive or this parser know (`\mleft`,
//! `\exponentialE`, `\sgn`, …) into ones plain LaTeX with amsmath defines.
//! Everything else is kept byte for byte.

use serde::{Deserialize, Serialize};

use crate::ast::{Ast, AstKind};
use crate::parser::parse_lines;
use crate::sema::leibniz_to_deriv;
use crate::token::{tokenize, Span, SymName, Tok};
use crate::{analyze_source, Diagnostic, InputFormat};

/// What one equation defines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutlineRole {
    /// `\dot{x} = …`, `x'' = …`, `\frac{dx}{dt} = …`: the `order`-th time
    /// derivative of `state`.
    #[serde(rename_all = "camelCase")]
    Flow {
        /// The state's identity, e.g. `"x_1"`, `"theta"`.
        state: String,
        /// The state's display form, e.g. `"x_{1}"`, `"\\theta"`.
        state_latex: String,
        /// The state's letter without its subscript, in the font its
        /// derivative was typed in, e.g. `"x"`, `"\\theta"`, `"\\mathrm{S}"`.
        base_latex: String,
        /// The state's subscript without braces, e.g. `"1"`; `None` if it
        /// has none.
        sub_latex: Option<String>,
        /// 1 for `\dot{x}`, 2 for `\ddot{x}`, ….
        order: u32,
    },
    /// `x_{n+1} = …`.
    #[serde(rename_all = "camelCase")]
    Map {
        /// The state's identity.
        state: String,
        /// The state's display form, e.g. `"x_{1}"`.
        state_latex: String,
        /// The state's letter without its subscript.
        base_latex: String,
        /// The state's subscript without braces.
        sub_latex: Option<String>,
    },
    /// `H(q, p) = …`.
    Hamiltonian,
    /// A parameter value such as `\sigma = 10`.
    #[serde(rename_all = "camelCase")]
    Parameter {
        /// The parameter's identity, e.g. `"sigma"`.
        name: String,
    },
    /// An equation that is not solved for its highest derivative, such as
    /// `\ddot{\theta} + b\dot{\theta} + \sin\theta = 0`. The export keeps
    /// both sides as typed.
    #[serde(rename_all = "camelCase")]
    Implicit {
        /// The state whose highest derivative the equation defines.
        state: String,
        /// The state's display form.
        state_latex: String,
        /// The state's letter without its subscript.
        base_latex: String,
        /// The state's subscript without braces.
        sub_latex: Option<String>,
        /// The order of that derivative.
        order: u32,
    },
}

/// A piece of one side of an equation: typed text, or a time derivative of a
/// state that an export rewrites in the notation the user picks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LatexPart {
    /// Text as typed, made portable ([`portable_latex`]).
    Text {
        /// The text.
        text: String,
    },
    /// `\dot{x}`, `x''`, `\frac{dx}{dt}`, … as typed.
    #[serde(rename_all = "camelCase")]
    Derivative {
        /// The state's identity, e.g. `"x_1"`.
        state: String,
        /// The state's letter without its subscript, in the font it was
        /// typed in, e.g. `"x"`, `"\\theta"`, `"\\mathrm{S}"`.
        base_latex: String,
        /// The state's subscript without braces.
        sub_latex: Option<String>,
        /// 1 for `\dot{x}`, 2 for `\ddot{x}`, ….
        order: u32,
        /// It was typed with `(t)` after it, as in `\dot{x}(t)`.
        of_t: bool,
    },
}

/// One equation of the typed system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquationOutline {
    /// Index of the line it was typed on (several equations can share a line,
    /// separated by `;` or `\\`).
    pub line: usize,
    /// What it defines.
    pub role: OutlineRole,
    /// The left-hand side as typed, made portable ([`portable_latex`]).
    pub lhs: String,
    /// The right-hand side as typed, made portable ([`portable_latex`]).
    pub rhs: String,
    /// The left-hand side in pieces, its derivatives picked out.
    pub lhs_parts: Vec<LatexPart>,
    /// The right-hand side in pieces, its derivatives picked out.
    pub rhs_parts: Vec<LatexPart>,
}

/// The equations of a LaTeX system, in the order typed, with what each one
/// defines and the text of both sides.
///
/// The system must compile: the lines go through the same analysis as
/// [`crate::compile`] (joined like the UI joins them), and its errors are
/// returned when it does not. Empty lines and lines holding only spacing are
/// skipped.
pub fn outline(lines: &[String]) -> Result<Vec<EquationOutline>, Vec<Diagnostic>> {
    let joined = lines
        .iter()
        .map(String::as_str)
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let mut diags = Vec::new();
    let analysis = analyze_source(&joined, InputFormat::Latex, &mut diags);
    if diags.iter().any(Diagnostic::is_error) {
        return Err(diags.into_iter().filter(Diagnostic::is_error).collect());
    }
    let Some(analysis) = analysis else {
        return Err(vec![Diagnostic::error("The system has no equations", None)]);
    };
    let states = &analysis.info.states;
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        for (start, eq, end) in equation_spans(line) {
            let text = &line[start..end];
            let mut d = Vec::new();
            let parsed = parse_lines(text, InputFormat::Latex, &mut d);
            let [parsed] = parsed.as_slice() else {
                return Err(vec![Diagnostic::error(
                    format!("Line {} could not be read on its own", index + 1),
                    None,
                )]);
            };
            let mut role = role_of(parsed.lhs.clone(), parsed.rhs.clone());
            // A flow or map must define one of the states the analysis found.
            if let OutlineRole::Flow { state, .. } | OutlineRole::Map { state, .. } = &role {
                debug_assert!(states.iter().any(|s| &s.name == state), "{state}");
            }
            // The derivatives on both sides, with their place in `line`.
            let mut derivs: Vec<(Span, LatexPart)> = Vec::new();
            for side in [&parsed.lhs, &parsed.rhs] {
                leibniz_to_deriv(side.clone()).walk(&mut |n| {
                    if let AstKind::Deriv(sym, k) = &n.kind {
                        let span = (start + n.span.0, start + n.span.1);
                        derivs.push((span, derivative_part(line, span, sym, *k)));
                    }
                });
            }
            derivs.sort_by_key(|(span, _)| span.0);
            // The left-hand side's state keeps the font it was typed in.
            if let OutlineRole::Flow { base_latex, .. } = &mut role {
                if let Some((
                    _,
                    LatexPart::Derivative {
                        base_latex: typed, ..
                    },
                )) = derivs.iter().find(|(span, _)| span.1 <= eq.0)
                {
                    *base_latex = typed.clone();
                }
            }
            out.push(EquationOutline {
                line: index,
                role,
                lhs: portable_latex(line[start..eq.0].replace('&', "").trim()),
                rhs: portable_latex(line[eq.1..end].replace('&', "").trim()),
                lhs_parts: side_parts(line, (start, eq.0), &derivs),
                rhs_parts: side_parts(line, (eq.1, end), &derivs),
            });
        }
    }
    Ok(out)
}

/// For each equation on a line: where it starts, where its `=` is and where
/// it ends (byte offsets; separators and wrapping environments excluded).
fn equation_spans(line: &str) -> Vec<(usize, (usize, usize), usize)> {
    let toks = tokenize(line, InputFormat::Latex);
    toks.split(|t| matches!(t.tok, Tok::Sep))
        .filter(|g| !g.is_empty())
        .filter_map(|g| {
            let eq = g.iter().find(|t| matches!(t.tok, Tok::Eq))?;
            Some((g[0].span.0, eq.span, g[g.len() - 1].span.1))
        })
        .collect()
}

/// A derivative typed at `span` of `line`, as a [`LatexPart`].
fn derivative_part(line: &str, span: Span, sym: &SymName, order: u32) -> LatexPart {
    let typed = &line[span.0..span.1];
    // The identifier as typed: `\mathrm{S}` where the parser only keeps `S`.
    let base = tokenize(typed, InputFormat::Latex)
        .into_iter()
        .rfind(|t| matches!(&t.tok, Tok::Ident(s) if s.base_latex == sym.base_latex))
        .map(|t| portable_latex(typed[t.span.0..t.span.1].trim()))
        .filter(|b| b.starts_with('\\') && b.contains('{'))
        .unwrap_or_else(|| sym.base_latex.clone());
    let compact: String = typed.chars().filter(|c| !c.is_whitespace()).collect();
    LatexPart::Derivative {
        state: sym.name.clone(),
        base_latex: base,
        sub_latex: sym.sub_latex.clone(),
        order,
        of_t: compact.ends_with("(t)"),
    }
}

/// The text of `line` in `range`, cut round the derivatives in it.
fn side_parts(line: &str, range: Span, derivs: &[(Span, LatexPart)]) -> Vec<LatexPart> {
    let mut out = Vec::new();
    let text = |a: usize, b: usize| portable_latex(&line[a..b].replace('&', ""));
    let mut at = range.0;
    for (span, part) in derivs
        .iter()
        .filter(|(s, _)| s.0 >= range.0 && s.1 <= range.1)
    {
        if span.0 > at {
            out.push(LatexPart::Text {
                text: text(at, span.0),
            });
        }
        out.push(part.clone());
        at = span.1;
    }
    if range.1 > at {
        out.push(LatexPart::Text {
            text: text(at, range.1),
        });
    }
    // Trim the side as a whole, as `lhs` and `rhs` are.
    if let Some(LatexPart::Text { text }) = out.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(LatexPart::Text { text }) = out.last_mut() {
        *text = text.trim_end().to_string();
    }
    out.retain(|p| !matches!(p, LatexPart::Text { text } if text.is_empty()));
    out
}

fn parts(s: &SymName) -> (String, String, String, Option<String>) {
    (
        s.name.clone(),
        s.latex(),
        s.base_latex.clone(),
        s.sub_latex.clone(),
    )
}

fn has_deriv(a: &Ast) -> bool {
    let mut found = false;
    a.walk(&mut |n| found |= matches!(n.kind, AstKind::Deriv(..)));
    found
}

/// The role of a parsed equation, read off its left-hand side the way the
/// analysis classifies lines.
fn role_of(lhs: Ast, rhs: Ast) -> OutlineRole {
    let lhs = leibniz_to_deriv(lhs);
    let rhs = leibniz_to_deriv(rhs);
    match (&lhs.kind, lhs.plain_symbol()) {
        (AstKind::Deriv(s, k), _) => {
            let (state, state_latex, base_latex, sub_latex) = parts(s);
            OutlineRole::Flow {
                state,
                state_latex,
                base_latex,
                sub_latex,
                order: *k,
            }
        }
        (AstKind::Next(s), _) => {
            let (state, state_latex, base_latex, sub_latex) = parts(s);
            OutlineRole::Map {
                state,
                state_latex,
                base_latex,
                sub_latex,
            }
        }
        (AstKind::Apply(s, _), _) if s.name == "H" => OutlineRole::Hamiltonian,
        (_, Some(s)) if !has_deriv(&rhs) => OutlineRole::Parameter { name: s.name },
        _ => {
            // Rearranged: the highest derivative on either side.
            let mut best: Option<(SymName, u32)> = None;
            let mut visit = |n: &Ast| {
                if let AstKind::Deriv(s, k) = &n.kind {
                    if best.as_ref().is_none_or(|b| *k > b.1) {
                        best = Some((s.clone(), *k));
                    }
                }
            };
            lhs.walk(&mut visit);
            rhs.walk(&mut visit);
            let (s, order) = best.unwrap_or((SymName::simple("x", "x"), 1));
            let (state, state_latex, base_latex, sub_latex) = parts(&s);
            OutlineRole::Implicit {
                state,
                state_latex,
                base_latex,
                sub_latex,
                order,
            }
        }
    }
}

/// Function names the parser accepts that LaTeX (with amsmath) defines.
const LATEX_FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "exp", "ln", "log", "sqrt", "sinh", "cosh", "tanh", "arcsin", "arccos",
    "arctan", "min", "max",
];

/// What a control word becomes in plain LaTeX, if it changes.
fn portable_word(word: &str) -> Option<&'static str> {
    Some(match word {
        "mleft" => "\\left",
        "mright" => "\\right",
        "exponentialE" => "\\mathrm{e}",
        "differentialD" => "\\mathrm{d}",
        "doubleprime" => "''",
        "tripleprime" => "'''",
        "lparen" => "(",
        "rparen" => ")",
        "lbrack" => "[",
        "rbrack" => "]",
        "mathnormal" => "",
        "abs" => "\\operatorname{abs}",
        "sgn" | "sign" => "\\operatorname{sgn}",
        "cbrt" => "\\operatorname{cbrt}",
        "asin" => "\\arcsin",
        "acos" => "\\arccos",
        "atan" => "\\arctan",
        _ => return None,
    })
}

/// `s` with the control words that only MathLive or this parser know
/// rewritten into plain LaTeX (amsmath): `\mleft`/`\mright` to
/// `\left`/`\right`, `\exponentialE` to `\mathrm{e}`, `\differentialD` to
/// `\mathrm{d}`, `\doubleprime` to `''`, `\lparen` to `(`, `\mathnormal{…}`
/// to `{…}`, and function names LaTeX lacks to `\operatorname{…}` (`\asin`
/// to `\arcsin`). A superscript made only of primes (`x^{\prime\prime}`,
/// `x^{\doubleprime}`) becomes primes (`x''`). Everything else is kept byte
/// for byte.
pub fn portable_latex(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'^' {
            if let Some((primes, len)) = prime_superscript(&s[i..]) {
                out.push_str(&"'".repeat(primes));
                i += len;
                continue;
            }
        }
        if b[i] != b'\\' {
            let ch = s[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < b.len() && b[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j == start {
            // A control symbol (`\,`, `\{`, `\\`): copy it whole.
            let ch = s[start..].chars().next();
            let end = start + ch.map_or(0, char::len_utf8);
            out.push_str(&s[i..end]);
            i = end;
            continue;
        }
        let word = &s[start..j];
        if word == "placeholder" {
            // `\placeholder[id]{content}`: keep the content group.
            let mut k = j;
            if b.get(k) == Some(&b'[') {
                while k < b.len() && b[k] != b']' {
                    k += 1;
                }
                k = (k + 1).min(b.len());
            }
            i = k;
            continue;
        }
        match portable_word(word) {
            Some(rep) => out.push_str(rep),
            None => {
                let is_function = crate::ast::Func::from_name(word).is_some();
                if is_function && !LATEX_FUNCTIONS.contains(&word) {
                    out.push_str(&format!("\\operatorname{{{word}}}"));
                } else {
                    out.push_str(&s[i..j]);
                }
            }
        }
        i = j;
    }
    out
}

/// `^{\prime\prime}`, `^{\doubleprime}`, `^\prime` at the start of `s`: how
/// many primes, and how many bytes they take.
fn prime_superscript(s: &str) -> Option<(usize, usize)> {
    let rest = s.strip_prefix('^')?;
    let count = |words: &str| -> Option<usize> {
        let mut n = 0;
        let mut w = words.trim();
        if w.is_empty() {
            return None;
        }
        while !w.is_empty() {
            let (k, len) = [
                ("\\tripleprime", 3),
                ("\\doubleprime", 2),
                ("\\prime", 1),
                ("'", 1),
            ]
            .iter()
            .find_map(|(p, k)| {
                let after = w.strip_prefix(p)?;
                let ends_word =
                    p.len() == 1 || !after.starts_with(|c: char| c.is_ascii_alphabetic());
                ends_word.then_some((*k, p.len()))
            })?;
            n += k;
            w = w[len..].trim_start();
        }
        Some(n)
    };
    if let Some(group) = rest.strip_prefix('{') {
        let close = group.find('}')?;
        let n = count(&group[..close])?;
        return Some((n, 2 + close + 1));
    }
    for (p, k) in [("\\tripleprime", 3), ("\\doubleprime", 2), ("\\prime", 1)] {
        if let Some(after) = rest.strip_prefix(p) {
            if !after.starts_with(|c: char| c.is_ascii_alphabetic()) {
                return Some((k, 1 + p.len()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Control words plain LaTeX with amsmath (and amssymb-free) defines,
    /// which the rewritten text may use.
    const PLAIN_LATEX: &[&str] = &[
        "left",
        "right",
        "big",
        "Big",
        "bigg",
        "Bigg",
        "bigl",
        "bigr",
        "Bigl",
        "Bigr",
        "biggl",
        "biggr",
        "Biggl",
        "Biggr",
        "quad",
        "qquad",
        "enspace",
        "thinspace",
        "medspace",
        "thickspace",
        "negthinspace",
        "negmedspace",
        "negthickspace",
        "displaystyle",
        "textstyle",
        "limits",
        "nolimits",
        "mathstrut",
        "lbrace",
        "rbrace",
        "cdot",
        "times",
        "ast",
        "div",
        "frac",
        "dfrac",
        "tfrac",
        "cfrac",
        "sqrt",
        "dot",
        "ddot",
        "dddot",
        "prime",
        "mathrm",
        "operatorname",
        "text",
        "textrm",
        "mathit",
        "mathcal",
        "mathsf",
        "mathbf",
        "boldsymbol",
        "textit",
        "begin",
        "end",
        "vert",
        "mid",
        "lvert",
        "rvert",
        "sin",
        "cos",
        "tan",
        "exp",
        "ln",
        "log",
        "sinh",
        "cosh",
        "tanh",
        "arcsin",
        "arccos",
        "arctan",
        "min",
        "max",
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

    /// Every control word the tokenizer gives a meaning to: each quoted
    /// word in its source (the `match` arms of `command`), plus the Greek
    /// letters and function names, kept when tokenizing `\word{x}` gives
    /// no error.
    fn accepted_control_words() -> Vec<String> {
        let src = include_str!("token.rs");
        let mut words: Vec<String> = Vec::new();
        for piece in src.split('"').skip(1).step_by(2) {
            if !piece.is_empty() && piece.bytes().all(|c| c.is_ascii_alphabetic()) {
                words.push(piece.to_string());
            }
        }
        words.extend(crate::ast::FUNCTION_WORDS.iter().map(|w| w.to_string()));
        words.sort();
        words.dedup();
        words
            .into_iter()
            .filter(|w| {
                let probe = format!("\\{w}{{x}}");
                let toks = tokenize(&probe, InputFormat::Latex);
                !toks.iter().any(|t| matches!(t.tok, Tok::Bad(_)))
            })
            .collect()
    }

    fn control_words(s: &str) -> Vec<String> {
        let b = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' {
                let mut j = i + 1;
                while j < b.len() && b[j].is_ascii_alphabetic() {
                    j += 1;
                }
                if j > i + 1 {
                    out.push(s[i + 1..j].to_string());
                }
                i = j.max(i + 2);
            } else {
                i += 1;
            }
        }
        out
    }

    #[test]
    fn every_accepted_control_word_becomes_plain_latex() {
        let words = accepted_control_words();
        // The scan found the commands MathLive emits, not just a few.
        for must in [
            "mleft",
            "exponentialE",
            "doubleprime",
            "lparen",
            "sgn",
            "asin",
            "theta",
        ] {
            assert!(
                words.iter().any(|w| w == must),
                "{must} missing from {words:?}"
            );
        }
        assert!(words.len() > 100, "{}", words.len());
        for w in &words {
            let out = portable_latex(&format!("\\{w}{{x}}"));
            for cw in control_words(&out) {
                assert!(
                    PLAIN_LATEX.contains(&cw.as_str()),
                    "\\{w} became {out:?}, which uses \\{cw}"
                );
            }
        }
    }

    #[test]
    fn portable_latex_rewrites_only_what_latex_lacks() {
        let cases = [
            ("\\mleft(x\\mright)", "\\left(x\\right)"),
            ("\\exponentialE^{x}", "\\mathrm{e}^{x}"),
            ("\\differentialD t", "\\mathrm{d} t"),
            ("x^{\\doubleprime}", "x''"),
            ("x^{\\prime\\prime}", "x''"),
            ("x^\\prime+y", "x'+y"),
            ("\\lparen x\\rparen", "( x)"),
            ("\\left\\lbrack x\\right\\rbrack", "\\left[ x\\right]"),
            ("\\mathnormal{x}", "{x}"),
            ("\\abs{x}", "\\operatorname{abs}{x}"),
            (
                "\\sgn x+\\sign y",
                "\\operatorname{sgn} x+\\operatorname{sgn} y",
            ),
            ("\\cbrt x", "\\operatorname{cbrt} x"),
            ("\\asin x\\acos y\\atan z", "\\arcsin x\\arccos y\\arctan z"),
            ("\\sin\\theta\\cdot 2", "\\sin\\theta\\cdot 2"),
            ("x\\,\\left|y\\right|", "x\\,\\left|y\\right|"),
            ("θ+é", "θ+é"),
            ("\\placeholder[a]{x}", "{x}"),
        ];
        for (from, to) in cases {
            assert_eq!(portable_latex(from), to, "{from}");
        }
    }

    fn lines(ls: &[&str]) -> Vec<String> {
        ls.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn roles_follow_the_left_hand_side() {
        let o = outline(&lines(&[
            "\\dot{x}=\\sigma\\left(y-x\\right)",
            "",
            "y'=x\\left(\\rho-z\\right)-y;\\frac{dz}{dt}=xy-\\beta z",
            "\\sigma=10\\\\\\rho=28",
        ]))
        .unwrap();
        let roles: Vec<String> = o
            .iter()
            .map(|e| match &e.role {
                OutlineRole::Flow { state, order, .. } => format!("{}:flow{order}:{state}", e.line),
                OutlineRole::Parameter { name } => format!("{}:param:{name}", e.line),
                r => format!("{r:?}"),
            })
            .collect();
        assert_eq!(
            roles,
            [
                "0:flow1:x",
                "2:flow1:y",
                "2:flow1:z",
                "3:param:sigma",
                "3:param:rho"
            ]
        );
        assert_eq!(o[0].lhs, "\\dot{x}");
        assert_eq!(o[0].rhs, "\\sigma\\left(y-x\\right)");
        assert_eq!(o[1].lhs, "y'");
        assert_eq!(o[2].lhs, "\\frac{dz}{dt}");
        assert_eq!(o[2].rhs, "xy-\\beta z");
        assert_eq!(o[4].rhs, "28");
    }

    #[test]
    fn subscripts_maps_hamiltonians_and_implicit_lines() {
        let o = outline(&lines(&["\\dot{x}_1=x_2", "\\dot{x}_{2}=-x_1"])).unwrap();
        match &o[0].role {
            OutlineRole::Flow {
                base_latex,
                sub_latex,
                state,
                ..
            } => {
                assert_eq!(state, "x_1");
                assert_eq!(base_latex, "x");
                assert_eq!(sub_latex.as_deref(), Some("1"));
            }
            r => panic!("{r:?}"),
        }
        let o = outline(&lines(&["x_{n+1}=y_n+1-ax_n^2", "y_{n+1}=bx_n"])).unwrap();
        assert!(matches!(&o[0].role, OutlineRole::Map { state, .. } if state == "x"));
        assert_eq!(o[0].rhs, "y_n+1-ax_n^2");
        let o = outline(&lines(&["H\\left(q,p\\right)=\\frac{p^2}{2}-\\cos q"])).unwrap();
        assert_eq!(o[0].role, OutlineRole::Hamiltonian);
        assert_eq!(o[0].lhs, "H\\left(q,p\\right)");
        let o = outline(&lines(&["\\ddot{\\theta}+b\\dot{\\theta}+\\sin\\theta=0"])).unwrap();
        assert!(
            matches!(&o[0].role, OutlineRole::Implicit { state, order: 2, .. } if state == "theta")
        );
        assert_eq!(o[0].rhs, "0");
        let o = outline(&lines(&["\\ddot{\\theta}=-\\sin\\theta"])).unwrap();
        assert!(matches!(&o[0].role, OutlineRole::Flow { order: 2, .. }));
    }

    #[test]
    fn alignment_marks_and_wrappers_are_dropped() {
        let o = outline(&lines(&[
            "\\begin{cases}\\dot{x}&=\\mleft(y\\mright)\\\\\\dot{y}&=-x\\end{cases}",
        ]))
        .unwrap();
        assert_eq!(o.len(), 2);
        assert_eq!(o[0].lhs, "\\dot{x}");
        assert_eq!(o[0].rhs, "\\left(y\\right)");
        assert_eq!(o[1].rhs, "-x");
    }

    #[test]
    fn a_system_with_errors_has_no_outline() {
        let e = outline(&lines(&["\\dot{x}=+"])).unwrap_err();
        assert!(!e.is_empty() && e.iter().all(Diagnostic::is_error));
        assert!(outline(&lines(&["", "  "])).is_err());
    }
}
