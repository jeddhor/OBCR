# OBCR Master AI Agent Development Guide

| | |
|---|---|
| Version | 1.0 — 2026-11-02 |
| Audience | AI coding agents (and the humans directing them) working on OBCR |
| Companions | [DESIGN.md](DESIGN.md) (what and why) · [MILESTONES.md](MILESTONES.md) (task catalog) · [STATUS.md](STATUS.md) (live state) · [re/](re/README.md) (knowledge base) |

This guide is the operating manual. [CLAUDE.md](../CLAUDE.md) at the repository root is the short version that loads automatically; this is the full version. When the two disagree, fix whichever is wrong in the same change.

---

## 1. Prime directives

1. **The original is the spec.** OBCR must behave like `COUNT.EXE`. Wolf3D source is a starting hypothesis, never proof of OBC behavior.
2. **No evidence, no claim.** Every statement about the original carries a confidence tag and a citation (§4). Never present a guess as a fact, in code comments, docs or replies.
3. **Verify, then mark done.** A task is done when its acceptance check passes and the evidence is recorded in STATUS.md (§11).
4. **Write it down.** Knowledge that exists only in your context window is lost at the end of the session. Findings go into `docs/re/` and `re/` as you make them, not at the end.
5. **Never commit original data or decompiled code.** Not as files, not as hex dumps, not as test fixtures, not pasted into comments (§10.3).
6. **Keep the simulation pure.** No I/O, clock, floating point or hash-map iteration in `obc-sim` (DESIGN §8).
7. **Leave the tree green.** `cargo xtask ci` passes at the end of every session, even if the task is unfinished.
8. **Small, claimed, focused tasks.** One task per branch. Claim it in STATUS.md. Do not wander into neighboring work; record it as a follow-up instead.
9. **Ask when the decision is the owner's** (§12). Do not guess on licensing, data acquisition, scope or fidelity trade-offs.

---

## 2. Orientation

### 2.1 Workspace map

