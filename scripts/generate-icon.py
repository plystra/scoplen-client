#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Draw the application icon source from the mark geometry.

    python3 scripts/generate-icon.py && pnpm tauri icon src-tauri/icons/source.png

Writes src-tauri/icons/source.png (1024 x 1024): the display mark in ink on a
paper tile with the platform's rounded-square margin, from
web/ui/src/brand/geometry.json (`scoplen-docs/18-visual-identity.md`).
Requires Python 3 with Pillow.
"""

import json
import math
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
GEOMETRY = json.loads((ROOT / "web/ui/src/brand/geometry.json").read_text())
COLORS = GEOMETRY["colors"]
SIZE = 1024
SUPERSAMPLE = 4


def corners(variant, scale, offset):
    g = GEOMETRY[variant]
    a, b, l, v, h, r = g["start"], g["end"], g["length"], g["vertical"], g["horizontal"], g["radius"]

    def arc(cx, cy, start, end):
        return [(cx + r * math.cos(math.radians(start + (end - start) * i / 16)),
                 cy + r * math.sin(math.radians(start + (end - start) * i / 16))) for i in range(17)]

    top_left = [(a + l, a), (a + l, a + h), (a + v, a + h), (a + v, a + l), (a, a + l)] + arc(a + r, a + r, 180, 270)
    bottom_right = [(b - l, b), (b - l, b - h), (b - v, b - h), (b - v, b - l), (b, b - l)] + arc(b - r, b - r, 0, 90)
    return [[(offset + x * scale, offset + y * scale) for x, y in shape] for shape in (top_left, bottom_right)]


def main():
    big = SIZE * SUPERSAMPLE
    image = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    # The rounded-square tile occupies the central 824 of 1024 units, as the
    # macOS icon grid expects; Windows uses the same artwork.
    margin = 100 * SUPERSAMPLE
    draw.rounded_rectangle((margin, margin, big - margin, big - margin), 185 * SUPERSAMPLE, fill=COLORS["paper"])
    mark = 560 * SUPERSAMPLE
    offset = (big - mark) / 2
    for shape in corners("display", mark / 64, offset):
        draw.polygon(shape, fill=COLORS["ink"])
    out = ROOT / "src-tauri/icons/source.png"
    image.resize((SIZE, SIZE), Image.LANCZOS).save(out, optimize=True)
    print(f"wrote {out.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
