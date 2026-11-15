# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Edits as data: every change is a diff that can be applied or undone.

A command records the exact words it writes *and the exact words that were
there before*. That redundancy is deliberate. An undo that recomputed the old
value would be a second implementation of the edit, and the two would
eventually disagree -- most visibly on the operations where it matters, like a
flood fill that ran over a shape somebody else had already changed.

Because both halves are stored, undo is just applying the inverse, and the
inverse is the same structure with `before` and `after` swapped. There is one
apply path, so there is one thing to get right.

Gestures are how a drag becomes one undo step. A paint stroke submits a command
per cell as the pointer moves, all tagged with the same gesture id; the history
coalesces them, so Ctrl+Z takes back the stroke rather than the last pixel of
it. Coalescing only ever merges *adjacent* commands with the same gesture, so
it can never join two things the user thinks of as separate.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace

from .document import MapDocument, ProjectDocument
from .errors import Diagnostic, ObcEditError, Severity
from .names import NativeName
from .planes import MapPlanes, linear_index
from .resources import Resource

#: A history longer than this is memory nobody uses. Operation Body Count maps are small,
#: so the cap that binds first is the edit count, not the step count.
DEFAULT_STEP_CAP = 200
DEFAULT_EDIT_CAP = 2_000_000


class CommandError(ObcEditError):
    pass


def _command_error(message: str, where: str = "") -> CommandError:
    return CommandError(Diagnostic("OBC-SCHEMA-002", Severity.ERROR, message, where))


@dataclass(frozen=True)
class CellEdit:
    """One word, on one plane, of one map."""

    map_uuid: str
    plane: int
    index: int
    before: int
    after: int

    def inverted(self) -> "CellEdit":
        return CellEdit(self.map_uuid, self.plane, self.index, self.after, self.before)

    @property
    def changes_anything(self) -> bool:
        return self.before != self.after


@dataclass(frozen=True)
class NameEdit:
    """A rename, which replaces the whole 16-byte field."""

    map_uuid: str
    before: NativeName
    after: NativeName

    def inverted(self) -> "NameEdit":
        return NameEdit(self.map_uuid, self.after, self.before)

    @property
    def changes_anything(self) -> bool:
        return self.before.raw != self.after.raw


@dataclass(frozen=True)
class MapEdit:
    """A map added to or removed from the project.

    The whole document is kept, not just its id, because undoing a deletion
    has to put the map *back* -- every plane, its name, its source reference
    and its identity. Operation Body Count maps are twelve thousand words, so holding one
    is cheap, and the alternative is a delete you cannot take back.
    """

    document: "MapDocument"
    index: int
    added: bool

    def inverted(self) -> "MapEdit":
        return MapEdit(self.document, self.index, not self.added)

    @property
    def changes_anything(self) -> bool:
        return True


@dataclass(frozen=True)
class SlotEdit:
    """A map's target archive slot changed."""

    map_uuid: str
    before: int
    after: int

    def inverted(self) -> "SlotEdit":
        return SlotEdit(self.map_uuid, self.after, self.before)

    @property
    def changes_anything(self) -> bool:
        return self.before != self.after


@dataclass(frozen=True)
class ResourceEdit:
    """A replacement (art, sound) or music file added, changed or removed.

    Records only: the bytes stay in the project's content-addressed store, so
    undoing an import or a replace always finds the file it points back to.
    """

    before: Resource | None
    after: Resource | None

    def inverted(self) -> "ResourceEdit":
        return ResourceEdit(self.after, self.before)

    @property
    def key(self):
        return (self.after or self.before).key

    @property
    def changes_anything(self) -> bool:
        return self.before != self.after


@dataclass(frozen=True)
class CampaignEdit:
    """The campaign's JSON before and after, for edits that must undo together
    with something else (renaming a music file the floors play)."""

    before: dict
    after: dict

    def inverted(self) -> "CampaignEdit":
        return CampaignEdit(self.after, self.before)

    @property
    def changes_anything(self) -> bool:
        return self.before != self.after


