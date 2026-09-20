# Licensing and IP

## This repository

All rights reserved. Copyright Captivated Ltd. See [`LICENSE`](../LICENSE).

The portfolio default, recorded deliberately rather than left silent, because
silence is the ambiguity the house rule objects to. Nothing here is offered
under an open-source licence and nothing is published to a registry — every
crate is `publish = false`.

## Dependencies

All permissively licensed (MIT or Apache-2.0, or both): `glam`, `tiny-skia`,
`rand_chacha`, `serde`, `thiserror`, `anyhow`, `clap`, `png`, `serde_json`, and
`visioncortex` with its `bit-vec`, `flo_curves` and `num-traits`. Nothing
copyleft, nothing with an attribution obligation beyond notice, and `cargo
audit` is clean as of 2026-09-20.

`lcms2` arrives at M5 and deserves checking before it lands rather than after:
it sits in the export path, where a licence problem would be expensive to
unwind, and it is a binding to a C library rather than a Rust crate, which makes
it a different kind of decision from everything currently in the tree.

## Artwork

**Peel Club's illustrations were acquired outright**, so the position on them is
clean.

**They are nonetheless unavailable to Dreamwoven.** The two brands share the
engine, the palette machinery and the pipeline, and nothing visual. Peel Club's
artwork cannot be used as a style primer for Dreamwoven's generated motifs — not
for want of rights, but because they are separate brands with no shared visual
identity, and a shared style would undermine both.

The consequence is a real cost on a concept-stage product: Dreamwoven needs its
own style reference built from nothing, and that gates all of
[M7](./roadmap.md#m7--dreamwoven-pipeline).

## The unresolved one

**Dreamwoven generates motifs from what a child likes, and what children like is
largely licensed characters.** This is the highest commercial risk in the
concept and it is not an edge case — expect it on a large share of orders.

Mitigations are layered, in order of effectiveness: guided category input rather
than a free-text box, which shapes expectations before a parent types a name; a
blocklist of characters, franchises and common misspellings checked before
generation; a model-side check on the generated motif, since a prompt can
describe a protected character without naming it; and a graceful redirect that
keeps the order alive.

None of this is built, and none of it is a legal opinion. It needs someone who
does this properly before Dreamwoven sells anything.

## Adding a dependency

Ask first. State the exact package and version, check when it was published, and
treat anything released in the last 72 hours as needing explicit confirmation.
