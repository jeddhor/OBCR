# Reverse-engineering knowledge base

Everything OBCR knows about the original Operation Body Count, and how it knows it. There is no source code for the game: each fact here was read from the disassembly of the original executable (`COUNT.EXE`, the floppy release; `BCCD.EXE`, the CD release), from its data files, or from the original's documentation. Each one carries a confidence grade and a citation. These files are the specification OBCR is built against; the Wolfenstein 3D source is only a starting hypothesis.

Rules for adding to it: [AGENT_GUIDE §4–§5](../AGENT_GUIDE.md#4-evidence-and-confidence).

## Confidence grades

| Grade | Meaning |
|---|---|
| **CONFIRMED** | Verified directly: observed in DOSBox-X, read instruction by instruction in the disassembly, or a parse of a data file that matches the original's use of it |
| **LIKELY** | Strong indirect evidence: decompiler output read but not traced, or a Wolf3D match plus a partial check of the disassembly |
| **HYPOTHESIS** | A plausible guess from names, strings, Wolf3D precedent or analogy |
| **UNKNOWN** | No useful evidence yet; usually an open question (`Q-###`) |

Addresses are Ghidra addresses in `COUNT.EXE` (segment base 0x1000) unless marked otherwise; [SYMBOLS.md](SYMBOLS.md) explains the conventions, and [CD_RELEASE.md](CD_RELEASE.md) the CD executable's.

## The files

**The data**

| File | Contents |
|---|---|
| [DATA_FORMATS.md](DATA_FORMATS.md) | Every original file's format: the executable, VSWAP (`GFXTILES.BC`), maps, VGAGRAPH, audio, `AUDIOMUS.BC`, FLC sequences, palettes, `CONFIG.DAT`, high scores |
| [LEVEL_CODES.md](LEVEL_CODES.md) | What the map planes' values mean: walls, doors, floors, spawns, push walls, animated walls, exits, the per-level tables |
| [STRINGS.md](STRINGS.md) | Anchor strings in `COUNT.EXE`, with their file and DS offsets |
| [SYMBOLS.md](SYMBOLS.md) | Address conventions, the segment map, landmark functions and variables |

**The game**

| File | Contents |
|---|---|
| [PLAYER.md](PLAYER.md) | The player: controls, movement, weapons and ammunition, pickups, damage, difficulty, the status bar, death, healing stations |
| [ACTORS.md](ACTORS.md) | Actors, state tables and enemy AI: the object layout, spawning, hit points, the shared AI and its differences from Wolf3D, every enemy class |
| [ALLIES.md](ALLIES.md) | Allies: follow and disperse, switching bodies, switch and stay, how they fight |
| [RENDERER.md](RENDERER.md) | The 3D renderer and visual effects: floors and ceilings, shading, see-through walls, the hand weapon, splats, lighting, the overhead map |
| [AUDIO.md](AUDIO.md) | Sound: devices, priorities, digitized sound and its 9,009 Hz rate, panning, IMF music, the sound table |
| [GAMEFLOW.md](GAMEFLOW.md) | Start-up, the intro and title, menus, a floor's life, sequences, saves, `CONFIG.DAT`, cheats |
| [NETWORK.md](NETWORK.md) | Network game rules: players, respawning, scoring, the KILLS table |
| [NETCOMMS.md](NETCOMMS.md) | The original's network transport and setup (IPX, serial, modem) |
| [CD_RELEASE.md](CD_RELEASE.md) | The CD release: its files, `BCCD.EXE`'s layout, every code difference from the floppy build, CD audio |

**Comparisons and unknowns**

| File | Contents |
|---|---|
| [DELTA_SPEC.md](DELTA_SPEC.md) | Differences from Wolfenstein 3D (`D-###`) and the original's quirks OBCR keeps (`K-###`) |
| [OPEN_QUESTIONS.md](OPEN_QUESTIONS.md) | What is not known yet (`Q-###`), with how to find out |
| [../MECHANICS_AUDIT.md](../MECHANICS_AUDIT.md) | Every mechanic, side by side: Wolf3D, the original, OBCR |

Machine-readable state lives in [`/re`](../../re/README.md) at the repository root: the symbol CSV and the type header for Ghidra.

## What never goes here

Original data, extracted art or sound, bulk text dumps, decompiled functions, traces and captures. Those stay in the git-ignored `.local/` (AGENT_GUIDE §10.3). These files describe what the original does in their own words, with addresses to find it.
