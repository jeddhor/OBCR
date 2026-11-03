# Machine-readable reverse-engineering state

Text files that tools read and write. The Ghidra project in `.local/ghidra/` is rebuilt from these (AGENT_GUIDE §5.1).

| Path | Created in | Contents |
|---|---|---|
| `symbols/count_exe.csv` | M0.12 | Every named address in `COUNT.EXE` (columns in [docs/re/SYMBOLS.md](../docs/re/SYMBOLS.md)) |
| `types/obc_types.h` | M0.12, M2.R6 | Structs and enums for Ghidra's C parser (`objtype`, `statetype`, `gamestate`, …) |
| `traces/schema/trace-v1.md` | M2.R14 | Trace field definitions and their memory locations in the original |
| `captures.toml` | M0.14 | Index of oracle captures and traces in `.local/` (metadata and hashes only) |

Human-readable findings go in [docs/re/](../docs/re/README.md).
