# Game flow, menus, sequences, saves and cheats

How `COUNT.EXE` (release `OBC-A`) gets from `main` to the menus, the game loop, the rendered sequences, the high-score table and the save files, and how the cheat and debug keys are activated. Audio is in [AUDIO.md](AUDIO.md).

Written 2026-11-02 from static analysis of the Ghidra export (`.local/ghidra/export`) and a linear disassembly of the image (LLVM `llvm-objdump --triple=i386-unknown-unknown-code16` over the MZ image wrapped as ELF). No DOSBox-X run was made.

Conventions:

- Addresses are `ghidra SSSS:OOOO` with Ghidra base segment 0x1000 (image segment = SSSS − 0x1000). DGROUP data is `DS:0xNNNN` (file offset = DS + 0x35580).
- Confidence tags follow AGENT_GUIDE §4. **CONFIRMED** here means the disassembly was read instruction by instruction (or a data file parse matches the code); **LIKELY** means the Ghidra decompiler output was read and is consistent, but not traced instruction by instruction; **HYPOTHESIS** is inference.
- Pic numbers: pic *i* is VGAGRAPH chunk 3 + *i* (DATA_FORMATS §4). `CA_CacheScreen(n)` takes the chunk, so `CA_CacheScreen(4)` is pic 1.
- Keyboard state is `Keyboard[]` at DS:0x8F22 (word array): `DS:0x8F22 + 2·scancode`. `LastScan` DS:0xBFEF, `LastASCII` DS:0x8F1F, `Paused` DS:0x8F20.

## 0. Function and variable index (proposed names)

These names are not yet in `re/symbols/count_exe.csv`; several existing `maybe_` names there are wrong (noted).

| Ghidra | Proposed name | Wolf3D counterpart | Confidence |
|---|---|---|---|
| 1497:17ed | `main` | `main` (WL_MAIN.C) | CONFIRMED |
| 1497:04f5 | `CheckFor386` (`no386` parameter) | `main` preamble / `JABHACK` | LIKELY |
| 1497:0dec | `InitGame` | `InitGame` | LIKELY |
| 1497:000b | `ReadConfig` | `ReadConfig` | LIKELY |
| 1497:036b | `WriteConfig` | `WriteConfig` | CONFIRMED (§10) |
| 1497:0549 | `NewGame(gametype)` | `NewGame` | CONFIRMED |
| 1497:061d | `SaveTheGame(handle, header)` | `SaveTheGame` | LIKELY |
| 1497:0867 | `LoadTheGame(handle, header)` | `LoadTheGame` | LIKELY |
| 1497:0d72 | `CheckParmExact(str)` (whole-word, case-insensitive) | `US_CheckParm` variant | LIKELY |
| 1497:119b | `NewViewSize` | `NewViewSize` | HYPOTHESIS |
| 1497:11f8 | `QuitNumber(n)` (`maybe_US_PrintUnsigned` in CSV is wrong) | `Quit` with a number | LIKELY |
| 1497:121d | `Quit(fmt, …)` | `Quit` | LIKELY |
| 1497:1442 | `DemoLoop` | `DemoLoop` | CONFIRMED |
| 1497:15ca | `TitleSplats` | — | LIKELY |
| 1618:061a | `US_ControlPanel(scancode)` | `US_ControlPanel` | LIKELY |
| 1618:1583 | `HandleMenu(iteminfo, items, routine)` | `HandleMenu` | LIKELY |
| 1618:0969 | `DrawMainMenu` | `DrawMainMenu` | LIKELY |
| 1618:1c13 | `Confirm(msg)` (Y/N) | `Confirm` | LIKELY |
| 1618:1ce9 | `Message(msg)` + wait | `Message` | LIKELY |
| 1618:1da3 | `MessageBox(msg)` (draw only) | `Message` | LIKELY |
| 1618:1efc | `StartCPMusic(song)` | `StartCPMusic` | LIKELY |
| 1618:2032 | `CP_CheckQuick` (F10 quit) | `CP_CheckQuick` | LIKELY |
| 1618:0a5a | `CP_GameType` | (`CP_NewGame` difficulty) | CONFIRMED |
| 1618:0b8b | `CP_ViewSize` | `CP_ChangeView` | CONFIRMED |
| 1618:0139 | `CP_AudioMix` | (`CP_Sound`) | CONFIRMED |
| 1618:0da3 | `CP_StartGame` | `CP_NewGame` | CONFIRMED |
| 1618:098e | `CP_AbortGame` | `CP_EndGame` | LIKELY |
| 1618:13cb | `CP_Quit` | `CP_Quit` | LIKELY |
| 1618:139d | `CP_ViewScores` | `CP_ViewScores` | LIKELY |
| 1618:10ec | `CP_LoadGame` | `CP_LoadGame` | LIKELY |
| 1618:0f3d | `CP_SaveGame(autosave)` | `CP_SaveGame` | CONFIRMED |
| 1618:10de | `AutoSave()` = `CP_SaveGame(9)` (`maybe_ShootSnd` in CSV is wrong) | — | CONFIRMED |
| 1618:1210 | `QuickSave()` (F9) | — | LIKELY |
| 1618:0e3f | `ReadSaveSlotName(slot)` | — | LIKELY |
| 1823:0ea3 | `GameLoop` | `GameLoop` | CONFIRMED |
| 1823:0919 | `SetupGameLevel` (+ level start) | `SetupGameLevel` | HYPOTHESIS |
| 1823:0cda | `Died` | `Died` | CONFIRMED |
| 1823:0c84 | `DrawPlayScreen` | `DrawPlayScreen` | HYPOTHESIS |
| 1942:1315 | `PlayLoop` | `PlayLoop` | CONFIRMED (anchor) |
| 1942:04ea | `CheckKeys` | `CheckKeys` | LIKELY |
| 1942:0d2d / 1942:0ce8 | `StartMusic` / `StopMusic` | same | CONFIRMED |
| 1942:0b4a | `Cheat_Warp` (Alt+W) | `DebugKeys` warp | LIKELY |
| 1942:0abb | `Cheat_Diagnostics` (Alt+LShift+D) | — | LIKELY |
| 1adb:0283 | `DebugKeys` (Backspace+key) | `DebugKeys` | LIKELY |
| 208c:104c | `Cmd_Use` | `Cmd_Use` | LIKELY |
| 2390:28d1 | `T_Victory` (sets `ex_victorious`) | `A_StartDeathCam` tail | CONFIRMED |
| 2390:2877 | `VictoryScreen` | — | CONFIRMED |
| 2f4d:035e | `SetPaletteMode(n)` (§3) | — | CONFIRMED |
| 2f4d:0405 | `SaveGamePalette` (`CA_OpenDebug_2f4d_0405` in CSV is wrong) | — | CONFIRMED |
| 2f4d:049f | `ClearFloorItems` (`CleanupControlPanel` in CSV is wrong) | — | LIKELY |
| 2f4d:12b5 | `GameMessage(fmt, …)` (status-line message, 125 tics) | — | LIKELY |
| 2f4d:13ed | `CheckHighScore` → rank or −1 | `CheckHighScore` part | LIKELY |
| 2f4d:1422 | `DrawHighScores(enter)` | `DrawHighScores` + `CheckHighScore` | LIKELY |
| 30d6:1bc0 | `TryExitFloor` | — | LIKELY |
| 32a2:0bcb | `PlaySequence(name)` | — | CONFIRMED |

