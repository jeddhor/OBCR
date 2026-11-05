"""Disassemble a range of COUNT.EXE (16-bit real mode) by Ghidra seg:off.

Usage: disasm16.py COUNT.EXE SEG:OFF [END_OFF | +COUNT]

Addresses use the Ghidra base segment 0x1000 (image start). Far-call targets
are shown as seg:off too. Needs `capstone` (pip install capstone); keep it
in a local virtualenv such as .local/venv.
"""

import sys

import capstone

HEADER = 0x3400  # MZ header size of release OBC-A


def main() -> None:
    exe = open(sys.argv[1], "rb").read()
    seg_s, off_s = sys.argv[2].split(":")
    seg, off = int(seg_s, 16), int(off_s, 16)
    end = sys.argv[3] if len(sys.argv) > 3 else "+40"
    file_off = (seg - 0x1000) * 16 + off + HEADER
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_16)
    code = exe[file_off : file_off + 0x2000]
    limit = int(end[1:]) if end.startswith("+") else None
    stop = None if limit is not None else int(end, 16)
    for n, ins in enumerate(md.disasm(code, off)):
        if limit is not None and n >= limit:
            break
        if stop is not None and ins.address >= stop:
            break
        text = f"{ins.mnemonic} {ins.op_str}"
        if ins.mnemonic == "lcall" and "[" not in ins.op_str:
            # Capstone prints image-relative segments ("0x942, 0xce8").
            s, o = (int(v.strip(), 16) for v in ins.op_str.split(","))
            text = f"lcall {s + 0x1000:04x}:{o:04x}"
        print(f"{seg:04x}:{ins.address:04x}  {text}")


if __name__ == "__main__":
    main()
