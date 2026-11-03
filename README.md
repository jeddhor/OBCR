# OBCR

A modern, native source port of **Operation Body Count** (Capstone Software, 1994), the Wolfenstein 3D–engine shooter with allies, grenades, flame throwers and network play.

OBCR is written in Rust on SDL3. It plays exactly like the original by default (tic-accurate simulation, pixel-exact 320×200 view) and offers opt-in modern enhancements: high resolution, widescreen, smooth motion, modern controls, Internet multiplayer.

> **Status: playable, in testing.** The whole single-player game runs; the first episode (floors 1–23, Sewer 1 to UN Building 1 Top) is in owner playtesting. Behaviour follows the disassembly but has not yet been checked against the original at run time. See [docs/STATUS.md](docs/STATUS.md).

## You need the original game

OBCR contains no game data. You need your own copy of Operation Body Count (retail or the Steam release). OBCR reads the original files (`GFXTILES.BC`, `MAPTEMP.BC`, `VGAGRAPH.BC`, `AUDIOT.BC`, `COUNT.EXE`, …) at run time.

## Documentation

| Document | For |
|---|---|
| [docs/DESIGN.md](docs/DESIGN.md) | Master design guide: goals, fidelity model, architecture, subsystem designs, verification |
| [docs/AGENT_GUIDE.md](docs/AGENT_GUIDE.md) | Master AI agent development guide: workflow, reverse engineering, oracle, standards, definition of done |
| [docs/MILESTONES.md](docs/MILESTONES.md) | Milestones M0–M15 with tasks and acceptance criteria |
| [docs/STATUS.md](docs/STATUS.md) | Current progress, active tasks, blockers |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Architecture decision records |
| [docs/re/](docs/re/README.md) | What we know about the original: formats, symbols, deltas from Wolf3D, open questions |
| [CLAUDE.md](CLAUDE.md) | Rules every AI agent session loads |

## Building

Requirements: Rust (the version in `rust-toolchain.toml` installs itself through rustup), CMake and a C compiler (SDL3 is built from source and linked statically). On Windows, Visual Studio's bundled CMake works; point `CMAKE` at it in a machine-local `.cargo/config.toml` if `cmake` on the PATH is too old.

```
cargo build --release
cargo xtask ci          # format, lints, tests, dependency licenses
cargo xtask test-data   # also the tests that need the game data
```

## Playing

```
target/release/obcr --data <your Operation Body Count directory>
```

OBCR remembers the data directory (and finds a Steam install by itself). Settings go to `obcr.toml` in your user config directory and saved games to your user data directory, never into the game directory.

Useful options: `--classic` (original 320×200), `--scale 2..8` and `--aspect 4:3|16:10|16:9|21:9` (Enhanced), `--fullscreen` / `--windowed`, `--play N` (start straight on floor N+1, for testing). Alt+Enter toggles fullscreen.

In the main menu, **O** opens OPTIONS: display mode (Classic or Enhanced), resolution, aspect, fullscreen, smooth filter, integer scaling, smooth motion, control scheme, always run, mouse and pad speed, rumble, volumes, and the number of sound voices (1 as the original, or 4 mixed). The original menu items work as in the original game (GAME TYPE picks difficulty 1–3 or Random; START GAME begins on floor 1).

### Controls

| Action | Modern (default) | Original | Gamepad |
|---|---|---|---|
| Move | W / S, arrows | ↑ / ↓ | left stick |
| Turn | mouse, ← / → | ← / →, mouse | right stick |
| Strafe | A / D | Alt + ← / → | left stick |
| Fire | left mouse, Ctrl | Ctrl, left mouse | right trigger |
| Use / open | Space, E, right mouse | Space, middle mouse | A |
| Run | Shift (inverted by Always run) | Shift, right mouse | left trigger, L3 |
| Weapons | 1–5, mouse wheel | 1–5 | LB / RB |
| Ally: switch / stay & switch / follow-disperse | Q / Z / F | A / S / F | X / B / Y |
| Wipe visor | V | W | D-pad down |
| Map (Tab overlay) | Tab | Tab | Back |
| Menu | Esc | Esc | Start |
| Quick save | F9 | F9 | — |
| Tremors on/off, floors on/off, pause | F8, F5/F6, Pause | same | — |

You leave a floor through its stairs or elevator once at least half of its enemies are dead (the kill lamp on the status bar turns green). There are no extra lives: save often (F9 saves to the slot you last saved from the menu; an AutoSave is written before every floor change).

## Legal

Operation Body Count is © its rights holders. OBCR is an unofficial, independent project and is not affiliated with or endorsed by them. OBCR's code is licensed under the GNU General Public License, version 3 or later ([LICENSE](LICENSE), [ADR-006](docs/DECISIONS.md#adr-006--license-gpl-30-or-later)).
