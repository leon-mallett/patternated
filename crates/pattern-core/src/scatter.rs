use glam::Vec2;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::lattice::Lattice;
use crate::placement::{wrap_into_tile, Placement};
use crate::torus::lattice_distance_sq;

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
    pub seed: u64,
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

    let mut rng = ChaCha8Rng::seed_from_u64(params.seed);
    let points = poisson_disc(radius, tile_size_mm, lattice, &mut rng);

    points
        .into_iter()
        .map(|position_mm| {
            let motif = choose_motif(&params.motif_mix, &mut rng);
            Placement {
                motif_id: motif.motif_id.clone(),
                position_mm,
                rotation_deg: symmetric(&mut rng, params.rotation_jitter_deg),
                scale: between(&mut rng, params.scale_range.0, params.scale_range.1),
            }
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

/// Bridson's dart-throwing, with every distance measured on the lattice torus.
///
/// Deliberately brute-force: each candidate is checked against every accepted
/// sample. The spatial grid the algorithm is usually written with indexes the
/// tile rectangle, and on an oblique lattice the rectangle is the wrong
/// neighbourhood — a correct grid would have to be built in lattice
/// coordinates, where the metric is sheared and the cell bounds stop being
/// obvious. At the counts a textile repeat uses, tens to a few hundred motifs,
/// the scan costs milliseconds and cannot be wrong in that particular way.
fn poisson_disc(
    radius: f32,
    tile_size_mm: Vec2,
    lattice: &Lattice,
    rng: &mut ChaCha8Rng,
) -> Vec<Vec2> {
    let radius_sq = radius * radius;
    let first = Vec2::new(unit(rng) * tile_size_mm.x, unit(rng) * tile_size_mm.y);
    let mut samples = vec![first];
    let mut active = vec![0usize];

    while !active.is_empty() {
        let slot = bounded(rng, active.len() as u32) as usize;
        let origin = samples[active[slot]];
        let mut accepted = false;

        for _ in 0..CANDIDATES_PER_SAMPLE {
            let candidate = wrap_into_tile(origin + annulus_offset(rng, radius), tile_size_mm);
            if samples
                .iter()
                .all(|s| lattice_distance_sq(candidate, *s, lattice) >= radius_sq)
            {
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
        let candidate = Vec2::new(symmetric(rng, 2.0 * radius), symmetric(rng, 2.0 * radius));
        let d = candidate.length_squared();
        if d >= inner_sq && d <= outer_sq {
            return candidate;
        }
    }
}

fn choose_motif<'a>(mix: &'a [MotifWeight], rng: &mut ChaCha8Rng) -> &'a MotifWeight {
    let total: f32 = mix.iter().map(|m| m.weight.max(0.0)).sum();
    if total <= 0.0 {
        return &mix[0];
    }
    let mut target = unit(rng) * total;
    for m in mix {
        target -= m.weight.max(0.0);
        if target <= 0.0 {
            return m;
        }
    }
    // Only reachable through floating-point accumulation at the very top of the
    // range; the last weighted motif is the right answer there.
    mix.iter().rev().find(|m| m.weight > 0.0).unwrap_or(&mix[0])
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
fn symmetric(rng: &mut ChaCha8Rng, bound: f32) -> f32 {
    (unit(rng) * 2.0 - 1.0) * bound
}

/// A float in `[low, high]`, tolerating the two being given the wrong way round.
fn between(rng: &mut ChaCha8Rng, low: f32, high: f32) -> f32 {
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

    const TILE: Vec2 = Vec2::new(240.0, 180.0);

    fn mix() -> Vec<MotifWeight> {
        vec![MotifWeight {
            motif_id: "cherry".into(),
            weight: 1.0,
            natural_size_mm: Vec2::new(22.0, 22.0),
        }]
    }

    fn params(density: f32, seed: u64) -> ScatterParams {
        ScatterParams {
            density,
            rotation_jitter_deg: 20.0,
            scale_range: (0.85, 1.15),
            motif_mix: mix(),
            seed,
        }
    }

    /// Closest pair among all lattice copies, which is the separation that
    /// actually reaches fabric.
    fn min_separation(placements: &[Placement], lattice: &Lattice) -> f32 {
        let mut best = f32::MAX;
        for (i, a) in placements.iter().enumerate() {
            for b in &placements[i + 1..] {
                best = best.min(lattice_distance_sq(a.position_mm, b.position_mm, lattice));
            }
        }
        best.sqrt()
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
            let points = poisson_disc(radius, TILE, &lattice, &mut rng);

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
            motif_mix: vec![
                MotifWeight {
                    motif_id: "cherry".into(),
                    weight: 6.0,
                    natural_size_mm: Vec2::new(30.0, 30.0),
                },
                MotifWeight {
                    motif_id: "pineapple".into(),
                    weight: 1.0,
                    natural_size_mm: Vec2::new(30.0, 30.0),
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
