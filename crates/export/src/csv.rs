//! Trajectories as CSV: RFC 4180 with LF line endings, UTF-8 without a
//! BOM, one header row (`t` or `n`, then every state by name), numbers in
//! shortest round-trip form. One file per initial condition: the backward
//! run (reversed, so time increases) then the forward run, without
//! repeating the start row. Each run is cut, in integration order, at its
//! first sample with a value that is not finite. A `.meta.json` beside it
//! says where the data comes from.

use dyn_expr::InputFormat;
use serde::Serialize;

use crate::latex_util::title_slug;
use crate::model::{
    codim2_label, ContinuationRun, Labels, LyapunovRun, Method, NormalForm, OrbitRun, PlaneRun,
    SectionSpec, SystemKind, SystemMeta, Trajectory,
};
use crate::num::shortest;
use crate::words::{status_id, time_name};

/// One field, quoted when it holds `,`, `"`, CR or LF (quotes doubled).
pub fn field(s: &str) -> String {
    if s.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// One record, LF-terminated.
pub fn record<S: AsRef<str>>(fields: &[S]) -> String {
    let mut out = fields
        .iter()
        .map(|f| field(f.as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    out.push('\n');
    out
}

/// Number of leading samples of a run with every value finite.
pub fn finite_rows(t: &[f64], x: &[f64], dim: usize) -> usize {
    let n = t.len().min(x.len().checked_div(dim).unwrap_or(t.len()));
    (0..n)
        .position(|k| !t[k].is_finite() || x[k * dim..(k + 1) * dim].iter().any(|v| !v.is_finite()))
        .unwrap_or(n)
}

/// The rows of one initial condition: `(t, state)` in increasing time.
fn rows<'a>(
    dim: usize,
    backward: Option<&'a Trajectory>,
    forward: Option<&'a Trajectory>,
) -> Vec<(f64, &'a [f64])> {
    let mut out: Vec<(f64, &[f64])> = Vec::new();
    if let Some(b) = backward {
        let m = finite_rows(&b.t, &b.x, dim);
        for k in (0..m).rev() {
            out.push((b.t[k], &b.x[k * dim..(k + 1) * dim]));
        }
    }
    if let Some(f) = forward {
        let m = finite_rows(&f.t, &f.x, dim);
        let skip = usize::from(!out.is_empty() && m > 0 && out.last().map(|r| r.0) == Some(f.t[0]));
        for k in skip..m {
            out.push((f.t[k], &f.x[k * dim..(k + 1) * dim]));
        }
    }
    out
}

/// The CSV of one initial condition. `columns` are the state names.
pub fn trajectory(
    kind: SystemKind,
    columns: &[String],
    backward: Option<&Trajectory>,
    forward: Option<&Trajectory>,
) -> String {
    let dim = columns.len();
    let mut header = vec![time_name(kind).to_string()];
    header.extend(columns.iter().cloned());
    let mut out = record(&header);
    for (t, x) in rows(dim, backward, forward) {
        let mut cells = Vec::with_capacity(dim + 1);
        cells.push(shortest(t));
        cells.extend(x.iter().map(|&v| shortest(v)));
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

#[derive(Serialize)]
struct MetaSystem<'a> {
    kind: SystemKind,
    input_format: InputFormat,
    lines: &'a [String],
}

#[derive(Serialize)]
struct MetaParam<'a> {
    name: &'a str,
    value: f64,
}

#[derive(Serialize)]
struct MetaSegment {
    direction: &'static str,
    t_range: Option<(f64, f64)>,
    samples: usize,
    status: &'static str,
}

#[derive(Serialize)]
struct Meta<'a> {
    format: &'static str,
    format_version: u32,
    app_version: &'a str,
    title: &'a str,
    csv: String,
    ic_id: &'a str,
    system: MetaSystem<'a>,
    params: Vec<MetaParam<'a>>,
    columns: Vec<String>,
    x0: &'a [f64],
    t0: f64,
    /// Maps only: iterates computed but not written (the first row is `n = T`).
    #[serde(skip_serializing_if = "Option::is_none")]
    transient: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_crossings: Option<usize>,
    segments: Vec<MetaSegment>,
    notes: Vec<&'static str>,
}

/// What [`files`] needs.
#[derive(Debug, Clone)]
pub struct CsvInput<'a> {
    /// Flow or map.
    pub kind: SystemKind,
    /// State names (CSV header) and parameter values.
    pub labels: &'a Labels,
    /// The runs, tagged with their initial condition and direction.
    pub trajectories: &'a [Trajectory],
    /// Which initial conditions to export, in order; empty for all.
    pub ic_ids: &'a [String],
    /// The lines as typed.
    pub lines: &'a [String],
    /// Their format.
    pub format: InputFormat,
    /// The system's title (file names).
    pub title: &'a str,
    /// The app's version, for the metadata.
    pub app_version: &'a str,
    /// Map iterates, or a section's crossings, computed but not written
    /// before each orbit.
    pub transient: usize,
    /// A Poincaré section: each run is the orbit's crossings of it.
    pub section: Option<SectionSpec>,
}

