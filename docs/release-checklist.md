# Release checklist for v1.0.0

These are the steps that only the owner can take, because they need the GitHub and PyPI accounts,
the updater's private key, or a real Mac and a real Windows PC. Do them in order. Tick each box when
its **Done when** line is true.

Before you start, run `bash scripts/release-check.sh` on `main`. Every row must read `PASS`.

To sign and notarize the Mac app, add the five Apple secrets in
[Sign and notarize the Mac app](macos-signing.md) before the build that people will install.

1. [ ] **Add the updater key.** Follow [Turn on automatic updates](updater.md): create the key, keep
   two offline backups, add the `TAURI_SIGNING_PRIVATE_KEY` and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets, then put the public key in
   `app/src-tauri/tauri.conf.json` and push the commit to `main`. Do this before any build that
   people will install. A copy built without the key can never update itself.

   **Done when** `bash scripts/release-check.sh --skip-gates` prints
   `PASS  updater-off-without-key: a minisign pubkey is configured, so the updater is on`, and both
   secrets appear under **Settings** > **Secrets and variables** > **Actions**.

2. [ ] **Run the Release workflow for a test build.** On GitHub, open **Actions**, click
   **Release**, click **Run workflow**, choose `main`, and click **Run workflow**. The run ends with
   a draft release named **Separatrix 1.0.0**.

   **Done when** all five jobs of the run are green and the draft release lists seven installers,
   `Separatrix_1.0.0_macos_universal.app.tar.gz`, the `.sig` signature files, `latest.json` and
   `SHA256SUMS.txt`.

3. [ ] **Download the draft and check the checksums.** Open the draft on the **Releases** page,
   download every file into one empty folder, and run `sha256sum -c SHA256SUMS.txt` in that folder
   (on a Mac, `shasum -a 256 -c SHA256SUMS.txt`).

   **Done when** every line ends in `OK` and `latest.json` lists seven platforms: `linux-x86_64`
   (the AppImage), `linux-x86_64-deb`, `linux-x86_64-rpm`, `windows-x86_64` (the setup `.exe`),
   `windows-x86_64-msi`, `darwin-x86_64` and `darwin-aarch64`.

4. [ ] **Test on a real Mac with macOS Sequoia.** Follow [the macOS guide](install/macos.md) word for
   word with the draft's `.dmg`, and correct any step or button name that differs. Then check that
   each of these works:
   - **Open…** in the gallery, **⌘O**, dragging a `.sepx` onto the window and onto the Dock icon,
     and double-clicking a `.sepx` in the Finder.
   - The **Recent** list in the gallery after you quit and start Separatrix again.
   - Choosing a value from a dropdown, such as **Solver**, and then clicking the plot once adds a
     trajectory on that first click.
   - **Separatrix** > **Check for Updates…** in the menu bar opens the update panel. Until v1.0.0
     is published, the panel says it couldn't check for updates. That is expected.
   - With Wi-Fi turned off, Separatrix starts as fast as with it on.

   **Done when** every step in the macOS guide worked as written on the Mac, and every check above
   passed.

5. [ ] **Test on a real Windows PC, online and offline.** Follow [the Windows guide](install/windows.md)
   twice. First install the standard installer on a PC with an internet connection. Then uninstall
   it, disconnect the PC from the network, and install the offline installer. If you can, use a PC
   or a virtual machine that has no WebView2 for the offline run. Check the same list as for the
   Mac, with **Ctrl+O** in place of **⌘O** and File Explorer in place of the Finder.

   **Done when** both installs started Separatrix and opened a project, the offline install worked
   with no network, and every step in the Windows guide worked as written.

6. [ ] **Run the professor test (the M1 gate).** Give a colleague who has not seen Separatrix the
   draft's installer for their own computer and one task, such as "show where the rabbits and the
   sheep can live together, and export the figure for a paper". Do not help or explain. Watch, and
   write down every place they stop, guess or ask. Fix what blocks them before you publish, or list
   it as a known issue in the release notes.

   **Done when** the colleague installed Separatrix, ran a first simulation and exported a figure
   without help, and each place they got stuck is either fixed on `main` or listed in the release
   notes.

