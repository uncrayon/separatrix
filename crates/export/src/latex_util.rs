//! Small pieces every LaTeX export shares: escaping, comments, label slugs,
//! units and the plain-text form of the equations.

use dyn_expr::{convert, InputFormat};

/// One CSS pixel in TeX big points (1 px = 1/96 in, 1 bp = 1/72 in).
pub const BP_PER_PX: f64 = 0.75;

/// Figure pixels per centimetre (96 px per inch).
pub const PX_PER_CM: f64 = 96.0 / 2.54;

/// `px` CSS pixels in big points, as written in a length (`"1.125bp"`).
pub fn bp(px: f64) -> String {
    format!("{}bp", fixed_trim(px * BP_PER_PX, 4))
}

/// `v` with at most `decimals` decimals and no trailing zeros, ASCII minus,
/// never `-0`.
pub fn fixed_trim(v: f64, decimals: usize) -> String {
    let s = crate::num::to_fixed(v, decimals);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// Greek letters typed as Unicode, for text that LaTeX's default input
/// encoding can't take.
fn greek_command(c: char) -> Option<&'static str> {
    Some(match c {
        'α' => "alpha",
        'β' => "beta",
        'γ' => "gamma",
        'δ' => "delta",
        'ε' => "varepsilon",
        'ϵ' => "epsilon",
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
        'φ' => "varphi",
        'ϕ' => "phi",
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
        _ => return None,
    })
}

/// `s` as LaTeX text (a caption, a title): the special characters
/// `& % $ # _ { } ~ ^ \` escaped, `§` as `\S{}`, en and em dashes as `--`
/// and `---`, the minus sign, `×` and Greek letters as maths, and other
/// characters outside Latin-1 and Latin Extended-A (which pdfLaTeX's
/// default input encoding reads) dropped.
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\\' => out.push_str("\\textbackslash{}"),
            '§' => out.push_str("\\S{}"),
            '–' => out.push_str("--"),
            '—' => out.push_str("---"),
            '−' => out.push_str("$-$"),
            '×' => out.push_str("$\\times$"),
            '‘' => out.push('`'),
            '’' => out.push('\''),
            '“' => out.push_str("``"),
            '”' => out.push_str("''"),
            '…' => out.push_str("\\dots{}"),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => {
                if let Some(g) = greek_command(c) {
                    out.push_str(&format!("$\\{g}$"));
                } else if (c as u32) < 0x20 {
                    // Control characters have no place in text.
                } else if (c as u32) <= 0x17F {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// `s` as one comment line's text: newlines become spaces, U+2212 becomes
/// `-`, and everything else stays (TeX skips the rest of a comment line).
pub fn comment_text(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\r' => ' ',
            '\u{2212}' => '-',
            c => c,
        })
        .collect()
}

/// A comment line (`% …\n`).
pub fn comment(s: &str) -> String {
    let t = comment_text(s);
    if t.is_empty() {
        "%\n".into()
    } else {
        format!("% {t}\n")
    }
}

fn ascii_fold(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "a",
        'æ' | 'Æ' => "ae",
        'ç' | 'Ç' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => "i",
        'ñ' | 'Ñ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => "o",
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => "u",
        'ý' | 'ÿ' | 'Ý' => "y",
        'ß' => "ss",
        '\'' => "-prime",
        _ => return None,
    })
}

/// The slug of a document title, where an apostrophe separates words
/// (`"Sel'kov glycolysis"` → `sel-kov-glycolysis`) instead of marking a
/// derivative as in [`slug`]; `system` when nothing is left.
pub fn title_slug(title: &str) -> String {
    slug(&title.replace(['\'', '’'], " "), "system")
}

