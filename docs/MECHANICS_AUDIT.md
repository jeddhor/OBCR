# Mechanics audit: Operation Body Count features in OBCR

Baseline: `../Wolfenstein_3D_vs_Operation_Body_Count_Complete_Comparison.md` (workspace root, research date 2026-11-05). That document lists OBC's mechanics from the manual, packaging and player reports, without running or disassembling the game. This audit checks each mechanic against what `COUNT.EXE` (release OBC-A) actually does (docs/re, cited by section) and against the OBCR code.

Audit date: 2026-11-05.

**Status legend**

| Mark | Meaning |
|---|---|
| ✅ | Ported from the disassembly; behaves as the original's code |
| ✅ ≠ | Ported; the original's code differs from the manual or player report, and OBCR follows the code |
| 🔧 | Wrong or missing before this audit; fixed by it |
| ❌ | Not implemented yet |
| n/a | Not in the OBC-A floppy release (the user's data) or not a game mechanic |

Where the comparison and the code disagree, the code wins: OBCR reproduces the shipped program, not its marketing.

---

## 1. Progression and campaign (comparison §3)

| Mechanic | Original (`COUNT.EXE`) | OBCR | Status |
|---|---|---|---|
| 40-floor linear campaign | `mapon` 0–39, strictly `mapon + 1` per exit; no episodes; maps 40–49 unused (GAMEFLOW §5.2, §5.7) | `World::advance_level`, game screen floor flow | ✅ |
| 50 % kill quota for the exit | *TryExit* `30d6:1bc0`: `killcount >= killtotal / 2` (integer division), else `UNFINISHED BUSINESS`; map 39 exits do nothing (PLAYER.md §3.6) | `player/interact.rs` | ✅ |
| Red / yellow / green status lamp | Kill lamp at (116,188): palette 1 red below half, 2 yellow at half, 0x0E green at all (PLAYER.md §5.2; colours checked in `gamepal`) | `hud.rs` | ✅ |
| Exit kinds (ladder, stairs, elevator) | Tiles 0x71/0x72 stairs, 0x73 elevator; sequence F2 after sewers 0–4, F5 after map 22, else F3/F4 (GAMEFLOW §5.2–5.3) | game screen `exit_sequence` | ✅ |
| End-of-floor statistics | **None**: `ex_completed` (1823:0f6f, read in this audit) goes sequence → palette 3 → `ClearFloorItems` → `mapon++`, `health += 35` | same | ✅ (PLAYER.md corrected) |
| Second building (map 23) | Message `UN BUILDING 2`; no ally codes on maps 23–39 (LEVEL_CODES: codes 224–227 only on maps 5–22) | `play_loop_start`; map data | ✅ |
| Hostage rescue | Story only; no hostage mechanic in the code | — | n/a |

## 2. Controls (comparison §4)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Move, turn, strafe (Alt), run (Shift), fire (Ctrl), use (Space), weapons 1–5 | PollControls, ControlMovement (PLAYER.md §3.1–3.3) | `controls.rs` original profile; modern and gamepad profiles add strafe-while-turning (ADR-014) | ✅ |
| A switch ally, S stay & switch, F follow/disperse | CheckKeys → ally keys (ALLIES.md §5.1) | `allies::handle_keys` | ✅ |
| Tab map | 0 → 1 → off on the next refresh (RENDERER.md §9.2) | `draw_overhead_map` | ✅ |
| W wipe visor | `ClearScreenSplats` | `check_keys` | ✅ |
| H exact health | `30d6:0779`: icon pic 24 at (viewwidth−40, 2), `%3d` in font 1 at (viewwidth−27, 0), black shadow, flat 0xFE below 50 health, font colours otherwise | `draw_health` | 🔧 (was a placeholder) |
| C crosshair | `30d6:07d0`: 11 + 11 dots every other pixel through the view centre, colour 0xE1 | `draw_crosshair` | 🔧 (was a placeholder) |
| F5 floors/ceilings off, F6 on | `floormode` DS:0x33D2 = 0 / 2, saved in CONFIG.DAT | game screen; `settings.game.floor_textures` | 🔧 (keys were reversed; now persisted) |
| F8 tremors | Toggles the screen shake (RENDERER.md §8.4) | `check_keys`, game screen | ✅ |
| F1 sound, F2 music, F9 quick save, F10 quit prompt, Pause | CheckKeys / `US_ControlPanel` (GAMEFLOW §4.1, §11.3) | game screen | ✅ (F10 🔧 added) |
| Esc ignored during a tremor | CheckKeys: no control panel while the shake counter and TREMORS are both set | game screen | 🔧 (added) |
| Alt+P/S/A CD track keys | CD-audio code exists but is never called; `cdmusic` only turns FM music off (AUDIO.md §8) | — | n/a |
| Vertical look, jump, crouch | Not in the game | — | n/a |

## 3. Squad and body switching (comparison §5)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Allies placed per floor | Plane-1 codes 224–227, maps 5–22 only; `init_allies` counts them at PlayLoop start | `allies.rs` | ✅ |
| Follow: allies do not shoot | Follow runs the shared `T_Chase` whose class-5 attack entry is a bare return (ALLIES.md §3.2) | `allies.rs` → `actors::t_chase` | ✅ |
| Disperse: roam and attack | `T_Ally` / `T_AllyShoot` (ALLIES.md §4) | `allies.rs` | ✅ |
| Switch / stay & switch | Body swap by `CopyActorState`; S freezes the body left behind (ALLIES.md §5) | `allies.rs` | ✅ |
| Ally kills | No score, **no kill count**, so they do not advance the 50 % quota (`allyfiring` DS:0x2BC8; ALLIES.md §4.4, KillActor tail) | `give_points`, `kill_actor` | ✅ ≠ (the comparison leaves this open) |
| Player death with allies alive | **Game over.** *TakeDamage* `208c:0494` (read in this audit) sets `playstate = 2` at health ≤ 0 with no ally switch; body-switching is not a lives system | game screen death flow | ✅ |
| Last ally dies | Event-box portrait, count 0, `NO ALLIES!` on A/S; nothing else | `hud.rs`, `allies.rs` | ✅ |
| Allies between floors | None carried over: each floor spawns its own | level setup | ✅ |

## 4. Weapons and ammunition (comparison §6)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Slot 1 assault shotgun, unlimited | Weapon 0: hitscan, no ammo, one shot per ≈80 tics (attackinfo DS:0x0F66), status pic 34 | `player/weapons.rs` | ✅ (renamed from "pistol" in this audit) |
| "Slugs penetrate many obstacles" | No special rule: weapons 0–2 share `GunAttack`, whose line check passes see-through tiles (plane 1 0x69–0x6B, grates) like every gun | same | ✅ ≠ |
| Uzi, Galil | Weapons 1 and 2, hitscan; damage bonus 20 / 50 (PLAYER.md §3.10) | same | ✅ |
| Flamethrower | Projectile (class 0x12, speed 0x3000) that ignites burnable statics and burns actors (`BurnTile`) | `spawn_player_flame`, `ignite` | ✅ |
| Grenade launcher | Straight projectile, speed 0x2000/tic, no arc, no fuse; splash box and `ExplosionTileEffects` (PLAYER.md §3.10) | `spawn_grenade`, `grenade_splash`, `explosion_tile_effects` | ✅ |
| Ammo units | One shared 30-round magazine; per-weapon **clip counts** capped at 19; a grenade launch empties the magazine, so one clip = one grenade; crates give 1–7 clips or 1–4 grenades (PLAYER.md §3.8, §4.2). The manual's "30-round clips" matches; "19 shots per fuel bottle" and "three grenades per group" do not | same | ✅ ≠ |
| Weapon drops from enemies | `DropItem` per class (ACTORS.md) | `drop_item` | ✅ |
| Self-damage | Explosion: 300 on the blocking tile, else `US_RndT() % 200` within the 8×8 box | `explosion_tile_effects` | ✅ |
| Demo-version taser | Not in OBC-A | — | n/a |

## 5. Destruction, fire, lighting, tremors (comparison §7)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Grenades destroy walls | The blocking wall is removed (tile, info, `actorat`; debris; 1/20 fire) unless info 0x7D (→ scorched 0x4D); other walls in the box: info 0x56/0x57/0x58/0x62 destroyed, 0x7D spared, the rest scorched to 0x4D | `explosion_tile_effects` | ✅ |
| Protected tiles | Map border ring (x or y 0 or 63); info 0x59/0x62 walls; exit tile 0x71; animated walls in the box | same, plus exits 0x72/0x73 (ADR-016) | ✅ ≠ (animated walls 🔧 wired) |
| Doors and blasts | The door a grenade hits is destroyed; other doors in the box become scorched wall 0x4D | The hit door is destroyed and never closes again; other doors keep working (ADR-016) | ✅ ≠ |
| Pickups destroyed by blasts | Statics in the [−2,+1] box become debris | same | ✅ |
| Breakable glass | 0x39–0x3C / 0x75–0x78: shots break panes in three frames (shootable-wall search), blasts set the broken tile; broken panes stay solid but see-through | `vis.rs` wall search, `post_actors` | ✅ |
| Bullet marks | Chip walls (0x13…0x7D) advance one damage stage per frame while fire is held at them (RENDERER.md §9.1) | `vis.rs`, `start_wall_hit_at` | ✅ |
| Fire: ignition, spread, burnout | Fire actors (class 21) from burnable statics; spread by TryWalk ignition and 1/256 per move; random burnout (ACTORS.md §7) | `actors.rs` | ✅ |
| Generator / power switch | Tiles 0x5F/0x60 toggle the dark palette (`GENERATORS ON/OFF`); a floor starts dark if it contains tile 96 | `cmd_use`, `World::lighting` | ✅ |
| Blasts and power equipment | No special case: a generator tile in a blast box is scorched (0x4D) and stops working; the palette stays as it was | emerges from the code | ✅ |
| Tremors | Blast: shake 8 frames, quake 10 frames (darkness flicker; 1/30 chance of the dark palette when it ends) | `explosion_tile_effects`, `World::frame` | 🔧 (the actors port had a private counter; tremors never ran) |
| Per-light dynamic lighting | Not in the game: palette swap and distance shading only | — | n/a |

## 6. Traps and hazards (comparison §8)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Mines and booby-trapped bodies | One item type (20): plane-1 codes 61 (body, sprite 39) and 62 (small grey object, sprite 40). Touching it: `BOOBY TRAP`, blast actor ahead, `TakeDamage(US_RndT())`, knock-back (PLAYER.md §4.1) | `get_bonus` | ✅ |
| Fake-dead enemies | Class 11 prone soldier (sprite 128) and the other disguised classes 8–10 stand up on sighting | `actors.rs` | ✅ |
| Damaging walls | Barbed-wire tiles 0x49–0x4C: 5 damage and 1/5 splat per blocked move while facing them | `try_move` | ✅ |
| Fire hazard | Fire actors hurt on contact (100 at distance 0, 5 at 1) | `actors.rs` | ✅ |
| Darkness | Dark palette until the generator is switched on | see §5 | ✅ |
| Visor blood and sludge | Up to 19 splats (sprites 640–655), `W` wipes | `add_screen_splat`, `draw_splats` | ✅ |
| Low-health blackout | Health ≤ 35: 1/15 chance per hit of a 50-step fade to black (game frozen) | game screen | 🔧 (the event was ignored) |
| Electric flying creature | Class 6 saucer drone: lightning within one tile, explodes on death, no corpse (ACTORS.md §8.4) | `actors.rs` | ✅ |

## 7. Enemies and bosses (comparison §9)

All classes in ACTORS.md are ported with their state tables read from `COUNT.EXE`: rats (3, 7), soldiers and disguised soldiers (4, 8–11), toilet terrorist (10), saucer (6), black suit (12), Victor (13, whose death chain sets `ex_victorious`), gunner (14), red vest (15), red giant (16), fat man and manhole (17, 22), swamp creature (23) with slime balls (20), fire (21), face apparition (26). Enemies shoot allies (`T_ShootAlly`), open door-look walls when they bump them (🔧 wired in this audit), and route through blasted gaps because they read the live `tilemap`. ✅

## 8. Map, sensor, HUD (comparison §10)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Twelve status indicators | Status bar layout (PLAYER.md §5.2) | `hud.rs` | ✅ |
| Pickup messages | `GameMessage`, 125 tics, shadowed, **in the font's own colours** | `hud.rs` | 🔧 (were drawn flat white) |
| Live map (Tab) | 24×24 tiles around the player, one pixel per tile, level-start wall bitmap, no fog (RENDERER.md §9.2) | `draw_overhead_map` | ✅ |
| Heat sensor | Item 10 sets the ThermoSensor for the floor: enemies red (palette 1), allies yellow (2), player blinking white/green (0xFC/0x0F); bogus mode also shows enemies | same | ✅ |
| "Reveals hidden rooms" | The map has no fog of war: every wall in the window shows, so secret areas are visible on any map | same | ✅ ≠ |

## 9. Level design, rendering (comparison §11–12)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Push walls | Plane-1 code 98, Use; coexist with blasts | `animwalls.rs` | ✅ |
| Locked doors | Four lock kinds, all cosmetic | `doors.rs` | ✅ |
| Textured floors and ceilings, several sets per map | Floor/ceiling set per plane-0 area code (`plane0 − 144`), sewer overrides | `view3d.rs` | ✅ |
| See-through walls (grates, windows) | Plane-1 0x69–0x6B; second ray pass | `view3d.rs` | ✅ |
| Exterior views | Wall textures behind see-through tiles; no outdoor geometry | same | ✅ |
| View size | 12–20 (XS–XL) | menu VIEW SIZE | ✅ |
| Widescreen, high resolution, smooth motion | Not in the original | Enhanced mode | (OBCR addition) |

## 10. Audio and cinematics (comparison §13)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Digitized effects, AdLib music | AUDIOMUS at 9,009 Hz with SB Pro panning; IMF music per floor (AUDIO.md) | `obc-audio`, `sound.rs` | ✅ |
| Floor sequences F2, F3, F4, F6 | FLC players with sound cues | `sequence.rs` | ✅ |
| F1, F5, F7, CD FMV | Not shipped in OBC-A | skipped as the original does | n/a |
| CD music and its mixer rows | Never used by OBC-A | — | n/a |
| AUDIO MIX menu | Two rows: FM music volume, sound-effect volume | OPTIONS screen (master, effects, music) | ✅ ≠ (different screen, same controls) |
| Sound device choice (PC speaker / AdLib / digitized) | Set by SETUP and CONFIG.DAT | OPTIONS → SOUND EFFECTS | 🔧 (added) |

## 11. Multiplayer (comparison §14)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Game types N (network), T (teams), M (modem) | `difficulty` 4–6; rules in segment `358f`, serial driver in `33bb` ([re/NETWORK.md](re/NETWORK.md), [re/NETCOMMS.md](re/NETCOMMS.md)) | GAME TYPE offers N, T, M; `obc-sim::net` ports the rules; `obc-net` carries them over UDP (ADR-017) | ✅ ≠ (UDP instead of IPX and modem) |
| Synchronisation | No lockstep: each machine owns its player and sends a 24-byte state packet per frame; others are puppets; damage decided by the victim | same | ✅ |
| Players, characters, teams | Up to 12; CHOOSE A PLAYER (Agent, Terrorist 1, Terrorist 2) or CHOOSE A TEAM (Team 1–3); the choice sets the sprite others see and the starting weapons | same; M is two players | ✅ |
| Team rule | In type T no damage from team-mates; team-mates share the highest score | same | ✅ |
| Damage | Attacker's shot replayed as the puppet's enemy attack on the victim's machine; ×3 (×4 in M) while health ≥ 30; 800 health | same | ✅ |
| Death and respawn | Death announced to all (kills table, frag to the killer); respawn at a random free cell on the same floor, no reload | same | ✅ |
| Levels | NETLEVELs 50–59; an exit taken by one player ends the floor for everyone who receives the Use; the KILLS table between floors | same | ✅ |
| Opening the menu | Announced as a suicide | same | ✅ |
| Socket choice | Hex keys on the setup screen change the IPX socket (0x4003) | the same keys change the UDP port (16387 = 0x4003) | ✅ ≠ |
| Joining over the Internet, lost packets, player ids | Not in the original (IPX LAN only; lost events are lost; ids from the node address) | call an address, gossip, redundancy, collision-free ids | 🔧 (added) |

## 12. Saving, difficulty, scoring (comparison §15–16)

| Mechanic | Original | OBCR | Status |
|---|---|---|---|
| Difficulty 1–3 | Damage taken/dealt multipliers and skill tiers (PLAYER.md §4.4) | sim | ✅ |
| Random (R) | `RandomNear` ±24 tiles, 10 tries, for actors and bonus items | `spawn_new_obj`, `spawn_static_scan` | ✅ |
| Random differs per run | `US_Startup` → `US_InitRndT(true)`: the RNG index starts from the clock | each new game seeds the index from the clock; saves keep it | 🔧 (every game used to start at index 0) |
| Nine save slots + AutoSave (slot 10) | `svgm0–8`, `autsv`; AutoSave before every floor change; LOAD only out of game | `saves.rs`, slot menus | ✅ |
| QuickSave F9 | To the slot last saved from the menu; original messages | game screen | ✅ |
| High scores | Seven entries, floor reached, not in bogus mode | `HighScores` | ✅ |
| Lives, treasure, keys as score items | None | — | n/a |

---

## 13. The comparison's verification checklist (§21)

"Code" answers come from the disassembly and are what OBCR does; "trace" means a DOSBox-X run is still needed to confirm the code reading.

| Test | Answer | Source |
|---|---|---|
| T01–T02 | Exit needs `killcount >= killtotal / 2`, integer division: 31 enemies need 15 | Code (TryExit) |
| T03 | All dead: lamp green; no bonus, no stats screen | Code |
| T04 | Ally kills count neither as kills nor score, so they never help the quota | Code |
| T05 | The blocking wall is cleared (tile, info, collision), debris left; sight and movement update at once | Code |
| T06 | The outer ring is immune | Code |
| T07 | Actors use the live tilemap, so they path through new gaps | Code |
| T08 | Hit door destroyed; nearby doors scorched into walls. OBCR leaves nearby doors working (ADR-016) | Code (trace advised) |
| T09 | Only exit tile 0x71 is protected; **0x72 and 0x73 can be scorched**, leaving no exit. OBCR protects all three (ADR-016) | Code |
| T10 | Pickups in the box become debris | Code |
| T11–T12 | Fire from burnable statics; spreads by walking and 1/256 per move; burns out randomly | Code |
| T13 | Own flame cannot hit the player (no target, the player is not shootable); fire actors and blasts can | Code |
| T14 | Allies in the 8×8 blast box are killed outright | Code |
| T15–T18 | ALLIES.md §3–5 | Code |
| T19 | Death ends the game even with allies alive | Code (read in this audit) |
| T20–T21 | Nothing special when the last ally dies; allies never change floors | Code |
| T22–T23 | Sensor per floor (cleared at PlayLoop start) | Code |
| T24 | The simulation runs while the map is open | Code |
| T25 | F5 off, F6 on, saved in CONFIG.DAT | Code |
| T26 | Per-cell set from the plane-0 area code | Code |
| T27 | Broken panes stay solid but see-through | Code (trace advised) |
| T28 | Chip walls advance one stage per frame of held fire; the change is permanent (tile number) | Code |
| T29–T30 | Generator swaps the palette; a blast scorches it into a dead wall | Code |
| T31 | Tremors: screen shake after blasts and booby traps; F8 turns the shake off | Code |
| T32–T33 | Booby-trap item 20 (bodies and mines alike): touch to trigger | Code |
| T34 | Disguised and prone soldiers stand up when they sight the player | Code |
| T35 | Splats stay until W, the ally switch or the next floor | Code |
| T36 | No shotgun penetration | Code |
| T37–T38 | Grenade straight, 0x2000/tic; splash box [−2,+1] for 100 − distance, kill box 8×8 | Code |
| T39–T42 | See §4 ammo row | Code |
| T43–T44 | Placement moves only; enemy types and the map stay; the seed comes from the clock | Code |
| T45–T47, T59 | Saves hold the whole world (blasted walls, fires, allies, lighting) | OBCR save tests |
| T48 | Free-for-all: random spawns, victim-side damage, deaths announced; levels are not synchronised beyond replayed Use | Code; two `obcr` processes over UDP |
| T49 | Team mode: no friendly damage, shared score | Code; `net_game` tests |
| T50 | 12 players including yourself (the original never checks: a 12th remote player overwrites memory) | Code; OBCR enforces 11 remote |
| T51 | Games on different sockets (ports) never see each other | Code |
| T52 | Modem/serial: two players, 16-byte packets without score; OBCR uses a direct UDP connection | Code (NETCOMMS.md) |
| T53, T58, T65 | Version questions; only OBC-A is supported | — |
| T54–T57, T60–T64 | Covered by the data formats, renderer and difficulty notes | Code |

## 14. Quirks the player may notice

These are faithful to the original's code; ADR-008 keeps them in Classic play. Each could become an opt-in fix flag.

- Kills by allies do not count toward the 50 % quota, so a squad that does most of the killing can leave the player short.
- "Fuel Bottle" from a supply crate is applied only when it fills the fuel clips to the cap (PLAYER.md §4.2).

### Fixed in OBCR (ADR-016)

Blasts could leave a floor with no way out in the original. OBCR changes this in every mode:

- Blasting a stairs (0x72) or elevator (0x73) exit scorched it. All three exit tiles are now spared.
- A grenade near a door turned the door into a scorched wall, which could seal the only route to the exit. Nearby doors now keep working.
- A door destroyed by a direct hit while open could later "close" into an invisible, unusable barrier. A destroyed door now never closes.

## 15. Remaining work from this audit

1. **Multiplayer (M13)**: done (ADR-017). A DOSBox-X two-machine IPX trace would confirm event timing.
2. DOSBox-X traces for the items marked "trace advised" and for ACTORS.md §14.
