# OBC level setup and map codes

Reverse-engineering notes for level setup, the meaning of every plane-0 and plane-1 value used by the 61 map slots, the per-level tables, level progression and area connectivity. Static analysis of `COUNT.EXE` (release OBC-A) from the Ghidra text export plus a 16-bit disassembly of ranges Ghidra did not turn into functions (llvm-objdump on the raw image), and parses of `MAPHEAD.BC`/`MAPTEMP.BC` and `GFXTILES.BC`. Nothing here was observed in DOSBox-X yet.

Conventions:

- Addresses are `ghidra SSSS:OOOO` (Ghidra base 0x1000, see [SYMBOLS.md](SYMBOLS.md)); code in this file is written as `1823:01c8` for short. Far calls in raw disassembly appear with image segments (`lcall 0x1390:0x04ca` = `2390:04ca`).
- DGROUP data is `DS:0xNNNN`. Initialized data is read from the file at `DS offset + 0x35580`.
- "Wolf3D" refers to `WOLFSRC/` (v1.4 source). Confidence tags follow AGENT_GUIDE §4.1. CONFIRMED here means "read instruction by instruction" (or a data-file parse), not traced.
- Directions: player starts use Wolf3D's compass (`angle = (1 − d)·90`): 0 = N, 1 = E, 2 = S, 3 = W. Actor spawn codes use Wolf3D `dirtype` after `dir·2`: 0 = E, 1 = N, 2 = W, 3 = S.
- "Skill" is `gamestate.difficulty` at `DS:0x86FC`. In OBC it is the **game type**: 0–2 = the three difficulty levels, 3 = Random, 4 = Network, 5 = Network Teams, 6 = Modem ([DELTA_SPEC D-014](DELTA_SPEC.md); [GAMEFLOW.md](GAMEFLOW.md) §4). Code tests `>= 4` for "network".
- Actor class names follow [ACTORS.md](ACTORS.md) §5, which holds the full actor details (states, hit points, drops). This file only covers how map codes create them.

## 0. Summary

- **Setup**: `SetupGameLevel` `1823:0919` → plane-0 pass (AREATILE 140) → `SpawnDoor` for 130–137 → `PrepareLevel` `2f4d:0167` → `ScanInfoPlane` `1823:01c8` (jump table `1823:06c3`, codes 19–317) → ambush clean-up (138). CONFIRMED.
- **Plane 0**: 1–129 walls, one page each (`page = code − 1`, no light/dark pairs); 130–137 doors (locks only change the texture); 138 ambush; 140–161 floor = area `code − 140` and textured floor/ceiling set `code − 144` (145 = ceiling-light set). Door-look walls 13/29/49/65/69, glass 57/117, generator switch 95/96, exits 113/114/115 (ladder/stairs/elevator going by their textures). CONFIRMED except the texture readings (HYPOTHESIS).
- **Plane 1**: < 19 ignored (18 = background); 19–22 player; 23–85 statics (sprite `code − 22`); 86–88/103/106/107 animated walls; 89 explosion-proof; 90–97 patrol arrows; 98 push wall; 104/105 see-through walls (LIKELY); 108–317 actors in three skill tiers (OBC thresholds ≥ 1, ≥ 2); 282–285 fire. CONFIRMED unless marked.
- **Per level**: roof/floor ramp indices and colours (`DS:0x9E6/0xA19/0xA4C/0xA7F`, ramps at `DS:0x9A4`), song table for maps 0–9 (random after), sewer overrides for floor textures and shading. CONFIRMED. No par times found (LIKELY).
- **Progression**: strictly `mapon + 1`, maps 0–39; exits need ≥ 50 % kills; map 39 has no exit, killing the trench-coat boss wins. No episodes; CRED 40–49 unused except as colour entries for NETLEVELs 50–59. CONFIRMED.
- **Areas**: NUMAREAS 22; doors and animated walls connect areas; OBC's `Thrust` flips `areabyplayer` on every area change, which interacts oddly with the 145 lamp islands (quirk candidate, §3.6).

---

## 1. Functions

| Ghidra | Proposed name | Wolf3D counterpart | Role | Confidence |
|---|---|---|---|---|
| `1823:0919` | `SetupGameLevel` | `WL_GAME.C:625` | Load map, fill `tilemap`/`actorat`, spawn doors, call ScanInfoPlane, clear ambush tiles | CONFIRMED |
| `1823:01c8` | `ScanInfoPlane` | `WL_GAME.C:221` | Plane-1 switch (Ghidra did not create the cases: code `1823:020f–06a5`, jump table `1823:06c3`, 0x12B entries for codes 19–317) | CONFIRMED |
| `2f4d:0167` | `PrepareLevel` (new) | — | Per-level resets, health clamp, automap wall bitmap, clears plane-1 bit 15 | CONFIRMED |
| `1942:0bdf` | `InitActorList` | `WL_PLAY.C` | | LIKELY |
| `2281:0404` | `InitDoorList` | `WL_ACT1.C:332` | Clears `areabyplayer` (`DS:0x48EA`, 0x2C bytes) and `areaconnect` (far, 0x1E4 = 22×22 bytes), `lastdoorobj = DS:0x7BAA`, `doornum = 0` | CONFIRMED |
| `2281:0006` | `InitStaticList` | `WL_ACT1.C:124` | `laststatobj (DS:0x7E2A) = DS:0x7E2C` | CONFIRMED |
| `273a:0fdc` | `CA_CacheMap` | `ID_CA.C` | Called with `mapon` directly (no `10·episode`) | LIKELY |
| `273a:0c51` | `CA_LoadAllSounds` | `ID_CA.C` | | LIKELY |
| `208c:1324` | `SpawnPlayer` | `WL_AGENT.C:1100` | codes 19–22 | CONFIRMED |
| `2281:0011` | `SpawnStatic` | `WL_ACT1.C:139` | codes 23–85; statinfo `DS:0x11DA` | CONFIRMED |
| `2281:020f` | `PlaceItemType` | `WL_ACT1.C:199` | drops by item type (`Quit(602)` if the type is not in statinfo) | CONFIRMED |
| `2281:02a3` | `PlaceStatic(index,x,y)` (new) | — | places `statinfo[index]` directly (explosion debris, indices 12/13) | CONFIRMED |
| `2281:0437` | `SpawnDoor` | `WL_ACT1.C:350` | plane-0 codes 130–137 | CONFIRMED |
| `2390:04ca` | `SpawnStand` | `WL_ACT2.C:847` | `SpawnStand(which,x,y,dir)` | CONFIRMED |
| `2390:0741` | `SpawnPatrol` | `WL_ACT2.C:983` | `SpawnPatrol(which,x,y,dir)` | CONFIRMED |
| `2390:0911`, `0f20`, `0f7e`, `0fdc`, `103a`, `1094` | special spawners | `SpawnBoss` etc. | codes 214, 196, 179, 232/233/250/251, 197, 215 | CONFIRMED |
| `2634:00e4` | `SpawnAlly` | — | codes 224–227 (+ tiers) | CONFIRMED |
| `30d6:0161` | `SpawnFireActor` | — | codes 282–285 (also explosions, burning) | CONFIRMED |
| `2f4d:0847` | `SpawnAnimWall` (new) | — | plane-1 86–88, 103, 106, 107, 124 | CONFIRMED |
| `2f4d:08e8` | `OperateAnimWall` | — | Use on a door-look wall | CONFIRMED |
| `2f4d:0978` | `MoveAnimWalls` | — | per-frame (called from PlayLoop) | CONFIRMED |
| `2f4d:054a` | `ConnectAnimWallAreas` | (`DoorOpening` area part) | | CONFIRMED |
| `2f4d:0adf` | `IsDoorFrameWall(x,y)` | — | tilemap ∈ 13–16, 29–32, 49–52, 65–68, 69–72 | CONFIRMED |
| `1e43:0cca` | `ActorBumpWall` | — | non-rat actors open door-look walls | CONFIRMED |
| `208c:104c` | `Cmd_Use` | `WL_AGENT.C:1008` | push walls, doors, switches, exits | CONFIRMED |
| `2281:0ce2` / `2281:0e58` | `PushWall` / `MovePWalls` | `WL_ACT1.C:732/809` | plane-1 98 | CONFIRMED |
| `2281:07f3` | `OperateDoor` | `WL_ACT1.C:498` | **no key check** | CONFIRMED |
| `2281:031c` / `037b` / `03c8` | `RecursiveConnect` / `ConnectAreas` / `InitAreas` | `WL_ACT1.C:293–316` | NUMAREAS = 22 | CONFIRMED |
| `208c:0dc7` | `Thrust` | `WL_AGENT.C:928` | updates `player->areanumber` and `areabyplayer` | CONFIRMED |
| `30d6:12b1` | `Explode(x,y,…)` (new) | — | damages walls/statics within ±2 tiles | CONFIRMED |
| `30d6:1bc0` | `TryExitLevel` (new) | (`VictoryTile`) | 50 % kill rule | CONFIRMED |
| `1823:0ea3` | `GameLoop` | `WL_GAME.C:1238` | see [GAMEFLOW.md](GAMEFLOW.md) §5 | CONFIRMED |
| `1942:1315` | `PlayLoop` | `WL_PLAY.C:1368` | loads per-level colour tables at start | CONFIRMED |
| `1942:0d2d` | `StartMusic` | `WL_PLAY.C:1023` | per-level song table | CONFIRMED |
| `1b4a:129c` | `ThreeDRefresh` | `WL_DRAW.C` | two AsmRefresh passes, floor/ceiling pass | CONFIRMED |
| `1d2a:007a` | `AsmRefresh` | `WL_DR_A.ASM` | plane-1 103–107 see-through cells | LIKELY |
| `1b4a:049a` / `05f1` | `HitVertWall` / `HitHorizWall` | `WL_DRAW.C` | page = tile − 1 | CONFIRMED |
| `1b4a:074e` / `08a1` | `HitHorizDoor` / `HitVertDoor` | `WL_DRAW.C` | door pages by lock | CONFIRMED |
| `1b4a:1494` / `1b4a:162b` / `30d6:1814` / `30d6:1a21` | floor/ceiling pass, per-tile projector, set selector, span drawer | — | textured floors and ceilings | CONFIRMED (structure), LIKELY (geometry) |
| `1b4a:0bc9` / `1b4a:0c72` | gradient fills (textures off / on) | `VGAClearScreen` | | CONFIRMED |
| `30d6:1c02` | `SetSewerFloors` (new) | — | overrides floor sets 0–3 on maps 0–4 | CONFIRMED |
| `1e43:1982` | `CheckLine` | `WL_STATE.C:1037` | plane-1 105/106/107 transparent | CONFIRMED |

---

## 2. SetupGameLevel (`1823:0919`) step by step

CONFIRMED (disassembly `1823:0919–0bfc`). Differences from `WOLFSRC/WL_GAME.C:625` in bold.

