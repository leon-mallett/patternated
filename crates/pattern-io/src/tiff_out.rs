use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use tiff::encoder::{colortype, Compression as TiffCompression, TiffEncoder};
use tiff::tags::ResolutionUnit;
use tiny_skia::Pixmap;

use crate::{Compression, ExportError};

/// Write a pixmap as a TIFF carrying its physical resolution.
///
/// Flat and single-layer — there is nothing else a pattern tile could be, and
/// it is what a bureau asks for.
///
/// **RGB rather than RGBA.** A print has no transparency: the ground is a
/// colour that gets printed like any other, and the renderer has already
/// composited onto it, so every alpha value is 255. Carrying the channel would
/// add a third to the file size and invite a bureau to wonder what it means.
///
/// TIFF records resolution as a rational, so the DPI survives exactly. PNG
/// stores integer pixels per metre, which turns 150 DPI into 150.0124 — a
/// rounding nobody will notice on fabric, and one worth not having.
pub fn write_tiff(
    path: &Path,
    pixmap: &Pixmap,
    dpi: f32,
    compression: Compression,
) -> Result<(), ExportError> {
    let file = File::create(path).map_err(|source| ExportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    // `with_compression` consumes and returns the encoder, so it has to be
    // bound before an image borrows from it.
    let mut encoder = TiffEncoder::new(BufWriter::new(file))
        .map_err(|e| ExportError::Encode {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?
        .with_compression(match compression {
            Compression::None => TiffCompression::Uncompressed,
            Compression::Lzw => TiffCompression::Lzw,
        });

    let mut rgb = Vec::with_capacity(pixmap.pixels().len() * 3);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgb.extend_from_slice(&[c.red(), c.green(), c.blue()]);
    }

    // A rational with a denominator of 1000 keeps a fractional DPI exact
    // without anybody having to reason about floating point in a file format.
    let resolution = tiff::encoder::Rational {
        n: (dpi * 1000.0).round() as u32,
        d: 1000,
    };

    let mut image = encoder
        .new_image::<colortype::RGB8>(pixmap.width(), pixmap.height())
        .map_err(|e| ExportError::Encode {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

    image.resolution(ResolutionUnit::Inch, resolution);
    image.write_data(&rgb).map_err(|e| ExportError::Encode {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}
