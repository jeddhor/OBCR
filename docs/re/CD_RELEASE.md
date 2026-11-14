# The CD release (`BCCD.EXE`)

The Steam and GOG editions ship the CD release as a BIN/CUE disc image. Its executable, `BCCD.EXE`, is a later build than the floppy `COUNT.EXE` (release OBC-A). This file records how the two differ.

**Address conventions.** Floppy addresses are Ghidra addresses in `COUNT.EXE`, as everywhere else in `docs/re/`. CD addresses are written `cd:SEG:OFF` and use the coordinates of `tools/re/disasm16.py` run on `BCCD.EXE`. That tool assumes the floppy's 0x3400-byte header, but `BCCD.EXE`'s header is 0x3600 bytes. So a tool address's linear value minus 0x200 is the image-relative linear address that far pointers and far calls in `BCCD.EXE` use. Far-call targets quoted from CD code (`lcall 20b2:048e`) are in that real, image-relative form. CD DS offsets are written `cd DS:0x....`.

Method (section 3). Every Ghidra function of the floppy build was matched to its CD twin by instruction skeleton: mnemonics and operand shapes, with numbers ignored, so moved globals do not count as changes. The differing pairs were then compared side by side. 728 functions are skeleton-identical. The rest were compared by hand where they matter for the simulation; see the list at the end of section 3.

## 1. Identification

Release id **OBC-CD** (`obc_formats::release::OBC_CD`, sizes and SHA-256 of every file). CONFIRMED by reading the Steam image `BODYCOUNT.cue`/`.bin`:

- **Disc layout.** One MODE1/2352 data track, then four Red Book audio tracks (2–5, INDEX 01 at 06:12:52, 14:01:68, 19:44:66 and 26:55:56; no pregaps). The ISO9660 file system is flat: every file is in the root.
- **Same as OBC-A** (byte-identical): `MAPHEAD.BC`, `MAPTEMP.BC`, `VGAHEAD.BC`, `VGADICT.BC`, `VGAGRAPH.BC`, `AUDIOHED.BC`, `AUDIOT.BC`, `AUDIOMUS.BC`, `F2.BC`, `F3.BC`, `F4.BC`, `F6.BC`, `SETUP.EXE`, `README.EXE`, `BCLOG.BAT` and `PRINTLOG.BAT`.
- **Different:** `BCCD.EXE` (instead of `COUNT.EXE`), `GFXTILES.BC`, `README.TXT` and `BC.BAT` (which runs `bccd %1 %2 %3 %4`).
- **CD only:** `F1.BC` (456 frames, the opening movie), `F5.BC` (177), `F7.BC` (450), `GFXINFOV.BC` (46 bytes, not used by OBCR) and `INSTALL.EXE`.
- **`GFXTILES.BC`.** 925 chunks instead of 937: the same walls; **668 sprites instead of 680**, because the floppy's last twelve (668–679, face apparition and sparkles) are gone; the same digitized sounds. 386 sprite chunks have the same index and length but different pixels (retouched art). Sprite numbers in the state tables are unchanged.

The Steam edition also ships the four music tracks as `02-Track 2.ogg` .. `05-Track 5.ogg`. OBCR rips them from the image instead (ADR-022), which needs nothing but the `.cue`/`.bin`.

## 2. Data tables

CONFIRMED by content matching (scratch scripts comparing the two executables; the values are in `obc_sim::data::OBC_CD` and `obc_formats::release::OBC_CD`) and by `crates/obc-sim/tests/cd_data.rs`, which loads both executables and compares every table.

`BCCD.EXE`: MZ header 0x3600 bytes; **DGROUP (DS:0) at file 0x35C20**.

