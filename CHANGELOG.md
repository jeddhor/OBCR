# Changelog

All notable changes to OBCR. Versions follow [Semantic Versioning](https://semver.org).

## Unreleased

- **Push walls** (#17): a push wall seen on its east or west face no longer flips its texture when it starts to move (the original mirrors it the other way round; north and south faces were already right).
- **GAMEPLAY FIXES menu** (#16): readable. Each fix takes two lines, and the selected fix's description is at the bottom, wrapped to the box.

## 1.0.0-beta.5 — 2026-11-19 (pre-release)

The fifth beta: fixes for the issues reported on beta 4 (#5–#15), opt-in fixes for bugs of the original game, and new FRAME RATE and SENSOR: DEAD options. The investigation behind each change: [docs/ISSUES_M19.md](docs/ISSUES_M19.md).

### Installers

- **Windows** (Windows 10 or 11): `OBCR-Setup-1.0.0-beta.5-windows-x64.exe`. **Linux** (x86-64, glibc 2.35 or newer, such as Ubuntu 22.04+): `OBCR-Setup-1.0.0-beta.5-linux-x86_64` (make it executable and run it).
- Running Setup over an earlier beta updates the installation; settings, saved games and high scores are kept.
- Built by GitHub Actions from the tagged source. `SHA256SUMS.txt` lists the files' checksums. They are not code-signed: Windows SmartScreen warns on first run (*More info* > *Run anyway*).

### Changes

- **Door sounds** (#15): every door, elevator doors included, played the blood-splat sounds instead of the original's door sound.
- **Far-away sounds** (#9): like the original, distant creature calls (sounds 50 and up) are no longer heard across the whole floor.
- **Lights after loading** (#11): a loaded floor keeps the lights it was saved with; before, it took floor 1's, so a lit floor could load dark and the switch needed two uses.
- **Floor 4's sewer man** (#10) wakes from gunfire as in DOS. He stands on a tile that gives him an area number past the end of the game's table, and the original's out-of-range read always says he is near the player; OBCR now reproduces it.
- **FRAME RATE** option (#13): the original computed as many frames as the computer could, and a few things ran once per frame (the weapon's bob, a booby trap's knock-back, a cap on running). OBCR played like a machine at 35 frames a second; choose 17 or 14 to match a slower DOS machine. Walking, enemies and doors are unchanged: the game clock is 70 Hz, as in the original.
- **GAMEPLAY FIXES** (OPTIONS): opt-in fixes for bugs of the original, off by default and kept with each saved game:
  - K-001 KEEP HEAT SENSOR ON LOAD (#12);
  - K-002 PICK UP ITEMS UNDERFOOT (#7);
  - K-003 KEEP WEAPON ON PICKUP (#14);
  - K-004 SEE THROUGH OPEN DOOR WALLS (#12).
- **SENSOR: DEAD** option (#8): killed enemies on the heat sensor map can be dimmed, grey or hidden.
- **OPTIONS** (#6): options that only work in Enhanced display are greyed out in Classic and say so, and a help line explains the selected option. TEXTURES' unsmoothed setting is now called SHARP PIXELS.
- Saved games use format 5 (they record the gameplay fixes); older saves still load.
- The README states the supported systems: Windows 10 and 11, and Linux (#5).

### Known limitations

- Behaviour follows the disassembly of the original; it has not yet been checked frame by frame against the original running in DOSBox-X. The findings for these issues come from the disassembly too.
- Saves made with this version cannot be loaded by earlier betas.
- Saved games are tied to the release they were made with: floppy saves do not load with CD files, and the other way round.
- Multiplayer has been tested on one machine (several game processes), not yet across a real network.

## 1.0.0-beta.4 — 2026-11-15 (pre-release)

The fourth beta. Mods can now bring their own art, sounds, music and network arenas, and OBCEdit gains a resources panel and a patrol-route tool.

### Installers

- **Windows**: `OBCR-Setup-1.0.0-beta.4-windows-x64.exe`. **Linux** (x86-64, glibc 2.35 or newer, such as Ubuntu 22.04+): `OBCR-Setup-1.0.0-beta.4-linux-x86_64` (make it executable and run it).
- The Linux installer is much smaller than beta 3's, and the Linux editor now opens on X11 as well as Wayland.
- Running Setup over an earlier beta updates the installation; settings, saved games and high scores are kept.
- Built by GitHub Actions from the tagged source. `SHA256SUMS.txt` lists the files' checksums. They are not code-signed: Windows SmartScreen warns on first run (*More info* > *Run anyway*).

### Changes

- **Mod art, sounds and music** (mod format 2): a mod can replace any wall texture (floors and ceilings included), sprite (objects, enemy frames, the weapons in hand) or digitized sound, and give each floor its own music as Ogg Vorbis, QOA or IMF. Images are ordinary PNGs, converted to the game's palette as the mod loads; Enhanced mode's texture upscaling covers them too. `obcr --convert-audio` turns a WAV into QOA music. The format: [docs/modding.md](docs/modding.md).
- **Network arenas in mods**: a mod can bring up to ten arenas for network games. Machines only see each other when they have the same mod and version; the network setup screen shows where the arenas come from. In a mod's arenas, players reappear only where the start can reach.
- **OBCEdit**:
  - a *Resources* panel to import and preview art, sounds and music (a WAV imported as music becomes QOA);
  - network arenas in the campaign;
  - a **patrol-route tool** (**T**): click the turns of a route and it places the arrows, turning the enemy to face the route;
  - patrol arrows drawn as arrows, and checks for patrols that stop against a wall and arrows nothing reads.
- The MODS screen lists what a mod brings: arenas, new art, sounds, music.
- Mods made with this version use format 2 only when they need it; a mod with just floors still plays in beta 3.

### Known limitations

- Behaviour follows the disassembly of the original; it has not yet been checked frame by frame against the original running in DOSBox-X.
- Mods replace the game's art and sounds; they cannot add new kinds of objects or enemies, or replace the title, status bar, menus, fonts or sequences.
- OBCEdit cannot play sounds or music itself; hear them by testing the floor in OBCR (F5).
- Saved games are tied to the release they were made with: floppy saves do not load with CD files, and the other way round.
- Multiplayer, including mod arenas, has been tested on one machine (several game processes), not yet across a real network.

## 1.0.0-beta.3 — 2026-11-15 (pre-release)

The third beta. It adds mods and custom maps, and OBCEdit, a level editor for making them.

### Installers

- **Windows**: `OBCR-Setup-1.0.0-beta.3-windows-x64.exe`. **Linux** (x86-64, glibc 2.35 or newer, such as Ubuntu 22.04+): `OBCR-Setup-1.0.0-beta.3-linux-x86_64` (make it executable and run it).
- Setup now offers to install OBCEdit beside the game (on by default; `--no-editor` in an unattended install leaves it out). The installers are larger because the editor carries its own Python and Qt.
- Running Setup over an earlier beta updates the installation; settings, saved games and high scores are kept.
- Built by GitHub Actions from the tagged source. `SHA256SUMS.txt` lists the files' checksums. They are not code-signed: Windows SmartScreen warns on first run (*More info* > *Run anyway*).

### Changes

- **Mods and custom maps**: OBCR plays mods, campaigns of new floors made from the game's own walls, objects and enemies. Put a `.obcmod` file or mod folder in the mods folder, press **D** in the main menu and choose it; each mod keeps its own saves and high scores. `obcr --mod <file>` plays one directly, and `obcr --check-mod <file>` checks one. A mod's floor can reuse an original floor from your own copy of the game, and can behave like any original floor (darkness, sewer walls, music, sequences). The format: [docs/modding.md](docs/modding.md).
- **OBCEdit**, a level editor, ships with OBCR. It is a port of the EC7Edit editor from the EC7Wolf port of Corridor 7, and lets you:
  - draw floors from a palette of the game's real walls, objects and enemies;
  - place doors, push walls, exits and the other compound features with one click;
  - check a floor as you draw, including whether it can be finished;
  - test it in OBCR with **F5**, or photograph it through OBCR's renderer;
  - export or install a mod.

  The manual: [docs/obcedit-manual.md](docs/obcedit-manual.md).
- Saved games use a new format that records the mod they belong to. Saves from earlier betas still load.

### Known limitations

- Behaviour follows the disassembly of the original; it has not yet been checked frame by frame against the original running in DOSBox-X.
- Mods can use only the game's own art, sounds and music, and are single-player only.
- OBCEdit has no tool for drawing patrol routes; patrol arrows are placed one at a time.
- Saved games are tied to the release they were made with: floppy saves do not load with CD files, and the other way round.
- Multiplayer has been tested on one machine (several game processes), not yet across a real network.

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
