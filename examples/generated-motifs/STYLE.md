# Style brief for generated test motifs

A written style spec, not a mood board. The spec's own advice on commissioning —
write the style down explicitly first, then produce against it — is also what
makes several separate generations read as one illustrator, which is the point
here.

**This is a stand-in.** It is not Peel Club's style and these motifs are not
Peel Club assets. They exist to put real generated artwork through the ingest
pipeline. Nothing here should end up on a garment.

## The style

- **Flat vector illustration.** No gradients, no shading, no texture, no
  rendering. Colour blocks only.
- **No outline.** Shapes are defined by where one flat colour meets another. A
  single darker tone of the subject's own colour carries interior detail.
- **Geometric simplification.** The subject reduces to a handful of clean
  shapes. A viewer should read it instantly at 20 mm across.
- **Four to six flat colours** per motif, and no more.
- **One subject, centred**, occupying about 80% of a square canvas.
- **Pure white background**, nothing else in the frame.
- **No text, no shadows, no reflections, no props, no scene.**

Why this idiom: the spec rates flat vector with geometric simplification as the
most portable across unlike subjects, because a lemon and a pineapple reduce to
the same visual grammar. It is also the idiom that vectorises cleanly, which
matters for what these are being used to test.

## The prompt

Paste the style block verbatim each time, changing only the subject line. Ask
for PNG, square, at least 1024 px.

> Flat vector illustration of {SUBJECT}. Single centred subject on a pure white
> background, filling about 80% of a square frame. Completely flat colour
> blocks: no gradients, no shading, no texture, no outline. Shapes defined by
> colour meeting colour, with one darker tone of the subject's own colour for
> interior detail. Geometric simplification — clean, minimal, readable at a
> glance. Four to six flat colours only. No text, no shadow, no reflection, no
> background elements.

## The five subjects, and what each one is testing

Each is chosen to break something different. Generate all five; the boring ones
are as useful as the awkward ones.

| # | Subject line | What it stresses |
| --- | --- | --- |
| 1 | `a pair of cherries on a single stem with one leaf` | Baseline. Directly comparable to the synthetic motif the tests already use. |
| 2 | `a whole pineapple, upright, with a crown of leaves` | Vectorisation. A crosshatched body and spiky crown produce far more outlines than anything tested so far, and a tall aspect ratio exercises the longest-edge sizing. |
| 3 | `a single cross-section slice of lemon` | Quantisation against the ground. The pith is near-white, so on a cream ground it may quantise into slot 0 and vanish. This is the case I have reasoned about and never seen happen. |
| 4 | `a small sprig of blueberries with two leaves` | Low contrast and small forms. Dark fruit on white matts easily, but the berries are small and round — a good test of whether the speck filter drops things it should keep. |
| 5 | `a wedge of watermelon, cut side facing forward` | Slot pressure. Flesh, rind, inner rind and seeds want four or five colours, more than the current test palettes carry, so the poor-fit warning should fire and be right. |

## What to do with them

Save as `01-cherries.png` … `05-watermelon.png` in this directory. They are
gitignored on purpose.

Then:

```bash
cargo run -p pattern-cli -- ingest examples/generated-motifs/01-cherries.png \
    --library out/generated --id cherries --size-mm 52 --from-paper \
    --palette-from examples/peel-club-scatter/pattern.json
```

`--from-paper` matters: these arrive on white with no alpha channel, so the
luminance threshold has to cut the background away. That is the matting path
real artwork will use and the one least tested against anything real.

---

## Round two: the awkward cases

The five flat-vector subjects above are the easy path. A second set was
generated to attack what they could not, and it found more than they did.

| File | What it was for | What it found |
| --- | --- | --- |
| `10-watercolour-fig` | Does textured artwork vectorise badly, as the spec claims? | Yes, mildly — 36 outlines against 5 for a flat motif, and quantisation flattens the washes. The spec's advice to keep the raster alongside holds. |
| `11-scan-even` | Photographed artwork on real paper, evenly lit | Matting broken: cream paper at luminance 0.895 was kept as motif |
| `12-scan-uneven` | The same, lit from one side | Worse — and the only case still not fully solved, because a hard cast shadow is not a gradient |
| `13-botanical` | Many distinct colours in one motif | Handled; 72 outlines, the most of any motif |
| `14-pale-flower` | A near-white subject on near-white paper | Matting removed the flower along with the paper |

A third round was not bought. Two hypotheses were tested for free instead —
whether interior holes survive tracing, and how paper grain behaves — and both
were answered by synthetic fixtures in `crates/motif-ingest/tests/pipeline.rs`.
**Test the hypothesis cheaply before paying for artwork to test it with.**

The five scan fixtures in that file encode everything this round found, so the
findings survive without the images.
