# Features

What exists today, at M4. Everything else is in [Roadmap](./roadmap.md).

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

## Motif ingest

One pipeline, whatever the artwork's origin: matte, trim, normalise, quantise,
vectorise. A scanned illustration, a generated image and a motif already in the
library all take the same path, because a motif is just pixels with an alpha
channel and a palette.

**Matting** cuts the subject from its background. Three methods: trust the
file's own alpha, which is right for exported artwork; a fixed luminance
threshold, which only works on a digitally clean background; and the default,
which **measures the paper** rather than assuming a brightness.

The default estimates how bright the paper is across the image — fitting a
plane, so a sheet lit from one side works — and keeps whatever is darker than
its own surroundings by more than the paper's own grain. Both halves are
measured, and both have to be: photographed watercolour paper sits at 0.895
under even light, below any threshold that also removes white; and it varies by
5.5% within itself, while a clean digital background varies by under 1%. A
margin wide enough for the first erases a pale subject on the second. See
[Decisions](./decisions.md#measuring-the-paper-rather-than-assuming-it).

It has one known limit, reported rather than hidden: a plane models gradual
falloff, not a hard-edged cast shadow across the page. Where that happens the
shadow survives as motif, and ingest says so — `border_retained` measures how
much of the frame edge was kept, and above 15% it warns that the fix is a
better photograph rather than a better threshold.

There is no background-removal model. The spec observes that illustrations on
white paper do better without one, and a model would slot in as a fourth method
if real artwork defeats these.

**Trim and normalise** crop to the subject's own extent and record the physical
size it stands for. The order matters: sizing before trimming would make a
motif's scale depend on how much paper it was drawn on.

Bounds come from connected regions, not individual pixels. A bounding box is
decided by its most extreme pixel, and on a photographed sheet that pixel is
grain — enough of it to hold the crop open across the whole frame even when
matting had worked. Grouping into regions and dropping the small ones relative
to the largest separates debris from thin connected features like a stem, and
keeps a motif made of several genuine parts.

**Quantise** maps every pixel onto a palette slot. Colour is flattened; coverage
is not, so a motif's edge stays as soft as it was while its interior becomes
strictly flat. Two things are measured and reported: how far each pixel moved,
and **how much of the motif became the ground**. The second is tracked
separately because it is a worse failure that scores as a mediocre one — a motif
that quantises onto slot 0 is the background colour in the shape of a lemon, and
will not read on fabric at all.

**Derive a palette** from the artwork itself rather than guessing one:
`pattern derive-palette <files…> --colours N`. Median cut over the *solidly*
covered pixels, so neither the paper nor a motif's edge ramp dominates — an
antialiased rim is a large population of tones belonging to neither the motif
nor the background, and median cut allocates buckets by population. Slot 0 is
supplied rather than derived, because the ground is a design decision.

Ask for more slots than you expect to need and look at what comes back: a
colour that covers a small share of the artwork, like a stem, loses the
competition for a bucket before a dominant one does.

**Vectorise** traces each slot into outlines, via `visioncortex`. It works from
the quantised raster rather than the original artwork, which is why it is
simple: tracing normally has to discover regions of similar colour, and
quantisation has already decided them. Paths carry slots, not fills, so a vector
motif recolours like everything else. The raster is always kept alongside,
because textured styles vectorise badly.

## The motif library

A directory of ingested motifs sharing one palette. **Motifs are immutable**:
re-ingesting artwork produces a new version rather than changing the old one, so
a pattern a designer approved months ago keeps rendering as approved.
Re-ingesting byte-identical artwork with identical parameters is a no-op rather
than a duplicate version.

Stored as readable metadata in `motif.json` beside the indexed channels in
`indexed.bin`, two bytes per pixel. The split is not cosmetic — see
[Decisions](./decisions.md#storing-the-indexed-raster-outside-its-metadata).

## Colourways by palette slot

A colourway is the same pattern in a different palette, and since M3 it works.
Motifs store slot indices; rendering resolves slot to colour. Swapping the
palette recolours every motif in a pattern consistently, in one operation, with
no re-ingestion and no change to any placement.

`examples/peel-club-scatter/` carries the same print in two colourways. The two
documents differ by three colours and nothing else.

One caveat, reported rather than hidden: forcing every motif onto one shared
palette is what makes a colourway a single swap, and it is also what quietly
ruins a motif whose colours are not represented. Ingest measures how well each
motif fits and warns when it does not.

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

The CLI reports how many palette slots a print resolves to — the ground plus
every slot the placed motifs actually use, rather than the palette's length. A
house palette of twenty colours drawn on four at a time is a four-colour print.

Screen printing charges per colour and stops being available past a certain
count, so this is a quotable manufacturing cost rather than a statistic — see
the open question in [Commercialisation](./commercialisation.md).

## The app shell

A Tauri 2 desktop app: live 3×3 preview, controls for the repeat and the
scatter, a motif picker, seed reroll, and export.

```bash
cd app && npm install && npm run tauri dev
```

Three decisions in it are worth knowing:

**The preview is fixed at 3×3.** A single tile says nothing about whether a
pattern tracks, and the spec is explicit that this is the view a designer will
judge the tool by. It is not a setting that can be turned down.

**The UI asks in screen pixels; the engine derives the resolution.** That
direction keeps physical units authoritative — resizing the window changes the
render, never the design.

**Colour count and achieved coverage sit beside the preview**, not in a panel
somewhere. One is what a screen printer charges by; the other is what the
density control actually delivered rather than what was asked for.

Renders are coalesced on a short timer and tagged, so dragging a slider asks
for one render rather than one per pixel and a slow render landing late cannot
overwrite a newer one.

Palette editing is deliberately absent — that is M5, and a half-built colour
editor is the kind of partial implementation that gets trusted.

## Command line

```
pattern derive-palette <source.png…> [--colours N] [--ground RRGGBB]
pattern ingest <source.png> --library <dir> --id <name> [--size-mm N] [--from-paper]
pattern info   <document.json> [--dpi N]
pattern render <document.json> --out <file.png> [--dpi N] [--cols N] [--rows N] [--seed N]
```

`--seed` re-runs a document's scatter, which is the seed reroll a designer will
press once there is a UI to press it in.

Worked examples: [`examples/peel-club-cherries/`](../examples/peel-club-cherries/)
places motifs by hand, [`examples/peel-club-scatter/`](../examples/peel-club-scatter/)
scatters them.
