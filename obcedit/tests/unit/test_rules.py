#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""OBC's placement rules and compound tools (docs/re/LEVEL_CODES.md)."""

from __future__ import annotations

import unittest

from obc_support import room

from obcedit_core import rules
from obcedit_core.planes import MapPlanes
from obcedit_core.prefabs import PREFABS, by_key, placeable


def apply(document, writes):
    planes = [list(p) for p in document.planes.planes]
    for plane, x, y, value in writes:
        planes[plane][y * document.width + x] = value
    from dataclasses import replace

    return replace(document, planes=MapPlanes(document.width, document.height,
                                              tuple(tuple(p) for p in planes)))


class Doors(unittest.TestCase):
    def test_the_code_carries_the_axis(self):
        self.assertEqual(rules.door_value(0, True), 130)
        self.assertEqual(rules.door_value(2, False), 135)
        self.assertTrue(rules.door_is_vertical(132))
        self.assertEqual(rules.door_lock(137), 3)

    def test_walls_north_and_south_make_a_vertical_door(self):
        document = room(["###", ".D.", "###"])
        self.assertTrue(rules.door_axis(document, 1, 1).vertical)
        self.assertEqual(rules.check_door(document, 1, 1), [])

    def test_a_door_turned_the_wrong_way_is_reported_with_a_fix(self):
        document = room(["#.#", "#D#", "#.#"])  # walls E/W: needs a horizontal door
        problems = rules.check_door(document, 1, 1)
        self.assertEqual([p.code for p in problems], ["OBC-DOOR-001"])
        self.assertEqual(problems[0].fix, "door-axis")
        self.assertEqual(rules.door_for_cell(document, 1, 1, 0), 131)

    def test_a_door_on_the_edge_leads_out_of_the_world(self):
        document = room(["D.."])
        self.assertEqual(rules.check_door(document, 0, 0)[0].code, "OBC-DOOR-002")


class PushWalls(unittest.TestCase):
    def test_it_needs_two_open_cells_ahead(self):
        open_room = room(["#####", "#.P..#", "#####"])
        self.assertEqual(rules.pushwall_directions(open_room, 2, 1), [1])  # pushed east
        self.assertEqual(rules.pushwall_path(2, 1, 1), [(3, 1), (4, 1)])
        cramped = room(["#####", "#.P.#", "#####"])
        self.assertEqual(rules.pushwall_directions(cramped, 2, 1), [])
        self.assertEqual(rules.check_pushwall(cramped, 2, 1)[0].code, "OBC-PUSH-002")

    def test_a_marker_on_floor_is_not_a_push_wall(self):
        document = room(["#...#"])
        from dataclasses import replace
        planes = list(document.planes.planes[1])
        planes[1] = 98
        document = replace(document, planes=MapPlanes(64, 64, (document.planes.planes[0],
                                                               tuple(planes), document.planes.planes[2])))
        self.assertEqual(rules.check_pushwall(document, 1, 0)[0].code, "OBC-PUSH-001")


class Prefabs(unittest.TestCase):
    def test_every_prefab_records_its_evidence_and_keys_are_unique(self):
        keys = [p.key for p in PREFABS]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertTrue(all(p.evidence for p in PREFABS))
        self.assertLess(len(placeable()), len(placeable(include_advanced=True)))

    def test_a_door_picks_its_axis_where_it_is_placed(self):
        door = by_key("prefab.door.2")
        across = room(["#.#", "#.#", "#.#"])
        self.assertEqual(door.check(across, 1, 1), [])
        self.assertIn((0, 1, 1, 133), door.placement_on(across, 1, 1))
        corridor = room(["###", "...", "###"])
        self.assertIn((0, 1, 1, 132), door.placement_on(corridor, 1, 1))

    def test_a_door_in_an_open_room_is_refused(self):
        door = by_key("prefab.door.1")
        self.assertEqual(door.check(room(["...", "...", "..."]), 1, 1)[0].code, "OBC-DOOR-003")

    def test_compound_words(self):
        self.assertEqual(by_key("prefab.doorlook.69.103").placement(5, 5),
                         [(0, 5, 5, 69), (1, 5, 5, 103)])
        self.assertEqual(by_key("prefab.glass").placement(1, 1), [(0, 1, 1, 57), (1, 1, 1, 104)])
        self.assertEqual(by_key("prefab.fan.fast").placement(0, 0), [(0, 0, 0, 73), (1, 0, 0, 88)])
        self.assertEqual(by_key("prefab.ambush.soldier").placement(2, 2),
                         [(0, 2, 2, 138), (1, 2, 2, 116)])
        self.assertEqual(by_key("prefab.generator.off").placement(0, 0), [(0, 0, 0, 96)])

    def test_push_wall_needs_a_wall(self):
        prefab = by_key("prefab.pushwall")
        document = room(["#.#"])
        self.assertEqual(prefab.check(document, 0, 0), [])
        self.assertTrue(prefab.check(document, 1, 0))

    def test_the_squad_rotates(self):
        squad = by_key("prefab.squad").rotated(1)
        self.assertEqual(sorted((w.dx, w.dy) for w in squad.writes), [(0, 0), (0, 1), (0, 2)])

    def test_exit_needs_floor_beside_it(self):
        exit_prefab = by_key("prefab.exit.elevator")
        self.assertEqual(exit_prefab.check(room(["#..#"]), 0, 0), [])
        self.assertTrue(exit_prefab.check(room(["####", "####"]), 1, 0))

    def test_placing_then_removing_leaves_floor(self):
        document = room(["....."])
        prefab = by_key("prefab.ambush.soldier")
        placed = apply(document, prefab.placement(2, 0))
        removed = apply(placed, prefab.removal(2, 0))
        self.assertEqual(removed.cell(0, 2, 0), 144)
        self.assertEqual(removed.cell(1, 2, 0), 18)


if __name__ == "__main__":
    unittest.main()
