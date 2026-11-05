//! The atomic write, and reading and writing real files.

mod common;

use std::fs;

use common::{fixture, fixture_path, listing, temp_dir, APP};
use separatrix_project::{
    is_read_only_file, parse, read_file, write_atomic, write_file, ProjectError, WriteReason,
    MAX_FILE_BYTES,
};

#[test]
fn creates_a_new_file_and_leaves_no_temp_file() {
    let dir = temp_dir("new");
    let path = dir.join("a.sepx");
    write_atomic(&path, b"hello\n").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"hello\n");
    assert_eq!(listing(&dir), ["a.sepx"]);
}

#[test]
fn replaces_an_existing_file() {
    let dir = temp_dir("replace");
    let path = dir.join("a.sepx");
    fs::write(&path, "old contents that are longer than the new ones").unwrap();
    write_atomic(&path, b"new").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    for i in 0..20 {
        write_atomic(&path, format!("v{i}").as_bytes()).unwrap();
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), "v19");
    assert_eq!(listing(&dir), ["a.sepx"]);
}

#[cfg(unix)]
#[test]
fn keeps_the_original_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("perms");
    // (0o444 is refused: see `a_read_only_file_is_refused_and_left_untouched`.)
    for mode in [0o600, 0o640, 0o644, 0o755] {
        let path = dir.join(format!("p{mode:o}.sepx"));
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let got = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(got, mode, "mode {mode:o}");
    }
}

#[cfg(unix)]
#[test]
fn writes_through_a_symlink_and_keeps_the_link() {
    let dir = temp_dir("symlink");
    let real_dir = dir.join("real");
    fs::create_dir(&real_dir).unwrap();
    let target = real_dir.join("t.sepx");
    fs::write(&target, "old").unwrap();
    let link = dir.join("link.sepx");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    write_atomic(&link, b"new").unwrap();
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    assert_eq!(listing(&dir), ["link.sepx", "real"]);
    assert_eq!(listing(&real_dir), ["t.sepx"]);
}

#[test]
fn a_failed_rename_removes_the_temp_file_and_keeps_the_original() {
    // Renaming a file over a non-empty folder fails on every OS.
    let dir = temp_dir("fail");
    let target = dir.join("x.sepx");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.txt"), "kept").unwrap();
    assert!(write_atomic(&target, b"new").is_err());
    assert_eq!(listing(&dir), ["x.sepx"]);
    assert_eq!(fs::read_to_string(target.join("keep.txt")).unwrap(), "kept");
}

#[test]
fn a_missing_folder_is_an_error_with_no_side_effects() {
    let dir = temp_dir("missing");
    let path = dir.join("nope").join("a.sepx");
    let err = write_atomic(&path, b"x").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(listing(&dir), Vec::<String>::new());

    let p = parse(&fixture("rabbits-vs-sheep.sepx")).unwrap().project;
    let err = write_file(&path, &p, APP).unwrap_err();
    assert_eq!(
        err,
        ProjectError::Write {
            file: "a.sepx".into(),
            reason: WriteReason::FolderMissing
        }
    );
    assert_eq!(
        err.to_string(),
        "Couldn't save \u{201c}a.sepx\u{201d}: its folder doesn't exist any more. Choose another folder with Save as\u{2026}"
    );
}

#[cfg(unix)]
#[test]
fn a_read_only_folder_is_reported_and_left_untouched() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("readonly");
    let path = dir.join("r.sepx");
    fs::write(&path, "old").unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    // root ignores folder permissions: then there is nothing to test.
    let probe = dir.join("probe");
    let writable = fs::write(&probe, "").is_ok();
    let _ = fs::remove_file(&probe);
    let p = parse(&fixture("rabbits-vs-sheep.sepx")).unwrap().project;
    let result = write_file(&path, &p, APP);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        eprintln!("skipped: the folder is writable even at 0555 (running as root)");
        result.unwrap();
        return;
    }
    let err = result.unwrap_err();
    assert_eq!(
        err,
        ProjectError::Write {
            file: "r.sepx".into(),
            reason: WriteReason::ReadOnly
        }
    );
    assert_eq!(
        err.to_string(),
        "Couldn't save \u{201c}r.sepx\u{201d}: the folder is read-only. Choose another folder with Save as\u{2026}"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(listing(&dir), ["r.sepx"]);
}

#[test]
fn write_file_then_read_file_round_trips() {
    let dir = temp_dir("roundtrip");
    let path = dir.join("lorenz.sepx");
    let opened = read_file(&fixture_path("lorenz-text-mode.sepx")).unwrap();
    write_file(&path, &opened.project, APP).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        fixture("lorenz-text-mode.sepx")
    );
    let again = read_file(&path).unwrap();
    assert_eq!(again, opened);
}

