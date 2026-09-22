use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::error::PatternError;
use crate::palette::{Color, Palette};

/// A motif as the engine stores it: palette slots, not colours.
///
/// This is the type the whole colourway mechanism rests on. Pixels carry an
/// index into a palette and an alpha, so swapping the palette recolours every
/// motif in a pattern consistently, in one operation, with no re-ingestion and
/// no per-motif work.
///
/// Colour and coverage are kept in separate channels on purpose. Antialiasing
/// and soft matte edges live in `alpha`, which is continuous, while `slots`
/// stays discrete — so quantising a motif to four flat colours does not also
/// quantise its edges into a staircase. A motif's edge can be soft while its
/// interior is strictly flat, which is what a printed illustration looks like.
///
/// Lives in the core rather than in `motif-ingest` because both the ingest
/// crate that produces one and the render crate that consumes one need it, and
/// rendering must not depend on matting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedMotif {
    pub width: u32,
    pub height: u32,
    /// One palette slot per pixel, row-major. Meaningless where alpha is 0.
    pub slots: Vec<u8>,
    /// Coverage per pixel, row-major. 0 is background, 255 is solid.
    pub alpha: Vec<u8>,
}

impl IndexedMotif {
    /// Build a motif, checking that the channels describe the stated size.
    pub fn new(
        width: u32,
        height: u32,
        slots: Vec<u8>,
        alpha: Vec<u8>,
    ) -> Result<Self, PatternError> {
        let expected = width as usize * height as usize;
        if slots.len() != expected || alpha.len() != expected {
            return Err(PatternError::MotifChannelMismatch {
                width,
                height,
                slots: slots.len(),
                alpha: alpha.len(),
            });
        }
        Ok(Self {
            width,
            height,
            slots,
            alpha,
        })
    }

    #[must_use]
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Every slot this motif actually uses, ascending.
    ///
    /// Counts only pixels with some coverage: a slot sitting under fully
    /// transparent pixels costs nothing to print and should not appear on a
    /// quote.
    #[must_use]
    pub fn slots_used(&self) -> Vec<u8> {
        let mut used: Vec<u8> = self
            .slots
            .iter()
            .zip(&self.alpha)
            .filter(|(_, a)| **a > 0)
            .map(|(s, _)| *s)
            .collect();
        used.sort_unstable();
        used.dedup();
        used
    }

    /// Share of this motif's ink that resolves to the ground slot, 0..1.
    ///
    /// Ink the same colour as the fabric it is printed on, which on a plain
    /// preview is invisible and on a changed ground turns into a hole the
    /// shape of whatever dissolved. It is not a fault in itself — a white star
    /// on a navy ground is exactly this, and correctly so — but it is the
    /// number to look at when a motif comes back with its middle missing.
    ///
    /// The cause is almost always a palette with no colour for that part of
    /// the artwork, so the nearest available is the ground. A peeled banana's
    /// pale interior against a six-colour palette put 18% of the motif here;
    /// widening the palette to ten dropped it to 2%.
    ///
    /// Counts only pixels with coverage, like [`Self::slots_used`]: a
    /// transparent pixel's slot is not printed and should not be counted
    /// against it.
    #[must_use]
    pub fn ground_share(&self) -> f32 {
        let mut covered = 0usize;
        let mut on_ground = 0usize;
        for (slot, alpha) in self.slots.iter().zip(&self.alpha) {
            if *alpha == 0 {
                continue;
            }
            covered += 1;
            if *slot == crate::palette::GROUND_SLOT {
                on_ground += 1;
            }
        }
        if covered == 0 {
            return 0.0;
        }
        on_ground as f32 / covered as f32
    }

    /// The highest slot this motif needs a palette to define.
    #[must_use]
    pub fn max_slot(&self) -> Option<u8> {
        self.slots_used().last().copied()
    }

