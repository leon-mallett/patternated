use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::error::PatternError;
use crate::lattice::Lattice;
use crate::palette::Palette;
use crate::placement::Placement;

/// A complete repeat pattern, and the whole serialisable document format.
///
/// Small, diffable, and renderable at any resolution, because placements are
/// physical coordinates resolved at draw time rather than baked pixels. The same
/// document is a 200 px web thumbnail and a four-metre print run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// Tile width and height in millimetres. A 120 mm repeat and a 400 mm
    /// repeat are different designs, not different zoom levels.
    pub tile_size_mm: Vec2,
    pub lattice: Lattice,
    pub placements: Vec<Placement>,
    pub palette: Palette,
    /// Seed for any randomised placement. Carried now though M1 places by hand,
    /// so that a pattern made today reproduces byte-identically later.
    #[serde(default)]
    pub seed: u64,
}

impl Pattern {
    /// Check that this pattern describes something that can actually be
    /// printed.
    ///
    /// Validation is separate from construction so the type stays plain data,
    /// and is called at the edges — deserialisation and render — rather than
    /// being enforced by a constructor nobody can bypass.
    pub fn validate(&self) -> Result<(), PatternError> {
        if self.tile_size_mm.x <= 0.0 || self.tile_size_mm.y <= 0.0 {
            return Err(PatternError::NonPositiveTile {
                width: self.tile_size_mm.x,
                height: self.tile_size_mm.y,
            });
        }
        if self.lattice.is_degenerate() {
            return Err(PatternError::DegenerateLattice);
        }
        for (index, p) in self.placements.iter().enumerate() {
            if p.scale <= 0.0 {
                return Err(PatternError::NonPositiveScale {
                    index,
                    motif_id: p.motif_id.clone(),
                    scale: p.scale,
                });
            }
            if self.palette.resolve(p.palette_slot).is_none() {
                return Err(PatternError::SlotOutOfRange {
                    index,
                    motif_id: p.motif_id.clone(),
                    slot: p.palette_slot,
                    len: self.palette.len(),
                });
            }
        }
        Ok(())
    }

    /// Distinct palette slots this pattern resolves to, ground included.
    ///
    /// Screen printing charges per colour, so this is a quotable cost. Counting
    /// slots actually used rather than the palette's length matters: a house
    /// palette of twenty colours used four at a time is a four-colour print.
    #[must_use]
    pub fn resolved_color_count(&self) -> usize {
        let mut slots: Vec<u8> = self.placements.iter().map(|p| p.palette_slot).collect();
        slots.push(crate::palette::GROUND_SLOT);
        slots.sort_unstable();
        slots.dedup();
        slots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Color;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    fn pattern(lattice: Lattice, placements: Vec<Placement>) -> Pattern {
        Pattern {
            tile_size_mm: TILE,
            lattice,
            placements,
            palette: Palette::new(vec![Color::new(255, 250, 240), Color::new(200, 30, 45)]),
            seed: 0,
        }
    }

    #[test]
    fn a_valid_pattern_validates() {
        assert_eq!(
            pattern(
                Lattice::block(TILE),
                vec![Placement::new("cherry", Vec2::ZERO)]
            )
            .validate(),
            Ok(())
        );
    }

    #[test]
    fn a_zero_tile_is_rejected() {
        let mut p = pattern(Lattice::block(TILE), vec![]);
        p.tile_size_mm = Vec2::new(0.0, 180.0);
        assert!(matches!(
            p.validate(),
            Err(PatternError::NonPositiveTile { .. })
        ));
    }

    #[test]
    fn a_slot_past_the_end_of_the_palette_is_rejected() {
        let mut placement = Placement::new("cherry", Vec2::ZERO);
        placement.palette_slot = 9;
        assert!(matches!(
            pattern(Lattice::block(TILE), vec![placement]).validate(),
            Err(PatternError::SlotOutOfRange {
                slot: 9,
                len: 2,
                ..
            })
        ));
    }

    #[test]
    fn a_zero_scale_placement_is_rejected() {
        let mut placement = Placement::new("cherry", Vec2::ZERO);
        placement.scale = 0.0;
        assert!(matches!(
            pattern(Lattice::block(TILE), vec![placement]).validate(),
            Err(PatternError::NonPositiveScale { .. })
        ));
    }

    #[test]
    fn colour_count_counts_slots_used_not_the_palette_length() {
        let mut two = Placement::new("leaf", Vec2::ZERO);
        two.palette_slot = 1;
        let p = pattern(
            Lattice::block(TILE),
            vec![Placement::new("cherry", Vec2::ZERO), two],
        );
        // Ground plus slot 1; slot 0 is shared by the ground and the cherry.
        assert_eq!(p.resolved_color_count(), 2);
    }

    #[test]
    fn an_empty_pattern_still_costs_the_ground_colour() {
        assert_eq!(
            pattern(Lattice::block(TILE), vec![]).resolved_color_count(),
            1
        );
    }

    #[test]
    fn a_pattern_round_trips_through_json() {
        let p = pattern(
            Lattice::half_drop(TILE),
            vec![Placement::new("cherry", Vec2::new(30.0, 40.0))],
        );
        let json = serde_json::to_string(&p).expect("pattern should serialise");
        let back: Pattern = serde_json::from_str(&json).expect("pattern should deserialise");
        assert_eq!(p, back);
    }
}
