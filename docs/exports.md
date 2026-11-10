# Exports

LaTeX is the primary export: one action copies a snippet that compiles in the
student's own document, in Strogatz's notation. PNG and SVG come from the UI's
own drawing code (`drawScene`); everything else is built in Rust by the crate
`crates/export` (`separatrix-export`), which has no Tauri dependency. The app
converts its IPC types into `separatrix_export::model` and writes the returned
text and files.

| Export | Rust | Notes |
| --- | --- | --- |
| LaTeX system | `latex_system::system` | `align` (numbered, with labels) or `align*` |
| LaTeX fixed points | `latex_fixed_points::table` | booktabs `table` float |
| LaTeX figure | `latex_figure::figure` | pgfplots; inline data (Copy) or `.dat` files (Save) |
| CSV + `.meta.json` | `csv::files` | one pair per initial condition |
| LaTeX Lyapunov spectrum | `latex_lyapunov::lyapunov_latex` | booktabs `table`, siunitx `\num{0.906 +- 0.004}` |
| Lyapunov CSV + `.meta.json` | `csv::lyapunov` | `t,lambda_1,…,lambda_n`, the running estimates |
| LaTeX orbit diagram | `latex_figure::orbit_figure` | pgfplots; dots up to 5,000, then a density PNG (Save) or thinned (Copy) |
| Orbit CSV + `.meta.json` | `csv::orbit` | `r,n,x`, one row per finite kept iterate |
| LaTeX branch plot | `latex_continuation::continuation_figure` | pgfplots; stable runs solid, unstable dashed, special points as glyphs with labels |
| LaTeX special points | `latex_continuation::special_points_table` | booktabs `table`, siunitx `\num{0.346410}` |
| Continuation CSV + `.meta.json` | `csv::continuation` | `branch,period,point,<param>,<states>,stable,unstable`, plus `<stem>-special.csv` |
| LaTeX parameter plane | `latex_curves::curves_figure` | pgfplots; each curve in its kind's colour and dash pattern, neutral stretches dotted, codimension-2 points as glyphs with labels, 1:q ticks |
| LaTeX codimension-2 points | `latex_curves::plane_points_table` | booktabs `table`, each point once with the curves it lies on and its coefficients |
| Parameter plane CSV + `.meta.json` | `csv::curves` | `curve,kind,period,point,<across>,<up>,<states>,neutral`, plus `<stem>-special.csv` |
| Export portrait | `simplify::simplify_portrait` | cut at the first non-finite point, RDP at 0.25 px (the graph of f in a cobweb at 0.0625 px, so it shows no facets when zoomed); a section's crossings and a 3D view's trajectories are only cut |
| Clipboard PNG | `raster::decode_rgba` | for `Image::new_owned(rgba, w, h)` |

Every LaTeX snippet starts with comment lines: the title, the equations as
plain text (`convert` to text), the parameter values, and a `% Needs:` line
with the packages it uses. Nothing in a `.tex` output is U+2212 (TeX's default
input encoding can't read it); the real minus sign stays in SVG and PNG.

## 3D view

A 3D view (the request's `zAxis`) exports as a picture of the box seen from
the studio's camera. PNG draws the trajectories with three.js offscreen (a
`WebGLRenderer` with `preserveDrawingBuffer` at `w·scale × h·scale` px),
copies them into the 2D canvas and draws the box, ticks, axis names and fixed
points over them at the same scale (`renderSpaceFigure`); without WebGL it
draws the lines in 2D through the same projection. The PNG keeps its pHYs at
`96·scale` dpi. SVG is the same drawing projected to 2D (`drawSpaceLines` and
`drawSpaceOverlay` on the SVG recorder), with one path per trajectory. Where
the figure is too small for the axis names (they sit a fixed number of px
outside the box), the camera is zoomed out until they and the end tick labels
are inside. The export portrait cuts a 3D view's trajectories but does not
simplify them: an x–y simplification can't know which samples the turned box
needs. The LaTeX figure is refused: the dialog disables it with the reason,
and `export_figure_latex` returns the same sentence for a request with
`zAxis`, so no path draws an x–y plane instead:

```
LaTeX figures of the 3D view aren't available yet. Export PNG or SVG, or switch to a phase plane.
```

The LaTeX equations and fixed points, and the CSV (every state of each
trajectory), are the same as in any other view of the flow.

## Lyapunov spectrum

The Lyapunov view (Strogatz §9.3, §10.5) exports its own run. The UI sends
the view's `LyapunovRequest` (`export_lyapunov_latex`,
`export_lyapunov_csv`); the core measures it again, so the export is the
core's numbers, and keeps the last run so the table, the CSV and their Save
share one measurement. Its formats:

| Format | What |
| --- | --- |
| LaTeX | Spectrum: the table below. Figure is disabled with the reason (the running estimates export as PNG or SVG); Fixed points is disabled (the view shows none); Equations is as in any view. |
| CSV | the running estimates, one row per sampled renormalization |
| PNG, SVG | the view as drawn (`drawLyapunov`): grid, axes, one curve per exponent in `traj-i`, the final values dashed, λ₁ … λₙ, the names `t` (or `n`) and `λ` |

The table is a `table` float with `\sisetup{uncertainty-mode = separate}`
inside it (siunitx 3 prints `0.906 ± 0.004`, never `0.906(4)`), one row per
exponent, `$\lambda_{1}$ & \num{0.906 +- 0.004} \\`, then `Σλ`, `⟨tr J⟩`
(a map: `⟨ln |det J|⟩`) and their difference, the integrator's error on the
tangent vectors:

