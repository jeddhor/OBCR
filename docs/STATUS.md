# OBCR status

Live project state. Every agent reads this at the start of a session and updates it at the end (AGENT_GUIDE §3). Keep edits small: change your own rows, append to the log.

**Current milestone: M12 first-episode playtest and M13 multiplayer LAN test (owner testing needed)**
**Last updated: 2026-11-14**

## Milestones

| # | Milestone | Status | Notes |
|---|---|---|---|
| M0 | Bootstrap and tooling | Done | Workspace, xtask ci/lint/test-data, Ghidra pipeline |
| M1 | Data formats and asset pipeline | Done | obc-formats: VSWAP, maps, VGAGRAPH, fonts, audio, AUDIOMUS, FLC, palettes, config; obc-tool |
| M2 | Reverse-engineering foundation | In progress | re/ notes for actors, player, allies, renderer, game flow, audio, level codes; no DOSBox-X traces yet |
| M3 | Platform shell | Done | SDL3 window, audio, keyboard/mouse/gamepad, fullscreen, rumble |
| M4 | Classic renderer | Done | OBC pipeline (floors/ceilings, shading, see-through walls, two-part weapon); widescreen Hor+ and k× scale |
| M5 | World simulation core | Done | Level setup, doors, push walls, animated walls, visibility pass, frame loop |
| M6 | Audio | Done | AdLib/PC sounds, 9,009 Hz digitized with OBC panning, IMF music; 1 or 4 voices |
| M7 | Weapons, combat and HUD | Done | Five weapons, pickups, damage, status bar, event box, flashes, splats, shake |
| M8 | Enemies and AI | Done | All classes, projectiles, fire, face apparition |
| M9 | Allies | Done | Follow/disperse, switching, stay; shared actors AI |
| M10 | Game flow and front end | Done | Title, control panel, options, help, quit/credits, sequences F2/F3/F4/F6, death, floor change |
| M11 | Persistence | Done | OBCR saves, AutoSave, QuickSave, high scores, settings |
| M12 | Single-player parity certification | In progress | Needs DOSBox-X traces; owner playtest of maps 0–22 |
| M13 | Multiplayer | Review | RE of 358f/33bb (re/NETWORK.md, re/NETCOMMS.md); rules in obc-sim::net; UDP transport obc-net; setup screen, KILLS table; tested in process and with 2–4 `obcr` processes; needs an owner LAN test |
| M14 | Enhanced mode | In progress | Widescreen, 2–8× scale, smooth motion, modern controls, gamepad, smooth floors, xBRZ textures done |
| M15 | Release engineering | In progress | Per-user Qt 6 installers for Windows and Linux (ADR-021); private GitHub repository jeddhor/OBCR |

Status values: `Not started`, `Ready` (prerequisites met), `In progress`, `Blocked`, `Done`.

## Active tasks

Task status values: `In progress`, `Paused`, `Blocked`, `Review`, `Done`. Remove `Done` rows when the milestone closes (the log keeps the history).

| Task | Title | Status | Owner | Branch | Started | Evidence / notes |
|---|---|---|---|---|---|---|
| — | — | — | — | — | — | — |

## Blockers and owner decisions needed

| Id | Question | Needed by | Status |
|---|---|---|---|
| B-1 | License | M15 | Resolved 2026-11-02: GPL-3.0-or-later (ADR-006) |
| B-2 | Remote for CI? | M0.7 | Resolved 2026-11-02: local builds only; `cargo xtask ci` is the gate. 2026-11-14: the repository is public; GitHub Actions runs `cargo xtask ci` on Ubuntu 22.04 and Windows for every push to main, and builds the installers and publishes the release for each `v*` tag (`.github/workflows`). 2026-11-06: the owner added a private GitHub repository (`jeddhor/OBCR`, remote `origin`) for the code; builds stay local |
| B-3 | Rendered sequences and CD audio data | M10.R9, M6.R2 | Resolved 2026-11-02: the owner's floppy install archives contain `F2.BC`, `F3.BC`, `F4.BC`, `F6.BC` (FLC animations). 2026-11-14: the owner added the Steam BIN/CUE image (`..\CD`): `F1`, `F5`, `F7` and four CD audio tracks (ADR-022). |
| B-4 | Agent commit policy | M0 | Resolved 2026-11-02: confirmed as written in CLAUDE.md |

