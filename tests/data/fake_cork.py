#!/usr/bin/env python3
"""Stands in for Folio Cork in the apps' diagram tests (K3-08): "saves" the diagram it was
given as a wider picture, keeping the drawing chunk, then exits."""
import struct
import sys
import time
import zlib

path = sys.argv[1]
data = open(path, "rb").read()
i, chunk = 8, None
while i + 12 <= len(data):
    n = struct.unpack(">I", data[i:i + 4])[0]
    if data[i + 4:i + 8] == b"foLK":
        chunk = data[i:i + 12 + n]
    i += 12 + n


def c(name, body):
    return struct.pack(">I", len(body)) + name + body + struct.pack(">I", zlib.crc32(name + body) & 0xFFFFFFFF)


w, h = 600, 200
raw = b"".join(b"\x00" + bytes([40, 90, 200, 255]) * w for _ in range(h))
png = b"\x89PNG\r\n\x1a\n" + c(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + c(b"IDAT", zlib.compress(raw))
png += (chunk or b"") + c(b"IEND", b"")
time.sleep(0.6)
open(path, "wb").write(png)
time.sleep(0.6)