7. [ ] **Replace the screenshot placeholders.** While you test on the Mac and the PC, take each
   screenshot below. Save it in `docs/install/images/` with a name such as `macos-open-anyway.png`,
   and replace its `> **Screenshot to add on a real ...**` line in the guide with an image link to
   that file, written the same way as the images in the Linux guide. Tick each box
   here as you replace its line, so the list stays equal to the placeholders still in the guides.
   - [ ] [macos.md](install/macos.md): The disk image window, with the Separatrix icon and the Applications folder side by side.
   - [ ] [macos.md](install/macos.md): The dialog that asks whether to open Separatrix, an app downloaded from the internet, with its Open button.
   - [ ] [macos.md](install/macos.md): The "Separatrix" Not Opened dialog, with the Done and Move to Trash buttons.
   - [ ] [macos.md](install/macos.md): System Settings, Privacy & Security, with the "Separatrix" was blocked message and the Open Anyway button.
   - [ ] [macos.md](install/macos.md): The last dialog that asks whether to open Separatrix, showing the exact name of the button that opens it.
   - [ ] [windows.md](install/windows.md): The Edge downloads list with the "isn't commonly downloaded" warning and the More actions menu showing Keep.
   - [ ] [windows.md](install/windows.md): The blue Windows protected your PC window after More info, showing the app name, Publisher: Unknown publisher, and the Run anyway button.
   - [ ] [windows.md](install/windows.md): The first page of the Separatrix installer.

   **Done when** `bash scripts/release-check.sh --skip-gates` reports `0 screenshot placeholders`
   on the `install-guides` row and the `checklist` row reads `PASS`.

8. [ ] **Set the release date in the changelog.** In `CHANGELOG.md`, replace `unreleased` in the
   `## [1.0.0]` heading with today's date, written as `YYYY-MM-DD`. Commit the date and the
   screenshots, and push them to `main`.

   **Done when** `main` on GitHub shows the dated heading and the new screenshots.

9. [ ] **Build the final draft from `main`.** The test draft points at the commit you built in
   step 2, so its tag would miss the screenshots and the date. On the **Releases** page, delete the
   test draft. Then tag the latest commit and push the tag:

   ```sh
   git checkout main && git pull
   git tag v1.0.0
   git push origin v1.0.0
   ```

   The tag starts the Release workflow, which makes a new draft. Repeat the checksum check from
   step 3 on the new draft's files.

   **Done when** the new draft is green, its files pass the checksum check, and its tag `v1.0.0`
   points at the commit from step 8.

10. [ ] **Publish v1.0.0.** Open the draft and click **Edit**. Make sure
   **Set as a pre-release** is clear and **Set as the latest release** is selected, and click **Publish release**. The updater reads
   `releases/latest/download/latest.json`, which only a full, latest release provides.

   **Done when** the release page shows v1.0.0 as **Latest**, and a copy installed from it says
   "Separatrix is up to date" when you click its version number.

11. [ ] **Post the PyPI notice.** Follow [the PyPI deprecation steps](pypi-deprecation.md) to
    publish `equation-phase-portrait-tool` 0.1.1 with the notice, then archive the project.

    **Done when** <https://pypi.org/project/equation-phase-portrait-tool/> shows version 0.1.1 with
    the notice and the archived banner, and version 0.1.0 is still installable.

12. [ ] **Run the class beta.** Send the beta class a link to the v1.0.0 release and the three
    install guides. Ask each student to install Separatrix on their own computer, open a project,
    and report any problem as a GitHub issue.

    **Done when** every student in the beta has opened a project in Separatrix on their own
    computer, and each problem they reported is a GitHub issue.
