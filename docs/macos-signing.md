# Sign and notarize the Mac app

The Release workflow signs the Mac app with your Developer ID certificate and has Apple notarize
it, so macOS opens it after one "downloaded from the internet" question instead of blocking it.
It does this only when the five secrets on this page exist. Without them, the app gets an ad-hoc
signature, as version 1.0.0 did, and the workflow says so in a warning.

Only the owner can do the steps on this page, because they need the Apple Developer account.

## What you need

- A paid Apple Developer Program membership, as an individual. It costs $99 USD a year.
- A **Developer ID Application** certificate with its private key, exported as a `.p12` file with
  a password. Only the account holder can create one, at
  [Certificates, Identifiers & Profiles](https://developer.apple.com/account/resources/certificates).
  Choose **G2 Sub-CA** when it asks for a profile type.
- An App Store Connect API key with **Developer** access, for notarization. Create it in
  [App Store Connect](https://appstoreconnect.apple.com) under **Users and Access** >
  **Integrations** > **App Store Connect API** > **Team Keys**. Apple lets you download its `.p8`
  file only once.

Keep the `.p12`, its password and the `.p8` file in two offline backups, as with the
[updater key](updater.md). Anyone who has them can sign apps in your name. If they leak, revoke
the certificate and the key on the same pages and make new ones.

## Add the secrets to GitHub

On GitHub, open the repository's **Settings** > **Secrets and variables** > **Actions**, and add
these repository secrets:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | The `.p12` file in base64, from `base64 -i separatrix-devid.p12 -o separatrix-devid.p12.b64` |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12` password |
| `APPLE_API_ISSUER` | The Issuer ID shown above the keys table in App Store Connect |
| `APPLE_API_KEY` | The key's Key ID |
| `APPLE_API_KEY_P8` | The whole text of the `.p8` file, with its BEGIN and END lines |

Set all five or none. If only some exist, the macOS build stops with an error that names the
missing ones, so a release can never ship signed but not notarized.

## Check a build

1. On GitHub, open **Actions**, click **Release**, click **Run workflow**, clear **Create or
   refresh the draft release**, and click **Run workflow**. This builds and checks the installers
   without touching any release.
2. Open the **Bundle (macos)** job. The step **Check the macOS signature and notarization** must be
   green. It prints the certificate's name, `Authority=Developer ID Application: …`, and fails if
   the app is not signed with it, lacks the hardened runtime, or has no stapled notarization
   ticket.
3. To check on a real Mac, download the `bundle-macos` artifact at the bottom of the run's page,
   open the `.dmg` from it, drag Separatrix to **Applications** and open it. macOS asks only
   whether you want to open an app downloaded from the internet.

## Renew the certificate

A Developer ID Application certificate expires. Keychain Access shows the date: for the first one,
9 December 2027. Versions you released before that keep opening, because each carries a
timestamp and a notarization ticket. To sign new versions after it, create a new certificate the
same way, export it, and replace `APPLE_CERTIFICATE` and `APPLE_CERTIFICATE_PASSWORD`.

The API key does not expire. Replace it only if you revoke it.

## How the workflow does it

The step **Apple signing and notarization** in `.github/workflows/release.yml` checks the secrets,
writes the `.p8` file to the runner's temporary folder, and installs Apple's Developer ID G2
intermediate certificate. It sets `APPLE_SIGNING_IDENTITY` to `Developer ID Application`, which
overrides the ad-hoc `"signingIdentity": "-"` in `app/src-tauri/tauri.conf.json`. Tauri then
imports the certificate into a temporary keychain, signs the app with the hardened runtime, sends
it to Apple's notary service, and staples the ticket to the app before it builds the `.dmg`.
