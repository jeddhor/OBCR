# Multiplayer

Operation Body Count shipped with network play: deathmatch and team games over an IPX network, and two-player games over a modem or a null-modem cable. OBCR brings all three back, over UDP. That means a LAN, as the original had, and also the Internet, which the original never did.

The rules are the original's. They were read out of the executable (segment `358f`) and ported as they are: the respawns, the damage, the frag counting, the teams, the network floors and the KILLS table. Only the way packets travel is new. [How it works](#how-it-works) at the end explains the model for the curious.

> **Tested so far on one machine only.** Two to four copies of OBCR have played each other over localhost: kills, deaths, respawns, exits and the KILLS table agree on every copy, also with 10 % of the packets thrown away on purpose. A game across a real LAN or the Internet has not been played yet. If you try one, please report how it went.

---

## The three game types

Network play is chosen where difficulty is chosen: **GAME TYPE** in the main menu. The original's manual supplement notes that the manual calls this "Difficulty Level". The menu shows the type as a letter.

| Letter | Original name | In OBCR | Players |
| --- | --- | --- | --- |
| **N** | Network ("beat up on your co-workers") | Deathmatch on the LAN or over the Internet | up to 12 |
| **T** | Network Teams ("Marketing vs. Shipping and Receiving") | The same, in three teams | up to 12 |
| **M** | Modem ("a different type of harassing phone call") | A direct connection between two machines: one calls the other's address | 2 |

Everyone in a game must pick the same type. Machines of different types ignore each other.

**Everyone needs the same game data release too.** A copy installed from the floppy release and one installed from the CD release cannot see each other. Each machine announces its release, and a mismatch is ignored. The same goes for different OBCR versions whose network protocol differs.

---

## Starting a game

### From the menu

