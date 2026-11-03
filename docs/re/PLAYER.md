# Player control, weapons, pickups, damage and the status bar

Static analysis of `COUNT.EXE` (release `OBC-A`) from the Ghidra text exports in `.local/ghidra/export/`. Where the decompiler fails (most functions in segment `208c` use jump tables that Ghidra splits into fake functions), the instructions were read from a raw 16-bit disassembly of `count.exe` made with `llvm-objdump --triple=i386-unknown-unknown-code16`. Nothing here has been traced in DOSBox-X yet.

- `CONFIRMED` here means **the instructions were read one by one** (AGENT_GUIDE §4.1). It does not mean observed at run time.
- `LIKELY` means the decompiler output or a partial read, or a confident interpretation of confirmed code (for example what a field *means*).
- Addresses: `ghidra SSSS:OOOO` (base segment 0x1000, SYMBOLS.md §1); `DS:0xNNNN` for DGROUP. Initialized DGROUP data was read from the file (file offset = DS + 0x35580). Wolf3D references are `WOLFSRC/<file>:<line>`.
- Names in *italics* are names proposed here; they are not yet in `re/symbols/count_exe.csv`.

Summary of the big differences from Wolf3D:

1. No lives, no extra men, no face. Death in single player is game over (death FLC, high score, menu).
2. Health runs 0–999 (start 400); there is a separate armor value (flak jacket) that absorbs half of each hit.
3. Five weapons: pistol (unlimited), Uzi, Galil, flame thrower, grenade launcher. Ammo is a **30-round magazine** (`gamestate.ammo`) plus a **clip count per weapon** (max 19). The status bar AMMO bar shows clips, not rounds.
4. 14 buttons instead of 8 (five weapon keys, strafe moved to index 13, four extra keys).
5. A difficulty value of 0–6 that doubles as the network game mode (4–6), with unusual damage multipliers.
6. A status bar of segmented bars (health, ammo, armor) plus score, kills, level, ally indicators and a 32×32 event box.

---

## 1. Function map

| ghidra | *Name* (proposed) | Wolf3D counterpart | Confidence | Notes |
|---|---|---|---|---|
| 1942:0006 | *PollKeyboardButtons* | `WOLFSRC/WL_PLAY.C:261` | CONFIRMED | 14 buttons (`buttonscan` DS:0x095E) |
| 1942:0030 | *PollMouseButtons* | `WL_PLAY.C:279` | LIKELY | unchanged |
| 1942:0076 | *PollJoystickButtons* | `WL_PLAY.C` | LIKELY | unchanged |
| 1942:010d | `PollKeyboardMove` | `WL_PLAY.C:341` | CONFIRMED | unchanged |
| 1942:021d | *PollMouseMove* | `WL_PLAY.C:376` | LIKELY | adds forward thrust while bt_run is held |
| 1942:0288 | `PollJoystickMove` | `WL_PLAY.C` | LIKELY | same size as Wolf3D |
| 1942:0400 | *PollControls* | `WL_PLAY.C:455` | LIKELY | demo record/playback removed |
| 1942:04ea | *CheckKeys* | `WL_PLAY.C:614` | LIKELY | cheats, ally orders, F-keys; §3.7 |
| 1942:0f37 / 0f44 / 0f9a | *ClearPaletteShifts* / *StartBonusFlash* / *StartDamageFlash* | `WL_PLAY.C:1143`, `:1157` | CONFIRMED | bonus flash has a 1/15 chance of being a random red flash instead |
| 1942:1315 | *PlayLoop* | `WL_PLAY.C:1368` | LIKELY | |
| 1823:0ea3 | *GameLoop* | `WOLFSRC/WL_GAME.C:1238` | CONFIRMED (dispatch) | §6 |
| 1823:0cda | *Died* | `WL_GAME.C:1114` | CONFIRMED | §6 |
| 1497:0549 | *NewGame* | `WOLFSRC/WL_MAIN.C:276` | CONFIRMED | §2 |
| 2f4d:0f7c | *ResetInventory* | — | CONFIRMED | called by NewGame and network respawn |
| 2f4d:0167 | *SetupLevelPlayerState* | (part of `SetupGameLevel`) | CONFIRMED | per-level resets; §2.3 |
| 1497:1083 | *SetViewSize* | `WL_MAIN.C` | CONFIRMED | §5.6 |
| 208c:0009 | *CalcWeaponOffsets* | — | CONFIRMED | per-viewheight hand-weapon placement |
| 208c:0164 | *CheckWeaponChange* | `WOLFSRC/WL_AGENT.C:117` | CONFIRMED | |
| 208c:01ca | *LoadMagazine* | — | CONFIRMED | ammo = 30 if the weapon has clips |
| 208c:024a | *ControlMovement* | `WL_AGENT.C:149` | CONFIRMED | Ghidra hides it inside 208c:01ca |
| 208c:03bd / 041a | *LatchDrawPic3* / *LatchNumberAt* | — | CONFIRMED | unused copies of 30d6:0ec4 / 0f1b (no callers found) |
| 208c:0494 | *TakeDamage* | `WL_AGENT.C:386` | CONFIRMED | §4.3 |
| 208c:05f4 | *HealSelf* | `WL_AGENT.C:434` | CONFIRMED | cap = maxhealth |
| 208c:0614 | *GivePoints* | `WL_AGENT.C:523` | CONFIRMED | no extra lives |
| 208c:066c | *GiveWeapon* | `WL_AGENT.C:581` | CONFIRMED | |
| 208c:06ab | *GiveClip* (`GiveAmmo`) | `WL_AGENT.C:617` | CONFIRMED | argument is a weapon number |
| 208c:074f | *GetBonus* | `WL_AGENT.C:667` | CONFIRMED | §4.1 |
| 208c:09f7 | *TryMove* | `WL_AGENT.C:801` | CONFIRMED | damaging walls; §3.5 |
| 208c:0cbc | `ClipMove` | `WL_AGENT.C:866` | LIKELY | decompiler output only; matches Wolf3D |
| 208c:0dc7 | *Thrust* | `WL_AGENT.C:928` | CONFIRMED | |
| 208c:0ed2 | *Cmd_Fire* | `WL_AGENT.C:983` | CONFIRMED | |
| 208c:1035 | *StartWallHit* (named `SD_PositionSound` by the matcher, wrongly) | — | CONFIRMED | sets `DS:0x09DA`=1 and the tile |
| 208c:104c | *Cmd_Use* | `WL_AGENT.C:1008` | CONFIRMED | §3.6 |
| 208c:1324 | *SpawnPlayer* | `WL_AGENT.C:1100` | CONFIRMED | |
| 208c:14e0 | *GunAttack* | `WL_AGENT.C:1168` | CONFIRMED | also launches flame and grenades |
| 208c:184c / 1865 | *HasAmmo* / *HasAmmoOrRedraw* | — | CONFIRMED | |
| 208c:1883 | *PlayWeaponSound* | — | CONFIRMED | |
| 208c:18dc | *T_Attack* | `WL_AGENT.C:1293` | CONFIRMED | Ghidra hides it inside 208c:1883 |
| 208c:1b72 | *UseAmmo* | — | CONFIRMED | |
| 208c:1bfb | *T_Player* | `WL_AGENT.C:1393` | CONFIRMED | |
| 208c:1c6a | *StartWallHitAt* | — | LIKELY | called from 1b4a:1494 (renderer) |
| 208c:1d24 | *Reload* | — | CONFIRMED | |
| 208c:1e18 | *GiveSupplyCrate* | — | CONFIRMED | §4.2 |
| 2390:0006 / 0125 | *ProjectileTryMove* / *T_Projectile* | `WOLFSRC/WL_ACT2.C:266`, `:302` | CONFIRMED | §3.10 |
| 2390:0ae4 | *SpawnBoobyTrapBlast* | — | LIKELY | |
| 2f4d:0ff3 | *SpawnGrenade* | — | CONFIRMED | FPU-emulated aiming when a target is given |
| 30d6:0493 / 0547 | *SpawnFlame* (matcher: `maybe_T_SchabbThrow`) | — | CONFIRMED | |
| 2f4d:116e | *GrenadeSplash* | — | CONFIRMED | |
| 30d6:12b1 | *ExplosionTileEffects* | — | CONFIRMED (main paths) | |
| 30d6:0012 / 0089 / 0161 | *IsBurnable* / *BurnTile* / *SpawnFire* | — | CONFIRMED | |
| 30d6:178d | *IsFacingTile* | — | CONFIRMED | |
| 30d6:01ec | *DrawPlayScreen* (status bar part) | `WOLFSRC/WL_GAME.C` `DrawPlayScreen` | CONFIRMED | §5 |
| 30d6:0ec4 | `StatusDrawPic` | `WL_AGENT.C:244` | CONFIRMED | draws into all three pages |
| 30d6:0f1b | *LatchNumber* | `WL_AGENT.C` `LatchNumber` | CONFIRMED | |
| 30d6:083a | *DrawWeaponPic* | `DrawWeapon` | CONFIRMED | |
| 30d6:097e | *DrawAllyCount* (matcher: `DrawWeapon`, wrong) | — | CONFIRMED | |
| 30d6:0aa2 | `DrawScore` | `WL_AGENT.C` | CONFIRMED | |
| 30d6:0abe | *DrawKills* (matcher: `maybe_DrawLives`) | — | CONFIRMED | |
| 30d6:0ad8 | *DrawKillLamp* | — | CONFIRMED | |
| 30d6:0b4a / 0bfd / 0cdd | *DrawHealth* / *DrawAmmo* / *DrawArmor* | `DrawHealth`, `DrawAmmo` | CONFIRMED | segmented bars |
| 30d6:08c0 | *DrawAllyMode* | — | CONFIRMED | |
| 30d6:0923 / 0999 / 09ea | *AnimAllyDeath* / *AnimSparkle* / *ShowItemIcon* | — | CONFIRMED | 32×32 event box |
| 30d6:0fbe / 10c5 / 10d5 | *AddScreenSplat* / *ClearScreenSplats* / *DrawScreenSplats* | — | LIKELY | blood on the "lens" |
| 30d6:1bc0 | *TryExit* | — | CONFIRMED | half the kills needed |
| 2f4d:12b5 / 12de / 124a | *GameMessage* / *DebugMessage* / *DrawGameMessage* | — | CONFIRMED | §5.5 |
| 2b79:15b3 | *SD_PlaySound* | `ID_SD.C` `SD_PlaySound` | LIKELY | sound numbers below are its argument |
| 2ddf:027a | *VL_FadeOut* | `ID_VL.C` `VL_FadeOut` | LIKELY | argument order (start,end,r,g,b,steps) |

