# Licensing and IP

## This repository

**AGPL-3.0-only**, Copyright Captivated Ltd. Applied 2026-11-02. See
[`LICENSE`](../LICENSE) for the licence and [`NOTICE`](../NOTICE) for what
Captivated reserves alongside it.

The text in `LICENSE` is the Free Software Foundation's, fetched verbatim and
unmodified. It is not paraphrased and must not be edited, because an altered licence
is a different licence, and nobody can tell which one they have agreed to.

Why this and not a permissive licence is in
[Commercialisation](./commercialisation.md#why-the-engine-is-free); how it
coexists with a closed Dreamwoven is below.

## Keeping Dreamwoven closed while the engine is open

*Not legal advice. The mechanism below is standard and well understood. It was
applied on Leon's explicit approval and still deserves a solicitor's
half-hour; the recommendation stands and is simply no longer blocking.*

### The mechanism is sole ownership

A copyright holder is not bound by the licence it grants to everybody else.
Captivated Ltd owns the engine outright, so it can publish it under AGPL-3.0
*and* use that same code inside Dreamwoven under entirely different terms. The
licence constrains the public; it does not constrain the owner.

This matters more than it might seem, because Dreamwoven **statically links**
the engine. It depends on `pattern-core` and the rest by path, and they are
compiled into its binary. Static linking is generally treated as producing a
combined work, so under a strict reading an AGPL engine would pull Dreamwoven
into AGPL as well. It does not, because the owner is not a licensee of its own
copyright. That is the whole trick, and it only works while the first sentence
of this section stays true.

### The thing that breaks it

**Accepting a code contribution.** A contributor holds copyright in their
patch. The moment one is merged, Captivated no longer owns the engine outright
and can no longer relicense it unilaterally, which means it can no longer use
that version inside a closed Dreamwoven without the AGPL question becoming
real.

Open-sourcing invites exactly this. Two ways to handle it:

- **Do not accept code contributions.** Issues and bug reports welcome, pull
  requests closed with thanks. This costs nothing, preserves total freedom, and
  is a common and legitimate position for a project with one maintainer. It is
  the recommendation for now.
- **A contributor licence agreement**, if contributions ever become worth
  having. Contributors grant Captivated a broad licence back, so ownership
  stays effectively whole. Worth the administrative weight only once there is
  something to administer.

### What has to be written down

Four things, all now in place, and the fourth is the one people forget:

1. `LICENSE` carries the AGPL-3.0 text.
2. Every `Cargo.toml` says `license = "AGPL-3.0-only"` rather than
   `UNLICENSED`.
3. A `NOTICE` stating the dual position explicitly: *"Copyright Captivated Ltd.
   Licensed to the public under AGPL-3.0. Captivated Ltd reserves all other
   rights, including the right to license this software under other terms and
   to use it in its own products."* Without that sentence the arrangement is
   invisible, and an invisible arrangement reads as an accident.
4. **Dreamwoven's own README says why it is allowed to do this.** A reader who
   finds an AGPL dependency statically linked into an all-rights-reserved
   product will reasonably conclude a licence violation. One line naming the
   separate grant from the copyright holder prevents that, and costs nothing.

Dreamwoven's repository stays private regardless. The explanation above is for
the day it does not.

### Two things the code licence does not cover

**Trademarks.** "Patternated", "Dreamwoven" and "Captivated" are not licensed
by AGPL and are reserved explicitly in the `NOTICE`. Somebody is free to fork
the engine; they are not free to call their fork by your name.

The engine was called "Pattern Engine" until 2026-11-14, and that reservation
would not have held: a descriptive mark is weak to unregistrable, and several
projects already call themselves a pattern engine. The rename to a coined word
is what makes the clause enforceable; see
[Decisions](./decisions.md#the-engine-is-called-patternated-and-the-description-is-not-the-name).

**Generated artwork.** Nothing about the engine's licence says anything about
who owns a motif a model produced for an order, which is a separate question
and belongs with [the unresolved one](#the-unresolved-one).

### If AGPL feels like too much rope

The alternative that keeps the same guarantee with less ambiguity is **not to
open the source at all**: ship a free, signed binary and keep the repository
private. Designers get the tool, you keep everything, and the only thing lost
is the credibility of being inspectable.

What does *not* work for this goal is a permissive licence. MIT or Apache-2.0
hands a competitor the right to build a closed Dreamwoven rival on your engine,
which is precisely the outcome the AGPL choice exists to prevent.

## Dependencies

All permissively licensed (MIT or Apache-2.0, or both): `glam`, `tiny-skia`,
`rand_chacha`, `serde`, `thiserror`, `anyhow`, `clap`, `png`, `serde_json`, and
`visioncortex` with its `bit-vec`, `flo_curves` and `num-traits`. Nothing
copyleft, nothing with an attribution obligation beyond notice, and `cargo
audit` is clean as of 2026-09-20.

`moxcms` (BSD-3-Clause or Apache-2.0) and `tiff` (MIT) arrive at M5. `moxcms`
replaces the `lcms2` the spec names, which is a binding to a C library and would
have been the first non-Rust dependency in the tree.

## Colour profiles

M5 ships ICC profiles, which is a licensing decision as much as a colour one,
and the terms were read rather than assumed. Every profile chosen is
redistributable without restriction: `sRGB2014.icc` from the ICC, and the ECI
and IDEAlliance CMYK profiles (PSO Coated, GRACoL, SWOP, CGATS21, XCMYK 2017).
Heidelberg's `ISOcoated` series is deliberately avoided, being bundleable but with
conditions about not charging extra for it, which is a needless complication
when unrestricted equivalents exist. Nothing from Adobe.

See [Commercialisation](./commercialisation.md#which-profiles-specifically) for
the table and the caveat that these are all offset litho profiles rather than
textile ones.

## Artwork

**Peel Club's illustrations were acquired outright**, so the position on them is
clean.

**They are nonetheless unavailable to Dreamwoven.** The two brands share the
engine, the palette machinery and the pipeline, and nothing visual. Peel Club's
artwork cannot be used as a style primer for Dreamwoven's generated motifs, and not
for want of rights, but because they are separate brands with no shared visual
identity, and a shared style would undermine both.

The consequence is a real cost on a concept-stage product: Dreamwoven needs its
own style reference built from nothing, and that gates all of
[M7](./roadmap.md#m7-dreamwoven-pipeline).

## The unresolved one

**Dreamwoven generates motifs from what a child likes, and what children like is
largely licensed characters.** This is the highest commercial risk in the
concept and it is not an edge case. Expect it on a large share of orders.

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
