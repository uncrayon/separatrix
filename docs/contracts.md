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
pub struct SymbolInfo { pub name: String, pub latex: String,
                         pub derived: bool /* introduced by order reduction: x' for x'' */ }
pub struct ParamInfo { pub name: String, pub latex: String, pub default: f64 }

pub fn convert(lines: &[String], from: InputFormat, to: InputFormat) -> Vec<ConvertedLine>;
pub struct ConvertedLine { pub source: String, pub converted: bool, pub error: bool,
                          pub note: Option<String> }
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
(`\sigma`, `\theta`, …), subscripted names (`x_1`, `k_{12}`), `\mathrm{name}` for multi-letter
names (`\operatorname{name}` is for functions: a multi-letter one that is not supported is an
error), `\frac`, `\sqrt`, `\sqrt[n]`, `^`, `\cdot`, `\times`, `\left( \right)`,
`\sin \cos \tan \exp \ln \log \sinh \cosh \tanh \arcsin \arccos \arctan`, `|x|`, `\pi`, `e`.
Text mode: identifiers are whole words (`sigma`, `theta` map to Greek), explicit `*` between names,
implicit multiplication only after numbers or between parenthesised groups, functions as `sin(x)`.
A name spelled as one identifier (`\mathrm{cost}` in LaTeX, the word `cost` in text mode) is that
name, even when it begins with a function name: it compiles (as a parameter, unless it has an
equation) with a warning that suggests the function (`did you mean \cos t?` / `cos(t)`). Letters
typed one by one that spell a function (`cost` in LaTeX) are still an error, since they would read
as a product `c·o·s·t`.
Text mode writes the `n`-th root (`\sqrt[n]{x}`) as `root(x, n)` with a whole number `n >= 2`; for
odd `n` it is real for `x < 0`, unlike `x^(1/n)`.
Reserved: `t` (time, ODE), `n` (iterate, maps), `\pi`, `e`.

#### One format per system; converting between them

A system has one input format, and every line is stored exactly as typed in it: the UI sends
`{ source, format }` as is, so the text parser (and its text-worded messages, `cos(t)`,
`x' = -x`) handles text, and diagnostic spans point into what the user typed.

`convert` rewrites a system's lines (one UI line each; `;` inside a line is kept, written `"; "`;
empty lines pass through, and a line with no equation in it, only spacing such as `\quad`, becomes
empty) from one format to the other. Each line is parsed on its own; the
constructs the formats read differently are rewritten to what they mean (`\frac{dx}{dt}` and `dx/dt`
are `x'`; `x(t)` is the state `x` in a flow that has one, otherwise LaTeX's product `x·t`; `H(…)` on
the left is kept), and the tree is written out node for node, never as a prettier form that is a
different tree (`\exp x` is `exp(x)`, `e^x` stays `e^x`). The guarantee:

- **A converted line means exactly the same and keeps every name.** The core parses its own output
  and requires the same tree (names by name, numbers bit for bit); when the original system
  compiles, the converted one must compile to the same kind, flags, states, parameters, defaults
  and right-hand sides (parameters in any order: LaTeX `\sin^{a} b` names `a` first, text
  `sin(b)^a` names `b` first), or nothing is converted. So the recompile after a switch finds the same
  shape, and sliders, view and initial conditions are kept.
- **A line that can't be converted is kept as typed**, with `converted: false` and a `note` the UI
  shows under it: a line with an error (`error: true`), or a valid line the target can't write
  exactly (`error: false`): a name the target can't write (`a_b_c` in LaTeX), or a number too long
  for LaTeX's digits-only form (`1e300`).
- **A valid line is never left in the old format.** Read in the other format it would mean
  something else or nothing (text `-x/9.109e-31` is `-x/9.109·e - 31` in LaTeX), so when any line
  comes back with `converted` and `error` both false the UI doesn't switch: every line stays as it
  is, in the current format, with the note on the lines that stopped it. A line with an error was
  broken already; the switch goes ahead and keeps it as typed.
- **Names:** text writes the name as is; LaTeX writes its usual form (`x_{1}`, `\omega_{0}`,
  `k_{\mathrm{on}}`) when that reads back as the same name, otherwise `\mathrm{name}`: text `x1` and
  `omega0` become `\mathrm{x1}` and `\mathrm{omega0}`, not `x_{1}` and `\omega_{0}`, which are other
  names.
- **Numbers:** text uses plain decimals, or exponent notation when it is clearly shorter (`1e-7`);
  LaTeX uses plain decimals (`0.0000001`).

Switching straight back with no line changed restores the lines from before the switch exactly
(the UI keeps them), so LaTeX → text → LaTeX gives back the user's own spelling.

