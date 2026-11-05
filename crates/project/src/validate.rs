//! Repairs that keep a file openable, each with a warning for the user.
//!
//! Only the structure is a hard error (see [`crate::check`]); everything
//! here is fixed in place: duplicate or empty ids, slider ranges, colors,
//! solver settings, the field grid, `x0` lengths. View ranges are only
//! reported: the UI falls back to the default view for a primary view whose
//! range isn't usable (see [`range_ok`]).

use std::collections::HashSet;

use crate::schema::{default_atol, default_method, default_rtol, default_t_end};
use crate::{ProjectFile, ProjectView, KNOWN_METHODS};

/// Number of trajectory colors (`traj-1` … `traj-7`).
pub const COLORS: i32 = 7;
pub const FIELD_GRID_MIN: u32 = 2;
pub const FIELD_GRID_MAX: u32 = 200;
pub const MAX_STEPS_MAX: u32 = 10_000_000;
pub const SAMPLES_MAX: u32 = 200_000;

/// True when a view range is usable: finite with `lo < hi`.
pub fn range_ok(r: &[f64; 2]) -> bool {
    r[0].is_finite() && r[1].is_finite() && r[0] < r[1]
}

/// The view v1 edits: the one with id `main`, otherwise the first.
pub fn primary_view_index(views: &[ProjectView]) -> Option<usize> {
    views
        .iter()
        .position(|v| v.id == "main")
        .or(if views.is_empty() { None } else { Some(0) })
}

/// Repairs `p` in place and returns one warning per repair.
pub fn validate(p: &mut ProjectFile) -> Vec<String> {
    let mut warnings = Vec::new();
    repair_ids(
        &mut p.system.params,
        "parameter",
        |x| &mut x.id,
        |_| "p".into(),
        &mut warnings,
    );
    repair_ids(
        &mut p.initial_conditions,
        "initial condition",
        |x| &mut x.id,
        |i| format!("ic-{}", i + 1),
        &mut warnings,
    );
    repair_ids(
        &mut p.views,
        "view",
        |x| &mut x.id,
        |i| format!("view-{}", i + 1),
        &mut warnings,
    );

    for param in &mut p.system.params {
        let (min, max) = (param.min, param.max);
        if !(min.is_finite() && max.is_finite() && min < max) {
            let (lo, hi) = default_param_range(param.value);
            let was = if min.is_finite() && max.is_finite() {
                format!("was {} to {} (it needs min < max)", show(min), show(max))
            } else {
                "was missing".to_string()
            };
            warnings.push(format!(
                "The slider range for {} {was}, so it is now {} to {}.",
                param.id,
                show(lo),
                show(hi)
            ));
            param.min = lo;
            param.max = hi;
        }
    }

    let states = &p.system.states;
    for ic in &mut p.initial_conditions {
        let color = ic.color.rem_euclid(COLORS);
        if color != ic.color {
            warnings.push(format!(
                "Initial condition \u{201c}{}\u{201d} had color {}; it now uses color {color}.",
                ic.id,
                show(f64::from(ic.color))
            ));
            ic.color = color;
        }
        if !states.is_empty() && ic.x0.len() != states.len() {
            warnings.push(format!(
                "Initial condition \u{201c}{}\u{201d} had {} for {} ({}); {}.",
                ic.id,
                plural(ic.x0.len(), "value"),
                plural(states.len(), "state"),
                states.join(", "),
                if ic.x0.len() < states.len() {
                    "the missing ones are 0"
                } else {
                    "the extra ones were dropped"
                }
            ));
            ic.x0.resize(states.len(), 0.0);
        }
    }

    let primary = primary_view_index(&p.views);
    for (i, view) in p.views.iter_mut().enumerate() {
        let grid = view.layers.field_grid.clamp(FIELD_GRID_MIN, FIELD_GRID_MAX);
        if grid != view.layers.field_grid {
            warnings.push(format!(
                "The field grid of the view \u{201c}{}\u{201d} was {}, so it is now {grid}.",
                view.id, view.layers.field_grid
            ));
            view.layers.field_grid = grid;
        }
        if Some(i) != primary {
            continue;
        }
        for (axis, r) in [("x", &view.x_range), ("y", &view.y_range)] {
            if !range_ok(r) {
                warnings.push(format!(
                    "The {axis} range of the view \u{201c}{}\u{201d} ({} to {}) is empty, so the default view is used.",
                    view.id,
                    show(r[0]),
                    show(r[1])
                ));
            }
        }
        for (name, axis) in [("x", &view.x_axis), ("y", &view.y_axis)] {
            let known = match axis.kind.as_str() {
                "time" => true,
                "state" => axis
                    .name
                    .as_ref()
                    .is_some_and(|n| states.is_empty() || states.iter().any(|s| s == n)),
                _ => false,
            };
            if !known {
                let what = match (axis.kind.as_str(), &axis.name) {
                    ("state", Some(n)) => format!("the state {n}, which the system doesn't have"),
                    ("state", None) => "a state without a name".to_string(),
                    (kind, _) => {
                        format!("an axis kind (\u{201c}{kind}\u{201d}) this version doesn't know")
                    }
                };
                warnings.push(format!(
                    "The {name} axis of the view \u{201c}{}\u{201d} uses {what}, so the default view is used.",
                    view.id
                ));
            }
        }
    }

    let s = &mut p.solver;
    if !KNOWN_METHODS.contains(&s.method.as_str()) {
        warnings.push(format!(
            "The solver method \u{201c}{}\u{201d} isn't available in this version of Separatrix, so Dormand\u{2013}Prince 5(4) is used.",
            s.method
        ));
        s.method = default_method();
    }
    let mut fix = |what: &str, was: String, now: String| {
        warnings.push(format!(
            "The solver's {what} was {was}, so it is now {now}."
        ));
    };
    if !(s.rtol > 0.0 && s.rtol.is_finite()) {
        fix("relative tolerance", show(s.rtol), show(default_rtol()));
        s.rtol = default_rtol();
    }
    if !(s.atol > 0.0 && s.atol.is_finite()) {
        fix("absolute tolerance", show(s.atol), show(default_atol()));
        s.atol = default_atol();
    }
    if !(s.h >= 0.0 && s.h.is_finite()) {
        fix("step size", show(s.h), "0 (the method's default)".into());
        s.h = 0.0;
    }
    let steps = s.max_steps.clamp(1, MAX_STEPS_MAX);
    if steps != s.max_steps {
        fix(
            "maximum number of steps",
            s.max_steps.to_string(),
            steps.to_string(),
        );
        s.max_steps = steps;
    }
    if !(s.t_end > 0.0 && s.t_end.is_finite()) {
        fix("end time", show(s.t_end), show(default_t_end()));
        s.t_end = default_t_end();
    }
    if s.samples > SAMPLES_MAX {
        fix(
            "number of samples",
            s.samples.to_string(),
            SAMPLES_MAX.to_string(),
        );
        s.samples = SAMPLES_MAX;
    }

    warnings
}