| Path | What | Access |
|---|---|---|
| `P:\msdos\OBC\OBCR\` | **This repository** | Read/write |
| `P:\msdos\OBC\OBC\` | Original game (pristine reference copy) | **Read-only.** Copy before running. |
| `P:\msdos\OBC\wolf3d\WOLFSRC\` | Wolf3D source, `WOLF3D.EXE` (with Turbo Debugger symbols), `WOLF3D.MAP` | Read-only |
| `P:\msdos\OBC\EC7Wolf\` | The owner's Corridor 7 port (ECWolf fork): best reference for Capstone engine changes OBC inherited | Read-only |
| `P:\msdos\OBC\ECWolf\` | ECWolf source | Read-only |
| `P:\msdos\OBC\ghidra\` | Ghidra project `OBC` (programs `count.exe`, `WOLF3D.EXE`). Outside `.local` because Ghidra rejects dot-folders in project paths. | Read/write, one writer at a time |
| `P:\msdos\OBC\dosbox-x\` | DOSBox-X source with debugger, JSON-RPC agent and control server | Build in place; source changes only on local branch `obc-tools` (§6.8) |
| `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC\` | Ghidra 12.2 (`support\analyzeHeadless.bat`, `support\pyghidraRun.bat`) | Read-only install |
| `OBCR\.local\` | Machine-local, git-ignored: Ghidra project, DOSBox runtime dirs, captures, traces from the original, exports | Read/write |

Installed toolchain (as of 2026-11-02): Rust 1.95.0 with clippy, rustfmt, cargo-deny, miri; CMake; LLVM/clang; Visual Studio (2022 and 18); Python 3.10, 3.13, 3.14; JDK 25 (Adoptium); Git 2.45.

### 2.2 Reading order for a new session

1. [CLAUDE.md](../CLAUDE.md).
2. [STATUS.md](STATUS.md): current milestone, claimed tasks, recent log, blockers.
3. The milestone section in [MILESTONES.md](MILESTONES.md) for your task.
4. The DESIGN.md sections for the subsystem you will touch.
5. The relevant knowledge-base files: [SYMBOLS](re/SYMBOLS.md), [DELTA_SPEC](re/DELTA_SPEC.md), [DATA_FORMATS](re/DATA_FORMATS.md), [OPEN_QUESTIONS](re/OPEN_QUESTIONS.md), [STRINGS](re/STRINGS.md).
6. `git status` and `git log --oneline -20`.

### 2.3 Useful commands

```powershell
cargo xtask ci                      # fmt check, clippy -D warnings, tests, cargo deny (no data needed)
cargo xtask test-data               # data-gated tests (uses OBC_DATA_DIR or ..\OBC)
cargo test -p obc-formats           # one crate
cargo run -p obc-tool -- info       # data inventory and release id
cargo run -p obc-tool -- maps render 6 --out .local/exports/maps
cargo run -p obcr -- --viewer --map 6
cargo run -p obcr -- --headless --map 0 --replay .local/traces/s1.jsonl --trace .local/traces/s1.obcr.jsonl
cargo run -p obc-tool -- trace diff .local/traces/s1.jsonl .local/traces/s1.obcr.jsonl
python -I tools\obc-oracle\oracle.py --help
```

(Commands become available as M0 and M1 land. If one does not exist yet, that is a task, not a reason to improvise a different workflow.)

---

## 3. Task lifecycle

### 3.1 Select

- Work on the **current milestone** in STATUS.md unless the owner directs otherwise. RE tasks for the next milestone may run ahead.
- Pick an unclaimed task whose prerequisites are done. If a task is too large for one session (more than about 500 changed lines, or more than one subsystem), split it in STATUS.md into sub-tasks (`M5.4a`, `M5.4b`) before starting.

### 3.2 Claim

Edit the task row in STATUS.md: status `In progress`, owner (agent or session name), branch name, date. Commit that change on its own if other agents work in parallel, so claims are visible.

### 3.3 Research

- For behavior tasks: run the **delta check** (DESIGN §2.3) before writing code. Find the function in `COUNT.EXE`, compare with Wolf3D, record deltas.
- For anything uncertain: check OPEN_QUESTIONS.md. If your task depends on an open question, resolving it is part of your task. Record what you did.

### 3.4 Plan

Write a short plan into the task's notes in STATUS.md (3–10 bullets): files to touch, tests to add, how you will verify. For tasks touching more than one crate, list the order of changes.

### 3.5 Implement

Follow the coding standards (§7). Keep commits small and logical. Cite evidence in doc comments.

### 3.6 Verify

Run the task's acceptance check (MILESTONES.md) plus `cargo xtask ci`. For behavior, that means trace diff or golden test. "It compiles" and "it looks right" are not verification when a precise check is possible.

### 3.7 Document

Update every affected doc (§9). Update STATUS.md: status `Done`, evidence (command plus result, test names, capture ids), follow-ups.

### 3.8 Hand off

Leave a short note in STATUS.md's log: what changed, what is next, what surprised you. If the session ends mid-task, set status `Paused` with exact resume instructions.

### 3.9 Parallel agents

- One branch and one git worktree per agent (`git worktree add ../OBCR-wt-<task> -b <branch>`), or the Agent tool's `isolation: "worktree"`.
- **Ownership by crate.** Two agents must not edit the same crate at once unless the tasks are explicitly coordinated in STATUS.md.
- **Shared docs** (SYMBOLS, DELTA_SPEC, OPEN_QUESTIONS, STATUS): append entries; do not reorder or reformat other entries; keep edits small to minimize merge conflicts.
- **Ghidra project**: only one process may write to `.local/ghidra/OBC.gpr` at a time. Other agents read the exported text in `.local/ghidra/export/` and propose symbol changes by editing `re/symbols/count_exe.csv`; the next import applies them.
- **DOSBox-X**: every agent uses its own runtime directory and its own named-pipe endpoint.

---

## 4. Evidence and confidence

### 4.1 Confidence levels

| Tag | Meaning | Minimum evidence |
|---|---|---|
| `CONFIRMED` | Verified directly | Observed in DOSBox-X (trace, memory, capture), **or** unambiguous disassembly that you read instruction by instruction, **or** parse of the data file that matches the original's use of it |
| `LIKELY` | Strong indirect evidence | Decompiler output read but not traced; a Wolf3D match plus partial disassembly check; consistent data statistics |
| `HYPOTHESIS` | Plausible guess | Naming, strings, Wolf3D precedent, analogy |
| `UNKNOWN` | No useful evidence yet | — |

Code may be built on `LIKELY` facts only if the task's acceptance test will confirm or refute them (for example a trace diff). Code built on `HYPOTHESIS` must be marked `// HYPOTHESIS(Q-###)` and cannot be marked done.

### 4.2 Citation formats

| Source | Format | Example |
|---|---|---|
| `COUNT.EXE` code or data | `img+0x<linear>` plus name if known; `DS:0x<off>` for DGROUP data; file offset in parentheses when useful | `img+0x2A4F2 rndtable (file 0x2D8F2)`; `DS:0x03A8 "BC ERROR: "` |
| Ghidra | `ghidra <seg>:<off> <FunctionName>` with the project's base noted in SYMBOLS.md | `ghidra <G+S>:0134 T_Chase` (illustrative) |
| Wolf3D source | `WOLFSRC/<FILE>:<line>` | `WOLFSRC/WL_ACT2.C:3069` (`T_Chase`) |
| ECWolf | `ECWolf/<path>:<line>` | `ECWolf/src/resourcefiles/file_gamemaps.cpp:60` |
| Oracle trace or capture | `trace:<id>` / `capture:<id>` (ids from `re/captures.toml`) | `trace:m0-sewer1-walk-01 frame 412` |
| Knowledge-base entry | `D-###`, `K-###`, `Q-###` | `D-007` |

### 4.3 Address conventions

`COUNT.EXE` facts:

- MZ header size is 0x3400 bytes (832 paragraphs). **Image-linear offset = file offset − 0x3400.**
- The header's initial CS:IP is 0000:0000, so **at the entry stop in DOSBox-X, CS equals the load segment `L`**. Any image segment `S` is at runtime segment `L + S`.
- DGROUP is image paragraph **0x3218** (file offset 0x35580; DS:0000 holds 4 zero bytes, then the Borland copyright string). After C0 startup, **DS = L + 0x3218**, and a DGROUP item's DS offset = file offset − 0x35580.
- Ghidra loads MZ files at its own base segment `G` (check *Window → Memory Map* on first import and record it in SYMBOLS.md). Ghidra segment = `G + S`.

