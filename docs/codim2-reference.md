# Reference systems for special bifurcations

Four systems from the literature with known codimension-2 points and one homoclinic orbit. They
are the test targets for two-parameter continuation: a curve of folds, Hopf points, flips or
Neimark–Sacker points followed in two parameters must pass through the points listed here.

Every value marked *recomputed* was found by solving the point's defining equations directly, not
by continuation, so it does not depend on the code under test. To rerun:

```sh
uv run --with numpy --with scipy --with sympy python scripts/reference/codim2_reference.py
```

The browser preview draws the Parameter plane from curves the same script writes:

```sh
uv run --with numpy --with scipy --with sympy python scripts/reference/codim2_reference.py \
  --mock-curves app/src/api/mockCurves.generated.json
```

It follows the CO oxidation fold and Hopf curves by pseudo-arclength continuation from the
one-parameter points at `K = 0.4`, and samples the predator–prey map's curves from their closed
forms. It fails if any point of the tables below that lies on a curve is more than `10⁻⁵` (in the
scale of the plotted box) off it.

In the tables, **published** means the number is printed in the source, and **derived** means it
is not printed anywhere and comes from the script alone.

## What to compare

- **Locations** (states and parameters) are safe to compare to the digits given.
- **Normal-form coefficients** depend on how the eigenvectors are scaled. Compare signs and zeros
  unless the scaling is matched:
  - MatCont prints the first Lyapunov coefficient as `Re(c₁)/2` with `⟨q,q⟩ = ⟨p,q⟩ = 1`, with no
    division by `ω`. Kuznetsov's `ℓ₁` with the same scaling is that value divided by `ω`.
  - `dyn-analysis` uses `⟨q,q⟩ = 1/2` (see `SpecialPoint` in
    `crates/dyn-analysis/src/continuation.rs`), a third scaling.
  - The sign of a fold coefficient follows the orientation of the null vector.

## 1. Bykov–Yablonskii–Kim CO oxidation (flow, 3 equations)

Source: Yu. A. Kuznetsov, MatCont Tutorial IV. The same model is BifurcationKit's "CO oxidation"
tutorial, with `q1…q6, k` for the parameters.

```text
ẋ = 2k₁z² − 2k₋₁x² − k₃xy
ẏ = k₂z − k₋₂y − k₃xy
ṡ = k₄(z − Ks)            z = 1 − x − y − s
k₁ = 2.5, k₋₁ = 1, k₃ = 10, k₋₂ = 0.1, k₄ = 0.0675        free: k₂, K
```

One parameter, `K = 0.4` (start: `x = 0.001137, y = 0.891483, s = 0.062345, k₂ = 2.204678`):

| Point | k₂ published | k₂ recomputed | (x, y, s) recomputed | Coefficient published |
| --- | --- | --- | --- | --- |
| Hopf | 1.051557 | 1.05155746 | (0.01635754, 0.52397118, 0.32833662) | ℓ₁ = 10.70086 (subcritical) |
| Fold | 1.042049 | 1.04204851 | (0.02471644, 0.45026004, 0.37501680) | a = 0.1166509 |
| Fold | 1.052200 | 1.05220029 | (0.05402942, 0.30224144, 0.45980653) | a = 0.1346534 |
| Hopf | 1.040991 | 1.04099157 | (0.07792777, 0.23306493, 0.49214807) | ℓ₁ = 4.332247 (subcritical) |

