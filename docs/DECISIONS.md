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

- **Status**: Superseded by ADR-017 (2026-11-05): the M13 RE found the original is not lockstep
- **Decision**: Peers exchange per-tic inputs over UDP; each runs the full simulation; periodic state hashes detect desyncs. IPX becomes LAN discovery plus direct IP; Modem becomes a direct two-player connection.
- **Alternatives**: Client-server with state replication (more complex, unnecessary given a deterministic simulation).

## ADR-010 — OPL emulation behind a trait, starting with Nuked-OPL3

- **Status**: Accepted (2026-11-02), amended
- **Decision**: Use the pure-Rust `nuked-opl3` crate (a port of Nuked-OPL3, LGPL-2.1+), which avoids a C toolchain dependency. Never use MAME `fmopl`.

## ADR-011 — Reverse-engineering state as text under version control

- **Status**: Accepted (2026-11-02)
- **Decision**: Names, types, comments and trace schemas live in `re/` as text (CSV, C header, Markdown). The Ghidra project in `.local/` is a disposable cache rebuilt by scripts.
- **Consequences**: Reviewable diffs of RE progress; multiple agents can contribute without sharing a binary project.

## ADR-012 — DOSBox-X JSON-RPC agent as the oracle interface

- **Status**: Accepted (2026-11-02)
- **Context**: The local DOSBox-X tree includes a JSON-RPC debugger agent (named pipe, Python client) and a line-based control server.
- **Decision**: `tools/obc-oracle` drives the original through the JSON-RPC agent. Missing capabilities (key input, frame capture) are added to DOSBox-X on a local `obc-tools` branch that is never sent upstream.

## ADR-013 — Classic frames are two tics

- **Status**: Accepted (2026-11-02)
- **Context**: The original's `PlayLoop` advances by `tics` measured by `CalcTics`; frame-rate-dependent logic (walk bob, animated walls, the event box, the 301-frame sparkle) runs once per frame, not per tic.
- **Decision**: OBCR runs one original frame every two 70 Hz tics (`World::frame(cmd, 2)`, 35 Hz). Replays and the oracle may feed recorded `tics` values instead.
- **Consequences**: Deterministic and close to the original on a typical machine; per-frame logic keeps its rate. Enhanced presentation interpolates between frames (ADR-015) without touching the simulation.

## ADR-014 — Modern input extends the tic command, not the simulation rules