@dataclass(frozen=True)
class Command:
    """A named, undoable change: a bag of edits and how to describe it."""

    label: str
    cells: tuple[CellEdit, ...] = ()
    names: tuple[NameEdit, ...] = ()
    slots: tuple[SlotEdit, ...] = ()
    maps: tuple[MapEdit, ...] = ()
    #: Adjacent commands sharing a gesture id coalesce into one undo step.
    gesture: str = ""
    resources: tuple[ResourceEdit, ...] = ()
    campaigns: tuple[CampaignEdit, ...] = ()

    def __len__(self) -> int:
        return (len(self.cells) + len(self.names) + len(self.slots) + len(self.maps)
                + len(self.resources) + len(self.campaigns))

    @property
    def changes_anything(self) -> bool:
        return any(edit.changes_anything
                   for edit in self.cells + self.names + self.slots + self.maps
                   + self.resources + self.campaigns)

    def inverted(self) -> "Command":
        return Command(
            label=self.label,
            cells=tuple(edit.inverted() for edit in reversed(self.cells)),
            names=tuple(edit.inverted() for edit in reversed(self.names)),
            slots=tuple(edit.inverted() for edit in reversed(self.slots)),
            maps=tuple(edit.inverted() for edit in reversed(self.maps)),
            gesture=self.gesture,
            resources=tuple(edit.inverted() for edit in reversed(self.resources)),
            campaigns=tuple(edit.inverted() for edit in reversed(self.campaigns)),
        )

    def merged_with(self, later: "Command") -> "Command":
        """Fold a later command of the same gesture into this one.

        Order matters: `later`'s edits happened after these, so they go last,
        and inverting the result unwinds them first. Cells written twice in one
        stroke are *not* collapsed -- the pairs still compose correctly, and
        collapsing them would need a second notion of what an edit means.
        """
        return Command(
            label=self.label,
            cells=self.cells + later.cells,
            names=self.names + later.names,
            slots=self.slots + later.slots,
            maps=self.maps + later.maps,
            gesture=self.gesture,
            resources=self.resources + later.resources,
            campaigns=self.campaigns + later.campaigns,
        )


def apply_command(project: ProjectDocument, command: Command) -> ProjectDocument:
    """Apply every edit, in order, and return the new document.

    Edits are grouped by map so a stroke over one map rebuilds its planes once
    rather than once per cell.
    """
    if not len(command):
        return project

    by_map: dict[str, list[CellEdit]] = {}
    for edit in command.cells:
        by_map.setdefault(edit.map_uuid, []).append(edit)

    maps = list(project.maps)
    index_of = {document.uuid: index for index, document in enumerate(maps)}

    for map_uuid, edits in by_map.items():
        if map_uuid not in index_of:
            raise _command_error(f"command edits map {map_uuid}, which is not open", map_uuid)
        document = maps[index_of[map_uuid]]
        planes = [list(plane) for plane in document.planes.planes]
        cells = document.planes.cell_count
        for edit in edits:
            if not 0 <= edit.plane < len(planes):
                raise _command_error(f"plane {edit.plane} does not exist", map_uuid)
            if not 0 <= edit.index < cells:
                raise _command_error(
                    f"cell {edit.index} is outside {document.width}x{document.height}", map_uuid
                )
            if not 0 <= edit.after <= 0xFFFF:
                raise CommandError(
                    Diagnostic(
                        "OBC-CELL-001",
                        Severity.ERROR,
                        f"value {edit.after} is outside 0..65535",
                        map_uuid,
                    )
                )
            planes[edit.plane][edit.index] = edit.after
        maps[index_of[map_uuid]] = document.with_planes(
            MapPlanes(document.width, document.height, tuple(tuple(p) for p in planes))
        )

    for edit in command.names:
        if edit.map_uuid not in index_of:
            raise _command_error(f"command renames map {edit.map_uuid}, which is not open")
        position = index_of[edit.map_uuid]
        maps[position] = replace(maps[position], native_name=edit.after)

    for edit in command.slots:
        if edit.map_uuid not in index_of:
            raise _command_error(f"command reslots map {edit.map_uuid}, which is not open")
        maps[index_of[edit.map_uuid]] = replace(
            maps[index_of[edit.map_uuid]], slot=edit.after
        )

    # Structural changes last, so a command that edits a map and then removes
    # it does both in the order it was written -- and so the indices the cell
    # edits used are still valid when they run.
    for edit in command.maps:
        if edit.added:
            if any(existing.uuid == edit.document.uuid for existing in maps):
                raise _command_error(f"map {edit.document.uuid} is already in the project")
            maps.insert(min(edit.index, len(maps)), edit.document)
        else:
            position = next(
                (i for i, existing in enumerate(maps)
                 if existing.uuid == edit.document.uuid), None
            )
            if position is None:
                raise _command_error(f"map {edit.document.uuid} is not in the project")
            del maps[position]

    if not command.resources and not command.campaigns:
        return project.with_maps(maps)
    campaign = project.campaign
    for edit in command.campaigns:
        if campaign != edit.before:
            raise _command_error("the campaign is not what this edit expects")
        campaign = edit.after
    records = {resource.key: resource for resource in project.resources}
    for edit in command.resources:
        current = records.get(edit.key)
        if current != edit.before:
            raise _command_error(f"{edit.key[0]} {edit.key[1]} is not what this edit expects")
        if edit.after is None:
            records.pop(edit.key, None)
        else:
            records[edit.key] = edit.after
    return replace(project, maps=tuple(maps), campaign=dict(campaign)).with_resources(
        records.values())


