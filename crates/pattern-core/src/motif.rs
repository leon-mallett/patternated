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

    /// The highest slot this motif needs a palette to define.
    #[must_use]
    pub fn max_slot(&self) -> Option<u8> {
        self.slots_used().last().copied()
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
    fn a_palette_too_short_is_refused_rather_than_approximated() {
        let short = Palette::new(vec![Color::new(0, 0, 0), Color::new(1, 1, 1)]);
        assert!(matches!(
            motif().resolve_rgba(&short),
            Err(PatternError::SlotOutOfRange { slot: 2, .. })
        ));
    }
}
