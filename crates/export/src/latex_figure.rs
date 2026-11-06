//! The phase portrait as a pgfplots figure that mirrors the screen
//! (`app/src/canvas/draw.ts`, light palette by default).
//!
//! All lengths are in big points (1 CSS px = 0.75 bp) on a figure of
//! `width_cm` × `height_cm` (96 px per inch). The axis is `scale only axis`
//! with the display ranges, so a pixel of the screen's drawing rules is a
//! pixel of the figure.
//!
//! - Grid: pgfplots' grid at every half step (`xtick` and `minor xtick`
//!   lists, so the lines beyond the first and last tick are kept), in
//!   `plot-grid`.
//! - Axis lines where 0 is in view (`axis x line*=middle`), none otherwise.
//!   Tick marks (4 px, below and to the left) and tick labels are TikZ
//!   elements placed by the screen's `layoutLabels` rule: below or above
//!   the x axis, left or right of the y axis, one `0` where both cross,
//!   and a label that would leave the plot or overlap another is left out.
//! - Axis names are placed by `axisNamePlacement`'s rule.
//! - Plots in z-order: field (quiver, data units), nullclines (dashed),
//!   trajectories (backward first, half opacity) with arrow heads where the
//!   screen puts them ([`crate::heads`]), map iterates (dots, squares above
//!   2,000), fixed points (halo, then the glyph).
//! - A cobweb (`Portrait::cobweb`, Strogatz §10.1) draws, in that order, the
//!   diagonal `x_{n+1} = x_n` (`plot-axis`, 0.75 bp), the graph of `f`
//!   (`plot-fp`, 1.125 bp), one staircase per orbit ([`crate::cobweb`];
//!   `traj-k`, 0.75 bp, miter joins, each segment once) and the fixed points
//!   on the diagonal, under the axis names `x_{n}` and `x_{n+1}`; no map
//!   dots. After a transient an open square marks where each staircase
//!   starts, `(x_T, x_T)`. Each of the three gets a comment line (`% Diagonal x_{n+1} =
//!   x_n`, `% Graph of f`, `% Cobweb of <ic>`), the figure's only per-plot
//!   comments, and a transient is named in the header.
//! - A Poincaré section (`Portrait::section`, Strogatz §8.7) draws each
//!   orbit's crossings as map iterates are drawn, under a `% Section of
//!   <ic>` comment. With both axes one state it is the return map: the
//!   pairs `(x_k, x_{k+1})` under `% Return map of <ic>`, over the diagonal
//!   (`% Diagonal x_{k+1} = x_k`), with the axis names `x_{k}` and
//!   `x_{k+1}`. The header names the section and the skipped crossings.
//! - Every curve is clipped to the view grown by two field cells, and
//!   breaks are `nan` rows with `unbounded coords=jump`.
//! - Data is inline (`table[row sep=\\]{…}`, for Copy) or in `.dat` files
//!   next to the `.tex`, read through `\sepxdir` (for Save).
//! - At most [`MAX_POINTS`] rows per figure: curves are simplified further,
//!   then map iterates and crossings go to a PNG layer (Save) or are
//!   thinned (Copy).

use dyn_expr::InputFormat;
use serde::{Deserialize, Serialize};

use crate::clip::{clip_polyline, Rect};
use crate::heads::{self, Head, HEAD_LENGTH_PX, MIN_GAP_PX};
use crate::latex_system::header_comments;
use crate::latex_util::{
    bp, comment, define_color, fixed_trim, slug, tex_tick_label, BP_PER_PX, PX_PER_CM,
};
use crate::model::{
    Axis, Labels, OrbitRun, Palette, Portrait, SystemKind, SystemMeta, Theme, Viewport,
    PALETTE_SIZE,
};
use crate::num::shortest;
use crate::raster;
use crate::simplify::{finite_prefix, rdp_indices};
use crate::ticks;

/// Most data rows a figure holds before it is lightened.
pub const MAX_POINTS: usize = 5000;

/// Further simplification tolerances tried, in px, when a figure is over
/// [`MAX_POINTS`].
const EXTRA_TOLERANCES: [f64; 3] = [0.5, 1.0, 2.0];

/// Map iterates per trajectory above which they are drawn as squares.
const SQUARE_DOTS_ABOVE: usize = 2000;

/// PNG layer resolution.
pub const RASTER_DPI: f64 = 300.0;

/// Where the figure's data goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Placement {
    /// Inside the snippet (Copy).
    Inline,
    /// In files `<base>-….dat` next to the `.tex` (Save).
    External {
        /// File name stem (made a slug).
        base: String,
    },
}

/// What [`figure`] needs.
#[derive(Debug, Clone)]
pub struct FigureInput<'a> {
    /// The portrait computed at the figure's aspect (simplified or not).
    pub portrait: &'a Portrait,
    /// State and parameter names.
    pub labels: &'a Labels,
    /// Width of the plot box.
    pub width_cm: f64,
    /// Height of the plot box.
    pub height_cm: f64,
    /// Which theme the palette is.
    pub theme: Theme,
    /// The plot colors.
    pub palette: &'a Palette,
    /// Title, for the comments.
    pub title: &'a str,
    /// Where the system comes from.
    pub source_ref: Option<&'a str>,
    /// The lines as typed, for the comments.
    pub lines: &'a [String],
    /// Their format.
    pub format: InputFormat,
    /// Inline or external data.
    pub placement: Placement,
    /// Wrap in a `standalone` document.
    pub standalone: bool,
}

/// A file the snippet reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FigureFile {
    /// File name (no directory).
    pub name: String,
    /// Contents.
    pub bytes: Vec<u8>,
}

/// The figure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FigureOutput {
    /// The snippet (or document).
    pub tex: String,
    /// Files it reads (empty for inline data).
    pub files: Vec<FigureFile>,
    /// Data rows written.
    pub points: usize,
    /// Map iterates or crossings were thinned to stay under [`MAX_POINTS`].
    pub thinned: bool,
    /// What the user should know (one sentence each).
    pub notes: Vec<String>,
}

/// A figure file stem: a slug of `name` without a `.tex` extension.
pub fn file_base(name: &str) -> String {
    let stem = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim_end_matches(".tex");
    slug(stem, "figure")
}

// ---------------------------------------------------------------------
// Screen geometry
// ---------------------------------------------------------------------

/// `axisLines`: where the axes are drawn (px), and whether 0 is in view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisLines {
    /// Horizontal pixel of the y axis (or the edge it sits on).
    pub x: f64,
    /// Vertical pixel of the x axis (or the edge it sits on).
    pub y: f64,
    /// 0 is inside the vertical range: the x axis is drawn.
    pub x_axis_in_view: bool,
    /// 0 is inside the horizontal range: the y axis is drawn.
    pub y_axis_in_view: bool,
}

/// `axisLines(vp)`, without the screen's half-pixel snapping.
pub fn axis_lines(vp: &Viewport) -> AxisLines {
    let (x0, x1) = vp.x_range;
    let (y0, y1) = vp.y_range;
    let y_axis_in_view = x0.min(x1) < 0.0 && x0.max(x1) > 0.0;
    let x_axis_in_view = y0.min(y1) < 0.0 && y0.max(y1) > 0.0;
    let x = if y_axis_in_view {
        vp.sx(0.0)
    } else if vp.sx(0.0) <= vp.left {
        vp.left
    } else {
        vp.right()
    };
    let y = if x_axis_in_view {
        vp.sy(0.0)
    } else if vp.sy(0.0) >= vp.bottom() {
        vp.bottom()
    } else {
        vp.top
    };
    AxisLines {
        x,
        y,
        x_axis_in_view,
        y_axis_in_view,
    }
}

/// Where the axis names go (`axisNamePlacement`, the rule PortraitCanvas
/// uses): the x name right-aligned 8 px in from the right edge, above the
/// x axis when that is more than 40 px below the top, otherwise below it;
/// the y name 8 px right of the y axis when there are more than 48 px of
/// room, otherwise left of it, 6 px below the top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisNames {
    /// Right edge of the x name (px).
    pub x_right: f64,
    /// The x name sits above the x axis (its bottom 4 px above it).
    pub x_above: bool,
    /// The x axis's pixel.
    pub x_axis_y: f64,
    /// The y name's anchor: its left edge when `y_right`, else its right edge.
    pub y_x: f64,
    /// The y name is right of the y axis.
    pub y_right: bool,
    /// Top of the y name (px).
    pub y_top: f64,
}