---

## 2. `gamestate` (DS:0x86FC, 0x90 bytes)

*NewGame* (`ghidra 1497:0549`) clears `DS:0x86FC`, length 0x90, then sets the starting values. This fixes the base and size. CONFIRMED.

### 2.1 Fields

"Wolf3D slot" names the `gametype` field at the same offset (`WOLFSRC/WL_DEF.H:734`); OBC kept the first part of the layout and appended its own fields.

| DS | +off | Type | Field | Start value | Evidence | Confidence |
|---|---|---|---|---|---|---|
| 0x86FC | +00 | int | `difficulty` — skill 0–3 in single player; 4–6 are network modes (§4.4) | menu, default 1 (`1497:025a`) | NewGame argument; menu `1618:0ad1`–`0b60` cycles 0–6; dozens of `< 4` tests guard network code | field CONFIRMED, meaning LIKELY |
| 0x86FE | +02 | int | `mapon` (0-based) | — | level digit, FLC choice | CONFIRMED |
| 0x8700 | +04 | long | (Wolf3D `oldscore`) | 0 | no direct references found | UNKNOWN |
| 0x8704 | +08 | long | `score`, capped at 1,000,000 | 0 | *GivePoints* `208c:0614` | CONFIRMED |
| 0x8708 | +0C | int | option bits: bit 0 toggled by bt[10] (S), bit 1 by bt[11] (I) | 3 | `2f4d:0bd3`; no reader found | field CONFIRMED, use UNKNOWN |
| 0x870A | +0E | byte | weapons owned, bit *w* = weapon *w* | 3 (pistol, Uzi) | NewGame, *GiveWeapon* | CONFIRMED |
| 0x870C | +10 | int | `health`, 0–999 | `starthealth[difficulty]` | §4.3 | CONFIRMED |
| 0x870E | +12 | int | `ammo`: rounds (or fuel units) left in the loaded magazine, 0–30 | 30 | §3.8 | CONFIRMED |
| 0x8710 | +14 | int | number of carried-item records at 0x875A (max 3) | 0 | `2f4d:04be` | LIKELY |
| 0x8712 | +16 | int | item/status flags; bits 0–6 survive level changes, bit 0x10 = healing-station active, 0x40 = turning toward station, 0x20 used by the map overlay | 0 | `2f4d:049f`, `06ec`, `0d31` | LIKELY |
| 0x8714 | +18 | int | `bestweapon` | 1 | | CONFIRMED |
| 0x8716 | +1A | int | `weapon` (−1 while dying hides the hands) | 1 | | CONFIRMED |
| 0x8718 | +1C | int | `chosenweapon` | 1 | | CONFIRMED |
| 0x871A | +1E | int | unknown; debug key `I` adds 100 capped by 0x872C; saved on warp | 0 | `1adb:0648`, `1823:1187` | UNKNOWN (Wolf3D slot `faceframe`) |
| 0x871C | +20 | int | `attackframe` | 0 | | CONFIRMED |
| 0x871E | +22 | int | `attackcount` | 0 | | CONFIRMED |
| 0x8720 | +24 | int | `weaponframe` | 0 | | CONFIRMED |
| 0x8722 | +26 | int | unknown | 250 | NewGame; only other use is the warp save | UNKNOWN (Wolf3D slot `episode`) |
| 0x8724 | +28 | int | damage immunity: *TakeDamage* skips all damage if nonzero; cleared every level; no setter found | 0 | `208c:0510` | LIKELY |
| 0x8726 | +2A | int | `secretcount` (incremented when a push wall starts moving) | 0 | `2281:0e13` | LIKELY |
| 0x8728 | +2C | int | `killcount` | 0 | status bar, *TryExit* | CONFIRMED |
| 0x872A | +2E | int | `secrettotal` | 0 | `1823:0258` | LIKELY |
| 0x872C | +30 | int | cap for 0x871A ("max fuel"?); Fuel Pack adds 100 (max 999); reset to 100 every level | 100 | | LIKELY (meaning UNKNOWN) |
| 0x872E | +32 | int | `killtotal` | 0 | | CONFIRMED |
| 0x8730 | +34 | long | `TimeCount` (level time, += tics each frame) | 0 | `1942:1652` | LIKELY |
| 0x8734 | +38 | long | `killx` | | `1e43:1538` | LIKELY |
| 0x8738 | +3C | long | `killy` | | | LIKELY |
| 0x873C | +40 | int | cheat mode ("BOGUS MODE") enabled | 0 | `30d6:0350` | CONFIRMED |
| 0x873E | +42 | int | index of the healing station being used | | `2f4d:06ec` | LIKELY |
| 0x8740 | +44 | int | `maxammo`: 200; +30 per supply crate (max 999); only read by the debug key `I` | 200 | | LIKELY (vestigial) |
| 0x8742 | +46 | int | `maxhealth`: cap for *HealSelf*; reset to **100** at every level start | 100 | `2f4d:01ac` | CONFIRMED |
| 0x8744 | +48 | char[8] | score as text (`ltoa`) | | | LIKELY |
| 0x874C | +50 | char[4] | `"%d"` of killtotal − killcount (set at PlayLoop start) | | `1942:1379` | LIKELY |
| 0x8750 | +54 | int | `armor` (flak jacket), 0–999 | 200 at difficulty 0, else 0 | `2f4d:0fa8` | CONFIRMED |
| 0x8752 | +56 | 4 bytes | unknown | | | UNKNOWN |
| 0x8756 | +5A | char[4] | `"%d"` of mapon+1 | | `2f4d:0198` | LIKELY |
| 0x875A | +5E | 3 × 10 bytes | carried-item records {width, height, latch, pic, flag} | | `2f4d:04be` | LIKELY |
| 0x8778 | +7C | int | shots fired this level (counts *T_Attack* code-1 shots only) | | `208c:1a8c` | LIKELY |
| 0x877A | +7E | long | shots fired, whole game | | `2f4d:01d8` | LIKELY |
| 0x877E | +82 | int | hits this level | | | LIKELY |
| 0x8780 | +84 | long | hits, whole game | | | LIKELY |
| 0x8784 | +88 | int[4] | `clips[weapon−1]`: [0] 9 mm (Uzi), [1] 7 mm (Galil), [2] fuel tanks (flame thrower), [3] grenades; each 0–19 | 2, 0, 0, 0 | §3.8 | CONFIRMED |

