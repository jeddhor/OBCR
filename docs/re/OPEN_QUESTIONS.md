# Open questions

Everything we do not yet know about the original that affects the port. Each entry: the question, why it matters (which milestone it blocks), evidence so far, suggested approach, and status. When resolved, keep the entry, set `Status: RESOLVED (<date>)`, write the answer with its confidence and evidence, and link the D, K or format entry that now holds the detail.

Seeded 2026-11-02 from the initial survey.

| Id | Topic | Blocks | Status |
|---|---|---|---|
| Q-001 | Plane-0 code meanings | M2, M4, M5 | OPEN |
| Q-002 | Plane-1 code meanings | M2, M5, M8, M9 | OPEN |
| Q-003 | Floor and ceiling rendering | M4 | OPEN |
| Q-004 | AUDIOMUS.BC layout and rate | M1, M6 | OPEN |
| Q-005 | Digitized SFX sample rate | M1, M6 | OPEN |
| Q-006 | IMF rate and music assignment | M6 | OPEN |
| Q-007 | Audio chunk layout (`NUMSOUNDS`) | M1 | OPEN |
| Q-008 | VGAGRAPH chunks 1, 2, 56, 57 | M1 | OPEN |
| Q-009 | Role of each pic | M1, M10 | OPEN |
| Q-010 | Credits maps | M10 | OPEN |
| Q-011 | Level progression | M5, M10 | OPEN |
| Q-012 | Random game type | M8, M10 | OPEN |
| Q-013 | Save format and slot count | M11 | OPEN |
| Q-014 | CONFIG.DAT settings layout | M1, M5 | OPEN |
| Q-015 | Rendered sequences | M10 | PARTIAL (FLC format known) |
| Q-016 | CD audio track mapping | M6 | DEFERRED (no data) |
| Q-017 | Ally system internals | M9 | OPEN |
| Q-018 | Generators and tremors | M5, M4 | OPEN |
| Q-019 | Multiplayer rules and protocol | M13 | OPEN |
| Q-020 | Memory model and calling conventions | M2 | OPEN |
| Q-021 | Weapon set and mechanics | M7 | OPEN |
| Q-022 | Enemy roster and sprite ranges | M2, M8 | OPEN |
| Q-023 | Status bar layout | M7 | OPEN |
| Q-024 | Timing (`TimeCount`, `MAXTICS`) | M2, M5 | OPEN |
| Q-025 | Palettes | M1, M3 | OPEN |
| Q-026 | Cheats and debug activation | M10 | OPEN |
| Q-027 | Map ThermoSensor | M10 | OPEN |
| Q-028 | Command-line parameter semantics | M10 | OPEN |
| Q-029 | Other releases and their differences | M0, M15 | OPEN |
| Q-030 | High-score semantics (`completed`, `episode`) | M10 | OPEN |
| Q-031 | Keys, cards, locked doors; `HIDDENCARD` | M5 | OPEN |
| Q-032 | Elevators and floor transitions | M5, M10 | OPEN |
| Q-033 | Booby traps | M5, M7 | OPEN |
| Q-034 | Push walls | M5 | OPEN |
| Q-035 | Door types | M4, M5 | OPEN |

---

### Q-001 — What do plane-0 values mean?
- **Matters for**: level setup, renderer, collision, sound propagation.
- **Evidence**: 103 distinct values (DATA_FORMATS §3.4). Floor codes 144–151 dominate walkable space; 145 forms regular grids inside 144 regions (every third tile), which does not fit Wolf3D's area numbering; solid fill values 40 and 91; map 6 shows patterns such as `61 62 63 64` (multi-tile wall art) and `93 94 93` (door or elevator frames?).
- **Approach**: find the `SetupGameLevel`/`ScanInfoPlane` equivalents (M2.R4); trace how plane-0 values become `tilemap`, `actorat`, door spawns and area numbers; confirm visually with the oracle at chosen tiles.
- **Status**: OPEN

### Q-002 — What do plane-1 values mean?
- **Matters for**: spawns (enemies, allies, items, decorations), patrol paths, player start, special tiles.
- **Evidence**: 215 distinct values (0–317); 18 is the background; 89, 103, 104 appear on wall cells; values above 255 exist.
- **Approach**: same as Q-001, following the plane-1 switch in the spawn routine; cross-check with sprite exports.
- **Status**: OPEN

