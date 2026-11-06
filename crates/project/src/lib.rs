//! The Separatrix project file (`.sepx`): one pretty-printed JSON file that
//! stores the system as typed, its parameters, initial conditions, views,
//! analyses and solver settings, and never computed data.
//!
//! The format is documented in `docs/project-format.md`. In short:
//!
//! - [`parse`] / [`read_file`] check the size and the format, refuse a file
//!   that needs a newer reader ([`ProjectError::TooNew`]), open a newer but
//!   readable format as the current one, migrate older ones forward step by
//!   step ([`migrate_with`]), run the structural check (path-precise
//!   messages), convert to the typed [`ProjectFile`] and repair what can be
//!   repaired, with a warning for each repair.
//! - [`to_json`] / [`write_file`] stamp the version fields and write the
//!   file atomically ([`write_atomic`]).
//! - Unknown keys at every level are kept on save (value-level: they come
//!   back with equal values, sorted by key).
//!
//! The crate depends only on serde and serde_json (with `float_roundtrip`,
//! so a second save of an unchanged file is byte-identical).

mod check;
mod error;
mod io;
mod migrate;
mod schema;
mod validate;

pub use error::{ProjectError, WriteReason};
pub use io::{
    file_name, from_value_checked, is_read_only_file, parse, parse_named, read_file, to_json,
    write_atomic, write_file, Opened, ReadOnlyFile,
};
pub use migrate::{migrate_with, Step, STEPS};
pub use schema::{
    ProjectAxis, ProjectCamera, ProjectFile, ProjectInitialCondition, ProjectLayers,
    ProjectLyapunov, ProjectParam, ProjectSection, ProjectSlice, ProjectSolver, ProjectSystem,
    ProjectView, DEFAULT_PITCH, DEFAULT_YAW, DEFAULT_ZOOM,
};
pub use validate::{
    cobweb_axes_ok, default_param_range, lyapunov_misfit, primary_view_index, range_ok,
    section_misfit, space_misfit, COBWEB, COBWEB_MISFIT, LYAPUNOV, LYAPUNOV_MISFIT, SECTION,
    SECTION_MISFIT, SPACE, SPACE_MISFIT, TRANSIENT_MAX,
};

/// The value of the `format` key.
pub const FORMAT: &str = "separatrix";
/// The schema version this build writes.
pub const CURRENT_VERSION: u32 = 1;
/// The `min_reader_version` this build writes for a current-format file.
pub const MIN_READER_VERSION: u32 = 1;
/// Larger files are refused (a project is a few kilobytes).
pub const MAX_FILE_BYTES: u64 = 4 << 20;
/// The solver methods this build knows; any other value falls back to
/// `dopri5` with a warning. Mirrors `dyn_solve::Method`'s lowercase serde
/// names (the app crate tests the parity).
pub const KNOWN_METHODS: [&str; 7] = [
    "euler",
    "heun",
    "rk4",
    "dopri5",
    "verlet",
    "yoshida4",
    "rosenbrock23",
];
/// The file extension, without the dot.
pub const EXTENSION: &str = "sepx";

#[cfg(all(test, feature = "ts"))]
mod bindings {
    use ts_rs::{Config, TS};

    use super::*;

    #[test]
    fn typescript_declarations_skip_extra_and_keep_snake_case() {
        let cfg = Config::new().with_large_int("number");
        let file = ProjectFile::decl(&cfg);
        assert!(file.contains("format_version: number"), "{file}");
        assert!(file.contains("analyses: unknown[]"), "{file}");
        assert!(file.contains("export_settings?: unknown"), "{file}");
        let decls = [
            file,
            ProjectSystem::decl(&cfg),
            ProjectParam::decl(&cfg),
            ProjectInitialCondition::decl(&cfg),
            ProjectView::decl(&cfg),
            ProjectAxis::decl(&cfg),
            ProjectSlice::decl(&cfg),
            ProjectLayers::decl(&cfg),
            ProjectSolver::decl(&cfg),
            ProjectSection::decl(&cfg),
            ProjectCamera::decl(&cfg),
            ProjectLyapunov::decl(&cfg),
        ];
        for d in &decls {
            assert!(!d.contains("extra"), "{d}");
        }
        assert!(decls[5].contains("name?: string"), "{}", decls[5]);
        assert!(decls[4].contains("kind?: string"), "{}", decls[4]);
        assert!(decls[8].contains("transient?: number"), "{}", decls[8]);
        assert!(decls[1].contains("template: string | null"), "{}", decls[1]);
        assert!(
            decls[4].contains("section?: ProjectSection"),
            "{}",
            decls[4]
        );
        assert!(decls[9].contains("value?: number"), "{}", decls[9]);
        assert!(decls[4].contains("camera?: ProjectCamera"), "{}", decls[4]);
        assert!(
            decls[4].contains("z_range?: [number, number]"),
            "{}",
            decls[4]
        );
        assert!(decls[10].contains("zoom: number"), "{}", decls[10]);
        assert!(
            decls[4].contains("lyapunov?: ProjectLyapunov"),
            "{}",
            decls[4]
        );
        assert!(decls[11].contains("blocks?: number"), "{}", decls[11]);
        assert!(
            decls[4].contains("x_range: [number, number]"),
            "{}",
            decls[4]
        );
    }
}
