//! What the export commands do, testable without a window: the export
//! portrait (computed at the figure's size, cut at the first non-finite
//! point and simplified), its conversion into `separatrix_export`'s model,
//! and the LaTeX and CSV builders. File dialogs and writes are in `lib.rs`.
//!
//! `service.rs` is not changed: this module calls its `pub` functions.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dyn_core::{SystemKind, VectorField};
use dyn_expr::CompiledSystem;
use separatrix_export::latex_util::{slug, PX_PER_CM};
use separatrix_export::simplify::{
    simplify_polyline, simplify_run, EXPORT_TOLERANCE_PX, GRAPH_TOLERANCE_PX,
};
use separatrix_export::{
    latex_figure, latex_fixed_points, latex_system, Labels, Palette, Placement, Viewport,
};

use crate::ipc::{
    AxisRef, CsvFileOut, CsvRequest, CsvResponse, ExportPortraitRequest, ExportTheme,
    FigureLatexRequest, FixedPointsLatexRequest, Notation, PortraitRequest, PortraitResponse,
    SectionSpec, SystemLatexRequest, SystemLatexResponse, TrajectoryTag,
};
use crate::service;

/// The design system's tokens (read only), for the figure palettes.
const TOKENS_JSON: &str = include_str!("../../src/styles/ds/tokens.json");

/// The app's version, stamped into project files and CSV metadata.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A figure size in CSS px from cm (96 px per inch), as `latex_figure` uses.
pub fn figure_px(cm: f64) -> f64 {
    (cm * PX_PER_CM).max(10.0)
}

/// The last export portrait, so the dialog's preview and its LaTeX for the
/// same options share one computation.
#[derive(Default)]
pub struct ExportCache {
    last: Mutex<Option<(CacheKey, Arc<PortraitResponse>)>>,
}

#[derive(Clone, PartialEq)]
struct CacheKey {
    portrait: PortraitRequest,
    width_px: f64,
    height_px: f64,
}

impl ExportCache {
    /// An empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, key: &CacheKey) -> Option<Arc<PortraitResponse>> {
        let last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        last.as_ref()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    fn put(&self, key: CacheKey, value: Arc<PortraitResponse>) {
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, value));
    }
}

fn to_axis(a: AxisRef) -> separatrix_export::Axis {
    match a {
        AxisRef::State { index } => separatrix_export::Axis::State { index },
        AxisRef::Time => separatrix_export::Axis::Time,
    }
}

pub const SPACE_LATEX_REFUSAL: &str =
    "LaTeX figures of the 3D view aren't available yet. Export PNG or SVG, or switch to a phase plane.";

/// `compute_export_portrait`: [`service::compute_portrait`], then every
/// curve cut at its first non-finite point and simplified with RDP at
/// [`EXPORT_TOLERANCE_PX`] in figure pixels (map iterates, a section's
/// crossings and a 3D view's trajectories are only cut; the graph of a
/// cobweb's `f` is simplified like the nullclines).
/// The last result is cached by request (its `requestId` aside).
pub fn export_portrait(
    cache: &ExportCache,
    system: &CompiledSystem,
    req: &ExportPortraitRequest,
) -> Result<Arc<PortraitResponse>, String> {
    if !(req.width_px.is_finite() && req.height_px.is_finite())
        || req.width_px <= 0.0
        || req.height_px <= 0.0
    {
        return Err(format!(
            "The figure size {} × {} px is not valid. Set a width and a height above 0, then export again.",
            req.width_px, req.height_px
        ));
    }
    let mut key = CacheKey {
        portrait: req.portrait.clone(),
        width_px: req.width_px,
        height_px: req.height_px,
    };
    key.portrait.request_id = 0;
    if let Some(hit) = cache.get(&key) {
        let mut out = (*hit).clone();
        out.request_id = req.portrait.request_id;
        return Ok(Arc::new(out));
    }
    let mut out = service::compute_portrait(system, &req.portrait)?;
    let f: &dyn VectorField = system;
    let vp = Viewport::new(
        req.width_px,
        req.height_px,
        req.portrait.x_range,
        req.portrait.y_range,
    );
    for nc in &mut out.nullclines {
        for line in &mut nc.polylines {
            *line = simplify_polyline(&vp, line, EXPORT_TOLERANCE_PX);
        }
    }
    for line in out.graph.iter_mut().flatten() {
        *line = simplify_polyline(&vp, line, GRAPH_TOLERANCE_PX);
    }
    let kind = if req.portrait.section.is_some() || req.portrait.z_axis.is_some() {
        SystemKind::Map
    } else {
        f.kind()
    };
    for tr in &mut out.trajectories {
        let (t, x) = simplify_run(
            &vp,
            kind,
            to_axis(req.portrait.x_axis),
            to_axis(req.portrait.y_axis),
            f.dim(),
            &tr.t,
            &tr.x,
            EXPORT_TOLERANCE_PX,
        );
        tr.t = t;
        tr.x = x;
    }
    let out = Arc::new(out);
    cache.put(key, out.clone());
    Ok(out)
}