### Q-003 — How are floors and ceilings drawn?
- **Matters for**: renderer parity (M4.3).
- **Evidence**: strings `Rindex %d`, `Findex %d`, `Rclr %d`, `Fclr %d` (debug output); plane-0 grid patterns.
- **Approach**: find who prints those strings (DS offsets in STRINGS.md); read the 3D refresh for floor and ceiling code; capture screenshots of rooms with 144 and 145 tiles to see whether ceilings or floors differ per tile.
- **Status**: OPEN

### Q-004 — What is the layout and sample rate of AUDIOMUS.BC?
- **Matters for**: background music (M6.6).
- **Evidence**: 21 × 64 KiB, PCM-like statistics, 0x00 padding (DATA_FORMATS §6).
- **Approach**: find the loader (string `AUDIOMUS.`) and the Sound Blaster programming (DSP time constant command `0x40`, or `0x41` for an output rate); check whether a directory exists in the file or in `COUNT.EXE`; check which game events start or switch it.
- **Status**: OPEN

### Q-005 — What sample rate do digitized SFX use?
- **Matters for**: correct pitch (M6.5).
- **Evidence**: Wolf3D uses about 7042 Hz.
- **Approach**: find the digitized-sound start routine and the DSP time constant; confirm by ear against DOSBox-X.
- **Status**: OPEN

### Q-006 — IMF playback rate and which track plays where
- **Matters for**: music (M6.4, M10).
- **Evidence**: 9–10 IMF chunks after chunk 300.
- **Approach**: timer ISR rate when music plays; per-level song table (Wolf3D `songs[]` in `WL_PLAY.C`).
- **Status**: OPEN

### Q-007 — Exact audio chunk layout
- **Matters for**: sound id mapping.
- **Evidence**: zero-length chunks 200–299, 4-byte chunk 300, `!ID!` markers.
- **Approach**: read the sound start routines' chunk arithmetic (`STARTADLIBSOUNDS` etc.).
- **Status**: OPEN

### Q-008 — What are VGAGRAPH chunks 1, 2, 56 and 57?
- **Evidence**: 12,314 and 1,650 bytes (fonts?), 5 bytes each (unknown).
- **Approach**: decode as Wolf3D `fontstruct`; find `CA_CacheGrChunk` calls with these numbers.
- **Status**: OPEN

### Q-009 — What is each of the 53 pics?
- **Evidence**: sizes (DATA_FORMATS §4).
- **Approach**: export (M1.8) and view; find usages in the code.
- **Status**: OPEN

### Q-010 — How are CRED1–CRED10 maps used?
- **Evidence**: 10 tiny maps named CRED; slot 60 empty CRED9.
- **Approach**: find references to map numbers 40–49; watch the credits in DOSBox-X.
- **Status**: OPEN

### Q-011 — What is the level progression?
- **Evidence**: map order by building; manual mentions floors and AutoSave before "new floor".
- **Approach**: find the level-completion code and any next-level table; note secret or branching exits; verify by playing.
- **Status**: OPEN

### Q-012 — How does the Random game type work?
- **Evidence**: manual: "nothing to keep you from getting stuck … so many enemies".
- **Approach**: follow the game-type variable from the new-game menu into level setup.
- **Status**: OPEN

### Q-013 — Save format and slot count
- **Evidence**: DATA_FORMATS §8.
- **Approach**: read the save and load routines; diff saves taken one action apart.
- **Status**: OPEN

### Q-014 — CONFIG.DAT settings layout
- **Evidence**: DATA_FORMATS §7 (hypothesis decoding).
- **Approach**: read the config read and write routines (string `CONFIG.`).
- **Status**: OPEN

### Q-015 — Rendered sequences: files, format, timing
- **Evidence**: strings (`ANM proc`, `frame %d uncool chunk # %d`, `norndseq`, `slowrndseq`). The owner's floppy archives (`bc3.cmp`, a plain PKZIP file) contain `F2.BC` (31 frames), `F3.BC` (30), `F4.BC` (54), `F6.BC` (34): Autodesk **FLC** files (magic `0xAF12`, 320×200, 8-bit). CONFIRMED.
- **Remaining**: which event plays which file (floor change, death; why there is no F1 or F5), playback speed (`slowrndseq`), palette handling, sound during playback.
- **Status**: PARTIAL (format known; usage open)

### Q-016 — CD audio track mapping
- **Evidence**: MSCDEX strings, `cdmusic` parameter. The owner has no CD audio (2026-11-02).
- **Decision**: CD music stays an optional feature for user-supplied files, implemented last; FM and PCM music are the defaults.
- **Status**: DEFERRED (no data)

