#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Campaigns, mod.toml, and mod export / audit (docs/MODS_AND_OBCEDIT.md section 3)."""

from __future__ import annotations

import tempfile
import tomllib
import unittest
import zipfile
from dataclasses import replace
from pathlib import Path

from obc_support import CATALOG, engine, real_data, room

from obcedit_core.campaign import Campaign, LevelEntry, render_manifest, toml_string, validate
from obcedit_core.document import ProjectDocument, SourceReference
from obcedit_core.errors import ExportError
from obcedit_core.modexport import audit_mod, build_mod, preview_mod, write_mod
from obcedit_core.paths import OutputGuard

GOOD = ["#######", "#S.ee.E", "#.....#", "#######"]


def project(*maps) -> ProjectDocument:
    result = ProjectDocument.create("Lobby Assault", author="Jane Doe")
    for document in maps:
        result = result.added(document)
    return result


def two_maps():
    first = room(GOOD, name="LOBBY")
    second = room(GOOD, name="ROOF")
    proj = project(first, second)
    campaign = Campaign(
        mod_id="lobby-assault", title="Lobby Assault", author="Jane Doe", version="1.0",
        description="Two floors.", levels=(
            LevelEntry(map_uuid=first.uuid, name="Lobby", like=6),
            LevelEntry(stock=7),
            LevelEntry(map_uuid=second.uuid, like=2, music=4, dark=True, sequence="F4",
                       message="ROOF"),
        ))
    return proj, campaign


class Schema(unittest.TestCase):
    def test_a_good_campaign_has_no_problems(self):
        proj, campaign = two_maps()
        self.assertEqual(validate(campaign, proj.maps), [])

    def test_limits(self):
        proj, campaign = two_maps()
        bad = replace(campaign, mod_id="Lobby Assault!", title="x" * 40, release="OBC-Z",
                      levels=(LevelEntry(stock=40), LevelEntry(map_uuid="missing"),
                              LevelEntry(stock=1, music=12, sequence="F9", exit="away"),
                              LevelEntry()))
        found = {p.code for p in validate(bad, proj.maps)}
        self.assertTrue({"OBC-MOD-001", "OBC-MOD-002", "OBC-MOD-003", "OBC-MOD-005",
                         "OBC-MOD-006", "OBC-MOD-007"} <= found, found)

    def test_original_maps_cannot_be_shared(self):
        proj, campaign = two_maps()
        imported = replace(proj.maps[0], source=SourceReference(sha256="abc", map_number=6))
        proj = proj.with_map(imported)
        self.assertIn("OBC-MOD-009", {p.code for p in validate(campaign, proj.maps)})
        self.assertEqual(validate(campaign, proj.maps, sharing=False), [])

    def test_json_round_trip(self):
        _, campaign = two_maps()
        self.assertEqual(Campaign.from_json(campaign.to_json()), campaign)

    def test_manifest(self):
        _, campaign = two_maps()
        data = tomllib.loads(render_manifest(campaign))
        self.assertEqual(data["format"], 1)
        self.assertEqual(data["mod"]["id"], "lobby-assault")
        self.assertEqual([lv.get("map", lv.get("stock")) for lv in data["level"]], [0, 7, 1])
        self.assertEqual(data["level"][2]["music"], 4)
        self.assertIs(data["level"][2]["dark"], True)
        self.assertEqual(data["level"][1], {"stock": 7})

    def test_strings_are_escaped(self):
        self.assertEqual(tomllib.loads("a = " + toml_string('say "hi" \\ ok'))["a"],
                         'say "hi" \\ ok')

    def test_default_campaign(self):
        proj, _ = two_maps()
        default = Campaign.default_for(proj)
        self.assertEqual(default.mod_id, "lobby-assault")
        self.assertEqual(len(default.levels), 2)


