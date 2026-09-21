//! The tests that decide whether fabric comes back right.
//!
//! The central one compares two independent routes to the same field: laying
//! rendered tiles at lattice offsets, which is what a printer does with the
//! file, against drawing every placement straight from the pattern definition,
//! which is what the repeat means. If a motif crossing a tile edge is handled
//! wrongly the two disagree, and the disagreement is exactly the seam that
//! would have appeared on the roll.

mod support;

use pattern_core::Lattice;
use pattern_render::{render_field, render_field_direct, render_tile};
use support::{bank, scattered_pattern, straddling_pattern, TILE};

/// Positions where the two renders differ, and by how much at the worst.
fn differences(a: &tiny_skia::Pixmap, b: &tiny_skia::Pixmap) -> (usize, u8) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let mut count = 0;
    let mut worst = 0u8;
    for (pa, pb) in a.pixels().iter().zip(b.pixels()) {
        let d = [
            pa.red().abs_diff(pb.red()),
            pa.green().abs_diff(pb.green()),
            pa.blue().abs_diff(pb.blue()),
            pa.alpha().abs_diff(pb.alpha()),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        if d > 0 {
            count += 1;
            worst = worst.max(d);
        }
    }
    (count, worst)
}

#[test]
fn a_block_repeat_tiles_with_no_seam() {
    let pattern = straddling_pattern(Lattice::block(TILE));
    let bank = bank();

    let tiled = render_field(&pattern, &bank, 96.0, 3, 3).expect("field should render");
    let direct = render_field_direct(&pattern, &bank, 96.0, 3, 3).expect("field should render");

    let (count, worst) = differences(&tiled, &direct);
    assert_eq!(
        count, 0,
        "{count} pixels differ (worst channel delta {worst}); tiles do not join"
    );
}

#[test]
fn a_half_drop_repeat_tiles_with_no_seam() {
    // The one that matters. A half-drop is where substituting the tile
    // rectangle for the lattice vectors stops being harmless, and the failure
    // is invisible in a single tile.
    let pattern = straddling_pattern(Lattice::half_drop(TILE));
    let bank = bank();

    let tiled = render_field(&pattern, &bank, 96.0, 3, 3).expect("field should render");
    let direct = render_field_direct(&pattern, &bank, 96.0, 3, 3).expect("field should render");

    let (count, worst) = differences(&tiled, &direct);
    assert_eq!(
        count, 0,
        "{count} pixels differ (worst channel delta {worst}); half-drop tiles do not join"
    );
}

#[test]
fn both_repeats_tile_at_every_output_resolution() {
    // Quantisation happens at render time, so seamlessness has to be re-proved
    // at each DPI rather than assumed from one. 150 gives an odd tile height
    // in this repeat, which is the awkward case.
    let bank = bank();
    for lattice in [Lattice::block(TILE), Lattice::half_drop(TILE)] {
        for dpi in [72.0, 96.0, 150.0, 300.0] {
            let pattern = straddling_pattern(lattice);
            let tiled = render_field(&pattern, &bank, dpi, 3, 3).expect("field should render");
            let direct =
                render_field_direct(&pattern, &bank, dpi, 3, 3).expect("field should render");
            let (count, _) = differences(&tiled, &direct);
            assert_eq!(count, 0, "seam at {dpi} DPI with {lattice:?}");
        }
    }
}

#[test]
fn opposite_tile_edges_carry_the_same_motif_pixels() {
    // The direct statement of the property, independent of the field
    // comparison: a motif overlapping the right edge must appear on the left.
    // Sampled on the tile itself, so it holds for the file the bureau receives.
    let pattern = straddling_pattern(Lattice::half_drop(TILE));
    let tile = render_tile(&pattern, &bank(), 96.0).expect("tile should render");

    let ground = pattern.palette.ground().expect("palette has a ground");
    let is_motif = |x: u32, y: u32| {
        let p = tile
            .pixel(x, y)
            .expect("sample should be inside the tile")
            .demultiply();
        (p.red(), p.green(), p.blue()) != (ground.r, ground.g, ground.b)
    };

    // The placement at x = 238 mm of a 240 mm tile is cut by the right edge,
    // so both edge columns must carry part of it.
    let last = tile.width() - 1;
    let left: Vec<u32> = (0..tile.height()).filter(|y| is_motif(0, *y)).collect();
    let right: Vec<u32> = (0..tile.height()).filter(|y| is_motif(last, *y)).collect();

    assert!(!left.is_empty(), "left edge carries no motif");
    assert!(!right.is_empty(), "right edge carries no motif");
}

