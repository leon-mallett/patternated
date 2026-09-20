//! The ingest pipeline end to end, on artwork drawn in code.
//!
//! Synthetic by choice while real Peel Club illustrations are being gathered.
//! The fixtures are built to exercise what real artwork will: a subject on
//! paper that is not pure white, a soft antialiased edge, a colour the palette
//! does not contain, and a speck too small to be worth an outline.

use motif_ingest::library::{IngestParams, Library};
use motif_ingest::{
    derive_palette, matte, natural_size_mm, quantise, trim, vectorise, MatteMethod, SourceImage,
    VectoriseOptions,
};
use pattern_core::{Color, Palette};

const PAPER: [u8; 4] = [248, 246, 242, 255];
const RED: [u8; 4] = [200, 30, 45, 255];
const GREEN: [u8; 4] = [60, 120, 55, 255];

fn house_palette() -> Palette {
    Palette::new(vec![
        Color::new(250, 246, 238),
        Color::new(200, 30, 45),
        Color::new(60, 120, 55),
    ])
}

/// A red disc with a green stem, on off-white paper, with a soft edge.
fn illustration_on_paper(size: u32) -> SourceImage {
    let mut img = SourceImage::filled(size, size, PAPER);
    let c = size as f32 / 2.0;
    let r = size as f32 * 0.32;
    for y in 0..size {
        for x in 0..size {
            let d = (((x as f32 + 0.5) - c).powi(2) + ((y as f32 + 0.5) - c * 1.2).powi(2)).sqrt();
            if d <= r {
                // Blend toward paper at the rim, the way ink or antialiasing
                // does, so matting has a real edge to find rather than a step.
                let t = ((r - d) / 2.0).clamp(0.0, 1.0);
                img.set_pixel(
                    x,
                    y,
                    [
                        (RED[0] as f32 * t + PAPER[0] as f32 * (1.0 - t)) as u8,
                        (RED[1] as f32 * t + PAPER[1] as f32 * (1.0 - t)) as u8,
                        (RED[2] as f32 * t + PAPER[2] as f32 * (1.0 - t)) as u8,
                        255,
                    ],
                );
            }
        }
    }
    for y in (size / 8)..(size / 2) {
        for x in (size / 2)..(size / 2 + size / 16).max(size / 2 + 1) {
            img.set_pixel(x, y, GREEN);
        }
    }
    img
}

#[test]
fn matting_removes_paper_and_keeps_ink() {
    let source = illustration_on_paper(64);
    let matted = matte(&source, MatteMethod::default());

    assert_eq!(matted.pixel(0, 0)[3], 0, "corner is paper");
    assert_eq!(matted.pixel(32, 38)[3], 255, "centre is ink");
}

#[test]
fn matting_leaves_a_soft_edge_rather_than_a_staircase() {
    // The reason matting feathers at all. Without partial coverage, a curve
    // prints with a visibly stepped outline.
    let matted = matte(&illustration_on_paper(64), MatteMethod::default());
    let partial = matted
        .pixels
        .chunks(4)
        .filter(|p| p[3] > 0 && p[3] < 255)
        .count();
    assert!(
        partial > 20,
        "expected a feathered rim, found {partial} pixels"
    );
}

#[test]
fn trimming_removes_the_paper_margin() {
    let matted = matte(&illustration_on_paper(64), MatteMethod::default());
    let (cropped, bounds) = trim(&matted, 8).expect("there is a subject");

    assert!(
        cropped.width < 64 && cropped.height < 64,
        "nothing was cropped"
    );
    assert!(bounds.x > 0, "subject does not start at the left edge");
}

#[test]
fn physical_size_is_taken_after_trimming_not_before() {
    // The property that keeps a motif's scale independent of how much paper it
    // was drawn on. The same subject on a larger sheet must come out the same
    // physical size.
    let small = matte(&illustration_on_paper(64), MatteMethod::default());
    let (small_crop, _) = trim(&small, 8).expect("subject");

    let mut big = SourceImage::filled(128, 128, PAPER);
    let src = illustration_on_paper(64);
    for y in 0..64 {
        for x in 0..64 {
            big.set_pixel(x + 32, y + 40, src.pixel(x, y));
        }
    }
    let (big_crop, _) = trim(&matte(&big, MatteMethod::default()), 8).expect("subject");

    let a = natural_size_mm(small_crop.width, small_crop.height, 50.0);
    let b = natural_size_mm(big_crop.width, big_crop.height, 50.0);
    assert!(
        (a.x - b.x).abs() < 0.5 && (a.y - b.y).abs() < 0.5,
        "{a:?} vs {b:?}"
    );
}

