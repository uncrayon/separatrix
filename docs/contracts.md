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
pub enum Method { Euler, Heun, Rk4, Dopri5, Verlet, Yoshida4 }   // serde: "verlet", "yoshida4"
impl Method { pub fn is_adaptive(self) -> bool; pub fn is_symplectic(self) -> bool }
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

- **Real λ**: `eigenvectors[k]` is a unit vector with `J v = λ v`, sign-normalized so that its
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
  centers), and span the invariant plane. For a circle the largest component of `v` is made real
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
center, star, degenerate node, non-isolated) and uses sink/source/saddle (with unstable dimension)
for higher dimensions; maps classify by |μ| relative to 1.

Newton seeds: the grid of `seeds_per_axis^dim` cell centers of `search` when it has at most
`max_seeds` points, otherwise the first `max_seeds` points of a Halton sequence in the box, so
`max_seeds` (clamped to `MAX_SEEDS = 4096`; `fixed_points` uses `MAX_SEEDS`) is a hard cap in every
dimension; `seed_count` says how many are used. The app's budget is `MAX_TOTAL_SEEDS = 1000`
(40, 14², 9³ grid points in 1D–3D; 2^10 = 1024 does not fit, so 10D and up use Halton points).

### Symplectic methods

```rust
impl CompiledSystem { pub fn is_separable(&self) -> bool }   // dyn-expr
```

`Method::Verlet` (Störmer–Verlet, kick–drift–kick, order 2) and `Method::Yoshida4` (Yoshida's
composition of three Verlet steps of sizes `w₁h, w₀h, w₁h`, order 4) integrate a Hamiltonian
`H(q, p) = T(p) + V(q)` with the state laid out `[q; p]`, `dof = dim / 2`, as `dyn-expr` builds it.
A kick `p += c·h·ṗ` reads the `ṗ` half of `f(q, ·)`; a drift `q += d·h·q̇` reads the `q̇` half of
`f(·, p)`. `w₁ = 1/(2 − 2^{1/3})` and `w₀ = 1 − 2w₁` are decimal literals, never `cbrt` at run
time, so every OS steps with the same bits. Both are fixed-step (`h = 0` means `|t1 − t0| / 1000`)
and use the stepper's monotone cubic Hermite dense output, so events and Poincaré sections work.
A step costs 3 evaluations (Verlet) or 7 (Yoshida 4), counting `f` at the new point. Both are
time-symmetric: from the pendulum at `(1, 0)`, 100 time units forward and back return within
1e−14. Halving `h` divides the error by 4.00 (Verlet) and 16.07 (Yoshida 4) on the harmonic
oscillator.

The `Stepper` trusts that `H` separates. `is_separable` is the check, and the service makes it.
It is structural. Below the sums, differences and negations at the top of `H`, and its products
with or quotients by factors that read no state, every term must read only coordinates or only
momenta, and `H` must not read `t`. So `p²/2 − cos q`, `p²/(2m) + q⁴` and `(p² + q²)/2` are
separable, and `q p`, `p² q`, `(p² + q²)²` and any `H` with `t` are not.

The service refuses, with a message:

- a symplectic method for a flow given by its equations of motion: "Störmer–Verlet needs a
  Hamiltonian H(q, p); this system is given by its equations of motion." (and "Yoshida 4 …");
- a symplectic method for a Hamiltonian that is not separable: "H(q, p) must split as T(p) + V(q)
  for a symplectic method.";
- a Lyapunov run of a flow with a symplectic method, since the variational equations are not
  separable.

A map ignores the method, so a map is never refused for it.

`TrajectoryOut.energy?: { h0, maxAbsDrift }` (`EnergyOut`) comes with every trajectory of a
Hamiltonian flow, whatever the method: `h0 = H(t0, x0)` and `maxAbsDrift` the largest `|H − h0|`
over the returned samples (the crossings, in a section view). It is absent for other systems and
when `H` is not finite at a sample. `CompileResponse.separable` tells the UI which systems the
symplectic methods can run. The UI's method table (`app/src/state/methods.ts`) disables both
methods otherwise and says why, and the Solver panel reads out `|ΔH| max = 3.1 × 10⁻⁷`.

