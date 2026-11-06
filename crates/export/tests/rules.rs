//! Rules of the exports, on hand-built portraits: CSV format, the
//! 5,000-point rule, clipping, and what a figure never contains.

mod common;

use common::palette;
use dyn_expr::InputFormat;
use separatrix_export::csv::{self, CsvInput};
use separatrix_export::latex_figure::{figure, FigureInput, Placement, MAX_POINTS};
use separatrix_export::{
    Arrow, Axis, Classification, Direction, FixedPoint, Labels, ParamValue, Portrait, SectionSpec,
    StateName, Status, SystemKind, Theme, Trajectory, TrajectoryTag,
};

fn labels(names: &[&str]) -> Labels {
    Labels {
        states: names
            .iter()
            .map(|n| StateName {
                name: n.to_string(),
                latex: n.to_string(),
            })
            .collect(),
        params: vec![ParamValue {
            name: "a".into(),
            latex: "a".into(),
            value: 0.1 + 0.2,
        }],
    }
}

fn run(id: &str, backward: bool, t: Vec<f64>, x: Vec<f64>, status: Status) -> Trajectory {
    Trajectory {
        tag: TrajectoryTag {
            ic_id: id.into(),
            backward,
            color: 2,
        },
        x0: vec![x[0], x[1]],
        t0: t[0],
        t,
        x,
        status,
    }
}

fn plane(trajectories: Vec<Trajectory>) -> Portrait {
    Portrait {
        kind: SystemKind::Ode,
        dim: 2,
        x_axis: Axis::State { index: 0 },
        y_axis: Axis::State { index: 1 },
        x_range: (-2.0, 2.0),
        y_range: (-1.5, 1.5),
        field_grid: 0,
        arrows: vec![],
        nullclines: vec![],
        fixed_points: vec![],
        trajectories,
        cobweb: false,
        graph: vec![],
        transient: 0,
        section: None,
    }
}

fn csv_of(trs: &[Trajectory], names: &[&str]) -> Vec<csv::CsvFile> {
    let l = labels(names);
    let lines = vec!["\\dot{x}=y".to_string(), "\\dot{y}=-ax".to_string()];
    csv::files(&CsvInput {
        kind: SystemKind::Ode,
        labels: &l,
        trajectories: trs,
        ic_ids: &[],
        lines: &lines,
        format: InputFormat::Latex,
        title: "Test: CSV",
        app_version: "0.2.0",
        transient: 0,
        section: None,
    })
}

#[test]
fn csv_joins_backward_and_forward_runs_in_time_order() {
    let back = run(
        "ic-1",
        true,
        vec![0.0, -0.5, -1.0, -1.5],
        vec![0.3, 0.2, 0.25, 0.1, f64::NAN, 0.0, 9.0, 9.0],
        Status::NonFinite,
    );
    let fwd = run(
        "ic-1",
        false,
        vec![0.0, 0.5, 1.0],
        vec![0.3, 0.2, 0.1 + 0.2, 1e-7, 0.4, 1e21],
        Status::Completed,
    );
    let files = csv_of(&[fwd, back], &["x", "y"]);
    assert_eq!(files.len(), 1);
    let f = &files[0];
    assert_eq!(f.stem, "test-csv-trajectory-1");
    // Backward: cut at its first non-finite sample (in integration order),
    // reversed; forward without its repeated start.
    assert_eq!(
        f.csv,
        "t,x,y\n-0.5,0.25,0.1\n0,0.3,0.2\n0.5,0.30000000000000004,1e-7\n1,0.4,1e+21\n"
    );
    assert!(!f.csv.contains('\r') && !f.csv.starts_with('\u{feff}'));
    // Every number reads back exactly.
    for line in f.csv.lines().skip(1) {
        for cell in line.split(',') {
            let v: f64 = cell.parse().unwrap();
            assert_eq!(separatrix_export::num::shortest(v), cell);
        }
    }
    let meta: serde_json::Value = serde_json::from_str(&f.meta).unwrap();
    assert!(f.meta.ends_with("}\n") && !f.meta.contains('\r'));
    assert_eq!(meta["columns"], serde_json::json!(["t", "x", "y"]));
    assert_eq!(meta["x0"], serde_json::json!([0.3, 0.2]));
    assert_eq!(meta["csv"], "test-csv-trajectory-1.csv");
    assert_eq!(meta["ic_id"], "ic-1", "the id stays in the metadata");
    assert_eq!(meta["segments"][0]["direction"], "backward");
    assert_eq!(meta["segments"][0]["samples"], 2);
    assert_eq!(meta["segments"][0]["status"], "nonFinite");
    assert_eq!(
        meta["segments"][0]["t_range"],
        serde_json::json!([-0.5, 0.0])
    );
    assert_eq!(
        meta["segments"][1]["t_range"],
        serde_json::json!([0.0, 1.0])
    );
    assert_eq!(
        meta["params"][0]["value"],
        serde_json::json!(0.30000000000000004)
    );
    assert_eq!(meta["system"]["input_format"], "latex");
}

