# OBCEdit

The level editor for Operation Body Count that ships with OBCR. It opens the
original maps read-only, draws new 64x64 floors, checks them against the
game's rules, playtests them in OBCR, and exports them as a **mod** OBCR plays
(a folder, or a `.obcmod` file).

OBCEdit is a port of **EC7Edit**, the editor from the EC7Wolf port of Corridor 7
(`EC7Wolf/editor`): its document model, undo, project files, path safety and
GUI are kept; its Corridor 7 codec, catalog, rules and ECWolf export are
replaced with Operation Body Count's. The plan is
[`docs/MODS_AND_OBCEDIT.md`](../docs/MODS_AND_OBCEDIT.md); the mod format is its
section 3.

This file is about the source tree. Its version is OBCR's.

## Layout

| Path | What it is |
| --- | --- |
| [`obcedit`](obcedit) | Run the GUI from a checkout, without installing |
| [`obcedit_core/`](obcedit_core/) | The Qt-free half, standard library only: codec, catalog, document, commands, project, validation, mod export, OBCR runner, CLI. The GUI depends on it; it never depends on the GUI |
| [`obcedit_gui/`](obcedit_gui/) | The PySide6 layer: window, canvas, palette, dialogs, workers |
| [`resources/catalog.json`](resources/catalog.json) | Every map word's meaning, **generated from OBCR's code** by `cargo run -p obc-tool -- catalog --out obcedit/resources/catalog.json`; `cargo xtask test-data` fails when it is stale. Names, kinds, small integers; no pixels |
| [`docs/native-formats.md`](docs/native-formats.md) | MAPHEAD.BC / MAPTEMP.BC as implemented, checked against the engine and both releases |
| [`scripts/make_fixtures.py`](scripts/make_fixtures.py) | Generates the synthetic test corpus (`write`, `digests`, `verify`) |
| [`tests/unit/`](tests/unit/) | Plain `unittest`, no runner dependency |

## The core, module by module

| Module | Purpose |
| --- | --- |
| `archive` | MAPHEAD.BC + MAPTEMP.BC reader and canonical writer; `verify_archive` re-encodes an archive and compares bytes |
| `rlew`, `names`, `planes` | RLEW planes, the raw 16-byte map name, the plane model |
| `catalog` | Palette entries from `catalog.json`: walls, specials (doors, door-look walls, glass, fans, exits, generators), floors, objects, enemies (one entry per kind and difficulty tier, four facings), starts, markers |
| `document`, `commands`, `transforms`, `tools` | Immutable revisioned documents, undoable commands, copy/rotate/flip (facings and door axes follow), line/rectangle/fill geometry |
| `rules`, `prefabs` | OBC's placement rules (door axis in the code, two-cell push walls) and one-click compound tools |
| `validation`, `reachability` | Errors and warnings with `OBC-*` codes; can the start reach an exit and enough enemies for the half-kill quota |
| `campaign`, `modexport` | The `mod.toml` schema; build, write and audit a mod |
| `engine_runner`, `snapshot`, `discovery` | OBCR's command line (`--mod`, `--play`, `--start`, `--check-mod`, `--shot`); first-run checks |
| `project`, `paths`, `bundle` | `.obcproject` files and their safe save; never writing into game data; frozen-build paths |
| `cli` | `python -m obcedit_core` |

## Running

```console
$ ../.local/obcedit-venv/Scripts/python obcedit                # the GUI (needs PySide6)
$ python -m obcedit_core inspect path/to/game                     # the 61 original maps
$ python -m obcedit_core verify-archive path/to/game              # byte-identical re-encode
$ python -m obcedit_core project-new mine.obcproject --name "My Mod" --room LOBBY
$ python -m obcedit_core mod-export mine.obcproject --output mine.obcmod
$ python -m obcedit_core mod-audit mine.obcmod
$ obcr --mod mine.obcmod                                          # play it
```

`project-import ARCHIVE --project P --map SLOT` copies an original floor into a
project to study or remix. Such a map is the game's, not yours: a mod meant for
sharing refuses it (`OBC-MOD-009`); reference the original floor with a `stock`
level instead, or export with `--private` for your own playtests.

## Tests

```console
$ cd obcedit
$ python -m unittest discover -s tests/unit
```

The core's tests need nothing but Python. Some are gated: with game data in
`OBC_DATA_DIR` (or `../.local/data/obc-full`, `obc-cd`) they also re-encode
both releases' archives byte for byte, validate every original floor and decode
every catalogued sprite; with an OBCR build in `../target/` they run
`obcr --check-mod` on the editor's exports and take a snapshot through the
engine.

## Two things worth knowing

**Re-encoding is byte-identical.** The writer uses EC7Edit's run threshold of
four and keeps each map's original place in MAPTEMP.BC (the original stores
slot 56 last), so an archive rewritten by OBCEdit differs from the original
only where somebody edited it.

**Fixtures are generated, never stored.** No test contains a byte of the
game. Plane words come from `0xE000` upward, a band the game's maps never use,
and `make_fixtures.py verify` regenerates and compares.
