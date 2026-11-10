# Parameter plane

The Parameter plane follows a bifurcation as two parameters change. The Branch view's parameter
runs across, a second parameter runs up, and each fold, Hopf, branch point, flip or
Neimark–Sacker point found in the Branch view becomes a curve. Choose **Parameter plane** in the
**View** panel. It is offered for a system with two parameters or more whose equations don't
contain `t` or `n`.

The desktop app follows the curves of any such system and marks the points where they meet. The
browser preview draws the curves of the two gallery templates below only.

Example project: [parameter-plane.sepx](examples/parameter-plane.sepx), CO oxidation after
**Search in two parameters**, with k₂ across and K up. Its fold and Hopf curves meet at the two
Bogdanov–Takens points, the fold curve has a cusp, and the Hopf curve two Bautin points.

## Reach it

- In the **Branch** view, click **Follow in two parameters…** on an LP, H, BP, PD or NS row, pick
  the parameter to free in **Free also**, and click **Follow**.
- Or click **Search in two parameters** in the **Special points** header, which follows every
  such point on the branches.
- Or choose **Parameter plane** in the View control. With no curve yet, it starts from the Branch
  view's special points. With none, it says so and offers **Go to the Branch view**.

## Read the plot

Each kind of curve has its own colour and pattern: the fold curve is solid, the Hopf and
Neimark–Sacker curves dashed, the flip curve dotted dashes, and the branch-point curve, **Branch
point (BP)** in the legend, a dash and two dots. On a branch-point curve another branch of fixed
points crosses, as at a transcritical or pitchfork bifurcation. A faint dotted stretch is no
bifurcation at all (a neutral saddle). Points where curves meet or change character are marked:

| Label | Point |
| --- | --- |
| BT | Bogdanov–Takens, where a fold curve and a Hopf curve meet |
| CP | Cusp, where two fold curves meet |
| GH | Bautin, where a Hopf point changes from subcritical to supercritical |
| ZH | Zero-Hopf, an eigenvalue 0 beside a pair ±iω, where a fold curve crosses a Hopf curve |
| HH | Hopf–Hopf, two pairs ±iω₁ and ±iω₂, where two Hopf curves cross |
| R1 to R4 | Strong resonances on a Neimark–Sacker curve, R1 and R2 also on fold and flip curves |
| CH | Chenciner, where a Neimark–Sacker point changes criticality |
| LPPD | Fold-flip, where a flip curve meets a fold or branch-point curve |
| GPD | Generalized flip, where a flip changes from subcritical to supercritical |

The crosshair is the current pair of parameter values, and the grey bar is the Branch view's range
at the value of the parameter up that its branches were computed at. Click anywhere to set both
parameters; the sliders and every other view follow. Click a row in **Special points** to set both
parameters there.

Each row of **Special points** says in a sentence what happens at the point, using only what holds
where it was found. A fold-flip found on a branch-point curve says there is no fold there, and a
Bautin or Chenciner row names the side of the point where the bifurcation is subcritical, as the
plot shows it: above, below, left or right. A Bogdanov–Takens row gives its normal-form
coefficients `a` and `b` and the sign of `ab`, and a cusp row its coefficient `c`. Only the sign of
`ab` is fixed: the row takes `a` positive. A row on a Neimark–Sacker curve gives the angle θ of the
multipliers. In the desktop app each row also says how finely the point was located: "located to
1.5 × 10⁻¹³" is the width of the last bisection step along the curve, each parameter as a fraction
of its range.

On a Neimark–Sacker curve, short ticks mark the weak resonances 1:5 to 1:9, where the multipliers
turn by a fraction 1/q of a full turn. Each is the tip of a resonance tongue, where a period-q cycle
appears.

## The inset

A small plot in the emptiest corner shows the system at the crosshair. For a flow it is the Phase
view as you last left it: its two states, its range and its start points, or five starts spread
over the view if it has none, with the fixed points. For a map it is the orbit from each start
point after at least 1,000 iterates. The caption under it gives the two parameters and what is
there: "3 fixed points in view: 2 stable, 1 saddle." For a map it adds how the orbits end: "The
orbit settles on a period-4 cycle." or "The orbit does not repeat within 3,000 iterates." While a
new inset is computing, the last one stays, faded. Systems with one state have no inset.

## Homoclinic curves

