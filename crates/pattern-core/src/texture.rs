//! Pattern grown rather than drawn: a reaction–diffusion ground.
//!
//! Every other ground in this engine is laid out. Bands are placed, motifs are
//! placed, grids are placed. This one is *simulated*: two notional chemicals
//! spread across the tile and consume each other, and after a few thousand
//! steps the field has organised itself into spots, or stripes, or a maze, or
//! coral, depending on two numbers. Nobody positions anything.
//!
//! It is here because it produces the one thing placement cannot: pattern with
//! no visible grid in it. A tossed layout still repeats on a lattice and the
//! eye finds it; a Gray–Scott field has no lattice to find, only the tile's own
//! period.
//!
//! # Why it tiles
//!
//! The field lives on a torus. Every Laplacian reads its neighbours modulo the
//! grid, so the left column's left neighbour *is* the right column, not a copy
//! of it, the same cells, in the same simulation. The result is seamless by
//! construction rather than by being fixed up at the edges afterwards, which is
//! the only way this could work: a blend or a mirror at the seam would be
//! visible precisely because the interior has no straight lines to hide it in.
//!
//! That also means a texture ground, like a band ground, needs a block lattice.
//! A half-drop tile's right edge does not meet its own left edge, and a torus
//! is exactly the claim that it does.
//!
//! # Why it is deterministic
//!
//! The simulation is `+`, `-` and `*` on `f32` and nothing else: no `exp`, no
//! `powf`, no library transcendentals, whose results are permitted to differ by
//! an ulp between platforms. Those four operations are exactly rounded by
//! IEEE 754 and Rust does not contract them into fused multiply-adds, so the
//! same seed gives the same field bit for bit on every machine the CI matrix
//! covers. A test asserts the whole field's checksum; do not weaken it.
//!
//! # What is stored
//!
//! Parameters, never the field. A 192-cell grid is 36,864 floats, which is
//! larger than every other part of a document put together and would go stale
//! the moment anything about it changed. The field is derived when the ground
//! is laid over a tile; see [`Ground::laid_over`](crate::Ground::laid_over).

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::palette::GROUND_SLOT;

/// Diffusion rate of the substrate, per step.
///
/// Two thirds rather than the 0.16 that appears in most write-ups, because
/// those use the bare five-point Laplacian and this uses the normalised
/// nine-point one, which is about four times smaller for the same field. The
/// rate has to rise to match or nothing diffuses far enough to interact:
/// measured at 0.16 the seeds freeze where they are put and the tile is a
/// scatter of four-cell squares, and at 1.0 the reagent smears out faster than
/// it can be made and the field dies flat within a hundred steps.
const DIFFUSION_U: f32 = 0.64;

/// Diffusion rate of the reagent, half the substrate's.
///
/// The ratio is what makes structure appear at all. Equal rates smooth into
/// nothing however long they run.
const DIFFUSION_V: f32 = 0.32;

/// The largest grid a texture may be simulated on.
///
/// Cost is cells squared times steps, and this is the number that stops a
/// mistyped parameter turning a preview into a minute of arithmetic.
pub const MOST_CELLS: u32 = 512;

/// The largest number of steps.
pub const MOST_STEPS: u32 = 20_000;

/// A named region of the Gray–Scott parameter space.
///
/// The map from `(feed, kill)` to what the fabric looks like is famously
/// unfriendly. The interesting band is a sliver a few thousandths wide, and
/// either side of it the field dies out to flat or floods to solid. These are
/// coordinates known to sit inside it, so that choosing a look does not mean
/// hunting for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Growth {
    /// Round dots, irregularly spaced, that never touch.
    Spots,
    /// Irregular blobs, some of them broken into crescents and rings. The
    /// least regular of them, and the nearest to something drawn by hand.
    Pebbles,
    /// Open rings, overlapping where they meet.
    Rings,
    /// Meandering dashes of even width. Fingerprint, or thumbprint paisley.
    Worms,
    /// Worms that join up: a labyrinth with no dead space.
    Maze,
    /// Dense reticulated growth, in the way of brain coral.
    Coral,
}

