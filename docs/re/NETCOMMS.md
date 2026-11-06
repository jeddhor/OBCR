# Multiplayer transport and setup (IPX, serial, modem)

How `COUNT.EXE` (release `OBC-A`) sets up and carries a multiplayer game: the setup screens, the IPX layer, the serial/modem driver and the packet model as seen by the transport. The game rules (respawn, damage, kills, teams, puppets) are in [NETWORK.md](NETWORK.md); this document only repeats what the transport needs.

Written 2026-11-05 from static analysis of the Ghidra export (`.local/ghidra/export`) plus `tools/re/disasm16.py` for code Ghidra did not disassemble or decompiled wrongly. No DOSBox-X run was made.

Conventions (as in GAMEFLOW.md):

- Addresses are `ghidra SSSS:OOOO`, base segment 0x1000. DGROUP data is `DS:0xNNNN` (file offset = DS + 0x35580). Far data segments are given in Ghidra numbering (`4139:0000` and so on).
- **CONFIRMED**: disassembly read instruction by instruction, or a data value read from the file. **LIKELY**: decompiler output read and consistent, not traced instruction by instruction. **HYPOTHESIS**: inference. **UNKNOWN**: not determined.
- `Keyboard[]` is the word array at DS:0x8F22 (`DS:0x8F22 + 2·scancode`). Menu input uses `ReadAnyControl` (`1618:1aa5`): `button0`, `button1`, `dir` (0 = up, 4 = down).

## 0. Where the code is (correction)

Segment `377a` is **not** network code. It is the Borland C++ 8087 floating-point **emulator** (§1). The multiplayer code is split as follows. CONFIRMED (string references, call graph and disassembly).

| Segment | Contents |
|---|---|
| `358f` | IPX layer (`000f`–`04d7`), setup screen `NetSetup` (`0959`) and its text helpers, `NetFrame` packet send (`0eea`), shutdown (`1e19`), plus the game rules (NETWORK.md) |
| `33bb` | Serial/modem driver: UART set-up, interrupt handler, ring buffers, packet framing, `HARDWARE SETUP` screen, modem init, `MODEM DIAL BOX`, modem command keys |
| `377a` | Borland floating-point emulator internals (arithmetic on the emulated register stack, compare, BCD conversion, exception raise) |
| `39f2` | Borland emulator / FPU fix-up entry (`e087_Entry`, the INT 34h–3Dh handler) |

The two "calls" from game code into `377a` that Ghidra lists (`358f:18e7` → `377a:2714`, `33bb:1abe` → `377a:09d9`/`377a:2714`) are decompiler artefacts: both functions end in a jump table that Ghidra followed into the wrong bytes. The real tables are at `358f:1961` (targets `1919`, `192b`, `193d`, `194f`) and `33bb:1d45` (targets `1b4d`, `1bcc`, `1c4a`, `1cbf`, `1d19`). CONFIRMED (disassembly). No network code calls the emulator.

## 1. Segment `377a`: Borland floating-point emulator

Evidence: every routine works on a 12-byte internal real (64-bit mantissa in four words, a signed exponent word where 0x4001 marks infinity/NaN and 0xC001 marks zero, a sign byte); results go through an exception routine that keeps x87-style status and control words at DS:0x3D30 and DS:0x3D32 (Ghidra already labels them `emu`); the emulated register stack pointer is DS:0x3D3C (12-byte entries); the sibling segment `39f2` holds `e087_Entry`, the Borland fix-up handler that rewrites `INT 34h`–`3Dh` into `WAIT` + ESC opcodes when a real FPU is present. CONFIRMED (disassembly of `377a:2714`, `377a:0e6b`; decompiler for the rest).

The game reaches this code only through the emulator interrupts (INT 34h–3Dh) that Borland compiles for floating-point statements; Ghidra did not disassemble the dispatcher, so the functions below have no callers outside `377a`. Known floating-point users in the game: `Died`'s turn towards the killer (`atan2`, GAMEFLOW §5.4) and, HYPOTHESIS, the start-up table builders inherited from Wolf3D. **Nothing here needs porting**: the port implements the callers' results directly.

| Ghidra | Proposed name | Purpose | Confidence |
|---|---|---|---|
| 377a:0012 | `emu_PickSpecialOperand` | chooses which operand to return when one is NaN/infinite (NaN propagation by larger significand), copies it to the destination | LIKELY |
| 377a:00f3 | `emu_Add(dst, a, b)` | add/subtract (sign xor selects subtract): exponent alignment, 64-bit add or subtract, normalise, round, overflow → infinity, underflow → zero | LIKELY |
| 377a:02bd | `emu_Mul(dst, a, b)` | multiply: exponents add, mantissas via `0353`, normalise and round | LIKELY |
| 377a:0353 | `emu_MulMantissa` | 64 × 64-bit mantissa product (high part) | LIKELY |
| 377a:048a | `emu_Div(dst, a, b)` | divide by long division of the 64-bit mantissas | LIKELY |
| 377a:09d9 | `emu_Compare(a, b)` | sets C3/C2/C0 in the status high byte DS:0x3D31: 0x45 unordered, 0x40 equal, 0x01 less, 0x00 greater; respects precision control (DS:0x3D33 bits 0–1) | LIKELY |
| 377a:0b49 | `emu_PackSingle` | packs sign/exponent/mantissa into a 32-bit float (store path) | HYPOTHESIS |
| 377a:0b60 | (jump-table case) | store case: raise exception for special values, then `0b49` | LIKELY |
| 377a:0dc1 | `emu_LoadConstant` | copies an 11-byte constant from a table into the destination | LIKELY |
| 377a:0e3d | `emu_MakeZeroOrInfinity` | writes signed zero or infinity | LIKELY |
| 377a:0e53 | `emu_MakeIndefinite` | raises invalid-operation, writes the x87 "real indefinite" NaN | LIKELY |
| 377a:0e6b, 0e87 | `emu_Push` | push an 11-byte value onto the emulated stack (two copies of the same routine) | CONFIRMED (0e6b) |
| 377a:0eba | `emu_Dup` | push a copy of ST(0) | LIKELY |
| 377a:1446 | `emu_RoundToInt` | round ST(0) to an integer (FRNDINT) | LIKELY |
| 377a:154e | `emu_Polynomial(table, n)` | Horner evaluation with a coefficient table (transcendentals) | LIKELY |
| 377a:1b5d, 1b96 | `emu_Transcendental*` | range reduction / series steps built from add, div, round, polynomial | HYPOTHESIS |
| 377a:1eb8 | `emu_Bcd4ToBin` | 4 packed-BCD digits → binary | LIKELY |
| 377a:1ee7 | `emu_LoadBcd` | FBLD (18-digit packed BCD → real) | LIKELY |
| 377a:1fb2 | `emu_BinToBcd4` | binary → 4 packed-BCD digits | LIKELY |
| 377a:1fd0 | `emu_StoreBcd` | FBSTP (limit 10^18) | LIKELY |
| 377a:2714 | `emu_RaiseException` | sets status flags; unmasked exceptions go to the RTL floating-point error path (`1000:05f0`) | CONFIRMED |