#[test]
fn csv_headers_are_utf8_and_quoted_when_needed() {
    let fwd = run(
        "ic-2",
        false,
        vec![0.0, 1.0],
        vec![1.0, 2.0, 3.0, 4.0],
        Status::Completed,
    );
    let files = csv_of(&[fwd], &["θ,1", "p\"q\""]);
    assert!(files[0]
        .csv
        .starts_with("t,\"θ,1\",\"p\"\"q\"\"\"\n0,1,2\n1,3,4\n"));
    // A run that is not finite from the start has a header only.
    let bad = run(
        "ic-3",
        false,
        vec![f64::NAN, 1.0],
        vec![1.0, 2.0, 3.0, 4.0],
        Status::NonFinite,
    );
    let files = csv_of(&[bad], &["x", "y"]);
    assert_eq!(files[0].csv, "t,x,y\n");
}

fn figure_of(p: &Portrait, placement: Placement) -> separatrix_export::FigureOutput {
    let l = labels(&["x", "y"]);
    let pal = palette(Theme::Light);
    let lines = vec!["\\dot{x}=y".to_string()];
    figure(&FigureInput {
        portrait: p,
        labels: &l,
        width_cm: 8.0,
        height_cm: 6.0,
        theme: Theme::Light,
        palette: &pal,
        title: "Rules",
        source_ref: None,
        lines: &lines,
        format: InputFormat::Latex,
        placement,
        standalone: false,
    })
}

/// All numbers in the figure's data rows.
fn data_values(tex: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for chunk in tex.split("table[row sep=\\\\]{\n").skip(1) {
        let body = &chunk[..chunk.find('}').unwrap()];
        for line in body.lines().skip(1) {
            for cell in line.trim_end_matches("\\\\").split_whitespace() {
                if cell != "nan" {
                    out.push(cell.parse::<f64>().unwrap());
                }
            }
        }
    }
    out
}

#[test]
fn curves_far_outside_the_view_are_clipped() {
    let t: Vec<f64> = (0..400).map(f64::from).collect();
    let mut x = Vec::new();
    for k in 0..400 {
        let a = f64::from(k) / 40.0;
        // Out to 1e12 and back, through the view.
        x.extend([a.cos() * 10f64.powf(12.0 * (a / 10.0)), a.sin()]);
    }
    let p = plane(vec![run("ic-1", false, t, x, Status::Completed)]);
    let out = figure_of(&p, Placement::Inline);
    let vals = data_values(&out.tex);
    assert!(!vals.is_empty());
    assert!(
        vals.iter().all(|v| v.abs() < 10.0),
        "{:?}",
        vals.iter().fold(0.0f64, |m, v| m.max(v.abs()))
    );
    assert!(
        out.tex.contains("nan nan\\\\"),
        "the curve leaves and comes back: a break"
    );
}

