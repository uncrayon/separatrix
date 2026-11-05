# The Separatrix project file (`.sepx`)

A project is one pretty-printed JSON file with the extension `.sepx`. It stores what the user
typed and chose — the system, its parameters, the initial conditions, the views, the analyses
and the solver settings — and never computed data: trajectories, nullclines and fixed points
are recomputed on open, so a file is a few kilobytes and travels by email or a course platform.

The format is implemented by the `separatrix-project` crate (`crates/project`, library
`separatrix_project`), which depends only on serde and serde_json. The Tauri shell calls it
from the `project_read` and `project_write` commands.

## Encoding

- UTF-8 JSON, no BOM (a BOM is ignored on read), LF line endings, 2-space indent, one trailing
  newline.
- At most 4 MiB (`MAX_FILE_BYTES`); larger files are refused before parsing.
- Keys are **snake_case**. This is the one documented exception to the camelCase IPC: the file
  is meant to be read, diffed and edited by people.
- Known keys are written in a fixed order (the order below), then any unknown keys, sorted.
- Numbers are written in the shortest form that reads back to the same `f64`, and are parsed
  exactly (`float_roundtrip`), so saving an unchanged project twice gives identical bytes.
  JSON can't hold NaN or infinities: a project that contains one is refused on save with a
  message naming the field.

## Schema, version 1

```json
{
  "format": "separatrix",
  "format_version": 1,
  "min_reader_version": 1,
  "app_version": "0.2.0",
  "system": {
    "kind": "ode",
    "input_format": "latex",
    "lines": ["\\dot{x}=x\\left(3-x-2y\\right)", "\\dot{y}=y\\left(2-x-y\\right)"],
    "states": ["x", "y"],
    "params": [{ "id": "a", "value": 3.0, "min": 0.0, "max": 6.0 }],
    "template": "rabbits-vs-sheep"
  },
  "initial_conditions": [
    { "id": "ic-1", "x0": [0.3, 0.2], "t0": 0.0, "color": 0, "hidden": false }
  ],
  "views": [
    {
      "id": "main",
      "x_axis": { "kind": "state", "name": "x" },
      "y_axis": { "kind": "state", "name": "y" },
      "x_range": [-0.25, 3.5],
      "y_range": [-0.25, 2.75],
      "slice": { "states": { "z": 0.0 }, "t": 0.0 },
      "layers": { "field": true, "field_grid": 21, "nullclines": true, "fixed_points": true, "backward": true }
    }
  ],
  "analyses": [],
  "solver": { "method": "dopri5", "rtol": 1e-6, "atol": 1e-9, "h": 0.0, "max_steps": 200000, "t_end": 20.0, "samples": 0 },
  "export_settings": { "…": "owned by the export dialog" }
}
```

(The real file puts each array element on its own line.)