| DS | Proposed name | Meaning | Confidence |
|---|---|---|---|
| 0x86FA | `playstate` | exit code (Wolf3D `exit_t`) | CONFIRMED |
| 0x86FC | `gamestate` (0x90 bytes); `+0` = game type | 0–2 Difficulty 1–3, 3 Random, 4 Network, 5 Network Teams, 6 Modem | CONFIRMED |
| 0x86FE | `gamestate.mapon` | map slot | CONFIRMED |
| 0x8704 | `gamestate.score` (i32) | | LIKELY |
| 0x870C | `gamestate.health` | | LIKELY |
| 0x8716 | `gamestate.weapon` | −1 on death | LIKELY |
| 0x8728 / 0x872E | `gamestate.killcount` / `killtotal` | | LIKELY |
| 0x873C | `gamestate.bogus` (cheats used) | §11 | CONFIRMED |
| 0x8A1C | `startgame` | | CONFIRMED |
| 0x8CBA | `loadedgame` | | CONFIRMED |
| 0x8CBC | `ingame` | | CONFIRMED |
| 0x8CB8 | `introshown` | DemoLoop has run its intro | CONFIRMED |
| 0x8F06 | `cp_exit` | control-panel exit code | LIKELY |
| 0x8F0C | `palettemode` | last `SetPaletteMode` argument | CONFIRMED |
| 0x0420 | `quicksaveslot` | −1 initially | CONFIRMED |
| 0x0922 | `exitkind` | 0 stairs, 1 elevator | CONFIRMED |
| 0x0954 | `debugmode` | §11 | CONFIRMED |
| 0x029A | `nopics` | | CONFIRMED |
| 0x2E14 | `cdmusic` | | CONFIRMED |

## 1. Startup

### 1.1 `main` (ghidra 1497:17ed) — CONFIRMED

```
SetFileExtensions();   // 1618:1ff5: ext = "BC"; "CONFIG." + "DAT"; "GFXTILES." + "BC"; "AUDIOMUS." + "BC"
CheckFor386();         // 1497:04f5
InitGame();            // 1497:0dec
DemoLoop();            // 1497:1442, never returns
CD_Shutdown();         // 32a2:01c6 (unreached)
Quit("Dmlp exit?");    // unreached
```

The configuration file is `CONFIG.DAT` (not `CONFIG.BC`); every other data file gets `.BC`.

### 1.2 `InitGame` (ghidra 1497:0dec) — LIKELY

Sets text mode, then prints one line per subsystem, each followed by a check mark (byte 0xFB) when it starts:

| Label (DS) | Call | Wolf3D |
|---|---|---|
| ` ANM proc ` (0x02C6) | 32a2:015e: rendered-sequence player; needs EMS (8 pages). The check mark is printed only if it succeeded (`DS:0x298` = 1) | — |
| ` MEM proc ` | `MM_Startup` | same |
| ` KBD proc ` | 288d:0614 | `IN_Startup` |
| ` PG1 proc ` | 2a1d:1506 | `PM_Startup` |
| ` PG2 proc ` | 2a1d:02c8(3) | page preload/lock |
| ` SND proc ` | 2b79:1227 | `SD_Startup` |
| ` CCH proc ` | `CA_Startup` | same |
| ` USR proc ` | 2d35:011d | `US_Startup` (only `harderr` and `US_InitRndT(1)`: the Wolf3D parameter loops are gone) |

Then: VGA mode and palette (1497:0d53), `SaveGamePalette` (2f4d:0405, copies `gamepal` 3A9B:0000 to DS:0xE295), `SetPaletteMode(3)`, `InitDigiMap`, map and block tables, `ReadConfig`, the `cdmusic` check, fonts (chunks 1 and 2), status-bar setup, and a **memory-based view-size cap** (a 32-bit `mminfo` field compared with 305,000, 325,000 and 345,000 bytes: below each, the maximum view size is 14, 16 or 18; otherwise 20). The same caps are applied in `CP_ViewSize`.

### 1.3 Command-line parameters

All parameters are matched case-insensitively after skipping leading non-letters (so `-nopics` and `/nopics` work). `CheckParmExact` (1497:0d72) compares the whole word; the Wolf3D-style list matcher `US_CheckParm` (2d35:0159) matches a prefix of the argument against each list entry.

