# OBC ⟷ Wolf3D delta specification

Every known difference between Operation Body Count and the Wolf3D source baseline (`D-###`), plus original bugs and oddities that OBCR handles deliberately (`K-###`). "No delta" results are recorded too, so nobody re-checks the same thing.

Entry template and rules: AGENT_GUIDE §5.6. Status values: `OPEN` (known, not implemented) → `IMPLEMENTED (<commit>)` → `VERIFIED (<test, trace or capture>)`. Seeded 2026-11-02 from the initial survey; most behavioral entries name a feature whose mechanics are still UNKNOWN and link to the open question that tracks it.

---

## Deltas

### D-001 — Data file set and naming
- **Area**: resources  **Status**: OPEN
- **Wolf3D**: `VSWAP.`, `GAMEMAPS.`, `MAPHEAD.`, `VGAHEAD.`, `VGADICT.`, `VGAGRAPH.`, `AUDIOHED.`, `AUDIOT.` with a per-game extension (`WOLFSRC/ID_CA.C`, `ID_PM.C`).
- **OBC**: extension `.BC`; page file named `GFXTILES.`; maps in `MAPTEMP.`; extra file `AUDIOMUS.`; also writes `BC.LOG` and reads or writes `DEBUG.TXT`.
- **Evidence**: file inventory; strings at file 0x381A0–0x382CC (STRINGS.md).  **Confidence**: CONFIRMED
- **Port impact**: `obc-formats::release` file table; case-insensitive lookup.

### D-002 — Map compression
- **Area**: resources  **Status**: OPEN
- **Wolf3D**: retail data uses Carmack + RLEW (`CARMACIZED`, `WOLFSRC/ID_CA.C`).
- **OBC**: RLEW only (the `MAPTEMP` path); plane data begins with the expanded byte count followed by RLEW words.
- **Evidence**: all 61 maps parse with RLEW only (DATA_FORMATS §3).  **Confidence**: CONFIRMED
- **Port impact**: `obc-formats::maps`.

### D-003 — Level set
- **Area**: game flow  **Status**: OPEN
- **Wolf3D**: 6 episodes × 10 maps.
- **OBC**: 61 map slots: 40 single-player floors (Sewer 1–5, Maintenance, UN Building 1 lobby to top, Building 2 floors 1 to top), 10 credit maps, 10 net levels, 1 empty slot.
- **Evidence**: map headers (DATA_FORMATS §3.3).  **Confidence**: CONFIRMED (contents); progression UNKNOWN (Q-011)
- **Port impact**: level table and progression in `obc-sim::game`.

### D-004 — Map plane semantics
- **Area**: level setup  **Status**: OPEN
- **Wolf3D**: plane 0 walls 1–63, doors 90–101, floor area codes from `AREATILE` (107); plane 1 objects from 19 upward, 0 = empty (`WOLFSRC/WL_GAME.C:221` `ScanInfoPlane`).
- **OBC**: plane-0 floor codes 144–151 (with grid patterns), solid fill values 40 and 91; plane-1 background value 18; wall-cell attributes (89, 103, 104); object codes up to 317.
- **Evidence**: plane statistics (DATA_FORMATS §3.4).  **Confidence**: CONFIRMED that semantics differ; meanings UNKNOWN (Q-001, Q-002)
- **Port impact**: `obc-formats::level_codes` tables from M2.R9.

### D-005 — Page file contents
- **Area**: resources  **Status**: OPEN
- **Wolf3D (WL6)**: 106 wall pages, about 436 sprites, digitized sounds.
- **OBC**: 136 wall pages, 680 sprites, 46 digitized sounds.
- **Evidence**: GFXTILES header (DATA_FORMATS §2).  **Confidence**: CONFIRMED
- **Port impact**: sprite and sound enums sized from data; names from M2.R13.

### D-006 — Graphics chunk set
- **Area**: resources, UI  **Status**: OPEN
- **Wolf3D**: about 150 VGAGRAPH chunks (`GFXV_WL6.H`): many pics, status-bar faces and digits, end screens, demos, text articles.
- **OBC**: 58 chunks: 2 fonts (LIKELY), 53 pics, 2 unknown 5-byte chunks.
- **Evidence**: DATA_FORMATS §4.  **Confidence**: CONFIRMED (counts)
- **Port impact**: pic enum from M1.R5; HUD composition differs (Q-023).

