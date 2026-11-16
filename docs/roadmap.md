# Roadmap

Milestones from the [technical spec](./pattern-engine-spec.md). Each ends in
something usable, and nothing here requires the milestone after it. An item
becomes a [Features](./features.md) entry when it is built, and disappears from
this page.

*A few entries below cite notes at `~/Projects/…`. Those are Captivated's own
operational notes, not part of this repository and not public. They are left in
because the reason for a decision is more useful than a tidy page, but nothing
here depends on reading them.*

**M1 to M5 are complete.** Their contents are in [Features](./features.md).

**The spec directs a stop and reassessment here**, on the grounds that a working
tool producing one good range answers the product question better than more
planning. That reassessment has happened; see below.

## Dreamwoven is the target (2026-09-29)

The spec sequences the work Peel Club first, on the grounds that it was the real
business and Dreamwoven the concept. That is no longer the case: **Peel Club is
a reference point, not a destination.** Everything the engine does is now aimed
at Dreamwoven.

The spec is left intact as the record of why things were built the way they
were. This page supersedes its sequencing, and the differences are these:

- **The fabric gate survives, in a smaller form.** Its purpose was never the
  brand. It was not debugging generation, style transfer and tiling at the same
  time. What replaces "a Peel Club range that came back right" is **one test
  metre**: a handful of motifs in roughly the target idiom, through the engine,
  onto jersey at a real bureau, looked at by eye. Same risk retired, no business
  required.
- **Tracking detection is simply required.** It used to be a convenience for one
  brand and a prerequisite for the other. Only the second half remains, because
  no human approves an order.
- **Screen printing is shelved.** Bespoke, one-garment-per-order pyjamas are
  direct-to-garment or dye-sublimation; a screen setup needs a run length that
  this product does not have. Per-colour separations remain a rendering pass
  over data the engine already holds, so the door stays open at the same cost it
  always did, but nothing is waiting on it, and the colour-count gate loses
  most of its force with it, a DTG bureau's slot limit being effectively
  unbounded.
- **The unattended order path is the largest gap**, and no milestone names it.
  See M7.

## M6: Mockups and tracking detection

