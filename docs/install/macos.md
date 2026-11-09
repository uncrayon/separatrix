# Install Separatrix on a Mac

This guide takes you from the download to your first project. One file works on every Mac, with
Apple silicon (M1 and later) or with an Intel processor. Separatrix needs no internet connection
after you install it.

The steps match macOS 15 Sequoia and newer. Older versions differ only at first launch, and the
first-launch section says how.

## Download the file

Go to the [Separatrix releases page](https://github.com/uncrayon/separatrix/releases/latest)
and look under **Assets**. Download these two files:

- `Separatrix_1.0.1_macos_universal.dmg`, the disk image that holds the app.
- `SHA256SUMS.txt`, the list of checksums you use to check the download.

Both land in your **Downloads** folder. The names on this page are for version 1.0.1. A newer
release has its own number in each name.

## Check the download

`SHA256SUMS.txt` lists the SHA-256 checksum of every file in the release. If your file's checksum
matches, the download is complete and is the file that was published.

1. Open **Terminal**. It is in **Applications**, in the **Utilities** folder.
2. Run this command:

   ```sh
   cd ~/Downloads && shasum -a 256 Separatrix_1.0.1_macos_universal.dmg
   ```

   Terminal prints a line of 64 letters and digits, then the file name.

3. Run this command to show the published checksum:

   ```sh
   grep macos_universal.dmg SHA256SUMS.txt
   ```

4. Compare the two lines of 64 characters. They must be identical.

If they differ, delete the file and download it again. Do not open a file that fails the check.

## Install it

1. Double-click `Separatrix_1.0.1_macos_universal.dmg` in **Downloads**. A window opens with the
   Separatrix icon and the **Applications** folder.
2. Drag the Separatrix icon onto the **Applications** folder.
3. Close the window. In the Finder sidebar, click the eject button beside **Separatrix** to eject
   the disk image.

> **Screenshot to add on a real Mac:** The disk image window, with the Separatrix icon and the Applications folder side by side.

## Start Separatrix for the first time

Separatrix 1.0.1 and later are signed by their developer and notarized by Apple, so macOS knows
where the app comes from and that Apple checked it.

1. Open **Applications** and double-click **Separatrix**.
2. macOS asks whether you are sure you want to open an app downloaded from the internet. Click
   **Open**.

   > **Screenshot to add on a real Mac:** The dialog that asks whether to open Separatrix, an app downloaded from the internet, with its Open button.

macOS asks only this first time.

Separatrix 1.0.0 is not notarized, and macOS blocks it the first time you open it. If you have
1.0.0, follow [the steps for 1.0.0](#if-you-have-separatrix-100), or download the latest version
instead.

Separatrix opens on a gallery of example systems from Strogatz's *Nonlinear Dynamics and Chaos*.
Click any card to start from that system. The [Linux guide](linux.md#start-separatrix-for-the-first-time)
has a picture of the gallery. It looks the same on a Mac.

### If you have Separatrix 1.0.0

Version 1.0.0 is not notarized by Apple, so macOS blocks it the first time you open it. You allow
it once, and after that it opens like any other app.

1. Open **Applications** and double-click **Separatrix**. macOS shows a dialog titled
   **"Separatrix" Not Opened**. Click **Done**.

   > **Screenshot to add on a real Mac:** The "Separatrix" Not Opened dialog, with the Done and Move to Trash buttons.

2. Open the Apple menu and choose **System Settings**.
3. Click **Privacy & Security** in the sidebar.
4. Scroll down to the **Security** section. It says **"Separatrix" was blocked to protect your
   Mac.** Click **Open Anyway**.

   > **Screenshot to add on a real Mac:** System Settings, Privacy & Security, with the "Separatrix" was blocked message and the Open Anyway button.

5. Type your login password, or use Touch ID, and click **OK**.
6. macOS asks one last time whether to open Separatrix. Click **Open**.

   > **Screenshot to add on a real Mac:** The last dialog that asks whether to open Separatrix, showing the exact name of the button that opens it.

**Open Anyway** stays available for about an hour after step 1. If it is gone, double-click
Separatrix again and repeat the steps.

On macOS 14 Sonoma or older, there is a shorter way. Control-click **Separatrix** in
**Applications**, choose **Open**, then click **Open** in the dialog.

### If macOS says Separatrix is damaged

If macOS says **"Separatrix" is damaged and can't be opened**, or, with version 1.0.0,
**Open Anyway** never appears, remove the quarantine flag that macOS put on the download:

1. Open **Terminal**.
2. Run this command:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Separatrix.app
   ```

3. Double-click **Separatrix** in **Applications** again.

The command changes only the Separatrix app. It does not turn off any protection for other apps.

## Open a project

Separatrix saves projects as `.sepx` files. You can open one in four ways:

- Click **Open…** at the top right of the gallery, or press **⌘O** at any time, then choose the
  file.
- Drag a `.sepx` file from the Finder onto the Separatrix window.
- Drag a `.sepx` file onto the Separatrix icon in the Dock.
- Double-click a `.sepx` file in the Finder.

The project opens with its equations, trajectories and view as they were saved. The gallery also
lists the projects you opened most recently under **Recent**. Click one to open it again.

## Update Separatrix

Separatrix 1.0.0 cannot update itself. If you have 1.0.0, install the latest version once by
following this guide from the top. From 1.0.1 on, updates install themselves.

Separatrix checks for a new version after it starts. When one exists, the status bar at the bottom
of the window shows **Update available** beside the version number, such as `v1.0.1`.

1. Click the version number. You can also choose **Separatrix** > **Check for Updates…** in the
   menu bar at any time.
2. Click **Install and restart**.

## Uninstall Separatrix

1. Quit Separatrix.
2. Open **Applications** and drag **Separatrix** to the Trash.

Uninstalling keeps your `.sepx` projects. To also delete the list of recent projects, delete the
folder `~/Library/Application Support/io.github.uncrayon.separatrix`.
