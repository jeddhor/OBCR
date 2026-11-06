# Network game rules (segment `358f`)

Multiplayer in OBC-A: game types **N** (network), **T** (network teams) and **M** (modem/serial), stored as `gamestate.difficulty` 4, 5 and 6. This document covers the rules and the synchronisation model in segment `358f` and its hooks in the rest of the game. Transport details (the IPX layer in this segment, the serial/modem driver and its setup screens in segment `33bb`) are in [NETCOMMS.md](NETCOMMS.md). Segment `377a` is the Borland floating-point emulator, not network code.

Tags follow the other RE documents: CONFIRMED (read in the code), LIKELY, HYPOTHESIS, UNKNOWN. Addresses are Ghidra `seg:off` (base segment 0x1000).

## 1. Model in one paragraph

There is **no lockstep**. Every machine runs its own copy of the level and is authoritative for its own player only. Once per frame each machine builds a 24-byte **state packet** describing its player (position, angle, weapon, score, character or team) plus a one-shot **event code** (moving, fired, used, died, burned to death). Other players appear on each machine as **puppet actors** (an ally or enemy sprite) driven by the latest packet from that player. Damage is always decided on the **victim's** machine: a shot makes the shooter's puppet run its enemy attack state there, so the normal enemy hit roll applies; flames and grenades are re-spawned from the puppet as local projectiles. Deaths are announced by the victim, which is how kills are counted. Level state (doors aside, see §6.3), items and fires are not shared. CONFIRMED (`358f:0eea`, `358f:19fd`, `358f:1283`, `358f:182c`).

## 2. Functions

| Address | Name (this port) | Purpose | Status |
|---|---|---|---|
| `358f:000f` | `Swap16` | byte swap (IPX headers are big-endian) | CONFIRMED |
| `358f:0019` | `FormatIpxAddress` | formats network (4 bytes) and node (6 bytes) as hex for the setup screen | CONFIRMED |
| `358f:00b7` | `IpxDetect` | INT 2Fh AX=7A00h; stores the IPX entry point | CONFIRMED |
| `358f:00f1`…`0196` | `Ipx*` | thin calls into the IPX entry point (open/close socket, send, listen, relinquish) | LIKELY |
| `358f:01a2` | `IpxInitEcbs` | 32 receive ECBs (30-byte header + 24-byte data each), 1 broadcast send ECB, 12 per-peer send ECBs | CONFIRMED |
| `358f:049a` | `IpxStartup(socket, …)` | detect, open the socket (fails below 0x100), init ECBs | CONFIRMED |
| `358f:04d7` | `IpxReceiveEsr` | receive handler: maps the sender to a slot, stores its packet (§4) | CONFIRMED |
| `358f:0631`, `0698`, `08d5` | text helpers | shadowed text, boxed message, centred text | CONFIRMED |
| `358f:07cd`, `0831` | `DrawChoiceList` | three-entry list (characters or teams), highlighted entry | CONFIRMED |
| `358f:0895` | `HexKeyDown` | which of the keys 0–9, A–F is down (socket entry) | CONFIRMED |
| `358f:0959` | `NetSetup(type)` | setup screen: player id, address, socket, character or team choice; returns 1 to start (§3) | CONFIRMED |
| `358f:0d7b` | `NetRequestRespawn` | respawn pending = 1, reload level = 0; called by *SpawnPlayer* in network types | CONFIRMED |
| `358f:0d8c` | `NetRandomSpot` | picks the respawn cell (§5.2) | CONFIRMED |
| `358f:0e90` | `AngleToDir` | angle → one of 8 directions (45° sectors centred on 0°) | CONFIRMED |
| `358f:0eea` | `NetFrame` | per-frame: respawn, build and send the packet, spawn puppets for new players (§4) | CONFIRMED |
| `358f:123e` | `NetRoomForPlayer` | fewer than 12 remote players; no caller found | CONFIRMED (unused) |
| `358f:1251` | `NetReloadLevel` | returns the reload flag; *GameLoop* skips *SetupGameLevel* when 0 | CONFIRMED |
| `358f:1259` | `NetTargetsMe(ob)` | the puppet's player last targeted the local player | CONFIRMED |
| `358f:1283` | `NetRemoteFire(ob)` | replays a remote shot locally, by weapon (§6.1) | CONFIRMED (disassembly) |
| `358f:135a` | `NetHitPuppet(ob)` | local hit on a puppet: pain state only (§6.1) | CONFIRMED |
| `358f:13a2` | `NetReportFire(flag, target)` | event 4 and the target id in the packet | CONFIRMED |
| `358f:13c5` | `NetDrawKills` | the KILLS table (§7) | CONFIRMED |
| `358f:1720` | `NetLevelDone` | close ECBs, forget all players, request respawn and reload, show KILLS | CONFIRMED |
| `358f:1796` | `NetResetPlayer` | respawn loadout (§5.3) | CONFIRMED |
| `358f:182c` | `NetPlayerDied(attacker)` | count the kill, send event 2 or 5 at once (§6.2) | CONFIRMED |
| `358f:18e7` | `NetDropWeapon(ob, x, y)` | a dead puppet drops its weapon (§6.4) | CONFIRMED (disassembly) |
| `358f:1969` | `NetClearDrop(weapon)` | a dropped weapon was picked up | CONFIRMED |
| `358f:1979` | `NetReportUse` | event 3 and send at once | CONFIRMED |
| `358f:198d` | `NetUseInProgress` | DS:0xEF93: a remote Use is being replayed | CONFIRMED |
| `358f:1995` | `NetMyId` | local player id | CONFIRMED |
| `358f:199d` | `NetSameTeam(ob)` | `ob->temp1 == local choice` | CONFIRMED |
| `358f:19b7` | `NetIdOnMyTeam(id)` | team game and that player's choice equals the local one | CONFIRMED |
| `358f:19fd` | `NetSyncPuppet(i, ob)` | applies the slot-*i* packet to puppet `ob` (§4.3) | CONFIRMED |
| `358f:1e19` | `NetShutdown` | close ECBs and socket (or the serial port) | CONFIRMED |
| `358f:1e76` | `NetToggleSend` | toggles DS:0x38FA and shows `IPX SndPckts ON/OFF`; no caller found | CONFIRMED (unused) |
| `358f:1ea4` | — | empty; called on Backspace, where the single-player ally keys are handled | CONFIRMED |