The chaos gate steps the pendulum `H = p²/2 − cos q` from `(1, 0)` (period
`2π / AGM(1, cos ½) ≈ 6.70`) over 10⁵ periods and reads `H` after every step. Störmer–Verlet at
h = 0.05 keeps `max |ΔH|` at 2.65e−4 (claim ≤ 1e−3), Yoshida 4 at h = 0.1 at 3.87e−6 (claim
≤ 1e−4). For both, the largest `|ΔH|` over the last 10³ periods is 1.000 times the largest over the
first 10³ (claim ≤ 2). RK4 at h = 0.1 gives 97.6 by the same rule, and the gate requires that
claim to fail.

### Stiff systems

```rust
impl Stepper<'_> { pub fn looks_stiff(&self) -> bool }   // dyn-solve
pub struct Solution { /* … */ pub stiff: bool }
```

`Method::Rosenbrock23` (serde `"rosenbrock23"`) is the L-stable linearly implicit Rosenbrock 2(3)
of MATLAB's `ode23s` (Shampine & Reichelt 1997), with `d = 1/(2 + √2)` and `e₃₂ = 6 + √2` written
as decimal literals. Each step solves three systems with `W = I − h·d·J`, factored once per try by
a dense LU with partial pivoting (`crates/dyn-solve/src/lu.rs`; dyn-solve has no linear-algebra
dependency). `J` is `VectorField::jacobian`, exact forward-mode AD for a compiled system, taken
once per accepted step; a rejected try only refactors `W`. `∂f/∂t` is a central difference with
`δ = ε^{1/3}·max(1, |t|)`, taken only when the field reads `t` (2 more evaluations per step).
A step costs 2 evaluations and one Jacobian. The new point is order 2 and the error estimate
`h/6 (k1 − 2k2 + k3)` order 3, so the controller is an I-controller with exponent 1/3 and the
starting step is Hairer's guess for order 3. It is adaptive like Dopri5 (`rtol`, `atol`, `h` as
the first step, `h_max`). Its continuous extension `x + θ(x₁ − x) + θ(1 − θ)·h(k1 − k2)/(1 − 2d)`
feeds `Stepper::interpolate`, so uniform sampling, events and Poincaré sections work. A singular
`W` is rejected like a non-finite step: the step shrinks by 5, and a collapse reports
`nonFinite`.

`crates/dyn-solve/tests/stiff.rs` checks it. Halving a fixed `h` on `x' = −x` divides the error by
4.02. On `y' = −1e6 (y − cos t)` at a fixed `h = 0.1`, an error of 1 off `cos t` is gone after one
step (`R(∞) = 0`). What stays is the scheme's stiff-limit error `h²·(1/(8d) − 1/2)·y''`, at most
7.35e−4 at h = 0.1 and 7.4e−6 at h = 0.01. Robertson's reaction reaches t = 40 at rtol 1e−8, atol
1e−14 in 6,737 steps, within 1.6e−7 relative of the reference below. A plane section of the
relaxation oscillator at μ = 1000 returns its crossings of `x = 0` half a period
`(3 − 2 ln 2)μ/2` apart.

`Stepper::looks_stiff` is Hairer's stiffness test from DOPRI5. After each accepted Dormand–Prince
step, `h·‖k7 − k6‖ / ‖x7 − x6‖` estimates `h·|λ|` for the dominant eigenvalue from the last two
stages. Above 3.25, at the edge of the method's stability region, the step counts as stiff. Six
non-stiff steps in a row reset the count. After 15 stiff steps the flag is set and stays set for
the run. The other methods never set it. Measured with Dopri5 at the default tolerances, the
flag fires on the relaxation oscillator from μ = 30 (template start points, t ≤ 40), and not at
μ = 20. A rejection-ratio rule was the alternative and would not work. On the stiff runs below,
Dopri5 rejects 1 try in 10⁴, fewer than on the non-stiff Van der Pol at μ = 1 (1 in 9), because
a step held back by stability is always accurate enough:

| problem | method | accepted | rejected | looks stiff |
| --- | --- | ---: | ---: | --- |
| Van der Pol μ = 1000 (Liénard), t ≤ 3000, rtol 1e−6 | Dopri5 | 1,585,080 | 98 | yes |
| | Rosenbrock23 | 4,366 | 54 | no |
| Robertson, t ≤ 40, rtol 1e−8, atol 1e−14 | Dopri5 | 34,856 | 2 | yes |
| | Rosenbrock23 | 6,737 | 2 | no |
| Van der Pol μ = 1, t ≤ 40, rtol 1e−6 | Dopri5 | 267 | 30 | no |

