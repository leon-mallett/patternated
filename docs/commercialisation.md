# Commercialisation

Two products over one engine, on opposite models.

- **Pattern Engine** — a free tool for anyone who makes repeat patterns. No
  revenue, deliberately.
- **Dreamwoven** — bespoke children's pyjamas, generated per order. The
  business.

An earlier version of this page described two consumer *brands* sharing an
engine, with Peel Club as the live one. That is gone; Peel Club is a reference
point and nothing more. The page is rewritten rather than annotated because the
model changed, not only the names.

## Why the engine is free

The category is crowded. Several tools already cover prompt-to-pattern, seam
fixing, colourways and separations, and selling into that against well-funded
competitors would be a weak bet with a small team. That argument has not
changed; what has changed is the conclusion drawn from it. Previously the
engine was justified because Peel Club needed it anyway. With Peel Club gone,
the honest options were to sell it badly, keep it private, or give it away.

Giving it away buys three things worth more than the revenue would be:

- **Real artwork through the pipeline.** The longest-standing weakness in this
  project is that almost nothing has been ingested that it did not draw itself,
  and nothing at all has gone to fabric and come back. Designers using it for
  free generate exactly that evidence, in files nobody here would have thought
  to make.
- **Credibility for Dreamwoven.** A bespoke product a parent has to trust is
  helped by an engine anybody can inspect.
- **Reputation**, which is the only one of the three that is not measurable
  and the only one that compounds.

### What "free" has to mean, specifically

This repository is **all rights reserved** today. Free is therefore a decision
with consequences, not a default to drift into, and there are three distinct
versions of it:

| Option | Designers get | A competitor gets |
| --- | --- | --- |
| Free binary, source closed | A tool | Nothing |
| Open, permissive (MIT / Apache-2.0) | A tool and the source | The right to build a closed Dreamwoven competitor on it |
| Open, copyleft (AGPL-3.0) | A tool and the source | The source, but must open anything they build on it |

**The recommendation is AGPL-3.0.** Captivated owns the copyright outright, so
it can continue using the engine inside Dreamwoven under its own terms — the
licence binds others, not the owner. That is the standard open-core position
and it is the only one of the three that is genuinely free to designers while
not handing the product's foundation to whoever wants to clone it.

The cost is real and should be stated: AGPL deters commercial adoption, and
some organisations ban it outright. That is acceptable here because the
audience is individual designers and the goal is evidence and credibility, not
enterprise uptake.

**Not yet decided. It needs Leon, and it needs deciding before anything
ships**, because a licence is very hard to take back.

### What shipping the engine actually involves

The desktop app, not the crates. Nobody is going to depend on `pattern-core`
from Cargo, and publishing crates commits to an API this project has
deliberately kept fluid.

1. A licence, applied to `LICENSE`, every `Cargo.toml` and the headers.
2. Signing and notarising for macOS — `~/Projects/MACOS_SIGNING_NOTARIZATION.md`.
3. Self-update on R2 — `~/Projects/TAURI_AUTO_UPDATER.md`.
4. A landing page on a custom domain — `~/Projects/CLOUDFLARE_WORKERS_DOMAINS.md`.
   Never a `*.pages.dev` as the canonical URL.
5. `docs/user-guide.md`, which does not exist. Everything written so far is for
   whoever builds this, not whoever uses it.
6. A `CHANGELOG.md` and a release process, neither of which exists.

Three things should be said plainly on the page rather than discovered:
determinism is verified on macOS and Linux but **not Windows**; there is **no
CMYK export** and will not be until a bureau supplies a profile; and the
shipped window is tested by hand rather than automatically.

## Dreamwoven is the business

Everything commercial lives here. The shape of it, and the order, is in
[Dreamwoven's own roadmap](../../Dreamwoven/docs/roadmap.md).

One gate dominates the rest and it is not an engineering one. **What children
like is largely licensed characters**, the intake is now free text, and nothing
in the pipeline notices. See [Licensing](./licensing.md#the-unresolved-one).
That needs a blocklist, a model-side check, and someone who does this properly
— before anything is sold, not before it scales.

## How the two relate

Work flows one way. Improvements to the engine reach Dreamwoven for nothing,
because it consumes the engine by path. Dreamwoven's own material — the prompt
catalogue, the order path, the intake — stays closed, and none of it belongs in
the engine anyway: the engine forbids network I/O, and that boundary is what
keeps the split honest rather than merely administrative.

Dreamwoven funds the engine by needing it. That is a weaker guarantee than Peel
Club was, and the risk is stated rather than hidden: if Dreamwoven does not
work, the engine has no consumer and its development stops being justified by
anything but reputation.


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
