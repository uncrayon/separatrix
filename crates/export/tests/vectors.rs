//! The vectors shared with the TypeScript tests: numbers, ticks and arrow
//! heads computed by the UI's own code must come out the same here.
//!
//! - `tests/vectors/numbers.json`: JavaScript `String(v)`.
//! - `tests/vectors/ticks.json`: `niceTicks` and `tickLabel`
//!   (`app/src/canvas/viewport.ts`).
//! - `tests/vectors/heads.json`: `drawHeads` (`app/src/canvas/draw.ts`)
//!   through a recording context, written by the UI's tests; checked when
//!   present.
//! - `tests/vectors/cobweb.json`: the cobweb staircase and diagonal
//!   (`app/src/canvas/cobweb.ts`), written by the UI's tests; checked when
//!   present.

use std::path::Path;

use separatrix_export::heads;
use separatrix_export::num::shortest;
use separatrix_export::ticks;
use separatrix_export::Viewport;
use serde_json::Value;

fn vector(name: &str) -> Option<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors")
        .join(name);
    let text = std::fs::read_to_string(path).ok()?;
    Some(serde_json::from_str(&text).expect("valid JSON"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().expect("a number")
}

#[test]
fn shortest_is_javascript_string() {
    let doc = vector("numbers.json").expect("numbers.json");
    let cases = doc["cases"].as_array().unwrap();
    assert!(cases.len() > 20);
    for c in cases {
        let s = c["s"].as_str().unwrap();
        let v: f64 = s.parse().unwrap();
        assert_eq!(shortest(v), s, "String({v:e})");
        assert_eq!(
            shortest(f(&c["v"])),
            s,
            "String({v:e}) from the JSON number"
        );
    }
}

#[test]
fn ticks_and_labels_match_the_screen() {
    let doc = vector("ticks.json").expect("ticks.json");
    let (mut exact, mut pow_cases) = (0, 0);
    for c in doc["ticks"].as_array().unwrap() {
        let (lo, hi, target) = (f(&c["lo"]), f(&c["hi"]), f(&c["target"]));
        let t = ticks::nice(lo, hi, target);
        let step = f(&c["step"]);
        if t.step != step {
            // V8's Math.pow is not correctly rounded (Math.pow(10, -5) is
            // 9.999999999999999e-6); Rust's is, like JavaScriptCore's, which
            // draws the screen on Linux and macOS. The step then differs in
            // its last bit and the ticks of a range at the edge of a step can
            // shift; only the step is compared.
            assert!(
                (t.step - step).abs() <= 4.0 * f64::EPSILON * step.abs(),
                "step of [{lo}, {hi}] / {target}: {} vs {step}",
                t.step
            );
            pow_cases += 1;
            continue;
        }
        let want: Vec<f64> = c["ticks"].as_array().unwrap().iter().map(f).collect();
        assert_eq!(t.ticks, want, "ticks of [{lo}, {hi}] / {target}");
        let labels: Vec<String> = t.ticks.iter().map(|&v| ticks::label(v, t.step)).collect();
        let want: Vec<&str> = c["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(labels, want, "labels of [{lo}, {hi}] / {target}");
        exact += 1;
    }
    assert!(
        exact >= 20 && pow_cases <= 3,
        "{exact} exact, {pow_cases} with V8's pow"
    );
    for c in doc["labels"].as_array().unwrap() {
        let (v, step) = (f(&c["v"]), f(&c["step"]));
        assert_eq!(
            ticks::label(v, step),
            c["label"].as_str().unwrap(),
            "tickLabel({v}, {step})"
        );
    }
}

/// `heads.json` cases: `{ pts: [x0, y0, x1, y1, …], vp: [left, top, width,
/// height], L, maxHeads, minGap, backward, placed: [x, y, …],
/// triangles: [[tipX, tipY, c1X, c1Y, c2X, c2Y], …], placedAfter: […] }`.
#[test]
fn heads_match_draw_heads() {
    let Some(doc) = vector("heads.json") else {
        eprintln!("tests/vectors/heads.json not generated yet: skipping");
        return;
    };
    let nums = |v: &Value| -> Vec<f64> { v.as_array().unwrap().iter().map(f).collect() };
    let pairs = |v: Vec<f64>| -> Vec<(f64, f64)> { v.chunks(2).map(|c| (c[0], c[1])).collect() };
    let cases = doc["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for (i, c) in cases.iter().enumerate() {
        let vp = nums(&c["vp"]);
        let vp = Viewport {
            left: vp[0],
            top: vp[1],
            width: vp[2],
            height: vp[3],
            x_range: (0.0, 1.0),
            y_range: (0.0, 1.0),
        };
        let pts = pairs(nums(&c["pts"]));
        let mut placed = pairs(nums(&c["placed"]));
        let l = f(&c["L"]);
        let out = heads::place(
            &pts,
            &vp,
            c["backward"].as_bool().unwrap_or(false),
            l,
            c["maxHeads"].as_u64().unwrap() as usize,
            &mut placed,
            f(&c["minGap"]),
        );
        let want: Vec<Vec<f64>> = c["triangles"]
            .as_array()
            .unwrap()
            .iter()
            .map(nums)
            .collect();
        assert_eq!(out.len(), want.len(), "case {i}: number of heads");
        for (h, w) in out.iter().zip(&want) {
            let tri = h.triangle(l);
            let got = [tri[0].0, tri[0].1, tri[1].0, tri[1].1, tri[2].0, tri[2].1];
            for (a, b) in got.iter().zip(w) {
                assert!(
                    (a - b).abs() <= 1e-9 * a.abs().max(1.0),
                    "case {i}: {got:?} vs {w:?}"
                );
            }
        }
        if !c["placedAfter"].is_null() {
            assert_eq!(placed, pairs(nums(&c["placedAfter"])), "case {i}: placed");
        }
    }
}

/// `cobweb.json` cases (written by the UI's `cobweb.test.mjs` from
/// `app/src/canvas/cobweb.ts`): `{ x: [v0, v1, …], xRange, yRange,
/// staircase: [x, y, …], diagonal: [ax, ay, bx, by] | null }`.
#[test]
fn cobweb_geometry_matches_the_screen() {
    use separatrix_export::cobweb;
    let Some(doc) = vector("cobweb.json") else {
        eprintln!("tests/vectors/cobweb.json not generated yet: skipping");
        return;
    };
    let nums = |v: &Value| -> Vec<f64> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap_or(f64::NAN))
            .collect()
    };
    let range = |v: &Value| {
        let r = nums(v);
        (r[0], r[1])
    };
    let cases = doc["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for (i, c) in cases.iter().enumerate() {
        let got: Vec<f64> = cobweb::staircase(&nums(&c["x"]))
            .into_iter()
            .flat_map(|(a, b)| [a, b])
            .collect();
        assert_eq!(got, nums(&c["staircase"]), "case {i}: staircase");
        let diagonal = cobweb::diagonal(range(&c["xRange"]), range(&c["yRange"]))
            .map(|((ax, ay), (bx, by))| vec![ax, ay, bx, by]);
        let want = (!c["diagonal"].is_null()).then(|| nums(&c["diagonal"]));
        assert_eq!(diagonal, want, "case {i}: diagonal");
    }
}
