# Pattern Engine

Deterministic repeat-pattern engine for textile printing. Motifs in, print-ready
seamless patterns out.

Seamless by construction rather than by seam repair: motifs are placed on a
lattice and drawn at every neighbour offset that lattice implies, so the tile is
correct on the first render and there is never an edge to fix.

Two brands share the engine and nothing visual — **Peel Club** (adult apparel,
live) and **Dreamwoven** (bespoke children's pyjamas, concept).

**Current milestone: M3 complete.** Block and half-drop lattices, hand placement
and toroidal Poisson-disc scatter, motif ingest with a versioned library, and
working colourways. PNG output at any resolution. Next is M4, the Tauri shell.

Real Peel Club files have not been through the pipeline yet, though artwork of
the same kind — digital, flat-colour illustration — has. See
[Test strategy](./docs/test-strategy.md#what-is-deliberately-not-tested).

## Try it

```bash
cargo run -p pattern-cli -- info examples/peel-club-cherries/pattern.json

# One tile — the artwork a print bureau receives
cargo run -p pattern-cli -- render examples/peel-club-cherries/pattern.json \
    --out out/tile.png --dpi 150

# A 3x3 field — the minimum at which tracking corridors become visible
cargo run -p pattern-cli -- render examples/peel-club-cherries/pattern.json \
    --out out/field.png --dpi 96 --cols 3

# A scattered print, and the same one reseeded
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern.json \
    --out out/scatter.png --dpi 96 --cols 3
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern.json \
    --out out/scatter-2.png --dpi 96 --cols 3 --seed 7

# The same print in another colourway — three colours differ, nothing else
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern-night.json \
    --out out/night.png --dpi 96 --cols 3
```

## Develop

```bash
cargo test -p pattern-core              # fast, run constantly
cargo test                              # includes the seamlessness tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

## Documentation

[`docs/`](./docs/) — start at [`docs/README.md`](./docs/README.md).

The [technical spec](./docs/pattern-engine-spec.md) is the source of the
invariants; [`CLAUDE.md`](./CLAUDE.md) states the five that are load-bearing.
The one worth knowing before reading any code: **toroidal wrap uses the lattice
vectors, never the tile's width and height.** Those agree for a block repeat and
disagree for every oblique one, which is why the mistake reaches fabric. It
applies twice — to the nine offsets the tiling draws at, and to the distance
that decides whether two scattered motifs collide.

## Licence

All rights reserved. Copyright Captivated Ltd. See [`LICENSE`](./LICENSE).
