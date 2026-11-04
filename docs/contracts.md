# Milestone 1 contracts

These are the interfaces between the crates and between Rust and the UI. Each
crate is built against them independently; change them here first.

## Rust crate APIs

All crates depend only on `dyn-core` (the `VectorField` trait) except where
noted. Jacobians are row-major. Time-like axis for maps is the iterate `n`.

### `dyn-expr` (parse, infer, evaluate)

```rust
pub enum InputFormat { Latex, Text }

pub fn compile(source: &str, format: InputFormat) -> CompileOutput;

pub struct CompileOutput {
    pub system: Option<CompiledSystem>,   // None when there are errors
    pub diagnostics: Vec<Diagnostic>,     // warnings even on success
}

pub struct Diagnostic {
    pub severity: Severity,               // Error | Warning | Info
    pub message: String,                  // plain language, actionable
    pub span: Option<(usize, usize)>,     // byte offsets into `source`
}

pub struct CompiledSystem { pub info: SystemInfo, /* program */ }
impl dyn_core::VectorField for CompiledSystem { /* exact AD Jacobians */ }

pub struct SystemInfo {
    pub kind: dyn_core::SystemKind,       // Ode | Map
    pub hamiltonian: bool,                // entered as H(q.., p..) = ...
    pub autonomous: bool,
    pub states: Vec<SymbolInfo>,          // in equation order; derived states for x'' appear after x
    pub params: Vec<ParamInfo>,           // in order of first appearance
    pub equations_latex: Vec<String>,     // first-order system, display form (\dot{x} = ...)
}
pub struct SymbolInfo { pub name: String, pub latex: String }
pub struct ParamInfo { pub name: String, pub latex: String, pub default: f64 }
```

Accepted input (one equation per line; `;` and `\\` also separate):

| Form | Examples |
| --- | --- |
| ODE, first order | `\dot{x} = ...`, `x' = ...`, `\frac{dx}{dt} = ...` |
| ODE, higher order | `\ddot{x} = ...`, `x'' = ...` (rewritten to first order; new state shown as `\dot{x}`) |
| Map | `x_{n+1} = r x_n (1 - x_n)` (bare `x` on the right means `x_n`) |
| Hamiltonian | `H(q, p) = \frac{p^2}{2} - \cos q` (first half of the argument list are coordinates, second half momenta) |
| Parameter default | `\sigma = 10` (a plain symbol equal to a constant expression) |

LaTeX mode: single-letter symbols with implicit multiplication (`2xy` = 2·x·y), Greek letters
(`\sigma`, `\theta`, …), subscripted names (`x_1`, `k_{12}`), `\operatorname{name}` / `\mathrm{name}`
for multi-letter names, `\frac`, `\sqrt`, `\sqrt[n]`, `^`, `\cdot`, `\times`, `\left( \right)`,
`\sin \cos \tan \exp \ln \log \sinh \cosh \tanh \arcsin \arccos \arctan`, `|x|`, `\pi`, `e`.
Text mode: identifiers are whole words (`sigma`, `theta` map to Greek), explicit `*` between names,
implicit multiplication only after numbers or between parenthesised groups, functions as `sin(x)`.
Reserved: `t` (time, ODE), `n` (iterate, maps), `\pi`, `e`.

### `dyn-solve` (integrate and iterate)

```rust
pub enum Method { Euler, Heun, Rk4, Dopri5 }
pub struct SolverOptions { pub method: Method, pub rtol: f64, pub atol: f64,
                           pub h: f64 /* fixed step, or initial step for adaptive (0 = auto) */,
                           pub h_max: f64, pub max_steps: usize }
pub enum Sampling { Steps, Uniform(usize) }   // raw accepted steps, or N dense-output samples
pub enum Status { Completed, BlewUp, MaxSteps, StepTooSmall, NonFinite }
pub struct Stats { pub accepted: usize, pub rejected: usize, pub evals: usize }
pub struct Solution { pub t: Vec<f64>, pub x: Vec<f64> /* row-major len = t.len()*dim */,
                      pub status: Status, pub stats: Stats }

pub fn integrate(f: &dyn VectorField, p: &[f64], t0: f64, x0: &[f64], t1: f64,
                 opts: &SolverOptions, sampling: Sampling) -> Solution;   // t1 < t0 integrates backwards
pub fn iterate_map(f: &dyn VectorField, p: &[f64], x0: &[f64], n: usize, transient: usize) -> Solution;

pub struct Event { pub g: Box<dyn Fn(f64, &[f64]) -> f64 + Send + Sync>, pub direction: Direction }
pub enum Direction { Rising, Falling, Both }
pub struct EventHit { pub event: usize, pub t: f64, pub x: Vec<f64>, pub rising: bool }
pub fn integrate_with_events(..., events: &[Event]) -> (Solution, Vec<EventHit>);
```

Blow-up: stop with `BlewUp` when any |x_i| > 1e12 (configurable later).

### `dyn-analysis` (portrait analyses)

```rust
pub enum Axis { State(usize), Time }
pub struct Plane { pub x: Axis, pub y: Axis, pub x_range: (f64, f64), pub y_range: (f64, f64),
                   pub base_state: Vec<f64> /* values for hidden variables */, pub t: f64 }

pub struct Arrow { pub x: f64, pub y: f64, pub u: f64, pub v: f64 }      // raw field components
pub fn slope_field(f: &dyn VectorField, p: &[f64], plane: &Plane, grid: usize) -> Vec<Arrow>;

pub struct Nullcline { pub state: usize, pub polylines: Vec<Vec<(f64, f64)>> }
pub fn nullclines(f: &dyn VectorField, p: &[f64], plane: &Plane, grid: usize) -> Vec<Nullcline>;

pub struct FixedPoint { pub x: Vec<f64>, pub eigenvalues: Vec<(f64, f64)> /* re, im */,
                        pub classification: Classification, pub stable: bool }
pub fn fixed_points(f: &dyn VectorField, p: &[f64], search: &[(f64, f64)] /* per state */,
                    seeds_per_axis: usize) -> Vec<FixedPoint>;     // ODE: f = 0; map: f(x) = x
pub fn classify(kind: SystemKind, eigenvalues: &[(f64, f64)]) -> Classification;
```

Classification follows Strogatz for 2D flows (saddle, stable/unstable node, stable/unstable spiral,
centre, star, degenerate node, non-isolated) and uses sink/source/saddle (with unstable dimension)
for higher dimensions; maps classify by |μ| relative to 1.

## IPC (Tauri commands)

JSON, camelCase. Types mirrored in `app/src/api/types.ts`.

| Command | Request | Response |
| --- | --- | --- |
| `compile_system` | `{ source, format: "latex" \| "text" }` | `CompileResponse` |
| `compute_portrait` | `PortraitRequest` | `PortraitResponse` |

The UI never sends anything stale: it sends a monotonically increasing `requestId`
and drops responses older than the latest it has drawn.
