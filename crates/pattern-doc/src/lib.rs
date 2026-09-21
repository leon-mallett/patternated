//! The pattern document, and the file handling every front end needs.
//!
//! Shared by the command line and the app shell, which want exactly the same
//! things: read a document, resolve the motifs it names, scatter, render, write
//! a PNG. Keeping that in one place is what stops the two drifting into
//! disagreeing about what a document means.
//!
//! This is the app layer rather than a library in the usual sense — it opens
//! files, and it uses `anyhow`, because its errors are destined for a person
//! rather than for a caller that will match on them. If a front end ever needs
//! to branch on a failure, that is the point to give this crate typed errors.

pub mod document;

pub use document::{
    derive_palette_from, ingest_to_library, load_source, new_from_images, render_for_export,
    save_to, sheet_tiles, Colourway, Document, GridSpec, MotifRef, ScatterSpec,
};
