# OBC data formats

Specification of every original file as far as it is known. Each fact carries a confidence tag (AGENT_GUIDE §4). All multi-byte values are little-endian. Sizes and counts are for release `OBC-A` (the copy in `..\OBC`, file dates 1996-12-24) unless noted.

Wolf3D references: `WOLFSRC/ID_CA.C` (cache manager), `WOLFSRC/ID_PM.C` (page manager), `WOLFSRC/ID_SD.C` (sound). ECWolf readers: `ECWolf/src/resourcefiles/file_vswap.cpp`, `file_gamemaps.cpp`, `file_vgagraph.cpp`, `file_audiot.cpp`.

---

## 1. File inventory (release OBC-A)

| File | Size | Role | Wolf3D equivalent | Status |
|---|---|---|---|---|
| `COUNT.EXE` | 235,440 | Main program | `WOLF3D.EXE` | §9 |
| `GFXTILES.BC` | 2,084,536 | Page file: walls, sprites, digitized sounds | `VSWAP.WL6` | CONFIRMED §2 |
| `MAPHEAD.BC` | 402 | Map directory | `MAPHEAD.WL6` | CONFIRMED §3 |
| `MAPTEMP.BC` | 351,762 | Maps (RLEW only) | `GAMEMAPS.WL6` (Carmack + RLEW) | CONFIRMED §3 |
| `VGAHEAD.BC` | 177 | Graphics chunk offsets | `VGAHEAD.WL6` | CONFIRMED §4 |
| `VGADICT.BC` | 1,024 | Huffman dictionary | `VGADICT.WL6` | CONFIRMED §4 |
| `VGAGRAPH.BC` | 317,566 | Graphics chunks | `VGAGRAPH.WL6` | CONFIRMED §4 |
| `AUDIOHED.BC` | 1,244 | Audio chunk offsets | `AUDIOHED.WL6` | CONFIRMED §5 |
| `AUDIOT.BC` | 197,419 | PC speaker, AdLib, music chunks | `AUDIOT.WL6` | LIKELY §5 |
| `AUDIOMUS.BC` | 1,376,256 | PCM background music | — (new) | LIKELY §6 |
| `CONFIG.DAT` | 287 | High scores and settings | `CONFIG.WL6` | §7 |
| `SVGM0.SAV`, `SVGM2.SAV` | 19,188 each | Save games | `SAVEGAM?.WL6` | UNKNOWN §8 |
| `AUTSV.SAV` | 18,594 | AutoSave slot | — | UNKNOWN §8 |
| `SETUP.EXE` | 55,134 | Sound/control setup program | — | Not needed by the port (behavior to note in M10) |
| `README.EXE`, `README.TXT` | 38,702 / 4,585 | Manual supplement viewer and text | — | Reference only |
| `BC.BAT`, `BCLOG.BAT`, `PRINTLOG.BAT` | — | Launcher (runs `COUNT %1..%4`, appends `MEM`, `DIR`, `VER`, `CONFIG.SYS`, `AUTOEXEC.BAT` to `BC.LOG` on error), log viewers | — | CONFIRMED |
| `SETUP.PIF` | 967 | Windows PIF | — | Ignore |

The file extension is `.BC`. `COUNT.EXE` builds names as `<BASE>.` + extension (strings `VGAHEAD.`, `GFXTILES.`, … and `.bc`). CONFIRMED (STRINGS.md).

### 1.1 Rendered sequences and install archives

The installed copy in `..\OBC` is a Minimal install. The owner's floppy distribution (`Operation Body Count (1994)(Capstone).zip` and `Operation Body Count.zip` in the workspace root; the latter holds three 1.44 MB FAT12 images with the same files) contains `INSTALL.EXE`, `UNCMP.EXE`, `README.*` and three archives:

| Archive | Format | Contents |
|---|---|---|
| `BC1.CMP` | Plain PKZIP (deflate) | `GFXTILES.BC`, `VGAGRAPH.BC`, `VGAHEAD.BC`, `VGADICT.BC`, `MAPHEAD.BC`, `MAPTEMP.BC` |
| `BC2.CMP` | Plain PKZIP | `SETUP.EXE`, `BC.BAT`, `BCLOG.BAT`, `PRINTLOG.BAT`, `AUDIOT.BC`, `AUDIOMUS.BC`, `AUDIOHED.BC`, `COUNT.EXE` |
| `BC3.CMP` | Plain PKZIP | `F2.BC` (843,532), `F3.BC` (740,692), `F4.BC` (96,492), `F6.BC` (207,508) — rendered sequences |

Every file in `BC1.CMP` and `BC2.CMP` is byte-identical to the installed copy (CONFIRMED by comparison). The development data directory `.local/data/obc-full/` holds the installed files plus the four sequences.

