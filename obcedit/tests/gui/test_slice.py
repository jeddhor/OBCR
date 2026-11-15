#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The slice workflow (slice_workflow.py) against the developer's own data.

Skipped unless the game data is present (OBC_DATA_DIR, or .local/data/obc-full
in this checkout), which it never is in CI. The OBCR half (--check-mod and a
snapshot) runs when a built `obcr` is in target/.
"""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from gui_support import obc_support

import slice_workflow


class SliceWorkflow(unittest.TestCase):
    def test_the_slice(self):
        data = obc_support.real_data("full")
        if data is None:
            self.skipTest("no game data (set OBC_DATA_DIR)")
        lines = []
        with tempfile.TemporaryDirectory() as work:
            failures = slice_workflow.run(data, Path(work), obc_support.engine(),
                                          out=lines.append)
        self.assertEqual(failures, [], "\n".join(lines))


if __name__ == "__main__":
    unittest.main(verbosity=1)
