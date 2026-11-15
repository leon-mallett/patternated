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
    /// A generated layer between the fabric and the motifs: stripes, a check,
    /// a tartan, a texture.
    ///
    /// Optional because most prints are motifs alone, and a pattern may be a
    /// ground with no motifs at all, which is what a tartan is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground: Option<crate::Ground>,
    pub palette: Palette,
    /// Seed for any randomised placement. Carried now though M1 places by hand,
    /// so that a pattern made today reproduces byte-identically later.
    #[serde(default)]
    pub seed: u64,
}

impl Pattern {
    /// The same design at a different physical size.
    ///
    /// Not a zoom. A 120 mm repeat and a 400 mm repeat are different designs,
    /// which is the first invariant in this engine. The reason this
    /// exists is the other half of that thought: the *same* design worn by a
    /// three-year-old and by an adult is also two different repeats, and
    /// arriving at the second by hand means redoing every length.
    ///
    /// Everything in millimetres moves together: the tile, the lattice that
    /// steps it, where each motif sits, and whatever the ground measures in
    /// lengths. Motif sizes are not here, because a motif belongs to a library
    /// rather than to one pattern; see `Document::scaled_by`.
    ///
    /// A placement's own `scale` is left alone: it is a multiplier on a motif's
    /// natural size, so it follows once that size moves, and scaling both
    /// would square the factor.
    #[must_use]
    pub fn scaled_by(&self, factor: f32) -> Self {
        Self {
            tile_size_mm: self.tile_size_mm * factor,
            lattice: Lattice {
                u: self.lattice.u * factor,
                v: self.lattice.v * factor,
            },
            placements: self
                .placements
                .iter()
                .map(|p| Placement {
                    position_mm: p.position_mm * factor,
                    ..p.clone()
                })
                .collect(),
            ground: self.ground.as_ref().map(|g| g.scaled_by(factor)),
            palette: self.palette.clone(),
            seed: self.seed,
        }
    }

    /// Check that this pattern describes something that can actually be
    /// printed.
    ///
    /// Validation is separate from construction so the type stays plain data,
    /// and is called at the edges, at deserialisation and render, rather than
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
        // Reflections generate translations by the tile's own width and
        // height, which is the block lattice and nothing else. Combining one
        // with a half-drop looks plausible in a parameter file and prints as a
        // break running down every mirror axis, so it is refused rather than
        // rendered.
        // A ground lives in the tile and repeats with it, so it only meets
        // itself when the lattice steps by whole tiles. On a half-drop the
        // neighbouring column arrives half a tile down: stripes step at every
        // join, and a texture grown on a torus meets an edge that is not the
        // edge it was grown against. The same thing that made a grid on a drop
        // lattice break, and refused here for the same reason.
        if self.ground.as_ref().is_some_and(|g| !g.is_empty()) {
            let block = crate::Lattice::block(self.tile_size_mm);
            if (self.lattice.u - block.u).length() >= 1e-3
                || (self.lattice.v - block.v).length() >= 1e-3
            {
                return Err(PatternError::GroundNeedsBlockLattice);
            }
        }

        // Bands resolve through the palette like everything else, so a slot
        // they name that the palette does not define is the same fault as a
        // motif naming one.
        if let Some(ground) = &self.ground {
            if let Some(slot) = ground
                .slots_used()
                .into_iter()
                .find(|s| self.palette.resolve(*s).is_none())
            {
                return Err(PatternError::SlotOutOfRange {
                    index: 0,
                    motif_id: String::from("<ground>"),
                    slot,
                    len: self.palette.len(),
                });
            }
        }

        let block = crate::Lattice::block(self.tile_size_mm);
        let is_block = (self.lattice.u - block.u).length() < 1e-3
            && (self.lattice.v - block.v).length() < 1e-3;

