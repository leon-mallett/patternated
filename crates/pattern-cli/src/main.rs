//! `pattern`: render a pattern document to PNG.
//!
//! The app layer: everything that touches a file or a command line lives here,
//! which is what lets `pattern-core` stay a pure function from parameters to
//! placements.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use motif_ingest::library::IngestParams;
use motif_ingest::MatteMethod;
use pattern_doc::{document, Document};
use pattern_io::{Compression, Export, Format, Shape};

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

        /// Compress a TIFF with LZW. Lossless, about a third the size, and
        /// slightly less universally readable than no compression at all.
        #[arg(long)]
        compress: bool,
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

    /// Derive a palette from artwork and print it as JSON.
    ///
    /// Paste the result into a document's palette, or use it to seed a
    /// library, rather than guessing colours by hand.
    DerivePalette {
        /// Source artwork (PNG). Give all the motifs a range will use.
        sources: Vec<PathBuf>,

        /// Total slots including the ground.
        #[arg(long, default_value_t = 5)]
        colours: usize,

        /// Ground colour as RRGGBB. Slot 0 is a design decision, not a
        /// property of the artwork, so it is supplied rather than derived.
        #[arg(long, default_value = "faf6ee")]
        ground: String,
    },

    /// Report what a document describes, without rendering it.
    Info {
        /// Pattern document (JSON).
        document: PathBuf,

        /// Resolution to report pixel dimensions for.
        #[arg(long, default_value_t = 150.0)]
        dpi: f32,
    },

    /// Print a garment's silhouette as SVG path data.
    ///
    /// For anything that wants the shape rather than a picture of it: a page
    /// that tiles the cloth itself and clips it to a garment, a cutting
    /// diagram, a plotter. The millimetres are the garment's own, origin at
    /// the top left of `extent_mm`.
    Garment {
        /// Garment (JSON): name, across_mm, length_mm and a cut. Omit it with
        /// --standard.
        garment: Option<PathBuf>,

        /// Print the standard size chart instead of one garment.
        #[arg(long)]
        standard: bool,
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
            compress,
        } => render(&document, &out, dpi, cols, rows, seed, compress),
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
        Command::DerivePalette {
            sources,
            colours,
            ground,
        } => derive_palette(&sources, colours, &ground),
        Command::Info { document, dpi } => info(&document, dpi),
        Command::Garment { garment, standard } => outline(garment.as_deref(), standard),
    }
}

/// Print one garment, or the whole size chart, with each silhouette as a path.
fn outline(path: Option<&Path>, standard: bool) -> Result<()> {
    let garments: Vec<pattern_render::Garment> = match (path, standard) {
        (Some(path), _) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            vec![serde_json::from_str(&text)
                .with_context(|| format!("{} is not a garment", path.display()))?]
        }
        (None, true) => pattern_render::Garment::standard(),
        (None, false) => anyhow::bail!("give a garment JSON file, or --standard"),
    };

    // The extent and the path are both derived, and a caller needs them
    // together: the extent is the viewBox the path is drawn in, and a path
    // without it cannot be placed. Serialised from the garment itself so the
    // measurements that produced the shape travel with it.
    let described = garments
        .iter()
        .map(|garment| {
            let mut value = serde_json::to_value(garment).context("serialising a garment")?;
            let extent = garment.extent_mm();
            let outline = garment
                .outline_svg_path()
                .ok_or_else(|| anyhow::anyhow!("'{}' has no silhouette", garment.name))?;
            let object = value
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("a garment serialises to an object"))?;
            object.insert("extent_mm".into(), serde_json::json!([extent.x, extent.y]));
            object.insert("outline_svg_path".into(), serde_json::json!(outline));
            Ok(value)
        })
        .collect::<Result<Vec<_>>>()?;

    // One garment asked for prints as one object, not as a list of one.
    let out = match described.as_slice() {
        [one] if path.is_some() => serde_json::to_string_pretty(one),
        many => serde_json::to_string_pretty(many),
    };
    println!("{}", out.context("serialising the outlines")?);
    Ok(())
}