    /// Radius of the smallest circle centred on the motif that contains all of
    /// its ink, as a fraction of half the bounding box diagonal.
    ///
    /// The number overlap prevention needs, and the reason it is a *fraction*
    /// rather than a length is that a motif's physical size is a design
    /// decision changed with a slider, while its shape is not.
    ///
    /// Measured from the artwork rather than assumed from the box. A trimmed
    /// bounding box touches the ink on all four sides, but its corners are
    /// usually empty: a round motif fills a square box to about 0.71 of the
    /// diagonal, so spacing by the box would hold motifs 40% further apart than
    /// they need to be and make every pattern look thin.
    ///
    /// Rotation-invariant by construction, which matters because scatter
    /// jitters rotation and a bound that changed as a motif turned would let
    /// overlaps back in at some angles.
    #[must_use]
    pub fn coverage_radius_fraction(&self) -> f32 {
        let (w, h) = (self.width as f32, self.height as f32);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let half_diagonal = (w * w + h * h).sqrt() / 2.0;
        if half_diagonal <= f32::EPSILON {
            return 1.0;
        }

        let mut furthest = 0.0f32;
        for (i, alpha) in self.alpha.iter().enumerate() {
            // Half coverage is the edge: a feathered rim fades well past where
            // the eye reads the motif ending, and bounding to the faintest
            // pixel would inflate every radius.
            if *alpha < 128 {
                continue;
            }
            let x = (i % self.width.max(1) as usize) as f32 + 0.5;
            let y = (i / self.width.max(1) as usize) as f32 + 0.5;
            let d = (x - cx) * (x - cx) + (y - cy) * (y - cy);
            furthest = furthest.max(d);
        }

        // A covered pixel is a square of ink, not a point: its far corner sits
        // half a diagonal beyond its centre. Ignoring that understates every
        // radius by up to a pixel, which is nothing on a 500 px motif and 12%
        // on an 8 px one — and the small case is the one a test fixture uses.
        const PIXEL_HALF_DIAGONAL: f32 = std::f32::consts::SQRT_2 / 2.0;
        ((furthest.sqrt() + PIXEL_HALF_DIAGONAL) / half_diagonal).clamp(0.05, 1.0)
    }

    /// Check every slot resolves against `palette`.
    pub fn check_resolvable(&self, palette: &Palette) -> Result<(), PatternError> {
        if let Some(slot) = self
            .slots_used()
            .iter()
            .find(|s| palette.resolve(**s).is_none())
        {
            return Err(PatternError::SlotOutOfRange {
                index: 0,
                motif_id: String::from("<motif>"),
                slot: *slot,
                len: palette.len(),
            });
        }
        Ok(())
    }

    /// Resolve to straight (non-premultiplied) RGBA bytes.
    ///
    /// Returns `None` for a slot the palette does not define rather than
    /// substituting a colour, on the same reasoning as [`Palette::resolve`]: a
    /// silently substituted colour is a print defect nobody notices until the
    /// fabric arrives.
    pub fn resolve_rgba(&self, palette: &Palette) -> Result<Vec<u8>, PatternError> {
        self.check_resolvable(palette)?;
        let mut out = Vec::with_capacity(self.pixel_count() * 4);
        for (slot, alpha) in self.slots.iter().zip(&self.alpha) {
            let Color { r, g, b } = palette.resolve(*slot).unwrap_or(Color::new(0, 0, 0));
            out.extend_from_slice(&[r, g, b, *alpha]);
        }
        Ok(out)
    }
}

/// A motif in a library, with the physical size its pixels stand for.
///
/// The size is what makes scale meaningful before any placement happens.
/// Without it a motif is just an image, and a print comes back with one
/// enormous dinosaur on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Motif {
    pub id: String,
    pub natural_size_mm: Vec2,
    pub indexed: IndexedMotif,
}

#[cfg(test)]
mod tests {

    #[test]
    fn ground_share_counts_ink_the_colour_of_the_fabric() {
        // Four covered pixels, one of them on the ground slot, and two
        // transparent ones carrying a ground slot that is never printed and
        // must not count against the motif.
        let m = IndexedMotif::new(3, 2, vec![0, 1, 2, 1, 0, 0], vec![255, 255, 255, 255, 0, 0])
            .expect("well formed");
        assert!(
            (m.ground_share() - 0.25).abs() < 1e-6,
            "{}",
            m.ground_share()
        );
    }

    #[test]
    fn a_motif_with_no_ink_has_no_ground_share() {
        let m = IndexedMotif::new(2, 1, vec![0, 0], vec![0, 0]).expect("well formed");
        assert_eq!(m.ground_share(), 0.0);
    }
    use super::*;

    fn palette() -> Palette {
        Palette::new(vec![
            Color::new(250, 246, 238),
            Color::new(200, 30, 45),
            Color::new(60, 120, 55),
        ])
    }

    fn motif() -> IndexedMotif {
        // 2x2: slots 1, 2, 1, 0 with the last pixel fully transparent.
        IndexedMotif::new(2, 2, vec![1, 2, 1, 0], vec![255, 255, 128, 0])
            .expect("fixture should be well formed")
    }

