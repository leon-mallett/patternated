//! The app shell's Rust half: hold the open document, answer the UI.
//!
//! Every pattern decision stays in the engine crates. This layer owns exactly
//! two things the engine cannot: which document is open, and what the last
//! preview looked like. Anything more would be a second implementation of the
//! pattern model living behind a window.

mod base64;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use glam::Vec2;
use pattern_core::{Color, Lattice, Mirror};
use pattern_doc::Document;
use pattern_render::MotifBank;
use serde::{Deserialize, Serialize};
use tauri::State;

/// The open document, and the motifs it resolved to.
struct Session {
    path: PathBuf,
    document: Document,
    bank: MotifBank,
    /// False until the document has been written somewhere. A new pattern has
    /// a suggested path but no file, so Save must ask rather than silently
    /// creating one.
    saved: bool,
    /// The document as it stood when it was last written or opened, serialised.
    ///
    /// Compared rather than tracked with a flag. A flag has to be set at every
    /// site that changes the document, and the cost of missing one is telling
    /// a designer their work is safe when it is not — the one direction this
    /// must not be wrong in. Comparing cannot miss a site, because there is
    /// nothing to remember to do.
    written: String,
}

impl Session {
    /// Whether closing now would lose work.
    fn unsaved_changes(&self) -> bool {
        !self.saved
            || serde_json::to_string(&self.document)
                .map(|now| now != self.written)
                .unwrap_or(true)
    }

    /// Mark the document as matching what is on disk.
    fn mark_written(&mut self) {
        self.written = serde_json::to_string(&self.document).unwrap_or_default();
    }
}

#[derive(Default)]
struct App {
    session: Mutex<Option<Session>>,
}

/// Everything the UI needs to draw its controls for a freshly opened document.
#[derive(Serialize)]
struct Opened {
    path: String,
    name: String,
    tile_mm: [f32; 2],
    lattice: String,
    seed: u64,
    density: f32,
    rotation_jitter_deg: f32,
    scale_range: [f32; 2],
    motifs: Vec<MotifEntry>,
    palette: Vec<String>,
    /// Colours the design is quantised onto, ground included.
    palette_colours: usize,
    colourways: Vec<String>,
    active_colourway: Option<String>,
    scattered: bool,
    layout: String,
    on_axis_share: f32,
    prevent_overlap: bool,
    grid_rows: u32,
    grid_cols: u32,
}

/// What the UI shows for each motif, including how much of it has dissolved
/// into the ground.
fn motif_entries(bank: &MotifBank, scatter: Option<&pattern_doc::ScatterSpec>) -> Vec<MotifEntry> {
    bank.motifs()
        .into_iter()
        .map(|m| MotifEntry {
            id: m.id.clone(),
            natural_size_mm: [m.natural_size_mm.x, m.natural_size_mm.y],
            longest_edge_mm: m.natural_size_mm.x.max(m.natural_size_mm.y),
            weight: scatter
                .and_then(|s| s.mix.iter().find(|e| e.motif_id == m.id))
                .map(|e| e.weight)
                .unwrap_or(1.0),
            ground_share: m.indexed.ground_share(),
        })
        .collect()
}

#[derive(Serialize)]
struct MotifEntry {
    id: String,
    natural_size_mm: [f32; 2],
    /// Longest edge in millimetres, which is the handle a designer actually
    /// wants: aspect ratio comes from the artwork and is never adjusted.
    longest_edge_mm: f32,
    weight: f32,
    /// Share of this motif's ink that resolves to the ground colour, 0..1.
    ///
    /// Invisible on a plain preview and a hole the shape of the missing part
    /// the moment the ground changes. Almost always a palette with no colour
    /// for that part of the artwork.
    ground_share: f32,
}

/// What the UI sends back when anything is dragged.
#[derive(Deserialize)]
struct Params {
    tile_mm: [f32; 2],
    lattice: String,
    density: f32,
    rotation_jitter_deg: f32,
    scale_range: [f32; 2],
    seed: u64,
    /// "tossed" for a scatter, or a named grid preset.
    #[serde(default)]
    layout: String,
    /// Defaults to on when absent, so a caller that forgets the field gets the
    /// guard rather than an overlapping print.
    #[serde(default = "guard_on")]
    prevent_overlap: bool,
    /// Share of motifs mounted on a mirror line. Meaningless without a mirror.
    #[serde(default)]
    on_axis_share: f32,
    #[serde(default)]
    grid_rows: u32,
    #[serde(default)]
    grid_cols: u32,
    weights: Vec<(String, f32)>,
    /// Longest edge per motif, in millimetres.
    #[serde(default)]
    sizes: Vec<(String, f32)>,
    /// How wide the preview should come back, in screen pixels. The UI knows
    /// its own layout; the engine turns that into a resolution.
    preview_width_px: u32,
    /// Tiles across and down in the preview. Three is the minimum at which
    /// tracking corridors become visible.
    cols: u32,
    rows: u32,
}

/// A rendered preview plus the numbers a designer is deciding on.
#[derive(Serialize)]
struct Preview {
    png_base64: String,
    width: u32,
    height: u32,
    placements: usize,
    colours: usize,
    achieved_density: f32,
    dpi: f32,
    /// Tiles actually shown, which may exceed what was asked for.
    cols: u32,
    rows: u32,
    /// Whether the window shown is itself a repeating unit — that is, whether
    /// holding its opposite edges together is a fair test of the pattern.
    window_repeats: bool,
    /// Below 1 when a grid shrank its motifs to stop them intersecting.
    /// Surfaced so a size control that has stopped moving has a visible reason.
    size_limited_to: f32,
    /// Rows and columns a grid actually used. Larger than asked for when the
    /// offsets would not otherwise close over the tile, and surfaced for the
    /// same reason as the window size above: a count that silently differs
    /// from the one typed is a control that appears not to work.
    grid_cells: [u32; 2],
}

fn guard_on() -> bool {
    true
}

type Reply<T> = Result<T, String>;

/// Errors cross to the UI as sentences, because that is what they become.
fn fail(context: &str, e: impl std::fmt::Display) -> String {
    format!("{context}: {e}")
}

#[tauri::command]
fn open_document(app: State<'_, App>, path: String) -> Reply<Opened> {
    let path = PathBuf::from(path);
    let (document, bank) =
        Document::load(&path).map_err(|e| fail("could not open that document", e))?;

    let opened = describe(&path, &document, &bank);
    let mut session = Session {
        path,
        document,
        bank,
        saved: true,
        written: String::new(),
    };
    session.mark_written();
    *app.session.lock().map_err(|e| fail("state", e))? = Some(session);
    Ok(opened)
}

fn describe(path: &Path, document: &Document, bank: &MotifBank) -> Opened {
    let p = &document.pattern;
    let scatter = document.scatter.as_ref();
    Opened {
        path: path.display().to_string(),
        name: path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from("pattern")),
        tile_mm: [p.tile_size_mm.x, p.tile_size_mm.y],
        lattice: lattice_name(&p.lattice, p.tile_size_mm),
        seed: scatter.map(|s| s.seed).unwrap_or(p.seed),
        density: scatter.map(|s| s.density).unwrap_or(0.3),
        rotation_jitter_deg: scatter.map(|s| s.rotation_jitter_deg).unwrap_or(0.0),
        scale_range: scatter
            .map(|s| [s.scale_range.0, s.scale_range.1])
            .unwrap_or([1.0, 1.0]),
        motifs: motif_entries(bank, scatter),
        palette: p.palette.colors.iter().map(hex).collect(),
        palette_colours: p.palette.len(),
        colourways: document.colourways.iter().map(|c| c.name.clone()).collect(),
        active_colourway: document.active_colourway.clone(),
        scattered: scatter.is_some() || document.grid.is_some(),
        layout: document
            .grid
            .as_ref()
            .map(|g| preset_name(g.row_offset, g.col_offset))
            .or_else(|| scatter.map(|s| scatter_layout_name(s.mirror)))
            .unwrap_or_else(|| "tossed".into()),
        on_axis_share: scatter.map(|s| s.on_axis_share).unwrap_or(0.0),
        prevent_overlap: scatter
            .map(|s| s.prevent_overlap)
            .or_else(|| document.grid.as_ref().map(|g| g.prevent_overlap))
            .unwrap_or(true),
        grid_rows: document.grid.as_ref().map(|g| g.rows).unwrap_or(3),
        grid_cols: document.grid.as_ref().map(|g| g.cols).unwrap_or(3),
    }
}

