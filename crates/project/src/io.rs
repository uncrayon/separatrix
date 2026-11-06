//! Reading, writing and the atomic file replace.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::check::check_project;
use crate::error::{quoted, read_error, write_error};
use crate::migrate::migrate;
use crate::validate::validate;
use crate::{
    ProjectError, ProjectFile, CURRENT_VERSION, FORMAT, MAX_FILE_BYTES, MIN_READER_VERSION,
};

/// A project that opened, with what the user should be told about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Opened {
    pub project: ProjectFile,
    /// One sentence per repair or notice, for the UI's banner.
    pub warnings: Vec<String>,
    /// The file's `format_version` when it was migrated forward.
    pub migrated_from: Option<u32>,
    /// The file's `format_version` when it is newer than this build's.
    pub newer_format: Option<u32>,
}

/// Parses a project from text. Messages say "This project"; use
/// [`parse_named`] when the file name is known.
pub fn parse(text: &str) -> Result<Opened, ProjectError> {
    parse_named(text, "")
}

/// Parses a project from text, naming `file` (e.g. `"x.sepx"`) in errors
/// and warnings.
pub fn parse_named(text: &str, file: &str) -> Result<Opened, ProjectError> {
    parse_inner(text, file).map_err(|e| e.with_file(file))
}

fn parse_inner(text: &str, file: &str) -> Result<Opened, ProjectError> {
    // 1. Size, BOM, JSON (serde_json's recursion limit of 128 stays on).
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(ProjectError::TooLarge {
            file: String::new(),
            bytes: text.len() as u64,
        });
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: Value = serde_json::from_str(text).map_err(|e| ProjectError::NotJson {
        file: String::new(),
        line: e.line(),
        column: e.column(),
    })?;
    // 2. Format.
    let Value::Object(mut map) = value else {
        return Err(ProjectError::NotProject {
            file: String::new(),
        });
    };
    if map.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(ProjectError::NotProject {
            file: String::new(),
        });
    }
    // 3. Versions.
    let format_version = version_field(&map, "format_version")?
        .ok_or_else(|| bad_version("format_version is missing".into()))?;
    let min_reader = match version_field(&map, "min_reader_version")? {
        Some(v) => v,
        None => {
            map.insert("min_reader_version".into(), Value::from(format_version));
            format_version
        }
    };
    let saved_by = map
        .get("app_version")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string);
    // 4. Needs a newer reader: refused, whatever the format version.
    if min_reader > CURRENT_VERSION {
        return Err(ProjectError::TooNew {
            file: String::new(),
            saved_by,
            min_reader_version: min_reader,
        });
    }
    let mut warnings = Vec::new();
    let mut newer_format = None;
    let mut migrated_from = None;
    if format_version > CURRENT_VERSION {
        // 5. Newer, but readable: open as the current version.
        newer_format = Some(format_version);
        warnings.push(match &saved_by {
            Some(v) => format!(
                "{} was saved in a newer format by Separatrix {v}. It opens, and saving keeps the parts this version doesn't use.",
                quoted(file)
            ),
            None => format!(
                "{} was saved in a newer format (version {format_version}). It opens, and saving keeps the parts this version doesn't use.",
                quoted(file)
            ),
        });
    } else if format_version < CURRENT_VERSION {
        // 6. Older: migrate forward step by step.
        migrate(&mut map, format_version)?;
        migrated_from = Some(format_version);
        warnings.push(format!(
            "{} was saved by an older version of Separatrix. It was updated to the current format; saving writes the new format.",
            quoted(file)
        ));
    }
    // 7–8. Structure, then types.
    let mut project = typed(Value::Object(map))?;
    // 9. Repairs.
    warnings.extend(validate(&mut project));
    Ok(Opened {
        project,
        warnings,
        migrated_from,
        newer_format,
    })
}

fn bad_version(detail: String) -> ProjectError {
    ProjectError::BadVersion {
        file: String::new(),
        detail,
    }
}

/// Reads an optional version field: a whole number from 1 to 4294967295
/// (`1.0`, as other tools may write it, counts as 1).
fn version_field(map: &Map<String, Value>, key: &str) -> Result<Option<u32>, ProjectError> {
    match map.get(key) {
        None => Ok(None),
        Some(v) => match crate::check::whole_number(v).and_then(|n| u32::try_from(n).ok()) {
            Some(n) if n >= 1 => Ok(Some(n)),
            _ => Err(bad_version(format!(
                "{key} should be a whole number from 1 to 4294967295, not {v}"
            ))),
        },
    }
}

