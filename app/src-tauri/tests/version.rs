//! One version for the whole app. Cargo's `[workspace.package]` version is
//! the source for Rust and Tauri (tauri.conf.json has no `version`, so Tauri
//! falls back to it); app/package.json and its lock must carry the same
//! string. It must also be a version the Windows MSI bundler accepts.

use std::path::Path;

use serde_json::Value;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn read_json(rel: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// MSI's ProductVersion is major.minor.build with major and minor at most
/// 255 and build at most 65535, and no pre-release suffix.
fn msi_safe(version: &str) -> Result<(), String> {
    let parts: Vec<&str> = version.split('.').collect();
    let limits = [255u32, 255, 65535];
    let numeric = parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if !numeric {
        return Err(format!(
            "version {version} is not MAJOR.MINOR.PATCH digits only (MSI rejects suffixes such as -rc)"
        ));
    }
    for ((part, limit), name) in parts.iter().zip(limits).zip(["major", "minor", "patch"]) {
        match part.parse::<u32>() {
            Ok(n) if n <= limit => {}
            _ => {
                return Err(format!(
                    "version {version}: {name} {part} is over MSI's limit {limit}"
                ))
            }
        }
    }
    Ok(())
}

#[test]
fn every_manifest_carries_the_cargo_version() {
    let package = read_json("../package.json");
    let lock = read_json("../package-lock.json");
    let tauri = read_json("tauri.conf.json");

    let found = [
        ("app/package.json version", &package["version"]),
        ("app/package-lock.json version", &lock["version"]),
        (
            "app/package-lock.json packages[\"\"].version",
            &lock["packages"][""]["version"],
        ),
    ];
    let mut problems: Vec<String> = found
        .iter()
        .filter(|(_, v)| v.as_str() != Some(VERSION))
        .map(|(file, v)| format!("{file} is {v}, but Cargo.toml [workspace.package] is {VERSION}"))
        .collect();
    if let Some(v) = tauri.get("version") {
        problems.push(format!(
            "app/src-tauri/tauri.conf.json sets version {v}; remove it so Tauri uses Cargo's {VERSION}"
        ));
    }
    for bad in ["1.0.0-rc.1", "1.0", "256.0.0", "1.256.0", "1.0.65536"] {
        assert!(msi_safe(bad).is_err(), "{bad} passed the MSI check");
    }
    if let Err(e) = msi_safe(VERSION) {
        problems.push(e);
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
