# Features

What exists today, at M1. Everything else is in [Roadmap](./roadmap.md).

## Seamless tiling

Block and half-drop repeats, seamless by construction. Each placement is drawn
at all nine lattice neighbour offsets and the result clipped to the tile, so a
motif crossing an edge arrives correctly on the opposite one. No blending, no
seam repair, no edge detection.

The nine offsets come from the lattice vectors `u` and `v`, never from the
tile's width and height. Those two definitions agree for a block repeat and
disagree for every oblique one, which is exactly why the mistake is worth
guarding against explicitly — see
[Test strategy](./test-strategy.md#the-test-that-earns-its-keep).

Brick, quarter-drop and mirror repeats are a constructor each and are not built,
because M1 does not call for them.

## Physical units throughout

Tile size, motif size and placement positions are millimetres. Pixels appear
only at render, derived from a requested DPI. A 120 mm repeat and a 400 mm
repeat are different designs rather than different zoom levels, and the engine
has no representation in which they could be confused.

## Structured placement

Motifs placed by hand at chosen positions, with rotation and scale. Positions
outside the tile are wrapped into it rather than rejected, so a document can
describe a motif as sitting at the edge without arithmetic.

Scatter placement is M2.

## Rendering

- A single tile — the artwork a print bureau receives.
- A field of any size, assembled the way a printer lays tiles out. Three by
  three is the minimum at which tracking corridors become visible, and the
  default worth looking at.
- Any resolution. 72–96 DPI for preview, 150–300 for print.

## PNG export with physical size

Exports carry their resolution in a `pHYs` chunk, so the file states the size it
is meant to be printed at rather than leaving it to be re-established by
whoever opens it.

One inaccuracy worth knowing: PNG records pixels per *metre* as an integer, so
150 DPI is stored as 5906 ppm and reads back as 150.0124 DPI. The error is
0.008%, about 0.02 mm across a 240 mm repeat. It is below the tolerance of every
printing process in scope, but it is not zero.

CMYK, TIFF, ICC profiles and per-colour separations are M5.

## Validation

A pattern is checked before it renders: positive tile size, a lattice that
actually covers the plane, positive scale on every placement, and no palette
slot past the end of the palette. Failures name the offending placement by index
and motif.

## Colour-count reporting

The CLI reports how many palette slots a pattern resolves to, counting slots
used rather than the palette's length. Screen printing charges per colour and
stops being available past a certain count, so this is a manufacturing cost
rather than a statistic — see the open question in
[Commercialisation](./commercialisation.md).

## Command line

```
pattern info   <document.json> [--dpi N]
pattern render <document.json> --out <file.png> [--dpi N] [--cols N] [--rows N]
```

A worked example is in [`examples/peel-club-cherries/`](../examples/peel-club-cherries/).
