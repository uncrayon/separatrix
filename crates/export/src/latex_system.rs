//! The system as a LaTeX `align` environment, in Strogatz notation.
//!
//! The right-hand sides are the user's own LaTeX, made portable
//! ([`dyn_expr::portable_latex`]), with every time derivative in them
//! rewritten in the chosen notation; the left-hand sides are rebuilt in that
//! notation from each state's letter (in the font it was typed in) and
//! subscript, so a subscripted state is `\dot{x}_{1}` (never `\dot{x_1}`).
//! An equation not solved for its derivative keeps both sides as typed, its
//! derivatives rewritten. A map's next iterate is
//! `x_{1,n+1}` (never the double subscript `x_1_{n+1}`). Parameter-value
//! lines leave the align; the current values follow it, in a sentence for
//! up to three parameters and in a booktabs table above that.

use dyn_expr::{
    convert, outline, EquationOutline, InputFormat, LatexPart, OutlineRole, SystemInfo,
};
use serde::{Deserialize, Serialize};

use crate::latex_util::{comment, packages_comment, plain_equations, slug};
use crate::num;
use crate::words::join_and;

/// How derivatives are written on the left-hand side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Notation {
    /// `\dot{x}`, `\ddot{\theta}`, `\dddot{x}`, `x^{(4)}` (Strogatz).
    Dot,
    /// `x'`, `x''`.
    Prime,
    /// `\frac{dx}{dt}`.
    Leibniz,
}

/// Parameters up to which the values are given in a sentence.
pub const MAX_INLINE_PARAMS: usize = 3;

/// Significant digits of a parameter value.
const PARAM_DIGITS: usize = 6;

/// What [`system`] needs.
#[derive(Debug, Clone)]
pub struct SystemInput<'a> {
    /// The lines as typed (empty lines allowed).
    pub lines: &'a [String],
    /// Their input format.
    pub format: InputFormat,
    /// What the compiler understood (states, parameters, the first-order
    /// display form used when text can't be written in LaTeX).
    pub info: &'a SystemInfo,
    /// The parameter values in use, in `info.params` order.
    pub params: &'a [f64],
    /// Notation of the left-hand sides.
    pub notation: Notation,
    /// Number the equations (`align` with labels) or not (`align*`).
    pub numbered: bool,
    /// The system's title (labels and the first comment line).
    pub title: &'a str,
    /// Where it comes from, e.g. "Strogatz §6.4".
    pub source_ref: Option<&'a str>,
}

/// The snippet and its preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemLatex {
    /// The snippet to paste.
    pub tex: String,
    /// The equations as one `aligned` block (no labels, no environment
    /// around it), for the dialog's typeset preview.
    pub preview_latex: String,
}

/// One row of the align.
struct Row {
    lhs: String,
    rhs: String,
    label: Option<String>,
}

/// `base` with the subscript `sub` (or none).
fn with_sub(base: &str, sub: Option<&str>) -> String {
    match sub {
        Some(s) => format!("{base}_{{{s}}}"),
        None => base.to_string(),
    }
}

/// The left-hand side of `d^k state / dt^k` in `notation`.
pub fn derivative_lhs(base: &str, sub: Option<&str>, order: u32, notation: Notation) -> String {
    let order = order.max(1);
    match notation {
        Notation::Dot => match order {
            1 => with_sub(&format!("\\dot{{{base}}}"), sub),
            2 => with_sub(&format!("\\ddot{{{base}}}"), sub),
            3 => with_sub(&format!("\\dddot{{{base}}}"), sub),
            k => format!("{}^{{({k})}}", with_sub(base, sub)),
        },
        Notation::Prime => match order {
            k @ 1..=3 => format!("{}{}", with_sub(base, sub), "'".repeat(k as usize)),
            k => format!("{}^{{({k})}}", with_sub(base, sub)),
        },
        Notation::Leibniz => {
            let x = with_sub(base, sub);
            match order {
                1 => format!("\\frac{{d{x}}}{{dt}}"),
                k => format!("\\frac{{d^{{{k}}}{x}}}{{dt^{{{k}}}}}"),
            }
        }
    }
}

/// A map's next iterate, `x_{n+1}` or `x_{1,n+1}`.
pub fn next_iterate(base: &str, sub: Option<&str>) -> String {
    match sub {
        Some(s) => format!("{base}_{{{s},n+1}}"),
        None => format!("{base}_{{n+1}}"),
    }
}

/// `s` ends with a control word such as `\cdot`.
fn ends_in_control_word(s: &str) -> bool {
    let letters = s.len() - s.trim_end_matches(|c: char| c.is_ascii_alphabetic()).len();
    letters > 0 && s[..s.len() - letters].ends_with('\\')
}

