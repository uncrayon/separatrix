//! Tauri shell. All maths lives in the `dyn-*` crates, the project format in
//! `separatrix-project` and the exports in `separatrix-export`; this crate
//! only exposes them to the webview as commands.
//!
//! - [`ipc`]: the JSON types, mirroring `app/src/api/types.ts`.
//! - [`service`]: what the maths commands do, testable without a window.
//! - [`export_service`]: what the export commands do, testable without a window.
//! - [`files`]: launch arguments, single instance, macOS `Opened`, drops,
//!   the close guard and the paths the webview may read or write.
//!
//! Commands (called from `app/src/api/client.ts` as `invoke(name, { req })`;
//! see `docs/contracts.md`):
//!
//! | Command | Request | Response |
//! | --- | --- | --- |
//! | `compile_system` | [`ipc::CompileRequest`] | [`ipc::CompileResponse`] |
//! | `convert_source` | [`ipc::ConvertRequest`] | [`ipc::ConvertResponse`] |
//! | `compute_portrait` | [`ipc::PortraitRequest`] | [`ipc::PortraitResponse`] |
//! | `compute_lyapunov` | [`ipc::LyapunovRequest`] | [`ipc::LyapunovResponse`] |
//! | `compute_orbit_diagram` | [`ipc::OrbitDiagramRequest`] | [`ipc::OrbitDiagramResponse`] |
//! | `project_choose_open_path` | none | `string \| null` |
//! | `project_read` | [`ipc::ProjectReadRequest`] | [`ipc::OpenedProject`] |
//! | `project_choose_save_path` | [`ipc::ProjectChooseSaveRequest`] | [`ipc::ChosenSavePath`] or `null` |
//! | `project_write` | [`ipc::ProjectWriteRequest`] | [`ipc::SavedProject`] |
//! | `launch_ready` | none | `string[]` |
//! | `set_document_state` | [`ipc::DocumentStateRequest`] | `()` |
//! | `close_window` | none | `()` |
//! | `compute_export_portrait` | [`ipc::ExportPortraitRequest`] | [`ipc::PortraitResponse`] |
//! | `export_figure_latex` | [`ipc::FigureLatexRequest`] | [`ipc::FigureLatexResponse`] |
//! | `save_figure_latex` | [`ipc::SaveFigureRequest`] | [`ipc::SavedFiles`] or `null` |
//! | `export_system_latex` | [`ipc::SystemLatexRequest`] | [`ipc::SystemLatexResponse`] |
//! | `export_fixed_points_latex` | [`ipc::FixedPointsLatexRequest`] | `string` |
//! | `export_csv` | [`ipc::CsvRequest`] | [`ipc::CsvResponse`] |
//! | `save_csv` | [`ipc::SaveCsvRequest`] | [`ipc::SavedFiles`] or `null` |
//! | `save_text_file` | [`ipc::SaveTextRequest`] | [`ipc::SavedFiles`] or `null` |
//! | `save_png_file` | raw PNG body, header `x-sepx-name` | [`ipc::SavedFiles`] or `null` |
//! | `clipboard_write_text` | [`ipc::ClipboardTextRequest`] | `()` |
//! | `clipboard_write_png` | raw PNG body | `()` |
//!
//! Every plugin (dialog, clipboard, single instance) is used from Rust
//! only, so no plugin command is exposed to the webview (the capability
//! stays `core:default`).

pub mod export_service;
pub mod files;
pub mod ipc;
pub mod service;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use export_service::ExportCache;
use files::{DocState, GrantedPaths, LaunchFiles, MAIN_WINDOW};
use separatrix_export::Placement;
use service::SystemStore;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder};

#[tauri::command]
fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn system_or_err(store: &SystemStore, id: u32) -> Result<Arc<dyn_expr::CompiledSystem>, String> {
    store.get(id).ok_or_else(|| {
        format!("System {id} is no longer available; edit the equations to recompile.")
    })
}

