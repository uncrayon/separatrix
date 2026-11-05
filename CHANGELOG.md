# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]
### Added
- Cobweb plots for one-dimensional maps (Strogatz §10.1): the graph of `f` (split at poles,
  jumps and the edges of its domain), the diagonal `x_{n+1} = x_n` and each orbit's staircase,
  with the fixed points on the diagonal and their multipliers `λ = f′(x*)`. The logistic map
  template opens as a cobweb (`r = 3.2`, `x₀ = 0.1`: the orbit winds out from the unstable
  `x* = 0.6875` onto the 2-cycle). A **transient** skips the first iterates of every orbit, in
  the cobweb and the time series. Exports: the LaTeX figure draws the cobweb (`% Diagonal`,
  `% Graph of f`, `% Cobweb of <ic>`), a one-dimensional map's fixed-point table lists
  `λ = f′(x*)` and `|λ|`, and the CSV metadata records the transient. Project files keep the
  view's `kind` and the solver's `transient` (additive keys; the format stays at version 1).
- Strogatz validation suite (`scripts/strogatz-gate.sh`): checks the chapter 2–8 templates
  against the book's own results (fixed points and their classification, the saddle-node at
  `r = 0`, the Sel'kov Hopf values `b = √0.12` and `√0.72`, pendulum energy drift, the Van der Pol
  limit cycle's period `T ≈ 6.66329` and amplitude `max x ≈ 2.00862` at `μ = 1`, measured on a
  Poincaré section) and that every template's section is in the 2nd edition's table of contents.
- Poincaré section view (Strogatz §8.7): the View control's **Section** shows where a flow's
  orbits cross a plane `x_i = c` (Increasing, Decreasing or Both), or its state once per forcing
  period (stroboscopic, period `T` and phase `φ`: samples at `t = φ + kT`), as dots in two chosen
  states, or as the return map `x_k → x_{k+1}` with the diagonal when both axes are one state.
  **Skip crossings** hides each orbit's first crossings, and the status bar counts "N crossings,
  k skipped". A click on a plane section starts an orbit on the plane (its state is the plane's
  value); a stroboscopic one starts at `t₀ = φ`. Project files save the view as `kind: "section"`
  with a `section` object (additive; the format stays at version 1).
- Exports of a section view: CSV writes each orbit's crossings (`t` and every state), and its
  metadata adds `"section": "plane z = 27 (increasing)"` (or `"stroboscopic, period T, phase φ"`)
  and `"skipped_crossings"`. The LaTeX figure draws the crossings as dots by the map rule
  (`% Section of <ic>`; rasterised or thinned past 5,000 points), a return map as pairs over the
  diagonal (`% Return map of <ic>`, axis names `x_{k}` and `x_{k+1}`), and names the section and
  the skipped crossings in its header. SVG and PNG draw the section as the screen does. The
  dialog hides the field and nullcline options, says why there are no fixed points, and counts
  crossings.