## 3. Setup and start (`NetSetup`, `358f:0959`)

Called from the main menu after a network game type is chosen (GAMEFLOW.md §4). CONFIRMED.

1. Clears the slot map and count. Types 4 and 5 open IPX on the socket (default **0x4003**, DS:0x38F8); failure shows `IPX initialization error` and returns 0. Type 6 opens the serial link (`33bb:0e9e`, NETCOMMS.md); failure returns 0.
2. **Player id** (DS:0x38FC): for IPX the last byte of the local node address; for serial `US_RndT()`. Two IPX machines whose node addresses end in the same byte collide (quirk).
3. Shows `Player ID number %d`, then `IPX address %s` and `Communicating on IPX socket %X` (IPX) or `I/O Address %X IRQ%d` and `Communicating at %ld bps` (serial).
4. Title `CHOOSE A TEAM` (type 5, list `Team 1`, `Team 2`, `Team 3`) or `CHOOSE A PLAYER` (list `Agent`, `Terrorist 1`, `Terrorist 2`). Up/down move the highlight (wrapping).
5. In IPX types a hex key (0–9, A–F) opens a 4-digit entry for a new socket number, closes the socket and starts over.
6. Enter/fire/button: shows `Get Ready!`, sets `mapon = 50` (the first NETLEVEL), stores the choice in DS:0xEF4F, returns 1. Esc: shuts the network down and returns 0.

There is no lobby, no host and no start handshake: each player starts on their own, and players find each other through packets (§4.2).

### 3.1 Characters and teams

The choice index (0–2) selects the puppet the **other** machines show and the starting weapons. In a team game the same index is the team, so team-mates look alike. CONFIRMED (tables DS:0x38FE–0x3920).

| Choice | Character / team | Puppet | Class | Stand / walk / shoot / pain states | Start weapons (§5.3) |
|---|---|---|---|---|---|
| 0 | Agent / Team 1 | `SpawnAlly(x, y, 2)` | 5 (ally) | DS:0x2A46 / 0x2A46 / 0x2AC6 / 0x2AA6 | assault shotgun + Uzi |
| 1 | Terrorist 1 / Team 2 | `SpawnStand(1, x, y, 2)` | 4 (soldier) | 0x17BE / 0x17CE / 0x184E / 0x182E | assault shotgun + Uzi |
| 2 | Terrorist 2 / Team 3 | `SpawnStand(9, x, y, 2)` | 12 (black suit) | 0x1C3E / 0x1C4E / 0x1CCE / 0x1CAE | assault shotgun + weapon 2 |