# ---------------------------------------------------------------------------
# Building commands
# ---------------------------------------------------------------------------


def paint_cells(document: MapDocument, plane: int, cells, value: int, *,
                label: str = "Paint", gesture: str = "") -> Command:
    """Write one value into a set of `(x, y)` cells, skipping the no-ops.

    Dropping unchanged cells is what makes a drag across already-painted floor
    produce nothing to undo, which is what a user expects.
    """
    edits = []
    for x, y in cells:
        if not (0 <= x < document.width and 0 <= y < document.height):
            continue
        index = linear_index(x, y, document.width)
        before = document.planes.planes[plane][index]
        if before != value:
            edits.append(CellEdit(document.uuid, plane, index, before, value))
    return Command(label, tuple(edits), gesture=gesture)


def write_words(document: MapDocument, writes, *, label: str = "Place",
                gesture: str = "") -> Command:
    """Write `(plane, x, y, value)` tuples: the general compound placement.

    A door is a plane-0 word; an alien facing east is a plane-1 word; a
    transporter is both a zone and a trigger. Anything the catalog describes
    as a compound write comes through here as one command, so it is one undo.
    """
    edits = []
    for plane, x, y, value in writes:
        if not (0 <= x < document.width and 0 <= y < document.height):
            continue
        index = linear_index(x, y, document.width)
        before = document.planes.planes[plane][index]
        if before != value:
            edits.append(CellEdit(document.uuid, plane, index, before, value))
    return Command(label, tuple(edits), gesture=gesture)


def set_resource(project: ProjectDocument, kind: str, target, resource: Resource | None, *,
                 label: str = "") -> Command:
    """Replace (or with None, revert) one wall page, sprite, sound or music file."""
    before = project.resource(kind, target)
    wanted = (kind, str(target).lower() if kind == "music" else str(target))
    if resource is not None and resource.key != wanted:
        raise _command_error(f"{resource.kind} {resource.target} cannot replace {kind} {target}")
    what = f"{kind} {target}"
    label = label or (f"Revert {what}" if resource is None
                      else f"Replace {what}" if before else f"Import {what}")
    return Command(label, resources=(ResourceEdit(before, resource),))


def rename_music(project: ProjectDocument, old: str, new: str) -> Command:
    """A music file under another name: one step, so undo restores both."""
    from dataclasses import replace as _replace

    current = project.resource("music", old)
    if current is None:
        raise _command_error(f"there is no music file {old}")
    if project.resource("music", new) is not None and new.lower() != old.lower():
        raise _command_error(f"a music file named {new} already exists")
    renamed = _replace(current, target=new)
    campaigns = ()
    if project.campaign:
        after = _renamed_music(project.campaign, old, new)
        if after != project.campaign:
            campaigns = (CampaignEdit(project.campaign, after),)
    return Command(f"Rename {old}", resources=(ResourceEdit(current, None),
                                               ResourceEdit(None, renamed)),
                   campaigns=campaigns)


def _renamed_music(campaign: dict, old: str, new: str) -> dict:
    """The campaign JSON with every floor and arena that played `old` playing `new`."""
    import copy

    after = copy.deepcopy(campaign)
    for key in ("levels", "arenas"):
        for entry in after.get(key, []) or []:
            if isinstance(entry, dict) and isinstance(entry.get("music"), str) \
                    and entry["music"].lower() == old.lower():
                entry["music"] = new
    return after


def remove_map(project: ProjectDocument, uuid: str, *, label: str = "Delete map") -> Command:
    """Take a map out of the project, undoably."""
    index = project.index_of(uuid)
    return Command(label, maps=(MapEdit(project.maps[index], index, added=False),))


def add_map(document: MapDocument, index: int, *, label: str = "Add map") -> Command:
    return Command(label, maps=(MapEdit(document, index, added=True),))


def add_maps(documents, *, index: int | None = None,
             label: str = "Add maps") -> Command:
    """Several maps as one command.

    An import of sixty maps that lands as sixty undo steps is one nobody can
    back out of, so the whole batch is a single edit. `index` is where the
    first one goes; the rest follow it, which keeps the archive's own order.
    """
    documents = list(documents)
    start = 0 if index is None else index
    return Command(label, maps=tuple(
        MapEdit(document, start + offset, added=True)
        for offset, document in enumerate(documents)))


