//! Round trips, unknown-field preservation, repairs and malformed files.

mod common;

use common::{fixture, APP};
use separatrix_project::{
    from_value_checked, parse, parse_named, to_json, Opened, ProjectAxis, ProjectError,
    ProjectFile, CURRENT_VERSION, LYAPUNOV_MISFIT, MAX_FILE_BYTES, ORBIT_MISFIT, SECTION_MISFIT,
    SPACE_MISFIT,
};
use serde_json::{json, Value};

const GOOD: [&str; 11] = [
    "rabbits-vs-sheep.sepx",
    "cobweb.sepx",
    "section-view.sepx",
    "space-view.sepx",
    "lyapunov-view.sepx",
    "orbit-view.sepx",
    "continuation-view.sepx",
    "float-integers.sepx",
    "lorenz-text-mode.sepx",
    "unknown-fields.sepx",
    "future-v2-readable.sepx",
];

/// Fixtures stored exactly as this build writes them.
const CANONICAL: [&str; 8] = [
    "rabbits-vs-sheep.sepx",
    "lorenz-text-mode.sepx",
    "cobweb.sepx",
    "section-view.sepx",
    "space-view.sepx",
    "lyapunov-view.sepx",
    "orbit-view.sepx",
    "continuation-view.sepx",
];

fn open(name: &str) -> Opened {
    parse_named(&fixture(name), name).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn canonical_fixtures_are_written_back_byte_for_byte() {
    let update = std::env::var_os("UPDATE_FIXTURES").is_some();
    for name in CANONICAL {
        let text = fixture(name);
        let opened = open(name);
        assert!(opened.warnings.is_empty(), "{name}: {:?}", opened.warnings);
        let out = to_json(&opened.project, APP).unwrap();
        if update {
            std::fs::write(common::fixture_path(name), &out).unwrap();
        } else {
            assert_eq!(
                out, text,
                "{name} is not in canonical form (UPDATE_FIXTURES=1 rewrites it)"
            );
        }
    }
}

#[test]
fn every_good_fixture_round_trips_and_a_second_write_is_identical() {
    for name in GOOD {
        let first = open(name);
        let a = to_json(&first.project, APP).unwrap();
        let second = parse(&a).unwrap();
        let mut expected = first.project.clone();
        expected.app_version = APP.into();
        assert_eq!(second.project, expected, "{name}");
        let b = to_json(&second.project, APP).unwrap();
        assert_eq!(a, b, "{name}: the second write differs");
        // Opening what we wrote needs no repairs.
        let repairs: Vec<_> = second
            .warnings
            .iter()
            .filter(|w| !w.contains("newer format"))
            .collect();
        assert!(repairs.is_empty(), "{name}: {repairs:?}");
    }
}

#[test]
fn floats_round_trip_exactly() {
    // Values the default (non-`float_roundtrip`) parser can get 1 ULP wrong.
    let tricky = [
        0.1 + 0.2,
        2.6666666666666665,
        1.0 / 3.0,
        1e-7,
        2.2250738585072014e-308,
        5e-324,
        1.7976931348623157e308,
        0.30000000000000004,
    ];
    let mut p = open("rabbits-vs-sheep.sepx").project;
    p.initial_conditions[0].x0 = vec![tricky[0], tricky[1]];
    p.views[0].slice.states = tricky
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("s{i}"), *v))
        .collect();
    let text = to_json(&p, APP).unwrap();
    let back = parse(&text).unwrap().project;
    for (i, v) in tricky.iter().enumerate() {
        assert_eq!(
            back.views[0].slice.states[&format!("s{i}")].to_bits(),
            v.to_bits()
        );
    }
    assert_eq!(back, p);
    assert_eq!(to_json(&back, APP).unwrap(), text);
}

#[test]
fn output_layout_is_pretty_lf_with_a_trailing_newline() {
    let out = to_json(&open("lorenz-text-mode.sepx").project, APP).unwrap();
    assert!(out.ends_with("}\n") && !out.ends_with("\n\n"));
    assert!(!out.contains('\r'));
    assert!(out.starts_with("{\n  \"format\": \"separatrix\",\n  \"format_version\": 1,\n"));
    assert!(!out.contains('\t'));
    assert!(!out.lines().any(|l| l.ends_with(' ')));
    // Non-ASCII stays UTF-8, not \u escapes.
    let mut p = open("lorenz-text-mode.sepx").project;
    p.system.lines[0] = "\u{1e8b} = \u{3c3}(y \u{2212} x)".into();
    let out = to_json(&p, APP).unwrap();
    assert!(out.contains("\u{1e8b} = \u{3c3}(y \u{2212} x)"));
}

#[test]
fn blank_lines_and_text_mode_are_kept_as_typed() {
    let p = open("lorenz-text-mode.sepx").project;
    assert_eq!(p.system.input_format, "text");
    assert_eq!(
        p.system.lines,
        [
            "x' = sigma*(y - x)",
            "y' = r*x - y - x*z",
            "",
            "z' = x*y - b*z"
        ]
    );
    assert_eq!(p.system.states, ["x", "y", "z"]);
    assert_eq!(p.system.params[2].value, 2.6666666666666665);
    assert!(p.initial_conditions[1].hidden);
    assert!(!p.views[0].layers.backward);
    assert_eq!(p.views[0].slice.states["y"], 0.0);
    assert_eq!(p.solver.t_end, 50.0);
}

#[test]
fn export_settings_are_opaque_and_kept() {
    let text = fixture("lorenz-text-mode.sepx");
    let p = parse(&text).unwrap().project;
    assert_eq!(
        p.export_settings,
        value(&text).get("export_settings").cloned()
    );
    let out = value(&to_json(&p, APP).unwrap());
    assert_eq!(out["export_settings"], value(&text)["export_settings"]);
    // Absent stays absent.
    let rabbits = to_json(&open("rabbits-vs-sheep.sepx").project, APP).unwrap();
    assert!(!rabbits.contains("export_settings"));
}

/// Every key/value of `input` appears in `output`: objects may gain keys
/// (defaults), arrays keep their length, scalars are equal. `skip` lists
/// paths whose value is expected to change.
fn assert_kept(input: &Value, output: &Value, path: &str, skip: &[&str]) {
    if skip.contains(&path) {
        return;
    }
    match (input, output) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                let Some(w) = b.get(k) else {
                    panic!("{p} was dropped")
                };
                assert_kept(v, w, &p, skip);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: length changed");
            for (i, (v, w)) in a.iter().zip(b).enumerate() {
                assert_kept(v, w, &format!("{path}[{i}]"), skip);
            }
        }
        (a, b) => assert!(
            a == b || (a.is_number() && b.is_number() && a.as_f64() == b.as_f64()),
            "{path}: {a} became {b}"
        ),
    }
}