Starting health `starthealth[difficulty]` is the int table at DS:0x32EC: `400 400 400 400 800 800 800 400` (indices 0–7). CONFIRMED (file read).

### 2.2 Other player globals

| DS | Name | Notes | Confidence |
|---|---|---|---|
| 0x86F6 | `player` (objtype *) | | LIKELY (matcher) |
| 0x86F8 | `newobj` (objtype *) | result of GetNewActor 1942:0c3b | LIKELY |
| 0x86FA | `playstate` | 1 completed, 2 died, 3 warp, 6 victorious, 7 abort to menu | LIKELY |
| 0x92B7 | `killerobj` | | CONFIRMED |
| 0xCD99 | *lastattacker* | set by *TakeDamage* | CONFIRMED |
| 0x9619 | `godmode` | toggled by the `INVINCIBLE` cheat and the debug menu | CONFIRMED |
| 0x95D9 / 0x95F9 | `buttonstate[14]` / `buttonheld[14]` | | CONFIRMED |
| 0x95BF | `anglefrac` | | CONFIRMED |
| 0x95BD | *walkcount* (hand bob index) | ++ per *frame* while moving | CONFIRMED |
| 0xCD9B / 0xCD95 / 0xCD91 | `thrustspeed` / `playerxmove` / `playerymove` (long) | | CONFIRMED |
| 0xCD8B / 0xCD89 | `plux` / `pluy` | | CONFIRMED |
| 0xBFEB | `madenoise` | | CONFIRMED |
| 0x8A1E / 0x8A20 | `shootdelta` (= viewwidth/10) / `centerx` | | CONFIRMED |
| 0x0D40 / 0x0D44 / 0x09D8 | screen shake frames / `TREMORS` option / quake counter | | LIKELY |
| 0x099E / 0x09DE | knock-back trigger / knock-back speed | §3.4 | CONFIRMED |
| 0x09DA / 0x09DC / 0x9419 / 0x9417 | wall-break active / countdown / tile x / tile y | §3.9 | CONFIRMED |
| 0x32FC / 0x2A24 | number of allies / ally order mode | status bar only here | LIKELY |

### 2.3 Lifetime of the fields

- *NewGame* (`1497:0549`): clear; difficulty; weapons = pistol+Uzi; weapon = chosen = best = 1; health = starthealth; ammo = 30; 0x8708 = 3; score text; 0x8722 = 250; *ResetInventory*; 0x872C = 100; maxammo = 200; maxhealth = 100. CONFIRMED.
- *ResetInventory* (`2f4d:0f7c`): carried items cleared; armor = 0 (200 at difficulty 0); 0x8724 = 0; cheat flag cleared; shot/hit totals cleared; clips = {2,0,0,0}; weapon = chosen = best = 1. Note it does **not** clear the owned-weapons bitmask. CONFIRMED.
- Every level start (`2f4d:0167`, unless loading a save): maxhealth = 0x872C = 100; health is **reduced** to starthealth if above it; attackframe = 0; 0x8724 = 0; level shot/hit counts are added to the totals and cleared; Tab-map state reset. CONFIRMED.
- Level completed (`1823:1045`): mapon++, **health += 35** (no cap here; the next level start caps it to starthealth). CONFIRMED.
- Warp (`playstate` 3, `1823:116f`): health, ammo, armor, 0x8722, 0x871A, weapon, weapons owned, best, chosen and the four clip counts are copied to DS:0x467E–0x4697. LIKELY purpose: carried across the warp.

---

## 3. Controls, movement and weapons

### 3.1 Buttons

`buttonscan` defaults at DS:0x095E (14 words), `buttonmouse` DS:0x097A, `buttonjoy` DS:0x0982, `dirscan` DS:0x0956 = {0x48, 0x4D, 0x50, 0x4B}. CONFIRMED (file read). This also decodes the CONFIG.DAT button block (DATA_FORMATS.md §7).

| Index | Default key (scan) | Use in game | Confidence |
|---|---|---|---|
| 0 | Ctrl (0x1D) | attack | CONFIRMED |
| 1 | M (0x32) | no reader found | LIKELY unused |
| 2 | RShift (0x36) | run (×2 keyboard speed); with a mouse also thrusts forward | CONFIRMED |
| 3 | Space (0x39) | use | CONFIRMED |
| 4–8 | 1–5 (0x02–0x06) | ready weapon 0–4 | CONFIRMED |
| 9 | Enter (0x1C) | no reader found | LIKELY unused |
| 10 | S (0x1F) | toggle `gamestate` 0x8708 bit 0 (effect unknown) | CONFIRMED (toggle) |
| 11 | I (0x17) | toggle 0x8708 bit 1 (effect unknown) | CONFIRMED (toggle) |
| 12 | Tab (0x0F) | map overlay: DS:0xC3CC 0 → 1 → −1 | CONFIRMED |
| 13 | Alt (0x38) | strafe | CONFIRMED |

Mouse buttons default {0 attack, 2 run, 3 use, none}; joystick buttons {0 attack, 13 strafe, 3 use, 2 run}. Buttons 10–12 are edge-triggered by hand in `2f4d:0bd3` (they set their own `buttonheld`).

### 3.2 PollControls (`ghidra 1942:0400`)

CalcTics; controlx = controly = 0; `buttonheld = buttonstate`; clear `buttonstate`; keyboard buttons; mouse buttons if `mouseenabled` (DS:0x49A4); joystick buttons if `joystickenabled` (DS:0x49A2); `PollKeyboardMove`; `PollJoystickMove` if enabled; a never-set flag DS:0x95B5 would force bt_use; mouse move if enabled; clamp both axes to ±100·tics. No demo code. LIKELY (decompiler, matches Wolf3D order).

