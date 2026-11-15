# Mods, custom maps and OBCEdit — the plan

**Milestone M17.** Status: planned 2026-11-15, being implemented on `m17/*` branches.

The owner asked for two things that only make sense together:

1. **Mods and custom maps in OBCR**: a player can install a mod and play its floors, as a campaign of their own, without touching the original files.
2. **OBCEdit**, a level editor that ships with OBCR and makes those mods. It is ported from **EC7Edit**, the owner's Corridor 7 editor in `..\EC7Wolf\editor`: the parts that are about editing Wolfenstein-family maps are kept, the parts that are about Corridor 7 or ECWolf are taken out, and Operation Body Count's own features are added.

This document is the plan for both: what a mod is, what the engine must change, how EC7Edit becomes OBCEdit module by module, how it ships, how it is tested, and the order of work. It is written to be executed by agents under [CLAUDE.md](../CLAUDE.md) and [AGENT_GUIDE.md](AGENT_GUIDE.md). Decisions that outlive the milestone go to [DECISIONS.md](DECISIONS.md) as ADR-023 (the mod format) and ADR-024 (OBCEdit).

---

## 1. Goals and non-goals

**Goals (this milestone)**

- G1. A **mod format** OBCR loads: one or more custom floors, played as a campaign in order, with per-floor behaviour (music, darkness, sequences, colours) chosen by the author. Floors from the original can be mixed in by reference, never copied.
- G2. **The engine plays a mod** exactly as it plays the original: same simulation, same rules, only the maps and their per-floor settings differ. With no mod active, nothing changes, bit for bit (soak and save tests prove it).
- G3. **Mods are safe to receive**: a mod is untrusted input. A malformed or malicious mod produces a clear error in the MODS screen, never a crash, never a write outside OBCR's own folders, never code execution.
- G4. **OBCEdit**: open the original maps read-only, start new 64×64 maps, paint walls, floors and objects from a palette of real thumbnails, place OBC's compound features with one click (doors, door-look walls, push walls, exits, generator switches, glass, fans, ambushes, allies, patrol routes), validate continuously, playtest in OBCR with one key, take an exact snapshot through OBCR's renderer, and export a mod.
- G5. **It ships with OBCR**: the Windows and Linux installers install OBCEdit beside the game (optional, on by default), with a menu entry; GitHub Actions builds it.
- G6. **No original data in the repository or in a mod**, ever. OBCEdit reads the user's files and never writes them; a mod may contain only maps the author drew (a map imported from the original game stays private to its author's machine).

**Non-goals (later milestones, recorded so they are not forgotten)**

