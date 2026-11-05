//! The fixed points as a booktabs table: the point, its eigenvalues (a
//! complex pair written once as `a ± bi`), the trace τ and determinant Δ
//! for 2D flows, and Strogatz's type. Maps list the multipliers μ and |μ|;
//! 1D flows list λ = f′(x*) only.

use dyn_expr::InputFormat;

use crate::latex_system::header_comments;
use crate::latex_util::{comment, escape_text, packages_comment, slug};
use crate::model::{FixedPoint, Labels, SystemKind};
use crate::num;
use crate::words::{classification_title, join_and};

/// Default significant digits.
pub const DEFAULT_DIGITS: usize = 4;

/// Values below this fraction of the largest in their column print as 0.
const SNAP: f64 = 1e-12;

/// What [`table`] needs.
#[derive(Debug, Clone)]
pub struct FixedPointsInput<'a> {
    /// Flow or map.
    pub kind: SystemKind,
    /// State and parameter names, with the parameter values.
    pub labels: &'a Labels,
    /// The fixed points the screen shows.
    pub fixed_points: &'a [FixedPoint],
    /// The box searched, one `(lo, hi)` per state.
    pub search: &'a [(f64, f64)],
    /// The lines as typed, for the comment.
    pub lines: &'a [String],
    /// Their format.
    pub format: InputFormat,
    /// Title (label and comment).
    pub title: &'a str,
    /// Where the system comes from.
    pub source_ref: Option<&'a str>,
    /// Significant digits, 2 to 8.
    pub digits: usize,
}

/// `v` as 0 when it is below `SNAP` × `scale`.
fn snap(v: f64, scale: f64) -> f64 {
    if v.abs() < SNAP * scale.max(1.0) {
        0.0
    } else {
        v
    }
}

fn column_scale(values: impl Iterator<Item = f64>) -> f64 {
    values
        .filter(|v| v.is_finite())
        .fold(0.0, |m, v| m.max(v.abs()))
}

/// The eigenvalues as table text: real ones as `$\num{-1}$`, a complex
/// pair once as `$\num{-0.5} \pm \num{1.32}\,i$`.
fn eigen_cell(ev: &[(f64, f64)], scale: f64, digits: usize) -> String {
    let mut used = vec![false; ev.len()];
    let mut parts = Vec::new();
    for i in 0..ev.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let (re, im) = (snap(ev[i].0, scale), snap(ev[i].1, scale));
        if im == 0.0 {
            parts.push(format!("${}$", num::si(re, digits)));
            continue;
        }
        let partner = (i + 1..ev.len())
            .find(|&j| !used[j] && snap(ev[j].0, scale) == re && snap(ev[j].1, scale) == -im);
        let re_text = if re == 0.0 {
            String::new()
        } else {
            format!("{} ", num::si(re, digits))
        };
        match partner {
            Some(j) => {
                used[j] = true;
                parts.push(format!("${re_text}\\pm {}\\,i$", num::si(im.abs(), digits)));
            }
            None => {
                let sign = if im < 0.0 {
                    "-"
                } else if re == 0.0 {
                    ""
                } else {
                    "+"
                };
                parts.push(
                    format!("${re_text}{sign} {}\\,i$", num::si(im.abs(), digits))
                        .replace("$ ", "$"),
                );
            }
        }
    }
    parts.join(", ")
}

/// τ and Δ from the eigenvalues of a 2D flow.
fn trace_det(ev: &[(f64, f64)]) -> Option<(f64, f64)> {
    if ev.len() != 2 {
        return None;
    }
    let (a, b) = ev[0];
    let (c, d) = ev[1];
    Some((a + c, a * c - b * d))
}