| Constant | OBC | Wolf3D | Source | Confidence |
|---|---|---|---|---|
| BASEMOVE | 35 (0x23) | 35 | `1942:010d` | CONFIRMED |
| RUNMOVE | 70 (0x46) | 70 | `1942:010d` | CONFIRMED |
| mouse | `controlx += mx·10/(13−mouseadjustment)` (DS:0x8A1A, default 5) | same | `1942:021d` | LIKELY |
| mouse + bt_run | `controly −= 35·tics` (walk forward) | — | `1942:021d` | LIKELY |
| max per frame | 100·tics | same | | LIKELY |

### 3.3 ControlMovement and Thrust (`208c:024a`, `208c:0dc7`)

Identical to `WOLFSRC/WL_AGENT.C:149` except that strafe is `buttonstate[13]` and there is no `victoryflag` test. MOVESCALE 150, BACKMOVESCALE 100, ANGLESCALE 20, strafe angle ±90°. CONFIRMED.

*Thrust*: `thrustspeed += speed`; speed capped at 0xAFFF (MINDIST·2−1); move by `FixedByFrac` of cos/sin; *ClipMove*; tile = position >> 16. Area: `areanumber = plane0 − 0x8C`; only if it differs and is < 22 (0x16), the old `areabyplayer[]` entry is cleared and the new one set (Wolf3D does this through doors). No EXITTILE/`VictoryTile` test. CONFIRMED.

### 3.4 T_Player (`208c:1bfb`) and per-frame extras in PlayLoop

*T_Player*: *CheckWeaponChange*; S/I/Tab handler `2f4d:0bd3`; if bt_use then *Cmd_Use* (every frame; *Cmd_Use* does its own edge test); if bt_attack and not held then *Cmd_Fire*; *ControlMovement*; plux/pluy; tile from position. No `UpdateFace`, no victory spin. CONFIRMED.

PlayLoop, before the actors run (`1942:1315`), LIKELY (decompiler):
- *walkcount* DS:0x95BD: if no run button, no forward key and no backward key, it is reset to 0; otherwise incremented once per **frame** (not per tic). It indexes the hand-bob tables (§3.11).
- Knock-back: when not moving, the speed DS:0x09DE decays by 3 toward 0 each frame and `controly += 3 × speed` (pushes the player backward). A booby trap sets DS:0x099E, which loads the speed with 42.
- `health` is clamped to 999 every frame.

### 3.5 Collision: TryMove / ClipMove (`208c:09f7`, `208c:0cbc`)

*TryMove*, CONFIRMED:
- PLAYERSIZE = MINDIST = 0x5800, as in Wolf3D.
- Tile pass over the box: `actorat` nonzero and below 0xA623 (a wall or door marker, not an object pointer) blocks. If the mover is the player and the tile is **0x49–0x4C**, and *IsFacingTile* (`30d6:178d`) says the player faces it, then: 1/5 chance of a screen splat (with sound), `TakeDamage(5, NULL)`, DrawHealth. A damaging wall (barbed wire or similar) hurts on every blocked move attempt while walking into it.
- Actor pass over the box grown by one tile: an object with FL_SHOOTABLE, without flag 0x80 and without bit 1 of byte +0x3A, blocks if within 0x10000 on both axes (MINACTORDIST, as in Wolf3D).
- Debug messages ("actorat not tilemap", position dump) when DS:0xC000 bit 0 is set.

*ClipMove* matches Wolf3D (try x+y, then x only, then y only); `noclip` (DS:0x9617) skips the test inside the map border. LIKELY.

### 3.6 Use (Cmd_Use, `208c:104c`)

The checked tile is the one in front, chosen by angle quadrant (east if angle < 45 or > 315, north < 135, west < 225, else south), as in Wolf3D. CONFIRMED.

Order of tests:
1. Info plane (plane 1) = **0x62** (push wall marker) → `PushWall(x, y, dir)` (`2281:0ce2`), regardless of `buttonheld`.
2. If `buttonheld[bt_use]` → nothing (edge trigger; the network flag DS:0xEF93 can override). Otherwise set `buttonheld[bt_use]`.
3. Door (`tilemap & 0x80`) → `OperateDoor` (`2281:07f3`).
4. By `tilemap` value:

| Tile | Effect | Message / sound | Confidence |
|---|---|---|---|
| 0x71, 0x72 | exit; DS:0x0922 = 0; single player: *TryExit* (below); network: `playstate` = 1 | | CONFIRMED |
| 0x73 | exit, DS:0x0922 = 1 (alternate ending FLC "F4" instead of "F3") | | CONFIRMED |
| 0x58 | medic station: if health < maxhealth, *HealSelf*(25), then the info plane at the tile is cleared (one use) | "Medic Pack", — | CONFIRMED |
| 0x60 | generators on: tile → 0x5F, lighting mode 1 (`2f4d:035e`) | "GENERATORS ON", sound 84 | CONFIRMED |
| 0x5F | generators off: tile → 0x60, lighting mode 2 | "GENERATORS OFF", sound 84 | CONFIRMED |
| 0x55 | toggle the barrier at this tile (table DS:0x46E2, `2f4d:08e8`) | | CONFIRMED (call) |
| 0x0D, 0x10 | switch, sound 81 | | CONFIRMED |
| 0x1D, 0x20 | switch, sound 80 | | CONFIRMED |
| 0x31, 0x34 | switch, sound 78 | | CONFIRMED |
| 0x41, 0x44, 0x45, 0x48 | switch, sound 79 | | CONFIRMED |

After a switch or the medic station: if the info plane there is 0x6A or 0x67, the barrier at that tile is toggled; otherwise the info plane value is cleared. CONFIRMED. (What barriers are belongs to the world/doors topic.)

*TryExit* (`30d6:1bc0`): on map 39 (0x27) nothing happens; otherwise if killcount ≥ killtotal/2 then `playstate` = 1, else the message "UNFINISHED BUSINESS" (DS:0x3468). CONFIRMED.

### 3.7 CheckKeys (`1942:04ea`)

LIKELY (decompiler). Skipped while a modal box is up (DS:0xBFF4) or DS:0x95D5 is set.

| Keys | Effect |
|---|---|
| Esc, F1, F2 | control panel (Wolf3D behavior) |
| F5 / F6 | DS:0x33D2 = 0 / 2 (floor/ceiling mode?) |
| F8 | toggle `TREMORS` (screen shake), message "TREMORS ON"/"TREMORS OFF" |
| F9 | quick action `1618:1210` if difficulty < 5 |
| Pause | "PAUSE" box |
| H / C | toggle DS:0x0D46 bit 0 / bit 6 (overlays drawn by 30d6:0779 / 07d0) |
| F | `2634:000e` |
| A / S (single player) | ally orders: "SWITCH" / "STAYPUT & SWITCH", or "NO ALLIES!" |
| W | clear screen splats |
| Backspace (debug flag DS:0x0954) | debug keys `1adb:0283` (Wolf3D `DebugKeys`); `I` there = GivePoints 100000, HealSelf 100, next weapon, ammo +100 |
| typed "nsr250" (single player) | cheat mode: "BOGUS MODE", also sets the ThermoSensor flag DS:0x3304 |
| Alt+R (cheat mode) | "RESTOCK": GiveWeapon 1–4, all clips = 19 |
| Alt+I (cheat mode) | "INVINCIBLE" / "VULNERABLE" (`godmode`) |
| Alt+H (cheat mode) | "HEALTH HELP": health += 10, capped at starthealth |
| Alt+B (cheat mode) | "BEWITCHED" / "BEWITCHED OFF" (DS:0x3306) |
| Alt+W, Alt+T, Alt+LShift+D | `1942:0b4a`, `30d6:05c4`, `1942:0abb` (not analyzed) |

### 3.8 Weapons and the ammo model

