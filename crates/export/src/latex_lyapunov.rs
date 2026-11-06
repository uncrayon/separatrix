//! A Lyapunov spectrum as a booktabs table (Strogatz §9.3, §10.5): one row
//! per exponent with its standard error, `\num{0.906 +- 0.004}` under
//! `\sisetup{uncertainty-mode = separate}` (siunitx prints `0.906 ± 0.004`),
//! then `Σλ` against what it must equal, the time average of `tr J` along
//! the orbit (a map: the mean of `ln |det J|`), and their difference, the
//! integrator's error on the tangent vectors.
//!
//! The error is rounded to one significant digit (two when that digit is a
//! one) and the value to the same decimal place, from their exact decimal
//! expansions ([`num::with_uncertainty`]), so the table is the same text on
//! every platform. The caption says how the run was made: the system and
//! its parameters, the integrator with its step or tolerances, the total
//! time and the transient, and how many renormalizations, how far apart.

use crate::latex_system::header_comments;
use crate::latex_util::{escape_text, packages_comment, title_slug};
use crate::model::{LyapunovRun, Method, Status, SystemKind, SystemMeta};
use crate::num;
use crate::words::{join_and, status_word};

const PLAIN_DIGITS: usize = 6;

/// The integrator's name in a sentence, as the method picker names it.
pub fn method_words(m: Method) -> &'static str {
    match m {
        Method::Euler => "Euler's method",
        Method::Heun => "improved Euler (Heun)",
        Method::Rk4 => "Runge--Kutta 4",
        Method::Dopri5 => "Dormand--Prince 5(4)",
        Method::Verlet => "St\\\"ormer--Verlet",
        Method::Yoshida4 => "Yoshida 4",
        Method::Rosenbrock23 => "Rosenbrock 2(3)",
    }
}

fn m(v: f64) -> String {
    num::tex(v, PLAIN_DIGITS)
}

fn count(n: usize) -> String {
    format!("\\num{{{n}}}")
}

fn caption(sys: &SystemMeta, run: &LyapunovRun) -> String {
    let map = sys.kind == SystemKind::Map;
    let s = &run.spectrum;
    let title = sys.title.trim();
    let mut out = if title.is_empty() {
        "Lyapunov spectrum of the system".to_string()
    } else {
        let the = if title.to_lowercase().starts_with("the ") {
            ""
        } else {
            "the "
        };
        format!("Lyapunov spectrum of {the}{}", escape_text(title))
    };
    let params: Vec<String> = sys
        .labels
        .params
        .iter()
        .map(|p| format!("${} = {}$", p.latex, m(p.value)))
        .collect();
    if !params.is_empty() {
        out.push_str(&format!(" with {}", join_and(&params)));
    }
    let names: Vec<&str> = sys.labels.states.iter().map(|s| s.latex.as_str()).collect();
    let start: Vec<String> = run.x0.iter().map(|&v| m(v)).collect();
    if names.len() == 1 && start.len() == 1 {
        out.push_str(&format!(", from ${}_0 = {}$", names[0], start[0]));
    } else if names.len() == start.len() && !names.is_empty() {
        out.push_str(&format!(
            ", from $({}) = ({})$",
            names.join(", "),
            start.join(", ")
        ));
    }
    let every = if map {
        let k = run.interval.round().max(1.0);
        if k == 1.0 {
            "every iterate".to_string()
        } else {
            format!("every ${}$ iterates", m(k))
        }
    } else {
        format!("every $\\tau = {}$", m(run.interval))
    };
    if map {
        out.push_str(&format!(
            ": the map iterated ${}$ times after a transient of ${}$ iterates",
            m(run.total.round()),
            m(run.transient.round())
        ));
    } else {
        let o = &run.solver;
        let how = if o.method.is_adaptive() {
            format!(
                "relative tolerance ${}$ and absolute tolerance ${}$",
                m(o.rtol),
                m(o.atol)
            )
        } else {
            format!("step $h = {}$", m(o.h))
        };
        out.push_str(&format!(
            ": {} with {how}, total time $T = {}$ after a transient of ${}$",
            method_words(o.method),
            m(run.total),
            m(run.transient)
        ));
    }
    out.push_str(&format!(
        ", {} renormalizations {every}.",
        count(s.renormalizations)
    ));
    if s.uncertainty.len() == s.exponents.len() && !s.exponents.is_empty() {
        out.push_str(&format!(
            " Uncertainties are standard errors over {} blocks.",
            run.blocks
        ));
    } else {
        out.push_str(" No uncertainties: fewer than two blocks were completed.");
    }
    if s.status != Status::Completed {
        out.push_str(&format!(
            " The run stopped early ({}).",
            status_word(s.status)
        ));
    }
    out
}

/// The table, with its leading comment lines.
pub fn lyapunov_latex(sys: &SystemMeta, run: &LyapunovRun) -> String {
    let s = &run.spectrum;
    let params: Vec<(String, f64)> = sys
        .labels
        .params
        .iter()
        .map(|p| (p.name.clone(), p.value))
        .collect();
    let mut tex = header_comments(
        "Lyapunov spectrum",
        sys.title,
        sys.source_ref,
        sys.lines,
        sys.format,
        &params,
    );
    tex.push_str(&packages_comment(&["amsmath", "booktabs", "siunitx"]));
    tex.push_str("\\begin{table}\n\\centering\n");
    tex.push_str("\\sisetup{uncertainty-mode = separate}\n");
    tex.push_str(&format!("\\caption{{{}}}\n", caption(sys, run)));
    tex.push_str(&format!(
        "\\label{{tab:{}-lyapunov}}\n",
        title_slug(sys.title)
    ));
    tex.push_str("\\begin{tabular}{@{}ll@{}}\n\\toprule\nExponent & Value \\\\\n\\midrule\n");
    if s.exponents.is_empty() {
        tex.push_str("\\multicolumn{2}{@{}l@{}}{No renormalization was completed.} \\\\\n");
    }
    for (i, &v) in s.exponents.iter().enumerate() {
        let cell = match s
            .uncertainty
            .get(i)
            .and_then(|&u| num::with_uncertainty(v, u))
        {
            Some((a, b)) => format!("\\num{{{a} +- {b}}}"),
            None => num::si(v, PLAIN_DIGITS),
        };
        tex.push_str(&format!("$\\lambda_{{{}}}$ & {cell} \\\\\n", i + 1));
    }
    if !s.exponents.is_empty() {
        let trace = if sys.kind == SystemKind::Map {
            "$\\langle \\ln\\lvert\\det J\\rvert \\rangle$"
        } else {
            "$\\langle \\operatorname{tr} J \\rangle$"
        };
        tex.push_str("\\midrule\n");
        tex.push_str(&format!(
            "$\\sum_i \\lambda_i$ & {} \\\\\n",
            num::si(s.sum, PLAIN_DIGITS)
        ));
        tex.push_str(&format!(
            "{trace} & {} \\\\\n",
            num::si(s.trace_average, PLAIN_DIGITS)
        ));
        tex.push_str(&format!(
            "Difference & {} \\\\\n",
            num::si(s.sum - s.trace_average, 2)
        ));
    }
    tex.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n");
    tex
}