Puppet fields: `temp1` = choice, `temp2` = player id, `temp3` = weapon. CONFIRMED.

## 4. Per-frame exchange

### 4.1 The packet (24 bytes, segment 0x4139)

| Offset | Size | Field | Status |
|---|---|---|---|
| +0x00 | 1 | event: 0 none, 1 moving, 0xFF "stopped" nudge, 2 died, 3 used, 4 fired, 5 burned to death | CONFIRMED |
| +0x01 | 1 | `mapon` | CONFIRMED |
| +0x02 | 1 | player id (+0x03 unused; receivers read the byte) | CONFIRMED |
| +0x04 | 4 | player x (16.16) | CONFIRMED |
| +0x08 | 4 | player y | CONFIRMED |
| +0x0C | 2 | player angle | CONFIRMED |
| +0x0E | 2 | current weapon | CONFIRMED |
| +0x10 | 2 | target id (event 4; −1 = none) or killer id (events 2, 5) | CONFIRMED |
| +0x12 | 2 | choice (character or team) | CONFIRMED |
| +0x14 | 1 | receiver-side lock while a copy is taken | CONFIRMED |
| +0x15 | 1 | receiver-side "already applied" flag | CONFIRMED |
| +0x16 | 2 | score (frag count, low word) | CONFIRMED |

### 4.2 Sending (`NetFrame`, `358f:0eea`, from *PlayLoop* every frame)

Runs only while DS:0x38FA (sending enabled, 1 by default) is set. CONFIRMED.

1. Re-arms receive ECBs when asked (after a death or level change).
2. If a respawn is pending: `NetRandomSpot`, then `NetResetPlayer` (§5).
3. Event: if none was set this frame, 1 when `controlx` or `controly` is non-zero; otherwise, once after a new player joined, 0xFF (forces a packet the newcomer can see).
4. Fills mapon, id, position, angle, choice, weapon and score.
5. Sends: right after a respawn, one **broadcast**. Otherwise a broadcast every **0x118 tics** (4 s) as a keep-alive. Then, if the packet differs from the last one sent (DS:0xEFB3), it is copied there and **sent to every known player individually**. A player who stands still and does nothing sends only the keep-alive.
6. New players (see 4.3): for each newly seen slot, shows `PLYR ID %d IN`, spawns its puppet at the packet's position (table in §3.1), sets the angle and direction, `hitpoints = 100`, `ticcount = 1`; sets `killcount` to the number of remote players (the status-bar kill counter shows players in network games); arms the 0xFF nudge.
7. Clears the event byte.

Events 2, 3 and 5 also call `NetFrame` immediately (§6), so they are sent in the frame they happen.

### 4.3 Receiving

`IpxReceiveEsr` (`358f:04d7`, interrupt time): ignores packets from the local id; looks up the sender's node byte in the slot map (DS:0xED7D, 256 bytes). An unknown sender gets the next slot number (1, 2, …, DS:0xEF51), marked "new" (negative) for `NetFrame`. The packet is copied to the slot's record (DS:0xEC5B + slot·24) unless the record is locked. Only the latest packet per player is kept: two packets inside one frame lose the first one's event (quirk). CONFIRMED.

`NetSyncPuppet(i, ob)` (`358f:19fd`) runs from *DoActor* for the actor at list position *i* whose class is 4, 5 or 12, so **puppet *i* is assumed to be slot *i*** (objlist[0] is the player; NETLEVELs have no enemies, so puppets follow in join order; a fire actor spawned before a join breaks the mapping, quirk). It copies the record, then: CONFIRMED

