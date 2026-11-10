# Special bifurcations: implementer brief

This is the working brief for whoever implements the plan
[Special bifurcations: implementation plan](https://claude.ai/artifact/CnEjzUVVUN88NjsX9VxSTt).
The plan says what is built and why, frontend first. This file says how it fits this codebase:
where the code is, the formulas, the traps already found, and the tasks and tests per milestone.

Read in this order: the plan, then this brief, then [codim2-reference.md](../codim2-reference.md)
for the numbers every gate uses, then [CONTRIBUTING.md](../../CONTRIBUTING.md).

Line numbers are those of commit `07cbdb3` and will drift. The identifiers are the anchor.

## 1. Ground rules

- **One milestone per session and per pull request.** Do not start the next one in the same
  change. M1 is design work in the Screens canvas; M2 to M9 are code.
- **Branch off `main`.** Never commit to `main`. Conventional Commits titles. Update
  `CHANGELOG.md` for anything a user can notice.
- **Every check in CONTRIBUTING.md must pass**: Rust tests in release mode, clippy on two
  toolchains, `cargo fmt`, the TypeScript build, the interface unit tests, the end-to-end tests,
  and the three numerical gates.
- **Existing output must not change.** `Residual::follow` produces the same points on Linux,
  macOS and Windows, and export golden files depend on it. See "Keep expected files identical on
  every OS" in CONTRIBUTING.md: no `libm` function may decide which points are kept. That is why
  `continuation.rs` has its own `cos`.
- **No new dependency** without asking. `nalgebra` is already used for eigenvalues of dimension 3
  and more; `lu_solve` and `lu_det` are in `crates/dyn-analysis/src/linalg.rs`.
- **Python is an oracle only.** `scripts/reference/codim2_reference.py` recomputes the reference
  values. It never runs in CI. Rust tests carry the numbers as constants.
- **Report honestly.** If a gate fails or a formula does not reproduce a reference value, say so
  with the numbers. Do not loosen a tolerance to pass.

### Decisions taken as defaults

The plan lists five open questions. Until Ulises answers them, use these:

| Question | Default |
| --- | --- |
| View name and short label | "Parameter plane", short "Params", view kind `curves` |
| Live inset | In M7, not before |
| Search when the view opens with no curve | Yes, it starts from the Branch view's special points, as Branch starts from the fixed points in view |
| Second parameter offered first | The next parameter in declaration order after the Branch view's parameter |
| Resonances | Strong resonances (1:1 to 1:4) in M6. Weak-resonance ticks in M7. Tongues only through existing tools |

## 2. Notation

| Symbol | Meaning |
| --- | --- |
| `f(x, p)` | The field (flow) or the map |
| `g(x, p)` | `f` for a flow; `f^k(x, p) − x` for a cycle of period `k` of a map |
| `A` | `f_x` for a flow; `D f^k` for a map (`Power::jacobian`) |
| `M` | The matrix that is singular on the curve: `A` (fold of a flow), `A − I` (fold of a map), `A + I` (flip), a bialternate product (Hopf, Neimark–Sacker) |
| `B(u, v)`, `C(u, v, w)` | Second and third derivative tensors of `f` (of `f^k` for a map) applied to vectors |
| `p₁`, `p₂` | The Branch view's parameter (drawn across) and the second parameter (drawn up) |
| `z` | The unknown of a two-parameter curve, `(x, p₁, p₂)`, `n + 2` numbers |

Labels: LP fold, H Hopf, PD flip, NS Neimark–Sacker (codimension 1, exist today). BT
Bogdanov–Takens, CP cusp, GH Bautin, ZH zero-Hopf, HH Hopf–Hopf, R1 to R4 strong resonances, CH
Chenciner, LPPD fold-flip, GPD generalized flip (codimension 2, new).

## 3. What exists in the Rust core

All in `crates/dyn-analysis/src/continuation.rs` unless noted.

| Item | Where | What it does |
| --- | --- | --- |
| `VectorField` | `crates/dyn-core/src/lib.rs:29` | `eval`, `jacobian`, `param_jacobian`, with finite-difference defaults |
| `CompiledSystem` | `crates/dyn-expr/src/system.rs:64` | Exact `jacobian` and `param_jacobian` by one dual-number pass per seed (`run_dual1`, :149). `second_derivative` (:168) already runs nested duals `D2`, but only for the Hamiltonian layout |
| `Scalar`, `Dual<T>` | `crates/dyn-expr/src/ad.rs` | The evaluator is generic over `Scalar`, so `Dual<Dual<f64>>` gives exact second derivatives |
| `Power` | :639 | `f^k` as a `VectorField`: `eval`, `jacobian` and `param_jacobian` by the chain rule along the orbit |
| `Residual` | :704 | `g` for one free parameter. `linear(z)` (:875) returns `g` and `[g_x | g_p]`, row-major `n × (n+1)` |
| `correct_along`, `tangent`, `newton`, `bordered` | :907, :921, :1058, :1088 | Pseudo-arclength corrector on a hyperplane, unit tangent from the bordered matrix, Newton with `TOL = 1e-10`. They use only `linear`, so they are already generic |
| `Residual::follow` | :744 | The step loop: predictor, corrector, angle limit, step halving and growth, range edges, detection, location. Specific to `(x, p)` |
| `DETECTORS` | :362 | One test function per codimension-1 bifurcation, with the number of eigenvalues that must cross (`jump`) |
| `sign_change`, `locate` | :348, :958 | Sign change between two accepted points, then bisection in arclength with `correct_along` |
| `first_lyapunov` | :470 | ℓ₁ at a Hopf point. `B` and `C` come from central differences of the exact Jacobian |
| `second_direction` | :930 | Null vector by shifted inverse iteration (`INVERSE_ITERATION_SHIFT`), used by `switch_branch` |
| `eigenvalues` | `linalg.rs:90` | Eigenvalues as `(re, im)` pairs |
| Tests | `crates/dyn-analysis/tests/continuation.rs` | Closed-form checks; copy this style |
| Gate | `app/src-tauri/tests/continuation.rs`, `scripts/continuation-gate.sh` | Claims driven through the service layer with `Run` records |

## 4. The engine (M3, M4)

### 4.1 The follower

A two-parameter curve is `N = n + 1` equations in `N + 1 = n + 2` unknowns, the same shape as a
branch (`n` equations in `n + 1` unknowns). `correct_along`, `tangent` and `newton` need only a
function `z ↦ (G(z), DG(z))`.

Preferred approach, lowest risk:

1. Introduce a small trait for "N equations in N + 1 unknowns" with one method,
   `linear(&self, z) -> (Vec<f64>, Vec<f64>)`. Implement it for `Residual`. Make `correct_along`,
   `tangent` and the bisection helper free functions over the trait.
2. Leave `Residual::follow` as it is. Its range-edge handling and its `jump` accounting are
   specific to branches.
3. Write a second, simpler loop for curves that shares the step control constants
   (`STEP_GROWTH`, `FAST_NEWTON_ITERS`, `SLOW_NEWTON_ITERS`, the angle limit).

Merging the two loops is allowed only if every existing test and golden file stays identical.
The plan's M3 gate is exactly that.

Differences the curve loop must handle:

- **A box, not an interval.** The curve stops when it leaves the rectangle of the two parameter
  ranges. Land on the edge the way `follow` does: predict the crossing, then correct with that
  parameter fixed.
- **Scaling.** Parameters can differ by orders of magnitude (Lorenz-84: `F` near 1.5, `T` near
  0.02). Follow the curve in scaled parameters `(p − lo)/(hi − lo)` so that Δs is a fraction of
  the box. Convert at the boundary of the defining system.
- **Closed curves.** A curve can return to its start. Stop when it comes back within one step of
  the first point with the same tangent direction.
- **Both directions.** Run forward and backward from the seed, as branches do.

### 4.2 The derivative primitive

Add to `VectorField` in `dyn-core`:

```rust
/// out[i*dim + j] = d/dε ∂f_i/∂x_j (x + ε·dx, p + ε·dp) at ε = 0.
fn jacobian_derivative(&self, t: f64, x: &[f64], p: &[f64], dx: &[f64], dp: &[f64], out: &mut [f64]);
```

- **Default:** central difference of `jacobian` along `(dx, dp)`.
- **`CompiledSystem`, direct layout:** exact. For each state `j`, one nested-dual pass with the
  outer seed on state `j` and the inner seed along `(dx, dp)`; read `.eps.eps` of every output.
  `second_derivative` is the model.
- **`CompiledSystem`, Hamiltonian layout:** keep the finite-difference default. It would need
  third-order duals.
- **`Power`:** the recurrence below, checked against finite differences.

```text
M₀ = I, dM₀ = 0, u₀ = dx
for each point x_j of the orbit:
    dM ← D(x_j; u, dp)·M + J(x_j)·dM      D = jacobian_derivative of f
    M  ← J(x_j)·M
    u  ← J(x_j)·u + f_p(x_j)·dp
result: dM
```

With this primitive `B(u, v) = D(u, 0)·v`, exactly, and `first_lyapunov` can use it for `B`.
Do that only if its tested values do not move beyond their tolerances.

### 4.3 The four defining systems

Each is `G(z) = (g(x, p); ψ(x, p))`, where ψ is the scalar `s` of a bordered system on the
matrix `M` that is singular on the curve:

```text
[ M    w₀ ] [ q ]   [ 0 ]          [ Mᵀ   v₀ ] [ w ]   [ 0 ]
[ v₀ᵀ  0  ] [ s ] = [ 1 ]          [ w₀ᵀ  0  ] [ s ] = [ 1 ]
```

- `s = 0` exactly when `M` is singular, and then `q` and `w` are its right and left null vectors.
- The derivative along any direction is `s' = −wᵀ M' q`. Checked numerically.
- `v₀`, `w₀` are fixed during a correction and replaced by the normalized `q`, `w` after each
  accepted step.
- For the first point there is no `q` yet. `second_direction` shows how this file already finds
  a null vector without an SVD.

| Curve | `M` | Size | `M'` along a direction |
| --- | --- | --- | --- |
| Fold, flow | `A` | n | `jacobian_derivative` |
| Fold, map | `A − I` | n | `jacobian_derivative` of `Power` |
| Flip | `A + I` | n | the same |
| Hopf | `2A ⊙ I` | n(n−1)/2 | the bialternate formula below applied to `A'`; it is linear in `A` |
| Neimark–Sacker | `A ⊙ A − I` | n(n−1)/2 | `A' ⊙ A + A ⊙ A'`, by the product rule on the formula below |

Rows and columns of a bialternate product are indexed by pairs `(p, q)` with `p > q`:

```text
(2A ⊙ I)[(p,q),(r,s)] =  −a_ps          if r = q
                          a_pr          if r ≠ p and s = q
                          a_pp + a_qq   if r = p and s = q
                          a_qs          if r = p and s ≠ q
                         −a_qr          if s = p
                          0             otherwise

(A ⊙ A)[(p,q),(r,s)]  =  a_pr·a_qs − a_ps·a_qr
```

Checked on random matrices for n = 2 to 5: the eigenvalues of `2A ⊙ I` are the sums `λ_i + λ_j`,
and those of `A ⊙ A` are the products `μ_i μ_j`. For n = 2 they reduce to the trace and the
determinant. For n = 1 there is no Hopf or Neimark–Sacker curve.

The Jacobian of `G` then has rows `[g_x | g_p₁ | g_p₂]` (from `jacobian` and `param_jacobian`) and
one last row `ψ_z`, with `ψ_xj = −wᵀ M'(e_j) q` and `ψ_pk = −wᵀ M'(e_pk) q`. For the fold and
flip, symmetry of `B` gives all state entries from one call: `ψ_x = −wᵀ D(q, 0)`.

### 4.4 Neutral saddles

The Hopf condition also holds where two real eigenvalues are `±λ`, and the Neimark–Sacker
condition where two real multipliers are `μ` and `1/μ`. That is intended: it lets the curve pass
through a BT or R2 point. Classify every point of the curve from its eigenvalues:

- **Flow.** Take the pair `(i, j)` with the smallest `|λ_i + λ_j|`. `κ = Re(λ_i λ_j)`. `κ > 0` is
  a Hopf point with `ω = √κ`; `κ < 0` is a neutral saddle.
- **Map.** Take the pair with the smallest `|μ_i μ_j − 1|`. `c = Re(μ_i + μ_j)/2`. `|c| < 1` is a
  Neimark–Sacker point with `cos θ = c`; `|c| > 1` is a neutral saddle.

The frontend draws neutral stretches dashed. Never report a neutral saddle as H or NS. The
reference page has a case: the CO oxidation model at `K = 2`.

### 4.5 Starting a curve

The seed is a special point from a branch: its kind, state, parameter value and period. Free the
second parameter and build the defining system at the seed. The seed satisfies `g = 0` and
nearly satisfies `ψ = 0`, so correct it first with the second parameter held fixed: `n + 1`
equations in `n + 1` unknowns. Then take the tangent from the bordered Jacobian and follow both
ways.

If `ψ_p₂` and `g_p₂` are both zero the second parameter does not move the bifurcation. Return an
error the UI can show as "K does not move this bifurcation."

## 5. Special points (M6)

Evaluate these at every accepted point of a curve. On a sign change, bisect in arclength as
`locate` does, and record the width of the final bracket: the UI shows it.

| Point | On curve | Test function (zero at the point) |
| --- | --- | --- |
| BT | Fold (flow) | `wᵀq`, the left and right null vectors from the bordered systems |
| BT | Hopf | `κ` |
| CP | Fold | Fold coefficient `a = ½ wᵀ B(q, q)` with `wᵀq = 1` |
| ZH | Fold (flow) | Largest real part among the complex pairs of the other eigenvalues, as today's Hopf detector does |
| ZH | Hopf | Product of the eigenvalues outside the critical pair |
| GH | Hopf, where `κ > 0` | ℓ₁ from `first_lyapunov` |
| HH | Hopf | Today's Hopf test on the eigenvalues outside the critical pair |
| R1 | NS; fold (map) | `c − 1`; on the fold curve `wᵀq` |
| R2 | NS; flip | `c + 1`; on the flip curve `wᵀq` with `M = A + I` |
| R3 | NS | `c + ½` |
| R4 | NS | `c` |
| CH | NS, where `|c| < 1` | Neimark–Sacker coefficient `d` (§6) |
| LPPD | Fold (map); flip | `det(A + I)`; `det(A − I)` |
| GPD | Flip | Flip coefficient (§6) |

Traps:

- **Poles look like zeros.** ℓ₁ divides by ω and blows up at BT. `d` blows up at R1 and R3 and
  is meaningless at R4. Accept a sign change only if the test value shrinks toward zero during
  the bisection. Discard it if it grows.
- **Orientation.** `wᵀq` is continuous only if `q` and `w` come from the bordered systems with
  borders carried from the previous point. Null vectors from a fresh decomposition flip sign at
  random.
- **The same point from two curves.** A BT point found on the fold curve and on the Hopf curve
  is one point. Merge by distance in the list, and test that the two locations agree.
- **A fold-flip that is not a fold.** In the predator–prey map the flip curve meets the
  multiplier-+1 curve at `(ε, a) = (2, 3)` on the transcritical line `y = 0`. Report it, but the
  sentence must not promise a fold.

**Switching.** At BT and ZH the other curve passes through the same point and its defining
system is regular there. Start it at the special point itself and take the tangent from its own
Jacobian. No predictor is needed. The same holds at LPPD between fold and flip curves.

## 6. Coefficients (M7)

Each formula was evaluated in Python during planning and reproduced the value in the last
column. The Rust versions still have to be written and tested, except ℓ₁.

| Coefficient | Formula | Scaling | Checked against |
| --- | --- | --- | --- |
| Fold `a` | `½⟨p, B(q, q)⟩` | `‖q‖ = 1`, `⟨p, q⟩ = 1` | MatCont, CO oxidation: 0.1167 and 0.1347, to four digits |
| Hopf ℓ₁ | In `first_lyapunov` | `⟨q, q⟩ = ½`, divided by ω | Closed forms in the existing Rust tests |
| Neimark–Sacker `d` | `Re(μ̄ c₁)`, `c₁ = ½⟨p, C(q,q,q̄) + 2B(q, (I−A)⁻¹B(q,q̄)) + B(q̄, (μ²I−A)⁻¹B(q,q))⟩` | `⟨q, q⟩ = ⟨p, q⟩ = 1` | MatContM, delayed logistic at r = 2: −1.0 |
| Flip `c` | `⅙⟨p, C(q,q,q) + 3B(q, (I−A)⁻¹B(q,q))⟩` | `‖q‖ = 1`, `⟨p, q⟩ = 1` | Logistic map at r = 3: 9, the scalar formula `f″²/4 + f‴/6` |
| BT `(a, b)` | `a = ½ p₁ᵀB(q₀,q₀)`, `b = p₀ᵀB(q₀,q₀) + p₁ᵀB(q₀,q₁)` | `A q₀ = 0`, `A q₁ = q₀`, `Aᵀp₁ = 0`, `Aᵀp₀ = p₁`, `‖q₀‖ = 1`, `q₀ᵀq₁ = 0`, `p₁ᵀq₁ = 1`, `p₀ᵀq₁ = 0` | MatCont, CO oxidation: (0.083784, 2.13628) and (0.048226, 1.93763) in absolute value |
| Cusp `c` | `⅙⟨p, C(q,q,q) + 3B(q, h₂)⟩`, `A h₂ = 2a q − B(q,q)` with `qᵀh₂ = 0` | `‖q‖ = 1`, `⟨p, q⟩ = 1` | MatCont, CO oxidation: 0.36279 |

Rules for comparing with the literature:

- **Signs that depend on orientation.** Fold `a` flips with `q`. BT `a` and `b` both flip with
  `q₀`. Compare `|a|`, `|b|` and the sign of the product `ab`, never the single signs. Cusp `c`,
  ℓ₁, `d` and flip `c` do not flip.
- **MatCont's ℓ₁** is `Re(c₁)/2` with `⟨q, q⟩ = 1` and no division by ω. Its 10.70086 for the CO
  oxidation Hopf point equals ℓ₁·ω in Kuznetsov's scaling. `dyn-analysis` uses a third scaling.
  Compare signs and zeros.
- **Bautin ℓ₂ and the other codimension-2 normal forms** need derivatives up to fifth order.
  They are not required. The row's sentence does not depend on them.
- **Third derivatives.** `C(u, v, w)` can stay a difference of `jacobian_derivative`, as
  `first_lyapunov` does today for `C`.

## 7. Homoclinic orbits and Shilnikov (M8)

Scope: an equilibrium of a flow whose unstable manifold is one-dimensional. With a
one-dimensional stable manifold instead, reverse time. Anything else is out of scope.

The recipe that reproduced AUTO's values for the Freire circuit:

1. At the equilibrium take the unstable eigenvalue `λᵤ`, its unit eigenvector `v`, and the left
   eigenvector `l` scaled so that `l·v = 1`.
2. Integrate from `x* + ε v` with `ε = 1e-6` (and `−v` for the other half), tight tolerances.
3. Let the orbit leave a small ball around `x*`, make its excursion, and come back. The **split
   function** is `l·(x − x*)` at the **first re-entry** into the ball.
4. A homoclinic orbit is a zero of the split function. Scan one parameter, bisect on a sign
   change.
5. Follow the zero in two parameters with the follower of §4.1: one equation in two unknowns,
   the equilibrium solved by Newton inside, the Jacobian by finite differences.

Traps, each of them met:

- **Do not use the closest approach to the equilibrium as the test.** It is discontinuous: it
  jumps between loops of the orbit and finds false roots.
- **Use the first re-entry only.** Where σ > 0 there are infinitely many other homoclinic orbits
  with more loops. At β = 0.4535645 a second root sits at ν = −0.72695630, with twice the return
  time. Seeing it is expected, and reporting it as the primary orbit is wrong.
- **The ball radii were absolute** in the check (0.05 to re-enter, 0.3 to count as having left).
  Make them relative to the size of the first excursion.
- **Precision is five digits.** The split function treats the stable manifold as flat inside the
  ball. A smaller ball is more exact and slower.
- **The saddle quantity** is `σ = λᵤ + Re λₛ`, with `λₛ` the stable eigenvalue nearest the
  imaginary axis. A saddle-focus has a complex `λₛ`. Shilnikov's chaos needs a saddle-focus and
  `σ > 0`. AUTO's start point for the Freire circuit has `σ = −0.017`: do not use it as the
  chaotic test case.
- **From a BT row.** The textbook predictor for the homoclinic curve needs the BT normal form
  and a parameter transformation. A simpler route to try first: on the side of BT where the Hopf
  curve is a true Hopf curve, hold one parameter at a small offset from BT and scan the other
  outward from the Hopf curve for a sign change of the split function. This is a proposal, not a
  verified method. The Freire circuit, whose BT point is at `ν = −β`, `β = √r`, is the test.

This milestone also needs the app's first job with progress and cancellation. Today every
command is request and response with no cancel: `LatestRequestScheduler` and the continuation
pump only drop answers nobody wants any more (§9).

## 8. Milestones: tasks and tests

The gate of each milestone is in the plan. This adds the tasks and the exact tests.

### M1 Screens

- Work in [Separatrix Screens](https://claude.ai/artifact/XNakYimVqGnpba23xQ88Q9). Follow that
  canvas's own instructions. Start from the artboard `Continuation.dc.html`.
- Add to the row "Chaos and bifurcations": the Parameter plane for a flow, the Parameter plane
  for a map, and the Branch view with **Follow in two parameters** on a row and **Search in two
  parameters** in the Special points header.
- Use real content: the CO oxidation model and the predator–prey map with the values of
  `codim2-reference.md`.
- Cover the states of the plan: unavailable, empty, running, a stalled curve, done.
- The design system gets one colour and one dash pattern per curve kind before the artboards
  use them. `app/src/styles/ds/` is a vendored copy; change the published system first.
- Done when Ulises approves the artboards. Do not write app code in this milestone.

### M2 View on mock data

- Fix the wire types first, in `app/src-tauri/src/ipc.rs`, and generate
  `app/src/api/ipc.generated.ts`. The Rust command may return "not implemented"; the mock
  serves the data.
- Mock data must be real. For the map, the three curves are closed forms (reference page,
  system 3). For the CO oxidation model, extend the reference script to write the fold and Hopf
  curves as JSON: sweep the state `x` and solve for `(y, s, k₂, K)`. This was tried: from the
  one-parameter LP and H points it runs through the cusp and reaches both BT points, with `x`
  from about 0.012 to 0.13. The curves are steep in `x` between 0.016 and 0.018, so use a finer
  grid there. Test that the mock curves pass through the reference points.
- Add the two reference systems as gallery templates now, with their `SYSTEMS` entries in
  `mock.ts`. The mock has no backend for anything else (§9.5). The existing mock branch follower
  must find their LP, H and NS points, since those are the seeds.
- Build the view kind, the canvas, the two lists, the crosshair, the slice line, and the two
  entry actions in the Branch view.
- Add a short `docs/guide/parameter-plane.md` and its mapping in `scripts/release-check.sh` in
  the same commit as the view kind, or CI fails (§9.5). M9 completes the page.
- Update the tests that hard-code the View control (§9.5).
- Tests: `app/src/review/curves.test.mjs` and `curves.e2e.cjs`, in the style of the
  continuation ones. The end-to-end test clicks from an H row in Branch to a BT row in the
  Parameter plane.

### M3 Engine

- §4.1 and §4.2.
- Tests: `jacobian_derivative` exact against finite differences for a compiled system and for
  `Power` with k = 3; every existing test, gate and golden file unchanged.

### M4 Curves in two parameters

- §4.3 to §4.5, with a public function beside `continue_branch`.
- Tests, in `crates/dyn-analysis/tests/`: the CO oxidation fold curve passes through
  `k₂ = 1.04204851` and `1.05220029` at `K = 0.4`, and its Hopf curve through `1.05155746` and
  `1.04099157`; the map's three curves match `a(ε)` to 1e-8; a neutral stretch is flagged; a
  second parameter with no effect returns the error.

### M5 Real data in the view

- The Tauri command, the service function, the limits on points, the contract section in
  `docs/contracts.md`. The mock stays for the browser preview and must agree with the engine on
  the two mock systems.
- Tests: an integration test through the service layer, like
  `app/src-tauri/tests/continuation.rs`; the M2 end-to-end test still passes.

### M6 Special points

- §5.
- Tests: every location of systems 1 to 3 of the reference page. Six digits in the parameters
  where the page gives eight; the cusp to five. The BT point from the fold curve and from the
  Hopf curve agree. No special point is reported on a neutral stretch except BT, ZH and R2.
- Add a gate `scripts/codim2-gate.sh` with its claims in `app/src-tauri/tests/`, and add it to
  `scripts/release-check.sh` and the list in CONTRIBUTING.md. Like the other gates, it must also
  hand itself wrong claims and require them to fail.

### M7 Sentences and inset

- The sentence and coefficient per row (§6), the live inset, weak-resonance ticks.
- Tests: coefficients against the table in §6 with the comparison rules; the inset stays inside
  the budgets of `docs/performance.md` while dragging.

### M8 Homoclinic and Shilnikov

- §7, and the cancellable job.
- Tests: the Freire circuit at β = 0.6 gives ν = −0.72131 and at β = 0.4535645 gives
  ν = −0.72569, both to five digits; σ is −0.017 and +0.289 there; the curve ends at the BT
  point. (This list first said −0.72132 at β = 0.6. That came from a shooting check with an
  absolute ball of 0.05; with a smaller ball it converges to AUTO's −0.7213093. See
  [codim2-reference.md](../codim2-reference.md) §4.)

### M9 Release

- Exports with golden files, the project view kind with round-trip fixtures, the full guide page
  and its example project, the changelog, the release checklist, and every doc that lists the
  views (§9.5).
- A gallery template that cites a Strogatz section needs a claim in the Strogatz gate. The two
  templates added in M2 cite other sources; say so in their metadata.

## 9. The existing Branch view, layer by layer

The new view copies this pattern. Paths are from the repository root.

### 9.1 Shell: IPC, service, commands

| Item | Where |
| --- | --- |
| IPC types: `ContinuationRequest`, `ContinuationStart`, `SwitchPoint`, `ContinuationDirection`, `ContinuationResponse`, `BranchOut`, `BranchEndOut`, `BranchStop`, `SpecialPointOut`, `SpecialKindOut` | `app/src-tauri/src/ipc.rs:558-768` |
| `MAX_CONTINUATION_POINTS = 20_000` per direction | `ipc.rs:558` |
| Generated bindings: the hand-kept list of `X::decl(&cfg)` and the `consts` table | `ipc.rs` `mod bindings`, `render()` at :1424-1522, consts at :1531-1587 |
| `compute_continuation`, `continuation_refusal` (errors as sentences), `join` (backward and forward halves), `validate_continuation` | `app/src-tauri/src/service.rs:417-657` |
| Kind mapping `SPECIAL_KINDS`, `kind_out`, `kind_in` | `service.rs:493-521` |
| The Tauri command, wrapped in `blocking()`, and its registration in `generate_handler!` | `app/src-tauri/src/lib.rs:137-144`, :1061-1098 |

There is no cancellation anywhere: no cancel command and no abort token.

### 9.2 Interface: API, state, drawing

| Item | Where |
| --- | --- |
| `computeContinuation`: Tauri `invoke`, or the mock in the browser preview | `app/src/api/client.ts:54-56` |
| Re-exports of every generated type | `app/src/api/types.ts` |
| The mock computes branches in JavaScript (`MockBranch`), for gallery templates only (`SYSTEMS`) | `app/src/api/mock.ts:87`, :474-783 |
| `ContinuationView`, `ContinuationSettings`, `ContinuationSeed` | `app/src/state/model.ts:155-172` |
| View plumbing: `View`, `ViewChoice`, `VIEW_CHOICES`, `VIEW_CHOICES_BY_KIND`, labels, availability | `model.ts:174-330` |
| `SPECIAL_KINDS`, the one label table | `model.ts:826-839` |
| `buildContinuationJobs`: one job per seed, keyed by `JSON.stringify(body)` | `model.ts:916-944` |
| `specialRows`, `isSameBifurcation` (`SAME_BIFURCATION_TOL`) | `model.ts:952-964` |
| `switchView`: what starts when the view is opened | `model.ts:1840-1898` |
| `pumpContinuation`: a serial loop, one request at a time, results cached by job key | `app/src/state/useStudio.ts:318-365` |
| `continueFrom` (the Continue button), `addSeed`, `removeSeed`, `setContinuation` | `useStudio.ts:836-910` |
| `BranchCanvas`, click sets the parameter | `app/src/canvas/BranchCanvas.tsx:18-130` |
| `drawBranch`, `stabilityRuns`, `nearestBranchVertex`; colours `th.traj[color % 7]` from `--traj-1…7`; unstable dashes; glyphs by `drawFixedPoint(…, "half")`; label placement | `app/src/canvas/branch.ts` |
| `drawAnalysisFigure`, used by export preview and SVG | `app/src/canvas/figure.ts:149-174` |
| `ContinuationSection`: settings, the Branches list, the Special points list, **Switch branch** | `app/src/components/Inspector.tsx:935-1175` |
| The View control, three segments per row | `Inspector.tsx:376-446` |
| The Continue button on a Fixed points row | `Inspector.tsx:1748-1756` |
| Status bar text | `app/src/components/Chrome.tsx:249-257`, :370-460 |

Continuation does not use `LatestRequestScheduler`; the portrait, Lyapunov and orbit views do.
The live inset should use the portrait path.

### 9.3 Project file and exports

| Item | Where |
| --- | --- |
| `ProjectView.continuation`, `ProjectContinuation`, `ProjectSeed` | `crates/project/src/schema.rs:174-322` |
| Structural check, the only hard-error layer | `crates/project/src/check.rs:120-153` |
| Validation and misfit warnings | `crates/project/src/validate.rs:224-458` |
| Save and load in the interface | `app/src/project/serialize.ts:225-258`, :314-388, :486-502 |
| Fixture and round-trip tests | `crates/project/tests/fixtures/continuation-view.sepx`, `tests/roundtrip.rs` (`GOOD`, `CANONICAL`) |
| Export model: `ContinuationRun`, `BranchRun`, `SpecialRow` | `crates/export/src/model.rs:375-510` |
| `continuation_figure`, `special_points_table` | `crates/export/src/latex_continuation.rs:100`, :332 |
| CSV and `.meta.json` | `crates/export/src/csv.rs:522-563` |
| Golden tests | `crates/export/tests/golden.rs:1137-1338`, files in `tests/golden/` |
| Export service and cache (reruns the requests) | `app/src-tauri/src/export_service.rs:286-455` |
| Interface side | `app/src/export/commands.ts:100-113`, `job.ts:181-230`, `ExportDialog.tsx` |

### 9.4 Tests and where they run

| Layer | File | Runs in CI? |
| --- | --- | --- |
| Core | `crates/dyn-analysis/tests/continuation.rs` | Yes |
| Project, export | `crates/project/tests/roundtrip.rs`, `crates/export/tests/golden.rs` | Yes |
| Shell: the gate claims and the service unit tests | `app/src-tauri/tests/continuation.rs`, `service.rs` | **No.** CI excludes the `separatrix` crate except the bindings and version tests |
| Interface units | `app/src/review/continuation.test.mjs` | **No** |
| End to end, against the mock | `app/src/review/continuation.e2e.cjs` | **No** |

So the shell tests, the gates, the Node tests and the end-to-end tests must be run by hand
before every pull request, as CONTRIBUTING.md lists them. The end-to-end tests never reach Rust.

### 9.5 Traps when adding a sibling view

**Generated and listed by hand**

- Every new IPC type needs a `decl` line in `render()` and its name in the re-export block of
  `app/src/api/types.ts`. The bindings test fails if `types.ts` misses a generated name or
  declares one itself. Regenerate with
  `UPDATE_IPC_BINDINGS=1 cargo test -p separatrix --lib typescript_bindings_are_up_to_date`.
- New project types also need a `decl` in `crates/project/src/lib.rs`. A test there asserts on
  the list by index, so an insertion shifts the indices.
- A new fixture must be added to `GOOD` and `CANONICAL` in `roundtrip.rs`.

**The compiler will force these** (`Record<ViewChoice, …>`): `VIEW_CHOICE_LABELS`,
`VIEW_CHOICE_SHORT`, `viewChoiceAvailability` in `model.ts`; `PICKER` in `CanvasToolbar.tsx`.

**The compiler will not force these.** Each is an if/else or ternary chain with a silent final
branch. Check every one:

- `model.ts`: `viewChoice` (:219), `isAnalysis` (:192), `PlotView` (:183), `adaptView` (:1533),
  `hasEqualScales` (:1691), the portrait job (:1717), `canAddOnPlot` (:1824), `switchView`
  (:1841), `savedCobwebFits` (:1964), `projectView` (:2104), `applyProjectLoad` (:2177).
- `useStudio.ts`: recompile handling (:484), `setAxis` (:753), `liftPoint` (:928).
- `App.tsx:198-221`, the canvas switch.
- `Inspector.tsx`: section visibility (:195-252), `ViewSection` (:380), `SectionKey` defaults.
- `Chrome.tsx:370-460`, the status bar.
- `CanvasToolbar.tsx`: :122-140, :379, :485-499.
- `export/job.ts`: `axisNames`, `AnalysisKind`, `analysisKind`. In `ExportDialog.tsx` many
  `chaos === …` ternaries end in the continuation wording, so a new kind would silently be
  called "the branch plot".
- `figure.ts:154-174`: the final `else` is the orbit diagram.
- `serialize.ts:314-388`: per-kind flags and key deletion.
- Rust: `validate.rs` sends any unknown non-plane kind down the cobweb path with
  `COBWEB_MISFIT`; `check.rs` `VIEW` must list the new sub-object; `export_service.rs:313` has a
  second exhaustive kind match.

**Tests that hard-code the View control.** Adding an option breaks them until updated:
`lyapunov.test.mjs:69` (continuation must be last), `section.test.mjs:48`, `space.test.mjs:117`,
`cobweb.e2e.cjs:263` and :286, `section.e2e.cjs:60`, :109, :115, and `continuation.e2e.cjs:149`
(three segments per row in projector mode at 1,024 px).

**The release check reads the `View` union.** `scripts/release-check.sh` (`view_kinds`,
`view_page`) requires a guide page for every `kind` literal in `export type View =`, and CI runs
it. So the commit that adds the view kind must add `docs/guide/parameter-plane.md` and its
mapping, even as a short page.

**The mock knows only gallery templates.** A system without a `SYSTEMS` entry in `mock.ts` has
no preview backend. The two reference templates therefore arrive with M2, with their mock
entries, not with M9.

**Keys and caches.** Jobs are keyed by `JSON.stringify(body)`, so property order matters. The
export cache compares whole request vectors with `request_id` zeroed. Golden files need LF line
endings.

**Sentences kept in two places.** The seed-misfit sentence is in `validate.rs:355` and
`model.ts:2057`; the "no branches" sentence in `export_service.rs:331` and `mock.ts:2297`.
Prefer a generated constant for new ones, as the orbit, section, 3D and Lyapunov misfits do.

**Docs that list every view:** `docs/guide/README.md`, `README.md`, `site/media.sh` (`VIEWS`),
`site/media/manifest.json`, `scripts/docs-screenshots.sh`, and the gate list in CONTRIBUTING.md.
