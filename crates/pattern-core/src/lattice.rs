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

    /// Brick (half-brick) repeat: each row shifted sideways by half a tile.
    ///
    /// The horizontal counterpart of a half-drop, and the one that suits
    /// geometrics and stripes, where a vertical stagger reads as a mistake and
    /// a horizontal one reads as masonry.
    #[must_use]
    pub fn brick(tile_size_mm: Vec2) -> Self {
        Self {
            u: Vec2::new(tile_size_mm.x, 0.0),
            v: Vec2::new(tile_size_mm.x / 2.0, tile_size_mm.y),
        }
    }

    /// Quarter-drop repeat: a gentler diagonal than a half-drop.
    ///
    /// Worth having as its own thing rather than as a half-drop turned down:
    /// the diagonal it produces runs at a different angle, which is the whole
    /// reason a designer reaches for it.
    #[must_use]
    pub fn quarter_drop(tile_size_mm: Vec2) -> Self {
        Self {
            u: Vec2::new(tile_size_mm.x, tile_size_mm.y / 4.0),
            v: Vec2::new(0.0, tile_size_mm.y),
        }
    }

    /// The nine offsets a motif must be drawn at for the tile to be seamless.
    ///
    /// These come from `u` and `v` themselves. Substituting the tile's own
    /// width and height, as `(±W, 0)` and `(0, ±H)`, happens to agree
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

    /// The smallest window, in whole tiles, that is itself a repeating unit.
    ///
    /// A preview is a rectangular window onto an infinite pattern, and a
    /// rectangle of tiles is not generally a repeating unit even when every
    /// tile in it is perfect. A half-drop gains half a tile of drop per column,
    /// so three columns end half a tile out and the window's left and right
    /// edges do not meet, which looks exactly like a broken repeat to anyone
    /// checking the obvious way, by holding the opposite edges together.
    ///
    /// The window repeats when translating across it lands on a lattice point:
    /// `(cols * W, 0)` and `(0, rows * H)` must both be integer combinations of
    /// `u` and `v`. For a half-drop that needs an even column count, for a
    /// quarter-drop a multiple of four, and for a brick an even row count.
    /// Rather than special-case each, the condition is solved directly, so a
    /// lattice nobody anticipated still gets a window that holds together.
    ///
    /// Returns the requested size unchanged if no larger window within reach
    /// works, which can happen for a hand-edited lattice with an irrational
    /// relationship to its tile. The caller can compare and say so.
    #[must_use]
    pub fn repeating_window(&self, tile_size_mm: Vec2, min_cols: u32, min_rows: u32) -> (u32, u32) {
        /// How far past the requested size to look before giving up. Eight is
        /// already a window nobody wants to squint at.
        const REACH: u32 = 8;

        let det = self.u.x * self.v.y - self.u.y * self.v.x;
        if det.abs() <= f32::EPSILON || tile_size_mm.x <= 0.0 || tile_size_mm.y <= 0.0 {
            return (min_cols.max(1), min_rows.max(1));
        }

        let whole = |value: f32| (value - value.round()).abs() < 1e-3;

        let cols = (min_cols.max(1)..=min_cols.max(1) + REACH)
            .find(|k| {
                let x = *k as f32 * tile_size_mm.x;
                whole(x * self.v.y / det) && whole(-self.u.y * x / det)
            })
            .unwrap_or(min_cols.max(1));

        let rows = (min_rows.max(1)..=min_rows.max(1) + REACH)
            .find(|m| {
                let y = *m as f32 * tile_size_mm.y;
                whole(-y * self.v.x / det) && whole(self.u.x * y / det)
            })
            .unwrap_or(min_rows.max(1));

        (cols, rows)
    }

    /// Area of one lattice cell, in square millimetres.
    ///
    /// Equals the tile area for a valid repeat. A degenerate lattice, with
    /// parallel `u` and `v`, gives zero, which is how [`Lattice::is_degenerate`] detects
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
    fn brick_staggers_sideways_rather_than_downwards() {
        let l = Lattice::brick(TILE);
        assert_eq!(l.u, Vec2::new(240.0, 0.0));
        assert_eq!(l.v, Vec2::new(120.0, 180.0));
    }

    #[test]
    fn quarter_drop_is_a_gentler_slope_than_half_drop() {
        let q = Lattice::quarter_drop(TILE);
        let h = Lattice::half_drop(TILE);
        assert_eq!(q.u, Vec2::new(240.0, 45.0));
        assert!(q.u.y < h.u.y, "a quarter drop should fall less than a half");
    }

    #[test]
    fn every_lattice_contains_the_identity_offset() {
        for l in [
            Lattice::block(TILE),
            Lattice::half_drop(TILE),
            Lattice::brick(TILE),
            Lattice::quarter_drop(TILE),
        ] {
            assert!(l.neighbour_offsets().contains(&Vec2::ZERO));
        }
    }

    #[test]
    fn both_lattices_have_a_cell_area_equal_to_the_tile_area() {
        // A half-drop shears the lattice without changing how much fabric one
        // tile covers.
        let tile_area = TILE.x * TILE.y;
        for lattice in [
            Lattice::block(TILE),
            Lattice::half_drop(TILE),
            Lattice::brick(TILE),
            Lattice::quarter_drop(TILE),
        ] {
            assert!(
                (lattice.cell_area_mm2() - tile_area).abs() < 1e-3,
                "{lattice:?} does not cover exactly one tile"
            );
        }
    }

    #[test]
    fn a_block_window_repeats_at_any_size() {
        for (c, r) in [(1, 1), (2, 3), (3, 3), (5, 4)] {
            assert_eq!(Lattice::block(TILE).repeating_window(TILE, c, r), (c, r));
        }
    }

    #[test]
    fn a_half_drop_window_needs_an_even_number_of_columns() {
        // Reported from the app: a 3x3 preview of a half-drop did not meet at
        // its left and right edges, which is the obvious way to check a repeat
        // and the way a designer will check it.
        let l = Lattice::half_drop(TILE);
        assert_eq!(l.repeating_window(TILE, 3, 3), (4, 3));
        assert_eq!(l.repeating_window(TILE, 2, 3), (2, 3));
        assert_eq!(l.repeating_window(TILE, 1, 1), (2, 1));
    }

    #[test]
    fn a_quarter_drop_window_needs_four_columns() {
        let l = Lattice::quarter_drop(TILE);
        assert_eq!(l.repeating_window(TILE, 3, 3), (4, 3));
        assert_eq!(l.repeating_window(TILE, 5, 1), (8, 1));
    }

    #[test]
    fn a_brick_window_needs_an_even_number_of_rows_instead() {
        // The mirror image of the half-drop case: a brick staggers sideways, so
        // it is the vertical edges that fail to meet on an odd count.
        let l = Lattice::brick(TILE);
        assert_eq!(l.repeating_window(TILE, 3, 3), (3, 4));
        assert_eq!(l.repeating_window(TILE, 3, 2), (3, 2));
    }

    #[test]
    fn an_unreachable_lattice_gives_back_what_was_asked_for() {
        // A hand-edited lattice with no small whole-number relationship to its
        // tile has no window worth showing. Saying so is the caller's job; the
        // honest answer here is the request.
        let odd = Lattice {
            u: Vec2::new(240.0, 71.3),
            v: Vec2::new(0.0, 180.0),
        };
        assert_eq!(odd.repeating_window(TILE, 3, 3), (3, 3));
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