| Table | OBC-A | OBC-CD | Notes |
|---|---|---|---|
| `righttable` / `lefttable` | DS:0x0594 / 0x0756 | DS:0x056C / 0x072E | identical |
| Song per map | DS:0x098A | DS:0x0962 | identical (unused with `cdmusic`) |
| Ramps, `Rindex`/`Findex`/`Rclr`/`Fclr` | DS:0x09A4, 0x09E6.. | DS:0x097C, 0x09BE.. | identical |
| `attackinfo` | DS:0x0F66 | DS:0x0F6C | identical |
| `statinfo` | DS:0x11DA | DS:0x11E0 | entry 48 (map object 71, sprite 49) has kind 17 (**blocks**) instead of 0 |
| Floor/ceiling sets | DS:0x33BE | DS:0x32D8 | identical |
| States: player | DS:0x0F46..0x0F56 | DS:0x0F4C..0x0F5C | |
| States: enemies, effects | DS:0x12EE..0x296E (361) | DS:0x12F4..0x28B4 (349) | the last 12 (face, sparkles) are missing |
| States: allies | DS:0x2A26..0x2BB6 | DS:0x296C..0x2AFC | |
| `gamepal` | file 0x2DDB0 | file 0x2E450 | identical |
| Dark palette | file 0x2E0B0 | file 0x2E750 | colour 2 differs (two bytes) |
| Picture palette | file 0x2E3B0 | file 0x2EA50 | identical |
| Default high scores | file 0x383E4 | file 0x38994 | identical |

**State-ID translation.** OBCR keeps OBC-A's DS offsets as state identities for both releases and translates the CD's when loading (`ExeLayout::canonical_state`): CD state `X` is floppy state `X − 6` for `0xF4C..0x28B4`, and `X + 186` for `0x296C..0x2AFC`. The CD has no counterpart of the floppy states `0x28BE..0x296E` (sprites 668–679: the face apparition and the sparkles). Every CD record's `next` translates to the floppy record's `next` (0 mismatches), and rotate, shape and tics agree except in the records listed below.

**Function pointers.** Pairing the aligned records' far pointers gives the CD addresses of every state function (image segment + 0x1000, as Ghidra numbers them): the actor functions are in segment 23B5 (floppy 2390), the player's in 20B2 (208C), the allies' in 2623 (2634). The table is `OBC_CD.funcs`. `A_DebugSparkle` and `A_ManholeCover` are referenced by no CD state.

**Records that differ** (floppy numbering): the saucer's attack `0x1B8E`, `0x1B9E` and `0x1BAE` (§3.5), `0x1D1E` (no `A_DebugSparkle`) and `0x22FE` (no `A_ManholeCover`) (§3.15).

## 3. Code deltas

"Sim" means OBCR's simulation must branch on the release. "Data" means the difference already comes from the CD's tables once they are loaded. "UI" means presentation only.

### 3.1 `TakeDamage` (floppy 208c:0494, cd:211a:000e) — CONFIRMED, Sim

- **Damage flash.** The floppy calls `StartDamageFlash(6)` unconditionally, right after recording the attacker. The CD removes that call. Instead, after health has been reduced, it calls `StartDamageFlash(points / 5)` (cd:211a:00f7–0100). `points` is the final amount after the armour halving and the difficulty multiplier. So in the CD:
  - God mode and the immunity flag give no flash.
  - Small hits under 5 points give none.
  - Larger hits flash more strongly.
- **Blackout roll.** The 1-in-15 roll that fades to black at health ≤ 35 (cd:211a:0108) no longer checks god mode or the game type. In god mode it still makes its `US_RndT` call. It also fires in network games. The floppy skips the roll in god mode (no RNG call) and fades only in single player (208c:059e…05ac).
- Everything else is unchanged: the network filter, the ×(difficulty/2+1) scaling at health ≥ 30, armour, the difficulty multipliers, the clamp to 0..999, and death.

### 3.2 `StartDamageFlash`, `StartBonusFlash` (floppy 1942:0f9a / 1942:0f44, cd:1a74:006e / cd:1a74:003e) — CONFIRMED, UI (+RNG)

- **Damage flash.** The CD's `StartDamageFlash(n)` is Wolf3D's plain `damagecount += n`. The floppy first clears both palette shifts (1942:0f37) and has a debug print. In the CD, repeated hits therefore add up. Red level = `min(damagecount/10 + 1, 4)`, decaying by `tics`. `UpdatePaletteShifts` is unchanged (1942:0fc9 = cd:1a7a:001a).
- **Bonus flash.** The CD's `StartBonusFlash` drops the floppy's `gametype ≥ 4 → return` (1942:0f47). In network games it therefore also rolls `US_RndT`: 1 in 15 is a red `rnd % 100` flash, otherwise a white flash of 0x12.

