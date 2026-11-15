# Test strategy

Tiling correctness is the thing that matters. Everything else here is ordinary.

The failure this suite exists to prevent has a specific shape: it looks correct
in every preview, passes any test written against a single tile, and is
discovered when fabric comes back wrong. Tests that can only confirm the
renderer agrees with itself are no use against it.

## The test that earns its keep

`crates/pattern-render/tests/seamlessness.rs` renders the same field two
independent ways and requires them identical:

- **By tiling**: render one tile, then lay copies at lattice offsets, which is
  what a printer does with the file.
- **Directly**: draw every placement straight from the pattern definition over
  the whole field, with no tile in between, which is what the repeat means.

Any mishandling of a motif crossing a tile edge makes the two disagree, and the
disagreement sits exactly where the seam would have been.

**This test was verified to fail.** The hardcoded-dimensions bug, where neighbour
offsets were taken from the tile rectangle instead of the lattice vectors, was
deliberately injected on 2026-09-20. The half-drop test failed and the block
test still passed, which is the whole shape of the hazard: for a block repeat
the two definitions coincide, so the bug survives any test suite that only
exercises block repeats.

A seamlessness test nobody has watched fail is a seamlessness test nobody should
trust. If this one is changed, break it again on purpose.

The same exercise on M2's toroidal distance found a weaker test than it looked.
Substituting a tile-rectangle distance left the scatter's boundary test green,
because that test measured with the engine's own distance function and so only
ever proved the scatter agreed with itself. It now computes distance a second
way, naively and inline, and four tests catch the substitution where two did.
**A test that calls the function under test to decide what the answer should be
is not evidence.**

Seamlessness is re-asserted at 72, 96, 150 and 300 DPI, because quantisation
happens at render time and cannot be proved once and assumed after. 150 DPI
gives an odd tile height for the test repeat, which is the awkward case.

## The mirror's equivalent

`crates/pattern-render/tests/mirror.rs` asserts the property a mirror repeat
exists for: **the rendered tile is pixel-for-pixel its own reflection**, with no
tolerance at all. That is a stronger claim than it sounds and it took two
things to earn.

It is asserted on pixels rather than on placements because the placements can
be perfectly symmetric while the picture is not: a sprite of odd width centred
on a pixel sits half a pixel to one side of it, and reflecting the centre
without reflecting that bias puts the copy a whole pixel out.

**Both halves of the argument were verified to fail.** Rounding a reflected
sprite's anchor the same way as an unreflected one breaks four of the seven
tests; removing the guard that keeps a motif clear of its own reflection breaks
three, and breaks two `pattern-core` tests that measure it directly rather than
through the picture. The direct ones matter: the symmetry test catches a
missing guard only *incidentally*, through compositing order, and a test that
catches a bug by accident will stop catching it by accident.

`an_ordinary_scatter_is_not_symmetric` is kept alongside, so a measurement that
returned zero for any input could not leave the whole file passing.

Mounting motifs *on* the mirror lines added a third failure to verify: with the
per-copy clip removed, six of the seven tests stay green and only
`motifs_on_the_mirror_lines_keep_the_tile_exactly_symmetric` fails. One test
carrying a defect on its own is a reason to trust it more, not less, but only
because it has been watched fail. `asking_for_motifs_on_the_lines_actually_puts_some_there`
sits beside it because every symmetry assertion in the file would also pass if
nothing were ever mounted.