/// The export crate's portrait from a response and its request.
pub fn to_export_portrait(
    system: &CompiledSystem,
    req: &PortraitRequest,
    res: &PortraitResponse,
    tags: &[TrajectoryTag],
) -> Result<separatrix_export::Portrait, String> {
    if tags.len() != req.trajectories.len() || res.trajectories.len() != req.trajectories.len() {
        return Err(format!(
            "Expected one tag per trajectory ({}) but got {}.",
            req.trajectories.len(),
            tags.len()
        ));
    }
    let f: &dyn VectorField = system;
    let is_map = f.kind() == SystemKind::Map;
    Ok(separatrix_export::Portrait {
        kind: f.kind(),
        dim: f.dim(),
        x_axis: to_axis(req.x_axis),
        y_axis: to_axis(req.y_axis),
        x_range: req.x_range,
        y_range: req.y_range,
        field_grid: req.field_grid,
        arrows: res
            .arrows
            .iter()
            .map(|a| separatrix_export::Arrow {
                x: a.x,
                y: a.y,
                u: a.u,
                v: a.v,
            })
            .collect(),
        nullclines: res
            .nullclines
            .iter()
            .map(|n| separatrix_export::Nullcline {
                state: n.state,
                polylines: n.polylines.clone(),
            })
            .collect(),
        fixed_points: res
            .fixed_points
            .iter()
            .map(|fp| separatrix_export::FixedPoint {
                x: fp.x.clone(),
                eigenvalues: fp.eigenvalues.clone(),
                classification: fp.classification,
                stable: fp.stable,
            })
            .collect(),
        trajectories: res
            .trajectories
            .iter()
            .zip(&req.trajectories)
            .zip(tags)
            .map(|((out, tr), tag)| separatrix_export::Trajectory {
                tag: separatrix_export::TrajectoryTag {
                    ic_id: tag.ic_id.clone(),
                    backward: tag.backward,
                    color: tag.color,
                },
                x0: tr.x0.clone(),
                t0: if is_map { 0.0 } else { tr.t0 },
                t: out.t.clone(),
                x: out.x.clone(),
                status: out.status,
            })
            .collect(),
        cobweb: req.graph_samples.is_some(),
        graph: res.graph.clone().unwrap_or_default(),
        transient: if is_map {
            req.transient
                .unwrap_or(0)
                .min(service::MAX_ITERATES.saturating_sub(max_iterates(req)))
        } else if req.section.is_some() {
            req.transient.unwrap_or(0)
        } else {
            0
        },
        section: req.section.filter(|_| !is_map).map(to_section),
    })
}

fn to_section(s: SectionSpec) -> separatrix_export::SectionSpec {
    match s {
        SectionSpec::Plane {
            state,
            value,
            direction,
        } => separatrix_export::SectionSpec::Plane {
            state,
            value,
            direction,
        },
        SectionSpec::Stroboscopic { period, phase } => {
            separatrix_export::SectionSpec::Stroboscopic { period, phase }
        }
    }
}

/// The largest iterate count requested, as `service::trajectory` rounds it.
fn max_iterates(req: &PortraitRequest) -> usize {
    req.trajectories
        .iter()
        .map(|tr| {
            if tr.t1.is_finite() {
                tr.t1.round().clamp(0.0, service::MAX_ITERATES as f64) as usize
            } else {
                0
            }
        })
        .max()
        .unwrap_or(0)
}

