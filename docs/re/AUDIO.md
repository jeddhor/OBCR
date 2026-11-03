# Audio

How `COUNT.EXE` (release `OBC-A`) plays sound effects and music: which data source each sound id comes from, the digitized sample rate, priorities, music selection, the Sound Blaster Pro mixer and the AUDIO MIX menu, and the (unused) CD-audio code. Game flow and menus are in [GAMEFLOW.md](GAMEFLOW.md).

Written 2026-11-02 from static analysis (Ghidra export plus a linear LLVM disassembly of the image); nothing was played back in DOSBox-X. Address conventions and confidence tags as in GAMEFLOW.md: `ghidra SSSS:OOOO` with base segment 0x1000, `DS:0xNNNN` in DGROUP; **CONFIRMED** = disassembly read instruction by instruction, or a data parse that matches the code; **LIKELY** = decompiler output read; **HYPOTHESIS** = inference.

## 1. Summary

| Question | Answer | Confidence |
|---|---|---|
| `NUMSOUNDS` | **100**: PC speaker chunks 0–99, AdLib 100–199, digitized placeholders 200–299 (all empty except 299 = 4 bytes; never read), music 300–309 | CONFIRMED (Q-007) |
| Digitized source | **`AUDIOMUS.BC`**, merged into the page manager at `PMSoundStart`; the 46 sounds inside `GFXTILES.BC` are shadowed and never played | CONFIRMED |
| Sound id → digitized index | Identity (`DigiMap[i] = i` for 0–99) | CONFIRMED |
| Digitized sample rate | **9,009 Hz** (DSP time constant 0x91 = 145; 1,000,000 / 111) | CONFIRMED (Q-005) |
| Digitized hardware | Sound Blaster only (`DigiMode` 3); 8-bit mono, 4 KB DMA segments, one voice | CONFIRMED |
| Music format and rate | IMF on AdLib; timer at **700 Hz** while AdLib music is enabled, 140 Hz otherwise | CONFIRMED (Q-006 rate) |
| Music per level | `songs[mapon]` for maps 0–9; a random song from the first nine for maps ≥ 10 | CONFIRMED |
| Menu and intro music | Song 0 whenever the main menu is drawn; song 8 from program start (intro, title) | CONFIRMED |
| CD audio | Code present (MSCDEX, TOC dump, play-track-range) but **never called**; `cdmusic` only turns FM music off | CONFIRMED (Q-016) |
| FM MUSIC VOLUME | SB Pro mixer FM register 0x26 (both channels), value 5–14 | CONFIRMED |
| SOUND FX VOLUME | Attenuation 1–9 combined with stereo position into SB Pro voice register 0x04 | CONFIRMED (mechanism), LIKELY (direction) |
| BCKGRND MUSIC, SOUND EFFECTS, MASTER VOLUME | Labels and dead code only; the menu shows two rows | CONFIRMED |

## 2. Chunk layout of `AUDIOHED.BC` / `AUDIOT.BC`

`SD_SetSoundMode` (ghidra 2b79:114c) points the sound table at `audiosegs[0]` for PC speaker (`SoundMode` 1) and `audiosegs[100]` for AdLib (`SoundMode` 2). `StartMusic` caches chunk `song + 300` (`add ax, 0x12c`, ghidra 1942:0d68) and `StopMusic` (1942:0ce8) walks 10 music slots. Hence:

| Chunks | Content | Notes |
|---|---|---|
| 0–99 | PC speaker sounds (`u32 length; u16 priority; data`) | 8–271 bytes |
| 100–199 | AdLib sounds (`u32 length; u16 priority; Instrument; u8 block; data`) | 34–278 bytes |
| 200–299 | Digitized placeholders | All zero length except 299 (4 bytes). Never cached |
| 300–309 | IMF songs 0–9 | 8,826 · 12,854 · 17,262 · 25,314 · 27,782 · 16,290 · 970 · 23,994 · 21,734 · 23,546 bytes |

DATA_FORMATS §5 says chunk 300 is the 4-byte chunk and 301–309 are music; that is off by one (chunk *i* spans `offset[i]`…`offset[i+1]`): the 4-byte chunk is 299 and the ten songs are 300–309.

Priorities (the `u16` after the length; PC and AdLib copies agree): 99 for 0–29, 48, 74–77, 84, 86, 88–96; 98 for 30–47 and 49–73; 80 for 78–83; 90 for 85; **0 for 87, 97, 98, 99** (dummies).

## 3. Digitized sounds

### 3.1 Source: `AUDIOMUS.BC` through the page manager — CONFIRMED