/// The table, with its leading comment lines.
pub fn table(input: &FixedPointsInput) -> String {
    let digits = input.digits.clamp(2, 8);
    let dim = input.labels.states.len();
    let is_map = input.kind == SystemKind::Map;
    let two_d_flow = !is_map && dim == 2;
    let title_slug = slug(input.title, "system");
    let params: Vec<(String, f64)> = input
        .labels
        .params
        .iter()
        .map(|p| (p.name.clone(), p.value))
        .collect();

    let mut tex = header_comments(
        "fixed points",
        input.title,
        input.source_ref,
        input.lines,
        input.format,
        &params,
    );
    tex.push_str(&packages_comment(&["amsmath", "booktabs", "siunitx"]));

    let fps = input.fixed_points;
    let coord_scale = column_scale(fps.iter().flat_map(|f| f.x.iter().copied()));
    let ev_scale = column_scale(
        fps.iter()
            .flat_map(|f| f.eigenvalues.iter().flat_map(|&(r, i)| [r, i])),
    );
    let td: Vec<Option<(f64, f64)>> = fps.iter().map(|f| trace_det(&f.eigenvalues)).collect();
    let tau_scale = column_scale(td.iter().flatten().map(|t| t.0));
    let det_scale = column_scale(td.iter().flatten().map(|t| t.1));
    let mods: Vec<Vec<f64>> = fps
        .iter()
        .map(|f| f.eigenvalues.iter().map(|&(r, i)| r.hypot(i)).collect())
        .collect();
    let mod_scale = column_scale(mods.iter().flatten().copied());

    let (header, spec): (Vec<&str>, &str) = if is_map {
        (
            vec![
                "Fixed point",
                "Multipliers $\\mu$",
                "$\\lvert\\mu\\rvert$",
                "Type",
            ],
            "@{}llll@{}",
        )
    } else if two_d_flow {
        (
            vec![
                "Fixed point",
                "Eigenvalues $\\lambda$",
                "$\\tau$",
                "$\\Delta$",
                "Type",
            ],
            "@{}llrrl@{}",
        )
    } else if dim == 1 {
        (vec!["Fixed point", "$\\lambda$", "Type"], "@{}lll@{}")
    } else {
        (
            vec!["Fixed point", "Eigenvalues $\\lambda$", "Type"],
            "@{}lll@{}",
        )
    };

    let mut caption = String::from("Fixed points of the system");
    if !input.labels.params.is_empty() {
        let items: Vec<String> = input
            .labels
            .params
            .iter()
            .map(|p| format!("${} = {}$", p.latex, num::tex(p.value, 6)))
            .collect();
        caption.push_str(&format!(" with {}", join_and(&items)));
    }
    let ranges: Vec<String> = input
        .labels
        .states
        .iter()
        .zip(input.search)
        .map(|(s, &(lo, hi))| {
            format!(
                "${} \\le {} \\le {}$",
                num::tex(lo, 6),
                s.latex,
                num::tex(hi, 6)
            )
        })
        .collect();
    if !ranges.is_empty() {
        caption.push_str(&format!(", found in {}", join_and(&ranges)));
    }
    caption.push('.');

    tex.push_str("\\begin{table}\n\\centering\n");
    tex.push_str(&format!("\\caption{{{caption}}}\n"));
    tex.push_str(&format!("\\label{{tab:{title_slug}-fixed-points}}\n"));
    tex.push_str(&format!("\\begin{{tabular}}{{{spec}}}\n\\toprule\n"));
    tex.push_str(&format!("{} \\\\\n\\midrule\n", header.join(" & ")));
    if fps.is_empty() {
        tex.push_str(&format!(
            "\\multicolumn{{{}}}{{@{{}}l@{{}}}}{{No fixed points found in this region.}} \\\\\n",
            header.len()
        ));
    }
    for (k, fp) in fps.iter().enumerate() {
        let point: Vec<String> =
            fp.x.iter()
                .map(|&v| num::si(snap(v, coord_scale), digits))
                .collect();
        let point = if point.len() == 1 {
            format!("${}$", point[0])
        } else {
            format!("$({})$", point.join(", "))
        };
        let ev = if fp.eigenvalues.is_empty() {
            "--".to_string()
        } else {
            eigen_cell(&fp.eigenvalues, ev_scale, digits)
        };
        let kind = escape_text(classification_title(fp.classification, dim));
        let mut cells = vec![point, ev];
        if is_map {
            let m: Vec<String> = mods[k]
                .iter()
                .map(|&v| num::si(snap(v, mod_scale), digits))
                .collect();
            cells.push(if m.is_empty() {
                "--".into()
            } else {
                m.join(", ")
            });
        } else if two_d_flow {
            match td[k] {
                Some((t, d)) => {
                    cells.push(num::si(snap(t, tau_scale), digits));
                    cells.push(num::si(snap(d, det_scale), digits));
                }
                None => cells.extend(["--".to_string(), "--".to_string()]),
            }
        }
        cells.push(kind);
        tex.push_str(&format!("{} \\\\\n", cells.join(" & ")));
    }
    tex.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n");
    if fps.is_empty() {
        tex.insert_str(0, &comment("No fixed points were found in the view."));
    }
    tex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_pairs_are_written_once() {
        let ev = [(-0.5, 1.3228756555322954), (-0.5, -1.3228756555322954)];
        assert_eq!(
            eigen_cell(&ev, 1.32, 3),
            "$\\num{-0.5} \\pm \\num{1.32}\\,i$"
        );
        let centre = [(1e-17, 2.0), (1e-17, -2.0)];
        assert_eq!(eigen_cell(&centre, 2.0, 4), "$\\pm \\num{2}\\,i$");
        let real = [(-1.0, 0.0), (-2.0, 0.0)];
        assert_eq!(eigen_cell(&real, 2.0, 4), "$\\num{-1}$, $\\num{-2}$");
        let lone = [(1.0, -2.0)];
        assert_eq!(eigen_cell(&lone, 2.0, 4), "$\\num{1} - \\num{2}\\,i$");
        assert_eq!(trace_det(&ev).map(|(t, _)| t), Some(-1.0));
        let (_, d) = trace_det(&ev).unwrap();
        assert!((d - 2.0).abs() < 1e-12);
    }
}
