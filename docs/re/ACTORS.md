# Actors, state tables and enemy AI

Static analysis of `COUNT.EXE` (release `OBC-A`). Sources: the Ghidra text exports in `.local/ghidra/export/`, plus raw 16-bit disassembly of the executable (`llvm-objdump --triple=i386-unknown-unknown-code16` on byte ranges cut from the file; Borland 8087-emulator interrupts `INT 34h`–`3Dh` decoded by hand). Ghidra missed many functions in segments `1823`, `1e43` and `2390` (they are reached only through jump tables or far pointers in state records), so most addresses below were read from that raw disassembly. Nothing here has been traced in DOSBox-X yet.

Conventions: Ghidra base segment 0x1000, DGROUP = `4218` (SYMBOLS.md §1). Code is cited as `ghidra SSSS:OOOO`; DGROUP data as `DS:0xNNNN` (file offset = DS + 0x35580). Wolf3D source is `WOLFSRC/<file>`. Confidence tags per AGENT_GUIDE §4: `CONFIRMED` here means "instructions or data bytes read one by one", not "observed at run time".

Allies (class 5, segment `2634`) belong to another investigation (Q-017). They are listed only where enemy code touches them.

---

## 1. Key facts

| Fact | Value | Confidence | Evidence |
|---|---|---|---|
| `objtype` size | 0x42 (66) bytes, word-aligned (Wolf3D: 0x3C) | CONFIRMED | `ghidra 1942:0be7` InitActorList (`imul 0x42`), `1942:0c63` GetNewActor (`memset(new,0,0x42)`) |
| `MAXACTORS` | 100 (Wolf3D: 150) | CONFIRMED | `ghidra 1942:0c10` (`cmp si,0x64`) |
| `objlist` | `DS:0xA623` .. `DS:0xC04A` | CONFIRMED | `ghidra 1942:0bee`, `1942:0c1b` |
| `objfreelist` / `lastobj` / `objcount` | `DS:0xA61D` / `DS:0xA61F` / `DS:0x9421` | CONFIRMED | `ghidra 1942:0c1b`–`0c27` |
| `player` / `new` | `DS:0x86F6` / `DS:0x86F8` | CONFIRMED | `ghidra 1942:0c32`–`0c35` |
| `statetype` | 16 bytes, same layout as Wolf3D; all records live in DGROUP | CONFIRMED | DoActor `ghidra 1942:119a`–`1272`; relocation entries at record+8 and record+0xC |
| Enemy/effect state records | `DS:0x12EE`–`DS:0x296E` (361 records); ally records `DS:0x2A26`–`DS:0x2BB6` (26) | CONFIRMED | MZ relocation table + chain walk (§11) |
| `shapenum` | 0-based index into the sprite list (`.local/exports/vswap/sprites/NNN.png`) | CONFIRMED | Every state's frames match the exported images (rats at 64, saucer at 304, fire at 624, …) |
| `AREATILE` | 0x8C (140); `AMBUSHTILE` 0x8A (138); area numbers 0..21 | CONFIRMED | `ghidra 1e43:019e` (`sub al,-0x74`), `2390:059a`, `1e43:0c7e`–`0c8e` |
| `ICONARROWS` | 0x5A (90), same as Wolf3D | CONFIRMED | `ghidra 2390:2018` SelectPathDir |
| `gamestate.difficulty` | `DS:0x86FC`; 0..2 gate spawns, 3 = scatter mode (§6.6), ≥4 = network modes | CONFIRMED (gating), HYPOTHESIS (meaning of 3 and ≥4) | ScanInfoPlane `ghidra 1823:0295`…; network segment `358f` tests 5 and 6 |
| `playstate` | `DS:0x86FA` (2 = died, 6 = victorious) | CONFIRMED | `ghidra 208c:05da`, `2390:28d7` |
| `madenoise` | `DS:0xBFEB`; set only by the player gun attack `ghidra 208c:14ee`, cleared per frame `1942:15d7` | CONFIRMED | grep of all segments |

---

## 2. `objtype` layout (OBC)

Word alignment (Borland `-a`), unlike the byte-packed layout one would get from `WL_DEF.H` without it. Wolf3D's compiled layout is also word-aligned (its `next`/`prev` sit at 0x38/0x3A; `export-wolf` `_GetNewActor`), so OBC = Wolf3D + three words inserted before `next`.

| Off | Size | Field | Notes | Confidence |
|---|---|---|---|---|
| 0x00 | 2 | `active` | 0 = no, 1 = yes (`ac_yes`) | CONFIRMED |
| 0x02 | 2 | `ticcount` | | CONFIRMED |
| 0x04 | 2 | `obclass` | §5 | CONFIRMED |
| 0x06 | 2 | `state` | near pointer into DGROUP | CONFIRMED |
| 0x08 | 1 | `flags` | Wolf3D bits: 0x01 SHOOTABLE, 0x02 BONUS, 0x04 NEVERMARK, 0x08 VISABLE, 0x10 ATTACKMODE, 0x20 FIRSTATTACK, 0x40 AMBUSH, 0x80 NONMARK. Byte 0x09 is padding. | CONFIRMED (uses match Wolf3D) |
| 0x0A | 4 | `distance` | long; negative = waiting for door `-distance-1` | CONFIRMED |
| 0x0E | 2 | `dir` | `dirtype`, 8 = `nodir` | CONFIRMED |
| 0x10 | 4 | `x` | 16.16 fixed | CONFIRMED |
| 0x14 | 4 | `y` | | CONFIRMED |
| 0x18 | 2 | `tilex` | | CONFIRMED |
| 0x1A | 2 | `tiley` | | CONFIRMED |
| 0x1C | 1 | `areanumber` | byte 0x1D padding | CONFIRMED |
| 0x1E | 2 | `viewx` | renderer | CONFIRMED (`ghidra 1b4a:0f21`) |
| 0x20 | 2 | `viewheight` | | CONFIRMED |
| 0x22 | 4 | `transx` | | CONFIRMED (`ghidra 1b4a:0117`–`0126`) |
| 0x26 | 4 | `transy` | | CONFIRMED |
| 0x2A | 2 | `angle` | projectiles; player facing | CONFIRMED |
| 0x2C | 2 | `hitpoints` | | CONFIRMED |
| 0x2E | 4 | `speed` | long | CONFIRMED |
| 0x32 | 2 | `temp1` | renderer uses it when `shapenum == -1` (`ghidra 1b4a:0f36`); network code copies it | CONFIRMED |
| 0x34 | 2 | `temp2` | SightPlayer reaction countdown; in network games also the owner/team id compared by ProjectileTryMove | CONFIRMED |
| 0x36 | 2 | `temp3` | second SightPlayer countdown; written only by network code (`358f`) | CONFIRMED |
| 0x38 | 2 | **OBC** `flags3`? | bit 0 set/cleared only by two unreferenced functions (`ghidra 2390:13df`, `2390:13ed`) | CONFIRMED (no live use found) |
| 0x3A | 1(+1) | **OBC** `flags2` | bit 0x02 = actor does not block the player (`ghidra 208c:0b86`, player TryMove). Set for rats, saucers, swamp creature, allies; toggled by the swamp creature | CONFIRMED |
| 0x3C | 2 | **OBC** `target` | `objtype*` a projectile damages on contact (player, an enemy, or 0 = none) | CONFIRMED |
| 0x3E | 2 | `next` | | CONFIRMED |
| 0x40 | 2 | `prev` | | CONFIRMED |

`areanumber` is still a byte; `TryWalk` only stores it if `0 <= tile-0x8C < 0x16` (§7.3).

## 3. `statetype`

```c
typedef struct statestruct {      // 16 bytes, DGROUP
    int  rotate;    // +0  0, 1 (8 rotations) or 2 (2-rotation pain frame: +0 or +4)
    int  shapenum;  // +2  sprite index
    int  tictime;   // +4
    void (far *think)(objtype*);   // +6
    void (far *action)(objtype*);  // +A
    struct statestruct *next;      // +E  (NULL ends the actor)
} statetype;
```

CONFIRMED: DoActor reads `+4`, `+6/+8`, `+A/+C`, `+E` (`ghidra 1942:119a`–`1272`); CalcRotate reads `+0 == 2` and returns `4*(angle/180)` exactly like Wolf3D (`ghidra 1b4a:0d75`). No `shapenum == -1` record exists in the tables.

## 4. Object list and DoActor

| Function | Address | Delta from Wolf3D | Confidence |
|---|---|---|---|
| `InitActorList` | `ghidra 1942:0bdf` | 100 entries of 0x42 bytes; `objlist[99].prev` cleared explicitly (`DS:0xBFE9`) | CONFIRMED |
| `GetNewActor` | `ghidra 1942:0c3b` | When the free list is empty it **does not Quit**: sets `new = NULL` and `DS:0x330F = 1` (shown by the renderer, `1b4a:13a2`). Every spawner checks `new` | CONFIRMED |
| `RemoveObj` | `ghidra 1942:0c9b` | Same; removing the player is `QuitNum(103)` | CONFIRMED |
| `DoActor` | `ghidra 1942:10f5` | See below | CONFIRMED |
| PlayLoop actor loop | `ghidra 1942:1602`–`163e` | `memset(actorpos,0,200)`, then `for (i=0, ob=player; ob; ob=ob->next, i++) DoActor(ob)` with `i` in `DS:0x95B7` | CONFIRMED |

`QuitNum(n)` (`ghidra 1497:11f8`) is `Quit(itoa(n))`; OBC replaced Wolf3D's error strings with numbers (Quit is `1497:121d`).

DoActor deltas (all CONFIRMED, `ghidra 1942:10f5`):

1. **BEWITCHED freeze.** If `DS:0x3306` (cheat toggle, messages `BEWITCHED` / `BEWITCHED OFF` at `DS:0x341D`/`0x340F`, toggled by `ghidra 30d6:0456`) is set, a jump table on `obclass-1` (`1942:12e1`) returns immediately for classes 2–4, 6–17, 22, 23 (all enemies). Player, ally, flame, grenade, slime ball, fire, explosion, sparkle and face (classes 1, 5, 18–21, 24–26) keep running. KillActor turns BEWITCHED off when it is on (`1e43:1382`).
2. `actorpos[i] = 0` (`DS:0x94ED`, 100 words) before the usual `if (!active && !areabyplayer[area]) return`.
3. If `difficulty >= 4`, `FUN_358f_19fd(i, ob)` (network sync) after clearing `actorat`.
4. If `obclass != 5 && ob != player`: `actorpos[i] = tilex<<8 | tiley`. The automap/radar (`ghidra 2f4d:0e53`) draws these when `DS:0x873C` (debug) or `DS:0x3304` is set; this is probably the Map ThermoSensor (Q-027, HYPOTHESIS). Note: it includes projectiles, fires and corpses.
5. The think/action/next loop is identical to Wolf3D.

---

## 5. Class enumeration

