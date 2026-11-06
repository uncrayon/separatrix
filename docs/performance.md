# Evaluation speed on the chaos workloads

The Lyapunov view and the orbit diagram call the compiled system millions of
times per run. The plan for M3 named a Cranelift JIT, with the interpreter
kept as the fallback. This page records how fast the interpreter is on those
workloads, what limits it, and what was decided.

To rerun the measurement:

```sh
cargo run --release -p dyn-analysis --example chaos_bench
```

## The workloads

Each workload runs on two sides:

- **compiled** is the gallery template (`app/src/templates/templates.json`)
  compiled by `dyn_expr::compile`, so `eval` runs the register program and
  `jacobian` runs one dual-number pass per state.
- **native** is the same system as a `dyn_core::FnField` with the field and an
  exact Jacobian written by hand in Rust. Generated code cannot beat it, so it
  is the most a JIT could gain.

| Workload | Run |
| --- | --- |
| (a) gate | Lorenz `lyapunov_spectrum`, the gate's settings: RK4 h = 0.01, transient 100, T = 1e4, τ = 0.1, 10 blocks |
| (b) view | Lorenz at the Lyapunov view's default T = 1e3, otherwise as (a) |
| (c) orbit | logistic `orbit_diagram`, 1200 columns over r ∈ [2.8, 4], 1000 transient and 300 kept iterates |
| (d) Hénon | Hénon `lyapunov_spectrum`, transient 1e3, N = 1e6, a QR step every iterate |

The check value is λ1 for (a), (b) and (d), and the sum of the kept x for (c).
It proves the work ran, and the two sides must agree on it.

## Decision rule

This rule was written before the first run. Budgets hold for a release build
on this 4-CPU machine, on one core:

| Workload | Budget |
| --- | --- |
| (b) view | ≤ 500 ms |
| (c) orbit | ≤ 50 ms |
| (a) gate | ≤ 5 s |

1. Optimize the interpreter only if a workload misses its budget **and** its
   compiled median is more than 3× its native median. A miss that native also
   has is not the interpreter's fault.
2. Build the Cranelift JIT only if the interpreter still misses a budget after
   that optimization **and** the JIT can be shown green in CI on Linux,
   Windows, macOS x64 and Apple Silicon. No run in M3 can show the CI half,
   because M3 work is not pushed. If step 2 fires, the JIT is recorded as
   blocked on that proof, not as rejected on speed.
3. Otherwise make no change.

Each number is the median of at least 5 runs per side, with compiled and
native runs alternating, and the range beside it.

## Results

Measured on 2026-11-06 on this machine: `nproc` 4, load average 0.33 before
the first run and 0.44 after the second, nothing else busy (`ps` showed only
the agent at 0.8 % CPU). Release profile as the app ships it (`opt-level = 3`,
thin LTO, one codegen unit). Both sides go through `&dyn VectorField`, so
dispatch is the same and only the evaluator differs.

Two invocations of `chaos_bench 7`, each with 7 alternating runs per side.
Times are the median in ms, with the range in brackets:

| Workload | Side | Run 1 | Run 2 | Evals | Check value |
| --- | --- | --- | --- | --- | --- |
| (a) gate | compiled | 955.6 (949.2–988.8) | 1029.2 (1010.2–1100.9) | 4 140 002 | λ1 = 0.906850434091 |
| (a) gate | native | 298.7 (298.3–310.6) | 301.6 (290.3–369.7) | 4 140 002 | λ1 = 0.906850434091 |
| (b) view | compiled | 98.0 (96.6–99.5) | 105.6 (104.8–107.7) | 450 002 | λ1 = 0.908966209646 |
| (b) view | native | 30.4 (30.3–32.0) | 30.4 (30.2–31.2) | 450 002 | λ1 = 0.908966209646 |
| (c) orbit | compiled | 25.6 (25.5–26.8) | 26.1 (25.5–26.9) | 1 558 800 | Σx = 230797.404020401387 |
| (c) orbit | native | 5.4 (5.3–5.9) | 5.4 (5.3–6.0) | 1 558 800 | Σx = 230797.404020401387 |
| (d) Hénon | compiled | 181.9 (181.1–188.5) | 158.7 (158.5–161.1) | 1 001 000 | λ1 = 0.419430527398 |
| (d) Hénon | native | 83.6 (82.7–86.7) | 83.6 (82.7–86.6) | 1 001 000 | λ1 = 0.419430527398 |

