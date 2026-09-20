//! Show what matting decided, as an image.
//!
//! Not shipped surface — a development tool for looking at an alpha channel
//! instead of inferring it from a trimmed size. Run with:
//!   cargo run -p pattern-cli --example matte-debug -- <in.png> <out.png>

use motif_ingest::{matte, trim, MatteMethod, SourceImage};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (src, dst) = (&args[0], &args[1]);

    let pixmap = tiny_skia::Pixmap::load_png(src).expect("decode");
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let image = SourceImage::new(pixmap.width(), pixmap.height(), rgba).expect("shape");

    let matted = matte(&image, MatteMethod::default());
    let covered = matted.pixels.chunks(4).filter(|p| p[3] > 8).count();
    let partial = matted
        .pixels
        .chunks(4)
        .filter(|p| p[3] > 8 && p[3] < 248)
        .count();
    println!(
        "{}x{}  kept {:.1}% of pixels, {:.1}% partly covered",
        image.width,
        image.height,
        100.0 * covered as f32 / image.pixel_count() as f32,
        100.0 * partial as f32 / image.pixel_count() as f32
    );
    match trim(&matted, 8) {
        Some((c, b)) => println!("  trims to {}x{} at ({}, {})", c.width, c.height, b.x, b.y),
        None => println!("  trims to nothing"),
    }

    // Write the alpha channel as greyscale: white is kept, black is cut.
    let mut out = tiny_skia::Pixmap::new(image.width, image.height).expect("alloc");
    for (px, chunk) in out.pixels_mut().iter_mut().zip(matted.pixels.chunks(4)) {
        let a = chunk[3];
        *px = tiny_skia::PremultipliedColorU8::from_rgba(a, a, a, 255).expect("opaque grey");
    }
    out.save_png(dst).expect("write");
    println!("  alpha written to {dst}");
}
