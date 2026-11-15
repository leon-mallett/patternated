//! Lattice, placement and tiling for repeat patterns.
//!
//! This crate is pure: a function from parameters to placements. It performs no
//! I/O, no async, no logging and reads no clock. Everything here is expressed in
//! millimetres; pixels appear only in the render and export layers.

mod bands;
mod colourway;
mod error;
mod grid;
mod ground;
mod headroom;
mod lattice;
mod mirror;
mod motif;
mod ornament;
mod palette;
mod pattern;
mod placement;
mod reseed;
mod scatter;
mod texture;
mod torus;
mod tracking;
mod vector;

pub use bands::{Band, Bands, Crossing};
pub use colourway::{rotate_hue, shift_lightness, swap_ground, with_colour};
pub use error::PatternError;
pub use grid::{grid, grid_with_outcome, GridOutcome, GridParams};
pub use ground::{Ground, LaidGround};
pub use headroom::{
    growth_headroom, growth_headroom_for_each, overlapping_pairs, scale_in_place, MOST_GROWTH,
};
pub use lattice::{Lattice, NEIGHBOUR_COUNT};
pub use mirror::{fold_into_cell, mirror_distance_sq, reflect, self_image_distance_sq, Mirror};
pub use motif::{IndexedMotif, Motif};
pub use ornament::{Figure, Grid, Ornament, OrnamentField, DEEPEST, FINEST_CELL_MM};
pub use palette::{Color, Palette, GROUND_SLOT};
pub use pattern::{resolved_color_count, Pattern};
pub use placement::{wrap_into_tile, Placement};
pub use reseed::{scatter_reseeding, ReseedLimits, Reseeded};
pub use scatter::{scatter, scatter_with_outcome, MotifWeight, ScatterOutcome, ScatterParams};
pub use texture::{Growth, SlotField, Texture, MOST_CELLS, MOST_STEPS};
pub use torus::lattice_distance_sq;
pub use tracking::{alignment, WORTH_A_LOOK};
pub use vector::{Unprintable, VectorMotif, VectorShape};

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
