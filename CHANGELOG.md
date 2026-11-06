# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]
### Added
- Rosenbrock 2(3) for stiff systems: the L-stable implicit method of MATLAB's `ode23s`, adaptive,
  with the exact Jacobian and its own dense output, so sampling and Poincaré sections work.
  Dormand–Prince now runs Hairer's stiffness test. When its steps are held back by stability
  rather than accuracy, the trajectory says so, and the Solver panel offers "Use Rosenbrock 2(3)"
  while the status bar warns "Steps look stiff". On the relaxation oscillator at μ = 1000,
  Rosenbrock 2(3) takes 4,366 steps where Dormand–Prince takes 1,585,080. Projects save the
  method as `"rosenbrock23"`; Lyapunov runs refuse it. The chaos gate solves Robertson's reaction
  to t = 40 within 1.6·10⁻⁷ of the published values.
- Symplectic methods for Hamiltonian systems (Strogatz §6.5): Störmer–Verlet (leapfrog, order 2)
  and Yoshida 4 (order 4) integrate an `H(q, p)` that splits as `T(p) + V(q)`, and keep its energy
  error bounded over long runs where Runge–Kutta drifts. The method picker offers them only for
  such a system and says why otherwise; the core refuses them for any other flow and for Lyapunov
  runs. Every trajectory of a Hamiltonian flow reports `H` at its start and its largest drift, and
  the Solver panel reads it out as `|ΔH| max = 3.1 × 10⁻⁷`. Projects save the methods as
  `"verlet"` and `"yoshida4"`. The chaos gate steps the pendulum over 10⁵ periods: Verlet at
  h = 0.05 stays within 2.7·10⁻⁴ and Yoshida 4 at h = 0.1 within 3.9·10⁻⁶, with no growth from the
  first 10³ periods to the last, while RK4 at h = 0.1 grows 98-fold.
- Lyapunov spectrum (Strogatz §9.3, §10.5): `dyn_analysis::lyapunov_spectrum` measures every
  exponent of a flow or a map by Benettin's QR method. A flow integrates its variational
  equations with the exact Jacobian. Each exponent gets a block standard error, and the sum is
  checked against the mean trace of `J` (flows) or the mean `ln |det J|` (maps). The result
  carries the running estimates against `t`. The `compute_lyapunov` command exposes it to the UI
  and refuses `τ ≤ 0`, `total ≤ 0`, `blocks < 2` and runs of more than 10⁷ QR steps with a
  message.
- Orbit diagrams of maps (Strogatz §10.2): `dyn_analysis::orbit_diagram` iterates a map across
  a range of one parameter and keeps the iterates after a transient, with NaN once an orbit
  escapes. `dyn_analysis::period_doublings` locates the period-doubling points of a
  one-dimensional map from the diagram: it follows each `P`-cycle by Newton's method and bisects
  for the multiplier `∏ f′(x_i) = −1`. The `compute_orbit_diagram` command exposes both, up to
  4096 parameter values and 4·10⁶ points, and refuses flows, indices outside the system, an
  invalid range, and doublings of a map with more than one state.
- Chaos gate (`scripts/chaos-gate.sh`): the Lorenz, Hénon and logistic templates reproduce
  published exponents (Lorenz 0.9056, 0, −14.5723 and Σλ = −41/3; Hénon 0.41922, −1.62319 and
  Σλ = ln 0.3; logistic `r = 4` ln 2) and the logistic period doublings (`r1 = 3`,
  `r2 = 1 + √6`, `r3 = 3.5440903`, `r4 = 3.5644073`, each within 1e−6) within stated tolerances.
  It prints `(r3 − r2)/(r4 − r3)` beside Feigenbaum's δ for information, and four wrong claims
  fail.
- Evaluation benchmark (`cargo run --release -p dyn-analysis --example chaos_bench`): the
  compiled Lorenz spectrum at the Lyapunov view's T = 10³ takes 98 ms (median of 7 runs, budget
  500 ms), 3.2× the hand-written Rust field. Every chaos workload is inside its budget, so the
  interpreter stays and no Cranelift JIT is built. `docs/performance.md` has the rule, the
  numbers and the limiter.
- 3D view (Strogatz ch. 9): the View control's **3D** shows any three states of a flow with at
  least 3 states in a box with ticks and axis names, seen from a camera you turn by dragging (or
  the arrow keys), zoom with the wheel (or `+` and `−`) and reset with a double-click (or `0`).
  Trajectories keep their colours and get thicker in projector mode; fixed points are drawn in
  Strogatz's glyphs and are searched over the third axis's range too, so the Lorenz points `C±`
  appear whatever the slice. The toolbar and the View panel pick the three states (a state
  already on another axis swaps with it). Lines are drawn with WebGL through three.js, loaded
  only for this view; without WebGL the view says so and draws on a 2D canvas, redrawing the
  lines when dragging stops. The box's edges are drawn under the trajectories (on screen, in PNG
  and in SVG), and the axis names stay on the canvas at any zoom. Turning the box never asks the
  core for a new portrait. Project files save the view as `kind: "space"` with `z_axis`, `z_range`
  and `camera` (additive; the format stays at version 1).
