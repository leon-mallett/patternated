use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};
use tiny_skia::Pixmap;

/// Metres per inch, for the PNG `pHYs` chunk, which is specified in pixels per
/// metre.
const METRES_PER_INCH: f32 = 0.0254;

/// Write a pixmap as a PNG carrying its physical resolution.
///
/// The resolution is the point of writing this by hand rather than using
/// `Pixmap::encode_png`, which emits no `pHYs` chunk. A pattern file with no
/// physical size in it is a file whose scale has to be re-established by
/// whoever opens it, and a print at the wrong scale looks fine on screen right
/// up until the fabric arrives.
pub fn write_png(path: &Path, pixmap: &Pixmap, dpi: f32) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating output {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let ppm = (dpi / METRES_PER_INCH).round() as u32;
    encoder.set_pixel_dims(Some(png::PixelDimensions {
        xppu: ppm,
        yppu: ppm,
        unit: png::Unit::Meter,
    }));

    // tiny-skia stores premultiplied alpha; PNG wants it straight.
    let mut data = Vec::with_capacity(pixmap.pixels().len() * 4);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }

    encoder
        .write_header()
        .context("writing PNG header")?
        .write_image_data(&data)
        .with_context(|| format!("writing PNG data to {}", path.display()))?;
    Ok(())
}
