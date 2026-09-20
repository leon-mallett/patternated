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

Governs [colourways](./roadmap.md#m5--colourways-and-export).

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

Governs [colourways](./features.md#colourways-by-palette-slot).

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