/// Colours a new document is quantised onto, the ground included.
///
/// Twelve rather than the six this started with. Six was a guess, and it is
/// too few for artwork with any shading in it: a pale interior finds no colour
/// of its own and lands on the ground slot, which reads as a hole the moment
/// the ground is changed. The count is a control now, and this is only where
/// it starts.
const DEFAULT_COLOURS: usize = 12;

/// The classic repeats, as a designer names them.
///
/// Each is a row offset and a column offset, and nothing else.
///
/// They used to pair an offset with a *lattice* — a diagonal was a shifting
/// grid on a half-drop lattice — and that was wrong twice over. The drop
/// contributed nothing, because wherever it produced a regular field the
/// result was point for point what a block lattice gives; and where it did
/// not, it broke the pattern along every tile edge, which is how the defect
/// was found. A repeat's drop belongs to the cells, not to the tile.
///
/// It also left brick and half-drop indistinguishable except by that lattice,
/// since both were a row offset of a half. They are different patterns: a
/// brick shifts alternate rows sideways, a half-drop drops alternate columns.
const PRESETS: &[(&str, f32, f32)] = &[
    ("square", 0.0, 0.0),
    ("brick", 0.5, 0.0),
    ("half-drop", 0.0, 0.5),
    ("diagonal", 1.0 / 3.0, 0.0),
    ("gentle-diagonal", 0.25, 0.0),
];

/// The layouts that are a scatter rather than a grid, and the symmetry each
/// one gives it.
///
/// A mirror is not a grid preset and not a lattice: the lattice stays block,
/// the tile stays the size the designer asked for, and what changes is that
/// the scatter fills a half or a quarter of the tile and the rest is its
/// reflection. Returning `None` here means "this is one of the grid presets".
fn mirror_for(layout: &str) -> Option<Mirror> {
    match layout {
        "tossed" => Some(Mirror::None),
        "mirror" => Some(Mirror::SideToSide),
        "mirror-vertical" => Some(Mirror::TopToBottom),
        "mirror-four" => Some(Mirror::FourWay),
        _ => None,
    }
}

/// The name the UI uses for a scatter's symmetry, so a reopened document comes
/// back on the layout it was saved with.
fn scatter_layout_name(mirror: Mirror) -> String {
    match mirror {
        Mirror::None => "tossed",
        Mirror::SideToSide => "mirror",
        Mirror::TopToBottom => "mirror-vertical",
        Mirror::FourWay => "mirror-four",
    }
    .into()
}

fn preset_name(row_offset: f32, col_offset: f32) -> String {
    PRESETS
        .iter()
        .find(|(_, row, col)| (row - row_offset).abs() < 1e-3 && (col - col_offset).abs() < 1e-3)
        .map(|(name, _, _)| (*name).to_string())
        .unwrap_or_else(|| "square".into())
}

fn hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Which named repeat a lattice is, for the UI's benefit.
///
/// Compared against the constructors rather than stored, so the document stays
/// the single source of truth and a hand-edited lattice that is neither is
/// reported honestly rather than silently relabelled.
fn lattice_name(lattice: &Lattice, tile: Vec2) -> String {
    let close = |a: Lattice| (a.u - lattice.u).length() < 0.01 && (a.v - lattice.v).length() < 0.01;
    if close(Lattice::block(tile)) {
        "block".into()
    } else if close(Lattice::half_drop(tile)) {
        "half-drop".into()
    } else if close(Lattice::brick(tile)) {
        "brick".into()
    } else if close(Lattice::quarter_drop(tile)) {
        "quarter-drop".into()
    } else {
        "custom".into()
    }
}

/// What a request to close did, or why it did not.
#[derive(Serialize)]
struct CloseOutcome {
    /// False when the close was refused because work would be lost.
    closed: bool,
    /// The pattern has never been written to a file, so closing discards it.
    never_saved: bool,
    /// The pattern has changed since it was last written.
    changed: bool,
}

/// Put the app back to its empty state.
///
/// There was no way out of a document once one was open: the start screen
/// offering New and Open could only be reached by restarting the app, so
/// trying a pattern and then wanting a different one meant quitting.
///
/// Refuses rather than asking, and leaves the asking to the interface. A
/// backend that puts up its own dialog cannot be tested and cannot be driven
/// by anything but a person — and the refusal carries the two facts the
/// question needs, so the interface does not have to work them out or guess.
#[tauri::command]
fn close_document(app: State<'_, App>, discard: bool) -> Reply<CloseOutcome> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_ref().ok_or("no document is open")?;

    let never_saved = !session.saved;
    let changed = session.unsaved_changes();
    if changed && !discard {
        return Ok(CloseOutcome {
            closed: false,
            never_saved,
            changed,
        });
    }

    *guard = None;
    Ok(CloseOutcome {
        closed: true,
        never_saved: false,
        changed: false,
    })
}

/// Start a pattern from image files, with no document in existence.
///
/// The route M4 was missing. Until this existed the only way into the app was
/// a hand-written JSON file, which is the thing the milestone is supposed to
/// remove.
#[tauri::command]
fn new_from_images(app: State<'_, App>, files: Vec<String>, longest_edge_mm: f32) -> Reply<Opened> {
    let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let document = pattern_doc::new_from_images(&paths, longest_edge_mm.max(2.0), DEFAULT_COLOURS)
        .map_err(|e| fail("could not start a pattern from those images", e))?;

    // An unsaved document has no path of its own yet; the first save asks.
    let anchor = paths
        .first()
        .and_then(|p| p.parent())
        .map(|p| p.join("untitled.json"))
        .unwrap_or_else(|| PathBuf::from("untitled.json"));

    let mut session = Session {
        path: anchor.clone(),
        document,
        bank: MotifBank::new(),
        saved: false,
        written: String::new(),
    };
    session.bank = session
        .document
        .load_motifs(&anchor)
        .map_err(|e| fail("could not read those images", e))?;
    session
        .document
        .apply_scatter_public(&session.bank)
        .map_err(|e| fail("could not scatter those motifs", e))?;

    let opened = describe(&anchor, &session.document, &session.bank);
    *app.session.lock().map_err(|e| fail("state", e))? = Some(session);
    Ok(opened)
}

#[tauri::command]
fn apply(app: State<'_, App>, params: Params) -> Reply<Preview> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;

    write_params(session, &params).map_err(|e| fail("could not apply those settings", e))?;
    render(session, &params).map_err(|e| fail("could not render a preview", e))
}

/// Push the UI's settings into the document, then re-scatter.
fn write_params(session: &mut Session, params: &Params) -> anyhow::Result<()> {
    let tile = Vec2::new(params.tile_mm[0].max(1.0), params.tile_mm[1].max(1.0));
    session.document.pattern.tile_size_mm = tile;
    session.document.pattern.lattice = match params.lattice.as_str() {
        "block" => Lattice::block(tile),
        "brick" => Lattice::brick(tile),
        "quarter-drop" => Lattice::quarter_drop(tile),
        _ => Lattice::half_drop(tile),
    };

    // Motif sizes first: the scatter solves its separation radius from motif
    // area, so a size change has to be visible to it.
    for (id, longest_edge) in &params.sizes {
        if let Some(motif) = session.bank.get(id) {
            let natural = motif.natural_size_mm;
            let longest = natural.x.max(natural.y).max(1e-3);
            let scaled = natural * (longest_edge.max(2.0) / longest);
            session.bank.set_natural_size(id, scaled);
            if let Some(entry) = session.document.motifs.iter_mut().find(|m| &m.id == id) {
                entry.natural_size_mm = Some(scaled);
            }
        }
    }

    // Switching between a scatter and a grid rewrites which of the two the
    // document carries, since they are mutually exclusive by design.
    apply_layout(session, params)?;

    if let Some(spec) = session.document.scatter.as_mut() {
        spec.density = params.density.clamp(0.0, 2.0);
        spec.prevent_overlap = params.prevent_overlap;
        spec.on_axis_share = params.on_axis_share.clamp(0.0, 1.0);
        spec.rotation_jitter_deg = params.rotation_jitter_deg;
        spec.scale_range = (params.scale_range[0], params.scale_range[1]);
        spec.seed = params.seed;
        for (id, weight) in &params.weights {
            if let Some(entry) = spec.mix.iter_mut().find(|e| &e.motif_id == id) {
                entry.weight = *weight;
            }
        }
        // Weights of zero would leave the sampler with nothing to place; drop
        // those motifs from the mix instead, which is what a designer means by
        // turning one off.
        spec.mix.retain(|e| e.weight > 0.0);
        if spec.mix.is_empty() {
            anyhow::bail!("every motif is switched off");
        }
    }

    if let Some(spec) = session.document.grid.as_mut() {
        spec.prevent_overlap = params.prevent_overlap;
        spec.rows = params.grid_rows.clamp(1, 24);
        spec.cols = params.grid_cols.clamp(1, 24);
        spec.rotation_jitter_deg = params.rotation_jitter_deg;
        spec.scale_range = (params.scale_range[0], params.scale_range[1]);
        spec.seed = params.seed;
        for (id, weight) in &params.weights {
            if let Some(entry) = spec.mix.iter_mut().find(|e| &e.motif_id == id) {
                entry.weight = *weight;
            }
        }
        if spec.mix.iter().all(|e| e.weight <= 0.0) {
            anyhow::bail!("every motif is switched off");
        }
    }

    let bank = session.bank.clone();
    session.document.reseed(params.seed, &bank)?;
    Ok(())
}