- Sets class, position, tile, angle, direction, choice, id and weapon from the packet; `ticcount = 1`.
- **Team score**: in type 5, if the puppet is a team-mate whose score is higher than the local score, the local score takes that value (team-mates share the highest score).
- If the packet's `mapon` differs from the local one: the puppet is moved to (0, 0) with `ticcount` 0, and `Activity on %d` (their floor number) is shown unless a message is already on screen (DS:0x46A4 is the message timer).
- Otherwise by event:
  - **2 / 5 (died)**: for 5, flames on the puppet (`SpawnFlame`, 2390:0bd5). Clear the slot's "alive" flag (DS:0xEF75 + 2·slot), *KillActor*(puppet). Kills table `[killer slot][victim slot] += 1`. If the killer is the local player, or in type 5 a team-mate of the local player, local score += 1.
  - First packet after a spawn or death (alive flag clear): make the puppet shootable, stand state, set the alive flag.
  - **4 (fired)**: if the puppet is class 5, or the shot targeted the local player, or the weapon is 3 (flamethrower) or 4 (grenade): `NetRemoteFire`, then set the slot's anim flag (DS:0xEF5D + 2·slot).
  - **3 (used)**: the puppet's object is copied over the player's, *Cmd_Use* runs with DS:0xEF93 = 1 (no button-held test, no re-send), then the player is restored. So remote players open doors, push walls and press switches on every machine. NETLEVELs have no doors, push walls or animated walls, so in practice this matters for **exits**: a player taking an exit completes the level on every machine on that floor, and the floor moves on together. A remote Use of a medic wall heals the *local* player (quirk).
  - **1 (moving)**: walk state once (anim flag).
  - **0**: back to the stand state if the anim flag is set and the puppet is not in its pain state.

## 5. Spawning and respawning

### 5.1 When

- Level start: *SpawnPlayer* (`208c:1324`) in a network type calls `NetRequestRespawn` instead of placing the player at the map start; the first `NetFrame` places them. CONFIRMED.
- Death: *GameLoop* (`1823:0ea3`) calls `NetRequestRespawn` and `NetResetPlayer`, sets `playstate = 0` and restarts **without reloading the level** (the reload flag is 0), so the floor keeps its state. There is no death sequence, no game over and no high-score entry. CONFIRMED.

### 5.2 Where (`NetRandomSpot`, `358f:0d8c`)

Repeats `x = RandomNear(0, 64)`, `y = RandomNear(0, 64)` (four `US_RndT` calls) until `actorat[x][y]` is empty, not all four neighbours' `actorat` are non-zero, and no static object stands on the cell. The player is placed at the cell centre with `areanumber = plane0 − 140`; the angle is kept. Nothing checks that the cell is inside the playable area. CONFIRMED.

### 5.3 Loadout (`NetResetPlayer`, `358f:1796`)

`health = starthealth[difficulty]` = **800** for types 4–6 (DS:0x32EC); magazine 30; option bits 3; `gamestate+0x26` = 250; *ResetInventory*; then by choice: class 12 (choice 2) owns weapons 0 and 2 with weapon 2 selected; others own 0 and 1 with weapon 1 selected. `+0x30` cap 100, `maxammo` 200, `maxhealth` 100. Dropped-weapon flags cleared. Player `temp1` = choice, `temp2` = id, `temp3` = weapon. CONFIRMED.

## 6. Combat

### 6.1 Firing

*GunAttack* (`208c:14e0`) in network types. CONFIRMED.

- Grenade (weapon 4) and flamethrower (3): report event 4 with target −1, fire normally, and stamp the projectile's `temp2` with the local id.
- Hitscan weapons: the usual closest-visible-shootable search. No target: event 4, target −1. A target: event 4 with the target's id (`temp2`), and `NetHitPuppet(target)`, which only puts the puppet in its pain state (unless it is the player or a team-mate in type 5). No damage is dealt locally and nothing else of *GunAttack* runs.

On the receiving side `NetRemoteFire` (`358f:1283`) switches on the puppet's weapon:

- 0–2: if the shot targeted the local player, or the puppet is class 5, the puppet enters its shoot state. Its attack frame runs *T_Shoot* against the local player with the normal enemy rules (line of sight, hit chance by distance and player speed, damage `US_RndT() >> 2/3/4` by distance).
- 3: `SpawnFlame`-style projectile from the puppet (`30d6:0493`), owner id = the shooter's id, target = the local player.
- 4: the puppet's object stands in for the player while the grenade is thrown (`2f4d:0ff3`), owner id stamped.

*DamageActor* (`1e43:174d`) in network types calls `NetHitPuppet` and returns, so nothing the local player does can hurt a puppet directly. CONFIRMED.

### 6.2 Taking damage and dying

*TakeDamage* (`208c:0494`) in network types: CONFIRMED