#[test]
fn unknown_fields_are_kept_at_every_level() {
    let text = fixture("unknown-fields.sepx");
    let opened = open("unknown-fields.sepx");
    let out = value(&to_json(&opened.project, APP).unwrap());
    assert_kept(&value(&text), &out, "", &["solver.method"]);
    // Spot checks at each level.
    assert_eq!(out["assignment"], Value::Null);
    assert_eq!(
        out["zz_top_level"],
        json!({ "alpha": [3, { "b": null, "y": true }], "zeta": 1 })
    );
    assert_eq!(out["system"]["notes"], "Strogatz \u{a7}6.4, competition");
    assert_eq!(out["system"]["params"][0]["unit"], "per year");
    assert_eq!(
        out["initial_conditions"][1]["style"],
        json!({ "width": 2, "dash": [1, 2] })
    );
    assert_eq!(out["views"][0]["title"], "Phase plane");
    assert_eq!(out["views"][0]["x_axis"]["scale"], "linear");
    assert_eq!(out["views"][0]["slice"]["r"], 1.5);
    assert_eq!(out["views"][0]["layers"]["cobweb"], false);
    assert_eq!(
        out["views"][1]["x_axis"],
        json!({ "kind": "parameter", "name": "k", "param": "params.k" })
    );
    assert_eq!(
        out["views"][1]["branches"][0]["points"],
        json!([[0, 0], [1, 1]])
    );
    assert_eq!(
        out["analyses"],
        json!([{ "kind": "lyapunov", "id": "lyap-1", "settings": { "n": 1000 } }])
    );
    assert_eq!(out["solver"]["events"], json!([{ "when": "x = 0" }]));
}

#[test]
fn unknown_keys_come_after_known_ones_sorted() {
    let out = to_json(&open("unknown-fields.sepx").project, APP).unwrap();
    let at = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing"))
    };
    // Top level: known keys in struct order, then extras sorted.
    let order = [
        "\n  \"format\"",
        "\n  \"system\"",
        "\n  \"initial_conditions\"",
        "\n  \"views\"",
        "\n  \"analyses\"",
        "\n  \"solver\"",
        "\n  \"export_settings\"",
        "\n  \"assignment\"",
        "\n  \"zz_top_level\"",
    ];
    for pair in order.windows(2) {
        assert!(
            at(pair[0]) < at(pair[1]),
            "{} should come before {}",
            pair[0],
            pair[1]
        );
    }
    // Nested unknown objects come back with their keys sorted (value-level).
    assert!(at("\"alpha\"") < at("\"zeta\""));
    assert!(at("\"b\": null") < at("\"y\": true"));
}

#[test]
fn assignment_null_and_object_survive() {
    let null_text = fixture("unknown-fields.sepx");
    assert!(null_text.contains("\"assignment\": null"));
    let out = to_json(&parse(&null_text).unwrap().project, APP).unwrap();
    assert!(out.contains("\"assignment\": null"));

    let mut v = value(&null_text);
    let assignment = json!({
        "id": "hw-3",
        "due": "2026-12-01",
        "tasks": [{ "target": "params.k", "prompt": "Find where the fixed points collide.", "points": 2 }],
        "nested": { "deep": { "deeper": [null, false, 1.25] } }
    });
    v["assignment"] = assignment.clone();
    let p = parse(&v.to_string()).unwrap().project;
    assert_eq!(p.extra["assignment"], assignment);
    let out = value(&to_json(&p, APP).unwrap());
    assert_eq!(out["assignment"], assignment);
}

#[test]
fn second_view_with_unknown_axis_kind_and_unknown_method_open_with_a_warning() {
    let opened = open("unknown-fields.sepx");
    let p = &opened.project;
    assert_eq!(p.views[1].x_axis.kind, "parameter");
    assert_eq!(p.views[1].x_axis.name.as_deref(), Some("k"));
    assert_eq!(p.solver.method, "dopri5");
    assert_eq!(
        opened.warnings,
        ["The solver method \u{201c}radau5\u{201d} isn't available in this version of Separatrix, so Dormand\u{2013}Prince 5(4) is used."]
    );
}

#[test]
fn unknown_axis_kind_in_the_primary_view_warns() {
    let mut v = value(&fixture("rabbits-vs-sheep.sepx"));
    v["views"][0]["x_axis"] = json!({ "kind": "polar", "name": "r" });
    v["views"][0]["y_axis"] = json!({ "kind": "state", "name": "w" });
    let opened = parse(&v.to_string()).unwrap();
    assert_eq!(
        opened.warnings,
        [
            "The x axis of the view \u{201c}main\u{201d} uses an axis kind (\u{201c}polar\u{201d}) this version doesn't know, so the default view is used.",
            "The y axis of the view \u{201c}main\u{201d} uses the state w, which the system doesn't have, so the default view is used.",
        ]
    );
    // Kept as written.
    let out = value(&to_json(&opened.project, APP).unwrap());
    assert_eq!(
        out["views"][0]["x_axis"],
        json!({ "kind": "polar", "name": "r" })
    );
}

#[test]
fn time_axis_has_no_name() {
    let mut p = open("rabbits-vs-sheep.sepx").project;
    p.views[0].x_axis = ProjectAxis::time();
    let out = value(&to_json(&p, APP).unwrap());
    assert_eq!(out["views"][0]["x_axis"], json!({ "kind": "time" }));
    assert_eq!(
        ProjectAxis::state("x"),
        parse(&fixture("rabbits-vs-sheep.sepx"))
            .unwrap()
            .project
            .views[0]
            .x_axis
    );
}

#[test]
fn a_minimal_file_opens_with_defaults() {
    let text = r#"{ "format": "separatrix", "format_version": 1,
        "system": { "input_format": "latex", "lines": ["\\dot{x}=x"] } }"#;
    let opened = parse(text).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let p = opened.project;
    assert_eq!(
        p.min_reader_version, 1,
        "a missing min_reader_version is the format version"
    );
    assert_eq!(p.app_version, "");
    assert!(p.initial_conditions.is_empty() && p.views.is_empty() && p.analyses.is_empty());
    assert_eq!(p.system.template, None);
    assert_eq!(p.solver.method, "dopri5");
    assert_eq!(
        (
            p.solver.rtol,
            p.solver.atol,
            p.solver.max_steps,
            p.solver.t_end
        ),
        (1e-6, 1e-9, 200_000, 20.0)
    );
    let out = value(&to_json(&p, APP).unwrap());
    assert_eq!(out["system"]["template"], Value::Null);
    assert_eq!(out["app_version"], APP);
    assert_eq!(out["solver"]["max_steps"], 200_000);
}

#[test]
fn a_utf8_bom_is_ignored() {
    let text = format!("\u{feff}{}", fixture("rabbits-vs-sheep.sepx"));
    assert_eq!(
        parse(&text).unwrap().project,
        open("rabbits-vs-sheep.sepx").project
    );
}

#[test]
fn bad_ranges_warn_and_the_ui_uses_the_default_view() {
    let opened = open("bad-range.sepx");
    assert_eq!(
        opened.warnings,
        [
            "The x range of the view \u{201c}main\u{201d} (3.5 to \u{2212}0.25) is empty, so the default view is used.",
            "The y range of the view \u{201c}main\u{201d} (1 to 1) is empty, so the default view is used.",
        ]
    );
    assert!(!separatrix_project::range_ok(
        &opened.project.views[0].x_range
    ));
    assert!(separatrix_project::range_ok(&[-1.0, 1.0]));
    assert!(!separatrix_project::range_ok(&[f64::NEG_INFINITY, 1.0]));
}

