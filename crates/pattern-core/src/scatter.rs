use glam::Vec2;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::lattice::Lattice;
use crate::mirror::{
    fold_into_cell, mirror_distance_sq, reflect, self_image_distance_sq, snap_to_axis, Mirror,
};
use crate::placement::Placement;

/// Candidate attempts around an active sample before it is retired.
///
/// Bridson's `k`. Thirty is the usual figure and the quality gain above it is
/// not visible; below about ten the distribution starts leaving gaps.
const CANDIDATES_PER_SAMPLE: u32 = 30;

/// How densely Poisson-disc sampling packs, as a fraction of hexagonal packing.
///
/// Points at minimum separation `r` pack most densely on a hexagonal lattice,
/// at `2 / (sqrt(3) * r^2)` points per unit area. Dart-throwing reaches a
/// consistent fraction of that rather than all of it, because it stops when no
/// candidate fits rather than when the space is optimally filled.
///
/// Measured against this sampler rather than taken from the literature, which
/// quotes a higher figure under a different normalisation. Across radii from 8
/// to 40 mm, two tile sizes and both lattices, twelve seeds each, this
/// implementation sits at 0.53 with no visible trend (measured 2026-09-20).
///
/// `packing_efficiency_is_what_the_density_solver_assumes` re-measures it and
/// fails if the sampler drifts, because the density control silently stops
/// meaning what it says when it does. Note that a scatter of only a handful of
/// motifs varies much more widely around this figure than a dense one — with
/// ten or so motifs a single seed can land 10% either side.
const PACKING_EFFICIENCY: f32 = 0.53;

/// One motif available to a scatter, and how often it should appear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MotifWeight {
    pub motif_id: String,
    /// Relative, not a probability: weights need not sum to anything. A print
    /// that is 60% cherries and 10% pineapples is written as 6 and 1.
    pub weight: f32,
    /// Needed to turn a coverage target into a separation radius, and to keep
    /// a large motif from being packed as tightly as a small one.
    pub natural_size_mm: Vec2,
    /// How much of its bounding box the motif's ink actually reaches, as a
    /// fraction of half the box diagonal. See
    /// [`IndexedMotif::coverage_radius_fraction`](crate::IndexedMotif::coverage_radius_fraction).
    ///
    /// Defaults to 1 — the whole box — because a caller that does not know the
    /// shape should be spaced conservatively rather than allowed to overlap.
    #[serde(default = "full_box")]
    pub radius_fraction: f32,
}

fn full_box() -> f32 {
    1.0
}

/// Everything a scatter needs that is not the tile or the lattice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScatterParams {
    /// Target coverage as a fraction of tile area, before overlap. Solved into
    /// a separation radius internally: nobody thinks in minimum separation
    /// radii, and exposing one would make the control meaningless whenever
    /// motif size changed.
    pub density: f32,
    /// Maximum rotation either way, in degrees. Fruit reads badly when every
    /// banana points the same way, and badly again when rotation is uniform
    /// over the full circle.
    #[serde(default)]
    pub rotation_jitter_deg: f32,
    /// Inclusive multiplier range on each motif's natural size. Small amounts
    /// of variation do a disproportionate amount of work; 0.8 to 1.2 is the
    /// usual band.
    #[serde(default = "unit_range")]
    pub scale_range: (f32, f32),
    pub motif_mix: Vec<MotifWeight>,
    /// Keep motifs from intersecting each other.
    ///
    /// On by default. Poisson-disc sampling guarantees a minimum distance
    /// between motif *centres*, solved from the coverage target — which says
    /// nothing about their edges. Past a certain density that separation is
    /// smaller than the motifs themselves and they begin to intersect. That is
    /// occasionally wanted and usually a defect.
    ///
    /// With this on, the required separation between two motifs is the greater
    /// of the density's radius and the sum of their own radii, so coverage
    /// still controls spacing while it can and overlap becomes a floor once it
    /// cannot. A pattern asked for more coverage than can be reached without
    /// overlapping therefore lands short of its target, which the achieved
    /// figure reports rather than hiding.
    #[serde(default = "prevent_overlap_default")]
    pub prevent_overlap: bool,
    /// Build the tile as a mirror repeat rather than strewing across all of it.
    ///
    /// The sampler then works in half or a quarter of the tile and the rest is
    /// reflections of it, so the same density buys a half or a quarter as many
    /// *distinct* motifs. See [`Mirror`].
    #[serde(default)]
    pub mirror: Mirror,
    /// Share of motifs mounted on a mirror axis rather than kept clear of one.
    ///
    /// Zero — the default — is the plain mirror: every motif sits clear of the
    /// mirror lines, which is what stops it being cut and doubled, and which
    /// leaves a bare corridor along each line when motifs are large.
    ///
    /// Above zero, that share of candidates is offered a position *on* an
    /// axis, where the reflection meets the motif exactly and symmetrises it
    /// instead of colliding with it. Filling the corridors that way is the
    /// damask move. Realised share runs lower than asked: an axis position is
    /// more constrained than a free one, so more candidates are refused.
    ///
    /// Means nothing without a [`Mirror`], and costs no random draw when it is
    /// zero, so a document written before it existed samples identically.
    #[serde(default)]
    pub on_axis_share: f32,
    pub seed: u64,
}

fn prevent_overlap_default() -> bool {
    true
}

fn unit_range() -> (f32, f32) {
    (1.0, 1.0)
}

/// Distribute motifs across the tile so they read as hand-strewn.
///
/// Returns placements in generation order, which is stable for a given seed and
/// is the order the renderer composites in.
///
/// Empty when the parameters describe nothing to place — no motifs, no density,
/// or a degenerate lattice. Returning an empty scatter rather than an error is
/// deliberate: a density slider at zero is a legitimate thing for a designer to
/// do, not a fault.
#[must_use]
pub fn scatter(params: &ScatterParams, tile_size_mm: Vec2, lattice: &Lattice) -> Vec<Placement> {
    if params.motif_mix.is_empty()
        || params.density <= 0.0
        || tile_size_mm.x <= 0.0
        || tile_size_mm.y <= 0.0
        || lattice.is_degenerate()
    {
        return Vec::new();
    }

    let radius = separation_radius(params, tile_size_mm);
    if radius <= 0.0 {
        return Vec::new();
    }

    // A mirror repeat is designed in part of the tile and reflected into the
    // rest, so that is the domain the sampler fills. Distances are still
    // measured across the whole tile, because a motif's reflections are as
    // real on the fabric as the motif is.
    let cell = params.mirror.cell_size_mm(tile_size_mm);
    let mut rng = ChaCha8Rng::seed_from_u64(params.seed);
    let designed = poisson_disc(params, radius, cell, lattice, &mut rng);
    reflect(&designed, tile_size_mm, params.mirror)
}

/// One accepted motif, with everything the separation test needs to know.
struct Sample {
    position_mm: Vec2,
    motif: usize,
    rotation_deg: f32,
    scale: f32,
    /// Radius of the circle containing this motif's ink at this scale.
    radius_mm: f32,
}

