# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Launching OBCR on an exported mod -- as data, not a string.

A launch is an argument *vector*, never a shell string: no quoting to get
wrong, no path with a space to break it, nothing a project file could put in a
filename that would become a command. A project is untrusted input; the only
things from it that reach this are a floor number and a mod folder the editor
wrote itself.

The engine contract (OBCR's command line, docs/MODS_AND_OBCEDIT.md 4.5):

* `obcr --data DIR --mod PATH --play N [--difficulty D] [--start X,Y,DIR]`
  plays floor N (0-based) of the mod; DIR is a compass 0 N, 1 E, 2 S, 3 W;
* `obcr --check-mod PATH` validates a mod: `mod`, `problem` and `result`
  lines, tab-separated; exit 0 ok, 2 errors, 1 unreadable;
* `obcr --version` prints `obcr <version>`;
* `--config FILE` keeps a playtest's settings and saves away from the
  player's own.

The plan is returned rather than run, so the GUI can show what it is about to
do and a test can assert on the arguments without starting a game.
"""

from __future__ import annotations

import enum
import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from .errors import Diagnostic, ObcEditError, Severity


class LaunchError(ObcEditError):
    pass


def _error(message: str) -> LaunchError:
    return LaunchError(Diagnostic("OBC-ENGINE-001", Severity.ERROR, message))


#: The version of the OBCR command-line contract this editor speaks. There is
#: no handshake: `probe_capabilities` reads the engine's `--help` and checks
#: that every flag the contract needs is there.
PROTOCOL_VERSION = 1
REQUIRED_FLAGS = ("--mod", "--check-mod", "--play", "--start", "--data", "--shot")

#: Compass directions for `--start`.
COMPASS = ("north", "east", "south", "west")


@dataclass(frozen=True)
class LaunchPlan:
    """Everything needed to start a playtest, and nothing else."""

    executable: Path
    arguments: list[str]
    cwd: Path
    environment: dict = field(default_factory=dict)
    #: Where this session's settings and saves go. Never the player's own.
    session_dir: Path | None = None
    #: The mod folder this launch is testing.
    mod: Path | None = None
    floor: int = 0
    export_digest: str = ""
    revision: int = -1
    session: str = ""

    # EC7Edit's name for the file under test.
    @property
    def preview(self) -> Path | None:
        return self.mod

    @property
    def argv(self) -> list[str]:
        return [str(self.executable), *self.arguments]

    def described(self) -> str:
        return f"{self.cwd}$ {' '.join(self.argv)}"

    def summary(self) -> dict:
        return {
            "executable": str(self.executable),
            "arguments": list(self.arguments),
            "cwd": str(self.cwd),
            "session_dir": str(self.session_dir) if self.session_dir else "",
            "mod": str(self.mod) if self.mod else "",
            "floor": self.floor,
            "export_digest": self.export_digest,
            "revision": self.revision,
        }


def start_argument(x: int, y: int, direction: int | str) -> str:
    """`X,Y,DIR` for `--start`; `direction` is 0-3 or a compass name."""
    if isinstance(direction, str):
        if direction not in COMPASS:
            raise _error(f"{direction!r} is not one of {', '.join(COMPASS)}")
        direction = COMPASS.index(direction)
    if not 0 <= direction <= 3:
        raise _error(f"direction {direction} is outside 0..3")
    if not (0 <= x < 64 and 0 <= y < 64):
        raise _error(f"start ({x}, {y}) is outside the 64x64 map")
    return f"{x},{y},{direction}"


def build_launch_plan(
    *,
    executable: Path | str,
    data_dir: Path | str,
    mod: Path | str,
    floor: int = 0,
    difficulty: int = 1,
    start: tuple[int, int, int | str] | None = None,
    session_dir: Path | str | None = None,
    extra: list[str] | None = None,
    export_digest: str = "",
    revision: int = -1,
) -> LaunchPlan:
    """The playtest command for floor `floor` (0-based) of the mod at `mod`."""
    executable = Path(executable).expanduser()
    data_dir = Path(data_dir).expanduser()
    mod = Path(mod).expanduser()
    if not executable.is_file():
        raise _error(f"no engine at {executable}")
    if not data_dir.is_dir():
        raise _error(f"no game data directory at {data_dir}")
    if not mod.exists():
        raise _error(f"no mod at {mod}")
    if not 0 <= floor < 100:
        raise _error(f"floor {floor} is outside 0..99")
    if not 0 <= difficulty <= 3:
        raise _error(f"game type {difficulty} is outside 0..3 (difficulty 1-3, Random)")

    arguments = [
        "--data", str(data_dir.resolve()),
        "--mod", str(mod.resolve()),
        "--play", str(floor),
        "--difficulty", str(difficulty),
    ]
    if start is not None:
        arguments += ["--start", start_argument(*start)]
    resolved_session = Path(session_dir).expanduser().resolve() if session_dir else None
    if resolved_session is not None:
        resolved_session.mkdir(parents=True, exist_ok=True)
        # The engine writes its settings on exit and saves when asked; a
        # playtest must not touch the player's own.
        arguments += ["--config", str(resolved_session / "obcr.toml")]
    if extra:
        arguments.extend(str(item) for item in extra)
    return LaunchPlan(
        executable=executable.resolve(),
        arguments=arguments,
        cwd=resolved_session or data_dir.resolve(),
        session_dir=resolved_session,
        mod=mod.resolve(),
        floor=floor,
        export_digest=export_digest,
        revision=revision,
    )


# ---------------------------------------------------------------------------
# The session
# ---------------------------------------------------------------------------


class SessionState(enum.Enum):
    """Where a playtest is. OBCR reports nothing while it runs, so the states
    follow the process: started, stopping, and how it ended."""

    IDLE = "idle"
    STARTING = "starting"
    LOADING = "loading"
    PLAYING = "playing"
    STOPPING = "stopping"
    FINISHED = "finished"
    FAILED = "failed"


TERMINAL_STATES = (SessionState.IDLE, SessionState.FINISHED, SessionState.FAILED)
_ERROR_LINE = re.compile(r"^OBCR error: (?P<message>.+)$")


@dataclass
class Session:
    """One playtest, and everything learned about it. Driven by feeding it lines."""

    plan: LaunchPlan
    state: SessionState = SessionState.IDLE
    log: list = field(default_factory=list)
    failure: str = ""
    exit_code: int | None = None
    events: list = field(default_factory=list)

    LOG_LIMIT = 4000

    def started(self) -> None:
        # OBCR opens its window and plays; there is nothing in between to wait for.
        self.state = SessionState.PLAYING

    def feed(self, line: str):
        text = line.rstrip("\r\n")
        self.log.append(text)
        if len(self.log) > self.LOG_LIMIT:
            del self.log[: len(self.log) - self.LOG_LIMIT]
        found = _ERROR_LINE.match(text)
        if found:
            self.failure = found.group("message")
        return None

    def stopping(self) -> None:
        self.state = SessionState.STOPPING

    def finished(self, exit_code: int) -> None:
        self.exit_code = exit_code
        if self.state in TERMINAL_STATES:
            return
        if self.state is SessionState.STOPPING or (exit_code == 0 and not self.failure):
            self.state = SessionState.FINISHED
            return
        self.state = SessionState.FAILED
        if not self.failure:
            tail = next((line for line in reversed(self.log) if line.strip()), "")
            self.failure = (f"OBCR exited with code {exit_code}"
                            + (f": {tail}" if tail else ""))

    @property
    def reached_the_map(self) -> bool:
        return self.state is SessionState.FINISHED or self.state is SessionState.PLAYING

    @property
    def running(self) -> bool:
        return self.state not in TERMINAL_STATES

    def describe(self) -> str:
        floor = self.plan.floor + 1
        if self.state is SessionState.FINISHED:
            return f"Played floor {floor}, exit code {self.exit_code}"
        if self.state is SessionState.FAILED:
            return self.failure or "OBCR did not start the floor"
        return {
            SessionState.IDLE: "Not started",
            SessionState.STARTING: "Starting OBCR…",
            SessionState.LOADING: "Loading the floor…",
            SessionState.PLAYING: f"Playing floor {floor}",
            SessionState.STOPPING: "Stopping…",
        }[self.state]


# ---------------------------------------------------------------------------
# Asking the engine
# ---------------------------------------------------------------------------


def probe_capabilities(executable: Path | str, *, timeout: float = 10.0) -> dict:
    """Read `obcr --help` and `--version`. Empty when it is not an OBCR that
    speaks this editor's contract."""
    path = Path(executable).expanduser()
    try:
        done = subprocess.run([str(path), "--help"], capture_output=True, text=True,
                              timeout=timeout, cwd=str(path.parent), stdin=subprocess.DEVNULL)
        version = subprocess.run([str(path), "--version"], capture_output=True, text=True,
                                 timeout=timeout, cwd=str(path.parent),
                                 stdin=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError):
        return {}
    text = done.stdout + done.stderr
    flags = sorted(set(re.findall(r"--[a-z][a-z0-9-]+", text)))
    if "--play" not in flags:
        return {}
    found = {"flags": ",".join(flags), "version": version.stdout.strip()}
    if all(flag in flags for flag in REQUIRED_FLAGS):
        found["editor-protocol"] = str(PROTOCOL_VERSION)
    return found


def protocol_supported(capabilities: dict) -> bool:
    """Whether an engine has every flag this editor uses."""
    return capabilities.get("editor-protocol") == str(PROTOCOL_VERSION)


@dataclass(frozen=True)
class ModProblem:
    severity: str  # "error" | "warning"
    level: int | None
    cell: tuple[int, int] | None
    message: str


@dataclass
class ModCheck:
    """What `obcr --check-mod` said."""

    exit_code: int
    mod_id: str = ""
    title: str = ""
    version: str = ""
    levels: int = 0
    problems: list[ModProblem] = field(default_factory=list)
    result: str = ""
    output: str = ""

    @property
    def ok(self) -> bool:
        return self.exit_code == 0 and self.result == "ok"

    @property
    def unreadable(self) -> bool:
        return self.exit_code == 1

    @property
    def errors(self) -> list[ModProblem]:
        return [p for p in self.problems if p.severity == "error"]


def parse_check_mod(text: str, exit_code: int) -> ModCheck:
    check = ModCheck(exit_code, output=text)
    for line in text.splitlines():
        fields = line.rstrip("\r").split("\t")
        if fields[0] == "mod" and len(fields) >= 5:
            check.mod_id, check.title, check.version = fields[1], fields[2], fields[3]
            check.levels = int(fields[4]) if fields[4].isdigit() else 0
        elif fields[0] == "problem" and len(fields) >= 5:
            level = int(fields[2]) if fields[2].isdigit() else None
            cell = None
            if "," in fields[3]:
                x, _, y = fields[3].partition(",")
                if x.isdigit() and y.isdigit():
                    cell = (int(x), int(y))
            check.problems.append(ModProblem(fields[1], level, cell, "\t".join(fields[4:])))
        elif fields[0] == "result" and len(fields) >= 2:
            check.result = fields[1]
    return check


def check_mod(executable: Path | str, mod: Path | str, *, data_dir: Path | str | None = None,
              timeout: float = 60.0) -> ModCheck:
    """Run `obcr --check-mod` (the engine is the export gate)."""
    command = [str(Path(executable).expanduser())]
    if data_dir:
        command += ["--data", str(data_dir)]
    command += ["--check-mod", str(mod)]
    try:
        done = subprocess.run(command, capture_output=True, text=True, timeout=timeout,
                              stdin=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError) as error:
        return ModCheck(1, result="unreadable", output=str(error))
    return parse_check_mod(done.stdout + done.stderr, done.returncode)
