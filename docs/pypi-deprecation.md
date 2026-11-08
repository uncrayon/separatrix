# Retire the equation-phase-portrait-tool package on PyPI

Before Separatrix, this repository published a Python package,
[equation-phase-portrait-tool](https://pypi.org/project/equation-phase-portrait-tool/), version
0.1.0. Separatrix replaces it, and the package gets no more fixes. This page holds the notice for
its PyPI page and the steps that publish it. Only the owner can run them, because they need the
PyPI account.

The plan is one last release, 0.1.1, that changes nothing but the notice and a warning on import,
and then archiving the project on PyPI. Do not yank 0.1.0. Courses and scripts that pin
`equation-phase-portrait-tool==0.1.0` keep working, and a yanked release would break them.

## The notice

This text becomes the package's `README.md`, which PyPI shows as the project description.

```markdown
# equation-phase-portrait-tool is no longer maintained

This package will get no more updates or fixes. Use **Separatrix** instead.

Separatrix is the desktop app that replaces this package. It draws phase portraits, slope
fields, time series, cobweb plots, 3D flows, Poincaré sections, Lyapunov exponents, orbit
diagrams and bifurcation branches. It runs on Windows, macOS and Linux with no Python install,
and it works offline.

- Download Separatrix: https://github.com/uncrayon/separatrix/releases/latest
- Install guides for Windows, macOS and Linux: https://github.com/uncrayon/separatrix/tree/main/docs/install

Version 0.1.1 is the last release. It is the same as 0.1.0, except for this notice and a
`DeprecationWarning` when you import the package. Version 0.1.0 stays available on PyPI, so
`pip install equation-phase-portrait-tool==0.1.0` keeps working.
```

## Before you start

You need:

- Python 3.10 or newer, and `git`.
- A PyPI API token for the project. On PyPI, open **Account settings** > **API tokens** >
  **Add API token**, and set its scope to `equation-phase-portrait-tool`. Copy the token, which
  starts with `pypi-`. PyPI shows it only once.

Do the work in a temporary clone, never in your working copy. The release comes from the last
commit that still had the Python package, `28ff415^`, the parent of the commit that removed it.

## Build version 0.1.1

1. Clone the repository into a temporary folder and check out the last Python tree:

   ```sh
   git clone https://github.com/uncrayon/separatrix.git /tmp/eqpp-0.1.1
   cd /tmp/eqpp-0.1.1
   git checkout 28ff415^
   ```

   Git says you are in "detached HEAD" state. That is expected, because you will not commit
   anything.

2. Delete the old build output. That commit has the 0.1.0 files in `dist/` and stale copies of the
   code in `build/`. If you keep them, the upload in the next section sends the 0.1.0 files again
   and fails.

   ```sh
   rm -rf build dist
   ```

3. Set the version to 0.1.1, put the notice in `README.md`, and add the warning on import. This
   script makes all three changes. Run it from the same folder:

   ```sh
   python3 - <<'EOF'
   import pathlib

   url = "https://github.com/uncrayon/separatrix/releases/latest"
   guides = "https://github.com/uncrayon/separatrix/tree/main/docs/install"

   def edit(path, old, new):
       p = pathlib.Path(path)
       text = p.read_text(encoding="utf-8")
       assert old in text, f"{path}: {old!r} not found"
       p.write_text(text.replace(old, new, 1), encoding="utf-8")

   edit("pyproject.toml", 'version = "0.1.0"', 'version = "0.1.1"')
   edit("pyproject.toml",
        'description = "Equation Phase Portrait Tool - local distribution"',
        'description = "No longer maintained. Use Separatrix instead."')
   edit("equation_phase_portrait_tool/_version.py", '"0.1.0"', '"0.1.1"')
   edit("equation_phase_portrait_tool/__init__.py",
        "from ._version import __version__",
        "import warnings\n\n"
        "warnings.warn(\n"
        '    "equation-phase-portrait-tool is no longer maintained. "\n'
        f'    "Use Separatrix instead: {url}",\n'
        "    DeprecationWarning,\n"
        "    stacklevel=2,\n"
        ")\n\n"
        "from ._version import __version__")

   pathlib.Path("README.md").write_text(f"""# equation-phase-portrait-tool is no longer maintained

   This package will get no more updates or fixes. Use **Separatrix** instead.

   Separatrix is the desktop app that replaces this package. It draws phase portraits, slope
   fields, time series, cobweb plots, 3D flows, Poincaré sections, Lyapunov exponents, orbit
   diagrams and bifurcation branches. It runs on Windows, macOS and Linux with no Python install,
   and it works offline.

   - Download Separatrix: {url}
   - Install guides for Windows, macOS and Linux: {guides}

   Version 0.1.1 is the last release. It is the same as 0.1.0, except for this notice and a
   `DeprecationWarning` when you import the package. Version 0.1.0 stays available on PyPI, so
   `pip install equation-phase-portrait-tool==0.1.0` keeps working.
   """, encoding="utf-8")
   print("version 0.1.1, notice and warning in place")
   EOF
   ```

   The script stops with an error that names the file if the tree is not the one it expects.

4. Build the package and check it:

   ```sh
   python3 -m venv .venv
   . .venv/bin/activate
   python -m pip install --upgrade build twine
   python -m build
   python -m twine check dist/*
   ```

   `dist/` now holds exactly two files, `equation_phase_portrait_tool-0.1.1.tar.gz` and
   `equation_phase_portrait_tool-0.1.1-py3-none-any.whl`, and `twine check` prints `PASSED` for
   both.

5. Check the warning. This command installs the new wheel without its dependencies and imports the
   package with warnings turned into errors:

   ```sh
   python -m pip install --no-deps dist/equation_phase_portrait_tool-0.1.1-py3-none-any.whl
   python -W error::DeprecationWarning -c "import equation_phase_portrait_tool"
   ```

   Python stops with `DeprecationWarning: equation-phase-portrait-tool is no longer maintained.`
   That is the result you want.

## Upload and archive

1. Upload the two files. When `twine` asks, type `__token__` as the username and paste your API
   token as the password:

   ```sh
   python -m twine upload dist/*
   ```

2. Open <https://pypi.org/project/equation-phase-portrait-tool/>. Check that it shows version
   0.1.1 and the notice.
3. Click **Manage project**, then **Settings**. Under **Project status**, click
   **Archive project** and confirm. PyPI then marks the project as archived and accepts no new
   uploads. You can unarchive it from the same page if you ever need to.
4. Leave 0.1.0 as it is. Do not yank it.
5. Delete the temporary clone:

   ```sh
   cd ~ && rm -rf /tmp/eqpp-0.1.1
   ```