Compiled/native ratios in run 1: (a) 3.20×, (b) 3.22×, (c) 4.74×, (d) 2.17×.
Within one invocation the spread is a few percent. Between invocations the
compiled median moves by up to 14 % ((d) 182 → 159 ms) while native does
not, so code placement in the binary probably matters (a guess, not checked).
Both runs stay inside every budget.

The work happened. Each side's check value is bit-identical on every run,
and the two sides agree bit for bit. For this, the native Hénon field computes
`y + 1 − a·(x·x)` in the order the compiled program uses. With `(a·x)·x`, the
chaotic orbits drift apart and λ1 differs by 2.3·10⁻⁴. The evaluation counts
match too. λ1 = 0.90685 for (a) is the gate's Lorenz row. Every Lyapunov run
must end with `Status::Completed`, and every orbit column must stay finite, or
the benchmark panics. A flow eval is one `Tangent` evaluation, which is one
`eval` plus one `jacobian`. For (c) the count is computed as
1200 × (1000 + 300 − 1), the iterates `orbit_diagram` runs when no column
escapes.

### What limits each workload

These shares come from `perf record` runs that are not reported above. They
were taken on a build with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`. The
percentages are self time out of all samples, and both sides run in the same
process.

- **(a), (b) Lorenz.** `CompiledSystem::jacobian`'s three dual-number passes
  (`run_dual1` in `crates/dyn-expr/src/system.rs`, which runs
  `Program::run` in `program.rs` on `Dual<f64>`) take 45 % of all samples,
  which is about 59 % of the compiled side's time. The `f64` `eval` takes
  7.5 %, about 10 %. The rest is shared with native: `Stepper::commit` and
  `Stepper::step` (25 %), `Tangent::eval`'s `J·Φ` product (11 %), and
  `memset` (6 %). Most of that `memset` comes from zero-filling the 16 × 16
  Jacobian scratch in `with_jacobian_scratch` on every `Tangent` evaluation.
  The native side costs about 30 ms for (b), and no evaluator can make (b)
  faster than that. So even a perfect JIT gains at most 3.2×, from 98 ms to
  30 ms.
- **(c) logistic orbit.** `CompiledSystem::eval` (the `Program::run` loop,
  plus zeroing the 32 stack registers in `with_regs` on each call) takes 65 %
  of samples. The `orbit_diagram` loop takes 23 %. That is about 16 ns per
  compiled iterate against 3.5 ns native (25.6 ms and 5.4 ms over 1 558 800
  iterates). So 50 ms covers about 3 million compiled iterates, for example
  2300 columns of 1300 iterates.
- **(d) Hénon.** The work is spread out. `run_dual1` (the Jacobian) takes 24 %,
  `log` 23 % (`ln |det J|` and the QR `ln R_ii`), `Accumulator::renormalize`
  19 %, and the map loop in `lyapunov_spectrum` 20 %. That is why the ratio is
  only 2.2×.

## Decision

**No change needed.** Every workload is inside its budget on the compiled
side:

| Workload | Budget | Compiled median (worse of the two runs) | Headroom |
| --- | --- | --- | --- |
| (b) view | 500 ms | 105.6 ms | 4.7× |
| (c) orbit | 50 ms | 26.1 ms | 1.9× |
| (a) gate | 5 s | 1.03 s | 4.9× |

Step 1 of the rule needs a missed budget, so the interpreter stays as it is.
The compiled/native ratio is above 3× for (a), (b) and (c), but that alone does
not trigger the rule. Step 2 never comes up, so no Cranelift JIT is built. A
JIT would be judged by the same rule: a measured missed budget first, then the
CI proof on all four targets.

If a later workload misses a budget, start from the limiter above. For flows,
it is the Jacobian's `n` dual passes, which could become one pass that carries
all `n` tangent directions. For maps, it is the per-call register setup in
`with_regs`.
