#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Mod format 2 in the editor: resources, imports, schema 2, export and audit.

docs/MOD_ASSETS_AND_ARENAS.md sections 2 and 4.
"""

from __future__ import annotations

import json
import struct
import tempfile
import tomllib
import unittest
import zipfile
from dataclasses import replace
from pathlib import Path

import asset_support as fx
from obc_support import CATALOG, REPO, room

from obcedit_core import artimport, audioimport
from obcedit_core.campaign import (
    ArenaEntry,
    Campaign,
    LevelEntry,
    parse_manifest,
    render_manifest,
    validate,
)
from obcedit_core.commands import History, rename_music, set_resource
from obcedit_core.document import ProjectDocument
from obcedit_core.engine_runner import CONVERT_UNAVAILABLE
from obcedit_core.errors import ExportError, ObcEditError, Severity
from obcedit_core.importing import ImportError_, import_art, import_music, import_sound, \
    music_name_for
from obcedit_core.modexport import audit_mod, build_mod, preview_mod, write_mod
from obcedit_core.project import deserialize, serialize
from obcedit_core.resources import (
    MUSIC,
    SOUND,
    SPRITE,
    WALL,
    Resource,
    ResourceError,
    ResourceStore,
    mod_path,
    parse_mod_path,
    store_for,
)
from obcedit_core.validation import ARENA, SINGLE_PLAYER, profile_for_map, validate_map

GOOD = ["#######", "#S.....", "#.....E", "#######"]
VECTORS = REPO / "docs" / "test-vectors" / "quantize.json"


class Fixture(unittest.TestCase):
    def setUp(self):
        self._dir = tempfile.TemporaryDirectory()
        self.root = Path(self._dir.name)
        self.store = ResourceStore(self.root / "store")

    def tearDown(self):
        self._dir.cleanup()

    def file(self, name: str, data: bytes) -> Path:
        path = self.root / "in" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return path

    def project(self, *maps, resources=()) -> ProjectDocument:
        project = ProjectDocument.create("Arena Pack", author="Jane Doe")
        for document in maps:
            project = project.added(document)
        return project.with_resources(resources)


# ---------------------------------------------------------------------------
# Records and the store
# ---------------------------------------------------------------------------


class Records(Fixture):
    def test_mod_paths_follow_section_2_1(self):
        self.assertEqual(mod_path(WALL, 12), "walls/012.png")
        self.assertEqual(mod_path(SPRITE, 450), "sprites/450.png")
        self.assertEqual(mod_path(SOUND, 7), "sounds/07.wav")
        self.assertEqual(mod_path(MUSIC, "theme.ogg"), "music/theme.ogg")

    def test_parse_mod_path(self):
        self.assertEqual(parse_mod_path("walls/12.png"), (WALL, 12))
        self.assertEqual(parse_mod_path("WALLS/012.PNG"), (WALL, 12))
        self.assertEqual(parse_mod_path("sprites/679.png"), (SPRITE, 679))
        self.assertEqual(parse_mod_path("sounds/98.wav"), (SOUND, 98))
        self.assertEqual(parse_mod_path("music/My_Theme-2.OGG"), (MUSIC, "My_Theme-2.OGG"))
        self.assertIsNone(parse_mod_path("README.txt"))
        self.assertIsNone(parse_mod_path("docs/notes.txt"))
        for bad in ("walls/136.png", "walls/1234.png", "walls/12.bmp", "sprites/680.png",
                    "sounds/99.wav", "sounds/100.wav", "music/x.mp3", "music/a b.ogg",
                    "music/" + "x" * 33 + ".ogg", "walls/sub/1.png"):
            with self.assertRaises(ResourceError, msg=bad):
                parse_mod_path(bad)

    def test_targets_are_checked(self):
        for kind, target in ((WALL, 136), (SPRITE, -1), (SOUND, 99), (MUSIC, "x.wav"),
                             ("picture", 1), (WALL, True)):
            with self.assertRaises(ResourceError, msg=(kind, target)):
                self.store.put(b"x", kind, target)

    def test_store_round_trip_and_digest(self):
        resource = self.store.put(b"hello", WALL, 3, "brick.png")
        self.assertEqual(self.store.read(resource), b"hello")
        self.assertEqual(resource.size, 5)
        self.assertEqual(self.store.put(b"hello", WALL, 3).sha256, resource.sha256)

    def test_a_changed_file_is_named(self):
        resource = self.store.put(b"hello", WALL, 3, "brick.png")
        self.store.path_of(resource).write_bytes(b"tampered")
        with self.assertRaises(ResourceError) as raised:
            self.store.read(resource)
        self.assertEqual(raised.exception.diagnostic.code, "OBC-RES-003")
        self.assertIn("brick.png", str(raised.exception))
        self.store.path_of(resource).unlink()
        problems = self.store.check([resource])
        self.assertEqual([p.code for p in problems], ["OBC-RES-002"])

    def test_adopt_and_prune(self):
        keep = self.store.put(b"keep", WALL, 1)
        drop = self.store.put(b"drop", WALL, 2)
        other = ResourceStore(self.root / "saved.assets")
        self.assertEqual(other.adopt([keep], self.store), 1)
        self.assertEqual(other.read(keep), b"keep")
        self.assertEqual(self.store.prune([keep]), 1)
        self.assertFalse(self.store.path_of(drop).exists())
        (self.store.root / "notes.txt").write_text("mine")
        self.assertEqual(self.store.prune([]), 1)
        self.assertTrue((self.store.root / "notes.txt").exists(), "only blobs are pruned")

    def test_store_locations(self):
        saved = store_for(self.root / "pack.obcproject", workspace=self.root / "ws",
                          project_uuid="u")
        self.assertEqual(saved.root.name, "pack.assets")
        unsaved = store_for(None, workspace=self.root / "ws", project_uuid="u")
        self.assertEqual(unsaved.root, (self.root / "ws" / "unsaved" / "u").resolve())


# ---------------------------------------------------------------------------
# Art
# ---------------------------------------------------------------------------


class Png(Fixture):
    def decode(self, blob):
        return artimport.read_png(blob)

    def test_rgba(self):
        image = self.decode(fx.png(3, 2, lambda x, y: (x * 10, y * 20, 7, 255 - x)))
        self.assertEqual((image.width, image.height), (3, 2))
        self.assertEqual(image.rgba[4:8], bytes((10, 0, 7, 254)))

    def test_every_colour_type_and_depth(self):
        cases = [
            (dict(mode=0, depth=1), lambda x, y: x & 1, (255, 255, 255, 255)),
            (dict(mode=0, depth=2), lambda x, y: 2, (170, 170, 170, 255)),
            (dict(mode=0, depth=4), lambda x, y: 15, (255, 255, 255, 255)),
            (dict(mode=0, depth=16), lambda x, y: 0x8000, (128, 128, 128, 255)),
            (dict(mode=2, depth=16), lambda x, y: (0xFFFF, 0, 0x0100), (255, 0, 1, 255)),
            (dict(mode=4, depth=8), lambda x, y: (9, 100), (9, 9, 9, 100)),
            (dict(mode=4, depth=16), lambda x, y: (0x0900, 0xFFFF), (9, 9, 9, 255)),
            (dict(mode=6, depth=16), lambda x, y: (0x0100, 0x0200, 0x0300, 0x0400),
             (1, 2, 3, 4)),
        ]
        for options, pixel, expected in cases:
            image = self.decode(fx.png(5, 3, pixel, **options))
            self.assertEqual(tuple(image.rgba[4:8]), expected, options)

    def test_palette_with_transparency(self):
        palette = bytes((10, 20, 30, 40, 50, 60))
        blob = fx.png(4, 4, lambda x, y: (x + y) & 1, mode=3, depth=1, palette=palette,
                      trns=b"\x00")
        image = self.decode(blob)
        self.assertEqual(tuple(image.rgba[0:4]), (10, 20, 30, 0))
        self.assertEqual(tuple(image.rgba[4:8]), (40, 50, 60, 255))

    def test_colour_key_transparency(self):
        blob = fx.png(2, 1, lambda x, y: (x * 255, 0, 0), mode=2,
                      trns=struct.pack(">HHH", 255, 0, 0))
        image = self.decode(blob)
        self.assertEqual(image.rgba[3], 255)
        self.assertEqual(image.rgba[7], 0)

    def test_interlaced_matches_plain(self):
        def pixel(x, y):
            return ((x * 37) & 255, (y * 91) & 255, (x ^ y) & 255, 255)
        plain = self.decode(fx.png(13, 11, pixel))
        laced = self.decode(fx.png(13, 11, pixel, interlace=True, filter_type=1))
        self.assertEqual(plain, laced)

    def test_hostile_files_are_refused_with_reasons(self):
        good = fx.art()
        cases = {
            "not a png": b"GIF89a" + b"\0" * 40,
            "truncated": good[:60],
            "bad crc": good[:20] + bytes((good[20] ^ 1,)) + good[21:],
            "huge": fx.png(1, 1, lambda x, y: 0, mode=0)[:16]
            + struct.pack(">IIBBBBB", 100000, 100000, 8, 0, 0, 0, 0),
            "bad depth": fx.png(1, 1, lambda x, y: 0, mode=0).replace(
                b"\x00\x00\x00\x01\x00\x00\x00\x01\x08\x00", b"\x00\x00\x00\x01\x00\x00\x00\x01\x03\x00"),
            "oversized file": good + b"\0" * (256 << 10),
            "no image data": fx.png(64, 64, lambda x, y: (0, 0, 0, 255)).split(b"IDAT")[0][:-4]
            + fx._chunk(b"IEND", b""),
        }
        for label, blob in cases.items():
            with self.assertRaises(artimport.ArtError, msg=label):
                artimport.read_png(blob)

    def test_a_decompression_bomb_stops_at_the_declared_size(self):
        # Inflating stops at the 64 rows the header declares; the rest of the
        # 4 MB stream is never expanded.
        image = artimport.read_png(zlib_bomb())
        self.assertEqual((image.width, image.height), (64, 64))

    def test_art_must_be_64(self):
        artimport.check_art(fx.art())
        with self.assertRaises(artimport.ArtError) as raised:
            artimport.check_art(fx.art(edge=32), "small.png")
        self.assertIn("32x32", str(raised.exception))


def zlib_bomb() -> bytes:
    """A 64x64 grey PNG whose IDAT inflates to far more than 64 rows."""
    import zlib
    body = struct.pack(">IIBBBBB", 64, 64, 8, 0, 0, 0, 0)
    data = zlib.compress(b"\0" * (64 * 65 * 1000))
    return (b"\x89PNG\r\n\x1a\n" + fx._chunk(b"IHDR", body) + fx._chunk(b"IDAT", data)
            + fx._chunk(b"IEND", b""))


class PaletteRule(unittest.TestCase):
    """Section 2.4, against the vectors OBCR's quantizer also passes."""

    def test_shared_vectors(self):
        vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
        quantizer = artimport.Quantizer(vectors["palette"])
        for case in vectors["cases"]:
            r, g, b, a = case["rgba"]
            self.assertEqual(quantizer.index(r, g, b, a, sprite=False), case["wall"], case)
            self.assertEqual(quantizer.index(r, g, b, a, sprite=True), case["sprite"], case)

    def test_entry_255_is_never_chosen(self):
        palette = [(0, 0, 0)] * 255 + [(255, 255, 255)]
        self.assertEqual(artimport.quantize(bytes((255, 255, 255, 255)), palette, sprite=False),
                         b"\0")

    def test_preview_buffers_match_the_thumbnails(self):
        palette = [(i, i, i) for i in range(256)]
        image = artimport.check_art(fx.art((90, 90, 90), hole=True))
        wall = artimport.preview(image, palette, sprite=False)
        sprite = artimport.preview(image, palette, sprite=True)
        self.assertEqual(len(wall), 64 * 64 * 3)
        self.assertEqual(len(sprite), 64 * 64 * 4)
        hole = (20 * 64 + 20) * 4
        self.assertEqual(sprite[hole + 3], 0, "transparent in a sprite")
        self.assertEqual(wall[(20 * 64 + 20) * 3], 0, "walls ignore alpha: black stays opaque")
        self.assertEqual(sprite[3], 255)