#[test]
fn repairs_ids_ranges_colors_solver_grid_and_x0() {
    let mut v = value(&fixture("rabbits-vs-sheep.sepx"));
    v["system"]["params"] = json!([
        { "id": "a", "value": 3, "min": 5, "max": 1 },
        { "id": "a", "value": -0.3 },
        { "id": "c", "value": 2, "min": 0, "max": 6 }
    ]);
    v["initial_conditions"] = json!([
        { "id": "ic-3", "x0": [1, 2] },
        { "id": "ic-3", "x0": [1, 2, 3], "color": 9 },
        { "id": "", "x0": [4], "color": -1 }
    ]);
    v["views"][0]["layers"]["field_grid"] = json!(500);
    v["views"].as_array_mut().unwrap().push(json!({
        "id": "main", "x_axis": { "kind": "time" }, "y_axis": { "kind": "state", "name": "x" },
        "x_range": [0, 10], "y_range": [0, 1], "layers": { "field_grid": 1 }
    }));
    v["solver"] = json!({ "method": "rk4", "rtol": 0, "atol": -1, "h": -0.1, "max_steps": 0,
        "t_end": 0, "samples": 300000 });
    let opened = parse(&v.to_string()).unwrap();
    let p = &opened.project;
    let w = &opened.warnings;

    assert_eq!(p.system.params[1].id, "a-2");
    assert_eq!(
        (p.system.params[0].min, p.system.params[0].max),
        (0.0, 10.0)
    );
    assert_eq!(
        (p.system.params[1].min, p.system.params[1].max),
        (-1.0, 0.0)
    );
    assert_eq!((p.system.params[2].min, p.system.params[2].max), (0.0, 6.0));

    let ids: Vec<_> = p
        .initial_conditions
        .iter()
        .map(|ic| ic.id.as_str())
        .collect();
    assert_eq!(ids, ["ic-3", "ic-3-2", "ic-3-3"]);
    let colors: Vec<_> = p.initial_conditions.iter().map(|ic| ic.color).collect();
    assert_eq!(colors, [0, 2, 6]);
    assert_eq!(p.initial_conditions[1].x0, [1.0, 2.0]);
    assert_eq!(p.initial_conditions[2].x0, [4.0, 0.0]);

    assert_eq!(p.views[0].layers.field_grid, 200);
    assert_eq!(p.views[1].id, "main-2");
    assert_eq!(p.views[1].layers.field_grid, 2);

    let s = &p.solver;
    assert_eq!(s.method, "rk4");
    assert_eq!(
        (s.rtol, s.atol, s.h, s.max_steps, s.t_end, s.samples),
        (1e-6, 1e-9, 0.0, 1, 20.0, 200_000)
    );

    let expected = [
        "The parameter id \u{201c}a\u{201d} appears more than once; this one is now \u{201c}a-2\u{201d}.",
        "The initial condition id \u{201c}ic-3\u{201d} appears more than once; this one is now \u{201c}ic-3-2\u{201d}.",
        "An initial condition had no id; it is now \u{201c}ic-3-3\u{201d}.",
        "The view id \u{201c}main\u{201d} appears more than once; this one is now \u{201c}main-2\u{201d}.",
        "The slider range for a was 5 to 1 (it needs min < max), so it is now 0 to 10.",
        "The slider range for a-2 was missing, so it is now \u{2212}1 to 0.",
        "Initial condition \u{201c}ic-3-2\u{201d} had color 9; it now uses color 2.",
        "Initial condition \u{201c}ic-3-2\u{201d} had 3 values for 2 states (x, y); the extra ones were dropped.",
        "Initial condition \u{201c}ic-3-3\u{201d} had color \u{2212}1; it now uses color 6.",
        "Initial condition \u{201c}ic-3-3\u{201d} had 1 value for 2 states (x, y); the missing ones are 0.",
        "The field grid of the view \u{201c}main\u{201d} was 500, so it is now 200.",
        "The field grid of the view \u{201c}main-2\u{201d} was 1, so it is now 2.",
        "The solver's relative tolerance was 0, so it is now 1e\u{2212}6.",
        "The solver's absolute tolerance was \u{2212}1, so it is now 1e\u{2212}9.",
        "The solver's step size was \u{2212}0.1, so it is now 0 (the method's default).",
        "The solver's maximum number of steps was 0, so it is now 1.",
        "The solver's end time was 0, so it is now 20.",
        "The solver's number of samples was 300000, so it is now 200000.",
    ];
    assert_eq!(w, &expected);

    // The repaired project opens again without warnings.
    let again = parse(&to_json(p, APP).unwrap()).unwrap();
    assert!(again.warnings.is_empty(), "{:?}", again.warnings);
}

#[test]
fn x0_is_not_checked_without_states() {
    let mut v = value(&fixture("rabbits-vs-sheep.sepx"));
    v["system"]["states"] = json!([]);
    v["initial_conditions"][0]["x0"] = json!([1, 2, 3]);
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings.is_empty());
    assert_eq!(opened.project.initial_conditions[0].x0.len(), 3);
}

#[test]
fn primary_view_is_main_otherwise_the_first() {
    let mut v = value(&fixture("rabbits-vs-sheep.sepx"));
    let main = v["views"][0].clone();
    let mut other = main.clone();
    other["id"] = json!("zoom");
    other["x_range"] = json!([1, 0]);
    v["views"] = json!([other.clone(), main]);
    let opened = parse(&v.to_string()).unwrap();
    assert_eq!(
        separatrix_project::primary_view_index(&opened.project.views),
        Some(1)
    );
    // The bad range is in a non-primary view, kept verbatim, no warning.
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(opened.project.views[0].x_range, [1.0, 0.0]);
    v["views"] = json!([other]);
    let opened = parse(&v.to_string()).unwrap();
    assert_eq!(
        separatrix_project::primary_view_index(&opened.project.views),
        Some(0)
    );
    assert_eq!(opened.warnings.len(), 1);
    assert_eq!(separatrix_project::primary_view_index(&[]), None);
}

// ---- malformed files ----

#[test]
fn not_json_reports_line_and_column() {
    let err = parse_named(&fixture("not-json.sepx"), "not-json.sepx").unwrap_err();
    let ProjectError::NotJson { line, column, .. } = err else {
        panic!("{err:?}")
    };
    assert!(line > 1 && column > 0);
    assert_eq!(
        err.to_string(),
        format!(
            "\u{201c}not-json.sepx\u{201d} isn't a Separatrix project: it isn't valid JSON (line {line}, column {column}). If you edited it by hand, fix it there, or open a backup copy."
        )
    );
    assert!(matches!(parse(""), Err(ProjectError::NotJson { .. })));
    assert!(matches!(
        parse("{\"format\": \"separatrix\",}"),
        Err(ProjectError::NotJson { .. })
    ));
}

#[test]
fn deep_nesting_hits_the_recursion_limit_not_the_stack() {
    let deep = format!(
        "{{\"format\":\"separatrix\",\"format_version\":1,\"x\":{}{}}}",
        "[".repeat(10_000),
        "]".repeat(10_000)
    );
    assert!(matches!(parse(&deep), Err(ProjectError::NotJson { .. })));
}