### D-007 — Audio set
- **Area**: audio  **Status**: OPEN
- **Wolf3D**: `NUMSOUNDS = 87` (WL6), IMF music, digitized sounds via VSWAP.
- **OBC**: probably `NUMSOUNDS = 100`, 9–10 IMF tracks, a second digitized-sound archive (`AUDIOMUS.BC`, 99 sounds, Corridor 7 layout), Red Book CD music via MSCDEX (`cdmusic` parameter), Sound Blaster Pro mixer volumes (master, effects, music).
- **Evidence**: AUDIOHED chunk sizes; AUDIOMUS directory parse; strings (STRINGS.md "Menus, sound, mixer", "CD audio").  **Confidence**: LIKELY (counts), CONFIRMED (features exist)
- **Port impact**: `obc-audio` music sources and mixer (DESIGN §7.4); Q-004, Q-006, Q-007, Q-016.

### D-008 — Random number table: no delta
- **Area**: core  **Status**: VERIFIED (byte comparison, 2026-11-02)
- **Wolf3D**: `rndtable[256]` (`WOLFSRC/ID_US_A.ASM:19`).
- **OBC**: identical bytes at file 0x2D8F2.
- **Evidence**: byte comparison of all 256 entries.  **Confidence**: CONFIRMED
- **Port impact**: `obc-core::rng` uses the Wolf3D table. The `US_RndT` routine itself still needs a delta check (index handling).

### D-009 — High-score records
- **Area**: UI, persistence  **Status**: OPEN
- **Wolf3D**: `char name[MaxHighName + 1]` with `MaxHighName = 57`, `long score`, `word completed, episode` (`WOLFSRC/ID_US.H`); 7 entries; default names from id staff.
- **OBC**: `char name[20]`, `i32 score`, `u16 completed`, `u16 episode` (28 bytes); 7 entries; defaults Hank 11200, Oliver 10100, Sam 9800, Ralph 9800, Alf 9600, Eb 9500, Arnold 9250.
- **Evidence**: `CONFIG.DAT` layout; default table at file 0x383E4.  **Confidence**: CONFIRMED
- **Port impact**: high-score model, name entry length limit (19 characters plus NUL).

### D-010 — Control configuration
- **Area**: input  **Status**: OPEN
- **Wolf3D**: `NUMBUTTONS = 8` (attack, strafe, run, use, four weapon slots).
- **OBC**: about 14 `buttonscan` entries in `CONFIG.DAT` (five weapon keys, Enter, S, I, Tab, M, Alt among the defaults).
- **Evidence**: `CONFIG.DAT` bytes (DATA_FORMATS §7).  **Confidence**: LIKELY
- **Port impact**: `TicCmd` button set; bindings (Q-014).

### D-011 — Allies subsystem (new)
- **Area**: simulation  **Status**: OPEN
- **Wolf3D**: none.
- **OBC**: allies with Follow and Disperse/Attack modes; Switch (`A`) and Switch & Stay (`S`); messages `FOLLOW PLAYER`, `DISPERSE AND ATTACK`, `SWITCH`, `STAYPUT & SWITCH`, `NO ALLIES!`; path following (`CT_Path`) with stuck and stall throttles.
- **Evidence**: `README.TXT` "Ally usage"; strings.  **Confidence**: CONFIRMED (existence)
- **Port impact**: `obc-sim::allies` (M9); Q-017.

### D-012 — Weapons and ammunition
- **Area**: simulation  **Status**: OPEN
- **Wolf3D**: knife, pistol, machine gun, chain gun; one ammo type.
- **OBC**: 9mm and 7mm clips, Uzi, Galil, Flame Thrower (Fuel Pack, Fuel Bottle), Grenades, Grenade Launcher; splash damage including self-damage; fire.
- **Evidence**: strings (STRINGS.md "Items and pickups"); manual tips.  **Confidence**: CONFIRMED (names); mechanics UNKNOWN (Q-021)
- **Port impact**: `obc-sim::weapons` (M7).

### D-013 — Items
- **Area**: simulation  **Status**: OPEN
- **Wolf3D**: food, first aid, treasure, extra lives, keys, ammo.
- **OBC**: Flak Jacket, Increased Health, Medic Pack, Map ThermoSensor, Booby Trap, plus weapon and ammo pickups. Treasure and lives UNKNOWN.
- **Evidence**: strings.  **Confidence**: CONFIRMED (names)
- **Port impact**: pickups (M5, M7); Q-027, Q-031, Q-033.