/// One exported initial condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvFile {
    /// File stem: `<stem>.csv` and `<stem>.meta.json`.
    pub stem: String,
    /// The CSV.
    pub csv: String,
    /// The metadata JSON (pretty, LF, trailing newline).
    pub meta: String,
}

/// The metadata of one initial condition.
pub fn meta(
    input: &CsvInput,
    stem: &str,
    backward: Option<&Trajectory>,
    forward: Option<&Trajectory>,
) -> String {
    let dim = input.labels.states.len();
    let first = forward.or(backward);
    let mut columns = vec![time_name(input.kind).to_string()];
    columns.extend(input.labels.states.iter().map(|s| s.name.clone()));
    let segment = |tr: &Trajectory, backward: bool| {
        let m = finite_rows(&tr.t, &tr.x, dim);
        let (a, b) = if m > 0 {
            (tr.t[0], tr.t[m - 1])
        } else {
            (0.0, 0.0)
        };
        MetaSegment {
            direction: if backward { "backward" } else { "forward" },
            t_range: (m > 0).then_some((a.min(b), a.max(b))),
            samples: m,
            status: status_id(tr.status),
        }
    };
    let mut segments = Vec::new();
    if let Some(b) = backward {
        segments.push(segment(b, true));
    }
    if let Some(f) = forward {
        segments.push(segment(f, false));
    }
    let mut notes = vec![match input.section {
        Some(SectionSpec::Plane { .. }) => {
            "Rows are the orbit's crossings of the section, in increasing time; the start point is not one."
        }
        Some(SectionSpec::Stroboscopic { .. }) => {
            "Rows are the samples at t = phase + k·period inside the run, in increasing time; t0 is a row when it is a sample time."
        }
        None => "Rows are in increasing time; the start row appears once.",
    }];
    if backward.is_some() && forward.is_some() && input.kind == SystemKind::Ode {
        notes.push("The time step can differ between the backward and forward segments.");
    }
    let doc = Meta {
        format: "separatrix-trajectory",
        format_version: 1,
        app_version: input.app_version,
        title: input.title,
        csv: format!("{stem}.csv"),
        ic_id: first.map_or("", |t| t.tag.ic_id.as_str()),
        system: MetaSystem {
            kind: input.kind,
            input_format: input.format,
            lines: input.lines,
        },
        params: input
            .labels
            .params
            .iter()
            .map(|p| MetaParam {
                name: &p.name,
                value: p.value,
            })
            .collect(),
        columns,
        x0: first.map_or(&[][..], |t| t.x0.as_slice()),
        t0: first.map_or(0.0, |t| t.t0),
        // The orbit's first row is x_T; `input.transient` when it has none.
        transient: (input.kind == SystemKind::Map).then(|| {
            first
                .and_then(|t| t.t.first())
                .filter(|n| n.is_finite() && **n >= 0.0)
                .map_or(input.transient, |&n| n as usize)
        }),
        section: input.section.map(|s| s.describe(input.labels)),
        skipped_crossings: input.section.map(|_| input.transient),
        segments,
        notes,
    };
    let mut s = serde_json::to_string_pretty(&doc).unwrap_or_default();
    s.push('\n');
    s
}