#[test]
fn a_tile_is_exactly_the_repeat_size_in_pixels() {
    // 240 x 180 mm at 150 DPI: 240/25.4*150 = 1417.32, 180/25.4*150 = 1062.99.
    //
    // A block repeat takes those roundings as they come.
    let block = straddling_pattern(Lattice::block(TILE));
    let tile = render_tile(&block, &bank(), 150.0).expect("tile should render");
    assert_eq!((tile.width(), tile.height()), (1417, 1063));

    // A half-drop rounds its height to 1064 instead, because it steps by half
    // of it and half of 1063 is not a whole pixel. Measured on an exported
    // sheet before this existed: four columns came back two pixels adrift and
    // the join between two copies was four times an interior step.
    let half_drop = straddling_pattern(Lattice::half_drop(TILE));
    let tile = render_tile(&half_drop, &bank(), 150.0).expect("tile should render");
    assert_eq!((tile.width(), tile.height()), (1417, 1064));
}

#[test]
fn a_scattered_pattern_tiles_with_no_seam() {
    // What M2 actually produces. The hand-placed tests above put motifs on
    // edges deliberately; here the sampler decides, at a density where several
    // straddle an edge at once and some overlap each other across it.
    let bank = bank();
    for lattice in [Lattice::block(TILE), Lattice::half_drop(TILE)] {
        for seed in [1u64, 20260920] {
            let pattern = scattered_pattern(lattice, seed, 0.55);
            assert!(pattern.placements.len() > 20, "expected a dense scatter");

            let tiled = render_field(&pattern, &bank, 96.0, 3, 3).expect("field should render");
            let direct =
                render_field_direct(&pattern, &bank, 96.0, 3, 3).expect("field should render");

            let (count, worst) = differences(&tiled, &direct);
            assert_eq!(
                count, 0,
                "{count} pixels differ (worst {worst}) for seed {seed} on {lattice:?}"
            );
        }
    }
}

/// Whether a rendered window is itself a repeating unit.
///
/// Tiling the window twice across and twice down must give exactly what
/// rendering a field of twice the size gives. That is the same argument the
/// tile tests use, one level up — and unlike comparing the outermost rows of
/// pixels it is exact, because opposite *edges* of an image are adjacent
/// columns in the pattern rather than the same column, so they differ by
/// however much the artwork changes in one pixel.
fn window_repeats(
    pattern: &pattern_core::Pattern,
    bank: &pattern_render::MotifBank,
    dpi: f32,
    cols: u32,
    rows: u32,
) -> (usize, u8) {
    let window = render_field(pattern, bank, dpi, cols, rows).expect("window should render");
    let doubled =
        render_field(pattern, bank, dpi, cols * 2, rows * 2).expect("field should render");

    let mut assembled =
        tiny_skia::Pixmap::new(doubled.width(), doubled.height()).expect("allocation");
    for y in 0..2 {
        for x in 0..2 {
            assembled.draw_pixmap(
                (x * window.width()) as i32,
                (y * window.height()) as i32,
                window.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    differences(&assembled, &doubled)
}

#[test]
fn a_window_sized_to_repeat_is_a_repeating_unit() {
    // Reported from the app: a 3x3 preview of a half-drop did not meet at its
    // left and right edges. The tile was seamless — the window was not a
    // repeating unit, which is a different thing and looks identical to a
    // broken repeat when you hold the edges together.
    let bank = bank();
    for lattice in [
        Lattice::block(TILE),
        Lattice::half_drop(TILE),
        Lattice::brick(TILE),
        Lattice::quarter_drop(TILE),
    ] {
        let pattern = scattered_pattern(lattice, 7, 0.3);
        let (cols, rows) = lattice.repeating_window(TILE, 3, 3);
        let (count, worst) = window_repeats(&pattern, &bank, 48.0, cols, rows);
        assert_eq!(
            count, 0,
            "{cols}x{rows} window of {lattice:?} is not a repeating unit: \
             {count} pixels differ, worst {worst}"
        );
    }
}

#[test]
fn an_unsuitable_window_is_what_the_complaint_looked_like() {
    // Kept so the test above is known to be measuring something that can fail.
    // Three columns of a half-drop end half a tile out.
    let pattern = scattered_pattern(Lattice::half_drop(TILE), 7, 0.3);
    let (count, _) = window_repeats(&pattern, &bank(), 48.0, 3, 3);
    assert!(
        count > 1000,
        "expected an odd half-drop window not to repeat, but only {count} pixels differed"
    );
}