### D-014 — Game types
- **Area**: game flow  **Status**: OPEN
- **Wolf3D**: four difficulties; episodes.
- **OBC**: Difficulty 1–3, Random, Network, Network Teams, Modem.
- **Evidence**: `README.TXT` "Game Types".  **Confidence**: CONFIRMED (existence)
- **Port impact**: M10, M13; Q-012.

### D-015 — Save system
- **Area**: persistence  **Status**: OPEN
- **Wolf3D**: 10 named slots, `SAVEGAM?.` files.
- **OBC**: named slots (`SVGMn.SAV`); an AutoSave slot (`AUTSV.SAV`) written only by the game just before a floor change; F9 QuickSave writes to the slot most recently saved from the main menu; messages `NO CURRENT SAVE`, `SAVED %s`, `UNABLE TO SAVE`, `Game Saved in AutoSave Slot.`
- **Evidence**: `README.TXT`; strings.  **Confidence**: CONFIRMED (behavior as described); slot count UNKNOWN (Q-013)
- **Port impact**: M11.

### D-016 — Rendered sequences
- **Area**: presentation  **Status**: OPEN
- **Wolf3D**: none.
- **OBC**: pre-rendered animations when changing floors or dying, optional at install time; parameters `norndseq` and `slowrndseq`; an animation player (`ANM proc`, `frame %d uncool chunk # %d`).
- **Evidence**: `README.TXT` "Install types"; strings.  **Confidence**: CONFIRMED (existence); format UNKNOWN (Q-015)
- **Port impact**: `obc-formats::sequences`, M10.

### D-017 — Diagnostics and logging
- **Area**: tooling  **Status**: OPEN
- **Wolf3D**: `Quit()` prints a message and exits.
- **OBC**: `BC ERROR:` messages and a report written to `BC.LOG`; per-subsystem labels (`ANM`, `MEM`, `KBD`, `PG1`, `PG2`, `SND`, `CCH`, `USR proc`); `DEBUG.TXT`; a diagnostics screen (heap, EMS, XMS, statics, doors, actors counts).
- **Evidence**: strings; `BC.BAT`.  **Confidence**: CONFIRMED
- **Port impact**: error reporting design (DESIGN §6.6); diagnostics overlay (M10.8).

### D-018 — Command-line parameters
- **Area**: startup  **Status**: OPEN
- **Wolf3D**: `-goobers`, `nowait`, `baby`, `easy`, `normal`, `hard`, `tedlevel`, `res`, `nojoys`, `nomouse`, … (`WOLFSRC/WL_MAIN.C`, `ID_*`).
- **OBC**: `GOLEVEL`, `NOWAIT`, `COMP`, `NOCOMP`, `nojoys`, `nomouse`, `noems`, `noxms`, `nomain`, `no386`, `nopics`, `cdmusic`, `norndseq`, `slowrndseq`, possibly `HIDDENCARD`.
- **Evidence**: strings.  **Confidence**: CONFIRMED (strings); semantics UNKNOWN (Q-028)
- **Port impact**: CLI flags (M10.10).

### D-019 — Debug and cheat features
- **Area**: tooling, simulation  **Status**: OPEN
- **Wolf3D**: debug keys after `-goobers`, MLI cheat, etc. (`WOLFSRC/WL_DEBUG.C`, `WL_PLAY.C`).
- **OBC**: passwords or sequences `vfr750hondarc45`, `nsr250`, `president`; effects `WALKTHRU ON/OFF`, `INVINCIBLE`, `VULNERABLE`, `BOGUS MODE`, `RESTOCK`, `HEALTH HELP`, `BEWITCHED`, `UNFINISHED BUSINESS`, `Area DBG`, `Walk DBG`, `GENERATORS ON/OFF`, `TREMORS ON/OFF`; position and tile readouts.
- **Evidence**: strings.  **Confidence**: CONFIRMED (strings); activation and effects UNKNOWN (Q-026)
- **Port impact**: `obc-sim::cheats`, M10.8.

### D-020 — Multiplayer
- **Area**: networking  **Status**: OPEN
- **Wolf3D**: none.
- **OBC**: IPX networking, serial null-modem and Hayes modem play; teams (Team 1–3); character choice (Agent, Terrorist 1, Terrorist 2); kill counts; 10 net levels.
- **Evidence**: strings; `README.TXT`.  **Confidence**: CONFIRMED (existence)
- **Port impact**: M13; Q-019.

