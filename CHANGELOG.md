# Changelog

All notable changes to OBCR. Versions follow [Semantic Versioning](https://semver.org).

## Unreleased

- **Mods and custom maps**: OBCR plays mods, campaigns of new floors made from the game's own walls, objects and enemies. Put a `.obcmod` file or mod folder in the mods folder, press **D** in the main menu and choose it; each mod keeps its own saves and high scores. `obcr --mod <file>` plays one directly. The format: [docs/modding.md](docs/modding.md).
- **OBCEdit**, a level editor, ships with OBCR (an option in Setup, on by default). It is a port of EC7Edit from the Corridor 7 port EC7Wolf, and lets you:
  - draw floors from a palette of the game's real walls, objects and enemies;
  - place doors, push walls, exits and the other compound features with one click;
  - check a floor as you draw, including whether it can be finished;
  - test it in OBCR with **F5**, or photograph it through OBCR's renderer;
  - export or install a mod.

  The manual: [docs/obcedit-manual.md](docs/obcedit-manual.md).

## 1.0.0-beta.2 — 2026-11-14 (pre-release)

The second beta. It adds the CD release that Steam and GOG sell, full control rebinding, and fixes push walls.

### Installers

- **Windows**: `OBCR-Setup-1.0.0-beta.2-windows-x64.exe`. **Linux** (x86-64, glibc 2.35 or newer, such as Ubuntu 22.04+): `OBCR-Setup-1.0.0-beta.2-linux-x86_64` (make it executable and run it).
- Point Setup at your Operation Body Count folder, or now at the **CD image** (`.cue`) of the Steam or GOG edition. Setup suggests the images it finds in the Steam and GOG folders. Running it over beta 1 updates the installation; settings and saved games are kept.
- Built by GitHub Actions from the tagged source. `SHA256SUMS.txt` lists the files' checksums. They are not code-signed: Windows SmartScreen warns on first run (*More info* > *Run anyway*).

### Changes

- **The CD release** (Steam, GOG; issue #1): Setup takes the BIN/CUE image directly, extracts the game files and rips the four CD music tracks. OBCR plays the CD release as its own executable did:
  - the opening, map 22 and victory movies;
  - CD music, with the original keys Alt+P (play), Alt+S (stop) and Alt+F (next track);
  - the CD build's gameplay changes (for example, grenades blow push walls open and the saucer fires at any range; full list in `docs/re/CD_RELEASE.md`).
  New options: MUSIC SOURCE (CD audio or AdLib) and CD TRACK END (play the next track, or the original's silence until the next floor). `obcr --extract-data <image or folder> <folder>` does the same as Setup from the command line.
- **Rebindable controls** (issue #2): KEY BINDINGS in OPTIONS (and after the help page) rebinds every action for keyboard and mouse (up to three inputs each, mouse buttons 4/5 and the wheel included) and for gamepads (two buttons each, triggers included).
- **Push walls** (issue #3) now move exactly as the original: twice as fast, exactly two cells, stopped by the player, and not pushable again once at rest. A wall left standing alone in a room after a push is how the original behaves too.

### Known limitations

- Behaviour follows the disassembly of the original; it has not yet been checked frame by frame against the original running in DOSBox-X.
- CD release: the damage of a flame's direct hit cannot be read from the original's code and uses a stand-in; the CD-only movies play at about 12 frames a second (the original played them as fast as the CD drive could read).
- Saved games are tied to the release they were made with: floppy saves do not load with CD files, and the other way round.
- Multiplayer has been tested on one machine (several game processes), not yet across a real network.

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
