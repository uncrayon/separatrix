# Enter equations

Type one equation per line in the **Equations** panel, the way you would write it on the board.
Separatrix works out the kind of system, its states and its parameters as you type, and redraws
the plot when the equations compile.

## Choose LaTeX or plain text

The **LaTeX** and **Text** switch at the top of the panel sets how you type.

- **LaTeX** is a maths field. Type `\dot{x}=x(3-x-2y)` or use the keys you know from LaTeX:
  `\frac`, `\sqrt`, `^`, `\sin`, Greek letters such as `\sigma`. A single letter is one symbol, so
  `2xy` means 2·x·y.
- **Text** is plain text, as in a programming language: `x' = x*(3 - x - 2*y)`. Names are whole
  words, so write `x*y`, not `xy`. Greek names such as `sigma` and `theta` show as σ and θ.
  Functions take parentheses: `sin(x)`.

Switching between the two converts every line. If a line can't be converted exactly, it stays as
you typed it, with a note under it, and the switch doesn't happen.

## Write each kind of system

| To enter | Type in LaTeX | Type in text |
| --- | --- | --- |
| A flow | `\dot{x} = y` and `\dot{y} = -x` | `x' = y` and `y' = -x` |
| A second-order equation | `\ddot{x} = -x - \mu(x^2-1)\dot{x}` | `x'' = -x - mu*(x^2 - 1)*x'` |
| A map | `x_{n+1} = r x_n (1 - x_n)` | `x_{n+1} = r*x_n*(1 - x_n)` |
| A Hamiltonian system | `H(q, p) = \frac{p^2}{2} - \cos q` | `H(q, p) = p^2/2 - cos(q)` |
| A parameter's starting value | `\sigma = 10` | `sigma = 10` |

- A flow uses `\dot{x}`, `x'` or `\frac{dx}{dt}` on the left. Time is `t`.
- A higher-order equation is rewritten as a first-order system. The new state appears as
  `\dot{x}`, and the panel shows the system that is solved.
- A map uses `x_{n+1}` on the left. On the right, a bare `x` means `x_n`. The step is `n`.
- In `H(q, p)`, the first half of the arguments are coordinates and the second half are momenta.
  Separatrix solves `q̇ = ∂H/∂p` and `ṗ = −∂H/∂q`.
- Any other name is a parameter. A line such as `\sigma = 10` sets its starting value. Without
  one, a parameter starts at 1.

The names `t`, `n`, `\pi` and `e` are reserved. LaTeX understands `\sin`, `\cos`, `\tan`, `\exp`,
`\ln`, `\log`, `\sinh`, `\cosh`, `\tanh`, `\arcsin`, `\arccos`, `\arctan`, `|x|` and `\sqrt[n]{x}`.
For names longer than one letter, use `\mathrm{name}` in LaTeX. Text mode writes the `n`-th root as
`root(x, n)`.

The complete grammar is in [docs/contracts.md](../contracts.md), under `dyn-expr`.

## Read what Separatrix inferred

Under the equations, the panel lists:

- **Kind**, which says ODE or Map, the dimension, and whether the system is autonomous or
  Hamiltonian.
- **States**, in equation order.
- **Parameters**, in order of first appearance.
- **Time** (`t`) for a flow or **Step** (`n`) for a map.

If a line has a mistake, the line is marked and a message under the equations says what is wrong
and how to fix it.

## Set parameters

Each parameter gets a slider in the **Parameters** panel.

- Drag the slider to change the value. Every view updates as you drag.
- Click the number box beside the slider to type an exact value. Up and Down step it by a hundredth
  of the slider's range.
- Click the minimum or the maximum under the slider to change its range.

## Set start points

Click the plot to add a trajectory or an orbit. Drag its start point to move it. The **Initial
conditions** panel lists every start point with its coordinates. To delete one, click the bin icon
beside it. The bin icon in the panel's header deletes them all.

## Choose the solver

The **Solver** panel sets how a flow is integrated.

| Method | Use it for |
| --- | --- |
| Dormand–Prince 5(4) | Most systems. Adaptive step with error control. The default. |
| Runge–Kutta 4, Heun, Euler | Teaching fixed-step methods. You set the step h. |
| Störmer–Verlet, Yoshida 4 | Hamiltonian systems entered as `H(q, p)`. The energy error stays bounded. |
| Rosenbrock 2(3) | Stiff systems. If Dormand–Prince's steps look stiff, the panel suggests it. |

**Rel. tolerance** and **Abs. tolerance** apply to the adaptive methods. **Time span** sets how far
each trajectory runs. **Backward time** also follows each start point backward in time, drawn
fainter.

A map needs no solver. Its **Iteration** panel sets the number of iterates and a transient to hide.
