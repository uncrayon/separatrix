//! The structural check: walks the JSON value against the v1 schema and
//! reports the first problem with its path, e.g.
//! `initial_conditions[2].x0[1] should be a number, not null`.
//!
//! It runs before the typed `serde` conversion (whose messages don't name
//! the path) on both the read and the write path. Unknown keys are never a
//! problem: they are kept. A whole number written as a float (`21.0`,
//! `1e5`, as Python's `json` or a spreadsheet may write it) where an
//! integer belongs is accepted and rewritten as an integer.

use serde_json::Value;

#[derive(Clone, Copy)]
enum Ty {
    Str,
    Bool,
    /// Any JSON number (a JSON number is always finite).
    Num,
    U32,
    I32,
    I64,
    /// `"latex"` or `"text"`.
    InputFormat,
    Nullable(&'static Ty),
    List(&'static Ty),
    /// Exactly two numbers.
    Pair,
    /// An object whose values are numbers.
    NumMap,
    Obj(&'static [F]),
    Any,
}

#[derive(Clone, Copy)]
struct F {
    key: &'static str,
    ty: Ty,
    required: bool,
}

const fn req(key: &'static str, ty: Ty) -> F {
    F {
        key,
        ty,
        required: true,
    }
}

const fn opt(key: &'static str, ty: Ty) -> F {
    F {
        key,
        ty,
        required: false,
    }
}

const PARAM: &[F] = &[
    req("id", Ty::Str),
    req("value", Ty::Num),
    opt("min", Ty::Num),
    opt("max", Ty::Num),
];

const SYSTEM: &[F] = &[
    opt("kind", Ty::Str),
    req("input_format", Ty::InputFormat),
    req("lines", Ty::List(&Ty::Str)),
    opt("states", Ty::List(&Ty::Str)),
    opt("params", Ty::List(&Ty::Obj(PARAM))),
    opt("template", Ty::Nullable(&Ty::Str)),
];

const INITIAL_CONDITION: &[F] = &[
    opt("id", Ty::Str),
    req("x0", Ty::List(&Ty::Num)),
    opt("t0", Ty::Num),
    opt("color", Ty::I32),
    opt("hidden", Ty::Bool),
];

const AXIS: &[F] = &[req("kind", Ty::Str), opt("name", Ty::Nullable(&Ty::Str))];

const SLICE: &[F] = &[opt("states", Ty::NumMap), opt("t", Ty::Num)];

const LAYERS: &[F] = &[
    opt("field", Ty::Bool),
    opt("field_grid", Ty::U32),
    opt("nullclines", Ty::Bool),
    opt("fixed_points", Ty::Bool),
    opt("backward", Ty::Bool),
];

const SECTION: &[F] = &[
    req("kind", Ty::Str),
    opt("state", Ty::Nullable(&Ty::Str)),
    opt("value", Ty::Nullable(&Ty::Num)),
    opt("direction", Ty::Nullable(&Ty::Str)),
    opt("period", Ty::Nullable(&Ty::Num)),
    opt("phase", Ty::Nullable(&Ty::Num)),
];

const CAMERA: &[F] = &[
    opt("yaw", Ty::Num),
    opt("pitch", Ty::Num),
    opt("zoom", Ty::Num),
];

const LYAPUNOV: &[F] = &[
    opt("transient", Ty::Nullable(&Ty::Num)),
    opt("total", Ty::Nullable(&Ty::Num)),
    opt("interval", Ty::Nullable(&Ty::Num)),
    opt("blocks", Ty::Nullable(&Ty::U32)),
];

const VIEW: &[F] = &[
    opt("id", Ty::Str),
    opt("kind", Ty::Nullable(&Ty::Str)),
    opt("section", Ty::Nullable(&Ty::Obj(SECTION))),
    req("x_axis", Ty::Obj(AXIS)),
    req("y_axis", Ty::Obj(AXIS)),
    opt("z_axis", Ty::Nullable(&Ty::Obj(AXIS))),
    req("x_range", Ty::Pair),
    req("y_range", Ty::Pair),
    opt("z_range", Ty::Nullable(&Ty::Pair)),
    opt("camera", Ty::Nullable(&Ty::Obj(CAMERA))),
    opt("lyapunov", Ty::Nullable(&Ty::Obj(LYAPUNOV))),
    opt("slice", Ty::Obj(SLICE)),
    opt("layers", Ty::Obj(LAYERS)),
];

const SOLVER: &[F] = &[
    opt("method", Ty::Str),
    opt("rtol", Ty::Num),
    opt("atol", Ty::Num),
    opt("h", Ty::Num),
    opt("max_steps", Ty::U32),
    opt("t_end", Ty::Num),
    opt("transient", Ty::Nullable(&Ty::I64)),
    opt("samples", Ty::U32),
];

const PROJECT: &[F] = &[
    opt("format", Ty::Str),
    opt("format_version", Ty::U32),
    opt("min_reader_version", Ty::U32),
    opt("app_version", Ty::Str),
    req("system", Ty::Obj(SYSTEM)),
    opt("initial_conditions", Ty::List(&Ty::Obj(INITIAL_CONDITION))),
    opt("views", Ty::List(&Ty::Obj(VIEW))),
    opt("analyses", Ty::List(&Ty::Any)),
    opt("solver", Ty::Obj(SOLVER)),
    opt("export_settings", Ty::Any),
];

/// Checks a whole project value. The error is a sentence fragment naming
/// the path, for [`crate::ProjectError::Damaged`].
pub fn check_project(v: &mut Value) -> Result<(), String> {
    if !v.is_object() {
        return Err(format!(
            "the project should be a JSON object, not {}",
            describe(v)
        ));
    }
    check(v, Ty::Obj(PROJECT), "")
}

/// `v` as a whole number: an integer, or a float with no fractional part
/// (`21.0`, `1e5`).
pub(crate) fn whole_number(v: &Value) -> Option<i128> {
    if let Some(n) = v.as_i64() {
        return Some(i128::from(n));
    }
    if let Some(n) = v.as_u64() {
        return Some(i128::from(n));
    }
    let f = v.as_f64()?;
    // Below 2^64 every f64 with no fraction is exactly an integer.
    (f.fract() == 0.0 && f.abs() < 18_446_744_073_709_551_616.0).then_some(f as i128)
}

/// `v` as a whole number in `lo..=hi`, rewritten as a JSON integer.
fn integer_in(v: &mut Value, lo: i128, hi: i128) -> bool {
    match whole_number(v) {
        Some(n) if (lo..=hi).contains(&n) => {
            if !v.is_i64() && !v.is_u64() {
                *v = if n < 0 {
                    Value::from(n as i64)
                } else {
                    Value::from(n as u64)
                };
            }
            true
        }
        _ => false,
    }
}

fn check(v: &mut Value, ty: Ty, path: &str) -> Result<(), String> {
    let shown_v = describe(v);
    let wrong = |expected: &str| {
        Err(format!(
            "{} should be {expected}, not {shown_v}",
            shown(path),
        ))
    };
    match ty {
        Ty::Any => Ok(()),
        Ty::Str => {
            if v.is_string() {
                Ok(())
            } else {
                wrong("text")
            }
        }
        Ty::Bool => {
            if v.is_boolean() {
                Ok(())
            } else {
                wrong("true or false")
            }
        }
        Ty::Num => {
            if v.is_number() {
                Ok(())
            } else {
                wrong("a number")
            }
        }
        Ty::U32 => {
            if integer_in(v, 0, i128::from(u32::MAX)) {
                Ok(())
            } else {
                wrong("a whole number from 0 to 4294967295")
            }
        }
        Ty::I32 => {
            if integer_in(v, i128::from(i32::MIN), i128::from(i32::MAX)) {
                Ok(())
            } else {
                wrong("a whole number")
            }
        }
        Ty::I64 => {
            if integer_in(v, i128::from(i64::MIN), i128::from(i64::MAX)) {
                Ok(())
            } else {
                wrong("a whole number")
            }
        }
        Ty::InputFormat => match v.as_str() {
            Some("latex" | "text") => Ok(()),
            _ => wrong("\"latex\" or \"text\""),
        },
        Ty::Nullable(inner) => {
            if v.is_null() {
                Ok(())
            } else {
                check(v, *inner, path)
            }
        }
        Ty::List(item) => {
            let Some(items) = v.as_array_mut() else {
                return wrong("a list");
            };
            for (i, x) in items.iter_mut().enumerate() {
                check(x, *item, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        Ty::Pair => {
            let Some(items) = v.as_array_mut() else {
                return wrong("a list of two numbers");
            };
            if items.len() != 2 {
                return Err(format!(
                    "{} should be a list of two numbers, not of {}",
                    shown(path),
                    items.len()
                ));
            }
            for (i, x) in items.iter_mut().enumerate() {
                check(x, Ty::Num, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        Ty::NumMap => {
            let Some(map) = v.as_object_mut() else {
                return wrong("an object");
            };
            for (k, x) in map.iter_mut() {
                check(x, Ty::Num, &join(path, k))?;
            }
            Ok(())
        }
        Ty::Obj(fields) => {
            let Some(map) = v.as_object_mut() else {
                return wrong("an object");
            };
            for f in fields {
                match map.get_mut(f.key) {
                    Some(x) => check(x, f.ty, &join(path, f.key))?,
                    None if f.required => return Err(format!("{} is missing", join(path, f.key))),
                    None => {}
                }
            }
            Ok(())
        }
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn shown(path: &str) -> &str {
    if path.is_empty() {
        "the project"
    } else {
        path
    }
}

fn describe(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let short: String = s.chars().take(40).collect();
            let more = if short.len() < s.len() {
                "\u{2026}"
            } else {
                ""
            };
            format!("\"{short}{more}\"")
        }
        Value::Array(_) => "a list".into(),
        Value::Object(_) => "an object".into(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn minimal() -> Value {
        json!({ "system": { "input_format": "latex", "lines": ["\\dot{x}=x"] } })
    }

    #[test]
    fn minimal_project_passes() {
        assert_eq!(check_project(&mut minimal()), Ok(()));
    }

    #[test]
    fn root_must_be_an_object() {
        assert_eq!(
            check_project(&mut json!([1])),
            Err("the project should be a JSON object, not a list".into())
        );
    }

    #[test]
    fn null_where_a_number_belongs_names_the_path() {
        let mut v = minimal();
        v["initial_conditions"] = json!([
            { "id": "ic-1", "x0": [0, 1] },
            { "id": "ic-2", "x0": [0, 1] },
            { "id": "ic-3", "x0": [0.5, null] },
        ]);
        assert_eq!(
            check_project(&mut v),
            Err("initial_conditions[2].x0[1] should be a number, not null".into())
        );
    }

    #[test]
    fn missing_required_keys() {
        assert_eq!(
            check_project(&mut json!({})),
            Err("system is missing".into())
        );
        let mut v = json!({ "system": { "input_format": "latex" } });
        assert_eq!(check_project(&mut v), Err("system.lines is missing".into()));
        let mut v = json!({ "system": { "lines": [] } });
        assert_eq!(
            check_project(&mut v),
            Err("system.input_format is missing".into())
        );
    }

    #[test]
    fn input_format_is_latex_or_text() {
        let mut v = minimal();
        v["system"]["input_format"] = json!("asciimath");
        assert_eq!(
            check_project(&mut v),
            Err("system.input_format should be \"latex\" or \"text\", not \"asciimath\"".into())
        );
    }

    #[test]
    fn ranges_are_pairs() {
        let mut v = minimal();
        v["views"] = json!([{ "id": "main", "x_axis": { "kind": "time" }, "y_axis": { "kind": "state", "name": "x" },
            "x_range": [0, 1, 2], "y_range": [0, 1] }]);
        assert_eq!(
            check_project(&mut v),
            Err("views[0].x_range should be a list of two numbers, not of 3".into())
        );
    }

    #[test]
    fn integers_are_checked() {
        let mut v = minimal();
        v["solver"] = json!({ "max_steps": -3 });
        assert_eq!(
            check_project(&mut v),
            Err("solver.max_steps should be a whole number from 0 to 4294967295, not -3".into())
        );
        let mut v = minimal();
        v["initial_conditions"] = json!([{ "x0": [], "color": 1.5 }]);
        assert_eq!(
            check_project(&mut v),
            Err("initial_conditions[0].color should be a whole number, not 1.5".into())
        );
    }

    #[test]
    fn whole_floats_are_integers() {
        let mut v = minimal();
        v["solver"] = json!({ "max_steps": 1e5, "samples": 2000.0 });
        v["initial_conditions"] =
            json!([{ "x0": [1.0], "color": 3e0 }, { "x0": [], "color": -2.0 }]);
        assert_eq!(check_project(&mut v), Ok(()));
        assert_eq!(v["solver"]["max_steps"], json!(100000));
        assert!(v["solver"]["max_steps"].is_u64());
        assert!(v["solver"]["samples"].is_u64());
        assert!(v["initial_conditions"][0]["color"].is_u64());
        assert_eq!(v["initial_conditions"][1]["color"], json!(-2));
        // Numbers that aren't integers stay as typed.
        assert_eq!(v["initial_conditions"][0]["x0"][0], json!(1.0));
        let mut v = minimal();
        v["solver"] = json!({ "max_steps": 4294967296.0 });
        assert_eq!(
            check_project(&mut v),
            Err(
                "solver.max_steps should be a whole number from 0 to 4294967295, not 4294967296.0"
                    .into()
            )
        );
    }

    #[test]
    fn slice_values_are_numbers() {
        let mut v = minimal();
        v["views"] = json!([{ "x_axis": { "kind": "time" }, "y_axis": { "kind": "time" },
            "x_range": [0, 1], "y_range": [0, 1], "slice": { "states": { "z": "zero" } } }]);
        assert_eq!(
            check_project(&mut v),
            Err("views[0].slice.states.z should be a number, not \"zero\"".into())
        );
    }

    #[test]
    fn unknown_keys_and_any_values_pass() {
        let mut v = minimal();
        v["assignment"] = json!(null);
        v["analyses"] = json!([{ "kind": "lyapunov" }, 3, null]);
        v["export_settings"] = json!("anything");
        v["system"]["future"] = json!({ "deep": [1, 2] });
        assert_eq!(check_project(&mut v), Ok(()));
    }
}
