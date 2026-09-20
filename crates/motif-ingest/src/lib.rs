//! Turning arbitrary source artwork into clean, recolourable motifs.
//!
//! Three entry points, one pipeline: a scanned illustration, a generated image
//! and a motif already in the library all take the same path, because a motif
//! is just pixels with an alpha channel and a palette.
//!
//! The crate takes decoded RGBA and returns data. It has no image-format
//! dependency at all — decoding a PNG or a TIFF is the caller's problem — which
//! keeps format support a decision made at the edge. The one exception is
//! [`library`], which is explicitly the I/O half and is kept in its own module
//! so the boundary is visible rather than implied.

mod matte;
mod quantise;
mod source;
mod trim;
mod vectorise;

pub mod library;

pub use matte::{border_retained, luminance, matte, MatteMethod};
pub use pattern_core::GROUND_SLOT;
pub use quantise::{colour_distance_sq, derive_palette, quantise, QuantisationReport};
pub use source::SourceImage;
pub use trim::{natural_size_mm, trim, TrimBounds};
pub use vectorise::{vectorise, VectoriseOptions};
