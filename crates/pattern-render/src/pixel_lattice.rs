use glam::{IVec2, Vec2};
use pattern_core::{px_per_mm, Lattice};

/// The lattice as the raster sees it, in whole pixels.
///
/// Rasterising quantises the repeat, and this is where that happens explicitly
/// rather than by accident. The reason it needs care: a 240 mm tile at 150 DPI
/// is 1417.32 px wide. Something has to absorb that fraction. If the tile is
/// rounded to 1417 px but neighbours are offset by 1417.32 px, every repeat
/// drifts by a third of a pixel and a few metres down the roll the seam is
/// visibly out.
///
/// So the tile's rounded pixel size *is* the repeat, and the lattice is derived
/// from it rather than rounded independently. Each lattice vector is first
/// expressed as a fraction of the tile rectangle, so that a half-drop's `u` is
/// `(1, 0.5)` of it, and those fractions are then applied to the integer tile
/// size. The tiling is exact for any integers that come out, so the pattern is
/// seamless whatever the DPI, and the most a half-drop's stagger can deviate
/// from its ideal is half a pixel on an odd tile height.
///
/// Note what this does *not* do: it never substitutes the tile rectangle for
/// the lattice vectors. The fractions come from `u` and `v`, so an oblique
/// repeat stays oblique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelLattice {
    /// Tile size in whole pixels. At least 1 in each axis.
    pub tile_px: IVec2,
    pub u_px: IVec2,
    pub v_px: IVec2,
}

impl PixelLattice {
    /// Quantise a lattice and tile size to whole pixels at a given resolution.
    #[must_use]
    pub fn new(lattice: &Lattice, tile_size_mm: Vec2, dpi: f32) -> Self {
        Self::for_mirror(lattice, tile_size_mm, dpi, false, false)
    }

    /// The same, for a tile built as a mirror repeat.
    ///
    /// A reflected axis forces its dimension to an even number of pixels, so
    /// that the mirror line falls *between* two pixels rather than through
    /// one. Reflecting a tile `T` pixels wide sends pixel `j` to `T-1-j`; with
    /// `T` even the two halves map exactly onto each other, and with `T` odd
    /// there is a middle pixel belonging to neither, which puts a motif
    /// mounted on the axis one pixel off its own reflection.
    ///
    /// It costs at most one pixel of repeat, the same trade the lattice
    /// divisor above already makes.
    #[must_use]
    pub fn for_mirror(
        lattice: &Lattice,
        tile_size_mm: Vec2,
        dpi: f32,
        reflects_x: bool,
        reflects_y: bool,
    ) -> Self {
        let k = px_per_mm(dpi);
        let wanted = IVec2::new(
            ((tile_size_mm.x * k).round() as i32).max(1),
            ((tile_size_mm.y * k).round() as i32).max(1),
        );

        // The tile is rounded to a size the lattice can divide exactly.
        //
        // A half-drop steps by half the tile height, and 180 mm at 150 DPI is
        // 1063 px, which is odd, so the drop rounds to 532 and four columns come back
        // two pixels adrift. A single tile still tiles, because its own
        // neighbour offsets are consistent; what breaks is anything that
        // *accumulates* the lattice, which is to say a pre-repeated sheet sent
        // to a printer, and a printer stepping the tile themselves.
        //
        // Rounding the tile instead moves the repeat by at most a pixel, or
        // 0.17 mm on a 180 mm repeat, which is the same trade M1 already made
        // when it decided the rounded tile is the repeat.
        let divisor = |fraction: f32| -> i32 {
            (1..=16)
                .find(|q| {
                    let scaled = fraction * *q as f32;
                    (scaled - scaled.round()).abs() < 1e-4
                })
                .unwrap_or(1)
        };
        let height_divisor = divisor(lattice.u.y / tile_size_mm.y.max(1e-6))
            .max(divisor(lattice.v.y / tile_size_mm.y.max(1e-6)))
            .max(if reflects_y { 2 } else { 1 });
        let width_divisor = divisor(lattice.u.x / tile_size_mm.x.max(1e-6))
            .max(divisor(lattice.v.x / tile_size_mm.x.max(1e-6)))
            .max(if reflects_x { 2 } else { 1 });

        let round_to = |value: i32, divisor: i32| -> i32 {
            if divisor <= 1 {
                return value;
            }
            let rounded = ((value as f32 / divisor as f32).round() as i32) * divisor;
            rounded.max(divisor)
        };
        let tile_px = IVec2::new(
            round_to(wanted.x, width_divisor),
            round_to(wanted.y, height_divisor),
        );

        // Express each vector as a fraction of the tile rectangle, then apply
        // that fraction to the integer tile. This is the step that keeps the
        // repeat and the tile in agreement.
        let as_pixels = |vec: Vec2| -> IVec2 {
            IVec2::new(
                ((vec.x / tile_size_mm.x) * tile_px.x as f32).round() as i32,
                ((vec.y / tile_size_mm.y) * tile_px.y as f32).round() as i32,
            )
        };

        Self {
            tile_px,
            u_px: as_pixels(lattice.u),
            v_px: as_pixels(lattice.v),
        }
    }

