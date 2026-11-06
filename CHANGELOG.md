# Changelog

All notable changes to OBCR. Versions follow [Semantic Versioning](https://semver.org).

## 1.0.0-beta.1 — 2026-11-06 (pre-release)

The first beta of OBCR, a modern source port of Operation Body Count (1994) written in Rust on SDL 3. It needs your own copy of the game (the floppy release, files dated 1996-12-24).

### Installers

- **Windows**: `OBCR-Setup-1.0.0-beta.1-windows-x64.exe`. **Linux** (x86-64, glibc 2.35 or newer, such as Ubuntu 22.04+): `OBCR-Setup-1.0.0-beta.1-linux-x86_64` (make it executable and run it).
- Setup installs for your user account only (no administrator rights): you point it at your Operation Body Count folder, it checks the files and copies the game plus the files it uses into `%LOCALAPPDATA%\Programs\OBCR` or `~/.local/opt/OBCR`, with menu entries and an uninstaller. Running it again updates or removes OBCR; your settings and saved games are kept unless you choose otherwise.
- The files are not code-signed: Windows SmartScreen warns on first run (*More info* > *Run anyway*).

### The game

- **The game**, ported from the original executable's code: weapons, enemies and bosses, allies with follow, disperse and body switching, destructible walls, fire, booby traps, tremors, lighting, the overhead map and the status bar, the menus, sequences (cutscenes), saved games, high scores and the original cheat parameter. Floors 1–23 (the first building) are covered by automated play tests; the later floors load but have not been tested yet.
- **Classic mode**: the original 320×200 picture and behaviour.
- **Enhanced mode** (default): 2–8× resolution, widescreen (16:10, 16:9, 21:9), smooth motion between game frames, modern controls (WASD and mouse) and gamepads with rumble.
  - **Smooth floors**: floors and ceilings in true perspective, without the original's grout lines and per-tile banding.
  - **xBRZ upscaling** (2×–6×) for walls, floors, sprites and the weapon, and for menus, pictures and cutscenes; shading and palette effects still apply. The status bar can use it too (off by default).
- **Multiplayer** over UDP: the original's network game types N (deathmatch), T (teams) and M (two players), on a LAN or over the Internet, up to 12 players.
- **Fixes** to the original that keep every floor finishable: blasts no longer destroy exits or seal doors, and kills by allies count toward the exit requirement.

### Known limitations

- Behaviour follows the disassembly of the original; it has not yet been checked frame by frame against the original running in DOSBox-X.
- Multiplayer has been tested on one machine (several game processes), not yet across a real network.
- Only the floppy release's data is recognised; the Steam release has not been verified.
- No macOS build; the Windows game and shortcuts use the generic program icon.
