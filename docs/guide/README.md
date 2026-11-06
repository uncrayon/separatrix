# Separatrix user guide

Separatrix draws phase portraits, cobwebs, Poincaré sections, Lyapunov spectra, orbit diagrams and
bifurcation branches of the systems you type. This guide has one page per task or view. To install
the app first, follow the guide for [Windows](../install/windows.md), [macOS](../install/macos.md)
or [Linux](../install/linux.md).

## Start here

- [Getting started](getting-started.md). Open an example, add a trajectory, drag a parameter,
  save and export.
- [Enter equations](equations.md). Flows, maps, higher-order and Hamiltonian systems, in LaTeX or
  plain text.

## The views

The **View** control in the right-hand panel switches between views. A flow offers Phase, Series,
Section, 3D, λ and Branch. A map offers Phase, Series, Cobweb, λ, Orbit and Branch. A view that
doesn't fit the system is greyed out, and the panel says why.

| View | Control | Page | Example project |
| --- | --- | --- | --- |
| Phase plane | Phase | [phase-plane.md](phase-plane.md) | [phase-plane.sepx](examples/phase-plane.sepx) |
| Time series | Series | [time-series.md](time-series.md) | [time-series.sepx](examples/time-series.sepx) |
| Cobweb | Cobweb | [cobweb.md](cobweb.md) | [cobweb.sepx](examples/cobweb.sepx) |
| 3D view | 3D | [3d-view.md](3d-view.md) | [3d-view.sepx](examples/3d-view.sepx) |
| Poincaré section | Section | [poincare-section.md](poincare-section.md) | [poincare-section.sepx](examples/poincare-section.sepx) |
| Lyapunov spectrum | λ | [lyapunov.md](lyapunov.md) | [lyapunov.sepx](examples/lyapunov.sepx) |
| Orbit diagram | Orbit | [orbit-diagram.md](orbit-diagram.md) | [orbit-diagram.sepx](examples/orbit-diagram.sepx) |
| Continuation | Branch | [continuation.md](continuation.md) | [continuation.sepx](examples/continuation.sepx) |

Each example project opens in its view. Download it and open it with **Open…**, or drop it on the
Separatrix window.

## Share and present

- [Export figures and data](exports.md). LaTeX, SVG, PNG and CSV.
- [Project files](project-files.md). Save, open, recent files and drag-and-drop.
- [Projector mode](projector-mode.md). A light theme with larger type for the lecture hall.
- [Keyboard shortcuts](keyboard-shortcuts.md). Every key the app answers to.

## Where the numbers come from

Separatrix checks its numbers against published values on every change. Three scripts in the
repository print one row per claim: `scripts/strogatz-gate.sh` for the chapter 2 to 8 examples of
Strogatz's *Nonlinear Dynamics and Chaos* (2nd edition), `scripts/chaos-gate.sh` for Lyapunov
exponents and period doublings, and `scripts/continuation-gate.sh` for bifurcation points. The
numbers quoted in this guide are copied from their output. [CONTRIBUTING.md](../../CONTRIBUTING.md)
says how to run them.

The screenshots come from the real app. `bash scripts/docs-screenshots.sh` opens each example
project and saves `docs/media/<view>.png`.