| # | Key | Weapon | Hand sprites (left / right half) | Status pic | Clip pool | Ammo used per attack step | Fire sound | Confidence |
|---|---|---|---|---|---|---|---|---|
| 0 | 1 | pistol (unlimited) | 544–551 / 584–591 | pic34 | — | 0 | 45 | CONFIRMED (code), name LIKELY (sprites) |
| 1 | 2 | Uzi | 552–559 / 592–599 | pic30 | `clips[0]` "9mm" | 1 | 46 | CONFIRMED |
| 2 | 3 | Galil | 560–567 / 600–607 | pic31 | `clips[1]` "7mm" | 2 | 47 | CONFIRMED |
| 3 | 4 | flame thrower | 568–575 / 608–615 | pic33 | `clips[2]` fuel | 1 | 48 | CONFIRMED |
| 4 | 5 | grenade launcher | 576–583 / 616–623 | pic32 | `clips[3]` grenades | 8, and the magazine is emptied after each launch | 49 | CONFIRMED |

Weapon names come from the pickup that gives each weapon (§4.1) and from the sprites. Sprite numbers are the `shapenum` values (`weaponscale` tables DS:0x0D2A and DS:0x0D34); they match the files in `.local/exports/vswap/sprites/` by eye. LIKELY.

Magazine and clips, all CONFIRMED:
- `gamestate.ammo` (0x870E) is one shared magazine of up to 30 for whichever weapon is out. *HasAmmo*(w) = `ammo != 0 || w == 0`.
- *LoadMagazine*(w) (`208c:01ca`), called on every weapon-key press for w ≥ 1: `ammo = clips[w−1] > 0 ? 30 : 0`. No clip is consumed.
- *UseAmmo* (`208c:1b72`) subtracts the per-step amount; at ≤ 0 it sets 0 and calls *Reload*.
- *Reload* (`208c:1d24`): `clips[w−1]−−`; if it is still > 0, `ammo = 30`; otherwise clips = 0 and weapon = chosen = 0, or 1 if the 9 mm count is > 0 (only when w is 2–4). The clip count therefore includes the magazine that is loaded: 2 clips at the start give 60 Uzi rounds.
- *GiveClip*(w) (`208c:06ab`): `clips[w−1]++` (max 19) **and `ammo = 30`**, whatever weapon is out. DrawAmmo.
- *GiveWeapon*(w) (`208c:066c`): if w ≥ bestweapon, then weapon = chosen = best = w; set the owned bit; *GiveClip*(w).

*CheckWeaponChange* (`208c:0164`): for w = 0 … bestweapon, if the weapon is owned and `buttonstate[4+w]` is set: *LoadMagazine*(w); if *HasAmmo*(w), then weapon = chosen = w. If nothing switched, it redraws the weapon picture and the ammo bar (every frame). CONFIRMED.

### 3.9 Firing: Cmd_Fire and T_Attack

*Cmd_Fire* (`208c:0ed2`), CONFIRMED: `buttonheld[0]` = 1; state = `s_attack` (DS:0x0F56); attackframe = 0; attackcount and weaponframe from entry 0. If ammo ≤ 0: DrawAmmo, *Reload*. Then, **weapon 0 only**: if the tile directly in front is a breakable wall, start the wall-break animation (*StartWallHit*):

| Tile in front | Countdown DS:0x09DC |
|---|---|
| 0x39 or 0x75 | 6 |
| 0x3A or 0x76 | 4 |
| 0x3B or 0x77 | 2 |
| 0x3C or 0x78 | 1 |

PlayLoop then, while DS:0x09DA is set, increments that tile number when the countdown is even and decrements the countdown each frame, so these walls always end at 0x3C / 0x78; while the tile is in 0x39–0x3C or 0x75–0x78 the info plane is set to 0x69. Sounds: countdown 5 → sound 65 or 66, countdown 1 → 64, 67 or 68; tiles 0x35 and 0x79–0x7C give 69 or 70 on odd tics. LIKELY (decompiler of PlayLoop).

`208c:1c6a` (*StartWallHitAt*), called from the renderer `1b4a:1494` (which reads `buttonstate[0]`; trigger not analyzed), handles more tiles. CONFIRMED (tables `208c:1cfc`, `1d0e`, `1d1a`):

| Countdown | Tiles |
|---|---|
| 6 | 0x39, 0x75 |
| 4 | 0x3A, 0x76 |
| 2 | 0x13, 0x14, 0x16, 0x17, 0x35, 0x36, 0x3B, 0x77, 0x79, 0x7A, 0x7C, 0x7D |
| 1 | 0x3C, 0x78 |
| unchanged (usually 0, which still gives one increment) | 0x15, 0x37, 0x38, 0x7B |

The effect is a chain of damage stages: each hit advances 0x79 → … → 0x7E or 0x13 → … → 0x18 by one tile number, and 0x35 → 0x39 leads into the glass sequence. LIKELY: bullets chip and break walls (belongs to the world/walls topic).

States: `s_player` DS:0x0F46 = {rot 0, shape 0, tics 0, think 208c:1bfb}; `s_attack` DS:0x0F56 = {think 208c:18dc}. CONFIRMED (file read).

`attackinfo` DS:0x0F66: 5 weapons × 15 entries × 4 bytes {tics, attack, frame, pad}, of which 5 are used. CONFIRMED (file read; the code indexes `weapon*0x3C + attackframe*4`).

| Weapon | Entry 0 | Entry 1 | Entry 2 | Entry 3 | Entry 4 |
|---|---|---|---|---|---|
| 0 pistol | 6, 0, 0 | 6, 2, 1 | 6, 0, 2 | **56**, 0, 3 | 36, −1, 4 |
| 1 Uzi | 6, 1, 0 | 6, 2, 1 | 6, 2, 2 | 6, 3, 3 | 6, −1, 4 |
| 2 Galil | 6, 1, 0 | 6, 2, 1 | 6, 2, 2 | 6, 3, 3 | 6, −1, 4 |
| 3 flame | 6, 0, 0 | 6, 5, 1 | 6, 1, 2 | 6, 4, 3 | 6, −1, 4 |
| 4 grenade | 6, 0, 0 | 6, 0, 1 | **20**, 1, 2 | 6, 3, 3 | 6, −1, 4 |

*T_Attack* (`208c:18dc`), CONFIRMED. As in Wolf3D, the entry processed is the current one, and after it `attackcount += cur.tics`, `attackframe++`, `weaponframe = next.frame` (so frame *k*+1 is shown for tics[*k*] tics). Attack codes:

| Code | Behavior |
|---|---|
| −1 | state = `s_player`. If the weapon has no ammo, step down to the next lower owned weapon that has ammo. Else if weapon ≠ chosen and chosen has ammo, switch back. attackframe = weaponframe = 0; **`buttonstate[0]` and `buttonheld[0]` are cleared**, so holding fire re-triggers *Cmd_Fire* next frame (auto-repeat for every weapon). Returns without *UseAmmo*. |
| 0 | nothing |
| 1 | if *HasAmmo*: weapon sound (not for weapon 3), *GunAttack*, shots++, hits += hit flag, muzzle-light bit DS:0xC3D2 \|= 4; else attackframe++ |
| 2 | *GunAttack* (with the weapon sound before and after for weapon 0 only); light bit |
| 3 | if ammo and fire held: weapons 1 and 2 → attackframe = −1 (restart at entry 0); others → attackframe −= 2. Then weapon 4: `ammo = 0` (forces a reload). Light bit |
| 4 | if ammo: weapon 3 with fire held → attackframe −= 1, other weapons with fire held → −= 2; then as code 1 |
| 5 | weapon sound only |

After every code except −1, *UseAmmo* runs (even for code 0 steps). The light bit is cleared when attackframe becomes odd. Resulting rates at 70 Hz (CONFIRMED arithmetic from the table):

