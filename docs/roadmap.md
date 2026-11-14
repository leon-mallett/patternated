# Roadmap

Milestones from the [technical spec](./pattern-engine-spec.md). Each ends in
something usable, and nothing here requires the milestone after it. An item
becomes a [Features](./features.md) entry when it is built, and disappears from
this page.

**M1 to M5 are complete.** Their contents are in [Features](./features.md).

**The spec directs a stop and reassessment here**, on the grounds that a working
tool producing one good range answers the product question better than more
planning. That reassessment has happened — see below.

## Dreamwoven is the target (2026-09-29)

The spec sequences the work Peel Club first, on the grounds that it was the real
business and Dreamwoven the concept. That is no longer the case: **Peel Club is
a reference point, not a destination.** Everything the engine does is now aimed
at Dreamwoven.

The spec is left intact as the record of why things were built the way they
were. This page supersedes its sequencing, and the differences are these:

- **The fabric gate survives, in a smaller form.** Its purpose was never the
  brand — it was not debugging generation, style transfer and tiling at the same
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
  always did — but nothing is waiting on it, and the colour-count gate loses
  most of its force with it, a DTG bureau's slot limit being effectively
  unbounded.
- **The unattended order path is the largest gap**, and no milestone names it.
  See M7.

## M6 — Mockups and tracking detection

**Mockups are built** — see [Features](./features.md#garment-mockups).

**Tracking detection was attempted and is not built.** Two approaches were
measured against real output and neither separates a corridored pattern from a
clean one; both were removed rather than shipped behind a threshold that does
not discriminate. The numbers and the likely next approach are in
[Decisions](./decisions.md#not-shipping-tracking-detection-having-built-it-twice).


What remains is corridor detection with automatic reseed. With no human
approving an order it is simply required, which is why a version that could not
discriminate was removed rather than shipped behind a threshold.

## M7 — Dreamwoven pipeline

Generation behind a trait, a style adapter, blocking quality gates, and IP
filtering.

**Garment-size-driven scale is built** — see
[Features](./features.md#a-repeat-is-for-a-garment). What remains of it is
mapping an age to a size automatically rather than a person choosing.

### The order path does not exist

Not a gate and not a gap in a gate — an absence. The product is that a parent
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

Not a style guide. The spec asks for one — a written description of Dreamwoven's
single house illustration style, plus roughly twenty commissioned reference
pieces to condition a model on — and that was right while the brand had one
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
portability argument survives the change intact — it is simply now a property
of each option on the menu rather than of the brand. Flat vector and bold
outline generalise across unlike subjects; fine linework and watercolour
collapse. Expect some offered styles to fail that test and be withdrawn.

The twenty commissioned pieces are not obsolete so much as **unproven as a
need**. Reference-image conditioning wants an exemplar per style, not twenty,
and a current model may hold a named style from text alone. Settle it by
trying, not by commissioning.

## Somewhere to download it from

The engine is AGPL and nobody can get it. The repository is private, there are
no tags and no releases, so "open source" is currently a licence file and an
intention. Four things stand between here and a download page that is not
embarrassing, and only the last is the website.

**1 · The repository goes public, and pull requests close the same day.**
Not sequential steps — the same step. Sole ownership is what lets Dreamwoven
statically link an AGPL engine, and the first merged patch ends that
permanently. [`CONTRIBUTING.md`](../CONTRIBUTING.md) already says issues yes,
code no; GitHub needs to enforce it rather than the file asking politely. See
[Licensing](./licensing.md#the-thing-that-breaks-it).

**2 · Signed builds, or the download does more harm than no download.**
An unsigned `.dmg` meets Gatekeeper on every machine it lands on, and an
unsigned `.exe` meets SmartScreen. Both read as "this is not safe" to exactly
the person who was about to try it. macOS goes through the Captivated Apple
account (`~/Projects/MACOS_SIGNING_NOTARIZATION.md`); Windows goes through
Azure Artifact Signing on the release VM (`~/Projects/CODE-SIGNING.md`). These
do not overlap and both are needed.

**3 · A tagged release, with the installers on R2.** Bucket
`patternated-releases`, following the convention already in use for Tauri
releases — one bucket per app, Captivated-owned domain, no egress fees. The
signed self-updater (`~/Projects/TAURI_AUTO_UPDATER.md`) can come later; it
reads from the same place, so nothing has to move.

**4 · The site.** `patternated.com`, registered 2026-11-14 through Cloudflare
Registrar, with `patternated.app` held alongside it. Astro on Cloudflare Pages,
which is the house archetype.

It earns its existence for one reason: **the output is visual and a README
cannot tile.** A page can show a repeat actually repeating, at true scale, next
to the motifs it was built from — which is the entire pitch and is unviewable
on a code host. Everything else the site needs is small: what it is, the three
commands that build it, the installers, and an unmissable line saying issues
are welcome and patches are not. Say that on the page and not only in the
repository, or it gets discovered one refused pull request at a time.

**One piece of timing.** The newly-registered-domain flag is purely
time-based — roughly two to four weeks, cleared by nothing but waiting, and it
is what sinkholed `captivated.online` on Leon's own home network. The clock
started 2026-11-14, so it has run out around mid-December, well before any of
the three items above are done. Nothing to schedule around. See
`~/Projects/NEW_DOMAIN_REPUTATION.md`.

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
  artwork *is* the subject. The fix is a decision rather than a patch — widen
  the view to the content, or keep clipping and raise a `Note` — so it is
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
  an illustrator's exports bring — very large dimensions, embedded profiles,
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
  environment — Screen Recording and Accessibility are granted to the terminal
  now, so a screenshot-and-click pass is possible and was used to confirm the
  grown ground end to end — but nothing does it repeatably.
  `~/Projects/AI_UI_TEST_AUTOMATION.md` and `DESKTOP_APP_CONTROL.md` describe
  the house approach when the full-stack tier becomes worth its upkeep.
- **House palettes shared across documents.** Colourways live in one document,
  which covers a range; it does not yet cover several ranges agreeing with each
  other. Small, and worth doing when there is a second range.
- **No CMYK profile ships with the tool**, deliberately — see
  [colour management](./features.md#colour-management). Until a bureau supplies
  one, a CMYK export cannot be produced at all, which is the honest state of
  affairs rather than a gap to paper over.
- **Tracking rejects a layout but does not block an order.** `scatter_reseeding`
  now throws away a layout that tracks too hard and tries another seed, which
  is the spec's reseed-and-retry for the one gate that can be answered that
  way. What it still is not is a *blocking* gate: a run that never clears the
  bar returns its best attempt with `acceptable` false rather than refusing,
  because refusing an order outright needs a confidence the measure has not
  earned. The remainder of the original debt stands —
- **Tracking is measured now, but as a ranking rather than a gate.** The third
  attempt discriminates where two did not, and the app warns on the worst few
  per cent; it catches roughly two in three at the level a person would reject
  and is blind to faint tracking. Making it the blocking gate M7 needs means
  carrying the radial dimension as well as the angular one, and a corpus of
  real patterns labelled by eye rather than synthetic pulls. See
  [Decisions](./decisions.md#tracking-detection-at-the-third-attempt-as-a-ranking-and-not-a-gate).
