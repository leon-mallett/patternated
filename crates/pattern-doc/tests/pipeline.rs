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

/// Cutting the same design for a different garment.
mod garment_scale {
    use std::path::PathBuf;

    fn artwork() -> Vec<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        vec![root.join("generated-motifs/01-cherries.png")]
    }

    fn document() -> pattern_doc::Document {
        pattern_doc::new_from_images(&artwork(), 60.0, 8).expect("document")
    }

    #[test]
    fn a_document_that_has_never_been_told_is_told_and_not_resized() {
        // There is no factor without two sizes. Guessing that an untold
        // document meant "adult" would silently resize somebody's work the
        // first time they opened the control.
        let mut d = document();
        let before = d.pattern.tile_size_mm;
        let factor = d.cut_for("T-shirt, adult M", 510.0).expect("should record");
        assert_eq!(factor, 1.0);
        assert_eq!(d.pattern.tile_size_mm, before);
        assert_eq!(d.drawn_for.as_ref().expect("recorded").across_mm, 510.0);
    }

    #[test]
    fn cutting_smaller_shrinks_the_repeat_and_the_motifs_together() {
        // The spec's requirement, and the failure it names: a size chart sets
        // a factor on both the tile and the motifs within it, or a small
        // garment ships with one enormous dinosaur and a great deal of
        // background.
        let mut d = document();
        d.cut_for("T-shirt, adult M", 510.0).expect("record");
        let tile = d.pattern.tile_size_mm;
        let motif = d.motifs[0].natural_size_mm.expect("a size");

        let factor = d.cut_for("T-shirt, age 3–4", 330.0).expect("cut");
        assert!((factor - 330.0 / 510.0).abs() < 1e-6);
        assert!((d.pattern.tile_size_mm - tile * factor).length() < 1e-3);
        assert!((d.motifs[0].natural_size_mm.expect("a size") - motif * factor).length() < 1e-3);
    }

    #[test]
    fn going_back_returns_the_design_rather_than_drifting() {
        // Why the reference is recorded rather than the factor accumulated.
        // A control that loses a little on every change is one a designer
        // learns not to touch.
        let mut d = document();
        d.cut_for("T-shirt, adult M", 510.0).expect("record");
        let tile = d.pattern.tile_size_mm;
        let motif = d.motifs[0].natural_size_mm.expect("a size");

        for (name, chest) in [
            ("T-shirt, age 3–4", 330.0),
            ("T-shirt, adult L", 560.0),
            ("T-shirt, age 9–10", 400.0),
            ("T-shirt, adult M", 510.0),
        ] {
            d.cut_for(name, chest).expect("cut");
        }
        assert!(
            (d.pattern.tile_size_mm - tile).length() < 0.01,
            "{:?}",
            d.pattern.tile_size_mm
        );
        assert!((d.motifs[0].natural_size_mm.expect("a size") - motif).length() < 0.01);
    }

    #[test]
    fn the_scaled_design_still_loads_and_renders() {
        // A scale control that can produce a pattern the engine refuses is a
        // scale control nobody can use.
        let mut d = document();
        d.cut_for("T-shirt, adult M", 510.0).expect("record");
        d.cut_for("T-shirt, age 3–4", 330.0).expect("cut");
        d.pattern.validate().expect("still a repeat");

        let dir = std::env::temp_dir().join("pattern-engine-garment-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let bank = d
            .load_motifs(&dir.join("pattern.json"))
            .expect("motifs should still load");
        assert!(!bank.is_empty());
    }

    #[test]
    fn a_garment_with_no_width_is_refused() {
        let mut d = document();
        assert!(d.cut_for("Nothing", 0.0).is_err());
        assert!(d.drawn_for.is_none(), "a refused cut records nothing");
    }
}

/// A drawing that cannot be outlines is drawn instead.
mod rendered_drawings {
    use std::path::PathBuf;

