//! What the calibration corpus actually looks like.
//!
//! A statistic that separates two populations is worth nothing until somebody
//! has checked that the populations are what they claim to be. Both previous
//! attempts at tracking detection passed every test written against synthetic
//! masks; this writes the synthetic patterns out as prints so the strengths
//! can be judged by eye against the numbers.
//!
//! `cargo test -p pattern-render --release --test tracking_look -- --ignored`

mod support;

use glam::Vec2;
use pattern_core::{scatter, Lattice, Mirror, MotifWeight, Pattern, Placement, ScatterParams};
use pattern_render::{render_field, MotifBank};
use std::path::PathBuf;
use support::{bank, palette};

const TILE: Vec2 = Vec2::new(240.0, 180.0);

fn clean(seed: u64) -> Vec<Placement> {
    scatter(
        &ScatterParams {
            density: 0.30,
            rotation_jitter_deg: 20.0,
            scale_range: (0.9, 1.1),
            motif_mix: vec![MotifWeight {
                motif_id: "cherry".into(),
                weight: 1.0,
                natural_size_mm: Vec2::splat(30.0),
                radius_fraction: 1.0,
            }],
            prevent_overlap: true,
            mirror: Mirror::None,
            on_axis_share: 0.0,
            seed,
        },
        TILE,
        &Lattice::half_drop(TILE),
    )
}

/// The same nudge toward parallel lines the calibration uses.
fn tracked(seed: u64, strength: f32, angle_deg: f32) -> Vec<Placement> {
    let mut placements = clean(seed);
    let n = placements.len();
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

#[test]
#[ignore = "writes pictures to judge the corpus by; run with --ignored"]
fn write_the_corpus() {
    let out = std::path::PathBuf::from("/tmp/claude-501/-Users-leon-Projects-pattern-engine/6413fcf8-733c-4904-9aa3-5f20aa7e6ec3/scratchpad");
    let bank: MotifBank = bank();
    for (name, placements) in [
        ("00-clean", clean(7)),
        ("15", tracked(7, 0.15, 35.0)),
        ("30", tracked(7, 0.30, 35.0)),
        ("50", tracked(7, 0.50, 35.0)),
        ("80", tracked(7, 0.80, 35.0)),
    ] {
        let pattern = Pattern {
            tile_size_mm: TILE,
            lattice: Lattice::half_drop(TILE),
            placements,
            ground: None,
            palette: palette(),
            seed: 7,
        };
        let field = render_field(&pattern, &bank, 40.0, 3, 3).expect("should render");
        let png = field.encode_png().expect("png");
        std::fs::write(out.join(format!("track-{name}.png")), png).expect("write");
    }
    eprintln!("wrote the corpus to {}", out.display());
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

/// The fourth harmonic of the near-neighbour direction distribution — the
/// candidate measure, copied from the calibration so the two agree.
fn fourth(placements: &[Placement], lattice: &Lattice) -> f32 {
    const REACH: f32 = 1.6;
    let n = placements.len();
    if n < 3 {
        return 0.0;
    }
    let cutoff = REACH * (TILE.x * TILE.y / n as f32).sqrt();
    let (mut c4, mut s4, mut pairs) = (0f64, 0f64, 0usize);
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
            let (cos2, sin2) = (x * x - y * y, 2.0 * x * y);
            c4 += cos2 * cos2 - sin2 * sin2;
            s4 += 2.0 * cos2 * sin2;
            pairs += 1;
        }
    }
    if pairs < 8 {
        return 0.0;
    }
    ((c4 * c4 + s4 * s4).sqrt() / (pairs as f64).sqrt()) as f32
}

#[test]
#[ignore = "the decisive check; run with --ignored --nocapture"]
fn render_the_clean_patterns_the_measure_ranks_highest_and_lowest() {
    // The experiment that matters, and the one neither previous attempt ran.
    //
    // Scoring a synthetic corridor proves only that the measure notices the
    // construction that made it. This asks the question the other way round:
    // over real scatters with nothing done to them, does the measure's opinion
    // agree with a person's? The highest scorers should be the ones that look
    // tracked, and the lowest should not.
    //
    // If they look the same, the measure is ranking noise and no threshold on
    // it means anything — which is exactly what would have saved the previous
    // two attempts a great deal of work.
    let out = PathBuf::from("/tmp/claude-501/-Users-leon-Projects-pattern-engine/6413fcf8-733c-4904-9aa3-5f20aa7e6ec3/scratchpad");
    let lattice = Lattice::half_drop(TILE);

    let mut scored: Vec<(f32, u64)> = (0..300u64)
        .map(|i| {
            let seed = 1 + i * 7919;
            (fourth(&clean(seed), &lattice), seed)
        })
        .collect();
    scored.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite"));

    let bank: MotifBank = bank();
    let write = |label: &str, (score, seed): (f32, u64)| {
        let pattern = Pattern {
            tile_size_mm: TILE,
            lattice,
            placements: clean(seed),
            ground: None,
            palette: palette(),
            seed,
        };
        let field = render_field(&pattern, &bank, 40.0, 3, 3).expect("should render");
        std::fs::write(
            out.join(format!("rank-{label}.png")),
            field.encode_png().expect("png"),
        )
        .expect("write");
        eprintln!("{label}: seed {seed} scored {score:.2}");
    };

    for (k, entry) in scored.iter().rev().take(3).enumerate() {
        write(&format!("high-{k}"), *entry);
    }
    for (k, entry) in scored.iter().take(3).enumerate() {
        write(&format!("low-{k}"), *entry);
    }
}
