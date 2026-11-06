//! Project files reaching the app from outside the webview: launch
//! arguments, a second instance (single-instance forwarding), macOS
//! `Opened` events and drag-and-drop; plus the document state that guards
//! closing the window, and the set of paths the webview may read or write.
//!
//! The logic is in plain types ([`LaunchFiles`], [`GrantedPaths`],
//! [`DocState`]) and pure functions so it is unit-tested without a window;
//! [`deliver`] only adds the Tauri calls.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, Url};

use crate::ipc::ProjectOpenRequested;

/// The main window's label (tauri.conf.json).
pub const MAIN_WINDOW: &str = "main";
/// Event: open this project (after the UI's unsaved-changes confirm).
pub const EVENT_OPEN_REQUESTED: &str = "project-open-requested";
/// Event: the user asked to close the window while the document is dirty.
pub const EVENT_CLOSE_REQUESTED: &str = "close-requested";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// True for a `.sepx` path (any case).
pub fn is_project_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(separatrix_project::EXTENSION))
}

/// The project files among a command line: skips `argv[0]` and flags
/// (`-psn_…` on macOS, `--foo`), keeps `*.sepx` in any case, and resolves
/// relative paths against `cwd` (the second instance's working directory,
/// which is not ours).
pub fn project_paths_from_args<I: IntoIterator<Item = OsString>>(
    args: I,
    cwd: &Path,
) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
        .filter(|p| is_project_path(p))
        .map(|p| if p.is_absolute() { p } else { cwd.join(p) })
        .collect()
}

/// The local path of a `file://` URL (macOS `Opened`); `None` for any other
/// URL.
pub fn file_url_to_path(url: &Url) -> Option<PathBuf> {
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

/// Paths the webview may pass to `project_read` and `project_write`: the
/// ones the user chose in a dialog, named on the command line, sent by a
/// second instance or by the OS, or dropped on the window. Any other path
/// is refused, so a page (or a project file) can't make the app read or
/// overwrite arbitrary files.
#[derive(Default)]
pub struct GrantedPaths(Mutex<HashSet<PathBuf>>);

impl GrantedPaths {
    /// Allows `path` (as given, and its canonical form when it exists).
    pub fn grant(&self, path: &Path) {
        let mut set = lock(&self.0);
        set.insert(path.to_path_buf());
        if let Ok(c) = path.canonicalize() {
            set.insert(c);
        }
    }

    /// Whether `path` was granted (as given, or by its canonical form).
    pub fn contains(&self, path: &Path) -> bool {
        let set = lock(&self.0);
        set.contains(path) || path.canonicalize().is_ok_and(|c| set.contains(&c))
    }
}

/// The recent projects file, in the app's config folder.
pub const RECENT_FILE: &str = "recent-projects.json";
/// How many recent projects are kept.
pub const RECENT_MAX: usize = 8;

/// The projects opened or saved last, most recent first, each once, as
/// `recent-projects.json` stores them: `{ "paths": [...] }`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentList {
    pub paths: Vec<PathBuf>,
}

impl RecentList {
    /// The stored list; a missing, unreadable or malformed file is an empty
    /// list, never an error.
    pub fn parse(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap_or_default()
    }

    pub fn to_json(&self) -> Vec<u8> {
        let mut out = serde_json::to_vec_pretty(self).unwrap_or_default();
        out.push(b'\n');
        out
    }

    /// `path` first, any earlier entry for it dropped, at most
    /// [`RECENT_MAX`] kept. Callers pass canonical paths (see
    /// [`canonical`]), so two spellings of one file are one entry.
    pub fn push(mut self, path: PathBuf) -> Self {
        self.paths.retain(|p| *p != path);
        self.paths.insert(0, path);
        self.paths.truncate(RECENT_MAX);
        self
    }
}

/// `path` with links and `..` resolved when it exists, else as given.
pub fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The recent projects, kept in memory and written to `file` after every
/// change. With no config folder (`file` is `None`) the list lasts for the
/// session only.
pub struct RecentProjects {
    file: Option<PathBuf>,
    list: Mutex<RecentList>,
}