/// One CSV and metadata file per initial condition.
pub fn files(input: &CsvInput) -> Vec<CsvFile> {
    let mut ids: Vec<String> = if input.ic_ids.is_empty() {
        let mut v: Vec<String> = Vec::new();
        for t in input.trajectories {
            if !v.contains(&t.tag.ic_id) {
                v.push(t.tag.ic_id.clone());
            }
        }
        v
    } else {
        input.ic_ids.to_vec()
    };
    ids.dedup();
    // Files are numbered as the dialog lists the trajectories ("Trajectory
    // 2" → `<title>-trajectory-2.csv`), by first appearance among the runs;
    // the initial condition's id stays in the metadata.
    let mut order: Vec<&str> = Vec::new();
    for t in input.trajectories {
        if !order.contains(&t.tag.ic_id.as_str()) {
            order.push(&t.tag.ic_id);
        }
    }
    let base = title_slug(input.title);
    let columns: Vec<String> = input.labels.states.iter().map(|s| s.name.clone()).collect();
    let mut stems: Vec<String> = Vec::new();
    ids.iter()
        .filter_map(|id| {
            let backward = input
                .trajectories
                .iter()
                .find(|t| &t.tag.ic_id == id && t.tag.backward);
            let forward = input
                .trajectories
                .iter()
                .find(|t| &t.tag.ic_id == id && !t.tag.backward);
            if backward.is_none() && forward.is_none() {
                return None;
            }
            let k = order.iter().position(|o| o == id).map_or(0, |i| i + 1);
            let mut stem = format!("{base}-trajectory-{k}");
            while stems.contains(&stem) {
                stem.push_str("-2");
            }
            stems.push(stem.clone());
            Some(CsvFile {
                csv: trajectory(input.kind, &columns, backward, forward),
                meta: meta(input, &stem, backward, forward),
                stem,
            })
        })
        .collect()
}

fn meta_params(labels: &Labels) -> Vec<MetaParam<'_>> {
    labels
        .params
        .iter()
        .map(|p| MetaParam {
            name: &p.name,
            value: p.value,
        })
        .collect()
}

fn meta_system<'a>(sys: &SystemMeta<'a>) -> MetaSystem<'a> {
    MetaSystem {
        kind: sys.kind,
        input_format: sys.format,
        lines: sys.lines,
    }
}

fn pretty<T: Serialize>(doc: &T) -> String {
    let mut s = serde_json::to_string_pretty(doc).unwrap_or_default();
    s.push('\n');
    s
}

#[derive(Serialize)]
struct MetaIntegrator {
    method: Method,
    #[serde(skip_serializing_if = "Option::is_none")]
    h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rtol: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atol: Option<f64>,
}

#[derive(Serialize)]
struct LyapunovMeta<'a> {
    format: &'static str,
    format_version: u32,
    app_version: &'a str,
    title: &'a str,
    csv: String,
    system: MetaSystem<'a>,
    params: Vec<MetaParam<'a>>,
    columns: Vec<String>,
    x0: &'a [f64],
    t0: f64,
    transient: f64,
    total: f64,
    interval: f64,
    blocks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    integrator: Option<MetaIntegrator>,
    renormalizations: usize,
    exponents: &'a [f64],
    uncertainty: &'a [f64],
    sum: f64,
    trace_average: f64,
    status: &'static str,
    notes: Vec<&'static str>,
}

