# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The campaign editor: a mod's identity and its floors, in order.

A mod (docs/MODS_AND_OBCEDIT.md section 3) is a list of floors played one
after another: each is one of this project's maps or an original floor by
reference, takes the behaviour of an original floor (`like`: darkness, the
sewer look, colours, music, the sequence after it) and may override any of
those. Finishing the last floor wins the campaign.

A table: the questions an author has -- which floor comes
first, what each one behaves like, whether the whole thing can be built --
are answered by reading down a column. Everything is checked by
`campaign.validate` as it is typed; the panel at the bottom is that
function's output, and OK stays available while only warnings stand.

"From like" in a column means the field is left out of `mod.toml`, so the
floor keeps what its `like` floor does.
"""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QAbstractItemView, QComboBox, QDialog, QDialogButtonBox, QFormLayout, QHBoxLayout,
    QHeaderView, QLabel, QLineEdit, QPlainTextEdit, QPushButton, QSpinBox, QTableWidget,
    QVBoxLayout,
)

from obcedit_core.campaign import (
    DEFAULT_LIKE, MAX_AUTHOR, MAX_ID, MAX_LEVEL_NAME, MAX_MESSAGE,
    MAX_TITLE, MAX_VERSION, RELEASES, SEQUENCES, STOCK_FLOORS, Campaign, LevelEntry, validate,
)
from obcedit_core.errors import Severity


def original_floor_name(slot: int) -> str:
    """What original floor `slot` (0-39) is, as the game's map names say."""
    if slot < 5:
        return f"Sewers {slot + 1}"
    if slot == 5:
        return "Maintenance level"
    if slot == 6:
        return "UN Bldg 1 lobby"
    if slot < 22:
        return f"UN Bldg 1 floor {slot - 5}"
    if slot == 22:
        return "UN Bldg 1 top"
    if slot < 39:
        return f"UN Bldg 2 floor {slot - 22}"
    return "UN Bldg 2 top (Victor)"


def _floor_label(slot: int) -> str:
    return f"{slot + 1}: {original_floor_name(slot)}"


#: Tri-state for booleans: (label, value).
_BOOLS = (("from like", None), ("yes", True), ("no", False))


