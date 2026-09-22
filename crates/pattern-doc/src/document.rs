use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use glam::Vec2;
use motif_ingest::library::{IngestParams, Library};
use motif_ingest::{matte, quantise, trim, MatteMethod, SourceImage};
use pattern_core::{scatter, Mirror, Motif, MotifWeight, Palette, Pattern, ScatterParams};

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
    /// The palette loose files are quantised against, if it differs from the
    /// one being rendered.
    ///
    /// The distinction is what makes a colourway work for a document that
    /// ingests artwork on the spot. Quantisation decides which *slot* each
    /// pixel belongs to and must therefore run against the palette the artwork
    /// was drawn for; rendering then resolves those slots through whatever
    /// palette this colourway uses. Quantising against the colourway instead
    /// asks which of an unrelated set of colours the artwork is nearest to,
    /// which is a different and meaningless question — a red cherry measured
    /// against apricot and mint.
    ///
    /// A library needs none of this: it holds one palette, motifs are
    /// quantised onto it once at ingest, and every colourway is a swap at
    /// render. This field is how a document without a library gets the same
    /// behaviour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_palette: Option<Palette>,
    pub pattern: Pattern,
    /// When present, placements are generated rather than authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scatter: Option<ScatterSpec>,
    /// Named alternative palettes for this same pattern.
    ///
    /// A colourway is the same print in different colours, and a range sells in
    /// several — so they belong in one document rather than in copies of it.
    /// Keeping them together is also what stops them drifting: a placement
    /// edited in one copy and not the others is a range that no longer
    /// coordinates.
    ///
    /// Every colourway must have the same number of slots as the palette the
    /// motifs were quantised against, because slot 3 has to mean the same part
    /// of the artwork in all of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colourways: Vec<Colourway>,
    /// Which colourway is currently rendered, by name. None means the
    /// pattern's own palette, which is the one motifs were quantised against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_colourway: Option<String>,
    /// Whether the placements were produced by the scatter or grid above,
    /// rather than written by hand.
    ///
    /// Both are stored, and they have to be told apart. A saved document keeps
    /// its placements so that the print reopens exactly as approved even if the
    /// sampler changes — a seed is a recipe, and a recipe is not a record. But
    /// a document carrying hand-placed motifs *and* a generator is genuinely
    /// ambiguous and is refused, because silently discarding either would be
    /// the worse failure.
    ///
    /// Without this flag the two cases are indistinguishable, and saving a
    /// scattered pattern produced a file that could not be reopened.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub placements_are_generated: bool,
    /// The other way of generating them: motifs on a regular grid.
    ///
    /// Mutually exclusive with `scatter`. A document carrying both is refused,
    /// on the same reasoning as one carrying a scatter and hand-placed motifs:
    /// there is no sensible merge and silently picking one would discard work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<GridSpec>,
    /// How far a grid had to shrink its motifs to stop them intersecting, as a
    /// fraction of the size asked for. Not part of the document; recomputed on
    /// every layout so a front end can say why a size control stopped moving.
    #[serde(skip, default = "one")]
    pub grid_scale_applied: f32,
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
    /// Keep motifs from intersecting. On unless a document says otherwise,
    /// including documents written before the option existed.
    #[serde(default = "prevent_overlap_default")]
    pub prevent_overlap: bool,
    /// Build the tile by reflecting a smaller designed cell into it.
    ///
    /// Absent in documents written before mirrors existed, which read as
    /// [`Mirror::None`] and are unaffected. A mirror needs a block lattice;
    /// combined with an oblique one the pattern is refused rather than
    /// rendered, because the reflected copies would not line up across it.
    #[serde(default)]
    pub mirror: Mirror,
    /// Share of motifs mounted on a mirror line, where the reflection
    /// symmetrises them instead of colliding with them. See
    /// [`ScatterParams::on_axis_share`](pattern_core::ScatterParams).
    #[serde(default)]
    pub on_axis_share: f32,
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

fn prevent_overlap_default() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

/// What changing the palette size did.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteResize {
    /// Colours the design is now quantised onto, ground included.
    pub colours: usize,
    /// Colourways discarded because they described the old slot count.
    pub colourways_dropped: Vec<String>,
}

/// One named alternative palette.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Colourway {
    pub name: String,
    pub palette: Palette,
}