`PML_OpenPageFile` (ghidra 2a1d:0597) opens `GFXTILES.BC` (handle DS:0x2D60) and `AUDIOMUS.BC` (DS:0x2D62). It reads the `GFXTILES` header and page table, then seeks `AUDIOMUS` to the hard-coded directory offset **0x14F000** (DS:0x2D66/0x2D68; error 418 if the seek fails) and reads **335** offsets and lengths (count hard-coded at DS:0x2D64). These **overwrite the page-table entries from `PMSoundStart` (816) onward**, and `ChunksInFile` becomes `PMSoundStart + 335` = 1,151. Sound pages are fetched with a dedicated page getter, 2a1d:1076 (used by `SDL_LoadDigiSegment` and `SDL_SetupDigi`), which loads through 2a1d:0b1e → 2a1d:051c, a reader that seeks and reads the `AUDIOMUS` handle; the ordinary getter loads through 2a1d:0ace → 2a1d:048e (`GFXTILES`).

Consequences:

- The 120 sound pages and the 46-entry sound list inside `GFXTILES.BC` (pages 816–936) are never read: leftovers from an earlier build.
- `SDL_SetupDigi` (2b79:0ce9) reads the last page (`ChunksInFile − 1` = `AUDIOMUS` page 334, the sound map) and copies **800 bytes = 100 entries** of `(u32 start_page, u32 length)` into `DigiList` (far, DS:0xDAAD). `NumDigi` (DS:0xDAB7) = 100. The map page holds only 792 bytes (99 entries), so **entry 99 is whatever follows in the page buffer** (undefined).
- `InitDigiMap` (1497:0dcb) fills `DigiMap` (DS:0x8CBE) from the pair list at DS:0x0106 (terminated by sound 100): the identity, sound *i* → digitized *i*.

### 3.2 Playback — CONFIRMED

- `SDL_StartSB` (2b79:0649): DSP command 0xD1 (speaker on), then **0x40 with time constant 0x91**. Rate = 1,000,000 / (256 − 145) = **9,009 Hz**, as in Corridor 7 (EC7Wolf `C7_SAMPLE_RATE`).
- `SD_PlayDigitized(which, left, right)` (2b79:0b31): `which ≥ NumDigi` shows `Bad Sound Num` on the message line and returns. Otherwise it stops the current digitized sound, sets the stereo position, takes the page range from `DigiList[which]`, and streams the sound in **4,096-byte segments** (one page each, locked while used) by DMA (2b79:0309 ends with DSP command 0xD4). `SDL_DigitizedDone` (2b79:0c40) and the service routine 2b79:0a3a queue the next page.
- Samples are 8-bit unsigned mono. No software mixing: one digitized sound at a time.
- `SD_SetDigiDevice` (2b79:0cb0) accepts 3 (Sound Blaster) only when a Blaster was detected (DS:0x8D8C). The Wolf3D Sound Source and PC-speaker digitized modes are not offered by the menus or defaults.
- Stereo: `SD_SetPosition(left, right)` (2b79:0af2 → 2b79:03ba; arguments 0–15, not both 15) acts only with mixer type 1 (SB Pro, §7): voice register 0x04 = `(15 − a − left) << 4 | (15 − a − right)`, where `a` is the SFX attenuation (DS:0x46A0). If `a + left ≥ 14` the left nibble becomes 1 and the right nibble keeps the *raw* position value (and vice versa): a quirk. Without that mixer, digitized sounds are not panned and the SFX setting does nothing for them.

## 4. Starting a sound

### 4.1 `SD_PlaySound(id)` (ghidra 2b79:15b3) — CONFIRMED

```
if SoundMode == 0 && DigiMode == 0: return 0
lp, rp, positioned = LeftPosition, RightPosition, nextsoundpos; clear the three
if id == -1: return 0
s = SoundTable[id]                     // PC or AdLib chunk, chosen by SoundMode
if s == NULL: Quit(2010)
if DigiMode != 0 and (s.priority >= DigiPriority or DS:0x43BC == 0 or !DigiPlaying):
    SD_PlayDigitized(DigiMap[id], lp, rp)
    SoundPositioned = positioned; DigiNumber = id
    SoundPriority = DigiPriority = s.priority
    return 1
if s.length == 0: Quit(2011)
if s.priority < SoundPriority: return 0
SoundMode 1: SDL_PCPlaySound(s); SoundMode 2: SDL_ALPlaySound(s)
SoundNumber = id; SoundPriority = s.priority; return 0
```

Differences from Wolf3D (`WOLFSRC/ID_SD.C` `SD_PlaySound`):

- **Every** id goes to the digitized path when a Blaster is enabled: there is no `DigiMap[id] != -1` test (every id has a digitized version).
- The digitized test compares the PC/AdLib chunk's priority with `DigiPriority` using `>=` (equal priority interrupts).
- If the digitized test fails (a higher-priority digitized sound is still playing), the sound **falls back to its FM or PC version** (subject to the usual `SoundPriority` test). Wolf3D returns instead.
- DS:0x43BC is set to 1 when a DMA transfer starts (2b79:0309) and cleared on stop; while it is 0 any sound may start.
- With `SoundMode` 0 and `DigiMode` 3 digitized sounds still play (`SoundTable` only supplies the priority).