| Key | Type | Required | Meaning |
|---|---|---|---|
| `format` | string | yes | Always `"separatrix"`. Anything else is "not a Separatrix project". |
| `format_version` | integer ≥ 1 | yes | The schema version the file was written in. |
| `min_reader_version` | integer ≥ 1 | no | The oldest schema version that can read the file. Missing means "equal to `format_version`". |
| `app_version` | string | no | The Separatrix version that saved the file (used in messages). |
| `system.kind` | string | no | `"ode"`, `"map"`, … Informational; recomputed on open. |
| `system.input_format` | `"latex"` \| `"text"` | yes | The syntax of `lines`. |
| `system.lines` | string[] | yes | **The source of truth**: every equation line exactly as typed, blank lines included. |
| `system.states` | string[] | no | State names, in the order `x0` arrays and `slice.states` refer to them. On open the app remaps start points by name if the compiled system orders its states differently. |
| `system.params[]` | object | no | `id` (the parameter's name; required), `value` (required), `min`, `max`: the slider. |
| `system.template` | string \| null | no | The template the system came from; `null` for untitled work. |
| `initial_conditions[]` | object | no | `id`, `x0` (one number per state; required), `t0` (default 0), `color` (palette index 0–6, default 0), `hidden` (default false). |
| `views[]` | object | no | See below. |
| `analyses` | array | no | Reserved: v1 keeps the entries verbatim. |
| `solver` | object | no | `method` (`"euler"`, `"heun"`, `"rk4"`, `"dopri5"`), `rtol`, `atol`, `h` (fixed step, 0 = the method's default), `max_steps`, `t_end` (for a map, the number of iterates N, shared by its time series and cobweb), `transient` (maps: iterates computed but not drawn before each orbit; a flow's section view: crossings computed but not drawn; absent = 0, written only when above 0, after `t_end`), `samples` (0 = automatic). Every key has a default. |
| `export_settings` | any | no | The export dialog's options for this file; opaque to the format. |

### Views

| Key | Type | Meaning |
|---|---|---|
| `id` | string | Unique among the views. |
| `kind` | `"plane"` \| `"cobweb"` \| `"section"` | Absent = `"plane"`: the phase plane and the time series, which the axes tell apart. Written only for a cobweb or a section, after `id`. |
| `section` | object | Only in a `"section"` view, after `kind`: the Poincaré section (below). |
| `x_axis`, `y_axis` | `{ "kind": "state", "name": "x" }` \| `{ "kind": "time" }` \| `{ "kind": "next", "name": "x" }` | Axes are stored by state name, never by index. `next` is the state one iterate later: `x_{n+1}` as the `y_axis` of a cobweb view, or the next crossing `x_{k+1}` as the `y_axis` of a section's return map; either way the `x_axis` is `{ "kind": "state" }` of the same state. |
| `x_range`, `y_range` | `[lo, hi]` | The user's ranges (not the ones equalized for the plot's aspect). |
| `slice` | `{ "states": { name: value }, "t": number }` | The values of the states that aren't on an axis, and the time the field is drawn at. |
| `layers` | object | `field`, `field_grid` (arrows per side, 2–200), `nullclines`, `fixed_points`, `backward`. |

A cobweb view (Strogatz §10.1) of the logistic map:

```json
{
  "id": "main",
  "kind": "cobweb",
  "x_axis": { "kind": "state", "name": "x" },
  "y_axis": { "kind": "next", "name": "x" },
  "x_range": [-0.05, 1.05],
  "y_range": [-0.05, 1.05],
  "…": "slice and layers as in any view"
}
```

A section view (Strogatz §8.7) of the Lorenz equations: where orbits cross the plane `z = 27`
upward, plotted in `x` and `y`, after the first `solver.transient` crossings of each orbit
(`crates/project/tests/fixtures/section-view.sepx`):

```json
{
  "id": "main",
  "kind": "section",
  "section": { "kind": "plane", "state": "z", "value": 27.0, "direction": "rising" },
  "x_axis": { "kind": "state", "name": "x" },
  "y_axis": { "kind": "state", "name": "y" },
  "x_range": [-20.0, 20.0],
  "y_range": [-25.0, 25.0],
  "…": "slice and layers as in any view; a section draws neither"
}
```

