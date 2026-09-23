//! Stripes, checks, ginghams and plaids: pattern that is not placed motifs.
//!
//! Everything else in this engine puts motifs somewhere. A stripe is not a
//! motif anywhere — it is the fabric being a different colour in bands, and
//! trying to express one as a very tall thin motif works until two of them
//! cross. Where a vertical band meets a horizontal one the result is neither
//! band's colour but a third, chosen by the designer, and a motif drawn over
//! another motif cannot produce a colour that neither contains.
//!
//! So bands are their own thing, and they sit **between the ground and the
//! motifs**: the fabric is filled, the bands are laid over it, and whatever is
//! strewn goes on top. That ordering is the whole design decision, and it is
//! what makes a check with flowers tossed across it cost nothing.
//!
//! Slots throughout, never colours, so a check recolours with a colourway like
//! everything else.

use serde::{Deserialize, Serialize};

use crate::palette::GROUND_SLOT;

/// One band, and the colour it paints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub width_mm: f32,
    /// [`GROUND_SLOT`] leaves the fabric showing, which is how the gaps
    /// between stripes are expressed rather than by making them a colour that
    /// happens to match.
    pub slot: u8,
}

impl Band {
    #[must_use]
    pub fn new(width_mm: f32, slot: u8) -> Self {
        Self { width_mm, slot }
    }

    /// A gap: the fabric, for this many millimetres.
    #[must_use]
    pub fn gap(width_mm: f32) -> Self {
        Self::new(width_mm, GROUND_SLOT)
    }

    fn paints(&self) -> bool {
        self.slot != GROUND_SLOT && self.width_mm > 0.0
    }
}

/// What a crossing of two painted bands resolves to.
///
/// Stored as a list rather than a matrix because a check uses one or two of
/// them and a matrix of every slot against every other would be mostly empty
/// and entirely hand-maintained.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crossing {
    pub a: u8,
    pub b: u8,
    pub slot: u8,
}

/// A striped or checked ground.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Bands {
    /// Bands laid out along the width, each running the full height — the ones
    /// that read as **vertical stripes**. Empty for a horizontal-only stripe.
    #[serde(default)]
    pub vertical: Vec<Band>,
    /// Bands laid out down the height, each running the full width — the ones
    /// that read as **horizontal stripes**.
    #[serde(default)]
    pub horizontal: Vec<Band>,
    /// What to paint where a vertical band crosses a horizontal one.
    ///
    /// The entry that makes a gingham a gingham: two colours give three tones,
    /// because the crossing is darker than either band alone. Without a
    /// matching entry the horizontal band wins, which is what drawing one over
    /// the other would have done anyway.
    #[serde(default)]
    pub crossings: Vec<Crossing>,
}

impl Bands {
    /// A plain stripe: a colour and a gap, repeating across.
    #[must_use]
    pub fn stripe(width_mm: f32, gap_mm: f32, slot: u8) -> Self {
        Self {
            vertical: vec![Band::new(width_mm, slot), Band::gap(gap_mm)],
            ..Self::default()
        }
    }

    /// A check: the same sequence both ways, with a named crossing.
    ///
    /// The crossing is the point. Give it a third, deeper slot and the result
    /// is a gingham; give it the band's own slot and it is a windowpane, where
    /// the lines simply overlap.
    #[must_use]
    pub fn check(width_mm: f32, gap_mm: f32, slot: u8, crossing: u8) -> Self {
        let sequence = vec![Band::new(width_mm, slot), Band::gap(gap_mm)];
        Self {
            vertical: sequence.clone(),
            horizontal: sequence,
            crossings: vec![Crossing {
                a: slot,
                b: slot,
                slot: crossing,
            }],
        }
    }