### 3.3 `T_Projectile` hit handling (floppy 2390:0125, cd:23e7:0001) — CONFIRMED, Sim

The wall-hit branch is unchanged. In the branch where the projectile is within 0xC000 of its target, the floppy's jump table for classes 18–25 (2390:038d, table 2390:04ba) becomes an `if` chain: classes 18, 19 and 20 get their own code, and everything else, including 25, goes to the common damage code (cd:23e7:0269–027e).

| Class | Floppy | CD |
|---|---|---|
| 18 flame | `Ignite(tilex, tiley)`, `StartBonusFlash`, damage `rnd>>1 + 30` to the player / `rnd + 30` to an actor | `Ignite`, `StartBonusFlash`, **no damage roll**: the damage passed on is the uninitialised local `[bp−0xA]` (cd:23e7:0316–0329, then 032b). No `US_RndT` call. Value UNKNOWN (stack leftover); needs observation in DOSBox-X. |
| 19 grenade | single player: `ExplosionTileEffects`; damage `rnd>>1 + 30` to the player / `rnd + 30` to an actor; `GrenadeSplash(dmg)` | single player: `ExplosionTileEffects`; damage **`rnd>>3 + 30`** to the player / **`rnd × difficulty + 80`** to an actor (`imul gamestate.difficulty`, cd:23e7:02f9–0305); `GrenadeSplash(dmg)` |
| 20 slime | unchanged: `rnd>>3 + 30` / `rnd + 25`, then the 3-in-4 screen splat | same |
| 25 sparkle | `state = NULL`, no damage | no special case (sparkles do not exist in the CD) |

After the table, both builds apply the damage: `TakeDamage` for the player, `DamageActor` otherwise, then `state = NULL`.

### 3.4 `GrenadeSplash` (floppy 2f4d:116e, cd:307f:000c) — CONFIRMED, Sim

The same box, `[tx−2, tx+2) × [ty−2, ty+2)`, and the same `actorat` and shootable tests. What changed:

- **Actors** take `dmg >> (|Δtilex| + |Δtiley|)` (cd:307f:0089–0094). The floppy deals `100 − (|Δtilex| + |Δtiley|)` and ignores `dmg`. The shift count is the byte sum of the two distances, at most 4.
- **The player** (network games only) takes the full `dmg` instead of `dmg / 2` (cd:307f:00b9).
- **Consequence.** A grenade that hits a wall in single player passes `dmg = 0` (2390:02c8 = cd:23e7:01a4, unchanged), so its splash calls `DamageActor(obj, 0)`. Actors in the 8×8 box are still killed by `ExplosionTileEffects`.

### 3.5 `T_Shoot` (floppy 2390:2506, cd:25d1:0200) — CONFIRMED, Sim

- **Class 6 (saucer).** The floppy switch has a class-6 case that shoots only at distance < 2. The CD removes it, so the saucer takes the default branch: it fires at any distance, with damage `rnd>>2`, `>>3` or `>>4` by distance (cd:25d1:02fb–0308, 037b).
- Classes 3, 7 and 21 and the hit chance are unchanged.
- The saucer's attack states also change (Data): floppy `0x1B8E` shape 368/1 tic/`A_Shoot`, `0x1B9E` shape 370/5 tics/`T_Shoot`, `0x1BAE` shape 369/1 tic. CD: 368/6/`A_Shoot`, 369/6/`T_Shoot`, 370/6.

### 3.6 `ExplosionTileEffects` (floppy 30d6:12b1, cd:3261:0002) — CONFIRMED, Sim

