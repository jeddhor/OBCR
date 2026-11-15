# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The shared PNG vectors (docs/test-vectors/png.json).

OBCR's decoder (obc_formats::modassets::decode_png) is tested against the
same files, so the editor's preview and the game read every kind of PNG the
same way: low bit depths scaled, 16-bit reduced to the high byte, tRNS.
"""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

from obcedit_core.artimport import read_png

VECTORS = Path(__file__).resolve().parents[3] / "docs" / "test-vectors"


class SharedPngVectors(unittest.TestCase):
    def test_every_image_decodes_to_the_expected_pixels(self):
        doc = json.loads((VECTORS / "png.json").read_text(encoding="utf-8"))
        self.assertEqual(len(doc["images"]), 14)
        for name, case in doc["images"].items():
            with self.subTest(name):
                image = read_png((VECTORS / case["file"]).read_bytes(), name)
                rgba = bytes(image.rgba)
                self.assertEqual(hashlib.sha256(rgba).hexdigest(), case["rgba_sha256"])
                for at, want in case["samples"].items():
                    x, y = map(int, at.split(","))
                    i = (y * 64 + x) * 4
                    self.assertEqual(list(rgba[i:i + 4]), want, at)


if __name__ == "__main__":
    unittest.main()
