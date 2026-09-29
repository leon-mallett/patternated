# Commercialisation

> **2026-09-29 — Dreamwoven is the target.** Peel Club is a reference point and
> not a destination, so the two-consumer framing below no longer describes the
> intent. The page is kept because most of its reasoning does not depend on
> which brand ships — the print route, the licensing position, the nightwear
> rules and the case for selling a garment rather than software all stand.
>
> One argument does **not** survive, and it is the load-bearing one: *"Peel Club
> needs the engine regardless of whether anyone else would buy it, which is what
> makes building it straightforwardly worthwhile."* With Peel Club out, the
> engine has no guaranteed consumer, and its worth now rests entirely on
> Dreamwoven working. That is a real change in the risk and is recorded here
> rather than quietly dropped.
>
> The stop-and-reassess paragraph below has been answered; see
> [Roadmap](./roadmap.md#dreamwoven-is-the-target-2026-09-29).

The engine is shared infrastructure under two consumer brands. **The product
sold is a garment, not software.**

That framing is deliberate and load-bearing. The AI pattern-generator category
is crowded — several tools already cover prompt-to-pattern, seam fixing,
colourways and separations — so selling this to pattern designers would be a
weak bet against well-funded competitors. Peel Club needs the engine regardless
of whether anyone else would buy it, which is what makes building it
straightforwardly worthwhile.

## The two consumers

**Peel Club** — adult apparel, fruit prints, one illustrator's existing artwork.
Live today. All of its constraints are engineering ones. Style coherence comes
free, because one person draws everything.

**Dreamwoven** — bespoke children's pyjamas, motifs generated per order. A
concept. The better business of the two if it works: a gift product with real
emotional pull and a buyer who is rarely the wearer. Also the harder engineering
problem, because nobody approves a print before it is made, so every quality
check becomes automatic and blocking.

Dreamwoven carries two risks that can stop the business rather than merely
degrade the output — character IP, covered in [Licensing](./licensing.md#the-unresolved-one),
and UK nightwear regulation, below.

Peel Club prints digitally; see [Print route](#print-route--decided).

## Scope discipline as a commercial position

The spec's instruction to stop and reassess after **M4, which is now
complete**, is a commercial judgement, not an engineering one: a working tool that produces one good Peel Club range answers the product
question better than any amount of further planning. Everything after M4 is
explicitly optional, and the build is scoped so the first outcome alone is worth
having.

## Print route — decided

**Digital printing first**, confirmed 21 September 2026. Screen printing stays
on the roadmap as an option rather than a requirement.

What follows from that, and it is mostly good news for the build:

- **The colour-count limit does not bite.** Digital printing has no per-colour
  cost, so a pattern resolving to twenty slots is as printable as one resolving
  to five. The count is still displayed, because it is the number that decides
  whether a design *could* move to screen later, and because a print that
  quietly needs thirty colours is usually a print whose palette wants looking
  at.
- **Per-colour separations leave M5.** They are only meaningful for screen
  printing, and building them now would be a partial implementation of a route
  nobody has chosen. They become a roadmap item, to be built when a range is
  actually going to screen.
- **M5 narrows to what digital needs**: palette management, colour management
  through a real ICC profile, and correct file naming.

Keeping the door open costs almost nothing, because the palette-slot model is
already exactly what separations need — one file per slot is a rendering pass
over data that already exists.

## Output spec — decided

Settled 21 September 2026. These are the requirements M5 is built against.

**Resolution: both 150 and 300 DPI, and anything else.** The engine already
renders at an arbitrary resolution, so this is an export choice rather than a
constraint. The reasoning behind it is the more important part: *we cannot
control what another printer asks for*, so the export layer is built to be told
rather than to assume.

**Colour: no proprietary profiles.** sRGB rather than Adobe RGB, and CMYK
through an openly licensed profile. The conversion happens in this tool — a
designer should not need Photoshop or Illustrator to produce a CMYK file.

**Format: TIFF definitely, PNG probably.** Minimal compression, flattened, no
layers.

**Two export shapes, as an option:** one large pre-repeated image that works
as-is for a printer with no tiling of its own, or one small tileable image for
printers that step and repeat themselves.

**The physical size is shown in the interface**, not left to be worked out. If
a design comes to 50 cm and someone needs 1.5 m, they should be able to see
that and decide between enlarging the repeat and exporting the tileable version
for the printer to handle.

### Which profiles, specifically

Researched rather than assumed, because a profile bundled into software that
might one day be public is a licensing decision as much as a colour one.

| Profile | Covers | Licence |
| --- | --- | --- |
| `sRGB2014.icc` | RGB | ICC. "Copied, distributed, embedded, made, used, and sold without restriction." |
| `PSO Coated` (ECI) | CMYK, European offset | ECI. "Used, embedded, exchanged, and shared without restriction." |
| `GRACoL` / `SWOP` / `CGATS21` | CMYK, US offset | IDEAlliance, X-Rite. Same unrestricted terms. |
| `XCMYK 2017` | CMYK, extended gamut | IDEAlliance / ColorLogic. Same unrestricted terms. |

Deliberately **not** used: `ISOcoated*` from Heidelberg, which is bundleable but
carries conditions about not charging extra for it — a needless complication
when equivalents are unrestricted. And anything from Adobe.

**The caveat that matters more than the list.** Every one of those is an
*offset litho* profile. Digital textile printing has a different gamut, and the
correct profile is the bureau's own. These are sensible defaults that let
someone produce a plausible CMYK file today; loading a supplied profile must be
easy, because that is what a real job will use.

### Print width

No single number, so this is a setting with defaults rather than a rule.

Standard apparel and home-textile printers run **1.6 to 1.8 m**, which is what
most fabric rolls are. Wide-format machines reach **3.2 m**, the conventional
start of "grand format", and industrial roll-to-roll goes to 5 m.

Erring high as instructed: warn gently above 1.6 m, since a repeat wider than a
standard roll is usually a mistake, and treat 3.2 m as the point where it is
almost certainly wrong. Both adjustable, because the real limit belongs to a
specific printer.

### One thing worth flagging

The reasoning given for resolution flexibility — that we cannot control what
other printers ask for, ahead of the app being commercialised or made public —
points somewhere the spec argues against. Its closing note is that selling this
to pattern designers is a weak bet in a crowded category, and that the strength
of the position is that the engine sits under two garment brands.

Nothing decided here conflicts with that: building an export layer that can be
*told* its target rather than assuming one is better engineering for a single
bureau too, and costs nothing extra. It is only worth noticing that a
"someone else might use this" requirement has entered the build, and that if it
grows, it is a strategy change rather than a feature.

## Regulation — UK children's nightwear

Not legal advice. Confirm with someone who does this properly before Dreamwoven
sells anything. The shape of it, from
[Business Companion](https://www.businesscompanion.info/en/quick-guides/product-safety/new-nightwear):

- Children's nightwear generally must meet the flammability performance
  requirements of BS 5722, but **pyjamas are among the listed exceptions**.
- Pyjamas must still be **labelled** to say whether the fabric meets those
  requirements. Where it does not, the label must read 'KEEP AWAY FROM FIRE' in
  red.
- **For online and mail-order sales**, that same wording must appear next to the
  listing inside a red-sided equilateral triangle. This is a constraint on the
  shop front, not only on the garment — worth knowing before anyone designs a
  product page.
- BS EN 14878 is recommended so suppliers meet General Product Safety
  Regulations obligations.
- UK REACH restricts certain azo dyes and flame retardants, which bears directly
  on a printed product. Raise it with the print bureau early.

One engineering consequence: jersey is the obvious pyjama substrate and it
stretches, which distorts a print. The repeat may need compensating before it
goes to the printer, which is a `pattern-io` concern nobody has scoped.
