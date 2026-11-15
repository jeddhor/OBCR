#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The patrol-route tool, driven through the real window.

Click the turns of a route on the canvas and finish it; the arrows land on
the right cells, the whole route is one undo step, a click the route cannot
take is refused without losing what was drawn, and Esc drops it. Offscreen,
synthetic, no game data.
"""

from __future__ import annotations

import os
import sys
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

EDITOR = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(EDITOR))

from PySide6.QtCore import QEvent, Qt
from PySide6.QtGui import QKeyEvent
from PySide6.QtWidgets import QApplication

from obcedit_core.commands import write_words
from obcedit_gui.tools import EMPTY_OBJECT, Tool

from test_editing import Base

#: A soldier patrolling east (+1 north, +2 west, +3 south).
PATROL_EAST = 120
E, N, W, S = 90, 92, 94, 96


class PatrolTool(Base):
    def click(self, x, y):
        self.window._on_press(x, y, Qt.LeftButton.value)
        self.window.tools.release(x, y)

    def canvas(self):
        return self.window.current_tab.canvas

    def test_the_tool_is_on_the_toolbar_with_its_key(self):
        action = self.window.tool_actions[Tool.PATROL]
        self.assertEqual(action.shortcut().toString(), "T")
        self.window.select_tool(Tool.PATROL)
        self.assertIs(self.window.tools.tool, Tool.PATROL)

    def test_a_loop_is_drawn_with_clicks_and_is_one_undo(self):
        self.window.select_tool(Tool.PATROL)
        depth = self.window.history.depth
        for cell in [(2, 2), (6, 2), (6, 6), (2, 6)]:
            self.click(*cell)
        # Nothing is written until the route is finished.
        self.assertEqual(self.window.history.depth, depth)
        self.assertIsNotNone(self.canvas().route)
        self.click(2, 2)  # back to the first cell: closes and finishes
        self.assertEqual(self.document.cell(1, 2, 2), E)
        self.assertEqual(self.document.cell(1, 6, 2), S)
        self.assertEqual(self.document.cell(1, 6, 6), W)
        self.assertEqual(self.document.cell(1, 2, 6), N)
        self.assertEqual(self.window.history.depth, depth + 1)
        self.assertIsNone(self.canvas().route)
        self.assertEqual(self.window.tools.route, [])
        self.window.undo()
        for cell in [(2, 2), (6, 2), (6, 6), (2, 6)]:
            self.assertEqual(self.document.cell(1, *cell), EMPTY_OBJECT)

    def test_a_double_click_finishes_an_open_route(self):
        self.window.select_tool(Tool.PATROL)
        self.click(2, 3)
        self.click(7, 3)
        self.click(7, 3)
        self.assertEqual(self.document.cell(1, 2, 3), E)
        self.assertEqual(self.document.cell(1, 7, 3), EMPTY_OBJECT)  # its end: no turn

    def test_a_route_from_a_patroller_turns_it(self):
        self.window.run_command(write_words(self.document, [(1, 3, 4, PATROL_EAST + 1)]))
        self.window.select_tool(Tool.PATROL)
        self.click(3, 4)
        self.click(7, 4)
        self.click(7, 7)
        self.window.tools.finish_route()
        self.assertEqual(self.document.cell(1, 3, 4), PATROL_EAST)   # now faces east
        self.assertEqual(self.document.cell(1, 7, 4), S)

    def test_a_refused_click_keeps_the_route(self):
        self.window.run_command(write_words(self.document, [(0, 4, 2, 1)]))   # a wall
        self.window.select_tool(Tool.PATROL)
        self.click(2, 2)
        self.click(6, 2)   # through the wall: refused
        self.assertEqual(self.window.tools.route, [(2, 2)])
        self.assertFalse(self.canvas().route.ok)
        self.assertEqual(self.canvas().route.bad_cell, (4, 2))
        self.click(2, 6)   # a different way works
        self.assertEqual(self.window.tools.route, [(2, 2), (2, 6)])

    def test_hovering_previews_the_next_leg(self):
        self.window.select_tool(Tool.PATROL)
        self.click(2, 2)
        self.window._on_hover(6, 2)
        route = self.canvas().route
        self.assertEqual(route.waypoints, ((2, 2), (6, 2)))
        self.assertTrue(route.ok)

    def test_enter_finishes_and_escape_cancels(self):
        self.window.select_tool(Tool.PATROL)
        canvas = self.canvas()
        self.click(2, 5)
        self.click(5, 5)
        QApplication.sendEvent(canvas, QKeyEvent(QEvent.KeyPress, Qt.Key_Escape, Qt.NoModifier))
        self.assertEqual(self.window.tools.route, [])
        self.assertIsNone(canvas.route)
        self.assertEqual(self.document.cell(1, 2, 5), EMPTY_OBJECT)
        self.click(2, 5)
        self.click(5, 5)
        QApplication.sendEvent(canvas, QKeyEvent(QEvent.KeyPress, Qt.Key_Return, Qt.NoModifier))
        self.assertEqual(self.document.cell(1, 2, 5), E)

    def test_changing_tool_drops_the_route(self):
        self.window.select_tool(Tool.PATROL)
        self.click(2, 2)
        self.window.select_tool(Tool.BRUSH)
        self.assertEqual(self.window.tools.route, [])
        self.assertIsNone(self.canvas().route)

    def test_arrows_and_routes_paint(self):
        """Arrows draw as arrows and the preview paints, without errors."""
        self.window.run_command(write_words(self.document, [(1, 3, 3, v) for v in (E,)]
                                            + [(1, 4, 3, 91), (1, 5, 3, 97)]))
        self.window.select_tool(Tool.PATROL)
        self.click(2, 2)
        self.window._on_hover(6, 2)
        canvas = self.canvas()
        canvas.set_zoom(20)
        image = canvas.grab().toImage()
        # An arrow is drawn in the arrow cell, not a flat inset.
        cell = canvas.cell_rect(3, 3)
        colours = {image.pixel(x, y) for x in range(cell.left(), cell.right())
                   for y in range(cell.top(), cell.bottom())}
        self.assertGreater(len(colours), 2)


if __name__ == "__main__":
    unittest.main()
