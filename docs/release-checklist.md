# Release checklist for v1.1.0

Version 1.1.0 adds the Parameter plane: curves of bifurcations in two parameters, the points where
they meet, homoclinic curves, its exports and its place in project files. Everything in it is on
`main` and passes every check. These are the steps that only the owner can take, because they
need the GitHub account, the updater's private key, a Linux machine for the screenshots, or a real
Mac and a real Windows PC. Do them in order. Tick each box when its **Done when** line is true.
The checklist for v1.0.0 is in the history of this file.

The updater key from v1.0.0 must still be in the repository secrets, `TAURI_SIGNING_PRIVATE_KEY`
and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and the five Apple secrets of
[Sign and notarize the Mac app](macos-signing.md). A build signed with any other key can't update
a 1.0.x install.

1. [ ] **Check `main`.** Pull `main` and run `bash scripts/release-check.sh`, with the gates. Then,
   on a computer with a TeX distribution, compile every LaTeX golden, the Parameter plane's new
   figures and tables among them. CI has no TeX, and they have never been compiled:

   ```sh
   REQUIRE_PDFLATEX=1 cargo test -p separatrix-export --test latex_compiles
   ```

   **Done when** every row of the release check reads `PASS`, the four gates among them, and the
   `latex_compiles` test passes with no file skipped.

2. [ ] **Set the version to 1.1.0.** Change `version` to `1.1.0` in `[workspace.package]` of
   `Cargo.toml` and in `app/package.json`, then run `npm install` in `app/` so that
   `app/package-lock.json` follows. `tauri.conf.json` inherits the version. Then run
   `bash scripts/release-check.sh --skip-gates`: the `install-guides` row names any asset in
   `docs/install/*.md` that still says 1.0.1. Change those names to 1.1.0.

   **Done when** the `versions` row reads `PASS  versions: 1.1.0 in Cargo, package.json,
   package-lock.json; tauri.conf.json inherits` and the `install-guides` row reads `PASS`.

3. [ ] **Take the Parameter plane's screenshot.** On a Linux machine with Xvfb and ImageMagick,
   build the app with the bundled UI and run the screenshot script, as its header says:

   ```sh
   cd app && npm run build && cd ..
   cargo build --release -p separatrix --features tauri/custom-protocol
   bash scripts/docs-screenshots.sh
   ```

   It opens `docs/guide/examples/parameter-plane.sepx` and saves `docs/media/parameter-plane.png`,
   and retakes the other views. Keep the other screenshots unless the app changed how they look.
   Then run `bash site/media.sh` (it needs ffmpeg with libwebp) to make the web copy and its
   entry in `site/media/manifest.json`. Last, add the picture to the guide page: in
   `docs/guide/parameter-plane.md`, before "Example project:", add an image of
   `../media/parameter-plane.png`, written as the one in `docs/guide/continuation.md`, with the
   alt text "CO oxidation in the Parameter plane: the fold and Hopf curves in k₂ and K, meeting at
   two Bogdanov–Takens points, with the cusp and two Bautin points marked".

   **Done when** `docs/media/parameter-plane.png` shows CO oxidation's fold and Hopf curves with
   their Bogdanov–Takens points, the guide page shows it, `bash scripts/release-check.sh
   --skip-gates` still passes its `docs-links` row, and `cd site && npm ci && npm run build`
   prints no warning about `parameter-plane`.

4. [ ] **Date the changelog.** In `CHANGELOG.md`, change `## [Unreleased]` to
   `## [1.1.0] - YYYY-MM-DD` with today's date. Commit the version, the screenshots and the date,
   and push them to `main`.

   **Done when** `main` on GitHub shows the dated heading, the new version and the new screenshot.

5. [ ] **Run the Release workflow for a test build.** On GitHub, open **Actions**, click
   **Release**, click **Run workflow**, choose `main`, and click **Run workflow**. The run ends with
   a draft release named **Separatrix 1.1.0**.

   **Done when** all five jobs of the run are green and the draft release lists seven installers,
   `Separatrix_1.1.0_macos_universal.app.tar.gz`, the `.sig` signature files, `latest.json` and
   `SHA256SUMS.txt`.

6. [ ] **Download the draft and check the checksums.** Open the draft on the **Releases** page,
   download every file into one empty folder, and run `sha256sum -c SHA256SUMS.txt` in that folder
   (on a Mac, `shasum -a 256 -c SHA256SUMS.txt`).

   **Done when** every line ends in `OK` and `latest.json` lists seven platforms: `linux-x86_64`
   (the AppImage), `linux-x86_64-deb`, `linux-x86_64-rpm`, `windows-x86_64` (the setup `.exe`),
   `windows-x86_64-msi`, `darwin-x86_64` and `darwin-aarch64`.