| Parameter | Consumer | Effect | Confidence |
|---|---|---|---|
| `no386` | 1497:04f5 (list DS:0x00FC) | Without it, a 386 sets DS:0x8A2C = 1 and calls `jabhack2` | LIKELY |
| `nopics` | `DemoLoop` 1497:1442 | Skips the intro and title (straight to the control panel); sets DS:0x29A so `Quit` skips the exit pictures | CONFIRMED |
| `cdmusic` | `InitGame` (after `ReadConfig`, so it overrides the saved value) | DS:0x2E14 = 1: `SD_SetMusicMode` forces music off and `StartCPMusic` does nothing. No CD code is reachable (AUDIO.md §8), so the game is silent musically | CONFIRMED |
| `norndseq` | `PlaySequence` 32a2:0bcb | Disables all rendered sequences | CONFIRMED |
| `slowrndseq` | `PlaySequence` | Sets DS:0x347C = 0; nothing ever reads DS:0x347C (exhaustive byte search), so **no effect** in this build | CONFIRMED |
| `nojoys`, `nomouse` | `IN_Startup` (list DS:0x2CF8) | Wolf3D semantics | LIKELY |
| `nomain`, `noems`, `noxms` | `PM_Startup` (list DS:0x2D6A); `noems`/`noxms` also listed for `MM_Startup` (DS:0x2D0E) | Wolf3D semantics | LIKELY |
| `HIDDENCARD` | `VL_Startup`-like 2ddf:000b (list DS:0x2FA8 pushed at ghidra 2ddf:001d) | Wolf3D: skip VGA detection | LIKELY |
| `GOLEVEL`, `NOWAIT`, `COMP`, `NOCOMP` | none | The string table DS:0x2E50 exists but nothing references it; OBC's `US_Startup` dropped the loops. **No effect** | CONFIRMED |
| `vfr750hondarc45` | menus and `CheckKeys` | Enables the diagnostics cheat (§11) | CONFIRMED |
| `nsr250` | `CheckKeys` | Enables the "bogus mode" cheat (§11) | CONFIRMED |
| `president` | `CP_StartGame` | Checked, then nothing: the code decrements DS:0x3DA and immediately restores it either way. **No effect** | CONFIRMED |

`BC.BAT` passes up to four parameters to `COUNT`.

## 2. Intro, title and the "demo loop"

`DemoLoop` (ghidra 1497:1442), CONFIRMED by disassembly:

1. `StartCPMusic(8)` (music chunk 308) once on entry.
2. If the intro has already run (`introshown`), go straight to step 9.
3. If `nopics`: set DS:0x29A and go to step 9.
4. `introshown = 1`; `PlaySequence("F1")` (`F1.BC` is not shipped, so nothing plays); memory housekeeping.
5. `SetPaletteMode(3)`; fade out; `CA_CacheScreen(4)` = **pic 1** (story text over soldiers); fade in over **65 steps**; `IN_UserInput(275)` (≈3.9 s, a key skips).
6. Fade out; `CA_CacheScreen(5)` = **pic 2** (title logo); fade in in 1 step; `SD_PlaySound(86)` (3.9 s digitized burst); `IN_UserInput(100)`.
7. `TitleSplats` (1497:15ca): draws pic 20 (16×16 red splat) nine times at random positions (four screen quadrants, `x = rnd % 125` or `300 − rnd % 125`, `y = rnd % 60` or `180 − rnd % 60`), with random pauses of `rnd % 10`, `% 20` or `% 30` tics between some of them. Uses 18–22 `US_RndT` calls (they advance the shared RNG index).
8. `IN_UserInput(210)` (3 s); fade out.
9. Loop forever: fade out, `US_ControlPanel(0)`; when it returns with `startgame` or `loadedgame` set, run `GameLoop`, then fade out.

All fades are `VL_FadeOut(0, 255, 0, 0, 0, 80)` unless stated. Palette for pics: `SetPaletteMode(3)` (picture palette).

**There is no demo playback or attract loop.** After the first pass, the program sits in the control panel. The intro plays only once per run. CONFIRMED.

## 3. Palettes: `SetPaletteMode(n)` (ghidra 2f4d:035e) — CONFIRMED

Copies 768 bytes into the working palette `gamepal` at 3A9B:0000 (file 0x2DDB0), copies that to a second buffer at 40A5:0000, calls `VL_SetPalette` unless DS:0xBFF4 is set, calls 1942:0db5, and records `n` in DS:0x8F0C.

| n | Source | File offset | Used for | Side effect |
|---|---|---|---|---|
| 0, 1, >3 | DS:0xE295 (copy of the original `gamepal`, made at start-up by 2f4d:0405) | 0x2DDB0 | 3D view (normal) | DS:0x3268 = 0 (n = 0, 1) |
| 2 | 3ACB:0000 | 0x2E0B0 | 3D view with **generators off** (`Cmd_Use` on tile 0x5F) | DS:0x3268 = 1 |
| 3 | 3AFB:0000 | 0x2E3B0 | Full-screen pics, menus, high scores, sequences' surroundings | — |

`VL_FadeIn` callers pass 3A9B:0000 (the working copy), so they fade into whatever mode was set last. This resolves the "third palette" part of Q-025: it is the generators-off palette.

## 4. Control panel

### 4.1 Entry: `US_ControlPanel(scancode)` (ghidra 1618:061a) — LIKELY

- In game with F10 (`0x44`): `CP_CheckQuick` asks `BACK TO DOS ` (Y/N, far string 3A60:0000) over the game; Yes silences the AdLib (registers 1–0xF5 = 0) and calls `Quit(NULL)`; No returns without a menu.
- F1 (`';'`, 0x3B): toggle sound (FM/PC and digitized together, AUDIO.md §6), show `SOUND  ON` / `SOUND  OFF`, return. F2 (`'<'`): toggle music, show `MUSIC ON` / `MUSIC OFF`, return. These messages use the in-game message line (125 tics).
- Otherwise: `SD_StopSound`; set the default cursor (START GAME when not in game, CONTINUE in game); set item activity (§4.2); in game, fade out, remember the palette mode and switch to mode 3; `StartCPMusic(0)` (music chunk 300); `DrawMainMenu` = `CA_CacheScreen(7)` (**pic 4**) plus cursors; fade in (10 steps); then `HandleMenu` until an item sets `cp_exit`.
- After the loop: in game, restore the palette mode; for network game types, run the network setup (358f:0959) and repeat the menu if it fails.

Return codes of `HandleMenu` handled by the panel: −1 (Esc) → `CP_Quit` (**Esc in the main menu asks `BACK TO DOS `; it does not resume the game**); 5 (CONTINUE) → `cp_exit = 2`; 6 (ABORT GAME) → if confirmed, fade out, `ingame = 0`, redisplay the menu in its out-of-game state; 0x65 → help screen.

### 4.2 Main menu items (table at 3A61:0000, 46-byte records) — CONFIRMED

Record: `u16 active; u16 x; u16 y; u16 hotkey; char text[34] (empty: the text is painted in pic 4); far routine`.

