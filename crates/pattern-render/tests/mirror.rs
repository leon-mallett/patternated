//! What makes a mirror repeat a mirror repeat: the rendered tile is its own
//! reflection.
//!
//! Seamlessness is covered where every other repeat's is, in `seamlessness.rs`.
//! These tests are about the property that is new here, and the reason it is
//! worth asserting on pixels rather than on placements is that the placements
//! can be perfectly symmetric while the picture is not. A sprite of odd width
//! centred on a pixel sits half a pixel to one side of it; reflect the centre
//! and round it the same way and the copy lands a whole pixel out. That is
//! invisible in a preview and is exactly the sort of thing that turns up as a
//! hairline down the middle of a printed metre.

mod support;

use pattern_core::{Lattice, Mirror};
use pattern_render::{render_field, render_field_direct, render_tile};
use support::{bank, mirrored_pattern, mirrored_pattern_with, scattered_pattern, TILE};

/// 127 DPI is five pixels to the millimetre exactly, so the 240 x 180 mm
/// fixture tile is 1200 x 900 px and a reflected position rounds to exactly
/// the reflection of the rounded position. At an arbitrary resolution the two
/// roundings can disagree by one pixel, which
/// `quantisation_is_the_only_thing_that_breaks_the_symmetry` measures.
const EXACT_DPI: f32 = 127.0;

/// Coverage high enough that the cell holds eight to eighteen motifs.
///
/// Higher than a plain scatter needs, because a mirror's cell is a half or a
/// quarter of the tile and the fixture motif is large against it.
const DENSITY: f32 = 1.2;

/// Pixels where an image differs from itself reflected left to right.
fn asymmetry_x(image: &tiny_skia::Pixmap) -> usize {
    let (w, h) = (image.width(), image.height());
    let px = image.pixels();
    let mut differing = 0;
    for y in 0..h {
        for x in 0..w {
            let a = px[(y * w + x) as usize];
            let b = px[(y * w + (w - 1 - x)) as usize];
            if a != b {
                differing += 1;
            }
        }
    }
    differing
}

/// Pixels where an image differs from itself reflected top to bottom.
fn asymmetry_y(image: &tiny_skia::Pixmap) -> usize {
    let (w, h) = (image.width(), image.height());
    let px = image.pixels();
    let mut differing = 0;
    for y in 0..h {
        for x in 0..w {
            let a = px[(y * w + x) as usize];
            let b = px[((h - 1 - y) * w + x) as usize];
            if a != b {
                differing += 1;
            }
        }
    }
    differing
}

#[test]
fn a_side_to_side_mirror_tile_is_its_own_reflection() {
    for seed in [1u64, 7, 20260921] {
        let pattern = mirrored_pattern(Mirror::SideToSide, seed, DENSITY);
        assert!(
            pattern.placements.len() >= 8,
            "seed {seed} placed only {} motifs",
            pattern.placements.len()
        );

        let tile = render_tile(&pattern, &bank(), EXACT_DPI).expect("tile should render");
        assert_eq!(
            asymmetry_x(&tile),
            0,
            "seed {seed}: tile is not symmetric left to right"
        );
    }
}

#[test]
fn a_four_way_mirror_tile_is_its_own_reflection_in_both_axes() {
    for seed in [1u64, 7, 20260921] {
        let pattern = mirrored_pattern(Mirror::FourWay, seed, DENSITY);
        let tile = render_tile(&pattern, &bank(), EXACT_DPI).expect("tile should render");
        assert_eq!(asymmetry_x(&tile), 0, "seed {seed}: not symmetric in x");
        assert_eq!(asymmetry_y(&tile), 0, "seed {seed}: not symmetric in y");
    }
}

#[test]
fn a_top_to_bottom_mirror_is_symmetric_only_in_the_axis_it_names() {
    // Guards against a mirror that reflects everything regardless of which was
    // asked for — which would pass both tests above and be the wrong pattern.
    let pattern = mirrored_pattern(Mirror::TopToBottom, 7, DENSITY);
    let tile = render_tile(&pattern, &bank(), EXACT_DPI).expect("tile should render");
    assert_eq!(asymmetry_y(&tile), 0, "not symmetric top to bottom");
    assert!(
        asymmetry_x(&tile) > 1000,
        "a top-to-bottom mirror should not be symmetric side to side, but only {} pixels differ",
        asymmetry_x(&tile)
    );
}

#[test]
fn an_ordinary_scatter_is_not_symmetric() {
    // Kept so the tests above are known to be measuring something that can
    // fail. Without it, a bug that made `asymmetry_x` return zero for any
    // input would leave all of them passing.
    let pattern = scattered_pattern(Lattice::block(TILE), 7, 0.3);
    let tile = render_tile(&pattern, &bank(), EXACT_DPI).expect("tile should render");
    assert!(
        asymmetry_x(&tile) > 1000,
        "a plain scatter came out symmetric, which means the measurement is broken"
    );
}

#[test]
fn quantisation_is_the_only_thing_that_breaks_the_symmetry() {
    // At a resolution where millimetres and pixels do not divide, a reflected
    // centre can round one pixel away from the reflection of the original
    // centre. The symmetry then holds everywhere except along motif edges, and
    // the measurement below is what "except along motif edges" costs.
    //
    // Recorded rather than asserted loosely: if this number climbs, something
    // has gone wrong that a tolerance would have hidden.
    let pattern = mirrored_pattern(Mirror::SideToSide, 7, DENSITY);
    let tile = render_tile(&pattern, &bank(), 150.0).expect("tile should render");
    let differing = asymmetry_x(&tile);
    let total = (tile.width() * tile.height()) as usize;

    let share = differing as f32 / total as f32;
    assert!(
        share < 0.02,
        "{differing} of {total} pixels ({:.2}%) break the symmetry at 150 DPI",
        share * 100.0
    );
}

