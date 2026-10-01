//! Calibrating a tracking measure against output this engine actually makes.
//!
//! Not a test of a detector — there is no detector. This is the step both
//! previous attempts left until last: build a corpus of real patterns, measure
//! a candidate statistic on the clean ones and the tracked ones, and look at
//! whether the two populations separate at all. Both earlier measures passed
//! every unit test written against hand-made masks and then scored good and
//! bad patterns in the same range.
//!
//! `cargo test -p pattern-core --release --test tracking_calibration --
//! --ignored --nocapture`

use glam::Vec2;
use pattern_core::{scatter, Lattice, Mirror, MotifWeight, Placement, ScatterParams};

const TILE: Vec2 = Vec2::new(240.0, 180.0);

fn mix(size_mm: f32) -> Vec<MotifWeight> {
    vec![MotifWeight {
        motif_id: "cherry".into(),
        weight: 1.0,
        required: false,
        natural_size_mm: Vec2::splat(size_mm),
        radius_fraction: 1.0,
    }]
}

fn clean(lattice: &Lattice, density: f32, seed: u64) -> Vec<Placement> {
    scatter(
        &ScatterParams {
            density,
            rotation_jitter_deg: 20.0,
            scale_range: (0.9, 1.1),
            motif_mix: mix(30.0),
            prevent_overlap: true,
            mirror: Mirror::None,
            on_axis_share: 0.0,
            seed,
        },
        TILE,
        lattice,
    )
}

/// A clean scatter pulled toward a set of parallel lines.
///
/// What tracking actually looks like: not a stripe of motifs but a scatter
/// whose positions drift onto rivers running through it. `strength` is how far
/// each motif moves toward its nearest line, from nothing to all the way, so
/// the corpus covers faint tracking as well as blatant.
fn tracked(
    lattice: &Lattice,
    density: f32,
    seed: u64,
    strength: f32,
    angle_deg: f32,
) -> Vec<Placement> {
    let mut placements = clean(lattice, density, seed);
    let n = placements.len();
    if n < 4 {
        return placements;
    }
    // Lines about one and a half spacings apart, which is roughly what reads
    // as a river rather than as a texture.
    let period = (TILE.x * TILE.y / n as f32).sqrt() * 1.5;
    let normal = Vec2::new(-angle_deg.to_radians().sin(), angle_deg.to_radians().cos());
    for p in placements.iter_mut() {
        let along = p.position_mm.dot(normal);
        let nearest = (along / period).round() * period;
        let at = p.position_mm + normal * ((nearest - along) * strength);
        p.position_mm = Vec2::new(at.x.rem_euclid(TILE.x), at.y.rem_euclid(TILE.y));
    }
    placements
}

/// The shortest displacement from `a` to `b` on the torus, as a vector.
fn shortest(a: Vec2, b: Vec2, lattice: &Lattice) -> Vec2 {
    let delta = b - a;
    let det = lattice.u.x * lattice.v.y - lattice.u.y * lattice.v.x;
    if det.abs() <= f32::EPSILON {
        return delta;
    }
    let s = (delta.x * lattice.v.y - delta.y * lattice.v.x) / det;
    let t = (lattice.u.x * delta.y - lattice.u.y * delta.x) / det;
    let base = delta - lattice.u * s.round() - lattice.v * t.round();
    let mut best = base;
    for i in -1..=1 {
        for j in -1..=1 {
            let candidate = base + lattice.u * i as f32 + lattice.v * j as f32;
            if candidate.length_squared() < best.length_squared() {
                best = candidate;
            }
        }
    }
    best
}