The gallery templates are authored in LaTeX; their text form is generated by `convert` into
`app/src/templates/templates.text.generated.json` (template id → lines), which the UI loads in text
mode without a conversion call. `UPDATE_TEMPLATE_TEXT=1 cargo test -p dyn-expr --test convert`
rewrites it, and the test fails while it is out of date.

A template's `view.kind` is absent (a phase plane or a time series), `cobweb`, `space` (with `z` and
`zRange`) or `section`, whose `view.section` names the plane's state
(`{ kind: "plane", state: "y", value, direction }`) or gives a stroboscopic `period` and `phase`;
`transient` skips that many crossings. `app/src/review/templates.test.mjs` checks that every
template has its generated text and a browser-preview system, and opens in the view it names.

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

pub enum Section { Plane { state: usize, value: f64, direction: Direction },
                   Stroboscopic { period: f64, phase: f64 } }
pub fn section(f: &dyn VectorField, p: &[f64], t0: f64, x0: &[f64], t1: f64,
               opts: &SolverOptions, section: &Section, skip: usize, max_points: usize) -> Solution;
```

`section` returns only the points where the orbit meets a Poincaré section: `t` holds the crossing
times and `x` the crossing states, row-major. A `Plane` crossing is a sign change of
`g = x[state] − value` in `direction`, judged in the direction of integration (so backwards in time
when `t1 < t0`), located by Brent's method on the dense output as for events (to within `4ε·max(|t|, 1)` in time,
`ε` the machine epsilon), and its `x[state]` is exactly `value`; starting on the plane is not a
crossing. A `Stroboscopic` sample is the dense output at `t_k = phase + k·period`
for every integer `k` with `t_k` between `t0` and `t1` inclusive, computed with one multiply and one
add and no library function, so the same samples are kept on every OS. The first `skip` crossings
are dropped. The run stops at `skip + max_points` crossings (`Completed`), at `t1`, or for any
reason `integrate` stops, with the same `status` and `stats`; the last computed point is never
appended.

Blow-up: stop with `BlewUp` when any |x_i| > 1e12 (configurable later), and also for blow-ups too
fast to reach that in double precision: Dopri5's step size collapsing while |x| grows (`x' = x^3`),
or a fixed step (Euler/Heun/RK4) overflowing to ±∞ in a growing component (`x' = e^x`) where `f` is
still ±∞ just short of the overflowing point (a probe at `P - 1e-6·(P - x)`). `NonFinite` is kept
for NaN/∞ that is not a blow-up (`sqrt` of a negative number, `-1/x` at 0, or a fixed step or stage
landing exactly on a pole at a finite point, `x' = 1/(2 - x)^3` at 2, which Dopri5 reports as
`StepTooSmall`).

### `dyn-analysis` (portrait analyses)

```rust
pub enum Axis { State(usize), Time }
pub struct Plane { pub x: Axis, pub y: Axis, pub x_range: (f64, f64), pub y_range: (f64, f64),
                   pub base_state: Vec<f64> /* values for hidden variables */, pub t: f64 }

pub struct Arrow { pub x: f64, pub y: f64, pub u: f64, pub v: f64 }      // raw field components
pub fn slope_field(f: &dyn VectorField, p: &[f64], plane: &Plane, grid: usize) -> Vec<Arrow>;

pub struct Nullcline { pub state: usize, pub polylines: Vec<Vec<(f64, f64)>> }
pub fn nullclines(f: &dyn VectorField, p: &[f64], plane: &Plane, grid: usize) -> Vec<Nullcline>;

pub const MAX_GRAPH_SAMPLES: usize = 4096;
pub fn map_graph(f: &dyn VectorField, p: &[f64], x_range: (f64, f64), y_range: (f64, f64),
                 samples: usize) -> Vec<Vec<(f64, f64)>>;   // y = f(x) of a 1-D map (cobweb)

pub struct FixedPoint { pub x: Vec<f64>, pub eigenvalues: Vec<(f64, f64)> /* re, im */,
                        pub eigenvectors: Vec<Option<Vec<f64>>>,    // one per eigenvalue
                        pub eigenvectors_im: Vec<Option<Vec<f64>>>, // complex λ only
                        pub classification: Classification, pub stable: bool }
pub fn fixed_points(f: &dyn VectorField, p: &[f64], search: &[(f64, f64)] /* per state */,
                    seeds_per_axis: usize) -> Vec<FixedPoint>;     // ODE: f = 0; map: f(x) = x
pub fn fixed_points_with_budget(f: &dyn VectorField, p: &[f64], search: &[(f64, f64)],
                                seeds_per_axis: usize, max_seeds: usize) -> Vec<FixedPoint>;
pub fn seed_count(dim: usize, seeds_per_axis: usize, max_seeds: usize) -> usize;
pub fn classify(kind: SystemKind, eigenvalues: &[(f64, f64)]) -> Classification;

pub struct Eigenvectors { pub re: Vec<Option<Vec<f64>>>, pub im: Vec<Option<Vec<f64>>> }
pub fn eigenvectors(kind: SystemKind, n: usize, a: &[f64], eigenvalues: &[(f64, f64)]) -> Eigenvectors;
```

