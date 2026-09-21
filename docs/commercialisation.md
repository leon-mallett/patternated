# Commercialisation

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

## The open question: the bureau's spec

Still unanswered, and it is the one that should be settled before the export
layer is built rather than after, because a wrong assumption here is invisible
until a sample arrives.

Concretely, what to ask whoever will print the fabric:

1. **Resolution.** What DPI do they want artwork at? 150 is common for fabric,
   some want 300. This decides the pixel dimensions of every export.
2. **Colour space, and whose profile.** RGB or CMYK? If RGB, which profile —
   sRGB or Adobe RGB? If CMYK, do they supply their own press profile, and can
   they send it? "Convert to CMYK" without a named profile is the single most
   common cause of a print that does not match the screen.
3. **File format.** PNG, or TIFF? If TIFF, what compression, and flattened or
   layered?
4. **What exactly they want in the file.** Just the one tile, ready to be
   stepped and repeated by them? Or a pre-repeated sheet? And do they want
   bleed around the tile, or the tile exactly?
5. **How they learn the physical size.** Do they read it from the embedded DPI,
   or do you state the repeat size separately on the order? Both, ideally — but
   it is worth knowing which one they actually act on.
6. **Maximum printable width**, and the fabric roll width. This constrains how
   large a repeat can sensibly be, and a repeat wider than the roll is a design
   that cannot be printed.

Answers to 1, 2 and 3 are what M5 is built against. Answers to 4, 5 and 6 are
what the export dialog needs to say.

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
