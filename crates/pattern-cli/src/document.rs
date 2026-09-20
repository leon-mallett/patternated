use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use glam::Vec2;
use motif_ingest::library::{IngestParams, Library};
use motif_ingest::{matte, quantise, trim, MatteMethod, SourceImage};
use pattern_core::{scatter, Motif, MotifWeight, Palette, Pattern, ScatterParams};
use pattern_render::MotifBank;
use serde::{Deserialize, Serialize};

/// A pattern plus the motif files it refers to.
///
/// The [`Pattern`] itself stays free of file paths — it is the portable
/// document, and a path is a fact about one machine. This wrapper is the app
/// layer's business and lives here with the rest of the I/O.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub motifs: Vec<MotifRef>,
    /// Directory of ingested motifs. Motifs listed without a `file` are taken
    /// from here, at their newest version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<PathBuf>,
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

/// Where one motif comes from.
///
/// Two routes, and they are the same pipeline. A `file` is ingested on the spot
/// — matted, trimmed and quantised onto the pattern's own palette — which is
/// what lets a document be written by hand against loose artwork. Without one,
/// the motif is taken from the document's `library`, already ingested and
/// immutable, which is what a real range uses.
///
/// There is no third route where raw pixels reach the renderer unquantised.
/// One representation, one pipeline, so a motif behaves the same however it
/// arrived.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotifRef {
    pub id: String,
    /// Relative to the document, so a project folder can be moved or shared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Required with `file`; taken from the library otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natural_size_mm: Option<Vec2>,
    /// How to cut the subject out. Defaults to trusting the file's own alpha,
    /// which is right for exported artwork; scans of paper want a luminance
    /// threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matte: Option<MatteMethod>,
}

impl Document {
    /// Load a document and resolve every motif it names.
    ///
    /// Motifs are resolved before any scatter runs, because a scatter solves
    /// its separation radius from motif area and a library motif's physical
    /// size is recorded in the library rather than in the document. Doing it
    /// the other way round would make density mean something different for a
    /// library motif than for a loose file.
    pub fn load(path: &Path) -> Result<(Self, MotifBank)> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading pattern document {}", path.display()))?;
        let mut doc: Self = serde_json::from_str(&text)
            .with_context(|| format!("parsing pattern document {}", path.display()))?;

