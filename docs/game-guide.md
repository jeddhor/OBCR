# Playing Operation Body Count

A player's guide to *Operation Body Count* (Capstone Software, 1994) as OBCR plays it. It covers what the game is, how a floor is won, what the weapons, items, enemies and allies do, and the handful of places where the original behaves in ways nobody would guess.

Everything here comes from reading the original executable's code, not from the box or from memory. Where the code was read but the behaviour has not yet been watched in the original running in DOSBox-X, the guide says so. Where the manual and the code disagree, OBCR follows the code, and so does this guide.

Keys are given as **modern / original**: the first is OBCR's default modern scheme (WASD and mouse), the second the 1994 scheme (arrow keys). Every one of them can be changed under **OPTIONS → KEY BINDINGS**, or from the help page (**H** in the main menu), which now opens that same screen.

---

## The premise

> *The Government has launched a top-secret military offensive against the foreign troops occupying the U.N. towers. Code name...*
>
> — the original's opening screen

You go in from below: up through the sewers, through a maintenance level, into the first tower, floor by floor to its top, then across into the second tower and up again to its roof, where the man running the show is waiting. The manual calls him Victor; the code only knows that when he dies, the game is won.

There is nobody to rescue, escort or protect, except the squad you bring with you.

## How the game is laid out

| Floors | Where | Notes |
|---|---|---|
| 1–5 | The sewers | Dark, wet, rats. Ladders out. |
| 6 | Maintenance level | Your first allies appear here. |
| 7–23 | UN Building 1: lobby, floors 2–16, the top | Allies on every floor. |
| 24–40 | UN Building 2: floors 1–16, the top | No allies at all. Floor 24 greets you with `UN BUILDING 2`. Victor is on 40. |

Progression is strictly linear: finishing a floor always takes you to the next one. There are no episodes, no secret levels and no branching. The map archive holds 61 slots in all: the 40 above, ten unused "credits" rooms, and ten network arenas (slots 50–59) used only by multiplayer.

### Leaving a floor

Each floor has exits built into the walls: **ladders** in the sewers, **stairs** and **elevators** above. Face one and press **Use** (**Space** or **E** / **Space**).

The exit only works once you have killed **at least half of the floor's enemies**. Try earlier and you get **`UNFINISHED BUSINESS`** and stay where you are. Half means integer half: a floor with 31 enemies needs 15.

The status bar has a small lamp beside the ally indicator that tells you where you stand:

| Lamp | Meaning |
|---|---|
| Red | Fewer than half dead. The exit will refuse you. |
| Yellow | Half or more. You can leave. |
| Green | Everyone is dead. No bonus for it; it just looks tidy. |

Kills made by your allies count toward the half. They did not in the original, which could leave a player short on a floor where the squad did most of the shooting; OBCR changed that, and allies still earn you no points ([ADR-018](DECISIONS.md)).

Finishing a floor gives you **35 health**, saves the game to the AutoSave slot and plays a short sequence: climbing out of the sewer, taking the stairs or riding the elevator. There is no end-of-floor statistics screen; the original never had one.

The last floor, 40, has no working exit. The only way out is through Victor.

### Death

There are no lives. Dying ends the game, whatever allies you have left: you get the death sequence, the high-score table if you qualify, and the menu. Save often. The original's own advice is below.

## Game types

The menu has no separate difficulty setting. **GAME TYPE** picks one of seven, shown as a letter:

| Type | What it is |
|---|---|
| **1** | For novices, says the manual. You also start with 200 armor, and your shots hit four times harder than on 2. |
| **2** | For those who have played before. The default. |
| **3** | "Uhhh, this is hard." You take eight times the damage while healthy (four times below 30 health) and your shots do the least. |
| **R** | Random. Enemies and items are scattered up to 24 tiles from where the map puts them, so a floor can bury you in a corner with half its population. The manual: *"If you can do well on the Random Play type, consider yourself a Body Count Expert."* |
| **N**, **T**, **M** | Network, Network Teams and Modem: multiplayer, covered in [multiplayer.md](multiplayer.md). |

What actually changes with the type, from the code:

| Type | Start health | Start armor | Damage you take (health 30+) | Damage you take (below 30) | Damage you deal | Enemies placed |
|---|---|---|---|---|---|---|
| 1 | 400 | 200 | ×2 | ×2 | ×8 | the easy set |
| 2 | 400 | 0 | ×2 | ×2 | ×2 | easy + medium |
| 3 | 400 | 0 | ×8 | ×4 | ×1 | all three sets |
| R | 400 | 0 | ×6 | ×3 | ×2 | all three sets, scattered |

Health runs from 0 to 999 and armor too. Enemies' hit points also grow with the type.

## The screen

### The status bar

