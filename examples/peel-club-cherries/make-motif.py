#!/usr/bin/env python3
"""Draw the placeholder cherry this example ships with.

A stand-in until real Peel Club illustrations go through M3's ingest pipeline.
Committed so the asset can be regenerated rather than merely trusted.

Deliberately separate from the synthetic motif in pattern-render's tests: that
one exists to make tiling bugs visible and is drawn lopsided on purpose, this
one exists to be looked at.
"""

from PIL import Image, ImageDraw

SIZE = 512
FRUIT = (200, 30, 45, 255)
SHINE = (236, 120, 125, 255)
STEM = (60, 120, 55, 255)


def main() -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    def circle(cx, cy, r, fill):
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=fill)

    # Stems first, so the fruit sits over where they meet it.
    d.line([(0.37 * SIZE, 0.66 * SIZE), (0.50 * SIZE, 0.18 * SIZE)], fill=STEM, width=14)
    d.line([(0.69 * SIZE, 0.74 * SIZE), (0.52 * SIZE, 0.18 * SIZE)], fill=STEM, width=14)
    d.polygon(
        [
            (0.52 * SIZE, 0.20 * SIZE),
            (0.74 * SIZE, 0.06 * SIZE),
            (0.80 * SIZE, 0.16 * SIZE),
            (0.58 * SIZE, 0.24 * SIZE),
        ],
        fill=STEM,
    )

    circle(0.37 * SIZE, 0.68 * SIZE, 0.24 * SIZE, FRUIT)
    circle(0.69 * SIZE, 0.76 * SIZE, 0.19 * SIZE, FRUIT)
    circle(0.31 * SIZE, 0.62 * SIZE, 0.06 * SIZE, SHINE)
    circle(0.64 * SIZE, 0.71 * SIZE, 0.045 * SIZE, SHINE)

    img.save("cherry.png")
    print("wrote cherry.png")


if __name__ == "__main__":
    main()
