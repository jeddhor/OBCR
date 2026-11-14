# Playing the CD release

Steam and GOG sell Operation Body Count as its **CD-ROM release**, packaged as a disc image (a `.cue` file and a large `.bin` file) that their DOSBox setup mounts as a CD. OBCR reads that image directly. This page covers what is on the disc, how to install from it, the movies and the music, and how the CD version plays differently from the floppy one.

The technical record behind it (file hashes, the executable's layout, every code difference with its address) is [re/CD_RELEASE.md](re/CD_RELEASE.md); the decision is [ADR-022](DECISIONS.md).

---

## What is on the disc

| | |
|---|---|
| The game | `BCCD.EXE` and the `.BC` data files, in the disc's root folder |
| Movies | `F1.BC` (the opening, 456 frames), `F5.BC` (after floor 23, 177 frames) and `F7.BC` (the victory, 450 frames): FLC animations that exist only on the CD |
| The floppy's sequences | `F2.BC`, `F3.BC`, `F4.BC`, `F6.BC`, identical to the floppy release's |
| Music | Four Red Book audio tracks (disc tracks 2–5, about 7½, 5¾, 7¼ and 7 minutes) |
| Not used | `INSTALL.EXE`, `SETUP.EXE`, `README.EXE`, `GFXINFOV.BC`, the batch files |

The CD version is a **later build** of the game than the floppy release (`COUNT.EXE`, files dated 1996-12-24). Its maps, screens, sounds and AdLib music are byte for byte the same, but its executable and sprite file differ (see [How it plays differently](#how-the-cd-version-plays-differently)).

A Steam installation also has a `C/BCCD` folder with the game's files copied out of the image, and OGG copies of the music. OBCR can run from that folder, but without the music; the image gives you everything, so use it.

---

## Installing from the image

**With Setup** (recommended): on *Your game files*, choose **CD image…** and pick the `.cue`. Setup suggests any `.cue` it finds under the usual Steam and GOG folders. It checks the files on the image, then extracts what OBCR uses into the installation's `data` folder and **rips the four music tracks**. The music takes about 57 MB and the whole install about 15 seconds. The image itself is not copied.

**Without Setup:**

```sh
obcr --check-data /path/to/BODYCOUNT.cue            # what is on it, nothing written
obcr --extract-data /path/to/BODYCOUNT.cue ~/OBC-cd  # extract and rip
obcr --data ~/OBC-cd
```

**What OBCR can read**: a `.cue` with its `.bin` (one file, or one file per track), a `.bin` whose `.cue` sits beside it with the same name, or a plain `.iso` (which has no music). The data track may be MODE1/2048, MODE1/2352 or MODE2; audio may be little- or big-endian. Nothing external is needed (no bchunk, no ffmpeg, no mounting).

**The music files.** The tracks are ripped to `TRACK02.QOA` … `TRACK05.QOA` (named after the disc tracks) in [QOA](https://qoaformat.org), a compact lossy format that OBCR decodes itself. Other tools can play them too, ffmpeg among them. Delete them and the game plays the AdLib music.

---

## The movies

| Movie | When it plays |
|---|---|
| `F1` | At start-up, before the story page and the title: a pilot flying in over the sea at sunset. |
| `F5` | After floor 23, the top of UN Building 1. |
| `F7` | After the victory screen, before the high scores. |

Esc, or the gamepad's Start or B, skips a movie. The CD version plays these three as fast as the CD drive could read them, rather than at a fixed frame rate. OBCR shows them at about 12 frames a second, which is what a double-speed CD-ROM drive of the time managed. The floppy's four sequences keep their own timing. With *XBRZ MENUS* on, movies are upscaled like the other screens.

---

## The music

With the music ripped, *OPTIONS → MUSIC SOURCE* is **CD AUDIO**, and OBCR plays the CD's tracks the way `BCCD.EXE` did with its `cdmusic` parameter:

- There is **no AdLib music at all**, not even in the menus: the CD version plays only CD audio in that mode, so the title and menus are silent until a game starts a track.
- The game keeps a **current track**, starting at the first of the four. Starting a floor plays the current track unless one is already playing.
- **The track does not change with the floor.** The original only moves to another track when you ask it to.
- **Alt+P** plays (or restarts) the current track, **Alt+S** stops the music, **Alt+F** goes on to the next track; each shows *CD : PLAY*, *CD : STOP* or *CD : ADVANCE TRACK*. A quirk kept from the original: Alt+F on the last track plays the first track but makes the second one current.
- The movies stop the music first, and the next floor starts it again.
- **F2** (music on/off) stops and restarts the track. That is OBCR's addition; the original's F2 did nothing to CD audio.

**When a track ends**, the original fell silent until you finished the floor, and then played the **same** track again. *OPTIONS → CD TRACK END* chooses:

- **NEXT TRACK** (default): go straight on to the next track, wrapping from the fourth to the first, so the music never stops.
- **ORIGINAL**: the original's silence until the next floor.

*MUSIC SOURCE → ADLIB* plays the floppy's AdLib music instead, as if the CD were not there. Music volume applies to both.

---

## How the CD version plays differently

OBCR plays each release as its own executable did. Nothing here is an option. Played from the CD's files, these are the differences from the floppy version:

**Removed**
- The ghostly **face** that could fly at you from floor 7 on.
- The **Alt+B BEWITCHED** cheat, and the developers' sparkle effect.
- The *UN BUILDING 2* greeting on floor 24.
- An open manhole left behind when a fat man climbs out of one.

**Changed**
- **Damage flashes** scale with the damage taken (a fifth of it) instead of always the same flash, and small hits do not flash at all. Repeated hits add up.
- At low health, the chance of **blacking out** applies in god mode and in network games too.
- **Grenades**: a direct hit on you does less (30 plus up to 31 points instead of up to 157), and a direct hit on an enemy does more on harder game types. The splash around a grenade does the grenade's damage halved for every tile of distance (the floppy took 1 point per tile off a flat 100), so a grenade that hits a wall, which carries no damage of its own, splashes for nothing. The blast still kills everything in its eight-tile box.
- **Grenades blow push walls open**; the floppy spared them.
- **Saucer drones** fire at any range, not only when they reach you, and their attack animation is slower.
- One map object (number 71) **blocks** your way in the CD version; in the floppy it does not.
- Reaching the exit of the last floor says *LAST FLOOR*.
- The CD's sprites are **retouched**: 386 of them differ in their pixels, and the face and sparkle sprites are gone.

**Not reproduced**
- The CD version also runs its AutoSave and allows F9 quick saves in network games, and moves the PAUSE text with the view size. OBCR keeps saving off in network games, and draws PAUSE where the floppy did.
- **Flame direct hits**: the CD version passes a value it never sets as the damage, so its real amount cannot be read from the program. OBCR uses the floppy version's average, 94 against you and 158 against an enemy. Watching the original in a debugger would settle it (open question Q-036).

---

## Saved games and network play

Saved games remember which release made them. A game saved with the CD files cannot be loaded with the floppy files, and the other way round, because the two versions' behaviour tables differ. In the same way, a CD copy and a floppy copy cannot play a network game together.