#[test]
fn wiggly_curves_are_simplified_further_to_stay_under_5000_points() {
    // 12,000 samples zig-zagging 0.6 px: RDP at 0.25 px keeps them all.
    let n = 12_000;
    let t: Vec<f64> = (0..n).map(f64::from).collect();
    let mut x = Vec::new();
    let upy = 3.0 / (6.0 * 96.0 / 2.54);
    for k in 0..n {
        let s = if k % 2 == 0 { 0.3 } else { -0.3 };
        x.extend([-1.9 + 3.8 * f64::from(k) / f64::from(n), s * upy]);
    }
    let p = plane(vec![run("ic-1", false, t, x, Status::Completed)]);
    let out = figure_of(&p, Placement::Inline);
    assert!(out.points <= MAX_POINTS, "{}", out.points);
    assert!(!out.thinned);
    assert!(
        out.notes.iter().any(|n| n.contains("simplified")),
        "{:?}",
        out.notes
    );
}

#[test]
fn a_dense_field_keeps_everything_and_recommends_lualatex() {
    let mut p = plane(vec![]);
    p.field_grid = 100;
    for i in 0..100 {
        for j in 0..100 {
            p.arrows.push(Arrow {
                x: -2.0 + 4.0 * (f64::from(i) + 0.5) / 100.0,
                y: -1.5 + 3.0 * (f64::from(j) + 0.5) / 100.0,
                u: 1.0,
                v: 0.5,
            });
        }
    }
    let out = figure_of(&p, Placement::Inline);
    assert!(out.points > MAX_POINTS);
    assert!(out.notes.iter().any(|n| n.contains("lualatex")));
    assert!(out.tex.contains("% Large figure: compile with lualatex"));
}

#[test]
fn map_iterates_are_squares_above_2000_and_discs_below() {
    let mk = |n: usize| {
        let t: Vec<f64> = (0..n).map(|k| k as f64).collect();
        let mut x = Vec::new();
        for k in 0..n {
            let a = k as f64 * 0.618;
            x.extend([1.5 * (a.sin()), 1.2 * (a * 1.3).cos()]);
        }
        let mut p = plane(vec![run("ic-1", false, t, x, Status::Completed)]);
        p.kind = SystemKind::Map;
        p
    };
    let few = figure_of(&mk(250), Placement::Inline);
    assert!(few.tex.contains("mark=*, mark size=1.95bp"));
    let some = figure_of(&mk(1000), Placement::Inline);
    assert!(some.tex.contains("mark=*, mark size=0.975bp"));
    let many = figure_of(&mk(3000), Placement::Inline);
    assert!(many.tex.contains("mark=square*, mark size=0.5625bp"));
    assert!(!many.thinned);
    let lots = figure_of(&mk(20_000), Placement::Inline);
    assert!(lots.thinned && lots.points <= MAX_POINTS);
    let saved = figure_of(
        &mk(20_000),
        Placement::External {
            base: "lots".into(),
        },
    );
    assert!(!saved.thinned);
    assert!(saved.files.iter().any(|f| f.name == "lots-points.png"));
    assert!(saved.tex.contains("{\\sepxdir lots-points.png}"));
}

#[test]
fn glyphs_heads_and_end_marks() {
    let t: Vec<f64> = (0..200).map(f64::from).collect();
    let mut x = Vec::new();
    for k in 0..200 {
        x.extend([-1.8 + 3.6 * f64::from(k) / 200.0, 0.5]);
    }
    let mut p = plane(vec![run("ic-1", false, t, x, Status::BlewUp)]);
    p.fixed_points = vec![
        FixedPoint {
            x: vec![0.0, 0.0],
            eigenvalues: vec![(0.0, 1.0), (0.0, -1.0)],
            classification: Classification::Centre,
            stable: false,
        },
        FixedPoint {
            x: vec![1.0, 0.0],
            eigenvalues: vec![(-1.0, 0.0), (-2.0, 0.0)],
            classification: Classification::StableNode,
            stable: true,
        },
    ];
    let out = figure_of(&p, Placement::Inline);
    assert!(out.tex.contains("mark=halfcircle*"));
    assert!(out
        .tex
        .contains("\\usetikzlibrary{arrows.meta,calc,plotmarks}"));
    assert_eq!(
        out.tex.matches("\\fill[sepxtraj3]").count(),
        1,
        "one head under 600 px"
    );
    assert!(
        out.tex.contains("\\draw[sepxtraj3, line width=1.125bp]"),
        "the end mark of a run that blew up"
    );
    assert!(!out.tex.contains('\u{2212}'));
}

