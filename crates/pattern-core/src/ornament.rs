//! Pattern by rule: a grid of cells, each carrying a figure, some of which
//! contain smaller copies of themselves.
//!
//! The other generated ground in this engine grows: two chemicals spreading
//! across the tile until structure appears, with no lattice in it anywhere.
//! This one is the opposite pole. It is all lattice: a cell size, a figure, and
//! a rule for what a figure contains. Interlocking curves, strapwork, and the
//! classical square fractals, which is the ornamental half of what a repeat
//! print is made of and the half reaction–diffusion cannot reach.
//!
//! # Why it tiles
//!
//! The grid is fitted to the tile, a whole number of cells each way, by the
//! same arithmetic that fits a band sequence, and for the same reason. Every
//! cell is then evaluated in its own coordinates and knows nothing of its
//! neighbours, so the tile's last column and its first are two ordinary cells
//! that happen to meet.
//!
//! The line figures add one requirement to that: every figure meets the cell
//! edge at the **middle of the edge and nowhere else**. Two neighbouring cells
//! therefore join whatever figures they drew, which is Truchet's observation
//! and the whole reason his tiles work. It holds across the tile boundary as
//! well, so the repeat joins without anything being done about it.
//!
//! # Why it is deterministic
//!
//! Integer arithmetic, comparison, multiply, add, and `sqrt`, which IEEE 754
//! requires to be correctly rounded, unlike the transcendentals this engine
//! avoids everywhere else. No trigonometry: a quarter arc is a distance
//! compared against a radius, not an angle swept.
//!
//! The per-cell choices come from an **integer hash of the cell's coordinates**
//! rather than from a stream of random numbers. That is a deliberate departure
//! from the scatter sampler, which takes its draws in order from `rand_chacha`,
//! and the reason is that a stream answers "what is the next number" while this
//! needs "what is the number *for this cell*". Sampling visits cells in
//! whatever order a renderer walks pixels, and a recursive figure visits the
//! same cell once per level; a stream would have to be materialised into an
//! array first, where a hash answers in place. It is seeded, pure, and has no
//! floating point in it at all, so it is if anything harder to make
//! non-deterministic than the sampler is.

use serde::{Deserialize, Serialize};

use crate::palette::GROUND_SLOT;

/// The smallest cell worth drawing, in millimetres.
///
/// Below this the figure is finer than a printer resolves and the ground reads
/// as a flat tone, which is an expensive way to dye fabric.
pub const FINEST_CELL_MM: f32 = 1.0;

/// How deep a recursive figure may go.
///
/// Each level is a third of the one above, so five levels take a 30 mm cell to
/// a tenth of a millimetre and there is nothing below that worth having.
pub const DEEPEST: u32 = 5;

/// What fills one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Figure {
    /// Two quarter arcs, corner to corner, turned one way or the other.
    /// Neighbouring cells join, and the join is what makes the wandering
    /// closed loops this is known for.
    Truchet,
    /// The same idea drawn straight: two segments cutting opposite corners,
    /// each joining the middles of two adjacent edges. A labyrinth.
    Weave,
    /// Clamshell: a filled semicircle sitting on each cell, with alternate
    /// rows shifted half a cell so the scallops interlock. Fish scale.
    Scale,
    /// The Sierpinski carpet: a cell, less its middle ninth, less the middle
    /// ninth of each of those, as far down as `depth` goes.
    Carpet,
    /// Cantor dust: only the corner ninths survive each level. The sparsest,
    /// and the one that reads as speckle with a structure behind it.
    Dust,
    /// The Vicsek figure: the middle ninth and the four beside it, so each
    /// level is a cross of crosses.
    Cross,
}

impl Figure {
    /// Whether `depth` means anything for this figure.
    ///
    /// The line figures are one level and stay that way. A subdivided Truchet
    /// cell would present connection points at a sixth and five sixths of each
    /// edge as well as at the middle, and an undivided neighbour has nothing to
    /// meet them with, so the lines would end in stubs along the join. Multi-
    /// scale Truchet designs solve that with dedicated connector tiles, which
    /// is a different and much larger thing than this.
    #[must_use]
    pub fn recurses(self) -> bool {
        matches!(self, Figure::Carpet | Figure::Dust | Figure::Cross)
    }

