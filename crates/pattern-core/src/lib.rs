//! Lattice, placement and tiling for repeat patterns.
//!
//! This crate is pure: a function from parameters to placements. It performs no
//! I/O, no async, no logging and reads no clock. Everything here is expressed in
//! millimetres — pixels appear only in the render and export layers.

mod error;
mod grid;
mod lattice;
mod motif;
mod palette;
mod pattern;
mod placement;
mod scatter;
mod torus;

pub use error::PatternError;
pub use grid::{grid, GridParams};
pub use lattice::{Lattice, NEIGHBOUR_COUNT};
pub use motif::{IndexedMotif, Motif};
pub use palette::{Color, Palette, GROUND_SLOT};
pub use pattern::{resolved_color_count, Pattern};
pub use placement::{wrap_into_tile, Placement};
pub use scatter::{scatter, MotifWeight, ScatterParams};
pub use torus::lattice_distance_sq;

/// Millimetres per inch. The one constant tying physical units to output DPI.
pub const MM_PER_INCH: f32 = 25.4;

/// Pixels per millimetre at a given output resolution.
///
/// The only sanctioned bridge between physical units and pixels. Render and
/// export layers call this; nothing in this crate does.
#[must_use]
pub fn px_per_mm(dpi: f32) -> f32 {
    dpi / MM_PER_INCH
}