```latex
\begin{table}
\centering
\sisetup{uncertainty-mode = separate}
\caption{Lyapunov spectrum of the Lorenz equations with $\sigma = 10$, $r = 28$ and $b = 2.66667$, from $(x, y, z) = (1, 1, 1)$: Runge--Kutta 4 with step $h = 0.01$, total time $T = 1000$ after a transient of $100$, \num{10000} renormalizations every $\tau = 0.1$. Uncertainties are standard errors over 10 blocks.}
\label{tab:lorenz-equations-lyapunov}
\begin{tabular}{@{}ll@{}}
\toprule
Exponent & Value \\
\midrule
$\lambda_{1}$ & \num{0.906 +- 0.004} \\
…
$\sum_i \lambda_i$ & \num{-13.6666} \\
$\langle \operatorname{tr} J \rangle$ & \num{-13.6667} \\
Difference & \num{6.7e-5} \\
\bottomrule
\end{tabular}
\end{table}
```

Rounding. The error is rounded half up to one significant digit, or two
when that digit is 1, and the value to the same decimal place:
0.90561 ± 0.00437 is `\num{0.906 +- 0.004}`, 0.4192 ± 0.0012 is
`\num{0.4192 +- 0.0012}`, and 0.0096 rounds to one digit one place up,
`0.01`. Both are rounded from the exact decimal value of the double
(`num::with_uncertainty`), never through `log10`, so the text is the same on
every platform. A run with fewer than two blocks has no errors: the
exponents get six significant digits and the caption says so.

The caption names the system and its parameters, the start point, the
integrator with its step (`step $h = 0.01$`) or its tolerances (an adaptive
method), the total time `T` and the transient, and how many renormalizations,
how far apart (`every $\tau = 0.1$`). A map's caption says it was iterated
`N` times after the transient, with renormalizations every iterate (or every
k). A run that stopped early says why.

The CSV is `t,lambda_1,…,lambda_n` (`n,…` for a map), the history the view
plots (at most 2,000 rows, the last the final estimate), cut at the first row
that is not finite. `<title>-lyapunov.meta.json`:

```json
{
  "format": "separatrix-lyapunov",
  "format_version": 1,
  "app_version": "0.2.0",
  "title": "Lorenz equations",
  "csv": "lorenz-equations-lyapunov.csv",
  "system": { "kind": "ode", "input_format": "latex", "lines": ["…"] },
  "params": [{ "name": "sigma", "value": 10.0 }, …],
  "columns": ["t", "lambda_1", "lambda_2", "lambda_3"],
  "x0": [1.0, 1.0, 1.0],
  "t0": 0.0,
  "transient": 100.0,
  "total": 1000.0,
  "interval": 0.1,
  "blocks": 10,
  "integrator": { "method": "rk4", "h": 0.01 },
  "renormalizations": 10000,
  "exponents": [0.90561, -0.00025, -14.57196],
  "uncertainty": [0.00437, 0.00026, 0.001],
  "sum": -13.6666,
  "trace_average": -13.666666666666666,
  "status": "completed",
  "notes": ["…"]
}
```

`integrator` is `{ "method", "h" }` for a fixed step, `{ "method", "rtol",
"atol" }` for an adaptive method, and absent for a map. Save writes the CSV
and its metadata through `save_csv_file`, which takes the file the export
command returned.

## Orbit diagram

