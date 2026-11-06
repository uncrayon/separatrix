//! Updates from GitHub Releases (docs/updater.md).
//!
//! The updater is on only in a build whose config carries the owner's
//! minisign public key (`plugins.updater.pubkey`). The shipped config has
//! none, so [`updater_setup`] says [`UpdaterSetup::Off`], the plugin is not
//! registered (it fails at init without its config section) and every
//! command answers "off".

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::UpdaterExt;

use crate::files::MAIN_WINDOW;
use crate::ipc::UpdateCheck;

/// Event: the startup check found a newer release ([`UpdateCheck::Available`]).
pub const EVENT_UPDATE_AVAILABLE: &str = "update-available";
/// Event: the macOS menu's "Check for Updates…" was chosen.
pub const EVENT_CHECK_REQUESTED: &str = "update-check-requested";
/// A check gives up after this long, so being offline costs nothing.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Whether this build can update itself, decided once from the bundled
/// config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdaterSetup {
    Off,
    On,
}

/// On only when `plugins.updater.pubkey` is a non-empty string.
pub fn updater_setup(plugins: &serde_json::Value) -> UpdaterSetup {
    match plugins["updater"]["pubkey"].as_str() {
        Some(key) if !key.trim().is_empty() => UpdaterSetup::On,
        _ => UpdaterSetup::Off,
    }
}

/// Managed state: the setup, and whether the startup check has run.
pub struct Updates {
    pub setup: UpdaterSetup,
    started: AtomicBool,
}

impl Updates {
    pub fn new(setup: UpdaterSetup) -> Self {
        Self {
            setup,
            started: AtomicBool::new(false),
        }
    }
}

/// Registers the plugin when the build is on.
pub fn register<R: Runtime>(builder: tauri::Builder<R>, setup: UpdaterSetup) -> tauri::Builder<R> {
    let builder = builder.manage(Updates::new(setup));
    match setup {
        UpdaterSetup::On => builder.plugin(tauri_plugin_updater::Builder::new().build()),
        UpdaterSetup::Off => builder,
    }
}

async fn check<R: Runtime>(app: &AppHandle<R>) -> UpdateCheck {
    if app.state::<Updates>().setup == UpdaterSetup::Off {
        return UpdateCheck::Off;
    }
    let found = match app.updater_builder().timeout(TIMEOUT).build() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(update)) => UpdateCheck::Available {
            version: update.version,
            notes: update.body.filter(|b| !b.trim().is_empty()),
            date: update.raw_json["pub_date"].as_str().map(str::to_owned),
        },
        Ok(None) => UpdateCheck::UpToDate {
            version: app.package_info().version.to_string(),
        },
        Err(e) => UpdateCheck::Failed {
            message: e.to_string(),
        },
    }
}

/// "Check for updates" in the status bar and the macOS menu.
#[tauri::command]
pub async fn check_for_update<R: Runtime>(app: AppHandle<R>) -> UpdateCheck {
    let result = check(&app).await;
    if let UpdateCheck::Failed { message } = &result {
        eprintln!("separatrix: couldn't check for updates: {message}");
    }
    result
}

/// Downloads the newest release, verifies its signature, installs it and
/// restarts (on Windows the installer closes the app itself).
#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if app.state::<Updates>().setup == UpdaterSetup::Off {
        return Err("Updates are off in this build.".into());
    }
    let failed = |e: tauri_plugin_updater::Error| {
        eprintln!("separatrix: couldn't install the update: {e}");
        "Couldn't install the update. You can keep working and try again later.".to_owned()
    };
    let updater = app
        .updater_builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(failed)?;
    let Some(mut update) = updater.check().await.map_err(failed)? else {
        return Err("There is no newer version to install.".into());
    };
    // A download can take longer than a check, so it has no timeout.
    update.timeout = None;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(failed)?;
    app.restart()
}

/// The quiet check after launch: at most once per launch, never awaited by
/// anything, failures to stderr only; the UI hears only of a newer release.
pub fn spawn_startup_check<R: Runtime>(app: &AppHandle<R>) {
    let updates = app.state::<Updates>();
    if updates.setup == UpdaterSetup::Off || updates.started.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match check(&app).await {
            available @ UpdateCheck::Available { .. } => {
                if let Err(e) = app.emit_to(MAIN_WINDOW, EVENT_UPDATE_AVAILABLE, available) {
                    eprintln!("separatrix: couldn't send {EVENT_UPDATE_AVAILABLE}: {e}");
                }
            }
            UpdateCheck::Failed { message } => {
                eprintln!("separatrix: the startup update check failed: {message}");
            }
            UpdateCheck::Off | UpdateCheck::UpToDate { .. } => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_updater_section_is_off() {
        assert_eq!(updater_setup(&json!({})), UpdaterSetup::Off);
        assert_eq!(updater_setup(&json!(null)), UpdaterSetup::Off);
        assert_eq!(
            updater_setup(&json!({ "updater": { "endpoints": ["https://example.com"] } })),
            UpdaterSetup::Off
        );
    }

    #[test]
    fn an_empty_pubkey_is_off() {
        for key in [json!(""), json!("   "), json!(null), json!(42)] {
            assert_eq!(
                updater_setup(&json!({ "updater": { "pubkey": key } })),
                UpdaterSetup::Off,
                "{key}"
            );
        }
    }

    #[test]
    fn a_pubkey_is_on() {
        let key = "not-a-real-key";
        assert_eq!(
            updater_setup(&json!({ "updater": { "pubkey": key } })),
            UpdaterSetup::On
        );
    }
}
