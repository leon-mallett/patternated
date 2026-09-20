# Scatter, and the same print in two colourways

```bash
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern.json \
    --out out/day.png --dpi 96 --cols 3
cargo run -p pattern-cli -- render examples/peel-club-scatter/pattern-night.json \
    --out out/night.png --dpi 96 --cols 3
```

The two documents are identical but for three colours. Everything else — the
scatter seed, every placement, every rotation — is the same, because motifs
store palette slots rather than colours and the palette is resolved at render.
That is the whole colourway mechanism, and it is why
[the slot machinery was built before anything could use it](../../docs/decisions.md).

The motif here is a loose PNG, ingested on the spot. A real range would ingest
once into a library instead, so that the motif is immutable and shared:

```bash
cargo run -p pattern-cli -- ingest examples/peel-club-scatter/cherry.png \
    --library out/library --id cherry --size-mm 26 \
    --palette-from examples/peel-club-scatter/pattern.json
```

A document then names the library and lists the motif by id, with no `file`:

```json
{ "motifs": [{ "id": "cherry" }], "library": "out/library", ... }
```

Add `--from-paper` when ingesting a scan rather than artwork exported with
transparency: it cuts the subject away from the paper by brightness, with a
feathered edge, instead of trusting an alpha channel that is not there.

The library is derived data and is not committed. Rebuild it with the command
above.