#[test]
fn quantising_flattens_colour_but_not_coverage() {
    // The single most important property of the indexed representation: a
    // motif can have four flat colours and still a soft outline.
    let matted = matte(&illustration_on_paper(64), MatteMethod::default());
    let (cropped, _) = trim(&matted, 8).expect("subject");
    let (indexed, _) = quantise(&cropped, &house_palette()).expect("should quantise");

    assert!(
        indexed.slots_used().len() <= 3,
        "expected flat colour, got {:?}",
        indexed.slots_used()
    );
    let partial = indexed
        .alpha
        .iter()
        .filter(|a| **a > 0 && **a < 255)
        .count();
    assert!(partial > 20, "coverage was flattened along with colour");
}

#[test]
fn a_colour_the_palette_lacks_is_reported_rather_than_hidden() {
    // Forced normalisation onto one shared palette is what makes colourways a
    // single swap, and it is also what quietly ruins a motif that does not
    // belong to the palette. This is the only warning anybody gets.
    let mut blue = SourceImage::filled(16, 16, [20, 60, 200, 255]);
    blue.set_pixel(0, 0, [20, 60, 200, 255]);

    let (_, report) = quantise(&blue, &house_palette()).expect("should quantise");
    assert!(
        report.is_poor_fit(),
        "a blue motif on a red-and-green palette should read as a poor fit, got mean {:.3}",
        report.mean_error
    );

    let red = SourceImage::filled(16, 16, RED);
    let (_, good) = quantise(&red, &house_palette()).expect("should quantise");
    assert!(!good.is_poor_fit(), "an exact match should not warn");
}

#[test]
fn transparent_pixels_do_not_count_toward_quantisation_error() {
    // Otherwise the score reports the background rather than the motif, and a
    // well-fitted subject on an unrelated paper colour reads as a bad fit.
    let mut img = SourceImage::filled(8, 8, [20, 60, 200, 0]);
    for x in 0..8 {
        img.set_pixel(x, 4, RED);
    }
    let (_, report) = quantise(&img, &house_palette()).expect("should quantise");
    assert!(
        report.mean_error < 0.02,
        "covered pixels match exactly, so error should be ~0, got {}",
        report.mean_error
    );
}

#[test]
fn a_derived_palette_finds_the_colours_actually_used() {
    let matted = matte(&illustration_on_paper(64), MatteMethod::default());
    let derived = derive_palette(&[&matted], Color::new(250, 246, 238), 4);

    assert_eq!(derived.len(), 4);
    assert_eq!(derived.ground(), Some(Color::new(250, 246, 238)));

    // Red and green are both present in the artwork, so both should survive.
    let has = |target: Color| {
        derived
            .colors
            .iter()
            .any(|c| motif_ingest::colour_distance_sq(*c, target) < 60_000.0)
    };
    assert!(
        has(Color::new(200, 30, 45)),
        "red missing from {:?}",
        derived.colors
    );
    assert!(
        has(Color::new(60, 120, 55)),
        "green missing from {:?}",
        derived.colors
    );
}

#[test]
fn deriving_a_palette_ignores_the_background() {
    // A scan is mostly paper. Sampling every pixel would hand back a palette of
    // near-whites and nothing else.
    let matted = matte(&illustration_on_paper(64), MatteMethod::default());
    let derived = derive_palette(&[&matted], Color::new(0, 0, 0), 3);
    for c in derived.colors.iter().skip(1) {
        assert!(
            motif_ingest::colour_distance_sq(*c, Color::new(248, 246, 242)) > 60_000.0,
            "paper leaked into the palette as {c:?}"
        );
    }
}

#[test]
fn palette_derivation_is_deterministic() {
    let matted = matte(&illustration_on_paper(64), MatteMethod::default());
    let a = derive_palette(&[&matted], Color::new(250, 246, 238), 5);
    let b = derive_palette(&[&matted], Color::new(250, 246, 238), 5);
    assert_eq!(a, b);
}

#[test]
fn vectorising_produces_outlines_tagged_by_slot_not_by_colour() {
    let matted = matte(&illustration_on_paper(96), MatteMethod::default());
    let (cropped, _) = trim(&matted, 8).expect("subject");
    let (indexed, _) = quantise(&cropped, &house_palette()).expect("should quantise");
    let vector = vectorise(&indexed, VectoriseOptions::default());

    assert!(vector.path_count() > 0, "nothing was traced");
    for path in &vector.paths {
        assert!(indexed.slots_used().contains(&path.slot));
        assert!(!path.d.is_empty());
    }
}

