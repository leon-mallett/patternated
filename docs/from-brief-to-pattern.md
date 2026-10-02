# From brief to pattern

How a Dreamwoven order becomes a printable repeat, and what the engine does and
does not do along the way.

A brief is a child's interests plus an occasion:

> I like ice cream, dinosaurs, monster trucks and unicorns. It's for pyjamas,
> so it should be night time.

That yields roughly a dozen generated motifs — several variants per interest —
plus a small occasion set, on a background colour. **The motifs carry the
design. The background is a colour.** An earlier draft of this page had it
backwards, with rule-drawn grounds carrying the character and generic motifs
strewn over them; that produces something that merely feels nocturnal, which is
not what is being sold.

## Style is set in the prompt, not in the engine

Cartoony, flat and geometric, photo-realistic, hand-drawn doodle, stickmen,
watercolour, or a borrowed colour scheme — these are instructions to the image
model. The engine never renders in a style. It receives whatever comes back and
assembles it.

So style is free to vary per order, and the renderer needs no cases for it. But
the engine imposes two clauses that every prompt carries **whatever the style
is**:

- **One subject, isolated on a flat background.** Not a stylistic preference:
  ingest mattes by measuring the paper and by alpha, and there is no
  background-removal model. A photo-realistic dinosaur standing in a
  photo-realistic jungle cannot be cut out. The spec says the same thing from
  the model's side — one isolated subject is the single thing image models do
  reliably.
- **Everything lands on one shared palette.** Twelve motifs generated
  separately arrive with twelve palettes; ingest quantises them all onto one.
  This is not a limitation to work around, it is the main reason a dozen
  unlike things read as one design rather than as a sticker sheet — much of
  what reads as style mismatch is really colour mismatch.

The second clause does bound the styles that survive well. Slots are `u8`, so a
palette tops out at 256 colours, and a watercolour or photo-real motif
quantised onto a small palette posterises. A generous palette carries it; what
that costs is colourways, which stop meaning much once the palette is large.
For direct-to-garment printing there is no per-colour cost, so this is a
fidelity decision rather than a money one.

## The three tiers

A brief sorts its own assets, and the hierarchy is what the scatter needs:

| Tier | What it is | Drawn | Frequency |
| --- | --- | --- | --- |
| Hero | The child's named interests | Large | Rare |
| Secondary | Variants that stop repetition reading as repetition | Medium | Occasional |
| Filler | The occasion set — moon, star, sparkle | Small | Numerous |

Fillers are not padding. They come from the occasion rather than from the
child, they are what makes the space between heroes look intentional, and they
are what carries the "night time" read that the background colour alone does
not.

## Every named interest appears

Scatter samples each placement independently from the weighted mix, so for most
of this engine's life nothing guaranteed that a given motif appeared at all.
Measured on the brief above, before the fix:

| Density | Placements | Assets present |
| --- | --- | --- |
| 0.30 | 22–26 | 7–9 of 12 |
| 0.45 | 34–41 | 8–9 of 12 |
| 0.60 | 46–50 | 7–9 of 12 |

Doubling the placements did nothing, because scarcity was not the problem. A
hero holds about 3% of the mix weight, so it was absent from roughly one tile
in five however many placements there were. The first render of this brief had
no unicorn on it.

A mix entry can now be marked **required**. A required motif is offered part of
each sample's dart budget, which means it is placed while the tile is still
emptiest, and `scatter_with_outcome` **names any requirement it could not
meet** rather than passing over it. Twelve of twelve now land on every seed and
density tried.

Two things about the shape of this are worth keeping:

- **It is a constraint in the sampler, not a gate after it.** A gate rejects
  what the sampler produced and asks for another seed; every seed draws from
  the same process, so no reseed makes an absent unicorn appear.
- **The report is as important as the constraint.** A tile can be genuinely too
  small for everything asked of it, and that has to arrive as a named
  shortfall — the caller can then shrink a motif, grow the tile or ask a
  person. Silence would be indistinguishable from success.

A grid refuses the flag outright rather than ignoring it: it fills every cell
from the weighted mix and has no coverage guarantee to offer, so accepting the
flag there would be a lie.

## How common each motif is

Abundance is per motif, not per tier. `MotifWeight::weight` is a relative
tendency — weights need not sum to anything — so a night sky can carry several
times more stars than moons, which a tier-wide weight could not express.

