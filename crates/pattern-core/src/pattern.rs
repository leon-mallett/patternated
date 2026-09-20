use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::error::PatternError;
use crate::lattice::Lattice;
use crate::motif::Motif;
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
        }
        Ok(())
    }
}

/// Colours a pattern resolves to: the ground, plus every slot its motifs
/// actually use.
///
/// This is a quotable cost rather than a statistic — screen printing charges
/// per colour and stops being available past a certain count.
///
/// It needs the motifs because, since ingest, colour lives in a motif's pixels
/// rather than on the placement. An earlier version counted a `palette_slot`
/// field on each placement and reported "1 colour" for an eight-colour print,
/// which is the sort of wrong number somebody quotes from.
///
/// Counts slots that are used, not the palette's length: a house palette of
/// twenty colours drawn on four at a time is a four-colour print.
#[must_use]
pub fn resolved_color_count(pattern: &Pattern, motifs: &[Motif]) -> usize {
    let mut slots = vec![crate::palette::GROUND_SLOT];
    for motif in motifs {
        if pattern.placements.iter().any(|p| p.motif_id == motif.id) {
            slots.extend(motif.indexed.slots_used());
        }
    }
    slots.sort_unstable();
    slots.dedup();
    slots.len()
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
    fn a_zero_scale_placement_is_rejected() {
        let mut placement = Placement::new("cherry", Vec2::ZERO);
        placement.scale = 0.0;
        assert!(matches!(
            pattern(Lattice::block(TILE), vec![placement]).validate(),
            Err(PatternError::NonPositiveScale { .. })
        ));
    }

    #[test]
    fn colour_count_reads_the_motifs_not_the_placements() {
        // The bug this replaced: counting a per-placement slot reported one
        // colour for a print using eight.
        use crate::motif::IndexedMotif;
        let motifs = vec![Motif {
            id: "cherry".into(),
            natural_size_mm: Vec2::new(40.0, 40.0),
            indexed: IndexedMotif::new(2, 2, vec![1, 2, 3, 0], vec![255, 255, 255, 255])
                .expect("fixture"),
        }];
        let p = pattern(
            Lattice::block(TILE),
            vec![Placement::new("cherry", Vec2::ZERO)],
        );
        // Ground plus slots 1, 2 and 3; slot 0 is shared with the ground.
        assert_eq!(resolved_color_count(&p, &motifs), 4);
    }

    #[test]
    fn a_motif_nothing_places_costs_nothing() {
        use crate::motif::IndexedMotif;
        let motifs = vec![Motif {
            id: "unused".into(),
            natural_size_mm: Vec2::new(40.0, 40.0),
            indexed: IndexedMotif::new(1, 1, vec![7], vec![255]).expect("fixture"),
        }];
        let p = pattern(Lattice::block(TILE), vec![]);
        assert_eq!(resolved_color_count(&p, &motifs), 1);
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
