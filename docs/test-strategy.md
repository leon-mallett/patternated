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

- **Real illustrations.** This is the largest gap and it is a deliberate one:
  M3 was built against synthetic input so the project would not stall waiting
  for artwork. Every test and every example uses a motif drawn in code. Nothing
  has met a real alpha edge, a scanned texture, paper that is not evenly lit, or
  artwork whose colours were not chosen by the test.

  So M3's own acceptance test — the existing Peel Club illustrations go in
  unmodified and come out clean and recolourable — **has not been run**. What is
  proven is that the pipeline does what it was designed to do on input designed
  to exercise it, which is not the same claim. The parts most likely to need
  adjusting are the matting threshold and feather width, and whether redmean
  quantisation is good enough on real colour.
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
cargo test                                  # whole workspace, 114 tests
cargo test -p pattern-core                  # fast, run constantly
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```