/// A cobweb of `orbit` (a one-dimensional map, both axes x).
fn cobweb(orbit: &[f64], status: Status) -> Portrait {
    let n = orbit.len();
    Portrait {
        kind: SystemKind::Map,
        dim: 1,
        x_axis: Axis::State { index: 0 },
        y_axis: Axis::State { index: 0 },
        x_range: (-0.2833, 1.3833),
        y_range: (-0.05, 1.05),
        field_grid: 0,
        arrows: vec![],
        nullclines: vec![],
        fixed_points: vec![FixedPoint {
            x: vec![0.6875],
            eigenvalues: vec![(-1.2, 0.0)],
            classification: Classification::Source,
            stable: false,
        }],
        trajectories: vec![Trajectory {
            tag: TrajectoryTag {
                ic_id: "ic-1".into(),
                backward: false,
                color: 0,
            },
            x0: vec![orbit[0]],
            t0: 0.0,
            t: (0..n).map(|k| k as f64).collect(),
            x: orbit.to_vec(),
            status,
        }],
        cobweb: true,
        graph: vec![(0..=100)
            .map(|k| {
                let x = f64::from(k) / 100.0;
                (x, 3.2 * x * (1.0 - x))
            })
            .collect()],
        transient: 0,
        section: None,
    }
}

fn cobweb_figure(p: &Portrait) -> separatrix_export::FigureOutput {
    let l = labels(&["x"]);
    let pal = palette(Theme::Light);
    let lines = vec!["x_{n+1}=ax_n\\left(1-x_n\\right)".to_string()];
    figure(&FigureInput {
        portrait: p,
        labels: &l,
        width_cm: 8.0,
        height_cm: 6.0,
        theme: Theme::Light,
        palette: &pal,
        title: "Cobweb",
        source_ref: None,
        lines: &lines,
        format: InputFormat::Latex,
        placement: Placement::Inline,
        standalone: false,
    })
}

/// The rows of the inline table of the plot after `marker` (a comment).
fn rows_after(tex: &str, marker: &str) -> Vec<Option<(f64, f64)>> {
    let rest = &tex[tex.find(marker).unwrap_or_else(|| panic!("no {marker}"))..];
    let open = "table[row sep=\\\\]{\n";
    let body = &rest[rest.find(open).unwrap() + open.len()..];
    let body = &body[..body.find('}').unwrap()];
    body.lines()
        .skip(1)
        .map(|l| {
            let cells: Vec<&str> = l.trim_end_matches("\\\\").split_whitespace().collect();
            if cells[0] == "nan" {
                None
            } else {
                Some((cells[0].parse().unwrap(), cells[1].parse().unwrap()))
            }
        })
        .collect()
}

