# Roadmap

Milestones from the [technical spec](./pattern-engine-spec.md). Each ends in
something usable, and nothing here requires the milestone after it. An item
becomes a [Features](./features.md) entry when it is built, and disappears from
this page.

**M1 to M5 are complete.** Their contents are in [Features](./features.md).

**The spec directs a stop and reassessment here**, on the grounds that a working
tool producing one good range answers the product question better than more
planning. Everything below is explicitly optional.

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

**Mockups are built** — see [Features](./features.md#garment-mockups).

**Tracking detection was attempted and is not built.** Two approaches were
measured against real output and neither separates a corridored pattern from a
clean one; both were removed rather than shipped behind a threshold that does
not discriminate. The numbers and the likely next approach are in
[Decisions](./decisions.md#not-shipping-tracking-detection-having-built-it-twice).


What remains is corridor detection with automatic reseed. It is a convenience
for Peel Club and a prerequisite for Dreamwoven, so it wants building properly
rather than as a warning badge — which is why a version that could not
discriminate was removed rather than shipped.

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

- **Cross-platform determinism is verified on two platforms, not three.** CI
  runs the suite on Linux x86-64 and development happens on macOS arm64; the
  golden images agree as identical bytes, rotation included. Windows is
  untested. See
  [Test strategy](./test-strategy.md#what-is-deliberately-not-tested).
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
- **Tracking is measured now, but as a ranking rather than a gate.** The third
  attempt discriminates where two did not, and the app warns on the worst few
  per cent; it catches roughly two in three at the level a person would reject
  and is blind to faint tracking. Making it the blocking gate M7 needs means
  carrying the radial dimension as well as the angular one, and a corpus of
  real patterns labelled by eye rather than synthetic pulls. See
  [Decisions](./decisions.md#tracking-detection-at-the-third-attempt-as-a-ranking-and-not-a-gate).
