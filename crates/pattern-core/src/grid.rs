use glam::Vec2;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::lattice::Lattice;
use crate::placement::Placement;
use crate::scatter::MotifWeight;
use crate::torus::lattice_distance_sq;

/// Motifs on a regular grid, rather than strewn.
///
/// The other half of the spec's placement model: structured placement puts a
/// few motifs at fixed points and lets the lattice do the work. Cheap,
/// predictable, and right for spot patterns and geometrics — where a scatter
/// reads as untidy rather than as hand-strewn.
///
/// The classic repeats a designer asks for by name are this plus a choice of
/// lattice. A square grid is a block lattice with no row offset; brick is the
/// same grid with alternate rows shifted half a cell; a diagonal is a grid
/// whose rows shift progressively.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridParams {
    pub rows: u32,
    pub cols: u32,
    /// How far each row is shifted sideways relative to the one above, as a
    /// fraction of the column spacing.
    ///
    /// One number covers the family: 0 is a square grid, 0.5 alternates like
    /// brickwork, and a third or a quarter walks the motifs steadily across to
    /// make a diagonal.
    #[serde(default)]
    pub row_offset: f32,
    /// Maximum rotation either way, in degrees. Zero keeps a grid strict,
    /// which is usually the point of choosing one.
    #[serde(default)]
    pub rotation_jitter_deg: f32,
    #[serde(default = "unit_range")]
    pub scale_range: (f32, f32),
    pub motif_mix: Vec<MotifWeight>,
    /// Keep motifs from intersecting, by shrinking them to fit their cell.
    ///
    /// A grid's cells are fixed by its rows and columns, so a motif larger than
    /// the gap between neighbours overlaps them and no amount of reseeding
    /// helps. Rather than silently drawing over its neighbours, the motif is
    /// scaled down to fit — which is visible, undoable, and tells a designer
    /// what to change.
    #[serde(default = "prevent_overlap_default")]
    pub prevent_overlap: bool,
    #[serde(default)]
    pub seed: u64,
}

fn prevent_overlap_default() -> bool {
    true
}

/// What a grid did, beyond placing motifs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridOutcome {
    /// Scale actually applied, as a fraction of what was asked for.
    ///
    /// Below 1 means motifs were shrunk to stop them intersecting. Worth
    /// surfacing: otherwise a size slider appears to stop working.
    pub scale_applied: f32,
}

fn unit_range() -> (f32, f32) {
    (1.0, 1.0)
}

/// Lay motifs out on a grid.
///
/// Positions are exact; only the choice of motif, and any rotation or scale
/// jitter, come from the seed. A grid with no jitter is therefore identical
/// whatever the seed, which is what "classic" means here — rerolling changes
/// which motif sits where, not where the cells are.
#[must_use]
pub fn grid(params: &GridParams, tile_size_mm: Vec2, lattice: &Lattice) -> Vec<Placement> {
    grid_with_outcome(params, tile_size_mm, lattice).0
}

/// As [`grid`], but also reporting what had to be adjusted.
#[must_use]
pub fn grid_with_outcome(
    params: &GridParams,
    tile_size_mm: Vec2,
    lattice: &Lattice,
) -> (Vec<Placement>, GridOutcome) {
    if params.motif_mix.is_empty()
        || params.rows == 0
        || params.cols == 0
        || tile_size_mm.x <= 0.0
        || tile_size_mm.y <= 0.0
    {
        return (Vec::new(), GridOutcome { scale_applied: 1.0 });
    }

    let mut rng = ChaCha8Rng::seed_from_u64(params.seed);
    let cell = Vec2::new(
        tile_size_mm.x / params.cols as f32,
        tile_size_mm.y / params.rows as f32,
    );

    let mut placements = Vec::with_capacity((params.rows * params.cols) as usize);
    for row in 0..params.rows {
        for col in 0..params.cols {
            // The shift accumulates down the rows and wraps, so a row offset of
            // a half alternates and a third walks across — both from the same
            // number.
            let shift = params.row_offset * row as f32;
            let x = ((col as f32 + 0.5 + shift) * cell.x).rem_euclid(tile_size_mm.x);
            let y = (row as f32 + 0.5) * cell.y;

            let motif = crate::scatter::choose_motif_index(&params.motif_mix, &mut rng);
            placements.push(Placement {
                motif_id: params.motif_mix[motif].motif_id.clone(),
                position_mm: Vec2::new(x, y),
                rotation_deg: crate::scatter::symmetric_value(&mut rng, params.rotation_jitter_deg),
                scale: crate::scatter::between_values(
                    &mut rng,
                    params.scale_range.0,
                    params.scale_range.1,
                ),
            });
        }
    }

    let scale_applied = if params.prevent_overlap {
        fit_to_cells(&mut placements, params, lattice)
    } else {
        1.0
    };
    (placements, GridOutcome { scale_applied })
}