#[test]
fn a_cobweb_draws_the_diagonal_the_graph_and_a_staircase_in_order() {
    let out = cobweb_figure(&cobweb(&[0.1, 0.288, 0.65618], Status::Completed));
    let tex = &out.tex;
    let at = |s: &str| tex.find(s).unwrap_or_else(|| panic!("no {s:?} in\n{tex}"));
    // z-order: diagonal, graph, staircase, fixed points.
    assert!(at("% Diagonal x_{n+1} = x_n") < at("% Graph of f"));
    assert!(at("% Graph of f") < at("% Cobweb of ic-1"));
    assert!(at("% Cobweb of ic-1") < at("mark=*"));
    // The diagonal across the whole view: from (max(x0, y0), ·) to (min(x1, y1), ·).
    assert!(tex.contains(
        "% Diagonal x_{n+1} = x_n\n\\addplot[sepxaxis, line width=0.75bp, line cap=butt] coordinates {(-0.05,-0.05) (1.05,1.05)};\n"
    ));
    assert!(tex.contains("% Graph of f\n\\addplot[sepxfp, line width=1.125bp, line join=round, line cap=round] table"));
    assert!(tex.contains(
        "% Cobweb of ic-1\n\\addplot[sepxtraj1, line width=0.75bp, line join=miter] table"
    ));
    let steps = rows_after(tex, "% Cobweb of ic-1");
    let v: Vec<(f64, f64)> = steps.into_iter().map(|r| r.unwrap()).collect();
    assert_eq!(
        v,
        vec![
            (0.1, 0.1),
            (0.1, 0.288),
            (0.288, 0.288),
            (0.288, 0.6562),
            (0.6562, 0.6562)
        ]
    );
    // The axis names, and no map dots or time-series line.
    assert!(tex.contains("{$x_{n}$}") && tex.contains("{$x_{n+1}$}"));
    assert!(!tex.contains("only marks, mark=*, mark size=1.95bp"));
    assert!(!tex.contains("opacity=0.4"));
    // The fixed point on the diagonal, open (|λ| > 1).
    assert!(tex.contains("mark options={fill=sepxbg, draw=sepxfp}"));
    let fps = rows_after(tex, "mark=*, mark size=5.625bp");
    let (x, y) = fps[0].unwrap();
    assert!(
        (x - 0.6875).abs() < 1e-3 && (y - 0.6875).abs() < 1e-3,
        "{fps:?}"
    );
}

#[test]
fn a_cobweb_that_blew_up_ends_in_an_x() {
    let out = cobweb_figure(&cobweb(&[0.5, 0.9, 1.0], Status::BlewUp));
    assert!(out.tex.contains("\\draw[sepxtraj1, line width=1.125bp]"));
    let done = cobweb_figure(&cobweb(&[0.5, 0.9, 1.0], Status::Completed));
    assert!(!done.tex.contains("\\draw[sepxtraj1"));
}

#[test]
fn a_cobweb_draws_a_converged_cycle_once() {
    // 1,000 iterates on a 2-cycle: 4 segments, however long the orbit.
    let (p, q) = (0.513_044_509_531_574_8, 0.799_455_490_468_425_2);
    let orbit: Vec<f64> = (0..1001).map(|k| if k % 2 == 0 { p } else { q }).collect();
    let out = cobweb_figure(&cobweb(&orbit, Status::Completed));
    let rows = rows_after(&out.tex, "% Cobweb of ic-1");
    let pieces = 1 + rows.iter().filter(|r| r.is_none()).count();
    let segments = rows.iter().filter(|r| r.is_some()).count() - pieces;
    assert!(segments <= 4, "{segments} segments: {rows:?}");
}

#[test]
fn a_transient_is_named_in_the_header() {
    let mut p = cobweb(&[0.5, 0.8], Status::Completed);
    p.transient = 200;
    let out = cobweb_figure(&p);
    assert!(out
        .tex
        .contains("% Transient: the first 200 iterates are not drawn.\n"));
    p.transient = 0;
    assert!(!cobweb_figure(&p).tex.contains("Transient"));
}

fn section_of(xs: &[f64], return_map: bool) -> Portrait {
    let n = xs.len();
    let mut p = plane(vec![run(
        "ic-1",
        false,
        (0..n).map(|k| k as f64).collect(),
        xs.iter().flat_map(|&x| [x, 0.0]).collect(),
        Status::Completed,
    )]);
    p.x_range = (0.0, 10.0);
    p.y_range = if return_map { (0.0, 10.0) } else { (-1.0, 1.0) };
    if return_map {
        p.y_axis = Axis::State { index: 0 };
    }
    p.section = Some(SectionSpec::Plane {
        state: 1,
        value: 0.0,
        direction: Direction::Falling,
    });
    p
}