Always store the image-linear offset (tool-independent) in `re/symbols/count_exe.csv`, plus `S:off` once the segment map is known (M2.R1).

---

## 5. Reverse-engineering playbook (Ghidra)

### 5.1 Project setup (M0.12)

- Project directory `P:\msdos\OBC\ghidra\`, project `OBC` (not under `.local`: Ghidra rejects project paths containing a folder that starts with a dot). Program `count.exe` imported with the MZ loader, language `x86:LE:16:Real Mode`.
- Immediately after import: set the **DS register** to DGROUP (`G + 0x3218`) over every code range (*Select All → Set Register Values → DS*, or the `SetDgroup` script). Without this, data references resolve wrongly.
- In the Borland medium/small data model, **SS = DS** after startup (stack lives in DGROUP). Pointers to locals passed as near pointers are DS-relative.
- Re-run auto-analysis after setting DS.
- Then apply `re/symbols/count_exe.csv` and parse `re/types/obc_types.h` (*File → Parse C Source*, or the `ApplyTypes` script).

The Ghidra project is a **cache**. The source of truth is the text state in `re/` (symbols CSV, types header). Anyone must be able to delete `.local/ghidra/` and rebuild it with one command.

### 5.2 Headless commands

```powershell
$G = 'C:\Users\jason\Desktop\ghidra_12.2_PUBLIC\support'
# Fresh import (verified 2026-11-02): DS/SS set before analysis, then names and export
& "$G\analyzeHeadless.bat" P:\msdos\OBC\ghidra OBC `
    -import P:\msdos\OBC\OBC\count.exe -loader MzLoader -processor "x86:LE:16:Real Mode" `
    -scriptPath P:\msdos\OBC\OBCR\tools\ghidra -preScript SetDgroup.java `
    -postScript ApplySymbols.java P:\msdos\OBC\OBCR -postScript ExportAll.java P:\msdos\OBC\OBCR\.local\ghidra\export
# Re-apply symbols and re-export without re-analysis
& "$G\analyzeHeadless.bat" P:\msdos\OBC\ghidra OBC -process count.exe -noanalysis `
    -scriptPath P:\msdos\OBC\OBCR\tools\ghidra `
    -postScript ApplySymbols.java P:\msdos\OBC\OBCR -postScript ExportAll.java P:\msdos\OBC\OBCR\.local\ghidra\export
# Wolf3D reference program, named from its linker map (DGROUP paragraph 3A39)
& "$G\analyzeHeadless.bat" P:\msdos\OBC\ghidra OBC `
    -import P:\msdos\OBC\wolf3d\WOLFSRC\WOLF3D.EXE -loader MzLoader -processor "x86:LE:16:Real Mode" `
    -scriptPath P:\msdos\OBC\OBCR\tools\ghidra -preScript ApplyWolfMap.java P:\msdos\OBC\wolf3d\WOLFSRC\WOLF3D.MAP 3A39 `
    -postScript ExportAll.java P:\msdos\OBC\OBCR\.local\ghidra\export-wolf
