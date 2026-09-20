# Features

What exists today, at M2. Everything else is in [Roadmap](./roadmap.md).

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

## Scatter placement

Many motifs distributed across the tile to read as hand-strewn, by toroidal
Poisson-disc sampling. Every pair is at least a minimum distance apart while the
distribution stays irregular, which is what separates a tossed print from
uniform random sampling — the latter clumps, leaves bald patches, and lays down
the corridors a half-drop exists to avoid.

**Distance is measured on the lattice, not on the tile rectangle.** This is the
same invariant as the tiling, in a second place, and it is the one the spec's
own formula gets wrong: wrapping each axis by width and height is right for a
block repeat and wrong for every oblique one. On a 240 × 180 mm half-drop,
motifs at (10, 10) and (230, 95) are 87 mm apart by that formula and 21 mm apart
in the pattern that reaches fabric.

Four controls, none of them a separation radius:

- **Density** — target coverage as a fraction of tile area, solved into a
  radius using the mix's weighted mean motif area. Achieved coverage lands
  within about five percentage points, and is reported by `pattern info`.
- **Rotation jitter** — bounded either way, in degrees.
- **Scale variation** — an inclusive multiplier range, typically 0.8 to 1.2.
- **Motif mix** — relative weights, so a print can be six parts cherries to one
  part pineapples.

A document carrying both authored placements and a scatter is refused rather
than resolved, since there is no sensible merge: a scatter's separation
guarantee knows nothing about motifs it did not place.

## Reproducibility from a seed

Placement generation contains no transcendental arithmetic — no sine, no cosine,
no power — so it uses only operations IEEE-754 pins down exactly and the same
seed gives the same placements on any platform. The annulus sampling that
Bridson's algorithm normally does with an angle and a radius is done here by
rejection in the bounding square instead, which costs a 41% rejection rate and
buys that guarantee. A test greps the module and fails if a trigonometric call
appears.

The float conversion from generator bits is written in this codebase rather than
taken from a distribution crate, so a dependency upgrade cannot silently move
it.

This is placement only. **Rendering still rotates through `sin_cos`**, so the
cross-platform claim stops at the pixels — see
[Test strategy](./test-strategy.md#what-is-deliberately-not-tested).

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
pattern render <document.json> --out <file.png> [--dpi N] [--cols N] [--rows N] [--seed N]
```

`--seed` re-runs a document's scatter, which is the seed reroll a designer will
press once there is a UI to press it in.

Worked examples: [`examples/peel-club-cherries/`](../examples/peel-club-cherries/)
places motifs by hand, [`examples/peel-club-scatter/`](../examples/peel-club-scatter/)
scatters them.
