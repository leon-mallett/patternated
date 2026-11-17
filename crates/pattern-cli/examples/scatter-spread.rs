//! Is each motif as evenly spread as the whole scatter is?
//!
//!     cargo run -p pattern-cli --example scatter-spread -- examples/showcase/*.json
//!
//! Poisson-disc sampling makes the *set* blue noise. The motif is drawn
//! independently at each candidate, so without
//! [`ScatterParams::class_separation`](pattern_core::ScatterParams) the subset
//! wearing any one motif is close to Poisson, and Poisson clumps. Nobody looks
//! at the set; they look at the brass flowers. This prints the statistic that
//! tells the two apart, which is the spread of nearest-neighbour distances
//! rather than their mean: the mean only says how many there are.
//!
//! Not meaningful for a mirrored document. A motif mounted on a mirror axis
//! sits exactly on its own reflection, so the distance between them is zero by
//! design and the figure goes through the roof.
use std::env;
use std::path::Path;

use glam::Vec2;
use pattern_doc::Document;

/// Shortest distance between two points on the lattice torus.
fn toroidal(a: Vec2, b: Vec2, u: Vec2, v: Vec2) -> f32 {
    let mut best = f32::MAX;
    for i in -1..=1 {
        for j in -1..=1 {
            let o = Vec2::new(
                u.x * i as f32 + v.x * j as f32,
                u.y * i as f32 + v.y * j as f32,
            );
            let d = ((a.x - b.x - o.x).powi(2) + (a.y - b.y - o.y).powi(2)).sqrt();
            if d < best {
                best = d;
            }
        }
    }
    best
}

fn stats(d: &[f32]) -> (f32, f32) {
    let n = d.len() as f32;
    let mean = d.iter().sum::<f32>() / n;
    let var = d.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    (mean, var.sqrt() / mean)
}

fn main() -> anyhow::Result<()> {
    for arg in env::args().skip(1) {
        let (doc, _bank) = Document::load(Path::new(&arg))?;
        let p = &doc.pattern;
        let (u, v) = (p.lattice.u, p.lattice.v);
        let pl = &p.placements;
        println!("\n{}  ({} placements)", arg, pl.len());

        let nn = |subset: &[usize]| -> Vec<f32> {
            subset
                .iter()
                .map(|&i| {
                    subset
                        .iter()
                        .filter(|&&j| j != i)
                        .map(|&j| toroidal(pl[i].position_mm, pl[j].position_mm, u, v))
                        .fold(f32::MAX, f32::min)
                })
                .collect()
        };

        let all: Vec<usize> = (0..pl.len()).collect();
        let (m, cv) = stats(&nn(&all));
        println!(
            "  every motif        n={:<3} mean nn {:6.1} mm   CV {:.3}",
            all.len(),
            m,
            cv
        );

        let mut ids: Vec<String> = pl.iter().map(|q| q.motif_id.clone()).collect();
        ids.sort();
        ids.dedup();
        for id in ids {
            let subset: Vec<usize> = (0..pl.len()).filter(|&i| pl[i].motif_id == id).collect();
            if subset.len() < 3 {
                println!("  {:<18} n={:<3} too few to measure", id, subset.len());
                continue;
            }
            let (m, cv) = stats(&nn(&subset));
            println!(
                "  {:<18} n={:<3} mean nn {:6.1} mm   CV {:.3}",
                id,
                subset.len(),
                m,
                cv
            );
        }
    }
    println!("\nCV is the spread of nearest-neighbour distances relative to their mean.");
    println!("Poisson (no structure at all) sits at about 0.52. Blue noise sits far below it.");
    Ok(())
}