impl Growth {
    /// The feed and kill rates that produce it.
    #[must_use]
    pub fn rates(self) -> (f32, f32) {
        // Found by sweeping, not by citation. The rates quoted in the usual
        // write-ups are for the five-point Laplacian and are dead here: at
        // 0.078 / 0.061, the commonly cited "worms", this model flattens
        // within a few hundred steps. A test walks all five and fails any
        // that covers almost none or almost all of the tile, which is what
        // being outside the live region looks like from the outside.
        match self {
            Growth::Spots => (0.034, 0.065),
            Growth::Pebbles => (0.018, 0.053),
            Growth::Rings => (0.058, 0.057),
            Growth::Worms => (0.034, 0.061),
            Growth::Maze => (0.026, 0.055),
            Growth::Coral => (0.050, 0.061),
        }
    }

    /// Every look, in the order an interface should offer them.
    #[must_use]
    pub fn all() -> [Growth; 6] {
        [
            Growth::Spots,
            Growth::Pebbles,
            Growth::Rings,
            Growth::Worms,
            Growth::Maze,
            Growth::Coral,
        ]
    }
}

/// A reaction–diffusion ground.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Texture {
    /// Which region of the parameter space to grow in.
    pub growth: Growth,
    /// Cells across the tile's width. Height follows from the tile's shape, so
    /// that a cell is square and the pattern is not stretched.
    ///
    /// This is the pattern's *scale*, not its quality: doubling it does not
    /// resolve the same picture more finely, it grows twice as many spots.
    pub cells: u32,
    /// How long to let it run.
    ///
    /// The field settles: measured on a 192-cell grid, coverage is within half
    /// a percent of its final value by two thousand steps and does not move
    /// again by four thousand. Stopping short of that gives a rougher, more
    /// broken version of the same look, which is sometimes what is wanted.
    pub steps: u32,
    /// The seed. Same seed, same field, on every machine.
    pub seed: u64,
    /// Slots from the lowest concentration to the highest, in order.
    ///
    /// Two is the usual answer and gives a silhouette. More gives contours,
    /// which read as a woven or marbled version of the same shape. Any of them
    /// may be [`GROUND_SLOT`], which leaves the fabric showing through.
    pub levels: Vec<u8>,
}

impl Default for Texture {
    fn default() -> Self {
        Self {
            growth: Growth::Worms,
            cells: 192,
            steps: 2_500,
            seed: 1,
            levels: vec![GROUND_SLOT, 1],
        }
    }
}

impl Texture {
    /// A texture of one growth, in one slot on the bare fabric.
    #[must_use]
    pub fn new(growth: Growth, slot: u8) -> Self {
        Self {
            growth,
            levels: vec![GROUND_SLOT, slot],
            ..Self::default()
        }
    }

    /// Whether this puts anything on the fabric at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells == 0 || self.levels.len() < 2 || self.levels.iter().all(|s| *s == GROUND_SLOT)
    }

    /// Every slot this needs the palette to define.
    ///
    /// Answered from the parameters without simulating, because the caller
    /// asking is usually counting colours for a quote and should not pay for a
    /// field to find out. A level that ends up covering none of the tile is
    /// still counted here; see [`SlotField::share_of`] for what actually
    /// landed.
    #[must_use]
    pub fn slots_used(&self) -> Vec<u8> {
        let mut out: Vec<u8> = self
            .levels
            .iter()
            .copied()
            .filter(|s| *s != GROUND_SLOT)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Grow the field and quantise it onto slots.
    ///
    /// The tile's proportions decide the grid's, so that cells stay square. A
    /// 300 x 225 mm tile at 192 cells is 192 x 144.
    #[must_use]
    pub fn grown(&self, tile_width_mm: f32, tile_height_mm: f32) -> SlotField {
        let cols = self.cells.clamp(1, MOST_CELLS);
        let rows = if tile_width_mm > 0.0 && tile_height_mm > 0.0 {
            let wanted = (cols as f32 * tile_height_mm / tile_width_mm).round();
            (wanted as u32).clamp(1, MOST_CELLS)
        } else {
            cols
        };

        let v = simulate(
            cols,
            rows,
            self.growth.rates(),
            self.steps.min(MOST_STEPS),
            self.seed,
        );
        quantise(&v, cols, rows, &self.levels)
    }
}

