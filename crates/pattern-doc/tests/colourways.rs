//! Colourways as a document carries them.
//!
//! The property that has to hold everywhere: a colourway changes what the
//! slots look like, never which slot a pixel is. Everything here is a way of
//! checking that the two cannot come apart.

use std::path::{Path, PathBuf};

use pattern_core::{rotate_hue, swap_ground, Color, Palette};
use pattern_doc::Document;

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/peel-club-scatter/pattern.json")
}

fn open() -> (Document, pattern_render::MotifBank) {
    Document::load(&example()).expect("the example should open")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pattern-doc-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch");
    dir.join(name)
}

#[test]
fn a_colourway_can_be_remembered_and_come_back() {
    let (mut doc, _) = open();
    let original = doc.pattern.palette.clone();
    doc.remember_colourway("Day");

    doc.pattern.palette = rotate_hue(&original, 150.0);
    doc.remember_colourway("Rotated");

    doc.active_colourway = Some("Day".into());
    doc.apply_colourway().expect("should switch back");
    assert_eq!(doc.pattern.palette, original);

    doc.active_colourway = Some("Rotated".into());
    doc.apply_colourway().expect("should switch");
    assert_ne!(doc.pattern.palette, original);
}

#[test]
fn remembering_the_same_name_twice_replaces_rather_than_duplicates() {
    let (mut doc, _) = open();
    doc.remember_colourway("Night");
    doc.pattern.palette = swap_ground(&doc.pattern.palette, 1);
    doc.remember_colourway("Night");

    assert_eq!(doc.colourways.len(), 1, "two colourways share a name");
    assert_eq!(
        doc.colourways[0].palette, doc.pattern.palette,
        "the second remember did not take"
    );
}

#[test]
fn a_colourway_of_the_wrong_length_is_refused() {
    // Slot 3 has to mean the same part of the artwork in every colourway of a
    // range. A palette of a different length cannot promise that, and the
    // failure would be a print in the wrong colours rather than an error.
    let (mut doc, _) = open();
    doc.remember_colourway("Day");
    doc.colourways.push(pattern_doc::Colourway {
        name: "Short".into(),
        palette: Palette::new(vec![Color::new(0, 0, 0)]),
    });
    doc.active_colourway = Some("Short".into());
    let refused = doc.apply_colourway();
    assert!(refused.is_err(), "a short colourway was accepted");
    assert!(
        format!("{:?}", refused.unwrap_err()).contains("slots would not line up"),
        "the error should say why"
    );
}

#[test]
fn a_colourway_that_does_not_exist_is_refused_by_name() {
    let (mut doc, _) = open();
    doc.active_colourway = Some("Autumn".into());
    let refused = doc.apply_colourway();
    assert!(refused.is_err());
    assert!(format!("{:?}", refused.unwrap_err()).contains("Autumn"));
}

#[test]
fn switching_colourway_changes_colours_and_not_placements() {
    // The one property the whole mechanism rests on.
    let (mut doc, bank) = open();
    doc.remember_colourway("Day");
    let placements = doc.pattern.placements.clone();

    doc.pattern.palette = swap_ground(&doc.pattern.palette, 1);
    doc.remember_colourway("Inverted");
    doc.apply_colourway().expect("switch");

    assert_eq!(
        doc.pattern.placements, placements,
        "a colourway moved a motif"
    );
    // And it still renders, which is the point of keeping slot counts equal.
    pattern_render::render_tile(&doc.pattern, &bank, 48.0).expect("should still render");
}

