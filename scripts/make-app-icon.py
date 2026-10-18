"""Generates the placeholder application icon source (1024x1024 RGBA PNG) with the standard library only.

Usage: python scripts/make-app-icon.py desktop/src-tauri/icons/app-icon.png
Then:  pnpm --dir desktop tauri icon src-tauri/icons/app-icon.png   (writes every platform size)
The design (a ball on a rounded square) is a placeholder until the product has real branding.
"""

import struct
import sys
import zlib

SIZE = 1024
BG = (0x5B, 0x3F, 0xE0)
BALL = (0xFF, 0xB0, 0x3B)
SHADOW = (0x2E, 0x1F, 0x80)


def coverage(inside, x, y, samples=4):
    hits = 0
    for sy in range(samples):
        for sx in range(samples):
            if inside(x + (sx + 0.5) / samples, y + (sy + 0.5) / samples):
                hits += 1
    return hits / (samples * samples)


def rounded_square(px, py, margin=64, radius=200):
    lo, hi = margin, SIZE - margin
    cx = min(max(px, lo + radius), hi - radius)
    cy = min(max(py, lo + radius), hi - radius)
    return lo <= px <= hi and lo <= py <= hi and (px - cx) ** 2 + (py - cy) ** 2 <= radius**2


def ball(px, py):
    return (px - 512) ** 2 + (py - 430) ** 2 <= 230**2


def shadow(px, py):
    return ((px - 512) / 210) ** 2 + ((py - 790) / 48) ** 2 <= 1


def blend(dst, src, a):
    return tuple(round(d * (1 - a) + s * a) for d, s in zip(dst, src))


def main(path):
    rows = []
    for y in range(SIZE):
        row = bytearray([0])
        for x in range(SIZE):
            a_bg = coverage(rounded_square, x, y) if 40 < x < SIZE - 40 and 40 < y < SIZE - 40 else 0.0
            rgb = BG
            if a_bg > 0:
                if 250 < y < 870 and 270 < x < 760:
                    rgb = blend(rgb, SHADOW, coverage(shadow, x, y))
                    rgb = blend(rgb, BALL, coverage(ball, x, y))
            row += bytes(rgb) + bytes([round(255 * a_bg)])
        rows.append(bytes(row))
    raw = zlib.compress(b"".join(rows), 9)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", raw) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


if __name__ == "__main__":
    main(sys.argv[1])
