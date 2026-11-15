use serde::{Deserialize, Serialize};

/// An 8-bit-per-channel opaque colour.
///
/// Spelled the American way to match `tiny-skia` and the rest of the graphics
/// ecosystem. The domain concept built on top of it, a colourway, keeps the
/// British spelling, since nothing upstream claims that word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// An ordered list of colours that motif slot indices resolve against.
///
/// Motifs store `u8` slot indices, never literal colours, so swapping the
/// palette recolours every motif in a pattern consistently in one operation.
/// Slot 0 is the ground, which makes inverting a print to a dark background an
/// ordinary palette edit rather than a special case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub colors: Vec<Color>,
}

/// The ground is slot 0 by convention, not by special case.
pub const GROUND_SLOT: u8 = 0;

impl Palette {
    #[must_use]
    pub fn new(colors: Vec<Color>) -> Self {
        Self { colors }
    }

    /// Resolve a slot index to a colour, or `None` if the palette is too short.
    ///
    /// Returning `None` rather than a fallback colour is deliberate: a silently
    /// substituted colour is a print defect nobody notices until fabric arrives.
    #[must_use]
    pub fn resolve(&self, slot: u8) -> Option<Color> {
        self.colors.get(slot as usize).copied()
    }

    /// The ground colour, slot 0.
    #[must_use]
    pub fn ground(&self) -> Option<Color> {
        self.resolve(GROUND_SLOT)
    }

    /// How many colours this palette resolves to.
    ///
    /// Screen printing charges per colour and stops being available at all past
    /// a certain count, so this number is a cost and a manufacturability limit
    /// rather than a statistic.
    #[must_use]
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::new(vec![
            Color::new(255, 250, 240),
            Color::new(200, 30, 45),
            Color::new(60, 120, 55),
        ])
    }

    #[test]
    fn slot_zero_is_the_ground() {
        assert_eq!(palette().ground(), Some(Color::new(255, 250, 240)));
    }

    #[test]
    fn slots_resolve_by_index() {
        assert_eq!(palette().resolve(1), Some(Color::new(200, 30, 45)));
        assert_eq!(palette().resolve(2), Some(Color::new(60, 120, 55)));
    }

    #[test]
    fn an_out_of_range_slot_resolves_to_nothing_rather_than_a_substitute() {
        assert_eq!(palette().resolve(7), None);
        assert_eq!(Palette::new(vec![]).ground(), None);
    }
}
