# GitHub issues #5–#15 — the remediation plan

**Milestone M19.** Status: planned 2026-11-19, being worked on `m19/*` branches.

Eleven issues were reported against 1.0.0-beta.4, most by a player comparing OBCR with the DOS original side by side. This document is the plan for all of them: what each report means, whether it is a defect of OBCR (it differs from the original) or of the original (OBCR copies a DOS bug faithfully), how it is investigated, how it is fixed and how the fix is proved.

The rule that decides every fix is DESIGN §4.3 and ADR-008:

- **OBCR differs from the original**: a bug in OBCR. Fixed outright, with evidence from `COUNT.EXE` (and DOSBox-X where the code is not enough).
- **The original has the bug too**: preserved in Classic and recorded as a quirk `K-###` in [re/DELTA_SPEC.md](re/DELTA_SPEC.md). A gameplay bug gets an **opt-in fix** in OPTIONS > GAMEPLAY FIXES. It is off by default, saved with the game, and part of the network identity. A cosmetic bug is fixed in Enhanced.
- **A request for something new**: an option, off by default when it changes what the original shows.

---

## 0. Groundwork (done first, on `m19/plan`)

- `obc_sim::fixes`: `Fix` (quirk number, label, one-line description) and `Fixes`, a bit per quirk number. `fixes::ALL` lists them in menu order; code asks `w.fixes.on(&fixes::K_0NN)`.
- `World::fixes`, set from the settings when a new game starts (`settings.game.fixes`), kept by its saves.
- **Save format 5**: the extra block gains the fixes. Format 4 and 3 saves still load, without fixes.
- The network identity includes the fixes, so machines with different fixes never connect.
- OPTIONS > **GAMEPLAY FIXES**: lists `fixes::ALL`, toggles each, applies to new games.
- Quirk numbers are reserved per workstream so parallel branches do not collide (renumbered densely when merged if any are unused).

## 1. The issues

| # | Report | First reading | Workstream |
|---|---|---|---|
| 5 | OBCR needs Windows 8+ APIs (`GetSystemTimePreciseAsFileTime`); the reporter runs Windows 7 | Not a defect. The Rust standard library, Qt 6 (Setup) and PySide6 (OBCEdit) all require Windows 10; the owner has said older Windows is not supported | W0 (docs) |
| 6 | xBRZ options seem to change nothing; request: Enhanced without texture filtering | To investigate: whether xBRZ is applied, whether SMOOTH FILTER takes effect in Enhanced | W6 |
| 7 | Items are picked up only when moving forward (DOS: forward and sideways) | OBCR bug for strafing; backward pickup failing is DOS's own behaviour (a quirk to record; candidate opt-in fix) | W2 |
| 8 | Dead enemies should look different on the heat sensor map | Feature request: an option, off by default | W6 |
| 9 | Sewer creatures' sounds play at full volume over the whole map once aware; cured by save and load | OBCR bug: a positioned sound playing unpositioned | W3 |
| 10 | Floor 4's sewer man does not wake when the rats near his room are killed (DOS: he does) | OBCR bug in noise propagation or area connection | W4 |
| 11 | After loading, the floor is dark; a light switch must be cycled twice | OBCR bug unless DOS does the same: the lights' state is not saved or not restored | W1 |
| 12a | The heat sensor (thermal sensor) is lost when a save is loaded | Reported as a DOS bug too: verify in `COUNT.EXE`; quirk with an opt-in fix if so | W1 |
| 12b | Some open doors cannot be shot through, and the enemy behind does not react until the player is in the doorway (retail, not shareware) | Reported as a DOS bug: verify; quirk with an opt-in fix if so | W4 |
| 13 | The game runs faster than DOS (movement, weapon bob) | OBCR bug suspected in the tic rate or tic accounting | W5 |
| 14 | Every weapon picked up is equipped, not only new ones | OBCR bug unless DOS does the same | W2 |
| 15 | Elevator doors (normal and presidential) play the wrong sound | OBCR bug in the door-sound mapping | W3 |

## 2. Workstreams

Each workstream is one branch `m19/<name>`, worked in its own git worktree. Every finding is written down with its confidence and a citation (AGENT_GUIDE §4): the `COUNT.EXE` function (`.local/ghidra/export`), the Wolf3D source it came from, and where needed a DOSBox-X observation (the game is run from a copy in `.local/dosbox/`, never from `..\OBC`). Each fix comes with a test that fails before it and passes after it, `cargo xtask ci` and, where data is involved, `cargo xtask test-data`. The reporter's saves are in `.local/issues/` (never committed).

### W1 — saved state (#11, #12a) · quirks K-001–K-004

1. Load `.local/issues/i12/svgm8.obcr` and a save made on a lit floor; list what the world and the `Game` screen hold about lighting: `w.dark`, `Game::lighting`, the generator state, the palette choice. Find which of them `SaveTheGame`/`LoadTheGame` (`COUNT.EXE`) write and read. That tells whether DOS restores the lights.
2. The heat sensor: find the flag the pickup sets and whether the original's save writes it (`SaveTheGame` field list).
3. Fix:
   - Whatever OBCR fails to save or restore that the original does is an OBCR bug: save it in the world or the extra block (format 5), restore it on load, and make the switch's next use toggle from the real state.
   - Whatever the original itself loses is a quirk with an opt-in fix (for example *K-001 KEEP THE HEAT SENSOR ON LOAD*).
