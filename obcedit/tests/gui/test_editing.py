#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""E5: the editing slice, driven through the real window.

Tools, the inspector, validation and the playtest plan, exercised the way a
person uses them: pick something in the palette, press on the canvas, drag,
release, undo. The document is checked after each step, because the thing that
goes wrong in an editor is not that a tool computes the wrong cells -- it is
that the cells go to the wrong map, or the wrong plane, or arrive after the
document has already moved on.

Offscreen, synthetic, no game data. Every map is a real 64x64 Operation Body
Count floor: a 10x10 room in the top-left corner (floor 144), walls everywhere
else, and the explosion-proof marker (89) round the map edge.
"""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

EDITOR = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(EDITOR))

from PySide6.QtCore import QSettings, Qt
from PySide6.QtWidgets import QApplication

from obcedit_core.catalog import default_catalog
from obcedit_core.document import MapDocument, ProjectDocument
from obcedit_core.names import NativeName
from obcedit_core.planes import MapPlanes, linear_index
from obcedit_core.errors import Severity
from obcedit_core.validation import validate_map

from obcedit_gui.application import build_application
from obcedit_gui.main_window import MainWindow
from obcedit_gui.settings import Settings
from obcedit_gui.inspector import PREVIEW_SIZE, SECONDARY_SIZE
from obcedit_gui.tools import EMPTY_OBJECT, Tool

from gui_support import PYTHON, fake_engine, make_game_data

CATALOG = default_catalog()
_application = build_application([])

WALL_ENTRY = "wall-2"
#: A soldier standing facing east, on every difficulty; north is +1, west +2,
#: south +3, patrolling +4.
SOLDIER = "enemy-116"
SOLDIER_EAST = 116
START = "player-start"
SIZE = 64


def open_map(room=10) -> MapDocument:
    """A walled room in a 64x64 floor, which is where editing usually starts.

    Floored with 144 (area 4, texture set 0) -- word 0 is not floor in
    Operation Body Count -- and with 89 round the map edge, as the original
    maps have it.
    """
    plane0, plane1 = [], []
    for y in range(SIZE):
        for x in range(SIZE):
            inside = 0 < x < room - 1 and 0 < y < room - 1
            plane0.append(MapDocument.DEFAULT_FLOOR if inside else 1)
            edge = x in (0, SIZE - 1) or y in (0, SIZE - 1)
            plane1.append(89 if edge else EMPTY_OBJECT)
    return MapDocument("edit-uuid", 0, NativeName.from_text("ROOM"),
                       MapPlanes(SIZE, SIZE, (tuple(plane0), tuple(plane1),
                                              (0,) * (SIZE * SIZE))))


class Base(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.settings = Settings(
            QSettings(str(self.root / "s.ini"), QSettings.IniFormat)
        )
        # Never the user's real ~/.local/share: a suite that autosaves there
        # leaves work behind and reads other runs' leftovers.
        self.settings.recovery_dir = self.root / "recovery"
        self.window = MainWindow(self.settings, catalog=CATALOG)
        self.window.set_project(ProjectDocument.create().added(open_map()))
        self.uuid = self.window.project.maps[0].uuid
        self.window.open_map(self.uuid)

    def tearDown(self):
        self.window.project = self.window.project.marked_saved(self.window.project.revision)
        self.window.pool.cancel_all()
        self.window.pool.wait(2000)
        self.window.close()
        self.window.deleteLater()
        QApplication.processEvents()
        self._tmp.cleanup()

    @property
    def document(self) -> MapDocument:
        return self.window.project.map_by_uuid(self.uuid)

    def choose(self, key: str) -> None:
        """Pick a palette entry the way the view does.

        Through `_on_palette_clicked` rather than by setting the tool's entry
        directly: a shortcut here is a shortcut past the handler, and past the
        bug where choosing a wall left a structure armed.
        """
        entry = CATALOG.by_key(key)
        self.assertIsNotNone(entry, key)
        self.window._on_palette_clicked(
            type("Index", (), {"data": lambda _self, _role: entry})()
        )

    def stroke(self, tool: Tool, points) -> None:
        """Press, drag through the points, release -- one gesture."""
        self.window.select_tool(tool)
        first = points[0]
        self.window._on_press(first[0], first[1], Qt.LeftButton.value)
        for point in points[1:]:
            self.window.tools.drag(point[0], point[1], Qt.LeftButton.value)
        self.window.tools.release(*points[-1])


class Brush(Base):
    def test_a_click_paints_one_cell(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(3, 3)])
        self.assertEqual(self.document.cell(0, 3, 3), 2)

    def test_a_drag_paints_a_continuous_line(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(2, 2), (5, 2), (8, 2)])
        for x in range(2, 9):
            self.assertEqual(self.document.cell(0, x, 2), 2, f"gap at x={x}")

    def test_a_whole_stroke_is_one_undo(self):
        self.choose(WALL_ENTRY)
        before = self.document.planes.planes
        self.stroke(Tool.BRUSH, [(2, 2), (4, 2), (6, 2)])
        self.assertEqual(self.window.history.depth, 1)
        self.window.undo()
        self.assertEqual(self.document.planes.planes, before)

    def test_two_strokes_are_two_undos(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(2, 2)])
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.assertEqual(self.window.history.depth, 2)

    def test_painting_an_object_writes_plane_one(self):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.assertEqual(self.document.cell(1, 4, 4), SOLDIER_EAST)
        self.assertEqual(self.document.cell(0, 4, 4), MapDocument.DEFAULT_FLOOR,
                         "plane 0 was disturbed")

    def test_the_plane_comes_from_the_catalog_not_the_tool(self):
        # Same code path, different plane, because the entry says so.
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(3, 3)])
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])
        self.assertEqual(self.document.cell(0, 3, 3), 2)
        self.assertEqual(self.document.cell(1, 5, 5), 19)

    def test_painting_the_same_value_twice_records_nothing(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(3, 3)])
        depth = self.window.history.depth
        self.stroke(Tool.BRUSH, [(3, 3)])
        self.assertEqual(self.window.history.depth, depth)


class NewMap(Base):
    """Starting from nothing, which is the other way in besides importing."""

    def test_it_adds_a_map_and_opens_it(self):
        before = len(self.window.project)
        document = self.window.new_map(name="FRESH", slot=9)
        self.assertIsNotNone(document)
        self.assertEqual(len(self.window.project), before + 1)
        self.assertEqual(self.window.current_tab.map_uuid, document.uuid)

    def test_it_comes_walled(self):
        # A blank map is all floor, so the first thing the validator would say
        # is that the player can walk out of the world.
        document = self.window.new_map(name="WALLED", slot=9)
        document = self.window.project.map_by_uuid(document.uuid)
        for x in range(SIZE):
            self.assertNotEqual(document.cell(0, x, 0), MapDocument.DEFAULT_FLOOR)
            self.assertNotEqual(document.cell(0, x, SIZE - 1), MapDocument.DEFAULT_FLOOR)
            self.assertEqual(document.cell(1, x, 0), 89, "the edge is explosion-proof")
        self.assertEqual(document.cell(0, 5, 5), MapDocument.DEFAULT_FLOOR)

    def test_the_object_plane_starts_empty_not_zero(self):
        # The original editor's background is 18. The one exception is the
        # player start in the middle, and the 89 round the edge.
        document = self.window.new_map(name="EMPTY", slot=9)
        document = self.window.project.map_by_uuid(document.uuid)
        self.assertEqual(document.cell(1, 1, 1), EMPTY_OBJECT)
        self.assertEqual(document.cell(1, 6, 6), EMPTY_OBJECT)

    def test_a_new_map_can_be_tested_straight_away(self):
        # The whole point of the start: a fresh map has to pass validation, or
        # pressing F5 gets a refusal from OBCR's --check-mod.
        document = self.window.new_map(name="TESTABLE", slot=9)
        self.window.open_map(document.uuid)
        errors = [p for p in self.window.validate() if p.severity.name == "ERROR"]
        self.assertEqual(errors, [])

    def test_it_picks_a_free_slot(self):
        # The project already holds slot 0 from setUp.
        document = self.window.new_map(name="AUTO")
        self.assertEqual(document.slot, 1)

    def test_you_can_paint_on_it_immediately(self):
        document = self.window.new_map(name="PAINT", slot=9)
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(3, 3), (6, 3)])
        painted = self.window.project.map_by_uuid(document.uuid)
        for x in range(3, 7):
            self.assertEqual(painted.cell(0, x, 3), 2)

    def test_a_new_map_validates_once_it_has_a_start(self):
        document = self.window.new_map(name="VALID", slot=9, width=SIZE, height=SIZE)
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])
        codes = [p.code for p in self.window.validate()]
        self.assertNotIn("OBC-BOUNDARY-001", codes)
        self.assertNotIn("OBC-START-001", codes)


class Shapes(Base):
    def test_a_line(self):
        self.choose(WALL_ENTRY)
        self.window.select_tool(Tool.LINE)
        self.window._on_press(2, 2, Qt.LeftButton.value)
        self.window.tools.release(6, 2)
        for x in range(2, 7):
            self.assertEqual(self.document.cell(0, x, 2), 2)

    def test_a_rectangle_outline(self):
        self.choose(WALL_ENTRY)
        self.window.filled_box.setChecked(False)
        self.window.select_tool(Tool.RECTANGLE)
        self.window._on_press(2, 2, Qt.LeftButton.value)
        self.window.tools.release(5, 5)
        self.assertEqual(self.document.cell(0, 2, 2), 2)
        self.assertEqual(self.document.cell(0, 5, 5), 2)
        self.assertEqual(self.document.cell(0, 3, 3), MapDocument.DEFAULT_FLOOR,
                         "the middle was filled")

    def test_a_filled_rectangle(self):
        self.choose(WALL_ENTRY)
        self.window.filled_box.setChecked(True)
        self.window.select_tool(Tool.RECTANGLE)
        self.window._on_press(2, 2, Qt.LeftButton.value)
        self.window.tools.release(5, 5)
        self.assertEqual(self.document.cell(0, 3, 3), 2)

    def test_a_fill_stops_at_the_wall(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.FILL, [(5, 5)])
        # The room's interior is 8x8; its walls must be untouched.
        self.assertEqual(self.document.cell(0, 5, 5), 2)
        self.assertEqual(self.document.cell(0, 0, 0), 1)

    def test_a_fill_is_one_undo(self):
        self.choose(WALL_ENTRY)
        before = self.document.planes.planes
        self.stroke(Tool.FILL, [(5, 5)])
        self.window.undo()
        self.assertEqual(self.document.planes.planes, before)


class Erasing(Base):
    def test_erasing_a_wall_leaves_floor(self):
        # Floor, meaning 144 -- not word 0, which is not floor at all.
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(3, 3)])
        self.stroke(Tool.ERASER, [(3, 3)])
        self.assertEqual(self.document.cell(0, 3, 3), MapDocument.DEFAULT_FLOOR)

    def test_erasing_an_exit_leaves_floor_too(self):
        from obcedit_core.prefabs import by_key

        self.window.tools.set_prefab(by_key("prefab.exit.elevator"))
        self.stroke(Tool.PREFAB, [(9, 4)])
        self.assertEqual(self.document.cell(0, 9, 4), 115)
        self.stroke(Tool.ERASER, [(9, 4)])
        self.assertEqual(self.document.cell(0, 9, 4), MapDocument.DEFAULT_FLOOR)

    def test_erasing_an_object_writes_the_empty_marker(self):
        # Not zero: the empty object-plane word is 18.
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.stroke(Tool.ERASER, [(4, 4)])
        self.assertEqual(self.document.cell(1, 4, 4), EMPTY_OBJECT)


class Eyedropper(Base):
    def test_it_selects_what_it_finds(self):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.choose(WALL_ENTRY)
        self.stroke(Tool.EYEDROPPER, [(4, 4)])
        self.assertEqual(self.window.selected_entry.actor, "Soldier")

    def test_it_changes_nothing(self):
        self.choose(WALL_ENTRY)
        before = self.document.planes.planes
        self.stroke(Tool.EYEDROPPER, [(3, 3)])
        self.assertEqual(self.document.planes.planes, before)
        self.assertFalse(self.window.history.can_undo)


class Selection(Base):
    def test_dragging_the_pointer_selects_a_rectangle(self):
        self.window.select_tool(Tool.POINTER)
        self.window._on_press(2, 2, Qt.LeftButton.value)
        self.window.tools.drag(5, 4, Qt.LeftButton.value)
        selection = self.window.tools.selection
        self.assertEqual((selection.x, selection.y, selection.width, selection.height),
                         (2, 2, 4, 3))

    def test_selecting_changes_nothing(self):
        before = self.document.planes.planes
        self.window.select_tool(Tool.POINTER)
        self.window._on_press(2, 2, Qt.LeftButton.value)
        self.window.tools.drag(5, 5, Qt.LeftButton.value)
        self.window.tools.release(5, 5)
        self.assertEqual(self.document.planes.planes, before)


class StubAssets:
    """An AssetSource-shaped stand-in: flat color pages, no game data.

    The preview path is decode -> QImage -> scale -> cache, and all of that is
    the editor's own code. What it must not need is a copy of the game.
    """

    fingerprint = "stub"

    def wall(self, page):
        return bytes([page % 256, 40, 90] * (64 * 64))

    def sprite(self, page):
        return bytes([200, page % 256, 30, 255] * (64 * 64))


class ProblemsPanel(Base):
    """E7: the panel is continuous, filterable, honest about staleness, and
    can apply the repairs that have exactly one right answer."""

    def break_the_boundary(self):
        document = self.document
        planes = list(document.planes.planes)
        words = list(planes[0])
        words[linear_index(0, 0, document.width)] = MapDocument.DEFAULT_FLOOR
        planes[0] = tuple(words)
        self.window.project = self.window.project.with_map(
            document.with_planes(MapPlanes(document.width, document.height,
                                           tuple(planes))))

    def test_editing_updates_the_panel_without_being_asked(self):
        before = self.window.problems.count()
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(0, 5)])   # paints over the boundary: legal
        self.stroke(Tool.ERASER, [(0, 5)])  # opens it: not
        QApplication.processEvents()
        codes = [self.window.problems.item(i).toolTip().split("\n")[0]
                 for i in range(self.window.problems.count())]
        self.assertIn("OBC-BOUNDARY-001", codes)
        self.assertNotEqual(self.window.problems.count(), before)

    def legal_map(self, *, with_exit=False):
        """A start on the floor, so the reachability rules run at all."""
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])
        if with_exit:
            self.choose("special-115")
            self.stroke(Tool.BRUSH, [(6, 5)])

    def test_the_filter_hides_warnings(self):
        # A start makes the map legal enough to reach the advisory rules, and
        # the missing exit is then a warning next to the boundary error.
        self.legal_map()
        self.stroke(Tool.ERASER, [(0, 5)])
        self.window.validate()
        self.window.problem_filter.setCurrentIndex(
            self.window.problem_filter.findText("Everything"))
        with_warnings = self.window.problems.count()
        self.window.problem_filter.setCurrentIndex(
            self.window.problem_filter.findText("Errors only"))
        self.assertLess(self.window.problems.count(), with_warnings)
        self.assertIn("hidden by the filter", self.window.problem_status.text())

    def test_a_stale_result_says_so(self):
        self.window.validate()
        self.assertNotIn("earlier edit", self.window.problem_status.text())
        # An edit that changes nothing the quick pass looks at still moves the
        # revision, which is exactly the case a panel must not hide.
        self.window._problems_revision = -99
        self.window._repaint_problems()
        self.assertIn("earlier edit", self.window.problem_status.text())

    def test_a_fix_is_offered_and_applies(self):
        self.stroke(Tool.ERASER, [(0, 5)])
        self.window.validate()
        self.window.problem_filter.setCurrentIndex(
            self.window.problem_filter.findText("Errors only"))
        row = next(i for i in range(self.window.problems.count())
                   if self.window.problems.item(i).data(Qt.UserRole + 1) == "seal_boundary")
        self.window.problems.setCurrentRow(row)
        self.assertTrue(self.window.problem_fix.isEnabled())
        self.assertEqual(self.window.problem_fix.text(), "Seal the map boundary")
        self.assertGreater(self.window.apply_fix(), 0)
        codes = [p.code for p in validate_map(self.document, CATALOG)]
        self.assertNotIn("OBC-BOUNDARY-001", codes)

    def test_a_fix_is_one_undo(self):
        self.stroke(Tool.ERASER, [(0, 5)])
        self.window.validate()
        before = self.document.planes.planes
        row = next(i for i in range(self.window.problems.count())
                   if self.window.problems.item(i).data(Qt.UserRole + 1) == "seal_boundary")
        self.window.problems.setCurrentRow(row)
        self.window.apply_fix()
        self.window.undo()
        self.assertEqual(self.document.planes.planes, before)

    def test_the_summary_says_whether_the_floor_can_be_finished(self):
        self.legal_map(with_exit=True)
        self.window.validate()
        self.assertIn("completable", self.window.problem_status.text())
        self.assertNotIn("NOT completable", self.window.problem_status.text())

    def test_an_unreachable_exit_is_not_completable(self):
        self.legal_map()
        self.window.validate()
        self.assertIn("NOT completable", self.window.problem_status.text())

    def test_a_door_on_the_wrong_axis_can_be_turned(self):
        from obcedit_core.commands import write_words

        self.legal_map()
        # A door in the room's right-hand wall: walls above and below, so
        # it needs the vertical code (even); 131 is the horizontal one.
        self.window.run_command(write_words(self.document, [(0, 9, 4, 131), (0, 10, 4, 144)]))
        self.window.validate()
        row = next((i for i in range(self.window.problems.count())
                    if self.window.problems.item(i).data(Qt.UserRole + 1) == "door-axis"), None)
        self.assertIsNotNone(row, [self.window.problems.item(i).text()
                                   for i in range(self.window.problems.count())])
        self.window.problems.setCurrentRow(row)
        self.assertEqual(self.window.problem_fix.text(), "Turn doors to match their walls")
        self.window.apply_fix()
        self.assertEqual(self.document.cell(0, 9, 4), 130)

    def test_a_problem_with_no_fix_offers_none(self):
        self.window.validate()
        self.window.problem_filter.setCurrentIndex(
            self.window.problem_filter.findText("Everything"))
        row = next(i for i in range(self.window.problems.count())
                   if not self.window.problems.item(i).data(Qt.UserRole + 1))
        self.window.problems.setCurrentRow(row)
        self.assertFalse(self.window.problem_fix.isEnabled())


class ExportPreflight(Base):
    """File > Export mod refuses a map OBCR would refuse, before asking where."""

    def export_asking(self):
        from PySide6.QtWidgets import QFileDialog

        asked = []
        original = QFileDialog.getSaveFileName
        QFileDialog.getSaveFileName = staticmethod(
            lambda *a, **k: (asked.append(True), ("", ""))[1])
        try:
            self.window.export_mod()
        finally:
            QFileDialog.getSaveFileName = original
        return asked

    def test_an_error_blocks_the_export(self):
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])
        self.choose(WALL_ENTRY)
        self.stroke(Tool.ERASER, [(0, 5)])          # opens the map edge
        errors = []
        self.window._error = lambda title, body: errors.append(title)
        asked = self.export_asking()
        self.assertTrue(errors, "the export was not refused")
        self.assertFalse(asked, "it asked where to write a file it must not write")

    def test_a_warning_does_not(self):
        # A legal room with no exit: one warning, no errors. An unfinished map
        # is a normal thing to want to look at in the engine.
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])
        problems = validate_map(self.document, CATALOG)
        self.assertTrue(any(p.severity is Severity.WARNING for p in problems))
        self.assertFalse([p for p in problems if p.severity is Severity.ERROR],
                         [str(p) for p in problems])
        self.assertTrue(self.export_asking(), "a warning should not block an export")


class ModExport(Base):
    """Export mod and Install in OBCR, written for real and read back."""

    def setUp(self):
        super().setUp()
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])
        self.choose("special-115")
        self.stroke(Tool.BRUSH, [(8, 5)])

    def test_a_one_file_mod(self):
        from obcedit_core.modexport import audit_mod

        written = self.window.export_mod(str(self.root / "out" / "room.obcmod"))
        self.assertIsNotNone(written)
        self.assertTrue(written.is_file())
        audit = audit_mod(written, catalog=CATALOG)
        self.assertTrue(audit.ok, audit.problems)
        self.assertEqual(audit.campaign.mod_id, "untitled")

    def test_a_folder_mod(self):
        written = self.window.export_mod(str(self.root / "out" / "room"))
        self.assertTrue((written / "mod.toml").is_file())
        self.assertTrue((written / "MAPHEAD.BC").is_file())
        self.assertTrue((written / "MAPTEMP.BC").is_file())
        manifest = (written / "mod.toml").read_text(encoding="ascii")
        self.assertIn('id = "untitled"', manifest)

    def test_the_campaign_decides_what_is_written(self):
        from obcedit_core.campaign import Campaign, LevelEntry

        self.window.set_campaign(Campaign(
            mod_id="tower", title="The Tower", author="Tester",
            levels=(LevelEntry(map_uuid=self.uuid, name="Lobby", dark=True),
                    LevelEntry(stock=39))))
        self.assertTrue(self.window.project.dirty)
        written = self.window.export_mod(str(self.root / "tower"))
        manifest = (written / "mod.toml").read_text(encoding="ascii")
        self.assertIn('id = "tower"', manifest)
        self.assertIn("dark = true", manifest)
        self.assertIn("stock = 39", manifest)

    def test_an_imported_original_map_is_refused(self):
        from dataclasses import replace

        from obcedit_core.document import SourceReference

        document = replace(self.document, source=SourceReference(
            display_path="MAPTEMP.BC", sha256="0" * 64, map_number=6))
        self.window.project = self.window.project.with_map(document)
        errors = []
        self.window._error = lambda title, body: errors.append(body)
        self.assertIsNone(self.window.export_mod(str(self.root / "x.obcmod")))
        self.assertTrue(any("original game" in e for e in errors), errors)

    def test_install_writes_into_the_mods_folder(self):
        self.settings.mods_dir = self.root / "obcr-mods"
        written = self.window.install_mod()
        self.assertEqual(written, self.root / "obcr-mods" / "untitled")
        self.assertTrue((written / "mod.toml").is_file())
        # Installing again replaces it in place.
        self.assertEqual(self.window.install_mod(), written)

    def test_the_default_mods_folder_is_obcrs(self):
        from obcedit_gui.settings import obcr_mods_dir

        folder = obcr_mods_dir()
        if os.name == "nt":
            self.assertEqual(folder.parts[-3:], ("OBCR", "data", "mods"))
        else:
            self.assertEqual(folder.parts[-2:], ("obcr", "mods"))

    def test_obcr_is_the_gate(self):
        # When an engine is configured, a mod --check-mod refuses is not
        # reported as exported.
        from obcedit_core.discovery import Profile

        engine = fake_engine(self.root / "bin", """
            out("mod\tuntitled\tUntitled\t1.0\t1",
                "problem\terror\t0\t5,5\tno exit can be reached",
                "result\terrors")
            sys.exit(2)
            """)
        self.settings.profile = Profile(engine_path=str(engine),
                                        workspace_dir=str(self.root / "ws"))
        errors = []
        self.window._error = lambda title, body: errors.append(body)
        self.assertIsNone(self.window.export_mod(str(self.root / "gated.obcmod")))
        self.assertIn("no exit can be reached", " ".join(errors))
        shown = [self.window.problems.item(i).text()
                 for i in range(self.window.problems.count())]
        self.assertTrue(any("OBCR error" in line for line in shown), shown)


class InspectorPreview(Base):
    def place_soldier(self, x=4, y=4):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(x, y)])
        self.window.inspector.show_cell(self.document, x, y)

    def with_assets(self):
        self.window.thumbnails.source = StubAssets()
        self.window.thumbnails.clear()

    def test_without_game_data_it_says_so(self):
        # No copy of the game means no artwork, which is a thing to say rather
        # than an empty box or a traceback.
        self.place_soldier()
        self.assertTrue(self.window.inspector.preview.pixmap().isNull())
        self.assertEqual(self.window.inspector.preview.text(), "no artwork")

    def test_it_shows_the_artwork_larger_than_the_palette_does(self):
        self.with_assets()
        self.place_soldier()
        pixmap = self.window.inspector.preview.pixmap()
        self.assertFalse(pixmap.isNull())
        self.assertEqual(pixmap.width(), PREVIEW_SIZE)

    def place_pushwall(self, x=9, y=4):
        # A pushwall is a plane-1 marker over a plane-0 wall: one cell, two
        # things, and the heading can only name one of them.
        from obcedit_core.prefabs import by_key

        self.window.tools.set_prefab(by_key("prefab.pushwall"))
        self.stroke(Tool.PREFAB, [(x, y)])
        self.window.inspector.show_cell(self.document, x, y)

    def test_a_marker_shows_the_wall_it_sits_on(self):
        # A pushwall marker has no page of its own: what it looks like is the
        # wall it was put on, so that gets the large tile rather than a
        # placeholder standing in for artwork that does not exist.
        self.with_assets()
        self.place_pushwall()
        inspector = self.window.inspector
        self.assertEqual(inspector.preview.pixmap().width(), PREVIEW_SIZE)
        self.assertTrue(inspector.secondary.isHidden())
        self.assertIn("push wall", inspector.heading.text().lower())

    def test_a_thing_on_a_wall_keeps_both_tiles(self):
        # An enemy has its own sprite, so the wall under it stays the small one.
        self.with_assets()
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.window.inspector.show_cell(self.document, 4, 4)
        self.assertEqual(self.window.inspector.preview.pixmap().width(), PREVIEW_SIZE)
        self.assertFalse(self.window.inspector.secondary.isHidden())
        self.assertEqual(self.window.inspector.secondary.pixmap().width(), SECONDARY_SIZE)

    def test_a_plain_wall_has_only_the_one_tile(self):
        self.with_assets()
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.window.inspector.show_cell(self.document, 4, 4)
        self.assertTrue(self.window.inspector.secondary.isHidden())

    def test_deselecting_clears_it(self):
        self.with_assets()
        self.place_soldier()
        self.window.inspector.show_cell(None, -1, -1)
        self.assertTrue(self.window.inspector.preview.pixmap().isNull())
        self.assertTrue(self.window.inspector.secondary.isHidden())

    def test_nothing_selected_takes_up_no_room(self):
        # A fixed hole above the heading reads as a broken panel.
        self.window.inspector.show_cell(None, -1, -1)
        self.assertTrue(self.window.inspector.preview.isHidden())


class InspectorPanel(Base):
    def place_soldier(self, x=4, y=4):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(x, y)])
        self.window.inspector.show_cell(self.document, x, y)

    def test_it_names_what_is_there(self):
        self.place_soldier()
        self.assertIn("Soldier", self.window.inspector.heading.text())

    def test_it_shows_the_raw_words(self):
        self.place_soldier()
        self.assertIn(str(SOLDIER_EAST), self.window.inspector.raw.text())

    def test_changing_the_facing_writes_a_different_word(self):
        self.place_soldier()
        inspector = self.window.inspector
        index = inspector.direction.findText("North")
        self.assertGreaterEqual(index, 0)
        inspector.direction.setCurrentIndex(index)
        QApplication.processEvents()
        self.assertEqual(self.document.cell(1, 4, 4), SOLDIER_EAST + 1)

    def test_a_facing_change_is_undoable(self):
        self.place_soldier()
        self.window.inspector.direction.setCurrentIndex(
            self.window.inspector.direction.findText("West")
        )
        QApplication.processEvents()
        self.window.undo()
        self.assertEqual(self.document.cell(1, 4, 4), SOLDIER_EAST)

    def test_switching_to_patrolling_writes_the_patrol_band(self):
        self.place_soldier()
        inspector = self.window.inspector
        self.assertTrue(inspector.movement.isEnabled())
        inspector.movement.setCurrentIndex(1)
        QApplication.processEvents()
        value = self.document.cell(1, 4, 4)
        entry = CATALOG.for_value(1, value)
        self.assertEqual(entry.variant, "patrol")
        self.assertEqual(entry.actor, "Soldier")

    def test_the_difficulty_band_can_be_changed(self):
        self.place_soldier()
        inspector = self.window.inspector
        self.assertTrue(inspector.rank.isEnabled())
        inspector.rank.setCurrentIndex(inspector.rank.count() - 1)
        QApplication.processEvents()
        entry = CATALOG.for_value(1, self.document.cell(1, 4, 4))
        self.assertEqual(entry.minskill, 2)
        self.assertEqual(inspector.rank.currentText(), "Difficulty 3 only")
        self.assertEqual(entry.actor, "Soldier")

    def test_a_thing_with_no_facing_disables_the_control(self):
        self.choose("static-23")
        self.stroke(Tool.BRUSH, [(6, 6)])
        self.window.inspector.show_cell(self.document, 6, 6)
        self.assertFalse(self.window.inspector.direction.isEnabled())

    def test_an_actor_with_no_patrol_variant_disables_movement(self):
        # There is no patrolling soldier on a stool, so the control must not
        # promise one.
        self.choose("enemy-126")
        self.stroke(Tool.BRUSH, [(6, 6)])
        self.window.inspector.show_cell(self.document, 6, 6)
        self.assertFalse(self.window.inspector.movement.isEnabled())

    def test_plain_floor_names_its_area_and_texture_set(self):
        # Floor is not nothing: the word is the cell's area (which rooms hear
        # each other) and its floor texture set.
        self.window.inspector.show_cell(self.document, 5, 5)
        heading = self.window.inspector.heading.text()
        self.assertIn("area 4", heading)
        self.assertIn("texture set 0", heading)

    def test_victor_has_no_facing(self):
        self.choose("enemy-214")
        self.stroke(Tool.BRUSH, [(6, 6)])
        self.window.inspector.show_cell(self.document, 6, 6)
        self.assertIn("Victor", self.window.inspector.heading.text())
        self.assertFalse(self.window.inspector.direction.isEnabled())


class Validation(Base):
    def test_a_room_without_a_start_reports_it(self):
        problems = self.window.validate()
        self.assertIn("OBC-START-001", [p.code for p in problems])
        self.assertGreater(self.window.problems.count(), 0)

    def test_placing_a_start_clears_it(self):
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])
        problems = self.window.validate()
        self.assertNotIn("OBC-START-001", [p.code for p in problems])

    def test_a_problem_carries_its_cell(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.ERASER, [(0, 3)])  # a hole in the boundary
        self.window.validate()
        items = [self.window.problems.item(i) for i in range(self.window.problems.count())]
        self.assertTrue(any(item.data(Qt.UserRole) for item in items))


class Structures(Base):
    """Compound tools: one click, one structure, one undo -- or a refusal."""

    def choose_prefab(self, key: str) -> None:
        from obcedit_core.prefabs import by_key

        prefab = by_key(key)
        self.assertIsNotNone(prefab, key)
        self.window.tools.set_prefab(prefab)
        self.window.select_tool(Tool.PREFAB)

    def place(self, x: int, y: int) -> None:
        self.window._on_press(x, y, Qt.LeftButton.value)
        self.window.tools.release(x, y)

    def test_a_push_wall_marks_the_wall_it_is_put_on(self):
        self.choose_prefab("prefab.pushwall")
        self.place(9, 4)
        self.assertEqual(self.document.cell(0, 9, 4), 1)
        self.assertEqual(self.document.cell(1, 9, 4), 98)

    def test_it_is_a_single_undo(self):
        self.choose_prefab("prefab.pushwall")
        before = self.document.planes.planes
        self.place(9, 4)
        self.assertEqual(self.window.history.depth, 1)
        self.window.undo()
        self.assertEqual(self.document.planes.planes, before)

    def test_a_refused_placement_writes_nothing(self):
        # An exit needs floor beside it; deep inside the walls there is none.
        self.choose_prefab("prefab.exit.elevator")
        before = self.document.planes.planes
        self.place(30, 30)
        self.assertEqual(self.document.planes.planes, before)
        self.assertFalse(self.window.history.can_undo)

    def test_a_refusal_is_reported(self):
        self.choose_prefab("prefab.exit.elevator")
        self.place(30, 30)
        self.assertGreater(self.window.problems.count(), 0)

    def test_a_push_wall_needs_a_wall(self):
        self.choose_prefab("prefab.pushwall")
        self.place(5, 5)          # floor
        self.assertEqual(self.document.cell(1, 5, 5), EMPTY_OBJECT)

    def test_exits_and_switches_go_in_any_wall_of_the_room(self):
        for key, word, (x, y) in (("prefab.exit.ladder", 113, (0, 4)),
                                  ("prefab.exit.stairs", 114, (9, 4)),
                                  ("prefab.exit.elevator", 115, (4, 9)),
                                  ("prefab.generator.off", 96, (4, 0))):
            with self.subTest(key=key):
                self.choose_prefab(key)
                self.place(x, y)
                self.assertEqual(self.document.cell(0, x, y), word)

    def test_a_door_takes_the_axis_its_walls_give_it(self):
        # A door joins two floors. In the room's bottom wall the walls are
        # left and right of the door, so it gets the horizontal code; in the
        # right-hand wall, vertical.
        from obcedit_core.commands import write_words

        self.window.run_command(write_words(self.document, [(0, 4, 10, 144), (0, 10, 4, 144)]))
        self.choose_prefab("prefab.door.1")
        self.place(4, 9)
        self.assertEqual(self.document.cell(0, 4, 9), 131)
        self.place(9, 4)
        self.assertEqual(self.document.cell(0, 9, 4), 130)

    def test_nothing_offers_a_turn(self):
        # The toolbar button hides itself while no structure has an
        # orientation, rather than sitting there doing nothing.
        self.choose_prefab("prefab.door.1")
        self.assertFalse(self.window.action_rotate.isVisible())

    def test_the_structures_tab_lists_them(self):
        from obcedit_core.prefabs import PREFABS

        self.assertEqual(self.window.prefab_list.count(), len(PREFABS))

    def test_choosing_from_the_list_arms_the_tool(self):
        self.window.prefab_list.setCurrentRow(0)
        QApplication.processEvents()
        self.assertIsNotNone(self.window.tools.prefab)
        self.assertEqual(self.window.tools.tool, Tool.PREFAB)


class PaletteHandover(Base):
    """Choosing from one palette has to disarm whatever the last one armed.

    The bug this exists for: pick a structure, then pick a wall, then click.
    The palette showed the wall selected and the click placed the structure.
    """

    def choose_prefab(self, key: str) -> None:
        from obcedit_core.prefabs import by_key

        self.window.tools.set_prefab(by_key(key))
        self.window.select_tool(Tool.PREFAB)

    def click(self, x, y):
        self.window._on_press(x, y, Qt.LeftButton.value)
        self.window.tools.release(x, y)

    def test_choosing_a_wall_after_a_structure_paints_the_wall(self):
        self.choose_prefab("prefab.exit.elevator")
        self.choose(WALL_ENTRY)
        self.click(4, 4)
        self.assertEqual(self.document.cell(0, 4, 4), 2, "painted the wall")
        self.assertNotEqual(self.document.cell(0, 4, 4), 115,
                            "the structure was placed anyway")

    def test_the_tool_goes_back_to_the_brush(self):
        self.choose_prefab("prefab.pushwall")
        self.choose(WALL_ENTRY)
        self.assertEqual(self.window.tools.tool, Tool.BRUSH)
        self.assertIsNone(self.window.tools.prefab)

    def test_switching_tools_by_hand_drops_the_structure(self):
        self.choose_prefab("prefab.pushwall")
        self.choose(WALL_ENTRY)
        self.window.select_tool(Tool.LINE)
        self.assertIsNone(self.window.tools.prefab)
        self.window._on_press(2, 6, Qt.LeftButton.value)
        self.window.tools.release(5, 6)
        for x in range(2, 6):
            self.assertEqual(self.document.cell(0, x, 6), 2)

    def test_going_back_to_a_structure_still_works(self):
        self.choose(WALL_ENTRY)
        self.choose_prefab("prefab.pushwall")
        self.click(9, 4)
        self.assertEqual(self.document.cell(1, 9, 4), 98)

    def test_reclicking_the_current_structure_rearms_it(self):
        # Coming back from the Walls tab, the row you want is already the
        # current one, so currentItemChanged never fires. Clicking it has to
        # arm it anyway or the structure list looks broken.
        self.window.prefab_list.setCurrentRow(0)
        QApplication.processEvents()
        self.choose(WALL_ENTRY)
        self.assertEqual(self.window.tools.tool, Tool.BRUSH)

        item = self.window.prefab_list.item(0)
        self.window.prefab_list.itemClicked.emit(item)
        QApplication.processEvents()
        self.assertEqual(self.window.tools.tool, Tool.PREFAB)
        self.assertIsNotNone(self.window.tools.prefab)

    def test_the_real_palette_signal_does_the_same(self):
        # Not the helper: the actual click handler the view is wired to.
        self.choose_prefab("prefab.pushwall")
        entry = CATALOG.by_key(WALL_ENTRY)
        self.window._on_palette_clicked(type("I", (), {"data": lambda self, r: entry})())
        self.assertEqual(self.window.tools.tool, Tool.BRUSH)
        self.click(5, 5)
        self.assertEqual(self.document.cell(0, 5, 5), 2)
        self.assertEqual(self.document.cell(1, 5, 5), EMPTY_OBJECT)


class Clipboard(Base):
    """Copy, paste and the transforms, through the window."""

    def select(self, x, y, w, h):
        self.window.select_tool(Tool.POINTER)
        self.window._on_press(x, y, Qt.LeftButton.value)
        self.window.tools.drag(x + w - 1, y + h - 1, Qt.LeftButton.value)
        self.window.tools.release(x + w - 1, y + h - 1)

    def test_copy_needs_a_selection(self):
        self.assertFalse(self.window.copy_selection())

    def test_copy_then_paste(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(2, 2)])
        self.select(2, 2, 2, 2)
        self.assertTrue(self.window.copy_selection())
        self.select(5, 5, 2, 2)
        self.assertTrue(self.window.paste_clipboard())
        self.assertEqual(self.document.cell(0, 5, 5), 2)

    def test_a_paste_is_one_undo(self):
        self.choose(WALL_ENTRY)
        self.stroke(Tool.BRUSH, [(2, 2)])
        self.select(2, 2, 2, 2)
        self.window.copy_selection()
        before = self.document.planes.planes
        self.select(5, 5, 2, 2)
        depth = self.window.history.depth
        self.window.paste_clipboard()
        self.assertEqual(self.window.history.depth, depth + 1)
        self.window.undo()
        self.assertEqual(self.document.planes.planes, before)

    def test_copy_takes_all_three_planes(self):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(2, 2)])
        self.select(2, 2, 1, 1)
        self.window.copy_selection()
        self.select(6, 6, 1, 1)
        self.window.paste_clipboard()
        self.assertEqual(self.document.cell(1, 6, 6), SOLDIER_EAST)

    def test_rotating_the_clipboard_turns_a_facing(self):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(2, 2)])
        self.select(2, 2, 1, 1)
        self.window.copy_selection()
        self.assertTrue(self.window.rotate_clipboard())
        self.select(6, 6, 1, 1)
        self.window.paste_clipboard()
        entry = CATALOG.for_value(1, self.document.cell(1, 6, 6))
        self.assertEqual(entry.actor, "Soldier")
        self.assertEqual(dict(entry.directions).get("south"), self.document.cell(1, 6, 6))

    def test_flipping_the_clipboard(self):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(2, 2)])
        self.select(2, 2, 1, 1)
        self.window.copy_selection()
        self.assertTrue(self.window.flip_clipboard_h())
        self.select(6, 6, 1, 1)
        self.window.paste_clipboard()
        entry = CATALOG.for_value(1, self.document.cell(1, 6, 6))
        self.assertEqual(dict(entry.directions).get("west"), self.document.cell(1, 6, 6))

    def test_transforms_need_something_copied(self):
        self.assertFalse(self.window.rotate_clipboard())
        self.assertFalse(self.window.flip_clipboard_v())


class Statistics(Base):
    def test_it_counts_what_is_there(self):
        stats = self.window.map_statistics()
        self.assertEqual(stats["cells"], SIZE * SIZE)
        self.assertEqual(stats["floor"], 64, "the 8x8 room")
        self.assertEqual(stats["markers"], 4 * SIZE - 4, "89 round the edge")
        self.assertGreater(stats["walls"], 0)
        self.assertGreater(stats["floor"], 0)

    def test_placing_an_enemy_shows_up(self):
        before = self.window.map_statistics()["enemies"]
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(4, 4)])
        self.assertEqual(self.window.map_statistics()["enemies"], before + 1)

    def test_the_used_filter_narrows_the_palette(self):
        from obcedit_gui.main_window import PALETTE_TABS

        index = [c for _, c in PALETTE_TABS].index("enemies")
        self.window.palette_tabs.setCurrentIndex(index)
        QApplication.processEvents()
        everything = self.window.palette_models["enemies"].rowCount()
        self.window.used_only.setChecked(True)
        QApplication.processEvents()
        self.assertLess(self.window.palette_models["enemies"].rowCount(), everything)

    def test_the_filter_shows_what_the_map_does_use(self):
        self.choose(SOLDIER)
        self.stroke(Tool.BRUSH, [(4, 4)])
        from obcedit_gui.main_window import PALETTE_TABS

        index = [c for _, c in PALETTE_TABS].index("enemies")
        self.window.palette_tabs.setCurrentIndex(index)
        self.window.used_only.setChecked(True)
        QApplication.processEvents()
        model = self.window.palette_models["enemies"]
        names = [model.data(model.index(r, 0)) for r in range(model.rowCount())]
        self.assertIn("Soldier, standing", names)


class MapsList(Base):
    """The right-click actions on the maps list."""

    def second_map(self):
        return self.window.new_map(name="SECOND", slot=2)

    def test_rename(self):
        uuid = self.window.project.maps[0].uuid
        self.assertTrue(self.window.rename_map(uuid, "ATRIUM"))
        self.assertEqual(self.window.project.map_by_uuid(uuid).name, "ATRIUM")

    def test_rename_is_undoable(self):
        uuid = self.window.project.maps[0].uuid
        before = self.window.project.map_by_uuid(uuid).name
        self.window.rename_map(uuid, "ATRIUM")
        self.window.undo()
        self.assertEqual(self.window.project.map_by_uuid(uuid).name, before)

    def test_an_unencodable_rename_is_refused(self):
        from PySide6.QtWidgets import QMessageBox

        uuid = self.window.project.maps[0].uuid
        before = self.window.project.map_by_uuid(uuid).name
        original = QMessageBox.warning
        QMessageBox.warning = staticmethod(lambda *a, **k: None)
        try:
            self.assertFalse(self.window.rename_map(uuid, "x" * 40))
        finally:
            QMessageBox.warning = original
        self.assertEqual(self.window.project.map_by_uuid(uuid).name, before)

    def test_change_slot(self):
        uuid = self.window.project.maps[0].uuid
        self.assertTrue(self.window.change_slot(uuid, 7))
        self.assertEqual(self.window.project.map_by_uuid(uuid).slot, 7)
        self.assertEqual(self.window.project.map_by_uuid(uuid).label, "slot 07")

    def test_a_taken_slot_is_refused(self):
        from PySide6.QtWidgets import QMessageBox

        self.second_map()
        first = self.window.project.maps[0].uuid
        original = QMessageBox.warning
        QMessageBox.warning = staticmethod(lambda *a, **k: None)
        try:
            self.assertFalse(self.window.change_slot(first, 2))
        finally:
            QMessageBox.warning = original
        self.assertEqual(self.window.project.map_by_uuid(first).slot, 0)

    def test_slot_change_is_undoable(self):
        uuid = self.window.project.maps[0].uuid
        self.window.change_slot(uuid, 7)
        self.window.undo()
        self.assertEqual(self.window.project.map_by_uuid(uuid).slot, 0)

    def test_duplicate(self):
        uuid = self.window.project.maps[0].uuid
        self.assertTrue(self.window.duplicate_map(uuid))
        self.assertEqual(len(self.window.project), 2)
        copy = self.window.project.maps[1]
        self.assertNotEqual(copy.uuid, uuid)
        self.assertEqual(copy.planes.planes,
                         self.window.project.map_by_uuid(uuid).planes.planes)

    def test_duplicate_takes_a_free_slot(self):
        uuid = self.window.project.maps[0].uuid
        self.window.duplicate_map(uuid)
        slots = [document.slot for document in self.window.project.maps]
        self.assertEqual(len(slots), len(set(slots)))

    def test_duplicate_is_undoable(self):
        uuid = self.window.project.maps[0].uuid
        self.window.duplicate_map(uuid)
        self.window.undo()
        self.assertEqual(len(self.window.project), 1)

    def test_delete(self):
        uuid = self.window.project.maps[0].uuid
        self.assertTrue(self.window.delete_map(uuid, confirm=False))
        self.assertEqual(len(self.window.project), 0)
        self.assertEqual(self.window.map_list.count(), 0)

    def test_delete_closes_the_tab(self):
        uuid = self.window.project.maps[0].uuid
        self.window.open_map(uuid)
        self.window.delete_map(uuid, confirm=False)
        self.assertEqual(self.window.tabs.count(), 0)

    def test_delete_is_undoable(self):
        # The whole reason it is a command: an editor whose delete cannot be
        # taken back is one people are afraid to use.
        uuid = self.window.project.maps[0].uuid
        before = self.window.project.map_by_uuid(uuid).planes.planes
        self.window.delete_map(uuid, confirm=False)
        self.window.undo()
        self.assertEqual(len(self.window.project), 1)
        restored = self.window.project.map_by_uuid(uuid)
        self.assertEqual(restored.planes.planes, before)

    def test_undoing_a_delete_puts_it_back_where_it_was(self):
        self.second_map()
        first = self.window.project.maps[0].uuid
        self.window.delete_map(first, confirm=False)
        self.window.undo()
        self.assertEqual(self.window.project.maps[0].uuid, first)

    def test_redo_deletes_again(self):
        uuid = self.window.project.maps[0].uuid
        self.window.delete_map(uuid, confirm=False)
        self.window.undo()
        self.window.redo()
        self.assertEqual(len(self.window.project), 0)

    def test_the_context_menu_offers_the_actions(self):
        menu = self.window.build_map_menu(0)
        self.assertIsNotNone(menu)
        names = [action.text() for action in menu.actions() if action.text()]
        for expected in ("&Open", "&Rename…", "Change &slot…", "D&uplicate",
                         "&Check this map", "&Test in OBCR", "&Delete"):
            self.assertIn(expected, names)
        menu.deleteLater()

    def test_the_menu_acts_on_the_row_it_was_opened_on(self):
        from PySide6.QtWidgets import QMessageBox

        self.second_map()
        menu = self.window.build_map_menu(1)
        delete = next(a for a in menu.actions() if a.text() == "&Delete")
        # The real action asks before deleting, which on the offscreen platform
        # would wait forever. Answer it.
        original = QMessageBox.question
        QMessageBox.question = staticmethod(lambda *a, **k: QMessageBox.Yes)
        try:
            delete.trigger()
            QApplication.processEvents()
        finally:
            QMessageBox.question = original
        menu.deleteLater()
        # The second map went, not the first or whichever is open.
        self.assertEqual(len(self.window.project), 1)
        self.assertEqual(self.window.project.maps[0].name, "ROOM")

    def test_a_row_that_is_not_there_offers_only_a_new_map(self):
        # Below the last row there is no map to act on, and a menu that does
        # nothing is a worse answer than the one thing worth offering there.
        # Read off the menu rather than through _map_context_menu: that one
        # calls exec(), and a test that opens a modal hangs offscreen.
        menu = self.window.build_map_menu(99)
        self.assertIsNotNone(menu)
        self.assertEqual([a.text() for a in menu.actions()], ["&New map…"])
        menu.deleteLater()

    def test_an_empty_project_still_offers_a_new_map(self):
        # Where somebody is most likely to right-click: a project with nothing
        # in it yet.
        self.window.set_project(ProjectDocument.create())
        menu = self.window.build_map_menu(-1)
        self.assertIsNotNone(menu)
        self.assertEqual([a.text() for a in menu.actions()], ["&New map…"])
        menu.deleteLater()

    def test_a_row_menu_leads_with_a_new_map(self):
        menu = self.window.build_map_menu(0)
        self.assertEqual(menu.actions()[0].text(), "&New map…")
        menu.deleteLater()

    def test_the_dock_has_its_own_button(self):
        # Neither the File menu nor Ctrl+M is discoverable from the maps list,
        # which is where the question gets asked.
        self.assertEqual(self.window.new_map_button.text(), "New map…")
        before = len(self.window.project)
        self.window.new_map(name="FROM BUTTON")
        self.assertEqual(len(self.window.project), before + 1)


class Playtest(Base):
    def test_it_refuses_without_a_configured_engine(self):
        from PySide6.QtWidgets import QMessageBox

        original = QMessageBox.warning
        QMessageBox.warning = staticmethod(lambda *a, **k: None)
        try:
            self.assertFalse(self.window.playtest())
        finally:
            QMessageBox.warning = original
        self.assertTrue(any("No engine configured" in self.window.problems.item(i).text()
                            for i in range(self.window.problems.count())))


class PlaytestWithOBCR(Base):
    """F5 and Shift+F5 against a stand-in OBCR that records what it was asked."""

    def setUp(self):
        super().setUp()
        from gui_support import OBCR_CONTRACT
        from obcedit_core.discovery import Profile

        self.bin = self.root / "bin"
        self.engine = fake_engine(self.bin, OBCR_CONTRACT)
        data = make_game_data(self.root / "game")
        self.settings.profile = Profile(engine_path=str(self.engine), data_dir=str(data),
                                        workspace_dir=str(self.root / "ws"))
        self.window._error = lambda title, body: self.errors.append(f"{title}: {body}")
        self.errors = []
        self.choose(START)
        self.stroke(Tool.BRUSH, [(5, 5)])

    def wait(self):
        from PySide6.QtCore import QProcess

        if self.window.process is not None and \
                self.window.process.state() != QProcess.ProcessState.NotRunning:
            self.window.process.waitForFinished(15000)
        QApplication.processEvents()

    def launched(self) -> list[str]:
        return (self.bin / "launched.txt").read_text(encoding="utf-8").strip().split("\t")

    def test_f5_checks_the_preview_mod_then_plays_it(self):
        self.assertTrue(self.window.playtest(), self.errors)
        self.wait()
        self.assertTrue((self.bin / "checked.txt").is_file(), "--check-mod was not run")
        argv = self.launched()
        self.assertIn("--mod", argv)
        self.assertEqual(argv[argv.index("--play") + 1], "0")
        self.assertNotIn("--start", argv)
        mod = Path(argv[argv.index("--mod") + 1])
        self.assertTrue((mod / "mod.toml").is_file())
        self.assertTrue(mod.is_relative_to((self.root / "ws").resolve()),
                        "the preview goes in the workspace")
        self.assertIn("--config", argv, "a playtest never uses the player's own settings")

    def test_shift_f5_starts_at_the_cursor_facing_east(self):
        self.window._on_hover(3, 4)
        self.assertTrue(self.window.playtest_here(), self.errors)
        self.wait()
        argv = self.launched()
        self.assertEqual(argv[argv.index("--start") + 1], "3,4,1")

    def test_shift_f5_in_a_wall_is_refused(self):
        self.window._on_hover(20, 20)
        self.assertFalse(self.window.playtest_here())
        self.assertFalse((self.bin / "launched.txt").exists())

    def test_a_refusal_from_check_mod_stops_the_launch(self):
        refuse = fake_engine(self.bin, """
            out("problem\terror\t0\t-\tthe floor has no exit", "result\terrors")
            sys.exit(2)
            """, name="refusing")
        from obcedit_core.discovery import Profile

        profile = self.settings.profile
        self.settings.profile = Profile(engine_path=str(refuse), data_dir=profile.data_dir,
                                        workspace_dir=profile.workspace_dir)
        self.assertFalse(self.window.playtest())
        self.assertTrue(any("the floor has no exit" in e for e in self.errors), self.errors)
        self.assertIsNone(self.window.session)

    def test_the_test_log_shows_the_output(self):
        self.window.playtest()
        self.wait()
        shown = [self.window.test_log.item(i).text().rstrip("\r")
                 for i in range(self.window.test_log.count())]
        self.assertIn("OBCR 1.0.0-beta.2 playing", shown)


class SnapshotDock(Base):
    """E10: the camera tool and the snapshot controller, without an engine."""

    def place_camera(self, x=5, y=5):
        self.window.select_tool(Tool.CAMERA)
        self.window._on_press(x, y, Qt.LeftButton.value)
        self.window.tools.release(x, y)

    def test_the_camera_tool_writes_nothing(self):
        # A snapshot is a view of the map, not a thing in it: placing the
        # camera must not become an edit or an undo step.
        before = self.document.planes.planes
        depth = self.window.history.depth
        self.place_camera()
        self.assertEqual(self.document.planes.planes, before)
        self.assertEqual(self.window.history.depth, depth)

    def test_the_camera_lands_in_the_middle_of_the_tile(self):
        self.place_camera(5, 6)
        self.assertEqual((self.window.camera.x, self.window.camera.y), (5.5, 6.5))

    def test_turning_keeps_the_place(self):
        self.place_camera(5, 5)
        self.window.turn_camera()
        self.assertEqual(self.window.camera.angle, 90)
        self.assertEqual((self.window.camera.x, self.window.camera.y), (5.5, 5.5))

    def test_turning_with_no_camera_says_so(self):
        self.window.turn_camera()
        self.assertIn("Place the camera first", self.window.snapshot_status.text())

    def test_a_snapshot_without_a_camera_is_refused(self):
        self.assertFalse(self.window.take_snapshot())
        self.assertIn("Place the camera first", self.window.snapshot_status.text())

    def test_a_camera_in_a_wall_is_refused_before_the_engine_runs(self):
        # (0, 0) is the room's corner wall. The engine would happily draw the
        # inside of it, and the cache would keep the result.
        self.place_camera(0, 0)
        self.assertFalse(self.window.take_snapshot())
        self.assertIn("inside a wall", self.window.snapshot_status.text())

    def test_no_engine_configured_is_reported_not_crashed(self):
        self.place_camera(5, 5)
        self.assertFalse(self.window.take_snapshot())
        self.assertIn("Setup", self.window.snapshot_status.text())

    def test_a_snapshot_is_taken_at_the_camera(self):
        # A stand-in OBCR that writes a PNG with enough colours to count as a
        # world, so the whole path -- preview mod, arguments, cache -- runs.
        from obcedit_core.discovery import Profile

        bin_dir = self.root / "bin"
        engine = fake_engine(bin_dir, r"""
            import struct, zlib
            with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "shot.txt"),
                      "w", encoding="utf-8") as log:
                log.write("\t".join(argv))
            out_path = argv[argv.index("--shot-out") + 1]
            width, height = 32, 20
            rows = b"".join(b"\0" + bytes(v for x in range(width)
                                          for v in (x * 8 % 256, y * 12 % 256, (x + y) * 5 % 256))
                            for y in range(height))
            def chunk(kind, data):
                return (struct.pack(">I", len(data)) + kind + data
                        + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF))
            png = (b"\x89PNG\r\n\x1a\n"
                   + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                   + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
            with open(out_path, "wb") as image:
                image.write(png)
            """)
        data = make_game_data(self.root / "game")
        self.settings.profile = Profile(engine_path=str(engine), data_dir=str(data),
                                        workspace_dir=str(self.root / "ws"))
        self.place_camera(5, 5)
        self.window.turn_camera()
        self.assertTrue(self.window.take_snapshot(), self.window.snapshot_status.text())
        argv = (bin_dir / "shot.txt").read_text(encoding="utf-8").split("\t")
        self.assertEqual(argv[argv.index("--shot") + 1], "30")
        self.assertIn("--classic", argv)
        self.assertEqual(argv[argv.index("--start") + 1], "5,5,0")
        self.assertFalse(self.window.snapshot_view.pixmap().isNull())
        # The second one comes from the cache, without running OBCR.
        (bin_dir / "shot.txt").unlink()
        self.assertTrue(self.window.take_snapshot())
        self.assertIn("from the cache", self.window.snapshot_status.text())
        self.assertFalse((bin_dir / "shot.txt").exists())


if __name__ == "__main__":
    unittest.main(verbosity=1)
