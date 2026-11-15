# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The Resources panel: the project's own art, sounds and music.

Mod format 2 (docs/MOD_ASSETS_AND_ARENAS.md section 4.3). Four tabs:

* **Walls** and **Sprites**: every wall page (0-135; floors and ceilings are
  wall pages too) and every sprite (0-679), with its thumbnail and a name
  from the catalog. A replaced one shows the replacement, converted to the
  game palette by OBCR's rule, and is marked. Import / Replace, Revert, and
  Save a copy of *the author's own* file -- never the game's.
* **Sounds**: the 99 digitized sounds by number, with what the engine plays
  them for (soundnames.py). Import / Revert.
* **Music**: the project's music files and where each is played. Import (a
  WAV is converted to QOA by OBCR), Rename, Remove.

Every change is one undoable command (`commands.set_resource`) through the
window's `run_command`, so Ctrl+Z takes back an import like a brush stroke.
Sounds and music are not played here (Qt Multimedia is not in the package):
F5 plays the map in OBCR, with them.
"""

from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QSize, Qt
from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import (
    QFileDialog,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QListView,
    QListWidget,
    QListWidgetItem,
    QMessageBox,
    QPushButton,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from obcedit_core.campaign import Campaign, music_file
from obcedit_core.catalog import CatalogEntry
from obcedit_core.commands import rename_music, set_resource
from obcedit_core.errors import ObcEditError
from obcedit_core.importing import import_art, import_music, import_sound
from obcedit_core.resources import (
    MUSIC,
    SOUND,
    SOUNDS,
    SPRITE,
    SPRITES,
    WALL,
    WALL_PAGES,
    is_music_name,
)
from obcedit_core.soundnames import sound_name

from .palette_models import CatalogModel, EntryRole

#: Marks a replaced page or sprite in the grids.
REPLACED = "★ "


def page_names(catalog) -> dict[int, str]:
    """Wall page -> what uses it, from the catalog (walls, doors, floor sets)."""
    names: dict[int, str] = {}
    if catalog is None:
        return names
    for entry in catalog:
        if entry.plane == 0 and entry.page is not None and int(entry.page) not in names:
            names[int(entry.page)] = entry.name
    for floor_set in catalog.floor_sets:
        number = floor_set.get("set")
        for key, what in (("floor_page", "floor"), ("ceiling_page", "ceiling"),
                          ("sewer_floor_page", "sewer floor"),
                          ("sewer_ceiling_page", "sewer ceiling")):
            page = floor_set.get(key)
            if page is not None and int(page) not in names:
                names[int(page)] = f"Floor set {number} {what}"
    return names


def sprite_names(catalog) -> dict[int, str]:
    """Sprite -> the first catalog entry that draws with it."""
    names: dict[int, str] = {}
    if catalog is None:
        return names
    for entry in catalog:
        if entry.sprite is not None and int(entry.sprite) not in names:
            names[int(entry.sprite)] = entry.name
    return names


def art_entry(kind: str, number: int, name: str) -> CatalogEntry:
    """A palette-style entry for one page or sprite, for the thumbnail grid."""
    if kind == WALL:
        return CatalogEntry(key=f"resource-wall-{number}", category="resources",
                            subcategory="walls", rank=number, name=name, plane=0,
                            value=number + 1, values=(), page=number)
    return CatalogEntry(key=f"resource-sprite-{number}", category="resources",
                        subcategory="sprites", rank=number, name=name, plane=1, value=number,
                        values=(), sprite=number)


class ResourcesPanel(QWidget):
    """The dock's contents. `window` is the MainWindow it edits through."""

    def __init__(self, window) -> None:
        super().__init__(window)
        self.window = window
        self.setObjectName("resources-panel")
        layout = QVBoxLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)
        self.tabs = QTabWidget(self)
        self.tabs.setObjectName("resources-tabs")
        layout.addWidget(self.tabs, 1)

        catalog = window.catalog
        self._names = {WALL: page_names(catalog), SPRITE: sprite_names(catalog)}
        self.views: dict[str, QListView] = {}
        self.models: dict[str, CatalogModel] = {}
        for kind, count, title in ((WALL, WALL_PAGES, "Walls"), (SPRITE, SPRITES, "Sprites")):
            view = QListView(self.tabs)
            view.setObjectName(f"resources-{kind}s")
            view.setViewMode(QListView.IconMode)
            view.setResizeMode(QListView.Adjust)
            view.setIconSize(QSize(48, 48))
            view.setSpacing(4)
            view.setUniformItemSizes(True)
            view.setAccessibleName(f"{title} to replace")
            model = CatalogModel(factory=window.thumbnails, pool=window.pool, parent=self)
            view.setModel(model)
            view.selectionModel().currentChanged.connect(lambda *_: self._show_selection())
            view.doubleClicked.connect(lambda _index: self.import_selected())
            self.views[kind] = view
            self.models[kind] = model
            self.tabs.addTab(view, title)

        self.sounds = QListWidget(self.tabs)
        self.sounds.setObjectName("resources-sounds")
        self.sounds.currentRowChanged.connect(lambda _row: self._show_selection())
        self.sounds.itemDoubleClicked.connect(lambda _item: self.import_selected())
        self.tabs.addTab(self.sounds, "Sounds")
        self.music = QListWidget(self.tabs)
        self.music.setObjectName("resources-music")
        self.music.currentRowChanged.connect(lambda _row: self._show_selection())
        self.tabs.addTab(self.music, "Music")
        self.tabs.currentChanged.connect(lambda _index: self._show_selection())

        detail = QHBoxLayout()
        self.preview = QLabel(self)
        self.preview.setObjectName("resources-preview")
        self.preview.setFixedSize(96, 96)
        self.preview.setAlignment(Qt.AlignCenter)
        self.info = QLabel("Choose a wall page, sprite, sound or music file", self)
        self.info.setObjectName("resources-info")
        self.info.setWordWrap(True)
        self.info.setTextFormat(Qt.TextFormat.PlainText)
        detail.addWidget(self.preview)
        detail.addWidget(self.info, 1)
        layout.addLayout(detail)

        buttons = QHBoxLayout()
        self.import_button = QPushButton("Import…", self)
        self.import_button.setObjectName("resources-import")
        self.revert_button = QPushButton("Revert", self)
        self.revert_button.setObjectName("resources-revert")
        self.rename_button = QPushButton("Rename…", self)
        self.rename_button.setObjectName("resources-rename")
        self.copy_button = QPushButton("Save a copy…", self)
        self.copy_button.setObjectName("resources-save-copy")
        self.copy_button.setToolTip("Save your imported file somewhere else")
        for button in (self.import_button, self.revert_button, self.rename_button,
                       self.copy_button):
            buttons.addWidget(button)
        buttons.addStretch(1)
        layout.addLayout(buttons)
        self.import_button.clicked.connect(lambda _checked=False: self.import_selected())
        self.revert_button.clicked.connect(lambda _checked=False: self.revert_selected())
        self.rename_button.clicked.connect(lambda _checked=False: self.rename_selected())
        self.copy_button.clicked.connect(lambda _checked=False: self.save_copy_selected())

        self.refresh(rebuild=True)

    # -- contents ---------------------------------------------------------

    @property
    def project(self):
        return self.window.project

    def _name(self, kind: str, number: int) -> str:
        default = f"Wall page {number}" if kind == WALL else f"Sprite {number}"
        return f"{number}: {self._names[kind].get(number, default)}"

    def refresh(self, *, rebuild: bool = False) -> None:
        """Show the project's records: markers, sounds and music lists."""
        replaced = {(r.kind, r.target) for r in self.project.resources}
        for kind, count in ((WALL, WALL_PAGES), (SPRITE, SPRITES)):
            entries = [art_entry(kind, n, (REPLACED if (kind, n) in replaced else "")
                                 + self._name(kind, n)) for n in range(count)]
            model = self.models[kind]
            view = self.views[kind]
            row = view.currentIndex().row()
            model.set_entries(entries)
            model.invalidate_icons()
            if row >= 0:
                view.setCurrentIndex(model.index(row, 0))
        self._fill_sounds()
        self._fill_music()
        self._show_selection()

    def _fill_sounds(self) -> None:
        row = self.sounds.currentRow()
        self.sounds.clear()
        for number in range(SOUNDS):
            resource = self.project.resource(SOUND, number)
            text = f"{number:02d}  {sound_name(number)}"
            if resource is not None:
                text = REPLACED + text + f"  ({resource.source_name or 'replaced'})"
            item = QListWidgetItem(text)
            item.setData(Qt.UserRole, number)
            self.sounds.addItem(item)
        if row >= 0:
            self.sounds.setCurrentRow(row)

    def music_uses(self) -> dict[str, list[str]]:
        """Music name (lower case) -> the floors and arenas that play it."""
        uses: dict[str, list[str]] = {}
        try:
            campaign = Campaign.from_json(self.project.campaign) if self.project.campaign \
                else Campaign()
        except (TypeError, ValueError):
            return uses
        for index, level in enumerate(campaign.levels):
            name = music_file(level.music)
            if name:
                uses.setdefault(name.lower(), []).append(f"floor {index + 1}")
        for index, arena in enumerate(campaign.arenas):
            name = music_file(arena.music)
            if name:
                uses.setdefault(name.lower(), []).append(f"arena {index + 1}")
        return uses

    def _fill_music(self) -> None:
        current = self.music.currentItem()
        selected = current.data(Qt.UserRole) if current else None
        self.music.clear()
        uses = self.music_uses()
        for resource in self.project.resources:
            if resource.kind != MUSIC:
                continue
            name = str(resource.target)
            where = uses.get(name.lower())
            text = f"{name}  ({resource.size // 1024} KiB) — " + (
                "played on " + ", ".join(where) if where else "not used yet (Campaign)")
            item = QListWidgetItem(text)
            item.setData(Qt.UserRole, name)
            self.music.addItem(item)
            if name == selected:
                self.music.setCurrentItem(item)

    # -- selection --------------------------------------------------------

    def selected(self):
        """`(kind, target)` of the selection, `(MUSIC, None)` on an empty music tab."""
        page = self.tabs.currentIndex()
        if page in (0, 1):
            kind = (WALL, SPRITE)[page]
            index = self.views[kind].currentIndex()
            entry = index.data(EntryRole) if index.isValid() else None
            if entry is None:
                return kind, None
            return kind, int(entry.page if kind == WALL else entry.sprite)
        if page == 2:
            item = self.sounds.currentItem()
            return SOUND, (item.data(Qt.UserRole) if item else None)
        item = self.music.currentItem()
        return MUSIC, (item.data(Qt.UserRole) if item else None)

    def select(self, kind: str, target) -> None:
        """Select something (also used by tests)."""
        page = {WALL: 0, SPRITE: 1, SOUND: 2, MUSIC: 3}[kind]
        self.tabs.setCurrentIndex(page)
        if kind in (WALL, SPRITE):
            self.views[kind].setCurrentIndex(self.models[kind].index(int(target), 0))
        elif kind == SOUND:
            self.sounds.setCurrentRow(int(target))
        else:
            for row in range(self.music.count()):
                if self.music.item(row).data(Qt.UserRole).lower() == str(target).lower():
                    self.music.setCurrentRow(row)
        self._show_selection()

    def _show_selection(self) -> None:
        kind, target = self.selected()
        resource = self.project.resource(kind, target) if target is not None else None
        self.import_button.setText("Import…" if kind == MUSIC or resource is None
                                   else "Replace…")
        self.import_button.setEnabled(kind == MUSIC or target is not None)
        self.revert_button.setText("Remove" if kind == MUSIC else "Revert")
        self.revert_button.setEnabled(resource is not None)
        self.rename_button.setVisible(kind == MUSIC)
        self.rename_button.setEnabled(resource is not None)
        self.copy_button.setEnabled(resource is not None)
        self.preview.clear()
        if target is None:
            self.info.setText("Import adds a music file; choose one to rename or remove it."
                              if kind == MUSIC else "Choose something to replace")
            return
        lines = []
        if kind in (WALL, SPRITE):
            lines.append(self._name(kind, int(target)))
            entry = art_entry(kind, int(target), "")
            self._show_preview(entry)
        elif kind == SOUND:
            lines.append(f"Sound {target:02d}: {sound_name(int(target))}")
        else:
            lines.append(str(target))
        if resource is not None:
            lines.append(f"Replaced with {resource.source_name or 'your file'} "
                         f"({resource.size} bytes)" if kind != MUSIC
                         else f"From {resource.source_name or 'your file'} "
                              f"({resource.size} bytes)")
        elif kind != MUSIC:
            lines.append("The game's own" + (" (64x64 PNG replaces it)" if kind != SOUND
                                             else " (a WAV replaces it)"))
        self.info.setText("\n".join(lines))

    def _show_preview(self, entry: CatalogEntry) -> None:
        factory = self.window.thumbnails
        if not factory.available and not factory.replaced(entry):
            return
        try:
            pixels, edge, alpha = factory.pixels_for(entry)
        except Exception:  # noqa: BLE001 - no picture is fine; the name still shows
            return
        image = factory.image_from(pixels, edge, alpha)
        self.preview.setPixmap(QPixmap.fromImage(image).scaled(
            96, 96, Qt.KeepAspectRatio, Qt.FastTransformation))

    # -- actions ----------------------------------------------------------

    def _ask_file(self, title: str, pattern: str) -> str:
        path, _ = QFileDialog.getOpenFileName(self, title, self.window._start_directory(),
                                              pattern)
        return path

    def _run(self, command) -> None:
        self.window.run_command(command)

    def import_selected(self, path: str | None = None) -> bool:
        """Import (or replace) the selection from `path`, asking when None."""
        kind, target = self.selected()
        if kind != MUSIC and target is None:
            return False
        if kind == MUSIC:
            return self.import_music(path)
        if path is None:
            path = self._ask_file(f"Replace {kind} {target}",
                                  "WAV sounds (*.wav)" if kind == SOUND
                                  else "PNG images, 64x64 (*.png)")
        if not path:
            return False
        store = self.window.resource_store()
        try:
            if kind == SOUND:
                resource = import_sound(store, int(target), path)
            else:
                resource = import_art(store, kind, int(target), path)
        except (OSError, ObcEditError) as error:
            self.window._error("Cannot import that file", str(error))
            return False
        self._run(set_resource(self.project, kind, target, resource))
        return True

    def import_music(self, path: str | None = None, *, name: str | None = None) -> bool:
        if path is None:
            path = self._ask_file("Import music",
                                  "Music (*.ogg *.qoa *.imf *.wav);;All files (*)")
        if not path:
            return False
        taken = [str(r.target) for r in self.project.resources if r.kind == MUSIC]
        engine = self.window.settings.profile.engine_path or None
        self.window.statusBar().showMessage(f"Importing {Path(path).name}…", 3000)
        try:
            resource = import_music(self.window.resource_store(), path, name=name, taken=taken,
                                    engine=engine)
        except (OSError, ObcEditError) as error:
            self.window._error("Cannot import that music", str(error))
            return False
        self._run(set_resource(self.project, MUSIC, resource.target, resource,
                               label=f"Import music {resource.target}"))
        self.select(MUSIC, resource.target)
        return True

    def revert_selected(self) -> bool:
        kind, target = self.selected()
        if target is None or self.project.resource(kind, target) is None:
            return False
        if kind == MUSIC and self.music_uses().get(str(target).lower()):
            answer = QMessageBox.question(
                self, "Remove music that is played?",
                f"{target} is played on {', '.join(self.music_uses()[str(target).lower()])}. "
                "Remove it anyway? Those floors will need other music before the mod can be "
                "exported.")
            if answer != QMessageBox.StandardButton.Yes:
                return False
        label = f"Remove music {target}" if kind == MUSIC else ""
        self._run(set_resource(self.project, kind, target, None, label=label))
        return True

    def rename_selected(self, new: str | None = None) -> bool:
        kind, target = self.selected()
        if kind != MUSIC or target is None:
            return False
        extension = str(target).rsplit(".", 1)[-1]
        if new is None:
            text, accepted = QInputDialog.getText(
                self, "Rename music", f"New name (1-32 of A-Z a-z 0-9 _ -, then .{extension}):",
                text=str(target))
            if not accepted:
                return False
            new = text.strip()
        if "." not in new:
            new = f"{new}.{extension}"
        if not is_music_name(new) or not new.lower().endswith("." + extension.lower()):
            self.window._error("That name cannot be used",
                               f"{new!r}: a music name is 1-32 characters of A-Z a-z 0-9 _ - "
                               f"and keeps its .{extension}")
            return False
        try:
            command = rename_music(self.project, str(target), new)
        except ObcEditError as error:
            self.window._error("Cannot rename", str(error))
            return False
        # One step: floors and arenas that played the old name play the new
        # one, and Ctrl+Z puts both back.
        self._run(command)
        self.select(MUSIC, new)
        return True

    def save_copy_selected(self, path: str | None = None) -> bool:
        """Save the author's own imported file elsewhere. Never game data: only
        replacements exist to save."""
        kind, target = self.selected()
        resource = self.project.resource(kind, target) if target is not None else None
        if resource is None:
            return False
        if path is None:
            suggested = str(Path(self.window._start_directory())
                            / (resource.source_name or resource.mod_path.rsplit("/", 1)[-1]))
            path, _ = QFileDialog.getSaveFileName(self, "Save a copy", suggested)
        if not path:
            return False
        from obcedit_core.paths import OutputGuard, atomic_write

        data_dir = self.window.settings.profile.data_dir
        guard = OutputGuard(protected_roots=(Path(data_dir),) if data_dir else ())
        try:
            atomic_write(path, self.window.resource_store().read(resource), guard=guard)
        except (OSError, ObcEditError) as error:
            self.window._error("Could not save the copy", str(error))
            return False
        return True


__all__ = ["REPLACED", "ResourcesPanel", "art_entry", "page_names", "sprite_names"]
