#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The campaign dialog: a mod's identity and its floors, and the export behind it.

Driven on the offscreen platform against real widgets. The dialog's whole job
is to be a view over `campaign.validate` -- so what is tested is that the view
and the validator cannot disagree: what the table says round-trips, OK follows
the errors, and the mod written from it says what the table said.

Nothing here needs Operation Body Count. Every map is drawn by the test.
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

from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QApplication

from gui_support import obc_support

from obcedit_core.campaign import Campaign, LevelEntry
from obcedit_core.catalog import default_catalog
from obcedit_core.document import ProjectDocument, SourceReference
from obcedit_core.modexport import audit_mod

from obcedit_gui.application import build_application
from obcedit_gui.campaign_dialog import CampaignDialog, original_floor_name
from obcedit_gui.main_window import MainWindow
from obcedit_gui.settings import Settings

_application = build_application([])
CATALOG = default_catalog()

ROOM = ["#######",
        "#S...E#",
        "#.....#",
        "#######"]


def a_project(count=2) -> ProjectDocument:
    project = ProjectDocument.create("Tower Pack")
    for slot in range(count):
        document = obc_support.room(ROOM, name=f"FLOOR {slot + 1}", slot=slot)
        project = project.added(document)
    return project


class Base(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        backend = QSettings(str(self.root / "settings.ini"), QSettings.IniFormat)
        self.settings = Settings(backend)
        self.settings.recovery_dir = self.root / "recovery"
        self.window = MainWindow(self.settings, catalog=CATALOG)
        self.window.set_project(a_project())

    def tearDown(self):
        self.window.project = self.window.project.marked_saved(
            self.window.project.revision)
        self.window.pool.cancel_all()
        self.window.pool.wait(2000)
        self.window.close()
        self.window.deleteLater()
        QApplication.processEvents()
        self._tmp.cleanup()

    @property
    def maps(self):
        return self.window.project.maps


class Dialog(Base):
    def open(self, campaign=None) -> CampaignDialog:
        campaign = campaign or Campaign.default_for(self.window.project)
        dialog = CampaignDialog(campaign, self.maps)
        self.addCleanup(dialog.deleteLater)
        return dialog

    def cell(self, dialog, row, column):
        return dialog.table.cellWidget(row, column)

    def set_combo(self, dialog, row, column, value):
        combo = self.cell(dialog, row, column)
        index = combo.findData(value)
        self.assertGreaterEqual(index, 0, (column, value))
        combo.setCurrentIndex(index)

    def test_the_default_is_every_map_in_order(self):
        dialog = self.open()
        self.assertEqual(dialog.table.rowCount(), 2)
        campaign = dialog.campaign()
        self.assertEqual([level.map_uuid for level in campaign.levels],
                         [d.uuid for d in self.maps])
        self.assertEqual(campaign.mod_id, "tower-pack")
        self.assertEqual(campaign.title, "Tower Pack")
        self.assertTrue(dialog.ok.isEnabled(), dialog.problems.text())
        self.assertEqual(dialog.problems.text(), "This mod is ready to build.")

    def test_the_columns_are_the_mod_schema(self):
        dialog = self.open()
        headers = [dialog.table.horizontalHeaderItem(i).text()
                   for i in range(dialog.table.columnCount())]
        self.assertEqual(headers, ["Floor", "Name", "Like", "Music", "Dark", "Sewer",
                                   "Colours", "Sequence", "Message", "Exit"])

    def test_nothing_of_corridor_7_is_left(self):
        dialog = self.open()
        text = " ".join(child.text() for child in dialog.findChildren(type(dialog.ok))
                        if child.text())
        for gone in ("MAP0", "MAPINFO", "WAD", "Route", "cinematic"):
            self.assertNotIn(gone, text)

    def test_every_field_round_trips(self):
        dialog = self.open()
        dialog.mod_id.setText("tower")
        dialog.title.setText("The Tower")
        dialog.author.setText("Tester")
        dialog.version.setText("2.1")
        dialog.description.setPlainText("Two floors\nof office.")
        dialog.release.setCurrentIndex(dialog.release.findData("OBC-CD"))
        self.cell(dialog, 0, 1).setText("Lobby")
        self.set_combo(dialog, 0, 2, 22)                 # like the top floor
        self.set_combo(dialog, 0, 3, "random")
        self.set_combo(dialog, 0, 4, True)
        self.set_combo(dialog, 0, 5, False)
        self.cell(dialog, 0, 6).setValue(12)
        self.set_combo(dialog, 0, 7, "F3")
        self.cell(dialog, 0, 8).setText("Get to the roof")
        self.set_combo(dialog, 1, 9, "none")
        self.set_combo(dialog, 1, 3, 4)

        campaign = dialog.campaign()
        self.assertEqual((campaign.mod_id, campaign.title, campaign.author, campaign.version,
                          campaign.release),
                         ("tower", "The Tower", "Tester", "2.1", "OBC-CD"))
        self.assertEqual(campaign.description, "Two floors of office.")
        first, second = campaign.levels
        self.assertEqual(first, LevelEntry(map_uuid=self.maps[0].uuid, name="Lobby", like=22,
                                           music="random", dark=True, sewer=False, colors=12,
                                           sequence="F3", message="Get to the roof"))
        self.assertEqual(second.exit, "none")
        self.assertEqual(second.music, 4)
        self.assertTrue(dialog.ok.isEnabled(), dialog.problems.text())

        # And back: a dialog opened on that campaign shows the same thing.
        again = self.open(campaign)
        self.assertEqual(again.campaign(), campaign)

    def test_unset_fields_stay_out_of_the_manifest(self):
        campaign = self.open().campaign()
        level = campaign.levels[0]
        self.assertEqual((level.like, level.music, level.dark, level.sewer, level.colors,
                          level.sequence, level.message, level.exit),
                         (None,) * 8)

    def test_a_bad_id_disables_ok_and_says_why(self):
        dialog = self.open()
        dialog.mod_id.setText("Not Valid!")
        self.assertFalse(dialog.ok.isEnabled())
        self.assertIn("mod id", dialog.problems.text())
        dialog.mod_id.setText("valid-id")
        self.assertTrue(dialog.ok.isEnabled())

    def test_an_empty_title_is_an_error(self):
        dialog = self.open()
        dialog.title.setText("")
        self.assertFalse(dialog.ok.isEnabled())
        self.assertIn("title", dialog.problems.text())

    def test_no_floors_is_an_error(self):
        dialog = self.open()
        dialog.table.setCurrentCell(0, 0)
        dialog.remove.click()
        dialog.table.setCurrentCell(0, 0)
        dialog.remove.click()
        self.assertEqual(dialog.table.rowCount(), 0)
        self.assertFalse(dialog.ok.isEnabled())
        self.assertIn("no floors", dialog.problems.text())

    def test_an_original_floor_can_be_added(self):
        dialog = self.open()
        dialog.add_stock.click()
        self.set_combo(dialog, 2, 0, "stock:39")
        level = dialog.campaign().levels[2]
        self.assertEqual((level.map_uuid, level.stock), ("", 39))
        self.assertTrue(dialog.ok.isEnabled(), dialog.problems.text())

    def test_original_floors_are_named(self):
        self.assertEqual(original_floor_name(0), "Sewers 1")
        self.assertEqual(original_floor_name(39), "UN Bldg 2 top (Victor)")
        dialog = self.open()
        combo = self.cell(dialog, 0, 0)
        labels = [combo.itemText(i) for i in range(combo.count())]
        self.assertIn("Original floor 40: UN Bldg 2 top (Victor)", labels)

    def test_moving_a_floor_reorders_the_campaign(self):
        dialog = self.open()
        before = [level.map_uuid for level in dialog.campaign().levels]
        dialog.table.setCurrentCell(1, 0)
        dialog.up.click()
        after = [level.map_uuid for level in dialog.campaign().levels]
        self.assertEqual(after, list(reversed(before)))
        self.assertEqual(dialog.table.currentRow(), 0)

    def test_a_too_long_message_is_cut_at_the_limit(self):
        from obcedit_core.campaign import MAX_MESSAGE

        dialog = self.open()
        self.cell(dialog, 0, 8).setText("x" * 100)
        self.assertEqual(len(dialog.campaign().levels[0].message), MAX_MESSAGE)

    def test_an_imported_map_cannot_be_shared(self):
        from dataclasses import replace

        document = replace(self.maps[0], source=SourceReference(
            display_path="MAPTEMP.BC", sha256="a" * 64, map_number=6))
        self.window.project = self.window.project.with_map(document)
        dialog = self.open()
        self.assertFalse(dialog.ok.isEnabled())
        self.assertIn("original game", dialog.problems.text())

    def test_a_missing_map_is_kept_and_reported(self):
        campaign = Campaign(mod_id="x", title="X",
                            levels=(LevelEntry(map_uuid="gone-uuid"),))
        dialog = self.open(campaign)
        self.assertEqual(dialog.campaign().levels[0].map_uuid, "gone-uuid")
        self.assertFalse(dialog.ok.isEnabled())
        self.assertIn("not in the project", dialog.problems.text())


class Window(Base):
    """File > Campaign, through the window."""

    def run_dialog(self, edit):
        from PySide6.QtWidgets import QDialog

        from obcedit_gui import campaign_dialog

        original = campaign_dialog.CampaignDialog.exec

        def fake_exec(dialog):
            edit(dialog)
            return QDialog.DialogCode.Accepted

        campaign_dialog.CampaignDialog.exec = fake_exec
        try:
            return self.window.edit_campaign()
        finally:
            campaign_dialog.CampaignDialog.exec = original

    def test_accepting_stores_the_campaign_and_dirties_the_project(self):
        self.window.project = self.window.project.marked_saved(self.window.project.revision)
        self.assertTrue(self.run_dialog(lambda dialog: dialog.title.setText("Renamed")))
        self.assertEqual(self.window.project.campaign["title"], "Renamed")
        self.assertTrue(self.window.project.dirty)
        self.assertIn("•", self.window.windowTitle())

    def test_the_stored_campaign_is_what_the_mod_says(self):
        def edit(dialog):
            dialog.mod_id.setText("tower")
            dialog.table.cellWidget(1, 4).setCurrentIndex(
                dialog.table.cellWidget(1, 4).findData(True))

        self.run_dialog(edit)
        written = self.window.export_mod(str(self.root / "tower.obcmod"))
        self.assertIsNotNone(written)
        audit = audit_mod(written, catalog=CATALOG)
        self.assertTrue(audit.ok, audit.problems)
        self.assertEqual(audit.campaign.mod_id, "tower")
        self.assertEqual([level.dark for level in audit.campaign.levels], [None, True])

    def test_the_campaign_survives_save_and_open(self):
        self.run_dialog(lambda dialog: dialog.author.setText("Saved Author"))
        path = self.root / "pack.obcproject"
        self.window.project_path = path
        self.assertTrue(self.window.save_project())
        self.window.open_project(str(path))
        self.assertEqual(self.window.current_campaign().author, "Saved Author")


if __name__ == "__main__":
    unittest.main(verbosity=1)
