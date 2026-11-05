# Separatrix

A desktop app for exploring dynamical systems, built for students and professors of mathematics, physics and applied science. Type a system the way it is written on the board, and see its phase portrait move as you drag.

> **Status:** rewrite in progress. Milestones 1 and 2 are done: the live phase plane; exports, project files and cobweb plots; Poincaré sections, the 3D view, and a template gallery whose chapter 2–8 examples reproduce Strogatz's results. The previous Python/web version (v0.1.0, PyPI `equation-phase-portrait-tool`) is deprecated and lives in git history.

## What it will do (v1)

- ODEs from 1D slope fields to n-dimensional systems, and discrete maps (logistic, Hénon).
- Cobweb plots of one-dimensional maps x_{n+1} = f(x_n), with a transient to hide the first iterates.
- Poincaré sections of flows: a plane x_i = c or a stroboscopic section of a forced oscillator, plotted in two states or as the return map x_k → x_{k+1}.
- Phase portraits, nullclines, fixed points with stability, 2D projections of any variables.
- An interactive 3D view of any three states of a flow (Lorenz, Rössler): drag to turn the box, scroll to zoom, with the fixed points in Strogatz's glyphs. Drawn with WebGL (three.js), or on a 2D canvas where WebGL is unavailable.
- Lyapunov exponents, orbit diagrams, and continuation of equilibria (fold, Hopf, flip, Neimark–Sacker).
- Symplectic integration for Hamiltonian systems.
- A gallery of 31 examples from Strogatz's *Nonlinear Dynamics and Chaos* (2nd edition), grouped by chapter, each citing its section. Every chapter 2–8 example reproduces a result of the book, checked by the Strogatz gate below.
- Export to LaTeX, PNG, SVG and CSV. Shareable project files.
- Windows, macOS and Linux. Works offline.

## Layout

| Path | What |
| --- | --- |
| `crates/dyn-core` | The `VectorField` trait every solver and analysis uses |
| `crates/dyn-expr` | Equation parsing, inference (states, parameters, ODE or map), evaluation and exact Jacobians |
| `crates/dyn-solve` | Integrators (Euler, RK4, Dormand–Prince, …), map iteration, events, Poincaré sections |
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

prints one row per claim the chapter 2–8 templates make (fixed points and their classification, bifurcation values, periods, a limit cycle's radius, a locked frequency, a Poincaré map's fixed point, energy drift), with the book's value, the measured one and the tolerance, and exits non-zero if any row fails. Expected values come from the book's formulas; the one exception is the relaxation oscillator's period at μ = 10, which the book gives only as the asymptote μ(3 − 2 ln 2) (printed as an info row), so the gate checks the known numerical value 19.0784 and that two solvers agree on it to 1e−5. `app/src-tauri/tests/strogatz.rs` holds the claims; a template that cites a chapter 2–8 section can't be added without one.

## License

MIT
