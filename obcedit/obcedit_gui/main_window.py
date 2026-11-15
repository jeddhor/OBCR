# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The main window: menus, docks, map tabs, and the state that ties them.

The window owns the project document and the undo history, and it is the only
place that replaces the document. Everything else asks it to run a command.
That is what keeps undo honest: there is one path by which a map changes, and
it goes through `History`.

Docks rather than a fixed layout, because the useful arrangement depends on the
screen and on the job. A layout that ends up wrong is recoverable from
View -> Reset layout, which matters more than it sounds: a window restored
off-screen from a previous monitor is otherwise unreachable.
"""

from __future__ import annotations

import dataclasses
from pathlib import Path

from PySide6.QtCore import QProcess, QTimer, QSize, Qt, Signal
from PySide6.QtGui import QAction, QActionGroup, QKeySequence, QPixmap
from PySide6.QtWidgets import (
    QCheckBox,
    QInputDialog,
    QMenu,
    QDialog,
    QDockWidget,
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListView,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QApplication,
    QComboBox,
    QMessageBox,
    QPushButton,
    QScrollArea,
    QStatusBar,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from obcedit_core import reachability, rules
from obcedit_core.archive import archive_paths, read_archive
from obcedit_core.bundle import catalog_path
from obcedit_core.catalog import CATEGORIES, Catalog, load_catalog
from obcedit_core.commands import History, add_maps
from obcedit_core.discovery import Profile
from obcedit_core.engine_runner import (
    COMPASS,
    Session,
    SessionState,
    build_launch_plan,
    check_mod,
)
from obcedit_core.modexport import MOD_SUFFIX, audit_mod, build_mod, preview_mod, write_mod
from obcedit_core.snapshot import (
    Camera,
    check_camera,
    looks_like_a_world,
    snapshot_arguments,
    snapshot_key,
)
from obcedit_core.transforms import copy_region, flip_clip, paste_writes, rotate_clip
from obcedit_core.validation import (
    fix_label,
    fix_writes,
    profile_for_map,
    profile_for_slot,
    summarize,
    validate_local,
    validate_map,
)
from obcedit_core.document import MapDocument, ProjectDocument, SourceReference, utc_now
from obcedit_core.errors import ObcEditError, Severity
from obcedit_core.paths import OutputGuard, SourceIdentity
from obcedit_core.prefabs import PREFABS, by_key as prefab_by_key
from obcedit_core.project import (
    PROJECT_SUFFIX,
    RecoveryStore,
    SaveConflict,
    load_project,
    new_project,
    project_identity,
    save_project,
)

from .import_dialog import ImportDialog
from .resources_panel import ResourcesPanel
from .inspector import Inspector
from .map_canvas import MapCanvas
from .palette_models import CatalogFilter, CatalogModel, EntryRole
from .settings import Settings
from .thumbnails import AssetSource, ThumbnailFactory
from .tools import Tool, ToolController
from .workers import WorkerPool

CATALOG_PATH = catalog_path()

#: What each catalog category is called on its palette tab.
CATEGORY_TITLES = {
    "walls": "Walls",
    "specials": "Special walls",
    "floors": "Floors",
    "objects": "Objects",
    "enemies": "Enemies",
    "starts": "Starts and Paths",
    "markers": "Markers",
    "raw": "Raw",
}

#: The palette's tabs: the compound tools, then one per catalog category.
#: "Doors and Specials" holds the *tools* rather than the raw words behind
#: them: a door has to be on the axis its walls say, and painting a push
#: wall's marker straight on to floor leaves a moving wall with no wall in it.
PALETTE_TABS = (("Doors and Specials", "prefabs"),) + tuple(
    (CATEGORY_TITLES.get(category, category.title()), category) for category in CATEGORIES)


class MapTab(QWidget):
    """One open map: a scrollable canvas."""

    def __init__(self, document: MapDocument, catalog: Catalog | None, parent=None) -> None:
        super().__init__(parent)
        self.canvas = MapCanvas(document, catalog, self)
        area = QScrollArea(self)
        area.setWidget(self.canvas)
        area.setAlignment(Qt.AlignCenter)
        area.setAccessibleName(f"Map {document.name}")
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.addWidget(area)
        self.map_uuid = document.uuid


class MainWindow(QMainWindow):
    """OBCEdit's window."""

    project_changed = Signal()

    def __init__(self, settings: Settings | None = None, *, catalog: Catalog | None = None,
                 parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle("OBCEdit")
        self.setObjectName("obcedit-main")
        self.resize(1180, 760)

        self.settings = settings or Settings()
        self.catalog = catalog if catalog is not None else self._load_catalog()
        self.project: ProjectDocument = new_project()
        self.history = History()
        self.project_path: Path | None = None
        self._identity: tuple | None = None

        self.pool = WorkerPool(self)
        self.thumbnails = ThumbnailFactory(catalog=self.catalog)
        #: The last validation result, and the project revision it
        #: describes -- so the panel can say when it has gone stale
        #: instead of quietly showing an answer about an older map.
        self._problems: list = []
        self._problems_revision: int = -1
        #: The playtest, if one has been run. One at a time, deliberately: two
        #: engines sharing a session directory would overwrite each other's
        #: config and saves.
        self.session = None
        self.process = None
        self._session_lines: list = []
        self._session_log_path = None
        #: Whatever a read left mid-line, kept for the next one.
        self._session_tail = ""
        self._session_counter = 0
        #: Where the next snapshot is taken from, and what was last shown.
        self.camera = None
        #: Which map the camera is on. There is one camera, and it belongs to
        #: a floor -- without this it was a pair of coordinates with no map,
        #: so opening another floor left it apparently pointing into that one,
        #: and check_camera would then judge it against the wrong geometry.
        self.camera_map = ""
        self._snapshot_revision = -1
        self._snapshot_key = ""
        #: Coalesces the full pass: every edit restarts it, so the expensive
        #: rules run once after a stroke rather than once per cell of it.
        self._validate_timer = QTimer(self)
        self._validate_timer.setSingleShot(True)
        self._validate_timer.setInterval(400)
        self._validate_timer.timeout.connect(lambda: self.validate())
        self.recovery = RecoveryStore(self.settings.recovery_dir)
        #: Autosave is a safety net, not a save: it writes a recovery copy into
        #: the application's own directory and never clears the dirty flag,
        #: because telling somebody their work is saved when it is somewhere
        #: they have never heard of would be a lie.
        self._autosave_timer = QTimer(self)
        self._autosave_timer.setInterval(60_000)
        self._autosave_timer.timeout.connect(lambda: self.autosave())
        self._autosave_timer.start()

        self.tools = ToolController(self)
        self.tools.command_ready.connect(self.run_command)
        self.tools.refused.connect(self._on_prefab_refused)
        self.tools.picked.connect(self._on_picked)
        self.tools.camera_placed.connect(self.set_camera)
        self.tools.catalog = self.catalog
        self.tools.route_changed.connect(self._on_route_changed)
        self.tools.message.connect(lambda text: self.statusBar().showMessage(text, 3000))

        self._build_menus()
        self._build_tools()
        #: The ResourceStore the open project's records were last read from,
        #: so a save to a new place can carry the files along.
        self._active_store = None
        self._resource_state = None
        self._build_docks()
        self._build_resources_dock()
        self._build_central()
        self.setStatusBar(QStatusBar(self))
        self._cell_label = QLabel("", self)
        self._cell_label.setAccessibleName("Cursor position")
        self.statusBar().addPermanentWidget(self._cell_label)

        self._restore_layout()
        self.open_assets(self.settings.profile)
        self._refresh()

    # -- construction -----------------------------------------------------

    def _load_catalog(self) -> Catalog | None:
        try:
            return load_catalog(CATALOG_PATH)
        except (OSError, ValueError):
            return None

    def _action(self, text, slot, shortcut=None, *, tip="", name="") -> QAction:
        action = QAction(text, self)
        action.setStatusTip(tip or text)
        action.setToolTip(tip or text)
        if shortcut:
            action.setShortcut(QKeySequence(shortcut))
        action.setObjectName(name or text.lower().replace("&", "").replace(" ", "-"))
        # QAction.triggered carries a `checked` bool, and PySide6 passes it to
        # any slot whose signature can take it. Every command here reads its
        # first parameter as "the caller already knows the answer" --
        # open_project(path), new_map(name), import_map(archive_path) -- so the
        # menu was handing them False. open_project treated that as an empty
        # path, which is what a canceled file dialog looks like, and returned
        # without ever showing one. Drop the argument: nothing here is
        # checkable, so nothing wants it.
        action.triggered.connect(lambda _checked=False, call=slot: call())
        return action

    #: How many recent projects the File menu offers. Settings keeps more than
    #: this; a menu is for the ones you are actually moving between.
    RECENT_SHOWN = 6

    def _rebuild_recent_menu(self) -> None:
        """Refill Open Recent from settings.

        Rebuilt on every open rather than once at startup: the list changes
        whenever a project is opened or saved, and a menu built at startup goes
        stale the first time it is used.
        """
        self.recent_menu.clear()
        entries = list(self.settings.recent_projects)[:self.RECENT_SHOWN]
        if not entries:
            empty = self.recent_menu.addAction("No recent projects")
            empty.setEnabled(False)
            return
        for index, path in enumerate(entries, start=1):
            name = Path(path).name
            # &1..&9 so the keyboard reaches them, and the full path in the tip
            # because two projects may well share a file name.
            action = self.recent_menu.addAction(f"&{index}  {name}")
            action.setObjectName(f"recent-{index}")
            action.setStatusTip(path)
            action.setToolTip(path)
            action.triggered.connect(
                lambda _checked=False, chosen=path: self.open_recent(chosen)
            )
        self.recent_menu.addSeparator()
        clear = self.recent_menu.addAction("Clear the list")
        clear.setObjectName("recent-clear")
        clear.triggered.connect(lambda _checked=False: self.clear_recent())

    def open_recent(self, path: str) -> None:
        """Open a remembered project, forgetting it if it is no longer there.

        A recent list that keeps offering a file somebody has moved or deleted
        is worse than a short one, so a miss drops the entry rather than only
        complaining about it.
        """
        if not Path(path).exists():
            self.settings.forget_project(path)
            self._error("That project is gone",
                        f"{path}\n\nIt has been removed from the recent list.")
            return
        self.open_project(path)

    def clear_recent(self) -> None:
        for path in list(self.settings.recent_projects):
            self.settings.forget_project(path)

    def _build_menus(self) -> None:
        file_menu = self.menuBar().addMenu("&File")
        self.action_new = self._action("&New project", self.new_project, QKeySequence.New)
        self.action_open = self._action("&Open project…", self.open_project, QKeySequence.Open)
        self.action_new_map = self._action("New &map…", self.new_map, "Ctrl+M",
                                           tip="Add an empty map to this project")
        self.action_import = self._action("&Import map from archive…", self.import_map, "Ctrl+I")
        self.action_save = self._action("&Save", self.save_project, QKeySequence.Save)
        self.action_save_as = self._action("Save &As…", self.save_project_as, "Ctrl+Shift+S")
        self.action_save_copy = self._action(
            "Save a &Copy…", self.save_copy,
            tip="Write a copy elsewhere and keep editing this one")
        self.action_campaign = self._action(
            "Campai&gn…", self.edit_campaign,
            tip="The mod's name and its floors, in the order they are played")
        self.action_export = self._action(
            "&Export mod…", self.export_mod, "Ctrl+E",
            tip="Write the mod as a folder or a single .obcmod file, for OBCR")
        self.action_install = self._action(
            "&Install in OBCR", self.install_mod, "Ctrl+Shift+E",
            tip="Copy the mod into OBCR's mods folder, so the game's Mods menu lists it")
        self.action_quit = self._action("&Quit", self.close, QKeySequence.Quit)
        for action in (self.action_new, self.action_open):
            file_menu.addAction(action)
        self.recent_menu = file_menu.addMenu("Open &Recent")
        self.recent_menu.setObjectName("open-recent-menu")
        self.recent_menu.aboutToShow.connect(self._rebuild_recent_menu)
        self._rebuild_recent_menu()
        for action in (self.action_new_map, self.action_import):
            file_menu.addAction(action)
        file_menu.addSeparator()
        for action in (self.action_save, self.action_save_as, self.action_save_copy):
            file_menu.addAction(action)
        file_menu.addSeparator()
        for action in (self.action_campaign, self.action_export, self.action_install):
            file_menu.addAction(action)
        file_menu.addSeparator()
        file_menu.addAction(self.action_quit)

        edit_menu = self.menuBar().addMenu("&Edit")
        self.action_undo = self._action("&Undo", self.undo, QKeySequence.Undo)
        self.action_redo = self._action("&Redo", self.redo, QKeySequence.Redo)
        self.action_copy = self._action("&Copy", self.copy_selection, QKeySequence.Copy)
        self.action_paste = self._action("&Paste", self.paste_clipboard, QKeySequence.Paste)
        self.action_rotate_sel = self._action(
            "Rotate selection", self.rotate_clipboard, "Ctrl+Shift+R",
            tip="Turn the copied area a quarter turn, facings included")
        self.action_flip_h = self._action("Flip selection across", self.flip_clipboard_h)
        self.action_flip_v = self._action("Flip selection down", self.flip_clipboard_v)
        edit_menu.addAction(self.action_undo)
        edit_menu.addAction(self.action_redo)
        edit_menu.addSeparator()
        for action in (self.action_copy, self.action_paste, self.action_rotate_sel,
                       self.action_flip_h, self.action_flip_v):
            edit_menu.addAction(action)

        view_menu = self.menuBar().addMenu("&View")
        self.action_zoom_in = self._action("Zoom &in", self.zoom_in, QKeySequence.ZoomIn)
        self.action_zoom_out = self._action("Zoom &out", self.zoom_out, QKeySequence.ZoomOut)
        self.action_grid = self._action("Show &grid", self.toggle_grid, "Ctrl+G")
        self.action_grid.setCheckable(True)
        self.action_grid.setChecked(True)
        self.action_reset_layout = self._action("&Reset layout", self.reset_layout)
        for action in (self.action_zoom_in, self.action_zoom_out, self.action_grid):
            view_menu.addAction(action)
        view_menu.addSeparator()
        self.view_menu = view_menu
        view_menu.addAction(self.action_reset_layout)

        self.tools_menu = self.menuBar().addMenu("&Tools")
        self.action_test = self._action(
            "&Test in OBCR", self.playtest, "F5",
            tip="Build a one-floor preview mod of the open map and play it in OBCR",
        )
        self.action_test_here = self._action(
            "Test from the &cursor", self.playtest_here, "Shift+F5",
            tip="Play the open map starting on the cell under the mouse, facing east",
        )
        self.action_validate = self._action("&Check this map", self.validate, "F8")
        self.action_snapshot = self._action(
            "Take a &snapshot", self.take_snapshot, "F7",
            tip="An exact OBCR frame from the camera")
        self.action_stop_test = self._action(
            "S&top the playtest", self.stop_session, "Ctrl+F5",
            tip="Close the running engine")
        self.action_stop_test.setEnabled(False)
        self.action_statistics = self._action("Map &statistics", self.show_statistics)
        self.action_setup = self._action("&Setup…", self.run_setup, tip="Engine, data, workspace")
        self.tools_menu.addAction(self.action_test)
        self.tools_menu.addAction(self.action_test_here)
        self.tools_menu.addAction(self.action_stop_test)
        self.tools_menu.addAction(self.action_snapshot)
        self.tools_menu.addAction(self.action_validate)
        self.tools_menu.addAction(self.action_statistics)
        self.tools_menu.addSeparator()
        self.tools_menu.addAction(self.action_setup)

        help_menu = self.menuBar().addMenu("&Help")
        help_menu.addAction(self._action("&About OBCEdit", self.about))

        bar = self.addToolBar("Main")
        bar.setObjectName("main-toolbar")
        bar.setMovable(True)
        for action in (self.action_new, self.action_open, self.action_save,
                       self.action_export, self.action_undo, self.action_redo,
                       self.action_test):
            bar.addAction(action)

    def _build_tools(self) -> None:
        """One exclusive action per tool, with its single-key shortcut."""
        bar = self.addToolBar("Tools")
        bar.setObjectName("tools-toolbar")
        group = QActionGroup(self)
        group.setExclusive(True)
        self.tool_actions: dict[Tool, QAction] = {}
        for tool in Tool:
            action = QAction(tool.label, self)
            action.setCheckable(True)
            action.setObjectName(f"tool-{tool.value}")
            action.setShortcut(QKeySequence(tool.shortcut))
            action.setStatusTip(f"{tool.label} ({tool.shortcut})")
            action.triggered.connect(lambda _checked, chosen=tool: self.select_tool(chosen))
            group.addAction(action)
            bar.addAction(action)
            self.tool_actions[tool] = action
        self.tool_actions[Tool.BRUSH].setChecked(True)

        # Only a structure with a footprint that actually has an orientation can
        # be turned. Every wall unit is a single word usable from whichever side
        # has floor, so none of them do -- and a button that can never do
        # anything is worse than no button. It reappears on its own if a
        # multi-cell structure is ever added.
        self.action_rotate = self._action(
            "&Turn structure", self.tools.rotate_prefab, "Ctrl+R",
            tip="Turn the selected structure a quarter turn clockwise",
        )
        self.action_rotate.setVisible(False)
        bar.addAction(self.action_rotate)

        self.filled_box = QCheckBox("Filled", self)
        self.filled_box.setAccessibleName("Filled rectangle")
        self.filled_box.toggled.connect(
            lambda checked: setattr(self.tools, "filled_rectangle", checked)
        )
        bar.addWidget(self.filled_box)

    def select_tool(self, tool: Tool) -> None:
        if tool is not Tool.PREFAB:
            self.tools.prefab = None
        self.tools.cancel_pending()
        self.tools.set_tool(tool)
        action = self.tool_actions.get(tool)
        if action is not None and not action.isChecked():
            action.setChecked(True)

    def _build_docks(self) -> None:
        self.map_list = QListWidget(self)
        self.map_list.setAccessibleName("Maps in this project")
        self.map_list.currentRowChanged.connect(self._on_map_selected)
        self.map_list.setContextMenuPolicy(Qt.CustomContextMenu)
        self.map_list.customContextMenuRequested.connect(self._map_context_menu)
        # A new project has no maps, so the list is empty -- and an empty list
        # is exactly where "how do I start one?" gets asked. The button is
        # always there; the context menu covers the right-click reflex, on a
        # row or on the blank space below the last one.
        self.new_map_button = QPushButton("New map…", self)
        self.new_map_button.setAccessibleName("New map")
        self.new_map_button.setToolTip("Add an empty 64x64 map to this project (Ctrl+M)")
        self.new_map_button.clicked.connect(lambda: self.new_map())

        maps_panel = QWidget(self)
        maps_layout = QVBoxLayout(maps_panel)
        maps_layout.setContentsMargins(0, 0, 0, 0)
        maps_layout.setSpacing(2)
        maps_layout.addWidget(self.map_list)
        maps_layout.addWidget(self.new_map_button)

        maps_dock = QDockWidget("Maps", self)
        maps_dock.setObjectName("maps-dock")
        maps_dock.setWidget(maps_panel)
        self.addDockWidget(Qt.LeftDockWidgetArea, maps_dock)
        self.maps_dock = maps_dock

        palette = QWidget(self)
        layout = QVBoxLayout(palette)
        layout.setContentsMargins(4, 4, 4, 4)
        self.search = QLineEdit(palette)
        self.search.setPlaceholderText("Search by name, class or raw value")
        self.search.setAccessibleName("Palette search")
        self.search.setClearButtonEnabled(True)
        self.search.textChanged.connect(self._refresh_palette)
        layout.addWidget(self.search)

        self.used_only = QCheckBox("Only what this map uses", palette)
        self.used_only.setAccessibleName("Filter to values used in this map")
        self.used_only.setToolTip(
            "Show only the entries the open map already has, which is how you find "
            "the wall you are looking at rather than the one you are looking for."
        )
        self.used_only.toggled.connect(lambda _: self._refresh_palette())
        layout.addWidget(self.used_only)

        self.palette_tabs = QTabWidget(palette)
        self.palette_tabs.setAccessibleName("Palette")
        self.palette_models: dict[str, CatalogModel] = {}

        # Structures are not catalog entries -- they are compound tools, and
        # they get a plain list because what matters is the name and what it
        # needs, not a picture of one word out of several.
        self.prefab_list = QListWidget(self)
        self.prefab_list.setAccessibleName("Doors and specials palette")
        # Both signals: `currentItemChanged` catches keyboard navigation, and
        # `itemClicked` catches clicking the row that is already current --
        # which is exactly what you do coming back from the Walls tab to the
        # structure you were using, and which would otherwise re-arm nothing.
        self.prefab_list.currentItemChanged.connect(
            lambda current, _previous: self._on_prefab_chosen(current, _previous))
        self.prefab_list.itemClicked.connect(
            lambda item: self._on_prefab_chosen(item, None))
        self.palette_tabs.addTab(self.prefab_list, "Doors and Specials")

        for title, category in PALETTE_TABS[1:]:
            view = QListView(self.palette_tabs)
            view.setViewMode(QListView.IconMode)
            view.setResizeMode(QListView.Adjust)
            view.setIconSize(QSize(48, 48))
            view.setSpacing(4)
            view.setUniformItemSizes(True)
            view.setAccessibleName(f"{title} palette")
            model = CatalogModel(factory=self.thumbnails, pool=self.pool, parent=self)
            view.setModel(model)
            view.clicked.connect(self._on_palette_clicked)
            self.palette_models[category] = model
            self.palette_tabs.addTab(view, title)
        self._refresh_prefabs()
        self.palette_tabs.currentChanged.connect(lambda _: self._refresh_palette())
        layout.addWidget(self.palette_tabs, 1)

        self.selection_label = QLabel("Nothing selected", palette)
        self.selection_label.setWordWrap(True)
        self.selection_label.setAccessibleName("Selected palette item")
        layout.addWidget(self.selection_label)

        palette_dock = QDockWidget("Palette", self)
        palette_dock.setObjectName("palette-dock")
        palette_dock.setWidget(palette)
        self.addDockWidget(Qt.RightDockWidgetArea, palette_dock)
        self.palette_dock = palette_dock

        self.inspector = Inspector(self.catalog, self.thumbnails, self)
        self.inspector.change_requested.connect(self._on_inspector_change)
        inspector_dock = QDockWidget("Inspector", self)
        inspector_dock.setObjectName("inspector-dock")
        inspector_dock.setWidget(self.inspector)
        self.addDockWidget(Qt.RightDockWidgetArea, inspector_dock)
        self.inspector_dock = inspector_dock

        self.problems = QListWidget(self)
        self.problems.setAccessibleName("Problems")
        self.problems.itemActivated.connect(self._on_problem_activated)
        self.problems.currentItemChanged.connect(self._on_problem_selected)

        # Warnings outnumber errors on any real map, and the plan's rule is
        # that a panel nobody can scan is a panel nobody reads. The filter is
        # a floor, not a set of checkboxes: "errors only" is the question
        # people actually ask.
        self.problem_filter = QComboBox(self)
        self.problem_filter.setAccessibleName("Show problems")
        self.problem_filter.addItem("Everything", Severity.INFORMATION)
        self.problem_filter.addItem("Warnings and errors", Severity.WARNING)
        self.problem_filter.addItem("Errors only", Severity.ERROR)
        self.problem_filter.setCurrentIndex(1)
        self.problem_filter.currentIndexChanged.connect(self._repaint_problems)

        self.problem_fix = QPushButton("Fix this", self)
        self.problem_fix.setAccessibleName("Fix the selected problem")
        self.problem_fix.setEnabled(False)
        self.problem_fix.clicked.connect(lambda _checked=False: self.apply_fix())

        self.problem_status = QLabel("", self)
        self.problem_status.setAccessibleName("Problem summary")

        controls = QHBoxLayout()
        controls.setContentsMargins(4, 2, 4, 2)
        controls.addWidget(self.problem_filter)
        controls.addWidget(self.problem_fix)
        controls.addWidget(self.problem_status, 1)

        problems_panel = QWidget(self)
        problems_layout = QVBoxLayout(problems_panel)
        problems_layout.setContentsMargins(0, 0, 0, 0)
        problems_layout.setSpacing(2)
        problems_layout.addLayout(controls)
        problems_layout.addWidget(self.problems)

        # Snapshot: an exact engine frame of the map, from a tile you pick.
        self.snapshot_view = QLabel("No snapshot yet", self)
        self.snapshot_view.setAccessibleName("Snapshot")
        self.snapshot_view.setAlignment(Qt.AlignCenter)
        self.snapshot_view.setMinimumHeight(120)
        self.snapshot_status = QLabel(
            "Pick the Camera tool, click a floor tile, then Take a snapshot", self)
        self.snapshot_status.setAccessibleName("Snapshot state")
        self.snapshot_status.setWordWrap(True)
        self.snapshot_take = QPushButton("Take a snapshot", self)
        self.snapshot_take.setAccessibleName("Take a snapshot")
        self.snapshot_take.clicked.connect(lambda _checked=False: self.take_snapshot())
        self.snapshot_turn = QPushButton("Turn 90°", self)
        self.snapshot_turn.setAccessibleName("Turn the camera")
        self.snapshot_turn.clicked.connect(lambda _checked=False: self.turn_camera())

        snap_controls = QHBoxLayout()
        snap_controls.setContentsMargins(4, 2, 4, 2)
        snap_controls.addWidget(self.snapshot_take)
        snap_controls.addWidget(self.snapshot_turn)
        snap_controls.addStretch(1)
        snap_panel = QWidget(self)
        snap_layout = QVBoxLayout(snap_panel)
        snap_layout.setContentsMargins(0, 0, 0, 0)
        snap_layout.setSpacing(2)
        snap_layout.addLayout(snap_controls)
        snap_layout.addWidget(self.snapshot_status)
        snap_layout.addWidget(self.snapshot_view, 1)
        snap_dock = QDockWidget("Snapshot", self)
        snap_dock.setObjectName("snapshot-dock")
        snap_dock.setWidget(snap_panel)
        self.addDockWidget(Qt.RightDockWidgetArea, snap_dock)
        self.snapshot_dock = snap_dock

        self.test_log = QListWidget(self)
        self.test_log.setAccessibleName("Test log")
        self.test_log_status = QLabel("No playtest has been run yet", self)
        self.test_log_status.setAccessibleName("Playtest state")
        test_panel = QWidget(self)
        test_layout = QVBoxLayout(test_panel)
        test_layout.setContentsMargins(4, 2, 4, 2)
        test_layout.setSpacing(2)
        test_layout.addWidget(self.test_log_status)
        test_layout.addWidget(self.test_log)
        test_dock = QDockWidget("Test Log", self)
        test_dock.setObjectName("test-log-dock")
        test_dock.setWidget(test_panel)
        self.addDockWidget(Qt.BottomDockWidgetArea, test_dock)
        self.test_log_dock = test_dock

        problems_dock = QDockWidget("Problems", self)
        problems_dock.setObjectName("problems-dock")
        problems_dock.setWidget(problems_panel)
        self.addDockWidget(Qt.BottomDockWidgetArea, problems_dock)
        self.problems_dock = problems_dock

    def _build_central(self) -> None:
        self.tabs = QTabWidget(self)
        self.tabs.setObjectName("map-tabs")
        self.tabs.setTabsClosable(True)
        self.tabs.setDocumentMode(True)
        self.tabs.setAccessibleName("Open maps")
        self.tabs.tabCloseRequested.connect(self._close_tab)
        self.tabs.currentChanged.connect(self._on_tab_changed)
        self.setCentralWidget(self.tabs)

    # -- assets -----------------------------------------------------------

    def open_assets(self, profile: Profile) -> bool:
        """Open the user's game data for thumbnails. Absent data is not fatal."""
        if not profile.data_dir:
            return False
        try:
            self.thumbnails.source = AssetSource.open(
                profile.data_dir, fingerprint=profile.data_fingerprint
            )
        except (OSError, ObcEditError, ValueError) as error:
            self.thumbnails.source = None
            self._note_problem(f"Game data unavailable: {error}")
            return False
        self._apply_wall_colors()
        return True

    def _apply_wall_colors(self) -> None:
        """Average colors for the canvas's texture layer, computed once.

        Walls and special walls from their page, floors from their floor
        set's floor texture, so the map reads like the game at a glance."""
        if not self.thumbnails.available or self.catalog is None:
            return
        colors = {}
        for category in ("walls", "specials", "floors"):
            for entry in self.catalog.in_category(category):
                if entry.plane == 0 and entry.value not in colors:
                    swatch = self.thumbnails.swatch(entry)
                    if swatch is not None:
                        colors[entry.value] = swatch
        for index in range(self.tabs.count()):
            self.tabs.widget(index).canvas.set_wall_colors(colors)

    # -- project ----------------------------------------------------------

    def set_project(self, project: ProjectDocument, path: Path | None = None) -> None:
        self.project = project
        self.project_path = path
        self._identity = project_identity(path) if path else None
        self.history.clear()
        self.tabs.clear()
        store = self.resource_store()
        self._active_store = store
        if project.resources:
            for problem in store.check(project.resources):
                self._note_problem(f"Resources: {problem.message}")
            if path is not None:
                store.prune(project.resources)
        self._refresh()

    def new_project(self) -> None:
        if not self._confirm_discard():
            return
        self.set_project(new_project())
        self.statusBar().showMessage("New project", 4000)

    def open_project(self, path: str | None = None) -> None:
        if not self._confirm_discard():
            return
        if path is None:
            path, _ = QFileDialog.getOpenFileName(
                self, "Open project", self._start_directory(),
                f"OBCEdit projects (*{PROJECT_SUFFIX})",
            )
        if not path:
            return
        try:
            project = load_project(path)
        except (OSError, ObcEditError) as error:
            self._error("Could not open that project", str(error))
            return
        self.set_project(project, Path(path))
        self.settings.remember_project(path)
        self.statusBar().showMessage(f"Opened {Path(path).name}", 4000)

    def new_map(self, name: str | None = None, slot: int | None = None,
                width: int = 64, height: int = 64) -> MapDocument | None:
        """Add an empty map to the project and open it.

        Operation Body Count's floors are always 64x64 (the engine's map
        arrays are fixed), so the size is there for tests rather than people.
        The slot -- the map's number in the project archive -- defaults to the
        first one this project is not already using. A mod renumbers its maps
        in campaign order, so the slot is bookkeeping, not the floor number.
        """
        used = {document.slot for document in self.project.maps}
        if slot is None:
            slot = next((n for n in range(100) if n not in used), None)
            if slot is None:
                self._error("No free slot", "This project already has 100 maps.")
                return None

        if name is None:
            name, accepted = QInputDialog.getText(
                self, "New map", f"Name for map {slot} (15 characters):",
                text=f"FLOOR {slot + 1}"
            )
            if not accepted:
                return None

        try:
            document = MapDocument.new_room(slot=slot, name=name or f"FLOOR {slot + 1}",
                                            width=width, height=height)
        except ObcEditError as error:
            self._error("Could not create that map", str(error))
            return None

        self.project = self.project.added(document)
        self._refresh()
        self.open_map(document.uuid)
        self.statusBar().showMessage(
            f"Added {document.name} ({width}x{height})", 4000
        )
        return document

    def import_map(self, archive_path: str | None = None, number: int | None = None) -> None:
        if archive_path is None:
            archive_path, _ = QFileDialog.getOpenFileName(
                self, "Import from a map archive (MAPHEAD.BC + MAPTEMP.BC)",
                self.settings.profile.data_dir or self._start_directory(),
                "Map archives (MAPHEAD.BC MAPTEMP.BC *.BC *.bc);;All files (*)",
            )
        if not archive_path:
            return
        try:
            # The pair is found beside whichever file was chosen (or in the
            # folder); MAPTEMP holds the maps, so it is what identity covers.
            _head, temp = archive_paths(archive_path)
            identity = SourceIdentity.probe(temp)
            archive = read_archive(archive_path)
        except (OSError, ObcEditError) as error:
            self._error("Could not read that archive", str(error))
            return

        if number is not None:
            wanted = [number]
        else:
            # The original holds 61 maps; let the user pick.
            dialog = ImportDialog(archive, str(archive_path), self)
            if dialog.exec() != QDialog.DialogCode.Accepted:
                return
            wanted = dialog.chosen()
        if not wanted:
            return

        # One command, whether it is one map or sixty: an import that lands as
        # sixty separate undo steps is one nobody can back out of.
        try:
            records = [archive.by_number(n) for n in wanted]
        except ObcEditError as error:
            self._error("No such map", str(error))
            return

        documents = [
            MapDocument.from_record(
                record,
                source=SourceReference(
                    display_path=str(archive_path), sha256=identity.digest,
                    map_number=record.number, imported_at=utc_now(),
                ),
            )
            for record in records
        ]
        self.run_command(add_maps(documents, index=len(self.project.maps), label=(
            f"Import {len(documents)} maps" if len(documents) > 1
            else f"Import {records[0].name.text or records[0].label}")))
        self.open_map(documents[0].uuid)
        identity.verify_unchanged()
        self.statusBar().showMessage(
            f"Imported {len(documents)} map(s) from {Path(temp).parent.name or temp}", 4000)

    def save_project(self) -> bool:
        if self.project_path is None:
            return self.save_project_as()
        previous = self._active_store
        try:
            self._carry_resources(previous)
            revision = save_project(self.project, self.project_path,
                                    expect_identity=self._identity)
        except SaveConflict:
            # Somebody -- another copy of the editor, a sync client, the user
            # in a text editor -- changed the file since it was opened. The one
            # thing that must not happen is a silent overwrite, so this asks,
            # and the default is the answer that loses nothing.
            answer = QMessageBox.question(
                self, "The file changed underneath this project",
                f"{self.project_path.name} has been modified since it was opened.\n\n"
                "Overwrite it with what is in the editor, or save this copy "
                "somewhere else?",
                QMessageBox.StandardButton.SaveAll | QMessageBox.StandardButton.Save
                | QMessageBox.StandardButton.Cancel,
                QMessageBox.StandardButton.SaveAll,
            )
            if answer == QMessageBox.StandardButton.Save:
                self._identity = project_identity(self.project_path)
                return self.save_project()
            if answer == QMessageBox.StandardButton.SaveAll:
                return self.save_project_as()
            return False
        except Exception as error:
            self._error("Could not save", str(error))
            return False
        self.project = self.project.marked_saved(revision)
        self._identity = project_identity(self.project_path)
        self._active_store = self.resource_store()
        if previous is not None and previous.root != self._active_store.root \
                and previous.root.parent.name == "unsaved":
            # First save of a new project: its files now live beside it.
            previous.discard()
        self.recovery.discard(self.project.uuid)
        self.settings.remember_project(self.project_path)
        self._refresh()
        self.statusBar().showMessage(f"Saved {self.project_path.name}", 4000)
        return True

    def save_project_as(self) -> bool:
        path, _ = QFileDialog.getSaveFileName(
            self, "Save project", self._start_directory(),
            f"OBCEdit projects (*{PROJECT_SUFFIX})",
        )
        if not path:
            return False
        if not path.endswith(PROJECT_SUFFIX):
            path += PROJECT_SUFFIX
        self.project_path = Path(path)
        self._identity = None
        return self.save_project()

    def autosave(self) -> bool:
        """Write a recovery copy if there is anything to recover."""
        if not self.project.dirty:
            return False
        try:
            self.recovery.autosave(self.project,
                                   str(self.project_path or ""))
        except Exception:
            # A failed autosave must never interrupt editing, and must never
            # be the thing that loses the work it was protecting.
            return False
        return True

    def offer_recovery(self) -> int:
        """At startup, offer back anything a previous run did not finish saving.

        Only work that was *ahead* of its last save is worth offering: a
        recovery copy whose revision matches what is on disk is a copy of a
        file the user already has.
        """
        try:
            records = [r for r in self.recovery.list_recoveries()
                       if r.autosaved_revision > r.saved_revision]
        except Exception:
            return 0
        if not records:
            return 0
        newest = max(records, key=lambda r: r.timestamp)
        where = Path(newest.original_path).name if newest.original_path else "an unsaved project"
        answer = QMessageBox.question(
            self, "Unsaved work from a previous session",
            f"OBCEdit has a recovery copy of {where} from {newest.timestamp}, "
            "with changes that were never saved.\n\nOpen it?",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.Yes,
        )
        if answer != QMessageBox.StandardButton.Yes:
            # Discarded deliberately: an offer that comes back every launch is
            # one people learn to dismiss without reading.
            self.recovery.discard(newest.project_uuid)
            return 0
        try:
            self.set_project(self.recovery.load(newest.project_uuid),
                             Path(newest.original_path) if newest.original_path else None)
        except Exception as error:
            self._error("Could not open the recovery copy", str(error))
            return 0
        self.statusBar().showMessage(
            "Recovered unsaved work — this project has not been saved yet", 10_000)
        return 1

    def save_copy(self) -> bool:
        """Write the project somewhere else and carry on editing this one.

        Save As moves where the project lives; this does not. It is what you
        want before trying something -- a snapshot you can go back to -- and
        the distinction matters because Save As silently changes what the next
        Ctrl+S overwrites.
        """
        path, _ = QFileDialog.getSaveFileName(
            self, "Save a copy", self._start_directory(),
            f"OBCEdit projects (*{PROJECT_SUFFIX})",
        )
        if not path:
            return False
        if not path.endswith(PROJECT_SUFFIX):
            path += PROJECT_SUFFIX
        if self.project_path is not None and Path(path) == self.project_path:
            self._error("That is this project",
                        "A copy has to go somewhere else. Use Save to write "
                        "this one.")
            return False
        try:
            if self.project.resources:
                from obcedit_core.resources import store_for

                store_for(Path(path), workspace="", project_uuid="").adopt_all(
                    self.resource_store())
            save_project(self.project, Path(path))
        except Exception as error:
            self._error("Could not write the copy", str(error))
            return False
        # Deliberately not touched: project_path, the dirty flag, the identity.
        # The copy is a copy.
        self.settings.remember_project(path)
        self.statusBar().showMessage(f"Wrote a copy to {Path(path).name}", 4000)
        return True

    def preflight(self, maps) -> list:
        """Every error in the maps about to leave the editor.

        An export-blocking invariant has a diagnostic and the diagnostic
        blocks the export: a mod OBCR refuses to load, or loads into an
        unplayable floor, is worse than a refusal here, because it fails
        somewhere with no idea what is wrong. Warnings do not block -- an
        unfinished map is a normal thing to want to look at.
        """
        if self.catalog is None:
            return []
        blocking = []
        for document in maps:
            for problem in validate_map(document, self.catalog,
                                        profile=self._map_profile(document)):
                if problem.severity is Severity.ERROR:
                    blocking.append((document, problem))
        return blocking

    def _preflight_refused(self, blocking, what: str) -> bool:
        """Report a refused export. True when the caller must stop."""
        if not blocking:
            return False
        lines = [f"{document.name or document.label}: {problem.message}"
                 for document, problem in blocking[:6]]
        if len(blocking) > 6:
            lines.append(f"...and {len(blocking) - 6} more")
        self._error(f"{what} has errors",
                    "This will not play correctly, so it has not been written:\n\n"
                    + "\n".join(lines)
                    + "\n\nThe Problems panel lists them all; some can be fixed "
                      "with one click.")
        return True

    # -- the mod ------------------------------------------------------------

    def current_campaign(self):
        """The project's campaign, or the default one (every map, in order)."""
        from obcedit_core.campaign import Campaign

        if self.project.campaign:
            return Campaign.from_json(self.project.campaign)
        return Campaign.default_for(self.project)

    def edit_campaign(self) -> bool:
        """The mod's identity and its floors, in order.

        Kept out of the map editor entirely: a campaign is about the project,
        not about any one map, and nothing here can change a plane word.
        """
        from .campaign_dialog import CampaignDialog

        try:
            current = self.current_campaign()
        except (ObcEditError, ValueError, TypeError) as error:
            self._error("That campaign cannot be read", str(error))
            return False

        dialog = CampaignDialog(current, self.project.maps, self,
                                music_names=self.music_names())
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return False
        return self.set_campaign(dialog.campaign())

    def set_campaign(self, campaign) -> bool:
        """Store a campaign in the project. Returns whether anything changed."""
        if self.project.campaign and campaign.to_json() == self.project.campaign:
            return False
        # A campaign is part of the project, so changing it makes the project
        # dirty like any other edit. It is not a setting that quietly persists.
        self.project = self.project.with_campaign(campaign.to_json())
        self._refresh_title()
        if getattr(self, "resources_panel", None) is not None:
            self.resources_panel.refresh()
        self._revalidate_soon()
        count = len(campaign.levels)
        self.statusBar().showMessage(
            f"Campaign: {campaign.title or campaign.mod_id}, {count} floor(s)", 6000)
        return True

    def _checked_mod(self, what: str):
        """The project's mod, built and validated, or None after saying why."""
        from obcedit_core.campaign import validate as validate_campaign

        if not self.project.maps and not self.project.campaign:
            self._error("Nothing to export", "This project has no maps yet.")
            return None
        try:
            campaign = self.current_campaign()
        except (ObcEditError, ValueError, TypeError) as error:
            self._error("That campaign cannot be read", str(error))
            return None
        problems = validate_campaign(campaign, self.project.maps,
                                     music_names=self.music_names())
        blocking = [p for p in problems if p.severity >= Severity.ERROR]
        if blocking:
            self._error(
                f"{what}: the campaign has errors",
                "\n".join(f"{p.message}" + (f" ({p.where})" if p.where else "")
                          for p in blocking[:6])
                + "\n\nFile > Campaign shows these as you fix them.")
            return None
        used = set(campaign.map_order())
        maps = [d for d in self.project.maps if d.uuid in used]
        if self._preflight_refused(self.preflight(maps), what):
            return None
        try:
            return build_mod(self.project, campaign, store=self.resource_store())
        except ObcEditError as error:
            self._error(f"{what}: could not build the mod", str(error))
            return None

    def _engine_check(self, mod: Path) -> bool:
        """`obcr --check-mod`, when an engine is configured. OBCR is the gate:
        a mod it would refuse is never reported as written successfully."""
        profile = self.settings.profile
        if not profile.engine_path or not Path(profile.engine_path).is_file():
            return True
        result = check_mod(profile.engine_path, mod, data_dir=profile.data_dir or None)
        self._show_mod_check(result)
        if result.unreadable:
            self._note_problem("OBCR could not check the mod: "
                               + (result.output.strip()[-200:] or "no output"))
            return True
        if not result.ok:
            first = result.errors[0].message if result.errors else result.output.strip()[-200:]
            self._error("OBCR refuses this mod", first)
            return False
        return True

    def _show_mod_check(self, result) -> None:
        """Put `--check-mod`'s findings in the Problems panel."""
        for problem in result.problems:
            where = []
            if problem.level is not None:
                where.append(f"floor {problem.level + 1}")
            if problem.cell is not None:
                where.append(f"cell {problem.cell[0]},{problem.cell[1]}")
            item = QListWidgetItem(f"OBCR {problem.severity}: {problem.message}"
                                   + (f"  [{', '.join(where)}]" if where else ""))
            item.setToolTip("obcr --check-mod")
            item.setData(Qt.UserRole, problem.cell)
            self.problems.addItem(item)

    def export_mod(self, path: str | None = None) -> Path | None:
        """Write the mod: a folder, or one `.obcmod` file.

        This is the file that is handed to other people, which is why it
        refuses maps imported from the original game (their words are the
        game's, not the author's), audits what it wrote by reading it back,
        and asks OBCR to check it before calling it done.
        """
        build = self._checked_mod("This mod")
        if build is None:
            return None
        if path is None:
            suggested = str(Path(self._start_directory()) / f"{build.campaign.mod_id}{MOD_SUFFIX}")
            path, chosen = QFileDialog.getSaveFileName(
                self, "Export mod", suggested,
                f"One file (*{MOD_SUFFIX});;A folder (*)")
            if not path:
                return None
            if chosen.startswith("One file") and not path.lower().endswith(MOD_SUFFIX):
                path += MOD_SUFFIX
        return self._write_and_check(build, Path(path))

    def _write_and_check(self, build, destination: Path) -> Path | None:
        profile = self.settings.profile
        roots = (Path(profile.data_dir),) if profile.data_dir else ()
        try:
            written = write_mod(build, destination, guard=OutputGuard(protected_roots=roots))
            audit = audit_mod(written, catalog=self.catalog)
        except (OSError, ObcEditError) as error:
            self._error("Could not write the mod", str(error))
            return None
        if not audit.ok:
            self._error("The written mod does not read back",
                        "\n".join(str(p) for p in audit.problems[:6]))
            return None
        if not self._engine_check(written):
            return None
        self.statusBar().showMessage(
            f"Wrote {written.name}: {build.campaign.title}, "
            f"{len(build.campaign.levels)} floor(s)", 8000)
        return written

    def install_mod(self) -> Path | None:
        """Export the mod into OBCR's own mods folder, as a folder named by
        its id, so the game's Mods menu lists it."""
        build = self._checked_mod("This mod")
        if build is None:
            return None
        target = Path(self.settings.mods_dir) / build.campaign.mod_id
        stale = target.with_suffix(MOD_SUFFIX)
        if stale.is_file():
            # Two copies of one id: OBCR would list the folder and the file.
            answer = QMessageBox.question(
                self, "Replace the installed mod?",
                f"{stale.name} is already installed. Replace it with this version?",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.Yes)
            if answer != QMessageBox.StandardButton.Yes:
                return None
            try:
                stale.unlink()
            except OSError as error:
                self._error("Could not replace the installed mod", str(error))
                return None
        written = self._write_and_check(build, target)
        if written is not None:
            self.statusBar().showMessage(
                f"Installed {build.campaign.title} in {written}. "
                "Choose it from Mods in OBCR's main menu.", 10000)
        return written

    # -- maps -------------------------------------------------------------

    def open_map(self, uuid: str) -> MapTab:
        for index in range(self.tabs.count()):
            if self.tabs.widget(index).map_uuid == uuid:
                self.tabs.setCurrentIndex(index)
                return self.tabs.widget(index)
        document = self.project.map_by_uuid(uuid)
        tab = MapTab(document, self.catalog, self)
        tab.canvas.hovered.connect(self._on_hover)
        tab.canvas.pressed.connect(self._on_press)
        tab.canvas.dragged.connect(self.tools.drag)
        tab.canvas.released.connect(self.tools.release)
        tab.canvas.finish_requested.connect(self.tools.finish_route)
        tab.canvas.cancel_requested.connect(self.tools.cancel_pending)
        index = self.tabs.addTab(tab, document.name or document.label)
        self.tabs.setCurrentIndex(index)
        self._apply_wall_colors()
        self._show_camera()
        return tab

    @property
    def current_tab(self) -> MapTab | None:
        widget = self.tabs.currentWidget()
        return widget if isinstance(widget, MapTab) else None

    def _close_tab(self, index: int) -> None:
        self.tabs.removeTab(index)

    def _on_tab_changed(self, index: int) -> None:
        tab = self.current_tab
        if tab is not None:
            self.tools.set_document(self.project.map_by_uuid(tab.map_uuid))
        self._show_camera()
        self._refresh_title()

    def _on_press(self, x: int, y: int, button: int) -> None:
        tab = self.current_tab
        if tab is None:
            return
        document = self.project.map_by_uuid(tab.map_uuid)
        self.tools.set_document(document)
        self.tools.press(x, y, button)
        self.inspector.show_cell(self.project.map_by_uuid(tab.map_uuid), x, y)

    def _on_picked(self, plane: int, value: int) -> None:
        """The eyedropper selects the catalog entry it found."""
        if self.catalog is None:
            return
        entry = self.catalog.for_value(plane, value)
        if entry is None:
            return
        self.selected_entry = entry
        self.tools.set_entry(entry)
        self.selection_label.setText(f"<b>{entry.name}</b> — raw {entry.value}")

    def _on_inspector_change(self, plane: int, x: int, y: int, value: int) -> None:
        tab = self.current_tab
        if tab is None:
            return
        document = self.project.map_by_uuid(tab.map_uuid)
        from obcedit_core.commands import write_words

        self.run_command(write_words(document, [(plane, x, y, value)], label="Change property"))
        self.inspector.show_cell(self.project.map_by_uuid(tab.map_uuid), x, y)

    def _on_problem_activated(self, item) -> None:
        """Jump the inspector to the cell a diagnostic names."""
        cell = item.data(Qt.UserRole)
        tab = self.current_tab
        if cell and tab is not None:
            self.inspector.show_cell(self.project.map_by_uuid(tab.map_uuid), *cell)

    # -- the maps list ------------------------------------------------------

    def _map_context_menu(self, point) -> None:
        """Right-click on a map. Everything acts on the map you clicked, not on
        whichever one happens to be open.

        Below the last row -- and in an empty project, anywhere -- there is no
        map to act on, so the menu is just the one thing worth offering there.
        """
        item = self.map_list.itemAt(point)
        row = self.map_list.row(item) if item is not None else -1
        menu = self.build_map_menu(row)
        if menu is not None:
            menu.exec(self.map_list.mapToGlobal(point))

    def build_map_menu(self, row: int) -> "QMenu | None":
        """The menu for one row, built but not shown.

        Separate from showing it so a test can read the actions off it. A test
        that has to open a modal to find out what is in a menu is a test that
        hangs on the offscreen platform.
        """
        menu = QMenu(self)
        menu.setObjectName("map-context-menu")

        if not 0 <= row < len(self.project.maps):
            # No row under the pointer: the only thing that makes sense here is
            # starting one. Returning None instead would leave a right-click in
            # an empty project doing nothing at all, which is where somebody
            # with no maps yet is most likely to try it.
            action = menu.addAction("&New map…")
            action.setObjectName("new-map")
            action.triggered.connect(lambda: self.new_map())
            return menu

        document = self.project.maps[row]
        actions = [
            ("&New map…", lambda: self.new_map()),
            (None, None),
            ("&Open", lambda: self.open_map(document.uuid)),
            ("&Rename…", lambda: self.rename_map(document.uuid)),
            ("Change &slot…", lambda: self.change_slot(document.uuid)),
            ("D&uplicate", lambda: self.duplicate_map(document.uuid)),
            (None, None),
            ("&Check this map", lambda: (self.open_map(document.uuid), self.validate())),
            ("&Test in OBCR", lambda: (self.open_map(document.uuid), self.playtest())),
            (None, None),
            ("&Delete", lambda: self.delete_map(document.uuid)),
        ]
        for title, slot in actions:
            if title is None:
                menu.addSeparator()
                continue
            action = menu.addAction(title)
            action.setObjectName(title.lower().replace("&", "").replace("…", "")
                                 .replace(" ", "-"))
            action.triggered.connect(slot)
        return menu

    def rename_map(self, uuid: str, name: str | None = None) -> bool:
        """Rename a map. This replaces the whole 16-byte field, deliberately."""
        document = self.project.map_by_uuid(uuid)
        if name is None:
            name, accepted = QInputDialog.getText(
                self, "Rename map", "Name (15 characters, plain ASCII):",
                text=document.name,
            )
            if not accepted:
                return False
        from obcedit_core.commands import rename_map as rename_command

        try:
            command = rename_command(document, name)
        except ObcEditError as error:
            self._error("That name will not fit", str(error))
            return False
        self.run_command(command)
        self._refresh()
        return True

    def change_slot(self, uuid: str, slot: int | None = None) -> bool:
        """Change a map's number in the project (0-99).

        Only bookkeeping: a mod numbers its maps in campaign order. It decides
        which validation profile applies (slots 50-59 are network arenas in
        the original), and it is what Import keeps.
        """
        document = self.project.map_by_uuid(uuid)
        if slot is None:
            slot, accepted = QInputDialog.getInt(
                self, "Change slot", "Map number (0-99):", document.slot, 0, 99)
            if not accepted:
                return False
        taken = {other.slot: other for other in self.project.maps if other.uuid != uuid}
        if slot in taken:
            self._error(
                "That slot is taken",
                f"Slot {slot} is already {taken[slot].name!r}.",
            )
            return False
        from obcedit_core.commands import set_slot

        self.run_command(set_slot(document, slot))
        self._refresh()
        return True

    def duplicate_map(self, uuid: str) -> bool:
        """Copy a map into the next free slot, with its own identity."""
        from dataclasses import replace as _replace

        from obcedit_core.commands import add_map
        from obcedit_core.document import new_uuid

        document = self.project.map_by_uuid(uuid)
        used = {other.slot for other in self.project.maps}
        slot = next((n for n in range(100) if n not in used), None)
        if slot is None:
            self._error("No free slot", "This project already has 100 maps.")
            return False

        copy = _replace(document, uuid=new_uuid(), slot=slot, source=document.source)
        self.run_command(add_map(copy, self.project.index_of(uuid) + 1,
                                 label=f"Duplicate {document.name}"))
        self._refresh()
        self.open_map(copy.uuid)
        return True

    def delete_map(self, uuid: str, *, confirm: bool = True) -> bool:
        """Remove a map. Undoable, which is why it is a command and not a pop."""
        document = self.project.map_by_uuid(uuid)
        if confirm:
            answer = QMessageBox.question(
                self, "Delete map",
                f"Delete {document.name!r} ({document.label}) from this project?\n\n"
                "Undo will bring it back.",
                QMessageBox.Yes | QMessageBox.No,
            )
            if answer != QMessageBox.Yes:
                return False

        from obcedit_core.commands import remove_map

        self.run_command(remove_map(self.project, uuid))
        self._refresh()
        self.statusBar().showMessage(f"Deleted {document.name}", 4000)
        return True

    def _on_map_selected(self, row: int) -> None:
        if 0 <= row < len(self.project.maps):
            self.open_map(self.project.maps[row].uuid)

    def _on_route_changed(self, route) -> None:
        """Show the patrol route being drawn on the current map only."""
        current = self.current_tab
        for index in range(self.tabs.count()):
            tab = self.tabs.widget(index)
            if isinstance(tab, MapTab):
                tab.canvas.set_route(route if tab is current else None)

    def _on_hover(self, x: int, y: int) -> None:
        tab = self.current_tab
        if tab is not None:
            self.tools.set_document(self.project.map_by_uuid(tab.map_uuid))
            self.tools.hover(x, y)
        if tab is None or x < 0:
            self._cell_label.setText("")
            return
        document = self.project.map_by_uuid(tab.map_uuid)
        self._hover_cell = (x, y)
        words = [document.cell(plane, x, y) for plane in range(3)]
        names = []
        if self.catalog is not None:
            for plane in (0, 1):
                if plane == 1 and words[1] < 19:
                    continue
                entry = self.catalog.for_value(plane, words[plane])
                if entry is not None:
                    names.append(entry.name)
        self._cell_label.setText(f"({x}, {y})  {words[0]} / {words[1]} / {words[2]}"
                                 + (f"  {' · '.join(names)}" if names else ""))

    # -- palette ----------------------------------------------------------

    def _refresh_palette(self) -> None:
        if self.catalog is None:
            return
        index = self.palette_tabs.currentIndex()
        if index < 0:
            return
        _, category = PALETTE_TABS[index]
        if category == "prefabs":
            self._refresh_prefabs()
            return
        entries = CatalogFilter(self.catalog).entries(
            category=category, query=self.search.text()
        )
        if self.used_only.isChecked():
            used = self.used_values()
            entries = [entry for entry in entries
                       if any((entry.plane, value) in used for value in entry.values)]
        self.palette_models[category].set_entries(entries)

    def _refresh_prefabs(self) -> None:
        """Rebuild the compound-tool list, honoring the search box."""
        query = self.search.text().strip().lower()
        self.prefab_list.clear()
        for prefab in PREFABS:
            haystack = " ".join((prefab.name, prefab.description, prefab.key,
                                 str(prefab.writes[0].value))).lower()
            if query and query not in haystack:
                continue
            item = QListWidgetItem(prefab.name + ("  (Advanced)" if prefab.advanced else ""))
            item.setToolTip(f"{prefab.description}\n\nSource: {prefab.evidence}")
            item.setData(Qt.UserRole, prefab.key)
            self.prefab_list.addItem(item)

    def _on_prefab_chosen(self, current, _previous) -> None:
        if current is None:
            return
        prefab = prefab_by_key(current.data(Qt.UserRole))
        if prefab is None:
            return
        self.tools.cancel_pending()
        self.tools.set_prefab(prefab)
        self.action_rotate.setVisible(prefab.rotatable)
        self.select_tool(Tool.PREFAB)
        needs = "; ".join(check.why for check in prefab.preconditions) or "nothing in particular"
        self.selection_label.setText(
            f"<b>{prefab.name}</b><br>{prefab.description}<br><i>Needs: {needs}</i>"
        )

    def _on_prefab_refused(self, problems) -> None:
        self.problems.clear()
        for problem in problems:
            item = QListWidgetItem(f"{problem.severity.name.lower()}: {problem.message}"
                                   + (f"  [{problem.where}]" if problem.where else ""))
            item.setToolTip(problem.code)
            self.problems.addItem(item)

    #: Tools that act on whatever is armed rather than on the palette entry.
    #: Picking something from a palette has to take you out of these, or the
    #: next click still does the old thing.
    _ARMED_TOOLS = (Tool.POINTER, Tool.EYEDROPPER, Tool.PREFAB)

    def _on_palette_clicked(self, index) -> None:
        entry = index.data(EntryRole)
        if entry is None:
            return
        self.selected_entry = entry
        self.tools.set_entry(entry)

        # Choosing a wall means you want to paint a wall. A door or special
        # left armed from its tab would otherwise swallow the click and place
        # the old thing -- with the palette showing the new one as selected.
        self.tools.set_prefab(None)
        self.tools.cancel_pending()
        self.action_rotate.setVisible(False)
        if self.tools.tool in self._ARMED_TOOLS:
            self.select_tool(Tool.BRUSH)

        self.selection_label.setText(
            f"<b>{entry.name}</b> — raw {entry.value} on plane {entry.plane}"
            + (f"<br>{entry.description}" if entry.description else "")
        )

    selected_entry = None

    # -- view -------------------------------------------------------------

    def zoom_in(self) -> None:
        if self.current_tab:
            self.current_tab.canvas.zoom_in()

    def zoom_out(self) -> None:
        if self.current_tab:
            self.current_tab.canvas.zoom_out()

    def toggle_grid(self) -> None:
        for index in range(self.tabs.count()):
            self.tabs.widget(index).canvas.set_show_grid(self.action_grid.isChecked())

    def reset_layout(self) -> None:
        """Put the docks back. The escape hatch for a window restored off-screen."""
        self.settings.reset_layout()
        for dock, area in (
            (self.maps_dock, Qt.LeftDockWidgetArea),
            (self.palette_dock, Qt.RightDockWidgetArea),
            (self.inspector_dock, Qt.RightDockWidgetArea),
            (self.problems_dock, Qt.BottomDockWidgetArea),
        ):
            dock.setFloating(False)
            dock.show()
            self.addDockWidget(area, dock)
        self.resources_dock.setFloating(False)
        self.addDockWidget(Qt.RightDockWidgetArea, self.resources_dock)
        self.tabifyDockWidget(self.palette_dock, self.resources_dock)
        self.palette_dock.raise_()
        self.resize(1180, 760)
        QTimer.singleShot(0, self._default_dock_sizes)
        self.statusBar().showMessage("Layout reset", 4000)

    # -- editing ----------------------------------------------------------

    def run_command(self, command) -> None:
        """The one path by which a map changes."""
        self.project = self.history.do(self.project, command)
        self.pool.set_revision(self.project.revision)
        # Orphans first: a command can now remove a map, so a tab may be
        # pointing at something that is no longer in the project, and both the
        # canvas sync and the tool below would ask for it by id.
        self._close_orphan_tabs()
        self._sync_canvases()
        tab = self.current_tab
        if tab is None:
            self.tools.set_document(None)
        else:
            # The controller holds a snapshot, and an immutable document means
            # the old one would keep reporting the words from before this edit.
            self.tools.set_document(self.project.map_by_uuid(tab.map_uuid))
        self._refresh_title()
        if command.resources or command.campaigns:
            self._sync_resources()
        # Continuous validation: the cheap rules on every edit, the whole set
        # when the hand stops. Painting a wall must not pay for a reachability
        # flood, and a panel that only updates on F8 is a panel describing a
        # map that no longer exists.
        self._revalidate_soon()
        self._refresh_snapshot_staleness()
        self.project_changed.emit()

    def _revalidate_soon(self) -> None:
        if self.catalog is None or self.current_tab is None:
            return
        self.validate(quick=True)
        self._validate_timer.start()

    def undo(self) -> None:
        self.project = self.history.undo(self.project)
        self._close_orphan_tabs()
        self._sync_canvases()
        self._refresh()

    def redo(self) -> None:
        self.project = self.history.redo(self.project)
        self._close_orphan_tabs()
        self._sync_canvases()
        self._refresh()

    def _close_orphan_tabs(self) -> None:
        """Shut tabs whose map is no longer in the project.

        A command can now add or remove a map, so undo and redo can leave a tab
        pointing at something that is not there.
        """
        alive = {document.uuid for document in self.project.maps}
        for index in range(self.tabs.count() - 1, -1, -1):
            if self.tabs.widget(index).map_uuid not in alive:
                self.tabs.removeTab(index)

    # -- clipboard ----------------------------------------------------------

    clipboard = None

    def copy_selection(self) -> bool:
        """Copy the pointer selection. All three planes, always.

        Copying only what is visible would silently drop the zone under the
        floor and whatever plane 2 holds, and the paste would look right until
        somebody played it.
        """
        tab = self.current_tab
        selection = self.tools.selection
        if tab is None or selection.empty:
            self.statusBar().showMessage("Select an area first, with the Select tool", 4000)
            return False
        document = self.project.map_by_uuid(tab.map_uuid)
        try:
            self.clipboard = copy_region(
                document, selection.x, selection.y, selection.width, selection.height
            )
        except ValueError as error:
            self._error("Could not copy", str(error))
            return False
        self.statusBar().showMessage(
            f"Copied {self.clipboard.width}x{self.clipboard.height} cells", 4000)
        return True

    def paste_clipboard(self) -> bool:
        """Paste at the selection's corner, as one undo step."""
        tab = self.current_tab
        if tab is None or self.clipboard is None:
            self.statusBar().showMessage("Nothing copied yet", 4000)
            return False
        document = self.project.map_by_uuid(tab.map_uuid)
        selection = self.tools.selection
        writes = paste_writes(document, self.clipboard, selection.x, selection.y)
        if not writes:
            self.statusBar().showMessage("That paste would land outside the map", 4000)
            return False
        from obcedit_core.commands import write_words

        self.run_command(write_words(document, writes, label="Paste"))
        self.statusBar().showMessage(f"Pasted at ({selection.x}, {selection.y})", 4000)
        return True

    def rotate_clipboard(self) -> bool:
        """Turn what was copied, rewriting facings through the catalog."""
        if self.clipboard is None:
            self.statusBar().showMessage("Nothing copied yet", 4000)
            return False
        self.clipboard = rotate_clip(self.clipboard, 1, self.catalog)
        self.statusBar().showMessage(
            f"Turned; now {self.clipboard.width}x{self.clipboard.height}", 4000)
        return True

    def flip_clipboard_h(self) -> bool:
        return self._flip("horizontal")

    def flip_clipboard_v(self) -> bool:
        return self._flip("vertical")

    def _flip(self, axis: str) -> bool:
        if self.clipboard is None:
            self.statusBar().showMessage("Nothing copied yet", 4000)
            return False
        self.clipboard = flip_clip(self.clipboard, axis, self.catalog)
        self.statusBar().showMessage(f"Flipped {axis}", 4000)
        return True

    # -- statistics ---------------------------------------------------------

    def map_statistics(self) -> dict:
        """What is actually on the open map, counted by what it means."""
        tab = self.current_tab
        if tab is None or self.catalog is None:
            return {}
        document = self.project.map_by_uuid(tab.map_uuid)
        from collections import Counter

        counts = Counter()
        for plane in (0, 1):
            for value in document.planes.planes[plane]:
                if plane == 1 and value < 19:
                    continue
                if plane == 0 and rules.is_floor(value):
                    continue
                entry = self.catalog.for_value(plane, value)
                counts[entry.category if entry else "unknown"] += 1
        floor = sum(1 for v in document.planes.planes[0] if rules.is_floor(v))
        reach = reachability.analyze(document, self.catalog)
        return {
            "cells": document.planes.cell_count,
            "floor": floor,
            "walls": counts.get("walls", 0),
            "specials": counts.get("specials", 0),
            "objects": counts.get("objects", 0),
            "enemies": counts.get("enemies", 0),
            "starts": counts.get("starts", 0),
            "markers": counts.get("markers", 0),
            "unknown": counts.get("unknown", 0),
            "quota": reach.quota,
        }

    def show_statistics(self) -> None:
        stats = self.map_statistics()
        if not stats:
            self.statusBar().showMessage("Open a map first", 4000)
            return
        QMessageBox.information(
            self, "Map statistics",
            "\n".join(f"{name.title():10} {count}" for name, count in stats.items()),
        )

    def used_values(self) -> set:
        """Every `(plane, value)` the open map actually uses, for the filter."""
        tab = self.current_tab
        if tab is None:
            return set()
        document = self.project.map_by_uuid(tab.map_uuid)
        used = set()
        for plane in (0, 1):
            used |= {(plane, value) for value in set(document.planes.planes[plane])}
        return used

    # -- validation and playtest -------------------------------------------

    def _next_session_id(self) -> str:
        """A short id, unique within this run of the editor.

        The engine echoes it on every event, which is what lets the reader tell
        this launch's output from another's -- and from anything the map under
        test decided to print. A counter is enough: two editors are two
        workspaces, and a stale directory from a previous run is not a session
        anything is still listening to.
        """
        self._session_counter += 1
        return f"obcedit-{self._session_counter:04d}"

    # -- snapshot ---------------------------------------------------------

    def set_camera(self, x: float, y: float) -> None:
        """The Camera tool put the camera here. Keeps whatever angle it had.

        There is exactly one camera. Placing it again moves it rather than
        adding a second, which is what a single viewpoint should do -- and now
        that it is drawn, moving it is something you can see happen.
        """
        tab = self.current_tab
        # A camera placed on another floor keeps its angle but not its place.
        same_map = tab is not None and self.camera_map == tab.map_uuid
        angle = self.camera.angle if (self.camera is not None and same_map) else 0.0
        self.camera = Camera(x, y, angle)
        self.camera_map = tab.map_uuid if tab is not None else ""
        self._show_camera()
        self.snapshot_status.setText(
            f"Camera at {self.camera.describe()} — Take a snapshot")

    def _show_camera(self) -> None:
        """Draw the camera on the floor it is on, and on no other.

        Called from everywhere the camera or the open map can change. Every
        canvas is told, rather than only the current one, so switching tabs
        cannot leave a camera drawn on a floor it is not on.
        """
        for index in range(self.tabs.count()):
            tab = self.tabs.widget(index)
            if not isinstance(tab, MapTab):
                continue
            here = self.camera if tab.map_uuid == self.camera_map else None
            tab.canvas.set_camera(here)

    def turn_camera(self) -> None:
        if self.camera is None:
            self.snapshot_status.setText("Place the camera first: pick the Camera tool "
                                         "and click a floor tile")
            return
        self.camera = Camera(self.camera.x, self.camera.y,
                             (self.camera.angle + 90.0) % 360.0)
        self._show_camera()
        self.snapshot_status.setText(f"Camera at {self.camera.describe()}")

    def _snapshot_cache_dir(self) -> Path:
        """Derived, disposable, and inside the workspace.

        A snapshot is a picture of the user's own game data, so it is theirs
        and it stays on their machine -- never in the project file, which is a
        thing people share.
        """
        root = Path(self.settings.profile.workspace_dir or Path.home())
        return root / ".obcedit-snapshots"

    def take_snapshot(self) -> bool:
        """Ask the engine for one frame from the camera, and show it."""
        tab = self.current_tab
        if tab is None:
            self.snapshot_status.setText("Open a map first")
            return False
        if self.camera is None:
            self.snapshot_status.setText("Place the camera first: pick the Camera tool "
                                         "and click a floor tile")
            return False
        if self.camera_map != tab.map_uuid:
            # One camera, and it is on a floor. Snapshotting the map in front
            # of you using a camera placed on a different one would draw
            # somewhere nobody asked about, and check_camera below would judge
            # the coordinates against the wrong geometry -- passing or failing
            # for reasons that have nothing to do with either map.
            self.snapshot_status.setText(
                "The camera is on another floor. Click here with the Camera "
                "tool to move it to this one.")
            return False
        document = self.project.map_by_uuid(tab.map_uuid)
        try:
            check_camera(document, self.camera)
        except ObcEditError as error:
            # Checked first, because it needs no engine and it is the thing the
            # user just did. Refused here rather than launched: the engine
            # would draw the inside of a wall without complaining, and the
            # cache would keep the result.
            self.snapshot_status.setText(str(error.diagnostic.message))
            return False

        profile = self.settings.profile
        if not profile.engine_path or not profile.data_dir:
            self.snapshot_status.setText("Tools -> Setup, and choose an engine and "
                                         "your game data")
            return False

        cache = self._snapshot_cache_dir()
        try:
            build = preview_mod(self.project, document.uuid, store=self.resource_store())
            key = snapshot_key(
                engine=profile.engine_path,
                data_fingerprint=profile.data_fingerprint,
                export_digest=build.digest,
                camera=self.camera,
            )
            image = cache / f"{key}.png"
            if image.is_file() and looks_like_a_world(image):
                self._show_snapshot(image, key, cached=True)
                return True

            guard = OutputGuard(protected_roots=(Path(profile.data_dir),))
            mod = write_mod(build, cache / key / "mod", guard=guard)
            # `snapshot_arguments` carries the camera as --start, so no start
            # is passed here.
            plan = build_launch_plan(
                executable=profile.engine_path,
                data_dir=profile.data_dir,
                mod=mod,
                floor=0,
                session_dir=cache / key,
                extra=snapshot_arguments(self.camera, image),
                export_digest=build.digest,
                revision=self.project.revision,
            )
        except (OSError, ObcEditError) as error:
            self.snapshot_status.setText(f"Could not prepare a snapshot: {error}")
            return False

        self.snapshot_status.setText(f"Drawing {self.camera.describe()}…")
        QApplication.processEvents()

        import subprocess

        try:
            done = subprocess.run(plan.argv, cwd=str(plan.cwd), capture_output=True,
                                  text=True, timeout=120)
        except (OSError, subprocess.SubprocessError) as error:
            self.snapshot_status.setText(f"The engine did not run: {error}")
            return False

        output = (done.stdout or "") + (done.stderr or "")
        line = next((l for l in output.splitlines() if "shot" in l.lower()), "")
        if not image.is_file() or not looks_like_a_world(image):
            # Nothing is cached on failure. A blank frame kept under this key
            # would be handed back for ever, and it is the exact failure this
            # project has shipped before -- a gate that passed while comparing
            # a black frame nobody had looked at.
            image.unlink(missing_ok=True)
            refused = next((l for l in output.splitlines() if l.startswith("OBCR error")), "")
            self.snapshot_status.setText(
                "OBCR drew no world. " + (refused or "See the Problems panel."))
            self._note_problem(f"Snapshot failed: {refused or output[-200:]}")
            return False

        self._show_snapshot(image, key, cached=False, detail=line)
        return True

    def _show_snapshot(self, image: Path, key: str, *, cached: bool,
                       detail: str = "") -> None:
        pixmap = QPixmap(str(image))
        if not pixmap.isNull():
            self.snapshot_view.setPixmap(pixmap.scaled(
                self.snapshot_view.width() or pixmap.width(), pixmap.height(),
                Qt.KeepAspectRatio, Qt.SmoothTransformation))
        self._snapshot_key = key
        self._snapshot_revision = self.project.revision
        tic = ""
        for token in detail.split():
            if token.startswith("tic="):
                tic = token[4:]
        # Still "Camera at ...". Dropping those two words after a snapshot
        # made a working camera read as a forgotten one, which is most of what
        # "the camera disappears after taking a snapshot" turned out to be.
        self.snapshot_status.setText(
            f"Camera at {self.camera.describe()}"
            + (f", tic {tic}" if tic else "")
            + (" (from the cache)" if cached else ""))
        self.snapshot_dock.show()
        self.snapshot_dock.raise_()

    def _refresh_snapshot_staleness(self) -> None:
        """Say when the picture is of an older version of the map.

        It is not thrown away: an out-of-date snapshot is still useful to look
        at, as long as nobody is told it is current.
        """
        if not self._snapshot_key:
            return
        if self._snapshot_revision != self.project.revision:
            text = self.snapshot_status.text()
            if "edited since" not in text:
                self.snapshot_status.setText(text + " — edited since")

    def _map_profile(self, document) -> "Profile":
        """Which rules apply to this map: an arena's if the campaign plays it as
        a network arena, otherwise the slot's (docs/MOD_ASSETS_AND_ARENAS.md)."""
        return profile_for_map(document, self._arena_maps())

    def _arena_maps(self) -> set:
        try:
            return self.current_campaign().arena_maps() if self.project.campaign else set()
        except (ObcEditError, ValueError, TypeError):
            return set()

    # -- resources (mod format 2) -----------------------------------------

    def _build_resources_dock(self) -> None:
        """The Resources panel, tabbed with the palette."""
        self.resources_panel = ResourcesPanel(self)
        dock = QDockWidget("Resources", self)
        dock.setObjectName("resources-dock")
        dock.setWidget(self.resources_panel)
        self.addDockWidget(Qt.RightDockWidgetArea, dock)
        self.tabifyDockWidget(self.palette_dock, dock)
        self.palette_dock.raise_()
        self.resources_dock = dock
        action = dock.toggleViewAction()
        action.setText("&Resources (art, sounds, music)")
        self.action_resources = action
        self.view_menu.insertAction(self.action_reset_layout, action)

    def resource_store(self):
        """Where the open project's replacement files live: beside the project
        (`<name>.assets/`), or in the workspace until it is first saved."""
        from obcedit_core.resources import store_for

        workspace = self.settings.profile.workspace_dir or str(
            Path(self.settings.recovery_dir).parent)
        return store_for(self.project_path, workspace=workspace, project_uuid=self.project.uuid)

    def music_names(self) -> list[str]:
        return [str(r.target) for r in self.project.resources if r.kind == "music"]

    def _sync_resources(self, *, force: bool = False) -> None:
        """Show the project's replacement art everywhere after it changed."""
        store = self.resource_store()
        state = (self.project.resources, str(store.root), self.thumbnails.available)
        if state == self._resource_state and not force:
            return
        self._resource_state = state
        if self.thumbnails.set_overrides(self.project.resources, store) or force:
            for model in self.palette_models.values():
                model.invalidate_icons()
            self._apply_wall_colors()
        if getattr(self, "resources_panel", None) is not None:
            self.resources_panel.refresh()

    def _carry_resources(self, previous) -> None:
        """Before writing the project somewhere new, copy its files along."""
        target = self.resource_store()
        if previous is not None and previous.root != target.root:
            target.adopt_all(previous)

    def validate(self, *, quick: bool = False) -> list:
        """Check the open map and fill the Problems panel.

        `quick` runs only the rules whose answer is local to the cells that
        changed and keeps the previous answer for the rest. That is what runs
        while somebody is painting; the full pass runs when they stop, and on
        anything that has to be right, such as an export.
        """
        tab = self.current_tab
        if tab is None or self.catalog is None:
            self.problems.clear()
            self._problems = []
            self._problems_revision = -1
            return []
        document = self.project.map_by_uuid(tab.map_uuid)
        profile = self._map_profile(document)
        if quick:
            problems = validate_local(document, self.catalog, profile=profile,
                                      previous=self._problems)
        else:
            problems = list(validate_map(document, self.catalog, profile=profile))
            self._reach = reachability.analyze(document, self.catalog)
        self._problems = problems
        self._problems_revision = self.project.revision
        self._repaint_problems()
        self.statusBar().showMessage(summarize(problems), 6000)
        return problems

    _reach = None

    def _reach_summary(self) -> str:
        """Whether the floor can be finished, as reachability sees it."""
        reach = self._reach
        if reach is None:
            return ""
        if not reach.started:
            return "no player start"
        enemies = len(reach.enemies)
        text = (f"exit needs {reach.quota} of {enemies} kills; "
                f"{len(reach.enemies_reached)} reachable")
        if reach.victor_reached:
            text = "Victor is reachable; " + text
        return ("completable: " if reach.completable else "NOT completable: ") + text

    def _repaint_problems(self) -> None:
        """Refill the list from the last result, honoring the severity floor."""
        floor = self.problem_filter.currentData()
        self.problems.clear()
        shown = 0
        for problem in self._problems:
            if floor is not None and problem.severity < floor:
                continue
            shown += 1
            item = QListWidgetItem(f"{problem.severity.name.lower()}: {problem.message}"
                                   + (f"  [{problem.where}]" if problem.where else ""))
            item.setToolTip(f"{problem.code}\n{problem.message}")
            item.setData(Qt.UserRole, problem.cell)
            item.setData(Qt.UserRole + 1, problem.fix)
            self.problems.addItem(item)
        hidden = len(self._problems) - shown
        stale = self._problems_revision != self.project.revision
        parts = [summarize(self._problems)]
        reach = self._reach_summary()
        if reach:
            parts.append(reach)
        if hidden:
            parts.append(f"{hidden} hidden by the filter")
        if stale:
            # Said rather than silently shown: a panel describing a map that
            # has since changed is worse than an empty one, because it looks
            # current.
            parts.append("from an earlier edit — press F8 to recheck")
        self.problem_status.setText("  ·  ".join(parts))
        self._on_problem_selected(self.problems.currentItem(), None)

    def _on_problem_selected(self, current, _previous) -> None:
        fix = current.data(Qt.UserRole + 1) if current is not None else ""
        self.problem_fix.setEnabled(bool(fix))
        self.problem_fix.setText(fix_label(fix) if fix else "Fix this")

    def apply_fix(self) -> int:
        """Apply the selected problem's repair, as one undoable command."""
        from obcedit_core.commands import write_words

        item = self.problems.currentItem()
        tab = self.current_tab
        if item is None or tab is None:
            return 0
        fix = item.data(Qt.UserRole + 1)
        if not fix:
            return 0
        document = self.project.map_by_uuid(tab.map_uuid)
        writes = fix_writes(fix, document)
        if not writes:
            return 0
        self.run_command(write_words(document, writes, label=fix_label(fix)))
        self.validate()
        return len(writes)

    def playtest(self, start: tuple[int, int, int] | None = None) -> bool:
        """Build a one-floor preview mod of the open map and play it in OBCR.

        The preview goes in a session directory in the workspace, never beside
        the game data, and OBCR checks it (`--check-mod`) before it is
        launched: a mod the engine would refuse is reported here, with the
        reason, instead of a window that appears and vanishes.
        """
        tab = self.current_tab
        if tab is None:
            self._error("Nothing to test", "Open a map first.")
            return False
        profile = self.settings.profile
        if not profile.engine_path or not profile.data_dir:
            self._error("No engine configured",
                        "Tools -> Setup, and choose OBCR and your game data.")
            return False

        document = self.project.map_by_uuid(tab.map_uuid)
        blocking = [problem for problem in self.validate()
                    if problem.severity is Severity.ERROR]
        if blocking:
            self._error(
                "This map will not load yet",
                blocking[0].message
                + (f"\n\nAt {blocking[0].where}." if blocking[0].where else "")
                + (f"\n\nand {len(blocking) - 1} more; see the Problems panel."
                   if len(blocking) > 1 else ""),
            )
            return False

        # One directory per session, holding the preview mod, the engine's
        # config, its saves and the log. Never the player's own: a playtest
        # that wrote into those would change the game somebody else plays here.
        workspace = Path(profile.workspace_dir or Path.home()) / ".obcedit-playtest"
        session = self._next_session_id()
        session_dir = workspace / session
        try:
            build = preview_mod(self.project, document.uuid, store=self.resource_store())
            guard = OutputGuard(protected_roots=(Path(profile.data_dir),))
            mod = write_mod(build, session_dir / "mod", guard=guard)
        except (OSError, ObcEditError) as error:
            self._error("Could not build the preview mod", str(error))
            return False

        self.statusBar().showMessage("OBCR is checking the preview mod…", 4000)
        QApplication.processEvents()
        result = check_mod(profile.engine_path, mod, data_dir=profile.data_dir)
        self._show_mod_check(result)
        if not result.ok:
            detail = (result.errors[0].message if result.errors
                      else result.output.strip()[-300:] or f"exit code {result.exit_code}")
            self._error("OBCR refuses this map", detail)
            return False

        try:
            plan = build_launch_plan(
                executable=profile.engine_path,
                data_dir=profile.data_dir,
                mod=mod,
                floor=0,
                start=start,
                session_dir=session_dir,
                # What was tested, so the log can be matched back to it.
                export_digest=build.digest,
                revision=self.project.revision,
            )
        except (OSError, ObcEditError) as error:
            self._error("Could not start a playtest", str(error))
            return False
        plan = dataclasses.replace(plan, session=session)
        where = (f" from {start[0]},{start[1]} facing {COMPASS[start[2]]}" if start else "")
        return self.start_session(plan, f"{document.name or document.label}{where}")

    _hover_cell: tuple[int, int] | None = None

    def playtest_here(self) -> bool:
        """Play the open map from the cell under the mouse, facing east."""
        tab = self.current_tab
        if tab is None:
            self._error("Nothing to test", "Open a map first.")
            return False
        cell = self._hover_cell
        if cell is None:
            self.statusBar().showMessage(
                "Point at a floor cell on the map, then press Shift+F5", 6000)
            return False
        document = self.project.map_by_uuid(tab.map_uuid)
        if not rules.is_floor(document.cell(0, *cell)):
            self._error("That is not floor",
                        f"The player cannot start inside cell {cell[0]},{cell[1]}: "
                        "point at a floor cell.")
            return False
        return self.playtest(start=(cell[0], cell[1], 1))

    # -- the playtest session ---------------------------------------------

    def start_session(self, plan, label: str) -> bool:
        """Run a launch plan under a QProcess and watch what it says.

        QProcess rather than subprocess because the editor has to stay usable
        while the game runs: output arrives on a signal, on the GUI thread,
        with nobody blocked on a pipe. The old code handed the process a file
        and forgot about it, which meant the editor could not tell a playtest
        that worked from one that never found the map -- and left the engine
        running when the editor closed.
        """
        if self._engine_alive():
            self._error("A playtest is already running",
                        "Stop it first, or let it finish.")
            return False
        # Let go of the previous one explicitly. Its finished() is connected to
        # a handler that writes into self.session, and self.session is about to
        # mean something else.
        if self.process is not None:
            try:
                self.process.finished.disconnect()
                self.process.readyReadStandardOutput.disconnect()
                self.process.errorOccurred.disconnect()
            except (RuntimeError, TypeError):
                pass
            self.process.deleteLater()
            self.process = None

        self.session = Session(plan)
        self._session_log_path = (plan.session_dir / "playtest.log"
                                  if plan.session_dir else None)
        self._session_lines = []

        process = QProcess(self)
        process.setProgram(str(plan.executable))
        process.setArguments(list(plan.arguments))
        process.setWorkingDirectory(str(plan.cwd))
        # One channel: the engine reports some failures on stderr -- "No player
        # 1 start!" among them -- and reading only stdout would lose exactly
        # the messages a playtest exists to surface.
        process.setProcessChannelMode(QProcess.ProcessChannelMode.MergedChannels)
        process.readyReadStandardOutput.connect(self._on_session_output)
        process.finished.connect(
            lambda code, _status: self._on_session_finished(code))
        process.errorOccurred.connect(self._on_session_error)

        self.process = process
        self.session.started()
        self._refresh_session_ui()
        process.start()
        if not process.waitForStarted(5000):
            # It may yet appear; do not leave it to.
            process.kill()
            process.waitForFinished(2000)
            self.session.finished(-1)
            self._error("The engine did not start",
                        f"{plan.executable}\n\n{process.errorString()}")
            self._refresh_session_ui()
            return False
        self.statusBar().showMessage(f"Testing {label} in OBCR", 6000)
        return True

    def _on_session_output(self) -> None:
        if self.process is None or self.session is None:
            return
        # Whole lines only. A read can land mid-line, and half of an event is
        # not an event -- the tail is kept for the next read rather than
        # parsed as if it were complete.
        text = bytes(self.process.readAllStandardOutput()).decode("utf-8", "replace")
        self._session_tail += text
        *lines, self._session_tail = self._session_tail.split("\n")
        for line in lines:
            self.session.feed(line)
            self._session_lines.append(line)
        if len(self._session_lines) > Session.LOG_LIMIT:
            # The same bound the Session keeps. A playtest prints for as long as
            # somebody plays, and this list is what the log file is written from.
            del self._session_lines[: len(self._session_lines) - Session.LOG_LIMIT]
        if lines:
            self._refresh_session_ui()

    def _on_session_error(self, _error) -> None:
        if self.process is not None:
            self._note_problem(f"Playtest: {self.process.errorString()}")

    def _on_session_finished(self, code: int) -> None:
        if self.session is None:
            return
        if self._session_tail:
            self.session.feed(self._session_tail)
            self._session_lines.append(self._session_tail)
            self._session_tail = ""
        self.session.finished(code)
        self._write_session_log()
        self._refresh_session_ui()
        if self.session.state is SessionState.FAILED:
            # Said in the Test Log, not in a modal. This arrives on a signal
            # from a process ending, which can be at any moment -- including
            # while the user is mid-gesture in the editor -- and a dialog that
            # steals focus then is worse than a panel that is already showing
            # the answer. The dock raises itself so it cannot be missed.
            self._note_problem(f"Playtest failed: {self.session.describe()}")
            self.test_log_dock.show()
            self.test_log_dock.raise_()
            self.statusBar().showMessage(
                f"Playtest failed: {self.session.describe()}", 12000)

    def _write_session_log(self) -> None:
        """Keep the log beside the session, tagged with what was tested."""
        if self._session_log_path is None or self.session is None:
            return
        try:
            self._session_log_path.parent.mkdir(parents=True, exist_ok=True)
            header = [
                f"# session {self.session.plan.session}",
                f"# revision {self.session.plan.revision}",
                f"# export {self.session.plan.export_digest}",
                f"# {self.session.plan.described()}",
                "",
            ]
            self._session_log_path.write_text(
                "\n".join(header + self._session_lines) + "\n", encoding="utf-8")
        except OSError:
            pass  # a log we could not write must not break the run it describes

    def stop_session(self) -> bool:
        """Ask the engine to close, and make sure it does."""
        if not self._engine_alive():
            return False
        if self.session is not None and self.session.running:
            self.session.stopping()
        self._refresh_session_ui()
        self.process.terminate()
        if not self.process.waitForFinished(3000):
            # It did not go. A playtest left running is a window the user
            # cannot get rid of and a process the next launch will refuse to
            # start alongside.
            self.process.kill()
            self.process.waitForFinished(2000)
        return True

    def _engine_alive(self) -> bool:
        """Whether a process is actually still there.

        Asked of the process, never of the session: the session goes terminal
        the moment the engine says session-result, which is before it has
        finished exiting. Anything that trusted the session alone would call a
        live process dead -- and that is the definition of the orphan this is
        supposed to prevent.
        """
        return (self.process is not None
                and self.process.state() != QProcess.ProcessState.NotRunning)

    def reconcile_orphan(self) -> bool:
        """At shutdown: never leave a playtest running without an editor.

        The engine is a child of this process. Closing the editor and leaving
        it behind means a window nobody owns, still writing to a session
        directory the editor may reuse.
        """
        if not self._engine_alive():
            return False
        if self.session is not None and self.session.running:
            self.session.stopping()
        self.process.terminate()
        if not self.process.waitForFinished(3000):
            self.process.kill()
            self.process.waitForFinished(2000)
        return True

    def _refresh_session_ui(self) -> None:
        running = self.session is not None and self.session.running
        self.action_stop_test.setEnabled(running)
        if self.session is None:
            self.test_log_status.setText("No playtest has been run yet")
            return
        self.test_log_status.setText(
            f"{self.session.state.value}: {self.session.describe()}")
        self.test_log.clear()
        for line in self._session_lines[-400:]:
            self.test_log.addItem(line)
        self.test_log.scrollToBottom()

    def _sync_canvases(self) -> None:
        for index in range(self.tabs.count()):
            tab = self.tabs.widget(index)
            try:
                tab.canvas.set_document(self.project.map_by_uuid(tab.map_uuid))
            except ObcEditError:
                pass  # the map was removed; the tab closes on the next refresh

    # -- setup ------------------------------------------------------------

    def run_setup(self) -> bool:
        """Show first-run setup. Returns whether a profile was saved.

        `QDialog.DialogCode.Accepted`, not `dialog.Accepted`: in PySide6 the
        enum lives on the class, and reading it off an instance raises. That
        is easy to write and impossible to see until the dialog closes.
        """
        from PySide6.QtWidgets import QDialog

        from .first_run import FirstRunDialog

        dialog = FirstRunDialog(self.settings.profile, self)
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return False
        profile = dialog.profile()
        self.settings.profile = profile
        self.settings.sync()
        self.thumbnails.clear()
        self.open_assets(profile)
        self._sync_resources(force=True)
        self._refresh_palette()
        self.statusBar().showMessage("Setup saved", 4000)
        return True

    # -- housekeeping -----------------------------------------------------

    def _refresh(self) -> None:
        self._sync_resources()
        self.map_list.clear()
        for document in self.project.maps:
            item = QListWidgetItem(f"{document.name or '(unnamed)'}")
            item.setToolTip(f"{document.label}, {document.width}x{document.height}")
            self.map_list.addItem(item)
        self._refresh_palette()
        self._refresh_title()

    def _refresh_title(self) -> None:
        name = self.project_path.name if self.project_path else "Untitled"
        marker = " •" if self.project.dirty else ""
        self.setWindowTitle(f"{name}{marker} — OBCEdit")
        self.action_undo.setEnabled(self.history.can_undo)
        self.action_redo.setEnabled(self.history.can_redo)
        self.action_undo.setText(f"&Undo {self.history.undo_label}".rstrip())
        self.action_redo.setText(f"&Redo {self.history.redo_label}".rstrip())

    def _note_problem(self, message: str) -> None:
        self.problems.addItem(message)

    def _error(self, title: str, detail: str) -> None:
        self._note_problem(f"{title}: {detail}")
        QMessageBox.warning(self, title, detail)

    def _start_directory(self) -> str:
        profile = self.settings.profile
        return profile.workspace_dir or str(Path.home())

    def _confirm_discard(self) -> bool:
        if not self.project.dirty:
            return True
        answer = QMessageBox.question(
            self, "Unsaved changes",
            "This project has unsaved changes. Save before continuing?",
            QMessageBox.Save | QMessageBox.Discard | QMessageBox.Cancel,
        )
        if answer == QMessageBox.Save:
            return self.save_project()
        return answer == QMessageBox.Discard

    def _restore_layout(self) -> None:
        geometry, state = self.settings.layout()
        if geometry:
            self.restoreGeometry(geometry)
        if state:
            self.restoreState(state)
        else:
            QTimer.singleShot(0, self._default_dock_sizes)

    def _default_dock_sizes(self) -> None:
        """First run and Reset layout: the palette gets most of the right-hand
        column, since it is where an author spends their time."""
        right = [self.palette_dock, self.inspector_dock, self.snapshot_dock]
        self.resizeDocks(right, [460, 200, 180], Qt.Vertical)
        self.resizeDocks([self.palette_dock], [440], Qt.Horizontal)
        self.resizeDocks([self.maps_dock], [200], Qt.Horizontal)
        self.resizeDocks([self.problems_dock], [150], Qt.Vertical)

    def closeEvent(self, event) -> None:
        if not self._confirm_discard():
            event.ignore()
            return
        self.settings.save_layout(self.saveGeometry(), self.saveState())
        self.settings.sync()
        self.pool.cancel_all()
        self.pool.wait(2000)
        # Never leave a playtest running without an editor: the engine is a
        # child of this process, and closing without it means a window nobody
        # owns, still writing into a session directory the next launch reuses.
        self.reconcile_orphan()
        event.accept()

    def about(self) -> None:
        from obcedit_core import __version__

        QMessageBox.about(
            self, "About OBCEdit",
            f"<b>OBCEdit {__version__}</b><br>"
            "The level editor that ships with OBCR, for making Operation Body "
            "Count mods.<br><br>"
            "Licensed under the GNU GPL, version 3 or later.<br><br>"
            "Operation Body Count and its data belong to their rights holders and "
            "are not included here. The editor reads your own copy and leaves it "
            "alone.",
        )
