//! Golden exports of gallery systems. `UPDATE_GOLDEN=1 cargo test -p
//! separatrix-export --test golden` rewrites `tests/golden/`.
//!
//! The systems are polynomial (rabbits vs sheep, Lorenz over a short time,
//! the Hénon map, a saddle-node, a linear system), so their numbers do not
//! depend on the platform's `sin`/`exp`, and every figure coordinate is
//! rounded to a tenth of a pixel.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use dyn_expr::{compile, InputFormat};
use separatrix_export::csv::{self, CsvInput};
use separatrix_export::latex_figure::{figure, FigureInput, FigureOutput, Placement};
use separatrix_export::latex_fixed_points::{self, FixedPointsInput};
use separatrix_export::latex_system::{system, Notation, SystemInput};
use separatrix_export::{
    continuation_figure, curves_figure, lyapunov_latex, orbit_figure, plane_points_table,
    special_points_table, BranchMeasure, BranchRun, ContinuationFigureInput, ContinuationRun,
    CurvesFigureInput, Labels, LyapunovRun, LyapunovSpectrum, Method, OrbitDiagram,
    OrbitFigureInput, OrbitRun, ParamValue, PlaneRun, PlaneSlice, SolverOptions, SpecialKind,
    SpecialRow, StateName, Status, SystemKind, SystemMeta, Theme,
};

const UPDATE: &str = "UPDATE_GOLDEN";

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Compares `text` with `tests/golden/<name>` (or writes it).
fn check(name: &str, text: &str) {
    let path = golden_dir().join(name);
    if std::env::var_os(UPDATE).is_some() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {name}; run {UPDATE}=1 cargo test -p separatrix-export --test golden"
        )
    });
    if want != text {
        assert!(
            want.replace("\r\n", "\n") != text.replace("\r\n", "\n"),
            "{name} matches the golden except for line endings: the golden was checked out \
             with CRLF; .gitattributes must keep it LF"
        );
        let line = want
            .lines()
            .zip(text.lines())
            .position(|(a, b)| a != b)
            .unwrap_or(want.lines().count().min(text.lines().count()));
        panic!(
            "{name} differs from the golden at line {}:\n  golden: {:?}\n  now:    {:?}\n\
             (run {UPDATE}=1 cargo test -p separatrix-export --test golden if the change is intended)",
            line + 1,
            want.lines().nth(line),
            text.lines().nth(line)
        );
    }
}

fn check_bytes(name: &str, bytes: &[u8]) {
    let path = golden_dir().join(name);
    if std::env::var_os(UPDATE).is_some() {
        std::fs::write(&path, bytes).unwrap();
        return;
    }
    let want = std::fs::read(&path).unwrap_or_else(|_| panic!("missing golden {name}"));
    assert!(want == bytes, "{name} differs from the golden");
}

/// Rules every LaTeX output follows.
fn assert_portable(name: &str, tex: &str) {
    assert!(!tex.contains('\u{2212}'), "{name} has U+2212");
    assert!(!tex.contains("\n\n"), "{name} has a blank line");
    for word in [
        "\\mleft",
        "\\exponentialE",
        "\\differentialD",
        "\\doubleprime",
        "\\lparen",
        "\\sgn",
        "\\cbrt",
        "\\asin",
    ] {
        assert!(!tex.contains(word), "{name} uses {word}");
    }
    let first = tex.lines().next().unwrap_or("");
    assert!(
        first.starts_with("% Separatrix"),
        "{name} starts with {first:?}"
    );
    assert!(
        tex.lines().any(|l| l.starts_with("% Needs: \\usepackage")),
        "{name} names no packages"
    );
}

fn system_tex(case: &Case, notation: Notation, numbered: bool) -> String {
    let t = &case.template;
    let out = system(&SystemInput {
        lines: &t.equations,
        format: t.format,
        info: &case.system.info,
        params: &case.params,
        notation,
        numbered,
        title: &t.title,
        source_ref: t
            .section
            .as_deref()
            .map(|s| format!("Strogatz §{s}"))
            .as_deref(),
    });
    out.tex
}

fn fixed_points_tex(case: &Case, digits: usize) -> String {
    let t = &case.template;
    latex_fixed_points::table(&FixedPointsInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        fixed_points: &case.portrait.fixed_points,
        search: &case.search,
        lines: &t.equations,
        format: t.format,
        title: &t.title,
        source_ref: None,
        digits,
    })
}

pub fn figure_of(
    case: &Case,
    placement: Placement,
    theme: Theme,
    standalone: bool,
) -> FigureOutput {
    let t = &case.template;
    let pal = palette(theme);
    let mut portrait = case.portrait.clone();
    let w = 8.0 * 96.0 / 2.54;
    let h = 6.0 * 96.0 / 2.54;
    separatrix_export::simplify::simplify_portrait(&mut portrait, w, h, 0.25);
    figure(&FigureInput {
        portrait: &portrait,
        labels: &case.labels,
        width_cm: 8.0,
        height_cm: 6.0,
        theme,
        palette: &pal,
        title: &t.title,
        source_ref: None,
        lines: &t.equations,
        format: t.format,
        placement,
        standalone,
    })
}

fn check_figure(name: &str, out: &FigureOutput) {
    check(&format!("{name}.tex"), &out.tex);
    assert_portable(name, &out.tex);
    assert!(out.points <= 5000 || !out.notes.is_empty(), "{name}");
    for f in &out.files {
        if f.name.ends_with(".png") {
            check_bytes(&f.name, &f.bytes);
        } else {
            let text = String::from_utf8(f.bytes.clone()).unwrap();
            assert!(!text.contains('\u{2212}'));
            check(&f.name, &text);
        }
        assert!(
            out.tex.contains(&format!("{{\\sepxdir {}}}", f.name)),
            "{name} reads {} without \\sepxdir",
            f.name
        );
    }
    // Every inline table has no blank line and ends its rows with \\.
    for chunk in out.tex.split("table[row sep=\\\\]{").skip(1) {
        let body = &chunk[..chunk.find('}').unwrap()];
        assert!(!body.contains("\n\n"));
        assert!(body.lines().skip(1).all(|l| l.ends_with("\\\\")), "{name}");
    }
}

