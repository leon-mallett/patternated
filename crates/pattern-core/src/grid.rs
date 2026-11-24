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
/// predictable, and right for spot patterns and geometrics, where a scatter
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
    /// The same downwards: how far each column is dropped relative to the one
    /// to its left, as a fraction of the row spacing.
    ///
    /// This is what a half-drop *is* on a grid, and until it existed the
    /// preset faked one by putting a brick on a half-drop lattice, which
    /// produced a brick, and broke wherever the lattice's drop was not a whole
    /// number of cells. A repeat's drop belongs to the cells, not to the tile.
    #[serde(default)]
    pub col_offset: f32,
    /// Maximum rotation either way, in degrees. Zero keeps a grid strict,
    /// which is usually the point of choosing one.
    #[serde(default)]
    pub rotation_jitter_deg: f32,
    #[serde(default = "unit_range")]
    pub scale_range: (f32, f32),
    pub motif_mix: Vec<MotifWeight>,
    /// Deal the mix out across the cells, rather than drawing one per cell
    /// independently.
    ///
    /// A weighted draw per cell is right for a scatter, where nobody can count
    /// what they are looking at. On a grid it is wrong, and visibly so,
    /// because the grid has already told the eye where to look: fifteen
    /// subjects over thirty cells come out as three of one, none of another,
    /// and two of the same side by side, and every one of those is something a
    /// person spots immediately. The usual remedy is to reroll the seed until
    /// it looks right, which is not a remedy at all for a print nobody
    /// approves before it goes to cloth.
    ///
    /// Dealing gives each motif its share of the cells by largest remainder,
    /// so the counts are exact rather than expected, shuffles them, and then
    /// pulls apart any two of the same that landed as neighbours. The result
    /// is still seeded and still reproducible; what changes is that the seed
    /// now chooses between good arrangements rather than between good and bad
    /// ones.
    ///
    /// Off by default, because it changes the layout of every grid document
    /// that already exists.
    #[serde(default)]
    pub deal: bool,
    /// Keep motifs from intersecting, by shrinking them to fit their cell.
    ///
    /// A grid's cells are fixed by its rows and columns, so a motif larger than
    /// the gap between neighbours overlaps them and no amount of reseeding
    /// helps. Rather than silently drawing over its neighbours, the motif is
    /// scaled down to fit, which is visible, undoable, and tells a designer
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
    /// Rows and columns actually used, which may exceed what was asked for.
    ///
    /// See [`cells_that_repeat`]. Surfaced for the same reason as the scale: a
    /// count that silently differs from the one typed is a control that
    /// appears not to work.
    pub rows: u32,
    pub cols: u32,
}

/// The smallest grid at least this large whose offsets close over the tile.
///
/// A row offset walks the motifs sideways and wraps at the tile edge. For the
/// walk to continue into the next tile rather than jump, the accumulated shift
/// over a whole tile, `rows * row_offset`, has to come out a whole number of
/// columns. A third of a column over five rows does not, so the diagonal
/// arrives at the join one and two thirds of a column out and the field breaks
/// along every tile edge. Six rows does, and it runs clean.
///
/// The same downwards for the column offset.
///
/// Solved rather than special-cased, so an offset nobody anticipated still
/// gets a grid that holds together, and returns the request unchanged when
/// nothing within reach works, so the caller can compare and say so.
///
/// This is [`Lattice::repeating_window`](crate::Lattice::repeating_window) one
/// level down: there a rectangle of tiles has to be a repeating unit, here a
/// rectangle of cells has to be.
#[must_use]
pub fn cells_that_repeat(rows: u32, cols: u32, row_offset: f32, col_offset: f32) -> (u32, u32) {
    /// How far past the request to look. A grid nobody wants to count is not a
    /// better answer than the one that was asked for.
    const REACH: u32 = 15;

    let closes = |count: u32, offset: f32| {
        let total = count as f32 * offset;
        (total - total.round()).abs() < 1e-3
    };

    let solve = |wanted: u32, offset: f32| {
        let wanted = wanted.max(1);
        (wanted..=wanted + REACH)
            .find(|n| closes(*n, offset))
            .unwrap_or(wanted)
    };

    (solve(rows, row_offset), solve(cols, col_offset))
}

