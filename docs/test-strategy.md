# Test strategy

Tiling correctness is the thing that matters. Everything else here is ordinary.

The failure this suite exists to prevent has a specific shape: it looks correct
in every preview, passes any test written against a single tile, and is
discovered when fabric comes back wrong. Tests that can only confirm the
renderer agrees with itself are no use against it.

## The test that earns its keep

`crates/pattern-render/tests/seamlessness.rs` renders the same field two
independent ways and requires them identical:

- **By tiling** — render one tile, then lay copies at lattice offsets, which is
  what a printer does with the file.
- **Directly** — draw every placement straight from the pattern definition over
  the whole field, with no tile in between, which is what the repeat means.

Any mishandling of a motif crossing a tile edge makes the two disagree, and the
disagreement sits exactly where the seam would have been.

**This test was verified to fail.** The hardcoded-dimensions bug — neighbour
offsets taken from the tile rectangle instead of the lattice vectors — was
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
be perfectly symmetric while the picture is not — a sprite of odd width centred
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
carrying a defect on its own is a reason to trust it more, not less — but only
because it has been watched fail. `asking_for_motifs_on_the_lines_actually_puts_some_there`
sits beside it because every symmetry assertion in the file would also pass if
nothing were ever mounted.

The residual asymmetry was measured rather than tolerated. Before positions
were quantised against the tile it ran between 0.28% and 1.6% of pixels with
channel differences over 200; that measurement is what made the quantisation
change worth [deciding](./decisions.md#quantising-positions-against-the-tile-not-against-the-dpi)
rather than a tolerance worth widening.

## The supporting levels

**Unit tests in `pattern-core`** cover the lattice, wrapping, palette resolution
and validation. The nine half-drop neighbour offsets are asserted against
explicit expected values rather than against a loop, since a loop could
reproduce the very mistake under test.

**Pixel-lattice tests** assert the property that makes tiles partition the
plane: the quantised cell area equals the tile area in pixels, at every
resolution tried.

**Determinism tests** assert byte-identical output for the same pattern, and —
deliberately — that a different lattice or a different seed produces different
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
design error the first time a colourway was rendered — loose files were being
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

- **Scanned illustrations — not on Peel Club's path.** Confirmed on
  2026-09-20: **Peel Club's artwork is digital**, not scanned. That makes its
  route through ingest the easy one — artwork exported with an alpha channel
  takes `ExistingAlpha`, and artwork on flat white is trivial for the paper
  measurement to handle. Neither meets uneven illumination, paper grain or ink
  bleed.

  The scan work is therefore insurance rather than a prerequisite: it serves
  the spec's third entry point, and Dreamwoven if generated motifs ever arrive
  on anything but a flat background. It was still worth doing — it found the
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

- **Cross-platform determinism, in the renderer.** Byte-identical output is
  asserted within a run on one machine (macOS, Apple silicon, 2026-09-20). The
  spec asks for identical output *across* platforms, and that is still
  unverified for pixels.

  M2 narrowed this rather than closing it. *Placement generation* is now
  believed cross-platform exact: scatter uses no transcendental arithmetic at
  all, only operations IEEE-754 specifies exactly, and a test enforces that. So
  the same seed should give the same placements anywhere. *Rendering* still
  rotates through `sin_cos`, which carries no such guarantee — and M2 makes
  rotation the common case rather than the exception, since scatter jitters
  every motif. Unrotated placements avoid the path entirely.

  Still needs a second platform, or a deterministic trigonometric
  implementation, before anyone relies on reproducing a print from a seed.
- **The CLI's own behaviour.** No test drives the binary. It is thin, but it
  has grown: document loading, on-the-fly ingest, library resolution, the
  design-palette check and PNG writing all live there now, and the
  design-palette bug was found by running it rather than by a test. Worth
  covering before M4 adds a second consumer.

- **Vectorisation output, visually.** Tests assert that every substantial slot
  is traced and that the vector agrees with the raster on size, but nothing
  renders the SVG and compares it against the pixels. That needs an SVG
  rasteriser — `resvg` is already named for M5 — and is the obvious thing to add
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