#[test]
fn write_file_refuses_non_finite_numbers_without_touching_the_file() {
    let dir = temp_dir("unsaveable");
    let path = dir.join("u.sepx");
    fs::write(&path, "old").unwrap();
    let mut p = parse(&fixture("rabbits-vs-sheep.sepx")).unwrap().project;
    p.solver.t_end = f64::INFINITY;
    assert!(matches!(
        write_file(&path, &p, APP),
        Err(ProjectError::Unsaveable { .. })
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(listing(&dir), ["u.sepx"]);
}

#[test]
fn read_file_errors() {
    let dir = temp_dir("read");
    let missing = read_file(&dir.join("gone.sepx")).unwrap_err();
    assert_eq!(
        missing,
        ProjectError::NotFound {
            file: "gone.sepx".into()
        }
    );
    assert_eq!(
        missing.to_string(),
        "Couldn't open \u{201c}gone.sepx\u{201d}: the file doesn't exist. It may have been moved or renamed; open it from its new location."
    );

    let folder = dir.join("folder.sepx");
    fs::create_dir(&folder).unwrap();
    assert_eq!(
        read_file(&folder).unwrap_err(),
        ProjectError::NotProject {
            file: "folder.sepx".into()
        }
    );

    let big = dir.join("big.sepx");
    let f = fs::File::create(&big).unwrap();
    f.set_len(MAX_FILE_BYTES + 1).unwrap();
    assert_eq!(
        read_file(&big).unwrap_err(),
        ProjectError::TooLarge {
            file: "big.sepx".into(),
            bytes: MAX_FILE_BYTES + 1
        }
    );

    let latin1 = dir.join("latin1.sepx");
    fs::write(&latin1, b"{\"format\": \"separatrix\", \"x\": \"caf\xe9\"}").unwrap();
    let err = read_file(&latin1).unwrap_err();
    assert_eq!(
        err,
        ProjectError::NotJson {
            file: "latin1.sepx".into(),
            line: 0,
            column: 0
        }
    );
    assert_eq!(
        err.to_string(),
        "\u{201c}latin1.sepx\u{201d} isn't a Separatrix project: it isn't UTF-8 JSON text. If you edited it by hand, fix it there, or open a backup copy."
    );

    let named = read_file(&fixture_path("not-json.sepx")).unwrap_err();
    assert!(matches!(named, ProjectError::NotJson { ref file, .. } if file == "not-json.sepx"));
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_is_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("unreadable");
    let path = dir.join("secret.sepx");
    fs::write(&path, fixture("rabbits-vs-sheep.sepx")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let result = read_file(&path);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    match result {
        Ok(_) => eprintln!("skipped: the file is readable at 0000 (running as root)"),
        Err(err) => {
            assert_eq!(
                err,
                ProjectError::PermissionDenied {
                    file: "secret.sepx".into()
                }
            );
            assert!(err
                .to_string()
                .contains("you don't have permission to read it"));
        }
    }
}

#[test]
fn other_error_messages() {
    let in_use = ProjectError::InUse {
        file: "x.sepx".into(),
    };
    assert_eq!(
        in_use.to_string(),
        "Couldn't save \u{201c}x.sepx\u{201d}: it is open in another program. Close it there, then save again."
    );
    let full = ProjectError::Write {
        file: "x.sepx".into(),
        reason: WriteReason::DiskFull,
    };
    assert_eq!(
        full.to_string(),
        "Couldn't save \u{201c}x.sepx\u{201d}: the disk is full. Free some space, then save again."
    );
    let unnamed = ProjectError::Write {
        file: String::new(),
        reason: WriteReason::ReadOnly,
    };
    assert!(unnamed
        .to_string()
        .starts_with("Couldn't save this project: the folder is read-only."));
    // Unsaveable has no file; with_file leaves it alone.
    let u = ProjectError::Unsaveable { detail: "d".into() }.with_file("x.sepx");
    assert_eq!(u, ProjectError::Unsaveable { detail: "d".into() });
}

#[test]
fn a_read_only_file_is_refused_and_left_untouched() {
    let dir = temp_dir("readonly-file");
    let path = dir.join("r.sepx");
    fs::write(&path, "old").unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&path, perms).unwrap();
    let err = write_atomic(&path, b"new").unwrap_err();
    assert!(is_read_only_file(&err), "{err}");
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    let p = parse(&fixture("rabbits-vs-sheep.sepx")).unwrap().project;
    let err = write_file(&path, &p, APP).unwrap_err();
    assert_eq!(
        err,
        ProjectError::Write {
            file: "r.sepx".into(),
            reason: WriteReason::FileReadOnly
        }
    );
    assert_eq!(
        err.to_string(),
        "Couldn't save \u{201c}r.sepx\u{201d}: the file is read-only. Make it writable, or save a copy with Save as\u{2026}"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(listing(&dir), ["r.sepx"]);
    let mut perms = fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(&path, perms).unwrap();
}

#[test]
fn a_long_name_saves_and_replaces() {
    // 80 CJK characters + ".sepx" = 245 bytes, near the 255-byte limit; the
    // temp file's name must not grow with it.
    let dir = temp_dir("long");
    for name in [
        format!("{}.sepx", "\u{6f22}".repeat(80)),
        format!("{}.sepx", "a".repeat(250)),
    ] {
        let path = dir.join(&name);
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
    }
    assert_eq!(listing(&dir).len(), 2);
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_is_written_through_and_stays_a_link() {
    let dir = temp_dir("dangling");
    let real_dir = dir.join("real");
    fs::create_dir(&real_dir).unwrap();
    let link = dir.join("link.sepx");
    // A relative link to a file that doesn't exist yet.
    std::os::unix::fs::symlink("real/t.sepx", &link).unwrap();
    write_atomic(&link, b"new").unwrap();
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(real_dir.join("t.sepx")).unwrap(), "new");
    assert_eq!(listing(&real_dir), ["t.sepx"]);
}