#[test]
fn other_formats_are_not_projects() {
    let err = parse_named(&fixture("other-format.json"), "other-format.json").unwrap_err();
    assert_eq!(
        err,
        ProjectError::NotProject {
            file: "other-format.json".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "\u{201c}other-format.json\u{201d} isn't a Separatrix project. Open a .sepx file saved by Separatrix."
    );
    for text in [
        "[]",
        "3",
        "null",
        "\"separatrix\"",
        "{}",
        "{\"format\": 1}",
        "{\"format\": \"Separatrix\"}",
    ] {
        assert!(
            matches!(parse(text), Err(ProjectError::NotProject { .. })),
            "{text}"
        );
    }
}

#[test]
fn bad_versions() {
    for (fv, extra, detail) in [
        (
            "0",
            "",
            "format_version should be a whole number from 1 to 4294967295, not 0",
        ),
        (
            "-1",
            "",
            "format_version should be a whole number from 1 to 4294967295, not -1",
        ),
        (
            "1.5",
            "",
            "format_version should be a whole number from 1 to 4294967295, not 1.5",
        ),
        (
            "\"1\"",
            "",
            "format_version should be a whole number from 1 to 4294967295, not \"1\"",
        ),
        (
            "null",
            "",
            "format_version should be a whole number from 1 to 4294967295, not null",
        ),
        (
            "1",
            ", \"min_reader_version\": 0",
            "min_reader_version should be a whole number from 1 to 4294967295, not 0",
        ),
        (
            "1",
            ", \"min_reader_version\": \"1\"",
            "min_reader_version should be a whole number from 1 to 4294967295, not \"1\"",
        ),
    ] {
        let text = format!(
            "{{\"format\": \"separatrix\", \"format_version\": {fv}{extra}, \"system\": {{\"input_format\": \"latex\", \"lines\": []}}}}"
        );
        let err = parse_named(&text, "v.sepx").unwrap_err();
        assert_eq!(
            err,
            ProjectError::BadVersion {
                file: "v.sepx".into(),
                detail: detail.into()
            },
            "{text}"
        );
    }
    // Too large for the format's 32-bit version: not clamped, refused.
    let text =
        "{\"format\": \"separatrix\", \"format_version\": 4294967296, \"min_reader_version\": 1}";
    assert_eq!(
        parse_named(text, "v.sepx").unwrap_err(),
        ProjectError::BadVersion {
            file: "v.sepx".into(),
            detail: "format_version should be a whole number from 1 to 4294967295, not 4294967296"
                .into()
        }
    );
    let err = parse("{\"format\": \"separatrix\"}").unwrap_err();
    assert_eq!(
        err.to_string(),
        "This project has an invalid format version: format_version is missing. The file may be damaged; open a backup copy."
    );
}

#[test]
fn damaged_types_name_the_path() {
    let err = parse_named(&fixture("damaged-type.sepx"), "damaged-type.sepx").unwrap_err();
    assert_eq!(
        err,
        ProjectError::Damaged {
            file: "damaged-type.sepx".into(),
            detail: "initial_conditions[1].x0[1] should be a number, not \"2.4\"".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "\u{201c}damaged-type.sepx\u{201d} is damaged: initial_conditions[1].x0[1] should be a number, not \"2.4\". Fix it in a text editor, or open a backup copy."
    );
}

#[test]
fn structure_errors_on_open() {
    let base = value(&fixture("rabbits-vs-sheep.sepx"));
    let cases: [(&str, Value, &str); 6] = [
        (
            "/system/lines",
            Value::Null,
            "system.lines should be a list, not null",
        ),
        (
            "/system/input_format",
            json!("markdown"),
            "system.input_format should be \"latex\" or \"text\", not \"markdown\"",
        ),
        (
            "/views/0/x_axis",
            json!("x"),
            "views[0].x_axis should be an object, not \"x\"",
        ),
        (
            "/system/params",
            json!([{ "value": 1 }]),
            "system.params[0].id is missing",
        ),
        (
            "/initial_conditions/0/hidden",
            json!("no"),
            "initial_conditions[0].hidden should be true or false, not \"no\"",
        ),
        (
            "/solver/samples",
            json!(1.5),
            "solver.samples should be a whole number from 0 to 4294967295, not 1.5",
        ),
    ];
    for (pointer, bad, detail) in cases {
        let mut v = base.clone();
        *v.pointer_mut(pointer).unwrap() = bad;
        let err = parse(&v.to_string()).unwrap_err();
        assert_eq!(
            err,
            ProjectError::Damaged {
                file: String::new(),
                detail: detail.into()
            },
            "{pointer}"
        );
    }
    let mut v = base.clone();
    v["system"].as_object_mut().unwrap().remove("lines");
    assert_eq!(
        parse(&v.to_string()).unwrap_err(),
        ProjectError::Damaged {
            file: String::new(),
            detail: "system.lines is missing".into()
        }
    );
    let mut v = base;
    v.as_object_mut().unwrap().remove("system");
    assert!(matches!(
        parse(&v.to_string()),
        Err(ProjectError::Damaged { .. })
    ));
}

#[test]
fn numbers_too_large_for_f64_are_not_json() {
    let text = fixture("rabbits-vs-sheep.sepx").replace("12.0", "1e999");
    assert!(matches!(parse(&text), Err(ProjectError::NotJson { .. })));
}

#[test]
fn too_large_text_is_refused() {
    let mut text = fixture("rabbits-vs-sheep.sepx");
    text.push_str(&" ".repeat(MAX_FILE_BYTES as usize));
    let err = parse_named(&text, "big.sepx").unwrap_err();
    assert!(matches!(err, ProjectError::TooLarge { .. }));
    assert!(
        err.to_string()
            .starts_with("\u{201c}big.sepx\u{201d} is 4.0 MB, which is too large"),
        "{err}"
    );
    assert!(
        err.to_string()
            .ends_with("(the limit is 4 MB). Check that you picked the right file."),
        "{err}"
    );
}

// ---- the write path ----

fn ui_value() -> Value {
    serde_json::to_value(open("rabbits-vs-sheep.sepx").project).unwrap()
}

#[test]
fn from_value_checked_accepts_what_the_ui_sends() {
    let p = from_value_checked(ui_value()).unwrap();
    assert_eq!(p, open("rabbits-vs-sheep.sepx").project);
    // The version fields may be left out; to_json stamps them.
    let mut v = ui_value();
    for k in [
        "format",
        "format_version",
        "min_reader_version",
        "app_version",
    ] {
        v.as_object_mut().unwrap().remove(k);
    }
    let p = from_value_checked(v).unwrap();
    assert_eq!(to_json(&p, APP).unwrap(), fixture("rabbits-vs-sheep.sepx"));
}

#[test]
fn from_value_checked_names_a_nan_that_became_null() {
    // JSON.stringify(NaN) is "null".
    let mut v = ui_value();
    v["initial_conditions"][2]["x0"][1] = Value::Null;
    let err = from_value_checked(v).unwrap_err();
    assert_eq!(
        err,
        ProjectError::Unsaveable {
            detail: "initial_conditions[2].x0[1] should be a number, not null".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "Couldn't save the project: initial_conditions[2].x0[1] should be a number, not null. Change that value, then save again."
    );
    let mut v = ui_value();
    v["views"][0]["y_range"][0] = Value::Null;
    assert_eq!(
        from_value_checked(v).unwrap_err(),
        ProjectError::Unsaveable {
            detail: "views[0].y_range[0] should be a number, not null".into()
        }
    );
    let mut v = ui_value();
    v["solver"]["t_end"] = Value::Null;
    assert_eq!(
        from_value_checked(v).unwrap_err(),
        ProjectError::Unsaveable {
            detail: "solver.t_end should be a number, not null".into()
        }
    );
    assert_eq!(
        from_value_checked(json!([])).unwrap_err(),
        ProjectError::Unsaveable {
            detail: "the project should be a JSON object, not a list".into()
        }
    );
}

#[test]
fn to_json_refuses_non_finite_numbers() {
    let base = open("lorenz-text-mode.sepx").project;
    type Breaker = fn(&mut ProjectFile);
    let cases: [(Breaker, &str); 5] = [
        (
            |p| p.initial_conditions[1].x0[2] = f64::NAN,
            "initial_conditions[1].x0[2]",
        ),
        (
            |p| p.system.params[0].max = f64::INFINITY,
            "system.params[0].max",
        ),
        (
            |p| p.views[0].x_range[1] = f64::NEG_INFINITY,
            "views[0].x_range[1]",
        ),
        (
            |p| *p.views[0].slice.states.get_mut("y").unwrap() = f64::NAN,
            "views[0].slice.states.y",
        ),
        (|p| p.solver.rtol = f64::NAN, "solver.rtol"),
    ];
    for (break_it, path) in cases {
        let mut p = base.clone();
        break_it(&mut p);
        assert_eq!(
            to_json(&p, APP).unwrap_err(),
            ProjectError::Unsaveable {
                detail: format!("{path} isn't a finite number")
            }
        );
    }
    // A missing slider bound from the UI reads as NaN and is refused too.
    let mut v = serde_json::to_value(&base).unwrap();
    v["system"]["params"][0]
        .as_object_mut()
        .unwrap()
        .remove("min");
    let p = from_value_checked(v).unwrap();
    assert_eq!(
        to_json(&p, APP).unwrap_err(),
        ProjectError::Unsaveable {
            detail: "system.params[0].min isn't a finite number".into()
        }
    );
}

#[test]
fn to_json_stamps_the_version_fields() {
    let mut p = open("rabbits-vs-sheep.sepx").project;
    p.format = "something".into();
    p.format_version = 0;
    p.min_reader_version = 7;
    p.app_version = "0.0.1".into();
    let out = value(&to_json(&p, "9.9.9").unwrap());
    assert_eq!(out["format"], "separatrix");
    assert_eq!(out["format_version"], CURRENT_VERSION);
    assert_eq!(out["min_reader_version"], 1);
    assert_eq!(out["app_version"], "9.9.9");
}

#[test]
fn whole_numbers_written_as_floats_open_and_save_as_integers() {
    // As Python's `json` or a spreadsheet exporter writes them.
    let opened = open("float-integers.sepx");
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let p = &opened.project;
    assert_eq!(p.format_version, 1);
    assert_eq!(p.initial_conditions[0].color, 3);
    assert_eq!(p.views[0].layers.field_grid, 21);
    assert_eq!(p.solver.max_steps, 100_000);
    let out = value(&to_json(p, APP).unwrap());
    assert_eq!(out["solver"]["max_steps"], json!(100000));
    assert!(out["views"][0]["layers"]["field_grid"].is_u64());
    // On save too: the UI may send 21.0.
    let mut v = value(&fixture("float-integers.sepx"));
    v["solver"]["samples"] = json!(500.0);
    assert_eq!(from_value_checked(v).unwrap().solver.samples, 500);
}

// ---------------------------------------------------------------------
// Cobweb views (`views[].kind`, the `next` axis) and `solver.transient`.
// ---------------------------------------------------------------------

const COBWEB_MISFIT: &str =
    "The saved cobweb view doesn't fit this system, so the default view is shown.";

#[test]
fn a_cobweb_view_and_its_transient_are_read_and_written_back() {
    let opened = open("cobweb.sepx");
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let p = &opened.project;
    let view = &p.views[0];
    assert_eq!(view.kind.as_deref(), Some("cobweb"));
    assert_eq!(view.x_axis, ProjectAxis::state("x"));
    assert_eq!(view.y_axis, ProjectAxis::next("x"));
    assert_eq!(p.solver.transient, Some(200));
    let text = to_json(p, APP).unwrap();
    // `kind` right after `id`, `transient` right after `t_end`.
    assert!(text.contains("\"id\": \"main\",\n      \"kind\": \"cobweb\",\n      \"x_axis\""));
    assert!(text.contains("\"t_end\": 60.0,\n    \"transient\": 200,\n    \"samples\": 0"));
}

#[test]
fn a_plane_view_writes_no_kind_and_no_zero_transient() {
    let mut p = open("rabbits-vs-sheep.sepx").project;
    assert_eq!(p.views[0].kind, None);
    assert_eq!(p.solver.transient, None);
    let before = to_json(&p, APP).unwrap();
    assert!(!before.contains("\"kind\": \"plane\"") && !before.contains("transient"));
    // An explicit "plane" is the plane view; a transient of 0 is not written.
    let mut v = value(&before);
    v["views"][0]["kind"] = json!("plane");
    v["solver"]["transient"] = json!(0);
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(opened.project.solver.transient, None);
    p.views[0].kind = None;
    assert_eq!(to_json(&p, APP).unwrap(), before);
}

#[test]
fn a_transient_is_a_whole_number_clamped_to_a_million() {
    let mut v = value(&fixture("cobweb.sepx"));
    v["solver"]["transient"] = json!(250.0);
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(opened.project.solver.transient, Some(250));
    assert!(to_json(&opened.project, APP)
        .unwrap()
        .contains("\"transient\": 250,"));
    for (bad, now, warning) in [
        (
            json!(5_000_000),
            Some(1_000_000),
            "The solver's transient was 5000000, so it is now 1000000 iterates.",
        ),
        (
            json!(-3),
            None,
            "The solver's transient was \u{2212}3, so it is now 0 iterates.",
        ),
    ] {
        v["solver"]["transient"] = bad;
        let opened = parse(&v.to_string()).unwrap();
        assert_eq!(opened.project.solver.transient, now);
        assert_eq!(opened.warnings, [warning]);
    }
    v["solver"]["transient"] = json!(2.5);
    let err = parse(&v.to_string()).unwrap_err().to_string();
    assert!(
        err.contains("solver.transient should be a whole number"),
        "{err}"
    );
    v["solver"]["transient"] = Value::Null;
    assert_eq!(
        parse(&v.to_string()).unwrap().project.solver.transient,
        None
    );
}

#[test]
fn a_cobweb_view_that_does_not_fit_falls_back_to_the_default_view() {
    let base = value(&fixture("cobweb.sepx"));
    let warns = |edit: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        edit(&mut v);
        parse(&v.to_string()).unwrap().warnings
    };
    // An older build re-saved it with its own default view and kept `kind`.
    let kept_kind = warns(&|v| {
        v["views"][0]["x_axis"] = json!({ "kind": "time" });
        v["views"][0]["y_axis"] = json!({ "kind": "state", "name": "x" });
    });
    assert_eq!(kept_kind, [COBWEB_MISFIT]);
    // `next` without the kind, or for another state.
    assert_eq!(
        warns(&|v| {
            v["views"][0].as_object_mut().unwrap().remove("kind");
        }),
        [COBWEB_MISFIT]
    );
    assert_eq!(
        warns(&|v| v["views"][0]["y_axis"] = json!({ "kind": "next", "name": "y" })),
        [COBWEB_MISFIT]
    );
    // Not a one-dimensional map.
    assert_eq!(
        warns(&|v| {
            v["system"]["states"] = json!(["x", "y"]);
            v["initial_conditions"][0]["x0"] = json!([0.1, 0.0]);
        }),
        [COBWEB_MISFIT]
    );
    assert_eq!(
        warns(&|v| v["system"]["kind"] = json!("ode")),
        [COBWEB_MISFIT]
    );
    // A kind this version doesn't know.
    assert_eq!(
        warns(&|v| v["views"][0]["kind"] = json!("bifurcation")),
        [COBWEB_MISFIT]
    );
    // Kept as written either way.
    let mut v = base.clone();
    v["views"][0]["kind"] = json!("bifurcation");
    let opened = parse(&v.to_string()).unwrap();
    assert_eq!(opened.project.views[0].kind.as_deref(), Some("bifurcation"));
    assert!(to_json(&opened.project, APP)
        .unwrap()
        .contains("\"kind\": \"bifurcation\""));
    // A cobweb in a second view is not checked (v1 edits the primary only).
    let mut v = value(&fixture("rabbits-vs-sheep.sepx"));
    let mut second = base["views"][0].clone();
    second["id"] = json!("other");
    v["views"].as_array_mut().unwrap().push(second);
    assert!(parse(&v.to_string()).unwrap().warnings.is_empty());
}

#[test]
fn a_section_view_is_read_and_written_back() {
    let opened = open("section-view.sepx");
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let view = &opened.project.views[0];
    assert_eq!(view.kind.as_deref(), Some("section"));
    let s = view.section.as_ref().unwrap();
    assert_eq!(
        (
            s.kind.as_str(),
            s.state.as_deref(),
            s.value,
            s.direction.as_deref()
        ),
        ("plane", Some("z"), Some(27.0), Some("rising"))
    );
    assert_eq!((s.period, s.phase), (None, None));
    assert_eq!(opened.project.solver.transient, Some(10));
    let text = to_json(&opened.project, APP).unwrap();
    assert!(text.contains(
        "\"kind\": \"section\",\n      \"section\": {\n        \"kind\": \"plane\",\n        \"state\": \"z\",\n        \"value\": 27.0,\n        \"direction\": \"rising\"\n      },\n      \"x_axis\""
    ));
}

#[test]
fn stroboscopic_and_return_map_sections_round_trip_identically() {
    let base = value(&fixture("section-view.sepx"));
    let strobe = {
        let mut v = base.clone();
        v["views"][0]["section"] =
            json!({ "kind": "stroboscopic", "period": std::f64::consts::TAU, "phase": 0.5 });
        v
    };
    let return_map = {
        let mut v = base.clone();
        v["views"][0]["y_axis"] = json!({ "kind": "next", "name": "x" });
        v
    };
    let strobe_return_map = {
        let mut v = strobe.clone();
        v["views"][0]["y_axis"] = json!({ "kind": "next", "name": "x" });
        v
    };
    for (name, v) in [
        ("stroboscopic", strobe),
        ("return map", return_map),
        ("stroboscopic return map", strobe_return_map),
    ] {
        let first = parse(&v.to_string()).unwrap();
        assert!(first.warnings.is_empty(), "{name}: {:?}", first.warnings);
        let a = to_json(&first.project, APP).unwrap();
        assert_eq!(value(&a)["views"][0], v["views"][0], "{name}");
        let second = parse(&a).unwrap();
        assert_eq!(to_json(&second.project, APP).unwrap(), a, "{name}");
    }
}

#[test]
fn a_section_view_that_does_not_fit_falls_back_to_the_default_view() {
    let base = value(&fixture("section-view.sepx"));
    let warns = |edit: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        edit(&mut v);
        parse(&v.to_string()).unwrap().warnings
    };
    assert_eq!(
        warns(&|v| v["views"][0]["section"]["kind"] = json!("hyperbola")),
        ["The section of the view \u{201c}main\u{201d} is of a kind (\u{201c}hyperbola\u{201d}) this version doesn't know, so the default view is used."]
    );
    assert_eq!(
        warns(&|v| v["views"][0]["section"]["state"] = json!("w")),
        ["The section of the view \u{201c}main\u{201d} holds the state w fixed, which the system doesn't have, so the default view is used."]
    );
    type Edit<'a> = &'a dyn Fn(&mut Value);
    let misfits: [(&str, Edit); 9] = [
        ("no section", &|v| {
            v["views"][0].as_object_mut().unwrap().remove("section");
        }),
        ("a map", &|v| v["system"]["kind"] = json!("map")),
        ("no value", &|v| {
            v["views"][0]["section"]
                .as_object_mut()
                .unwrap()
                .remove("value");
        }),
        ("an unknown direction", &|v| {
            v["views"][0]["section"]["direction"] = json!("sideways")
        }),
        ("a plane of one state", &|v| {
            v["system"]["states"] = json!(["z"]);
            v["initial_conditions"] = json!([]);
            v["views"][0]["x_axis"] = json!({ "kind": "state", "name": "z" });
            v["views"][0]["y_axis"] = json!({ "kind": "next", "name": "z" });
        }),
        ("a period of 0", &|v| {
            v["views"][0]["section"] = json!({ "kind": "stroboscopic", "period": 0.0 })
        }),
        ("a time axis", &|v| {
            v["views"][0]["x_axis"] = json!({ "kind": "time" })
        }),
        ("one state on both axes", &|v| {
            v["views"][0]["y_axis"] = json!({ "kind": "state", "name": "x" })
        }),
        ("the next crossing of another state", &|v| {
            v["views"][0]["y_axis"] = json!({ "kind": "next", "name": "y" })
        }),
    ];
    for (name, edit) in misfits {
        assert_eq!(warns(edit), [SECTION_MISFIT], "{name}");
    }
    let mut v = base.clone();
    v["views"][0]["section"]["kind"] = json!("hyperbola");
    v["views"][0]["section"]["focus"] = json!([1, 2]);
    let opened = parse(&v.to_string()).unwrap();
    assert_eq!(
        value(&to_json(&opened.project, APP).unwrap())["views"][0]["section"],
        v["views"][0]["section"]
    );
    // JSON can't hold a non-finite section value.
    let mut p = open("section-view.sepx").project;
    p.views[0].section.as_mut().unwrap().value = Some(f64::NAN);
    assert!(to_json(&p, APP)
        .unwrap_err()
        .to_string()
        .contains("views[0].section.value"));
}

#[test]
fn a_space_view_is_read_and_written_back() {
    let opened = open("space-view.sepx");
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let view = &opened.project.views[0];
    assert_eq!(view.kind.as_deref(), Some("space"));
    assert_eq!(view.z_axis, Some(ProjectAxis::state("z")));
    assert_eq!(view.z_range, Some([0.0, 50.0]));
    let c = view.camera.as_ref().unwrap();
    assert_eq!(
        (c.yaw, c.pitch, c.zoom),
        (
            -std::f64::consts::FRAC_PI_4,
            std::f64::consts::PI / 6.0,
            1.0
        )
    );
    let text = to_json(&opened.project, APP).unwrap();
    assert!(text.contains(
        "\"y_axis\": {\n        \"kind\": \"state\",\n        \"name\": \"y\"\n      },\n      \"z_axis\": {"
    ));
    assert!(text.contains(
        "\"z_range\": [\n        0.0,\n        50.0\n      ],\n      \"camera\": {\n        \"yaw\": -0.7853981633974483,"
    ));
    let mut v = value(&fixture("space-view.sepx"));
    v["views"][0]["camera"] = json!({ "zoom": 2.5, "roll": 0.1 });
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let c = opened.project.views[0].camera.as_ref().unwrap();
    assert_eq!(
        (c.yaw, c.pitch, c.zoom),
        (
            -std::f64::consts::FRAC_PI_4,
            std::f64::consts::PI / 6.0,
            2.5
        )
    );
    assert_eq!(
        value(&to_json(&opened.project, APP).unwrap())["views"][0]["camera"]["roll"],
        json!(0.1)
    );
}

#[test]
fn a_space_view_that_does_not_fit_falls_back_to_the_default_view() {
    let base = value(&fixture("space-view.sepx"));
    let warns = |edit: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        edit(&mut v);
        parse(&v.to_string()).unwrap().warnings
    };
    type Edit<'a> = &'a dyn Fn(&mut Value);
    let misfits: [(&str, Edit); 6] = [
        ("a map", &|v| v["system"]["kind"] = json!("map")),
        ("two states", &|v| {
            v["system"]["states"] = json!(["x", "y"]);
            v["initial_conditions"] = json!([]);
        }),
        ("no third axis", &|v| {
            v["views"][0].as_object_mut().unwrap().remove("z_axis");
        }),
        ("no third range", &|v| {
            v["views"][0].as_object_mut().unwrap().remove("z_range");
        }),
        ("a time axis", &|v| {
            v["views"][0]["z_axis"] = json!({ "kind": "time" })
        }),
        ("one state twice", &|v| {
            v["views"][0]["z_axis"] = json!({ "kind": "state", "name": "x" })
        }),
    ];
    for (name, edit) in misfits {
        assert_eq!(warns(edit), [SPACE_MISFIT], "{name}");
    }
    assert_eq!(
        warns(&|v| v["views"][0]["z_axis"]["name"] = json!("w")),
        ["The z axis of the view \u{201c}main\u{201d} uses the state w, which the system doesn't have, so the default view is used."]
    );
    assert_eq!(
        warns(&|v| v["views"][0]["z_range"] = json!([5.0, 5.0])),
        ["The z range of the view \u{201c}main\u{201d} (5 to 5) is empty, so the default view is used."]
    );
    assert!(warns(&|v| {
        v["views"][0].as_object_mut().unwrap().remove("kind");
        v["views"][0]["z_range"] = json!([5.0, 5.0]);
    })
    .is_empty());
    // JSON can't hold a non-finite camera.
    let mut p = open("space-view.sepx").project;
    p.views[0].camera.as_mut().unwrap().zoom = f64::INFINITY;
    assert!(to_json(&p, APP)
        .unwrap_err()
        .to_string()
        .contains("views[0].camera.zoom"));
}

