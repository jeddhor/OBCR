# OBC 3D renderer and visual effects

Reverse-engineering notes for the 3D view of `COUNT.EXE` (release `OBC-A`): frame order, floors and ceilings, walls, doors, sprites, projection, palette effects and screen shake. Written for a pixel-exact reimplementation in OBCR.

Status: static analysis of the Ghidra text export (2026-11-02). Nothing here has been traced in DOSBox-X yet. Confidence tags follow AGENT_GUIDE §4: `CONFIRMED` below means "read instruction by instruction in the disassembly" (or a data-file parse); every such claim still needs a capture before it becomes a golden test.

Conventions:

- Code addresses are `ghidra SSSS:OOOO` (Ghidra base segment 0x1000, so image segment = SSSS − 0x1000; SYMBOLS.md). DGROUP data is `DS:0xNNNN`. Initialized DGROUP data (DS below about 0x4230) is at file offset DS + 0x35580; higher DS offsets are BSS (zero at start).
- Far data: raw segment immediates in the code are image segments (Ghidra listings show them +0x1000). File offset = image segment × 16 + offset + 0x3400.
- Several assembly and floating-point routines were re-disassembled outside Ghidra with `llvm-objdump --triple=i386-unknown-unknown-code16`, after rewriting Borland's emulator calls (`INT 34h+n` → x87 escape `D8h+n`, `INT 3Dh` → `FWAIT`). Ghidra stops decoding at those interrupts, so its listings of `30d6:11b6`, `1497:0a7b`, `1497:0bf9`, `1d8b:0230`, `1d8b:0a47` and `1b4a:162b` are wrong or truncated.
- "Wolf3D" means `WOLFSRC` v1.4. Deltas are marked **Δ**.

---

## 1. Routine map

| Ghidra | Role | Wolf3D counterpart | Confidence |
|---|---|---|---|
| `1b4a:129c` | `ThreeDRefresh` | `WL_DRAW.C ThreeDRefresh` | CONFIRMED |
| `1b4a:0bc9` | Background gradient, "ramp" mode (F5) | `VGAClearScreen` | CONFIRMED |
| `1b4a:0c72` | Background gradient, "colour" mode (F6, default) | `VGAClearScreen` | CONFIRMED |
| `1b4a:11b1` | `WallRefresh` (view setup only) | first half of `WallRefresh` | CONFIRMED |
| `1b4a:1273` | Partial-tile setup and `lastside = -1` | second half of `WallRefresh` | CONFIRMED |
| `1d2a:007a` | `AsmRefresh` (ray caster; three passes) | `WL_DR_A.ASM AsmRefresh` | CONFIRMED |
| `1d2a:000e`, `1d2a:0044` | `xpartialbyystep`, `ypartialbyxstep` | same | CONFIRMED |
| `1b4a:049a` / `05f1` | `HitVertWall` / `HitHorizWall` | same | CONFIRMED |
| `1b4a:08a1` / `074e` | `HitVertDoor` / `HitHorizDoor` | same | CONFIRMED |
| `1b4a:0ade` / `09f3` | `HitVertPWall` / `HitHorizPWall` | same | CONFIRMED |
| `1b4a:0347` | `CalcHeight` (unchanged) | same | CONFIRMED |
| `1b4a:03f3` (near), `1b4a:0492` (far thunk) | `ScalePost` (adds distance shading) | same | CONFIRMED |
| `1b4a:0048`, `1b4a:01af` | `TransformActor`, `TransformTile` (unchanged apart from divide-by-zero `Quit`s) | same | LIKELY |
| `1b4a:0d98` | `DrawScaleds` (two-call variant) | same | CONFIRMED |
| `1b4a:0d1b` | `CalcRotate` | same | LIKELY |
| `1b4a:1013` | `DrawPlayerWeapon` (two-part weapon) | same | CONFIRMED |
| `1b4a:1494` | Per-tile pass: textured ceiling/floor tiles + shootable-wall search | — (OBC) | CONFIRMED |
| `1b4a:1b8b` | *IsShootableTile*(x, y) → `DS:0x0D22` (§9.1) | — (OBC) | CONFIRMED |
| `1b4a:1ccf` | Tile-centre projection for the search (TransformActor variant, §9.1) | `TransformActor` | CONFIRMED |
| `1b4a:1b36` | *ScoreShootableTile*(x, y): keep the best `\|dx\|·\|dy\|/2` (§9.1) | — (OBC) | CONFIRMED |
| `1b4a:162b` | Project one tile's ceiling quad | — (OBC) | CONFIRMED |
| `1b4a:19ef` | Floor-code lookup from neighbours | — (OBC) | CONFIRMED |
| `30d6:1814` | Rasterise one ceiling/floor quad | — (OBC) | CONFIRMED |
| `30d6:1a21` | Draw one textured ceiling span and its mirrored floor span | — (OBC) | CONFIRMED |
| `30d6:11b6` | Build distance-shade table `DS:0xC2CA` (floating point) | — (OBC) | CONFIRMED |
| `2f4d:02b7` | Build `DS:0xE598` (`i*3`) then call `30d6:11b6` | — (OBC) | CONFIRMED |
| `2f4d:0adf` | "Wall type has a door frame" test | — (OBC) | CONFIRMED |
| `1d8b:001f` | `SetupScaling` | same | LIKELY (decompiler read) |
| `1d8b:0230` | `BuildCompScale` (new emitted code) | same | CONFIRMED |
| `1d8b:0593` | `ScaleLine` (passes the shade) | same | CONFIRMED |
| `1d8b:070a` | `ScaleShape` (adds shading) | same | CONFIRMED |
| `1d8b:0a47` | `SimpleScaleShape` (optional ×1.1 scale) | same | CONFIRMED |
| `1497:0a7b` | `BuildTables` (floating point) | same | CONFIRMED |
| `1497:0bf9` | `CalcProjection` (floating point) | same | CONFIRMED |
| `1497:1083` | `SetViewSize` (extended) | same | CONFIRMED |
| `1497:119b` | `NewViewSize` | same | CONFIRMED |
| `208c:0009` | Weapon-overlay placement per view height | — (OBC) | CONFIRMED |
| `1942:0db5` | `InitRedShifts` | same | LIKELY (decompiler read) |
| `1942:0fc9` / `1942:10c9` | `UpdatePaletteShifts` / `FinishPaletteShifts` | same | LIKELY (decompiler read) |
| `2f4d:035e` | Select game palette (normal / dark / picture) | — (OBC) | CONFIRMED |
| `2f4d:0d17` | Overhead map: −1 → 0 | — (OBC) | CONFIRMED |
| `2f4d:0d31` | Draw the overhead map and ThermoSensor dots (§9.2) | — (OBC) | CONFIRMED |
| `1618:1480` | `DrawOutline(x, y, w, h, c1, c2)` (c1 top/left, c2 bottom/right) | `WL_MENU.C DrawOutline` (colours swapped) | CONFIRMED |
| `2ddf:04f9` | `VL_Plot(x, y, colour)` | `ID_VL.C VL_Plot` | CONFIRMED |
| `30d6:1b3d` | Find the level's lighting marker tile | — (OBC) | LIKELY |
| `2ddf:0209` | `VL_SetPalette(pal, keep16)` | `VL_SetPalette` (extra argument) | CONFIRMED |

---

## 2. ThreeDRefresh (`ghidra 1b4a:129c`)

Order of operations (CONFIRMED from `asm/0C73C_FUN_1b4a_129c.asm`):