**Mockups are built**; see [Features](./features.md#garment-mockups).

**Tracking detection was attempted and is not built.** Two approaches were
measured against real output and neither separates a corridored pattern from a
clean one; both were removed rather than shipped behind a threshold that does
not discriminate. The numbers and the likely next approach are in
[Decisions](./decisions.md#not-shipping-tracking-detection-having-built-it-twice).


What remains is corridor detection with automatic reseed. With no human
approving an order it is simply required, which is why a version that could not
discriminate was removed rather than shipped behind a threshold.

## M7: Dreamwoven pipeline

Generation behind a trait, a style adapter, blocking quality gates, and IP
filtering.

**Garment-size-driven scale is built**; see
[Features](./features.md#a-repeat-is-for-a-garment). What remains of it is
mapping an age to a size automatically rather than a person choosing.

### The order path does not exist

Not a gate and not a gap in a gate, but an absence. The product is that a parent
names a child's age and a few favourite things and a print file comes out.
Everything built so far is a desktop app a person drives, plus a CLI that
renders a document somebody has already made. Nothing turns a brief into a
finished pattern without a person in the middle.

The [quality gates](./pattern-engine-spec.md#unattended-quality-gates) describe
what checks that loop; three of the five are measured today and none of them
blocks, because there is nothing yet for them to block. Building the loop is the
larger job and the one that makes the gates mean anything.

### Intellectual property is the top commercial risk

Not a footnote. The spec expects licensed characters on a large share of orders
rather than as an edge case, and rates **guided categories over a free-text box**
as the single most effective control. That is an intake design decision, so it
wants taking before the intake is written rather than retrofitted to it.

### What M7 is actually gated on

Not a style guide. The spec asks for one, meaning a written description of
Dreamwoven's single house illustration style plus roughly twenty commissioned
reference pieces to condition a model on, and that was right while the brand had one
fixed style for everything it printed.

It no longer does. Style is **a choice the buyer makes**, expressed as a clause
in the generation prompt, so there is no house style to write down and nothing
to commission twenty pieces *of*. See
[From brief to pattern](./from-brief-to-pattern.md#style-is-set-in-the-prompt-not-in-the-engine).

What replaces it is a **prompt catalogue**, and it is a much smaller thing:

- the styles on offer, by the name a parent sees;
- for each, the prompt text that reliably produces it;
- for each, one example image, so a buyer knows what they are choosing.

Drafting it is an afternoon. *Validating* it needs generation, because the only
question that matters is empirical: does this prompt hold its look across a
dinosaur, a monster truck and an ice cream in the same garment? The spec's
portability argument survives the change intact. It is simply now a property
of each option on the menu rather than of the brand. Flat vector and bold
outline generalise across unlike subjects; fine linework and watercolour
collapse. Expect some offered styles to fail that test and be withdrawn.

The twenty commissioned pieces are not obsolete so much as **unproven as a
need**. Reference-image conditioning wants an exemplar per style, not twenty,
and a current model may hold a named style from text alone. Settle it by
trying, not by commissioning.

## Somewhere to download it from

The engine is AGPL and, until 2026-11-16, nobody could get it. The repository
is public now; there are still no tags and no releases, so a download page is
two gates away. The website is built but not deployed, and it went first
because it was the only gate that waited on nobody.

**1 · The repository goes public, and pull requests close the same day. Done,
2026-11-16**, at
[github.com/leon-mallett/patternated](https://github.com/leon-mallett/patternated).
Not sequential steps but the same step. Sole ownership is what lets Dreamwoven
statically link an AGPL engine, and the first merged patch ends that
permanently. See [Licensing](./licensing.md#the-thing-that-breaks-it).

**This gate expected GitHub to enforce it. GitHub cannot.** There is no setting
that refuses pull requests: anyone who can see a public repository can fork it
and open one, and a ruleset governs who may *push*, not who may *ask*. So the
mechanism is the file asking politely, and somebody closing promptly, and that
is the whole of it. What was added instead is a pull request template, which
puts the refusal and the reasoning in front of a contributor at the moment they
open one rather than after they have waited for a review. The closing is still
a commitment a person keeps.

Also done at the same time, because a public repository is judged on them:
the repository was renamed from `pattern-engine` to match the product, a
[`SECURITY.md`](../SECURITY.md) now gives a private reporting route, GitHub's
private vulnerability reporting is switched on, and a pre-publication audit
fixed three things a stranger would have hit on day one. Those are in the
commit log around `e77c7f1`.

**2 · Signed builds, or the download does more harm than no download.**
An unsigned `.dmg` meets Gatekeeper on every machine it lands on, and an
unsigned `.exe` meets SmartScreen. Both read as "this is not safe" to exactly
the person who was about to try it. macOS goes through the Captivated Apple
account (`~/Projects/MACOS_SIGNING_NOTARIZATION.md`); Windows goes through
Azure Artifact Signing on the release VM (`~/Projects/CODE-SIGNING.md`). These
do not overlap and both are needed.

**3 · A tagged release, with the installers on R2.** Bucket
`patternated-releases`, following the convention already in use for Tauri
releases: one bucket per app, Captivated-owned domain, no egress fees. The
signed self-updater (`~/Projects/TAURI_AUTO_UPDATER.md`) can come later; it
reads from the same place, so nothing has to move.

**4 · The site. Built 2026-11-16, out of order, and not yet deployed.**
`patternated.com`, registered 2026-11-14 through Cloudflare Registrar, with
`patternated.app` held alongside it. Astro on Cloudflare Pages, which is the
house archetype. The repository is
[leon-mallett/patternated-website](https://github.com/leon-mallett/patternated-website),
private, and its README is the live statement of what the site still owes.

It earns its existence for one reason: **the output is visual and a README
cannot tile.** Every band on the page is one image, repeated by the reader's
browser with `background-repeat`, so a seam would show a dozen times over on
one screen and no CSS would hide it. The page cannot claim a seamlessness the
engine does not have, which is a stronger thing than a screenshot.

**This gate said the site comes last, and it did not.** Gates 2 and 3 need the
Captivated Apple account and the Azure release VM, so they wait on a person;
the site needed neither. The domain was also registered and serving nothing,
and a domain with no records accrues age without accruing reputation. So the
page was built without the part that is not ready: it says in as many words
that there is nothing to download and why, and the download section lands with
gate 3. A download page with no download would have been worse than no page.

What it does carry: what the engine is, the three hard parts that do not show
up in a preview, three pictures that are **the same document with one field
changed** (`row_offset: 0.5` makes a brick, `col_offset: 0.5` makes a
half-drop), a build command a stranger can actually run against artwork in
this repository, and the refusal of patches with its reason. That last one
belongs on the page and not only here, or it gets discovered one refused pull
request at a time.

**What remains is a Cloudflare Pages project**, which needs the dashboard OAuth
handshake, and the two custom domains. Until then the security headers have
never been served by a real edge.

**One piece of timing.** The newly-registered-domain flag is purely
time-based, roughly two to four weeks, cleared by nothing but waiting, and it
is what sinkholed `captivated.online` on Leon's own home network. The clock
started 2026-11-14 and runs out around mid-December. Having something served
before then is the reason the site went first.
See `~/Projects/NEW_DOMAIN_REPUTATION.md`.

## Carried debts

Not milestones, but not forgotten.

- **SVG artwork outside its own view box is silently dropped.** `read_svg`
  keeps a shape whose bounds fall outside the view, but the view is what gets
  rasterised, so the shape is clipped away at render with no note. Measured on
  a two-disc test file: one disc inside the box, one at x 150–210 against a
  0–100 view, both kept at import and only one printable. `palette_sample`
  weights colours by bounding-box area, so the lost shape can also pull a
  colour into the derived palette that never reaches the cloth. Clipping is
  correct for a browser showing an SVG and wrong for a motif, where the
  artwork *is* the subject. The fix is a decision rather than a patch, between
  widening the view to the content and keeping the clipping but raising a `Note`, so it is
  recorded rather than guessed at.
- **A stretch repeat is not compensated.** Jersey is the obvious pyjama
  substrate and it stretches, which distorts a print; the repeat may need
  pre-distorting before it goes to the bureau. Recorded in the spec's prose and
  nowhere else until now. How much, and in which axis, is a question for the
  test metre rather than for a guess.

- **Cross-platform determinism is verified on two platforms, not three.** CI
  runs the suite on Linux x86-64 and development happens on macOS arm64; the
  golden images agree as identical bytes, rotation included. Windows is
  untested. See
  [Test strategy](./test-strategy.md#what-is-deliberately-not-tested).
- **Circular, wave, ogee and damask arrangements.** Asked for once and
  deliberately deferred: they are the family the spec places in v2, and they are
  neither a lattice nor a grid, so they need a motif frame rather than another
  preset.
- **~~Real Peel Club files have not been through the pipeline.~~** Retired on
  2026-09-29 when Peel Club stopped being a destination. The debt was about what
  an illustrator's exports bring: very large dimensions, embedded profiles,
  CMYK, semi-transparent texture layers, baked-in drop shadows. Dreamwoven's
  inputs are model outputs on flat backgrounds, which is the easy route through
  ingest and is already well covered. Left visible rather than deleted so that a
  later reader can tell it was answered rather than forgotten.
- **Quantisation uses an approximate colour metric.** Redmean rather than Oklab
  or CIELAB, because a perceptual space needs a cube root and `f32`
  transcendentals carry no cross-platform bit-exactness guarantee. Ingest runs
  once and stores its result, so the stakes are lower here than in placement;
  worth revisiting if real artwork quantises badly.
- **No background-removal model.** Paper measurement and alpha only. Fine for
  digital artwork and for generated motifs on flat backgrounds, which is what
  both brands actually supply; it will not cope with a photographed subject on a
  busy background.
- **A hard-edged cast shadow across a page is not handled.** The illumination
  estimate is a plane, which models gradual falloff rather than a step, so the
  shadow survives as motif. Detected and warned about rather than silently
  wrong. Not on either brand's path today.
- **The app's window is tested at the component tier, not the shipped one.**
  Thirty-three component tests drive the real interface against a faked engine,
  and sixty-odd Rust tests cover the layer beneath the commands. What is not
  automated is the shipped window: the app has been driven by hand from this
  environment, since Screen Recording and Accessibility are granted to the
  terminal now, so a screenshot-and-click pass is possible and was used to
  confirm the grown ground end to end, but nothing does it repeatably.
  `~/Projects/AI_UI_TEST_AUTOMATION.md` and `DESKTOP_APP_CONTROL.md` describe
  the house approach when the full-stack tier becomes worth its upkeep.
- **House palettes shared across documents.** Colourways live in one document,
  which covers a range; it does not yet cover several ranges agreeing with each
  other. Small, and worth doing when there is a second range.
- **No CMYK profile ships with the tool**, deliberately; see
  [colour management](./features.md#colour-management). Until a bureau supplies
  one, a CMYK export cannot be produced at all, which is the honest state of
  affairs rather than a gap to paper over.
- **Tracking rejects a layout but does not block an order.** `scatter_reseeding`
  now throws away a layout that tracks too hard and tries another seed, which
  is the spec's reseed-and-retry for the one gate that can be answered that
  way. What it still is not is a *blocking* gate: a run that never clears the
  bar returns its best attempt with `acceptable` false rather than refusing,
  because refusing an order outright needs a confidence the measure has not
  earned. The rest of the original debt stands, as the next entry records.
- **Tracking is measured now, but as a ranking rather than a gate.** The third
  attempt discriminates where two did not, and the app warns on the worst few
  per cent; it catches roughly two in three at the level a person would reject
  and is blind to faint tracking. Making it the blocking gate M7 needs means
  carrying the radial dimension as well as the angular one, and a corpus of
  real patterns labelled by eye rather than synthetic pulls. See
  [Decisions](./decisions.md#tracking-detection-at-the-third-attempt-as-a-ranking-and-not-a-gate).