| # | Painted label (pic 4) | Cursor x, y | Hotkey | Routine | Active |
|---|---|---|---|---|---|
| 0 | VIEW SIZE | 283, 11 | `V` | 1618:0b8b `CP_ViewSize` | always |
| 1 | GAME TYPE | 283, 25 | `P` | 1618:0a5a `CP_GameType` | not in game |
| 2 | HIGH SCORES | 283, 41 | `H` | 1618:139d `CP_ViewScores` | always |
| 3 | AUDIO MIX | 283, 56 | `M` | 1618:0139 `CP_AudioMix` | always |
| 4 | START GAME | 283, 85 | `N` | 1618:0da3 `CP_StartGame` | not in game |
| 5 | CONTINUE | 283, 112 | `C` | none (returns 5) | in game |
| 6 | ABORT GAME | 283, 127 | `A` | 1618:098e `CP_AbortGame` | in game |
| 7 | EXIT TO DOS | 283, 142 | `Q` | 1618:13cb `CP_Quit` | always |
| 8 | LOAD GAME | 283, 171 | `L` | 1618:10ec `CP_LoadGame` | not in game, game type < 4 |
| 9 | SAVE GAME | 283, 185 | `S` | 1618:0f3d `CP_SaveGame` | in game, game type < 4 |

Item info (DS:0x3D2): `amount = 10`, current position at DS:0x3D4. The last selected index is also stored in DS:0x444.

### 4.3 Cursor and keys in `HandleMenu` (ghidra 1618:1583) — LIKELY

- The cursor is an 8×8 pic drawn at the item's (x, y): pic 23 (chunk 26, DS:0x3F2, "on") and pic 22 (chunk 25, DS:0x3F0, dark blue "off").
- Blink: on for the first 500 tics; then off at tic 525, back on after tic 535, and the counter resets to 501. So after a 7 s pause the cursor blinks with a 35-tic period (≈11 tics off, ≈24 on).
- Up/Down: move to the previous/next **active** item, wrapping at both ends; `SD_PlaySound(85)` and `TicDelay(20)` per step.
- Typing a letter jumps to the next active item with that hotkey (searching forward, then from the top).
- Select: Space, Enter, or button 0 (Alt is cleared). Back: Esc or button 1 → returns −1.
- `H` (Keyboard[0x23]) in the main menu returns 0x65: **help screen** = `CA_CacheScreen(6)` (**pic 3**, default keys) shown with `FizzleFade` (5 frames per step), wait for a key, redraw the menu. CONFIRMED that pic 3 is shown this way; the guard is an `_fmemcmp` against the main-menu table (LIKELY: only in the main menu).
- With the `vfr750hondarc45` parameter, holding Tab+P calls 1adb:06f5, an empty function.

### 4.4 Sub-screens

**VIEW SIZE** (1618:0b8b) — CONFIRMED. Sizes 12, 14, 16, 18, 20 labelled `XS`, `S`, `M`, `L`, `XL` (odd values `??`), drawn at x 294/297, y 9. Up arrow (or direction North) increases; Down arrow (or direction **East**) decreases (a quirk: Right arrow shrinks the view). Above the memory cap (§1.2): message `MEM TOO LOW ` and the cap is used. Esc, Space or Enter leave; `NewViewSize` is called.

**GAME TYPE** (1618:0a5a) — CONFIRMED. Only when not in game. The current type is a letter drawn at (298, 23): `1`, `2`, `3`, `R`, `N`, `T`, `M` for types 0–6 (Difficulty 1–3, Random, Network, Network Teams, Modem). Up increases, Down (or East) decreases, clamped at 0 and 6 (no wrap). The value persists in `CONFIG.DAT` (§10). There is **no separate difficulty menu**: START GAME uses the current game type.

**HIGH SCORES** (1618:139d): `DrawHighScores(0)`, fade in (30 steps), wait for a key (§8).

**AUDIO MIX** (1618:0139): see AUDIO.md §7.

**START GAME** (1618:0da3) — CONFIRMED. In game it would ask `END GAME Y/N ` first, but the item is inactive in game. Calls `NewGame(gametype)`, 30d6:10c5, sets `cp_exit = 1`.

`NewGame` (1497:0549), CONFIRMED: clears `gamestate` (0x90 bytes), game type, DS:0x870A = 3 (two weapons owned, LIKELY), weapon fields DS:0x8714/0x8716/0x8718 = 1, health = `maxhealth[gametype]` (DS:0x32EC: 400 for types 0–3, 800 for 4–6), DS:0x870E = 30, DS:0x8708 = 3, score string, DS:0x8722 = 250, DS:0x872C = 100, DS:0x8740 = 200, DS:0x8742 = 100, `startgame = 1`. `mapon` = 0 (sewer level 1). No lives field is set (§5.4).

**ABORT GAME** (1618:098e): `ABORT THIS GAME Y/N `; Yes sets `playstate = ex_abort (7)`, deactivates SAVE and ABORT, moves the cursor to START GAME.

**EXIT TO DOS** (1618:13cb): `Confirm("BACK TO DOS ")`; Yes: network shutdown (game type ≥ 4), stop sounds and music, silence AdLib, `Quit(NULL)`.

`Quit(NULL)` (1497:121d), LIKELY: unless `nopics`, fade out, `CA_CacheScreen(8)` = **pic 5** (city with lightning), palette mode 3, fade in (50 steps), wait 70 tics, then `CA_CacheScreen(9)` = **pic 6** (credits) via `FizzleFade` (25 frames per step), wait 70 tics, fade out; `WriteConfig`; print `BC Done.`; exit code 0. `Quit(message)` (an error): if in game, **AutoSave first**; print `BC exited on its own.`, `Game Saved in AutoSave Slot.` (only if in game) and `Report written to "BC.LOG".`; append `\nBC ERROR: ` + message to `bc.log`; exit code 1. If `playstate == ex_victorious` at that moment, it exits with code 200 without writing anything.

**LOAD GAME / SAVE GAME**: §9.

## 5. Game loop

### 5.1 `GameLoop` (ghidra 1823:0ea3) — CONFIRMED

```
restart:
  housekeeping; startgame = 0
  if loadedgame: loadedgame = 0                 // state already loaded
  else if gametype < 4: SetupGameLevel()
  else if NetSetup() (358f:1251): SetupGameLevel()
  ingame = 1; StartMusic(); page housekeeping
  if !gamestate.bogus && !died && gametype < 4:
      MessageBox("One moment...") ; PM_Preload with progress (30d6:1cb4)
  died = 0
  VL_FadeOut(…, 30); DrawPlayScreen(); PlayLoop(); ingame = 0
  if startgame || loadedgame: goto restart
  switch playstate: (§5.3–5.6)
```