    #[test]
    fn channels_must_match_the_stated_size() {
        assert!(matches!(
            IndexedMotif::new(2, 2, vec![1, 2, 3], vec![255; 4]),
            Err(PatternError::MotifChannelMismatch { .. })
        ));
    }

    #[test]
    fn a_transparent_pixel_does_not_cost_a_colour() {
        // Slot 0 appears in the data but only under zero alpha, so it is not a
        // colour anybody has to pay to print.
        assert_eq!(motif().slots_used(), vec![1, 2]);
        assert_eq!(motif().max_slot(), Some(2));
    }

    #[test]
    fn resolving_substitutes_colours_for_slots() {
        let rgba = motif().resolve_rgba(&palette()).expect("should resolve");
        assert_eq!(&rgba[0..4], &[200, 30, 45, 255]);
        assert_eq!(&rgba[4..8], &[60, 120, 55, 255]);
    }

    #[test]
    fn a_soft_edge_survives_quantisation() {
        // The property the split channels exist for: the third pixel is half
        // covered and keeps its exact alpha while its colour is a flat slot.
        let rgba = motif().resolve_rgba(&palette()).expect("should resolve");
        assert_eq!(&rgba[8..12], &[200, 30, 45, 128]);
    }

    #[test]
    fn swapping_the_palette_recolours_everything_at_once() {
        // Colourways, in one assertion.
        let inverted = Palette::new(vec![
            Color::new(20, 20, 30),
            Color::new(255, 190, 60),
            Color::new(120, 200, 255),
        ]);
        let rgba = motif().resolve_rgba(&inverted).expect("should resolve");
        assert_eq!(&rgba[0..4], &[255, 190, 60, 255]);
        assert_eq!(&rgba[4..8], &[120, 200, 255, 255]);
        // Alpha is untouched by a colourway change.
        assert_eq!(rgba[11], 128);
    }

    #[test]
    fn a_full_bleed_motif_reaches_the_corners_of_its_box() {
        let solid = IndexedMotif::new(8, 8, vec![1; 64], vec![255; 64]).expect("fixture");
        assert!((solid.coverage_radius_fraction() - 1.0).abs() < 0.02);
    }

    #[test]
    fn a_round_motif_is_measurably_smaller_than_its_box() {
        // The whole reason this is measured. A disc in a square box reaches
        // about 0.71 of the diagonal; spacing by the box would hold motifs 40%
        // further apart than they need to be.
        let n = 64u32;
        let (mut slots, mut alpha) = (Vec::new(), Vec::new());
        let c = n as f32 / 2.0;
        for y in 0..n {
            for x in 0..n {
                let d = (((x as f32 + 0.5) - c).powi(2) + ((y as f32 + 0.5) - c).powi(2)).sqrt();
                slots.push(1);
                alpha.push(if d <= c { 255 } else { 0 });
            }
        }
        let disc = IndexedMotif::new(n, n, slots, alpha).expect("fixture");
        let f = disc.coverage_radius_fraction();
        assert!(
            (f - 0.707).abs() < 0.03,
            "a disc should reach about 0.71 of the diagonal, got {f:.3}"
        );
    }

    #[test]
    fn a_faint_rim_does_not_inflate_the_radius() {
        // Feathering fades well past where the eye reads the motif ending.
        let n = 32u32;
        let (mut slots, mut alpha) = (Vec::new(), Vec::new());
        let c = n as f32 / 2.0;
        for y in 0..n {
            for x in 0..n {
                let d = (((x as f32 + 0.5) - c).powi(2) + ((y as f32 + 0.5) - c).powi(2)).sqrt();
                slots.push(1);
                // Solid core, then a barely-there halo all the way out.
                alpha.push(if d <= c * 0.4 { 255 } else { 20 });
            }
        }
        let haloed = IndexedMotif::new(n, n, slots, alpha).expect("fixture");
        assert!(
            haloed.coverage_radius_fraction() < 0.45,
            "the halo inflated the radius to {}",
            haloed.coverage_radius_fraction()
        );
    }

    #[test]
    fn a_palette_too_short_is_refused_rather_than_approximated() {
        let short = Palette::new(vec![Color::new(0, 0, 0), Color::new(1, 1, 1)]);
        assert!(matches!(
            motif().resolve_rgba(&short),
            Err(PatternError::SlotOutOfRange { slot: 2, .. })
        ));
    }
}
