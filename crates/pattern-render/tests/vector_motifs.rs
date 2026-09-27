//! A drawn motif is drawn at the size it is printed.
//!
//! The claim that justifies keeping a motif as outlines at all. A raster motif
//! is sampled once, when it is imported, at whatever resolution somebody
//! guessed; enlarging it past that shows the guess. These tests are about the
//! difference being real rather than nominal.

mod support;

use glam::Vec2;
use pattern_core::{Lattice, Palette, Pattern, Placement, VectorMotif, VectorShape};
use pattern_render::{render_tile, MotifBank};
use support::inks;
use tiny_skia_path::{PathBuilder, Rect};

const TILE: Vec2 = Vec2::new(100.0, 100.0);

/// A disc, which has no straight edges to hide a coarse rasterisation behind.
fn disc() -> VectorMotif {
    let mut b = PathBuilder::new();
    b.push_circle(50.0, 50.0, 45.0);
    VectorMotif {
        view: Rect::from_xywh(0.0, 0.0, 100.0, 100.0).expect("view"),
        shapes: vec![VectorShape {
            path: b.finish().expect("path"),
            slot: 1,
            even_odd: false,
            opacity: 255,
        }],
    }
}

fn one_motif(bank: &mut MotifBank) -> Pattern {
    bank.insert_vector("disc", Vec2::new(50.0, 50.0), disc())
        .expect("should rasterise");
    Pattern {
        tile_size_mm: TILE,
        lattice: Lattice::block(TILE),
        placements: vec![Placement::new("disc", Vec2::new(50.0, 50.0))],
        ground: None,
        palette: inks(1),
        seed: 1,
    }
}

/// How rough the disc's rim is: the share of its edge pixels that are a hard
/// step rather than a partial coverage.
///
/// A rim rasterised too coarsely and then enlarged has long straight runs and
/// few intermediate values. One rasterised at the size it is drawn has an
/// intermediate value almost everywhere around it.
fn softness(tile: &tiny_skia::Pixmap, palette: &Palette) -> f32 {
    let ink = palette.resolve(1).expect("an ink");
    let ground = palette.ground().expect("a fabric");
    let (mut edge, mut between) = (0usize, 0usize);
    for p in tile.pixels() {
        let c = p.demultiply();
        let rgb = (c.red(), c.green(), c.blue());
        if rgb == (ground.r, ground.g, ground.b) {
            continue;
        }
        edge += 1;
        if rgb != (ink.r, ink.g, ink.b) {
            between += 1;
        }
    }
    if edge == 0 {
        return 0.0;
    }
    between as f32 / edge as f32
}

#[test]
fn a_drawing_is_rasterised_at_the_resolution_it_is_drawn_at() {
    // The property, stated as a ratio so a good guess cannot satisfy it: the
    // share of the disc that is a soft rim falls away as the render gets
    // finer, because a rim is about a pixel wide at any resolution while the
    // disc is more pixels. A motif sampled once at import and scaled to fit
    // improves far more slowly, because past its own size it is enlarging
    // pixels rather than drawing.
    //
    // Measured 2026-09-27, 48 DPI to 600 DPI. From outlines: 0.0424 to 0.0037,
    // a fall of eleven and a half. From a 512 px import raster: 0.0802 to
    // 0.0185, a fall of four. Eight is between them with room either side.
    let mut bank = MotifBank::new();
    let pattern = one_motif(&mut bank);

    let coarse = render_tile(&pattern, &bank, 48.0).expect("should render");
    let fine = render_tile(&pattern, &bank, 600.0).expect("should render");

    let (a, b) = (
        softness(&coarse, &pattern.palette),
        softness(&fine, &pattern.palette),
    );
    assert!(
        b < a / 8.0,
        "the rim was {a:.4} of the motif at 48 DPI and {b:.4} at 600 — a fall of \
         {:.1}, where drawing from outlines gives about eleven and enlarging a \
         fixed raster about four",
        a / b
    );
}

#[test]
fn a_print_resolution_render_is_drawn_not_enlarged() {
    // The same claim as an absolute, at the resolution a print is actually
    // written at. Measured at 600 DPI: 0.0037 from outlines against 0.0185
    // from an import raster, so this passes by five times and fails by two.
    let mut bank = MotifBank::new();
    let pattern = one_motif(&mut bank);
    let tile = render_tile(&pattern, &bank, 600.0).expect("should render");
    let share = softness(&tile, &pattern.palette);
    assert!(
        share < 0.008,
        "{share:.4} of the motif is neither ink nor fabric: its rim is several \
         pixels wide, which is what enlarging a fixed raster looks like"
    );
}

#[test]
fn the_measurements_do_not_depend_on_the_reference_raster() {
    // The bank rasterises once for measuring. That raster must not leak into
    // what is printed — if it did, this would be the size everything came out.
    let mut bank = MotifBank::new();
    let pattern = one_motif(&mut bank);
    let motif = bank.get("disc").expect("in the bank");

    // A disc fills about 0.9 of its box here, so the radius fraction is well
    // under 1 and would be exactly 1 if the reference had been a filled square.
    let f = motif.indexed.coverage_radius_fraction();
    assert!((0.6..0.75).contains(&f), "coverage radius fraction is {f}");
    assert_eq!(motif.indexed.slots_used(), vec![1]);

    // And the print is not 512 px wide just because the reference was.
    let tile = render_tile(&pattern, &bank, 300.0).expect("should render");
    assert!(tile.width() > 1000, "{} px", tile.width());
}