**Rendered sequences** are Autodesk Animator Pro **FLC** files (CONFIRMED from headers): `u32 size` (equals file size), `u16 magic = 0xAF12`, `u16 frames` (F2: 31, F3: 30, F4: 54, F6: 34), `u16 width = 320`, `u16 height = 200`, `u16 depth = 8`, `u16 flags = 3`, `u32 speed` in milliseconds (F2 and F6: 142; F3 and F4: 71). Chunk types to support: `COLOR_256` (4), `COLOR_64` (11), `DELTA_FLC` (7), `DELTA_FLI` (12), `BLACK` (13), `BYTE_RUN` (15), `LITERAL` (16), `PSTAMP` (18, skip). Usage: Q-015.

---

## 2. GFXTILES.BC — page file (VSWAP)

CONFIRMED by parsing.

| Offset | Type | Value (OBC-A) | Meaning |
|---|---|---|---|
| 0 | u16 | 937 | `ChunksInFile` |
| 2 | u16 | 136 | `PMSpriteStart` |
| 4 | u16 | 816 | `PMSoundStart` |
| 6 | u32[937] | | Page offsets (0 = sparse; none are sparse in OBC-A) |
| 6 + 4·937 | u16[937] | | Page lengths |
| 0x1600 | | | First page data (512-byte aligned) |

- **Walls**: pages 0–135, all exactly 4,096 bytes: 64×64 palette indices, **column-major** (64 bytes per column). 68 light/dark pairs (Wolf3D convention: even = light, odd = dark; confirm in M4.R5).
- **Sprites**: pages 136–815 (680 sprites). Wolf3D `t_compshape`: `u16 leftpix, rightpix; u16 dataofs[rightpix − leftpix + 1]`, then per-column post lists of `(u16 end·2, i16 corrected_top, u16 start·2)` terminated by `0`, followed by pixel data. Page sizes 134–3,706 bytes. Observed `leftpix`/`rightpix` within 0–63.
- **Digitized sounds**: pages 816–935 hold 8-bit unsigned mono PCM; page 936 (the last) is the **sound info list**: 46 pairs of `(u16 start_page_relative_to_PMSoundStart, u16 length_bytes)`. A sound spans consecutive pages until `length` bytes are collected. First pairs: (0, 5996), (2, 3793), (3, 19995), (8, 10022), … Sample rate UNKNOWN (Q-005; Wolf3D ≈ 7042 Hz).

---

## 3. MAPHEAD.BC and MAPTEMP.BC — maps

CONFIRMED by parsing all 61 maps.

### 3.1 MAPHEAD.BC

| Offset | Type | Meaning |
|---|---|---|
| 0 | u16 | RLEW tag = `0xABCD` |
| 2 | i32[100] | Map header offsets into MAPTEMP; `-1` (0xFFFFFFFF) = unused |

402 bytes total; no tile-info appendix. Slots 0–60 used.

### 3.2 MAPTEMP.BC

- Starts with the 8-byte signature `TED5v1.0`. Map 0's header follows immediately at offset 8.
- **Map header** (38 bytes, Wolf3D `maptype`):

| Offset | Type | Meaning |
|---|---|---|
| 0 | i32[3] | `planestart` (file offsets of planes 0–2) |
| 12 | u16[3] | `planelength` (compressed bytes) |
| 18 | u16 | width (always 64) |
| 20 | u16 | height (always 64) |
| 22 | char[16] | name, NUL-terminated, garbage after NUL |

- **Plane encoding: RLEW only** (no Carmack). Each plane: `u16 expanded_length_bytes` (always 8,192 = 64·64·2), then RLEW words: `tag, count, value` expands to `count` copies of `value`; any other word is literal. This is Wolf3D's non-`CARMACIZED` `MAPTEMP` path; ECWolf handles it (`file_gamemaps.cpp:60`, "Maptemp = RLEW").
- The string `!ID!` follows the plane data (TED5 marker).
- **Plane 2 is all zero in every map.**

### 3.3 Map list

| Slot | Name | Slot | Name |
|---|---|---|---|
| 0–4 | SEWER LEVEL 1–5 | 40–49 | CRED1–CRED10 |
| 5 | Maintence Level | 50 | NETLEVEL 5 |
| 6 | UN BLDG1 LOBBY | 51 | NETLEVEL 8 |
| 7–21 | UN BLDG1 FLR2–FLR16 | 52 | NETLEVEL 9 |
| 22 | UN BLDG1 TOP | 53 | NETLEVEL 10 |
| 23–38 | BLDG2 FLR1–FLR16 | 54 | NETLEVEL 6 |
| 39 | BLDG2 TOP | 55 | NETLEVEL 7 |
| | | 56 | NETLEVEL 4 |
| | | 57 | NETLEVEL 2 |
| | | 58 | NETLEVEL 3 |
| | | 59 | NETLEVEL 1 |
| | | 60 | CRED9 (all planes empty, 8-byte planes) |