#[test]
fn rabbits_vs_sheep() {
    let case = case("rabbits-vs-sheep", &Options::default());
    let dot = system_tex(&case, Notation::Dot, true);
    check("rabbits-vs-sheep-system.tex", &dot);
    assert_portable("system", &dot);
    assert!(dot.contains("\\begin{align}\n  \\dot{x} &= x\\left(3-x-2y\\right), \\label{eq:rabbits-vs-sheep-x} \\\\\n"));
    assert!(dot.contains(
        "  \\dot{y} &= y\\left(2-x-y\\right). \\label{eq:rabbits-vs-sheep-y}\n\\end{align}\n"
    ));
    let prime = system_tex(&case, Notation::Prime, false);
    check("rabbits-vs-sheep-system-prime.tex", &prime);
    assert!(prime.contains("\\begin{align*}") && !prime.contains("\\label"));
    assert!(prime.contains("x' &= "));
    let leibniz = system_tex(&case, Notation::Leibniz, true);
    assert!(leibniz.contains("\\frac{dx}{dt} &= "));

    let fp = fixed_points_tex(&case, 4);
    check("rabbits-vs-sheep-fixed-points.tex", &fp);
    assert_portable("fixed points", &fp);
    assert!(fp.contains("Saddle") && fp.contains("Stable node") && fp.contains("Unstable node"));
    assert!(fp.contains("$(\\num{1}, \\num{1})$"));

    let inline = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("rabbits-vs-sheep-figure", &inline);
    assert!(inline.files.is_empty());
    assert!(inline.tex.contains("\\usetikzlibrary{arrows.meta,calc}"));
    assert!(inline
        .tex
        .contains("quiver={u=\\thisrow{u}, v=\\thisrow{v}}"));
    assert!(inline.tex.contains("dash pattern=on 4.5bp off 3.75bp"));
    assert!(inline.tex.contains("\\fill[sepxtraj1]"), "arrow heads");
    assert!(inline.tex.contains("opacity=0.5"), "backward runs");
    let external = figure_of(
        &case,
        Placement::External {
            base: "Rabbits vs sheep.tex".into(),
        },
        Theme::Light,
        false,
    );
    check_figure("rabbits-vs-sheep-figure-external", &external);
    assert!(external.tex.starts_with("% Separatrix figure"));
    assert!(external.tex.contains("\\providecommand{\\sepxdir}{}"));
    let names: Vec<&str> = external.files.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"rabbits-vs-sheep-field.dat"), "{names:?}");
    assert!(names.contains(&"rabbits-vs-sheep-nullcline-1.dat"));
    assert!(names.contains(&"rabbits-vs-sheep-ic-1-forward.dat"));
    assert!(names.contains(&"rabbits-vs-sheep-ic-1-backward.dat"));
    assert!(names.contains(&"rabbits-vs-sheep-fixed-points.dat"));
    let dark = figure_of(&case, Placement::Inline, Theme::Dark, true);
    check_figure("rabbits-vs-sheep-figure-dark-standalone", &dark);
    assert!(dark.tex.contains("axis background/.style={fill=sepxbg}"));
    assert!(dark.tex.contains("\\documentclass[border=2mm]{standalone}"));

    let csv_files = csv::files(&CsvInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        trajectories: &case.portrait.trajectories,
        ic_ids: &["ic-1".to_string()],
        lines: &case.template.equations,
        format: case.template.format,
        title: &case.template.title,
        app_version: "0.2.0",
        transient: case.portrait.transient,
        section: None,
    });
    assert_eq!(csv_files.len(), 1);
    assert_eq!(csv_files[0].stem, "rabbits-vs-sheep-trajectory-1");
    check("rabbits-vs-sheep-trajectory-1.csv", &csv_files[0].csv);
    check(
        "rabbits-vs-sheep-trajectory-1.meta.json",
        &csv_files[0].meta,
    );
}

#[test]
fn lorenz() {
    let case = case(
        "lorenz",
        &Options {
            t_end: Some(10.0),
            ..Options::default()
        },
    );
    let tex = system_tex(&case, Notation::Dot, true);
    check("lorenz-system.tex", &tex);
    assert!(tex.contains("\\dot{z} &= xy-bz, \\label{eq:lorenz-equations-z}\n\\end{align}\nwhere $\\sigma = 10$, $r = 28$ and $b = 2.66667$.\n"));
    let fp = fixed_points_tex(&case, 4);
    check("lorenz-fixed-points.tex", &fp);
    // The two outer points have a complex pair, written once.
    assert!(fp.contains("\\pm \\num{10.19}\\,i$"), "{fp}");
    assert!(!fp.contains("$\\tau$"), "no trace column in 3D");
    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("lorenz-figure", &fig);
    // One head per trajectory in 3D.
    assert_eq!(fig.tex.matches("\\fill[sepxtraj1]").count(), 1);
}

/// The Lorenz orbit's crossings of the plane z = 27, upward, after `skip`
/// crossings. Fixed-step RK4: no step-size `powf`, and the
/// right-hand side is polynomial, so every crossing is plain arithmetic.
fn lorenz_section(case: &Case, skip: usize) -> dyn_solve::Solution {
    let opts = dyn_solve::SolverOptions {
        method: dyn_solve::Method::Rk4,
        h: 0.005,
        max_steps: 200_000,
        ..dyn_solve::SolverOptions::default()
    };
    let plane = dyn_solve::Section::Plane {
        state: 2,
        value: 27.0,
        direction: dyn_solve::Direction::Rising,
    };
    let x0 = &case.template.initial_conditions[0];
    dyn_solve::section(
        &case.system,
        &case.params,
        0.0,
        x0,
        60.0,
        &opts,
        &plane,
        skip,
        100_000,
    )
}

#[test]
fn lorenz_section_in_x_and_y() {
    let mut case = case(
        "lorenz",
        &Options {
            field_grid: 0,
            ..Options::default()
        },
    );
    let sol = lorenz_section(&case, 10);
    let all = lorenz_section(&case, 0);
    assert_eq!(sol.t.len() + 10, all.t.len(), "ten crossings are skipped");
    assert_eq!(
        sol.t[0], all.t[10],
        "the first crossing written is the eleventh"
    );
    assert!(sol.x.chunks(3).all(|x| (x[2] - 27.0).abs() < 1e-9));
    let p = &mut case.portrait;
    p.x_axis = separatrix_export::Axis::State { index: 0 };
    p.y_axis = separatrix_export::Axis::State { index: 1 };
    (p.x_range, p.y_range) = equalize((-20.0, 20.0), (-25.0, 25.0), 8.0 / 6.0);
    p.nullclines.clear();
    p.fixed_points.clear();
    p.transient = 10;
    p.section = Some(separatrix_export::SectionSpec::Plane {
        state: 2,
        value: 27.0,
        direction: separatrix_export::Direction::Rising,
    });
    p.trajectories[0].t = sol.t.clone();
    p.trajectories[0].x = sol.x.clone();
    p.trajectories[0].status = sol.status;
    let crossings = sol.t.len();

    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("lorenz-section-figure", &fig);
    let tex = &fig.tex;
    for want in [
        "% Section: plane z = 27 (increasing).\n",
        "% Skipped crossings: the first 10 of each orbit are not drawn.\n",
        "% Section of ic-1\n\\addplot[sepxtraj1, only marks, mark=*, mark size=1.95bp",
        "{$x$}",
        "{$y$}",
    ] {
        assert!(tex.contains(want), "no {want:?}");
    }
    assert!(
        !tex.contains("\\fill[sepxtraj1]"),
        "crossings have no arrow heads"
    );
    assert!(
        !tex.contains("% Diagonal"),
        "a section in two states has no diagonal"
    );
    let rest = &tex[tex.find("% Section of ic-1").unwrap()..];
    let body = &rest[rest.find("]{\n").unwrap() + 3..];
    let rows = body[..body.find('}').unwrap()].lines().skip(1).count();
    assert_eq!(rows, crossings);

    let files = csv::files(&CsvInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        trajectories: &case.portrait.trajectories,
        ic_ids: &[],
        lines: &case.template.equations,
        format: case.template.format,
        title: "Lorenz section",
        app_version: "0.2.0",
        transient: case.portrait.transient,
        section: case.portrait.section,
    });
    assert_eq!(files.len(), 1);
    check("lorenz-section-trajectory-1.csv", &files[0].csv);
    assert_eq!(files[0].csv.lines().count(), 1 + crossings);
    let meta = &files[0].meta;
    assert!(
        meta.contains("\n  \"section\": \"plane z = 27 (increasing)\",\n"),
        "{meta}"
    );
    assert!(meta.contains("\n  \"skipped_crossings\": 10,\n"), "{meta}");
    assert!(meta.contains("crossings of the section"), "{meta}");
    assert!(!meta.contains("\"transient\""), "{meta}");
}