`TrajectoryOut.stiff?: boolean` is `true` when the flag fired, and absent otherwise. A section
run reports it too. The UI shows "These steps look stiff: Dormand–Prince is limited by stability,
not accuracy." in the Solver panel with a "Use Rosenbrock 2(3)" button, and a "Steps look stiff"
warning in the status bar that opens the Solver panel. A Lyapunov run of a flow refuses
Rosenbrock23 with a message, because the variational system it integrates has no exact Jacobian.

The chaos gate solves Robertson (`x' = −0.04x + 1e4·yz`, `y' = 0.04x − 1e4·yz − 3e7·y²`,
`z' = 3e7·y²` from `(1, 0, 0)`) through `compute_portrait`. The SUNDIALS `cvRoberts_dns` and
Hairer–Wanner values at t = 40 are (7.158271e−1, 9.185535e−6, 2.841637e−1). Dopri5 at rtol 1e−11
confirms them to 1.6e−7 relative, and Rosenbrock23 at rtol 1e−8 to the same (claim 1e−5 per
component). Dopri5 at rtol 1e−8 flags that run as stiff and takes 34,856 steps, so the wrong
claim "within 2,000 steps" fails. On the relaxation-oscillator template at μ = 1000, Rosenbrock23
takes 0.0028 of Dopri5's steps at rtol 1e−6 (claim ≤ 0.1), and Dopri5 flags the run. At Robertson's
rtol 1e−8 the ratio is only 0.19: an order-2 method pays for tight tolerances.

### Lyapunov spectrum

```rust
pub struct LyapunovOptions { pub transient: f64 /* time, or iterates for a map */, pub total: f64,
                             pub interval: f64 /* τ; iterates per QR step for a map */,
                             pub blocks: usize /* ≥ 2 */, pub history: usize /* max samples */ }
pub struct LyapunovSpectrum { pub exponents: Vec<f64>, pub uncertainty: Vec<f64>, pub sum: f64,
                              pub trace_average: f64, pub history_t: Vec<f64>,
                              pub history: Vec<f64> /* row-major, history_t.len() · n */,
                              pub renormalizations: usize, pub status: Status, pub stats: Stats }
pub fn lyapunov_spectrum(f: &dyn VectorField, p: &[f64], t0: f64, x0: &[f64],
                         solver: &SolverOptions, opts: &LyapunovOptions) -> LyapunovSpectrum;
```

Benettin's method with QR by modified Gram–Schmidt, applied twice. A flow first runs `transient`
with `solver`, then integrates the orbit with `n` tangent vectors and one more state, the state
`(x, Φ, s)` of length `n + n² + 1` with `Φ' = J(x)Φ` (`J` from `VectorField::jacobian`: exact
forward-mode AD for compiled systems) and `s' = tr J(x)`. The integrator lands exactly on every
`t_start + k·τ`, where the columns of `Φ` are orthonormalized and `ln R_ii` added to exponent `i`.
The last interval is shortened to end at `t_start + total`, and `total / τ` within 1e−9 of an
integer counts as that integer. `solver.maxSteps` bounds the accepted steps of the whole run. A
map runs `transient` iterates, then `Q ← J(x_n) Q` and `x_{n+1} = f(x_n)` for `total` iterates
(rounded), with a QR step every `interval` iterates (rounded, at least 1) and after the last.

- `exponents[i]` is `Σ ln R_ii` over the elapsed time (or iterates), in column order, so the first
  is the largest. It is empty when no QR step was completed.
- `uncertainty[i]` is the standard error over blocks. The QR steps are split into `blocks`
  consecutive blocks (step `k` of `K` goes to block `⌊k·B/K⌋`), each block gives its own
  exponent, and the uncertainty is their sample standard deviation over `√B`, counting the
  blocks reached. It is empty when fewer than two blocks were reached.
- `sum = Σ exponents`. `trace_average` is what it must equal. For a flow that is `s` summed over
  the intervals over the elapsed time, the time average of `tr J`. For a map it is the mean of
  `ln |det J|` by Gaussian elimination with partial pivoting. For a flow, `sum − trace_average` is
  the integrator's error on `ln det Φ`. On Lorenz with RK4 it is 1.0e−4 at h = 0.01, 6.2e−6 at
  h = 0.005 and 9.8e−9 at h = 0.001, so it shrinks like h⁴.
- The running estimate (cumulative `ln R / elapsed`) is sampled every `⌈K / history⌉`-th QR step
  and after the last, at most `history` samples. `history_t` is the time, or the iterate index of
  a map.
