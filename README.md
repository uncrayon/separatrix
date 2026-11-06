# Separatrix

A desktop app for exploring dynamical systems, built for students and professors of mathematics, physics and applied science. Type a system the way it is written on the board, and see its phase portrait move as you drag.

> **Status:** rewrite in progress. Milestones 1 and 2 are done: the live phase plane; exports, project files and cobweb plots; Poincaré sections, the 3D view, and a template gallery whose chapter 2–8 examples reproduce Strogatz's results. The previous Python/web version (v0.1.0, PyPI `equation-phase-portrait-tool`) is deprecated and lives in git history.

## What it will do (v1)

- ODEs from 1D slope fields to n-dimensional systems, and discrete maps (logistic, Hénon).
- Cobweb plots of one-dimensional maps x_{n+1} = f(x_n), with a transient to hide the first iterates.
- Poincaré sections of flows: a plane x_i = c or a stroboscopic section of a forced oscillator, plotted in two states or as the return map x_k → x_{k+1}.
- Phase portraits, nullclines, fixed points with stability, 2D projections of any variables.
- An interactive 3D view of any three states of a flow (Lorenz, Rössler): drag to turn the box, scroll to zoom, with the fixed points in Strogatz's glyphs. Drawn with WebGL (three.js), or on a 2D canvas where WebGL is unavailable.
- The Lyapunov spectrum of a flow or a map: the View control's λ view plots the running estimate of every exponent against t (or n) and reads out λ₁ = 0.906 ± 0.004 with its block standard error, and checks Σλ against the mean trace of the Jacobian.
- The orbit diagram of a map: the period-doubling route to chaos of the logistic map, one column per pixel, darker where the orbit returns more often. Click a column to set the parameter there; the inspector lists the period doublings r₁ = 3.000000, r₂ = 3.449490, … of a one-dimensional map.
- Continuation of equilibria and of a map's fixed points and cycles: the Branch view follows a fixed point as a parameter changes, solid where it is stable and dashed where it is not, and marks where it folds (LP), loses stability at a Hopf point (H), crosses another branch (BP), period-doubles (PD) or undergoes a Neimark–Sacker bifurcation (NS). Sel'kov's Hopf points come out at b = 0.346410 and 0.848528 with their frequency ω and the sign of ℓ₁. Switch branch at a BP or PD row follows the second branch, so the logistic cascade r₁ … r₄ = 3.000000, 3.449490, 3.544090, 3.564407 takes three clicks. Click a row or a branch to set the parameter there.
- Symplectic integration for Hamiltonian systems, and an implicit solver for stiff ones, suggested when Dormand–Prince's steps look stiff.
- A gallery of 31 examples from Strogatz's *Nonlinear Dynamics and Chaos* (2nd edition), grouped by chapter, each citing the section it comes from (the opening slope field cites none). Every chapter 2–8 example reproduces a result of the book, checked by the Strogatz gate below.
- Export to LaTeX, PNG, SVG and CSV, the Lyapunov spectrum as a table with siunitx uncertainties and the orbit diagram as a figure. Shareable project files.
- Windows, macOS and Linux. Works offline.

## Layout

| Path | What |
| --- | --- |
| `crates/dyn-core` | The `VectorField` trait every solver and analysis uses |
| `crates/dyn-expr` | Equation parsing, inference (states, parameters, ODE or map), evaluation and exact Jacobians |
| `crates/dyn-solve` | Integrators (Euler, RK4, Dormand–Prince, Störmer–Verlet, Yoshida 4, Rosenbrock 2(3)), map iteration, events, Poincaré sections |
| `crates/dyn-analysis` | Fixed points, stability, slope fields, nullclines, the graph of a 1-D map's f, and later chaos and continuation |
| `app/` | Tauri shell (`src-tauri/`) and the React + TypeScript interface (`src/`) |

## Develop

Prerequisites: Rust (stable), Node 22. On Linux also `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev`.

```bash
cargo test --workspace --exclude separatrix
```

```bash
cd app && npm install && npm run tauri dev
```

### The Strogatz gate

```bash
bash scripts/strogatz-gate.sh
```

prints one row per claim the chapter 2–8 templates make (fixed points and their classification, bifurcation values, periods, a limit cycle's radius, a locked frequency, a Poincaré map's fixed point, energy drift), with the book's value, the measured one and the tolerance, and exits non-zero if any row fails. Expected values come from the book's formulas; the one exception is the relaxation oscillator's period at μ = 10, which the book gives only as the asymptote μ(3 − 2 ln 2) (printed as an info row), so the gate checks the known numerical value 19.0784 and that two solvers agree on it to 1e−5. The pendulum's energy drift over t = 100 is held to 1e−6 at rtol 1e−8 and to 5e−5 at the app's default tolerances (rtol 1e−6). A test also hands the gate four wrong claims (a pitchfork at r = 0.1, a transcritical point at r = 10⁻⁵, a Van der Pol period of 6.0, a drift of 1e−9) and requires each to fail. `app/src-tauri/tests/strogatz.rs` holds the claims; a template that cites a chapter 2–8 section can't be added without one.

### The chaos gate

```bash
bash scripts/chaos-gate.sh
```

