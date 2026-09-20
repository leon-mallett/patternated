use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Number of lattice neighbours a motif is drawn at: the placement itself plus
/// the eight surrounding cells.
pub const NEIGHBOUR_COUNT: usize = 9;

/// The two translation vectors that generate a repeat.
///
/// Copies of the tile sit at every integer combination `i*u + j*v`. Every named
/// textile repeat is a choice of these two vectors over a tile of width `W` and
/// height `H`; the tile itself stays a `W x H` rectangle in all of them.
///
/// Components are millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lattice {
    pub u: Vec2,
    pub v: Vec2,
}

impl Lattice {
    /// Block (straight) repeat: an orthogonal lattice.
    ///
    /// Shows obvious rows and columns, which suits geometrics and reads as
    /// naive on organic motifs.
    #[must_use]
    pub fn block(tile_size_mm: Vec2) -> Self {
        Self {
            u: Vec2::new(tile_size_mm.x, 0.0),
            v: Vec2::new(0.0, tile_size_mm.y),
        }
    }

    /// Half-drop repeat: an oblique lattice, each column dropped by `H/2`.
    ///
    /// The commercial default for organic motifs, because the vertical stagger
    /// disrupts the tracking lines the eye finds when motifs align across tiles.
    #[must_use]
    pub fn half_drop(tile_size_mm: Vec2) -> Self {
        Self {
            u: Vec2::new(tile_size_mm.x, tile_size_mm.y / 2.0),
            v: Vec2::new(0.0, tile_size_mm.y),
        }
    }

    /// The nine offsets a motif must be drawn at for the tile to be seamless.
    ///
    /// These come from `u` and `v` themselves. Substituting the tile's own
    /// width and height — `(±W, 0)`, `(0, ±H)` and so on — happens to agree
    /// with a block lattice and is wrong for every oblique one. It produces a
    /// tile that looks right in a preview grid and fails on the printer, which
    /// is an expensive place to discover it.
    #[must_use]
    pub fn neighbour_offsets(&self) -> [Vec2; NEIGHBOUR_COUNT] {
        let mut offsets = [Vec2::ZERO; NEIGHBOUR_COUNT];
        let mut n = 0;
        for i in -1..=1 {
            for j in -1..=1 {
                offsets[n] = self.u * i as f32 + self.v * j as f32;
                n += 1;
            }
        }
        offsets
    }

    /// Area of one lattice cell, in square millimetres.
    ///
    /// Equals the tile area for a valid repeat. A degenerate lattice — parallel
    /// `u` and `v` — gives zero, which is how [`Lattice::is_degenerate`] detects
    /// a lattice that does not tile the plane.
    #[must_use]
    pub fn cell_area_mm2(&self) -> f32 {
        (self.u.x * self.v.y - self.u.y * self.v.x).abs()
    }

    /// Whether `u` and `v` are parallel, in which case they generate a line
    /// rather than a plane and no arrangement of tiles covers the fabric.
    #[must_use]
    pub fn is_degenerate(&self) -> bool {
        self.cell_area_mm2() <= f32::EPSILON
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    #[test]
    fn block_is_orthogonal() {
        let l = Lattice::block(TILE);
        assert_eq!(l.u, Vec2::new(240.0, 0.0));
        assert_eq!(l.v, Vec2::new(0.0, 180.0));
    }

    #[test]
    fn half_drop_offsets_the_column_by_half_the_tile_height() {
        let l = Lattice::half_drop(TILE);
        assert_eq!(l.u, Vec2::new(240.0, 90.0));
        assert_eq!(l.v, Vec2::new(0.0, 180.0));
    }

    /// The nine offsets are asserted against explicit expected values rather
    /// than against a loop, which could reproduce the very mistake under test.
    #[test]
    fn half_drop_neighbours_are_not_the_tile_rectangle() {
        let offsets = Lattice::half_drop(TILE).neighbour_offsets();

        let expected = [
            Vec2::new(-240.0, -270.0),
            Vec2::new(-240.0, -90.0),
            Vec2::new(-240.0, 90.0),
            Vec2::new(0.0, -180.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 180.0),
            Vec2::new(240.0, -90.0),
            Vec2::new(240.0, 90.0),
            Vec2::new(240.0, 270.0),
        ];
        assert_eq!(offsets, expected);

        // The specific bug this guards: a naive rectangle of tile dimensions.
        let naive = Vec2::new(240.0, 180.0);
        assert!(
            !offsets.contains(&naive),
            "half-drop neighbours must not include the tile rectangle corner"
        );
    }

    #[test]
    fn block_neighbours_do_coincide_with_the_tile_rectangle() {
        // Stated explicitly because it is the reason the naive version survives
        // testing: for a block repeat the two definitions agree exactly.
        let offsets = Lattice::block(TILE).neighbour_offsets();
        assert!(offsets.contains(&Vec2::new(240.0, 180.0)));
        assert!(offsets.contains(&Vec2::new(-240.0, -180.0)));
    }

    #[test]
    fn every_lattice_contains_the_identity_offset() {
        for l in [Lattice::block(TILE), Lattice::half_drop(TILE)] {
            assert!(l.neighbour_offsets().contains(&Vec2::ZERO));
        }
    }

    #[test]
    fn both_lattices_have_a_cell_area_equal_to_the_tile_area() {
        // A half-drop shears the lattice without changing how much fabric one
        // tile covers.
        let tile_area = TILE.x * TILE.y;
        assert!((Lattice::block(TILE).cell_area_mm2() - tile_area).abs() < 1e-3);
        assert!((Lattice::half_drop(TILE).cell_area_mm2() - tile_area).abs() < 1e-3);
    }

    #[test]
    fn parallel_vectors_are_degenerate() {
        let l = Lattice {
            u: Vec2::new(10.0, 5.0),
            v: Vec2::new(20.0, 10.0),
        };
        assert!(l.is_degenerate());
        assert!(!Lattice::half_drop(TILE).is_degenerate());
    }
}
