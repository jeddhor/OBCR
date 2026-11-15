#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The MAPHEAD.BC/MAPTEMP.BC codec: synthetic fixtures always, the user's data when present."""

from __future__ import annotations

import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from obc_support import make_fixtures, real_data, write_archive

from obcedit_core.archive import (
    MAPHEAD_BYTES,
    SIGNATURE,
    MapRecord,
    encode_archive,
    parse_archive,
    read_archive,
    replace_records,
    verify_archive,
)
from obcedit_core.errors import NativeFormatError
from obcedit_core.names import NativeName
from obcedit_core.planes import MapPlanes


class Fixtures(unittest.TestCase):
    def test_the_generator_is_deterministic(self):
        self.assertEqual(make_fixtures.digests(), make_fixtures.digests())

    def test_fixture_words_are_synthetic(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = read_archive(write_archive(Path(tmp)))
        for record in archive:
            self.assertTrue(record.name.text.startswith("SYNTH"))
            for plane in record.planes.planes:
                self.assertTrue(all(word >= make_fixtures.SYNTH_BASE for word in plane))


class Reading(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.dir = write_archive(Path(self._tmp.name))

    def tearDown(self):
        self._tmp.cleanup()

    def test_slots_sizes_and_names(self):
        archive = read_archive(self.dir)
        self.assertEqual([(r.number, r.width, r.height, r.name.text) for r in archive],
                         [(0, 8, 8, "SYNTH00"), (1, 16, 16, "SYNTH01"), (5, 64, 64, "SYNTH05")])
        self.assertEqual(archive.by_slot(5).label, "slot 05")

    def test_either_file_or_the_folder_opens_it(self):
        for path in (self.dir, self.dir / "MAPHEAD.BC", self.dir / "MAPTEMP.BC"):
            self.assertEqual(len(read_archive(path)), 3)

    def test_a_missing_slot_is_an_error(self):
        with self.assertRaises(NativeFormatError):
            read_archive(self.dir).by_number(2)

    def test_malformed_archives_are_refused(self):
        fixtures = make_fixtures.fixture_set()
        for case in ("bad-short-maphead", "bad-truncated", "bad-empty", "bad-zero-width"):
            with self.subTest(case=case), self.assertRaises(NativeFormatError):
                parse_archive(fixtures[f"{case}.MAPHEAD.BC"], fixtures[f"{case}.MAPTEMP.BC"])

    def test_name_tails_are_kept(self):
        fixtures = make_fixtures.fixture_set()
        archive = parse_archive(fixtures["tailname.MAPHEAD.BC"], fixtures["tailname.MAPTEMP.BC"])
        record = archive[0]
        self.assertFalse(record.name.is_canonical)
        self.assertIn("OBC-NATIVE-007", archive.diagnostics.codes())
        head, temp = encode_archive(archive.records)
        self.assertEqual(parse_archive(head, temp)[0].name.raw, record.name.raw)


class Writing(unittest.TestCase):
    def record(self, slot, salt=0):
        words = tuple((salt + i) % 7 + 140 for i in range(64 * 64))
        return MapRecord(slot, NativeName.from_text(f"MAP {slot}"),
                         MapPlanes(64, 64, (words, (18,) * 4096, (0,) * 4096)))

    def test_canonical_layout(self):
        head, temp = encode_archive([self.record(3), self.record(0)])
        self.assertEqual(len(head), MAPHEAD_BYTES)
        self.assertTrue(temp.startswith(SIGNATURE))
        self.assertTrue(temp.endswith(b"!ID!"))
        archive = parse_archive(head, temp)
        self.assertEqual([r.number for r in archive], [0, 3])
        # New maps go in slot order; unused slots are -1.
        self.assertLess(archive.by_slot(0).source.header_offset,
                        archive.by_slot(3).source.header_offset)

    def test_round_trip(self):
        records = [self.record(0, 1), self.record(7, 2)]
        archive = parse_archive(*encode_archive(records))
        for original, back in zip(records, archive):
            self.assertEqual(original.planes, back.planes)

    def test_a_reencode_keeps_the_original_order(self):
        records = [self.record(0), self.record(1)]
        head, temp = encode_archive(records)
        archive = parse_archive(head, temp)
        # Pretend slot 1 came first in the file (as the original stores slot 56 last).
        moved = [replace(archive[0], source=replace(archive[0].source, header_offset=10**9)),
                 archive[1]]
        _, again = encode_archive(moved)
        self.assertLess(again.find(b"MAP 1"), again.find(b"MAP 0"))

    def test_duplicate_slots_are_refused(self):
        with self.assertRaises(NativeFormatError):
            encode_archive([self.record(2), self.record(2)])

    def test_replace_records_changes_only_that_slot(self):
        head, temp = encode_archive([self.record(0, 1), self.record(1, 2)])
        archive = parse_archive(head, temp)
        new = self.record(0, 5)
        archive2 = parse_archive(*replace_records(archive, {1: new}))
        self.assertEqual(archive2.by_slot(0).planes, archive.by_slot(0).planes)
        self.assertEqual(archive2.by_slot(1).planes, new.planes)
        with self.assertRaises(NativeFormatError):
            replace_records(archive, {9: new})


class OriginalData(unittest.TestCase):
    """Data-gated: re-encoding the user's own archive is byte-identical."""

    def test_both_releases_round_trip_exactly(self):
        found = False
        for release in ("full", "cd"):
            directory = real_data(release)
            if directory is None:
                continue
            found = True
            with self.subTest(release=release):
                result = verify_archive(directory)
                self.assertEqual(result.maps, 61)
                self.assertTrue(result.identical, result)
        if not found:
            self.skipTest("no game data (set OBC_DATA_DIR)")


if __name__ == "__main__":
    unittest.main()