/// The running estimates of a Lyapunov run: `t` (or `n`), then
/// `lambda_1 … lambda_n`, one row per sampled renormalization (the last row
/// is the final estimate), cut at the first row with a value that is not
/// finite; and its `.meta.json` with the settings and the result.
pub fn lyapunov(sys: &SystemMeta, run: &LyapunovRun, app_version: &str) -> CsvFile {
    let s = &run.spectrum;
    let n = s.exponents.len();
    let mut columns = vec![time_name(sys.kind).to_string()];
    columns.extend((1..=n).map(|i| format!("lambda_{i}")));
    let mut csv = record(&columns);
    let rows = s
        .history_t
        .len()
        .min(s.history.len().checked_div(n).unwrap_or(0));
    for k in 0..rows {
        let row = &s.history[k * n..(k + 1) * n];
        if !s.history_t[k].is_finite() || row.iter().any(|v| !v.is_finite()) {
            break;
        }
        let mut cells = vec![shortest(s.history_t[k])];
        cells.extend(row.iter().map(|&v| shortest(v)));
        csv.push_str(&cells.join(","));
        csv.push('\n');
    }
    let stem = format!("{}-lyapunov", title_slug(sys.title));
    let o = &run.solver;
    let integrator = (sys.kind == SystemKind::Ode).then(|| {
        let adaptive = o.method.is_adaptive();
        MetaIntegrator {
            method: o.method,
            h: (!adaptive).then_some(o.h),
            rtol: adaptive.then_some(o.rtol),
            atol: adaptive.then_some(o.atol),
        }
    });
    let meta = pretty(&LyapunovMeta {
        format: "separatrix-lyapunov",
        format_version: 1,
        app_version,
        title: sys.title,
        csv: format!("{stem}.csv"),
        system: meta_system(sys),
        params: meta_params(sys.labels),
        columns,
        x0: &run.x0,
        t0: run.t0,
        transient: run.transient,
        total: run.total,
        interval: run.interval,
        blocks: run.blocks,
        integrator,
        renormalizations: s.renormalizations,
        exponents: &s.exponents,
        uncertainty: &s.uncertainty,
        sum: s.sum,
        trace_average: s.trace_average,
        status: status_id(s.status),
        notes: vec![
            "Rows are the running estimates after sampled renormalizations, in increasing time; the last row is the final estimate.",
            "uncertainty is the standard error over the blocks; trace_average is what sum must equal (flow: the time average of tr J; map: the mean of ln |det J|).",
        ],
    });
    CsvFile { stem, csv, meta }
}

#[derive(Serialize)]
struct OrbitMeta<'a> {
    format: &'static str,
    format_version: u32,
    app_version: &'a str,
    title: &'a str,
    csv: String,
    system: MetaSystem<'a>,
    params: Vec<MetaParam<'a>>,
    columns: Vec<String>,
    swept: &'a str,
    range: Option<(f64, f64)>,
    parameter_values: usize,
    state: &'a str,
    x0: &'a [f64],
    transient: usize,
    kept: usize,
    rows: usize,
    notes: Vec<&'static str>,
}

/// An orbit diagram as rows `r,n,x` (the swept parameter's and the state's
/// names), column by column, one per finite kept iterate, `n` its index
/// (`T … T + kept − 1`); and its `.meta.json`.
pub fn orbit(sys: &SystemMeta, run: &OrbitRun, app_version: &str) -> CsvFile {
    let d = &run.diagram;
    let param = sys
        .labels
        .params
        .get(run.param)
        .map_or("r", |p| p.name.as_str());
    let state = sys
        .labels
        .states
        .get(run.state)
        .map_or("x", |s| s.name.as_str());
    let columns = vec![param.to_string(), "n".to_string(), state.to_string()];
    let mut csv = record(&columns);
    let mut rows = 0;
    for (j, &r) in d.r.iter().enumerate() {
        let col = d.x.get(j * d.kept..(j + 1) * d.kept).unwrap_or(&[]);
        for (k, &x) in col.iter().enumerate() {
            if x.is_finite() {
                csv.push_str(&format!(
                    "{},{},{}\n",
                    shortest(r),
                    run.transient + k,
                    shortest(x)
                ));
                rows += 1;
            }
        }
    }
    let stem = format!("{}-orbit-diagram", title_slug(sys.title));
    let meta = pretty(&OrbitMeta {
        format: "separatrix-orbit-diagram",
        format_version: 1,
        app_version,
        title: sys.title,
        csv: format!("{stem}.csv"),
        system: meta_system(sys),
        params: meta_params(sys.labels),
        columns,
        swept: param,
        range: d.r.first().zip(d.r.last()).map(|(&a, &b)| (a, b)),
        parameter_values: d.r.len(),
        state,
        x0: &run.x0,
        transient: run.transient,
        kept: d.kept,
        rows,
        notes: vec![
            "One row per kept iterate, parameter value by parameter value; n counts from the start point, so the first kept iterate is n = transient.",
            "Iterates of an orbit that escaped are left out, and so is the rest of that orbit.",
        ],
    });
    CsvFile { stem, csv, meta }
}

