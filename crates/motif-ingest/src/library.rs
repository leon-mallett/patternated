//! On-disk motif library — the I/O half of this crate, kept in its own module
//! so the boundary with the pure pipeline is visible rather than implied.
//!
//! **Motifs are immutable.** Re-ingesting artwork produces a new version rather
//! than changing the old one, so a pattern a designer approved months ago keeps
//! rendering exactly as they approved it. Nothing in here ever rewrites a
//! version that already exists.

use std::path::{Path, PathBuf};

use glam::Vec2;
use pattern_core::{IndexedMotif, Palette};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::matte::{matte, MatteMethod};
use crate::quantise::{quantise, QuantisationReport};
use crate::source::SourceImage;
use crate::trim::{natural_size_mm, trim, TrimBounds};
use crate::vectorise::{vectorise, TracedOutlines, VectoriseOptions};

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("motif '{id}' is entirely transparent after matting; nothing to ingest")]
    NothingToIngest { id: String },

    #[error("motif id '{id}' is not usable as a directory name")]
    UnusableId { id: String },

    #[error(transparent)]
    Pattern(#[from] pattern_core::PatternError),
}

/// How a motif should be brought in.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IngestParams {
    pub matte: MatteMethod,
    /// Coverage below which a pixel counts as background when trimming.
    pub trim_alpha_floor: u8,
    /// Physical size of the motif's longest edge, in millimetres.
    pub longest_edge_mm: f32,
    pub vectorise: VectoriseOptions,
}

impl Default for IngestParams {
    fn default() -> Self {
        Self {
            matte: MatteMethod::default(),
            trim_alpha_floor: 8,
            longest_edge_mm: 50.0,
            vectorise: VectoriseOptions::default(),
        }
    }
}

/// One immutable version of one motif.
#[derive(Debug, Clone, PartialEq)]
pub struct MotifVersion {
    pub id: String,
    pub version: u32,
    pub natural_size_mm: Vec2,
    /// Digest of the source artwork this was made from.
    pub source_digest: String,
    pub trimmed_from: TrimBounds,
    /// Share of the source's frame edge still covered after matting. High
    /// means background was kept — see [`crate::border_retained`].
    pub border_retained: f32,
    pub quantisation: QuantisationReport,
    pub indexed: IndexedMotif,
    pub vector: TracedOutlines,
}

/// A motif's metadata, as it sits in `motif.json`.
///
/// The indexed raster is deliberately *not* in here. Serialising 176,000 slot
/// and alpha values as a JSON array of numbers produced a 3.3 MB file for
/// 352 KB of data — nearly ten times the size, and worse for larger artwork,
/// where a single version of a 2000 x 2000 illustration would run to tens of
/// megabytes. Versions accumulate and are never deleted, so that compounds
/// into a repository nobody can clone.
///
/// The channels therefore live beside this file in `indexed.bin`, two bytes per
/// pixel, slot then alpha. Metadata stays JSON so a person can read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredMotif {
    id: String,
    version: u32,
    natural_size_mm: Vec2,
    source_digest: String,
    trimmed_from: TrimBounds,
    #[serde(default)]
    border_retained: f32,
    quantisation: QuantisationReport,
    width: u32,
    height: u32,
    vector: TracedOutlines,
}

/// Interleaved slot and alpha bytes, the companion to [`StoredMotif`].
const INDEXED_FILE: &str = "indexed.bin";
const METADATA_FILE: &str = "motif.json";

fn encode_channels(indexed: &IndexedMotif) -> Vec<u8> {
    let mut out = Vec::with_capacity(indexed.pixel_count() * 2);
    for (slot, alpha) in indexed.slots.iter().zip(&indexed.alpha) {
        out.push(*slot);
        out.push(*alpha);
    }
    out
}

fn decode_channels(width: u32, height: u32, bytes: &[u8]) -> Option<IndexedMotif> {
    if bytes.len() != width as usize * height as usize * 2 {
        return None;
    }
    let mut slots = Vec::with_capacity(bytes.len() / 2);
    let mut alpha = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        slots.push(pair[0]);
        alpha.push(pair[1]);
    }
    IndexedMotif::new(width, height, slots, alpha).ok()
}

