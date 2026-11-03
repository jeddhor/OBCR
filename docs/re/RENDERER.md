# OBC 3D renderer and visual effects

Reverse-engineering notes for the 3D view of `COUNT.EXE` (release `OBC-A`): frame order, floors and ceilings, walls, doors, sprites, projection, palette effects and screen shake. Written for a pixel-exact reimplementation in OBCR.

Status: static analysis of the Ghidra text export (2026-11-02). Nothing here has been traced in DOSBox-X yet. Confidence tags follow AGENT_GUIDE §4. `CONFIRMED` below means "read instruction by instruction in the disassembly" (or a data-file parse); it still needs a capture to become a golden test.

Conventions:

- Addresses are `ghidra SSSS:OOOO` (Ghidra base segment 0x1000, so image segment = SSSS − 0x1000; see SYMBOLS.md). DGROUP data is `DS:0xNNNN`. Initialized DGROUP data (DS < about 0x4230) is at file offset DS + 0x35580; higher DS offsets are BSS (zero at start).
- Several assembly routines were re-disassembled outside Ghidra with `llvm-objdump --triple=i386-unknown-unknown-code16` because Ghidra stops at Borland floating-point emulator calls (`INT 34h`–`3Dh`; `INT 34h+n` = x87 escape `D8h+n`, `INT 3Dh` = `FWAIT`). The helper used is a throwaway scratch script; any equivalent works.
- Wolf3D names are used for matched routines. Names in `count_exe.csv` that this document corrects are listed in §12.

---

## 1. Routine map

| Ghidra | Role (this doc) | Wolf3D counterpart | Confidence |
|---|---|---|---|
| `1b4a:129c` | `ThreeDRefresh` | `WL_DRAW.C ThreeDRefresh` | CONFIRMED |
| `1b4a:0bc9` | Floor/ceiling gradient, "ramp" mode (F5) | `VGAClearScreen` | CONFIRMED |
| `1b4a:0c72` | Floor/ceiling gradient, "colour" mode (F6, default) | `VGAClearScreen` | CONFIRMED |
| `1b4a:11b1` | `WallRefresh` (view setup only) | first half of `WallRefresh` | CONFIRMED |
| `1b4a:1273` | Partial-tile setup + `lastside = -1` | second half of `WallRefresh` | CONFIRMED |
| `1d2a:007a` | `AsmRefresh` (ray caster, three modes) | `WL_DR_A.ASM AsmRefresh` | CONFIRMED |
| `1d2a:000e` / `1d2a:0044` | `xpartialbyystep` / `ypartialbyxstep` | same | CONFIRMED |
| `1b4a:049a` | `HitVertWall` | same | CONFIRMED |
| `1b4a:05f1` | `HitHorizWall` | same | CONFIRMED |
| `1b4a:074e` | `HitHorizDoor` | same | CONFIRMED |
| `1b4a:08a1` | `HitVertDoor` | same | CONFIRMED |
| `1b4a:09f3` | `HitHorizPWall` | same | CONFIRMED |
| `1b4a:0ade` | `HitVertPWall` | same | CONFIRMED |
| `1b4a:0347` | `CalcHeight` | same, unchanged | CONFIRMED |
| `1b4a:03f3` (near) / `1b4a:0492` (far thunk) | `ScalePost` (adds distance shading) | same | CONFIRMED |
| `1d8b:001f` | `SetupScaling` | same | LIKELY |
| `1d8b:0230` | `BuildCompScale` (new code generator) | same | CONFIRMED |
| `2f4d:0adf` | "is door-frame wall type" test | — (OBC) | CONFIRMED |
| `30d6:11b6` | Build distance-shade table `DS:0xC2CA` (floating point) | — (OBC) | CONFIRMED |
| `1497:0a7b` | `BuildTables` (floating point, as Wolf3D) | same | CONFIRMED |
| `1497:0bf9` | `CalcProjection` (floating point, as Wolf3D) | same | CONFIRMED |
| `1497:1083` | `SetViewSize` | same, extended | CONFIRMED |
| `1497:119b` | `NewViewSize` | same | CONFIRMED |