# ---------------------------------------------------------------------------
# Sound and music
# ---------------------------------------------------------------------------


class Audio(unittest.TestCase):
    def test_wav(self):
        info = audioimport.check_wav(fx.wav(rate=22050, channels=2))
        self.assertEqual((info.channels, info.rate, info.bits), (2, 22050, 16))
        self.assertEqual(audioimport.check_wav(fx.float_wav()).bits, 32)
        audioimport.check_wav(fx.wav(width=1))

    def test_wav_refusals(self):
        cases = {
            "not riff": b"RIFX" + fx.wav()[4:],
            "too long": fx.wav(seconds=31, rate=4000, width=1),
            "rate": fx.wav(rate=96000),
            "slow": fx.wav(rate=3000),
            "24-bit": fx.wav(width=3),
            "channels": fx.wav(channels=3),
            "no data": fx.wav()[:36],
        }
        for label, blob in cases.items():
            with self.assertRaises(audioimport.AudioError, msg=label):
                audioimport.check_wav(blob)

    def test_music(self):
        self.assertEqual(audioimport.check_music(fx.ogg(), "ogg").rate, 44100)
        self.assertEqual(audioimport.check_music(fx.qoa(1, 22050), "qoa").channels, 1)
        self.assertAlmostEqual(audioimport.check_music(fx.imf(7), "imf").seconds, 0.7)
        # A headerless ("type 0") song too, by OBCR's rule.
        headerless = fx.imf(7)[2:]
        self.assertAlmostEqual(audioimport.check_music(headerless, "imf").seconds, 0.7)

    def test_music_refusals(self):
        cases = [
            (b"OggS" + b"\0" * 80, "ogg"),
            (fx.ogg(codec=b"OpusHea"), "ogg"),
            (fx.ogg(channels=6), "ogg"),
            (fx.ogg(rate=96000), "ogg"),
            (fx.qoa(channels=3), "qoa"),
            (fx.qoa(samples=0), "qoa"),
            (b"qoa!" + b"\0" * 20, "qoa"),
            (struct.pack("<H", 4000) + b"\0" * 8, "imf"),
            (struct.pack("<H", 6) + b"\0" * 8, "imf"),
            (fx.ogg(), "mp3"),
            (fx.ogg() + b"\0" * (16 << 20), "ogg"),
        ]
        for blob, extension in cases:
            with self.assertRaises(audioimport.AudioError, msg=(blob[:8], extension)):
                audioimport.check_music(blob, extension)


