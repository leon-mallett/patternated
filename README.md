# Patternated

Deterministic repeat-pattern engine for textile printing. Motifs in, print-ready
seamless patterns out.

Seamless by construction rather than by seam repair: motifs are placed on a
lattice and drawn at every neighbour offset that lattice implies, so the tile is
correct on the first render and there is never an edge to fix.

Two brands share the engine and nothing visual: **Peel Club** (adult apparel,
live) and **Dreamwoven** (bespoke children's pyjamas, concept).

**M1 to M5 are complete, and so are garment mockups.** A pattern is started from
artwork in the desktop app, arranged by scatter, by one of five classic grid
layouts or as a mirror repeat, recoloured through named colourways, previewed on
a garment at true scale, and exported as TIFF or PNG at any resolution, as a
single tile or a pre-repeated sheet, in RGB or in CMYK through a bureau's ICC
profile with soft proofing.

The spec directs a stop and reassessment at this point. What remains is listed
in [Roadmap](./docs/roadmap.md) and is explicitly optional.

Real Peel Club files have not been through the pipeline yet, though artwork of
the same kind, digital flat-colour illustration, has. See
[Test strategy](./docs/test-strategy.md#what-is-deliberately-not-tested).

## What you need

- **Rust.** Install [rustup](https://rustup.rs); the version is pinned by
  [`rust-toolchain.toml`](./rust-toolchain.toml) and rustup will fetch it on
  the first build. Nothing else needs choosing.
- **Node 20 or newer**, for the desktop app's front end. Only the app needs it;
  the engine and the command line do not.
- **On Linux**, the desktop shell's system libraries:

  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev \
      librsvg2-dev libxdo-dev libssl-dev patchelf
  ```

  On macOS the Xcode command line tools are enough: `xcode-select --install`.

## Run the app

```bash
cd app
npm install          # once, or after a dependency changes
npm run tauri dev    # builds the engine and opens the window
```

The first build compiles the whole workspace and takes a few minutes; after
that it is seconds, and editing a Rust file rebuilds and relaunches by itself.

Start a pattern from PNG artwork, one motif per file, on a transparent
background or on plain paper, or open one of the saved documents under
[`examples/`](./examples/).

The dev server runs on port **1421** rather than Tauri's default 1420, which
every scaffolded Tauri app claims and which something else on a shared machine
is often already holding.

## Try it without the app

Everything the engine does is reachable from the command line.

```bash
cargo run -p pattern-cli -- info examples/peel-club-cherries/pattern.json

# One tile, the artwork a print bureau receives
cargo run -p pattern-cli -- render examples/peel-club-cherries/pattern.json \
    --out out/tile.png --dpi 150

# A 3x3 field, the minimum at which tracking corridors become visible
cargo run -p pattern-cli -- render examples/peel-club-cherries/pattern.json \
    --out out/field.png --dpi 96 --cols 3

# A scattered print, and the same one reseeded
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern.json \
    --out out/scatter.png --dpi 96 --cols 3
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern.json \
    --out out/scatter-2.png --dpi 96 --cols 3 --seed 7

# The same print in another colourway: three colours differ, nothing else
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern-night.json \
    --out out/night.png --dpi 96 --cols 3
```

## Develop

```bash
cargo build                             # the whole workspace
cargo test -p pattern-core              # fast, run constantly
cargo test --workspace                  # includes the seamlessness tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all
python3 scripts/check-docs.py           # every documentation link resolves
cd app && npm test                      # the interface, in a headless DOM
cd app && npm run build                 # type-check and bundle the front end
```

[CI](./.github/workflows/ci.yml) runs all of these on every push, on Linux, while
development happens on macOS, so that is also the only standing check that the
same seed gives the same bytes on a second platform.

## Documentation

[`docs/`](./docs/): start at [`docs/README.md`](./docs/README.md).

The repository, its directory and the spec's filename still read
`pattern-engine`, which is what this was called until 2026-11-14. The slugs are
referenced by path from several places and renaming them buys nothing; the
lowercase "repeat-pattern engine" is the description, not the name.

The [technical spec](./docs/pattern-engine-spec.md) is the source of the
invariants; [`CLAUDE.md`](./CLAUDE.md) states the five that are load-bearing.
The one worth knowing before reading any code: **toroidal wrap uses the lattice
vectors, never the tile's width and height.** Those agree for a block repeat and
disagree for every oblique one, which is why the mistake reaches fabric. It
applies twice: to the nine offsets the tiling draws at, and to the distance
that decides whether two scattered motifs collide.

## Licence

All rights reserved. Copyright Captivated Ltd. See [`LICENSE`](./LICENSE).