#[test]
fn colourways_survive_being_saved_and_reopened() {
    let (mut doc, _) = open();
    doc.remember_colourway("Day");
    doc.pattern.palette = rotate_hue(&doc.pattern.palette, 90.0);
    doc.remember_colourway("Spring");

    let path = scratch("range.json");
    // Motif paths are rewritten relative to where it lands, so copy the motif
    // next to it first.
    let motif = example().parent().expect("dir").join("cherry.png");
    std::fs::copy(&motif, path.parent().expect("dir").join("cherry.png")).expect("copy");
    pattern_doc::save_to(&doc, &path).expect("should save");

    let (reopened, _) = Document::load(&path).expect("should reopen");
    let names: Vec<&str> = reopened
        .colourways
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, vec!["Day", "Spring"]);
    assert_eq!(reopened.active_colourway.as_deref(), Some("Spring"));
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn the_design_palette_is_pinned_so_a_switch_never_requantises() {
    // Without this, switching colourway on a document built from loose files
    // would re-quantise the artwork against the colourway — asking which of an
    // unrelated set of colours a red cherry is nearest to.
    let (mut doc, _) = open();
    assert!(doc.design_palette.is_none(), "nothing to pin yet");
    let original = doc.pattern.palette.clone();

    doc.remember_colourway("Day");
    assert_eq!(
        doc.design_palette.as_ref(),
        Some(&original),
        "the design palette should be pinned once colourways exist"
    );

    doc.pattern.palette = rotate_hue(&original, 180.0);
    doc.remember_colourway("Opposite");
    assert_eq!(
        doc.design_palette.as_ref(),
        Some(&original),
        "the design palette moved with the colourway"
    );
}

#[test]
fn a_saved_range_renders_the_same_geometry_in_every_colourway() {
    // What a coordinated range means: one print, several colours.
    let (mut doc, bank) = open();
    doc.remember_colourway("Day");
    doc.pattern.palette = rotate_hue(&doc.pattern.palette, 120.0);
    doc.remember_colourway("Summer");

    let mut sizes = Vec::new();
    for name in ["Day", "Summer"] {
        doc.active_colourway = Some(name.into());
        doc.apply_colourway().expect("switch");
        let tile = pattern_render::render_tile(&doc.pattern, &bank, 48.0).expect("render");
        sizes.push((tile.width(), tile.height()));
    }
    assert_eq!(sizes[0], sizes[1]);
}

fn _unused(_: &Path) {}

// --- saving and reopening ---

#[test]
fn a_scattered_pattern_survives_being_saved_and_reopened() {
    // This was broken. Saving writes the placements the scatter produced, so
    // that a print reopens exactly as approved even if the sampler changes —
    // and the loader refused that combination as ambiguous, which made every
    // saved scatter unopenable. A seed is a recipe; the file has to be a
    // record.
    let (doc, _) = open();
    assert!(!doc.pattern.placements.is_empty(), "the example scatters");

    let path = scratch("saved.json");
    let motif = example().parent().expect("dir").join("cherry.png");
    std::fs::copy(&motif, path.parent().expect("dir").join("cherry.png")).expect("copy");
    pattern_doc::save_to(&doc, &path).expect("should save");

    let (reopened, _) = Document::load(&path).expect("a saved scatter must reopen");
    assert_eq!(
        reopened.pattern.placements, doc.pattern.placements,
        "reopening moved the motifs"
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn hand_placed_motifs_beside_a_generator_are_still_refused() {
    // The case the old rule existed for, and it has to keep working: a
    // document somebody wrote by hand with both is genuinely ambiguous, and
    // discarding either would be the worse failure.
    let (mut doc, bank) = open();
    doc.placements_are_generated = false;
    let refused = doc.apply_scatter_public(&bank);
    assert!(refused.is_err(), "an ambiguous document was accepted");
    assert!(
        format!("{:?}", refused.unwrap_err()).contains("keep one or the other"),
        "the error should say what to do"
    );
}

#[test]
fn changing_a_setting_regenerates_rather_than_keeping_the_saved_print() {
    // The other half: a stored print is kept until somebody touches a control,
    // at which point they plainly want a new one.
    let (mut doc, bank) = open();
    let before = doc.pattern.placements.clone();
    doc.reseed(4321, &bank).expect("reseed");
    assert_ne!(
        doc.pattern.placements, before,
        "reseeding kept the old print"
    );
    assert!(doc.placements_are_generated);
}
