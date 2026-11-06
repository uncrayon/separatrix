//! Repairs that keep a file openable, each with a warning for the user.
//!
//! Only the structure is a hard error (see [`crate::check`]); everything
//! here is fixed in place: duplicate or empty ids, slider ranges, colors,
//! solver settings (the transient too), the field grid, `x0` lengths. View
//! ranges, axes and a cobweb, section, 3D, Lyapunov or orbit view that
//! doesn't fit the system are only reported: the UI falls back to the default
//! view for a primary view whose range isn't usable (see [`range_ok`]), whose
//! cobweb axes are wrong (see [`cobweb_axes_ok`]), whose section doesn't fit
//! (see [`section_misfit`]), whose 3D axes don't (see [`space_misfit`]),
//! whose Lyapunov run can't be run (see [`lyapunov_misfit`]) or whose orbit
//! diagram can't be drawn (see [`orbit_misfit`]).

use std::collections::HashSet;

use crate::schema::{default_atol, default_method, default_rtol, default_t_end};
use crate::{ProjectFile, ProjectView, KNOWN_METHODS};

/// Number of trajectory colors (`traj-1` … `traj-7`).
pub const COLORS: i32 = 7;
pub const FIELD_GRID_MIN: u32 = 2;
pub const FIELD_GRID_MAX: u32 = 200;
pub const MAX_STEPS_MAX: u32 = 10_000_000;
pub const SAMPLES_MAX: u32 = 200_000;
/// The most transient iterates a map may skip (the app's `MAX_ITERATES`).
pub const TRANSIENT_MAX: i64 = 1_000_000;
/// The view kind of a cobweb plot.
pub const COBWEB: &str = "cobweb";
/// The warning for a saved cobweb view the app can't show; the app gives the
/// same one when the compiled system turns out not to be a one-dimensional
/// autonomous map.
pub const COBWEB_MISFIT: &str =
    "The saved cobweb view doesn't fit this system, so the default view is shown.";

/// The view kind of a Poincaré section.
pub const SECTION: &str = "section";
/// The warning for a saved section view the app can't show (the app gives
/// the same one when the compiled system can't take it).
pub const SECTION_MISFIT: &str =
    "The saved Poincaré section doesn't fit this system, so the default view is shown.";

/// Why the section view `view` can't be shown for a system of `kind` with
/// `states` (either may be empty: not known until the app compiles), or
/// `None` when it can. A section kind this version doesn't know and a plane
/// through a state the system lacks are named; anything else is
/// [`SECTION_MISFIT`]: no section, a map, a plane of a one-state system, a
/// missing value or an unknown direction, a period that isn't positive, or
/// axes other than two states or a state and its next crossing.
pub fn section_misfit(view: &ProjectView, kind: &str, states: &[String]) -> Option<String> {
    let misfit = || Some(SECTION_MISFIT.to_string());
    let Some(s) = &view.section else {
        return misfit();
    };
    let has = |n: &String| states.is_empty() || states.contains(n);
    match s.kind.as_str() {
        "plane" => {
            if let Some(n) = s.state.as_ref().filter(|n| !has(n)) {
                return Some(format!(
                    "The section of the view \u{201c}{}\u{201d} holds the state {n} fixed, which the system doesn't have, so the default view is used.",
                    view.id
                ));
            }
            let direction = s.direction.as_deref();
            let two = states.is_empty() || states.len() >= 2;
            if s.state.is_none()
                || s.value.is_none()
                || !matches!(direction, Some("rising" | "falling" | "both"))
                || !two
            {
                return misfit();
            }
        }
        "stroboscopic" => {
            if !s.period.is_some_and(|p| p > 0.0) {
                return misfit();
            }
        }
        other => {
            return Some(format!(
                "The section of the view \u{201c}{}\u{201d} is of a kind (\u{201c}{other}\u{201d}) this version doesn't know, so the default view is used.",
                view.id
            ))
        }
    }
    let (x, y) = (&view.x_axis, &view.y_axis);
    let pair = x.kind == "state"
        && x.name.is_some()
        && match y.kind.as_str() {
            "state" => y.name.is_some() && y.name != x.name,
            "next" => y.name == x.name,
            _ => false,
        };
    if kind == "map" || !pair {
        return misfit();
    }
    None
}

