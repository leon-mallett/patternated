use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glam::Vec2;
use pattern_core::Pattern;
use pattern_render::{MotifBank, RasterMotif};
use serde::{Deserialize, Serialize};

/// A pattern plus the motif files it refers to.
///
/// The [`Pattern`] itself stays free of file paths — it is the portable
/// document, and a path is a fact about one machine. This wrapper is the app
/// layer's business and lives here with the rest of the I/O.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub motifs: Vec<MotifRef>,
    pub pattern: Pattern,
}

/// One motif file, and the physical size its pixels stand for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotifRef {
    pub id: String,
    /// Relative to the document, so a project folder can be moved or shared.
    pub file: PathBuf,
    pub natural_size_mm: Vec2,
}

impl Document {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading pattern document {}", path.display()))?;
        let doc: Self = serde_json::from_str(&text)
            .with_context(|| format!("parsing pattern document {}", path.display()))?;
        doc.pattern
            .validate()
            .with_context(|| format!("validating pattern in {}", path.display()))?;
        Ok(doc)
    }

    /// Decode every motif file this document names.
    pub fn load_motifs(&self, document_path: &Path) -> Result<MotifBank> {
        let base = document_path.parent().unwrap_or(Path::new("."));
        let mut bank = MotifBank::new();
        for m in &self.motifs {
            let path = base.join(&m.file);
            let pixmap = tiny_skia::Pixmap::load_png(&path)
                .with_context(|| format!("decoding motif '{}' from {}", m.id, path.display()))?;
            bank.insert(m.id.clone(), RasterMotif::new(pixmap, m.natural_size_mm));
        }
        Ok(bank)
    }
}
