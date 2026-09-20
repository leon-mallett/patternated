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
deliberately — that a different lattice produces different bytes, so the
equality assertions are known to be comparing something that could differ.
These are not to be weakened to make a change pass.

**Golden images**, small and reviewable by eye, regenerated with
`UPDATE_GOLDEN=1 cargo test -p pattern-render`. Regenerating to make a failing
test pass discards the only record of what the renderer used to produce.

## What is deliberately not tested

Stated rather than left as silence, since silence reads as coverage.

- **Real illustrations.** Every test uses a motif drawn in code. Nothing in M1
  has met a real alpha edge, a scanned texture, or artwork whose colours were
  not chosen by the test. First contact is at M3.
- **Cross-platform determinism.** Byte-identical output is asserted within a
  run on one machine (macOS, Apple silicon, 2026-09-20). The spec asks for
  identical output *across* platforms, and that is unverified. Rotation is the
  specific risk: `sin_cos` is not guaranteed bit-identical between platforms or
  libm versions, so a rotated placement could differ in its last bit somewhere
  else. Unrotated placements take a code path that avoids it entirely. This
  needs a second platform, or a deterministic trigonometric implementation, and
  it needs deciding before anyone relies on reproducing a print from a seed.
- **The CLI's own behaviour.** No test drives the binary. It is thin, and its
  two interesting parts — document loading and PNG writing — would be worth
  covering once a second consumer exists.
- **Tracking.** Whether a pattern reads as accidentally corridored is not
  checked. That is M6, and it is a blocking gate for Dreamwoven rather than a
  nicety.

## Running them

```bash
cargo test                                  # whole workspace
cargo test -p pattern-core                  # fast, run constantly
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```
