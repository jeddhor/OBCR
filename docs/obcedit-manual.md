# OBCEdit — the manual

OBCEdit is the level editor that comes with OBCR. With it you draw new floors for Operation Body Count, test them in OBCR with one key, and turn them into a **mod** that anyone with OBCR and the game can play.

It is a port of **EC7Edit**, the editor of the owner's Corridor 7 port EC7Wolf, rebuilt for Operation Body Count's maps, objects and rules. What a mod is, and the format OBCEdit writes, is in [modding.md](modding.md).

## Contents

1. [What you need](#1-what-you-need)
2. [First run](#2-first-run)
3. [The window](#3-the-window)
4. [Your first floor](#4-your-first-floor)
5. [The tools](#5-the-tools)
6. [The palette: what the values mean](#6-the-palette-what-the-values-mean)
7. [Doors and specials](#7-doors-and-specials)
8. [Problems](#8-problems)
9. [Testing in OBCR](#9-testing-in-obcr)
10. [Snapshots](#10-snapshots)
11. [Projects and saving](#11-projects-and-saving)
12. [Studying the original floors](#12-studying-the-original-floors)
13. [Building a mod](#13-building-a-mod)
14. [Keyboard reference](#14-keyboard-reference)
15. [The command line](#15-the-command-line)
16. [Known limitations](#16-known-limitations)

---

## 1. What you need

- **OBCR**, installed by Setup with the *Install OBCEdit* option left on (the default). OBCEdit is then in the Start menu or your applications menu, and in `<install folder>/obcedit/`.
- **Your copy of Operation Body Count**: the game files OBCR uses. OBCEdit shows the game's real walls, floors and sprites from them, and OBCR plays your floors with them. You can draw without them, with coloured squares instead of pictures, but you cannot test.

From a source checkout, OBCEdit runs with Python 3.10 or newer and PySide6: `python obcedit/obcedit` (see [obcedit/README.md](../obcedit/README.md)).

## 2. First run

The first start opens **Setup**, which asks for three things and answers with a checklist rather than a verdict. Each line says what it looked for and what it found:

- **OBCR**: the game executable. An installed OBCEdit finds the `obcr` beside it by itself.
- **Game data**: the folder with your game files (an installation made by Setup has them in `<install folder>/data`). OBCEdit asks OBCR to identify them (`obcr --check-data`), so it accepts exactly what the game accepts: the floppy release or the CD release.
- **Workspace**: where your projects and test builds go.

*Tools → Setup…* opens it again.

## 3. The window

| Part | What it is |
|---|---|
| **Maps** (left) | The maps in your project. Double-click to open one; right-click to rename, change its slot, duplicate, check, test or delete it. *New map…* below the list. |
| **Map tabs** (centre) | The open maps, drawn cell by cell from above: walls, floors, doors, objects and enemies, each in its own colour, with markers for what sits on plane 1. |
| **Palette** (right) | Everything you can place, in tabs: *Doors and Specials*, then *Walls*, *Special walls*, *Floors*, *Objects*, *Enemies*, *Starts*, *Markers* and *Raw*. Type in the search box to find a thing by name, class or raw value; *Only what this map uses* narrows the tabs to the values already on the map. |
| **Inspector** | The cell under the cursor or the selection: its raw values on all three planes, what they mean, a picture, and for enemies their **facing**, whether they stand or patrol, and the **difficulty** that brings them in. Changing those rewrites the cell. |
| **Snapshot** | A frame of your map rendered by OBCR itself (section 10). |
| **Problems** (bottom) | What is wrong with the open map, checked as you draw, plus a line saying whether the floor can be finished (section 8). |
| **Test Log** (bottom) | What OBCR said the last time it played your map. |

Docks can be moved, floated and closed; *View → Reset layout* puts them back.

## 4. Your first floor

1. *File → New project* (**Ctrl+N**), then *New map…* (**Ctrl+M**). A new map is 64×64, every Operation Body Count map's size, with a wall all the way round and open floor inside.
2. **Walls**: pick a wall in the *Walls* tab and paint with **B**, draw lines with **L** or boxes with **R** (tick *Filled* for a solid block).
3. **Floors**: the *Floors* tab. A floor value carries two things: its **area** (rooms that doors connect: sound and enemy alertness travel by area) and its **texture set** (which floor and ceiling OBCR draws). Give each room its own area and doors between them.
4. **Doors**: the *Doors and Specials* tab, then **Door, look 1–4**, and click a cell between two walls. The door turns itself to fit the walls around it.
5. **The player**: the *Starts* tab, *Player start*, facing north, east, south or west. A floor needs exactly one.
6. **The way out**: *Exit: stairs* (or ladder or elevator) on a wall the player can walk up to. The exit only works once half the floor's enemies are dead, as in the original game.
7. **Enemies**: the *Enemies* tab. Each enemy comes standing or patrolling, in four facings, and in three difficulty tiers: *every difficulty*, *2 and 3*, or *3 only*. Turn them in the Inspector.
8. **Check**: **F8**, or just look at the Problems pane, which updates as you draw.
9. **Play it**: **F5** builds a one-floor test mod and starts OBCR on it. Press Esc in the game to leave.
10. **Save** (**Ctrl+S**). The project is a `.obcproject` file in your workspace.

## 5. The tools

| Tool | Key | What it does |
|---|---|---|
| Select | **S** | Click a cell to inspect it; drag to select a region for copy, paste, rotate and flip. |
| Paint | **B** | Paint the palette's choice cell by cell as you drag. |
| Line | **L** | A straight line from press to release. |
| Rectangle | **R** | A box outline, or a solid block with *Filled* ticked. |
| Fill | **F** | Flood-fill the area of matching cells. |
| Erase | **E** | Clear cells on the plane of the palette's choice: with an object, enemy or marker chosen it removes things (plane 1); with a wall or floor chosen it leaves plain floor (value 144). |
| Pick | **I** | Take the value under the cursor into the palette. |
| Place | **P** | Place the chosen compound structure (section 7). |
| Camera | **K** | Set where a snapshot looks from (section 10). |

*Edit* has **Undo** (**Ctrl+Z**) and **Redo** (**Ctrl+Y** / **Ctrl+Shift+Z**) for every change, including renames and deleted maps; **Copy** and **Paste** of a selection; and rotation and flipping of what you copied. Rotating turns enemies, the player start and patrol arrows with the region, so a copied room keeps looking the right way. Zoom with **Ctrl+=** / **Ctrl+−** or **Ctrl** and the mouse wheel; **Ctrl+G** toggles the grid.

## 6. The palette: what the values mean

An Operation Body Count map is three grids of numbers (planes). OBCEdit always shows the raw numbers in the Inspector, and its palette names every one of them. The names come from a catalog generated from OBCR's own code, so OBCEdit and the game never disagree about what a value means.

**Plane 0: walls, doors and floors**
- **1–129: walls**, each its own texture. Some are special (next section): sliding-panel walls, glass, the porthole fan, generator switches, the three exits, the damaged wall.
- **130–137: doors**, in four looks and two orientations. In Operation Body Count no door is locked: the look is only paint.
- **138: ambush marker**, under a standing enemy who waits until he sees you.
- **140–161: floors**: area 0–21 (the value minus 140); values 144–153 also choose one of ten floor-and-ceiling texture sets. 145 gives the ceiling-light pattern the original uses in its rooms.

**Plane 1: things**
- **19–22: player start**, facing north, east, south, west.
- **23–85: objects**, decorations, obstacles and pickups (ammo, grenades, weapons, flak jackets, health, the Map ThermoSensor, booby traps). Object 71 blocks your way in the CD release only; OBCEdit warns about it.
- **Enemies and allies**, from rats to Victor, standing or patrolling, four facings, three difficulty tiers. *Victor* ends the game when he dies.
- **Markers**: 89 makes a wall explosion-proof, 90–97 turn a patrolling enemy (patrol arrows), 98 is the push-wall marker, 104 and 105 make a wall see-through (a window blocks enemy sight, a grille does not).
- **Fire**, which burns until it burns out.

**Plane 2** is unused by the game; OBCEdit keeps whatever is there.

The *Raw* tab lets you place any value at all. The Problems pane tells you when a value means nothing to the game.

## 7. Doors and specials

The first palette tab holds structures that need more than one value, placed with one click (**P**):

| Structure | What it writes and does |
|---|---|
| **Door, look 1–4** | A door between two walls; its orientation is chosen from the walls around it. |
| **Sliding panel 13/29/49/65/69** | A wall that slides open when used and stays open until used again; *see-through* versions let you and enemies see through when closed. |
| **Push wall (secret)** | The secret-wall marker on a wall. Pushed, it slides **exactly two cells** and then stays put for good; OBCEdit shows the cells it will cross and warns when they are not open floor. |
| **Exit: ladder / stairs / elevator** | The floor's way out. Ladders and stairs play the climbing sequence, the elevator its own. |
| **Generator switch (lights on / lights off)** | A floor with a switch in the *off* position starts in the dark; using it turns the lights on (and off again). |
| **Glass pane, Screen pane** | A see-through pane that explosions shatter. |
| **Porthole fan (slow / medium / fast)** | The spinning fan wall. |
| **Rock opening (sewer)** | The sewers' open rock wall. |
| **Explosion-proof wall, See-through wall (window / grille)** | Markers on an existing wall. |
| **Ambush: soldier** | A soldier on an ambush marker: he stays put until he sees you. |
| **Squad of three allies** | Three allies in a row, as the original lines them up at the start of a floor. |

## 8. Problems

The Problems pane re-checks the open map as you work. **Errors** stop OBCR from playing it: no player start or more than one, more than 64 doors or the game's limits on objects, animated walls and actors, a floor value outside 140–161. **Warnings** play, but probably not as you meant: a door without walls on both sides, open floor on the map's edge, a push wall whose path is blocked, an unknown value, a crowded map. Notes are advice.

Above the list, a line answers the question that matters most: **can the floor be finished?** OBCEdit walks the map from the player start through doors, sliding panels and push walls, and reports whether an exit can be reached and how many of the enemies needed for the half-kill quota can be reached. It is advice, not a proof. A floor it calls finishable can still be hard, and an enemy behind a door only explosions open may not count.

Some problems have a fix: select one and press *Fix this* (for example, *seal the boundary* or *turn doors to match their walls*). Fixes are ordinary edits and can be undone.

## 9. Testing in OBCR

| Key | |
|---|---|
| **F5** | *Test in OBCR*: builds a one-floor mod of the open map, checks it with `obcr --check-mod`, and plays it. |
| **Shift+F5** | *Test from the cursor*: the same, with the player starting on the floor cell under the mouse (facing east). |
| **Ctrl+F5** | Stop the running test. |

OBCEdit refuses to start a test the game would reject, and says why in the Test Log. The test floor behaves like an original UN floor (it is a one-floor campaign with the default settings). Use the full campaign export (section 13) to test darkness, music and the rest.

## 10. Snapshots

A snapshot is a frame rendered by OBCR, not an imitation of it: there is one renderer, and it is the game's. Pick the **Camera** tool (**K**), click a floor cell, *Turn 90°* to face where you want, and **F7** (*Take a snapshot*). OBCR renders the map from there in the original 320×200 picture and OBCEdit shows it. Snapshots are kept until the map changes.

## 11. Projects and saving

A project (`.obcproject`) holds your maps, their names and slots, and the campaign. It is JSON, so it can be read, compared and kept in version control. **Ctrl+S** saves; *Save As* (**Ctrl+Shift+S**) and *Save a Copy* write elsewhere. Saves are written to a temporary file and then renamed into place, so a failed save never damages the previous one.

OBCEdit also autosaves your work every minute to a recovery file (`.obcrecovery`), outside the project. After a crash it offers to bring the work back. Projects you opened recently are in *File → Recent*.

OBCEdit's own settings live in your user profile (`OBCR/OBCEdit`); nothing is written into the game's folder or OBCR's installation.

## 12. Studying the original floors

*File → Import map from archive…* (**Ctrl+I**) opens the original game's `MAPHEAD.BC`/`MAPTEMP.BC` (from your data folder) and copies a floor into your project, so you can see how the original was built. The game files are only read, never written.

An imported floor stays the original's work: OBCEdit marks it, and refuses to put it in a mod meant for sharing. To include an original floor in your campaign, add it as a **stock** floor instead (section 13). It is then played from each player's own copy of the game.

## 13. Building a mod

*File → Campaign…* describes your mod and its floors, in the order they are played:

- **The mod**: its **id** (a-z, 0-9 and `-`; keep it the same across versions, since saves and scores are filed under it), **title**, author, version, description, and which **release** it needs (any, floppy or CD).
- **The floors**: each row is one of your maps or an original floor (**stock**), with an optional name and the original floor it should **behave like**. That choice brings darkness, sewer textures, colours, music and the sequence after the floor. Each can be changed: music (a song or random), dark, sewer, colours, the sequence (auto, F1–F7 or none), a message shown when the floor starts, and whether the exits work at all.

Everything is checked as you type, with the same list of problems the command line prints. *OK* stays disabled while there are errors. The meaning of every field is in [modding.md](modding.md#the-manifest-modtoml).

Then:
- *File → Export mod…* (**Ctrl+E**) writes the mod as a folder or as a single `.obcmod` file, reads it back to check it, and asks OBCR to check it too.
- *File → Install in OBCR* (**Ctrl+Shift+E**) writes it straight into OBCR's mods folder. Start OBCR, press **D** in the main menu, choose your mod and START GAME.

## 14. Keyboard reference

| Keys | |
|---|---|
| **Ctrl+N** / **Ctrl+O** / **Ctrl+S** / **Ctrl+Shift+S** | New, open, save, save as |
| **Ctrl+M** / **Ctrl+I** | New map, import a map from the original game |
| **Ctrl+E** / **Ctrl+Shift+E** | Export mod, install in OBCR |
| **Ctrl+Z** / **Ctrl+Y** | Undo, redo |
| **Ctrl+C** / **Ctrl+V** | Copy, paste a selection |
| **S B L R F E I P K** | Select, paint, line, rectangle, fill, erase, pick, place, camera |
| **F5** / **Shift+F5** / **Ctrl+F5** | Test, test from the cursor, stop the test |
| **F7** / **F8** | Snapshot, check this map |
| **Ctrl+=** / **Ctrl+−** / **Ctrl+G** | Zoom in, zoom out, grid |

## 15. The command line

The editor's core runs without a window, which is useful for scripts and checks. From a checkout, in `obcedit/`, run `python -m obcedit_core <command>`:

| Command | |
|---|---|
| `inspect ARCHIVE` | List the maps of a `MAPHEAD.BC`/`MAPTEMP.BC` pair (a folder or either file). |
| `validate ARCHIVE` | Check maps against the game's rules (`--map N`, `--all`). |
| `verify-archive ARCHIVE` | Decode and re-encode an archive and report whether the bytes come out identical (they do for both releases). |
| `catalog-info` | Summarize the catalog. |
| `project-new`, `project-import`, `project-inspect` | Make a project, copy a map into it, list it. |
| `mod-export PROJECT --output PATH` | Export a project as a mod (a folder, or a `.obcmod` when PATH ends so). |
| `mod-audit PATH` | Read a mod back and report what is in it. |

`--help` on any command gives its options. The packaged editor, `OBCEdit --selftest`, reports what a build is without a display or game data.

## 16. Known limitations

- **Maps and settings only.** A mod arranges what the game already has: no new walls, sprites, sounds or music yet.
- **Patrol routes** are placed arrow by arrow from the *Markers* tab; there is no route-drawing tool yet.
- **Test from the cursor** always faces east; **snapshots** are always in the original 320×200 picture.
- **The test floor** (F5) uses default floor settings; export the campaign to test darkness, music and sequences.
- **Network floors** cannot be made into mods yet.
