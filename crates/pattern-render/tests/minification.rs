//! Drawing artwork smaller than it was drawn.
//!
//! Almost every render minifies. Artwork arrives at illustration resolution —
//! a thousand pixels or more — and a 100 mm motif is under two hundred pixels
//! at preview scale and about six hundred at print. Bilinear sampling reads a
//! 2x2 neighbourhood, so reducing by eight in one step looks at four source
//! pixels in every sixty-four and discards the rest. A dark keyline a few
//! pixels wide falls between samples and vanishes in places.
//!
//! Reported from the app as "the preview is far worse than the source", which
//! is the right description: it was not merely smaller, it was wrong.
//!
//! The shared fixture could not have caught it. Its cherry is drawn at 128 px,
//! so nothing is ever reduced by more than about two and the defect does not
//! appear — the same trap the log already records about synthetic fixtures,
//! arriving from the other direction. These tests draw the same cherry at
//! illustration resolution instead.

mod support;

use glam::Vec2;
use motif_ingest::{matte, quantise, trim, MatteMethod, SourceImage};
use pattern_core::{Lattice, Motif, Pattern, Placement};
use pattern_render::{render_tile, MotifBank};
use support::{cherry, palette};

/// 254 mm is ten inches, so the tile is a whole number of pixels at both
/// resolutions below and the reference downsample is an exact integer ratio.
const TILE_MM: f32 = 254.0;
const FINE_DPI: f32 = 300.0;
const COARSE_DPI: f32 = 50.0;

/// The fixture cherry at a chosen source resolution, through the real ingest.
fn bank_at(source_px: u32) -> MotifBank {
    let drawn = cherry(source_px);
    let mut rgba = Vec::with_capacity((drawn.width() * drawn.height() * 4) as usize);
    for p in drawn.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let source = SourceImage::new(drawn.width(), drawn.height(), rgba).expect("well formed");
    let (cropped, _) = trim(&matte(&source, MatteMethod::ExistingAlpha), 8).expect("not blank");
    let (indexed, _) = quantise(&cropped, &palette()).expect("should quantise");

    let mut bank = MotifBank::new();
    bank.insert(Motif {
        id: "cherry".into(),
        natural_size_mm: Vec2::new(100.0, 100.0),
        indexed,
    });
    bank
}

fn single_motif() -> Pattern {
    let tile = Vec2::splat(TILE_MM);
    Pattern {
        tile_size_mm: tile,
        lattice: Lattice::block(tile),
        placements: vec![Placement::new("cherry", Vec2::splat(TILE_MM / 2.0))],
        palette: palette(),
        seed: 1,
    }
}

/// Box-average `factor` x `factor` blocks of a pixmap.
///
/// A correct minification, worked out here rather than asked of the renderer,
/// so that a mistake shared with the code under test cannot confirm itself.
/// Every source pixel contributes exactly once, which is the property bilinear
/// sampling gives up.
fn box_filtered(src: &tiny_skia::Pixmap, factor: u32) -> Vec<[f64; 3]> {
    let (w, h) = (src.width(), src.height());
    let (out_w, out_h) = (w / factor, h / factor);
    let px = src.pixels();
    let n = f64::from(factor * factor);

    let mut out = Vec::with_capacity((out_w * out_h) as usize);
    for y in 0..out_h {
        for x in 0..out_w {
            let mut sum = [0f64; 3];
            for dy in 0..factor {
                for dx in 0..factor {
                    let c = px[((y * factor + dy) * w + x * factor + dx) as usize].demultiply();
                    sum[0] += f64::from(c.red());
                    sum[1] += f64::from(c.green());
                    sum[2] += f64::from(c.blue());
                }
            }
            out.push([sum[0] / n, sum[1] / n, sum[2] / n]);
        }
    }
    out
}

/// How far a coarse render falls from a correctly minified fine one.
///
/// Returns the mean and the worst single pixel. The worst is what matters: a
/// keyline dropping out is a handful of pixels being completely wrong, which
/// a mean over a mostly-empty tile barely registers.
fn error_against_box_filter(source_px: u32) -> (f64, f64) {
    let bank = bank_at(source_px);
    let pattern = single_motif();

    let fine = render_tile(&pattern, &bank, FINE_DPI).expect("fine tile should render");
    let coarse = render_tile(&pattern, &bank, COARSE_DPI).expect("coarse tile should render");
    assert_eq!(
        fine.width() % coarse.width(),
        0,
        "the two resolutions must divide, or the reference is not exact"
    );

    let reference = box_filtered(&fine, fine.width() / coarse.width());
    let px = coarse.pixels();
    let (mut total, mut worst) = (0.0, 0.0f64);
    for (i, want) in reference.iter().enumerate() {
        let c = px[i].demultiply();
        let d = ((f64::from(c.red()) - want[0]).abs()
            + (f64::from(c.green()) - want[1]).abs()
            + (f64::from(c.blue()) - want[2]).abs())
            / 3.0;
        total += d;
        worst = worst.max(d);
    }
    (total / reference.len() as f64, worst)
}

#[test]
fn more_detailed_artwork_does_not_render_worse() {
    // The whole property, and the one that cannot be satisfied by accident.
    //
    // Reducing correctly means every source pixel contributes, so a source
    // with more detail can only give a *better* answer. Sampling a 2x2
    // neighbourhood means a source with more detail gives a worse one, because
    // the share of it that is read shrinks.
    //
    // Measured 2026-09-22, worst-pixel error against the box-filtered
    // reference. With pre-filtering: 92, 78, 77 at 256, 512 and 1024 px.
    // Without it: 92, 122, 145 — the wrong direction, and by enough that no
    // tolerance could cover both.
    let coarse = error_against_box_filter(256);
    let detailed = error_against_box_filter(1024);

    assert!(
        detailed.1 <= coarse.1,
        "a 1024 px source rendered worse than a 256 px one ({:.1} against {:.1}); \
         minification is discarding detail rather than filtering it",
        detailed.1,
        coarse.1
    );
    assert!(
        detailed.0 <= coarse.0,
        "mean error rose with source detail: {:.3} against {:.3}",
        detailed.0,
        coarse.0
    );
}

#[test]
fn a_heavily_reduced_motif_stays_close_to_a_correct_reduction() {
    // The absolute claim beside the relative one. A 1024 px cherry drawn at
    // 100 mm and 50 DPI is reduced about five times.
    let (mean, worst) = error_against_box_filter(1024);
    assert!(
        mean < 0.2,
        "mean channel error {mean:.3} against a box-filtered reference"
    );
    assert!(
        worst < 90.0,
        "worst pixel is {worst:.1} channel steps from a correct reduction"
    );
}

#[test]
fn a_source_that_needs_no_reduction_is_untouched_by_the_filter() {
    // The control. At 100 mm and 50 DPI the motif is about 197 px, so a 256 px
    // source is already within a factor of two and no level below the base is
    // chosen. If this moved, the chain would be reducing when it should not.
    let (mean, worst) = error_against_box_filter(256);
    assert!(mean < 0.3, "mean {mean:.3}");
    assert!(
        worst > 50.0,
        "expected the un-prefiltered case to carry its usual bilinear error, got {worst:.1}"
    );
}
