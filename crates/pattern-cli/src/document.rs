use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use glam::Vec2;
use pattern_core::{scatter, MotifWeight, Pattern, ScatterParams};
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
    /// When present, placements are generated rather than authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scatter: Option<ScatterSpec>,
}

/// A scatter as a document describes it.
///
/// Distinct from `pattern_core::ScatterParams` because a document names motifs
/// by id and their physical sizes are already recorded once, on the motif
/// itself. Repeating a size here would let the two disagree, and the engine
/// would believe the wrong one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScatterSpec {
    pub density: f32,
    #[serde(default)]
    pub rotation_jitter_deg: f32,
    #[serde(default = "unit_range")]
    pub scale_range: (f32, f32),
    pub mix: Vec<MixEntry>,
    #[serde(default)]
    pub seed: u64,
}

fn unit_range() -> (f32, f32) {
    (1.0, 1.0)
}

/// One motif's share of a scatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixEntry {
    pub motif_id: String,
    pub weight: f32,
    #[serde(default)]
    pub palette_slot: u8,
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
        let mut doc: Self = serde_json::from_str(&text)
            .with_context(|| format!("parsing pattern document {}", path.display()))?;
        doc.apply_scatter()
            .with_context(|| format!("scattering motifs for {}", path.display()))?;
        doc.pattern
            .validate()
            .with_context(|| format!("validating pattern in {}", path.display()))?;
        Ok(doc)
    }

    /// Replace the pattern's placements with a generated scatter, if the
    /// document asks for one.
    ///
    /// Refuses rather than choosing when a document carries both authored
    /// placements and a scatter. Silently discarding hand-placed work would be
    /// the worse failure, and there is no sensible way to merge the two: a
    /// scatter's separation guarantee does not know about motifs it did not
    /// place.
    fn apply_scatter(&mut self) -> Result<()> {
        let Some(spec) = &self.scatter else {
            return Ok(());
        };
        if !self.pattern.placements.is_empty() {
            return Err(anyhow!(
                "document has {} authored placements and a scatter; keep one or the other",
                self.pattern.placements.len()
            ));
        }

        let mut mix = Vec::with_capacity(spec.mix.len());
        for entry in &spec.mix {
            let motif = self
                .motifs
                .iter()
                .find(|m| m.id == entry.motif_id)
                .ok_or_else(|| {
                    anyhow!(
                        "scatter mixes motif '{}', which the document does not define",
                        entry.motif_id
                    )
                })?;
            mix.push(MotifWeight {
                motif_id: entry.motif_id.clone(),
                weight: entry.weight,
                natural_size_mm: motif.natural_size_mm,
                palette_slot: entry.palette_slot,
            });
        }

        let params = ScatterParams {
            density: spec.density,
            rotation_jitter_deg: spec.rotation_jitter_deg,
            scale_range: spec.scale_range,
            motif_mix: mix,
            seed: spec.seed,
        };
        self.pattern.seed = spec.seed;
        self.pattern.placements =
            scatter(&params, self.pattern.tile_size_mm, &self.pattern.lattice);

        if self.pattern.placements.is_empty() {
            return Err(anyhow!(
                "scatter produced no placements; check density ({}) and motif sizes",
                spec.density
            ));
        }
        Ok(())
    }

    /// Re-run the scatter with a different seed.
    pub fn reseed(&mut self, seed: u64) -> Result<()> {
        if let Some(spec) = self.scatter.as_mut() {
            spec.seed = seed;
            self.pattern.placements.clear();
            self.apply_scatter()?;
            self.pattern.validate()?;
        }
        Ok(())
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