- The wall loop's first skip test drops info-plane value **0x62** (push-wall marker) and keeps only 0x59 (floppy 30d6:1457, missing after cd:3261:019f).
- Push walls inside the blast box are therefore no longer spared. They reach the later "info 0x56/0x57/0x58/0x62 → destroyed" test (cd:3261:03f8), so **a grenade blows push walls open**.
- Nothing else changed: the `US_RndT` calls are in the same places, and the shake is 8 and the tremor 10.

### 3.7 `TryExit` (floppy 30d6:1bc0, cd:32f1:0008) — CONFIRMED, Sim (message)

On map 39 the floppy returns silently. The CD shows the message **`LAST FLOOR`** (cd DS:0x340C) and then returns. `UNFINISHED BUSINESS` is unchanged.

### 3.8 `Cmd_Use` (floppy 208c:104c, cd:21b6:…) — CONFIRMED, UI

The generator switches (tiles 0x60/0x5F) no longer call `FinishPaletteShifts` before `SetPaletteMode`. A running red or white flash is no longer cut off when the lights are toggled.

### 3.9 `CheckKeys` (floppy 1942:04ea, cd:19cd:0003) — CONFIRMED

- **New CD-audio keys** at the top, each only while `cdmusic` (cd DS:0x2CFA) is set. See §4.
  - **Alt+P**: `CD : PLAY`, then `CD_NextTrack(0)`.
  - **Alt+S**: `CD_Stop`, then `CD : STOP`.
  - **Alt+F**: `CD : ADVANCE TRACK`, then `CD_NextTrack(1)`.
  - Keyboard array: floppy `Keyboard` = DS:0x8F22, cd DS:0x8ED2.
- **Removed:** Alt+T (sparkle spawn, 30d6:05c4) and Alt+B (BEWITCHED, 30d6:0456). The CD still has the bogus-mode keys W, R, I and H, and Alt+LShift+B / Alt+LShift+D. BEWITCHED cannot be switched on in the CD, and the sparkle cannot be spawned. (Sim: `Cheat::Bewitched` is unavailable for the CD.)
- **F9 QuickSave** is no longer limited to game type ≤ 4 (floppy 1942:07f2; CD calls it unconditionally, cd:19cd:035e). UI.
- **Pause text** `PAUSE` is drawn at y = `200 − viewheight-ish`, x = `viewwidth/2 − 2` (two globals, cd DS:0x89D2 and 0x89D4) instead of the fixed (0x87, 0x50). UI.
- **Debug mode:** a new key **T** (with `debugmode`, cd DS:0x92C) prints the number 102 through a message routine (cd:15de:0004). Debug only.

### 3.10 `GameLoop` (floppy 1823:0ea3, cd:1946:0002) — CONFIRMED

- **Level completed:** `AutoSave` (floppy 1618:10de, cd real 161d:121d) now runs in network games as well; the floppy skips it for game type ≥ 4. UI/save.
- The sequence choice is unchanged: `F2` after maps 0–4 and `F5` after map 22; then `F3`/`F4` by exit kind in single player.
- **CD music:** if `cdmusic` is set and no track is playing, `CD_Play(track)` runs after the map 4/22 sequences (cd:1946:0134) and again after every floor advance once the drive is idle (cd:1946:01be–01e0). See §4.

### 3.11 `PlayLoop` (floppy 1942:1315, cd:1aab:0001) — CONFIRMED

- **Start of play.** The CD no longer clears BEWITCHED (DS:0x3306) or the sparkle flag (DS:0x3308); those variables are gone. It also **no longer shows `UN BUILDING 2` on map 23**: the string is absent from `BCCD.EXE`. Sim (message).
- **Per frame.** New: if cd DS:0x9565 ≠ cd DS:0x956F, the CD calls `SetPaletteMode(DS:0x956F)` and copies the value (cd:1aab:031a–0332). The only writer of 0x956F found is `ResetInventory`, which sets it to 0 (cd:3060:0015), so in practice this has **no visible effect** (LIKELY).
- **Face apparition removed.** The floppy's random face apparition at 1942:18c1–18fb is gone. That code was: not in bogus mode or not yet shown, `US_RndT()==0`, then `US_RndT()%15==0`, single player, then `2390:10f2`. This removes **two `US_RndT` calls per frame** on the paths that reached them, which affects RNG order (Sim).