    /// Whether the figure is stroked, and therefore has a line width.
    #[must_use]
    pub fn stroked(self) -> bool {
        matches!(self, Figure::Truchet | Figure::Weave)
    }

    /// Whether alternate rows are shifted half a cell.
    ///
    /// A scallop only reads as one when the row below breaks the joint of the
    /// row above, which is a brick offset, and a brick offset closes over the
    /// tile only if there is an **even** number of rows, because it is the row
    /// index's parity that decides the shift. An odd count puts two unshifted
    /// rows against each other at the join, which is a visible line straight
    /// across the fabric. [`Ornament::grid_on`] rounds to an even count for
    /// these figures; it is the same obligation as a band sequence closing and
    /// a grid's cells closing, in a fifth place.
    #[must_use]
    pub fn bricked(self) -> bool {
        matches!(self, Figure::Scale)
    }

    /// Whether the cell's coin turns the figure over.
    ///
    /// Truchet and Weave are two states and the coin chooses between them.
    /// Turning a scallop upside down at random would stop it being a scallop.
    #[must_use]
    fn turns(self) -> bool {
        matches!(self, Figure::Truchet | Figure::Weave)
    }

    /// Every figure, in the order an interface should offer them: the drawn
    /// ones first, then the recursive ones.
    #[must_use]
    pub fn all() -> [Figure; 6] {
        [
            Figure::Truchet,
            Figure::Weave,
            Figure::Scale,
            Figure::Carpet,
            Figure::Dust,
            Figure::Cross,
        ]
    }

    /// Which ninths survive a level of subdivision, as a nine-bit mask read
    /// row by row.
    fn surviving(self) -> u16 {
        match self {
            // Everything but the middle.
            Figure::Carpet => 0b111_101_111,
            // The four corners.
            Figure::Dust => 0b101_000_101,
            // The middle and the four beside it.
            Figure::Cross => 0b010_111_010,
            _ => 0b111_111_111,
        }
    }
}

/// A ground drawn by rule on a grid of cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ornament {
    pub figure: Figure,
    /// How big a cell wants to be. The grid is fitted to the tile, so what is
    /// drawn is the nearest whole number of cells to this.
    pub cell_mm: f32,
    /// How many times a recursive figure divides. One is the figure itself.
    pub depth: u32,
    /// Line width, for the figures that are stroked.
    pub stroke_mm: f32,
    /// How often a cell departs from the strict figure, between 0 and 1.
    ///
    /// At zero every cell is the classical figure and the seed does nothing:
    /// a Sierpinski carpet is a Sierpinski carpet. Above it, a block here and
    /// there is left whole instead of being divided, which is what turns a
    /// diagram into something that looks drawn.
    pub variation: f32,
    pub seed: u64,
    /// A slot per level, coarsest first. A figure with one level uses the
    /// first; a recursive one reads as tones stepping inward. [`GROUND_SLOT`]
    /// anywhere leaves the fabric showing.
    pub levels: Vec<u8>,
}

impl Default for Ornament {
    fn default() -> Self {
        Self {
            figure: Figure::Truchet,
            cell_mm: 24.0,
            depth: 3,
            stroke_mm: 4.0,
            variation: 0.0,
            seed: 1,
            levels: vec![1],
        }
    }
}

