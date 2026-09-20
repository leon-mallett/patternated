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

## Scope discipline as a commercial position

The spec's instruction to stop and reassess after
[M4](./roadmap.md#m4--tauri-shell) is a commercial judgement, not an engineering
one: a working tool that produces one good Peel Club range answers the product
question better than any amount of further planning. Everything after M4 is
explicitly optional, and the build is scoped so the first outcome alone is worth
having.

## Open questions

Both bite at [M5](./roadmap.md#m5--colourways-and-export) and neither blocks
anything before it. Both need answering before the export layer is built,
because the wrong assumption is invisible until a sample arrives.

- **Is Peel Club printing digitally or by screen?** This decides how hard the
  colour-count limit bites. Screen printing charges per colour and is simply
  unavailable past a certain count; digital has no such limit. It changes how
  the palette generator should behave and what the UI should warn about.
- **What is the target bureau's spec?** DPI expectations and colour handling
  vary, and the export layer should not be built against a guess.

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