#[test]
fn symplectic_methods_are_known_and_written_back_byte_for_byte() {
    for method in ["verlet", "yoshida4"] {
        let text = fixture("rabbits-vs-sheep.sepx").replace(
            "\"method\": \"dopri5\",\n    \"rtol\": 1e-6,\n    \"atol\": 1e-9,\n    \"h\": 0.0,",
            &format!("\"method\": \"{method}\",\n    \"rtol\": 1e-6,\n    \"atol\": 1e-9,\n    \"h\": 0.05,"),
        );
        assert!(
            text.contains(&format!("\"method\": \"{method}\"")),
            "{method}"
        );
        let opened = parse_named(&text, "symplectic.sepx").unwrap();
        assert!(
            opened.warnings.is_empty(),
            "{method}: {:?}",
            opened.warnings
        );
        assert_eq!(opened.project.solver.method, method);
        assert_eq!(to_json(&opened.project, APP).unwrap(), text, "{method}");
    }
}

#[test]
fn rosenbrock23_is_known_and_written_back_byte_for_byte() {
    let text = fixture("rabbits-vs-sheep.sepx")
        .replace("\"method\": \"dopri5\",", "\"method\": \"rosenbrock23\",");
    assert!(text.contains("\"method\": \"rosenbrock23\""));
    let opened = parse_named(&text, "stiff.sepx").unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(opened.project.solver.method, "rosenbrock23");
    assert_eq!(to_json(&opened.project, APP).unwrap(), text);
}

