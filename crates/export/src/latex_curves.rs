//! The parameter plane as a pgfplots figure with the screen's frame, and
//! its codimension-2 points as a booktabs table.
//!
//! The figure mirrors `app/src/canvas/curves.ts` `drawCurves`: the across
//! parameter across and the up parameter up; the Branch view's range as a
//! bar at the up value its branches were computed at; each curve in its
//! kind's `curve-*` color and dash pattern ([`STYLES`]), 1.75 px wide, its
//! neutral-saddle stretches faded and dotted; the homoclinic curves; the
//! weak resonances 1:q as ticks across the Neimark–Sacker curves; then each
//! codimension-2 point once as a half-filled glyph on a halo, labelled
//! `BT`, `CP`, `GH`, … 7 px right of and 5 px above it. Each curve is one
//! group of `\addplot`s under a `% Curve k …` comment.
//!
//! The table lists every codimension-2 point once
//! ([`PlaneRun::special_points`]): its type, both parameters, the state, the
//! period for a map, the curves it lies on, and the coefficients the core
//! computed: the Bogdanov–Takens `a` and `b`, the cusp's `c`, the angle `θ`
//! on a Neimark–Sacker curve. A column no row fills is left out.

use crate::clip::{clip_polyline, Rect};
use crate::latex_figure::{
    at_px, definitions, file_base, finish, packages, table_text, FigureFile, FigureOutput, Frame,
    Placement, Quantizer, Table, FP_HALO_R, MAX_POINTS,
};
use crate::latex_system::header_comments;
use crate::latex_util::{bp, comment, escape_text, packages_comment, title_slug, PX_PER_CM};
use crate::model::{
    codim2_label, Codim2Kind, NormalForm, Palette, PlaneCurve, PlaneCurveKind, PlaneRun,
    SystemKind, SystemMeta, Theme, Viewport,
};
use crate::num;

const CLIP_GROW_PX: f64 = 8.0;
/// Half the length of a resonance tick, across the curve (`TICK_HALF`).
const TICK_HALF_PX: f64 = 7.0;
/// Half the height of the bars at the ends of the slice.
const SLICE_END_PX: f64 = 3.0;

/// How one curve kind is drawn: the TikZ color, the words of its comment
/// and its dash pattern in bp, the design system's (PhasePlane README,
/// Parameter plane): the screen's pattern at 0.75 bp per px.
pub struct CurveStyle {
    /// Which kind.
    pub kind: PlaneCurveKind,
    /// The color's TikZ name.
    pub color: &'static str,
    /// The kind in a comment.
    pub name: &'static str,
    /// The pattern in words.
    pub look: &'static str,
    /// `dash pattern=…`, or empty for a solid line.
    pub dash: &'static str,
}

/// Every curve kind's style, in [`PlaneCurveKind`] order.
pub const STYLES: [CurveStyle; 6] = [
    CurveStyle {
        kind: PlaneCurveKind::Fold,
        color: "sepxfold",
        name: "fold",
        look: "solid",
        dash: "",
    },
    CurveStyle {
        kind: PlaneCurveKind::Hopf,
        color: "sepxhopf",
        name: "Hopf",
        look: "dashed",
        dash: "on 6bp off 2.25bp",
    },
    CurveStyle {
        kind: PlaneCurveKind::BranchPoint,
        color: "sepxbp",
        name: "branch-point",
        look: "dash-dot-dotted",
        dash: "on 6bp off 1.875bp on 1.125bp off 1.875bp on 1.125bp off 1.875bp",
    },
    CurveStyle {
        kind: PlaneCurveKind::Flip,
        color: "sepxflip",
        name: "flip",
        look: "short dashes",
        dash: "on 2.25bp off 2.25bp",
    },
    CurveStyle {
        kind: PlaneCurveKind::NeimarkSacker,
        color: "sepxns",
        name: "Neimark–Sacker",
        look: "dashed",
        dash: "on 6bp off 2.25bp",
    },
    // The README has no bp pattern for homoclinic curves: this is its screen
    // pattern 8, 3, 1.5, 3 × s at the same 0.75 bp per px as the others.
    CurveStyle {
        kind: PlaneCurveKind::Homoclinic,
        color: "sepxhom",
        name: "homoclinic",
        look: "dash-dotted",
        dash: "on 6bp off 2.25bp on 1.125bp off 2.25bp",
    },
];

