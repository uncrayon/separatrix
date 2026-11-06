# Install Separatrix on Windows

This guide takes you from the download to your first project. You need a 64-bit PC with
Windows 10 or Windows 11. The installer puts Separatrix in your own user account, so you do not
need an administrator password. Separatrix needs no internet connection after you install it.

## Choose an installer

Go to the [Separatrix releases page](https://github.com/uncrayon/slope-field-vizualiser/releases/latest)
and look under **Assets**. Windows has three files. Most people want the first one.

| File | Use it when |
| --- | --- |
| `Separatrix_1.0.0_windows_x64-setup.exe` | You have an internet connection while you install. This is the standard installer. |
| `Separatrix_1.0.0_windows_x64_offline-setup.exe` | The PC has no internet connection, or it cannot download WebView2. This offline installer is much larger. |
| `Separatrix_1.0.0_windows_x64.msi` | Your IT department installs software for everyone on a PC. The MSI needs an administrator. |

Separatrix draws its window with Microsoft Edge WebView2. Windows 11 always has it, and Windows 10
gets it through Windows Update. If the PC lacks it, the standard installer downloads it, and that
needs the internet. The offline installer carries WebView2 inside it, so it works on a PC with
neither WebView2 nor an internet connection. If you are not sure, use the offline installer. It
works on every PC.

Download `SHA256SUMS.txt` too. The names on this page are for version 1.0.0. A newer release has
its own number in each name.

### If Edge blocks the download

Microsoft Edge may say that the installer **isn't commonly downloaded**. The file is safe to keep
once it passes the check in the next section.

1. In the downloads list, point to the file and click **...** (More actions).
2. Click **Keep**.
3. Click **Show more**, then click **Keep anyway**.

> **Screenshot to add on a real Windows PC:** The Edge downloads list with the "isn't commonly downloaded" warning and the More actions menu showing Keep.

## Check the download

`SHA256SUMS.txt` lists the SHA-256 checksum of every file in the release. If your file's checksum
matches, the download is complete and is the file that was published.

1. Open the **Start** menu, type `PowerShell`, and press Enter.
2. Go to your **Downloads** folder:

   ```powershell
   cd $HOME\Downloads
   ```

3. Run this command. If you chose another installer, put its name in place of the standard one:

   ```powershell
   Get-FileHash .\Separatrix_1.0.0_windows_x64-setup.exe -Algorithm SHA256
   ```

   PowerShell prints the checksum under **Hash**, a line of 64 letters and digits.

4. Open `SHA256SUMS.txt` in Notepad and find the line for your installer.
5. Compare the two checksums. They must be identical, except that PowerShell prints capital
   letters and the file has small ones.

If they differ, delete the file and download it again. Do not run a file that fails the check.

## Install it

1. Double-click the installer in your **Downloads** folder.
2. Windows may show a blue window titled **Windows protected your PC**. Separatrix is not signed
   with a Microsoft certificate, so Windows does not know it yet. Click **More info**, then click
   **Run anyway**.

   > **Screenshot to add on a real Windows PC:** The blue Windows protected your PC window after More info, showing the app name, Publisher: Unknown publisher, and the Run anyway button.

3. Follow the installer. Click **Next** until you see **Install**, click **Install**, then click
   **Finish**. The defaults are fine.

   > **Screenshot to add on a real Windows PC:** The first page of the Separatrix installer.

If you used the MSI, Windows asks for an administrator password before it installs.

## Start Separatrix for the first time

Open the **Start** menu and click **Separatrix**. If the installer's last page offered to run
Separatrix and you left that selected, it is already open.

Separatrix opens on a gallery of example systems from Strogatz's *Nonlinear Dynamics and Chaos*.
Click any card to start from that system. The [Linux guide](linux.md#start-separatrix-for-the-first-time)
has a picture of the gallery. It looks the same on Windows.

## Open a project

Separatrix saves projects as `.sepx` files. You can open one in three ways:

- Click **Open…** at the top right of the gallery, or press **Ctrl+O** at any time, then choose the
  file.
- Drag a `.sepx` file from File Explorer onto the Separatrix window.
- Double-click a `.sepx` file in File Explorer.

The project opens with its equations, trajectories and view as they were saved. The gallery also
lists the projects you opened most recently under **Recent**. Click one to open it again.

## Update Separatrix

Separatrix checks for a new version after it starts. When one exists, the status bar at the bottom
of the window shows **Update available** beside the version number, such as `v1.0.0`.

1. Click the version number. You can click it at any time to check by hand.
2. Click **Install and restart**. A small progress window appears while the update installs, and
   then Separatrix starts again.

If you installed with the MSI, ask your IT department to install the new MSI instead.

## Uninstall Separatrix

1. Open **Settings**, then **Apps**, then **Installed apps**. On Windows 10, open **Settings**,
   then **Apps**.
2. Find **Separatrix** in the list.
3. Click **...** beside it, or click the app on Windows 10, then click **Uninstall**.

Uninstalling keeps your `.sepx` projects. To also delete the list of recent projects, delete the
folder `%APPDATA%\io.github.uncrayon.separatrix`.
