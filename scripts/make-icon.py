#!/usr/bin/env python3
"""Genera src-tauri/app-icon.png (1024x1024): fondo #0f0f0f con rombo ámbar #f59e0b. Sin dependencias."""
import struct
import zlib
from pathlib import Path

SIZE = 1024
BG = (0x0F, 0x0F, 0x0F, 255)
FG = (0xF5, 0x9E, 0x0B, 255)
C = SIZE // 2
R = int(SIZE * 0.34)

rows = []
for y in range(SIZE):
    row = bytearray([0])  # filtro None
    dy = abs(y - C)
    for x in range(SIZE):
        px = FG if abs(x - C) + dy <= R else BG
        row += bytes(px)
    rows.append(bytes(row))
raw = b"".join(rows)


def chunk(tag: bytes, data: bytes) -> bytes:
    body = tag + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(raw, 9))
    + chunk(b"IEND", b"")
)

out = Path(__file__).resolve().parent.parent / "src-tauri" / "app-icon.png"
out.write_bytes(png)
print(f"escrito {out}")