/// Shrink motifs until none of them reaches its nearest neighbour.
///
/// The limit is the closest any two positions come *on the lattice torus*, not
/// the cell size. For a square grid those are the same; for a brick or diagonal
/// offset they are not, because the nearest neighbour is then a staggered one —
/// and for any of them the closest pair may be across a tile edge, which is
/// where an overlap would be least visible and most expensive.
///
/// One scale for the whole grid rather than one per motif. A grid is regular by
/// definition and a designer reading it expects its motifs to relate to each
/// other the way they were set to.
fn fit_to_cells(placements: &mut [Placement], params: &GridParams, lattice: &Lattice) -> f32 {
    let mut closest = f32::MAX;
    for (i, a) in placements.iter().enumerate() {
        for b in &placements[i + 1..] {
            closest = closest.min(lattice_distance_sq(a.position_mm, b.position_mm, lattice));
        }
    }
    // A single motif in the tile has no neighbour inside it, but it still has
    // its own copies in the next tile along.
    if placements.len() == 1 {
        for offset in lattice.neighbour_offsets() {
            if offset.length_squared() > f32::EPSILON {
                closest = closest.min(offset.length_squared());
            }
        }
    }
    if closest == f32::MAX || closest <= 0.0 {
        return 1.0;
    }
    let allowed = closest.sqrt() / 2.0;

    let mut worst = 1.0f32;
    for placement in placements.iter() {
        let entry = params
            .motif_mix
            .iter()
            .find(|m| m.motif_id == placement.motif_id);
        let Some(entry) = entry else { continue };
        let radius = entry.natural_size_mm.length() / 2.0
            * entry.radius_fraction.clamp(0.05, 1.0)
            * placement.scale;
        if radius > allowed {
            worst = worst.min(allowed / radius);
        }
    }

    if worst < 1.0 {
        for placement in placements.iter_mut() {
            placement.scale *= worst;
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    fn block() -> Lattice {
        Lattice::block(TILE)
    }

    /// The closest any two motifs' ink comes. Negative means they intersect.
    fn closest_approach(placements: &[Placement], p: &GridParams, lattice: &Lattice) -> f32 {
        let radius_of = |q: &Placement| {
            let e = p
                .motif_mix
                .iter()
                .find(|m| m.motif_id == q.motif_id)
                .expect("placement names a motif in the mix");
            e.natural_size_mm.length() / 2.0 * e.radius_fraction * q.scale
        };
        let mut worst = f32::MAX;
        for (i, a) in placements.iter().enumerate() {
            for b in &placements[i + 1..] {
                let d = lattice_distance_sq(a.position_mm, b.position_mm, lattice).sqrt();
                worst = worst.min(d - radius_of(a) - radius_of(b));
            }
        }
        worst
    }

    fn mix() -> Vec<MotifWeight> {
        vec![MotifWeight {
            motif_id: "cherry".into(),
            weight: 1.0,
            natural_size_mm: Vec2::new(40.0, 40.0),
            radius_fraction: 0.71,
        }]
    }

    fn params(rows: u32, cols: u32, row_offset: f32) -> GridParams {
        GridParams {
            rows,
            cols,
            row_offset,
            rotation_jitter_deg: 0.0,
            scale_range: (1.0, 1.0),
            motif_mix: mix(),
            // Off in most fixtures: these check where the cells are, and the
            // guard changes motif scale rather than position.
            prevent_overlap: false,
            seed: 1,
        }
    }

    #[test]
    fn a_square_grid_is_evenly_spaced_and_centred_in_its_cells() {
        let p = grid(&params(2, 2, 0.0), TILE, &block());
        assert_eq!(p.len(), 4);
        let xs: Vec<f32> = p.iter().map(|q| q.position_mm.x).collect();
        let ys: Vec<f32> = p.iter().map(|q| q.position_mm.y).collect();
        assert!(xs.contains(&60.0) && xs.contains(&180.0));
        assert!(ys.contains(&45.0) && ys.contains(&135.0));
    }

    #[test]
    fn a_half_offset_alternates_like_brickwork() {
        let p = grid(&params(2, 2, 0.5), TILE, &block());
        let row_one: Vec<f32> = p
            .iter()
            .filter(|q| q.position_mm.y < 90.0)
            .map(|q| q.position_mm.x)
            .collect();
        let row_two: Vec<f32> = p
            .iter()
            .filter(|q| q.position_mm.y > 90.0)
            .map(|q| q.position_mm.x)
            .collect();
        assert!(row_one.contains(&60.0));
        // Half a cell is 60 mm, so the second row lands midway between.
        assert!(row_two.contains(&120.0));
    }

    #[test]
    fn a_third_offset_walks_across_to_make_a_diagonal() {
        // Each row is the row above it, shifted. The shift wraps at the tile
        // edge — which is not an inconvenience but the reason the diagonal
        // tiles at all, so the property is stated modulo the tile width rather
        // than as "each row starts further right".
        let cols = 3;
        let offset = 1.0 / 3.0;
        let p = grid(&params(3, cols, offset), TILE, &block());
        let cell = TILE.x / cols as f32;

        let row_xs = |row: u32| {
            let mut xs: Vec<f32> = p
                .iter()
                .filter(|q| (q.position_mm.y / (TILE.y / 3.0)).floor() as u32 == row)
                .map(|q| q.position_mm.x)
                .collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            xs
        };

        let base = row_xs(0);
        for row in 1..3u32 {
            let mut expected: Vec<f32> = base
                .iter()
                .map(|x| (x + cell * offset * row as f32).rem_euclid(TILE.x))
                .collect();
            expected.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let actual = row_xs(row);
            for (e, a) in expected.iter().zip(&actual) {
                assert!(
                    (e - a).abs() < 0.01,
                    "row {row}: expected {expected:?}, got {actual:?}"
                );
            }
        }

        // And it is genuinely a diagonal rather than a square grid: no row
        // repeats the one above it.
        assert!(row_xs(1) != base, "a third offset left the rows aligned");
    }

    #[test]
    fn every_motif_lands_inside_the_tile() {
        for offset in [0.0, 0.5, 1.0 / 3.0, 0.75] {
            for q in grid(&params(4, 4, offset), TILE, &block()) {
                assert!(
                    (0.0..TILE.x).contains(&q.position_mm.x),
                    "{:?}",
                    q.position_mm
                );
                assert!(
                    (0.0..TILE.y).contains(&q.position_mm.y),
                    "{:?}",
                    q.position_mm
                );
            }
        }
    }

    #[test]
    fn a_grid_without_jitter_ignores_the_seed_for_position() {
        // What "classic" means: rerolling changes which motif sits where, not
        // where the cells are.
        let a = grid(&params(3, 3, 0.5), TILE, &block());
        let mut other = params(3, 3, 0.5);
        other.seed = 99;
        let b = grid(&other, TILE, &block());
        let pos = |v: &[Placement]| v.iter().map(|p| p.position_mm).collect::<Vec<_>>();
        assert_eq!(pos(&a), pos(&b));
    }

    #[test]
    fn a_grid_too_tight_for_its_motifs_shrinks_them_rather_than_overlapping() {
        // Reported from the app: motifs on a grid intersected and there was no
        // control for it, because overlap prevention had only ever been built
        // for the scatter.
        let mut p = params(2, 2, 0.0);
        p.motif_mix[0].natural_size_mm = Vec2::new(140.0, 140.0);

        p.prevent_overlap = false;
        let loose = grid(&p, TILE, &block());
        assert!(
            closest_approach(&loose, &p, &block()) < 0.0,
            "this fixture is meant to overlap without the guard"
        );

        p.prevent_overlap = true;
        let (guarded, outcome) = grid_with_outcome(&p, TILE, &block());
        assert!(
            closest_approach(&guarded, &p, &block()) >= -1e-3,
            "motifs still intersect by {:.2} mm",
            closest_approach(&guarded, &p, &block())
        );
        assert!(outcome.scale_applied < 1.0, "nothing was shrunk");
        assert_eq!(
            guarded.len(),
            loose.len(),
            "the guard should not drop motifs"
        );
    }

    #[test]
    fn a_grid_with_room_to_spare_is_left_alone() {
        // The guard must not quietly shrink a design that was already fine.
        let mut p = params(2, 2, 0.0);
        p.motif_mix[0].natural_size_mm = Vec2::new(20.0, 20.0);
        p.prevent_overlap = true;
        let (_, outcome) = grid_with_outcome(&p, TILE, &block());
        assert!((outcome.scale_applied - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_guard_measures_the_staggered_neighbour_on_an_offset_grid() {
        // On a brick or diagonal offset the nearest neighbour is a staggered
        // one, not the cell to the side, so measuring the cell would let
        // motifs touch diagonally.
        let mut p = params(3, 3, 0.5);
        p.motif_mix[0].natural_size_mm = Vec2::new(90.0, 90.0);
        p.prevent_overlap = true;
        let (placements, _) = grid_with_outcome(&p, TILE, &block());
        assert!(
            closest_approach(&placements, &p, &block()) >= -1e-3,
            "staggered neighbours intersect by {:.2} mm",
            closest_approach(&placements, &p, &block())
        );
    }

    #[test]
    fn the_guard_looks_across_the_tile_edge() {
        // An overlap at a seam is the expensive kind: invisible in one tile and
        // obvious on fabric.
        let mut p = params(1, 1, 0.0);
        p.motif_mix[0].natural_size_mm = Vec2::new(400.0, 400.0);
        p.prevent_overlap = true;
        let (placements, outcome) = grid_with_outcome(&p, TILE, &block());
        assert_eq!(placements.len(), 1);
        assert!(
            outcome.scale_applied < 1.0,
            "a single motif wider than its own tile must still be shrunk"
        );
        let e = &p.motif_mix[0];
        let radius = e.natural_size_mm.length() / 2.0 * e.radius_fraction * placements[0].scale;
        assert!(
            radius <= TILE.y / 2.0 + 1e-3,
            "the motif still reaches into the next tile"
        );
    }

    #[test]
    fn a_degenerate_grid_returns_nothing_rather_than_failing() {
        assert!(grid(&params(0, 3, 0.0), TILE, &block()).is_empty());
        assert!(grid(&params(3, 0, 0.0), TILE, &block()).is_empty());
        let mut empty = params(2, 2, 0.0);
        empty.motif_mix.clear();
        assert!(grid(&empty, TILE, &block()).is_empty());
    }
}