#[test]
fn a_return_map_plots_successive_crossings_over_the_diagonal() {
    let out = figure_of(&section_of(&[1.0, 2.0, 4.0, 8.0], true), Placement::Inline);
    let tex = &out.tex;
    assert!(
        tex.contains("% Section: plane y = 0 (decreasing).\n"),
        "{tex}"
    );
    assert!(
        tex.contains("% Diagonal x_{k+1} = x_k\n\\addplot[sepxaxis"),
        "{tex}"
    );
    assert!(
        tex.contains("{$x_{k}$}") && tex.contains("{$x_{k+1}$}"),
        "{tex}"
    );
    let rest = &tex[tex
        .find("% Return map of ic-1\n")
        .expect("a return map comment")..];
    let body = &rest[rest.find("]{\n").unwrap() + 3..];
    let rows: Vec<&str> = body[..body.find('}').unwrap()].lines().skip(1).collect();
    assert_eq!(rows, ["1 2\\\\", "2 4\\\\", "4 8\\\\"]);
    assert!(!tex.contains("Skipped crossings"), "none skipped");

    let plain = figure_of(&section_of(&[1.0, 2.0, 4.0, 8.0], false), Placement::Inline);
    assert!(plain.tex.contains("% Section of ic-1\n"));
    assert!(!plain.tex.contains("% Diagonal") && !plain.tex.contains("\\fill[sepxtraj3]"));
}

#[test]
fn many_crossings_are_rasterised_or_thinned_like_map_iterates() {
    let xs: Vec<f64> = (0..6000).map(|k| f64::from(k) / 600.0).collect();
    let p = section_of(&xs, false);
    let inline = figure_of(&p, Placement::Inline);
    assert!(inline.thinned && inline.points <= MAX_POINTS);
    assert!(inline.tex.contains("mark=square*"), "squares above 2,000");
    assert!(
        inline.tex.contains("% One crossing in 2 is kept"),
        "{}",
        inline.tex
    );
    assert!(inline
        .tex
        .contains("% Thinned: some crossings were left out"));
    let saved = figure_of(
        &p,
        Placement::External {
            base: "section".into(),
        },
    );
    assert!(!saved.thinned);
    assert!(saved.files.iter().any(|f| f.name == "section-points.png"));
    assert!(saved
        .tex
        .contains("% The crossings (6000 points) are drawn as a 300-dpi PNG layer"));
}

#[test]
fn a_stroboscopic_section_is_named_in_the_csv_metadata() {
    let tr = run(
        "ic-1",
        false,
        vec![0.5, 6.5],
        vec![1.0, 2.0, 3.0, 4.0],
        Status::Completed,
    );
    let l = labels(&["x", "y"]);
    let lines = vec!["\\dot{x}=y".to_string()];
    let files = csv::files(&CsvInput {
        kind: SystemKind::Ode,
        labels: &l,
        trajectories: &[tr],
        ic_ids: &[],
        lines: &lines,
        format: InputFormat::Latex,
        title: "Forced",
        app_version: "0.2.0",
        transient: 3,
        section: Some(SectionSpec::Stroboscopic {
            period: 6.0,
            phase: 0.5,
        }),
    });
    assert_eq!(files[0].csv, "t,x,y\n0.5,1,2\n6.5,3,4\n");
    let meta: serde_json::Value = serde_json::from_str(&files[0].meta).unwrap();
    assert_eq!(meta["section"], "stroboscopic, period 6, phase 0.5");
    assert_eq!(meta["skipped_crossings"], 3);
    assert!(meta.get("transient").is_none());
    let note = meta["notes"][0].as_str().unwrap();
    assert!(note.contains("t0 is a row"), "{note}");
    assert!(!note.contains("the start point is not one"), "{note}");
}
