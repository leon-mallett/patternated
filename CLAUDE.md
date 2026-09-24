# CLAUDE.md

Deterministic repeat-pattern engine for textile printing. Motifs in, print-ready
seamless patterns out.

Two consumers share this engine:

- **Peel Club** — adult apparel, fruit prints, illustrations already exist. Live.
- **Dreamwoven** — bespoke children's pyjamas, motifs generated per order. Concept.

They share the engine and nothing visual. Full technical spec lives in
`docs/pattern-engine-spec.md`.

**Current milestone: past M5.** M1–M5 are complete and the spec directs a stop
and reassessment there; `docs/roadmap.md` is the live statement of what is done,
what is optional, and what is owed. Read it before assuming a thing is unbuilt.

## Invariants

Breaking any of these produces output that looks correct in preview and fails at
the printer. Treat them as load-bearing.

1. **Physical units throughout.** All geometry in millimetres. Convert to pixels
   only inside the export layer. Never store or reason about a pattern in pixels —
   a 12 cm repeat and a 40 cm repeat are different designs, not different zoom
   levels.

2. **Toroidal wrap uses lattice vectors, not tile dimensions.** The nine neighbour
   offsets are `i*u + j*v` for `i, j ∈ {-1, 0, 1}`. Never `±W, ±H`. Hardcoding
   width and height tiles correctly for block repeats and silently fails for
   half-drop, which is not visible until fabric comes back wrong.

3. **`pattern-core` is pure.** No I/O, no async, no network, no logging, no clock.
   It is a function from parameters to placements. If something in there needs a
   file handle, the design has gone wrong — move it out.

4. **Determinism is a feature.** `rand_chacha` with an explicit seed, never
   `thread_rng`. Same seed plus same parameters must give byte-identical output
   across platforms and versions. Tests assert this; do not weaken them to make a
   change pass.

5. **Palette slots, never literal colours.** Motifs store `u8` slot indices and
   resolve to colour at render time. Colourways depend entirely on this, and
   retrofitting it later is painful.

## Layout

```
crates/
  pattern-core/     lattice, placement, tiling, colourways — pure
  pattern-render/   raster and vector rendering at any DPI
  motif-ingest/     matting, trim, vectorise, palette extraction
  pattern-io/       export, colour management, file naming
app/                Tauri 2 shell (React + TypeScript)
docs/               spec and reference material
```

## Commands

```bash
cargo test -p pattern-core              # fast, run constantly
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cd app && npm run tauri dev             # app shell
```

## Testing

Tiling correctness is the thing that matters. Every lattice type needs a test that
renders a 3×3 field and asserts edge continuity — sample pixels either side of each
tile boundary and assert they match. A single tile that looks fine proves nothing.

Determinism tests: same seed, same parameters, assert identical bytes.

Golden-image tests for render output. Regenerate deliberately and review the diff;
never regenerate to make a failing test pass.

## Conventions

- Rust 2021. `thiserror` for library errors, `anyhow` only in the app layer.
- `color` for primitive types, matching the ecosystem (`tiny-skia` and friends).
  `colourway` for the domain concept, where there is no upstream clash.
- No `unwrap()` or `expect()` outside tests.
- `pattern-core`'s public API takes and returns plain data. No builders, no traits
  until there are genuinely two implementations.
- Ask before adding a dependency.

## Scope discipline

The spec runs M1 to M7. M1–M5 are built; M6 is built but for corridor
detection, and M7 is gated on things that do not exist yet. Do not implement
generation or style adapters ahead of their milestone, however tempting the
adjacent code makes it. Partial implementations of later milestones are worse
than none — they get forgotten and then trusted.

M7 is gated on a written style guide and reference artwork, and on a Peel Club
range having gone to fabric and come back right. Those are the gates, not the
calendar.

Explicitly out of bounds right now:

- Generative image calls (M7, and behind a trait when they arrive)
- Any network I/O at all
- Cloud sync, multi-user, hosted API
- CMYK export, until a bureau supplies a profile — see
  `docs/features.md#colour-management` for why shipping one would be a guess
