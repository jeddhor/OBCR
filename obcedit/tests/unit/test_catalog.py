#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The catalog, generated from OBCR by `obc-tool catalog`, and the transforms that use it."""

from __future__ import annotations

import json
import unittest

from obc_support import CATALOG, EDITOR, room

from obcedit_core.catalog import CATEGORIES, CatalogError, catalog_from_data, catalog_from_json
from obcedit_core.transforms import copy_region, flip_clip, rotate_clip


class Shipped(unittest.TestCase):
    def test_every_category_has_entries(self):
        for category in CATEGORIES:
            self.assertTrue(CATALOG.in_category(category), category)

    def test_known_words(self):
        self.assertEqual(CATALOG.for_value(0, 1).category, "walls")
        self.assertEqual(CATALOG.wall_page(1), 0)
        self.assertEqual(CATALOG.kind(0, 130), "door")
        self.assertEqual(CATALOG.kind(0, 144), "floor")
        self.assertEqual(CATALOG.kind(0, 115), "exit")
        self.assertEqual(CATALOG.kind(1, 98), "push_wall")
        self.assertEqual(CATALOG.kind(1, 116), "enemy")
        self.assertEqual(CATALOG.kind(1, 224), "ally")
        self.assertEqual(CATALOG.kind(1, 52), "pickup")

    def test_an_actor_is_one_entry_with_four_facings(self):
        soldier = CATALOG.for_value(1, 117)
        self.assertEqual(dict(soldier.directions),
                         {"east": 116, "north": 117, "west": 118, "south": 119})
        self.assertEqual(soldier.minskill, 0)
        self.assertEqual(CATALOG.for_value(1, 153).minskill, 1)
        self.assertEqual(CATALOG.for_value(1, 189).minskill, 2)
        self.assertEqual(soldier.facing_of(118), "west")

    def test_the_player_start_has_four_facings(self):
        start = CATALOG.by_key("player-start")
        self.assertEqual(dict(start.directions),
                         {"north": 19, "east": 20, "south": 21, "west": 22})

    def test_victor_ends_the_game(self):
        self.assertTrue(CATALOG.record(1, 214)["ends_game"])

    def test_the_one_release_difference_is_recorded(self):
        self.assertTrue(CATALOG.record(1, 71).get("blocking_cd"))

    def test_limits(self):
        self.assertEqual(CATALOG.limit("max_doors"), 64)
        self.assertEqual(CATALOG.limit("num_areas"), 22)

    def test_search(self):
        names = [e.name for e in CATALOG.search("uzi")]
        self.assertIn("Uzi", names)
        self.assertTrue(CATALOG.search("116"))


class Schema(unittest.TestCase):
    def raw(self):
        return json.loads((EDITOR / "resources" / "catalog.json").read_text(encoding="utf-8"))

    def test_a_newer_catalog_is_refused(self):
        data = self.raw()
        data["version"] = 99
        with self.assertRaises(CatalogError):
            catalog_from_data(data)

    def test_something_else_is_refused(self):
        with self.assertRaises(CatalogError):
            catalog_from_json('{"schema": "editor_catalog"}')
        with self.assertRaises(CatalogError):
            catalog_from_json("not json")


class Transforms(unittest.TestCase):
    def test_rotation_turns_facings_and_doors(self):
        document = room([
            "#####",
            "#e..#",
            "#.S.#",
            "#####",
        ])
        clip = copy_region(document, 0, 0, 5, 4)
        turned = rotate_clip(clip, 1, CATALOG)
        values = set(turned.planes[1])
        # Clockwise on screen: east becomes south.
        self.assertIn(119, values)       # soldier facing south
        self.assertIn(21, values)        # start facing south
        door = room(["#D#"])
        door_clip = rotate_clip(copy_region(door, 0, 0, 3, 1), 1, CATALOG)
        self.assertIn(131, door_clip.planes[0])  # vertical 130 became horizontal 131

    def test_four_turns_are_identity(self):
        document = room(["#e.S#", "#.A.#"])
        clip = copy_region(document, 0, 0, 5, 2)
        turned = clip
        for _ in range(4):
            turned = rotate_clip(turned, 1, CATALOG)
        self.assertEqual(turned.planes, clip.planes)

    def test_flip_mirrors_facings(self):
        document = room(["#e.S#"])
        flipped = flip_clip(copy_region(document, 0, 0, 5, 1), "horizontal", CATALOG)
        self.assertIn(118, flipped.planes[1])  # east -> west
        self.assertIn(22, flipped.planes[1])


if __name__ == "__main__":
    unittest.main()