`obclass = which + 3` for SpawnStand/SpawnPatrol (as Wolf3D's `guardobj+which`); special spawners write the class directly. All values CONFIRMED from the `mov [bx+4],imm` sites listed. Names are descriptive (from sprites) unless stated.

| Class | Name (sprites) | Spawned by | Plane-1 codes | Score | Drop |
|---|---|---|---|---|---|
| 1 | player | `ghidra 208c:1324` SpawnPlayer | 19–22 | | |
| 2 | inert (Wolf3D `inertobj`) | none found; `TryWalk` still special-cases it | | | |
| 3 | rat (64–112) | SpawnStand/Patrol `which 0` | §6.1 | 25 | |
| 4 | soldier, green fatigues (187–237) | `which 1` | §6.1 | 100 | item 11 |
| 5 | ally | `ghidra 2634:00e4` (`which 2` hit-point table) | 224–227, 242–245, 260–263 | 1 (network only) | item 8 |
| 6 | saucer drone (304–370) | SpawnPatrol `which 3` | §6.1 | 75 | |
| 7 | giant rat (133–186) | `which 4` | §6.1 | 100 | |
| 8 | soldier on a stool (113–117) | SpawnStand `which 5` | §6.1 | 100 | item 11 |
| 9 | crouching soldier (118–122) | `which 6` | | 100 | item 11 |
| 10 | soldier on a toilet (123–127) | `which 7` | | 100 | item 11 |
| 11 | soldier lying prone, "playing dead" (128–132) | `which 8` | | 100 | item 11 |
| 12 | black-suit gunman (371–423) | SpawnStand `which 9` | 278–281 (+gated) | 100 | item 5 |
| 13 | trench-coat boss (424–439), killing him wins the game | `ghidra 2390:0911` | 214 | 1500 | |
| 14 | gunner, shirtless with rifle (440–451) | `ghidra 2390:0f20` | 196 | 500 | |
| 15 | red-vest brute (452–463) | SpawnStand `which 12` | 142–143, 160–161 | 400 | item 11 |
| 16 | red giant (464–479) | `ghidra 2390:0fdc` | 232–233, 250–251 | 600 | |
| 17 | sewer fat man (510–525) | `ghidra 2390:0f7e` | 179 | 500 | item 11 |
| 18 | flame / fireball | `2390:0bd5` SpawnFlame; player flamethrower `30d6:0547` | | | |
| 19 | grenade | player `2f4d:0ff3`; dead enemy code `2390:13fb` | | | |
| 20 | slime ball | `2390:1508` A_SlimeThrow (swamp creature) | | | |
| 21 | fire (spreading, 624–639) | `30d6:0161` SpawnFireActor | 282–285 | (100, unreachable) | |
| 22 | sewer fat man rising from a manhole (504–509 then 510–525) | `ghidra 2390:103a` | 197 | 500 | item 11 |
| 23 | swamp creature (526–543) | `ghidra 2390:1094` | 215 | 200 | |
| 24 | explosion (660–667) | `2390:0ae4` (player weapon code, `208c:077a`) | | | |
| 25 | green sparkle "tracer" (676–679) | debug: `30d6:05c4`, `2390:1274` | | | |
| 26 | face apparition (668–675) | `2390:10f2` (PlayLoop random event, debug keys) | | | |

Item numbers are `PlaceItemType` keys (`ghidra 2281:020f`, statinfo table `DS:0x11DA`, 4-byte entries `{shapenum, itemtype}`): item 11 → sprite 30 (small gun pickup), item 5 → sprite 31 (submachine gun), item 8 → sprite 37 (flak jacket). Item meanings: HYPOTHESIS (pickup semantics belong to the player/items investigation).

**Victor.** Class 13's last death frame (`DS:0x1FFE`, sprite 439) has think `ghidra 2390:28d1`, which sets `playstate = 6` (`ex_victorious`). He has the largest hit points (4000+) and a `T_Chase`-based attack loop that never ends (§8.11). That he is "Victor" from the manual is LIKELY; the code proves only that his death ends the game.

---

## 6. Spawning

### 6.1 ScanInfoPlane plane-1 codes (`ghidra 1823:01c8`)

Ghidra did not create this function. It loops `y` over `mapheight`, `x` over `mapwidth`, reads `mapsegs[1]` (`DS:0x9411`), ignores codes < 19 and jumps through a 0x12B-entry table at `1823:06c3` (codes 19..317). CONFIRMED.

Difficulty gating follows Wolf3D's pattern but with OBC thresholds: Wolf3D tests `difficulty < gd_hard (3)` and `< gd_medium (2)`; OBC tests `< 2` and `< 1`. So the three tiers spawn at difficulty ≥0, ≥1, ≥2.

| Codes (tier 0 / ≥1 / ≥2) | Effect | dir |
|---|---|---|
| 19–22 | `SpawnPlayer(x,y,code-19)` (`208c:1324`) | |
| 23–85 | `SpawnStatic(x,y,code-23)` (`2281:0011`) | |
| 86–88, 103, 106, 107, 124 | `FUN_2f4d_0847(x,y,type)` with type `code-83`, 6, 1, 2, 6: special-wall objects (not actors) | |
| 98 | `secrettotal++` unless loaded game (`DS:0x872A`, `DS:0x8CBA`) | |
| 108–111 / 144–147 / 180–183 | rat stand | code − base |
| 112–115 / 148–151 / 184–187 | rat patrol | |
| 116–119 / 152–155 / 188–191 | soldier stand | |
| 120–123 / 156–159 / 192–195 | soldier patrol | |
| 126–129, 130–133, 134–137, 138–141 / 162–177 / 198–213 | disguised soldier stand, `which` 5, 6, 7, 8 per block of four | |
| 142–143, 160–161 (no gating) | red-vest brute stand (`which 12`), dirs 0–1 only | |
| 179 | sewer fat man (class 17) | north |
| 196 | gunner (class 14) | north |
| 197 | manhole fat man (class 22) | north |
| 214 | trench-coat boss (class 13) | south |
| 215 | swamp creature (class 23) | north |
| 216–219 / 234–237 / 252–255 | giant rat stand | |
| 220–223 / 238–241 / 256–259 | giant rat patrol | |
| 224–227 / 242–245 / 260–263 | ally (`2634:00e4`) | |
| 228–231 / 246–249 / 264–267 | nothing (reserved block; the gated ones still evaluate the test) | |
| 232–233, 250–251 (no gating) | red giant (class 16) | north |
| 270–273 / 288–291 / 306–309 | saucer patrol, hovering (`DS:0x9299 = 0` → speed 0) | |
| 274–277 / 292–295 / 310–313 | saucer patrol, moving (`DS:0x9299 = 0x300`) | |
| 278–281 / 296–299 / 314–317 | black-suit gunman stand | |
| 282–285 (no gating) | fire actor `SpawnFireActor(x,y)`; direction ignored | |
| 89–97, 99–102, 104–105, 125, 178, 268–269, 286–287, 300–305 | nothing in ScanInfoPlane (90–97 are path arrows; 105–107 are also see-through for CheckLine, §7.8) | |

### 6.2 SpawnStand (`ghidra 2390:04ca`)

`SpawnStand(which, tilex, tiley, dir)`; table at `2390:0727`. CONFIRMED.

| which | state | | which | state |
|---|---|---|---|---|
| 0 | `0x14AE` rat | | 7 | `0x195E` toilet |
| 1 | `0x17BE` soldier | | 8 | `0x196E` prone |
| 4 | `0x160E` giant rat | | 9 | `0x1C3E` black suit |
| 5 | `0x193E` stool | | 12 | `0x1DEE` red vest |
| 6 | `0x194E` crouch | | 2, 3, 10, 11 | `QuitNum(1901)` |

After SpawnNewObj: the AMBUSHTILE block is Wolf3D's (same neighbour order +1, −mapwidth, +mapwidth, −1, same uninitialised `tile` if no neighbour qualifies), but it is skipped when `new == NULL`, and in difficulty 3 the area tile is re-derived from the actor's (scattered) position. Then: `killtotal++` unless loaded game (`DS:0x872E`); speed = **0x300** (Wolf3D SPDPATROL is 0x200), or for rats `US_RndT()+0x300` and `flags2 |= 2`; `obclass = which+3`; `SetHitPoints(which)`; `dir = dir*2`; `flags |= FL_SHOOTABLE`.

### 6.3 SpawnPatrol (`ghidra 2390:0741`)

Table at `2390:0907`: which 0 → `0x14BE`, 1 → `0x17CE`, 3 → `0x1ABE` (saucer), 4 → `0x161E`, else `QuitNum(1902)`. Sets `killtotal++`, `obclass`, `dir*2`, `distance = TILEGLOBAL` (from `DS:0x0D1A`), `FL_SHOOTABLE`, `active = 1`. Speed: rat `US_RndT()+0x300` + `flags2|=2`; saucer `DS:0x9299 ? US_RndT()*2 + DS:0x9299 : 0` + `flags2|=2`; others 0x300. Then `SetHitPoints(which)`. CONFIRMED.

**K: patrol start tile.** Wolf3D moves a patroller one tile along its direction. OBC moves it, then restores the original tile unless `actorat` at the new tile is **≥ 0x8C** (an object pointer): `ghidra 2390:08c9` `cmp [actorat],0x8C; jae keep`. An empty tile (0) or a wall restores the old tile, so in practice patrollers start on their own tile. CONFIRMED (instruction level); intent unknown.

### 6.4 Boss and special spawners

All set `speed` low word, `obclass`, call `SetHitPoints`, set `dir`, `flags`, and `killtotal++` unless loaded. CONFIRMED.

| Address | Class | State | speed | dir | flags | SetHitPoints | Other |
|---|---|---|---|---|---|---|---|
| `2390:0911` | 13 | `0x1F1E` | 0x300 | 6 (south) | `=1` | 10 | |
| `2390:0f20` | 14 | `0x21AE` | 0x300 | 2 (north) | `=1` | 11 | writes 10 to `DS:0x2232` (tictime of state `0x222E`) |
| `2390:0fdc` | 16 | `0x205E` | 0x300 | 2 | `=1` | 13 | writes 10 to `DS:0x20E2` (tictime of `0x20DE`) |
| `2390:0f7e` | 17 | `0x229E` | 0x300 | 2 | `=1` | 14 | writes 10 to `DS:0x2382` (tictime of `0x237E`) |
| `2390:103a` | 22 | `0x22AE` | 0x300 | 2 | `|=1` | 16 | same write |
| `2390:1094` | 23 | `0x244E` | 0x100 | 2 | `|=0x41` (shootable+ambush) | 17 | `flags2 |= 2`; same write |
| `30d6:0161` | 21 | `0x138E` | 0x19 | 6 | `=0x84` (NONMARK+NEVERMARK, not shootable) | 15 | refuses if `objcount >= 80`; plays sound 71/72/73 (`DS:0x32FE`) when `DS:0x8CBC`; does **not** count in killtotal |

The three tictime writes store the value already in the file (10); no other writer was found. HYPOTHESIS: leftover tuning code; harmless to port as a no-op.

### 6.5 SetHitPoints (`ghidra 2390:28df`)

```
mult = difficulty + 3
which == 2 (ally): hitpoints = DS:0x32EC[difficulty]   // {400,400,400,400,800,800,800,400}
otherwise:         hitpoints = (US_RndT() / div) * mult + base, capped at 5000
```
(`idiv`, so integer quotient; one `US_RndT` call.) Table at `2390:29f1`. CONFIRMED.

| which | class | div | base | difficulty 0 range |
|---|---|---|---|---|
| 0 | 3 rat | 4 | 1 | 1–190 |
| 1 | 4 soldier | 4 | 175 | 175–364 |
| 3 | 6 saucer | 5 | 25 | 25–178 |
| 4 | 7 giant rat | 4 | 10 | 10–199 |
| 5–8 | 8–11 disguised | 4 | 250 | 250–439 |
| 9 | 12 black suit | 3 | 275 | 275–530 |
| 10 | 13 trench-coat boss | 1 | 4000 | 4000–4765 (cap 5000) |
| 11 | 14 gunner | 2 | 2100 | 2100–2481 |
| 12 | 15 red vest | 1 | 3000 | 3000–3765 |
| 13 | 16 red giant | 2 | 1400 | 1400–1781 |
| 14 | 17 sewer fat man | 2 | 2300 | 2300–2681 |
| 15 | 21 fire | 1 | 1300 | 1300–2065 |
| 16 | 22 manhole fat man | 2 | 2300 | 2300–2681 |
| 17 | 23 swamp creature | 4 | 250 | 250–439 |
| other | | 4 | 25 | after `QuitNum(1903)` |

The two remaining zero-divisor and unknown-type paths call `QuitNum(1904)` / `QuitNum(1903)`.

### 6.6 SpawnNewObj (`ghidra 1e43:0007`)

Wolf3D's function plus three OBC changes (CONFIRMED):

1. `new = NULL` first; if `state == 0x137E` (fire "stand", never spawned in practice) and `objcount > 50`, return.
2. **Difficulty 3 scatter.** If `difficulty == 3`: up to 10 times pick `x = RandomNear(tilex, 24)`, `y = RandomNear(tiley, 24)` until `actorat[x][y] == 0` (`RandomNear` = `ghidra 2f4d:1388`: `sign = US_RndT()>128 ? -1 : 1; return abs(v + sign*(US_RndT()%range)) % 64`; range is `DS:0x2FD0*8` = 24). Give up if the 10th spot is occupied. If a static object occupies the spot, give up, except that a fire actor on a bonus item ignites it (`Ignite(origx,origy)`). Otherwise use the new tile.
3. `GetNewActor` may return NULL; then nothing is set. `flags = 0` is written explicitly.

`ticcount = tictime ? US_RndT() % tictime : 0`, `dir = nodir`, `areanumber = map0 − 0x8C`, `actorat` set: as Wolf3D.

---

## 7. Shared AI: deltas from Wolf3D

| Wolf3D function | OBC address | Result |
|---|---|---|
| `SpawnNewObj` | `ghidra 1e43:0007` | Deltas, §6.6 |
| `NewState` | `ghidra 1e43:01ab` | No delta (CONFIRMED) |
| `TryWalk` | `ghidra 1e43:01c3` | Deltas, §7.3 |
| — (new) ActorBumpWall | `ghidra 1e43:0cca` | §7.3 |
| `SelectDodgeDir` | `ghidra 1e43:0d05` | **Delta**: returns immediately, leaving `dir` unchanged, unless `US_RndT() % 5 == 0` |
| `SelectChaseDir` | `ghidra 1e43:0e75` | No delta (including Wolf3D's north..west loop that tries northwest) |
| `SelectRunDir` | `ghidra 1e43:0fdd` | No delta |
| `MoveObj` | `ghidra 1e43:10b1` | Deltas, §7.4 |
| `DropItem` | `ghidra 1e43:12e6` | Returns at once if `difficulty >= 4`; otherwise Wolf3D |
| `KillActor` | `ghidra 1e43:1377` | Deltas, §7.5 |
| — (new) UpdateKillRatio | `ghidra 1e43:1710` | status-bar indicator 0/1/2 from killcount vs killtotal and killtotal/2 |
| `DamageActor` | `ghidra 1e43:174d` | Deltas, §7.6 |
| `CheckLine` | `ghidra 1e43:1982` | Deltas, §7.8 |
| `CheckSight` | `ghidra 1e43:1d35` | No delta (MINSIGHT 0x18000, same facing tests). Ghidra's callee list for it is wrong |
| `FirstSighting` | `ghidra 1e43:1e38` | Per-class table, §7.7 |
| `SightPlayer` | `ghidra 1e43:2313` | Deltas, §7.7 |
| `T_Stand` | `ghidra 2390:1d1d` | `SightPlayer(ob)`: no delta |
| `SelectPathDir` | `ghidra 2390:1ff4` | No delta |
| `T_Path` | `ghidra 2390:2047` | Deltas, §7.2 |
| `T_Chase` | `ghidra 2390:1d2d` | Deltas, §7.1 |
| `T_Shoot` | `ghidra 2390:2506` | Deltas, §7.9 |
| `A_DeathScream` | `ghidra 2390:0c59` | Sound tables, §9 |
| `T_Projectile` / `ProjectileTryMove` | `ghidra 2390:0125` / `2390:0006` | §10 |

### 7.1 T_Chase (`ghidra 2390:1d2d`)

- `if (difficulty >= 4) return;` replaces Wolf3D's `if (gamestate.victoryflag) return;`. Enemies do not move or attack in network modes. CONFIRMED.
- Attack decision is Wolf3D's: `CheckLine`, `dist = max(|dx|,|dy|)` in tiles, `chance = (!dist || (dist==1 && distance<0x4000)) ? 300 : (tics<<4)/dist`, attack if `US_RndT() < chance`, else `dodge = 1`.
- Attack states by class (table `2390:1fce`): 3 → `0x153E`, 4 and 8–11 → `0x184E`, 6 → `0x1B8E`, 7 → `0x169E`, 12 → `0x1CCE`, 13 → `0x200E`, 15 → `0x1EBE` (unused: class 15 uses T_BossChase), 16 → `0x214E`, 21 → `0x144E` followed by `FireFlash()` (`1942:0f44`). Classes 5, 14, 17–20 return without acting.
- Direction choice when `dir == nodir` and at each tile: classes 5 and 6 always use `SelectChaseDir` (never dodge). Others with `dodge` call `HalveSpeed(ob)` (`2390:2a15`, `speed >>= 1` arithmetic), `SelectDodgeDir`, `DoubleSpeed(ob)` (`2390:2a2f`). **K:** the halve/double pair clears bit 0 of `speed` permanently (rats have odd speeds). CONFIRMED.
- Door waiting uses `doorobjlist` at `DS:0x7BA0` (10-byte entries, `action` at +6, 0 = open): as Wolf3D.

Consequence of the SelectDodgeDir 1-in-5 gate (HYPOTHESIS about visible behaviour, CONFIRMED code): when a dodging actor reaches its tile centre and SelectDodgeDir keeps the old direction without calling TryWalk, `distance` keeps its old value and `tilex/tiley` are not advanced, so the next MoveObj continues in the same direction without a collision check until the loop snaps it back to the old tile centre. Needs an oracle trace before porting decisions (Q-ACT-1 below).

### 7.2 T_Path (`ghidra 2390:2047`)

Wolf3D's loop, plus: returns at once if `difficulty >= 4`; for class 23 (swamp creature) only acts when `US_RndT() % 5 == 0`, sets `flags2 |= 2` before `SightPlayer`, and on a sighting clears `flags2` bit 1 **and `FL_SHOOTABLE`**. The "T_Path hit a wall at %u,%u, dir %u" check uses `> 64` and `Quit` (string `DS:0x2A00`). CONFIRMED.

### 7.3 TryWalk (`ghidra 1e43:01c3`)

Wolf3D's structure (inert class 2 moves blindly; CHECKDIAG / CHECKSIDE macros). Deltas, CONFIRMED:

- Class **15** (red-vest brute) takes the CHECKDIAG path on cardinal moves, the role Wolf3D gives to dogs and fakes: it cannot open doors.
- In every CHECKDIAG and CHECKSIDE block (20 sites: 4 cardinal × 2 variants + 4 diagonal × 3), class **21** (fire) first tests `IsBurnable(x,y)` (`30d6:0012`: tile empty and either an object pointer in `actorat` or a static with flag 0x02 there); if so it calls `Ignite(x,y)` (`30d6:0089`) and the move fails.
- In the four cardinal CHECKSIDE blocks, a wall (`actorat < 0x80`) whose `tilemap` value is 13, 29, 49, 65 or 69 calls `ActorBumpWall(ob,x,y)` (`1e43:0cca`), which (except for classes 3, 6, 7, 21) calls `FUN_2f4d_08e8(x,y)`, the special-wall/gate list at `DS:0x46E2` (12-byte entries, count `DS:0xE7A8`); the move still fails. HYPOTHESIS: these are walls that open when an enemy pushes them (other investigation).
- Bad direction: `QuitNum(302)`.
- At the end the area number is stored only if `0 <= map0-0x8C < 0x16`.

### 7.4 MoveObj (`ghidra 1e43:10b1`)

- No `areabyplayer` test: the "not on top of the player" back-up (MINACTORDIST 0x10000) applies everywhere. No ghost damage. CONFIRMED.
- After `distance -= move`, class 21 calls `SpawnFireActor(tilex,tiley)` when `US_RndT() == 200` (fire spreads, 1/256 per move). CONFIRMED.
- Bad direction: `QuitNum(303)`.

### 7.5 KillActor (`ghidra 1e43:1377`)

Order (CONFIRMED):
1. If BEWITCHED (`DS:0x3306`) is on, toggle it off (`30d6:0456`, only when `DS:0x873C`).
2. `ob == NULL` → debug print "Kill NULL Actor" (`DS:0x0F32`) and return. Not shootable → return (nothing else happens).
3. If `DS:0x0F1C` → `SpawnFlame(ob)`. No writer of `DS:0x0F1C` was found (value 0 in the file): LIKELY dead.
4. `tilex/tiley = x>>16, y>>16`; per-class `GivePoints` (`208c:0614`), death state, `DropItem` (table `1e43:16e6`; values in §5 and §8). Classes 14 and 15 also copy the player position to `DS:0x8734..0x873A` (Wolf3D `gamestate.killx/killy`, used for the boss death cam in Wolf3D). Class 5 (ally, single player): `DS:0x330A = 1`, `DS:0x330C = 0`, `DS:0x32FC--` (ally count; `30d6:097e`, misnamed `DrawWeapon`, draws it), clear `FL_SHOOTABLE`, drop item 8.
5. Single player: unless `DS:0x2BC8` (set by ally code while an ally deals damage) or the victim is an ally, `killcount++` (`DS:0x8728`) and redraw; format the remaining-enemies count into `DS:0x874C`. If `killtotal == 0` → `QuitNum(304)`. Kill-percentage trigger: if `DS:0xCD87 <= 0` and `DS:0x2FD2[difficulty] < killcount*100/killtotal` (table {10, 75, 100, 100, 85}) and `DS:0xCD87 != -2`, set `DS:0xCD87 = tics<<4`; if all are dead, `tics<<4 + 1`. No reader of `DS:0xCD87` was found (UNKNOWN). Network: `FUN_358f_18e7(ob,x,y)` instead.
6. Clear `FL_SHOOTABLE`, `actorat[tilex][tiley] = NULL`, set `FL_NONMARK` unless network, `UpdateKillRatio()`.

### 7.6 DamageActor (`ghidra 1e43:174d`)

1. Network (`difficulty >= 4`): `FUN_358f_135a(ob)` and return.
2. **New:** if `gamestate.chosenweapon` (`DS:0x8718`) is 3 and `US_RndT() % 50 == 0`, `KillActor(ob)` outright. HYPOTHESIS: weapon 3 is the flamethrower.
3. `if (!(flags & FL_ATTACKMODE)) damage <<= 1;` (Wolf3D).
4. **New:** class 23 in any of its chase states `0x250E..0x255E` takes no damage.
5. `hitpoints -= damage`; `<= 0` → KillActor.
6. Not in attack mode → FirstSighting.
7. Pain (table `1e43:196e`, classes 3–12): rat `hp&1 ? 0x151E : 0x152E`; soldier, classes 10 and 11 `hp&1 ? 0x182E : 0x183E`; ally `0x2AA6/0x2AB6`; giant rat `0x167E/0x168E`; classes 8 and 9 only play sound 76 or 77 (table `DS:0x0F1E`) when `tics` is odd; black suit always `0x1CAE` and 1-in-7 sound 76/77; saucer and classes 13+ nothing.

**Wolf3D's `madenoise = true` is gone** from DamageActor. Only the player's gun attack (`208c:14ee`) makes noise. CONFIRMED.

### 7.7 SightPlayer (`ghidra 1e43:2313`) and FirstSighting (`ghidra 1e43:1e38`)

SightPlayer, CONFIRMED:
```
if (difficulty >= 4) return false;
if (obclass == 23 && US_RndT() < 150) return false;          // new
if (flags & FL_ATTACKMODE) QuitNum(308);
if (temp2) { temp2 -= tics; if (temp2 > 0) return false; temp2 = 0; goto see; }
if (temp3) { temp3 -= tics; if (temp3 > 0 && !CheckSight(ob)) return false; temp3 = 0; goto see; }   // new, network-only writer
if (!areabyplayer[area]) return false;
if (flags & FL_AMBUSH) { if (!CheckSight) return false; flags &= ~FL_AMBUSH; }
else if (!madenoise && !CheckSight) return false;
temp2 = reaction[class]; return false;
see: FirstSighting(ob); return true;
```

Reaction time `temp2` (table `1e43:246f`; quotient of `US_RndT()`): rat, fire, classes 22, 23: `1 + rnd/4`; soldier and ally: 2; all others 6–17: `1 + rnd/6`; classes 18–20: unchanged.

FirstSighting (table `1e43:22e9`): sound, chase state, speed change; then `distance < 0 → 0`, `flags |= ATTACKMODE|FIRSTATTACK`, `active = 1`. A trailing `switch(dir)` (when `DS:0x0954 != 0`) only validates `dir <= 8` (`QuitNum(307)`). Returns at once in network modes.

| Class | Sound | State | Speed |
|---|---|---|---|
| 3 rat | — | `0x156E` | ×3 |
| 4 soldier | one of {54,55,56,53,53,51} (`DS:0x0F22`, `rnd%6`) | `0x187E` | ×5 |
| 5 ally | — | `0x2AE6` | ×4 |
| 6 saucer | — | `0x1ACE` | ×5 |
| 7 giant rat | — | `0x16DE` | ×3 |
| 8, 9, 10, 11 | as soldier | `0x197E`, `0x19CE`, `0x1A1E`, `0x1A6E` (stand-up animations) | ×4 |
| 12 black suit | as soldier | `0x1D0E` | ×5 |
| 13 boss | 58 | `0x1F2E` | `= 0x900` |
| 14 gunner | 60 | `0x21BE` | ×3 |
| 15 red vest | one of {52,61} (`DS:0x0F2E`) | `0x1DFE` | ×3 |
| 16 red giant | 57 | `0x206E` | ×3 |
| 17 fat man | 57 | `0x230E` | ×3 |
| 18 flame | — | `0x260E` | — |
| 21 fire | 71 | `0x138E` | ×3 |
| 22 manhole | 57 | `0x22BE` (rise) | ×3 |
| 23 swamp | 50 | `0x245E` (rise) | ×3 |

Wolf3D multipliers for comparison: guard ×3, officer ×5, SS ×4, mutant ×3, dog ×2, from SPDPATROL 0x200.

### 7.8 CheckLine (`ghidra 1e43:1982`)

Wolf3D's DDA (player position `plux` = `DS:0xCD8B`, `pluy` = `DS:0xCD89`), with two deltas, CONFIRMED:
- `deltafrac == 0` → `QuitNum(305)` (x pass) / `QuitNum(306)` (y pass).
- Before the tile test, a cell whose **plane-1** value is 0x69, 0x6A or 0x6B (105, 106, 107) is skipped (see-through). 106 and 107 are the special-wall types 1 and 2 of §6.1.

### 7.9 T_Shoot (`ghidra 2390:2506`) and enemy damage

```
if (difficulty < 4 && T_ShootAlly(ob)) return;       // 2390:26b9, new
if (!CheckLine(ob)) return;                           // Wolf3D's areabyplayer test is gone
dist = max(|dx|,|dy|);
hitchance = (thrustspeed >= 6000) ? 160 - dist*(VISABLE?16:8) : 256 - dist*(VISABLE?16:8);   // thrustspeed DS:0xCD9B
if (US_RndT() < hitchance) {
    damage = dist<2 ? US_RndT()>>2 : dist<4 ? US_RndT()>>3 : US_RndT()>>4;
    switch (obclass) {
      case 21: dist==0 → TakeDamage(100); dist==1 → TakeDamage(5); else nothing;
      case 3:  dist<=1 → TakeDamage(5);
      case 6, 7: dist<=1 → TakeDamage(damage);
      default: TakeDamage(damage);
    }
}
```
No firing sound here (Wolf3D plays one per class); sounds come from `A_Shoot` (§9), which always ends by calling `T_Shoot`. CONFIRMED.

`T_ShootAlly` (`2390:26b9`): `target = FUN_2634_036e(ob)`; if it is an ally (class 5) and `FUN_2634_0412(target, ob)` (line of sight), the enemy shoots the ally instead of the player and returns 1. **K:** the damage is computed only when `US_RndT() < 128`, but `DamageActor(ally, damage)` runs either way, so a miss applies an uninitialised stack value (`2390:2726`–`27dd`). Class 21: dist 0 → 75, dist 1 → 5; class 3 → 5 at dist ≤ 1; class 7 → damage at dist ≤ 1; others → damage (saucer not special-cased here). CONFIRMED.

---

## 8. Classes in detail

Common actions: `A_Shoot` = `2390:2321` (class attack sound, then `T_Shoot`); `A_IdleSound` = `2390:21b7`; `A_DeathScream` = `2390:0c59`; `A_Remove` = `2390:0c4a` (`ob->state = NULL`, so DoActor removes the object); `A_Nop` = `2390:2a49`. State tables are in §11. Shapes: `rot 1` frames use 8 consecutive sprites; `rot 2` frames use `shape` (front half) or `shape+4`.

### 8.1 Rat (class 3)
Stand `0x14AE` (T_Stand), path `0x14BE`, chase `0x156E` loop, pain `0x151E`/`0x152E`, attack `0x153E`, die `0x15CE`. Speed `0x300+rnd` (×3 in chase), does not block the player. Attack: sound 30/31/32, 5 damage only within 1 tile. Death sound 2/3; 25 points; no drop. Sprite layout copies Wolf3D's guard (pain1, die1–3, pain2, dead, shoot1–3 at 104–112).

### 8.2 Soldier (class 4)
Stand `0x17BE`, path `0x17CE` (all frames 5 tics), sighting enters `0x187E` (sprite 237, 5 tics) then chase `0x188E`, pain `0x182E`/`0x183E`, attack `0x184E` (one A_Shoot), die `0x18EE`. Sight/idle sounds 51–56; attack 39; death 10–13, or 14/15/27/28 when the current weapon (`DS:0x8716`) is 3. 100 points, drops item 11. Chase speed 0x300×5 = 0xF00.

### 8.3 Disguised soldiers (classes 8–11)
Non-rotating stand states `0x193E`, `0x194E`, `0x195E`, `0x196E` (sprites 113, 118, 123, 128) with `T_Stand`; FirstSighting plays a soldier sight sound and runs a 5-frame, 15-tic stand-up animation that ends in the soldier chase state `0x188E`. They keep their own class: attack sound 38/39, pain (8, 9: sound only; 10, 11: soldier pain states), death as soldier (sound 10–13), 100 points, item 11. Corridor 7's "morph" furniture enemies are the same idea (`EC7Wolf/wadsrc/static/actors/corridor7/monsters.txt:207`), but OBC's values come from the code above.

### 8.4 Saucer drone (class 6)
Spawned only as a patroller: `0x1ABE` is a 1-tic `T_Path` loop. Codes 270–273 give speed 0 (it never moves, even after the ×5 sighting multiplier); 274–277 give `0x300 + 2·rnd`. Chase `0x1ACE` (12 frames, 3 tics, `A_Nop` actions), attack `0x1B8E` (sound 41, lightning sprites 368–370) then `T_Shoot` again: damage only within 1 tile. Never dodges. Death `0x1BBE` explodes (360–367) and ends in `A_Remove`: no corpse. Death sound 8/9, 75 points. Does not block the player.

### 8.5 Giant rat (class 7)
Stand `0x160E`, path `0x161E`, chase `0x16DE`, pain `0x167E`/`0x168E` (rot 0), attack `0x169E` (sound 33/34, `T_Shoot` at `0x16BE`, damage only within 1 tile), die `0x173E`. Death sound 4/5, 100 points.

### 8.6 Black-suit gunman (class 12)
Stand `0x1C3E`, path `0x1C4E`, chase `0x1D0E` (the second frame's action is `A_DebugSparkle`, inert unless `DS:0x3268` and `DS:0x3308` are both set), pain `0x1CAE`, attack `0x1CCE` (A_Shoot sound 38, then T_Shoot at `0x1CEE`: two shots), die `0x1D6E`. Sight/idle 51–56, death as soldier. 100 points, item 5. `0x1CBE` is unreachable.

### 8.7 Red-vest brute (class 15)
Non-rotating. Stand `0x1DEE`, chase `0x1DFE` with `T_BossChase` (`2390:179b`), attack `0x1EBE` (six 10-tic frames, one A_Shoot, sound 43), die `0x1E5E`. No pain state. Cannot open doors (§7.3). Sight 52/61, idle 61/52, death 18/19 or 28 with weapon 3. 400 points, item 11.

`T_BossChase` is Wolf3D's T_Schabb/T_Gift pattern: attack if `CheckLine` and `US_RndT() < tics<<3` (class 14 → `0x227E`, else `0x1EBE`); at each tile `dist < 4 → SelectRunDir`, else dodge or chase (with the speed halve/double). CONFIRMED.

### 8.8 Gunner (class 14)
Non-rotating. Stand `0x21AE`, chase `0x21BE` (T_BossChase), attack `0x227E` (T_Shoot, then A_Shoot sound 44: two shots), die `0x221E`. Sight and idle 60, death 20/21 or 27. 500 points.

### 8.9 Red giant (class 16)
Non-rotating. Stand `0x205E`, chase `0x206E` (plain T_Chase), attack `0x214E` (six frames; five T_Shoot and one A_Shoot with sound 40: six shots; the sprites show fire from the hands but the attack is hitscan), die `0x20CE` (melts to a skeleton, last frame loops). Sight/idle 57, death 6 or 7 with weapon 3. 600 points.

### 8.10 Sewer fat man (classes 17 and 22)
Class 17 stands at `0x229E`; class 22 waits at `0x22AE` (sprite 504, the manhole) and on sighting plays the rise `0x22BE`–`0x22FE`, whose last action `A_ManholeCover` (`2390:1b17`) places static `statinfo[49]` (sprite 50, an open manhole) at its tile, then attacks. Chase `0x230E` with `T_FatChase` (`2390:1966`, same pattern as T_BossChase, attack state `0x23EE`); attack fires four shots (T_Shoot, A_Shoot sound 37, T_Shoot, T_Shoot); die `0x236E` (sprite 521 held 40 tics). Sight/idle 57, death 17. 500 points, item 11.

### 8.11 Trench-coat boss (class 13)
Non-rotating. Stand `0x1F1E`, chase `0x1F2E` (T_Chase, idle sound on the second frame), attack `0x200E`: A_Shoot (sound 42) then three T_Shoot, and the last attack state's `next` is `0x200E` again. **K:** once he attacks he never returns to chasing. Speed 0x900 after sighting. No pain state. Death `0x1F8E` (sound 16) dissolves him (432–439, 15 tics each); the last frame's think `T_BossDead` sets `playstate = 6`. Fire contact does random damage to him instead of killing him (§10.3). 1500 points.

### 8.12 Swamp creature (class 23)
- Submerged `0x244E` (sprite 526, think T_Path every frame, acting 1 in 5), speed 0x100 at spawn, does not block the player. Only `T_Path` and SightPlayer (150/256 extra miss chance) see the player.
- On sighting: `FL_SHOOTABLE` cleared, rise `0x245E`–`0x249E` (527–531) to attack `0x25AE`.
- Attack: A_Shoot (sound 35/36, plus a hitscan `T_Shoot`), then `A_SlimeThrow` (`2390:1508`) at `0x25DE`: sets `FL_SHOOTABLE` and throws a class-20 slime ball at the player. Then chase `0x250E` with `T_SwampChase` (`2390:1b31`).
- T_SwampChase: 1 in 20 per think, submerge (`0x24AE`, frames 531→526 back to `0x244E`) with `flags = 0`, `flags2 |= 2`, speed 0x600. Otherwise attack when `CheckLine` and `US_RndT() < tics<<3` (speed set to 0x100), and run away (`SelectRunDir`) whenever closer than 4 tiles.
- DamageActor ignores damage while it is in a chase state (`0x250E`–`0x255E`).
- Death `0x256E` is a green splash ending in `A_Remove` (no corpse); sound 0/1; 200 points.

### 8.13 Fire (class 21)
Spawned by codes 282–285 or by spreading. Not shootable (flags 0x84). Chase loop `0x138E`–`0x143E` (sprites 624–635, think T_Chase on four frames), speed 25. `T_FireBurnout` (`2390:1620`) on the last frame: if `|US_RndT() - US_RndT()| < 4`, go to the burn-out `0x148E` (638–639, then `A_Remove`) and `SpawnFlame(ob)`. Attack `0x144E`: A_Shoot (sound 71/73), contact damage 100 at distance 0, 5 at distance 1; `FireFlash()` palette effect. Spreads: TryWalk ignition (§7.3) and 1/256 per move (§7.4). KillActor has a 100-point case for it, but fire is never shootable, so KillActor returns before scoring (LIKELY unreachable). Fire actors are not added to `killtotal`. `0x146E`, `0x147E` are unreachable; `0x137E` is never spawned.

---

## 9. Sounds

Sound ids are the first argument of `PlaySoundAt(id, x, y)` (`ghidra 1823:0106`, Wolf3D `PlaySoundLocGlobal`). Their mapping to AUDIOMUS entries is not resolved here (UNKNOWN). All CONFIRMED from the tables at `DS:0x2998`–`0x29EC`, `DS:0x0F1E`–`0x0F30`, `DS:0x32FE`.

| Class | Sight | Idle (`A_IdleSound`) | Attack (`A_Shoot`) | Death (`A_DeathScream`) | Pain |
|---|---|---|---|---|---|
| 3 rat | — | — | 30, 31, 32 | 2, 3 | — |
| 4 soldier | 54,55,56,53,53,51 | 54,55,56,51,53 | 39 | 10,12,11,13 (weapon 3: 14,15,27,28) | — |
| 5 ally | — | — | — | 22–26 | — |
| 6 saucer | — | — | 41 | 8, 9 | — |
| 7 giant rat | — | — | 33, 34 | 4, 5 | — |
| 8–11 disguised | as soldier | — | 38, 39 | 10,12,11,13 | 8, 9: 76/77 |
| 12 black suit | as soldier | as soldier | 38 | as soldier | 76/77 (1 in 7) |
| 13 boss | 58 | 58 | 42 | 16 | — |
| 14 gunner | 60 | 60 | 44 | 20, 21 (weapon 3: 27) | — |
| 15 red vest | 52, 61 | 61, 52 | 43 | 18, 19 (weapon 3: 28) | — |
| 16 red giant | 57 | 57 | 40 | 6 (weapon 3: 7) | — |
| 17 fat man | 57 | 57 | 37 | 17 | — |
| 21 fire | 71 | — | 71, 73 | — | — |
| 22 manhole | 57 | — | 37 | 17 | — |
| 23 swamp | 50 | 50 | 35, 36 | 0, 1 | — |

- `A_IdleSound` gate: `areabyplayer[area]`, `US_RndT()%9 == 0`, and (`DS:0x46A2 & 1` or `US_RndT()%7 == 0`); classes 4 and 12 need a further `US_RndT()%7 == 0`. `DS:0x46A2` is also tested by ID_SD (`2b79:0563`); meaning UNKNOWN.
- `A_DeathScream` first calls `VL_WaitVBL(12)` if `DigiNumber` (`DS:0xCDA1`) is 45..49 (a blocking wait; port as a no-op or a deliberate delay, Q-ACT-4).
- Fire spawn sound: 71, 72 or 73 (`DS:0x32FE`), only when `DS:0x8CBC`.

---

## 10. Projectiles and effects

Projectiles are ordinary actors with `T_Projectile` (`ghidra 2390:0125`) as think, a `target` at `+0x3C`, `dir = nodir`, `flags` 0x80 or 0x84, `active = 1`. CONFIRMED.

### 10.1 T_Projectile

```
speed*tics → deltax = FixedByFrac(move, cos[angle]), deltay = -FixedByFrac(move, sin[angle]); each clamped to ≤ 0x10000 (as Wolf3D)
x += deltax; y += deltay;
if (target) d = |x-target.x|, |y-target.y| else d = 0xC000 (never "hits")
if (!ProjectileTryMove(ob, &hitx, &hity)) switch (obclass) {
   20 slime: NewState(0x134E)  (splash)
   19 grenade: dmg = difficulty<4 ? 0 : (rnd>>3)+30; GrenadeSplash(dmg, ob);
               single player: FUN_30d6_12b1(hitx, hity, ob)  (blast hits walls/objects: other investigation)
               NewState(0x12FE)  (small explosion)
   18 flame: if IsBurnable(hitx,hity) { Ignite(hitx,hity); FireFlash(); } state = NULL; debug "F HIT"
   default: state = NULL
} else if (dx < 0xC000 && dy < 0xC000) switch (obclass) {   // touched target
   18: Ignite(tilex,tiley); FireFlash(); dmg = target==player ? (rnd>>1)+30 : rnd+30
   19: (single) FUN_30d6_12b1(tilex,tiley,ob); dmg = target==player ? (rnd>>1)+30 : rnd+30; GrenadeSplash(dmg,ob)
   20: dmg = target==player ? (rnd>>3)+30 : rnd+25; if (US_RndT()%4) { DS:0x0D42 = 1; FUN_30d6_0fbe(1); }  (slime on screen)
   25: state = NULL; return (no damage)
   then: target==player ? TakeDamage(dmg, ob) : DamageActor(target, dmg); state = NULL
} else { tilex = x>>16; tiley = y>>16; }
```
Classes 21–24 reaching the hit branch would use an uninitialised `dmg`; it cannot happen because they have no target. Grenades therefore damage twice on a direct hit (splash, then direct). CONFIRMED.

`ProjectileTryMove` (`2390:0006`): PROJSIZE **0x1000** (Wolf3D 0x2000). Scans the covered tiles row by row (y outer), records the tile in `hitx/hity`, fails on a wall, on a non-object `actorat` value, and (new) on a shootable object; in network games objects or the player with the same `temp2` (team) are skipped. CONFIRMED.

`GrenadeSplash(dmg, ob)` (`2f4d:116e`): for x in `[tx-2, tx+2)` and y in `[ty-2, ty+2)` (**K:** asymmetric 4×4 area), each shootable object gets `DamageActor(obj, 100 - (|dx|+|dy|))` (ignores `dmg`); only in network games the player on a covered tile takes `dmg/2`. CONFIRMED.

### 10.2 Projectile and effect table

| Class | Spawner | State | Speed | Target | Notes |
|---|---|---|---|---|---|
| 18 fireball | `30d6:0547` (player flamethrower) | `0x260E` (501–503) | 0x3000 | aimed actor | flags 0x84 |
| 18 burning flames | `2390:0bd5` SpawnFlame(ob) | `0x263E` (485–500 ×2, 10 tics) | 0x14 | none | at `ob`'s position, player's angle; ends `A_Remove` |
| 19 grenade | `2f4d:0ff3` (player) | `0x12EE` (480) | 0x2000 | aimed actor | renderer rotates it by `angle` (CalcRotate special case, as Wolf3D rockets) |
| 20 slime ball | `2390:1508` | `0x133E` (481) | 0x2000 | player | |
| 24 explosion | `2390:0ae4` | `0x283E` (660–667) | 0 | none | one tile in front of the player |
| 25 sparkle | `30d6:05c4`, `2390:1274` | `0x293E`/`0x294E`/`0x295E`/`0x296E` (random) | 0x5000 | player or actor | debug only (HYPOTHESIS) |
| 26 face | `2390:10f2` | `0x28BE` (668–675) | 0x300 | none | only if `mapon >= 6` (`DS:0x86FE`); PlayLoop (`1942:18c1`) spawns it when `US_RndT()==0 && US_RndT()%15==0` and `DS:0x95F9 == 0`, as long as `DS:0x0AB2 == 0` (set to 1 after the first spawn and never cleared: once per program run) or debug mode `DS:0x873C` is on; also two debug key flags in `1942:0a79`/`0aa3`; ends with `A_Remove` |

Dead code (no caller and no far-pointer reference): `2390:096c` (fireball ahead of the player), `2390:13fb` (enemy grenade throw), `2390:166f` (enemy fireball throw), `2390:27ec` (position check), `2390:13df`/`13ed`.

### 10.3 Fire and ignition

- `Ignite(x,y)` (`30d6:0089`, single player only): a shootable object on the tile: class 13 takes `DamageActor(US_RndT())`; the player takes `TakeDamage(30)`; classes 6, 21, 23 are immune; **any other actor is killed outright** (`SpawnFlame` + `KillActor`). If no object was affected, every bonus static on the tile (flag 0x02, `shapenum != -1`) becomes a new fire actor and is removed (`shapenum = -1`) when `SpawnFireActor` succeeds.
- `SpawnFireActor` refuses at 80 objects; SpawnNewObj refuses the fire stand state above 50.
- `FireFlash()` (`1942:0f44`): single player, `US_RndT()%15 == 0` → damage flash of `US_RndT()%100`, else bonus flash 18.

---

## 11. State tables

Every record from `DS:0x12EE` to `DS:0x296E`, grouped by owner. Columns are the raw record fields; `think`/`action` names are the ones used in this document. Ally records (`DS:0x2A26`–`0x2BB6`, think `2634:07b8`, action `2634:0214`, death `0x2B46` reuses `A_DeathScream`) are left to the ally investigation. CONFIRMED (parsed from the file; every far pointer has its relocation entry).

#### Grenade (class 19)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `12ee` | flight | 0 | 480 | 3 | T_Projectile | - | self |
| `12fe` | explode | 0 | 656 | 3 | - | - | `130e` |
| `130e` |  | 0 | 657 | 3 | - | - | `131e` |
| `131e` |  | 0 | 658 | 3 | - | - | `132e` |
| `132e` |  | 0 | 659 | 3 | - | A_Remove | self |

#### Slime ball (class 20)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `133e` | flight | 0 | 481 | 3 | T_Projectile | - | self |
| `134e` | splash | 0 | 482 | 6 | - | - | `135e` |
| `135e` |  | 0 | 483 | 6 | - | - | `136e` |
| `136e` |  | 0 | 484 | 6 | - | A_Remove | NULL |

#### Fire (class 21)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `137e` | stand (never spawned) | 0 | 624 | 0 | T_Chase | - | self |
| `138e` | chase | 0 | 624 | 8 | T_Chase | - | `139e` |
| `139e` |  | 0 | 625 | 8 | - | - | `13ae` |
| `13ae` |  | 0 | 626 | 8 | T_Chase | - | `13be` |
| `13be` |  | 0 | 627 | 8 | - | - | `13ce` |
| `13ce` |  | 0 | 628 | 8 | - | - | `13de` |
| `13de` |  | 0 | 629 | 8 | T_Chase | - | `13ee` |
| `13ee` |  | 0 | 630 | 8 | - | - | `13fe` |
| `13fe` |  | 0 | 631 | 8 | - | - | `140e` |
| `140e` |  | 0 | 632 | 8 | - | - | `141e` |
| `141e` |  | 0 | 633 | 8 | T_Chase | - | `142e` |
| `142e` |  | 0 | 634 | 8 | - | - | `143e` |
| `143e` |  | 0 | 635 | 8 | T_FireBurnout | - | `138e` |
| `144e` | attack | 0 | 636 | 8 | - | A_Shoot | `145e` |
| `145e` |  | 0 | 637 | 8 | - | - | `13ae` |
| `146e` | unreachable | 0 | 637 | 8 | - | T_Shoot | `13be` |
| `147e` |  | 0 | 637 | 8 | - | - | `138e` |
| `148e` | burn out | 0 | 638 | 12 | - | - | `149e` |
| `149e` |  | 0 | 639 | 10 | - | A_Remove | self |

#### Rat (class 3)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `14ae` | stand | 1 | 64 | 0 | T_Stand | - | self |
| `14be` | path | 1 | 72 | 20 | T_Path | - | `14ce` |
| `14ce` |  | 1 | 72 | 5 | - | - | `14de` |
| `14de` |  | 1 | 80 | 15 | T_Path | - | `14ee` |
| `14ee` |  | 1 | 88 | 20 | T_Path | - | `14fe` |
| `14fe` |  | 1 | 88 | 5 | - | - | `150e` |
| `150e` |  | 1 | 96 | 15 | T_Path | - | `14be` |
| `151e` | pain | 2 | 104 | 10 | - | - | `156e` |
| `152e` | pain1 | 2 | 108 | 10 | - | - | `156e` |
| `153e` | attack | 0 | 110 | 20 | - | A_Shoot | `154e` |
| `154e` |  | 0 | 111 | 20 | - | - | `155e` |
| `155e` |  | 0 | 112 | 20 | - | - | `156e` |
| `156e` | chase | 1 | 72 | 10 | T_Chase | - | `157e` |
| `157e` |  | 1 | 72 | 3 | - | - | `158e` |
| `158e` |  | 1 | 80 | 8 | T_Chase | - | `159e` |
| `159e` |  | 1 | 88 | 10 | T_Chase | - | `15ae` |
| `15ae` |  | 1 | 88 | 3 | - | - | `15be` |
| `15be` |  | 1 | 96 | 8 | T_Chase | - | `156e` |
| `15ce` | die | 0 | 105 | 15 | - | A_DeathScream | `15de` |
| `15de` |  | 0 | 106 | 15 | - | - | `15ee` |
| `15ee` |  | 0 | 107 | 15 | - | - | `15fe` |
| `15fe` |  | 0 | 109 | 0 | - | - | self |

#### Giant rat (class 7)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `160e` | stand | 1 | 133 | 0 | T_Stand | - | self |
| `161e` | path | 1 | 141 | 20 | T_Path | - | `162e` |
| `162e` |  | 1 | 141 | 5 | - | - | `163e` |
| `163e` |  | 1 | 149 | 15 | T_Path | - | `164e` |
| `164e` |  | 1 | 157 | 20 | T_Path | - | `165e` |
| `165e` |  | 1 | 157 | 5 | - | - | `166e` |
| `166e` |  | 1 | 165 | 15 | T_Path | - | `161e` |
| `167e` | pain | 0 | 185 | 10 | - | - | `16de` |
| `168e` | pain1 | 0 | 186 | 10 | - | - | `16de` |
| `169e` | attack | 0 | 181 | 6 | - | A_Shoot | `16ae` |
| `16ae` |  | 0 | 182 | 20 | - | - | `16be` |
| `16be` |  | 0 | 183 | 10 | - | T_Shoot | `16ce` |
| `16ce` |  | 0 | 184 | 20 | - | - | `16de` |
| `16de` | chase | 1 | 141 | 10 | T_Chase | - | `16ee` |
| `16ee` |  | 1 | 141 | 3 | - | - | `16fe` |
| `16fe` |  | 1 | 149 | 8 | T_Chase | - | `170e` |
| `170e` |  | 1 | 157 | 10 | T_Chase | - | `171e` |
| `171e` |  | 1 | 157 | 3 | - | - | `172e` |
| `172e` |  | 1 | 165 | 8 | T_Chase | - | `16de` |
| `173e` | die | 0 | 173 | 7 | - | A_DeathScream | `174e` |
| `174e` |  | 0 | 174 | 7 | - | - | `175e` |
| `175e` |  | 0 | 175 | 7 | - | - | `176e` |
| `176e` |  | 0 | 176 | 7 | - | - | `177e` |
| `177e` |  | 0 | 177 | 7 | - | - | `178e` |
| `178e` |  | 0 | 178 | 7 | - | - | `179e` |
| `179e` |  | 0 | 179 | 7 | - | - | `17ae` |
| `17ae` |  | 0 | 180 | 0 | - | - | self |

#### Soldier (class 4; classes 8-11 after standing up)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `17be` | stand | 1 | 187 | 0 | T_Stand | - | self |
| `17ce` | path | 1 | 195 | 5 | T_Path | - | `17de` |
| `17de` |  | 1 | 195 | 5 | - | - | `17ee` |
| `17ee` |  | 1 | 203 | 5 | T_Path | - | `17fe` |
| `17fe` |  | 1 | 211 | 5 | T_Path | - | `180e` |
| `180e` |  | 1 | 211 | 5 | - | - | `181e` |
| `181e` |  | 1 | 219 | 5 | T_Path | - | `17ce` |
| `182e` | pain | 0 | 227 | 10 | - | - | `188e` |
| `183e` | pain1 | 0 | 231 | 5 | - | - | `188e` |
| `184e` | attack | 0 | 234 | 10 | - | A_Shoot | `185e` |
| `185e` |  | 0 | 235 | 10 | - | - | `186e` |
| `186e` |  | 0 | 236 | 20 | - | - | `188e` |
| `187e` | alert (FirstSighting) | 0 | 237 | 5 | - | - | `188e` |
| `188e` | chase | 1 | 195 | 5 | T_Chase | - | `189e` |
| `189e` |  | 1 | 195 | 5 | - | - | `18ae` |
| `18ae` |  | 1 | 203 | 5 | T_Chase | A_IdleSound | `18be` |
| `18be` |  | 1 | 211 | 5 | T_Chase | - | `18ce` |
| `18ce` |  | 1 | 211 | 5 | - | - | `18de` |
| `18de` |  | 1 | 219 | 5 | T_Chase | - | `188e` |
| `18ee` | die | 0 | 228 | 9 | - | A_DeathScream | `18fe` |
| `18fe` |  | 0 | 229 | 9 | - | - | `190e` |
| `190e` |  | 0 | 230 | 9 | - | - | `191e` |
| `191e` |  | 0 | 232 | 9 | - | - | `192e` |
| `192e` |  | 0 | 233 | 0 | - | - | self |

#### Disguised soldiers (classes 8-11)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `193e` | stand class 8 | 0 | 113 | 10 | T_Stand | - | self |
| `194e` | stand class 9 | 0 | 118 | 10 | T_Stand | - | self |
| `195e` | stand class 10 | 0 | 123 | 10 | T_Stand | - | self |
| `196e` | stand class 11 | 0 | 128 | 10 | T_Stand | - | self |
| `197e` | stand up class 8 | 0 | 113 | 15 | - | - | `198e` |
| `198e` |  | 0 | 114 | 15 | - | - | `199e` |
| `199e` |  | 0 | 115 | 15 | - | - | `19ae` |
| `19ae` |  | 0 | 116 | 15 | - | - | `19be` |
| `19be` |  | 0 | 117 | 15 | - | - | `188e` |
| `19ce` | stand up class 9 | 0 | 118 | 15 | - | - | `19de` |
| `19de` |  | 0 | 119 | 15 | - | - | `19ee` |
| `19ee` |  | 0 | 120 | 15 | - | - | `19fe` |
| `19fe` |  | 0 | 121 | 15 | - | - | `1a0e` |
| `1a0e` |  | 0 | 122 | 15 | - | - | `188e` |
| `1a1e` | stand up class 10 | 0 | 123 | 15 | - | - | `1a2e` |
| `1a2e` |  | 0 | 124 | 15 | - | - | `1a3e` |
| `1a3e` |  | 0 | 125 | 15 | - | - | `1a4e` |
| `1a4e` |  | 0 | 126 | 15 | - | - | `1a5e` |
| `1a5e` |  | 0 | 127 | 15 | - | - | `188e` |
| `1a6e` | stand up class 11 | 0 | 128 | 15 | - | - | `1a7e` |
| `1a7e` |  | 0 | 129 | 15 | - | - | `1a8e` |
| `1a8e` |  | 0 | 130 | 15 | - | - | `1a9e` |
| `1a9e` |  | 0 | 131 | 15 | - | - | `1aae` |
| `1aae` |  | 0 | 132 | 15 | - | - | `188e` |

#### Saucer drone (class 6)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `1abe` | patrol | 1 | 304 | 1 | T_Path | - | self |
| `1ace` | chase | 1 | 304 | 3 | T_Chase | A_Nop | `1ade` |
| `1ade` |  | 1 | 304 | 3 | - | - | `1aee` |
| `1aee` |  | 1 | 312 | 3 | - | - | `1afe` |
| `1afe` |  | 1 | 320 | 3 | - | - | `1b0e` |
| `1b0e` |  | 1 | 320 | 3 | - | - | `1b1e` |
| `1b1e` |  | 1 | 328 | 3 | T_Chase | - | `1b2e` |
| `1b2e` |  | 1 | 336 | 3 | T_Chase | A_Nop | `1b3e` |
| `1b3e` |  | 1 | 336 | 3 | - | - | `1b4e` |
| `1b4e` |  | 1 | 344 | 3 | - | - | `1b5e` |
| `1b5e` |  | 1 | 344 | 3 | - | - | `1b6e` |
| `1b6e` |  | 1 | 352 | 3 | - | - | `1b7e` |
| `1b7e` |  | 1 | 352 | 3 | T_Chase | - | `1ace` |
| `1b8e` | attack | 0 | 368 | 1 | - | A_Shoot | `1b9e` |
| `1b9e` |  | 0 | 370 | 5 | - | T_Shoot | `1bae` |
| `1bae` |  | 0 | 369 | 1 | - | - | `1ace` |
| `1bbe` | die | 0 | 360 | 7 | - | A_DeathScream | `1bce` |
| `1bce` |  | 0 | 361 | 7 | - | - | `1bde` |
| `1bde` |  | 0 | 362 | 7 | - | - | `1bee` |
| `1bee` |  | 0 | 363 | 7 | - | - | `1bfe` |
| `1bfe` |  | 0 | 364 | 7 | - | - | `1c0e` |
| `1c0e` |  | 0 | 365 | 7 | - | - | `1c1e` |
| `1c1e` |  | 0 | 366 | 7 | - | A_Remove | `1c2e` |
| `1c2e` |  | 0 | 367 | 0 | - | A_Remove | NULL |

#### Black-suit gunman (class 12)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `1c3e` | stand | 1 | 371 | 0 | T_Stand | - | self |
| `1c4e` | path | 1 | 379 | 20 | T_Path | - | `1c5e` |
| `1c5e` |  | 1 | 379 | 5 | - | - | `1c6e` |
| `1c6e` |  | 1 | 387 | 15 | T_Path | - | `1c7e` |
| `1c7e` |  | 1 | 395 | 20 | T_Path | - | `1c8e` |
| `1c8e` |  | 1 | 395 | 5 | - | - | `1c9e` |
| `1c9e` |  | 1 | 403 | 15 | T_Path | - | `1c4e` |
| `1cae` | pain | 0 | 423 | 10 | - | - | `1d0e` |
| `1cbe` | unreachable | 0 | 423 | 10 | - | - | `1cce` |
| `1cce` | attack | 0 | 419 | 20 | - | A_Shoot | `1cde` |
| `1cde` |  | 0 | 420 | 6 | - | - | `1cee` |
| `1cee` |  | 0 | 421 | 20 | - | T_Shoot | `1cfe` |
| `1cfe` |  | 0 | 422 | 6 | - | - | `1d0e` |
| `1d0e` | chase | 1 | 379 | 10 | T_Chase | - | `1d1e` |
| `1d1e` |  | 1 | 379 | 3 | - | A_DebugSparkle | `1d2e` |
| `1d2e` |  | 1 | 387 | 8 | T_Chase | A_IdleSound | `1d3e` |
| `1d3e` |  | 1 | 395 | 10 | T_Chase | - | `1d4e` |
| `1d4e` |  | 1 | 395 | 3 | - | - | `1d5e` |
| `1d5e` |  | 1 | 403 | 8 | T_Chase | - | `1d0e` |
| `1d6e` | die | 0 | 411 | 10 | - | A_DeathScream | `1d7e` |
| `1d7e` |  | 0 | 412 | 10 | - | - | `1d8e` |
| `1d8e` |  | 0 | 413 | 10 | - | - | `1d9e` |
| `1d9e` |  | 0 | 414 | 10 | - | - | `1dae` |
| `1dae` |  | 0 | 415 | 10 | - | - | `1dbe` |
| `1dbe` |  | 0 | 416 | 10 | - | - | `1dce` |
| `1dce` |  | 0 | 417 | 10 | - | - | `1dde` |
| `1dde` |  | 0 | 418 | 0 | - | - | self |

#### Red-vest brute (class 15)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `1dee` | stand | 0 | 452 | 0 | T_Stand | - | self |
| `1dfe` | chase | 0 | 452 | 10 | T_BossChase | - | `1e0e` |
| `1e0e` |  | 0 | 452 | 3 | - | - | `1e1e` |
| `1e1e` |  | 0 | 453 | 8 | T_BossChase | A_IdleSound | `1e2e` |
| `1e2e` |  | 0 | 454 | 10 | T_BossChase | - | `1e3e` |
| `1e3e` |  | 0 | 454 | 3 | - | - | `1e4e` |
| `1e4e` |  | 0 | 455 | 8 | T_BossChase | - | `1dfe` |
| `1e5e` | die | 0 | 458 | 10 | - | A_DeathScream | `1e6e` |
| `1e6e` |  | 0 | 459 | 10 | - | - | `1e7e` |
| `1e7e` |  | 0 | 460 | 10 | - | - | `1e8e` |
| `1e8e` |  | 0 | 461 | 10 | - | - | `1e9e` |
| `1e9e` |  | 0 | 462 | 10 | - | - | `1eae` |
| `1eae` |  | 0 | 463 | 0 | - | - | self |
| `1ebe` | attack | 0 | 456 | 10 | - | - | `1ece` |
| `1ece` |  | 0 | 457 | 10 | - | A_Shoot | `1ede` |
| `1ede` |  | 0 | 456 | 10 | - | - | `1eee` |
| `1eee` |  | 0 | 457 | 10 | - | - | `1efe` |
| `1efe` |  | 0 | 456 | 10 | - | - | `1f0e` |
| `1f0e` |  | 0 | 457 | 10 | - | - | `1dfe` |

#### Trench-coat boss (class 13)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `1f1e` | stand | 0 | 424 | 0 | T_Stand | - | self |
| `1f2e` | chase | 0 | 424 | 10 | T_Chase | - | `1f3e` |
| `1f3e` |  | 0 | 424 | 3 | - | A_IdleSound | `1f4e` |
| `1f4e` |  | 0 | 425 | 8 | T_Chase | - | `1f5e` |
| `1f5e` |  | 0 | 426 | 10 | T_Chase | - | `1f6e` |
| `1f6e` |  | 0 | 426 | 3 | - | - | `1f7e` |
| `1f7e` |  | 0 | 427 | 8 | T_Chase | - | `1f2e` |
| `1f8e` | die | 0 | 432 | 15 | - | A_DeathScream | `1f9e` |
| `1f9e` |  | 0 | 433 | 15 | - | - | `1fae` |
| `1fae` |  | 0 | 434 | 15 | - | - | `1fbe` |
| `1fbe` |  | 0 | 435 | 15 | - | - | `1fce` |
| `1fce` |  | 0 | 436 | 15 | - | - | `1fde` |
| `1fde` |  | 0 | 437 | 15 | - | - | `1fee` |
| `1fee` |  | 0 | 438 | 15 | - | - | `1ffe` |
| `1ffe` |  | 0 | 439 | 0 | T_BossDead | - | self |
| `200e` | attack (loops) | 0 | 428 | 30 | - | A_Shoot | `201e` |
| `201e` |  | 0 | 429 | 10 | - | - | `202e` |
| `202e` |  | 0 | 428 | 10 | - | T_Shoot | `203e` |
| `203e` |  | 0 | 430 | 10 | - | T_Shoot | `204e` |
| `204e` |  | 0 | 431 | 10 | - | T_Shoot | `200e` |

#### Red giant (class 16)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `205e` | stand | 0 | 464 | 0 | T_Stand | - | self |
| `206e` | chase | 0 | 464 | 20 | T_Chase | - | `207e` |
| `207e` |  | 0 | 464 | 6 | - | A_IdleSound | `208e` |
| `208e` |  | 0 | 465 | 8 | T_Chase | - | `209e` |
| `209e` |  | 0 | 466 | 20 | T_Chase | - | `20ae` |
| `20ae` |  | 0 | 466 | 6 | - | - | `20be` |
| `20be` |  | 0 | 467 | 8 | T_Chase | - | `206e` |
| `20ce` | die | 0 | 472 | 10 | - | A_DeathScream | `20de` |
| `20de` |  | 0 | 473 | 10 | - | - | `20ee` |
| `20ee` |  | 0 | 474 | 10 | - | - | `20fe` |
| `20fe` |  | 0 | 475 | 10 | - | - | `210e` |
| `210e` |  | 0 | 476 | 10 | - | - | `211e` |
| `211e` |  | 0 | 477 | 10 | - | - | `212e` |
| `212e` |  | 0 | 478 | 10 | - | - | `213e` |
| `213e` |  | 0 | 479 | 5 | - | - | self |
| `214e` | attack | 0 | 468 | 10 | - | T_Shoot | `215e` |
| `215e` |  | 0 | 469 | 10 | - | T_Shoot | `216e` |
| `216e` |  | 0 | 470 | 10 | - | A_Shoot | `217e` |
| `217e` |  | 0 | 471 | 10 | - | T_Shoot | `218e` |
| `218e` |  | 0 | 469 | 10 | - | T_Shoot | `219e` |
| `219e` |  | 0 | 470 | 10 | - | T_Shoot | `206e` |

#### Gunner (class 14)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `21ae` | stand | 0 | 440 | 0 | T_Stand | - | self |
| `21be` | chase | 0 | 440 | 10 | T_BossChase | A_IdleSound | `21ce` |
| `21ce` |  | 0 | 440 | 3 | - | - | `21de` |
| `21de` |  | 0 | 441 | 8 | T_BossChase | - | `21ee` |
| `21ee` |  | 0 | 442 | 10 | T_BossChase | - | `21fe` |
| `21fe` |  | 0 | 442 | 3 | - | - | `220e` |
| `220e` |  | 0 | 443 | 8 | T_BossChase | - | `21be` |
| `221e` | die | 0 | 446 | 10 | - | A_DeathScream | `222e` |
| `222e` |  | 0 | 447 | 10 | - | - | `223e` |
| `223e` |  | 0 | 448 | 10 | - | - | `224e` |
| `224e` |  | 0 | 449 | 10 | - | - | `225e` |
| `225e` |  | 0 | 450 | 10 | - | - | `226e` |
| `226e` |  | 0 | 451 | 0 | - | - | self |
| `227e` | attack | 0 | 444 | 30 | - | T_Shoot | `228e` |
| `228e` |  | 0 | 445 | 10 | - | A_Shoot | `21be` |

#### Sewer fat man (classes 17 and 22)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `229e` | stand class 17 | 0 | 510 | 0 | T_Stand | - | self |
| `22ae` | stand class 22 (manhole) | 0 | 504 | 0 | T_Stand | - | self |
| `22be` | rise | 0 | 505 | 10 | - | - | `22ce` |
| `22ce` |  | 0 | 506 | 10 | - | - | `22de` |
| `22de` |  | 0 | 507 | 10 | - | - | `22ee` |
| `22ee` |  | 0 | 508 | 10 | - | - | `22fe` |
| `22fe` |  | 0 | 509 | 10 | - | A_ManholeCover | `23ee` |
| `230e` | chase | 0 | 510 | 10 | - | - | `231e` |
| `231e` |  | 0 | 510 | 3 | T_FatChase | A_IdleSound | `232e` |
| `232e` |  | 0 | 511 | 8 | - | - | `233e` |
| `233e` |  | 0 | 512 | 10 | T_FatChase | - | `234e` |
| `234e` |  | 0 | 512 | 3 | - | - | `235e` |
| `235e` |  | 0 | 513 | 8 | T_FatChase | - | `230e` |
| `236e` | die | 0 | 518 | 10 | - | A_DeathScream | `237e` |
| `237e` |  | 0 | 519 | 10 | - | - | `238e` |
| `238e` |  | 0 | 520 | 10 | - | - | `239e` |
| `239e` |  | 0 | 521 | 40 | - | - | `23ae` |
| `23ae` |  | 0 | 522 | 15 | - | - | `23be` |
| `23be` |  | 0 | 523 | 10 | - | - | `23ce` |
| `23ce` |  | 0 | 524 | 10 | - | - | `23de` |
| `23de` |  | 0 | 525 | 10 | - | - | self |
| `23ee` | attack | 0 | 514 | 30 | - | - | `23fe` |
| `23fe` |  | 0 | 515 | 10 | - | T_Shoot | `240e` |
| `240e` |  | 0 | 516 | 10 | - | A_Shoot | `241e` |
| `241e` |  | 0 | 517 | 10 | - | T_Shoot | `242e` |
| `242e` |  | 0 | 516 | 10 | - | T_Shoot | `243e` |
| `243e` |  | 0 | 517 | 10 | - | - | `230e` |

#### Swamp creature (class 23)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `244e` | submerged | 0 | 526 | 0 | T_Path | - | self |
| `245e` | rise | 0 | 527 | 10 | - | - | `246e` |
| `246e` |  | 0 | 528 | 10 | - | - | `247e` |
| `247e` |  | 0 | 529 | 10 | - | - | `248e` |
| `248e` |  | 0 | 530 | 10 | - | - | `249e` |
| `249e` |  | 0 | 531 | 10 | - | - | `25ae` |
| `24ae` | submerge | 0 | 531 | 10 | - | - | `24be` |
| `24be` |  | 0 | 530 | 10 | - | - | `24ce` |
| `24ce` |  | 0 | 529 | 10 | - | - | `24de` |
| `24de` |  | 0 | 528 | 10 | - | - | `24ee` |
| `24ee` |  | 0 | 527 | 10 | - | - | `24fe` |
| `24fe` |  | 0 | 526 | 10 | - | - | `244e` |
| `250e` | chase | 0 | 532 | 10 | T_SwampChase | - | `251e` |
| `251e` |  | 0 | 532 | 3 | - | A_IdleSound | `252e` |
| `252e` |  | 0 | 533 | 8 | T_SwampChase | - | `253e` |
| `253e` |  | 0 | 534 | 10 | T_SwampChase | - | `254e` |
| `254e` |  | 0 | 534 | 3 | - | - | `255e` |
| `255e` |  | 0 | 535 | 8 | T_SwampChase | A_IdleSound | `250e` |
| `256e` | die | 0 | 540 | 10 | - | A_DeathScream | `257e` |
| `257e` |  | 0 | 541 | 10 | - | - | `258e` |
| `258e` |  | 0 | 542 | 10 | - | - | `259e` |
| `259e` |  | 0 | 543 | 10 | - | A_Remove | self |
| `25ae` | attack | 0 | 536 | 30 | - | A_Shoot | `25be` |
| `25be` |  | 0 | 537 | 10 | - | - | `25ce` |
| `25ce` |  | 0 | 538 | 10 | - | - | `25de` |
| `25de` |  | 0 | 539 | 10 | - | A_SlimeThrow | `25ee` |
| `25ee` |  | 0 | 538 | 10 | - | - | `25fe` |
| `25fe` |  | 0 | 539 | 10 | - | - | `250e` |

#### Flame and fireball (class 18)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `260e` | fireball flight | 0 | 501 | 7 | T_Projectile | - | `261e` |
| `261e` |  | 0 | 502 | 7 | T_Projectile | - | `262e` |
| `262e` |  | 0 | 503 | 7 | T_Projectile | - | `260e` |
| `263e` | burning flames | 0 | 485 | 10 | T_Projectile | - | `264e` |
| `264e` |  | 0 | 486 | 10 | T_Projectile | - | `265e` |
| `265e` |  | 0 | 487 | 10 | T_Projectile | - | `266e` |
| `266e` |  | 0 | 488 | 10 | T_Projectile | - | `267e` |
| `267e` |  | 0 | 489 | 10 | T_Projectile | - | `268e` |
| `268e` |  | 0 | 490 | 10 | T_Projectile | - | `269e` |
| `269e` |  | 0 | 491 | 10 | T_Projectile | - | `26ae` |
| `26ae` |  | 0 | 492 | 10 | T_Projectile | - | `26be` |
| `26be` |  | 0 | 493 | 10 | T_Projectile | - | `26ce` |
| `26ce` |  | 0 | 494 | 10 | T_Projectile | - | `26de` |
| `26de` |  | 0 | 495 | 10 | T_Projectile | - | `26ee` |
| `26ee` |  | 0 | 496 | 10 | T_Projectile | - | `26fe` |
| `26fe` |  | 0 | 497 | 10 | T_Projectile | - | `270e` |
| `270e` |  | 0 | 498 | 10 | T_Projectile | - | `271e` |
| `271e` |  | 0 | 499 | 10 | T_Projectile | - | `272e` |
| `272e` |  | 0 | 500 | 10 | T_Projectile | - | `273e` |
| `273e` |  | 0 | 485 | 10 | T_Projectile | - | `274e` |
| `274e` |  | 0 | 486 | 10 | T_Projectile | - | `275e` |
| `275e` |  | 0 | 487 | 10 | T_Projectile | - | `276e` |
| `276e` |  | 0 | 488 | 10 | T_Projectile | - | `277e` |
| `277e` |  | 0 | 489 | 10 | T_Projectile | - | `278e` |
| `278e` |  | 0 | 490 | 10 | T_Projectile | - | `279e` |
| `279e` |  | 0 | 491 | 10 | T_Projectile | - | `27ae` |
| `27ae` |  | 0 | 492 | 10 | T_Projectile | - | `27be` |
| `27be` |  | 0 | 493 | 10 | T_Projectile | - | `27ce` |
| `27ce` |  | 0 | 494 | 10 | T_Projectile | - | `27de` |
| `27de` |  | 0 | 495 | 10 | T_Projectile | - | `27ee` |
| `27ee` |  | 0 | 496 | 10 | T_Projectile | - | `27fe` |
| `27fe` |  | 0 | 497 | 10 | T_Projectile | - | `280e` |
| `280e` |  | 0 | 498 | 10 | T_Projectile | - | `281e` |
| `281e` |  | 0 | 499 | 10 | T_Projectile | - | `282e` |
| `282e` |  | 0 | 500 | 10 | A_Remove | - | self |

#### Explosion (class 24)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `283e` | explosion | 0 | 660 | 8 | T_Projectile | - | `284e` |
| `284e` |  | 0 | 661 | 8 | T_Projectile | - | `285e` |
| `285e` |  | 0 | 662 | 8 | T_Projectile | - | `286e` |
| `286e` |  | 0 | 663 | 8 | T_Projectile | - | `287e` |
| `287e` |  | 0 | 664 | 8 | T_Projectile | - | `288e` |
| `288e` |  | 0 | 665 | 8 | T_Projectile | - | `289e` |
| `289e` |  | 0 | 666 | 8 | T_Projectile | - | `28ae` |
| `28ae` |  | 0 | 667 | 8 | A_Remove | - | self |

#### Face apparition (class 26)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `28be` | face | 0 | 668 | 8 | T_Projectile | - | `28ce` |
| `28ce` |  | 0 | 669 | 8 | T_Projectile | - | `28de` |
| `28de` |  | 0 | 670 | 8 | T_Projectile | - | `28ee` |
| `28ee` |  | 0 | 671 | 8 | T_Projectile | - | `28fe` |
| `28fe` |  | 0 | 672 | 8 | T_Projectile | - | `290e` |
| `290e` |  | 0 | 673 | 8 | T_Projectile | - | `291e` |
| `291e` |  | 0 | 674 | 8 | T_Projectile | - | `292e` |
| `292e` |  | 0 | 675 | 8 | A_Remove | - | self |

#### Debug sparkle (class 25)

| DS addr | role | rot | shape | tics | think | action | next |
|---|---|---|---|---|---|---|---|
| `293e` | sparkle A | 0 | 676 | 3 | T_Projectile | - | self |
| `294e` | sparkle B | 0 | 677 | 3 | T_Projectile | - | self |
| `295e` | sparkle C | 0 | 678 | 3 | T_Projectile | - | self |
| `296e` | sparkle D | 0 | 679 | 3 | T_Projectile | - | self |

---

## 12. Proposed symbol names

For `re/symbols/count_exe.csv` (not edited by this investigation). Confidence is for the name.

| Address | Name | Confidence |
|---|---|---|
| `1497:11f8` | `QuitNum` | CONFIRMED |
| `1823:0106` | `PlaySoundAt` (PlaySoundLocGlobal) | LIKELY |
| `1823:01c8` | `ScanInfoPlane` | CONFIRMED |
| `1942:0bdf` / `0c3b` / `0c9b` | `InitActorList` / `GetNewActor` / `RemoveObj` | CONFIRMED |
| `1942:0f44` | `FireFlash` | LIKELY |
| `1e43:0007` | `SpawnNewObj` | CONFIRMED |
| `1e43:01c3` | `TryWalk` | CONFIRMED |
| `1e43:0cca` | `ActorBumpWall` | HYPOTHESIS |
| `1e43:0d05` / `0e75` / `0fdd` | `SelectDodgeDir` / `SelectChaseDir` / `SelectRunDir` | CONFIRMED |
| `1e43:10b1` | `MoveObj` | CONFIRMED |
| `1e43:12e6` | `DropItem` | CONFIRMED |
| `1e43:1377` | `KillActor` | CONFIRMED |
| `1e43:1710` | `UpdateKillRatio` | HYPOTHESIS |
| `1e43:174d` | `DamageActor` | CONFIRMED |
| `1e43:1982` | `CheckLine` | CONFIRMED |
| `1e43:1e38` | `FirstSighting` | CONFIRMED |
| `1e43:2313` | `SightPlayer` | CONFIRMED |
| `2281:020f` | `PlaceItemType` | CONFIRMED |
| `2281:02a3` | `SpawnStaticAt(index,x,y)` | LIKELY |
| `208c:0494` | `TakeDamage` | LIKELY |
| `208c:0614` | `GivePoints` | LIKELY |
| `2390:0006` / `0125` | `ProjectileTryMove` / `T_Projectile` | CONFIRMED |
| `2390:04ca` / `0741` | `SpawnStand` / `SpawnPatrol` | CONFIRMED |
| `2390:0911` | `SpawnBoss` (class 13) | LIKELY |
| `2390:0ae4` | `SpawnExplosionAhead` | LIKELY |
| `2390:0bd5` | `SpawnFlame` | LIKELY |
| `2390:0c4a` | `A_Remove` | CONFIRMED |
| `2390:0c59` | `A_DeathScream` | CONFIRMED |
| `2390:0f20` / `0f7e` / `0fdc` / `103a` / `1094` | `SpawnGunner` / `SpawnFatMan` / `SpawnRedGiant` / `SpawnManholeFat` / `SpawnSwamp` | LIKELY |
| `2390:10f2` | `SpawnFaceApparition` | LIKELY |
| `2390:1274` | `A_DebugSparkle` | HYPOTHESIS |
| `2390:1508` | `A_SlimeThrow` | LIKELY |
| `2390:1620` | `T_FireBurnout` | LIKELY |
| `2390:179b` / `1966` / `1b31` | `T_BossChase` / `T_FatChase` / `T_SwampChase` | LIKELY |
| `2390:1b17` | `A_ManholeCover` | LIKELY |
| `2390:1d1d` / `1d2d` | `T_Stand` / `T_Chase` | CONFIRMED |
| `2390:1ff4` / `2047` | `SelectPathDir` / `T_Path` | CONFIRMED |
| `2390:21b7` | `A_IdleSound` | LIKELY |
| `2390:2321` | `A_Shoot` | LIKELY |
| `2390:2506` / `26b9` | `T_Shoot` / `T_ShootAlly` | CONFIRMED / LIKELY |
| `2390:28d1` | `T_BossDead` | LIKELY |
| `2390:28df` | `SetHitPoints` | CONFIRMED |
| `2390:2a15` / `2a2f` / `2a49` | `HalveSpeed` / `DoubleSpeed` / `A_Nop` | CONFIRMED |
| `2f4d:0ff3` / `116e` | `SpawnGrenade` / `GrenadeSplash` | LIKELY |
| `2f4d:1388` | `RandomNear` | CONFIRMED |
| `30d6:0012` / `0089` / `0161` | `IsBurnable` / `Ignite` / `SpawnFireActor` | LIKELY |

DGROUP: `DS:0x94ED actorpos[100]`, `DS:0x95B7 actorindex`, `DS:0x3306 bewitched`, `DS:0x330F objlistfull`, `DS:0x32FC allycount`, `DS:0x8716 gamestate.weapon`, `DS:0x8718 gamestate.chosenweapon`, `DS:0x8728 killcount`, `DS:0x872A secrettotal`, `DS:0x872E killtotal`, `DS:0x8CBA loadedgame`, `DS:0xBFEB madenoise`, `DS:0xCD9B thrustspeed`, `DS:0x7BA0 doorobjlist`, `DS:0x7E2A laststatobj`, `DS:0x7E2C statobjlist`, `DS:0x0E68 opposite`, `DS:0x0E7A diagonal`, `DS:0x00DC dirangle` (all three tables byte-identical in meaning to Wolf3D).

---

## 13. Quirks to preserve (candidate `K-###`)

1. SelectDodgeDir acts only 1 time in 5 (§7.1), with the stale-`distance` consequence.
2. HalveSpeed/DoubleSpeed around SelectDodgeDir clears bit 0 of `speed`.
3. Patrollers keep their start tile unless an object stands on the next tile (§6.3).
4. T_ShootAlly damages the ally with an uninitialised value on a miss (§7.9).
5. The trench-coat boss never leaves his attack loop (§8.11).
6. GrenadeSplash covers an asymmetric 4×4 area and ignores its damage argument for actors (§10.1).
7. Direct grenade hits apply splash and direct damage.
8. SpawnStand's ambush-tile replacement can use an uninitialised `tile` (inherited from Wolf3D).
9. A_DeathScream blocks for 12 VBLs while digitized sounds 45–49 play.

## 14. Open items

- **Q-ACT-1** Visible effect of the SelectDodgeDir gate when an actor is mid-tile (§7.1). Approach: DOSBox-X trace of a soldier dodging in the open.
- **Q-ACT-2** Meaning of `difficulty == 3` (scatter mode) and of values 4–6 (network modes?). Approach: menu code in `1618`, network `358f`.
- **Q-ACT-3** Sound id → AUDIOMUS mapping for `PlaySoundAt` (`1823:0106`).
- **Q-ACT-4** `DS:0xCD87` kill-percentage timer has no reader found; `DS:0x0F1C` (flame on kill) has no writer; `DS:0x46A2` (idle-sound gate) meaning.
- **Q-ACT-5** Weapon numbering: confirm that weapon 3 is the flamethrower (DamageActor 2% instant kill, burning death screams).
- **Q-ACT-6** What `FUN_30d6_12b1` (grenade hits a wall/object) and `FUN_2f4d_08e8` (enemy bumps special wall) do; owned by the map/doors investigation.
- **Q-ACT-7** Whether class 13 is the manual's "Victor", and whether the face apparition (class 26) is his face. Approach: oracle playthrough of the last map; manual text.
- **Q-ACT-8** Item numbers 5, 8, 11 dropped by enemies (sprites 31, 37, 30): pickup semantics.
- Ally specifics (`2634`), Q-017.
- Nothing here is trace-confirmed. First traces to take: soldier idle→sight→chase→death, rat bite at range 1, saucer hover, swamp creature cycle, fire spread, trench-coat boss death (playstate 6).
