//! User-facing messages about the input.

use serde::{Deserialize, Serialize};

/// How serious a [`Diagnostic`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The system cannot be built.
    Error,
    /// The system was built, but something looks unintended.
    Warning,
    /// Purely informative.
    Info,
}

/// A plain-language message about the input, optionally pointing at the
/// part of the source it is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// How serious the problem is.
    pub severity: Severity,
    /// What is wrong and how to fix it.
    pub message: String,
    /// Byte offsets `(start, end)` into the source string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

impl Diagnostic {
    /// An [`Severity::Error`] diagnostic.
    pub fn error(message: impl Into<String>, span: Option<(usize, usize)>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }

    /// A [`Severity::Warning`] diagnostic.
    pub fn warning(message: impl Into<String>, span: Option<(usize, usize)>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            message: message.into(),
            span,
        }
    }

    /// True for [`Severity::Error`].
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}