### 4.2 Positioned sounds: `PlaySoundLocGlobal(id, x, y)` (ghidra 1823:0106) — LIKELY

`SetSoundLoc` (1823:0006) computes `leftchannel` and `rightchannel` from the Wolf3D 15 × 30 tables (lookup bases DS:0x765 and DS:0x5A3). The sound starts only if

`left + right < 13` **or** `id < 50` **or** (mixer type 1 **and** (`id > 62` **or** `id < 75`))

The last clause is always true with an SB Pro mixer, so **with that mixer nothing is culled**; otherwise sounds 50–99 are skipped when both channels are attenuated (far away). The condition looks like a typo for `&&`; preserve it. Then `SD_PositionSound(left, right)` and `SD_PlaySound`; if a digitized sound started, `globalsoundx/y` are stored for `UpdateSoundLoc` (1823:017f, called by `PlayLoop`), as in Wolf3D.

### 4.3 Waiting

`SD_WaitSoundDone` (2b79:173d) waits while a digitized sound plays, **at most 49 tics**, running the 3D refresh each pass. Used by `Died` and `VictoryScreen`.

## 5. Music

### 5.1 Selection — CONFIRMED

`StartMusic` (ghidra 1942:0d2d), called at every level start and after the in-game control panel:

```
SD_MusicOff()
song = mapon < 10 ? songs[mapon] : songs[US_RndT() % 9]     // songs[] at DS:0x98A
cache chunk 300 + song (purge off, lock)
if DS:0x9297 != 0: DS:0x9297 = 0; return                    // one-shot "do not start" flag
SD_StartMusic(audiosegs[300 + song])
```

`songs[]` = `{3, 5, 1, 2, 4, 7, 9, 8, 6, 0}`:

| Map slot | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | ≥ 10 (incl. network 50–59) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Song (chunk − 300) | 3 | 5 | 1 | 2 | 4 | 7 | 9 | 8 | 6 | 0 | random of 3, 5, 1, 2, 4, 7, 9, 8, 6 |

`StartMusic` therefore **consumes one `US_RndT` per call on maps ≥ 10** (it shares `rndindex` with the simulation). Esc out of the in-game menu calls it again: the song restarts, and on maps ≥ 10 a new random song may be chosen.

`StartCPMusic(song)` (1618:1efc), skipped entirely with `cdmusic`: frees the previous menu song, `SD_MusicOff`, caches chunk `300 + song`, `SD_StartMusic`. Called with **song 8** at the start of `DemoLoop` (so it plays under the intro, title and the first menu) and with **song 0** each time the main menu is drawn.

`StopMusic` (1942:0ce8): `SD_MusicOff`, then make all ten song chunks purgeable and unlocked. Called before floor changes, death, victory and the in-game control panel. No music plays during rendered sequences, the high-score screen after death or victory, or the victory screen.

### 5.2 Playback — CONFIRMED (timer), LIKELY (sequencer)

- `SDL_SetTimerSpeed` (2b79:0069): `MusicMode == 1` → 700 Hz with ISR 2d04:0010; otherwise 140 Hz with ISR 2d04:01ee. PIT divisor = 1,192,030 / rate (2b79:0037 → 2b79:000f). IMF delays are in 700 Hz ticks, as in Wolf3D.
- `SD_StartMusic(seg)` (2b79:17d6) acts only when `MusicMode == 1`: stops the old song, points the `sqHack` sequencer at the chunk (`u16 length`, then 4-byte IMF records), resets `alTimeCount`, applies the FM volume (DS:0x469E) and turns the sequencer on.
- `SD_MusicOff` (2b79:1781): with the SB Pro mixer (DS:0xDADD = 1) it first **fades the FM mixer volume** from its setting down to 2, one step per `IN_UserInput(5)` (5 tics, cut short by a key), then silences the OPL (register 0xBD and 0xB0–0xB8 = 0) and stops the sequencer. `SD_MusicOn` (2b79:1762) restores the FM volume and resumes. 2b79:1867 is a matching fade-in with no caller. `SD_MusicPlaying` always returns 0.
- Pause shows `PAUSE`, calls `SD_MusicOff`, waits for a key, then `SD_MusicOn` (the song continues where it stopped).
- Song 6 is 970 bytes (a short loop or jingle, HYPOTHESIS); it is used only for map 8.

### 5.3 Modes and toggles — LIKELY

| Variable (DS) | Values | Set by |
|---|---|---|
| `SoundMode` (0x8D8A) | 0 off, 1 PC speaker, 2 AdLib (2 falls back to 1 without an AdLib, DS:0x8D8E) | `ReadConfig`, F1 |
| `DigiMode` (0x8D88) | 0 off, 3 Sound Blaster | `ReadConfig`, F1 |
| `MusicMode` (0x8D86) | 0 off, 1 AdLib (needs an AdLib; forced to 0 by `cdmusic`) | `ReadConfig`, F2 |