From left to right:

- **The event box**: a 32×32 window that shows the icon of what you just picked up, plays a short animation when an ally dies, and sparkles now and then.
- **Your weapon**.
- **Your squad**: a letter for the ally mode and the number of allies still alive.
- **The kill lamp** (above) and your **game type**.
- **Three bars**: health, ammo and armor. Each has 19 segments, coloured by how many are lit. Health and armor fill up at about 342, so a bar can look full while you are far from the 999 maximum. The ammo bar counts **clips**, not rounds (below).
- **Score**, **kills** and the **floor** number.

Messages appear in the top-left corner of the view for a couple of seconds: pickups, `UNFINISHED BUSINESS`, ally orders and so on.

### Overlays

| Key | What |
|---|---|
| **H** | Your exact health in a corner of the view (it turns flat-coloured below 50). |
| **C** | A dotted crosshair through the centre of the view. |
| **Tab** | The map: a small window, one pixel per tile, 24×24 tiles around you, drawn over the view while you keep playing. It has no fog of war, so secret rooms show on it. |
| **V** / **W** | Wipe the visor (below). |

### The visor

Blood and swamp slime splash onto your visor: up to 19 splats at once, from enemies hit at close range and from slime balls. They stay until you **wipe the visor** (**V** / **W**), switch bodies with an ally (plain Switch, not Switch & Stay), or reach the next floor.

## Weapons

You carry up to five, on **1** to **5**. The mouse wheel and the gamepad's shoulder buttons step through them.

| Key | Weapon | Ammo | Notes |
|---|---|---|---|
| **1** | Assault shotgun | none needed | Slow: one shot every second or so. Also how you chip at walls (below). |
| **2** | Uzi | 9 mm clips | Owned from the start, with two clips. Fires in threes while you hold the trigger, using four rounds for every three shots. |
| **3** | Galil | 7 mm clips | Same burst as the Uzi, more damage, twice the ammo per burst. Dropped by the men in black suits. |
| **4** | Flame thrower | fuel | A stream of fireballs that burn people and set burnable things alight. |
| **5** | Grenade launcher | grenades | Fires straight: no arc, no bounce, no fuse. It goes off on whatever it hits. |

**How ammo works.** All your weapons share one 30-round magazine; for each weapon you also carry a number of **clips**, up to 19. Selecting a weapon loads a full magazine if you have a clip for it, and when the magazine runs out a clip is used up. The ammo bar shows the clip count. A grenade launch empties the magazine, so one grenade costs one clip. Run a weapon dry and you drop to the next one down that still has ammunition.

Hitscan weapons (shotgun, Uzi, Galil) hurt more the closer you are, from 120–200 points point-blank down to a flat 20 at ten tiles and beyond, then scaled by the game type. There is no miss roll at long range.

**Grenades are dangerous at both ends.** The explosion kills every target in a box about eight tiles across outright, and it hurts you too if you are in that box: up to 200 points, or 300 if you are standing on the spot it hit, before armor and the game type's multiplier. The original's tip says *"Don't get too close to your own Grenade launch,"* and it is right. Explosions also:

- break glass and turn pickups into rubble;
- blow away the wall they hit and scorch the walls around it (most walls go in two hits: scorched first, then gone); the map's outer walls and some marked walls never break;
- shake the screen and set the building trembling (**F8** turns the shake off);
- start a fire, now and then.

OBCR spares the exits and keeps nearby doors working, because in the original a careless grenade could seal the only way off a floor ([ADR-016](DECISIONS.md)).

**Chipping walls.** Holding fire with the shotgun, Uzi or Galil at certain walls wears them down a stage at a time, and some give way entirely. Glass panes break in a few frames; broken panes stay solid but you can see through them.

**Fire.** Flame hits on burnable objects start fires, and fires are actors of their own: they spread as things walk into them and burn out at random. A fire hurts anything standing in it (100 points to you at point-blank). *"Fire burns,"* says the manual, accurately.

## Items

Walk over an item to take it. Most are refused if you are already at the limit.

| Item | What it does |
|---|---|
| **Supply crate** | Usually 1–7 clips of 9 mm (three times in four); sometimes 7 mm clips or 1–4 grenades; rarely a fuel bottle. |
| **Uzi** (dropped by soldiers) | One 9 mm clip. |
| **Galil** | The Galil, with a clip. |
| **Flame Thrower** | The flame thrower, with fuel. |
| **Grenade Launcher** | The launcher, with a grenade. |
| **Grenades** | One more grenade (up to 19). |
| **Fuel Pack** | One more fuel clip. |
| **Flak Jacket** | 100 armor, if you have less than 400. Armor takes half of every hit you receive and wears down by that half. Dead allies leave one behind. |
| **Increased Health** | 100 health, if you have less than 400, and it raises your healing limit for the floor. |
| **Map ThermoSensor** | For the rest of the floor the **Tab** map shows enemies in red, allies in yellow and you blinking. |

