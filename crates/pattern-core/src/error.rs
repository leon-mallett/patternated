use thiserror::Error;

/// Ways a pattern can fail to describe something printable.
#[derive(Debug, Error, PartialEq)]
pub enum PatternError {
    #[error("tile size must be positive in both axes, got {width} x {height} mm")]
    NonPositiveTile { width: f32, height: f32 },

    #[error("lattice vectors are parallel, so the tile does not cover the plane")]
    DegenerateLattice,

    #[error("placement {index} ('{motif_id}') has non-positive scale {scale}")]
    NonPositiveScale {
        index: usize,
        motif_id: String,
        scale: f32,
    },

    #[error("palette is empty; a motif cannot be quantised onto no colours")]
    EmptyPalette,

    #[error(
        "motif declares {width} x {height} but carries {slots} slots and {alpha} alpha values"
    )]
    MotifChannelMismatch {
        width: u32,
        height: u32,
        slots: usize,
        alpha: usize,
    },

    #[error(
        "placement {index} ('{motif_id}') is reflected, which needs a block lattice; this tile \
         repeats obliquely, so the reflected copies would not line up across it"
    )]
    MirrorNeedsBlockLattice { index: usize, motif_id: String },

    #[error(
        "placement {index} ('{motif_id}') uses palette slot {slot}, but the palette has {len} \
         colours"
    )]
    SlotOutOfRange {
        index: usize,
        motif_id: String,
        slot: u8,
        len: usize,
    },
}
