#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Graphics decoders on synthetic data, and the command line end to end."""

from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

from obc_support import CATALOG, make_fixtures, real_data, write_archive

from obcedit_core.assets import (
    AssetError,
    ImageCache,
    average_color,
    load_palette,
    palette_offset,
    parse_gfx_header,
    sprite_rgba,
    wall_rgb,
)
from obcedit_core.cli import EXIT_ERROR, EXIT_OK, main


class Assets(unittest.TestCase):
    def setUp(self):
        fixtures = make_fixtures.fixture_set()
        self.gfx = fixtures["synthetic.GFXTILES.BC"]
        self.palette = load_palette(fixtures["synthetic.COUNT.EXE"])

    def test_palette_by_release(self):
        self.assertEqual(palette_offset(b"\0" * 235440), 0x2DDB0)
        self.assertEqual(palette_offset(b"", "BCCD.EXE"), 0x2E450)
        self.assertEqual(len(self.palette), 768)
        with self.assertRaises(AssetError):
            palette_offset(b"\0" * 100)
        cd = make_fixtures.build_palette_executable("BCCD.EXE")
        self.assertEqual(load_palette(cd), self.palette)

    def test_walls_and_sprites(self):
        header = parse_gfx_header(self.gfx)
        self.assertEqual((header.sprite_start, header.sound_start), (2, 4))
        wall = wall_rgb(header.chunk(self.gfx, 0), self.palette)
        self.assertEqual(len(wall), 64 * 64 * 3)
        sprite = sprite_rgba(header.chunk(self.gfx, header.sprite_start), self.palette)
        opaque = sum(1 for alpha in sprite[3::4] if alpha)
        self.assertEqual(opaque, 24 * 32)
        self.assertEqual(len(average_color(wall)), 3)

    def test_a_lying_sprite_is_refused(self):
        with self.assertRaises(AssetError):
            sprite_rgba(b"\x05\x00\x02\x00", self.palette)
        with self.assertRaises(AssetError):
            parse_gfx_header(b"\x01\x00")

    def test_cache_budget(self):
        cache = ImageCache(budget_bytes=10)
        cache.put("a", b"12345")
        cache.put("b", b"12345")
        cache.put("c", b"12345")
        self.assertIsNone(cache.get("a"))
        self.assertEqual(cache.fetch("d", lambda: b"x"), b"x")

    def test_real_sprites_decode(self):
        data = real_data("full")
        if data is None:
            self.skipTest("no game data")
        gfx = (data / "GFXTILES.BC").read_bytes()
        palette = load_palette((data / "COUNT.EXE").read_bytes())
        header = parse_gfx_header(gfx)
        for entry in CATALOG:
            if entry.sprite is not None:
                sprite_rgba(header.chunk(gfx, header.sprite_start + entry.sprite), palette)
            if entry.page is not None:
                wall_rgb(header.chunk(gfx, entry.page), palette)


def run(*argv) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
        code = main([str(a) for a in argv])
    return code, out.getvalue()


class CommandLine(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.archive = write_archive(self.root / "archive")

    def tearDown(self):
        self._tmp.cleanup()

    def test_inspect_and_verify(self):
        code, text = run("inspect", self.archive)
        self.assertEqual(code, EXIT_OK)
        self.assertIn("slot 05", text)
        code, text = run("verify-archive", self.archive)
        self.assertIn("maps", text)
        code, text = run("catalog-info")
        self.assertIn("enemies", text)

    def test_project_to_mod_and_back(self):
        project = self.root / "p.obcproject"
        self.assertEqual(run("project-new", project, "--name", "CLI Mod", "--room", "ROOM")[0],
                         EXIT_OK)
        code, text = run("project-inspect", project, "--json")
        self.assertIn('"name": "CLI Mod"', text)
        code, text = run("mod-export", project, "--output", self.root / "cli.obcmod")
        self.assertEqual(code, EXIT_OK, text)
        code, text = run("mod-audit", self.root / "cli.obcmod")
        self.assertEqual(code, EXIT_OK, text)
        self.assertIn("result: ok", text)

    def test_an_imported_map_needs_private(self):
        project = self.root / "imp.obcproject"
        code, text = run("project-import", self.archive, "--project", project, "--map", 5)
        self.assertEqual(code, EXIT_OK, text)
        code, text = run("mod-export", project, "--output", self.root / "x.obcmod")
        self.assertEqual(code, EXIT_ERROR)
        self.assertIn("OBC-MOD-009", text)
        # Private, and forced past the synthetic map's validation errors.
        code, text = run("mod-export", project, "--output", self.root / "x.obcmod",
                         "--private", "--force")
        self.assertIn("x.obcmod", text)

    def test_writing_into_the_source_is_refused(self):
        project = self.root / "archive" / "inside.obcproject"
        (self.root / "archive" / "GFXTILES.BC").write_bytes(b"x")
        code, _ = run("project-import", self.archive, "--project", project, "--map", 0)
        self.assertEqual(code, EXIT_ERROR)


if __name__ == "__main__":
    unittest.main()
