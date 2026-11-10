# Parameter plane

The Parameter plane follows a bifurcation as two parameters change. The Branch view's parameter
runs across, a second parameter runs up, and each fold, Hopf, branch point, flip or
Neimark–Sacker point found in the Branch view becomes a curve. Choose **Parameter plane** in the
**View** panel. It is offered for a system with two parameters or more whose equations don't
contain `t` or `n`.

The desktop app follows the curves of any such system, but it does not mark the points where
curves meet yet. The browser preview draws the curves of the two gallery templates below only, and
marks those points on them.

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
| R1 to R4 | Strong resonances on a Neimark–Sacker curve |
| CH | Chenciner, where a Neimark–Sacker point changes criticality |
| LPPD | Fold-flip, where a flip curve meets a fold or branch-point curve |

The crosshair is the current pair of parameter values, and the grey bar is the Branch view's range
at the current value of the parameter up. Click anywhere to set both parameters; the sliders and
every other view follow. Click a row in **Special points** to set both parameters there.

## Try it

- **CO oxidation** (Bykov, Yablonskii and Kim; MatCont Tutorial IV): the Branch view over k₂ finds
  H, LP, LP, H. Followed in K, the fold and Hopf curves meet at two Bogdanov–Takens points, the
  fold curve has a cusp, and the Hopf curve two Bautin points.
- **Predator–prey map** (MatContM Tutorial M-II): the Neimark–Sacker point at a = 5, followed in
  ε, passes the 1:4, 1:3 and 1:2 resonances and a Chenciner point. The branch point at a = 5/3 is
  where the coexistence fixed point meets the one without predators. Followed in ε, its curve
  passes a fold-flip at (a, ε) = (3, 2), where the predator-free fixed point flips. That flip
  happens at a = 3 whatever ε is, so the plane does not follow it in ε: it says ε doesn't move it.

The plane is not saved in project files yet: saving while it is shown saves the Branch view.
