# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The tool controller: pointer gestures to commands, and nothing in between.

Every tool ends the same way -- a list of `(plane, x, y, value)` writes handed
to one command. That is what makes undo uniform: a flood fill, a dragged line
and a single click are the same kind of thing to the history, and none of them
needed its own inverse.

A drag is one undo step. The controller opens a gesture on press, tags every
command with it, and closes it on release, so the history coalesces the stroke.
Ctrl+Z takes back the line you drew, not the last cell of it.

Which plane a tool writes is the *catalog's* decision, not the tool's. A wall
brush and an enemy brush are the same code; the difference is that the entry
says plane 0 or plane 1, which is why placing a door and placing an enemy did
not need two implementations.
"""

from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from enum import Enum

from PySide6.QtCore import QObject, Qt, Signal

from obcedit_core.catalog import CatalogEntry
from obcedit_core.commands import Command, write_words
from obcedit_core.document import MapDocument
from obcedit_core.patrol import Route, plan_route
from obcedit_core.prefabs import Prefab
from obcedit_core.tools import (
    FILL_BUDGET,
    flood_cells,
    line_cells,
    pick,
    rectangle_bounds,
    rectangle_cells,
)

#: The empty object marker (the original editor's background), which is what
#: erasing plane 1 writes.
EMPTY_OBJECT = 18
#: What erasing plane 0 writes: open floor, area 4 with texture set 0. See
#: MapDocument.DEFAULT_FLOOR for why it must not be zero.
DEFAULT_FLOOR = MapDocument.DEFAULT_FLOOR


class Tool(Enum):
    POINTER = "pointer"
    BRUSH = "brush"
    LINE = "line"
    RECTANGLE = "rectangle"
    FILL = "fill"
    ERASER = "eraser"
    EYEDROPPER = "eyedropper"
    PREFAB = "prefab"
    CAMERA = "camera"
    PATROL = "patrol"

    @property
    def label(self) -> str:
        return {
            Tool.POINTER: "Select",
            Tool.BRUSH: "Paint",
            Tool.LINE: "Line",
            Tool.RECTANGLE: "Rectangle",
            Tool.FILL: "Fill",
            Tool.ERASER: "Erase",
            Tool.EYEDROPPER: "Pick",
            Tool.PREFAB: "Place",
            Tool.CAMERA: "Camera",
            Tool.PATROL: "Patrol route",
        }[self]

    @property
    def shortcut(self) -> str:
        return {
            Tool.POINTER: "S", Tool.BRUSH: "B", Tool.LINE: "L",
            Tool.RECTANGLE: "R", Tool.FILL: "F", Tool.ERASER: "E",
            Tool.EYEDROPPER: "I", Tool.PREFAB: "P", Tool.CAMERA: "K",
            Tool.PATROL: "T",
        }[self]


@dataclass
class Selection:
    """A rectangle of cells, in map coordinates."""

    x: int = 0
    y: int = 0
    width: int = 0
    height: int = 0

    @property
    def empty(self) -> bool:
        return self.width <= 0 or self.height <= 0

    def cells(self):
        for y in range(self.y, self.y + self.height):
            for x in range(self.x, self.x + self.width):
                yield x, y


class ToolController(QObject):
    """Turns canvas events into commands, one gesture at a time."""

    #: Emitted with a command the window should run.
    command_ready = Signal(object)
    #: A prefab could not go where it was clicked; carries the diagnostics.
    refused = Signal(object)
    #: The eyedropper found this raw value on this plane.
    picked = Signal(int, int)
    #: The selection changed.
    selection_changed = Signal(object)
    #: Something worth saying in the status bar.
    message = Signal(str)
    #: Where a snapshot should be taken from: tile center, in tile units. Not a
    #: document change -- the camera is a view of the map, not a thing in it.
    camera_placed = Signal(float, float)
    #: The patrol route being drawn (a `Route`, for the canvas to preview), or
    #: None when there is none.
    route_changed = Signal(object)

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self.tool = Tool.BRUSH
        self.entry: CatalogEntry | None = None
        self.prefab: Prefab | None = None
        #: Quarter turns applied to the selected prefab's footprint.
        self.prefab_rotation = 0
        self.document: MapDocument | None = None
        self.selection = Selection()
        self.filled_rectangle = False
        self._gesture = ""
        self._anchor: tuple[int, int] | None = None
        self._preview: list[tuple[int, int]] = []
        #: The catalog, for patrol routes (arrow words, which actors patrol).
        self.catalog = None
        #: The cells clicked so far for a patrol route.
        self.route: list[tuple[int, int]] = []

    # -- configuration ----------------------------------------------------

    def set_tool(self, tool: Tool) -> None:
        if tool is not self.tool:
            self.cancel_pending()
        self.tool = tool
        self._anchor = None
        self.message.emit(f"{tool.label} tool")

    def set_entry(self, entry: CatalogEntry | None) -> None:
        self.entry = entry

    def set_prefab(self, prefab: Prefab | None) -> None:
        self.prefab = prefab
        self.prefab_rotation = 0
        if prefab is not None:
            self.tool = Tool.PREFAB
            self.message.emit(f"{prefab.name}: {prefab.description}")

    def rotate_prefab(self) -> None:
        """Turn the pending prefab a quarter turn clockwise."""
        if self.prefab is None or not self.prefab.rotatable:
            self.message.emit("This one has no orientation to turn")
            return
        self.prefab_rotation = (self.prefab_rotation + 1) % 4
        self.message.emit(f"{self.prefab.name}, turned {self.prefab_rotation * 90} degrees")

    @property
    def oriented_prefab(self) -> Prefab | None:
        if self.prefab is None:
            return None
        return self.prefab.rotated(self.prefab_rotation)

    def _place_prefab(self, x: int, y: int) -> None:
        """One click, one compound structure, one undo step -- or a refusal.

        Refusing is the useful half. An exit with no floor in front of it is a
        wall nobody can use, and a door with no walls on its sides is a door
        the game draws wrong; finding that out at playtest is much worse than
        being told at the click. `placement_on` resolves what depends on the
        map, which is a door's axis.
        """
        prefab = self.oriented_prefab
        if prefab is None or self.document is None:
            return
        problems = prefab.check(self.document, x, y)
        if problems:
            self.refused.emit(problems)
            self.message.emit(problems[0].message)
            return
        command = write_words(self.document, prefab.placement_on(self.document, x, y),
                              label=prefab.name, gesture=self._gesture)
        if command.changes_anything:
            self.command_ready.emit(command)

    def cancel_pending(self) -> None:
        """Drop a patrol route half drawn: the only tool that takes more
        than one click. The window calls this whenever the tool changes."""
        if self.route:
            self.route = []
            self.route_changed.emit(None)

    # -- patrol routes ----------------------------------------------------

    def _route_click(self, x: int, y: int) -> None:
        """One click of a patrol route: start it, add a turn, or finish it.

        Clicking the last cell again (a double-click does) finishes the
        route there; clicking the first cell again closes the loop and
        finishes. A click the route cannot take is refused with the reason
        and leaves the route as it was.
        """
        if not self.route:
            route = plan_route(self.document, [(x, y)], self.catalog)
            if not route.ok:
                self.message.emit(route.reason)
                self.route_changed.emit(route)
                return
            self.route = [(x, y)]
            self.route_changed.emit(route)
            self.message.emit("Patrol route: click each turn. Double-click, Enter or the first "
                              "cell again to finish; Esc cancels")
            return
        if (x, y) == self.route[-1]:
            self.finish_route()
            return
        route = plan_route(self.document, self.route + [(x, y)], self.catalog)
        if not route.ok:
            self.message.emit(route.reason)
            self.route_changed.emit(route)
            return
        self.route.append((x, y))
        if route.closed:
            self.finish_route()
            return
        self.route_changed.emit(route)

    def hover(self, x: int, y: int) -> None:
        """Preview the next leg of a route as the pointer moves."""
        if self.tool is not Tool.PATROL or not self.route or self.document is None:
            return
        cells = list(self.route)
        if x >= 0 and (x, y) != cells[-1]:
            cells.append((x, y))
        self.route_changed.emit(plan_route(self.document, cells, self.catalog))

    def finish_route(self) -> None:
        """Place the route's arrows as one command (one undo step)."""
        if self.tool is not Tool.PATROL or not self.route or self.document is None:
            return
        route: Route = plan_route(self.document, self.route, self.catalog)
        if not route.complete:
            self.message.emit(route.reason if not route.ok
                              else "A route needs a second cell: click where it turns")
            return
        self.route = []
        self.route_changed.emit(None)
        command = write_words(self.document, route.writes, label="Patrol route",
                              gesture=uuid.uuid4().hex)
        if not command.changes_anything:
            self.message.emit("The route's arrows were already there")
            return
        self.command_ready.emit(command)
        arrows = len(route.arrows)
        text = f"Patrol route placed: {arrows} arrow{'s' if arrows != 1 else ''}"
        if route.closed:
            text += ", a closed loop"
        if route.turned:
            text += "; the enemy was turned to face the first leg"
        self.message.emit(text)

    def set_document(self, document: MapDocument | None) -> None:
        """Point at a map, or at a newer snapshot of the one already open.

        Only a *different* map abandons the gesture in progress. Documents are
        immutable, so every edit produces a new snapshot and the window hands
        it over mid-stroke; clearing the anchor on those refreshes ended every
        drag after its first cell.
        """
        changed = (
            document is None
            or self.document is None
            or document.uuid != self.document.uuid
        )
        self.document = document
        if changed:
            self._anchor = None
            self._gesture = ""
            self.cancel_pending()

    # -- what a tool writes -----------------------------------------------

    @property
    def target_plane(self) -> int:
        """The plane the current entry lives on. Zero when nothing is chosen."""
        return self.entry.plane if self.entry else 0

    def _value(self) -> int | None:
        if self.tool is Tool.ERASER:
            # Erasing a wall leaves floor: 144, area 4 with texture set 0.
            # Word 0 is not floor in Operation Body Count (floors are
            # 140-161). See MapDocument.DEFAULT_FLOOR.
            return EMPTY_OBJECT if self.target_plane == 1 else DEFAULT_FLOOR
        if self.entry is None:
            return None
        return self.entry.value

    def _writes(self, cells) -> list[tuple[int, int, int, int]]:
        value = self._value()
        if value is None or self.document is None:
            return []
        plane = self.target_plane
        return [(plane, x, y, value) for x, y in cells]

    def _emit(self, cells, label: str) -> None:
        writes = self._writes(cells)
        if not writes:
            return
        command = write_words(self.document, writes, label=label, gesture=self._gesture)
        if command.changes_anything:
            self.command_ready.emit(command)

    # -- events -----------------------------------------------------------

    def press(self, x: int, y: int, button: int) -> None:
        if self.document is None:
            return
        # A gesture id per stroke: the history coalesces commands that share
        # one, so a drag is a single undo step.
        self._gesture = uuid.uuid4().hex
        self._anchor = (x, y)

        if self.tool is Tool.EYEDROPPER:
            # Pick what is actually there, most specific first: an object if
            # the cell holds one, otherwise the wall or floor under it. Picking
            # from whichever plane the *previous* selection happened to use
            # would make the tool depend on invisible state.
            for plane in (1, 0):
                value = pick(self.document, plane, x, y)
                if value is None or (plane == 1 and value < 19):
                    continue
                self.picked.emit(plane, value)
                self.message.emit(f"Picked {value} from plane {plane}")
                return
            self.message.emit("Nothing to pick here")
            return

        if self.tool is Tool.POINTER:
            self.selection = Selection(x, y, 1, 1)
            self.selection_changed.emit(self.selection)
            return

        if self.tool is Tool.PREFAB:
            self._place_prefab(x, y)
            return

        if self.tool is Tool.PATROL:
            self._route_click(x, y)
            return

        if self.tool is Tool.CAMERA:
            # Placing the camera writes nothing: the snapshot is a view of the
            # map, not a thing in it, so this must not become an undo step or a
            # change to the document.
            self.camera_placed.emit(float(x) + 0.5, float(y) + 0.5)
            return

        if self.tool is Tool.FILL:
            cells, truncated = flood_cells(self.document, self.target_plane, x, y)
            self._emit(cells, "Fill")
            self.message.emit(
                f"Filled {len(cells)} cells"
                + (f" (stopped at the {FILL_BUDGET}-cell limit)" if truncated else "")
            )
            return

        if self.tool in (Tool.BRUSH, Tool.ERASER):
            self._emit([(x, y)], self.tool.label)

    def drag(self, x: int, y: int, button: int) -> None:
        if self.document is None or self._anchor is None:
            return

        if self.tool is Tool.POINTER:
            bounds = rectangle_bounds(*self._anchor, x, y)
            self.selection = Selection(*bounds)
            self.selection_changed.emit(self.selection)
            return

        if self.tool in (Tool.BRUSH, Tool.ERASER):
            # Straight from the last cell, so a fast drag does not leave gaps
            # where the pointer skipped between events.
            cells = line_cells(*self._anchor, x, y)
            self._emit(cells, self.tool.label)
            self._anchor = (x, y)

    def release(self, x: int, y: int) -> None:
        if self.document is None or self._anchor is None:
            self._gesture = ""
            return

        if self.tool is Tool.LINE and x >= 0:
            self._emit(line_cells(*self._anchor, x, y), "Line")
        elif self.tool is Tool.RECTANGLE and x >= 0:
            self._emit(
                rectangle_cells(*self._anchor, x, y, filled=self.filled_rectangle),
                "Rectangle",
            )
        self._anchor = None
        self._gesture = ""