1. If not `loadedgame` (`DS:0x8CBA`): zero `DS:0x8728` (killcount), `0x8726` (secretcount), `0x872E` (killtotal), `0x872A` (secrettotal), `0x8730/0x8732` (TimeCount, long). **No `US_InitRndT` call here** (Wolf3D seeds the RNG in SetupGameLevel).
2. `CA_CacheMap(mapon)` — **`mapon` (`DS:0x86FE`) is the map slot itself; there is no episode offset.**
3. `mapwidth/mapheight` from `mapheaderseg[mapon]` (`DS:0x9397`); if not 64×64, `Quit(500)` (`1497:11f8`, OBC's numeric `Quit`).
4. `memset(tilemap DS:0x6AAA, 0, 0x1000)`, `memset(actorat DS:0x4AAA, 0, 0x2000)`.
5. Plane-0 pass (`y` outer, `x` inner): `tile < 0x8C` (**AREATILE = 140**, Wolf3D 107) → `tilemap[x][y] = actorat[x][y] = tile` (solid). Otherwise the cell is open (`tilemap = actorat = 0`); **if `tile == 143` the plane-1 word at that cell gets bit 15 set** (`1823:09eb`), see §4.7.
6. `InitActorList`, `InitDoorList`, `InitStaticList`.
7. Plane-0 pass: codes **130–137** (Wolf3D 90–101) → `SpawnDoor(x, y, vertical, lock)` with even codes `vertical = 1, lock = (tile−130)/2` and odd codes `vertical = 0, lock = (tile−131)/2` (`1823:0a68–0ab5`, table `1823:0bfe`). Only 4 door kinds instead of Wolf3D's 6.
8. **`PrepareLevel()` (`2f4d:0167`, new)**, then `ScanInfoPlane()`.
9. Ambush pass: for each cell with plane-0 **138** (AMBUSHTILE, Wolf3D 106) or `actorat == 138`, **only if the plane-1 value there is ≥ 108** (any actor code): `tilemap = 0`, clear `actorat` if it still holds 138, and replace the plane-0 code by a neighbouring area code (east, north, south, west, last match wins; same order as Wolf3D). Every 138 cell in the shipped maps has a plane-1 value ≥ 108 (checked over all 61 maps), so the condition never leaves a 138 wall behind.
10. `CA_LoadAllSounds()`; `sprintf(DS:0x874C, "%d", killtotal − killcount)` (the "enemies left" counter).

`PrepareLevel` (`2f4d:0167`), CONFIRMED:
- Clears the animated-wall list (`DS:0xE7A8 = 0`), `DS:0xE7B2 = 0`, and 200 bytes at `DS:0x94ED`.
- If not `loadedgame`: `sprintf(DS:0x8756, "%d", mapon+1)` (floor number for the status bar), `DS:0x872C = DS:0x8742 = 100`, **health (`DS:0x870C`) = min(health, maxhealth[gametype])** with `maxhealth` at `DS:0x32EC` = {400, 400, 400, 400, 800, 800}; resets `DS:0x871C`, `DS:0x8724`, folds `DS:0x8778`/`0x877E` into running long totals, clears `DS:0xC3CC` and `DS:0xCD87`.
- Builds a 64×64 bit map at `DS:0xE093` (512 bytes, bit masks at `DS:0x3258`): a bit is set for every solid wall whose plane-1 value is not 107, and for every open or door cell whose plane-1 word has bit 15 set; bit 15 is then cleared. This is the wall layer of the Tab overhead map ([RENDERER.md](RENDERER.md) §9.2): CONFIRMED. "Solid" means `tilemap` 1–0x7F: plane-0 codes 128–139 (including ambush 138) have bit 7 set and take the door test, so they are not drawn.

---

## 3. Plane 0

### 3.1 Value classes

| Range | Meaning | Evidence | Confidence |
|---|---|---|---|
| 0 | open cell with area −140 (only map 60, which is empty) | `1823:09c2` | CONFIRMED |
| 1–129 | solid wall; texture page = **value − 1** on both faces | `1823:09c2`, `1b4a:05d3`, `1b4a:0726` | CONFIRMED |
| 130–137 | doors (4 lock kinds × 2 orientations) | `1823:0a68` | CONFIRMED |
| 138 | ambush marker (solid until cleared by step 9) | `1823:0b08` | CONFIRMED |
| 139 | solid wall code with no special handling (unused) | | CONFIRMED |
| 140–161 | open floor; **area number = value − 140** (NUMAREAS 22); **floor/ceiling texture set = value − 144** when 0–9 | `1823:09eb`, `208c:0e91`, `1b4a:1960` | CONFIRMED |
| 143 | open floor, area 3, plus "show on automap" flag (unused in the shipped maps) | `1823:09eb` | CONFIRMED |
| ≥ 162 | would be open floor with an out-of-range area (Quit 603–605 in ConnectAreas); unused | `2281:0328` | CONFIRMED |

### 3.2 Wall textures

- `HitVertWall` and `HitHorizWall` both use `wallpic = tilehit − 1` (`1b4a:05c9`, `1b4a:0726`). **There are no light/dark pairs**: Wolf3D's `horizwall[i] = (i−1)·2`, `vertwall[i] = (i−1)·2+1` (`WL_MAIN.C:712`) is gone, and the 136 wall pages are 136 single textures. Pages 0–135 map to codes 1–136. CONFIRMED.
- **Door-frame faces.** When the plane-1 value of the wall cell is **125**, and the cell the ray came from is a door (`tilemap & 0x80`) or a door-look wall (`IsDoorFrameWall`, `2f4d:0adf`), the face draws page `DOORWALL + 2` = **130** (Wolf3D draws `DOORWALL+3`/`+2` for every face next to a door, flagged by `tilehit & 0x40`). Code 125 does not occur in the shipped maps, so in practice door jambs are ordinary wall textures. CONFIRMED (code), observed effect UNKNOWN.
- Distance shading: `ScalePost` picks a shade from the 256-entry table `DS:0xC2CA` built by `30d6:11b6` (Borland float emulation), with a different curve when `DS:0x33B8 = 1` (sewers, §5). LIKELY.

### 3.3 Special walls

All CONFIRMED from `Cmd_Use` (`208c:104c`), `MoveAnimWalls` (`2f4d:0978`) and `Explode` (`30d6:12b1`) unless noted.

| Codes | What | Behaviour |
|---|---|---|
| 13–16, 29–32, 49–52, 65–68, 69–72 | **door-look walls** (4 animation frames each; magenta pixels are transparent in the second drawing pass, §4.6) | Use plays a sound (13/16 → 0x51, 29/32 → 0x50, 49/52 → 0x4E, 65/68/69/72 → 0x4F; `208c:12b5–12e3`). Then, if the plane-1 value is **103 or 106**, `OperateAnimWall(x,y)`; **otherwise the plane-1 value is cleared to 0** (`208c:1303`). Opening increments `tilemap` frame by frame to the last frame (code + 3), then makes the cell passable; closing reverses. Maps use only the first frame (13, 29, 49, 65, 69). Plane 1 on them: 69 → 103 (508 cells) or 18 (73 cells: a door look-alike that only plays its sound), 29 → 103/106, 49 and 65 → 103, 13 → 106/103. Non-rat actors open them by walking into them (`1e43:0cca`; classes 3, 6, 7, 21 cannot). |
| 73–76 | animated wall (porthole fan, pages 72–75) | Loops frames 73→76→73 when plane 1 holds 86/87/88 (§4.4). |
| 25 | wall | Only special for floor textures (§3.5). |
| 57–60 | glass pane (57 intact, 60 broken) | An explosion turns 57–60 into **60** (`30d6:14b1`); floor set 7 under it. Plane-1 104 makes it see-through (§4.6). |
| 117–120 | second glass/screen pane (117 intact, 120 broken) | Explosion → **120** (`30d6:14c0`). |
| 77 | damaged wall | Any other destructible wall within ±2 tiles of an explosion becomes 77 (`30d6:1705`); the centre cell is destroyed instead (§4.8). Also placed by the designers. |
| 85 | special panel | Use calls `OperateAnimWall` directly (`208c:12a7`) and opening it does not connect areas (`2f4d:09cb`). Not used as plane 0 in the shipped maps. |
| 88 | rock opening (page 87: rock with a transparent hole), maps 0, 3, 4; every one carries plane-1 **107** (passable, thin, §4.4) | Use on it: if health (`DS:0x870C`) < `DS:0x8742`, heal 25 (`208c:05f4`) and show `Medic Pack`; then the plane-1 rule above (107 is neither 103 nor 106, so the 107 is cleared to 0, which does not undo the already-spawned open record). Whether this healing opening is intended is UNKNOWN; the code path is CONFIRMED. |
| 95 / 96 | **generator switch** off/on | Use toggles 95↔96 (`tilemap ±1`), `FinishPaletteShifts`, palette mode 2/1 (`2f4d:035e`), message `GENERATORS OFF`/`GENERATORS ON`, sound 0x54 ([GAMEFLOW.md](GAMEFLOW.md) §3, D-021). |
| 113 | **exit, ladder** (sewers 0–4) | Use: `exitkind (DS:0x922) = 0`, `TryExitLevel` (§6). |
| 114 | **exit, stairs** | Same as 113 (`exitkind = 0`). In network games (`gametype ≥ 4`) sets `playstate = 1` directly, without the kill rule. |
| 115 | **exit, elevator** | `exitkind = 1`, `TryExitLevel` (also in network games). |
| 130 (page) | door jamb | `DOORWALL + 2` (§3.2). |

Texture appearance (from the page export, HYPOTHESIS): 113 → page 112 a ladder, 114 → page 113 stairs, 115 → page 114 an elevator keypad; pages 94/95 (codes 95/96) a generator/switch box. That `F3` is the stairs sequence and `F4` the elevator one follows from this: LIKELY ([GAMEFLOW.md](GAMEFLOW.md) §6 describes the sequences).

### 3.4 Doors (130–137)

- `SpawnDoor(x, y, vertical, lock)` (`2281:0437`), CONFIRMED: MAXDOORS 64 (`Quit(606)`), `doorobjlist` at `DS:0x7BAA`, **10-byte records** `{u8 tilex, u8 tiley, u16 vertical, u8 lock, pad, u16 action, u16 ticcount}`, `action = dr_closed (1)`, `tilemap = actorat = doornum | 0x80`, and the plane-0 cell copies the area code from the west (vertical) or north (horizontal) neighbour, as Wolf3D.
- **Locks are cosmetic.** `OperateDoor` (`2281:07f3`) switches only on `action` (table `2281:0829`: open/opening → CloseDoor `2281:0528`, closed/closing → OpenDoor `2281:04f0`); Wolf3D's `gamestate.keys` test (`WL_ACT1.C:505`) is absent. CONFIRMED.
- Door page by lock (`HitHorizDoor` `1b4a:0848`, table `1b4a:0895`; `HitVertDoor` table `1b4a:09e7`; both orientations use the same page): with `DOORWALL = PMSpriteStart − 8 = 128` (`DS:0xCA64` is `PMSpriteStart`, used as such by `ScaleShape` `1d8b:0715`; value 136 from GFXTILES):

| Lock | Plane-0 codes (vert, horiz) | Door page | Uses in maps |
|---|---|---|---|
| 0 | 130, 131 | 128 (DOORWALL+0) | 4 + 119 |
| 1 | 132, 133 | 132 (DOORWALL+4) | 7 + 50 |
| 2 | 134, 135 | 133 (DOORWALL+5) | 38 + 2 |
| 3 | 136, 137 | 132 (DOORWALL+4) | unused |
| 4, 5 | (not reachable from maps) | 133, 135 | — |

  CONFIRMED (code). Page 128 looks like a radial light gradient and page 133 like an eagle emblem panel; whether page 128 really is the lock-0 door face should be checked in DOSBox-X (open item).
- The "door" routines also handle **non-door tiles** (`tilehit & 0x80 == 0`): `doornum` stays 99 and the page is `tilehit − 1`, drawn as a thin mid-cell wall. This is how door-look walls and other plane-1 103/106/107 cells are drawn (§4.6). CONFIRMED.

### 3.5 Areas, floor and ceiling textures (140–161)

- **Area numbers.** `areanumber = plane0 − 140` everywhere (SpawnNewObj `1e43:05ce`, SpawnPlayer `208c:1467`, Thrust `208c:0e91`, TryWalk `1e43:0cae`, door/anim-wall connection). NUMAREAS = **22** (`2281:0328`, `areaconnect` 22×22). Used values: 140 (area 0, 15 cells in map 3) and 144–151 (areas 4–11). CONFIRMED.
- **Floor/ceiling set.** For each visible cell (`spotvis`) and the player's cell, when floor textures are on (`DS:0x33D2 ≠ 0`, a config option toggled in CheckKeys `1942:07a5`), `1b4a:162b` projects the cell's four corners and `30d6:1814` draws it with set `n = plane0 − 144` (0–9) from the 10×2-byte table `DS:0x33BE`: byte `2n` = floor page + 1, byte `2n+1` = ceiling page + 1 (span drawer `30d6:1a21` writes the `2n+1` texture to row `y` in the upper half and the `2n` texture to the mirrored row in the lower half). CONFIRMED (code and table), LIKELY (which half is which, from the row arithmetic).

| Plane 0 | Set | Floor page | Ceiling page | Maps 0–4 (sewers, overridden by `30d6:1c02`) |
|---|---|---|---|---|
| 144 | 0 | 80 | 81 | floor 11, ceiling 11 |
| 145 | 1 | 82 | 83 | floor 105, ceiling 107 |
| 146 | 2 | 84 | 81 | floor 109, ceiling 111 |
| 147 | 3 | 86 | 81 | floor 11, ceiling 107 |
| 148 | 4 | 104 | 81 | (as left) |
| 149 | 5 | 106 | 81 | |
| 150 | 6 | 108 | 81 | |
| 151 | 7 | 110 | 110 | |
| 152 | 8 | 91 | 91 | (unused) |
| 153 | 9 | 110 | 111 | (unused; also the fallback) |

  `30d6:1c02` runs at level start (from `30d6:0e5b` in PlayLoop) and rewrites entries 0–3 for maps 0–4 and restores the defaults (81,82),(83,84),(85,82),(87,82) for every other map. CONFIRMED.
- **The 145 grid** inside 144 rooms is therefore a ceiling-light pattern: set 1 uses ceiling page 83 (white tile) and floor page 82 instead of 81/80. HYPOTHESIS for the visual reading; CONFIRMED that 145 selects set 1.
- **Set selection order** (`1b4a:18cf–197a`): the cell's own `tilemap` is tested first, which only matters for visible cells that hold a thin wall (open door-look wall, glass): 25 and 29–32 → set 3, 49–52 → set 0, 57–60 → set 7, 65–72 and 117–120 → set 0. Otherwise the set is `plane0 − 144`; if that is not 0–9 (plane-0 140–143, or a wall code left under a pushed or destroyed wall), `1b4a:19ef` searches 8 neighbours (offsets `DS:0x0D5C`) with the same two rules and falls back to **set 9**. **K: the neighbour table is (0,1), (1,0), (0,1), (−1,0), (−1,−1), (1,−1), (1,1), (−1,1)** — `(0,−1)` is missing and `(0,1)` appears twice. CONFIRMED.
- Texel shading: floor/ceiling texels are darkened by the distance shade `DS:0xC004` (from `DS:0xC2CA`) but clamped to the base of their 16-colour ramp (first texel's high nibble) (`30d6:1a21`). CONFIRMED.
- When floor textures are off, the gradient fill `1b4a:0bc9` uses the per-level ramps (§5.1); when on, `1b4a:0c72` first fills with the per-level colours `Rclr`/`Fclr`, then textured cells overwrite it.

### 3.6 Area quirk candidate (145 islands)

`Thrust` (`208c:0dc7`) does **not** call `ConnectAreas` when the player's area changes (neither does Wolf3D's `Thrust`, `WL_AGENT.C:959`), but OBC adds: if the new area ≠ old and < 22, `areabyplayer[old] = 0; areabyplayer[new] = 1; player->areanumber = new` (`208c:0e97–0ecc`). Because the 145 ceiling-light cells are area 5 islands inside area-4 rooms (and areas only connect through doors/anim walls, §7), stepping onto a 145 cell switches `areabyplayer` from area 4 to area 5. `CheckSight` (`1e43:1d48`) and `DoActor` (`1942:113a`) require `areabyplayer[ob->areanumber]`, so inactive area-4 actors would stop noticing the player while he stands on a lamp cell, and area-5 actors (anyone standing on a 145 cell) only notice him there. CONFIRMED (code); gameplay effect HYPOTHESIS until observed. Candidate `K-###` for DELTA_SPEC.

### 3.7 Plane-0 value 143 and the automap flag

Code 143 (area 3) sets bit 15 of the plane-1 word (`1823:09f2`); `PrepareLevel` turns that into an automap "wall" bit for an open cell and clears bit 15. No shipped map uses 143. CONFIRMED.

### 3.8 All plane-0 values present in the 61 maps

Count = cells over all maps. Page = GFXTILES wall page (0-based, `.local/exports/vswap/walls/NNN.png`). Evidence: `1823:09c2` = solid/open split; `1b4a:05d3`/`0726` = page = code − 1. Texture names are left out on purpose for plain walls (see the page export).

| Code | Count | Maps | Meaning | Page | Evidence | Conf. |
|---|---|---|---|---|---|---|
| 0 | 4096 | 60 | empty (map 60 only) | - | `1823:09c2` | CONFIRMED |
| 1 | 29 | 1-4 | plain wall | 0 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 2 | 209 | 1-4 | plain wall | 1 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 3 | 29 | 1-4 | plain wall | 2 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 4 | 38 | 2-4 | plain wall | 3 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 5 | 3463 | 2-4 | plain wall | 4 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 6 | 879 | 2-4 | plain wall | 5 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 7 | 6416 | 0-4 | plain wall | 6 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 8 | 1960 | 0-4 | plain wall | 7 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 9 | 77 | 1-4,19,22,35,39 | plain wall | 8 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 10 | 571 | 2-4 | plain wall | 9 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 11 | 84 | 2-4 | plain wall | 10 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 12 | 1085 | 1-4 | plain wall | 11 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 13 | 27 | 0-4,19,22,35,39 | door-look wall, teal bars (frames 13-16); opens only with plane-1 103/106 | 12 | `208c:114a, 2f4d:0978` | CONFIRMED |
| 17 | 725 | 8,11,14,16,20-21,25,27,29,34 | plain wall | 16 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 18 | 189 | 6-7,11,14,16,18,20-21,23,25,27,29,32,34,37,55,58 | plain wall | 17 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 19 | 3873 | 27,30-33,36,53,58 | plain wall | 18 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 22 | 8192 | 6-18,20-22,24-30,32,34,36-39,52-53,55,58-59 | plain wall | 21 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 25 | 351 | 6-18,20-21,23-34,36-38,55,58-59 | plain wall; counts as floor set 3 in the floor-set rules (§3.5) | 24 | `1b4a:1922` | CONFIRMED |
| 26 | 2549 | 6-18,20-21,23-34,36-38,55,58-59 | plain wall | 25 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 27 | 210 | 6-18,20-21,23-29,31,33-34,37-38,55,59 | plain wall | 26 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 28 | 160 | 6-18,20-21,23-29,31,33-34,37-38,55,59 | plain wall | 27 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 29 | 101 | 6-9,11-14,16-18,20-21,23-29,31-32,34,36-38 | door-look wall, sliding panel (frames 29-32); opens only with plane-1 103/106 | 28 | `208c:114a, 2f4d:0978` | CONFIRMED |
| 33 | 55 | 10,19,35,38,59 | plain wall | 32 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 34 | 143 | 10,25,38,59 | plain wall | 33 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 35 | 263 | 10,24-25,27,31,33,36 | plain wall | 34 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 36 | 157 | 19,31-33,35 | plain wall | 35 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 37 | 338 | 6,11,15,20,24,26,31,34,36,38,59 | plain wall | 36 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 38 | 1509 | 6,8-13,15,19,24-27,29,33-35,38,51,59 | plain wall | 37 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 39 | 576 | 6,9,11,17-18,25,27,29,31,38,59 | plain wall | 38 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 40 | 42823 | 0-18,20-39,50-55,57-59 | solid fill | 39 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 41 | 395 | 23-34,36-39,58-59 | plain wall | 40 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 42 | 931 | 8,12-13,16-17,21,26-27,30,32,34,36,58 | plain wall | 41 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 43 | 331 | 9,23,25,27,29,37 | plain wall | 42 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 44 | 247 | 23,25,29,32,37 | plain wall | 43 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 45 | 56 | 24,36 | plain wall | 44 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 46 | 554 | 28-29,32,58 | plain wall | 45 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 47 | 228 | 19,35,56 | plain wall | 46 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 48 | 4787 | 19,22,35,39,56 | plain wall | 47 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 49 | 58 | 6-7,9-10,14-17,24,28,31-33,38 | door-look wall, double door (frames 49-52); opens only with plane-1 103/106 | 48 | `208c:114a, 2f4d:0978` | CONFIRMED |
| 53 | 535 | 6-8,10-12,15,20 | plain wall | 52 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 57 | 1071 | 6-18,20-21,23-26,28-29,31,34,36-38,55,59 | glass pane, intact (57-59 -> 60 broken) | 56 | `30d6:148f` | CONFIRMED |
| 58 | 112 | 38,59 | plain wall | 57 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 61 | 284 | 6-18,20-22,32,36,55 | plain wall | 60 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 62 | 298 | 6-18,20-22,32,36,55 | plain wall | 61 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 63 | 287 | 6-18,20-22,32,36,55 | plain wall | 62 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 64 | 283 | 6-18,20-22,32,36,55 | plain wall | 63 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 65 | 49 | 6-9,11,14,16-18,20,25-26,29,34-35 | door-look wall, dark wood (frames 65-68); opens only with plane-1 103/106 | 64 | `208c:114a, 2f4d:0978` | CONFIRMED |
| 69 | 581 | 5-39 | door-look wall, light wood (frames 69-72); opens only with plane-1 103/106 | 68 | `208c:114a, 2f4d:0978` | CONFIRMED |
| 73 | 17 | 1,3-4 | animated wall (porthole fan), frame 1 of 73-76 | 72 | `2f4d:0a65 (with plane-1 86-88)` | CONFIRMED |
| 77 | 857 | 7-9,11,13,17-18,20-21,38,55,59 | damaged wall (explosion result; also placed) | 76 | `30d6:157a` | CONFIRMED |
| 78 | 376 | 23-34,36-39,58-59 | plain wall | 77 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 79 | 365 | 23-34,36-39,58-59 | plain wall | 78 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 80 | 16 | 27,31,33-34,36 | plain wall | 79 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 82 | 355 | 5-6,19,24,35,57 | plain wall | 81 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 83 | 27 | 29 | plain wall | 82 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 84 | 8 | 19,35 | plain wall | 83 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 86 | 546 | 5-6,9,11-12,16-17,24,26,28,30,32-34,36-37,53,57-58 | plain wall | 85 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 88 | 24 | 0,3-4 | rock opening, always with plane-1 107; Use heals 25 (`Medic Pack`) | 87 | `208c:121e` | CONFIRMED |
| 89 | 399 | 17,19,24,28,35-36,53 | plain wall | 88 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 90 | 125 | 6,15,20-21,27 | plain wall | 89 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 91 | 48671 | 5-49,51,54-55,57-59 | solid fill / border | 90 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 92 | 238 | 10,14-15,17,24-25,27,29,31 | plain wall | 91 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 93 | 246 | 6-9,11,14,16-18,20-21,23-25,27,29,32,34,37-38,59 | plain wall | 92 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 94 | 72 | 6-8,11,14,16-18,23-25,27,29,37-38,59 | plain wall | 93 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 95 | 9 | 3-4,6,10,18,23,28,31,36 | generator switch OFF (Use -> 96) | 94 | `208c:1241/1274` | CONFIRMED |
| 96 | 11 | 0-2,8,12,16,21,25,33,38,59 | generator switch ON (Use -> 95) | 95 | `208c:1241/1274` | CONFIRMED |
| 97 | 995 | 5-18,20-21,23-34,36-38,57-59 | plain wall | 96 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 98 | 6763 | 14,19,23-26,28-35,37-38,54,59 | plain wall | 97 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 99 | 63 | 14,26,31,33,54 | plain wall | 98 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 100 | 69 | 14,26,31,33,54 | plain wall | 99 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 101 | 320 | 14,19,23-26,28-29,31,33-35,37 | plain wall | 100 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 102 | 346 | 23-34,36-39,58-59 | plain wall | 101 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 103 | 75 | 14,19,26,31,33,35,54 | plain wall | 102 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 104 | 62 | 14,26,31,33,54 | plain wall | 103 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 105 | 177 | 31-32,58 | plain wall | 104 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 107 | 44 | 27 | plain wall | 106 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 109 | 1492 | 5-21,23-39,52,56,59 | plain wall | 108 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 111 | 48 | 19,25,35 | plain wall | 110 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 113 | 9 | 0-4 | exit: ladder (sewers) | 112 | `208c:11d8, 30d6:1bc0` | CONFIRMED |
| 114 | 72 | 5-18,20-38,50-51,53-55,57-58 | exit: stairs | 113 | `208c:11d8, 30d6:1bc0` | CONFIRMED |
| 115 | 19 | 5-21,52,56 | exit: elevator | 114 | `208c:11d8, 30d6:1bc0` | CONFIRMED |
| 116 | 1882 | 7-10,12-14,16-17,21,23-27,29,31,36-38,50,55,59 | plain wall | 115 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 117 | 1195 | 6-18,20-22,24-27,29-34,36,39,52-53,55,58 | glass/screen pane, intact (117-119 -> 120 broken) | 116 | `30d6:148f` | CONFIRMED |
| 121 | 8697 | 6-18,20-21,24-26,29-30,32,38,55,58-59 | plain wall | 120 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 124 | 1970 | 6-8,10-13,17-18,20-21,24,26,28,31,34,36,38,50,55,59 | plain wall | 123 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 127 | 291 | 6-8,11-13,17,20-21,23-24,26,28,31-32,34,36-37 | plain wall | 126 | `1823:09c2, 1b4a:05d3` | CONFIRMED |
| 130 | 4 | 19,35 | door, vertical, lock 0 | door pages | `1823:0a89/0aa0, 2281:0437` | CONFIRMED |
| 131 | 119 | 6-18,20-21,23-34,36-38 | door, horizontal, lock 0 | door pages | `1823:0a89/0aa0, 2281:0437` | CONFIRMED |
| 132 | 7 | 22,30 | door, vertical, lock 1 | door pages | `1823:0a89/0aa0, 2281:0437` | CONFIRMED |
| 133 | 50 | 17-19,21-22,24,26,28-30,32,34-37 | door, horizontal, lock 1 | door pages | `1823:0a89/0aa0, 2281:0437` | CONFIRMED |
| 134 | 38 | 5-21,35 | door, vertical, lock 2 | door pages | `1823:0a89/0aa0, 2281:0437` | CONFIRMED |
| 135 | 2 | 22,39 | door, horizontal, lock 2 | door pages | `1823:0a89/0aa0, 2281:0437` | CONFIRMED |
| 138 | 935 | 0-39 | ambush marker (cleared at setup if an enemy stands on it) | - | `1823:0b08, 2390:059a` | CONFIRMED |
| 140 | 15 | 3 | area 0, no floor set (neighbour search) | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 144 | 23732 | 0-39,55-56,58-59 | area 4, floor/ceiling set 0 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 145 | 5943 | 0-4,6-37,55,58 | area 5, floor/ceiling set 1 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 146 | 4648 | 2-4,7-8,10-15,18,20-31,33-34,37-39,54 | area 6, floor/ceiling set 2 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 147 | 4005 | 0,2-4,6-18,20-21,23-34,36-38,55,58-59 | area 7, floor/ceiling set 3 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 148 | 23280 | 6-21,23-32,34-38,51-53,56-59 | area 8, floor/ceiling set 4 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 149 | 8439 | 6-16,18,20-21,23-29,31-32,34,36-38,50,54-55,58-59 | area 9, floor/ceiling set 5 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 150 | 2687 | 5-39,56,59 | area 10, floor/ceiling set 6 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |
| 151 | 6737 | 5-19,21,23-38,40-49,57-59 | area 11, floor/ceiling set 7 | - | `1823:09eb, 1b4a:1960` | CONFIRMED |

The two solid fills: 40 (page 39, grey brick) fills solid regions of most maps; 91 (page 90) fills borders and solid regions of maps 5–59. Plane-0 values the code knows but the maps do not use: 14–16, 30–32, 50–52, 66–68, 70–72, 74–76 (later animation frames), 60, 120 (broken glass), 136/137 (lock-3 doors), 141–143, 152–161.

---

## 4. Plane 1

### 4.1 Overview

- Codes **< 19 are ignored** by ScanInfoPlane (`1823:01f2`). **18 is the editor background** (214,093 cells); 0 occurs only in empty map 60. CONFIRMED.
- Codes 19–317 go through the jump table at `1823:06c3` (index `code − 19`). Codes with no case: 89–97, 99–102, 104, 105, 125, 178, 228–231 (and their tiers 246–249, 264–267 after the skill test), 268, 269, 286, 287, 300–305. Several of these are read by other code (89, 90–97, 104, 105, 125). CONFIRMED.
- Plane-1 words are also **modified at run time**: bit 15 from plane-0 143 (§3.7), cleared by Use on door-look walls without 103/106 (§3.3), the push-wall marker 98 moves with the wall (§4.7), explosions clear destroyed cells (§4.8).
- Spawning order is row-major (`y` outer, `x` inner) like Wolf3D; it matters for `US_RndT` consumption: rats (`SpawnStand/Patrol which 0`), moving saucers, random item scattering (game type 3), player scattering (game type 3) and fire sounds draw random numbers during ScanInfoPlane. CONFIRMED.

### 4.2 Player starts (19–22)

`SpawnPlayer(x, y, code − 19)` (`208c:1324`): `angle = (1 − dir)·90` → 19 N, 20 E, 21 S, 22 W (as Wolf3D); state `DS:0x0F46`; `areanumber = plane0 − 140`; `Thrust(0,0,0)`; `InitAreas`. **Game type 3 (Random)**: if `actorat` at the start cell is non-zero, the code tests the W, E, N, S neighbours, but the chosen neighbour is then overwritten with the original cell at `208c:143d` (**K candidate**: only the "all four occupied" case has an effect, which picks a random free cell within ±`DS:0x2FD0`·8 tiles via `2f4d:1388`). Network types call `358f:0d7b`. Every single-player map has exactly one start; CRED maps have one at (13,23). CONFIRMED.

### 4.3 Static objects (23–85)

`SpawnStatic(x, y, code − 23)` (`2281:0011`), CONFIRMED:
- statinfo at `DS:0x11DA`, 4 bytes `{i16 shapenum, i16 type}`; for codes 23–85 the sprite is **code − 22** (sprite page 136 + shapenum − 1… i.e. `.local/exports/vswap/sprites/NNN.png` with NNN = code − 22).
- `staticcount (DS:0x12DA)++`; `Quit(601)` when the 224-entry list (`DS:0x7E2C`, **10-byte records**: `u8 tilex, u8 tiley, u16 visspot, i16 shapenum, u8 flags, u8 (+7), u8 itemnumber, pad`) is full; **returns without spawning if `actorat[x][y] == 1`** (cell already blocked by a static; the counter has already been incremented).
- By `type` (table `2281:01e5`): 0 → passable dressing; 1 → blocking (`actorat = 1`); 17 → blocking, byte +7 = 2; 18 → passable, +7 = 2 (unused); 2 → +7 = 1, itemnumber = statinfo index (unused); 3–14 and 20 → bonus (`flags = FL_BONUS 2`, itemnumber = type); 15, 16, 19 → no setup (unused).
- **Game type 3 (Random) scatters every bonus item**: up to 10 tries at a random cell within ±`DS:0x2FD0`·8 that is empty and holds no other static (`2281:0105–0195`).
- Pickup effects, `GetBonus` (`208c:074f`, switch on `itemnumber − 3`, table `208c:09d3`), CONFIRMED (messages are the strings pushed):

| Type | Plane 1 | Sprite | Effect |
|---|---|---|---|
| 3 | 56 | 34 | ammo `DS:0x8740` += 30 (cap 999), no message |
| 4 | 57 | 35 | `Grenades!` (only if grenades `DS:0x878A` < 19) |
| 5 | 53 | 31 | `Galil` (weapon 2) |
| 8 | 59 | 37 | `Flak Jacket`: armor `DS:0x8750` += 100 if < 400 (cap 999) |
| 9 | 60 | 38 | `Increased Health`: health += 100 if < 400 (cap 999), and max health `DS:0x8742` = health |
| 10 | 85 | 63 | `Map ThermoSensor` |
| 11 | 52 | 30 | `Uzi` |
| 12 | 54 | 32 | `Flame Thrower` |
| 13 | 55 | 33 | `Grenade Launcher` |
| 14 | 58 | 36 | `Fuel Pack` |
| 20 | 61, 62 | 39, 40 | `BOOBY TRAP`: explosion at the item (`2390:0ae4`), damage, sound 0x4B |

Weapon numbering and exact amounts belong to [PLAYER.md](PLAYER.md); the table above only ties map codes to `GetBonus` cases.

### 4.4 Animated walls (86–88, 103, 106, 107, 124)

`SpawnAnimWall(x, y, type)` (`2f4d:0847`), CONFIRMED. List at `DS:0x46E2`, max 32 (`Quit(703)`), count `DS:0xE7A8`, 12-byte records `{x, y, frame, state, type, timer}`. Initial state by type (`2f4d:08e0`): type 2 → state 4 (open, permanent: `actorat = 0`, areas connected); types 3–5 → state 5–7 (looping); others → state 1 (closed).

| Plane 1 | Type | Placed on (plane 0) | Behaviour |
|---|---|---|---|
| 86, 87, 88 | 3, 4, 5 | 73 (porthole fan) | Looping animation: every `timer` frames advance `tilemap` one frame, wrapping after 4 (`2f4d:0a65`); timer reload byte `DS:0x3217 + state` = 6, 3, 0 for states 5, 6, 7, i.e. one frame every 7th, 4th or every call. Cell stays solid. 87 also sits on other walls in maps 19 and 35. |
| 106 | 1 | 13, 29 | **Openable door-look wall**, closed. Drawn thin (mid-cell) with transparency; **transparent to `CheckLine`** (bars/glass). |
| 103, 124 | 6 | 69 (508), 29, 49, 65, 13, door cells 132/133, ladder 113 | **Openable door-look wall**, closed; drawn thin with transparency; opaque to `CheckLine`. 124 is unused. |
| 107 | 2 | 4, 88, 80 | Always open and passable, drawn thin: openings with transparent centres (page 3 pipe mouth, page 87 rock hole, page 79); transparent to `CheckLine`; excluded from the automap. |

States (`MoveAnimWalls` `2f4d:0978`): 1 closed → Use → 2 opening (frame 0: `ConnectAnimWallAreas(open)` unless the tile is 85; `tilemap++` each frame to frame 3; then state 0 and `actorat = 0`); 0 open → Use → if `actorat` is empty, state 3 and `actorat = 1` → closing (`tilemap--` each frame; at frame 0 state 1, `ConnectAnimWallAreas(close)`). State 4 ignores Use. The full rule of `OperateAnimWall` (`2f4d:08e8`, CONFIRMED): the first record on the tile decides; state 0 or 2 → state 3 and `actorat = 1` if `actorat` is empty (an opening wall still holds its wall marker, so in practice only state 0 closes); state 4 → nothing; every other state (1, 3, 5–7) → state 2, so a second Use reopens a closing wall. Animation advances one frame per call (per refresh, not per tic): LIKELY, timing to confirm. Unlike Wolf3D doors they **never close by themselves**. CONFIRMED.

`ConnectAnimWallAreas` (`2f4d:054a`) picks the two neighbours across the wall (east/west if both are open in `tilemap`, otherwise north/south), subtracts 140, checks 0–21 (`Quit(701/702)`), adjusts `areaconnect[a][b]` and `[b][a]` by ±1 and calls `ConnectAreas`. CONFIRMED.

### 4.5 Actors

[ACTORS.md](ACTORS.md) §5–6 has the class table and spawner details. Mapping, CONFIRMED from the case code at `1823:0295–069b`:

- `SpawnStand(which, x, y, dir)` / `SpawnPatrol(which, x, y, dir)`, `obclass = which + 3`, `dir·2`.
- Three skill tiers per group: tier 0 always, tier 1 if `skill ≥ 1`, tier 2 if `skill ≥ 2`. For the groups at 108–141 the tiers are +36/+72 (Wolf3D layout); for 216–231 and 270–281 they are +18/+36. Game types ≥ 3 therefore get every tier.
- `which 5–8` (disguised soldiers) and `which 12` (red-vest brute) are stand-only; `which 3` (saucer) is patrol-only and its speed comes from `DS:0x9299`, set to 0 by codes 270–273 (hovering in place) and 0x300 by 274–277 (moving). 142/143 and 160/161 spawn the brute with only directions E/N.
- Ambush: `SpawnStand` (not `SpawnPatrol` or the special spawners) on a plane-0 138 cell clears the tile, fixes the area code and sets `FL_AMBUSH` (`2390:059a–06b5`).
- `killtotal++` for every enemy spawn unless a game was loaded (not for allies or fire).

| Codes (tier 0 / 1 / 2) | Spawn | Class |
|---|---|---|
| 108–111 / 144–147 / 180–183 | `SpawnStand(0,…)` | 3 rat |
| 112–115 / 148–151 / 184–187 | `SpawnPatrol(0,…)` | 3 rat |
| 116–119 / 152–155 / 188–191 | `SpawnStand(1,…)` | 4 soldier |
| 120–123 / 156–159 / 192–195 | `SpawnPatrol(1,…)` | 4 soldier |
| 126–129 / 162–165 / 198–201 | `SpawnStand(5,…)` | 8 soldier on a stool |
| 130–133 / 166–169 / 202–205 | `SpawnStand(6,…)` | 9 crouching soldier |
| 134–137 / 170–173 / 206–209 | `SpawnStand(7,…)` | 10 soldier on a toilet |
| 138–141 / 174–177 / 210–213 | `SpawnStand(8,…)` | 11 prone soldier |
| 142–143, 160–161 | `SpawnStand(12,…)` dir 0–1 | 15 red-vest brute |
| 179 | `2390:0f7e(x,y)` | 17 sewer fat man |
| 196 | `2390:0f20(x,y)` | 14 gunner |
| 197 | `2390:103a(x,y)` | 22 manhole fat man (unused) |
| 214 | `2390:0911(x,y)` | 13 trench-coat boss — **his death ends the game** |
| 215 | `2390:1094(x,y)` | 23 swamp creature (ambush flag) |
| 216–219 / 234–237 / 252–255 | `SpawnStand(4,…)` | 7 giant rat |
| 220–223 / 238–241 / 256–259 | `SpawnPatrol(4,…)` | 7 giant rat |
| 224–227 / 242–245 / 260–263 | `SpawnAlly(x,y,dir)` `2634:00e4` | 5 ally |
| 228–231 / 246–249 / 264–267 | nothing | — |
| 232–233, 250–251 | `2390:0fdc(x,y)` | 16 red giant |
| 270–273 / 288–291 / 306–309 | `SpawnPatrol(3,…)`, `DS:0x9299 = 0` | 6 saucer drone, hovering |
| 274–277 / 292–295 / 310–313 | `SpawnPatrol(3,…)`, `DS:0x9299 = 0x300` | 6 saucer drone, moving |
| 278–281 / 296–299 / 314–317 | `SpawnStand(9,…)` | 12 black-suit gunman |
| 282–285 | `SpawnFireActor(x,y)` `30d6:0161` | 21 fire (max 80 objects; random sound 71–73) |

**Values above 255** (257–317) are ordinary actor codes in this scheme (saucers, black-suit gunmen, giant-rat tier 2); nothing treats the high byte specially except the run-time bit 15 (§3.7). CONFIRMED.

### 4.6 See-through and thin cells (103–107)

`ThreeDRefresh` (`1b4a:129c`) calls `AsmRefresh` (`1d2a:007a`) up to three times: `1b4a:12f3` with the hit routines disabled (`DS:0xC282 = 0`, only `spotvis` is filled for the floor pass), `1b4a:1303` for the walls, and after `DrawScaleds` (`1b4a:130c`) a third time with `DS:0xC3D0 = 1` when `DS:0xC3CE > 0` (`1b4a:130f–1322`). LIKELY (structure read, not traced):
- Pass 1: a ray crossing a cell whose plane-1 low byte is **103–107** does not stop there; it marks the cell visible and counts it (`DS:0xC3CE`), and goes on to the wall behind (`1d2a:02bd`, `1d2a:0339`).
- Overlay pass (`DS:0xC3D0 ≠ 0`, `1d2a:0577`), drawn **after the sprites**: rays walk only through cells already marked visible and stop at those cells: **103, 106, 107 are drawn as thin mid-cell walls** through the door routines (non-door path, page `tilehit − 1`); **104, 105 are drawn as full-cell faces** through the wall routines. Magenta texels are skipped (transparent), so the wall behind shows through.
- `CheckLine` (`1e43:1af8`, `1e43:1b2d`, and the ally copy `2634:0581`) lets actor sight pass through cells with plane-1 **105, 106, 107**, but not 103 or 104. CONFIRMED.

| Plane 1 | Typical plane 0 | Reading |
|---|---|---|
| 103 | 69, 29, 49, 65, 13 | opaque door (thin, animated, openable) |
| 104 | 57, 117 (glass), 86, 127, 18, 58 | window: see-through for the player, blocks enemy sight |
| 105 | 47, 86, 117, 127, 9 | grille/fence: see-through for both |
| 106 | 13, 29 | barred/glass door (thin, openable, see-through for both) |
| 107 | 4, 88, 80 | thin always-open opening (pipe mouth, rock hole) |

The visual readings in the last column are HYPOTHESIS; the code behaviour is as stated.

### 4.7 Push walls (98)

- ScanInfoPlane: `secrettotal (DS:0x872A)++` unless loaded (`1823:024e`). CONFIRMED.
- `Cmd_Use` checks plane 1 of the faced cell first: 98 → `PushWall(x, y, dir)` (`208c:10eb`), before doors. CONFIRMED.
- `PushWall` (`2281:0ce2`): returns if a push wall is moving (`DS:0x4916`) or the cell is empty; checks `actorat` of the next cell in `dir` (0 N, 1 E, 2 S, 3 W) and **immediately makes that cell solid with the same tile** (Wolf3D does this in `MovePWalls`); `secretcount (DS:0x8726)++` if the plane-1 value is 98; `pwallx/y/dir` at `DS:0x48E8/0x48E6/0x48E4`, `pwallstate = 1`, `pwallpos = 0`; marks the moving tile with **`| 0x80`** (Wolf3D `| 0xC0`); sound 0x53. **It does not clear the plane-1 marker.** CONFIRMED.
- `MovePWalls` (`2281:0e58`): per block clears the old cell, writes `player->areanumber + 140` into plane 0 (Wolf3D), **clears plane 1 at the old cell and writes 98 at the new one**, so the marker travels with the wall; stops when `pwallstate > 256` (two cells, as Wolf3D). CONFIRMED (partial read); whether the resting wall can be pushed again: LIKELY yes (it keeps plane-1 98).
- Push walls are immune to explosions (§4.8).

### 4.8 Explosions and wall attributes (89, 98, 125)

`Explode` (`30d6:12b1`, called from the projectile code at `2390:02f9` and `2390:03fe`; `functions.tsv` wrongly lists `2281:0e58` as its caller because Ghidra merged bodies; [ACTORS.md](ACTORS.md)/[PLAYER.md](PLAYER.md) own the damage part, which hits actors in a 8×8-tile box around the centre), for each cell within ±2 of the centre and inside 1–62, CONFIRMED:
1. Every static in the cell is removed (`shapenum = −1`) and replaced by rubble, `PlaceStatic(12 or 13, x, y)` (`2281:02a3`; statinfo indices 12/13 = sprites 13/14, the same rubble as plane-1 35/36), chosen with `US_RndT() & 1`.
2. If `tilemap` is 0, nothing else. If the plane-1 value is **89** or **98** the wall is **indestructible** (`30d6:144e`, `30d6:1457`). 89 marks every map border and many interior walls (17,178 cells).
3. Tiles 113 (ladder exit) are never touched; glass 57–60 → 60 and 117–120 → 120 (broken).
4. Centre cell: any animated-wall record there is deleted; if plane 1 is **125** the wall becomes 77 (damaged) and stays solid; otherwise it is **destroyed** (`tilemap = plane1 = actorat = 0`, rubble `PlaceStatic(13)`, and with `US_RndT() % 20 == 0` a `SpawnFireActor`).
5. Other cells: skipped if they hold an animated-wall record; otherwise plane-1 86/87/88 → destroyed (reachable only once the record is gone), 98 → destroyed (never reached, filtered in step 2), 125 → skipped, every other wall → **77** (`tilemap = actorat = 77`, plane 1 cleared) (`30d6:169a–1735`).

So walls are destroyed in two steps (first hit nearby → 77, a centred hit → gone), except 89/98-flagged ones. Code 125 (door-frame flag, damage-only) is unused in the shipped maps.

### 4.9 Patrol turn markers (90–97)

`T_Path`'s `SelectPathDir` (`2390:1ff4`) reads plane 1 at the actor's cell: `code − 90 < 8` → `ob->dir = code − 90` (0 E, 1 NE, 2 N, … 7 SE, Wolf3D `ICONARROWS = 90`). Allies use the same rule (`2634:0ae1`, `2634:0ca5`). Maps use only 90 (E), 92 (N), 94 (W), 96 (S); they sit on floor cells in maps 2–4, 7–11, 13, 15, 25. CONFIRMED.

### 4.10 All plane-1 values present in the 61 maps

Evidence = ScanInfoPlane case address in segment 1823 (from the jump table), or the reader for codes without a case.

| Code | Count | Maps | Meaning | Spawn call / reader | Evidence | Conf. |
|---|---|---|---|---|---|---|
| 0 | 4096 | 60 | empty map (60) | - | `1823:01f2` | CONFIRMED |
| 18 | 214093 | 0-59 | ignored by ScanInfoPlane (background) | - | `1823:01f2` | CONFIRMED |
| 19 | 23 | 2-4,23-39,55,58-59 | player start, facing N | SpawnPlayer 208c:1324 | `1823:020f` | CONFIRMED |
| 20 | 29 | 5-21,40-49,51-52 | player start, facing E | SpawnPlayer 208c:1324 | `1823:020f` | CONFIRMED |
| 21 | 4 | 0-1,22,54 | player start, facing S | SpawnPlayer 208c:1324 | `1823:020f` | CONFIRMED |
| 22 | 4 | 50,53,56-57 | player start, facing W | SpawnPlayer 208c:1324 | `1823:020f` | CONFIRMED |
| 23 | 17 | 6-8,10,14,16,20,26-27,31 | static, sprite 1, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 24 | 144 | 6-7,11,13-17,20,25-26,55-56,59 | static, sprite 2, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 25 | 24 | 6,11,15-17,29,38 | static, sprite 3, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 26 | 226 | 6-11,13,15-16,21,23-24,27,29,32,37,55,58 | static, sprite 4, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 27 | 106 | 6-18,20-21,23-34,36-38,55,58-59 | static, sprite 5, type 1: block | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 28 | 94 | 6-21,23-29,31-32,34-37,52,55-56,58 | static, sprite 6, type 1: block | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 29 | 202 | 6-8,10-14,16-21,23-26,29,31-32,34-38,52,56 | static, sprite 7, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 30 | 46 | 6-9,11-12,16-17,19-21,29,33,35,38,56,59 | static, sprite 8, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 31 | 32 | 24-25,27-29,32,58 | static, sprite 9, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 32 | 78 | 6,8-10,13-16,20,25,31,33,38 | static, sprite 10, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 33 | 11 | 6,8,26,32,58 | static, sprite 11, type 1: block | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 34 | 40 | 8,10-11,20,25,27,33 | static, sprite 12, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 35 | 19 | 38 | static, sprite 13, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 36 | 30 | 38 | static, sprite 14, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 37 | 13 | 38 | static, sprite 15, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 38 | 11 | 8,31-32,58 | static, sprite 16, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 39 | 213 | 5,10-11,13,19,25,35,51,56-57 | static, sprite 17, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 40 | 97 | 2-6,8-12,16,18,21,23,25,28,31,33,36 | static, sprite 18, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 41 | 57 | 8-18,20-21,23-29,31-32,34,36-38,55,58-59 | static, sprite 19, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 42 | 125 | 3,6-21,23-27,29-38,55,58-59 | static, sprite 20, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 43 | 82 | 6-18,20-21,23-25,27-32,36-37,55,58 | static, sprite 21, type 1: block | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 44 | 1 | 28 | static, sprite 22, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 45 | 223 | 6,10-11,14-29,31-33,35-37,39,52,56 | static, sprite 23, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 46 | 338 | 6-8,10-14,17-18,20-21,23-27,29,32-34,37,52 | static, sprite 24, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 47 | 185 | 8-9,11-13,18,21,26,52 | static, sprite 25, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 48 | 30 | 6,14,20-21,23,25-27,29,34,37 | static, sprite 26, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 49 | 108 | 6-8,10,14,16,18,20,24-25,29,31-32,36,38,58 | static, sprite 27, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 50 | 302 | 6-18,20-21,23,25-26,29-30,33-34,36-38,50 | static, sprite 28, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 51 | 68 | 19,22,24,35-36,56 | static, sprite 29, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 52 | 110 | 0-3,6,11,14,16,18,21,23,28-30,36-37,50-55,57-59 | static, sprite 30, type 11: bonus: Uzi | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 53 | 173 | 2-4,6-8,11,14,16-21,23-31,33-34,36-38,50-57,59 | static, sprite 31, type 5: bonus: Galil | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 54 | 73 | 9-39,50-59 | static, sprite 32, type 12: bonus: Flame Thrower | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 55 | 26 | 17,21-39,53,58 | static, sprite 33, type 13: bonus: Grenade Launcher | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 56 | 333 | 2-38,50-52,54-55,57-58 | static, sprite 34, type 3: bonus: ammo +30 | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 57 | 40 | 21-22,25,29,33,37-39,52,55 | static, sprite 35, type 4: bonus: Grenades! | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 58 | 124 | 38-39,50-59 | static, sprite 36, type 14: bonus: Fuel Pack | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 59 | 126 | 0-12,14-16,18-21,23-38,51-59 | static, sprite 37, type 8: bonus: Flak Jacket (+100 armor) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 60 | 438 | 0-39,50-59 | static, sprite 38, type 9: bonus: Increased Health (+100) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 61 | 116 | 7-10,12-15,20-21,23-29,32,38 | static, sprite 39, type 20: bonus: BOOBY TRAP | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 62 | 228 | 5-18,20-21,39 | static, sprite 40, type 20: bonus: BOOBY TRAP | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 63 | 175 | 1-3,7-10,12-15,17,20-21,23-29,32,38 | static, sprite 41, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 64 | 16 | 24-25,29,32-33,36 | static, sprite 42, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 65 | 73 | 1-4 | static, sprite 43, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 66 | 56 | 7-10,12-14,17,21,23-29,32,38 | static, sprite 44, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 67 | 219 | 15,17,19,24,27-29,31,34-36,53 | static, sprite 45, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 68 | 39 | 26-29,31-32,34,36 | static, sprite 46, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 69 | 87 | 9,11-12,16-17,24,26,32,34,58 | static, sprite 47, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 70 | 32 | 14,24-27,29,31-32,36 | static, sprite 48, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 71 | 17 | 23,25,28-29,36-37 | static, sprite 49, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 72 | 36 | 23-25,27,29,31,37,59 | static, sprite 50, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 73 | 37 | 0-2,5-15,17,20,22-23,37,39,56,59 | static, sprite 51, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 74 | 28 | 23,25,27,29,37 | static, sprite 52, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 75 | 222 | 0-4 | static, sprite 53, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 76 | 3 | 36 | static, sprite 54, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 77 | 43 | 2 | static, sprite 55, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 78 | 103 | 0,3-4 | static, sprite 56, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 79 | 123 | 14,23-24,26-28,31-32,36-37,58-59 | static, sprite 57, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 80 | 40 | 17,21,33 | static, sprite 58, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 81 | 41 | 27,31,34,36 | static, sprite 59, type 0: dressing, passable | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 82 | 152 | 30-31,33,37 | static, sprite 60, type 1: block | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 83 | 117 | 19,21,24-25,29,33-34,36 | static, sprite 61, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 84 | 8 | 39 | static, sprite 62, type 17: block (+7=2) | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 85 | 30 | 1-3,6-18,20-21,23-26,28-33,36-37 | static, sprite 63, type 10: bonus: Map ThermoSensor | SpawnStatic 2281:0011 | `1823:0224` | CONFIRMED |
| 86 | 3 | 1,3-4 | animated wall type 3 (looping 4 frames) | SpawnAnimWall 2f4d:0847 | `1823:0239` | CONFIRMED |
| 87 | 18 | 1,3-4,19,35 | animated wall type 4 (looping 4 frames) | SpawnAnimWall 2f4d:0847 | `1823:0239` | CONFIRMED |
| 88 | 8 | 1,3-4 | animated wall type 5 (looping 4 frames) | SpawnAnimWall 2f4d:0847 | `1823:0239` | CONFIRMED |
| 89 | 17178 | 0-39,50-51,53-56,58-59 | explosion-proof wall attribute | none (read by 30d6:12b1) | `30d6:144e` | CONFIRMED |
| 90 | 22 | 2-4,7-11,13,15,25 | patrol turn arrow E | none (read by T_Path 2390:1ff4) | `2390:2018` | CONFIRMED |
| 92 | 22 | 2-4,7-11,13,15,25 | patrol turn arrow N | none (read by T_Path 2390:1ff4) | `2390:2018` | CONFIRMED |
| 94 | 22 | 2-4,7-11,13,15,25 | patrol turn arrow W | none (read by T_Path 2390:1ff4) | `2390:2018` | CONFIRMED |
| 96 | 24 | 2-4,7-11,13,15,25 | patrol turn arrow S | none (read by T_Path 2390:1ff4) | `2390:2018` | CONFIRMED |
| 98 | 225 | 0-18,20-37,39 | push wall (counts secrettotal) | ScanInfoPlane inc DS:0x872A | `1823:024e` | CONFIRMED |
| 103 | 805 | 0-3,5-39 | anim door type 6 (opaque, closed); thin, see-through draw | SpawnAnimWall(...,6) | `1823:0284` | CONFIRMED |
| 104 | 2921 | 4,6-18,20-34,36-39,52,55,57-59 | see-through full-tile wall (2nd pass); blocks CheckLine | none (AsmRefresh) | `1d2a:02bd` | LIKELY |
| 105 | 789 | 1-6,9,11-12,14,16-17,19,21,24,26-27,31-36,53,56,58 | see-through full-tile wall; transparent to CheckLine | none (AsmRefresh, CheckLine) | `1d2a:02bd` | LIKELY |
| 106 | 23 | 0-4,11,13 | anim door type 1 (closed); thin; transparent to CheckLine | SpawnAnimWall(...,1) | `1823:0262` | CONFIRMED |
| 107 | 78 | 0,2-4,27,31,33-34,36 | anim wall type 2 (always open, passable); thin; transparent to CheckLine | SpawnAnimWall(...,2) | `1823:0273` | CONFIRMED |
| 108 | 31 | 0-4,9,23 | rat (cl 3) stand, dir E, all skills | SpawnStand 2390:04ca(0,x,y,0) | `1823:02af` | CONFIRMED |
| 109 | 23 | 1-4,12,27 | rat (cl 3) stand, dir N, all skills | SpawnStand 2390:04ca(0,x,y,1) | `1823:02af` | CONFIRMED |
| 110 | 32 | 1-4,15 | rat (cl 3) stand, dir W, all skills | SpawnStand 2390:04ca(0,x,y,2) | `1823:02af` | CONFIRMED |
| 111 | 21 | 0-4,14,27-29,37 | rat (cl 3) stand, dir S, all skills | SpawnStand 2390:04ca(0,x,y,3) | `1823:02af` | CONFIRMED |
| 112 | 4 | 3-4 | rat (cl 3) patrol, dir E, all skills | SpawnPatrol 2390:0741(0,x,y,0) | `1823:02e0` | CONFIRMED |
| 113 | 3 | 2-3 | rat (cl 3) patrol, dir N, all skills | SpawnPatrol 2390:0741(0,x,y,1) | `1823:02e0` | CONFIRMED |
| 114 | 5 | 2-4 | rat (cl 3) patrol, dir W, all skills | SpawnPatrol 2390:0741(0,x,y,2) | `1823:02e0` | CONFIRMED |
| 115 | 1 | 3 | rat (cl 3) patrol, dir S, all skills | SpawnPatrol 2390:0741(0,x,y,3) | `1823:02e0` | CONFIRMED |
| 116 | 96 | 6-12,14-18,20-22,24,26,28-34,36-37 | soldier (cl 4) stand, dir E, all skills | SpawnStand 2390:04ca(1,x,y,0) | `1823:0311` | CONFIRMED |
| 117 | 157 | 5,7-18,20-21,23-25,27-31,33,36-37 | soldier (cl 4) stand, dir N, all skills | SpawnStand 2390:04ca(1,x,y,1) | `1823:0311` | CONFIRMED |
| 118 | 96 | 6-9,13,15-18,20,22,24,26-34,36-37 | soldier (cl 4) stand, dir W, all skills | SpawnStand 2390:04ca(1,x,y,2) | `1823:0311` | CONFIRMED |
| 119 | 94 | 6-8,10,12-15,17,20-21,23-25,27-34,36-37 | soldier (cl 4) stand, dir S, all skills | SpawnStand 2390:04ca(1,x,y,3) | `1823:0311` | CONFIRMED |
| 120 | 5 | 7-8,13 | soldier (cl 4) patrol, dir E, all skills | SpawnPatrol 2390:0741(1,x,y,0) | `1823:0342` | CONFIRMED |
| 121 | 4 | 8,10,13 | soldier (cl 4) patrol, dir N, all skills | SpawnPatrol 2390:0741(1,x,y,1) | `1823:0342` | CONFIRMED |
| 122 | 7 | 7-8,25 | soldier (cl 4) patrol, dir W, all skills | SpawnPatrol 2390:0741(1,x,y,2) | `1823:0342` | CONFIRMED |
| 123 | 4 | 8,10,13 | soldier (cl 4) patrol, dir S, all skills | SpawnPatrol 2390:0741(1,x,y,3) | `1823:0342` | CONFIRMED |
| 126 | 11 | 16,26,34 | soldier on stool (cl 8) stand, dir E, all skills | SpawnStand 2390:04ca(5,x,y,0) | `1823:0461` | CONFIRMED |
| 127 | 7 | 11-12,32 | soldier on stool (cl 8) stand, dir N, all skills | SpawnStand 2390:04ca(5,x,y,1) | `1823:0461` | CONFIRMED |
| 128 | 3 | 9,17 | soldier on stool (cl 8) stand, dir W, all skills | SpawnStand 2390:04ca(5,x,y,2) | `1823:0461` | CONFIRMED |
| 129 | 4 | 24,32 | soldier on stool (cl 8) stand, dir S, all skills | SpawnStand 2390:04ca(5,x,y,3) | `1823:0461` | CONFIRMED |
| 130 | 3 | 6,8,14 | crouching soldier (cl 9) stand, dir E, all skills | SpawnStand 2390:04ca(6,x,y,0) | `1823:0478` | CONFIRMED |
| 131 | 5 | 13-15,25,32 | crouching soldier (cl 9) stand, dir N, all skills | SpawnStand 2390:04ca(6,x,y,1) | `1823:0478` | CONFIRMED |
| 132 | 6 | 7,15,25,27-28,32 | crouching soldier (cl 9) stand, dir W, all skills | SpawnStand 2390:04ca(6,x,y,2) | `1823:0478` | CONFIRMED |
| 133 | 11 | 14,26-27,32 | crouching soldier (cl 9) stand, dir S, all skills | SpawnStand 2390:04ca(6,x,y,3) | `1823:0478` | CONFIRMED |
| 134 | 8 | 8,11,25,27,29,37-38 | soldier on toilet (cl 10) stand, dir E, all skills | SpawnStand 2390:04ca(7,x,y,0) | `1823:048f` | CONFIRMED |
| 135 | 3 | 17 | soldier on toilet (cl 10) stand, dir N, all skills | SpawnStand 2390:04ca(7,x,y,1) | `1823:048f` | CONFIRMED |
| 136 | 3 | 6-7,14 | soldier on toilet (cl 10) stand, dir W, all skills | SpawnStand 2390:04ca(7,x,y,2) | `1823:048f` | CONFIRMED |
| 137 | 7 | 10,15,23,26 | soldier on toilet (cl 10) stand, dir S, all skills | SpawnStand 2390:04ca(7,x,y,3) | `1823:048f` | CONFIRMED |
| 138 | 9 | 14,17,38 | prone soldier (cl 11) stand, dir E, all skills | SpawnStand 2390:04ca(8,x,y,0) | `1823:04a6` | CONFIRMED |
| 139 | 7 | 6,13,17,38 | prone soldier (cl 11) stand, dir N, all skills | SpawnStand 2390:04ca(8,x,y,1) | `1823:04a6` | CONFIRMED |
| 140 | 5 | 13,15 | prone soldier (cl 11) stand, dir W, all skills | SpawnStand 2390:04ca(8,x,y,2) | `1823:04a6` | CONFIRMED |
| 141 | 11 | 7,9,12,28,38 | prone soldier (cl 11) stand, dir S, all skills | SpawnStand 2390:04ca(8,x,y,3) | `1823:04a6` | CONFIRMED |
| 142 | 42 | 16,20,22,30,34,38-49 | red-vest brute (cl 15) stand, dir E, all skills | SpawnStand 2390:04ca(12,x,y,0) | `1823:0508` | CONFIRMED |
| 143 | 28 | 12,16,20,22,26,30,34,38-49 | red-vest brute (cl 15) stand, dir N, all skills | SpawnStand 2390:04ca(12,x,y,1) | `1823:0508` | CONFIRMED |
| 144 | 28 | 0,2,4,14,23,27,29 | rat (cl 3) stand, dir E, skill>=1 | SpawnStand 2390:04ca(0,x,y,0) | `1823:02a2` | CONFIRMED |
| 145 | 11 | 0-1,4,7,12 | rat (cl 3) stand, dir N, skill>=1 | SpawnStand 2390:04ca(0,x,y,1) | `1823:02a2` | CONFIRMED |
| 146 | 8 | 4,11 | rat (cl 3) stand, dir W, skill>=1 | SpawnStand 2390:04ca(0,x,y,2) | `1823:02a2` | CONFIRMED |
| 147 | 24 | 0-1,3-4,20,25,27-29,37 | rat (cl 3) stand, dir S, skill>=1 | SpawnStand 2390:04ca(0,x,y,3) | `1823:02a2` | CONFIRMED |
| 148 | 3 | 3 | rat (cl 3) patrol, dir E, skill>=1 | SpawnPatrol 2390:0741(0,x,y,0) | `1823:02d3` | CONFIRMED |
| 149 | 2 | 2 | rat (cl 3) patrol, dir N, skill>=1 | SpawnPatrol 2390:0741(0,x,y,1) | `1823:02d3` | CONFIRMED |
| 150 | 1 | 3 | rat (cl 3) patrol, dir W, skill>=1 | SpawnPatrol 2390:0741(0,x,y,2) | `1823:02d3` | CONFIRMED |
| 152 | 35 | 7-10,13-15,18,20,22,24-26,29,32-34,38 | soldier (cl 4) stand, dir E, skill>=1 | SpawnStand 2390:04ca(1,x,y,0) | `1823:0304` | CONFIRMED |
| 153 | 52 | 6-7,9,11-13,16-18,20,25,28-32,36 | soldier (cl 4) stand, dir N, skill>=1 | SpawnStand 2390:04ca(1,x,y,1) | `1823:0304` | CONFIRMED |
| 154 | 23 | 7-10,14,17-18,20-22,25,27,33-34,37 | soldier (cl 4) stand, dir W, skill>=1 | SpawnStand 2390:04ca(1,x,y,2) | `1823:0304` | CONFIRMED |
| 155 | 32 | 6-7,15,18,21,23-24,27-28,32,34,37-38 | soldier (cl 4) stand, dir S, skill>=1 | SpawnStand 2390:04ca(1,x,y,3) | `1823:0304` | CONFIRMED |
| 160 | 75 | 16,22,26,30,34,39-49 | red-vest brute (cl 15) stand, dir E, all skills | SpawnStand 2390:04ca(12,x,y,0) | `1823:051f` | CONFIRMED |
| 161 | 15 | 8,12,20,22,26,30,38-39 | red-vest brute (cl 15) stand, dir N, all skills | SpawnStand 2390:04ca(12,x,y,1) | `1823:051f` | CONFIRMED |
| 174 | 2 | 12,18 | prone soldier (cl 11) stand, dir E, skill>=1 | SpawnStand 2390:04ca(8,x,y,0) | `1823:0440` | CONFIRMED |
| 177 | 7 | 8,18,23-25 | prone soldier (cl 11) stand, dir S, skill>=1 | SpawnStand 2390:04ca(8,x,y,3) | `1823:0440` | CONFIRMED |
| 179 | 4 | 3-4 | sewer fat man (cl 17) | 2390:0f7e(x,y) | `1823:04db` | CONFIRMED |
| 180 | 47 | 0,2,4,6,11,23,25,29,37 | rat (cl 3) stand, dir E, skill>=2 | SpawnStand 2390:04ca(0,x,y,0) | `1823:0295` | CONFIRMED |
| 181 | 22 | 0-2,11-12 | rat (cl 3) stand, dir N, skill>=2 | SpawnStand 2390:04ca(0,x,y,1) | `1823:0295` | CONFIRMED |
| 182 | 11 | 1-2,4,6,9 | rat (cl 3) stand, dir W, skill>=2 | SpawnStand 2390:04ca(0,x,y,2) | `1823:0295` | CONFIRMED |
| 183 | 20 | 1-2,4,6,27-28 | rat (cl 3) stand, dir S, skill>=2 | SpawnStand 2390:04ca(0,x,y,3) | `1823:0295` | CONFIRMED |
| 184 | 2 | 3 | rat (cl 3) patrol, dir E, skill>=2 | SpawnPatrol 2390:0741(0,x,y,0) | `1823:02c6` | CONFIRMED |
| 186 | 2 | 3 | rat (cl 3) patrol, dir W, skill>=2 | SpawnPatrol 2390:0741(0,x,y,2) | `1823:02c6` | CONFIRMED |
| 187 | 1 | 3 | rat (cl 3) patrol, dir S, skill>=2 | SpawnPatrol 2390:0741(0,x,y,3) | `1823:02c6` | CONFIRMED |
| 188 | 22 | 6-7,12,14,18,22-23,25-27,31,33,37 | soldier (cl 4) stand, dir E, skill>=2 | SpawnStand 2390:04ca(1,x,y,0) | `1823:02f7` | CONFIRMED |
| 189 | 56 | 6-7,9-11,13-18,23-29,31-34,36-37 | soldier (cl 4) stand, dir N, skill>=2 | SpawnStand 2390:04ca(1,x,y,1) | `1823:02f7` | CONFIRMED |
| 190 | 14 | 7,11,14,16-17,22,24,27,33 | soldier (cl 4) stand, dir W, skill>=2 | SpawnStand 2390:04ca(1,x,y,2) | `1823:02f7` | CONFIRMED |
| 191 | 28 | 8,10,12-13,17,20,23-25,27-30,34 | soldier (cl 4) stand, dir S, skill>=2 | SpawnStand 2390:04ca(1,x,y,3) | `1823:02f7` | CONFIRMED |
| 196 | 8 | 30,34,38-39 | gunner (cl 14) | 2390:0f20(x,y) | `1823:04ea` | CONFIRMED |
| 199 | 4 | 21 | soldier on stool (cl 8) stand, dir N, skill>=2 | SpawnStand 2390:04ca(5,x,y,1) | `1823:0359` | CONFIRMED |
| 200 | 1 | 21 | soldier on stool (cl 8) stand, dir W, skill>=2 | SpawnStand 2390:04ca(5,x,y,2) | `1823:0359` | CONFIRMED |
| 201 | 1 | 21 | soldier on stool (cl 8) stand, dir S, skill>=2 | SpawnStand 2390:04ca(5,x,y,3) | `1823:0359` | CONFIRMED |
| 202 | 3 | 7,13 | crouching soldier (cl 9) stand, dir E, skill>=2 | SpawnStand 2390:04ca(6,x,y,0) | `1823:037a` | CONFIRMED |
| 204 | 2 | 20 | crouching soldier (cl 9) stand, dir W, skill>=2 | SpawnStand 2390:04ca(6,x,y,2) | `1823:037a` | CONFIRMED |
| 211 | 1 | 7 | prone soldier (cl 11) stand, dir N, skill>=2 | SpawnStand 2390:04ca(8,x,y,1) | `1823:03bc` | CONFIRMED |
| 212 | 2 | 17,21 | prone soldier (cl 11) stand, dir W, skill>=2 | SpawnStand 2390:04ca(8,x,y,2) | `1823:03bc` | CONFIRMED |
| 213 | 8 | 7-8,10,18,20-21 | prone soldier (cl 11) stand, dir S, skill>=2 | SpawnStand 2390:04ca(8,x,y,3) | `1823:03bc` | CONFIRMED |
| 214 | 1 | 39 | trench-coat boss (cl 13); death -> victory | 2390:0911(x,y) | `1823:04bd` | CONFIRMED |
| 215 | 50 | 1-4 | swamp creature (cl 23) | 2390:1094(x,y) | `1823:04cc` | CONFIRMED |
| 216 | 11 | 0,2-4 | giant rat (cl 7) stand, dir E, all skills | SpawnStand 2390:04ca(4,x,y,0) | `1823:056e` | CONFIRMED |
| 217 | 9 | 0,3-4,12,14 | giant rat (cl 7) stand, dir N, all skills | SpawnStand 2390:04ca(4,x,y,1) | `1823:056e` | CONFIRMED |
| 218 | 8 | 0,2-4 | giant rat (cl 7) stand, dir W, all skills | SpawnStand 2390:04ca(4,x,y,2) | `1823:056e` | CONFIRMED |
| 219 | 19 | 0-4 | giant rat (cl 7) stand, dir S, all skills | SpawnStand 2390:04ca(4,x,y,3) | `1823:056e` | CONFIRMED |
| 220 | 1 | 4 | giant rat (cl 7) patrol, dir E, all skills | SpawnPatrol 2390:0741(4,x,y,0) | `1823:059f` | CONFIRMED |
| 221 | 1 | 3 | giant rat (cl 7) patrol, dir N, all skills | SpawnPatrol 2390:0741(4,x,y,1) | `1823:059f` | CONFIRMED |
| 222 | 1 | 4 | giant rat (cl 7) patrol, dir W, all skills | SpawnPatrol 2390:0741(4,x,y,2) | `1823:059f` | CONFIRMED |
| 223 | 2 | 3 | giant rat (cl 7) patrol, dir S, all skills | SpawnPatrol 2390:0741(4,x,y,3) | `1823:059f` | CONFIRMED |
| 224 | 48 | 5-21 | ally (cl 5), dir E, all skills | 2634:00e4(x,y,0) | `1823:05d0` | CONFIRMED |
| 225 | 1 | 8 | ally (cl 5), dir N, all skills | 2634:00e4(x,y,1) | `1823:05d0` | CONFIRMED |
| 227 | 5 | 5,22 | ally (cl 5), dir S, all skills | 2634:00e4(x,y,3) | `1823:05d0` | CONFIRMED |
| 233 | 1 | 22 | red giant (cl 16) | 2390:0fdc(x,y) | `1823:0536` | CONFIRMED |
| 234 | 8 | 0-3 | giant rat (cl 7) stand, dir E, skill>=1 | SpawnStand 2390:04ca(4,x,y,0) | `1823:0561` | CONFIRMED |
| 235 | 5 | 1-2 | giant rat (cl 7) stand, dir N, skill>=1 | SpawnStand 2390:04ca(4,x,y,1) | `1823:0561` | CONFIRMED |
| 236 | 4 | 1-2,4 | giant rat (cl 7) stand, dir W, skill>=1 | SpawnStand 2390:04ca(4,x,y,2) | `1823:0561` | CONFIRMED |
| 237 | 7 | 0-4 | giant rat (cl 7) stand, dir S, skill>=1 | SpawnStand 2390:04ca(4,x,y,3) | `1823:0561` | CONFIRMED |
| 238 | 1 | 3 | giant rat (cl 7) patrol, dir E, skill>=1 | SpawnPatrol 2390:0741(4,x,y,0) | `1823:0592` | CONFIRMED |
| 239 | 1 | 4 | giant rat (cl 7) patrol, dir N, skill>=1 | SpawnPatrol 2390:0741(4,x,y,1) | `1823:0592` | CONFIRMED |
| 240 | 1 | 3 | giant rat (cl 7) patrol, dir W, skill>=1 | SpawnPatrol 2390:0741(4,x,y,2) | `1823:0592` | CONFIRMED |
| 250 | 1 | 39 | red giant (cl 16) | 2390:0fdc(x,y) | `1823:0545` | CONFIRMED |
| 252 | 3 | 0-1,4 | giant rat (cl 7) stand, dir E, skill>=2 | SpawnStand 2390:04ca(4,x,y,0) | `1823:0554` | CONFIRMED |
| 253 | 5 | 0-3 | giant rat (cl 7) stand, dir N, skill>=2 | SpawnStand 2390:04ca(4,x,y,1) | `1823:0554` | CONFIRMED |
| 254 | 3 | 1,3-4 | giant rat (cl 7) stand, dir W, skill>=2 | SpawnStand 2390:04ca(4,x,y,2) | `1823:0554` | CONFIRMED |
| 255 | 5 | 0-1,4 | giant rat (cl 7) stand, dir S, skill>=2 | SpawnStand 2390:04ca(4,x,y,3) | `1823:0554` | CONFIRMED |
| 257 | 2 | 3 | giant rat (cl 7) patrol, dir N, skill>=2 | SpawnPatrol 2390:0741(4,x,y,1) | `1823:0585` | CONFIRMED |
| 259 | 1 | 3 | giant rat (cl 7) patrol, dir S, skill>=2 | SpawnPatrol 2390:0741(4,x,y,3) | `1823:0585` | CONFIRMED |
| 270 | 44 | 5-6,9-13,15,17,19,21,27-30,32,35 | saucer drone (cl 6) patrol hovering, speed 0, dir E, all skills | SpawnPatrol 2390:0741(3,x,y,0) | `1823:061f` | CONFIRMED |
| 271 | 100 | 5-6,9-13,15-19,24,26-29,32-33,35 | saucer drone (cl 6) patrol hovering, speed 0, dir N, all skills | SpawnPatrol 2390:0741(3,x,y,1) | `1823:061f` | CONFIRMED |
| 272 | 40 | 5,13,15-16,19,21,24-29,35,37 | saucer drone (cl 6) patrol hovering, speed 0, dir W, all skills | SpawnPatrol 2390:0741(3,x,y,2) | `1823:061f` | CONFIRMED |
| 273 | 23 | 12-13,15,19,21,23-24,26-30,35 | saucer drone (cl 6) patrol hovering, speed 0, dir S, all skills | SpawnPatrol 2390:0741(3,x,y,3) | `1823:061f` | CONFIRMED |
| 274 | 36 | 6,10-11,13,15-16,24,27-28,32-33,35 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir E, all skills | SpawnPatrol 2390:0741(3,x,y,0) | `1823:064f` | CONFIRMED |
| 275 | 81 | 9-11,13-15,17-19,21,26-29,32-33,35,37 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir N, all skills | SpawnPatrol 2390:0741(3,x,y,1) | `1823:064f` | CONFIRMED |
| 276 | 43 | 6,10-11,13,15,19,21,23-25,29,35,37 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir W, all skills | SpawnPatrol 2390:0741(3,x,y,2) | `1823:064f` | CONFIRMED |
| 277 | 60 | 5-6,10-11,13,19,21,26-28,30,35,37 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir S, all skills | SpawnPatrol 2390:0741(3,x,y,3) | `1823:064f` | CONFIRMED |
| 278 | 88 | 6-8,11-18,20-21,23-26,28-38 | black-suit gunman (cl 12) stand, dir E, all skills | SpawnStand 2390:04ca(9,x,y,0) | `1823:067f` | CONFIRMED |
| 279 | 179 | 6-14,16-18,20-21,23-36,38-39 | black-suit gunman (cl 12) stand, dir N, all skills | SpawnStand 2390:04ca(9,x,y,1) | `1823:067f` | CONFIRMED |
| 280 | 94 | 7-9,12-16,18,20-21,23-26,28-39 | black-suit gunman (cl 12) stand, dir W, all skills | SpawnStand 2390:04ca(9,x,y,2) | `1823:067f` | CONFIRMED |
| 281 | 121 | 6,8-12,14-18,20-34,36-39 | black-suit gunman (cl 12) stand, dir S, all skills | SpawnStand 2390:04ca(9,x,y,3) | `1823:067f` | CONFIRMED |
| 283 | 11 | 38 | fire (cl 21) | SpawnFire 30d6:0161(x,y) | `1823:0697` | CONFIRMED |
| 288 | 30 | 6,10-11,16,23,25-30 | saucer drone (cl 6) patrol hovering, speed 0, dir E, skill>=1 | SpawnPatrol 2390:0741(3,x,y,0) | `1823:0612` | CONFIRMED |
| 289 | 56 | 6,9-13,15,17-18,23-25,27-29,32-33,38 | saucer drone (cl 6) patrol hovering, speed 0, dir N, skill>=1 | SpawnPatrol 2390:0741(3,x,y,1) | `1823:0612` | CONFIRMED |
| 290 | 33 | 5-6,12,15-17,19,21,23-24,26-27,37 | saucer drone (cl 6) patrol hovering, speed 0, dir W, skill>=1 | SpawnPatrol 2390:0741(3,x,y,2) | `1823:0612` | CONFIRMED |
| 291 | 17 | 13,19,24,27,29,32,37 | saucer drone (cl 6) patrol hovering, speed 0, dir S, skill>=1 | SpawnPatrol 2390:0741(3,x,y,3) | `1823:0612` | CONFIRMED |
| 292 | 14 | 9,22 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir E, skill>=1 | SpawnPatrol 2390:0741(3,x,y,0) | `1823:0645` | CONFIRMED |
| 293 | 5 | 14 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir N, skill>=1 | SpawnPatrol 2390:0741(3,x,y,1) | `1823:0645` | CONFIRMED |
| 294 | 2 | 9 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir W, skill>=1 | SpawnPatrol 2390:0741(3,x,y,2) | `1823:0645` | CONFIRMED |
| 296 | 25 | 6,8,10-12,14,16,20-25,28-29,38 | black-suit gunman (cl 12) stand, dir E, skill>=1 | SpawnStand 2390:04ca(9,x,y,0) | `1823:0675` | CONFIRMED |
| 297 | 68 | 6-7,10-18,20-21,23-28,30,36-37,39 | black-suit gunman (cl 12) stand, dir N, skill>=1 | SpawnStand 2390:04ca(9,x,y,1) | `1823:0675` | CONFIRMED |
| 298 | 28 | 7-8,10,12,15,21-22,25-27,29,31,33,37,39 | black-suit gunman (cl 12) stand, dir W, skill>=1 | SpawnStand 2390:04ca(9,x,y,2) | `1823:0675` | CONFIRMED |
| 299 | 23 | 7-8,12,14,18,20,23,29,32-33,36,39 | black-suit gunman (cl 12) stand, dir S, skill>=1 | SpawnStand 2390:04ca(9,x,y,3) | `1823:0675` | CONFIRMED |
| 306 | 43 | 5,13-19,23-26,28,30,32,37-39 | saucer drone (cl 6) patrol hovering, speed 0, dir E, skill>=2 | SpawnPatrol 2390:0741(3,x,y,0) | `1823:0605` | CONFIRMED |
| 307 | 49 | 5-6,9-10,13-14,17-19,21,23,26-29,38-39 | saucer drone (cl 6) patrol hovering, speed 0, dir N, skill>=2 | SpawnPatrol 2390:0741(3,x,y,1) | `1823:0605` | CONFIRMED |
| 308 | 22 | 10,12-13,15-16,18-19,24,26-28,39 | saucer drone (cl 6) patrol hovering, speed 0, dir W, skill>=2 | SpawnPatrol 2390:0741(3,x,y,2) | `1823:0605` | CONFIRMED |
| 309 | 26 | 5-6,11,13,15,25,28-29,32-33,37-38 | saucer drone (cl 6) patrol hovering, speed 0, dir S, skill>=2 | SpawnPatrol 2390:0741(3,x,y,3) | `1823:0605` | CONFIRMED |
| 310 | 6 | 39 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir E, skill>=2 | SpawnPatrol 2390:0741(3,x,y,0) | `1823:063b` | CONFIRMED |
| 312 | 16 | 22,39 | saucer drone (cl 6) patrol moving, speed 0x300+2*rnd, dir W, skill>=2 | SpawnPatrol 2390:0741(3,x,y,2) | `1823:063b` | CONFIRMED |
| 314 | 27 | 6,10,12,14,16,18,20-21,24,26,29,32,36 | black-suit gunman (cl 12) stand, dir E, skill>=2 | SpawnStand 2390:04ca(9,x,y,0) | `1823:066b` | CONFIRMED |
| 315 | 55 | 6-8,10-13,15,17-18,20-21,25,28-29,32,37-39 | black-suit gunman (cl 12) stand, dir N, skill>=2 | SpawnStand 2390:04ca(9,x,y,1) | `1823:066b` | CONFIRMED |
| 316 | 26 | 7,10,12,14-16,18,24,26-27,29,31,36-39 | black-suit gunman (cl 12) stand, dir W, skill>=2 | SpawnStand 2390:04ca(9,x,y,2) | `1823:066b` | CONFIRMED |
| 317 | 34 | 13-14,20,23,25-26,30-33,38-39 | black-suit gunman (cl 12) stand, dir S, skill>=2 | SpawnStand 2390:04ca(9,x,y,3) | `1823:066b` | CONFIRMED |

---

## 5. Per-level tables

### 5.1 Roof and floor colours (`PlayLoop` `1942:13a4–149d`)

CONFIRMED (code and data). Index `i = mapon` for maps 0–49; for `mapon ≥ 50` (net levels) `i = clamp(mapon − 10, 0, 50)`, so **net level slots 50–59 use the CRED entries 40–49**. Tables of 51 bytes:

| DS | Name (debug string in CheckKeys `1942:04ea`) | Copied to | Meaning |
|---|---|---|---|
| `0x09E6` | `Rindex` | `DS:0x09A0` | roof ramp index 0–12 |
| `0x0A19` | `Findex` | `DS:0x09A1` | floor ramp index 0–12 |
| `0x0A4C` | `Rclr` | `DS:0x09A2` | roof colour (palette index) |
| `0x0A7F` | `Fclr` | `DS:0x09A3` | floor colour |

- Ramp table `DS:0x09A4`, 4 bytes per ramp `{first, 0, last, 0}`: ramp *r* = colours `17+16r … 30+16r` (13 ramps, 17–30 … 209–222). From it: `DS:0x9428 = last(Rindex)`, `DS:0x9426 = first(Rindex)`, `DS:0x9427 = first(Findex)`, `DS:0x9425 = last(Findex)`; `DS:0x9424 = (Rclr & 0xF0) + 1`, `DS:0x9423 = (Fclr & 0xF0) + 15`.
- **Floor textures off** (`DS:0x33D2 = 0`): `1b4a:0bc9` fills the ceiling from `last(Rindex)` down to `first(Rindex)` and the floor from `first(Findex)` (after 45 rows) up to `last(Findex)`, stepping one colour every 3 rows, two-colour dithered. **On**: `1b4a:0c72` fills from `Rclr` toward `(Rclr&0xF0)+1` and from `Fclr` toward `(Fclr&0xF0)+15`, one step every 10 rows, then the textured cells (§3.5) are drawn over it. CONFIRMED (decompile read), visual result LIKELY.
- CheckKeys debug keys cycle these four values live (`1942:092e–0a68`). CONFIRMED.

### 5.2 Other per-level values

| What | Where | Values | Confidence |
|---|---|---|---|
| `DS:0x09E2` (long) | `PlayLoop` `1942:13ab` | 0x8400 for maps 0–4, 0x7600 otherwise; read by the floor span drawer (`30d6:1a90`: below 0x8000 the first and last rows use texel 0) | CONFIRMED value, purpose UNKNOWN |
| `DS:0x33B8` | `PlayLoop` `1942:13b7` | 1 for maps 0–4, else 0; selects the distance-shade curve in `30d6:11b6` (darker sewers, LIKELY) | CONFIRMED |
| Floor sets 0–3 | `30d6:1c02` | sewer textures on maps 0–4 (§3.5) | CONFIRMED |
| Music | `StartMusic` `1942:0d2d` | `mapon < 10`: song `DS:0x098A[mapon]` = 3, 5, 1, 2, 4, 7, 9, 8, 6, 0; else **`US_RndT() % 9`** picks one of the first nine; audio chunk = song + 300 | CONFIRMED |
| Max health clamp | `PrepareLevel` | `min(health, {400,400,400,400,800,800}[gametype])` | CONFIRMED |
| Message on arrival | `PlayLoop` `1942:1517` | map 23: `UN BUILDING 2` | CONFIRMED |
| Face-apparition event | `2390:10f2` | only when `mapon > 5`; spawns class 26 in front of the player ([ACTORS.md](ACTORS.md) §5) | LIKELY |
| Par times, level-end tally | — | **none found**: `ex_completed` goes straight to AutoSave, sequence and `mapon++` ([GAMEFLOW.md](GAMEFLOW.md) §5.3) | LIKELY (absence) |

Random music for maps ≥ 10 consumes `US_RndT` at every level start (determinism note for OBCR).

### 5.3 Per-map dump

`T` columns: table index, `Rindex` (ramp colours), `Findex` (ramp colours), `Rclr`, `Fclr`. `9E2/33B8` = §5.2. Floors = plane-0 codes ≥ 140 present. Start = plane-1 19–22 cell and facing. Exits = plane-0 113/114/115 present.

| Map | Name | i | Rindex | Findex | Rclr | Fclr | Song | 9E2/33B8 | Floors | Start | Exits |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | SEWER LEVEL 1 | 0 | 2 (49-62) | 4 (81-94) | 81 | 81 | 3 | 0x8400/1 | 144,145,147 | (58,41) S | 113 |
| 1 | SEWER LEVEL 2 | 1 | 2 (49-62) | 1 (33-46) | 81 | 81 | 5 | 0x8400/1 | 144,145 | (27,25) S | 113 |
| 2 | SEWER LEVEL 3 | 2 | 6 (113-126) | 9 (161-174) | 81 | 179 | 1 | 0x8400/1 | 144,145,146,147 | (61,27) N | 113 |
| 3 | SEWER LEVEL 4 | 3 | 4 (81-94) | 10 (177-190) | 81 | 179 | 2 | 0x8400/1 | 140,144,145,146,147 | (33,46) N | 113 |
| 4 | SEWER LEVEL 5 | 4 | 7 (129-142) | 11 (193-206) | 81 | 179 | 4 | 0x8400/1 | 144,145,146,147 | (4,60) N | 113 |
| 5 | Maintence Level | 5 | 6 (113-126) | 4 (81-94) | 102 | 102 | 7 | 0x7600/0 | 144,150,151 | (3,60) E | 114,115 |
| 6 | UN BLDG1 LOBBY | 6 | 4 (81-94) | 7 (129-142) | 120 | 116 | 9 | 0x7600/0 | 144,145,147,148,149,150,151 | (4,2) E | 114,115 |
| 7 | UN BLDG1 FLR2 | 7 | 4 (81-94) | 7 (129-142) | 120 | 116 | 8 | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,2) E | 114,115 |
| 8 | UN BLDG1 FLR3 | 8 | 4 (81-94) | 0 (17-30) | 119 | 114 | 6 | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,3) E | 114,115 |
| 9 | UN BLDG1 FLR4 | 9 | 4 (81-94) | 4 (81-94) | 120 | 116 | 0 | 0x7600/0 | 144,145,147,148,149,150,151 | (4,3) E | 114,115 |
| 10 | UN BLDG1 FLR5 | 10 | 4 (81-94) | 8 (145-158) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,2) E | 114,115 |
| 11 | UN BLDG1 FLR6 | 11 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,2) E | 114,115 |
| 12 | UN BLDG1 FLR7 | 12 | 4 (81-94) | 1 (33-46) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,3) E | 114,115 |
| 13 | UN BLDG1 FLR8 | 13 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,2) E | 114,115 |
| 14 | UN BLDG1 FLR9 | 14 | 4 (81-94) | 4 (81-94) | 120 | 197 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,2) E | 114,115 |
| 15 | UN BLDG1 FLR10 | 15 | 6 (113-126) | 9 (161-174) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,2) E | 114,115 |
| 16 | UN BLDG1 FLR11 | 16 | 4 (81-94) | 4 (81-94) | 120 | 116 | random | 0x7600/0 | 144,145,147,148,149,150,151 | (4,2) E | 114,115 |
| 17 | UN BLDG1 FLR12 | 17 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 144,145,147,148,150,151 | (4,2) E | 114,115 |
| 18 | UN BLDG1 FLR13 | 18 | 4 (81-94) | 1 (33-46) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,2) E | 114,115 |
| 19 | UN BLDG1 FLR14 | 19 | 6 (113-126) | 4 (81-94) | 126 | 114 | random | 0x7600/0 | 144,145,148,150,151 | (4,2) E | 115 |
| 20 | UN BLDG1 FLR15 | 20 | 4 (81-94) | 4 (81-94) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150 | (4,3) E | 114,115 |
| 21 | UN BLDG1 FLR16 | 21 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (4,3) E | 114,115 |
| 22 | UN BLDG1 TOP | 22 | 6 (113-126) | 4 (81-94) | 120 | 116 | random | 0x7600/0 | 144,145,146,150 | (17,38) S | 114 |
| 23 | BLDG2 FLR1 | 23 | 8 (145-158) | 7 (129-142) | 119 | 114 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (18,59) N | 114 |
| 24 | BLDG2 FLR2 | 24 | 8 (145-158) | 7 (129-142) | 119 | 114 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (18,59) N | 114 |
| 25 | BLDG2 FLR3 | 25 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (20,59) N | 114 |
| 26 | BLDG2 FLR4 | 26 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (18,61) N | 114 |
| 27 | BLDG2 FLR5 | 27 | 6 (113-126) | 8 (145-158) | 119 | 114 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (20,59) N | 114 |
| 28 | BLDG2 FLR6 | 28 | 8 (145-158) | 4 (81-94) | 119 | 117 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (20,59) N | 114 |
| 29 | BLDG2 FLR7 | 29 | 8 (145-158) | 7 (129-142) | 119 | 114 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (20,59) N | 114 |
| 30 | BLDG2 FLR8 | 30 | 4 (81-94) | 3 (65-78) | 102 | 114 | random | 0x7600/0 | 144,145,146,147,148,150,151 | (20,59) N | 114 |
| 31 | BLDG2 FLR9 | 31 | 8 (145-158) | 7 (129-142) | 119 | 35 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (18,59) N | 114 |
| 32 | BLDG2 FLR10 | 32 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 144,145,147,148,149,150,151 | (18,59) N | 114 |
| 33 | BLDG2 FLR11 | 33 | 8 (145-158) | 4 (81-94) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,150,151 | (18,59) N | 114 |
| 34 | BLDG2 FLR12 | 34 | 8 (145-158) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (18,61) N | 114 |
| 35 | BLDG2 FLR13 | 35 | 6 (113-126) | 4 (81-94) | 126 | 114 | random | 0x7600/0 | 144,145,148,150,151 | (29,61) N | 114 |
| 36 | BLDG2 FLR14 | 36 | 4 (81-94) | 9 (161-174) | 119 | 114 | random | 0x7600/0 | 144,145,147,148,149,150,151 | (18,59) N | 114 |
| 37 | BLDG2 FLR15 | 37 | 8 (145-158) | 4 (81-94) | 119 | 69 | random | 0x7600/0 | 144,145,146,147,148,149,150,151 | (18,59) N | 114 |
| 38 | BLDG2 FLR16 | 38 | 7 (129-142) | 12 (209-222) | 119 | 114 | random | 0x7600/0 | 144,146,147,148,149,150,151 | (20,59) N | 114 |
| 39 | BLDG2 TOP | 39 | 6 (113-126) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 144,146,150 | (43,46) N | - |
| 40 | CRED1 | 40 | 8 (145-158) | 4 (81-94) | 119 | 67 | random | 0x7600/0 | 151 | (13,23) E | - |
| 41 | CRED2 | 41 | 8 (145-158) | 7 (129-142) | 119 | 114 | random | 0x7600/0 | 151 | (13,23) E | - |
| 42 | CRED3 | 42 | 8 (145-158) | 7 (129-142) | 119 | 113 | random | 0x7600/0 | 151 | (13,23) E | - |
| 43 | CRED4 | 43 | 8 (145-158) | 7 (129-142) | 119 | 114 | random | 0x7600/0 | 151 | (13,23) E | - |
| 44 | CRED5 | 44 | 8 (145-158) | 4 (81-94) | 119 | 50 | random | 0x7600/0 | 151 | (13,23) E | - |
| 45 | CRED6 | 45 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 151 | (13,23) E | - |
| 46 | CRED7 | 46 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 151 | (13,23) E | - |
| 47 | CRED8 | 47 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 151 | (13,23) E | - |
| 48 | CRED9 | 48 | 8 (145-158) | 4 (81-94) | 119 | 67 | random | 0x7600/0 | 151 | (13,23) E | - |
| 49 | CRED10 | 49 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 151 | (13,23) E | - |
| 50 | NETLEVEL 5 | 40 | 8 (145-158) | 4 (81-94) | 119 | 67 | random | 0x7600/0 | 149 | (42,19) W | 114 |
| 51 | NETLEVEL 8 | 41 | 8 (145-158) | 7 (129-142) | 119 | 114 | random | 0x7600/0 | 148 | (22,47) E | 114 |
| 52 | NETLEVEL 9 | 42 | 8 (145-158) | 7 (129-142) | 119 | 113 | random | 0x7600/0 | 148 | (30,28) E | 115 |
| 53 | NETLEVEL 10 | 43 | 8 (145-158) | 7 (129-142) | 119 | 114 | random | 0x7600/0 | 148 | (51,37) W | 114 |
| 54 | NETLEVEL 6 | 44 | 8 (145-158) | 4 (81-94) | 119 | 50 | random | 0x7600/0 | 146,149 | (36,6) S | 114 |
| 55 | NETLEVEL 7 | 45 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 144,145,147,149 | (31,60) N | 114 |
| 56 | NETLEVEL 4 | 46 | 4 (81-94) | 7 (129-142) | 120 | 116 | random | 0x7600/0 | 144,148,150 | (45,42) W | 115 |
| 57 | NETLEVEL 2 | 47 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 148,151 | (54,47) W | 114 |
| 58 | NETLEVEL 3 | 48 | 8 (145-158) | 4 (81-94) | 119 | 67 | random | 0x7600/0 | 144,145,147,148,149,151 | (18,59) N | 114 |
| 59 | NETLEVEL 1 | 49 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | 144,147,148,149,150,151 | (20,59) N | - |
| 60 | CRED9 | 50 | 8 (145-158) | 4 (81-94) | 119 | 114 | random | 0x7600/0 | - | - | - |

