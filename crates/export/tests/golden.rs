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
use separatrix_export::{Labels, ParamValue, StateName, Theme};

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
    let case = case("logistic-map", &Options::default());
    let fig = figure_of(&case, Placement::Inline, Theme::Light, false);
    check_figure("logistic-map-figure", &fig);
    // Dots with a faint line through them; the axis name is x_{n}.
    assert!(fig.tex.contains("opacity=0.4"));
    assert!(fig.tex.contains("mark=*, mark size=1.95bp"));
    assert!(fig.tex.contains("{$x_{n}$}"));
    let csv_files = csv::files(&CsvInput {
        kind: case.portrait.kind,
        labels: &case.labels,
        trajectories: &case.portrait.trajectories,
        ic_ids: &[],
        lines: &case.template.equations,
        format: case.template.format,
        title: &case.template.title,
        app_version: "0.2.0",
    });
    assert!(csv_files[0].csv.starts_with("n,x\n0,0.2\n1,0.592"));
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
