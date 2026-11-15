# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Turning a catalog entry into a picture, using the user's own game data.

The catalog says *which* wall page or sprite a thing is. This is where that
becomes pixels — read from the copy of Operation Body Count the user pointed
the editor at (`GFXTILES.BC`, and the palette inside `COUNT.EXE` or
`BCCD.EXE`), decoded in memory, and cached in memory. Nothing is written to
disk.

That is the licensing boundary made concrete: the catalog is a file this
project can ship, and the artwork is not, so the artwork is never in the
catalog, never in a project file, and never in a cache on disk. It exists
while the editor is running and then it is gone.

An entry whose page will not decode still gets a tile — a labeled placeholder
rather than a gap. A palette that silently omits broken items is a palette that
hides the fact something is wrong with the data.

A project's replacement art (mod format 2) is drawn in place of the page or
sprite it replaces, converted to the game palette by the same rule OBCR uses
(`ArtOverrides`), so the palette, the canvas colours and the Resources panel
show what the game will draw.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from PySide6.QtCore import QSize, Qt
from PySide6.QtGui import QColor, QImage, QPainter, QPixmap

from obcedit_core.assets import (
    AssetError,
    ImageCache,
    average_color,
    load_palette,
    parse_gfx_header,
    sprite_rgba,
    wall_rgb,
)
from obcedit_core import artimport
from obcedit_core.catalog import Catalog, CatalogEntry
from obcedit_core.discovery import find_file, game_executable
from obcedit_core.resources import SPRITE, WALL


@dataclass
class AssetSource:
    """The user's game data, opened once and read from many times."""

    data_dir: Path
    palette: list[int]
    gfx: bytes
    header: object
    fingerprint: str = ""

    @classmethod
    def open(cls, data_dir: Path | str, *, fingerprint: str = "") -> "AssetSource":
        data_dir = Path(data_dir)
        executable = game_executable(data_dir)
        gfx_path = find_file(data_dir, "GFXTILES.BC")
        if executable is None or gfx_path is None:
            raise AssetError(f"{data_dir} has no COUNT.EXE/BCCD.EXE and GFXTILES.BC")
        palette = load_palette(executable.read_bytes(), executable.name.upper())
        gfx = gfx_path.read_bytes()
        return cls(data_dir, palette, gfx, parse_gfx_header(gfx), fingerprint)

    def wall(self, page: int) -> bytes:
        if not 0 <= page < self.header.sprite_start:
            raise AssetError(f"wall page {page} is outside 0..{self.header.sprite_start - 1}")
        return wall_rgb(self.header.chunk(self.gfx, page), self.palette)

    def sprite(self, page: int) -> bytes:
        index = self.header.sprite_start + page
        if not self.header.sprite_start <= index < self.header.sound_start:
            raise AssetError(f"sprite page {page} is outside the artwork")
        return sprite_rgba(self.header.chunk(self.gfx, index), self.palette)


class ArtOverrides:
    """The project's replacement walls and sprites, as thumbnail pixels.

    Converted lazily (off the GUI thread, from `pixels_for`) and kept: an
    image is 4,096 pixels, so a few hundred replacements stay small. `tag`
    changes whenever the set of replacements does, which is what keys the
    pixmap cache.
    """

    def __init__(self, resources=(), store=None, palette=None) -> None:
        self.resources = {(r.kind, r.target): r for r in resources
                          if r.kind in (WALL, SPRITE)}
        self.store = store
        self.palette = palette
        self._quantizer = artimport.Quantizer(palette) if palette else None
        self._pixels: dict[tuple, bytes] = {}
        self.tag = ",".join(sorted(f"{k}{t}:{r.sha256[:12]}"
                                   for (k, t), r in self.resources.items())) or "none"

    def __contains__(self, key) -> bool:
        return key in self.resources

    def pixels(self, kind: str, target: int) -> bytes | None:
        """Preview pixels for a replaced page or sprite, or None if not replaced
        (or its file cannot be read -- the original is shown, and the problem
        is reported where resources are listed)."""
        resource = self.resources.get((kind, target))
        if resource is None or self.store is None:
            return None
        found = self._pixels.get((kind, target))
        if found is not None:
            return found
        try:
            image = artimport.check_art(self.store.read(resource), resource.source_name)
        except Exception:  # noqa: BLE001 - a bad file falls back to the original art
            return None
        sprite = kind == SPRITE
        if self._quantizer is not None:
            found = artimport.preview(image, self.palette, sprite=sprite,
                                      quantizer=self._quantizer)
        else:
            found = artimport.raw_preview(image, sprite=sprite)
        self._pixels[(kind, target)] = found
        return found


