//! Garment mockups, and the one property they exist for.
//!
//! A mockup that renders the print at a convenient size rather than a true one
//! is worse than no mockup: it invites a scale decision made against a picture
//! that lied. So most of this is about scale.

mod support;

use pattern_core::Lattice;
use pattern_render::{render_mockup, Garment};
use support::{bank, scattered_pattern, TILE};

/// Widest unbroken run of motif-coloured pixels on any row — near enough to
/// "how big a motif looks" for a scale comparison.
fn apparent_motif_px(pixmap: &tiny_skia::Pixmap) -> u32 {
    let (w, h) = (pixmap.width() as usize, pixmap.height() as usize);
    let px = pixmap.pixels();
    let mut widest = 0u32;
    for y in 0..h {
        let mut run = 0u32;
        for x in 0..w {
            let c = px[y * w + x].demultiply();
            let inky = c.red() > 150 && c.green() < 110 && c.blue() < 110;
            run = if inky { run + 1 } else { 0 };
            widest = widest.max(run);
        }
    }
    widest
}

fn garment(name: &str) -> Garment {
    Garment::standard()
        .into_iter()
        .find(|g| g.name == name)
        .expect("a standard garment by that name")
}

#[test]
fn the_same_repeat_looks_bigger_on_a_smaller_garment() {
    // The whole point, and the spec's own example: a repeat reads completely
    // differently on an adult shirt and a three-year-old's. Both are drawn to
    // the same width on screen, so the only thing that can differ is how much
    // of the garment one motif covers.
    let bank = bank();
    let pattern = scattered_pattern(Lattice::half_drop(TILE), 7, 0.3);

    let child =
        render_mockup(&pattern, &bank, &garment("T-shirt, age 3–4"), 420).expect("should render");
    let adult =
        render_mockup(&pattern, &bank, &garment("T-shirt, adult L"), 420).expect("should render");

    let small = apparent_motif_px(&child.pixmap);
    let large = apparent_motif_px(&adult.pixmap);
    assert!(
        small > large,
        "a motif covered {small} px of a child's shirt and {large} px of an adult's, \
         which is the wrong way round"
    );

    // And by roughly the ratio of the garments, not merely in the right
    // direction — a mockup that is qualitatively right and quantitatively
    // wrong still invites a bad decision.
    let expected = adult.extent_mm.x / child.extent_mm.x;
    let actual = small as f32 / large as f32;
    assert!(
        (actual / expected - 1.0).abs() < 0.25,
        "expected about {expected:.2}x, got {actual:.2}x"
    );
}

#[test]
fn repeats_across_the_chest_is_reported_and_correct() {
    let bank = bank();
    let pattern = scattered_pattern(Lattice::block(TILE), 1, 0.3);
    let g = garment("T-shirt, adult M");
    let mockup = render_mockup(&pattern, &bank, &g, 300).expect("should render");

    // 510 mm across the chest, 240 mm repeat.
    assert!(
        (mockup.repeats_across - 510.0 / 240.0).abs() < 0.01,
        "reported {:.2}",
        mockup.repeats_across
    );
}

#[test]
fn a_bigger_repeat_means_fewer_of_them_across_the_chest() {
    let bank = bank();
    let mut pattern = scattered_pattern(Lattice::block(TILE), 1, 0.3);
    let g = garment("T-shirt, adult M");

    let tight = render_mockup(&pattern, &bank, &g, 300)
        .expect("render")
        .repeats_across;
    pattern.tile_size_mm.x *= 2.0;
    let loose = render_mockup(&pattern, &bank, &g, 300)
        .expect("render")
        .repeats_across;
    assert!(loose < tight, "{tight:.2} then {loose:.2}");
}

#[test]
fn every_standard_garment_renders() {
    // Including the cushion, which has no sleeves and takes a different path
    // through the outline.
    let bank = bank();
    let pattern = scattered_pattern(Lattice::half_drop(TILE), 3, 0.3);
    for g in Garment::standard() {
        let mockup = render_mockup(&pattern, &bank, &g, 240)
            .unwrap_or_else(|e| panic!("{} failed to render: {e}", g.name));
        assert!(mockup.pixmap.width() >= 240);
        assert!(mockup.extent_mm.x > 0.0 && mockup.extent_mm.y > 0.0);
    }
}

#[test]
fn a_garment_is_taller_than_it_is_wide_unless_it_is_a_cushion() {
    // A cheap guard on the outline maths: a t-shirt whose extent came out
    // square would mean the sleeve reach had swallowed the body.
    for g in Garment::standard() {
        let e = g.extent_mm();
        if g.name.starts_with("Cushion") {
            assert!((e.x - e.y).abs() < 1.0, "{} is not square", g.name);
        } else {
            assert!(
                e.x > g.chest_mm && e.x < g.chest_mm * 2.0,
                "{} reaches {:.0} mm across a {:.0} mm chest",
                g.name,
                e.x,
                g.chest_mm
            );
        }
    }
}

#[test]
fn the_print_on_a_garment_is_a_repeating_unit() {
    // The swatch filling the garment is tiled by a shader as a plain
    // rectangle, so it has to be a repeating unit itself. A three-by-three
    // window of a half-drop is not, and the garment would show a seam every
    // three repeats — the same trap as a pre-repeated export.
    let pattern = scattered_pattern(Lattice::half_drop(TILE), 5, 0.3);
    let (cols, rows) = pattern.lattice.repeating_window(TILE, 1, 1);
    assert_eq!(
        pattern.lattice.repeating_window(TILE, cols, rows),
        (cols, rows),
        "the swatch the mockup fills with does not tile"
    );
}
