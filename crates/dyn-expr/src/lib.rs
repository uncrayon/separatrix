//! Parse, understand and evaluate the user's equations.
//!
//! [`compile`] turns source text (LaTeX from the MathLive editor, or plain
//! text) into a [`CompiledSystem`], which implements
//! [`dyn_core::VectorField`] with exact Jacobians, plus a [`SystemInfo`]
//! describing what was understood and a list of [`Diagnostic`]s.
//!
//! ```
//! use dyn_core::VectorField;
//! use dyn_expr::{compile, InputFormat};
//!
//! let src = r"\dot{x}=\sigma\left(y-x\right)\\\dot{y}=x\left(\rho-z\right)-y\\\dot{z}=xy-\beta z
//! \sigma=10\\\rho=28\\\beta=\frac{8}{3}";
//! let out = compile(src, InputFormat::Latex);
//! let sys = out.system.expect("compiles");
//! assert_eq!(sys.dim(), 3);
//! let names: Vec<_> = sys.info.params.iter().map(|p| p.name.as_str()).collect();
//! assert_eq!(names, ["sigma", "rho", "beta"]);
//! let f = sys.eval_alloc(0.0, &[1.0, 2.0, 3.0], &sys.default_params());
//! assert_eq!(f[0], 10.0);
//! ```
//!
//! # Pipeline
//!
//! 1. `token`: tokenizer (LaTeX or text mode), byte spans kept.
//! 2. `parser`: recursive descent to a syntax tree, one tree per line.
//! 3. `sema`: classify lines (flow / map / Hamiltonian / parameter value),
//!    find states and parameters, reduce higher-order equations to first
//!    order, produce plain-language diagnostics and display LaTeX.
//! 4. `program`: compile to a flat register program with shared common
//!    subexpressions and folded constants.
//! 5. [`ad`]: evaluation is generic over [`ad::Scalar`]; `f64` gives
//!    values, [`ad::Dual`] gives exact derivatives.

#![warn(missing_docs)]

pub mod ad;
mod ast;
mod diag;
mod latex;
mod parser;
mod program;
mod sema;
mod system;
mod token;

use serde::{Deserialize, Serialize};

pub use diag::{Diagnostic, Severity};
pub use system::CompiledSystem;

/// Which syntax the source uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputFormat {
    /// LaTeX as produced by MathLive: single-letter names, `\sigma`,
    /// `\frac{a}{b}`, implicit multiplication (`2xy`).
    Latex,
    /// Plain text: whole-word names (`sigma`), explicit `*`, `sin(x)`.
    Text,
}

/// Result of [`compile`].
#[derive(Debug)]
pub struct CompileOutput {
    /// The compiled system; `None` when there are errors.
    pub system: Option<CompiledSystem>,
    /// Errors, and warnings even on success, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
}

/// What the compiler understood about the system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// Flow (ODE) or map.
    pub kind: dyn_core::SystemKind,
    /// Entered as `H(q.., p..) = ...`.
    pub hamiltonian: bool,
    /// False if `t` (flows) or `n` (maps) appears on a right-hand side.
    pub autonomous: bool,
    /// State variables in equation order; for `x'' = …` the derived state
    /// `x'` (shown as `\dot{x}`) comes right after `x`. Hamiltonian
    /// systems list coordinates then momenta.
    pub states: Vec<SymbolInfo>,
    /// Parameters in order of first appearance on the right-hand sides.
    pub params: Vec<ParamInfo>,
    /// The first-order system for display, one per state
    /// (`\dot{x} = \sigma \left(y - x\right)`).
    pub equations_latex: Vec<String>,
}

/// A state variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Identity, e.g. `"x"`, `"theta"`, `"x_1"`, `"x'"`.
    pub name: String,
    /// Display form, e.g. `"x"`, `"\theta"`, `"x_{1}"`, `"\dot{x}"`.
    pub latex: String,
}

/// A parameter and its default value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamInfo {
    /// Identity, e.g. `"sigma"`.
    pub name: String,
    /// Display form, e.g. `"\sigma"`.
    pub latex: String,
    /// 1.0 unless a line such as `\sigma = 10` sets it.
    pub default: f64,
}

/// Compile `source` into a vector field.
///
/// Equations are separated by newlines, `;` or `\\`. Problems never
/// panic: they are reported in [`CompileOutput::diagnostics`] with byte
/// spans into `source`, and `system` is `None` if any is an error.
pub fn compile(source: &str, format: InputFormat) -> CompileOutput {
    let mut diags = Vec::new();
    let lines = parser::parse_lines(source, format, &mut diags);
    let analysis = if lines.is_empty() {
        if diags.is_empty() {
            let ex = match format {
                InputFormat::Latex => "\\dot{x} = -x",
                InputFormat::Text => "x' = -x",
            };
            diags.push(Diagnostic::error(
                format!("Enter at least one equation, for example {ex}"),
                None,
            ));
        }
        None
    } else {
        sema::analyze(lines, format, &mut diags)
    };
    let has_errors = diags.iter().any(Diagnostic::is_error);
    let system = match analysis {
        Some(a) if !has_errors => Some(CompiledSystem::new(a)),
        _ => None,
    };
    diags.sort_by_key(|d| d.span.map_or(usize::MAX, |s| s.0));
    CompileOutput { system, diagnostics: diags }
}
