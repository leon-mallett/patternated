//! Measuring whether a scatter has corridors in it.
//!
//! Tracking is the fault a designer looks for and the engine could not: motifs
//! falling into rivers, with empty lanes between them, so the eye reads lines
//! running through a print that was meant to look strewn. It is the one
//! quality in this engine that had been left to opinion.
//!
//! # What this is, after two that were not
//!
//! Two earlier measures were built, calibrated against real output, found not
//! to discriminate, and removed. Both are recorded in `docs/decisions.md`; the
//! short of it is that a repeat is not a field of independent motifs — nine
//! copies of one tile repeat whatever structure that tile has — so the
//! variation between *good* patterns swamped the signal from a corridor.
//!
//! This measures the **directions between near neighbours**. Every pair closer
//! than about one and a half times the mean spacing contributes the shortest
//! displacement between them on the torus, and what is asked of those
//! directions is how far they depart from being spread evenly.
//!
//! Specifically the **fourth harmonic** of their angular distribution. The
//! second — a single preferred axis — was tried first and is blind here: a
//! field of rivers has both along-river and across-river neighbours, which sit
//! at right angles, and doubling the angle maps those onto opposite points
//! where they cancel. It scored a perfect square grid 1.4, barely above a
//! clean scatter. The fourth harmonic is the one that sees a grid.
//!
//! # What it can and cannot do
//!
//! Calibrated on 600 scatters across three lattices, five densities and forty
//! seeds, against the same patterns pulled toward parallel lines by a known
//! amount. Measured 2026-09-28:
//!
//! | Score above | Clean flagged | Blatant caught | At the level a designer rejects |
//! | --- | --- | --- | --- |
//! | 0.8 | 6.2% | 95% | 66% |
//! | 1.2 | 0.8% | 89% | 38% |
//! | 2.0 | 0.0% | 66% | 4% |
//!
//! It is **blind to faint tracking** and it is *not* a pass-or-fail gate: at
//! the level where a person would reject a print it catches roughly two in
//! three at best. What it is good for is ranking. Rendered, the highest-
//! scoring scatters out of three hundred — with nothing done to them — show
//! visible rivers, and the lowest look evenly strewn. So a high score is worth
//! looking at and a low score is not a guarantee, which is what
//! [`WORTH_A_LOOK`] says and no more.
//!
//! # Determinism
//!
//! Square roots, multiplications and divisions, and no call to `atan2`: the
//! doubled and quadrupled angles come from the double-angle identities on the
//! displacement's own components. The frequency domain was the recorded next
//! idea and would have brought an FFT and its transcendentals with it; this
//! reaches the same question without either, so the score is as reproducible
//! as the placements it measures.

use glam::Vec2;

use crate::lattice::Lattice;
use crate::placement::Placement;

/// How far apart two motifs can be and still be neighbours, as a multiple of
/// the mean spacing.
///
/// Nearest neighbours are where the eye reads alignment. Measured across 1.6,
/// 2.2 and 3.0, the signal was strongest at the smallest: wider reaches mix in
/// pairs that are simply near each other and dilute what the close ones say.
const REACH: f32 = 1.6;

/// The score above which a pattern is worth a person's eye.
///
/// Set where about one scatter in sixteen is flagged, because that is roughly
/// the share whose renders show rivers. Deliberately not set where the flagged
/// share is nearly zero: the three worst of three hundred real scatters scored
/// 0.95, 0.95 and 0.80, and every one of them was visibly tracked, so a
/// threshold that looked clean on paper would have missed all three.
pub const WORTH_A_LOOK: f32 = 0.8;