#[test]
fn a_mirror_repeat_tiles_with_no_seam() {
    // The property every repeat in this engine has to have, restated for the
    // one whose tile is built by reflection rather than by sampling all of it.
    let bank = bank();
    for mirror in [Mirror::SideToSide, Mirror::TopToBottom, Mirror::FourWay] {
        for dpi in [96.0, 150.0] {
            let pattern = mirrored_pattern_with(mirror, 7, DENSITY, 0.45);
            let tiled = render_field(&pattern, &bank, dpi, 3, 3).expect("field should render");
            let direct =
                render_field_direct(&pattern, &bank, dpi, 3, 3).expect("field should render");
            assert_eq!(
                tiled.width(),
                direct.width(),
                "{mirror:?} at {dpi} DPI: fields differ in size"
            );
            let differing = tiled
                .pixels()
                .iter()
                .zip(direct.pixels())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(differing, 0, "seam in {mirror:?} at {dpi} DPI");
        }
    }
}

#[test]
fn a_reflected_placement_needs_a_block_lattice() {
    // A mirror's reflections generate translations by the tile's own width and
    // height. On a half-drop those are not lattice vectors, so the reflected
    // copies would not line up and the tile would break along every axis.
    let mut pattern = mirrored_pattern(Mirror::SideToSide, 7, DENSITY);
    pattern.lattice = Lattice::half_drop(TILE);
    let err = render_tile(&pattern, &bank(), EXACT_DPI).expect_err("should be refused");
    assert!(
        err.to_string().contains("block lattice"),
        "unhelpful message: {err}"
    );
}

/// How much of a band either side of a mirror axis is bare ground.
///
/// The corridor a plain mirror leaves is exactly this: a stripe down each
/// mirror line that nothing may enter, because a motif entering it would be
/// cut and doubled lopsidedly.
fn ground_share_near_the_axis(image: &tiny_skia::Pixmap, ground: pattern_core::Color) -> f32 {
    let (w, h) = (image.width(), image.height());
    let band = w / 12;
    let axis = w / 2;
    let px = image.pixels();

    let mut bare = 0usize;
    let mut total = 0usize;
    for y in 0..h {
        for x in axis.saturating_sub(band)..(axis + band).min(w) {
            let p = px[(y * w + x) as usize].demultiply();
            total += 1;
            if (p.red(), p.green(), p.blue()) == (ground.r, ground.g, ground.b) {
                bare += 1;
            }
        }
    }
    bare as f32 / total.max(1) as f32
}

#[test]
fn motifs_on_the_mirror_lines_keep_the_tile_exactly_symmetric() {
    // The property that makes mounting a motif on an axis worth doing at all.
    // A motif there is cut at the line and completed by its own reflection, so
    // it comes out bilaterally symmetrical rather than doubled over itself.
    for mirror in [Mirror::SideToSide, Mirror::TopToBottom, Mirror::FourWay] {
        for seed in [1u64, 7, 20260921] {
            let pattern = mirrored_pattern_with(mirror, seed, DENSITY, 0.45);
            let tile = render_tile(&pattern, &bank(), EXACT_DPI).expect("tile should render");
            if mirror.reflects_x() {
                assert_eq!(
                    asymmetry_x(&tile),
                    0,
                    "{mirror:?} seed {seed}: not symmetric in x"
                );
            }
            if mirror.reflects_y() {
                assert_eq!(
                    asymmetry_y(&tile),
                    0,
                    "{mirror:?} seed {seed}: not symmetric in y"
                );
            }
        }
    }
}

#[test]
fn asking_for_motifs_on_the_lines_actually_puts_some_there() {
    // Guards the whole feature against quietly doing nothing: the symmetry
    // tests above would all still pass if no motif were ever mounted.
    let pattern = mirrored_pattern_with(Mirror::SideToSide, 7, DENSITY, 0.45);
    let axes = [0.0, TILE.x / 2.0, TILE.x];
    let mounted = pattern
        .placements
        .iter()
        .filter(|p| axes.iter().any(|a| (p.position_mm.x - a).abs() < 1e-3))
        .count();
    assert!(
        mounted >= 4,
        "only {mounted} of {} placements sit on a mirror line",
        pattern.placements.len()
    );
}

#[test]
fn mounting_motifs_on_the_lines_is_what_clears_the_corridor() {
    // The reason the feature exists. A plain mirror keeps every motif its own
    // radius clear of each axis, which with a large motif reads as a bare
    // stripe down the middle of the repeat.
    let bank = bank();
    let ground = mirrored_pattern(Mirror::SideToSide, 7, DENSITY)
        .palette
        .ground()
        .expect("palette has a ground");

    let plain = render_tile(
        &mirrored_pattern(Mirror::SideToSide, 7, DENSITY),
        &bank,
        EXACT_DPI,
    )
    .expect("tile should render");
    let damask = render_tile(
        &mirrored_pattern_with(Mirror::SideToSide, 7, DENSITY, 0.45),
        &bank,
        EXACT_DPI,
    )
    .expect("tile should render");

    let bare_plain = ground_share_near_the_axis(&plain, ground);
    let bare_damask = ground_share_near_the_axis(&damask, ground);

    assert!(
        bare_plain > 0.9,
        "a plain mirror should leave the axis bare, but only {:.0}% of the band is ground",
        bare_plain * 100.0
    );
    assert!(
        bare_damask < bare_plain - 0.15,
        "mounting motifs on the line left {:.0}% bare against the plain mirror's {:.0}%",
        bare_damask * 100.0,
        bare_plain * 100.0
    );
}
