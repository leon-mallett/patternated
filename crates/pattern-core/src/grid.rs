use glam::Vec2;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::placement::Placement;
use crate::scatter::MotifWeight;

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
    #[serde(default)]
    pub seed: u64,
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
pub fn grid(params: &GridParams, tile_size_mm: Vec2) -> Vec<Placement> {
    if params.motif_mix.is_empty()
        || params.rows == 0
        || params.cols == 0
        || tile_size_mm.x <= 0.0
        || tile_size_mm.y <= 0.0
    {
        return Vec::new();
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
    placements
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

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
            seed: 1,
        }
    }

    #[test]
    fn a_square_grid_is_evenly_spaced_and_centred_in_its_cells() {
        let p = grid(&params(2, 2, 0.0), TILE);
        assert_eq!(p.len(), 4);
        let xs: Vec<f32> = p.iter().map(|q| q.position_mm.x).collect();
        let ys: Vec<f32> = p.iter().map(|q| q.position_mm.y).collect();
        assert!(xs.contains(&60.0) && xs.contains(&180.0));
        assert!(ys.contains(&45.0) && ys.contains(&135.0));
    }

    #[test]
    fn a_half_offset_alternates_like_brickwork() {
        let p = grid(&params(2, 2, 0.5), TILE);
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
        let p = grid(&params(3, cols, offset), TILE);
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
            for q in grid(&params(4, 4, offset), TILE) {
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
        let a = grid(&params(3, 3, 0.5), TILE);
        let mut other = params(3, 3, 0.5);
        other.seed = 99;
        let b = grid(&other, TILE);
        let pos = |v: &[Placement]| v.iter().map(|p| p.position_mm).collect::<Vec<_>>();
        assert_eq!(pos(&a), pos(&b));
    }

    #[test]
    fn a_degenerate_grid_returns_nothing_rather_than_failing() {
        assert!(grid(&params(0, 3, 0.0), TILE).is_empty());
        assert!(grid(&params(3, 0, 0.0), TILE).is_empty());
        let mut empty = params(2, 2, 0.0);
        empty.motif_mix.clear();
        assert!(grid(&empty, TILE).is_empty());
    }
}