impl Ornament {
    /// Whether this puts anything on the fabric at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cell_mm <= 0.0
            || self.levels.is_empty()
            || self.levels.iter().all(|s| *s == GROUND_SLOT)
            || (self.figure.stroked() && self.stroke_mm <= 0.0)
    }

    /// Every slot this needs the palette to define.
    #[must_use]
    pub fn slots_used(&self) -> Vec<u8> {
        let mut out: Vec<u8> = self
            .levels
            .iter()
            .copied()
            .filter(|s| *s != GROUND_SLOT)
            .take(self.usable_levels())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// How many of the levels the figure can actually reach.
    fn usable_levels(&self) -> usize {
        if self.figure.recurses() {
            self.depth.clamp(1, DEEPEST) as usize
        } else {
            1
        }
    }

    /// Lay the grid on a tile: a whole number of cells each way.
    ///
    /// The same arithmetic that fits a band sequence, and the same reason.
    /// Cells come out near-square rather than exactly square, because a tile
    /// is whatever size the design is and two whole-number divisions of it
    /// rarely agree. At a 240 x 180 mm repeat and a 10 mm cell they agree
    /// exactly, and at 300 x 225 they differ by two per cent.
    #[must_use]
    pub fn grid_on(&self, tile_width_mm: f32, tile_height_mm: f32) -> Grid {
        let wanted = self.cell_mm.max(FINEST_CELL_MM);
        let cols = (tile_width_mm / wanted).round().max(1.0);
        // Rows from the *fitted* width, not from the asked-for cell size, so
        // that the two directions stay as close to each other as the tile lets
        // them rather than each rounding away on its own.
        let fitted = tile_width_mm / cols;
        let mut rows = (tile_height_mm / fitted).round().max(1.0);
        if self.figure.bricked() {
            // See `Figure::bricked`: the shift is decided by the row's parity,
            // so an odd count puts two unshifted rows against each other at
            // the join.
            rows = (rows / 2.0).round().max(1.0) * 2.0;
        }
        Grid {
            cols: cols as u32,
            rows: rows as u32,
            cell_w: tile_width_mm / cols,
            cell_h: tile_height_mm / rows,
        }
    }
}

/// The grid an ornament was fitted to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub cols: u32,
    pub rows: u32,
    pub cell_w: f32,
    pub cell_h: f32,
}

/// An ornament with a grid under it, ready to be sampled.
#[derive(Debug, Clone, PartialEq)]
pub struct OrnamentField {
    ornament: Ornament,
    grid: Grid,
}

impl OrnamentField {
    #[must_use]
    pub fn new(ornament: &Ornament, tile_width_mm: f32, tile_height_mm: f32) -> Self {
        Self {
            ornament: ornament.clone(),
            grid: ornament.grid_on(tile_width_mm, tile_height_mm),
        }
    }

    /// The grid this was fitted to, so an interface can say what it did with
    /// the cell size it was given.
    #[must_use]
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// The slot at a point in the tile, or `None` where the fabric shows.
    #[must_use]
    pub fn slot_at(&self, x_mm: f32, y_mm: f32) -> Option<u8> {
        let o = &self.ornament;
        if o.is_empty() || self.grid.cell_w <= 0.0 || self.grid.cell_h <= 0.0 {
            return None;
        }

        // Reduced into the tile before anything is divided by a cell size.
        // The renderer already hands over tile-local coordinates, and this is
        // the second lock on the same door: dividing 246 mm by a 30 mm cell
        // and dividing 6 mm by it do not give the same fraction in `f32`, and
        // a block boundary is exactly where that last bit decides the answer.
        let (tile_w, tile_h) = (
            self.grid.cols as f32 * self.grid.cell_w,
            self.grid.rows as f32 * self.grid.cell_h,
        );
        let x_mm = x_mm.rem_euclid(tile_w);
        let y_mm = y_mm.rem_euclid(tile_h);

        // Which cell, and where in it. Still wrapped, because reducing leaves
        // a coordinate that can sit exactly on the tile's far edge.
        let cy = (y_mm / self.grid.cell_h).floor();
        let row = (cy as i64).rem_euclid(i64::from(self.grid.rows)) as u64;
        let v = (y_mm / self.grid.cell_h - cy).clamp(0.0, 1.0);

        // The brick shift is applied to the coordinate before the column is
        // taken, so it moves the cell boundaries rather than the drawing
        // inside them. Wrapping by the tile adds a whole number of cells to
        // this, so the shift survives the join untouched.
        let shifted = x_mm / self.grid.cell_w
            + if self.ornament.figure.bricked() && row % 2 == 1 {
                0.5
            } else {
                0.0
            };
        let cx = shifted.floor();
        let col = (cx as i64).rem_euclid(i64::from(self.grid.cols)) as u64;
        let u = (shifted - cx).clamp(0.0, 1.0);

        let painted = if o.figure.recurses() {
            self.recursive(col, row, u, v)
        } else {
            self.drawn(col, row, u, v)
        };
        painted.filter(|s| *s != GROUND_SLOT)
    }