#[test]
fn a_vector_motif_recolours_with_the_palette() {
    // The reason paths carry slots rather than baked fills.
    let matted = matte(&illustration_on_paper(96), MatteMethod::default());
    let (cropped, _) = trim(&matted, 8).expect("subject");
    let (indexed, _) = quantise(&cropped, &house_palette()).expect("should quantise");
    let vector = vectorise(&indexed, VectoriseOptions::default());

    let house = vector.to_svg(&house_palette());
    let inverted = vector.to_svg(&Palette::new(vec![
        Color::new(20, 20, 30),
        Color::new(255, 190, 60),
        Color::new(120, 200, 255),
    ]));

    assert!(house.contains("#c81e2d"), "house red missing");
    assert!(inverted.contains("#ffbe3c"), "colourway red missing");
    assert!(
        !inverted.contains("#c81e2d"),
        "old colour survived the swap"
    );
}

#[test]
fn specks_are_dropped_rather_than_traced() {
    // Quantisation leaves single-pixel debris at colour boundaries. Traced,
    // each becomes a path, and the motif acquires hundreds of invisible
    // outlines that every downstream tool then carries.
    let matted = matte(&illustration_on_paper(96), MatteMethod::default());
    let (cropped, _) = trim(&matted, 8).expect("subject");
    let (indexed, _) = quantise(&cropped, &house_palette()).expect("should quantise");

    let kept_all = vectorise(
        &indexed,
        VectoriseOptions {
            min_area_px: 0,
            ..Default::default()
        },
    );
    let filtered = vectorise(&indexed, VectoriseOptions::default());
    assert!(
        filtered.path_count() <= kept_all.path_count(),
        "filtering should never add outlines"
    );
}

