# OBCR Milestone Plan

| | |
|---|---|
| Version | 1.0 — 2026-11-02 |
| Companions | [DESIGN.md](DESIGN.md) · [AGENT_GUIDE.md](AGENT_GUIDE.md) · [STATUS.md](STATUS.md) |

Each milestone has a goal, prerequisites, reverse-engineering (RE) tasks, implementation tasks, deliverables, and **acceptance criteria**. A milestone is done only when every acceptance criterion is met and recorded in [STATUS.md](STATUS.md) with evidence (a command, test name, or capture id).

Task ids are `M<milestone>.<n>`. RE tasks are `M<milestone>.R<n>`. Agents claim tasks in STATUS.md before starting (see [AGENT_GUIDE §3](AGENT_GUIDE.md#3-task-lifecycle)).

---

## Dependency graph

```
M0 ─► M1 ─┬─► M2 ─┬─► M4 ─────────────────────────────┐
          │       └─► M5 ─► M7 ─► M8 ─► M9 ─┐          │
          └─► M3 ─┬─► M4                    │          │
                  ├─► M6 (needs M1, M3; M5 for in-game triggers)
                  └─► M10 (menus can start after M3; game flow needs M5, M7)
                                            ▼          ▼
                                   M11 ─► M12 ─┬─► M13
                                               ├─► M14 (renderer parts may start after M4)
                                               └─► M15
```

RE runs continuously. M2 builds the foundation; every later milestone has its own RE tasks.

**Parallel streams** once M1 and M3 are done:

- Stream A, Simulation: M5 → M7 → M8 → M9 (sequential: each builds on the last).
- Stream B, Presentation: M4 → M14 renderer parts.
- Stream C, Audio: M6.
- Stream D, Front end: M10 menus and screens.
- Stream E, RE: ahead of the stream that needs it.

---

## M0 — Bootstrap and tooling

**Goal:** everything agents need to work efficiently and to check their work against the original.

**Prerequisites:** none.

### Implementation tasks

| Id | Task |
|---|---|
| M0.1 | Cargo workspace with every crate from DESIGN §6.2 as an empty library (module docs only), `obcr` and `obc-tool` binaries printing `--version`. `rust-toolchain.toml` pinned to the installed stable (1.95.0). `[workspace.lints]`: `unsafe_code = "deny"` (exceptions per crate with justification), `clippy::all = "warn"`, `clippy::pedantic` selectively. `rustfmt.toml`. Profiles: `dev` and `test` with `overflow-checks = true`. |
| M0.2 | `xtask` crate with `ci` (fmt check, clippy `-D warnings`, test, deny), `lint` (crate dependency rules from DESIGN §6.3; scan of tracked files for original-data signatures), `test-data`, and placeholders for `capture`, `golden`, `trace-diff`. |
| M0.3 | `.gitignore` and `.gitattributes` final; `cargo-deny` config (`deny.toml`) allowing GPL-compatible licenses only. |
| M0.4 | Data discovery in `obc-formats::release` (or a small `obc-data` module): resolution order from DESIGN §6.7; case-insensitive lookup; SHA-256 manifest for this copy (record it as release `OBC-A`, file dates 1996-12-24). |
| M0.5 | `obc-tool info [--data DIR]` prints the release id, the file inventory with sizes and hashes, missing optional components (sequences, CD audio). |
| M0.6 | `obc_testdata` helper (`require!()`) for data-gated tests. |
| M0.7 | Local builds only (owner decision, 2026-11-02): `cargo xtask ci` is the gate. Revisit if a remote is added. |

### Tooling tasks

| Id | Task |
|---|---|
| M0.8 | **DOSBox-X oracle build.** Build `../dosbox-x/vs/dosbox-x.sln` configuration `Agent Debug SDL2|x64` (defines `C_DEBUG`, `C_HEAVY_DEBUG`, `C_DOSBOX_AGENT`). Output: `../dosbox-x/bin/x64/Agent Debug SDL2/dosbox-x.exe`. Record the exact msbuild command and result in `tools/dosbox/README.md`. Use `Agent Debug No Heavy SDL2` if heavy debug is too slow for normal play. |
| M0.9 | **Runtime directory tooling.** `tools/dosbox/` script that copies `../OBC` into `.local/dosbox/run-<name>/` (never runs the pristine copy), writes an agent `.env` config (named pipe endpoint unique per run), and a DOSBox-X `.conf` (fixed cycles, `core=normal`, Sound Blaster 16 plus OPL, fixed `memsize`). |
| M0.10 | **Oracle client.** `tools/obc-oracle` (Python 3.10+, run with `python -I`, imports `../dosbox-x/client/python/dosbox_agent`). Commands: `start`, `bp`, `run-to`, `regs`, `read`, `write`, `dump-frames` (breakpoint loop writing JSONL), `stop`. Smoke test: start `COUNT.EXE`, stop at entry, read the PSP, continue, stop. |
| M0.11 | **DOSBox-X agent extensions** (local branch `obc-tools` in `../dosbox-x`; never sent upstream). Add `input.key` (press or release by scan code at the keyboard controller) and `video.capture` (current VGA mode, 320×200 indexed frame including unchained mode, and DAC palette) RPC methods, following the existing agent's layering (`src/agent/`). Document them in `tools/dosbox/README.md`. |
| M0.12 | **Ghidra bootstrap.** `tools/ghidra/` headless scripts: import `COUNT.EXE` (MZ loader, `x86:LE:16:Real Mode`), set DS to DGROUP across code, run analysis, apply `re/symbols/count_exe.csv` and `re/types/obc_types.h`, export the function list, all decompilations to `.local/ghidra/export/` (one file per function), and strings with segment:offset. Reproducible from scratch with one command. |
| M0.13 | **Wolf3D symbol seeding.** Import `../wolf3d/WOLFSRC/WOLF3D.EXE`, apply names from `WOLF3D.MAP` (publics by value; add Ghidra's load-segment base), then match into `COUNT.EXE` using Function ID, BSim or Version Tracking. Write matches to `re/symbols/count_exe.csv` with confidence `LIKELY` and source `wolf3d-match`. |
| M0.14 | **Baseline captures** of the original (in `.local/captures/`, indexed in `re/captures.toml`): every menu screen, the intro sequence, first frame of each single-player level (via `GOLEVEL`), the status bar in several states, a death, a floor change, and a 2-minute audio recording of music and SFX. Indexed by name, map and description. |
| M0.15 | Owner decisions recorded in DECISIONS.md: license (ADR-006), remote and CI (M0.7), whether CD or Steam data with rendered sequences is available (Q-015). |

### Acceptance criteria

- [ ] `cargo xtask ci` passes on a clean checkout (no data needed).
- [ ] `obc-tool info` on `../OBC` prints all 21 files with matching manifest hashes and identifies release `OBC-A`.
- [ ] `tools/obc-oracle` can start `COUNT.EXE`, set a breakpoint, run to it, read and write memory, and stop cleanly, all from a script.
- [ ] The Ghidra import is reproducible from scratch with one documented command, and the symbol CSV round-trips (export → fresh import → export gives identical files).
- [ ] At least 100 Wolf3D-derived names applied to `COUNT.EXE` with `LIKELY` confidence.
- [ ] `re/captures.toml` lists the baseline captures.
- [ ] Owner decisions recorded (or explicitly deferred with a date).

---

## M1 — Data formats and asset pipeline

**Goal:** every original file can be parsed, inspected and exported. No game logic yet.

**Prerequisites:** M0.1–M0.6.

### RE tasks

| Id | Task |
|---|---|
| M1.R1 | Confirm the exact location and count of palettes in `COUNT.EXE` (candidates at file offsets 0x2DDB0 and 0x2E0B0) and how they are used (game palette, fade or effect palettes). Q-025. |
| M1.R2 | Determine `NUMSOUNDS` and the audio chunk layout (PC, AdLib, digi, music ranges; meaning of the 4-byte chunk 300). Q-007. |
| M1.R3 | Determine the `AUDIOMUS.BC` layout and sample rate: find the loader (string "AUDIOMUS." at file 0x3822C), the playback routine, and the DSP time constant or rate written to the Sound Blaster. Q-004. |
| M1.R4 | Determine the digitized SFX sample rate (DSP time constant). Q-005. |
| M1.R5 | Identify VGAGRAPH chunks 1 and 2 (fonts?), chunks 56 and 57 (5 bytes each), and the role of each of the 53 pics. Q-008, Q-009. |
| M1.R6 | `CONFIG.DAT` full layout (settings after the high-score table, extra button bindings). Q-014, Q-030. |

### Implementation tasks

| Id | Task |
|---|---|
| M1.1 | `vswap`: directory, walls, sprite shape decoder (to indexed 64×64 plus mask), sound info list, digitized sound assembly. |
| M1.2 | `maps`: MAPHEAD, MAPTEMP (RLEW), Carmack (for completeness), raw planes. |
| M1.3 | `vgagraph`: Huffman, sparse offsets, pic table, planar → linear, fonts, external chunks. |
| M1.4 | `audio`: AUDIOHED and AUDIOT chunk reader; PC speaker, AdLib sound and IMF music structures. |
| M1.5 | `exe`: palette reader at offsets verified per release hash; generic "read table at offset" helper for later tables. |
| M1.6 | `audiomus` reader per M1.R3. |
| M1.7 | `config`: CONFIG.DAT reader. |
| M1.8 | `obc-tool` exporters: `vswap export` (PNG sheets for walls and sprites with indices; WAV for digitized sounds), `maps list`, `maps render <n>` (PNG with wall textures plus labeled object codes; ASCII and JSON dumps of raw planes), `maps find --plane P --value V` (every map and tile where a code occurs), `maps stats` (value histograms per plane), `vga export` (PNG per pic and font sheet), `audio export` (PC speaker and AdLib SFX and IMF music to WAV; requires a minimal OPL path or defers to M6), `pal export`, `audiomus export`. All exports go to `.local/exports/`. |
| M1.9 | Property tests and a fuzz target for every parser: never panic, never allocate unbounded memory. |
| M1.10 | Data-gated golden tests: chunk counts (937 VSWAP chunks, 61 maps, 53 pics, 310 audio chunks), per-asset hashes of decoded output. |
| M1.11 | `re/DATA_FORMATS.md` complete for every format implemented, with confidence tags. |

### Acceptance criteria

- [ ] Every file in the release manifest has a parser, and `obc-tool` can export its contents.
- [ ] Exported walls, sprites and pics look correct when viewed (agents can view PNGs with the Read tool) and match the corresponding baseline captures from M0.14.
- [ ] Digitized sounds play correctly in an external player at the RE-determined rate.
- [ ] Parsers survive 1 million fuzz or property cases without panic.
- [ ] Data-gated golden tests pass.
- [ ] Q-004, Q-005, Q-007, Q-008, Q-009, Q-014, Q-025 resolved or explicitly narrowed with next steps.

---

## M2 — Reverse-engineering foundation

**Goal:** a reliable map of `COUNT.EXE`: what each segment and important function is, how the core data structures are laid out, what every map code means, and where to read state for traces.

**Prerequisites:** M0.10–M0.13, M1.2.

### RE tasks

| Id | Task |
|---|---|
| M2.R1 | **Segment map.** Establish the full segment list (the 34 segments containing relocation entries are a lower bound; modules without far references have none) and map each segment to a probable source module (Wolf3D builds one `_TEXT` segment per C or ASM file, as `WOLF3D.MAP` shows). Table in `re/SYMBOLS.md`. |
| M2.R2 | **Borland runtime.** Identify RTL helpers (`LXMUL@`, `LDIV@`, `LUDIV@`, `LMOD@`, `LXLSH@`, `LXRSH@`, `N_SCOPY@`, `F_SCOPY@`, `_fmemcpy`, `_fmemset`, file I/O, `printf`, `exit`) so decompilations read cleanly. |
| M2.R3 | **ID library.** Match `ID_CA`, `ID_MM`, `ID_PM`, `ID_IN`, `ID_SD`, `ID_US`, `ID_VL`, `ID_VH` functions; record deltas (the cache manager reads `.BC` files, `GFXTILES`, `MAPTEMP`). |
| M2.R4 | **Main loops.** Locate the equivalents of `main`, `InitGame`, `DemoLoop`, `GameLoop`, `PlayLoop`, `SetupGameLevel`, `ScanInfoPlane`, `ThreeDRefresh`, `DoActor`, `PollControls`, `CalcTics`. |
| M2.R5 | **Globals.** Locate and type: `gamestate`, `player`, `objlist` and free list, `tilemap`, `actorat`, `spotvis`, `doorobjlist`, `doorposition`, `statobjlist`, `areaconnect`, `areabyplayer`, `madenoise`, `tics`, `TimeCount`, `controlx`, `controly`, `buttonstate`, `Keyboard[]`, `viewsize`, `rndindex`. Record DS offsets. |
| M2.R6 | **`objtype` layout** (size and every field, including OBC additions for allies and projectiles). Confirm against live memory in DOSBox-X. `re/types/obc_types.h`. |
| M2.R7 | **`statetype` layout and all state tables.** Enumerate every state record reachable from spawn functions; label think and action functions; export as a table per actor class. |
| M2.R8 | **Class enumeration** (`classtype`) and the spawn function per class. |
| M2.R9 | **Level-code semantics.** From `ScanInfoPlane`/`SetupGameLevel`, decode every plane-0 and plane-1 value used by the 61 maps: walls, doors (types, locks), floor codes (144–151 and others), area numbers, plane-1 background (18), wall-attribute values (89, 103, 104), player starts, enemies (with difficulty variants), allies, items, decorations, path markers, exits and elevators, generators. Q-001, Q-002. Output: `re/LEVEL_CODES.md` plus a machine-readable table consumed by `obc-formats::level_codes`. |
| M2.R10 | **Floors and ceilings.** How OBC draws them (`Rindex`, `Findex`, `Rclr`, `Fclr` strings). Q-003. |
| M2.R11 | **Timing.** Timer ISR rate, `TimeCount` increments, `MAXTICS`, `CalcTics` logic. Q-024. |
| M2.R12 | **Level progression.** Order of maps, how floors chain (elevators or exits), where Sewer → Maintenance → UN Building 1 → Building 2 transitions happen, secret or bonus floors, credits maps usage, net levels. Q-010, Q-011, Q-032. |
| M2.R13 | **Sprite and sound naming.** Using M1 exports plus state tables plus `SD_PlaySound` call sites, assign names to all 680 sprites and all sounds. Tables in `re/SYMBOLS.md` and `re/symbols/`. Q-022. |
| M2.R14 | **Trace points.** Choose the addresses at which to sample state once per frame (for example right after `DoActor` loop) and define trace schema v1 (`re/traces/schema/trace-v1.md`) with the memory location for every field. |
| M2.R15 | Draft `DELTA_SPEC.md` entries for every delta found in M2.R3–M2.R12. |

### Implementation tasks

| Id | Task |
|---|---|
| M2.1 | `obc-oracle dump-frames --schema trace-v1`: runs the original, samples state at the trace points every frame, writes JSONL. Must handle hundreds of frames at acceptable speed (seconds per 100 frames). |
| M2.2 | `obc-oracle setstate`: warp to a map, set player position and angle, give items (for render goldens and scenario setup). |
| M2.3 | `obc-tool maps render` uses `level_codes` to draw labeled object maps for all 61 maps; spot-check against captures. |
| M2.4 | `obc-core`: `Fixed`, angles, `rndtable` and RNG (with tests against values dumped from the original), tic constants. |

### Acceptance criteria

- [ ] Every segment has a module attribution (or `UNKNOWN` with notes).
- [ ] At least 80% of the functions in Ghidra have meaningful names, and every function on the `PlayLoop` call path is named.
- [ ] `objtype`, `statetype`, `gamestate`, door and static structures are typed in `re/types/obc_types.h` and verified against live memory.
- [ ] Every distinct plane-0 and plane-1 value present in the 61 maps has a classification in `re/LEVEL_CODES.md` (`UNKNOWN` allowed only with a recorded investigation).
- [ ] A 600-frame trace of a real play session on map 0 is captured with `obc-oracle`, and its fields validate against trace schema v1.
- [ ] RNG tests: OBCR's RNG reproduces 1,000 consecutive values from the original's trace.

---

## M3 — Platform shell

**Goal:** a window that shows original screens with correct colors, fades and timing; input and audio devices open; the app state machine skeleton exists.

**Prerequisites:** M1.3, M1.5.

| Id | Task |
|---|---|
| M3.1 | `obc-platform`: SDL3 init through the `sdl3` crate (build from source, static link). Window (resizable, fullscreen-desktop toggle `Alt+Enter`), streaming texture, vsync control, aspect-correct present (4:3 default), integer-scale option. |
| M3.2 | Palette pipeline: 6-bit VGA to 8-bit expansion (verify against DOSBox-X screenshots), palette-change and fade support at tic granularity. |
| M3.3 | Input: SDL scancode → original PC scan-code table; key state array; mouse relative mode; gamepad via SDL gamepad API; text input. |
| M3.4 | Clock: 70 Hz tic accumulator with capped catch-up; frame pacing. |
| M3.5 | Audio device: SDL audio stream (48 kHz stereo f32); a silent mixer stub that `obc-audio` will fill. |
| M3.6 | `obc-ui`: `Screen` trait, state stack, `Sequence` builder (DESIGN §7.6). |
| M3.7 | Title and intro sequence using pics from VGAGRAPH, with fade timing matched to the original (frame-step comparison against M0.14 captures). Honor the `nopics` equivalent (`--nopics`). |
| M3.8 | Screenshot key (PNG of the indexed framebuffer through the palette) and `--headless` flag plumbing. |
| M3.9 | Logging setup (`tracing`, log file in the user data dir), fatal error message box. |
| M3.10 | Config file (`obcr.toml`) load and save with defaults. |

### Acceptance criteria

- [ ] `obcr` opens a window and plays the title and intro pics with correct colors; a screenshot from OBCR matches the DOSBox-X capture pixel for pixel (after palette expansion).
- [ ] Fade durations match the original within ±1 tic (frame-step comparison).
- [ ] Window resize, fullscreen toggle and aspect correction work on Windows and Linux.
- [ ] Keyboard, mouse and gamepad input reach `InputState`; a debug overlay displays them.
- [ ] `obcr --headless` runs the title sequence without opening a window or audio device.

---

## M4 — Classic renderer

**Goal:** a pixel-exact port of the 3D view, plus a free-camera map viewer for every level.

**Prerequisites:** M1, M2.R5, M2.R9, M2.R10, M3.

### RE tasks

| Id | Task |
|---|---|
| M4.R1 | Delta-check `CalcProjection`, `BuildTables`, `SetViewSize`, `ThreeDRefresh`, `WallRefresh`/`AsmRefresh`, `ScalePost`, `HitVertWall`/`HitHorizWall`, door and push-wall hit code, `DrawScaleds`, `ScaleShape`, `SimpleScaleShape`, `DrawPlayerWeapon`, `TransformActor`/`TransformTile`, `CalcHeight`. |
| M4.R2 | Dump the computed tables (`pixelangle`, `finetangent`, `sintable`, `scale` tables, view-size-dependent values) from memory for every view size. |
| M4.R3 | Floor and ceiling algorithm (from M2.R10) in full detail. |
| M4.R4 | Palette shift and flash effects (damage, bonus, fire, anything new), screen shake (tremors). |
| M4.R5 | Wall-page selection rules (light and dark pages, door sides, OBC special walls such as animated or switchable walls, `ANumSWalls`). |

### Implementation tasks

| Id | Task |
|---|---|
| M4.1 | Projection setup and tables; tests against the M4.R2 dumps. |
| M4.2 | Wall raycaster (columns, doors, push walls, wall-page selection). |
| M4.3 | Floor and ceiling rendering per M4.R3. |
| M4.4 | Sprite visibility, transform, sort and scaling with transparency; directional sprites. |
| M4.5 | Player weapon overlay. |
| M4.6 | View sizes, borders, status-bar region, palette shifts, screen shake. |
| M4.7 | 2D primitives: pics, fonts and proportional strings, bars, lines, `tile8`. |
| M4.8 | **Map viewer**: `obcr --viewer [--map N]` free camera (no collision), shows tile coordinates and angle, steps through all 61 maps, renders statics and initial actors from level codes. |
| M4.9 | Render golden tests: for at least 5 positions per map category (sewer, maintenance, UN building, building 2, net) plus specific door, push-wall and sprite-heavy views, capture with `obc-oracle setstate` plus `video.capture` and compare exact indexed pixels. |

### Acceptance criteria

- [ ] All render goldens pass with zero differing pixels (at least 30 reference views, every view size used by the original).
- [ ] Table tests match memory dumps exactly.
- [ ] The map viewer runs every map with no panic and no visual garbage, at over 1,000 fps headless on the reference machine.
- [ ] Every M4.R task has its delta entries in DELTA_SPEC.

---

## M5 — World simulation core

**Goal:** levels are playable to walk through: player movement, collision, doors, push walls, pickups (non-weapon), exits and floor changes. Trace parity for everything without enemies.

**Prerequisites:** M2, M4 (for visual feedback; the sim does not depend on it).

### RE tasks

| Id | Task |
|---|---|
| M5.R1 | Delta-check `PollControls` (keyboard, mouse, joystick paths, run and strafe, speeds), `ControlMovement`, `Thrust`, `ClipMove`, `TryMove`, `T_Player`. |
| M5.R2 | Doors: `OpenDoor`, `CloseDoor`, `OperateDoor`, `MoveDoors`, `DoorOpening`, `DoorClosing`, locks and cards, timing constants, area connection on open. Door codes per M2.R9. |
| M5.R3 | Push walls: `PushWall`, `MovePWalls`, distance, speed, sounds. |
| M5.R4 | Statics and pickups: `SpawnStatic`, `PlaceItemType`, `GetBonus`: health items, flak jacket, ThermoSensor, cards, fuel, any keys. |
| M5.R5 | Areas: `InitAreas`, `ConnectAreas`, `madenoise` propagation. |
| M5.R6 | Exits: elevator or exit switches, floor transition triggers, generators and tremors (if world-level), booby traps (triggering; damage in M7). |

### Implementation tasks

| Id | Task |
|---|---|
| M5.1 | `World` and `GameState` structures; level setup from level codes; actor arena with objlist semantics. |
| M5.2 | `TicCmd` construction (`InputMapper`) identical to `PollControls`. |
| M5.3 | Player movement and collision. |
| M5.4 | Doors, push walls, statics, pickups, areas. |
| M5.5 | Exits and floor change events; game-state carry-over between floors. |
| M5.6 | `trace` module emitting schema v1; `obcr --headless --replay` consuming the original's `(tics, cmd)` per frame from a trace file. |
| M5.7 | `obc-tool trace diff`. |
| M5.8 | Developer console: `warp`, `pos`, `noclip`, `tic`. |
| M5.9 | Playable "walkthrough" build: `obcr` lets you walk every single-player level, open doors, push walls, collect items and exit, with enemies not spawned. |

### Acceptance criteria

- [ ] Five original sessions of at least 1,000 frames each (movement, strafing, running, door use, push walls, pickups, a floor change), recorded with enemies disabled through memory pokes, replay in OBCR with **zero divergence** in player, door, push-wall, static and gamestate fields.
- [ ] Every single-player map can be walked from start to its exit.
- [ ] `obc-sim` has no floating point, no I/O, and is covered by unit tests for collision corner cases.

---

## M6 — Audio

**Goal:** every sound and music path, with original selection, priority and timing semantics.

**Prerequisites:** M1.4, M1.6, M3.5. In-game triggering needs M5; standalone playback does not.

### RE tasks

| Id | Task |
|---|---|
| M6.R1 | Delta-check `ID_SD`: `SD_PlaySound` (priority rules), `SD_SoundPlaying`, `SD_StopSound`, `SD_WaitSoundDone`, PC speaker and AdLib service routines and their rates, `DigiMap`, digitized playback, stereo positioning (`SD_PositionSound`, `SetSoundLoc`). |
| M6.R2 | Music: IMF rate and track list per level and screen; AUDIOMUS selection and looping; CD track mapping (`cdmusic`). Q-006, Q-016. |
| M6.R3 | SB Pro mixer semantics (master, FX, music ranges and defaults). |

### Implementation tasks

| Id | Task |
|---|---|
| M6.1 | OPL emulator integration behind an `Opl` trait (Nuked-OPL3 via `cc`, LGPL-2.1+; record in THIRD_PARTY_NOTICES). |
| M6.2 | PC speaker synth. |
| M6.3 | AdLib SFX player. |
| M6.4 | IMF music sequencer. |
| M6.5 | Digitized SFX with resampling and positional stereo. |
| M6.6 | AUDIOMUS streaming player. |
| M6.7 | CD music replacement (user files via `symphonia`), with fallback order. |
| M6.8 | Deterministic sound state (priority and "is playing" answers) inside the sim-visible world; tic-stamped command queue to the audio thread. |
| M6.9 | Mixer with Master, FX and Music; sound-mode selection (PC, AdLib, SB digitized, Modern). |
| M6.10 | `obc-tool audio render` offline renders to WAV for comparison with DOSBox-X recordings. |

### Acceptance criteria

- [ ] Every sound chunk and music track renders offline, and the owner signs off on a listening review against the original (side by side).
- [ ] Sound-event traces (id, tic, position) match the original for the M5 sessions plus one combat session.
- [ ] No audio glitches (underruns) at 48 kHz under normal load on Windows and Linux.

---

## M7 — Weapons, combat and HUD

**Goal:** the complete weapon set, projectiles, damage model and status bar.

**Prerequisites:** M5, M6 (for sound events), M4.5.

### RE tasks

| Id | Task |
|---|---|
| M7.R1 | Weapon list and slots (melee?), switching keys, ammo types and caps (9mm, 7mm, fuel, grenades), attack state tables, fire rates. Q-021. |
| M7.R2 | Hitscan: `GunAttack` deltas (range, damage formula, RNG use). |
| M7.R3 | Projectiles: grenades (throw arc or straight, bounce, fuse), grenade launcher, flame thrower (flame actors, burning, "Fire burns"), splash damage and self-damage ("Don't get too close to your own Grenade launch"). |
| M7.R4 | Player damage: `TakeDamage` deltas, flak jacket armor model, difficulty scaling, booby traps. Q-033. |
| M7.R5 | Status bar layout, numbers, weapon icons, face or portrait (if any), ally indicator; pickup messages (`"%d 9mm Clips"`, `"Galil"`, ...). Q-023. |

### Implementation tasks

| Id | Task |
|---|---|
| M7.1 | Weapon state machines, ammo, switching. |
| M7.2 | Hitscan. |
| M7.3 | Projectile and fire actors. |
| M7.4 | Damage, armor, death of the player (death sequence hook for M10). |
| M7.5 | HUD composition and in-game messages. |
| M7.6 | Weapon and ammo pickups. |

### Acceptance criteria

- [ ] Combat sessions (each weapon fired at walls and dummy targets; grenade and flame splash; player taking damage with and without flak jacket) replay with zero divergence.
- [ ] HUD frames pixel-exact against captures in at least 10 states (each weapon, low ammo, armor, damage flash).

---

## M8 — Enemies and AI

**Goal:** every enemy class behaves exactly like the original.

**Prerequisites:** M7.

### RE tasks

| Id | Task |
|---|---|
| M8.R1 | Enemy roster (terrorist types, rats, swamp creatures, boss Victor, any others): spawn codes per difficulty, hit points, speeds, damage, drops, sounds. Q-022. |
| M8.R2 | AI deltas: `T_Stand`, `T_Path`, `T_Chase`, `T_DogChase`-style melee, `T_Shoot`, `T_Bite`, `SightPlayer`, `CheckSight`, `CheckLine`, `FirstSighting`, `SelectDodgeDir`, `SelectChaseDir`, `SelectRunDir`, `MoveObj`, `KillActor`, `DamageActor`. |
| M8.R3 | Boss behavior (Victor) and the victory trigger. |
| M8.R4 | Random game type rules (if they affect spawns). Q-012. |

### Implementation tasks

| Id | Task |
|---|---|
| M8.1 | Actor framework complete (states, think and action dispatch, rotation). |
| M8.2 | One module per enemy class with transcribed state tables. |
| M8.3 | Sight, hearing, path, chase and attack logic. |
| M8.4 | Drops and death. |
| M8.5 | Boss and victory. |

### Acceptance criteria

- [ ] For every enemy class, at least two original sessions (ambush and patrol) replay with zero divergence for at least 2,000 frames.
- [ ] Full-level sessions for three representative floors (one per building) replay with zero divergence for at least 5,000 frames.

---

## M9 — Allies

**Goal:** the ally system, OBC's signature feature.

**Prerequisites:** M8.

### RE tasks

| Id | Task |
|---|---|
| M9.R1 | Ally data model (roster, identities and sprites, where allies spawn, per-ally attributes), control keys (`A`, `S`, mode toggles), messages (`FOLLOW PLAYER`, `DISPERSE AND ATTACK`, `SWITCH`, `STAYPUT & SWITCH`, `NO ALLIES!`). Q-017. |
| M9.R2 | Follow mode: `CT_Path` pathing ("CT_Path hit a wall at %u,%u, dir %u"), stuck and stall throttles. |
| M9.R3 | Disperse and attack mode: targeting, firing, friendly fire rules. |
| M9.R4 | Switching: what transfers (health inherited; guns and ammo kept), what happens to the previous body, camera and HUD changes; switch and stay; coming out of stay on next follow toggle. |
| M9.R5 | Ally death, game-over conditions involving allies. |

### Implementation tasks

| Id | Task |
|---|---|
| M9.1 | Ally actors and state tables. |
| M9.2 | Modes and commands. |
| M9.3 | Switching and stay. |
| M9.4 | HUD and messages for allies. |

### Acceptance criteria

- [ ] Ally sessions (follow through doors, disperse fights, switch, switch and stay, ally death) replay with zero divergence for at least 3,000 frames each.
- [ ] Manual check by the owner that ally behavior "feels" identical in a 10-minute side-by-side session.

---

## M10 — Game flow and front end

**Goal:** the complete game from launch to credits, every screen and menu.

**Prerequisites:** M3 for menus; M5 and M7 for in-game flows; M6 for sound in menus.

### RE tasks

| Id | Task |
|---|---|
| M10.R1 | Control panel structure: every menu, item, cursor behavior, sounds, layouts, text. |
| M10.R2 | New game: game-type selection (Difficulty 1–3, Random, Network, Network Teams, Modem) and what each sets. |
| M10.R3 | Intro, title and outro flows; `nopics`, `NOWAIT`. |
| M10.R4 | Floor change flow: autosave, rendered sequence, "Get Ready!", level stats (if any). Death ("expire") flow and sequences. Victory and ending. |
| M10.R5 | Credits: how CRED1–CRED10 maps are used. Q-010. |
| M10.R6 | High scores: eligibility, entry UI, table layout. |
| M10.R7 | Help screen (`H` at main menu). |
| M10.R8 | Cheats and debug: activation sequences and every effect; diagnostics screen. Q-026. |
| M10.R9 | Rendered sequences: file names, container, codec, timing. Q-015 (needs the CD or Steam data). |
| M10.R10 | Command-line parameter semantics. Q-028. |
| M10.R11 | Map ThermoSensor behavior (automap or radar?). Q-027. |

### Implementation tasks

| Id | Task |
|---|---|
| M10.1 | Menu framework reproducing the control-panel look and behavior. |
| M10.2 | All menus and dialogs (including Y/N prompts such as `ABORT THIS GAME Y/N`, `END GAME Y/N`). |
| M10.3 | Game-type flows and difficulty. Random mode. |
| M10.4 | Intro, title, floor change, death, victory, ending, credits sequences. |
| M10.5 | Rendered-sequence player (optional data). |
| M10.6 | High scores. |
| M10.7 | Help screen. |
| M10.8 | Cheats and debug keys; diagnostics overlay. |
| M10.9 | ThermoSensor or map feature. |
| M10.10 | Command-line flags equivalent to the original's parameters. |

### Acceptance criteria

- [ ] Every screen from the M0.14 baseline (and any captured later) matches pixel for pixel in Classic.
- [ ] The full single-player game is completable from New Game to credits on each difficulty.
- [ ] Every cheat and debug feature listed in DELTA_SPEC works as documented.

---

## M11 — Persistence

**Goal:** saving, loading, autosave, quicksave, config, high scores.

**Prerequisites:** M10 (save and load UI), M9 (full state).

| Id | Task |
|---|---|
| M11.R1 | Original save semantics: slot count, names, what AutoSave stores and when, F9 rules, error messages. Q-013. |
| M11.R2 | (Stretch) original `.SAV` layout for import. |
| M11.1 | Versioned save format with migration hooks. |
| M11.2 | Slots, AutoSave, QuickSave behavior per original. |
| M11.3 | Config and bindings persistence; first-run `CONFIG.DAT` import. |
| M11.4 | High-score file. |
| M11.5 | (Stretch) original save import. |

### Acceptance criteria

- [ ] Save at any tic and load: the subsequent 2,000-tic trace equals the trace of the unsaved run (automated test on several levels and states, including mid-door, mid-push-wall, projectiles in flight, allies in stay mode).
- [ ] AutoSave and F9 QuickSave behave as the manual describes, including the "no current save" case.
- [ ] Saves from an older OBCR version load or fail with a clear message (version policy tested).

---

## M12 — Single-player parity certification (1.0-SP)

**Goal:** prove the single-player game matches the original.

**Prerequisites:** M4–M11.

| Id | Task |
|---|---|
| M12.1 | Capture a parity corpus: at least one long session (over 10,000 frames) per single-player floor, across difficulties and game types, using `obc-oracle`. |
| M12.2 | Run the corpus through `trace-diff`; triage every divergence into a fix, a `K-###` quirk, or a schema problem. |
| M12.3 | Close every single-player open question in OPEN_QUESTIONS.md (or justify deferral). |
| M12.4 | Owner playthrough: complete the game in Classic; log every perceived difference. |
| M12.5 | Performance check against DESIGN §10. |
| M12.6 | Tag `v1.0.0-sp`. |

### Acceptance criteria

- [ ] Zero unexplained divergences across the corpus.
- [ ] Every remaining difference is documented as a quirk fix or a deliberate delta.
- [ ] Owner sign-off.

---

## M13 — Multiplayer

**Goal:** Network, Network Teams and Modem modes, modernized.

**Prerequisites:** M12 (deterministic, verified simulation).

### RE tasks

| Id | Task |
|---|---|
| M13.R1 | Run two DOSBox-X instances with IPX emulation; observe and trace the original's net game: player limit, spawns, respawn, kill scoring, team rules, character choice, chat or messages, end conditions, net-level selection. Q-019. |
| M13.R2 | Original synchronization model (lockstep or otherwise), packet contents (useful for understanding rules, not for compatibility). |
| M13.R3 | Modem and serial mode differences. |

### Implementation tasks

Revised after the RE (ADR-017: the original broadcasts state; it is not lockstep).

| Id | Task |
|---|---|
| M13.1 | `obc-sim::net`: segment 358f's rules (packets, puppets, remote fire and Use, damage filter, deaths and the KILLS table, respawn, dropped weapons, level flow), with in-process multi-world tests. |
| M13.2 | `obc-net`: the UDP transport (versioned header with the data-release id, LAN broadcast discovery, join by address, peer-list gossip, three-packet redundancy with sequence numbers, ids that avoid collisions, keep-alive, timeout and leave). |
| M13.3 | UI: game types N, T and M in GAME TYPE; the network setup screen (player id, address, port, CHOOSE A PLAYER / CHOOSE A TEAM, join address); the M direct-connection screen; the KILLS table; Save and Load disabled; opening the menu counts as a death. |
| M13.4 | Game screen: transport polling, respawn after the death turn, the network level flow (no cutscenes, KILLS, next NETLEVEL), messages. |
| M13.5 | Headless loopback tests: several `obcr` processes on one machine with scripted input; soak with packet loss. |

---|---|
| M13.1 | Lockstep core: per-tic input exchange, input delay, stall handling. |
| M13.2 | UDP transport: sequencing, redundancy, keepalive, timeout, versioned handshake (release id, options). |
| M13.3 | Lobby: host, join by address, LAN discovery, player and team selection screens matching the original. |
| M13.4 | Multiplayer rules in `obc-sim::net_rules`. |
| M13.5 | Desync detection with state hashes and trace dumps. |
| M13.6 | Headless loopback tests: N simulated peers over loopback for 100,000 tics with random inputs and no desync. |

### Acceptance criteria

- [ ] 2-player and 12-player sessions over LAN and over the Internet (direct IP) run 30 minutes with no failure.
- [ ] Rules match the original's code (re/NETWORK.md); a DOSBox-X two-machine trace is still advised.
- [ ] With 5 % packet loss and 150 ms latency, kills, deaths and exits still register on every machine.

---

## M14 — Enhanced mode

**Goal:** modern comforts, never touching Classic.

**Prerequisites:** M12 (most items); renderer items may start after M4.

| Id | Task |
|---|---|
| M14.1 | High-res renderer (scale factor or arbitrary resolution), with snapshot tests. |
| M14.2 | Widescreen Hor+ FOV; HUD layout options. |
| M14.3 | Render interpolation; uncapped frame rate. |
| M14.4 | Unquantized mouse turning, sensitivity, invert, always-run, modern WASD profile. |
| M14.5 | Rebinding UI; gamepad support with rumble (optional). |
| M14.6 | Post filters: sharp bilinear, CRT shader (optional). |
| M14.7 | Quality of life: unlimited saves, quick-load key, autosave slots, crosshair option, message log. |
| M14.8 | Accessibility: subtitles or captions for key sounds (optional), high-contrast HUD option, remappable everything, reduced flashing option. |
| M14.9 | Opt-in quirk fixes (`K-###` flags) exposed in an options menu. |
| M14.10 | Smooth floors and ceilings: perspective-correct, gapless, smooth distance shading (ADR-019). |
| M14.11 | xBRZ 2×–6× texture upscaling for the 3D view, optional for the 2D screens (ADR-020). |

### Acceptance criteria

- [ ] All Classic parity tests still pass with every Enhanced option toggled off.
- [ ] Every Enhanced option is independent and persisted.
- [ ] 4K at 144 fps on the reference machine.

---

## M15 — Release engineering

**Goal:** ship.

**Prerequisites:** M12 (SP release), M13 and M14 for full 1.0.

| Id | Task |
|---|---|
| M15.1 | Packaging: Windows zip and installer, Linux AppImage (and optionally Flatpak), macOS app bundle (signing and notarization if the owner has accounts). Done for Windows and Linux as a per-user Qt 6 installer (ADR-021, `installer/`); macOS and signing remain. |
| M15.2 | First-run data setup: locate data (Steam detection), validate, explain what is missing; never redistribute data. |
| M15.3 | THIRD_PARTY_NOTICES, LICENSE (per ADR-006), README and user manual. |
| M15.4 | Crash reporting (local log plus instructions; no telemetry without opt-in). |
| M15.5 | Release checklist and versioning (SemVer); changelog. |

### Acceptance criteria

- [ ] Clean-machine install tests on Windows, Linux and macOS with each supported data release.
- [ ] License and notice files complete; `cargo deny check` passes.
- [ ] Owner sign-off on the release candidate.