/// Move the document between a scatter and a named grid preset.
///
/// The two are mutually exclusive, so this swaps one for the other and carries
/// the motif mix across — a designer trying layouts should not lose which
/// motifs they had chosen.
fn apply_layout(session: &mut Session, params: &Params) -> anyhow::Result<()> {
    let mirror = mirror_for(&params.layout);
    let wants_grid = mirror.is_none();
    let mix: Vec<pattern_doc::document::MixEntry> = session
        .document
        .scatter
        .as_ref()
        .map(|s| s.mix.clone())
        .or_else(|| session.document.grid.as_ref().map(|g| g.mix.clone()))
        .unwrap_or_default();
    if mix.is_empty() {
        return Ok(());
    }

    if wants_grid {
        let (_, row_offset, col_offset) = PRESETS
            .iter()
            .find(|(name, _, _)| *name == params.layout)
            .copied()
            .unwrap_or(("square", 0.0, 0.0));

        // Always block. The offsets make the pattern; an oblique lattice on
        // top of them is redundant at best and a break along every tile edge
        // at worst.
        let tile = session.document.pattern.tile_size_mm;
        session.document.pattern.lattice = Lattice::block(tile);
        session.document.scatter = None;
        session.document.grid = Some(pattern_doc::GridSpec {
            rows: params.grid_rows.clamp(1, 24),
            cols: params.grid_cols.clamp(1, 24),
            prevent_overlap: params.prevent_overlap,
            row_offset,
            col_offset,
            rotation_jitter_deg: params.rotation_jitter_deg,
            scale_range: (params.scale_range[0], params.scale_range[1]),
            mix,
            seed: params.seed,
        });
    } else {
        let mirror = mirror.unwrap_or(Mirror::None);
        session.document.grid = None;
        if session.document.scatter.is_none() {
            session.document.scatter = Some(pattern_doc::ScatterSpec {
                density: params.density.clamp(0.0, 2.0),
                prevent_overlap: params.prevent_overlap,
                mirror,
                on_axis_share: params.on_axis_share.clamp(0.0, 1.0),
                rotation_jitter_deg: params.rotation_jitter_deg,
                scale_range: (params.scale_range[0], params.scale_range[1]),
                mix,
                seed: params.seed,
            });
        } else if let Some(spec) = session.document.scatter.as_mut() {
            spec.mirror = mirror;
        }
        // Reflections generate translations by the tile itself, which is the
        // block lattice. Leaving an oblique one in place from a grid preset
        // the designer tried a moment ago would make the pattern refuse to
        // render, and the fault would look like the mirror's.
        if mirror != Mirror::None {
            let tile = session.document.pattern.tile_size_mm;
            session.document.pattern.lattice = Lattice::block(tile);
        }
    }
    session.document.pattern.placements.clear();
    Ok(())
}

fn render(session: &Session, params: &Params) -> anyhow::Result<Preview> {
    let pattern = &session.document.pattern;
    // Grow the window to one that is itself a repeating unit. A rectangle of
    // tiles is not generally one: a half-drop gains half a tile of drop per
    // column, so three columns end half a tile out and the preview's left and
    // right edges do not meet — which looks exactly like a broken repeat to
    // anyone checking it the obvious way.
    let (cols, rows) = pattern.lattice.repeating_window(
        pattern.tile_size_mm,
        params.cols.max(1),
        params.rows.max(1),
    );

    // The UI asks in screen pixels; physical units stay authoritative and the
    // resolution is derived, never the other way round.
    let field_mm = pattern.tile_size_mm.x * cols as f32;
    let dpi = (params.preview_width_px.max(200) as f32 / field_mm) * pattern_core::MM_PER_INCH;

    let pixmap = pattern_render::render_field(pattern, &session.bank, dpi, cols, rows)?;
    let png = pixmap
        .encode_png()
        .map_err(|e| anyhow::anyhow!("encoding preview: {e}"))?;

    let motifs: Vec<pattern_core::Motif> = session.bank.motifs().into_iter().cloned().collect();
    let mean_area: f32 = if motifs.is_empty() {
        1.0
    } else {
        motifs
            .iter()
            .map(|m| m.natural_size_mm.x * m.natural_size_mm.y)
            .sum::<f32>()
            / motifs.len() as f32
    };
    let tile_area = pattern.tile_size_mm.x * pattern.tile_size_mm.y;

    Ok(Preview {
        png_base64: base64::encode(&png),
        width: pixmap.width(),
        height: pixmap.height(),
        placements: pattern.placements.len(),
        colours: pattern_core::resolved_color_count(pattern, &motifs),
        achieved_density: pattern.placements.len() as f32 * mean_area / tile_area,
        dpi,
        cols,
        rows,
        // False only for a hand-edited lattice with no small whole-number
        // relationship to its tile, where no window within reach repeats.
        window_repeats: pattern
            .lattice
            .repeating_window(pattern.tile_size_mm, cols, rows)
            == (cols, rows),
        size_limited_to: session.document.grid_scale_applied,
        grid_cells: [
            session.document.grid_cells_used.0,
            session.document.grid_cells_used.1,
        ],
    })
}

/// What an export will be, before it is written.
///
/// Shown before the file dialog so the physical size and its verdict are in
/// front of a designer while they still have a choice — a 2 m sheet is a
/// decision, not something to discover from a bureau.
#[derive(Serialize)]
struct ExportPlan {
    width_mm: f32,
    height_mm: f32,
    width_px: u32,
    height_px: u32,
    cols: u32,
    rows: u32,
    /// "fits", "wide" or "too-wide".
    width_verdict: String,
    megabytes: f32,
    suggested_name: String,
}

#[derive(Deserialize)]
struct ExportRequest {
    /// "png" or "tiff".
    format: String,
    dpi: f32,
    /// "tile" for one repeat, "sheet" for a pre-repeated image.
    shape: String,
    #[serde(default = "three")]
    min_cols: u32,
    #[serde(default = "three")]
    min_rows: u32,
    #[serde(default)]
    compress: bool,
    /// Width of the fabric roll this is destined for, in millimetres.
    #[serde(default = "standard_roll")]
    roll_width_mm: f32,
    /// Path to a CMYK ICC profile. Absent means write RGB.
    ///
    /// Supplied rather than bundled: every freely redistributable CMYK profile
    /// describes an offset litho press, and a digital textile press is not one.
    /// A default would produce a file that looks converted and is wrong.
    #[serde(default)]
    cmyk_profile: Option<String>,
    /// Path to the RGB profile the artwork is in. Defaults to the system sRGB
    /// profile, which is what a screen-authored palette actually is.
    #[serde(default)]
    rgb_profile: Option<String>,
}

/// Where sRGB lives on this machine, for the common case where nobody has
/// thought about it. Artwork authored on screen is sRGB whether or not anyone
/// said so.
const SYSTEM_SRGB: &str = "/System/Library/ColorSync/Profiles/sRGB Profile.icc";

fn rgb_profile_path(request: &ExportRequest) -> PathBuf {
    request
        .rgb_profile
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SYSTEM_SRGB))
}

fn three() -> u32 {
    3
}

fn standard_roll() -> f32 {
    pattern_io::STANDARD_ROLL_MM
}

fn to_export(request: &ExportRequest) -> pattern_io::Export {
    pattern_io::Export {
        format: if request.format.eq_ignore_ascii_case("tiff") {
            pattern_io::Format::Tiff
        } else {
            pattern_io::Format::Png
        },
        compression: if request.compress {
            pattern_io::Compression::Lzw
        } else {
            pattern_io::Compression::None
        },
        dpi: request.dpi.clamp(24.0, 1200.0),
        shape: if request.shape.eq_ignore_ascii_case("sheet") {
            pattern_io::Shape::Sheet {
                min_cols: request.min_cols.clamp(1, 24),
                min_rows: request.min_rows.clamp(1, 24),
            }
        } else {
            pattern_io::Shape::Tile
        },
    }
}

