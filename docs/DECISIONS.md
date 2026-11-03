# Architecture decision records

One entry per significant decision. Status: `Proposed` (awaiting the owner), `Accepted`, `Superseded by ADR-NNN`. New ADRs go at the end; never rewrite an accepted ADR, supersede it.

Template:

```markdown
## ADR-NNN — <title>
- **Status**: Proposed | Accepted (<date>) | Superseded by ADR-NNN
- **Context**: <forces at play>
- **Decision**: <what we do>
- **Alternatives**: <considered, and why not>
- **Consequences**: <costs, follow-ups>
```

---

## ADR-001 — Rust and SDL3

- **Status**: Accepted (2026-11-02; the owner offered Rust + Bevy, Rust + SDL3, or another stack, and delegated the choice)
- **Context**: The original is a tic-based, single-threaded software renderer with global state. Parity requires exact integer math and full control over frame timing, audio timing and palette output. Agents must be able to work reliably with stable, well-documented APIs.
- **Decision**: Rust (stable, edition 2024) with SDL3 via the `sdl3` crate (built from source, statically linked) for window, input, gamepad, audio device and timing. All rendering is our own software code into an indexed framebuffer; SDL only presents it.
- **Alternatives**: **Bevy**: its ECS, scheduler and GPU render graph fight a deterministic tic loop and software framebuffer; long compile times and frequent API changes reduce agent reliability. **winit + pixels + cpal + gilrs**: viable pure-Rust stack, but four integrations instead of one; kept as the fallback if SDL3 bindings block us. **macroquad**: too little control over audio and windowing. **C++ (fork ECWolf)**: see ADR-002.
- **Consequences**: A C toolchain and CMake are needed to build SDL3 (both installed). `obc-platform` stays thin so the fallback remains cheap.

## ADR-002 — Port from Wolf3D plus reverse engineering; ECWolf as reference only

- **Status**: Accepted (2026-11-02)
- **Context**: ECWolf already lists OBC as a target and reads `MAPTEMP`. But it is a ZDoom-style engine (DECORATE actors, MAPINFO, translation layers) whose behavior intentionally differs from the originals in places, and OBC's deltas (allies, projectiles, new map semantics, multiplayer) would need deep native changes.
- **Decision**: OBCR's game logic is a structure-preserving port of the Wolf3D source with OBC deltas from reverse engineering. ECWolf is consulted for modern solutions (format readers, high-res projection, aspect correction, audio playback, Steam detection), not for structure or OBC behavior.
- **Alternatives**: Extend ECWolf with OBC support: faster start, but parity would be hard to prove and the result would not be Rust.
- **Consequences**: More reverse engineering up front; much easier differential verification against the original.

## ADR-003 — Fixed 70 Hz simulation with tics as an input

- **Status**: Accepted (2026-11-02)
- **Context**: The original advances by a variable number of tics per rendered frame. That makes behavior depend on frame rate and makes traces hard to reproduce.
- **Decision**: `obc-sim::step(tics)` takes `tics` explicitly. Normal play calls it once per 1/70 s with `tics = 1`. Differential tests replay the exact per-frame `tics` sequence recorded from the original.
- **Alternatives**: Reproduce variable tics at run time (frame-rate-dependent gameplay); fixed step with larger tics (coarser than the original's best case).
- **Consequences**: Normal play corresponds to the original running at a full 70 fps. Formulas that use `tics` remain correct by construction.

## ADR-004 — Software renderer, Classic pixel parity, separate high-res path

- **Status**: Accepted (2026-11-02)
- **Decision**: Classic renders 320×200 indexed pixels with the original algorithms and must match captures exactly. Enhanced uses a separate high-res path that may not alter Classic output.
- **Consequences**: Two renderer paths to maintain; Classic golden tests guard against regressions.

## ADR-005 — User-supplied data, never committed

- **Status**: Accepted (2026-11-02)
- **Decision**: OBCR reads the user's original files at run time, identified by SHA-256 manifests per release. No original data, extracted assets, traces, captures or decompiled code are committed. Bulk art-like data embedded in `COUNT.EXE` (palette and similar) is read from the user's executable at run time.
- **Consequences**: Data-gated tests; CI without data runs the non-gated suite only.

## ADR-006 — License: GPL-3.0-or-later

- **Status**: Accepted (2026-11-02, owner decision)
- **Context**: OBCR ports logic from the Wolf3D source, which id Software later released under the GPL. ECWolf's id-derived files are "GPL-2+ or EULA" and its newer files are BSD-3-Clause (`ECWolf/docs/copyright`). ECWolf's `src/mame/` files carry the non-GPL MAME license.
- **Decision**: License OBCR's code as **GPL-3.0-or-later** (`LICENSE` holds the GPL-3.0 text). Sources we draw from must be GPL-3.0-compatible: GPL-2.0-or-later, GPL-3.0, LGPL-2.1-or-later, LGPL-3.0, MIT, BSD, zlib, Apache-2.0 are fine; GPL-2.0-only and MAME-licensed code are not.
- **Consequences**: `cargo deny` allows only GPL-3.0-compatible licenses; Nuked-OPL3 (LGPL-2.1+) and SDL3 (zlib) are compatible.

## ADR-007 — Pure simulation with events out

- **Status**: Accepted (2026-11-02)
- **Decision**: `obc-sim` has no I/O, clock, floating point or nondeterministic iteration; it communicates outward only through `SimEvent`s. Renderer, audio and UI read state or consume events.
- **Consequences**: Headless testing, replays, lockstep multiplayer and save/load equivalence all follow from this.

## ADR-008 — Fidelity presets and quirk policy

- **Status**: Accepted (2026-11-02)
- **Decision**: Classic is the default and preserves gameplay quirks; Enhanced changes presentation and comfort only. Gameplay-changing fixes are individual opt-in flags (`K-###`), stored in saves and the multiplayer handshake. Crash bugs are always fixed safely.

## ADR-009 — Multiplayer by deterministic lockstep over UDP

- **Status**: Accepted (2026-11-02), pending M13 RE of the original model
- **Decision**: Peers exchange per-tic inputs over UDP; each runs the full simulation; periodic state hashes detect desyncs. IPX becomes LAN discovery plus direct IP; Modem becomes a direct two-player connection.
- **Alternatives**: Client-server with state replication (more complex, unnecessary given a deterministic simulation).

## ADR-010 — OPL emulation behind a trait, starting with Nuked-OPL3

- **Status**: Proposed (decide in M6.1)
- **Decision (proposed)**: Integrate Nuked-OPL3 (C, LGPL-2.1+) through the `cc` crate behind an `Opl` trait. A pure-Rust port may replace it later. Never use MAME `fmopl`.

## ADR-011 — Reverse-engineering state as text under version control

- **Status**: Accepted (2026-11-02)
- **Decision**: Names, types, comments and trace schemas live in `re/` as text (CSV, C header, Markdown). The Ghidra project in `.local/` is a disposable cache rebuilt by scripts.
- **Consequences**: Reviewable diffs of RE progress; multiple agents can contribute without sharing a binary project.

## ADR-012 — DOSBox-X JSON-RPC agent as the oracle interface

- **Status**: Accepted (2026-11-02)
- **Context**: The local DOSBox-X tree includes a JSON-RPC debugger agent (named pipe, Python client) and a line-based control server.
- **Decision**: `tools/obc-oracle` drives the original through the JSON-RPC agent. Missing capabilities (key input, frame capture) are added to DOSBox-X on a local `obc-tools` branch that is never sent upstream.