### 3.12 `SpawnFaceApparition` (floppy 2390:10f2, cd:2467:000e) — CONFIRMED, dead code in the CD

The CD keeps a twin of this function, but nothing calls it: there are no far calls to its real address 23b5:092e. It also differs:

- the `mapon ≥ 6` check is gone;
- it spawns class 18 (flame) with state cd `0x2614` (floppy `0x260E`), speed 0x400;
- the floppy spawns class 26 with state `0x28BE`, speed 0x300.

The debug-mode keys M/V (floppy 1942:0a86) no longer reach it either. No Sim effect.

### 3.13 `DoActor` / `KillActor` (floppy 1942:10f5 / 1e43:1377, cd:1a8e:0006 / cd:1fbf:0005) — CONFIRMED

The only change is that the BEWITCHED code is gone:

- `DoActor` no longer skips thinks while BEWITCHED;
- `KillActor` no longer switches BEWITCHED off.

The jump tables match one to one, with every target 12 bytes earlier. Since BEWITCHED cannot be set in the CD (§3.9), there is no behavioural difference.

### 3.14 `DamageActor` (floppy 1e43:174d, cd:1fdc:0097) — CONFIRMED, debug only

After subtracting the damage, the CD shows `Ouch %d Dam %u` (remaining health, damage) when `debugmode` (cd DS:0x92C) is set. No other change.

### 3.15 Removed state actions (Data, CONFIRMED)

- **Floppy `0x1D1E`** (`A_DebugSparkle`): the CD record (cd `0x1D24`) has no action.
- **Floppy `0x22FE`** (manhole fat man rising, `A_ManholeCover`): the CD record (cd `0x2304`) has no action. So **no open-manhole static (statinfo 49) is spawned** when a manhole enemy rises. LIKELY: no other spawner of that static was found, but the search was not exhaustive.
- **`statinfo`:** entry 48 (map object 71, sprite 49) changes kind from 0 to 17, i.e. it **blocks**, in the CD.

Both state changes and the `statinfo` change come from the tables.

### 3.16 `SetPaletteMode` (floppy 2f4d:035e, cd:2f9f:0008) — CONFIRMED

The palettes are the same per mode. The CD no longer writes DS:0x3268, the "generators off" flag that only `A_DebugSparkle` read. No effect.

### 3.17 Sequences: `PlaySequence` and its per-frame hook (floppy 32a2:0bcb / 32a2:09fa, cd:33bd:0002 / cd:33a7:0003) — CONFIRMED, UI

