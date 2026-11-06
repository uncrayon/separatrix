//! A branch plot (Strogatz §3.1–3.4, §8.2) as a pgfplots figure with the
//! screen's frame, and its special points as a booktabs table.
//!
//! The figure mirrors `app/src/canvas/branch.ts` `drawBranch`: the swept
//! parameter across and a state (or ‖x‖₂) up; each branch in
//! `traj-(color mod 7 + 1)`, 1.75 px wide, every point of a cycle drawn;
//! stable stretches solid and unstable ones dashed (6 px on, 4 px off),
//! split at every special point; then each special point as a half-filled
//! fixed-point glyph on a halo, at every point of its cycle, labelled `LP`,
//! `H`, `BP`, `PD` or `NS` 7 px right of and 5 px above the first. Each
//! branch's two stabilities are two `\addplot`s (breaks are `nan` rows),
//! each under a `% Branch k …` comment.
//!
//! The table lists every special point once in increasing parameter
//! ([`ContinuationRun::special_points`]): its type, the parameter, the state
//! (a cycle's first point), the period for a map, the Hopf frequency ω or
//! the Neimark–Sacker angle θ, and the first Lyapunov coefficient ℓ₁. The
//! parameter and ω/θ are `\num{}` to six decimals ([`num::to_fixed`]),
//! states and ℓ₁ six significant digits ([`num::si`]); both round the exact
//! decimal value of the double, so the table is the same on every platform.
//! An empty column (no Hopf or Neimark–Sacker point, no ℓ₁) is left out.

use crate::clip::{clip_polyline, Rect};
use crate::latex_figure::{
    at_px, definitions, file_base, finish, packages, table_text, FigureFile, FigureOutput, Frame,
    Placement, Quantizer, Table, FP_HALO_R, MAX_POINTS,
};
use crate::latex_system::header_comments;
use crate::latex_util::{bp, comment, escape_text, packages_comment, slug, PX_PER_CM};
use crate::model::{
    BranchMeasure, ContinuationRun, Palette, SpecialKind, SystemKind, SystemMeta, Theme, Viewport,
    PALETTE_SIZE,
};
use crate::num;

const CLIP_GROW_PX: f64 = 8.0;

/// What [`continuation_figure`] needs.
#[derive(Debug, Clone)]
pub struct ContinuationFigureInput<'a> {
    /// The system's names, parameter values and source.
    pub sys: &'a SystemMeta<'a>,
    /// The branches.
    pub run: &'a ContinuationRun,
    /// The view's parameter range (across).
    pub x_range: (f64, f64),
    /// The view's range of the measure (up).
    pub y_range: (f64, f64),
    /// Width of the plot box.
    pub width_cm: f64,
    /// Height of the plot box.
    pub height_cm: f64,
    /// Which theme the palette is.
    pub theme: Theme,
    /// The plot colors.
    pub palette: &'a Palette,
    /// Inline or external data.
    pub placement: Placement,
    /// Wrap in a `standalone` document.
    pub standalone: bool,
}

fn measure_latex(sys: &SystemMeta, m: BranchMeasure) -> String {
    match m {
        BranchMeasure::State { index } => sys
            .labels
            .states
            .get(index)
            .map_or("x".to_string(), |s| s.latex.clone()),
        BranchMeasure::Norm => "\\lVert x \\rVert".to_string(),
    }
}

fn measure_name(sys: &SystemMeta, m: BranchMeasure) -> String {
    match m {
        BranchMeasure::State { index } => sys
            .labels
            .states
            .get(index)
            .map_or("x".to_string(), |s| s.name.clone()),
        BranchMeasure::Norm => "the norm of the state".to_string(),
    }
}

fn fixed_params(sys: &SystemMeta, swept: usize) -> Vec<(String, f64)> {
    sys.labels
        .params
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != swept)
        .map(|(_, p)| (p.name.clone(), p.value))
        .collect()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The branch plot.
