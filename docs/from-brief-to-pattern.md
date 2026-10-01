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

## The unsolved problem: a named interest can be absent

**This is the one that stops an order shipping.** Scatter samples each
placement independently from the weighted mix, so nothing guarantees that every
asset appears. Measured on the brief above, twelve assets in a 300 mm tile:

| Density | Placements | Assets present |
| --- | --- | --- |
| 0.30 | 22–26 | 7–9 of 12 |
| 0.45 | 34–41 | 8–9 of 12 |
| 0.60 | 46–50 | 7–9 of 12 |

Doubling the placements does not improve coverage, because the problem is not
scarcity. A hero holds about 3% of the mix weight, so its chance of being
absent from a given tile is roughly one in five however many placements there
are. The first render of this brief had no unicorn on it at all, and a parent
who asked for unicorns will not accept a pattern without one.

The fix is a **coverage constraint rather than a probability**: every asset
placed at least once per tile before the weighted mix fills the remainder. That
is an engine change, it is not in M1–M5, and it belongs with the unattended
gates rather than alongside them — a gate can only reject what the sampler
produced, and no amount of reseeding makes an absent unicorn appear.

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