- `status` is `blewUp` when a state component, including a tangent vector of a flow, exceeds
  `BLOW_UP_THRESHOLD` (keep `τ` short enough that `e^{λ1 τ}` stays far below it). It is
  `nonFinite` when `f`, `J` or a tangent vector stops being finite, and otherwise the solver's
  status. The estimates always cover the QR steps completed.

`Tangent::eval` does not allocate. The Jacobian scratch is on the stack for `n ≤ 16` and a
thread-local buffer above.

### Orbit diagrams

```rust
pub struct OrbitDiagramOptions { pub param: usize, pub range: (f64, f64), pub columns: usize,
                                 pub state: usize, pub transient: usize, pub kept: usize }
pub struct OrbitDiagram { pub r: Vec<f64>, pub x: Vec<f64> /* columns × kept, by column */,
                          pub kept: usize }
pub fn orbit_diagram(f: &dyn VectorField, p: &[f64], x0: &[f64],
                     opts: &OrbitDiagramOptions) -> OrbitDiagram;
pub struct Doubling { pub r: f64, pub period: usize /* the cycle that loses stability */ }
pub fn period_doublings(f: &dyn VectorField, p: &[f64], param: usize,
                        diagram: &OrbitDiagram, max: usize) -> Vec<Doubling>;
```

`orbit_diagram` iterates the map `x_{n+1} = f(n, x_n)` from `x0` at each of `columns` parameter
values, `r_j = lo + j·step` with `step = (hi − lo)/(columns − 1)` (one multiplication and one
addition, so the same bits on every platform; one column is `[lo]`). `p[param]` is replaced by
`r_j` and the other parameters are kept. Column `j` is `x[j·kept .. (j + 1)·kept]`, the values of
state `state` at `x_T … x_{T+kept−1}` (`x_0 = x0`, `T = transient`). From the first iterate with a
component beyond `BLOW_UP_THRESHOLD` or not finite, the rest of the column is NaN. The columns
run one after another on one thread (rayon is not a dependency; the evaluation-speed measurement
decides whether one core is enough).

`period_doublings` works on one-dimensional autonomous maps and is empty for anything else. It
labels each column with the smallest `P ∈ {1, 2, 4, …}` (`2P ≤ kept`) such that every kept
iterate agrees with the one `P` later, `|x_{n+P} − x_n| ≤ 1e−7·(1 + |x_n|)`, and skips columns
with no label. A column labelled `P` followed by one labelled `2P` marks a doubling of the
`P`-cycle. The labels alone don't bracket it. Just below a doubling the orbit still alternates
about the `P`-cycle, decaying so slowly that it repeats at lag `2P` within the tolerance. On the
logistic map with 2000 columns over `[2.8, 3.57]` and a 2000-iterate transient, 8 columns below
`r1 = 3` already read period 2. So the `P`-cycle is followed column by column, from the column
before the last `P` column, by Newton's method on `f^P(x) = x`. The first seed is that column's
last kept iterate, and each later one is the previous column's cycle point. The walk stops when
the multiplier `μ_P(r) = ∏ f′(x_i)` (from `VectorField::jacobian`, exact for compiled systems)
drops to −1 or below. The last two columns bracket the doubling, which is located by bisection on
`μ_P(r) + 1` to a bracket of 1e−12. A doubling is skipped when the walk starts at or below −1,
never gets there, or Newton fails. On the logistic map (above, `kept = 64`) it finds `r1 … r5`
with `r1` and `r2 = 1 + √6` within 1e−13.

## IPC (Tauri commands)

JSON, camelCase. `app/src-tauri/src/ipc.rs` is the source of the TypeScript types: ts-rs renders
every IPC type, and the maths crates' enums it reuses (`SystemKind`, `InputFormat`, `Severity`,
`Method`, `Status`, `Stats`, `Direction`, `Classification`, derived behind each crate's `ts` feature), into
`app/src/api/ipc.generated.ts` (`UPDATE_IPC_BINDINGS=1 cargo test -p separatrix
typescript_bindings`). `app/src/api/types.ts` only re-exports the generated types, so the UI
compiles against what Rust sends, and CI fails while the generated file is stale. The same file
carries the constants the UI shares with the core: `MAX_SECTION_POINTS`, `MAX_RENORMALIZATIONS`,
`MAX_ORBIT_COLUMNS`, `MAX_ORBIT_POINTS`, `BLOW_UP_THRESHOLD`, `LYAPUNOV_HISTORY`, the `SECTION_MISFIT` and
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
| `compute_lyapunov` | `LyapunovRequest` | `LyapunovResponse` |
| `compute_orbit_diagram` | `OrbitDiagramRequest` | `OrbitDiagramResponse` |

