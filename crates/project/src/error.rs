//! Errors, with messages for people: each says what happened and what to do.

use std::fmt;

/// Why a save failed (see [`ProjectError::Write`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteReason {
    /// The folder (or the disk) can't be written to.
    ReadOnly,
    /// The file itself is marked read-only.
    FileReadOnly,
    /// The disk is full.
    DiskFull,
    /// The folder doesn't exist any more.
    FolderMissing,
    /// Anything else, with the system's message.
    Other(String),
}

/// Everything that can go wrong opening or saving a project.
///
/// `file` is the file's name (not its path), used in the message; an empty
/// name reads as "This project".
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectError {
    /// The file doesn't exist.
    NotFound { file: String },
    /// The file can't be read.
    PermissionDenied { file: String },
    /// The file is larger than [`crate::MAX_FILE_BYTES`].
    TooLarge { file: String, bytes: u64 },
    /// Not JSON (or not UTF-8); `line` and `column` are 1-based, 0 when unknown.
    NotJson {
        file: String,
        line: usize,
        column: usize,
    },
    /// JSON, but not a Separatrix project (`format` is missing or different).
    NotProject { file: String },
    /// `format_version` or `min_reader_version` isn't a whole number ≥ 1.
    BadVersion { file: String, detail: String },
    /// The file needs a newer Separatrix (`min_reader_version` > [`crate::CURRENT_VERSION`]).
    TooNew {
        file: String,
        saved_by: Option<String>,
        min_reader_version: u32,
    },
    /// The structure is wrong; `detail` names the path, e.g.
    /// `initial_conditions[2].x0[1] should be a number`.
    Damaged { file: String, detail: String },
    /// The project can't be written (a non-finite number, or a bad shape from the UI).
    Unsaveable { detail: String },
    /// Writing failed.
    Write { file: String, reason: WriteReason },
    /// Writing failed because another program holds the file open (Windows).
    InUse { file: String },
}

/// `“x.sepx”`, or "This project" when the name is unknown.
pub(crate) fn quoted(file: &str) -> String {
    if file.is_empty() {
        "This project".to_string()
    } else {
        format!("\u{201c}{file}\u{201d}")
    }
}

/// `“x.sepx”`, or "this project" mid-sentence.
fn quoted_mid(file: &str) -> String {
    if file.is_empty() {
        "this project".to_string()
    } else {
        quoted(file)
    }
}

impl ProjectError {
    /// The same error, naming `file` (used once the caller knows the name).
    pub fn with_file(mut self, name: &str) -> Self {
        match &mut self {
            ProjectError::NotFound { file }
            | ProjectError::PermissionDenied { file }
            | ProjectError::TooLarge { file, .. }
            | ProjectError::NotJson { file, .. }
            | ProjectError::NotProject { file }
            | ProjectError::BadVersion { file, .. }
            | ProjectError::TooNew { file, .. }
            | ProjectError::Damaged { file, .. }
            | ProjectError::Write { file, .. }
            | ProjectError::InUse { file } => *file = name.to_string(),
            ProjectError::Unsaveable { .. } => {}
        }
        self
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::NotFound { file } => write!(
                f,
                "Couldn't open {}: the file doesn't exist. It may have been moved or renamed; open it from its new location.",
                quoted_mid(file)
            ),
            ProjectError::PermissionDenied { file } => write!(
                f,
                "Couldn't open {}: you don't have permission to read it. Change the file's permissions, or copy it to a folder of your own.",
                quoted_mid(file)
            ),
            ProjectError::TooLarge { file, bytes } => write!(
                f,
                "{} is {}, which is too large for a Separatrix project (the limit is {}). Check that you picked the right file.",
                quoted(file),
                megabytes(*bytes),
                megabytes(crate::MAX_FILE_BYTES)
            ),
            ProjectError::NotJson { file, line, column } => {
                write!(f, "{} isn't a Separatrix project: ", quoted(file))?;
                if *line > 0 {
                    write!(f, "it isn't valid JSON (line {line}, column {column}).")?;
                } else {
                    write!(f, "it isn't UTF-8 JSON text.")?;
                }
                write!(f, " If you edited it by hand, fix it there, or open a backup copy.")
            }
            ProjectError::NotProject { file } => write!(
                f,
                "{} isn't a Separatrix project. Open a .sepx file saved by Separatrix.",
                quoted(file)
            ),
            ProjectError::BadVersion { file, detail } => write!(
                f,
                "{} has an invalid format version: {detail}. The file may be damaged; open a backup copy.",
                quoted(file)
            ),
            ProjectError::TooNew { file, saved_by: Some(v), .. } => write!(
                f,
                "{} was saved by Separatrix {v} and needs a newer version to open. Update Separatrix, then open it again.",
                quoted(file)
            ),
            ProjectError::TooNew { file, saved_by: None, .. } => write!(
                f,
                "{} needs a newer version of Separatrix to open. Update Separatrix, then open it again.",
                quoted(file)
            ),
            ProjectError::Damaged { file, detail } => write!(
                f,
                "{} is damaged: {detail}. Fix it in a text editor, or open a backup copy.",
                quoted(file)
            ),
            ProjectError::Unsaveable { detail } => write!(
                f,
                "Couldn't save the project: {detail}. Change that value, then save again."
            ),
            ProjectError::Write { file, reason } => {
                write!(f, "Couldn't save {}: ", quoted_mid(file))?;
                match reason {
                    WriteReason::ReadOnly => write!(
                        f,
                        "the folder is read-only. Choose another folder with Save as\u{2026}"
                    ),
                    WriteReason::FileReadOnly => write!(
                        f,
                        "the file is read-only. Make it writable, or save a copy with Save as\u{2026}"
                    ),
                    WriteReason::DiskFull => {
                        write!(f, "the disk is full. Free some space, then save again.")
                    }
                    WriteReason::FolderMissing => write!(
                        f,
                        "its folder doesn't exist any more. Choose another folder with Save as\u{2026}"
                    ),
                    WriteReason::Other(msg) => {
                        write!(f, "{msg}. Try again, or choose another folder with Save as\u{2026}")
                    }
                }
            }
            ProjectError::InUse { file } => write!(
                f,
                "Couldn't save {}: it is open in another program. Close it there, then save again.",
                quoted_mid(file)
            ),
        }
    }
}

