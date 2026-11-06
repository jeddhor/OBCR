# Third-party notices

OBCR is free software under the GNU General Public License, version 3 or later (see `LICENSE`). The programs and installers built from this repository include or link the components below. The original game's data is **not** included: OBCR needs the user's own copy of Operation Body Count, whose files remain the property of their owners.

## In the game (`obcr`)

- **SDL 3.4** (via the `sdl3` and `sdl3-sys` crates; built from source and linked statically) — zlib licence. <https://libsdl.org>
- **Nuked OPL3** (via the `nuked-opl3` crate; linked statically) — LGPL-2.1-or-later. OBCR's complete source is available under the GPL-3.0-or-later, so the program can be rebuilt with a modified library.
- **xBRZ 1.6** by Zenju — GPL-3.0. `crates/obc-render/src/xbrz.rs` is a port and carries that licence (ADR-020).
- The Rust crates listed below, linked statically.

## In OBCR Setup (the installers)

- **Qt 6** (Qt Core, Gui, Widgets and their platform plugins; Linux also Qt DBus, OpenGL, XCB QPA and Wayland client) — LGPL-3.0. Setup links Qt dynamically and ships the unmodified Qt libraries next to itself and its uninstaller copy; you may replace them with your own build. Windows packages use Qt 6.11 (MinGW kit), Linux packages Qt 6.2 from Ubuntu 22.04. Source: <https://download.qt.io/official_releases/qt/>.
- **Windows**: the MinGW-w64 runtime — `libstdc++-6.dll` and `libgcc_s_seh-1.dll` (GPL-3.0 with the GCC Runtime Library Exception), `libwinpthread-1.dll` (MIT and ZPL-2.1).
- **Linux**: ICU 70 (Unicode licence), double-conversion (BSD-3-Clause), libb2 (CC0-1.0), md4c (MIT), PCRE2 (BSD-3-Clause with the PCRE2 exception), libxkbcommon (MIT), the xcb-util libraries (MIT), libICE and libSM (X11/MIT), libgomp (GPL-3.0 with the GCC Runtime Library Exception).
- The self-extracting launcher (`crates/obcr-setup`) uses `miniz_oxide` (MIT OR Zlib OR Apache-2.0).

## Rust crates

Generated with `cargo tree -p obcr -p obcr-setup -e normal --target all`; every licence is on the project's allow list (`deny.toml`).

| Crate | Version | Licence |
|---|---|---|
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| anstream | 1.0.0 | MIT OR Apache-2.0 |
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| anstyle-parse | 1.0.0 | MIT OR Apache-2.0 |
| anstyle-query | 1.1.5 | MIT OR Apache-2.0 |
| anstyle-wincon | 3.0.11 | MIT OR Apache-2.0 |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| atomic-polyfill | 1.0.3 | MIT OR Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 |
| byteorder | 1.5.0 | Unlicense OR MIT |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| clap | 4.6.7 | MIT OR Apache-2.0 |
| clap_builder | 4.6.7 | MIT OR Apache-2.0 |
| clap_derive | 4.6.7 | MIT OR Apache-2.0 |
| clap_lex | 1.1.1 | MIT OR Apache-2.0 |
| cobs | 0.3.0 | MIT OR Apache-2.0 |
| colorchoice | 1.0.5 | MIT OR Apache-2.0 |
| const-oid | 0.10.2 | Apache-2.0 OR MIT |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 |
| crc32fast | 1.5.4 | MIT OR Apache-2.0 |
| critical-section | 1.2.0 | MIT OR Apache-2.0 |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 |
| digest | 0.11.3 | MIT OR Apache-2.0 |
| directories | 6.0.0 | MIT OR Apache-2.0 |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| hash32 | 0.2.1 | MIT OR Apache-2.0 |
| heapless | 0.7.17 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 |
| is_terminal_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| libc | 0.2.191 | MIT OR Apache-2.0 |
| libredox | 0.1.25 | MIT |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| nuked-opl3 | 0.1.0 | LGPL-2.1-or-later |
| once_cell_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| option-ext | 0.2.0 | MPL-2.0 |
| png | 0.18.1 | MIT OR Apache-2.0 |
| postcard | 1.1.3 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| redox_users | 0.5.3 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| sdl3 | 0.22.0 | MIT |
| sdl3-sys | 0.7.2+SDL-3.4.18 | Zlib |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 |
| sha2 | 0.11.1 | MIT OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| spin | 0.9.9 | MIT |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| utf8parse | 0.2.2 | Apache-2.0 OR MIT |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| winnow | 1.0.4 | MIT |
