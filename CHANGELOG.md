# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]
### Added
- Pseudo-arclength continuation in the core (`dyn_analysis::continue_branch`), modelled on
  BifurcationKit.jl (MIT). It follows a branch of equilibria of a flow, or of fixed points of `f^k`
  for a map, as one parameter varies, through folds. Each step predicts along the tangent (first
  step) or the secant and corrects by Newton's method on the bordered system with the exact
  Jacobians. The step grows after fast corrections and halves after slow or failed ones, or when
  successive tangents turn by more than the angle limit, so the branch does not jump. The last
  point of a branch that leaves the parameter range lies exactly on its edge. Each point carries
  its eigenvalues (of `J`, or of `D f^k` for a map), its unstable count and its stability. Not yet
  reachable from the app.
- Special points on a continuation branch (`Branch::special`), each located by bisection in
  arclength to `|Δp| ≤ 1e−13 (1 + |p|)` and inserted into the branch: folds (LP), Hopf points (H)
  and branch points (BP) of flows, and folds, branch points, flips (PD) and Neimark–Sacker points
  (NS) of maps. One table of signed test functions finds them: `ṗ` of the tangent, the determinant
  of the bordered Jacobian, the largest `Re λ` of a complex pair (so a neutral saddle's real pair
  `λ, −λ` is never a Hopf point), `det(D f^k + I)`, and the largest `|μ|` of a complex pair minus 1.
  A step is kept only when its change in the unstable count is the jump of the one test that
  changed sign (none: no change), so a Hopf point inside a step whose pair was still real at one
  end is not missed. The next step is limited to 1.5 times the predicted arclength to the nearest
  crossing of the stability boundary, so two close crossings are not stepped over together. A
  Hopf point carries `ω = Im λ` and the first Lyapunov coefficient `ℓ₁` (Kuznetsov's formula,
  with `⟨q, q⟩ = 1/2` so the normal form `ṙ = μr + ar³, θ̇ = ω` gives `ℓ₁ = a/ω`); a
  Neimark–Sacker point its angle `θ = arg μ`. At a special point the critical eigenvalues count as
  neutral, so it is not stable and its unstable count is the smaller of its two sides'. Checked
  against the saddle-node, pitchfork and transcritical normal forms at `r = 0`, the Hopf normal
  form (`ω = 1`, `ℓ₁ = ∓1`), Sel'kov's Hopf points `b = √0.12, √0.72` at `a = 0.08`, the
  Brusselator's `b = 1 + a²`, the logistic map's flip at `r = 3`, Hénon's fold and flip at
  `a = −(1 − b)²/4` and `3(1 − b)²/4`, and the delayed logistic map's Neimark–Sacker point at
  `r = 2` with `θ = π/3`. Not yet reachable from the app.