/// Bridson's dart-throwing, with every distance measured on the lattice torus.
///
/// Motif, rotation and scale are drawn for each *candidate* rather than
/// assigned afterwards, because with overlap prevention the distance a
/// candidate needs depends on which motif it turned out to be and how large it
/// is. Deciding that after the position is fixed would mean testing one motif's
/// spacing and then placing another.
///
/// Accepted samples bucketed in lattice coordinates, so that a candidate is
/// compared with its neighbours rather than with everything placed so far.
///
/// The reason this is not the usual spatial grid: that one indexes the tile
/// rectangle, and on an oblique lattice the rectangle is the wrong
/// neighbourhood — two motifs a whole tile apart may be adjacent in the
/// pattern. Indexing the *lattice* instead makes wrapping a modulo on the
/// bucket number and keeps the neighbourhood honest for any repeat.
///
/// It returns a superset and nothing more: every candidate it yields is still
/// put through the exact distance test. So the accept and reject decisions are
/// identical to comparing against every sample, the random draw is therefore
/// identical, and the placements are unchanged — which the golden image and
/// the determinism tests are what actually hold to account.
struct Neighbourhood {
    buckets: Vec<Vec<usize>>,
    across: usize,
    down: usize,
    u: Vec2,
    v: Vec2,
    det: f32,
    /// Largest radius in the index, so a query can be widened enough to catch
    /// a big motif whose own size, rather than the density, sets the spacing.
    max_radius: f32,
}

impl Neighbourhood {
    /// Bucket the lattice cell finely enough that a query spans about one
    /// bucket each way.
    ///
    /// A displacement of `d` in millimetres moves the first lattice
    /// coordinate by at most `d * |v| / |det|` and the second by at most
    /// `d * |u| / |det|` — Cauchy-Schwarz on the rows of the inverse. Sizing
    /// the buckets by that keeps the scanned neighbourhood small whatever the
    /// shear.
    fn new(lattice: &Lattice, cell_size_mm: Vec2, radius: f32) -> Self {
        /// Per axis. The total is bounded separately, by how many samples are
        /// actually going to arrive: a fine grid in a sparse cell spends more
        /// on allocating empty buckets than the scan it saves, and a coarse
        /// one in a dense cell puts the quadratic back inside each bucket.
        /// Both were measured — a fixed cap of four thousand buckets made a
        /// forty-eight-thousand-motif repeat six times slower.
        const MOST: usize = 512;

        // The packing relation the density solver already uses, read the
        // other way round: how many samples a cell of this size holds at this
        // separation.
        let expected = (PACKING_EFFICIENCY * 1.154_700_5 * cell_size_mm.x * cell_size_mm.y
            / (radius * radius).max(f32::MIN_POSITIVE))
        .clamp(64.0, 65536.0) as usize;

        let det = lattice.u.x * lattice.v.y - lattice.u.y * lattice.v.x;
        let span = |edge: Vec2| {
            let reach = radius * edge.length() / det.abs().max(f32::MIN_POSITIVE);
            if reach <= 0.0 || !reach.is_finite() {
                1
            } else {
                ((1.0 / reach).floor() as usize).clamp(1, MOST)
            }
        };
        let (mut across, mut down) = (span(lattice.v), span(lattice.u));
        while across * down > expected && (across > 1 || down > 1) {
            if across >= down {
                across = (across / 2).max(1);
            } else {
                down = (down / 2).max(1);
            }
        }

        Self {
            buckets: vec![Vec::new(); across * down],
            across,
            down,
            u: lattice.u,
            v: lattice.v,
            det,
            max_radius: 0.0,
        }
    }

    /// Where a point sits in the lattice, as fractions of `u` and `v`.
    fn coordinates(&self, point: Vec2) -> (f32, f32) {
        if self.det.abs() <= f32::EPSILON {
            return (0.0, 0.0);
        }
        (
            (point.x * self.v.y - point.y * self.v.x) / self.det,
            (self.u.x * point.y - self.u.y * point.x) / self.det,
        )
    }

    fn bucket(&self, alpha: f32, beta: f32) -> usize {
        let wrap = |value: f32, count: usize| {
            let n = count as f32;
            let i = (value.rem_euclid(1.0) * n).floor();
            (i as usize).min(count - 1)
        };
        wrap(beta, self.down) * self.across + wrap(alpha, self.across)
    }

    fn insert(&mut self, index: usize, point: Vec2, radius: f32) {
        let (alpha, beta) = self.coordinates(point);
        let at = self.bucket(alpha, beta);
        self.buckets[at].push(index);
        self.max_radius = self.max_radius.max(radius);
    }

    /// Every sample that could lie within `reach` millimetres of `point`.
    fn near(&self, point: Vec2, reach: f32, out: &mut Vec<usize>) {
        out.clear();
        let scale = self.det.abs().max(f32::MIN_POSITIVE);
        let steps = |edge: Vec2, count: usize| -> usize {
            let cells = reach * edge.length() / scale * count as f32;
            // One bucket of slack, because a point sits somewhere inside its
            // own bucket rather than at the corner of it.
            ((cells.ceil() as usize) + 1).min(count / 2 + 1)
        };
        let (da, db) = (steps(self.v, self.across), steps(self.u, self.down));

        let (alpha, beta) = self.coordinates(point);
        let home = |value: f32, count: usize| {
            ((value.rem_euclid(1.0) * count as f32).floor() as usize).min(count - 1) as isize
        };
        let (ha, hb) = (home(alpha, self.across), home(beta, self.down));

        for j in -(db as isize)..=(db as isize) {
            let row = (hb + j).rem_euclid(self.down as isize) as usize;
            for i in -(da as isize)..=(da as isize) {
                let col = (ha + i).rem_euclid(self.across as isize) as usize;
                out.extend_from_slice(&self.buckets[row * self.across + col]);
            }
        }
        // A small index can name the same bucket twice through the wrap.
        out.sort_unstable();
        out.dedup();
    }
}