def set_slot(document: MapDocument, slot: int, *, label: str = "Change slot") -> Command:
    return Command(label, slots=(SlotEdit(document.uuid, document.slot, slot),))


def rename_map(document: MapDocument, text: str, *, label: str = "Rename") -> Command:
    """A rename. `NativeName.from_text` refuses anything unencodable."""
    return Command(label, names=(NameEdit(document.uuid, document.native_name,
                                          NativeName.from_text(text)),))


# ---------------------------------------------------------------------------
# History
# ---------------------------------------------------------------------------


@dataclass
class History:
    """Undo and redo stacks, bounded, with gesture coalescing.

    The caps are two: how many steps to keep, and how many individual cell
    edits across all of them. The second is what actually protects memory -- a
    single flood fill on a 64x64 map is four thousand edits, so twenty of them
    is a bigger number than two hundred small steps.
    """

    step_cap: int = DEFAULT_STEP_CAP
    edit_cap: int = DEFAULT_EDIT_CAP
    _undo: list[Command] = field(default_factory=list)
    _redo: list[Command] = field(default_factory=list)
    #: Edits dropped to stay inside the caps, so a UI can say "older steps
    #: were discarded" rather than silently losing them.
    dropped_steps: int = 0

    @property
    def can_undo(self) -> bool:
        return bool(self._undo)

    @property
    def can_redo(self) -> bool:
        return bool(self._redo)

    @property
    def undo_label(self) -> str:
        return self._undo[-1].label if self._undo else ""

    @property
    def redo_label(self) -> str:
        return self._redo[-1].label if self._redo else ""

    @property
    def depth(self) -> int:
        return len(self._undo)

    @property
    def edit_count(self) -> int:
        return sum(len(command) for command in self._undo)

    def do(self, project: ProjectDocument, command: Command) -> ProjectDocument:
        """Apply a command and record it.

        A command that changes nothing is not recorded: an undo step that does
        nothing when you press Ctrl+Z is worse than no step at all.
        """
        if not command.changes_anything:
            return project

        result = apply_command(project, command)
        # Redo is only reachable by undoing; doing something new abandons it.
        self._redo.clear()

        if (
            command.gesture
            and self._undo
            and self._undo[-1].gesture == command.gesture
        ):
            self._undo[-1] = self._undo[-1].merged_with(command)
        else:
            self._undo.append(command)

        self._enforce_caps()
        return result

    def undo(self, project: ProjectDocument) -> ProjectDocument:
        if not self._undo:
            return project
        command = self._undo.pop()
        result = apply_command(project, command.inverted())
        self._redo.append(command)
        return result

    def redo(self, project: ProjectDocument) -> ProjectDocument:
        if not self._redo:
            return project
        command = self._redo.pop()
        result = apply_command(project, command)
        self._undo.append(command)
        return result

    def end_gesture(self) -> None:
        """Close the current stroke so the next command starts a new step."""
        if self._undo:
            self._undo[-1] = replace(self._undo[-1], gesture="")

    def clear(self) -> None:
        self._undo.clear()
        self._redo.clear()
        self.dropped_steps = 0

    def _enforce_caps(self) -> None:
        while len(self._undo) > self.step_cap:
            del self._undo[0]
            self.dropped_steps += 1
        while self.edit_count > self.edit_cap and len(self._undo) > 1:
            del self._undo[0]
            self.dropped_steps += 1


class Transaction:
    """Collect several commands and commit them as one undo step.

    Used where one user action means several writes that must undo together --
    placing a transporter pair, stamping a prefab, importing a map. On an
    exception nothing is committed, because the document was never mutated:
    the transaction only ever built a list.
    """

    def __init__(self, history: History, label: str, *, gesture: str = "") -> None:
        self.history = history
        self.label = label
        self.gesture = gesture
        self._commands: list[Command] = []

    def add(self, command: Command) -> None:
        self._commands.append(command)

    def __enter__(self) -> "Transaction":
        return self

    def __exit__(self, kind, value, traceback) -> bool:
        return False

    def commit(self, project: ProjectDocument) -> ProjectDocument:
        cells: tuple[CellEdit, ...] = ()
        names: tuple[NameEdit, ...] = ()
        slots: tuple[SlotEdit, ...] = ()
        maps: tuple[MapEdit, ...] = ()
        for command in self._commands:
            cells += command.cells
            names += command.names
            slots += command.slots
            maps += command.maps
        return self.history.do(
            project, Command(self.label, cells, names, slots, maps, gesture=self.gesture)
        )
