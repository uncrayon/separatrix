//! Versioning: newer readable formats, files that need a newer reader, and
//! the migration chain.

mod common;

use common::{fixture, fixture_path, APP};
use separatrix_project::{
    migrate_with, parse, parse_named, read_file, to_json, ProjectError, Step, CURRENT_VERSION,
    KNOWN_METHODS, MIN_READER_VERSION, STEPS,
};
use serde_json::{json, Map, Value};

fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn a_newer_readable_format_opens_with_a_warning_and_keeps_its_parts() {
    let opened = read_file(&fixture_path("future-v2-readable.sepx")).unwrap();
    assert_eq!(opened.newer_format, Some(2));
    assert_eq!(opened.migrated_from, None);
    assert_eq!(
        opened.warnings,
        ["\u{201c}future-v2-readable.sepx\u{201d} was saved in a newer format by Separatrix 0.3.0. It opens, and saving keeps the parts this version doesn't use."]
    );
    let p = &opened.project;
    assert_eq!(p.format_version, 2);
    assert_eq!(p.views[0].x_range, [-0.25, 3.5]);

    // Saving keeps the format and reader versions, and the v2-only keys.
    let out = value(&to_json(p, APP).unwrap());
    assert_eq!(out["format_version"], 2);
    assert_eq!(out["min_reader_version"], 1);
    assert_eq!(out["app_version"], APP);
    let input = value(&fixture("future-v2-readable.sepx"));
    assert_eq!(out["assignment"], input["assignment"]);
    assert_eq!(out["system"]["units"], input["system"]["units"]);
}

#[test]
fn a_newer_format_without_app_version_names_the_format_version() {
    let mut v = value(&fixture("future-v2-readable.sepx"));
    v.as_object_mut().unwrap().remove("app_version");
    let opened = parse_named(&v.to_string(), "hw.sepx").unwrap();
    assert_eq!(
        opened.warnings,
        ["\u{201c}hw.sepx\u{201d} was saved in a newer format (version 2). It opens, and saving keeps the parts this version doesn't use."]
    );
    let opened = parse(&v.to_string()).unwrap();
    assert!(opened.warnings[0].starts_with("This project was saved in a newer format"));
}

#[test]
fn a_file_that_needs_a_newer_reader_is_refused() {
    let err = read_file(&fixture_path("future-v3-unreadable.sepx")).unwrap_err();
    assert_eq!(
        err,
        ProjectError::TooNew {
            file: "future-v3-unreadable.sepx".into(),
            saved_by: Some("0.5.0".into()),
            min_reader_version: 3
        }
    );
    assert_eq!(
        err.to_string(),
        "\u{201c}future-v3-unreadable.sepx\u{201d} was saved by Separatrix 0.5.0 and needs a newer version to open. Update Separatrix, then open it again."
    );
}

#[test]
fn min_reader_version_is_checked_at_any_format_version() {
    // format_version 1 (not newer), but it asks for reader 2.
    let err = parse_named(&fixture("v1-min-reader-2.sepx"), "v1-min-reader-2.sepx").unwrap_err();
    assert_eq!(
        err,
        ProjectError::TooNew {
            file: "v1-min-reader-2.sepx".into(),
            saved_by: Some("0.4.1".into()),
            min_reader_version: 2
        }
    );
}

#[test]
fn too_new_without_app_version() {
    for app in ["", "\"app_version\": \"\",", "\"app_version\": \"   \","] {
        let text = format!("{{ {app} \"format\": \"separatrix\", \"format_version\": 4, \"min_reader_version\": 2 }}");
        let err = parse_named(&text, "x.sepx").unwrap_err();
        assert_eq!(
            err.to_string(),
            "\u{201c}x.sepx\u{201d} needs a newer version of Separatrix to open. Update Separatrix, then open it again.",
            "{text}"
        );
    }
    let err =
        parse("{\"format\": \"separatrix\", \"format_version\": 9, \"min_reader_version\": 9}")
            .unwrap_err();
    assert_eq!(
        err.to_string(),
        "This project needs a newer version of Separatrix to open. Update Separatrix, then open it again."
    );
}

#[test]
fn a_missing_min_reader_version_counts_as_the_format_version() {
    let mut v = value(&fixture("future-v2-readable.sepx"));
    v.as_object_mut().unwrap().remove("min_reader_version");
    assert!(matches!(
        parse(&v.to_string()),
        Err(ProjectError::TooNew {
            min_reader_version: 2,
            ..
        })
    ));
}

#[test]
fn too_new_is_checked_before_the_structure() {
    // A v3 file needn't have a v1 `system`.
    let err = parse(&fixture("future-v3-unreadable.sepx")).unwrap_err();
    assert!(matches!(err, ProjectError::TooNew { .. }));
}

