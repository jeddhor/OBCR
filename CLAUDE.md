# OBCR — agent instructions

OBCR is a Rust + SDL3 source port of Operation Body Count (1994), a game built on a modified Wolfenstein 3D engine. There is no original source code: we port the Wolf3D source and apply OBC's differences, found by reverse engineering `COUNT.EXE` (Ghidra) and by observing the original in DOSBox-X.

The full manual is [docs/AGENT_GUIDE.md](docs/AGENT_GUIDE.md). Read the relevant parts before working; this file is the summary.

## Start of every session

1. Read [docs/STATUS.md](docs/STATUS.md): current milestone, active tasks, blockers.
2. Read your task in [docs/MILESTONES.md](docs/MILESTONES.md) and the relevant sections of [docs/DESIGN.md](docs/DESIGN.md).
3. Check the knowledge base in [docs/re/](docs/re/README.md) before investigating anything.
4. Claim your task in STATUS.md before you start.

## Rules

- **The original is the spec.** Wolf3D behavior is a hypothesis for OBC, never proof. Every ported function gets a delta check against `COUNT.EXE` (DESIGN §2.3); unchecked code is marked `// DELTA-UNCHECKED`.
- **Evidence and confidence.** Every claim about the original carries `CONFIRMED`, `LIKELY`, `HYPOTHESIS` or `UNKNOWN` and a citation (AGENT_GUIDE §4). Never state a guess as fact.
- **Write findings down as you go** in `docs/re/` and `re/` (AGENT_GUIDE §5.6). The chat is not documentation.
- **Simulation purity.** `obc-sim` has no I/O, no clock, no floating point, no hash-map iteration order, no randomness except `world.rng`. It emits events; it never calls audio, render or UI.
- **Structure-preserving ports.** Mirror the original's control flow, integer widths (`int` → `i16`, `long` → `i32`) and RNG call order. Use `wrapping_*` where the original overflows. Cite origin and deltas in doc comments; add `#[doc(alias = "OriginalName")]`.
- **No blocking loops.** The original's nested loops and waits become `Screen`s and `Sequence`s (DESIGN §7.6).
- **Verify before done.** Run the task's acceptance check plus `cargo xtask ci`. Behavior tasks need a trace diff or golden test. Definition of done: AGENT_GUIDE §11.
- **Never commit original data or decompiled code**: no game files, extracted assets, palettes, map dumps, traces, captures or decompiled functions. They stay in `.local/` (git-ignored).
- **Never modify the reference trees** `..\OBC`, `..\wolf3d`, `..\ECWolf`. Never run the game from `..\OBC`; copy it to `.local\dosbox\` first. DOSBox-X changes go only on its local `obc-tools` branch and are never pushed upstream.
- **No ECS, async runtime, scripting language or plugin system.** Plain data, functions, state tables and `fn` pointers.
- **Leave the tree green** at the end of every session, even mid-task (set the task to `Paused` with resume notes).

## Commit policy (owner may change this)

Commit completed, verified work on a task branch (`m<N>/<task-id>-<slug>`) using Conventional Commits with the task id, for example `feat(sim): port MoveDoors (M5.4)`. Merging to `main` and pushing to any remote require the owner's go-ahead.

## Ask the owner when

Licensing, missing data only they can supply (CD or Steam release), behavior that cannot be determined and would play differently either way, scope changes, destructive actions, or anything touching paths outside this repository. Record the question under Blockers in STATUS.md. Details: AGENT_GUIDE §12.

## Paths

| Path | What |
|---|---|
| `..\OBC\` | Original game (read-only) |
| `..\wolf3d\WOLFSRC\` | Wolf3D source, `WOLF3D.EXE` (Turbo Debugger symbols) and `WOLF3D.MAP` |
| `..\EC7Wolf\` | The owner's Corridor 7 port: best reference for Capstone engine changes OBC inherited (concepts and leads; OBC's numbers come from `COUNT.EXE`) |
| `..\ECWolf\` | ECWolf source (reference only; never port from `src\mame\`) |
| `..\dosbox-x\` | DOSBox-X with debugger, JSON-RPC agent (`docs\rpc-agent-usage.md`, `client\python`) |
| `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC\` | Ghidra (`support\analyzeHeadless.bat`) |
| `..\ghidra\` | Ghidra project `OBC`; named text exports in `.local\ghidra\export\` (`decomp/`, `asm/`, `functions.tsv`, `strings.tsv`) |
| `.local\` | Git-ignored workspace: Ghidra project, DOSBox runtime dirs, captures, traces, exports |

## Commands

```powershell
cargo xtask ci            # format, clippy -D warnings, tests, cargo deny
cargo xtask test-data     # tests that need the original data (OBC_DATA_DIR or ..\OBC)
cargo run -p obc-tool -- info
powershell -File installer\build-windows.ps1   # Windows installer -> target\dist (Qt 6 MinGW kit)
installer/build-linux.sh                       # Linux installer -> target/dist (run in WSL Ubuntu 22.04)
```

Commands appear as milestone M0 lands; if one is missing, building it is a task.

## Key facts (details in docs/re/)

- `COUNT.EXE`: Borland C++ 16-bit real mode, not packed. Header 0x3400 bytes: image offset = file offset − 0x3400. DGROUP is image paragraph 0x3218 (file 0x35580); DS offset = file offset − 0x35580. At the DOSBox-X entry stop, CS is the load segment.
- Data files use the `.BC` extension; `GFXTILES.BC` is the VSWAP page file; `MAPTEMP.BC` maps are RLEW-only (no Carmack); 61 map slots.
- OBC's `rndtable` is byte-identical to Wolf3D's.
- `gamepal` is at `COUNT.EXE` file 0x2DDB0; full-screen pictures use the palette at 0x2E3B0.
- `AUDIOMUS.BC` holds 99 digitized sounds (Corridor 7 layout), not music.