7. [ ] **Test the Parameter plane on a real Mac.** Install the draft's `.dmg` over 1.0.1, then
   check each of these:
   - Open `docs/guide/examples/parameter-plane.sepx`. It opens in the Parameter plane with CO
     oxidation's fold and Hopf curves, and **Special points** lists two BT, a CP and two GH rows,
     each with its sentence.
   - The first BT row reads `k₂ = 1.161199  K = 0.722339` and `a = 0.0482, b = 1.9376 · ab > 0`.
   - On that row, **Follow homoclinic** draws a homoclinic curve, and **Cancel** on a second run
     stops it.
   - The inset follows the crosshair while you drag the K slider.
   - Open the **Predator–prey map** template, open the Branch view, and click **Search in two
     parameters**. The Neimark–Sacker curve has the ticks 1:5 to 1:9.
   - **Export…** in the plane: the LaTeX figure and the special points table compile in a LaTeX
     document with the packages their `% Needs:` line names, and the PNG, SVG and CSV save.
   - **Save** the plane, quit, and open the file again. It opens in the plane with the same curves.
   - A project saved by 1.0.1 still opens in the view it was saved in.
   - **Separatrix** > **Check for Updates…** opens the update panel.

   **Done when** every check above passed on the Mac.

8. [ ] **Test on a real Windows PC.** Install the draft's setup `.exe` over 1.0.1 and repeat the
   first, sixth and seventh checks of step 7, with **Ctrl+O** in place of **⌘O**.

   **Done when** those checks passed on Windows.

9. [ ] **Replace the screenshot placeholders.** These are still open from v1.0.0. While you test on
   the Mac and the PC, take each screenshot below. Save it in `docs/install/images/` with a name
   such as `macos-open-anyway.png`, and replace its `> **Screenshot to add on a real ...**` line in
   the guide with an image link to that file, written the same way as the images in the Linux
   guide. Tick each box here as you replace its line, so the list stays equal to the placeholders
   still in the guides.
   - [ ] [macos.md](install/macos.md): The disk image window, with the Separatrix icon and the Applications folder side by side.
   - [ ] [macos.md](install/macos.md): The dialog that asks whether to open Separatrix, an app downloaded from the internet, with its Open button.
   - [ ] [macos.md](install/macos.md): The "Separatrix" Not Opened dialog, with the Done and Move to Trash buttons.
   - [ ] [macos.md](install/macos.md): System Settings, Privacy & Security, with the "Separatrix" was blocked message and the Open Anyway button.
   - [ ] [macos.md](install/macos.md): The last dialog that asks whether to open Separatrix, showing the exact name of the button that opens it.
   - [ ] [windows.md](install/windows.md): The Edge downloads list with the "isn't commonly downloaded" warning and the More actions menu showing Keep.
   - [ ] [windows.md](install/windows.md): The blue Windows protected your PC window after More info, showing the app name, Publisher: Unknown publisher, and the Run anyway button.
   - [ ] [windows.md](install/windows.md): The first page of the Separatrix installer.

   **Done when** `bash scripts/release-check.sh --skip-gates` reports `0 screenshot placeholders`
   on the `install-guides` row and the `checklist` row reads `PASS`. If you skip this step for
   1.1.0, leave the boxes open: the `checklist` row passes as long as the list matches the guides.

10. [ ] **Build the final draft from `main`.** The test draft points at the commit you built in
    step 5. On the **Releases** page, delete the test draft. Then tag the latest commit and push
    the tag:

    ```sh
    git checkout main && git pull
    git tag v1.1.0
    git push origin v1.1.0
    ```

    The tag starts the Release workflow, which makes a new draft. Repeat the checksum check from
    step 6 on the new draft's files.

    **Done when** the new draft is green, its files pass the checksum check, and its tag `v1.1.0`
    points at the latest commit on `main`.

11. [ ] **Publish v1.1.0.** Open the draft and click **Edit**. Paste the `[1.1.0]` section of
    `CHANGELOG.md` as the release notes. Make sure **Set as a pre-release** is clear and **Set as
    the latest release** is selected, and click **Publish release**. The updater reads
    `releases/latest/download/latest.json`, which only a full, latest release provides.

    **Done when** the release page shows v1.1.0 as **Latest**.

12. [ ] **Check the update from 1.0.1.** On a computer with 1.0.1 installed, click its version
    number in the status bar, or **Separatrix** > **Check for Updates…** on a Mac, and install the
    update.

    **Done when** the updated app starts, says "Separatrix is up to date", and offers the
    **Parameter plane** in the View control.