#[test]
fn a_lyapunov_view_is_read_and_written_back() {
    let opened = open("lyapunov-view.sepx");
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let view = &opened.project.views[0];
    assert_eq!(view.kind.as_deref(), Some("lyapunov"));
    assert_eq!(view.x_axis, ProjectAxis::time());
    assert_eq!(view.y_axis, ProjectAxis::exponent());
    let l = view.lyapunov.as_ref().unwrap();
    assert_eq!(
        (l.transient, l.total, l.interval, l.blocks),
        (Some(100.0), Some(10000.0), Some(0.1), Some(10))
    );
    let text = to_json(&opened.project, APP).unwrap();
    assert!(text.contains(
        "\"y_range\": [\n        -16.0,\n        2.0\n      ],\n      \"lyapunov\": {\n        \"transient\": 100.0,\n        \"total\": 10000.0,\n        \"interval\": 0.1,\n        \"blocks\": 10\n      },\n      \"slice\": {"
    ));
    let mut v = value(&fixture("lyapunov-view.sepx"));
    v["views"][0]["lyapunov"]["method"] = json!("qr-householder");
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(
        value(&to_json(&opened.project, APP).unwrap())["views"][0]["lyapunov"]["method"],
        json!("qr-householder")
    );
}

#[test]
fn a_lyapunov_view_that_cannot_run_falls_back_to_the_default_view() {
    let base = value(&fixture("lyapunov-view.sepx"));
    let warns = |edit: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        edit(&mut v);
        parse(&v.to_string()).unwrap().warnings
    };
    type Edit<'a> = &'a dyn Fn(&mut Value);
    let misfits: [(&str, Edit); 8] = [
        ("no run", &|v| {
            v["views"][0].as_object_mut().unwrap().remove("lyapunov");
        }),
        ("no blocks", &|v| {
            v["views"][0]["lyapunov"]
                .as_object_mut()
                .unwrap()
                .remove("blocks");
        }),
        ("one block", &|v| {
            v["views"][0]["lyapunov"]["blocks"] = json!(1)
        }),
        ("more blocks than QR steps", &|v| {
            v["views"][0]["lyapunov"]["total"] = json!(0.5)
        }),
        ("a negative transient", &|v| {
            v["views"][0]["lyapunov"]["transient"] = json!(-1.0)
        }),
        ("no interval", &|v| {
            v["views"][0]["lyapunov"]["interval"] = json!(0.0)
        }),
        ("a state up the side", &|v| {
            v["views"][0]["y_axis"] = json!({ "kind": "state", "name": "x" })
        }),
        ("half an iterate of a map", &|v| {
            v["system"]["kind"] = json!("map");
            v["views"][0]["lyapunov"]["interval"] = json!(0.5);
        }),
    ];
    for (name, edit) in misfits {
        assert_eq!(warns(edit), [LYAPUNOV_MISFIT], "{name}");
    }
    assert!(warns(&|v| {
        v["system"]["kind"] = json!("map");
        v["views"][0]["lyapunov"]["interval"] = json!(1.0);
    })
    .is_empty());
    assert_eq!(
        warns(&|v| {
            v["views"][0].as_object_mut().unwrap().remove("kind");
        }),
        ["The y axis of the view \u{201c}main\u{201d} uses an axis kind (\u{201c}exponent\u{201d}) this version doesn't know, so the default view is used."]
    );
    let mut v = base.clone();
    v["views"][0]["lyapunov"]["blocks"] = json!("ten");
    assert!(parse(&v.to_string())
        .unwrap_err()
        .to_string()
        .contains("views[0].lyapunov.blocks"));
    let mut p = open("lyapunov-view.sepx").project;
    p.views[0].lyapunov.as_mut().unwrap().total = Some(f64::INFINITY);
    assert!(to_json(&p, APP)
        .unwrap_err()
        .to_string()
        .contains("views[0].lyapunov.total"));
}

