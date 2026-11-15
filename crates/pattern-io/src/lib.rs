//! Getting a pattern out of the engine and into a file somebody can print.
//!
//! Everything upstream works in millimetres. This is where a resolution is
//! finally chosen and pixels exist, which makes it also the place that knows
//! whether the thing being written can actually be printed; see [`PrintSize`].

mod colour;
mod naming;
mod png_out;
mod size;
mod tiff_out;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tiny_skia::Pixmap;

pub use colour::{ColourError, ColourSpace, Conversion, Expect, Profiles};
pub use naming::suggested_name;
pub use size::{PrintSize, WidthVerdict, GRAND_FORMAT_MM, STANDARD_ROLL_MM};

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("could not write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not encode {path}: {reason}")]
    Encode { path: PathBuf, reason: String },
}

/// File format for an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// Quick, widely readable, carries resolution as integer pixels per metre.
    Png,
    /// What a print bureau asks for. Carries resolution exactly.
    Tiff,
}

impl Format {
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Tiff => "tif",
        }
    }
}

/// How hard to work at making the file smaller.
///
/// Both are lossless; a print file has no business being anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Compression {
    /// No compression at all. The most compatible thing a bureau can receive,
    /// and large: a 300 DPI metre-square sheet is about 100 MB.
    #[default]
    None,
    /// Lossless and universally supported, usually a third of the size.
    Lzw,
}

/// What shape of file to produce.
///
/// The two things a printer might want, and they are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    /// One tile, for a printer that steps and repeats it themselves.
    Tile,
    /// A pre-repeated sheet, for a printer that does not.
    ///
    /// The tile count is a minimum: it is grown to a window that is itself a
    /// repeating unit, because a sheet that does not tile shows a seam the
    /// moment the printer repeats it, which is the same failure as a broken
    /// tile, arrived at one level up.
    Sheet { min_cols: u32, min_rows: u32 },
}

/// Everything that decides what lands on disk.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Export {
    pub format: Format,
    pub compression: Compression,
    pub dpi: f32,
    pub shape: Shape,
}

/// What to write, in what colours.
///
/// Separate from [`Export`] because a colour conversion needs loaded profiles,
/// which are files rather than settings.
pub struct Written<'a> {
    pub export: Export,
    /// Present for a CMYK export: the converted samples and the profile to
    /// embed alongside them.
    pub cmyk: Option<(&'a [u8], &'a [u8])>,
}

/// Write a rendered pattern to disk, in RGB.
pub fn write(path: &Path, pixmap: &Pixmap, export: Export) -> Result<(), ExportError> {
    write_written(path, pixmap, Written { export, cmyk: None })
}

/// Write a rendered pattern, in whatever colours it was prepared in.
pub fn write_written(
    path: &Path,
    pixmap: &Pixmap,
    written: Written<'_>,
) -> Result<(), ExportError> {
    let export = written.export;
    match (export.format, written.cmyk) {
        (Format::Png, _) => png_out::write_png(path, pixmap, export.dpi),
        (Format::Tiff, None) => tiff_out::write_tiff(path, pixmap, export.dpi, export.compression),
        (Format::Tiff, Some((samples, profile))) => tiff_out::write_tiff_cmyk(
            path,
            pixmap,
            export.dpi,
            export.compression,
            samples,
            profile,
        ),
    }
}
