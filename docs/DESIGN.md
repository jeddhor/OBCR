# OBCR Master Design Guide

| | |
|---|---|
| Document | Master Design Guide (architecture, technical design, fidelity policy) |
| Version | 1.0 — 2026-11-02 |
| Status | Living document. Architectural changes require an ADR in [DECISIONS.md](DECISIONS.md). |
| Companions | [AGENT_GUIDE.md](AGENT_GUIDE.md) (how to work) · [MILESTONES.md](MILESTONES.md) (what to build, in order) · [STATUS.md](STATUS.md) (where we are) · [re/](re/README.md) (what we know about the original) |

---

## 0. How to read this document

- Sections 1–6 are mandatory reading before any work. Sections 7–12 are reference material for the subsystem you are touching.
- This guide describes **how OBCR is built**. Facts about **how the original game works** live in [`docs/re/`](re/README.md). When this guide mentions an original-game fact, it carries a confidence tag (`CONFIRMED`, `LIKELY`, `HYPOTHESIS`, `UNKNOWN` — defined in [AGENT_GUIDE.md §4](AGENT_GUIDE.md#4-evidence-and-confidence)) and links to the evidence.
- "Wolf3D" means the id Software source release in `../wolf3d/WOLFSRC`. "ECWolf" means `../ECWolf`. "The original" or "COUNT.EXE" means Operation Body Count as shipped in `../OBC`.

---

## 1. Mission

OBCR is a modern, native, cross-platform source port of **Operation Body Count** (Capstone Software, 1994), a first-person shooter built on a licensed and substantially modified Wolfenstein 3D engine. No original source code exists. OBCR rebuilds the game from three things:

1. **The Wolf3D source**, OBC's direct ancestor, as the baseline.
2. **Reverse engineering of `COUNT.EXE`** in Ghidra, which tells us where and how OBC diverges from that baseline.
3. **Observation of the original running in DOSBox-X**, which is the final authority on behavior.

The result is written in Rust, runs on SDL3, reads the user's own copy of the original data files, and plays exactly like the original by default.

### 1.1 Goals (priority order)

1. **Faithful.** In Classic mode, gameplay matches the original tic for tic: same movement, same AI decisions, same damage, same random-number consumption. The 320×200 view matches the original pixel for pixel.
2. **Complete.** Every feature ships: all 40 single-player floors, the allies system, every game type (Difficulty 1–3, Random, Network, Network Teams, Modem), multiplayer, autosave and quicksave, cheats and debug features, cutscenes, music.
3. **Modern.** Native on Windows, Linux and macOS. Window and fullscreen at any resolution, correct aspect ratio, mouse and gamepad, rebindable keys, Internet multiplayer, and opt-in enhancements such as high resolution, widescreen and smooth motion.
4. **Verifiable.** Every ported behavior traces to evidence, and automated differential tests against the original catch regressions.
5. **Agent-friendly.** The codebase, tools and docs are built so AI agents can do most of the work safely: small, well-specified tasks; text-based reverse-engineering state; headless tests; machine-checkable acceptance criteria.

### 1.2 Non-goals (for 1.0)

- New content, a level editor, or an ECWolf-style mod/scripting system.
- Support for other Wolf3D-engine games (Corridor 7, Wolf3D itself, and so on).
- A polygon or GPU renderer, vertical look, or any change to the raycaster's fundamental look. Enhanced mode is still a raycaster.
- Bit-exact emulation of sound hardware. Audio must be correct in content and timing; synthesized waveforms are allowed to differ within the limits of OPL emulation.
- Shipping, embedding or committing any original game data (see §11).

---

## 2. Source materials and the hierarchy of truth

### 2.1 Inventory

| Material | Location (relative to `OBCR/`) | Authoritative for | Caveats |
|---|---|---|---|
| Original game | `../OBC/` | Data files; `COUNT.EXE` (main program) | **Read-only reference.** Never run the game in place: it writes `CONFIG.DAT`, saves and `BC.LOG`. Copy it to `.local/` first. |
| `COUNT.EXE` in DOSBox-X | built from `../dosbox-x/` | **Behavior.** The final word on what the game does. | Timing-dependent; needs the instrumentation in §9 to be reproducible. |
| Ghidra analysis of `COUNT.EXE` | `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC`; project in `.local/ghidra/` | Algorithms, constants, data layouts, tables | 16-bit segmented decompilation is imperfect. Cross-check anything subtle in DOSBox-X. |
| Wolf3D source | `../wolf3d/WOLFSRC/` | Baseline algorithms and naming; starting point for every port | **OBC modified this engine.** Never assume OBC = Wolf3D without a delta check (§2.3). |
| `WOLF3D.EXE` + `WOLF3D.MAP` | `../wolf3d/WOLFSRC/` | Symbol seeding. The EXE carries Turbo Debugger symbols (`FB52` signature at file offset 311232) and the MAP lists every public with its segment and offset. | Built from a slightly different Wolf3D revision. Use it to match library and engine functions, not as an OBC fact. |
| ECWolf | `../ECWolf/` | Proven modern solutions: format readers (it already reads `MAPTEMP`), high-res raycasting math, aspect correction, OPL playback, input, Steam detection (`src/filesys_steam.cpp` already knows OBC's Steam app id 1627120) | Not authoritative for OBC behavior. ZDoom-derived C++ architecture: port ideas, not structure. `src/mame/` has a non-GPL license: do not port from it. |
| DOSBox-X source | `../dosbox-x/` | Debug tooling: debugger, JSON-RPC agent (`docs/rpc-agent-usage.md`, `client/python`), control server (`mcp_server`) | Local modifications stay local (see AGENT_GUIDE §6.8). |
| Manual supplement | `../OBC/readme.txt` | Intended feature behavior (allies, autosave, quicksave, game types) | Marketing-level detail only. |

### 2.2 Hierarchy of truth

When sources disagree, the higher one wins:

1. Behavior observed in the original under DOSBox-X (trace, memory dump or capture).
2. `COUNT.EXE` disassembly or decompilation.
3. Original data files.
4. Wolf3D source.
5. ECWolf and other source ports.
6. Recollection, web pages, forum posts, and model memory.

Level 6 is never evidence on its own. It can only suggest where to look.

### 2.3 The delta check

Every function ported from Wolf3D goes through a **delta check** before it is called done:

1. Find the corresponding function in `COUNT.EXE`. Record its address in [`re/SYMBOLS.md`](re/SYMBOLS.md).
2. Compare its logic with the Wolf3D version: constants, branches, called functions, struct offsets.
3. Record every difference in [`re/DELTA_SPEC.md`](re/DELTA_SPEC.md) as a `D-###` entry, or record "no delta" if there is none.
4. Implement the OBC behavior, and cite the Wolf3D origin and the `D-###` entries in the doc comment.

A function that has not been delta-checked is marked `// DELTA-UNCHECKED` and cannot be marked done in [STATUS.md](STATUS.md).

---

## 3. What we know about the original (summary)

Detailed and continuously updated in [`re/`](re/README.md). Snapshot as of 2026-11-02:

### 3.1 Files

| OBC file | Wolf3D equivalent | Size | Facts | Confidence |
|---|---|---|---|---|
| `COUNT.EXE` | `WOLF3D.EXE` | 235,440 | 16-bit real-mode MZ, Borland C++ (1991 RTL), TLINK, **not packed**, no overlay, no debug info. Header 0x3400 bytes, 2,933 relocations spread over 34 segments (a lower bound on the segment count); DGROUP at image paragraph 0x3218 (file offset 0x35580). Requires a 286. | CONFIRMED |
| `GFXTILES.BC` | `VSWAP.WL6` | 2,084,536 | Standard page file: 937 chunks; walls 0–135 (all 64×64); sprites 136–815 (680); digitized sound pages 816–935; chunk 936 = 46-entry sound info list. | CONFIRMED |
| `MAPHEAD.BC` | `MAPHEAD.WL6` | 402 | RLEW tag `0xABCD` + 100 × int32 offsets; 61 used. No tile-info appendix. | CONFIRMED |
| `MAPTEMP.BC` | `GAMEMAPS.WL6` | 351,762 | `TED5v1.0` signature. **RLEW only, no Carmack compression** (Wolf3D's non-`CARMACIZED` path). 61 maps, all 64×64, plane 2 always empty. | CONFIRMED |
| `VGAHEAD/VGADICT/VGAGRAPH.BC` | `VGA*.WL6` | 177 / 1,024 / 317,566 | Huffman, 58 chunks: pic table (53 pics), 2 likely fonts, 53 pics (8 full-screen 320×200, one 320×40 status-bar-sized pic), 2 small 5-byte chunks. | CONFIRMED (counts); LIKELY (roles) |
| `AUDIOHED/AUDIOT.BC` | `AUDIO*.WL6` | 1,244 / 197,419 | 310 chunks; likely 100 sounds × (PC speaker, AdLib, digi placeholder) + IMF music. | LIKELY |
| `AUDIOMUS.BC` | — (new) | 1,376,256 | Exactly 21 × 64 KiB. Byte distribution centered on 0x80 with zero padding, so most likely raw unsigned 8-bit PCM background music. | LIKELY |
| `CONFIG.DAT` | `CONFIG.WL6` | 287 | 7 high scores × 28 bytes (`name[20]`, `int32 score`, `u16 completed`, `u16 episode`; same layout as the default table in `COUNT.EXE` at file 0x383E4), then Wolf-like settings including scan-code bindings. | CONFIRMED (scores); LIKELY (settings) |
| `SVGM*.SAV`, `AUTSV.SAV` | `SAVEGAM?.WL6` | 19,188 / 18,594 | Save games; layout unknown. | UNKNOWN |
| `SETUP.EXE`, `README.EXE`, `BC.BAT` | — | — | Sound setup, manual viewer, launcher that collects `BC.LOG` diagnostics. | CONFIRMED |

Not present in this install (Minimal install type): the **rendered sequences** played between floors and on death. The CD and Steam releases should have them. See [OPEN_QUESTIONS Q-015](re/OPEN_QUESTIONS.md).

### 3.2 Levels (from map headers, CONFIRMED)

| Slots | Names | Purpose (LIKELY) |
|---|---|---|
| 0–4 | SEWER LEVEL 1–5 | Single player |
| 5 | Maintence Level (sic) | Single player |
| 6–22 | UN BLDG1 LOBBY, FLR2–FLR16, TOP | Single player |
| 23–39 | BLDG2 FLR1–FLR16, TOP | Single player |
| 40–49 | CRED1–CRED10 | Credits scenes (tiny maps) |
| 50–59 | NETLEVEL 1–10 (stored out of order) | Multiplayer |
| 60 | CRED9 (empty planes) | Placeholder or unused |

### 3.3 Feature inventory (from the manual supplement and `COUNT.EXE` strings)

- **Game types**: Difficulty 1/2/3, Random, Network, Network Teams, Modem.
- **Allies**: Follow mode, Disperse/Attack mode, Switch (`A`: become an ally, inheriting their health but keeping your weapons), Switch & Stay (`S`). Ally count, identities and spawn rules are UNKNOWN (Q-017).
- **Weapons and ammo**: 9mm clips, 7mm clips, Uzi, Galil, Flame Thrower (Fuel Pack, Fuel Bottle), Grenades, Grenade Launcher. Melee is UNKNOWN.
- **Items**: Flak Jacket, Increased Health, Medic Pack, Map ThermoSensor, Booby Trap. A `HIDDENCARD` string exists (purpose UNKNOWN: key card, parameter or cheat).
- **World mechanics**: generators on/off, tremors (screen shake, HYPOTHESIS), push walls, fire that burns, booby traps.
- **Enemies** (from the manual's tips, LIKELY): rats, swamp creatures, a final boss named Victor; human enemies, probably terrorists (the network character list includes "Terrorist 1" and "Terrorist 2").
- **Saving**: named slots, an AutoSave slot written before each floor change, and F9 QuickSave to the last slot saved from the main menu.
- **Audio**: PC speaker, AdLib, Sound Blaster digitized SFX; FM music; PCM background music; Red Book CD music via MSCDEX (`cdmusic`); SB Pro mixer (master, FX and music volumes).
- **Presentation**: intro and outro pictures (`nopics` skips them), rendered sequences (`norndseq`, `slowrndseq`), help screen (`H` at main menu), high scores.
- **Multiplayer**: IPX network, serial null-modem and Hayes modem; teams; character choice (Agent, Terrorist 1, Terrorist 2); kill scoreboard; 10 net levels.
- **Diagnostics**: debug password, walk-through (noclip), invincibility, area and walk debug overlays, memory report, `BC.LOG` error log, `DEBUG.TXT`.
- **Command line**: `GOLEVEL`, `NOWAIT`, `COMP`/`NOCOMP`, `nojoys`, `nomouse`, `noems`, `noxms`, `nomain`, `no386`, `nopics`, `cdmusic`, `norndseq`, `slowrndseq`.

### 3.4 Confirmed engine-level facts

- `rndtable[256]` is byte-identical to Wolf3D's (`ID_US_A.ASM`). It lives at file offset 0x2D8F2. CONFIRMED.
- Map planes use OBC-specific codes: plane-0 floor codes 144–151 (with grid patterns that suggest per-tile floor or ceiling variation), a plane-1 background value of 18 instead of 0, and object codes up to about 317. CONFIRMED that they differ; meanings UNKNOWN. See [Q-001..Q-003](re/OPEN_QUESTIONS.md).
- Strings `Rindex`, `Findex`, `Rclr`, `Fclr` suggest roof and floor index or color handling, possibly textured or per-area floors and ceilings. HYPOTHESIS.

---

## 4. Fidelity model

### 4.1 Classic and Enhanced

OBCR has two **presets** over a set of independent options:

| Preset | Intent |
|---|---|
| **Classic** (default) | Plays and looks exactly like the original. 320×200 internal resolution, 4:3 presentation, 70 Hz simulation, original controls semantics, original quirks preserved. |
| **Enhanced** | Same game rules and same simulation. Presentation and comfort upgrades: high resolution, widescreen, interpolation, free mouse turning, modern menus for options, quality-of-life fixes. |

Hard rule: **no option may change simulation results unless it is explicitly listed as a gameplay option** (for example, "fix original bug K-012"). Gameplay options are saved in save files and multiplayer sessions, and they are off in Classic.

### 4.2 Parity targets

| Area | Target | How it is measured |
|---|---|---|
| Simulation | Bit-exact. Given the same level, the same starting state and the same per-frame `(tics, inputs)` sequence, every tracked variable matches the original on every frame. | Differential traces (§9.4) |
| Classic rendering | Pixel-exact 320×200 indexed frame for the same game state | Golden frames captured from DOSBox-X (§9.3) |
| UI and screens | Pixel-exact layouts and identical flow, timing and fades in Classic | Captures, frame-step comparison |
| Audio content | Same sound chosen, same priority decisions, same start and stop tic, same music track | Event traces |
| Audio waveform | Digitized and PCM data bit-exact before resampling. PC speaker exact in frequency and timing. OPL "as accurate as the chosen emulator". | Offline render plus listening review |
| Multiplayer | Same rules as the original. Transport is modern. | Rule tests; manual comparison using two DOSBox-X instances over emulated IPX |

### 4.3 Quirks and bugs

Every original bug we find gets a `K-###` entry in the Quirks section of [`re/DELTA_SPEC.md`](re/DELTA_SPEC.md), classified as one of:

- **Gameplay quirk** (affects play, for example a collision oddity). Preserved in Classic. May get an opt-in fix flag.
- **Crash or corruption** (would crash or corrupt memory). Always fixed. The fix must reproduce the original's behavior up to the point where it would have crashed, then fail safe.
- **Cosmetic** (visual glitch). Preserved in Classic, fixed in Enhanced.

Never "fix" anything silently. Undocumented divergence is a bug in OBCR.

---

## 5. Technology choices

Full rationale is in [DECISIONS.md](DECISIONS.md). Summary:

| Concern | Choice | Notes |
|---|---|---|
| Language | Rust, stable channel, edition 2024 | Toolchain pinned in `rust-toolchain.toml` (1.95.0 is installed locally as of 2026-11-02). |
| Platform layer | **SDL3** via the `sdl3` crate, built from source and statically linked | Window, input, gamepad, audio stream, timing, message boxes. One C dependency, stable API, available on every target. CMake and a C compiler are already installed. |
| Rendering | Our own software renderer into an 8-bit indexed framebuffer; palette expansion to RGBA; SDL streaming texture; GPU only for the final scale | Matches the original's pipeline, so parity is achievable and testable without a window. |
| Audio synthesis | Our own mixer. OPL2/OPL3 emulator behind a trait (initially Nuked-OPL3, LGPL-2.1+, via `cc`; a pure-Rust port is allowed later). PC speaker and PCM in Rust. | Never MAME `fmopl` (license). |
| Audio decoding (CD music replacement) | `symphonia` (or `lewton` for Ogg only) | User-supplied tracks only. |
| Serialization | `serde`; TOML for config; versioned binary for saves (`postcard`), JSON for debug dumps and traces | |
| CLI | `clap` | `obc-tool`, `obcr` flags |
| Logging | `tracing` + `tracing-subscriber` | Structured; `BC.LOG`-equivalent file sink |
| Errors | `thiserror` in libraries, `anyhow` in binaries | |
| Testing | built-in tests, `insta` (snapshots), `proptest` (parsers), optional `cargo-fuzz` | |
| Images and WAV for tools | `png`, `hound` | Tools only; the runtime does not need them |
| Networking | `std::net::UdpSocket` with our own lockstep protocol | No async runtime in the game. |
| Task automation | `cargo xtask` | No extra tool installs. Commands: `ci`, `lint`, `test-data`, `capture`, `golden`, `trace-diff`. |

Rejected: **Bevy** (ECS and render graph fight a tic-based single-threaded design; heavy compile times; API churn hurts agent reliability), **extending ECWolf** (fidelity and architecture mismatch; see ADR-002), **macroquad** (limited audio and windowing control), **winit + pixels + cpal + gilrs** (viable, but four moving parts instead of one; kept as a fallback if SDL3 bindings block us).

---

## 6. Architecture

### 6.1 Layers

```
 ┌──────────────────────────────────────────────────────────────────────┐
 │ obcr (binary)          app wiring, CLI flags, main loop, state stack │
 ├───────────────┬───────────────┬───────────────┬──────────────────────┤
 │ obc-ui        │ obc-render    │ obc-audio     │ obc-net              │
 │ menus, HUD,   │ software 3D   │ mixer, OPL,   │ lockstep, UDP,       │
 │ screens, flow │ view, 2D,     │ PC speaker,   │ lobby                │
 │               │ fades         │ PCM, music    │                      │
 ├───────────────┴───────┬───────┴───────────────┴──────────────────────┤
 │ obc-sim                │  deterministic game simulation (no I/O)     │
 ├────────────────────────┴─────────────────────────────────────────────┤
 │ obc-core     fixed-point math, angle tables, RNG, ids, tic constants │
 ├──────────────────────────────────────────────────────────────────────┤
 │ obc-formats  parsers for every original file (pure, no I/O policy)   │
 └──────────────────────────────────────────────────────────────────────┘
 obc-platform (SDL3)  window, input, audio device, clock — used only by obcr
 tools/obc-tool       asset inspection/export, trace diff, data validation
 tools/obc-oracle     DOSBox-X driver: traces, golden frames, input scripts
```

### 6.2 Repository layout

```
OBCR/
├── Cargo.toml                 workspace; shared [workspace.lints] and [workspace.dependencies]
├── rust-toolchain.toml
├── CLAUDE.md / AGENTS.md      agent entry points
├── README.md
├── crates/
│   ├── obc-formats/
│   ├── obc-core/
│   ├── obc-sim/
│   ├── obc-render/
│   ├── obc-audio/
│   ├── obc-ui/
│   ├── obc-net/
│   ├── obc-platform/
│   └── obcr/                  the game binary
├── tools/
│   ├── obc-tool/              Rust CLI
│   ├── obc-oracle/            Python (uses ../dosbox-x/client/python) or Rust; drives DOSBox-X
│   ├── ghidra/                headless scripts, symbol import/export
│   └── dosbox/                configs and runtime-dir templates
├── xtask/                     cargo xtask automation
├── re/                        machine-readable RE state (committed, text only)
│   ├── symbols/count_exe.csv  address → name → type → confidence
│   ├── types/obc_types.h      struct and enum definitions for Ghidra import
│   └── traces/schema/         trace field definitions per build of COUNT.EXE
├── docs/                      human-readable design, guides, RE knowledge base
├── tests/                     workspace integration tests (data-gated)
└── .local/                    git-ignored: Ghidra project, DOSBox runtime dirs, captures, traces, exports
```

### 6.3 Crate responsibilities and dependency rules

| Crate | Owns | May depend on | Must not |
|---|---|---|---|
| `obc-formats` | Reading every original file into typed Rust values: VSWAP, maps, VGAGRAPH, fonts, audio chunks, AUDIOMUS, palette from `COUNT.EXE`, `CONFIG.DAT`, original saves (read-only), rendered sequences | `thiserror`, `byteorder`-style helpers | Do file discovery policy, panic on bad input, or know about game rules |
| `obc-core` | `Fixed` (16.16), angle types, fine-angle tables, RNG (`rndtable` + index), tic constants, id newtypes (`SpriteId`, `SoundId`, `MapId`, `TileCode`) | — | Contain any `f32` or `f64` in values the simulation uses at run time |
| `obc-sim` | World state, level setup, player, actors, AI, allies, weapons, projectiles, doors, push walls, statics, areas, game rules, cheats' effects, events out (sound, HUD, palette flash, messages), trace emission | `obc-core`, `obc-formats` (for level data types) | Read the clock, touch files, call audio or render, use floating point, use `HashMap` iteration order for game logic |
| `obc-render` | 3D view (raycaster, sprites, weapon), 2D drawing (pics, fonts, tiles, bars), palette effects, fades, screen shake; Classic and high-res paths | `obc-core`, `obc-formats`, `obc-sim` (read-only view of world) | Mutate simulation state |
| `obc-audio` | Sound and music playback logic: priorities, channels, OPL, PC speaker, digitized, IMF, AUDIOMUS streaming, CD replacement, mixer | `obc-core`, `obc-formats` | Depend on SDL (it renders sample buffers; the platform layer delivers them) |
| `obc-ui` | Application state machine: title, menus, game-type selection, load/save UI, help, intermission and floor change, sequences, high scores, credits, HUD composition, in-game messages | `obc-core`, `obc-formats`, `obc-sim`, `obc-render`, `obc-audio` (via command queue) | Block or loop waiting for input (see §7.6) |
| `obc-net` | Lockstep session, input exchange, desync detection, lobby, transport | `obc-core`, `obc-sim` (input and hash types) | Leak network timing into simulation |
| `obc-platform` | SDL3: window, renderer and texture, events to `InputState`, audio device and stream, high-resolution clock, clipboard, message box | `sdl3` | Contain game logic |
| `obcr` | Composition root, CLI, config load, main loop | everything | — |

These dependency rules are enforced with a CI check (`cargo xtask lint` inspects `cargo metadata`).

### 6.4 The main loop

```
loop {
    platform.pump_events(&mut input_state);            // SDL → abstract input
    let now = platform.clock();
    tic_accumulator.add(now - last);                   // 70 Hz tic clock (1/70 s per tic)
    while tic_accumulator.take_tic() {                 // fixed-step; catch-up capped
        let cmd = input_mapper.sample(&input_state);   // TicCmd: controlx/controly/buttons
        net.exchange(cmd)?;                            // single-player: identity
        app.tick(&cmd);                                // ui state machine → sim.step(tics=1)
        audio.apply(app.drain_audio_events());
    }
    let alpha = tic_accumulator.fraction();            // only used by Enhanced interpolation
    app.render(&mut framebuffer, alpha);               // ui + render
    platform.present(&framebuffer, &palette);
}
```

- The simulation advances in **fixed 1-tic steps at 70 Hz** (ADR-003). The original advanced by a variable `tics` (1..MAXTICS) per rendered frame. `obc-sim::step(tics)` takes `tics` as a parameter so differential tests can replay the exact `tics` sequence recorded from the original.
- Catch-up is capped (for example 10 tics) so the game slows down instead of spiraling after a stall.
- Rendering never mutates simulation state. Classic mode renders the latest state with no interpolation and is capped to 70 fps by default.

### 6.5 Core conventions

| Concept | Convention |
|---|---|
| Fixed point | `Fixed(i32)`, 16.16, mirrors Wolf3D `fixed` (`GLOBAL1 = 1 << 16`). All simulation positions, speeds and distances. |
| Tile coordinates | `u16`/`i16` tile x,y in 0..63; world = tile << 16 (`TILESHIFT = 16`). |
| Angles | Integer degrees 0..359 for actor and player angles (`ANGLES = 360`); fine angles 0..3599 for rendering (`FINEANGLES = 3600`). Tables built to match the original exactly (§7.3.1). |
| Integer widths | Mirror the original's C types: `int` = `i16`, `unsigned` = `u16`, `long` = `i32`, `byte` = `u8`. Wherever the original could overflow, use explicit `wrapping_*` operations with a comment, never implicit release-mode wrapping. |
| Division and shifts | Borland signed `/` truncates toward zero and `>>` on signed values is arithmetic, which matches Rust. Assembly helpers such as `FixedByFrac` use **sign-magnitude** fractions and must be ported exactly. |
| Floating point | Forbidden in `obc-sim` and in any table the simulation reads, unless the table is verified bit-for-bit against a memory dump from the original. |
| Tics | `TICRATE = 70`. `MAXTICS` value for OBC to be confirmed (Wolf3D: 10). |
| RNG | `US_RndT` equivalent over the shared `rndtable` with a `u8` index. One RNG stream per world. Every call site documented, because call order is part of parity. Save files store the index. |
| Ids | Newtypes; original numeric values preserved (`SpriteId(u16)` = VSWAP page offset from sprite start; `SoundId(u16)` = audio chunk index base). Names come from RE (`re/symbols`). |
| Object order | The original's linked object list order (player first, new objects appended, removed objects unlinked) is preserved by an arena plus an explicit ordered list. Think order equals list order. |

### 6.6 Errors, panics and logging

- Parsers return `Result`. No `unwrap` or `expect` on data-derived values anywhere outside tests.
- `obc-sim` must not panic. Invariant violations log through `tracing::error!`, emit a `SimEvent::Fault`, and take the safest equivalent action. Debug builds may `debug_assert!`.
- The original's fatal errors (`Quit("...")`, `BC ERROR:` messages) map to a `GameError` shown in a message box and written to the log file, mirroring `BC.LOG`.
- `overflow-checks = true` in dev and test profiles, so accidental overflow shows up in tests. The release profile follows the explicit `wrapping_*` policy above.

### 6.7 Configuration, paths and data discovery

Data directory resolution order (first match wins):

1. `--data <dir>` command-line flag.
2. `OBC_DATA_DIR` environment variable.
3. `data_dir` in the user config (`<config dir>/obcr/obcr.toml`, via the `directories` crate).
4. Next to the executable (`./OBC`, `./data`).
5. Known store installs: Steam app 1627120 (`<library>/steamapps/common/<game>/C/BCCD`, per ECWolf's `wl_iwad.cpp`), plus any GOG layout we confirm later.
6. Developer convenience (debug builds only): `.local/data/obc-full` (installed files plus the rendered sequences from the floppy archives), then `../OBC`.

File lookup is case-insensitive on every OS. Data is identified by a SHA-256 manifest of known releases (`obc-formats::release`), so the game can say exactly which release it found, or which files are missing or unknown.

User-writable files (config, saves, logs, screenshots) live in the OS-appropriate user data directory, never in the game data directory.

---

## 7. Subsystem designs

### 7.1 Data layer (`obc-formats`)

**Principles:** pure functions from bytes to typed values; zero-copy where cheap; exhaustive bounds checks; every format documented in [`re/DATA_FORMATS.md`](re/DATA_FORMATS.md) before or with its implementation; round-trip or golden tests on real data (data-gated, §9.2).

| Module | Responsibility |
|---|---|
| `release` | Known-release manifests (SHA-256 per file), release detection, which optional components are present (sequences, CD tracks) |
| `vswap` | Page directory; walls as 64×64 column-major; sprite shape decoding (`leftpix`, `rightpix`, column offsets, post commands) to a 64×64 indexed image plus a mask; digitized sound page assembly from the info list |
| `maps` | `MAPHEAD` offsets; `MAPTEMP` headers (38 bytes); RLEW expansion with the file's tag; Carmack decoding as well (cheap, and it lets the tests cross-check against ECWolf); raw planes as `[u16; 4096]` |
| `level_codes` | **Interpretation** of plane values (walls, doors, floor/area codes, objects) driven by tables from RE. Kept separate from raw parsing so new findings change one table. |
| `vgagraph` | Huffman decode with `VGADICT` (node 254 = root); 3-byte offsets with sparse `0xFFFFFF`; pic table; unchained-VGA planar to linear conversion; fonts; any `tile8` blocks; external chunks |
| `audio` | `AUDIOHED`/`AUDIOT`: PC speaker sounds, AdLib sounds (instrument + block + notes), IMF music; `!ID!` markers; chunk-range layout (`NUMSOUNDS` from RE) |
| `audiomus` | PCM background-music container (layout from RE, Q-004) |
| `exe` | Reads tables we choose not to transcribe (palette, possibly fonts or text) from the user's `COUNT.EXE` at offsets verified per release hash |
| `config` | `CONFIG.DAT` read (high scores, original bindings) for one-time import |
| `savegame` | Original `.SAV` reader (stretch goal, read-only) |
| `sequences` | Rendered-sequence (cutscene) container and codec (format from RE, Q-015) |

### 7.2 Simulation (`obc-sim`)

#### 7.2.1 State model

```rust
pub struct World {
    pub level: LevelInfo,                 // map id, names, floor/ceiling info, par data
    pub tiles: TileMap,                   // 64×64 wall/door/floor codes after setup
    pub occupancy: OccupancyGrid,         // actorat equivalent: Empty | Wall(code) | Door(i) | Actor(h) | BlockingStatic(i)
    pub doors: Vec<Door>,                 // MAXDOORS limit from RE
    pub pushwall: PushWallState,
    pub statics: Vec<StaticObj>,          // statobjlist equivalent; capacity per RE
    pub actors: ActorArena,               // stable handles + ordered list (objlist semantics)
    pub areas: AreaGraph,                 // areaconnect, areabyplayer
    pub game: GameState,                  // gamestate equivalent: score, health, ammo, weapons, keys, difficulty, mode...
    pub allies: AllyState,                // OBC: roster, mode, controlled character
    pub rng: Rng,
    pub tic: u32,
    pub flags: WorldFlags,                // generators, tremors, cheats, debug
    pub events: Vec<SimEvent>,            // drained by the app each tic
}
```

- Field names and semantics follow the original's globals (`gamestate`, `player`, `tilemap`, `actorat`, `doorobjlist`, `statobjlist`, `areaconnect`, `areabyplayer`, `madenoise`, and so on). The mapping is recorded in `re/symbols/count_exe.csv`.
- `ActorArena` reproduces the doubly linked `objlist`: insertion at the tail, unlink on removal, a free list with the original capacity (Wolf3D `MAXACTORS = 150`; OBC value from RE). Iteration order equals the original's think order.
- Capacity limits (actors, statics, doors) are reproduced in Classic, because overflowing them changes behavior. They are soft-raised in Enhanced only if RE shows no gameplay dependency.

#### 7.2.2 Actor state machine

The original drives actors through `statetype` records `{ rotate, shapenum, tictime, think, action, next }` in static tables. OBCR keeps that design:

```rust
pub struct State { pub rotate: u8, pub sprite: SpriteId, pub tics: i16,
                   pub think: Option<ThinkFn>, pub action: Option<ActionFn>, pub next: StateId }
pub type ThinkFn = fn(&mut World, ActorHandle);
```

- State tables are transcribed into Rust `static` arrays, one module per actor class. Every entry cites its `COUNT.EXE` table address.
- `DoActor` and `DoThink` order of operations is ported exactly, including the "think every frame vs. on state change" split.
- New OBC actor kinds (allies, projectiles, fire, rats, swamp creatures, the boss) use the same mechanism.

#### 7.2.3 Per-tic order of operations

Port Wolf3D's `PlayLoop` order (`WL_PLAY.C`) and apply OBC deltas from RE (to be confirmed in M2):

1. Read the `TicCmd` (`controlx`, `controly`, `buttonstate`, special keys).
2. Player-control-dependent updates (`PollControls` result consumption, weapon change, ally commands, map toggle).
3. `MoveDoors`, `MovePWalls`.
4. For each object in list order, `DoActor` (the player is objlist[0] in Wolf3D).
5. Palette-shift bookkeeping (damage, bonus), OBC effects (fire, tremor timers, generator state).
6. Sound location updates (positional emitters to events).
7. `gamestate.TimeCount += tics` and other counters.
8. Checks for level exit, death, victory, and debug and cheat keys.

#### 7.2.4 Subsystems inside `obc-sim`

| Module | Ports / implements |
|---|---|
| `setup` | `SetupGameLevel`, `ScanInfoPlane`, area and door init, spawn functions; difficulty filtering; Random game type rules |
| `player` | `T_Player`, movement (`ControlMovement`, `Thrust`, `ClipMove`, `TryMove`), use button (doors, push walls, switches, elevators), pickups (`GetBonus` with OBC items), damage (`TakeDamage`, flak jacket), death |
| `weapons` | Attack state machines, ammo types (9mm, 7mm, fuel, grenades), hitscan (`GunAttack`), projectiles (grenade, launcher round, flame), splash, burn damage over time |
| `actors` | Spawn, `T_Stand`, `T_Path`, `T_Chase`, `T_Shoot`, `T_Bite`, pain and death per class; sight (`CheckLine`, `CheckSight`, `SightPlayer`); hearing (`madenoise`, area connectivity); drops |
| `allies` | OBC: follow and disperse modes, `CT_Path` path following, switching, switch-and-stay, health inheritance, throttles ("Ally stuck throttle", "Ally stalled") |
| `world_fx` | Generators, tremors, fire, booby traps, any OBC environment logic |
| `doors` | Door states, timing, locks and cards, area linking, sounds |
| `pushwall` | Push wall movement |
| `game` | Level progression, floor change (autosave hook event), victory, game types, scoring, high-score eligibility |
| `cheats` | Effects of cheat and debug commands (activation input handled in `obc-ui`) |
| `trace` | Snapshot of tracked fields per tic, same schema as oracle traces (§9.4) |
| `net_rules` | Multiplayer rules: spawns, frags, teams, respawn |

#### 7.2.5 Events out

The simulation communicates outward only through `SimEvent`s:

- `PlaySound { id, source: Positional(x, y) | Player | Ui, priority }`, `StopSound`, `StartMusic`, `StopMusic`
- `PaletteFlash { kind, amount }`, `ScreenShake { .. }`
- `HudChanged`, `Message { text_id, args }` (for example "Galil", "%d 9mm Clips")
- `LevelExit { kind }`, `PlayerDied`, `RequestAutosave`, `Victory`
- `Fault { .. }` (invariant violated; logged)

### 7.3 Renderer (`obc-render`)

#### 7.3.1 Classic path (parity target)

A faithful port of `WL_DRAW.C`, `WL_SCALE.C` and the assembly ray tracer (`WL_DR_A.ASM` `AsmRefresh`), plus OBC deltas:

- **Projection setup**: `CalcProjection`, `BuildTables` (`pixelangle`, `finetangent`, `sintable`/`costable`, `focallength`, `scale`, `heightnumerator`). The original builds some of these with floating point. OBCR either reproduces the same arithmetic or embeds tables generated by a tool that verifies them against a memory dump from the original. The test fails if any entry differs.
- **Walls**: per-column ray casting with the original's fixed-point stepping, `xpartial`/`ypartial`, door half-tile offsets, push-wall offsets, light and dark wall pages, `postsource`/`postx` column scaling. Scaling is a loop, not compiled scalers, but produces identical pixels.
- **Floors and ceilings**: Wolf3D clears to solid colors. OBC behavior is still unknown (Q-003): solid per-level colors, per-area colors (`Rclr`/`Fclr`), or textured (`Rindex`/`Findex`). Implement after M2 RE.
- **Sprites**: `spotvis`, the visible-statics and visible-actors list, `TransformActor` and `TransformTile`, distance sort, `ScaleShape` with post commands and transparency, directional rotation (`rotate` field, 8 angles), the player weapon overlay (`DrawPlayerWeapon`).
- **View**: view sizes (`SetViewSize`), border and bevels, status-bar area, palette shifts (red damage, white bonus, and any OBC fire or flash effects), fades (`VL_FadeOut`/`FadeIn` in tic-accurate steps), screen shake (tremors).
- **2D**: `VWB_DrawPic`, `VWB_DrawPropString`, fonts, `tile8`, `VWB_Bar`, `VWB_Hlin`/`Vlin`, the original's text-centering helpers.

Output: `Framebuffer { w: 320, h: 200, pixels: Vec<u8> }` plus the current 256-entry palette (6-bit VGA values, expanded to 8-bit with `(v << 2) | (v >> 4)`, matching what DOSBox-X does for screenshots; verify in M3).

#### 7.3.2 High-resolution path (Enhanced)

- Same algorithms evaluated at an integer scale factor (2×–8×) or an arbitrary resolution, using the generalized projection math ECWolf and Wolf4SDL use (`screenofs`, `viewscreenx`, wider `pixelangle` tables). Textures are still 64×64 indexed, sampled nearest.
- Widescreen is Hor+: vertical FOV preserved, horizontal widened, HUD pillarboxed or stretched by option.
- Interpolation: positions and angles lerped between the last two tic states for rendering only.
- High-res is a different code path from Classic and must never change Classic output. Golden tests run against both: Classic must match the original, and high-res must match its own snapshots.

#### 7.3.3 Presentation

`obc-platform` converts indexed pixels to RGBA through a 256-entry lookup, uploads to an SDL streaming texture, and presents it with:

- 4:3 aspect correction (320×200 shown as 320×240 logical) by default; "square pixels" optional.
- Nearest filtering by default; optional sharp-bilinear and CRT-style shader in Enhanced.
- Integer scaling option, letterboxing, fullscreen-desktop.

### 7.4 Audio (`obc-audio`)

| Source | Original behavior (Wolf3D baseline; OBC deltas from RE) | OBCR design |
|---|---|---|
| PC speaker SFX | One byte per service tick (Wolf3D: 140 Hz), byte → PIT divisor (`byte × 60`), 0 = silence | Band-limited square-wave synth at the output rate; exact frequencies and durations |
| AdLib SFX | Instrument (16 bytes), block, one note byte per 140 Hz tick, on OPL channel 0 | OPL emulator; register writes on the same tick schedule |
| IMF music | Register/value/delay stream at 700 Hz (Wolf3D), OPL channels 1–8 | Sequencer at the original rate; OBC rate to confirm (Q-006) |
| Digitized SFX | 8-bit unsigned mono pages from VSWAP via the info list; Wolf3D rate about 7042 Hz; SB Pro stereo positioning | Resample with a quality filter; OBC rate from RE (Q-005); positional panning per original formula |
| PCM background music (`AUDIOMUS.BC`) | Unknown container; 64 KiB blocks of 8-bit unsigned PCM (LIKELY) | Streaming player; layout and rate from RE (Q-004) |
| CD audio | MSCDEX Red Book tracks (`cdmusic`) | User-supplied files (`music/trackNN.{flac,ogg,wav}`), mapping from RE (Q-016); falls back to FM or PCM music |
| Mixer | SB Pro mixer: Master, FX, Music volumes | Same three sliders, plus OS-level output device selection |

Rules:

- Priority and channel semantics (`SD_PlaySound` priority comparison, digitized versus AdLib/PC fallback via `DigiMap`, `SD_SoundPlaying` checks used by game logic) are ported exactly. Game logic that queries "is a sound playing" gets the answer the original would give. This logic lives in a deterministic, tic-driven **sound state** that is part of the simulation-visible world; only waveform generation lives in the audio thread.
- The audio thread consumes an ordered command queue stamped with tic numbers, so timing is tic-accurate regardless of video frame rate.
- Sound modes are selectable like the original (PC speaker, AdLib, Sound Blaster digitized), plus "Modern" (digitized when available, best quality).

### 7.5 Input

- `obc-platform` turns SDL events into an `InputState` (keys held as **original PC scan codes** via a mapping table, mouse deltas, buttons, gamepad axes).
- `InputMapper` builds the per-tic `TicCmd` exactly as `PollControls` does: keyboard, mouse and joystick contributions to `controlx`/`controly`, run modifier, strafe, `buttonstate[]`, with the original's scaling and clamping. Enhanced mode adds unquantized mouse turning by allowing fractional accumulation.
- Bindings: original defaults (from `CONFIG.DAT`, confirmed in M2) plus modern defaults (WASD + mouse) as a separate profile. Rebindable in Enhanced menus.
- Text entry (save names, high scores) uses SDL text input mapped to the original's allowed character set.

### 7.6 Front end (`obc-ui`)

The original runs **nested blocking loops**: `DemoLoop` → `GameLoop` → `PlayLoop`, with menus (`US_ControlPanel`), `IN_Ack()` waits, fades and intermissions all blocking. OBCR converts these to an explicit, non-blocking **state stack**:

```rust
pub trait Screen {
    fn tick(&mut self, ctx: &mut UiCtx, cmd: &TicCmd) -> Transition;   // Push/Pop/Replace/None
    fn draw(&self, ctx: &mut DrawCtx);
}
```

- Linear scripted flows (title, intro, floor change, "Get Ready!", death, victory, credits) use a small `Sequence` builder: `fade_out(n) → show_pic(p) → fade_in(n) → wait_ack_or(tics) → …`, evaluated one tic at a time.
- Each original blocking function is ported as a screen or sequence whose tic-by-tic visible behavior matches the original, including fade step counts and timeouts.
- Screens: Title and intro, Main menu, New Game (game-type select), Difficulty, Sound (with mixer), Control config, Load, Save, Help, Quit confirm, In-game (HUD plus messages), Pause, Automap or ThermoSensor (if applicable), Floor change and Get Ready, Death and Expire, Victory and End, Credits, High-score entry and table, Network setup (IPX→UDP, Modem→direct connect), Player and team choice, Network scoreboard, Debug and diagnostics overlay.

### 7.7 Persistence

- **Saves**: OBCR's own format. Header (magic, format version, OBCR version, data-release id, level, slot name, timestamp, gameplay options) plus a postcard-serialized `World`, `GameState` and RNG index. Loading and then continuing must produce the same trace as never having saved (tested).
- **Slots**: named slots like the original; the **AutoSave** slot is written only by the game before floor changes; **F9 QuickSave** writes to the slot most recently saved from the main menu, and shows the original messages (`NO CURRENT SAVE`, `SAVED %s`, `UNABLE TO SAVE`) when appropriate. Enhanced adds unlimited slots.
- **Config**: `obcr.toml` (video, audio, input, gameplay options, paths). `CONFIG.DAT` import on first run (high scores, bindings) if found.
- **High scores**: own file, seeded from `CONFIG.DAT` when present.
- **Original `.SAV` import**: stretch goal after M12, read-only, behind `obc-formats::savegame`.

### 7.8 Multiplayer (`obc-net`)

- **Model**: deterministic **lockstep**. Every peer runs the full simulation; per tic, peers exchange `TicCmd`s; a tic executes when all commands for it have arrived. This mirrors how Wolf3D-era network games worked and needs no state replication. The original's model is to be confirmed in M13 RE.
- **Transport**: UDP with sequence numbers, redundant input sends (last N tics per packet), configurable input delay, keepalives, timeouts. The protocol is versioned and checks the data-release id and gameplay options at handshake.
- **Desync detection**: every N tics peers exchange a 64-bit hash of tracked simulation state. On mismatch, dump both traces for diagnosis.
- **Mapping of original modes**: Network (IPX) becomes LAN discovery (UDP broadcast) and host/join by IP. Network Teams becomes the same, plus team assignment. Modem and serial become a direct two-player connection by IP or hostname. Player limits, characters, teams and kill rules follow the original.
- **Not in scope**: NAT punch-through, dedicated servers, matchmaking (possible post-1.0).

### 7.9 Rendered sequences and CD music

- Sequences are optional data. Missing sequences are skipped exactly as `norndseq` would skip them.
- The decoder lives in `obc-formats::sequences`; playback timing (`slowrndseq` semantics) from RE.
- CD music needs a track mapping (which level or screen plays which track). Users drop audio files into a `music/` folder; the release manifest documents known track lists.

### 7.10 Developer features

- **Original cheats and debug keys**: ported 1:1, including activation sequences. The debug-mode password and the effects behind `WALKTHRU`, `INVINCIBLE`, `BEWITCHED`, `RESTOCK`, `HEALTH HELP`, `UNFINISHED BUSINESS`, `GENERATORS ON/OFF`, `TREMORS ON/OFF` and the diagnostics screen are documented in [`re/DELTA_SPEC.md`](re/DELTA_SPEC.md) as they are reverse engineered.
- **Developer console** (`~`, debug builds or `--dev`): `warp <map>`, `pos <x> <y> <angle>`, `give`, `god`, `noclip`, `tic` (single-step), `trace start|stop`, `screenshot`, `dump world`.
- **Overlays**: tile grid, actor states, area numbers, sound events, frame timing.
- **Trace output**: `--trace out.jsonl` writes the per-tic trace (§9.4).
- **Headless mode**: `obcr --headless --map 6 --replay input.rec --tics 2000 --trace out.jsonl` runs without a window or audio device. This is the workhorse for agents and CI.

---

## 8. Determinism contract

1. `obc-sim` is a pure function of (initial state, sequence of `(tics, TicCmd)`, gameplay options).
2. No wall-clock, no OS RNG, no floating point, no hash-map iteration order, no thread timing inside the simulation.
3. All randomness goes through `world.rng`, in the original call order.
4. Data-dependent behavior uses the original data only; the release id is part of the save and net handshake.
5. Cross-platform: identical results on every OS and architecture (integer-only math makes this achievable). CI runs the replay suite on at least Windows and Linux.

---

## 9. Verification architecture

### 9.1 Test pyramid

| Level | What | Where | Runs |
|---|---|---|---|
| Unit | Fixed-point math, RLEW, Huffman, RNG, collision helpers, state-table integrity | crate `#[cfg(test)]` | Always |
| Property and fuzz | Every parser on random and truncated input: no panic, bounded memory | `proptest`, optional `cargo-fuzz` | Always (fuzz: nightly job) |
| Data-gated golden | Real data: chunk counts, decoded image hashes, map plane hashes, sound lengths | `tests/` with `OBC_DATA_DIR` | When data is present (skip with an explicit message otherwise) |
| Render golden | 320×200 indexed frames for scripted camera positions compared with captures from the original | `tests/render_golden` | Data-gated |
| Differential trace | OBCR sim versus traces recorded from the original | `cargo xtask trace-diff` | Data- and capture-gated |
| Replay regression | Recorded OBCR input sessions compared with their stored trace hashes | `tests/replays` | Data-gated |
| Manual | Listening tests, feel tests, full playthroughs by the project owner | checklist in MILESTONES | Per milestone |

### 9.2 Data-gated tests

- Tests needing original data call `obc_testdata::require!()`. It reads `OBC_DATA_DIR` (or `../OBC` in-workspace), verifies the release manifest, and skips with a loud message if data is missing.
- CI machines without data still run every non-gated test. The owner's machine runs everything: `cargo xtask test-data`.
- Expected values derived from copyrighted data (images, sounds) are stored as **hashes**, never as extracted assets.

### 9.3 Golden frames from the original

1. The oracle (§9.5) starts `COUNT.EXE` with `GOLEVEL` or warps to a map, stops at a known render point (end of `ThreeDRefresh`), writes player `x`, `y` and `angle` (and any other state) into memory, renders, and captures the 320×200 indexed frame plus palette (by reading VGA memory or with an added capture RPC; see AGENT_GUIDE §6).
2. The capture is stored in `.local/captures/` and indexed in `re/captures.toml` (metadata only: map, position, angle, view size, frame hash).
3. OBCR renders the same state headlessly; the test compares indexed pixels exactly and writes a diff image on failure.

### 9.4 Differential traces

**Trace format** (`re/traces/schema/trace-v1.md`): JSON Lines, one record per frame of the original (or per tic for OBCR). Fields:

```json
{"frame":123,"tics":3,"cmd":{"cx":0,"cy":-35,"buttons":"0x0004"},
 "player":{"x":"0x001E8000","y":"0x00238000","angle":90,"health":100,"...":"..."},
 "gamestate":{"score":0,"ammo":[...],"weapon":2,"...":"..."},
 "rng":17,
 "actors":[{"slot":1,"class":"terrorist1","state":"0x1A2C","x":"...","y":"...","hp":25,"flags":"0x05","ticcount":4}],
 "doors":[{"i":0,"pos":"0xFFFF","action":"open","ticcount":0}],
 "sounds":[{"id":23,"pos":[10,12]}]}
```

- The schema names every field and its original memory location (from M2). OBCR's `trace` module emits the same fields.
- `obc-tool trace diff original.jsonl obcr.jsonl` reports the first divergence with context (fields, previous frames, last RNG consumers).
- OBCR replays the original's recorded `tics` and `cmd` per frame, so it does not depend on the original being deterministic. **Recorded inputs plus recorded tics, and OBCR's state must follow.**

### 9.5 The oracle (`tools/obc-oracle`)

A scriptable driver for the original running in the local DOSBox-X build. It uses the JSON-RPC debugger agent built into `../dosbox-x` (`--agent-config`, named pipe, Python client in `../dosbox-x/client/python`; see AGENT_GUIDE §6). Responsibilities:

- Prepare a runtime directory (a copy of the game data) and start a session.
- Set execution breakpoints at known addresses (from `re/symbols`), dump memory regions per frame into trace records, and resume.
- Inject state (`memory.write`): positions, angles, level warps, weapon grants.
- Inject input: write the original's keyboard state array, or use an input RPC we add to DOSBox-X (M0).
- Capture frames: read VGA memory (four planes) and the DAC palette, or use a capture RPC we add (M0).

---

## 10. Performance and platform targets

| Target | Requirement |
|---|---|
| Classic | 70 fps locked with under 2 ms per frame on any 2015+ x86-64 or ARM64 machine |
| Enhanced | 4K at 144 fps on a mid-range 2020 desktop CPU (software renderer, multi-threaded column rendering allowed as long as output is identical) |
| Startup | Under 1 s to title with data present |
| Memory | Under 256 MB resident |
| Platforms | Windows 10+ x64 (primary), Linux x64 (CI), macOS ARM64. Optional later: Linux ARM64, Steam Deck verification. |

---

## 11. Legal and data handling

- OBCR contains **no** original data: no graphics, sounds, maps, text dumps or decompiled code in the repository. `.gitignore` enforces the obvious paths, and `cargo xtask lint` scans for data-file signatures (`TED5v1.0`, VSWAP headers, MZ headers) in tracked files.
- Users supply their own copy (the retail floppy or CD, or the Steam release).
- **Decompiled code** from `COUNT.EXE` stays in `.local/` and is never pasted into the repository. Reimplementation is written from understanding, with comments that reference addresses and our own documentation.
- **License for OBCR's code**: **GPL-3.0-or-later** (ADR-006, accepted). Everything we draw from must be GPL-3.0-compatible: GPL-2.0-or-later and LGPL-2.1-or-later code is fine, GPL-2.0-only code is not. Never copy code from ECWolf's `src/mame/` or from any other incompatible source.
- Third-party crate and C library licenses are tracked with `cargo-deny` (already installed) and listed in `THIRD_PARTY_NOTICES.md` at release.
- "Operation Body Count" is a trademark of its owner. The project name stays OBCR; release materials must not imply endorsement.

---

## 12. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| 16-bit decompilation is hard to read (segments, far pointers, Borland helpers) | Slow RE, wrong conclusions | Wolf3D symbol seeding; DS and DGROUP setup; Borland runtime helper identification first; dynamic confirmation in DOSBox-X for anything subtle |
| OBC changed core engine internals more than expected (for example floors and ceilings, object codes) | Rework in the renderer and sim | M2 decodes level semantics before M4 and M5 begin; level-code interpretation isolated in tables |
| Original non-determinism (timer-driven tics) makes traces hard to compare | False divergences | Record `tics` and inputs per frame and replay them (§9.4) |
| Floating-point-built tables differ from Rust's results | Off-by-one pixel and movement drift | Dump tables from the original and verify; fall back to embedding verified tables generated by a tool |
| Missing optional data (rendered sequences, CD audio) | Incomplete feature set | Ask the owner for the CD or Steam release; design as optional components |
| SDL3 Rust bindings immaturity | Platform bugs | Thin `obc-platform` boundary; fallback stack (winit, pixels, cpal, gilrs) documented in ADR-001 |
| Netcode semantics unknown and IPX/serial hard to observe | Multiplayer mismatch | DOSBox-X emulates IPX and modems: run two instances and trace both; rules verified before transport work |
| Agents hallucinate Wolf3D-isms as OBC facts | Silent divergence | Evidence and confidence rules, the delta check, `DELTA-UNCHECKED` markers, verification gates in STATUS.md |
| Incompatible third-party license slips in | Release blocked | ADR-006 (GPL-3.0-or-later) plus `cargo deny` license allow-list |

---

## 13. Milestone overview

Full detail, tasks and acceptance criteria are in [MILESTONES.md](MILESTONES.md).

| # | Milestone | Outcome |
|---|---|---|
| M0 | Bootstrap and tooling | Workspace, CI, data discovery, DOSBox-X oracle, Ghidra pipeline, baseline captures |
| M1 | Data formats and asset pipeline | Every original file parsed; `obc-tool` exports everything |
| M2 | Reverse-engineering foundation | Symbols, structs, state tables, level-code semantics, trace points |
| M3 | Platform shell | SDL3 window, palette, fades, input, audio device, 70 Hz clock, title screen |
| M4 | Classic renderer | Pixel-exact 3D view and free-camera map viewer |
| M5 | World simulation core | Walkable levels with doors, push walls, pickups, exits; trace parity for movement |
| M6 | Audio | All SFX and music paths with original priority semantics |
| M7 | Weapons, combat and HUD | Full weapon set, projectiles, damage, status bar |
| M8 | Enemies and AI | Every enemy class with trace parity |
| M9 | Allies | The ally system with trace parity |
| M10 | Game flow and front end | Menus, game types, transitions, sequences, endings, credits, cheats |
| M11 | Persistence | Saves, AutoSave, QuickSave, config, high scores |
| M12 | Single-player parity certification | Full-game parity audit; 1.0-SP |
| M13 | Multiplayer | Network, Teams and Modem modes over UDP |
| M14 | Enhanced mode | High-res, widescreen, interpolation, modern controls, quality of life |
| M15 | Release engineering | Packaging, data setup UX, notices, release |

---

## 14. Glossary

| Term | Meaning |
|---|---|
| Tic | 1/70 s simulation step (`TICRATE`) |
| `TicCmd` | One tic's player input: `controlx`, `controly`, buttons, special keys |
| Plane 0 / 1 / 2 | Map layers: walls and floor codes / objects / unused in OBC |
| RLEW | Run-length encoding on 16-bit words with a tag word (0xABCD) |
| Carmack compression | LZ-style map compression (not used by OBC) |
| VSWAP / page file | Container of wall pages, sprite pages and digitized sound pages (`GFXTILES.BC`) |
| DGROUP | The program's data segment (DS) in the Borland memory model |
| Delta | A behavioral difference between OBC and Wolf3D (`D-###`) |
| Quirk | An original bug or oddity we preserve or fix deliberately (`K-###`) |
| Oracle | The original game running under the instrumented DOSBox-X, used as ground truth |
| Golden | A stored expected output (frame hash, trace) used by tests |
| Classic / Enhanced | Fidelity presets (§4.1) |