#[tauri::command]
fn export_plan(app: State<'_, App>, request: ExportRequest) -> Reply<ExportPlan> {
    let guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_ref().ok_or("no document is open")?;
    let export = to_export(&request);
    let pattern = &session.document.pattern;

    let (cols, rows) = match export.shape {
        pattern_io::Shape::Tile => (1, 1),
        pattern_io::Shape::Sheet { min_cols, min_rows } => {
            pattern_doc::sheet_tiles(&session.document, min_cols, min_rows)
        }
    };
    // Ask the renderer's own quantisation what the file will be, rather than
    // recomputing it here and risking the plan disagreeing with the export.
    let pl = pattern_render::pixel_lattice_for(pattern, export.dpi)
        .map_err(|e| fail("could not work out the export size", e))?;
    let size = pattern_io::PrintSize::from_pixels(
        (pl.tile_px.x * cols as i32).max(1) as u32,
        (pl.tile_px.y * rows as i32).max(1) as u32,
        export.dpi,
    );

    // Three bytes a pixel for TIFF, and PNG compresses but not predictably;
    // an honest over-estimate is more use than an optimistic one when the
    // question is whether a file can be emailed.
    let megabytes = size.width_px as f32 * size.height_px as f32 * 3.0 / 1_048_576.0;

    Ok(ExportPlan {
        width_mm: size.width_mm,
        height_mm: size.height_mm,
        width_px: size.width_px,
        height_px: size.height_px,
        cols,
        rows,
        width_verdict: match size.width_verdict(request.roll_width_mm) {
            pattern_io::WidthVerdict::Fits => "fits",
            pattern_io::WidthVerdict::WiderThanStandardRoll => "wide",
            pattern_io::WidthVerdict::WiderThanGrandFormat => "too-wide",
        }
        .into(),
        megabytes,
        suggested_name: pattern_io::suggested_name(
            session
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "pattern".into())
                .as_str(),
            &lattice_name(&pattern.lattice, pattern.tile_size_mm),
            pattern.tile_size_mm,
            export.dpi,
            export.shape,
            export.format,
        ),
    })
}

#[tauri::command]
fn export(app: State<'_, App>, path: String, request: ExportRequest) -> Reply<String> {
    let guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_ref().ok_or("no document is open")?;
    let export = to_export(&request);
    let path = PathBuf::from(path);

    match (&request.cmyk_profile, export.format) {
        (Some(profile), pattern_io::Format::Tiff) => {
            let (pixmap, _, samples, embedded) = pattern_doc::render_cmyk(
                &session.document,
                &session.bank,
                export,
                &rgb_profile_path(&request),
                Path::new(profile),
            )
            .map_err(|e| fail("could not convert to CMYK", e))?;
            pattern_io::write_written(
                &path,
                &pixmap,
                pattern_io::Written {
                    export,
                    cmyk: Some((&samples, &embedded)),
                },
            )
            .map_err(|e| fail("could not write the file", e))?;
        }
        (Some(_), pattern_io::Format::Png) => {
            return Err("PNG cannot carry CMYK — choose TIFF, or export in RGB".into());
        }
        (None, _) => {
            let (pixmap, _) =
                pattern_doc::render_for_export(&session.document, &session.bank, export)
                    .map_err(|e| fail("could not render for export", e))?;
            pattern_io::write(&path, &pixmap, export)
                .map_err(|e| fail("could not write the file", e))?;
        }
    }
    Ok(path.display().to_string())
}

/// The garments a print can be seen on.
#[derive(Serialize)]
struct GarmentOption {
    name: String,
    chest_mm: f32,
    length_mm: f32,
}

#[tauri::command]
fn garments() -> Vec<GarmentOption> {
    pattern_render::Garment::standard()
        .into_iter()
        .map(|g| GarmentOption {
            name: g.name,
            chest_mm: g.chest_mm,
            length_mm: g.length_mm,
        })
        .collect()
}

/// A print on a garment, at true scale.
#[derive(Serialize)]
struct MockupView {
    png_base64: String,
    width: u32,
    height: u32,
    /// How many repeats span the chest — the number this view exists to make
    /// obvious.
    repeats_across: f32,
    chest_mm: f32,
    length_mm: f32,
}

/// Show the print on a garment.
///
/// Worth its weight despite looking cosmetic: seeing a repeat at its real size
/// against a real body is how a scale error is caught before fabric is bought.
#[tauri::command]
fn mockup(
    app: State<'_, App>,
    params: Params,
    garment: String,
    width_px: u32,
) -> Reply<MockupView> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    write_params(session, &params).map_err(|e| fail("could not apply those settings", e))?;

    let chosen = pattern_render::Garment::standard()
        .into_iter()
        .find(|g| g.name == garment)
        .ok_or_else(|| format!("no garment called '{garment}'"))?;

    let rendered = pattern_render::render_mockup(
        &session.document.pattern,
        &session.bank,
        &chosen,
        width_px.clamp(160, 1600),
    )
    .map_err(|e| fail("could not render the garment", e))?;

    let png = rendered
        .pixmap
        .encode_png()
        .map_err(|e| fail("could not encode the mockup", e))?;
    Ok(MockupView {
        png_base64: base64::encode(&png),
        width: rendered.pixmap.width(),
        height: rendered.pixmap.height(),
        repeats_across: rendered.repeats_across,
        chest_mm: chosen.chest_mm,
        length_mm: chosen.length_mm,
    })
}

/// Preview a pattern in the colours a press can reach.
///
/// Converts to the press's colours and straight back, so anything it cannot
/// reach comes back changed. Seeing a red go flat before the fabric is bought
/// is the entire purpose — "it looked different on screen" is the commonest
/// complaint in fabric printing and is almost always a conversion nobody
/// looked at.
#[tauri::command]
fn proof(app: State<'_, App>, params: Params, cmyk_profile: String) -> Reply<Preview> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;

    write_params(session, &params).map_err(|e| fail("could not apply those settings", e))?;
    let mut preview = render(session, &params).map_err(|e| fail("could not render", e))?;

    let pattern = &session.document.pattern;
    let (cols, rows) = pattern.lattice.repeating_window(
        pattern.tile_size_mm,
        params.cols.max(1),
        params.rows.max(1),
    );
    let field_mm = pattern.tile_size_mm.x * cols as f32;
    let dpi = (params.preview_width_px.max(200) as f32 / field_mm) * pattern_core::MM_PER_INCH;
    let pixmap = pattern_render::render_field(pattern, &session.bank, dpi, cols, rows)
        .map_err(|e| fail("could not render", e))?;

    let proofed =
        pattern_doc::soft_proof(&pixmap, Path::new(SYSTEM_SRGB), Path::new(&cmyk_profile))
            .map_err(|e| fail("could not soft proof", e))?;
    let png = proofed
        .encode_png()
        .map_err(|e| fail("could not encode the proof", e))?;
    preview.png_base64 = base64::encode(&png);
    Ok(preview)
}

/// Whether Save can write without asking, and where it would suggest.
#[derive(Serialize)]
struct SaveTarget {
    needs_path: bool,
    suggested: String,
}

#[tauri::command]
fn save_target(app: State<'_, App>) -> Reply<SaveTarget> {
    let guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_ref().ok_or("no document is open")?;
    Ok(SaveTarget {
        needs_path: !session.saved,
        suggested: session.path.display().to_string(),
    })
}

#[tauri::command]
fn save_document(app: State<'_, App>, path: Option<String>) -> Reply<String> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    let target = match path {
        Some(p) => PathBuf::from(p),
        None if session.saved => session.path.clone(),
        None => return Err("this pattern has not been saved yet, so it needs a filename".into()),
    };

    // Motif paths are rewritten relative to wherever the document lands, so a
    // project folder can be moved or shared without breaking.
    //
    // Placements are written out too. A scatter regenerates them from the seed,
    // but recording them means the file still renders identically if the
    // sampler ever changes — a document is the record of what was approved,
    // not a recipe for approximating it.
    let rewritten = pattern_doc::save_to(&session.document, &target)
        .map_err(|e| fail("could not write the file", e))?;
    session.document = rewritten;
    session.path = target.clone();
    session.saved = true;
    session.mark_written();
    Ok(target.display().to_string())
}
/// Parse a CSS hex colour, which is how the interface talks about colour.
fn parse_hex(hex: &str) -> Option<Color> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Color::new(byte(0)?, byte(2)?, byte(4)?))
}