/// A grown field: a concentration per cell, and the slots it is cut into.
///
/// The concentrations are kept rather than reduced to a slot per cell, and
/// that is the difference between a preview and something that prints. A cell
/// is 1.5 mm at a typical scale, which is eighteen pixels at print resolution:
/// classifying per cell puts an eighteen-pixel staircase along every curve. A
/// sample taken *between* cells and then cut into levels gives an edge that
/// stays smooth at any output resolution, and the cell count goes back to
/// meaning what it should, which is how large the shapes are, not how ragged.
///
/// Normalised to 0..1 on the way in, so the thresholds are fixed and a caller
/// cannot re-derive them differently somewhere else.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotField {
    cols: u32,
    rows: u32,
    /// Row-major, one normalised concentration per cell.
    values: Vec<f32>,
    /// Slots from the lowest level to the highest. Empty means a field with
    /// nothing to cut, which paints nothing.
    levels: Vec<u8>,
}

impl SlotField {
    /// The slot at a point in the tile, or `None` where the fabric shows.
    ///
    /// Bilinear on the torus: the four cells around the sample, wrapped, so a
    /// point in the last half-cell of the tile blends with the first cell of
    /// the next tile along rather than with a clamped copy of its own edge.
    /// That wrap is what keeps the join invisible at the one place a seam
    /// would be easiest to see.
    #[must_use]
    pub fn slot_at(&self, x: f32, y: f32, tile_width_mm: f32, tile_height_mm: f32) -> Option<u8> {
        if self.levels.is_empty() || tile_width_mm <= 0.0 || tile_height_mm <= 0.0 {
            return None;
        }

        // Cell centres sit at half-integers, so shift by half a cell before
        // taking the surrounding pair. Without it the interpolation is offset
        // by half a cell and the field drifts against the tile.
        let fx = (x / tile_width_mm) * self.cols as f32 - 0.5;
        let fy = (y / tile_height_mm) * self.rows as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);

        let (cx0, cy0) = (wrapped(x0, self.cols), wrapped(y0, self.rows));
        let (cx1, cy1) = ((cx0 + 1) % self.cols, (cy0 + 1) % self.rows);
        let at = |c: u32, r: u32| self.values[(r * self.cols + c) as usize];

        let top = at(cx0, cy0) + tx * (at(cx1, cy0) - at(cx0, cy0));
        let bottom = at(cx0, cy1) + tx * (at(cx1, cy1) - at(cx0, cy1));
        self.level(top + ty * (bottom - top))
    }

    /// Which slot a normalised concentration falls in.
    fn level(&self, value: f32) -> Option<u8> {
        let last = self.levels.len() - 1;
        let i = (value * self.levels.len() as f32) as usize;
        let slot = self.levels[i.min(last)];
        (slot != GROUND_SLOT).then_some(slot)
    }

    /// The share of the tile a slot covers, between 0 and 1.
    ///
    /// Counted per cell, which is the field's own resolution rather than the
    /// render's. The interface uses it to say when a colour the design is
    /// paying for has barely appeared, which a reaction can do: two levels may
    /// land either side of a threshold nothing in the field crosses.
    #[must_use]
    pub fn share_of(&self, slot: u8) -> f32 {
        if self.values.is_empty() || self.levels.is_empty() {
            return 0.0;
        }
        let n = self
            .values
            .iter()
            .filter(|v| self.level(**v) == Some(slot))
            .count();
        n as f32 / self.values.len() as f32
    }

    /// Cells across and down.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.cols, self.rows)
    }
}

