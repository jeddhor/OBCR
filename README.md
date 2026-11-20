# OBCR — Operation Body Count Reborn

**OBCR is a native source port of *Operation Body Count* (Capstone Software, 1994), written from scratch in Rust on SDL 3 and rebuilt from the original executable.**

It plays the original game, floor for floor and tic for tic, from your own copy of the game files. You can play it in the original 320×200 picture, or in a modern one: high resolution and widescreen, smooth motion, xBRZ-upscaled textures, mouse and gamepad controls, and network play over the Internet.

> **Status: beta, in testing.** The whole single-player game and network play run. Behaviour follows the disassembly of the original, but it has not yet been compared frame by frame with the original running in DOSBox-X. The current version is **1.0.0-beta.6** ([CHANGELOG.md](CHANGELOG.md), [releases](https://github.com/jeddhor/OBCR/releases)); progress is tracked in [docs/STATUS.md](docs/STATUS.md).

> ### 🎮 The controls are modern by default
>
> **W A S D** to move, the **mouse** to turn and fire, **Space** or **E** to open doors and push walls, **Shift** to run, **Q / Z / F** for your allies.
>
> That is *not* what Operation Body Count shipped with in 1994: the original moved with the **arrow keys**, sidestepped with **Alt** and opened things with **Space**. Pick the original scheme under *OPTIONS → CONTROL SCHEME*, and change any key under *OPTIONS → KEY BINDINGS*. See [Controls](#controls).

---

## Contents

- [Why does this exist?](#why-does-this-exist)
- [About Operation Body Count](#about-operation-body-count)
- [What OBCR adds, and what it keeps](#what-obcr-adds-and-what-it-keeps)
- [Documentation](#documentation)
- [Getting the game data](#getting-the-game-data)
- [Installing](#installing)
- [Running it](#running-it)
- [Controls](#controls)
- [Multiplayer](#multiplayer)
- [Mods and OBCEdit](#mods-and-obcedit)
- [When it goes wrong](#when-it-goes-wrong)
- [Building from source](#building-from-source)
- [How it is built](#how-it-is-built)
- [Testing](#testing)
- [Known deviations](#known-deviations)
- [Credits](#credits)
- [Legal](#legal)

---

## Why does this exist?

Operation Body Count is one of the stranger children of Wolfenstein 3D: a licensed Wolf3D engine pushed into a whole extra set of systems, including squads of allies you can swap bodies with, a flamethrower that sets rooms on fire, grenades that blow holes in walls, floors with lights you can switch off, swamp creatures that splatter your visor with slime, and network deathmatch in 1994.

Its source code was never released. Today Steam and GOG sell it as the original DOS program running inside DOSBox, which works, but keeps it at 320×200 with DOS-era controls and no modern network play.

OBCR is a port without source code. Every behaviour comes from the original executable: the Wolfenstein 3D source code (which id Software released) serves as a starting hypothesis, and each function is checked against the disassembly of `COUNT.EXE`, with the differences written down as it goes. The knowledge base in [`docs/re/`](docs/re/README.md) records what was found, where in the executable it was found, and how sure we are of it.

The project is developed by AI coding agents (Anthropic's Claude) under the owner's direction, following written rules for evidence, testing and fidelity ([CLAUDE.md](CLAUDE.md), [docs/AGENT_GUIDE.md](docs/AGENT_GUIDE.md)). Nothing from the original game is included in this repository: no data, no extracted art or sound, no decompiled code.

---

## About Operation Body Count

> *The Government has launched a top-secret military offensive against the foreign troops occupying the U.N. towers. Code name…*
>
> — the original's opening screen

You go in through the sewers beneath the United Nations, fight up through the maintenance level and every floor of the first UN tower, cross to the second tower and fight to its roof, where the man behind it all waits. As the original's manual supplement put it: *"Game victory? Kill Victor."*

What sets it apart from its Wolfenstein 3D ancestry:

- **40 floors** in four stretches: five sewer levels, a maintenance level, the 17 floors of UN Building 1 from the lobby to the top, then the 17 floors of Building 2. Ten more floors are network arenas.
- **A body count, not a key hunt.** A floor's exit (stairs, a ladder or an elevator) only lets you leave once **half of the floor's enemies** are dead. A lamp on the status bar turns from red to yellow at half and to green when everyone is dead; trying to leave early earns you *UNFINISHED BUSINESS*.
- **Allies.** Friendly soldiers wait on many floors. Tell them to follow you or to spread out and fight. Or *switch* into one of them, taking over their body and health while keeping your guns, and optionally leave your old body standing guard.
- **Five weapons**: an assault shotgun, an Uzi, a Galil, a flamethrower that sets whatever it touches alight, and a grenade launcher whose blasts break walls (and hurt you, if you stand too close).
- **A destructible, living building**: walls that crack and break, fires that spread and burn out, booby traps, generators whose switches plunge a floor into darkness, push walls hiding secret rooms, tremors that shake the screen, and splats of blood and slime across your visor that you have to wipe off.
- **Enemies from soldiers to sewer things**: rats and giant rats, soldiers who ambush you from stools, toilets and the floor, men in black suits, swamp creatures, fat men who climb out of manholes, saucer drones, red giants, and finally Victor.
- **Network play** in 1994: deathmatch, team play and two-player modem games on floors built for it.

The full player's guide is [docs/game-guide.md](docs/game-guide.md).

### Technical profile of the original

| | |
|---|---|
| Year, publisher | 1994, Capstone Software (IntraCorp) |
| Engine | A licensed and heavily modified Wolfenstein 3D engine |
| Executable | `COUNT.EXE` (floppy release, 235,440 bytes) or `BCCD.EXE` (CD release, 237,056 bytes): Borland C++, 16-bit real mode, not packed |
| Data files | Wolf3D's family of files with a `.BC` extension |
| Maps | `MAPTEMP.BC` and `MAPHEAD.BC`: 61 map slots, 64×64 tiles, RLEW compression only (no Carmack layer) |
| Walls, sprites, sounds | `GFXTILES.BC`, Wolf3D's VSWAP page file: 680 sprites on the floppy, 668 on the CD |
| Screens, fonts, status bar | `VGAHEAD.BC`, `VGADICT.BC`, `VGAGRAPH.BC` |
| Music and effects | `AUDIOHED.BC`, `AUDIOT.BC`: AdLib music (IMF) and AdLib/PC-speaker effects |
| Digitized sound | `AUDIOMUS.BC`: 99 sounds at 9,009 Hz, in the same layout as Corridor 7's |
| Animated sequences | FLC animations `F1.BC`–`F7.BC`; `F1`, `F5` and `F7` exist only on the CD |
| CD music | Four Red Book audio tracks on the CD |
| Palettes | Stored inside the executable |

---

## What OBCR adds, and what it keeps

**It keeps the game.** The simulation runs at the original's 70 Hz tic rate with its integer arithmetic, its random-number table and its quirks. **Classic** mode shows the original 320×200 picture with the original's renderer: the per-tile floors and ceilings with their grout lines, the shading, the see-through walls and the two-part hand weapon.

**It adds, in Enhanced mode** (the default):

- **Resolution and shape**: 2× to 8× the original resolution, at 4:3, 16:10, 16:9 or 21:9 (wider views show more to the sides, as Wolf3D ports do), windowed or fullscreen, with optional integer scaling and a smoothing filter.
- **Smooth motion**: the view and every actor are drawn between game tics, so the picture moves at your monitor's refresh rate while the game still runs at 70 Hz.
- **xBRZ textures**: walls, doors, floors, ceilings, enemies, items and the weapon upscaled 2× to 6× with xBRZ. Shading, damage flashes and fades still work, because the upscale is done on palette indices. The menus, pictures and animated sequences can be upscaled too.
- **Smooth floors**: floors and ceilings drawn in true perspective and continuous across tiles, without the original's banding.
- **Modern controls**: WASD and the mouse, gamepads with analogue movement and rumble, and full rebinding of every action for keyboard, mouse and gamepad.
- **Sound**: up to four digitized voices mixed instead of the original's single Sound Blaster voice; the original AdLib and PC-speaker effects remain available.
- **The CD release in full**: install straight from the Steam or GOG disc image, with its movies, its gameplay changes and its CD music ripped from the image ([docs/cd-release.md](docs/cd-release.md)).
- **Network play over UDP**, on a LAN or across the Internet, for up to 12 players ([docs/multiplayer.md](docs/multiplayer.md)).
- **Settings in one readable file** (`obcr.toml`), saved games in your user profile, and the original's ten save slots, AutoSave and F9 quick save.

**It fixes two things that could leave a floor unfinishable**, as deliberate departures from the original ([DECISIONS.md](docs/DECISIONS.md)): grenades and fires no longer destroy a floor's exits or seal its doors (ADR-016), and kills by your allies count toward the exit's quota (ADR-018), though they still earn no points.

Every option is described in [docs/options.md](docs/options.md).

---

## Documentation

**For players**

| Document | What it is |
|---|---|
| [docs/game-guide.md](docs/game-guide.md) | The player's guide: floors, the body count, weapons, items, enemies, allies, hazards and the original's own tips. |
| [docs/options.md](docs/options.md) | Every menu option, every command-line option, the settings file, and where OBCR keeps things. |
| [docs/cd-release.md](docs/cd-release.md) | Playing the Steam/GOG CD release: installing from the disc image, the movies, the CD music and how the CD version plays differently. |
| [docs/multiplayer.md](docs/multiplayer.md) | Network play: game types, setting up a LAN or Internet game, ports and routers, the rules, troubleshooting. |
| [docs/modding.md](docs/modding.md) | Mods: installing and playing them, and making them: the `mod.toml` manifest, map rules, checking and sharing. |
| [docs/obcedit-manual.md](docs/obcedit-manual.md) | OBCEdit, the level editor that comes with OBCR: drawing floors, testing them in OBCR, building a mod. |
| [installer/README.md](installer/README.md) | OBCR Setup: what it installs, where, the command line, and how the installers are built. |
| [docs/obcr.6](docs/obcr.6) | A manual page (`man -l docs/obcr.6`). |
| [CHANGELOG.md](CHANGELOG.md) | What changed in each release. |

**For developers and the curious**

| Document | What it is |
|---|---|
| [docs/DESIGN.md](docs/DESIGN.md) | The design: goals, the fidelity model, architecture, every subsystem, verification. |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Architecture decision records: every significant choice and why (ADR-001 onward). |
| [docs/MECHANICS_AUDIT.md](docs/MECHANICS_AUDIT.md) | Every Operation Body Count mechanic, what the original's code does, and where OBCR implements it. |
| [docs/MODS_AND_OBCEDIT.md](docs/MODS_AND_OBCEDIT.md) | The plan for mods and OBCEdit: the format, the engine changes, how EC7Edit became OBCEdit. |
| [docs/MOD_ASSETS_AND_ARENAS.md](docs/MOD_ASSETS_AND_ARENAS.md) | The plan for mod art, sounds, music and network arenas, and OBCEdit's patrol-route tool. |
| [obcedit/README.md](obcedit/README.md) | OBCEdit's source tree. |
| [docs/re/](docs/re/README.md) | The reverse-engineering knowledge base: file formats, the executable's layout, actors, player, allies, renderer, audio, game flow, network, the CD release, and open questions. Every claim carries a confidence grade and a citation. |
| [docs/ci.md](docs/ci.md) | The test gates, what GitHub Actions runs and cannot run, and how a release is cut. |
| [docs/STATUS.md](docs/STATUS.md), [docs/MILESTONES.md](docs/MILESTONES.md) | Where the project stands, and the milestone plan it follows. |
| [docs/AGENT_GUIDE.md](docs/AGENT_GUIDE.md), [CLAUDE.md](CLAUDE.md) | The rules the AI agents that develop OBCR work by. |
| [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) | Third-party components and their licences. |

---

## Getting the game data

**This repository contains no Operation Body Count data.** You need your own copy. OBCR recognizes two releases by the SHA-256 of their files:

| Release | Where you get it | What OBCR needs |
|---|---|---|
| **The CD release** (`BCCD.EXE`) | Steam and GOG sell it; both ship the CD as a BIN/CUE disc image | The **`.cue` file** (with its `.bin` beside it). Setup takes the files, the movies and the music straight off the image. |
| **The floppy release** (`COUNT.EXE`, files dated 1996-12-24) | Old retail floppies and their copies | The folder with the game files. |

The CD image is the best source: it is the only one with the opening, map 22 and victory movies and the CD music. A Steam installation's `C/BCCD` folder holds the CD release's files too and works as a folder, but without the music.

These are the files OBCR uses. Everything else in an installation (`SETUP.EXE`, `INSTALL.EXE`, `README.EXE`, the batch files, `GFXINFOV.BC`, `CONFIG.DAT` and DOS saved games) is ignored:

```
COUNT.EXE  or  BCCD.EXE    the executable: OBCR reads its tables and palettes      required
GFXTILES.BC                walls, sprites and some digitized sounds                required
MAPHEAD.BC, MAPTEMP.BC     the maps                                                required
VGAHEAD.BC, VGADICT.BC,    screens, fonts, status bar and menu art                 required
VGAGRAPH.BC
AUDIOHED.BC, AUDIOT.BC     AdLib music and AdLib/PC-speaker effects                required
AUDIOMUS.BC                the 99 digitized sounds                                 required
F2.BC F3.BC F4.BC F6.BC    animated sequences (floor changes, death)               optional
F1.BC F5.BC F7.BC          the CD's movies (opening, map 22, victory)              optional, CD only
TRACK02.QOA .. TRACK05.QOA the CD music, ripped from the image by Setup            optional, CD only
```

The executable is required because Operation Body Count keeps its behaviour tables (every enemy's animation states, the weapons' timing, the items) and its palettes inside it. OBCR reads them at start-up; nothing is copied into OBCR itself. A missing sequence is skipped silently, as the original does.

The files must match a known release exactly. Patched, cracked or otherwise modified executables are refused, because the tables would not be where OBCR looks for them. The shareware release has not been examined yet and is not recognized.

---

## Installing

| If you want | Use |
|---|---|
| the usual way, on Windows or Linux | [OBCR Setup](#obcr-setup), the graphical installer |
| a scripted or unattended install | [Setup from a terminal](#setup-from-a-terminal) |
| no installer at all | [The game on its own](#the-game-on-its-own) |
| to compile it yourself | [Building from source](#building-from-source) |

Whichever route you take, OBCR keeps your settings and saved games in your user profile, never in the game folder (see [Running it](#running-it)).

### OBCR Setup

Download the installer for your system from the [releases page](https://github.com/jeddhor/OBCR/releases):

- **Windows** (Windows 10 or 11, 64-bit): `OBCR-Setup-<version>-windows-x64.exe`. It is not code-signed, so SmartScreen warns the first time: choose *More info → Run anyway*.
- **Linux** (x86-64, glibc 2.35 or newer, such as Ubuntu 22.04 and later): `OBCR-Setup-<version>-linux-x86_64`. Make it executable (`chmod +x`) and run it. It carries its own Qt; the graphics drivers, X11 or Wayland and the sound system come from your desktop.

Setup installs for your user account only and never asks for administrator rights. Its pages:

1. **Your game files.** Choose **CD image…** and pick the `.cue` of the Steam or GOG edition, or **Folder…** for a folder with the game files. Setup suggests the images and folders it finds in the usual Steam and GOG locations. It checks every file against the known releases and tells you what it found: the release, how many files it will copy, how many animated sequences, and from a CD image how many music tracks it will rip.
2. **Installation folder.** By default `%LOCALAPPDATA%\Programs\OBCR` on Windows and `~/.local/opt/OBCR` on Linux. It must be new, empty, or an earlier OBCR installation (which is then updated). Choose whether you want menu entries, a desktop shortcut and, on Linux, an `obcr` command in `~/.local/bin`.
3. **Installing.** The game, the game files it uses and, from a CD image, the ripped music (about 57 MB, about 15 seconds) go into the folder, with an uninstaller.

The result:

```
<folder>/obcr[.exe]          the game
<folder>/data/               your game files (and TRACK02.QOA .. from a CD image)
<folder>/uninstall/          the uninstaller
<folder>/install-manifest.txt   everything Setup created, for the uninstaller
```

On Windows OBCR appears in the Start menu and in *Settings → Apps*; on Linux in your applications menu. Running Setup again offers to update or remove the installation.

### Setup from a terminal

```sh
OBCR-Setup --unattended --data /path/to/BODYCOUNT.cue
OBCR-Setup --unattended --data /path/to/game/folder --target ~/Games/OBCR --desktop-shortcut
OBCR-Setup --uninstall --unattended
```

Unattended runs print their progress and exit with 0 on success, 1 on failure and 2 when the game files are incomplete. All options: [installer/README.md](installer/README.md).

### The game on its own

The game is a single executable with no installation step. Build it ([Building from source](#building-from-source)) or take `obcr` from an installation, and point it at the game files:

```sh
obcr --data /path/to/game/folder
```

From a CD image, extract the files first. This does exactly what Setup does, music included:

```sh
obcr --extract-data /path/to/BODYCOUNT.cue ~/Games/OBC-data
obcr --data ~/Games/OBC-data
```

Without `--data`, OBCR looks in this order: the `OBC_DATA_DIR` environment variable; `data_dir` in the settings file; the folders `OBC`, `data` or the executable's own folder next to the executable; Steam's default `Operation Body Count/C/BCCD` folder.

### Uninstalling

Use *Settings → Apps* (Windows), the *Uninstall OBCR* menu entry, `<folder>/uninstall/OBCR-Uninstall`, or Setup itself. It removes exactly what the installation created. Your settings and saved games stay unless you tick the box to remove them too (`--purge` from a terminal).

---

## Running it

Start OBCR from the menu entry, or run `obcr`. You get the original's opening (the CD release's movie first, then the story page and the title), then the main menu:

| Main menu | |
|---|---|
| **START GAME** | Begin on floor 1 with the current game type |
| **GAME TYPE** | Difficulty 1–3, Random, or the network types N, T and M |
| **VIEW SIZE** | The size of the 3D view |
| **LOAD GAME / SAVE GAME** | Ten slots; the last holds the AutoSave written before every floor |
| **AUDIO MIX** | OBCR's OPTIONS, opened at the sound settings |
| **HIGH SCORES**, **CONTINUE**, **ABORT GAME**, **EXIT TO DOS** | As in the original |
| **O** | OBCR's **OPTIONS**: display, textures, controls, key bindings, sound and music |
| **D** | **MODS**: choose the original game or an installed mod ([docs/modding.md](docs/modding.md)) |
| **H** | The original's help page, followed by the key bindings |

| Where things go | Windows | Linux |
|---|---|---|
| Settings (`obcr.toml`) | `%APPDATA%\OBCR\config\` | `~/.config/obcr/` |
| Saved games | `%APPDATA%\OBCR\data\saves\` | `~/.local/share/obcr/saves/` |
| The game files | wherever you installed them; OBCR never writes there | the same |

`--config FILE` puts the settings file elsewhere, with saved games in a `saves` folder beside it. Useful command-line options:

```sh
obcr --classic                       # the original 320x200 picture
obcr --scale 6 --aspect 21:9         # Enhanced at 6x, ultra-wide
obcr --fullscreen
obcr --xbrz 0 --floors original      # Enhanced resolution, original-looking textures
obcr --play 6 --difficulty 2         # straight onto floor 7 (map slot 6), difficulty 3
obcr --nsr250                        # the original's cheat parameter (see the game guide)
obcr --help                          # all of them
```

The complete list, the settings file and every menu option are in [docs/options.md](docs/options.md). Alt+Enter toggles fullscreen at any time.

---

## Controls

**The defaults are modern, not the original's.** Out of the box you move with **W A S D**, turn with the mouse and open things with **Space** or **E**. Operation Body Count in 1994 moved with the **arrow keys**, sidestepped by holding **Alt**, and opened things with **Space**. Switch with *OPTIONS → CONTROL SCHEME*. Each scheme comes with its own defaults.

| Action | Modern (default) | Original | Gamepad |
|---|---|---|---|
| Move forward / back | W / S, ↑ / ↓ | ↑ / ↓ | left stick, D-pad up |
| Turn | mouse, ← / → | ← / →, mouse | right stick, D-pad left/right |
| Sidestep | A / D | hold Alt and turn | left stick |
| Fire | left mouse, Ctrl | Ctrl, left mouse | right trigger |
| Open, use, push | Space, E, right mouse | Space, middle mouse | A |
| Run | Shift | Shift, right mouse | left trigger, left stick click |
| Weapons | 1–5, mouse wheel | 1–5, mouse wheel | LB / RB |
| Ally: switch / switch and stay / follow or disperse | Q / Z / F | A / S / F | X / B / Y |
| Wipe the visor | V | W | D-pad down |
| Overhead map | Tab | Tab | Back |
| Menu | Esc | Esc | Start |

The rest is the same in both schemes:

| Key | Action |
|---|---|
| F9 | Quick save, to the slot you last saved to from the menu |
| H / C | Exact health overlay / crosshair |
| F1 / F2 | Sound on or off / music on or off |
| F5 / F6 | Textured floors and ceilings off / on |
| F8 | Tremors (screen shake) on or off |
| Pause | Pause |
| F10 | Back to DOS (quit) |
| Alt+P / Alt+S / Alt+F | CD music: play / stop / next track (CD release) |
| Alt+Enter | Fullscreen |

**Rebinding.** *OPTIONS → KEY BINDINGS* (also shown after the help page) lists every action. Each takes up to three keyboard or mouse inputs (mouse buttons 4 and 5 and the wheel included) and two gamepad buttons (triggers included). Left and Right switch between the keyboard-and-mouse list and the gamepad list. Enter (A) waits for the new input, Backspace or Delete (X) clears a row, and the last row restores the scheme's defaults. An input bound to another action moves to the new one, and the screen says so. Esc and Start always open the menu; the sticks always move and turn.

**Always run** (OPTIONS) inverts the run key. **Mouse speed** and **pad turn speed** are in OPTIONS too; rumble can be switched off.

---

## Multiplayer

Operation Body Count had network and modem play; OBCR brings it back over UDP, on a LAN or across the Internet, for up to 12 players.

Set **GAME TYPE** to **N** (every player for themselves), **T** (three teams) or **M** (two players, the old modem game), then **START GAME**. On the setup screen pick your character or team and press Enter. Players on the same network find each other by themselves. To play across the Internet, press **J** and type the other player's address; the player you call must let UDP port 16387 through their router. From the command line:

```sh
obcr --net N                                  # deathmatch, find LAN players
obcr --net T --net-choice 1                   # teams, join team 2
obcr --net N --net-join example.org:16387     # call a player over the Internet
```

Network games are not compatible with the DOS original, and floppy and CD copies cannot play together. So far network play has been tested with several copies on one machine, not yet across a real network. Everything else, including the rules, scoring, the KILLS table, ports and troubleshooting, is in [docs/multiplayer.md](docs/multiplayer.md).

---

## Mods and OBCEdit

OBCR plays **mods**: campaigns of new floors, built from the original game's walls, objects and enemies, and optionally with their own wall textures, sprites, sounds, music and network arenas. Put a `.obcmod` file or a mod folder in the mods folder (`%APPDATA%\OBCR\data\mods\` on Windows, `~/.local/share/obcr/mods/` on Linux), press **D** in the main menu, choose it, and START GAME plays its floors in order. Each mod keeps its own saved games and high scores. `obcr --mod <file>` plays one without installing it.

**OBCEdit**, the level editor, comes with OBCR. Setup installs it unless you untick the option, with its own menu entry. It is a port of the EC7Edit editor from the EC7Wolf port of Corridor 7. In OBCEdit you can:
- open the original floors to study them;
- draw new ones from a palette of the game's real walls, objects and enemies;
- place doors, push walls, exits and the other compound features with one click;
- draw patrol routes by clicking their turns;
- have it check your map as you draw, including whether the exit can be reached and the quota met;
- import your own wall textures, sprites, sounds and music, previewed in the game's palette;
- build network arenas for multiplayer games;
- press **F5** to play your floor in OBCR at once;
- export a mod ready to share.

How mods work and how to write one by hand: [docs/modding.md](docs/modding.md). The editor's manual: [docs/obcedit-manual.md](docs/obcedit-manual.md).

---

## When it goes wrong

| Symptom | Cause and fix |
|---|---|
| *"Operation Body Count data not found"* | OBCR could not find the game files. Pass `--data <folder>`, set `OBC_DATA_DIR`, or reinstall with Setup. |
| *"missing or unrecognized files: …"* | The named files are missing or are not exactly a known release's. Use the original files: patched or cracked executables are refused. |
| Setup says the folder is *"not a complete copy of the supported release"* | Choose the folder that contains `GFXTILES.BC`, or the `.cue` of the disc image (not the `.bin`, although a `.bin` with its `.cue` beside it works too). |
| No CD music | The music is ripped only when installing from the CD image. Check *OPTIONS → MUSIC SOURCE* is CD AUDIO; ADLIB plays the FM music instead. |
| No opening movie | `F1.BC` exists only on the CD image; the floppy release never had it. |
| The music stops partway through a floor | *OPTIONS → CD TRACK END* is set to ORIGINAL, which keeps the original's silence until the next floor. NEXT TRACK moves on. |
| A saved game will not load (*"saved with data release …"*) | Saved games belong to the release they were made with. Floppy saves need the floppy files, and CD saves need the CD files. |
| SmartScreen blocks Setup | The installers are not code-signed. Choose *More info → Run anyway*. |
| Linux Setup will not start | It needs glibc 2.35 or newer. Run it from a terminal to see the error. |
| *"The procedure entry point GetSystemTimePreciseAsFileTime could not be located"* (or another missing `KERNEL32.dll` function) | OBCR needs Windows 10 or 11. The Rust toolchain it is built with, Qt 6 (Setup) and PySide6 (OBCEdit) all require Windows 10, so Windows 7 and 8 are not supported. On an older PC, the Linux version is the way to play. |
| The picture looks too soft or too sharp | Toggle *TEXTURES* (xBRZ), *SMOOTH FILTER* and *INTEGER SCALING* in OPTIONS, or use `--classic`. |
| Text on the status bar looks distorted | *XBRZ STATUS BAR* is on; it is off by default for that reason. |

If something looks like a bug rather than a setting, please [open an issue](https://github.com/jeddhor/OBCR/issues). Include the release you play (floppy or CD), what you did, and a screenshot if possible.

---

## Building from source

You need **Rust** (the version pinned in `rust-toolchain.toml` installs itself through rustup), **CMake** and a C compiler. SDL 3 is built from source and linked statically, so there is nothing else to install on Windows. On Linux, install SDL 3's build dependencies first. On Ubuntu:

```sh
sudo apt install build-essential cmake ninja-build libx11-dev libxext-dev libxrandr-dev \
    libxcursor-dev libxfixes-dev libxi-dev libxss-dev libxtst-dev libxkbcommon-dev \
    libwayland-dev wayland-protocols libdecor-0-dev libegl1-mesa-dev libgl1-mesa-dev \
    libgles2-mesa-dev libdrm-dev libgbm-dev libasound2-dev libpulse-dev \
    libpipewire-0.3-dev libdbus-1-dev libudev-dev libibus-1.0-dev
```

Then:

```sh
cargo build --release                # target/release/obcr
cargo run --release -- --data /path/to/game/folder
```

On Windows, the CMake bundled with Visual Studio works; if an older `cmake` on the PATH gets in the way, point the `CMAKE` environment variable at a newer one (for example in a machine-local `.cargo/config.toml`).

The installers are built with `installer/build-windows.ps1` (Qt 6 MinGW kit) and `installer/build-linux.sh` (on Ubuntu 22.04, so the result runs on newer systems); see [installer/README.md](installer/README.md). GitHub Actions builds both for every release ([docs/ci.md](docs/ci.md)).

---

## How it is built

OBCR is a Cargo workspace of small crates with one-way dependencies:

| Crate | What it does |
|---|---|
| `obc-core` | Fixed-point maths, angles, the original's random-number table, constants and input state |
| `obc-formats` | Parsers for every original file (VSWAP, maps, VGAGRAPH, audio, FLC, palettes, the executable's tables), release identification, the CD image reader and the QOA codec |
| `obc-sim` | The game itself: a deterministic simulation with no I/O, no clock, no floating point and no randomness except the original's table |
| `obc-render` | The software renderer: the original's ray caster and its Enhanced extensions, xBRZ |
| `obc-audio` | AdLib synthesis (Nuked OPL3), PC speaker, digitized sound mixing, IMF music, CD-audio streaming |
| `obc-net` | The UDP transport for network games |
| `obc-platform` | SDL 3: window, presentation, audio device, keyboard, mouse, gamepads, rumble |
| `obc-ui` | Screens, menus, the status bar, game flow, settings, saved games |
| `obcr` | The executable that ties it together, and the installer's helpers |
| `obcr-setup` | The self-extracting launcher for the Qt installer in `installer/` |
| `tools/obc-tool` | Inspect and export the original data (for development; exports stay out of the repository) |
| `xtask` | Build automation: `cargo xtask ci`, `test-data`, `pack-installer` |

The simulation mirrors the original's control flow function by function, with its integer widths, its overflow and its random-number call order, and it cites the address of the original function it ports. Because it is deterministic, a saved game continues bit-for-bit as if it had never been saved, and every network peer computes the same game. The design is in [docs/DESIGN.md](docs/DESIGN.md).

---

## Testing

```sh
cargo xtask ci          # formatting, clippy with warnings as errors, all tests, licence and dependency checks
cargo xtask test-data   # also the tests that need the original game data (OBC_DATA_DIR)
```

The tests that need no game data cover the parsers (including property tests against malformed input), the simulation's rules, the QOA codec, the disc reader, the mixer, the controls and the network transport (including a four-player game with 10% packet loss). The data-gated tests load the real files and play them:

- **Soak tests** play every single-player floor at every difficulty for 2,500 frames with scripted input, on the floppy and on the CD release, failing on any panic, arithmetic overflow or simulation fault.
- **Game-flow tests** drive the real screens through floor changes, saving and loading (which must continue identically), death and the high-score entry.
- **Release tests** check that the floppy and CD executables' tables line up, and that the CD image identifies and its music survives encoding.

GitHub Actions runs `cargo xtask ci` on Linux and Windows for every push. The data-gated tests cannot run there, because the game data is never uploaded. See [docs/ci.md](docs/ci.md).

---

## Known deviations

OBCR aims to play exactly like the original. Where it does not, it says so:

- **Not yet checked at run time.** Behaviour comes from reading the disassembly, and the soak and flow tests guard against crashes, not against subtle differences. A frame-by-frame comparison with the original running in DOSBox-X is planned and has not been done.
- **Two deliberate fixes** keep floors finishable: blasts spare exits and doors (ADR-016), and ally kills count toward the exit quota (ADR-018).
- **CD release**: one value cannot be read from the code. A flame's direct hit passes an uninitialised value as its damage, so OBCR uses the floppy formula's average instead (Q-036). The CD-only movies play at about 12 frames a second; the original played them as fast as the CD drive could read them.
- **CD music** moves on to the next track by default when one ends; the original fell silent until the next floor (*CD TRACK END → ORIGINAL* restores that).
- **Saved games** are OBCR's own format; DOS saved games cannot be loaded.
- **Network play** uses OBCR's own UDP transport and cannot play against the DOS original. The rules are the original's.
- **Not ported**: the developers' debug keys. The healing stations are ported but, as in the original, nothing on the floppy release's floors can start them.

The full mechanic-by-mechanic comparison is [docs/MECHANICS_AUDIT.md](docs/MECHANICS_AUDIT.md), and the unknowns are listed in [docs/re/OPEN_QUESTIONS.md](docs/re/OPEN_QUESTIONS.md).

---

## Credits

**Operation Body Count** © 1994 Capstone Software / IntraCorp. From the game's own credits screen:

| Role | Credit |
|---|---|
| Executive Producer | David Turner |
| Producer | Amy Smith Boylan |
| Assistant Producer | Katheryn Gangi |
| Programmers | Les Bird, Jeff Schulz, Rafael Paiz |
| Art | Ruben Cabrera, Scott Nixon |
| 3-D Animation | Carlos Ibarra, Ernesto Roque |
| Manual | Richard Henning |
| Level Design | Ruben Cabrera, Richard Henning, Rafael Paiz, Scott Nixon |
| Music & Sound | Joe Abbati |
| Quality Assurance | James Wheeler |

**Wolfenstein 3D** © id Software: the engine Operation Body Count licensed, and whose released source code is OBCR's starting point.

**OBCR** stands on open-source work: SDL 3, the Nuked OPL3 emulator, xBRZ by Zenju, Qt 6 for the installer, and the Rust crates listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). [ECWolf](https://maniacsvault.net/ecwolf/) and the EC7Wolf port of Corridor 7 were references for the Capstone engine family; Operation Body Count shares Corridor 7's engine lineage.

**OBCR is an unofficial preservation project. It is not affiliated with or endorsed by Capstone Software, IntraCorp, id Software, or the current rights holders of Operation Body Count.**

---

## Legal

- **OBCR's code** is licensed under the GNU General Public License, version 3 or later ([LICENSE](LICENSE), [ADR-006](docs/DECISIONS.md)). Binaries that include xBRZ are distributed under GPL-3.0.
- **Third-party components** and their licences: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
- **Operation Body Count** and all of its data (maps, art, sound, music, movies, the executables and the tables and palettes in them) remain the property of their rights holders. None of it is included in, embedded in or distributed with OBCR; you supply it from your own copy.