impl std::error::Error for ProjectError {}

fn megabytes(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 10.0 || mb.fract() == 0.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{mb:.1} MB")
    }
}

/// Classifies an I/O error from reading `file`.
pub(crate) fn read_error(err: &std::io::Error, file: &str) -> ProjectError {
    match err.kind() {
        std::io::ErrorKind::NotFound => ProjectError::NotFound { file: file.into() },
        std::io::ErrorKind::PermissionDenied => {
            ProjectError::PermissionDenied { file: file.into() }
        }
        std::io::ErrorKind::InvalidData => ProjectError::NotJson {
            file: file.into(),
            line: 0,
            column: 0,
        },
        _ => ProjectError::Damaged {
            file: file.into(),
            detail: format!("it couldn't be read ({err})"),
        },
    }
}

/// Classifies an I/O error from writing `file`.
pub(crate) fn write_error(err: &std::io::Error, file: &str) -> ProjectError {
    let file = file.to_string();
    if crate::io::is_read_only_file(err) {
        return ProjectError::Write {
            file,
            reason: WriteReason::FileReadOnly,
        };
    }
    if let Some(code) = err.raw_os_error() {
        if cfg!(windows) {
            // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION, ERROR_USER_MAPPED_FILE
            if matches!(code, 32 | 33 | 1224) {
                return ProjectError::InUse { file };
            }
            // ERROR_HANDLE_DISK_FULL, ERROR_DISK_FULL
            if matches!(code, 39 | 112) {
                return ProjectError::Write {
                    file,
                    reason: WriteReason::DiskFull,
                };
            }
            // ERROR_WRITE_PROTECT
            if code == 19 {
                return ProjectError::Write {
                    file,
                    reason: WriteReason::ReadOnly,
                };
            }
        } else {
            // EROFS (Linux and macOS both use 30)
            if code == 30 {
                return ProjectError::Write {
                    file,
                    reason: WriteReason::ReadOnly,
                };
            }
            // ENOSPC, EDQUOT (Linux 122, macOS 69)
            if code == 28
                || (cfg!(target_os = "linux") && code == 122)
                || (cfg!(target_os = "macos") && code == 69)
            {
                return ProjectError::Write {
                    file,
                    reason: WriteReason::DiskFull,
                };
            }
        }
    }
    let reason = match err.kind() {
        std::io::ErrorKind::PermissionDenied => WriteReason::ReadOnly,
        std::io::ErrorKind::NotFound => WriteReason::FolderMissing,
        _ => WriteReason::Other(err.to_string()),
    };
    ProjectError::Write { file, reason }
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use super::*;

    fn reason(err: Error) -> ProjectError {
        write_error(&err, "x.sepx")
    }

    #[test]
    fn write_errors_are_classified() {
        let file = String::from("x.sepx");
        assert_eq!(
            reason(Error::from(ErrorKind::PermissionDenied)),
            ProjectError::Write {
                file: file.clone(),
                reason: WriteReason::ReadOnly
            }
        );
        assert_eq!(
            reason(Error::from(ErrorKind::NotFound)),
            ProjectError::Write {
                file: file.clone(),
                reason: WriteReason::FolderMissing
            }
        );
        assert!(matches!(
            reason(Error::other("boom")),
            ProjectError::Write { reason: WriteReason::Other(ref m), .. } if m == "boom"
        ));
        #[cfg(unix)]
        {
            // EACCES, EROFS, ENOSPC
            for (code, want) in [
                (13, WriteReason::ReadOnly),
                (30, WriteReason::ReadOnly),
                (28, WriteReason::DiskFull),
            ] {
                assert_eq!(
                    reason(Error::from_raw_os_error(code)),
                    ProjectError::Write {
                        file: file.clone(),
                        reason: want
                    },
                    "errno {code}"
                );
            }
        }
        #[cfg(windows)]
        {
            for code in [32, 33] {
                assert_eq!(
                    reason(Error::from_raw_os_error(code)),
                    ProjectError::InUse { file: file.clone() }
                );
            }
            assert_eq!(
                reason(Error::from_raw_os_error(112)),
                ProjectError::Write {
                    file: file.clone(),
                    reason: WriteReason::DiskFull
                }
            );
        }
    }

    #[test]
    fn read_errors_are_classified() {
        assert_eq!(
            read_error(&Error::from(ErrorKind::NotFound), "a.sepx"),
            ProjectError::NotFound {
                file: "a.sepx".into()
            }
        );
        assert_eq!(
            read_error(&Error::from(ErrorKind::PermissionDenied), "a.sepx"),
            ProjectError::PermissionDenied {
                file: "a.sepx".into()
            }
        );
        assert!(matches!(
            read_error(&Error::other("bad sector"), "a.sepx"),
            ProjectError::Damaged { ref detail, .. } if detail == "it couldn't be read (bad sector)"
        ));
    }

    #[test]
    fn sizes_read_as_megabytes() {
        assert_eq!(megabytes(4 << 20), "4 MB");
        assert_eq!(megabytes((4 << 20) + 200_000), "4.2 MB");
        assert_eq!(megabytes(50 << 20), "50 MB");
    }
}
