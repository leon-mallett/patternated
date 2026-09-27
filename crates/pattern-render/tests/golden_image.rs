//! Golden-image test for render output.
//!
//! Regenerate deliberately with `UPDATE_GOLDEN=1 cargo test -p pattern-render`,
//! then look at the diff before committing it. Regenerating to make a failing
//! test pass discards the only record of what the renderer used to produce.
//!
//! Kept small on purpose: a golden image is reviewed by eye, and nobody reviews
//! a four-megapixel diff.

mod support;

use std::path::PathBuf;

use pattern_core::{Band, Bands, Crossing, Figure, Growth, Lattice, Ornament, Texture};
use pattern_render::{render_field, render_tile};
use support::{bank, ground_only, inks, scattered_pattern, straddling_pattern, TILE};

/// Resolution for the ground goldens.
///
/// Higher than the motif ones, and a single tile rather than a field of nine,
/// because a ground is reviewed for what it looks like rather than for whether
/// it tiles — the seamlessness suite settles tiling exactly, and nobody can
/// see a 30 mm Truchet cell at 24 DPI.
const GROUND_DPI: f32 = 60.0;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

fn check_against_golden(name: &str, pixmap: &tiny_skia::Pixmap) {
    let path = golden_path(name);
    let actual = pixmap.encode_png().expect("render should encode as PNG");

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).expect("golden should be writable");
        eprintln!("regenerated {}", path.display());
        return;
    }

    let expected = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}. Generate it with UPDATE_GOLDEN=1 and review it before \
             committing.",
            path.display()
        )
    });

    if expected == actual {
        return;
    }

    // Report how far off rather than just that it differs — a one-channel
    // rounding change and a broken tile deserve different reactions.
    let golden = tiny_skia::Pixmap::decode_png(&expected).expect("golden should decode");
    assert_eq!(
        (golden.width(), golden.height()),
        (pixmap.width(), pixmap.height()),
        "golden {name} has different dimensions"
    );
    let differing = golden
        .pixels()
        .iter()
        .zip(pixmap.pixels())
        .filter(|(a, b)| a != b)
        .count();
    panic!(
        "{name} differs from its golden in {differing} of {} pixels. If this change is intended, \
         regenerate with UPDATE_GOLDEN=1 and review the diff.",
        golden.pixels().len()
    );
}

#[test]
fn half_drop_field_matches_its_golden() {
    let pattern = straddling_pattern(Lattice::half_drop(TILE));
    let field = render_field(&pattern, &bank(), 24.0, 3, 3).expect("field should render");
    check_against_golden("half-drop-3x3.png", &field);
}

#[test]
fn block_field_matches_its_golden() {
    let pattern = straddling_pattern(Lattice::block(TILE));
    let field = render_field(&pattern, &bank(), 24.0, 3, 3).expect("field should render");
    check_against_golden("block-3x3.png", &field);
}

#[test]
fn a_scattered_field_matches_its_golden() {
    // The determinism tests compare two renders in one process, which cannot
    // detect the scatter changing between versions. This can: the baseline is
    // committed, so a sampler that drifts fails here rather than quietly
    // producing a different print from the same seed months later.
    let pattern = scattered_pattern(Lattice::half_drop(TILE), 20260920, 0.3);
    let field = render_field(&pattern, &bank(), 24.0, 3, 3).expect("field should render");
    check_against_golden("scatter-half-drop-3x3.png", &field);
}

// Goldens for the ground layer.
//
// Three kinds of ground and six figures had no pinned picture at all until
// these: every ground test asserted a property — that it tiles, that its edges
// carry intermediate tones, that a level covers some of the tile — and a
// property that still holds can hold over a print that has changed completely.
// Changing how every ground's edges are drawn broke no golden, which is how the
// gap was noticed.
//
// One per kind rather than one per figure. What a figure *is* is already pinned
// exactly, and more cheaply, by the digest in `pattern-core`'s ornament tests;
// what these add is everything after it — millimetres to pixels, stroke widths,
// slots through the palette, and the softening of an edge.

