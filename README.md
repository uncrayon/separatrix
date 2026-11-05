# Separatrix

A desktop app for exploring dynamical systems, built for students and professors of mathematics, physics and applied science. Type a system the way it is written on the board, and see its phase portrait move as you drag.

> **Status:** rewrite in progress (milestone 1 done: the live phase plane). The previous Python/web version (v0.1.0, PyPI `equation-phase-portrait-tool`) is deprecated and lives in git history.

## What it will do (v1)

- ODEs from 1D slope fields to n-dimensional systems, and discrete maps (logistic, Hénon).
- Cobweb plots of one-dimensional maps x_{n+1} = f(x_n), with a transient to hide the first iterates.
- Poincaré sections of flows: a plane x_i = c or a stroboscopic section of a forced oscillator, plotted in two states or as the return map x_k → x_{k+1}.
- Phase portraits, nullclines, fixed points with stability, 2D projections of any variables.
- An interactive 3D view of any three states of a flow (Lorenz, Rössler): drag to turn the box, scroll to zoom, with the fixed points in Strogatz's glyphs. Drawn with WebGL (three.js), or on a 2D canvas where WebGL is unavailable.
- Lyapunov exponents, orbit diagrams, and continuation of equilibria (fold, Hopf, flip, Neimark–Sacker).
- Symplectic integration for Hamiltonian systems.
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

## License

MIT