class ThumbnailFactory:
    """Catalog entry to `QPixmap`, cached by key, size and data fingerprint.

    The fingerprint is in the key because a user may repoint the editor at a
    different copy of the game -- a different rip, a patched executable -- and
    a cache that ignored that would show the old pictures indefinitely.
    """

    def __init__(self, source: AssetSource | None = None, *, budget_bytes: int = 32 << 20,
                 catalog: Catalog | None = None) -> None:
        self.source = source
        #: For floors: their picture is the floor-set page the catalog names.
        self.catalog = catalog
        self._pixels = ImageCache(budget_bytes)
        self._pixmaps: dict[str, QPixmap] = {}
        self.overrides = ArtOverrides()

    @property
    def available(self) -> bool:
        return self.source is not None

    def set_overrides(self, resources, store) -> bool:
        """Use these replacement records. Returns whether anything changed."""
        palette = getattr(self.source, "palette", None)
        fresh = ArtOverrides(resources, store, palette)
        if fresh.tag == self.overrides.tag and (store is None or self.overrides.store is None
                                               or fresh.store.root == self.overrides.store.root):
            return False
        self.overrides = fresh
        self._pixmaps.clear()
        return True

    def replaced(self, entry: CatalogEntry) -> bool:
        page = self.page_for(entry)
        if page is not None:
            return (WALL, page) in self.overrides
        return entry.sprite is not None and (SPRITE, entry.sprite) in self.overrides

    def key_for(self, entry: CatalogEntry, size: int) -> str:
        fingerprint = self.source.fingerprint if self.source else "none"
        replaced = self.overrides.tag if self.replaced(entry) else ""
        return f"{fingerprint}:{entry.key}:{size}:{replaced}"

    def pixels_for(self, entry: CatalogEntry) -> tuple[bytes, int, bool]:
        """Decoded pixels, their edge length, and whether they have alpha.

        Runs off the GUI thread. Returns buffers, not Qt objects: a QPixmap may
        only be made on the GUI thread, and a worker that built one would
        either crash or quietly corrupt the display.
        """
        if self.source is None:
            raise AssetError("no game data is open")

        page = self.page_for(entry)
        if page is not None:
            replaced = self.overrides.pixels(WALL, page)
            if replaced is not None:
                return replaced, 64, False
            return self._pixels.fetch(f"wall:{page}", lambda: self.source.wall(page)), 64, False
        if entry.sprite is not None:
            page = entry.sprite
            replaced = self.overrides.pixels(SPRITE, page)
            if replaced is not None:
                return replaced, 64, True
            return self._pixels.fetch(f"sprite:{page}", lambda: self.source.sprite(page)), 64, True
        raise AssetError(f"{entry.key} has no artwork to draw")

    def page_for(self, entry: CatalogEntry) -> int | None:
        """The wall page that pictures a plane-0 entry: its own (walls, doors,
        specials) or, for a floor, its texture set's floor page."""
        if entry.plane != 0:
            return None
        if entry.page is not None:
            return int(entry.page)
        floor_set = entry.data.get("floor_set") if entry.data else None
        if floor_set is not None and self.catalog is not None:
            page = self.catalog.floor_set(int(floor_set)).get("floor_page")
            return int(page) if page is not None else None
        return None

    def image_from(self, pixels: bytes, edge: int, alpha: bool) -> QImage:
        """Wrap decoded bytes as a QImage. GUI thread only."""
        fmt = QImage.Format_RGBA8888 if alpha else QImage.Format_RGB888
        stride = edge * (4 if alpha else 3)
        # copy(): the QImage must not alias a Python buffer that may be freed.
        return QImage(pixels, edge, edge, stride, fmt).copy()

    def placeholder(self, entry: CatalogEntry, size: int) -> QPixmap:
        """A labeled tile for an entry with no artwork, or artwork that failed.

        Color comes from the entry's own key, so the same thing is the same
        color every time and a palette of placeholders is still scannable.
        """
        pixmap = QPixmap(size, size)
        tint = QColor.fromHsv((hash(entry.key) % 360), 70, 90)
        pixmap.fill(tint)
        painter = QPainter(pixmap)
        painter.setPen(QColor(235, 235, 235))
        painter.drawRect(0, 0, size - 1, size - 1)
        label = str(entry.value)
        font = painter.font()
        font.setPixelSize(max(8, size // 3))
        painter.setFont(font)
        painter.drawText(pixmap.rect(), Qt.AlignCenter, label)
        painter.end()
        return pixmap

    def cached(self, entry: CatalogEntry, size: int) -> QPixmap | None:
        return self._pixmaps.get(self.key_for(entry, size))

    def store(self, entry: CatalogEntry, size: int, pixmap: QPixmap) -> QPixmap:
        self._pixmaps[self.key_for(entry, size)] = pixmap
        return pixmap

    def swatch(self, entry: CatalogEntry, size: int = 16) -> QColor:
        """The average color of an entry's artwork, for a compact list."""
        try:
            pixels, _, alpha = self.pixels_for(entry)
        except AssetError:
            return QColor.fromHsv((hash(entry.key) % 360), 70, 90)
        if alpha:
            opaque = bytearray()
            for index in range(0, len(pixels), 4):
                if pixels[index + 3]:
                    opaque += pixels[index : index + 3]
            pixels = bytes(opaque)
        return QColor(*average_color(pixels))

    def clear(self) -> None:
        self._pixels.clear()
        self._pixmaps.clear()
        self.overrides = ArtOverrides(self.overrides.resources.values(), self.overrides.store,
                                      getattr(self.source, "palette", None))

    @property
    def memory_bytes(self) -> int:
        return self._pixels.size_bytes