/// One side of an equation, its derivatives written in `notation`.
fn render_parts(parts: &[LatexPart], notation: Notation) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            LatexPart::Text { text } => out.push_str(text),
            LatexPart::Derivative {
                base_latex,
                sub_latex,
                order,
                of_t,
                ..
            } => {
                let d = derivative_lhs(base_latex, sub_latex.as_deref(), *order, notation);
                // `\cdot\dot{x}` in prime notation is `\cdot x'`, not `\cdotx'`.
                if d.starts_with(|c: char| c.is_ascii_alphabetic()) && ends_in_control_word(&out) {
                    out.push(' ');
                }
                out.push_str(&d);
                if *of_t {
                    out.push_str("(t)");
                }
            }
        }
    }
    out
}

fn rows_from_outline(o: &[EquationOutline], notation: Notation, title_slug: &str) -> Vec<Row> {
    o.iter()
        .filter_map(|e| {
            let (lhs, label) = match &e.role {
                OutlineRole::Parameter { .. } => return None,
                OutlineRole::Flow {
                    state,
                    base_latex,
                    sub_latex,
                    order,
                    ..
                } => (
                    derivative_lhs(base_latex, sub_latex.as_deref(), *order, notation),
                    slug(state, "x"),
                ),
                OutlineRole::Map {
                    state,
                    base_latex,
                    sub_latex,
                    ..
                } => (
                    next_iterate(base_latex, sub_latex.as_deref()),
                    slug(state, "x"),
                ),
                OutlineRole::Implicit { state, .. } => {
                    (render_parts(&e.lhs_parts, notation), slug(state, "x"))
                }
                OutlineRole::Hamiltonian => (e.lhs.clone(), "hamiltonian".to_string()),
            };
            Some(Row {
                lhs,
                rhs: render_parts(&e.rhs_parts, notation),
                label: Some(format!("eq:{title_slug}-{label}")),
            })
        })
        .collect()
}

/// The first-order form the compiler shows (`\dot{x} = …`), for a system
/// whose lines can't all be written in LaTeX.
fn rows_from_display(info: &SystemInfo, title_slug: &str) -> Vec<Row> {
    info.equations_latex
        .iter()
        .enumerate()
        .map(|(i, eq)| {
            let (lhs, rhs) = eq.split_once(" = ").unwrap_or((eq.as_str(), ""));
            let name = info.states.get(i).map_or("x", |s| s.name.as_str());
            Row {
                lhs: dyn_expr::portable_latex(lhs),
                rhs: dyn_expr::portable_latex(rhs),
                label: Some(format!("eq:{title_slug}-{}", slug(name, "x"))),
            }
        })
        .collect()
}

/// The header comment lines shared by the LaTeX snippets: the title, the
/// equations as plain text and the parameter values.
pub fn header_comments(
    kind_word: &str,
    title: &str,
    source_ref: Option<&str>,
    lines: &[String],
    format: InputFormat,
    params: &[(String, f64)],
) -> String {
    let mut out = String::new();
    let mut first = format!(
        "Separatrix {kind_word}: {}",
        if title.trim().is_empty() {
            "Untitled system"
        } else {
            title.trim()
        }
    );
    if let Some(r) = source_ref.filter(|r| !r.trim().is_empty()) {
        first.push_str(&format!(" ({})", r.trim()));
    }
    out.push_str(&comment(&first));
    for eq in plain_equations(lines, format) {
        out.push_str(&comment(&format!("  {eq}")));
    }
    if !params.is_empty() {
        let vals: Vec<String> = params
            .iter()
            .map(|(n, v)| format!("{n} = {}", num::shortest(*v)))
            .collect();
        out.push_str(&comment(&format!("Parameters: {}", vals.join(", "))));
    }
    out
}