#[test]
fn henon_map() {
    let case = case("henon-map", &Options::default());
    let tex = system_tex(&case, Notation::Dot, true);
    check("henon-map-system.tex", &tex);
    assert!(tex.contains("x_{n+1} &= y_n+1-ax_n^2,"));
    assert!(tex.contains("where $a = 1.4$ and $b = 0.3$."));
    let fp = fixed_points_tex(&case, 4);
    check("henon-map-fixed-points.tex", &fp);
    assert!(fp.contains("Multipliers $\\mu$ & $\\lvert\\mu\\rvert$"));
    // 10,000 iterates: squares, thinned for Copy, a PNG layer for Save.
    let inline = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("henon-map-figure", &inline);
    assert!(inline.thinned);
    assert!(inline.points <= 5000);
    assert!(inline.tex.contains("mark=square*, mark size=0.5625bp"));
    assert!(inline.tex.contains("% Thinned"));
    let external = figure_of(
        &case,
        Placement::External {
            base: "henon".into(),
        },
        Theme::Light,
        false,
    );
    check_figure("henon-figure-external", &external);
    assert!(!external.thinned);
    assert!(external.tex.contains("\\addplot graphics[xmin=-1.6, xmax=1.6, ymin=-0.5, ymax=0.5] {\\sepxdir henon-points.png};"));
    assert!(external.files.iter().any(|f| f.name == "henon-points.png"));
    assert!(!external
        .files
        .iter()
        .any(|f| f.name.contains("ic-1-forward")));
}

#[test]
fn logistic_map_time_series() {
    // The template opens as a cobweb; its time series is n against x.
    let case = case(
        "logistic-map",
        &Options {
            axes: Some(("n", "x")),
            view: Some(((-2.0, 62.0), (-0.05, 1.05))),
            ..Options::default()
        },
    );
    assert!(!case.portrait.cobweb);
    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("logistic-map-figure", &fig);
    // Dots with a faint line through them; the axis name is x_{n}.
    assert!(fig.tex.contains("opacity=0.4"));
    assert!(fig.tex.contains("mark=*, mark size=1.95bp"));
    assert!(fig.tex.contains("{$x_{n}$}"));
    assert!(!fig.tex.contains("% Diagonal") && !fig.tex.contains("% Cobweb"));
    let csv_files = csv::files(&CsvInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        trajectories: &case.portrait.trajectories,
        ic_ids: &[],
        lines: &case.template.equations,
        format: case.template.format,
        title: &case.template.title,
        app_version: "0.2.0",
        transient: case.portrait.transient,
        section: None,
    });
    assert!(csv_files[0].csv.starts_with("n,x\n0,0.1\n1,0.288"));
    assert!(csv_files[0].meta.contains("\"transient\": 0,"));
}

#[test]
fn logistic_map_cobweb() {
    let case = case("logistic-map", &Options::default());
    assert!(case.portrait.cobweb);
    assert_eq!(case.portrait.graph.len(), 1);
    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("logistic-map-cobweb-figure", &fig);
    let tex = &fig.tex;
    for want in [
        "% Diagonal x_{n+1} = x_n\n\\addplot[sepxaxis, line width=0.75bp, line cap=butt] coordinates",
        "% Graph of f\n\\addplot[sepxfp, line width=1.125bp, line join=round, line cap=round]",
        "% Cobweb of ic-1\n\\addplot[sepxtraj1, line width=0.75bp, line join=miter]",
        "{$x_{n}$}",
        "{$x_{n+1}$}",
    ] {
        assert!(tex.contains(want), "no {want:?}");
    }
    assert!(!tex.contains("mark=*, mark size=1.95bp"), "no map dots");
    assert!(!tex.contains("Transient"));
    let external = figure_of(
        &case,
        Placement::External {
            base: "logistic-map".into(),
        },
        Theme::Light,
        false,
    );
    check_figure("logistic-map-cobweb-figure-external", &external);
    let names: Vec<&str> = external.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "logistic-map-graph.dat",
            "logistic-map-ic-1-cobweb.dat",
            "logistic-map-fixed-points.dat"
        ]
    );
    assert!(external
        .tex
        .contains("table {\\sepxdir logistic-map-graph.dat};"));

    // Strogatz §10.1: λ = f'(x*) and |λ| for a one-dimensional map.
    let fp = fixed_points_tex(&case, 4);
    check("logistic-map-fixed-points.tex", &fp);
    assert_portable("fixed points", &fp);
    assert!(
        fp.contains("Fixed point & $\\lambda = f'(x^*)$ & $\\lvert\\lambda\\rvert$ & Type"),
        "{fp}"
    );
    assert!(!fp.contains("\\mu"), "{fp}");
    assert!(fp.contains("$\\num{-1.2}$"), "{fp}");

    // The same orbit as the time series: the CSV is the orbit, not the staircase.
    let csv_files = csv::files(&CsvInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        trajectories: &case.portrait.trajectories,
        ic_ids: &[],
        lines: &case.template.equations,
        format: case.template.format,
        title: &case.template.title,
        app_version: "0.2.0",
        transient: 0,
        section: None,
    });
    assert!(csv_files[0].csv.starts_with("n,x\n0,0.1\n"));
    assert_eq!(csv_files[0].csv.lines().count(), 1 + 61);
}

#[test]
fn a_cobweb_with_a_transient_starts_at_x_t() {
    let case = case(
        "logistic-map",
        &Options {
            transient: 200,
            t_end: Some(1000.0),
            ..Options::default()
        },
    );
    assert_eq!(case.portrait.trajectories[0].t[0], 200.0);
    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    assert!(fig
        .tex
        .contains("% Transient: the first 200 iterates are not drawn.\n"));
    // Converged onto the 2-cycle: at most 4 segments for 1,000 iterates.
    let rest = &fig.tex[fig.tex.find("% Cobweb of ic-1").unwrap()..];
    let body = &rest[rest.find("]{\n").unwrap() + 3..];
    let body = &body[..body.find('}').unwrap()];
    let rows: Vec<&str> = body.lines().skip(1).collect();
    let breaks = rows.iter().filter(|r| r.starts_with("nan")).count();
    let segments = rows.len() - breaks - (breaks + 1);
    assert!(segments <= 4, "{segments} segments:\n{body}");
    let csv_files = csv::files(&CsvInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        trajectories: &case.portrait.trajectories,
        ic_ids: &[],
        lines: &case.template.equations,
        format: case.template.format,
        title: &case.template.title,
        app_version: "0.2.0",
        transient: 200,
        section: None,
    });
    assert!(csv_files[0].csv.starts_with("n,x\n200,"));
    assert_eq!(csv_files[0].csv.lines().count(), 1 + 1001);
    assert!(csv_files[0].meta.contains("\"transient\": 200,"));
}

#[test]
fn a_chaotic_cobweb_keeps_everything_with_the_lualatex_note() {
    let case = case(
        "logistic-map",
        &Options {
            params: vec![("r", 4.0)],
            t_end: Some(5000.0),
            ..Options::default()
        },
    );
    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    assert!(fig.points > 5000, "{}", fig.points);
    assert!(!fig.thinned, "a staircase is a curve, never thinned");
    assert!(
        fig.notes.iter().any(|n| n.contains("lualatex")),
        "{:?}",
        fig.notes
    );
    assert!(!fig.tex.contains(".png"), "nothing is rasterised");
}

