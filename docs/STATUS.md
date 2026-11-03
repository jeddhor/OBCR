# OBCR status

Live project state. Every agent reads this at the start of a session and updates it at the end (AGENT_GUIDE §3). Keep edits small: change your own rows, append to the log.

**Current milestone: M0 — Bootstrap and tooling**
**Last updated: 2026-11-02**

## Milestones

| # | Milestone | Status | Notes |
|---|---|---|---|
| M0 | Bootstrap and tooling | Ready | Repository and planning docs created; no code yet |
| M1 | Data formats and asset pipeline | Not started | Initial format survey done (re/DATA_FORMATS.md) |
| M2 | Reverse-engineering foundation | Not started | |
| M3 | Platform shell | Not started | |
| M4 | Classic renderer | Not started | |
| M5 | World simulation core | Not started | |
| M6 | Audio | Not started | |
| M7 | Weapons, combat and HUD | Not started | |
| M8 | Enemies and AI | Not started | |
| M9 | Allies | Not started | |
| M10 | Game flow and front end | Not started | |
| M11 | Persistence | Not started | |
| M12 | Single-player parity certification | Not started | |
| M13 | Multiplayer | Not started | |
| M14 | Enhanced mode | Not started | |
| M15 | Release engineering | Not started | |

Status values: `Not started`, `Ready` (prerequisites met), `In progress`, `Blocked`, `Done`.

## Active tasks

Task status values: `In progress`, `Paused`, `Blocked`, `Review`, `Done`. Remove `Done` rows when the milestone closes (the log keeps the history).

| Task | Title | Status | Owner | Branch | Started | Evidence / notes |
|---|---|---|---|---|---|---|
| — | — | — | — | — | — | — |

## Blockers and owner decisions needed

| Id | Question | Needed by | Status |
|---|---|---|---|
| B-1 | License | M15 | Resolved 2026-11-02: GPL-3.0-or-later (ADR-006) |
| B-2 | Remote for CI? | M0.7 | Resolved 2026-11-02: local builds only; `cargo xtask ci` is the gate |
| B-3 | Rendered sequences and CD audio data | M10.R9, M6.R2 | Resolved 2026-11-02: the owner's floppy install archives contain `F2.BC`, `F3.BC`, `F4.BC`, `F6.BC` (FLC animations). No CD audio is available. |
| B-4 | Agent commit policy | M0 | Resolved 2026-11-02: confirmed as written in CLAUDE.md |

## Owner goal (2026-11-02)

Work milestone by milestone until the owner must test something, or until **the first episode is playable start to finish**. Must include modern graphics features (widescreen, upscaling) and gamepad support.

## Log

Append one line per completed task or notable event: `YYYY-MM-DD — <task> — <what changed> — <evidence>`.

- 2026-11-02 — setup — Repository created; DESIGN, AGENT_GUIDE, MILESTONES, DECISIONS, STATUS and the re/ knowledge base written from an initial survey of `..\OBC`, `..\wolf3d`, `..\ECWolf`, `..\dosbox-x` — data parsed with throwaway Python scripts (61 maps, 937 VSWAP chunks, 58 VGAGRAPH chunks, 310 audio chunks); `rndtable` byte-match confirmed.
