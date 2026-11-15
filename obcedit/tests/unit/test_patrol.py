#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Patrol routes: planning arrows from clicks, and following patrols as the engine does."""

from __future__ import annotations

import unittest
from dataclasses import replace

from obc_support import CATALOG, real_data, room

from obcedit_core import patrol
from obcedit_core.document import MapDocument, SourceReference
from obcedit_core.errors import Severity
from obcedit_core.planes import MapPlanes
from obcedit_core.validation import patrol_problems, validate_map

E, NE, N, NW, W, SW, S, SE = range(8)
#: A soldier patrolling east / north / west / south (LEVEL_CODES section 5).
SOLDIER = {E: 120, N: 121, W: 122, S: 123}
STANDING_SOLDIER = 116
HOVERING_SAUCER = 270
AMMO = 56          # a pickup: does not block
BARREL = 23        # a blocking static

HALL = [
    "#########",
    "#.......#",
    "#.......#",
    "#.......#",
    "#########",
]


def put(document, *words):
    """Write (plane, x, y, value) words."""
    planes = [list(p) for p in document.planes.planes]
    for plane, x, y, value in words:
        planes[plane][y * document.width + x] = value
    return replace(document, planes=MapPlanes(document.width, document.height,
                                              tuple(tuple(p) for p in planes)))


def hall(*objects):
    return put(room(HALL), *((1, x, y, v) for x, y, v in objects))


def apply(document, route):
    return put(document, *route.writes)


def arrow(direction):
    return patrol.arrow_value(direction, CATALOG)


class Basics(unittest.TestCase):
    def test_arrow_words_follow_the_catalog(self):
        self.assertEqual([arrow(d) for d in range(8)], list(range(90, 98)))
        self.assertEqual(patrol.arrow_value(S), 96)
        self.assertEqual(patrol.arrow_direction(94), W)
        self.assertIsNone(patrol.arrow_direction(89))
        self.assertIsNone(patrol.arrow_direction(98))

    def test_patrollers_face_their_code_with_or_without_the_catalog(self):
        for catalog in (CATALOG, None):
            self.assertEqual(patrol.patrol_facing(120, catalog), E)
            self.assertEqual(patrol.patrol_facing(121, catalog), N)
            self.assertEqual(patrol.patrol_facing(122, catalog), W)
            self.assertEqual(patrol.patrol_facing(123, catalog), S)
            self.assertIsNone(patrol.patrol_facing(STANDING_SOLDIER, catalog))
            self.assertIsNone(patrol.patrol_facing(HOVERING_SAUCER, catalog))
            self.assertEqual(patrol.turned_to(121, E, catalog), 120)
            self.assertIsNone(patrol.turned_to(121, NE, catalog))

    def test_directions_between_cells(self):
        self.assertEqual(patrol.direction_between((1, 1), (5, 1)), E)
        self.assertEqual(patrol.direction_between((5, 3), (5, 1)), N)
        self.assertEqual(patrol.direction_between((1, 1), (3, 3)), SE)
        self.assertIsNone(patrol.direction_between((1, 1), (4, 2)))
        self.assertIsNone(patrol.direction_between((1, 1), (1, 1)))