- **F1** (1618:12d5): if `SoundMode` is 0, `SD_SetSoundMode(DS:0x8E26 or 2)` and, with a Blaster, `SD_SetDigiDevice(3)`; otherwise both off. One key toggles FM/PC and digitized effects together. Message `SOUND  ON` / `SOUND  OFF`.
- **F2** (1618:132e): `SD_SetMusicMode(!MusicMode)`. Message `MUSIC ON` / `MUSIC OFF`.
- Defaults without `CONFIG.DAT` (1497:025a): AdLib effects and AdLib music if a Blaster or an AdLib is present, digitized effects if a Blaster is present, FM volume 11, SFX attenuation 4 (and game type 1, view size 18).

## 6. Hardware notes

`SD_Startup` (2b79:1227) reads the `BLASTER` variable (DS:0x2E44). DSP command port `base + 0x20C`, mixer `base + 0x204/0x205` with `base` in DS:0x2DD6; IRQ number DS:0x2DD8 (vector table DS:0x2DDC); DMA channel tables DS:0x2DC9/0x2DCD/0x2DD1 (2b79:0516, channels 0–3).

## 7. Mixer and the AUDIO MIX menu

### 7.1 Detection (inside `SDL_StartSB`, 2b79:0649) — CONFIRMED

1. Save mixer register 0x26, write 0xBB, read back. Different → no mixer (DS:0x46A2 = 0).
2. Save register 0x0F, write 0xDD, read back. Equal → **type 2** (unknown chip: it saves registers 0x05, 0x09, 0x03, 0x07, 0x0B), DS:0xDADD = 0.
3. Otherwise **type 1 = Sound Blaster Pro (CT1345)**: saves registers 0x04, 0x2E, 0x28, 0x22, writes register 0x0E = 0 (mono, filter on), DS:0xDADD = 1, DS:0x46A2 = 1.

### 7.2 `SD_SetMixerVolume(value, channel)` (2b79:0559) — CONFIRMED

| Channel | Meaning | Type 1 register (DS:0x2E16) | Type 2 registers (DS:0x2E1B) | Variable (DS), range | In menu |
|---|---|---|---|---|---|
| 0 | FM (OPL) | 0x26 | 0x05/0x06 | FM volume 0x469E, 5–14 | yes |
| 1 | Voice (digitized) | 0x04 | 0x09/0x0A | SFX attenuation 0x46A0, 1–9 (callers pass `10 − a`) | yes |
| 2 | CD | 0x28 | 0x07/0x08 | 0x469C | no |
| 3 | Line | 0x2E | 0x03/0x04 | 0x469A | no |
| 4 | Master | 0x22 | 0x0B/0x0C (table DS:0x2E2A) | 0x4698 | no |

Type 1 writes `v << 4 | v` (left = right), adding 5 first for the voice channel. Type 2 maps the value through DS:0x2E20 (`v − 5` for FM) and sets bits 0x20 and 0x40 on a register pair. `ReadConfig` applies channels 0, 2, 3 and 4 at start-up; the CD, line and master values are stored in `CONFIG.DAT` (GAMEFLOW §10) but cannot be changed in the game.

FM music and AdLib sound effects share the OPL output, so **FM MUSIC VOLUME also scales the AdLib effects**.

### 7.3 `CP_AudioMix` (ghidra 1618:0139) — CONFIRMED

- No mixer: message box `MIXER NOT SUPPORTED ` and return.
- Screen: drawn over the main menu (pic 4); the cursor pics (chunks 25, 26) are cached; fade in (10 steps). Row labels at x 24, y 8 and y 40 (`FM MUSIC VOLUME`, `SOUND FX VOLUME`; the selected row is drawn highlighted, colour 0xA2). Sliders at x 24, y 20 and y 52: 19 segments of 8 px drawn with the two cursor pics, filled to `FM − 5` and `10 − attenuation` (both 0–9).
- Keys: Up/Down select row 0 or 1 (**the row index is clamped to 0–1**). Right: FM + 1 (max 14) or attenuation − 1 (min 1, louder). Left: FM − 1 (min 5) or attenuation + 1 (max 9). Each change redraws, waits 10 tics and applies: FM through channel 0; SFX with type 1 by playing digitized #49 at position `(15 − a, 15 − a)` (so register 0x04 gets the new value), with type 2 by `SD_SetMixerVolume(10 − a, 1)` and `SD_PlaySound(45)`.
- Enter or button 0 keeps the values. Esc or button 1 restores all five saved values and reapplies them.
- With the `vfr750hondarc45` parameter, Tab+P calls an empty stub.