fn unit_range() -> (f32, f32) {
    (1.0, 1.0)
}

/// Lay motifs out on a grid.
///
/// Positions are exact; only the choice of motif, and any rotation or scale
/// jitter, come from the seed. A grid with no jitter is therefore identical
/// whatever the seed, which is what "classic" means here: rerolling changes
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
        return (
            Vec::new(),
            GridOutcome {
                scale_applied: 1.0,
                rows: params.rows,
                cols: params.cols,
            },
        );
    }

    // Grow to a grid whose offsets close over the tile, or the field breaks
    // along every tile edge however well each tile is drawn.
    let (rows, cols) = cells_that_repeat(
        params.rows,
        params.cols,
        params.row_offset,
        params.col_offset,
    );

    let mut rng = ChaCha8Rng::seed_from_u64(params.seed);
    let cell = Vec2::new(tile_size_mm.x / cols as f32, tile_size_mm.y / rows as f32);

    let cells = (rows * cols) as usize;
    let hand = if params.deal {
        Some(deal(&params.motif_mix, cells, &mut rng))
    } else {
        None
    };

    let mut placements = Vec::with_capacity(cells);
    for row in 0..rows {
        for col in 0..cols {
            // The shifts accumulate and wrap, so an offset of a half
            // alternates and a third walks steadily across, both from the
            // same number, and the same in both axes.
            let shift = params.row_offset * row as f32;
            let drop = params.col_offset * col as f32;
            let x = ((col as f32 + 0.5 + shift) * cell.x).rem_euclid(tile_size_mm.x);
            let y = ((row as f32 + 0.5 + drop) * cell.y).rem_euclid(tile_size_mm.y);

            // When dealing, the hand was drawn before the loop. Otherwise the
            // draw happens here, between the position and the jitter, exactly
            // where it always has: moving it would change the random sequence
            // and with it every existing grid document's output.
            let motif = match &hand {
                Some(h) => h[placements.len()],
                None => crate::scatter::choose_motif_index(&params.motif_mix, &mut rng),
            };
            placements.push(Placement {
                motif_id: params.motif_mix[motif].motif_id.clone(),
                position_mm: Vec2::new(x, y),
                rotation_deg: crate::scatter::symmetric_value(&mut rng, params.rotation_jitter_deg),
                scale: crate::scatter::between_values(
                    &mut rng,
                    params.scale_range.0,
                    params.scale_range.1,
                ),
                flip_x: false,
                flip_y: false,
            });
        }
    }

    if params.deal {
        separate_like_neighbours(&mut placements, lattice);
    }

    let scale_applied = if params.prevent_overlap {
        fit_to_cells(&mut placements, params, lattice)
    } else {
        1.0
    };
    (
        placements,
        GridOutcome {
            scale_applied,
            rows,
            cols,
        },
    )
}

/// Give each motif its exact share of the cells, then shuffle.
///
/// Largest remainder rather than rounding, so the counts always sum to the
/// number of cells however the weights fall. Rounding each share on its own
/// leaves a handful of cells over or short, and whatever is done with those
/// re-introduces the bias the deal exists to remove.
fn deal(mix: &[MotifWeight], cells: usize, rng: &mut ChaCha8Rng) -> Vec<usize> {
    let total: f32 = mix.iter().map(|m| m.weight.max(0.0)).sum();
    let mut counts = vec![0usize; mix.len()];
    let mut remainders: Vec<(f32, usize)> = Vec::with_capacity(mix.len());
    let mut given = 0usize;
    for (i, m) in mix.iter().enumerate() {
        let exact = if total > 0.0 {
            cells as f32 * m.weight.max(0.0) / total
        } else {
            cells as f32 / mix.len() as f32
        };
        counts[i] = exact.floor() as usize;
        given += counts[i];
        remainders.push((exact - exact.floor(), i));
    }
    // Ties broken by index rather than by whatever sort order happens to fall
    // out, so the deal is the same on every platform.
    remainders.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    let mut k = 0;
    while given < cells && !remainders.is_empty() {
        counts[remainders[k % remainders.len()].1] += 1;
        given += 1;
        k += 1;
    }

    let mut hand: Vec<usize> = Vec::with_capacity(cells);
    for (i, c) in counts.iter().enumerate() {
        hand.extend(std::iter::repeat_n(i, *c));
    }
    // Fisher-Yates, drawing from the document's own generator so that a deal
    // is as reproducible as everything else here.
    for i in (1..hand.len()).rev() {
        let j = (crate::scatter::between_values(rng, 0.0, (i + 1) as f32) as usize).min(i);
        hand.swap(i, j);
    }
    hand
}

