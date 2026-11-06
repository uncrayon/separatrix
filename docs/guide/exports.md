# Export figures and data

The export dialog writes the current view as LaTeX for your notes, SVG or PNG for slides, or CSV
for your own analysis. To open it, click **Export…** at the top of the window or press Ctrl+E
(⌘E on a Mac).

## Choose a format

| Format | What you get |
| --- | --- |
| **LaTeX** | A snippet that compiles in your own document, in Strogatz's notation. |
| **SVG** | The figure as vector graphics, drawn by the same code as the screen. |
| **PNG** | The figure as an image, at 2×, 3× or 4× the pixels of its size. |
| **CSV** | The numbers behind the figure, with a `.meta.json` file that records the system and the settings. |

The preview shows the result. The copy button (**Copy LaTeX**, **Copy SVG**, **Copy image** or
**Copy CSV**) puts it on the clipboard, and **Save…** writes a file.

## Choose what LaTeX writes

The **Content** menu offers what fits the view:

| Content | Views | What it is |
| --- | --- | --- |
| **Figure** | Phase plane, time series, cobweb, section, orbit diagram, Branch | A pgfplots figure of the plot. |
| **Equations** | All | The system in an `align` environment, numbered or not. |
| **Fixed points** | Phase plane, time series, cobweb, section, 3D | A booktabs table of the fixed points, their eigenvalues and their type. A system whose equations contain `t` has none. |
| **Spectrum** | Lyapunov | A booktabs table of the exponents, with siunitx uncertainties. |
| **Special points** | Branch | A booktabs table of the bifurcations with siunitx numbers. |

The first lines of every snippet are comments that name the system, its parameter values and the
LaTeX packages it needs. **Standalone document** wraps the snippet in a document that compiles on
its own. **Notation** writes derivatives as ẋ (Strogatz), x′ or dx/dt.

The 3D view and the Lyapunov view have no LaTeX figure. Export their plots as PNG or SVG.

## Size and colors

- **Width** and **Height** set the figure's size, in cm for LaTeX and SVG, and in px for PNG.
- **Colors** is **Projector** (dark lines on white, the default) or **Dark** (the screen's
  chalkboard colors).
- **Transparent background** leaves the background out of SVG and PNG.
- **Include** switches the direction field, the nullclines and the fixed points on or off in the
  figure.

## Export data as CSV

A trajectory's CSV has a column for `t` or `n` and one for each state. **Samples per run** sets
how many rows each trajectory gets. Other views write their own columns. A section writes each
crossing, the Lyapunov view the running estimates, the orbit diagram one row per kept iterate, and
the Branch view one row per branch point, with the special points in a second file.

## Your settings are kept

The dialog remembers its settings in the project file, so the next export of the same project
starts where you left off.

The exact contents of every format are in [docs/exports.md](../exports.md).