## 2. Transport and setup functions (`358f`, `33bb`)

Names for `358f` follow NETWORK.md §2 where it already names them.

| Ghidra | Proposed name | Purpose | Confidence |
|---|---|---|---|
| 358f:000f | `Swap16` | byte swap (IPX socket numbers are big-endian) | CONFIRMED |
| 358f:0019 | `FormatIpxAddress` | 4 network bytes, `:`, 6 node bytes, each `%02X`, into DS:0x45A4 | CONFIRMED |
| 358f:00b7 | `IpxDetect` | INT 2Fh AX=7A00h; on AL=0xFF stores the IPX entry point at DS:0xEF97 | CONFIRMED |
| 358f:00f1 | `IpxGetAddress(buf)` | IPX function 09h (get internetwork address) | CONFIRMED |
| 358f:0108 | `IpxOpenSocket(socket)` | IPX function 00h, AL = 0 (short-lived); returns DX | CONFIRMED |
| 358f:0128 | `IpxCloseSocket(socket)` | meant to close; **calls function 00h (open) again** (§5.6) | CONFIRMED |
| 358f:0151 | `IpxCancel(ecb)` | IPX function 06h | CONFIRMED |
| 358f:0168 | `IpxSend(ecb)` | IPX function 03h | CONFIRMED |
| 358f:017f | `IpxListen(ecb)` | IPX function 04h | CONFIRMED |
| 358f:0196 | `IpxRelinquish` | IPX function 0Ah; no caller | CONFIRMED (unused) |
| 358f:01a2 | `IpxInitEcbs(len, bufoff, bufseg)` | builds the ECB pools (§5.1) | CONFIRMED |
| 358f:049a | `IpxStartup(socket, len, bufoff, bufseg)` | detect, open, init ECBs, set "socket open" DS:0xEFCB | CONFIRMED |
| 358f:04d7 | `IpxReceiveEsr` | event service routine for receive ECBs (§5.3) | CONFIRMED |
| 358f:0631 | `PrintShadowAt(x, y, fmt, …)` | shadowed text at a position | LIKELY |
| 358f:0698 | `MessageBoxCentered(fmt, …)` | text in a framed box at screen centre (`INITIALIZING MODEM`) | LIKELY |
| 358f:07cd / 0831 | `DrawPlayerList` / `DrawTeamList(x, sel)` | the three choices, highlighted entry | LIKELY |
| 358f:0895 | `HexKeyDown` | index 0–15 of the first held key among 0–9, A–F (scancode table DS:0x3932), else −1 | CONFIRMED |
| 358f:08d5 | `PrintShadowCentered(y, fmt, …)` | horizontally centred shadowed text | LIKELY |
| 358f:0959 | `NetSetup(type)` | setup screen; returns 1 to start (§4.2) | LIKELY |
| 358f:0eea | `NetFrame` | per-frame packet build and send (§7.3) | CONFIRMED |
| 358f:1e19 | `NetShutdown` | IPX: cancel listens, "close" socket; serial: `SerialShutdown` | LIKELY |
| 358f:1e76 | `NetToggleSend` | toggles DS:0x38FA, message `IPX SndPckts ON/OFF.`; no caller anywhere in the image | CONFIRMED (dead) |
| 358f:1ea4 | — | empty; called on Backspace in network games (`2f4d:0bd3`) | CONFIRMED |
| 33bb:0009 | `SerialParseFrame` | finds one framed packet in the receive ring and expands it (§6.3) | CONFIRMED |
| 33bb:0203 | `SerialIsr` | UART interrupt handler (not in the Ghidra function list) | CONFIRMED |
| 33bb:032f | `SerialEnableRx` | IER = 0x0B (receive, transmit-empty, modem-status) | CONFIRMED |
| 33bb:033e | `SerialDisableRx` | IER = 0x02 (transmit-empty only) | CONFIRMED |
| 33bb:034d | `SerialOpen(port, baud, irq)` | UART detect and program, hook IRQ (§6.1) | CONFIRMED |
| 33bb:052b | `SerialShutdown` | IER = 0, MCR = 0, mask IRQ, restore vector | LIKELY |
| 33bb:0579 | `SerialNoCarrierMessage` | `No connection...` when not connected and no message is showing | LIKELY |
| 33bb:0597 | `SerialSendPacket(pkt)` | frames and queues a packet; when not connected, re-reads the carrier | CONFIRMED |
| 33bb:072c | `DrawLinkType` | MODEM / SERIAL radio buttons and INIT STRING field | LIKELY |
| 33bb:0813 | `DrawBaudColumn` | BAUD RATE radio buttons | LIKELY |
| 33bb:095a | `DrawPortColumn` | PORT radio buttons COM1–4 | LIKELY |
| 33bb:0a92 | `DrawIrqColumn` | IRQ radio buttons 3/4/7/10 | LIKELY |
| 33bb:0bc6 | `DrawCustomRow` | CUSTOM port (`%03X`) and IRQ (`%d`) fields | LIKELY |
| 33bb:0c48 | `DrawHardwareSetup` | frame, title `HARDWARE SETUP`, all fields | LIKELY |
| 33bb:0d3e | `DrawSetupCursor(item, on)` | rectangle around item (tables DS:0x3632 x, 0x3654 y, 0x3676 w, 0x3698 h) | LIKELY |
| 33bb:0e9e | `SerialSetup` | the hardware setup screen, UART open, modem init (§4.3, §4.4) | CONFIRMED (disassembly) |
| 33bb:12af | `ModemSend(str)` | wait for transmit-empty, replace the transmit ring with the string, start sending | LIKELY |
| 33bb:131a | `ModemSendInit` | sends `\r<init string>\r` | LIKELY |
| 33bb:133f | `ModemSawConnect` | searches the 256-byte receive ring for `\r\nCONNECT` | LIKELY |
| 33bb:13d0 | `ModemSawOk` | searches the receive ring for `\r\nOK\r\n` (DS:0x36BC) | LIKELY |
| 33bb:1462 | `ModemDropDtr` | MCR bit 0 = 0 | CONFIRMED |
| 33bb:146f | `ModemPause` | waits up to 900 tics or until `IN_CheckAck` | LIKELY |
| 33bb:1496 | `ModemDial` | reset, init, dial and wait for CONNECT with retries (§6.5) | LIKELY |
| 33bb:171a | `DrawDialCursor(item, on)` | cursor in the dial box (table DS:0x36C4) | LIKELY |
| 33bb:18dd, 193b, 1999 | `DrawDialField*` | DIAL COMMAND, PHONE NUMBER, RETRIES fields | LIKELY |
| 33bb:19f9, 1a09 | `DrawDialButton*` | DIAL and ABORT buttons (Ghidra's `CleanupControlPanel` names are wrong) | HYPOTHESIS |
| 33bb:1a19 | `DrawDialBox` | frame and title `MODEM DIAL BOX`, fields | LIKELY |
| 33bb:1abe | `ModemDialBox` | the dial box loop (§6.5) | CONFIRMED (disassembly) |

## 3. Settings and their defaults

None of these are saved: `CONFIG.DAT` (GAMEFLOW §10) has no network field except the game type, and no reference to these addresses exists in `ReadConfig`/`WriteConfig`. Every run starts from the defaults below. CONFIRMED (file data; reference search over the export).

| DS | Meaning | Default | Set by | Confidence |
|---|---|---|---|---|
| 0x38F8 | IPX socket (host order) | **0x4003** | hex entry on the setup screen | CONFIRMED |
| 0x38FA | packet sending enabled | 1 | only the dead `NetToggleSend` | CONFIRMED |
| 0x38FC | local player id | 0xFFFF until set | `NetSetup`: IPX = last byte of the node address; serial = `US_RndT()` | CONFIRMED |
| 0x35C2 | link type: 1 = MODEM (Hayes type), 2 = SERIAL (null modem) | 1 | hardware setup | CONFIRMED |
| 0x35BE | UART base port | 0x3F8 (COM1) | hardware setup | CONFIRMED |
| 0x35C0 | IRQ | 4 | hardware setup | CONFIRMED |
| 0x362E | baud rate (long) | 9600 | hardware setup | CONFIRMED |
| 0x35C4 | retries (modem init and dial) | 10 | dial box | CONFIRMED |
| 0xEEAB | modem init string (30 bytes) | `ATQ0V1X4&C1&D2` (copied on first entry if empty) | hardware setup | LIKELY |
| 0xEEC9 | dial command (30 bytes) | `ATDT` (same rule) | dial box | LIKELY |
| 0xEE8D | phone number (30 bytes) | empty | dial box | LIKELY |
| 0xEF4F | character / team choice 0–2 | 0 | `NetSetup` | CONFIRMED |
| 0x36BA | hardware setup cursor (kept between visits) | 0 | | CONFIRMED |
| 0x45A2 | dial box cursor (kept between visits) | 0 | | LIKELY |

Option tables (CONFIRMED, file data): baud rates DS:0x361E {9600, 14400, 19200, 38400} (longs); UART divisors DS:0x3606 {12, 8, 6, **4**}; ports DS:0x3616 {0x3F8, 0x2F8, 0x3E8, 0x2E8}; IRQs DS:0x360E {3, 4, 7, 10}; IRQ → interrupt vector DS:0x35C6 (IRQ 0–7 → 08h–0Fh, 8–15 → 70h–77h); PIC mask bits DS:0x35E6.

## 4. Setup screens and flow

### 4.1 Entry

The main menu's GAME TYPE item selects N (4), T (5) or M (6) (GAMEFLOW §4). After START GAME closes the menu, `US_ControlPanel` (`1618:061a`) calls `NetSetup(type)`; if it returns 0 the menu reappears (palette mode 3). CONFIRMED (GAMEFLOW §4.1, re-read). There is no separate network menu in the main menu.

Flow by type (LIKELY):

```
N, T:  START GAME → NetSetup: IPX start-up → CHOOSE A PLAYER / CHOOSE A TEAM → Get Ready! → game
M:     START GAME → NetSetup → SerialSetup: HARDWARE SETUP → UART open → (MODEM only) INITIALIZING MODEM
              → CHOOSE A PLAYER → Get Ready! → game → (in game) Alt+D MODEM DIAL BOX, or wait for the call
```

### 4.2 `NetSetup(type)` (`358f:0959`) — LIKELY (decompiler, checked against the call graph)

1. Fade out, palette mode 1. Clear the peer table (DS:0xED7D, 256 bytes) and peer count (DS:0xEF51); `gamestate.difficulty = type`.
2. Transport start-up:
   - IPX types: `IpxStartup(socket, 24, 4139:0000)`. On failure: status-bar backdrop, `IPX initialization error` centred at y = 80 (colour 0xBF), fade in, wait for a key, fade out, return 0.
   - Modem type: `SerialSetup()` (§4.3); 0 → return 0 (no message here).
3. Local id DS:0x38FC (§3). DS:0xEF8F = 1 (reload level), DS:0xEF95 = 1 (arm receivers at the first frame).
4. Screen over the status-bar backdrop (`DrawPlayScreen`), all lines centred:
   - y = 2: `Player ID number %d` (the local id).
   - IPX: y = 14 `IPX address %s` (network:node in hex), y = 26 `Communicating on IPX socket %X`.
   - Modem: y = 14 `I/O Address %X IRQ%d`, y = 26 `Communicating at %ld bps` (the selected rate, see §6.1 for the 38400 quirk).
   - y = 40: `CHOOSE A TEAM` (type 5) or `CHOOSE A PLAYER` (types 4, 6), colour 0x9F.
   - y = 60, 72, 84: `Team 1`, `Team 2`, `Team 3` or `Agent`, `Terrorist 1`, `Terrorist 2` (tables DS:0x3922 / DS:0x3928), in a column centred on the widest entry; highlight colour 0xBF, others 0x94 + 2·i.
5. Keys:
   - Up / Down: move the highlight, wrapping 0 ↔ 2; redraw and `TicDelay(10)`.
   - Enter, Space or button 0: redraw the list without highlight, `Get Ready!` at y = 120 (colour 0xBF), `mapon = 50`, DS:0xEF4F = choice, return 1.
   - Esc or button 1: fade out, `NetShutdown()`, return 0.
   - IPX types only: a held hex key (0–9, A–F) opens a 4-character line input at (237, 20) prefilled with that digit. Accepted: `IpxCloseSocket(old)`, socket = the typed hex value, and the whole procedure restarts from step 1 with the new socket. Esc in the input keeps waiting.
6. No packet is sent or received in `NetSetup`. CONFIRMED (no call to the send/listen wrappers).

The choice index selects the puppet others see and the start weapons; in type 5 it is also the team (NETWORK.md §3.1).

### 4.3 `SerialSetup` (`33bb:0e9e`), the HARDWARE SETUP screen — CONFIRMED (disassembly)

On entry the init string and dial command get their defaults if empty. The screen is a framed panel titled `HARDWARE SETUP`, drawn over the status-bar backdrop, with 17 cursor positions (DS:0x36BA). Radio buttons show selected in colour 0x1F, unselected 0x90.

| Item | Field | Effect of Enter / button 0 | Cursor box (x, y) |
|---|---|---|---|
| 0 | `MODEM (Hayes type)` | link type = 1 | (55, 37) |
| 1 | `SERIAL (null modem)` | link type = 2 | (55, 49) |
| 2 | `INIT STRING` | line input, max 20 characters, into DS:0xEEAB; skipped by the cursor (and drawn greyed) unless the link type is MODEM | (55, 80) |
| 3–6 | `BAUD RATE` `9600 bps` `14400 bps` `19200 bps` `38400 bps` | baud = table entry | (55, 114…150) |
| 7–10 | `PORT` `COM1`–`COM4` | port = table entry; IRQ = 4 for COM1/COM3, 3 for COM2/COM4 | (154, 114…150) |
| 11–14 | `IRQ` `IRQ3` `IRQ4` `IRQ7` `IRQ10` | IRQ = table entry | (215, 114…150) |
| 15 | `CUSTOM` port | line input, 4 hex digits (`%03X`), parsed base 16 | (163, 165) |
| 16 | `CUSTOM` IRQ | line input, 2 decimal digits | (223, 165) |

Keys: Up/Down move (wrapping 0 ↔ 16, skipping item 2 for SERIAL); Enter or button 0 acts on the item; **Esc or button 1 accepts the settings and connects**. There is no way back to the menu from this screen except a failed connection. CONFIRMED.

Connect: `SerialOpen(port, baud, irq)`; failure → return 0. For SERIAL, return 1 at once. For MODEM, modem initialisation (§4.4).

### 4.4 Modem initialisation at setup — CONFIRMED (disassembly)

Shows `INITIALIZING MODEM` in a centred box, clears the keyboard, then repeats up to DS:0x35C4 (10) times: send `\rATE0S0=1\r` (echo off, **auto-answer on the first ring**; the user's init string is *not* sent here) and wait up to **450 tics** (6.4 s) for `\r\nOK\r\n` anywhere in the receive ring. `OK` seen → return 1. All attempts timed out → return 0 (back to the menu, no message). A key press ends the wait and returns 1. Quirk (LIKELY): the retry test is "elapsed tics exactly equal to 450"; if `CalcTics` overshoots, the timeout counts as success.

So after setup both modem players are in auto-answer mode; the call itself is made in game (§6.5).

### 4.5 In-game screens

- `Get Ready!`: only at the end of `NetSetup` (above). The game then loads NETLEVEL slot 50 and starts at once; nobody waits for anybody. CONFIRMED.
- `PLYR ID %d IN`: in-game message (`GameMessage`, 125 tics) when `NetFrame` creates the puppet of a newly heard player; the number is that player's id (the last byte of their IPX node address, the same number their own setup screen showed as `Player ID number`; on serial always 1). CONFIRMED.
- KILLS table (`358f:13c5`): full screen after each completed level (`NetLevelDone`, `358f:1720`), after Alt+W warps to a network level, and before quitting from EXIT TO DOS in a network game (`1618:13cb`). A 12 × 12 grid, title `KILLS` at the top, `KILLED` written vertically down the left edge, row = killer slot, column = victim slot (NETWORK.md §7). Slot 0 is labelled **`U`** (you); other rows `%2d` and columns `%d` with the **local slot number**, not the player id. A row/column is filled only when that slot has a record (its id byte is non-zero, so a player whose id is 0 never shows, quirk). Waits for a key. CONFIRMED.
- `Activity on %d`: not debug output. When a puppet's latest packet comes from a different `mapon`, the puppet is hidden and this message shows the other player's floor number (`mapon + 1`, the number on their status bar, so 51–60 for slots 50–59). Only shown when no other message is up (DS:0x46A4 is the `GameMessage` timer, PLAYER.md). CONFIRMED.
- `No connection...`: modem type, every frame while no carrier is detected and no message is up. LIKELY.
- `IPX SndPckts %s.` (`ON`/`OFF`): from `NetToggleSend`, which nothing calls: a removed debug key (Backspace now calls the empty `358f:1ea4`). Unreachable in `OBC-A`. CONFIRMED.

## 5. IPX transport

### 5.1 Start-up and buffers (`358f:049a`, `358f:01a2`) — CONFIRMED

1. `IpxDetect`: INT 2Fh AX=7A00h; AL = 0xFF required; the far entry point is kept at DS:0xEF97.
2. `IpxOpenSocket(socket)`, short-lived. Success test: the returned DX (socket in network order) must be ≥ 0x100. The completion code in AL is not checked, so this mostly tests the socket number: a socket whose low byte is 0 (for example 0x4000) is rejected as an `IPX initialization error`. LIKELY.
3. `IpxGetAddress` into `4217:0000` (4-byte network, 6-byte node).
4. ECB pools. Every packet is a 30-byte IPX header plus a **24-byte** payload in two fragments; IPX packet type 4; destination socket = our socket.

| Pool | ECBs | Header buffers | Payload | Destination |
|---|---|---|---|---|
| receive | 32 at `4162:i·0x30`, ESR = `358f:04d7` | `41db:i·30` | `40d5:i·24` | — |
| broadcast send | 1 at `415f:0000` | `41d9:0000` | the packet buffer `4139:0000` | network 0 (this segment), node FF:FF:FF:FF:FF:FF, immediate address FF…FF |
| per-player send | 12 at `413b:(s−1)·0x30` for slot *s* = 1…12 | `41c2:(s−1)·30` | `4139:0000` | filled in when slot *s* is first heard: network + node from that packet's source, immediate address = its source node |

Because the immediate address is the peer's own node and the broadcast goes to network 0, **all players must be on the same LAN segment**; IPX routing is never used (no Get Local Target call). LIKELY.

### 5.2 Ids, slots and discovery — CONFIRMED

- A player's identity on the wire is **one byte**: the last byte of their node address (DS:0x38FC on their own machine).
- Each machine keeps a 256-entry map DS:0xED7D from that byte to a **local slot number**: 0 = unknown, negative = newly heard (puppet not yet spawned), positive = slot. Slots are handed out 1, 2, 3 … in the order players are first heard (count DS:0xEF51), so the same player usually has different slot numbers on different machines. Slot 0 is the local player.
- Discovery is passive. A machine sends a **broadcast** right after every (re)spawn, including the level start, and then every **0x118 tics (4 s)** as a keep-alive (§7.3). The first packet from an unknown byte gives it a slot and fills that slot's per-player ECB; the next frame `NetFrame` spawns its puppet, shows `PLYR ID %d IN` and, if the local player is standing still, sends a one-off event 0xFF so the newcomer hears from us at once (any changed packet does the same).
- There is no lobby, no host, no handshake, no start agreement and no level choice: everyone starts on slot 50 when they press Enter, and joins whatever game is running on the socket. Players who start later simply appear. CONFIRMED.
- The slot map is cleared at the end of every level (`NetLevelDone`) and in `NetSetup`; players re-acquire slots (possibly different ones) as their packets arrive on the next floor. The KILLS matrix (segment 0x4127) is never cleared, so after a floor change its rows can belong to different players than before (quirk, LIKELY: no clearing code found).

### 5.3 Receive ESR (`358f:04d7`) — CONFIRMED (disassembly)

Runs at interrupt time for each completed receive ECB:

1. Increments DS:0xEF09 and sets DS:0xEF0C (write-only statistics; nothing reads them).
2. If the ECB completed without error: ignore the packet if the source node's last byte equals the local id (this filters our own broadcasts, and also every other machine whose node ends in the same byte: two such machines never see each other).
3. Stamp payload byte +2 with the source node byte. Look up the slot; if unknown, assign the next slot, mark it new, copy the source node into the slot's ECB immediate address and the source network + node into its header.
4. Copy the 24-byte payload into the slot's record DS:0xEC5B + 24·slot, unless the record's lock byte (+0x14) is set. Only the newest packet per player is kept.
5. Re-post the listen, **only if the completion code was 0**. An ECB that completes with an error is not re-posted until the next re-arm (after a death, a menu visit or a floor change), so the receive pool can shrink. LIKELY.

There is no bound on the slot number (see §7.5).

### 5.4 Sending

All sends are synchronous: after `IpxSend` the code spins until the ECB's in-use flag clears. It never calls IPX Relinquish Control. Schedule in §7.3. CONFIRMED.

### 5.5 Arming and disarming

`NetFrame` posts every idle receive ECB when DS:0xEF95 is set: after `NetSetup`, after `NetPlayerDied` (death, and opening the menu) and after `NetLevelDone`. `NetPlayerDied` and `NetLevelDone` first cancel every posted receive ECB, so **nothing is received while the menu, the death sequence or the KILLS table is up**. CONFIRMED.

### 5.6 Shutdown and the close bug

`NetShutdown` (`358f:1e19`, from `NetSetup` on Esc and from `Quit` via `2f4d:137a`): if the socket-open flag DS:0xEFCB is set, cancel posted receive ECBs and call `IpxCloseSocket`; otherwise, in the modem type, `SerialShutdown`. `IpxCloseSocket` loads IPX function **00h (open)** instead of 01h, so the socket is never closed; it stays open until IPX drops short-lived sockets at program exit. Changing the socket on the setup screen therefore leaves the old one open too. CONFIRMED (disassembly of `358f:013f`).

## 6. Serial and modem transport (`33bb`)

### 6.1 UART set-up (`SerialOpen`, `33bb:034d`) — CONFIRMED (disassembly)

1. Reset both ring buffers (transmit `4107:0000`, receive `4117:0000`, 256 bytes each, indices masked to 8 bits).
2. Look the baud rate up in DS:0x361E; not found → `Invalid baud rate selected: %ld`, return 0. The divisor comes from DS:0x3606: 12, 8, 6, **4**. 115200/4 is **28800**, so choosing `38400 bps` really runs at 28800 bps (quirk; harmless when both ends choose the same).
3. Detect: LSR (base + 5) must have bits 5 and 6 set, else `UART not detected at port %X`, return 0.
4. LCR = 0x80, divisor low byte, divisor high byte 0, LCR = 3 (8 data bits, no parity, 1 stop). The 16550 FIFO is not enabled.
5. If 0 < IRQ < 16: save the old vector, install `SerialIsr`, unmask the IRQ at the master (0x21) or slave (0xA1) PIC, IER = 0x0B. IRQ 0 (possible through CUSTOM) installs nothing, so nothing is ever received.
6. MCR = 0x0B (DTR, RTS, OUT2). Return 1.

`SerialShutdown`: IER = 0, MCR = 0 (drops DTR, which hangs up a modem set to `&D2`), re-mask the IRQ, restore the vector. LIKELY.

### 6.2 Interrupt handler (`33bb:0203`) — CONFIRMED (disassembly)

Loops while IIR reports a pending interrupt:

- Modem-status change: connected flag DS:0xEF05 = DCD (MSR bit 7) for MODEM, DCD or DSR (bits 7, 5) for SERIAL; DS:0xEE83 = not connected (write-only).
- Transmit-empty: if the transmit ring is not empty, send the next byte (counter DS:0xEEE7).
- Receive: move every available byte into the receive ring (counter DS:0xEEE9). There is no overflow check: a full ring overwrites unread bytes.
- While no remote player has joined (DS:0x872E, which is `gamestate.killtotal`, is 0): run `SerialParseFrame`; after the first complete packet set DS:0x872E = 1, give the remote player **slot 1** (map entry for byte 1 = −1, "new"), and copy the packet into slot 1's record. After that, frames are parsed from the game loop instead (`NetSyncPuppet` calls `SerialParseFrame` every frame, NETWORK.md §4.3).

EOI is sent to one PIC only (the slave for IRQ ≥ 8), so IRQ 10 never acknowledges the master's cascade line. LIKELY (standard PC behaviour; the single EOI is CONFIRMED).

`SerialDisableRx` (IER = 2) is called by `NetPlayerDied` and `NetLevelDone`; `NetFrame` turns receive interrupts back on with `SerialEnableRx` at the next re-arm. Bytes arriving in between overrun the UART and are lost. HYPOTHESIS (consequence).

### 6.3 Framing and compression — CONFIRMED (disassembly)

Each 24-byte packet travels as **16 bytes**: `01 FF`, 12 payload bytes, `02 FE`.

| Payload bytes | Content |
|---|---|
| 0–1 (word) | bits 15–12 event, 11–8 `mapon − 50`, 7–4 weapon, 3–0 choice |
| 2–3 | angle |
| 4–7 | x (16.16) |
| 8–11 | y (16.16) |

Sending (`SerialSendPacket`): an event 2 with target −1 is first rewritten as event 0xF (the caller's buffer is modified). The event byte is sign-extended before the shift, so event 0xFF also becomes nibble 0xF. Bytes go into the transmit ring; if the transmitter is idle the first byte is written directly and the interrupt sends the rest. If the link is not connected, the packet is dropped and the carrier status is re-read instead.

Receiving (`SerialParseFrame`): scans from the ring tail; skips bytes until `01 FF`; if the bytes 14 and 15 further on are not yet `02 FE` it returns and waits (a lost or extra byte can stall parsing until the ring contents happen to realign, HYPOTHESIS). A complete frame is zeroed in the ring and expanded into `40d5:0000`: event (nibble 0xF → event 2 with target −1), `mapon = nibble + 50`, sender id byte **1**, position, angle, weapon, choice; target = **the local id** for every other packet; the "applied" byte +0x15 = 0. The score (+0x16) and the real target/killer id are **not transmitted**.

Consequences (LIKELY; the gameplay effect belongs to NETWORK.md):

- The one-off "hello" event 0xFF that `NetFrame` sends after a player joins arrives as **event 2 (died) with no killer**.
- Every remote shot "targets" the local player and every remote death is credited to the local player (two players only, so mostly right; a remote suicide or menu visit also counts as your kill).
- Team score sharing cannot work over serial (no score field; type 6 has no teams anyway).
- If the local random id happens to be 1, the code that compares packet ids with the local id confuses the remote player with the local one (1 in 256 chance per setup). HYPOTHESIS.

### 6.4 Connection state

Connected = DS:0xEF05 non-zero (§6.2). It is updated by the interrupt, by `SerialSendPacket` while disconnected, and after leaving the dial box. While it is 0 nothing is sent and `No connection...` is shown. There is no timeout or "player left" detection. CONFIRMED.

### 6.5 Calling: dial box, dial routine and modem keys

The call is made **during the game**. In the modem type, `2f4d:0bd3` (run from `T_Player`) handles Alt + key every frame without edge detection (holding the keys repeats the action). CONFIRMED (keyboard offsets).

| Keys | Action |
|---|---|
| Alt+A | send `\rATE0S0=1\r` (auto-answer on) |
| Alt+N | send `\rATE0S0=0\r` (auto-answer off) |
| Alt+B | send `\rATH1\r` (off hook) |
| Alt+H | drop DTR, pause up to 900 tics or a key, send `\rATH\r` |
| Alt+R or Alt+I | drop DTR, pause, send `\r<init string>\r` |
| Alt+D | `MODEM DIAL BOX` |

DTR is raised again only by `ModemDial`, so after Alt+H, Alt+R or Alt+I it stays low (with `&D2` the modem stays on hook) until a dial. HYPOTHESIS (modem behaviour). The pause usually ends at once because `IN_CheckAck` sees the still-recent key press. HYPOTHESIS.

`ModemDialBox` (`33bb:1abe`), CONFIRMED (disassembly). The game is frozen while it is open (it runs inside `T_Player`). Items:

| Item | Field | Action |
|---|---|---|
| 0 | `DIAL COMMAND` | line input, max 18 characters, into DS:0xEEC9 |
| 1 | `PHONE NUMBER` | line input, max 18, into DS:0xEE8D |
| 2 | `RETRIES` | line input, 3 digits, into DS:0x35C4 |
| 3 | `DIAL` | `ModemDial`, then close |
| 4 | `ABORT` | close |

Up wraps 0 ↔ 4; Down wraps after 5, so a sixth, invisible position 5 exists where Enter does nothing (quirk). Esc or button 1 closes. On closing: redraw the status-bar backdrop, clear the keyboard, re-read the carrier.

`ModemDial` (`33bb:1496`), LIKELY. A box with `DIALING`, `Phone # %s`, `Baud Rate = %ld`, then up to DS:0x35C4 attempts, each:

1. `Status = RESETTING MODEM`, `Attempts = n/max`: drop DTR, pause up to 900 tics (12.9 s), raise DTR, clear the receive ring, send `<init string>\r`.
2. `Status = INITIALIZING MODEM`: wait up to **1800 tics** (25.7 s) for `OK`; none → drop DTR, pause, next attempt.
3. Clear the receive ring, send `<dial command><phone number>\r`, `Status = DIALING NUMBER`: wait up to **3600 tics** (51.4 s) for `\r\nCONNECT`. Seen → return (connected). None → `Status = TIME-OUT`, drop DTR, pause, next attempt.

Any key aborts at once. After the last attempt DTR is raised and the routine returns. The other player answers through the auto-answer set at setup (or Alt+A); there is no manual answer (`ATA`) command. CONFIRMED (strings, call list).

### 6.6 Differences from IPX in gameplay terms

| | IPX (N, T) | Modem/serial (M) |
|---|---|---|
| Players | up to 12 (§7.5) | **2**: the remote is always slot 1, joined once per floor |
| Local id | node byte (stable) | random byte each setup, never transmitted |
| Keep-alive broadcast | every 4 s | none |
| Fields sent | all 24 bytes | event, map, weapon, choice (4 bits each), angle, position |
| Target/killer id | sent | replaced by the local id at the receiver |
| Score | sent | not sent |
| Teams | type 5 | no team type |
| Start | press Enter, peers appear | press Enter; play alone until the call connects (`No connection...`) |

CONFIRMED (code paths above).

## 7. Packet model (transport view)

### 7.1 Model

Not lockstep and not input-sharing: each machine is authoritative over its own player and sends **its own state** (position, angle, weapon, choice, score) plus a one-shot event code; remote players are puppets driven by the newest state received (NETWORK.md §1). CONFIRMED.

### 7.2 Layout

NETWORK.md §4.1 has the full table. Transport-level details (CONFIRMED):

- +0x02 is effectively **one byte** (sender id). The IPX ESR overwrites only that byte with the real source node byte; +0x03 is never written by the sender and stays 0. `PLYR ID %d IN` reads +2 as a word, which works because +3 is 0.
- +0x14 is a lock: the consumer sets it while copying the record so the ESR does not write into it. Senders leave it 0; a sender with +0x14 set would freeze its record on every receiver.
- +0x15 is an "already applied" flag: the consumer sets it after copying; every new arrival overwrites the record with the sender's 0, which marks it as new. (NETWORK.md calls it "puppet exists"; this reading follows the serial path, which clears it on each new frame and sets it after the copy.)
- The packet buffer is `4139:0000`; the last packet sent is kept at DS:0xEFB3 for the change test.

### 7.3 When packets are sent (`NetFrame`, `358f:0eea`, once per `PlayLoop` frame) — CONFIRMED (disassembly)

Per frame (frames are variable length, measured in 70 Hz tics), if DS:0x38FA is set (always):

1. Re-arm receivers if requested (§5.5); serial: enable receive interrupts.
2. If a respawn is pending, place and reset the player (NETWORK.md §5).
3. Set the event byte if no event was set this frame: 1 while `controlx` or `controly` is non-zero; otherwise 0xFF once after a new player joined.
4. Fill the packet.
5. Modem type: `No connection...` check.
6. Broadcast (IPX) or serial send:
   - just respawned: one broadcast (IPX) or one serial send; DS:0xEFCD = 1 (write-only, nothing in the image reads it);
   - otherwise, IPX only: a broadcast if at least 280 tics (4 s) have passed since the last keep-alive.
7. If the packet differs from the last one sent: remember it and send it to **every known player one by one** (IPX, waiting for each), or once over serial.
8. Spawn puppets for newly heard players.
9. Clear the event byte.

`NetReportUse` (event 3) and `NetPlayerDied` (events 2, 5) call `NetFrame` immediately, so those events leave in the frame they happen. NETWORK.md §6.

Rates in practice: a moving player sends one packet per frame to each other player (position changes every frame); a player who stops sends one more packet (event back to 0) and then only the keep-alive. HYPOTHESIS (follows from the change test).

### 7.4 Loss, timeouts and disconnects

- No acknowledgement, sequence number, retransmission or checksum beyond IPX's own. A lost packet is simply superseded by the next one; a lost event (death, use, shot) is lost for good. CONFIRMED.
- Only the newest packet per player survives between two frames, so two events inside one frame lose the first. CONFIRMED (NETWORK.md §4.3).
- No timeout: a player who quits or loses the link stops sending and their puppet stays frozen where it was on everyone else's screen until the next floor (slot map cleared). No "player left" message exists. CONFIRMED (no such code or string).
- Quitting the program: KILLS table (EXIT TO DOS), then `Quit` → `NetShutdown`. No goodbye packet. LIKELY.

### 7.5 Player limit

**12 players including yourself** (11 remote slots). Evidence: 12 per-player send ECBs (slots 1–12 would fit them), the record array DS:0xEC5B + 24·slot holds slots 0–11 and slot 12 would overwrite DS:0xED7B (the record pointer) and the start of the slot map; the KILLS matrix in segment 0x4127 is exactly 12 × 12 words (the segment is 288 bytes long); `NetSyncPuppet` ignores slots ≥ 12; `NetRoomForPlayer` (`358f:123e`, "fewer than 12") exists but nothing calls it. The ESR does not check the limit, so a 12th remote player corrupts memory. CONFIRMED (structure), LIKELY (intended limit). Nothing in `33bb` or `377a` limits IPX further (answers NETWORK.md Q-NET-4). Serial is limited to 2.

## 8. Hooks between transport and the game (question 6)

`377a` makes no calls into `358f` or the game; see §0. The transport's connections to the rest of the game (CONFIRMED unless noted):

| Caller | Callee | Data |
|---|---|---|
| `US_ControlPanel` `1618:061a` (after START GAME, type ≥ 4) | `NetSetup(type)` `358f:0959` | type; returns start/cancel; sets `mapon = 50`, DS:0xEF4F, DS:0x38FC |
| `NetSetup` | `IpxStartup` / `SerialSetup` `33bb:0e9e` | socket DS:0x38F8, payload 24 bytes at `4139:0000` / port, baud, IRQ |
| `US_ControlPanel` (menu opened in a network game) | `NetPlayerDied(player)` `358f:182c` → `SerialDisableRx` or ECB cancel, `NetFrame` | suicide event 2 sent at once |
| `PlayLoop` `1942:1315`, every frame | `NetFrame` `358f:0eea` → `IpxSend`, `IpxListen`, `SerialSendPacket`, `SerialEnableRx`, `SerialNoCarrierMessage` | player x, y, angle, `gamestate.weapon` (DS:0x8716), score (DS:0x8704), `mapon`, choice |
| `DoActor` → `NetSyncPuppet` `358f:19fd` | `SerialParseFrame` `33bb:0009` (modem only) | slot records DS:0xEC5B + 24·slot |
| `T_Player` → `2f4d:0bd3` | modem keys (§6.5), `358f:1ea4` (Backspace, empty) | strings DS:0x3279–0x3291 |
| `GameLoop` `1823:0ea3` | `NetReloadLevel` `358f:1251`, `NetLevelDone` `358f:1720` (→ `SerialDisableRx`, KILLS) | DS:0xEF8F reload flag |
| `TakeDamage` `208c:0494` | `NetPlayerDied(attacker)` | killer id |
| `Cmd_Use` `208c:104c` | `NetReportUse` `358f:1979` | event 3 |
| `CP_Quit` `1618:13cb` | `NetDrawKills` `358f:13c5` | KILLS matrix |
| `Quit` `1497:121d` → `2f4d:137a` | `NetShutdown` `358f:1e19` → `SerialShutdown` | — |

## 9. Notes for the port's menus

What a faithful network menu has to offer, from the above (the transport itself will differ):

- No network options in the main menu; picking game type N, T or M and START GAME leads to one setup screen.
- N/T screen: own player id, own address, socket (default 0x4003, editable as 4 hex digits), choose one of three characters (N) or teams (T), Enter starts at NETLEVEL slot 50, Esc returns to the menu. No waiting room.
- M: the HARDWARE SETUP panel (link type, init string, baud, port, IRQ, custom port/IRQ; Esc continues), then the character screen; the call is placed in game (Alt+D dial box with dial command, phone number, retries; Alt+A/N/B/H/R/I modem commands).
- Players join by being heard; `PLYR ID n IN` announces them; floors are left individually; KILLS after each floor.
- Nothing is saved between runs.

## 10. Open questions

- **Q-NC-1** What the original IPX driver does with the buggy "close" (function 00h on an open socket): harmless failure, or a second socket? Approach: DOSBox-X IPX run, socket table dump.
- **Q-NC-2** Whether IPX calls the ESR for ECBs cancelled by `IpxCancel` (completion 0xFC), and so whether cancelled ECBs are skipped or re-posted. Approach: DOSBox-X IPX emulation behaviour.
- **Q-NC-3** Real frame rates in a two-machine IPX game, to size the per-frame unicast traffic (packets per second per player). Approach: two DOSBox-X instances, IPX packet counters.
- **Q-NC-4** Gameplay effect of the serial "hello → died" decoding (§6.3): does the receiver's puppet die and revive on the next packet? Approach: NETWORK.md puppet logic, or a null-modem run in two DOSBox-X instances.
- **Q-NC-5** Whether the serial frame parser can stall permanently after a lost byte (§6.3). Approach: inject a byte drop in a null-modem DOSBox-X run.
- **Q-NC-6** Purpose of the write-only counters DS:0xEF09, DS:0xEF0C, DS:0xEEE7, DS:0xEEE9, DS:0xEE83 and DS:0xEFCD (presumably removed debug displays alongside `IPX SndPckts`). Low priority.