class CampaignDialog(QDialog):
    """Edit a project's mod campaign. Returns the new one through `campaign()`."""

    COLUMNS = ("Floor", "Name", "Like", "Music", "Dark", "Sewer", "Colours",
               "Sequence", "Message", "Exit")

    def __init__(self, campaign: Campaign, documents, parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Campaign")
        self.setObjectName("campaign-dialog")
        self._documents = list(documents)
        self._base = campaign

        layout = QVBoxLayout(self)
        form = QFormLayout()
        self.mod_id = QLineEdit(campaign.mod_id)
        self.mod_id.setMaxLength(MAX_ID)
        self.mod_id.setObjectName("campaign-id")
        self.mod_id.setPlaceholderText("lower-case letters, digits and '-'")
        self.title = QLineEdit(campaign.title)
        self.title.setMaxLength(MAX_TITLE)
        self.title.setObjectName("campaign-title")
        self.author = QLineEdit(campaign.author)
        self.author.setMaxLength(MAX_AUTHOR)
        self.author.setObjectName("campaign-author")
        self.version = QLineEdit(campaign.version)
        self.version.setMaxLength(MAX_VERSION)
        self.version.setObjectName("campaign-version")
        self.release = QComboBox()
        self.release.setObjectName("campaign-release")
        for value in RELEASES:
            self.release.addItem({"any": "Either release", "OBC-A": "Floppy release only",
                                  "OBC-CD": "CD release only"}[value], value)
        self.release.setCurrentIndex(max(0, self.release.findData(campaign.release)))
        self.description = QPlainTextEdit(campaign.description)
        self.description.setObjectName("campaign-description")
        self.description.setFixedHeight(54)
        form.addRow("Mod id", self.mod_id)
        form.addRow("Title", self.title)
        form.addRow("Author", self.author)
        form.addRow("Version", self.version)
        form.addRow("Plays on", self.release)
        form.addRow("Description", self.description)
        layout.addLayout(form)

        self.table = QTableWidget(0, len(self.COLUMNS), self)
        self.table.setObjectName("campaign-levels")
        self.table.setHorizontalHeaderLabels(self.COLUMNS)
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.verticalHeader().setVisible(True)
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(QHeaderView.ResizeMode.Interactive)
        header.setSectionResizeMode(8, QHeaderView.ResizeMode.Stretch)
        for column, width in enumerate((230, 130, 190, 100, 90, 90, 90, 90, 150, 110)):
            self.table.setColumnWidth(column, width)
        layout.addWidget(self.table, 1)

        buttons = QHBoxLayout()
        self.add = QPushButton("Add one of your maps")
        self.add.setObjectName("campaign-add")
        self.add_stock = QPushButton("Add an original floor")
        self.add_stock.setObjectName("campaign-add-stock")
        self.remove = QPushButton("Remove")
        self.remove.setObjectName("campaign-remove")
        self.up = QPushButton("Move up")
        self.up.setObjectName("campaign-up")
        self.down = QPushButton("Move down")
        self.down.setObjectName("campaign-down")
        for widget in (self.add, self.add_stock, self.remove, self.up, self.down):
            buttons.addWidget(widget)
        buttons.addStretch(1)
        self.start = QLabel()
        self.start.setObjectName("campaign-start")
        buttons.addWidget(self.start)
        layout.addLayout(buttons)

        self.problems = QLabel()
        self.problems.setObjectName("campaign-problems")
        self.problems.setWordWrap(True)
        self.problems.setTextFormat(Qt.TextFormat.PlainText)
        layout.addWidget(self.problems)

        box = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok
                               | QDialogButtonBox.StandardButton.Cancel)
        box.accepted.connect(self.accept)
        box.rejected.connect(self.reject)
        self.ok = box.button(QDialogButtonBox.StandardButton.Ok)
        layout.addWidget(box)

        self.add.clicked.connect(lambda _checked=False: self._add_map())
        self.add_stock.clicked.connect(lambda _checked=False: self._add_stock())
        self.remove.clicked.connect(lambda _checked=False: self._remove_level())
        self.up.clicked.connect(lambda _checked=False: self._move(-1))
        self.down.clicked.connect(lambda _checked=False: self._move(1))
        for field in (self.mod_id, self.title, self.author, self.version):
            field.textChanged.connect(lambda _text="": self._revalidate())
        self.description.textChanged.connect(self._revalidate)
        self.release.currentIndexChanged.connect(lambda _i=0: self._revalidate())

        for level in campaign.levels:
            self._append_row(level)
        if not campaign.levels:
            for document in self._documents:
                self._append_row(LevelEntry(map_uuid=document.uuid,
                                            name=document.name[:MAX_LEVEL_NAME]))
        self._revalidate()
        self.resize(1360, 560)

    # -- rows ---------------------------------------------------------------

    def _combo(self, items, current) -> QComboBox:
        box = QComboBox()
        for label, value in items:
            box.addItem(label, value)
        index = next((i for i, (_, value) in enumerate(items) if value == current), 0)
        box.setCurrentIndex(index)
        box.currentIndexChanged.connect(lambda _i=0: self._revalidate())
        return box

    def _source_items(self, level: LevelEntry):
        items = [(f"Your map: {d.name or d.label}", f"map:{d.uuid}") for d in self._documents]
        if level.map_uuid and not any(d.uuid == level.map_uuid for d in self._documents):
            # A campaign can name a map the project no longer has. Keeping the row
            # and letting the validator say so beats dropping a floor silently.
            items.append(("(a map that is no longer in the project)", f"map:{level.map_uuid}"))
        items += [(f"Original floor {_floor_label(s)}", f"stock:{s}") for s in range(STOCK_FLOORS)]
        return items

    def _append_row(self, level: LevelEntry) -> None:
        row = self.table.rowCount()
        self.table.insertRow(row)
        current = f"map:{level.map_uuid}" if level.map_uuid else f"stock:{level.stock or 0}"
        self.table.setCellWidget(row, 0, self._combo(self._source_items(level), current))

        name = QLineEdit(level.name)
        name.setMaxLength(MAX_LEVEL_NAME)
        name.textChanged.connect(lambda _text="": self._revalidate())
        self.table.setCellWidget(row, 1, name)

        like_items = [(f"(default: {_floor_label(DEFAULT_LIKE)})", None)]
        like_items += [(_floor_label(s), s) for s in range(STOCK_FLOORS)]
        self.table.setCellWidget(row, 2, self._combo(like_items, level.like))

        music = [("from like", None), ("random", "random")] + [(f"song {n}", n) for n in range(10)]
        self.table.setCellWidget(row, 3, self._combo(music, level.music))
        self.table.setCellWidget(row, 4, self._combo(_BOOLS, level.dark))
        self.table.setCellWidget(row, 5, self._combo(_BOOLS, level.sewer))

        colors = QSpinBox()
        colors.setRange(-1, 50)
        colors.setSpecialValueText("from like")
        colors.setValue(-1 if level.colors is None else level.colors)
        colors.setToolTip("Which original floor's roof and floor colours to use (0-50)")
        colors.valueChanged.connect(lambda _v=0: self._revalidate())
        self.table.setCellWidget(row, 6, colors)

        sequences = [("from like", None)] + [(s, s) for s in SEQUENCES]
        self.table.setCellWidget(row, 7, self._combo(sequences, level.sequence))

        message = QLineEdit(level.message or "")
        message.setMaxLength(MAX_MESSAGE)
        message.setPlaceholderText("from like")
        message.textChanged.connect(lambda _text="": self._revalidate())
        self.table.setCellWidget(row, 8, message)

        exits = (("from like", None), ("normal", "normal"),
                 ("none: only Victor ends it", "none"))
        self.table.setCellWidget(row, 9, self._combo(exits, level.exit))

    def _add_map(self) -> None:
        used = {level.map_uuid for level in self._levels()}
        spare = [d for d in self._documents if d.uuid not in used] or self._documents
        if not spare:
            return
        self._append_row(LevelEntry(map_uuid=spare[0].uuid, name=spare[0].name[:MAX_LEVEL_NAME]))
        self._revalidate()

    def _add_stock(self) -> None:
        self._append_row(LevelEntry(stock=0))
        self._revalidate()

    def _remove_level(self) -> None:
        row = self.table.currentRow()
        if row >= 0:
            self.table.removeRow(row)
            self._revalidate()

    def _move(self, delta: int) -> None:
        """Reorder by rebuilding the table: swapping cell widgets loses their
        connections, and reading the rows out and back in cannot half-work."""
        row = self.table.currentRow()
        target = row + delta
        if row < 0 or not 0 <= target < self.table.rowCount():
            return
        levels = list(self._levels())
        levels[row], levels[target] = levels[target], levels[row]
        self.table.setRowCount(0)
        for level in levels:
            self._append_row(level)
        self.table.setCurrentCell(target, 0)
        self._revalidate()

    # -- reading the table back --------------------------------------------

    def _levels(self):
        for row in range(self.table.rowCount()):
            cell = lambda column, r=row: self.table.cellWidget(r, column)  # noqa: E731
            source = cell(0).currentData() or ""
            kind, _, value = source.partition(":")
            message = cell(8).text()
            colors = cell(6).value()
            yield LevelEntry(
                map_uuid=value if kind == "map" else "",
                stock=int(value) if kind == "stock" else None,
                name=cell(1).text(),
                like=cell(2).currentData(),
                music=cell(3).currentData(),
                dark=cell(4).currentData(),
                sewer=cell(5).currentData(),
                colors=None if colors < 0 else colors,
                sequence=cell(7).currentData(),
                message=message if message else None,
                exit=cell(9).currentData(),
            )

    def campaign(self) -> Campaign:
        from dataclasses import replace

        return replace(
            self._base,
            mod_id=self.mod_id.text().strip(), title=self.title.text(),
            author=self.author.text(), version=self.version.text().strip(),
            description=" ".join(self.description.toPlainText().split()),
            release=self.release.currentData(), levels=tuple(self._levels()),
        )

    # -- the panel ----------------------------------------------------------

    def _revalidate(self) -> None:
        campaign = self.campaign()
        count = len(campaign.levels)
        self.start.setText(f"{count} floor{'s' if count != 1 else ''}; the first row is floor 1"
                           if count else "No floors")
        problems = validate(campaign, self._documents)
        errors = [p for p in problems if p.severity >= Severity.ERROR]
        warnings = [p for p in problems if p.severity == Severity.WARNING]
        lines = [f"{p.severity.name.lower()}: {p.message}" + (f" ({p.where})" if p.where else "")
                 for p in errors + warnings]
        self.problems.setText("\n".join(lines[:6]) if lines else "This mod is ready to build.")
        self.ok.setEnabled(not errors)
