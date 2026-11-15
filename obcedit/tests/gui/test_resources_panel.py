#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The Resources panel, arenas in the campaign, and format-2 export, offscreen.

docs/MOD_ASSETS_AND_ARENAS.md section 4. Synthetic game data (make_game_data)
and a stand-in engine; every image, sound and song is made up by the test.
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

from gui_support import OBCR_CONTRACT, fake_engine, make_game_data, obc_support

import asset_support as fx

from obcedit_core.campaign import ArenaEntry, Campaign, LevelEntry
from obcedit_core.catalog import default_catalog
from obcedit_core.discovery import Profile
from obcedit_core.document import ProjectDocument
from obcedit_core.modexport import audit_mod
from obcedit_core.project import load_project
from obcedit_core.resources import MUSIC, SOUND, SPRITE, WALL

from obcedit_gui.application import build_application
from obcedit_gui.campaign_dialog import CampaignDialog
from obcedit_gui.main_window import MainWindow
from obcedit_gui.palette_models import EntryRole
from obcedit_gui.resources_panel import REPLACED
from obcedit_gui.settings import Settings

_application = build_application([])
CATALOG = default_catalog()

ROOM = ["#######", "#S...E#", "#.....#", "#######"]
ARENA = ["#######", "#S....#", "#.....#", "#######"]

#: The editor's contract, plus a --convert-audio that writes a small QOA.
ENGINE = r'''
if "--convert-audio" in argv:
    i = argv.index("--convert-audio")
    data = b"qoaf" + (22050).to_bytes(4, "big") + bytes((1,)) + (22050).to_bytes(3, "big") + b"\0" * 64
    open(argv[i + 2], "wb").write(data)
    sys.exit(0)
''' + OBCR_CONTRACT