- "Get Ready!" (DS:0x3ADD) is used only by the network code (358f); single player shows **"One moment..."** while the level's pages are preloaded (Wolf3D's "Get Psyched!"). With bogus mode on, the box and preload are skipped (quirk).
- `PlayLoop` start (1942:1315): palette mode 1, the "enemies left" counter string (`killtotal − killcount`), per-map roof and floor settings (tables DS:0x9E6/0xA19/0xA4C/0xA7F indexed by `mapon`, network maps by `mapon − 10`), sewer maps (`mapon < 5`) set DS:0x9E2 = 0x8400 and DS:0x33B8 = 1, BEWITCHED and DS:0x3304 are cleared, and on **map 23** (BLDG2 FLR1) the message `UN BUILDING 2` is shown. Each frame, health is capped at 999.

### 5.2 Exits and floor change

Exits are wall tiles used with Space (`Cmd_Use`, ghidra 208c:104c), LIKELY:

| Tile (plane 0, upper bits clear) | Meaning | `exitkind` | Sequence |
|---|---|---|---|
| 0x71, 0x72 (113, 114) | Stairs | 0 | `F3` |
| 0x73 (115) | Elevator | 1 | `F4` |

In single player, `TryExitFloor` (30d6:1bc0, LIKELY) applies two rules before setting `playstate = ex_completed (1)`:

1. On map 39 (BLDG2 TOP) exits do nothing (the game ends by killing Victor).
2. If `killcount < killtotal / 2` the message **`UNFINISHED BUSINESS`** appears and you cannot leave: at least half the floor's enemies must be dead.

In network games the stairs tiles set `ex_completed` directly.

Other `Cmd_Use` tiles: 0x5F/0x60 generator switch (`GENERATORS OFF` + palette 2 / `GENERATORS ON` + palette 1, sound 84; the tile toggles between the two values); 0x58 medic station (heals 25 if health is below DS:0x8742, message `Medic Pack`); 0x62 push wall (2281:0ce2); 0x0D/0x10, 0x1D/0x20, 0x31/0x34, 0x41/0x44/0x45/0x48 play sounds 81, 80, 78, 79 and clear the info-plane value at that tile; tile values with bit 7 are doors (2281:07f3).

### 5.3 `ex_completed` (and `ex_secretlevel` 9) — CONFIRMED

