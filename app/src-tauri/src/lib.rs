//! Tauri shell. All maths lives in the `dyn-*` crates; this crate only
//! exposes them to the webview as commands.
//!
//! - [`ipc`]: the JSON types, mirroring `app/src/api/types.ts`.
//! - [`service`]: what the commands do, testable without a window.
//!
//! Commands (called from `app/src/api/client.ts` as
//! `invoke(name, { req })`):
//!
//! | Command | Request | Response |
//! | --- | --- | --- |
//! | `compile_system` | [`ipc::CompileRequest`] | [`ipc::CompileResponse`] |
//! | `compute_portrait` | [`ipc::PortraitRequest`] | [`ipc::PortraitResponse`] |

pub mod ipc;
pub mod service;

use service::SystemStore;
use tauri::State;

#[tauri::command]
fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Parses the equations and keeps the compiled system for later portraits.
/// Compiling takes well under a millisecond, so this runs synchronously.
#[tauri::command]
fn compile_system(req: ipc::CompileRequest, store: State<'_, SystemStore>) -> ipc::CompileResponse {
    service::compile_system(&store, &req)
}

/// Computes field, nullclines, fixed points and trajectories for one view.
///
/// This can take tens of milliseconds (long trajectories), so it runs on a
/// blocking worker thread and never stalls the window's event loop.
#[tauri::command]
async fn compute_portrait(
    req: ipc::PortraitRequest,
    store: State<'_, SystemStore>,
) -> Result<ipc::PortraitResponse, String> {
    let system = store.get(req.system_id).ok_or_else(|| {
        format!("System {} is no longer available; edit the equations to recompile.", req.system_id)
    })?;
    tauri::async_runtime::spawn_blocking(move || service::compute_portrait(&system, &req))
        .await
        .map_err(|e| format!("The computation stopped unexpectedly: {e}"))?
}

/// Starts the desktop app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SystemStore::new())
        .invoke_handler(tauri::generate_handler![core_version, compile_system, compute_portrait])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
