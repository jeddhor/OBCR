#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The OBCEdit slice, driven through the real window against real game data.

    slice_workflow.py DATA_DIR WORK_DIR [OBCR]

Does, in order, what a person must be able to do with the editor:

  1. import floor 1 (slot 0) from the owned map archive, read-only;
  2. confirm the palette decoded real thumbnails (a wall page, a floor set's
     floor texture, an enemy sprite) from the artwork;
  3. paint a wall chosen from that palette;
  4. place an enemy and change its facing in the inspector;
  5. undo both and redo both;
  6. move the player start;
  7. save the project, reopen it, compare;
  8. build the one-floor preview mod a playtest uses and read it back;
  9. with OBCR: `--check-mod` accepts it, and a `--shot 30` snapshot from the
     new start draws a world.

Run by test_slice.py when the developer's own data is present (never in CI,
which has none). The archive is only ever read; its digest is checked at both
ends. Nothing it writes leaves WORK_DIR, and nothing of the game is printed.
"""

from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

EDITOR = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(EDITOR))

from PySide6.QtCore import QSettings, Qt
from PySide6.QtWidgets import QApplication

from obcedit_core import rules
from obcedit_core.archive import archive_paths
from obcedit_core.catalog import default_catalog
from obcedit_core.discovery import Profile, data_fingerprint
from obcedit_core.project import load_project

from obcedit_gui.application import build_application
from obcedit_gui.main_window import MainWindow
from obcedit_gui.settings import Settings
from obcedit_gui.tools import EMPTY_OBJECT, Tool

PLAYER_START_KEY = "player-start"
ENEMY_KEY = "enemy-116"          # a soldier, standing, every difficulty
WALL_KEY = "wall-2"


class Slice:
    def __init__(self, out=print):
        self.failures: list[str] = []
        self.out = out

    def check(self, condition, message):
        self.out(("  ok   " if condition else "  FAIL ") + message)
        if not condition:
            self.failures.append(message)
        return condition

    def note(self, message):
        self.out(f"  ..   {message}")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def open_floor(document, *, avoid=()):
    """The first open floor cell with nothing on it, scanning from the top."""
    for y in range(1, document.height - 1):
        for x in range(1, document.width - 1):
            if (x, y) in avoid:
                continue
            if rules.is_floor(document.cell(0, x, y)) and document.cell(1, x, y) < 19:
                return x, y
    return None


def run(data_dir: Path, work: Path, engine: Path | None = None, *, out=print) -> list[str]:
    """Run the slice; returns the failures (empty when everything held)."""
    s = Slice(out)
    data_dir, work = data_dir.resolve(), work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    _, archive = archive_paths(data_dir)
    before = digest(archive)

    application = build_application([])
    catalog = default_catalog()
    settings = Settings(QSettings(str(work / "settings.ini"), QSettings.IniFormat))
    settings.recovery_dir = work / "recovery"
    settings.profile = Profile(
        profile_id="slice", engine_path=str(engine or ""), data_dir=str(data_dir),
        workspace_dir=str(work), data_fingerprint=data_fingerprint(data_dir),
    )
    window = MainWindow(settings, catalog=catalog)
    try:
        _run(s, window, application, catalog, data_dir, work, archive, before, engine)
    finally:
        window.project = window.project.marked_saved(window.project.revision)
        window.reconcile_orphan()
        window.pool.cancel_all()
        window.pool.wait(2000)
        window.close()
        QApplication.processEvents()
    return s.failures


def _run(s, window, application, catalog, data_dir, work, archive, before, engine):
    s.out("Import")
    window.import_map(str(archive), 0)
    if not s.check(len(window.project) == 1, "floor 1 (slot 0) imported"):
        return
    uuid = window.project.maps[0].uuid
    document = window.project.map_by_uuid(uuid)
    s.note(f"{document.width}x{document.height}")
    s.check(document.source is not None and document.source.sha256 == before,
            "the import recorded the archive's digest")

    s.out("\nArtwork")
    s.check(window.thumbnails.available, "the game data opened for thumbnails")
    for key in (WALL_KEY, "floor-144", ENEMY_KEY):
        entry = catalog.by_key(key)
        try:
            pixels, edge, alpha = window.thumbnails.pixels_for(entry)
        except Exception as error:  # noqa: BLE001 - reported as a failure
            s.check(False, f"{entry.name}: {error}")
            continue
        s.check(len(pixels) == edge * edge * (4 if alpha else 3) and len(set(pixels)) > 4,
                f"{entry.name}: a real {edge}x{edge} thumbnail")

    s.out("\nPaint a wall from the palette")
    floor = open_floor(document)
    if not s.check(floor is not None, "found open floor to edit"):
        return
    wall = catalog.by_key(WALL_KEY)
    window._on_palette_clicked(type("I", (), {"data": lambda self, r: wall})())
    window._on_press(floor[0], floor[1], Qt.LeftButton.value)
    window.tools.release(*floor)
    document = window.project.map_by_uuid(uuid)
    s.check(document.cell(0, *floor) == wall.value, f"wall {wall.value} painted at {floor}")

    s.out("\nPlace and configure an enemy")
    enemy_cell = open_floor(document, avoid={floor})
    enemy = catalog.by_key(ENEMY_KEY)
    window._on_palette_clicked(type("I", (), {"data": lambda self, r: enemy})())
    window._on_press(enemy_cell[0], enemy_cell[1], Qt.LeftButton.value)
    window.tools.release(*enemy_cell)
    document = window.project.map_by_uuid(uuid)
    s.check(document.cell(1, *enemy_cell) == enemy.value,
            f"{enemy.name} placed at {enemy_cell} facing east ({enemy.value})")
    window.inspector.show_cell(document, *enemy_cell)
    window.inspector.direction.setCurrentIndex(window.inspector.direction.findText("North"))
    application.processEvents()
    document = window.project.map_by_uuid(uuid)
    north = dict(enemy.directions)["north"]
    s.check(document.cell(1, *enemy_cell) == north,
            f"the inspector turned it to face north ({north})")

    s.out("\nUndo and redo")
    depth = window.history.depth
    window.undo()
    window.undo()
    document = window.project.map_by_uuid(uuid)
    s.check(document.cell(1, *enemy_cell) == EMPTY_OBJECT,
            "undo took back the enemy and its facing")
    window.redo()
    window.redo()
    document = window.project.map_by_uuid(uuid)
    s.check(document.cell(1, *enemy_cell) == north, "redo put both back")
    s.check(window.history.depth == depth, "the history is where it started")

    s.out("\nMove the player start")
    start = catalog.by_key(PLAYER_START_KEY)
    old_start = next(((i % document.width, i // document.width)
                      for i, v in enumerate(document.planes.planes[1]) if v in start.values),
                     None)
    if not s.check(old_start is not None, "floor 1 has a player start"):
        return
    new_start = None
    for y in range(document.height - 2, 0, -1):
        for x in range(document.width - 2, 0, -1):
            if rules.is_floor(document.cell(0, x, y)) and document.cell(1, x, y) < 19 \
                    and (x, y) not in (floor, enemy_cell, old_start):
                new_start = (x, y)
                break
        if new_start:
            break
    from obcedit_core.commands import write_words

    window.run_command(write_words(
        document, [(1, *old_start, EMPTY_OBJECT), (1, *new_start, start.value_for("east"))],
        label="Move start"))
    document = window.project.map_by_uuid(uuid)
    s.check(document.cell(1, *new_start) == start.value_for("east"),
            f"the start moved from {old_start} to {new_start}")

    s.out("\nSave and reopen")
    project_path = work / "slice.obcproject"
    window.project_path = project_path
    s.check(window.save_project(), f"saved {project_path.name}")
    reopened = load_project(project_path)
    s.check(reopened.maps[0].planes.planes == document.planes.planes,
            "every word survived save and reopen")
    s.check(reopened.maps[0].native_name.raw == document.native_name.raw,
            "the 16-byte name survived")
    s.check(not reopened.dirty, "the reopened project is clean")

    s.out("\nThe preview mod")
    from obcedit_core.archive import parse_archive
    from obcedit_core.modexport import MAPHEAD_NAME, MAPTEMP_NAME, preview_mod

    build = preview_mod(window.project, uuid)
    parsed = parse_archive(build.files[MAPHEAD_NAME], build.files[MAPTEMP_NAME])
    s.check(len(parsed) == 1 and parsed[0].planes.planes == document.planes.planes,
            "the preview mod holds the project's words")
    errors = []
    window._error = lambda title, body: errors.append(f"{title}: {body}")
    s.check(window.export_mod(str(work / "shared.obcmod")) is None
            and any("original game" in e for e in errors),
            "an imported original floor is refused for a shared mod")

    if engine is None:
        s.note("no OBCR given; skipping --check-mod and the snapshot")
    else:
        s.out("\nOBCR")
        from obcedit_core.engine_runner import check_mod
        from obcedit_core.modexport import write_mod

        mod = write_mod(build, work / "preview-mod")
        result = check_mod(engine, mod, data_dir=data_dir)
        s.check(result.ok, f"obcr --check-mod accepts the preview ({result.result})")
        window.open_map(uuid)
        window.set_camera(new_start[0] + 0.5, new_start[1] + 0.5)
        taken = window.take_snapshot()
        s.check(taken, "a --shot 30 snapshot from the new start drew a world: "
                + window.snapshot_status.text())
        if taken:
            shot = window._snapshot_cache_dir() / f"{window._snapshot_key}.png"
            s.note(f"snapshot at {shot}")

    s.out("\nThe archive was only read")
    s.check(digest(archive) == before, "MAPTEMP.BC is unchanged")


def main(argv) -> int:
    if len(argv) < 3:
        print(__doc__)
        return 2
    engine = Path(argv[3]) if len(argv) > 3 else None
    failures = run(Path(argv[1]), Path(argv[2]), engine)
    print(f"\n{'FAILED' if failures else 'OK'}: {len(failures)} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