/// How strongly a scatter's near neighbours line up.
///
/// Zero is no preferred direction at all. Growing values mean more of the
/// pattern's neighbours share a few directions, which is what a river is.
/// Scale-free: the count of pairs is divided out, so a dense pattern does not
/// score lower for having more of them.
#[must_use]
pub fn alignment(placements: &[Placement], tile_size_mm: Vec2, lattice: &Lattice) -> f32 {
    let n = placements.len();
    if n < 3 || tile_size_mm.x <= 0.0 || tile_size_mm.y <= 0.0 {
        return 0.0;
    }
    let cutoff = REACH * (tile_size_mm.x * tile_size_mm.y / n as f32).sqrt();

    let (mut cos4, mut sin4, mut pairs) = (0f64, 0f64, 0usize);
    for i in 0..n {
        for j in (i + 1)..n {
            let d = shortest(
                placements[i].position_mm,
                placements[j].position_mm,
                lattice,
            );
            let len = d.length();
            if len <= f32::EPSILON || len > cutoff {
                continue;
            }
            let (x, y) = (f64::from(d.x / len), f64::from(d.y / len));
            // Doubled, then doubled again, by the double-angle identities.
            // A displacement and its reverse are the same alignment, which is
            // what the first doubling says; the second is what lets two
            // families at right angles reinforce rather than cancel.
            let (c2, s2) = (x * x - y * y, 2.0 * x * y);
            cos4 += c2 * c2 - s2 * s2;
            sin4 += 2.0 * c2 * s2;
            pairs += 1;
        }
    }

    // Too few neighbours to say anything. A handful of pairs will look aligned
    // by chance however they fell.
    if pairs < 8 {
        return 0.0;
    }
    ((cos4 * cos4 + sin4 * sin4).sqrt() / (pairs as f64).sqrt()) as f32
}