### D-021 — World mechanics: generators and tremors
- **Area**: simulation, rendering  **Status**: OPEN
- **Wolf3D**: none.
- **OBC**: generator state toggled (`GENERATORS ON/OFF`); tremors toggled (`TREMORS ON/OFF`).
- **Evidence**: strings.  **Confidence**: CONFIRMED (existence); mechanics UNKNOWN (Q-018)
- **Port impact**: `obc-sim::world_fx`, renderer screen shake.

### D-022 — Floors and ceilings
- **Area**: rendering  **Status**: OPEN
- **Wolf3D**: solid ceiling color per level (`vgaCeiling[]`), solid floor color (`WOLFSRC/WL_DRAW.C`).
- **OBC**: debug strings `Rindex %d`, `Findex %d`, `Rclr %d`, `Fclr %d` suggest roof and floor indices or colors; plane-0 floor code patterns suggest per-tile variation.
- **Evidence**: strings; plane statistics.  **Confidence**: HYPOTHESIS
- **Port impact**: renderer (M4.3); Q-003.

### D-023 — Palette
- **Area**: rendering  **Status**: OPEN
- **Wolf3D**: `gamepal` linked from `OBJ/GAMEPAL.OBJ`.
- **OBC**: `gamepal` at file 0x2DDB0 for the 3D view, sprites and HUD; a separate picture palette at file 0x2E3B0 for full-screen pictures; a third palette at 0x2E0B0 (role open).
- **Evidence**: `VL_FadeIn` call with far pointer 3A9B:0000; exported images.  **Confidence**: CONFIRMED (gamepal, picture palette)
- **Port impact**: `obc-formats::exe` palette reader; screens must switch palettes like the original; Q-025.

### D-024 — Help screen
- **Area**: UI  **Status**: OPEN
- **Wolf3D**: read-this text articles (F1).
- **OBC**: pressing `H` at the main menu shows default keys.
- **Evidence**: `README.TXT` "Game Help".  **Confidence**: CONFIRMED (existence)
- **Port impact**: M10.7.

---

## Quirks

Original bugs and oddities. Classification per DESIGN §4.3: `gameplay` (preserve in Classic), `crash` (always fix safely), `cosmetic` (fix in Enhanced only).

Template:

```markdown
### K-0NN — <title>
- **Class**: gameplay | crash | cosmetic  **Status**: OPEN | PRESERVED | FIXED (flag `<name>`)
- **Behavior**: <what the original does>
- **Evidence**: <trace, address>
- **OBCR handling**: <what we do in Classic and Enhanced>
```

