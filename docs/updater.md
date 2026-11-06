# Turn on automatic updates

Separatrix can check GitHub Releases for a newer version, download it, check its signature and
install it. The status bar's version button (`v1.0.0`) runs the check, and so does
**Check for Updates…** in the macOS app menu. A quiet check also runs once after launch.

The updater stays off until you, the owner, give the app a signing key. A build without a key
shows "Updates are off in this build" and never contacts GitHub. The repository holds no key,
real or placeholder, so a build from a fresh checkout is always off.

Only the owner can do the steps on this page, because they create and keep the private key.

## Know what the key protects

The updater installs a file only if it carries a signature made with your private key. The app
checks that signature with the public key built into it.

- Never commit the private key, or paste it into an issue, a chat or a workflow file.
- If you lose the private key or its password, the copies people have installed can never update
  themselves again. Each person would have to download and install a new build by hand.
- A copy built without the public key can never update itself either. Add the key before you
  build the release that people will install, so that v1.0.0 can update to v1.0.1.

## Create the key

1. In a terminal, go to the `app` folder of the repository and run this command:

   ```sh
   npx tauri signer generate -w ~/.tauri/separatrix.key
   ```

2. Type a strong password when the command asks for one, and type it again to confirm it.
3. Check that the command wrote two files: `~/.tauri/separatrix.key` (the private key) and
   `~/.tauri/separatrix.key.pub` (the public key).

The private key file is outside the repository, so `git` never sees it.

## Keep two offline backups

Store the private key file and its password in two places that are not this computer:

1. An encrypted USB drive. Copy `~/.tauri/separatrix.key` to it, and keep the drive somewhere safe.
2. A password manager, or a printed copy kept with your important papers. Store the password, and
   the text of `separatrix.key` too.

Check both backups before you go on. Open the file from the USB drive and confirm that its text
matches `~/.tauri/separatrix.key`.

## Add the key to GitHub

The Release workflow signs the update files with two repository secrets.

1. On GitHub, open the repository and go to **Settings**, then **Secrets and variables**, then
   **Actions**.
2. Click **New repository secret**. Name it `TAURI_SIGNING_PRIVATE_KEY`, and paste the whole text
   of `~/.tauri/separatrix.key` as its value.
3. Click **New repository secret** again. Name it `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and type
   the password as its value.

The build step of `.github/workflows/release.yml` must pass both secrets to `tauri-action` as
environment variables of the same names. Without them, a build with `createUpdaterArtifacts`
fails because it cannot sign the update files.

## Put the public key in the app

1. Open `app/src-tauri/tauri.conf.json`.
2. Add a `plugins` section at the top level, and paste the whole one-line text of
   `~/.tauri/separatrix.key.pub` as the `pubkey`:

   ```json
   "plugins": {
     "updater": {
       "pubkey": "PASTE THE CONTENTS OF separatrix.key.pub HERE",
       "endpoints": [
         "https://github.com/uncrayon/slope-field-vizualiser/releases/latest/download/latest.json"
       ],
       "windows": {
         "installMode": "passive"
       }
     }
   }
   ```

3. In the existing `bundle` section, add this line:

   ```json
   "createUpdaterArtifacts": true
   ```

4. Commit `tauri.conf.json`. The public key is safe to publish.

## Check that the updater is on

1. Run `bash scripts/release-check.sh --skip-gates`. The `updater-off-without-key` row reads
   `PASS  updater-off-without-key: a minisign pubkey is configured, so the updater is on`. If it
   reads "is not a minisign public key", you pasted the wrong file or only part of it.
2. Run the Release workflow. The draft release now has a `.sig` file beside each installer, and a
   `latest.json` that lists them. `tauri-action` writes `latest.json` when the build made
   signatures.
3. Publish the release as a full release, not a draft or a pre-release. The endpoint
   `releases/latest/download/latest.json` points at the newest full release only, so the app
   sees a draft or a pre-release as no update.

To test an update, install the published release on a computer, then publish a newer version.
When the installed copy starts, its quiet check finds the new version and shows
"Update available" beside its version.
Click the version, then click **Install and restart**.

## What the app does with the key

`app/src-tauri/src/updater.rs` reads the bundled config once at startup. With a non-empty
`plugins.updater.pubkey`, it registers `tauri-plugin-updater` and the version button can check
and install. Without one, it registers nothing and every check answers "off". Each check gives
up after 15 seconds. The check after launch runs in the background, so a computer with no network
opens the gallery as fast as one with a network. That check reports only a newer version. It
writes failures to the terminal, not to the window.