1. `StopMusic`, housekeeping, `IN_ClearKeysDown`.
2. If game type < 4: **`AutoSave()`** (§9.3), *before* `mapon` advances.
3. `VL_FadeOut(…, 100)`.
4. Sequence: `mapon` 0–4 (sewers) → `F2`; `mapon` 22 (UN BLDG1 TOP) → `F5`; otherwise, in single player only, `exitkind` 0 → `F3`, 1 → `F4`.
5. `VL_FadeOut(…, 2)`; single player: `SetPaletteMode(3)` and `ClearFloorItems` (2f4d:049f: clears the low 7 bits of DS:0x8712, which are probably keys/cards, and 30 bytes at DS:0x875A); network: 358f:1720.
6. `mapon++`; **health += 35** with no cap here (only the per-frame cap of 999, so health can exceed the game type's maximum; the sample `SVGM0.SAV` has health 488 with maximum 400).
7. Back to `restart`.

The level order is therefore strictly linear: slots 0 → 1 → … → 39. There is no level table and no branching. CONFIRMED.

### 5.4 Death (`ex_died`, 2) — CONFIRMED

`Died` (1823:0cda): `gamestate.weapon = −1`; `SD_PlaySound(22 + rnd % 5)` (table DS:0x918 = 22–26); if a killer is recorded (DS:0x92B7) the view turns to face it (Wolf3D logic using floating-point `atan2`, Borland emulator interrupts `INT 34h–3Dh`); `SD_WaitSoundDone` (at most 49 tics); `FinishPaletteShifts`; palette mode 1. **There is no lives counter**: unlike Wolf3D, `Died` never restarts the level.

Then, single player: housekeeping, `IN_ClearKeysDown`, `VL_FadeOut(0,255, 10,0,0, 150)` (a slow fade to dark red), `VL_FadeOut(…, 50)` to black, `PlaySequence("F6")` (Victor laughing; sound 92), `VL_FadeOut(…, 0)`, palette mode 3; if the score qualifies, `DrawHighScores(1)` (name entry); return to `DemoLoop` (control panel). This is the README's "expire" sequence.

Network: 358f:0d7b, 358f:1796, `playstate = 0`, back to `restart` (respawn).

### 5.5 Victory (`ex_victorious`, 6) — CONFIRMED

The only writer of `playstate = 6` is `T_Victory` (2390:28d1), the think function of the last state in a death chain: states DS:0x1F8E → … → 0x1FFE, sprites 432–439, 15 tics each, the first state with action 2390:0c59 (death scream), the last (sprite 439, 0 tics, looping) setting `ex_victorious`. `KillActor` (1e43:1377 region) enters this chain with `GivePoints(1500)`. Sprite 432 shows a blond man in a long dark coat, matching Victor in `F6` (LIKELY Victor).

`GameLoop` then: clears DS:0xD40 and DS:0x9D8; `IN_UserInput(1000)` (≈14 s or a key); `StopMusic`; `VictoryScreen` (2390:2877: wait for digitized sound, `CA_CacheScreen(10)` = **pic 7** (logo over a pipe) with `FizzleFade` (35 frames per step), `IN_UserInput(350)`, fade out 35 steps); `PlaySequence("F7")` (not shipped); fade; palette mode 3; `DrawHighScores(1)`; `IN_ClearKeysDown`; `IN_UserInput(1000)`; `playstate = 0`; return to the control panel. No credits roll here (the credits pic 6 appears on exit to DOS).

### 5.6 Other exits

- `ex_warped` (3, from the Alt+W cheat): copies 13 words of player state (health, DS:0x870E, 0x8750, 0x8722, 0x871A, weapon, DS:0x870A, 0x8714, 0x8718, ammo DS:0x8784–0x878A) to DS:0x467E–0x4696 (presumably restored by level setup), and for `mapon ≥ 50` calls 358f:1720; then `restart`.
- `ex_abort` (7) and the others: `StopMusic`, `restart`. In practice abort always coincides with `startgame` or `loadedgame`, because the control panel does not return otherwise.

### 5.7 Episodes, credits maps

- **No episode concept** in the code: `mapon` runs 0–39; the high-score `episode` field is never written (stays 0). The only "chapter" break is the transition from map 22 (UN BLDG1 TOP) to map 23 (BLDG2 FLR1): it selects sequence `F5` (not shipped, so nothing plays) and map 23 shows `UN BUILDING 2` on arrival. CONFIRMED.
- **CRED maps 40–49 are not referenced**: no code sets or compares `mapon` with 40–49; the warp cheat accepts floors 1–40 and 51–60 only (maps 0–39, 50–59). LIKELY unused (Q-010).

## 6. Rendered sequences: `PlaySequence(name)` (ghidra 32a2:0bcb) — CONFIRMED

- Plays only if the sequence player initialised (`ANM proc`: EMS present and 8 pages allocated, DS:0x4594 ≥ 0, DS:0x298 = 1) and `norndseq` is absent.
- File name: the first two characters of `name` + `.bc` (DS:0x3487), opened read-only; the first 128 bytes are read into DS:0xEBD3 and the magic at offset 4 must be 0xAF12 (FLC). A missing file is silently skipped.
- Before playback the four VGA planes are saved to EMS and memory is tidied (when not in game). After playback: fade out (30 steps), restore the planes, `SetPaletteMode(1)`, redraw the play screen if in game.
- Frame pacing: wait until `16 × (TimeCount − frame_start) ≥ speed`, where `speed` is the FLC header's u32 at offset 16 (milliseconds). One 70 Hz tic counts as 16 "ms", so the delay is `ceil(speed / 16)` tics: **9 tics (128.6 ms) for F2 and F6 (speed 142), 5 tics (71.4 ms) for F3 and F4 (speed 71)**. The loop runs while `frame < frames − 1` and Esc (`Keyboard[1]`) is not held.
- Per-frame sounds (32a2:09fa, frame index from 0): see the table. The palette comes from the FLC colour chunks.

| File | Shipped | Frames, speed | Content (exported frames) | Played when | Sound cues |
|---|---|---|---|---|---|
| `F1` | no | — | — | `DemoLoop` intro, first pass | frames 0, 128, 185, 258, 319 → sounds 88, 89, 88, 90, 91 |
| `F2` | yes | 31, 142 ms | Climbing a ladder up a round shaft | Leaving maps 0–4 (sewers) | frame 1 → 93 |
| `F3` | yes | 30, 71 ms | Walking up a staircase | Stairs exit (tiles 0x71/0x72), maps 5–21, 23–38 | frame 1 → 94 |
| `F4` | yes | 54, 71 ms | Elevator car in a red-lit shaft | Elevator exit (tile 0x73), same maps | none |
| `F5` | no | — | — | Leaving map 22 | frames 75, 130 → 95, 96 |
| `F6` | yes | 34, 142 ms | Victor laughing | Player death ("expire") | frame 1 → 92 |
| `F7` | no | — | — | Victory | none |

So the README's "Minimal install" behaviour (no sequences) is exactly the missing-file path, and `F1`, `F5`, `F7` are missing from the "Regular" install too (OBC-A floppies).

## 7. Level start summary

| Event | What happens | Confidence |
|---|---|---|
| New game | `NewGame(gametype)`, map 0 | CONFIRMED |
| Every level start (single player) | `SetupGameLevel`, `StartMusic` (AUDIO.md §5), "One moment..." + preload (not in bogus mode), fade, `DrawPlayScreen`, palette 1 | CONFIRMED |
| Map 23 | Message `UN BUILDING 2` | CONFIRMED |
| Floor change | AutoSave, sequence, health +35, keys cleared (LIKELY), `mapon + 1` | CONFIRMED |

## 8. High scores

- `CheckHighScore` (2f4d:13ed): first of 7 entries whose score ≤ `gamestate.score`; −1 if none. LIKELY.
- `DrawHighScores(enter)` (2f4d:1422), LIKELY: if `enter == 1`, **not in bogus mode**, and the score qualifies, shift lower entries down and insert `{name "", score, completed = mapon + 1}` (the floor reached). Background `CA_CacheScreen(11)` = **pic 8** (blue city). Title `HIGH SCORES` centred at y 20; headers `NAME` (x 24), `L` (x 210, the floor column), `SCORE` (x 246) at y 43; rows at y 62 + 18·i: rank `%d.` at x 6, name at x 24, score right-aligned at x 300 (`%ld`), floor `%d` at x 210; each row gets a different colour (`'W' − 2i` and `'o' − 2i`). A new entry is boxed (bar colour 0x30, outline 0x6F) and the name is typed with `US_LineInput` (max **18 characters**, 262 px).
- Called with `enter = 1` after death and victory; with 0 from the menu.
- The `episode` field is never written. Answers Q-030: `completed` = floor number reached (1-based); `episode` unused.

## 9. Save and load

### 9.1 Slots and files — CONFIRMED

| Slot | File | Menu label | Writable from the menu |
|---|---|---|---|
| 0–8 | `svgm0.sav` … `svgm8.sav` | the saved name, or `available` (save menu) / `empty` (load menu) | yes |
| 9 | `autsv.sav` | `AutoSave` | **no** (item deactivated in the save menu) |

The load/save list (items at 3A7E:0000, 46-byte records, item info DS:0x3D6 with 10 entries; cursor at x 18, y 10 + 15·i) draws slot *i*'s label at x 35–36, y 8 + 15·i, with name entry at x 37. Save names: `US_LineInput`, max **11 characters**, 192 px. Slot names are read from the first 22 bytes of each file (`ReadSaveSlotName`, 1618:0e3f) for slots 0–8 only.

- LOAD is active only when not in game and game type < 4 (to load during play: ABORT GAME first). Loading sets `loadedgame` and leaves the menu.
- SAVE is active only in game and game type < 4. A successful menu save stores the slot in `quicksaveslot` (DS:0x420).

### 9.2 QuickSave (F9) — CONFIRMED

`CheckKeys` calls `QuickSave` when F9 is pressed and **game type ≤ 4** (so also in Network games, where the SAVE item itself is inactive). `QuickSave` (1618:1210): `quicksaveslot == −1` → `NO CURRENT SAVE`; outside 0–9 → `UNABLE TO SAVE`; `creat` fails → `CANT SAVE %s`; otherwise `SAVED %s` (shown before writing) and `SaveTheGame`. `quicksaveslot` is written only by the save menu and is **never reset** by new game, load or floor change, so F9 keeps overwriting the same file across floors (the README's "save on the current floor first" is advice, not a rule).

### 9.3 AutoSave — CONFIRMED

- Written by `AutoSave()` = `CP_SaveGame(9)`: at every floor change in single player (§5.3, before `mapon` increments) and by `Quit` on a fatal error while in game.
- The file therefore holds the **completed** floor's state: the player stands at the exit with the old `mapon`. Data check: `AUTSV.SAV` has `mapon` 7 and score 17,150, `SVGM2.SAV` has `mapon` 8 and the same score.
- **Bug (K candidate)**: the AutoSave branch never initialises its slot-index local (`[bp−4]`, ghidra 1618:0f3d…1083). It fills the slot-9 name record (DS:0x8EEE = 9, `AutoSave`) but writes the 22-byte header from `0x8E28 + 22·[bp−4]` (stack garbage), and if that garbage is < 9 it also overwrites `quicksaveslot`. The sample `AUTSV.SAV` header is indeed garbage (`00 00 00 00 26 00 1F 01 …`). Harmless for loading (the label is the constant `AutoSave`), but F9 can be redirected.

### 9.4 Save file format (`SaveTheGame` 1497:061d / `LoadTheGame` 1497:0867)

Free-disk check first (`No room on disk to save` box). Then, with no checksum (Wolf3D writes one):

| Offset | Size | Content | Confidence |
|---|---|---|---|
| 0x0000 | 22 | Header: `u8 slot; char name[21]` (slot 9's header is garbage, §9.3) | CONFIRMED |
| 0x0016 | 144 | `gamestate` (DS:0x86FC) | CONFIRMED |
| 0x00A6 | 4,096 | `tilemap` | CONFIRMED |
| 0x10A6 | 8,192 | `actorat` | CONFIRMED |
| 0x30A6 | 484 | `areaconnect[22][22]` (far, 3B43:0000) — **NUMAREAS = 22** (OpenDoor checks area codes 0x8C–0xA1) | LIKELY |
| 0x328A | 44 | `areabyplayer[22]` (DS:0x48EA) | LIKELY |
| 0x32B6 | 66·n | Actor list from `player`, 66 bytes each (`objtype`) | CONFIRMED |
| … | 66 | Terminator record (first word 0xFFFF) | CONFIRMED |
| … | 2 | Actor count (DS:0x467C); load checks it | CONFIRMED |
| … | 2 | DS:0x7E2A (`laststatobj`, LIKELY) | LIKELY |
| … | 2,250 | DS:0x7E2C (`statobjlist`) | LIKELY |
| … | 128 | `doorposition[64]` | CONFIRMED |
| … | 640 | DS:0x7BAA `doorobjlist[64]` (10 bytes each) | LIKELY |
| … | 384 | DS:0x46E2 | UNKNOWN |
| … | 128 | DS:0x4862 | UNKNOWN |
| … | 62 | DS:0x46A4 (message timer + message text) | LIKELY |
| … | 2 × 5 | DS:0x4916, 0x48E8, 0x48E6, 0x48E4, 0x4998 | UNKNOWN |
| … | 26 | DS:0x467E (warp-carried player state, §5.6) | LIKELY |

Fixed part = 16,680 bytes; file size = 16,680 + 66·n. Checked against the samples: `SVGM0.SAV` and `SVGM2.SAV` 19,188 = 16,680 + 38·66 (count field 38), `AUTSV.SAV` 18,594 = 16,680 + 29·66. CONFIRMED by size arithmetic.

`LoadTheGame` reads the header and `gamestate`, runs `SetupGameLevel` (so the map is reloaded from `MAPTEMP`), then overwrites tilemap, `actorat`, areas, actors, statics, doors and the rest in the same order.

## 10. CONFIG.DAT (`WriteConfig` 1497:036b) — CONFIRMED

287 bytes; decodes the shipped `CONFIG.DAT` exactly. Resolves most of Q-014.

| Offset | Size | Field (DS) | Sample |
|---|---|---|---|
| 0x000 | 196 | High scores, 7 × 28 (DS:0x2E64) | |
| 0x0C4 | 2 | `SoundMode` | 1 |
| 0x0C6 | 2 | `MusicMode` | 0 |
| 0x0C8 | 2 | `DigiMode` | 0 |
| 0x0CA | 2 | `mouseenabled` (DS:0x49A4) | 1 |
| 0x0CC | 2 | `joystickenabled` (DS:0x49A2) | 0 |
| 0x0CE | 2 | `joypadenabled` (DS:0x49A0) | 0 |
| 0x0D0 | 2 | `joystickprogressive` | 0 |
| 0x0D2 | 2 | `joystickport` | 0 |
| 0x0D4 | 8 | `dirscan[4]` (DS:0x956) | 48 4D 50 4B |
| 0x0DC | 28 | `buttonscan[14]` (DS:0x95E) | 1D 32 36 39 02 03 04 05 06 1C 1F 17 0F 38 |
| 0x0F8 | 8 | `buttonmouse[4]` (DS:0x97A) | 0, 2, 3, −1 |
| 0x100 | 8 | `buttonjoy[4]` (DS:0x982) | 0, 13, 3, 2 |
| 0x108 | 2 | `viewsize` (DS:0x499A) | 16 |
| 0x10A | 2 | `mouseadjustment` (DS:0x8A1A) | 5 |
| 0x10C | 2 | DS:0x48E2 | 0 |
| 0x10E | 2 | FM music volume (DS:0x469E) | 11 |
| 0x110 | 2 | Sound FX volume (DS:0x46A0) | 3 |
| 0x112 | 6 | Mixer values DS:0x469C, 0x469A, 0x4698 (not editable in the menu) | 5, 5, 8 |
| 0x118 | 2 | `cdmusic` (DS:0x2E14), overridden by the parameter | 0 |
| 0x11A | 1 | Roof/floor mode (DS:0x33D2: F5 sets 0, F6 sets 2) | 2 |
| 0x11B | 2 | Tremors (DS:0xD44, F8) | 1 |
| 0x11D | 2 | Game type (DS:0x86FC) | 0 |

## 11. Cheats and debug keys

Activation is two-staged: a **command-line password** plus an **in-game key chord**. Everything below is LIKELY (decompiler read; key scan codes derived from `Keyboard[]` offsets).

### 11.1 Bogus mode (`nsr250`)

Run `COUNT nsr250`; in a single-player game hold **Alt + Left Shift + B** (`CheckKeys` → 30d6:0350). Message `BOGUS MODE`; sets `gamestate.bogus` (DS:0x873C) = 1 and DS:0x3304 = 1. `gamestate.bogus` is saved with the game; it suppresses the "One moment..." preload and **disqualifies the game from the high-score table**.

With bogus mode on, Alt + key (without Left Shift) does:

| Keys | Handler | Effect |
|---|---|---|
| Alt+W | 1942:0b4a | Warp: prompt ` FLOOR: ` (2 digits); 1–40 or 51–60 → `mapon = n − 1`, `ex_warped` |
| Alt+R | 30d6:038c | `RESTOCK`: give weapons 1–4, ammo fields DS:0x8784–0x878A = 19 |
| Alt+I | 30d6:03e3 | Toggle `INVINCIBLE` / `VULNERABLE` (DS:0x9619) |
| Alt+H | 30d6:041c | `HEALTH HELP`: health + 10, capped at `maxhealth[gametype]` |
| Alt+B | 30d6:0456 | Toggle `BEWITCHED` / `BEWITCHED OFF` (DS:0x3306): `DoActor` skips the think of most actor classes (enemies freeze, LIKELY); killing any actor turns it off; reset at level start |
| Alt+T | 30d6:05c4(0) | Spawns an object at the player (class set from `rnd % 4`, state 0x295E/0x296E) and with probability 1/3 sets DS:0x3308; purpose UNKNOWN |

### 11.2 Diagnostics mode (`vfr750hondarc45`)

Run `COUNT vfr750hondarc45`; in game hold **Alt + Left Shift + D** (1942:0abb). Message ` DIAGNOSTICS`; sets `debugmode` (DS:0x954) = 1 and bogus mode, health = maximum, DS:0x870E = 200, **score = 0**, DS:0x8708 = 3. (In the menus, Tab+P with this parameter calls an empty stub.)

With `debugmode`, plain keys in `CheckKeys`:

| Key | Effect |
|---|---|
| D | Toggle DS:0x33B8 (single player), redraw (30d6:11b6) |
| Q / E | Cycle roof / floor texture index 0–12 (`Rindex %d`, `Findex %d`, table DS:0x9A4) |
| I / U | Roof colour + / − (16–224, `Rclr %d`) |
| K / J | Floor colour + / − (16–224, `Fclr %d`) |
| M, V | 2390:10f2 (map ≥ 6 only; spawns something, UNKNOWN) |
| X | `Quit("Testing Log")` (writes `BC.LOG`) |
| Backspace + key | `DebugKeys` (1adb:0283), below |

`DebugKeys` (Backspace held):

| Key | Effect |
|---|---|
| A | `Area DBG ON/OFF` (DS:0xC000 bit 1) |
| D | `Walk DBG ON/OFF` (DS:0xC000 bit 0) |
| C | Object counts (`Total statics`, `In use statics`, `Doors`, `Tot actrs`, `Actv actrs`) |
| F | Position info (world X,Y; tile X,Y; facing tile, its tile and info value; angle; direction; area number; view height) |
| G | God mode toggle (`INVINCIBLE` / `VULNERABLE`) |
| I | Items: +100,000 points, heal 100, next weapon, DS:0x870E + 100 (cap DS:0x8740), DS:0x871A + 100 (cap DS:0x872C) |
| M | Memory report (`NEAR heap`, `FAR heap`, EMS, XMS, main, code size) |
| N | `WALKTHRU ON/OFF` (`noclip`) |
| P | Empty stub |
| S | 2f4d:1375 (stub) then redraw the play screen |

### 11.3 Ordinary in-game keys handled by `CheckKeys` (for reference)

Esc → control panel (the level music restarts from the beginning afterwards); F1/F2 sound/music toggles; F5 roof/floor on (DS:0x33D2 = 0); F6 off (= 2); F8 tremors toggle (`TREMORS ON/OFF`); F9 QuickSave; F10 quit prompt; H exact-health display toggle (DS:0xD46 bit 0); C crosshair toggle (DS:0xD46 bit 6); F change ally focus; A switch to ally (`SWITCH` or `NO ALLIES!`); S switch and stay (`STAYPUT & SWITCH`); W wipe visor; Pause shows `PAUSE` and stops the music until a key. Matches pic 3.

`president` is not a cheat (§1.3).

## 12. Quirk candidates (for DELTA_SPEC `K-###`)

| Quirk | Evidence | Class (suggested) |
|---|---|---|
| AutoSave writes an uninitialised header and may change the QuickSave slot | 1618:0f3d; sample `AUTSV.SAV` | gameplay (header) / cosmetic |
| Esc in the main menu asks `BACK TO DOS ` instead of resuming | 1618:061a | gameplay (UI) |
| Right arrow shrinks the view size and lowers the game type | 1618:0b8b, 0a5a | cosmetic |
| `slowrndseq`, `GOLEVEL`, `NOWAIT`, `COMP`, `NOCOMP`, `president` have no effect | §1.3 | — |
| Health +35 per floor without cap (cap 999 per frame) | 1823:1045 | gameplay |
| F9 QuickSave also works in Network game type 4 | ghidra 1942:07f2 | gameplay |
| `Quit` exits with code 200 if `playstate` is victorious | 1497:121d | — |
| `F4` sound cues (frames 75, 130) can never fire: `F4.BC` has 54 frames; they belong to `F5` | 32a2:09fa | — |

## 13. Open gaps

- `SetupGameLevel` (1823:0919) and how the Random game type differs (Q-012) were not read.
- Whether the warp-carried state (DS:0x467E) is restored, and by whom.
- Network flow (358f, 377a) beyond the hooks named here (Q-019).
- Several `CheckKeys` and debug handlers (Alt+T, M/V, D) need a closer read.
- Everything here is static analysis; the timings (blink, fades, sequence pacing) should be confirmed with a DOSBox-X capture.
