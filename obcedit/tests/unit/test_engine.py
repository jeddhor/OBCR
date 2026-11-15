#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Launching OBCR, snapshots, and finding the engine and the data."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from obc_support import engine, make_fixtures, real_data, room

from obcedit_core.discovery import (
    FAIL,
    OK,
    Profile,
    check_data_directory,
    check_engine,
    check_profile,
    parse_check_data,
)
from obcedit_core.engine_runner import (
    LaunchError,
    Session,
    SessionState,
    build_launch_plan,
    parse_check_mod,
    probe_capabilities,
    protocol_supported,
    start_argument,
)
from obcedit_core.errors import ObcEditError
from obcedit_core.snapshot import (
    SNAPSHOT_TIC,
    Camera,
    check_camera,
    looks_like_a_world,
    snapshot_arguments,
    snapshot_key,
)


class Fixture(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.exe = self.root / "obcr.exe"
        self.exe.write_bytes(b"not really")
        self.exe.chmod(0o755)
        self.data = self.root / "data"
        self.data.mkdir()
        self.mod = self.root / "mod"
        self.mod.mkdir()

    def tearDown(self):
        self._tmp.cleanup()


class Launch(Fixture):
    def test_the_argument_vector(self):
        plan = build_launch_plan(executable=self.exe, data_dir=self.data, mod=self.mod, floor=2,
                                 difficulty=0, start=(10, 11, "south"),
                                 session_dir=self.root / "session")
        self.assertEqual(plan.arguments[:8], ["--data", str(self.data.resolve()), "--mod",
                                              str(self.mod.resolve()), "--play", "2",
                                              "--difficulty", "0"])
        self.assertIn("10,11,2", plan.arguments)
        self.assertIn(str((self.root / "session").resolve() / "obcr.toml"), plan.arguments)
        self.assertTrue((self.root / "session").is_dir())
        self.assertEqual(plan.argv[0], str(self.exe.resolve()))

    def test_refusals(self):
        with self.assertRaises(LaunchError):
            build_launch_plan(executable=self.root / "none", data_dir=self.data, mod=self.mod)
        with self.assertRaises(LaunchError):
            build_launch_plan(executable=self.exe, data_dir=self.data, mod=self.mod, difficulty=5)
        with self.assertRaises(LaunchError):
            start_argument(70, 1, 0)
        with self.assertRaises(LaunchError):
            start_argument(1, 1, "up")

    def test_session_outcomes(self):
        plan = build_launch_plan(executable=self.exe, data_dir=self.data, mod=self.mod)
        clean = Session(plan)
        clean.started()
        self.assertIs(clean.state, SessionState.PLAYING)
        clean.finished(0)
        self.assertIs(clean.state, SessionState.FINISHED)
        broken = Session(plan)
        broken.started()
        broken.feed("OBCR error: loading the mod: map 0 has no player start\n")
        broken.finished(1)
        self.assertIs(broken.state, SessionState.FAILED)
        self.assertIn("player start", broken.describe())
        stopped = Session(plan)
        stopped.started()
        stopped.stopping()
        stopped.finished(1)
        self.assertIs(stopped.state, SessionState.FINISHED)

    def test_check_mod_output(self):
        text = ("mod\tlobby\tLobby\t1.0\t3\n"
                "problem\terror\t0\t12,30\tthe map has 2 player starts\n"
                "problem\twarning\t-\t-\tsomething\n"
                "result\terrors\n")
        check = parse_check_mod(text, 2)
        self.assertFalse(check.ok)
        self.assertEqual((check.mod_id, check.levels), ("lobby", 3))
        self.assertEqual(check.errors[0].cell, (12, 30))
        self.assertEqual(check.problems[1].level, None)
        self.assertTrue(parse_check_mod("mod\tx\tX\t1\t1\nresult\tok\n", 0).ok)


class Snapshots(unittest.TestCase):
    def test_camera_snaps_to_a_compass_direction(self):
        self.assertEqual(Camera(10.6, 20.2, 0).arguments(), ["--start", "10,20,1"])
        self.assertEqual(Camera(1, 1, 90).direction, 0)
        self.assertEqual(Camera(1, 1, 181).direction, 3)
        self.assertEqual(Camera(1, 1, 265).describe(), "(1, 1) facing south")

    def test_arguments_and_key(self):
        args = snapshot_arguments(Camera(5, 5, 0), "x.png")
        self.assertEqual(args, ["--start", "5,5,1", "--shot", str(SNAPSHOT_TIC), "--shot-out",
                                "x.png", "--classic"])
        self.assertGreaterEqual(SNAPSHOT_TIC, 30)
        key = dict(engine="none", data_fingerprint="d", export_digest="e")
        self.assertNotEqual(snapshot_key(camera=Camera(5, 5, 0), **key),
                            snapshot_key(camera=Camera(5, 6, 0), **key))

    def test_the_camera_must_stand_on_floor(self):
        document = room(["#S..#"])
        check_camera(document, Camera(2, 0, 0))
        with self.assertRaises(ObcEditError):
            check_camera(document, Camera(0, 0, 0))
        with self.assertRaises(ObcEditError):
            check_camera(document, Camera(80, 0, 0))

    def test_a_blank_frame_is_not_a_world(self):
        from obcedit_core.assets import encode_png

        with tempfile.TemporaryDirectory() as tmp:
            flat = Path(tmp) / "flat.png"
            flat.write_bytes(encode_png(32, 20, bytes(32 * 20 * 3), alpha=False))
            self.assertFalse(looks_like_a_world(flat))
            busy = Path(tmp) / "busy.png"
            busy.write_bytes(encode_png(32, 20, bytes((i * 37) & 0xFF for i in range(32 * 20 * 3)),
                                        alpha=False))
            self.assertTrue(looks_like_a_world(busy))


class Discovery(Fixture):
    def test_a_folder_with_the_game_files(self):
        fixtures = make_fixtures.fixture_set()
        for name in ("MAPHEAD.BC", "MAPTEMP.BC", "GFXTILES.BC", "COUNT.EXE"):
            (self.data / name).write_bytes(fixtures[f"synthetic.{name}"])
        report = check_data_directory(self.data)
        statuses = {check.name: check.status for check in report}
        self.assertEqual(statuses["Required game files"], OK)
        self.assertEqual(statuses["Palette"], OK)
        self.assertEqual(statuses["Map archive"], OK)
        self.assertTrue(report.usable)

    def test_missing_files_fail(self):
        report = check_data_directory(self.data)
        self.assertFalse(report.usable)
        self.assertEqual(check_data_directory(self.root / "nowhere").failures[0].status, FAIL)

    def test_the_engine_is_not_run_until_asked(self):
        report = check_engine(self.exe)
        self.assertEqual([c.status for c in report][-1], "warning")

    def test_check_data_output(self):
        identity = parse_check_data("release\tOBC-CD\tOperation Body Count, CD-ROM release\n"
                                    "source\tfolder\nfile\trequired\tok\tBCCD.EXE\tx\t1\n"
                                    "complete\tyes\n")
        self.assertEqual((identity.release, identity.complete, identity.source),
                         ("OBC-CD", True, "folder"))

    def test_profile_round_trip(self):
        profile = Profile("p", str(self.exe), str(self.data), str(self.root), "f", "v", "OBC-A")
        self.assertEqual(Profile.from_json(profile.to_json()), profile)


class RealEngine(unittest.TestCase):
    """Engine-gated: the built OBCR speaks the contract, and a snapshot is a world."""

    def setUp(self):
        self.binary = engine()
        if self.binary is None:
            self.skipTest("no OBCR build in target/")

    def test_capabilities(self):
        capabilities = probe_capabilities(self.binary)
        self.assertTrue(protocol_supported(capabilities), capabilities)
        self.assertTrue(capabilities["version"].startswith("obcr "))

    def test_identity_and_release(self):
        data = real_data("full")
        if data is None:
            self.skipTest("no game data")
        report = check_profile(Profile(engine_path=str(self.binary), data_dir=str(data)),
                               probe=True)
        statuses = {check.name: check for check in report}
        self.assertEqual(statuses["Engine identity"].status, OK)
        self.assertEqual(statuses["Release"].status, OK, statuses["Release"])

    def test_a_snapshot_of_an_exported_room(self):
        data = real_data("full")
        if data is None:
            self.skipTest("no game data")
        from obcedit_core.document import ProjectDocument
        from obcedit_core.modexport import build_mod, write_mod

        document = room(["#######", "#S....E", "#.....#", "#######"])
        proj = ProjectDocument.create("Snap").added(document)
        with tempfile.TemporaryDirectory() as tmp:
            mod = write_mod(build_mod(proj), Path(tmp) / "mod")
            shot = Path(tmp) / "shot.png"
            plan = build_launch_plan(executable=self.binary, data_dir=data, mod=mod,
                                     session_dir=Path(tmp) / "session",
                                     extra=snapshot_arguments(Camera(2, 1, 0), shot))
            done = subprocess.run(plan.argv, cwd=plan.cwd, capture_output=True, timeout=120)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertTrue(looks_like_a_world(shot))


if __name__ == "__main__":
    unittest.main()