/// Runs `job` on a blocking worker thread, so the window's event loop never
/// stalls (and a dialog never runs on the main thread).
async fn blocking<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| format!("The computation stopped unexpectedly: {e}"))?
}

/// Parses the equations and keeps the compiled system for later portraits.
#[tauri::command(async)]
fn compile_system(req: ipc::CompileRequest, store: State<'_, SystemStore>) -> ipc::CompileResponse {
    service::compile_system(&store, &req)
}

/// Rewrites the equations in the other input format, for a mode switch.
#[tauri::command(async)]
fn convert_source(req: ipc::ConvertRequest) -> ipc::ConvertResponse {
    service::convert_source(&req)
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
    let system = system_or_err(&store, req.system_id)?;
    blocking(move || service::compute_portrait(&system, &req)).await
}

/// Measures the Lyapunov spectrum of one orbit: seconds of work for long
/// runs, so it runs on a blocking worker thread.
#[tauri::command]
async fn compute_lyapunov(
    req: ipc::LyapunovRequest,
    store: State<'_, SystemStore>,
) -> Result<ipc::LyapunovResponse, String> {
    let system = system_or_err(&store, req.system_id)?;
    blocking(move || service::compute_lyapunov(&system, &req)).await
}

/// Iterates a map across a parameter range: up to millions of iterates, so
/// it runs on a blocking worker thread.
#[tauri::command]
async fn compute_orbit_diagram(
    req: ipc::OrbitDiagramRequest,
    store: State<'_, SystemStore>,
) -> Result<ipc::OrbitDiagramResponse, String> {
    let system = system_or_err(&store, req.system_id)?;
    blocking(move || service::compute_orbit_diagram(&system, &req)).await
}

// ---------------------------------------------------------------------
// Dialogs
// ---------------------------------------------------------------------

/// The folder the last dialog ended in (this session only).
#[derive(Default)]
struct LastDir(Mutex<Option<PathBuf>>);

impl LastDir {
    fn get(&self) -> Option<PathBuf> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn remember(&self, path: &Path) {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.to_path_buf());
        }
    }
}

enum DialogKind {
    Open,
    Save,
}