class Base(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.settings = Settings(QSettings(str(self.root / "settings.ini"), QSettings.IniFormat))
        self.settings.recovery_dir = self.root / "recovery"
        self.settings.mods_dir = self.root / "mods"
        data = make_game_data(self.root / "game")
        self.engine = fake_engine(self.root / "bin", ENGINE)
        self.settings.profile = Profile(engine_path=str(self.engine), data_dir=str(data),
                                        workspace_dir=str(self.root / "ws"))
        self.window = MainWindow(self.settings, catalog=CATALOG)
        self.errors: list[str] = []
        self.window._error = lambda title, body: self.errors.append(f"{title}: {body}")
        project = ProjectDocument.create("Arena Pack")
        self.floor = obc_support.room(ROOM, name="FLOOR")
        self.arena = obc_support.room(ARENA, name="PIT", slot=1)
        project = project.added(self.floor).added(self.arena)
        self.window.set_project(project)
        self.panel = self.window.resources_panel

    def tearDown(self):
        window = self.window
        window.project = window.project.marked_saved(window.project.revision)
        window.pool.cancel_all()
        window.pool.wait(2000)
        window.close()
        window.deleteLater()
        QApplication.processEvents()
        self._tmp.cleanup()

    def file(self, name: str, data: bytes) -> str:
        path = self.root / "in" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return str(path)

    def replace_wall(self, page=0, color=(250, 10, 10)) -> None:
        self.panel.select(WALL, page)
        self.assertTrue(self.panel.import_selected(self.file(f"w{page}.png", fx.art(color))),
                        self.errors)


class Panel(Base):
    def test_it_lists_everything_that_can_be_replaced(self):
        self.assertEqual(self.panel.models[WALL].rowCount(), 136)
        self.assertEqual(self.panel.models[SPRITE].rowCount(), 680)
        self.assertEqual(self.panel.sounds.count(), 99)
        self.assertIn("Weapon 1 fires", self.panel.sounds.item(45).text())
        named = [self.panel.models[WALL].index(n, 0).data(Qt.DisplayRole) for n in range(136)]
        self.assertTrue(any(":" in name and "Wall page" not in name for name in named),
                        "pages are named from the catalog")

    def test_replacing_a_wall_is_one_undoable_step(self):
        self.replace_wall(0)
        record = self.window.project.resource(WALL, 0)
        self.assertIsNotNone(record)
        self.assertEqual(self.window.history.undo_label, "Import wall 0")
        label = self.panel.models[WALL].index(0, 0).data(Qt.DisplayRole)
        self.assertTrue(label.startswith(REPLACED), label)
        self.assertEqual(self.panel.import_button.text(), "Replace…")
        self.window.undo()
        self.assertEqual(self.window.project.resources, ())
        self.assertFalse(self.panel.models[WALL].index(0, 0).data(Qt.DisplayRole)
                         .startswith(REPLACED))
        self.window.redo()
        self.assertIsNotNone(self.window.project.resource(WALL, 0))

    def test_the_palette_shows_the_replacement(self):
        entry = next(e for e in CATALOG if e.plane == 0 and e.page is not None)
        page = int(entry.page)
        before, _, _ = self.window.thumbnails.pixels_for(entry)
        self.replace_wall(page, (250, 10, 10))
        self.assertTrue(self.window.thumbnails.replaced(entry))
        after, edge, alpha = self.window.thumbnails.pixels_for(entry)
        self.assertEqual((edge, alpha), (64, False))
        self.assertNotEqual(before, after)
        self.assertEqual(len(set(zip(after[0::3], after[1::3], after[2::3]))), 1,
                         "one flat colour, from the game palette")

    def test_sprites_keep_transparency(self):
        self.panel.select(SPRITE, 3)
        self.assertTrue(self.panel.import_selected(self.file("s.png", fx.art(hole=True))))
        entry = next(self.panel.models[SPRITE].index(3, 0).data(EntryRole) for _ in [0])
        pixels, _, alpha = self.window.thumbnails.pixels_for(entry)
        self.assertTrue(alpha)
        self.assertEqual(pixels[(20 * 64 + 20) * 4 + 3], 0)

    def test_revert(self):
        self.replace_wall(5)
        self.panel.select(WALL, 5)
        self.assertTrue(self.panel.revert_selected())
        self.assertIsNone(self.window.project.resource(WALL, 5))
        self.assertEqual(self.window.history.undo_label, "Revert wall 5")

    def test_a_bad_file_is_refused_with_a_reason(self):
        self.panel.select(WALL, 1)
        self.assertFalse(self.panel.import_selected(self.file("big.png", fx.art(edge=32))))
        self.assertIn("64x64", self.errors[-1])
        self.panel.select(SOUND, 7)
        self.assertFalse(self.panel.import_selected(self.file("x.wav", b"junk")))
        self.assertEqual(self.window.project.resources, ())

    def test_sounds(self):
        self.panel.select(SOUND, 7)
        self.assertTrue(self.panel.import_selected(self.file("bang.wav", fx.wav())))
        self.assertIn("bang.wav", self.panel.sounds.item(7).text())

    def test_music_import_rename_and_remove(self):
        self.assertTrue(self.panel.import_music(self.file("Theme.ogg", fx.ogg())))
        self.window.set_campaign(Campaign(
            mod_id="arena-pack", title="Arena Pack", version="1.0",
            levels=(LevelEntry(map_uuid=self.floor.uuid, music="Theme.ogg"),)))
        self.panel.select(MUSIC, "Theme.ogg")
        self.assertIn("played on floor 1", self.panel.music.currentItem().text())
        self.assertTrue(self.panel.rename_selected("anthem"))
        self.assertIsNotNone(self.window.project.resource(MUSIC, "anthem.ogg"))
        self.assertEqual(self.window.current_campaign().levels[0].music, "anthem.ogg",
                         "the floor follows the rename")
        self.window.undo()
        self.assertEqual(self.window.current_campaign().levels[0].music, "Theme.ogg")
        self.assertIsNotNone(self.window.project.resource(MUSIC, "Theme.ogg"))
        self.assertFalse(self.panel.rename_selected("bad name!"))

    def test_wav_music_is_converted_by_obcr(self):
        self.assertTrue(self.panel.import_music(self.file("loop.wav", fx.wav(seconds=1))),
                        self.errors)
        record = self.window.project.resource(MUSIC, "loop.qoa")
        self.assertIsNotNone(record)

    def test_wav_music_without_an_engine_says_why(self):
        self.settings.profile = Profile(engine_path="", data_dir=self.settings.profile.data_dir,
                                        workspace_dir=self.settings.profile.workspace_dir)
        self.assertFalse(self.panel.import_music(self.file("loop.wav", fx.wav())))
        self.assertIn("--convert-audio", self.errors[-1])

    def test_save_a_copy_of_your_own_file(self):
        self.replace_wall(2)
        self.panel.select(WALL, 2)
        target = self.root / "out" / "mine.png"
        self.assertTrue(self.panel.save_copy_selected(str(target)))
        self.assertEqual(target.read_bytes(), fx.art((250, 10, 10)))
        self.panel.select(WALL, 3)
        self.assertFalse(self.panel.save_copy_selected(str(self.root / "game.png")),
                         "the game's own page is never written out")


class Storage(Base):
    def test_files_move_beside_the_project_on_first_save(self):
        self.replace_wall(0)
        unsaved = self.window.resource_store()
        self.assertIn("unsaved", str(unsaved.root))
        record = self.window.project.resource(WALL, 0)
        self.assertTrue(unsaved.path_of(record).is_file())
        path = self.root / "proj" / "pack.obcproject"
        self.window.project_path = path
        self.assertTrue(self.window.save_project())
        saved = self.window.resource_store()
        self.assertEqual(saved.root, (self.root / "proj" / "pack.assets").resolve())
        self.assertEqual(saved.read(record), fx.art((250, 10, 10)))
        self.assertFalse(unsaved.root.exists(), "the workspace copy is gone")
        self.assertEqual(load_project(path).resources, (record,))

    def test_save_as_keeps_undo_history_files(self):
        self.replace_wall(0, (1, 2, 3))
        self.replace_wall(0, (200, 200, 0))
        first = self.root / "a.obcproject"
        self.window.project_path = first
        self.window.save_project()
        # What Save As does: a new destination, with no identity to compare.
        self.window.project_path = self.root / "b.obcproject"
        self.window._identity = None
        self.assertTrue(self.window.save_project())
        self.window.undo()
        record = self.window.project.resource(WALL, 0)
        self.assertEqual(self.window.resource_store().read(record), fx.art((1, 2, 3)))

    def test_a_changed_file_is_reported_on_open(self):
        self.replace_wall(0)
        path = self.root / "pack.obcproject"
        self.window.project_path = path
        self.window.save_project()
        record = self.window.project.resource(WALL, 0)
        self.window.resource_store().path_of(record).write_bytes(b"edited elsewhere")
        self.window.open_project(str(path))
        notes = [self.window.problems.item(i).text() for i in range(self.window.problems.count())]
        self.assertTrue(any("changed outside OBCEdit" in note for note in notes), notes)


class Export(Base):
    def test_install_carries_assets_and_arenas(self):
        self.replace_wall(0)
        self.panel.select(SOUND, 3)
        self.panel.import_selected(self.file("bang.wav", fx.wav()))
        self.panel.import_music(self.file("theme.ogg", fx.ogg()))
        self.window.set_campaign(Campaign(
            mod_id="arena-pack", title="Arena Pack", version="1.0",
            levels=(LevelEntry(map_uuid=self.floor.uuid, music="theme.ogg"),),
            arenas=(ArenaEntry(map_uuid=self.arena.uuid, name="Pit"),)))
        written = self.window.install_mod()
        self.assertIsNotNone(written, self.errors)
        self.assertTrue((written / "walls" / "000.png").is_file())
        self.assertTrue((written / "sounds" / "03.wav").is_file())
        self.assertTrue((written / "music" / "theme.ogg").is_file())
        audit = audit_mod(written, catalog=CATALOG)
        self.assertTrue(audit.ok, [str(p) for p in audit.problems])
        self.assertEqual(audit.manifest["format"], 2)
        self.assertEqual(len(audit.campaign.arenas), 1)

    def test_a_missing_music_file_blocks_export(self):
        self.window.set_campaign(Campaign(
            mod_id="arena-pack", title="Arena Pack", version="1.0",
            levels=(LevelEntry(map_uuid=self.floor.uuid, music="gone.ogg"),)))
        self.assertIsNone(self.window.install_mod())
        self.assertIn("gone.ogg", self.errors[-1])

    def test_arena_maps_are_checked_as_arenas(self):
        enemy = obc_support.room(["#######", "#S..e.#", "#######"], name="BAD", slot=2)
        self.window.set_project(self.window.project.added(enemy))
        self.window.set_campaign(Campaign(
            mod_id="arena-pack", title="Arena Pack", version="1.0",
            levels=(LevelEntry(map_uuid=self.floor.uuid),),
            arenas=(ArenaEntry(map_uuid=enemy.uuid),)))
        self.window.open_map(enemy.uuid)
        codes = {p.code for p in self.window.validate()}
        self.assertIn("OBC-ARENA-001", codes)
        self.assertEqual(self.window._map_profile(enemy).key, "arena")
        self.assertEqual(self.window._map_profile(self.floor).key, "single_player")


class Arenas(Base):
    def dialog(self, campaign=None, music=("theme.ogg",)):
        campaign = campaign or Campaign(mod_id="arena-pack", title="Arena Pack", version="1.0",
                                        levels=(LevelEntry(map_uuid=self.floor.uuid),))
        return CampaignDialog(campaign, self.window.project.maps, music_names=music)

    def test_add_an_arena_with_file_music(self):
        dialog = self.dialog()
        try:
            dialog.add_arena.click()
            table = dialog.arena_table
            self.assertEqual(table.rowCount(), 1)
            table.cellWidget(0, 0).setCurrentIndex(table.cellWidget(0, 0).findData(
                self.arena.uuid))
            music = table.cellWidget(0, 2)
            music.setCurrentIndex(music.findData("theme.ogg"))
            table.cellWidget(0, 3).setValue(55)
            campaign = dialog.campaign()
            self.assertEqual(campaign.arenas, (ArenaEntry(map_uuid=self.arena.uuid, name="PIT",
                                                          music="theme.ogg", colors=55),))
            self.assertTrue(dialog.ok.isEnabled(), dialog.problems.text())
            self.assertIn("(1)", dialog.tabs.tabText(1))
        finally:
            dialog.deleteLater()

    def test_floor_music_by_file_and_a_missing_one(self):
        campaign = Campaign(mod_id="arena-pack", title="Arena Pack", version="1.0",
                            levels=(LevelEntry(map_uuid=self.floor.uuid, music="gone.ogg"),))
        dialog = self.dialog(campaign)
        try:
            combo = dialog.table.cellWidget(0, 3)
            self.assertEqual(combo.currentText(), "(missing) gone.ogg")
            self.assertFalse(dialog.ok.isEnabled())
            combo.setCurrentIndex(combo.findData("theme.ogg"))
            self.assertTrue(dialog.ok.isEnabled(), dialog.problems.text())
            self.assertEqual(dialog.campaign().levels[0].music, "theme.ogg")
        finally:
            dialog.deleteLater()

    def test_round_trip_through_the_dialog(self):
        campaign = Campaign(mod_id="arena-pack", title="Arena Pack", version="1.0",
                            levels=(LevelEntry(map_uuid=self.floor.uuid),),
                            arenas=(ArenaEntry(map_uuid=self.arena.uuid, name="Pit", music=4,
                                               dark=True),))
        dialog = self.dialog(campaign)
        try:
            self.assertEqual(dialog.campaign(), campaign)
            dialog.arena_table.setCurrentCell(0, 0)
            dialog.remove_arena.click()
            self.assertEqual(dialog.campaign().arenas, ())
        finally:
            dialog.deleteLater()


if __name__ == "__main__":
    unittest.main()