A five-entry label table (DS:0x400: `FM MUSIC VOLUME`, `SOUND FX VOLUME`, ` BCKGRND MUSIC `, ` SOUND EFFECTS `, ` MASTER VOLUME `), the y list (DS:0x3F6: 8, 40, 72, 104, 136), three more slider routines (1618:0038, 004f, 0066 at y 84, 116, 148) and Space handlers for rows 2 and 3 (toggle music, toggle sound effects) exist, but rows 2–4 can never be selected and are never drawn. So **BCKGRND MUSIC** was meant to be the music on/off toggle (same as F2), **SOUND EFFECTS** the effects toggle (same as F1) and **MASTER VOLUME** a slider for channel 4. All three are dead in `OBC-A`.

## 8. CD audio — CONFIRMED

Segment 32a2 contains an MSCDEX interface: `MSCDEX not installed...???`; a TOC reader that prints `DiskInfo.lo_tno`, `hi_tno`, lead-out and each track's start address and control bits (`4 chnl`, `2 chnl`, pre-emphasis, data, copy) (32a2:0f30 with 0cf9, 0d71, 1060, 10dd); and a "play from track `lo + a` to track `lo + b`" routine (32a2:0ec2 → 0e25). **None of them has a caller**: the only far calls into segment 32a2 are to 015e (sequence-player start-up), 01c6 (shutdown) and 0bcb (`PlaySequence`), and no far pointer to the CD routines exists in the file.

The `cdmusic` parameter (also stored in `CONFIG.DAT`, but the parameter overrides it) sets DS:0x2E14, which makes `SD_SetMusicMode` force music off and `StartCPMusic` return early. **With `cdmusic` the game plays no music at all.** There is no track table to recover from this build; Q-016 can be closed as "unused in OBC-A" (a CD release could differ, Q-029).

## 9. Sounds in rendered sequences

`PlaySequence` calls a per-frame hook (32a2:09fa) that starts sounds 88–96 at fixed frames (GAMEFLOW §6) and services the digitized stream between frames (2b79:0a3a). Sounds 91–96 (5–14 s) are soundtracks for the animations: 93 for the ladder (`F2`), 94 for the stairs (`F3`), 92 for Victor's laugh (`F6`). The elevator (`F4`) has no sound; the cue frames 75 and 130 belong to the missing `F5`.

## 10. Sound ids

Digitized lengths come from the `AUDIOMUS` sound map, in seconds at 9,009 Hz. "PC len, prio" and "AdLib len, prio" come from `AUDIOT`. Contexts come from constant call sites and sound-id tables in the code; names in bold are certain from context, everything else is HYPOTHESIS. Sounds 87–90, 97 and 98 are byte-identical copies of #45 (placeholders). The `PlayLoop` terrain sounds test the `tilemap` byte at DS:0x9417/0x9419 (ghidra 1942:1791 onward).