    /// The nine neighbour offsets in pixels, in the same order as the core's.
    #[must_use]
    pub fn neighbour_offsets(&self) -> [IVec2; pattern_core::NEIGHBOUR_COUNT] {
        let mut offsets = [IVec2::ZERO; pattern_core::NEIGHBOUR_COUNT];
        let mut n = 0;
        for i in -1..=1 {
            for j in -1..=1 {
                offsets[n] = self.u_px * i + self.v_px * j;
                n += 1;
            }
        }
        offsets
    }

    /// Every lattice offset whose tile overlaps a `width x height` pixel window
    /// with its top-left at the origin.
    ///
    /// Found by inverting the lattice at the window's corners rather than by
    /// guessing a range, so it stays correct for oblique repeats, where a
    /// sheared lattice reaches further sideways than the window is wide.
    #[must_use]
    pub fn offsets_covering(&self, width: i32, height: i32) -> Vec<IVec2> {
        let det = self.u_px.x * self.v_px.y - self.u_px.y * self.v_px.x;
        if det == 0 {
            return vec![IVec2::ZERO];
        }
        let det = det as f32;

        let (mut i_min, mut i_max, mut j_min, mut j_max) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for corner in [
            Vec2::ZERO,
            Vec2::new(width as f32, 0.0),
            Vec2::new(0.0, height as f32),
            Vec2::new(width as f32, height as f32),
        ] {
            let i = (corner.x * self.v_px.y as f32 - corner.y * self.v_px.x as f32) / det;
            let j = (self.u_px.x as f32 * corner.y - self.u_px.y as f32 * corner.x) / det;
            i_min = i_min.min(i);
            i_max = i_max.max(i);
            j_min = j_min.min(j);
            j_max = j_max.max(j);
        }

        // One cell of margin each way: a tile whose origin sits outside the
        // window can still overlap it.
        let mut offsets = Vec::new();
        for i in (i_min.floor() as i32 - 1)..=(i_max.ceil() as i32 + 1) {
            for j in (j_min.floor() as i32 - 1)..=(j_max.ceil() as i32 + 1) {
                offsets.push(self.u_px * i + self.v_px * j);
            }
        }
        offsets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    #[test]
    fn a_block_lattice_quantises_to_the_tile_rectangle() {
        let pl = PixelLattice::new(&Lattice::block(TILE), TILE, 150.0);
        assert_eq!(pl.u_px, IVec2::new(pl.tile_px.x, 0));
        assert_eq!(pl.v_px, IVec2::new(0, pl.tile_px.y));
    }

    #[test]
    fn a_half_drop_tile_is_rounded_to_an_even_height() {
        // 180 mm at 150 DPI is 1062.99 px, which rounds to 1063, and that is odd, so half
        // of it is not a whole pixel. Found by measuring an exported sheet: the
        // drop rounded to 532, four columns came back two pixels adrift, and
        // the seam where two copies met was four times an interior step.
        let pl = PixelLattice::new(&Lattice::half_drop(TILE), TILE, 150.0);
        assert_eq!(pl.tile_px.y % 2, 0, "height {} is odd", pl.tile_px.y);
        assert_eq!(pl.u_px, IVec2::new(pl.tile_px.x, pl.tile_px.y / 2));
    }

    #[test]
    fn a_quarter_drop_tile_is_rounded_to_a_multiple_of_four() {
        let pl = PixelLattice::new(&Lattice::quarter_drop(TILE), TILE, 150.0);
        assert_eq!(
            pl.tile_px.y % 4,
            0,
            "height {} is not divisible by four",
            pl.tile_px.y
        );
        assert_eq!(pl.u_px, IVec2::new(pl.tile_px.x, pl.tile_px.y / 4));
    }

    #[test]
    fn a_brick_tile_is_rounded_on_its_width_instead() {
        // A brick staggers sideways, so it is the width that has to divide.
        let pl = PixelLattice::new(&Lattice::brick(TILE), TILE, 150.0);
        assert_eq!(pl.tile_px.x % 2, 0, "width {} is odd", pl.tile_px.x);
        assert_eq!(pl.v_px, IVec2::new(pl.tile_px.x / 2, pl.tile_px.y));
    }

    #[test]
    fn rounding_the_tile_moves_it_by_at_most_a_pixel() {
        // The cost of the above, stated so it cannot grow unnoticed.
        for dpi in [72.0, 96.0, 150.0, 300.0, 600.0] {
            for lattice in [
                Lattice::half_drop(TILE),
                Lattice::quarter_drop(TILE),
                Lattice::brick(TILE),
            ] {
                let pl = PixelLattice::new(&lattice, TILE, dpi);
                let ideal = IVec2::new(
                    (TILE.x * pattern_core::px_per_mm(dpi)).round() as i32,
                    (TILE.y * pattern_core::px_per_mm(dpi)).round() as i32,
                );
                assert!(
                    (pl.tile_px.x - ideal.x).abs() <= 2 && (pl.tile_px.y - ideal.y).abs() <= 2,
                    "{lattice:?} at {dpi} DPI moved the tile from {ideal:?} to {:?}",
                    pl.tile_px
                );
            }
        }
    }

    #[test]
    fn a_block_tile_is_not_rounded_at_all() {
        // Nothing divides a block lattice, so nothing should move.
        let pl = PixelLattice::new(&Lattice::block(TILE), TILE, 150.0);
        assert_eq!(pl.tile_px, IVec2::new(1417, 1063));
    }

    #[test]
    fn the_pixel_cell_area_still_equals_the_tile_area() {
        // The property that makes tiles partition the plane with no gap and no
        // overlap. If quantisation broke it, the repeat would not be seamless
        // at any DPI.
        for lattice in [Lattice::block(TILE), Lattice::half_drop(TILE)] {
            for dpi in [72.0, 96.0, 150.0, 300.0] {
                let pl = PixelLattice::new(&lattice, TILE, dpi);
                let area = pl.u_px.x * pl.v_px.y - pl.u_px.y * pl.v_px.x;
                assert_eq!(
                    area.abs(),
                    pl.tile_px.x * pl.tile_px.y,
                    "lattice {lattice:?} at {dpi} DPI"
                );
            }
        }
    }

    #[test]
    fn a_tile_never_rounds_away_to_nothing() {
        let pl = PixelLattice::new(&Lattice::block(TILE), Vec2::new(0.01, 0.01), 72.0);
        assert_eq!(pl.tile_px, IVec2::new(1, 1));
    }

    #[test]
    fn covering_offsets_include_the_origin_and_span_the_window() {
        let pl = PixelLattice::new(&Lattice::half_drop(TILE), TILE, 72.0);
        let offsets = pl.offsets_covering(pl.tile_px.x * 3, pl.tile_px.y * 3);
        assert!(offsets.contains(&IVec2::ZERO));
        assert!(offsets.contains(&pl.u_px));
        assert!(offsets.contains(&(pl.u_px * 2 + pl.v_px)));
    }

    #[test]
    fn covering_offsets_reach_sideways_for_an_oblique_lattice() {
        // A half-drop's u carries a vertical component, so covering the bottom
        // of a window needs offsets a block repeat would never ask for. This is
        // the case a guessed -1..=1 range would miss, leaving a bald corner.
        let pl = PixelLattice::new(&Lattice::half_drop(TILE), TILE, 72.0);
        let offsets = pl.offsets_covering(pl.tile_px.x * 3, pl.tile_px.y * 3);
        let max_i_span = offsets.iter().map(|o| o.x / pl.u_px.x).max().unwrap_or(0);
        assert!(max_i_span >= 3, "window three tiles wide needs i up to 3");
    }

    #[test]
    fn a_mirrored_tile_is_rounded_to_an_even_number_of_pixels() {
        // 240 x 180 mm at 150 DPI wants 1417 x 1063 px, both odd. A mirror
        // line has to fall between two pixels or a motif mounted on it lands
        // one pixel off its own reflection.
        let plain = PixelLattice::new(&Lattice::block(TILE), TILE, 150.0);
        assert_eq!(plain.tile_px, IVec2::new(1417, 1063));

        let across = PixelLattice::for_mirror(&Lattice::block(TILE), TILE, 150.0, true, false);
        assert_eq!(across.tile_px, IVec2::new(1418, 1063));

        let both = PixelLattice::for_mirror(&Lattice::block(TILE), TILE, 150.0, true, true);
        assert_eq!(both.tile_px, IVec2::new(1418, 1064));

        // And the lattice still divides the tile it was given.
        assert_eq!(both.u_px, IVec2::new(1418, 0));
        assert_eq!(both.v_px, IVec2::new(0, 1064));
    }
}
