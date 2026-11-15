#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The validator and the reachability model, on small maps and the original floors."""

from __future__ import annotations

import unittest
from dataclasses import replace

from obc_support import CATALOG, real_data, room

from obcedit_core.document import MapDocument, SourceReference
from obcedit_core.errors import Severity
from obcedit_core.planes import MapPlanes
from obcedit_core.reachability import analyze
from obcedit_core.validation import (
    LOCAL_CODES,
    NETWORK,
    fix_writes,
    profile_for_slot,
    summarize,
    validate_local,
    validate_map,
)


def codes(document, **kw):
    return {p.code for p in validate_map(document, CATALOG, **kw)}


def with_word(document, plane, x, y, value):
    planes = [list(p) for p in document.planes.planes]
    planes[plane][y * document.width + x] = value
    return replace(document, planes=MapPlanes(document.width, document.height,
                                              tuple(tuple(p) for p in planes)))


GOOD = [
    "#######",
    "#S.ee.E",
    "#.....#",
    "#######",
]


class Errors(unittest.TestCase):
    def test_a_good_room_has_no_errors(self):
        problems = validate_map(room(GOOD), CATALOG)
        self.assertFalse([p for p in problems if p.severity is Severity.ERROR], problems)
        self.assertNotIn("OBC-EXIT-001", {p.code for p in problems})

    def test_a_new_room_is_valid_but_has_no_exit(self):
        found = codes(MapDocument.new_room())
        self.assertEqual(found & {c for c in found if c.startswith("OBC-START")}, set())
        self.assertIn("OBC-EXIT-001", found)

    def test_size(self):
        self.assertIn("OBC-SIZE-001", codes(room(GOOD, size=32)))

    def test_starts(self):
        self.assertIn("OBC-START-001", codes(room(["#....E"])))
        self.assertIn("OBC-START-002", codes(room(["#S.S.E"])))

    def test_open_boundary_has_a_fix(self):
        document = with_word(room(GOOD), 0, 3, 0, 144)
        problems = [p for p in validate_map(document, CATALOG) if p.code == "OBC-BOUNDARY-001"]
        self.assertEqual(problems[0].fix, "seal_boundary")
        fixed = document
        for plane, x, y, value in fix_writes("seal_boundary", document):
            fixed = with_word(fixed, plane, x, y, value)
        self.assertNotIn("OBC-BOUNDARY-001", codes(fixed))

    def test_bad_words(self):
        self.assertIn("OBC-CELL-002", codes(with_word(room(GOOD), 0, 2, 2, 0)))
        self.assertIn("OBC-CELL-002", codes(with_word(room(GOOD), 0, 2, 2, 170)))
        self.assertIn("OBC-THING-002", codes(with_word(room(GOOD), 1, 2, 2, 400)))
        self.assertIn("OBC-THING-003", codes(with_word(room(GOOD), 1, 2, 2, 99)))

    def test_things_in_walls(self):
        inside = with_word(room(GOOD), 1, 1, 3, 116)
        self.assertIn("OBC-THING-001", codes(inside))
        imported = replace(inside, source=SourceReference(sha256="x"))
        problem = next(p for p in validate_map(imported, CATALOG) if p.code == "OBC-THING-001")
        self.assertIs(problem.severity, Severity.WARNING)

    def test_too_many_doors(self):
        rows = ["#" * 63] + ["#.D." * 15 for _ in range(1)] + ["#" * 63]
        document = room(GOOD)
        for i in range(65):
            document = with_word(document, 0, 1 + (i % 60), 2 + (i // 60) * 2, 130)
        self.assertIn("OBC-LIMIT-001", codes(document))

    def test_ambush_without_an_actor(self):
        self.assertIn("OBC-AMBUSH-001", codes(with_word(room(GOOD), 0, 2, 2, 138)))

    def test_local_pass_agrees_with_the_full_one(self):
        document = with_word(with_word(room(GOOD), 0, 3, 0, 144), 1, 1, 3, 116)
        full = [p for p in validate_map(document, CATALOG) if p.code in LOCAL_CODES]
        self.assertEqual(validate_local(document, CATALOG), full)

    def test_summary(self):
        self.assertEqual(summarize([]), "No problems found")
        self.assertIn("error", summarize(validate_map(room(["#....E"]), CATALOG)))


class Reachability(unittest.TestCase):
    def test_an_exit_behind_a_door_is_reachable(self):
        document = room([
            "#####",
            "#S.D.E",
            "#####",
        ])
        document = with_word(document, 0, 3, 1, 130)
        reach = analyze(document, CATALOG)
        self.assertTrue(reach.exits)

    def test_a_walled_off_exit_is_not(self):
        document = room(["#S.#.E"])
        self.assertIn("OBC-EXIT-001", codes(document))

    def test_a_push_wall_opens_the_way(self):
        document = room(["#S.P..E"])
        reach = analyze(document, CATALOG)
        self.assertTrue(reach.pushed)
        self.assertTrue(reach.exits)

    def test_door_look_walls_open(self):
        document = room(["#S.L.E"])
        self.assertTrue(analyze(document, CATALOG).exits)

    def test_the_half_kill_quota(self):
        document = room(["#S.e.E#ee#ee"])  # four enemies walled off, one reachable
        reach = analyze(document, CATALOG)
        self.assertEqual((len(reach.enemies), len(reach.enemies_reached), reach.quota), (5, 1, 2))
        self.assertIn("OBC-KILL-001", codes(document))

    def test_victor_ends_the_floor_without_an_exit(self):
        document = room(["#S.B.#"])
        self.assertTrue(analyze(document, CATALOG).completable)
        self.assertNotIn("OBC-EXIT-001", codes(document))

    def test_network_arenas_need_no_exit(self):
        self.assertIs(profile_for_slot(55), NETWORK)
        self.assertNotIn("OBC-EXIT-001", codes(room(["#S..#"]), profile=NETWORK))


class OriginalFloors(unittest.TestCase):
    """Data-gated: the shipped floors validate without errors and can be finished."""

    def test_every_single_player_floor(self):
        directory = real_data("full")
        if directory is None:
            self.skipTest("no game data (set OBC_DATA_DIR)")
        from obcedit_core.archive import read_archive

        for record in read_archive(directory):
            if record.number >= 40:
                continue
            document = MapDocument.from_record(record, source=SourceReference(sha256="original"))
            problems = validate_map(document, CATALOG, profile=profile_for_slot(record.number))
            with self.subTest(slot=record.number):
                self.assertFalse([p for p in problems if p.severity is Severity.ERROR])
                self.assertTrue(analyze(document, CATALOG).completable)


if __name__ == "__main__":
    unittest.main()
