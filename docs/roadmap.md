# Roadmap

Milestones from the [technical spec](./pattern-engine-spec.md). Each ends in
something usable, and nothing here requires the milestone after it. An item
becomes a [Features](./features.md) entry when it is built, and disappears from
this page.

**M1 to M4 are complete.** Their contents are in [Features](./features.md).

**The spec directs a stop and reassessment here**, on the grounds that a working
tool producing one good range answers the product question better than more
planning. Everything below is explicitly optional.

## M5 — Colourways and export

Palette management in the app, named house palettes, colour management through
a real ICC profile, and file naming that carries the repeat size and resolution.

Colour management is the one to be careful with: a conversion through a real
profile rather than a formula, with soft-proofing. "It looked different on
screen" is the commonest complaint in fabric printing and is almost always an
unmanaged conversion.

**Narrowed by the print-route decision.** Peel Club prints digitally, so there
is no per-colour cost and no colour-count ceiling to enforce. Per-colour
separations have moved out of this milestone to the list below, because they
only mean anything for screen printing and building them now would be a partial
implementation of a route nobody has chosen.

**The export half is built** — see [Features](./features.md#export). TIFF and
PNG at any resolution, tile or pre-repeated sheet, physical size and roll-width
verdict before the file dialog, and filenames that say what the file is.

What remains:

- **Colour management in the tool**, so nobody needs Photoshop to make a CMYK
  file. `moxcms` does ICC transforms between RGB and CMYK through an XYZ or Lab
  connection space, in pure Rust — which removes the C binding this milestone
  was expected to need. Best done once a bureau has said what they want, since
  the bundled profiles are offset litho ones and a textile press is not.
- **Openly licensed profiles only**: `sRGB2014` for RGB, and PSO Coated or
  GRACoL for CMYK, all redistributable without restriction. Loading a bureau's
  own profile must be easy, because those defaults are offset litho profiles
  and a textile press is not one.
- **Physical size shown in the interface**, with a gentle warning past a
  standard 1.6 m roll and a firm one past 3.2 m. Both adjustable.
- **Palette management**: named house palettes, and editing a colourway without
  hand-writing JSON.

## Screen printing, if a range goes that way

Not a milestone, because nothing has chosen it. Recorded so the door stays
visibly open.

Per-colour separations are one file per palette slot, which is a rendering pass
over data the engine already holds — the slot model was built for colourways
and happens to be exactly what separations need. Alongside it: enforcing a
maximum colour count in the palette generator, and showing a designer when a
print has drifted past what screen printing can do.

Cheap to add when wanted. Expensive to have guessed at now, because the
constraints that shape it come from a specific printer nobody has picked.

## M6 — Mockups and tracking detection

Garment preview at true scale, and Radon-based corridor detection over a
low-resolution three-by-three mask with automatic reseed.

Both look cosmetic and are not. Seeing a print at true scale on a shirt is how a
scale error gets caught before fabric is bought. Tracking detection is a
convenience for Peel Club and a prerequisite for Dreamwoven, so it is built
properly here rather than as a warning badge.

## M7 — Dreamwoven pipeline

Generation behind a trait, a style adapter, blocking quality gates, IP
filtering, and garment-size-driven scale.

Not to be started until M1–M6 have produced a Peel Club range that went to
fabric and came back right. Building it earlier means debugging generation,
style transfer and tiling simultaneously.

Gated on two things that do not exist: a written style guide — line weight, fill
treatment, level of abstraction, palette rules — and roughly twenty commissioned
reference pieces spanning an animal, a vehicle, a food, a plant, a manufactured
object and a figure. The written guide is the more valuable of the two, since it
governs prompt construction, vectorisation and palette rules as well as the
adapter, and it outlives whatever generation model is current.

Bootstrap the style with generated candidate exemplars before commissioning
anything; that keeps outlay at zero until the approach is shown to hold.

## Carried debts

Not milestones, but not forgotten.

- **Cross-platform determinism is unverified.** See
  [Test strategy](./test-strategy.md#what-is-deliberately-not-tested). Needs
  deciding before anyone relies on reproducing a print from a seed months later.
- **Mirror repeat is unbuilt.** Brick and quarter-drop landed with the classic
  layouts; mirror is more than a constructor, needing a reflected 2 × 2
  super-tile, and nothing has asked for it yet.
- **Circular, wave, ogee and damask arrangements.** Asked for once and
  deliberately deferred: they are the family the spec places in v2, and they are
  neither a lattice nor a grid, so they need a motif frame rather than another
  preset.
- **Real Peel Club files have not been through the pipeline.** Downgraded from
  the largest debt on 2026-09-20, when the artwork was confirmed to be
  **digital rather than scanned**: that is the easy route through ingest and is
  well covered by artwork of the same kind. What real exports may still bring is
  very large dimensions, embedded colour profiles, CMYK, semi-transparent
  texture layers, and baked-in drop shadows. M3's stated acceptance test still
  cannot be ticked until the files exist.
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
- **The app's window is untested.** Eleven tests cover the layer beneath the
  commands — what a slider actually does — but nothing drives the UI itself.
  The app was built and run successfully; its window could not be inspected
  from this environment because Screen Recording and Accessibility permissions
  are not granted to the terminal. `~/Projects/AI_UI_TEST_AUTOMATION.md` and
  `DESKTOP_APP_CONTROL.md` describe the house approach when that becomes worth
  doing.
- **No palette editing in the app.** Deliberate: that is M5.
- **Whether a scatter tracks is still judged by eye.** M2 removes clumps and
  bald patches, both of which are measurable and tested. Corridors are neither,
  until [M6](./roadmap.md#m6--mockups-and-tracking-detection) builds the Radon
  check. Until then "no visible corridors" is an opinion, and it should be
  described as one.
- **Scatter is brute-force**, checking each candidate against every accepted
  sample. Milliseconds at textile counts; the usual spatial grid indexes the
  tile rectangle, which is the wrong neighbourhood on an oblique lattice, so a
  correct one would have to be built in lattice coordinates. Worth doing only if
  M4's live preview turns out to need it.
