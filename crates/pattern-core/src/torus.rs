use glam::Vec2;

use crate::lattice::Lattice;

/// Squared distance between two points on the pattern's torus.
///
/// The distance that matters is not the one inside the tile. Every motif
/// appears at every lattice translate of its position, so two motifs are as
/// close as their nearest pair of copies in the infinite pattern — which may be
/// a pair that never appears in the same tile.
///
/// **This is the lattice invariant a second time, and it bites harder here.**
/// The obvious toroidal distance wraps each axis by the tile's width and
/// height. That is right for a block repeat and wrong for every oblique one. On
/// a 240 x 180 mm half-drop, a motif at (10, 10) and one at (230, 95) are 87 mm
/// apart wrapping by the rectangle, and 21 mm apart in the pattern that
/// actually gets printed, because the right-hand neighbour arrives dropped by
/// half the tile height. Sample a scatter with the rectangle distance and
/// motifs collide across the boundary — invisible in one tile, obvious at three
/// by three, and the collision is worst exactly where a half-drop was chosen to
/// avoid one.
///
/// Returned squared so that callers comparing against a radius need no square
/// root. That is not only speed: it keeps placement arithmetic to multiply,
/// add and compare, which IEEE-754 pins down exactly, so the same seed gives
/// the same placements on any platform.
#[must_use]
pub fn lattice_distance_sq(a: Vec2, b: Vec2, lattice: &Lattice) -> f32 {
    let delta = a - b;

    // Reduce into one lattice cell first, so the nearest copy is guaranteed to
    // be among the immediate neighbours of the reduced point rather than some
    // arbitrary distance away.
    let det = lattice.u.x * lattice.v.y - lattice.u.y * lattice.v.x;
    if det.abs() <= f32::EPSILON {
        return delta.length_squared();
    }
    let alpha = (delta.x * lattice.v.y - delta.y * lattice.v.x) / det;
    let beta = (lattice.u.x * delta.y - lattice.u.y * delta.x) / det;
    let reduced = lattice.u * (alpha - alpha.floor()) + lattice.v * (beta - beta.floor());

    let mut best = f32::MAX;
    for i in -1..=1 {
        for j in -1..=1 {
            let candidate = reduced + lattice.u * i as f32 + lattice.v * j as f32;
            best = best.min(candidate.length_squared());
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    fn dist(a: Vec2, b: Vec2, lattice: &Lattice) -> f32 {
        lattice_distance_sq(a, b, lattice).sqrt()
    }

    /// The tile-rectangle version, kept here only so the tests can show what it
    /// gets wrong. Nothing in the engine calls it.
    fn rectangle_distance(a: Vec2, b: Vec2, tile: Vec2) -> f32 {
        let dx = (a.x - b.x).abs().rem_euclid(tile.x);
        let dy = (a.y - b.y).abs().rem_euclid(tile.y);
        Vec2::new(dx.min(tile.x - dx), dy.min(tile.y - dy)).length()
    }

    #[test]
    fn inside_a_tile_it_is_just_the_ordinary_distance() {
        let l = Lattice::block(TILE);
        assert!((dist(Vec2::new(10.0, 10.0), Vec2::new(13.0, 14.0), &l) - 5.0).abs() < 1e-3);
    }

    #[test]
    fn opposite_edges_are_adjacent() {
        let l = Lattice::block(TILE);
        // 4 mm apart across the left-right wrap, not 236 mm.
        assert!((dist(Vec2::new(2.0, 90.0), Vec2::new(238.0, 90.0), &l) - 4.0).abs() < 1e-3);
    }

    #[test]
    fn a_half_drop_brings_the_neighbouring_column_closer_than_the_rectangle_does() {
        // The case the whole module exists for, with the numbers from the doc
        // comment above. A scatter sampled on the rectangle distance would
        // accept this pair as comfortably separated and print them overlapping.
        let a = Vec2::new(10.0, 10.0);
        let b = Vec2::new(230.0, 95.0);

        let rectangle = rectangle_distance(a, b, TILE);
        let lattice = dist(a, b, &Lattice::half_drop(TILE));

        assert!(
            (rectangle - 87.3).abs() < 0.5,
            "rectangle distance was {rectangle}"
        );
        assert!(
            (lattice - 20.6).abs() < 0.5,
            "lattice distance was {lattice}"
        );
        assert!(lattice < rectangle / 4.0);
    }

    #[test]
    fn for_a_block_repeat_the_two_definitions_agree() {
        // Stated explicitly because it is why the wrong one survives testing.
        let l = Lattice::block(TILE);
        for (a, b) in [
            (Vec2::new(10.0, 10.0), Vec2::new(230.0, 95.0)),
            (Vec2::new(5.0, 175.0), Vec2::new(235.0, 5.0)),
            (Vec2::new(120.0, 90.0), Vec2::new(60.0, 40.0)),
        ] {
            let d = dist(a, b, &l);
            let r = rectangle_distance(a, b, TILE);
            assert!((d - r).abs() < 1e-2, "{a:?} to {b:?}: {d} vs {r}");
        }
    }

    #[test]
    fn distance_is_symmetric() {
        let l = Lattice::half_drop(TILE);
        let (a, b) = (Vec2::new(17.0, 151.0), Vec2::new(201.0, 23.0));
        assert!((dist(a, b, &l) - dist(b, a, &l)).abs() < 1e-3);
    }

    #[test]
    fn a_point_is_at_no_distance_from_its_own_lattice_copies() {
        let l = Lattice::half_drop(TILE);
        let p = Vec2::new(37.0, 121.0);
        for (i, j) in [(1, 0), (0, 1), (-1, 0), (2, -1), (-3, 2)] {
            let copy = p + l.u * i as f32 + l.v * j as f32;
            assert!(
                dist(p, copy, &l) < 1e-2,
                "copy at ({i}, {j}) should coincide with the original"
            );
        }
    }

    #[test]
    fn nothing_is_ever_further_than_half_a_cell_diagonal() {
        // A sanity bound: on a torus, distance is bounded by the cell, so a
        // reduction bug that returned an unreduced delta would show up here.
        let l = Lattice::half_drop(TILE);
        let bound = (l.u + l.v).length();
        for a in [Vec2::new(0.0, 0.0), Vec2::new(239.0, 179.0)] {
            for b in [Vec2::new(120.0, 90.0), Vec2::new(1.0, 178.0)] {
                assert!(dist(a, b, &l) <= bound);
            }
        }
    }
}