/// The view kind of a 3D view of a flow.
pub const SPACE: &str = "space";
/// The warning for a saved 3D view the app can't show (the app gives the
/// same one when the compiled system can't take it).
pub const SPACE_MISFIT: &str =
    "The saved 3D view doesn't fit this system, so the default view is shown.";

/// Why the 3D view `view` can't be shown for a system of `kind` with
/// `states` (either may be empty: not known until the app compiles), or
/// `None` when it can: a map, a system with fewer than three states, or axes
/// other than three different states with a third range. A state the system
/// lacks and an empty range are reported with the other axes and ranges.
pub fn space_misfit(view: &ProjectView, kind: &str, states: &[String]) -> Option<String> {
    let names: Vec<Option<&String>> =
        [Some(&view.x_axis), Some(&view.y_axis), view.z_axis.as_ref()]
            .iter()
            .map(|a| {
                a.filter(|a| a.kind == "state")
                    .and_then(|a| a.name.as_ref())
            })
            .collect();
    let distinct = names.iter().all(Option::is_some)
        && names[0] != names[1]
        && names[0] != names[2]
        && names[1] != names[2];
    let three = states.is_empty() || states.len() >= 3;
    if kind == "map" || !three || !distinct || view.z_range.is_none() {
        return Some(SPACE_MISFIT.to_string());
    }
    None
}

/// The view kind of the Lyapunov spectrum's running estimates.
pub const LYAPUNOV: &str = "lyapunov";
/// The warning for a saved Lyapunov view the app can't run (the app gives
/// the same one when it judges the run itself).
pub const LYAPUNOV_MISFIT: &str =
    "The saved Lyapunov view can't be run as saved, so the default view is shown.";

/// Why the Lyapunov view `view` can't be run for a system of `kind` (empty
/// when not known until the app compiles), or `None` when it can: the axes
/// must be time across and `{ "kind": "exponent" }` up, the transient 0 or
/// more, the total and interval above 0 (whole numbers for a map), and
/// `blocks` from 2 to the number of QR steps, total / interval rounded up.
/// The app also refuses more QR steps than the core takes in one run.
pub fn lyapunov_misfit(view: &ProjectView, kind: &str) -> Option<String> {
    let misfit = || Some(LYAPUNOV_MISFIT.to_string());
    let Some(l) = &view.lyapunov else {
        return misfit();
    };
    let (Some(transient), Some(total), Some(interval), Some(blocks)) =
        (l.transient, l.total, l.interval, l.blocks)
    else {
        return misfit();
    };
    let map = kind == "map";
    let whole = |v: f64| !map || v.fract() == 0.0;
    let ok = view.x_axis.kind == "time"
        && view.y_axis.kind == "exponent"
        && transient.is_finite()
        && transient >= 0.0
        && whole(transient)
        && total.is_finite()
        && total > 0.0
        && whole(total)
        && interval.is_finite()
        && interval > 0.0
        && whole(interval);
    if !ok {
        return misfit();
    }
    let ratio = total / interval;
    let steps = if (ratio - ratio.round()).abs() <= 1e-9 * ratio.max(1.0) {
        ratio.round()
    } else {
        ratio.ceil()
    };
    if blocks < 2 || f64::from(blocks) > steps {
        return misfit();
    }
    None
}

/// The view kind of a map's orbit diagram.
pub const ORBIT: &str = "orbit";
/// The warning for a saved orbit diagram the app can't draw (the app gives
/// the same one when it judges the view itself).
pub const ORBIT_MISFIT: &str =
    "The saved orbit diagram doesn't fit this system, so the default view is shown.";
/// The most iterates an orbit diagram's column drops.
pub const ORBIT_TRANSIENT_MAX: u32 = 100_000;
/// The most iterates an orbit diagram's column draws.
pub const ORBIT_KEPT_MAX: u32 = 976;

