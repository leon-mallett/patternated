# Decision log

One entry per non-obvious decision. What was decided, what was rejected, and
what would reopen it.

---

## Quantising the lattice rather than the offsets

**Decided.** The tile's rounded pixel size is the repeat. Each lattice vector is
expressed as a fraction of the tile rectangle and that fraction applied to the
integer tile, rather than each vector being rounded to pixels on its own.
Implemented as `PixelLattice` in `pattern-render`.

**Why.** Rasterising quantises the repeat whether or not anyone decides how. A
240 mm tile at 150 DPI is 1417.32 px. Round the tile to 1417 px but keep
offsetting neighbours by 1417.32 px and every repeat drifts by a third of a
pixel — invisible in a preview, and out by a visible margin some metres down a
roll. Something has to absorb the fraction, and the tile is the only thing that
can, because the tile is what the bureau receives.

**Rejected.** Rounding each lattice vector independently, which breaks the
identity between cell area and tile area and lets gaps open at some
resolutions. Also rejected: constraining tile sizes to whole pixels at some
nominal DPI, which would make physical repeat size depend on output resolution
and contradicts the first invariant.

**Cost.** A half-drop's stagger can sit half a pixel from its ideal when the
tile height rounds to an odd number. The tiling stays exact; only the
aesthetic stagger moves, by a distance no fabric records.

**Would reopen if.** Vector export lands at M5 and the same question has to be
answered again without a pixel grid to answer it with — there the answer is
probably that it does not arise, since SVG has no raster quantisation.

