//! Whether a ground's edges are drawn or merely decided.
//!
//! For the first three kinds of ground this question did not arise. A stripe's
//! edges are axis-aligned, and one sample per pixel puts them exactly where
//! they belong. A Truchet arc and a grown blob are curves, and one sample per
//! pixel puts a staircase along every one of them, next to motifs that
//! tiny-skia draws with filtering, so the two layers of the same print did not
//! look as though they had been made by the same tool.
//!
//! Reported from the app as the grounds looking "very pixelated", which is the
//! right description.
//!
//! These tests state the property rather than pinning a picture: a curved edge
//! carries intermediate tones, and the areas either side of it do not.

mod support;

use glam::Vec2;
use pattern_core::{Figure, Growth, Ornament, Pattern, Texture};
use pattern_render::{render_tile, MotifBank};
use support::{ground_only, inks};

const TILE: Vec2 = Vec2::new(240.0, 180.0);
const DPI: f32 = 96.0;

/// How the tile's pixels divide into the two named colours and everything else.
///
/// Returns the share that is exactly one of the two, the share that is neither
/// (which for a two-colour ground can only be an edge) and how many distinct
/// tones there are in total.
///
/// The count matters as much as the share. The *number* of edge pixels is
/// roughly the perimeter of the figure however many samples are taken per
/// pixel, so a share alone cannot tell four samples from two; the number of
/// distinct levels along the edge can, and it is the levels that decide
/// whether a step is visible.
fn tones(pattern: &Pattern) -> (f32, f32, usize) {
    let tile = render_tile(pattern, &MotifBank::new(), DPI).expect("should render");
    let named: Vec<(u8, u8, u8)> = pattern
        .palette
        .colors
        .iter()
        .map(|c| (c.r, c.g, c.b))
        .collect();

    let (mut flat, mut between) = (0usize, 0usize);
    let mut distinct = std::collections::HashSet::new();
    for p in tile.pixels() {
        let c = p.demultiply();
        let rgb = (c.red(), c.green(), c.blue());
        distinct.insert(rgb);
        if named.contains(&rgb) {
            flat += 1;
        } else {
            between += 1;
        }
    }
    let total = tile.pixels().len() as f32;
    (flat as f32 / total, between as f32 / total, distinct.len())
}

/// The same ground decided once per pixel instead of drawn, as it was before.
///
/// Worked out here rather than asked of the renderer, so that the claim below
/// is measured against something independent of the code under test.
fn hard_edged_between(pattern: &Pattern) -> f32 {
    let laid = pattern
        .ground
        .as_ref()
        .expect("a ground")
        .laid_over(TILE.x, TILE.y);
    let tile = render_tile(pattern, &MotifBank::new(), DPI).expect("should render");
    let (w, h) = (tile.width(), tile.height());
    let scale = Vec2::new(TILE.x / w as f32, TILE.y / h as f32);

    let named: Vec<(u8, u8, u8)> = pattern
        .palette
        .colors
        .iter()
        .map(|c| (c.r, c.g, c.b))
        .collect();
    let mut between = 0usize;
    for y in 0..h {
        for x in 0..w {
            let slot = laid.slot_at((x as f32 + 0.5) * scale.x, (y as f32 + 0.5) * scale.y);
            let colour = slot
                .and_then(|s| pattern.palette.resolve(s))
                .unwrap_or(pattern.palette.ground().expect("a fabric"));
            if !named.contains(&(colour.r, colour.g, colour.b)) {
                between += 1;
            }
        }
    }
    between as f32 / (w * h) as f32
}

fn truchet() -> Pattern {
    ground_only(
        Ornament {
            figure: Figure::Truchet,
            cell_mm: 30.0,
            stroke_mm: 5.0,
            levels: vec![1],
            ..Ornament::default()
        }
        .into(),
        inks(1),
    )
}

fn pebbles() -> Pattern {
    ground_only(
        Texture {
            growth: Growth::Pebbles,
            cells: 96,
            steps: 1200,
            seed: 3,
            levels: vec![pattern_core::GROUND_SLOT, 1],
        }
        .into(),
        inks(1),
    )
}

#[test]
fn a_curved_ground_carries_intermediate_tones_along_its_edges() {
    // The whole property. A two-colour ground whose pixels are only ever one
    // of those two colours has decided each pixel rather than drawn it, and
    // every curve in it is a staircase.
    for (name, pattern) in [("truchet", truchet()), ("pebbles", pebbles())] {
        let (_, between, distinct) = tones(&pattern);
        // Half a per cent rather than something rounder: a field of pebbles is
        // a handful of blobs and has far less perimeter than a field of
        // strokes, so the share that is edge is genuinely small. What is not
        // small is the difference from nothing, which is what the next test
        // pins.
        assert!(
            between > 0.005,
            "{name}: only {:.2}% of the tile is between the two colours, so its \
             edges are decided rather than drawn",
            between * 100.0
        );
        // Sixteen samples give seventeen levels of coverage plus the two flat
        // colours. Four samples would give five, which is few enough to see.
        assert!(
            distinct > 8,
            "{name}: {distinct} distinct tones in a two-colour ground is too few \
             for an edge that does not step"
        );
    }
}

#[test]
fn deciding_each_pixel_once_is_what_the_complaint_looked_like() {
    // Kept so the test above is known to be measuring something that can fail.
    // Sampling the middle of each pixel and taking the answer, which is what
    // this did until it was reported, can only ever produce the palette's own
    // colours, so there is nothing along an edge but a step.
    for (name, pattern) in [("truchet", truchet()), ("pebbles", pebbles())] {
        assert_eq!(
            hard_edged_between(&pattern),
            0.0,
            "{name}: a once-per-pixel ground should produce no intermediate tone at all"
        );
    }
}

#[test]
fn the_areas_either_side_of_an_edge_stay_flat() {
    // The other half of the property, and the one that would catch this being
    // "fixed" by blurring the whole layer. A print is flat colour with edges
    // between; most of the tile should still be exactly one of the two.
    for (name, pattern) in [("truchet", truchet()), ("pebbles", pebbles())] {
        let (flat, _, _) = tones(&pattern);
        assert!(
            flat > 0.80,
            "{name}: only {:.1}% of the tile is exactly one of the two colours",
            flat * 100.0
        );
    }
}

#[test]
fn a_straight_ground_is_left_alone() {
    // A stripe's edges are axis-aligned, so they land on pixel boundaries and
    // there is nothing to soften. Worth stating: it is the reason one sample
    // per pixel was right for as long as bands were the only ground, and the
    // reason this change should be invisible on a check.
    //
    // 20 mm bands into a 240 mm tile divide exactly, so every edge is a whole
    // number of pixels at 96 DPI... except that 240 mm at 96 DPI is 907 px and
    // a 20 mm band is 75.6, so they do not. The assertion is therefore that
    // softening is confined to those edges rather than absent.
    let stripe = ground_only(pattern_core::Bands::stripe(20.0, 20.0, 1).into(), inks(1));
    let (flat, between, _) = tones(&stripe);
    assert!(
        between < 0.02,
        "a stripe should be almost entirely flat colour, not {:.1}% edges",
        between * 100.0
    );
    assert!(flat > 0.97, "{flat}");
}