**Healing** is scarce. Floors give 35 health each. Some walls are **medic stations**: use one for `Medic Pack` and 25 health, but only while you are below your healing limit, which is 100 on a floor unless you found Increased Health there. The code also contains complete walk-in **healing booths** that turn you to face them, close a door and pour health into you, but nothing in the shipped game ever starts one. OBCR ports them anyway, equally unused.

**Booby traps.** Some bodies and some small grey objects on the floor are rigged. Touch one and you get `BOOBY TRAP`, an explosion in front of you, random damage, and a shove backwards. The manual's tip: *"Booby traps or spontaneous human combustion ?"*

There are no keys or key cards. Some doors look locked; the locks are only paint.

## Enemies

Each enemy has a sight sound, an attack and a death of its own. Hit points grow with the game type.

| Enemy | Notes |
|---|---|
| **Rats** | Fast, weak, bite only when adjacent. Worth 25 points. *"Why waste Good ammo on rats ?"* |
| **Giant rats** | Bigger rats with more bite, still only up close. |
| **Soldiers** | The basic terrorist with a gun. Drop an Uzi clip. |
| **Disguised soldiers** | The ambushes: men sitting on a stool, crouching, **on the toilet**, or lying on the floor playing dead. They stand up when they see you, then fight like soldiers. |
| **Men in black suits** | Tougher gunmen who fire two shots at a time and drop the **Galil**. |
| **Saucer drones** | Small flying machines that hover or patrol and zap you with lightning when they reach you. They explode when killed and do not block your way. |
| **Swamp creatures** | Lurk submerged, rise to spit slime balls that splat your visor, back away when you get close, and dive again. They can only be hurt while up and attacking. *"Swamp Creatures: shoot 'em UP."* |
| **Fat men** | Heavy gunmen; one kind waits under a manhole and climbs out when you come near. Four shots at a time. |
| **Red-vest brutes** | Big, slow-to-die and unable to open doors. |
| **Gunners** | Shirtless men with rifles; they dodge and fire two shots at a time. |
| **Red giants** | Six shots per attack, and they melt into a skeleton when they die. |
| **Victor** | The final boss, on floor 40. Around 4,000 hit points and more. Once he starts shooting he never stops. His death ends the game. |
| **Fire** | Not an enemy as such, but it moves, spreads and hurts. You can't shoot it out. |

From floor 7 on, once per session, a ghostly **face** may come flying at you out of nowhere. It does no harm. (The CD release removed it.)

## Your squad

On floors 6 to 23 you have **allies**: usually three soldiers lined up beside you at the start. They have 400 health each, never follow you to the next floor, and drop a flak jacket when they die. Building 2 has none.

| Key | Order | What it does |
|---|---|---|
| **F** | Follow / Disperse | Toggles the squad between **Disperse and Attack** (the default at the start of each floor) and **Follow Player**. |
| **Q** / **A** | Switch | You **become** the next living ally: their position, their facing and their health. Your weapons, ammo and score come with you; the body you leave behind gets your old health and position. It also wipes your visor. |
| **Z** / **S** | Switch & Stay | The same, but the body you leave stays put. |

- **Follow Player**: allies walk toward you and stay close. They are, as the manual puts it, *"too preoccupied in this mode to do any shooting"*: in Follow they never fire.
- **Disperse and Attack**: allies roam on their own, find enemies they can see and shoot them.
- **Switching** is the game's trick: a nearly dead soldier can hand the fight to a fresh body on the other side of the floor. *"Your Allies are very HEALTHy."* It is not a spare life, though: if **you** die, the game is over even with allies alive.
- **Staying**: the manual says a stay ally ambushes whatever wanders by. The code says a staying ally does nothing at all: no moving, no shooting. Enemies can still shoot it, so it works as a decoy. This hasn't yet been checked in the running original. Toggling into Follow wakes it up.
- `NO ALLIES!` means there is nobody left to switch to.

## The building

**Doors** open with Use, as in any game of this family. Some walls are **door-look-alikes** that slide open the same way; unlike real doors they never close on their own.

**Secret walls** (push walls). The manual: *"There are many secret rooms hidden on each level... Each of the secret rooms has a wall tile which can be moved."* Use one and it slides away from you **exactly two tiles**, then stops for good: it can't be pushed again. You stop it by standing in its way. Some floors use rows of them as partitions between open rooms, so a block can end up standing alone in the middle of a room. That is how the original does it too.

