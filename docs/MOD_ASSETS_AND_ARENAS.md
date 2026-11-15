# Mod art, sounds, music, network arenas and patrol routes — the plan

**Milestone M18.** Status: planned 2026-11-15, being implemented on `m18/*` branches.

M17 ([MODS_AND_OBCEDIT.md](MODS_AND_OBCEDIT.md)) gave OBCR mods made of maps and per-floor settings, and OBCEdit to make them. It deferred three things on purpose (its §1 non-goals and §8), and the owner has now asked for all three, plus one editor tool:

1. **New art, sounds and music in mods**, made with OBCEdit.
2. **Network floors in mods**: a mod can bring its own arenas for multiplayer games.
3. **A patrol-route tool** in OBCEdit.
4. Also, from the 1.0.0-beta.3 release: **a slimmer Linux installer** (it was 128 MB against Windows' 46 MB).

This document is the plan for all four: what a mod may now contain, what the engine changes, what OBCEdit gains, how it is tested, and the order of work. Decisions that outlive the milestone go to [DECISIONS.md](DECISIONS.md) as ADR-025 (mod format 2: assets) and ADR-026 (mod arenas and network identity).

---

## 1. Goals and non-goals

**Goals**

- G1. **Art**: a mod can replace any of the game's 136 wall pages (walls, doors, and the floors and ceilings, which are wall pages too) and any of its 680 sprites (objects, enemies and their animation frames, the weapons in hand, splats), with 64×64 PNG images. Everything that draws a page or sprite, including the Enhanced mode's xBRZ upscaling, draws the replacement.
- G2. **Sounds**: a mod can replace any of the 99 digitized sounds with a WAV file.
- G3. **Music**: a floor can play a music file the mod brings, an **Ogg Vorbis**, **QOA** or **IMF** (AdLib) file, instead of one of the original's ten AdLib songs.
- G4. **Arenas**: a mod can bring up to ten network floors. A network game started while that mod is active plays them instead of the original arenas, and only machines with the identical mod can join each other.
- G5. **OBCEdit** imports art, sounds and music, shows imported art in its palette, map and snapshots, builds arenas, and has a **patrol-route tool**: click the turns of a route, and it places the arrows.
- G6. **Unchanged rules**: with no mod active, the original game is bit-identical (soak, save and network tests unchanged). A mod's assets change only what is drawn and heard, never the simulation.
- G7. **Still safe to receive**: images, sounds and music are untrusted input, decoded by bounded, memory-safe decoders with size limits. A bad file is a clear error in the MODS screen, never a crash.
- G8. **Still no original data in a mod or the repository.** A mod carries the author's own files. OBCEdit never exports the game's own pages, sounds or palette.

**Non-goals (later, recorded)**

- New sprites or walls *in addition to* the original's: the engine's map codes, state tables and sound ids are the original's fixed numbers, so assets replace, they do not add. New map codes or actor behaviour stay out (DESIGN: no scripting).
- High-resolution art (bigger than 64×64) for Enhanced mode.
- Replacing full-screen pictures (title, status bar, menus), fonts or the FLC sequences.
- Per-floor (rather than per-mod) art sets.
- Hearing sounds and music inside OBCEdit (Qt Multimedia is pruned from the package); they are heard with F5 in OBCR.

---

## 2. Mod format 2

### 2.1 Layout

A mod is still a folder or a `.obcmod` ZIP. Format 2 adds four optional folders:

```
mod.toml
MAPHEAD.BC, MAPTEMP.BC
README.txt
walls/NNN.png      replaces wall page NNN (0-135), 64x64
sprites/NNN.png    replaces sprite NNN (0-679), 64x64, transparent where alpha < 128
sounds/NN.wav      replaces digitized sound NN (0-98)
music/NAME.ogg     music files a floor or arena names (also .qoa, .imf)
```

File names are matched case-insensitively. A file in these folders that does not match the pattern is an error (a mistyped number must not be silently ignored); files anywhere else are ignored and reported, as in format 1. A format-1 manifest that has these folders gets a warning that they are not used.

**Why PNG and nearest-colour, not palette-indexed data.** Authors draw in ordinary tools. OBCR converts each image to the game's palette when it loads it, by one rule that OBCEdit's preview implements identically (§2.4). The mod therefore never contains the game's palette (which is original data), and an author's file stays editable.

**Why page numbers.** The engine draws page numbers: map code *n* draws wall page *n − 1*, doors draw pages 128–135, floors and ceilings draw the pages named by the floor sets. OBCEdit names every page and sprite (its catalog and thumbnails), so authors pick from pictures, not numbers.

### 2.2 The manifest

`format = 2`. Everything in format 1 is unchanged. Additions:

- **`[[level]]`**: `music` also accepts a file name in `music/`, for example `music = "theme.ogg"`.
- **`[[arena]]`**: a network floor, at most 10, played in order:

  | Key | Values | Default | |
  |---|---|---|---|
  | `map` | 0–99 | required | The mod's own map. (`stock` is not offered: the original arenas are what a mod without arenas plays.) |
  | `name` | up to 16 characters | the map's name | |
  | `music` | 0–9, `"random"`, a file | `"random"` | As for levels. |
  | `colors` | 0–59 | 50 + index | Roof and floor colours (network floors have their own entries, 50–59). |
  | `dark`, `sewer` | true, false | false | As for levels. |

OBCR refuses `format` higher than it knows, as now, so a format-2 mod in beta.3 is refused with a clear message.

### 2.3 Limits

| | Limit |
|---|---|
| Images | PNG, 64×64, any bit depth and colour type the `png` crate reads (interlaced included); at most 256 KiB each |
| Sounds | WAV PCM 8/16-bit or float, mono or stereo, 4,000–48,000 Hz, at most 30 s and 4 MiB |
| Music | Ogg Vorbis, QOA (mono/stereo, up to 48 kHz) or IMF; at most 16 MiB each, 64 music files |
| The package | at most 2,000 files (was 1,000) and 128 MiB in all (was 64); one file at most 16 MiB (unchanged) |

### 2.4 The palette rule

For every pixel: alpha < 128 → transparent (index 255, sprites only; a wall pixel is always opaque). Otherwise the palette entry 0–254 with the smallest squared RGB distance to the pixel, the lowest index on a tie. One rule, written down here, implemented in Rust (`obc-render::quantize`) and Python (`obcedit_core.artimport`), and checked by shared test vectors that use a synthetic palette (`docs/test-vectors/quantize.json`), so no game palette is ever in the repository.

### 2.5 Checks (`obcr --check-mod`)

Each asset is decoded in full when a mod is checked, so a broken file shows in the MODS screen rather than mid-game: wrong size, bad PNG, unsupported WAV, undecodable Vorbis or QOA, an IMF whose length field is wrong, an unknown page or sound number, a level or arena naming a music file that is not there, a music file nothing uses (warning).

---

## 3. Engine changes (OBCR)

### 3.1 Reading assets (`obc-formats`)

- `modpack`: `Manifest` format 2, `ArenaEntry`, `MusicSpec::File`; `Mod` keeps an index of its asset files (name → entry), read on demand; the checks of §2.5 in `modpack::check_assets`.
- `zip`: limits raised as in §2.3.
- New `modassets` module: `decode_png` (the `png` crate, already a dependency), `decode_wav` (`hound`, already in the workspace), the Vorbis probe (`lewton`, MIT/Apache-2.0, pure Rust, new), the QOA and IMF parsers that exist.

### 3.2 Art (`obc-render`, `obc-ui`)

- `obc-render::quantize`: the §2.4 rule, with a 32 KiB lookup cache per palette.
- `Shape::from_indexed(pixels)` builds the column posts of a sprite from a 64×64 indexed image (the inverse of `decode_shape`), so a replaced sprite is drawn by the same code as an original one.
- `ModArt`: the replaced walls and sprites, converted when a mod game starts, plus their own xBRZ caches (`OnceLock`s, built lazily like the base ones).
- A `Layered<'a> { base: &Assets, art: &ModArt }` implements the renderer's `Textures` trait: a replaced page or sprite comes from the mod, everything else from the game. `Game` passes it where it passes `&Assets` now (the 3D view and the weapon). With no mod, `Game` passes `&Assets` as before: the original path is untouched.
- Floors and ceilings are wall pages, so they follow automatically; so do push walls, doors, thin walls and the door frame.

### 3.3 Sounds

`SoundManager` takes an override table: for a replaced id, the WAV converted to 8-bit unsigned mono at its own rate (the mixer resamples any rate). The id's AdLib chunk is still read for its priority and is played when digitized sound is off, so priority behaviour is the original's. Menu sounds (`play_digi`) in a mod game use the same table.

### 3.4 Music

- `LevelMusic::File(name)`; `level_song` returns no song number for it and consumes no random number (the RNG call order of `Random` and `Table` floors is unchanged).
- The mixer's CD voice becomes a **stream voice** over a `Source` (QOA or Vorbis), with a loop flag: CD tracks play once as now; mod music loops. A Vorbis file is decoded a packet at a time on the audio thread, like QOA frames, never whole.
- An IMF file plays through the existing OPL path.
- A floor with mod music plays it whatever MUSIC SOURCE says (AdLib or CD); with music off, nothing plays.
- `obcr --convert-audio IN.wav OUT.qoa` (OBCR's QOA encoder) lets OBCEdit turn a WAV into compact music without a Python encoder.

### 3.5 Network arenas

- `Campaign.arenas`: the mod's arenas (empty for the original game or a mod without them).
- `Game::start_level` in a network game: with arenas, `mapon` 50 + *i* plays arena *i* with its profile; taking an exit on the last arena returns to the first (the original's last arena has no exit, so this never arises there). Without arenas, the original arenas as now.
- **Identity**: the network header's release id becomes FNV-1a of the release name, and, with a mod active, its id, version and the SHA-256 of its contents (manifest, maps, assets). Peers with a different mod, or a different version of it, are refused exactly as a different release is now, with no protocol change. The setup screen shows the arenas' source: `ARENAS: ORIGINAL` or `ARENAS: <mod title>`, and the mismatch line says `other version, game type or mod`.
- **Checks for arenas** (`check_map` with an arena profile): exactly one player start (used as the reference point), **no enemies** (an error: in a network game they spawn but cannot be killed, which leaves the elevator exit unusable), and a warning for open floor that cannot be reached from the start (network players reappear on random free cells).
- **Respawn safety**: in a mod arena, `random_spot` picks only cells connected to the player start (a flood fill through floor and doors, computed when the floor loads). The original arenas keep the original's rule.

### 3.6 Saves

`LevelMusic::File` is a new variant at the end of the enum, so format-4 saves of original and format-1 mod games still read. Saves do not contain assets; loading a mod save re-reads the mod, as now.

### 3.7 Tests (engine)

- Quantizer vectors (shared with Python); `Shape::from_indexed` round trip against `decode_shape` on generated shapes (and, data-gated, on every original sprite: decode → re-encode → same pixels).
- PNG, WAV, Vorbis, QOA, IMF: good and hostile files (truncated, oversized, wrong dimensions, absurd headers) — proptest where a decoder is ours.
- A generated format-2 mod with a replaced wall, sprite, sound and a music file: `--check-mod` passes; a test renders a frame with and without it and finds the replaced page's pixels only with it; the stream voice plays Vorbis and loops.
- Arenas: two in-process network machines with the same mod play its arena; a machine with a different mod is refused; exits wrap; the respawn region is the start's.
- Unchanged: every existing test, including the soak tests and the network tests on the original arenas.

---

## 4. OBCEdit

### 4.1 Project storage (schema 2)

A project gains `resources`: one record per replaced page, sprite or sound, and per music file — `{kind, target, file, sha256, source_name}`. The bytes live beside the project in `<project>.assets/` (`walls/012.png`, `sprites/0450.png`, `sounds/17.wav`, `music/theme.ogg`), the same layout as the mod, so export copies them. An unsaved project keeps them in the workspace (`unsaved/<uuid>/`) and moves them on first save. Schema 1 projects migrate (no resources). Export refuses a resource whose file is missing or whose digest no longer matches, naming it.

### 4.2 Importing (`obcedit_core.artimport`, `audioimport`)

- **Art**: PNG (OBCEdit's stdlib reader, extended to keep alpha, 16-bit and palette images) → checked 64×64 → quantized by the §2.4 rule for the preview. The file stored is the author's PNG, so OBCR's conversion and the preview agree by construction of the rule, not by copying converted pixels.
- **Sounds**: WAV checked against §2.3 (header and length).
- **Music**: Ogg/QOA/IMF checked by header; WAV converted to QOA through `obcr --convert-audio`.
- Every import is one undoable command.

### 4.3 The Resources panel

A dock with four tabs, *Walls*, *Sprites*, *Sounds*, *Music*:

- Walls and sprites: a grid of every page or sprite with its name from the catalog and its thumbnail (the replacement's, marked, when there is one). Import / Replace / Revert / Export-to-PNG (of the author's own file only).
- Sounds: the 99 sounds by number and name; Import / Revert.
- Music: the project's music files; Import / Rename / Remove; where each is used.

Replaced art shows everywhere: the palette's thumbnails, the map's object icons and floor tints, and snapshots (which render through OBCR with the preview mod, which now carries the assets).

### 4.4 Campaign: arenas and music

The campaign dialog gains an **Arenas** list beside the floors (up to 10, own maps only) with name, music and colours; a level's or arena's music can name an imported file. Validation uses the **arena** profile for maps used as arenas (no enemies, reachable floor), whatever their slot, matching OBCR's checks.

### 4.5 The patrol-route tool

- **Tool** *Patrol route* (shortcut **R**): click the first cell of a route (normally beside a patrolling enemy, or the enemy's own cell), then click each turn; double-click, Enter or clicking the first cell again finishes; Esc cancels. Segments are straight horizontal, vertical or 45° diagonal lines; the canvas previews the route as it is drawn.
- **Arrows** are placed on every turn cell, pointing along the next segment (90–97: E, NE, N, NW, W, SW, S, SE). A closed route also gets an arrow where it closes. One command, one undo step.
- **Refusals** (the preview turns red and says why): a segment through a wall or closed cell; a turn on a cell holding an object, marker or actor (one word per cell), except that a route may *start* on the patrolling actor's own cell, where no arrow is needed if the actor already faces the first segment, and otherwise the tool offers to turn the actor to face it.
- **Pure core**: `obcedit_core.patrol.plan_route(document, cells, catalog) -> RouteResult` (writes or the reason), tested without Qt.
- **Validation** gains the planned rules: a patrol arrow that no patrolling actor's path reaches (warning), and a patrolling actor whose path walks into a wall without an arrow (warning: it would stop there). Paths are simulated with the engine's rule: walk the facing, re-read the arrow at each cell centre.
- The canvas draws arrows as arrows (today markers are a coloured inset).

---

## 5. The Linux installer (M18.0)

Measured on Ubuntu 22.04: the frozen OBCEdit is 143 MB on disk, but the installer packs it as more because `xtask pack-installer` follows symbolic links, and PyInstaller's Linux freeze has 16 links to its biggest libraries (ICU data 32 MB, Qt Core/Gui/Widgets, PySide6), so each is packed twice. Also unneeded: Qt's translations (7 MB; the editor is English only), the GTK platform theme and the GTK libraries it pulls in (~15 MB), the EGL device-integration plugins.

- The installer payload records symbolic links as links (a new entry kind in the packer and in `obcr-setup`'s extractor), Setup recreates them as links and lists them in the install manifest, and the uninstaller removes them.
- `package.py` prunes the translations, the GTK theme and its libraries, and the EGL integrations on Linux, and reports the size without following links.
- Acceptance: the Linux installer built in CI is under 70 MB, and the frozen editor still passes `--selftest`, and opens a window under WSLg (xcb and Wayland).

---

## 6. Order of work

| Task | What | Acceptance |
|---|---|---|
| M18.0 | Linux installer: links kept, Qt pruned | CI Linux installer < 70 MB; selftest; window under WSLg |
| M18.1 | This plan; ADR-025, ADR-026 | Committed |
| M18.2 | Format 2: manifest, arenas, asset index, checks, limits, decoders | Unit, hostile-file and proptest tests; format-1 mods unchanged |
| M18.3 | Art: quantizer, `Shape::from_indexed`, `ModArt`, `Layered` textures, xBRZ | Vectors; round trips; render test with and without a mod |
| M18.4 | Sounds and music: overrides, stream voice (QOA, Vorbis, loop), IMF, `LevelMusic::File`, `--convert-audio` | Mixer tests; a mod floor's music plays and loops; RNG order unchanged |
| M18.5 | Arenas: campaign, network start and wrap, identity hash, setup screen, arena checks, respawn region | In-process network tests with mods; mismatch refused; original network tests unchanged |
| M18.6 | OBCEdit: schema 2, resources, import, Resources panel, thumbnails and snapshots with replacements, export | Core and GUI tests; exported mod passes `obcr --check-mod`; snapshot shows a replaced wall |
| M18.7 | OBCEdit: arenas in the campaign, arena validation, music by file | Tests; an exported arena mod plays in an `obcr --net` game |
| M18.8 | OBCEdit: patrol-route tool, arrow rendering, patrol validation | Core route tests; GUI test draws a loop; validation finds orphan arrows |
| M18.9 | Docs (modding guide, manual, README, CHANGELOG, notices for `lewton`), as built | Links checked; `cargo xtask ci`, `test-data`, OBCEdit tests |

---

## 7. Risks

| Risk | Mitigation |
|---|---|
| A decoder bug in an untrusted image or sound | Only maintained pure-Rust decoders (`png`, `hound`, `lewton`, our QOA); size checked before decoding; hostile-file tests; decoding only in `--check-mod` and at game start, never on the audio thread except Vorbis packets of a file that already decoded fully once |
| Preview and game convert colours differently | One written rule (§2.4), shared vectors run by both test suites |
| Replacements break the original path | `Layered` is used only when a mod has art; the soak tests run the original path |
| Network peers with mismatched mods desynchronise | The identity hash covers every byte of the mod; mismatches never connect |
| The audio thread stalls decoding Vorbis | Packet-at-a-time decoding like QOA; a test plays a long file and measures the callback time |
| Package size grows (lewton) | Small (pure Rust, ~200 KB); checked |
| Project files referencing missing asset files | Digest check on open and export, with the missing file named |

---
