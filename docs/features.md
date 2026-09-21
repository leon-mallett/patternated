# Features

What exists today: M1 to M5. Everything else is in [Roadmap](./roadmap.md).

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

Brick and quarter-drop joined them once the app needed the classic repeats by
name. Mirror is still unbuilt: unlike the others it is more than a constructor,
needing a reflected 2 × 2 super-tile.

## Physical units throughout

Tile size, motif size and placement positions are millimetres. Pixels appear
only at render, derived from a requested DPI. A 120 mm repeat and a 400 mm
repeat are different designs rather than different zoom levels, and the engine
has no representation in which they could be confused.

## Structured placement

Motifs placed by hand at chosen positions, with rotation and scale. Positions
outside the tile are wrapped into it rather than rejected, so a document can
describe a motif as sitting at the edge without arithmetic.

## Classic layouts

Motifs on a regular grid, as an alternative to strewing them — the other half of
the spec's placement model, and what a designer means by a square grid, brick or
a diagonal.

One parameter covers the family. Each row is shifted sideways relative to the
one above, as a fraction of the column spacing: **0** is a square grid, **0.5**
alternates like brickwork, and **a third or a quarter** walks the motifs
steadily across into a diagonal. The shift wraps at the tile edge, which is not
an inconvenience but the reason the diagonal tiles at all.

Five named presets pair a row offset with a repeat: square, brick, half-drop,
diagonal, gentle diagonal. The names are how anybody asks for them.

A grid ignores the seed for position — rerolling changes which motif sits where,
not where the cells are. That is what "classic" means here.

**Overlap prevention applies here too**, and differently. A grid cannot respond
by placing fewer motifs, because its cells are fixed by the rows and columns, so
it shrinks them to fit instead. The limit is the closest any two positions come
on the lattice torus rather than the cell size: on a brick or diagonal offset
the nearest neighbour is a staggered one, and for any layout the closest pair
may be across a tile edge. The preview says when motifs have been shrunk, so a
size control that has stopped moving has a stated reason.

Circular and wave arrangements are deliberately absent. They belong to the ogee
and damask family the spec places in v2, and they are neither a lattice nor a
grid, so building them ad hoc would be the wrong shape.

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

**Overlap prevention, on by default.** Poisson-disc sampling guarantees a
minimum distance between motif *centres*, solved from the coverage target, which
says nothing about where their edges are — so past a certain density motifs
begin to intersect. With the guard on, the distance two motifs need is the
greater of the density's radius and the sum of their own radii: coverage still
controls spacing while it can, and overlap becomes a floor once it cannot.

A pattern asked for more coverage than fits without intersecting therefore lands
short of its target, which the achieved figure reports rather than hides. Turn
the guard off to let motifs layer deliberately.

The radius is measured from the artwork, not its bounding box — a round motif
fills a square box to about 0.71 of the diagonal, and spacing by the box would
hold every pattern 40% further apart than it needs.

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

## Colourways

A colourway is the same pattern in a different palette. Motifs store slot
indices; rendering resolves slot to colour. Swapping the palette recolours every
motif consistently, in one operation, with no re-ingestion and no change to any
placement.

**Editable in the app.** Every slot, including the ground — inverting a print to
a dark background is an ordinary edit rather than a special case, which is why
the ground was made slot 0 in the first place.

**Three derivations**, the ones the spec names: rotate the hue, shift the
lightness, swap figure for ground. Candidates to pick from rather than answers.
Everything moves together, so a palette that was harmonious stays harmonious.

All of it is arithmetic — converting through HSL needs only comparison,
subtraction and division — so a colourway derived today comes back identical on
another machine, and a test greps the module for trigonometry to keep it that
way.

**Named, and stored in the document.** A range sells in several colourways, so
they live in one file rather than in copies of it. That is also what stops them
drifting: a placement edited in one copy and not the others is a range that no
longer coordinates.

Every colourway must have the same number of slots as the palette the motifs
were quantised against, since slot 3 has to mean the same part of the artwork in
all of them. The design palette is pinned as soon as colourways exist, so
switching one can never re-quantise the artwork against it.

One caveat, reported rather than hidden: forcing every motif onto one shared
palette is what makes a colourway a single swap, and it is also what quietly
ruins a motif whose colours are not represented. Ingest measures how well each
motif fits and warns when it does not.

## Rendering

- A single tile — the artwork a print bureau receives.
- A field of any size, assembled the way a printer lays tiles out. Three by
  three is the minimum at which tracking corridors become visible, and the
  default worth looking at. Note that a field is not necessarily a repeating
  unit — see `Lattice::repeating_window`, and the note the command line prints
  when a requested field would not tile.