#[test]
fn saddle_node_in_one_dimension() {
    let case = case("saddle-node", &Options::default());
    let tex = system_tex(&case, Notation::Dot, true);
    check("saddle-node-system.tex", &tex);
    assert!(tex.contains(
        "\\dot{x} &= r+x^2, \\label{eq:saddle-node-bifurcation-x}\n\\end{align}\nwhere $r = -1$.\n"
    ));
    let fp = fixed_points_tex(&case, 4);
    check("saddle-node-fixed-points.tex", &fp);
    assert!(fp.contains("Fixed point & $\\lambda$ & Type"));
    assert!(fp.contains("$\\num{-1}$ & $\\num{-2}$ & Stable"));
    assert!(fp.contains("$\\num{1}$ & $\\num{2}$ & Unstable"));
    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("saddle-node-figure", &fig);
    // Time on the x axis: the fixed points sit 12 px in from the left.
    assert!(fig.tex.contains("{$t$}") && fig.tex.contains("{$x$}"));
}

#[test]
fn linear_system_with_a_parameter_table() {
    let case = case("linear-2d", &Options::default());
    let tex = system_tex(&case, Notation::Dot, true);
    check("linear-2d-system.tex", &tex);
    assert!(tex.contains("\\dot{y} &= cx+dy. \\label"));
    assert!(tex.contains(
        "\\begin{center}\n\\begin{tabular}{@{}ll@{}}\n\\toprule\nParameter & Value \\\\"
    ));
    assert!(tex.contains("$a$ & \\num{-0.5} \\\\"));
    assert!(
        tex.contains("% Needs: \\usepackage{amsmath} \\usepackage{booktabs} \\usepackage{siunitx}")
    );
    assert!(!tex.contains("where"));
    let fp = fixed_points_tex(&case, 3);
    check("linear-2d-fixed-points.tex", &fp);
    assert!(
        fp.contains("$\\num{-0.5} \\pm \\num{1.41}\\,i$ & \\num{-1} & \\num{2.25} & Stable spiral"),
        "{fp}"
    );
}

fn compiled_info(lines: &[&str], format: InputFormat) -> (Vec<String>, dyn_expr::SystemInfo) {
    let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    let out = compile(&lines.join("\n"), format);
    (lines, out.system.expect("compiles").info)
}

fn plain_system(
    lines: &[String],
    format: InputFormat,
    info: &dyn_expr::SystemInfo,
    notation: Notation,
    title: &str,
) -> String {
    let params: Vec<f64> = info.params.iter().map(|p| p.default).collect();
    system(&SystemInput {
        lines,
        format,
        info,
        params: &params,
        notation,
        numbered: true,
        title,
        source_ref: None,
    })
    .tex
}

#[test]
fn subscripted_states_in_every_notation() {
    let (lines, info) = compiled_info(&["\\dot{x}_1=x_2", "\\dot{x}_{2}=-x_1"], InputFormat::Latex);
    let dot = plain_system(
        &lines,
        InputFormat::Latex,
        &info,
        Notation::Dot,
        "Oscillator",
    );
    assert!(dot.contains("\\dot{x}_{1} &= x_2,"), "{dot}");
    assert!(dot.contains("\\label{eq:oscillator-x-1}"));
    let prime = plain_system(
        &lines,
        InputFormat::Latex,
        &info,
        Notation::Prime,
        "Oscillator",
    );
    assert!(prime.contains("x_{1}' &= x_2,"));
    let leib = plain_system(
        &lines,
        InputFormat::Latex,
        &info,
        Notation::Leibniz,
        "Oscillator",
    );
    assert!(leib.contains("\\frac{dx_{1}}{dt} &= x_2,"));
    let (lines, info) = compiled_info(
        &["x_{1,n+1}=x_{2,n}", "x_{2,n+1}=-0.5x_{1,n}"],
        InputFormat::Latex,
    );
    let map = plain_system(&lines, InputFormat::Latex, &info, Notation::Dot, "Map");
    assert!(map.contains("x_{1,n+1} &= x_{2,n},"), "{map}");
    // Second order: the derived state stays out of the align.
    let (lines, info) = compiled_info(&["\\ddot{\\theta}=-\\sin\\theta"], InputFormat::Latex);
    let pend = plain_system(
        &lines,
        InputFormat::Latex,
        &info,
        Notation::Prime,
        "Pendulum",
    );
    assert!(pend.contains("\\theta'' &= -\\sin\\theta."), "{pend}");
    assert!(pend.contains("\\label{eq:pendulum-theta}"));
}

#[test]
fn mathlive_commands_and_parameter_lines() {
    let (lines, info) = compiled_info(
        &[
            "\\dot{x}=\\sigma\\mleft(y-x\\mright)+\\exponentialE^{-t}\\sgn x",
            "\\dot{y}=x\\lparen\\rho-y\\rparen",
            "\\sigma=10\\\\\\rho=28",
        ],
        InputFormat::Latex,
    );
    let tex = plain_system(&lines, InputFormat::Latex, &info, Notation::Dot, "MathLive");
    check("mathlive-system.tex", &tex);
    assert_portable("mathlive", &tex);
    assert!(tex
        .contains("\\dot{x} &= \\sigma\\left(y-x\\right)+\\mathrm{e}^{-t}\\operatorname{sgn} x,"));
    assert!(tex.contains("\\dot{y} &= x(\\rho-y),"));
    assert!(!tex.contains("&= 10"), "parameter lines leave the align");
    assert!(tex.contains("where $\\sigma = 10$ and $\\rho = 28$."));
}

#[test]
fn text_mode_systems() {
    let (lines, info) = compiled_info(
        &["x' = x*(3 - x - 2*y)", "y' = y*(2 - x - y)"],
        InputFormat::Text,
    );
    let tex = plain_system(&lines, InputFormat::Text, &info, Notation::Dot, "Rabbits");
    assert!(
        tex.contains("\\dot{x} &= x\\left(3-x-2y\\right),") || tex.contains("\\dot{x} &= x"),
        "{tex}"
    );
    assert!(tex.contains("%   x' = x*(3 - x - 2*y)"));
    // A number LaTeX can't write exactly: the first-order form, with a note.
    let (lines, info) = compiled_info(&["x' = -x/9.109e-31"], InputFormat::Text);
    let tex = plain_system(&lines, InputFormat::Text, &info, Notation::Dot, "Electron");
    check("text-fallback-system.tex", &tex);
    assert!(
        tex.contains("% Line 1 (x' = -x/9.109e-31) can't be written in LaTeX exactly"),
        "{tex}"
    );
    assert!(tex.contains("\\dot{x} &= "));
}