runs `cargo test --workspace --release --test chaos`, so right after `cargo test --workspace --release` it reuses the build and takes about 10 s. It measures the Lyapunov spectra of three gallery templates through the same `compute_lyapunov` command the UI calls, and prints one row per published value with its measured value, its block uncertainty `±` and the tolerance. It exits non-zero if any row fails. The Lorenz system (σ = 10, r = 28, b = 8/3, RK4 at h = 0.01, T = 10⁴, τ = 0.1) reproduces Sprott's 0.9056, 0 and −14.5723 to 0.01, 0.01 and 0.02. A second run at h = 0.001 checks Σλ = −(σ + 1 + b) = −41/3 to 10⁻⁶. At h = 0.01, the app's default, RK4 is 10⁻⁴ off that sum, an error that shrinks like h⁴; an info row prints it. The Hénon map (a = 1.4, b = 0.3, 10⁶ iterates) reproduces 0.41922 and −1.62319 to 0.005, and Σλ = ln 0.3 to 10⁻⁹. The logistic map at r = 4 gives ln 2 to 0.005. An info column says whether each published value lies within 3·`±`. A test also hands the gate three wrong claims (Lorenz λ1 = 1.0, Lorenz Σλ = −13, Hénon λ1 = 0.5) and requires each to fail. `app/src-tauri/tests/chaos.rs` holds the claims.

The same gate checks the symplectic methods on the Hamiltonian pendulum `H = p²/2 − cos q` from `(q, p) = (1, 0)`, stepped over 10⁵ periods with `H` read after every step. Störmer–Verlet at h = 0.05 keeps `max |ΔH|` below 10⁻³ and Yoshida 4 at h = 0.1 below 10⁻⁴. For both, the largest `|ΔH|` over the last 10³ periods is at most twice the largest over the first 10³, so the energy error stays bounded instead of drifting. RK4 at h = 0.1 drifts and fails that rule, a fourth wrong claim the gate requires to fail.

It also solves Robertson's stiff reaction to t = 40 with Rosenbrock 2(3) at rtol 10⁻⁸ and checks each component against the SUNDIALS and Hairer–Wanner values (7.158271·10⁻¹, 9.185535·10⁻⁶, 2.841637·10⁻¹) to 10⁻⁵ relative, after a Dormand–Prince run at rtol 10⁻¹¹ confirms them. Dormand–Prince must flag the stiff runs, and Rosenbrock 2(3) must need at most a tenth of its steps on the relaxation oscillator at μ = 1000. On Robertson at rtol 10⁻⁸ the second-order method gains less, about a fifth of the steps, which an info row prints without gating. The wrong claim that Dormand–Prince at rtol 10⁻⁸ finishes Robertson within 2,000 steps must fail (it takes 34,856).

### The continuation gate

```bash
bash scripts/continuation-gate.sh
```

runs `cargo test --workspace --release --test continuation` and prints one row per claim with its reference value, the measured value, the difference Δ, the tolerance and PASS or FAIL. It exits non-zero if any row fails. Every reference is derived in closed form, and every branch is followed through the same `compute_continuation` command the UI calls, so the gate checks the request path as well as the maths. It covers 27 rows:

- the fold (LP) of the saddle-node `ẋ = r + x²` and the branch points (BP) of the transcritical `ẋ = rx − x²` and pitchfork `ẋ = rx − x³`, all at `r = 0`, with the transcritical branch staying on `x = 0` to 10⁻¹²;
- the Hopf normal form at `μ = 0` with `ω = 1`, and `ℓ₁ = −1` (supercritical) or `+1` (subcritical) to 10⁻⁶;
- no Hopf point on the neutral saddle `ẋ = y, ẏ = x + μy`;
- Sel'kov's glycolysis model at `a = 0.08`, with Hopf points at `b = √0.12` and `√0.72`, `ω = √0.2` and `√0.8`, to 10⁻⁸, and both Hopf points still found with steps up to 2;
- the Brusselator's Hopf point `b = 1 + a² = 5` with `ω = a = 2`;
- the logistic map's flip (PD) at `r1 = 3` to 10⁻⁶;
- the Hénon map's fold `a0 = −0.1225` and flip `a1 = 0.3675` at `b = 0.3`, from one branch followed both ways;
- the delayed logistic map's Neimark–Sacker point (NS) at `r = 2` with `θ = π/3`;
- branch switching at the pitchfork's BP onto `r = x²`, with both signs of `x`, and at the transcritical's onto `x = r`, both to 10⁻¹⁰;
- the logistic period-doubling cascade, switching at each flip onto the doubled cycle over `r ∈ [2.8, 3.57]`: `r2 = 1 + √6`, `r3 = 3.544090360` and `r4 = 3.564407266` to 10⁻⁶, and `r1 … r4` within 10⁻⁹ of the chaos gate's `period_doublings` on its 2,000-column orbit diagram.

The Δ of `r3` and `r4` (about 10⁻¹⁰) is the rounding of their 9-decimal references; the cascade agrees with `period_doublings` to about 10⁻¹³. An info row prints the Feigenbaum estimate `(r3 − r2)/(r4 − r3)` beside `δ = 4.669201609…`, ungated. Every other row has a tolerance of 10⁻⁹. `ℓ₁` uses Kuznetsov's normalization `⟨p, q⟩ = 1`, `⟨q, q⟩ = 1/2`, so `ṙ = μr + a r³, θ̇ = ω` has `ℓ₁ = a/ω`. A second test hands the gate seven wrong claims and requires each to fail: a pitchfork at `r = 10⁻⁴`, Sel'kov's first Hopf point at `b = 0.35`, `ω = 1.001`, a Hopf point on the neutral saddle, `r1 = 3.00001`, `r3 = 3.5441` and a switched pitchfork branch `x = r`. `app/src-tauri/tests/continuation.rs` holds the claims.

### Evaluation speed

```bash
cargo run --release -p dyn-analysis --example chaos_bench
```

times the chaos workloads (Lorenz Lyapunov spectra at T = 10⁴ and 10³, a logistic orbit diagram, the Hénon spectrum) on the compiled gallery systems and on the same systems written by hand in Rust. [docs/performance.md](docs/performance.md) records the budgets, the numbers, what limits each workload, and why the interpreter needs no JIT yet.

## License

MIT