/// Why the orbit diagram `view` can't be drawn for a system of `kind`, with
/// `states` and `params` (each empty when not known until the app
/// compiles), or `None` when it can: the system is a map, `x_axis` is
/// `{ "kind": "param", "name": p }` of a parameter it has, `y_axis` a state
/// it has, and `orbit` holds `transient` (0 to 100,000) and `kept` (1 to
/// 976, so a diagram of the widest plot's 4,096 columns stays within
/// `MAX_ORBIT_POINTS`).
pub fn orbit_misfit(
    view: &ProjectView,
    kind: &str,
    states: &[String],
    params: &[String],
) -> Option<String> {
    let has = |list: &[String], name: &Option<String>| {
        name.as_ref()
            .is_some_and(|n| list.is_empty() || list.iter().any(|s| s == n))
    };
    let ok = (kind.is_empty() || kind == "map")
        && view.x_axis.kind == "param"
        && has(params, &view.x_axis.name)
        && view.y_axis.kind == "state"
        && has(states, &view.y_axis.name)
        && view.orbit.as_ref().is_some_and(|o| {
            o.transient.is_some_and(|t| t <= ORBIT_TRANSIENT_MAX)
                && o.kept.is_some_and(|k| (1..=ORBIT_KEPT_MAX).contains(&k))
        });
    (!ok).then(|| ORBIT_MISFIT.to_string())
}

/// True when `view` has the shape of a cobweb plot: `x_axis` is the state
/// `s` and `y_axis` is `{ "kind": "next", "name": s }`, for a state the
/// system has (any, when `states` is empty).
pub fn cobweb_axes_ok(view: &ProjectView, states: &[String]) -> bool {
    let (x, y) = (&view.x_axis, &view.y_axis);
    x.kind == "state"
        && y.kind == "next"
        && x.name == y.name
        && x.name
            .as_ref()
            .is_some_and(|n| states.is_empty() || states.iter().any(|s| s == n))
}

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
        let kind = view.kind.as_deref();
        let section = kind == Some(SECTION);
        if section {
            if let Some(w) = section_misfit(view, &p.system.kind, states) {
                warnings.push(w);
                continue;
            }
        }
        let space = kind == Some(SPACE);
        if space {
            if let Some(w) = space_misfit(view, &p.system.kind, states) {
                warnings.push(w);
                continue;
            }
        }
        let lyapunov = kind == Some(LYAPUNOV);
        if lyapunov {
            if let Some(w) = lyapunov_misfit(view, &p.system.kind) {
                warnings.push(w);
                continue;
            }
        }
        let orbit = kind == Some(ORBIT);
        if orbit {
            let params: Vec<String> = p.system.params.iter().map(|q| q.id.clone()).collect();
            if let Some(w) = orbit_misfit(view, &p.system.kind, states, &params) {
                warnings.push(w);
                continue;
            }
        }
        let cobweb = !section
            && !space
            && !lyapunov
            && !orbit
            && (kind.is_some_and(|k| k != "plane")
                || [&view.x_axis, &view.y_axis]
                    .iter()
                    .any(|a| a.kind == "next"));
        if cobweb {
            let one_d_map =
                states.len() <= 1 && (p.system.kind.is_empty() || p.system.kind == "map");
            if !(kind == Some(COBWEB) && cobweb_axes_ok(view, states) && one_d_map) {
                warnings.push(COBWEB_MISFIT.to_string());
                continue;
            }
        }
        let z_range = view.z_range.filter(|_| space);
        let ranges = [("x", Some(&view.x_range)), ("y", Some(&view.y_range))];
        for (axis, r) in ranges.into_iter().chain([("z", z_range.as_ref())]) {
            let Some(r) = r else { continue };
            if !range_ok(r) {
                warnings.push(format!(
                    "The {axis} range of the view \u{201c}{}\u{201d} ({} to {}) is empty, so the default view is used.",
                    view.id,
                    show(r[0]),
                    show(r[1])
                ));
            }
        }
        let z_axis = view.z_axis.as_ref().filter(|_| space);
        let axes = [("x", Some(&view.x_axis)), ("y", Some(&view.y_axis))];
        for (name, axis) in axes.into_iter().chain([("z", z_axis)]) {
            let Some(axis) = axis else { continue };
            let known = match axis.kind.as_str() {
                "time" => true,
                // Checked above, with the view's kind.
                "next" if cobweb || section => true,
                "exponent" if lyapunov => true,
                // Checked above, with the system's parameters.
                "param" if orbit => true,
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
    match s.transient {
        Some(t) if !(0..=TRANSIENT_MAX).contains(&t) => {
            let now = t.clamp(0, TRANSIENT_MAX);
            fix("transient", show(t as f64), format!("{now} iterates"));
            s.transient = (now > 0).then_some(now);
        }
        // Written only when above 0.
        Some(0) => s.transient = None,
        _ => {}
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