/// The library index, as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Index {
    /// Slot 0 is the ground.
    palette: Palette,
    /// Motif id to the versions recorded for it, ascending.
    entries: Vec<IndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct IndexEntry {
    id: String,
    versions: Vec<u32>,
    /// Digest of the newest version's source, so a re-ingest of unchanged
    /// artwork can be recognised without reading the version back off disk.
    latest_digest: String,
}

/// A directory of motifs, sharing one palette.
///
/// **One palette for the whole library**, which every motif is quantised onto
/// at ingest. This is the decision that makes a colourway a single swap across
/// a whole range rather than a per-motif negotiation, and the spec asks for it
/// on the grounds that much of what reads as style mismatch is really colour
/// mismatch.
///
/// The cost is real and is reported rather than hidden: a motif whose colours
/// are not in the palette still quantises onto it, and
/// [`QuantisationReport::is_poor_fit`] is how anyone finds out.
#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    index: Index,
}

impl Library {
    /// Open a library, creating it with `palette` if the directory is empty.
    ///
    /// The palette is only applied on creation. An existing library keeps its
    /// own, because changing it would invalidate every slot index already
    /// stored — which is a migration, not an option on an open call.
    pub fn open(root: impl AsRef<Path>, palette: Palette) -> Result<Self, LibraryError> {
        let root = root.as_ref().to_path_buf();
        let index_path = root.join("library.json");

        if index_path.exists() {
            let text = std::fs::read_to_string(&index_path).map_err(|source| LibraryError::Io {
                path: index_path.clone(),
                source,
            })?;
            let index: Index =
                serde_json::from_str(&text).map_err(|source| LibraryError::Parse {
                    path: index_path,
                    source,
                })?;
            return Ok(Self { root, index });
        }

        std::fs::create_dir_all(&root).map_err(|source| LibraryError::Io {
            path: root.clone(),
            source,
        })?;
        let library = Self {
            root,
            index: Index {
                palette,
                entries: Vec::new(),
            },
        };
        library.write_index()?;
        Ok(library)
    }