/// A cell index wrapped into the grid.
fn wrapped(index: f32, extent: u32) -> u32 {
    (index as i64).rem_euclid(extent as i64) as u32
}

/// Gray–Scott on a torus, returning the reagent concentration per cell.
///
/// The substrate `u` starts full and the reagent `v` starts empty but for a few
/// seeded patches. Both diffuse; where they meet, `u` is consumed to make more
/// `v`, `v` decays, and `u` is topped up. That is the entire model, and the
/// structure is emergent rather than encoded anywhere in it.
fn simulate(cols: u32, rows: u32, (feed, kill): (f32, f32), steps: u32, seed: u64) -> Vec<f32> {
    let n = (cols * rows) as usize;
    let mut u = vec![1.0f32; n];
    let mut v = vec![0.0f32; n];
    seed_patches(&mut u, &mut v, cols, rows, seed);
    run(&mut u, &mut v, cols, rows, (feed, kill), steps);
    v
}

/// The stepping itself, separated from how the field was seeded.
///
/// Split out so that a test can start from an initial condition it chose,
/// which is the only way to state the toroidal property as an equality: a
/// patch at the origin and the same patch moved ten cells along must give the
/// same field, moved ten cells along. Seeding at random cannot express that.
fn run(
    u: &mut Vec<f32>,
    v: &mut Vec<f32>,
    cols: u32,
    rows: u32,
    (feed, kill): (f32, f32),
    steps: u32,
) {
    let n = (cols * rows) as usize;
    let (mut next_u, mut next_v) = (vec![0.0f32; n], vec![0.0f32; n]);
    for _ in 0..steps {
        for row in 0..rows {
            // Neighbour rows, wrapped once per row rather than once per cell.
            let up = (row + rows - 1) % rows;
            let down = (row + 1) % rows;
            for col in 0..cols {
                let left = (col + cols - 1) % cols;
                let right = (col + 1) % cols;
                let at = |r: u32, c: u32| (r * cols + c) as usize;
                let i = at(row, col);

                // The nine-point Laplacian. Orthogonal neighbours count for
                // 0.2 and diagonal ones for 0.05, which is the standard
                // isotropic weighting. A five-point stencil grows visibly
                // square spots, and on textile the eye reads that as a fault.
                let lap = |f: &[f32]| {
                    0.2 * (f[at(up, col)] + f[at(down, col)] + f[at(row, left)] + f[at(row, right)])
                        + 0.05
                            * (f[at(up, left)]
                                + f[at(up, right)]
                                + f[at(down, left)]
                                + f[at(down, right)])
                        - f[i]
                };

                let reaction = u[i] * v[i] * v[i];
                next_u[i] = u[i] + DIFFUSION_U * lap(u) - reaction + feed * (1.0 - u[i]);
                next_v[i] = v[i] + DIFFUSION_V * lap(v) + reaction - (feed + kill) * v[i];
            }
        }
        std::mem::swap(u, &mut next_u);
        std::mem::swap(v, &mut next_v);
    }
}

/// Drop a handful of reagent patches for the reaction to start from.
///
/// Patches rather than uniform noise: Gray–Scott from noise alone takes
/// thousands of steps to find structure and often settles into a flat field
/// instead. A few square seeds grow outward and meet, which is both faster and
/// what gives the field its organic irregularity.
fn seed_patches(u: &mut [f32], v: &mut [f32], cols: u32, rows: u32, seed: u64) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Scaled to area rather than fixed, and in *cells* rather than as a share
    // of the tile: the reaction spreads at its own rate per step, so seeds a
    // constant number of cells apart take a constant number of steps to meet
    // however large the grid is. Seeding a 384-cell grid like a 96-cell one
    // would leave it still filling in when the simulation stopped.
    let patches = ((cols * rows) / CELLS_PER_PATCH).clamp(4, 512);

    for _ in 0..patches {
        let cx = rng.next_u32() % cols;
        let cy = rng.next_u32() % rows;
        for dy in 0..PATCH {
            for dx in 0..PATCH {
                let i = (((cy + dy) % rows) * cols + (cx + dx) % cols) as usize;
                u[i] = 0.5;
                v[i] = 0.25;
            }
        }
    }
}