| Id | Digitized bytes / s | PC len, prio | AdLib len, prio | Where it is played |
|---|---|---|---|---|
| 0 | 14112 / 1.57 | 109, 99 | 114, 99 | death scream (pair DS:0x29B2, used by 2390:0c59) |
| 1 | 15360 / 1.70 | 67, 99 | 90, 99 | death scream (pair DS:0x29B2, used by 2390:0c59) |
| 2 | 5024 / 0.56 | 60, 99 | 65, 99 | death scream (pair DS:0x29B6) |
| 3 | 3808 / 0.42 | 43, 99 | 35, 99 | death scream (pair DS:0x29B6) |
| 4 | 5600 / 0.62 | 80, 99 | 36, 99 | death scream (pair DS:0x29BA) |
| 5 | 5888 / 0.65 | 59, 99 | 35, 99 | death scream (pair DS:0x29BA) |
| 6 | 10528 / 1.17 | 199, 99 | 132, 99 | death scream (constant in 2390:0c59) |
| 7 | 10560 / 1.17 | 90, 99 | 85, 99 | death scream (constant in 2390:0c59) |
| 8 | 4952 / 0.55 | 57, 99 | 72, 99 | death scream (pair DS:0x29BE) |
| 9 | 5138 / 0.57 | 60, 99 | 66, 99 | death scream (pair DS:0x29BE) |
| 10 | 6080 / 0.67 | 65, 99 | 78, 99 | death scream (set DS:0x2998) |
| 11 | 6352 / 0.71 | 77, 99 | 76, 99 | death scream (set DS:0x2998) |
| 12 | 7968 / 0.88 | 70, 99 | 84, 99 | death scream (set DS:0x2998) |
| 13 | 5568 / 0.62 | 108, 99 | 63, 99 | death scream (set DS:0x2998) |
| 14 | 10800 / 1.20 | 108, 99 | 104, 99 | death scream (pair DS:0x29AA) |
| 15 | 7744 / 0.86 | 83, 99 | 77, 99 | death scream (pair DS:0x29AA) |
| 16 | 36992 / 4.11 | 217, 99 | 198, 99 | death scream (constant in 2390:0c59) |
| 17 | 18304 / 2.03 | 90, 99 | 144, 99 | death scream (constant in 2390:0c59) |
| 18 | 6880 / 0.76 | 92, 99 | 93, 99 | death scream (pair DS:0x29C2) |
| 19 | 6056 / 0.67 | 104, 99 | 81, 99 | death scream (pair DS:0x29C2) |
| 20 | 11008 / 1.22 | 117, 99 | 80, 99 | death scream (pair DS:0x29C6) |
| 21 | 8384 / 0.93 | 113, 99 | 82, 99 | death scream (pair DS:0x29C6) |
| 22 | 17732 / 1.97 | 167, 99 | 160, 99 | **player death scream** (`Died` 1823:0cda, `22 + rnd % 5`); also enemy set DS:0x29A0 |
| 23 | 15552 / 1.73 | 139, 99 | 144, 99 | **player death scream** (`Died` 1823:0cda, `22 + rnd % 5`); also enemy set DS:0x29A0 |
| 24 | 9632 / 1.07 | 98, 99 | 128, 99 | **player death scream** (`Died` 1823:0cda, `22 + rnd % 5`); also enemy set DS:0x29A0 |
| 25 | 15040 / 1.67 | 118, 99 | 186, 99 | **player death scream** (`Died` 1823:0cda, `22 + rnd % 5`); also enemy set DS:0x29A0 |
| 26 | 6672 / 0.74 | 76, 99 | 94, 99 | **player death scream** (`Died` 1823:0cda, `22 + rnd % 5`); also enemy set DS:0x29A0 |
| 27 | 11285 / 1.25 | 115, 99 | 155, 99 | death scream (constant in 2390:0c59; pair DS:0x29AE) |
| 28 | 13888 / 1.54 | 116, 99 | 123, 99 | death scream (constant in 2390:0c59; pair DS:0x29AE) |
| 29 | 8224 / 0.91 | 151, 99 | 204, 99 | no constant call site found |
| 30 | 11576 / 1.28 | 17, 98 | 29, 98 | actor set DS:0x29D8 (2390:21b7) |
| 31 | 7504 / 0.83 | 22, 98 | 38, 98 | actor set DS:0x29D8 (2390:21b7) |
| 32 | 3808 / 0.42 | 23, 98 | 26, 98 | actor set DS:0x29D8 (2390:21b7) |
| 33 | 4720 / 0.52 | 51, 98 | 30, 98 | actor set DS:0x29DE (2390:238b) |
| 34 | 9248 / 1.03 | 31, 98 | 41, 98 | actor set DS:0x29DE (2390:238b) |
| 35 | 6944 / 0.77 | 32, 98 | 160, 98 | actor set DS:0x29EA (2390:21b7) |
| 36 | 10240 / 1.14 | 55, 98 | 111, 98 | actor set DS:0x29EA (2390:21b7) |
| 37 | 3648 / 0.40 | 46, 98 | 44, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS) |
| 38 | 3648 / 0.40 | 8, 98 | 10, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS); actor set DS:0x29E2 (2390:238b) |
| 39 | 3152 / 0.35 | 5, 98 | 9, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS); actor set DS:0x29E2 (2390:238b) |
| 40 | 7040 / 0.78 | 49, 98 | 92, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS) |
| 41 | 3008 / 0.33 | 3, 98 | 34, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS) |
| 42 | 11352 / 1.26 | 148, 98 | 159, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS) |
| 43 | 10560 / 1.17 | 41, 98 | 78, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS) |
| 44 | 5760 / 0.64 | 58, 98 | 66, 98 | constant in 2390:238b (enemy attack, HYPOTHESIS) |
| 45 | 4576 / 0.51 | 6, 98 | 21, 98 | **player weapon fire**, `45 + gamestate.weapon` (weapon 0–4, 208c:1883); SFX-volume test with mixer type 2 (1618:060b); ally set DS:0x2BCA |
| 46 | 5616 / 0.62 | 23, 98 | 40, 98 | **player weapon fire**, `45 + gamestate.weapon` (weapon 0–4, 208c:1883); ally set DS:0x2BCA (2634:0214) |
| 47 | 5216 / 0.58 | 30, 98 | 39, 98 | **player weapon fire**, `45 + gamestate.weapon` (weapon 0–4, 208c:1883); ally set DS:0x2BCA (2634:0214) |
| 48 | 9344 / 1.04 | 264, 99 | 212, 99 | **player weapon fire**, `45 + gamestate.weapon` (weapon 0–4, 208c:1883) |
| 49 | 12420 / 1.38 | 33, 98 | 245, 98 | **player weapon fire**, `45 + gamestate.weapon` (weapon 0–4, 208c:1883); played as digitized #49 by the SB Pro SFX-volume test (1618:03c1) |
| 50 | 8448 / 0.94 | 82, 98 | 96, 98 | constant in `FirstSighting` (1e43:1f45) and 2390:21b7 (alert, HYPOTHESIS) |
| 51 | 3192 / 0.35 | 85, 98 | 64, 98 | sighting tables DS:0xF22 (`CheckSight` 1e43:1d35, `FirstSighting`) and DS:0x29CA (2390:21b7) |
| 52 | 4484 / 0.50 | 44, 98 | 82, 98 | sighting tables DS:0xF22 (`CheckSight` 1e43:1d35, `FirstSighting`) and DS:0x29CA (2390:21b7) |
| 53 | 6000 / 0.67 | 19, 98 | 100, 98 | sighting tables DS:0xF22 (`CheckSight` 1e43:1d35, `FirstSighting`) and DS:0x29CA (2390:21b7) |
| 54 | 2928 / 0.33 | 34, 98 | 25, 98 | sighting tables DS:0xF22 (`CheckSight` 1e43:1d35, `FirstSighting`) and DS:0x29CA (2390:21b7) |
| 55 | 2784 / 0.31 | 32, 98 | 46, 98 | sighting tables DS:0xF22 (`CheckSight` 1e43:1d35, `FirstSighting`) and DS:0x29CA (2390:21b7) |
| 56 | 3584 / 0.40 | 35, 98 | 75, 98 | sighting tables DS:0xF22 (`CheckSight` 1e43:1d35, `FirstSighting`) and DS:0x29CA (2390:21b7) |
| 57 | 15616 / 1.73 | 87, 98 | 117, 98 | constant in `FirstSighting` (1e43:1f45) and 2390:21b7 (alert, HYPOTHESIS) |
| 58 | 14656 / 1.63 | 196, 98 | 205, 98 | constant in `FirstSighting` (1e43:1f45) and 2390:21b7 (alert, HYPOTHESIS) |
| 59 | 6944 / 0.77 | 69, 98 | 85, 98 | no constant call site found |
| 60 | 10496 / 1.17 | 104, 98 | 138, 98 | constant in `FirstSighting` (1e43:1f45) and 2390:21b7 (alert, HYPOTHESIS) |
| 61 | 6272 / 0.70 | 102, 98 | 134, 98 | sighting tables DS:0xF22 (`CheckSight` 1e43:1d35, `FirstSighting`) and DS:0x29CA (2390:21b7) |
| 62 | 1328 / 0.15 | 35, 98 | 30, 98 | visor blood splat (30d6:0fbe, DS:0x33B4, every third splat) |
| 63 | 2240 / 0.25 | 25, 98 | 26, 98 | visor blood splat (30d6:0fbe, DS:0x33B4, every third splat) |
| 64 | 14368 / 1.59 | 44, 98 | 55, 98 | `PlayLoop` terrain sound, tile codes 117–120, counter DS:0x9DC = 1 (DS:0xAB4) |
| 65 | 3952 / 0.44 | 24, 98 | 30, 98 | `PlayLoop` terrain sound, tile codes 117–120, counter = 5 (DS:0xABA) |
| 66 | 3768 / 0.42 | 21, 98 | 43, 98 | `PlayLoop` terrain sound, tile codes 117–120, counter = 5 (DS:0xABA) |
| 67 | 14528 / 1.61 | 57, 98 | 138, 98 | `PlayLoop` terrain sound, tile codes 117–120, counter DS:0x9DC = 1 (DS:0xAB4) |
| 68 | 11740 / 1.30 | 3, 98 | 59, 98 | `PlayLoop` terrain sound, tile codes 117–120, counter DS:0x9DC = 1 (DS:0xAB4) |
| 69 | 3648 / 0.40 | 1, 98 | 19, 98 | `PlayLoop` terrain sound, tile codes 121–124 or 53, odd tics (DS:0xABE) |
| 70 | 2960 / 0.33 | 2, 98 | 1, 98 | `PlayLoop` terrain sound, tile codes 121–124 or 53, odd tics (DS:0xABE) |
| 71 | 11285 / 1.25 | 52, 98 | 59, 98 | explosion or fire spawn (30d6:0161, DS:0x32FE); `FirstSighting` constant; actor set DS:0x29E6 |
| 72 | 10208 / 1.13 | 58, 98 | 80, 98 | explosion or fire spawn (30d6:0161, DS:0x32FE) |
| 73 | 5728 / 0.64 | 1, 98 | 108, 98 | explosion or fire spawn (30d6:0161, DS:0x32FE); actor set DS:0x29E6 |
| 74 | 15424 / 1.71 | 194, 99 | 173, 99 | 30d6:12b1 (table DS:0x33BA) |
| 75 | 12128 / 1.35 | 171, 99 | 185, 99 | 30d6:12b1 (table DS:0x33BA); 208c:074f |
| 76 | 2920 / 0.32 | 1, 99 | 18, 99 | 1e43:174d (table DS:0xF1E; pain, HYPOTHESIS) |
| 77 | 2792 / 0.31 | 1, 99 | 6, 99 | 1e43:174d (table DS:0xF1E; pain, HYPOTHESIS) |
| 78 | 3998 / 0.44 | 51, 80 | 33, 80 | `Cmd_Use` on wall tiles 0x31/0x34 |
| 79 | 3712 / 0.41 | 20, 80 | 23, 80 | `Cmd_Use` on wall tiles 0x41/0x44/0x45/0x48 |
| 80 | 7552 / 0.84 | 27, 80 | 57, 80 | `Cmd_Use` on wall tiles 0x1D/0x20 |
| 81 | 5424 / 0.60 | 43, 80 | 43, 80 | `Cmd_Use` on wall tiles 0x0D/0x10 |
| 82 | 3440 / 0.38 | 51, 80 | 53, 80 | **door** open and close (2281:0868, 2281:0528) |
| 83 | 19520 / 2.17 | 196, 80 | 217, 80 | **push wall** (2281:0ce2) |
| 84 | 6896 / 0.77 | 62, 99 | 53, 99 | **generator switch** (`Cmd_Use` tiles 0x5F/0x60) |
| 85 | 2096 / 0.23 | 35, 90 | 89, 90 | **menu cursor move** (`HandleMenu`); also 208c:074f and 208c:1d24 (weapon change, HYPOTHESIS) |
| 86 | 34991 / 3.88 | 164, 99 | 1, 99 | **title screen** burst (`DemoLoop`) |
| 87 | 4576 / 0.51 | 2, 0 | 1, 0 | placeholder (copy of #45), never referenced |
| 88 | 4576 / 0.51 | 2, 99 | 2, 99 | `F1` cue, frames 0 and 185 (copy of #45) |
| 89 | 4576 / 0.51 | 3, 99 | 2, 99 | `F1` cue, frame 128 (copy of #45) |
| 90 | 4576 / 0.51 | 2, 99 | 3, 99 | `F1` cue, frame 258 (copy of #45) |
| 91 | 124928 / 13.87 | 3, 99 | 1, 99 | `F1` cue, frame 319 |
| 92 | 46592 / 5.17 | 227, 99 | 173, 99 | **`F6` death sequence**, frame 1 (Victor laughing) |
| 93 | 67480 / 7.49 | 200, 99 | 233, 99 | **`F2` ladder sequence**, frame 1 |
| 94 | 48640 / 5.40 | 194, 99 | 206, 99 | **`F3` stairs sequence**, frame 1 |
| 95 | 73216 / 8.13 | 2, 99 | 2, 99 | `F5` cue, frame 75 (`F5.BC` not shipped) |
| 96 | 56832 / 6.31 | 1, 99 | 2, 99 | `F5` cue, frame 130 (`F5.BC` not shipped) |
| 97 | 4576 / 0.51 | 3, 0 | 2, 0 | placeholder (copy of #45), never referenced |
| 98 | 4576 / 0.51 | 2, 0 | 7, 0 | placeholder (copy of #45), never referenced |
| 99 | — | 3, 0 | 2, 0 | digitized entry undefined (map has 99 entries) |

Ids with no constant call site may still be played through tables not yet decoded (actor state code in 2390, the ally code in 2634). The per-class indexing of the death-scream and sighting tables (DS:0x2998–0x29EC, DS:0xF1E–0xF31) needs the obclass switches in 2390:0c59 and 1e43:1f45, which the decompiler could not handle.

## 11. Port notes

- Play digitized sounds at **9,009 Hz** from `AUDIOMUS.BC`; ignore the sound pages in `GFXTILES.BC`.
- One digitized voice with the priority and fallback rules of §4.1; positioned sounds with the Wolf3D tables and the culling rule of §4.2.
- `US_RndT` consumers that look like presentation but share `rndindex`: `StartMusic` on maps ≥ 10, `Died` (scream choice), `PlayLoop` terrain sounds, title splats (GAMEFLOW §2), visor splats. A structure-preserving port must call them in the same order (or prove they cannot affect the simulation).
- Treat `cdmusic` as "music off" for fidelity; any CD-track mapping in OBCR is an enhancement with no original behaviour to match.
- AdLib music uses the OPL at a 700 Hz tick; when a mixer is present, `SD_MusicOff` fades the FM volume over about `5 × (volume − 2)` tics before stopping.

## 12. Open questions

- Which actor class uses which death-scream and sighting table entry (manual read of 2390:0c59 and 1e43:1f45).
- Contexts for ids 29, 59, 74 and the 37–44 group; the meaning of the terrain-sound tiles 117–124 and 53.
- The type-2 mixer chip (registers 0x03–0x0F).
- Confirm by ear in DOSBox-X: pitch at 9,009 Hz, the FM fade-out in `SD_MusicOff`, and the names in §10.