    fn gradient() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/drawings/gradient-mark.svg")
    }

    fn pear() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/drawings/pear.svg")
    }

    #[test]
    fn a_gradient_mark_starts_a_pattern_rather_than_refusing() {
        // The reported case. This is a real logo, one path, filled with a
        // linear gradient, which no palette slot can stand for, so reading it
        // as outlines leaves nothing at all. It is drawn and quantised, which
        // is exactly what exporting a PNG by hand would have achieved.
        let mut notes = Vec::new();
        let document = pattern_doc::new_from_images_noting(&[gradient()], 60.0, 10, &mut notes)
            .expect("a gradient mark should still make a pattern");

        assert_eq!(document.motifs.len(), 1);
        assert!(
            notes.iter().any(|n| n.says.contains("not sharpen")),
            "nobody was told what was traded: {notes:?}"
        );
    }

    #[test]
    fn the_drawn_mark_comes_out_as_a_ramp_and_not_one_flat_tone() {
        // What a gradient has to become on cloth. Flattened to a single colour
        // it would be a different logo; quantised, it is a ramp of flat tones,
        // which is what a screen print of a gradient is anyway.
        //
        // Not asserted as "both ends of the gradient survive": median cut
        // allocates by population and the extreme ends of a ramp are its least
        // populous parts, so the far end lands near rather than on. Measured,
        // the palette runs from (245, 115, 144) to (254, 133, 243), most of
        // the way along a gradient whose ends are (255, 136, 255) and
        // (242, 107, 103).
        let mut notes = Vec::new();
        let document = pattern_doc::new_from_images_noting(&[gradient()], 60.0, 10, &mut notes)
            .expect("document");

        // Everything but the fabric, which is slot 0 and supplied rather than
        // derived.
        let inks = &document.pattern.palette.colors[1..];
        assert!(inks.len() >= 4, "a ramp needs steps: {inks:?}");

        let spread = inks
            .iter()
            .flat_map(|a| {
                inks.iter()
                    .map(move |b| motif_ingest::colour_distance_sq(*a, *b))
            })
            .fold(0.0f32, f32::max);
        assert!(
            spread > 4000.0,
            "the mark came out as one tone in several shades ({spread:.0})"
        );
    }

    #[test]
    fn it_renders_at_print_resolution_from_the_document() {
        let mut notes = Vec::new();
        let mut document = pattern_doc::new_from_images_noting(&[gradient()], 60.0, 10, &mut notes)
            .expect("document");
        document.motifs[0].file = Some(PathBuf::from("gradient-mark.svg"));
        let bank = document
            .load_motifs(&gradient())
            .expect("the drawing should load from beside the document");
        assert!(!bank.is_empty());
        // Drawn, not kept as outlines, which is the trade the note describes.
        assert!(
            bank.vector(&document.motifs[0].id).is_none(),
            "a rendered drawing should be artwork, not outlines"
        );
    }

    /// A drawing that mostly reads as outlines but has one gradient in it.
    fn mixed() -> PathBuf {
        let dir = std::env::temp_dir().join("pattern-engine-mixed-drawing");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("mixed.svg");
        std::fs::write(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="40mm" viewBox="0 0 80 80">
                 <defs><linearGradient id="g"><stop stop-color="#c8202d"/>
                   <stop offset="1" stop-color="#3c7837"/></linearGradient></defs>
                 <rect x="0" y="0" width="80" height="40" fill="#1b2a4a"/>
                 <rect x="0" y="40" width="40" height="40" fill="#c8a020"/>
                 <rect x="40" y="40" width="40" height="40" fill="url(#g)"/>
               </svg>"##,
        )
        .expect("write");
        path
    }

    #[test]
    fn a_drawing_that_mostly_reads_as_outlines_is_still_drawn_if_a_shape_would_be_lost() {
        // The subtler half, and the more dangerous one. This reads as outlines
        // perfectly well, since two of its three shapes are flat, and the result is
        // a motif with a quarter of it simply missing. A motif that will not
        // sharpen is a worse picture; a motif missing a piece is the wrong
        // picture, and only one of those is obvious on screen.
        let mut notes = Vec::new();
        let mut document =
            pattern_doc::new_from_images_noting(&[mixed()], 40.0, 8, &mut notes).expect("document");
        assert!(
            notes.iter().any(|n| n.says.contains("not sharpen")),
            "it was kept as outlines with a shape missing: {notes:?}"
        );

        document.motifs[0].file = Some(PathBuf::from("mixed.svg"));
        let bank = document.load_motifs(&mixed()).expect("load");
        assert!(
            bank.vector(&document.motifs[0].id).is_none(),
            "kept as outlines, so the gradient square is not in the motif"
        );
    }

    #[test]
    fn a_drawing_that_needs_no_rendering_still_keeps_its_outlines() {
        // The fallback must not swallow the ordinary case. The pear is flat
        // shapes and a stroke, and stays vector.
        let mut notes = Vec::new();
        let mut document =
            pattern_doc::new_from_images_noting(&[pear()], 50.0, 8, &mut notes).expect("document");
        document.motifs[0].file = Some(PathBuf::from("pear.svg"));
        let bank = document.load_motifs(&pear()).expect("load");
        assert!(
            bank.vector("pear").is_some(),
            "a drawing that reads as outlines should stay outlines"
        );
        assert!(notes.is_empty(), "nothing to report: {notes:?}");
    }
}

#[test]
#[ignore = "writes a picture; run with --ignored"]
fn render_the_reported_mark() {
    use std::path::PathBuf;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = root.join("examples/drawings/gradient-mark.svg");
    let mut notes = Vec::new();
    let mut document =
        pattern_doc::new_from_images_noting(&[src], 42.0, 10, &mut notes).expect("document");
    document.pattern.tile_size_mm = glam::Vec2::new(180.0, 135.0);
    document.pattern.lattice = pattern_core::Lattice::half_drop(document.pattern.tile_size_mm);
    if let Some(s) = document.scatter.as_mut() {
        s.density = 0.20;
        s.seed = 4;
    }
    document.motifs[0].file = Some(PathBuf::from("gradient-mark.svg"));
    let out = root.join("examples/drawings/gradient-mark.json");
    let bank = document.load_motifs(&out).expect("load");
    document.reseed(4, &bank).expect("scatter");
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&document).expect("json") + "\n",
    )
    .expect("write");
    for n in &notes {
        eprintln!("NOTE: {}: {}", n.about, n.says);
    }
}