A homoclinic orbit leaves an equilibrium and comes back to it. In a flow, a curve of them starts
at every Bogdanov–Takens point. **Follow homoclinic** on a BT row of **Special points** finds the
orbit beside the point and follows the curve, drawn with a dash and a dot and listed as
**Homoclinic (Hom)** in the legend. Each point is found by shooting along the saddle's
one-dimensional unstable manifold (or its stable manifold, in reversed time), so a curve takes
seconds, not milliseconds. Its row counts the points as they arrive, and **Cancel** stops the
run and keeps nothing of it.

The row says how the curve ends: one end shrinks onto the saddle at the BT point. It also says
where the saddle quantity σ, the sum of the real parts of the leading unstable and stable
eigenvalues, changes sign, and where the orbit is a Shilnikov orbit with chaos nearby: the saddle
is a saddle-focus and its real eigenvalue outweighs the complex pair, `σ > 0` when the unstable
manifold is the one-dimensional one.

The browser preview has the homoclinic curves of the CO oxidation template only.

## Try it

- **CO oxidation** (Bykov, Yablonskii and Kim; MatCont Tutorial IV): the Branch view over k₂ finds
  H, LP, LP, H. Followed in K, the fold and Hopf curves meet at two Bogdanov–Takens points, the
  fold curve has a cusp, and the Hopf curve two Bautin points.
- **Predator–prey map** (MatContM Tutorial M-II): the Neimark–Sacker point at a = 5, followed in
  ε, passes the 1:4, 1:3 and 1:2 resonances and a Chenciner point. The branch point at a = 5/3 is
  where the coexistence fixed point meets the one without predators. Followed in ε, its curve
  passes a fold-flip at (a, ε) = (3, 2), where the predator-free fixed point flips. That flip
  happens at a = 3 whatever ε is, so the plane does not follow it in ε: it says ε doesn't move it.
  Its other flip, at a = 9/11, followed in ε, passes a generalized flip at
  (a, ε) = (0.646447, −1.671573).
- **Freire circuit** (AUTO-07p's HomCont demo `cir`): type
  `x' = (-(beta + nu)*x + beta*y - 0.328578*x^3 + 0.933578*(y - x)^3)/0.6`,
  `y' = beta*x - beta*y - z - 0.933578*(y - x)^3` and `z' = y`, set β to 0.8, give ν's slider the
  range −1.2 to 0, and open the Branch view over ν. It finds a branch point at ν = −0.8 and a Hopf point at ν = −0.4173. Followed in
  β, the Hopf curve passes a Bogdanov–Takens point at (ν, β) = (−0.774597, 0.774597). **Follow homoclinic** there draws the
  curve of homoclinic orbits to the origin. σ changes sign near β = 0.5912: below it the orbit is
  a Shilnikov orbit.

## Check the numbers

The codimension-2 gate follows these curves as the plane does and checks every point against the
reference values. Two examples from its output:

- CO oxidation's first Bogdanov–Takens point is at (k₂, K) = (1.417628, 0.971398) on the fold
  curve, and the Hopf curve finds the same point to within 2 × 10⁻¹³.
- The predator–prey map's Neimark–Sacker curve passes the 1:4, 1:3 and 1:2 resonances at
  a = 7, 10.5 and 14, where ε = 7/6, 49/38 and 35/26, each to within 10⁻¹².
- The Freire circuit's homoclinic orbit is at ν = −0.721309 for β = 0.6 and at ν = −0.725693
  for β = 0.4535645, where AUTO-07p has −0.7213093 and −0.7256925. σ is −0.0170 at the first and
  +0.2888 at the second, the Shilnikov case.

## Save and open

**Save** while the plane is shown writes it into the project file: the two parameters, the box,
Δs and the most points, the points each curve was followed from, the Bogdanov–Takens points each
homoclinic curve was followed from, and the Branch view it came from, with the parameter values
its branches were computed at. Opening the file follows every curve again, so it shows the same
curves, special points and slice. **Branch** then shows the Branch view the plane came from.

A curve that no longer fits the system, because you changed the equations, is left out and the
open banner says which. A plane whose parameter is gone opens as its Branch view, or as the
default view. Separatrix 1.0.1 and older open a saved plane in the default view.

## Export

**Export…** writes the plane as a pgfplots figure in the curves' colours and patterns, with the
special points and the resonance ticks, and the special points as a booktabs table with their
coefficients. It also writes the curves as CSV, with the special points in a second file, and the
plot as PNG or SVG. The crosshair and the inset are left out. See
[Export figures and data](exports.md).