#[test]
fn names_that_need_escaping() {
    let case = case("rabbits-vs-sheep", &Options::default());
    let title = "Rabbits & sheep: 50% #1 {x}_~^\\ — §6.4\nnext";
    let fp = latex_fixed_points::table(&FixedPointsInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        fixed_points: &case.portrait.fixed_points,
        search: &case.search,
        lines: &case.template.equations,
        format: case.template.format,
        title,
        source_ref: Some("Strogatz §6.4"),
        digits: 4,
    });
    let first = fp.lines().next().unwrap();
    assert_eq!(
        first,
        "% Separatrix fixed points: Rabbits & sheep: 50% #1 {x}_~^\\ — §6.4 next (Strogatz §6.4)"
    );
    assert!(fp.contains("\\label{tab:rabbits-sheep-50-1-x-6-4-next-fixed-points}"));
    // CSV headers quote names with commas, quotes or newlines.
    let labels = Labels {
        states: vec![
            StateName {
                name: "x,1".into(),
                latex: "x".into(),
            },
            StateName {
                name: "say \"y\"".into(),
                latex: "y".into(),
            },
        ],
        params: vec![ParamValue {
            name: "a".into(),
            latex: "a".into(),
            value: 1.0,
        }],
    };
    let tr = &case.portrait.trajectories[0];
    let csv_text = csv::trajectory(
        case.portrait.kind,
        &labels
            .states
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<_>>(),
        None,
        Some(tr),
    );
    assert!(csv_text.starts_with("t,\"x,1\",\"say \"\"y\"\"\"\n"));
}

#[test]
fn derivatives_on_the_right_follow_the_notation() {
    // Text mode: the user typed primes; Dot notation writes dots on both sides.
    let (lines, info) = compiled_info(&["x'' = -x - mu*(x^2 - 1)*x'"], InputFormat::Text);
    let prime = plain_system(&lines, InputFormat::Text, &info, Notation::Prime, "VdP");
    assert!(prime.contains("x'' &= "), "{prime}");
    assert!(!prime.contains("\\dot"), "{prime}");
    assert!(prime.contains("x'"), "{prime}");
    let leib = plain_system(&lines, InputFormat::Text, &info, Notation::Leibniz, "VdP");
    assert!(leib.contains("\\frac{d^{2}x}{dt^{2}} &= "), "{leib}");
    assert!(leib.contains("\\frac{dx}{dt}"), "{leib}");
    // Third order: \dddot in dot notation, one notation throughout.
    let (lines, info) = compiled_info(&["\\dddot{x}=-x-\\dot{x}-\\ddot{x}"], InputFormat::Latex);
    let dot = plain_system(&lines, InputFormat::Latex, &info, Notation::Dot, "Third");
    assert!(dot.contains("\\dddot{x} &= -x-\\dot{x}-\\ddot{x}"), "{dot}");
    let leib = plain_system(
        &lines,
        InputFormat::Latex,
        &info,
        Notation::Leibniz,
        "Third",
    );
    assert!(
        leib.contains("\\frac{d^{3}x}{dt^{3}} &= -x-\\frac{dx}{dt}-\\frac{d^{2}x}{dt^{2}}"),
        "{leib}"
    );
    // An implicit equation keeps its shape; its derivatives are rewritten.
    let (lines, info) = compiled_info(
        &["\\ddot{\\theta} + b \\dot{\\theta} + \\sin\\theta = 0"],
        InputFormat::Latex,
    );
    let prime = plain_system(&lines, InputFormat::Latex, &info, Notation::Prime, "Pend");
    assert!(
        prime.contains("\\theta'' + b \\theta' + \\sin\\theta &= 0"),
        "{prime}"
    );
    // A control word before a derivative keeps its space.
    let (lines, info) = compiled_info(&["\\ddot{x}=-x\\cdot\\dot{x}"], InputFormat::Latex);
    let prime = plain_system(&lines, InputFormat::Latex, &info, Notation::Prime, "Cdot");
    assert!(prime.contains("x'' &= -x\\cdot x'"), "{prime}");
}

#[test]
fn states_keep_the_font_they_were_typed_in() {
    let (lines, info) = compiled_info(
        &[
            "\\dot{\\mathrm{S}}=-\\beta\\mathrm{S}\\mathrm{I}",
            "\\dot{\\mathrm{I}}=\\beta\\mathrm{S}\\mathrm{I}-\\gamma\\mathrm{I}",
        ],
        InputFormat::Latex,
    );
    let prime = plain_system(&lines, InputFormat::Latex, &info, Notation::Prime, "SIR");
    assert!(
        prime.contains("\\mathrm{S}' &= -\\beta\\mathrm{S}\\mathrm{I}"),
        "{prime}"
    );
    let dot = plain_system(&lines, InputFormat::Latex, &info, Notation::Dot, "SIR");
    assert!(dot.contains("\\dot{\\mathrm{I}} &= "), "{dot}");
    let leib = plain_system(&lines, InputFormat::Latex, &info, Notation::Leibniz, "SIR");
    assert!(leib.contains("\\frac{d\\mathrm{S}}{dt} &= "), "{leib}");
}

fn made_up(id: &str, format: InputFormat, equations: &[&str], x: &str, y: &str) -> Template {
    Template {
        id: id.into(),
        title: id.into(),
        section: None,
        format,
        equations: equations.iter().map(|e| e.to_string()).collect(),
        params: Default::default(),
        view: TemplateView {
            kind: None,
            x: x.into(),
            y: y.into(),
            x_range: (-1.0, 1.0),
            y_range: (-1.0, 1.0),
        },
        initial_conditions: vec![],
        t_end: 10.0,
        backward: Some(false),
    }
}

#[test]
fn tick_labels_carry_their_own_scale() {
    // pgfplots would add its own "·10^{-2}" over labels that already read
    // 0.02, or that already carry their exponent: it must not.
    let mut t = template("linear-2d");
    t.initial_conditions = vec![vec![0.03, 0.01]];
    let small = case_of(
        t.clone(),
        &Options {
            view: Some(((-0.06, 0.06), (-0.045, 0.045))),
            ..Options::default()
        },
    );
    let fig = figure_of(&small, Placement::Inline, Theme::Light, false);
    check_figure("tiny-view-figure", &fig);
    assert!(fig.tex.contains("scaled ticks=false"));
    assert!(fig.tex.contains("{$0.0"), "plain decimals");
    let huge = case_of(
        t,
        &Options {
            view: Some(((-6e-6, 6e-6), (-6e7, 6e7))),
            ..Options::default()
        },
    );
    let fig = figure_of(&huge, Placement::Inline, Theme::Light, false);
    check_figure("huge-view-figure", &fig);
    assert!(fig.tex.contains("scaled ticks=false"));
    assert!(fig.tex.contains("\\cdot 10^{"), "labels carry the exponent");
}

#[test]
fn fourth_order_axis_names_compile_with_the_listed_packages() {
    let t = made_up(
        "fourth-order",
        InputFormat::Text,
        &["x'''' = -x - x'''"],
        "x''",
        "x'''",
    );
    let c = case_of(t.clone(), &Options::default());
    let fig = figure_of(&c, Placement::Inline, Theme::Light, false);
    check_figure("fourth-order-figure", &fig);
    assert!(fig.tex.contains("{$\\dddot{x}$}"));
    assert!(fig
        .tex
        .contains("% Needs: \\usepackage{amsmath} \\usepackage{pgfplots}"));
    let mut t = t;
    t.view.x = "t".into();
    t.view.x_range = (0.0, 10.0);
    let c = case_of(t, &Options::default());
    let fig = figure_of(&c, Placement::Inline, Theme::Light, true);
    check_figure("fourth-order-time-standalone", &fig);
    assert!(fig
        .tex
        .contains("\\documentclass[border=2mm]{standalone}\n\\usepackage{amsmath}\n"));
}