/// Each candidate is checked against its neighbours, found through a
/// [`Neighbourhood`], rather than against every sample accepted so far.
///
/// It was brute-force until a 1.2 m repeat of small motifs made it a hang.
/// Quadratic is fine at the tens to a few hundred motifs a textile repeat
/// usually holds, and is not fine one order of magnitude up: seventeen
/// thousand placements took thirty-eight seconds to sample.
///
/// The reason it stayed brute-force as long as it did was a real objection,
/// not neglect — the spatial grid the algorithm is usually written with
/// indexes the tile rectangle, and on an oblique lattice the rectangle is the
/// wrong neighbourhood. [`Neighbourhood`] indexes the lattice instead, which
/// is what that objection was asking for.
fn poisson_disc(
    params: &ScatterParams,
    radius: f32,
    cell_size_mm: Vec2,
    lattice: &Lattice,
    rng: &mut ChaCha8Rng,
) -> Vec<Placement> {
    let radius_sq = radius * radius;
    let mirror = params.mirror;

    // Offer a candidate a place on a mirror axis, or leave it where it fell.
    //
    // Consumes a draw only when there is a share to honour, so every document
    // that predates this — and every mirror asking for none — walks the same
    // random sequence it always did.
    let mount = |rng: &mut ChaCha8Rng, position: Vec2| -> Vec2 {
        if mirror == Mirror::None || params.on_axis_share <= 0.0 {
            return position;
        }
        if unit(rng) < params.on_axis_share {
            snap_to_axis(position, cell_size_mm, mirror)
        } else {
            position
        }
    };

    let draw = |rng: &mut ChaCha8Rng, position_mm: Vec2| -> Sample {
        let motif = choose_motif_index(&params.motif_mix, rng);
        let rotation_deg = symmetric_value(rng, params.rotation_jitter_deg);
        let scale = between_values(rng, params.scale_range.0, params.scale_range.1);
        let entry = &params.motif_mix[motif];
        let half_diagonal = entry.natural_size_mm.length() / 2.0;
        Sample {
            position_mm,
            motif,
            rotation_deg,
            scale,
            radius_mm: half_diagonal * entry.radius_fraction.clamp(0.05, 1.0) * scale,
        }
    };

    // The seed sample has nothing to be tested against — except, under a
    // mirror, itself. Dropped astride an axis it collides with its own
    // reflection, and because it is the one sample that skips the acceptance
    // test the collision survives into the tile. Without a mirror
    // `self_image_distance_sq` is infinite, the first attempt always takes,
    // and the random draw is the same sequence it always was.
    let mut first = None;
    for _ in 0..CANDIDATES_PER_SAMPLE {
        let start = Vec2::new(unit(rng) * cell_size_mm.x, unit(rng) * cell_size_mm.y);
        let start = mount(rng, start);
        let candidate = draw(rng, start);
        let touching = 2.0 * candidate.radius_mm;
        let required_sq = if params.prevent_overlap {
            radius_sq.max(touching * touching)
        } else {
            radius_sq
        };
        if self_image_distance_sq(candidate.position_mm, lattice, mirror) >= required_sq {
            first = Some(candidate);
            break;
        }
    }
    // Nowhere in the cell is far enough from its own mirror image, which means
    // the motif is more than half the cell across. A mirror repeat cannot hold
    // it without doubling it, and saying so beats printing the double.
    let Some(first) = first else {
        return Vec::new();
    };
    let mut index = Neighbourhood::new(lattice, cell_size_mm, radius);
    index.insert(0, first.position_mm, first.radius_mm);
    let mut nearby: Vec<usize> = Vec::new();
    let mut found: Vec<usize> = Vec::new();
    let mut samples = vec![first];
    let mut active = vec![0usize];

    while !active.is_empty() {
        let slot = bounded(rng, active.len() as u32) as usize;
        let origin = samples[active[slot]].position_mm;
        let mut accepted = false;

        for _ in 0..CANDIDATES_PER_SAMPLE {
            let position =
                fold_into_cell(origin + annulus_offset(rng, radius), cell_size_mm, mirror);
            let position = mount(rng, position);
            let candidate = draw(rng, position);

            let required = |other: f32| {
                if params.prevent_overlap {
                    let touching = candidate.radius_mm + other;
                    radius_sq.max(touching * touching)
                } else {
                    radius_sq
                }
            };

            // A motif sitting on a mirror axis collides with its own
            // reflection, and no pairwise test sees that: the copy does not
            // exist yet when the candidate is judged.
            let clear_of_itself = self_image_distance_sq(candidate.position_mm, lattice, mirror)
                >= required(candidate.radius_mm);

            // Wide enough to catch the largest motif placed so far, since a
            // big one's own size rather than the density can set the spacing.
            let reach = radius.max(candidate.radius_mm + index.max_radius);

            // Looked up at every image of the candidate, not only at the
            // candidate. Under a mirror a sample is a threat when *its*
            // reflection lands nearby, which is the same thing as the sample
            // lying near a reflection of the candidate — and an index that
            // only knew about the candidate would silently stop finding them.
            nearby.clear();
            for &(flip_x, flip_y) in mirror.quadrants() {
                let image = Vec2::new(
                    if flip_x {
                        -candidate.position_mm.x
                    } else {
                        candidate.position_mm.x
                    },
                    if flip_y {
                        -candidate.position_mm.y
                    } else {
                        candidate.position_mm.y
                    },
                );
                index.near(image, reach, &mut found);
                nearby.extend_from_slice(&found);
            }
            nearby.sort_unstable();
            nearby.dedup();

            let fits = clear_of_itself
                && nearby.iter().all(|i| {
                    let s = &samples[*i];
                    mirror_distance_sq(candidate.position_mm, s.position_mm, lattice, mirror)
                        >= required(s.radius_mm)
                });

            if fits {
                index.insert(samples.len(), candidate.position_mm, candidate.radius_mm);
                samples.push(candidate);
                active.push(samples.len() - 1);
                accepted = true;
                break;
            }
        }

        if !accepted {
            active.swap_remove(slot);
        }
    }

    samples
        .into_iter()
        .map(|s| Placement {
            motif_id: params.motif_mix[s.motif].motif_id.clone(),
            position_mm: s.position_mm,
            rotation_deg: s.rotation_deg,
            scale: s.scale,
            flip_x: false,
            flip_y: false,
        })
        .collect()
}

/// Turn a coverage target into the minimum separation that achieves it.
///
/// Coverage is `count * mean motif area / tile area`, so the count wanted
/// follows from the target, and the radius follows from the count through the
/// packing relation. Mean motif area is weighted by the mix, since a scatter
/// that is mostly small motifs needs them closer together to cover the same
/// fabric.
fn separation_radius(params: &ScatterParams, tile_size_mm: Vec2) -> f32 {
    let total_weight: f32 = params.motif_mix.iter().map(|m| m.weight.max(0.0)).sum();
    if total_weight <= 0.0 {
        return 0.0;
    }
    let mean_area: f32 = params
        .motif_mix
        .iter()
        .map(|m| {
            let area = m.natural_size_mm.x.max(0.0) * m.natural_size_mm.y.max(0.0);
            area * m.weight.max(0.0) / total_weight
        })
        .sum();
    if mean_area <= 0.0 {
        return 0.0;
    }

    let tile_area = tile_size_mm.x * tile_size_mm.y;
    let wanted = (params.density * tile_area / mean_area).max(1.0);

    // count = efficiency * 2 / (sqrt(3) * r^2) * area, solved for r.
    const TWO_OVER_ROOT_THREE: f32 = 1.154_700_5;
    (PACKING_EFFICIENCY * TWO_OVER_ROOT_THREE * tile_area / wanted).sqrt()
}