/// The structural check, then the typed conversion.
fn typed(mut v: Value) -> Result<ProjectFile, ProjectError> {
    check_project(&mut v).map_err(|detail| ProjectError::Damaged {
        file: String::new(),
        detail,
    })?;
    serde_json::from_value(v).map_err(|e| ProjectError::Damaged {
        file: String::new(),
        detail: e.to_string(),
    })
}

/// Converts a project value (from the UI, on save) into the typed file:
/// the structural check with path-precise messages, then the typed
/// conversion. No repairs run. Errors are [`ProjectError::Unsaveable`].
pub fn from_value_checked(v: Value) -> Result<ProjectFile, ProjectError> {
    typed(v).map_err(|e| match e {
        ProjectError::Damaged { detail, .. } => ProjectError::Unsaveable { detail },
        other => other,
    })
}

/// Reads and opens a project file.
pub fn read_file(path: &Path) -> Result<Opened, ProjectError> {
    let file = file_name(path);
    let meta = fs::metadata(path).map_err(|e| read_error(&e, &file))?;
    if meta.is_dir() {
        return Err(ProjectError::NotProject { file });
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(ProjectError::TooLarge {
            file,
            bytes: meta.len(),
        });
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|f| f.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| read_error(&e, &file))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ProjectError::TooLarge {
            file,
            bytes: bytes.len() as u64,
        });
    }
    let text = String::from_utf8(bytes).map_err(|_| ProjectError::NotJson {
        file: file.clone(),
        line: 0,
        column: 0,
    })?;
    parse_named(&text, &file)
}

/// The file's name for messages (`x.sepx`), lossy for non-UTF-8 names.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Serializes a project: pretty-printed JSON (2-space indent, LF line
/// endings) with a trailing newline. Known keys come in struct order, then
/// the unknown ones, sorted.
///
/// Stamps the version fields: `format = "separatrix"`, `format_version =
/// max(doc, CURRENT)`, `min_reader_version` kept from a newer-format
/// document and [`MIN_READER_VERSION`] otherwise, `app_version` =
/// `app_version`. Refuses non-finite numbers, which JSON can't hold.
pub fn to_json(p: &ProjectFile, app_version: &str) -> Result<String, ProjectError> {
    if let Some(path) = first_non_finite(p) {
        return Err(ProjectError::Unsaveable {
            detail: format!("{path} isn't a finite number"),
        });
    }
    let mut doc = p.clone();
    doc.format = FORMAT.to_string();
    // A newer-format file keeps its own version and reader requirement.
    if doc.format_version <= CURRENT_VERSION {
        doc.format_version = CURRENT_VERSION;
        doc.min_reader_version = MIN_READER_VERSION;
    }
    doc.app_version = app_version.to_string();
    let mut out = serde_json::to_string_pretty(&doc).map_err(|e| ProjectError::Unsaveable {
        detail: e.to_string(),
    })?;
    out.push('\n');
    Ok(out)
}

/// The path of the first non-finite number in the typed fields (the
/// unknown ones are JSON values, which can't hold one).
fn first_non_finite(p: &ProjectFile) -> Option<String> {
    let bad = |v: f64| !v.is_finite();
    for (i, q) in p.system.params.iter().enumerate() {
        for (k, v) in [("value", q.value), ("min", q.min), ("max", q.max)] {
            if bad(v) {
                return Some(format!("system.params[{i}].{k}"));
            }
        }
    }
    for (i, ic) in p.initial_conditions.iter().enumerate() {
        if let Some(j) = ic.x0.iter().position(|v| bad(*v)) {
            return Some(format!("initial_conditions[{i}].x0[{j}]"));
        }
        if bad(ic.t0) {
            return Some(format!("initial_conditions[{i}].t0"));
        }
    }
    for (i, v) in p.views.iter().enumerate() {
        for (k, r) in [("x_range", v.x_range), ("y_range", v.y_range)] {
            if let Some(j) = r.iter().position(|x| bad(*x)) {
                return Some(format!("views[{i}].{k}[{j}]"));
            }
        }
        if let Some((name, _)) = v.slice.states.iter().find(|(_, x)| bad(**x)) {
            return Some(format!("views[{i}].slice.states.{name}"));
        }
        if bad(v.slice.t) {
            return Some(format!("views[{i}].slice.t"));
        }
        if let Some(j) = v.z_range.and_then(|r| r.iter().position(|x| bad(*x))) {
            return Some(format!("views[{i}].z_range[{j}]"));
        }
        if let Some(c) = &v.camera {
            for (k, x) in [("yaw", c.yaw), ("pitch", c.pitch), ("zoom", c.zoom)] {
                if bad(x) {
                    return Some(format!("views[{i}].camera.{k}"));
                }
            }
        }
        if let Some(l) = &v.lyapunov {
            for (k, x) in [
                ("transient", l.transient),
                ("total", l.total),
                ("interval", l.interval),
            ] {
                if x.is_some_and(bad) {
                    return Some(format!("views[{i}].lyapunov.{k}"));
                }
            }
        }
        if let Some(s) = &v.section {
            for (k, x) in [("value", s.value), ("period", s.period), ("phase", s.phase)] {
                if x.is_some_and(bad) {
                    return Some(format!("views[{i}].section.{k}"));
                }
            }
        }
    }
    let s = &p.solver;
    for (k, v) in [
        ("rtol", s.rtol),
        ("atol", s.atol),
        ("h", s.h),
        ("t_end", s.t_end),
    ] {
        if bad(v) {
            return Some(format!("solver.{k}"));
        }
    }
    None
}