/// `axisNamePlacement` (without the screen's toolbar avoidance).
pub fn axis_names(vp: &Viewport, axes: &AxisLines) -> AxisNames {
    let right = vp.right();
    let y_right = right - axes.x > 48.0;
    AxisNames {
        x_right: right - 8.0,
        x_above: axes.y - vp.top > 40.0,
        x_axis_y: axes.y,
        y_x: if y_right { axes.x + 8.0 } else { axes.x - 8.0 },
        y_right,
        y_top: vp.top + 6.0,
    }
}

/// `withSubscript(latex, index)` (`app/src/math/format.ts`): `x_{n}`, and
/// `x_{1,n}` for `x_{1}`.
pub fn with_subscript(latex: &str, index: &str) -> String {
    fn last_group_start(s: &str) -> Option<usize> {
        if !s.ends_with('}') {
            return None;
        }
        let b = s.as_bytes();
        let mut depth = 0i32;
        for i in (0..b.len()).rev() {
            if b[i] == b'}' {
                depth += 1;
            } else if b[i] == b'{' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        None
    }
    if let Some(open) = last_group_start(latex) {
        if open > 0 && latex.as_bytes()[open - 1] == b'_' {
            return format!("{},{index}}}", &latex[..latex.len() - 1]);
        }
        if open > 0 && latex.as_bytes()[open - 1] == b'^' {
            let base = &latex[..open - 1];
            if let Some(sub) = last_group_start(base) {
                if sub > 0 && base.as_bytes()[sub - 1] == b'_' {
                    return format!(
                        "{},{index}}}{}",
                        &base[..base.len() - 1],
                        &latex[open - 1..]
                    );
                }
            }
        }
    }
    format!("{latex}_{{{index}}}")
}

/// The axis's name in maths: the state's LaTeX, `t` or `n`, and a map's
/// state against time with the subscript `n` (`axisLabelLatex`).
pub fn axis_latex(axis: Axis, other: Axis, kind: SystemKind, labels: &Labels) -> String {
    match axis {
        Axis::Time => if kind == SystemKind::Map { "n" } else { "t" }.to_string(),
        Axis::State { index } => {
            let base = labels
                .states
                .get(index)
                .map_or("?".to_string(), |s| s.latex.clone());
            if kind == SystemKind::Map && other == Axis::Time {
                with_subscript(&base, "n")
            } else {
                base
            }
        }
    }
}

/// The axis names of a cobweb plot: `x_{n}` across and `x_{n+1}` up, from
/// the state's own LaTeX.
pub fn cobweb_axis_latex(labels: &Labels) -> (String, String) {
    let base = labels
        .states
        .first()
        .map_or("?".to_string(), |s| s.latex.clone());
    (with_subscript(&base, "n"), with_subscript(&base, "n+1"))
}

type BoxPx = [f64; 4];

fn overlaps(a: &BoxPx, b: &BoxPx) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

/// Tick length (px).
const TICK_LEN: f64 = 4.0;
/// Space between a tick and its label (px).
const TICK_LABEL_PAD: f64 = 3.0;
/// Halo radius of a fixed point (px).
const FP_HALO_R: f64 = 7.5;
/// Height of a tick label (px): `\footnotesize` in a 10 pt document.
const LABEL_HEIGHT_PX: f64 = 8.0 * 96.0 / 72.27;

/// Width (px) of a tick label set in `\footnotesize` maths (Computer
/// Modern: digits 0.531 em, the minus sign 0.778 em, at 8 pt).
fn label_width_px(tex: &str) -> f64 {
    let pt_per_px = 72.27 / 96.0;
    let mut pt = 0.0;
    let plain = tex.replace("\\cdot", "\u{b7}");
    let mut sup = false;
    for c in plain.chars() {
        let w = match c {
            '0'..='9' => 4.25,
            '.' => 2.36,
            '-' => 6.22,
            '\u{b7}' => 2.36 + 3.0,
            '^' => {
                sup = true;
                0.0
            }
            '{' | '}' => 0.0,
            _ => 4.25,
        };
        pt += if sup && c != '^' { w * 0.75 } else { w };
    }
    pt / pt_per_px
}

/// A tick label: its text, anchor point (px) and box.
#[derive(Debug, Clone, PartialEq)]
struct TickLabel {
    tex: String,
    x: f64,
    y: f64,
    anchor: &'static str,
    bx: BoxPx,
}

fn make_label(tex: String, x: f64, y: f64, align: &str, base: &str) -> TickLabel {
    let wid = label_width_px(&tex);
    let hgt = LABEL_HEIGHT_PX;
    let l = match align {
        "center" => x - wid / 2.0,
        "right" => x - wid,
        _ => x,
    };
    let t = match base {
        "top" => y,
        "middle" => y - hgt / 2.0,
        _ => y - hgt,
    };
    let anchor = match (align, base) {
        ("center", "top") => "north",
        ("center", "bottom") => "south",
        ("right", "middle") => "east",
        ("left", "middle") => "west",
        ("right", "top") => "north east",
        ("left", "top") => "north west",
        ("right", "bottom") => "south east",
        ("left", "bottom") => "south west",
        _ => "center",
    };
    TickLabel {
        tex,
        x,
        y,
        anchor,
        bx: [l - 3.0, t - 3.0, l + wid + 3.0, t + hgt + 3.0],
    }
}

/// Height of an axis name (px): maths at 10 pt.
const NAME_HEIGHT_PX: f64 = 14.0;

/// Rough width (px) of an axis name in maths at 10 pt: a letter or a
/// command such as `\theta` is about 7 px, a subscript or superscript
/// 70% of that.
fn name_width_px(latex: &str) -> f64 {
    let b = latex.as_bytes();
    let mut w = 0.0;
    let mut i = 0;
    let mut script = 0usize;
    let mut depth = Vec::new();
    while i < b.len() {
        let c = b[i];
        let size = if script > 0 { 0.7 } else { 1.0 };
        match c {
            b'\\' => {
                let mut j = i + 1;
                while j < b.len() && b[j].is_ascii_alphabetic() {
                    j += 1;
                }
                let word = &latex[i + 1..j];
                if !matches!(
                    word,
                    "dot" | "ddot" | "mathrm" | "operatorname" | "left" | "right"
                ) {
                    w += 7.0 * size;
                }
                i = j.max(i + 2);
                continue;
            }
            b'_' | b'^' => {
                script += 1;
                if b.get(i + 1) == Some(&b'{') {
                    depth.push(true);
                    i += 2;
                } else {
                    i += 1;
                    if i < b.len() {
                        w += 7.0 * 0.7;
                        i += 1;
                    }
                    script -= 1;
                }
                continue;
            }
            b'{' => depth.push(false),
            b'}' => {
                if depth.pop() == Some(true) {
                    script = script.saturating_sub(1);
                }
            }
            b' ' => {}
            _ => w += 7.0 * size,
        }
        i += 1;
    }
    w
}

/// The boxes (px) the axis names take.
fn name_boxes(names: &AxisNames, x_name: &str, y_name: &str) -> Vec<BoxPx> {
    let (xw, yw) = (name_width_px(x_name), name_width_px(y_name));
    let h = NAME_HEIGHT_PX;
    let xt = if names.x_above {
        names.x_axis_y - 4.0 - h
    } else {
        names.x_axis_y + 4.0
    };
    let yl = if names.y_right {
        names.y_x
    } else {
        names.y_x - yw
    };
    vec![
        [names.x_right - xw, xt, names.x_right, xt + h],
        [yl, names.y_top, yl + yw, names.y_top + h],
    ]
}

/// `layoutLabels`: which tick labels are drawn and where.
fn layout_labels(
    vp: &Viewport,
    tx: &ticks::Ticks,
    ty: &ticks::Ticks,
    axes: &AxisLines,
    prefer_y_right: bool,
    prefer_x_above: bool,
    avoid: &[BoxPx],
) -> Vec<TickLabel> {
    let gap = (TICK_LEN + TICK_LABEL_PAD).max(FP_HALO_R + 2.0);
    let right = vp.right();
    let bottom = vp.bottom();
    let hgt = LABEL_HEIGHT_PX;
    let both = axes.x_axis_in_view && axes.y_axis_in_view;
    let mut out: Vec<TickLabel> = Vec::new();
    let keep = |lab: TickLabel, out: &mut Vec<TickLabel>| {
        let [l, t, r, b] = lab.bx;
        if l < vp.left || r > right || t < vp.top || b > bottom {
            return;
        }
        if avoid.iter().any(|a| overlaps(&lab.bx, a)) {
            return;
        }
        if out.iter().any(|o| overlaps(&lab.bx, &o.bx)) {
            return;
        }
        out.push(lab);
    };
    let x_below = if prefer_x_above {
        axes.y - gap - hgt - 3.0 < vp.top
    } else {
        axes.y + gap + hgt + 3.0 <= bottom
    };
    let xy = if x_below { axes.y + gap } else { axes.y - gap };
    for &v in &tx.ticks {
        if v == 0.0 && both {
            continue;
        }
        let tex = tex_tick_label(&ticks::label(v, tx.step));
        keep(
            make_label(
                tex,
                vp.sx(v),
                xy,
                "center",
                if x_below { "top" } else { "bottom" },
            ),
            &mut out,
        );
    }
    let y_widest = ty
        .ticks
        .iter()
        .map(|&v| label_width_px(&tex_tick_label(&ticks::label(v, ty.step))))
        .fold(0.0, f64::max);
    let y_left = if prefer_y_right {
        axes.x + gap + y_widest + 3.0 > right
    } else {
        axes.x - gap - y_widest - 3.0 >= vp.left
    };
    let yx = if y_left { axes.x - gap } else { axes.x + gap };
    for &v in &ty.ticks {
        if v == 0.0 && both {
            continue;
        }
        let tex = tex_tick_label(&ticks::label(v, ty.step));
        keep(
            make_label(
                tex,
                yx,
                vp.sy(v),
                if y_left { "right" } else { "left" },
                "middle",
            ),
            &mut out,
        );
    }
    if both {
        keep(
            make_label(
                "0".into(),
                if y_left { axes.x - gap } else { axes.x + gap },
                if x_below { axes.y + gap } else { axes.y - gap },
                if y_left { "right" } else { "left" },
                if x_below { "top" } else { "bottom" },
            ),
            &mut out,
        );
    }
    out
}

// ---------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------

/// A data table: column names and rows (`None` is a break).
#[derive(Debug, Clone, Default)]
struct Table {
    columns: Vec<&'static str>,
    rows: Vec<Option<Vec<f64>>>,
}

impl Table {
    fn new(columns: &[&'static str]) -> Table {
        Table {
            columns: columns.to_vec(),
            rows: Vec::new(),
        }
    }

    fn data_rows(&self) -> usize {
        self.rows.iter().filter(|r| r.is_some()).count()
    }

    /// Pieces separated by breaks.
    fn push_pieces(&mut self, pieces: &[Vec<(f64, f64)>]) {
        for p in pieces {
            if p.len() < 2 {
                continue;
            }
            if !self.rows.is_empty() {
                self.rows.push(None);
            }
            self.rows.extend(p.iter().map(|&(a, b)| Some(vec![a, b])));
        }
    }
}

/// Rounds values per column to a tenth of a figure pixel (at least three
/// significant digits of the axis span) and writes them shortest.
struct Quantizer {
    decimals: Vec<usize>,
}

impl Quantizer {
    fn decimals_for(units_per_px: f64, span: f64, lo: f64, hi: f64) -> usize {
        let q = units_per_px / 10.0;
        let from_px = if q > 0.0 && q.is_finite() {
            (-q.log10()).ceil()
        } else {
            6.0
        };
        let size = lo.abs().max(hi.abs()).max(span.abs());
        let from_sig = if size > 0.0 && size.is_finite() {
            2.0 - size.log10().floor()
        } else {
            0.0
        };
        from_px.max(from_sig).clamp(0.0, 100.0) as usize
    }

    fn write(&self, col: usize, v: f64) -> String {
        if !v.is_finite() {
            return "nan".into();
        }
        let d = self.decimals.get(col).copied().unwrap_or(6);
        let s = fixed_trim(v, d);
        match s.parse::<f64>() {
            Ok(r) => shortest(r),
            Err(_) => s,
        }
    }
}

fn table_text(t: &Table, q: &Quantizer, inline: bool) -> String {
    let end = if inline { "\\\\\n" } else { "\n" };
    let mut out = format!("{}{end}", t.columns.join(" "));
    for row in &t.rows {
        match row {
            Some(vals) => {
                let cells: Vec<String> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| q.write(i, v))
                    .collect();
                out.push_str(&cells.join(" "));
            }
            None => out.push_str(&vec!["nan"; t.columns.len()].join(" ")),
        }
        out.push_str(end);
    }
    out
}

// ---------------------------------------------------------------------
// The figure
// ---------------------------------------------------------------------

/// One `\addplot` (or a raw TikZ command when `table` is `None`).
#[derive(Debug, Clone)]
struct Plot {
    options: String,
    table: Option<(String, Table)>,
    /// Curves that may be simplified further, as pieces (data coords).
    curve: Option<Vec<Vec<(f64, f64)>>>,
    cloud: Option<Cloud>,
    raw: String,
}

#[derive(Debug, Clone)]
struct Cloud {
    points: Vec<(f64, f64)>,
    dot: Dot,
    color: String,
}

fn ax_value(axis: Axis, t: &[f64], x: &[f64], dim: usize, k: usize) -> f64 {
    axis.value(t, x, dim, k)
}

/// `(x, y)` pixel (y down) to a TikZ point: the plot's top-left corner plus
/// an offset in bp.
fn at_px(x: f64, y: f64) -> String {
    format!(
        "($(rel axis cs:0,1)+({}bp,{}bp)$)",
        fixed_trim(x * BP_PER_PX, 3),
        fixed_trim(-y * BP_PER_PX, 3)
    )
}

struct Frame {
    vp: Viewport,
    tx: ticks::Ticks,
    ty: ticks::Ticks,
    axes: AxisLines,
    names: AxisNames,
    xn: String,
    yn: String,
    name_boxes: Vec<BoxPx>,
    labels: Vec<TickLabel>,
}

impl Frame {
    fn new(
        vp: Viewport,
        xn: String,
        yn: String,
        prefer_y_right: bool,
        prefer_x_above: bool,
    ) -> Frame {
        let (tx, ty) = ticks::for_viewport(vp.width, vp.height, vp.x_range, vp.y_range);
        let axes = axis_lines(&vp);
        let names = axis_names(&vp, &axes);
        let name_boxes = name_boxes(&names, &xn, &yn);
        let labels = layout_labels(
            &vp,
            &tx,
            &ty,
            &axes,
            prefer_y_right,
            prefer_x_above,
            &name_boxes,
        );
        Frame {
            vp,
            tx,
            ty,
            axes,
            names,
            xn,
            yn,
            name_boxes,
            labels,
        }
    }

    fn axis_begin(&self, theme: Theme) -> String {
        let vp = &self.vp;
        let mut axis_opts: Vec<String> = vec![
            format!("width={}, height={}, scale only axis", bp(vp.width), bp(vp.height)),
            format!(
                "xmin={}, xmax={}, ymin={}, ymax={}",
                shortest(vp.x_range.0),
                shortest(vp.x_range.1),
                shortest(vp.y_range.0),
                shortest(vp.y_range.1)
            ),
            "enlargelimits=false, scaled ticks=false, clip marker paths=true, unbounded coords=jump, filter discard warning=false".into(),
        ];
        let list = |vs: &[f64]| {
            vs.iter()
                .map(|&v| shortest(v))
                .collect::<Vec<_>>()
                .join(",")
        };
        let xh = ticks::half_steps(vp.x_range.0, vp.x_range.1, self.tx.step);
        let yh = ticks::half_steps(vp.y_range.0, vp.y_range.1, self.ty.step);
        let x_minor: Vec<f64> = xh
            .iter()
            .copied()
            .filter(|v| !self.tx.ticks.contains(v))
            .collect();
        let y_minor: Vec<f64> = yh
            .iter()
            .copied()
            .filter(|v| !self.ty.ticks.contains(v))
            .collect();
        axis_opts.push(format!(
            "xtick={{{}}}, ytick={{{}}}, xticklabels={{}}, yticklabels={{}}",
            list(&self.tx.ticks),
            list(&self.ty.ticks)
        ));
        axis_opts.push(format!(
            "minor xtick={{{}}}, minor ytick={{{}}}",
            list(&x_minor),
            list(&y_minor)
        ));
        let grid = format!("sepxgrid, line width={}", bp(1.0));
        axis_opts.push(format!(
            "grid=both, every major grid/.style={{{grid}}}, every minor grid/.style={{{grid}}}, \
             major tick length=0pt, minor tick length=0pt"
        ));
        axis_opts.push(format!(
            "axis line style={{sepxaxis, line width={}}}",
            bp(1.0)
        ));
        let x_line = if self.axes.x_axis_in_view {
            "axis x line*=middle".to_string()
        } else {
            let side = if self.axes.y >= vp.bottom() {
                "bottom"
            } else {
                "top"
            };
            format!("axis x line*={side}, x axis line style={{draw=none}}")
        };
        let y_line = if self.axes.y_axis_in_view {
            "axis y line*=middle".to_string()
        } else {
            let side = if self.axes.x <= vp.left {
                "left"
            } else {
                "right"
            };
            format!("axis y line*={side}, y axis line style={{draw=none}}")
        };
        axis_opts.push(format!("{x_line}, {y_line}"));
        if theme == Theme::Dark {
            axis_opts.push("axis background/.style={fill=sepxbg}".into());
        }

        let mut body = String::new();
        // The halo: the label's box filled in the background color, 1.5 px
        // (1.125 bp) round the text; the negative outer sep keeps the anchors
        // on the text itself.
        body.push_str(
            "\\begin{tikzpicture}[sepxlabel/.style={inner sep=1.125bp, outer sep=-1.125bp, \
             fill=sepxhalo, fill opacity=0.8, text opacity=1, text=sepxtick}]\n\\begin{axis}[\n",
        );
        for (i, o) in axis_opts.iter().enumerate() {
            body.push_str("  ");
            body.push_str(o);
            body.push_str(if i + 1 < axis_opts.len() { ",\n" } else { "\n" });
        }
        body.push_str("]\n");

        let mut marks = Vec::new();
        if self.axes.y_axis_in_view {
            for &v in &self.ty.ticks {
                if v != 0.0 {
                    let y = vp.sy(v);
                    marks.push(format!(
                        "{} -- {}",
                        at_px(self.axes.x, y),
                        at_px(self.axes.x - TICK_LEN, y)
                    ));
                }
            }
        }
        if self.axes.x_axis_in_view {
            for &v in &self.tx.ticks {
                if v != 0.0 {
                    let x = vp.sx(v);
                    marks.push(format!(
                        "{} -- {}",
                        at_px(x, self.axes.y),
                        at_px(x, self.axes.y + TICK_LEN)
                    ));
                }
            }
        }
        if !marks.is_empty() {
            body.push_str(&format!(
                "\\draw[sepxaxis, line width={}, line cap=butt]\n  {};\n",
                bp(1.0),
                marks.join("\n  ")
            ));
        }
        body
    }

    fn axis_end(&self) -> String {
        let mut body = String::new();
        for l in &self.labels {
            body.push_str(&format!(
                "\\node[anchor={}, sepxlabel, font=\\footnotesize] at {} {{${}$}};\n",
                l.anchor,
                at_px(l.x, l.y),
                l.tex
            ));
        }
        let (xa, xy) = if self.names.x_above {
            ("south east", self.names.x_axis_y - 4.0)
        } else {
            ("north east", self.names.x_axis_y + 4.0)
        };
        body.push_str(&format!(
            "\\node[anchor={xa}, sepxlabel] at {} {{${}$}};\n",
            at_px(self.names.x_right, xy),
            dyn_expr::portable_latex(&self.xn)
        ));
        body.push_str(&format!(
            "\\node[anchor={}, sepxlabel] at {} {{${}$}};\n",
            if self.names.y_right {
                "north west"
            } else {
                "north east"
            },
            at_px(self.names.y_x, self.names.y_top),
            dyn_expr::portable_latex(&self.yn)
        ));
        body.push_str("\\end{axis}\n\\end{tikzpicture}\n");
        body
    }
}

/// The `% Needs:` comment and the preamble of a standalone figure that
/// uses the TikZ libraries `libs`. amsmath: axis names are state LaTeX
/// (`\\dddot{x}` for a fourth-order system), which only amsmath defines.
fn packages(libs: &[&str]) -> (String, String) {
    let libs = libs.join(",");
    (
        comment(&format!(
            "Needs: \\usepackage{{amsmath}} \\usepackage{{pgfplots}} \\pgfplotsset{{compat=1.18}} \\usetikzlibrary{{{libs}}}"
        )),
        format!(
            "\\usepackage{{amsmath}}\n\\usepackage{{pgfplots}}\n\\pgfplotsset{{compat=1.18}}\n\\usetikzlibrary{{{libs}}}\n"
        ),
    )
}

fn definitions(external: bool, colors: &[(String, String)]) -> String {
    let mut defs = String::new();
    if external {
        defs.push_str(&comment(
            "If you \\input this file from another folder, put \\def\\sepxdir{figures/} before it.",
        ));
        defs.push_str("\\providecommand{\\sepxdir}{}%\n");
    }
    for (name, hex) in colors {
        defs.push_str(&define_color(name, hex));
    }
    defs
}

fn finish(mut tex: String, preamble: &str, defs: &str, body: &str, standalone: bool) -> String {
    if standalone {
        tex.push_str("\\documentclass[border=2mm]{standalone}\n");
        tex.push_str(preamble);
        tex.push_str("\\begin{document}\n");
        tex.push_str(defs);
        tex.push_str(body);
        tex.push_str("\\end{document}\n");
    } else {
        tex.push_str(defs);
        tex.push_str(body);
    }
    tex
}

#[derive(Debug, Clone, Copy)]
struct Dot {
    radius_px: f64,
    square: bool,
}

impl Dot {
    fn mark(self) -> &'static str {
        if self.square {
            "square*"
        } else {
            "*"
        }
    }
}

