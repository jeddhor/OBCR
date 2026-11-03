# COUNT.EXE symbols and landmarks

Human-readable index of important addresses. The complete, machine-readable symbol table is `re/symbols/count_exe.csv` (created in M0.12; Ghidra imports and exports it). This file holds the conventions, the segment map, and landmarks worth knowing by heart.

## 1. Address conventions

See AGENT_GUIDE §4.3. Summary for release `OBC-A`:

| Quantity | Value |
|---|---|
| MZ header size | 0x3400 bytes |
| Image-linear offset | file offset − 0x3400 |
| Runtime segment of image segment `S` | `L + S`, where `L` = CS at the program entry stop (header CS = 0) |
| DGROUP (DS after startup) | image paragraph 0x3218; runtime `L + 0x3218`; DS offset = file offset − 0x35580 |
| Ghidra base segment `G` | **TBD in M0.12** (record here after the first import) |

CSV columns (`re/symbols/count_exe.csv`): `img_linear, seg, off, file_off, kind (func|data|label|table), name, type, wolf3d_name, confidence, source, notes`.

## 2. Segment map

Segments that contain relocation entries, from the MZ relocation table (CONFIRMED). This is a lower bound on the segment list: modules without far references contribute no entries. Module attribution is M2.R1.

| Image seg | File offset | Relocs | Probable module | Confidence |
|---|---|---|---|---|
| 0x0000 | 0x03400 | 18 | C0 startup / RTL (`_TEXT`) | HYPOTHESIS |
| 0x0494 | 0x07D40 | 1 | | UNKNOWN |
| 0x0497 | 0x07D70 | 319 | | UNKNOWN |
| 0x0618 | 0x09580 | 229 | | UNKNOWN |
| 0x0823 | 0x0B630 | 116 | | UNKNOWN |
| 0x0942 | 0x0C820 | 143 | | UNKNOWN |
| 0x0ADB | 0x0E1B0 | 99 | | UNKNOWN |
| 0x0B4A | 0x0E8A0 | 55 | | UNKNOWN |
| 0x0D2A | 0x106A0 | 7 | | UNKNOWN |
| 0x0D8B | 0x10CB0 | 17 | | UNKNOWN |
| 0x0E43 | 0x11830 | 147 | | UNKNOWN |
| 0x108C | 0x13CC0 | 134 | | UNKNOWN |
| 0x1281 | 0x15C10 | 26 | | UNKNOWN |
| 0x1390 | 0x16D00 | 244 | | UNKNOWN |
| 0x1634 | 0x19740 | 60 | | UNKNOWN |
| 0x173A | 0x1A7A0 | 123 | | UNKNOWN |
| 0x188D | 0x1BCD0 | 22 | | UNKNOWN |
| 0x194C | 0x1C8C0 | 34 | | UNKNOWN |
| 0x1A1D | 0x1D5D0 | 88 | | UNKNOWN |
| 0x1B79 | 0x1EB90 | 51 | | UNKNOWN |
| 0x1D04 | 0x20440 | 7 | | UNKNOWN |
| 0x1D35 | 0x20750 | 30 | | UNKNOWN |
| 0x1DDB | 0x211B0 | 2 | | UNKNOWN |
| 0x1DDF | 0x211F0 | 35 | | UNKNOWN |
| 0x1E72 | 0x21B20 | 25 | | UNKNOWN |
| 0x1F4D | 0x228D0 | 89 | | UNKNOWN |
| 0x20D6 | 0x24160 | 90 | | UNKNOWN |
| 0x22A2 | 0x25E20 | 79 | | UNKNOWN |
| 0x2371 | 0x26B10 | 24 | | UNKNOWN |
| 0x23BB | 0x26FB0 | 226 | | UNKNOWN |
| 0x258F | 0x28CF0 | 170 | | UNKNOWN |
| 0x277A | 0x2ABA0 | 1 | Contains `rndtable` (file 0x2D8F2) → `ID_US_A`-like data, possibly several small ASM modules | HYPOTHESIS |
| 0x2A61 | 0x2DA10 | 9 | Near palette candidates and scan-code tables | HYPOTHESIS |
| 0x3218 | 0x35580 | 213 | DGROUP (`_DATA`, `_BSS`, stack) | CONFIRMED |

For comparison, `WOLFSRC/WOLF3D.MAP` lines 1–60 list Wolf3D's segment table (`WL_MAIN_TEXT`, `WL_DRAW_TEXT`, `ID_CA_TEXT`, …), in link order. OBC's link order is probably similar, which helps attribution.

## 3. Landmarks

| Image-linear | File offset | DS offset | Name | Kind | Wolf3D counterpart | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| 0x00000 | 0x03400 | | entry (`_start`) | func | C0 startup | CONFIRMED | Header CS:IP = 0000:0000 |
| 0x2A4F2 | 0x2D8F2 | | `rndtable` | table[256] u8 | `rndtable` (`ID_US_A.ASM:19`) | CONFIRMED | Byte-identical to Wolf3D |
| ≈0x2A9B0 | ≈0x2DDB0 | | palette candidate 1 | table[768] | `gamepal` | LIKELY | Exact start TBD (Q-025) |
| ≈0x2ACB0 | ≈0x2E0B0 | | palette candidate 2 | table[768] | — | LIKELY | Q-025 |
| 0x2B2B2 | 0x2E6B2 | | scan-code → ASCII (unshifted) | table | `ASCIINames` (`ID_IN.C`) | LIKELY | |
| 0x2B332 | 0x2E732 | | scan-code → ASCII (shifted) | table | `ShiftNames` (`ID_IN.C`) | LIKELY | |
| 0x32180 | 0x35580 | 0x0000 | DGROUP base | data | DGROUP | CONFIRMED | |
| 0x34FE4 | 0x383E4 | 0x2E64 | default high scores | table[7] of 28 bytes | `Scores` (`ID_US_1.C`) | CONFIRMED | Hank 11200 … Arnold 9250 |

More string anchors with DS offsets: [STRINGS.md](STRINGS.md).
