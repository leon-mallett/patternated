//! What did a grid actually put in each cell?
//!
//!     cargo run -p pattern-cli --example grid-layout -- path/to/document.json
//!
//! A grid draws its motif per cell independently, so with fifteen subjects
//! over thirty cells some land three times, some not at all, and two of the
//! same can sit side by side. On a tossed scatter none of that shows. On a
//! grid it is the first thing the eye finds, because the grid has already
//! told the eye where to look.
//!
//! Prints one line per placement so a caller can score a seed, then the two
//! facts worth scoring on: how many times each subject appears, and how many
//! neighbouring cells hold the same subject. "Neighbouring" is measured on the
//! lattice torus rather than by cell index, because a half-drop means the
//! nearest cell in the next column is not the one with the same row number.
use std::collections::BTreeMap;
use std::env;
use std::path::Path;

use glam::Vec2;
use pattern_doc::Document;

fn toroidal(a: Vec2, b: Vec2, u: Vec2, v: Vec2) -> f32 {
    let mut best = f32::MAX;
    for i in -1..=1 {
        for j in -1..=1 {
            let o = Vec2::new(
                u.x * i as f32 + v.x * j as f32,
                u.y * i as f32 + v.y * j as f32,
            );
            let d = ((a.x - b.x - o.x).powi(2) + (a.y - b.y - o.y).powi(2)).sqrt();
            best = best.min(d);
        }
    }
    best
}

fn main() -> anyhow::Result<()> {
    for arg in env::args().skip(1) {
        let (doc, _bank) = Document::load(Path::new(&arg))?;
        let p = &doc.pattern;
        let (u, v) = (p.lattice.u, p.lattice.v);
        let pl = &p.placements;

        for q in pl {
            println!(
                "cell {:<14} {:7.2} {:7.2}",
                q.motif_id, q.position_mm.x, q.position_mm.y
            );
        }

        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for q in pl {
            *counts.entry(q.motif_id.as_str()).or_default() += 1;
        }
        for m in &doc.motifs {
            counts.entry(m.id.as_str()).or_default();
        }

        // The spacing a cell implies, from the area each placement has to
        // itself. Anything closer than 1.4 of that is a neighbour rather than
        // a cell further along the row.
        let cell = ((u.x * v.y - u.y * v.x).abs() / pl.len() as f32).sqrt();
        let reach = cell * 1.4;
        let mut touching = 0;
        for i in 0..pl.len() {
            for j in (i + 1)..pl.len() {
                if pl[i].motif_id == pl[j].motif_id
                    && toroidal(pl[i].position_mm, pl[j].position_mm, u, v) < reach
                {
                    touching += 1;
                }
            }
        }

        let missing = counts.values().filter(|&&n| n == 0).count();
        let most = counts.values().copied().max().unwrap_or(0);
        let least = counts.values().copied().min().unwrap_or(0);
        println!(
            "summary placements {} cells, subjects {}, each {}..{}, missing {}, \
             same-subject neighbours {} (within {:.0} mm)",
            pl.len(),
            counts.len(),
            least,
            most,
            missing,
            touching,
            reach
        );
    }
    Ok(())
}
