# House styles

Dreamwoven's style is a **choice the buyer makes**, not one constant baked into
the engine. This page is the draft the choice is made from. It is a working
document — the palettes and the prose are meant to be edited.

The [spec's style-coherence section](./pattern-engine-spec.md#style-coherence)
argues that style must be *fixed*, or a dinosaur, an ice cream and a football
come back drawn by three different illustrators. That argument is about one
garment, not about the whole product. Fixed **within** an order, chosen
**between** orders, is the same guarantee with a dial on it.

## Two dials, not one list

A theme and an idiom are different things and they compose.

- A **theme** decides subject matter and colour. Starry night. Deep sea.
- An **idiom** decides how any subject is drawn. Flat geometric. Bold outline.

Kept apart, the reference artwork a style adapter needs is **one set per
idiom** and the palette work is **one per theme**, so the cost is
`idioms + themes`. Rolled into a single list of styles it becomes
`idioms × themes` — three idioms and twelve themes is thirty-six reference
sets, and that is the difference between a commissionable amount of artwork
and an uncommissionable one.

## Palette roles

Every theme fills the same five roles. This is the part of a style guide the
engine can enforce today, because [`quantise`](./architecture.md) already forces
any artwork onto shared slot indices.

| Slot | Role | Notes |
| --- | --- | --- |
| 0 | Cloth | Never painted. What shows through. |
| 1 | Ground figure | The rule-drawn or grown layer behind the motifs. |
| 2 | Fill | A motif's primary colour. |
| 3 | Accent | A motif's second colour. |
| 4 | Line | Outlines and detail. |

Because the roles are fixed, a motif drawn for one theme recolours into another
for free — that is the colourway machinery already in the engine, used for
style rather than for a colour range.

**The line role is defined against the cloth, not against the fill.** A
near-black line on near-black cloth is not a subtle outline, it is an absent
one; the bold-outline idiom vanished completely on the starry-night theme until
that role was read correctly. Dark themes need a light line.

## The idioms

### Flat geometric

Subject reduced to flat shape and colour. No outline. Two to four shapes per
motif.

The spec's most portable idiom: a dinosaur and an ice cream resolve to the same
visual grammar, so coherence survives arbitrary subject matter. It is also the
cheapest thing to vectorise cleanly and the kindest to a small palette.

### Bold outline

A heavy uniform outline carrying the style, flat fill inside it. Equally
portable, because the outline is what reads as the style and it is indifferent
to what it encloses.

**It has an ingest requirement the other idioms do not.** The outline has to be
a real stroke converted to an outline, which is what the engine already
specifies for vector artwork. The cheap substitute — scale the shape up and lay
it underneath — was tried in the draft renders and is wrong for any shape with
a hole in it: growing a crescent grows the bite out of it too, and the outline
wanders off the edge it is meant to trace.

### Chalk wash

Flat fill with a second, offset wash over it at partial opacity.

This is as close to watercolour as a slot-based engine honestly gets, and the
gap is worth stating plainly rather than discovering later. **True watercolour
fights the palette model**: it lives in soft edges and continuous gradients, and
the engine resolves every motif to flat palette slots. A watercolour idiom would
either posterise into bands or need so many slots that colourways stop meaning
anything. The spec's own portability table rates fine linework and watercolour
*low* for exactly the related reason — the style lives in subject-specific
detail, so it collapses across unlike objects.

If a soft look is wanted, this is the achievable version of it. If genuine
watercolour is wanted, that is a different engine.

## The themes

Draft palettes. Hex values are meant to be argued with.

| Theme | Cloth | Ground figure | Fill | Accent | Line |
| --- | --- | --- | --- | --- | --- |
| Starry night | `#1A204A` | `#2D376E` | `#F7E7B0` | `#9EC2E8` | `#D6E4F8` |
| Deep sea | `#103644` | `#1C5460` | `#F4B07A` | `#7ECEC4` | `#0A222C` |
| Woodland dusk | `#2A3628` | `#3E523A` | `#E29A48` | `#C4D2AA` | `#1C241C` |
| Sherbet sky | `#FCEEE4` | `#F6D6CC` | `#EE8A92` | `#AAA8DA` | `#60465C` |

Each theme also picks a ground generator, which is where a lot of its character
comes from:

| Theme | Ground | Reads as |
| --- | --- | --- |
| Starry night | Texture, spots, 120 cells | Scattered distant stars |
| Deep sea | Texture, coral, 96 cells | Weed and reef |
| Woodland dusk | Ornament, scale, 22 mm | Overlapping leaves |
| Sherbet sky | Texture, spots, 72 cells | Soft clouds |

Starry night first used the `Dust` ornament, on the reasoning that speckle is
what a night sky is. At this cell size it renders as square digital noise — very
close to fragments of a QR code — and was swapped for fine spots.

## Seeing them

```bash
cargo run -p pattern-cli --example house-styles -- <out-dir>
```

Twelve PNGs, every theme against every idiom, drawn from code with no model
involved and nothing reaching the network. Edit the palettes at the top of
[the example](../crates/pattern-cli/examples/house-styles.rs) and re-run.

The subjects — star, crescent, cloud, twinkle — are deliberately **shared
across all four themes**, which is why a moon turns up under the sea. They are
there to isolate the idiom dial, not to propose subject matter. Real themes
bring their own subjects.

## What this is not yet

The renders are a sketch of the *choice*, not the style guide itself. Still
missing, and still the gate on M7:

- The written prose for each idiom — line weight in millimetres, how shading
  and highlights work, level of abstraction, what a motif may not do.
- Roughly twenty reference pieces per idiom, spanning an animal, a vehicle, a
  food, a plant, a manufactured object and a figure. The spec's advice is to
  bootstrap these from generated candidates before commissioning anything, so
  the outlay stays at zero until the approach is shown to hold.
- A home in the app. `IngestParams` is per-ingest and a palette lives inside one
  document, so there is currently no object that says "this is the house style"
  and outlives a single file. That is the same gap the roadmap carries as
  *house palettes shared across documents*, and the style guide is what it was
  waiting for.