class Export(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def test_folder_and_zip_carry_the_same_files(self):
        proj, campaign = two_maps()
        build = build_mod(proj, replace(campaign, readme="Have fun.\r\n"))
        self.assertEqual(sorted(build.files), ["MAPHEAD.BC", "MAPTEMP.BC", "README.txt", "mod.toml"])
        folder = write_mod(build, self.root / "lobby")
        packed = write_mod(build, self.root / "lobby.obcmod")
        with zipfile.ZipFile(packed) as archive:
            self.assertEqual(sorted(archive.namelist()), sorted(build.files))
            for name in build.files:
                self.assertEqual(archive.read(name), (folder / name).read_bytes())
        for path in (folder, packed):
            audit = audit_mod(path, catalog=CATALOG)
            self.assertTrue(audit.ok, audit.problems)
            self.assertEqual(audit.maps, {0: "LOBBY", 1: "ROOF"})
            self.assertEqual(len(audit.campaign.levels), 3)

    def test_the_build_is_deterministic(self):
        proj, campaign = two_maps()
        self.assertEqual(build_mod(proj, campaign).digest, build_mod(proj, campaign).digest)

    def test_an_invalid_campaign_is_not_built(self):
        proj, campaign = two_maps()
        with self.assertRaises(ExportError):
            build_mod(proj, replace(campaign, mod_id=""))

    def test_a_stock_only_mod_needs_no_maps(self):
        proj, _ = two_maps()
        build = build_mod(proj, Campaign(mod_id="remix", title="Remix",
                                         levels=(LevelEntry(stock=3), LevelEntry(stock=1))))
        self.assertEqual(sorted(build.files), ["mod.toml"])
        self.assertTrue(audit_mod(write_mod(build, self.root / "remix"), catalog=CATALOG).ok)

    def test_preview_is_one_private_floor(self):
        proj, campaign = two_maps()
        imported = replace(proj.maps[1], source=SourceReference(sha256="abc"))
        proj = proj.with_map(imported).with_campaign(campaign.to_json())
        build = preview_mod(proj, imported.uuid)
        self.assertEqual(len(build.campaign.levels), 1)
        self.assertEqual(build.campaign.levels[0].like, 2)  # its level settings came along

    def test_the_guard_keeps_exports_out_of_game_data(self):
        game = self.root / "game"
        game.mkdir()
        (game / "GFXTILES.BC").write_bytes(b"x")
        proj, campaign = two_maps()
        guard = OutputGuard(protected_roots=(game,))
        with self.assertRaises(ExportError):
            write_mod(build_mod(proj, campaign), game / "mine.obcmod", guard=guard)

    def test_the_guard_matches_a_root_however_it_is_spelled(self):
        # A root given in another spelling of the same folder (here "..";
        # on Windows also an 8.3 short name or a junction) is still protected.
        game = self.root / "game"
        (game / "sub").mkdir(parents=True)
        (game / "GFXTILES.BC").write_bytes(b"x")
        proj, campaign = two_maps()
        guard = OutputGuard(protected_roots=(game / "sub" / "..",))
        with self.assertRaises(ExportError):
            write_mod(build_mod(proj, campaign), game / "mine.obcmod", guard=guard)

    def test_a_stale_readme_is_removed_from_a_folder(self):
        proj, campaign = two_maps()
        folder = write_mod(build_mod(proj, replace(campaign, readme="old")), self.root / "m")
        write_mod(build_mod(proj, campaign), folder)
        self.assertFalse((folder / "README.txt").exists())


class Hostile(unittest.TestCase):
    """The audit applies OBCR's limits to a mod somebody sent."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def zip_with(self, entries: dict[str, bytes]) -> Path:
        path = self.root / "hostile.obcmod"
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
            for name, data in entries.items():
                archive.writestr(name, data)
        return path

    def test_path_traversal_is_refused(self):
        path = self.zip_with({"../evil.txt": b"x", "mod.toml": b"format = 1"})
        self.assertIn("OBC-MOD-012", {p.code for p in audit_mod(path).problems})

    def test_unknown_keys_and_formats(self):
        manifest = b'format = 2\nextra = 1\n[mod]\nid = "x"\ntitle = "X"\nversion = "1"\nwho = 1\n' \
                   b'[[level]]\nstock = 1\nscript = "rm"\n'
        found = {p.code for p in audit_mod(self.zip_with({"mod.toml": manifest})).problems}
        self.assertIn("OBC-MOD-011", found)

    def test_not_a_zip(self):
        path = self.root / "broken.obcmod"
        path.write_bytes(b"not a zip at all")
        self.assertFalse(audit_mod(path).ok)

    def test_a_missing_map_slot(self):
        manifest = b'format = 1\n[mod]\nid = "x"\ntitle = "X"\nversion = "1"\n[[level]]\nmap = 3\n'
        self.assertIn("OBC-MOD-014", {p.code for p in audit_mod(self.zip_with({"mod.toml": manifest})).problems})


class Engine(unittest.TestCase):
    """Engine-gated: OBCR's own --check-mod accepts what the editor writes."""

    def test_obcr_accepts_the_export(self):
        binary = engine()
        if binary is None:
            self.skipTest("no OBCR build in target/")
        from obcedit_core.engine_runner import check_mod

        proj, campaign = two_maps()
        with tempfile.TemporaryDirectory() as tmp:
            for name in ("mod", "mod.obcmod"):
                path = write_mod(build_mod(proj, campaign), Path(tmp) / name)
                data = real_data("full")
                result = check_mod(binary, path, data_dir=data)
                self.assertTrue(result.ok, result.output)
                self.assertEqual((result.mod_id, result.levels), ("lobby-assault", 3))


if __name__ == "__main__":
    unittest.main()