1. `out 3C4h, 2` (select Map Mask); `bufferofs += screenofs` (`DS:0x8D98 += DS:0x8A26`). Wolf3D order.
2. **Δ** If `DS:0xC3CC < 0` call `2f4d:0d17` (switches the overhead map overlay off; both branches store 0; §9.2).
3. **Δ** Background: if `floormode` (`DS:0x33D2`, byte) is 0 call the ramp gradient `1b4a:0bc9`, else the colour gradient `1b4a:0c72` (§3.1). Wolf3D has a solid two-colour `VGAClearScreen`.
4. Clear `spotvis` (`DS:0x961D`, 64×64 bytes; `rep stosw` of 0x800 words). Wolf3D clears it in `ThreeDRefresh` too.
5. `WallRefresh` (`1b4a:11b1`): `viewangle`, `midangle = viewangle*10`, `viewsin/viewcos`, `viewx/viewy`, `focaltx/ty`, `viewtx/ty` exactly as Wolf3D.
6. `DS:0xC3D0` (masked-wall pass flag) = 0.
7. `1b4a:1273`: `xpartialdown = viewx.lo` (`DS:0xC2A8`), `xpartialup = -viewx.lo` (`DS:0xC2AA`), `ypartialdown = viewy.lo` (`DS:0xC2A4`), `ypartialup = -viewy.lo` (`DS:0xC2A6`), `lastside = -1` (`DS:0xC2C2`). Same values as Wolf3D.
8. **Δ** If `DS:0x099C` (set to 1 by `PlayLoop` on its first frame and after returning from a menu, cleared after every frame) is 0, set `drawwalls` (`DS:0xC282`) = 0.
9. **Δ** `AsmRefresh` — **pass A**. With `drawwalls = 0` every `Hit*` routine returns immediately, so this pass only marks `spotvis` and counts see-through tiles. (On a frame with `DS:0x099C = 1`, `drawwalls` keeps its previous value, which is 1 after any earlier frame, so pass A also draws; steps 10–11 then overdraw it, so the final image is the same.)
10. **Δ** `1b4a:1494`: textured ceiling and floor tiles for every `spotvis` tile and the player's tile (§3.2), plus the shootable-wall search used while fire is held (§9.1).
11. `drawwalls = 1`; `AsmRefresh` — **pass B** draws solid walls, doors and push walls (§4).
12. `ScalePost()` (flush the last post) and `DrawScaleds()` (§5).
13. **Δ** If pass B counted any see-through tile (`DS:0xC3CE > 0`): `lastside = -1`, `DS:0xC3D0 = 1`, `AsmRefresh` — **pass C** (masked walls, §4.4), `ScalePost()`, `DrawScaleds()` again (§5.1).
14. `DrawPlayerWeapon` (`1b4a:1013`, §6).
15. **Δ** Overlays, in this order: debug bars if `DS:0xC3CA` (`2f4d:0b2e`: masked-tile count, `tics`, frame rate as 1-pixel bars at screen rows 194/196/198); overhead map if `DS:0xC3CC == 1` (`2f4d:0d31`, §9.2); on-screen message if `DS:0x46A4 > 0` (`2f4d:124a`: counts down by `tics`; draws the string at `DS:0x46A6` at view-relative (6,6) then (5,5) with font colour `DS:0x9024 = 0`, or once at (5,5) with colour 0xDF when `DS:0x3274` is set; LIKELY); screen splats if `DS:0x0D42` (`30d6:10d5`, §6.2); debug text if `DS:0x0D46 & 1`, debug plot if `DS:0x0D46 & 0x40`; status-bar face/icon redraws (`30d6:0923` if `DS:0x330A`; countdown byte `DS:0x330E` calls `30d6:09ea(99)` when it reaches 1; `30d6:0999` if `DS:0x330F`).
16. If `fizzlein` (`DS:0x92C5`): `FizzleFade(bufferofs, displayofs+screenofs, viewwidth, viewheight, 20, false)`, clear it, zero `TimeCount` and `DS:0x95C7`. Wolf3D.
17. `bufferofs -= screenofs`; `displayofs = bufferofs`.
18. **Δ** Screen shake and page flip (§8.4), then `bufferofs += 0x4100`, wrap to 0 if above 0x8200 (Wolf3D), `frameon++` (`DS:0x95C3`, long), `PM_NextFrame` (`2a1d:1426`).

Order of drawing into the back buffer, therefore: gradient → ceiling/floor tiles → walls → sprites → masked walls → sprites again → weapon → overlays.

---

## 3. Floors and ceilings

OBC has two floor/ceiling modes chosen by `floormode` (`DS:0x33D2`, one byte; initialized data value 2; saved in `CONFIG.DAT` at offset 0x11A). In play, **F5** sets it to 0 and **F6** sets it to 2 (`CheckKeys` `1942:04ea`, cases at `1942:07a5`/`07b2`, CONFIRMED). The shipped `CONFIG.DAT` holds 2.

| `floormode` | Background (§3.1) | Per-tile textures (§3.2) |
|---|---|---|
| 0 ("ramp", F5) | ramp gradient `1b4a:0bc9` | no |
| ≠ 0 ("colour", F6, default) | colour gradient `1b4a:0c72` | yes, drawn over the gradient |

### 3.1 Background gradient