fn dot_size(count: usize) -> Dot {
    let (radius_px, square) = if count > SQUARE_DOTS_ABOVE {
        (0.75, true)
    } else if count > 300 {
        (1.3, false)
    } else {
        (2.6, false)
    };
    Dot { radius_px, square }
}

/// What [`orbit_figure`] needs.
#[derive(Debug, Clone)]
pub struct OrbitFigureInput<'a> {
    /// The system's names, parameter values and source.
    pub sys: &'a SystemMeta<'a>,
    /// The diagram.
    pub run: &'a OrbitRun,
    /// The view's parameter range (across).
    pub x_range: (f64, f64),
    /// The view's state range (up).
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

/// An orbit diagram (Strogatz §10.2) as a pgfplots figure with the screen's
/// frame: the swept parameter across, the state up, every finite kept
/// iterate a dot in `plot-fp` (sized as map iterates are). Above
/// [`MAX_POINTS`] dots the iterates become a density PNG under vector axes,
/// `<base>-points.png` drawn with `\\addplot graphics` (Save), or one in
/// `k` is kept and the figure says it was thinned (Copy).
pub fn orbit_figure(input: &OrbitFigureInput) -> FigureOutput {
    let (sys, run, pal) = (input.sys, input.run, input.palette);
    let d = &run.diagram;
    let w = (input.width_cm * PX_PER_CM).max(10.0);
    let h = (input.height_cm * PX_PER_CM).max(10.0);
    let vp = Viewport::new(w, h, input.x_range, input.y_range);
    let (upx, upy) = vp.units_per_px();
    let param = sys.labels.params.get(run.param);
    let state = sys.labels.states.get(run.state);
    let frame = Frame::new(
        vp,
        param.map_or("r".to_string(), |p| p.latex.clone()),
        state.map_or("x".to_string(), |s| s.latex.clone()),
        false,
        false,
    );
    let quant = Quantizer {
        decimals: vec![
            Quantizer::decimals_for(upx, vp.x_range.1 - vp.x_range.0, vp.x_range.0, vp.x_range.1),
            Quantizer::decimals_for(upy, vp.y_range.1 - vp.y_range.0, vp.y_range.0, vp.y_range.1),
        ],
    };

    let all: Vec<(f64, f64)> =
        d.r.iter()
            .enumerate()
            .flat_map(|(j, &r)| {
                let col = d.x.get(j * d.kept..(j + 1) * d.kept).unwrap_or(&[]);
                col.iter().filter(|x| x.is_finite()).map(move |&x| (r, x))
            })
            .collect();
    let dot = dot_size(all.len());
    let radius = dot.radius_px;
    let grown = Rect {
        x0: vp.x_range.0 - radius * upx,
        x1: vp.x_range.1 + radius * upx,
        y0: vp.y_range.0 - radius * upy,
        y1: vp.y_range.1 + radius * upy,
    };
    let mut points: Vec<(f64, f64)> = all.into_iter().filter(|&q| grown.contains(q)).collect();
    let mut notes: Vec<String> = Vec::new();
    let mut thinned = false;
    let mut layer: Option<Vec<u8>> = None;
    let shown = points.len();
    if shown > MAX_POINTS {
        match &input.placement {
            Placement::External { .. } => {
                layer = Some(raster::density_png(
                    d,
                    vp.x_range,
                    vp.y_range,
                    (w, h),
                    &pal.fp,
                ));
                points.clear();
                notes.push(format!(
                    "The iterates ({shown} points) are drawn as a density PNG under vector axes."
                ));
            }
            Placement::Inline => {
                let stride = shown.div_ceil(MAX_POINTS).max(2);
                points = points.into_iter().step_by(stride).collect();
                thinned = true;
                notes.push(format!(
                    "One iterate in {stride} is kept so the copied figure stays under 5,000 \
                     points; Save… draws them all as an image."
                ));
            }
        }
    }

    let mut tex = header_comments(
        "orbit diagram",
        sys.title,
        sys.source_ref,
        sys.lines,
        sys.format,
        &sys.labels
            .params
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != run.param)
            .map(|(_, q)| (q.name.clone(), q.value))
            .collect::<Vec<_>>(),
    );
    let (needs, preamble) = packages(&["calc"]);
    tex.push_str(&needs);
    tex.push_str(&comment(&format!(
        "Orbit diagram: {} from {} to {} in {} values; {} after the first {} iterates, {} kept for each.",
        param.map_or("r", |p| p.name.as_str()),
        d.r.first().map_or("?".into(), |&v| shortest(v)),
        d.r.last().map_or("?".into(), |&v| shortest(v)),
        d.r.len(),
        state.map_or("x", |s| s.name.as_str()),
        run.transient,
        d.kept
    )));
    for n in &notes {
        tex.push_str(&comment(n));
    }
    if thinned {
        tex.push_str(&comment(
            "Thinned: some iterates were left out of this copy (see above).",
        ));
    }
    let external = match &input.placement {
        Placement::External { base } => Some(file_base(base)),
        Placement::Inline => None,
    };
    let colors: Vec<(String, String)> = vec![
        ("sepxbg".into(), pal.bg.clone()),
        ("sepxgrid".into(), pal.grid.clone()),
        ("sepxaxis".into(), pal.axis.clone()),
        ("sepxtick".into(), pal.tick.clone()),
        ("sepxfp".into(), pal.fp.clone()),
        ("sepxhalo".into(), pal.halo.clone()),
    ];
    let defs = definitions(external.is_some(), &colors);

    let mut body = frame.axis_begin(input.theme);
    let mut files: Vec<FigureFile> = Vec::new();
    if let (Some(png), Some(base)) = (layer, &external) {
        let file = format!("{base}-points.png");
        body.push_str(&format!(
            "\\addplot graphics[xmin={}, xmax={}, ymin={}, ymax={}] {{\\sepxdir {file}}};\n",
            shortest(vp.x_range.0),
            shortest(vp.x_range.1),
            shortest(vp.y_range.0),
            shortest(vp.y_range.1)
        ));
        files.push(FigureFile {
            name: file,
            bytes: png,
        });
    }
    if !points.is_empty() {
        let mut t = Table::new(&["x", "y"]);
        t.rows.extend(points.iter().map(|&(a, b)| Some(vec![a, b])));
        let data = table_text(&t, &quant, external.is_none());
        let source = match &external {
            None => format!("table[row sep=\\\\]{{\n{data}}}"),
            Some(base) => {
                let file = format!("{base}-orbit.dat");
                files.push(FigureFile {
                    name: file.clone(),
                    bytes: data.into_bytes(),
                });
                format!("table {{\\sepxdir {file}}}")
            }
        };
        body.push_str(&format!(
            "\\addplot[sepxfp, only marks, mark={}, mark size={}, mark options={{draw opacity=0, fill=sepxfp}}] {source};\n",
            dot.mark(),
            bp(radius)
        ));
    }
    body.push_str(&frame.axis_end());

    FigureOutput {
        tex: finish(tex, &preamble, &defs, &body, input.standalone),
        files,
        points: points.len(),
        thinned,
        notes,
    }
}

