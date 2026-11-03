# Reverse-engineering knowledge base

Everything OBCR knows about the original Operation Body Count, with evidence. Rules for adding to it: [AGENT_GUIDE §4–§5](../AGENT_GUIDE.md#4-evidence-and-confidence).

| File | Contents |
|---|---|
| [DATA_FORMATS.md](DATA_FORMATS.md) | Every original file's format, with confidence tags |
| [SYMBOLS.md](SYMBOLS.md) | Address conventions, segment map, landmark addresses |
| [STRINGS.md](STRINGS.md) | Anchor strings in `COUNT.EXE` with file and DS offsets |
| [DELTA_SPEC.md](DELTA_SPEC.md) | Differences from Wolf3D (`D-###`) and original quirks (`K-###`) |
| [OPEN_QUESTIONS.md](OPEN_QUESTIONS.md) | Unknowns (`Q-###`) with approach and status |
| `LEVEL_CODES.md` | Map plane value meanings (created in M2.R9) |

Machine-readable state lives in [`/re`](../../re/README.md) at the repository root: symbol CSV, type header for Ghidra, trace schemas, capture index.

What never goes here: original data, bulk text dumps, decompiled functions, traces or captures. Those stay in `.local/` (AGENT_GUIDE §10.3).
