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

**Reopened the same week**, by exactly that request. See
[Motifs on a mirror line](#motifs-on-a-mirror-line-are-symmetrised-not-collided).
The control gained a word for the third option: a share of motifs mounted *on*
the lines, where the reflection meets the motif exactly. The corridor is now a
choice rather than a consequence.

---

## Motifs on a mirror line are symmetrised, not collided

**Decided.** Under a mirror, a reflection that leaves a motif exactly where it
is imposes no separation at all; a reflection that moves it must still clear
it. `ScatterParams::on_axis_share` is the proportion of candidates offered a
position snapped onto the nearer mirror line.

**Why this rule and not a flag.** "Allow motifs on the axes" as a boolean would
have had to mean "allow them *near* the axes" too, and near is the case worth
refusing: a motif straddling a line by an arbitrary amount is cut and doubled
lopsidedly, which is the butterfly nobody asked for. Exactly on the line is a
different thing — the motif is halved and its own reflection completes it,
symmetrically. Distinguishing the two by whether the reflection *moves* the
motif costs one comparison and gets the four-way case right for free: a motif
mounted on the vertical line still has to clear its top-to-bottom reflection,
because that reflection does move it.

**Only one coordinate snaps**, the one already nearer a line. Snapping both
would put every mounted motif on a corner where four reflections meet — a much
stronger and rarer ornament than a designer asking for motifs on the lines has
in mind.

**Both lines get ornaments**, the one through the middle of the tile and the
one along its edge. Mounting only on the interior line would have been simpler
— the edge one involves a coordinate of exactly `W`, which wraps to zero if
anything wraps it — but it would have left the corridor on every second line,
which is the defect this exists to remove.

**Cost.** The realised share is below the one asked for, because an axis
position is more constrained than a free one. Mounting still places more motifs
overall, since the corridor stops being wasted, and a test asserts that.

Described in [Features](./features.md#motifs-on-the-mirror-lines).

---

## Each copy of a mirrored motif draws only on its own side of the line

**Decided.** A reflected placement is clipped to its half of the tile, and an
unreflected one to the other half. Axes that are not reflected are not clipped
at all.

**Why.** Drawing a motif and its whole reflection over each other is not a
mirror. Source-over compositing is not commutative, so wherever the two overlap
the result takes its colour from whichever was painted last — and reflection
reverses that order, so the picture is not symmetrical. It cost nothing while
every motif was kept clear of the axes, because nothing overlapped; it became
the whole problem the moment a motif was allowed to sit on one.

Clipping is what a designer does by hand: draw half, mirror it. It makes every
case exact rather than only the easy one, including a motif that straddles a
line rather than sitting on it.

**The trap inside it.** An unreflected axis must be left *unbounded*, not
clipped to the tile. A repeat is periodic, so a motif near one edge legitimately
draws past the opposite one — that overhang is how a tile's edges join at all.
Clipping every copy to its own tile removed it and broke all three golden
images at once.

**Consequence.** A mirrored tile is rounded to an even number of pixels in the
reflected axis, so the line falls between two pixels rather than through one.
Odd, and a motif mounted on the line sits a pixel off its own reflection. It
costs at most one pixel of repeat, which is the trade the lattice divisor
already makes.

**Verified to fail.** Removing the clip leaves six of the seven mirror tests
green and breaks exactly the one that renders a mounted motif — which is the
right shape, and the reason that test is worth more than it looks.

---

## Pre-filtering before minification

**Decided.** Each motif carries a chain of successively halved copies, built by
integer box average; rendering picks the smallest level still at least as large
as the target and lets bilinear cover the remainder.

**Why.** Almost every render minifies, and bilinear sampling reads a 2 × 2
neighbourhood. Reducing by eight in one step therefore looks at four source
pixels in every sixty-four. A dark keyline a few pixels wide falls between
samples and disappears in places, which is why a preview looked coarser than
the artwork rather than merely smaller.

**Rejected.** Bicubic, which tiny-skia also offers. A 4 × 4 kernel is better
than 2 × 2 and still nowhere near enough at eight times; it would have made the
symptom subtler without fixing it, which is worse.

**Rejected.** Box-filtering straight to the exact target size per placement. It
is the better filter, and it costs the full source area for every placement on
every preview — a dozen megapixels per slider movement. The chain is built once
per motif and shared.

**Cost.** About a third of each motif's memory again, and the final step is
still bilinear rather than something better. Measured against a Lanczos
reference at preview scale: mean channel error 5.94 to 3.75, worst 166 to 102.

**Would reopen if.** Vector export lands, where there is no raster to filter,
or if the residual difference from a proper windowed filter ever shows on
fabric.

Described in [Features](./features.md#drawing-artwork-smaller-than-it-was-drawn).

---

## Palette size is a control, not a constant

**Decided.** How many colours a design is quantised onto is a slider, monotone
at one end and photographic at the other, twelve by default. It re-derives the
palette and re-quantises every motif.

**Why.** Six was hardcoded when the app first needed a number, and it is too
few for artwork with any shading: a pale interior finds no colour of its own
and lands on the ground slot, which is invisible until the ground changes and
then becomes a hole. But the answer is not simply "more" — screen printing
charges by the colour and digital printing does not, so the right count is a
commercial decision that differs between the two brands this engine serves.
A constant cannot express that and a default cannot either.

**Rejected.** Raising the default and leaving it fixed. It would have fixed the
reported defect and left the decision in the wrong place.

**Rejected.** A linear slider from 2 to 64. Three quarters of its travel would
sit between counts nobody chooses. The control steps through a list — 2, 3, 4,
5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64 — so the useful range gets the
travel it deserves.

**Consequence accepted.** Colourways are discarded when the slot count changes,
and named in the result. A colourway is a colour per slot; after a resize it
describes slots that no longer exist. Leaving it in place would produce a
document that fails to open, which is worse than losing it knowingly.

**Ground carried across, not re-derived.** Slot 0 is a design decision rather
than a property of the artwork, so a navy ground survives asking for more
colours.

Described in [Features](./features.md#palette-size).

---

## A motif's ground share is worth reporting on its own

**Decided.** `IndexedMotif::ground_share` — the share of a motif's ink that
resolves to the ground slot — is computed in the core and reported per motif in
the app above 8%.

**Why the existing check was not enough.** `QuantisationReport` already tracked
this and warned above a third, which catches a motif that has vanished — the
lemon that came out as cream in the shape of a lemon. It does not catch a motif
with a *hole* in it. A peeled banana put 18% of its ink on the ground, scored a
mean error of 0.05, and passed every check while its interior was the
background colour.

**The lesson is about which number describes the failure.** A mean is a poor
detector of a contiguous, focal region being wrong, and the share crossing a
"mostly gone" threshold is a different question from "is any of this
invisible". Reporting the number beats tuning the threshold, because 20% of a
motif being ground-coloured is a defect in one design and the entire point of
another — a white star on a navy ground is exactly this.

**One definition, two callers.** `quantise` asks the motif rather than counting
again, because the same number is wanted in the app where there is no report to
hand, and a share computed in two places is a share that will eventually
disagree with itself.

---

## Unsaved work is detected by comparison, not by a flag

**Decided.** `Session` keeps the document as it stood when last written or
opened, serialised, and compares against it. There is no dirty flag.

**Why.** A flag has to be set at every site that changes the document —
parameters, palette slots, colourways, colour count, layout — and the cost of
missing one is telling a designer their work is safe when it is not. That is
the one direction this must not be wrong in. Comparison cannot miss a site,
because there is nothing to remember to do at a site.

Palette editing is the case that makes the point: it does not go through the
parameter path at all, so a flag set in `write_params` would already have
missed it.

**Cost.** A serialisation of the document on every close request, and a stored
copy of its text. A pattern document is a few kilobytes; the placements make it
larger but not enough to matter beside a render.

**Rejected.** Deriving PartialEq on `Document` and keeping a clone. Equivalent,
and it spreads a derive across a dozen types in the core to serve a question
the app layer is asking.

**Consequence.** A pattern that has never been written counts as unsaved
whatever its state, because closing discards it entirely. The refusal says
which of the two it is so the interface can ask the right question.

Described in [Features](./features.md#getting-back-out).

---

## The dev server does not sit on Tauri's default port

**Decided.** Vite serves the app shell on 1421, named in both
`app/vite.config.ts` and `tauri.conf.json`.

**Why.** Every scaffolded Tauri app takes 1420, and this machine runs several
of them. Whichever starts second dies with "Port 1420 is already in use" and no
indication of which project is holding it — which is how this was found, with
`~/Projects/unpack` holding the port and pattern-engine refusing to start.

**The alternative was worse than it looks.** Freeing the port means killing
another project's dev server, and on a shared machine the process holding a
default port is very often not yours. Taking a port of our own costs nothing
and removes the question.

**Not made configurable.** The port is fixed rather than found, because
`devUrl` names it: a moving port means a blank window with no obvious cause,
which is the reason the original comment in `vite.config.ts` gives for fixing
it in the first place. That reasoning is unchanged; only the number moved.

---

## A grid's drop belongs to its cells, not to its tile

**Decided.** Grid presets are a row offset and a column offset on a **block**
lattice. `GridParams` gained `col_offset`, the vertical counterpart of
`row_offset`. No grid preset chooses an oblique lattice any more.

**Why.** Each preset used to pair an offset with a lattice — a diagonal was a
shifting grid on a half-drop. Measured over a field of tiles, that lattice was
never doing anything useful: **wherever it produced a regular field, the point
set was identical to the same grid on a block lattice**, and wherever it did
not, it broke the field along every tile edge. It could only be redundant or
harmful.

**How it showed.** A diagonal grid with five rows had a visible break running
down the middle of the preview. Every tile was drawn correctly and the tiling
was exact — the seamlessness tests were right — but a half-drop moves the
neighbouring column of tiles by half a tile, which is two and a half cells of a
five-row grid, so the grid's own rhythm arrived at the join out of step.

**The second thing it was hiding.** Brick and half-drop were both a row offset
of a half, told apart only by the lattice underneath. That is not what they
are: a brick shifts alternate rows sideways, a half-drop drops alternate
columns. The preset could not express a half-drop grid at all, and was
producing a brick when asked for one.

**Consequence.** The constraints became friendly. With the drop in the cells, a
grid closes when `rows × row_offset` and `cols × col_offset` are whole: a
diagonal needs a multiple of three rows rather than six, a gentle diagonal four
rather than sixteen. The grid grows to the nearest size that closes and the app
says which it drew, exactly as the preview window already does.

**Not applied to saved documents.** `apply_grid` uses whatever lattice the
document carries, so a file written before this keeps its own. It heals the
moment the app touches the layout, which happens on the first render after
opening.

**Measured, not derived.** The rule came from generating the point set over a
field of tiles and asking whether every motif has the same neighbourhood. The
first version of that check counted nearest neighbours and reported a plain
square grid as broken, because a grid is full of ties and "the six nearest"
picks among them arbitrarily. Comparing everything within a radius is
tie-proof.

Described in [Features](./features.md#classic-layouts).

---

## Indexing the lattice, not the tile rectangle

**Decided.** The scatter keeps accepted samples in a bucket grid built in
lattice coordinates, and tests each candidate against the buckets near it
rather than against every sample placed so far.

**Why now.** It was brute-force by an explicit decision, recorded as a debt
with the condition "worth doing only if the live preview turns out to need it".
A 1.2 m canvas of small motifs made it a hang. The condition was met.

Measured in the unoptimised build a designer actually runs, per interaction —
and the preview re-renders on every slider movement:

| motifs | before | after |
| --- | --- | --- |
| 1,081 | 4.0 s | 0.03 s |
| 4,342 | 66.6 s | 0.12 s |
| 17,375 | 1,194.7 s | — |

Twenty minutes for one drag of a slider. The release figures below are the
same work with the optimiser on; both changes landed together, and the
unoptimised column is the one that describes what was reported.

**Why it was not done sooner, and why that was right.** The objection in the
debt was real rather than neglect: the grid this algorithm is usually written
with indexes the *tile rectangle*, and on an oblique lattice the rectangle is
the wrong neighbourhood — two motifs a whole tile apart may be adjacent in the
pattern. A rectangle-indexed grid would have been fast and quietly wrong, which
is worse than slow. Indexing the lattice is what the objection was asking for.

**The property that makes it safe.** The index returns a superset and nothing
more; everything it offers still goes through the exact distance test. So the
accept and reject decisions are identical to the brute-force scan, the random
draw is therefore identical, and the placements are unchanged. The golden
image — itself a scatter — is what holds that to account, and
`the_index_never_hides_a_neighbour` tests the superset property directly
against the distance function rather than against a second copy of the index.

**Mirrors needed one more thing.** A sample threatens a candidate when *its*
reflection lands nearby, which is the same as the sample lying near a
reflection of the candidate. So every image of the candidate is looked up, not
only the candidate. An index that knew about the candidate alone would silently
stop finding those, and the symptom would be motifs overlapping on fabric.

**Bucket count is sized by expected occupancy**, not by a fixed cap. Both ends
cost: a fine grid in a sparse cell spends more on allocating empty buckets than
the scan saves, and a coarse one in a dense cell puts the quadratic back inside
each bucket. A fixed cap of 4096 buckets, tried first, made the
forty-eight-thousand-motif case six times slower.

**Verified to fail.** Rounding the search span down instead of up breaks the
superset test and five separation tests. A separate mutation — removing a
bucket of slack — changed nothing, which showed the slack was redundant rather
than that the test was weak.

Described in [Features](./features.md#finding-neighbours-without-comparing-everything).

---

## The engine is optimised in development builds

**Decided.** `[profile.dev.package]` sets `opt-level = 2` for the four engine
crates and for all dependencies. The app crate itself stays unoptimised.

**Why.** `npm run tauri dev` runs an unoptimised binary and the preview
re-renders on every slider movement, so the difference between a tool that
feels immediate and one that feels stuck is entirely this. On a 1.2 m repeat of
four thousand motifs: 3.4 seconds an interaction unoptimised, 0.29 optimised —
the same work a release build does in 0.22. A designer trying the tool in dev
was meeting the first number and reasonably calling it a hang.

**Why not the app crate too.** It is the one being edited, so its compile time
is what a rebuild costs, and it does no per-pixel work worth optimising.
Incremental rebuilds stay at 1.7 seconds for an app edit and 2.9 for an engine
edit.

**No effect on determinism.** Optimisation levels do not change `f32`
semantics in Rust — there is no fast-math — so the same seed gives the same
bytes at any `opt-level`. CI compiles at the default and would notice if that
were ever untrue.

---

## Component tests, and the versions they are pinned to

**Decided.** Vitest with Testing Library and jsdom, mocking `app/src/api.ts`.
Five dev-dependencies, exact pins, no carets: `vitest@4.1.11`,
`@testing-library/react@16.3.3`, `@testing-library/dom@10.4.2`,
`@testing-library/user-event@14.6.7`, `jsdom@30.0.1`. 83 packages with
transitives; one has an install script, `fsevents`, which is the macOS
file-watcher and is not installed on Linux at all.

**Why this tier.** The command layer beneath the interface has 45 tests and
cannot see which controls a layout shows, whether a warning appears, or whether
a confirmation gates anything. Those are the parts that had been built and
never driven.

**Why not the newest versions of two of them**, which is the part worth
recording:

- **jsdom 30.0.1 rather than 30.1.1.** The current release had been published
  the previous day, inside the 72-hour window the portfolio's rules flag for
  explicit confirmation. 30.0.1 is what Ragtag has run for two months.
- **Vitest 4.1.11 rather than 5.0.1.** Vitest 5.0.0 was three weeks old and
  nothing else in the portfolio ran it. 4.1.11 declares support for Vite 8 and
  is proven next door.

This is the lesson this project already paid for once, with `vtracer`: prefer
the version other people have actually run. Neither choice costs anything here
— both do exactly what is needed — so the only thing being bought is the
absence of being the first to find out.

**Mocked at our own seam, not the framework's.** `api.ts` is the single module
every command goes through, so a test overrides two or three exports and the
rest stay real. Mocking Tauri's internals would have meant a much larger
surface and a test that breaks when Tauri changes something we never touched.

**What this tier still cannot do** is see a layout. Every control can be
present, labelled and correctly enabled while sitting on top of its neighbour —
the playbook has a scar from exactly that. An anchor-driven test proves
behaviour; only a picture proves layout.

---

## Bands are a ground treatment, not a kind of motif

**Decided.** `Pattern` carries an optional `Bands` — two sequences of coloured
bands and a list of crossings — rendered between the ground fill and the
motifs.

**Why not a motif.** A stripe can be faked as a tall thin motif on a grid, and
that holds until two stripes cross. Where a vertical band meets a horizontal
one the result is a third colour that neither band contains, and compositing
one motif over another can only ever produce a colour one of them already had.
Two colours giving three tones is what a gingham *is*.

**Why between the ground and the motifs.** It makes "a check with flowers
tossed over it" the two things it looks like rather than a third construct.
Placements are untouched, export and mockups and colourways all work unchanged,
and the band slots join the colour count a printer charges by.

**Rejected.** A crossing matrix of every slot against every other. A check uses
one entry; the matrix would be mostly empty and maintained by hand. A list of
`(a, b) -> slot` with the horizontal band winning by default covers gingham,
windowpane and tartan, and says what it does.

**The constraint the tests found.** Bands need a block lattice. A sequence
lives in the tile and repeats with it, so on a half-drop the neighbouring
column arrives half a tile down and the stripes step at every join. The
seamlessness test caught it immediately — a million pixels of disagreement
between the two render routes, not a subtle seam — and it is refused rather
than rendered.

**The same shape, a third time.** A rectangle of tiles has to be a repeating
unit; a grid's cells have to close over the tile; a band sequence has to as
well. Anything laid out inside the tile has to agree with how the tile repeats,
and an oblique lattice is what usually disagrees. Worth checking first the next
time something new is laid out inside a tile.

Described in [Features](./features.md#stripes-checks-and-ginghams).

---

## A crossing blends, because cloth is not ink

**Decided.** Where two bands cross, the default is the **average** of the two
colours. Multiplying — what a second pass of translucent ink does — is offered
as "overprint", and "across-band wins" adds no colour at all.

**Why, and how it was found.** Crossings multiplied first, on what seemed like
solid reasoning: a crossing is ink over ink, and multiplying gives a two-colour
check its third tone. The first tartan rendered came out very nearly black,
because multiplying navy by navy is far darker than navy. In cloth, half the
threads at a crossing are each colour — so the answer is their average, and a
thread crossing itself stays itself.

**The model said nothing.** Every test passed, the slots were allocated
correctly, the sett reflected correctly, and the tiling was exact. It was
visible in the first picture and in nothing else, which is the argument for
rendering a real example before believing a colour model.

**What the correction exposed.** Under blending, a check needs **two threads
rather than a thread and a gap** — a gap has nothing to blend with, so an ink
and a gap give two tones. An ink and a pale thread give three, which is what a
gingham is. The preset writes a pale thread now.

**Rejected.** Picking one rule. All three are real behaviours and the choice
changes the colour count, which is what a screen printer charges by. A select
with three named options costs one `match`.

---

## The ground owns the tail of the palette

**Decided.** The sequence editor names colours; the engine allocates slots. The
ground's slots are always the tail of the palette, so rebuilding one is a
truncate and an append.

**Why.** The alternative is tracking indices that would have to stay in step
with every other palette operation — the colour count, colourways, slot edits.
A tail needs no bookkeeping: its length is `bands.slots_used().len()`, and
everything before it belongs to the motifs.

**The bug this invariant caught.** Crossing colours are de-duplicated so that a
thread crossing itself does not add a slot. Searching the *whole* palette for a
match let a crossing borrow a motif's slot — and since the tail is what the
next rebuild truncates, changing the ground would have taken a motif's colour
with it. The search is confined to the tail.

**Consequence handled.** A change of colour count replaces the palette
wholesale, which would strand the ground's slots. Their colours are carried
across and the sequence renumbered, so a striped document survives a resize
instead of coming back referring to slots that no longer exist.

---

## A ground is a kind, not a band sequence with options

**Decided.** `Pattern.ground` is an enum, `Ground`, tagged by `kind` in JSON.
Bands are one variant; a grown texture is another. Everything downstream —
palette resolution, colourways, colour counting, seamlessness, the lattice
check — stops knowing which it is.

**Why.** The interface a ground needs is small and was already there in bands:
a function from a point in the tile to a slot, or to nothing where the fabric
shows. Nothing else about a check is general, and nothing else has to be.

**Tagged rather than inferred.** A document that gained a second kind would
otherwise be identified by which fields it happened to carry, which is
ambiguous the moment two kinds share a field name.

**One ground, not a stack.** Nothing has asked for a texture underneath a
check. A list of grounds would be untested surface until something does.

**What this cost.** One rename across the workspace and a migration of three
committed examples. Doing it before the second generator existed rather than
after meant the second generator had somewhere to go.

---

## A grown ground, and why it tiles

**Decided.** Reaction–diffusion (Gray–Scott) as the second generator: two
notional chemicals spread across the tile and consume each other, and after a
few thousand steps the field has organised itself into spots, or a maze, or
coral. Quantised onto palette slots like everything else.

**Why this one.** It produces the thing placement cannot: pattern with no
visible grid in it. A tossed layout still repeats on a lattice and the eye
finds it; a grown field has no lattice to find, only the tile's own period.

**Why it tiles.** The field lives on a torus — every Laplacian reads its
neighbours modulo the grid, so the left column's left neighbour *is* the right
column, in the same simulation. Seamless by construction rather than fixed up
at the edges afterwards, which is the only thing that could work here: a blend
or a mirror at the seam would be visible precisely because the interior has no
straight lines to hide it in. The unit test states it as an equality — the same
seed patch, started ten cells along, must give the same field ten cells along,
bit for bit.

**Parameters, never the field.** A 192-cell grid is 36,864 floats. The document
stores the growth, the scale, the step count and the seed; the field is grown
when the ground is laid on a tile. That is also why `Ground::laid_over` exists
at all — no ground can be sampled without knowing its tile, and bands had the
same obligation without it being named.

**The published rates are for a different Laplacian.** Every write-up quotes
feed and kill rates against the bare five-point stencil. This uses the
normalised nine-point one, which is about four times smaller for the same
field, so the diffusion rate has to rise to match — and the live region of the
parameter space moves with it. At the cited rates for "worms" this model is
dead within a few hundred steps. The six named growths were found by sweeping
the space and looking at the renders, and a test walks all six and fails any
that covers almost none or almost all of the tile, which is what being outside
the live region looks like from the outside.

**The field is sampled, not classified.** `SlotField` keeps concentrations and
cuts them into levels on the way out. Reducing to a slot per cell was the first
version and it printed as a staircase: a cell is eighteen pixels at print
resolution, so every curve stepped, and raising the cell count to hide it made
the shapes smaller instead. Bilinear-then-threshold gives an edge that stays
smooth at any output resolution and gives the cell count back its only
meaning — how large the shapes are. The interpolation wraps on the torus, so
the last half-cell of the tile blends with the first cell of the next rather
than with a clamped copy of its own edge.

**A memo, not a faster simulation.** Laying a ground on a tile is a pure
function of two arguments and the most expensive thing in a render. The
renderer remembers the last one, so a preview that changes the motif density
does not grow the field again, and the interface's own look at the field —
what share each level covered — shares the same one. Texture edits are
additionally coalesced in front of the control, the way previews already were.
Band edits are not, deliberately: `mirrored` is a one-shot instruction, and
coalescing a reflect with the edit after it would cancel the pending send and
lose the reflection.

**Determinism.** The step is `+`, `-` and `*` on `f32` and nothing else — no
`exp`, no `powf`, nothing whose last bit is allowed to differ between
platforms. A pinned field checksum stands in for a golden image, and CI is
Linux while development is macOS, so the claim is checked somewhere other than
where it was made.

---

## Sampling a ground in the tile's coordinates, not the canvas's

**Decided.** The renderer takes the pixel's position within its own tile before
converting to millimetres, rather than letting the coordinate grow across a
field of tiles.

**Why.** A field of nine tiles is 720 mm across where a tile is 240, and an
`f32` carries about six decimal digits. Asking for the slot at 683.1693 mm and
at 3.1693 mm is the same question and not the same arithmetic; near a cell or
band boundary the two answers can differ.

**How it was found.** A grown ground disagreed with itself in ninety pixels at
150 DPI and in none at 96 — the seamlessness test compares two independent
routes to the same field, and they take different paths to the same
millimetre. Bands had carried the identical hazard from the start and had never
been unlucky enough to show it. Taking the remainder in *pixels* is exact,
because a field is always a whole number of tiles.

---

## A second generator that is all lattice

**Decided.** Ornament: a grid of cells, each carrying a figure, three of which
contain smaller copies of themselves. The second mathematical generator, and
deliberately the opposite pole from reaction–diffusion rather than more of it.

**Why this and not an L-system or an IFS.** An L-system's turtle needs
headings, and headings need trigonometry, whose last bit is not guaranteed
across platforms — the determinism invariant would have had to be weakened or
worked around with stored constants. An IFS attractor does not tile: wrapping a
chaos game into a torus destroys the structure that made it worth plotting. A
grid of rule-drawn cells tiles by construction, needs no transcendental beyond
`sqrt` (which IEEE 754 *does* require to be correctly rounded), and lands on
the ornamental family — strapwork, clamshell, square fractals — that a repeat
print actually uses.

**An integer hash, not a stream.** The per-cell choices come from SplitMix64
over the cell's coordinates rather than from `rand_chacha`. A stream answers
"what is the next number"; this needs "what is the number *for this cell*",
because sampling visits cells in whatever order a renderer walks pixels and a
recursive figure visits the same cell once per level. A stream would have to be
materialised into an array first. It is seeded, pure, and has no floating point
in it, so it is if anything harder to make non-deterministic than the sampler.

**What the tests found.** Two real defects, both from stating a property rather
than looking at a picture. The Weave was drawn corner to corner, which meets
the cell at its corners where a neighbour has nothing to meet it — the render
looked like a plausible maze and was full of broken stubs. And `slot_at`
divided a far-off coordinate by a cell size before reducing it into the tile,
which is the same `f32` precision trap the renderer hit the day before, in a
second place.

---

## A test that compares two render routes cannot see a broken rhythm

**Decided.** The brick parity of a clamshell ground is guarded by a unit test
on the grid, not by the seamlessness suite, and the suite's fixture says so.

**Why.** Both render routes reduce a coordinate into the tile before sampling,
so both ask the ground the same question — a ground that disagrees with its own
repeat disagrees identically down both routes and the comparison sees nothing.
That suite catches route-dependent faults: fitting, rounding, sampling,
compositing order. It cannot catch a figure whose rhythm breaks at the join.

**How this was found.** By mutation, not by reasoning: the even-row rounding was
removed and the render test still passed. Worse, the first fixture had chosen a
cell size whose row count came out even by luck, so even a test that *could*
have discriminated would not have.

**And the hazard test was vacuous too.** The first version compared the top
sliver of the tile against the bottom sliver, which differ for a correct
clamshell as much as for a broken one — it asserted something true either way.
The replacement measures where the scallops of the first and last rows sit and
requires them to be half a cell apart, which is exactly what the alternation is
for.

---

## A ground is drawn, not decided per pixel

**Decided.** Every ground is sampled sixteen times per pixel and averaged,
rather than once and taken.

**Why it was once.** Bands were the only ground for a long time, and a band's
edges are axis-aligned: one sample per pixel is not an approximation there, it
is exact. The cost only appeared when grounds gained curves.

**What it looked like.** Reported from the app as "very pixelated". The give-
away was the contrast within a single render: the motifs were smooth because
tiny-skia filters them, and the ground beneath them stepped, so the two layers
of one print looked like the work of two different tools.

**Why a fixed sample count rather than an adaptive one.** An early-out that
decided a pixel was uniform from a few probes would miss any feature finer than
a pixel, and a hairline is exactly what a designer reaches for. Measured, the
fixed cost does not dominate: a 300 DPI tile of a Truchet ground renders in
1.8 seconds against 1.8 for a gingham, so the supersampling is lost behind
motif drawing and PNG encoding.

**Why sRGB.** To match how the motif layer composites. Blending the two layers
in different spaces would trade a visible inconsistency for a subtler one.

**What the test says.** Not a golden — a property. A two-colour ground whose
pixels are only ever one of those two colours has decided each pixel rather
than drawn it; and the count of *distinct* tones tells four samples from two,
where the share of edge pixels cannot. The hazard test computes the same ground
once per pixel and asserts it produces no intermediate tone at all.

**A gap this exposed.** No golden image covers a ground of any kind — all three
are motif-only — which is why a change to every ground's edges broke no golden.
Recorded in the roadmap rather than fixed in passing.

---

## Controls that cannot act are not shown

**Decided.** A pattern with no artwork in it does not get the Layout panel —
arrangement, overlap guard, rows and columns — nor the readouts that count
motifs and coverage. A pattern with a ground does not get offered the three
oblique repeats, which the engine would refuse.

**Why.** Reported from the app: a design made entirely of a ground still led
with a panel of controls for placing motifs. A control that does nothing is
worse than a shorter panel, because it gets moved, nothing happens, and the
rest of the interface stops being believed. Offering a repeat that is then
refused by the engine is the same fault with an error message attached.

**The rule this is a case of.** It is the same one the ornament panel already
follows, where depth is shown only for the figures that recurse and a line
width only for the ones that are stroked.

**And then the panel is composed per workflow, not filtered.** Hiding what
cannot act was the first half and did not go far enough. A design made entirely
of a ground and a design of motifs strewn over one are different jobs, and a
panel that opens the same way for both asks the designer to skip past the half
that does not apply. So:

| | Ground-only | With artwork |
|---|---|---|
| | Ground | Layout |
| | Repeat | Repeat |
| | Background | Scatter · Seed · Motifs |
| | Colourways | Colours |
| | | Background |
| | | Ground |
| | | Motif colours |
| | | Colourways |

The ground leads when it *is* the design and sits under the background when it
is what the motifs are strewn on, which reads as layering in one case and as
priority in the other.

**Three things this turned up.** The palette-size control re-derives colours
from the artwork, so on a ground-only design asking for sixteen returns the two
it had and changes nothing — the most confusing thing a control can do. A
section headed "Motifs" carried the note explaining that there were none, which
is the same fault as the panel it was explaining; that note now opens the ground
panel, where it is both the first thing seen and the thing to do about it. And
"Motif colours" held the colourway derivations — shift hue, swap ground — which
have nothing to do with motifs: a tartan has a palette too. They moved to
Colourways, where what they make is what the section is about.

Both compositions are pinned by a test, because an ordering nobody asserts is
an ordering that drifts back.

---

## A relative control has to show what it is relative to

**Decided.** The per-motif weight is labelled *Share*, reads out as a
percentage of the mix, and is not shown at all when there is only one motif.
The panel says, in context, what does set the number of motifs.

**What was reported.** That the slider did nothing: a designer dropped a
motif's size, raised how often it appeared, and the number on the fabric did
not move.

**It was doing exactly what it is for.** Measured through the app layer: with
one motif, a weight of 1 and a weight of 8 both give eighteen placements, and
they always will — the weight is a share of a mix and a single motif is the
whole of it however the slider is set. With several, raising one takes from the
others and the total holds. A test now pins both halves, including that raising
every share together changes nothing.

**So the fault was the label and the framing.** "How often" reads as a
frequency, it sat beside a Size slider that does change the count, and the only
explanation was a sentence below every row. A percentage that moves when its
neighbours move says what kind of control it is without being read.

**And the count is set somewhere else, differently per layout.** On a scatter
it is Coverage — and motif size, because coverage is a share of the fabric
rather than a count, so smaller motifs means more of them. On a grid it is rows
times columns, and neither the size nor the share can move it. That is almost
certainly the case reported: on a grid, both of the controls tried are inert
for the purpose they were being tried for. The hint now says which, and changes
with the layout.

**The general rule.** A control whose effect is relative must display the
relation, not the operand. A number on its own — a weight of 6 — is
indistinguishable from an absolute one, and will be read as absolute.

---

## The export dialogue is a modal, and its button says Save as

**Decided.** Exporting opens a modal over the window instead of unfolding a
panel below the preview, and the button that writes the file is *Save as…*
rather than *Choose a file…*.

**Why a modal.** Exporting is a decision with a dozen settings and an end — a
file written. Below the preview it pushed the image out of view exactly when a
designer wanted to look at it, and read as one more panel of the same kind as
the controls beside it, which are all live edits to the document. It is not one
of those: nothing it does changes the pattern.

**Why not the platform's `<dialog>`.** It would give the backdrop, the Escape
key and a focus trap for nothing, and in the shipped WKWebView it is fully
supported. But jsdom — which the component tests run in — still does not
implement `showModal` at version 30, so a native dialogue could only be driven
in a test behind a shim faking the very thing being claimed: the tests would
pass over a dialogue that was never modal. Thirty lines of our own, all of it
exercised — Escape, focus on open, focus returned to the opener on close, and
Tab kept inside — was the better trade. Worth reconsidering when jsdom
implements it; platform focus handling is better than anybody's.

**Why the label.** *Choose a file* describes picking one of the files that
exist. This names one that does not exist yet, which is what every platform
calls Save as. The ellipsis stays because it opens a further dialogue.

---

## A drawing keeps its outlines; artwork keeps its pixels

**Decided.** SVG motifs are held as outlines and rasterised per render at the
resolution that render needs. Raster artwork goes on being sampled once, at
import, as it always has.

**Why not rasterise at import.** It was the smaller change by a long way, and
it throws away the only reason the file was vector. Measured: a disc drawn from
outlines has a soft rim over 0.0037 of its area at 600 DPI; the same disc
sampled at import to 512 px and scaled up has 0.0185 — five times as much, and
that is what a designer sees as a soft, slightly wrong edge on a large print.

**How it fits without a second motif type downstream.** A vector motif is a
*source*: the bank rasterises it into an ordinary `IndexedMotif` at the size
each render wants, so the sprite cache, the mip chain, the overlap radius and
the colourway all go on working on the one representation they always had. The
bank also keeps one reference rasterisation for the measurements that are only
ever asked in fractions — the coverage radius, the slots used — and that
reference is never printed.

**Why not resvg.** It was approved, and then not needed. Rendering an SVG
faithfully is a different job from finding the shapes a press can put ink into:
once every fill is quantised to a palette slot, what is left is geometry, which
`usvg` parses and `tiny-skia` — already ours — fills. Taking the smaller
dependency was the better trade, and it made the refusals explicit rather than
silently rendering things that cannot be printed.

**The `text` feature stays off, on determinism grounds.** It resolves type
through the fonts installed on the machine doing the work. The same file would
rasterise differently elsewhere, which would quietly retire the engine's
central promise. Text must be outlined before import, and the app says so.

**The slot of a pixel two shapes share** is the shape covering most of it, and
its coverage is what both laid down. That keeps a flat interior exactly flat —
what the split channels exist for — while an edge between two colours stays
soft. Ties go to the later shape, because shapes arrive in painter's order; a
test caught that being strictly-greater, which rendered a drawing with its
layers reversed.

---

## The capability manifest is checked against the interface that uses it

**Decided.** A test reads every dialogue API the front end imports and asserts
the Tauri capability file grants each one.

**The defect it was written for.** `confirm` arrived with the Close button, the
capability file had not been touched since the shell was built, and closing a
document with unsaved work failed with a permission error. It had been broken
since the day the button was added.

**Why nothing caught it.** The component tests mock the dialog plugin, so they
drive dialogues the shipped app is not permitted to open and never notice; the
Rust tests never reach the front end. The manifest sits between the two layers
and is answerable to neither. It is also the file nobody remembers to edit,
because adding an import is the visible act and granting a permission is not.

**And the failure hid where it mattered.** Close works perfectly with nothing
unsaved — the confirmation is only reached when there is something to lose, so
the path that fails is exactly the path a designer meets at the worst moment.

**The general shape.** A declared permission, capability or manifest that some
*other* layer must agree with is a seam no test on either side covers. The
cheap guard is to read both and compare them, which is a dozen lines and runs
with everything else.

---

## The preset menu names what the sequence *is*, not what was last clicked

**Decided.** The ground's Pattern menu works out which preset the current
sequence matches, by comparing it, rather than remembering a choice or reading
"has any bands" as "has been edited".

**What was reported.** Choosing Pinstripe showed *Custom* immediately, and so
did every preset but Plain — an intentional choice looked as though it had not
taken.

**Why comparison rather than a remembered choice.** The same reasoning as
detecting unsaved work by comparison: a flag has to be cleared by every edit
that invalidates it and is wrong the moment one is forgotten, and it knows
nothing about a document that was *opened* rather than built. A comparison
names a reopened stripe correctly, and is right by construction.

**What counts as the same.** The widths, the direction, whether each band is an
ink or a gap, and — only where both directions paint — the crossing rule.
Colours are deliberately excluded: a preset describes an arrangement, and
recolouring a stripe leaves it a stripe. Whether a band is a gap *is* compared,
because that is arrangement — a gingham is two threads where a stripe is a
thread and a gap.

**A tartan needed care.** Its preset names four bands and asks for a
reflection; what the engine stores is the expanded six. The comparison expands
the preset the same way before matching.

**And it found a lying mock.** The component tests' fake engine echoed the
sequence back unreflected, so a tartan looked like four bands where the real
engine returns six. No test had noticed, because none had asked. A mock that
answers differently from the thing it stands for turns a component test into a
test of the mock; it reflects now.

---

## Tracking detection, at the third attempt, as a ranking and not a gate

**Decided.** The engine measures how strongly a scatter's near neighbours line
up, and the app says "worth a look" above a calibrated score. It is **not** the
blocking gate the spec asks for, and is not presented as one.

**What it measures.** Every pair of motifs closer than 1.6 times the mean
spacing contributes the shortest displacement between them *on the lattice*.
What is asked of those directions is the strength of the **fourth harmonic** of
their angular distribution.

**Why the fourth and not the second.** The second harmonic — a single preferred
axis — was written first and is blind here. A field of rivers has both
along-river and across-river neighbours, at right angles, and doubling the
angle maps those onto opposite points where they cancel. It scored a perfect
square grid 1.4, barely above a clean scatter. Before that, a histogram's
tallest bin was tried and measured mostly counting noise: a few hundred pairs
over thirty-six bins is five to a bin, and Poisson scatter alone puts the peak
two or three times the mean.

**What it can and cannot do.** Calibrated on 600 scatters — three lattices,
five densities, forty seeds — against the same patterns pulled toward parallel
lines by a known amount:

| Score above | Clean flagged | Blatant caught | At the level a designer rejects |
| --- | --- | --- | --- |
| 0.8 | 6.2% | 95% | 66% |
| 1.2 | 0.8% | 89% | 38% |
| 2.0 | 0.0% | 66% | 4% |

The rejection level was established by rendering the corpus and looking: at 30%
pull the banding is faint, at 50% there are clear rivers with empty lanes, at
80% it is blatant.

**The experiment that decided it, and that neither previous attempt ran.**
Scoring a synthetic corridor proves only that a measure notices the
construction that made it. So three hundred *unmodified* scatters were scored
and the extremes rendered. The highest — 0.95, 0.95, 0.80 — visibly track, with
lanes running through them. The lowest look evenly strewn. The measure's
ranking agrees with the eye on output nobody interfered with, which is the
claim the previous two could never make.

**Why a ranking rather than a gate.** At the level where a person rejects a
print it catches roughly two in three at best, and it is blind to faint
tracking. An unattended gate built on it would pass most of the borderline
cases, and the project's own rule — that a partial implementation is worse than
none — applies to a gate that is trusted precisely because nobody can see it
working. As a warning it is honest: a high score is reliably worth looking at.

**The threshold is set low on purpose.** 0.8 flags about one scatter in
sixteen. A threshold chosen for a near-zero false-alarm rate would have missed
all three of the worst real patterns, which scored 0.95, 0.95 and 0.80.

**Determinism, without the frequency domain.** The recorded next idea was a 2D
power spectrum, which would have brought an FFT and its transcendentals, and a
note that a score driving a *seed choice* must not be platform-dependent. This
reaches the same question with square roots and multiplications: the doubled
and quadrupled angles come from the double-angle identities on the
displacement's own components, so there is no `atan2` and the score is as
reproducible as the placements it measures.

**A known weakness, left visible.** What the measure reports depends on which
families of neighbour fall inside the reach. A 5×5 grid in a 240×180 tile puts
its diagonals inside it as well as its rows and columns, and those partly
cancel, so it scores 0.78 — below the warning threshold. The grid test is
comparative rather than absolute, and says why.

**What would make it a gate.** Carrying the *radial* dimension as well as the
angular one — a corridor is regular spacing along a direction, not only a
direction — and a labelled corpus of real patterns rather than synthetic pulls.
Neither is needed for a warning.

---

## Two vector types, kept apart on purpose

**Decided.** The raster tracer's output is `TracedOutlines`; artwork that
arrived as outlines is `pattern_core::VectorMotif`. They were briefly both
called `VectorMotif`, in the same crate.

**Merging them was considered and would have been wrong.** They look alike —
slotted outlines — and are opposites in use. One is a *tracing of a bitmap*,
produced for vector export and kept alongside the raster, never instead of it,
because textured artwork traces badly and the spec says those ship as raster.
The other *is* the artwork, held so the renderer can draw it at whatever
resolution a render needs. A single type would have invited drawing the tracing
instead of the bitmap, which is worse output for exactly the artwork the
tracing represents least well.

**So the fix was a name and a paragraph**, not a refactor. Recorded because the
merge is the obvious move and the reason not to make it is not visible from the
types.

---

## A repeat is cut for a garment, from a recorded reference

**Decided.** A document records the garment its measurements are for. Changing
it scales the repeat, the lattice, the placements, the ground's lengths and
every motif's natural size by the ratio of the two chest widths.

**Chest rather than length or shoulder.** A repeat reads *across* a body: it is
the dimension the eye counts repeats along, and the one the garment preview
already reports.

**Measured against counted.** Everything in millimetres moves. A texture's cell
count is how many fit across the tile, so it follows the tile already and
doubling it would halve the pattern instead of keeping it. Getting that
distinction wrong is silent — the render still looks like a texture, just the
wrong one.

**From the reference, not compounded.** The alternative — multiply whatever the
sizes currently are — drifts a little on every change, and a control that loses
something each time you touch it is one a designer learns not to touch. A test
walks four sizes and back and requires the design to return.

**An untold document is told, not resized.** There is no factor without two
sizes. Guessing that a document written before this existed meant "adult" would
silently resize somebody's work the first time they opened the control.

**Separate from the garment preview**, which shows a print on a body without
changing it. One is a lens and the other is a decision, so they are different
controls in different places — the preview in the header, the cut beside the
millimetres it changes.

---

## A library returns what it wants to say, it does not print it

**Decided.** `pattern-doc` no longer writes warnings to stderr. Ingest collects
them as `Note`s and the caller decides what to do with them; the desktop app
shows them above the preview, dismissible.

**What this was actually costing.** These are the two faults most likely to
reach a printer unnoticed — most of a motif dissolving into the fabric colour,
and a shadow kept as ink — and the desktop app could not see either. A library
that writes to stderr has decided its caller is a terminal. Ours mostly is not.

**Two methods rather than one changed signature.** `load_motifs` keeps its
shape and delegates; `load_motifs_noting` takes a sink. Of fifteen callers,
thirteen genuinely have nobody to tell — a render, a test, an export — and
making all of them thread an empty vector through would be ceremony for the two
that do.

**Dismissible on purpose.** These are notes about artwork that *did* come in,
not refusals. A warning that cannot be put away is one that gets ignored,
including on the day it matters.

**Not a logging framework.** A facade would have satisfied the same readiness
criterion and left the app no better off: a subscriber writing to a terminal
nobody is watching is the same failure with more machinery. Returned data is
what an interface can show, a test can assert on, and a caller can ignore
deliberately.

---

## A garment has a cut, and its width is not a chest

**Decided.** `Garment` carries an explicit `Cut` — panel, top, legs,
dungarees, hat, tote, pyjamas — and its width is `across_mm`.

**Why a cut.** The shape used to be inferred from whether `sleeve_mm` was
greater than one, which worked for exactly the two shapes there were and could
not express a third. A hat has no sleeve length to be zero.

**Why not `chest_mm`.** It is the number a repeat is scaled against, so it has
to mean the same thing for every shape, and a hat has no chest. `across_mm` is
named for what it is *for* — the width the print reads across — rather than for
one garment's anatomy.

**Pyjamas are one shape, not two garments.** The outline is two contours in one
path, so they are filled and stroked together and the same cloth runs across
both halves. That is the difference between a pair of pyjamas and a top and
trousers that happen to match, and it costs nothing: the renderer already
filled whatever path it was given.

**Three faults a picture found and no test would have.** A long sleeve reached
in proportion to its length and gave a T-pose three times as wide as it was
tall. A pair of dungarees lost the tops of its straps off the top of the frame.
A hat's brim overshot the hem by ten millimetres. Every number was right in all
three; each was obvious the moment the shape was drawn.

**So two tests now say what the pictures said.** Every silhouette must fit the
frame its own extent describes — the extent decides the canvas, so anything
past it is clipped silently — and no two cuts may cover the same share of their
frame. The share is measured by *drawing* the silhouette, because the first
attempt compared bounding boxes and a pair of legs and a pair of dungarees both
fill theirs exactly.

**And the reach is worked out in one place.** The extent and the outline each
computed the sleeve separately and could disagree; a silhouette drawn wider
than the frame it was measured for is clipped without anything failing.

---

## An error reaches the interface with its cause attached

**Decided.** The app formats failures with `{e:#}`, `anyhow`'s alternate
`Display`, which prints the whole chain.

**What it cost to get wrong.** Loading an SVG of gradients reported *"could not
start a pattern from those images: reading &lt;path&gt;"* and stopped there. The
sentence that said it was a gradient, and what to do about it, was the
innermost link and plain `Display` prints only the outermost. The engine had
the right answer, in words chosen for a designer, and threw it away one layer
from the screen.

**Why it hid.** Every layer added context correctly and the chain was built
properly. Nothing was missing until the last format string, and a single `#`
was the whole difference between a message somebody can act on and one that
tells them a file exists. The command line was never affected — `anyhow`'s
`Termination` prints the chain — so the fault lived only where the errors were
put into words by hand.

**Not a logging decision.** These are what the app *shows*, and the more
carefully an error is worded further down, the more there is to lose.

---

## A drawing that cannot be outlines is drawn, not refused

**Decided.** Where an SVG carries something no palette slot can stand for, it
is rendered with `resvg` and quantised like any other artwork.

**Reported from the app.** A logo whose only path is filled with a linear
gradient. The engine refused it correctly and told the designer to export a PNG
and come back, which is work the tool can do itself.

**Reversing an earlier decision, with the reason.** `resvg` was approved and
then declined on the grounds that once every fill is a palette slot, what is
left is geometry. That holds for reading outlines and does not cover this: a
gradient is not geometry, and the fallback is *rendering* SVG, which is what
`resvg` is for. Two new crates — `resvg` and `rgb` — because `usvg` was already
carrying the rest.

**`text` stays off**, so live text is still refused rather than rendered. Type
resolved through installed fonts would rasterise differently on another
machine, and that is the one promise this engine cannot bend. An embedded
bitmap is refused for a duller reason: without the raster-image features there
is no decoder, so rendering would drop it too.

**Which is the rule for when to fall back.** Only where rendering would
actually recover what the outlines could not carry. Falling back for live text
would trade the whole drawing's sharpness and still show no text.

**The subtler half.** It fires when a drawing *mostly* reads as outlines too. A
gradient among flat shapes reads perfectly well and yields a motif with a shape
missing — the wrong picture rather than a softer one, and only one of those is
visible on screen. Both halves are mutation-checked; the first version of the
test only covered the drawing that failed outright, and removing the partial
case's guard broke nothing.

**What it costs.** Resolution independence, for that motif, stated in a note
the app shows. Rendered at 1024 px on the longest edge, which is where the
trade is made and is named as a constant so it is obvious where.

---

## The preview counts tiles; the export writes what was asked for

**Reported as a curiosity.** An exported file looked like a much smaller
viewport than the preview — the same motifs, three or four times the size,
far fewer of them.

**Nothing was wrong.** The preview shows a field of tiles, four by three here,
so that tracking can be judged; the export's default writes **one tile**,
because that is what a printer repeats. Checked rather than assumed: at 150 DPI
the same document renders 1063 × 798 px as a tile and 4252 × 2394 as a four-by-
three field — exactly four and exactly three times, so a motif is the same
number of pixels and the same millimetres in both.

**But the interface said neither thing clearly.** The header read
*"untitled · 4×3 repeat"*, which parses as though the design were a 4×3 repeat
rather than as a count of what is on screen — and the repeat's real size,
240 × 180 mm, was in a readout below. So an export of one tile looked like a
crop. It says *"showing 4×3 tiles"* now.

**And the export says what a tile will look like.** Choosing *One tile* now
carries a line: that it is seamless on all four sides, that the printer lays it
edge to edge, and that it will look far emptier than the preview — same size on
the cloth, fewer of them in the picture. The control's own wording was already
accurate; what was missing was the comparison the eye was about to make.
