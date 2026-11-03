//! Tauri shell. All maths lives in the `dyn-*` crates; this crate only
//! exposes them to the webview as commands.

#[tauri::command]
fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