- **Pacing.** A new helper (cd:33bd:0164) returns 1 for `F2`, `F3`, `F4` and `F6`. For the other sequences (`F1`, `F5`, `F7`, the CD-only files) a flag is set, cd DS:0x342C. The FLC header's frame delay (header +16, cd DS:0xEB93) is then forced to 0, so these play unthrottled (cd:333a:001e). `slowrndseq` clears the flag and keeps the header speed. In the floppy, `slowrndseq` has no effect (GAMEFLOW §1.3).
- **CD music.** Before `F1`, `F5` or `F7`, a playing CD track is stopped (`CD_Stop`, cd:33bd:0086).
- **Sound cues** now depend on the file name:
  - F1: none (the floppy's F1 cues 88–91 at frames 0/128/185/258/319 are gone);
  - F2: frame 1 → 93;
  - F3: frame 1 → 94;
  - F4: none;
  - F5: frames 75 and 130 → 95 and 96;
  - F6: frame 1 → 92;
  - F7: none (a test for frame 200 does nothing).
- **Frame counts and header speeds** of the CD files:

  | File | Frames | Speed (ms) |
  |---|---|---|
  | F1 | 456 | 185 |
  | F5 | 177 | 85 |
  | F7 | 450 | 71 |

  F2/F3/F4/F6 are identical to the floppy's.
- **Callers.** `F1`: DemoLoop, unless `nopics` (cd:15fa:008a). `F2`/`F5`: GameLoop. `F3`/`F4`: exit kinds. `F6`: death. `F7`: victory (cd:196e:0032). The same places as the floppy.

### 3.18 Not audited in detail (presentation, menus, network)

| Area | Floppy | What differs |
|---|---|---|
| `AUDIO MIX` | 1618:0139 | More rows (the loop bound is 5 instead of 2); probably the CD-music items |
| Control panel | 1618:061a, 1618:09dd | — |
| `CP_SaveGame` | 1618:0f3d | Slot-text x position; the AutoSave slot-index bug was not re-checked |
| Renderer | 1b4a:1273/129c/1494 | The wall/floor pass call order moved into 1b4a:1273's twin; the floor pass's inner `floormode` test went, while the caller-level test stays |
| Scalers | 1d8b:0230, 1d8b:0a47 | — |
| Other | 273a:0ee1 `CA_CacheScreen`, 2a1d:0d2c, 2b79:153c (sound), 358f:18e7 (network), 1000:0e60 (C runtime) | — |

None of these is part of the simulation.

## 4. CD audio

The floppy build already parses `cdmusic`, but the CD build is the one that plays disc audio. CONFIRMED by disassembly.

- **Routines** (real far segment 0x2B64):
  - `CD_Busy` at 18bd;
  - `CD_NextTrack(advance)` at 18f7;
  - `CD_Play(n)` at 193f;
  - `CD_Stop` at 19a3.
- **Variables:**
  - cd DS:0x2CFA = `cdmusic` (initial 0, set by the parameter);
  - cd DS:0x2CF8 = playing;
  - cd DS:0x2CF4 = current track, **initially 1**.
- **Tracks.** Music tracks 1..4 are disc tracks 2..5; track 1 of the disc is data. `CD_Play(n)` takes a start and a length from a five-entry table at cd DS:0x2D1E, sends the MSCDEX play request and sets "playing".
- **`CD_NextTrack(advance)`** (cd 2b64:18f7): if the track is 0 or above 3, play track 1 and set the track to **2** (so the next restart plays track 2, not 1). Otherwise, if `advance`, add 1 to the track, then play it. Only the Alt keys call it: the track is **not** chosen per level and does not advance on its own.
- **`CD_Play(n)`** sets "playing" (0x2CF8 = 1); **`CD_Stop`** clears it (cd 2b64:19a3 writes 0 before the MSCDEX stop request). Nothing clears it when a track simply ends.
- **`StartMusic`** (cd:1a50:0041): with `cdmusic`, it plays the current track unless "playing" is set. Without `cdmusic`, it uses the IMF song table as on the floppy. `StartCPMusic` returns at once with `cdmusic`, so the menus play no music.
- **`GameLoop`, not `PlayLoop`, restarts tracks.** After the map 4 / map 22 sequences, the current track is played if "playing" is clear (cd:1946:0134). After every finished floor (right after `mapon++` and `health += 35`), it is played again if the drive is idle (`CD_Busy`, cd:1946:01be–01e0, the code at cd:195f:002e–cd:1961:0030). There is no per-frame check, so **a track that ends mid-level leaves the game silent until the next floor**, which then plays the same track again. (An earlier draft of this section placed the idle check in `PlayLoop`; the surrounding code is `GameLoop`'s level advance.)
- **Keys** (§3.9): Alt+P, Alt+S, Alt+F. Messages `CD : PLAY`, `CD : STOP`, `CD : ADVANCE TRACK`.
- **`PlaySequence`** stops CD audio before `F1`, `F5` and `F7` (§3.17), which clears "playing", so `StartMusic` at the next level start plays the current track again.

### 4.1 In OBCR (ADR-022)

`crates/obc-ui/src/cdaudio.rs` ports the above with MUSIC SOURCE = CD AUDIO (the default when ripped tracks exist; ADLIB gives the floppy behaviour). One Enhanced option, CD TRACK END = NEXT TRACK (default), plays the next track (4 wraps to 1) when one ends, instead of the original's silence until the next floor; ORIGINAL restores that. F2 (music toggle) stops and restarts the CD track, an OBCR addition.