**Generators.** Some floors start in the dark. Find the generator switch and use it: `GENERATORS ON`, and the lights come back. Switch it off again for `GENERATORS OFF`. A grenade that scorches the switch leaves it dead and the lights as they were.

**Barbed wire** hurts: walk into it and you lose 5 health per try, with a chance of a splat on the visor.

**Darkness and tremors.** Explosions and booby traps shake the screen and make the lights flicker for a while; occasionally the lights go out when the trembling stops. **F8** turns the screen shake off.

**Low health.** At 35 health or less, any hit has a 1-in-15 chance of making you black out: a slow fade to black, with the game frozen until it passes.

## Other keys

| Key | What |
|---|---|
| **Esc** / gamepad Start | The menu (not during a tremor, as in the original). |
| **F1** | Sound effects on or off. |
| **F2** | Music on or off. |
| **F5** / **F6** | Textured floors and ceilings off / on (remembered). |
| **F8** | Screen shake on or off. |
| **F9** | Quick save, into the slot you last saved to from the menu. |
| **F10** | Quit, after asking. |
| **Pause** | Pause. |

**Saving.** Nine slots plus the AutoSave slot, which the game fills every time you finish a floor. Saves are OBCR's own format and belong to the release they were made with: a floppy-version save does not load with the CD files, and the other way round.

## The CD release plays a little differently

The CD release that Steam and GOG sell is a later build of the game than the floppy one, and OBCR plays each as its own executable did. With CD files ([cd-release.md](cd-release.md) explains installing from the disc image, the movies and the music; [re/CD_RELEASE.md](re/CD_RELEASE.md) has the technical detail):

- **Movies**: an opening movie before the title, one after floor 23 and one for the victory. They exist only on the CD.
- **CD music** instead of the AdLib songs. **Alt+P** plays, **Alt+S** stops, **Alt+F** goes to the next track. OPTIONS → MUSIC SOURCE switches back to AdLib.
- **Grenades blow secret walls open.** On the floppy version they can't touch them.
- **The saucer drone** zaps you from any distance, not just point-blank, and its attack is a little slower.
- **No ghostly face**, and no `UN BUILDING 2` greeting on floor 24.
- **Floor 40's exit** says `LAST FLOOR` instead of nothing.
- **The red damage flash** follows the size of the hit: small hits don't flash, big ones flash harder, and hits add up.
- **Low-health blackouts** can also happen in god mode and in network games.
- **Enemy grenades and flames** that hit their target directly do different damage. A direct flame hit's damage can't be read from the code, so OBCR uses an estimate for it.
- **A rising manhole fat man** no longer leaves an open manhole behind.
- **One kind of decoration** (map object 71) blocks your way.
- **Some sprites** are retouched, and twelve debug and effect sprites are gone.
- **The BEWITCHED cheat** (Alt+B) is gone.

## Cheats

The original has a cheat parameter, and OBCR keeps it: start with `obcr --nsr250`. In a single-player game, hold **Alt + Left Shift + B** for `BOGUS MODE`. Then:

| Keys | What |
|---|---|
| **Alt+R** | `RESTOCK`: every weapon, full clips. |
| **Alt+I** | `INVINCIBLE` / `VULNERABLE`. |
| **Alt+H** | `HEALTH HELP`: 10 health. |
| **Alt+B** | `BEWITCHED`: most enemies freeze until you kill something (floppy version only). |
| **Alt+W** | Warp: type a floor number. |

A game that has been in bogus mode never enters the high-score table. The original's second, diagnostic parameter is not ported.

## The original's own tips

From the manual supplement on the CD, verbatim:

> Game victory ? Kill Victor.
>
> Some tips on playing BodyCount:
>
> - Booby traps or spontaneous human combustion ?
> - FlameThrower in Arabic means "mother of all killing machines".
> - Your Allies are very HEALTHy.
> - Don't get too close to your own Grenade launch.
> - Fire burns.
> - Why waste Good ammo on rats ?
> - Swamp Creatures: shoot 'em UP.
> - Switch & Stay, not Stop Drop and Roll.

And one of OBCR's: half the floor is enough. Save the ammo for Victor.

---

### Where this comes from

The rules above were read from the original executable's code: weapons, items, damage and the status bar in [re/PLAYER.md](re/PLAYER.md); enemies in [re/ACTORS.md](re/ACTORS.md); allies in [re/ALLIES.md](re/ALLIES.md); walls, exits and progression in [re/LEVEL_CODES.md](re/LEVEL_CODES.md) and [re/GAMEFLOW.md](re/GAMEFLOW.md); the CD release in [re/CD_RELEASE.md](re/CD_RELEASE.md). [MECHANICS_AUDIT.md](MECHANICS_AUDIT.md) checks each mechanic against the manual and player reports, and lists the few places OBCR deliberately departs from the original.