CRED maps are tiny (plane 0 compresses to 384 bytes, plane 1 to 96 bytes).

### 3.4 Plane value statistics (all 61 maps)

Meanings are **UNKNOWN** until M2.R9; these statistics guide the investigation (Q-001, Q-002).

- **Plane 0**: 103 distinct values. Most frequent: 91 (48,671), 40 (42,823), 144 (23,732), 148 (23,280), 121 (8,697), 149 (8,439), 22 (8,192), 98 (6,763), 151 (6,737), 7 (6,416), 145 (5,943), 48 (4,787), 146 (4,648), 147 (4,005), 19 (3,873), 5 (3,463), 150 (2,687). Values 144–151 fill walkable space; 145 appears in regular grids inside 144 regions (every third tile), which suggests per-tile floor or ceiling variation rather than area numbers. 40 and 91 fill solid regions and map borders.
- **Plane 1**: 215 distinct values, range 0–317. Value **18 is the background** (214,093 of 249,856 tiles) where Wolf3D uses 0. Value 89 (17,178) and 103/104 appear on wall cells (for example `89 103 89` across a door flanked by walls), suggesting per-wall attributes. Values above 255 exist (270–317).

Tools: `obc-tool maps render`, `maps find` (M1.8, M2.3).

---

## 4. VGAHEAD.BC, VGADICT.BC, VGAGRAPH.BC — graphics

CONFIRMED by parsing (counts, sizes); roles LIKELY.

- **VGAHEAD**: 59 three-byte offsets (`0xFFFFFF` = sparse) → 58 chunks.
- **VGADICT**: Huffman nodes of `(u16 bit0, u16 bit1)`; 1,024 bytes = 256 node slots, 255 used; values < 256 are leaves, values ≥ 256 point to node `value − 256`; node 254 is the root. Verified by decoding every chunk to its declared length.
- **VGAGRAPH chunk**: `u32 expanded_length` then Huffman data (LSB-first bits).

| Chunk | Expanded size | Content |
|---|---|---|
| 0 | 212 | Pic table: 53 × `(u16 width, u16 height)` |
| 1 | 12,314 | Font (LIKELY: Wolf3D `fontstruct`: `u16 height; u16 location[256]; u8 width[256]; data`) |
| 2 | 1,650 | Font (LIKELY) |
| 3–55 | | Pics 0–52 (pic *i* = chunk 3 + *i*), unchained-VGA planar layout (4 planes stored sequentially, each width/4 × height) |
| 56, 57 | 5 each | UNKNOWN (Q-008) |

Pic dimensions (index: size): 0: 8×8 · 1–8: 320×200 (eight full screens) · 9–19: 8×8 · 20: 16×16 · 21: 32×48 · 22–28: 8×8 · 29: 320×40 (status-bar sized) · 30–35: 64×32 · 36–52: 32×32. Roles: Q-009.

---

## 5. AUDIOHED.BC and AUDIOT.BC — audio chunks

- **AUDIOHED**: 311 × u32 offsets into AUDIOT → 310 chunks. CONFIRMED.
- **Layout**: LIKELY Wolf3D's `STARTPCSOUNDS = 0`, `STARTADLIBSOUNDS = NUMSOUNDS`, `STARTDIGISOUNDS = 2·NUMSOUNDS`, `STARTMUSIC = 3·NUMSOUNDS` with **`NUMSOUNDS = 100`**: chunks 200–299 have zero length (digitized placeholders), chunk 300 is 4 bytes, chunks 301–309 are 8–25 KB (IMF music). `!ID!` markers occur at AUDIOT offsets 7,565, 18,839 and 18,843. Exact boundaries: Q-007.
- **PC speaker sound** (Wolf3D `PCSound`): `u32 length; u16 priority; u8 data[length]` (one byte per service tick, value × 60 = PIT divisor, 0 = silence). Chunk 0 begins `length = 109, priority = 99`. CONFIRMED for chunk 0's header.
- **AdLib sound** (Wolf3D `AdLibSound`): `u32 length; u16 priority; Instrument inst (16 bytes); u8 block; u8 data[length]`. LIKELY.
- **IMF music** (Wolf3D `MusicGroup`): `u16 length; then (u8 reg, u8 value, u16 delay)` records. Rate LIKELY 700 Hz (Q-006).

---

## 6. AUDIOMUS.BC — PCM background music

LIKELY raw unsigned 8-bit PCM.

