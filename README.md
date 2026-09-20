# Pattern Engine

Deterministic repeat-pattern engine for textile printing. Motifs in, print-ready
seamless patterns out.

Seamless by construction rather than by seam repair: motifs are placed on a
lattice and drawn at every neighbour offset that lattice implies, so the tile is
correct on the first render and there is never an edge to fix.

Two brands share the engine and nothing visual — **Peel Club** (adult apparel,
live) and **Dreamwoven** (bespoke children's pyjamas, concept).

**Current milestone: M1.** Bare engine, CLI-driven, block and half-drop
lattices, PNG output.

## Try it

```bash
cargo run -p pattern-cli -- info examples/peel-club-cherries/pattern.json

# One tile — the artwork a print bureau receives
cargo run -p pattern-cli -- render examples/peel-club-cherries/pattern.json \
    --out out/tile.png --dpi 150

# A 3x3 field — the minimum at which tracking corridors become visible
cargo run -p pattern-cli -- render examples/peel-club-cherries/pattern.json \
    --out out/field.png --dpi 96 --cols 3
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
disagree for every oblique one, which is why the mistake reaches fabric.

## Licence

All rights reserved. Copyright Captivated Ltd. See [`LICENSE`](./LICENSE).