- Poincaré sections in the core (Strogatz §8.7): `dyn_solve::section` returns the crossings of a
  hyperplane `x_i = c` (rising, falling or both, located by Brent's method on the dense output) or
  the stroboscopic samples `t_k = t0 + phase + k·T` of a periodically forced flow, skipping a
  transient of crossings. Over IPC the stroboscopic `phase` is absolute (`t ≡ phase mod T`), so
  every orbit is sampled at the same forcing phase whatever its start time. `compute_portrait` takes an optional `section` and then returns each
  flow trajectory as its crossings; `transient` counts the skipped ones. The studio's section view
  builds on this.

### Changed
- The View control shows the options of the system's kind: Phase, Series and Section for a flow,
  Phase, Series and Cobweb for a map (an option the system can't show stays, disabled, with its
  reason).
- Rewrite as a cross-platform desktop app (Tauri 2 + Rust core). No Python, no server.
- Renamed the product to **Separatrix** (app identifier `io.github.uncrayon.separatrix`).
- New Cargo workspace: `dyn-core` (shared `VectorField` contract), `dyn-expr`, `dyn-solve`, `dyn-analysis`, and the Tauri shell in `app/`.
- New CI: Rust tests on Linux, macOS and Windows; release workflow that builds unsigned installers.

- Text mode is compiled by the core's text parser: every line is stored as typed in the system's
  one input format, and switching modes asks the core to convert (`convert_source`), which checks
  that each converted line, and the whole system, means exactly the same and keeps every name.
  A line with an error is kept as typed with a note; a valid line the other format can't write
  exactly (a number like `9.109e-31` in LaTeX) calls the switch off, with a note, since kept as
  typed it would mean something else there. The UI's hand-written LaTeX/text translators are gone. Template text is generated (`templates.text.generated.json`).
- Text mode has `root(x, n)` for the `n`-th root (`\sqrt[n]{x}`).

### Fixed
- A one-dimensional fixed point on a kink of `f` (the tent map's peak at `r = 1`, `|x|` at 0)
  has no multiplier and is non-hyperbolic, instead of λ = 0 "superstable"; an interval of fixed
  points (`f(x) = x` on `[a, b]`) is reported by its two ends as non-isolated instead of once per
  Newton seed.
- The Nullclines toggle is shown whenever nullclines are drawn, including time-series views of
  2-D and larger systems.
- Diagnostics keep the hyphens of their prose ("right-hand side") instead of turning them into
  minus signs, and a prime (`x'+y'`) is no longer taken for a quote.
- Names spelled as one identifier that begin with a function name (`\mathrm{cost}`, `\mathrm{signal}`, the word `cost` in text mode) compile as that name, with a warning suggesting the function, instead of failing.
- A selected fixed point stays selected while the list is recomputed and re-sorted (it is tracked by position) and is cleared when it disappears.
- The fixed-point search never uses more than 1000 Newton seeds, in any dimension (10D and up used 2^d).
- The UI's IPC types are the ts-rs bindings generated from Rust (`types.ts` re-exports them); states carry `derived` for those introduced by order reduction.

### Known limitations
- macOS: quitting from the Dock, through AppleScript or by logging out or shutting down skips the
  unsaved-changes question (tao doesn't implement `applicationShouldTerminate:`); Cmd+Q and the
  app menu's Quit do ask. Windows logoff and shutdown don't ask either. See
  docs/project-format.md, "Known limitations".

### Removed
- Python backend (FastAPI, SciPy/Numba solvers, job queue, SQLite), the web frontend and the PyPI packaging. The `equation-phase-portrait-tool` package on PyPI is deprecated; v0.1.0 remains in git history.

## [0.1.0] - 2025-10-02
### Added
- Initial PyPI release as `equation-phase-portrait-tool`.
- Single-source version at `equation_phase_portrait_tool/_version.py`.
- Top-level import package `equation_phase_portrait_tool` exposing `__version__` and `backend`.
- Module CLI entry (`__main__.py`) and compatibility shim (`main.py`) for flexible invocation.
- Console scripts: `eqpp-server` (existing) and `equation-phase-portrait-tool`.
- Packaging configuration updates in `pyproject.toml` (console scripts, package discovery, dev extras).
- Included static assets under `backend/static` and excluded development artefacts (`backend/jobs.sqlite`, `backend/worker_runs.log`) via `MANIFEST.in` and packaging config.
- CI workflow `.github/workflows/release.yml` for build/test and tag-triggered publishing to TestPyPI/PyPI.
- Release documentation: `RELEASE.md` and README release section.
- Dev/test dependency adjustments (added `requests` to dev requirements to support integration tests).

### Fixed
- Packaging and entry-point issues to ensure a clean wheel/sdist without local dev artefacts.

### Notes
- This release was published to TestPyPI and PyPI on 2025-10-02.
- See `RELEASE.md` for the release procedure and verification steps.