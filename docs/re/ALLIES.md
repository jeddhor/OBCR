# Allies (D-011, Q-017)

Static analysis of `COUNT.EXE` (release OBC-A) from the Ghidra text exports in `.local/ghidra/export/` plus raw disassembly of ranges Ghidra did not cover (jump-table cases, 16-bit `llvm-objdump` over the file bytes). Nothing here has been traced in DOSBox-X yet. Confidence tags follow AGENT_GUIDE §4: "CONFIRMED" below means *read instruction by instruction in the disassembly*; behaviour that also depends on run-time values (pointers, registers, stack contents) is at most LIKELY until a trace confirms it.

Addresses: `ghidra SSSS:OOOO` (base segment 0x1000, image segment = SSSS − 0x1000; `img+0x…` = image-linear). DGROUP data as `DS:0x….` (file offset = DS + 0x35580). Wolf3D references are `WOLFSRC/<file>:<line>`.

Summary of the model:

- There is **one ally class** (`obclass` 5), one sprite set, one state table, no per-ally identity. Allies are placed by map codes 224–227 (plus unused difficulty-tier codes) and start with **400 hit points**.
- A global **ally mode** (`DS:0x2A24`): 0 = Disperse/Attack (the default at every level start), 1 = Follow. `F` toggles it.
- **Follow** is not path finding: the ally runs the generic enemy `T_Chase`, which greedily steps toward the player's tile; the "attack" branch for class 5 is an empty case, so the ally stops when close and never shoots.
- **Disperse/Attack** is a copy of `T_Chase` aimed at the first enemy the ally has a clear line to; it fires through `s_allyshoot`.
- **Switch** (`A`) swaps position, angle, direction and health between the player object and the next living ally (cycling); weapons and ammo stay in `gamestate`. **Switch & Stay** (`S`) also freezes the body you leave.
- The string `CT_Path hit a wall…` is **not** ally code: it is Wolf3D's `T_Path` bounds message (see §10).

---

## 1. How allies come to exist

### 1.1 Map codes (plane 1)