The script's `ℓ₁·ω` is 10.70102 and 4.331379, and its `|a|` is 0.1166593 and 0.1346505.
BifurcationKit's tutorial prints the same four points at `q2 = 1.04100305, 1.04204852, 1.05220009,
1.05218722`, the Hopf points to its looser tolerance.

Two parameters:

| Point | On | (k₂, K) published | (k₂, K) recomputed | (x, y, s) recomputed | Coefficient published |
| --- | --- | --- | --- | --- | --- |
| Bogdanov–Takens | fold, Hopf | (1.417628, 0.971397) | (1.41762764, 0.97139757) | (0.11590872, 0.31546728, 0.28843700) | (a, b) = (−0.08378442, −2.136280) |
| Bogdanov–Takens | fold, Hopf | (1.161199, 0.722339) | (1.16119871, 0.72233907) | (0.01633704, 0.63840988, 0.20045593) | (a, b) = (−0.04822563, −1.937632) |
| Cusp | fold | (1.006408, 0.355991) | (1.00640833, 0.35599127) | (0.03594064, 0.35200543, 0.45137011) | c = 0.3627844 |
| Bautin | Hopf | (0.891319, 0.232487) | (0.89131898, 0.23248735) | (0.01802242, 0.36823794, 0.49796831) | ℓ₂ = −776.8996 |
| Bautin | Hopf | (0.924255, 0.305879) | (0.92425502, 0.30587927) | (0.06431133, 0.21109514, 0.55487023) | ℓ₂ = −240.1234 |

The cusp's state is the least well conditioned: MatCont prints `(0.035940, 0.352008, 0.451368)`.
The coefficients `(a, b)`, `c` and `ℓ₂` are not recomputed.

At `K = 2` the Hopf test changes sign twice on the equilibrium curve. Both points are neutral
saddles, with real eigenvalues `±κ`, and must not be reported as Hopf points:

| Point | k₂ published | k₂ recomputed | (x, y, s) recomputed | Eigenvalues recomputed |
| --- | --- | --- | --- | --- |
| Neutral saddle | 1.271108 | 1.2711072431 | (0.01963903, 0.72632119, 0.08467993) | ±0.141035, −10.805364 |
| Neutral saddle | 1.640254 | 1.6402536400 | (0.12869877, 0.38410555, 0.16239856) | ±0.126780, −10.833563 |

The first published value is one unit off in its sixth decimal: the recomputed `1.2711072431`
rounds to `1.271107`. The two-parameter Hopf curve from `K = 0.4` reaches `K = 2` at the same
`1.2711072431`.

## 2. Extended Lorenz-84 (flow, 4 equations)

Source: BifurcationKit.jl documentation, "Extended Lorenz-84 model". It adds the zero-Hopf and
Hopf–Hopf points, which need three and four dimensions.

```text
Ẋ = −Y² − Z² − αX + αF − γU²
Ẏ = XY − βXZ − Y + G
Ż = βXY + XZ − Z
U̇ = −δU + γUX + T
α = 1/4, β = 1, G = 0.25, δ = 1.04, γ = 0.987        free: F, T
```

One parameter, `T = 0.04`, from `F = 3` at
`(2.9787004394953343, −0.03868302503393752, 0.058232737694740085, −0.02105288273117459)`:

| Point | F published | F recomputed |
| --- | --- | --- |
| Hopf | 2.85996783 | 2.85987541 |
| Hopf | 2.46723305 | 2.46722212 |
| Hopf | 1.61975642 | 1.61965782 |
| Fold | 1.54664839 | 1.54664837, at (1.31178553, −0.04287500, 0.18038939, −0.15702758) |

Two parameters:

| Point | On | (F, T) published | Recomputed | Coefficient published |
| --- | --- | --- | --- | --- |
| Bogdanov–Takens | fold, Hopf | (1.4467165285, 0.0209401396) | (1.4467167, 0.02094017) at (1.2256405, −0.03632079, 0.19728832, −0.12339001) | a = 0.2144233, b = 0.6065142 |
| Zero-Hopf | fold | T = 0.00012644, F not printed | (1.2834192961, 0.0001265408) at (1.0598439, −0.01327681, 0.23513412, −0.02086092) | none |
| Bautin | Hopf | (2.3763590350, 0.0501975132) | (2.3763600882, 0.0501974320) at (1.15285369, −0.02825526, 0.2131069, −0.51291693), ω = 0.6903675 | ℓ₂ = 0.1557753, ω = 0.6903671 |
| Hopf–Hopf | Hopf | T = 0.02627340, F not printed | (2.5332211070, 0.0262739430) at (1.10143957, −0.02072803, 0.22506669, −0.55758627), ω = 1.151545, 0.743219 | none |

The fold curve is symmetric in `T`: the source lists the mirror points at `T = −0.00012655` and
`T = −0.02094041`. At the Bogdanov–Takens point the other two eigenvalues are `0.18549 ± 1.24506i`.

The recomputed values solve each point's conditions on the characteristic polynomial
`λ⁴ + p₁λ³ + p₂λ² + p₃λ + p₄` directly: zero-Hopf `p₄ = 0` and `p₃ = p₁p₂`; Hopf–Hopf `p₁ = p₃ = 0`;
Bautin the Hopf condition `p₁p₂p₃ − p₃² − p₁²p₄ = 0` with `ℓ₁ = 0` for the critical pair. The
zero-Hopf mirror is at `T = −0.0001265408`, symmetric to every digit, where the source's two values
differ in their fourth figure. The source's Hopf points, Bautin `F` and Hopf–Hopf `T` are off by
`10⁻⁴`, `10⁻⁶` and `5·10⁻⁷`, the same looseness.

## 3. Predator–prey map (2 equations)

Source: MatContM Tutorial M-II. The tutorial names the points found on each curve and the order
they appear in, but prints no numbers for the codimension-2 points, so those are derived.

```text
x ↦ ax(1 − x) − bxy/(1 + εx)
y ↦ dxy/(1 + εx)
b = 3, d = 3.5        free: ε, a
```

Fixed points: `F1 = (0, 0)`, `F2 = ((a − 1)/a, 0)` and
`F3 = (1/(d − ε), d(a(1 − x) − 1)/(b(d − ε)))`. Published: `F3 = (0.4000, 0.6813)` at `a = 4.1`,
`ε = 1`; branch points at `a = 1` and `a = 5/3` and a Neimark–Sacker point at `a = 5`, all at
`ε = 1`.

Curves of `F3` in closed form (derived):

| Curve | a(ε) |
| --- | --- |
| Neimark–Sacker (det = 1) | (2ε − 7)/(2ε − 3) |
| Flip | (4ε² − 56ε + 147)/(4ε² + 4ε − 7) |
| Branch points (F3 = F2, transcritical) | (2ε − 7)/(2ε − 5) |

Codimension-2 points on the Neimark–Sacker curve (derived):

| Point | ε | a | F3 |
| --- | --- | --- | --- |
| 1:4 resonance | 7/6 | 7 | (3/7, 3/2) |
| 1:3 resonance | 49/38 | 21/2 | (0.4523809524, 2.5069444444) |
| 1:2 resonance | 35/26 | 14 | (0.4642857143, 3.5208333333) |
| Chenciner | −4.6400549446 | 1.3257299842 | (0.1228492936, 0.0233424908), θ = 0.62560327 |

- Going forward from `ε = 1` the tutorial finds 1:4, 1:3, then 1:2, and going backward the
  Chenciner point, the same order as here. Past the 1:2 point the curve is a neutral saddle.
- The Neimark–Sacker coefficient is `d = −11.2197` at `ε = 1`, `a = 5`, with MatContM's scaling.
  The same routine gives `−1.0` for the delayed logistic map below, the value MatContM prints.
- On the branch-point curve F3 lies on F2 at `(1/(3.5 − ε), 0)`. Its multipliers are `1` and
  `(2ε − 3)/(2ε − 5)`. The line `y = 0` is invariant, so `[g_x | g_a]` and `[g_x | g_ε]` have rank 1
  there: a fold system is singular on this curve, and a branch-point system is needed.

The flip curve meets the branch-point curve at a fold-flip (derived). The tutorial reports a
fold-flip on its flip curve, but gives no location to compare.

| Point | ε | a | F3 | Multipliers |
| --- | --- | --- | --- | --- |
| Fold-flip (LPPD) | 2 | 3 | (2/3, 0) | 1, −1 |

`F2 = ((a − 1)/a, 0)` has the multipliers `2 − a` and `3.5x/(1 + εx)`. The second is −1 on the flip
curve `a = (ε + 7/2)/(ε + 9/2)`, which has two more codimension-2 points (derived):

| Point | ε | a | F2 |
| --- | --- | --- | --- |
| Generalized flip (GPD) | 2√2 − 9/2 = −1.6715728753 | 1 − √2/4 = 0.6464466094 | (−(2√2 + 1)/7, 0) = (−0.5469181607, 0) |
| 1:2 resonance | −5 | 3 | (2/3, 0) |

- The flip coefficient vanishes at the generalized flip, solved symbolically
  (`crates/dyn-analysis/tests/curves.rs`).
- At the 1:2 resonance the curve crosses `a = 3`, where `2 − a = −1` as well, and
  `D f = [[−1, 6/7], [0, −1]]` is a Jordan block.

### 3b. Delayed logistic map with migration

Source: MatContM Tutorial M-I.

```text
x ↦ rx(1 − y) + ε
y ↦ x
```

- Published: Neimark–Sacker at `r = 2`, `ε = 0`, with arguments `±60°` and coefficient `−1.0`
  (supercritical). At `ε = 0` a period-7 cycle exists between folds at `r = 2.174…` and
  `r = 2.201…`.
- Derived: the Neimark–Sacker curve is `x = 1/r`, `ε = 2/r − 1`, with `2 cos θ = r − 1`. Its 1:4
  resonance is at `(r, ε) = (1, 1)`; the weak resonances 1:5 and 1:7 are at
  `(1.6180339887, 0.2360679775)` and `(2.2469796037, −0.1099162642)`. The period-7 window at
  `ε = 0` is the tongue rooted at the 1:7 point.

## 4. Freire circuit (flow, 3 equations): Shilnikov homoclinic

Source: AUTO-07p, HomCont demo `cir` (`doc/auto.tex`, `demos/cir`). It has a homoclinic orbit to a
saddle-focus and a Bogdanov–Takens point on the same curve.

```text
ẋ = [−(β + ν)x + βy − a₃x³ + b₃(y − x)³] / r
ẏ = βx − (β + γ)y − z − b₃(y − x)³
ż = y
γ = 0, r = 0.6, a₃ = 0.328578, b₃ = 0.933578        free: ν, β        equilibrium: the origin
```

Points on the curve of homoclinic orbits. The origin has one unstable eigenvalue `λᵤ` and a stable
pair; the saddle quantity is `σ = λᵤ + Re λₛ`, and Shilnikov's chaos needs `σ > 0`.

| Point | (ν, β) published | Eigenvalues at the origin (derived) | σ (derived) |
| --- | --- | --- | --- |
| Start of the demo | (−0.7213093, 0.6) | 0.36379, −0.38081 ± 0.64090i | −0.01701 |
| Neutral saddle-focus | (−0.7204001, 0.5912315) | 0.37588, −0.37592 ± 0.65682i | −0.00003 |
| Neutrally-divergent saddle-focus | (−0.7256925, 0.4535645) | 0.57767, −0.28884 ± 0.83768i | 0.28883 |
| Shilnikov–Hopf | (−1.026445, −0.00002330391) | 1.71078, 0.00001 ± 1.00000i | does not apply |
| Double real leading eigenvalue | (−0.7590583, 0.7428734) | 0.14544, −0.42769, −0.43365 | does not apply |
| Neutral saddle | (−0.7746686, 0.7746147) | 0.01077, −0.01077, −0.77453 | does not apply |

- **Which side to test.** The demo's start point has `σ < 0`: a saddle-focus homoclinic without
  chaos. `σ` changes sign at the neutral saddle-focus, so points with `β < 0.5912` are the
  Shilnikov case.
- **Bogdanov–Takens (derived).** The Jacobian at the origin has determinant `−(β + ν)/r` and, on
  `ν = −β`, second coefficient `1 − β²/r`, so the double zero is at `ν = −β`, `β = √r`:
  `(ν, β) = (−0.7745967, 0.7745967)`. AUTO's run of the homoclinic curve ends beside it at
  `(−0.7746628, 0.7746453)`.
- **Shooting check (derived).** Following the unstable manifold and bisecting on `ν` until it
  returns to the origin gives `ν = −0.72131951` at `β = 0.6` and `ν = −0.72568945` at
  `β = 0.4535645`. AUTO has `−0.7213093` and `−0.7256925`.
- **Precision.** Use five digits. AUTO's demo truncates the orbit to a finite interval on a coarse
  mesh, and the shooting check takes the stable manifold as flat within 0.05 of the origin.
- At `β = 0.4535645` the shooting also finds a second homoclinic orbit at `ν = −0.72695630`, with
  about twice the return time. Such subsidiary orbits are expected where `σ > 0`.

## Not used

- **FitzHugh–Nagumo travelling waves** (AUTO demo `fnb`, MatCont's homoclinic tutorial): AUTO and
  MatCont agree on a double-real-eigenvalue point near `(a, c) = (0.13181, 0.21716)`. A second
  Shilnikov case if one is needed; not recomputed here, and it has no Bogdanov–Takens point.
- **Rössler:** the published homoclinic values have two or three digits.
- **Cubic Chua** (Algaba et al. 1999): the linearisation at the printed "Shilnikov" parameters has
  real eigenvalues.

## Sources

- MatCont Tutorial IV: <https://webspace.science.uu.nl/~kouzn101/NBA/LAB4.pdf>
- MatContM Tutorial M-I: <https://webspace.science.uu.nl/~kouzn101/NBA/TutorialMIApr2018.pdf>
- MatContM Tutorial M-II: <https://webspace.science.uu.nl/~kouzn101/NBA/TutorialMIIMay2018.pdf>
- BifurcationKit, CO oxidation:
  <https://bifurcationkit.github.io/BifurcationKitDocs.jl/dev/tutorials/ode/tutorialCO>
- BifurcationKit, extended Lorenz-84:
  <https://bifurcationkit.github.io/BifurcationKitDocs.jl/dev/tutorials/ode/lorenz84>
- AUTO-07p: <https://github.com/auto-07p/auto-07p>
- MatCont homoclinic tutorial: <https://webspace.science.uu.nl/~kouzn101/NBA/LAB6.pdf>
