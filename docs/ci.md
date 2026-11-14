# Tests, continuous integration and releases

How OBCR is checked, what GitHub Actions runs and what it cannot, and how a release is made.

## The gates

Two commands, both run locally:

```sh
cargo xtask ci          # the gate for every change
cargo xtask test-data   # the tests that need the original game data
```

**`cargo xtask ci`** runs, in order, and stops at the first failure:

1. `cargo fmt --all --check`: formatting (`rustfmt.toml`).
2. `cargo clippy --workspace --all-targets -- -D warnings`: every lint is an error.
3. `cargo test --workspace`: all tests. Tests that need game data skip themselves with a `SKIPPED` note when it is missing.
4. `cargo deny check licenses bans`: every dependency's licence is compatible with GPL-3.0-or-later and no banned crate is used (`deny.toml`). It is skipped with a note if `cargo-deny` is not installed.

**`cargo xtask test-data`** runs the whole test suite again with the game data available. It looks for the data in `OBC_DATA_DIR`, then `.local/data/obc-full`, then `../OBC`. The CD release's tests also need `OBC_CD_DATA_DIR` (default `.local/data/obc-cd`, a folder with the CD's files) and `OBC_CD_IMAGE` (default: the first `.cue` in `../CD`); each skips itself if its data is missing.

A change is done when both pass (the full definition is [AGENT_GUIDE §11](AGENT_GUIDE.md)).

## What the tests cover

| Without game data | |
|---|---|
| Parsers | Every format, with property tests that feed them random and truncated input and require an error, never a panic |
| Simulation | Rules ported from the original (damage, pickups, doors, push walls, allies, the network rules), each against the behaviour the disassembly shows |
| Codecs and media | QOA round trips against the reference tables; the CUE/ISO reader on a synthetic image; the mixer streaming a CD track to its end |
| Controls | Default bindings, rebinding, the original mouse, lenient reading of hand-edited settings |
| Network | The UDP transport, including redundancy under loss and a four-player game with 10% packet loss |

| With game data | |
|---|---|
| Data | Every map, picture, sound and sequence decodes; both releases are identified by hash |
| Release layouts | `BCCD.EXE`'s tables, loaded through the CD layout, line up with `COUNT.EXE`'s record for record |
| The CD image | Identified straight off the disc; its audio tracks are where the cue sheet says and survive QOA |
| Soak | Every single-player floor at every game type for 2,500 frames of scripted play, on both releases, with overflow checks on: no panic and no simulation fault |
| Save and load | A game saved mid-floor, loaded and continued matches the one never saved, bit for bit |
| Game flow | The real screens through a floor change, saving and loading, death and the high-score entry |
| CD music | The CD version's track logic over a real mixer |

The network test that runs four players with packet loss is timing-sensitive, and it has failed once on a heavily loaded machine. If it fails alone, run it again before suspecting a change.

## GitHub Actions

The repository is public, so GitHub's hosted runners are free to use. There are two workflows in `.github/workflows/`:

**CI** (`ci.yml`) runs `cargo xtask ci` on Ubuntu 22.04 and Windows Server 2022 for every push to `main`, every pull request, and on demand. A newer push to the same branch cancels a run still in progress.

**Release** (`release.yml`) builds both installers with the same scripts used locally:

- **Windows**: Qt 6.10.3 with its MinGW 13.1 kit, installed into `C:\Qt` by `jurplel/install-qt-action`, then `installer/build-windows.ps1`. The local builds use Qt 6.11.1, but the Qt downloader the action uses (aqtinstall 3.3) cannot yet read Qt 6.11's repository. Setup only needs Qt 6, so 6.10 is fine.
- **Linux**: Ubuntu 22.04 (so the result runs on any newer distribution) with the distribution's Qt 6 packages, then `installer/build-linux.sh`.

Run by hand (*Actions → Release → Run workflow*), it only builds the installers and keeps them as the run's artifacts. On a `v*` tag it also publishes the release.

**What CI cannot run**: the data-gated tests. The game data is copyrighted and is never uploaded, so those tests skip themselves there. A self-hosted runner holding the data could run `cargo xtask test-data`; none is set up, so run it locally before every merge.

## Cutting a release

1. On a branch, update `version` in the workspace `Cargo.toml` and refresh `Cargo.lock` (any `cargo build`).
2. Add a `## <version> — <date> (pre-release)` section to `CHANGELOG.md`: the installers, the changes, the known limitations. The release workflow copies this section into the release notes word for word.
3. Update the version in `README.md`, run both gates, and merge to `main`.
4. Tag and push:

   ```sh
   git tag -a v1.0.0-beta.3 -m "OBCR 1.0.0-beta.3"
   git push origin main v1.0.0-beta.3
   ```

5. The Release workflow builds both installers. Once both succeed, it publishes the release with the two installers and `SHA256SUMS.txt`. A version with a hyphen (`-beta.3`) becomes a **pre-release**, and a `-beta.N` version is titled *OBCR x.y.z Beta N (pre-release)*.

If a build fails, nothing is published. Fix it on `main`, then move the tag to the fixed commit:

```sh
git tag -d v1.0.0-beta.3 && git push origin :refs/tags/v1.0.0-beta.3
git tag -a v1.0.0-beta.3 -m "OBCR 1.0.0-beta.3" && git push origin v1.0.0-beta.3
```

Only move a tag before its release has been published. After that, make a new version instead.

The installers are not code-signed, so Windows SmartScreen warns on first run. `SHA256SUMS.txt` lets users check their download (`sha256sum -c SHA256SUMS.txt`, or `Get-FileHash` on Windows).
