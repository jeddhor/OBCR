# OBCR Setup

A Qt 6 installer and uninstaller for OBCR on Windows and Linux (ADR-021). It installs for the current user only (no administrator rights), checks the user's own copy of Operation Body Count, and copies the game plus the game files it uses into one folder.

## What it does

1. **Game files**: the user points it at their Operation Body Count folder. Setup runs the bundled game with `obcr --check-data <dir>`, which identifies the files by SHA-256 against the known releases (`obc-formats::release`), and only continues when every required file matches. The Steam default locations are suggested when present.
2. **Folder**: by default `%LOCALAPPDATA%\Programs\OBCR` on Windows and `~/.local/opt/OBCR` on Linux. It must be new, empty or an earlier OBCR installation (which is then replaced; settings and saved games are kept).
3. **Install**:

   ```text
   <folder>/obcr[.exe]               the game
   <folder>/data/*.BC, COUNT.EXE     the required files and any sequences (F2.BC ...), nothing else
   <folder>/obcr.png                 icon (drawn by Setup, not taken from the game)
   <folder>/uninstall/               OBCR-Uninstall[.exe] and its Qt runtime
   <folder>/install-manifest.txt     everything Setup created, for the uninstaller
   ```

   - Windows: a Start menu folder (OBCR, Uninstall OBCR), an optional desktop shortcut, and an entry in *Settings > Apps* (`HKCU\...\Uninstall\OBCR`).
   - Linux: `obcr.desktop` and `obcr-uninstall.desktop` in `~/.local/share/applications`, an optional desktop launcher, and a `~/.local/bin/obcr` link (never replacing a file that is not a link).

   The game finds `data/` next to itself, so no settings are written. A failed installation removes what it created.
4. **Uninstall**: *Settings > Apps*, the *Uninstall OBCR* menu entry, `uninstall/OBCR-Uninstall`, or Setup again (it offers *Update* or *Remove* when OBCR is installed). It removes exactly what the manifest lists. Settings and saved games (written by the game in `%APPDATA%\OBCR`, or `~/.config/obcr` and `~/.local/share/obcr`) are kept unless the user ticks the box (`--purge` unattended). On Windows the uninstaller first copies itself to a temporary folder, because a running program cannot delete itself.

## Command line

```text
OBCR-Setup                                  the wizard
OBCR-Setup --unattended --data DIR [--target DIR] [--desktop-shortcut] [--no-menu] [--no-path-link]
OBCR-Setup --uninstall [--target DIR] [--unattended] [--purge]
OBCR-Uninstall [--unattended] [--purge]     the installed uninstaller
OBCR-Setup --screenshots DIR [--data DIR]   development: render the pages to PNG
```

Unattended runs print progress on standard output and exit with 0 on success, 1 on failure, 2 when the game files are incomplete.

## Building

Each script builds the release game and Setup, bundles the Qt runtime, and packs everything into **one self-extracting file** in `target/dist/`. The `obcr-setup` launcher (`crates/obcr-setup`) carries the Setup folder as a deflate-compressed archive (`cargo xtask pack-installer`), unpacks it into a temporary folder, runs Setup and removes the folder afterwards.

**Windows** (Qt 6 MinGW kit from the Qt online installer, Rust, CMake):

```powershell
powershell -ExecutionPolicy Bypass -File installer\build-windows.ps1   # -Qt, -MinGW, -Ninja to override paths
# -> target\dist\OBCR-Setup-<version>-windows-x64.exe (about 15 MB)
```

**Linux** (build on an older distribution, such as Ubuntu 22.04, so the result runs on newer ones):

```bash
sudo apt install build-essential cmake ninja-build patchelf qt6-base-dev qt6-wayland \
    libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxfixes-dev libxi-dev libxss-dev libxtst-dev \
    libxkbcommon-dev libwayland-dev wayland-protocols libdecor-0-dev libegl1-mesa-dev libgl1-mesa-dev \
    libgles2-mesa-dev libdrm-dev libgbm-dev libasound2-dev libpulse-dev libpipewire-0.3-dev \
    libdbus-1-dev libudev-dev libibus-1.0-dev
installer/build-linux.sh
# -> target/dist/OBCR-Setup-<version>-linux-x86_64 (about 27 MB)
```

The Linux bundle carries the Qt libraries, ICU and the xcb helper libraries; the C runtime, graphics drivers, X11/Wayland core, fonts, GLib and D-Bus come from the system, as with AppImages.

## Licences

Setup is part of OBCR (GPL-3.0-or-later). It links Qt dynamically (LGPL-3.0) and ships the Qt libraries unmodified next to itself.