        let bank = doc
            .load_motifs(path)
            .with_context(|| format!("loading motifs for {}", path.display()))?;
        doc.apply_scatter(&bank)
            .with_context(|| format!("scattering motifs for {}", path.display()))?;
        doc.pattern
            .validate()
            .with_context(|| format!("validating pattern in {}", path.display()))?;
        Ok((doc, bank))
    }

    /// Physical size of a motif as this document and its library see it.
    pub fn motif_size(&self, bank: &MotifBank, id: &str) -> Option<Vec2> {
        bank.get(id).map(|m| m.natural_size_mm)
    }

    /// Replace the pattern's placements with a generated scatter, if the
    /// document asks for one.
    ///
    /// Refuses rather than choosing when a document carries both authored
    /// placements and a scatter. Silently discarding hand-placed work would be
    /// the worse failure, and there is no sensible way to merge the two: a
    /// scatter's separation guarantee does not know about motifs it did not
    /// place.
    fn apply_scatter(&mut self, bank: &MotifBank) -> Result<()> {
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
            let natural_size_mm = bank
                .get(&entry.motif_id)
                .map(|m| m.natural_size_mm)
                .ok_or_else(|| {
                    anyhow!(
                        "scatter mixes motif '{}', which the document does not define",
                        entry.motif_id
                    )
                })?;
            mix.push(MotifWeight {
                motif_id: entry.motif_id.clone(),
                weight: entry.weight,
                natural_size_mm,
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
    pub fn reseed(&mut self, seed: u64, bank: &MotifBank) -> Result<()> {
        if let Some(spec) = self.scatter.as_mut() {
            spec.seed = seed;
            self.pattern.placements.clear();
            self.apply_scatter(bank)?;
            self.pattern.validate()?;
        }
        Ok(())
    }

    /// Resolve every motif this document names into slot-indexed form.
    pub fn load_motifs(&self, document_path: &Path) -> Result<MotifBank> {
        let base = document_path.parent().unwrap_or(Path::new("."));
        let library = match &self.library {
            Some(path) => Some(
                Library::open(base.join(path), self.pattern.palette.clone())
                    .with_context(|| format!("opening library {}", path.display()))?,
            ),
            None => None,
        };

        let mut bank = MotifBank::new();
        for m in &self.motifs {
            let motif = match &m.file {
                Some(file) => {
                    let path = base.join(file);
                    let size = m.natural_size_mm.ok_or_else(|| {
                        anyhow!("motif '{}' gives a file but no natural_size_mm", m.id)
                    })?;
                    ingest_file(&path, &m.id, size, m.matte, &self.pattern.palette)?
                }
                None => {
                    let library = library.as_ref().ok_or_else(|| {
                        anyhow!(
                            "motif '{}' has no file and the document names no library",
                            m.id
                        )
                    })?;
                    let stored = library
                        .latest(&m.id)
                        .ok_or_else(|| anyhow!("library has no motif '{}'", m.id))?
                        .with_context(|| format!("reading motif '{}' from the library", m.id))?;
                    Motif {
                        id: m.id.clone(),
                        natural_size_mm: m.natural_size_mm.unwrap_or(stored.natural_size_mm),
                        indexed: stored.indexed,
                    }
                }
            };
            bank.insert(motif);
        }
        Ok(bank)
    }
}

/// Decode a PNG into straight RGBA, ready for the ingest pipeline.
///
/// `tiny-skia` stores premultiplied alpha; ingest reasons about colour
/// independently of coverage, so it is undone here rather than carried
/// through.
pub fn load_source(path: &Path) -> Result<SourceImage> {
    let pixmap = tiny_skia::Pixmap::load_png(path)
        .with_context(|| format!("decoding {}", path.display()))?;
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    SourceImage::new(pixmap.width(), pixmap.height(), rgba)
        .ok_or_else(|| anyhow!("decoded {} to an inconsistent buffer", path.display()))
}

/// Run one loose file through the pipeline, without storing it in a library.
fn ingest_file(
    path: &Path,
    id: &str,
    natural_size_mm: Vec2,
    method: Option<MatteMethod>,
    palette: &Palette,
) -> Result<Motif> {
    let source = load_source(path)?;
    let matted = matte(&source, method.unwrap_or(MatteMethod::ExistingAlpha));
    let (cropped, _) = trim(&matted, 8)
        .ok_or_else(|| anyhow!("motif '{id}' is entirely transparent after matting"))?;
    let (indexed, report) =
        quantise(&cropped, palette).with_context(|| format!("quantising motif '{id}'"))?;

    if report.is_poor_fit() {
        eprintln!(
            "warning: motif '{id}' fits this palette poorly (mean error {:.2}, worst {:.2}). \
             Its colours may not be represented.",
            report.mean_error, report.max_error
        );
    }
    Ok(Motif {
        id: id.to_string(),
        natural_size_mm,
        indexed,
    })
}

/// Ingest artwork into a library on disk.
pub fn ingest_to_library(
    library_root: &Path,
    palette: Palette,
    id: &str,
    source_path: &Path,
    params: IngestParams,
) -> Result<()> {
    let mut library = Library::open(library_root, palette)
        .with_context(|| format!("opening library {}", library_root.display()))?;
    let source = load_source(source_path)?;
    let stored = library
        .ingest(id, &source, params)
        .with_context(|| format!("ingesting '{id}'"))?;

    println!(
        "{} v{} — {} x {} px, {:.0} x {:.0} mm, {} slots, {} outlines",
        stored.id,
        stored.version,
        stored.indexed.width,
        stored.indexed.height,
        stored.natural_size_mm.x,
        stored.natural_size_mm.y,
        stored.quantisation.slots_used.len(),
        stored.vector.path_count()
    );
    if stored.quantisation.is_poor_fit() {
        println!(
            "  warning: fits the library palette poorly (mean {:.2}, worst {:.2})",
            stored.quantisation.mean_error, stored.quantisation.max_error
        );
    }
    Ok(())
}