```

- Prefer **Java GhidraScripts** for headless reliability. PyGhidra (`pyghidraRun.bat`) is available for Python 3 scripts; confirm the exact headless invocation once and record it.
- `ExportAll` writes `.local/ghidra/export/functions.tsv` (address, name, size, callers, callees), `strings.tsv` (address, DS offset, text), and `decomp/<seg>_<off>_<name>.c` per function. Agents grep these instead of driving the GUI.
- `ExportSymbols` writes names, labels, comments and types back to `re/symbols/count_exe.csv` and `re/types/obc_types.h`. **Run it after every naming session**, and commit the CSV and header.
- If a Ghidra MCP server is configured in the session, it may be used for interactive queries. Changes must still flow back into `re/` through `ExportSymbols`.

### 5.3 Seeding from Wolf3D (M0.13)

1. Import `WOLFSRC\WOLF3D.EXE` into the same Ghidra project.
2. Apply names from `WOLFSRC\WOLF3D.MAP` ("Publics by Value", about 1,500 lines starting at line 2099). Map addresses are `segment:offset` relative to image segment 0; add Ghidra's base `G`. The segment table at the top of the MAP lists each module's `_TEXT` segment, which is how you attribute segments to modules.
3. The EXE also carries Turbo Debugger symbol data (`FB 52` at file offset 311232) with locals and types. Parsing it is optional extra credit.
4. Transfer names into `COUNT.EXE`: `python -I tools/ghidra/match_functions.py` compares normalized instruction sequences of every function pair and writes `re/symbols/count_exe.csv` (279 of 937 functions matched on 2026-11-02, mostly the RTL and `ID_*` libraries). Then `python -I tools/ghidra/match_globals.py` aligns the matched pairs to transfer DGROUP variable names into `re/symbols/count_exe_data.csv` (273 offsets). `ApplySymbols.java` applies both; ambiguous matches get a `maybe_` prefix.
5. Every transferred name enters the CSV as `LIKELY`, source `wolf3d-match`. It becomes `CONFIRMED` only after someone reads the function.

### 5.4 Finding things

| Want | Technique |
|---|---|
| Function using a string | Compute the DS offset (file offset − 0x35580; see [STRINGS.md](re/STRINGS.md)) and search for that immediate (*Search → For Scalars*), typically in `push imm` or `mov ax, imm` before a call |
| A table | Search for a known byte sequence. The `rndtable` was found this way at file 0x2D8F2. Wolf3D tables (sin, `pixelangle`, state records) have recognizable shapes. |
| A Wolf3D function | Find a called helper you already know (for example `US_RndT`, `SD_PlaySound`, `CA_CacheGrChunk`), then walk callers. Constants from Wolf3D (`MINDIST = 0x5800`, `0x10000`, `ANGLES = 360`) are good scalar searches. |
| Interrupt handlers | Look for writes to the vector table (`es:[0x20]` for INT 8, `es:[0x24]` for INT 9) or `setvect`/`getvect` RTL calls |
| Data structure layout | Find the allocator or spawn function, then all `[bx+N]` offsets used on the returned pointer; confirm with a live memory dump in DOSBox-X |
| What a value means at run time | Set a breakpoint or memory-change breakpoint in DOSBox-X (§6) |

### 5.5 Pitfalls in 16-bit Borland code

- **Long arithmetic helpers.** `LXMUL@` (DX:AX × CX:BX), `LDIV@`, `LUDIV@`, `LMOD@`, `LUMOD@`, `LXLSH@`, `LXRSH@`, `LXURSH@` appear as calls. Name them first; then 32-bit math becomes readable.
- **Pointer helpers.** `PADD@`, `PSUB@`, `PSBP@` normalize huge pointers; `N_SCOPY@` and `F_SCOPY@` copy structs.
- **Far and near.** Medium model: functions are far (`CALLF`, `RETF`, args start at `[bp+6]`), data pointers are near (DS) unless declared far. Big buffers (graphics, page cache, EMS/XMS frames) use far pointers.
- **Register-argument assembly.** `ID_*_A.ASM` and `WL_*.ASM` routines take arguments in registers and globals. The decompiler gets these wrong: read the disassembly.
- **386 code paths.** The `no386` parameter implies 386-specific code (`66h` operand-size prefixes). Check disassembly for 32-bit operations the decompiler may render oddly.
- **Generated code.** Wolf3D builds its wall and sprite scalers at run time (`BuildCompScale`). If OBC does the same, the scaler code is not in the EXE; read the generator.
- **Jump tables.** Switches via `jmp cs:[bx+table]` may need manual table definition.
- **Sign-magnitude fixed point.** `FixedByFrac` and similar helpers use sign-magnitude fractions, not two's complement.
- **Overlapping segments.** Borland segments overlap in paragraph space; a far pointer can name the same byte with different segment:offset pairs. Normalize to image-linear offsets in notes.
- **Segment constants shown as strings.** Ghidra often renders a pushed segment value as a pointer into a DGROUP string; for example `(char *)s_Communicating_on_IPX_socket__X_4218_3a90 + 0xb` is really the segment `0x3A9B`. Read the disassembly for far-pointer arguments.
- **Wolf3D bias.** When a decompiled function "looks like" its Wolf3D counterpart, read it anyway, line by line. OBC changes are often single constants or one extra branch.

### 5.6 Recording RE results

| Finding | Goes to |
|---|---|
| Function or global name | `re/symbols/count_exe.csv` (via `ExportSymbols`) plus a row in [SYMBOLS.md](re/SYMBOLS.md) if it is a landmark |
| Struct or enum | `re/types/obc_types.h` |
| File format detail | [DATA_FORMATS.md](re/DATA_FORMATS.md) |
| Map code meaning | `re/LEVEL_CODES.md` (created in M2) plus the machine-readable table |
| Behavior difference from Wolf3D | [DELTA_SPEC.md](re/DELTA_SPEC.md) `D-###` |
| Original bug or oddity | [DELTA_SPEC.md](re/DELTA_SPEC.md) Quirks `K-###` |
| Unresolved question | [OPEN_QUESTIONS.md](re/OPEN_QUESTIONS.md) `Q-###` (with what you tried) |
| Interesting string | [STRINGS.md](re/STRINGS.md) |

Entry template for DELTA_SPEC (copy it):

```markdown
### D-0NN — <short title>
- **Area**: <subsystem>  **Status**: OPEN | IMPLEMENTED (<commit>) | VERIFIED (<trace/test>)
- **Wolf3D**: <behavior> (`WOLFSRC/<FILE>:<line>`)
- **OBC**: <behavior> (`img+0x....` <name>)
- **Evidence**: <how you know>  **Confidence**: CONFIRMED | LIKELY | HYPOTHESIS
- **Port impact**: <what OBCR must do; crate/module>
```

---

## 6. DOSBox-X oracle playbook

The local DOSBox-X tree contains two automation interfaces. **Use the JSON-RPC agent** for everything it supports; use the control server only for raw debugger commands.

### 6.1 Build (M0.8)

