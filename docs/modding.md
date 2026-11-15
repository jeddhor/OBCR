# Mods and custom maps

OBCR plays **mods**: new floors for Operation Body Count, arranged as a campaign of their own. This page is for both sides of that:

- [Playing mods](#playing-mods): installing one, choosing it, and what changes when you play it.
- [Making mods](#making-mods): what a mod is, the `mod.toml` manifest field by field, and the rules a map must follow. **OBCEdit**, the level editor that comes with OBCR, writes all of this for you; its manual is [obcedit-manual.md](obcedit-manual.md).

The design behind it is [MODS_AND_OBCEDIT.md](MODS_AND_OBCEDIT.md) and [ADR-023](DECISIONS.md).

---

## Playing mods

### Installing a mod

A mod is a single `.obcmod` file or a folder. Put it in OBCR's **mods** folder:

| | |
|---|---|
| Windows | `%APPDATA%\OBCR\data\mods\` |
| Linux | `~/.local/share/obcr/mods/` |

Create the folder if it is not there yet. OBCEdit's *Install in OBCR* puts mods there by itself.

### Choosing one

Press **D** in the main menu to open **MODS**. The list holds **ORIGINAL GAME** and every installed mod, with its author, version, number of floors and description. Choose one with Enter and it becomes the game that **START GAME** starts; the main menu shows its title in yellow. Choose **ORIGINAL GAME** to go back. The choice is remembered.

A mod that cannot be played is shown in grey with the reason in red: a damaged file, a map that breaks the game's limits, or a mod made for the other release (the floppy or the CD). It cannot be chosen.

To play a mod once without choosing it, start OBCR with `--mod`:

```sh
obcr --mod ~/Downloads/lobby-assault.obcmod
obcr --mod lobby-assault            # an installed mod, by its id
```

### What changes

Only the floors. Weapons, enemies, allies, items, the half-kill quota, the status bar, saving and every rule are the original game's. Each floor of a mod behaves like one of the original's floors, chosen by its author: a sewer floor is dark with the sewer's textures and ladder sequence, a UN floor is lit, and so on.

- **The campaign** runs the mod's floors in order. Finishing the last floor wins it: the victory screen, then the high scores. Killing Victor, if a mod includes him, wins it too, as always.
- **Saved games** are kept per mod: each mod has its own ten save slots (in `saves/<mod id>/`), so a mod's saves never mix with the original game's. F9 and the AutoSave work as usual.
- **High scores** are kept per mod as well.
- **Network games** always use the original's ten arenas.

A mod plays with either release of the game (floppy or CD) unless its author says otherwise. The mod contains only the author's own maps: walls, sprites and sounds come from your copy of the game.

---

## Making mods

The easy way is **OBCEdit**: draw your floors, set up the campaign in *File → Campaign*, test with **F5**, and *Export mod* or *Install in OBCR*. Everything below is what OBCEdit writes, for those who want to know or to write it by hand.

### What a mod is

A folder, or the same folder packed as a ZIP file with the extension `.obcmod`:

```
mod.toml          the manifest: who made it and the list of floors
MAPHEAD.BC        the mod's maps, in Operation Body Count's own map format
MAPTEMP.BC        (needed only if a floor uses map =)
README.txt        optional, shown in the MODS screen's details
```

OBCR reads nothing else, and tells you about other files when it checks the mod. A `.obcmod` may use stored or deflated entries only, no encryption, no folders above its root, and stays under 64 MB.

The maps are in the **original's own format**, which any Wolfenstein-family tool that reads RLEW `MAPHEAD`/`MAPTEMP` pairs can open: up to 100 map slots, each map 64×64 cells with three planes.

### The manifest, `mod.toml`

```toml
format = 1

[mod]
id = "lobby-assault"
title = "Lobby Assault"
author = "Jane Doe"
version = "1.0"
description = "Three floors of the UN lobby, rebuilt."
release = "any"

[[level]]
map = 0
name = "Lobby"
like = 6

[[level]]
stock = 7

[[level]]
map = 1
like = 2
music = 4
```

**`format`** is the manifest version, 1. OBCR refuses a higher one.

**`[mod]`**

| Key | | |
|---|---|---|
| `id` | required | 1–32 characters `a`–`z`, `0`–`9`, `-`. Names the mod's save folder and score table, so keep it the same across versions. |
| `title` | required | Up to 32 printable ASCII characters (the game's font has no others). |
| `author`, `version`, `description` | optional | Up to 64, 16 and 400 characters. |
| `release` | optional | `any` (default), `OBC-A` (floppy only) or `OBC-CD` (CD only). |
| `obcr` | optional | The oldest OBCR version the mod needs; shown, not enforced. |

**`[[level]]`**, once per floor, in order (the first is floor 1; at most 100):

| Key | Values | Default | |
|---|---|---|---|
| `map` | 0–99 | — | The mod's own map, by slot in its `MAPTEMP.BC`. |
| `stock` | 0–39 | — | One of the original's floors (slot 0 is floor 1), played with its own behaviour. It is read from the player's copy of the game, never copied into the mod. |
| `name` | up to 16 characters | the map's name | Shown in OBCEdit and lists. |
| `like` | 0–39 | 6 | The original floor whose **behaviour** this floor takes (below). |
| `music` | 0–9, `"random"` | from `like` | The AdLib song: original floors 1–10 have one each, later floors pick at random. |
| `dark` | true, false | from `like` | Start in the dark, as the sewers do. |
| `sewer` | true, false | from `like` | The sewer's floor and ceiling textures and its darker look. |
| `colors` | 0–50 | from `like` | Which original floor's roof and floor colours and shading to use. |
| `sequence` | `"auto"`, `"F1"`–`"F7"`, `"none"` | from `like` | The animated sequence after the floor. `auto` plays the stairs (F3) or elevator (F4) one by the exit used. The original plays F2 (the ladder) after the sewers and, on the CD, F5 after floor 23. A sequence the player's copy lacks is skipped. |
| `message` | up to 40 characters | from `like` | A message when the floor starts (the original greets floor 24 with `UN BUILDING 2`). |
| `exit` | `"normal"`, `"none"` | from `like` | `none`: the exits refuse to work, as on the original's floor 40, where only killing Victor ends the game. |

Give each level exactly one of `map` and `stock`. Unknown keys are errors, so a typo is reported rather than ignored.

**What `like` brings.** Original floors 1–5 (slots 0–4) are dark sewers with sewer textures and the ladder sequence. Floor 6 (slot 5) is the maintenance level. Floors 7–23 (slots 6–22) are UN Building 1, where the ghostly face can appear (floppy only) and floor 23 is followed by the CD's F5 movie. Floors 24–40 (slots 23–39) are Building 2, whose first floor greets you and whose last floor has no working exit. Each floor also brings its colours and, for floors 1–10, its song. Overrides change any of these.

### Map rules

OBCR checks every map before a mod can be played (`obcr --check-mod`), with the limits the original game has:

**Errors** (the mod cannot be played):
- the map is not 64×64;
- there is not exactly one player start (plane-1 codes 19–22);
- more than 64 doors, 224 objects, 32 animated walls, or 99 actors at the hardest game type;
- a plane-0 value that is not a wall, door or floor (floors are 140–161: an area number 0–21 plus 140).

**Warnings** (it plays, but probably not as you meant):
- a door without walls on both its sides;
- open floor on the map's edge;
- a push-wall marker on a cell that is not a wall;
- a plane-1 value the game does not use;
- a crowded map, since fires, explosions and projectiles share the actors' slots.

OBCEdit checks these and more as you draw, including whether the exit can be reached and whether half the enemies can be killed. Every map value, what it means and how it behaves is in [re/LEVEL_CODES.md](re/LEVEL_CODES.md), and OBCEdit's palette names them all.

### Checking a mod

```sh
obcr --check-mod lobby-assault.obcmod
```

prints one tab-separated line per finding, then `result ok` or `result errors`. The exit code is 0 when it can be played and 2 when it cannot. The MODS screen shows the same result.

### Testing a floor directly

```sh
obcr --mod my-mod --play 2                 # floor 3 of the mod
obcr --mod my-mod --play 0 --start 12,30,1 # floor 1, the player at cell (12, 30) facing east
```

`--start X,Y,DIR` puts the player on a given cell, facing 0 north, 1 east, 2 south or 3 west. OBCEdit's *play from here* uses it.

### Sharing

A mod may contain only maps you made. Maps copied from the original game belong to its rights holders. OBCEdit lets you open them to look and learn, but refuses to export one into a mod meant for sharing. To include an original floor in your campaign, use `stock =`, which plays it from the player's own copy.
