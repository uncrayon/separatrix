# Build the installers on your own machine

The Release workflow (`.github/workflows/release.yml`) builds the installers that ship. This page
is for building them yourself, to try a change in the real app before you push it. Each OS builds
its own installers: Tauri cannot build a Windows installer on a Mac, for example. From a Mac you
can still build all three, with Docker for Linux and GitHub Actions for Windows.

| You want | Run it on | Command | Output in `target/` |
| --- | --- | --- | --- |
| macOS `.app` and `.dmg` | a Mac | [macOS](#macos) | `release/bundle/macos/`, `release/bundle/dmg/` |
| Linux `.deb`, `.rpm`, AppImage | Linux, or any machine with Docker | [Linux](#linux) | `release/bundle/{deb,rpm,appimage}/`, or `dist/linux/` from Docker |
| Windows `-setup.exe` and `.msi` | Windows, or GitHub Actions | [Windows](#windows) | `release/bundle/{nsis,msi}/` |
| All three at once | GitHub Actions | [All platforms](#all-platforms-from-github-actions) | the run's artifacts |

## Before you start, on every OS

- **Rust 1.90 or newer**, from [rustup](https://rustup.rs), and **Node 22**.
- Run `npm ci` in `app/` once, and again whenever `app/package-lock.json` changes.
- **The updater key.** `tauri.conf.json` turns on the signed updater, so a plain
  `npx tauri build` builds everything and then stops with "A public key has been found, but no
  private key". Only the Release workflow holds that key (see [updater.md](updater.md)). Locally,
  turn the updater files off for the build:

  ```sh
  npx tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}'
  ```

  The installers work the same. They just can't serve as an update feed. Every command below
  includes this flag.
- The version in the file names comes from `Cargo.toml`, so a local build of main is named after
  the last release (for example `Separatrix_1.0.1_aarch64.dmg`) until the version is bumped.

## macOS

```sh
cd app
npx tauri build --bundles app,dmg --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

This builds for the Mac's own chip, in about 3 minutes on an M-series Mac with a warm cache:
`target/release/bundle/macos/Separatrix.app` and
`target/release/bundle/dmg/Separatrix_<version>_aarch64.dmg`.

- **The release's universal app (Intel and Apple silicon).** Add both targets once, then build
  with the release's arguments. The output moves to `target/universal-apple-darwin/release/bundle/`.

  ```sh
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  cd app
  npx tauri build --target universal-apple-darwin --bundles app,dmg --config '{"bundle":{"createUpdaterArtifacts":false}}'
  ```

- **Signing.** A local build gets an ad-hoc signature (`signingIdentity: "-"`) and is not
  notarized. It runs on the Mac that built it. Another Mac's Gatekeeper refuses it unless you
  right-click it and choose Open. Developer ID signing and notarization happen only in the
  Release workflow (see [macos-signing.md](macos-signing.md)).
- **Homebrew's rustup.** If `cargo` is "not found" but `rustup` works, Homebrew installed rustup
  without putting its tools on your PATH. Add them for the session:

  ```sh
  export PATH="$(brew --prefix rustup)/bin:$PATH"
  ```

## Linux

**On Ubuntu 22.04 or Debian 12**, install the system packages once, then build:

```sh
sudo apt install build-essential file rpm patchelf libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
cd app
npx tauri build --bundles deb,rpm,appimage --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

The packages land in `target/release/bundle/deb/`, `rpm/` and `appimage/`. Build on the oldest
distribution you want to support: an AppImage built on a newer glibc won't start on an older one.
The release uses Ubuntu 22.04.

**From a Mac, or any machine with Docker**, the script below builds them in an Ubuntu 22.04
container. It works on a copy of the repository, so your own `target/` and `app/node_modules`
are left alone:

```sh
bash scripts/build/linux-in-docker.sh
```

- The packages land in `dist/linux/`.
- The default builds x86_64 packages, like the release. On Apple silicon they build under
  emulation: about 10 minutes the first time, faster afterwards, because Docker volumes keep the
  toolchain and the build.
- Under emulation you get the `.deb` and `.rpm` but no AppImage: the AppImage tools are AppImages
  themselves and don't run emulated. For the x86_64 AppImage, use an x86_64 Linux machine or
  GitHub Actions below.
- `ARCH=arm64 bash scripts/build/linux-in-docker.sh` builds arm64 packages, including the
  AppImage, natively on Apple silicon, for an arm64 Linux machine or VM. This path hasn't been
  tried yet.
- To free the disk space afterwards: `docker volume rm $(docker volume ls -q | grep separatrix-linux)`.

To check the packages the way a user installs them, run `bash scripts/smoke/linux-bundle.sh` on
Linux (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## Windows

**On Windows 10 or 11**, install once:
- Visual Studio Build Tools with the "Desktop development with C++" workload, which `rustup` asks
  for on its first run;
- Rust from rustup, and Node 22;
- WebView2, which Windows 11 already has.

Then, in PowerShell or Git Bash:

```sh
cd app
npx tauri build --bundles nsis,msi --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

The installers land in `target\release\bundle\nsis\` (`Separatrix_<version>_x64-setup.exe`) and
`target\release\bundle\msi\`.

- In PowerShell, write the `--config` value with doubled inner quotes,
  `'{""bundle"":{""createUpdaterArtifacts"":false}}'`, or use Git Bash, where the line above works
  as written.
- The release's offline installer, which carries WebView2 for a PC with no internet, adds one
  setting:

  ```sh
  npx tauri build --bundles nsis --config '{"bundle":{"createUpdaterArtifacts":false,"windows":{"webviewInstallMode":{"type":"offlineInstaller"}}}}'
  ```

- The installers are unsigned, so SmartScreen warns on another PC. That is expected for a local
  build. The install guide ([install/windows.md](install/windows.md)) shows what users see.

**From a Mac**, use GitHub Actions below. A Windows VM works too, but on Apple silicon it runs
Windows on ARM, where building the x64 installers the release ships needs extra setup that this
page doesn't cover.

## All platforms from GitHub Actions

The Release workflow builds every installer on GitHub's machines. You can run it without
publishing anything:

1. Push your branch.
2. On GitHub, open **Actions → Release → Run workflow**, pick the branch, and **clear "Create or
   refresh the draft release"**.
3. When the run is green, download `bundle-macos`, `bundle-linux`, `bundle-windows` and
   `bundle-windows-offline` at the bottom of the run's page. They keep for 7 days.

These builds are named and signed exactly like a release: Developer ID and notarized on macOS when
the Apple secrets exist, plus updater signatures. With the GitHub CLI:

```sh
gh workflow run release.yml --ref <your-branch> -f publish=false
gh run list --workflow release.yml --limit 1   # note the run's ID
gh run watch <run-id>
gh run download <run-id> --name bundle-windows
```

With publish cleared, no tag, release or draft is created.