// --- library ---

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pattern-engine-test-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn ingesting_stores_a_motif_that_reads_back_identically() {
    let dir = scratch_dir("roundtrip");
    let mut library = Library::open(&dir, house_palette()).expect("should create");
    let stored = library
        .ingest(
            "cherry",
            &illustration_on_paper(64),
            IngestParams::default(),
        )
        .expect("should ingest");

    let read = library
        .latest("cherry")
        .expect("motif exists")
        .expect("should read");
    assert_eq!(stored, read);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn re_ingesting_changed_artwork_adds_a_version_and_keeps_the_old_one() {
    // Immutability, which is the whole point: a pattern approved months ago
    // must keep rendering as approved.
    let dir = scratch_dir("versions");
    let mut library = Library::open(&dir, house_palette()).expect("should create");

    let v1 = library
        .ingest(
            "cherry",
            &illustration_on_paper(64),
            IngestParams::default(),
        )
        .expect("should ingest");
    let v2 = library
        .ingest(
            "cherry",
            &illustration_on_paper(80),
            IngestParams::default(),
        )
        .expect("should ingest again");

    assert_eq!(v1.version, 1);
    assert_eq!(v2.version, 2);
    assert_eq!(library.versions("cherry"), vec![1, 2]);

    let reread = library.read_version("cherry", 1).expect("v1 still there");
    assert_eq!(reread, v1, "version 1 changed under us");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn re_ingesting_identical_artwork_does_not_stack_up_versions() {
    let dir = scratch_dir("noop");
    let mut library = Library::open(&dir, house_palette()).expect("should create");
    let art = illustration_on_paper(64);

    library
        .ingest("cherry", &art, IngestParams::default())
        .expect("first");
    library
        .ingest("cherry", &art, IngestParams::default())
        .expect("second");
    assert_eq!(library.versions("cherry"), vec![1]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changing_only_the_parameters_still_makes_a_new_version() {
    // The same pixels matted at a different threshold are a different motif.
    // A no-op check that ignored parameters would discard the change somebody
    // just asked for.
    let dir = scratch_dir("params");
    let mut library = Library::open(&dir, house_palette()).expect("should create");
    let art = illustration_on_paper(64);

    library
        .ingest("cherry", &art, IngestParams::default())
        .expect("first");
    library
        .ingest(
            "cherry",
            &art,
            IngestParams {
                longest_edge_mm: 90.0,
                ..IngestParams::default()
            },
        )
        .expect("second");
    assert_eq!(library.versions("cherry"), vec![1, 2]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopening_a_library_keeps_its_palette_rather_than_taking_a_new_one() {
    // Changing a library's palette would invalidate every slot index already
    // stored. That is a migration, not something an open call should do.
    let dir = scratch_dir("palette");
    {
        let mut library = Library::open(&dir, house_palette()).expect("create");
        library
            .ingest(
                "cherry",
                &illustration_on_paper(64),
                IngestParams::default(),
            )
            .expect("ingest");
    }
    let reopened = Library::open(&dir, Palette::new(vec![Color::new(0, 0, 0)])).expect("reopen");
    assert_eq!(reopened.palette(), &house_palette());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_blank_image_is_refused_rather_than_stored_as_an_empty_motif() {
    let dir = scratch_dir("blank");
    let mut library = Library::open(&dir, house_palette()).expect("create");
    let blank = SourceImage::filled(16, 16, [255, 255, 255, 255]);
    assert!(library
        .ingest("nothing", &blank, IngestParams::default())
        .is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_id_that_would_escape_the_library_directory_is_refused() {
    let dir = scratch_dir("ids");
    let mut library = Library::open(&dir, house_palette()).expect("create");
    let art = illustration_on_paper(32);
    for bad in ["../escape", "a/b", "", "with.dot"] {
        assert!(
            library.ingest(bad, &art, IngestParams::default()).is_err(),
            "id {bad:?} should be refused"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stored_motifs_do_not_bloat_on_disk() {
    // Serialising the indexed raster as JSON numbers cost 3.3 MB for 352 KB of
    // data. Versions are immutable and accumulate, so that compounds into a
    // repository nobody can clone. The channels live in a binary sidecar; this
    // fails if they migrate back into the metadata.
    let dir = scratch_dir("size");
    let mut library = Library::open(&dir, house_palette()).expect("create");
    let stored = library
        .ingest(
            "cherry",
            &illustration_on_paper(128),
            IngestParams::default(),
        )
        .expect("ingest");

    let version_dir = dir.join("motifs").join("cherry").join("v1");
    let metadata = std::fs::metadata(version_dir.join("motif.json")).expect("metadata exists");
    let raw = std::fs::metadata(version_dir.join("indexed.bin")).expect("channels exist");

    let pixels = stored.indexed.pixel_count() as u64;
    assert_eq!(
        raw.len(),
        pixels * 2,
        "channels should be two bytes a pixel"
    );
    assert!(
        metadata.len() < pixels,
        "metadata is {} bytes for {pixels} pixels; the raster has leaked back in",
        metadata.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_truncated_channel_file_is_refused_rather_than_misread() {
    // A short read would otherwise be interpreted as a smaller motif and
    // render as garbage.
    let dir = scratch_dir("truncated");
    let mut library = Library::open(&dir, house_palette()).expect("create");
    library
        .ingest(
            "cherry",
            &illustration_on_paper(64),
            IngestParams::default(),
        )
        .expect("ingest");

    let bin = dir
        .join("motifs")
        .join("cherry")
        .join("v1")
        .join("indexed.bin");
    let bytes = std::fs::read(&bin).expect("read");
    std::fs::write(&bin, &bytes[..bytes.len() - 4]).expect("truncate");

    assert!(library.read_version("cherry", 1).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_substantial_slot_gets_at_least_one_outline() {
    // The vector and the raster are two views of one motif and must not
    // disagree about what is in it. A slot covering a real area of the motif
    // with no path traced for it means that colour vanishes from any vector
    // export — invisible until somebody opens the SVG.
    let matted = matte(&illustration_on_paper(128), MatteMethod::default());
    let (cropped, _) = trim(&matted, 8).expect("subject");
    let (indexed, _) = quantise(&cropped, &house_palette()).expect("should quantise");
    let vector = vectorise(&indexed, VectoriseOptions::default());

    for slot in indexed.slots_used() {
        let area = indexed
            .slots
            .iter()
            .zip(&indexed.alpha)
            .filter(|(s, a)| **s == slot && **a >= 128)
            .count();
        if area < 64 {
            continue; // genuinely small, and filtered by design
        }
        assert!(
            vector.paths.iter().any(|p| p.slot == slot),
            "slot {slot} covers {area} px but was never traced"
        );
    }
}

#[test]
fn vector_and_raster_agree_on_the_motifs_extent() {
    // A traced outline that drifted from the pixels it came from would print
    // at the wrong size or in the wrong place.
    let matted = matte(&illustration_on_paper(128), MatteMethod::default());
    let (cropped, _) = trim(&matted, 8).expect("subject");
    let (indexed, _) = quantise(&cropped, &house_palette()).expect("should quantise");
    let vector = vectorise(&indexed, VectoriseOptions::default());

    assert_eq!(vector.width, indexed.width);
    assert_eq!(vector.height, indexed.height);

    let svg = vector.to_svg(&house_palette());
    assert!(
        svg.contains(&format!(
            "viewBox=\"0 0 {} {}\"",
            indexed.width, indexed.height
        )),
        "svg viewBox does not match the raster"
    );
}