| `section` key | Type | Meaning |
|---|---|---|
| `kind` | `"plane"` \| `"stroboscopic"` | A plane `x_i = c`, or the state once per forcing period. |
| `state` | string | Plane: the state held fixed, by name. Needs a system with at least 2 states. |
| `value` | number | Plane: where it cuts that state's axis. |
| `direction` | `"rising"` \| `"falling"` \| `"both"` | Plane: which crossings count (the studio's Increasing, Decreasing, Both). |
| `period` | number > 0 | Stroboscopic: the forcing period `T`. |
| `phase` | number | Stroboscopic: the samples are at `t = phase + k·T` (absolute time, not from each start's `t0`); absent = 0. |

The axes of a section view are two different states, or a state across and `next` of the same
state up: the return map `x_k → x_{k+1}`. Unknown keys inside `section` are kept.

`kind`, `section` and `transient` are additive: a file without them reads exactly as before, a file
saved from a plane view is byte for byte what this build wrote before cobwebs, and
`format_version` and `min_reader_version` stay 1 (no migration step). An older 0.2.0 build that
opens a cobweb file sees the axis kind `next` ("an axis kind this version doesn't know"),
shows its default view and keeps `kind` and `transient` as unknown keys; it misreads nothing,
so `min_reader_version` is not raised (the rule below). The 0.2.0 reader that knows cobwebs
(trunk before section views) opens a section file with its view-kind warning, "The saved cobweb
view doesn't fit this system, so the default view is shown.", shows its default view, and keeps
`kind` and `section` as unknown keys (written back after the known keys, so not byte for byte).
Its studio drops `kind` when it saves from its default view and keeps `section`; this build reads
such a view as the plane view its axes say, and drops the stale `section` on its next save.

The **primary view** is the one with id `main`, otherwise the first. Version 1 edits only the
primary view and writes it back under its own id; other views are kept verbatim.

### Ids

Every object has an id that is unique within its array and stable across saves, so later
versions can point at it: `params.<id>`, `initial_conditions.<id>`, `views.<id>`.

### Not saved

The compile result, the portrait (trajectories, nullclines, fixed points), the selection and
hover, the home view, the plot's aspect, conversion notes, errors, the theme, the eigenvector
toggle, panel folds and the template gallery.

## Unknown keys

Keys the reader doesn't know are kept on save, at every level (the file, `system`, each
parameter, initial condition, view, axis, slice, layer set and the solver), so a file edited by
a newer Separatrix or by hand loses nothing.

Preservation is **value-level**: an unknown key comes back with an equal value, but inside an
unknown object the keys are written in sorted order and the original whitespace is not kept.
Two limits: an unknown number keeps its value within f64 precision, or exactly when it is a
64-bit integer (`123456789012345678901234567890` comes back as `1.2345678901234568e+29`); and
`null` for a known optional key (`export_settings`, an axis's `name`) is treated as absent, so
the key is not written back.

`export_settings` is opaque to the reader: the export dialog merges its options onto the stored
object, so keys a newer Separatrix keeps there survive a save from this one.

`assignment` is reserved for version 2 (course assignments that point at `params.a` or a view).
Version 1 never reads or writes it; it rides along as an unknown key, `null` included.

## Versions and migration

Opening a file:

1. Check the size, strip a BOM and parse the JSON (with serde_json's nesting limit of 128).
2. `format` must be `"separatrix"`.
3. `format_version` and `min_reader_version` must be whole numbers from 1 to 4294967295
   (`1.0` counts as 1).
4. If `min_reader_version` is newer than this build's `CURRENT_VERSION`, the file is refused:
   "“x.sepx” was saved by Separatrix 0.5.0 and needs a newer version to open. Update
   Separatrix, then open it again." This applies whatever `format_version` says.
5. If `format_version` is newer but `min_reader_version` allows this build, the file opens as
   the current version with a warning: "“x.sepx” was saved in a newer format by Separatrix
   0.3.0. It opens, and saving keeps the parts this version doesn't use." Saving keeps its
   `format_version` and `min_reader_version`.
6. If `format_version` is older, migration steps run in order, one version at a time
   (1 → 2 → 3 …). Each step rewrites the raw JSON and bumps `format_version`. Saving then
   writes the current format.
7. The structure is checked, with the path of the first problem
   (`initial_conditions[2].x0[1] should be a number, not "0.5"`).
8. The typed conversion, then repairs (below).

Writing stamps `format = "separatrix"`, `format_version = max(file, current)`,
`min_reader_version` (the file's own for a newer format, otherwise this build's
`MIN_READER_VERSION`) and `app_version` (the running app).

**A rule for future versions.** Raise `min_reader_version` only when an older reader would
misread the file; adding keys never requires it, because older readers keep unknown keys.

## Hard errors and repairs

Only the structure is a hard error: invalid JSON, another format, a version that isn't a whole
number from 1 to 4294967295, a missing `system`, `system.lines` or `system.input_format`, an `input_format`
other than `"latex"` or `"text"`, and values of the wrong JSON type. Every message names the
file, says what is wrong and what to do. A whole number written as a float (`21.0`, `1e5`, as
Python's `json` or a spreadsheet exporter writes it) is accepted where an integer belongs
(`color`, `field_grid`, `max_steps`, `samples`, `transient`, the versions) and saved as an integer.

Everything else is repaired on open, with one warning per repair:

| Problem | Repair |
|---|---|
| Duplicate or empty id | A suffix (`ic-3` → `ic-3-2`); an empty id starts from its position (`ic-4`). |
| Slider `min ≥ max`, or a missing bound | The app's default range around `value`. |
| `color` outside 0–6 | Taken mod 7. |
| `x0` with the wrong number of values | Padded with 0 or truncated to `system.states` (only when `states` is present). |
| `field_grid` outside 2–200 | Clamped. |
| `transient` outside 0–1,000,000 | Clamped (a whole number written as a float, `200.0`, is accepted; `0` is dropped). |
| A primary view of `kind` `"cobweb"` whose axes are not state `s` across and `next` `s` up, a `next` axis outside a cobweb, a kind this version doesn't know, or a cobweb for a system that is not a one-dimensional map (`states`, `system.kind`) | "The saved cobweb view doesn't fit this system, so the default view is shown." The app gives the same warning when the compiled system turns out not to be a one-dimensional autonomous map. The view is kept as written. |
| A primary `"section"` view whose `section.kind` this version doesn't know | "The section of the view “main” is of a kind (“…”) this version doesn't know, so the default view is used." The view is kept as written. |
| A primary `"section"` plane through a state the system doesn't have | "The section of the view “main” holds the state w fixed, which the system doesn't have, so the default view is used." |
| Any other section that doesn't fit: no `section`, a map, a plane of a one-state system, no `value`, an unknown `direction`, a `period` that isn't above 0, or axes other than two states or a state and its `next` | "The saved Poincaré section doesn't fit this system, so the default view is shown." The app gives the same sentences when the compiled system can't take the section. The view is kept as written. |
| Unknown solver `method` | `dopri5` (Dormand–Prince 5(4)). |
| `rtol` or `atol` ≤ 0, `h` < 0, `t_end` ≤ 0 | The default (1e-6, 1e-9, 0, 20). |
| `max_steps` outside 1–10⁷, `samples` above 200 000 | Clamped. |
| Primary view range not finite with lo < hi, or an axis the system doesn't have | A warning; the app uses the default view. |

## Saving safely

A save never leaves a half-written file. The bytes go to a temporary file
`.sepx-<pid>-<nanos>-<n>.tmp` in the same folder (a short name, so a target name close to the
system's 255-byte limit still saves), are flushed to disk, and the temporary file is renamed
over the target (an atomic replace on every OS). On Unix the original file's permissions are
kept and the folder is synced. Saving through a symbolic link writes the link's target and
keeps the link, also when the target doesn't exist yet. A file marked read-only is refused on
every OS ("the file is read-only. Make it writable, or save a copy with Save as…"), as Windows
refuses to replace it, rather than silently replaced. On any error the temporary file is removed
and the original is untouched. Exports (`.tex`, `.dat`, `.csv`, `.svg`, `.png`) are written the
same way.

## Opening by double-click

Installers register `.sepx` (MIME type `application/x-separatrix-project`, a subclass of
`application/json`; on macOS the exported type `io.github.uncrayon.separatrix.project`, which
conforms to `public.json`, and the document type claims it through `contentTypes`
(`LSItemContentTypes`) as well as the extension, so a JSON editor that claims `public.json`
isn't preferred; check with `mdls -name kMDItemContentType x.sepx` after installing). Opening a second file while Separatrix runs sends it to the running
window. The AppImage build can't register file types; open files from inside the app there.

## Known limitations

- **macOS: Quit from the Dock, AppleScript, logout and shutdown.** Cmd+Q and the app menu's
  Quit go through the unsaved-changes guard (the menu item closes the window, which asks). The
  other routes send `terminate:` to the app delegate; tao 0.37 doesn't implement
  `applicationShouldTerminate:`, so Separatrix can't answer it and quits without asking. On
  Windows, logoff and shutdown (`WM_QUERYENDSESSION`) likewise reach no guard. Save before
  logging out.