class Planning(unittest.TestCase):
    def test_a_straight_route_marks_its_start(self):
        route = patrol.plan_route(room(HALL), [(1, 1), (5, 1)], CATALOG)
        self.assertTrue(route.complete, route.reason)
        self.assertEqual(route.arrows, ((1, 1, arrow(E)),))
        self.assertEqual(route.cells, tuple((x, 1) for x in range(1, 6)))
        self.assertFalse(route.closed)

    def test_an_l_turns_on_its_corner(self):
        route = patrol.plan_route(room(HALL), [(1, 1), (5, 1), (5, 3)], CATALOG)
        self.assertTrue(route.ok, route.reason)
        self.assertEqual(set(route.arrows), {(1, 1, arrow(E)), (5, 1, arrow(S))})

    def test_clicking_along_a_line_adds_no_arrow(self):
        route = patrol.plan_route(room(HALL), [(1, 1), (3, 1), (5, 1)], CATALOG)
        self.assertEqual(route.arrows, ((1, 1, arrow(E)),))

    def test_a_closed_loop_is_followed_forever(self):
        loop = [(1, 1), (5, 1), (5, 3), (1, 3), (1, 1)]
        route = patrol.plan_route(room(HALL), loop, CATALOG)
        self.assertTrue(route.ok and route.closed, route.reason)
        self.assertEqual(set(route.arrows), {(1, 1, arrow(E)), (5, 1, arrow(S)),
                                             (5, 3, arrow(W)), (1, 3, arrow(N))})
        document = put(apply(room(HALL), route), (1, 3, 1, SOLDIER[E]))
        walk = patrol.follow(patrol.Grid(document, CATALOG), 3, 1, E)
        self.assertTrue(walk.loops)
        self.assertEqual(walk.arrows, {(1, 1), (5, 1), (5, 3), (1, 3)})

    def test_there_and_back(self):
        route = patrol.plan_route(room(HALL), [(1, 2), (6, 2), (1, 2)], CATALOG)
        self.assertTrue(route.ok and route.closed, route.reason)
        self.assertEqual(set(route.arrows), {(1, 2, arrow(E)), (6, 2, arrow(W))})

    def test_a_diagonal(self):
        route = patrol.plan_route(room(HALL), [(1, 1), (3, 3)], CATALOG)
        self.assertTrue(route.ok, route.reason)
        self.assertEqual(route.arrows, ((1, 1, arrow(SE)),))
        self.assertEqual(route.cells, ((1, 1), (2, 2), (3, 3)))

    def test_a_diagonal_cannot_squeeze_past_a_corner(self):
        document = put(room(HALL), (0, 2, 1, 1))          # a wall beside the corner
        route = patrol.plan_route(document, [(1, 1), (3, 3)], CATALOG)
        self.assertFalse(route.ok)
        self.assertEqual(route.bad_cell, (2, 1))

    def test_a_diagonal_cannot_pass_a_door(self):
        document = put(room(HALL), (0, 2, 2, 130))
        self.assertFalse(patrol.plan_route(document, [(1, 1), (3, 3)], CATALOG).ok)

    def test_a_straight_leg_passes_a_door(self):
        document = put(room(HALL), (0, 3, 2, 130))
        self.assertTrue(patrol.plan_route(document, [(1, 2), (6, 2)], CATALOG).ok)

    def test_refused_through_a_wall(self):
        document = put(room(HALL), (0, 3, 1, 1))
        route = patrol.plan_route(document, [(1, 1), (5, 1)], CATALOG)
        self.assertFalse(route.ok)
        self.assertEqual(route.bad_cell, (3, 1))
        self.assertIn("blocked", route.reason)

    def test_refused_through_a_blocking_object_but_not_a_pickup(self):
        self.assertFalse(patrol.plan_route(hall((3, 1, BARREL)), [(1, 1), (5, 1)], CATALOG).ok)
        self.assertTrue(patrol.plan_route(hall((3, 1, AMMO)), [(1, 1), (5, 1)], CATALOG).ok)

    def test_refused_through_a_standing_enemy(self):
        route = patrol.plan_route(hall((3, 1, STANDING_SOLDIER)), [(1, 1), (5, 1)], CATALOG)
        self.assertFalse(route.ok)
        self.assertEqual(route.bad_cell, (3, 1))

    def test_refused_off_a_straight_line(self):
        route = patrol.plan_route(room(HALL), [(1, 1), (4, 2)], CATALOG)
        self.assertFalse(route.ok)
        self.assertEqual(route.bad_cell, (4, 2))

    def test_refused_on_an_occupied_turn(self):
        route = patrol.plan_route(hall((5, 1, AMMO)), [(1, 1), (5, 1), (5, 3)], CATALOG)
        self.assertFalse(route.ok)
        self.assertEqual(route.bad_cell, (5, 1))
        self.assertIn("to itself", route.reason)

    def test_refused_on_a_wall_cell(self):
        self.assertFalse(patrol.plan_route(room(HALL), [(0, 1), (3, 1)], CATALOG).ok)

    def test_starting_on_a_patroller_facing_the_first_leg(self):
        document = hall((1, 1, SOLDIER[E]))
        route = patrol.plan_route(document, [(1, 1), (5, 1), (5, 3)], CATALOG)
        self.assertTrue(route.ok, route.reason)
        self.assertEqual(route.arrows, ((5, 1, arrow(S)),))
        self.assertIsNone(route.turned)
        self.assertFalse([w for w in route.writes if w[1:3] == (1, 1)])

    def test_starting_on_a_patroller_facing_elsewhere_turns_it(self):
        document = hall((1, 1, SOLDIER[N]))
        route = patrol.plan_route(document, [(1, 1), (5, 1), (5, 3)], CATALOG)
        self.assertTrue(route.ok, route.reason)
        self.assertEqual(route.turned, (1, 1, SOLDIER[N], SOLDIER[E]))
        self.assertIn((1, 1, 1, SOLDIER[E]), route.writes)
        walk = patrol.follow(patrol.Grid(apply(document, route), CATALOG), 1, 1, E)
        self.assertEqual(walk.stops_at, (5, 3))

    def test_a_patroller_cannot_start_on_a_diagonal(self):
        route = patrol.plan_route(hall((1, 1, SOLDIER[N])), [(1, 1), (3, 3)], CATALOG)
        self.assertFalse(route.ok)
        self.assertIn("east, north, west or south", route.reason)

    def test_a_loop_through_the_patroller_must_pass_straight(self):
        document = hall((3, 1, SOLDIER[E]))
        turning = patrol.plan_route(document, [(3, 1), (5, 1), (5, 3), (3, 3), (3, 1)], CATALOG)
        self.assertFalse(turning.ok)
        self.assertIn("pass straight through", turning.reason)
        good = patrol.plan_route(document, [(3, 1), (5, 1), (5, 3), (1, 3), (1, 1), (3, 1)],
                                 CATALOG)
        self.assertTrue(good.ok and good.closed, good.reason)
        self.assertNotIn((3, 1), {(x, y) for x, y, _v in good.arrows})
        walk = patrol.follow(patrol.Grid(apply(document, good), CATALOG), 3, 1, E)
        self.assertTrue(walk.loops)

    def test_standing_enemies_and_hovering_saucers_do_not_patrol(self):
        self.assertIn("does not patrol", patrol.plan_route(
            hall((1, 1, STANDING_SOLDIER)), [(1, 1), (5, 1)], CATALOG).reason)
        self.assertIn("never moves", patrol.plan_route(
            hall((1, 1, HOVERING_SAUCER)), [(1, 1), (5, 1)], CATALOG).reason)

    def test_an_arrow_in_the_way_must_agree(self):
        self.assertFalse(patrol.plan_route(hall((3, 1, arrow(S))), [(1, 1), (5, 1)], CATALOG).ok)
        self.assertTrue(patrol.plan_route(hall((3, 1, arrow(E))), [(1, 1), (5, 1)], CATALOG).ok)

    def test_an_existing_arrow_on_a_turn_is_replaced(self):
        route = patrol.plan_route(hall((5, 1, arrow(N))), [(1, 1), (5, 1), (5, 3)], CATALOG)
        self.assertTrue(route.ok)
        self.assertIn((1, 5, 1, arrow(S)), route.writes)

    def test_a_route_cannot_cross_itself_where_it_turns(self):
        route = patrol.plan_route(
            room(HALL), [(1, 2), (3, 2), (3, 3), (5, 3), (5, 2), (1, 2)], CATALOG)
        self.assertFalse(route.ok)
        self.assertEqual(route.bad_cell, (3, 2))

    def test_one_click_is_not_a_route_yet(self):
        route = patrol.plan_route(room(HALL), [(1, 1)], CATALOG)
        self.assertTrue(route.ok)
        self.assertFalse(route.complete)
        self.assertEqual(route.writes, ())