class Importing(Fixture):
    def test_art_and_sound(self):
        wall = import_art(self.store, WALL, 12, self.file("brick.png", fx.art()))
        self.assertEqual((wall.kind, wall.target, wall.source_name), (WALL, 12, "brick.png"))
        self.assertEqual(self.store.read(wall), fx.art(), "stored exactly as given")
        sound = import_sound(self.store, 7, self.file("bang.wav", fx.wav()))
        self.assertEqual(sound.mod_path, "sounds/07.wav")
        with self.assertRaises(ObcEditError):
            import_art(self.store, SPRITE, 5, self.file("big.png", fx.art(edge=128)))
        with self.assertRaises(ObcEditError):
            import_sound(self.store, 7, self.file("fake.wav", b"nope"))

    def test_music_names(self):
        self.assertEqual(music_name_for("My Song (final).ogg"), "My-Song-final.ogg")
        self.assertEqual(music_name_for("x.ogg", ["x.ogg", "X-2.ogg"]), "x-3.ogg")
        self.assertEqual(len(music_name_for("y" * 50 + ".qoa")), 36)
        resource = import_music(self.store, self.file("Theme Song.ogg", fx.ogg()))
        self.assertEqual(resource.target, "Theme-Song.ogg")
        imf = import_music(self.store, self.file("a.imf", fx.imf()), name="boss.imf")
        self.assertEqual(imf.target, "boss.imf")

    def test_wav_music_goes_through_obcr(self):
        engine = fx.fake_obcr(self.root / "engine", fx.CONVERTS)
        resource = import_music(self.store, self.file("loop.wav", fx.wav(seconds=2)),
                                engine=engine)
        self.assertEqual(resource.target, "loop.qoa")
        self.assertTrue(self.store.read(resource).startswith(b"qoaf"))

    def test_wav_music_without_a_new_enough_obcr(self):
        song = self.file("loop.wav", fx.wav())
        with self.assertRaises(ImportError_) as raised:
            import_music(self.store, song, engine=None)
        self.assertEqual(str(raised.exception.diagnostic.message), CONVERT_UNAVAILABLE)
        old = fx.fake_obcr(self.root / "old", fx.TOO_OLD)
        with self.assertRaises(ImportError_) as raised:
            import_music(self.store, song, engine=old)
        self.assertIn("--convert-audio", raised.exception.diagnostic.message)