/// The figure.
pub fn figure(input: &FigureInput) -> FigureOutput {
    let p = input.portrait;
    let pal = input.palette;
    let w = (input.width_cm * PX_PER_CM).max(10.0);
    let h = (input.height_cm * PX_PER_CM).max(10.0);
    let vp = Viewport::new(w, h, p.x_range, p.y_range);
    let (upx, upy) = vp.units_per_px();
    let is_map = p.kind == SystemKind::Map;
    let section = p.section.filter(|_| !is_map);
    let dots = is_map || section.is_some();
    let return_map = section.is_some() && p.x_axis == p.y_axis && p.x_axis != Axis::Time;
    let points_word = if is_map { "map iterates" } else { "crossings" };
    let time_series = p.x_axis == Axis::Time || p.y_axis == Axis::Time;
    let mut notes: Vec<String> = Vec::new();

    let quant = Quantizer {
        decimals: vec![
            Quantizer::decimals_for(upx, p.x_range.1 - p.x_range.0, p.x_range.0, p.x_range.1),
            Quantizer::decimals_for(upy, p.y_range.1 - p.y_range.0, p.y_range.0, p.y_range.1),
            Quantizer::decimals_for(upx, p.x_range.1 - p.x_range.0, 0.0, 0.0),
            Quantizer::decimals_for(upy, p.y_range.1 - p.y_range.0, 0.0, 0.0),
        ],
    };
    let point_quant = Quantizer {
        decimals: vec![quant.decimals[0], quant.decimals[1], 0],
    };

    // The view grown by two field cells.
    let cells = if p.field_grid > 0 {
        p.field_grid as f64
    } else {
        20.0
    };
    let margin = 2.0 * w.min(h) / cells;
    let rect = Rect {
        x0: p.x_range.0 - margin * upx,
        x1: p.x_range.1 + margin * upx,
        y0: p.y_range.0 - margin * upy,
        y1: p.y_range.1 + margin * upy,
    };

    // Ticks, axes and labels.
    let cobweb = p.cobweb && is_map;
    let (xn, yn) = if cobweb {
        cobweb_axis_latex(input.labels)
    } else if return_map {
        let base = axis_latex(p.x_axis, p.y_axis, p.kind, input.labels);
        (with_subscript(&base, "k"), with_subscript(&base, "k+1"))
    } else {
        (
            axis_latex(p.x_axis, p.y_axis, p.kind, input.labels),
            axis_latex(p.y_axis, p.x_axis, p.kind, input.labels),
        )
    };
    let frame = Frame::new(vp, xn, yn, p.x_axis == Axis::Time, p.y_axis == Axis::Time);
    // The field clears the tick labels and, like the screen, the axis names
    // grown by 3 px.
    let label_boxes: Vec<BoxPx> = frame
        .labels
        .iter()
        .map(|l| l.bx)
        .chain(
            frame
                .name_boxes
                .iter()
                .map(|b| [b[0] - 3.0, b[1] - 3.0, b[2] + 3.0, b[3] + 3.0]),
        )
        .collect();

    // Colors used.
    let mut colors: Vec<(String, String)> = vec![
        ("sepxbg".into(), pal.bg.clone()),
        ("sepxgrid".into(), pal.grid.clone()),
        ("sepxaxis".into(), pal.axis.clone()),
        ("sepxtick".into(), pal.tick.clone()),
    ];
    let use_color = |name: String, hex: &str, colors: &mut Vec<(String, String)>| -> String {
        if !colors.iter().any(|(n, _)| *n == name) {
            colors.push((name.clone(), hex.to_string()));
        }
        name
    };

    let mut plots: Vec<Plot> = Vec::new();

    // Direction field.
    if !p.arrows.is_empty() && p.field_grid > 0 {
        let field = use_color("sepxfield".into(), &pal.field, &mut colors);
        let cell = w.min(h) / p.field_grid as f64;
        let len = 0.6 * cell;
        let head = 0.25 * len;
        let r = len / 2.0 + 1.0;
        let sxu = w / (p.x_range.1 - p.x_range.0);
        let syu = h / (p.y_range.1 - p.y_range.0);
        let mut t = Table::new(&["x", "y", "u", "v"]);
        for a in &p.arrows {
            let dx = a.u * sxu;
            let dy = -a.v * syu;
            let m = crate::geom::len(dx, dy);
            if !m.is_finite() || m < 1e-12 {
                continue;
            }
            let (cx, cy) = (vp.sx(a.x), vp.sy(a.y));
            let bx = [cx - r, cy - r, cx + r, cy + r];
            if label_boxes.iter().any(|c| overlaps(&bx, c)) {
                continue;
            }
            let (ux, uy) = (dx / m, dy / m);
            let du = ux * len * upx;
            let dv = -uy * len * upy;
            t.rows
                .push(Some(vec![a.x - du / 2.0, a.y - dv / 2.0, du, dv]));
        }
        if !t.rows.is_empty() {
            plots.push(Plot {
                options: format!(
                    "{field}, line width={}, line cap=round, line join=round, \
                     quiver={{u=\\thisrow{{u}}, v=\\thisrow{{v}}}}, \
                     -{{Straight Barb[angle'=51.4, length={}]}}",
                    bp(1.0),
                    bp(head)
                ),
                table: Some(("field".into(), t)),
                curve: None,
                cloud: None,
                raw: String::new(),
            });
        }
    }

    // Nullclines.
    for nc in &p.nullclines {
        let color = use_color(
            format!("sepxnull{}", nc.state % pal.nullcline.len().max(1) + 1),
            &pal.nullcline[nc.state % pal.nullcline.len().max(1)],
            &mut colors,
        );
        let mut pieces = Vec::new();
        for line in &nc.polylines {
            let line = &line[..finite_prefix(line)];
            pieces.extend(clip_polyline(&rect, line));
        }
        if pieces.is_empty() {
            continue;
        }
        let mut t = Table::new(&["x", "y"]);
        t.push_pieces(&pieces);
        plots.push(Plot {
            options: format!(
                "{color}, line width={}, dash pattern=on {} off {}, line cap=butt, line join=round",
                bp(1.5),
                bp(6.0),
                bp(5.0)
            ),
            table: Some((format!("nullcline-{}", nc.state + 1), t)),
            curve: Some(pieces),
            cloud: None,
            raw: String::new(),
        });
    }

    if cobweb || return_map {
        if let Some((a, b)) = crate::cobweb::diagonal(p.x_range, p.y_range) {
            let pt = |(x, y): (f64, f64)| format!("({},{})", quant.write(0, x), quant.write(1, y));
            let k = if cobweb { "n" } else { "k" };
            plots.push(Plot {
                options: String::new(),
                table: None,
                curve: None,
                cloud: None,
                raw: format!(
                    "% Diagonal x_{{{k}+1}} = x_{k}\n\\addplot[sepxaxis, line width={}, line cap=butt] coordinates {{{} {}}};\n",
                    bp(1.0),
                    pt(a),
                    pt(b)
                ),
            });
        }
    }
    if cobweb {
        let mut pieces = Vec::new();
        for line in &p.graph {
            let line = &line[..finite_prefix(line)];
            pieces.extend(clip_polyline(&rect, line));
        }
        if !pieces.is_empty() {
            let fpc = use_color("sepxfp".into(), &pal.fp, &mut colors);
            plots.push(Plot {
                options: String::new(),
                table: None,
                curve: None,
                cloud: None,
                raw: "% Graph of f\n".into(),
            });
            let mut t = Table::new(&["x", "y"]);
            t.push_pieces(&pieces);
            plots.push(Plot {
                options: format!(
                    "{fpc}, line width={}, line join=round, line cap=round",
                    bp(1.5)
                ),
                table: Some(("graph".into(), t)),
                curve: Some(pieces),
                cloud: None,
                raw: String::new(),
            });
        }
    }

    // Trajectories, backward first.
    let mut order: Vec<usize> = (0..p.trajectories.len()).collect();
    order.sort_by_key(|&i| usize::from(!p.trajectories[i].tag.backward));
    let mut placed: Vec<(f64, f64)> = Vec::new();
    let mut ic_names: Vec<String> = Vec::new();
    let mut half_marks = false;
    for &i in &order {
        let tr = &p.trajectories[i];
        let dim = p.dim;
        let n = tr.t.len().min(tr.x.len().checked_div(dim).unwrap_or(0));
        let next = usize::from(return_map);
        let shown: Vec<(f64, f64)> = (0..n.saturating_sub(next))
            .map(|k| {
                (
                    ax_value(p.x_axis, &tr.t, &tr.x, dim, k),
                    ax_value(p.y_axis, &tr.t, &tr.x, dim, k + next),
                )
            })
            .collect();
        let shown = &shown[..finite_prefix(&shown)];
        if shown.len() < 2 && !(dots && shown.len() == 1) {
            continue;
        }
        let ci = usize::from(tr.tag.color) % PALETTE_SIZE;
        let color = use_color(
            format!("sepxtraj{}", ci + 1),
            &pal.traj[ci % pal.traj.len()],
            &mut colors,
        );
        let mut stem = slug(&tr.tag.ic_id, "ic");
        let dir = if tr.tag.backward {
            "backward"
        } else {
            "forward"
        };
        while ic_names.contains(&format!("{stem}-{dir}")) {
            stem.push_str("-2");
        }
        ic_names.push(format!("{stem}-{dir}"));
        let name = format!("{stem}-{dir}");

        if cobweb {
            // The staircase of the orbit x_T … x_{T+N}, each segment drawn
            // once (keyed on the figure's rounding), clipped like every curve.
            let orbit: Vec<f64> = shown.iter().map(|&(a, _)| a).collect();
            let vertices = crate::cobweb::staircase(&orbit);
            let key = |(a, b): (f64, f64)| (quant.write(0, a), quant.write(1, b));
            let pieces: Vec<Vec<(f64, f64)>> = crate::cobweb::drop_retraced(&vertices, key)
                .iter()
                .flat_map(|piece| clip_polyline(&rect, piece))
                .collect();
            if !pieces.is_empty() {
                plots.push(Plot {
                    options: String::new(),
                    table: None,
                    curve: None,
                    cloud: None,
                    raw: comment(&format!("Cobweb of {}", tr.tag.ic_id)),
                });
                let mut t = Table::new(&["x", "y"]);
                t.push_pieces(&pieces);
                plots.push(Plot {
                    options: format!("{color}, line width={}, line join=miter", bp(1.0)),
                    table: Some((format!("{stem}-cobweb"), t)),
                    curve: Some(pieces),
                    cloud: None,
                    raw: String::new(),
                });
            }
            // After a transient the staircase starts at (x_T, x_T), away
            // from x_0: an open square on a halo marks it, as on screen.
            if p.transient > 0 {
                if let Some(&(a, b)) = vertices.first() {
                    let (x, y) = (vp.sx(a), vp.sy(b));
                    if x >= vp.left && x <= vp.right() && y >= vp.top && y <= vp.bottom() {
                        let halo = use_color("sepxhalo".into(), &pal.halo, &mut colors);
                        plots.push(Plot {
                            options: String::new(),
                            table: None,
                            curve: None,
                            cloud: None,
                            raw: start_mark(&color, &halo, x, y),
                        });
                    }
                }
            }
            if tr.status != crate::model::Status::Completed {
                if let Some(&(a, b)) = vertices.last() {
                    let (x, y) = (vp.sx(a), vp.sy(b));
                    if x >= vp.left && x <= vp.right() && y >= vp.top && y <= vp.bottom() {
                        plots.push(Plot {
                            options: String::new(),
                            table: None,
                            curve: None,
                            cloud: None,
                            raw: end_mark(&color, x, y),
                        });
                    }
                }
            }
            continue;
        }

        if dots {
            if time_series && shown.len() >= 2 {
                let pieces = clip_polyline(&rect, shown);
                if !pieces.is_empty() {
                    let mut t = Table::new(&["x", "y"]);
                    t.push_pieces(&pieces);
                    plots.push(Plot {
                        options: format!(
                            "{color}, opacity=0.4, line width={}, line join=round",
                            bp(1.0)
                        ),
                        table: Some((format!("{name}-line"), t)),
                        curve: Some(pieces),
                        cloud: None,
                        raw: String::new(),
                    });
                }
            }
            let dot = dot_size(shown.len());
            let r = dot.radius_px;
            let grown = Rect {
                x0: p.x_range.0 - r * upx,
                x1: p.x_range.1 + r * upx,
                y0: p.y_range.0 - r * upy,
                y1: p.y_range.1 + r * upy,
            };
            let pts: Vec<(f64, f64)> = shown
                .iter()
                .copied()
                .filter(|&q| grown.contains(q))
                .collect();
            if pts.is_empty() {
                continue;
            }
            if section.is_some() {
                let what = if return_map { "Return map" } else { "Section" };
                plots.push(Plot {
                    options: String::new(),
                    table: None,
                    curve: None,
                    cloud: None,
                    raw: comment(&format!("{what} of {}", tr.tag.ic_id)),
                });
            }
            let mut t = Table::new(&["x", "y"]);
            t.rows.extend(pts.iter().map(|&(a, b)| Some(vec![a, b])));
            plots.push(Plot {
                options: format!(
                    "{color}, only marks, mark={}, mark size={}, mark options={{draw opacity=0, fill={color}}}",
                    dot.mark(),
                    bp(r)
                ),
                table: Some((name, t)),
                curve: None,
                cloud: Some(Cloud {
                    points: pts,
                    dot,
                    color: pal.traj[ci % pal.traj.len()].clone(),
                }),
                raw: String::new(),
            });
            continue;
        }

        // Flow: the curve, its heads and an end mark for an incomplete run.
        let px: Vec<(f64, f64)> = shown
            .iter()
            .map(|&(a, b)| (clamp_px(vp.sx(a)), clamp_px(vp.sy(b))))
            .collect();
        let hs: Vec<Head> = heads::place(
            &px,
            &vp,
            tr.tag.backward,
            HEAD_LENGTH_PX,
            heads::max_heads(p.dim),
            &mut placed,
            MIN_GAP_PX,
        );
        let opacity = if tr.tag.backward { ", opacity=0.5" } else { "" };
        let pieces = clip_polyline(&rect, shown);
        if !pieces.is_empty() {
            let mut t = Table::new(&["x", "y"]);
            t.push_pieces(&pieces);
            plots.push(Plot {
                options: format!(
                    "{color}{opacity}, line width={}, line cap=round, line join=round",
                    bp(1.5)
                ),
                table: Some((name, t)),
                curve: Some(pieces),
                cloud: None,
                raw: String::new(),
            });
        }
        let mut raw = String::new();
        for hd in &hs {
            let tri = hd.triangle(HEAD_LENGTH_PX);
            let corners: Vec<String> = tri.iter().map(|&(x, y)| at_px(x, y)).collect();
            raw.push_str(&format!(
                "\\fill[{color}{opacity}] {} -- cycle;\n",
                corners.join(" -- ")
            ));
        }
        if tr.status != crate::model::Status::Completed {
            let (x, y) = px[px.len() - 1];
            if x >= vp.left && x <= vp.right() && y >= vp.top && y <= vp.bottom() {
                raw.push_str(&end_mark(&color, x, y));
            }
        }
        if !raw.is_empty() {
            plots.push(Plot {
                options: String::new(),
                table: None,
                curve: None,
                cloud: None,
                raw,
            });
        }
    }

    // Fixed points.
    if !p.fixed_points.is_empty() {
        let mut t = Table::new(&["x", "y", "glyph"]);
        for fp in &p.fixed_points {
            let px = match p.x_axis {
                Axis::Time => vp.left + 12.0,
                Axis::State { index } => vp.sx(fp.x.get(index).copied().unwrap_or(f64::NAN)),
            };
            let py = match p.y_axis {
                Axis::Time => vp.bottom() - 12.0,
                Axis::State { index } => vp.sy(fp.x.get(index).copied().unwrap_or(f64::NAN)),
            };
            if !px.is_finite() || !py.is_finite() {
                continue;
            }
            let inside = px >= vp.left - FP_HALO_R
                && px <= vp.right() + FP_HALO_R
                && py >= vp.top - FP_HALO_R
                && py <= vp.bottom() + FP_HALO_R;
            if !inside {
                continue;
            }
            use crate::model::Classification::*;
            let glyph = match fp.classification {
                Centre | NonHyperbolic | NonIsolated => 2.0,
                _ if fp.stable => 0.0,
                _ => 1.0,
            };
            half_marks |= glyph == 2.0;
            t.rows.push(Some(vec![
                p.x_range.0 + px * upx,
                p.y_range.1 - py * upy,
                glyph,
            ]));
        }
        if !t.rows.is_empty() {
            let fpc = use_color("sepxfp".into(), &pal.fp, &mut colors);
            let halo = use_color("sepxhalo".into(), &pal.halo, &mut colors);
            let glyphs = [
                (0.0, format!("{fpc}, mark=*, mark options={{fill={fpc}, draw={fpc}}}")),
                (1.0, format!("{fpc}, mark=*, mark options={{fill=sepxbg, draw={fpc}}}")),
                (
                    2.0,
                    format!("{fpc}, mark=halfcircle*, mark options={{rotate=90, fill={fpc}, draw={fpc}}}"),
                ),
            ];
            let mut opts = vec![format!(
                "{halo}, only marks, mark=*, mark size={}, mark options={{draw opacity=0, fill={halo}}}",
                bp(FP_HALO_R)
            )];
            for (g, style) in &glyphs {
                if t.rows.iter().flatten().any(|r| r[2] == *g) {
                    opts.push(format!(
                        "only marks, {style}, mark size={}, line width={}, \
                         restrict expr to domain={{\\thisrow{{glyph}}}}{{{g}:{g}}}",
                        bp(5.0),
                        bp(1.5)
                    ));
                }
            }
            plots.push(Plot {
                options: opts.join("\n"),
                table: Some(("fixed-points".into(), t)),
                curve: None,
                cloud: None,
                raw: String::new(),
            });
        }
    }

    // The 5,000-point rule.
    let count = |plots: &[Plot]| -> usize {
        plots
            .iter()
            .filter_map(|p| p.table.as_ref())
            .map(|(_, t)| t.data_rows())
            .sum()
    };
    let mut total = count(&plots);
    if total > MAX_POINTS {
        for tol in EXTRA_TOLERANCES {
            for pl in plots.iter_mut() {
                let Some(pieces) = &pl.curve else { continue };
                let simplified: Vec<Vec<(f64, f64)>> = pieces
                    .iter()
                    .map(|piece| {
                        let px: Vec<(f64, f64)> =
                            piece.iter().map(|&(a, b)| (vp.sx(a), vp.sy(b))).collect();
                        rdp_indices(&px, tol)
                            .into_iter()
                            .map(|k| piece[k])
                            .collect()
                    })
                    .collect();
                if let Some((_, t)) = &mut pl.table {
                    t.rows.clear();
                    t.push_pieces(&simplified);
                }
                pl.curve = Some(simplified);
            }
            total = count(&plots);
            if total <= MAX_POINTS {
                notes.push(format!(
                    "Curves were simplified to within {} px so the figure stays under 5,000 points.",
                    shortest(EXTRA_TOLERANCES.iter().take_while(|&&t| t <= tol).sum::<f64>() + 0.25)
                ));
                break;
            }
        }
    }
    let mut thinned = false;
    let mut raster_layer: Option<Vec<u8>> = None;
    if total > MAX_POINTS && plots.iter().any(|p| p.cloud.is_some()) {
        let cloud_rows: usize = plots
            .iter()
            .filter(|p| p.cloud.is_some())
            .filter_map(|p| p.table.as_ref())
            .map(|(_, t)| t.data_rows())
            .sum();
        match &input.placement {
            Placement::External { base } => {
                let clouds: Vec<raster::Dots> = plots
                    .iter()
                    .filter_map(|p| p.cloud.as_ref())
                    .map(|c| raster::Dots {
                        points: c
                            .points
                            .iter()
                            .map(|&(a, b)| (vp.sx(a), vp.sy(b)))
                            .collect(),
                        radius_px: c.dot.radius_px,
                        square: c.dot.square,
                        color: c.color.clone(),
                    })
                    .collect();
                raster_layer = Some(raster::points_png(w, h, RASTER_DPI, &clouds));
                let file = format!("{}-points.png", file_base(base));
                let graphics = format!(
                    "\\addplot graphics[xmin={}, xmax={}, ymin={}, ymax={}] {{\\sepxdir {file}}};\n",
                    shortest(p.x_range.0),
                    shortest(p.x_range.1),
                    shortest(p.y_range.0),
                    shortest(p.y_range.1)
                );
                // The layer takes the place of the first cloud in the z-order.
                let first = plots.iter().position(|p| p.cloud.is_some()).unwrap_or(0);
                plots[first] = Plot {
                    options: String::new(),
                    table: None,
                    curve: None,
                    cloud: None,
                    raw: graphics,
                };
                plots.retain(|p| p.cloud.is_none());
                notes.push(format!(
                    "The {points_word} ({cloud_rows} points) are drawn as a {RASTER_DPI}-dpi PNG \
                     layer under vector axes."
                ));
            }
            Placement::Inline => {
                let others = total - cloud_rows;
                let budget = MAX_POINTS.saturating_sub(others).max(1);
                let stride = cloud_rows.div_ceil(budget).max(2);
                for pl in plots.iter_mut().filter(|p| p.cloud.is_some()) {
                    if let Some((_, t)) = &mut pl.table {
                        let kept: Vec<Option<Vec<f64>>> =
                            t.rows.iter().step_by(stride).cloned().collect();
                        t.rows = kept;
                    }
                }
                thinned = true;
                notes.push(format!(
                    "One {} in {stride} is kept so the copied figure stays under \
                     5,000 points; Save… keeps them all.",
                    if is_map { "map iterate" } else { "crossing" }
                ));
            }
        }
        total = count(&plots);
    }
    let over = total > MAX_POINTS;
    if over {
        notes.push(format!(
            "This figure has {total} points, more than pgfplots handles comfortably; compile \
             it with lualatex."
        ));
    }

    // ---- Write ----
    let mut tex = String::new();
    let params: Vec<(String, f64)> = input
        .labels
        .params
        .iter()
        .map(|q| (q.name.clone(), q.value))
        .collect();
    tex.push_str(&header_comments(
        "figure",
        input.title,
        input.source_ref,
        input.lines,
        input.format,
        &params,
    ));
    let mut libs = vec!["arrows.meta", "calc"];
    if half_marks {
        libs.push("plotmarks");
    }
    let (needs, preamble) = packages(&libs);
    tex.push_str(&needs);
    if is_map && p.transient > 0 {
        tex.push_str(&comment(&format!(
            "Transient: the first {} iterates are not drawn.",
            p.transient
        )));
    }
    if let Some(s) = section {
        tex.push_str(&comment(&format!("Section: {}.", s.describe(input.labels))));
        if p.transient > 0 {
            tex.push_str(&comment(&format!(
                "Skipped crossings: the first {} of each orbit are not drawn.",
                p.transient
            )));
        }
    }
    for n in &notes {
        tex.push_str(&comment(n));
    }
    if thinned {
        tex.push_str(&comment(&format!(
            "Thinned: some {points_word} were left out of this copy (see above)."
        )));
    }
    if over {
        tex.push_str(&comment(
            "Large figure: compile with lualatex if pdflatex runs out of memory.",
        ));
    }
    let external = match &input.placement {
        Placement::External { base } => Some(file_base(base)),
        Placement::Inline => None,
    };
    // Tick labels and axis names sit on a halo, as on screen.
    use_color("sepxhalo".into(), &pal.halo, &mut colors);
    let defs = definitions(external.is_some(), &colors);

    let mut body = frame.axis_begin(input.theme);

    let mut files: Vec<FigureFile> = Vec::new();
    let inline = external.is_none();
    for pl in &plots {
        if let Some((name, t)) = &pl.table {
            let q = if t.columns.len() == 3 {
                &point_quant
            } else {
                &quant
            };
            let data = table_text(t, q, inline);
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
            for opt in pl.options.split('\n') {
                body.push_str(&format!("\\addplot[{opt}] {source};\n"));
            }
        }
        body.push_str(&pl.raw);
    }
    if let (Some(png), Some(base)) = (&raster_layer, &external) {
        files.push(FigureFile {
            name: format!("{base}-points.png"),
            bytes: png.clone(),
        });
    }

    body.push_str(&frame.axis_end());

    FigureOutput {
        tex: finish(tex, &preamble, &defs, &body, input.standalone),
        files,
        points: total,
        thinned,
        notes,
    }
}