- Configuration `Agent Debug SDL2|x64` defines `C_DEBUG=1`, `C_HEAVY_DEBUG=1`, `C_DOSBOX_AGENT=1`. `Agent Debug No Heavy SDL2|x64` drops heavy debug (faster; no CPU trace or memory-change breakpoints).
- Build per `..\dosbox-x\docs\rpc-development.md` (task `RPC-A01`): `msbuild .\vs\dosbox-x.sln /m /t:dosbox-x /p:Configuration="Agent Debug SDL2" /p:Platform=x64` from `..\dosbox-x`, using the MSBuild found by `vswhere`. The Agent configurations link libraries from `bin\x64\Debug SDL2\`; build those first if the link fails.
- Output: `..\dosbox-x\bin\x64\Agent Debug SDL2\dosbox-x.exe`. Record the working command and any fixes in `tools/dosbox/README.md`.

### 6.2 Runtime directories

- Never point DOSBox-X at `..\OBC`. `tools/dosbox` copies it to `.local\dosbox\run-<name>\` (the game writes `CONFIG.DAT`, saves, `BC.LOG`).
- Use a fixed DOSBox configuration: `core=normal`, fixed `cycles`, Sound Blaster 16 with OPL, a fixed `memsize`, EMS and XMS on (the game uses both). Store it as `tools/dosbox/obc.conf`.

### 6.3 JSON-RPC agent (preferred)

Documentation (Chinese): `..\dosbox-x\docs\rpc-agent-usage.md` (usage), `docs\rpc.md` (protocol). Python client: `..\dosbox-x\client\python\dosbox_agent` (`AgentClient`, `MemoryAddress`).

- Start: `dosbox-x.exe --agent-config <file.env>`. The `.env` file (absolute paths only) sets `transport=named_pipe`, `endpoint=\\.\pipe\obc-<unique>`, `dosbox_executable`, `dosbox_workdir` (the runtime dir, mounted as `C:`), `profile=production`, `request_timeout_ms`, `max_message_bytes`, `max_memory_read_bytes` (≤ 65536 per read), `max_trace_events`.
- Methods: `agent.capabilities`, `session.start` (stops at program entry), `session.status`, `session.stop`, `breakpoints.create|delete|list` (execution breakpoints use segmented addresses; `once=True` available; memory-change breakpoints need heavy debug), `execution.continue|pause|step|wait`, `state.get_registers`, `memory.read`, `memory.write` (requires `expected_sha256` of the current bytes), `trace.start|read|stop` (CPU trace, heavy debug), `debugger.execute_command` (read-only diagnostics).
- Target name and arguments may contain only letters, digits, `.`, `_`, `-`. Example: `agent.start("COUNT.EXE", ["NOPICS"], mount_path=workdir)`.
- Observation is only possible while stopped. Long-running operations return an operation id; poll with `wait`.
- Do not auto-retry side-effecting requests.

Typical session:

```python
with AgentClient.from_config(cfg) as agent:
    s = agent.start("COUNT.EXE", ["NOPICS"], mount_path=workdir)
    L = int(agent.get_registers(s.id).segments["cs"], 16)      # load segment (header CS is 0)
    DS = L + 0x3218                                            # DGROUP after startup
    bp = agent.create_execution_breakpoint(s.id, segment=f"0x{L+SEG_PLAYLOOP:04X}", offset=f"0x{OFF_TRACEPOINT:04X}")
    for frame in range(600):
        op = agent.continue_(s.id); r = agent.wait(s.id, op.id, timeout_ms=5000)
        while r.running: r = agent.wait(s.id, op.id, timeout_ms=5000)
        blob = agent.read_memory(s.id, MemoryAddress.segmented(f"0x{DS:04X}", f"0x{OFF_GAMESTATE:04X}"), SIZE_GAMESTATE)
        ...  # decode per trace schema, append JSONL