| Weapon | First shot after press | Repeat while held | Ammo per cycle |
|---|---|---|---|
| 0 pistol | 12 tics | 1 shot per 80 tics (cycle ends; auto re-fire) | none |
| 1 Uzi | 6 tics | 3 hitscan shots per 24 tics | 4 rounds |
| 2 Galil | 6 tics | 3 hitscan shots per 24 tics | 8 rounds |
| 3 flame | 18 tics | 1 flame per 6 tics | 1 fuel unit per 6 tics |
| 4 grenade | 18 tics | 1 grenade per 26 tics | 1 grenade (one clip) |

### 3.10 Damage: GunAttack, flame, grenades

*GunAttack* (`208c:14e0`), CONFIRMED. Weapon 4 → *SpawnGrenade*(NULL); weapon 3 → *SpawnFlame*(player, NULL); then return. Weapons 0–2 are hitscan:
- madenoise = 1; target search as Wolf3D (FL_SHOOTABLE and FL_VISABLE, `|viewx − centerx| < shootdelta`, nearest `transx`, `CheckLine` (`1e43:1982`), retry).
- Network modes report the hit or miss and return; damage is applied remotely.
- `dist = max(|dtilex|, |dtiley|)`. `r = US_RndT() % 50`, plus 50 (weapon 0), 20 (weapon 1) or 50 (weapon 2).

| dist | damage |
|---|---|
| < 1 | r + 100 |
| < 2 | r + 75 |
| < 4 | r + 50 |
| < 6 | r + 25 |
| < 8 | r + 10 |
| < 10 | r + 5 |
| ≥ 10 | 20 (no random part, no weapon bonus) |

- Difficulty multiplier on damage dealt: 0 → ×8, 1 → ×2, 2 → ×1, 3 → ×2, others ×1.
- `DamageActor(target, damage)` (`1e43:174d`). There is no Wolf3D long-range miss roll.
- Before the damage, at dist ≤ 2, some enemy classes may splat the screen (*AddScreenSplat* type 2): class 4 and 12 1/35, class 7 1/10, classes 14–16 1/25, class 17 1/15, class 24 1/5 at any distance (table `208c:1822`). CONFIRMED.
- There is no separate knife or `KnifeAttack`.

**Flame** (*SpawnFlame* `30d6:0547`), CONFIRMED: new actor at the player's position, obclass 0x12, state DS:0x260E (sprites 501→502→503, 7 tics each, loop, think `T_Projectile` 2390:0125), dir = nodir, angle = player angle, speed 0x3000 per tic, flags 0x84, +0x3C (target) = NULL.

**Grenade** (*SpawnGrenade* `2f4d:0ff3`), CONFIRMED: obclass 0x13, state DS:0x12EE (sprite 480, 3 tics, loop), speed 0x2000 per tic, flags 0x84, target NULL; DS:0xDD87 = 12. Flies straight: no arc, no bounce, no fuse.