/// The palette as it stands, after any change.
#[derive(Serialize)]
struct PaletteState {
    palette: Vec<String>,
    colourways: Vec<String>,
    active_colourway: Option<String>,
}

fn palette_state(session: &Session) -> PaletteState {
    PaletteState {
        palette: session
            .document
            .pattern
            .palette
            .colors
            .iter()
            .map(hex)
            .collect(),
        colourways: session
            .document
            .colourways
            .iter()
            .map(|c| c.name.clone())
            .collect(),
        active_colourway: session.document.active_colourway.clone(),
    }
}

/// Change one slot's colour.
///
/// Slot 0 is the ground and is edited like any other, which is the point of
/// having made it a slot: inverting a print to a dark background is an
/// ordinary palette edit rather than a special case.
#[tauri::command]
fn set_slot_colour(app: State<'_, App>, slot: u8, colour: String) -> Reply<PaletteState> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    let colour = parse_hex(&colour).ok_or_else(|| format!("'{colour}' is not a hex colour"))?;
    session.document.pattern.palette =
        pattern_core::with_colour(&session.document.pattern.palette, slot, colour);
    Ok(palette_state(session))
}

/// Derive a candidate colourway from the one on screen.
///
/// Four colourways in a second and pick one, which is what the spec asks of
/// this: the moves are cheap and the judgement is not.
#[tauri::command]
fn derive_colourway(app: State<'_, App>, kind: String, amount: f32) -> Reply<PaletteState> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    let current = &session.document.pattern.palette;
    let derived = match kind.as_str() {
        "hue" => pattern_core::rotate_hue(current, amount),
        "lighter" => pattern_core::shift_lightness(current, amount.abs()),
        "darker" => pattern_core::shift_lightness(current, -amount.abs()),
        "invert" => pattern_core::swap_ground(current, amount.max(1.0) as u8),
        other => return Err(format!("no such colourway move: '{other}'")),
    };
    session.document.pattern.palette = derived;
    Ok(palette_state(session))
}

/// Store the palette on screen under a name.
#[tauri::command]
fn remember_colourway(app: State<'_, App>, name: String) -> Reply<PaletteState> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    let name = name.trim();
    if name.is_empty() {
        return Err("a colourway needs a name".into());
    }
    session.document.remember_colourway(name);
    Ok(palette_state(session))
}

/// Switch to a stored colourway.
#[tauri::command]
fn use_colourway(app: State<'_, App>, name: Option<String>) -> Reply<PaletteState> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    session.document.active_colourway = name;
    session
        .document
        .apply_colourway()
        .map_err(|e| fail("could not switch colourway", e))?;
    Ok(palette_state(session))
}

/// What a change of palette size did.
#[derive(Serialize)]
struct PaletteSized {
    palette: Vec<String>,
    palette_colours: usize,
    colourways: Vec<String>,
    active_colourway: Option<String>,
    /// Named rather than counted: a designer should be told which of their
    /// work was discarded, not how much.
    colourways_dropped: Vec<String>,
    motifs: Vec<MotifEntry>,
}

/// Re-derive the design palette at a different number of colours.
///
/// A separate command rather than another field on `apply`, because it re-runs
/// ingest: the motifs are quantised again against the new palette and the bank
/// is rebuilt. Everything else in `apply` leaves the slots alone and moves
/// motifs about.
#[tauri::command]
fn set_palette_size(app: State<'_, App>, colours: usize) -> Reply<PaletteSized> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;

    let resize = session
        .document
        .set_palette_size(colours, &session.path)
        .map_err(|e| fail("could not change the colour count", e))?;
    session.bank = session
        .document
        .load_motifs(&session.path)
        .map_err(|e| fail("could not re-quantise the motifs", e))?;

    let scatter = session.document.scatter.as_ref();
    Ok(PaletteSized {
        palette: session
            .document
            .pattern
            .palette
            .colors
            .iter()
            .map(hex)
            .collect(),
        palette_colours: resize.colours,
        colourways: session
            .document
            .colourways
            .iter()
            .map(|c| c.name.clone())
            .collect(),
        active_colourway: session.document.active_colourway.clone(),
        colourways_dropped: resize.colourways_dropped,
        motifs: motif_entries(&session.bank, scatter),
    })
}