```

### 6.4 Control server (raw debugger commands)

- Enable with `mcp_server=<port>` in the `[dosbox]` section. DOSBox-X connects **as a TCP client** to `127.0.0.1:<port>`, so our tool must listen first.
- Line protocol: send `REQ <id> PING|BREAK|EXEC <debugger command>`; receive `BEGIN <id> OK|ERR`, output lines, `END <id>`. `RUN`, `RUNWATCH` and `VRT` resume and answer immediately.
- Useful debugger commands: `BP seg:off`, `BPINT nn [ah]`, `BPM seg:off`, `BPLIST`, `BPDEL`, `MEMDUMPBIN seg:off len file`, `MEMDUMP`, `SM seg:off val…`, `SR reg val`, `EV`, `IV`/`SV`/`LV` (named variables), `LOG n`, `HEAVYLOG`, `VGA …`, `DOS MCBS|XMS|EMS`, `VRT` (run to next vertical retrace).

### 6.5 Determinism and inputs

- Do not rely on DOSBox-X being deterministic across runs. Instead record the original's per-frame `tics` and `TicCmd`, and replay them into OBCR (DESIGN §9.4).
- Emulation is frozen while stopped at a breakpoint, including the PIT, so per-frame stops do not inject extra tics.
- Input options: (a) the owner plays live in the DOSBox-X window while the oracle records (the game stutters, but the recorded data is valid); (b) scripted key presses via `input.key` (added in M0.11); (c) poking the game's keyboard state array (`Keyboard[]`, located in M2.R5) at a breakpoint.

### 6.6 Frame and palette capture

The game draws in unchained VGA mode (Wolf3D does; confirm for OBC). Plain `memory.read` of `0xA0000` returns one plane at best. Use `video.capture` (added in M0.11), which returns the visible 320×200 indexed frame plus the DAC palette. Until it exists, golden frames need the owner's screenshot hotkey; do not invent other capture paths.

### 6.7 Scenario setup

Prefer the game's own mechanisms (`GOLEVEL` parameter, debug mode), then memory pokes at a stable breakpoint (player x, y, angle; weapons; health; disabling actors by setting them inactive). Every scenario is a script under `tools/obc-oracle/scenarios/` so it can be re-run.

### 6.8 Modifying DOSBox-X

- Work on local branch `obc-tools` in `..\dosbox-x`. Never push it upstream: DOSBox-X does not accept AI-generated code (`..\dosbox-x\AI SUGGESTED TODO.md`).
- Follow the existing agent architecture under `src\agent\` (bridge, debugger, parser, protocol, server, trace, transport) and its tests.
- Rebuild and re-run the agent tests in `tests\agent` after changes.

---

## 7. Coding standards

### 7.1 General

- Rust 2024, stable toolchain from `rust-toolchain.toml`. `cargo fmt`; clippy clean with `-D warnings`.
- `unsafe` is denied workspace-wide. Exceptions need an ADR and a `// SAFETY:` comment per block (expected only in `obc-platform`, and only if the `sdl3` crate leaves no safe option).
- No `static mut`, no global mutable state, no `lazy_static`-style singletons for game state. Pass `&mut World` explicitly.
- No `unwrap`/`expect` on data-derived values outside tests. `expect` with a message is allowed for true programming invariants.
- Public items have doc comments. Modules start with a `//!` summary that names the original source files they port.
- Prefer plain data plus functions over trait hierarchies. Actors use enums, state tables and `fn` pointers, not trait objects or ECS.
- Dependencies: add to `[workspace.dependencies]` only; justify each new crate in the commit message; must pass `cargo deny`.

### 7.2 Porting original code

- **Structure-preserving first.** Port a function so its control flow mirrors the original (same branches, same order of RNG calls, same early returns). Refactor for readability only after parity tests exist.
- Rust names are `snake_case` versions of the originals, with `#[doc(alias = "OriginalName")]` so searches find them: `#[doc(alias = "T_Chase")] pub fn t_chase(...)`.
- Every ported function's doc comment cites its origin, OBC address and deltas:

  ```rust
  /// Chase behavior for most enemies.
  ///
  /// Port of `T_Chase` (`WOLFSRC/WL_ACT2.C:3069`). OBC: `img+0x?????` (CONFIRMED).
  /// Deltas: D-0NN (dodge chance), D-0NN (allies as targets).   <- illustrative ids
  #[doc(alias = "T_Chase")]
  pub fn t_chase(world: &mut World, actor: ActorHandle) { ... }
  ```

- Mirror integer widths (`int` → `i16`, `unsigned` → `u16`, `long` → `i32`, `byte` → `u8`, `fixed` → `Fixed`). Where the original can overflow, use `wrapping_*` and comment why. Where it truncates (assigning a `long` to an `int`), cast explicitly with `as i16` and comment.
- Constants keep their original names in `SCREAMING_SNAKE_CASE` with a citation.
- Tables transcribed from `COUNT.EXE` (state tables, speed tables) cite their address. Bulk art-like data (palettes, fonts, pictures) is **read from the user's files at run time**, never transcribed.
- Mark incomplete ports: `// DELTA-UNCHECKED`, `// HYPOTHESIS(Q-###)`, `// TODO(M7.3): ...`. CI counts them; they block "done".

### 7.3 Simulation rules (enforced in review)

- No `f32`/`f64`, no `std::time`, no I/O, no threads, no `HashMap`/`HashSet` iteration affecting results (use `BTreeMap` or vectors), no randomness except `world.rng`.
- Simulation code returns events; it never calls audio, render or UI.
- Every `world.rng` call site has a comment naming its original counterpart.

### 7.4 Rendering rules

- Classic path: integer math only, identical to the original. Hot loops may be optimized (slices, iterators, unchecked indexing is not allowed) but must keep golden tests green.
- High-res path: may use different math but must not share mutable state with the Classic path.

### 7.5 Performance

Measure before optimizing (`cargo bench` with criterion in `benches/`, or the `--headless` timing output). Do not trade parity for speed.

---

## 8. Testing standards

| Kind | Rule |
|---|---|
| Unit tests | Every parser, math helper, and ported function with non-trivial branches. Put them next to the code. |
| Property and fuzz | Every parser. Inputs: random bytes, truncations, mutated real files. Property: no panic, bounded allocation. |
| Data-gated tests | Use `obc_testdata::require!()`. Never hard-code paths. Expected values from copyrighted data are stored as hashes or counts. |
| Golden frames | `tests/render_golden`. Reference captures live in `.local/captures/`; tests compare exact indexed pixels and write a diff PNG to `.local/test-output/` on failure. |
| Trace diffs | Every behavior task adds or extends at least one scenario in the trace corpus and records its id in STATUS.md. |
| Snapshots (`insta`) | For text and structured outputs (exports, map dumps). Updating snapshots requires explaining why in the commit message. |
| Determinism | A test runs the same replay twice and on two thread counts and compares trace hashes. |