    /// The slot for a level, cycling if fewer were given than reached.
    fn level_slot(&self, level: usize) -> u8 {
        let levels = &self.ornament.levels;
        levels[level.min(levels.len() - 1)]
    }

    /// Truchet, Weave and Scale: one level, a coin from the cell's hash.
    fn drawn(&self, col: u64, row: u64, u: f32, v: f32) -> Option<u8> {
        // Turning the cell is reflecting it, which for these figures is the
        // second of the two states rather than a rotation of the first.
        let turned = self.ornament.figure.turns() && hash(self.ornament.seed, col, row, 0) & 1 == 1;
        let (u, v) = if turned { (1.0 - u, v) } else { (u, v) };

        let (cw, ch) = (self.grid.cell_w, self.grid.cell_h);
        let half = self.ornament.stroke_mm / 2.0;

        let hit = match self.ornament.figure {
            // Two quarter arcs, one about each of a pair of opposite corners,
            // each of radius half a cell, so both meet the cell edge at its
            // middle, and a neighbour meets them there whichever way it turned.
            Figure::Truchet => {
                let near = |ox: f32, oy: f32| {
                    let dx = (u - ox) * cw;
                    let dy = (v - oy) * ch;
                    let r = ((dx * dx) / (cw * cw / 4.0) + (dy * dy) / (ch * ch / 4.0)).sqrt();
                    // Scaled back into millimetres so the stroke is the width
                    // it was asked for rather than a share of a cell.
                    ((r - 1.0) * cw.min(ch) / 2.0).abs() <= half
                };
                near(0.0, 0.0) || near(1.0, 1.0)
            }
            // The straight version: the same two connections Truchet makes,
            // drawn as segments rather than arcs. Corner to corner was the
            // first attempt and is wrong: a diagonal meets the cell at its
            // *corners*, where a neighbour has nothing to meet it, and the
            // lines broke at every edge whose two cells had turned opposite
            // ways. These cut the corners instead, so both ends land on edge
            // middles like everything else here.
            Figure::Weave => {
                let (px, py) = (u * cw, v * ch);
                near_segment(px, py, 0.0, ch / 2.0, cw / 2.0, 0.0, half)
                    || near_segment(px, py, cw / 2.0, ch, cw, ch / 2.0, half)
            }
            // A filled semicircle standing on the cell's top edge, as wide as
            // the cell, so neighbouring scallops in a row are tangent and the
            // shifted row below breaks the joint between them.
            Figure::Scale => {
                let dx = (u - 0.5) * 2.0;
                let dy = v;
                dx * dx + dy * dy <= 1.0
            }
            _ => false,
        };
        hit.then(|| self.level_slot(0))
    }