# ---------------------------------------------------------------------------
# Schema 2 and commands
# ---------------------------------------------------------------------------


class Schema(Fixture):
    def test_round_trip(self):
        wall = self.store.put(b"w", WALL, 3, "w.png")
        song = self.store.put(b"m", MUSIC, "theme.ogg", "theme.ogg")
        project = self.project(room(GOOD), resources=[song, wall])
        loaded = deserialize(serialize(project))
        self.assertEqual(loaded.resources, (wall, song), "sorted by kind")
        self.assertEqual(serialize(loaded), serialize(project))

    def test_migration_from_schema_1(self):
        payload = json.loads(serialize(self.project(room(GOOD))))
        payload["schema_version"] = 1
        del payload["resources"]
        loaded = deserialize(json.dumps(payload))
        self.assertEqual(loaded.resources, ())
        self.assertEqual(loaded.schema_version, 2)

    def test_hostile_records_are_refused(self):
        base = json.loads(serialize(self.project(room(GOOD))))
        good = {"kind": "wall", "target": 3, "sha256": "a" * 64, "source_name": "", "size": 1}
        for bad in ({**good, "kind": "picture"}, {**good, "target": 999},
                    {**good, "sha256": "../../x"}, {**good, "path": "C:/x"},
                    {**good, "kind": "music", "target": "../evil.ogg"},
                    {**good, "size": -1}):
            payload = dict(base, resources=[bad])
            with self.assertRaises(ObcEditError, msg=bad):
                deserialize(json.dumps(payload))
        with self.assertRaises(ObcEditError):
            deserialize(json.dumps(dict(base, resources=[good, good])))

    def test_import_replace_revert_undo(self):
        project = self.project(room(GOOD))
        history = History()
        first = self.store.put(b"one", WALL, 4)
        second = self.store.put(b"two", WALL, 4)
        project = history.do(project, set_resource(project, WALL, 4, first))
        self.assertEqual(history.undo_label, "Import wall 4")
        project = history.do(project, set_resource(project, WALL, 4, second))
        self.assertEqual(history.undo_label, "Replace wall 4")
        project = history.do(project, set_resource(project, WALL, 4, None))
        self.assertIsNone(project.resource(WALL, 4))
        project = history.undo(project)
        self.assertEqual(project.resource(WALL, 4), second)
        project = history.undo(project)
        self.assertEqual(project.resource(WALL, 4), first)
        self.assertEqual(self.store.read(project.resource(WALL, 4)), b"one",
                         "the replaced file is still there to undo to")
        project = history.undo(project)
        self.assertEqual(project.resources, ())
        self.assertTrue(project.dirty)

    def test_rename_music(self):
        project = self.project(room(GOOD))
        history = History()
        song = self.store.put(b"m", MUSIC, "a.ogg")
        project = history.do(project, set_resource(project, MUSIC, "a.ogg", song))
        project = history.do(project, rename_music(project, "a.ogg", "b.ogg"))
        self.assertIsNone(project.resource(MUSIC, "a.ogg"))
        self.assertEqual(project.resource(MUSIC, "B.OGG").sha256, song.sha256)
        project = history.undo(project)
        self.assertIsNotNone(project.resource(MUSIC, "a.ogg"))

    def test_a_record_cannot_replace_something_else(self):
        project = self.project(room(GOOD))
        with self.assertRaises(ObcEditError):
            set_resource(project, WALL, 4, self.store.put(b"x", WALL, 5))