#[test]
fn this_build_writes_the_current_version() {
    assert_eq!(CURRENT_VERSION, 1);
    assert_eq!(MIN_READER_VERSION, 1);
    assert!(STEPS.is_empty());
    let out = value(
        &to_json(
            &parse(&fixture("rabbits-vs-sheep.sepx")).unwrap().project,
            APP,
        )
        .unwrap(),
    );
    assert_eq!(out["format_version"], CURRENT_VERSION);
    assert_eq!(out["min_reader_version"], MIN_READER_VERSION);
}

#[test]
fn known_methods_are_lowercase_and_unique() {
    for m in KNOWN_METHODS {
        assert_eq!(m, m.to_lowercase());
    }
    let mut sorted = KNOWN_METHODS.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), KNOWN_METHODS.len());
    assert!(KNOWN_METHODS.contains(&"dopri5"));
}

// ---- the migration chain, with synthetic steps ----

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// v1 → v2: `system.equations` is renamed `system.lines`.
fn step_1_to_2(v: &mut Map<String, Value>) -> Result<(), String> {
    let system = v
        .get_mut("system")
        .and_then(Value::as_object_mut)
        .ok_or("system is missing")?;
    if let Some(eq) = system.remove("equations") {
        system.insert("lines".into(), eq);
    }
    push_log(v, "1->2");
    Ok(())
}

/// v2 → v3: `solver.tmax` becomes `solver.t_end`.
fn step_2_to_3(v: &mut Map<String, Value>) -> Result<(), String> {
    let solver = v.entry("solver").or_insert_with(|| json!({}));
    let solver = solver.as_object_mut().ok_or("solver should be an object")?;
    if let Some(t) = solver.remove("tmax") {
        solver.insert("t_end".into(), t);
    }
    push_log(v, "2->3");
    Ok(())
}

fn failing(_: &mut Map<String, Value>) -> Result<(), String> {
    Err("the views couldn't be converted".into())
}

/// Records the order steps ran in, and the version each one saw.
fn push_log(v: &mut Map<String, Value>, what: &str) {
    let seen = v["format_version"].clone();
    let log = v.entry("log").or_insert_with(|| json!([]));
    log.as_array_mut().unwrap().push(json!([what, seen]));
}

const CHAIN: [Step; 2] = [step_1_to_2, step_2_to_3];

#[test]
fn migration_applies_each_step_in_order_and_bumps_the_version() {
    let mut v = obj(json!({
        "format": "separatrix", "format_version": 1,
        "system": { "equations": ["x' = x"] }, "solver": { "tmax": 5 }
    }));
    migrate_with(&mut v, 1, 3, &CHAIN).unwrap();
    assert_eq!(v["format_version"], 3);
    assert_eq!(v["system"], json!({ "lines": ["x' = x"] }));
    assert_eq!(v["solver"], json!({ "t_end": 5 }));
    assert_eq!(v["log"], json!([["1->2", 1], ["2->3", 2]]));
}

#[test]
fn migration_starts_at_the_file_version() {
    let mut v = obj(json!({ "format_version": 2, "system": {}, "solver": { "tmax": 1 } }));
    migrate_with(&mut v, 2, 3, &CHAIN).unwrap();
    assert_eq!(v["log"], json!([["2->3", 2]]));
    assert_eq!(v["format_version"], 3);
}

#[test]
fn migration_to_the_same_or_an_older_version_does_nothing() {
    let v0 = obj(json!({ "format_version": 3, "a": 1 }));
    let mut v = v0.clone();
    migrate_with(&mut v, 3, 3, &CHAIN).unwrap();
    assert_eq!(v, v0);
    migrate_with(&mut v, 3, 2, &CHAIN).unwrap();
    assert_eq!(v, v0);
}

#[test]
fn a_failing_step_is_reported() {
    let mut v = obj(json!({ "format_version": 1, "system": {} }));
    let err = migrate_with(&mut v, 1, 3, &[step_1_to_2, failing]).unwrap_err();
    assert_eq!(
        err,
        ProjectError::Damaged {
            file: String::new(),
            detail: "it couldn't be updated from format version 2: the views couldn't be converted"
                .into()
        }
    );
    let err = migrate_with(&mut obj(json!({ "format_version": 1 })), 1, 2, &CHAIN).unwrap_err();
    assert_eq!(
        err.with_file("old.sepx").to_string(),
        "\u{201c}old.sepx\u{201d} is damaged: it couldn't be updated from format version 1: system is missing. Fix it in a text editor, or open a backup copy."
    );
}

#[test]
fn a_missing_step_is_reported() {
    let mut v = obj(json!({ "format_version": 1, "system": {} }));
    let err = migrate_with(&mut v, 1, 4, &CHAIN).unwrap_err();
    assert_eq!(
        err,
        ProjectError::Damaged {
            file: String::new(),
            detail: "this version of Separatrix can't update format version 3 to 4".into()
        }
    );
}