    /// The square fractals: divide into ninths, keep the ones the figure
    /// keeps, and go again.
    fn recursive(&self, col: u64, row: u64, u: f32, v: f32) -> Option<u8> {
        let o = &self.ornament;
        let surviving = o.figure.surviving();
        let depth = o.depth.clamp(1, DEEPEST);
        let vary = o.variation.clamp(0.0, 1.0);

        let (mut u, mut v) = (u, v);
        // Absolute cell coordinates at each level, so that the hash addresses a
        // block rather than a position within its parent, since two blocks in the
        // same place at different levels must not draw the same coin.
        let (mut bx, mut by) = (col * 3, row * 3);

        for level in 0..depth {
            let ix = ((u * 3.0) as u32).min(2);
            let iy = ((v * 3.0) as u32).min(2);
            bx = bx * 3 + u64::from(ix);
            by = by * 3 + u64::from(iy);

            // A block left whole instead of divided. This is what `variation`
            // buys: the classical figure has every block divided the same way
            // at every level, which is legible as a diagram and not as cloth.
            if vary > 0.0 && coin(o.seed, bx, by, u64::from(level)) < vary {
                return Some(self.level_slot(level as usize));
            }

            let bit = iy * 3 + ix;
            if surviving & (1 << (8 - bit)) == 0 {
                return None;
            }

            u = (u * 3.0 - f32::from(ix as u16)).clamp(0.0, 1.0);
            v = (v * 3.0 - f32::from(iy as u16)).clamp(0.0, 1.0);
        }
        Some(self.level_slot(depth as usize - 1))
    }
}

/// Whether a point is within `half` of a segment, all in millimetres.
///
/// Squared throughout, so there is no square root and nothing to round.
fn near_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, half: f32) -> bool {
    let (ex, ey) = (bx - ax, by - ay);
    let len2 = ex * ex + ey * ey;
    if len2 <= 0.0 {
        return false;
    }
    // Clamped, so the ends are round caps rather than an infinite line: a
    // segment that ran on would paint straight through the neighbouring cell.
    let t = (((px - ax) * ex + (py - ay) * ey) / len2).clamp(0.0, 1.0);
    let (dx, dy) = (px - (ax + t * ex), py - (ay + t * ey));
    dx * dx + dy * dy <= half * half
}

/// A value per cell, addressed rather than drawn in order.
///
/// SplitMix64's finaliser, which is integer arithmetic throughout and so
/// carries none of the platform questions a floating-point generator would.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn hash(seed: u64, a: u64, b: u64, c: u64) -> u64 {
    mix(seed ^ mix(a ^ mix(b ^ mix(c))))
}