class Following(unittest.TestCase):
    def test_a_patroller_walks_until_a_wall(self):
        document = hall((1, 1, SOLDIER[E]))
        walk = patrol.follow(patrol.Grid(document, CATALOG), 1, 1, E)
        self.assertEqual(walk.stops_at, (7, 1))
        self.assertEqual(walk.blocked_by, (8, 1))
        self.assertEqual(walk.reason, "a wall")
        self.assertFalse(walk.loops)

    def test_patrollers_are_found_in_reading_order(self):
        document = hall((5, 2, SOLDIER[W]), (1, 1, SOLDIER[S]), (2, 2, STANDING_SOLDIER))
        self.assertEqual(patrol.patrollers(document, CATALOG), [(1, 1, S), (5, 2, W)])


class Validation(unittest.TestCase):
    def codes(self, document):
        return {p.code for p in validate_map(document, CATALOG)}

    def test_a_patroller_walking_into_a_wall(self):
        problems = patrol_problems(hall((1, 1, SOLDIER[E])), CATALOG)
        self.assertEqual([p.code for p in problems], ["OBC-PATROL-001"])
        self.assertEqual(problems[0].cell, (1, 1))
        self.assertIn("(7, 1)", problems[0].message)
        self.assertIs(problems[0].severity, Severity.WARNING)

    def test_a_standing_enemy_in_the_way_is_only_a_note(self):
        problems = patrol_problems(hall((1, 1, SOLDIER[E]), (5, 1, STANDING_SOLDIER)), CATALOG)
        self.assertEqual([(p.code, p.severity) for p in problems],
                         [("OBC-PATROL-001", Severity.INFORMATION)])
        self.assertIn("unless that enemy has moved", problems[0].message)

    def test_an_orphan_arrow(self):
        problems = patrol_problems(hall((4, 2, arrow(N))), CATALOG)
        self.assertEqual([(p.code, p.cell) for p in problems], [("OBC-PATROL-002", (4, 2))])

    def test_a_drawn_loop_is_clean(self):
        document = hall((3, 1, SOLDIER[E]))
        route = patrol.plan_route(document, [(3, 1), (5, 1), (5, 3), (1, 3), (1, 1), (3, 1)],
                                  CATALOG)
        document = apply(document, route)
        self.assertFalse(patrol_problems(document, CATALOG))
        self.assertFalse({"OBC-PATROL-001", "OBC-PATROL-002"} & self.codes(document))

    def test_the_validator_reports_them(self):
        self.assertIn("OBC-PATROL-001", self.codes(hall((1, 1, SOLDIER[E]))))
        self.assertIn("OBC-PATROL-002", self.codes(hall((4, 2, arrow(N)))))

    def test_imported_maps_get_notes(self):
        document = replace(hall((1, 1, SOLDIER[E])), source=SourceReference(sha256="x"))
        problems = [p for p in validate_map(document, CATALOG) if p.code == "OBC-PATROL-001"]
        self.assertEqual([p.severity for p in problems], [Severity.INFORMATION])


class OriginalFloors(unittest.TestCase):
    """Data-gated: follow every patrol on the shipped floors without errors."""

    def test_patrols_on_the_original_floors(self):
        directory = real_data("full")
        if directory is None:
            self.skipTest("no game data (set OBC_DATA_DIR)")
        from obcedit_core.archive import read_archive

        stops = orphans = patrols = 0
        for record in read_archive(directory):
            document = MapDocument.from_record(record, source=SourceReference(sha256="original"))
            for x, y, facing in patrol.patrollers(document, CATALOG):
                patrols += 1
                walk = patrol.follow(patrol.Grid(document, CATALOG), x, y, facing)
                self.assertTrue(walk.loops or walk.stops_at is not None)
            problems = patrol_problems(document, CATALOG, imported=True)
            self.assertTrue(all(p.severity is Severity.INFORMATION for p in problems))
            stops += sum(p.code == "OBC-PATROL-001" for p in problems)
            orphans += sum(p.code == "OBC-PATROL-002" for p in problems)
        self.assertGreater(patrols, 0)
        print(f"\n  original floors: {patrols} patrols, {stops} stop for good, "
              f"{orphans} arrows unread", end="")


if __name__ == "__main__":
    unittest.main()
