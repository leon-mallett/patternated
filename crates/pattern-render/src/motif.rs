use std::collections::HashMap;

use glam::Vec2;
use tiny_skia::Pixmap;

/// A motif as M1 knows it: pixels plus the physical size those pixels stand for.
///
/// The physical size is the point. Without it a motif is just an image and
/// scale becomes guesswork at export, which is how a print comes back with one
/// enormous dinosaur on it.
///
/// M3 adds vector paths and slot-indexed artwork alongside this; the raster
/// stays, because watercolour and grain vectorise badly.
#[derive(Debug, Clone)]
pub struct RasterMotif {
    pub pixmap: Pixmap,
    pub natural_size_mm: Vec2,
}

impl RasterMotif {
    #[must_use]
    pub fn new(pixmap: Pixmap, natural_size_mm: Vec2) -> Self {
        Self {
            pixmap,
            natural_size_mm,
        }
    }
}

/// Motifs available to a render, looked up by the `motif_id` on a placement.
///
/// Holds decoded pixels, never paths: loading files is the CLI's job, which is
/// what keeps this crate free of I/O.
#[derive(Debug, Default, Clone)]
pub struct MotifBank {
    motifs: HashMap<String, RasterMotif>,
}

impl MotifBank {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: impl Into<String>, motif: RasterMotif) {
        self.motifs.insert(id.into(), motif);
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&RasterMotif> {
        self.motifs.get(id)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.motifs.is_empty()
    }
}
