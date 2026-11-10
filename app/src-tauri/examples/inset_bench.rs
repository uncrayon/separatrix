//! Times the Parameter plane's live inset in the desktop core: each
//! `compute_portrait` request the interface sent while the crosshair was
//! dragged, replayed through `service::compute_portrait`.
//!
//! ```sh
//! cargo run --release -p separatrix --example inset_bench -- requests.json [runs]
//! ```
//!
//! `requests.json` is `[{ "template": "<id>", "request": <PortraitRequest> }]`,
//! as the end-to-end test `curves.e2e.cjs` writes it when
//! `SEPX_INSET_REQUESTS` names a path. Each request's `systemId` is replaced
//! by the template compiled here.

use std::collections::BTreeMap;
use std::time::Instant;

use separatrix_lib::ipc::{CompileRequest, InputFormat, PortraitRequest};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

#[derive(Deserialize)]
struct Template {
    id: String,
    format: InputFormat,
    equations: Vec<String>,
}

#[derive(Deserialize)]
struct Captured {
    template: String,
    request: PortraitRequest,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: inset_bench requests.json [runs]");
    let runs: usize = args.next().map_or(7, |r| r.parse().expect("runs"));
    let captured: Vec<Captured> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read requests"))
            .expect("parse requests");
    let templates: Vec<Template> = serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
    let store = SystemStore::new();
    let mut by_template: BTreeMap<String, Vec<PortraitRequest>> = BTreeMap::new();
    for c in captured {
        by_template.entry(c.template).or_default().push(c.request);
    }
    for (id, requests) in &mut by_template {
        let t = templates
            .iter()
            .find(|t| &t.id == id)
            .unwrap_or_else(|| panic!("no template {id}"));
        let res = service::compile_system(
            &store,
            &CompileRequest {
                source: t.equations.join("\n"),
                format: t.format,
            },
        );
        let system_id = res
            .system_id
            .unwrap_or_else(|| panic!("{id} doesn't compile"));
        let system = store.get(system_id).expect("compiled system");
        for r in requests.iter_mut() {
            r.system_id = system_id;
        }
        let mut times = Vec::new();
        let (mut errors, mut points, mut fixed) = (0usize, 0usize, 0usize);
        for run in 0..=runs {
            for r in requests.iter() {
                let started = Instant::now();
                let out = service::compute_portrait(&system, r);
                let ms = started.elapsed().as_secs_f64() * 1e3;
                match out {
                    Ok(out) if run > 0 => {
                        times.push(ms);
                        points += out.trajectories.iter().map(|tr| tr.t.len()).sum::<usize>();
                        fixed += out.fixed_points.len();
                    }
                    Ok(_) => {}
                    Err(_) => errors += 1,
                }
            }
        }
        times.sort_by(f64::total_cmp);
        let at = |q: f64| times[((times.len() - 1) as f64 * q).round() as usize];
        println!(
            "{id}: {} requests × {runs} runs, median {:.2} ms, p95 {:.2} ms, max {:.2} ms, min {:.2} ms; \
             {errors} errors; {points} trajectory points and {fixed} fixed points returned",
            requests.len(),
            at(0.5),
            at(0.95),
            at(1.0),
            at(0.0),
        );
    }
}