#[test]
fn axis_names_stay_apart_when_both_ranges_start_at_zero() {
    let c = case(
        "selkov-glycolysis",
        &Options {
            view: Some(((0.0, 3.0), (0.0, 3.0))),
            ..Options::default()
        },
    );
    let fig = figure_of(&c, Placement::Inline, Theme::Light, false);
    check_figure("selkov-zero-corner-figure", &fig);
    // The axes sit on the left and bottom edges, like the screen's.
    assert!(fig.tex.contains("axis x line*=bottom") && fig.tex.contains("axis y line*=left"));
    let at = |name: &str| -> String {
        let line = fig
            .tex
            .lines()
            .find(|l| l.ends_with(&format!("{{${name}$}};")) && l.contains("sepxlabel]"))
            .unwrap_or_else(|| panic!("no {name} name"));
        line.split(" at ").nth(1).unwrap().to_string()
    };
    assert_ne!(at("x"), at("y"));
    assert!(
        fig.tex.contains("\\node[anchor=north west, sepxlabel] at"),
        "the y name right of the y axis"
    );
}

#[test]
fn every_label_has_a_halo() {
    let c = case("rabbits-vs-sheep", &Options::default());
    let fig = figure_of(&c, Placement::Inline, Theme::Light, false);
    assert!(fig.tex.contains("\\definecolor{sepxhalo}{HTML}{FFFFFF}%"));
    assert!(fig
        .tex
        .contains("sepxlabel/.style={inner sep=1.125bp, outer sep=-1.125bp, fill=sepxhalo"));
    for l in fig.tex.lines().filter(|l| l.starts_with("\\node")) {
        assert!(l.contains("sepxlabel"), "{l}");
    }
    // No line before the picture leaves a space in the paragraph.
    let before = &fig.tex[..fig.tex.find("\\begin{tikzpicture}").unwrap()];
    for l in before.lines() {
        assert!(l.starts_with('%') || l.ends_with('%'), "{l:?}");
    }
}

fn template_labels(id: &str, set: &[(&str, f64)]) -> (Template, Labels) {
    let t = template(id);
    let out = compile(&t.equations.join("\n"), t.format);
    let info = out.system.expect("compiles").info;
    let values: Vec<f64> = info
        .params
        .iter()
        .map(|p| {
            set.iter()
                .find(|(n, _)| *n == p.name)
                .map(|&(_, v)| v)
                .or_else(|| t.params.get(&p.name).copied())
                .unwrap_or(p.default)
        })
        .collect();
    let labels = Labels::from_info(&info, &values);
    (t, labels)
}

fn lorenz_lyapunov_run() -> LyapunovRun {
    let exponents = vec![0.90561, -0.00025, -14.57196];
    LyapunovRun {
        spectrum: LyapunovSpectrum {
            sum: exponents.iter().sum(),
            exponents,
            uncertainty: vec![0.00437, 0.00026, 0.0010],
            trace_average: -41.0 / 3.0,
            history_t: vec![300.0, 500.0, 700.0, 900.0, 1100.0],
            history: vec![
                0.9312, 0.0123, -14.6011, //
                0.9105, -0.0031, -14.5802, //
                0.9071, 0.0008, -14.5755, //
                0.9049, -0.0004, -14.5738, //
                0.90561, -0.00025, -14.57196,
            ],
            renormalizations: 10000,
            status: Status::Completed,
            stats: Default::default(),
        },
        x0: vec![1.0, 1.0, 1.0],
        t0: 0.0,
        transient: 100.0,
        total: 1000.0,
        interval: 0.1,
        blocks: 10,
        solver: SolverOptions {
            method: Method::Rk4,
            h: 0.01,
            ..SolverOptions::default()
        },
    }
}

#[test]
fn lorenz_lyapunov_table_and_csv() {
    let (t, labels) = template_labels("lorenz", &[]);
    let sys = SystemMeta {
        kind: SystemKind::Ode,
        labels: &labels,
        title: &t.title,
        source_ref: Some("Strogatz §9.3"),
        lines: &t.equations,
        format: t.format,
    };
    let run = lorenz_lyapunov_run();
    let tex = lyapunov_latex(&sys, &run);
    check("lorenz-lyapunov-table.tex", &tex);
    assert_portable("lorenz-lyapunov-table", &tex);
    assert!(
        tex.contains("$\\lambda_{1}$ & \\num{0.906 +- 0.004} \\\\"),
        "{tex}"
    );
    assert!(tex.contains("\\sisetup{uncertainty-mode = separate}"));
    let file = csv::lyapunov(&sys, &run, "0.2.0");
    assert_eq!(file.stem, "lorenz-equations-lyapunov");
    check("lorenz-lyapunov.csv", &file.csv);
    check("lorenz-lyapunov.meta.json", &file.meta);
}

fn logistic_orbit_run() -> OrbitRun {
    let (lo, hi, columns, transient, kept) = (2.8, 3.7, 10, 100, 4);
    let step = (hi - lo) / (columns - 1) as f64;
    let r: Vec<f64> = (0..columns).map(|j| lo + j as f64 * step).collect();
    let mut x = Vec::new();
    for &rj in &r {
        let mut v: f64 = 0.2;
        for n in 0..transient + kept {
            if n >= transient {
                x.push(v);
            }
            v = rj * v * (1.0 - v);
        }
    }
    OrbitRun {
        param: 0,
        state: 0,
        x0: vec![0.2],
        transient,
        diagram: OrbitDiagram { r, x, kept },
    }
}

#[test]
fn logistic_orbit_csv_and_figure() {
    let (t, labels) = template_labels("logistic-map", &[("r", 3.2)]);
    let sys = SystemMeta {
        kind: SystemKind::Map,
        labels: &labels,
        title: &t.title,
        source_ref: Some("Strogatz §10.2"),
        lines: &t.equations,
        format: t.format,
    };
    let run = logistic_orbit_run();
    let file = csv::orbit(&sys, &run, "0.2.0");
    assert_eq!(file.stem, "logistic-map-orbit-diagram");
    assert_eq!(file.csv.lines().next(), Some("r,n,x"));
    assert_eq!(file.csv.lines().count(), 1 + 40);
    check("logistic-orbit.csv", &file.csv);
    let pal = palette(Theme::Light);
    let fig = orbit_figure(&OrbitFigureInput {
        sys: &sys,
        run: &run,
        x_range: (2.75, 3.75),
        y_range: (0.0, 1.0),
        width_cm: 8.0,
        height_cm: 6.0,
        theme: Theme::Light,
        palette: &pal,
        placement: Placement::Inline,
        standalone: false,
    });
    assert_eq!((fig.points, fig.thinned), (40, false));
    assert!(fig.files.is_empty());
    check_figure("logistic-orbit-figure", &fig);
}

fn selkov_continuation_run() -> ContinuationRun {
    let a = 0.08;
    let hopf = [
        (
            0.346_410_161_513_799_83,
            [0.346_410_161_513_799_83, 1.732_050_807_568_853],
            0.447_213_595_499_976_8,
            -0.527_073_167_303_874_5,
        ),
        (
            0.848_528_137_423_867_6,
            [0.848_528_137_423_867_6, 1.060_660_171_779_810_8],
            0.894_427_190_999_925_8,
            -0.141_904_313_900_418_8,
        ),
    ];
    let mut br = BranchRun {
        period: 1,
        color: 0,
        p: vec![],
        x: vec![],
        stable: vec![],
        unstable: vec![],
        special: vec![],
    };
    let mut next = 0;
    for i in 0..=30 {
        let b = i as f64 * 0.05;
        while next < hopf.len() && hopf[next].0 < b {
            let (p, x, omega, l1) = hopf[next];
            br.special.push(SpecialRow {
                kind: SpecialKind::Hopf,
                index: br.p.len(),
                p,
                x: x.to_vec(),
                omega: Some(omega),
                lyapunov: Some(l1),
            });
            br.p.push(p);
            br.x.extend(x);
            br.stable.push(false);
            br.unstable.push(0);
            next += 1;
        }
        let stable = b * b < 0.12 || b * b > 0.72;
        br.p.push(b);
        br.x.extend([b, b / (a + b * b)]);
        br.stable.push(stable);
        br.unstable.push(if stable { 0 } else { 2 });
    }
    ContinuationRun {
        param: 1,
        range: (0.0, 1.5),
        y_measure: BranchMeasure::State { index: 1 },
        branches: vec![br],
    }
}