fn render(
    doc_path: &Path,
    out: &Path,
    dpi: f32,
    cols: Option<u32>,
    rows: Option<u32>,
    seed: Option<u64>,
    compress: bool,
) -> Result<()> {
    let (mut doc, bank) = Document::load(doc_path)?;
    if let Some(seed) = seed {
        doc.reseed(seed, &bank).context("reseeding scatter")?;
    }

    let format = match out.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff") => Format::Tiff,
        _ => Format::Png,
    };
    let shape = match cols {
        Some(c) => Shape::Sheet {
            min_cols: c,
            min_rows: rows.unwrap_or(c),
        },
        None => Shape::Tile,
    };

    if let Shape::Sheet { min_cols, min_rows } = shape {
        // Said rather than silently corrected: a command line asked for three
        // columns should give three, and be told why it gave four.
        let grown = pattern_doc::sheet_tiles(&doc, min_cols, min_rows);
        if grown != (min_cols.max(1), min_rows.max(1)) {
            eprintln!(
                "note: a {}x{} sheet of this repeat would not tile, because its opposite edges \
                 would not meet. Writing {}x{}, the nearest that does.",
                min_cols.max(1),
                min_rows.max(1),
                grown.0,
                grown.1
            );
        }
    }

    let export = Export {
        format,
        compression: if compress {
            Compression::Lzw
        } else {
            Compression::None
        },
        dpi,
        shape,
    };

    let (pixmap, size) =
        pattern_doc::render_for_export(&doc, &bank, export).context("rendering for export")?;
    pattern_io::write(out, &pixmap, export).context("writing the export")?;

    let motifs: Vec<pattern_core::Motif> = bank.motifs().into_iter().cloned().collect();
    println!(
        "{}: {} x {} px, {:.0} x {:.0} mm at {dpi} DPI, {} motifs, {} colours",
        out.display(),
        size.width_px,
        size.height_px,
        size.width_mm,
        size.height_mm,
        doc.pattern.placements.len(),
        pattern_core::resolved_color_count(&doc.pattern, &motifs)
    );
    match size.width_verdict(pattern_io::STANDARD_ROLL_MM) {
        pattern_io::WidthVerdict::Fits => {}
        pattern_io::WidthVerdict::WiderThanStandardRoll => eprintln!(
            "note: {:.2} m wide, past a standard {:.1} m fabric roll. Wide-format printers \
             take this; most do not.",
            size.width_mm / 1000.0,
            pattern_io::STANDARD_ROLL_MM / 1000.0
        ),
        pattern_io::WidthVerdict::WiderThanGrandFormat => eprintln!(
            "warning: {:.2} m wide, past grand format. This is unlikely to be printable as \
             one piece. Export the tile instead and let the printer repeat it.",
            size.width_mm / 1000.0
        ),
    }
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
        Some(path) => Document::read_palette(path)?,
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

fn derive_palette(sources: &[PathBuf], colours: usize, ground: &str) -> Result<()> {
    let ground = parse_hex(ground)?;
    let palette = document::derive_palette_from(sources, ground, colours)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&palette).context("serialising palette")?
    );
    Ok(())
}

fn parse_hex(hex: &str) -> Result<pattern_core::Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        anyhow::bail!("ground must be six hex digits, got '{hex}'");
    }
    let byte = |i: usize| {
        u8::from_str_radix(&hex[i..i + 2], 16)
            .with_context(|| format!("'{hex}' is not a hex colour"))
    };
    Ok(pattern_core::Color::new(byte(0)?, byte(2)?, byte(4)?))
}

/// Colours this print resolves to, counting what the motifs actually use.
fn colour_count(doc: &Document, bank: &pattern_render::MotifBank) -> usize {
    let motifs: Vec<pattern_core::Motif> = bank.motifs().into_iter().cloned().collect();
    pattern_core::resolved_color_count(&doc.pattern, &motifs)
}

/// Report what a document describes, without rendering it.
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

    // The window a preview or a sheet actually needs, which is not always the
    // one asked for: a rectangle of tiles is not generally a repeating unit.
    let (cols, rows) = pattern_doc::sheet_tiles(&doc, 3, 3);
    println!("3x3 window  shown as {cols}x{rows} to make a repeating unit");

    println!(
        "placements  {}{}",
        p.placements.len(),
        if doc.scatter.is_some() {
            " (scattered)"
        } else if doc.grid.is_some() {
            " (grid)"
        } else {
            " (authored)"
        }
    );
    println!("motifs      {} resolved", bank.len());
    println!(
        "colours     {} resolved of {} in palette",
        colour_count(&doc, &bank),
        p.palette.len()
    );

    for dpi in [150.0f32, 300.0] {
        let size = pattern_io::PrintSize::new(p.tile_size_mm, dpi);
        println!(
            "tile at {:>3} {} x {} px  ({:.0} x {:.0} mm)",
            dpi as i32, size.width_px, size.height_px, size.width_mm, size.height_mm
        );
    }
    println!("seed        {}", p.seed);
    Ok(())
}
