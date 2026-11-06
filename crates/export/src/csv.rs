//! Trajectories as CSV: RFC 4180 with LF line endings, UTF-8 without a
//! BOM, one header row (`t` or `n`, then every state by name), numbers in
//! shortest round-trip form. One file per initial condition: the backward
//! run (reversed, so time increases) then the forward run, without
//! repeating the start row. Each run is cut, in integration order, at its
//! first sample with a value that is not finite. A `.meta.json` beside it
//! says where the data comes from.

use dyn_expr::InputFormat;
use serde::Serialize;

use crate::latex_util::slug;
use crate::model::{
    Labels, LyapunovRun, Method, OrbitRun, SectionSpec, SystemKind, SystemMeta, Trajectory,
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
    let base = slug(input.title, "system");
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
    let stem = format!("{}-lyapunov", slug(sys.title, "system"));
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
    let stem = format!("{}-orbit-diagram", slug(sys.title, "system"));
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