impl RecentProjects {
    pub fn load(file: Option<PathBuf>) -> Self {
        let list = file
            .as_deref()
            .and_then(|f| std::fs::read(f).ok())
            .map(|b| RecentList::parse(&b))
            .unwrap_or_default();
        Self {
            file,
            list: Mutex::new(list),
        }
    }

    /// Puts `path` first and saves the list. The lock is held through the
    /// write, so two saves can't store an older list over a newer one.
    pub fn remember(&self, path: &Path) -> std::io::Result<()> {
        let mut list = lock(&self.list);
        *list = std::mem::take(&mut *list).push(canonical(path));
        let Some(file) = &self.file else {
            return Ok(());
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        separatrix_project::write_atomic(file, &list.to_json())
    }

    /// The listed files that still exist and have UTF-8 paths, most recent
    /// first. Missing ones stay stored: a file on an unplugged drive comes
    /// back when the drive does.
    pub fn existing(&self) -> Vec<PathBuf> {
        lock(&self.list)
            .paths
            .iter()
            .filter(|p| p.is_file() && p.to_str().is_some())
            .cloned()
            .collect()
    }
}

/// Files that arrive before the UI listens for them.
#[derive(Default)]
struct LaunchState {
    ready: bool,
    pending: Vec<PathBuf>,
}

/// The queue between the OS (launch arguments, second instance, `Opened`)
/// and the UI: paths are queued until the UI calls `launch_ready`, then
/// emitted as `project-open-requested`.
#[derive(Default)]
pub struct LaunchFiles(Mutex<LaunchState>);

impl LaunchFiles {
    /// Queues `path`, or returns it to be emitted now when the UI is ready
    /// and the main window exists. The lock makes "queue or emit" atomic
    /// against [`LaunchFiles::take_ready`].
    pub fn offer(&self, path: PathBuf, window_exists: bool) -> Option<PathBuf> {
        let mut s = lock(&self.0);
        if s.ready && window_exists {
            Some(path)
        } else {
            s.pending.push(path);
            None
        }
    }

    /// Marks the UI ready and returns what was queued, oldest first.
    pub fn take_ready(&self) -> Vec<PathBuf> {
        let mut s = lock(&self.0);
        s.ready = true;
        std::mem::take(&mut s.pending)
    }
}

/// What Rust knows about the open document.
#[derive(Default)]
struct DocInner {
    dirty: bool,
}

/// The document's dirty flag, sent by the UI with `set_document_state`.
#[derive(Default)]
pub struct DocState(Mutex<DocInner>);

impl DocState {
    /// Records whether the document has unsaved changes.
    pub fn set_dirty(&self, dirty: bool) {
        lock(&self.0).dirty = dirty;
    }
}

/// Whether a close request must be stopped (and the UI asked to confirm).
pub fn should_prevent_close(state: &DocState) -> bool {
    lock(&state.0).dirty
}

/// Hands project files from outside to the UI: the first one only (the app
/// shows one document), granted, then queued or emitted; the window is
/// brought to the front. Called from the single-instance D-Bus thread on
/// Linux, so it uses only the mutexes, `emit` and window calls (which Tauri
/// sends to the main thread).
pub fn deliver<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>) {
    // The webview gets the path as a string; a path that isn't UTF-8 can't
    // make the round trip.
    let Some(path) = paths
        .into_iter()
        .find(|p| is_project_path(p) && p.to_str().is_some())
    else {
        focus_main(app);
        return;
    };
    app.state::<GrantedPaths>().grant(&path);
    let window = app.get_webview_window(MAIN_WINDOW);
    if let Some(path) = app.state::<LaunchFiles>().offer(path, window.is_some()) {
        let payload = ProjectOpenRequested {
            path: path.to_string_lossy().into_owned(),
        };
        if let Err(e) = app.emit_to(MAIN_WINDOW, EVENT_OPEN_REQUESTED, payload) {
            eprintln!("separatrix: couldn't send {EVENT_OPEN_REQUESTED}: {e}");
        }
    }
    focus_main(app);
}