/// The plot colours of `theme`.
pub fn palette(theme: ExportTheme) -> Result<Palette, String> {
    let theme = match theme {
        ExportTheme::Light => separatrix_export::Theme::Light,
        ExportTheme::Dark => separatrix_export::Theme::Dark,
    };
    Palette::from_tokens(TOKENS_JSON, theme)
}

/// The pgfplots figure: inline data for Copy, or files named after `base`
/// for Save.
pub fn figure_latex(
    cache: &ExportCache,
    system: &CompiledSystem,
    req: &FigureLatexRequest,
    placement: Placement,
) -> Result<separatrix_export::FigureOutput, String> {
    if !(req.width_cm.is_finite() && req.height_cm.is_finite())
        || req.width_cm <= 0.0
        || req.height_cm <= 0.0
    {
        return Err(format!(
            "The figure size {} × {} cm is not valid. Set a width and a height above 0, then export again.",
            req.width_cm, req.height_cm
        ));
    }
    if req.portrait.z_axis.is_some() {
        return Err(SPACE_LATEX_REFUSAL.to_string());
    }
    let res = export_portrait(
        cache,
        system,
        &ExportPortraitRequest {
            portrait: req.portrait.clone(),
            width_px: figure_px(req.width_cm),
            height_px: figure_px(req.height_cm),
        },
    )?;
    let portrait = to_export_portrait(system, &req.portrait, &res, &req.tags)?;
    let labels = Labels::from_info(&system.info, &req.portrait.params);
    let palette = palette(req.theme)?;
    Ok(latex_figure::figure(&separatrix_export::FigureInput {
        portrait: &portrait,
        labels: &labels,
        width_cm: req.width_cm,
        height_cm: req.height_cm,
        theme: match req.theme {
            ExportTheme::Light => separatrix_export::Theme::Light,
            ExportTheme::Dark => separatrix_export::Theme::Dark,
        },
        palette: &palette,
        title: &req.title,
        source_ref: req.source_ref.as_deref(),
        lines: &req.lines,
        format: req.format,
        placement,
        standalone: req.standalone,
    }))
}

/// The system as an `align` environment.
pub fn system_latex(system: &CompiledSystem, req: &SystemLatexRequest) -> SystemLatexResponse {
    let out = latex_system::system(&separatrix_export::SystemInput {
        lines: &req.lines,
        format: req.format,
        info: &system.info,
        params: &req.params,
        notation: match req.notation {
            Notation::Dot => separatrix_export::Notation::Dot,
            Notation::Prime => separatrix_export::Notation::Prime,
            Notation::Leibniz => separatrix_export::Notation::Leibniz,
        },
        numbered: req.numbered,
        title: &req.title,
        source_ref: req.source_ref.as_deref(),
    });
    SystemLatexResponse {
        tex: out.tex,
        preview_latex: out.preview_latex,
    }
}

/// The fixed-points table, from the screen request's fixed points.
pub fn fixed_points_latex(
    system: &CompiledSystem,
    req: &FixedPointsLatexRequest,
) -> Result<String, String> {
    // Only the fixed points are needed: skip the field, nullclines, runs and
    // a cobweb's graph.
    let mut screen = req.portrait.clone();
    screen.fixed_points = true;
    screen.field_grid = 0;
    screen.nullclines = false;
    screen.trajectories.clear();
    screen.graph_samples = None;
    let res = service::compute_portrait(system, &screen)?;
    let f: &dyn VectorField = system;
    let portrait = to_export_portrait(system, &screen, &res, &[])?;
    let labels = Labels::from_info(&system.info, &screen.params);
    let search = service::fixed_point_search_box(&screen, f.dim());
    Ok(latex_fixed_points::table(
        &separatrix_export::FixedPointsInput {
            kind: f.kind(),
            labels: &labels,
            fixed_points: &portrait.fixed_points,
            search: &search,
            lines: &req.lines,
            format: req.format,
            title: &req.title,
            source_ref: req.source_ref.as_deref(),
            digits: req.digits,
        },
    ))
}

