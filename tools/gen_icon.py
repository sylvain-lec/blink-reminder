#!/usr/bin/env python3
"""Generate the Blink Reminder app icon with no third-party dependencies.

Renders a rounded-square "eye" icon and writes:
  assets/icon.png   1024x1024 master (used by package-macos.sh -> .icns)
  assets/icon.ico   multi-size Windows icon (embedded in the .exe by build.rs)

Run from the repo root:  python3 tools/gen_icon.py
"""

import math
import os
import struct
import zlib


def _clamp(v, lo=0.0, hi=1.0):
    return lo if v < lo else hi if v > hi else v


def _over(dst, src):
    """Alpha-composite src (r,g,b,a floats 0..1) over dst, return (r,g,b,a)."""
    sr, sg, sb, sa = src
    dr, dg, db, da = dst
    oa = sa + da * (1.0 - sa)
    if oa <= 0.0:
        return (0.0, 0.0, 0.0, 0.0)
    orr = (sr * sa + dr * da * (1.0 - sa)) / oa
    og = (sg * sa + dg * da * (1.0 - sa)) / oa
    ob = (sb * sa + db * da * (1.0 - sa)) / oa
    return (orr, og, ob, oa)


def _rounded_rect_sdf(px, py, cx, cy, hw, hh, r):
    qx = abs(px - cx) - (hw - r)
    qy = abs(py - cy) - (hh - r)
    ax, ay = max(qx, 0.0), max(qy, 0.0)
    return math.hypot(ax, ay) + min(max(qx, qy), 0.0) - r


def render(S):
    """Return a bytearray of S*S RGBA pixels."""
    buf = bytearray(S * S * 4)
    c = S / 2.0

    # Background rounded square with a soft vertical blue gradient.
    margin = S * 0.06
    hw = hh = c - margin
    corner = S * 0.235
    top = (0.30, 0.60, 0.86)     # #4D99DB
    bot = (0.16, 0.40, 0.68)     # #2966AD

    # Eye geometry.
    eye_rx, eye_ry = S * 0.34, S * 0.205
    iris_r = S * 0.135
    pupil_r = S * 0.065
    hi_r = S * 0.045
    hi_x, hi_y = c - S * 0.05, c - S * 0.05

    for y in range(S):
        t = y / (S - 1)
        bg = (
            top[0] + (bot[0] - top[0]) * t,
            top[1] + (bot[1] - top[1]) * t,
            top[2] + (bot[2] - top[2]) * t,
        )
        for x in range(S):
            px, py = x + 0.5, y + 0.5

            # Start transparent, paint background within the rounded rect.
            sdf = _rounded_rect_sdf(px, py, c, c, hw, hh, corner)
            bg_a = _clamp(0.5 - sdf)
            col = (0.0, 0.0, 0.0, 0.0)
            col = _over(col, (bg[0], bg[1], bg[2], bg_a))

            # White eye almond.
            nd = math.hypot((px - c) / eye_rx, (py - c) / eye_ry)
            eye_a = _clamp((1.0 - nd) * min(eye_rx, eye_ry) + 0.5)
            col = _over(col, (0.96, 0.97, 1.0, eye_a))

            # Iris (dark blue) and pupil (near-black), clipped to the eye.
            d = math.hypot(px - c, py - c)
            iris_a = _clamp(iris_r - d + 0.5) * _clamp(eye_a)
            col = _over(col, (0.12, 0.23, 0.45, iris_a))
            pupil_a = _clamp(pupil_r - d + 0.5) * _clamp(eye_a)
            col = _over(col, (0.05, 0.07, 0.12, pupil_a))

            # Specular highlight.
            dh = math.hypot(px - hi_x, py - hi_y)
            hi_a = _clamp(hi_r - dh + 0.5) * 0.9
            col = _over(col, (1.0, 1.0, 1.0, hi_a))

            i = (y * S + x) * 4
            buf[i] = int(_clamp(col[0]) * 255 + 0.5)
            buf[i + 1] = int(_clamp(col[1]) * 255 + 0.5)
            buf[i + 2] = int(_clamp(col[2]) * 255 + 0.5)
            buf[i + 3] = int(_clamp(col[3]) * 255 + 0.5)
    return buf


def png_bytes(S, buf):
    def chunk(typ, data):
        return (
            struct.pack(">I", len(data))
            + typ
            + data
            + struct.pack(">I", zlib.crc32(typ + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", S, S, 8, 6, 0, 0, 0)  # 8-bit RGBA
    stride = S * 4
    raw = bytearray()
    for y in range(S):
        raw.append(0)  # filter: none
        raw += buf[y * stride : (y + 1) * stride]
    idat = zlib.compress(bytes(raw), 9)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


def write_ico(path, pngs):
    n = len(pngs)
    out = struct.pack("<HHH", 0, 1, n)  # reserved, type=icon, count
    offset = 6 + n * 16
    body = b""
    for size, png in pngs:
        dim = 0 if size >= 256 else size  # 0 means 256 in ICO
        out += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png), offset)
        body += png
        offset += len(png)
    with open(path, "wb") as f:
        f.write(out + body)


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    assets = os.path.join(root, "assets")
    os.makedirs(assets, exist_ok=True)

    with open(os.path.join(assets, "icon.png"), "wb") as f:
        f.write(png_bytes(1024, render(1024)))

    ico_sizes = [16, 32, 48, 64, 128, 256]
    write_ico(
        os.path.join(assets, "icon.ico"),
        [(s, png_bytes(s, render(s))) for s in ico_sizes],
    )
    print("wrote assets/icon.png (1024) and assets/icon.ico")


if __name__ == "__main__":
    main()
