# Features

What exists today: M1 to M5, and the mockup half of M6. Everything else is in
[Roadmap](./roadmap.md).

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
name. Mirror joined later and is a different kind of thing — see
[Mirror repeats](#mirror-repeats).

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

Two parameters cover the family. Each row is shifted sideways relative to the
one above, and each column is dropped relative to the one to its left, both as
a fraction of a cell: **0** is a square grid, a row offset of **0.5**
alternates like brickwork, a column offset of **0.5** is a half-drop, and **a
third or a quarter** sideways walks the motifs steadily across into a diagonal.
The shift wraps at the tile edge, which is not an inconvenience but the reason
the diagonal tiles at all.

Five named presets: square, brick, half-drop, diagonal, gentle diagonal. The
names are how anybody asks for them.

**Grids sit on a block lattice, always.** The offsets make the pattern; the
lattice does not help. Each preset used to pair an offset with an oblique
lattice — a diagonal was a shifting grid on a half-drop — and that was wrong
twice over. Wherever the drop produced a regular field the result was point for
point what a block lattice gives, so it added nothing; and where it did not, it
broke the field along every tile edge. **A repeat's drop belongs to the cells,
not to the tile.**

It also left brick and half-drop indistinguishable except by that lattice,
since both were a row offset of a half. They are different patterns, and now
they are different parameters.

### The grid has to close over the tile

A row offset walks the motifs sideways and wraps at the tile edge. For the walk
to continue into the next tile rather than jump, the accumulated shift over a
whole tile — `rows × row_offset` — has to come out a whole number of columns. A
third of a column over five rows does not, so the diagonal arrives at the join
one and two thirds of a column out and the field breaks along every tile edge.
Six rows does, and it runs clean.

The grid therefore grows to the smallest size at least as large as the one
asked for whose offsets close, and the app says which size it drew. That is
[`Lattice::repeating_window`](#seamless-tiling) one level down: there a
rectangle of tiles has to be a repeating unit, here a rectangle of cells has to
be.

Reported from the app as a diagonal that broke down the middle of the preview.
Every tile was drawn correctly and the tiling was exact; what did not line up
was the diagonal's own rhythm.

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

## Mirror repeats

The one classic repeat that is not a translation. Block, brick, half-drop and
quarter-drop are all the same tile slid somewhere else, which is why a pair of
lattice vectors describes all four. A mirror reflects, and no choice of `u` and
`v` produces one.

What it is, in terms the engine already had: part of the tile is designed and
the rest is its reflection. Half the tile for a side-to-side or top-to-bottom
mirror, a quarter for a four-way. The tile as a whole then repeats by ordinary
block translation, so the lattice stays block and the tile stays the size the
designer asked for — **choosing a mirror subdivides the repeat rather than
doubling it**, because a 300 mm repeat is a decision about the print and not a
consequence of how it was built.

Seamlessness comes free twice over. The reflected quadrants meet along the
mirror axes by construction, and the tile meets its own translates because the
designed cell is still sampled on the torus. Nothing is repaired at either
join.

Three things follow, and each needed building:

- **A motif near a mirror axis is near itself.** Its reflection is a second
  copy the sampler never compares it with, because that copy does not exist
  when the candidate is judged. Left unchecked, motifs sit astride the axes and
  come back doubled — the butterfly a mirror is famous for when it is not
  wanted. Overlap prevention therefore covers a motif against its own
  reflections as well as against other motifs.
- **Two motifs either side of an axis are as close as their reflections put
  them**, which is a pair that never appears in the designed cell at all.
  Separation is measured over the reflections as well as the translations.
- **A mirror needs a block lattice.** Its reflections generate translations by
  the tile's own width and height, which no oblique lattice contains. The
  combination is refused rather than rendered, and choosing a mirror in the app
  puts the repeat back on a block lattice so a designer who tried a diagonal a
  moment ago is not left holding a pattern that will not draw.

The practical consequence for a designer: a mirror asks more of the
motif-to-repeat ratio than a scatter does. The cell is smaller, and nothing may
come within its own radius of an axis, so a motif that filled a tossed print
comfortably leaves a bare corridor down each mirror line. Shrink the motif,
enlarge the repeat, or put motifs on the lines deliberately — see below.
`examples/generated-motifs/bananas-mirror.json` is the same artwork as
`bananas.json` at 45% of the size, which is what it took to fill.

## Motifs on the mirror lines

The damask move, and the corridor's proper remedy rather than a workaround for
it. **On the lines** is a share: that proportion of motifs is placed *on* a
mirror axis instead of clear of one.

A motif exactly on an axis is reflected onto itself. It is cut at the line and
its own reflection supplies the other half, so it comes out bilaterally
symmetrical — a formal ornament sitting among the strewn motifs, which is the
whole visual language of a damask.

Three things had to be true for that to work, and each is a rule rather than a
special case:

- **A reflection that leaves a motif where it is symmetrises it; one that moves
  it must clear it.** That single distinction is what lets a motif sit on a
  line while a motif *near* a line is still refused. Near is the bad case: it
  gets cut at the line and doubled lopsidedly, which is the butterfly nobody
  asked for. On a four-way mirror the rule works per reflection, so a motif
  mounted on the vertical line still has to clear its top-to-bottom reflection.
- **Each copy draws only on its own side of the line.** Drawing a motif and its
  whole reflection over each other is not a mirror: compositing is not
  commutative, so the overlap takes its colour from whichever was painted last
  and the result is not symmetrical at all. Clipping each copy to its own half
  is what a designer does by hand — draw half, mirror it — and it makes every
  case exact, including a motif that straddles a line rather than sitting on
  one.
- **A mirrored tile is an even number of pixels across.** The mirror line then
  falls between two pixels rather than through one, so a motif mounted on it
  lands exactly on its own reflection. It costs at most a pixel of repeat.

Both mirror lines get ornaments — the one through the middle of the tile and
the one along its edge. Leaving either bare would put the corridor back on every
second line.

Only one coordinate is ever snapped, the one already nearer to a line. Snapping
both would put every mounted motif on a corner where four reflections meet,
which is a far stronger and rarer ornament than "motifs on the lines" implies.

The realised share runs below the one asked for, because an axis position is
more constrained than a free one and more candidates are refused. Mounting
still places *more* motifs overall, since the corridor stops being wasted.
`examples/generated-motifs/bananas-damask.json` is `bananas.json` at full size
with the share at 45%, which is the other way out of the corridor.

## Stripes, checks and ginghams

The first thing in this engine that is not a placed motif. A stripe is not a
motif anywhere — it is the fabric being a different colour in bands — and
expressing one as a very tall thin motif works right up until two of them
cross.

**The crossing is the whole reason bands are their own construct.** Where a
vertical band meets a horizontal one the result is neither band's colour but a
third, and a motif drawn over another motif cannot produce a colour that
neither contains. That third tone is what makes a gingham a gingham: two
colours, three tones. Crossings are a short list of "these two slots meet as
this one" rather than a matrix, because a check uses one entry and a matrix of
every slot against every other would be mostly empty and entirely
hand-maintained. Without a matching entry the horizontal band wins, which is
what drawing one over the other would have done anyway.

**Bands sit between the ground and the motifs.** The fabric is filled, the
bands are laid over it, the motifs go on top. That ordering is the design
decision, and it is what makes a check with flowers tossed across it cost
nothing — it is the two things it looks like rather than a special case.

Slots throughout, never colours, so a check recolours with a colourway like
everything else, and its slots are counted in what a printer would charge for.

### Two constraints, both familiar

**The sequence has to close over the tile.** A sequence that does not divide
the tile is cut at its edge, and since the next tile starts the sequence again
the join carries a band of the wrong width — down every repeat, on every
metre. The widths are scaled to the nearest whole number of repeats rather than
the tile being grown, because the repeat size is a decision about the print and
a band width is a proportion nobody will miss half a millimetre of.

**Bands need a block lattice**, and this one was found by the test rather than
by reasoning. A band sequence lives in the tile and repeats with it, so on a
half-drop the neighbouring column arrives half a tile down and the stripes step
at every join. The combination is refused rather than rendered.

That is now the third time the same shape has appeared: a rectangle of tiles
has to be a repeating unit, a grid's cells have to close over the tile, and a
band sequence has to as well. Worth naming as a family — **anything laid out
inside the tile has to agree with how the tile repeats**, and an oblique
lattice is what most often disagrees.

`examples/generated-motifs/bananas-gingham.json` and `bananas-stripe.json` are
both the banana artwork over a banded ground.

### In the app

A **Ground** panel edits the sequence directly: a row per band, each with a
colour and a width, with rows added, removed, or turned into gaps. The named
grounds — stripe, pinstripe, check, windowpane, tartan — are **starting points
that fill the sequence in**, not modes, which is why a tartan needs no special
support: it is a longer sequence.

**Colours go down and slots come back.** The editor names colours and the
engine allocates palette slots behind it, so the interface never handles a
slot. The ground owns the *tail* of the palette, so changing it is a truncate
and an append rather than an index map that would have to stay in step with
every other palette operation — and a crossing may only borrow a slot from
within that tail, or the next rebuild would truncate away a motif's colour.

**Mirroring is applied once, not remembered.** "Mirror the sequence" writes the
reflected bands into the list, so what is stored is the repeat and what is
shown is the repeat. A remembered flag would have to be guessed at when a
document was reopened, and guessing it wrong would reflect an
already-reflected sett.

Choosing a ground moves the repeat back to a block lattice. That is held in the
parameter path rather than the layout switch, because a document that places
its motifs by hand never reaches the layout switch and can have a striped
ground too.

### What happens where two bands cross

Three answers, because there are three real behaviours:

- **Blend** — the default, and what cloth does. Half the threads at a crossing
  are each colour, so the result is their average, and **a thread crossing
  itself stays itself**.
- **Overprint** — multiply, which is what a second pass of translucent ink
  does. Right for a design built from overlapping bands.
- **Across-band wins** — nothing extra, and the fewest colours to print.

This was got wrong first. Crossings multiplied to begin with, on the reasoning
that a crossing is a second pass of ink, and the first tartan rendered came out
**very nearly black** — multiplying navy by navy is far darker than navy.
Correct for ink, wrong for cloth, and obvious in the render while invisible in
the model.

The same correction shows why a check needs **two threads rather than a thread
and a gap**: a gap has nothing to blend with, so an ink and a gap give two
tones, while an ink and a pale thread give three. That is what makes a gingham
a gingham, and it is why the check preset writes a pale thread rather than a
gap.

`examples/generated-motifs/bananas-tartan.json` is a Black Watch sett — four
threads, ten crossings, six of them new colours, reflected about its pivots.

## Grounds are a kind, not a setting

Bands were the first ground, and the shape they needed turned out to be
general: a function from a point in the tile to a slot, or to nothing where the
fabric shows. `Pattern.ground` is therefore an enum tagged by `kind` in the
document, and everything below it — palette resolution, colourways, colour
counting, seamlessness, the block-lattice requirement — stops knowing which
kind it has.

Sampling one takes a step: `Ground::laid_over(tile_w, tile_h)`. No ground can
be sampled without knowing its tile. For bands that means fitting the sequence
so it closes; for a texture it means growing the field. Making it one call
means neither can be skipped by accident.

One ground, not a stack of them — nothing has yet wanted a texture underneath a
check.

## Grown grounds

Reaction–diffusion: two notional chemicals spread across the tile and consume
each other, and after a few thousand steps the field has organised itself into
spots, or worms, or a maze. Nobody positions anything.

It is here because it produces the one thing placement cannot: **pattern with
no visible grid in it.** A tossed layout still repeats on a lattice and the eye
finds it. A grown field has no lattice to find, only the tile's own period.

**Why it tiles.** The field lives on a torus. Every Laplacian reads its
neighbours modulo the grid, so the left column's left neighbour *is* the right
column — not a copy of it, the same cells in the same simulation. Seamless by
construction rather than repaired at the edges, which is the only thing that
could work: a blend or a mirror at the seam would be visible precisely because
the interior has no straight lines to hide it in. Like bands, it needs a block
lattice, and for a sharper version of the same reason — a half-drop tile's
right edge is not the edge the field was grown against.

**Six growths**, each a pair of feed and kill rates: Spots, Pebbles, Rings,
Worms, Maze, Coral. The interesting band of that parameter space is a sliver a
few thousandths wide and either side of it the field dies flat or floods solid,
so these are coordinates known to sit inside it. They were found by sweeping
and looking, not by citation — the rates in the usual write-ups are quoted
against a different Laplacian and are dead in this one.

**Scale, not quality.** The cell count is how large the spots are, not how
finely they are resolved: doubling it grows twice as many, half the size.

**Levels.** A colour per band of concentration, sparsest first, any of which
may be the fabric showing through — which is how the same growth gives
light-on-dark as well as dark-on-light without needing two of everything. The
thresholds are evenly spaced by concentration rather than by area, because a
spot print is mostly empty and splitting it into equal shares would turn a
scatter of dots into half the fabric. The cost is that a level can come out
covering nothing, so the interface reports what each one actually covered.

**Sampled between the cells, not at them.** The grown field keeps its
concentrations and the levels are cut on the way out, so an edge is where the
concentration crosses a threshold rather than where one cell ends. A cell is
about 1.5 mm at a typical scale — eighteen pixels at print resolution — so
classifying per cell would put an eighteen-pixel staircase along every curve,
and the cell count would be controlling the raggedness of the edges rather than
the size of the shapes. The interpolation wraps like everything else, which is
what keeps the join invisible at the one place it would be easiest to see.

**Parameters, never the field.** A 192-cell grid is 36,864 floats, larger than
every other part of a document put together. The document stores the growth,
the scale, the steps and the seed.

`examples/generated-motifs/bananas-texture.json` is bananas tossed over a grown
pebble ground — two layers, both repeating, one of them with no lattice in it.

## Drawn grounds

The opposite pole from a grown one. A texture has no lattice in it anywhere;
this is all lattice — a cell size, a figure, and a rule for what a figure
contains. Interlocking curves, strapwork, clamshell and the classical square
fractals: the ornamental half of what a repeat print is made of, and the half
reaction–diffusion cannot reach.

**Six figures.** Truchet (two quarter arcs, corner to corner), Weave (the same
two connections drawn straight), Scale (clamshell), and three that contain
smaller copies of themselves — Carpet (Sierpinski), Dust (Cantor) and Cross
(Vicsek). The recursive three answer to a depth; the drawn three do not, and
the panel does not offer them a control that would do nothing.

**Why it tiles.** The grid is fitted to the tile — a whole number of cells each
way, by the arithmetic that fits a band sequence and for the same reason. Every
cell is then evaluated in its own coordinates and knows nothing of its
neighbours, so the tile's last column and its first are two ordinary cells that
happen to meet.

**Why the line work joins.** Every drawn figure meets the cell edge at the
*middle of the edge and nowhere else*. Two neighbouring cells therefore join
whatever they drew, which is Truchet's observation and the whole reason his
tiles work — and it holds across the tile boundary as well, so the repeat joins
without anything being done about it. A first attempt drew the Weave as a
corner-to-corner diagonal, which meets the cell at its *corners*, and the lines
broke at every edge whose two cells had turned opposite ways.

**The bricked figure, and a fifth sighting of the same rule.** A scallop only
reads as one when the row below breaks the joint of the row above, which is a
brick offset — and the shift is decided by the row index's parity, so it closes
over the tile only over an **even** number of rows. An odd count puts two
unshifted rows against each other at the join: a doubled row, once down the
fabric. This is the same obligation as a band sequence closing, a grid's cells
closing, a preview window being a repeating unit and a mirror needing a block
lattice. Anything laid out inside the tile has to agree with how the tile
repeats.

**Variation and the seed.** At zero every cell is the classical figure and the
seed does nothing — a Sierpinski carpet is a Sierpinski carpet. Above it a
block here and there is left whole instead of being divided, which is what
turns a diagram into something that looks drawn. The interface disables *Draw
another* while variation is zero rather than offering a control that cannot
change anything.

**The cell size is a wish.** The grid has to close over the repeat, so the
request is rounded to the nearest size that does and the panel reports the grid
it actually drew.

`examples/generated-motifs/bananas-ornament.json` is bananas tossed over
Truchet strapwork.

## A ground's edges are drawn, not decided

Every kind of ground is sampled sixteen times per pixel and averaged. For the
first three years of this engine's existence that would have been waste: a
stripe's edges are axis-aligned and one sample per pixel puts them exactly
where they belong. A Truchet arc and a grown blob are curves, and one sample
per pixel puts a staircase along every one — the more visible for sitting
beside motifs, which are drawn with filtering, so the two layers of the same
print did not look as though they had been made by the same tool.

Reported from the app as the grounds looking "very pixelated", which was the
right description.

Averaged in sRGB rather than in a linear space, to match how the motif layer
composites. Two layers blending differently would trade one visible
inconsistency for a subtler one.

**What this does not change.** The palette. Intermediate tones along an edge
are the same thing a motif's antialiased outline already produced; the colour
count is a statement about the design's slots, not about how many distinct
values end up in the file. A screen-printing separation would want the
unsoftened mask, and that is noted where separations are.

## The panel is composed for the workflow

A pattern made entirely of a ground and a pattern of motifs strewn over one are
different jobs. The controls are composed for whichever is in hand rather than
filtered down from one fixed list:

- **Ground-only** — Ground, Repeat, Background, Colourways. Nothing about
  arranging motifs, reducing their colours or editing them, because there are
  none.
- **With artwork** — Layout, Repeat, the scatter controls, Colours, Background,
  Ground, Motif colours, Colourways.

The ground leads when it *is* the design and sits under the background when it
is what motifs are strewn on. Within a panel the same rule applies: an ornament
shows a depth only for the figures that recurse and a line width only for the
ones that are stroked, and the repeat type stops offering the three oblique
lattices once a ground is present, since a ground needs a block repeat and the
engine would refuse them.

The rule behind all of it: a control that cannot act is worse than a shorter
panel, because it gets moved, nothing happens, and the rest of the interface
stops being believed.

## What sets how many motifs there are

Not the same control in both layouts, which is why the panel says which.

On a **scatter** it is coverage, a share of the fabric rather than a count — so
shrinking a motif puts more of them down to cover the same area, and the
coverage readout reports what was actually reached rather than what was asked
for. On a **grid** it is rows times columns; the cells are fixed, so neither
the size nor the mix can change the count.

A motif's **share** is not part of that answer in either layout. It is relative
— six against one is six times as common, and raising every share together
changes nothing at all — so it decides *which* motif goes down, never how many.
It is shown as a percentage of the mix, so that moving one slider visibly moves
its neighbours' readouts, and it is not shown at all when a document has a
single motif, which is the whole mix however the slider is set.

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

### Finding neighbours without comparing everything

Each candidate is tested against the samples near it, found through a spatial
index built in **lattice coordinates** rather than over the tile rectangle.

That distinction is the whole reason the index took as long to arrive as it
did. The grid this algorithm is usually written with indexes the rectangle, and
on an oblique lattice the rectangle is the wrong neighbourhood — two motifs a
whole tile apart may be adjacent in the pattern. Indexing the lattice makes
wrapping a modulo on the bucket number and keeps the neighbourhood honest for
any repeat, including a mirror, where a sample can threaten a candidate through
its reflection and so every image of the candidate is looked up.

The index returns a **superset** and nothing more: everything it offers still
goes through the exact distance test. So the accept and reject decisions are
identical to comparing against every sample, the random draw is therefore
identical, and the placements are unchanged — which the golden image, itself a
scatter, is what actually holds to account.

Measured on a 1.2 m repeat, which is what prompted it:

| motifs | before | after |
| --- | --- | --- |
| 1,081 | 0.30 s | 0.15 s |
| 4,358 | 2.70 s | 0.22 s |
| 17,306 | 38.5 s | 0.53 s |
| 48,109 | minutes | 1.27 s |

The bucket grid is sized by how many samples are going to arrive rather than by
a fixed cap, because both ends cost: a fine grid in a sparse cell spends more on
allocating empty buckets than the scan saves, and a coarse one in a dense cell
puts the quadratic back inside each bucket. A fixed cap of four thousand buckets
made the forty-eight-thousand-motif case six times slower.

## Tracking: a ranking, not a verdict

Corridors — motifs falling into rivers with empty lanes between them — are the
fault a designer looks for and the engine could not name. It now measures how
strongly a scatter's near neighbours line up, and says *worth a look* above a
calibrated score.

Deliberately a ranking. At the level where a person would reject a print it
catches roughly two in three, and it is blind to faint tracking, so a high
score is reliably worth looking at and a low score is not a guarantee. The
interface says exactly that rather than implying a verdict.

It is the third attempt; the two before it did not discriminate at all and were
removed. What settled this one was rendering three hundred *unmodified*
scatters and checking that the measure's ranking agreed with the eye — the
highest scorers track visibly, the lowest look evenly strewn. The numbers, the
calibration and what it would take to become the blocking gate Dreamwoven needs
are in [Decisions](./decisions.md#tracking-detection-at-the-third-attempt-as-a-ranking-and-not-a-gate).

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

## Drawing artwork smaller than it was drawn

Almost every render minifies. Artwork arrives at illustration resolution — a
thousand pixels or more — and a 100 mm motif is under two hundred pixels at
preview scale and about six hundred at print.

Each motif carries a chain of successively halved copies, built by integer box
average so that every source pixel contributes to the level below. Rendering
picks the smallest level that is still at least as large as the target, leaving
the final resample a reduction of under a factor of two.

**Why it needs saying at all.** Bilinear sampling reads a 2 × 2 neighbourhood,
so reducing by eight in one step looks at four source pixels in every
sixty-four and discards the rest. A dark keyline a few pixels wide falls
between samples and vanishes in places — reported from the app as "the preview
is far worse than the source", which was the right description: it was not
merely smaller, it was wrong. Measured against a correct reduction at preview
scale, mean channel error fell from 5.9 to 3.8 and the worst pixel from 166 to
102.

The chain is integer arithmetic throughout, because it sits upstream of
everything a seed is supposed to reproduce.

## Palette size

How many colours the artwork is reduced to, the ground included, from
**monotone** at one end to **photographic** at the other. Twelve by default.

This is a commercial decision as much as a visual one, which is why it is a
control rather than a constant: screen printing charges by the colour and
digital printing does not, so Peel Club and Dreamwoven will not want the same
answer. Changing it re-reads the artwork and quantises every motif again, so it
is a heavier operation than a palette edit — changing a slot's *colour* leaves
the slots alone.

Colourways cannot survive a change of slot count: a colourway names a colour
per slot, and there is a different number of slots afterwards. They are
discarded and named rather than left to fail on the next open, which would be a
document that no longer loads. The ground is carried across rather than
re-derived, because a designer who has set a navy ground should not have it
revert because they asked for more colours.

### When a motif dissolves into the ground

Each motif reports the share of its ink that resolves to the ground slot, and
the app says so above 8%. Ink the colour of the fabric it is printed on is
invisible on a plain preview and turns into a hole the shape of whatever
dissolved the moment the ground changes.

It is not a fault in itself — a white star on a navy ground is exactly this,
correctly — but it is almost always a palette with no colour for that part of
the artwork, so the nearest available is the ground. A peeled banana's pale
interior against the old six-colour default put 18% of the motif there and
nothing warned, because the quantiser's own threshold looks for a motif that
has vanished entirely rather than one with a hole in it. Widening the palette
to ten dropped it to 2%.

## Drawings, kept as drawings

A motif can arrive as an SVG, and when it does its outlines are kept rather
than sampled. Artwork is a picture of a thing and has one true resolution; a
drawing is the thing a picture would be *of*, and sampling it once at import
throws away the only reason it was vector. The same motif is sixty pixels
across in a preview and six hundred in a print, and it is drawn afresh at both.

**Slots, not colours, as everywhere else.** Each shape carries a palette slot,
so a colourway recolours a drawing and a photograph by the same mechanism. It
is cleaner here: a fill is one flat colour by construction, so the quantisation
a photograph needs is a lookup. And because reading and quantising are separate
steps, changing a document's colour count re-quantises a drawing from its own
exact colours rather than from a rasterisation of them.

**Strokes become outlines at import**, so that everything downstream is one
kind of thing, and it stays exact at any resolution because it is done on
geometry.

**A drawing that cannot be outlines is drawn instead.** A gradient has no
single colour a palette slot could stand for, so it is rendered and quantised
like any other artwork — which is exactly what exporting a PNG by hand would
have achieved, done for you. The trade is stated: that motif is pixels from
then on and will not sharpen when printed large.

This also fires where a drawing *mostly* reads as outlines. One gradient among
twenty flat shapes reads perfectly well and produces a motif with a shape
silently missing, which is the wrong picture rather than a softer one — and
only one of those is obvious on screen.

**What is still refused, and why by name.** Live text needs a font, and an
embedded bitmap needs a decoder this engine does not carry; rendering would
drop both exactly as reading them did, having given up the drawing's sharpness
for nothing. Each is reported with what to do about it in the drawing program
rather than as a parse failure.

**Live text is refused, and that is a determinism decision.** Type would be
resolved through the fonts installed on *this* machine, so the same file would
print differently elsewhere — which is the one promise this engine cannot bend.
Outlining text before export is one checkbox.

**The declared size is honoured.** A drawing that says it is 46 × 62 mm arrives
at those proportions, scaled so its longest edge is whatever was asked for.

`examples/drawings/pears.json` is a drawing tossed as a half-drop repeat.

## What the ingest wants to say

Bringing artwork in can succeed and still be worth remarking on: most of a
motif dissolving into the fabric colour, a shadow kept as ink where matting
could not model it, a drawing carrying something no press can print. These are
returned as notes rather than printed, and the app shows them above the
preview — dismissible, because the artwork did come in and a warning that
cannot be put away is one that gets ignored.

They used to go to stderr from inside the engine, where the desktop app could
not see them at all. The two most common are also the two most likely to reach
a printer unnoticed.

## Motif ingest

One pipeline, whatever the artwork's origin: matte, trim, normalise, quantise,
vectorise. A scanned illustration, a generated image and a motif already in the
library all take the same path, because a motif is just pixels with an alpha
channel and a palette.

**Matting** cuts the subject from its background. Three methods: trust the
file's own alpha, which is right for exported artwork; a fixed luminance
threshold, which only works on a digitally clean background; and the one that
**measures the paper** rather than assuming a brightness.

Which to use is decided from the image rather than configured. Artwork carrying
alpha has been cut out already, and measuring paper that is not there removes
the subject — a transparent PNG has no paper, so the brightest thing in frame is
the motif.

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

**Editable in the app.** The background has its own control and the motif
colours sit beside it, but they are the same mechanism: the ground is slot 0 of
the palette and nothing more. That is why inverting a print to a dark background
costs nothing, and why the background recolours with a colourway like everything
else.

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

## A motif can be required to appear

A scatter mix entry carries `required`. Weight is a tendency and a tendency is
not a guarantee — a motif holding a few per cent of the mix is absent from
about one tile in five, and adding placements barely moves that — so anything
that *must* be on the cloth says so.

A required motif takes part of each sample's dart budget, which places it while
the tile is still emptiest. `scatter_with_outcome` returns the placements
alongside `unplaced_required`: the motifs it could not fit, by name. An empty
list is the ordinary case; a non-empty one means the tile is too small for
everything asked of it, which the caller can act on and silence could not.

A grid refuses the flag rather than ignoring it, having no coverage guarantee
to offer.

Defaults to false, so every pattern that predates it behaves exactly as it did.

See [From brief to pattern](./from-brief-to-pattern.md#every-named-interest-appears).

## A repeat is for a garment

A 240 mm repeat reads completely differently on an adult shirt and on a
three-year-old's, so a repeat without a garment beside it is a number with no
scale attached. A document records which garment its measurements are for, and
changing that cuts the same design for the new one: the size chart sets a
factor on **both the tile and the motifs within it**, because scaling the tile
alone leaves one enormous motif on a small garment — which is the failure the
spec names.

Everything measured in millimetres moves together: the repeat, the lattice that
steps it, where each motif sits, a band's widths, an ornament's cell and
stroke. What is *counted* rather than measured is left alone — a texture's
cells are how many fit across the tile, so they follow it already.

**Scaled from the recorded reference, never compounded.** Switching between
sizes and back returns the design rather than drifting a little further each
time. A document that has never been told which garment it is for is simply
told, and nothing is scaled: there is no factor without two sizes, and guessing
that an untold document meant "adult" would silently resize somebody's work the
first time they touched the control.

Distinct from the garment *preview*, which shows a print on a body without
changing it. One is a lens; this is a decision.

## Garment mockups

The print on a garment, at true scale. Eighteen to choose from, built from
standard size-chart figures, in seven cuts: tops short- and long-sleeved,
trousers, shorts, dungarees, a bucket hat, a tote bag, a cushion, and pyjamas —
a top and trousers side by side in the one cloth, which is what makes them a
pair rather than two garments that happen to match.

The size range is the point. A 240 mm repeat reads completely differently on an
adult shirt and a three-year-old's, and the only way to see that is to look.
Beside each is **repeats across** — two is a bold print, eleven is a texture, a
fifth of one is a placement print that happens to be tiled. Across the chest on
a top, the hip on trousers, the brim on a hat: one number that means the same
thing for every shape, because it is also what a repeat is scaled against when
a design is [cut for a garment](#a-repeat-is-for-a-garment).

A long sleeve hangs rather than sticking out. Reaching in proportion to its
length is right for a short sleeve and gives a long-sleeved top a T-pose three
times as wide as it is tall — which defeats the only thing this view is for,
because the body it is meant to be judged against becomes a strip in the
middle.

A silhouette rather than a photograph. Warping a print onto folds is a much
larger piece of work, and it would make the one thing this exists for — the
relationship between the repeat and the body — harder to judge rather than
easier.

The resolution follows from the garment's real width, so nothing in the render
can pick a size that flatters the print. The garment view follows the pattern
live, so a scale judgement is made against what is actually on screen.

The measurements are approximate standard figures rather than any one
manufacturer's chart: right enough to judge a repeat against, and not a
substitute for a real spec.

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

## Getting back out

**Close** returns the app to its start screen, where New and Open live. Until
it existed there was no way out of a document: trying a pattern and then
wanting a different one meant quitting and relaunching.

The engine refuses to close while there is unsaved work, and says which kind —
a pattern that has never been written at all, or one changed since it was last
saved. The interface then asks the question that situation calls for rather
than a blanket "are you sure". Keeping the refusal in the engine and the
question in the interface means the rule can be tested; a backend that puts up
its own dialog cannot be.

Whether there is unsaved work is answered by **comparing the document against
what was last written**, rather than by a flag set wherever the document
changes. A flag has to be maintained at every such site, and the cost of
missing one is telling a designer their work is safe when it is not — which is
the one direction this must not be wrong in. A colour edit does not go through
the parameter path at all, and a flag set there would already have missed it.

A test asserts that opening a document and letting the interface settle does
*not* count as a change, since a prompt that always fires is a prompt people
learn to dismiss.

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