/// One CSV and `.meta.json` per initial condition, from the full (not
/// simplified) trajectories at the request's own `samples`.
pub fn csv(system: &CompiledSystem, req: &CsvRequest) -> Result<CsvResponse, String> {
    let mut runs = req.portrait.clone();
    runs.fixed_points = false;
    runs.field_grid = 0;
    runs.nullclines = false;
    runs.graph_samples = None;
    let res = service::compute_portrait(system, &runs)?;
    let portrait = to_export_portrait(system, &runs, &res, &req.tags)?;
    let labels = Labels::from_info(&system.info, &runs.params);
    let files = separatrix_export::csv::files(&separatrix_export::csv::CsvInput {
        kind: portrait.kind,
        labels: &labels,
        trajectories: &portrait.trajectories,
        ic_ids: &req.ic_ids,
        lines: &req.lines,
        format: req.format,
        title: &req.title,
        app_version: APP_VERSION,
        transient: portrait.transient,
        section: portrait.section,
    });
    Ok(CsvResponse {
        files: files
            .into_iter()
            .map(|f| CsvFileOut {
                stem: f.stem,
                csv: f.csv,
                meta: f.meta,
            })
            .collect(),
    })
}

/// `path` with extension `ext` (no dot) added when it has another one or
/// none (`notes.txt` → `notes.txt.tex`); the flag says it was added.
pub fn with_extension(path: &Path, ext: &str) -> (PathBuf, bool) {
    let has = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext));
    if has {
        (path.to_path_buf(), false)
    } else {
        let mut s = path.as_os_str().to_os_string();
        s.push(".");
        s.push(ext);
        (PathBuf::from(s), true)
    }
}

/// A suggested file name with `.ext`: the UI's name, without a folder,
/// with the extension added when missing.
pub fn suggested_file_name(suggested: &str, ext: &str) -> String {
    let base = suggested
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim();
    let base = if base.is_empty() { "untitled" } else { base };
    let (p, _) = with_extension(Path::new(base), ext);
    p.to_string_lossy().into_owned()
}

/// A file's name for messages and [`crate::ipc::SavedFiles`].
pub fn display_name(path: &Path) -> String {
    separatrix_project::file_name(path)
}

/// Where each CSV export file goes once the user chose `chosen` (a
/// `.csv` path): with one initial condition, `<chosen>.csv` and
/// `<chosen stem>.meta.json`; with several, `<chosen stem>-<ic>.csv`
/// (and `.meta.json`) for each, `<ic>` being the part of the export's own
/// stem after the system's title. The metadata's `csv` key is rewritten
/// to the name actually written. Returns `(path, contents)` pairs.
pub fn csv_save_plan(chosen: &Path, title: &str, files: &[CsvFileOut]) -> Vec<(PathBuf, String)> {
    let dir = chosen.parent().map(Path::to_path_buf).unwrap_or_default();
    let chosen_stem = chosen
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "trajectory".into());
    let title_prefix = format!("{}-", slug(title, "system"));
    let mut out = Vec::new();
    for f in files {
        let stem = if files.len() == 1 {
            chosen_stem.clone()
        } else {
            let ic = f.stem.strip_prefix(&title_prefix).unwrap_or(&f.stem);
            format!("{chosen_stem}-{ic}")
        };
        let csv_name = format!("{stem}.csv");
        let meta = f.meta.replacen(
            &format!("\"csv\": \"{}.csv\"", f.stem),
            &format!(
                "\"csv\": {}",
                serde_json::to_string(&csv_name).unwrap_or_default()
            ),
            1,
        );
        out.push((dir.join(&csv_name), f.csv.clone()));
        out.push((dir.join(format!("{stem}.meta.json")), meta));
    }
    out
}