- **Status**: Accepted (2026-11-02)
- **Decision**: The modern profile and gamepads produce `TicCmd`s with two extra fields: `strafe` (sideways thrust while turning, same scale as the strafe button) and `turn` (turning added after `PollControls`' ±100·tics clamp, so mouse look is not speed-limited). The original profile never sets them. Everything downstream of the command is the original logic, so replays and lockstep stay deterministic.
- **Consequences**: A modern-profile player can strafe and turn at once and turn faster than the original allowed; this is an input capability, recorded in the command stream.

## ADR-015 — Enhanced frame interpolation is presentation only

- **Status**: Accepted (2026-11-02)
- **Decision**: With SMOOTH MOTION on (Enhanced only), the renderer blends the player pose and moving actors between the last two simulation frames at the display rate, using a fine view angle (`FINEANGLES` units) and float sine/cosine off whole degrees. Gameplay visibility, hit tests and everything else read the simulation state.
- **Consequences**: Up to one frame (≈29 ms) of display latency; Classic keeps integer angles and the original tables.

## ADR-016 — Blasts never leave a floor unfinishable

- **Status**: Accepted (2026-11-05), at the owner's request
- **Context**: In the original, `ExplosionTileEffects` spares only exit tile 0x71. A grenade can scorch or remove the stairs (0x72) and elevator (0x73) exits, and it scorches nearby doors into solid wall 0x4D. Either can leave the exit unreachable. A door destroyed by a direct hit keeps its door record, so if it was open it later "closes" into an invisible barrier.
- **Decision**: An exception to ADR-008, in Classic and Enhanced alike: blasts spare all three exit tiles and every door except the one hit, and `CloseDoor` refuses once the door's tile is gone.
- **Consequences**: Floors stay finishable after any blast. Recorded traces that blow up an exit or a nearby door diverge from the original at that point.

## ADR-017 — Multiplayer reproduces the original's state broadcast, over UDP

- **Status**: Accepted (2026-11-05); supersedes ADR-009
- **Context**: The M13 RE ([re/NETWORK.md](re/NETWORK.md), [re/NETCOMMS.md](re/NETCOMMS.md)) shows the original is not lockstep. Every machine runs its own copy of the level and owns its player; once per frame it sends a 24-byte state packet with a one-shot event; other players are puppet actors; the victim's machine decides damage with the enemy-fire rules. The single-player port already contains the original's `difficulty >= 4` branches written for this model.
- **Decision**:
  - `obc-sim::net` ports segment 358f's rules. The transport stays outside the simulation: the platform fills `NetGame::inbox` before a frame and drains `NetGame::outbox` after it, so a frame is still a pure function of its inputs (now including received packets).
  - New crate `obc-net` carries the packets over UDP. The IPX socket number becomes the UDP port (default 0x4003 = 16387). LAN discovery uses broadcast, like the original. Internet play works by joining an address, and peers share their peer lists. Each datagram carries the sender's last three packets with sequence numbers, so single losses are recovered and duplicates are dropped.
  - Game type M (modem/serial) becomes a two-player direct UDP connection with the original's rules for type 6.
- **OBCR changes to the original's behaviour** (all at the transport edge):
  - Packets are queued per player instead of overwriting each other within a frame, so no event is lost.
  - Puppets are found by slot, not by actor-list position.
  - Ids are 1–254, chosen to avoid ids already heard, and a collision is resolved by re-rolling. The original used the last byte of the IPX node address and dropped colliding players' packets.
  - A player who quits or times out is hidden instead of freezing in place.
- **Alternatives**: Lockstep (ADR-009). It would need a multi-player rewrite of the simulation, add input delay to every movement, and would no longer play like the original.
- **Consequences**: There is no global consistency to check, so there are no desync hashes. As in the original, items, fires and level state can differ between machines. Network games cannot be saved.

## ADR-018 — Ally kills count toward the exit quota

- **Status**: Accepted (2026-11-05), at the owner's request
- **Context**: *KillActor* (`1e43:161c`) does not increment `killcount` while `allyfiring` is set (ALLIES.md §4.4). The exit needs `killcount >= killtotal / 2` (*TryExit*), so a squad that does most of the killing can leave the floor short, and the HUD kill count and lamp ignore the squad's work.
- **Decision**: Another exception to ADR-008 in Classic and Enhanced alike, in the spirit of ADR-016: every enemy death counts in `killcount`, whoever caused it. Allies still earn no score (`GivePoints` keeps its `allyfiring` check), and killing an ally still does not count.
- **Consequences**: The exit opens, and the kill number and lamp advance, when allies kill. Recorded traces with an ally kill diverge from the original in `killcount` and the unread kill timer (`DS:0xCD87`) from that point.

## ADR-019 — Smooth floors and ceilings (Enhanced)

- **Status**: Accepted (2026-11-05), at the owner's request
- **Context**: The original draws each visible tile's floor and ceiling as a separate quad, slightly smaller than the tile (`d = 0x7600`), with the texture laid in screen space, one texel per pixel, restarting at each quad; one shade per tile; the background gradient shows between quads as grout lines ([re/RENDERER.md](re/RENDERER.md) §3, CONFIRMED). In play this reads as seams and stripes. The owner asked for smooth floors.
- **Decision**: An Enhanced option, FLOORS = SMOOTH (default) or ORIGINAL. SMOOTH runs after the wall pass: for each screen column, every pixel below the wall (and its mirror above) is cast to the map, so the texture is in perspective, 64 texels per tile, continuous across tiles, with no gaps. The floor sets, floor codes and shading rule (`texel − shade`, clamped to the page's ramp base) are the original's; the shade follows each row's distance through a continuous version of the floor shade table, blending the two nearest steps. Classic and FLOORS = ORIGINAL keep the original quads.
- **Consequences**: Enhanced floors no longer look like the original's. Rows just below very distant walls (drawn at least 8 tall by `CalcHeight`) take the nearest point on the map. The framebuffer can blend two palette indices per pixel (see ADR-020).

## ADR-020 — xBRZ texture upscaling (Enhanced)

- **Status**: Accepted (2026-11-05), at the owner's request
- **Context**: The owner asked for xBRZ 2×–6× upscaling. The renderer is palette-indexed: distance shading subtracts from palette indices, and damage flashes, fades and the generator blackout change the palette. An RGB upscale would lose all of that.
- **Decision**:
  - `obc-render::xbrz` ports xBRZ 1.6 (Zenju; the copy in `dosbox-x/src/libs/xBRZ`) to palette images. Colour distance comes from `gamepal`; each output texel is a `Mix` of at most two palette indices with a weight; transparency (index 0xFF, or a sprite's mask) is resolved at 50 %. Walls repeat sideways and floors both ways when scaled, so tiles join without seams.
  - The `Framebuffer` gains a lazily allocated plane with a second index and weight per pixel; plain writes clear it. Presentation blends the two colours through the palette of the moment, so shading and every palette effect still apply.
  - Enhanced option TEXTURES = ORIGINAL or XBRZ 2X–6X (default 4X) for walls, doors, floors, ceilings, sprites, the hand weapon and splats. Each texel's screen span shows its n×n sub-texels, so geometry and clipping are unchanged; sprites are clipped column by column. Textures are built on first use, and a new floor builds its walls, flats, statics, the actors' current frames and the hand weapons up front on all cores.
  - The 320×200 2D layer can be upscaled too, at the presentation scale (at most 6×, then nearest), cached while the layer and palette stay the same; a recompute runs on all cores. Two Enhanced options: XBRZ MENUS (default on) for menus, title and intro pictures, animated sequences, help and score pages; XBRZ STATUS BAR (default off) for the in-game status bar and overlays, whose small text xBRZ distorts.
  - Classic never uses either.
- **Licence**: xBRZ is distributed under the GPL version 3. `xbrz.rs` is a derivative and carries that licence; it is compatible with the project's GPL-3.0-or-later (ADR-006), and a binary that includes it is distributed under GPL-3.0.
- **Consequences**: About 0.2 MB per texture at 4× and 0.4 MB at 6×, for the textures a session has shown. Distance shading on upscaled sprites and walls is still one step per post, as in the original.