    /// A sequence reflected about its two end bands.
    ///
    /// Most tartan setts are written this way: the threadcount runs from one
    /// pivot to the other, and the full repeat is that sequence followed by
    /// itself reversed with **both pivots left out**, so neither end band is
    /// doubled. `A B C D` repeats as `A B C D C B`, not `A B C D D C B A`.
    ///
    /// Worth its own function and its own test because the off-by-one is
    /// invisible: doubling the pivots gives a sett that is still symmetrical,
    /// still tiles, and is simply the wrong tartan.
    #[must_use]
    pub fn reflected(sequence: &[Band]) -> Vec<Band> {
        if sequence.len() < 3 {
            return sequence.to_vec();
        }
        let mut out = sequence.to_vec();
        out.extend(
            sequence
                .iter()
                .rev()
                .skip(1)
                .take(sequence.len() - 2)
                .copied(),
        );
        out
    }

    /// Whether anything is painted at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.vertical.iter().any(Band::paints) && !self.horizontal.iter().any(Band::paints)
    }

    /// How far the sequence runs before repeating, in millimetres.
    #[must_use]
    pub fn period(sequence: &[Band]) -> f32 {
        sequence.iter().map(|b| b.width_mm.max(0.0)).sum()
    }

    /// The same bands, with every width scaled so the sequence divides the
    /// tile exactly.
    ///
    /// A sequence that does not divide the tile is cut at the tile's edge, and
    /// since the next tile starts the sequence again the join carries a band of
    /// the wrong width — down every repeat, on every metre. This is the third
    /// time the same shape has come up: a rectangle of tiles has to be a
    /// repeating unit, a grid's cells have to close over the tile, and now so
    /// does a band sequence.
    ///
    /// Scaling the widths rather than growing the tile, because the repeat size
    /// is a decision about the print and a band width is a proportion the
    /// designer will not miss half a millimetre of. The factor is returned so a
    /// caller can say what it did.
    #[must_use]
    pub fn fitted_to(&self, tile_width_mm: f32, tile_height_mm: f32) -> (Self, (f32, f32)) {
        let scale = |sequence: &[Band], extent: f32| -> (Vec<Band>, f32) {
            let period = Self::period(sequence);
            if period <= 0.0 || extent <= 0.0 {
                return (sequence.to_vec(), 1.0);
            }
            // The nearest whole number of repeats, never none.
            let repeats = (extent / period).round().max(1.0);
            let factor = extent / (period * repeats);
            (
                sequence
                    .iter()
                    .map(|b| Band::new(b.width_mm * factor, b.slot))
                    .collect(),
                factor,
            )
        };

        let (vertical, across) = scale(&self.vertical, tile_width_mm);
        let (horizontal, down) = scale(&self.horizontal, tile_height_mm);
        (
            Self {
                vertical,
                horizontal,
                crossings: self.crossings.clone(),
            },
            (across, down),
        )
    }

    /// The slot painted at a point, or `None` where the fabric shows.
    ///
    /// Expects widths that already divide the tile — see [`Self::fitted_to`] —
    /// and wraps, so a point outside the tile lands where its repeat would.
    #[must_use]
    pub fn slot_at(&self, x_mm: f32, y_mm: f32) -> Option<u8> {
        let vertical = Self::band_at(&self.vertical, x_mm);
        let horizontal = Self::band_at(&self.horizontal, y_mm);
        self.resolve(vertical, horizontal)
    }

    /// How the two directions combine at one point.
    #[must_use]
    pub fn resolve(&self, vertical: Option<u8>, horizontal: Option<u8>) -> Option<u8> {
        match (vertical, horizontal) {
            (None, other) | (other, None) => other,
            (Some(v), Some(h)) => Some(
                self.crossings
                    .iter()
                    .find(|c| (c.a == v && c.b == h) || (c.a == h && c.b == v))
                    .map_or(h, |c| c.slot),
            ),
        }
    }

    /// The vertical band covering a position across the fabric.
    #[must_use]
    pub fn vertical_slot_at(&self, x_mm: f32) -> Option<u8> {
        Self::band_at(&self.vertical, x_mm)
    }

    /// The horizontal band covering a position down the fabric.
    #[must_use]
    pub fn horizontal_slot_at(&self, y_mm: f32) -> Option<u8> {
        Self::band_at(&self.horizontal, y_mm)
    }

    /// The painting band covering a coordinate, wrapping by the sequence's own
    /// period.
    fn band_at(sequence: &[Band], value_mm: f32) -> Option<u8> {
        let period = Self::period(sequence);
        if period <= 0.0 {
            return None;
        }
        let mut at = value_mm.rem_euclid(period);
        for band in sequence {
            let width = band.width_mm.max(0.0);
            if at < width {
                return band.paints().then_some(band.slot);
            }
            at -= width;
        }
        // Only reachable through floating-point accumulation at the very end of
        // the sequence; the last painting band is the right answer there.
        sequence.iter().rev().find(|b| b.paints()).map(|b| b.slot)
    }

    /// Every slot these bands need a palette to define.
    #[must_use]
    pub fn slots_used(&self) -> Vec<u8> {
        let mut used: Vec<u8> = self
            .vertical
            .iter()
            .chain(&self.horizontal)
            .filter(|b| b.paints())
            .map(|b| b.slot)
            .chain(self.crossings.iter().map(|c| c.slot))
            .collect();
        used.sort_unstable();
        used.dedup();
        used
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stripe_paints_its_band_and_leaves_its_gap() {
        let b = Bands::stripe(10.0, 30.0, 2);
        assert_eq!(b.slot_at(0.0, 0.0), Some(2));
        assert_eq!(b.slot_at(9.9, 50.0), Some(2), "still inside the band");
        assert_eq!(b.slot_at(10.1, 0.0), None, "the gap is the fabric");
        assert_eq!(b.slot_at(39.9, 0.0), None);
        // And it repeats.
        assert_eq!(b.slot_at(40.0, 0.0), Some(2));
        assert_eq!(b.slot_at(400.0, 0.0), Some(2));
    }

    #[test]
    fn a_vertical_stripe_does_not_vary_down_the_fabric() {
        // The property that makes it a stripe rather than a grid of blocks.
        let b = Bands::stripe(10.0, 30.0, 2);
        for y in [0.0f32, 13.0, 500.0, -7.0] {
            assert_eq!(b.slot_at(5.0, y), Some(2), "at y = {y}");
            assert_eq!(b.slot_at(25.0, y), None, "at y = {y}");
        }
    }

    #[test]
    fn a_check_gives_three_tones_from_two_colours() {
        // What makes a gingham a gingham, and the thing no amount of drawing
        // one motif over another produces: a colour neither band contains.
        let b = Bands::check(10.0, 10.0, 2, 3);

        assert_eq!(b.slot_at(15.0, 15.0), None, "both gaps: the fabric");
        assert_eq!(b.slot_at(5.0, 15.0), Some(2), "vertical band only");
        assert_eq!(b.slot_at(15.0, 5.0), Some(2), "horizontal band only");
        assert_eq!(b.slot_at(5.0, 5.0), Some(3), "both: the crossing colour");
    }

    #[test]
    fn a_crossing_is_found_whichever_way_round_it_is_written() {
        let mut b = Bands::check(10.0, 10.0, 2, 9);
        b.crossings = vec![Crossing {
            a: 3,
            b: 2,
            slot: 9,
        }];
        b.horizontal = vec![Band::new(10.0, 3), Band::gap(10.0)];
        assert_eq!(b.slot_at(5.0, 5.0), Some(9));
    }

    #[test]
    fn without_a_crossing_the_horizontal_band_wins() {
        // Stated rather than left to chance: it is what drawing one over the
        // other would have done, so the fallback surprises nobody.
        let mut b = Bands::check(10.0, 10.0, 2, 3);
        b.crossings.clear();
        b.horizontal = vec![Band::new(10.0, 4), Band::gap(10.0)];
        assert_eq!(b.slot_at(5.0, 5.0), Some(4));
    }

    #[test]
    fn widths_are_scaled_so_the_sequence_divides_the_tile() {
        // 25 mm of sequence does not divide a 240 mm tile: nine repeats is
        // 225 and ten is 250, so the join would carry a band of the wrong
        // width down every repeat.
        let b = Bands::stripe(10.0, 15.0, 2);
        assert!((Bands::period(&b.vertical) - 25.0).abs() < 1e-6);

        let (fitted, (across, _)) = b.fitted_to(240.0, 180.0);
        let period = Bands::period(&fitted.vertical);
        let repeats = 240.0 / period;
        assert!(
            (repeats - repeats.round()).abs() < 1e-4,
            "{repeats} repeats of a {period} mm sequence in a 240 mm tile"
        );
        assert!((repeats - 10.0).abs() < 1e-4, "nearest whole count is ten");
        // A 4% nudge, which is what "the nearest whole number of repeats" cost.
        assert!((across - 0.96).abs() < 0.01, "scaled by {across}");
    }

    #[test]
    fn a_sequence_that_already_divides_is_left_alone() {
        let b = Bands::stripe(10.0, 10.0, 2);
        let (fitted, (across, down)) = b.fitted_to(240.0, 180.0);
        assert_eq!(fitted.vertical, b.vertical);
        assert!((across - 1.0).abs() < 1e-6 && (down - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fitting_never_asks_for_less_than_one_repeat() {
        // A sequence wider than the tile would otherwise scale to zero
        // repeats and divide by nothing.
        let b = Bands::stripe(400.0, 400.0, 2);
        let (fitted, _) = b.fitted_to(240.0, 180.0);
        let period = Bands::period(&fitted.vertical);
        assert!((period - 240.0).abs() < 1e-3, "one repeat fills the tile");
    }

    #[test]
    fn a_reflected_sett_does_not_double_its_pivots() {
        // The off-by-one that gives a sett which is still symmetrical, still
        // tiles, and is the wrong tartan.
        let seq: Vec<Band> = [1u8, 2, 3, 4].iter().map(|s| Band::new(10.0, *s)).collect();
        let slots: Vec<u8> = Bands::reflected(&seq).iter().map(|b| b.slot).collect();
        assert_eq!(slots, vec![1, 2, 3, 4, 3, 2]);
    }

    #[test]
    fn reflecting_a_short_sequence_leaves_it_alone() {
        // Two bands reflected about both of them is the two bands. Reflecting
        // anyway would repeat the sequence rather than mirror it.
        for n in 0..3 {
            let seq: Vec<Band> = (0..n).map(|i| Band::new(10.0, i as u8 + 1)).collect();
            assert_eq!(Bands::reflected(&seq), seq, "length {n}");
        }
    }

    #[test]
    fn a_reflected_sett_is_symmetrical_about_its_pivots() {
        // The property the rule exists for, checked rather than assumed: read
        // the repeat forwards from the first pivot and backwards from it, and
        // the colours agree.
        let seq: Vec<Band> = [1u8, 2, 3, 4, 5]
            .iter()
            .map(|s| Band::new(10.0, *s))
            .collect();
        let full = Bands::reflected(&seq);
        let n = full.len();
        for i in 1..n {
            assert_eq!(
                full[i].slot,
                full[n - i].slot,
                "band {i} should mirror band {}",
                n - i
            );
        }
    }

    #[test]
    fn bands_report_the_slots_a_palette_has_to_define() {
        let b = Bands::check(10.0, 10.0, 2, 5);
        assert_eq!(b.slots_used(), vec![2, 5]);
        assert!(Bands::default().slots_used().is_empty());
    }

    #[test]
    fn nothing_painted_is_nothing_painted() {
        assert!(Bands::default().is_empty());
        assert!(Bands::stripe(10.0, 10.0, GROUND_SLOT).is_empty());
        assert!(!Bands::stripe(10.0, 10.0, 1).is_empty());
    }
}
