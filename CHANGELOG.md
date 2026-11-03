# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]
### Changed
- Rewrite as a cross-platform desktop app (Tauri 2 + Rust core). No Python, no server.
- New Cargo workspace: `dyn-core` (shared `VectorField` contract), `dyn-expr`, `dyn-solve`, `dyn-analysis`, and the Tauri shell in `app/`.
- New CI: Rust tests on Linux, macOS and Windows; release workflow that builds unsigned installers.

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