Agents can open PNG files with the Read tool. Use it to sanity-check exports and captures, but never as the only evidence where an exact comparison exists.

---

## 9. Documentation duties

After every task, update what applies:

| Changed | Update |
|---|---|
| Any task state | [STATUS.md](STATUS.md) row and log |
| Learned something about the original | `docs/re/*` and `re/*` (§5.6) |
| Implemented a delta | Set the `D-###` status to `IMPLEMENTED (<commit>)`, then `VERIFIED (<evidence>)` |
| Resolved an open question | Mark `Q-###` resolved with the answer and evidence; link the D, K or format entry |
| Architecture or dependency choice | New ADR in [DECISIONS.md](DECISIONS.md) |
| User-visible behavior or flags | README (and the manual, later) |
| New tool or workflow | This guide or `tools/*/README.md` |

Docs describe the current state. Do not keep "previously we thought X" narratives except in ADRs and resolved-question records.

---

## 10. Git and collaboration

### 10.1 Branches and commits

- Branch per task: `m<milestone>/<task-id>-<slug>`, for example `m5/m5.4-doors`.
- Commit messages: Conventional Commits with the task id: `feat(sim): port MoveDoors and door timing (M5.4)`. Types: `feat`, `fix`, `refactor`, `test`, `docs`, `re` (knowledge-base-only changes), `tools`, `build`, `chore`.
- Keep commits small and buildable. Do not mix reformatting with behavior changes.

### 10.2 Owner policy

The owner sets the commit, merge and push policy in [CLAUDE.md](../CLAUDE.md). Default until changed: agents commit completed, verified work to their task branch; merging to `main` and any push to a remote require the owner's go-ahead.

### 10.3 Never commit