pub fn continuation_figure(input: &ContinuationFigureInput) -> FigureOutput {
    let (sys, run, pal) = (input.sys, input.run, input.palette);
    let dim = sys.labels.states.len();
    let w = (input.width_cm * PX_PER_CM).max(10.0);
    let h = (input.height_cm * PX_PER_CM).max(10.0);
    let vp = Viewport::new(w, h, input.x_range, input.y_range);
    let (upx, upy) = vp.units_per_px();
    let param = sys.labels.params.get(run.param);
    let frame = Frame::new(
        vp,
        param.map_or("r".to_string(), |p| p.latex.clone()),
        measure_latex(sys, run.y_measure),
        false,
        false,
    );
    let quant = Quantizer {
        decimals: vec![
            Quantizer::decimals_for(upx, vp.x_range.1 - vp.x_range.0, vp.x_range.0, vp.x_range.1),
            Quantizer::decimals_for(upy, vp.y_range.1 - vp.y_range.0, vp.y_range.0, vp.y_range.1),
        ],
    };
    let rect = Rect {
        x0: vp.x_range.0 - CLIP_GROW_PX * upx,
        x1: vp.x_range.1 + CLIP_GROW_PX * upx,
        y0: vp.y_range.0 - CLIP_GROW_PX * upy,
        y1: vp.y_range.1 + CLIP_GROW_PX * upy,
    };
    let external = match &input.placement {
        Placement::External { base } => Some(file_base(base)),
        Placement::Inline => None,
    };

    let mut colors: Vec<(String, String)> = vec![
        ("sepxbg".into(), pal.bg.clone()),
        ("sepxgrid".into(), pal.grid.clone()),
        ("sepxaxis".into(), pal.axis.clone()),
        ("sepxtick".into(), pal.tick.clone()),
        ("sepxfp".into(), pal.fp.clone()),
        ("sepxhalo".into(), pal.halo.clone()),
    ];
    let mut body = frame.axis_begin(input.theme);
    let mut files: Vec<FigureFile> = Vec::new();
    let mut points = 0;
    let mut add_table = |body: &mut String, name: String, t: &Table, options: &[String]| {
        let data = table_text(t, &quant, external.is_none());
        let source = match &external {
            None => format!("table[row sep=\\\\]{{\n{data}}}"),
            Some(base) => {
                let file = format!("{base}-{name}.dat");
                files.push(FigureFile {
                    name: file.clone(),
                    bytes: data.into_bytes(),
                });
                format!("table {{\\sepxdir {file}}}")
            }
        };
        points += t.data_rows();
        for o in options {
            body.push_str(&format!("\\addplot[{o}] {source};\n"));
        }
    };

    for (b, br) in run.branches.iter().enumerate() {
        let slot = br.color as usize % PALETTE_SIZE;
        let color = format!("sepxtraj{}", slot + 1);
        if !colors.iter().any(|(n, _)| *n == color) {
            colors.push((color.clone(), pal.traj[slot % pal.traj.len()].clone()));
        }
        let mut stable = Table::new(&["x", "y"]);
        let mut unstable = Table::new(&["x", "y"]);
        for (from, to, is_stable) in br.stability_runs() {
            for m in 0..br.period {
                let line: Vec<(f64, f64)> = (from..=to)
                    .map(|i| (br.p[i], run.y_measure.value(br.state(dim, i, m))))
                    .collect();
                let pieces = clip_polyline(&rect, &line);
                if is_stable {
                    stable.push_pieces(&pieces);
                } else {
                    unstable.push_pieces(&pieces);
                }
            }
        }
        let width = format!(
            "{color}, line width={}, line cap=butt, line join=round",
            bp(1.75)
        );
        let period = if sys.kind == SystemKind::Map && br.period > 1 {
            format!(" (period {})", br.period)
        } else {
            String::new()
        };
        for (kind, word, t, dash) in [
            ("stable", "stable (solid)", &stable, ""),
            (
                "unstable",
                "unstable (dashed)",
                &unstable,
                ", dash pattern=on 4.5bp off 3bp",
            ),
        ] {
            if t.rows.is_empty() {
                continue;
            }
            body.push_str(&comment(&format!("Branch {}{period}, {word}", b + 1)));
            add_table(
                &mut body,
                format!("branch-{}-{kind}", b + 1),
                t,
                &[format!("{width}{dash}")],
            );
        }
    }

    let mut marks = Table::new(&["x", "y"]);
    let mut labels = String::new();
    let inside = |x: f64, y: f64| {
        x >= vp.left - FP_HALO_R
            && x <= vp.right() + FP_HALO_R
            && y >= vp.top - FP_HALO_R
            && y <= vp.bottom() + FP_HALO_R
    };
    for br in &run.branches {
        for sp in &br.special {
            for m in 0..br.period {
                let y = run.y_measure.value(br.state(dim, sp.index, m));
                let (px, py) = (vp.sx(sp.p), vp.sy(y));
                if !(px.is_finite() && py.is_finite() && inside(px, py)) {
                    continue;
                }
                marks.rows.push(Some(vec![sp.p, y]));
                if m == 0 {
                    labels.push_str(&format!(
                        "\\node[anchor=south west, sepxlabel, text=sepxfp, font=\\footnotesize] at {} {{{}}};\n",
                        at_px(px + 7.0, py - 5.0),
                        sp.kind.label()
                    ));
                }
            }
        }
    }
    let has_marks = !marks.rows.is_empty();
    if has_marks {
        body.push_str(&comment("Special points"));
        add_table(
            &mut body,
            "special-points".into(),
            &marks,
            &[
                format!(
                    "sepxhalo, only marks, mark=*, mark size={}, mark options={{draw opacity=0, fill=sepxhalo}}",
                    bp(FP_HALO_R)
                ),
                format!(
                    "only marks, sepxfp, mark=halfcircle*, mark options={{rotate=90, fill=sepxfp, draw=sepxfp}}, mark size={}, line width={}",
                    bp(5.0),
                    bp(1.5)
                ),
            ],
        );
        body.push_str(&labels);
    }
    body.push_str(&frame.axis_end());

    let mut tex = header_comments(
        "branch plot",
        sys.title,
        sys.source_ref,
        sys.lines,
        sys.format,
        &fixed_params(sys, run.param),
    );
    let libs: &[&str] = if has_marks {
        &["calc", "plotmarks"]
    } else {
        &["calc"]
    };
    let (needs, preamble) = packages(libs);
    tex.push_str(&needs);
    tex.push_str(&comment(&format!(
        "Branch plot: {} against {} from {} to {}; {}, {}.",
        measure_name(sys, run.y_measure),
        param.map_or("r", |p| p.name.as_str()),
        num::shortest(run.range.0),
        num::shortest(run.range.1),
        plural(run.branches.len(), "branch", "branches"),
        plural(
            run.special_points().len(),
            "special point",
            "special points"
        )
    )));
    let mut notes = Vec::new();
    if points > MAX_POINTS {
        notes.push(format!(
            "The branches have {points} points; compile with lualatex if pdflatex runs out of memory."
        ));
    }
    for n in &notes {
        tex.push_str(&comment(n));
    }
    let defs = definitions(external.is_some(), &colors);
    FigureOutput {
        tex: finish(tex, &preamble, &defs, &body, input.standalone),
        files,
        points,
        thinned: false,
        notes,
    }
}