4. Tests: a save and load of a lit floor stays lit; a dark floor stays dark; one switch use toggles; the sensor survives a load with the fix on, and behaves as DOS with it off.

### W2 — the player (#7, #14) · quirks K-005–K-008

1. Pickups: find where the original picks up statics (OBC's `Thrust`/`ClipMove`/`GetBonus` path, with its delta against Wolf3D) and how the movement direction enters it. Compare with OBCR's port.
   - A pickup that fails on strafing is an OBCR bug. Fix it.
   - One that fails moving backward in DOS too is a quirk. Offer an opt-in fix if it is a small, contained change.
2. Weapons: find the original's `GiveWeapon` (or OBC's pickup of a dropped weapon). Does it switch `chosenweapon` only when the weapon is new?
3. Tests: strafing onto an item picks it up; walking backward matches the original (and the fix when on); picking up a weapon already owned adds ammunition and does not switch.

### W3 — sound (#9, #15) · quirks K-009–K-012

1. Positioned sounds: trace how actor sounds reach `SoundManager::play` (`SoundAt` vs `Sound`), and why a save and load cures it. A sound that loses its position or listener after some event is the likely cause: a voice whose `at` is dropped, a sound started without coordinates, an actor sound emitted from a state that should be `PlaySoundLocActor`.
2. Elevator doors: the door kinds' open and close sounds against the original's `OpenDoor`/`CloseDoor` (and OBC's elevator door types), and the sound ids in `doors.rs`.
3. Tests: an aware sewer creature's sounds are positioned (attenuated or panned at distance) before and after a load; each door kind plays the original's sound ids.

### W4 — doors and enemy alertness (#10, #12b) · quirks K-013–K-016

1. Floor 4's sewer man: with the reporter's saves (`.local/issues/i10-port`, and the DOS save `i10-dos/SVGM4.SAV` in DOSBox-X), follow the noise of killing the rats: `madenoise`, `areabyplayer`, `ConnectAreas` through doors, and the sewer man's state and `FL_AMBUSH`. Compare each step with `COUNT.EXE`.
2. Open doors that block shots: the original's hit-scan (`GunAttack`) and sight (`CheckLine`) treat doors by their position; the retail build reportedly differs from the shareware.
   - Confirm in `COUNT.EXE`: OBCR may port exactly that bug, or have a bug of its own.
   - If the original blocks, it is a quirk with an opt-in fix covering both shooting and sight. Otherwise fix OBCR.
3. Tests: the reporter's floor-4 situation wakes the sewer man; a shot through the save's open door behaves as DOS (and as fixed with the fix on).

### W5 — game speed (#13) · quirks K-017–K-019

1. Measure: in the same floor and inputs, player speed in map units per second, weapon-bob period and actor speed in OBCR against DOSBox-X (cycles set as the project's DOSBox setup specifies).
2. Compare OBCR's frame loop with the original's: `TimeCount` rate, how `tics` is computed and clamped (`CalcTics`), what the simulation does per tic, and whether OBCR runs extra tics, a doubled tic count, or a 70 Hz clock where OBC uses another.
3. Fix the timing to match the original. Record the measured before and after.
4. Tests: a fixed number of seconds of input moves the player the original's distance; the bob period matches.

### W6 — presentation (#6, #8) · no quirks

1. xBRZ: verify in a running game (Enhanced, scale 4, TEXTURES XBRZ 4X) that walls and sprites are upscaled, and that XBRZ MENUS and XBRZ STATUS BAR change the 2D layer. Fix what is not applied, and make the effect visible in the option's description. A likely suspect: the setting reaches the renderer only on some path.
2. Sharp pixels in Enhanced: check that SMOOTH FILTER OFF gives nearest-neighbour scaling for every layer (3D view, HUD, menus). Fix any path that always filters.
3. Heat sensor: an Enhanced option *SENSOR: DEAD ENEMIES* (`SHOWN` like the original / `DIMMED` / `HIDDEN`), drawing only; default as the original.
4. Tests: rendering with xBRZ differs from rendering without it; the dead-enemy option changes the map's pixels only for dead actors.

### W0 — documentation (#5)

- README and installer README: supported systems (Windows 10 and 11, 64-bit; Linux with glibc 2.35 or newer), and why older Windows cannot be supported: the Rust toolchain's minimum (Windows 10), Qt 6 for Setup and PySide6 for OBCEdit.

## 3. Order of work and closing

1. Groundwork (§0), committed on `m19/plan`; the workstreams branch from it.
2. W1–W6 in parallel; W0 alongside.
3. Merge into `m19/issues`; renumber the quirks if needed; update DELTA_SPEC's quirk list, CHANGELOG, options documentation, STATUS.
4. Full verification: `cargo xtask ci`, `cargo xtask test-data`, the OBCEdit suites.
5. With the owner's go-ahead: merge and push, a release, and a note on each issue saying what was found and changed. Where the original has the bug too, the note names the GAMEPLAY FIXES option.