/// A label or file-name slug: lowercase `[a-z0-9-]`, words joined by single
/// hyphens (`"Rabbits vs sheep"` → `rabbits-vs-sheep`, `"theta'"` →
/// `theta-prime`, `"Hénon map"` → `henon-map`). `fallback` when nothing is
/// left.
pub fn slug(s: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    let mut push = |piece: &str, out: &mut String| {
        for c in piece.chars() {
            if c.is_ascii_alphanumeric() {
                if pending_dash && !out.is_empty() {
                    out.push('-');
                }
                pending_dash = false;
                out.push(c.to_ascii_lowercase());
            } else {
                pending_dash = true;
            }
        }
    };
    for c in s.chars() {
        if let Some(f) = ascii_fold(c) {
            push(f, &mut out);
        } else if c.is_ascii_alphanumeric() {
            push(c.encode_utf8(&mut [0; 4]), &mut out);
        } else if let Some(g) = greek_command(c) {
            push(&format!("-{g}-"), &mut out);
        } else {
            push("-", &mut out);
        }
    }
    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

/// A tick label (`tickLabel`'s text) for TeX maths: the real minus sign
/// becomes `-`, and `1.5e-7` becomes `1.5\cdot 10^{-7}`.
pub fn tex_tick_label(label: &str) -> String {
    let ascii = label.replace('\u{2212}', "-");
    match ascii.split_once('e') {
        Some((m, e)) => {
            let e = e.trim_start_matches('+');
            format!("{m}\\cdot 10^{{{e}}}")
        }
        None => ascii,
    }
}

/// The equations as plain text, one per line, for the comment lines that
/// open every snippet (`x' = x*(3 - x - 2*y)`). A line that can't be
/// converted is given as typed.
pub fn plain_equations(lines: &[String], format: InputFormat) -> Vec<String> {
    let lines: Vec<String> = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .cloned()
        .collect();
    match format {
        InputFormat::Text => lines,
        InputFormat::Latex => convert(&lines, InputFormat::Latex, InputFormat::Text)
            .into_iter()
            .map(|c| c.source)
            .collect(),
    }
}

/// A `\usepackage` line for each package, as a comment.
pub fn packages_comment(packages: &[&str]) -> String {
    let uses: Vec<String> = packages
        .iter()
        .map(|p| format!("\\usepackage{{{p}}}"))
        .collect();
    comment(&format!("Needs: {}", uses.join(" ")))
}

/// `\definecolor{name}{HTML}{RRGGBB}` for a `#rrggbb` colour.
pub fn define_color(name: &str, hex: &str) -> String {
    format!(
        "\\definecolor{{{name}}}{{HTML}}{{{}}}%\n",
        hex.trim_start_matches('#').to_ascii_uppercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captions_escape_every_special_character() {
        assert_eq!(
            escape_text("A&B 50% $5 #1 a_b {x} ~ ^ \\ § 2–3 a—b"),
            "A\\&B 50\\% \\$5 \\#1 a\\_b \\{x\\} \\textasciitilde{} \\textasciicircum{} \
             \\textbackslash{} \\S{} 2--3 a---b"
        );
        assert_eq!(escape_text("Hénon −1 σ\n"), "Hénon $-$1 $\\sigma$ ");
        assert_eq!(escape_text("日本"), "");
    }

    #[test]
    fn slugs_and_comments() {
        assert_eq!(slug("Rabbits vs sheep", "system"), "rabbits-vs-sheep");
        assert_eq!(slug("theta'", "x"), "theta-prime");
        assert_eq!(slug("x''", "x"), "x-prime-prime");
        assert_eq!(slug("x_1", "x"), "x-1");
        assert_eq!(slug("Hénon map (§12.2)", "system"), "henon-map-12-2");
        assert_eq!(slug("Rössler", "system"), "rossler");
        assert_eq!(slug("σ", "x"), "sigma");
        assert_eq!(slug("  ", "system"), "system");
        assert_eq!(title_slug("Sel'kov glycolysis"), "sel-kov-glycolysis");
        assert_eq!(title_slug("Sel’kov"), "sel-kov");
        assert_eq!(comment("a\nb − c"), "% a b - c\n");
        assert_eq!(bp(1.5), "1.125bp");
        assert_eq!(bp(8.0), "6bp");
        assert_eq!(fixed_trim(-0.00001, 3), "0");
    }

    #[test]
    fn tick_labels_for_tex() {
        assert_eq!(tex_tick_label("\u{2212}20"), "-20");
        assert_eq!(tex_tick_label("1.5e-7"), "1.5\\cdot 10^{-7}");
        assert_eq!(tex_tick_label("-2.5e5"), "-2.5\\cdot 10^{5}");
        assert_eq!(tex_tick_label("0.25"), "0.25");
    }
}
