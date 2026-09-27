/// A drawing goes through the same pipeline as artwork, and keeps its outlines.
mod drawings {
    use std::path::PathBuf;

    fn pear() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/drawings/pear.svg")
    }

    #[test]
    fn a_document_can_be_started_from_a_drawing() {
        // The whole point of the feature, end to end: choose an SVG, get a
        // pattern. Nothing about the rest of the pipeline knows it was one.
        let document = pattern_doc::new_from_images(&[pear()], 50.0, 8)
            .expect("a drawing should make a document");
        assert_eq!(document.motifs.len(), 1);
        assert_eq!(document.motifs[0].id, "pear");

        // Size comes from the drawing's own declared millimetres, scaled so
        // its longest edge is what was asked for.
        let size = document.motifs[0].natural_size_mm.expect("a size");
        assert!((size.y - 50.0).abs() < 0.01, "{size:?}");
        assert!(
            (size.x / size.y - 46.0 / 62.0).abs() < 0.01,
            "the drawing's proportions were not kept: {size:?}"
        );
    }

    #[test]
    fn the_palette_is_derived_from_the_colours_the_drawing_uses() {
        // A drawing's colours are exact, which is the one place it is a better
        // source for a palette than a picture would be. The pear's leaf green
        // is nowhere near its flesh, and both should survive.
        let document = pattern_doc::new_from_images(&[pear()], 50.0, 8).expect("document");
        let palette = &document.pattern.palette;
        let has = |want: pattern_core::Color| {
            palette.colors.iter().any(|c| {
                let d = motif_ingest::colour_distance_sq(*c, want);
                d < 900.0
            })
        };
        assert!(has(pattern_core::Color::new(201, 214, 74)), "the skin");
        assert!(has(pattern_core::Color::new(63, 125, 52)), "the leaf");
        assert!(has(pattern_core::Color::new(90, 61, 30)), "the stalk");
    }

    #[test]
    fn loading_it_puts_outlines_in_the_bank_rather_than_pixels() {
        // The claim that makes it worth reading an SVG at all rather than
        // rasterising it at import.
        let dir = std::env::temp_dir().join("pattern-engine-drawing-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("pattern.json");
        let mut document = pattern_doc::new_from_images(&[pear()], 50.0, 8).expect("document");
        // The document records an absolute path here, which is what
        // new_from_images produces for loose files.
        document.pattern.placements = vec![pattern_core::Placement::new(
            "pear",
            glam::Vec2::new(120.0, 90.0),
        )];

        let bank = document.load_motifs(&path).expect("should load");
        assert!(
            bank.vector("pear").is_some(),
            "the drawing was flattened to pixels on the way in"
        );
        let outlines = bank.vector("pear").expect("outlines");
        assert_eq!(outlines.shapes.len(), 4, "skin, flesh, stalk, leaf");
        assert!(!outlines.slots_used().is_empty());
    }
}

#[test]
#[ignore = "writes an example document; run with --ignored"]
fn write_the_drawing_example() {
    use std::path::PathBuf;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = root.join("examples/drawings/pear.svg");
    let mut document = pattern_doc::new_from_images(&[src], 46.0, 10).expect("document");
    document.pattern.tile_size_mm = glam::Vec2::new(200.0, 150.0);
    document.pattern.lattice = pattern_core::Lattice::half_drop(document.pattern.tile_size_mm);
    if let Some(s) = document.scatter.as_mut() {
        s.density = 0.22;
        s.seed = 7;
    }
    // Relative to where the document will live.
    document.motifs[0].file = Some(PathBuf::from("pear.svg"));
    let out = root.join("examples/drawings/pears.json");
    let bank = document
        .load_motifs(&out)
        .expect("the drawing should load from beside the document");
    document.reseed(7, &bank).expect("scatter");
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&document).expect("json") + "\n",
    )
    .expect("write");
    eprintln!("wrote {}", out.display());
}