#[test]
fn a_checked_ground_matches_its_golden() {
    // Two threads and their crossing: the three tones that no arrangement of
    // motifs can produce. Pins the crossing colours as drawn, and that band
    // edges stay hard — they are axis-aligned, so softening them would be a
    // regression rather than an improvement.
    let navy = 1;
    let pale = 2;
    let mixed = 3;
    let thread = |slot| vec![Band::new(20.0, slot), Band::new(20.0, pale)];
    let bands = Bands {
        vertical: thread(navy),
        horizontal: thread(navy),
        crossings: vec![
            Crossing {
                a: navy,
                b: navy,
                slot: navy,
            },
            Crossing {
                a: navy,
                b: pale,
                slot: mixed,
            },
            Crossing {
                a: pale,
                b: pale,
                slot: pale,
            },
        ],
    };
    let pattern = ground_only(bands.into(), inks(3));
    let tile = render_tile(&pattern, &bank(), GROUND_DPI).expect("should render");
    check_against_golden("ground-check.png", &tile);
}

#[test]
fn a_grown_ground_matches_its_golden() {
    // The whole chain as a picture: a Gray-Scott field, quantised onto slots,
    // sampled between its cells and resolved through the palette. The unit
    // digest pins the field; nothing pinned what became of it.
    let texture = Texture {
        growth: Growth::Pebbles,
        cells: 48,
        steps: 1200,
        seed: 3,
        levels: vec![pattern_core::GROUND_SLOT, 1],
    };
    let pattern = ground_only(texture.into(), inks(1));
    let tile = render_tile(&pattern, &bank(), GROUND_DPI).expect("should render");
    check_against_golden("ground-texture.png", &tile);
}

#[test]
fn a_drawn_ground_matches_its_golden() {
    // Truchet rather than one of the other five, because it is the one made
    // entirely of curves: a change in how an edge is softened, or in how a
    // stroke width in millimetres becomes pixels, shows here first.
    let ornament = Ornament {
        figure: Figure::Truchet,
        cell_mm: 30.0,
        stroke_mm: 5.0,
        variation: 0.0,
        seed: 12,
        levels: vec![1],
        ..Ornament::default()
    };
    let pattern = ground_only(ornament.into(), inks(1));
    let tile = render_tile(&pattern, &bank(), GROUND_DPI).expect("should render");
    check_against_golden("ground-ornament.png", &tile);
}

#[test]
fn motifs_over_a_ground_match_their_golden() {
    // The composition, which none of the three above can show: the fabric is
    // filled, the ground layer is laid over it, and the motifs go on top. That
    // ordering is the design decision the whole ground model rests on — it is
    // what makes flowers on a check cost nothing — and it was pinned by no
    // picture.
    //
    // Built here rather than taken from the seamlessness fixture, and the
    // difference is the point of a golden. That fixture draws its ground in
    // slot 1, which in the motif palette is the cherries' own red, and strews
    // motifs thickly enough to cover most of it: every property still held and
    // the picture was useless to review. A golden is looked at, so it has to be
    // legible — a ground in a colour that is not a motif's, and enough of it
    // showing to see that the motifs are on top of it.
    let mut pattern = scattered_pattern(Lattice::block(TILE), 7, 0.12);
    let ink = pattern.palette.colors.len() as u8;
    pattern
        .palette
        .colors
        .push(pattern_core::Color::new(128, 141, 134));
    pattern.ground = Some(
        Ornament {
            figure: Figure::Truchet,
            cell_mm: 40.0,
            stroke_mm: 7.0,
            levels: vec![ink],
            ..Ornament::default()
        }
        .into(),
    );
    let tile = render_tile(&pattern, &bank(), GROUND_DPI).expect("should render");
    check_against_golden("ground-under-motifs.png", &tile);
}