- New art, sounds or music in mods (custom walls, sprites, IMF songs). The mod format reserves room for them (`format` field, §3.2) but v1 accepts maps and level settings only. EC7Edit's E13/E14 (custom resources, cinematics) are not ported.
- Network floors in mods. Network games always use the original's ten arenas in v1.
- New actor behaviour, new map codes, scripting. A mod can only arrange what the game already has (DESIGN's "no scripting language or plugin system" rule stands).
- A 3D editing view. EC7Edit's decision (E10) holds: a second renderer would be a second authority; OBCEdit shows OBCR's own frames instead.

---

## 2. What a mod is

### 2.1 For the player

A mod is a **campaign**: an ordered list of floors. Starting a mod starts its first floor; taking an exit goes to the next; exiting the last floor (or killing Victor, as always) wins the campaign, with the victory screen and the high-score table. Everything else is the original game: weapons, enemies, allies, items, the kill quota, the status bar, saving.

Installed mods appear in a **MODS** screen in the main menu. Choosing one makes it the active game until another is chosen or **ORIGINAL GAME** is picked; the main menu shows which is active. Each mod keeps its own high-score table, and saved games remember the mod they belong to.

### 2.2 Where mods live

| Place | What |
|---|---|
| `<user data>/mods/` | Installed mods: a folder, or a `.obcmod` file (Windows `%APPDATA%\OBCR\data\mods\`, Linux `~/.local/share/obcr/mods/`) |
| `obcr --mod PATH` | Play a mod from anywhere for this run (OBCEdit's playtest uses this) |
| OBCEdit *Export → Install in OBCR* | Writes into the mods folder |

### 2.3 Release requirements

Maps refer to wall pages, floor sets, sprites and actor codes. These are the same in both releases except the CD's twelve removed sprites (the face and sparkles, not placeable) and one static whose blocking flag differs. A mod therefore plays on **either release** unless its manifest says otherwise; OBCEdit warns when a map uses something whose behaviour differs between releases (static 71).

---

## 3. The mod package

### 3.1 Layout

A mod is a folder, or the same folder as a ZIP archive with the extension `.obcmod` (stored or deflated entries only):

```
mod.toml          the manifest (required)
MAPHEAD.BC        the mod's maps, in Operation Body Count's own format
MAPTEMP.BC        (both required unless every level is a stock reference)
README.txt        optional, shown in the MODS screen's details
```

Nothing else is read. Unknown files are ignored (and reported by `--check-mod`), so a later format can add folders without breaking older OBCRs that refuse them by version instead (§3.2).

**Why the original's map format** and not a new one: the engine already parses it, OBCEdit's codec already writes it byte-identically, and a modder's maps open in any Wolf3D-family tool that understands RLEW `MAPHEAD`/`MAPTEMP` pairs. Maps are always 64×64 with three planes (the original `Quit`s on anything else; OBCR refuses it with a message).

### 3.2 The manifest

```toml
format = 1                         # the mod format version; OBCR refuses a higher one

[mod]
id = "lobby-assault"               # 1-32 chars: a-z, 0-9, '-'; unique; names the save and score files
title = "Lobby Assault"            # up to 32 printable ASCII characters (the game's fonts)
author = "Jane Doe"
version = "1.0"
description = "Three floors of the UN lobby, rebuilt."   # up to 400 characters
release = "any"                    # "any" (default), "OBC-A" or "OBC-CD"
obcr = "1.0.0-beta.3"              # optional: the oldest OBCR that plays it

[[level]]                          # one per floor, in order; the first is floor 1
map = 0                            # slot in this mod's MAPTEMP.BC ...
name = "Lobby"                     # optional, up to 16 chars (map header name if omitted)
like = 6                           # optional: the original floor (slot 0-39) whose behaviour it takes; default 6

[[level]]
stock = 7                          # ... or an original floor, by slot (0-39), with its own behaviour

[[level]]
map = 1
like = 2                           # a sewer: dark, sewer textures, ladder exit sequence
music = 4                          # optional overrides, below
```

**Level fields**

| Field | Values | Default | What it does |
|---|---|---|---|
| `map` | 0..99 | — | The map, by slot in the mod's archive. Exactly one of `map` and `stock`. |
| `stock` | 0..39 | — | An original floor, played with its original behaviour. Reads the user's own files; nothing is copied into the mod. |
| `name` | text | map name | Shown in the MODS screen's floor list and in OBCEdit. |
| `like` | 0..39 | 6 | The **behaviour profile** (§4.1) of that original floor. |
| `music` | 0..9, `"random"` | from `like` | AdLib song (`AUDIOT` chunk 300 + n), or the original's random pick. |
| `dark` | bool | from `like` | Starts dark, as the sewers do. |
| `sewer` | bool | from `like` | Sewer floor and ceiling textures and the sewer render tint. |
| `colors` | 0..50 | from `like` | Which original floor's roof/floor colour and shading ramps to use (the per-level tables). |
| `sequence` | `"auto"`, `"F2"`, `"F3"`, `"none"` | `"auto"` | The sequence after this floor: auto = ladder (F2) for sewer-like floors, else stairs (F3) or elevator (F4) by the exit used. |
| `after` | `"F5"`, `"none"` | from `like` | An extra movie after this floor (the CD's F5 plays after floor 23). |
| `message` | text | from `like` | A message when the floor starts (the original greets floor 24 with `UN BUILDING 2`). |
| `exit` | `"normal"`, `"none"` | from `like` | `none` = the exits refuse to work (the original's floor 40): only killing Victor ends it. |

Unknown keys are an error (`deny_unknown_fields`), so a typo is reported instead of silently ignored. Limits: at most 100 levels, map slots that exist, every string printable ASCII within its length.

### 3.3 Validation when loading (`obcr --check-mod`)

The engine validates a mod completely before offering it, and the same code prints a report for OBCEdit:

- the manifest parses, `format` ≤ 1, every field in range, at least one level;
- every referenced map exists, decodes (RLEW within bounds, exact plane lengths), is 64×64;
- each map has **exactly one player start**, at most 64 doors, plane-0 codes in 0..161, area numbers < 22, and does not exceed the original's actor and static limits (the engine's own constants);
- ZIP: no entry outside the root (`..`, absolute paths, drive letters), no entry over 16 MB, total under 64 MB, deflate only; anything else is refused before reading.

Errors name the level and cell. A mod that fails is listed in the MODS screen with its first error and cannot be started.

---

## 4. Engine changes (OBCR)

### 4.1 The level profile

Today the original's per-floor behaviour is spread through the code as tests on `mapon` (the map slot). Each one becomes a field of a `LevelProfile`, kept in `World` and saved with it:

| Today | Profile field | Stock value (slot `s`) |
|---|---|---|
| `(0..=4).contains(&mapon)` → dark at start | `dark` | `s <= 4` |
| sewer floor sets (`level_floor_sets`), sewer render tint (`scene.rs` quad) | `sewer` | `s <= 4` |
| `LevelTables::index(mapon)` → Rindex/Findex/Rclr/Fclr | `colors` | `s` (net levels 50–59 → 40–49, as now) |
| `level_song`: table for 0–9, random after | `music: Option<u16>` | `songs[s]` for `s < 10`, else random |
| face apparition only when `mapon >= 6` | `face` | `s >= 6` |
| `mapon == 23` → `UN BUILDING 2` (floppy) | `message` | that text on slot 23 (floppy only) |
| `mapon == 39` → exits refuse (`LAST FLOOR` on the CD) | `last` | `s == 39` |
| exit sequence: `0..=4` → F2, 22 → F5, else F3/F4 | `sequence`, `after` | as now |
| status bar floor number `mapon + 1` | unchanged: the campaign index + 1 | |

`LevelProfile::stock(slot, release)` reproduces today's behaviour exactly; a mod level builds its profile from `like` plus overrides. `World::setup_game_level(map, mapon, profile)` stores it; every former `mapon` test reads the profile. **Acceptance**: with no mod, every existing test (soak on both releases, save/load identity, game flow) passes unchanged.

`gamestate.mapon` keeps meaning "index in the current campaign"; for the original game that is the map slot, as before.

### 4.2 Campaigns

`obc-ui` gains a `Campaign`: a list of `(map source, profile)` with a title and an id. `Campaign::original(assets)` is the 40 stock floors. A mod builds its campaign from the manifest. `Game` asks the campaign for floor `mapon`'s map and profile instead of `assets.maps.map(mapon)`; advancing past the last floor runs the victory path. The active campaign lives in `Assets` (`Arc<Campaign>`), switched by the MODS screen.

### 4.3 Mod discovery and loading (`obc-formats::modpack`, `obc-formats::zip`)

- `zip`: a minimal, bounded ZIP reader (central directory, stored and deflate via `miniz_oxide`, already a dependency of `obcr-setup`): rejects encryption, ZIP64, multi-disk, unsafe names and oversize entries.
- `modpack`: `ModSource` (folder or zip) → `Mod { manifest, maps: MapArchive, readme }`, with the validation of §3.3, and `scan(dir)` for the mods folder. Pure parsing, no game state; the map checks that need engine constants live in `obc-sim` (`level::check_map`).

### 4.4 User interface

- **MODS** screen (main menu hotkey **D**, also listed in the help line): installed mods with title, author, version, floor count and status (playable / error); Enter activates, **ORIGINAL GAME** first in the list. Details pane shows the description and the README's first lines.
- The main menu shows the active mod's title above START GAME.
- High scores: one table per campaign (`[game.mod_scores.<id>]` in `obcr.toml`); the original's table is unchanged.
- Saved games: the header gains `mod_id` and `mod_version` (save `FORMAT` 3 → 4). Loading a mod save with another campaign active switches to that mod if it is installed, otherwise refuses with the mod's name. Older format-3 saves keep loading as original-game saves (a small compatibility reader).

### 4.5 Command line

| Option | Purpose |
|---|---|
| `--mod PATH\|ID` | Activate a mod for this run (a folder, a `.obcmod`, or an installed mod's id) |
| `--check-mod PATH` | Validate a mod and print a report (tab-separated, like `--check-data`); exit 0 / 2 |
| `--play N` | (exists) start on floor N of the active campaign |
| `--start X,Y,DIR` | Developer/editor: override the player start (tile and compass direction) for `--play` |

`--start` exists for OBCEdit's "play from here" and its snapshots: `obcr --mod tmp --play 0 --start 12,30,1 --shot 2 --shot-out snap.png`.

### 4.6 Tests (engine)

- `zip` and `modpack` unit tests on synthetic archives, including malicious names, oversize entries and bad manifests (property tests: random bytes never panic).
- A synthetic test mod (generated in the test, never stored): two tiny floors written with the map writer, played through `Game` from floor 1 to victory.
- Profile equivalence: `LevelProfile::stock` against today's behaviour for all 40 slots on both releases.
- Data-gated: a mod referencing stock floors plays them identically to the original campaign (same RNG, same world after N frames).

---

## 5. OBCEdit

### 5.1 Language, layout, licence

OBCEdit stays **Python 3.10+ with PySide6**, as EC7Edit is: the owner asked to reuse it, and it is a mature, tested, layered program (≈ 25,000 lines with tests). Its rule that the core depends on nothing but the standard library is kept, so the core's tests run anywhere, including CI without Qt or game data.

```
obcedit/
  obcedit                  launcher script (run from a checkout)
  obcedit_core/            Qt-free: codec, catalog, document, commands, project, validation, export, CLI
  obcedit_gui/             PySide6: window, canvas, palette, dialogs, workers
  resources/catalog.json   generated by `cargo run -p obc-tool -- catalog` (§5.4)
  scripts/                 fixtures, catalog verification, packaging
  tests/unit, tests/gui
  pyproject.toml, README.md
```

EC7Edit is GPL-3.0-or-later and the owner's own work, so it can be copied into OBCR (GPL-3.0-or-later) with its copyright lines kept. The version is OBCR's (`1.0.0-beta.N`), for the reason EC7Edit gives: one product, one number.

### 5.2 Module map: keep, adapt, rewrite, drop

| EC7Edit module | OBCEdit | Notes |
|---|---|---|
| `planes`, `names`, `errors`, `paths`, `bundle`, `version` | **keep** (rename) | Coordinate model, raw 16-byte names, typed errors, write guards, frozen-path logic |
| `rlew` | **keep** | RLEW codec and the "run threshold 4" rule; re-verified against OBC's archive (§5.6) |
| `document`, `commands`, `transforms`, `tools` | **adapt** | Generic raw-word document, undo, drawing geometry. Facing rotation tables change to OBC's actor and player codes; map size fixed at 64×64 |
| `project` | **adapt** | JSON project (`.obcproject`), schema renamed and versioned 1; campaign fields per §3.2 |
| `archive` | **rewrite** | Corridor 7 keeps a self-contained TED5 file with the header in the executable; OBC uses a `MAPHEAD.BC` (RLEW tag + 100 offsets) and `MAPTEMP.BC` (38-byte headers, RLEW planes with a length word). Reader, canonical writer, byte-identical round trip of the user's own archive (checked at run time, never stored) |
| `assets`, `imagery` (decoders) | **adapt** | VSWAP (`GFXTILES.BC`) walls and sprites; the palette read from `COUNT.EXE`/`BCCD.EXE` at the release's offset; floor-set pages for floor thumbnails. `imagery`'s PNG writer kept for thumbnails and snapshots |
| `catalog` + `editor_catalog.json` | **rewrite** | Generated from OBCR itself (§5.4), not from ECWolf's translator |
| `rules` | **rewrite** | OBC's door axis rule (even/odd code = vertical/horizontal, decided by the code, not topology), push-wall travel (exactly two cells), door-look-wall and exit rules, from `docs/re/LEVEL_CODES.md` |
| `prefabs` | **rewrite** | OBC's compound tools (§5.5) |
| `validation`, `reachability` | **rewrite rules, keep frame** | The problem model, codes (`OBC-…`), severity and the advisory reachability flood are kept; the rules are OBC's (§5.7) |
| `campaign` | **rewrite** | Builds `mod.toml` instead of MAPINFO; the same bounded-schema idea |
| `wad`, `packfile` | **replace** | `modexport`: writes a mod folder or `.obcmod` (§3), and audits it by reading it back |
| `engine_runner`, `snapshot` | **adapt** | Launch `obcr --mod … --play N [--start …]`; snapshots through `obcr --shot` |
| `discovery` | **adapt** | Find `obcr` and the game data; identify the data by running `obcr --check-data` (the engine stays the authority on releases) |
| `cli` | **adapt** | `inspect`, `validate`, `project-import`, `project-inspect`, `project-export`, `mod-export`, `mod-audit`, `catalog-verify` |
| `xlat`, `decorate`, `custom`, `resources`, `flic`, `video` | **drop** | ECWolf translation and actor scripts, custom resources and cinematics: not in v1 (§1 non-goals) |
| `ec7edit_gui/*` | **adapt** | Window, canvas, palette, inspector, problems, test log, snapshot, first run, import, campaign dialog; resource dialog dropped; Corridor 7 wording and tools replaced |

### 5.3 The OBC map model in the editor

- 64×64, three planes. Plane 0: walls 1–129, doors 130–137, ambush 138, floors 140–161 (area = code − 140, floor/ceiling set = code − 144). Plane 1: player starts 19–22, statics 23–85, animated walls 86–88/103/106/107/124, explosion-proof 89, patrol arrows 90–97, push wall 98, see-through 104/105, actors 108–317 (three difficulty tiers), fire 282–285. Plane 2: unused by the game, preserved as read.
- The palette shows names and thumbnails from the catalog; the raw value is always visible in the inspector (EC7Edit's "raw words are canonical" rule).
- Difficulty tiers: actor codes come in three bands; the palette offers "all difficulties / 2 and 3 / 3 only" for each placement, as the original's codes do.

### 5.4 The catalog, generated from OBCR

`cargo run -p obc-tool -- catalog --out obcedit/resources/catalog.json` writes, from the Rust code that actually spawns things (no hand-copied tables):

- every plane-0 value: kind (wall, door with lock and axis, ambush, floor with area and set, special wall: door-look, glass, generator, exit ladder/stairs/elevator, fan, damaged), wall page for thumbnails, solidity;
- every plane-1 value: kind (player start + facing, static + statinfo kind + sprite, animated wall, marker, patrol arrow + direction, push wall, actor + class + stand/patrol + facing + tier + first sprite, ally, fire), with a display name;
- the floor sets' floor and ceiling pages.

Sprite numbers come from the state tables OBCR reads from the user's executable; they are small integers, not art, and are the same in both releases for everything placeable. `obc-tool catalog --verify` (in `cargo xtask ci` via a test) fails when the committed catalog no longer matches the code. **The catalog contains no pixels, palettes or map data.**

### 5.5 OBC compound tools (prefabs)

| Tool | What it writes |
|---|---|
| **Door** (4 looks) | Plane 0 130–137, axis from the neighbouring walls (vertical when walls are north and south); the lock only changes the look |
| **Door-look wall** (5 styles) | Plane 0 13/29/49/65/69 + plane 1 103 (sliding panel) or 106 (sliding, see-through) |
| **Push wall** | Plane 1 98 on a wall cell; previews the two cells it will travel and warns when they are not open floor |
| **Exit** (ladder / stairs / elevator) | Plane 0 113/114/115 on a wall the player can face |
| **Generator switch** | Plane 0 95 (off) or 96 (on) |
| **Glass pane** | Plane 0 57 or 117 + plane 1 104 (see-through) |
| **Fan** (porthole) | Plane 0 73 + plane 1 86/87/88 (speed) |
| **Ambush enemy** | Plane 0 138 + an actor code (the original clears 138 to the neighbouring floor) |
| **Patrol route** | Plane 1 90–97 arrows, drawn as a path |
| **Squad** | Ally codes beside the player start |
| **Rock opening** (sewer) | Plane 0 88 + plane 1 107 |

### 5.6 Codec fidelity

As EC7Edit did for Corridor 7: re-encoding the user's own `MAPTEMP.BC` must be byte-identical (run threshold, terminator, header layout). This is checked at run time against the user's data (`obcedit verify-archive`) and in a data-gated test; fixtures in the repository are generated from a reserved word band (`0xE000+`), never retail bytes.

### 5.7 Validation (OBC rules)

Errors (block export and playtest): no player start, more than one, more than 64 doors, more than the actor or static limits, plane-0 code ≥ 162, a door with no walls on its sides, a door on the map edge, a push wall not on a wall, unknown plane-1 codes, a floor with an area ≥ 22, a mod level referencing a missing map.

Warnings (advisory): no exit reachable from the start (reachability flood through doors, door-look walls and push walls), fewer enemies than half the quota can be killed (unreachable enemies), a push wall whose two-cell path is blocked, an exit not facing open floor, an ambush code without an actor, a patrol arrow no actor can reach, static 71 (blocks on the CD only), an imported original map in a mod meant for sharing.

### 5.8 Playtest and snapshot

- **F5 Playtest**: exports the project to a temporary mod folder, then runs `obcr --mod <tmp> --play <floor>`; **Shift+F5** plays from the cursor (`--start`). Output and exit status appear in the Test Log.
- **Snapshot**: `obcr … --start x,y,dir --shot 2 --shot-out <png>` at the chosen cell and facing, in Classic or Enhanced; shown in the Snapshot pane. One renderer, one authority.

### 5.9 Packaging and shipping

- PyInstaller **onedir** (EC7Edit's measured decision: same download as onefile, 5× faster start), built by `obcedit/scripts/package.py` into `target/obcedit/`.
- **Installers**: the build scripts copy the OBCEdit folder into Setup's deploy folder; Setup gets an option *Install OBCEdit, the level editor* (on by default), copies it to `<install>/obcedit/`, adds a menu entry (and `~/.local/bin/obcedit` on Linux), and lists every file in the manifest so the uninstaller removes it.
- **CI**: `ci.yml` runs OBCEdit's unit tests (no Qt, no data) and its GUI tests (offscreen Qt) on Linux and Windows; `release.yml` builds OBCEdit with PyInstaller on both runners before the installers.
- **Licences**: PySide6/Qt (LGPL-3.0, dynamically linked), Python (PSF), PyInstaller's bootloader (GPL with the bootloader exception) go to THIRD_PARTY_NOTICES.

---

## 6. Order of work

| Task | What | Acceptance |
|---|---|---|
| M17.1 | This plan; ADR-023 (mod format), ADR-024 (OBCEdit) | Owner-requested, committed |
| M17.2 | `LevelProfile`: every `mapon` behaviour test moved into the profile; save format 4 | All existing tests pass unchanged; profile equivalence test |
| M17.3 | `zip`, `modpack`, map checks, `--check-mod` | Unit and property tests; malicious-archive tests |
| M17.4 | `Campaign`, `--mod`, `--start`, MODS screen, per-mod scores and saves | A generated two-floor mod plays to victory in a test; saves round-trip with the mod |
| M17.5 | `obc-tool catalog` and its verification test | Catalog regenerates identically; covers every code the spawner handles |
| M17.6 | OBCEdit core: copied, renamed, OBC codec, catalog, rules, prefabs, validation, export, runner, CLI | Unit tests (ported and new) pass; byte-identical archive round trip on the user's data (data-gated) |
| M17.7 | OBCEdit GUI | GUI tests on the offscreen platform; a map drawn, validated, exported and played by `obcr` in a test |
| M17.8 | Packaging: PyInstaller, Setup option, build scripts, CI | Installers carry OBCEdit; unattended install creates it and the uninstaller removes it |
| M17.9 | Documentation: `docs/modding.md` (players and authors), the OBCEdit manual, README, options, CHANGELOG | Links checked |

M17.2–M17.5 (Rust) and M17.6–M17.7 (Python) can proceed in parallel once the mod format (§3) and the catalog schema (§5.4) are fixed, which this document does.

---

## 7. Risks

| Risk | Mitigation |
|---|---|
| A `mapon` test is missed and a mod floor behaves like the wrong original floor | Grep-driven inventory (§4.1) plus the rule that `mapon` comparisons outside `LevelProfile::stock` are a review failure |
| Save compatibility | Format 4 with a reader for format 3; tested |
| Untrusted mods | Bounded ZIP reader, no path from a mod ever reaches the filesystem, manifest limits, property tests |
| The editor writes maps the engine rejects | The engine's `--check-mod` is the export gate; OBCEdit runs it before playtest and export |
| Installer size (Python + Qt ≈ 70 MB compressed) | OBCEdit is optional in Setup; download size stated in the release notes |
| PySide6 not available for a platform or Python version in CI | Pin PySide6 and Python 3.12 in CI; the core has no Qt dependency and always tests |
| Porting drift between EC7Edit and OBCEdit | OBCEdit is a fork, not a shared library: each evolves for its own game; this plan records the starting point (EC7Wolf `editor/` at the time of the port) |