- Any resolution. 72–96 DPI for preview, 150–300 for print.

## Export

**TIFF** for a print bureau and **PNG** for a quick look, at any resolution.

TIFF is written flat, single-layer, RGB — a print has no transparency, since the
ground is a colour that gets printed like any other and the renderer has already
composited onto it. Compression is off by default, which is the most compatible
thing a bureau can receive; LZW is available, lossless, and about a third the
size.

**Two shapes**, and they are not interchangeable:

- **One tile**, for a printer that steps and repeats it themselves.
- **A pre-repeated sheet**, for one that does not. The tile count is a minimum:
  it is grown to a window that is itself a repeating unit, because a sheet that
  does not tile shows a seam the moment the printer repeats it.

**Physical size is reported before the file dialog**, along with the pixel
count, an estimate of the file size, and how the width compares with a fabric
roll — within a standard 1.6 m roll, past it but inside grand format, or past
3.2 m and unlikely to be printable as one piece. The roll width is a setting,
because the real limit belongs to a specific printer.

**Filenames say what the file is**: `cherries-halfdrop-240x180mm-150dpi.tif`.
The spec calls this out as removing an entire category of mistake, and it is
right — a print file's repeat size and resolution are invisible once it is on
somebody else's machine.

Two accuracies worth knowing. PNG records pixels per *metre* as an integer, so
150 DPI reads back as 150.0124; TIFF records a rational and keeps it exactly.
And the physical size reported is the one implied by the file's pixels rather
than the design's nominal size, because quantising the tile moves it by a
fraction of a millimetre — see below.

## Colour management

Conversion to CMYK happens in the tool, so nobody needs Photoshop to produce a
file for a bureau. **There is no formula anywhere in it.** "It looked different
on screen" is the commonest complaint in fabric printing and is almost always a
formula applied where a profile was needed, so everything goes through an ICC
profile and the profile is a file somebody chose.

**Nothing is bundled, and that is a decision.** Every CMYK profile that is free
to redistribute describes an *offset litho* press, and a digital textile press
is not one. Shipping one as a default would produce a file that looks converted
and is wrong — worse than producing nothing, because the point of colour
management is that the numbers mean something. The bureau has the right
profile; asking for it is a one-line email.

**RGB stays the default.** Many digital textile bureaux prefer it and do their
own separation, because they know their press and you do not.

**Soft proofing** converts to the press's colours and straight back, so anything
it cannot reach comes back changed. An electric green goes visibly flat while a
magenta survives — and seeing that before the fabric is bought is the entire
purpose.

A CMYK export carries its profile inside the file. Without it the file is a set
of numbers nobody can interpret, which is the failure colour management exists
to prevent.

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

**Two ways in.** *New pattern from artwork* takes one or more PNG images, each
showing a single motif, and builds a pattern from them — matting the background
away, deriving a palette from the artwork itself, and scattering at a starting
density. *Open a saved pattern* takes a `.json` document written earlier.

A saved pattern is a small JSON file recording the repeat, the motifs it uses
and where they sit. Motif paths are rewritten relative to wherever the document
is saved, so a project folder can be moved or shared without breaking.

Three further decisions are worth knowing:

**The preview is at least 3×3, grown to a window that repeats.** A single tile
says nothing about whether a pattern tracks, and the spec is explicit that this
is the view a designer will judge the tool by — so it is not a setting that can
be turned down.

It is grown because a rectangle of tiles is not generally a repeating unit even
when every tile in it is perfect. A half-drop gains half a tile of drop per
column, so three columns end half a tile out and the window's left and right
edges cannot meet — which looks exactly like a broken repeat to anyone checking
it the obvious way, by holding opposite edges together. A half-drop therefore
shows 4×3, a quarter-drop 4×3, and a brick — which staggers the other way —
3×4. The header says which.

**The UI asks in screen pixels; the engine derives the resolution.** That
direction keeps physical units authoritative — resizing the window changes the
render, never the design.

**Colour count and achieved coverage sit beside the preview**, not in a panel
somewhere. One is what a screen printer charges by; the other is what the
density control actually delivered rather than what was asked for.

Renders are coalesced on a short timer and tagged, so dragging a slider asks
for one render rather than one per pixel and a slow render landing late cannot
overwrite a newer one.

Each motif has its own size control, in millimetres on its longest edge. Scale
is a design decision rather than a property of the file — the same drawing at
40 mm and 90 mm is two different prints — and changing it does not re-ingest the
artwork. Aspect ratio always comes from the artwork and is never adjusted.

Colour and colourways are edited here too — see
[Colourways](#colourways).

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
