//! Exports of a Separatrix portrait: LaTeX (the system, its fixed points and
//! a pgfplots figure), CSV with metadata, and the PNG point layer of large
//! figures.
//!
//! The crate has no Tauri dependency: the app converts its IPC types into
//! [`model`] and writes the returned text and files (see `docs/exports.md`).
//!
//! | Export | Function |
//! | --- | --- |
//! | LaTeX system (`align`) | [`latex_system::system`] |
//! | LaTeX fixed points (booktabs) | [`latex_fixed_points::table`] |
//! | LaTeX figure (pgfplots) | [`latex_figure::figure`] |
//! | LaTeX Lyapunov spectrum (booktabs, siunitx) | [`latex_lyapunov::lyapunov_latex`] |
//! | LaTeX orbit diagram (pgfplots) | [`latex_figure::orbit_figure`] |
//! | LaTeX branch plot (pgfplots) | [`latex_continuation::continuation_figure`] |
//! | LaTeX special points (booktabs, siunitx) | [`latex_continuation::special_points_table`] |
//! | CSV and `.meta.json` | [`csv::files`], [`csv::lyapunov`], [`csv::orbit`], [`csv::continuation`] |
//! | Curve simplification for exports | [`simplify::simplify_portrait`] |
//! | Clipboard PNG decoding | [`raster::decode_rgba`] |
//! | Cobweb geometry (staircase, diagonal) | [`cobweb`] |
//!
//! Numbers are written as the UI writes them: [`num::shortest`] is
//! JavaScript's `String(v)` (CSV, tables), and tick labels are exact ports
//! of the screen's ([`ticks`]).

#![warn(missing_docs)]

pub mod clip;
pub mod cobweb;
pub mod csv;
mod geom;
pub mod heads;
pub mod latex_continuation;
pub mod latex_figure;
pub mod latex_fixed_points;
pub mod latex_lyapunov;
pub mod latex_system;
pub mod latex_util;
pub mod model;
pub mod num;
pub mod raster;
pub mod simplify;
pub mod ticks;
pub mod words;

pub use latex_continuation::{continuation_figure, special_points_table, ContinuationFigureInput};
pub use latex_figure::{
    figure, orbit_figure, FigureFile, FigureInput, FigureOutput, OrbitFigureInput, Placement,
};
pub use latex_fixed_points::FixedPointsInput;
pub use latex_lyapunov::lyapunov_latex;
pub use latex_system::{system, Notation, SystemInput, SystemLatex};
pub use model::*;
