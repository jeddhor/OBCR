# Mods and custom maps

OBCR plays **mods**: new floors for Operation Body Count, arranged as a campaign of their own, optionally with their own wall textures, sprites, sounds and music, and their own network arenas. This page is for both sides of that:

- [Playing mods](#playing-mods): installing one, choosing it, and what changes when you play it.
- [Making mods](#making-mods): what a mod is, the `mod.toml` manifest field by field, the art, sound and music files, network arenas, and the rules a map must follow. **OBCEdit**, the level editor that comes with OBCR, writes all of this for you; its manual is [obcedit-manual.md](obcedit-manual.md).

The design behind it is [MODS_AND_OBCEDIT.md](MODS_AND_OBCEDIT.md) and [MOD_ASSETS_AND_ARENAS.md](MOD_ASSETS_AND_ARENAS.md), with ADR-023, ADR-025 and ADR-026 in [DECISIONS.md](DECISIONS.md).

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

The floors, and what a mod chooses to replace of what you see and hear: wall textures, sprites (objects, enemies, the weapons in hand), sounds and music. Weapons, enemies, allies, items, the half-kill quota, the status bar, saving and every rule are the original game's. Each floor of a mod behaves like one of the original's floors, chosen by its author: a sewer floor is dark with the sewer's textures and ladder sequence, a UN floor is lit, and so on.

- **The campaign** runs the mod's floors in order. Finishing the last floor wins it: the victory screen, then the high scores. Killing Victor, if a mod includes him, wins it too, as always.
- **Saved games** are kept per mod: each mod has its own ten save slots (in `saves/<mod id>/`), so a mod's saves never mix with the original game's. F9 and the AutoSave work as usual.
- **High scores** are kept per mod as well.
- **Network games** play the mod's own arenas when it has some (the MODS screen lists them, and the network setup screen shows `ARENAS:` and the mod's title); otherwise the original's ten. Everyone in the game needs the same version of the mod: machines with a different mod, or none, do not see each other, and the setup screen says `other version, game type or mod`.

A mod plays with either release of the game (floppy or CD) unless its author says otherwise. The mod contains only its author's own work: everything it does not replace comes from your copy of the game.

---

## Making mods

The easy way is **OBCEdit**: draw your floors, set up the campaign in *File → Campaign*, test with **F5**, and *Export mod* or *Install in OBCR*. Everything below is what OBCEdit writes, for those who want to know or to write it by hand.

### What a mod is

A folder, or the same folder packed as a ZIP file with the extension `.obcmod`:

```
mod.toml          the manifest: who made it, the floors and the arenas
MAPHEAD.BC        the mod's maps, in Operation Body Count's own map format
MAPTEMP.BC        (needed only if a floor or arena uses map =)
README.txt        optional, shown in the MODS screen's details
walls/NNN.png     format 2: replaces wall page NNN (0-135)
sprites/NNN.png   format 2: replaces sprite NNN (0-679)
sounds/NN.wav     format 2: replaces digitized sound NN (0-98)
music/NAME.ogg    format 2: music a floor or arena names (also .qoa, .imf)
```

OBCR reads nothing else, and tells you about other files when it checks the mod. A `.obcmod` may use stored or deflated entries only, no encryption, no folders above its root, at most 2,000 files, and stays under 128 MB (16 MB for any one file). A folder has the same limits.

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

**`format`** is the manifest version: 1 for floors only, 2 for a mod with art, sounds, music files or arenas. OBCR refuses a higher one than it knows (OBCR 1.0.0-beta.3 and earlier read only format 1).

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
| `music` | 0–9, `"random"`, a file | from `like` | The AdLib song: original floors 1–10 have one each, later floors pick at random. Format 2: the name of a file in `music/`, for example `"theme.ogg"`. |
| `dark` | true, false | from `like` | Start in the dark, as the sewers do. |
| `sewer` | true, false | from `like` | The sewer's floor and ceiling textures and its darker look. |
| `colors` | 0–50 | from `like` | Which original floor's roof and floor colours and shading to use. |
| `sequence` | `"auto"`, `"F1"`–`"F7"`, `"none"` | from `like` | The animated sequence after the floor. `auto` plays the stairs (F3) or elevator (F4) one by the exit used. The original plays F2 (the ladder) after the sewers and, on the CD, F5 after floor 23. A sequence the player's copy lacks is skipped. |
| `message` | up to 40 characters | from `like` | A message when the floor starts (the original greets floor 24 with `UN BUILDING 2`). |
| `exit` | `"normal"`, `"none"` | from `like` | `none`: the exits refuse to work, as on the original's floor 40, where only killing Victor ends the game. |

Give each level exactly one of `map` and `stock`. Unknown keys are errors, so a typo is reported rather than ignored.

**What `like` brings.** Original floors 1–5 (slots 0–4) are dark sewers with sewer textures and the ladder sequence. Floor 6 (slot 5) is the maintenance level. Floors 7–23 (slots 6–22) are UN Building 1, where the ghostly face can appear (floppy only) and floor 23 is followed by the CD's F5 movie. Floors 24–40 (slots 23–39) are Building 2, whose first floor greets you and whose last floor has no working exit. Each floor also brings its colours and, for floors 1–10, its song. Overrides change any of these.

### Art, sounds and music (format 2)

A mod replaces the game's own numbered pages, sprites and sounds; it cannot add new ones, because the game's map codes, enemies and sound effects refer to those numbers. OBCEdit's *Resources* panel shows every one with its name and picture, imports your files and writes them under the right names.

- **Walls**: `walls/NNN.png`, 64×64. Page *n* is drawn by wall code *n* + 1. Doors are pages 128–135, and the floors and ceilings are wall pages too (the floor sets name them), so replacing a page changes every place it appears.
- **Sprites**: `sprites/NNN.png`, 64×64, transparent where the alpha is below 128, drawn standing on the bottom edge like the originals. Sprites are the objects, every frame of every enemy and its eight facings, the weapons in hand (544–623) and the blood splats.
- **Sounds**: `sounds/NN.wav`, PCM 8-, 16-, 24- or 32-bit or float, mono or stereo, 4,000–48,000 Hz, at most 30 seconds. OBCR plays it as 8-bit mono at its own rate. The original sound's AdLib version still decides which sound wins when several play, and still plays when digitized sound is off.
- **Music**: `music/NAME.ogg` (Ogg Vorbis), `.qoa` (QOA) or `.imf` (AdLib IMF, with or without the length header), named by a floor's or arena's `music =`. NAME is 1–32 characters `A–Z a–z 0–9 _ -`. Music files loop, and play whatever MUSIC SOURCE is set to. `obcr --convert-audio song.wav song.qoa` turns a WAV into QOA, which OBCEdit does when you import a WAV.

Numbers may have leading zeros (`walls/012.png` and `walls/12.png` are the same page, and having both is an error). A wrongly named file in these folders is an error rather than being skipped, so a typo cannot go unnoticed.

**Colours.** Images are ordinary PNGs of any kind. OBCR converts each pixel to the game's palette as it loads the mod: the palette entry nearest to it in RGB, so a mod never contains the game's palette and your files stay editable. OBCEdit previews exactly this conversion. Colours far from the game's palette come out as the nearest it has: the game's palette is mostly browns, greys and skin tones.

### Network arenas (format 2)

`[[arena]]`, once per arena, in the order they are played (at most 10):

| Key | Values | Default | |
|---|---|---|---|
| `map` | 0–99 | required | The mod's own map. |
| `name` | up to 16 characters | the map's name | |
| `music` | 0–9, `"random"`, a file | `"random"` | As for floors. |
| `colors` | 0–59 | 50 + its index | Roof and floor colours (the original arenas use entries 50–59). |
| `dark`, `sewer` | true, false | false | As for floors. |

A network game started with the mod active plays its arenas instead of the original's; taking an exit on the last arena goes back to the first. An arena needs one player start, which marks the region players appear in: in a mod's arenas, players reappear only on cells connected to the start. Arenas cannot have enemies or allies: in a network game they would appear but could not be killed.

```toml
format = 2
[mod]
id = "warehouse"
title = "Warehouse"

[[level]]
map = 0
music = "theme.ogg"

[[arena]]
map = 1
name = "Loading Bay"
```

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

**Arenas** also get an error for any enemy or ally, and a warning for open floor that cannot be reached from the start. **Asset files** get an error when they cannot be read (a damaged or wrongly sized image, an unsupported sound, music that does not decode to the end), and a warning for a music file nothing plays.

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

A mod may contain only maps, art, sounds and music you made or have the right to share. Maps copied from the original game belong to its rights holders. OBCEdit lets you open them to look and learn, but refuses to export one into a mod meant for sharing. To include an original floor in your campaign, use `stock =`, which plays it from the player's own copy.