/// Pull apart any two cells holding the same motif that landed as neighbours.
///
/// A shuffle gives the right number of each subject and says nothing about
/// where they land, so two dinosaurs side by side is as likely as it ever was,
/// and on a grid that reads as a mistake. This swaps motifs between cells,
/// which cannot change the counts, until no two of a kind are adjacent or
/// until no swap helps.
///
/// "Adjacent" is measured on the lattice torus from the closest pair in the
/// grid, not from the cell size. On a half-drop the nearest cell in the next
/// column is not the one with the same row number, and the closest pair may be
/// across a tile edge, which is exactly where a designer will not notice it
/// until the cloth arrives.
fn separate_like_neighbours(placements: &mut [Placement], lattice: &Lattice) {
    let n = placements.len();
    if n < 3 {
        return;
    }
    let mut closest_sq = f32::MAX;
    for (i, a) in placements.iter().enumerate() {
        for b in &placements[i + 1..] {
            closest_sq = closest_sq.min(lattice_distance_sq(a.position_mm, b.position_mm, lattice));
        }
    }
    // The immediate ring and nothing beyond it. On a 50 x 45 cell half-drop
    // that is 45 mm straight down and 55 mm diagonally, and excludes the next
    // cell but one at 84 mm.
    let reach_sq = closest_sq * 1.45 * 1.45;

    let clashes = |pl: &[Placement], i: usize| -> usize {
        (0..pl.len())
            .filter(|&j| {
                j != i
                    && pl[j].motif_id == pl[i].motif_id
                    && lattice_distance_sq(pl[i].position_mm, pl[j].position_mm, lattice) < reach_sq
            })
            .count()
    };

    // Bounded. A grid that cannot be untangled is better left with a few
    // neighbours than spun on forever, and the caller can count them.
    for _ in 0..(n * 4) {
        let Some(i) = (0..n).find(|&i| clashes(placements, i) > 0) else {
            return;
        };
        let before = clashes(placements, i);
        let mut swapped = false;
        for k in 0..n {
            if placements[k].motif_id == placements[i].motif_id {
                continue;
            }
            let was = before + clashes(placements, k);
            let a = placements[i].motif_id.clone();
            let b = placements[k].motif_id.clone();
            placements[i].motif_id = b.clone();
            placements[k].motif_id = a.clone();
            if clashes(placements, i) + clashes(placements, k) < was {
                swapped = true;
                break;
            }
            placements[i].motif_id = a;
            placements[k].motif_id = b;
        }
        if !swapped {
            return;
        }
    }
}