/// A displacement uniformly distributed in the annulus between `r` and `2r`.
///
/// Found by rejection in the bounding square rather than from an angle and a
/// radius. The usual formulation needs a sine and a cosine, and `f32`
/// transcendentals carry no bit-exactness guarantee across platforms or libm
/// versions. Rejection uses only multiply, subtract and compare, all of which
/// IEEE-754 pins down exactly, so a seed reproduces the same placements
/// anywhere. Roughly 41% of darts are rejected, which costs nothing here.
fn annulus_offset(rng: &mut ChaCha8Rng, radius: f32) -> Vec2 {
    let inner_sq = radius * radius;
    let outer_sq = 4.0 * inner_sq;
    loop {
        let candidate = Vec2::new(
            symmetric_value(rng, 2.0 * radius),
            symmetric_value(rng, 2.0 * radius),
        );
        let d = candidate.length_squared();
        if d >= inner_sq && d <= outer_sq {
            return candidate;
        }
    }
}

pub(crate) fn choose_motif_index(mix: &[MotifWeight], rng: &mut ChaCha8Rng) -> usize {
    let total: f32 = mix.iter().map(|m| m.weight.max(0.0)).sum();
    if total <= 0.0 {
        return 0;
    }
    let mut target = unit(rng) * total;
    for (i, m) in mix.iter().enumerate() {
        target -= m.weight.max(0.0);
        if target <= 0.0 {
            return i;
        }
    }
    // Only reachable through floating-point accumulation at the very top of the
    // range; the last weighted motif is the right answer there.
    mix.iter().rposition(|m| m.weight > 0.0).unwrap_or(0)
}