Measured over twelve seeds at the reference settings, with stars weighted 3.4
against moons at 1.0:

| Motif | Per tile |
| --- | --- |
| star | 5.7 |
| sparkle | 4.8 |
| cone (either flavour) | 1.9 |
| owl | 1.7 |
| moon | 1.3 |
| each hero | 1.0–1.2 |

**A weight is a tendency and `required` is a floor, and the floor wins.** Nine
of the twelve assets here are required, so nine of about twenty-two placements
are spoken for before the weighted mix draws anything — which is why the heroes
land at 1.0 to 1.2 rather than the 0.8 their weight alone implies, and why the
stars land a little under their share. The fewer placements a tile holds, the
more of it the floor occupies and the less the ratios can say. Strong ratios
need either a busier tile or a shorter required list.

There is no control for an exact count. `required` guarantees *at least one*;
beyond that a tile is a draw, not a quantity. A minimum above one would be a
small extension to the same mechanism if an order ever wants "three dinosaurs,
not one".

## A size control must not re-solve the layout

The buyer sees the pattern and can adjust it — resize individual motifs, choose
an arrangement. That has one hard engineering consequence, measured rather than
guessed.

Scatter is a whole-cloth solve. Separation depends on every motif's radius, so
changing one motif's size changes acceptance decisions, which changes which
darts land, which changes everything after them. Re-solving the reference brief
with a single asset resized:

| Change | Layout surviving a re-solve | Scaled in place |
| --- | --- | --- |
| unicorn ×1.4 | 4% | 100%, 1 overlapping pair |
| unicorn ×1.2 | 8% | 100%, 1 overlapping pair |
| stegosaur ×1.3 | 4% | 100%, 2 overlapping pairs |
| sparkle ×1.5 | 8% | 100%, 0 overlapping pairs |

A slider that re-solves is not "make the dinosaur bigger". It is "reroll the
entire design, and also the dinosaur is bigger" — on every tick of the drag.

So a size control changes [`Placement::scale`](../crates/pattern-core/src/placement.rs)
on the placements already solved, and touches no position. The layout survives
by construction and the only question left is collisions, which come out at
nought to two pairs at the sizes a person would plausibly ask for. That is
small enough to clamp the control against: the slider's maximum is where the
existing layout starts to collide, which is computable from the layout in hand.

### Design at the maximum, ship under it

Knowing the headroom exposes a second problem. `prevent_overlap` packs a tile
until motifs nearly touch, so a layout solved at the size it is drawn has
almost no slack in it — the heroes of the reference brief come out at **×1.01**.
A "make the unicorn bigger" slider with one per cent of travel is not a
control.

The fix is to solve the layout as though every motif were larger than it will
be drawn, and then draw it smaller. Every motif can then grow back into space
the layout already reserved, and shrinking was always free. At a slack of 1.3
the heroes go from ×1.01 to about ×1.4–1.5.

It is paid for in **density, and only density**. Solving at larger sizes fits
fewer motifs — 25 placements become 14 — and the density control buys them
back: 0.30 with no slack and 0.55 at a slack of 1.3 both give 24 or 25.

One claim was nearly recorded here that turned out to be false. A single render
at slack 1.3 scored 0.54 for tracking against 0.28 without, and read as more
regular by eye, which looked like slack buying corridors. Averaged over eight
seeds the figures are 0.42, 0.49 and 0.34 for slack 1.0, 1.2 and 1.3 — noise,
with no trend. One seed is not a measurement of a stochastic process.

Re-solving still belongs behind an explicit control — a **shuffle** — where a
wholesale change is what was asked for rather than a surprise. Shuffle is also
a reseed, which is what the tracking check wants when it fires, so the same
button serves the buyer and the quality gate.

## Seeing it

```bash
cargo run -p pattern-cli --example dreamwoven-brief -- <out-dir>
D=0.45 S=3 cargo run -p pattern-cli --example dreamwoven-brief -- <out-dir>
```

Twelve silhouettes drawn in code, standing in for generated artwork. No model
is involved and nothing reaches the network. They are crude on purpose:
assembly is a question of size, visual weight and how many of each thing there
are, none of which care how well the dinosaur is drawn. A beautifully drawn
pile assembled wrongly still reads as a sticker sheet.
