#!/usr/bin/env python3
"""Extract the MSFS text-encoding table into a compact binary asset.

The simulator stores compiled text using a position dependent substitution
table. The reference decompiler shipped that table as a generated C# source
file (`TextDecode.cs`) holding 256 rows of the form:

    S[65] = new byte[250] { 107, 70, 106, ... };

This script parses those rows and serialises them into `assets/textdecode.bin`,
which the Rust crate embeds with `include_bytes!`. The binary layout is 256
records, one per source character code, each a little endian `u16` length
followed by that many bytes.

Usage:
    python tools/extract_textdecode.py <TextDecode.cs> [output.bin]
"""

import re
import sys
import struct
from pathlib import Path

ROW_RE = re.compile(r"S\[(\d+)\]\s*=\s*new byte\[\d+\]\s*\{(.*?)\};", re.DOTALL)
NUM_RE = re.compile(r"\d+")


def extract(source: str) -> dict[int, list[int]]:
    rows: dict[int, list[int]] = {}
    for match in ROW_RE.finditer(source):
        index = int(match.group(1))
        values = [int(n) for n in NUM_RE.findall(match.group(2))]
        rows[index] = values
    if not rows:
        raise SystemExit("No S[] rows found; is this the right TextDecode.cs?")
    return rows


def serialise(rows: dict[int, list[int]]) -> bytes:
    out = bytearray()
    for code in range(256):
        values = rows.get(code, [])
        out += struct.pack("<H", len(values))
        out += bytes(values)
    return bytes(out)


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    source_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("assets/textdecode.bin")

    rows = extract(source_path.read_text(encoding="utf-8", errors="replace"))
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(serialise(rows))

    print(f"Rows extracted: {len(rows)}")
    print(f"Wrote {output_path} ({output_path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