#[test]
fn an_orbit_diagram_is_read_and_written_back() {
    let opened = open("orbit-view.sepx");
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let view = &opened.project.views[0];
    assert_eq!(view.kind.as_deref(), Some("orbit"));
    assert_eq!(view.x_axis, ProjectAxis::param("r"));
    assert_eq!(view.y_axis, ProjectAxis::state("x"));
    let o = view.orbit.as_ref().unwrap();
    assert_eq!((o.transient, o.kept), (Some(1000), Some(200)));
    let text = to_json(&opened.project, APP).unwrap();
    assert!(text.contains(
        "\"y_range\": [\n        -0.05,\n        1.05\n      ],\n      \"orbit\": {\n        \"transient\": 1000,\n        \"kept\": 200\n      },\n      \"slice\": {"
    ));
    let mut v = value(&fixture("orbit-view.sepx"));
    v["views"][0]["orbit"]["shading"] = json!("log");
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(
        value(&to_json(&opened.project, APP).unwrap())["views"][0]["orbit"]["shading"],
        json!("log")
    );
}

#[test]
fn an_orbit_diagram_that_cannot_be_drawn_falls_back_to_the_default_view() {
    let base = value(&fixture("orbit-view.sepx"));
    let warns = |edit: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        edit(&mut v);
        parse(&v.to_string()).unwrap().warnings
    };
    type Edit<'a> = &'a dyn Fn(&mut Value);
    let misfits: [(&str, Edit); 7] = [
        ("no columns", &|v| {
            v["views"][0].as_object_mut().unwrap().remove("orbit");
        }),
        ("nothing drawn", &|v| {
            v["views"][0]["orbit"]["kept"] = json!(0)
        }),
        ("more drawn than the widest plot fits", &|v| {
            v["views"][0]["orbit"]["kept"] = json!(977)
        }),
        ("too long a transient", &|v| {
            v["views"][0]["orbit"]["transient"] = json!(100_001)
        }),
        ("a parameter the system lacks", &|v| {
            v["views"][0]["x_axis"] = json!({ "kind": "param", "name": "mu" })
        }),
        ("time up the side", &|v| {
            v["views"][0]["y_axis"] = json!({ "kind": "time" })
        }),
        ("a flow", &|v| v["system"]["kind"] = json!("ode")),
    ];
    for (name, edit) in misfits {
        assert_eq!(warns(edit), [ORBIT_MISFIT], "{name}");
    }
    assert!(warns(&|v| v["views"][0]["orbit"]["kept"] = json!(976)).is_empty());
    let w = warns(&|v| {
        v["views"][0].as_object_mut().unwrap().remove("kind");
    });
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("(\u{201c}param\u{201d})"), "{w:?}");
    let mut v = base.clone();
    v["views"][0]["orbit"]["kept"] = json!(0.5);
    assert!(parse(&v.to_string()).is_err());
}