/// The house palettes a document can be rendered in.
///
/// Reads what the document already carries rather than offering an editor:
/// palette management is M5, and a half-built one here would be the kind of
/// partial implementation that gets trusted.
#[tauri::command]
fn palette(app: State<'_, App>) -> Reply<Vec<String>> {
    let guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_ref().ok_or("no document is open")?;
    Ok(session
        .document
        .pattern
        .palette
        .colors
        .iter()
        .map(hex)
        .collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App::default())
        .invoke_handler(tauri::generate_handler![
            open_document,
            new_from_images,
            apply,
            save_target,
            export_plan,
            export,
            proof,
            garments,
            mockup,
            save_document,
            set_slot_colour,
            derive_colourway,
            remember_colourway,
            use_colourway,
            set_palette_size,
            close_document,
            palette
        ])
        .run(tauri::generate_context!())
        .expect("the app should start");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real document, opened the way the app opens one.
    ///
    /// The commands themselves are three lines of lock-and-delegate; what is
    /// worth testing is the layer underneath, which decides what a slider
    /// actually does. These tests reach it directly, so the app's behaviour is
    /// covered without needing a window — which matters because a window is the
    /// one thing CI will not have.
    fn session() -> Session {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/peel-club-scatter/pattern.json");
        let (document, bank) = Document::load(&path).expect("the example should open");
        let mut session = Session {
            path,
            document,
            bank,
            saved: true,
            written: String::new(),
        };
        session.mark_written();
        session
    }

    fn params(density: f32, seed: u64) -> Params {
        Params {
            tile_mm: [240.0, 180.0],
            lattice: "half-drop".into(),
            density,
            rotation_jitter_deg: 20.0,
            scale_range: [0.85, 1.15],
            seed,
            weights: vec![("cherry".into(), 1.0)],
            sizes: vec![],
            layout: "tossed".into(),
            prevent_overlap: true,
            on_axis_share: 0.0,
            grid_rows: 3,
            grid_cols: 3,
            preview_width_px: 600,
            cols: 3,
            rows: 3,
        }
    }

    /// Files a designer would actually pick: loose artwork, no document.
    fn artwork() -> Vec<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        vec![
            root.join("generated-motifs/01-cherries.png"),
            root.join("generated-motifs/03-lemon-slice.png"),
        ]
    }

    #[test]
    fn a_pattern_can_be_started_from_images_alone() {
        // The gap M4 shipped with: the app could open a document and had no way
        // to make one, so the only route in was hand-writing JSON — which is
        // the thing the milestone exists to remove.
        let files = artwork();
        if !files.iter().all(|f| f.exists()) {
            // The generated artwork is not committed; skip rather than fail on
            // a machine that has not produced it.
            return;
        }
        let mut document =
            pattern_doc::new_from_images(&files, 50.0, 6).expect("should build a document");
        let anchor = files[0].parent().unwrap().join("untitled.json");
        let bank = document
            .load_motifs(&anchor)
            .expect("should read the images");
        document
            .apply_scatter_public(&bank)
            .expect("should scatter");

        assert_eq!(bank.len(), 2, "both images should become motifs");
        assert!(
            !document.pattern.placements.is_empty(),
            "a new pattern should arrive already scattered, not blank"
        );
        // The palette is derived rather than guessed, which is what stops a new
        // pattern quantising its artwork onto colours it does not contain.
        assert!(document.pattern.palette.len() >= 3);
        assert!(document.scatter.is_some(), "a new pattern is scattered");
    }

    #[test]
    fn motif_size_is_a_control_rather_than_a_property_of_the_file() {
        // The same drawing at 40 mm and 90 mm is two different prints, and
        // changing it must not mean ingesting the artwork again.
        let mut s = session();
        let mut p = params(0.3, 1);
        p.sizes = vec![("cherry".into(), 90.0)];
        write_params(&mut s, &p).expect("apply");
        let big = s.bank.get("cherry").expect("motif").natural_size_mm;
        assert!((big.x.max(big.y) - 90.0).abs() < 0.01);

        p.sizes = vec![("cherry".into(), 30.0)];
        write_params(&mut s, &p).expect("apply");
        let small = s.bank.get("cherry").expect("motif").natural_size_mm;
        assert!((small.x.max(small.y) - 30.0).abs() < 0.01);

        // Aspect ratio comes from the artwork and is never adjusted.
        assert!(((big.x / big.y) - (small.x / small.y)).abs() < 1e-3);
    }

    #[test]
    fn a_smaller_motif_means_more_of_them_at_the_same_coverage() {
        // Density is a share of the fabric, not a count, so shrinking a motif
        // has to put more of them down to cover the same area.
        let mut s = session();
        let mut p = params(0.3, 1);
        p.sizes = vec![("cherry".into(), 60.0)];
        write_params(&mut s, &p).expect("apply");
        let few = s.document.pattern.placements.len();
        p.sizes = vec![("cherry".into(), 25.0)];
        write_params(&mut s, &p).expect("apply");
        let many = s.document.pattern.placements.len();
        assert!(many > few, "shrinking motifs gave {few} then {many}");
    }

    #[test]
    fn switching_to_a_grid_swaps_the_layout_rather_than_keeping_both() {
        // Scatter and grid are mutually exclusive; a document carrying both is
        // refused, so the swap has to be complete.
        let mut s = session();
        let mut p = params(0.3, 1);
        p.layout = "brick".into();
        write_params(&mut s, &p).expect("apply");
        assert!(s.document.grid.is_some() && s.document.scatter.is_none());
        // The count is the grid the engine settled on, not the one typed: a
        // brick's half-cell shift needs an even number of rows to close over
        // the tile, and three was asked for.
        let (rows, cols) = s.document.grid_cells_used;
        assert_eq!(s.document.pattern.placements.len(), (rows * cols) as usize);
        assert_eq!((rows, cols), (4, p.grid_cols));

        p.layout = "tossed".into();
        write_params(&mut s, &p).expect("apply");
        assert!(s.document.scatter.is_some() && s.document.grid.is_none());
    }

    #[test]
    fn the_motif_mix_survives_a_change_of_layout() {
        // A designer trying arrangements should not lose which motifs they
        // chose along the way.
        let mut s = session();
        let mut p = params(0.3, 1);
        p.layout = "diagonal".into();
        write_params(&mut s, &p).expect("apply");
        let grid_mix: Vec<String> = s
            .document
            .grid
            .as_ref()
            .expect("grid")
            .mix
            .iter()
            .map(|e| e.motif_id.clone())
            .collect();
        assert_eq!(grid_mix, vec!["cherry".to_string()]);
    }

    #[test]
    fn the_colour_count_can_be_changed_and_the_motifs_follow_it() {
        let mut s = session();
        let before = s.document.pattern.palette.len();
        let reply = s
            .document
            .set_palette_size(20, &s.path)
            .expect("should re-derive");
        s.bank = s.document.load_motifs(&s.path).expect("should re-quantise");

        assert_eq!(reply.colours, 20);
        assert_ne!(before, 20);
        assert_eq!(s.document.pattern.palette.len(), 20);
        // The motifs were quantised again, not merely recoloured: a slot above
        // the old palette's range can only exist if they were.
        let highest = s
            .bank
            .motifs()
            .iter()
            .filter_map(|m| m.indexed.max_slot())
            .max()
            .expect("motifs have slots");
        assert!(
            highest as usize >= before,
            "highest slot {highest} is still within the old {before}-colour palette"
        );
    }

    #[test]
    fn a_wider_palette_pulls_a_motif_back_out_of_the_ground() {
        // The defect this control exists for, measured rather than described.
        // The cherry fixture's artwork is flat, so the interesting case is the
        // narrow end: squeeze the palette and ink starts landing on the ground.
        let mut s = session();
        s.document
            .set_palette_size(3, &s.path)
            .expect("should re-derive");
        s.bank = s.document.load_motifs(&s.path).expect("re-quantise");
        let narrow = s.bank.motifs()[0].indexed.ground_share();

        s.document
            .set_palette_size(16, &s.path)
            .expect("should re-derive");
        s.bank = s.document.load_motifs(&s.path).expect("re-quantise");
        let wide = s.bank.motifs()[0].indexed.ground_share();

        assert!(
            wide <= narrow,
            "widening the palette put more of the motif on the ground, {wide:.3} against \
             {narrow:.3}"
        );
    }

    #[test]
    fn changing_the_colour_count_drops_colourways_it_would_invalidate() {
        // And says which. A colourway is a list of colours for a set of slots;
        // change how many slots there are and it describes nothing. Left in
        // place it would fail on the next open, which is a document that no
        // longer loads.
        let mut s = session();
        s.document.remember_colourway("Night");
        assert_eq!(s.document.colourways.len(), 1);

        let reply = s
            .document
            .set_palette_size(s.document.pattern.palette.len() + 4, &s.path)
            .expect("should re-derive");

        assert_eq!(reply.colourways_dropped, vec!["Night".to_string()]);
        assert!(s.document.colourways.is_empty());
        assert!(s.document.active_colourway.is_none());
    }

    #[test]
    fn the_ground_colour_survives_a_change_of_colour_count() {
        // A designer who has set a navy ground should not have it revert to
        // off-white because they asked for more colours.
        let mut s = session();
        let navy = pattern_core::Color::new(24, 28, 38);
        s.document.pattern.palette =
            pattern_core::with_colour(&s.document.pattern.palette, 0, navy);

        s.document
            .set_palette_size(10, &s.path)
            .expect("should re-derive");
        assert_eq!(s.document.pattern.palette.ground(), Some(navy));
    }

    #[test]
    fn a_freshly_opened_document_has_nothing_to_lose() {
        // The claim the close prompt rests on. If merely opening a pattern
        // counted as a change, the warning would fire every time and stop
        // meaning anything.
        let s = session();
        assert!(!s.unsaved_changes());
    }

    #[test]
    fn the_interface_settling_after_an_open_is_not_a_change() {
        // The case that decides whether the warning is worth having. The app
        // renders a preview as soon as a document is open, and that goes
        // through the same path as moving a slider. If it wrote anything back
        // the document would be dirty the instant it appeared, and a prompt
        // that always fires teaches people to dismiss it.
        let mut s = session();
        let shown = describe(&s.path, &s.document, &s.bank);
        let settle = Params {
            tile_mm: shown.tile_mm,
            lattice: shown.lattice.clone(),
            density: shown.density,
            rotation_jitter_deg: shown.rotation_jitter_deg,
            scale_range: shown.scale_range,
            seed: shown.seed,
            layout: shown.layout.clone(),
            prevent_overlap: shown.prevent_overlap,
            on_axis_share: shown.on_axis_share,
            grid_rows: shown.grid_rows,
            grid_cols: shown.grid_cols,
            weights: shown
                .motifs
                .iter()
                .map(|m| (m.id.clone(), m.weight))
                .collect(),
            sizes: shown
                .motifs
                .iter()
                .map(|m| (m.id.clone(), m.longest_edge_mm))
                .collect(),
            preview_width_px: 600,
            cols: 3,
            rows: 3,
        };

        write_params(&mut s, &settle).expect("apply");
        assert!(
            !s.unsaved_changes(),
            "showing the document changed it, so the close prompt would always fire"
        );
    }

    #[test]
    fn touching_the_document_is_enough_to_count_as_a_change() {
        let mut s = session();
        let mut p = params(0.3, 1);
        p.seed = 4242;
        write_params(&mut s, &p).expect("apply");
        assert!(
            s.unsaved_changes(),
            "a re-seeded scatter should count as unsaved work"
        );
    }

    #[test]
    fn a_change_no_deeper_than_a_palette_slot_still_counts() {
        // The reason this is a comparison rather than a flag: a colour edit
        // does not go through the parameter path at all, and a flag set there
        // would miss it.
        let mut s = session();
        s.document.pattern.palette = pattern_core::with_colour(
            &s.document.pattern.palette,
            0,
            pattern_core::Color::new(1, 2, 3),
        );
        assert!(s.unsaved_changes());
    }

    #[test]
    fn a_pattern_that_has_never_been_written_always_has_something_to_lose() {
        let mut s = session();
        s.saved = false;
        assert!(
            s.unsaved_changes(),
            "an unsaved pattern is lost entirely by closing, changed or not"
        );
    }

    #[test]
    fn a_mirror_is_a_scatter_and_not_a_grid() {
        // A mirror belongs with the strewn arrangements: it has a coverage,
        // not rows and columns. Filed under the grid presets it would lose the
        // density control and gain two that mean nothing.
        let mut s = session();
        let mut p = params(0.3, 1);
        for layout in ["mirror", "mirror-vertical", "mirror-four"] {
            p.layout = layout.into();
            write_params(&mut s, &p).expect("apply");
            assert!(
                s.document.scatter.is_some() && s.document.grid.is_none(),
                "{layout} did not stay a scatter"
            );
        }
    }

    #[test]
    fn choosing_a_mirror_puts_the_repeat_back_on_a_block_lattice() {
        // Reflections repeat by the tile itself, which no oblique lattice
        // does. A document carrying one — from a hand-written file, or from
        // the days when a grid preset chose one — would otherwise be left with
        // a combination the engine refuses to render, and the refusal would
        // look like the mirror's fault.
        let mut s = session();
        let mut p = params(0.3, 1);
        s.document.pattern.lattice = Lattice::half_drop(s.document.pattern.tile_size_mm);
        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "half-drop"
        );

        p.layout = "mirror-four".into();
        write_params(&mut s, &p).expect("apply");
        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "block"
        );
        s.document.pattern.validate().expect("should be renderable");
    }

    #[test]
    fn each_mirror_reflects_the_tile_it_names() {
        let mut s = session();
        let mut p = params(0.3, 1);
        for (layout, copies) in [
            ("tossed", 1),
            ("mirror", 2),
            ("mirror-vertical", 2),
            ("mirror-four", 4),
        ] {
            p.layout = layout.into();
            write_params(&mut s, &p).expect("apply");
            let placements = &s.document.pattern.placements;
            assert_eq!(
                placements.len() % copies,
                0,
                "{layout} placed {} motifs, not a multiple of {copies}",
                placements.len()
            );
            let reflected = placements.iter().filter(|q| q.flip_x || q.flip_y).count();
            assert_eq!(
                reflected,
                placements.len() - placements.len() / copies,
                "{layout} reflected the wrong share of its motifs"
            );
        }
    }

    #[test]
    fn motifs_can_be_mounted_on_the_mirror_lines() {
        let mut s = session();
        let mut p = params(0.4, 1);
        p.layout = "mirror".into();
        write_params(&mut s, &p).expect("apply");
        let plain = s.document.pattern.placements.len();

        p.on_axis_share = 0.5;
        write_params(&mut s, &p).expect("apply");
        let mounted = s.document.pattern.placements.len();

        assert_eq!(
            s.document.scatter.as_ref().expect("scatter").on_axis_share,
            0.5
        );
        assert!(
            mounted > plain,
            "mounting motifs on the lines gave {mounted} against the plain mirror's {plain}"
        );
    }

    #[test]
    fn the_mirror_line_share_is_kept_within_bounds() {
        let mut s = session();
        let mut p = params(0.4, 1);
        p.layout = "mirror".into();
        p.on_axis_share = 4.0;
        write_params(&mut s, &p).expect("apply");
        assert_eq!(
            s.document.scatter.as_ref().expect("scatter").on_axis_share,
            1.0
        );
    }

    #[test]
    fn a_mirror_survives_being_saved_and_reopened() {
        // The layout the UI shows on reopening is read back from the
        // document, so a mirror has to be recoverable from what was written
        // rather than remembered in the session.
        let mut s = session();
        let mut p = params(0.3, 1);
        p.layout = "mirror-four".into();
        write_params(&mut s, &p).expect("apply");

        let json = serde_json::to_string(&s.document).expect("serialise");
        let reopened: pattern_doc::Document = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(
            scatter_layout_name(reopened.scatter.as_ref().expect("scatter").mirror),
            "mirror-four"
        );
    }

    #[test]
    fn a_document_written_before_mirrors_reads_as_having_none() {
        let json = r#"{"density":0.3,"prevent_overlap":true,"mix":[],"seed":7}"#;
        let spec: pattern_doc::ScatterSpec = serde_json::from_str(json).expect("deserialise");
        assert_eq!(scatter_layout_name(spec.mirror), "tossed");
    }

    #[test]
    fn every_grid_preset_sits_on_a_block_lattice() {
        // They used to pick an oblique one each, which contributed nothing
        // where it worked and broke the field along every tile edge where it
        // did not. The offsets make the pattern now.
        let mut s = session();
        let mut p = params(0.3, 1);
        for preset in [
            "square",
            "brick",
            "half-drop",
            "diagonal",
            "gentle-diagonal",
        ] {
            p.layout = preset.into();
            write_params(&mut s, &p).expect("apply");
            assert_eq!(
                lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
                "block",
                "preset {preset}"
            );
        }
    }

    #[test]
    fn brick_and_half_drop_are_no_longer_the_same_grid() {
        // Both used to be a row offset of a half, told apart only by the
        // lattice underneath. A brick shifts alternate rows sideways; a
        // half-drop drops alternate columns.
        let mut s = session();
        let mut p = params(0.3, 1);

        p.layout = "brick".into();
        write_params(&mut s, &p).expect("apply");
        let brick = s.document.grid.clone().expect("grid");

        p.layout = "half-drop".into();
        write_params(&mut s, &p).expect("apply");
        let half_drop = s.document.grid.clone().expect("grid");

        assert_eq!((brick.row_offset, brick.col_offset), (0.5, 0.0));
        assert_eq!((half_drop.row_offset, half_drop.col_offset), (0.0, 0.5));
        assert_eq!(preset_name(0.5, 0.0), "brick");
        assert_eq!(preset_name(0.0, 0.5), "half-drop");
    }

    #[test]
    fn a_grid_grows_until_its_offsets_close_over_the_tile() {
        // Reported from the app: a diagonal grid broke along a line down the
        // middle. Five rows of a one-third shift accumulate five thirds of a
        // column, so the next tile starts two thirds adrift.
        let mut s = session();
        let mut p = params(0.3, 1);
        p.layout = "diagonal".into();
        p.grid_rows = 5;
        p.grid_cols = 4;
        write_params(&mut s, &p).expect("apply");
        assert_eq!(s.document.grid_cells_used, (6, 4));

        // And a count that already closes is left exactly as asked.
        p.grid_rows = 6;
        write_params(&mut s, &p).expect("apply");
        assert_eq!(s.document.grid_cells_used, (6, 4));
    }

    #[test]
    fn preventing_overlap_is_on_by_default_and_can_be_turned_off() {
        // Reported from the app: motifs visibly intersecting at a plausible
        // coverage.
        let mut s = session();
        let mut p = params(0.7, 3);
        p.sizes = vec![("cherry".into(), 70.0)];

        p.prevent_overlap = true;
        write_params(&mut s, &p).expect("apply");
        let guarded = s.document.pattern.placements.len();

        p.prevent_overlap = false;
        write_params(&mut s, &p).expect("apply");
        let loose = s.document.pattern.placements.len();

        assert!(
            loose > guarded,
            "the guard changed nothing: {guarded} guarded, {loose} loose"
        );
    }

    #[test]
    fn the_density_control_changes_how_many_motifs_are_placed() {
        let mut s = session();
        write_params(&mut s, &params(0.15, 1)).expect("apply");
        let sparse = s.document.pattern.placements.len();
        write_params(&mut s, &params(0.45, 1)).expect("apply");
        let dense = s.document.pattern.placements.len();
        assert!(dense > sparse, "density did nothing: {sparse} then {dense}");
    }

    #[test]
    fn rerolling_the_seed_changes_the_arrangement() {
        // The control a designer presses most. It has to do something, and the
        // same seed has to come back to the same print.
        let mut s = session();
        write_params(&mut s, &params(0.3, 1)).expect("apply");
        let first: Vec<_> = s
            .document
            .pattern
            .placements
            .iter()
            .map(|p| p.position_mm)
            .collect();
        write_params(&mut s, &params(0.3, 2)).expect("apply");
        let second: Vec<_> = s
            .document
            .pattern
            .placements
            .iter()
            .map(|p| p.position_mm)
            .collect();
        assert_ne!(first, second);

        write_params(&mut s, &params(0.3, 1)).expect("apply");
        let again: Vec<_> = s
            .document
            .pattern
            .placements
            .iter()
            .map(|p| p.position_mm)
            .collect();
        assert_eq!(first, again, "the same seed must return the same print");
    }

    #[test]
    fn switching_the_repeat_switches_the_lattice() {
        let mut s = session();
        let mut p = params(0.3, 1);
        p.lattice = "block".into();
        write_params(&mut s, &p).expect("apply");
        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "block"
        );

        p.lattice = "half-drop".into();
        write_params(&mut s, &p).expect("apply");
        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "half-drop"
        );
    }

    #[test]
    fn changing_the_repeat_size_moves_the_lattice_with_it() {
        // A lattice built for the old tile would silently stop tiling.
        let mut s = session();
        let mut p = params(0.3, 1);
        p.tile_mm = [320.0, 200.0];
        write_params(&mut s, &p).expect("apply");
        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "half-drop"
        );
        assert_eq!(s.document.pattern.lattice.u.y, 100.0);
    }

    #[test]
    fn turning_every_motif_off_is_refused_rather_than_rendered_empty() {
        let mut s = session();
        let mut p = params(0.3, 1);
        p.weights = vec![("cherry".into(), 0.0)];
        assert!(write_params(&mut s, &p).is_err());
    }

    #[test]
    fn the_preview_grows_to_a_window_that_repeats() {
        // Reported from the app: a 3x3 preview of a half-drop did not meet at
        // its left and right edges. The tile was fine; the window was not a
        // repeating unit, so the obvious way to check a repeat gave the wrong
        // answer.
        let mut s = session();
        let mut p = params(0.3, 1);

        p.lattice = "half-drop".into();
        write_params(&mut s, &p).expect("apply");
        let half_drop = render(&s, &p).expect("render");
        assert_eq!((half_drop.cols, half_drop.rows), (4, 3));
        assert!(half_drop.window_repeats);

        p.lattice = "block".into();
        write_params(&mut s, &p).expect("apply");
        let block = render(&s, &p).expect("render");
        assert_eq!((block.cols, block.rows), (3, 3), "a block needs no growing");

        p.lattice = "brick".into();
        write_params(&mut s, &p).expect("apply");
        let brick = render(&s, &p).expect("render");
        assert_eq!(
            (brick.cols, brick.rows),
            (3, 4),
            "a brick staggers the other way"
        );
    }

    #[test]
    fn the_preview_comes_back_at_roughly_the_width_asked_for() {
        // The UI asks in screen pixels and the engine derives a resolution, so
        // that physical units stay authoritative. If this drifts, the preview
        // stops filling its pane.
        let mut s = session();
        let p = params(0.3, 1);
        write_params(&mut s, &p).expect("apply");
        let preview = render(&s, &p).expect("render");
        assert!(
            (preview.width as i32 - 600).abs() < 12,
            "asked for 600 px wide, got {}",
            preview.width
        );
        assert!(!preview.png_base64.is_empty());
        assert_eq!(preview.placements, s.document.pattern.placements.len());
    }

    /// The preview as pixels, decoded back from what the interface receives.
    ///
    /// Asserting on the bytes the app actually hands over, rather than on the
    /// pixmap before encoding, because the encoding and the base64 are part of
    /// what could be wrong.
    fn preview_pixels(preview: &Preview) -> pattern_render::tiny_skia::Pixmap {
        let raw = base64::decode(&preview.png_base64).expect("preview should be valid base64");
        pattern_render::tiny_skia::Pixmap::decode_png(&raw).expect("preview should be a PNG")
    }

    /// Share of the preview that is not the ground colour.
    fn inked_share(preview: &Preview, session: &Session) -> f32 {
        let image = preview_pixels(preview);
        let ground = session
            .document
            .pattern
            .palette
            .ground()
            .expect("palette has a ground");
        let inked = image
            .pixels()
            .iter()
            .filter(|p| {
                let c = p.demultiply();
                (c.red(), c.green(), c.blue()) != (ground.r, ground.g, ground.b)
            })
            .count();
        inked as f32 / image.pixels().len() as f32
    }

    #[test]
    fn the_preview_has_something_drawn_on_it() {
        // A renderer that draws nothing is indistinguishable from a fast one,
        // and every signal agrees that all is well: the command succeeds, the
        // dimensions are right, the PNG decodes, the placement count is
        // correct. The only assertion this file used to make on the image was
        // that its base64 was non-empty, which a picture of plain background
        // passes.
        let mut s = session();
        let p = params(0.3, 1);
        write_params(&mut s, &p).expect("apply");
        let preview = render(&s, &p).expect("render");

        let inked = inked_share(&preview, &s);
        assert!(
            inked > 0.05,
            "only {:.1}% of the preview is anything but the ground colour",
            inked * 100.0
        );
    }

    #[test]
    fn the_preview_changes_when_the_pattern_does() {
        // The failure the test above cannot see: a preview frozen on its first
        // frame. It is not blank, the dimensions are right, and it looks
        // entirely correct in a screenshot — it is simply the wrong picture.
        let mut s = session();
        let mut p = params(0.3, 1);
        write_params(&mut s, &p).expect("apply");
        let first = render(&s, &p).expect("render").png_base64;

        p.seed = 20260923;
        write_params(&mut s, &p).expect("apply");
        let reseeded = render(&s, &p).expect("render").png_base64;
        assert_ne!(
            first, reseeded,
            "rerolling the seed did not change the preview"
        );

        // And to a change that is not the seed, since a preview could be
        // regenerated from the seed alone and still ignore its parameters.
        p.seed = 1;
        p.density = 0.6;
        write_params(&mut s, &p).expect("apply");
        let denser = render(&s, &p).expect("render").png_base64;
        assert_ne!(
            first, denser,
            "raising the coverage did not change the preview"
        );
    }

    #[test]
    fn more_coverage_puts_more_ink_on_the_preview() {
        // Stronger than "the bytes differ": the picture has to change in the
        // direction the control promises. A preview wired to the wrong
        // parameter would still differ.
        let mut s = session();
        let mut p = params(0.15, 4);
        write_params(&mut s, &p).expect("apply");
        let sparse = inked_share(&render(&s, &p).expect("render"), &s);

        p.density = 0.6;
        write_params(&mut s, &p).expect("apply");
        let dense = inked_share(&render(&s, &p).expect("render"), &s);

        assert!(
            dense > sparse * 1.5,
            "coverage 0.15 inked {:.1}% and 0.6 inked {:.1}%",
            sparse * 100.0,
            dense * 100.0
        );
    }

    #[test]
    fn the_preview_shows_the_colours_the_palette_names() {
        // Ties the image to the palette rather than to itself. A preview that
        // ignored a colourway would pass every assertion above.
        let mut s = session();
        let p = params(0.3, 1);
        write_params(&mut s, &p).expect("apply");

        let magenta = pattern_core::Color::new(255, 0, 144);
        s.document.pattern.palette =
            pattern_core::with_colour(&s.document.pattern.palette, 1, magenta);
        let preview = render(&s, &p).expect("render");

        let image = preview_pixels(&preview);
        let found = image.pixels().iter().any(|q| {
            let c = q.demultiply();
            (c.red(), c.green(), c.blue()) == (magenta.r, magenta.g, magenta.b)
        });
        assert!(
            found,
            "slot 1 was set to magenta and none of it reached the preview"
        );
    }

    #[test]
    fn the_preview_reports_the_colours_a_printer_would_charge_for() {
        let mut s = session();
        let p = params(0.3, 1);
        write_params(&mut s, &p).expect("apply");
        let preview = render(&s, &p).expect("render");
        // Ground, fruit and stem: three, not the one a per-placement slot count
        // used to report, and not the palette's length.
        assert_eq!(preview.colours, 3);
    }

    #[test]
    fn a_lattice_is_named_by_what_it_is_not_by_what_was_asked_for() {
        let tile = Vec2::new(240.0, 180.0);
        assert_eq!(lattice_name(&Lattice::block(tile), tile), "block");
        assert_eq!(lattice_name(&Lattice::half_drop(tile), tile), "half-drop");

        // A hand-edited document may carry a lattice that is neither, and
        // labelling it as one would be a lie the UI then acts on.
        let odd = Lattice {
            u: Vec2::new(240.0, 60.0),
            v: Vec2::new(0.0, 180.0),
        };
        assert_eq!(lattice_name(&odd, tile), "custom");
    }

    #[test]
    fn hex_colours_are_read_the_way_a_colour_input_writes_them() {
        assert_eq!(parse_hex("#faf6ee"), Some(Color::new(250, 246, 238)));
        assert_eq!(parse_hex("FAF6EE"), Some(Color::new(250, 246, 238)));
        assert_eq!(parse_hex("  #000000 "), Some(Color::new(0, 0, 0)));
    }

    #[test]
    fn a_colour_that_is_not_a_colour_is_refused_rather_than_guessed() {
        // Silently substituting a colour is the failure mode this whole
        // palette model exists to avoid.
        for bad in ["", "#fff", "#gggggg", "rebeccapurple", "#1234567"] {
            assert_eq!(parse_hex(bad), None, "{bad:?} was accepted");
        }
    }

    #[test]
    fn colours_render_as_css_hex() {
        assert_eq!(hex(&Color::new(250, 246, 238)), "#faf6ee");
        assert_eq!(hex(&Color::new(0, 0, 0)), "#000000");
    }
}