### Q-017 — How do allies work internally?
- **Evidence**: D-011.
- **Approach**: M9.R1–R5: find the ally class and state tables, the `A`/`S` key handlers, `CT_Path`, the switch routine.
- **Status**: OPEN

### Q-018 — What do generators and tremors do?
- **Evidence**: toggle strings; manual tip "Booby traps or spontaneous human combustion?"
- **Approach**: find the toggle code and all readers of the flags; observe in game.
- **Status**: OPEN

### Q-019 — Multiplayer rules and protocol
- **Evidence**: D-020.
- **Approach**: two DOSBox-X instances with IPX emulation; RE of packet build and receive (M13.R1–R3).
- **Status**: OPEN

### Q-020 — Memory model and calling conventions
- **Evidence**: Borland RTL, near-data messages; LIKELY medium model.
- **Approach**: inspect prologues and epilogues (`RETF`), DS usage, pointer sizes in M0.12.
- **Status**: OPEN

### Q-021 — Weapon set and mechanics
- **Evidence**: D-012; five weapon-select keys in `CONFIG.DAT` defaults.
- **Approach**: M7.R1–R3.
- **Status**: OPEN

### Q-022 — Enemy roster and sprite ranges
- **Evidence**: 680 sprites; manual mentions rats, swamp creatures, Victor.
- **Approach**: M2.R13, M8.R1.
- **Status**: OPEN

### Q-023 — Status bar layout
- **Evidence**: pic 29 is 320×40; pics 30–35 are 64×32 (weapon icons?); pics 36–52 are 32×32 (faces or portraits?).
- **Approach**: capture the HUD in several states; find the draw routines.
- **Status**: OPEN

### Q-024 — Timing
- **Evidence**: Wolf3D `TimeCount` at 70 Hz, `MAXTICS = 10`.
- **Approach**: timer ISR and `CalcTics` equivalent (M2.R11).
- **Status**: OPEN

### Q-025 — Palettes
- **Evidence**: two 768-byte candidates (DATA_FORMATS §9).
- **Approach**: find `VL_SetPalette` callers and the palette pointer; compare with a DAC dump from DOSBox-X.
- **Status**: OPEN

### Q-026 — Cheat and debug activation
- **Evidence**: D-019.
- **Approach**: find the keyboard-sequence matcher (Wolf3D: `CheckKeys` in `WL_PLAY.C`) and the string compare against the passwords.
- **Status**: OPEN

### Q-027 — What does the Map ThermoSensor do?
- **Evidence**: pickup string only.
- **Approach**: find the item's effect flag and its readers; observe in game.
- **Status**: OPEN

### Q-028 — Command-line parameter semantics
- **Evidence**: D-018.
- **Approach**: find the `US_CheckParm` equivalent and each parameter's consumer.
- **Status**: OPEN

### Q-029 — Which releases exist, and how do they differ?
- **Evidence**: this copy is dated 1996-12-24 (`OBC-A`); ECWolf knows a Steam release (app 1627120, data under `C/BCCD`).
- **Approach**: ask the owner for other copies; hash and diff; record manifests in `obc-formats::release`.
- **Status**: OPEN

### Q-030 — High-score field semantics
- **Evidence**: `completed = 1`, `episode = 0` in all defaults.
- **Approach**: find the high-score check and draw routines.
- **Status**: OPEN

### Q-031 — Keys, cards and locked doors; what is `HIDDENCARD`?
- **Evidence**: `HIDDENCARD` string near the parameter table.
- **Approach**: find the string's users; door lock handling (M5.R2).
- **Status**: OPEN

### Q-032 — Elevators and floor transitions
- **Evidence**: building floors; `Get Ready!`; AutoSave "just before you get taken to a new floor".
- **Approach**: find the exit trigger and the transition sequence (M5.R6, M10.R4).
- **Status**: OPEN

### Q-033 — Booby traps
- **Evidence**: `BOOBY TRAP` pickup or message string; manual tip.
- **Approach**: find the string's user and the trap trigger.
- **Status**: OPEN

### Q-034 — Push walls
- **Evidence**: manual says secret rooms use push walls opened with Space (Wolf3D behavior).
- **Approach**: delta-check `PushWall` and `MovePWalls`; find the push-wall marker code in plane 1.
- **Status**: OPEN

### Q-035 — Door types
- **Evidence**: candidate door codes in plane 0 (69, 93/94, 117, 138 in map 6) to be confirmed.
- **Approach**: M5.R2.
- **Status**: OPEN