fn kind_words(kind: SpecialKind) -> &'static str {
    match kind {
        SpecialKind::Fold => "fold (limit point)",
        SpecialKind::Hopf => "Hopf bifurcation",
        SpecialKind::BranchPoint => "branch point",
        SpecialKind::Flip => "period doubling (flip)",
        SpecialKind::NeimarkSacker => "Neimark--Sacker bifurcation",
    }
}

const SNAP: f64 = 1e-12;

fn fixed6(v: f64) -> String {
    if v.is_finite() {
        format!("\\num{{{}}}", num::to_fixed(v, 6))
    } else {
        num::si(v, 6)
    }
}

/// The special points as a booktabs table, with its leading comment lines.
pub fn special_points_table(sys: &SystemMeta, run: &ContinuationRun) -> String {
    let rows = run.special_points();
    let map = sys.kind == SystemKind::Map;
    let param = sys.labels.params.get(run.param);
    let p_latex = param.map_or("r".to_string(), |p| p.latex.clone());
    let names: Vec<&str> = sys.labels.states.iter().map(|s| s.latex.as_str()).collect();
    let has_omega = rows.iter().any(|r| r.2.omega.is_some());
    let has_l1 = rows.iter().any(|r| r.2.lyapunov.is_some());
    let has_theta = rows
        .iter()
        .any(|r| r.2.kind == SpecialKind::NeimarkSacker && r.2.omega.is_some());
    let has_hopf = rows
        .iter()
        .any(|r| r.2.kind == SpecialKind::Hopf && r.2.omega.is_some());
    let scale = rows
        .iter()
        .flat_map(|r| r.2.x.iter().copied())
        .filter(|v| v.is_finite())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    let state_num = |v: f64| {
        if v.abs() < SNAP * scale.max(1.0) {
            num::si(0.0, 6)
        } else {
            num::si(v, 6)
        }
    };

    let mut tex = header_comments(
        "special points",
        sys.title,
        sys.source_ref,
        sys.lines,
        sys.format,
        &fixed_params(sys, run.param),
    );
    tex.push_str(&packages_comment(&["amsmath", "booktabs", "siunitx"]));

    let title = sys.title.trim();
    let mut caption = format!(
        "{}: special points of the branches as ${p_latex}$ varies",
        if title.is_empty() {
            "Untitled system".to_string()
        } else {
            escape_text(title)
        }
    );
    let others: Vec<String> = sys
        .labels
        .params
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != run.param)
        .map(|(_, p)| format!("${} = {}$", p.latex, num::tex(p.value, 6)))
        .collect();
    if !others.is_empty() {
        caption.push_str(&format!(", with {}", crate::words::join_and(&others)));
    }
    caption.push('.');
    let mut kinds: Vec<SpecialKind> = Vec::new();
    for r in &rows {
        if !kinds.contains(&r.2.kind) {
            kinds.push(r.2.kind);
        }
    }
    if !kinds.is_empty() {
        let legend: Vec<String> = kinds
            .iter()
            .map(|k| format!("{}: {}", k.label(), kind_words(*k)))
            .collect();
        caption.push_str(&format!(" {}.", legend.join("; ")));
    }
    if map {
        caption.push_str(" The state of a cycle is its first point.");
    }
    if has_hopf {
        caption.push_str(" $\\omega$ is the frequency at a Hopf point.");
    }
    if has_theta {
        caption.push_str(" $\\theta = \\arg\\mu$ is the angle of the critical multiplier.");
    }
    if has_l1 {
        caption.push_str(
            " $\\ell_1$ is the first Lyapunov coefficient, negative when the Hopf bifurcation is \
             supercritical, scaled so that $\\dot r = \\mu r + a r^3$, $\\dot\\theta = \\omega$ has \
             $\\ell_1 = a/\\omega$.",
        );
    }

    let state_head = if names.len() == 1 {
        format!("${}$", names[0])
    } else {
        format!("$({})$", names.join(", "))
    };
    let omega_head = match (has_hopf, has_theta) {
        (true, true) => "$\\omega$ / $\\theta$",
        (false, true) => "$\\theta$",
        _ => "$\\omega$",
    };
    let mut head = vec!["Type".to_string(), format!("${p_latex}$"), state_head];
    if map {
        head.push("Period".into());
    }
    if has_omega {
        head.push(omega_head.into());
    }
    if has_l1 {
        head.push("$\\ell_1$".into());
    }

    tex.push_str("\\begin{table}\n\\centering\n");
    tex.push_str(&format!("\\caption{{{caption}}}\n"));
    tex.push_str(&format!(
        "\\label{{tab:{}-special-points}}\n",
        slug(sys.title, "system")
    ));
    tex.push_str(&format!(
        "\\begin{{tabular}}{{@{{}}{}@{{}}}}\n\\toprule\n{} \\\\\n\\midrule\n",
        "l".repeat(head.len()),
        head.join(" & ")
    ));
    if rows.is_empty() {
        tex.push_str(&format!(
            "\\multicolumn{{{}}}{{@{{}}l@{{}}}}{{No special point was found on the branches.}} \\\\\n",
            head.len()
        ));
    }
    for (_, period, sp) in &rows {
        let state: Vec<String> = sp.x.iter().map(|&v| state_num(v)).collect();
        let state = if state.len() == 1 {
            state[0].clone()
        } else {
            format!("$({})$", state.join(", "))
        };
        let mut cells = vec![sp.kind.label().to_string(), fixed6(sp.p), state];
        if map {
            cells.push(format!("\\num{{{period}}}"));
        }
        if has_omega {
            cells.push(sp.omega.map_or("--".into(), fixed6));
        }
        if has_l1 {
            cells.push(sp.lyapunov.map_or("--".into(), |v| num::si(v, 6)));
        }
        tex.push_str(&format!("{} \\\\\n", cells.join(" & ")));
    }
    tex.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n");
    tex
}