`ScanInfoPlane` (`ghidra 1823:01c8`, switch table at `1823:06c3`, 0x12B entries for codes 0x13–0x13D; the cases lie outside Ghidra's function body) spawns allies with Wolf3D's difficulty-tier fall-through pattern (`WOLFSRC/WL_GAME.C` `ScanInfoPlane`, but with a step of 18 instead of 36):

| Codes | Case address | Condition | Effect |
|---|---|---|---|
| 260–263 | `1823:05b6` | `gamestate.difficulty (DS:0x86FC) ≥ 2`, then code −= 18 | falls into 242–245 |
| 242–245 | `1823:05c3` | `difficulty ≥ 1`, then code −= 18 | falls into 224–227 |
| 224–227 | `1823:05d0` | always | `SpawnAlly(x, y, code − 224)` (`1823:05da`) |

Direction argument 0–3 = east, north, west, south. CONFIRMED (disassembly of the case code and the jump table).

Use in the shipped maps (`obc-tool maps find --plane 1`, CONFIRMED data parse): only the base codes occur; the tier codes 242–263 are unused.

| Map slots | Ally tiles | Notes |
|---|---|---|
| 5 Maintenance Level | (1,62) code 224; (5,39), (2,46) code 227 | 3 allies |
| 6–21 UN BLDG1 Lobby … FLR16 | three tiles in row 2 or 3, columns 1–3 (code 224, facing east); FLR3 has one code 225 | Player start (code 20) at (4,2)/(4,3): the squad lines up beside the player |
| 22 UN BLDG1 TOP | (17,35), (17,36), (17,37) code 227 (south) | Player start at (17,38) |
| 0–4 sewers, 23–39 Building 2, 40–60 credits/net | none | |

So allies exist only on maps 5–22, three per map, re-placed (with full health) on every floor. Nothing carries allies between levels (LIKELY: no code found that saves them).

### 1.2 `SpawnAlly(tilex, tiley, dir)` — `ghidra 2634:00e4`

CONFIRMED (asm, including the jump-table cases at `2634:0190`–`01b1` disassembled by hand):

1. `SpawnNewObj(tilex, tiley, &s_allychase1 /*DS:0x2AE6*/)` (`ghidra 1e43:0007`). OBC's `SpawnNewObj` returns with `new = NULL` when the object list is full (100 objects, `objlist` at `DS:0xA623`, 0x42 bytes each); `SpawnAlly` then does nothing. In game type 3 ("Random", `difficulty == 3`) `SpawnNewObj` relocates every actor, allies included, to a random free tile nearby.
2. `obclass = 5`; `dir = dir*2` (8-way `dirtype`); `speed = 0x400`; `hitpoints = allyhp[difficulty]` (`DS:0x32EC`, see §2.3); `distance = TILEGLOBAL` (copied from `DS:0x0D1A` = 0x00010000).
3. `flags |= FL_SHOOTABLE | FL_AMBUSH` (0x01 | 0x40). If single player (`difficulty < 4`): `flags2(+0x3A) |= 2` (player can walk through, §2.1).
4. `active = 1`; also writes `1` to `new->state->rotate` (the state record itself; it is already 1, so no effect).
5. Wolf3D `SpawnPatrol` tile shift (`WOLFSRC/WL_ACT2.C` `SpawnPatrol`): clear `actorat` at the spawn tile, step `tilex/tiley` one tile in `dir`, **then revert to the spawn tile if `actorat[new tile] < 0x8C`**, and store the ally pointer in `actorat` at the resulting tile. Because an empty tile reads 0, the shift is kept only when the next tile already holds an actor pointer (or a door code ≥ 0x8C). `x`/`y` are not moved. OBC addition; intent UNKNOWN (HYPOTHESIS: meant to reject walls).

Who else calls it: the network code (`ghidra 358f:0eea`, at `358f:11c0`) spawns remote players as class 5 through `SpawnAlly(x, y, 2)` when `DS:0x38FE[player] == 2` (LIKELY: team mates in Network Teams). Network allies are out of scope here; every ally routine below short-circuits when `difficulty ≥ 4`.

### 1.3 Count and limit — `InitAllies`, `ghidra 30d6:0e5b`

Called once per `PlayLoop` entry (`ghidra 1942:1315`, level start and after loading). CONFIRMED (decomp checked against asm):

```
allycount(DS:0x32FC, byte) = 0; allyswitchidx(DS:0x2BD1, byte) = 0;
if single player: count objects after player with obclass==5 && active && !(flags & 0x80)
                  if allycount > 5: Quit with error code 1802 (0x70A)
DrawAllyMode(); FUN_30d6_1c02(); allymode(DS:0x2A24) = 0   // Disperse/Attack
```

Then `PlayLoop` calls `DrawAllyCount()` when single player. Maximum 5 allies per map. Mode resets to Disperse at every level start and load (LIKELY: save files may hold the mode, but it is overwritten here).

### 1.4 Identity, sprites, portrait

- One class, one state table, one sprite set: **no distinct identities**. CONFIRMED (every ally state and spawn path uses class 5 and the same tables).
- Sprites 246–303 (VSWAP sprite index = `shapenum`; checked visually in `.local/exports/vswap/sprites/`): a soldier with a teal helmet and goggles carrying an Uzi-like gun.

| Sprites | Use | Rotated |
|---|---|---|
| 246–253, 254–261, 262–269, 270–277 | walk frames 1–4 × 8 directions | yes |
| 278–284, 285 | death sequence, corpse | no |
| 286, 287 | pain 1, pain 2 | no |
| 288–295 | aim | yes |
| 296–303 | fire (muzzle flash) | yes |

- **HUD**: there is no per-ally face. VGA pics 36–42 (chunks 0x27–0x2D) are a single green soldier portrait slot at status-bar position (1, 0xA8): pic 36 is the empty frame, 37–42 the portrait dissolving. It plays only when an ally dies (§7). Pics 10–19 are digits 0–9; the living-ally count is drawn with them. Pics 25–28 are the mode letters (§2.2). LIKELY (images viewed; draw calls read).

---

## 2. Ally data

### 2.1 `objtype` fields used by allies

OBC's `objtype` is 0x42 bytes and evidently compiled with word alignment (the `flags` byte is followed by a pad, so `distance` sits at +0x0A). Offsets below are CONFIRMED by use in the ally code; names follow Wolf3D (`WOLFSRC/WL_DEF.H` `objstruct`) where the use matches. The general layout belongs in `ACTORS.md`.

| Off | Type | Name | Ally use |
|---|---|---|---|
| +0x00 | int | `active` | 1 at spawn |
| +0x02 | int | `ticcount` | |
| +0x04 | int | `obclass` | **5 = ally**; 1 = player; class 21 (0x15) is also excluded from ally targeting (identity UNKNOWN) |
| +0x06 | near ptr | `state` | |
| +0x08 | byte | `flags` | 0x01 `FL_SHOOTABLE`, 0x08 `FL_VISABLE` (set by the renderer, `ghidra 1b4a:0f69`), 0x10 `FL_ATTACKMODE`, 0x20 `FL_FIRSTATTACK`, 0x40 `FL_AMBUSH`, 0x80 `FL_NONMARK` (set by `KillActor` in single player; used as "dead") |
| +0x0A | long | `distance` | Wolf3D semantics; negative = waiting for door `−distance−1` |
| +0x0E | int | `dir` | 0–7, 8 = `nodir` |
| +0x10 | long | `x` | low word = fraction within the tile, high word (+0x12) = tile |
| +0x14 | long | `y` | likewise (+0x16) |
| +0x18 / +0x1A | int | `tilex` / `tiley` | |
| +0x1C | byte | `areanumber` | copied on switch |
| +0x1E | int | `viewx` | copied on switch (harmless) |
| +0x2A | int | `angle` | allies never update it; copied on switch (§5) |
| +0x2C | int | `hitpoints` | |
| +0x2E | long | `speed` | see §3.4 |
| +0x39 | byte | (OBC) | **bit 0 = STAY** (only ally code touches this byte) |
| +0x3A | byte | `flags2` (OBC) | **bit 1 = the player may pass through** (the player's `TryMove`, `ghidra 208c:0b86`, ignores such actors); set on allies in single player and on several enemy classes |
| +0x3E / +0x40 | near ptr | `next` / `prev` | |

### 2.2 Globals

| DS | Size | Proposed name | Meaning | Confidence |
|---|---|---|---|---|
| 0x2A24 | int | `allymode` | 0 = Disperse/Attack, 1 = Follow. Only ever set to 0 or 1 | CONFIRMED |
| 0x2BC6 | int | `switchstay` | 1 during an `S` switch, 0 during `A` | CONFIRMED |
| 0x2BC8 | int | `allyfiring` | 1 only while an ally's shot is inside `DamageActor`; suppresses score and kill count | CONFIRMED |
| 0x2BCA | int[3] | `allyfiresnds` | {47, 45, 46}: fire sound, chosen by `US_RndT() % 3` | CONFIRMED (data) |
| 0x2BD1 | byte | `allyswitchidx` | position in the switch cycle | CONFIRMED |
| 0x32EC | int[8] | `allyhp` | {400, 400, 400, 400, 800, 800, 800, 400}, indexed by game type | CONFIRMED (data) |
| 0x32FC | byte | `allycount` | living allies; drawn as a digit | CONFIRMED |
| 0x29A0 | int[5] | (death screams) | {22, 23, 24, 25, 26}: ally death sound, `US_RndT() % 5` | CONFIRMED (data) |
| 0x330A / 0x330C / 0x330D | int / byte / byte | ally-death portrait animation: active, frame, delay | §7 | CONFIRMED |
| 0xCDA3 | objtype | `swaptemp` | scratch object for the switch; its `hitpoints` is `DS:0xCDCF` | CONFIRMED |
| 0xCDE5 | objtype* | `allytarget` | **one global** target shared by all allies (§4.4) | CONFIRMED |
| 0x86FC | int | `gamestate.difficulty` | game type: 0–2 difficulty, 3 Random, ≥ 4 network/modem. All ally code requires `< 4` | LIKELY (name from `count_exe_data.csv`; the 1–3/Random/Network/Teams/Modem order follows `README.TXT`) |
| 0x870C | int | `gamestate.health` | clamped to 0–999 | LIKELY |
| 0x0954 | int | debug flag | gates `DebugMessage` and the debug keys | LIKELY |

HUD (`DrawAllyMode`, `ghidra 30d6:08c0`): `StatusDrawPic(0x0E, 0xAA, chunk)` with chunk 0x1F (pic 28, blank) when `allycount == 0`, 0x1E (pic 27, **"D"**) when the mode is 0, 0x1D (pic 26, **"F"**) when the mode is 1, and 0x1C (pic 25, **"A"**) when the mode is 0x1B. That last mode is never set: a vestigial third mode (HYPOTHESIS: "Attack"). `DrawAllyCount` (`ghidra 30d6:097e`, named `DrawWeapon` in `count_exe.csv` by a Wolf3D match; that name is wrong) is `StatusDrawPic(0x10, 0xAB, 13 + allycount)`, the digit pic. CONFIRMED.

### 2.3 State table (`DS:0x2A26`–`0x2BC5`)

`statetype` is 16 bytes: `rotate, shapenum, tictime` (ints), `think`, `action` (far pointers stored as offset and image segment), `next` (near pointer). CONFIRMED (data dump; think and action pointers resolve to the functions named here).

| DS | Proposed name | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| 0x2A26 | `s_allystay1` | 1 | 246 | 25 | — | — | 0x2A36 |
| 0x2A36 | `s_allystay2` | 1 | **247** | 35 | `T_Ally` | — | 0x2A26 |
| 0x2A46–0x2A96 | unreferenced walk loop | 1 | 246, 246, 254, 262, 262, 270 | 5, 5, 15, 20, 5, 15 | — | — | loops to 0x2A46 |
| 0x2AA6 | `s_allypain1` | 0 | 286 | 8 | — | — | 0x2AB6 |
| 0x2AB6 | `s_allypain2` | 0 | 287 | 8 | — | — | 0x2AE6 |
| 0x2AC6 | `s_allyshoot1` | 1 | 288 | 6 | — | `T_AllyShoot` | 0x2AD6 |
| 0x2AD6 | `s_allyshoot2` | 1 | 296 | 20 | — | — | 0x2AE6 |
| 0x2AE6 | `s_allychase1` | 1 | 246 | 8 | `T_Ally` | — | 0x2AF6 |
| 0x2AF6 | `s_allychase1s` | 1 | 246 | 8 | — | — | 0x2B06 |
| 0x2B06 | `s_allychase2` | 1 | 254 | 8 | `T_Ally` | — | 0x2B16 |
| 0x2B16 | `s_allychase2s` | 1 | 262 | 8 | — | — | 0x2B26 |
| 0x2B26 | `s_allychase3` | 1 | 262 | 8 | `T_Ally` | — | 0x2B36 |
| 0x2B36 | `s_allychase3s` | 1 | 270 | 8 | — | — | 0x2AE6 |
| 0x2B46 | `s_allydie1` | 0 | 278 | 7 | — | `A_DeathScream` (`2390:0c59`) | 0x2B56 |
| 0x2B56–0x2BA6 | `s_allydie2`–`7` | 0 | 279–284 | 7 each | — | — | next |
| 0x2BB6 | `s_allydead` | 0 | 285 | 0 | — | — | itself |

Notes:
- `s_allystay2` uses shape 247 with `rotate = 1`, so the eight views draw sprites 247–254: every view is one rotation step off, and the eighth view shows a walk-2 frame. Data CONFIRMED; the visual effect is HYPOTHESIS.
- The chase cycle alternates think and no-think states of 8 tics each. Allies therefore move only during half of the cycle, as Wolf3D guards do during their `…s` states (LIKELY, assuming OBC's `DoActor` keeps Wolf3D's think semantics).
- The death and pain states are entered from `KillActor` and `DamageActor` (§4.5, §7). `0x2A46`–`0x2A96` has no reference in the code or in other states.

---

## 3. Follow mode (`allymode == 1`)

### 3.1 `T_Ally` dispatcher — `ghidra 2634:07b8`

CONFIRMED (asm read in full):

```
T_Ally(ob):
    if ob.flags39 & STAY: return                  // stay: no think at all
    AllyClampSpeed(ob)                            // 2634:0a24
    if difficulty >= 4: return
    if allymode == 1: T_Chase(ob); return         // 2390:1d2d, the generic enemy chase
    ... Disperse/Attack logic (§4.1)
```

### 3.2 What Follow actually does: generic `T_Chase` — `ghidra 2390:1d2d`

This is OBC's shared `T_Chase` (`WOLFSRC/WL_ACT2.C` `T_Chase`), with the ally-relevant behaviour CONFIRMED in the asm:

1. Returns at once in network games.
2. `CheckLine(ob)` toward the **player** (`ghidra 1e43:1982`). If clear: `dist = max(|Δtilex|, |Δtiley|)`; `chance = 300` if `dist == 0`, or if `dist == 1 && distance < 0x4000`; otherwise `chance = (tics << 4) / dist`. If `US_RndT() < chance`, it jumps through the class table at `2390:1fce`. **Class 5's entry is `2390:1fc9`, a bare return**: the ally neither fires nor moves this tic. Otherwise `dodge = 1`.
3. Classes 5 and 6 never dodge: direction choice is always `SelectChaseDir` (`ghidra 1e43:0e75`), Wolf3D's greedy rule toward `player->tilex/tiley` (try the dominant axis, then the other axis, then the old direction, then a random sweep north…west, then turning around).
4. Movement: `move = speed * tics`; doors as in Wolf3D (`OpenDoor`, `ghidra 2281:04f0`, has no lock check, so allies open locked doors, LIKELY); `MoveObj` (`ghidra 1e43:10b1`); snap to the tile centre and choose a new direction at each tile.

Consequences (LIKELY; follow from the code):
- "Find you" is a greedy chase with no path finding. Allies get stuck behind walls between them and you, exactly as Wolf3D guards do.
- "Stay with you": with a clear line of sight within one tile, the chance is 300 > 255, so the ally always halts. At distance `d` it halts with probability ≈ `(16·tics/d)/256` per think call.
- "Too preoccupied to shoot": the class-5 attack case is empty. Follow-mode allies never fire, even at enemies.

### 3.3 Path finding and `CT_Path`

There is **no** dedicated ally path finder. In the Disperse/Attack direction choosers only (§4.2), an ally with no target that stands on a Wolf3D path arrow (plane-1 codes 90–97, `ICONARROWS`) takes the arrow's direction. The `CT_Path…` string belongs to the enemy patrol routine (§10). CONFIRMED.

### 3.4 Speeds, stuck and stall handling

Units are 1/65536 tile per tic (70 tics per second).

| Event | Speed | Where |
|---|---|---|
| Spawn | 0x0400 | `2634:011b` |
| `F` → Disperse | 0x0C00 | `2634:0080` |
| `F` → Follow | 0x1000 | `2634:00c0` |
| Switch (both bodies) | 0x0400 | `CopyActorState`, `2634:1049` |
| `FirstSighting` (first damage while not in attack mode) | × 4 | `1e43:2037` |
| Every `T_Ally` call (not in stay) | clamped to [0x400, 0x1000] | `AllyClampSpeed`, `2634:0a24` |
| Dodge selection | halved, then doubled around `SelectDodgeDirTo` | `2390:2a15` / `2a2f`. `TryWalk` never reads `speed`, so this is a no-op apart from clearing bit 0 |

`AllyClampSpeed` (CONFIRMED): if `speed > 0x1000`, it prints `DebugMessage("Ally stuck throttle")` and sets 0x1000. If `speed < 0x400`, it prints `DebugMessage("Ally stalled")` and sets 0x400. `DebugMessage` (`ghidra 2f4d:12de`) prints only when the debug flag `DS:0x0954` is set; `GameMessage` (`ghidra 2f4d:12b5`) always prints (`vsprintf` into `DS:0x46A6`, timer `DS:0x46A4 = 125`). So the "stuck/stall" handling is only a speed clamp. Its practical job is to undo `FirstSighting`'s ×4: 0x1000 or 0xC00 × 4 is clamped back to 0x1000 before the next move. There is no position-based stuck detection.

For scale (Wolf3D constants): `SPDPATROL` = 512 and an alerted guard moves at 1536. An ally at 0x1000 moves at about 4.4 tiles/s while in a think state, about 2.2 tiles/s on average over the chase cycle. A swapped-out body at 0x400 stays slow until the next `F` or its first hit.

---

## 4. Disperse/Attack mode (`allymode == 0`)

### 4.1 `T_Ally` attack path — `ghidra 2634:07b8` (from `2634:07f3`)

A `T_Chase` clone with a target in place of the player. CONFIRMED (asm):

```
target = NULL; dodge = 0
for o = player->next; o; o = o->next:                      // object-list order
    if o.obclass in {1, 21, 5}: continue                   // player, class 21, allies
    if CheckLineBetween(ob, o) && (o.flags & FL_SHOOTABLE): target = o; break
if target:
    dist = max(|Δtilex|, |Δtiley|)
    chance = (dist==0 || (dist==1 && ob.distance<0x4000)) ? 300 : (tics<<4)/dist
    if US_RndT() < chance: allytarget = target; NewState(ob, s_allyshoot1); return
    dodge = 1
if ob.dir == nodir:
    if dodge: HalveSpeed; SelectDodgeDirTo(ob, target); DoubleSpeed
    else:     SelectChaseDirTo(ob, target)                 // target may be NULL
    if ob.dir == nodir: return
move = speed*tics; loop as Wolf3D T_Chase (door wait, MoveObj, snap, re-choose:
    dodge → SelectDodgeDirTo, else SelectChaseDirTo)
```

Target selection is **the first qualifying actor in object-list order** (spawn order), not the nearest. `CheckLine` is evaluated before the shootable test, so a dead actor with a clear line still costs a ray cast. "Enemy" means any class other than 1, 5 and 21, so allies may fire at any shootable non-ally actor (LIKELY; class roster in `ACTORS.md`).

### 4.2 Direction choosers — `ghidra 2634:0c48` (chase) and `2634:0a79` (dodge)

Wolf3D's `SelectChaseDir` and `SelectDodgeDir` (`WOLFSRC/WL_STATE.C`) with the player replaced by `target`. CONFIRMED differences:
- **No target (NULL)**: if plane 1 under the ally holds a path arrow (90–97), take that direction and `TryWalk`; otherwise use Δ = (+1, +1). Allies with no enemy in sight drift east and south (preferring east), the literal "disperse" (LIKELY).
- Extra last resort before `nodir`: `dir = opposite[player->dir]`, then `TryWalk`.
- Dodge honours `FL_FIRSTATTACK` (one free turnaround) and uses the `diagonal[9][9]` table at `DS:0x0E7A`. `opposite[]` is at `DS:0x0E68` = {4, 5, 6, 7, 0, 1, 2, 3, 8}.
- `TryWalk` is OBC's shared one (`ghidra 1e43:01c3`, about 0xB40 bytes, mostly undiscovered by Ghidra). Living shootable actors block, allies included.

### 4.3 Line of sight — `CheckLineBetween(a, b)`, `ghidra 2634:0412`

Wolf3D `CheckLine` (`WOLFSRC/WL_STATE.C`) generalised to two actors, plus an OBC addition: tiles whose **plane-1** value (`mapsegs[1]`, segment in `DS:0x9411`) is 0x69–0x6B (105–107) are see-through even when `tilemap` is solid. Doors block according to `doorposition`, as in Wolf3D. Actors do not block. CONFIRMED.

**Bug (K candidate), CONFIRMED by instruction read during the port:** the start point uses the **low word** of the first actor's `x`/`y` (the fraction inside its tile) where Wolf3D uses `x >> 8` (global position in 1/256 tiles). The traced ray therefore starts from a point unrelated to the actor's real position and is skewed by about 45° on average. OBCR keeps the bug.

### 4.4 Firing — `T_AllyShoot(ob)`, `ghidra 2634:0214` (action of `s_allyshoot1`)

CONFIRMED (asm, including the hand-disassembled switch at `2634:032c`, table at `2634:0366`):

```
if difficulty >= 4: T_Shoot(ob); return                    // 2390:2506
PlaySoundLocActor(allyfiresnds[US_RndT() % 3], ob)        // even with no target
t = allytarget; if !t: return
if t.obclass == 1 || t.obclass == 5: Quit with error code 1602 (0x642)
if !CheckLineBetween(t, ob): return
dist = max(|Δtilex|, |Δtiley|)
ob.dir = opposite[t.dir]
if US_RndT() < 128:                                        // fixed 50% to hit
    DI = dist<2 ? US_RndT()>>2 : dist<4 ? US_RndT()>>3 : US_RndT()>>4
// else: DI is NOT assigned (see below)
if 8 <= t.obclass <= 11 && !(t.flags & (FL_ATTACKMODE|FL_FIRSTATTACK)): FirstSighting(t)
allyfiring = 1; DamageActor(t, DI*2); allyfiring = 0
```

- **Damage is applied even on a miss.** `DamageActor` is called outside the hit test (unlike `WOLFSRC/WL_ACT2.C` `T_Shoot`), and on a miss `DI` still holds the caller's value. `DoActor` (`ghidra 1942:10f5`) keeps the actor pointer in `DI` (`1942:10fd`) when it calls the state action (`1942:1229`), so a miss deals `(2 × ob) mod 65536`, where `ob` is the ally's own `DS` address in `objlist` (0xA665–0xBFA9): **19,658–32,594 damage, an instant kill**. If the target is not yet in attack mode, `DamageActor` doubles it again. The 16-bit result is then negative (−26,220 to −348), which **heals** the target, and `FirstSighting` wakes it. Classes 8–11 are woken *before* `DamageActor`, so a miss always kills them. Code path: CONFIRMED. In-game effect: LIKELY. Needs a DOSBox-X trace before OBCR relies on it (port impact: reproduce `damage = (ally_ptr_as_DS_offset * 2) as i16`, which requires modelling `objlist` addresses).
- Hit damage is twice Wolf3D's guard damage (0–126).
- Kills and damage by allies give **no score** (`GivePoints`, `ghidra 208c:0614`, returns when `allyfiring`) and **do not count** in the kill ratio (`KillActor`, `1e43:161c`). CONFIRMED.
- `allytarget` is a single global. Another ally deciding to fire during the 6-tic `s_allyshoot1` overwrites it, so an ally can shoot another ally's target (CONFIRMED structure; frequency UNKNOWN). `T_AllyShoot` does not re-check `FL_SHOOTABLE`; a negative-damage "heal" landing on a just-killed target would put a corpse into its pain state (HYPOTHESIS).
- `ob.dir = opposite[t.dir]` changes the movement direction as well as the facing. The next `T_Ally` continues `MoveObj` in that direction without `TryWalk` until the tile is reached (HYPOTHESIS: occasional wall clipping).

### 4.5 Allies being hurt (friendly fire and enemy fire)

`DamageActor(ob, dmg)` (`ghidra 1e43:174d`; the body continues past Ghidra's 213 bytes to `1e43:196d`), CONFIRMED:
- Network: hand-off to `258f:135a`.
- If `gamestate` word `DS:0x8718 == 3`: a 1-in-50 chance (`US_RndT() % 50 == 0`) to `KillActor` outright. This applies to **any** victim, allies included (field meaning UNKNOWN; possibly the current weapon).
- `if !(flags & FL_ATTACKMODE) dmg <<= 1`. Allies are spawned, and reset by every `F`, without `FL_ATTACKMODE`, so **the first hit after spawn or after each `F` deals double damage**.
- `hitpoints -= dmg; if ≤ 0 → KillActor`.
- Not in attack mode → `FirstSighting` (`ghidra 1e43:1e38`). Its class-5 case (`1e43:2024`) does `NewState(s_allychase1)` and `speed *= 4`; the common tail sets `FL_ATTACKMODE | FL_FIRSTATTACK` and `active = 1`.
- Pain (class-5 case `1e43:18fc`): odd `hitpoints` → `s_allypain1`, even → `s_allypain2`.

Sources of damage:
- **Player gunfire hits allies.** `GunAttack` (`ghidra 208c:14e0`) picks the closest shootable, visible actor with no class filter. Class 5 takes the default damage branch (`208c:1755`): weapon- and distance-based, ×8 at difficulty 0, ×2 at 1 and 3, ×1 at 2. CONFIRMED (table at `208c:1822`). Splash and other `DamageActor` callers (`2390:0125`, `2f4d:116e`, `30d6:0089`) contain no class test (LIKELY: allies take splash and fire damage too).
- **Enemy fire.** In single player, the enemy `T_Shoot` (`ghidra 2390:2506`) first calls `EnemyShootAlly` (`ghidra 2390:26b9`). If that returns 1, the player is not shot that time:
  1. `FindNearestVisibleActor(enemy)` (`ghidra 2634:036e`): actors after the player with `obclass ≠ enemy.obclass`, `FL_SHOOTABLE` and **`FL_VISABLE`**. `FL_VISABLE` is set by the renderer, so only actors **currently drawn on the player's screen** qualify.
  2. **Bug**: the "distance" is `|Δx.lo| + |Δy.lo|` using only the **low 16 bits** (the sub-tile fraction) of `x` and `y`, so "nearest" is effectively arbitrary. Actors resting at tile centres all score 0 and the first in list order wins. CONFIRMED (asm `2634:03b3`–`03e0`).
  3. If the result is not an ally, or `CheckLineBetween(ally, enemy)` fails → return 0 (shoot the player normally).
  4. Otherwise: 50% hit chance; damage as Wolf3D (`>>2/>>3/>>4` by distance), **uninitialised (stack garbage) on a miss**; the class rules mirror `T_Shoot`: class 3 does 5 at `dist < 2` only; class 7 applies the damage only at `dist < 2`; class 21 does 75 at `dist 0` and 5 at `dist 1`; all other classes apply the damage at any range. Returns 1.

  Enemies never *chase* allies: `SelectChaseDir` and `SightPlayer` still target the player. Allies wake enemies only by damaging them (LIKELY).
- Allies do not block the player (`flags2` bit 1) but do block enemies and other allies (`TryWalk`).

---

## 5. Switch (`A`) and Switch & Stay (`S`)

### 5.1 Key handling — `CheckKeys`, `ghidra 1942:04ea`

Raw scan codes from `Keyboard[]` (`DS:0x8F22`). They are hard-wired and not part of the configurable controls. They are only read when not paused (`DS:0x8F20 == 0`) and not with Alt+Shift. Checked in this order within one frame: `H`, `C`, then `F`, `A`, `S`, `W`; the first one found returns. CONFIRMED.

| Key | Scan | Condition | Action | Messages |
|---|---|---|---|---|
| `F` | 0x21 | — (the function checks `< 4`) | clear key; `ToggleAllyMode()` | `DISPERSE AND ATTACK` (`DS:0x2BD2`) / `FOLLOW PLAYER` (`DS:0x2BE6`) |
| `A` | 0x1E | `difficulty < 4` | clear key; `switchstay = 0`; `SwitchAlly()` → 0: `NO ALLIES!` (`DS:0x0AE6`); 1: `SWITCH` (`DS:0x0ADF`) and `ClearScreenSplats()` (`ghidra 30d6:10c5`) | |
| `S` | 0x1F | `difficulty < 4` | clear key; `switchstay = 1`; `SwitchAlly()` → 0: `NO ALLIES!` and `switchstay = 0`; 1: `STAYPUT & SWITCH` (`DS:0x0AF1`) | |

`ClearScreenSplats` zeroes the on-screen blood-splat overlay (`DS:0x33B2` count, `DS:0x0D42` active; the splats are added by `ghidra 30d6:0fbe` from the attack code; LIKELY). `S` does **not** clear them: an inconsistency between `A` and `S` (CONFIRMED).

### 5.2 `ToggleAllyMode()` — `ghidra 2634:000e`

CONFIRMED:

```
if difficulty >= 4: return
allymode = !allymode
for each o after player with obclass==5 && (flags & FL_SHOOTABLE):
    NewState(o, s_allychase1)                 // cancels shooting, pain and stay animations
    o.flags = FL_SHOOTABLE | FL_AMBUSH        // clears ATTACKMODE, FIRSTATTACK, VISABLE
    o.flags2 |= 2
    if allymode == 0: o.speed = 0x0C00; GameMessage("DISPERSE AND ATTACK")
    else:  CheckSight(o) /* result ignored, no side effects */; o.flags39 &= ~STAY
           o.speed = 0x1000; GameMessage("FOLLOW PLAYER")
DrawAllyMode()
```

The message is issued once per living ally (same text). With no living allies the mode still flips silently and the HUD shows the blank pic. Only a toggle **into Follow** clears STAY. A stay ally toggled into Disperse gets the chase state, but `T_Ally` returns at once, so it walks in place (HYPOTHESIS for the visual).

### 5.3 `SwitchAlly()` — `ghidra 2634:0e35`

CONFIRMED:

```
if difficulty >= 4: return 0
if allyswitchidx == allycount: allyswitchidx = 0
if allycount == 1: allyswitchidx = 0
k = 0; walk o from player->next counting living allies
    (obclass==5 && FL_SHOOTABLE && !(flags & 0x80)); stop at first with ++k > allyswitchidx
if none found:                                    // wrapped past the last one
    allyswitchidx = 0; o = first living ally; if none: return 0   // "NO ALLIES!"
    DebugMessage("BAD SWATCH"); k = 1
else if debug flag: FUN_2634_0fde(o)              // empty stub
CopyActorState(&swaptemp, o)
CopyActorState(o, player)
CopyActorState(player, &swaptemp)
if switchstay: o.flags39 |= STAY; NewState(o, s_allystay1)   // done twice, redundant
gamestate.health = swaptemp.hitpoints
DrawHealth()                                      // 30d6:0b4a (LIKELY name)
allyswitchidx = k
return 1
```

Repeated `A` presses therefore cycle through the living allies in object-list order (spawn order: rows top to bottom, columns left to right).

### 5.4 What transfers — `CopyActorState(dst, src)`, `ghidra 2634:0fe3`

CONFIRMED. Copied fields: `dir`, `x`, `y`, `tilex`, `tiley`, `areanumber`, `viewx`, `angle`, and `hitpoints`. For `hitpoints`, if `src` is the player (`obclass == 1`) the value comes from `gamestate.health`. `dst.hitpoints` is capped at 999. Then `dst.speed = 0x400` and `dst.flags39 &= ~STAY`.

Not copied: `obclass`, `state`, `flags`, `flags2`, `distance`, `ticcount`, `active`, `temp*`, list links. Nothing in `gamestate` changes except `health`.

| Item | Player after the switch | Ally object (the body you leave) |
|---|---|---|
| Position, tile, area | the ally's | yours |
| View angle | the ally's `angle`. Allies never update `angle`: it is 0 (east) from `GetNewActor`'s zero fill, or the angle you had when you last left that body | yours (unused while an ally) |
| `dir` | the ally's | yours. Whether player movement maintains `player->dir` is UNKNOWN (HYPOTHESIS: it does not, so the left body keeps a stale direction until it re-chooses) |
| Health | `gamestate.health = ally.hitpoints` (≤ 999) | your old `gamestate.health` |
| Weapons, ammo, keys, score, items | unchanged (still in `gamestate`) | — |
| Speed | — | 0x400 (slow) until `F` or its first hit |
| State, flags | player state unchanged | ally keeps its current state and flags (it may be mid-shot or mid-pain); with `S`: STAY set and `s_allystay1` |
| `distance` | — | **not reset**: the left body continues its current move step from your (uncentred) position until the step completes and it snaps to the tile centre (LIKELY) |

Camera and HUD: the view is drawn from `player`, so it jumps to the new body the next frame with no fade or animation (LIKELY: no transition call in `SwitchAlly`). `DrawHealth` runs immediately; the status face follows the new health (LIKELY). The mode letter and ally count do not change. `actorat` is not touched; `DoActor` re-marks the ally on its next tick. `InitAreas` and `areabyplayer` are **not** recomputed (HYPOTHESIS for the consequence: enemies in an area not yet connected to the new position may stay inactive until a door event; UNKNOWN whether OBC refreshes `areabyplayer` elsewhere).

### 5.5 The stay state

- STAY = `flags39` bit 0. Set only by `S`; cleared by `CopyActorState` (when you switch into that body) and by `F` into Follow. CONFIRMED.
- While STAY is set, `T_Ally` returns on its first instruction: **no movement, no speed clamp, no target search, no firing**. The stay ally cycles `s_allystay1/2` and can still be damaged (pain → `s_allychase1` → still inert), shot by enemies (it is a candidate in `FindNearestVisibleActor`) and killed. CONFIRMED in code.
- Consequence (LIKELY, contradicts the manual): a stay ally does **not** "ambush" anything by itself. It acts as a stationary decoy that enemies may shoot instead of you. Verify in DOSBox-X: an enemy walking past a stay ally should not take fire.

---

## 6. `NO ALLIES!`

`SwitchAlly` returns 0, and `CheckKeys` shows `NO ALLIES!`, when the game is a network game (`difficulty ≥ 4`; in practice `CheckKeys` already ignores `A`/`S` there) or when no object has `obclass == 5`, `FL_SHOOTABLE` and `!(flags & 0x80)`. That covers maps without allies (sewers, Building 2) and maps where every ally is dead. CONFIRMED.

---

## 7. Ally death and game over

`KillActor` (`ghidra 1e43:1377`), class-5 case `1e43:142a` (outside Ghidra's function body), CONFIRMED:

- Network: `GivePoints(1, 0)` (`ghidra 208c:0614`, which returns immediately when `difficulty ≥ 4`, so it is a no-op) and `NewState(s_allydie1)`.
- Single player:
  - `DS:0x330A = 1` and `DS:0x330C = 0`: starts the portrait animation.
  - `allycount--`, then `DrawAllyCount()`.
  - `NewState(s_allydie1)` and `flags &= ~FL_SHOOTABLE`.
  - `PlaceItemType(8, tilex, tiley)` (`ghidra 1e43:12e6`): a **Flak Jacket** (statinfo entry 36, sprite 37; LIKELY from the sprite) is dropped on the ally's tile if its `actorat` is empty, otherwise on the first empty tile of the surrounding 3×3 (x outer, y inner). If no tile is free, nothing drops. Single player only.
- Common tail (`1e43:1615`): the kill count is **not** incremented for class 5 or for kills with `allyfiring` set. Then `flags &= ~FL_SHOOTABLE`, `actorat` is cleared, and `flags |= FL_NONMARK (0x80)` in single player.
- Death scream (`A_DeathScream`, `ghidra 2390:0c59`, class-5 case `2390:0d74`): `PlaySoundLocActor({22, 23, 24, 25, 26}[US_RndT() % 5])`.
- Portrait animation (`ghidra 30d6:0923`, called from the refresh at `ghidra 1b4a:129c` while `DS:0x330A`): `StatusDrawPic(1, 0xA8, 0x28 + frame)` for frames 0–5 (pics 37–42), each held for 3 refreshes (`DS:0x330D` countdown from 2). Then it draws chunk 0x27 (pic 36, empty) and clears the flag.

Game over: **no** ally-related loss condition was found. Player death is unchanged (`gamestate.health ≤ 0` → `playstate = ex_died`, `ghidra 208c:0494`), with no automatic switch to a surviving ally (LIKELY; `TakeDamage` decompiles badly but contains no ally references). Losing every ally only empties the HUD letter and makes `A`/`S` say `NO ALLIES!`. Victory is "kill Victor" (`README.TXT`).

---

## 8. Function index (proposed names)

| Ghidra | Proposed name | Kind | Confidence |
|---|---|---|---|
| `2634:000e` | `ToggleAllyMode` | `F` key | CONFIRMED |
| `2634:00e4` | `SpawnAlly(tilex, tiley, dir)` | spawn | CONFIRMED |
| `2634:0209` | `SetAllyTarget(ob)` | stores `DS:0xCDE5` | CONFIRMED |
| `2634:0214` | `T_AllyShoot(ob)` | state action | CONFIRMED |
| `2634:036e` | `FindNearestVisibleActor(ob)` | enemy targeting helper | CONFIRMED |
| `2634:0412` | `CheckLineBetween(a, b)` | ray cast | LIKELY (decomp read; matches Wolf3D `CheckLine`) |
| `2634:07b8` | `T_Ally(ob)` | state think | CONFIRMED |
| `2634:0a24` | `AllyClampSpeed(ob)` | | CONFIRMED |
| `2634:0a79` | `SelectDodgeDirTo(ob, target)` | | LIKELY (decomp) |
| `2634:0c48` | `SelectChaseDirTo(ob, target)` | | LIKELY (decomp) |
| `2634:0e35` | `SwitchAlly()` | `A`/`S` | CONFIRMED |
| `2634:0fde` | (empty debug stub) | | CONFIRMED |
| `2634:0fe3` | `CopyActorState(dst, src)` | | CONFIRMED |
| `2390:1d2d` | `T_Chase` (shared) | Follow mode | CONFIRMED (ally path) |
| `2390:2506` | `T_Shoot` (shared enemy) | | LIKELY |
| `2390:26b9` | `EnemyShootAlly(ob)` | | CONFIRMED |
| `2390:2a15` / `2a2f` | `HalveSpeed` / `DoubleSpeed` | | CONFIRMED |
| `2390:0c59` | `A_DeathScream` | | LIKELY |
| `30d6:0e5b` | `InitAllies` | | CONFIRMED |
| `30d6:08c0` | `DrawAllyMode` | HUD | CONFIRMED |
| `30d6:097e` | `DrawAllyCount` (not `DrawWeapon`) | HUD | CONFIRMED |
| `30d6:0923` | `AnimAllyDeathPortrait` | HUD | CONFIRMED |
| `30d6:10c5` | `ClearScreenSplats` | | LIKELY |
| `2f4d:12b5` / `2f4d:12de` | `GameMessage` / `DebugMessage` | | CONFIRMED |
| `1497:11f8` | `QuitCode(n)` (`itoa` + `Quit`; named `maybe_US_PrintUnsigned` in the CSV) | | CONFIRMED |
| `1e43:0007`, `01c3`, `0d05`, `0e75`, `10b1`, `12e6`, `1377`, `174d`, `1982`, `1e38` | `SpawnNewObj`, `TryWalk`, `SelectDodgeDir`, `SelectChaseDir`, `MoveObj`, `PlaceItemType`, `KillActor`, `DamageActor`, `CheckLine`, `FirstSighting` | WL_STATE | LIKELY (structure and Wolf3D match read) |
| `1942:0c3b` | `GetNewActor` (returns NULL when full; no `Quit`) | | CONFIRMED |
| `1823:01c8` | `ScanInfoPlane` | | LIKELY |

Error codes passed to `QuitCode`: 1602 (`T_AllyShoot` target is the player or an ally), 1802 (more than 5 allies).

---

## 9. Port notes (`obc-sim::allies`, M9)

- Mirror the global `allytarget`, the first-in-list target choice, the low-word "nearest" bug, the miss-damage behaviour (once traced) and the double damage on the first hit. Each changes the RNG call order or the outcomes. RNG calls per `T_AllyShoot`: 1 (sound), 1 (hit roll), +1 on a hit.
- `T_AllyShoot` plays its sound before checking the target; `T_Ally` calls `CheckLine` for every candidate up to the first hit.
- The `S`/`A` splat difference, `CheckSight`'s ignored call and the doubled `NewState` in `SwitchAlly` are harmless but cheap to mirror.
- Stack-garbage damage in `EnemyShootAlly` cannot be reproduced without a trace; it needs a `// HYPOTHESIS(Q-017)` marker.

## 10. Corrections for other documents (not edited here)

- STRINGS.md / D-011: the string at `DS:0x29FF` is `C` + `T_Path hit a wall at %u,%u, dir %u`. The real string starts at `DS:0x2A00` and is Wolf3D's `T_Path` bounds message (`WOLFSRC/WL_ACT2.C:3404`), pushed at `ghidra 2390:2151` inside the shared `T_Path` (`ghidra 2390:2047`). It has nothing to do with allies. D-011's "path following (`CT_Path`)" should read "greedy `T_Chase` toward the player".
- `re/symbols/count_exe.csv`: `30d6:097e DrawWeapon` is `DrawAllyCount`; `1497:11f8 maybe_US_PrintUnsigned` is an error-code `Quit`.
- The stuck and stall "throttles" are debug-only messages around a speed clamp.

## 11. Open questions (for Q-017)

1. Trace `T_AllyShoot` on a miss: confirm `DI` = the ally pointer and the resulting kill or heal (breakpoint at `2634:0345`, read `DI` and the target's `hitpoints`).
2. Trace `EnemyShootAlly` misses: what the uninitialised `[bp-8]` holds in practice.
3. Confirm in play that stay allies never fire, and that Disperse allies with no target drift east and south.
4. Identity of classes 8–11 (woken before damage) and 21 (excluded from targeting); the meaning of `DS:0x8718 == 3` in `DamageActor`.
5. Whether a stale `areabyplayer` after a switch matters, and whether OBC refreshes it elsewhere.
6. `FUN_30d6_1c02` (called by `InitAllies`) failed to decompile; its role is UNKNOWN.
7. Save and load: whether `allymode` and `flags39` are saved (the mode is reset by `InitAllies` anyway).
