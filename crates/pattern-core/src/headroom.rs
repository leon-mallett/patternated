//! How much a motif can grow before it touches its neighbours.
//!
//! This exists so that a size control can change a pattern without destroying
//! it. Scatter is a whole-cloth solve, since the separation test depends on every
//! motif's radius, so resizing one asset changes which candidates were
//! accepted and cascades through every placement after them. Measured on a
//! twelve-asset brief, re-solving with a single motif resized keeps **four to
//! eight per cent** of the layout. A slider wired to a re-solve is not "make
//! the dinosaur bigger"; it is "reroll the whole design, and also the dinosaur
//! is bigger", on every tick of the drag.
//!
//! The alternative is to leave every position alone and change only
//! [`Placement::scale`]. The layout then survives by construction and the one
//! remaining question is whether anything now collides, which is what this
//! module answers, ahead of time, so the control can be bounded rather than
//! the result checked.
//!
//! Shrinking needs no such bound: a motif that gets smaller cannot begin to
//! overlap anything. Only growth is constrained.

use crate::lattice::Lattice;
use crate::placement::Placement;
use crate::scatter::MotifWeight;
use crate::torus::lattice_distance_sq;

/// The largest growth factor this module will report.
///
/// A ceiling rather than an infinity, because the number is going to a slider
/// and "unbounded" is not a position on one. Reached when nothing constrains
/// the motif: a sparse layout, or a motif that is not placed at all.
pub const MOST_GROWTH: f32 = 4.0;

/// The radius of one placement's ink, at the size it is currently drawn.
fn radius_of(placement: &Placement, mix: &[MotifWeight]) -> Option<f32> {
    let entry = mix.iter().find(|m| m.motif_id == placement.motif_id)?;
    let half_diagonal = entry.natural_size_mm.length() / 2.0;
    Some(half_diagonal * entry.radius_fraction.clamp(0.05, 1.0) * placement.scale)
}

/// How far `motif_id` can be scaled up before its ink meets a neighbour's.
///
/// Returns a multiplier on the motif's current size: `1.0` means it is already
/// touching something and cannot grow, `2.0` that it could be twice the size.
/// Capped at [`MOST_GROWTH`].
///
/// Distances are measured on the lattice torus, so a motif near a tile edge is
/// judged against the neighbours it actually has on the cloth rather than the
/// ones that happen to be in the same rectangle.
///
/// # Two motifs grow together
///
/// A neighbour that is a *different* motif stays where it is, so the whole gap
/// between them is available: the bound is `(d - other) / own`. A neighbour
/// that is the *same* motif grows at the same moment and by the same factor,
/// so the pair closes from both ends at once and the bound is `d / (own +
/// other)`. Treating the second case like the first is the obvious mistake and
/// it lets a motif grow until its own copies intersect.
#[must_use]
pub fn growth_headroom(
    placements: &[Placement],
    lattice: &Lattice,
    mix: &[MotifWeight],
    motif_id: &str,
) -> f32 {
    let mut limit = MOST_GROWTH;

    for (i, subject) in placements.iter().enumerate() {
        if subject.motif_id != motif_id {
            continue;
        }
        let Some(own) = radius_of(subject, mix) else {
            continue;
        };
        if own <= 0.0 {
            continue;
        }

        for (j, other) in placements.iter().enumerate() {
            if i == j {
                continue;
            }
            let Some(other_radius) = radius_of(other, mix) else {
                continue;
            };
            let d = lattice_distance_sq(subject.position_mm, other.position_mm, lattice).sqrt();

            let allowed = if other.motif_id == motif_id {
                // Both ends move.
                d / (own + other_radius)
            } else {
                // Only this one moves, and a neighbour already inside the gap
                // leaves nothing to grow into.
                (d - other_radius) / own
            };
            limit = limit.min(allowed.max(0.0));
        }
    }

    limit.clamp(0.0, MOST_GROWTH)
}

/// Every motif's headroom, in the order the mix lists them.
///
/// The interface clamps a slider per motif and wants them all at once; asking
/// one at a time would walk the placements once per motif for no reason.
#[must_use]
pub fn growth_headroom_for_each(
    placements: &[Placement],
    lattice: &Lattice,
    mix: &[MotifWeight],
) -> Vec<(String, f32)> {
    mix.iter()
        .map(|m| {
            (
                m.motif_id.clone(),
                growth_headroom(placements, lattice, mix, &m.motif_id),
            )
        })
        .collect()
}

/// Scale every placement of one motif where it stands.
///
/// A deliberate alternative to re-solving, and the only reason it is a named
/// function rather than two lines at the call site is that the reason needs
/// somewhere to live. See the module documentation.
pub fn scale_in_place(placements: &mut [Placement], motif_id: &str, factor: f32) {
    for placement in placements.iter_mut() {
        if placement.motif_id == motif_id {
            placement.scale *= factor;
        }
    }
}