#[test]
fn continuation_view_round_trips_byte_for_byte() {
    let name = "continuation-view.sepx";
    let opened = open(name);
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let view = &opened.project.views[0];
    assert_eq!(view.kind.as_deref(), Some("continuation"));
    assert_eq!(view.x_axis, ProjectAxis::param("r"));
    assert_eq!(view.y_axis, ProjectAxis::state("x"));
    let c = view.continuation.as_ref().unwrap();
    assert_eq!(
        (c.range, c.ds, c.max_points),
        (Some([2.8, 3.57]), Some(0.05), Some(1000))
    );
    let seeds: Vec<_> = c
        .seeds
        .iter()
        .map(|s| {
            (
                s.from.as_str(),
                s.period,
                s.kind.as_deref(),
                s.tangent.is_some(),
            )
        })
        .collect();
    assert_eq!(
        seeds,
        [
            ("point", 1, None, false),
            ("switch", 1, Some("flip"), true),
            ("switch", 2, Some("flip"), true),
            ("switch", 4, Some("flip"), true),
        ]
    );
    assert_eq!(to_json(&opened.project, APP).unwrap(), fixture(name));

    let mut v = value(&fixture(name));
    v["views"][0]["continuation"]["seeds"][0]["label"] = json!("the r = 2.8 node");
    v["views"][0]["continuation"]["ds_max"] = json!(0.1);
    v["views"][0]["y_axis"] = json!({ "kind": "norm" });
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let back = value(&to_json(&opened.project, APP).unwrap());
    assert_eq!(back["views"][0], v["views"][0]);
}

#[test]
fn a_continuation_seed_that_does_not_fit_is_left_out() {
    let base = value(&fixture("continuation-view.sepx"));
    let open_edited = |edit: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        edit(&mut v);
        parse(&v.to_string()).unwrap()
    };
    let seeds = |o: &Opened| -> Vec<f64> {
        o.project.views[0]
            .continuation
            .as_ref()
            .unwrap()
            .seeds
            .iter()
            .map(|s| s.param)
            .collect()
    };
    let all = seeds(&open_edited(&|_| {}));
    type Edit<'a> = &'a dyn Fn(&mut Value);
    let misfits: [(&str, usize, Edit); 4] = [
        ("two values for one state", 2, &|v| {
            v["views"][0]["continuation"]["seeds"][1]["x"] = json!([0.6, 0.6])
        }),
        ("no state at all", 1, &|v| {
            v["views"][0]["continuation"]["seeds"][0]["x"] = json!([])
        }),
        ("a switch without a tangent", 4, &|v| {
            v["views"][0]["continuation"]["seeds"][3]
                .as_object_mut()
                .unwrap()
                .remove("tangent");
        }),
        ("a tangent of the wrong length", 3, &|v| {
            v["views"][0]["continuation"]["seeds"][2]["tangent"] = json!([1.0])
        }),
    ];
    for (name, branch, edit) in misfits {
        let opened = open_edited(edit);
        assert_eq!(
            opened.warnings,
            [format!(
                "The saved continuation's branch {branch} doesn't fit this system, so it was left out."
            )],
            "{name}"
        );
        let mut kept = all.clone();
        kept.remove(branch - 1);
        assert_eq!(seeds(&opened), kept, "{name}");
    }
    let opened = open_edited(&|v| {
        v["views"][0]["continuation"]["seeds"][0]["x"] = json!([0.6, 0.6]);
        v["views"][0]["continuation"]["seeds"][2]["tangent"] = json!(null);
    });
    assert_eq!(
        opened.warnings,
        [
            "The saved continuation's branch 1 doesn't fit this system, so it was left out.",
            "The saved continuation's branch 3 doesn't fit this system, so it was left out.",
        ]
    );
    assert_eq!(seeds(&opened), [all[1], all[3]]);
    let opened = open_edited(&|v| {
        v["views"][0]["continuation"]["seeds"][0]["tangent"] = json!(null);
        v["views"][0]["continuation"]["seeds"][0]["from"] = json!("point");
    });
    assert!(
        opened.warnings.is_empty(),
        "a point seed needs no tangent: {:?}",
        opened.warnings
    );
}

#[test]
fn a_branch_plot_names_a_parameter_the_system_has() {
    let mut v = value(&fixture("continuation-view.sepx"));
    v["views"][0]["x_axis"] = json!({ "kind": "param", "name": "mu" });
    let opened = parse(&v.to_string()).unwrap();
    assert_eq!(
        opened.warnings,
        ["The x axis of the view \u{201c}main\u{201d} uses the parameter mu, which the system doesn't have, so the default view is used."]
    );
    let mut v = value(&fixture("continuation-view.sepx"));
    v["views"][0]["continuation"]["seeds"][0]["period"] = json!(-1);
    assert!(parse(&v.to_string()).is_err());
    v["views"][0]["continuation"]["seeds"][0]
        .as_object_mut()
        .unwrap()
        .remove("period");
    let opened = parse(&v.to_string()).unwrap();
    assert_eq!(
        opened.project.views[0].continuation.as_ref().unwrap().seeds[0].period,
        1
    );
}