fn logistic_cascade_run() -> ContinuationRun {
    let branch = |period: usize, color: u8, special: &[(SpecialKind, f64, f64)]| BranchRun {
        period,
        color,
        p: special.iter().map(|s| s.1).collect(),
        x: special
            .iter()
            .flat_map(|s| std::iter::repeat_n(s.2, period))
            .collect(),
        stable: vec![false; special.len()],
        unstable: vec![0; special.len()],
        special: special
            .iter()
            .enumerate()
            .map(|(index, &(kind, p, x))| SpecialRow {
                kind,
                index,
                p,
                x: vec![x],
                omega: None,
                lyapunov: None,
            })
            .collect(),
    };
    let (bp, pd) = (SpecialKind::BranchPoint, SpecialKind::Flip);
    ContinuationRun {
        param: 0,
        range: (0.0, 3.57),
        y_measure: BranchMeasure::State { index: 0 },
        branches: vec![
            branch(1, 0, &[(bp, 0.999_999_999_999_963_7, 0.0)]),
            branch(
                1,
                1,
                &[
                    (bp, 0.999_999_999_999_963_7, 3.6e-14),
                    (pd, 2.999_999_999_999_940_5, 0.666_666_666_666_660_1),
                ],
            ),
            branch(
                2,
                2,
                &[
                    (pd, 3.449_489_742_783_287_6, 0.849_937_779_554_800_1),
                    (pd, 3.449_489_742_783_288, 0.439_960_113_1),
                ],
            ),
            branch(
                4,
                3,
                &[
                    (pd, 3.544_090_359_551_739_7, 0.884_049_545_401_325_9),
                    (pd, 3.544_090_359_551_74, 0.367_8),
                ],
            ),
            branch(
                8,
                4,
                &[
                    (pd, 3.564_407_266_095_344_5, 0.890_787_378_461_704),
                    (pd, 3.564_407_266_095_345, 0.35),
                ],
            ),
        ],
    }
}

#[test]
fn selkov_branch_plot_special_points_and_csv() {
    let (t, labels) = template_labels("selkov-glycolysis", &[("b", 0.6)]);
    let sys = SystemMeta {
        kind: SystemKind::Ode,
        labels: &labels,
        title: &t.title,
        source_ref: Some("Strogatz §7.3"),
        lines: &t.equations,
        format: t.format,
    };
    let run = selkov_continuation_run();
    let pal = palette(Theme::Light);
    let input = ContinuationFigureInput {
        sys: &sys,
        run: &run,
        x_range: (-0.06, 1.56),
        y_range: (-0.1, 2.0),
        width_cm: 8.0,
        height_cm: 6.0,
        theme: Theme::Light,
        palette: &pal,
        placement: Placement::Inline,
        standalone: false,
    };
    let fig = continuation_figure(&input);
    check_figure("selkov-continuation-figure", &fig);
    assert!(fig.files.is_empty());
    assert_eq!(fig.tex.matches("% Branch 1, stable (solid)").count(), 1);
    assert_eq!(fig.tex.matches("% Branch 1, unstable (dashed)").count(), 1);
    assert_eq!(fig.tex.matches("dash pattern=on 4.5bp off 3bp").count(), 1);
    assert_eq!(fig.tex.matches("{H};").count(), 2);
    assert!(fig.tex.contains("\\usetikzlibrary{calc,plotmarks}"));
    assert!(fig.tex.contains("% Parameters: a = 0.08\n"), "{}", fig.tex);

    let saved = continuation_figure(&ContinuationFigureInput {
        placement: Placement::External {
            base: "Selkov branches.tex".into(),
        },
        ..input
    });
    let names: Vec<&str> = saved.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "selkov-branches-branch-1-stable.dat",
            "selkov-branches-branch-1-unstable.dat",
            "selkov-branches-special-points.dat"
        ]
    );
    for f in &saved.files {
        assert!(saved.tex.contains(&format!("{{\\sepxdir {}}}", f.name)));
    }

    let table = special_points_table(&sys, &run);
    check("selkov-special-points.tex", &table);
    assert_portable("selkov-special-points", &table);
    assert!(table.contains(
        "H & \\num{0.346410} & $(\\num{0.34641}, \\num{1.73205})$ & \\num{0.447214} & \\num{-0.527073} \\\\"
    ));
    assert!(table.contains("\\num{0.848528}"));

    let csv = csv::continuation(&sys, &run, "0.2.0");
    assert_eq!(csv.file.stem, "sel-kov-glycolysis-continuation");
    assert_eq!(
        csv.file.csv.lines().next(),
        Some("branch,period,point,b,x,y,stable,unstable")
    );
    assert_eq!(csv.file.csv.lines().count(), 1 + 33);
    assert_eq!(csv.special.lines().count(), 1 + 2);
    check("selkov-continuation.csv", &csv.file.csv);
    check("selkov-continuation.meta.json", &csv.file.meta);
    check("selkov-continuation-special.csv", &csv.special);
}

#[test]
fn logistic_cascade_lists_each_flip_once() {
    let (t, labels) = template_labels("logistic-map", &[("r", 3.2)]);
    let sys = SystemMeta {
        kind: SystemKind::Map,
        labels: &labels,
        title: "Logistic cascade",
        source_ref: Some("Strogatz §10.2"),
        lines: &t.equations,
        format: t.format,
    };
    let run = logistic_cascade_run();
    let table = special_points_table(&sys, &run);
    check("logistic-cascade-special-points.tex", &table);
    assert_portable("logistic-cascade-special-points", &table);
    let rows: Vec<&str> = table
        .lines()
        .filter(|l| l.starts_with("BP &") || l.starts_with("PD &"))
        .collect();
    assert_eq!(
        rows,
        [
            "BP & \\num{1.000000} & \\num{0} & \\num{1} \\\\",
            "PD & \\num{3.000000} & \\num{0.666667} & \\num{1} \\\\",
            "PD & \\num{3.449490} & \\num{0.849938} & \\num{2} \\\\",
            "PD & \\num{3.544090} & \\num{0.88405} & \\num{4} \\\\",
            "PD & \\num{3.564407} & \\num{0.890787} & \\num{8} \\\\",
        ]
    );
    assert!(!table.contains("\\omega") && !table.contains("\\ell_1"));
}

fn co_oxidation_plane() -> PlaneRun {
    PlaneRun {
        free: (0, 1),
        ranges: ((0.5, 2.5), (0.0, 2.0)),
        curves: plane::co_oxidation_curves(),
        slice: Some(PlaneSlice {
            range: (0.8, 1.3),
            at: 0.4,
        }),
    }
}