### K-001 — The Map ThermoSensor is lost when a game is loaded
- **Class**: gameplay  **Status**: PRESERVED (fix flag `K_001`, KEEP HEAT SENSOR ON LOAD)
- **Behavior**: `PlayLoop`'s entry clears the ThermoSensor flag DS:0x3304 every time it runs, and `SaveTheGame` does not write it; so loading a game (like starting a floor or restarting after death) loses a sensor picked up on that floor. Reported as GitHub issue #12.
- **Evidence**: `PlayLoop` 1942:1315 (`DAT_3304 = 0` at entry; CONFIRMED); the save layout, GAMEFLOW.md §9.4 (no 0x3304 field; CONFIRMED by size arithmetic).
- **OBCR handling**: Classic and Enhanced clear it on load as the original does. With the fix on, `World::resume_play_loop` keeps it across a load (OBCR's save holds it); a new floor and a restart after death still start without it.

### K-002 — Items are taken only ahead of the view
- **Class**: gameplay  **Status**: PRESERVED; FIXED (flag `K_002`, OPTIONS > GAMEPLAY FIXES)
- **Behavior**: *GetBonus* runs only from *DrawScaleds*, for an item *TransformTile* places 0.13–0.79 tiles ahead of the player and within half a tile sideways. Walking backward never picks anything up, and strafing does only when the item is a little ahead (GitHub #7).
- **Evidence**: `1b4a:0d98`, `1b4a:01af` (`1b4a:02fd`–`0327`), `mindist` `DS:0x0D1E` = 0x5800, `focallength` 0x5700; CONFIRMED by instruction read. docs/re/PLAYER.md §4.5.
- **OBCR handling**: Classic as the original. The fix also takes an item in the player's own cell, in the same loop and order (`World::scan_scaleds`).

### K-003 — Picking up an owned weapon selects it
- **Class**: gameplay  **Status**: PRESERVED; FIXED (flag `K_003`, OPTIONS > GAMEPLAY FIXES)
- **Behavior**: *GiveWeapon* selects any weapon at least as good as `bestweapon`, owned or not, so a dropped Galil switches a player holding the Uzi back to the Galil (GitHub #14). Wolf3D selects only a better weapon.
- **Evidence**: `208c:066c` (`cmp ax,[0x8714]; jl` at `208c:0684`) in `COUNT.EXE`; the same bytes in `BCCD.EXE` (file 0x14798); CONFIRMED by instruction read. docs/re/PLAYER.md §4.5.
- **OBCR handling**: Classic as the original. The fix selects only a weapon not owned before; `bestweapon` still rises so the number keys reach it (`player::give_weapon`).

### K-004 — An opened door-look wall blocks shots and sight
- **Class**: gameplay  **Status**: PRESERVED; FIXED with flag `K_004` (OPTIONS > GAMEPLAY FIXES, off by default)
- **Behavior**: A door-look wall (plane 0 = its wall code, plane 1 = 103) opens as an animated wall: the player walks through, but its open frame stays in `tilemap` (69 → 72 for the floor-6 door at (10,48)). `CheckLine` passes a wall cell only when its plane-1 value is 0x69, 0x6A or 0x6B (105–107), so an open door-look wall stops the player's hit-scan shots (`GunAttack` retries with `CheckLine`), enemy sight (`CheckSight`) and enemy shots (`T_Shoot`). An enemy beyond it reacts only once the player stands in the doorway. GitHub issue #12. The reporter says the shareware build does not have this; UNKNOWN here (no shareware `COUNT.EXE` was examined).
- **Evidence**: `CheckLine` 1e43:1982, the plane-1 tests at 1e43:1af8, 1b0e, 1b2d (x pass) and 1e43:1ca0, 1cb6, 1cd5 (y pass), then `tilemap` 0 → clear, 0x80–0x100 → door test, anything else → blocked. CONFIRMED.
- **OBCR handling**: Classic plays it as the original (`actors::line_cell_clear`). With the fix, a cell holding an animated wall in state 0 (open) or 4 (permanently open) is clear, for shots and sight alike. Test: `crates/obc-sim/tests/doors_and_noise.rs`.

### K-005 — An actor on an ambush tile without ambush handling reads `areabyplayer` out of bounds
- **Class**: gameplay  **Status**: PRESERVED (reproduced; no fix offered)
- **Behavior**: `areabyplayer` is a 22-word array at `DS:0x48EA`, read with the actor's area byte and no bound check. Spawners other than `SpawnStand` do not handle ambush tiles (plane 0 = 0x8A). The sewer fat man (class 17, spawner 2390:0f7e) on floor 4's ambush tile at (58,10) is an example. Such an actor gets area 0x8A − 0x8C = 254, and `areabyplayer[254]` reads `DS:0x4AE6` = `actorat[0][30]` (`actorat` is `DS:0x4AAA`, words `[x][y]`). That is the map's west-edge wall, so the read is non-zero. The actor therefore always thinks (`DoActor`), hears every shot anywhere on the floor (`SightPlayer`'s `madenoise` path) and passes `CheckSight`'s area test. GitHub issue #10: in DOS the fat man wakes when the rats near him are shot; beta 4 returned false for out-of-range areas and he never woke.
- **Evidence**: `cmp word [area*2 + 0x48EA], 0` in SightPlayer 1e43:23a7, CheckSight 1e43:1d48, DoActor 1942:113a and A_IdleSound 2390:21c7; `DS:0x4AAA` is `actorat` (`SHL BX,7` indexing at 1e43:0452; 8,192 bytes saved by `SaveTheGame`). The fat man's area 254 was read from the reporter's beta-4 save and from the floor-4 map (plane 0 = 138, plane 1 = 179 at (58,10)). CONFIRMED.
- **OBCR handling**: `World::area_by_player` reproduces the read: areas 224–255 read `actorat[0][area − 224]`. Areas 22–223 would read other globals (`DS:0x4916`–`0x4AA9`, UNKNOWN values in play) and return false, as before. The original also *writes* `areabyplayer[area] = 1` with actor or player areas (2281:037b, 2281:03c8, 208c:0ebf). For an area of 22 or more this would corrupt `actorat`; it is not reproduced (UNKNOWN whether any floor triggers it). Test: `crates/obc-sim/tests/doors_and_noise.rs`.
