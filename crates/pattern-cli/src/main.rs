//! `pattern` — render a pattern document to PNG.
//!
//! The app layer: everything that touches a file or a command line lives here,
//! which is what lets `pattern-core` stay a pure function from parameters to
//! placements.

mod document;
mod png_out;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use document::Document;
use motif_ingest::library::IngestParams;
use motif_ingest::MatteMethod;

#[derive(Parser)]
#[command(
    name = "pattern",
    about = "Deterministic repeat patterns for textile printing"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a pattern document to PNG.
    Render {
        /// Pattern document (JSON).
        document: PathBuf,

        /// Output PNG.
        #[arg(short, long)]
        out: PathBuf,

        /// Output resolution. 72-96 for preview, 150-300 for print.
        #[arg(long, default_value_t = 150.0)]
        dpi: f32,

        /// Render a field of this many tiles across rather than a single tile.
        /// A single tile says nothing about tracking; 3 is the minimum useful
        /// view.
        #[arg(long)]
        cols: Option<u32>,

        /// Tiles down. Defaults to `--cols`.
        #[arg(long)]
        rows: Option<u32>,

        /// Re-run the document's scatter with this seed instead of its own.
        /// Ignored by a document with authored placements.
        #[arg(long)]
        seed: Option<u64>,
    },

    /// Bring artwork into a motif library.
    ///
    /// The library holds one palette and every motif is quantised onto it, so
    /// a colourway is later a single swap across the whole range.
    Ingest {
        /// Source artwork (PNG).
        source: PathBuf,

        /// Library directory. Created if it does not exist.
        #[arg(long)]
        library: PathBuf,

        /// Id to store the motif under.
        #[arg(long)]
        id: String,

        /// Physical size of the motif's longest edge, in millimetres.
        #[arg(long, default_value_t = 50.0)]
        size_mm: f32,

        /// Cut the subject from a light background by brightness, rather than
        /// trusting the file's own alpha. For scans of paper.
        #[arg(long)]
        from_paper: bool,

        /// A document whose palette seeds a new library. Ignored if the
        /// library already exists, since changing a palette would invalidate
        /// every slot already stored.
        #[arg(long)]
        palette_from: Option<PathBuf>,
    },

    /// Report what a document describes, without rendering it.
    Info {
        /// Pattern document (JSON).
        document: PathBuf,

        /// Resolution to report pixel dimensions for.
        #[arg(long, default_value_t = 150.0)]
        dpi: f32,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Render {
            document,
            out,
            dpi,
            cols,
            rows,
            seed,
        } => render(&document, &out, dpi, cols, rows, seed),
        Command::Ingest {
            source,
            library,
            id,
            size_mm,
            from_paper,
            palette_from,
        } => ingest(
            &source,
            &library,
            &id,
            size_mm,
            from_paper,
            palette_from.as_deref(),
        ),
        Command::Info { document, dpi } => info(&document, dpi),
    }
}

fn render(
    doc_path: &Path,
    out: &Path,
    dpi: f32,
    cols: Option<u32>,
    rows: Option<u32>,
    seed: Option<u64>,
) -> Result<()> {
    let (mut doc, bank) = Document::load(doc_path)?;
    if let Some(seed) = seed {
        doc.reseed(seed, &bank).context("reseeding scatter")?;
    }

    let pixmap = match cols {
        Some(c) => {
            let r = rows.unwrap_or(c);
            pattern_render::render_field(&doc.pattern, &bank, dpi, c.max(1), r.max(1))
                .context("rendering field")?
        }
        None => pattern_render::render_tile(&doc.pattern, &bank, dpi).context("rendering tile")?,
    };

    png_out::write_png(out, &pixmap, dpi)?;

    let tile = doc.pattern.tile_size_mm;
    println!(
        "{} — {} x {} px at {dpi} DPI, repeat {} x {} mm, {} motifs, {} colours",
        out.display(),
        pixmap.width(),
        pixmap.height(),
        tile.x,
        tile.y,
        doc.pattern.placements.len(),
        doc.pattern.resolved_color_count()
    );
    Ok(())
}

fn info(doc_path: &Path, dpi: f32) -> Result<()> {
    let (doc, bank) = Document::load(doc_path)?;
    let p = &doc.pattern;
    let pl = pattern_render::pixel_lattice_for(p, dpi).context("quantising lattice")?;

    println!("repeat      {} x {} mm", p.tile_size_mm.x, p.tile_size_mm.y);
    println!(
        "lattice     u = {:?}  v = {:?}  (mm)",
        p.lattice.u, p.lattice.v
    );
    println!(
        "at {dpi} DPI  tile {} x {} px, u = {:?}, v = {:?} (px)",
        pl.tile_px.x, pl.tile_px.y, pl.u_px, pl.v_px
    );
    println!(
        "placements  {}{}",
        p.placements.len(),
        if doc.scatter.is_some() {
            " (scattered)"
        } else {
            " (authored)"
        }
    );
    if let Some(spec) = &doc.scatter {
        let motif_area: f32 = spec
            .mix
            .iter()
            .filter_map(|e| doc.motif_size(&bank, &e.motif_id))
            .map(|size| size.x * size.y)
            .sum::<f32>()
            / spec.mix.len().max(1) as f32;
        let coverage =
            p.placements.len() as f32 * motif_area / (p.tile_size_mm.x * p.tile_size_mm.y);
        println!(
            "density     {:.0}% asked, {:.0}% achieved",
            spec.density * 100.0,
            coverage * 100.0
        );
        println!(
            "jitter      rotation +/-{:.0} deg, scale {:.2}-{:.2}",
            spec.rotation_jitter_deg, spec.scale_range.0, spec.scale_range.1
        );
    }
    println!("motifs      {} resolved", bank.len());
    println!(
        "colours     {} resolved of {} in palette",
        p.resolved_color_count(),
        p.palette.len()
    );
    println!("seed        {}", p.seed);
    Ok(())
}

fn ingest(
    source: &Path,
    library: &Path,
    id: &str,
    size_mm: f32,
    from_paper: bool,
    palette_from: Option<&Path>,
) -> Result<()> {
    let palette = match palette_from {
        Some(path) => {
            let (doc, _) = Document::load(path)
                .with_context(|| format!("reading a palette from {}", path.display()))?;
            doc.pattern.palette
        }
        None => pattern_core::Palette::new(vec![pattern_core::Color::new(255, 255, 255)]),
    };

    let params = IngestParams {
        matte: if from_paper {
            MatteMethod::default()
        } else {
            MatteMethod::ExistingAlpha
        },
        longest_edge_mm: size_mm,
        ..IngestParams::default()
    };
    document::ingest_to_library(library, palette, id, source, params)
}