Eigenvectors (of `J` for flows, of `Df` for maps), one entry per eigenvalue in the same order:

- **Real λ**: `eigenvectors[k]` is a unit vector with `J v = λ v`, sign-normalised so that its
  largest-magnitude component is positive (the first one on a tie); `eigenvectorsIm[k]` is `null`.
- **Repeated λ** (same resolution as the classification, `|λ1 - λ2| ≲ 3e-5·max|λ|`; for 2D flows
  `τ² - 4Δ ≈ 0`): as many vectors as the eigenspace has dimensions, the rest `null`. A star
  (`J = λI`) gets an orthonormal basis (any basis is valid); a degenerate node gets its single
  eigenvector and one `null`. Eigenvalues of different stability (flows: sign of `Re λ`; maps: sign
  of `|μ| - 1`, as in the classification) are never treated as repeated, however close, so a
  saddle's stable and unstable vectors are never mixed up (`kind` says which rule applies).
  Distinct eigenvalues that are only nearly repeated (e.g. `diag(-1, -1 - 1e-5)`, drawn as a star)
  still get each the vector of its own eigenvalue.
- **Rounded repeated λ**: a repeated real eigenvalue that rounding turned into a slightly complex
  pair (`|Im λ| ≤ 1.6e-5·max|λ|`, e.g. a degenerate node computed as `-1 ± 1e-6 i`) is treated as
  real: `eigenvectors[k]` is a real vector (or `null` for the missing direction) and
  `eigenvectorsIm[k]` is `null`, although `Im λ ≠ 0`. Consumers should take an entry with a vector
  and a `null` Im part as a real eigenvector of `Re λ`, not re-test `Im λ`.
- **Complex pair α ± iω**: `v = re + i·im` with `J v = λ v`, returned as `eigenvectors[k] = re`
  and `eigenvectorsIm[k] = im`; the conjugate eigenvalue gets `(re, -im)`. The phase is fixed so
  that `|re|² + |im|² = 1`, `re ⊥ im` and `|re| ≥ |im|`: re and im are the major and minor
  semi-axes of the ellipses traced by the linearised orbits `Re(c·e^{λt}·v)` (spirals and
  centres), and span the invariant plane. For a circle the largest component of `v` is made real
  and positive. Sign as for real vectors, by `re`.
- No linearisation (non-finite Jacobian, or a one-dimensional `f` with a kink at the point, such
  as the tent map's peak or `|x|` at 0, found by one-sided difference quotients that disagree at
  two step sizes): `eigenvalues`, `eigenvectors`, `eigenvectorsIm` are all empty and the point
  is `non-hyperbolic`. Never NaN.
- One dimension: neighbouring points with `g = 0` (to `1e-12`, relative) all the way between
  them and at least 1/1000 of the box apart lie on an interval of fixed points (`f(x) = x` for
  the tent map at `r = 1`, `f = 0` for a flow). Each such run is reported as the interval's two
  ends inside the box, found by bisection, as `non-isolated` points.

`map_graph` is the graph of a one-dimensional map's `f` (at `n = 0`; empty for any other
system), sampled at `samples` (clamped to 2..=4096) evenly spaced `x` with both ends included.
Within a polyline `x` strictly increases and stays in `x_range`; a new one starts at every gap:

- a non-finite value: the edge of the domain is found by 12 bisections, so `√x` starts within
  `cell/4096` of 0;
