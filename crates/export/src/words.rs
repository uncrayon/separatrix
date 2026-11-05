//! The words exports use, as the UI shows them.

use crate::model::{Classification, Status, SystemKind};

/// `classificationTitle(c, dim)` (`app/src/math/format.ts`): Strogatz's
/// name in sentence case. In 1D only stability is decided by the
/// linearization ("Stable", "Unstable"); `f′(x*) = 0` stays
/// "Non-hyperbolic".
pub fn classification_title(c: Classification, dim: usize) -> &'static str {
    use Classification::*;
    if dim == 1 {
        match c {
            Sink | StableNode | StableSpiral | StableStar | StableDegenerateNode => {
                return "Stable"
            }
            Source | UnstableNode | UnstableSpiral | UnstableStar | UnstableDegenerateNode => {
                return "Unstable"
            }
            _ => {}
        }
    }
    match c {
        Saddle => "Saddle",
        StableNode => "Stable node",
        UnstableNode => "Unstable node",
        StableSpiral => "Stable spiral",
        UnstableSpiral => "Unstable spiral",
        Centre => "Center",
        StableStar => "Stable star",
        UnstableStar => "Unstable star",
        StableDegenerateNode => "Stable degenerate node",
        UnstableDegenerateNode => "Unstable degenerate node",
        NonIsolated => "Non-isolated",
        Sink => "Sink",
        Source => "Source",
        SaddleND => "Saddle",
        NonHyperbolic => "Non-hyperbolic",
    }
}

/// The status word of a run, as in the status bar ("completed",
/// "blew up", …), lowercase for a sentence or a JSON value.
pub fn status_word(s: Status) -> &'static str {
    match s {
        Status::Completed => "completed",
        Status::BlewUp => "blew up",
        Status::MaxSteps => "maximum steps reached",
        Status::StepTooSmall => "step size too small",
        Status::NonFinite => "not finite",
    }
}

/// The status as its IPC name (`"blewUp"`), for `.meta.json`.
pub fn status_id(s: Status) -> &'static str {
    match s {
        Status::Completed => "completed",
        Status::BlewUp => "blewUp",
        Status::MaxSteps => "maxSteps",
        Status::StepTooSmall => "stepTooSmall",
        Status::NonFinite => "nonFinite",
    }
}

/// The time variable's name: `t` for flows, `n` for maps.
pub fn time_name(kind: SystemKind) -> &'static str {
    match kind {
        SystemKind::Ode => "t",
        SystemKind::Map => "n",
    }
}

/// `"a, b and c"`.
pub fn join_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_follow_the_ui() {
        assert_eq!(classification_title(Classification::Centre, 2), "Center");
        assert_eq!(classification_title(Classification::SaddleND, 3), "Saddle");
        assert_eq!(
            classification_title(Classification::StableNode, 1),
            "Stable"
        );
        assert_eq!(classification_title(Classification::Source, 1), "Unstable");
        assert_eq!(
            classification_title(Classification::NonHyperbolic, 1),
            "Non-hyperbolic"
        );
        assert_eq!(
            join_and(&["a".into(), "b".into(), "c".into()]),
            "a, b and c"
        );
        assert_eq!(join_and(&["a".into(), "b".into()]), "a and b");
    }
}