/// The shortest displacement from `a` to `b` across the repeat.
///
/// On the lattice, not the tile rectangle: the same invariant as every other
/// distance in this engine, and the same reason. Two motifs at opposite edges
/// of a half-drop are neighbours by way of a copy, and a measure that missed
/// that would see corridors where the repeat has none and miss the ones it
/// has.
fn shortest(a: Vec2, b: Vec2, lattice: &Lattice) -> Vec2 {
    let delta = b - a;
    let det = lattice.u.x * lattice.v.y - lattice.u.y * lattice.v.x;
    if det.abs() <= f32::EPSILON {
        return delta;
    }
    let s = (delta.x * lattice.v.y - delta.y * lattice.v.x) / det;
    let t = (lattice.u.x * delta.y - lattice.u.y * delta.x) / det;
    let base = delta - lattice.u * s.round() - lattice.v * t.round();
    let mut best = base;
    for i in -1..=1 {
        for j in -1..=1 {
            let candidate = base + lattice.u * i as f32 + lattice.v * j as f32;
            if candidate.length_squared() < best.length_squared() {
                best = candidate;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{grid, scatter, GridParams, Mirror, MotifWeight, ScatterParams};

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    fn mix() -> Vec<MotifWeight> {
        vec![MotifWeight {
            motif_id: "cherry".into(),
            weight: 1.0,
            required: false,
            natural_size_mm: Vec2::splat(30.0),
            radius_fraction: 1.0,
        }]
    }

    fn strewn(seed: u64, lattice: &Lattice) -> Vec<Placement> {
        scatter(
            &ScatterParams {
                density: 0.30,
                rotation_jitter_deg: 20.0,
                scale_range: (0.9, 1.1),
                motif_mix: mix(),
                prevent_overlap: true,
                mirror: Mirror::None,
                on_axis_share: 0.0,
                seed,
            },
            TILE,
            lattice,
        )
    }

    /// A scatter pulled toward parallel lines, as the calibration builds one.
    fn pulled(seed: u64, lattice: &Lattice, strength: f32) -> Vec<Placement> {
        let mut placements = strewn(seed, lattice);
        let n = placements.len();
        let period = (TILE.x * TILE.y / n as f32).sqrt() * 1.5;
        let normal = Vec2::new(-(35.0f32.to_radians()).sin(), (35.0f32.to_radians()).cos());
        for p in placements.iter_mut() {
            let along = p.position_mm.dot(normal);
            let nearest = (along / period).round() * period;
            let at = p.position_mm + normal * ((nearest - along) * strength);
            p.position_mm = Vec2::new(at.x.rem_euclid(TILE.x), at.y.rem_euclid(TILE.y));
        }
        placements
    }

    #[test]
    fn a_grid_is_nothing_but_alignment() {
        // The sanity check. If a measure of lining-up cannot see a pattern
        // that is entirely lined up, nothing else it says is worth reading.
        let lattice = Lattice::block(TILE);
        let ruled = grid(
            &GridParams {
                rows: 5,
                cols: 5,
                row_offset: 0.0,
                col_offset: 0.0,
                rotation_jitter_deg: 0.0,
                scale_range: (1.0, 1.0),
                motif_mix: mix(),
                prevent_overlap: false,
                seed: 1,
            },
            TILE,
            &lattice,
        );
        let ruled_score = alignment(&ruled, TILE, &lattice);
        let strewn_score = alignment(&strewn(7, &lattice), TILE, &lattice);
        assert!(
            ruled_score > strewn_score * 2.0,
            "a 5x5 grid scored {ruled_score:.2} against a scatter's {strewn_score:.2}"
        );
        // Comparative and not against `WORTH_A_LOOK`, which it does not clear.
        // A 5x5 grid in this tile puts its diagonal neighbours inside the
        // reach as well as its rows and columns, and those two families are
        // forty-five degrees apart, where the fourth harmonic partly cancels
        // them against each other. It scores 0.78.
        //
        // Left as a comparison rather than tuned away, because it is a true
        // thing about the measure: what it reports depends on which families
        // of neighbour fall inside the cutoff. A ruled grid is also not what
        // the threshold is for — nobody needs telling that a grid lines up.
    }

    #[test]
    fn pulling_a_scatter_into_rivers_raises_it() {
        // The property the whole thing exists for, across seeds rather than on
        // one — a single pair proves only that two numbers differ.
        let lattice = Lattice::half_drop(TILE);
        let mut raised = 0;
        for seed in [1u64, 7, 42, 99, 20_260_928] {
            let before = alignment(&strewn(seed, &lattice), TILE, &lattice);
            let after = alignment(&pulled(seed, &lattice, 0.8), TILE, &lattice);
            if after > before {
                raised += 1;
            }
        }
        assert_eq!(raised, 5, "pulling into rivers did not raise every seed");
    }

    #[test]
    fn it_says_nothing_about_a_pattern_too_small_to_have_a_texture() {
        // A handful of motifs will look aligned by chance whatever they did,
        // and a warning on three cherries would be noise a designer learns to
        // ignore.
        let few: Vec<Placement> = (0..4)
            .map(|i| Placement::new("cherry", Vec2::new(20.0 * i as f32, 30.0)))
            .collect();
        assert_eq!(alignment(&few, TILE, &Lattice::block(TILE)), 0.0);
    }

    #[test]
    fn it_measures_across_the_repeat_and_not_the_rectangle() {
        // The invariant every distance in this engine keeps. Two motifs by
        // opposite edges of a half-drop are neighbours by way of a copy; a
        // measure that wrapped by width and height would pair them wrongly and
        // report corridors the repeat does not have.
        let lattice = Lattice::half_drop(TILE);
        let a = alignment(&strewn(7, &lattice), TILE, &lattice);
        let b = alignment(&strewn(7, &lattice), TILE, &Lattice::block(TILE));
        assert!(
            (a - b).abs() > 1e-4,
            "the lattice made no difference ({a:.4} against {b:.4}), so it is \
             not being used"
        );
    }

    #[test]
    fn the_same_pattern_always_scores_the_same() {
        let lattice = Lattice::half_drop(TILE);
        let p = strewn(20_260_928, &lattice);
        assert_eq!(
            alignment(&p, TILE, &lattice).to_bits(),
            alignment(&p, TILE, &lattice).to_bits()
        );
    }
}