- Size 1,376,256 = 0x150000 = exactly 21 × 64 KiB.
- Byte histogram peaks at 0x80 and neighbors (128: 3,651; 126: 2,511; 129: 2,475 …) with many 0x00 bytes (11,668 in the first 64 KiB) and a zero-filled tail: consistent with audio blocks padded to 64 KiB boundaries.
- Mean absolute difference between consecutive bytes ≈ 14.7: smooth, audio-like.
- Associated UI: `BCKGRND MUSIC` and `FM MUSIC VOLUME` mixer labels.
- UNKNOWN: block or track directory, sample rate, looping, how tracks map to levels (Q-004).

---

## 7. CONFIG.DAT

| Offset | Size | Content | Confidence |
|---|---|---|---|
| 0x00 | 7 × 28 | High scores: `char name[20]; i32 score; u16 completed; u16 episode` | CONFIRMED (same record layout as the default table in `COUNT.EXE` at file 0x383E4: Hank 11200, Oliver 10100, Sam 9800, Ralph 9800, Alf 9600, Eb 9500, Arnold 9250, each `completed = 1`) |
| 0xC4 | 91 | Settings | LIKELY Wolf3D `WriteConfig` order, extended |

Settings decoding (HYPOTHESIS until M1.R6): `SoundMode = 1`, `MusicMode = 0`, `DigiMode = 0`, `mouseenabled = 1`, `joystickenabled = 0`, `joypadenabled = 0`, `joystickprogressive = 0`, `joystickport = 0`; `dirscan[4] = 0x48, 0x4D, 0x50, 0x4B` (Up, Right, Down, Left scan codes); then about 14 `buttonscan` entries (`0x1D` Ctrl, `0x32` M, `0x36` RShift, `0x39` Space, `0x02`–`0x06` keys 1–5, `0x1C` Enter, `0x1F` S, `0x17` I, `0x0F` Tab, `0x38` Alt), then `buttonmouse[4]`, `buttonjoy[4]`, `viewsize`, `mouseadjustment` and OBC additions (mixer levels?). Wolf3D has 8 `buttonscan` entries; OBC has more.

---

## 8. Save files

UNKNOWN layout (Q-013). Observations:

- `SVGM0.SAV` and `SVGM2.SAV` are 19,188 bytes; `AUTSV.SAV` is 18,594 bytes. Size differences may reflect actor counts or a different header.
- `SVGM0.SAV` begins with `0x00` followed by the description `Thorsten`. A score of 18,550 appears both as an i32 (`0x4876`) near offset 0x1E and as ASCII `18550` at offset 0x5A (perhaps a cached status-bar string).
- File names: `svgm0.sav` (pattern `svgmN.sav`), `autsv.sav` (AutoSave).

---

## 9. COUNT.EXE

| Property | Value | Confidence |
|---|---|---|
| Format | DOS MZ, 16-bit real mode | CONFIRMED |
| Compiler | Borland C++ (RTL string "Borland C++ - Copyright 1991 Borland Intl."), TLINK signature at header offset 0x1C | CONFIRMED |
| Packing | None (image size equals file size; no LZEXE/PKLITE stub) | CONFIRMED |
| Header | 0x3400 bytes (832 paragraphs); 2,933 relocations at 0x3E | CONFIRMED |
| Entry | CS:IP = 0000:0000; SS:SP = 4115:0080 | CONFIRMED |
| Segments | Relocation entries fall in 34 distinct segments (a lower bound on the segment count; see SYMBOLS.md), highest 0x3218 | CONFIRMED |
| DGROUP | Image paragraph 0x3218 = file 0x35580 (4 zero bytes, then the RTL copyright) | CONFIRMED |
| Memory model | Medium (far code, near data) | LIKELY (Wolf3D uses it; near-data RTL message "Null pointer assignment") |
| CPU | 286 minimum ("Sorry, this program requires a 286 or better."); 386 paths (`no386` parameter) | CONFIRMED (strings) |

Embedded tables:

| Table | File offset | Image-linear | Confidence |
|---|---|---|---|
| `rndtable[256]` (identical to `WOLFSRC/ID_US_A.ASM:19`) | 0x2D8F2 | 0x2A4F2 | CONFIRMED |
| Palette candidate 1 (768 bytes of 6-bit values) | ≈0x2DDB0 | ≈0x2A9B0 | LIKELY (Q-025) |
| Palette candidate 2 (same first 12 bytes as candidate 1) | ≈0x2E0B0 | ≈0x2ACB0 | LIKELY (Q-025) |
| Scan-code to ASCII tables (unshifted, shifted) | 0x2E6B2, 0x2E732 | | LIKELY (`ID_IN`) |
| Default high scores (7 × 28 bytes) | 0x383E4 | DS:0x2E64 | CONFIRMED |