/// The `x` at the end of a run that did not complete, at pixel `(x, y)`.
/// The open square at the start of a staircase drawn after a transient:
/// half-side 3 px, a 1.5 px stroke on a 3 px halo (draw.ts `drawCobweb`).
fn start_mark(color: &str, halo: &str, x: f64, y: f64) -> String {
    let h = 3.0;
    let square = format!("{} rectangle {}", at_px(x - h, y - h), at_px(x + h, y + h));
    format!(
        "\\draw[{halo}, line width={}] {square};\n\\draw[{color}, line width={}] {square};\n",
        bp(3.0),
        bp(1.5)
    )
}

fn end_mark(color: &str, x: f64, y: f64) -> String {
    let c = 4.0;
    format!(
        "\\draw[{color}, line width={}] {} -- {} {} -- {};\n",
        bp(1.5),
        at_px(x - c, y - c),
        at_px(x + c, y + c),
        at_px(x + c, y - c),
        at_px(x - c, y + c)
    )
}

const CLAMP: f64 = 1e5;

fn clamp_px(v: f64) -> f64 {
    v.clamp(-CLAMP, CLAMP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscripts_for_map_axes() {
        assert_eq!(with_subscript("x", "n"), "x_{n}");
        assert_eq!(with_subscript("x_{1}", "n"), "x_{1,n}");
        assert_eq!(with_subscript("x_{1}^{(4)}", "0"), "x_{1,0}^{(4)}");
        assert_eq!(with_subscript("\\theta", "n"), "\\theta_{n}");
    }

    #[test]
    fn axis_lines_and_names_follow_the_screen() {
        let vp = Viewport::new(400.0, 300.0, (-1.0, 3.0), (-1.0, 2.0));
        let a = axis_lines(&vp);
        assert!(a.x_axis_in_view && a.y_axis_in_view);
        assert!((a.x - 100.0).abs() < 1e-9);
        assert!((a.y - 200.0).abs() < 1e-9);
        let n = axis_names(&vp, &a);
        assert!(n.x_above && n.y_right);
        assert_eq!(n.x_right, 392.0);
        let vp = Viewport::new(400.0, 300.0, (1.0, 3.0), (0.5, 2.0));
        let a = axis_lines(&vp);
        assert!(!a.x_axis_in_view && !a.y_axis_in_view);
        assert_eq!((a.x, a.y), (0.0, 300.0));
        let vp = Viewport::new(400.0, 300.0, (-3.0, -1.0), (1.0, 2.0));
        let a = axis_lines(&vp);
        assert_eq!((a.x, a.y), (400.0, 300.0));
        assert!(!axis_names(&vp, &a).y_right);
    }

    #[test]
    fn axis_name_widths_and_label_layout() {
        assert_eq!(name_width_px("x"), 7.0);
        assert_eq!(name_width_px("\\theta"), 7.0);
        assert_eq!(name_width_px("\\dot{x}"), 7.0);
        assert!((name_width_px("x_{n}") - 11.9).abs() < 1e-9);
        assert!((name_width_px("x_{1,n}") - 21.7).abs() < 1e-9);
        // A label that would overlap an axis name is left out.
        let vp = Viewport::new(400.0, 300.0, (-1.0, 3.0), (-1.0, 2.0));
        let a = axis_lines(&vp);
        let (tx, ty) = ticks::for_viewport(400.0, 300.0, vp.x_range, vp.y_range);
        let all = layout_labels(&vp, &tx, &ty, &a, false, false, &[]);
        assert!(all.iter().any(|l| l.tex == "0"));
        let avoid = [all[0].bx];
        let fewer = layout_labels(&vp, &tx, &ty, &a, false, false, &avoid);
        assert_eq!(fewer.len(), all.len() - 1);
        // Labels never use the real minus sign.
        assert!(all.iter().all(|l| !l.tex.contains('\u{2212}')));
    }

    #[test]
    fn file_base_is_a_slug() {
        assert_eq!(
            file_base("/home/me/Rabbits vs sheep.tex"),
            "rabbits-vs-sheep"
        );
        assert_eq!(file_base("C:\\figs\\Lorenz.tex"), "lorenz");
        assert_eq!(file_base(""), "figure");
    }
}
