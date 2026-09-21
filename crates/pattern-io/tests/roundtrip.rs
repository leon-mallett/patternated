//! What lands on disk, read back.
//!
//! An export is the last thing the engine does and the first thing somebody
//! else sees, so these check the file rather than the call that wrote it:
//! right pixels, right size, and — the part that silently ruins a print —
//! the right physical resolution.

use glam::Vec2;
use pattern_io::{write, Compression, Export, Format, PrintSize, Shape};
use tiny_skia::{Paint, Pixmap, Rect, Transform};

fn swatch(w: u32, h: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(w, h).expect("allocation");
    let mut ground = Paint::default();
    ground.set_color_rgba8(250, 246, 238, 255);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, 0.0, w as f32, h as f32).expect("rect"),
        &ground,
        Transform::identity(),
        None,
    );
    let mut ink = Paint::default();
    ink.set_color_rgba8(200, 30, 45, 255);
    pixmap.fill_rect(
        Rect::from_xywh(1.0, 1.0, (w / 2) as f32, (h / 2) as f32).expect("rect"),
        &ink,
        Transform::identity(),
        None,
    );
    pixmap
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pattern-io-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir.join(name)
}

#[test]
fn a_tiff_reads_back_with_the_pixels_it_was_given() {
    let pixmap = swatch(64, 48);
    let path = scratch("tile.tif");
    write(
        &path,
        &pixmap,
        Export {
            format: Format::Tiff,
            compression: Compression::None,
            dpi: 150.0,
            shape: Shape::Tile,
        },
    )
    .expect("should write");

    let file = std::fs::File::open(&path).expect("open");
    let mut decoder = tiff::decoder::Decoder::new(std::io::BufReader::new(file)).expect("decode");
    assert_eq!(decoder.dimensions().expect("dimensions"), (64, 48));

    let image = decoder.read_image().expect("read");
    let tiff::decoder::DecodingResult::U8(data) = image else {
        panic!("expected eight-bit samples");
    };
    // Three channels, not four: a print has no transparency and the ground is
    // a colour like any other.
    assert_eq!(data.len(), 64 * 48 * 3);
    // The swatch's ink starts one pixel in, so the very corner is ground and
    // the pixel inside it is not — which also checks the rows are the right
    // way up and the right width.
    assert_eq!(&data[0..3], &[250, 246, 238], "the corner should be ground");
    let inside = (64 + 1) * 3;
    assert_eq!(
        &data[inside..inside + 3],
        &[200, 30, 45],
        "one pixel in should be ink"
    );

    let far = ((47 * 64) + 63) * 3;
    assert_eq!(
        &data[far..far + 3],
        &[250, 246, 238],
        "the far corner should be ground"
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn a_tiff_records_its_resolution_exactly() {
    // The reason TIFF is the format a bureau asks for. PNG stores integer
    // pixels per metre, which turns 150 DPI into 150.0124; TIFF stores a
    // rational and keeps it.
    for dpi in [150.0f32, 300.0, 72.0] {
        let path = scratch("res.tif");
        write(
            &path,
            &swatch(8, 8),
            Export {
                format: Format::Tiff,
                compression: Compression::None,
                dpi,
                shape: Shape::Tile,
            },
        )
        .expect("should write");

        let file = std::fs::File::open(&path).expect("open");
        let mut decoder =
            tiff::decoder::Decoder::new(std::io::BufReader::new(file)).expect("decode");
        // A resolution tag is a rational, so it comes back as a numerator and
        // a denominator rather than as one number.
        let parts = decoder
            .get_tag(tiff::tags::Tag::XResolution)
            .expect("x resolution")
            .into_u32_vec()
            .expect("a rational pair");
        let x = parts[0] as f64 / parts[1] as f64;
        assert!(
            (x - dpi as f64).abs() < 1e-6,
            "wrote {dpi} DPI and read back {x} ({}/{})",
            parts[0],
            parts[1]
        );
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }
}

#[test]
fn compression_is_lossless_whichever_is_chosen() {
    // Both options exist to trade size against compatibility, never accuracy.
    let pixmap = swatch(32, 32);
    let mut read_back = Vec::new();
    for compression in [Compression::None, Compression::Lzw] {
        let path = scratch("c.tif");
        write(
            &path,
            &pixmap,
            Export {
                format: Format::Tiff,
                compression,
                dpi: 150.0,
                shape: Shape::Tile,
            },
        )
        .expect("should write");
        let file = std::fs::File::open(&path).expect("open");
        let mut decoder =
            tiff::decoder::Decoder::new(std::io::BufReader::new(file)).expect("decode");
        let tiff::decoder::DecodingResult::U8(data) = decoder.read_image().expect("read") else {
            panic!("expected eight-bit samples");
        };
        read_back.push(data);
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }
    assert_eq!(read_back[0], read_back[1], "LZW changed the pixels");
}

#[test]
fn a_png_still_carries_its_physical_size() {
    // Unchanged behaviour, checked because the writer moved crates.
    let path = scratch("tile.png");
    write(
        &path,
        &swatch(16, 16),
        Export {
            format: Format::Png,
            compression: Compression::None,
            dpi: 150.0,
            shape: Shape::Tile,
        },
    )
    .expect("should write");

    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&path).expect("open"),
    ));
    let reader = decoder.read_info().expect("info");
    let info = reader.info();
    let dims = info.pixel_dims.expect("a PNG export must carry pHYs");
    assert_eq!(dims.unit, png::Unit::Meter);
    // 150 DPI is 5905.51 pixels per metre, which an integer field cannot hold.
    assert_eq!(dims.xppu, 5906);
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
}

#[test]
fn the_print_size_matches_what_was_written() {
    // Ties the two halves together: the size a designer is shown and the file
    // that comes out have to agree, or the readout is decoration.
    let size = PrintSize::new(Vec2::new(240.0, 180.0), 150.0);
    let pixmap = swatch(size.width_px, size.height_px);
    let path = scratch("agree.tif");
    write(
        &path,
        &pixmap,
        Export {
            format: Format::Tiff,
            compression: Compression::Lzw,
            dpi: 150.0,
            shape: Shape::Tile,
        },
    )
    .expect("should write");

    let file = std::fs::File::open(&path).expect("open");
    let mut decoder = tiff::decoder::Decoder::new(std::io::BufReader::new(file)).expect("decode");
    assert_eq!(
        decoder.dimensions().expect("dimensions"),
        (size.width_px, size.height_px)
    );
    let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
}