# ---------------------------------------------------------------------------
# Campaign: arenas and music files
# ---------------------------------------------------------------------------


def arena_project(fixture: Fixture, *, with_music=True):
    floor = room(GOOD, name="FLOOR")
    arena = room(["#######", "#S....#", "#.....#", "#######"], name="ARENA")
    resources = []
    if with_music:
        resources.append(fixture.store.put(fx.ogg(), MUSIC, "theme.ogg", "theme.ogg"))
    resources.append(fixture.store.put(fx.art((10, 200, 10)), WALL, 0, "green.png"))
    resources.append(fixture.store.put(fx.wav(), SOUND, 3, "bang.wav"))
    project = fixture.project(floor, arena, resources=resources)
    campaign = Campaign(
        mod_id="arena-pack", title="Arena Pack", version="1.0",
        levels=(LevelEntry(map_uuid=floor.uuid, music="theme.ogg"),),
        arenas=(ArenaEntry(map_uuid=arena.uuid, name="Pit", music=3, colors=52, dark=True),),
    )
    return project, campaign, floor, arena


class Arenas(Fixture):
    def test_manifest(self):
        project, campaign, _, _ = arena_project(self)
        self.assertEqual(validate(campaign, project.maps, music_names={"theme.ogg"}), [])
        text = render_manifest(campaign)
        data = tomllib.loads(text)
        self.assertEqual(data["format"], 2)
        self.assertEqual(data["level"][0]["music"], "theme.ogg")
        self.assertEqual(data["arena"], [{"map": 1, "name": "Pit", "music": 3, "colors": 52,
                                          "dark": True}])
        self.assertEqual(parse_manifest(text)[1], [])

    def test_format_1_when_nothing_needs_2(self):
        _, campaign, _, _ = arena_project(self)
        plain = replace(campaign, arenas=(), levels=(replace(campaign.levels[0], music=2),))
        self.assertIn("format = 1", render_manifest(plain))

    def test_arena_limits(self):
        project, campaign, floor, arena = arena_project(self)
        bad = replace(campaign, arenas=tuple(
            ArenaEntry(map_uuid=arena.uuid, colors=60, music="missing.ogg") for _ in range(11)))
        codes = {p.code for p in validate(bad, project.maps, music_names={"theme.ogg"})}
        self.assertTrue({"OBC-MOD-017", "OBC-MOD-007", "OBC-MOD-015"} <= codes, codes)
        self.assertIn("OBC-MOD-017", {p.code for p in validate(
            replace(campaign, arenas=(ArenaEntry(),)), project.maps)})

    def test_format_1_cannot_name_music_files(self):
        _, problems = parse_manifest('format = 1\n[mod]\nid="a"\ntitle="A"\nversion="1"\n'
                                     '[[level]]\nstock = 1\nmusic = "x.ogg"\n'
                                     '[[arena]]\nmap = 0\n')
        messages = " ".join(p.message for p in problems)
        self.assertIn("format = 2", messages)
        self.assertIn("'arena'", messages)

    def test_json_round_trip(self):
        _, campaign, _, _ = arena_project(self)
        self.assertEqual(Campaign.from_json(campaign.to_json()), campaign)


