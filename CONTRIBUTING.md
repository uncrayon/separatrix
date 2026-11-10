# Contributing to Separatrix

This page lists what you need to build Separatrix, the checks every change must pass, and the
rules that keep its numbers the same on every OS. Run the commands from the repository root unless
a step says otherwise.

## Prerequisites

- Rust 1.90 or newer, with `clippy` and `rustfmt`. CI also runs clippy with the newest stable Rust.
- Node 22.
- On Linux, the WebKitGTK and GTK development packages:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
  ```

- For the end-to-end tests, Playwright with its Chromium, installed globally:

  ```sh
  npm install -g playwright
  npx playwright install chromium
  ```

Install the app's packages once:

```sh
cd app && npm ci
```

## Run the checks

Every change must pass all of these.

1. Rust tests, in release mode because the numerical tests are slow in debug:

   ```sh
   cargo test --workspace --release
   ```

2. Clippy with warnings as errors, on your toolchain and on the newest stable one:

   ```sh
   cargo clippy --workspace --all-targets -- -D warnings
   cargo +stable clippy --workspace --exclude separatrix --all-targets -- -D warnings
   ```

3. Formatting:

   ```sh
   cargo fmt --all -- --check
   ```

4. The TypeScript build, which type-checks and bundles the interface:

   ```sh
   cd app && npm run build
   ```

5. The unit tests of the interface:

   ```sh
   cd app && node --test src/review/*.test.mjs
   ```

6. The end-to-end tests, which drive the interface in Chromium against a Vite server. Start the
   server on a free port, run each test file against it, then stop the server:

   ```sh
   cd app
   npx vite --port 5601 --strictPort &
   for f in src/review/*.e2e.cjs; do
   	UI_URL=http://localhost:5601/ NODE_PATH=$(npm root -g) node --test "$f"
   done
   kill %1
   ```

   The browser preview has no Rust core. `app/src/api/mock.ts` stands in for the Tauri commands.

## Keep the generated files up to date

The TypeScript types of the Tauri commands, `app/src/api/ipc.generated.ts`, are generated from
the Rust types in `app/src-tauri/src/ipc.rs`. A test fails while they are out of date. After you
change an IPC type, regenerate them:

```sh
UPDATE_IPC_BINDINGS=1 cargo test -p separatrix --lib typescript_bindings_are_up_to_date
```

Other tests rewrite their expected files the same way:

| Variable | Test | What it rewrites |
| --- | --- | --- |
| `UPDATE_GOLDEN=1` | `cargo test -p separatrix-export --test golden` | The expected LaTeX and CSV exports. |
| `UPDATE_FIXTURES=1` | `cargo test -p separatrix-project --test roundtrip` | The `.sepx` fixtures, in canonical form. |
| `UPDATE_TEMPLATE_TEXT=1` | `cargo test -p dyn-expr --test convert` | The text-mode form of the gallery templates. |

Read the diff of a rewritten file before you commit it.

## Run the numerical gates

Four scripts check Separatrix's numbers against published values. Each prints one row per claim,
with the reference, the measured value, the tolerance and PASS or FAIL, and exits non-zero if any
row fails. Each also hands itself wrong claims and requires them to fail.

```sh
bash scripts/strogatz-gate.sh
bash scripts/chaos-gate.sh
bash scripts/continuation-gate.sh
bash scripts/codim2-gate.sh
```

- The **Strogatz gate** checks every gallery example from chapters 2 to 8 against the book:
  fixed points and their types, bifurcation values, periods, a limit cycle's radius and energy
  drift. A template that cites a chapter 2–8 section can't be added without a claim in
  `app/src-tauri/tests/strogatz.rs`.
- The **chaos gate** checks Lyapunov spectra (Lorenz, Hénon, the logistic map), the logistic
  map's period doublings, the symplectic methods' bounded energy error, and the stiff solver on
  Robertson's reaction. The claims are in `app/src-tauri/tests/chaos.rs`.
- The **continuation gate** checks 27 bifurcation points derived in closed form. They cover folds,
  branch points, Hopf points with ω and ℓ₁, flips and a Neimark–Sacker point, and branch switching
  through the logistic cascade. The claims are in `app/src-tauri/tests/continuation.rs`.
- The **codimension-2 gate** checks 74 claims about the points where curves in two parameters
  meet, on the reference systems of [docs/codim2-reference.md](docs/codim2-reference.md): the
  Bogdanov–Takens, cusp, Bautin, zero-Hopf, Hopf–Hopf, strong-resonance, Chenciner, fold-flip and
  generalized-flip points of CO oxidation, Lorenz-84 and the predator–prey map, and the
  Bogdanov–Takens `|a|`, `|b|`, sign of `ab` and cusp `c` of CO oxidation against MatCont. Each
  curve starts from a Branch view point and is followed with the Parameter plane's own request.
  The gate also checks that a point found on two curves is in the same place on both, that each
  curve has no other point, and that only BT, ZH, R1 and R2 lie on a neutral stretch. The claims
  are in `app/src-tauri/tests/codim2.rs`.

Right after `cargo test --workspace --release`, each gate reuses the build. To time the evaluation
workloads, run `cargo run --release -p dyn-analysis --example chaos_bench`.
[docs/performance.md](docs/performance.md) records the budgets.

## Check that a release is ready

```sh
bash scripts/release-check.sh
```

prints PASS or FAIL for each release requirement, including the four gates, and exits with the
number of FAIL rows. `--skip-gates` skips the gates for a quick run. The owner's remaining steps
are in [docs/release-checklist.md](docs/release-checklist.md).

To check the Linux packages the way a professor gets them, run

```sh
bash scripts/smoke/linux-bundle.sh
```

It builds the .deb, .rpm and AppImage, names them and checks `SHA256SUMS.txt` with the release
scripts, installs and removes the .deb, then starts the AppImage under Xvfb and opens one project by
drag-and-drop and another with Open…. It needs 3 GB free, Xvfb, openbox, xdotool, ImageMagick,
python-xlib, rpm, and root or sudo for `dpkg`. Screenshots land in `target/smoke-shots`, and the
packages are deleted at the end.

To refresh the user guide's screenshots, build the app with its bundled interface and run the
screenshot script. It needs Xvfb and ImageMagick.

```sh
(cd app && npm run build)
cargo build --release -p separatrix --features tauri/custom-protocol
bash scripts/docs-screenshots.sh
```

## Keep expected files identical on every OS

Golden files and expected outputs must be byte-identical on Linux, macOS and Windows. CI runs the
Rust tests on all three.

Platform maths libraries differ in the last bit of `sin`, `exp`, `hypot`, `atan2` and the other
`libm` functions. So never let such a function decide which points are kept or how a number is
printed at a rounding boundary. Compare squared distances instead of `hypot`, and round on values
that come from exact arithmetic. If a value must come from `libm`, keep it away from the cut-off,
or print fewer digits than the platforms disagree on.

## Write commits

Use [Conventional Commits](https://www.conventionalcommits.org/) titles, such as
`fix(ui): …`, `feat(core): …`, `docs: …` or `ci(release): …`. The body says what changed for the
user and why. Update [CHANGELOG.md](CHANGELOG.md) for any change a user can notice.

## Where to read more

- [docs/contracts.md](docs/contracts.md) is the reference for the Rust crates and the Tauri
  commands.
- [docs/exports.md](docs/exports.md) specifies every export.
- [docs/project-format.md](docs/project-format.md) specifies the `.sepx` format.
- [docs/updater.md](docs/updater.md) says how the owner switches on the updater.
- [docs/codim2-reference.md](docs/codim2-reference.md) lists the literature systems and values
  that two-parameter continuation is tested against.