/// One seed patch per this many cells.
const CELLS_PER_PATCH: u32 = 236;

/// How many cells across each seed patch is.
///
/// Too small and it diffuses away before the reaction catches; measured, a
/// single cell dies at these diffusion rates and four across does not.
const PATCH: u32 = 5;

/// Normalise the field to 0..1 and record the levels it will be cut into.
///
/// Normalised between the field's own lowest and highest concentration, and
/// the levels are then evenly spaced by *value*, not by area: a spot pattern is
/// mostly empty, and splitting it into equal shares would turn a scatter of
/// dots into half the fabric. The cost is that a level can come out covering
/// nothing, which is why [`SlotField::share_of`] exists.
fn quantise(v: &[f32], cols: u32, rows: u32, levels: &[u8]) -> SlotField {
    let blank = SlotField {
        cols,
        rows,
        values: vec![0.0; v.len()],
        levels: Vec::new(),
    };
    if levels.len() < 2 {
        return blank;
    }

    let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
    for c in v {
        if c.is_finite() {
            low = low.min(*c);
            high = high.max(*c);
        }
    }
    // A field that never got going is flat, and a flat field has no levels to
    // cut. Better bare fabric than an arbitrary one of them covering the tile.
    if !(high - low).is_finite() || high - low <= 1e-6 {
        return blank;
    }

    let span = high - low;
    SlotField {
        cols,
        rows,
        values: v.iter().map(|c| (c - low) / span).collect(),
        levels: levels.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A patch of reagent at a chosen cell, so a test can control the initial
    /// condition instead of taking what the seed gives.
    fn patched(cols: u32, rows: u32, at: (u32, u32), radius: u32) -> (Vec<f32>, Vec<f32>) {
        let n = (cols * rows) as usize;
        let (mut u, mut v) = (vec![1.0f32; n], vec![0.0f32; n]);
        for dy in 0..radius {
            for dx in 0..radius {
                let x = (at.0 + dx) % cols;
                let y = (at.1 + dy) % rows;
                let i = (y * cols + x) as usize;
                u[i] = 0.5;
                v[i] = 0.25;
            }
        }
        (u, v)
    }

    /// A cheap, order-sensitive digest of a field's exact bits.
    fn checksum(field: &[f32]) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for c in field {
            h ^= u64::from(c.to_bits());
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    #[test]
    fn the_field_is_grown_on_a_torus() {
        // The property the whole thing rests on. If the Laplacian clamped or
        // reflected at the edges instead of wrapping, a patch near a boundary
        // would behave differently from the same patch in the middle, and the
        // tile would not meet itself, which on fabric is a visible line down
        // every repeat, found after printing.
        //
        // Stated as an equality rather than an inspection: the same patch,
        // started somewhere else, must give the same field moved by the same
        // amount. Exactly, not approximately.
        const COLS: u32 = 48;
        const ROWS: u32 = 32;
        let shift = (17u32, 29u32);

        let rates = Growth::Worms.rates();
        let (mut u_a, mut v_a) = patched(COLS, ROWS, (4, 5), 6);
        run(&mut u_a, &mut v_a, COLS, ROWS, rates, 400);

        let (mut u_b, mut v_b) =
            patched(COLS, ROWS, ((4 + shift.0) % COLS, (5 + shift.1) % ROWS), 6);
        run(&mut u_b, &mut v_b, COLS, ROWS, rates, 400);

        for y in 0..ROWS {
            for x in 0..COLS {
                let a = v_a[(y * COLS + x) as usize];
                let b = v_b[(((y + shift.1) % ROWS) * COLS + (x + shift.0) % COLS) as usize];
                assert_eq!(
                    a.to_bits(),
                    b.to_bits(),
                    "cell ({x}, {y}) is not the same cell moved"
                );
            }
        }
        assert!(
            v_a.iter().any(|c| *c > 0.1),
            "the reaction died, so the comparison above compared two empty fields"
        );
    }

    #[test]
    fn the_same_seed_grows_the_same_field() {
        let t = Texture {
            cells: 40,
            steps: 300,
            seed: 7,
            ..Texture::default()
        };
        assert_eq!(t.grown(100.0, 100.0), t.grown(100.0, 100.0));

        let other = Texture {
            seed: 8,
            ..t.clone()
        };
        assert_ne!(
            t.grown(100.0, 100.0),
            other.grown(100.0, 100.0),
            "a different seed gave the same field, so the seed is not reaching the simulation"
        );
    }

    #[test]
    fn the_field_is_identical_on_every_platform() {
        // Pinned, like a golden image, and for the same reason: cross-platform
        // determinism is a claim about machines this one is not, so it has to
        // be checked somewhere else: CI is Linux, development is macOS.
        //
        // Regenerate only when the model itself is meant to change, and review
        // what moved. Never to make a failing test pass.
        //
        // Recorded 2026-09-23, aarch64-apple-darwin.
        let (mut u, mut v) = patched(32, 32, (8, 8), 5);
        run(&mut u, &mut v, 32, 32, Growth::Maze.rates(), 250);
        assert_eq!(checksum(&v), 0x01eb_0bee_b5c3_df95, "field checksum moved");
    }

    #[test]
    fn cells_are_square_whatever_shape_the_tile_is() {
        // Otherwise a texture stretches with the tile: the same parameters on a
        // 300 x 225 mm repeat and a 300 x 900 mm one would give circles in one
        // and ellipses in the other.
        let t = Texture {
            cells: 192,
            steps: 1,
            ..Texture::default()
        };
        assert_eq!(t.grown(300.0, 225.0).size(), (192, 144));
        assert_eq!(t.grown(300.0, 300.0).size(), (192, 192));
        assert_eq!(t.grown(200.0, 400.0).size(), (192, 384));
    }

    #[test]
    fn every_named_growth_is_a_different_pattern_and_none_of_them_dies() {
        // Two claims in one place because they fail together. A mistyped rate
        // either duplicates another preset or falls outside the narrow band
        // where Gray-Scott does anything at all, and outside it the field
        // flattens and quantises to bare fabric, a ground that silently is
        // not there.
        let mut seen: Vec<(Growth, f32)> = Vec::new();
        for growth in Growth::all() {
            let t = Texture {
                growth,
                cells: 64,
                steps: 2_000,
                seed: 3,
                levels: vec![GROUND_SLOT, 1],
            };
            let field = t.grown(100.0, 100.0);
            let share = field.share_of(1);
            assert!(
                (0.02..0.98).contains(&share),
                "{growth:?} covers {:.1}% of the tile, which is not a pattern",
                share * 100.0
            );
            for (other, other_share) in &seen {
                assert!(
                    (share - other_share).abs() > 1e-4,
                    "{growth:?} and {other:?} came out the same"
                );
            }
            seen.push((growth, share));
        }
    }

    #[test]
    fn a_ground_slot_level_leaves_the_fabric_showing() {
        let t = Texture {
            cells: 48,
            steps: 800,
            levels: vec![GROUND_SLOT, 4],
            ..Texture::default()
        };
        let field = t.grown(100.0, 100.0);
        assert!(
            field.share_of(GROUND_SLOT) < 1e-6,
            "slot 0 is absence, not a colour"
        );
        assert!(field.share_of(4) > 0.0);
        assert_eq!(
            t.slots_used(),
            vec![4],
            "the fabric is not a colour the print pays for"
        );
    }

    #[test]
    fn more_levels_means_more_slots() {
        let t = Texture {
            levels: vec![GROUND_SLOT, 2, 3, 2],
            ..Texture::default()
        };
        assert_eq!(
            t.slots_used(),
            vec![2, 3],
            "named twice is still one colour"
        );
    }

    #[test]
    fn sampling_wraps_at_the_tile_edge() {
        // A point a hair past the tile belongs to the next tile's first cell,
        // not to this one's last. Clamping instead would smear the final
        // column across the join.
        let t = Texture {
            cells: 16,
            steps: 400,
            seed: 2,
            ..Texture::default()
        };
        let field = t.grown(100.0, 100.0);
        for y in [0.0f32, 12.5, 61.0, 99.9] {
            assert_eq!(
                field.slot_at(0.1, y, 100.0, 100.0),
                field.slot_at(100.1, y, 100.0, 100.0)
            );
            assert_eq!(
                field.slot_at(0.1, y, 100.0, 100.0),
                field.slot_at(-99.9, y, 100.0, 100.0)
            );
        }
    }

    #[test]
    fn a_texture_with_nothing_to_paint_is_empty() {
        assert!(Texture {
            levels: vec![],
            ..Texture::default()
        }
        .is_empty());
        assert!(Texture {
            levels: vec![GROUND_SLOT, GROUND_SLOT],
            ..Texture::default()
        }
        .is_empty());
        assert!(Texture {
            cells: 0,
            ..Texture::default()
        }
        .is_empty());
        assert!(!Texture::default().is_empty());
    }

    #[test]
    fn a_level_boundary_falls_between_cells_rather_than_on_one() {
        // What keeps the print off a staircase. A cell is about 1.5 mm at a
        // typical scale, which is eighteen pixels at print resolution:
        // classifying per cell would put an eighteen-pixel step along every
        // curve, and the cell count would be controlling the raggedness of the
        // edges rather than the size of the shapes.
        //
        // Three cells rising 0, 0, 1 and three levels. The lowest threshold is
        // a third of the way up, so it is crossed a third of the way from the
        // middle cell's centre to the last one's - at 61% of the tile, where
        // the cells themselves change at 67%.
        let field = quantise(&[0.0, 0.0, 1.0], 3, 1, &[GROUND_SLOT, 1, 2]);
        let tile = 300.0;

        assert_eq!(
            field.slot_at(150.0, 50.0, tile, 100.0),
            None,
            "the middle cell"
        );
        assert_eq!(
            field.slot_at(0.64 * tile, 50.0, tile, 100.0),
            Some(1),
            "past the first threshold but short of the cell boundary"
        );
        assert_eq!(field.slot_at(0.58 * tile, 50.0, tile, 100.0), None);
    }

    #[test]
    fn the_interpolation_wraps_too() {
        // The one place a seam would be easiest to see. The last half-cell of
        // the tile blends with the *first* cell, not with a clamped copy of
        // its own edge, so a sample just inside the right edge and the same
        // sample just inside the left edge of the next tile agree.
        let field = quantise(&[1.0, 0.0, 0.0, 0.0], 4, 1, &[GROUND_SLOT, 1, 2]);
        let tile = 400.0;
        for x in [0.0f32, 1.0, 12.0, 399.0, 399.9] {
            assert_eq!(
                field.slot_at(x, 50.0, tile, 100.0),
                field.slot_at(x + tile, 50.0, tile, 100.0),
                "at {x} mm"
            );
        }
        // Half a cell short of the right edge, with only the cell at the *left*
        // edge painted. Clamping instead of wrapping would blend this sample
        // with a copy of its own cell, which is bare fabric, and the shape at
        // the left edge would stop dead at the join.
        assert_eq!(field.slot_at(399.0, 50.0, tile, 100.0), Some(1));
    }

    #[test]
    fn a_flat_field_paints_nothing_rather_than_something_arbitrary() {
        // Zero steps leaves the seeded patches and nothing else; a thousand
        // steps at rates outside the interesting band flattens to a constant.
        // Either way there are no levels to cut, and inventing one would put a
        // solid colour on the fabric that the designer did not ask for.
        let flat = quantise(&vec![0.3f32; 64], 8, 8, &[GROUND_SLOT, 1]);
        assert_eq!(flat.share_of(1), 0.0);
        assert!(flat.slot_at(1.0, 1.0, 10.0, 10.0).is_none());
    }
}
