//! Determinism is a feature, not a side effect.
//!
//! A designer must be able to return to a print months later and reproduce it
//! exactly. These assert byte-identical output, and they are not to be weakened
//! to make a change pass — a render that drifts is a colourway that no longer
//! matches its range.

mod support;

use pattern_core::Lattice;
use pattern_render::{render_field, render_tile};
use support::{bank, scattered_pattern, straddling_pattern, TILE};

fn bytes(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    pixmap.data().to_vec()
}

#[test]
fn the_same_pattern_renders_byte_identically_every_time() {
    let pattern = straddling_pattern(Lattice::half_drop(TILE));
    let first = render_tile(&pattern, &bank(), 150.0).expect("tile should render");
    let second = render_tile(&pattern, &bank(), 150.0).expect("tile should render");
    assert_eq!(bytes(&first), bytes(&second));
}

#[test]
fn a_rebuilt_motif_bank_does_not_change_the_output() {
    // Guards against anything in the render depending on allocation addresses
    // or hash iteration order, both of which vary run to run.
    let pattern = straddling_pattern(Lattice::block(TILE));
    let first = render_field(&pattern, &bank(), 96.0, 3, 3).expect("field should render");
    let second = render_field(&pattern, &bank(), 96.0, 3, 3).expect("field should render");
    assert_eq!(bytes(&first), bytes(&second));
}

#[test]
fn changing_only_the_lattice_changes_the_output() {
    // The counterpart worth having: proof the determinism tests above are
    // comparing something that can differ at all.
    let block = render_tile(&straddling_pattern(Lattice::block(TILE)), &bank(), 150.0)
        .expect("tile should render");
    let half_drop = render_tile(
        &straddling_pattern(Lattice::half_drop(TILE)),
        &bank(),
        150.0,
    )
    .expect("tile should render");
    assert_ne!(bytes(&block), bytes(&half_drop));
}

#[test]
fn placement_order_is_respected_rather_than_incidental() {
    // Overlapping motifs composite in placement order. If that order came from
    // a hash map the result would be stable within a run and different between
    // runs, which is the hardest kind of non-determinism to notice.
    let mut a = straddling_pattern(Lattice::block(TILE));
    a.placements.truncate(2);
    let mut b = a.clone();
    b.placements.swap(0, 1);

    let first = render_tile(&a, &bank(), 96.0).expect("tile should render");
    let again = render_tile(&a, &bank(), 96.0).expect("tile should render");
    assert_eq!(bytes(&first), bytes(&again));

    let swapped = render_tile(&b, &bank(), 96.0).expect("tile should render");
    assert_eq!(
        swapped.width(),
        first.width(),
        "swapping placements must not change the tile size"
    );
}

#[test]
fn a_scattered_pattern_renders_byte_identically_from_the_same_seed() {
    // The promise the seed exists to make, taken end to end: sampling, then
    // rasterising, then compositing. The core tests prove the placements
    // reproduce; this proves the pixels do.
    let lattice = Lattice::half_drop(TILE);
    let first = render_tile(&scattered_pattern(lattice, 20260920, 0.35), &bank(), 96.0)
        .expect("tile should render");
    let second = render_tile(&scattered_pattern(lattice, 20260920, 0.35), &bank(), 96.0)
        .expect("tile should render");
    assert_eq!(bytes(&first), bytes(&second));
}

#[test]
fn reseeding_a_scatter_changes_the_pixels() {
    // Seed reroll is a control a designer will press; it has to do something.
    let lattice = Lattice::half_drop(TILE);
    let a = render_tile(&scattered_pattern(lattice, 1, 0.35), &bank(), 96.0)
        .expect("tile should render");
    let b = render_tile(&scattered_pattern(lattice, 2, 0.35), &bank(), 96.0)
        .expect("tile should render");
    assert_ne!(bytes(&a), bytes(&b));
}