fn focus_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[cfg(unix)]
    fn cwd() -> PathBuf {
        PathBuf::from("/home/ada/work")
    }
    #[cfg(windows)]
    fn cwd() -> PathBuf {
        PathBuf::from(r"C:\Users\ada\work")
    }

    #[test]
    fn args_skip_the_program_and_flags_and_keep_projects() {
        let got = project_paths_from_args(
            os(&[
                "separatrix",
                "--verbose",
                "-psn_0_12345",
                "notes.txt",
                "a.sepx",
                "B.SEPX",
                "c.Sepx",
                "sepx",
                ".sepx.bak",
            ]),
            &cwd(),
        );
        assert_eq!(
            got,
            vec![
                cwd().join("a.sepx"),
                cwd().join("B.SEPX"),
                cwd().join("c.Sepx")
            ]
        );
    }

    #[test]
    fn the_program_name_is_never_a_project() {
        assert!(project_paths_from_args(os(&["evil.sepx"]), &cwd()).is_empty());
        assert!(project_paths_from_args(Vec::<OsString>::new(), &cwd()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn absolute_and_relative_paths_on_unix() {
        let got = project_paths_from_args(
            os(&["sx", "/tmp/x.sepx", "sub/y.sepx", "../z.sepx"]),
            Path::new("/home/ada"),
        );
        assert_eq!(
            got,
            vec![
                PathBuf::from("/tmp/x.sepx"),
                PathBuf::from("/home/ada/sub/y.sepx"),
                PathBuf::from("/home/ada/../z.sepx"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_arguments_do_not_panic() {
        use std::os::unix::ffi::OsStringExt;
        let bad = OsString::from_vec(b"caf\xe9.sepx".to_vec());
        let got = project_paths_from_args(vec![OsString::from("sx"), bad.clone()], Path::new("/w"));
        assert_eq!(got, vec![Path::new("/w").join(bad)]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths() {
        let got = project_paths_from_args(
            os(&[
                r"C:\Program Files\Separatrix\separatrix.exe",
                r"D:\course\week 3.sepx",
                r"sub\y.SEPX",
                r"\\server\share\z.sepx",
            ]),
            &cwd(),
        );
        assert_eq!(
            got,
            vec![
                PathBuf::from(r"D:\course\week 3.sepx"),
                PathBuf::from(r"C:\Users\ada\work\sub\y.SEPX"),
                PathBuf::from(r"\\server\share\z.sepx"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_urls() {
        let u = Url::parse("file:///Users/ada/My%20Project.sepx").unwrap();
        assert_eq!(
            file_url_to_path(&u),
            Some(PathBuf::from("/Users/ada/My Project.sepx"))
        );
        let u = Url::parse("https://example.com/a.sepx").unwrap();
        assert_eq!(file_url_to_path(&u), None);
        let u = Url::parse("file://server/share/a.sepx").unwrap();
        assert_eq!(
            file_url_to_path(&u),
            None,
            "a remote host is not a local path"
        );
    }

    #[test]
    fn deliveries_queue_until_the_ui_is_ready() {
        let q = LaunchFiles::default();
        assert_eq!(q.offer("a.sepx".into(), true), None, "not ready yet");
        assert_eq!(q.offer("b.sepx".into(), false), None);
        assert_eq!(
            q.take_ready(),
            vec![PathBuf::from("a.sepx"), PathBuf::from("b.sepx")]
        );
        assert!(q.take_ready().is_empty(), "taken once");
        assert_eq!(
            q.offer("c.sepx".into(), true),
            Some(PathBuf::from("c.sepx")),
            "ready with a window: emit"
        );
        assert_eq!(q.offer("d.sepx".into(), false), None, "no window: queue");
        assert_eq!(q.take_ready(), vec![PathBuf::from("d.sepx")]);
    }

    #[test]
    fn close_is_prevented_only_while_dirty() {
        let d = DocState::default();
        assert!(!should_prevent_close(&d));
        d.set_dirty(true);
        assert!(should_prevent_close(&d));
        d.set_dirty(false);
        assert!(!should_prevent_close(&d));
    }

    #[test]
    fn only_granted_paths_are_allowed() {
        let dir = std::env::temp_dir().join(format!("sepx-granted-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.sepx");
        std::fs::write(&file, "{}").unwrap();
        let g = GrantedPaths::default();
        assert!(!g.contains(&file));
        g.grant(&file);
        assert!(g.contains(&file));
        // The same file by another spelling (through `..`) is the same grant.
        let other = dir.join("..").join(dir.file_name().unwrap()).join("a.sepx");
        assert!(g.contains(&other));
        assert!(!g.contains(&dir.join("b.sepx")));
        assert!(!g.contains(Path::new("/etc/passwd")));
        // A path that doesn't exist yet (Save as…) is granted as given.
        let new = dir.join("new.sepx");
        g.grant(&new);
        assert!(g.contains(&new));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn recent(paths: &[&str]) -> RecentList {
        RecentList {
            paths: paths.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn recent_puts_the_newest_first_and_keeps_each_file_once() {
        let list = RecentList::default()
            .push("a.sepx".into())
            .push("b.sepx".into())
            .push("c.sepx".into());
        assert_eq!(list, recent(&["c.sepx", "b.sepx", "a.sepx"]));
        let list = list.push("a.sepx".into());
        assert_eq!(
            list,
            recent(&["a.sepx", "c.sepx", "b.sepx"]),
            "moved, not added"
        );
        assert_eq!(list.clone().push("a.sepx".into()), list, "already first");
    }

    #[test]
    fn recent_keeps_at_most_eight() {
        let list = (0..12).fold(RecentList::default(), |l, k| {
            l.push(format!("{k}.sepx").into())
        });
        assert_eq!(list.paths.len(), RECENT_MAX);
        assert_eq!(list.paths[0], PathBuf::from("11.sepx"));
        assert_eq!(list.paths[RECENT_MAX - 1], PathBuf::from("4.sepx"));
    }

    #[test]
    fn recent_garbage_parses_to_an_empty_list() {
        for bytes in [
            &b""[..],
            b"not json",
            b"null",
            b"[]",
            b"{}",
            b"{\"paths\": 3}",
            b"{\"paths\": [1, 2]}",
            b"\xff\xfe",
        ] {
            assert_eq!(
                RecentList::parse(bytes),
                RecentList::default(),
                "{:?}",
                String::from_utf8_lossy(bytes)
            );
        }
        let list = recent(&["/w/a.sepx", "/w/b c.sepx"]);
        assert_eq!(RecentList::parse(&list.to_json()), list, "round trip");
        assert_eq!(
            RecentList::parse(br#"{"paths":["/w/a.sepx"],"later":true}"#),
            recent(&["/w/a.sepx"]),
            "unknown keys are ignored"
        );
    }

    #[test]
    fn recent_is_stored_by_canonical_path_and_lists_only_files_that_exist() {
        let dir = std::env::temp_dir().join(format!("sepx-recent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let a = dir.join("a.sepx");
        let b = dir.join("b.sepx");
        std::fs::write(&a, "{}").unwrap();
        std::fs::write(&b, "{}").unwrap();
        let file = dir.join("config").join(RECENT_FILE);

        let store = RecentProjects::load(Some(file.clone()));
        assert!(store.existing().is_empty(), "no file yet: empty");
        store.remember(&a).unwrap();
        store.remember(&b).unwrap();
        store
            .remember(&dir.join("sub").join("..").join("a.sepx"))
            .unwrap();
        let (a, b) = (canonical(&a), canonical(&b));
        assert_eq!(
            store.existing(),
            vec![a.clone(), b.clone()],
            "a once, first"
        );

        let reloaded = RecentProjects::load(Some(file.clone()));
        assert_eq!(reloaded.existing(), vec![a.clone(), b.clone()], "persisted");
        std::fs::remove_file(&b).unwrap();
        assert_eq!(
            reloaded.existing(),
            vec![a.clone()],
            "a deleted file is not listed"
        );
        assert_eq!(
            RecentList::parse(&std::fs::read(&file).unwrap())
                .paths
                .len(),
            2,
            "but stays stored"
        );

        std::fs::write(&file, "garbage").unwrap();
        assert!(RecentProjects::load(Some(file)).existing().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn project_extension() {
        assert!(is_project_path(Path::new("x.sepx")));
        assert!(is_project_path(Path::new("X.SePx")));
        assert!(!is_project_path(Path::new("x.sepx.json")));
        assert!(!is_project_path(Path::new("sepx")));
        assert!(!is_project_path(Path::new("x.json")));
    }
}