- a pole or a jump: an interval is examined when its rise exceeds the threshold
  `max(JUMP_FACTOR · median rise, Δy/50)`; when its rise exceeds `JUMP_FACTOR · median rise` (or
  its step is against both neighbours' and larger than theirs) and `f` changes sign across it or
  its midpoint is larger in magnitude than both ends (a pole in a zoomed-out view, where the
  samples either side are small); or when it is next to a steep one and its midpoint stands out
  above both ends by the threshold (an even pole midway between two samples). It is bisected up
  to 12 levels, the more suspicious half first (the larger rise or, when the midpoint is larger
  in magnitude than both ends, the half towards the larger end, which follows `1/x²`), then the
  other. A half is continuous once its rise is below the threshold and has shrunk on two
  successive halvings (one drop is not enough: a midpoint exactly on a jump, such as
  `sign(0) = 0`, halves the rise once). A half still rising after 12 levels by more than 1/64 of
  the interval's rise splits there (a continuous curve's shrinks about 4096-fold; a jump's stays
  and a pole's grows), as does a non-finite midpoint; the refined points are kept on each side
  so the curve reaches towards the pole. At most 48 evaluations go to one interval and
  `12 · samples` to the whole graph.

`y` is then clipped to the band `[y0 − Δy, y1 + Δy]` (`Δy = y1 − y0`), with the curve's ends on
the band interpolated linearly. `[]` means `f` is undefined or outside the band everywhere in
view. Cost: `samples` evaluations plus at most 12 per steep interval or domain edge (24 more
for a non-finite midpoint).

For a one-dimensional map the only multiplier is `λ = f′(x*)`, from the exact Jacobian:
`fixedPoints[i].eigenvalues[0][0]`, classified by `|λ|` against 1 (Strogatz §10.1).

Classification follows Strogatz for 2D flows (saddle, stable/unstable node, stable/unstable spiral,
centre, star, degenerate node, non-isolated) and uses sink/source/saddle (with unstable dimension)
for higher dimensions; maps classify by |μ| relative to 1.

Newton seeds: the grid of `seeds_per_axis^dim` cell centres of `search` when it has at most
`max_seeds` points, otherwise the first `max_seeds` points of a Halton sequence in the box, so
`max_seeds` (clamped to `MAX_SEEDS = 4096`; `fixed_points` uses `MAX_SEEDS`) is a hard cap in every
dimension; `seed_count` says how many are used. The app's budget is `MAX_TOTAL_SEEDS = 1000`
(40, 14², 9³ grid points in 1D–3D; 2^10 = 1024 does not fit, so 10D and up use Halton points).

## IPC (Tauri commands)

JSON, camelCase. `app/src-tauri/src/ipc.rs` is the source of the TypeScript types: ts-rs renders
every IPC type, and the maths crates' enums it reuses (`SystemKind`, `InputFormat`, `Severity`,
`Method`, `Status`, `Stats`, `Direction`, `Classification`, derived behind each crate's `ts` feature), into
`app/src/api/ipc.generated.ts` (`UPDATE_IPC_BINDINGS=1 cargo test -p separatrix
typescript_bindings`). `app/src/api/types.ts` only re-exports the generated types, so the UI
compiles against what Rust sends, and CI fails while the generated file is stale. The same file
carries the constants the UI shares with the core: `MAX_SECTION_POINTS`, the `SECTION_MISFIT` and
`SPACE_MISFIT` warnings, `SPACE_LATEX_REFUSAL` and the 3D view's `DEFAULT_CAMERA`.

`CompileResponse.states[]` is `StateInfo { name, latex, derived }`; `derived` marks the states
added by reducing a higher-order equation to first order (`\theta'` for `\ddot{\theta} = …`), never a
Hamiltonian momentum or a first-order state, and the UI uses it (not the display LaTeX) to decide
when to show the first-order form as `\frac{d\theta}{dt} = \dot{\theta}`.

| Command | Request | Response |
| --- | --- | --- |
| `compile_system` | `{ source, format: "latex" \| "text" }` | `CompileResponse` |
| `convert_source` | `{ lines: string[], from, to }` (`ConvertRequest`) | `{ lines: { source, converted, error, note? }[] }` (`ConvertResponse`) |
| `compute_portrait` | `PortraitRequest` | `PortraitResponse` |

`compile_system`'s `source` is the system's non-empty lines, as typed, joined with `"\n"`, in its
one `format`. `convert_source` is `dyn_expr::convert` (above): one response line per request line,
in order.

### Cobweb plots and transients

`PortraitRequest` has two optional keys, `PortraitResponse` one (absent keys keep the earlier
meaning, so older requests are unchanged):

| Key | Type | Meaning |
| --- | --- | --- |
| `graphSamples` (request) | `number?` | Sample the graph `y = f(x)` across `xRange` (`map_graph`, clamped to 2–4096). |
| `transient` (request) | `number?` | Map iterates computed but not returned before each orbit's `n`. Absent = 0; clamped so that `transient + n ≤ 1,000,000`. For a flow it counts skipped section crossings (see Poincaré sections below) and is ignored without a `section`. |
| `graph` (response) | `[x, f(x)][][]?` | Present exactly when `graphSamples` was sent (possibly `[]`). |

A **cobweb request** is `xAxis = yAxis = { kind: "state", index: 0 }`, `graphSamples` set,
`fieldGrid: 0`, `nullclines: false`. Then the fixed-point search box is the visible `x` range
and a fixed point and a start point land on the diagonal at `(x*, x*)` and `(x₀, x₀)`.
`graphSamples` is refused (the command fails with the message) unless the system is a
one-dimensional autonomous map with `xAxis` state 0:

- "The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system has 2 states."
- "The cobweb view needs a one-dimensional map x_{n+1} = f(x_n); this system is a flow."
- "This map changes with n, so f has no single graph to cobweb."
- "The cobweb view needs x_n on the horizontal axis."

A map's trajectory with transient `T` and `t1 = N` returns `t = [T, T+1, …, T+N]` and
`x = [x_T … x_{T+N}]` (empty when it blows up during the transient), statuses as before. The
staircase is drawn from it by the UI (`app/src/canvas/cobweb.ts`) and by the LaTeX export
(`separatrix_export::cobweb`) with one definition: vertices `(v₀, v₀)`, then
`(v_{k−1}, v_k), (v_k, v_k)` for `k = 1…m`, cut at the first non-finite value; the diagonal is
`(a, a)–(b, b)` with `a = max(x0, y0)`, `b = min(x1, y1)`, none when `a ≥ b`. The shared test
vectors are `crates/export/tests/vectors/cobweb.json`.

### Poincaré sections

`PortraitRequest.section` (`SectionSpec?`) asks for a Poincaré section of every flow trajectory:

| `kind` | Fields | Meaning |
| --- | --- | --- |
| `"plane"` | `state`, `value`, `direction: "rising" \| "falling" \| "both"` | crossings of `x[state] = value` |
| `"stroboscopic"` | `period`, `phase` | the state at `t_k = phase + k·period`, for every `t_k` from the trajectory's `t0` to its `t1` |

The `phase` is absolute (`t ≡ phase mod period`) both over IPC and in `dyn_solve::section`, so
orbits started at different times are sampled at the same forcing phase. The studio starts a click on a stroboscopic section at
`t0 = phase`, so the start point is itself a point of the section.

With `section` present, each `PortraitResponse.trajectories[i]` is that orbit's crossings
(`dyn_solve::section` above) instead of its samples, in the same `TrajectoryOut` shape, and
`samples` is not used. `transient` then counts the crossings skipped before the first returned one
(the iterates of the Poincaré map, as for maps), and at most `MAX_SECTION_POINTS` (100,000)
crossings follow it. The other layers (field, nullclines, fixed points) are computed as usual.
`section` is refused with one of these messages:

- "A Poincaré section needs a flow; this system is a map."
- "The section's state is out of range: this system has 2 states."
- "The section's value must be a finite number."
- "The stroboscopic period must be a positive number."
- "The stroboscopic phase must be a finite number."

### The 3D view

`PortraitRequest.zAxis` (`AxisRef?`) and `zRange` (`[lo, hi]?`) are the third axis of a 3D view
and its range. Both are absent for every other view. The fixed-point search box treats the third
axis as visible, as `xAxis` and `yAxis` are: that state is searched over `zRange` instead of
around its slice value (`fixed_point_search_box`), so the Lorenz points
`C± = (±√(b(r−1)), ±√(b(r−1)), r−1)` are found with `z` on the third axis over `[0, 50]` whatever
the slice. The studio's 3D request also has `fieldGrid: 0` and `nullclines: false`; trajectories
are whole orbits, forward and (with the layer) backward, as on the phase plane. The camera is
not part of the request: turning the box redraws without asking the core. The pair is refused
with one of these messages:

- "The third axis shows state 4 but the system only has 3."
- "The third axis of a 3D view must be a state."
- "The third axis range [1, 1] is not a valid interval."
- "A 3D view needs both a third axis and its range."

`PortraitResponse.fixedPoints[]` is `FixedPointOut { x, eigenvalues: [re, im][],
eigenvectors: (number[] | null)[], eigenvectorsIm: (number[] | null)[], classification, stable }`,
with the eigenvector conventions of `dyn-analysis` above.

The UI selects a fixed point by position, not by its index in `fixedPoints` (which is re-sorted
on every recompute): after each response the selection moves to the nearest fixed point within 10%
of the larger visible state-axis span, provided that point is not nearer to another of the
previous response's fixed points (so a point that merges into a neighbour does not hand the
selection to it), and is cleared otherwise.

The UI never sends anything stale: it sends a monotonically increasing `requestId`
and drops responses older than the latest it has drawn.