The orbit diagram (Strogatz §10.2) exports the view's iterates. A figure asks
for its own diagram over the view's range, one column per pixel of its image
(the column at each pixel's center, as on screen): the PNG's pixels at its
scale, 2× for the image an SVG embeds, and 300 dpi for a LaTeX figure's
layer. The CSV is the screen's own diagram.

| Format | What |
| --- | --- |
| LaTeX | Figure: the pgfplots figure below. Fixed points is disabled (the view shows none); Equations is as in any view. |
| CSV | `r,n,x`, every finite kept iterate |
| PNG | the view as drawn (`drawOrbit`, without the screen's parameter marker): the density binned per device pixel, log-scaled opacity in `plot-fp`, over the grid and axes |
| SVG | vector grid, axes, ticks and names, with the density as one embedded `data:image/png` `<image>` (`image-rendering: pixelated`), which the CSP's `img-src data:` allows |

The LaTeX figure has the screen's frame (grid, axes, tick labels and the
names, the parameter's LaTeX across and the state's up) and one dot per
finite kept iterate in `plot-fp`, sized as map iterates are. Its header names
the sweep (`% Orbit diagram: r from 2.8 to 3.7 in 10 values; x after the
first 100 iterates, 4 kept for each.`) and leaves the swept parameter out of
the `Parameters:` line. Up to 5,000 dots are data (inline for Copy, a
`<base>-orbit.dat` for Save). Above that:

- Save writes `<base>-points.png` (`raster::density_png`): one pixel column
  per parameter value, as many rows as keep the plot box's aspect, transparent
  where no iterate falls and `plot-fp` from 30 % opaque for one iterate to
  fully opaque for the most visited, log-scaled, with `pHYs` set to the box's
  width. The figure draws it with `\addplot graphics[xmin=…, xmax=…, ymin=…,
  ymax=…] {\sepxdir <base>-points.png};` under vector axes, and no dots.
- Copy keeps one iterate in `k`, `k = ⌈points / 5,000⌉` (at least 2), with
  `thinned: true`, a note and a `% Thinned:` comment.

`save_orbit_latex` writes the `.tex` and its one data file.

The CSV is one row per finite kept iterate, parameter value by parameter
value: the swept parameter's name, `n` and the state's name (`r,n,x` for the
logistic map). `n` counts from the start point, so a column's first row is
`n = T`, the transient, as in a map's trajectory CSV. Iterates of an orbit
that escaped are left out, and so is the rest of that orbit.
`<title>-orbit-diagram.meta.json` has `format: "separatrix-orbit-diagram"`,
the system, the parameters, `columns`, `swept`, `range` (the first and last
`r`), `parameter_values`, `state`, `x0`, `transient`, `kept`, `rows` and notes.

## Continuation

The Branch view (a continuation view, Strogatz §3.1–3.4 and §8.2) exports its
branches. The LaTeX and the CSV come from the core: `export_continuation_latex`,
`save_continuation_latex` and `export_continuation_csv` take the view's own
`ContinuationRequest`s, one per branch with its palette slot
(`ContinuationExportRequest`), and follow them again. The last run is cached by
those requests (their `requestId`s aside), so the figure, the table, the CSV
and their Save share one. A branch the core can't follow is left out, as the
plot leaves it out; with no branch left the export fails with that branch's
reason, and with no branch at all with "There are no branches to export yet."
PNG and SVG draw the screen's own branches through `drawBranch`.

| Format | What |
| --- | --- |
| LaTeX | Figure: the pgfplots figure below. Special points: the booktabs table below. Fixed points is disabled (the view shows none); Equations is as in any view. |
| CSV | the branches' points, `<stem>.meta.json`, and the special points in `<stem>-special.csv` |
| PNG, SVG | the view as drawn (`drawBranch`, without the screen's parameter marker), with the axis names |

The figure has the screen's frame, the parameter's LaTeX across and the
state's (or `\lVert x \rVert`) up. Each branch is two `\addplot`s in
`traj-(color mod 7 + 1)`, 1.75 px wide: its stable stretches solid and its
unstable ones dashed (`dash pattern=on 4.5bp off 3bp`, the screen's 6 px on, 4
px off), every point of a cycle drawn, the stretches split at every special
point and `nan` rows between them. Each has a comment, `% Branch 2 (period 2),
unstable (dashed)`. The special points come last: a half-filled fixed-point
glyph (`halfcircle*`, `plotmarks`) on a halo at every point of the cycle, and
the label (`LP`, `H`, `BP`, `PD`, `NS`) once, 7 px right of and 5 px above the
first, in `plot-fp` on the label halo. Curves are clipped to the view grown by
8 px. The header names the sweep (`% Branch plot: y against b from 0 to 1.5; 1
branch, 2 special points.`) and leaves the swept parameter out of the
`Parameters:` line. Data is inline for Copy; Save writes
`<base>-branch-<k>-stable.dat`, `<base>-branch-<k>-unstable.dat` and
`<base>-special-points.dat` beside the `.tex`. Above 5,000 rows a comment
suggests lualatex; nothing is thinned.

The special points table lists each special point once, in increasing
parameter (`ContinuationRun::special_points`: both halves of a switched cycle,
and two branches through one branch point, report the same one; the same kind
and period within `1e-7·(1 + |p|)` is one row, as the inspector lists them).
Columns: Type (the label), the parameter, the state (a cycle's first point),
Period for a map, `ω` (Hopf) or `θ = arg μ` (Neimark–Sacker), and `ℓ₁`. A
column no row fills is left out. The parameter and ω/θ are
`\num{<num::to_fixed(v, 6)>}` (`\num{0.346410}`), states and ℓ₁ six
significant digits (`num::si`), and a state below `1e-12` of the column's
largest prints as 0. The caption names the system, the swept parameter, the
other parameters' values and a legend of the types present, and says what ω,
θ and ℓ₁ are (ℓ₁ with `⟨p, q⟩ = 1`, `⟨q, q⟩ = 1/2`, so `ṙ = μr + ar³` has
`ℓ₁ = a/ω`). Label `tab:<title-slug>-special-points`.

The CSV has one row per point of each branch in arclength order: `branch`
(from 1), `period`, `point` (from 0), the swept parameter's name, every state
by name, `stable` (`true` or `false`) and `unstable` (the count). A point of a
k-cycle of a map is k rows, `x, f(x), …, f^(k−1)(x)`. `<stem>-special.csv` has
`branch,point,type,period,<param>,<states>,omega,lyapunov`, one row per special
point as each branch reports it (not merged), `omega` and `lyapunov` empty
where the kind has none. `<stem>.meta.json` has `format:
"separatrix-continuation"`, `csv`, `special_csv`, the system, the sliders'
parameters, `columns`, `special_columns`, `swept`, `range`, one entry per
branch (`branch`, `period`, `points`, `rows`, `special`) and notes. The stem
is `<title-slug>-continuation`; Save writes `<name>.csv`, `<name>-special.csv`
and `<name>.meta.json` and rewrites `csv` and `special_csv` to the names
written.

Every number goes through `num::shortest`, `num::si` or `num::to_fixed`,
which round the exact decimal value of the double, and the norm is
`sqrt(Σ xᵢ²)` (`+ ×` and a correctly rounded `sqrt`). No platform `libm`
function decides a printed digit or a kept point. The goldens are built from
literal data: the Sel'kov branch at `a = 0.08` with the core's Hopf points,
and the logistic cascade's special points.

## Parameter plane

The Parameter plane (curves of bifurcations in two parameters) exports its
curves. The LaTeX and the CSV come from the core: `export_curves_latex`,
`save_curves_latex` and `export_curves_csv` take the view's own
`CurveRequest`s, one per curve the plane draws (a row merged into an earlier
curve, or one that failed, draws nothing and is not sent), in a
`CurvesExportRequest`, and follow them again. The last run is cached by those
requests (their `requestId`s aside), so the figure, the table, the CSV and
their Save share one. A curve the core can't follow is left out; with none
left the export fails with that curve's reason, and with no curve at all
with `CURVES_EXPORT_EMPTY` (a generated constant, so the browser preview says
the same): "There are no curves to export yet. Follow a fold, Hopf, flip or
Neimark–Sacker point of the Branch view in two parameters first." A
homoclinic curve is a job that takes seconds to minutes and can be
cancelled, so the request carries its points as the view drew them
(`HomoclinicExport { p1, p2, x }`) instead of following it again; the shell
checks that their lengths fit. The Branch view's range, when the plane draws
it, comes as `slice: { range, at }`. PNG and SVG draw the screen's own curves
through `drawCurves`.

| Format | What |
| --- | --- |
| LaTeX | Figure: the pgfplots figure below. Special points: the booktabs table of the codimension-2 points below. Fixed points is disabled (the view shows none); Equations is as in any view. |
| CSV | the curves' points, `<stem>.meta.json`, and the codimension-2 points in `<stem>-special.csv` |
| PNG, SVG | the view as drawn (`drawCurves`, without the crosshair and the live inset), in the export palette's `curve-*` colours, with the axis names |

The figure has the screen's frame, the across parameter's LaTeX across and
the up parameter's up. In z-order:

- The Branch view's range (`% The Branch view's range, at K = 0.4`), when the
  plane draws it: a line in `plot-tick`, 0.75 bp, at the up value the
  branches were computed at, with a 3 px bar at each end that lies in view.
- Each curve, in kind order (fold, Hopf, branch point, flip, Neimark–Sacker,
  then the homoclinic curves) and numbered as the CSV numbers them, as one
  group of `\addplot`s under `% Curve 2: Hopf (H), dashed`: its neutral-saddle
  stretches first (a segment is neutral when both its ends are, as
  `segmentRuns` splits it on screen), in its colour at 0.55 opacity, 1.125 bp
  wide, round dots `dash pattern=on 0bp off 2.6bp`; then the rest, 1.3125 bp
  (1.75 px), butt caps, in its kind's pattern.
- The weak resonances 1:q of each Neimark–Sacker curve: a 14 px stroke across
  the curve at its nearest point on screen (`nearestSegment`, the normal
  turned down the screen) in `curve-ns`, 0.75 bp, and the label `$1{:}5$` in
  `plot-tick` on the label halo 3 px beyond it.
- Each codimension-2 point once (merged across curves as below): a
  half-filled glyph on a halo, and its label (`BT`, `CP`, `GH`, `ZH`, `HH`,
  `R1` to `R4`, `CH`, `LPPD`, `GPD`) 7 px right of and 5 px above it in
  `plot-fp`.

| Kind | Colour | Pattern (bp) |
| --- | --- | --- |
| Fold (LP) | `curve-fold` | solid |
| Hopf (H) | `curve-hopf` | `on 6bp off 2.25bp` |
| Branch point (BP) | `curve-bp` | `on 6bp off 1.875bp on 1.125bp off 1.875bp on 1.125bp off 1.875bp` |
| Flip (PD) | `curve-flip` | `on 2.25bp off 2.25bp` |
| Neimark–Sacker (NS) | `curve-ns` | `on 6bp off 2.25bp` |
| Homoclinic (Hom) | `curve-hom` | `on 6bp off 2.25bp on 1.125bp off 2.25bp` |
| Neutral stretch | its kind's, 0.55 opacity | `on 0bp off 2.6bp` |

The patterns are the design system's (PhasePlane README, Parameter plane):
the screen's patterns in units of the line scale at 0.75 bp per px. The
README gives none for homoclinic curves; theirs is derived the same way from
the screen's `8, 3, 1.5, 3 × s` (`CURVE_STYLES` in `app/src/canvas/curves.ts`),
which gives `6, 2.25, 1.125, 2.25`. The slice and the ticks are `\addplot
coordinates` in data units, so they keep the screen's z-order; only the
labels are nodes. The labels always sit right of and above their point,
where the screen tries four sides. There is no legend inside the figure, as
in the other exports; the header has one in a comment line instead:

```
% Parameter plane: K against k_2, followed in k_2 from 0.5 to 2.5 and K from 0 to 2; 3 curves, 5 codimension-2 points.
% Legend: LP fold (solid); H Hopf (dashed); Hom homoclinic (dash-dotted); dotted where a curve is a neutral saddle, no bifurcation; the bar is the Branch view's range.
```

The `Parameters:` line leaves both free parameters out. Curves are clipped to
the view grown by 8 px. Data is inline for Copy; Save writes
`<base>-curve-<k>.dat`, `<base>-curve-<k>-neutral.dat` and
`<base>-special-points.dat` beside the `.tex`. Above 5,000 rows a comment
suggests lualatex; nothing is thinned.

The codimension-2 points table lists each point once
(`PlaneRun::special_points`, the inspector's `planePointRows`: the same kind
and period with each parameter within `1e-6` of its range of the other's is
one row, naming every curve it was found on), by kind and then across.
Columns: Type (the label), both parameters, the state (a cycle's first point),
Period for a map, Curves (the labels of the curves it lies on, `LP, H`), and
the coefficients the core computed: `a` and `b` (Bogdanov–Takens), `c` (the
cusp), `θ` (on a Neimark–Sacker curve). A column no row fills is left out,
and a row without a value has `--`. The parameters and θ are
`\num{<num::to_fixed(v, 6)>}`, states and coefficients six significant digits
(`num::si`), and a state below `1e-12` of the column's largest prints as 0.
The rows' sentences (what each point means) are not written: they are
paragraphs that read the screen's sides ("above", "below"), not table cells.
The caption names the system, the plane, the other parameters' values, a
legend of the types present and of the curves' labels, and says what `a`,
`b`, `c` and `θ` are (`ẇ₁ = β₁ + β₂w₀ + a w₀² + b w₀w₁` with `a ≥ 0`;
`ẇ = β₁ + β₂w + c w³` with `‖q‖ = 1`; the angle of the critical multipliers
`e^{±iθ}`). Label `tab:<title-slug>-plane-points`; Save suggests
`<stem>-plane-points.tex`.

```latex
Type & $k_{2}$ & $K$ & $(x, y, s)$ & Curves & $a$ & $b$ & $c$ \\
\midrule
BT & \num{1.161199} & \num{0.722339} & $(\num{0.016337}, \num{0.63841}, \num{0.200456})$ & LP, H & \num{0.0482256} & \num{1.93763} & -- \\
CP & \num{1.006408} & \num{0.355991} & $(\num{0.0359406}, \num{0.352005}, \num{0.45137})$ & LP & -- & -- & \num{0.362789} \\
```

The CSV has one row per point of each curve in arclength order: `curve`
(from 1, the bifurcation curves in the view's order, then the homoclinic
curves), `kind` (`LP`, `H`, `BP`, `PD`, `NS`, `Hom`), `period`, `point` (from
0), the across and up parameters' names, every state by name, and `neutral`
(`true` at a neutral saddle; always `false` on a homoclinic curve).
`<stem>-special.csv` has `curve,point,type,period,<across>,<up>,<states>,a,b,c,theta`,
one row per codimension-2 point as each curve reports it (not merged), the
coefficients empty where the point has none. `<stem>.meta.json` has `format:
"separatrix-curves"`, `csv`, `special_csv`, the system, the sliders'
parameters, `columns`, `special_columns`, `across`, `up`, `ranges` (the box),
one entry per curve (`curve`, `kind`, `period`, `points`, `special`, and
`ticks`, the weak resonances `{ q, p }`) and notes. The stem is
`<title-slug>-curves`; Save writes `<name>.csv`, `<name>-special.csv` and
`<name>.meta.json` and rewrites `csv` and `special_csv` to the names written.

Numbers go through `num::shortest`, `num::si` and `num::to_fixed`, and the
tick's normal uses `sqrt` (no `hypot`), so no platform `libm` function decides
a printed digit. The goldens are built from literal data, a few curves of the
reference script's output (`tests/common/plane.rs`): CO oxidation's fold
curve (a cusp and two Bogdanov–Takens points), its Hopf curve (both BT, two
Bautin points, neutral past BT) and a homoclinic curve, with the Branch
view's range; and the predator–prey map's Neimark–Sacker curve (R2, R3, R4,
CH and the ticks 1:5 to 1:9), a flip curve and the branch-point curve, which
meet at one fold-flip point.

## The system

- The right-hand sides are the user's own LaTeX, split by `dyn_expr::outline`
  (which reuses the compiler's analysis for each line's role) and made
  portable by `dyn_expr::portable_latex`: `\mleft`/`\mright` → `\left`/`\right`,
  `\exponentialE` → `\mathrm{e}`, `\differentialD` → `\mathrm{d}`,
  `\doubleprime` → `''`, `\lparen` → `(`, `\lbrack` → `[`, `\mathnormal{…}` →
  `{…}`, `\abs` → `\operatorname{abs}`, `\sgn`/`\sign` → `\operatorname{sgn}`,
  `\cbrt` → `\operatorname{cbrt}`, `\asin` → `\arcsin` (and `\acos`, `\atan`),
  a superscript of primes (`x^{\prime\prime}`) → `x''`. A test runs every
  control word the tokenizer accepts through it and checks that the result
  uses only commands plain LaTeX with amsmath defines.
- The left-hand sides are rebuilt from the state's letter (in the font it was
  typed in: `\dot{\mathrm{S}}` gives `\mathrm{S}'`) and subscript in the
  chosen notation, and every time derivative on the right-hand side
  (`LatexPart::Derivative` in `EquationOutline::rhs_parts`) is rewritten the
  same way, so one equation never mixes notations:

  | | order 1 | order 2 | order ≥ 3 | subscripted |
  | --- | --- | --- | --- | --- |
  | Dot | `\dot{x}` | `\ddot{\theta}` | `\dddot{x}`, then `x^{(4)}` | `\dot{x}_{1}` |
  | Prime | `x'` | `x''` | `x'''`, then `x^{(4)}` | `x_{1}'` |
  | Leibniz | `\frac{dx}{dt}` | `\frac{d^{2}x}{dt^{2}}` | same | `\frac{dx_{1}}{dt}` |
  | Map | `x_{n+1}` | | | `x_{1,n+1}` |

  A Hamiltonian line is kept as typed; an equation not solved for its highest
  derivative (`\ddot{\theta}+b\dot{\theta}+\sin\theta=0`) keeps its shape,
  its derivatives rewritten (`\theta''+b\theta'+\sin\theta=0` in prime
  notation).
- Parameter-value lines (`\sigma=10`) leave the align. With up to three
  parameters every line ends with `,` and the align is followed by
  "where $\sigma = 10$, $r = 28$ and $b = 2.66667$."; with more, the last line
  ends with `.` and a non-floating booktabs Parameter/Value table with `\num{}`
  follows (booktabs and siunitx join the packages). With none, the last line
  ends with `.`.
- `\label{eq:<title-slug>-<state-slug>}` on each line of `align` only (in a
  starred environment a label would pick up the wrong number). Slugs are
  `[a-z0-9-]`: `theta'` is `theta-prime`; the hyphen avoids `:` (active under
  babel-french).
- A text-mode system is converted with `dyn_expr::convert`. If a line can't be
  written in LaTeX exactly, the snippet uses the compiler's first-order form
  (`equations_latex`) and a comment names the line.
- `previewLatex` is the same rows as one `\begin{aligned}…\end{aligned}`
  without labels: the dialog typesets it, never the raw file text.

## Fixed points

A `table` float (`\centering`, booktabs, `@{}…@{}`), caption "Fixed points of
the system with $a = 3$, found in $-0.25 \le x \le 3.5$ and …" and
`\label{tab:<slug>-fixed-points}`.

| System | Columns |
| --- | --- |
| 2D flow | point, eigenvalues λ, τ, Δ, type |
| 1D flow | point, λ, type (Stable / Unstable / Non-hyperbolic) |
| nD flow | point, eigenvalues λ, type |
| 1D map | point, λ = f′(x\*), \|λ\|, type (Strogatz §10.1) |
| nD map | point, multipliers μ, \|μ\|, type |

A complex pair is written once, `$\num{-0.5} \pm \num{1.32}\,i$` (`$\pm
\num{2}\,i$` for a center). τ and Δ come from the eigenvalues. Digits: 2 to 8
significant (default 4); a value below 1e-12 × max(1, largest |value| in its
column) prints as 0. Types are the UI's `classificationTitle`.

## The figure

The figure mirrors `draw.ts` in the projector (light) palette by default, the
colors read from `app/src/styles/ds/tokens.json` (`Palette::from_tokens`; the
app embeds the file with `include_str!`). The theme changes colors only; line
and glyph scale are 1, as on screen.

- Lengths are big points: 1 CSS px = 0.75 bp, and the plot box is
  `width_cm × 96/2.54` px wide. The axis is `scale only axis` with the display
  ranges (already equalized for the figure's aspect).
- Grid: every half step (`xtick` plus `minor xtick`, so the lines beyond the
  first and last tick are kept), `grid=both` in `plot-grid`, no tick lines
  from pgfplots.
- Axis lines where 0 is in view (`axis x line*=middle`); otherwise
  `axis x line*=bottom|top` with the line not drawn, as on screen.
- Tick marks (4 px, below the x axis and left of the y axis, none at 0) and
  tick labels are TikZ elements inside the axis, placed by the port of
  `layoutLabels`: below the x axis when there is room (above otherwise, and
  above first when time is on the y axis), left of the y axis when there is
  room, one `0` where both axes cross, and a label that would leave the plot,
  overlap another label or overlap an axis name is left out. Labels are
  `\footnotesize` maths in `plot-tick` on a halo (`sepxlabel`: the label's box
  filled with `plot-halo` at 0.8 opacity, 1.5 px round the text, as the
  screen's 3 px stroke; a negative `outer sep` keeps the anchors on the text);
  `−` becomes `-` and `1.5e-7` becomes
  `1.5\cdot 10^{-7}`. This deviates from pgfplots' own `xticklabels`, which
  can't drop or flip individual labels the way the screen does; the axis sets
  `scaled ticks=false`, since the labels carry their own exponent and pgfplots
  would otherwise add a second `\cdot 10^{k}` factor.
- Axis names are nodes placed by the port of `axisNamePlacement`: the x name
  right-aligned 8 px in from the right edge, above the x axis when that is more
  than 40 px below the top, otherwise below it; the y name 8 px right of the y
  axis when there are more than 48 px of room, otherwise left of it. An axis
  exactly on the edge (a range starting at 0) counts as on the left or bottom
  edge, in `axisLines` and its port alike, so the two names never meet in a
  corner. A map's state against time is `x_{n}`. Axis names have the same halo
  as tick labels.
- Plots in z-order: field (`quiver`, columns `x y u v` in data units, the
  screen's segment length 0.6 cell and barb `angle'=51.4`, arrows under a
  tick label or axis name left out), nullclines (`dash pattern=on 4.5bp off
  3.75bp`, `null-k`), trajectories (backward first at `opacity=0.5`, then
  forward; 1.125 bp, round caps and joins, `traj-(color mod 7)`), map
  iterates, fixed points (halo, then the glyph: stable filled, unstable or
  saddle open, center or non-hyperbolic `halfcircle*` rotated 90°, which needs
  `plotmarks`). No eigenvectors, selection, hover, handles or stale fade.
- Arrow heads come from `heads::place`, an exact port of `drawHeads` (no head
  under 3·L of visible length, one up to 600 px, then one per 300 px, at most
  12, or 1 when the system has 3 or more states; the ±12 px bend window; a
  slide of 1/6 of the head's share to a straighter spot; 24 px crowding gap).
  Each head is a filled triangle anchored at its point in `axis cs` with bp
  offsets (`\usetikzlibrary{calc}`). A run that did not complete gets an `x`
  at its end.
- Map iterates: `mark=*` of radius 2.6 px (1.3 px above 300 iterates);
  above 2,000, `mark=square*` of half-side 0.75 px, as `fillRect` on screen.
  With time on an axis, a 0.4-opacity line joins them.
- A cobweb (a one-dimensional map with both axes `x` and the graph of `f`; the
  request's `graphSamples`) draws, in z-order, the grid and axes, the diagonal
  `x_{n+1} = x_n` across the view (`sepxaxis`, 0.75 bp, butt caps, inline
  `coordinates`), the graph of `f` (`sepxfp`, 1.125 bp, round joins and caps,
  `nan nan` breaks), one staircase per orbit (`sepxtraj<k>`, 0.75 bp, miter
  joins, no heads; each segment once, keyed on the figure's rounding, so a
  converged p-cycle is 2p segments; clipped like every curve; an `x` at the
  last vertex of a run that did not complete, and, after a transient, an open
  square of half-side 3 px, 1.125 bp on a 2.25 bp `sepxhalo` outline, where it
  starts at `(x_T, x_T)`), then the fixed points at `(x*, x*)`. No map dots and no time-series line. The axis names are `x_{n}`
  and `x_{n+1}` (`with_subscript` of the state's own LaTeX). Each of the three
  plots is preceded by a comment line, the figure's only per-plot comments, for
  a person editing the `.tex`:

  ```
  % Diagonal x_{n+1} = x_n
  \addplot[sepxaxis, line width=0.75bp, line cap=butt] coordinates {(-0.05,-0.05) (1.05,1.05)};
  % Graph of f
  \addplot[sepxfp, line width=1.125bp, line join=round, line cap=round] table …;
  % Cobweb of ic-1
  \addplot[sepxtraj1, line width=0.75bp, line join=miter] table …;
  ```

  With a transient `T > 0` the header says `% Transient: the first T iterates
  are not drawn.` The graph and the staircases are curves for the 5,000-point
  rule (simplified further, never thinned or rasterised).
- Sections. A Poincaré section of a flow (the request's `section`, carried
  into `Portrait::section`) draws each orbit's crossings with the map
  iterates' dot and square rule, preceded by `% Section of <ic>`. No field,
  nullclines, fixed points or arrow heads. Crossings are points: the export
  portrait only cuts them, never simplifies them, and for the 5,000-point rule
  they are rasterised (Save) or thinned (Copy) as map iterates are. With both
  axes one state the figure is the return map: the pairs `(x_k, x_{k+1})` of
  successive crossings under `% Return map of <ic>`, over the diagonal
  `% Diagonal x_{k+1} = x_k` (drawn as a cobweb's), with the axis names
  `x_{k}` and `x_{k+1}`. The header names the section and the skipped
  crossings:

  ```
  % Section: plane z = 27 (increasing).
  % Skipped crossings: the first 10 of each orbit are not drawn.
  ```

  A stroboscopic section reads `% Section: stroboscopic, period 6.283185307179586, phase 0.`
  (the phase in absolute time, as the IPC has it). SVG and PNG draw the same
  scene through `draw.ts` (one disc, or above 2,000 one square, per crossing).
- Every curve is clipped (Liang–Barsky) to the view grown by two field cells
  (cells of 1/20 of the shorter side without a field) before it is written, so
  no coordinate reaches TeX's dimension limit; where a curve leaves and comes
  back, the plot has a break.
- Breaks are `nan nan` rows with `unbounded coords=jump`, inline and in files:
  an inline `table[row sep=\\]{…}` can't hold a blank line.
- Coordinates are rounded to a tenth of a figure pixel of the axis span (at
  least three significant digits), then written in shortest form.
- Dark theme: `axis background/.style={fill=sepxbg}`.
- `standalone: true` wraps the figure in `\documentclass[border=2mm]{standalone}`
  with the preamble lines.
- The `% Needs:` line and the standalone preamble always load amsmath: axis
  names are state LaTeX, and `\dddot{x}` (a fourth-order system) is amsmath's.
- Every definition line before the picture (`\definecolor`,
  `\providecommand`) ends with `%`, so no stray space comes before the
  `tikzpicture` (it would widen a standalone page or shift the figure in a
  `\resizebox` or table cell).

### Data placement

- Copy (`Placement::Inline`): `\addplot[…] table[row sep=\\]{x y\\ …};`.
- Save (`Placement::External { base }`): the snippet starts with
  `\providecommand{\sepxdir}{}` and the comment "If you \input this file from
  another folder, put \def\sepxdir{figures/} before it.", and reads
  each table as `table {\sepxdir <base>-field.dat}`. Files (`FigureOutput::files`,
  written next to the `.tex` with `write_atomic`):

  | File | Contents |
  | --- | --- |
  | `<base>-field.dat` | `x y u v` |
  | `<base>-nullcline-<k>.dat` | `x y`, k = equation number from 1 |
  | `<base>-<icid>-forward.dat`, `-backward.dat` | `x y` |
  | `<base>-<icid>-forward-line.dat` | `x y`, a map's line against time |
  | `<base>-graph.dat` | `x y`, the graph of `f` in a cobweb |
  | `<base>-<icid>-cobweb.dat` | `x y`, an orbit's staircase |
  | `<base>-fixed-points.dat` | `x y glyph` (0 stable, 1 open, 2 half) |
  | `<base>-points.png` | the map iterates' or crossings' PNG layer (see below) |

  `<base>` is a slug of the name the user saved (`latex_figure::file_base`).

### At most 5,000 points

1. Count the data rows.
2. Simplify the curves again at 0.5, then 1, then 2 px (the error stays within
   the sum of the tolerances) until the figure fits.
3. Then the map iterates (or a section's crossings): Save draws them all into `<base>-points.png`
   (`raster.rs`, 300 dpi, transparent, discs or squares by the screen rule,
   `pHYs` set), included with `\addplot graphics[xmin=…,xmax=…,ymin=…,ymax=…]
   {\sepxdir <base>-points.png};` under vector axes. Copy keeps every k-th
   iterate (`thinned: true`, and a comment says so).
4. If it is still over (a very dense field), everything is kept, with a note and
   a comment recommending lualatex.

`FigureOutput { tex, files, points, thinned, notes }`.

## CSV

RFC 4180, except for the documented LF line endings: UTF-8 without a BOM,
one header row, a field quoted only when it holds `,`, `"`, CR or LF (quotes
doubled). Columns are `t` (or `n`) and every state by name; numbers are
`num::shortest` (JavaScript's `String(v)`, which reads back exactly).

Each run is cut, in integration order, at its first sample with a value that
is not finite. The backward run is then reversed and comes first, followed by
the forward run without repeating the start row. The time step can differ
between the two segments (dense output at each run's own samples).

`<stem>.meta.json` (pretty, LF, trailing newline; snake_case like `.sepx`):

```json
{
  "format": "separatrix-trajectory",
  "format_version": 1,
  "app_version": "0.2.0",
  "title": "Rabbits vs sheep",
  "csv": "rabbits-vs-sheep-trajectory-1.csv",
  "ic_id": "ic-1",
  "system": { "kind": "ode", "input_format": "latex", "lines": ["…"] },
  "params": [{ "name": "a", "value": 3.0 }],
  "columns": ["t", "x", "y"],
  "x0": [0.3, 0.2],
  "t0": 0.0,
  "segments": [
    { "direction": "backward", "t_range": [-12.0, 0.0], "samples": 2000, "status": "completed" },
    { "direction": "forward", "t_range": [0.0, 12.0], "samples": 2000, "status": "completed" }
  ],
  "notes": ["…"]
}
```

A map's file also has `"transient": T` after `t0` (the iterates computed but
not written; its first row is `n = T`), an additive key: the meta
`format_version` stays 1. The time-series and cobweb views of one map export
the same file: the staircase is a drawing of the orbit, not new data.

Sections. In a Poincaré section view each row is a crossing (`t` and every
state at the located crossing time), in increasing time. For a plane section
the start point is not a row, and the held state is exactly the plane's value
(`z` is `27` in every row of a `z = 27` section). For a stroboscopic section
the rows are the samples at `t = phase + k·period`, so `t0` is a row when it is
a sample time. The first note says which. The export dialog has no samples
field for a section. The metadata adds, after `t0`, `"section": "plane z = 27
(increasing)"` (or `"stroboscopic, period T, phase φ"`) and
`"skipped_crossings": k`, the crossings computed but not written before the
first row; both additive. A return map exports the same file as the section in
two states.

`x0` is in `columns[1..]` order. `status` is the IPC name (`blewUp`, …).
`stem` is `<title-slug>-trajectory-<k>`, k numbering the trajectories as the export
dialog lists them ("Trajectory 2" → `rabbits-vs-sheep-trajectory-2.csv`); the initial
condition's own id stays in `ic_id`. Saving several asks for one name and writes
`<name>-trajectory-<k>.csv` beside it.

## Numbers

- `num::shortest(v)` is JavaScript's `String(v)`: Rust's `{}` for
  1e-6 ≤ |v| < 1e21, exponent notation with an explicit `e+` otherwise
  (`1e-7`, `1e+21`), `-0` as `0`. JavaScript switches below 1e-6 (`String(1e-7)`
  is `"1e-7"`), where Rust's `{}` would write `0.0000001`.
- `num::to_fixed` and `num::to_exponential` round the exact decimal value half
  away from zero, as JavaScript does (Rust's `{:.N}` rounds ties to even).
- `num::readable`, `num::tex` and `num::si` are the app's `fmt`: ASCII `-`,
  `1.23\times 10^{-5}` in maths, `\num{1.23e-5}` in tables.
- `ticks::nice`, `ticks::label` and `ticks::half_steps` port `niceTicks`,
  `tickLabel` and `drawGrid`'s half steps. One known difference: V8's
  `Math.pow` is not correctly rounded (`Math.pow(10, -5)` is
  `9.999999999999999e-6`), Rust's is (like JavaScriptCore's, which draws the
  screen on Linux and macOS), so for a few powers of ten the step differs in
  its last bit.

## Test vectors

Shared with the TypeScript tests, in `crates/export/tests/vectors/`:

- `numbers.json`: `{ cases: [{ v, s }] }`, `s = String(v)`.
- `ticks.json`: `{ ticks: [{ lo, hi, target, step, ticks, labels }], labels:
  [{ v, step, label }] }` from `niceTicks`/`tickLabel`.
  `node crates/export/tests/vectors/generate.mjs` rewrites both from
  `app/src/canvas/viewport.ts`.
- `cobweb.json` (written by `app/src/review/cobweb.test.mjs` from
  `app/src/canvas/cobweb.ts`): `{ cases: [{ x: [v0, v1, …], xRange, yRange,
  staircase: [x, y, …], diagonal: [ax, ay, bx, by] | null }] }`. The Rust test
  checks `cobweb::staircase` and `cobweb::diagonal` against it when the file
  exists.
- `heads.json` (written by the UI's tests from `drawHeads` through a recording
  context): `{ cases: [{ pts: [x0, y0, x1, y1, …], vp: [left, top, width,
  height], L, maxHeads, minGap, backward, placed: [x, y, …], triangles:
  [[tipX, tipY, c1X, c1Y, c2X, c2Y], …], placedAfter: [x, y, …] }] }`, the
  triangles as recorded (`moveTo`, `lineTo`, `lineTo`). The Rust test checks
  it when the file exists.

## Tests

- `cargo test -p separatrix-export`: unit tests, `tests/golden.rs` (rabbits vs
  sheep, Lorenz over t ≤ 10, the Hénon map, the logistic map as a time
  series and as a cobweb (with its fixed-point table and external files), a saddle-node, a
  linear system with a parameter table, MathLive commands, text mode,
  subscripted states, names that need escaping, the Sel'kov branch plot, and
  the CO oxidation and predator–prey parameter planes;
  `UPDATE_GOLDEN=1 cargo test -p separatrix-export --test golden` rewrites
  `tests/golden/`), `tests/rules.rs` (CSV, clipping, the 5,000-point rule),
  `tests/vectors.rs`.
- `tests/latex_compiles.rs` compiles every golden `.tex` with
  `pdflatex -interaction=nonstopmode -halt-on-error` in an `article` that loads
  exactly the packages of its `% Needs:` line, and `\input`s an external
  figure from a subdirectory with the line the figure's comment documents
  (`\def\sepxdir{sub/}`; `\renewcommand` would fail on the first figure,
  since `\sepxdir` only exists once a figure has been read), and checks that a
  standalone figure's page is its axis plus the 2 mm border. It is
  skipped when `pdflatex` is missing and fails then when `REQUIRE_PDFLATEX=1`.
  On Ubuntu: `apt-get install --no-install-recommends texlive-latex-base
  texlive-latex-recommended texlive-latex-extra texlive-pictures
  texlive-science texlive-fonts-recommended lmodern`.