#[derive(Serialize)]
struct MetaBranch {
    branch: usize,
    period: usize,
    points: usize,
    rows: usize,
    special: usize,
}

#[derive(Serialize)]
struct ContinuationMeta<'a> {
    format: &'static str,
    format_version: u32,
    app_version: &'a str,
    title: &'a str,
    csv: String,
    special_csv: String,
    system: MetaSystem<'a>,
    params: Vec<MetaParam<'a>>,
    columns: Vec<String>,
    special_columns: Vec<String>,
    swept: &'a str,
    range: (f64, f64),
    branches: Vec<MetaBranch>,
    notes: Vec<&'static str>,
}

/// A branch plot's (or the parameter plane's) CSV, its `.meta.json` and
/// the special points' `<stem>-special.csv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationCsv {
    /// `<stem>.csv` and `<stem>.meta.json`.
    pub file: CsvFile,
    /// `<stem>-special.csv`.
    pub special: String,
}

/// The branches as rows `branch,period,point,<param>,<states…>,stable,unstable`
/// (branches from 1, points from 0 in arclength order; a `k`-cycle's point
/// is `k` rows, `x, f(x), …`), the special points as rows
/// `branch,point,type,period,<param>,<states…>,omega,lyapunov` (empty where
/// a kind has none), and the metadata.
pub fn continuation(sys: &SystemMeta, run: &ContinuationRun, app_version: &str) -> ContinuationCsv {
    let dim = sys.labels.states.len();
    let param = sys
        .labels
        .params
        .get(run.param)
        .map_or("r", |p| p.name.as_str());
    let states: Vec<String> = sys.labels.states.iter().map(|s| s.name.clone()).collect();
    let mut columns = vec![
        "branch".to_string(),
        "period".into(),
        "point".into(),
        param.into(),
    ];
    columns.extend(states.iter().cloned());
    columns.extend(["stable".to_string(), "unstable".into()]);
    let mut special_columns = vec![
        "branch".to_string(),
        "point".into(),
        "type".into(),
        "period".into(),
        param.into(),
    ];
    special_columns.extend(states.iter().cloned());
    special_columns.extend(["omega".to_string(), "lyapunov".into()]);
    let mut csv = record(&columns);
    let mut special = record(&special_columns);
    let mut branches = Vec::new();
    for (b, br) in run.branches.iter().enumerate() {
        let mut rows = 0;
        for i in 0..br.len() {
            for m in 0..br.period {
                let mut cells = vec![
                    (b + 1).to_string(),
                    br.period.to_string(),
                    i.to_string(),
                    shortest(br.p[i]),
                ];
                cells.extend(br.state(dim, i, m).iter().map(|&v| shortest(v)));
                cells.push(br.stable.get(i).copied().unwrap_or(false).to_string());
                cells.push(br.unstable.get(i).copied().unwrap_or(0).to_string());
                csv.push_str(&record(&cells));
                rows += 1;
            }
        }
        for sp in &br.special {
            let mut cells = vec![
                (b + 1).to_string(),
                sp.index.to_string(),
                sp.kind.label().to_string(),
                br.period.to_string(),
                shortest(sp.p),
            ];
            cells.extend(sp.x.iter().map(|&v| shortest(v)));
            cells.push(sp.omega.map_or(String::new(), shortest));
            cells.push(sp.lyapunov.map_or(String::new(), shortest));
            special.push_str(&record(&cells));
        }
        branches.push(MetaBranch {
            branch: b + 1,
            period: br.period,
            points: br.len(),
            rows,
            special: br.special.len(),
        });
    }
    let stem = format!("{}-continuation", title_slug(sys.title));
    let meta = pretty(&ContinuationMeta {
        format: "separatrix-continuation",
        format_version: 1,
        app_version,
        title: sys.title,
        csv: format!("{stem}.csv"),
        special_csv: format!("{stem}-special.csv"),
        system: meta_system(sys),
        params: meta_params(sys.labels),
        columns,
        special_columns,
        swept: param,
        range: run.range,
        branches,
        notes: vec![
            "One row per point of each branch in arclength order, point counted from 0; a point of a k-cycle of a map is k rows, x, f(x), …, f^(k-1)(x).",
            "stable: every eigenvalue has Re < 0 (flow) or every multiplier |mu| < 1 (map); unstable: how many do not. A special point is neutral: not stable, with the smaller count of its two sides.",
            "The special CSV names each point by branch and point; omega is the frequency at a Hopf point (H) and the angle arg mu at a Neimark-Sacker point (NS); lyapunov is the first Lyapunov coefficient at H, with <p, q> = 1 and <q, q> = 1/2.",
            "The swept parameter's value in params is the slider's; each branch starts at its own value.",
        ],
    });
    ContinuationCsv {
        file: CsvFile { stem, csv, meta },
        special,
    }
}