        for (index, p) in self.placements.iter().enumerate() {
            if p.scale <= 0.0 {
                return Err(PatternError::NonPositiveScale {
                    index,
                    motif_id: p.motif_id.clone(),
                    scale: p.scale,
                });
            }
            if (p.flip_x || p.flip_y) && !is_block {
                return Err(PatternError::MirrorNeedsBlockLattice {
                    index,
                    motif_id: p.motif_id.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Colours a pattern resolves to: the ground, plus every slot its motifs
/// actually use.
///
/// This is a quotable cost rather than a statistic: screen printing charges
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
    // A striped ground is printed too, and a designer quoting a colour count
    // that omitted it would be quoting for the wrong job.
    if let Some(ground) = &pattern.ground {
        slots.extend(ground.slots_used());
    }
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
mod scaling_tests {
    use super::*;
    use crate::{Band, Bands, Figure, Ornament, Palette, Texture};

    fn tile() -> Vec2 {
        Vec2::new(240.0, 180.0)
    }

    fn pattern(ground: Option<crate::Ground>) -> Pattern {
        Pattern {
            tile_size_mm: tile(),
            lattice: Lattice::half_drop(tile()),
            placements: vec![Placement::new("cherry", Vec2::new(60.0, 40.0))],
            ground,
            palette: Palette::new(vec![
                crate::Color::new(250, 246, 238),
                crate::Color::new(1, 2, 3),
            ]),
            seed: 1,
        }
    }

    #[test]
    fn every_length_moves_together() {
        // The whole point. If the tile grew and the lattice did not, the
        // repeat would step by the wrong amount and every join would break; if
        // positions did not, the motifs would huddle in one corner of a bigger
        // tile.
        let big = pattern(None).scaled_by(2.0);
        assert_eq!(big.tile_size_mm, Vec2::new(480.0, 360.0));
        assert_eq!(big.lattice.u, Lattice::half_drop(tile()).u * 2.0);
        assert_eq!(big.lattice.v, Lattice::half_drop(tile()).v * 2.0);
        assert_eq!(big.placements[0].position_mm, Vec2::new(120.0, 80.0));
    }

    #[test]
    fn the_repeat_still_repeats_at_the_new_size() {
        // A design that validated before has to validate after, or the scale
        // control is a way of producing a pattern the engine will not render.
        for ground in [
            None,
            Some(Bands::check(9.0, 8.0, 1, 1).into()),
            Some(
                Ornament {
                    levels: vec![1],
                    ..Ornament::default()
                }
                .into(),
            ),
        ] {
            let mut p = pattern(ground);
            // A ground needs a block lattice, so give it one before scaling.
            if p.ground.is_some() {
                p.lattice = Lattice::block(p.tile_size_mm);
            }
            p.validate().expect("valid before");
            p.scaled_by(0.4).validate().expect("valid after");
        }
    }

    #[test]
    fn a_bands_widths_are_lengths_and_move() {
        let banded = pattern(Some(
            Bands {
                vertical: vec![Band::new(20.0, 1)],
                ..Bands::default()
            }
            .into(),
        ))
        .scaled_by(0.5);
        let bands = banded
            .ground
            .as_ref()
            .and_then(crate::Ground::as_bands)
            .expect("bands");
        assert_eq!(bands.vertical[0].width_mm, 10.0);
    }

    #[test]
    fn an_ornaments_cell_is_a_length_and_its_texture_is_a_count() {
        // The distinction the scaling turns on. A cell size is millimetres and
        // has to grow with the fabric; a texture's cells are how many fit
        // across the tile, so they follow it already and doubling them would
        // halve the pattern instead of keeping it.
        let drawn = pattern(Some(
            Ornament {
                cell_mm: 24.0,
                stroke_mm: 4.0,
                levels: vec![1],
                figure: Figure::Truchet,
                ..Ornament::default()
            }
            .into(),
        ))
        .scaled_by(2.0);
        let o = drawn
            .ground
            .as_ref()
            .and_then(crate::Ground::as_ornament)
            .expect("ornament");
        assert_eq!((o.cell_mm, o.stroke_mm), (48.0, 8.0));

        let grown = pattern(Some(
            Texture {
                cells: 96,
                levels: vec![crate::GROUND_SLOT, 1],
                ..Texture::default()
            }
            .into(),
        ))
        .scaled_by(2.0);
        let t = grown
            .ground
            .as_ref()
            .and_then(crate::Ground::as_texture)
            .expect("texture");
        assert_eq!(t.cells, 96, "a count is not a length");
    }

    #[test]
    fn a_placements_own_scale_is_left_where_it_was() {
        // It multiplies a motif's natural size, and that size moves with the
        // garment too. Scaling both would apply the factor twice.
        let mut p = pattern(None);
        p.placements[0].scale = 1.3;
        assert_eq!(p.scaled_by(3.0).placements[0].scale, 1.3);
    }

    #[test]
    fn scaling_there_and_back_returns_the_design() {
        let there = pattern(Some(Bands::check(9.0, 8.0, 1, 1).into())).scaled_by(400.0 / 330.0);
        let back = there.scaled_by(330.0 / 400.0);
        let original = pattern(Some(Bands::check(9.0, 8.0, 1, 1).into()));
        assert!((back.tile_size_mm - original.tile_size_mm).length() < 1e-3);
        assert!(
            (back.placements[0].position_mm - original.placements[0].position_mm).length() < 1e-3
        );
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
            ground: None,
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
