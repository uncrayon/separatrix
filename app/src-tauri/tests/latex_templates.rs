//! Every gallery template's LaTeX exports (the system, its fixed points and
//! the figure, with inline data and with external `.dat` files) compile
//! with pdfLaTeX, through the same functions the export commands call.
//!
//! Skipped when `pdflatex` is not installed, unless `REQUIRE_PDFLATEX=1`
//! (CI's TeX job), which turns a missing `pdflatex` into a failure. The
//! packages are listed in docs/exports.md.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use separatrix_export::latex_figure::file_base;
use separatrix_export::Placement;
use separatrix_lib::export_service::{self, ExportCache};
use separatrix_lib::ipc::{
    AxisRef, CompileRequest, CompileResponse, ExportTheme, FigureLatexRequest,
    FixedPointsLatexRequest, InputFormat, Method, Notation, PortraitRequest, Severity,
    SolverOptions, SystemKind, SystemLatexRequest, TrajectoryRequest, TrajectoryTag,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    id: String,
    title: String,
    #[serde(default)]
    chapter: Option<String>,
    format: InputFormat,
    equations: Vec<String>,
    params: HashMap<String, f64>,
    view: View,
    initial_conditions: Vec<Vec<f64>>,
    t_end: f64,
    #[serde(default)]
    backward: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct View {
    x: String,
    y: String,
    x_range: (f64, f64),
    y_range: (f64, f64),
}

fn have_pdflatex() -> bool {
    let found = Command::new("pdflatex")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !found && std::env::var_os("REQUIRE_PDFLATEX").is_some_and(|v| v == "1") {
        panic!("REQUIRE_PDFLATEX=1 but pdflatex is not installed");
    }
    if !found {
        eprintln!("pdflatex not found: skipping the template LaTeX compile test");
    }
    found
}

fn axis(name: &str, res: &CompileResponse) -> AxisRef {
    if name == "t" || name == "n" {
        return AxisRef::Time;
    }
    let index = res
        .states
        .iter()
        .position(|s| s.name == name || s.latex == name)
        .unwrap_or_else(|| panic!("axis {name:?} is not a state"));
    AxisRef::State { index }
}

/// The template's portrait request and one tag per trajectory, as the UI
/// builds them for its first view.
fn request(t: &Template, res: &CompileResponse) -> (PortraitRequest, Vec<TrajectoryTag>) {
    let dim = res.states.len();
    let is_map = res.kind == Some(SystemKind::Map);
    let x_axis = axis(&t.view.x, res);
    let y_axis = axis(&t.view.y, res);
    let time_window = match (x_axis, y_axis) {
        (AxisRef::Time, _) => Some(t.view.x_range),
        (_, AxisRef::Time) => Some(t.view.y_range),
        _ => None,
    };
    let mut trajectories = Vec::new();
    let mut tags = Vec::new();
    for (k, ic) in t.initial_conditions.iter().enumerate() {
        let x0: Vec<f64> = (0..dim)
            .map(|i| ic.get(i).copied().unwrap_or(0.0))
            .collect();
        let mut push = |t1: f64, backward: bool| {
            trajectories.push(TrajectoryRequest {
                x0: x0.clone(),
                t0: 0.0,
                t1,
            });
            tags.push(TrajectoryTag {
                ic_id: format!("ic-{}", k + 1),
                backward,
                color: k as u8,
            });
        };
        if is_map {
            push(t.t_end.round().max(1.0), false);
        } else if let Some((lo, hi)) = time_window {
            if hi > 0.0 {
                push(hi, false);
            }
            if lo < 0.0 {
                push(lo, true);
            }
        } else {
            push(t.t_end, false);
            if t.backward != Some(false) {
                push(-t.t_end, true);
            }
        }
    }
    let req = PortraitRequest {
        request_id: 1,
        system_id: res.system_id.unwrap(),
        params: res
            .params
            .iter()
            .map(|p| t.params.get(&p.name).copied().unwrap_or(p.default))
            .collect(),
        x_axis,
        y_axis,
        x_range: t.view.x_range,
        y_range: t.view.y_range,
        base_state: vec![0.0; dim],
        field_time: 0.0,
        field_grid: 21,
        nullclines: true,
        fixed_points: res.autonomous,
        trajectories,
        solver: SolverOptions {
            method: Method::Dopri5,
            rtol: 1e-6,
            atol: 1e-9,
            h: 0.0,
            h_max: None,
            max_steps: 200_000,
        },
        samples: 2000,
    };
    (req, tags)
}

fn pdflatex(dir: &Path, main: &str) {
    let out = Command::new("pdflatex")
        .args(["-interaction=nonstopmode", "-halt-on-error", main])
        .current_dir(dir)
        .output()
        .expect("run pdflatex");
    if !out.status.success() {
        let log = String::from_utf8_lossy(&out.stdout);
        let tail: Vec<&str> = log.lines().rev().take(40).collect();
        panic!(
            "{main} failed to compile in {}:\n{}",
            dir.display(),
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        );
    }
}

/// The preamble lines of the snippets' `% Needs:` lines, each once.
fn preamble(snippets: &[&str]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for s in snippets {
        let needs = s
            .lines()
            .find_map(|l| l.strip_prefix("% Needs: "))
            .expect("every snippet lists its packages");
        for l in needs.replace(" \\", "\n\\").lines() {
            if !lines.iter().any(|x| x == l) {
                lines.push(l.to_string());
            }
        }
    }
    lines.join("\n")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sepx-templates-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn every_template_exports_latex_that_compiles() {
    let templates: Vec<Template> = serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
    assert!(!templates.is_empty());
    let run_tex = have_pdflatex();
    let store = SystemStore::new();
    let cache = ExportCache::new();
    for t in &templates {
        let res = service::compile_system(
            &store,
            &CompileRequest {
                source: t.equations.join("\n"),
                format: t.format,
            },
        );
        assert!(
            res.ok
                && !res
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == Severity::Error),
            "template {} doesn't compile: {:?}",
            t.id,
            res.diagnostics
        );
        let system = store.get(res.system_id.unwrap()).unwrap();
        let (portrait, tags) = request(t, &res);
        let source_ref = t.chapter.clone();

        let sys = export_service::system_latex(
            &system,
            &SystemLatexRequest {
                system_id: portrait.system_id,
                lines: t.equations.clone(),
                format: t.format,
                params: portrait.params.clone(),
                notation: Notation::Dot,
                numbered: true,
                title: t.title.clone(),
                source_ref: source_ref.clone(),
            },
        );
        let prime = export_service::system_latex(
            &system,
            &SystemLatexRequest {
                system_id: portrait.system_id,
                lines: t.equations.clone(),
                format: t.format,
                params: portrait.params.clone(),
                notation: Notation::Leibniz,
                numbered: false,
                title: t.title.clone(),
                source_ref: source_ref.clone(),
            },
        );
        let fixed = res.autonomous.then(|| {
            export_service::fixed_points_latex(
                &system,
                &FixedPointsLatexRequest {
                    portrait: portrait.clone(),
                    title: t.title.clone(),
                    source_ref: source_ref.clone(),
                    lines: t.equations.clone(),
                    format: t.format,
                    digits: 4,
                },
            )
            .unwrap_or_else(|e| panic!("{}: fixed points: {e}", t.id))
        });
        let fig = FigureLatexRequest {
            portrait: portrait.clone(),
            tags: tags.clone(),
            width_cm: 8.0,
            height_cm: 6.0,
            theme: ExportTheme::Light,
            title: t.title.clone(),
            source_ref: source_ref.clone(),
            lines: t.equations.clone(),
            format: t.format,
            standalone: false,
        };
        let inline = export_service::figure_latex(&cache, &system, &fig, Placement::Inline)
            .unwrap_or_else(|e| panic!("{}: figure: {e}", t.id));
        let base = file_base(&format!("{}.tex", t.id));
        let external = export_service::figure_latex(
            &cache,
            &system,
            &FigureLatexRequest {
                theme: ExportTheme::Dark,
                ..fig.clone()
            },
            Placement::External { base: base.clone() },
        )
        .unwrap_or_else(|e| panic!("{}: figure: {e}", t.id));
        for tex in [&sys.tex, &prime.tex, &inline.tex, &external.tex]
            .into_iter()
            .chain(fixed.as_ref())
        {
            assert!(!tex.contains('\u{2212}'), "{}: U+2212 in\n{tex}", t.id);
        }
        assert!(inline.files.is_empty());
        assert!(!external.files.is_empty(), "{}", t.id);

        if !run_tex {
            continue;
        }
        let dir = scratch(&t.id);
        for f in &external.files {
            std::fs::write(dir.join(&f.name), &f.bytes).unwrap();
        }
        std::fs::write(dir.join(format!("{base}.tex")), &external.tex).unwrap();
        let mut snippets: Vec<&str> = vec![&sys.tex, &prime.tex, &inline.tex, &external.tex];
        if let Some(f) = &fixed {
            snippets.push(f);
        }
        let body = format!(
            "{}\n\n{}\n\n{}\n\n{}\n\n\\input{{{base}.tex}}\n",
            sys.tex,
            prime.tex,
            fixed.as_deref().unwrap_or(""),
            inline.tex
        );
        let doc = format!(
            "\\documentclass{{article}}\n{}\n\\begin{{document}}\n{body}\\end{{document}}\n",
            preamble(&snippets)
        );
        std::fs::write(dir.join("main.tex"), doc).unwrap();
        pdflatex(&dir, "main.tex");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