/// A float in `[0, 1)`.
///
/// Derived here rather than taken from a distribution crate so the mapping from
/// generator bits to floats is fixed by this code and cannot change under a
/// dependency upgrade. Reproducing a print months later depends on it.
fn unit(rng: &mut ChaCha8Rng) -> f32 {
    // 24 bits is the full precision of an f32 mantissa; taking more would add
    // no values and would round.
    (rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32
}

/// A float in `[-bound, bound]`.
pub(crate) fn symmetric_value(rng: &mut ChaCha8Rng, bound: f32) -> f32 {
    (unit(rng) * 2.0 - 1.0) * bound
}

/// A float in `[low, high]`, tolerating the two being given the wrong way round.
pub(crate) fn between_values(rng: &mut ChaCha8Rng, low: f32, high: f32) -> f32 {
    let (low, high) = if low <= high {
        (low, high)
    } else {
        (high, low)
    };
    low + unit(rng) * (high - low)
}

/// An integer in `[0, bound)`, by multiply-shift rather than modulo.
fn bounded(rng: &mut ChaCha8Rng, bound: u32) -> u32 {
    if bound == 0 {
        return 0;
    }
    ((rng.next_u32() as u64 * bound as u64) >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{Color, Palette};
    use crate::pattern::Pattern;
    use crate::torus::lattice_distance_sq;

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    fn mix() -> Vec<MotifWeight> {
        vec![MotifWeight {
            motif_id: "cherry".into(),
            weight: 1.0,
            natural_size_mm: Vec2::new(22.0, 22.0),
            radius_fraction: 1.0,
        }]
    }

    fn params(density: f32, seed: u64) -> ScatterParams {
        ScatterParams {
            density,
            rotation_jitter_deg: 20.0,
            scale_range: (0.85, 1.15),
            motif_mix: mix(),
            // Off in most fixtures: these tests measure the sampler's own
            // separation guarantee, and overlap prevention adds a second,
            // larger floor that would mask it.
            prevent_overlap: false,
            mirror: Mirror::None,
            on_axis_share: 0.0,
            seed,
        }
    }

    fn mirrored(mirror: Mirror, density: f32, seed: u64, prevent_overlap: bool) -> ScatterParams {
        ScatterParams {
            prevent_overlap,
            mirror,
            ..params(density, seed)
        }
    }

    const MIRRORS: [Mirror; 3] = [Mirror::SideToSide, Mirror::TopToBottom, Mirror::FourWay];

    #[test]
    fn the_index_never_hides_a_neighbour() {
        // The whole correctness argument. The index only has to return a
        // *superset* — every candidate it yields is still put through the
        // exact distance test — so if it never omits anything within reach,
        // the accept and reject decisions are identical to comparing against
        // every sample, and so is the random draw, and so are the placements.
        //
        // Checked against the distance function rather than against a second
        // copy of the index, and over the oblique lattices as well as the
        // block one, since a sheared cell is the case a rectangular grid gets
        // wrong.
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        for lattice in [
            Lattice::block(TILE),
            Lattice::half_drop(TILE),
            Lattice::brick(TILE),
            Lattice::quarter_drop(TILE),
        ] {
            for radius in [6.0f32, 18.0, 45.0] {
                let mut index = Neighbourhood::new(&lattice, TILE, radius);
                let points: Vec<Vec2> = (0..250)
                    .map(|_| Vec2::new(unit(&mut rng) * TILE.x, unit(&mut rng) * TILE.y))
                    .collect();
                for (i, p) in points.iter().enumerate() {
                    index.insert(i, *p, 0.0);
                }

                let mut found = Vec::new();
                for _ in 0..60 {
                    let query = Vec2::new(unit(&mut rng) * TILE.x, unit(&mut rng) * TILE.y);
                    // Query wider than the index was sized for, which is what
                    // a large motif does in anger.
                    let reach = radius * 1.7;
                    index.near(query, reach, &mut found);

                    for (i, p) in points.iter().enumerate() {
                        let d = lattice_distance_sq(query, *p, &lattice).sqrt();
                        if d <= reach {
                            assert!(
                                found.contains(&i),
                                "{lattice:?} r={radius}: point {i} is {d:.2} mm away, \
                                 inside a {reach:.2} mm reach, and the index did not offer it"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn separation_still_holds_at_tens_of_thousands() {
        // The index earns its place at counts the brute-force scan could not
        // reach: a 1.2 m repeat of small motifs is seventeen thousand of them,
        // which took thirty-eight seconds to sample and now takes half of one.
        // If the index hid a neighbour anywhere in that, this is where it
        // shows.
        let tile = Vec2::new(1200.0, 900.0);
        let mut p = params(0.45, 3);
        p.motif_mix[0].natural_size_mm = Vec2::new(8.0, 8.0);
        let lattice = Lattice::half_drop(tile);

        let out = scatter(&p, tile, &lattice);
        assert!(
            out.len() > 4000,
            "expected a dense field, got {} placements",
            out.len()
        );

        // Every pair would be a hundred million comparisons in an unoptimised
        // test build. Two hundred motifs against all of the others is three
        // million, and a missed neighbour is a property of the index rather
        // than of one unlucky pair, so a sample finds it.
        let radius = separation_radius(&p, tile);
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        for _ in 0..200 {
            let i = bounded(&mut rng, out.len() as u32) as usize;
            for (j, b) in out.iter().enumerate() {
                if i == j {
                    continue;
                }
                let d = lattice_distance_sq(out[i].position_mm, b.position_mm, &lattice).sqrt();
                assert!(
                    d >= radius - 1e-2,
                    "two of {} motifs are {d:.3} mm apart, under the {radius:.3} mm separation",
                    out.len()
                );
            }
        }
    }

    #[test]
    fn a_mirrored_scatter_reflects_every_motif_into_every_quadrant() {
        for mirror in MIRRORS {
            let p = mirrored(mirror, 0.35, 4, true);
            let out = scatter(&p, TILE, &Lattice::block(TILE));
            assert!(!out.is_empty(), "{mirror:?} placed nothing");
            assert_eq!(
                out.len() % mirror.copies(),
                0,
                "{mirror:?} gave {} placements, not a multiple of {}",
                out.len(),
                mirror.copies()
            );

            // Each group is one designed motif and its reflections: same
            // motif, same rotation, same size, positions mirrored.
            for group in out.chunks(mirror.copies()) {
                let first = &group[0];
                assert!(!first.flip_x && !first.flip_y);
                for copy in group {
                    assert_eq!(copy.motif_id, first.motif_id);
                    assert_eq!(copy.rotation_deg, first.rotation_deg);
                    assert_eq!(copy.scale, first.scale);
                    let want_x = if copy.flip_x {
                        TILE.x - first.position_mm.x
                    } else {
                        first.position_mm.x
                    };
                    let want_y = if copy.flip_y {
                        TILE.y - first.position_mm.y
                    } else {
                        first.position_mm.y
                    };
                    assert_eq!(copy.position_mm, Vec2::new(want_x, want_y));
                }
            }
        }
    }

    #[test]
    fn nothing_in_a_mirrored_scatter_intersects_its_own_reflection() {
        // The failure a mirror repeat is prone to and no ordinary separation
        // test can see: a motif on an axis is doubled into a butterfly by its
        // own reflection, and the sampler never compares a candidate with a
        // copy that does not exist yet.
        //
        // Measured here on the designed motifs, because a reflection of a
        // reflection is the original and would trivially pass.
        for mirror in MIRRORS {
            for seed in [1u64, 4, 99] {
                let p = mirrored(mirror, 0.35, seed, true);
                let lattice = Lattice::block(TILE);
                let out = scatter(&p, TILE, &lattice);
                let entry = &p.motif_mix[0];
                let half_diagonal = entry.natural_size_mm.length() / 2.0;

                for designed in out.chunks(mirror.copies()) {
                    let radius = half_diagonal * designed[0].scale;
                    // Computed a second way, not with the engine's own
                    // function: reflecting the point by hand and taking the
                    // plain distance. A mistake shared with the sampler would
                    // otherwise confirm itself.
                    let p0 = designed[0].position_mm;
                    let mut nearest = f32::INFINITY;
                    for (fx, fy) in [(true, false), (false, true), (true, true)] {
                        if (fx && !mirror.reflects_x()) || (fy && !mirror.reflects_y()) {
                            continue;
                        }
                        let image = Vec2::new(
                            if fx { TILE.x - p0.x } else { p0.x },
                            if fy { TILE.y - p0.y } else { p0.y },
                        );
                        nearest = nearest.min((image - p0).length());
                    }
                    assert!(
                        nearest >= 2.0 * radius - 1e-3,
                        "{mirror:?} seed {seed}: a motif of radius {radius:.2} mm sits \
                         {nearest:.2} mm from its own reflection"
                    );
                }
            }
        }
    }

    #[test]
    fn a_mirrored_scatter_separates_motifs_across_the_reflections_too() {
        for mirror in MIRRORS {
            let p = mirrored(mirror, 0.35, 4, false);
            let lattice = Lattice::block(TILE);
            let out = scatter(&p, TILE, &lattice);
            let radius = separation_radius(&p, TILE);

            // Every pair of placed motifs, reflections included, since every
            // one of them is printed.
            for (i, a) in out.iter().enumerate() {
                for b in out.iter().skip(i + 1) {
                    let d = lattice_distance_sq(a.position_mm, b.position_mm, &lattice).sqrt();
                    assert!(
                        d >= radius - 1e-2,
                        "{mirror:?}: two motifs {d:.2} mm apart, under the {radius:.2} mm \
                         separation the density asked for"
                    );
                }
            }
        }
    }

    #[test]
    fn every_mirrored_placement_lands_inside_the_tile() {
        for mirror in MIRRORS {
            for placement in scatter(
                &mirrored(mirror, 0.35, 4, true),
                TILE,
                &Lattice::block(TILE),
            ) {
                let p = placement.position_mm;
                assert!(
                    (0.0..=TILE.x).contains(&p.x) && (0.0..=TILE.y).contains(&p.y),
                    "{mirror:?}: {p:?} is outside the tile"
                );
            }
        }
    }

    #[test]
    fn a_motif_too_large_for_its_mirror_cell_places_nothing() {
        // Rather than placing one astride an axis and doubling it. A four-way
        // cell here is 120 x 90 mm and the motif is 200 mm across, so there is
        // nowhere in the cell a motif can sit without reflecting into itself.
        let mut p = mirrored(Mirror::FourWay, 0.35, 4, true);
        p.motif_mix[0].natural_size_mm = Vec2::new(200.0, 200.0);
        assert!(scatter(&p, TILE, &Lattice::block(TILE)).is_empty());

        // The same motif without a mirror is merely a sparse scatter, not an
        // impossible one — so the emptiness above is the mirror's doing.
        p.mirror = Mirror::None;
        assert!(!scatter(&p, TILE, &Lattice::block(TILE)).is_empty());
    }

    #[test]
    fn asking_for_motifs_on_the_mirror_lines_puts_them_there() {
        for mirror in MIRRORS {
            let mut p = mirrored(mirror, 0.35, 4, true);
            p.on_axis_share = 0.5;
            let out = scatter(&p, TILE, &Lattice::block(TILE));

            let on_axis = out
                .iter()
                .filter(|q| {
                    let axes_x = [0.0, TILE.x / 2.0, TILE.x];
                    let axes_y = [0.0, TILE.y / 2.0, TILE.y];
                    (mirror.reflects_x()
                        && axes_x.iter().any(|a| (q.position_mm.x - a).abs() < 1e-3))
                        || (mirror.reflects_y()
                            && axes_y.iter().any(|a| (q.position_mm.y - a).abs() < 1e-3))
                })
                .count();
            assert!(
                on_axis >= 2,
                "{mirror:?}: only {on_axis} of {} placements sit on a line",
                out.len()
            );
        }
    }

    #[test]
    fn mounting_motifs_on_the_lines_fits_more_of_them_in() {
        // The point of the feature: the band either side of each axis is
        // unusable to a free motif, and mounting reclaims it.
        for mirror in MIRRORS {
            let plain = scatter(
                &mirrored(mirror, 0.5, 11, true),
                TILE,
                &Lattice::block(TILE),
            );
            let mut params = mirrored(mirror, 0.5, 11, true);
            params.on_axis_share = 0.5;
            let damask = scatter(&params, TILE, &Lattice::block(TILE));
            assert!(
                damask.len() > plain.len(),
                "{mirror:?}: mounting gave {} placements against the plain mirror's {}",
                damask.len(),
                plain.len()
            );
        }
    }

    #[test]
    fn a_mounted_motif_still_clears_every_other_motif() {
        // Being allowed to meet its own reflection is not a licence to meet
        // anything else.
        let mut p = mirrored(Mirror::SideToSide, 0.5, 4, true);
        p.on_axis_share = 0.6;
        let lattice = Lattice::block(TILE);
        let out = scatter(&p, TILE, &lattice);
        let radius = separation_radius(&p, TILE);

        for (i, a) in out.iter().enumerate() {
            for b in out.iter().skip(i + 1) {
                // Two copies of one mounted motif coincide by design — that is
                // what mounting means — so only distinct positions are judged.
                let d = lattice_distance_sq(a.position_mm, b.position_mm, &lattice).sqrt();
                if d < 1e-3 {
                    continue;
                }
                assert!(
                    d >= radius - 1e-2,
                    "two motifs {d:.2} mm apart, under the {radius:.2} mm separation asked for"
                );
            }
        }
    }

    #[test]
    fn a_share_means_nothing_without_a_mirror_and_something_with_one() {
        // Two claims in one place because they are the same claim: the share
        // is consulted exactly when there are axes to mount motifs on.
        //
        // The compatibility half — that a share of zero walks the same random
        // sequence as a build that never heard of the option — is pinned by
        // the golden image in `pattern-render`, which renders a scatter and so
        // would change the moment an unconditional draw were added here.
        let plain = mirrored(Mirror::None, 0.35, 12, true);
        let mut plain_with_share = plain.clone();
        plain_with_share.on_axis_share = 0.4;
        assert_eq!(
            scatter(&plain, TILE, &Lattice::block(TILE)),
            scatter(&plain_with_share, TILE, &Lattice::block(TILE)),
            "with no mirror there are no axes, so a share must change nothing"
        );

        for mirror in MIRRORS {
            let params = mirrored(mirror, 0.35, 12, true);
            let mut with_a_share = params.clone();
            with_a_share.on_axis_share = 0.4;

            let a = scatter(&params, TILE, &Lattice::block(TILE));
            assert!(!a.is_empty());
            assert_ne!(
                a,
                scatter(&with_a_share, TILE, &Lattice::block(TILE)),
                "{mirror:?}: a share above zero should change the scatter"
            );
        }
    }

    #[test]
    fn a_mirrored_scatter_is_reproducible_from_its_seed() {
        for mirror in MIRRORS {
            let p = mirrored(mirror, 0.35, 77, true);
            let a = scatter(&p, TILE, &Lattice::block(TILE));
            let b = scatter(&p, TILE, &Lattice::block(TILE));
            assert_eq!(a, b, "{mirror:?} is not reproducible");
            assert!(!a.is_empty());
        }
    }

    #[test]
    fn asking_for_no_mirror_leaves_the_scatter_exactly_as_it_was() {
        // The compatibility claim, asserted rather than assumed: every
        // document written before mirrors existed deserialises to
        // `Mirror::None`, and must produce the placements it always did.
        for seed in [1u64, 4, 99] {
            let plain = params(0.35, seed);
            let with_field = ScatterParams {
                mirror: Mirror::None,
                ..plain.clone()
            };
            assert_eq!(
                scatter(&plain, TILE, &Lattice::half_drop(TILE)),
                scatter(&with_field, TILE, &Lattice::half_drop(TILE))
            );
        }
    }

    /// Closest pair among all lattice copies, which is the separation that
    /// actually reaches fabric.
    /// Parameters for measuring the sampler itself: one motif, no overlap
    /// floor, so what is measured is the separation guarantee and nothing else.
    fn probe_params() -> ScatterParams {
        let mut p = params(0.45, 0);
        p.prevent_overlap = false;
        p
    }

    fn min_separation(placements: &[Placement], lattice: &Lattice) -> f32 {
        let mut best = f32::MAX;
        for (i, a) in placements.iter().enumerate() {
            for b in &placements[i + 1..] {
                best = best.min(lattice_distance_sq(a.position_mm, b.position_mm, lattice));
            }
        }
        best.sqrt()
    }

    /// The closest any two motifs' ink comes, in millimetres. Negative means
    /// they intersect.
    fn closest_approach(placements: &[Placement], lattice: &Lattice, mix: &[MotifWeight]) -> f32 {
        let radius_of = |p: &Placement| {
            let entry = mix
                .iter()
                .find(|m| m.motif_id == p.motif_id)
                .expect("placement names a motif in the mix");
            entry.natural_size_mm.length() / 2.0 * entry.radius_fraction * p.scale
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

    #[test]
    fn motifs_can_be_kept_from_intersecting() {
        // Reported from the app: at a plausible coverage the motifs visibly
        // overlapped. The sampler was doing what it promised — a minimum
        // distance between *centres*, solved from the coverage target — which
        // says nothing about where their edges are.
        let lattice = Lattice::half_drop(TILE);
        let mut p = params(0.6, 4);
        p.motif_mix[0].natural_size_mm = Vec2::new(40.0, 40.0);
        p.motif_mix[0].radius_fraction = 0.72;

        p.prevent_overlap = false;
        let loose = scatter(&p, TILE, &lattice);
        assert!(
            closest_approach(&loose, &lattice, &p.motif_mix) < 0.0,
            "this fixture is meant to overlap without the guard"
        );

        p.prevent_overlap = true;
        let guarded = scatter(&p, TILE, &lattice);
        assert!(
            closest_approach(&guarded, &lattice, &p.motif_mix) >= -1e-3,
            "motifs still intersect by {:.2} mm",
            closest_approach(&guarded, &lattice, &p.motif_mix)
        );
        assert!(guarded.len() > 3, "the guard emptied the pattern");
    }

    #[test]
    fn preventing_overlap_holds_across_the_tile_boundary() {
        // Overlap at a seam is the expensive kind: invisible in one tile and
        // obvious on fabric, which is the same trap the toroidal distance
        // exists for.
        let lattice = Lattice::half_drop(TILE);
        let mut p = params(0.6, 11);
        p.motif_mix[0].natural_size_mm = Vec2::new(38.0, 38.0);
        p.motif_mix[0].radius_fraction = 0.72;
        p.prevent_overlap = true;

        let placements = scatter(&p, TILE, &lattice);
        let mut checked = 0;
        for (i, a) in placements.iter().enumerate() {
            for b in &placements[i + 1..] {
                let wrapped = lattice_distance_sq(a.position_mm, b.position_mm, &lattice).sqrt();
                let interior = (a.position_mm - b.position_mm).length();
                if wrapped < interior - 1e-3 {
                    checked += 1;
                    let radius = 38.0 * std::f32::consts::SQRT_2 / 2.0 * 0.72;
                    assert!(
                        wrapped >= radius * (a.scale + b.scale) - 1e-2,
                        "a pair overlapping across the seam at {wrapped:.2} mm"
                    );
                }
            }
        }
        assert!(
            checked > 0,
            "no pair was closest via a neighbour, so this proved nothing"
        );
    }

    #[test]
    fn preventing_overlap_costs_coverage_rather_than_failing() {
        // A pattern asked for more coverage than fits without overlapping
        // lands short. That is the honest outcome, and the achieved figure is
        // what tells a designer so.
        let lattice = Lattice::half_drop(TILE);
        let mut p = params(0.85, 6);
        p.motif_mix[0].natural_size_mm = Vec2::new(40.0, 40.0);
        p.motif_mix[0].radius_fraction = 0.72;

        p.prevent_overlap = false;
        let loose = scatter(&p, TILE, &lattice).len();
        p.prevent_overlap = true;
        let guarded = scatter(&p, TILE, &lattice).len();

        assert!(
            guarded < loose,
            "the guard changed nothing: {loose} then {guarded}"
        );
        assert!(guarded > 0, "the guard emptied the pattern");
    }

    #[test]
    fn a_looser_shape_packs_tighter_than_its_bounding_box() {
        // Why the radius is measured from the artwork rather than the box: a
        // round motif reaches about 0.71 of its box diagonal, and spacing by
        // the box would hold every pattern 40% further apart than it needs.
        let lattice = Lattice::block(TILE);
        let mut p = params(0.6, 3);
        p.prevent_overlap = true;
        p.motif_mix[0].natural_size_mm = Vec2::new(36.0, 36.0);

        p.motif_mix[0].radius_fraction = 1.0;
        let boxed = scatter(&p, TILE, &lattice).len();
        p.motif_mix[0].radius_fraction = 0.71;
        let shaped = scatter(&p, TILE, &lattice).len();

        assert!(
            shaped > boxed,
            "shape-aware spacing gave {boxed} then {shaped}"
        );
    }

    #[test]
    fn no_two_motifs_land_closer_than_the_separation_radius() {
        // "No clumps", stated as the property rather than as an impression.
        for lattice in [Lattice::block(TILE), Lattice::half_drop(TILE)] {
            let p = params(0.45, 20260920);
            let placements = scatter(&p, TILE, &lattice);
            assert!(
                placements.len() >= 35,
                "expected roughly the spec's 40-motif scatter, got {}",
                placements.len()
            );

            let radius = separation_radius(&p, TILE);
            let closest = min_separation(&placements, &lattice);
            assert!(
                closest >= radius - 1e-2,
                "closest pair {closest:.2} mm is inside the {radius:.2} mm radius ({lattice:?})"
            );
        }
    }

    /// Distance over explicit lattice copies, written out here rather than
    /// calling the engine's own `lattice_distance_sq`.
    ///
    /// The independence is the point. An earlier version of this test measured
    /// with the production function and so only ever proved the scatter was
    /// consistent with whatever metric that function implemented — substituting
    /// a tile-rectangle distance left this test green. Computing it a second
    /// way, naively and on purpose, is what makes it evidence.
    fn brute_force_distance(a: Vec2, b: Vec2, lattice: &Lattice) -> f32 {
        let mut best = f32::MAX;
        for i in -2..=2 {
            for j in -2..=2 {
                let offset = lattice.u * i as f32 + lattice.v * j as f32;
                best = best.min((a - b + offset).length_squared());
            }
        }
        best.sqrt()
    }

    #[test]
    fn separation_holds_across_the_tile_boundary_on_a_half_drop() {
        // The pair a rectangle-wrapped sampler would have let through: two
        // motifs comfortably apart inside the tile whose nearest copies in the
        // printed pattern are almost touching.
        let lattice = Lattice::half_drop(TILE);
        let p = params(0.45, 7);
        let placements = scatter(&p, TILE, &lattice);
        let radius = separation_radius(&p, TILE);

        let mut checked = 0;
        for (i, a) in placements.iter().enumerate() {
            for b in &placements[i + 1..] {
                let wrapped = brute_force_distance(a.position_mm, b.position_mm, &lattice);
                let interior = (a.position_mm - b.position_mm).length();
                if wrapped < interior - 1e-3 {
                    checked += 1;
                    assert!(
                        wrapped >= radius - 1e-2,
                        "wrapped pair at {wrapped:.2} mm, inside the {radius:.2} mm radius"
                    );
                }
            }
        }
        assert!(
            checked > 0,
            "no pair was closest via a lattice neighbour, so this proved nothing"
        );
    }

    #[test]
    fn the_engine_distance_agrees_with_a_naive_one() {
        // Pins `lattice_distance_sq` against an independent implementation over
        // the whole tile, so the reduction step cannot quietly go wrong.
        let lattice = Lattice::half_drop(TILE);
        for gx in 0..17 {
            for gy in 0..13 {
                let a = Vec2::new(gx as f32 * 15.0, gy as f32 * 14.0);
                let b = Vec2::new(203.0 - gy as f32 * 7.0, 31.0 + gx as f32 * 9.0);
                let engine = lattice_distance_sq(a, b, &lattice).sqrt();
                let naive = brute_force_distance(a, b, &lattice);
                assert!(
                    (engine - naive).abs() < 1e-2,
                    "{a:?} to {b:?}: engine {engine:.3}, naive {naive:.3}"
                );
            }
        }
    }

    #[test]
    fn nowhere_in_the_tile_is_far_from_a_motif() {
        // "No bald patches", as a measured property rather than an
        // impression.
        //
        // The bound is 1.25x the separation radius, not 1x, and the difference
        // is a correction. Dart-throwing with a finite number of candidate
        // attempts produces a set that is *near*-maximal, not maximal: a small
        // pocket can survive because no dart happened to land in it before its
        // neighbours were retired. Asserting 1x failed at 1.03x on the first
        // seed tried. Measured across twelve seeds, three densities and both
        // lattices on 2026-09-20, the worst gap was 1.13x, so 1.25x holds with
        // margin while still excluding anything approaching the 2x that would
        // read as a real bald patch.
        let lattice = Lattice::half_drop(TILE);
        let p = params(0.45, 20260920);
        let placements = scatter(&p, TILE, &lattice);
        let radius = separation_radius(&p, TILE);

        let mut worst: f32 = 0.0;
        for gx in 0..60 {
            for gy in 0..45 {
                let point = Vec2::new(
                    (gx as f32 + 0.5) * TILE.x / 60.0,
                    (gy as f32 + 0.5) * TILE.y / 45.0,
                );
                let nearest = placements
                    .iter()
                    .map(|pl| lattice_distance_sq(point, pl.position_mm, &lattice))
                    .fold(f32::MAX, f32::min)
                    .sqrt();
                worst = worst.max(nearest);
            }
        }
        assert!(
            worst <= radius * 1.25,
            "a gap {worst:.2} mm from the nearest motif, with a {radius:.2} mm radius"
        );
    }

    #[test]
    fn packing_efficiency_is_what_the_density_solver_assumes() {
        // The constant the radius solver is built on, measured rather than
        // asserted from the literature. If the sampler changes and this drifts,
        // the density control silently stops meaning what it says — so this
        // fails first, and names the figure to update.
        let lattice = Lattice::block(TILE);
        let mut ratios = Vec::new();
        for seed in 0..8u64 {
            let radius = 18.0;
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let points = poisson_disc(&probe_params(), radius, TILE, &lattice, &mut rng);

            let area = TILE.x * TILE.y;
            let hexagonal = 1.154_700_5 * area / (radius * radius);
            ratios.push(points.len() as f32 / hexagonal);
        }
        let mean = ratios.iter().sum::<f32>() / ratios.len() as f32;
        assert!(
            (mean - PACKING_EFFICIENCY).abs() < 0.03,
            "measured packing efficiency {mean:.3}, solver assumes {PACKING_EFFICIENCY}"
        );
    }

    #[test]
    fn achieved_coverage_lands_near_the_target() {
        // What the density control promises a designer, end to end.
        let lattice = Lattice::half_drop(TILE);
        for target in [0.25, 0.45, 0.65] {
            let p = params(target, 11);
            let placements = scatter(&p, TILE, &lattice);
            let motif_area = 22.0 * 22.0;
            let achieved = placements.len() as f32 * motif_area / (TILE.x * TILE.y);
            assert!(
                (achieved - target).abs() < 0.05,
                "asked {target}, got {achieved:.3}"
            );
        }
    }

    #[test]
    fn density_is_monotonic() {
        // A slider that does not move one way is worse than no slider.
        let lattice = Lattice::half_drop(TILE);
        let counts: Vec<usize> = [0.1, 0.2, 0.4, 0.6]
            .iter()
            .map(|d| scatter(&params(*d, 3), TILE, &lattice).len())
            .collect();
        assert!(
            counts.windows(2).all(|w| w[1] > w[0]),
            "counts not increasing with density: {counts:?}"
        );
    }

    #[test]
    fn the_same_seed_gives_identical_placements() {
        let lattice = Lattice::half_drop(TILE);
        let a = scatter(&params(0.35, 20260920), TILE, &lattice);
        let b = scatter(&params(0.35, 20260920), TILE, &lattice);
        assert_eq!(a, b);
    }

    #[test]
    fn a_different_seed_gives_a_different_scatter() {
        let lattice = Lattice::half_drop(TILE);
        let a = scatter(&params(0.35, 1), TILE, &lattice);
        let b = scatter(&params(0.35, 2), TILE, &lattice);
        assert_ne!(a, b, "reseeding must actually reroll");
    }

    #[test]
    fn placement_generation_uses_no_transcendental_arithmetic() {
        // Not a behavioural test but a guard on a property this module claims:
        // scatter stays bit-reproducible across platforms because it never
        // calls a function libm is free to round differently. If a sine appears
        // here later, this fails and the claim in the docs has to change with
        // it.
        let source = include_str!("scatter.rs");
        let body = source
            .split("#[cfg(test)]")
            .next()
            .expect("module has a non-test part");
        for banned in [
            ".sin(",
            ".cos(",
            ".sin_cos(",
            ".tan(",
            ".exp(",
            ".ln(",
            ".powf(",
        ] {
            assert!(
                !body.contains(banned),
                "{banned} in scatter would break cross-platform reproducibility"
            );
        }
    }

    #[test]
    fn rotation_and_scale_stay_inside_their_bounds() {
        let lattice = Lattice::half_drop(TILE);
        let placements = scatter(&params(0.35, 5), TILE, &lattice);
        assert!(!placements.is_empty());
        for p in &placements {
            assert!(p.rotation_deg.abs() <= 20.0 + 1e-3, "{}", p.rotation_deg);
            assert!((0.85..=1.15).contains(&p.scale), "{}", p.scale);
        }
    }

    #[test]
    fn every_placement_lands_inside_the_tile() {
        let lattice = Lattice::half_drop(TILE);
        for p in scatter(&params(0.4, 9), TILE, &lattice) {
            assert!(
                (0.0..TILE.x).contains(&p.position_mm.x),
                "{:?}",
                p.position_mm
            );
            assert!(
                (0.0..TILE.y).contains(&p.position_mm.y),
                "{:?}",
                p.position_mm
            );
        }
    }

    #[test]
    fn motif_mix_weights_are_respected() {
        let lattice = Lattice::block(TILE);
        let p = ScatterParams {
            density: 0.5,
            rotation_jitter_deg: 0.0,
            scale_range: (1.0, 1.0),
            prevent_overlap: false,
            mirror: Mirror::None,
            on_axis_share: 0.0,
            motif_mix: vec![
                MotifWeight {
                    motif_id: "cherry".into(),
                    weight: 6.0,
                    natural_size_mm: Vec2::new(30.0, 30.0),
                    radius_fraction: 1.0,
                },
                MotifWeight {
                    motif_id: "pineapple".into(),
                    weight: 1.0,
                    natural_size_mm: Vec2::new(30.0, 30.0),
                    radius_fraction: 1.0,
                },
            ],
            seed: 42,
        };
        let placements = scatter(&p, TILE, &lattice);
        let cherries = placements
            .iter()
            .filter(|pl| pl.motif_id == "cherry")
            .count() as f32;
        let share = cherries / placements.len() as f32;
        assert!(
            (share - 6.0 / 7.0).abs() < 0.15,
            "cherries were {share:.2} of the scatter, expected about 0.86"
        );
    }

    #[test]
    fn degenerate_requests_return_nothing_rather_than_failing() {
        // A density slider at zero is a legitimate thing for a designer to do.
        let lattice = Lattice::half_drop(TILE);
        assert!(scatter(&params(0.0, 1), TILE, &lattice).is_empty());

        let mut no_motifs = params(0.35, 1);
        no_motifs.motif_mix.clear();
        assert!(scatter(&no_motifs, TILE, &lattice).is_empty());

        let degenerate = Lattice {
            u: Vec2::new(10.0, 5.0),
            v: Vec2::new(20.0, 10.0),
        };
        assert!(scatter(&params(0.35, 1), TILE, &degenerate).is_empty());
    }

    #[test]
    fn a_scattered_pattern_validates() {
        // The output has to be something the renderer will accept, which is
        // easy to break by assigning a slot the palette does not have.
        let lattice = Lattice::half_drop(TILE);
        let pattern = Pattern {
            tile_size_mm: TILE,
            lattice,
            placements: scatter(&params(0.35, 6), TILE, &lattice),
            palette: Palette::new(vec![Color::new(250, 246, 238), Color::new(200, 30, 45)]),
            seed: 6,
        };
        assert_eq!(pattern.validate(), Ok(()));
    }
}