#[test]
fn co_oxidation_plane_figure_table_and_csv() {
    let (t, labels) = template_labels("co-oxidation", &[]);
    let sys = SystemMeta {
        kind: SystemKind::Ode,
        labels: &labels,
        title: &t.title,
        source_ref: Some("MatCont Tutorial IV"),
        lines: &t.equations,
        format: t.format,
    };
    let run = co_oxidation_plane();
    let pal = palette(Theme::Light);
    let input = CurvesFigureInput {
        sys: &sys,
        run: &run,
        x_range: (0.66, 1.7),
        y_range: (0.1, 2.06),
        width_cm: 8.0,
        height_cm: 6.0,
        theme: Theme::Light,
        palette: &pal,
        placement: Placement::Inline,
        standalone: false,
    };
    let fig = curves_figure(&input);
    check_figure("co-oxidation-plane-figure", &fig);
    assert!(fig.files.is_empty());
    for line in [
        "% Curve 1: fold (LP), solid\n",
        "% Curve 2: Hopf (H), neutral saddle (dotted)\n",
        "% Curve 2: Hopf (H), dashed\n",
        "% Curve 3: homoclinic (Hom), dash-dotted\n",
        "% The Branch view's range, at K = 0.4\n\\addplot[sepxtick, line width=0.75bp, line cap=butt] coordinates {(0.8,0.4) (1.3,0.4)};\n",
        "% Parameter plane: K against k_2, followed in k_2 from 0.5 to 2.5 and K from 0 to 2; 3 curves, 5 codimension-2 points.\n",
        "\\definecolor{sepxfold}{HTML}{B35900}%\n",
        "\\definecolor{sepxhopf}{HTML}{0072B2}%\n",
        "\\definecolor{sepxhom}{HTML}{6A3FC8}%\n",
    ] {
        assert!(fig.tex.contains(line), "{line}");
    }
    assert!(
        !fig.tex.contains("% Parameters:"),
        "both parameters are free"
    );
    assert_eq!(
        fig.tex.matches("dash pattern=on 6bp off 2.25bp]").count(),
        1
    );
    assert_eq!(
        fig.tex
            .matches("dash pattern=on 6bp off 2.25bp on 1.125bp off 2.25bp]")
            .count(),
        1
    );
    assert_eq!(fig.tex.matches("dash pattern=on 0bp off 2.6bp]").count(), 1);
    assert_eq!(fig.tex.matches("{BT};").count(), 2, "one glyph per point");
    assert_eq!(fig.tex.matches("{CP};").count(), 1);
    assert_eq!(fig.tex.matches("{GH};").count(), 2);

    let saved = curves_figure(&CurvesFigureInput {
        placement: Placement::External {
            base: "CO plane.tex".into(),
        },
        ..input
    });
    let names: Vec<&str> = saved.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "co-plane-curve-1.dat",
            "co-plane-curve-2-neutral.dat",
            "co-plane-curve-2.dat",
            "co-plane-curve-3.dat",
            "co-plane-special-points.dat"
        ]
    );
    for f in &saved.files {
        assert!(saved.tex.contains(&format!("{{\\sepxdir {}}}", f.name)));
    }

    let table = plane_points_table(&sys, &run);
    check("co-oxidation-plane-points.tex", &table);
    assert_portable("co-oxidation-plane-points", &table);
    let rows: Vec<&str> = table
        .lines()
        .filter(|l| ["BT &", "CP &", "GH &"].iter().any(|k| l.starts_with(k)))
        .collect();
    assert_eq!(rows.len(), 5, "{table}");
    assert_eq!(
        rows[0],
        "BT & \\num{1.161199} & \\num{0.722339} & $(\\num{0.016337}, \\num{0.63841}, \\num{0.200456})$ & LP, H & \\num{0.0482256} & \\num{1.93763} & -- \\\\"
    );

    let csv = csv::curves(&sys, &run, "0.2.0");
    assert_eq!(csv.file.stem, "co-oxidation-curves");
    assert_eq!(
        csv.file.csv.lines().next(),
        Some("curve,kind,period,point,k_2,K,x,y,s,neutral")
    );
    assert_eq!(
        csv.special.lines().next(),
        Some("curve,point,type,period,k_2,K,x,y,s,a,b,c,theta")
    );
    let points: usize = run.curves.iter().map(|c| c.len()).sum();
    assert_eq!(csv.file.csv.lines().count(), 1 + points);
    assert_eq!(
        csv.special.lines().count(),
        1 + 7,
        "as each curve reports them"
    );
    check("co-oxidation-curves.csv", &csv.file.csv);
    check("co-oxidation-curves.meta.json", &csv.file.meta);
    check("co-oxidation-curves-special.csv", &csv.special);
}

#[test]
fn predator_prey_plane_ticks_and_merged_fold_flip() {
    let (t, labels) = template_labels("predator-prey-map", &[]);
    let sys = SystemMeta {
        kind: SystemKind::Map,
        labels: &labels,
        title: &t.title,
        source_ref: Some("MatContM Tutorial M-II"),
        lines: &t.equations,
        format: t.format,
    };
    let run = PlaneRun {
        free: (0, 1),
        ranges: ((0.0, 16.0), (-6.0, 3.0)),
        curves: plane::predator_prey_curves(),
        slice: None,
    };
    let pal = palette(Theme::Light);
    let fig = curves_figure(&CurvesFigureInput {
        sys: &sys,
        run: &run,
        x_range: (-0.5, 16.5),
        y_range: (-6.3, 3.3),
        width_cm: 8.0,
        height_cm: 6.0,
        theme: Theme::Light,
        palette: &pal,
        placement: Placement::Inline,
        standalone: false,
    });
    check_figure("predator-prey-plane-figure", &fig);
    for q in 5..=9 {
        assert_eq!(
            fig.tex.matches(&format!("{{$1{{:}}{q}$}};")).count(),
            1,
            "1:{q}"
        );
    }
    assert_eq!(
        fig.tex
            .matches("\\addplot[sepxns, line width=0.75bp, line cap=butt] coordinates")
            .count(),
        5
    );
    assert_eq!(fig.tex.matches("{LPPD};").count(), 1, "one glyph per point");
    assert_eq!(fig.tex.matches("{R2};").count(), 1);
    assert!(fig.tex.contains(
        "dash pattern=on 6bp off 1.875bp on 1.125bp off 1.875bp on 1.125bp off 1.875bp]"
    ));
    assert!(fig.tex.contains("dash pattern=on 2.25bp off 2.25bp]"));
    assert!(!fig.tex.contains("The Branch view's range"));

    let table = plane_points_table(&sys, &run);
    check("predator-prey-plane-points.tex", &table);
    assert_portable("predator-prey-plane-points", &table);
    assert!(
        table.lines().any(
            |l| l.starts_with("LPPD & \\num{3.000000} & \\num{2.000000} & ")
                && l.contains(" & \\num{1} & BP, PD & ")
        ),
        "{table}"
    );
    assert!(table.contains(" & \\num{1.570796} \\\\"), "R4's θ = π/2");
    assert!(table.contains("Period & Curves & $\\theta$"));
    assert!(
        !table.contains("$b$") && !table.contains("$c$"),
        "no BT or cusp column"
    );

    let csv = csv::curves(&sys, &run, "0.2.0");
    assert!(csv.file.meta.contains("\"format\": \"separatrix-curves\""));
    assert!(csv.file.meta.contains("\"q\": 5"));
    assert_eq!(
        csv.file
            .csv
            .lines()
            .nth(1)
            .map(|l| l.split(',').take(2).collect::<Vec<_>>()),
        Some(vec!["1", "NS"])
    );
}