*T_Projectile* (`2390:0125`), CONFIRMED (shared with enemy projectiles):
- Move `speed × tics` along the angle, each axis clamped to +0x10000.
- *ProjectileTryMove* (`2390:0006`): PROJSIZE **0x1000** (Wolf3D 0x2000); blocked by any nonzero `tilemap` byte (so door tiles always block, even open ones), by a wall/door `actorat`, or by any FL_SHOOTABLE actor (network: unless same owner +0x34). It returns the blocking tile.
- Player projectiles have no target, so they only detonate through *ProjectileTryMove*.
  - **Flame blocked**: if *IsBurnable*(tile) then *BurnTile*(tile) and *StartBonusFlash*; the flame is removed. *BurnTile* (`30d6:0089`, single player only): a shootable actor of class 0x0D takes `DamageActor(US_RndT())`; class 1 takes `TakeDamage(30)`; classes 6, 0x15, 0x17 are immune; every other class gets `2390:0bd5` then **`KillActor`** (instant kill). With no actor, every static on the tile with flag bit 1 is replaced by a fire actor (class 0x15, state DS:0x138E, sprites 624–635, 8 tics each; at most 80 objects; sounds 71–73).
  - **Grenade blocked**: *GrenadeSplash*(0, grenade) (`2f4d:116e`): every FL_SHOOTABLE actor whose `actorat` tile is in x ∈ [tx−2, tx+1], y ∈ [ty−2, ty+1] (tx, ty = the grenade's own tile; the box is asymmetric) takes `DamageActor(100 − (|dx|+|dy|))`, with the distance in tiles. The damage argument is used only for the network player: `TakeDamage(arg/2)`. Then, in single player only, *ExplosionTileEffects*, then state DS:0x12FE (explosion sprites 656–659, 3 tics each, then removed by 2390:0c4a).
- *ExplosionTileEffects* (`30d6:12b1`), CONFIRMED main paths:
  - Random sound 74 or 75.
  - Over x, y ∈ [grenade tile −4, +3]: the **player takes `TakeDamage(300)` if standing on the blocking tile, otherwise `TakeDamage(US_RndT() % 200)`** (self-damage, before armor and difficulty scaling). Every FL_SHOOTABLE actor in that 8×8 box is **killed outright** (`KillActor`).
  - Over [blocking tile −2, +1] (inside 1..62): statics are removed and replaced with debris (`2281:02a3` type 0x0C/0x0D).
  - Walls, skipping info plane 0x59 or 0x62 and the exit tile 0x71: breakable walls 0x39–0x3C → 0x3C, 0x75–0x78 → 0x78.
  - The blocking wall itself is destroyed (tile, info and `actorat` cleared, debris, 1/20 chance of fire), unless its info is 0x7D, in which case it becomes tile 0x4D.
  - Other walls in the box: info 0x56/0x57/0x58/0x62 destroyed; info 0x7D untouched; doors untouched; anything else becomes tile 0x4D (scorched).
  - Finally screen shake 8 frames, quake counter 10.

Enemy projectiles that reach their target hit the player for: flame `US_RndT()/2 + 30`, grenade the same (plus splash), class 0x14 `US_RndT()/8 + 30` with a 3/4 chance of a screen splat. LIKELY (only the jump-table paths were read).

### 3.11 Hand weapon drawing (`1b4a:1013`)

CONFIRMED. If weapon = −1 nothing is drawn. Each weapon is two 64×64 sprites drawn with the scaler `1d8b:0a47` at x = viewwidth/2 + `sway[walkcount & 15]`, height = viewheight/4 (DS:0xC01A[w], set in `1d8b:00ec`), the second one shifted by DS:0xC016 bytes of `bufferofs`. The frame is `base + weaponframe` while attacking (buttonheld[0] or attackframe ≠ 0), else `base + bob[walkcount & 15]`.

- `sway` DS:0x31DC = 0 1 2 3 4 3 2 1 0 −1 −2 −3 −4 −3 −2 −1
- `bob` DS:0x31FC = 7 7 6 5 4 5 6 7 7 7 6 5 4 5 6 7 (idle and walk use frames 4–7)

*CalcWeaponOffsets* (`208c:0009`) sets the `bufferofs` offset DS:0xC018 and the second-half offset DS:0xC016 by viewheight:

| viewheight | 88 | 96 | 104 | 112 | 120 | 128 | 136 | 144 | 152 | 160 | 200 | other |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| C018 | 0x6E0 | 0x6D6 | 0x820 | 0x816 | 0x960 | 0x906 | 0xA8C | 0xA46 | 0xE0C | 0xB36 | 0xF96 | 0 |
| C016 | 10 | 12 | 12 | 14 | 14 | 16 | 12 | 18 | 14 | 21 | 24 | 0 |

---

## 4. Pickups, damage and difficulty

### 4.1 GetBonus (`208c:074f`)

Called from the renderer `1b4a:0e05` exactly as in Wolf3D: when *TransformTile* reports the static as grabbable and its flags have bit 1 (FL_BONUS). The switch is on the static's item byte (+8). When picked up, shapenum = −1. CONFIRMED.

| Item | Condition | Effect | Message (DS) | Icon / sound |
|---|---|---|---|---|
| 3 | always | maxammo += 30 (max 999); *GiveSupplyCrate*; DrawAmmo | per crate result (§4.2) | pic47 (9 mm) |
| 4 | grenades < 19 | *GiveClip*(4) | "Grenades!" (0x10CC) | pic44 |
| 5 | always | *GiveWeapon*(2), *GiveClip*(2) | "Galil" (0x10D6) | sound 85 |
| 8 | armor < 400 | armor += 100 (max 999); DrawArmor | "Flak Jacket" (0x10AF) | pic45 |
| 9 | health < 400 | health += 100; **maxhealth = health** (max 999); DrawHealth | "Increased Health" (0x10BB) | pic46 |
| 10 | always | DS:0x3304 = 1 (thermal map mode) | "Map ThermoSensor" (0x10FF) | — |
| 11 | 9 mm clips < 19, or network | *GiveClip*(1) (no weapon is given; the Uzi is owned from the start) | "Uzi" (0x10DC) | sound 85 |
| 12 | always | *GiveWeapon*(3), *GiveClip*(3) | "Flame Thrower" (0x10E0) | — |
| 13 | always | *GiveWeapon*(4), *GiveClip*(4) | "Grenade Launcher" (0x10EE) | — |
| 14 | fuel clips < 19 | 0x872C += 100 (max 999); *GiveClip*(3) | "Fuel Pack" (0x1110) | pic48 |
| 20 | always | **booby trap**: shake 4 frames; blast actor 1 tile ahead (class 0x18, state DS:0x283E, sprites 660–667, 8 tics); `TakeDamage(US_RndT(), NULL)`; knock-back (DS:0x099E = 10) | "BOOBY TRAP" (0x10A4) | sound 75 |
| 6, 7, 15–19 | — | not picked up | | |

"Medic Pack" is not an item: it is the wall station 0x58 (§3.6). Weapons and armor give no score.

### 4.2 Supply crate (*GiveSupplyCrate*, `208c:1e18`)

`r = US_RndT() + 1` (1–256). CONFIRMED.

| r | Chance | Effect | Message |
|---|---|---|---|
| 1–5 | 5/256 | fuel clips +1; **only if that reaches the cap 19**: clips = 19, ammo = 30, message | "Fuel Bottle" (shown only at the cap: probably an inverted test, K candidate) |
| 6–20 | 15/256 | n = US_RndT()/64 + 1 (1–4) grenades, cap 19 (ammo = 30 if capped) | " %d Grenades" / " %d Grenade" |
| 21–64 | 44/256 | n = US_RndT()/37 + 1 (1–7) 7 mm clips, cap 19 | " %d 7mm Clips" / " %d 7mm Clip", sound 85 |
| 65–256 | 192/256 | n = US_RndT()/37 + 1 (1–7) 9 mm clips, cap 19 | " %d 9mm Clips" / " %d 9mm Clip", sound 85 |

### 4.3 TakeDamage (`208c:0494`)

CONFIRMED (`points` = si, `attacker` = di):

```text
if difficulty >= 4: network filter (team mode 5 may ignore; only other players, obclass 0x12/0x13 shots count)
if health >= 30: points *= difficulty/2 + 1
if modal box up (DS:0xBFF4): return
lastattacker = attacker; StartDamageFlash(6)            // fixed 6, not points
if !godmode && !immune(0x8724):
    if armor > 0: points >>= 1 (arith); armor -= points; clamp >= 0; DrawArmor
    switch difficulty: 0,1 -> points *= 2; 2 -> points *= 4; 3 -> points *= 3
    health -= points; clamp 0..999; DrawHealth
if !godmode && health <= 35 && US_RndT() % 15 == 0 && difficulty < 4:
    VL_FadeOut(0, 255, 0, 0, 0, 50)                     // "blackout" (LIKELY: fade-out function)
if health <= 0: (network notify); health = 0; playstate = ex_died; killerobj = attacker
```

Armor model: armor absorbs half the incoming points, health takes the other half (before the difficulty multiplier). Armor goes down by that half. Armor is never checked against the damage, so it absorbs a full half even when it has less left. *HealSelf* caps health at maxhealth, which is 100 after each level start and is raised only by "Increased Health"; the medic wall (HealSelf 25) and the healing stations (`2f4d:06ec`, HealSelf(tics) per frame from a per-station pool) therefore never heal above 100 unless that item was taken on the level. *HealSelf* itself clamps `health` down to maxhealth, so an unguarded call with health 400 would drop it to 100; the medic wall and the stations only call it while health < maxhealth, but the debug key `I` calls HealSelf(100) unguarded (CONFIRMED, `208c:05f4`, `1adb:0618`).

### 4.4 Difficulty and game mode summary

| Value | Start health | Start armor | Damage taken (health ≥ 30) | Damage taken (health < 30) | Player damage dealt | Notes |
|---|---|---|---|---|---|---|
| 0 | 400 | 200 | ×2 | ×2 | ×8 | |
| 1 | 400 | 0 | ×2 | ×2 | ×2 | default |
| 2 | 400 | 0 | ×8 | ×4 | ×1 | |
| 3 | 400 | 0 | ×6 | ×3 | ×2 | *SpawnPlayer*: if the start tile is occupied, use a free neighbor or a random free tile |
| 4–6 | 800 | 0 | ×3 (4, 5) / ×4 (6) | ×1 | network path | network modes; 5 = team mode (friendly-fire check), ≥ 6 enables Alt ally keys |

The multipliers are CONFIRMED from code; the labels of 0–6 are LIKELY.

---

## 5. Status bar

### 5.1 Pictures and fonts

Picture arguments are VGAGRAPH chunk numbers; the exported `picNN` = chunk − 3. CONFIRMED (status bar chunk 32 = `pic29_320x40`).

| Chunk | Export | Use |
|---|---|---|
| 3 | pic00 8×8 | background tile filling all three pages (40×25 tiles) |
| 12 | pic09 | blank digit |
| 13–22 | pic10–19 | digits 0–9 (8×8) |
| 28–31 | pic25–28 | ally-order letter: 0x1C (mode 0x1B), 0x1D (mode 1), 0x1E (mode 0), 0x1F (no allies) |
| 32 | pic29 | status bar 320×40 at (0,160) |
| 33–38 | pic30–35 | weapon pictures 64×32; 38 = blank |
| 39 | pic36 | blank 32×32 event box |
| 40–45 | pic37–42 | ally-death animation |
| 47–51 | pic44–48 | item icons |
| 53–55 | pic50–52 | sparkle animation |

### 5.2 Layout (pixels; x is the LatchDrawPic argument × 8)

| Element | Position | Function | Rule |
|---|---|---|---|
| status bar | (0,160) | *DrawPlayScreen* `30d6:01ec` | |
| event box 32×32 | (8,168) | §5.4 | |
| weapon picture | (48,168) | *DrawWeaponPic* `30d6:083a` | by `chosenweapon`: 0→pic34, 1→pic30, 2→pic31, 3→pic33, 4→pic32, other→pic35 |
| ally order letter | (112,170) | `30d6:08c0` | |
| ally count digit | (128,171) | `30d6:097e` | digit = DS:0x32FC |
| kill lamp | bar (116,188) 4×6 | `30d6:0ad8` | single player only: color 0x0E if kills ≥ total, 2 if kills ≥ total/2 (exit open), else 1 |
| difficulty digit | (128,187) | `30d6:01ec` | digit difficulty+1 (0–3), digit 5 for network |
| health bar | y = 174 | `30d6:0b4a` | |
| ammo bar | y = 182 | `30d6:0bfd` | value = 19 for the pistol, else `clips[weapon−1]` (clips, not rounds) |
| armor bar | y = 190 | `30d6:0cdd` | |
| score | (256,171), 7 digits | `DrawScore` | |
| kills | (256,187), 3 digits | `30d6:0abe` | |
| level | (288,187), 3 digits | `30d6:01ec` | mapon+1 |

Bars, CONFIRMED: 19 segments at x = 171 + 4i, each 3×4, drawn with `VWB_Bar` into all three pages. Lit count: health and armor = value/19 + 1 (so at least one segment is lit, and every value ≥ 342 shows full); ammo = clip count. Color by lit count: > 14 → 0x0E, 7–14 → 0x02, ≤ 6 → 0x01; unlit = 0. These are palette indices; colors were not checked.

*LatchNumber* (`30d6:0f1b`): `ltoa`, right-aligned in `width` cells, leading blanks, digit chunk = char − 0x23. CONFIRMED.

### 5.3 What redraws when

DrawHealth on damage and heal; DrawArmor on damage and flak jacket; DrawAmmo on reload, clip pickup and weapon change (and every frame from *CheckWeaponChange* when no weapon key is pressed); score on *GivePoints*; kills from `KillActor`. CONFIRMED (callers).

### 5.4 The event box (8,168)

CONFIRMED (calls from `1b4a:129c`, the per-frame view refresh):
- Item icon (*ShowItemIcon* `30d6:09ea`): item 3 → pic47, 4 → pic44, 8 → pic45, 9 → pic46, 14 → pic48, others → blank. It stays 24 frames (DS:0x330E = 25, decremented per frame), then blank.
- Ally death (an ally killed, `1e43:144b`): pics 37–42 advance every third frame, then blank; the ally count is decremented and redrawn.
- Sparkle (DS:0x330F): set by PlayLoop every 301 frames (counter DS:0x09E0 > 300, `1942:1917`) and by GetNewActor when the object list is full (`1942:0c4b`). Shows pics 50, 51, 52, then blank, one step per frame, only while no other box animation runs.

There is no BJ face and no `UpdateFace` (Wolf3D `WL_AGENT.C:270`, `:307`).

### 5.5 In-game messages

*GameMessage*(fmt, …) (`2f4d:12b5`): `vsprintf` into DS:0x46A6, timer DS:0x46A4 = **125 tics**. *DebugMessage* (`2f4d:12de`): only with the debug flag DS:0x0954, timer 31 tics, color 0xDF. *DrawGameMessage* (`2f4d:124a`, every frame after the hand weapon): timer −= tics; while > 0, draw at view-relative (6,6) in color 0 (shadow), then at (5,5) with the second proportional-font routine (`2e72:0569`). A new message replaces the old one. CONFIRMED.

### 5.6 View size

*SetViewSize*(w, h) (`1497:1083`), CONFIRMED: if h > 160 then the view is 320×200 with `screenofs` 0 (full screen; the status bar area is covered). Otherwise width = w & ~15, height = h & ~1, `screenofs = ((160−h)/2)·80 + (320−w)/8`. Then `centerx = w/2 − 1`, `shootdelta = w/10`, `CalcProjection(0x5700)`. The status bar is always drawn at y = 160 by *DrawPlayScreen*; whether anything hides it in full-screen mode was not checked (UNKNOWN).

Other view effects: screen shake (DS:0x0D40 frames, uses CRTC start-address offsets DS:0x0D47/0x0D51; only with TREMORS on, health ≥ 20, single player); screen splats (`30d6:0fbe`, up to 19 sprites 640–647 drawn over the view). LIKELY.

---

## 6. Death and what follows

CONFIRMED unless noted.

1. *TakeDamage* sets `playstate` = 2 (ex_died) and `killerobj`. PlayLoop exits at the end of the frame; for `playstate` 2 it sets clips = {2,0,0,0} (used by network respawn) and skips `FinishPaletteShifts`.
2. *GameLoop* (`1823:0ea3`) dispatches `playstate − 1` through the table at `1823:11e4`: 1 and 9 → level completed, 2 → died, 6 → victory, others → `1823:1168` (warp save for 3).
3. *Died* (`1823:0cda`): weapon = −1 (no hands); random death sound from DS:0x0918 = {22, 23, 24, 25, 26}; if a killer exists, turn toward it with `atan2` (FPU emulation) at 4°/tic, redrawing the view each step (as in Wolf3D); sound cleanup, FinishPaletteShifts, lighting mode 1.
4. Network: respawn through `358f:0d7b` / `358f:1796` and continue the same level.
5. Single player: fades, plays FLC "F6" (DS:0x094E) through `32a2:0bcb`, lighting mode 3, then the high-score check `2f4d:13ed`; if the score qualifies, name entry `2f4d:1422`. Return to the menu. **No lives: one death ends the game.** Wolf3D's lives logic (`WL_GAME.C:1114`) is gone.

For comparison, level completion plays FLC "F2" after maps 0–4, "F5" after map 22, otherwise "F3" (exit 0x71/0x72) or "F4" (exit 0x73), then *LevelCompleted* stats (single player); victory plays "F7" (DS:0x0951). LIKELY: the names at DS:0x0942–0x0951 are FLC files.

---

## 7. Quirks to keep (candidates for DELTA_SPEC K-entries)

All CONFIRMED in code; their in-game effect is unverified.

1. Pressing the key of an owned weapon calls *LoadMagazine* first: if that weapon has no clips the current magazine is set to 0 even though the switch fails; if it has clips the magazine refills to 30 without spending a clip (free reload by re-selecting).
2. *GiveClip* refills the loaded magazine to 30 whatever weapon is out.
3. *UseAmmo* runs on every attack step, including code-0 steps (Uzi uses 4 rounds per 3 shots).
4. Supply-crate "Fuel Bottle" only appears (and refills) when the fuel count reaches the cap.
5. Hand bob (`walkcount`) and the event-box animations advance per frame, not per tic.
6. *GrenadeSplash* scans an asymmetric 4×4 box; *ExplosionTileEffects* uses an asymmetric 8×8 box and kills every shootable actor in it.
7. Projectiles stop on door tiles even when the door is open (`tilemap` test).
8. Health and armor bars saturate at about 342 while values run to 999.
9. Pistol cycle: entry 3 holds the raised frame for 56 tics (one shot per 80 tics).
10. *HealSelf* clamps health *down* to maxhealth (100 on most levels); only caller guards prevent losing health by healing.
11. *TryExit* needs half the kills; the kill lamp shows this state.

## 8. Open questions

- Meaning of `gamestate` 0x8708 (S/I toggles), 0x871A, 0x8722, 0x872C, 0x8740; who sets 0x8724 and DS:0x95B5.
- Names of difficulty values 0–6 (menu pictures not checked).
- `DamageActor` (`1e43:174d`) and `KillActor` (`1e43:1377`) belong to the actor topic; the final damage to enemies depends on them.
- The barrier system (DS:0x46E2 table, info codes 0x55/0x67/0x6A) and the healing stations (DS:0x4866) need their own write-up.
- Lighting modes of `2f4d:035e` and the overlays toggled by H, C and Tab.
- What the renderer path `1b4a:1494` → `208c:1c6a` does for hitscan shots into walls.
- Run-time confirmation: fire rates, damage numbers and the blackout fade should be checked in DOSBox-X.