1. Type 5: no damage from a team-mate (`attacker->temp1 == local choice`).
2. Damage only if the attacker's player targeted the local player (`NetTargetsMe`) or the attacker is a flame (class 18) or grenade (class 19); everything else is ignored.
3. While `health >= 30` the points are multiplied by `difficulty/2 + 1` (3 for N and T, 4 for M). No further difficulty multiplier. Armor halves as usual. No blackout roll.
4. At 0: `NetPlayerDied(attacker)` (`358f:182c`): kills table `[attacker slot][0] += 1` (`[0][0]` when the attacker id is the local id: suicide); event 5 if the attacker is a flame, otherwise 2; killer id = `attacker->temp2`; send at once.

*ProjectileTryMove* (`2390:0006`) in network types: a shootable actor with the projectile's own `temp2` does not block it, and the local player's cell blocks a projectile whose `temp2` is not the local id. Grenade splash hurts the local player by `((US_RndT() >> 3) + 30) / 2` and a network grenade has no wall effects (already in ACTORS.md §10.1). CONFIRMED.

### 6.3 Opening the menu

*US_ControlPanel* (`1618:061a`) during a network game calls `NetPlayerDied(player)`: opening the menu is announced as a suicide (KILLS `[0][0]`, event 2 with the local id as killer). Nothing happens locally: the player is not killed or moved, and on the other machines the puppet dies and stands up again where it was with the next packet. The game does not pause, and nothing is received while the menu is up (NETCOMMS.md). CONFIRMED.

### 6.4 Dropped weapons

*KillActor* (`1e43:1377`) in network types calls `NetDropWeapon(ob, x, y)`: a puppet with weapon 1–4 drops pickup 11, 5, 12 or 13 respectively (`PlaceItemType`), at most one of each kind on the floor (flags DS:0xEF53 + 2·weapon, cleared by `NetClearDrop` when picked up and on respawn). *KillActor* scores nothing and leaves `killcount` alone in network types. CONFIRMED.

## 7. Levels and the KILLS table

- The first network level is slot 50 (`NETLEVEL 5`). Taking an exit completes the level **for that player only** (`Cmd_Use` skips *TryExit* for stairs and ladders in network types; the elevator's *TryExit* passes because `killtotal` is 0). *GameLoop* calls `NetLevelDone`, then `mapon += 1` and `health += 35` as in single player. Slots 50–59 are NETLEVEL 5, 8, 9, 10, 6, 7, 4, 2, 3, 1; slot 59 has no exit. Players on different floors see each other's activity message only. CONFIRMED (flow), LIKELY (intent).
- `NetLevelDone` (`358f:1720`): cancels pending sends, forgets all players (slot map cleared; they re-join as their packets arrive), requests respawn and reload, and shows the **KILLS** table. The Alt+W warp to a level ≥ 50 does the same. CONFIRMED.
- KILLS table (`358f:13c5`): a 12 × 12 grid, row = killer slot, column = victim slot; slot 0 = the local player, labelled `U`; the others are labelled with their local slot numbers; only slots that have a player are filled (`%2d`); wait for a key. The main menu can also show it (`1618:13cb`). The table lives in segment 0x4127 and survives level changes. CONFIRMED.
- Score in network games is the frag count (+1 per kill by you or, in type 5, your team); *GivePoints* adds nothing. CONFIRMED.

## 8. Quirks

- Player ids are the last byte of the IPX node address; collisions are not detected.
- Puppets are matched to slots by actor-list position.
- Only the latest packet per player per frame is kept; lost or overwritten packets lose their events (a missed death means a missed kill; a missed Use means a door that opened on one machine only).
- Doors, items, fires and pushed walls are local except through replayed Use events; picked-up items stay on other machines.
- `NetRandomSpot` can place a player in a sealed-off part of the map.
- Opening the menu is announced as a suicide.
- A remote player's Use of a medic wall heals the local player; their Use of an exit ends the level for everyone who receives it.

## 9. Open questions

Answered by NETCOMMS.md: nothing reads DS:0xEFCD; DS:0x46A4 is the message timer; the serial link sends 16-byte packets without score and target and has no keep-alive (NETCOMMS.md §6.6); nothing lowers the 12-player limit, and the receive handler never checks it (a 12th remote player overwrites memory).

- Q-NET-5: DOSBox-X trace of a two-machine IPX game to confirm the event timing (advised).
