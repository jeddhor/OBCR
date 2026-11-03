# Player control, weapons, pickups, damage and the status bar

Static analysis of `COUNT.EXE` (release `OBC-A`) from the Ghidra text exports in `.local/ghidra/export/` (decompiler output and, where the decompiler fails on jump tables, raw 16-bit disassembly of the executable). Nothing here has been traced in DOSBox-X yet, so the strongest tag is `CONFIRMED` only where the instructions were read one by one.

Conventions: Ghidra base segment 0x1000 (SYMBOLS.md §1); `ghidra SSSS:OOOO` addresses; `DS:0xNNNN` for DGROUP. Wolf3D references are `WOLFSRC/<file>:<line>`.

> Work in progress. Sections are filled as the analysis proceeds.