#[derive(Serialize)]
struct MetaTick {
    q: usize,
    p: (f64, f64),
}

#[derive(Serialize)]
struct MetaCurve {
    curve: usize,
    kind: &'static str,
    period: usize,
    points: usize,
    special: usize,
    ticks: Vec<MetaTick>,
}

#[derive(Serialize)]
struct CurvesMeta<'a> {
    format: &'static str,
    format_version: u32,
    app_version: &'a str,
    title: &'a str,
    csv: String,
    special_csv: String,
    system: MetaSystem<'a>,
    params: Vec<MetaParam<'a>>,
    columns: Vec<String>,
    special_columns: Vec<String>,
    across: &'a str,
    up: &'a str,
    ranges: ((f64, f64), (f64, f64)),
    curves: Vec<MetaCurve>,
    notes: Vec<&'static str>,
}

/// The parameter plane's curves as rows
/// `curve,kind,period,point,<across>,<up>,<states…>,neutral` (curves from 1
/// in the view's order, the homoclinic curves last; points from 0 in
/// arclength order), the codimension-2 points as rows
/// `curve,point,type,period,<across>,<up>,<states…>,a,b,c,theta` as each
/// curve reports them (empty where a point has none), and the metadata with
/// the weak resonances of each curve.
pub fn curves(sys: &SystemMeta, run: &PlaneRun, app_version: &str) -> ContinuationCsv {
    let dim = sys.labels.states.len();
    let name = |i: usize, fallback: &'static str| {
        sys.labels
            .params
            .get(i)
            .map_or(fallback, |p| p.name.as_str())
    };
    let (across, up) = (name(run.free.0, "p"), name(run.free.1, "q"));
    let states: Vec<String> = sys.labels.states.iter().map(|s| s.name.clone()).collect();
    let mut columns: Vec<String> = ["curve", "kind", "period", "point", across, up]
        .iter()
        .map(|s| s.to_string())
        .collect();
    columns.extend(states.iter().cloned());
    columns.push("neutral".into());
    let mut special_columns: Vec<String> = ["curve", "point", "type", "period", across, up]
        .iter()
        .map(|s| s.to_string())
        .collect();
    special_columns.extend(states.iter().cloned());
    special_columns.extend(["a", "b", "c", "theta"].iter().map(|s| s.to_string()));
    let mut csv = record(&columns);
    let mut special = record(&special_columns);
    let mut curves = Vec::new();
    for (k, c) in run.curves.iter().enumerate() {
        for i in 0..c.len() {
            let mut cells = vec![
                (k + 1).to_string(),
                c.kind.label().to_string(),
                c.period.to_string(),
                i.to_string(),
                shortest(c.p1[i]),
                shortest(c.p2[i]),
            ];
            let x = c.x.get(i * dim..(i + 1) * dim).unwrap_or(&[]);
            cells.extend(x.iter().map(|&v| shortest(v)));
            cells.push(c.neutral.get(i).copied().unwrap_or(false).to_string());
            csv.push_str(&record(&cells));
        }
        for sp in &c.special {
            let mut cells = vec![
                (k + 1).to_string(),
                sp.index.to_string(),
                codim2_label(sp.kind).to_string(),
                c.period.to_string(),
                shortest(sp.p.0),
                shortest(sp.p.1),
            ];
            cells.extend(sp.x.iter().map(|&v| shortest(v)));
            let (a, b, cc) = match sp.normal_form {
                Some(NormalForm::BogdanovTakens { a, b }) => {
                    (shortest(a), shortest(b), String::new())
                }
                Some(NormalForm::Cusp { c }) => (String::new(), String::new(), shortest(c)),
                None => (String::new(), String::new(), String::new()),
            };
            cells.extend([a, b, cc, sp.angle.map_or(String::new(), shortest)]);
            special.push_str(&record(&cells));
        }
        curves.push(MetaCurve {
            curve: k + 1,
            kind: c.kind.label(),
            period: c.period,
            points: c.len(),
            special: c.special.len(),
            ticks: c
                .ticks
                .iter()
                .map(|t| MetaTick { q: t.q, p: t.p })
                .collect(),
        });
    }
    let stem = format!("{}-curves", title_slug(sys.title));
    let meta = pretty(&CurvesMeta {
        format: "separatrix-curves",
        format_version: 1,
        app_version,
        title: sys.title,
        csv: format!("{stem}.csv"),
        special_csv: format!("{stem}-special.csv"),
        system: meta_system(sys),
        params: meta_params(sys.labels),
        columns,
        special_columns,
        across,
        up,
        ranges: run.ranges,
        curves,
        notes: vec![
            "One row per point of each curve in arclength order, point counted from 0; kind is LP (fold), H (Hopf), BP (branch point), PD (flip), NS (Neimark-Sacker) or Hom (homoclinic); a map's state is the cycle's first point.",
            "neutral: the point is a neutral saddle, where the curve's equations hold without a bifurcation (real eigenvalues +-kappa on a Hopf curve, real multipliers mu and 1/mu on a Neimark-Sacker curve).",
            "The special CSV names each codimension-2 point by curve and point; a and b are the Bogdanov-Takens coefficients (oriented so that a >= 0), c the cusp's cubic coefficient, theta the angle of the critical multipliers on a Neimark-Sacker curve.",
            "ticks are the weak resonances 1:q on a Neimark-Sacker curve, p the across and up parameters there.",
            "The across and up parameters' values in params are the sliders'; each curve has its own.",
        ],
    });
    ContinuationCsv {
        file: CsvFile { stem, csv, meta },
        special,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_quoted_only_when_needed() {
        assert_eq!(field("x"), "x");
        assert_eq!(field("theta'"), "theta'");
        assert_eq!(field("a,b"), "\"a,b\"");
        assert_eq!(field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(field("a\nb"), "\"a\nb\"");
        assert_eq!(field("a\rb"), "\"a\rb\"");
        assert_eq!(record(&["t", "x,1", "é"]), "t,\"x,1\",é\n");
    }
}
