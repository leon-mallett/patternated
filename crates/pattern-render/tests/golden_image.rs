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

use pattern_core::Lattice;
use pattern_render::render_field;
use support::{bank, scattered_pattern, straddling_pattern, TILE};

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