/// Shrink motifs until none of them reaches its nearest neighbour.
///
/// The limit is the closest any two positions come *on the lattice torus*, not
/// the cell size. For a square grid those are the same; for a brick or diagonal
/// offset they are not, because the nearest neighbour is then a staggered one,
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
            required: false,
            natural_size_mm: Vec2::new(40.0, 40.0),
            radius_fraction: 0.71,
        }]
    }

    fn params(rows: u32, cols: u32, row_offset: f32) -> GridParams {
        GridParams {
            rows,
            cols,
            row_offset,
            col_offset: 0.0,
            deal: false,
            rotation_jitter_deg: 0.0,
            scale_range: (1.0, 1.0),
            motif_mix: mix(),
            // Off in most fixtures: these check where the cells are, and the
            // guard changes motif scale rather than position.
            prevent_overlap: false,
            seed: 1,
        }
    }

    /// Every motif's neighbourhood, to a fixed radius, over a field of tiles.
    ///
    /// If they are all the same the pattern is one regular field and no tile
    /// join can be visible; if they differ, the difference is the break.
    ///
    /// A radius rather than a count of nearest neighbours: a grid is full of
    /// ties, with four diagonal neighbours at the same distance, and "the six
    /// nearest" picks arbitrarily among them, which makes identical
    /// neighbourhoods look different. That mistake reported a plain square
    /// grid as broken while this was being written.
    fn neighbourhoods(placements: &[Placement], tile: Vec2, cell: Vec2) -> usize {
        const REACH: i32 = 2;
        let mut field = Vec::new();
        for i in -REACH..=REACH {
            for j in -REACH..=REACH {
                let offset = Vec2::new(tile.x * i as f32, tile.y * j as f32);
                field.extend(placements.iter().map(|p| p.position_mm + offset));
            }
        }

        // Relative to the cell, not the tile. A tile-sized radius reaches
        // past the edge of the generated field, and the truncated
        // neighbourhoods there look like differences that are not there.
        let radius = 2.2 * cell.x.max(cell.y);
        let mut signatures = std::collections::HashSet::new();
        for p in placements {
            let mut near: Vec<(i64, i64)> = field
                .iter()
                .map(|q| *q - p.position_mm)
                .filter(|d| d.length_squared() <= radius * radius)
                .map(|d| ((d.x * 100.0).round() as i64, (d.y * 100.0).round() as i64))
                .collect();
            near.sort_unstable();
            signatures.insert(near);
        }
        signatures.len()
    }

    /// The grid a designer asked for, with cells small enough that the
    /// neighbourhood radius above takes in more than the immediate ring.
    fn field_of(rows: u32, cols: u32, row_offset: f32, col_offset: f32) -> (Vec<Placement>, Vec2) {
        let p = GridParams {
            deal: false,
            col_offset,
            ..params(rows, cols, row_offset)
        };
        let (placements, outcome) = grid_with_outcome(&p, TILE, &block());
        let cell = Vec2::new(TILE.x / outcome.cols as f32, TILE.y / outcome.rows as f32);
        (placements, cell)
    }

    #[test]
    fn a_diagonal_grid_is_one_unbroken_field() {
        // Reported from the app: a diagonal grid broke along a line down the
        // middle of the preview. Every tile was drawn correctly and the tiling
        // was exact. What did not line up was the diagonal's own rhythm,
        // which arrived at the join part of a column out.
        for rows in [3u32, 4, 5, 6, 7] {
            let (field, cell) = field_of(rows, 4, 1.0 / 3.0, 0.0);
            assert_eq!(
                neighbourhoods(&field, TILE, cell),
                1,
                "a diagonal asked for with {rows} rows does not make one field"
            );
        }
    }

    #[test]
    fn a_half_drop_grid_is_one_unbroken_field() {
        for cols in [3u32, 4, 5] {
            let (field, cell) = field_of(4, cols, 0.0, 0.5);
            assert_eq!(
                neighbourhoods(&field, TILE, cell),
                1,
                "a half-drop asked for with {cols} columns does not make one field"
            );
        }
    }

    #[test]
    fn an_offset_that_does_not_close_is_what_the_complaint_looked_like() {
        // Kept so the two tests above are known to be measuring something that
        // can fail. Five rows of a one-third diagonal accumulate five thirds
        // of a column over the tile, so the next tile down starts two thirds
        // of a column adrift.
        let p = GridParams {
            deal: false,
            rows: 5,
            ..params(5, 4, 1.0 / 3.0)
        };
        let mut placements = Vec::new();
        let cell = Vec2::new(TILE.x / 4.0, TILE.y / 5.0);
        for row in 0..5u32 {
            for col in 0..4u32 {
                let shift = p.row_offset * row as f32;
                placements.push(Placement::new(
                    "cherry",
                    Vec2::new(
                        ((col as f32 + 0.5 + shift) * cell.x).rem_euclid(TILE.x),
                        (row as f32 + 0.5) * cell.y,
                    ),
                ));
            }
        }
        assert!(
            neighbourhoods(&placements, TILE, cell) > 1,
            "an unclosed diagonal came out regular, so the measurement is broken"
        );
    }

    #[test]
    fn a_grid_grows_to_the_smallest_size_its_offsets_close_at() {
        // A third of a column needs a multiple of three rows, a half needs an
        // even number, a quarter a multiple of four. Solved rather than
        // tabulated, so an offset nobody anticipated is still handled.
        assert_eq!(cells_that_repeat(5, 4, 1.0 / 3.0, 0.0), (6, 4));
        assert_eq!(cells_that_repeat(6, 4, 1.0 / 3.0, 0.0), (6, 4));
        assert_eq!(cells_that_repeat(3, 4, 0.5, 0.0), (4, 4));
        assert_eq!(cells_that_repeat(5, 4, 0.25, 0.0), (8, 4));
        // No offset, no constraint.
        assert_eq!(cells_that_repeat(5, 7, 0.0, 0.0), (5, 7));
        // And the columns are constrained by the column offset, not the row's.
        assert_eq!(cells_that_repeat(5, 5, 0.0, 0.5), (5, 6));
    }

    #[test]
    fn an_offset_with_no_whole_number_in_reach_gives_back_what_was_asked_for() {
        // A hand-edited offset with no small whole-number relationship to its
        // grid has no size worth growing to. Saying so is the caller's job;
        // the honest answer here is the request.
        assert_eq!(cells_that_repeat(4, 4, 0.137, 0.0), (4, 4));
    }

    #[test]
    fn a_column_offset_drops_columns_rather_than_shifting_rows() {
        // The difference between a half-drop and a brick, which the presets
        // could not express until this existed: one drops alternate columns,
        // the other shifts alternate rows.
        let (dropped, _) = field_of(4, 2, 0.0, 0.5);
        let (bricked, _) = field_of(4, 2, 0.5, 0.0);

        let ys = |v: &[Placement]| {
            let mut y: Vec<i64> = v.iter().map(|p| (p.position_mm.y * 10.0) as i64).collect();
            y.sort_unstable();
            y.dedup();
            y
        };
        let xs = |v: &[Placement]| {
            let mut x: Vec<i64> = v.iter().map(|p| (p.position_mm.x * 10.0) as i64).collect();
            x.sort_unstable();
            x.dedup();
            x
        };

        // Dropping columns gives twice as many distinct heights and leaves the
        // columns where they were; shifting rows does the opposite.
        assert!(ys(&dropped).len() > ys(&bricked).len());
        assert!(xs(&bricked).len() > xs(&dropped).len());
    }

    #[test]
    fn the_grid_reports_the_size_it_actually_used() {
        // A count that silently differs from the one typed is a control that
        // appears not to work.
        let p = GridParams {
            deal: false,
            col_offset: 0.0,
            ..params(5, 4, 1.0 / 3.0)
        };
        let (placements, outcome) = grid_with_outcome(&p, TILE, &block());
        assert_eq!((outcome.rows, outcome.cols), (6, 4));
        assert_eq!(placements.len(), 24);
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
        // edge, which is not an inconvenience but the reason the diagonal
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

    /// Fifteen subjects over thirty cells, dealt.
    fn many(n: usize) -> Vec<MotifWeight> {
        (0..n)
            .map(|i| MotifWeight {
                motif_id: format!("m{i}"),
                weight: 1.0,
                required: false,
                natural_size_mm: Vec2::new(30.0, 30.0),
                radius_fraction: 0.71,
            })
            .collect()
    }

    fn tally(placements: &[Placement]) -> std::collections::BTreeMap<String, usize> {
        let mut t = std::collections::BTreeMap::new();
        for p in placements {
            *t.entry(p.motif_id.clone()).or_insert(0) += 1;
        }
        t
    }

    #[test]
    fn a_deal_gives_every_motif_its_exact_share() {
        // The point of the thing. A weighted draw gives each motif two cells
        // *on average*, which over thirty cells means three of one and none of
        // another often enough to see it on most seeds.
        let tile = Vec2::new(300.0, 225.0);
        let lattice = Lattice::block(tile);
        for seed in 0..40u64 {
            let p = GridParams {
                motif_mix: many(15),
                deal: true,
                seed,
                ..params(5, 6, 0.0)
            };
            let t = tally(&grid(&p, tile, &lattice));
            assert_eq!(t.len(), 15, "seed {seed} lost a motif entirely");
            assert!(
                t.values().all(|&n| n == 2),
                "seed {seed} did not deal evenly: {t:?}"
            );
        }
    }

    #[test]
    fn a_deal_respects_an_uneven_mix() {
        // Weights still mean something: twice the weight is twice the cells,
        // and the counts have to come out whole and sum to the grid.
        let tile = Vec2::new(300.0, 225.0);
        let lattice = Lattice::block(tile);
        let mut mix = many(3);
        mix[0].weight = 4.0;
        mix[1].weight = 1.0;
        mix[2].weight = 1.0;
        let p = GridParams {
            motif_mix: mix,
            deal: true,
            seed: 7,
            ..params(5, 6, 0.0)
        };
        let t = tally(&grid(&p, tile, &lattice));
        assert_eq!(t["m0"], 20);
        assert_eq!(t["m1"], 5);
        assert_eq!(t["m2"], 5);
        assert_eq!(t.values().sum::<usize>(), 30);
    }

    #[test]
    fn a_deal_does_not_put_two_of_a_kind_side_by_side() {
        // The other half, and the half a shuffle alone does not give. Measured
        // on the torus, so a pair that meets across a tile edge counts.
        let tile = Vec2::new(300.0, 225.0);
        let lattice = Lattice::block(tile);
        for seed in 0..40u64 {
            let p = GridParams {
                motif_mix: many(15),
                deal: true,
                col_offset: 0.5,
                seed,
                ..params(5, 6, 0.0)
            };
            let pl = grid(&p, tile, &lattice);
            let mut closest = f32::MAX;
            for (i, a) in pl.iter().enumerate() {
                for b in &pl[i + 1..] {
                    closest =
                        closest.min(lattice_distance_sq(a.position_mm, b.position_mm, &lattice));
                }
            }
            let reach = closest * 1.45 * 1.45;
            for (i, a) in pl.iter().enumerate() {
                for b in &pl[i + 1..] {
                    if a.motif_id == b.motif_id {
                        assert!(
                            lattice_distance_sq(a.position_mm, b.position_mm, &lattice) >= reach,
                            "seed {seed}: two {} next to each other",
                            a.motif_id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_deal_is_deterministic() {
        let tile = Vec2::new(300.0, 225.0);
        let lattice = Lattice::block(tile);
        let p = GridParams {
            motif_mix: many(15),
            deal: true,
            col_offset: 0.5,
            seed: 20261124,
            ..params(5, 6, 0.0)
        };
        assert_eq!(grid(&p, tile, &lattice), grid(&p, tile, &lattice));
    }

    #[test]
    fn dealing_changes_what_is_in_the_cells_and_not_where_they_are() {
        // What a designer is entitled to assume when they turn it on. The
        // grid is the grid: the cells do not move.
        //
        // The jitter does change, and that is not a defect. Dealing draws the
        // whole hand from the document's generator before the loop, so every
        // draw after it lands on a different number. Giving the deal its own
        // generator to avoid that would buy a comparison nobody makes, at the
        // cost of a second seed to explain.
        let tile = Vec2::new(300.0, 225.0);
        let lattice = Lattice::block(tile);
        let p = GridParams {
            motif_mix: many(4),
            rotation_jitter_deg: 12.0,
            scale_range: (0.9, 1.1),
            seed: 3,
            ..params(4, 4, 0.0)
        };
        let before = grid(&p, tile, &lattice);
        let dealt = grid(
            &GridParams {
                deal: true,
                ..p.clone()
            },
            tile,
            &lattice,
        );
        assert_eq!(before.len(), dealt.len());
        assert!(
            before
                .iter()
                .zip(&dealt)
                .all(|(a, b)| a.position_mm == b.position_mm),
            "dealing moved the cells; it may only change which motif sits in them"
        );
    }
}