Both variants fill the view rectangle (already offset by `screenofs`) with `rep stosw` to all four planes (Map Mask 0x0F). One stored word covers 8 screen pixels: the **low byte** fills view columns 0–3, 8–11, … and the high byte fills 4–7, 12–15, … (four-pixel groups counted from the view's left edge). Each row stores `viewwidth/8` words; rows advance by 80 bytes. The ceiling is the first `viewheight/2` rows; the floor follows directly with another `viewheight/2` rows. `AH`/`AL` below are the high/low bytes; after every row they are exchanged (`XCHG AH,AL`), which makes a 4×1 checkerboard dither. All comparisons are **signed 8-bit** (`JLE`/`JGE`). CONFIRMED (`asm/0C069…`, `asm/0C112…`).

**Colour mode** (`1b4a:0c72`, `floormode ≠ 0`):

```
ceiling: AH = cc; AL = cc-1; n = 0                    ; cc = DS:0x09A2, clim = DS:0x9424
  repeat viewheight/2 rows:
    store row; swap(AH,AL)
    if (int8)AL > (int8)clim: n++; if n >= 10: n = 0; AH--; AL--
floor:   AH = fc; AL = fc-1; n = 0                    ; fc = DS:0x09A3, flim = DS:0x9423
  repeat viewheight/2 rows:
    store row; swap(AH,AL)
    if (int8)AL < (int8)flim: n++; if n >= 20: n = 0; AH++; AL++
```

**Ramp mode** (`1b4a:0bc9`, `floormode = 0`):

```
ceiling: AH = rc; AL = rc-1; n = 0                    ; rc = DS:0x9428, rlim = DS:0x9426
  repeat viewheight/2 rows:
    store row; n++; swap(AH,AL)
    if (int8)AH > (int8)rlim and n > 2: n = 0; AH--; AL--
floor:   AH = rf; AL = rf+1; n = 0; k = 0 (int8)      ; rf = DS:0x9427, fl = DS:0x9425
  repeat viewheight/2 rows:
    store row; n++; swap(AH,AL)
    if n > 2: k++; if k > 45 and (int8)AH < (int8)fl: n = 0; AH++; AL++
```

Note the floor of ramp mode starts with `AL = rf+1` (the others use −1), and stays flat for its first 48 rows.

**Per-level parameters** (set at the start of `PlayLoop`, `ghidra 1942:13cb`–`149d`, CONFIRMED). Index `m = mapon` (`DS:0x86FE`) if `mapon < 50` (signed), else `clamp(mapon − 10, 0, 50)`:

| Variable | Value |
|---|---|
| `DS:0x09A0` R (ceiling ramp) | `byte DS:0x09E6[m]` |
| `DS:0x09A1` F (floor ramp) | `byte DS:0x0A19[m]` |
| `DS:0x09A2` cc | `byte DS:0x0A4C[m]` |
| `DS:0x09A3` fc | `byte DS:0x0A7F[m]` |
| `DS:0x9424` clim | `(cc − cc%16) + 1` (unsigned) |
| `DS:0x9423` flim | `(fc − fc%16) + 15` |
| `DS:0x9428` rc / `DS:0x9426` rlim | `ramp[R].hi` / `ramp[R].lo` |
| `DS:0x9427` rf / `DS:0x9425` fl | `ramp[F].lo` / `ramp[F].hi` |

`ramp[i]` is a 4-byte record at `DS:0x09A4 + 4i` (file 0x35F24), 13 entries: `lo = 16i+17`, `pad 0`, `hi = 16i+30`, `pad 0` (i = 0 → 17/30 … i = 12 → 209/222), i.e. palette rows 1–13 without their first and last colour.

The four 51-byte tables (files 0x35F66, 0x35F99, 0x35FCC, 0x35FFF), index 0–50:

```
09E6 R : 2 2 6 4 7 6 4 4 4 4 4 4 4 4 4 6 4 4 4 6 4 4 6 8 8 8 8 6 8 8 4 8 8 8 8 6 4 8 7 6 8 8 8 8 8 4 4 8 8 8 8
0A19 F : 4 1 9 10 11 4 7 7 0 4 8 7 1 7 4 9 4 7 1 4 4 7 4 7 7 4 4 8 4 7 3 7 4 4 7 4 9 4 12 4 4 7 7 7 4 7 7 4 4 4 4
0A4C cc: 81 81 81 81 81 102 120 120 119 120 120 120 120 120 120 120 120 120 120 126 120 120 120 119 119 119 119 119 119 119 102 119 119 120 120 126 119 119 119 119 119 119 119 119 119 120 120 119 119 119 119
0A7F fc: 81 81 179 179 179 102 116 116 114 116 116 116 116 116 197 116 116 116 116 114 116 116 116 114 114 114 114 114 117 114 114 35 114 116 116 114 114 69 114 114 67 114 113 114 50 116 116 114 67 114 114
```

Debug keys (only when the debug flag `DS:0x0954` is set; CONFIRMED in `CheckKeys`): Q cycles R (0–12) and reloads rc/rlim, E cycles F and reloads rf/fl (messages `Rindex %d`, `Findex %d`); I/U raise/lower cc and K/J raise/lower fc within 0x10–0xE0 (`Rclr %d`, `Fclr %d`). These four do **not** recompute clim/flim.

### 3.2 Textured ceiling and floor tiles (colour mode only)

`1b4a:1494` (CONFIRMED) loops `x = 0..63` (outer), `y = 0..63` (inner) and, when `floormode ≠ 0` and (`spotvis[x][y]` or (x,y) is the player's tile), fills a tile record with `x`, `y` and the tile centre `(x<<16)+0x8000, (y<<16)+0x8000` and calls `1b4a:162b`. Tiles are drawn in that loop order with no depth sort; later tiles overwrite earlier ones, and walls (pass B) overwrite all of them.

**Projection of the ceiling quad** (`1b4a:162b`). `d` = `DS:0x09E2` (long) = **0x7600** on maps with `mapon > 4`, **0x8400** on maps 0–4 (the sewers) (`PlayLoop`). For corner `k = 0..3` with offsets `(+d,+d)`, `(+d,−d)`, `(−d,−d)`, `(−d,+d)`:

```
gx = cx + dx − viewx;  gy = cy + dy − viewy                    (32-bit)
nx = FixedByFrac(gx,viewcos) − FixedByFrac(gy,viewsin)
ny = FixedByFrac(gy,viewcos) + FixedByFrac(gx,viewsin)
if nx < mindist: nx = mindist (0x5800)          ; no ACTORSIZE term
sx = centerx + (long)(ny*scale)/nx              ; LXMUL, LDIV, low word
h  = heightnumerator / (int16)(nx>>8)           ; 32/16 idiv; if (nx>>8)==0 the corner keeps its previous screen point
sy = (viewheight/2) − (h>>3)                    ; signed /2, unsigned >>3: the top edge of a wall at that depth
corner[k] = (sx, sy)  →  DS:0xC006 + 4k (x), DS:0xC008 + 4k (y)
```

Then (still in `162b`): `shade = shadetab[clamp((h₃>>2) − 5, 0, 255)]` → `DS:0xC004`, where `h₃` is corner 3's height (§4.5 for `shadetab`). The floor code → `DS:0xC005`:

- if `tilemap[x][y]` is 0x19 or 0x1D–0x20 → 3; 0x31–0x34 → 0; 0x39–0x3C → 7; 0x41–0x48 or 0x75–0x78 → 0 (see-through wall tiles whose plane 0 holds a wall number);
- otherwise `code = (byte)plane0[y][x] − 0x90` (floor codes 144–153 → 0–9);
- if `code > 9`: `code = 1b4a:19ef(x,y)`, which tries the eight neighbours in the order (0,+1), (+1,0), (0,+1) again, (−1,0), (−1,−1), (+1,−1), (+1,+1), (−1,+1) (table `DS:0x0D5C`, coordinates wrap mod 64), applies the same tile-value rules (reading plane 0 as a **word** minus 0x90), returns the first result in 0–9, and returns 9 if none.

**Rasterising** (`30d6:1814`): textures from the table at `DS:0x33BE` (file 0x3893E), two bytes per code, each a page number + 1:

| code | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| floor page (`33BE[2c]−1`) | 80 | 82 | 84 | 86 | 104 | 106 | 108 | 110 | 91 | 110 |
| ceiling page (`33BF[2c]−1`) | 81 | 83 | 81 | 81 | 81 | 81 | 81 | 110 | 91 | 111 |

`ceilbase = (ceilpage[0] >> 4) << 4` and `floorbase` likewise: the palette row of each texture's **first texel** (`DS:0xE7B9`, `DS:0xE7B8`). A code above 9 here is a fatal error (`Quit 0x70E`).

```
maxx = max corner x, clamped to <= viewwidth;  minx = min corner x, clamped to >= 0
for col = minx .. maxx-1:
    ymin = 200; ymax = -1
    for each edge (prev, cur) = (3,0),(0,1),(1,2),(2,3):
        (x1,y1)=corner[prev]; (x2,y2)=corner[cur]
        skip if (x1<col and x2<col) or (x1>col and x2>col) or x1==x2
        slope = ((y2−y1)<<7) / (x2−x1)                  ; int16, truncating
        y = ((y1<<7) + (col−x1)*slope) >> 7             ; int16 wraparound, arithmetic shift
        ymin = min(ymin,y); ymax = max(ymax,y)
    if ymax >= viewheight/2: ymax = viewheight/2
    if ymin < 0: ymin = 0
    if ymin <= ymax: span(col, ymin, ymax−ymin, (col−minx) % 64)
```

`span(x, y, count, u)` (`30d6:1a21`) sets the Map Mask to `1 << (x & 3)` and draws `count` pixels down from ceiling row `y` and, mirrored, up from floor row `viewheight−1−y`:

```
v = 0
for i = count-1 down to 0:
    if d < 0x8000 and i == 0: t = 0                    ; non-sewer maps: last row uses texel 0
    else                     t = v*64 + u               ; page byte index (pages are column-major)
    ceiling pixel = max(ceilpage[t]  − shade, ceilbase)   ; signed 16-bit compare
    floor pixel   = max(floorpage[t] − shade, floorbase)
    ceiling row += 1; floor row −= 1
    v++; if v == 63: v = 0                              ; wraps at 63, so byte offsets 4032-4095 are never used
```

Unlike walls, this shading ignores the full-bright mask (§4.2): every texel is darkened and clamped to its texture's row base, never to black. The textures are laid in **screen space**, one texel per pixel, restarting at the top of each column span and at `minx` for each quad; there is no perspective mapping. Quads with `d = 0x7600` are smaller than a tile, so the gradient shows through between them (a grid of grout lines); sewer quads (`0x8400`) overlap. All CONFIRMED.

---

## 4. Walls, doors and push walls

### 4.1 Wall pages

- **Δ** Solid walls: `page = tilehit − 1` for **both** vertical and horizontal faces (`HitVertWall` `1b4a:05c9`, `HitHorizWall` `1b4a:0726`). There is no light/dark side pairing (Wolf3D uses `(tile−1)*2+1` / `(tile−1)*2`). CONFIRMED.
- **Δ** Door frames: if `plane1[ytile][xtile] == 125` (0x7D) at the hit wall, and the tile in front of it along the ray (`[xtile−xtilestep][ytile]` for vertical faces, `[xtile][ytile−ytilestep]` for horizontal) is a door (`tilemap & 0x80`) or a wall of type 13–16, 29–32, 49–52, 65–68 or 69–72 (`2f4d:0adf`), the page is `PMSpriteStart − 6` = **130** on both orientations. Wolf3D tests `tilehit & 0x40` and uses DOORWALL+3/+2. CONFIRMED.
- `PMSpriteStart` is the page-file header value read into `DS:0xCA64` (136 in OBC-A; `PML_OpenPageFile` `2a1d:05f8`).
- **Δ** Doors (`HitHorizDoor`, `HitVertDoor`): `page = PMSpriteStart − 8 + k` with `k` from `doorobjlist[door].lock` (byte at `DS:0x7BAE + 10·door`; door records are 10 bytes): lock 0 → +0 (**128**), 1 and 3 → +4 (**132**), 2 and 4 → +5 (**133**), 5 → +7 (**135**); the same for both orientations (jump tables at `1b4a:0895` and `1b4a:09e7`). A lock value above 5 leaves the page register uninitialised. Wolf3D: +0/+1, +6/+7, +4/+5. CONFIRMED.
- Push walls: `page = (tilehit & 0x7F) − 1`. CONFIRMED.
- Animated walls have no renderer support: the game changes `tilemap` values (for example the destructible-wall sequence in `PlayLoop`, which increments the tile every other frame within 57–60 and 117–120), and the page follows `tile − 1`.

### 4.2 Pixel colour: distance shading and transparency

Every wall and sprite post goes through a compiled scaler (`BuildCompScale`, §7.2). For each source texel `t` the emitted code is:

```
mov al,[si+n]; cmp al,0FFh; je skip; call shade; mov es:[di+row*80],al ... ; skip:
shade:  mov bx,ax; and cl,ss:[bx+2FDCh]; sub al,cl; mov cl,ch
        mov bl,ss:[bx+30DCh]; sub bx,ax; and al,bh; ret
```

With `AH = 0` and the shade in both `CL` and `CH`, this computes (CONFIRMED, `1d8b:0495`–`0554`):

```
if t == 0xFF: pixel not written (transparent)
s = t − (shade & mask[t])
out = (s > base[t]) ? s : 0
```

`mask` (`DS:0x2FDC`, file 0x38B5C) is 0xFF for indices 16–223 and 0x00 for 0–15 and 224–255; `base` (`DS:0x30DC`, file 0x38C5C) is `t & 0xF0` for 16–223 and 0 elsewhere. Both are static (no writer). So:

- palette rows 1–13 (16–223) are 16-colour ramps, dark → bright; shading walks down the ramp and turns **black (index 0)** when it reaches the row's first colour;
- indices 0–15 and 224–255 are **full-bright** (never shaded);
- **K (quirk)**: a texel equal to its row base (16, 32, …, 208) always becomes 0, even with shade 0. This also affects the weapon and splats, which use shade 0.
- index 255 is transparent in walls as well as sprites. Wall pages containing 0xFF texels in OBC-A: 3, 8, 12–15, 17, 28–32, 46, 48–51, 56–59, 65–67, 69–71, 79, 85, 87, 91 (one texel), 101 (one texel), 112, 116–119, 126, 132, 134 (counted from `GFXTILES.BC`). The background or anything drawn earlier shows through.

### 4.3 Ray casting, passes A and B (`AsmRefresh` `1d2a:007a`)

Setup per column is Wolf3D's (`finetangent`, quadrant patching of the `jle`/`jge` at `cs:02A6`, `cs:0322`, and the pass-C copies at `cs:0560`, `cs:05C4`). Changes in the core loop (CONFIRMED):

- **Δ** Before testing `tilemap`, the loop reads `plane1[yinttile][xtile]` (vertical check) or `plane1[ytile][xinttile]` (horizontal check) from `mapsegs[1]` (`DS:0x9411`), low byte only. Values **0x67–0x6B (103–107)** are see-through markers: the tile is treated as empty (`spotvis` set, trace continues) and `DS:0xC3CE` is incremented, whatever `tilemap` holds.
- Door-type tiles (`tilemap & 0x80`): **Δ** instead of Wolf3D's `& 0x40` test, the tile is a moving push wall when `plane1` at the tile is **0x62, 0x65 or 0x66 (98, 101, 102)**; then the Wolf3D push-wall offset (`step * pwallpos / 64`, `pwallpos` = `DS:0x4998`) is used and `HitVertPWall`/`HitHorizPWall` is called. Otherwise it is a door: half-step to the door plane, `doorposition` (`DS:0x4918`) test, `HitVertDoor`/`HitHorizDoor`. Same arithmetic as Wolf3D.
- The `Hit*` routines are Wolf3D's apart from the pages (§4.1), the `drawwalls` early return (§2 step 9) and the door texture for non-door tiles (§4.4). `CalcHeight` is unchanged (`MINDIST` 0x5800 at `DS:0x0D1E`).

### 4.4 Masked walls, pass C

Pass C (`DS:0xC3D0 = 1`) re-traces every column through the same tiles (CONFIRMED, `1d2a:055e`–`0612`):

- A tile whose `plane1` low byte is **0x68 or 0x69** is drawn as a normal wall on its boundary (`HitVertWall`/`HitHorizWall`, `tilehit = tilemap` value).
- **0x67, 0x6A, 0x6B** are drawn on the tile's centre plane through the door path. If the `tilemap` value has bit 7 the door-position test applies; otherwise there is none, `HitVertDoor`/`HitHorizDoor` use `doornum = 99` ("not a door"), `texture = (intercept >> 4) & 0xFC0` and `page = tilehit − 1`.
- Any other tile: continue if `spotvis` is set (the ray passed it in pass B), otherwise stop the column without drawing.
- After drawing, the column ends. So only the nearest see-through tile per column is drawn, over the walls and sprites of pass B, with 0xFF texels transparent.
- **K (quirk)**: if such a centre-plane tile is also an open door, the "continue" path jumps back into the pass-A/B loop (`1d2a:02d7`/`0353`), which then draws the next solid wall over the sprites. LIKELY rare in the maps; to be checked.

### 4.5 Distance-shade table (`DS:0xC2CA`, 256 bytes)

Built by `30d6:11b6` (CONFIRMED), from `BuildTables` at start-up (via `2f4d:02b7`), at every level start in `PlayLoop`, and whenever the darkness flag changes (tremor frames, debug key D):

```
g = (DS:0x33B8 != 0) ? 5.5 : 2.5                     ; floats at DS:0x343F / DS:0x3443
table[0] = 16
for i = 1..255:  s = (float)(16.0 / sqrt(i / g) − 1.0)    ; x87, stored as float32
                 if s > 16.0: s = 16.0
                 table[i] = trunc(s)                        ; FTOL@ uses chop rounding
```

(The RTL routine at `1000:0f5a` that this calls is `sqrt`, not `tan`; it errors with 0x70C/0x70D on a zero argument or result.) The resulting tables, as "first index: value":

```
g=2.5 (normal): 0:16 3:13 4:11 5:10 6:9 7:8 8:7 11:6 14:5 18:4 26:3 41:2 72:1 161:0
g=5.5 (dark):   0:16 5:15 6:14 7:13 8:12 9:11 10:10 12:9 15:8 18:7 23:6 29:5 40:4 57:3 89:2 157:1
```

The only exact-integer cases (i/g = 4, 16, 64) are exact in any precision, so the table is reproducible with IEEE doubles plus float32 rounding.

**Lookup index** (CONFIRMED):

| User | Index into the table |
|---|---|
| Wall post (`ScalePost` `1b4a:040b`) | `min(255, wallheight[postx]/2 + 2*DS:0xC3D2 + DS:0x48E2)` — one shade for the whole post |
| Sprite (`ScaleShape` `1d8b:0727`) | `min(255, height/2 + 2*DS:0xC3D2 + DS:0x48E2)` |
| Ceiling/floor tile | `clamp(h₃/4 − 5, 0, 255)` (§3.2) |
| Weapon, splats (`SimpleScaleShape`) | none: shade 0 |

`wallheight` and sprite heights are in eighth-pixel units (`height>>3` is the scaler index; a scaler index `i` draws `2i` pixels), so `height/2` ≈ 2 × the on-screen half-height in pixels. The comparison with 255 is signed; a negative index is not clamped.

- `DS:0xC3D2` **muzzle flash**: bit 2 (value 4, adding 8 to the index) is set by `T_Attack` when a shot is fired (`208c:1a97`, `1abc`, `1b10`) and cleared at the end of `T_Attack` when `gamestate.attackframe` (`DS:0x871C`) becomes odd (`208c:1b51`). The code is in a range Ghidra did not turn into a function (after `208c:1883`). CONFIRMED for the writes; LIKELY for the frame timing.
- `DS:0x48E2` **brightness bias**: read from `CONFIG.DAT` offset 0x10C (word) and written back; no in-game writer was found, so presumably set by `SETUP.EXE`. The shipped `CONFIG.DAT` holds 0. LIKELY.
- `DS:0x33B8` **darkness**: set to `mapon <= 4` at level start (sewers dark), inverted with `NOT` every frame of a tremor and forced to 0 when the tremor ends (so the sewers become "lit" after their first tremor; K), toggled by debug key D. CONFIRMED.

---

## 5. Sprites

### 5.1 DrawScaleds (`1b4a:0d98`)

CONFIRMED except where noted. The visible list is built only when `DS:0xC3D0 == 0` (first call of the frame):

- `vislist` at `DS:0xC02A`, 8-byte entries `{viewx, viewheight, shapenum, drawcount}`, **75 entries** (`visptr < DS:0xC27A`; Wolf3D 50).
- Statics: `statobjlist` at `DS:0x7E2C` with **10-byte** records (`tilex`, `tiley`, `visspot` near pointer, `shapenum` at +4, `flags` at +6), end pointer `laststatobj` at `DS:0x7E2A`. Skip if `shapenum == −1` or `*visspot == 0`; `TransformTile`; if it returned true and `flags & FL_BONUS (2)` call `GetBonus` (`208c:074f`) and skip; skip if `viewheight == 0`. Wolf3D logic.
- Actors from `player->next` (+0x3E): skip if `state->shapenum` (state+2) is 0; the 9-tile `spotvis`/`tilemap` neighbourhood test is Wolf3D's; if not visible clear `FL_VISABLE` (8); else `active = 1`, `TransformActor`, skip if `viewheight == 0`, use `temp1` (+0x32) when `shapenum == −1`, add `CalcRotate` (`1b4a:0d1b`) if `state->rotate`, set `FL_VISABLE`.
- `CalcRotate` is Wolf3D's (`angle = player->angle + (centerx − viewx)/8 − 180 − dirangle[dir]` or `− ob->angle` for obclass 0x13, `+22`, wrap 0–359, `rotate_2` → `4*(angle/180)`, else `angle/45`). LIKELY.

Drawing (both calls): repeat `numvisable` times: pick the entry with the smallest `viewheight` among those with `drawcount == 0` (first call) or `drawcount ∈ {0,1}` (second call), `ScaleShape(viewx, shapenum, viewheight)` unless `shapenum == −1`, then `drawcount++`. The second call (after pass C) therefore redraws **all** sprites, far to near; `ScaleShape` clips each column against the current `wallheight[]`, which pass C has lowered to the masked wall's height in the columns where it drew one. Net effect: sprites behind a masked wall stay behind it, sprites in front are drawn over it.

### 5.2 ScaleShape (`1d8b:070a`) and ScaleLine (`1d8b:0593`)

Identical to Wolf3D (`scale = height>>3`, reject `0` or `> maxscale` (`DS:0xCB83`), left and right halves, `wallheight[x] >= height` occlusion, `ScaleLine` patches a `RETF` into the scaler) except that **Δ** the shade from §4.5 is stored in `DS:0xCB7F` and `ScaleLine` loads `CX = [CB7F]; CH = CL; AX = 0` before each scaler call. Sprite posts use the same shading and 0xFF transparency as walls (§4.2). CONFIRMED.

---

## 6. Player weapon and screen splats

### 6.1 DrawPlayerWeapon (`1b4a:1013`)

**Δ** The weapon is two sprites drawn side by side (CONFIRMED):

```
if weapon (DS:0x8716) == -1: return
idle  = (DS:0x95F9 == 0 and attackframe (DS:0x871C) == 0)     ; DS:0x95F9 = attack in progress
b     = DS:0x95BD & 15                                       ; walk counter, ++ per frame while moving, 0 when still
frame = idle ? bobframe[b] : weaponframe (DS:0x8720)
x     = viewwidth/2 + bobx[b]
size  = DS:0xC01A[weapon] = viewheight/4                     ; set by SetupScaling for all 5 weapons
bufferofs += off1;            SimpleScaleShape(x, left[weapon]  + frame, size)   ; with the x1.1 flag
bufferofs += off2;            SimpleScaleShape(x, right[weapon] + frame, size)
bufferofs -= off1 + off2
```

Tables (initialized DGROUP): `left[] = DS:0x0D2A` = 544, 552, 560, 568, 576; `right[] = DS:0x0D34` = 584, 592, 600, 608, 616 (sprite numbers, page = `PMSpriteStart` + n); `bobx[] = DS:0x31DC` = 0 1 2 3 4 3 2 1 0 −1 −2 −3 −4 −3 −2 −1; `bobframe[] = DS:0x31FC` = 7 7 6 5 4 5 6 7 7 7 6 5 4 5 6 7.

With the flag `DS:0x0D3E = 1`, `SimpleScaleShape` uses `scale = trunc(size × 1.1)` (double at `DS:0x0E60`; otherwise `size>>1`), so each part is `2·scale` pixels per 64 texels, vertically centred in the view by the scaler (`toppix = (viewheight − 2·scale)/2`) and then moved by `off1`. `off1` (`DS:0xC018`) and `off2` (`DS:0xC016`) are byte offsets in planar memory (80 per row, 1 per 4 pixels), set by `208c:0009` from `viewheight` (CONFIRMED):

| viewheight | 88 | 96 | 104 | 112 | 120 | 128 | 136 | 144 | 152 | 160 | 200 | other |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `off1` | 0x6E0 | 0x6D6 | 0x820 | 0x816 | 0x960 | 0x906 | 0xA8C | 0xA46 | 0xE0C | 0xB36 | 0xF96 | 0 |
| `off2` | 10 | 12 | 12 | 14 | 14 | 16 | 12 | 18 | 14 | 21 | 24 | 0 |

For example at view size 16 (256×128): scale 35 (70 px), `off1 = 29·80 − 10` (29 rows down, 40 px left), `off2 = 16` (second part 64 px to the right of the first). Several entries put the bottom of the weapon a few rows below the view; reproduce as is. `SimpleScaleShape` does no clipping. Weapon pixels are unshaded but subject to the row-base quirk (§4.2).

### 6.2 Splats (`30d6:10d5`)

When `DS:0x0D42` is set, up to `DS:0x33B2` (≤ 19) entries of 8 bytes at `DS:0x3312` `{shape, x, size, type}` are drawn with `SimpleScaleShape` (no ×1.1): type 2 → sprite `shape + 640`, `size >> 1`; type 1 → sprite `shape + 648`, `size`. When the count reaches 0 the flag is cleared. CONFIRMED for the drawing; the spawner is not analysed.

---

## 7. Projection and scaling

### 7.1 Constants and tables

| Item | Value | Where | Confidence |
|---|---|---|---|
| `FOCALLENGTH` | 0x5700 | pushed by `SetViewSize` `1497:10f6` | CONFIRMED |
| `MINDIST` | 0x5800 | `DS:0x0D1E` (long), also hard-coded in `CalcProjection` | CONFIRMED |
| `facedist` | `focallength + MINDIST` = 0xAF00 | `CalcProjection` | CONFIRMED |
| `centerx` | `viewwidth/2 − 1` | `DS:0x8A20` | CONFIRMED |
| `shootdelta` | `viewwidth/10` | `DS:0x8A1E` | CONFIRMED |
| `scale` | `trunc(halfview × facedist / 32768.0)` | `DS:0x8796` (long) | CONFIRMED |
| `heightnumerator` | `scale.lo << 10` | `DS:0x878E` (long) | CONFIRMED |
| `minheightdiv` | `heightnumerator/0x7FFF + 1` | `DS:0x878C` | CONFIRMED |
| `pixelangle[]` | `a = (float32)atan(((long)i·65536/viewwidth) / facedist)`; `trunc(a × radtoint)`; stored at `[halfview−1−i]` and negated at `[halfview+i]` | `DS:0x879A` | CONFIRMED |
| `maxslope` | `finetangent[pixelangle[0]] >> 8` | `DS:0x8792` | CONFIRMED |
| `finetangent[900]` | `trunc(tan((i+0.5)/radtoint) × 65536)` for i < 450, and `trunc(1/tan × 65536)` at `899−i` | far, image seg 0x2BD3 (file 0x2F130) | CONFIRMED |
| `sintable[451]` | `trunc(sin(a) × 65536)` with `a` accumulated in float32 by π/180 (0x3C8EFA35), i = 0..90 written to `[i]`, `[i+360]`, `[180−i]`, and with bit 31 set (sign-magnitude) to `[360−i]`, `[180+i]` | far, image seg 0x2B62 (file 0x2EA20); `costable` = +90 entries | CONFIRMED |
| `radtoint` | 572.9578 (float32 at `DS:0x0102`) | | CONFIRMED |

Caveat: the RTL `sin` (`1000:0f99`) and `tan` (`1000:0fe6`) use the real `fsin`/`fsincos` instructions when a 387 is detected (`DS:0x3B40 >= 3`) and the emulator otherwise, so an occasional last-bit difference after `× 65536` and truncation is possible between machines. For golden data, dump `finetangent`, `sintable`, `pixelangle` and the shade table from DOSBox-X memory rather than trusting a recomputation (Q-R1).

All of these are Wolf3D's formulas and constants, **built with floating point** (`BuildTables` `1497:0a7b`, `CalcProjection` `1497:0bf9`, Borland emulator; `FTOL@` truncates). The one new floating-point table is the shade table (§4.5); `BuildCompScale` also uses floating point for `toppix` (division by 2.0, same result as Wolf3D's integer division).

### 7.2 Compiled scalers

`SetupScaling(trunc(viewwidth × 1.5))` (`1d8b:001f`; factor at `DS:0x0342`): `maxscaleheight = n/2`, `maxscale = maxscaleheight − 1` (`DS:0xCB83`), `maxscaleshl2` (`DS:0xC3D4`), `stepbytwo = viewheight/2` (`DS:0xCB7B`); a scaler is built for every `i = 1..maxscaleheight` (height `2i`), stepping by 3 once `i >= stepbytwo` with `i+1`, `i+2` aliased to `i`; `scaledirectory` at `DS:0xCB85`, `fullscalefarcall` at `DS:0xC3D6` (entries past `maxscaleheight` point at `BadScale` `1d8b:000f`). Work buffer 10,000 bytes (Wolf3D 20,000). **Δ** It also sets the five weapon sizes `DS:0xC01A..0xC022 = viewheight/4`. LIKELY (decompiler).

`BuildCompScale(height)` (`1d8b:0230`) keeps Wolf3D's geometry exactly (`step = (height<<16)/64`, `toppix = (viewheight − height)/2`, texel `src` covers rows `[startpix, endpix)` clipped to the view, no code for `src = 64`); only the emitted instructions differ (§4.2). CONFIRMED.

`ScalePost` (`1b4a:03f3`) uses Wolf3D's `mapmasks1/2/3` (`DS:0x0D7B/0D9B/0DBB`) and `postx/postwidth/postsource` (`DS:0xC7DC/C7DA/C7DE`). CONFIRMED.

---

## 8. Palette effects and screen shake

### 8.1 Damage and bonus flashes

`UpdatePaletteShifts` (`1942:0fc9`) and `InitRedShifts` (`1942:0db5`) follow Wolf3D with different constants (LIKELY, decompiler read; values cross-checked against the code):

| | Wolf3D | OBC |
|---|---|---|
| red shifts | 6, `REDSTEPS` 8 | **4**, steps 8: `r += (64−r)·i/8`, `g −= g·i/8`, `b −= b·i/8` (C truncating division) |
| red index | `damagecount/10 + 1`, max 6 | `damagecount/10 + 1`, max **4** |
| white shifts | 3, `WHITESTEPS` 20 | 3, steps **10**: `r += (64−r)·i/10`, `g += (62−g)·i/10`, `b −= b·i/10` |
| white index | `bonuscount/6 + 1`, max 3 | same |
| tables | BSS | far, image seg 0x2D78 (red, file 0x30B80) and 0x2CB8 (white, file 0x2FF80), built at run time from the current `gamepal` |

`damagecount` is `DS:0x941F`, `bonuscount` `DS:0x941D`, `palshifted` `DS:0x941B`. **Δ** `VL_SetPalette` (`2ddf:0209`) takes a second argument; when it is non-zero (all shifted palettes) it first copies the first 48 bytes of `gamepal` over the shifted palette, so **indices 0–15 are never tinted**. CONFIRMED.

### 8.2 Normal, dark and picture palettes

`2f4d:035e(n)` (CONFIRMED) copies a source palette into `gamepal` (image seg 0x2A9B, file 0x2DDB0), keeps a copy at image seg 0x30A5, uploads it (unless `DS:0xBFF4` is set), rebuilds the red/white shifts and stores `n` in `DS:0x8F0C`:

| n | Source | Note |
|---|---|---|
| 0, 1 (and > 3) | `DS:0xE295`, a copy of the original `gamepal` taken at start-up (`2f4d:0405`) | normal |
| 2 | image seg 0x2ACB (file 0x2E0B0) | **dark**: indices 16–242 and 252–254 differ (about 40 % brightness); 0–15, 243–251 and 255 unchanged |
| 3 | image seg 0x2AFB (file 0x2E3B0) | full-screen picture palette |

`PlayLoop` selects palette 1, then 2 if `30d6:1b3d` finds a tile of value **96** (`'`'`) in `tilemap` (95 `'_'` means normal; finding more than one marker is a fatal error). When a tremor ends there is a 1-in-30 chance (`US_RndT() % 30 == 0`) of switching to the dark palette. LIKELY for the marker semantics.

### 8.3 Palette cycling

A rotation routine exists (`2f4d:0332`: rotate entries 249–254 of the palette copy and upload it), but no caller was found (no far or near call, no stored pointer). Treat it as unused. LIKELY. The RTC interrupt installed by `2f4d:0071` (INT 70h) only counts frames; the names `VL_SetColor_2f4d_0024` and friends are wrong (they write CMOS registers).

### 8.4 Tremors and screen shake

- A tremor is started by `30d6:12b1` (explosion handling; LIKELY): `DS:0x0D40 = 8` (shake counter, byte) and `DS:0x09D8 = 10` (tremor frames).
- Each `PlayLoop` frame while `DS:0x09D8 > 0`: decrement it; if the shake counter is 0 set it to 4; `DS:0x33B8 = NOT DS:0x33B8` (darkness flicker); when it reaches 0, maybe switch to the dark palette (§8.2) and set `DS:0x33B8 = 0`; rebuild the shade table. CONFIRMED (`1942:1681`–`16c7`).
- In `ThreeDRefresh`, the counter is zeroed unless tremors are enabled (`DS:0x0D44`, default 1, toggled by F8 with the messages `TREMORS ON`/`TREMORS OFF`, saved in `CONFIG.DAT` offset 0x11B), `DS:0x870C` (health, LIKELY) ≥ 20, `DS:0xBFF4 == 0`, and `difficulty` (`DS:0x86FC`) ≤ 3.
- Page flip with shake (`1b4a:1420`): if the counter `c` is non-zero, CRTC Start Address High = `hi(displayofs) + T1[c]`, Low = `T2[c]`, then `c--`; otherwise High = `hi(displayofs)`, Low = 0. `T1 = DS:0x0D47` (all zeros for c = 1–9), `T2 = DS:0x0D51`: `T2[c] = 0xA0` for odd c and 0 for even c (file 0x362D1). So on alternate frames the **whole screen, status bar included, scrolls up by 2 scan lines** (160 bytes), and the bottom 2 lines show memory past the 200-line page. CONFIRMED.
- Esc is ignored while the screen is shaking (`CheckKeys`).

---

## 9. View sizes and layout

- `viewsize` (`DS:0x499A`) comes only from `CONFIG.DAT` offset 0x108 (default 18 without a config; the shipped file has 16). `InitGame` lowers it by free conventional memory (`mminfo.mainmem`, `DS:0x8E1E`): below 305,000 bytes → at most 14, below 325,000 → 16, below 345,000 → 18 (CONFIRMED). `NewViewSize` (`1497:119b`) quits with error 0x902 above 20. **Δ** No other writer exists: OBC has no in-game view-size control. CONFIRMED.
- `SetViewSize(w = viewsize·16, h = trunc(w × 0.5))`: `viewwidth = w & ~15`, `viewheight = h & ~1`, `screenofs = ((160 − viewheight)/2)·80 + (320 − viewwidth)/8` (Wolf3D). A height above 160 would select a 320×200 view at offset 0, but `NewViewSize` cannot produce one. So view sizes are `16n × 8n` for n ≤ 20, centred in the top 160 lines: 14 → 224×112 at (48,24), 16 → 256×128 at (32,16), 18 → 288×144 at (16,8), 20 → 320×160 at (0,0). CONFIRMED.
- The status bar is a 320×40 picture drawn at y = 160 into all three pages (`30d6:01ec` → `StatusDrawPic(0, 160, …)`), after the screen is tiled with 8×8 border pic 3. Pages are at 0, 0x4100, 0x8200 (`DS:0x0D12`). LIKELY (decompiler).
- **Δ** `SetViewSize` also sets overlay anchors: `DS:0x467A = (320 − vw)/2 + 4`, `DS:0x4678 = (200 − vh)/2 + 4` (overhead map position), `DS:0x4668 = vw − 40`, `DS:0x466A = 5`, `DS:0x4666 = 2`, `DS:0x4664 = vh − 10`, `DS:0x4672 = 25`, `DS:0x4670 = vw − 27`, `DS:0x466E = 0`, `DS:0x466C = vh − 12`, `DS:0x4676 = vw/2`, `DS:0x4674 = vh/2`. Users not analysed.
- `1b4a:1494` also runs the shootable-wall search (§9.1) and `SetViewSize`'s `DS:0x467A`/`0x4678` place the overhead map (§9.2). Pass A exists so that this search and the ceiling tiles see this frame's `spotvis`.

### 9.1 Shootable-wall search (`1b4a:1494`)

Gameplay code inside the renderer: while fire is held, every frame picks at most one damageable wall near the line of sight and starts its break animation. CONFIRMED by instruction read of `1b4a:1494`–`1629`, `1b4a:1b36`–`1cc3` (with the jump table at `1b4a:1cc5`), `1b4a:1ccf`–`1e0d` and `208c:1c6a`–`1cfb`. OBCR runs it in the simulation's visibility pass with the Classic view geometry (`obc-sim` `vis.rs`, `World::shootable_wall_search`), between ray pass A and the `DrawScaleds` bookkeeping, as here.

**When.** Every `ThreeDRefresh`, i.e. every PlayLoop frame, after pass A (so it sees this frame's `spotvis`) and before pass B and `DrawScaleds` (so before `GetBonus`). The condition is tested per tile, but nothing in the loop changes it: `buttonstate[bt_attack]` (`DS:0x95D9`) ≠ 0 and `gamestate.weapon` (`DS:0x8716`) < 3, signed (pistol, Uzi, Galil; also −1 while dying: the death swing of `Died` redraws the view with `weapon = −1` and the last frame's `buttonstate`, so a player killed with fire held can still chip a wall; LIKELY, not reproduced by OBCR's `World::death_turn`). It is **not tied to shots**: holding fire triggers it every frame; `T_Attack` clears `buttonstate[0]` only on the frame an attack cycle ends.

**Loop.** `x = 0..63` outer, `y = 0..63` inner (the floor-tile loop). For each tile:

1. *IsShootableTile*(x, y) (`1b4a:1b8b`, result in `DS:0x0D22`):
   - **chip walls**: `tilemap[x][y]` ∈ {0x13, 0x14, 0x16, 0x17, 0x35, 0x36, 0x79, 0x7A, 0x7C, 0x7D} (0x15, 0x7B are excluded) and **one of the four neighbours is in `spotvis`** (byte == 1). The neighbours are read at byte offsets −64, +64, −1, +1 from `spotvis + x·64 + y`: for `y = 0` and `y = 63` the north/south reads land on `[x−1][63]` and `[x+1][0]`; for `x = 0` and `x = 63` they land outside `spotvis` (`DS:0x95DD + y`: `buttonstate[2..13]`, `controlx/y`, `buttonheld[]`, `noclip`, `godmode`; and `DS:0xA61D + y`, unknown BSS). No shipped map has a chip or glass tile in column 0 or 63 (scan of all 61 `MAPTEMP` slots: such tiles occur only on rows 0 and 63, in maps 7, 10, 11, 13–18, 20, 21, 24, 26, 30, 32, 58), so OBCR reads out-of-range neighbours as not visible without visible difference.
   - **glass**: `tilemap` ∈ 0x39–0x3B or 0x75–0x77 (intact and cracked panes; the broken 0x3C/0x78 are excluded) and **its own `spotvis` is set** (pane cells carry see-through plane-1 markers, so pass A marks them).
2. Projection (`1b4a:1ccf`, a `TransformActor` variant applied to a stack `objtype` whose x, y are the tile centre `(t << 16) + 0x8000`):
   ```text
   gx = cx − viewx;  gy = cy − viewy
   nx = FixedByFrac(gx, viewcos) − FixedByFrac(gy, viewsin)      ; no ACTORSIZE
   ny = FixedByFrac(gy, viewcos) + FixedByFrac(gx, viewsin)
   if nx < MINDIST (0x5800): nx = MINDIST                          ; clamped, not rejected
   viewx = centerx + low16(LDIV@(LXMUL@(ny, scale), nx))          ; also stored in DS:0xC006
   ```
   (`nx == 0` would `Quit(0x6AC)`; impossible after the clamp.) Because of the clamp, a tile beside or behind the player with a small `ny` also projects near the centre.
3. Aim (`1b4a:1529`): keep the tile only if `|DS:0xC006 − centerx| < shootdelta` (`DS:0x8A1E` = `viewwidth/10`), computed in 16 bits (`CWD`/`XOR`/`SUB`).
4. Score (`1b4a:1b36`): `s = (|x − player.tilex| · |y − player.tiley|) / 2` (16-bit `IMUL`, truncating). If `s < best` (`DS:0x0D24`): `best = s`, target = (x, y) in `DS:0xC2C8`/`DS:0xC2C6`, found flag `DS:0xC2C4 = 1`. Strict `<`: on ties the first tile in loop order wins.

After the loop, if the flag is set: `StartWallHitAt(x, y)` (`208c:1c6a`), flag = 0, `best = 90`.

`best` is initialised data (`DS:0x0D24` = 90) and only drops when a target is found, which always ends in the reset, so **every frame starts at 90**: only tiles with `|dx|·|dy| < 180` qualify. The score is 0 for every tile in the player's row or column, so in practice the choice is "the first tile in x-major order that is in line with the player, else the most nearly in line".

**Effect** (PLAYER.md §3.9). `StartWallHitAt` sets the wall-break state (`DS:0x09DA` = 1, tile `DS:0x9419`/`0x9417`) and a countdown by tile (6: 0x39, 0x75; 4: 0x3A, 0x76; 2: the chip tiles, 0x3B, 0x77; 1: 0x3C, 0x78; unchanged otherwise). PlayLoop's wall-break step then adds 1 to the tile when the countdown is even and decrements it. Since the search restarts the countdown on every frame fire is held, a chip wall advances one stage per frame until it reaches a non-candidate: 0x13 → 0x14 → **0x15**, 0x16 → 0x17 → **0x18**, 0x35 → 0x36 → **0x37**, 0x79 → 0x7A → **0x7B**, 0x7C → 0x7D → **0x7E**; a pane 0x39 becomes 0x3A, 0x3B and **0x3C** on three consecutive frames (sounds at countdown 5 and 1), 0x75 → 0x78 likewise. Follows from the code (search CONFIRMED, the PlayLoop wall-break step LIKELY); not observed in DOSBox-X.

### 9.2 Overhead map and ThermoSensor (`2f4d:0d31`)

CONFIRMED by instruction read of `2f4d:0d17`–`0f7b`, `1618:1480` (`DrawOutline`), `2e72:0607`/`063b` (`VWB_Hlin`/`VWB_Vlin`), `2ddf:04f9` (`VL_Plot`), `1497:111f`–`1140` (`SetViewSize`) and `2f4d:0200`–`02ae` (bitmap build). Nothing here was observed at run time.

**Switching.** `DS:0xC3CC` (`w.ply.map_overlay`): Tab (`buttonstate[12]`, edge-triggered in `2f4d:0bd3` during `T_Player`) steps 0 → 1 and 1 → −1 (−1 ignores Tab). At the start of the next `ThreeDRefresh` (`1b4a:12ad`) a negative value calls `2f4d:0d17`, which stores 0 in both of its branches (`viewsize > 18` or not). `T_Player` runs before the refresh of the same frame, so −1 is never drawn: **Tab toggles the map on and off**. Cleared at every new level (`2f4d:01f4`; not on loading a save). OBCR resets −1 to 0 at the start of `World::visibility_pass`.

**When and where in the frame.** Every refresh with `DS:0xC3CC == 1`, into the back buffer after `DrawPlayerWeapon` and the debug bars, before the message text, the visor splats, the debug overlays and the status-bar icon updates (§2 step 15). Nothing is erased first: pixels the map does not write keep the 3D view. Messages and splats draw over the map. The screen shake (§8.4) moves it with the rest of the screen.

**Coordinates.** `bufferofs −= screenofs` for the duration, so all positions are absolute pixels of the 320×200 page. The origin is set by `SetViewSize`:

```text
X = DS:0x467A = (320 − viewwidth)/2 + 4
Y = DS:0x4678 = (200 − viewheight)/2 + 4        ; 200, not 160: 20 rows lower than the view centring
```

| viewsize | view | view origin | map origin (X, Y) | map box |
|---|---|---|---|---|
| 14 | 224×112 | (48, 24) | (52, 48) | x 52–115, y 48–111 |
| 16 | 256×128 | (32, 16) | (36, 40) | x 36–99, y 40–103 |
| 18 | 288×144 | (16, 8) | (20, 32) | x 20–83, y 32–95 |
| 20 | 320×160 | (0, 0) | (4, 24) | x 4–67, y 24–87 |

Scale: **one pixel per tile**; map tile (tx, ty) is pixel (X + tx, Y + ty), east to the right, south down. Nothing is clipped to the view (smaller view sizes than 14 would draw over the border).

**Drawing, in order** (later writes overwrite earlier ones):

1. **Frame**: `DrawOutline(X, Y, 63, 63, 0x0C, 0x0C)`: `VWB_Hlin(X, X+63, Y)`, `VWB_Vlin(Y, Y+63, X)`, `VWB_Hlin(X, X+63, Y+63)`, `VWB_Vlin(Y, Y+63, X+63)`, colour **0x0C**, end points inclusive: a one-pixel 64×64 outline on map rows/columns 0 and 63 (where the border walls are drawn again in the same colour).
2. **Walls**: a window of the static wall bitmap.
   - Window: `c0 = player.tilex − 12`, if < 0 → 0, then if `c0 + 24 > 64` → 40; `r0` the same from `tiley`. Rows `r0 … r0+23` are drawn; columns start at **`c0 & ~7`** (whole bitmap bytes), so the columns drawn are `c0a … c0a+23` with `c0a = c0 & ~7` (up to 7 columns left of the intended window, and the same number missing on its right). Each row is drawn at its true place inside the box (screen row `Y + ty`), not at the box's top.
   - Every set bit writes colour **0x0C** at (X + tx, Y + ty); clear bits write nothing (no background). The write is planar (each bitmap byte → two planar bytes with Map Mask = high nibble, then low nibble); with the bit order below and `X ≡ 0 (mod 4)` for every view size, tile tx lands exactly on pixel X + tx.
   - With `gamestate+0x16 & 0x20` the window is the whole 64×64 map; nothing sets that bit in OBC-A.
   - The bitmap (`DS:0xE093`, 512 bytes: row `ty`, byte `tx / 8`, bit mask `DS:0x3258[tx & 7]` = 0x10, 0x20, 0x40, 0x80, 0x01, 0x02, 0x04, 0x08) is built by `PrepareLevel` in every `SetupGameLevel` (also when a save is loaded), **after** the plane-0 pass and `SpawnDoor` and **before** `ScanInfoPlane`. Bit set iff (`tilemap` is a solid wall, 1–0x7F, and the plane-1 word ≠ 107) or (`tilemap` is 0 or a door and plane-1 bit 15 is set). Bit 15 is then cleared. In terms of the map file: **plane-0 1–127 unless plane 1 is 107** (rock openings, pipe mouths), **plus plane-0 143** (unused in the shipped maps). Codes 128–139 (walls 128/129, the doors, the ambush code 138, 139) have bit 7 set in `tilemap` and take the door test, so they are not drawn. The bitmap never changes during the level: destroyed or chipped walls, opened door-look walls and moved push walls keep their level-start look; doors never show.
3. **ThermoSensor dots** (single player, `difficulty < 4`, and `gamestate.cheatmode` (`DS:0x873C`, BOGUS MODE) or the ThermoSensor flag `DS:0x3304` (item 10; cleared at every PlayLoop start, so it lasts one level)): for i = 0…99, `v = actorpos[i]` (`DS:0x94ED`, words); if `v ≠ 0`: `tx = v >> 8`, `ty = v & 0xFF`; if `c0a ≤ tx < c0a + 24` and `r0 ≤ ty < r0 + 24` (the wall window; 64 wide/high in whole-map mode): `VL_Plot(X + tx, Y + ty, 1)` — colour **1**. `actorpos[i]` is `tilex << 8 | tiley`, written by `DoActor` for list position i in this frame's actor loop, for every object that runs (active, or in an area connected to the player) except allies (class 5) and the player (index 0 is never written): enemies alive or dead, projectiles, fire, explosions, sparkles, the face. It is cleared before the loop, and BEWITCHED-frozen enemies return before writing, so they disappear from the map.
4. **Allies** (single player): every object after the player with `obclass == 5`: `VL_Plot(X + tilex, Y + tiley, 2)` — colour **2**, **not** limited to the window and shown without the ThermoSensor.
5. **Player** (all modes): `VL_Plot(X + player.tilex, Y + player.tiley, c)`, blinking: byte `DS:0x3272` (initial 0) is toggled on each draw; when it was 0 the colour is **0xFC** (and it becomes 1), otherwise **0x0F** (and it becomes 0). It is never reset, so the phase carries over when the map is closed and reopened.

Network modes (`difficulty ≥ 4`) skip steps 3 and 4.

`VL_Plot` writes one pixel (Map Mask `DS:0x2FAC[x & 3]`, byte `ylookup[y] + x/4 + bufferofs`) with no clipping. All colours are palette indices of the current game palette (normal or dark, §8.2), unshaded; the red/white shifts (§8.1) leave 1, 2, 0x0C and 0x0F alone (indices 0–15 are restored) but tint 0xFC.

For OBCR: the wall bitmap must be captured at level setup (the simulation clears plane-1 bit 15 and later changes `tilemap` and the planes), for example from the original `MAPTEMP` planes with the rule above.

---

## 10. Deltas from Wolf3D (summary)

1. Three ray-cast passes per frame (visibility-only, walls, masked walls) and two `DrawScaleds` calls (§2, §4.4, §5.1).
2. Floor/ceiling: dithered gradients (two modes, per-level tables) plus, in the default mode, screen-space textured ceiling/floor quads per visible tile with per-tile floor codes from plane 0 (§3).
3. Distance shading of walls, sprites and floor tiles through a 256-entry float-built table, 16-colour ramps, full-bright rows 0 and 14–15, black floor; muzzle-flash and config brightness bias; darker table on sewer levels and during tremors (§4.2, §4.5).
4. Palette index 255 transparent on walls (§4.2).
5. One page per wall type (`tile − 1`), door frames via plane-1 marker 125, door pages by lock independent of orientation (§4.1).
6. Push walls identified by plane-1 values 98/101/102; see-through tiles by plane-1 values 103–107 (§4.3).
7. Two-part, bobbing weapon scaled ×1.1 and placed from a per-view-height table (§6.1); screen splats (§6.2).
8. Red shift 4 steps, white shift steps of 10, palette entries 0–15 excluded; dark level palette; no palette cycling (§8).
9. Tremor screen shake via CRTC start address (§8.4).
10. `MAXVISABLE` 75; static objects 10 bytes; no in-game view-size change (§5.1, §9).
11. Gameplay inside the renderer: the shootable-wall search runs every frame fire is held (§9.1).
12. Tab overhead map: a 64×64-pixel box at one pixel per tile, a 24×24-tile window of a level-start wall bitmap, ThermoSensor actor dots, ally dots and a blinking player dot (§9.2).

---

## 11. Open questions

- Q-R1: Confirm by DOSBox-X capture: gradient rows (both modes), ceiling-tile spans, shade values, and the shake offsets. The pixel formulas above are derived from code only.
- ~~Q-R2: How `1b4a:1b8b` and `208c:1c6a` choose and damage shootable tiles (gameplay, not rendering).~~ RESOLVED (2026-11-02): §9.1, CONFIRMED by instruction read; ported in `obc-sim` `vis.rs`.
- Q-R3: Who sets `DS:0x48E2` (brightness bias): `SETUP.EXE`? Range of values?
- Q-R4: Exact meaning of plane-1 values 103–107 and 125 for level designers (window, fence, grate?), and of tile values 95/96; to be settled with the map-code investigation (LEVEL_CODES).
- Q-R5: The two-part weapon sprites: confirm which of sprites 544–623 are left/right halves and the attack frame range, and whether the off-view rows in §6.1 are visible.
- Q-R6: Message text (`2f4d:124a`) and the anchors `DS:0x4664`–`0x4676` belong to a HUD document. (The overhead map and its anchors `DS:0x467A`/`0x4678` are §9.2.)

---

## 12. Symbol corrections

Names in `re/symbols/count_exe.csv` that this analysis contradicts (not edited here):

| Address | Current name | Should be |
|---|---|---|
| `1000:0f5a` | `tan` | `sqrt` |
| `1000:02c1` | `F_LUMOD@` | `LDIV@` (signed long divide; `xor cx,cx` entry) |
| `1000:4886` | `__fmemset_1000_4886` | far memory copy `(dst, src, n)` |
| `2f4d:0024`, `2f4d:000c` | `VL_SetColor_2f4d_0024`, — | CMOS register write / read |
| `2f4d:0405` | `CA_OpenDebug_2f4d_0405` | save `gamepal` to `DS:0xE295` |
| `1000:0fe3` | `tan_1000_0fe3` | `tan` (LIKELY) starts at `1000:0fe6`, the call target; `0fe1`–`0fe5` is the tail of `sin` |
| `1d8b:0a47` | `caseD_5` | `SimpleScaleShape` |
| `1b4a:1ccf` | `TransformActor_1b4a_1ccf` | transform for the shootable-wall search (§9.1) |
| `1b4a:1b8b` | `FUN_1b4a_1b8b` | *IsShootableTile* (§9.1) |
| `2f4d:0d31` | `FUN_2f4d_0d31` | *DrawOverheadMap* (§9.2) |
| `DS:0x0D7B` | `weaponscale+0x9` | `mapmasks1` |
| `DS:0x0D1E` | `mindist` | correct (`MINDIST` = 0x5800, initialized data) |

Useful unnamed globals: `spotvis` `DS:0x961D`, `wallheight[]` `DS:0xC7E4`, `pixx` `DS:0xC29C`, `tilehit` `DS:0xC29E`, `xtile`/`ytile` `DS:0xC29A`/`C298`, `xtilestep`/`ytilestep` `DS:0xC296`/`C294`, `xintercept`/`yintercept` `DS:0xC290`/`C28C` (long), `xstep`/`ystep` `DS:0xC288`/`C284` (long), `lastside` `DS:0xC2C2`, `lastintercept` `DS:0xC2BE` (long), `lasttilehit` `DS:0xC2BC`, `pwallpos` `DS:0x4998`, `mapsegs[0..1]` `DS:0x940F`/`9411`, `farmapylookup` `DS:0x7B2A`, `PMSpriteStart` `DS:0xCA64`, `viewsize` `DS:0x499A`, `screenofs` `DS:0x8A26`, `ylookup` `DS:0xDBEF`, `linewidth` `DS:0xDD7F`.