# ---------------------------------------------------------------------------
# Export and audit
# ---------------------------------------------------------------------------


class Export(Fixture):
    def test_build_carries_the_assets(self):
        project, campaign, _, _ = arena_project(self)
        unused = self.store.put(fx.ogg(), MUSIC, "unused.ogg")
        project = project.with_resources(project.resources + (unused,))
        build = build_mod(project, campaign, store=self.store)
        self.assertEqual(build.format, 2)
        self.assertEqual(sorted(n for n in build.files if "/" in n),
                         ["music/theme.ogg", "sounds/03.wav", "walls/000.png"])

    def test_assets_alone_make_format_2(self):
        project, campaign, _, _ = arena_project(self, with_music=False)
        plain = replace(campaign, arenas=(), levels=(replace(campaign.levels[0], music=None),))
        build = build_mod(project, plain, store=self.store)
        self.assertEqual(build.format, 2)
        self.assertIn(b"format = 2", build.files["mod.toml"])

    def test_a_missing_or_changed_file_is_named(self):
        project, campaign, _, _ = arena_project(self)
        wall = project.resource(WALL, 0)
        self.store.path_of(wall).write_bytes(b"changed")
        with self.assertRaises(ExportError) as raised:
            build_mod(project, campaign, store=self.store)
        self.assertIn("green.png", str(raised.exception))
        with self.assertRaises(ExportError):
            build_mod(project, campaign, store=None)

    def test_music_must_exist(self):
        project, campaign, _, _ = arena_project(self, with_music=False)
        with self.assertRaises(ExportError) as raised:
            build_mod(project, campaign, store=self.store)
        self.assertEqual(raised.exception.diagnostic.code, "OBC-MOD-015")

    def test_preview_carries_assets(self):
        project, campaign, floor, arena = arena_project(self)
        project = project.with_campaign(campaign.to_json())
        build = preview_mod(project, arena.uuid, store=self.store)
        self.assertIn("walls/000.png", build.files)
        self.assertEqual(build.campaign.levels[0].music, 3, "the arena's music")
        build = preview_mod(project, floor.uuid, store=self.store)
        self.assertIn("music/theme.ogg", build.files)

    def test_written_and_audited(self):
        project, campaign, _, _ = arena_project(self)
        build = build_mod(project, campaign, store=self.store)
        for target in (self.root / "out" / "pack", self.root / "out" / "pack.obcmod"):
            written = write_mod(build, target)
            audit = audit_mod(written, catalog=CATALOG)
            self.assertTrue(audit.ok, [str(p) for p in audit.problems])
            self.assertEqual(set(audit.assets.values()),
                             {(WALL, 0), (SOUND, 3), (MUSIC, "theme.ogg")})
            self.assertEqual(len(audit.campaign.arenas), 1)

    def test_a_folder_export_removes_stale_replacements_only(self):
        project, campaign, _, _ = arena_project(self)
        folder = write_mod(build_mod(project, campaign, store=self.store), self.root / "pack")
        (folder / "walls" / "notes.txt").write_text("keep me")
        without = project.with_resources(r for r in project.resources if r.kind != WALL)
        write_mod(build_mod(without, campaign, store=self.store), folder)
        self.assertFalse((folder / "walls" / "000.png").exists())
        self.assertTrue((folder / "walls" / "notes.txt").exists())