/// Decodes the `x-sepx-name` header (`encodeURIComponent` in the UI).
pub fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = s.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{CompileRequest, InputFormat, Method, SolverOptions, TrajectoryRequest};
    use crate::service::SystemStore;

    fn rabbits() -> (SystemStore, u32) {
        let store = SystemStore::new();
        let res = service::compile_system(
            &store,
            &CompileRequest {
                source: "\\dot{x}=x(3-x-2y)\n\\dot{y}=y(2-x-y)".into(),
                format: InputFormat::Latex,
            },
        );
        assert!(res.ok);
        let id = res.system_id.unwrap();
        (store, id)
    }

    fn request(system_id: u32) -> PortraitRequest {
        PortraitRequest {
            request_id: 3,
            system_id,
            params: vec![],
            x_axis: AxisRef::State { index: 0 },
            y_axis: AxisRef::State { index: 1 },
            x_range: (-0.25, 3.5),
            y_range: (-0.25, 2.75),
            base_state: vec![0.0, 0.0],
            field_time: 0.0,
            field_grid: 15,
            nullclines: true,
            fixed_points: true,
            trajectories: vec![
                TrajectoryRequest {
                    x0: vec![0.3, 0.2],
                    t0: 0.0,
                    t1: 20.0,
                },
                TrajectoryRequest {
                    x0: vec![0.3, 0.2],
                    t0: 0.0,
                    t1: -20.0,
                },
            ],
            solver: SolverOptions {
                method: Method::Dopri5,
                rtol: 1e-6,
                atol: 1e-9,
                h: 0.0,
                h_max: None,
                max_steps: 200_000,
            },
            samples: 4000,
            graph_samples: None,
            transient: None,
            section: None,
            z_axis: None,
            z_range: None,
        }
    }

    fn tags() -> Vec<TrajectoryTag> {
        vec![
            TrajectoryTag {
                ic_id: "ic-1".into(),
                backward: false,
                color: 0,
            },
            TrajectoryTag {
                ic_id: "ic-1".into(),
                backward: true,
                color: 0,
            },
        ]
    }

    #[test]
    fn known_methods_match_the_solver() {
        let all = [
            Method::Euler,
            Method::Heun,
            Method::Rk4,
            Method::Dopri5,
            Method::Verlet,
            Method::Yoshida4,
        ];
        let names: Vec<String> = all
            .iter()
            .map(|m| {
                serde_json::to_value(m)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(names, separatrix_project::KNOWN_METHODS);
        // Every method in the solver is listed (a new variant breaks this match).
        for m in all {
            match m {
                Method::Euler
                | Method::Heun
                | Method::Rk4
                | Method::Dopri5
                | Method::Verlet
                | Method::Yoshida4 => {}
            }
        }
    }

    #[test]
    fn export_portrait_is_simplified_and_cached() {
        let (store, id) = rabbits();
        let sys = store.get(id).unwrap();
        let cache = ExportCache::new();
        let req = ExportPortraitRequest {
            portrait: request(id),
            width_px: 400.0,
            height_px: 320.0,
        };
        let full = service::compute_portrait(&sys, &req.portrait).unwrap();
        let a = export_portrait(&cache, &sys, &req).unwrap();
        assert_eq!(a.trajectories.len(), 2);
        for (s, f) in a.trajectories.iter().zip(&full.trajectories) {
            assert!(s.t.len() < f.t.len() / 4, "{} of {}", s.t.len(), f.t.len());
            assert_eq!(s.x.len(), s.t.len() * 2);
            assert_eq!(s.t.first(), f.t.first());
            assert_eq!(s.t.last(), f.t.last(), "endpoints kept");
        }
        // Same request with another id: the cached result, with the new id.
        let mut again = req.clone();
        again.portrait.request_id = 99;
        let b = export_portrait(&cache, &sys, &again).unwrap();
        assert_eq!(b.request_id, 99);
        assert_eq!(b.trajectories, a.trajectories);
        assert_eq!(b.elapsed_ms, a.elapsed_ms, "not recomputed");
        // Another size: recomputed.
        let mut other = req.clone();
        other.width_px = 800.0;
        let c = export_portrait(&cache, &sys, &other).unwrap();
        assert!(c.trajectories[0].t.len() >= a.trajectories[0].t.len());
        let mut bad = req;
        bad.height_px = 0.0;
        assert!(export_portrait(&cache, &sys, &bad).is_err());
    }

    #[test]
    fn figure_system_fixed_points_and_csv() {
        let (store, id) = rabbits();
        let sys = store.get(id).unwrap();
        let cache = ExportCache::new();
        let lines = vec![
            "\\dot{x}=x(3-x-2y)".to_string(),
            "\\dot{y}=y(2-x-y)".to_string(),
        ];
        let fig = FigureLatexRequest {
            portrait: request(id),
            tags: tags(),
            width_cm: 8.0,
            height_cm: 6.4,
            theme: ExportTheme::Light,
            title: "Rabbits vs sheep".into(),
            source_ref: Some("Strogatz §6.4".into()),
            lines: lines.clone(),
            format: InputFormat::Latex,
            standalone: false,
        };
        let inline = figure_latex(&cache, &sys, &fig, Placement::Inline).unwrap();
        assert!(inline.tex.contains("\\begin{axis}"), "{}", inline.tex);
        assert!(inline.files.is_empty());
        assert!(!inline.tex.contains('\u{2212}'));
        let ext = figure_latex(
            &cache,
            &sys,
            &fig,
            Placement::External {
                base: "rabbits".into(),
            },
        )
        .unwrap();
        assert!(!ext.files.is_empty());
        assert!(ext.files.iter().all(|f| f.name.starts_with("rabbits-")));
        let mut wrong = fig.clone();
        wrong.tags.pop();
        assert!(figure_latex(&cache, &sys, &wrong, Placement::Inline).is_err());

        let s = system_latex(
            &sys,
            &SystemLatexRequest {
                system_id: id,
                lines: lines.clone(),
                format: InputFormat::Latex,
                params: vec![],
                notation: Notation::Dot,
                numbered: true,
                title: "Rabbits vs sheep".into(),
                source_ref: None,
            },
        );
        assert!(s.tex.contains("\\begin{align}"), "{}", s.tex);
        assert!(s.preview_latex.contains("\\dot{x}"), "{}", s.preview_latex);

        let fp = fixed_points_latex(
            &sys,
            &FixedPointsLatexRequest {
                portrait: request(id),
                title: "Rabbits vs sheep".into(),
                source_ref: None,
                lines: lines.clone(),
                format: InputFormat::Latex,
                digits: 4,
            },
        )
        .unwrap();
        assert!(fp.contains("\\begin{table}"), "{fp}");
        assert!(fp.contains("Saddle") || fp.contains("saddle"), "{fp}");

        let c = csv(
            &sys,
            &CsvRequest {
                portrait: request(id),
                tags: tags(),
                ic_ids: vec![],
                lines,
                format: InputFormat::Latex,
                title: "Rabbits vs sheep".into(),
            },
        )
        .unwrap();
        assert_eq!(c.files.len(), 1);
        assert_eq!(c.files[0].stem, "rabbits-vs-sheep-trajectory-1");
        assert!(c.files[0].csv.starts_with("t,x,y\n"));
        // Backward reversed, then forward without repeating t0: 2·4000 − 1 rows.
        assert_eq!(c.files[0].csv.lines().count(), 1 + 2 * 4000 - 1);
        assert!(c.files[0].meta.contains(APP_VERSION));
    }

    fn logistic() -> (SystemStore, u32, PortraitRequest) {
        let store = SystemStore::new();
        let res = service::compile_system(
            &store,
            &CompileRequest {
                source: "x_{n+1}=rx_n\\left(1-x_n\\right)".into(),
                format: InputFormat::Latex,
            },
        );
        assert!(res.ok);
        let id = res.system_id.unwrap();
        let mut req = request(id);
        req.params = vec![3.5];
        req.x_axis = AxisRef::State { index: 0 };
        req.y_axis = AxisRef::State { index: 0 };
        req.x_range = (-0.05, 1.05);
        req.y_range = (-0.05, 1.05);
        req.base_state = vec![0.0];
        req.field_grid = 0;
        req.nullclines = false;
        req.trajectories = vec![TrajectoryRequest {
            x0: vec![0.1],
            t0: 0.0,
            t1: 60.0,
        }];
        req.graph_samples = Some(2048);
        req.transient = Some(200);
        (store, id, req)
    }

    #[test]
    fn a_cobweb_exports_its_graph_staircase_and_transient() {
        let (store, id, req) = logistic();
        let sys = store.get(id).unwrap();
        let tags = vec![TrajectoryTag {
            ic_id: "ic-1".into(),
            backward: false,
            color: 0,
        }];
        // The graph is simplified, more finely than a nullcline (a parabola
        // needs far fewer than 2048 points at 0.0625 px); the orbit is only cut.
        let cache = ExportCache::new();
        let out = export_portrait(
            &cache,
            &sys,
            &ExportPortraitRequest {
                portrait: req.clone(),
                width_px: 400.0,
                height_px: 400.0,
            },
        )
        .unwrap();
        let graph = out.graph.as_ref().unwrap();
        assert_eq!(graph.len(), 1);
        assert!(graph[0].len() < 400, "{}", graph[0].len());
        assert_eq!(out.trajectories[0].t.len(), 61);
        assert_eq!(out.trajectories[0].t[0], 200.0);
        let p = to_export_portrait(&sys, &req, &out, &tags).unwrap();
        assert!(p.cobweb);
        assert_eq!(p.transient, 200);
        assert_eq!(p.graph, *graph);

        let fig = figure_latex(
            &cache,
            &sys,
            &FigureLatexRequest {
                portrait: req.clone(),
                tags: tags.clone(),
                width_cm: 8.0,
                height_cm: 8.0,
                theme: ExportTheme::Light,
                title: "Logistic map".into(),
                source_ref: None,
                lines: vec!["x_{n+1}=rx_n\\left(1-x_n\\right)".into()],
                format: InputFormat::Latex,
                standalone: false,
            },
            Placement::Inline,
        )
        .unwrap();
        for want in [
            "% Transient: the first 200 iterates are not drawn.",
            "% Diagonal x_{n+1} = x_n",
            "% Graph of f",
            "% Cobweb of ic-1",
            "{$x_{n+1}$}",
            // The open square at (x_T, x_T), where the staircase starts.
            "\\draw[sepxhalo, line width=2.25bp] ($(rel axis cs:0,1)",
            "rectangle",
        ] {
            assert!(fig.tex.contains(want), "no {want:?} in\n{}", fig.tex);
        }

        let table = fixed_points_latex(
            &sys,
            &FixedPointsLatexRequest {
                portrait: req.clone(),
                title: "Logistic map".into(),
                source_ref: None,
                lines: vec![],
                format: InputFormat::Latex,
                digits: 4,
            },
        )
        .unwrap();
        assert!(table.contains("$\\lambda = f'(x^*)$"), "{table}");

        let files = csv(
            &sys,
            &CsvRequest {
                portrait: req,
                tags,
                ic_ids: vec![],
                lines: vec![],
                format: InputFormat::Latex,
                title: "Logistic map".into(),
            },
        )
        .unwrap()
        .files;
        assert!(files[0].csv.starts_with("n,x\n200,"), "{}", files[0].csv);
        assert_eq!(files[0].csv.lines().count(), 1 + 61);
        assert!(
            files[0].meta.contains("\"transient\": 200,"),
            "{}",
            files[0].meta
        );
    }

    #[test]
    fn a_3d_view_has_no_latex_figure_and_keeps_every_sample() {
        let store = SystemStore::new();
        let res = service::compile_system(
            &store,
            &CompileRequest {
                source: "\\dot{x}=10(y-x)\n\\dot{y}=x(28-z)-y\n\\dot{z}=xy-\\frac{8}{3}z".into(),
                format: InputFormat::Latex,
            },
        );
        assert!(res.ok, "{:?}", res.diagnostics);
        let id = res.system_id.unwrap();
        let sys = store.get(id).unwrap();
        let mut portrait = request(id);
        portrait.x_range = (-25.0, 25.0);
        portrait.y_range = (-30.0, 30.0);
        portrait.base_state = vec![0.0, 0.0, 0.0];
        portrait.field_grid = 0;
        portrait.nullclines = false;
        portrait.trajectories = vec![TrajectoryRequest {
            x0: vec![1.0, 1.0, 1.0],
            t0: 0.0,
            t1: 25.0,
        }];
        portrait.z_axis = Some(AxisRef::State { index: 2 });
        portrait.z_range = Some((0.0, 50.0));
        let fig = FigureLatexRequest {
            portrait: portrait.clone(),
            tags: vec![tags()[0].clone()],
            width_cm: 8.0,
            height_cm: 6.4,
            theme: ExportTheme::Light,
            title: "Lorenz equations".into(),
            source_ref: None,
            lines: vec![],
            format: InputFormat::Latex,
            standalone: true,
        };
        let cache = ExportCache::new();
        let refusal = "LaTeX figures of the 3D view aren't available yet. Export PNG or SVG, or switch to a phase plane.";
        for placement in [
            Placement::Inline,
            Placement::External {
                base: "lorenz".into(),
            },
        ] {
            assert_eq!(
                figure_latex(&cache, &sys, &fig, placement).unwrap_err(),
                refusal
            );
        }
        let mut plane = fig.clone();
        plane.portrait.z_axis = None;
        plane.portrait.z_range = None;
        assert!(figure_latex(&cache, &sys, &plane, Placement::Inline).is_ok());

        let at = |p: &PortraitRequest| {
            export_portrait(
                &ExportCache::new(),
                &sys,
                &ExportPortraitRequest {
                    portrait: p.clone(),
                    width_px: 400.0,
                    height_px: 300.0,
                },
            )
            .unwrap()
            .trajectories[0]
                .t
                .len()
        };
        assert_eq!(at(&portrait), portrait.samples);
        assert!(at(&plane.portrait) < portrait.samples / 2);
    }

    #[test]
    fn csv_save_names() {
        let one = vec![CsvFileOut {
            stem: "lorenz-trajectory-1".into(),
            csv: "t,x\n".into(),
            meta: "{\n  \"csv\": \"lorenz-trajectory-1.csv\",\n  \"x\": 1\n}\n".into(),
        }];
        let plan = csv_save_plan(Path::new("/d/run.csv"), "Lorenz", &one);
        assert_eq!(plan[0].0, Path::new("/d/run.csv"));
        assert_eq!(plan[1].0, Path::new("/d/run.meta.json"));
        assert!(plan[1].1.contains("\"csv\": \"run.csv\""), "{}", plan[1].1);
        let mut two = one.clone();
        two.push(CsvFileOut {
            stem: "lorenz-trajectory-2".into(),
            csv: String::new(),
            meta: "{\n  \"csv\": \"lorenz-trajectory-2.csv\"\n}\n".into(),
        });
        let plan = csv_save_plan(Path::new("/d/run.csv"), "Lorenz", &two);
        let names: Vec<_> = plan.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(
            names,
            vec![
                PathBuf::from("/d/run-trajectory-1.csv"),
                PathBuf::from("/d/run-trajectory-1.meta.json"),
                PathBuf::from("/d/run-trajectory-2.csv"),
                PathBuf::from("/d/run-trajectory-2.meta.json"),
            ]
        );
        assert!(plan[3].1.contains("\"csv\": \"run-trajectory-2.csv\""));
    }

    #[test]
    fn extensions_and_names() {
        assert_eq!(
            with_extension(Path::new("a/b.tex"), "tex"),
            (PathBuf::from("a/b.tex"), false)
        );
        assert_eq!(
            with_extension(Path::new("a/b.TEX"), "tex"),
            (PathBuf::from("a/b.TEX"), false)
        );
        assert_eq!(
            with_extension(Path::new("a/b"), "sepx"),
            (PathBuf::from("a/b.sepx"), true)
        );
        assert_eq!(
            with_extension(Path::new("a/b.json"), "sepx"),
            (PathBuf::from("a/b.json.sepx"), true)
        );
        assert_eq!(suggested_file_name("Lorenz", "sepx"), "Lorenz.sepx");
        assert_eq!(suggested_file_name("../../x.svg", "svg"), "x.svg");
        assert_eq!(suggested_file_name("  ", "csv"), "untitled.csv");
        assert_eq!(figure_px(2.54), 96.0);
    }

    #[test]
    fn header_names_are_percent_decoded() {
        assert_eq!(
            percent_decode("Rabbits%20vs%20sheep%20%E2%80%94%202x.png").as_deref(),
            Some("Rabbits vs sheep — 2x.png")
        );
        assert_eq!(percent_decode("plain.png").as_deref(), Some("plain.png"));
        assert_eq!(percent_decode("bad%2"), None);
        assert_eq!(percent_decode("bad%zz"), None);
        assert_eq!(percent_decode("%FF"), None, "not UTF-8");
    }

    #[test]
    fn both_palettes_load() {
        let light = palette(ExportTheme::Light).unwrap();
        let dark = palette(ExportTheme::Dark).unwrap();
        assert_eq!(light.traj.len(), 7);
        assert_ne!(light.bg, dark.bg);
    }
}