/// A grid layout as a document describes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridSpec {
    pub rows: u32,
    pub cols: u32,
    /// Shrink motifs to fit their cell rather than letting them intersect.
    #[serde(default = "prevent_overlap_default")]
    pub prevent_overlap: bool,
    #[serde(default)]
    pub row_offset: f32,
    #[serde(default)]
    pub rotation_jitter_deg: f32,
    #[serde(default = "unit_range")]
    pub scale_range: (f32, f32),
    pub mix: Vec<MixEntry>,
    #[serde(default)]
    pub seed: u64,
}

/// One motif's share of a scatter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixEntry {
    pub motif_id: String,
    pub weight: f32,
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
    /// Read just the palette, without resolving any motif.
    ///
    /// Seeding a library from a document is otherwise circular: a full load
    /// resolves motifs, resolving motifs needs the library, and the library is
    /// what is being created. Reading only the palette breaks that, and the
    /// palette is all `--palette-from` ever wanted.
    pub fn read_palette(path: &Path) -> Result<Palette> {
        #[derive(Deserialize)]
        struct PaletteOnly {
            pattern: PatternPalette,
        }
        #[derive(Deserialize)]
        struct PatternPalette {
            palette: Palette,
        }

        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let parsed: PaletteOnly = serde_json::from_str(&text)
            .with_context(|| format!("reading a palette from {}", path.display()))?;
        Ok(parsed.pattern.palette)
    }

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

        // The design palette is settled before motifs resolve, because that is
        // what they are quantised against. Switching colourway afterwards
        // changes what the slots look like, never which slot a pixel is.
        doc.pin_design_palette();
        doc.apply_colourway()
            .with_context(|| format!("selecting a colourway in {}", path.display()))?;

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

    /// Remember the palette the motifs are quantised against.
    ///
    /// Without this, switching colourway on a document whose motifs come from
    /// loose files would re-quantise the artwork against the colourway — asking
    /// which of an unrelated set of colours a red cherry is nearest to, which
    /// is the mistake `design_palette` exists to prevent. Pinning it the first
    /// time a document is opened makes every later switch a swap rather than a
    /// re-ingest.
    pub fn pin_design_palette(&mut self) {
        if self.design_palette.is_none() && !self.colourways.is_empty() {
            self.design_palette = Some(self.pattern.palette.clone());
        }
    }

    /// Put the named colourway into the pattern, if one is selected.
    pub fn apply_colourway(&mut self) -> Result<()> {
        let Some(name) = self.active_colourway.clone() else {
            return Ok(());
        };
        let chosen = self
            .colourways
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| anyhow!("this document has no colourway called '{name}'"))?
            .palette
            .clone();

        let expected = self
            .design_palette
            .as_ref()
            .map(|p| p.len())
            .unwrap_or_else(|| self.pattern.palette.len());
        if chosen.len() != expected {
            return Err(anyhow!(
                "colourway '{name}' has {} colours and the motifs were quantised against {}; \
                 slots would not line up",
                chosen.len(),
                expected
            ));
        }
        self.pattern.palette = chosen;
        Ok(())
    }

    /// Store the pattern's current palette under a name.
    ///
    /// Replaces a colourway of the same name rather than adding a second, since
    /// two colourways called "Night" is not a state anybody wants.
    pub fn remember_colourway(&mut self, name: &str) {
        self.pin_design_palette();
        if self.design_palette.is_none() {
            self.design_palette = Some(self.pattern.palette.clone());
        }
        let palette = self.pattern.palette.clone();
        match self.colourways.iter_mut().find(|c| c.name == name) {
            Some(existing) => existing.palette = palette,
            None => self.colourways.push(Colourway {
                name: name.to_string(),
                palette,
            }),
        }
        self.active_colourway = Some(name.to_string());
    }

    /// Re-derive the design palette at a different number of colours.
    ///
    /// The count is a commercial decision as much as an aesthetic one — screen
    /// printing charges by the colour, and digital printing does not — so it
    /// belongs to the designer rather than being fixed at six because that was
    /// a reasonable guess when the app first needed a number.
    ///
    /// Every motif is re-quantised against the new palette, which is why this
    /// is a document operation rather than a palette edit: changing a slot's
    /// *colour* leaves the slots alone, and changing how many there are does
    /// not.
    ///
    /// Colourways describing the old palette cannot survive a change of slot
    /// count — they would be a list of colours for slots that no longer exist —
    /// so they are dropped and named in the result rather than left to fail
    /// later, which would be a document that no longer opens.
    pub fn set_palette_size(
        &mut self,
        colours: usize,
        document_path: &Path,
    ) -> Result<PaletteResize> {
        let base = document_path.parent().unwrap_or(Path::new("."));
        let sources: Vec<PathBuf> = self
            .motifs
            .iter()
            .filter_map(|m| m.file.as_ref().map(|f| base.join(f)))
            .collect();
        if sources.len() != self.motifs.len() {
            return Err(anyhow!(
                "this document takes motifs from a library, which holds one palette for the \
                 whole range; change the colour count where the library is built"
            ));
        }

        // The ground is a design decision, not a property of the artwork, so it
        // is carried across rather than re-derived. A designer who has set a
        // navy ground should not have it revert to off-white because they
        // asked for more colours.
        let ground = self
            .design_palette
            .as_ref()
            .unwrap_or(&self.pattern.palette)
            .ground()
            .ok_or_else(|| anyhow!("the palette has no ground to keep"))?;

        let colours = colours.clamp(2, 256);
        let palette = derive_palette_from(&sources, ground, colours)?;

        let dropped: Vec<String> = self
            .colourways
            .iter()
            .filter(|c| c.palette.len() != palette.len())
            .map(|c| c.name.clone())
            .collect();
        self.colourways.retain(|c| c.palette.len() == palette.len());
        if self
            .active_colourway
            .as_ref()
            .is_some_and(|name| !self.colourways.iter().any(|c| &c.name == name))
        {
            self.active_colourway = None;
        }

        self.pattern.palette = palette.clone();
        self.design_palette = if self.colourways.is_empty() {
            None
        } else {
            Some(palette.clone())
        };

        Ok(PaletteResize {
            colours: palette.len(),
            colourways_dropped: dropped,
        })
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
        if self.scatter.is_some() && self.grid.is_some() {
            return Err(anyhow!(
                "document has both a scatter and a grid; keep one or the other"
            ));
        }
        if self.scatter.is_none() && self.grid.is_none() {
            return Ok(());
        }
        if !self.pattern.placements.is_empty() {
            if self.placements_are_generated {
                // Written by this generator and saved. Keep them: the file is
                // the record of a print somebody approved, and regenerating
                // from the seed would only reproduce it as long as the sampler
                // never changes.
                return Ok(());
            }
            return Err(anyhow!(
                "document has {} hand-placed motifs and a generator; keep one or the other",
                self.pattern.placements.len()
            ));
        }
        if let Some(spec) = self.grid.clone() {
            return self.apply_grid(&spec, bank);
        }
        let Some(spec) = &self.scatter else {
            return Ok(());
        };

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
                radius_fraction: bank
                    .get(&entry.motif_id)
                    .map(|m| m.indexed.coverage_radius_fraction())
                    .unwrap_or(1.0),
            });
        }

        let params = ScatterParams {
            density: spec.density,
            rotation_jitter_deg: spec.rotation_jitter_deg,
            scale_range: spec.scale_range,
            motif_mix: mix,
            prevent_overlap: spec.prevent_overlap,
            mirror: spec.mirror,
            on_axis_share: spec.on_axis_share,
            seed: spec.seed,
        };
        self.pattern.seed = spec.seed;
        self.pattern.placements =
            scatter(&params, self.pattern.tile_size_mm, &self.pattern.lattice);
        self.placements_are_generated = true;

        if self.pattern.placements.is_empty() {
            if spec.mirror != Mirror::None {
                return Err(anyhow!(
                    "a {:?} mirror leaves no room for a motif this size: nothing can come \
                     within its own width of a mirror axis without being reflected into \
                     itself. Use a larger repeat or a smaller motif.",
                    spec.mirror
                ));
            }
            return Err(anyhow!(
                "scatter produced no placements; check density ({}) and motif sizes",
                spec.density
            ));
        }
        Ok(())
    }

    /// Lay motifs out on a grid rather than strewing them.
    fn apply_grid(&mut self, spec: &GridSpec, bank: &MotifBank) -> Result<()> {
        let mut mix = Vec::with_capacity(spec.mix.len());
        for entry in &spec.mix {
            let motif = bank.get(&entry.motif_id).ok_or_else(|| {
                anyhow!(
                    "grid uses motif '{}', which the document does not define",
                    entry.motif_id
                )
            })?;
            mix.push(MotifWeight {
                motif_id: entry.motif_id.clone(),
                weight: entry.weight,
                natural_size_mm: motif.natural_size_mm,
                radius_fraction: motif.indexed.coverage_radius_fraction(),
            });
        }
        mix.retain(|m| m.weight > 0.0);
        if mix.is_empty() {
            return Err(anyhow!("every motif is switched off"));
        }

        self.pattern.seed = spec.seed;
        let (placements, outcome) = pattern_core::grid_with_outcome(
            &pattern_core::GridParams {
                rows: spec.rows,
                cols: spec.cols,
                row_offset: spec.row_offset,
                rotation_jitter_deg: spec.rotation_jitter_deg,
                scale_range: spec.scale_range,
                motif_mix: mix,
                prevent_overlap: spec.prevent_overlap,
                seed: spec.seed,
            },
            self.pattern.tile_size_mm,
            &self.pattern.lattice,
        );
        self.pattern.placements = placements;
        self.placements_are_generated = true;
        self.grid_scale_applied = outcome.scale_applied;
        if self.pattern.placements.is_empty() {
            return Err(anyhow!(
                "that grid places no motifs; check rows and columns"
            ));
        }
        Ok(())
    }

    /// Run the scatter for a document assembled in memory rather than loaded.
    pub fn apply_scatter_public(&mut self, bank: &MotifBank) -> Result<()> {
        self.apply_scatter(bank)?;
        self.pattern.validate()?;
        Ok(())
    }

    /// Re-run the scatter with a different seed.
    pub fn reseed(&mut self, seed: u64, bank: &MotifBank) -> Result<()> {
        if let Some(spec) = self.scatter.as_mut() {
            spec.seed = seed;
        }
        if let Some(spec) = self.grid.as_mut() {
            spec.seed = seed;
        }
        if self.scatter.is_some() || self.grid.is_some() {
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
                    let design = self
                        .design_palette
                        .as_ref()
                        .unwrap_or(&self.pattern.palette);
                    if design.len() != self.pattern.palette.len() {
                        return Err(anyhow!(
                            "design_palette has {} colours and the render palette has {}; slots \
                             would not line up",
                            design.len(),
                            self.pattern.palette.len()
                        ));
                    }
                    ingest_file(&path, &m.id, size, m.matte, design)?
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
    warn_if_background_kept(id, &matted);
    let (cropped, _) = trim(&matted, 8)
        .ok_or_else(|| anyhow!("motif '{id}' is entirely transparent after matting"))?;
    let (indexed, report) =
        quantise(&cropped, palette).with_context(|| format!("quantising motif '{id}'"))?;

    if report.dissolves_into_ground() {
        eprintln!(
            "warning: {:.0}% of motif '{id}' became the ground colour and it will barely show. \
             The palette has no colour for what this mostly is.",
            report.ground_share * 100.0
        );
    } else if report.is_poor_fit() {
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
    if stored.border_retained > 0.15 {
        println!(
            "  warning: {:.0}% of the frame edge survived matting. Background has been kept as \
             motif — usually a hard shadow across the paper.",
            stored.border_retained * 100.0
        );
    }
    if stored.quantisation.dissolves_into_ground() {
        println!(
            "  WARNING: {:.0}% of this motif became the ground colour. It will be close to \
             invisible on the fabric — the palette has no colour for what this mostly is.",
            stored.quantisation.ground_share * 100.0
        );
    } else if stored.quantisation.is_poor_fit() {
        println!(
            "  warning: fits the library palette poorly (mean {:.2}, worst {:.2})",
            stored.quantisation.mean_error, stored.quantisation.max_error
        );
    }
    Ok(())
}

/// Derive a palette from artwork, rather than guessing one by hand.
///
/// The obvious first step with a new set of motifs, and it had no route from
/// the command line until real artwork made the omission obvious: every fruit
/// generated for testing was ingested onto a palette of cream, red and green
/// because that was the only palette to hand.
pub fn derive_palette_from(
    sources: &[PathBuf],
    ground: pattern_core::Color,
    count: usize,
) -> Result<Palette> {
    let images: Vec<SourceImage> = sources
        .iter()
        .map(|p| {
            let source = load_source(p)?;
            // Matte first: a palette derived from un-matted scans is a palette
            // of paper, since the background outnumbers the subject. The method
            // comes from the image, because artwork that already carries alpha
            // has no paper to measure.
            let method = motif_ingest::method_for(&source);
            Ok(motif_ingest::matte(&source, method))
        })
        .collect::<Result<Vec<_>>>()?;
    let refs: Vec<&SourceImage> = images.iter().collect();
    Ok(motif_ingest::derive_palette(&refs, ground, count))
}

/// Say so when matting has plainly kept background.
fn warn_if_background_kept(id: &str, matted: &SourceImage) {
    let retained = motif_ingest::border_retained(matted, 8);
    if retained > 0.15 {
        eprintln!(
            "warning: {:.0}% of the frame edge survived matting for '{id}'. Background has been \
             kept as motif — usually a hard shadow across the paper, which the illumination \
             estimate cannot model. Re-shoot with even light, or cut the subject out first and \
             ingest without --from-paper.",
            retained * 100.0
        );
    }
}

/// Build a document from loose image files, with nothing else to hand.
///
/// The starting point M4 was missing. The app could open a document and had no
/// way to make one, so the only route in was hand-writing JSON — which is
/// precisely what the milestone exists to remove.
///
/// Everything here is a default meant to be changed immediately: a 240 x 180 mm
/// half-drop scattered at 30%. The one thing that is *not* guessed is the
/// palette, which is derived from the artwork itself, because a guessed palette
/// is how a motif ends up quantised onto colours it does not contain.
pub fn new_from_images(
    files: &[PathBuf],
    longest_edge_mm: f32,
    colours: usize,
) -> Result<Document> {
    if files.is_empty() {
        return Err(anyhow!("no images were chosen"));
    }

    let mut sources = Vec::with_capacity(files.len());
    for path in files {
        sources.push((path.clone(), load_source(path)?));
    }

    let matted: Vec<SourceImage> = sources
        .iter()
        .map(|(_, image)| motif_ingest::matte(image, motif_ingest::method_for(image)))
        .collect();
    let refs: Vec<&SourceImage> = matted.iter().collect();

    // Slot 0 is the ground and is a design decision rather than a property of
    // the artwork, so it is supplied. A warm off-white reads as unprinted
    // fabric, which is what most ranges sit on.
    let ground = pattern_core::Color::new(250, 246, 238);
    let palette = motif_ingest::derive_palette(&refs, ground, colours.max(3));

    let mut motifs = Vec::with_capacity(files.len());
    let mut mix = Vec::with_capacity(files.len());
    for ((path, image), matted) in sources.iter().zip(&matted) {
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().replace(['.', '/', '\\'], "-"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("motif-{}", motifs.len() + 1));

        // Size from the trimmed subject, not the file, so a motif drawn small
        // on a big canvas is not scaled by its margins.
        let (trimmed, _) = motif_ingest::trim(matted, 8)
            .ok_or_else(|| anyhow!("'{}' is empty once its background is removed", id))?;
        let size = motif_ingest::natural_size_mm(trimmed.width, trimmed.height, longest_edge_mm);

        motifs.push(MotifRef {
            id: id.clone(),
            file: Some(path.clone()),
            natural_size_mm: Some(size),
            matte: Some(motif_ingest::method_for(image)),
        });
        mix.push(MixEntry {
            motif_id: id,
            weight: 1.0,
        });
    }

    let tile = Vec2::new(240.0, 180.0);
    Ok(Document {
        motifs,
        library: None,
        design_palette: None,
        pattern: Pattern {
            tile_size_mm: tile,
            lattice: pattern_core::Lattice::half_drop(tile),
            placements: Vec::new(),
            palette,
            seed: 1,
        },
        grid: None,
        placements_are_generated: false,
        colourways: Vec::new(),
        active_colourway: None,
        grid_scale_applied: 1.0,
        scatter: Some(ScatterSpec {
            density: 0.3,
            prevent_overlap: true,
            mirror: Mirror::None,
            on_axis_share: 0.0,
            rotation_jitter_deg: 20.0,
            scale_range: (0.85, 1.15),
            mix,
            seed: 1,
        }),
    })
}

/// Write a document, making motif paths relative to where it lands.
///
/// A document created from loose files holds absolute paths, which break the
/// moment the project folder is moved or shared. Rewriting them on save keeps
/// the promise the `file` field already makes.
pub fn save_to(document: &Document, path: &Path) -> Result<Document> {
    let base = path.parent().unwrap_or(Path::new("."));
    let mut out = document.clone();
    for motif in &mut out.motifs {
        if let Some(file) = &motif.file {
            if let Some(relative) = relative_to(file, base) {
                motif.file = Some(relative);
            }
        }
    }
    let text = serde_json::to_string_pretty(&out).context("serialising the document")?;
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(out)
}

/// A path expressed relative to `base`, where that is possible without `..`
/// climbing out of the filesystem.
fn relative_to(file: &Path, base: &Path) -> Option<PathBuf> {
    let file = file.canonicalize().ok()?;
    let base = base.canonicalize().ok()?;
    if let Ok(stripped) = file.strip_prefix(&base) {
        return Some(stripped.to_path_buf());
    }
    // One level up covers the common case of a document beside its images'
    // folder; beyond that an absolute path is more honest than a chain of
    // parent hops.
    let parent = base.parent()?;
    file.strip_prefix(parent)
        .ok()
        .map(|rest| Path::new("..").join(rest))
}

/// Render a pattern in the shape an export asks for.
///
/// The one place that knows a sheet must be a repeating unit. A pre-repeated
/// image that does not tile shows a seam the moment the printer repeats it,
/// which is the same failure as a broken tile arrived at one level up — and
/// harder to spot, because every tile inside it is perfect.
pub fn render_for_export(
    document: &Document,
    bank: &MotifBank,
    export: pattern_io::Export,
) -> Result<(tiny_skia::Pixmap, pattern_io::PrintSize)> {
    let pattern = &document.pattern;
    let (cols, rows) = match export.shape {
        pattern_io::Shape::Tile => (1, 1),
        pattern_io::Shape::Sheet { min_cols, min_rows } => {
            pattern
                .lattice
                .repeating_window(pattern.tile_size_mm, min_cols.max(1), min_rows.max(1))
        }
    };

    let pixmap = if cols == 1 && rows == 1 {
        pattern_render::render_tile(pattern, bank, export.dpi)
    } else {
        pattern_render::render_field(pattern, bank, export.dpi, cols, rows)
    }
    .context("rendering for export")?;

    // From the pixmap rather than from the design: rounding the tile to whole
    // pixels, and to a count its lattice divides, moves the repeat slightly,
    // and the number shown should be the one in the file.
    let size = pattern_io::PrintSize::from_pixels(pixmap.width(), pixmap.height(), export.dpi);
    Ok((pixmap, size))
}

/// The tiles a sheet will actually contain, which may exceed what was asked.
#[must_use]
pub fn sheet_tiles(document: &Document, min_cols: u32, min_rows: u32) -> (u32, u32) {
    document.pattern.lattice.repeating_window(
        document.pattern.tile_size_mm,
        min_cols.max(1),
        min_rows.max(1),
    )
}

/// Render an export and convert it to the colours a press will lay down.
///
/// Returns the samples and the profile to embed beside them. A CMYK file
/// without its profile is a set of numbers nobody can interpret.
pub fn render_cmyk(
    document: &Document,
    bank: &MotifBank,
    export: pattern_io::Export,
    rgb_profile: &Path,
    cmyk_profile: &Path,
) -> Result<(tiny_skia::Pixmap, pattern_io::PrintSize, Vec<u8>, Vec<u8>)> {
    let (pixmap, size) = render_for_export(document, bank, export)?;

    let source = pattern_io::Profiles::load(rgb_profile, pattern_io::Expect::Rgb)
        .context("loading the RGB profile")?;
    let destination = pattern_io::Profiles::load(cmyk_profile, pattern_io::Expect::Cmyk)
        .context("loading the CMYK profile")?;
    let conversion = pattern_io::Conversion::to_cmyk(&source, &destination, false)
        .context("building the colour transform")?;

    let samples = conversion
        .pixmap_to_cmyk(&pixmap)
        .context("converting to CMYK")?;
    let profile = std::fs::read(cmyk_profile)
        .with_context(|| format!("reading {}", cmyk_profile.display()))?;
    Ok((pixmap, size, samples, profile))
}

/// Show on screen what a press will do to a pattern.
pub fn soft_proof(
    pixmap: &tiny_skia::Pixmap,
    rgb_profile: &Path,
    cmyk_profile: &Path,
) -> Result<tiny_skia::Pixmap> {
    let source = pattern_io::Profiles::load(rgb_profile, pattern_io::Expect::Rgb)
        .context("loading the RGB profile")?;
    let destination = pattern_io::Profiles::load(cmyk_profile, pattern_io::Expect::Cmyk)
        .context("loading the CMYK profile")?;
    let conversion = pattern_io::Conversion::to_cmyk(&source, &destination, true)
        .context("building the colour transform")?;
    conversion.soft_proof(pixmap).context("soft proofing")
}