/// Serializes `p` (see [`to_json`]) and writes it with [`write_atomic`].
pub fn write_file(path: &Path, p: &ProjectFile, app_version: &str) -> Result<(), ProjectError> {
    let file = file_name(path);
    let text = to_json(p, app_version)?;
    write_atomic(path, text.as_bytes()).map_err(|e| write_error(&e, &file))
}

/// The error [`write_atomic`] returns (kind `PermissionDenied`) for a target
/// that is marked read-only, on every OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOnlyFile;

impl std::fmt::Display for ReadOnlyFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the file is read-only")
    }
}

impl std::error::Error for ReadOnlyFile {}

/// `err` is [`write_atomic`]'s refusal of a read-only file.
pub fn is_read_only_file(err: &std::io::Error) -> bool {
    err.get_ref().is_some_and(|e| e.is::<ReadOnlyFile>())
}

/// Where a write to `path` lands: `path` itself, or the file a symlink (or
/// a chain of them) points to, even when that file doesn't exist yet.
fn resolve_target(path: &Path) -> std::io::Result<PathBuf> {
    let mut p = path.to_path_buf();
    for _ in 0..40 {
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                let to = fs::read_link(&p)?;
                p = match p.parent() {
                    Some(dir) if to.is_relative() => dir.join(to),
                    _ => to,
                };
            }
            _ => return Ok(p),
        }
    }
    Err(std::io::Error::other("too many levels of symbolic links"))
}

/// Replaces `path` with `bytes` so that a reader sees either the old file
/// or the new one, never a partial write:
///
/// 1. An existing target is canonicalized (a symlink's target is written,
///    and the link stays a link, also when it points to a file that doesn't
///    exist yet), and its permissions are read. A target marked read-only is
///    refused with [`ReadOnlyFile`] (as Windows refuses to replace it), not
///    silently replaced.
/// 2. The bytes go to `.sepx-<pid>-<nanos>-<n>.tmp` in the same folder
///    (`create_new`, `write_all`, `sync_all`); on Unix the temp file gets the
///    original's permissions. The name is short whatever the target's, so a
///    target name near the system's limit (255 bytes) still saves.
/// 3. The temp file is renamed over the target (`MoveFileExW` with
///    `MOVEFILE_REPLACE_EXISTING` on Windows).
/// 4. On Unix the folder is synced (best effort).
///
/// On any error the temp file is removed and the original is untouched.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let (target, original) = match fs::canonicalize(path) {
        Ok(real) => {
            let meta = fs::metadata(&real)?;
            (real, Some(meta))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (resolve_target(path)?, None),
        Err(e) => return Err(e),
    };
    if original
        .as_ref()
        .is_some_and(|m| m.is_file() && m.permissions().readonly())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            ReadOnlyFile,
        ));
    }
    if target.file_name().is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the path has no file name",
        ));
    }
    let dir = match target.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".sepx-{}-{nanos}-{n}.tmp", std::process::id()));

    let result =
        write_temp(&tmp, bytes, original.as_ref()).and_then(|()| fs::rename(&tmp, &target));
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    #[cfg(unix)]
    {
        let _ = fs::File::open(&dir).and_then(|d| d.sync_all());
    }
    Ok(())
}

fn write_temp(tmp: &Path, bytes: &[u8], original: Option<&fs::Metadata>) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(meta) = original {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // Never wider than the original while the bytes are written.
        options.mode(meta.permissions().mode() & 0o777);
    }
    let mut f = options.open(tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    #[cfg(unix)]
    if let Some(meta) = original {
        // The umask may have narrowed the mode above: set it exactly.
        fs::set_permissions(tmp, meta.permissions())?;
    }
    #[cfg(not(unix))]
    let _ = original;
    Ok(())
}
