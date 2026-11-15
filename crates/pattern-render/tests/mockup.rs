//! Garment mockups, and the one property they exist for.
//!
//! A mockup that renders the print at a convenient size rather than a true one
//! is worse than no mockup: it invites a scale decision made against a picture
//! that lied. So most of this is about scale.

mod support;

use pattern_core::Lattice;
use pattern_render::{render_mockup, Cut, Garment};
use support::{bank, scattered_pattern, TILE};

/// Widest unbroken run of motif-coloured pixels on any row, near enough to
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
    // direction, because a mockup that is qualitatively right and quantitatively
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
fn every_silhouette_fits_the_frame_it_was_measured_for() {
    // The extent decides the canvas; the outline is drawn into it. Anything
    // the outline reaches past is clipped, and nothing fails. A pair of
    // dungarees lost the tops of its straps that way and a hat lost the
    // underside of its brim, both found by looking at a picture rather than by
    // any test here.
    for g in Garment::standard() {
        let e = g.extent_mm();
        let bounds = g.outline_bounds().expect("every garment has an outline");
        assert!(
            bounds.left() >= -0.5
                && bounds.top() >= -0.5
                && bounds.right() <= e.x + 0.5
                && bounds.bottom() <= e.y + 0.5,
            "{} is drawn outside the {:.0} x {:.0} mm frame it asked for: {bounds:?}",
            g.name,
            e.x,
            e.y
        );
    }
}

#[test]
fn an_extent_says_what_each_cut_actually_needs() {
    // Replaces a test that required every garment to be wider than its chest.
    // That was true of the only two shapes there were and is false of
    // trousers, a hat and a tote, which are exactly as wide as they measure.
    for g in Garment::standard() {
        let e = g.extent_mm();
        assert!(
            e.x >= g.across_mm - 0.5 && e.y >= g.length_mm - 0.5,
            "{} is smaller than its own measurements",
            g.name
        );
        match &g.cut {
            // Sleeves reach past the body, and not by more than the body.
            Cut::Top { .. } => assert!(
                e.x > g.across_mm && e.x < g.across_mm * 2.0,
                "{} reaches {:.0} mm across a {:.0} mm chest",
                g.name,
                e.x,
                g.across_mm
            ),
            // Two pieces side by side, so wider than either.
            Cut::Pyjamas { .. } => assert!(
                e.x > g.across_mm * 2.0,
                "{} should stand two pieces side by side, and is {:.0} mm wide",
                g.name,
                e.x
            ),
            // Handles rise above the bag rather than beside it.
            Cut::Tote { .. } => {
                assert!((e.x - g.across_mm).abs() < 0.5);
                assert!(e.y > g.length_mm, "{} has no room for its handles", g.name);
            }
            Cut::Panel | Cut::Legs { .. } | Cut::Dungarees { .. } | Cut::Hat { .. } => {
                assert!(
                    (e.x - g.across_mm).abs() < 0.5 && (e.y - g.length_mm).abs() < 0.5,
                    "{} should be exactly its own measurements",
                    g.name
                );
            }
        }
    }
}

#[test]
fn no_two_cuts_draw_the_same_shape() {
    // A silhouette is judged by eye, and the one thing a test can say about a
    // set of them is that they are a set: a cut whose arm fell through to
    // another's would be trousers that rendered as a cushion, and the numbers
    // would all still be right.
    //
    // Compared as the share of its own frame the silhouette covers when it is
    // drawn: a square panel fills it, a pair of legs does not. The area of a
    // *bounding box* was the first attempt and is one for legs and one for
    // dungarees, which are not remotely the same shape.
    let mut by_cut: Vec<(&'static str, f32)> = Vec::new();
    for g in Garment::standard() {
        let label = match g.cut {
            Cut::Panel => "panel",
            Cut::Top { .. } => "top",
            Cut::Legs { .. } => "legs",
            Cut::Dungarees { .. } => "dungarees",
            Cut::Hat { .. } => "hat",
            Cut::Tote { .. } => "tote",
            Cut::Pyjamas { .. } => "pyjamas",
        };
        if by_cut.iter().any(|(seen, _)| *seen == label) {
            continue;
        }
        by_cut.push((label, g.coverage()));
    }

    assert_eq!(by_cut.len(), 7, "a cut is missing from the size chart");
    for (i, (a, fill_a)) in by_cut.iter().enumerate() {
        for (b, fill_b) in by_cut.iter().skip(i + 1) {
            assert!(
                (fill_a - fill_b).abs() > 0.01,
                "{a} and {b} fill their frames identically ({fill_a:.4})"
            );
        }
    }
}

#[test]
fn the_print_on_a_garment_is_a_repeating_unit() {
    // The swatch filling the garment is tiled by a shader as a plain
    // rectangle, so it has to be a repeating unit itself. A three-by-three
    // window of a half-drop is not, and the garment would show a seam every
    // three repeats, the same trap as a pre-repeated export.
    let pattern = scattered_pattern(Lattice::half_drop(TILE), 5, 0.3);
    let (cols, rows) = pattern.lattice.repeating_window(TILE, 1, 1);
    assert_eq!(
        pattern.lattice.repeating_window(TILE, cols, rows),
        (cols, rows),
        "the swatch the mockup fills with does not tile"
    );
}