`compile_system`'s `source` is the system's non-empty lines, as typed, joined with `"\n"`, in its
one `format`. `convert_source` is `dyn_expr::convert` (above): one response line per request line,
in order.

`compute_lyapunov` takes `LyapunovRequest { requestId, systemId, params, x0, t0, transient, total,
interval, blocks, solver }` and returns `LyapunovResponse { requestId, exponents, uncertainty,
sum, traceAverage, historyT, history, renormalizations, status, stats, elapsedMs }`. That is
`lyapunov_spectrum` (above) with `LYAPUNOV_HISTORY = 2000` running-estimate samples. For a map,
`transient`, `total` and `interval` count iterates and `t0` and `solver` are ignored. Refusals
are messages, not panics. The command refuses a parameter count or starting point that doesn't
fit the system, a non-finite start, a negative transient, `total ≤ 0` or `interval ≤ 0` (below
half an iterate for a map), more than `MAX_RENORMALIZATIONS = 10⁷` QR steps (`total / interval`),
`blocks < 2`, and more blocks than QR steps. A long run takes seconds. The command runs on a
blocking worker thread and the UI drops stale responses by `requestId`.

The studio's Lyapunov view sends one request per run (`buildLyapunovJob` in state/model.ts): the
selected start point, else the first; the view's `transient`, `total`, `interval` and `blocks`;
and the studio's own solver, so the status bar and an export caption name the integrator that
ran. Because `solver.maxSteps` counts the whole run, the view raises the studio's step limit to
what the run needs when that is larger: `(transient + total) / h` plus one shortened step per QR
step for a fixed step, 100 steps per renormalization interval for an adaptive method, at most
10⁹. It asks for nothing while there is no start point, while a setting would be refused (the
fields say why), or in any other view, and zooming or panning the plot asks for nothing. The
Lyapunov view asks for no portrait.

`compute_orbit_diagram` takes `OrbitDiagramRequest { requestId, systemId, params, param, range:
[lo, hi], columns, state, x0, transient, kept, doublings }` and returns `OrbitDiagramResponse {
requestId, r, x, doublings?: { r, period }[], elapsedMs }`. That is `orbit_diagram` (above), and
`period_doublings` with no limit when `doublings` is true; `doublings` is absent otherwise. `x`
is `columns · kept` values, column by column, with `null` (JSON has no NaN) from the iterate where
the orbit escaped. The command refuses a flow, a parameter index or state index outside the
system, a range that is not finite with `lo < hi`, a parameter count or starting point that
doesn't fit, a non-finite start, `columns` outside `2..=MAX_ORBIT_COLUMNS = 4096`, `kept = 0`,
more than `MAX_ORBIT_POINTS = 4·10⁶` points (`columns · kept`), and `doublings` on a map with more
than one state or one that depends on `n`. It runs on a blocking worker thread.

The studio's orbit diagram sends one request per view of it (`buildOrbitJob` in state/model.ts):
`columns` is the plot's width in device pixels (at most 4,096), `range` the centers of the first
and last pixel across the view's `xRange`, so column `j` is the `j`-th pixel's center and a click
there sets the parameter to exactly that column's `r` (`orbitColumnAt`). The swept parameter's
own value is sent as `range[0]` (each column replaces it), so moving its slider asks for nothing;
zooming or panning across, a wider plot, another start point (the selected one, else the first),
the transient or the iterates drawn ask again, and zooming up the side asks for nothing.
`doublings` is asked for exactly when the map is one-dimensional and autonomous. The inspector
names the doublings Strogatz's way: `r₁, r₂, …` for the run of periods 1, 2, 4, … the list starts
with (a zoomed range that starts past `r₁` begins at `r₂`), and any other doubling by its periods
(`3 → 6`). The orbit view asks for no portrait.

The screen path stays JSON with `f64` values (`null` for an escaped iterate), measured in the
release app on Linux under Xvfb at its default size: the logistic map's default request (774
columns × 200 iterates kept, 1,000 dropped, `r ∈ [0, 4]`, doublings asked for), seven runs
alternated with another view, took 19 ms in Rust (16–22 ms), 53 ms round trip including the
transfer (52–60 ms) and 4 ms to draw (3–5 ms): about 57 ms in all, under the 100 ms at which a
binary `f32` response would be worth its second encoding. The transfer grows with the points: the
widest plot (4,096 columns, 5.3 times as many) would take about 0.2 s by that measure (an
estimate, not measured).

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