    #[must_use]
    pub fn palette(&self) -> &Palette {
        &self.index.palette
    }

    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.index.entries.iter().map(|e| e.id.as_str()).collect()
    }

    /// Bring artwork in, producing a new immutable version.
    ///
    /// Re-ingesting byte-identical artwork under the same id is a no-op that
    /// returns the existing version rather than stacking up duplicates. That is
    /// a convenience, not a weakening of immutability: anything that differs at
    /// all, in pixels or in parameters, makes a new version.
    pub fn ingest(
        &mut self,
        id: &str,
        source: &SourceImage,
        params: IngestParams,
    ) -> Result<MotifVersion, LibraryError> {
        if id.is_empty() || id.contains(['/', '\\', '.']) {
            return Err(LibraryError::UnusableId { id: id.to_string() });
        }

        let digest = version_digest(source, params);
        if let Some(entry) = self.index.entries.iter().find(|e| e.id == id) {
            if entry.latest_digest == digest {
                if let Some(latest) = entry.versions.last() {
                    return self.read_version(id, *latest);
                }
            }
        }

        let matted = matte(source, params.matte);
        let border_kept = crate::matte::border_retained(&matted, params.trim_alpha_floor);
        let (cropped, bounds) = trim(&matted, params.trim_alpha_floor)
            .ok_or_else(|| LibraryError::NothingToIngest { id: id.to_string() })?;
        let (indexed, quantisation) = quantise(&cropped, &self.index.palette)?;
        let vector = vectorise(&indexed, params.vectorise);

        let version = self
            .index
            .entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.versions.last().copied())
            .unwrap_or(0)
            + 1;

        let motif = MotifVersion {
            id: id.to_string(),
            version,
            natural_size_mm: natural_size_mm(cropped.width, cropped.height, params.longest_edge_mm),
            source_digest: digest.clone(),
            trimmed_from: bounds,
            border_retained: border_kept,
            quantisation,
            indexed,
            vector,
        };

        self.write_version(&motif)?;

        match self.index.entries.iter_mut().find(|e| e.id == id) {
            Some(entry) => {
                entry.versions.push(version);
                entry.latest_digest = digest;
            }
            None => self.index.entries.push(IndexEntry {
                id: id.to_string(),
                versions: vec![version],
                latest_digest: digest,
            }),
        }
        self.write_index()?;
        Ok(motif)
    }

    /// The newest version of a motif.
    pub fn latest(&self, id: &str) -> Option<Result<MotifVersion, LibraryError>> {
        let entry = self.index.entries.iter().find(|e| e.id == id)?;
        let version = *entry.versions.last()?;
        Some(self.read_version(id, version))
    }

    /// Every version recorded for a motif, ascending.
    #[must_use]
    pub fn versions(&self, id: &str) -> Vec<u32> {
        self.index
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.versions.clone())
            .unwrap_or_default()
    }

    pub fn read_version(&self, id: &str, version: u32) -> Result<MotifVersion, LibraryError> {
        let dir = self.version_dir(id, version);
        let meta_path = dir.join(METADATA_FILE);
        let text = std::fs::read_to_string(&meta_path).map_err(|source| LibraryError::Io {
            path: meta_path.clone(),
            source,
        })?;
        let stored: StoredMotif =
            serde_json::from_str(&text).map_err(|source| LibraryError::Parse {
                path: meta_path,
                source,
            })?;

        let bin_path = dir.join(INDEXED_FILE);
        let bytes = std::fs::read(&bin_path).map_err(|source| LibraryError::Io {
            path: bin_path.clone(),
            source,
        })?;
        let indexed = decode_channels(stored.width, stored.height, &bytes).ok_or(
            pattern_core::PatternError::MotifChannelMismatch {
                width: stored.width,
                height: stored.height,
                slots: bytes.len() / 2,
                alpha: bytes.len() / 2,
            },
        )?;

        Ok(MotifVersion {
            id: stored.id,
            version: stored.version,
            natural_size_mm: stored.natural_size_mm,
            source_digest: stored.source_digest,
            trimmed_from: stored.trimmed_from,
            border_retained: stored.border_retained,
            quantisation: stored.quantisation,
            indexed,
            vector: stored.vector,
        })
    }

    fn write_version(&self, motif: &MotifVersion) -> Result<(), LibraryError> {
        let dir = self.version_dir(&motif.id, motif.version);
        std::fs::create_dir_all(&dir).map_err(|source| LibraryError::Io {
            path: dir.clone(),
            source,
        })?;

        let stored = StoredMotif {
            id: motif.id.clone(),
            version: motif.version,
            natural_size_mm: motif.natural_size_mm,
            source_digest: motif.source_digest.clone(),
            trimmed_from: motif.trimmed_from,
            border_retained: motif.border_retained,
            quantisation: motif.quantisation.clone(),
            width: motif.indexed.width,
            height: motif.indexed.height,
            vector: motif.vector.clone(),
        };
        let meta_path = dir.join(METADATA_FILE);
        let text = serde_json::to_string_pretty(&stored).map_err(|source| LibraryError::Parse {
            path: meta_path.clone(),
            source,
        })?;
        std::fs::write(&meta_path, text).map_err(|source| LibraryError::Io {
            path: meta_path,
            source,
        })?;

        let bin_path = dir.join(INDEXED_FILE);
        std::fs::write(&bin_path, encode_channels(&motif.indexed)).map_err(|source| {
            LibraryError::Io {
                path: bin_path,
                source,
            }
        })
    }

    fn version_dir(&self, id: &str, version: u32) -> PathBuf {
        self.root
            .join("motifs")
            .join(id)
            .join(format!("v{version}"))
    }

    fn write_index(&self) -> Result<(), LibraryError> {
        let path = self.root.join("library.json");
        let text =
            serde_json::to_string_pretty(&self.index).map_err(|source| LibraryError::Parse {
                path: path.clone(),
                source,
            })?;
        std::fs::write(&path, text).map_err(|source| LibraryError::Io { path, source })
    }
}

/// Digest covering the artwork *and* the parameters it was ingested with.
///
/// Both, because the same pixels matted at a different threshold are a
/// different motif, and a no-op re-ingest that ignored parameters would
/// silently discard the change somebody just asked for.
fn version_digest(source: &SourceImage, params: IngestParams) -> String {
    let pixels = source.digest();
    let settings = serde_json::to_string(&params).unwrap_or_default();
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x1000_0000_01b3;
    let mut hash = OFFSET;
    for byte in pixels.as_bytes().iter().chain(settings.as_bytes()) {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}