- Anything from `..\OBC` or derived from it in bulk: data files, extracted images or sounds, map dumps, palette bytes, text dumps, save files.
- Decompiled or disassembled listings of `COUNT.EXE` (short excerpts of a few instructions in a doc to explain a finding are acceptable; whole functions are not).
- Oracle traces and captures (they contain game data). They live in `.local/`; only metadata (`re/captures.toml`) and hashes are committed.
- Secrets, machine-specific absolute paths in code (docs may mention the owner's paths).

`cargo xtask lint` scans for data signatures; it is a safety net, not permission.

---

## 11. Definition of done

A task is **Done** when all of these hold:

- [ ] Acceptance check from MILESTONES.md passes, and its evidence (commands, test names, trace or capture ids, results) is recorded in STATUS.md.
- [ ] `cargo xtask ci` passes; `cargo xtask test-data` passes when data is available.
- [ ] Delta check done for every ported function; no `DELTA-UNCHECKED` or `HYPOTHESIS` markers remain in code the task touched.
- [ ] Knowledge base updated (SYMBOLS, DELTA_SPEC, DATA_FORMATS, OPEN_QUESTIONS as applicable).
- [ ] No original data or decompiled code added to the repository.
- [ ] Commits on the task branch with conventional messages.
- [ ] STATUS.md row set to `Done`, log entry added, follow-ups recorded as new tasks.

A milestone is **Done** when every task is done, every milestone acceptance criterion is checked with evidence, and the owner has signed off where the criteria call for it.

---

## 12. When to stop and ask the owner

Ask (and record the question in STATUS.md "Blockers") when:

- A decision is legal or licensing-related (ADR-006, third-party code, distribution).
- Data is missing that only the owner can provide (CD or Steam release for rendered sequences and CD audio, other releases for manifests).
- The original's behavior cannot be determined and two reasonable choices would play differently.
- Work would modify anything outside `OBCR\` other than DOSBox-X's `obc-tools` branch and build outputs.
- An action is destructive or hard to undo (deleting branches, rewriting history, deleting captures or traces).
- Scope changes: adding features not in the original or in DESIGN.md, dropping a planned feature, changing a parity target.
- Anything costs money or touches external services (CI minutes, signing certificates, publishing).

Do not ask about things you can determine yourself from the code, the data, the original or these documents.

---

## 13. Recipes

### R1 — Port a Wolf3D function with OBC deltas

1. Find the Wolf3D function (`WOLFSRC/…`) and its callers and callees.
2. Find its `COUNT.EXE` counterpart (SYMBOLS CSV, Wolf3D seeding, call graph from a known neighbor, or a string or constant search).
3. Read both side by side. List differences in constants, branches, calls, struct offsets, RNG calls.
4. Record `D-###` entries (or "no delta") and update the symbol's confidence.
5. Write the Rust port (structure-preserving), citing origin, address and deltas.
6. Add unit tests for edge cases and a trace scenario that exercises it.
7. Run the trace diff; investigate any divergence (R6) before calling it done.

### R2 — Investigate an unknown map code

1. `obc-tool maps find --plane 1 --value 277` lists every map and tile where it occurs.
2. Render those maps (`maps render`) and look at the surroundings.
3. Find the code in the `ScanInfoPlane` equivalent (switch on tile value) and follow the spawn function.
4. Confirm in DOSBox-X: warp to the map, inspect the spawned object at that tile.
5. Record in `re/LEVEL_CODES.md` and the code table; resolve the related `Q-###`.

### R3 — Find a function from a string

1. Look up the string in [STRINGS.md](re/STRINGS.md) (file offset and DS offset).
2. Search scalars in Ghidra for the DS offset; inspect each use.
3. Name the function (`LIKELY`), read it, upgrade to `CONFIRMED` when understood.

### R4 — Verify a rendering change

1. `cargo test -p obc-render` (unit tests on table math).
2. `cargo xtask golden` (all golden frames; exact match required).
3. For new goldens: create a scenario script, capture with the oracle, register in `re/captures.toml`, add the test.

### R5 — Capture a trace scenario

1. Write `tools/obc-oracle/scenarios/<id>.py` (map, setup pokes, input script or "owner plays").
2. Run it: produces `.local/traces/<id>.jsonl` and registers metadata in `re/captures.toml`.
3. Add the id to the regression corpus list (`tests/trace_corpus.toml`).

### R6 — Diagnose a trace divergence

1. `obc-tool trace diff` gives the first divergent frame and field.
2. Check the previous frame's RNG index: an RNG offset usually means an extra or missing `US_RndT` call. List every RNG call site executed in that frame on both sides.
3. Check think order (object list order) and the `tics` value used.
4. If needed, set a breakpoint in the original at the function that writes the divergent field and step through that frame.
5. Fix, then add a unit test or scenario that pins the behavior.

### R7 — Add or decode a data format

1. Gather evidence: loader function in `COUNT.EXE`, file statistics, hex inspection.
2. Write the spec in DATA_FORMATS.md first (with confidence tags).
3. Implement the parser with property tests and data-gated golden counts.
4. Add an `obc-tool` exporter so others can inspect the data.

### R8 — Add an actor class

1. RE: spawn codes, state table address, think and action functions, hit points, speeds, sounds, drops.
2. Transcribe the state table into `crates/obc-sim/src/actors/<class>.rs` with citations.
3. Port think and action functions (R1 for each).
4. Name the sprites and sounds involved in SYMBOLS.
5. Scenario traces: idle, patrol, ambush, death.

---

## 14. Anti-patterns

- "Wolf3D does it this way, so OBC does too." Check.
- Writing code before the delta check because "it is probably the same".
- Fixing a trace divergence by adjusting values until they match, without understanding the cause.
- Updating golden files or snapshots to make a failing test pass without explaining why the new output is right.
- Adding floating point to "simplify" simulation math.
- Converting the original's blocking loops into blocking Rust loops (`while !key_pressed {}`); use screens and sequences.
- Introducing an ECS, async runtime, scripting language or plugin system.
- Big-bang changes spanning several subsystems.
- Leaving findings only in the chat or in a commit message.
- Committing captures, traces, exports or decompiled code.
- Running the game from `..\OBC` directly.
- Editing reference trees (`..\OBC`, `..\wolf3d`, `..\ECWolf`), or DOSBox-X outside the `obc-tools` branch.

---

## 15. Sub-agent briefs

When delegating, give the sub-agent everything it needs; it starts with no context. Templates:

### 15.1 RE investigator

```
Goal: determine <question> about Operation Body Count's COUNT.EXE (Q-### in OBCR/docs/re/OPEN_QUESTIONS.md).
Read first: OBCR/CLAUDE.md, OBCR/docs/AGENT_GUIDE.md §4–§6, OBCR/docs/re/SYMBOLS.md, <specific files>.
Known so far: <facts with confidence tags>. Already ruled out: <...>.
Resources: Ghidra exports in OBCR/.local/ghidra/export/ (read-only; do not open the Ghidra project for writing),
Wolf3D source in P:\msdos\OBC\wolf3d\WOLFSRC, oracle in OBCR/tools/obc-oracle (own runtime dir, pipe name <unique>).
Deliver: answer with confidence tag and citations; proposed updates to DELTA_SPEC/SYMBOLS/OPEN_QUESTIONS as text.
Do not: edit Rust code, commit, or modify reference directories.
```

### 15.2 Implementer

```
Task: <M#.#> from OBCR/docs/MILESTONES.md — <title>. Branch: <branch> in worktree <path>.
Read first: OBCR/CLAUDE.md, AGENT_GUIDE.md §7–§11, DESIGN.md §<sections>, re/DELTA_SPEC.md entries <D-...>.
Inputs: <symbols, struct layouts, deltas>. Acceptance: <exact check from MILESTONES>.
Constraints: only touch crates <...>; sim purity rules apply; cite origins in doc comments.
Deliver: code plus tests, passing `cargo xtask ci`; STATUS.md updated with evidence; summary of anything unresolved.
```

### 15.3 Verifier

```
Verify task <M#.#> on branch <branch>. Do not trust the implementer's summary; check independently.
Run: cargo xtask ci; cargo xtask test-data; <trace diff / golden commands>.
Review: delta check completeness (compare code with COUNT.EXE at <addresses> and WOLFSRC/<file>), sim purity, citations, docs updated.
Report: pass/fail per Definition of Done item with evidence; list concrete defects with file:line.
```