/// The same, as a number between 0 and 1.
///
/// Built from the top 24 bits over a power of two, so the division is exact and
/// the comparison against a threshold cannot land differently anywhere.
fn coin(seed: u64, a: u64, b: u64, c: u64) -> f32 {
    (hash(seed, a, b, c) >> 40) as f32 / 16_777_216.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: (f32, f32) = (240.0, 180.0);

    fn field(figure: Figure, over: impl Fn(&mut Ornament)) -> OrnamentField {
        let mut o = Ornament {
            figure,
            cell_mm: 20.0,
            stroke_mm: 4.0,
            seed: 9,
            levels: vec![1],
            ..Ornament::default()
        };
        over(&mut o);
        OrnamentField::new(&o, TILE.0, TILE.1)
    }

    /// What share of the tile a figure covers, by sampling.
    fn coverage(f: &OrnamentField) -> f32 {
        let (mut hit, mut total) = (0u32, 0u32);
        for i in 0..300 {
            for j in 0..300 {
                let x = (i as f32 + 0.5) / 300.0 * TILE.0;
                let y = (j as f32 + 0.5) / 300.0 * TILE.1;
                total += 1;
                if f.slot_at(x, y).is_some() {
                    hit += 1;
                }
            }
        }
        hit as f32 / total as f32
    }

    #[test]
    fn a_drawn_figure_reaches_the_cell_edge_only_at_its_middle() {
        // Truchet's observation, and the only reason a grid of independently
        // turned cells reads as continuous line work: every figure meets the
        // cell edge in a band about the middle and nowhere else, so whatever
        // two neighbours drew, they join. A figure that reached the edge
        // anywhere else would end in stubs all over the fabric wherever its
        // neighbour had turned the other way.
        //
        // Checked at every interior edge rather than at one, because the cells
        // are turned by a hash and an edge that happens to match on both sides
        // proves nothing.
        for figure in [Figure::Truchet, Figure::Weave] {
            let f = field(figure, |_| {});
            let grid = f.grid();
            let (eps, stroke) = (0.001, 4.0);
            for col in 1..grid.cols {
                let edge = col as f32 * grid.cell_w;
                for side in [-eps, eps] {
                    for row in 0..grid.rows {
                        let middle = (row as f32 + 0.5) * grid.cell_h;
                        assert!(
                            f.slot_at(edge + side, middle).is_some(),
                            "{figure:?} does not reach the middle of the edge at \
                             column {col}, row {row}"
                        );
                    }
                    for step in 0..200 {
                        let y = (step as f32 + 0.5) / 200.0 * TILE.1;
                        let from_middle =
                            (y / grid.cell_h - (y / grid.cell_h).floor() - 0.5).abs() * grid.cell_h;
                        if from_middle > stroke {
                            assert!(
                                f.slot_at(edge + side, y).is_none(),
                                "{figure:?} reaches the edge of column {col} at {y} mm, \
                                 {from_middle:.2} mm from the middle of its edge"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn an_arc_crosses_the_edge_without_a_step_in_it() {
        // The stronger form, which holds for the arcs and not for the
        // chevrons: an arc meets the edge at a right angle, so the painted set
        // either side of the join is the same set. A chevron meets it at
        // forty-five degrees and paints a little above the middle on one side
        // and a little below on the other. The line is continuous and its
        // footprint is not, which is why the test above states the property
        // the way it does rather than as an equality for both.
        let f = field(Figure::Truchet, |_| {});
        let grid = f.grid();
        for col in 1..grid.cols {
            let edge = col as f32 * grid.cell_w;
            for step in 0..200 {
                let y = (step as f32 + 0.5) / 200.0 * TILE.1;
                assert_eq!(
                    f.slot_at(edge - 0.001, y),
                    f.slot_at(edge + 0.001, y),
                    "an arc steps at the edge of column {col}, {y} mm down"
                );
            }
        }
    }

    #[test]
    fn the_grid_closes_over_the_tile() {
        // A whole number of cells each way, by the arithmetic that fits a band
        // sequence and for the same reason. The asked-for size is a wish.
        for cell in [7.0f32, 13.0, 20.0, 31.5, 100.0] {
            let f = field(Figure::Truchet, |o| o.cell_mm = cell);
            let grid = f.grid();
            assert!(
                (grid.cols as f32 * grid.cell_w - TILE.0).abs() < 1e-3,
                "{} cells of {} mm is not {} mm",
                grid.cols,
                grid.cell_w,
                TILE.0
            );
            assert!((grid.rows as f32 * grid.cell_h - TILE.1).abs() < 1e-3);
        }
    }

    #[test]
    fn a_bricked_figure_gets_an_even_number_of_rows() {
        // The shift is decided by the row index's parity, so an odd count
        // puts two unshifted rows against each other at the tile join.
        for cell in [7.0f32, 11.0, 13.0, 17.0, 20.0, 23.0, 29.0] {
            let scale = field(Figure::Scale, |o| o.cell_mm = cell);
            assert_eq!(
                scale.grid().rows % 2,
                0,
                "{cell} mm cells gave {} rows",
                scale.grid().rows
            );
        }
    }

    /// Where the scallops of a row sit, as an offset within one cell.
    ///
    /// Read near the bottom of the cell, where only a narrow band around each
    /// scallop's centre is painted. At the top every column is inside one, so
    /// there would be nothing to see.
    fn scallop_offset(f: &OrnamentField, row: u32) -> f32 {
        let grid = f.grid();
        let y = (row as f32 + 0.92) * grid.cell_h;
        for i in 0..2000 {
            let x = i as f32 / 2000.0 * grid.cell_w * 2.0;
            if f.slot_at(x, y).is_some() {
                return x % grid.cell_w;
            }
        }
        f32::NAN
    }

    #[test]
    fn an_odd_row_count_is_what_the_hazard_looks_like() {
        // Kept so that the rounding above is known to be preventing something.
        //
        // The failure is not a broken edge: every row is a complete band of
        // scallops and the pixels at the join line up fine. It is the *rhythm*:
        // the shift is the row index's parity, so with an odd count the last
        // row and the first row are both unshifted, and stacking two tiles
        // puts them against each other. A doubled row, once down the fabric.
        //
        // Built by hand rather than by disabling the rounding, so the fixture
        // and the engine stay independent.
        let ornament = Ornament {
            figure: Figure::Scale,
            cell_mm: 20.0,
            ..Ornament::default()
        };
        let odd = OrnamentField {
            ornament: ornament.clone(),
            grid: Grid {
                cols: 12,
                rows: 9,
                cell_w: 20.0,
                cell_h: 20.0,
            },
        };
        let gap = |f: &OrnamentField, rows: u32| {
            (scallop_offset(f, rows - 1) - scallop_offset(f, 0)).abs()
        };
        assert!(
            gap(&odd, 9) < 0.1,
            "an odd brick should put two rows of the same shift against each other \
             at the join, but they differed by {}",
            gap(&odd, 9)
        );

        // What the engine actually produces for the same request, for contrast.
        let even = OrnamentField::new(&ornament, TILE.0, TILE.1);
        let rows = even.grid().rows;
        assert!(
            gap(&even, rows) > even.grid().cell_w / 4.0,
            "the rows either side of the join should be half a cell apart, not {}",
            gap(&even, rows)
        );
    }

    #[test]
    fn every_figure_wraps_at_the_tile_edge() {
        // The same question asked of the join from the other side: a
        // coordinate past the tile belongs to the next repeat, and a renderer
        // walking a field of tiles hands over exactly that.
        for figure in Figure::all() {
            let f = field(figure, |o| {
                o.cell_mm = 30.0;
                o.depth = 3;
                o.variation = 0.3;
            });
            for i in 0..120 {
                let x = i as f32 * 2.0;
                let y = i as f32 * 1.5;
                assert_eq!(
                    f.slot_at(x, y),
                    f.slot_at(x + TILE.0, y + TILE.1),
                    "{figure:?} at {x}, {y}"
                );
                assert_eq!(f.slot_at(x, y), f.slot_at(x - TILE.0, y), "{figure:?}");
            }
        }
    }

    #[test]
    fn every_figure_draws_something_and_no_two_draw_the_same() {
        // A rule that paints nothing, or everything, is a ground that is not
        // there, and two figures that come out identical mean a mask or a
        // match arm is wrong rather than that the engine has six looks.
        let mut seen: Vec<(Figure, f32)> = Vec::new();
        for figure in Figure::all() {
            let f = field(figure, |o| {
                o.cell_mm = 30.0;
                o.depth = 3;
            });
            let share = coverage(&f);
            assert!(
                (0.02..0.98).contains(&share),
                "{figure:?} covers {:.1}% of the tile",
                share * 100.0
            );
            for (other, other_share) in &seen {
                assert!(
                    (share - other_share).abs() > 1e-3,
                    "{figure:?} and {other:?} came out the same"
                );
            }
            seen.push((figure, share));
        }
    }

    #[test]
    fn a_recursive_figure_is_the_same_figure_one_third_the_size() {
        // Self-similarity stated as an equality rather than admired in a
        // picture: a point inside a surviving block at depth n+1 is painted
        // exactly when the corresponding point of the whole figure is painted
        // at depth n. If the levels did not nest, this would be a grid of
        // unrelated marks rather than a fractal.
        for figure in [Figure::Carpet, Figure::Dust, Figure::Cross] {
            let shallow = field(figure, |o| {
                o.cell_mm = TILE.0;
                o.depth = 2;
            });
            let deep = field(figure, |o| {
                o.cell_mm = TILE.0;
                o.depth = 3;
            });
            let cell = shallow.grid();

            // The top-left ninth, which survives in all three figures except
            // Cross, so take the middle ninth for that one.
            let (bx, by) = if figure == Figure::Cross {
                (1.0, 1.0)
            } else {
                (0.0, 0.0)
            };
            let mut compared = 0;
            for i in 0..60 {
                for j in 0..60 {
                    let (u, v) = ((i as f32 + 0.5) / 60.0, (j as f32 + 0.5) / 60.0);
                    let whole = shallow.slot_at(u * cell.cell_w, v * cell.cell_h);
                    let third =
                        deep.slot_at((bx + u) / 3.0 * cell.cell_w, (by + v) / 3.0 * cell.cell_h);
                    assert_eq!(whole, third, "{figure:?} at {u}, {v}");
                    compared += 1;
                }
            }
            assert_eq!(compared, 3600);
        }
    }

    #[test]
    fn the_seed_moves_a_varied_figure_and_leaves_a_strict_one_alone() {
        // `variation` is what the seed is for. At zero the figure is the
        // classical one and asking for another of the same is meaningless,
        // a Sierpinski carpet being a Sierpinski carpet, so the control should be
        // seen to do nothing there rather than appearing to work.
        let strict = |seed: u64| {
            field(Figure::Carpet, |o| {
                o.seed = seed;
                o.depth = 3;
                o.variation = 0.0;
            })
        };
        assert_eq!(coverage(&strict(1)), coverage(&strict(2)));

        let varied = |seed: u64| {
            field(Figure::Carpet, |o| {
                o.seed = seed;
                o.depth = 3;
                o.variation = 0.3;
            })
        };
        assert_ne!(coverage(&varied(1)), coverage(&varied(2)));
    }

    #[test]
    fn the_same_ornament_is_identical_on_every_platform() {
        // Pinned like the texture's field and for the same reason: this is a
        // claim about machines that are not this one. Integer arithmetic and
        // `sqrt` throughout, so there is nothing here a platform is allowed to
        // round differently, but the way to know that is to check it
        // somewhere else, and CI is Linux while this is macOS.
        //
        // Recorded 2026-09-24, aarch64-apple-darwin.
        let mut digest = 0xcbf2_9ce4_8422_2325u64;
        for figure in Figure::all() {
            let f = field(figure, |o| {
                o.cell_mm = 27.0;
                o.depth = 3;
                o.variation = 0.25;
            });
            for i in 0..97 {
                for j in 0..71 {
                    let slot = f.slot_at(i as f32 * 2.3, j as f32 * 2.1).unwrap_or(0);
                    digest ^= u64::from(slot) + 1;
                    digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        assert_eq!(digest, 0x389f_7833_2c82_1794, "ornament digest moved");
    }

    #[test]
    fn an_ornament_with_nothing_to_paint_is_empty() {
        assert!(Ornament {
            levels: vec![],
            ..Ornament::default()
        }
        .is_empty());
        assert!(Ornament {
            levels: vec![GROUND_SLOT],
            ..Ornament::default()
        }
        .is_empty());
        assert!(Ornament {
            cell_mm: 0.0,
            ..Ornament::default()
        }
        .is_empty());
        assert!(
            Ornament {
                figure: Figure::Truchet,
                stroke_mm: 0.0,
                ..Ornament::default()
            }
            .is_empty(),
            "a line with no width is not a line"
        );
        assert!(!Ornament::default().is_empty());
    }

    #[test]
    fn only_the_levels_a_figure_reaches_cost_a_colour() {
        // A screen printer charges by the colour. A figure that does not
        // recurse has one level however many were named, and a recursive one
        // has as many as its depth. Counting the rest would quote for screens
        // that are never cut.
        let deep = Ornament {
            figure: Figure::Carpet,
            depth: 2,
            levels: vec![1, 2, 3, 4],
            ..Ornament::default()
        };
        assert_eq!(deep.slots_used(), vec![1, 2]);

        let flat = Ornament {
            figure: Figure::Truchet,
            levels: vec![1, 2, 3, 4],
            ..Ornament::default()
        };
        assert_eq!(flat.slots_used(), vec![1]);
    }
}