The residual asymmetry was measured rather than tolerated. Before positions
were quantised against the tile it ran between 0.28% and 1.6% of pixels with
channel differences over 200; that measurement is what made the quantisation
change worth [deciding](./decisions.md#quantising-positions-against-the-tile-not-against-the-dpi)
rather than a tolerance worth widening.

## Asserting a principle instead of a tolerance

`crates/pattern-render/tests/minification.rs` is the clearest example of a
shape worth copying. The property under test is that artwork survives being
drawn smaller than it was drawn, and the obvious assertion, that "the error is
below X", would have to pick an X, which is a number nobody can defend.

What it asserts instead is the direction: **reducing correctly means every
source pixel contributes, so more detailed artwork can only render better.**
Sampling a fixed neighbourhood means more detailed artwork renders worse,
because the share of it that is read shrinks. Worst-pixel error against a
box-filtered reference, at 256, 512 and 1024 px sources: 92, 78, 77 with
pre-filtering; 92, 122, 145 without. No tolerance covers both, and the
assertion needs no constant.

The reference is a box filter worked out in the test rather than asked of the
renderer, on the rule from the tracking work above: a test that calls the
function under test to decide what the answer should be is not evidence.

**The shared fixture could not have caught this**, which is the other half of
the lesson. Its cherry is drawn at 128 px, so nothing is ever reduced by more
than about two and the defect never appears. That is the synthetic-fixture trap
the log already records, arriving from the opposite direction: not a fixture
that contains the signal too cleanly, but one that cannot contain it at all.
These tests draw the same cherry at illustration resolution instead.

## What a golden covers, and what it does not

Seven images, and each is answerable for something the others are not.

Three pin the motif layer: a block field, a half-drop field, and a scatter
whose sampler would otherwise be free to drift between versions without any
test in one process noticing.

Four pin the ground layer, which until 2026-09-26 had none at all. That gap
was found the hard way: the way *every* ground's edges are drawn was changed,
and no golden moved, because all three covered patterns with no ground in them.
Every ground test asserted a property, that it tiles, that its edges carry
intermediate tones, that a level covers some of the tile, and a property that
still holds can hold over a print that has changed completely.

- `ground-check.png`: two threads and their crossing. The three tones no
  arrangement of motifs can produce, and a check that band edges stay hard.
- `ground-texture.png`: a grown field, quantised, sampled between its cells
  and resolved through the palette.
- `ground-ornament.png`: Truchet, which is made entirely of curves, so it is
  where a change to edge softening or to a stroke width in millimetres shows
  first.
- `ground-under-motifs.png`: the composition. The fabric, then the ground
  layer, then the motifs. Reversing those last two fails this image and
  nothing else in the suite.

**One per kind, not one per figure.** What a figure *is* is pinned exactly and
far more cheaply by the digest in `pattern-core`'s ornament tests, which walks
all six. What a golden adds is everything after that: millimetres to pixels,
stroke widths, slots through the palette, and the softening of an edge.

**A golden has to be legible.** The first version of `ground-under-motifs.png`
reused a seamlessness fixture, which draws its ground in slot 1, the cherries'
own red, under motifs thick enough to cover most of it. Every property held
and the picture was useless to review. A golden is looked at by a person, so a
fixture chosen to stress the engine is usually the wrong one for it.

## Seams that neither side tests

The component tests mock what the app sends to; the Rust tests never reach the
interface. Anything that has to *agree* between them belongs to neither, and
the capability manifest is the clearest case: the app imports a dialogue, Tauri
grants dialogues by name, and nothing checks that the two lists match.

`every_dialogue_the_interface_opens_is_one_it_is_allowed_to` reads both and
compares them. It was written after `confirm` had been missing from the
manifest since the Close button was built, and the failure showed only when a
document had unsaved changes, which is the moment it costs most.

A test like this is a dozen lines and needs no fixtures. Where two layers hold
half a fact each, read both halves.

## Asserting that something was drawn

An engine whose output is a picture has to assert on the picture. Until
2026-09-23 the only thing the app layer said about its preview was that the
base64 was non-empty, which a picture of plain background passes, along with
every other signal: the command succeeds, the dimensions are right, the PNG
decodes, the placement count is correct.

Four assertions now stand between the app and a preview that is quietly wrong,
in increasing order of what they catch:

1. **Something is drawn.** More than 5% of the preview is not the ground
   colour.
2. **It changes when the pattern does.** Rerolling the seed changes the bytes,
   and so does raising the coverage, the second because a preview regenerated
   from the seed alone could still ignore its parameters.
3. **It changes in the direction the control promises.** Four times the
   coverage puts measurably more ink down. A preview wired to the wrong
   parameter would pass (2) and fail this.
4. **It shows the colours the palette names.** Set slot 1 to magenta and find
   magenta in the image. Ties the picture to the palette rather than to itself.

All four are made on the **decoded base64** the interface actually receives,
not on the pixmap before encoding, because the encoding and the base64 are part
of what could be wrong. The decoder that makes that possible is checked against
the RFC's own vectors in both directions rather than by round-tripping the
encoder. A round trip agrees with itself whatever two mirrored mistakes it
contains.

**Verified to fail.** A renderer that resolves every sprite and draws none of
them breaks all four, and leaves the width, colour-count and window-size
assertions passing. That is the shape of the hazard: the signals you would
naturally check all agree that nothing is wrong.

## Anchors for driving the interface

Every control a test would drive carries a `data-testid`: the repeat fields,
the layout and repeat-type menus, every slider, the overlap checkbox, and the
New, Open, Save, Close and Export buttons. Readouts carry one derived from
their own label, since a readout has no behaviour to get wrong and a handle
derived from what it already says cannot drift out of step with it.

They are added because the house playbook is blunt that they are nearly free
while building and a tax to retrofit, and this project had accrued that tax,
having built a palette control, a damask slider, a close dialogue and a grid
warning with no way to drive any of them.

What anchors cannot do is show a layout. The playbook records a desktop UI
verified entirely through the accessibility tree, with every control present,
labelled and correctly enabled, whose right-hand panel was clipping its own
buttons. An anchor-driven test proves behaviour; only a picture proves layout.

## Driving the interface

`app/src/App.test.tsx` renders the real components in a headless DOM and drives
them by anchor, faking the engine at **one seam**, `app/src/api.ts`, the module
every Tauri command goes through. Mocking there rather than at the framework's
internals keeps the surface tiny: a test names the handful of commands it cares
about and every other export stays real.

It catches what the layer beneath it cannot reach. Which controls a layout
shows, in that a grid has rows and columns and no coverage, a mirror has coverage
*and* the mirror-line share. Whether a warning appears when the engine reports
a problem. Whether a confirmation actually gates a destructive action. All of
that lives in the component, and none of it is visible from the command layer,
however well tested that is.

**Verified to fail**, on the two that guard something that matters:

- Closing without asking, by calling `closeDocument(true)` straight away, breaks both
  close tests. That is a destructive action shipping ungated, and it is the
  kind of thing that works perfectly in every manual trial where you meant to
  say yes.
- Handing the engine the slider's index instead of the colour count breaks the
  palette test. The control steps through a list, so its own value is an index
  and the off-by-a-lookup is available at all times.

Three of the fourteen failed on their first run, and all three were faults in
the tests: `clear()` does not apply to a range input, and the preview's alt
text names the window and the document rather than saying "preview". Worth
recording because the eleven that passed immediately are the useful signal:
the harness and the app agreed about the app without being made to.

## The supporting levels

**Unit tests in `pattern-core`** cover the lattice, wrapping, palette resolution
and validation. The nine half-drop neighbour offsets are asserted against
explicit expected values rather than against a loop, since a loop could
reproduce the very mistake under test.

**Pixel-lattice tests** assert the property that makes tiles partition the
plane: the quantised cell area equals the tile area in pixels, at every
resolution tried.

**Determinism tests** assert byte-identical output for the same pattern, and,
deliberately, that a different lattice or a different seed produces different
bytes, so the equality assertions are known to be comparing something that could
differ. These are not to be weakened to make a change pass.

**Ingest tests** assert the pipeline's properties rather than its output:
paper is cut away and ink is not, the edge stays soft, physical size is
independent of how much paper the subject was drawn on, colour is flattened
while coverage is not, a colour the palette lacks is reported, a derived palette
ignores the background, and vector and raster agree about what is in the motif
and how big it is. Library tests cover the part that must not go wrong: a new
version never changes an old one, identical artwork does not stack up versions,
changed *parameters* do make a new one, and an id that would escape the library
directory is refused.

Two of those exist because something went wrong rather than because the property
seemed worth asserting. A test pins the stored format at two bytes per pixel
with metadata smaller than the pixel count, after JSON-encoding the channels
produced a 3.3 MB file for 352 KB of data. And the poor-fit warning found a real
design error the first time a colourway was rendered. Loose files were being
quantised against the colourway rather than against the palette the artwork was
drawn for.

**Scatter tests** assert the properties rather than impressions of them: no pair
of motifs closer than the separation radius *measured across lattice copies*, no
point in the tile further than 1.25 × that radius from a motif, coverage within
five points of the target, density monotonic in its input, and mix weights
respected. One test greps the scatter module for trigonometric calls, because
the cross-platform reproducibility claim depends on there being none.

Two of those bounds are empirical and both were corrected after measurement
rather than assumed. The packing constant was 0.69 from memory, which is a
different normalisation; measured against this sampler it is 0.533, and at the
wrong figure the density control was quietly delivering 77% of what it promised
while a loose tolerance hid it. Maximality was asserted at 1.0 × the radius;
Bridson with finite candidate attempts is near-maximal rather than maximal, and
the measured worst gap is 1.13 ×. Both measurements are recorded next to the
constants they justify, so a future drift fails loudly instead of degrading the
control.

**Golden images**, small and reviewable by eye, regenerated with
`UPDATE_GOLDEN=1 cargo test -p pattern-render`. Regenerating to make a failing
test pass discards the only record of what the renderer used to produce.

## What is deliberately not tested

Stated rather than left as silence, since silence reads as coverage.

- **Scanned illustrations, not on Peel Club's path.** Confirmed on
  2026-09-20: **Peel Club's artwork is digital**, not scanned. That makes its
  route through ingest the easy one: artwork exported with an alpha channel
  takes `ExistingAlpha`, and artwork on flat white is trivial for the paper
  measurement to handle. Neither meets uneven illumination, paper grain or ink
  bleed.

  The scan work is therefore insurance rather than a prerequisite: it serves
  the spec's third entry point, and Dreamwoven if generated motifs ever arrive
  on anything but a flat background. It was still worth doing, since it found the
  trimming defect, which affected *every* path including digital artwork, since
  a bounding box over every covered pixel is fragile whatever produced the
  pixels.

  What remains untested is a real scanner's own noise, real ink bleed, and a
  sheet's warp against a platen. Those are approximated by generated
  photographs, not reproduced. Nothing currently planned depends on them.

- **Real Peel Club artwork.** Still not seen, but the gap is now much smaller
  than it was described as. Digital illustration is well covered by the
  flat-vector motifs already put through the pipeline, which are digital
  artwork in every respect that matters to ingest. What real files may still
  bring: very large dimensions, embedded colour profiles, CMYK exports,
  semi-transparent airbrush or texture layers, and drop shadows baked into the
  export.

- **Cross-platform determinism, now largely evidenced, and no longer only an
  argument.** The concern was that *rendering* rotates through `sin_cos`, which
  IEEE-754 does not pin down, and that scatter makes rotation the common case
  rather than the exception. Placement generation was already believed exact,
  since it uses no transcendental arithmetic at all and a test enforces that, but
  pixels were not.

  Since 2026-09-22, CI runs the whole suite on **Linux x86-64** while
  development happens on **macOS arm64**. The golden images are generated on
  macOS and match on Linux, not approximately, but as identical encoded PNG
  bytes, which is what the comparison actually does. One of them,
  `a_scattered_field_matches_its_golden`, is a scatter with 30 degrees of
  rotation jitter, so the `sin_cos` path is exercised on every motif in it;
  another grows a Gray–Scott field, which is several million floating-point
  steps whose last bit has to agree.

  Two architectures, two C libraries, two rasteriser builds, identical output.
  That is much stronger than the argument it replaces, and it is a *standing*
  check rather than a one-off measurement.

  What it still does not cover: **Windows**, and any platform whose libm
  differs from both. And it evidences agreement between two implementations
  rather than proving a guarantee. A future Rust or libm release could still
  move one of them, which is now exactly what CI is there to notice.
- **The CLI's own behaviour.** No test drives the binary. It is thin, but it
  has grown: document loading, on-the-fly ingest, library resolution, the
  design-palette check and PNG writing all live there now, and the
  design-palette bug was found by running it rather than by a test. Worth
  covering before M4 adds a second consumer.

- **Vectorisation output, visually.** Tests assert that every substantial slot
  is traced and that the vector agrees with the raster on size, but nothing
  renders the SVG and compares it against the pixels. That needs an SVG
  rasteriser, and `resvg` is already named for M5, so it is the obvious thing to add
  when one is present.
- **Tracking.** Whether a pattern reads as accidentally corridored is not
  checked. Clumps and bald patches are measurable and are tested; corridors are
  not, until M6 builds the Radon check. Any claim that a scatter shows no
  visible corridors is currently somebody's opinion, and the golden images are
  there to be looked at rather than to settle it. It is a blocking gate for
  Dreamwoven rather than a nicety.

## Running them

```bash
cargo test                                  # whole workspace, 119 tests
cargo test -p pattern-core                  # fast, run constantly
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```