## Owner goal (2026-11-02)

Work milestone by milestone until the owner must test something, or until **the first episode is playable start to finish**. Must include modern graphics features (widescreen, upscaling) and gamepad support.

## Log

Append one line per completed task or notable event: `YYYY-MM-DD — <task> — <what changed> — <evidence>`.

- 2026-11-02 — setup — Repository created; DESIGN, AGENT_GUIDE, MILESTONES, DECISIONS, STATUS and the re/ knowledge base written from an initial survey of `..\OBC`, `..\wolf3d`, `..\ECWolf`, `..\dosbox-x` — data parsed with throwaway Python scripts (61 maps, 937 VSWAP chunks, 58 VGAGRAPH chunks, 310 audio chunks); `rndtable` byte-match confirmed.
- 2026-11-02 — M1–M11 — Engine, renderer, audio, simulation (player, enemies, allies), front end and persistence implemented; three parallel agents merged (player 6bfe0cb, allies 09344bb, actors 8faa675) — `cargo xtask ci` and `cargo xtask test-data` pass.
- 2026-11-02 — M12 — Soak test plays maps 0–22 at game types 0–3 for 2,500 frames each without panics or faults; every first-episode exit is reachable; save/load continues bit-identically; UI flow tests cover floor change, load and death. Fixed: the player was not objlist[0] (maps with an actor before the start lost the player's think).

- 2026-11-02 — M7/M8/M10 — Healing stations, shootable-wall search and overhead map (agent 6f10c71); grenade tremors fixed (actors had a private DS:0x09D8); enemies operate door-look walls; Tab map; bogus-mode cheats; F1/F2 toggles; victory screen; Enhanced smooth motion — 33 test suites pass.

- 2026-11-05 — audit — Mechanics checked against the Wolf3D/OBC comparison (docs/MECHANICS_AUDIT.md); fixed F5/F6, message and H/C overlays, blackout, F10, tremor Esc rule, clock-seeded RNG, effects device option; weapon 0 renamed assault shotgun — 33 test suites pass.

- 2026-11-05 — fix — Blasts never leave a floor unfinishable (ADR-016): exits 0x72/0x73 and nearby doors are spared; a destroyed door never closes again — test `blasts_spare_exits_and_doors`.
- 2026-11-05 — M13 — Multiplayer: the original's state-broadcast model reverse-engineered and ported (ADR-017); game types N, T, M; LAN discovery, calling an address, gossip, redundancy — `net_game` (8 tests incl. a 4-player soak with 10 % loss), `obc-net` (8 loopback tests); 2- and 4-process `obcr` runs: kills, deaths, respawn, exits and the KILLS table agree on every machine.
- 2026-11-05 — fix — Ally kills count toward the exit quota, still without score (ADR-018) — test `ally_kills_count_toward_the_exit`.
- 2026-11-05 — M14.10/M14.11 — Enhanced smooth floors and ceilings (ADR-019: perspective-correct, gapless, smooth distance shading, rows drawn in parallel) and xBRZ 2×–6× textures (ADR-020: palette-index port of xBRZ 1.6, two-index blend plane in the framebuffer, optional for the HUD and menus); options TEXTURES, FLOORS, XBRZ HUD/MENUS and `--xbrz`, `--floors`, `--xbrz-screens` — 14 new unit tests (xbrz, framebuffer, layer, view3d: no gaps, every pixel drawn, Classic unchanged, same wall geometry); screenshots on maps 5 and 12; 8× 16:9 view: smooth floors 12 ms vs 28 ms original, +3 ms for xBRZ walls.
- 2026-11-06 — M14.11 — xBRZ for the 2D screens split in two: XBRZ MENUS (default on: menus, title and intro pictures, animated sequences, help and score pages) and XBRZ STATUS BAR (default off); 2D xBRZ about 2.5× faster (padded kernel, per-pixel touched blocks), direct layer writes; developer flag `--sequence F2|F3|F4|F6` — screenshots of the intro, menu and F2 with the option on and off.
- 2026-11-06 — repo — m14 merged into main; private GitHub repository `jeddhor/OBCR` created and main plus the task branches pushed (history audited: no game data or decompiled code).
- 2026-11-06 — M15.1 — Per-user Qt 6 installer and uninstaller for Windows and Linux (ADR-021): `installer/` wizard, `obcr --check-data`, `obcr-setup` self-extracting launcher, `cargo xtask pack-installer`, build scripts; `obcr` is a GUI program on Windows — Windows (15 MB) and Linux (27 MB) packages each installed unattended, game ran from the installed folder, update in place and uninstall left nothing behind (no Qt on PATH on Windows; WSL Ubuntu 22.04 for Linux); wizard pages rendered on both.
- 2026-11-14 — M14.5 (issue #2) — Full rebinding: 33 actions, up to three keyboard/mouse inputs and two gamepad buttons each (mouse 4/5, wheel, triggers included), saved by name in obcr.toml; KEY BINDINGS screen in OPTIONS and after the help page; scheme defaults; F5/F6 corrected to the original (F5 floors off, F6 on; CheckKeys 1942:0666/0688) — 8 unit tests, screenshots of both views and a rebind.
- 2026-11-14 — fix (issue #3) — Push walls follow the original `MovePWalls` exactly (full disassembly read): twice Wolf3D's speed, exactly two cells, the player blocks them, and a wall at rest loses its marker and cannot be pushed again. The free-standing blocks in the report come from the original's two-cell rule and map 8's push-wall partitions between open rooms — test `push_walls` (3).
- 2026-11-06 — M15.5 — Version 1.0.0-beta.1: CHANGELOG.md, THIRD_PARTY_NOTICES.md (also in Setup's licence dialog), tag `v1.0.0-beta.1`, GitHub pre-release with both installers.
- 2026-11-14 — M15.6 (issue #1) — The CD release (ADR-022, docs/re/CD_RELEASE.md): release OBC-CD and `BCCD.EXE`'s table layout (state records renamed to OBC-A's), the CD build's code differences behind `GameData::cd` (instruction-skeleton comparison of all functions, then a read of each changed one); BIN/CUE/ISO reader and QOA codec in obc-formats; `obcr --extract-data` rips the music; Setup takes a CD image; F1/F5/F7 play; CD music with the original's track logic and Alt+P/S/F — tests `cd_tables_line_up_with_the_floppy_ones`, `reads_the_cd_image`, `cd_music_follows_the_cd_release`, qoa and disc unit tests; unattended install and uninstall from `BODYCOUNT.cue` checked.
- 2026-11-14 — M15.7 — Version 1.0.0-beta.2 published as a GitHub pre-release, both installers built by GitHub Actions (Windows with Qt 6.10.3: aqtinstall 3.3 cannot read Qt 6.11); the Windows installer from the release installed and removed cleanly from `BODYCOUNT.cue`. Issues #1, #2 and #3 answered and closed.
- 2026-11-14 — docs — README rewritten as a full user manual (the game, getting the data, installing, running, controls, multiplayer, troubleshooting, building, architecture, testing, known deviations, credits); new guides docs/game-guide.md, docs/options.md, docs/cd-release.md, docs/multiplayer.md, docs/ci.md and a manual page docs/obcr.6; docs/re/README.md indexes every RE file. Fixed on the way: an unknown name in `[input.bindings]` no longer resets every setting (test `unknown_names_are_skipped_not_fatal`).

## Known gaps (first episode)

- No DOSBox-X trace comparison yet: behaviour follows the disassembly, unverified against the original at run time.
- Sequences F1, F5, F7 exist only on the CD release; with floppy data they are skipped, as the original does.
- CD release: a flame's direct-hit damage is a stand-in (Q-036).
- Debug keys (`vfr750hondarc45`, Backspace) not ported; the bogus-mode cheats are (`--nsr250`).
- Multiplayer has been tested on one machine (several processes over localhost UDP), not yet across a real LAN or the Internet.
- Healing stations are ported but, as in OBC-A, nothing can start them (PLAYER.md §9).