Governs the tiling in [Features](./features.md#seamless-tiling).

---

## Rasterising each placement once and blitting at whole-pixel offsets

**Decided.** A placement is scaled and rotated into its own sprite, then copied
to each position the lattice requires. Positions quantise to whole pixels.

**Why.** It is what makes the two field renders agree exactly, which is what the
seamlessness test rests on. If each copy were resampled at its own sub-pixel
position, two copies of one motif would differ slightly and the difference
would sit precisely at the tile join, which is the one place it must not.

**Cost.** 0.17 mm of positional quantisation at 150 DPI, and less at print
resolutions. Accepted without much hesitation: it is far below the tolerance of
any fabric printing process.

**Would reopen if.** A preview renderer moves to the GPU at M4 and cannot cheaply
reproduce the same quantisation. The preview disagreeing slightly with the
export is tolerable; the export disagreeing with itself is not.

---

## Carrying palette slots before anything can resolve against them

**Decided.** `Placement` carries a `palette_slot` and `Pattern` carries a
`Palette` from the first commit, though M1's raster motifs render their own
colours and ignore the slot.

**Why.** The spec is explicit that retrofitting this is painful, and it is
right: every stored pattern would need migrating, and the migration would have
to invent slot assignments for artwork already approved. Carrying an unused
field costs a byte.

**Honest about what this is.** The machinery is declared, not working. Slots
resolve to colours at M5 and motifs start carrying them at M3. Validation
already rejects a slot past the end of the palette, so a document cannot record
something that will not resolve later.

**Would reopen if.** Ingest at M3 finds that real illustrations do not quantise
to a small slot set without unacceptable loss — in which case the raster path
stays literal-coloured and slots apply only to vectorised motifs, which is a
narrowing rather than a reversal.

Governs [colourways](./features.md#colourways).

---

## Removing `Pattern::draw_operations` rather than leaving it unused

**Decided.** The core briefly exposed a function returning every placement at
every lattice neighbour offset, in millimetres. It was removed once the renderer
turned out not to be able to use it.

**Why.** The renderer must work in the quantised pixel lattice, so it
re-derives the offsets in integers. That left a public function in the core
that looked like the tiling algorithm, was tested as though it were, and was
called by nothing. A later reader would reasonably have trusted it. The project's
own scope rule — that partial implementations are worse than none because they
get forgotten and then trusted — applies to leftovers as much as to things built
early.

**Would reopen if.** M6's tracking detection or an SVG renderer needs the
offsets in millimetres, at which point three lines come back with a caller
attached.

---

## A separate `pattern-cli` crate

**Decided.** The CLI is its own crate rather than a binary target inside
`pattern-io`.

**Why.** `pattern-io` is the colour-management and export crate and does not
exist yet; putting M1's command line there would have pulled `lcms2` and TIFF
handling into a milestone that needs neither. `pattern-cli` is the app layer
until the Tauri shell arrives, which also gives `anyhow` somewhere to live
without it leaking into a library.

**Would reopen if.** The Tauri shell at M4 makes the CLI redundant. More likely
it does not — a command line is how the engine gets scripted for a range, and
how it gets tested without a UI.

---

## Testing with a motif drawn in code

**Decided.** The integration tests draw their own cherry rather than loading an
illustration, and the example ships a different placeholder motif with the
script that drew it.

**Why.** Three reasons pulling the same way: golden images can be regenerated on
any machine without hunting for an original; the test suite does not break when
artwork is redrawn; and no binary asset accumulates in history. The test motif
is drawn deliberately lopsided, because a motif symmetric about its centre hides
a whole class of tiling bug — a mirrored or half-shifted copy looks identical to
a correct one.

**Cost.** Nothing in M1 touches a real illustration, so the first contact with
real alpha edges and real colour is at M3. Recorded as a known gap in
[Test strategy](./test-strategy.md#what-is-deliberately-not-tested).

---

## Measuring the toroidal distance on the lattice, not the tile rectangle

**Decided.** `lattice_distance_sq` reduces the separation between two points
into one lattice cell and minimises over the nine neighbours — the same nine
the tiling uses.

**Why.** The spec gives the toroidal distance as wrapping each axis by the
tile's width and height. That is the tiling invariant's mistake in a second
place, and it does more damage here. On a 240 × 180 mm half-drop, motifs at
(10, 10) and (230, 95) are 87 mm apart under the rectangle formula and 21 mm
apart in the printed pattern, because the right-hand neighbour arrives dropped
by half the tile height. A scatter sampled that way accepts the pair as well
separated and prints them overlapping — and the collision lands exactly where a
half-drop was chosen to prevent one.

**Rejected.** Following the spec as written. Recorded here rather than silently
diverging, because the spec is otherwise the authority and a future reader
comparing the two should find the disagreement explained.

**Cost.** Nine distance evaluations instead of one, on a brute-force scan.
Immaterial at the counts a textile repeat uses.

**Would reopen if.** Never, for correctness. The implementation would change if
scatter needed a spatial index — see below.

Governs [scatter placement](./features.md#scatter-placement).

---

## No transcendental arithmetic in placement generation

**Decided.** Scatter samples its annulus by rejection in the bounding square
rather than from an angle and a radius, keeps distances squared, and derives
floats from generator bits with code in this repository rather than a
distribution crate.

**Why.** Reproducibility is a stated invariant and `f32` transcendentals do not
support it: `sin_cos` carries no bit-exactness guarantee across platforms or
libm versions. Avoiding it entirely means placement uses only multiply,
subtract, compare and division, which IEEE-754 pins down exactly, so a seed
gives the same placements anywhere. Writing the bit-to-float conversion locally
closes the other route by which the same seed could stop meaning the same thing:
a dependency upgrade changing its sampling.

**Cost.** About 41% of darts are rejected. Irrelevant — the sampler runs in
milliseconds.

**Enforced.** A test greps the module for trigonometric calls and fails if one
appears, because this property is invisible in the output and would otherwise be
undone by a plausible-looking refactor.

**Limit.** Placement only. Rendering still rotates through `sin_cos`, and M2
makes rotation the common case. The debt is narrowed, not paid.

---

## Brute-force neighbour search in the scatter

**Decided.** Each candidate is checked against every accepted sample, with no
spatial index.

**Why.** Bridson's algorithm is usually written with a grid of cell size
`r/√2` indexing the tile rectangle. On an oblique lattice the rectangle is the
wrong neighbourhood — two samples adjacent in the printed pattern can sit in
opposite corners of it — so a correct grid would have to be built in lattice
coordinates, where the metric is sheared and the cell bounds stop being obvious.
That is a subtle thing to get right, and getting it wrong reintroduces exactly
the bug the lattice distance exists to prevent, in a place that is harder to
see.

At tens to a few hundred motifs the scan costs milliseconds.

**Would reopen if.** M4's live preview needs faster reseeding at high density.
The fix then is a lattice-coordinate grid with tests that compare it against
this brute-force version, which is the sort of thing worth keeping around for
that purpose.

---

## Correcting two empirical constants after measuring them

**Decided.** The packing efficiency the density solver uses is 0.53, measured;
the maximality bound the gap test asserts is 1.25 × the separation radius,
measured.

**Why this is in the decision log.** Both were originally written from memory —
0.69 and 1.0 — and both were wrong, and one of them was wrong in a way that had
already slipped past a test. At 0.69 the density control delivered about 77% of
the coverage it promised, and the coverage test's tolerance was loose enough to
pass anyway. The lesson is not about the numbers: **a constant nobody measured
is a guess, and a tolerance wide enough to accommodate a guess will hide it.**

The measurements are recorded beside the constants, and a test re-measures the
packing figure on every run so a sampler change fails loudly rather than
degrading a control nobody is watching.

**Would reopen if.** The sampler changes at all — in which case the test will
say so first.

---

## One palette for the library, every motif quantised onto it

**Decided.** A library holds a single palette. Every motif is quantised onto it
at ingest and stores slot indices, never colours.

**Why.** It is what makes a colourway a single swap across a whole range rather
than a per-motif negotiation, and textile ranges sell in colourways. The spec
asks for it on the further grounds that much of what reads as style mismatch is
really colour mismatch — which matters for Dreamwoven later, but costs nothing
to have now.

**Rejected.** Each motif keeping its own extracted palette, which preserves
every motif faithfully and then leaves a pattern mixing three motifs with three
palettes and no single swap that recolours it. Also rejected: supporting both,
which would mean two code paths through render and two meanings for
`palette_slot` on a milestone where nothing has met real artwork.

**Cost, and how it is surfaced.** A motif whose colours are not in the palette
still quantises onto it, and nothing about the resulting file says it went
wrong. So ingest measures the fit and reports it, and
`QuantisationReport::is_poor_fit` is how anyone finds out.

**Would reopen if.** Real Peel Club artwork turns out not to quantise onto any
single palette without unacceptable loss — in which case the likely answer is
more slots rather than more palettes.

Governs [colourways](./features.md#colourways).

---

## Quantising against the design palette, not the colourway

**Decided.** A document may carry a `design_palette` separate from the palette
it renders with. Loose files are quantised against the first and resolved
through the second.

**Why.** Found by the poor-fit warning firing on the first colourway ever
rendered, which is the best argument for having built it. Quantisation decides
which *slot* a pixel belongs to, so it has to run against the palette the
artwork was drawn for. Running it against the colourway asks which of an
unrelated set of colours the artwork is nearest to — a red cherry measured
against apricot and mint — which is a different question and a meaningless one.

A library never had this problem: it holds one palette, quantises once at
ingest, and every colourway is a swap at render. `design_palette` is how a
document without a library gets the same behaviour, and the two palettes are
required to be the same length so slots line up.

**Would reopen if.** Nothing obvious. It is a small field that makes the two
routes behave identically, which is the point.

---

## Storing the indexed raster outside its metadata

**Decided.** A stored motif is `motif.json` for metadata and `indexed.bin` for
the slot and alpha channels, two bytes per pixel.

**Why.** Serialising the channels as JSON numbers produced a 3.3 MB file for
352 KB of data. A 2000 × 2000 illustration would run to tens of megabytes per
version, versions are immutable and accumulate, and the portfolio has already
been bitten by a repository whose history became the problem before its working
tree did.

**Cost.** The authoritative data is no longer human-readable. Accepted: the
metadata beside it is, and that is what anybody actually reads. An indexed PNG
would be both compact and inspectable, and is the obvious improvement if it
becomes worth a dependency.

**Enforced.** A test asserts the binary is exactly two bytes per pixel and the
metadata is smaller than the pixel count, so the raster cannot migrate back into
the JSON unnoticed.

---

## visioncortex rather than vtracer

**Decided.** Vectorisation uses `visioncortex` 0.9.3 directly, not the `vtracer`
wrapper around it.

**Why.** `vtracer` 0.6.5 was proposed and approved first, on the basis that it
was stable, permissively licensed and pure Rust — all true. Vetting it after
approval showed it also bundles a CLI it does not feature-gate, bringing 110
crates and four RUSTSEC advisories (`atty` unmaintained and unsound, `ansi_term`
and `adler` unmaintained) and a 2021-era image stack. `visioncortex` is the
library `vtracer` wraps and does the actual tracing: same authors, same licence,
newer stable line, 74 crates, clean `cargo audit`.

The image stack was never needed, because this pipeline takes decoded pixels
rather than files.

**Worth keeping in view.** The approval was sound on the facts available and
wrong on the facts that mattered. Vetting a dependency means reading its tree,
not its description.

---

## No background-removal model

**Decided.** Matting is a luminance threshold with a feathered edge, or trusting
the file's existing alpha. No BiRefNet, no SAM, no ONNX runtime.

**Why.** The spec observes that hand illustrations on white paper are better
served without one — a threshold is faster and more faithful — and generated
motifs are asked for on flat backgrounds precisely so that this holds. A model
would bring a large runtime dependency, a downloaded weights file, and
non-determinism, to solve a problem this project does not yet have.

**Would reopen if.** Real artwork defeats thresholding: a photographed subject,
a textured ground, or a scan with heavy shadow. It would arrive as another
`MatteMethod` variant rather than as a rewrite, which is why that type is an
enum.

---

## Removing `Placement::palette_slot` once motifs carried their own

**Decided.** The per-placement palette slot is gone. Colour comes from a
motif's per-pixel slots.

**Why.** It was carried from the first commit so colourways would need no
migration, and it did that job: the machinery it justified is what M3 built on.
Once motifs became slot-indexed it had no remaining meaning, and the renderer
never read it.

It was not harmless. The colour-count report was reading it, and so reported
**one colour for an eight-colour print** — which is the number somebody would
quote a screen printer from. A dead field is not neutral when something else
still trusts it.

**Rejected.** Keeping it as a per-placement colour override, which is a real
feature — a scatter where some cherries are gold — but one nothing has asked
for and no milestone specifies. Same reasoning as `draw_operations` in M1:
partial implementations get forgotten and then trusted.

**Would reopen if.** A per-placement recolour is genuinely wanted, at which
point it comes back with a caller and a test rather than as a field waiting for
a purpose.

---

## Reporting how much of a motif became the ground

**Decided.** `QuantisationReport` carries `ground_share` alongside the error
figures, and warns in its own words when a third or more of a motif lands on
slot 0.

**Why.** Observed, not predicted — or rather, predicted and then mis-rated. A
comment in `palette.rs` had already noted that a motif whose colour is nearest
the ground would resolve to it, and called that faithful. A yellow lemon
ingested onto cream, red and green put 98% of its pixels on the ground and came
out as a lemon-shaped piece of background. Faithful is the wrong word for that.

The existing mean-error warning did fire, at 0.34, which reads as "not a great
fit" rather than "this motif is invisible". **A failure that scores as a
mediocre one needs its own signal**, because the number is what people act on.

**Would reopen if.** A design deliberately wants motifs dropping out into the
ground, which is a real technique. The warning is advisory and nothing refuses
to proceed, so this is already survivable.

---

## Measuring the paper rather than assuming it

**Decided.** The default matte estimates paper brightness across the image and
keeps whatever is darker than its own surroundings by more than the paper's own
grain. Both the level and the margin are measured; neither is a constant.

**Why.** The fixed threshold was broken for the only case it existed for, and
measurement showed there was no number that would have worked. Photographed
watercolour paper came in at 0.895 under even light and 0.818 on the shadowed
side of a sheet, both below the 0.92 the threshold cut above, so paper survived
as motif. An elderflower at 0.917 against paper at 0.971 was removed along with
its background. One case needs a low cut, the other a high one.

The margin has the same shape of problem. Textured paper varies by about 5.5%
within itself, so a margin under that keeps grain as ink; a clean digital
background varies by under 1%, where a 5.5% margin erases a pale subject.

**Rejected, with the reasons kept in the code**, because both looked like the
obvious approach:

*Dilating the block measurements*, so paper level reaches blocks containing no
paper, over-estimates on a gradient — a shadowed corner inherits the brightness
of its sunnier neighbours and keeps itself as motif.

*Any local operator*, because none of them reaches the middle of a large motif.
A subject filling half the frame leaves interior blocks many blocks deep in ink,
which is where a wrong estimate does most damage. A global surface extrapolates
*through* the motif instead.

**Known limit.** A plane models gradual falloff, not a hard-edged cast shadow.
That case is detected rather than silently wrong — see `border_retained` — and
the honest advice is a better photograph.

**Would reopen if.** Real scans routinely carry hard shadows, in which case the
answer is a higher-order surface or a local method with the interior problem
solved some other way, not a wider margin.

Governs [motif ingest](./features.md#motif-ingest).

---

## Trimming on connected regions

**Decided.** The crop is the bounding box of connected regions above a size
floor, relative to the largest, rather than of every covered pixel.

**Why.** Matting can be right and trimming still throw the result away. A
correctly matted photograph of a pear kept 19% of its pixels and still cropped
to the full frame, because a bounding box is decided by its most extreme pixel
and paper grain scatters those into the corners: 896 of 1024 rows held some
covered pixel while only 672 held more than twenty.

**Rejected.** Requiring covered neighbours, which removes isolated speckle but
not clumped speckle. And trimming by cumulative alpha mass, which clips thin
features — a tendril carries very little mass per row and would be cut off
part-way along.

Connectedness is what actually separates the two: a stem is joined to the motif
however thin it is, and grain is joined to nothing.

**Cost.** A motif made of genuinely separate parts needs them to be comparable
in size, which the 2% floor allows; something tiny and detached is treated as
debris.

**Would reopen if.** A motif style appears with deliberate fine speckle as part
of the artwork — spatter, stippling — where the distinction stops holding.

---

## Digital printing first, screen printing left open

**Decided** 21 September 2026. Peel Club prints digitally. Screen printing is a
roadmap option rather than a requirement.

**What it changes.** No per-colour cost, so no colour-count ceiling to enforce
and no palette generator constrained by one. Per-colour separations move out of
M5, because they mean nothing for digital and building them would be a partial
implementation of an unchosen route. M5 narrows to palette management, colour
management through a real profile, and file naming.

**What it does not change.** The colour count stays on screen. It is what
decides whether a design could move to screen later, and a print quietly
resolving to thirty colours usually has a palette worth looking at whatever the
route.

**Cheap to reverse**, which is why leaving it open costs nothing: separations
are one file per palette slot, and the slot model already exists because
colourways needed it.

---

## The app needed a way to start, not just a way to open

**Decided.** The shell creates a pattern from image files: matte, derive a
palette from the artwork, scatter at a starting density.

**Why.** M4 shipped able to open a document and unable to make one, so the only
route in was hand-writing JSON — which is exactly what the milestone exists to
remove. Its stated acceptance test is that a designer produces a print without
touching a config file, and the first thing the app asked for was a config file.
That was caught by someone opening it, not by any test, which is the honest
lesson: **an empty state is a feature, and it is the one most likely to be
tested only by the person who cannot see it fresh.**

**Derived, not guessed.** The palette comes from the artwork. Everything else —
240 × 180 mm, half-drop, 30% coverage, 50 mm motifs — is a starting point meant
to be changed within seconds. A guessed *palette* is different in kind: it is
how a motif ends up quantised onto colours it does not contain, and the failure
is not obvious on screen.

**Would reopen if.** Ingesting into a library becomes the normal route, at
which point this is a shortcut for trying things rather than the main door.

---

## Preventing overlap by summing motif radii

**Decided.** The distance two motifs must keep is the greater of the density's
separation radius and the sum of their own radii. On by default, and
switchable off.

**Why.** Reported from the app: at a plausible coverage the motifs visibly
intersected. The sampler was doing exactly what it promised — a minimum distance
between *centres*, solved from a coverage target — and that promise says nothing
about edges. This was half-noticed at M2, where a test fixture was observed
overlapping and filed as a stress case rather than as a defect.

Taking the greater of the two keeps both properties: coverage still controls
spacing at densities where it can, and overlap becomes a floor once it cannot.
The cost is that a pattern asked for more coverage than fits lands short, which
is the honest outcome and is already reported.

**The consequence for the sampler**, which is the part worth knowing: motif,
rotation and scale are now drawn for each *candidate* rather than assigned after
sampling, because the distance a candidate needs depends on which motif it
turned out to be. Deciding that afterwards would mean testing one motif's
spacing and then placing another.

**Rejected.** Spacing by the bounding box, which is simpler and wrong by a lot:
a round motif fills a square box to about 0.71 of the diagonal, so every pattern
would sit 40% looser than it needs. The radius is measured from the artwork's
own alpha instead, and is rotation-invariant by construction — a bound that
changed as a motif turned would let overlaps back in at some angles.

**Carried to the grid afterwards, which it should not have needed.** The guard
shipped working for the scatter and doing nothing for grid layouts, with its
control rendered inside a panel that grid layouts hide — so it looked absent and
was absent. A control that appears general and silently covers half the cases is
the partial implementation this project's rules are about, and the milestone
rule ("do not implement ahead") does not excuse implementing *behind*: a feature
added to one code path has to be followed to the others that share its meaning.

A grid answers differently because its cells are fixed by rows and columns and
it cannot place fewer motifs. It shrinks them to fit, which is visible and
reversible, and reports that it did.

**Known limit.** A circle is still a crude model of a cloud. Two wide, flat
motifs stacked vertically are held apart by their width. True shape-aware
packing would need the alpha masks compared at every candidate position, which
is a different order of cost; the guard can be turned off in the meantime.

Governs [scatter placement](./features.md#scatter-placement).

---

## Growing the preview to a window that repeats

**Decided.** The preview asks for at least 3×3 and is enlarged to the nearest
window that is itself a repeating unit.

**Why.** Reported from the app: the preview's opposite edges did not line up.
The check was the right one — holding two edges of a swatch together is how
anybody tests a repeat — and the diagnosis was nearly right. The *tile* was
seamless, which is what the export sends and what the tiling tests prove. The
*window* was not.

A rectangle of tiles is not generally a repeating unit even when every tile in
it is perfect. A half-drop gains half a tile of drop per column, so three
columns end half a tile out. Measured: 50.6 mean channel difference across the
left and right edges of a 3-wide window, against 1.7 for a 2-wide one.

**Solved from the lattice rather than special-cased.** The condition is that
translating across the window lands on a lattice point, which falls out of the
same inverse the toroidal distance uses. A half-drop needs an even column
count, a quarter-drop a multiple of four, a brick an even row count — and a
lattice nobody anticipated still gets a window that holds together, or is told
that none is within reach.

**Rejected.** Leaving it and explaining in the interface. The preview is the
thing a designer judges the tool by, and one that fails the obvious test
teaches them not to trust it.

**A note on the test**, because the first version was wrong in an instructive
way. It compared the outermost columns of pixels — which are *adjacent*
columns in the pattern, not the same column, so it measured how fast the
artwork changes in one pixel and reported a perfectly good block repeat as
broken. The working version reuses the argument the tile tests already make,
one level up: tiling the window twice must equal rendering a field of twice the
size. **When a property is about repetition, compare two constructions of the
same thing rather than sampling near a boundary.**

Governs [the app shell](./features.md#the-app-shell).

---

## Rounding the tile to a size its lattice divides

**Decided.** A tile's pixel dimensions are rounded to a multiple the lattice
steps by: even height for a half-drop, a multiple of four for a quarter-drop,
even width for a brick, nothing for a block.

**Why.** Found by measuring an exported sheet rather than by reasoning. A
240 × 180 mm repeat at 150 DPI is 1417 × 1063 px — and 1063 is odd, so a
half-drop's step of half the tile height rounds to 532. Four columns then
accumulate 2128 against two tiles' 2126, and the sheet comes back two pixels
adrift. Butting two copies together gave a seam of 2.79 mean channel difference
against an interior step of 0.73; afterwards, 0.84.

**What was not broken.** A single tile, at any resolution. Its own neighbour
offsets are consistent with each other, which is what the seamlessness tests
check and why they passed throughout. What breaks is anything that
*accumulates* the lattice — a pre-repeated sheet, and a printer stepping the
tile themselves.

**Cost.** The repeat moves by at most a pixel: 0.17 mm on 180 mm. That is the
same trade M1 made when it decided the rounded tile is the repeat, applied one
step further, and a test pins it so it cannot grow unnoticed.

**A note on how it was found.** The unit tests, the seamlessness tests and the
golden images were all green. The defect appeared only when a written file was
opened and measured — which is an argument for testing the artefact a project
exists to produce, not only the code that produces it.

Governs [export](./features.md#export).

---

## Telling a saved print apart from a hand-edited one

**Decided.** A document records whether its placements were generated. The
loader keeps generated ones and refuses hand-placed ones sitting beside a
generator.

**Why.** Saving a scattered pattern produced a file that could not be reopened,
and it had been that way since save was written.

Two rules were each right and together wrong. Saving writes the placements the
scatter produced, so that a print reopens exactly as approved even if the
sampler changes — **a seed is a recipe, and a recipe is not a record**. And the
loader refuses a document carrying both placements and a generator, because a
hand-edited one is genuinely ambiguous and silently discarding either half would
be worse than failing.

The cases were indistinguishable in the file, so one flag now distinguishes
them. Touching any control regenerates, because somebody who moves a slider
plainly wants a new print.

**How it was found.** Writing colourway tests, which needed to save and reopen a
document. Not by the feature's own tests, which only ever saved. The saving path
and the loading path each had tests; the round trip had none.

**Would reopen if.** Never — but the shape is worth remembering: two components
that are individually correct can still be incompatible at their join, and only
a test that crosses the join will say so.

---

## Shipping no CMYK profile at all

**Decided.** The tool converts to CMYK through a profile the user supplies.
None is bundled.

**Why.** The licensing was researched and several CMYK profiles are free to
redistribute without restriction — ECI's PSO Coated, IDEAlliance's GRACoL and
SWOP. Any of them could have shipped. All of them describe an **offset litho
press**, and a digital textile press is not one.

A bundled default would make the button work, and the file it produced would be
wrong in a way nobody could see until fabric arrived. That is worse than the
button being unavailable, because the entire proposition of colour management is
that the numbers mean something. A conversion through the wrong profile is a
formula with extra steps.

The bureau has the correct profile. Asking for it is a one-line email, and the
[question list](./commercialisation.md#output-spec--decided) asks
for it already.

**Rejected.** Bundling one with a warning. Warnings are read once and the file
is produced every time.

**Would reopen if.** A textile-press profile turns up that is genuinely
representative and freely redistributable, which would make a default better
than nothing rather than worse.

---

## moxcms 0.8.1 rather than the current release

**Decided.** Colour management uses `moxcms` 0.8.1, not 0.9.1.

**Why.** 0.9.1 was six days old with 2,600 downloads; 0.8.1 has 47 million.
Reading both showed the CMYK transform path is the same, so the settled release
costs nothing. Same reasoning that picked `tauri` 2.11.5 over a release
published the day before, and it is becoming a pattern worth naming: **prefer
the version other people have already run**, unless the newer one does something
the older cannot.

**Verified before building on it.** A probe converted white to almost no ink,
black to 87% on the black plate, and red to magenta and yellow rather than
cyan — because a colour library that compiles is not the same as one that
converts correctly, and its own documentation was ambiguous about whether CMYK
was supported at all.

---

## Not shipping tracking detection, having built it twice

**Decided.** M6's tracking detection is not implemented. Two approaches were
built and measured, neither separates a corridored pattern from a clean one on
patterns this engine actually produces, and both were removed rather than
shipped behind a threshold that does not work.

**What was tried.**

*The spec's approach*: render the field to a low-resolution mask and measure how
unevenly coverage spreads along each of fifteen integer directions, scoring the
worst against the others. On synthetic masks it works cleanly — evenly spread
blobs score 1.3, a field with one corridor added scores 9.0, hard stripes score
191 to 320. On real patterns it does not. Forty-five good scatters across three
lattices, three densities and five seeds scored **1.66 to 6.57**; eight of the
same patterns with a line of motifs walked through them scored **2.57 to 6.61**.

*Measuring positions instead of pixels*: for each direction, project every motif
centre and its eight lattice copies onto the perpendicular, and find the
fullest band. Cheaper, exact, and more directly a measure of alignment. Clean
scatters scored **2.09 to 3.99**; corridored ones **2.58 to 3.49** — entirely
inside the clean range.

**Why both fail.** A repeat is not a field of independently placed motifs. A
three-by-three view is nine copies of one tile, so whatever structure that tile
has repeats coherently instead of averaging out; and with twenty to forty
motifs per tile that structure is large and varies a great deal between seeds.
The natural variation between good patterns is bigger than the signal from a
corridor. Poisson-disc sampling makes it worse, not better: it produces a
characteristic spacing, which is itself structure.

**Why it was removed rather than left behind a flag.** The spec makes this a
*blocking gate* for Dreamwoven — a pattern fails it and is regenerated with no
human in the loop. A gate built on a measurement that does not discriminate
would pass and fail prints at random, and would be trusted precisely because
nobody could see it working. The project's rule that a partial implementation is
worse than none was written for exactly this.

**What would likely work**, recorded so the next attempt does not start from
nothing: the frequency domain. Tracking is periodic structure at frequencies
the lattice does not explain, which is what a two-dimensional power spectrum
shows and what neither of these spatial measures can isolate. It needs an FFT,
which brings transcendental arithmetic — acceptable for a warning, and needing
thought before a *seed* is chosen from the result, since that would make a
chosen print platform-dependent.

**Also worth knowing:** the synthetic fixtures said it worked. Both approaches
passed every unit test written against hand-made masks. Only calibrating
against output from the real pipeline showed the distributions overlapping —
the same lesson as the export defect, arrived at from the other end.

---

## Deciding the matting method from the image, in one place

**Decided.** `motif_ingest::method_for` chooses between trusting an image's
alpha and measuring its paper. Every route that mattes calls it.

**Why.** The choice was being made in two places. Creating a pattern from images
chose correctly; deriving a palette from the same images always measured the
paper. Given artwork with a transparent background — which has no paper, so the
brightest thing in frame is the subject — the second removed the whole image and
returned a palette containing only its ground.

Both call sites were written weeks apart and neither referenced the other. **A
decision made twice is a decision that will eventually disagree with itself**,
and the disagreement surfaces at whichever call site is exercised less.

**Found by artwork rather than by a test.** Both paths had tests and both
passed, because each was tested with input of the kind it happened to handle.

---

## The background is a palette slot, not a setting

**Decided.** The app's background control edits slot 0 of the palette. There is
no separate background colour anywhere in the model.

**Why it is worth recording as a decision at all.** Asked whether the background
could be set in the app, the true answer was that it already could and nobody
would have guessed — the ground sat in an undifferentiated row of swatches with
a hover tooltip.

The fix was a label and a heading, and that is the point. Because the ground was
made slot 0 back at M1 rather than a field of its own, it recolours with a
colourway, swaps with a motif colour, exports correctly, and resolves through
the same code as everything else — for free. A separate `background: Color`
would have needed all of that written again, and would have drifted.

**The lesson is about the gap between the two**, not the model: a thing that
works and cannot be found is, to the person using it, a thing that does not
work.

---

## A mirror is a symmetry of the tile, not a lattice

**Decided.** Mirror repeats are built by reflecting placements inside the tile.
`Placement` carries `flip_x` and `flip_y`; `pattern-core::reflect` expands a
cell of placements into the whole tile; the lattice stays `Lattice::block`. No
`Lattice::mirror` constructor exists, and the roadmap entry that asked for one
was wrong about where the work lived.

**Why.** `Lattice` is a pair of translation vectors, and reflections are not
translations. Every other classic repeat — block, brick, half-drop,
quarter-drop — is the same tile slid somewhere else, which is why one type
describes all four. Writing a mirror as a lattice would have meant either
lying about what `u` and `v` mean, or giving `Lattice` a mode flag that half
its methods would have to honour and the other half quietly ignore.

Expressing it as reflected placements instead means every consumer already
handles it: export writes the tile, mockups fill a garment with it, colourways
recolour it, the seamlessness test compares two independent renders of it, and
none of them needed changing.

**Consequences accepted.** The tile is not doubled — the designed cell is half
or a quarter of the repeat the designer set, because repeat size is a decision
about the print rather than about construction. A mirror requires a block
lattice, and the combination with an oblique one is refused by
`Pattern::validate` rather than rendered into a pattern that breaks along every
axis.

**Rejected.** Reflecting at render time, with the pattern carrying a mirror
mode. It would give pixel-exact symmetry for free, but every consumer would
have to honour the flag, and the seamlessness test would lose the thing that
makes it worth having: two routes to the same field that were written
independently.

**Rejected.** Folding the reflection into `rotation_deg`. It is expressible —
a reflected rotation is a sign flip under one convention and an addition of 180
degrees under another — and that is precisely the objection. The two
conventions disagree in the cases nobody checks, and a pair of flags cannot be
wrong in that way.

Described in [Features](./features.md#mirror-repeats).

---

## Quantising positions against the tile, not against the DPI

**Decided.** A placement's pixel position is `position_mm * tile_px /
tile_size_mm`, using the tile's quantised pixel size, rather than
`position_mm * px_per_mm(dpi)`.

**Why.** `PixelLattice` had already decided that the rounded integer tile *is*
the repeat, and derived the lattice vectors from it rather than rounding them
separately. Positions were still being scaled by `k`, so they lived in a tile a
fraction of a pixel wider than the one they were drawn into — an inconsistency
with no visible consequence until a mirror arrived.

**What made it visible.** Reflect a position across a mirror axis and the two
roundings can land a whole pixel apart. Between 0.28% and 1.6% of pixels in a
mirrored tile broke the symmetry that way, with channel differences over 200
where a motif edge had moved. Symmetry is the entire point of a mirror repeat,
and a designer checks one by flipping it. After the change, every mirrored tile
measured is its own reflection **exactly**, at every resolution tried.

**Cost.** Motif *sizes* are still scaled by `k`, so position spacing and motif
size differ by whatever the tile rounding absorbed — under 0.1%. The golden
image was regenerated: of fifteen motifs, thirteen were unmoved and two moved a
single pixel, and the two renders are indistinguishable by eye.

**Would reopen if.** Vector export lands and positions have to be emitted
without a pixel grid, where the question does not arise and the millimetre
value is simply used.

---

## Overlap prevention covers a motif's own reflections

**Decided.** Under a mirror, "prevent overlapping" keeps a motif clear of its
own reflections as well as of other motifs, and separation between two motifs
is measured over the reflections as well as the translations.

**Why.** A motif astride a mirror axis intersects its own reflection, and no
pairwise test can see it: the sampler never compares a candidate with a copy
that does not exist yet. The first accepted sample was worse — it skipped the
acceptance test entirely, which is harmless without a mirror and drops a
doubled motif on an axis with one.

**Cost, and it is a real one.** Every mirror axis gets a clear band one motif
radius wide. With a motif large against its repeat that band reads as a bare
corridor, and a four-way mirror loses roughly two thirds of its cell to axis
clearance, so the coverage control under-delivers and says so. The remedies are
a smaller motif, a larger repeat, or turning overlap prevention off — which
lets motifs cross the axes and is what a designer reaching for a mirror often
wants.

**Rejected.** Making the self-reflection test independent of the overlap
control. It would fill the corridors by default, but "prevent overlapping" then
means two different things depending on which layout is selected, and a motif
intersecting its own reflection is still a motif intersecting something.

**Would reopen if.** Somebody asks for deliberate butterflies — motifs snapped
*onto* an axis so they symmetrise cleanly rather than partially overlapping.
That is the classic damask move and is a third option this control has no word
for.