---

## 6. Level progression

Details of the sequence player, AutoSave and high scores are in [GAMEFLOW.md](GAMEFLOW.md) §5–8; this section ties them to map codes. CONFIRMED unless noted.

1. **Linear.** A completed floor (`playstate` 1 `ex_completed`, or 9 `ex_secretlevel`, which shares the same case in `GameLoop`'s table `1823:11e4`) always does `mapon++` (`1823:1041`). There is no next-level table, no secret-level branch and no episode offset: maps 0 → 1 → … → 39. Sewer 1–5 (0–4) → Maintenance (5) → UN BLDG1 lobby (6), floors 2–16 (7–21), top (22) → BLDG2 floors 1–16 (23–38) → BLDG2 top (39).
2. **Leaving a floor** needs an exit wall (§3.3) and Use. `TryExitLevel` (`30d6:1bc0`): if `killcount < killtotal / 2` it shows **`UNFINISHED BUSINESS`** and refuses; **on map 39 it does nothing** (no exit on the last floor). Network games use 113/114 without the kill rule.
3. **The game ends** only when the trench-coat boss (plane-1 214, map 39 only) dies: his death state chain `DS:0x1F8E` → … → `DS:0x1FFE` has think `2390:28d1`, which sets `playstate = 6` (`ex_victorious`) (`KillActor` `1e43:1513`, 1500 points).
4. **Transition sequences**: maps 0–4 → `F2` (ladder), map 22 → `F5` (missing file), other floors → `F3` if the exit was 113/114, `F4` if 115 (`1823:0f9e–100f`). Health +35, AutoSave before `mapon++`.
5. **Episodes.** None in the code; the high-score `episode` field is never written ([GAMEFLOW.md](GAMEFLOW.md) §5.7, §8). The only chapter break is map 22 → 23 (`F5`, then `UN BUILDING 2`). If a "first episode" is wanted for presentation, the natural split is the sewers (0–4); nothing in the code marks it. 
6. **CRED maps (40–49)** are not loaded by any code: the debug warp (`1942:0b4a`) accepts floor numbers 1–40 and 51–60 only (`1942:0bb9–0bcb`), `GameLoop` never steps past 39, and no code compares `mapon` with 40–49. Their only use is as colour-table entries for the net levels (§5.1). Their content: a 151-floor room with one player start (13,23) and red-vest brutes (142/143/160). LIKELY unused (Q-010).
7. **NETLEVELs (50–59)**: network start sets `mapon = 50` (`358f:0d68`); the slot names say NETLEVEL 5, 8, 9, 10, 6, 7, 4, 2, 3, 1 for slots 50–59. Progression inside network games is handled in segment `358f` (not analysed here, Q-019).
8. Slot 60 (`CRED9`, all planes empty) is unused.

---

## 7. Areas and connectivity

- `AREATILE = 140`, `NUMAREAS = 22`, `areaconnect` 22×22 bytes far at `seg 0x3B43:0000` (image seg 0x2B43), `areabyplayer` 22 words at `DS:0x48EA`. CONFIRMED.
- Doors: `DoorOpening`/`DoorClosing` (`2281:0868`, `2281:0a95`) adjust `areaconnect` between the two neighbours of the door (plane 0 − 140, `2281:0968`, `2281:0c29`) and call `ConnectAreas` (Wolf3D): LIKELY (area part read, rest not). Animated walls do the same through `ConnectAnimWallAreas` (§4.4, CONFIRMED). Push walls write the player's area into the cells they vacate (Wolf3D, CONFIRMED).
- `ConnectAreas` (`2281:037b`) = Wolf3D (`Quit(604)` if the player's area ≥ 22); `InitAreas` (`2281:03c8`) = Wolf3D. CONFIRMED.
- Player movement updates `areabyplayer` itself (§3.6). Actors update `areanumber` on each new cell when the plane-0 value is a valid area (`1e43:0cb4`). CONFIRMED.
- Only areas 0 and 4–11 occur. Most maps use 4 (144) as the main area with 5 (145) lamp islands; 148–151 mark other rooms.

---

## 8. Summary of deltas from Wolf3D (candidates for DELTA_SPEC D-004 and new K entries)

- AREATILE 140 (107), AMBUSHTILE 138 (106), doors 130–137 (90–101), 4 door kinds, NUMAREAS 22 (37), plane-1 objects 19–317 with an OBC-specific layout; background 18 instead of 0.
- One texture per wall code (no light/dark pair); door pages by lock 0/1–3/2–4/5 = +0/+4/+5/+7.
- Door locks do nothing; real "doors" are mostly animated walls (plane 1 103/106) that never auto-close and that rats cannot open.
- Textured floors and ceilings per cell from plane-0 codes 144–153 (+ sewer override), and per-level gradient colours.
- Explosions damage and destroy walls; 89 makes walls indestructible.
- 50 % kill rule for exits; victory by killing the map-39 boss.
- Push wall: destination solid at push time, marker 98 travels, moving tile flagged 0x80.
- Game type 3 scatters items and the player start.
- Quirk candidates: neighbour table missing (0,−1) (§3.5); `Thrust` area flip vs. 145 lamp islands (§3.6); Use on a door-look wall (or a tile-88 opening) whose plane-1 value is not 103/106 overwrites that plane-1 value with 0 (`208c:1303`). In the shipped maps those cells hold 18 (on 69) or 107 (on 88), so the overwrite is harmless there; a port must still do it. CONFIRMED (code).

---

## 9. Open items

1. **Oracle checks** (DOSBox-X): lock-0 door face (page 128?), 145 lamp-cell sight effect (§3.6), which half of the floor-set table is the ceiling (§3.5), anim-wall frame timing (per refresh vs per tic), and how the see-through overlay pass (drawn after the sprites, §4.6) handles depth against sprites in front of a window (renderer investigation, [RENDERER.md](RENDERER.md)).
2. `DS:0x09E2` (0x8400 sewers / 0x7600 elsewhere): purpose in `30d6:1a21` (view height? horizon fudge?).
3. Exact distance-shade curve (`30d6:11b6`, floating point) and `DS:0x33B8`'s effect.
4. `2390:10f2` on maps > 5: confirm when PlayLoop triggers the face apparition and whether it consumes RNG at level start.
5. `ConnectAnimWallAreas` on cells whose two sides are not both floor (e.g. 103 on ladder 113, on door cells 132/133): may `Quit(701/702)` or connect odd pairs if ever opened; check that these records are never operated.
6. Tile 85 (Use without area connection) and plane-1 124/125, plane-0 136/137/143/152–161: implemented but unused; keep in the port as written.
7. Statics type 17 vs 1 (byte +7 = 2): which code reads byte +7 (burnable? `IsBurnable` `30d6:0012` checks static flag 0x02 per [ACTORS.md](ACTORS.md)) — belongs to the static/fire investigation.
8. Network-game level flow in segment `358f` (`358f:1720`, `358f:1251`) and how NETLEVEL slots are chosen after 50.
9. Hit points per `which` (`2390:28df`) are in [ACTORS.md](ACTORS.md); not repeated here.
10. Machine-readable code table (`obc-formats::level_codes`) should be generated from §3.8 and §4.10.