/// Makes ids non-empty and unique: an empty id becomes `empty(index)`, and
/// a repeat gets the first free suffix (`ic-3` → `ic-3-2`).
fn repair_ids<T>(
    items: &mut [T],
    what: &str,
    id: impl Fn(&mut T) -> &mut String,
    empty: impl Fn(usize) -> String,
    warnings: &mut Vec<String>,
) {
    // Ids already taken by a later item stay with it: only repeats move.
    let mut seen: HashSet<String> = HashSet::new();
    let all: HashSet<String> = items.iter_mut().map(|x| id(x).clone()).collect();
    for (i, item) in items.iter_mut().enumerate() {
        let current = id(item);
        let base = if current.is_empty() {
            empty(i)
        } else {
            current.clone()
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while seen.contains(&candidate) || (candidate != *current && all.contains(&candidate)) {
            candidate = format!("{base}-{n}");
            n += 1;
        }
        if candidate != *current {
            if current.is_empty() {
                let article = if what.starts_with(['a', 'e', 'i', 'o', 'u']) {
                    "An"
                } else {
                    "A"
                };
                warnings.push(format!(
                    "{article} {what} had no id; it is now \u{201c}{candidate}\u{201d}."
                ));
            } else {
                warnings.push(format!(
                    "The {what} id \u{201c}{current}\u{201d} appears more than once; this one is now \u{201c}{candidate}\u{201d}."
                ));
            }
            *current = candidate.clone();
        }
        seen.insert(candidate);
    }
}

/// The slider range the app gives a parameter (port of `defaultParamRange`
/// in `app/src/state/model.ts`).
pub fn default_param_range(v: f64) -> (f64, f64) {
    if v == 0.0 || !v.is_finite() {
        return (-1.0, 1.0);
    }
    let span = nice_ceil(v.abs() * 2.0);
    if v > 0.0 {
        (0.0, span)
    } else {
        (-span, 0.0)
    }
}

/// Port of `niceCeil` in `app/src/state/model.ts`.
fn nice_ceil(v: f64) -> f64 {
    if v <= 0.0 {
        return 1.0;
    }
    let p = 10f64.powf(v.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if m * p >= v - 1e-12 {
            return m * p;
        }
    }
    10.0 * p
}

/// A number for a message, with the real minus sign.
fn show(v: f64) -> String {
    let a = v.abs();
    let s = if v == 0.0 {
        "0".to_string()
    } else if (1e-4..1e15).contains(&a) {
        format!("{v}")
    } else {
        format!("{v:e}")
    };
    s.replace('-', "\u{2212}")
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_range_matches_the_app() {
        assert_eq!(default_param_range(0.0), (-1.0, 1.0));
        assert_eq!(default_param_range(3.0), (0.0, 10.0));
        assert_eq!(default_param_range(1.0), (0.0, 2.0));
        assert_eq!(default_param_range(-0.3), (-1.0, 0.0));
        assert_eq!(default_param_range(28.0), (0.0, 100.0));
        assert_eq!(default_param_range(1.2), (0.0, 2.5));
    }

    #[test]
    fn numbers_use_the_real_minus_sign() {
        assert_eq!(show(-0.25), "\u{2212}0.25");
        assert_eq!(show(1e-6), "1e\u{2212}6");
        assert_eq!(show(3.0), "3");
        assert_eq!(show(-0.0), "0");
    }

    #[test]
    fn ids_get_suffixes() {
        let mut ids = vec![
            "ic-3".to_string(),
            "ic-3".into(),
            "".into(),
            "ic-3-2".into(),
        ];
        let mut w = Vec::new();
        repair_ids(
            &mut ids,
            "initial condition",
            |x| x,
            |i| format!("ic-{}", i + 1),
            &mut w,
        );
        // The second "ic-3" can't take "ic-3-2": a later item already has it.
        // The empty one starts from its index ("ic-3"), which is taken too.
        assert_eq!(ids, ["ic-3", "ic-3-3", "ic-3-4", "ic-3-2"]);
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn empty_id_takes_its_index() {
        let mut ids = vec!["ic-1".to_string(), "".into()];
        let mut w = Vec::new();
        repair_ids(
            &mut ids,
            "initial condition",
            |x| x,
            |i| format!("ic-{}", i + 1),
            &mut w,
        );
        assert_eq!(ids, ["ic-1", "ic-2"]);
        assert_eq!(
            w,
            ["An initial condition had no id; it is now \u{201c}ic-2\u{201d}."]
        );
    }
}