class Audit(Fixture):
    def mod(self, files: dict[str, bytes], *, zip_it=False) -> Path:
        target = self.root / ("mod.obcmod" if zip_it else "mod")
        if zip_it:
            with zipfile.ZipFile(target, "w") as archive:
                for name, data in files.items():
                    archive.writestr(name, data)
            return target
        for name, data in files.items():
            path = target / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        return target

    MANIFEST = ('format = 2\n[mod]\nid = "m"\ntitle = "M"\nversion = "1"\n'
                '[[level]]\nstock = 3\nmusic = "song.ogg"\n')

    def codes(self, audit):
        return {p.code for p in audit.problems}

    def test_bad_assets_are_found(self):
        for zip_it in (False, True):
            audit = audit_mod(self.mod({
                "mod.toml": self.MANIFEST.encode(),
                "walls/13.png": fx.art(edge=32),
                "walls/12.png": fx.art(),
                "walls/012.png": fx.art(),
                "walls/0012.png": fx.art(),
                "sprites/700.png": fx.art(),
                "sounds/05.wav": b"not a wav",
                "music/song.ogg": fx.ogg(),
                "music/extra.qoa": fx.qoa(),
                "notes/readme.md": b"ignored",
            }, zip_it=zip_it), catalog=CATALOG)
            codes = self.codes(audit)
            self.assertIn("OBC-IMG-002", codes)          # 32x32
            self.assertIn("OBC-MOD-016", codes)          # duplicate 12, bad numbers
            self.assertIn("OBC-AUDIO-001", codes)        # not a wav
            self.assertIn("OBC-MOD-018", codes)          # extra.qoa unused
            self.assertIn("OBC-MOD-013", codes)          # notes/ ignored
            self.assertFalse(audit.ok)

    def test_missing_music_and_format_1_folders(self):
        audit = audit_mod(self.mod({"mod.toml": self.MANIFEST.encode()}), catalog=CATALOG)
        self.assertIn("OBC-MOD-015", self.codes(audit))
        old = self.MANIFEST.replace("format = 2", "format = 1").replace('music = "song.ogg"\n',
                                                                        "")
        self._dir.cleanup()
        self._dir = tempfile.TemporaryDirectory()
        self.root = Path(self._dir.name)
        audit = audit_mod(self.mod({"mod.toml": old.encode(), "walls/1.png": fx.art()}),
                          catalog=CATALOG)
        self.assertIn("OBC-MOD-016", self.codes(audit))
        self.assertTrue(audit.ok, "format 1 with folders is a warning")

    def test_arenas_are_validated_as_arenas(self):
        project, campaign, floor, arena = arena_project(self)
        enemy = room(["#######", "#S..e.#", "#######"], name="BAD")
        project = project.added(enemy)
        campaign = replace(campaign, arenas=(ArenaEntry(map_uuid=enemy.uuid),))
        build = build_mod(project, campaign, store=self.store)
        audit = audit_mod(write_mod(build, self.root / "bad"), catalog=CATALOG)
        self.assertIn("OBC-ARENA-001", self.codes(audit))


