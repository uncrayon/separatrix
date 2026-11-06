# Project files

A project file, with the extension `.sepx`, holds what you typed and chose: the equations, the
parameters and their ranges, the start points, the view, the solver settings and the export
settings. It holds no computed data, so a file is a few kilobytes and travels by email or a course
platform. Separatrix recomputes the trajectories, nullclines and fixed points when it opens one.

## Save a project

- **Save**: Ctrl+S (⌘S on a Mac), or the disk icon at the top of the window. The first save asks
  for a file name.
- **Save as…**: Ctrl+Shift+S (⌘⇧S on a Mac) saves a copy under a new name.

A dot after the project's name at the top of the window means it has unsaved changes. A save never
leaves a half-written file. Separatrix writes a temporary file next to it and then replaces the
old file in one step.

## Open a project

Any of these opens a project. If the current project has unsaved changes, Separatrix asks first.

- Click **Open…** in the template gallery, or the folder icon at the top of the window, or press
  Ctrl+O (⌘O on a Mac).
- In the gallery, click a project under **Recent**. It lists the last 8 projects you opened or
  saved that still exist, newest first.
- Drag a `.sepx` file onto the Separatrix window.
- Double-click a `.sepx` file. The Windows, macOS, `.deb` and `.rpm` installers register the file
  type. The AppImage can't, so open files from inside the app there.

If Separatrix is already running, a project you double-click opens in the running window.

## Start a new project

Ctrl+N (⌘N on a Mac) or the page icon at the top of the window starts an empty system. **Templates…**
opens the gallery of examples.

## Share a project

Send the `.sepx` file. A student who opens it sees the same system, start points and view, and
Separatrix recomputes everything else. A file saved by a newer version of Separatrix opens with a
warning, and saving it keeps the parts this version doesn't use. A file that needs a newer version
says so and asks you to update.

If a part of a file doesn't fit the system, such as a 3D view of a two-state system, Separatrix
opens the rest, shows the default view, and says what it changed.

## Open the example projects

The guide's [example projects](examples/) open in each view. Download one and open it as above.

The full format, with every key and every repair, is in [docs/project-format.md](../project-format.md).
