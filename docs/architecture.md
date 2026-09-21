# Architecture

Rust workspace, four crates planned and three built. The boundaries are chosen
so the core stays free of anything that could stop it being a pure function, and
that constraint is doing more work than it appears to.

See [Decisions](./decisions.md) for why each boundary sits where it does, and
[Features](./features.md) for what is actually implemented.

## The crates

| Crate | Carries | Depends on | Status |
| --- | --- | --- | --- |
| `pattern-core` | Lattice, toroidal distance, placement, scatter, palette, validation | `glam`, `rand_chacha`, `serde`, `thiserror` | Built |
| `pattern-render` | Rasterising a pattern at any resolution | `pattern-core`, `tiny-skia` | Built |
| `pattern-doc` | Document format, ingest-at-load, PNG writing | the above, `serde_json`, `png`, `anyhow` | Built |
| `pattern-cli` | Command line over `pattern-doc` | the above, `clap` | Built |
| `pattern-app` | Tauri 2 shell: session state and commands | the above, `tauri`, `tauri-plugin-dialog` | Built |
| `motif-ingest` | Matting, trim, quantise, vectorise, library | `pattern-core`, `visioncortex` | Built |
| `pattern-io` | Export, print size, file naming | `tiff`, `png` | Built |
| `pattern-io` (colour) | CMYK and ICC profiles | `moxcms` | M5 |
| `app/src` | React and TypeScript front end | `react`, `vite` | Built |

`pattern-cli` and `pattern-doc` are not in the spec's crate table. The spec
names a CLI for M1 without saying where it lives, and putting a binary in
`pattern-io` would have pulled colour management into M1 to get a command line.

`pattern-doc` was split out at M4 for a concrete reason: a GUI cannot depend on
a binary crate, and having it depend on one called "cli" would mislead the next
reader. It holds what every front end needs identically — what a document
means, how its motifs resolve, how a PNG is written — so the two cannot drift
into disagreeing about it. It is the app layer, and it is where `anyhow` is
allowed.

## Where the lattice invariant lives

Two places, and they must agree:

- `Lattice::neighbour_offsets` — the nine offsets the tiling draws at.
- `torus::lattice_distance_sq` — how far apart two motifs really are, given
  that each appears at every lattice translate.

Both exist because the tile rectangle is not the lattice. Substituting it is
correct for a block repeat and wrong for every oblique one, which is why the
mistake survives testing and reaches fabric. `PixelLattice` is a third, in
integer pixels, and mirrors the first deliberately.

## Where colour enters

Exactly one place: `pattern_render::motif::resolve`, which turns slot indices
into pixels using the pattern's palette. Everything upstream — ingest, storage,
placement, scatter — deals in indices.

That is why recolouring a whole range costs one palette and no re-rendering of
anything else, and it is the reason `palette_slot` was carried on placements
from the first commit, months of milestones before anything could resolve it.

`motif-ingest` is pure except for [`library`](./features.md#the-motif-library),
which is explicitly the I/O half and lives in its own module so the boundary is
visible rather than implied.

## The boundary that matters

**`pattern-core` performs no I/O, no async, no logging, and reads no clock.**

The stated reason is testability, and that is true but undersells it. Three
consequences follow that would each be expensive to retrofit:

- A pattern is a small JSON struct and the core is a pure function over it, so a
  hosted API — which Dreamwoven eventually needs — is a thin HTTP layer rather
  than a rewrite.
- Determinism is checkable. A function that reads no clock and opens no file
  cannot produce a different answer on Tuesday.
- Rendering can be moved to a GPU, a worker thread or a different language
  binding without the placement logic coming along.

If something in `pattern-core` appears to need a file handle, the design has
gone wrong somewhere upstream of it.

## How a render actually happens

Physical units go in and pixels come out at exactly one place.

1. `pattern-cli` reads a document, decodes motif PNGs, and hands
   `pattern-render` decoded pixels. The core never learns a path exists.
2. `pattern-render` quantises the lattice to whole pixels at the requested DPI —
   see `PixelLattice`, and [Decisions](./decisions.md#quantising-the-lattice-rather-than-the-offsets)
   for why this step exists at all.
3. Each placement is rasterised **once** into a sprite, then blitted at
   whole-pixel offsets wherever the lattice says it appears.
4. `pattern-cli` writes a PNG carrying the physical resolution.

## Two routes to a field, and why both exist

`pattern-render` offers `render_field`, which lays rendered tiles at lattice
offsets the way a printer does, and `render_field_direct`, which draws every
placement straight from the pattern definition with no tile in between.

The second is not a convenience. It is an independent implementation of the same
thing, and the fact that the two agree pixel for pixel is what the seamlessness
tests assert. A single implementation can only be checked against a golden image
somebody generated from it, which proves nothing about whether tiles join. See
[Test strategy](./test-strategy.md).

## Dependencies carrying real weight

- **`glam`** for vectors. Chosen because the spec names it and it is already
  used elsewhere in this portfolio, not for any property specific to 2D.
- **`tiny-skia`** for rasterising. Pure Rust, no C++ build chain, and a
  software rasteriser is the right default when output correctness matters more
  than preview speed. `resvg` joins it at M3 for vector motifs.
- **`rand_chacha`** rather than the default RNG. Reproducibility depends on the
  generator being stable across versions and platforms, which `thread_rng` does
  not promise. Only its raw bits are used: the conversion to floats lives in
  `scatter.rs` so that no dependency upgrade can move it. `rand_core` comes
  through `rand_chacha`'s own re-export rather than as a second dependency.
- **`png`** used directly, alongside the copy `tiny-skia` already pulls in,
  because `Pixmap::encode_png` writes no `pHYs` chunk and physical size has to
  survive into the file.
- **`moxcms`** rather than `lcms2` when colour management arrives at M5. The
  spec names `lcms2`, which is a binding to a C library and would have been the
  first non-Rust dependency in the tree. `moxcms` does ICC transforms between
  RGB and CMYK through an XYZ or Lab connection space in pure Rust, under
  BSD-3-Clause or Apache-2.0. Confirmed by reading its transform code rather
  than its description — its own documentation is ambiguous about CMYK.
- **`visioncortex`** for tracing, rather than the `vtracer` wrapper around it —
  see [Decisions](./decisions.md#visioncortex-rather-than-vtracer). Notably
  `motif-ingest` has **no image-format dependency at all**: it takes decoded
  RGBA, so decoding is a choice made at the edge rather than a dependency baked
  into the pipeline.