- Exports of the Lyapunov and orbit-diagram views (docs/exports.md). The export dialog lists each
  view's formats and says why the others are disabled. The Lyapunov view's LaTeX is its spectrum
  as a booktabs table under `\sisetup{uncertainty-mode = separate}`, one row per exponent,
  `$\lambda_{1}$ & \num{0.906 +- 0.004}`, then Σλ against ⟨tr J⟩ and their difference. The error
  is rounded to one significant digit (two when it starts with 1) and the value to the same place,
  from the exact decimal value, so the text is the same on every platform. The caption names the
  system and parameters, the integrator with its step or tolerances, the total time and the
  transient, and the count and interval of the renormalizations. Its CSV is `t,lambda_1,…` with a
  `.meta.json`, and its PNG and SVG are the view's plot. An orbit diagram's LaTeX figure has the
  screen's frame and a dot per iterate up to 5,000; above that Save writes `<base>-points.png`, a
  density image with one pixel column per parameter value under vector axes, and Copy keeps one
  iterate in k. Its CSV is `r,n,x` for every finite iterate. Its SVG has vector axes with the
  density as an embedded PNG. Figures ask the core for one column per pixel of their image (300
  dpi for LaTeX). New commands: `export_lyapunov_latex`, `export_lyapunov_csv` (the core measures
  the view's request again and keeps the last run), `export_orbit_latex`, `save_orbit_latex`,
  `export_orbit_csv` and `save_csv_file`.
- The orbit diagram (Strogatz §10.2, Figure 10.2.7): a new choice of the View control, Orbit, for
  maps with a parameter (disabled with the reason for a map without one). It sweeps a parameter,
  the first by default, over its slider's range, or over From and To in the inspector's Orbit
  diagram panel, with one column per device pixel, each at its pixel's center. Every column is
  iterated from the selected start point, else the first: `N₀ = 1,000` iterates are dropped and
  `N = 200` drawn as a density, binned per pixel with log-scaled opacity in `plot-fp`, on a 2D
  canvas. A vertical line in `accent` marks the parameter's value, and clicking a column sets the
  parameter to that column's `r`. Zooming or panning across asks the core for the new range;
  moving the slider asks for nothing. For a one-dimensional map the inspector lists the period
  doublings Strogatz's way (`r₁ = 3.000000`, `r₂ = 3.449490`, `r₃ = 3.544090`, `r₄ = 3.564407`
  for the logistic map), from the same `period_doublings` the chaos gate checks. The status bar
  reads `Orbit 19 ms` and `774 columns · 200 iterates each`, and its tooltip gives the round trip
  and the drawing time. In the release app the default request takes 19 ms in Rust and 53 ms
  round trip, and draws in 4 ms (medians of seven runs), so the plain JSON response is kept.
  Projects save the view as kind `"orbit"` with the axis `{ "kind": "param", "name": "r" }` and an
  `orbit` object of `transient` and `kept` (format version 1, additive). The browser preview
  iterates the map and finds the doublings in JavaScript. The View control's segments have the
  narrow padding, so five labels fit in projector mode at 1,024 px.
- The Lyapunov view (Strogatz §9.3, §10.5): a new choice of the View control, λ, for flows and
  maps. It runs `compute_lyapunov` on the selected start point (else the first) with the studio's
  solver and plots the running estimate of each exponent against `t` (or `n`) in its trajectory
  color, its final value dashed and λ₁ … λₙ beside it, fitted to each run and zoomed and panned
  like the phase plane. The inspector's Lyapunov panel sets the transient, the time (or iterates)
  measured, the renormalization interval τ and the blocks, and reads out `λ₁ = 0.906 ± 0.004`,
  `Σλ`, `⟨tr J⟩` (a map's `⟨ln |det J|⟩`) and their difference. The status bar reads
  `10⁵ renormalizations · RK4 · h 0.01`. The method picker disables the methods the core can't
  use for the variational equations, and the step limit is raised to cover the whole run.
  Projects save the view as kind `"lyapunov"` with a `lyapunov` object and the axis kind
  `exponent` (format version 1, additive). The browser preview computes the spectrum in
  JavaScript (RK4 and Gram–Schmidt). The View control's segments now take their labels' widths,
  so five fit at 1,024 px.
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
  Trajectories keep their colors and get thicker in projector mode; fixed points are drawn in
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
- A fixed-step run that needs exactly the step limit (h = 0.05 to T = 10⁴ with 200,000 steps)
  finishes at T instead of stopping a sliver short with "Step limit": fixed steps sit at
  t₀ + k·h rather than accumulating rounding by repeated addition.
- Opening a system that has no separable H(q, p) while Störmer–Verlet or Yoshida 4 is chosen
  switches to Dormand–Prince 5(4), with a note in the Solver section, instead of failing every
  computation.
- A click on the orbit diagram sets the parameter of the column drawn under the pointer (it
  could land one column to the left), and the marker sits on that column.
- Opening a project in the Lyapunov view keeps the file's framing instead of refitting it to the
  first run.
- The core refuses an orbit-diagram transient above 100,000 iterates and a map Lyapunov run of
  more than 10⁸ iterates (`MAX_LYAPUNOV_ITERATES`), which used to keep the command busy with no
  way to cancel.
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