/// Shows a file dialog over the main window, on a blocking thread, starting
/// in `dir` (or the last folder used). `filter` is (name, extensions).
async fn file_dialog<R: Runtime>(
    app: &AppHandle<R>,
    kind: DialogKind,
    filter: (&'static str, &'static [&'static str]),
    file_name: Option<String>,
    dir: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let dir = dir.or_else(|| app.state::<LastDir>().get());
    let app2 = app.clone();
    let picked = blocking(move || {
        let mut d: FileDialogBuilder<R> = app2.dialog().file().add_filter(filter.0, filter.1);
        if let Some(w) = app2.get_webview_window(MAIN_WINDOW) {
            d = d.set_parent(&w);
        }
        if let Some(dir) = dir {
            d = d.set_directory(dir);
        }
        if let Some(name) = file_name {
            d = d.set_file_name(name);
        }
        let picked = match kind {
            DialogKind::Open => d.blocking_pick_file(),
            DialogKind::Save => d.blocking_save_file(),
        };
        match picked {
            None => Ok(None),
            Some(p) => p
                .into_path()
                .map(Some)
                .map_err(|e| format!("That location can't be used: {e}")),
        }
    })
    .await?;
    if let Some(p) = &picked {
        app.state::<LastDir>().remember(p);
    }
    Ok(picked)
}

const PROJECT_FILTER: (&str, &[&str]) = ("Separatrix project", &["sepx"]);

fn text_filter(ext: ipc::TextExtension) -> ((&'static str, &'static [&'static str]), &'static str) {
    match ext {
        ipc::TextExtension::Tex => (("LaTeX", &["tex"]), "tex"),
        ipc::TextExtension::Svg => (("SVG image", &["svg"]), "svg"),
        ipc::TextExtension::Csv => (("CSV", &["csv"]), "csv"),
    }
}

fn path_string(p: &Path) -> Result<String, String> {
    p.to_str().map(str::to_owned).ok_or_else(|| {
        format!(
            "Separatrix can't use the path {} because it isn't valid Unicode. Choose another name.",
            p.display()
        )
    })
}

/// Writes `bytes` atomically, with a message that names the file.
fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    separatrix_project::write_atomic(path, bytes).map_err(|e| {
        let name = export_service::display_name(path);
        if separatrix_project::is_read_only_file(&e) {
            format!(
                "Couldn't save \u{201c}{name}\u{201d}: the file is read-only. Make it writable, or choose another name."
            )
        } else {
            format!("Couldn't save \u{201c}{name}\u{201d}: {e}. Check the folder, then save again.")
        }
    })
}

// ---------------------------------------------------------------------
// Project files
// ---------------------------------------------------------------------

/// The Open dialog (`*.sepx`). The chosen path is granted.
#[tauri::command]
async fn project_choose_open_path<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    let Some(path) = file_dialog(&app, DialogKind::Open, PROJECT_FILTER, None, None).await? else {
        return Ok(None);
    };
    let s = path_string(&path)?;
    app.state::<GrantedPaths>().grant(&path);
    Ok(Some(s))
}

/// Reads a granted project file.
#[tauri::command]
async fn project_read(
    req: ipc::ProjectReadRequest,
    granted: State<'_, GrantedPaths>,
) -> Result<ipc::OpenedProject, String> {
    let path = PathBuf::from(&req.path);
    if !granted.contains(&path) {
        return Err(not_granted(&path));
    }
    let opened =
        blocking(move || separatrix_project::read_file(&path).map_err(|e| e.to_string())).await?;
    Ok(ipc::OpenedProject {
        name: separatrix_project::file_name(Path::new(&req.path)),
        path: req.path,
        project: opened.project,
        warnings: opened.warnings,
        migrated_from: opened.migrated_from,
        newer_format: opened.newer_format,
    })
}

fn not_granted(path: &Path) -> String {
    format!(
        "Separatrix only opens and saves files you chose. Use Open\u{2026} or Save as\u{2026} to pick \u{201c}{}\u{201d}.",
        export_service::display_name(path)
    )
}

/// The Save dialog for a project. `.sepx` is added when missing; when that
/// file already exists the native dialog never asked, so the UI does.
#[tauri::command]
async fn project_choose_save_path<R: Runtime>(
    app: AppHandle<R>,
    req: ipc::ProjectChooseSaveRequest,
) -> Result<Option<ipc::ChosenSavePath>, String> {
    let dir = req
        .current_path
        .as_deref()
        .map(Path::new)
        .and_then(Path::parent)
        .filter(|d| d.is_dir())
        .map(Path::to_path_buf);
    let name = export_service::suggested_file_name(&req.suggested_name, "sepx");
    let Some(chosen) = file_dialog(&app, DialogKind::Save, PROJECT_FILTER, Some(name), dir).await?
    else {
        return Ok(None);
    };
    let (path, appended) = export_service::with_extension(&chosen, separatrix_project::EXTENSION);
    let s = path_string(&path)?;
    app.state::<GrantedPaths>().grant(&path);
    Ok(Some(ipc::ChosenSavePath {
        appended_exists: appended && path.exists(),
        path: s,
    }))
}

/// Checks and writes a project to a granted `.sepx` path.
#[tauri::command]
async fn project_write(
    req: ipc::ProjectWriteRequest,
    granted: State<'_, GrantedPaths>,
) -> Result<ipc::SavedProject, String> {
    let path = PathBuf::from(&req.path);
    if !files::is_project_path(&path) {
        return Err(format!(
            "Projects are saved as .sepx files, not \u{201c}{}\u{201d}. Choose a name that ends in .sepx.",
            export_service::display_name(&path)
        ));
    }
    if !granted.contains(&path) {
        return Err(not_granted(&path));
    }
    let name = separatrix_project::file_name(&path);
    blocking(move || {
        let project =
            separatrix_project::from_value_checked(req.project).map_err(|e| e.to_string())?;
        separatrix_project::write_file(&path, &project, export_service::APP_VERSION)
            .map_err(|e| e.with_file(&name).to_string())?;
        Ok(ipc::SavedProject {
            path: req.path,
            name,
        })
    })
    .await
}

/// The UI listens for `project-open-requested`: returns the files that
/// arrived before (granted), and emits from now on.
#[tauri::command]
fn launch_ready(launch: State<'_, LaunchFiles>) -> Vec<String> {
    launch
        .take_ready()
        .into_iter()
        .filter_map(|p| p.to_str().map(str::to_owned))
        .collect()
}

/// The window title and the dirty flag that guards closing.
#[tauri::command]
fn set_document_state<R: Runtime>(
    app: AppHandle<R>,
    req: ipc::DocumentStateRequest,
    doc: State<'_, DocState>,
) -> Result<(), String> {
    doc.set_dirty(req.dirty);
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        w.set_title(&req.title).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Closes the window for good (after the UI's confirm).
#[tauri::command]
fn close_window<R: Runtime>(app: AppHandle<R>, doc: State<'_, DocState>) -> Result<(), String> {
    doc.set_dirty(false);
    match app.get_webview_window(MAIN_WINDOW) {
        Some(w) => w.destroy().map_err(|e| e.to_string()),
        None => {
            app.exit(0);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------

/// The portrait at the figure's size, cut and simplified for export.
#[tauri::command]
async fn compute_export_portrait(
    req: ipc::ExportPortraitRequest,
    store: State<'_, SystemStore>,
    cache: State<'_, Arc<ExportCache>>,
) -> Result<ipc::PortraitResponse, String> {
    let system = system_or_err(&store, req.portrait.system_id)?;
    let cache = Arc::clone(&cache);
    blocking(move || export_service::export_portrait(&cache, &system, &req).map(|r| (*r).clone()))
        .await
}

/// The pgfplots figure with inline data (Copy).
#[tauri::command]
async fn export_figure_latex(
    req: ipc::FigureLatexRequest,
    store: State<'_, SystemStore>,
    cache: State<'_, Arc<ExportCache>>,
) -> Result<ipc::FigureLatexResponse, String> {
    let system = system_or_err(&store, req.portrait.system_id)?;
    let cache = Arc::clone(&cache);
    blocking(move || {
        let out = export_service::figure_latex(&cache, &system, &req, Placement::Inline)?;
        Ok(ipc::FigureLatexResponse {
            tex: out.tex,
            points: out.points,
            thinned: out.thinned,
            notes: out.notes,
        })
    })
    .await
}

/// Saves the figure as a `.tex` with its data files next to it.
#[tauri::command]
async fn save_figure_latex<R: Runtime>(
    app: AppHandle<R>,
    req: ipc::SaveFigureRequest,
    store: State<'_, SystemStore>,
    cache: State<'_, Arc<ExportCache>>,
) -> Result<Option<ipc::SavedFiles>, String> {
    let system = system_or_err(&store, req.figure.portrait.system_id)?;
    let name = export_service::suggested_file_name(&req.suggested_name, "tex");
    let Some(chosen) = file_dialog(
        &app,
        DialogKind::Save,
        ("LaTeX", &["tex"]),
        Some(name),
        None,
    )
    .await?
    else {
        return Ok(None);
    };
    let (path, _) = export_service::with_extension(&chosen, "tex");
    let cache = Arc::clone(&cache);
    blocking(move || {
        let base = separatrix_export::latex_figure::file_base(&export_service::display_name(&path));
        let out = export_service::figure_latex(
            &cache,
            &system,
            &req.figure,
            Placement::External { base },
        )?;
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut names = vec![export_service::display_name(&path)];
        for f in &out.files {
            write_file(&dir.join(&f.name), &f.bytes)?;
            names.push(f.name.clone());
        }
        write_file(&path, out.tex.as_bytes())?;
        Ok(Some(ipc::SavedFiles { names }))
    })
    .await
}

/// The system as an `align` environment.
#[tauri::command(async)]
fn export_system_latex(
    req: ipc::SystemLatexRequest,
    store: State<'_, SystemStore>,
) -> Result<ipc::SystemLatexResponse, String> {
    let system = system_or_err(&store, req.system_id)?;
    Ok(export_service::system_latex(&system, &req))
}

/// The fixed points as a booktabs table.
#[tauri::command]
async fn export_fixed_points_latex(
    req: ipc::FixedPointsLatexRequest,
    store: State<'_, SystemStore>,
) -> Result<String, String> {
    let system = system_or_err(&store, req.portrait.system_id)?;
    blocking(move || export_service::fixed_points_latex(&system, &req)).await
}

/// One CSV and `.meta.json` per initial condition.
#[tauri::command]
async fn export_csv(
    req: ipc::CsvRequest,
    store: State<'_, SystemStore>,
) -> Result<ipc::CsvResponse, String> {
    let system = system_or_err(&store, req.portrait.system_id)?;
    blocking(move || export_service::csv(&system, &req)).await
}

/// Saves the CSV files (and their metadata) where the user chooses.
#[tauri::command]
async fn save_csv<R: Runtime>(
    app: AppHandle<R>,
    req: ipc::SaveCsvRequest,
    store: State<'_, SystemStore>,
) -> Result<Option<ipc::SavedFiles>, String> {
    let system = system_or_err(&store, req.csv.portrait.system_id)?;
    let name = export_service::suggested_file_name(&req.suggested_name, "csv");
    let Some(chosen) =
        file_dialog(&app, DialogKind::Save, ("CSV", &["csv"]), Some(name), None).await?
    else {
        return Ok(None);
    };
    let (path, _) = export_service::with_extension(&chosen, "csv");
    blocking(move || {
        let res = export_service::csv(&system, &req.csv)?;
        let mut names = Vec::new();
        for (p, contents) in export_service::csv_save_plan(&path, &req.csv.title, &res.files) {
            write_file(&p, contents.as_bytes())?;
            names.push(export_service::display_name(&p));
        }
        Ok(Some(ipc::SavedFiles { names }))
    })
    .await
}

/// Saves a text file built by the UI (`.tex`, `.svg`, `.csv`).
#[tauri::command]
async fn save_text_file<R: Runtime>(
    app: AppHandle<R>,
    req: ipc::SaveTextRequest,
) -> Result<Option<ipc::SavedFiles>, String> {
    let (filter, ext) = text_filter(req.extension);
    let name = export_service::suggested_file_name(&req.suggested_name, ext);
    let Some(chosen) = file_dialog(&app, DialogKind::Save, filter, Some(name), None).await? else {
        return Ok(None);
    };
    let (path, _) = export_service::with_extension(&chosen, ext);
    blocking(move || {
        write_file(&path, req.contents.as_bytes())?;
        Ok(Some(ipc::SavedFiles {
            names: vec![export_service::display_name(&path)],
        }))
    })
    .await
}

/// The bytes of a raw-body command. Over the custom protocol they come raw;
/// if the webview falls back to `postMessage` (its CSP blocks the `ipc:`
/// protocol, say), Tauri sends a `Uint8Array` as a JSON array of numbers.
fn raw_body(request: &tauri::ipc::Request<'_>) -> Result<Vec<u8>, String> {
    body_bytes(request.body())
}

fn body_bytes(body: &tauri::ipc::InvokeBody) -> Result<Vec<u8>, String> {
    match body {
        tauri::ipc::InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        tauri::ipc::InvokeBody::Json(v @ serde_json::Value::Array(_)) => {
            serde_json::from_value::<Vec<u8>>(v.clone()).map_err(|_| bad_image_body())
        }
        tauri::ipc::InvokeBody::Json(_) => Err(bad_image_body()),
    }
}

fn bad_image_body() -> String {
    "The image didn't reach Separatrix intact. Try again; if it keeps happening, restart Separatrix.".to_string()
}

/// Saves a PNG built by the UI (raw body; the suggested name in the
/// URI-encoded `x-sepx-name` header).
#[tauri::command]
async fn save_png_file<R: Runtime>(
    app: AppHandle<R>,
    request: tauri::ipc::Request<'_>,
) -> Result<Option<ipc::SavedFiles>, String> {
    let bytes = raw_body(&request)?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(bad_image_body());
    }
    let suggested = request
        .headers()
        .get("x-sepx-name")
        .and_then(|v| v.to_str().ok())
        .and_then(export_service::percent_decode)
        .unwrap_or_else(|| "figure".to_string());
    let name = export_service::suggested_file_name(&suggested, "png");
    let Some(chosen) = file_dialog(
        &app,
        DialogKind::Save,
        ("PNG image", &["png"]),
        Some(name),
        None,
    )
    .await?
    else {
        return Ok(None);
    };
    let (path, _) = export_service::with_extension(&chosen, "png");
    blocking(move || {
        write_file(&path, &bytes)?;
        Ok(Some(ipc::SavedFiles {
            names: vec![export_service::display_name(&path)],
        }))
    })
    .await
}

/// Copies text.
#[tauri::command]
fn clipboard_write_text<R: Runtime>(
    app: AppHandle<R>,
    req: ipc::ClipboardTextRequest,
) -> Result<(), String> {
    app.clipboard().write_text(req.text).map_err(|e| {
        format!("Couldn't copy to the clipboard: {e}. Try again, or use Save\u{2026} instead.")
    })
}

/// Copies a PNG (raw body) as an image. The clipboard takes raw RGBA, so
/// the PNG's physical size (pHYs) is not kept.
#[tauri::command]
async fn clipboard_write_png<R: Runtime>(
    app: AppHandle<R>,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let bytes = raw_body(&request)?;
    let (rgba, w, h) = blocking(move || separatrix_export::raster::decode_rgba(&bytes)).await?;
    let image = tauri::image::Image::new_owned(rgba, w, h);
    app.clipboard()
        .write_image(&image)
        .map_err(|e| {
            format!("Couldn't copy the image to the clipboard: {e}. Try again, or use Save\u{2026} instead.")
        })
}

// ---------------------------------------------------------------------
// The app
// ---------------------------------------------------------------------

#[cfg(target_os = "macos")]
const QUIT_MENU_ID: &str = "quit";

/// The default macOS menu with Quit replaced by an item that closes the
/// window, so Cmd+Q and the app menu's Quit go through the unsaved-changes
/// guard (tao has no `applicationShouldTerminate`: the predefined Quit skips
/// it). Known gap: Quit from the Dock, AppleScript and logout/shutdown send
/// `terminate:` straight to the app delegate, which tao doesn't let us
/// answer, so they still quit without asking (docs/project-format.md,
/// "Known limitations").
#[cfg(target_os = "macos")]
fn macos_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{
        AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
        WINDOW_SUBMENU_ID,
    };
    let name = app.package_info().name.clone();
    let about = AboutMetadata {
        name: Some(name.clone()),
        version: Some(app.package_info().version.to_string()),
        ..Default::default()
    };
    let quit = MenuItem::with_id(
        app,
        QUIT_MENU_ID,
        format!("Quit {name}"),
        true,
        Some("CmdOrCtrl+Q"),
    )?;
    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                &name,
                true,
                &[
                    &PredefinedMenuItem::about(app, None, Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?,
            &Submenu::with_items(
                app,
                "File",
                true,
                &[&PredefinedMenuItem::close_window(app, None)?],
            )?,
            &Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(app, None)?],
            )?,
            &Submenu::with_id_and_items(
                app,
                WINDOW_SUBMENU_ID,
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(app, None)?,
                    &PredefinedMenuItem::maximize(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::close_window(app, None)?,
                ],
            )?,
            &Submenu::with_id_and_items(app, HELP_SUBMENU_ID, "Help", true, &[])?,
        ],
    )
}

/// Starts the desktop app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // Managed on the builder, not in `setup`: macOS `Opened` can arrive
        // before `setup` has finished.
        .manage(SystemStore::new())
        .manage(Arc::new(ExportCache::new()))
        .manage(LaunchFiles::default())
        .manage(DocState::default())
        .manage(GrantedPaths::default())
        .manage(LastDir::default())
        // First, so a second instance exits before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let paths = files::project_paths_from_args(
                argv.into_iter().map(OsString::from),
                Path::new(&cwd),
            );
            files::deliver(app, paths);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let cwd = std::env::current_dir().unwrap_or_default();
            // `args_os`: `args` panics on an argument that isn't UTF-8.
            let paths = files::project_paths_from_args(std::env::args_os(), &cwd);
            files::deliver(app.handle(), paths);
            #[cfg(target_os = "macos")]
            {
                let menu = macos_menu(app.handle())?;
                app.set_menu(menu)?;
                app.on_menu_event(|app, event| {
                    if event.id() == QUIT_MENU_ID {
                        match app.get_webview_window(MAIN_WINDOW) {
                            // Raises CloseRequested: the guard below runs.
                            Some(w) => {
                                let _ = w.close();
                            }
                            None => app.exit(0),
                        }
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                if files::should_prevent_close(&window.state::<DocState>()) {
                    api.prevent_close();
                    let _ = window.emit_to(
                        MAIN_WINDOW,
                        files::EVENT_CLOSE_REQUESTED,
                        ipc::CloseRequested {},
                    );
                }
            }
            WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                let projects: Vec<PathBuf> = paths
                    .iter()
                    .filter(|p| files::is_project_path(p))
                    .cloned()
                    .collect();
                if !projects.is_empty() {
                    files::deliver(window.app_handle(), projects);
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            core_version,
            compile_system,
            convert_source,
            compute_portrait,
            compute_lyapunov,
            compute_orbit_diagram,
            project_choose_open_path,
            project_read,
            project_choose_save_path,
            project_write,
            launch_ready,
            set_document_state,
            close_window,
            compute_export_portrait,
            export_figure_latex,
            save_figure_latex,
            export_system_latex,
            export_fixed_points_latex,
            export_csv,
            save_csv,
            save_text_file,
            save_png_file,
            clipboard_write_text,
            clipboard_write_png,
        ]);
    builder
        .build(tauri::generate_context!())
        .expect("error while building the app")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                let paths = urls.iter().filter_map(files::file_url_to_path).collect();
                files::deliver(_app, paths);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::body_bytes;
    use tauri::ipc::InvokeBody;

    #[test]
    fn image_bytes_arrive_raw_or_as_a_json_array() {
        let png = b"\x89PNG\r\n\x1a\n".to_vec();
        assert_eq!(body_bytes(&InvokeBody::Raw(png.clone())), Ok(png.clone()));
        // The postMessage fallback: `Array.from(uint8Array)`.
        let json = serde_json::json!(png);
        assert_eq!(body_bytes(&InvokeBody::Json(json)), Ok(png));
        for bad in [
            serde_json::json!({ "0": 137 }),
            serde_json::json!([256]),
            serde_json::json!([-1]),
            serde_json::json!("x"),
        ] {
            assert!(body_bytes(&InvokeBody::Json(bad)).is_err());
        }
    }

    /// Without `connect-src ipc:`, the webview can't reach the IPC custom
    /// protocol and every command falls back to `postMessage`.
    #[test]
    fn the_csp_lets_the_webview_reach_the_ipc_protocol() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        for key in ["csp", "devCsp"] {
            let csp = conf["app"]["security"][key].as_str().unwrap();
            let connect = csp
                .split(';')
                .map(str::trim)
                .find(|d| d.starts_with("connect-src"))
                .unwrap_or_else(|| panic!("{key} has no connect-src"));
            assert!(connect.contains(" ipc:"), "{key}: {connect}");
            assert!(connect.contains("http://ipc.localhost"), "{key}: {connect}");
        }
        // macOS binds the document type to the exported UTI.
        let assoc = &conf["bundle"]["fileAssociations"][0];
        assert_eq!(
            assoc["contentTypes"][0],
            assoc["exportedType"]["identifier"]
        );
    }
}