/// A neutral stretch: its kind's color at 0.55 opacity, 1.5 px, round dots.
const NEUTRAL_DASH: &str = "on 0bp off 2.6bp";

fn style(kind: PlaneCurveKind) -> &'static CurveStyle {
    &STYLES[kind as usize]
}

/// What [`curves_figure`] needs.
#[derive(Debug, Clone)]
pub struct CurvesFigureInput<'a> {
    /// The system's names, parameter values and source.
    pub sys: &'a SystemMeta<'a>,
    /// The curves.
    pub run: &'a PlaneRun,
    /// The view's across range.
    pub x_range: (f64, f64),
    /// The view's up range.
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

fn param_latex(sys: &SystemMeta, i: usize, fallback: &str) -> String {
    sys.labels
        .params
        .get(i)
        .map_or(fallback.to_string(), |p| p.latex.clone())
}

fn param_name(sys: &SystemMeta, i: usize, fallback: &str) -> String {
    sys.labels
        .params
        .get(i)
        .map_or(fallback.to_string(), |p| p.name.clone())
}

fn fixed_params(sys: &SystemMeta, free: (usize, usize)) -> Vec<(String, f64)> {
    sys.labels
        .params
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != free.0 && i != free.1)
        .map(|(_, p)| (p.name.clone(), p.value))
        .collect()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The point of `c` nearest `p` on screen and the unit normal there, turned
/// to point down the screen (`nearestSegment`): `(x, y, nx, ny)` in px.
fn nearest_segment(vp: &Viewport, c: &PlaneCurve, p: (f64, f64)) -> Option<(f64, f64, f64, f64)> {
    let (tx, ty) = (vp.sx(p.0), vp.sy(p.1));
    let mut best: Option<(f64, f64, f64, f64, f64)> = None;
    for i in 0..c.len().saturating_sub(1) {
        let (ax, ay) = (vp.sx(c.p1[i]), vp.sy(c.p2[i]));
        let (bx, by) = (vp.sx(c.p1[i + 1]), vp.sy(c.p2[i + 1]));
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt();
        // A zero-length (or NaN) segment has no normal.
        if len.is_nan() || len <= 0.0 {
            continue;
        }
        let t = (((tx - ax) * dx + (ty - ay) * dy) / (len * len)).clamp(0.0, 1.0);
        let (x, y) = (ax + t * dx, ay + t * dy);
        let d = ((x - tx) * (x - tx) + (y - ty) * (y - ty)).sqrt();
        if best.is_some_and(|b| d >= b.0) {
            continue;
        }
        let flip = if dx < 0.0 { -1.0 } else { 1.0 };
        best = Some((d, x, y, -dy / len * flip, dx / len * flip));
    }
    best.map(|(_, x, y, nx, ny)| (x, y, nx, ny))
}

