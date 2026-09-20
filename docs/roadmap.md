# Roadmap

Milestones from the [technical spec](./pattern-engine-spec.md). Each ends in
something usable, and nothing here requires the milestone after it. An item
becomes a [Features](./features.md) entry when it is built, and disappears from
this page.

**M1, M2 and M3 are complete.** Their contents are in [Features](./features.md).

## M4 — Tauri shell

Tauri 2 with React and TypeScript. Live three-by-three preview, parameter
controls, motif picker, seed reroll. The preview being instant is what a
designer will judge the tool on.

Test: a designer produces a print they like without touching a config file.

**The spec directs a stop and reassessment here** rather than continuing
automatically, on the grounds that a working tool producing one good range
answers the product question better than more planning.

## M5 — Colourways and export

Palette management, named house palettes, live colour-count display with a
settable maximum, CMYK and ICC export, per-colour separations, and file naming
that carries the repeat size and resolution.

Colour management is the one to be careful with: RGB to CMYK through a real ICC
profile rather than a formula, with soft-proofing. "It looked different on
screen" is the commonest complaint in fabric printing and is almost always an
unmanaged conversion.

Blocked on an [open question](./commercialisation.md#open-questions): whether
Peel Club prints digitally or by screen, and the target bureau's spec.

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
- **The remaining three lattices** — brick, quarter-drop, mirror — are a
  constructor each, unbuilt because neither M1 nor M2 calls for them. Mirror is
  more than a constructor: it needs a reflected 2 × 2 super-tile.
- **Nothing has met a real illustration.** Every test and every example uses
  artwork drawn in code. The pipeline is built against synthetic input by an
  explicit decision, taken to avoid holding up the project while real Peel Club
  artwork is gathered, and the parts most likely to need adjusting when it
  arrives are the matting threshold, the feather width, and whether redmean
  quantisation is good enough. M3's own acceptance test — the existing
  illustrations go in unmodified and come out clean and recolourable — cannot be
  run yet.
- **Quantisation uses an approximate colour metric.** Redmean rather than Oklab
  or CIELAB, because a perceptual space needs a cube root and `f32`
  transcendentals carry no cross-platform bit-exactness guarantee. Ingest runs
  once and stores its result, so the stakes are lower here than in placement;
  worth revisiting if real artwork quantises badly.
- **No background-removal model.** Luminance thresholding only. Fine for
  illustration on paper and for generated motifs on flat backgrounds, which is
  what the spec says to expect; it will not cope with a photographed subject or
  a busy background.
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