1. In the main menu, select **GAME TYPE** (or press **P**), then press **Up** or **Down** until it shows **N**, **T** or **M**. Press **Enter**.
2. Select **START GAME** (or press **N**). Instead of starting a floor, the network setup screen opens.
3. Choose your character, or in a team game your team, with **Up** and **Down**.
4. Wait until the others appear under *Players here*, or call one of them (see below).
5. Press **Enter** (or Space, Ctrl, or the gamepad's A or Start). The screen says *Get Ready!* and you are dropped onto the first network floor.

There is no lobby, no host and no "everyone ready" step. That is how the original worked: each player starts when they like, and players find each other as they play. Someone who starts late simply turns up (*PLYR ID n IN*).

### The setup screen

| What it shows | Meaning |
| --- | --- |
| `Player ID number n` | Your id in this game (1–254). Other players see it when you join, and it labels you in their messages. |
| `UDP address a.b.c.d` | This machine's address on the local network. Give it to LAN players who want to call you. |
| `Communicating on UDP port 4003` | The port, **in hexadecimal** as the original showed its IPX socket. 4003 hex is **16387**. |
| `CHOOSE A PLAYER` / `CHOOSE A TEAM` | Your choice (below). |
| `Players here: …` | The ids of the players heard so far. |
| `Searching the LAN` / `Calling …` / `Waiting for a call` | What the machine is doing to find players. |

| Key | Does |
| --- | --- |
| Up / Down | Choose the character or team |
| **J** | Call an address: type `host` or `host:port`, then Enter |
| Delete | Forget the addresses you called |
| 0–9, A–F | Change the port: type up to four hex digits, then Enter (the original's socket entry; 0100 hex is the lowest) |
| Enter, Space, Ctrl | Start |
| Esc | Back to the menu |

OBCR remembers the last address you called, the port and your choice for next time. They are stored in the `[net]` section of `obcr.toml` as `join`, `port` (in decimal) and `choice`.

### From the command line

`--net N`, `--net T` or `--net M` starts a network game at once, without the setup screen:

```sh
obcr --net N                                  # deathmatch, find players on the LAN
obcr --net T --net-choice 2                   # team game, on Team 3
obcr --net N --net-join 203.0.113.7           # also call this machine (port 16387)
obcr --net M --net-join friend.example.org:20000 --net-port 20000
obcr --net T --net-setup                      # open the setup screen for type T instead
```

| Option | Meaning |
| --- | --- |
| `--net N\|T\|M` | Game type |
| `--net-choice 0\|1\|2` | Character or team: 0 Agent / Team 1, 1 Terrorist 1 / Team 2, 2 Terrorist 2 / Team 3 (default 0) |
| `--net-join ADDRESS` | An address to call, `host` or `host:port`; repeat it to call several |
| `--net-port PORT` | The UDP port, in decimal (default 16387) |
| `--net-setup` | Open the setup screen instead of starting straight away |
| `--net-loss PERCENT` | Testing: throw away this share of outgoing packets |
| `--net-place X,Y,ANGLE` | Developer: always respawn at this spot instead of a random one |

---

## Finding each other

### On a LAN

Do nothing. Every machine announces itself by broadcast on its port, as the original did with IPX, and machines that hear each other start exchanging play. **Everyone must use the same port** for this to work; the default is fine.

### Over the Internet, or across networks that broadcasts do not cross

One player calls another with **J** on the setup screen (or `--net-join`). Once two machines know each other they share the addresses of everyone else they know. So in a bigger game, each newcomer only has to call one player who is already in.

Things to know:

- **The port is UDP 16387** unless you changed it. A machine behind a home router can only be called if the router forwards that UDP port to it. The steadiest set-up is for every player to forward it. With two players, forwarding on the side that is being called is usually enough, because the caller's router lets the replies back in.
- The address on the setup screen is your *local* one. Over the Internet, the others need your *public* address (any "what is my IP" page shows it).
- **IPv4 only.** OBCR opens an IPv4 socket, so an IPv6-only connection will not work.
- A virtual LAN (Tailscale, ZeroTier, Hamachi and the like) avoids port forwarding. Call the other player's address on that virtual network with **J**, because broadcasts often do not travel across them.
- Windows may ask whether to let OBCR use the network the first time it opens the port. Allow it on the networks you will play on.

### Game type M

M is the original's modem game, made into a direct connection between two machines. Nothing is broadcast: one player waits (*Waiting for a call*) and the other calls their address with **J** or `--net-join`. A third machine is not accepted.

---

## Characters and teams

You choose what the **other** players see you as, and with it your starting weapons:

| Choice | Deathmatch character | Team | You look like | You start with |
| --- | --- | --- | --- | --- |
| 0 | Agent | Team 1 | an ally | assault shotgun and Uzi |
| 1 | Terrorist 1 | Team 2 | a soldier | assault shotgun and Uzi |
| 2 | Terrorist 2 | Team 3 | a man in a black suit | assault shotgun and Galil |

In a team game your team is your look, so team-mates look alike. Team-mates cannot hurt each other, and the whole team shares the best score among its members.

---

## How a match plays

**The floors.** Network games are played on the ten network floors, which a single-player game never visits. You start on the first one, floor 51. Like the single-player floors they have exits. Taking an exit finishes the floor **for you**: you see the KILLS table and go on to the next floor, and the others follow when they take an exit themselves. Players on another floor are not in your game until they arrive. While they are elsewhere you only see a note like *Activity on 52* when they do something. Floor 60, the last, has no exit.

**Health and respawns.** Everyone spawns with 800 health at a random free spot, carrying the weapons of their choice above. When you die there is no death sequence and no game over: after a moment you respawn at another random spot on the same floor, which keeps its state. A random spot can occasionally be in a sealed-off corner, as in the original.

**Damage.** Your own machine decides whether you are hit. Another player's shot at you uses the original's enemy-fire rules there: the closer they are, and the faster you are moving, the more it matters. Their flames and grenades are thrown again on your machine and hurt you as projectiles. While your health is 30 or more, damage is multiplied: by 3 in game types N and T, and by 4 in M. Armour halves it as usual.

**Dropped weapons.** A player who dies drops the weapon they were holding, unless it was the assault shotgun.

**Scoring.** Your score is your frag count: one per kill, whoever's weapon did it. In a team game, kills by team-mates count for the whole team. The kills counter on the status bar shows how many other players are in the game. In the network game types the floors have no kill quota.

**The KILLS table** appears when you take an exit, and when you quit. It is a grid with the killers down the side, the victims across the top, and you as `U`. The others are labelled by the order in which your machine heard them. Your deaths that nobody caused (falling into your own grenade, for instance) count against `U` as killer and victim both.

**Things the network game types do differently** from single player:

- **No saving or loading.** SAVE GAME is unavailable, and F9 QuickSave and the automatic save at the end of a floor are off.
- **Opening the menu counts as a suicide.** The original announces it to the others as a death, and OBCR does the same. Your character does not actually die: on the other screens you drop and get up again where you stood. Your game does not run while the menu or the pause is up, but the others carry on, and the first packets after you come back put you right.
- Floors are not kept in step. Each machine runs its own copy of the floor, so items, fires and other floor state can differ between machines. What is replayed everywhere is what players *do*: their shots, deaths, and uses of doors, switches and exits.

---

## When it does not work

| Symptom | Likely cause |
| --- | --- |
| *Network initialization error* on the setup screen | The UDP port is in use, perhaps by another copy of OBCR on the same machine. Choose another port (hex keys on the setup screen, or `--net-port`). |
| Nobody appears on a LAN | Different ports, a firewall blocking UDP, or a network that drops broadcasts (some Wi-Fi guest networks do). Call the other machine's address with **J**. |
| A note says *other version or game type* | That machine runs a different game type, a different data release (floppy and CD do not mix), or an OBCR version with a different protocol. |
| Calling over the Internet does nothing | The called player's router does not forward UDP 16387, or you called their local address instead of the public one. |
| A player vanished | Nothing was heard from them for 15 seconds (or they quit). They disappear from your floor. If they come back they join again. |
| A door is open for one player and shut for another | The packet carrying the "use" was lost, which OBCR makes much less likely than the original did. Floor state is otherwise not shared. |

---

## Compared with the original

**Not compatible with the DOS game.** OBCR does not speak IPX or drive a serial port or modem, so it cannot play against a copy of the original running in DOSBox. Two OBCR copies of the same release are needed.

**What OBCR changed, and why.** All of it is in how packets travel; none of it changes a rule:

- **Packets carry recent history.** Each datagram also carries the sender's previous two state packets, numbered, and the newest is resent twice shortly after. A single lost packet no longer loses a death, a shot or a door. The original kept only the latest packet per player and frame, so an event could be overwritten.
- **Player ids are chosen to be unique.** The original used the last byte of the network card's IPX address, so two machines could collide and silently lose each other's packets. OBCR picks an unused id and picks again if two machines clash.
- **Players are matched to their actors by player.** The original matched them by position in its actor list, which a fire or another actor spawned at the wrong moment could throw off.
- **Players who leave disappear.** In the original, someone who quit stayed frozen where they last stood.
- **Calling by address, and sharing peer lists.** These make play over the Internet possible.

---

## How it works

The original is **not** lockstep, the scheme most DOS shooters used, where every machine runs the same game from the same inputs. Instead, every machine runs **its own copy of the floor** and is in charge of its own player only.

Once per frame each machine describes its player in a 24-byte state packet: floor, position, angle, weapon, character or team, frag count, and a one-off event (moving, fired, used something, died, burned to death). It sends the packet to every player it knows, but only when something changed. A broadcast every four seconds keeps it visible to the others. On the other machines that player is a **puppet**: an ordinary game actor (an ally, a soldier or a black-suited enemy, depending on the choice) moved by the latest packet.

Damage is always decided by the **victim**. When you shoot at someone, your machine only marks your puppet on theirs as firing at them, and their machine runs its normal enemy attack against itself. Flames and grenades are thrown again from your puppet on their machine. When someone dies, their machine announces it with the killer's id, and that is how every copy counts the same kills. A "use" is replayed by running the use action on every machine, which is how exits, doors and switches work for everyone.

OBCR keeps this model and moves it from IPX to UDP. The simulation itself never touches the network: before each frame the received packets are handed to it, and after the frame the packets it produced are taken away and sent. A network frame is therefore as reproducible as a single-player one, given the same packets, which is what the automated tests rely on. The transport adds:
- a header naming the protocol, game type and data release;
- the recent-history scheme above;
- broadcast beacons on the LAN;
- calls by address, with shared peer lists, so everyone learns everyone else;
- a goodbye when a player quits, and a 15-second time-out.

The technical record, with addresses in the original executable, is in [re/NETWORK.md](re/NETWORK.md) (the rules) and [re/NETCOMMS.md](re/NETCOMMS.md) (IPX, serial and modem). The design decision is ADR-017 in [DECISIONS.md](DECISIONS.md).