/// The parameter plane.
pub fn curves_figure(input: &CurvesFigureInput) -> FigureOutput {
    let (sys, run, pal) = (input.sys, input.run, input.palette);
    let w = (input.width_cm * PX_PER_CM).max(10.0);
    let h = (input.height_cm * PX_PER_CM).max(10.0);
    let vp = Viewport::new(w, h, input.x_range, input.y_range);
    let (upx, upy) = vp.units_per_px();
    let frame = Frame::new(
        vp,
        param_latex(sys, run.free.0, "p"),
        param_latex(sys, run.free.1, "q"),
        false,
        false,
    );
    let quant = Quantizer {
        decimals: vec![
            Quantizer::decimals_for(upx, vp.x_range.1 - vp.x_range.0, vp.x_range.0, vp.x_range.1),
            Quantizer::decimals_for(upy, vp.y_range.1 - vp.y_range.0, vp.y_range.0, vp.y_range.1),
        ],
    };
    let grow = |px: f64| Rect {
        x0: vp.left - px,
        x1: vp.right() + px,
        y0: vp.top - px,
        y1: vp.bottom() + px,
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
    let mut use_color = |kind: PlaneCurveKind| {
        let name = style(kind).color;
        if !colors.iter().any(|(n, _)| n == name) {
            colors.push((name.into(), pal.curve(kind).to_string()));
        }
    };
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

    // Pixel `(x, y)` in data units, rounded as the tables are.
    let pt = |x: f64, y: f64| {
        format!(
            "({},{})",
            quant.write(0, vp.x_range.0 + (x - vp.left) * upx),
            quant.write(1, vp.y_range.1 - (y - vp.top) * upy)
        )
    };
    let segment = |color: &str, a: (f64, f64), b: (f64, f64)| {
        format!(
            "\\addplot[{color}, line width={}, line cap=butt] coordinates {{{} {}}};\n",
            bp(1.0),
            pt(a.0, a.1),
            pt(b.0, b.1)
        )
    };

    let up = param_name(sys, run.free.1, "q");
    let mut drew_slice = false;
    if let Some(slice) = run.slice {
        let y = vp.sy(slice.at);
        let (a, b) = (vp.sx(slice.range.0), vp.sx(slice.range.1));
        let (lo, hi) = (a.min(b), a.max(b));
        let r = grow(CLIP_GROW_PX);
        if y >= r.y0 && y <= r.y1 && hi >= r.x0 && lo <= r.x1 {
            let (x0, x1) = (lo.max(r.x0), hi.min(r.x1));
            body.push_str(&comment(&format!(
                "The Branch view's range, at {up} = {}",
                num::shortest(slice.at)
            )));
            body.push_str(&segment("sepxtick", (x0, y), (x1, y)));
            for (x, end) in [(x0, lo), (x1, hi)] {
                if x == end {
                    body.push_str(&segment(
                        "sepxtick",
                        (x, y - SLICE_END_PX),
                        (x, y + SLICE_END_PX),
                    ));
                }
            }
            drew_slice = true;
        }
    }

    let mut order: Vec<(usize, &PlaneCurve)> = run.curves.iter().enumerate().collect();
    order.sort_by_key(|(_, c)| c.kind);
    let mut kinds: Vec<PlaneCurveKind> = Vec::new();
    let mut any_neutral = false;
    for &(k, c) in &order {
        let st = style(c.kind);
        use_color(c.kind);
        if !kinds.contains(&c.kind) {
            kinds.push(c.kind);
        }
        let mut neutral = Table::new(&["x", "y"]);
        let mut main = Table::new(&["x", "y"]);
        for (from, to, is_neutral) in c.segment_runs() {
            let line: Vec<(f64, f64)> = (from..=to).map(|i| (c.p1[i], c.p2[i])).collect();
            let pieces = clip_polyline(&rect, &line);
            if is_neutral {
                neutral.push_pieces(&pieces);
            } else {
                main.push_pieces(&pieces);
            }
        }
        let period = if sys.kind == SystemKind::Map && c.period > 1 {
            format!(" of period {}", c.period)
        } else {
            String::new()
        };
        let head = format!("Curve {}: {}{period} ({})", k + 1, st.name, c.kind.label());
        if !neutral.rows.is_empty() {
            any_neutral = true;
            body.push_str(&comment(&format!("{head}, neutral saddle (dotted)")));
            add_table(
                &mut body,
                format!("curve-{}-neutral", k + 1),
                &neutral,
                &[format!(
                    "{}, opacity=0.55, line width={}, line cap=round, line join=round, dash pattern={NEUTRAL_DASH}",
                    st.color,
                    bp(1.5)
                )],
            );
        }
        if !main.rows.is_empty() {
            body.push_str(&comment(&format!("{head}, {}", st.look)));
            let dash = if st.dash.is_empty() {
                String::new()
            } else {
                format!(", dash pattern={}", st.dash)
            };
            add_table(
                &mut body,
                format!("curve-{}", k + 1),
                &main,
                &[format!(
                    "{}, line width={}, line cap=butt, line join=round{dash}",
                    st.color,
                    bp(1.75)
                )],
            );
        }
    }

    let inside = |x: f64, y: f64| {
        x >= vp.left - FP_HALO_R
            && x <= vp.right() + FP_HALO_R
            && y >= vp.top - FP_HALO_R
            && y <= vp.bottom() + FP_HALO_R
    };
    let mut ticks = String::new();
    for &(_, c) in &order {
        for tick in &c.ticks {
            let (px, py) = (vp.sx(tick.p.0), vp.sy(tick.p.1));
            if !(px.is_finite() && py.is_finite() && inside(px, py)) {
                continue;
            }
            let Some((x, y, nx, ny)) = nearest_segment(&vp, c, tick.p) else {
                continue;
            };
            let h = TICK_HALF_PX;
            ticks.push_str(&segment(
                style(c.kind).color,
                (x - nx * h, y - ny * h),
                (x + nx * h, y + ny * h),
            ));
            ticks.push_str(&format!(
                "\\node[anchor={}, sepxlabel, text=sepxtick, font=\\footnotesize] at {} {{$1{{:}}{}$}};\n",
                if nx >= 0.0 { "west" } else { "east" },
                at_px(x + nx * (h + 3.0), y + ny * (h + 3.0)),
                tick.q
            ));
        }
    }
    if !ticks.is_empty() {
        body.push_str(&comment("Weak resonances 1:q, each the tip of a tongue"));
        body.push_str(&ticks);
    }

    let rows = run.special_points();
    let mut marks = Table::new(&["x", "y"]);
    let mut labels = String::new();
    for r in &rows {
        let (a, b) = r.point.p;
        let (px, py) = (vp.sx(a), vp.sy(b));
        if !(px.is_finite() && py.is_finite() && inside(px, py)) {
            continue;
        }
        marks.rows.push(Some(vec![a, b]));
        labels.push_str(&format!(
            "\\node[anchor=south west, sepxlabel, text=sepxfp, font=\\footnotesize] at {} {{{}}};\n",
            at_px(px + 7.0, py - 5.0),
            codim2_label(r.point.kind)
        ));
    }
    let has_marks = !marks.rows.is_empty();
    if has_marks {
        body.push_str(&comment("Codimension-2 points"));
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
        "parameter plane",
        sys.title,
        sys.source_ref,
        sys.lines,
        sys.format,
        &fixed_params(sys, run.free),
    );
    let libs: &[&str] = if has_marks {
        &["calc", "plotmarks"]
    } else {
        &["calc"]
    };
    let (needs, preamble) = packages(libs);
    tex.push_str(&needs);
    let across = param_name(sys, run.free.0, "p");
    let ((a0, a1), (b0, b1)) = run.ranges;
    tex.push_str(&comment(&format!(
        "Parameter plane: {up} against {across}, followed in {across} from {} to {} and {up} from {} to {}; {}, {}.",
        num::shortest(a0),
        num::shortest(a1),
        num::shortest(b0),
        num::shortest(b1),
        plural(run.curves.len(), "curve", "curves"),
        plural(rows.len(), "codimension-2 point", "codimension-2 points")
    )));
    let mut legend: Vec<String> = kinds
        .iter()
        .map(|&k| format!("{} {} ({})", k.label(), style(k).name, style(k).look))
        .collect();
    if any_neutral {
        legend.push("dotted where a curve is a neutral saddle, no bifurcation".into());
    }
    if drew_slice {
        legend.push("the bar is the Branch view's range".into());
    }
    if !legend.is_empty() {
        tex.push_str(&comment(&format!("Legend: {}.", legend.join("; "))));
    }
    let mut notes = Vec::new();
    if points > MAX_POINTS {
        notes.push(format!(
            "The curves have {points} points; compile with lualatex if pdflatex runs out of memory."
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

fn codim2_words(kind: Codim2Kind) -> &'static str {
    match kind {
        Codim2Kind::BogdanovTakens => "Bogdanov--Takens",
        Codim2Kind::Cusp => "cusp",
        Codim2Kind::Bautin => "Bautin (generalized Hopf)",
        Codim2Kind::ZeroHopf => "zero-Hopf",
        Codim2Kind::HopfHopf => "Hopf--Hopf",
        Codim2Kind::Resonance1 => "1:1 resonance",
        Codim2Kind::Resonance2 => "1:2 resonance",
        Codim2Kind::Resonance3 => "1:3 resonance",
        Codim2Kind::Resonance4 => "1:4 resonance",
        Codim2Kind::Chenciner => "Chenciner",
        Codim2Kind::FoldFlip => "fold-flip",
        Codim2Kind::GeneralizedFlip => "generalized flip",
    }
}

fn curve_words(kind: PlaneCurveKind) -> &'static str {
    match kind {
        PlaneCurveKind::Fold => "fold",
        PlaneCurveKind::Hopf => "Hopf",
        PlaneCurveKind::BranchPoint => "branch point",
        PlaneCurveKind::Flip => "flip",
        PlaneCurveKind::NeimarkSacker => "Neimark--Sacker",
        PlaneCurveKind::Homoclinic => "homoclinic",
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

/// The codimension-2 points as a booktabs table, with its leading comment
/// lines.
pub fn plane_points_table(sys: &SystemMeta, run: &PlaneRun) -> String {
    let rows = run.special_points();
    let map = sys.kind == SystemKind::Map;
    let (p1, p2) = (
        param_latex(sys, run.free.0, "p"),
        param_latex(sys, run.free.1, "q"),
    );
    let names: Vec<&str> = sys.labels.states.iter().map(|s| s.latex.as_str()).collect();
    let bt = |r: &crate::model::PlanePointRow| match r.normal_form {
        Some(NormalForm::BogdanovTakens { a, b }) => Some((a, b)),
        _ => None,
    };
    let cusp = |r: &crate::model::PlanePointRow| match r.normal_form {
        Some(NormalForm::Cusp { c }) => Some(c),
        _ => None,
    };
    let has_bt = rows.iter().any(|r| bt(r).is_some());
    let has_cusp = rows.iter().any(|r| cusp(r).is_some());
    let has_theta = rows.iter().any(|r| r.angle.is_some());
    let scale = rows
        .iter()
        .flat_map(|r| r.point.x.iter().copied())
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
        "codimension-2 points",
        sys.title,
        sys.source_ref,
        sys.lines,
        sys.format,
        &fixed_params(sys, run.free),
    );
    tex.push_str(&packages_comment(&["amsmath", "booktabs", "siunitx"]));

    let title = sys.title.trim();
    let mut caption = format!(
        "{}: codimension-two points of the bifurcation curves in the $({p1}, {p2})$ plane",
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
        .filter(|&(i, _)| i != run.free.0 && i != run.free.1)
        .map(|(_, p)| format!("${} = {}$", p.latex, num::tex(p.value, 6)))
        .collect();
    if !others.is_empty() {
        caption.push_str(&format!(", with {}", crate::words::join_and(&others)));
    }
    caption.push('.');
    let mut kinds: Vec<Codim2Kind> = Vec::new();
    let mut curves: Vec<PlaneCurveKind> = Vec::new();
    for r in &rows {
        if !kinds.contains(&r.point.kind) {
            kinds.push(r.point.kind);
        }
        for k in &r.on {
            if !curves.contains(k) {
                curves.push(*k);
            }
        }
    }
    curves.sort();
    if !kinds.is_empty() {
        let legend: Vec<String> = kinds
            .iter()
            .map(|k| format!("{}: {}", codim2_label(*k), codim2_words(*k)))
            .collect();
        caption.push_str(&format!(" {}.", legend.join("; ")));
        let on: Vec<String> = curves
            .iter()
            .map(|k| format!("{} {}", k.label(), curve_words(*k)))
            .collect();
        caption.push_str(&format!(
            " Curves names the curves each point was found on ({}).",
            on.join(", ")
        ));
    }
    if map {
        caption.push_str(" The state of a cycle is its first point.");
    }
    if has_bt {
        caption.push_str(
            " $a$ and $b$ are the Bogdanov--Takens coefficients of $\\dot w_0 = w_1$, \
             $\\dot w_1 = \\beta_1 + \\beta_2 w_0 + a w_0^2 + b w_0 w_1$, oriented so that \
             $a \\ge 0$: the sign of $b$ is that of $ab$.",
        );
    }
    if has_cusp {
        caption.push_str(
            " $c$ is the cusp's coefficient in $\\dot w = \\beta_1 + \\beta_2 w + c w^3$, \
             with $\\lVert q \\rVert = 1$.",
        );
    }
    if has_theta {
        caption.push_str(
            " $\\theta$ is the angle of the critical multipliers $e^{\\pm i\\theta}$ on the \
             Neimark--Sacker curve.",
        );
    }

    let state_head = if names.len() == 1 {
        format!("${}$", names[0])
    } else {
        format!("$({})$", names.join(", "))
    };
    let mut head = vec![
        "Type".to_string(),
        format!("${p1}$"),
        format!("${p2}$"),
        state_head,
    ];
    if map {
        head.push("Period".into());
    }
    head.push("Curves".into());
    if has_bt {
        head.push("$a$".into());
        head.push("$b$".into());
    }
    if has_cusp {
        head.push("$c$".into());
    }
    if has_theta {
        head.push("$\\theta$".into());
    }

    tex.push_str("\\begin{table}\n\\centering\n");
    tex.push_str(&format!("\\caption{{{caption}}}\n"));
    tex.push_str(&format!(
        "\\label{{tab:{}-plane-points}}\n",
        title_slug(sys.title)
    ));
    tex.push_str(&format!(
        "\\begin{{tabular}}{{@{{}}{}@{{}}}}\n\\toprule\n{} \\\\\n\\midrule\n",
        "l".repeat(head.len()),
        head.join(" & ")
    ));
    if rows.is_empty() {
        tex.push_str(&format!(
            "\\multicolumn{{{}}}{{@{{}}l@{{}}}}{{No codimension-two point was found on the curves.}} \\\\\n",
            head.len()
        ));
    }
    for r in &rows {
        let sp = r.point;
        let state: Vec<String> = sp.x.iter().map(|&v| state_num(v)).collect();
        let state = if state.len() == 1 {
            state[0].clone()
        } else {
            format!("$({})$", state.join(", "))
        };
        let mut cells = vec![
            codim2_label(sp.kind).to_string(),
            fixed6(sp.p.0),
            fixed6(sp.p.1),
            state,
        ];
        if map {
            cells.push(format!("\\num{{{}}}", r.period));
        }
        cells.push(
            r.on.iter()
                .map(|k| k.label())
                .collect::<Vec<_>>()
                .join(", "),
        );
        if has_bt {
            match bt(r) {
                Some((a, b)) => {
                    cells.push(num::si(a, 6));
                    cells.push(num::si(b, 6));
                }
                None => cells.extend(["--".to_string(), "--".to_string()]),
            }
        }
        if has_cusp {
            cells.push(cusp(r).map_or("--".into(), |c| num::si(c, 6)));
        }
        if has_theta {
            cells.push(r.angle.map_or("--".into(), fixed6));
        }
        tex.push_str(&format!("{} \\\\\n", cells.join(" & ")));
    }
    tex.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n");
    tex
}