/// Pairs of placements whose ink intersects, measured on the torus.
///
/// The relation the sampler enforces while placing, asked after the fact,
/// which is what a control that moves nothing but size needs in order to check
/// its own work.
#[must_use]
pub fn overlapping_pairs(
    placements: &[Placement],
    lattice: &Lattice,
    mix: &[MotifWeight],
) -> usize {
    let mut hits = 0;
    for i in 0..placements.len() {
        let Some(a) = radius_of(&placements[i], mix) else {
            continue;
        };
        for j in (i + 1)..placements.len() {
            let Some(b) = radius_of(&placements[j], mix) else {
                continue;
            };
            let touching = a + b;
            if lattice_distance_sq(
                placements[i].position_mm,
                placements[j].position_mm,
                lattice,
            ) < touching * touching
            {
                hits += 1;
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    const TILE: Vec2 = Vec2::new(200.0, 200.0);

    fn mix() -> Vec<MotifWeight> {
        vec![
            MotifWeight {
                motif_id: "big".into(),
                weight: 1.0,
                required: false,
                // Half-diagonal of a 20x20 square is about 14.1 mm.
                natural_size_mm: Vec2::new(20.0, 20.0),
                radius_fraction: 1.0,
            },
            MotifWeight {
                motif_id: "small".into(),
                weight: 1.0,
                required: false,
                natural_size_mm: Vec2::new(10.0, 10.0),
                radius_fraction: 1.0,
            },
        ]
    }

    fn at(id: &str, x: f32, y: f32) -> Placement {
        Placement::new(id, Vec2::new(x, y))
    }

    #[test]
    fn a_lone_motif_is_capped_rather_than_unbounded() {
        // Nothing to touch. The answer is a number a slider can use, not an
        // infinity it cannot.
        let placements = vec![at("big", 100.0, 100.0)];
        let lattice = Lattice::block(TILE);
        assert_eq!(
            growth_headroom(&placements, &lattice, &mix(), "big"),
            MOST_GROWTH
        );
    }

    #[test]
    fn a_motif_that_is_not_placed_reports_the_cap() {
        let placements = vec![at("small", 100.0, 100.0)];
        let lattice = Lattice::block(TILE);
        assert_eq!(
            growth_headroom(&placements, &lattice, &mix(), "big"),
            MOST_GROWTH
        );
    }

    #[test]
    fn growing_by_exactly_the_headroom_does_not_overlap_and_a_little_more_does() {
        // The pair that makes the number mean something. A headroom that were
        // merely plausible would pass the first half of this and fail the
        // second.
        let lattice = Lattice::block(TILE);
        let mix = mix();
        let placements = vec![at("big", 60.0, 100.0), at("small", 110.0, 100.0)];

        let room = growth_headroom(&placements, &lattice, &mix, "big");
        assert!(room > 1.0, "there is a gap to grow into, got {room}");

        let mut safe = placements.clone();
        scale_in_place(&mut safe, "big", room * 0.99);
        assert_eq!(
            overlapping_pairs(&safe, &lattice, &mix),
            0,
            "growing to the stated headroom collided"
        );

        let mut over = placements;
        scale_in_place(&mut over, "big", room * 1.05);
        assert_eq!(
            overlapping_pairs(&over, &lattice, &mix),
            1,
            "growing past the stated headroom did not collide, so the \
             headroom is not a bound"
        );
    }

    #[test]
    fn two_copies_of_one_motif_close_the_gap_from_both_ends() {
        // Both grow at once, so the pair meets at half the distance each. A
        // bound written as though only one end moved would allow about twice
        // this and let a motif intersect its own copies.
        let lattice = Lattice::block(TILE);
        let mix = mix();
        let placements = vec![at("big", 60.0, 100.0), at("big", 120.0, 100.0)];

        let room = growth_headroom(&placements, &lattice, &mix, "big");
        let mut grown = placements.clone();
        scale_in_place(&mut grown, "big", room * 0.99);
        assert_eq!(overlapping_pairs(&grown, &lattice, &mix), 0);

        let mut over = placements;
        scale_in_place(&mut over, "big", room * 1.05);
        assert!(
            overlapping_pairs(&over, &lattice, &mix) > 0,
            "same-motif neighbours were treated as though they stood still"
        );
    }

    #[test]
    fn the_wrap_counts_as_a_neighbour() {
        // Two motifs at opposite edges are far apart in the rectangle and
        // adjacent on the cloth. A measure that forgot the torus would let
        // them grow into each other across the join, which is the class of
        // bug that only shows up on fabric.
        let lattice = Lattice::block(TILE);
        let mix = mix();
        let near_edges = vec![at("big", 6.0, 100.0), at("big", 194.0, 100.0)];
        let mid_tile = vec![at("big", 6.0, 100.0), at("big", 100.0, 100.0)];

        assert!(
            growth_headroom(&near_edges, &lattice, &mix, "big")
                < growth_headroom(&mid_tile, &lattice, &mix, "big"),
            "wrapping neighbours were not seen"
        );
    }

    #[test]
    fn a_motif_already_touching_cannot_grow() {
        let lattice = Lattice::block(TILE);
        let mix = mix();
        // Half-diagonals are ~14.14 and ~7.07, summing to ~21.2.
        let placements = vec![at("big", 100.0, 100.0), at("small", 110.0, 100.0)];
        assert!(
            growth_headroom(&placements, &lattice, &mix, "big") < 1.0,
            "a motif whose ink already overlaps was offered room to grow"
        );
    }

    #[test]
    fn every_motif_is_reported_in_mix_order() {
        let lattice = Lattice::block(TILE);
        let mix = mix();
        let placements = vec![at("big", 60.0, 100.0), at("small", 110.0, 100.0)];
        let all = growth_headroom_for_each(&placements, &lattice, &mix);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "big");
        assert_eq!(all[1].0, "small");
    }
}