/// The system as a LaTeX snippet.
pub fn system(input: &SystemInput) -> SystemLatex {
    let title_slug = slug(input.title, "system");
    let mut notes: Vec<String> = Vec::new();

    let latex_lines: Option<Vec<String>> = match input.format {
        InputFormat::Latex => Some(input.lines.to_vec()),
        InputFormat::Text => {
            let conv = convert(input.lines, InputFormat::Text, InputFormat::Latex);
            if let Some((i, c)) = conv.iter().enumerate().find(|(_, c)| !c.converted) {
                notes.push(format!(
                    "Line {} ({}) can't be written in LaTeX exactly ({}), so these are the \
                     equations as Separatrix reads them.",
                    i + 1,
                    input.lines[i].trim(),
                    c.note.as_deref().unwrap_or("not converted")
                ));
                None
            } else {
                Some(conv.into_iter().map(|c| c.source).collect())
            }
        }
    };
    let rows = match latex_lines.as_deref().map(outline) {
        Some(Ok(o)) => rows_from_outline(&o, input.notation, &title_slug),
        Some(Err(_)) => {
            notes.push(
                "The equations don't compile as typed, so these are the equations as \
                 Separatrix reads them."
                    .into(),
            );
            rows_from_display(input.info, &title_slug)
        }
        None => rows_from_display(input.info, &title_slug),
    };

    let params: Vec<(String, String, f64)> = input
        .info
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                p.name.clone(),
                p.latex.clone(),
                input.params.get(i).copied().unwrap_or(p.default),
            )
        })
        .collect();
    let table = params.len() > MAX_INLINE_PARAMS;

    let mut tex = header_comments(
        "system",
        input.title,
        input.source_ref,
        input.lines,
        input.format,
        &params
            .iter()
            .map(|(n, _, v)| (n.clone(), *v))
            .collect::<Vec<_>>(),
    );
    for n in &notes {
        tex.push_str(&comment(n));
    }
    let mut packages = vec!["amsmath"];
    if table {
        packages.extend(["booktabs", "siunitx"]);
    }
    tex.push_str(&packages_comment(&packages));

    let env = if input.numbered { "align" } else { "align*" };
    let last_punct = if !params.is_empty() && !table {
        ","
    } else {
        "."
    };
    tex.push_str(&format!("\\begin{{{env}}}\n"));
    let mut preview = String::from("\\begin{aligned}");
    for (i, r) in rows.iter().enumerate() {
        let last = i + 1 == rows.len();
        let punct = if last { last_punct } else { "," };
        let label = match (&r.label, input.numbered) {
            (Some(l), true) => format!(" \\label{{{l}}}"),
            _ => String::new(),
        };
        let end = if last { "" } else { " \\\\" };
        tex.push_str(&format!("  {} &= {}{punct}{label}{end}\n", r.lhs, r.rhs));
        preview.push_str(&format!("{} &= {}{punct}", r.lhs, r.rhs));
        if !last {
            preview.push_str("\\\\");
        }
    }
    preview.push_str("\\end{aligned}");
    tex.push_str(&format!("\\end{{{env}}}\n"));

    if !params.is_empty() {
        if table {
            tex.push_str("\\begin{center}\n\\begin{tabular}{@{}ll@{}}\n\\toprule\nParameter & Value \\\\\n\\midrule\n");
            for (_, latex, v) in &params {
                tex.push_str(&format!("${latex}$ & {} \\\\\n", num::si(*v, PARAM_DIGITS)));
            }
            tex.push_str("\\bottomrule\n\\end{tabular}\n\\end{center}\n");
        } else {
            let items: Vec<String> = params
                .iter()
                .map(|(_, latex, v)| format!("${latex} = {}$", num::tex(*v, PARAM_DIGITS)))
                .collect();
            tex.push_str(&format!("where {}.\n", join_and(&items)));
        }
    }
    SystemLatex {
        tex,
        preview_latex: preview,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_hand_sides_in_every_notation() {
        use Notation::*;
        let cases = [
            (("x", None, 1, Dot), "\\dot{x}"),
            (("\\theta", None, 2, Dot), "\\ddot{\\theta}"),
            (("x", Some("1"), 1, Dot), "\\dot{x}_{1}"),
            (("x", Some("1"), 2, Dot), "\\ddot{x}_{1}"),
            (("x", None, 3, Dot), "\\dddot{x}"),
            (("x", None, 4, Dot), "x^{(4)}"),
            (("x", Some("1"), 4, Dot), "x_{1}^{(4)}"),
            (("x", None, 1, Prime), "x'"),
            (("x", None, 2, Prime), "x''"),
            (("x", Some("1"), 1, Prime), "x_{1}'"),
            (("x", None, 4, Prime), "x^{(4)}"),
            (("x", None, 1, Leibniz), "\\frac{dx}{dt}"),
            (("x", None, 2, Leibniz), "\\frac{d^{2}x}{dt^{2}}"),
            (("x", Some("1"), 1, Leibniz), "\\frac{dx_{1}}{dt}"),
        ];
        for ((b, s, k, n), want) in cases {
            assert_eq!(derivative_lhs(b, s, k, n), want);
        }
        assert_eq!(next_iterate("x", None), "x_{n+1}");
        assert_eq!(next_iterate("x", Some("1")), "x_{1,n+1}");
    }
}