- Exports of a 3D view: PNG draws the box as the screen does (three.js offscreen at the PNG's
  scale, or 2D lines without WebGL) with pHYs at `96·scale` dpi, and SVG is the same picture
  projected to 2D, one path per trajectory. A small figure zooms the camera out so the axis names
  stay inside. The LaTeX figure is disabled with the reason "LaTeX figures of the 3D view aren't
  available yet. Export PNG or SVG, or switch to a phase plane.", which `export_figure_latex` also
  returns for a request with `zAxis`; the LaTeX equations and fixed points and the CSV are
  unchanged.
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
  Every chapter from 2 to 8 has a passing claim, and a template citing a chapter 2–8 section
  needs a claim of its own (both are tests). Four deliberately wrong claims must fail the same
  evaluation (also a test), so the gate is shown to catch a wrong result.
- 18 Strogatz templates, each with its claim in the gate: ẋ = sin x (§2.1), logistic growth
  (§2.3), the transcritical (§3.2) and supercritical pitchfork (§3.4) bifurcations, the bead on a
  rotating hoop (§3.5, pitchfork at `γ = 1`), the imperfect bifurcation (§3.6,
  `h_c = 2/(3√3)`), the nonuniform oscillator (§4.3, `T = 2π/√(ω² − a²)`), Romeo and Juliet (§5.3,
  a center with period `2π/√(ab)`), the double well (§6.5) and a reversible system (§6.6), the
  relaxation oscillator (§7.5, Van der Pol at `μ = 10` in the Liénard plane, period 19.0784),
  the saddle-node in the plane (§8.1), the supercritical Hopf
  bifurcation (§8.2, radius `√μ`, period 2π), the chlorine dioxide–iodine–malonic acid reaction
  (§8.3, Hopf at `b_c = 3a/5 − 25/a`), the driven pendulum and Josephson junction (§8.5), coupled
  oscillators (§8.6, locked at `(K₂ω₁ + K₁ω₂)/(K₁ + K₂)`), the Poincaré map of `ẋ + x = A sin t`
  (§8.7, fixed point `−A/2`, opening as its return map) and the forced double-well oscillator
  (§12.5, opening on its stroboscopic section).
- The gallery groups the templates under Strogatz's chapters, in the book's order, and draws a
  thumbnail only once its card scrolls into view. An equation too wide for its card is scaled
  down to fit. Templates can open on a Poincaré section (`view.kind: "section"`, the plane's
  state by name) and skip a `transient` of crossings. The Rössler template now opens in 3D.
- Poincaré section view (Strogatz §8.7): the View control's **Section** shows where a flow's
  orbits cross a plane `x_i = c` (Increasing, Decreasing or Both), or its state once per forcing
  period (stroboscopic, period `T` and phase `φ`: samples at `t = φ + kT`), as dots in two chosen
  states, or as the return map `x_k → x_{k+1}` with the diagonal when both axes are one state.
  **Skip crossings** hides each orbit's first crossings, and the status bar reads "N crossings,
  first k per orbit skipped". Backward time is off in a section view, which follows each orbit
  forward only. A click on a plane section starts an orbit on the plane (its state is the plane's
  value); a stroboscopic one starts at `t₀ = φ`. Project files save the view as `kind: "section"`
  with a `section` object (additive; the format stays at version 1).
- Exports of a section view: CSV writes each orbit's crossings (`t` and every state), and its
  metadata adds `"section": "plane z = 27 (increasing)"` (or `"stroboscopic, period T, phase φ"`)
  and `"skipped_crossings"`, and its note says whether the start point can be a row (a
  stroboscopic sample at `t0` is one). The LaTeX figure draws the crossings as dots by the map rule
  (`% Section of <ic>`; rasterised or thinned past 5,000 points), a return map as pairs over the
  diagonal (`% Return map of <ic>`, axis names `x_{k}` and `x_{k+1}`), and names the section and
  the skipped crossings in its header. SVG and PNG draw the section as the screen does. The
  dialog hides the field and nullcline options, says why there are no fixed points, and counts
  crossings.
- Poincaré sections in the core (Strogatz §8.7): `dyn_solve::section` returns the crossings of a
  hyperplane `x_i = c` (rising, falling or both, located by Brent's method on the dense output to
  within a few ulps of `t`, with the held state exactly `c`, so late crossings of a long run stay
  on the plane) or the stroboscopic samples `t_k = phase + k·T` of a periodically forced flow, skipping a
  transient of crossings. The stroboscopic `phase` is absolute (`t ≡ phase mod T`), so
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