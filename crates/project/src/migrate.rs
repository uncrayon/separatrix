//! Forward migration, one version at a time (1 → 2 → 3 …).
//!
//! A step works on the raw JSON object, before the structural check and the
//! typed conversion, so it can rename and reshape keys freely. Steps never
//! run backwards: a file from a newer format opens as the current version
//! (see [`crate::parse`]).

use serde_json::{Map, Value};

use crate::{ProjectError, CURRENT_VERSION};

/// One migration step: rewrites a document of version `n` into version
/// `n + 1`. It doesn't touch `format_version`; [`migrate_with`] bumps it.
pub type Step = fn(&mut Map<String, Value>) -> Result<(), String>;

/// The steps for this build: `STEPS[i]` migrates version `i + 1` to `i + 2`.
/// Empty while [`CURRENT_VERSION`] is 1.
pub const STEPS: [Step; (CURRENT_VERSION - 1) as usize] = [];

/// Applies `steps[from - 1 .. to - 1]` in order, bumping `format_version`
/// after each. `from >= to` is a no-op.
pub fn migrate_with(
    v: &mut Map<String, Value>,
    from: u32,
    to: u32,
    steps: &[Step],
) -> Result<(), ProjectError> {
    if from == 0 {
        return Err(ProjectError::BadVersion {
            file: String::new(),
            detail: "format_version should be at least 1".into(),
        });
    }
    for version in from..to {
        let step = steps
            .get((version - 1) as usize)
            .ok_or_else(|| ProjectError::Damaged {
                file: String::new(),
                detail: format!(
                    "this version of Separatrix can't update format version {version} to {}",
                    version + 1
                ),
            })?;
        step(v).map_err(|msg| ProjectError::Damaged {
            file: String::new(),
            detail: format!("it couldn't be updated from format version {version}: {msg}"),
        })?;
        v.insert("format_version".into(), Value::from(version + 1));
    }
    Ok(())
}

/// Applies this build's [`STEPS`].
pub(crate) fn migrate(v: &mut Map<String, Value>, from: u32) -> Result<(), ProjectError> {
    migrate_with(v, from, CURRENT_VERSION, &STEPS)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn current_build_has_one_step_per_version() {
        assert_eq!(STEPS.len(), (CURRENT_VERSION - 1) as usize);
    }

    #[test]
    fn from_current_is_a_no_op() {
        let mut v = obj(json!({ "format_version": CURRENT_VERSION, "a": 1 }));
        let before = v.clone();
        migrate(&mut v, CURRENT_VERSION).unwrap();
        assert_eq!(v, before);
    }

    #[test]
    fn version_zero_is_refused() {
        let mut v = Map::new();
        assert!(matches!(
            migrate_with(&mut v, 0, 1, &[]),
            Err(ProjectError::BadVersion { .. })
        ));
    }
}