/// Directional structure in the near-neighbour displacements, as the strength
/// of the second and fourth harmonics of their angular distribution.
///
/// Two earlier statistics, and why each failed.
///
/// A histogram's tallest bin measured mostly noise: a few hundred pairs over
/// thirty-six bins is five to a bin, and Poisson scatter alone puts the peak
/// two or three times the mean — the whole range a faint corridor could have
/// moved it into.
///
/// The mean resultant of doubled angles fixed the noise and introduced a worse
/// problem. Doubling maps a direction and its reverse together, which is
/// right, but it also maps two *perpendicular* families onto opposite points,
/// where they cancel. A field of rivers has both along-river and across-river
/// neighbours, so the measure read it as having no preferred direction at all,
/// and scored a perfect square grid 1.4.
///
/// The fourth harmonic is the one that sees a grid or a field of rivers, where
/// the second sees a single preferred axis. Both are reported, each scaled by
/// the square root of the pair count, so that a dense pattern does not look
/// cleaner than a sparse one merely for having more pairs to average over.
fn harmonics(placements: &[Placement], lattice: &Lattice, reach: f32) -> (f32, f32) {
    let n = placements.len();
    if n < 3 {
        return (0.0, 0.0);
    }
    let spacing = (TILE.x * TILE.y / n as f32).sqrt();
    let cutoff = reach * spacing;

    let (mut c2, mut s2, mut c4, mut s4, mut pairs) = (0f64, 0f64, 0f64, 0f64, 0usize);
    for i in 0..n {
        for j in (i + 1)..n {
            let d = shortest(
                placements[i].position_mm,
                placements[j].position_mm,
                lattice,
            );
            let len = d.length();
            if len <= f32::EPSILON || len > cutoff {
                continue;
            }
            let (x, y) = (f64::from(d.x / len), f64::from(d.y / len));
            // Doubled and quadrupled angles from the components by the
            // double-angle identities, so there is no call to atan2 here.
            let (cos2, sin2) = (x * x - y * y, 2.0 * x * y);
            c2 += cos2;
            s2 += sin2;
            c4 += cos2 * cos2 - sin2 * sin2;
            s4 += 2.0 * cos2 * sin2;
            pairs += 1;
        }
    }
    if pairs < 8 {
        return (0.0, 0.0);
    }
    let scale = 1.0 / (pairs as f64).sqrt();
    (
        ((c2 * c2 + s2 * s2).sqrt() * scale) as f32,
        ((c4 * c4 + s4 * s4).sqrt() * scale) as f32,
    )
}

fn stats(values: &[f32]) -> (f32, f32, f32) {
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    (v[0], mean, v[v.len() - 1])
}

#[test]
#[ignore = "calibration, not a check; run with --ignored --nocapture"]
fn measure_the_two_populations() {
    // Reach 1.6 and the fourth harmonic, which the first sweep showed to be
    // the pair that carries the signal: nearest neighbours are where the eye
    // reads alignment, and the fourth harmonic is the one that sees a field of
    // rivers where the second cancels itself on them.
    const REACH: f32 = 1.6;

    let lattices = [
        Lattice::block(TILE),
        Lattice::half_drop(TILE),
        Lattice::brick(TILE),
    ];
    let densities = [0.15f32, 0.22, 0.30, 0.38, 0.45];
    // A long run of seeds, because what matters for a gate is the tail of the
    // clean population and a handful of samples says nothing about a tail.
    let seeds: Vec<u64> = (0..40).map(|i| 1 + i * 7919).collect();

    let mut clean_scores = Vec::new();
    for lattice in &lattices {
        for density in densities {
            for seed in &seeds {
                clean_scores.push(harmonics(&clean(lattice, density, *seed), lattice, REACH).1);
            }
        }
    }
    let (lo, mean, hi) = stats(&clean_scores);
    println!(
        "clean (n={}): {lo:.2} .. {hi:.2}, mean {mean:.2}",
        clean_scores.len()
    );

    let strengths = [0.15f32, 0.30, 0.50, 0.80];
    let mut tracked_scores: Vec<(f32, Vec<f32>)> = Vec::new();
    for strength in strengths {
        let mut scores = Vec::new();
        for lattice in &lattices {
            for density in densities {
                for seed in &seeds {
                    for angle in [0.0f32, 35.0, 62.0, 90.0] {
                        scores.push(
                            harmonics(
                                &tracked(lattice, density, *seed, strength, angle),
                                lattice,
                                REACH,
                            )
                            .1,
                        );
                    }
                }
            }
        }
        let (lo, mean, hi) = stats(&scores);
        println!(
            "tracked {:2.0}% (n={}): {lo:.2} .. {hi:.2}, mean {mean:.2}",
            strength * 100.0,
            scores.len()
        );
        tracked_scores.push((strength, scores));
    }

    let share_above =
        |v: &[f32], t: f32| v.iter().filter(|x| **x > t).count() as f32 / v.len() as f32 * 100.0;

    println!("\nthreshold   false alarms        caught at 15% / 30% / 50% / 80%");
    for t in [0.8f32, 1.0, 1.2, 1.5, 2.0, 2.5] {
        let caught: Vec<String> = tracked_scores
            .iter()
            .map(|(_, v)| format!("{:5.1}%", share_above(v, t)))
            .collect();
        println!(
            "  {t:4.1}      {:5.1}%          {}",
            share_above(&clean_scores, t),
            caught.join(" ")
        );
    }
}