class ArenaProfile(unittest.TestCase):
    def test_enemies_are_errors_and_reach_is_a_warning(self):
        document = room(["##########", "#S..e.####", "######...#", "##########"])
        problems = validate_map(document, CATALOG, profile=ARENA)
        by_code = {p.code: p for p in problems}
        self.assertEqual(by_code["OBC-ARENA-001"].severity, Severity.ERROR)
        self.assertEqual(by_code["OBC-UNREACH-001"].severity, Severity.WARNING)
        self.assertNotIn("OBC-EXIT-001", by_code, "arenas need no exit")
        single = {p.code: p for p in validate_map(document, CATALOG, profile=SINGLE_PLAYER)}
        self.assertNotIn("OBC-ARENA-001", single)
        self.assertEqual(single["OBC-UNREACH-001"].severity, Severity.INFORMATION)

    def test_one_start(self):
        document = room(["#######", "#S..S.#", "#######"])
        self.assertIn("OBC-START-002", {p.code for p in validate_map(document, CATALOG,
                                                                      profile=ARENA)})

    def test_profile_by_membership(self):
        document = room(GOOD, slot=55)
        self.assertEqual(profile_for_map(document, {document.uuid}).key, "arena")
        self.assertEqual(profile_for_map(document).key, "network")
        self.assertEqual(profile_for_map(room(GOOD)).key, "single_player")


if __name__ == "__main__":
    unittest.main()
