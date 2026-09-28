// Which preset a ground's sequence *is*, worked out rather than remembered.
//
// The complaint this answers: choosing Pinstripe showed Custom immediately, so
// an intentional choice looked as though it had not taken.

import { describe, expect, it } from "vitest";

import { GROUND_PRESETS, presetOf } from "./api";

/** A ground as it comes back from the engine, which is what the menu reads. */
function stored(id: string) {
  const preset = GROUND_PRESETS.find((p) => p.id === id)!;
  const spec = preset.make("#3c78a0");
  // The engine applies a reflection once and returns the expanded sequence;
  // `mirrored` is an instruction, never a stored mode.
  const bands = spec.mirrored
    ? [...spec.bands, ...spec.bands.slice(1, -1).reverse()]
    : spec.bands;
  return {
    bands,
    direction: spec.direction,
    // A one-way ground has no crossings, so the engine reports the rule that
    // means "nothing is mixed".
    crossings: spec.direction === "both" ? spec.crossings : "across",
  };
}

describe("naming a ground's preset", () => {
  it("recognises every preset it offers", () => {
    // Including the tartan, whose stored sequence is six bands where the
    // preset names four — a reflection is applied once and kept.
    for (const preset of GROUND_PRESETS) {
      expect(presetOf(stored(preset.id))).toBe(preset.id);
    }
    expect(GROUND_PRESETS.length).toBeGreaterThan(4);
  });

  it("still recognises one that has only been recoloured", () => {
    // A preset describes an arrangement. Changing a stripe's ink leaves it a
    // stripe, and being told otherwise is the same surprise as before.
    const stripe = stored("stripe");
    const recoloured = {
      ...stripe,
      bands: stripe.bands.map((b) => (b.colour ? { ...b, colour: "#8a1c3b" } : b)),
    };
    expect(presetOf(recoloured)).toBe("stripe");
  });

  it("calls it custom once the arrangement changes", () => {
    const stripe = stored("stripe");
    // A width edit.
    expect(presetOf({ ...stripe, bands: [{ ...stripe.bands[0], width_mm: 5 }, stripe.bands[1]] })).toBe("");
    // A band added.
    expect(presetOf({ ...stripe, bands: [...stripe.bands, { width_mm: 4, colour: "#111" }] })).toBe("");
    // A gap turned into a thread, which is what separates a stripe from a check.
    expect(
      presetOf({
        ...stripe,
        bands: [stripe.bands[0], { ...stripe.bands[1], colour: "#eee" }],
      }),
    ).toBe("");
  });

  it("calls a ground with no bands plain", () => {
    expect(presetOf({ bands: [], direction: "both", crossings: "blend" })).toBe("none");
  });

  it("does not confuse two presets that differ only in their widths", () => {
    // Stripe and pinstripe are the same shape and the same direction; only the
    // numbers tell them apart, so a comparison that skipped widths would
    // report whichever came first in the list.
    expect(presetOf(stored("stripe"))).toBe("stripe");
    expect(presetOf(stored("pinstripe"))).toBe("pinstripe");
  });
});
