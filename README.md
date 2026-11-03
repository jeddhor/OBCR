# OBCR

A modern, native source port of **Operation Body Count** (Capstone Software, 1994), the Wolfenstein 3D–engine shooter with allies, grenades, flame throwers and network play.

OBCR is written in Rust on SDL3. It plays exactly like the original by default (tic-accurate simulation, pixel-exact 320×200 view) and offers opt-in modern enhancements: high resolution, widescreen, smooth motion, modern controls, Internet multiplayer.

> **Status: planning.** The repository currently contains the design and development guides. No playable code yet. See [docs/STATUS.md](docs/STATUS.md).

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

Not yet: the Cargo workspace is milestone M0. Planned toolchain: Rust stable (pinned in `rust-toolchain.toml`), CMake and a C compiler (for SDL3).

## Legal

Operation Body Count is © its rights holders. OBCR is an unofficial, independent project and is not affiliated with or endorsed by them. OBCR's code is licensed under the GNU General Public License, version 3 or later ([LICENSE](LICENSE), [ADR-006](docs/DECISIONS.md#adr-006--license-gpl-30-or-later)).
