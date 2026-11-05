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
| Export portrait | `simplify::simplify_portrait` | cut at the first non-finite point, RDP at 0.25 px (the graph of f in a cobweb at 0.0625 px, so it shows no facets when zoomed) |
| Clipboard PNG | `raster::decode_rgba` | for `Image::new_owned(rgba, w, h)` |

Every LaTeX snippet starts with comment lines: the title, the equations as
plain text (`convert` to text), the parameter values, and a `% Needs:` line
with the packages it uses. Nothing in a `.tex` output is U+2212 (TeX's default
input encoding can't read it); the real minus sign stays in SVG and PNG.

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
colours read from `app/src/styles/ds/tokens.json` (`Palette::from_tokens`; the
app embeds the file with `include_str!`). The theme changes colours only; line
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
state at the located crossing time), in increasing time, and the start point
is not a row. The export dialog has no samples field for it. The metadata
adds, after `t0`, `"section": "plane z = 27 (increasing)"` (or
`"stroboscopic, period T, phase φ"`) and `"skipped_crossings": k`, the
crossings computed but not written before the first row; both additive. A
return map exports the same file as the section in two states. A plane
crossing's held state is the plane's value to the event location's accuracy:
Brent's method stops when the time bracket is within `1e-12·max(|t|, 1)`, so
`|z − 27|` grows with `t` (up to about `3·10⁻⁸` on the Lorenz attractor by
`t = 500`).

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
  subscripted states, names that need escaping;
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
