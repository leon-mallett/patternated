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
    // Thirty-two squared, and a hundredth of it has to differ. The closest
    // real pair is legs against panel, at 28 cells of 1024: a pair of trousers
    // stretched into its own frame IS nearly a rectangle, and the crotch is
    // the whole of the difference. Everything else is 158 cells apart or more.
    const SIDE: u32 = 32;
    // A silhouette is judged by eye, and the one thing a test can say about a
    // set of them is that they are a set: a cut whose arm fell through to
    // another's would be trousers that rendered as a cushion, and the numbers
    // would all still be right.
    //
    // Compared cell by cell rather than by the share of the frame each one
    // fills. That share was the second attempt, after the area of a bounding
    // box, which is one for legs and one for dungarees. It lasted longer and
    // failed the same way: one number cannot say two shapes are different, and
    // a pair of pyjamas and a hat turned out to cover 0.5643 of their frames
    // each, being nothing like each other.
    let mut by_cut: Vec<(&'static str, Vec<bool>)> = Vec::new();
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
        by_cut.push((label, g.occupancy(SIDE)));
    }

    assert_eq!(by_cut.len(), 7, "a cut is missing from the size chart");
    for (i, (a, grid_a)) in by_cut.iter().enumerate() {
        for (b, grid_b) in by_cut.iter().skip(i + 1) {
            let differ = grid_a
                .iter()
                .zip(grid_b)
                .filter(|(one, two)| one != two)
                .count();
            assert!(
                differ * 100 >= (SIDE * SIDE) as usize,
                "{a} and {b} draw the same shape: {differ} cells of {} differ",
                SIDE * SIDE
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

/// On-curve points of a path: the destination of each command, which is where
/// the shape actually goes.
///
/// Not every number, because a control point may legitimately sit outside the
/// frame. That is the whole reason `outline_bounds` takes a tight bound rather
/// than `Path::bounds`, and a test that checked every number would refuse a
/// brim that fits.
fn on_curve(d: &str) -> Vec<(f32, f32)> {
    let mut numbers: Vec<f32> = Vec::new();
    let mut commands: Vec<(char, usize)> = Vec::new();
    for token in d.split_whitespace() {
        let (command, rest) = match token.chars().next() {
            Some(c) if c.is_ascii_alphabetic() => (Some(c), &token[1..]),
            _ => (None, token),
        };
        if let Some(c) = command {
            commands.push((c, numbers.len()));
        }
        if !rest.is_empty() {
            numbers.push(rest.parse().expect("a path holds numbers"));
        }
    }
    commands
        .iter()
        .filter_map(|(command, first)| {
            let pairs = match command {
                'M' | 'L' => 1,
                'Q' => 2,
                'C' => 3,
                _ => return None,
            };
            let at = first + (pairs - 1) * 2;
            Some((numbers[at], numbers[at + 1]))
        })
        .collect()
}

#[test]
fn a_silhouette_path_stays_inside_the_frame_it_is_measured_for() {
    // The extent is the viewBox the path is drawn in, exactly as it is the
    // canvas `render_mockup` draws into. Anything the path reaches past is
    // clipped, and nothing fails: this is the same trap `outline_bounds`
    // exists for, checked on the shape a caller is actually handed.
    for g in Garment::standard() {
        let d = g
            .outline_svg_path()
            .expect("every standard garment has a shape");
        let extent = g.extent_mm();
        let points = on_curve(&d);
        assert!(
            points.len() >= 4,
            "'{}' came back as {} points, which is not a shape",
            g.name,
            points.len()
        );
        for (x, y) in points {
            assert!(
                (-0.01..=extent.x + 0.01).contains(&x) && (-0.01..=extent.y + 0.01).contains(&y),
                "'{}' puts a point at {x}, {y}, outside its own {} x {} mm frame",
                g.name,
                extent.x,
                extent.y
            );
        }
    }
}

#[test]
fn a_set_comes_back_as_one_contour_per_piece() {
    // A neck and a crotch are notches in their own contour rather than
    // contours of their own, which is what lets a caller punch the garment out
    // of a backdrop with even-odd. If one of them ever became a separate
    // contour it would be punched back in, as a hole in the cloth.
    let contours = |name: &str| {
        garment(name)
            .outline_svg_path()
            .expect("a shape")
            .matches('M')
            .count()
    };
    assert_eq!(
        contours("T-shirt, adult M"),
        1,
        "a neck is not a second piece"
    );
    assert_eq!(contours("Trousers, adult M"), 1, "nor is a crotch");
    assert_eq!(
        contours("Pyjamas, age 3–4"),
        2,
        "a top and a pair of trousers"
    );
    assert_eq!(contours("Tote bag"), 3, "a bag and two handles");
    assert_eq!(contours("Dungarees, age 3–4"), 3, "a body and two straps");
}

#[test]
fn the_pieces_of_a_set_do_not_overlap() {
    // The other half of what makes even-odd safe. Two contours that shared a
    // region would cancel there, and a pair of pyjamas would come back with a
    // hole where the trousers crossed the top.
    let d = garment("Pyjamas, age 3–4")
        .outline_svg_path()
        .expect("a shape");
    let spans: Vec<(f32, f32)> = d
        .split('M')
        .filter(|piece| !piece.trim().is_empty())
        .map(|piece| {
            // Every command here takes whole pairs, so the numbers alternate
            // x, y from the start of the contour.
            let xs: Vec<f32> = on_curve(&format!("M{piece}"))
                .iter()
                .map(|(x, _)| *x)
                .collect();
            (
                xs.iter().copied().fold(f32::MAX, f32::min),
                xs.iter().copied().fold(f32::MIN, f32::max),
            )
        })
        .collect();

    assert_eq!(spans.len(), 2, "a pair of pyjamas is two pieces");
    assert!(
        spans[0].1 < spans[1].0,
        "the top reaches to {} and the trousers start at {}, so they overlap",
        spans[0].1,
        spans[1].0
    );
}

/// The cuff of the left sleeve: how wide it is, and how far down it reaches.
///
/// Found rather than looked up, so the test is about the shape rather than
/// about the code that drew it. Everything on the outline past the chest
/// belongs to a sleeve, and the two points furthest out there are the corners
/// of the cuff: the underarm seam turns back towards the body well inside
/// them.
fn cuff(g: &Garment) -> (f32, f32) {
    let d = g.outline_svg_path().expect("a top draws something");
    let past_the_chest = g.extent_mm().x / 2.0 - g.across_mm / 2.0;
    let mut sleeve: Vec<(f32, f32)> = on_curve(&d)
        .into_iter()
        .filter(|(x, _)| *x < past_the_chest)
        .collect();
    sleeve.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(
        sleeve.len() >= 2,
        "{}: a top with sleeves reaches past its own chest",
        g.name
    );
    let (outer, inner) = (sleeve[0], sleeve[1]);
    (
        (outer.0 - inner.0).hypot(outer.1 - inner.1),
        outer.1.max(inner.1),
    )
}

#[test]
fn a_longer_sleeve_hangs_lower_and_ends_narrower() {
    // The silhouette is judged by eye; this holds the direction it was judged
    // in. The second half is the one that matters: a sleeve drawn as a wedge
    // tapering to a point, which is what this was until 2026-11-27, has no
    // cuff to measure at any length, and nothing in the numbers said so.
    let top = |sleeve_mm: f32| Garment {
        name: format!("{sleeve_mm:.0} mm sleeve"),
        across_mm: 330.0,
        length_mm: 420.0,
        cut: Cut::Top {
            shoulder_mm: 285.0,
            sleeve_mm,
        },
    };

    let (short_cuff, short_hang) = cuff(&top(110.0));
    let (long_cuff, long_hang) = cuff(&top(280.0));

    assert!(
        long_hang > short_hang + 20.0,
        "a 280 mm sleeve reaches {long_hang:.0} mm down and a 110 mm one {short_hang:.0} mm"
    );
    assert!(
        long_cuff + 5.0 < short_cuff,
        "a 280 mm sleeve ends {long_cuff:.0} mm across and a 110 mm one {short_cuff:.0} mm"
    );
    for (label, width) in [("110", short_cuff), ("280", long_cuff)] {
        assert!(
            width > 30.0,
            "the {label} mm sleeve ends {width:.0} mm across, which is a point rather than a cuff"
        );
    }
}
