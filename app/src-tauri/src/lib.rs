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
    /// a designer their work is safe when it is not, which is the one direction this
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
    /// The striped or checked ground, if there is one, as the sequence
    /// editor needs it, so that reopening a document shows the bands it has
    /// rather than an empty editor.
    ground: GroundSummary,
    /// The garment this repeat's measurements were chosen against, if it has
    /// ever been told. Absent means no opinion, not "adult".
    drawn_for: Option<String>,
    /// Anything worth telling a person about the artwork that still came in.
    ///
    /// These used to go to stderr from inside the library, where this app
    /// could not see them at all, so a designer never learned that most of a
    /// motif had dissolved into the fabric colour, or that a shadow had been
    /// kept as ink.
    notes: Vec<pattern_doc::Note>,
    band_slots: Vec<u8>,
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
    /// How strongly the motifs' near neighbours line up.
    ///
    /// A ranking rather than a verdict: high is worth looking at, low is not a
    /// guarantee. See `pattern_core::alignment`.
    alignment: f32,
    /// The score above which the interface says something.
    alignment_warns_above: f32,
    dpi: f32,
    /// Tiles actually shown, which may exceed what was asked for.
    cols: u32,
    rows: u32,
    /// Whether the window shown is itself a repeating unit: that is, whether
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
/// Put a failure into words for the interface.
///
/// `{e:#}` rather than `{e}`, which is the whole difference between a message
/// a designer can act on and one they cannot. An `anyhow::Error` carries a
/// chain, each layer adding what it was trying to do, and its plain `Display`
/// prints only the outermost link. Loading an SVG full of gradients reported
/// "reading <path>" and stopped there, throwing away the sentence that said it
/// was a gradient and what to do about it.
///
/// The alternate form prints the chain. Types that are not `anyhow` ignore the
/// flag, so this is safe for the plain strings and lock errors also passed
/// here.
fn fail(context: &str, e: impl std::fmt::Display) -> String {
    format!("{context}: {e:#}")
}

#[tauri::command]
fn open_document(app: State<'_, App>, path: String) -> Reply<Opened> {
    let path = PathBuf::from(path);
    let (document, _) =
        Document::load(&path).map_err(|e| fail("could not open that document", e))?;

    // Loaded again through the noting path, so anything the ingest wanted to
    // say reaches the interface instead of a stderr nobody reads.
    let mut notes = Vec::new();
    let bank = document
        .load_motifs_noting(&path, &mut notes)
        .map_err(|e| fail("could not read this document's artwork", e))?;

    let opened = Opened {
        notes,
        ..describe(&path, &document, &bank)
    };
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
        ground: ground_summary(p),
        drawn_for: document.drawn_for.as_ref().map(|d| d.garment.clone()),
        notes: Vec::new(),
        band_slots: p
            .ground
            .as_ref()
            .map(pattern_core::Ground::slots_used)
            .unwrap_or_default(),
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
/// They used to pair an offset with a *lattice*, so that a diagonal was a
/// shifting grid on a half-drop lattice, and that was wrong twice over. The drop
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
/// by anything but a person, and the refusal carries the two facts the
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

/// Start a pattern with no artwork at all.
///
/// A tartan, a stripe or a procedural ground has no motifs in it. The engine
/// never required any, and will render a pattern with an empty bank, but the
/// only way into a document was through artwork, so a ground-only print could
/// be written by hand and not made here.
#[tauri::command]
fn new_plain(app: State<'_, App>) -> Reply<Opened> {
    // The warm off-white a range usually sits on, and the same default
    // `new_from_images` uses for its ground.
    let document = pattern_doc::new_plain(Color::new(250, 246, 238));
    let anchor = PathBuf::from("untitled.json");

    let mut session = Session {
        path: anchor.clone(),
        document,
        bank: MotifBank::new(),
        saved: false,
        written: String::new(),
    };
    session.mark_written();

    let opened = describe(&anchor, &session.document, &session.bank);
    *app.session.lock().map_err(|e| fail("state", e))? = Some(session);
    Ok(opened)
}

/// Start a pattern from image files, with no document in existence.
///
/// The route M4 was missing. Until this existed the only way into the app was
/// a hand-written JSON file, which is the thing the milestone is supposed to
/// remove.
#[tauri::command]
fn new_from_images(app: State<'_, App>, files: Vec<String>, longest_edge_mm: f32) -> Reply<Opened> {
    let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    // Noting, because bringing artwork in is where most of what a designer
    // needs to hear about it gets discovered: a drawing that had to be
    // rendered, a motif that dissolved into the fabric colour.
    let mut notes = Vec::new();
    let document = pattern_doc::new_from_images_noting(
        &paths,
        longest_edge_mm.max(2.0),
        DEFAULT_COLOURS,
        &mut notes,
    )
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
        .load_motifs_noting(&anchor, &mut notes)
        .map_err(|e| fail("could not read those images", e))?;
    session
        .document
        .apply_scatter_public(&session.bank)
        .map_err(|e| fail("could not scatter those motifs", e))?;

    // Each file is read once for the document and once for the bank, so the
    // same thing is worth saying only once.
    notes.dedup_by(|a, b| a.about == b.about && a.says == b.says);
    let opened = Opened {
        notes,
        ..describe(&anchor, &session.document, &session.bank)
    };
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

    // A band sequence lives in the tile and repeats with it, so an oblique
    // lattice steps the stripes at every join. Held here rather than in
    // `apply_layout`, which returns early for a document that places its
    // motifs by hand, and such a document can have a striped ground too.
    if session
        .document
        .pattern
        .ground
        .as_ref()
        .is_some_and(|g| !g.is_empty())
    {
        session.document.pattern.lattice = Lattice::block(tile);
    }

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
/// the motif mix across, because a designer trying layouts should not lose which
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
                // No control for this in the shell yet, so a scatter the app
                // creates gets the sampler's long-standing behaviour. The
                // update path below mutates named fields and leaves this one
                // alone, so a document that arrives carrying a value keeps it
                // and is not quietly reset by moving a slider.
                class_separation: 0.0,
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
    // right edges do not meet, which looks exactly like a broken repeat to
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
        alignment: pattern_core::alignment(
            &pattern.placements,
            pattern.tile_size_mm,
            &pattern.lattice,
        ),
        alignment_warns_above: pattern_core::WORTH_A_LOOK,
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
/// front of a designer while they still have a choice. A 2 m sheet is a
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
            return Err("PNG cannot carry CMYK: choose TIFF, or export in RGB".into());
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
    across_mm: f32,
    length_mm: f32,
}

#[tauri::command]
fn garments() -> Vec<GarmentOption> {
    pattern_render::Garment::standard()
        .into_iter()
        .map(|g| GarmentOption {
            name: g.name,
            across_mm: g.across_mm,
            length_mm: g.length_mm,
        })
        .collect()
}

/// What cutting a design for a garment did.
#[derive(Serialize)]
struct Cut {
    /// What everything was multiplied by. One means the document had never
    /// been told which garment it was for, so there was no factor to apply.
    factor: f32,
    tile_mm: [f32; 2],
    motifs: Vec<MotifEntry>,
    drawn_for: String,
}

/// Cut this design for a garment, scaling the repeat and the motifs together.
///
/// Its own command rather than a parameter on `apply`, because it rewrites
/// lengths that `apply` reads: a scale and a tile size arriving in the same
/// request would race, and the one the designer moved last should win.
#[tauri::command]
fn cut_for(app: State<'_, App>, garment: String) -> Reply<Cut> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;

    let chest = pattern_render::Garment::standard()
        .into_iter()
        .find(|g| g.name == garment)
        .map(|g| g.across_mm)
        .ok_or_else(|| format!("'{garment}' is not a size this tool knows"))?;

    let factor = session
        .document
        .cut_for(&garment, chest)
        .map_err(|e| fail("that size cannot be cut", e))?;

    // The bank holds the sizes a render uses, and they have just moved.
    let bank = session.document.load_motifs(&session.path);
    if let Ok(bank) = bank {
        session.bank = bank;
    }

    let tile = session.document.pattern.tile_size_mm;
    Ok(Cut {
        factor,
        tile_mm: [tile.x, tile.y],
        motifs: motif_entries(&session.bank, session.document.scatter.as_ref()),
        drawn_for: garment,
    })
}

/// A print on a garment, at true scale.
#[derive(Serialize)]
struct MockupView {
    png_base64: String,
    width: u32,
    height: u32,
    /// How many repeats span the garment's width, which is the number this view
    /// exists to make obvious. A chest on a top, a hip on trousers, a brim on
    /// a hat.
    repeats_across: f32,
    across_mm: f32,
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
        across_mm: chosen.across_mm,
        length_mm: chosen.length_mm,
    })
}

/// Preview a pattern in the colours a press can reach.
///
/// Converts to the press's colours and straight back, so anything it cannot
/// reach comes back changed. Seeing a red go flat before the fabric is bought
/// is the entire purpose. "it looked different on screen" is the commonest
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
    // sampler ever changes. A document is the record of what was approved,
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

/// One band as the interface describes it: a width and a colour, or a gap.
#[derive(Deserialize, Serialize, Clone)]
struct GroundBand {
    width_mm: f32,
    /// `None` is a gap, the fabric showing. Expressed as the absence of a
    /// colour rather than as a colour that happens to match the ground, so
    /// that recolouring the fabric takes the gaps with it.
    ///
    /// Note what a gap cannot do: take part in a crossing. Two *threads* are
    /// what give a check its third tone, so a gingham is written as an ink and
    /// a fabric-coloured thread rather than an ink and a gap.
    colour: Option<String>,
}

/// A reaction-diffusion ground, as its panel sends it.
#[derive(Deserialize, Serialize, Clone)]
struct TextureSpec {
    /// One of [`pattern_core::Growth`], kebab-cased.
    growth: String,
    /// Cells across the tile: the pattern's scale, not its quality.
    cells: u32,
    steps: u32,
    seed: u64,
    /// A colour per level, lowest concentration first. `None` is the fabric,
    /// showing through, the same convention as a gap in a band sequence, and
    /// the reason a design can be light-on-dark or dark-on-light without
    /// needing two of every growth.
    levels: Vec<Option<String>>,
}

impl Default for TextureSpec {
    fn default() -> Self {
        let t = pattern_core::Texture::default();
        Self {
            growth: "spots".into(),
            cells: t.cells,
            steps: t.steps,
            seed: t.seed,
            levels: vec![None, Some("#1f3d3a".into())],
        }
    }
}

/// A ground drawn by rule, as its panel sends it.
#[derive(Deserialize, Serialize, Clone)]
struct OrnamentSpec {
    /// One of [`pattern_core::Figure`], kebab-cased.
    figure: String,
    /// How big a cell wants to be. The grid is fitted to the tile, so what
    /// comes back is the nearest whole number of cells to this.
    cell_mm: f32,
    /// How many times a recursive figure divides.
    depth: u32,
    /// Line width, for the figures that are stroked.
    stroke_mm: f32,
    /// How often a cell departs from the strict figure, 0 to 1.
    variation: f32,
    seed: u64,
    /// A colour per level, coarsest first. `None` is the fabric showing.
    levels: Vec<Option<String>>,
}

impl Default for OrnamentSpec {
    fn default() -> Self {
        let o = pattern_core::Ornament::default();
        Self {
            figure: "truchet".into(),
            cell_mm: o.cell_mm,
            depth: o.depth,
            stroke_mm: o.stroke_mm,
            variation: o.variation,
            seed: o.seed,
            levels: vec![Some("#1f3d3a".into())],
        }
    }
}

/// A whole ground, as its editor sends it.
#[derive(Deserialize)]
struct GroundSpec {
    /// "bands" or "texture". Defaulted so that the band editor, which predates
    /// there being a second kind, can go on sending what it always sent.
    #[serde(default = "bands_kind")]
    kind: String,
    #[serde(default)]
    texture: TextureSpec,
    #[serde(default)]
    ornament: OrnamentSpec,
    #[serde(default)]
    bands: Vec<GroundBand>,
    /// Reflect the sequence about its end bands, which is how a tartan sett is
    /// written. See [`pattern_core::Bands::reflected`].
    ///
    /// A one-shot instruction rather than a stored mode: the reflected
    /// sequence is what gets kept, so nothing has to remember whether it has
    /// already been applied.
    #[serde(default)]
    mirrored: bool,
    /// "vertical", "horizontal" or "both".
    direction: String,
    /// How a crossing resolves: "blend", "overprint" or "across".
    #[serde(default = "blend")]
    crossings: String,
}

fn blend() -> String {
    "blend".into()
}

fn bands_kind() -> String {
    "bands".into()
}

/// A ground as the editor needs it back.
///
/// Every field is read off the bands themselves. `mirrored` is deliberately
/// absent: reflecting is applied once and the reflected sequence is what gets
/// stored, so there is no hidden flag that a reopened document would have to
/// guess at, and guessing it wrong would double the reflection on the next
/// edit.
#[derive(Serialize)]
struct GroundSummary {
    kind: String,
    bands: Vec<GroundBand>,
    direction: String,
    crossings: String,
    texture: TextureSpec,
    ornament: OrnamentSpec,
}

fn ground_summary(pattern: &pattern_core::Pattern) -> GroundSummary {
    let blank = || GroundSummary {
        kind: bands_kind(),
        bands: Vec::new(),
        direction: "both".into(),
        crossings: blend(),
        texture: TextureSpec::default(),
        ornament: OrnamentSpec::default(),
    };
    if let Some(o) = pattern
        .ground
        .as_ref()
        .filter(|g| !g.is_empty())
        .and_then(pattern_core::Ground::as_ornament)
    {
        return GroundSummary {
            kind: "ornament".into(),
            ornament: ornament_spec(o, &pattern.palette),
            ..blank()
        };
    }
    if let Some(t) = pattern
        .ground
        .as_ref()
        .filter(|g| !g.is_empty())
        .and_then(pattern_core::Ground::as_texture)
    {
        return GroundSummary {
            kind: "texture".into(),
            texture: texture_spec(t, &pattern.palette),
            ..blank()
        };
    }
    let Some(b) = pattern
        .ground
        .as_ref()
        .filter(|g| !g.is_empty())
        .and_then(pattern_core::Ground::as_bands)
    else {
        return blank();
    };
    let sequence = if b.vertical.is_empty() {
        &b.horizontal
    } else {
        &b.vertical
    };
    GroundSummary {
        kind: bands_kind(),
        texture: TextureSpec::default(),
        ornament: OrnamentSpec::default(),
        bands: sequence
            .iter()
            .map(|band| GroundBand {
                width_mm: band.width_mm,
                colour: (band.slot != 0)
                    .then(|| pattern.palette.resolve(band.slot).map(|c| hex(&c)))
                    .flatten(),
            })
            .collect(),
        direction: match (b.vertical.is_empty(), b.horizontal.is_empty()) {
            (false, true) => "vertical".into(),
            (true, false) => "horizontal".into(),
            _ => "both".into(),
        },
        crossings: if b.crossings.is_empty() {
            "across".into()
        } else {
            blend()
        },
    }
}

/// What to paint where two bands cross.
///
/// **Blend** is the woven answer and the default: half the threads at a
/// crossing are each colour, so the result is their average, and a colour
/// crossing itself comes out as itself, which is what a tartan needs.
///
/// This was got wrong first time round by multiplying instead, on the
/// reasoning that a crossing is a second pass of ink. That is right for ink
/// and wrong for cloth, and the difference is not subtle: a navy sett came out
/// very nearly black, because multiplying two dark colours is far darker than
/// either. It was obvious in the first render and invisible in the model.
///
/// **Overprint** multiplies, which is what a second pass of translucent ink
/// really does, right for a design built from overlapping bands, and the
/// reason it is still offered rather than deleted.
///
/// **Across** adds nothing: the across-band wins, which is the fewest colours
/// to print.
fn crossing_colour(rule: &str, a: Color, b: Color) -> Option<Color> {
    let mix = |x: u8, y: u8| ((u16::from(x) + u16::from(y)) / 2) as u8;
    let mul = |x: u8, y: u8| ((u16::from(x) * u16::from(y)) / 255) as u8;
    match rule {
        "overprint" => Some(Color::new(mul(a.r, b.r), mul(a.g, b.g), mul(a.b, b.b))),
        "across" => None,
        _ => Some(Color::new(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b))),
    }
}

/// Where the ground's slots start.
///
/// They are always the tail of the palette, so a ground can be rebuilt by
/// truncating and appending rather than by tracking indices that would have to
/// stay in step with every other palette operation.
fn ground_base(session: &Session) -> usize {
    let used = session
        .document
        .pattern
        .ground
        .as_ref()
        .map(pattern_core::Ground::slots_used)
        .map_or(0, |s| s.len());
    session.document.pattern.palette.len().saturating_sub(used)
}

/// The grid an ornament was fitted to, for the interface to report.
#[derive(Serialize, Clone, Copy)]
struct FittedGrid {
    cols: u32,
    rows: u32,
    cell_w_mm: f32,
    cell_h_mm: f32,
}

/// What the ground looks like after a change.
#[derive(Serialize)]
struct BandState {
    /// The slots the bands paint with, so the interface can show those
    /// swatches beside the control rather than lost among the motif colours.
    slots: Vec<u8>,
    /// The sequence as the editor should show it, so the interface can stay
    /// out of the business of allocating slots.
    bands: Vec<GroundBand>,
    direction: String,
    crossings: String,
    /// Colours this ground puts on the fabric, crossings included: what a
    /// screen printer would charge for on the ground alone.
    ground_colours: usize,
    palette: Vec<String>,
    lattice: String,
    /// Which kind of ground this is, echoed back so the interface shows the
    /// editor for what was stored rather than for what was last asked for.
    kind: String,
    texture: TextureSpec,
    ornament: OrnamentSpec,
    /// The grid an ornament was actually fitted to, when the ground is one.
    grid: Option<FittedGrid>,
    /// The share of the tile each texture level actually covers.
    ///
    /// Reported rather than assumed because a reaction can leave a level
    /// empty: the thresholds are evenly spaced by concentration, and a field
    /// may simply not reach between two of them. A colour that covers nothing
    /// is one the design is paying a screen for and not printing.
    level_shares: Vec<f32>,
}

/// Put a striped or checked ground under the pattern, or take it away.
///
/// Its own command rather than another parameter because it rewrites the tail
/// of the palette, which `apply` never does.
///
/// The interface sends colours and this allocates the slots, so the sequence
/// editor never has to know that a palette slot exists. The ground's slots are
/// always the tail, so rebuilding is a truncate and an append rather than an
/// index map that would have to stay in step with every other palette
/// operation.
#[tauri::command]
fn set_ground(app: State<'_, App>, spec: GroundSpec) -> Reply<BandState> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    apply_ground(session, &spec).map_err(|e| fail("that ground cannot be printed", e))?;
    ground_state(session, &spec)
}

/// Read a stored texture back as its panel describes it.
fn texture_spec(t: &pattern_core::Texture, palette: &pattern_core::Palette) -> TextureSpec {
    TextureSpec {
        growth: growth_name(t.growth).into(),
        cells: t.cells,
        steps: t.steps,
        seed: t.seed,
        levels: t
            .levels
            .iter()
            .map(|slot| {
                (*slot != pattern_core::GROUND_SLOT)
                    .then(|| palette.resolve(*slot).map(|c| hex(&c)))
                    .flatten()
            })
            .collect(),
    }
}

/// Read a stored ornament back as its panel describes it.
fn ornament_spec(o: &pattern_core::Ornament, palette: &pattern_core::Palette) -> OrnamentSpec {
    OrnamentSpec {
        figure: figure_name(o.figure).into(),
        cell_mm: o.cell_mm,
        depth: o.depth,
        stroke_mm: o.stroke_mm,
        variation: o.variation,
        seed: o.seed,
        levels: o
            .levels
            .iter()
            .map(|slot| {
                (*slot != pattern_core::GROUND_SLOT)
                    .then(|| palette.resolve(*slot).map(|c| hex(&c)))
                    .flatten()
            })
            .collect(),
    }
}

/// The wire name of a figure, and back. Spelled out for the same reason the
/// growths are: the interface's vocabulary belongs here, not in a derive.
fn figure_name(figure: pattern_core::Figure) -> &'static str {
    use pattern_core::Figure::*;
    match figure {
        Truchet => "truchet",
        Weave => "weave",
        Scale => "scale",
        Carpet => "carpet",
        Dust => "dust",
        Cross => "cross",
    }
}

fn figure_named(name: &str) -> Option<pattern_core::Figure> {
    pattern_core::Figure::all()
        .into_iter()
        .find(|f| figure_name(*f) == name)
}

/// The wire name of a growth, and back.
///
/// Spelled out in both directions rather than round-tripped through serde, so
/// that the interface's vocabulary is visible here instead of being an
/// accident of how the enum happens to be derived.
fn growth_name(growth: pattern_core::Growth) -> &'static str {
    use pattern_core::Growth::*;
    match growth {
        Spots => "spots",
        Pebbles => "pebbles",
        Rings => "rings",
        Worms => "worms",
        Maze => "maze",
        Coral => "coral",
    }
}

fn growth_named(name: &str) -> Option<pattern_core::Growth> {
    pattern_core::Growth::all()
        .into_iter()
        .find(|g| growth_name(*g) == name)
}

/// The work behind [`set_ground`], as a plain function so it can be tested
/// without a running app.
fn apply_ground(session: &mut Session, spec: &GroundSpec) -> anyhow::Result<()> {
    let base = ground_base(session);
    let mut colors = session.document.pattern.palette.colors.clone();
    colors.truncate(base);

    if spec.kind == "texture" {
        return apply_texture(session, &spec.texture, colors);
    }
    if spec.kind == "ornament" {
        return apply_ornament(session, &spec.ornament, colors);
    }

    let painted: Vec<GroundBand> = spec
        .bands
        .iter()
        .filter(|b| b.width_mm > 0.0)
        .cloned()
        .collect();
    if painted.is_empty() || !painted.iter().any(|b| b.colour.is_some()) {
        session.document.pattern.ground = None;
        session.document.pattern.palette = pattern_core::Palette::new(colors);
        return Ok(());
    }

    // One slot per distinct colour, in the order the sequence first uses them,
    // so the palette reads the way the editor does.
    let mut slot_of: Vec<(String, u8)> = Vec::new();
    for band in painted.iter().filter_map(|b| b.colour.as_ref()) {
        if slot_of.iter().any(|(c, _)| c == band) {
            continue;
        }
        let colour =
            parse_hex(band).ok_or_else(|| anyhow::anyhow!("'{band}' is not a hex colour"))?;
        slot_of.push((band.clone(), colors.len() as u8));
        colors.push(colour);
    }

    let sequence: Vec<pattern_core::Band> = painted
        .iter()
        .map(|b| {
            let slot = b
                .colour
                .as_ref()
                .and_then(|c| slot_of.iter().find(|(k, _)| k == c))
                .map_or(0, |(_, s)| *s);
            pattern_core::Band::new(b.width_mm, slot)
        })
        .collect();
    let sequence = if spec.mirrored {
        pattern_core::Bands::reflected(&sequence)
    } else {
        sequence
    };

    let (vertical, horizontal) = match spec.direction.as_str() {
        "vertical" => (sequence.clone(), Vec::new()),
        "horizontal" => (Vec::new(), sequence.clone()),
        _ => (sequence.clone(), sequence),
    };

    // A crossing exists only where both directions paint, so a one-way ground
    // needs none of this.
    let mut crossings = Vec::new();
    if !vertical.is_empty() && !horizontal.is_empty() {
        let slots: Vec<u8> = slot_of.iter().map(|(_, s)| *s).collect();
        for (i, a) in slots.iter().enumerate() {
            for b in slots.iter().skip(i) {
                let Some(mixed) =
                    crossing_colour(&spec.crossings, colors[*a as usize], colors[*b as usize])
                else {
                    continue;
                };
                // A colour blended with itself is itself, so it needs no slot
                // of its own, and giving it one would put a duplicate in
                // front of a designer counting colours.
                //
                // Searched only within the ground's own tail. Reaching back
                // into the motif colours would let a crossing borrow a slot
                // from outside the tail, and the tail is what the next rebuild
                // truncates: the ground would then take a motif's colour with
                // it when it was changed.
                let slot = colors[base..].iter().position(|c| *c == mixed).map_or_else(
                    || {
                        colors.push(mixed);
                        colors.len() as u8 - 1
                    },
                    |found| (base + found) as u8,
                );
                crossings.push(pattern_core::Crossing { a: *a, b: *b, slot });
            }
        }
    }

    session.document.pattern.palette = pattern_core::Palette::new(colors);
    session.document.pattern.ground = Some(pattern_core::Ground::Bands(pattern_core::Bands {
        vertical,
        horizontal,
        crossings,
    }));

    // A band sequence lives in the tile and repeats with it, so an oblique
    // lattice steps the stripes at every join.
    let tile = session.document.pattern.tile_size_mm;
    session.document.pattern.lattice = Lattice::block(tile);

    session.document.pattern.validate()?;
    Ok(())
}

/// Grow a texture under the pattern, allocating one slot per distinct level
/// colour at the tail of the palette.
///
/// The same slot discipline as the band editor and for the same reason: the
/// ground's colours are the tail, so rebuilding one is a truncate and an
/// append. The palette arrives already truncated.
fn apply_texture(
    session: &mut Session,
    spec: &TextureSpec,
    colors: Vec<pattern_core::Color>,
) -> anyhow::Result<()> {
    let growth = growth_named(&spec.growth)
        .ok_or_else(|| anyhow::anyhow!("'{}' is not a growth this engine has", spec.growth))?;

    let (colors, levels) = allocate_levels(colors, &spec.levels)?;

    let texture = pattern_core::Texture {
        growth,
        cells: spec.cells.clamp(16, pattern_core::MOST_CELLS),
        steps: spec.steps.clamp(1, pattern_core::MOST_STEPS),
        seed: spec.seed,
        levels,
    };

    session.document.pattern.palette = pattern_core::Palette::new(colors);
    session.document.pattern.ground = (!texture.is_empty()).then(|| texture.into());

    // Grown on a torus, so it meets itself only where the lattice steps by
    // whole tiles.
    let tile = session.document.pattern.tile_size_mm;
    session.document.pattern.lattice = Lattice::block(tile);

    session.document.pattern.validate()?;
    Ok(())
}

/// Draw an ornament under the pattern, allocating one slot per distinct level
/// colour at the tail of the palette.
///
/// The same slot discipline as the other two grounds, and the palette arrives
/// already truncated. The allocation itself is [`allocate_levels`], shared
/// with the texture because both grounds name their colours the same way; what
/// is not shared is what the levels then mean, which is the whole difference
/// between the two.
fn apply_ornament(
    session: &mut Session,
    spec: &OrnamentSpec,
    colors: Vec<pattern_core::Color>,
) -> anyhow::Result<()> {
    let figure = figure_named(&spec.figure)
        .ok_or_else(|| anyhow::anyhow!("'{}' is not a figure this engine draws", spec.figure))?;

    let (colors, levels) = allocate_levels(colors, &spec.levels)?;

    let ornament = pattern_core::Ornament {
        figure,
        cell_mm: spec.cell_mm.max(pattern_core::FINEST_CELL_MM),
        depth: spec.depth.clamp(1, pattern_core::DEEPEST),
        stroke_mm: spec.stroke_mm.max(0.0),
        variation: spec.variation.clamp(0.0, 1.0),
        seed: spec.seed,
        levels,
    };

    session.document.pattern.palette = pattern_core::Palette::new(colors);
    session.document.pattern.ground = (!ornament.is_empty()).then(|| ornament.into());

    // The grid is fitted to the tile and repeats with it.
    let tile = session.document.pattern.tile_size_mm;
    session.document.pattern.lattice = Lattice::block(tile);

    session.document.pattern.validate()?;
    Ok(())
}

/// Turn a list of level colours into slots at the tail of the palette.
///
/// One slot per distinct colour, in the order the levels first use them, so
/// the palette reads the way the editor does. `None` is the fabric and costs
/// nothing.
fn allocate_levels(
    mut colors: Vec<pattern_core::Color>,
    levels: &[Option<String>],
) -> anyhow::Result<(Vec<pattern_core::Color>, Vec<u8>)> {
    let mut slot_of: Vec<(String, u8)> = Vec::new();
    let mut out = Vec::with_capacity(levels.len());
    for level in levels {
        let Some(hex) = level else {
            out.push(pattern_core::GROUND_SLOT);
            continue;
        };
        if let Some((_, slot)) = slot_of.iter().find(|(c, _)| c == hex) {
            out.push(*slot);
            continue;
        }
        let colour =
            parse_hex(hex).ok_or_else(|| anyhow::anyhow!("'{hex}' is not a hex colour"))?;
        let slot = colors.len() as u8;
        colors.push(colour);
        slot_of.push((hex.clone(), slot));
        out.push(slot);
    }
    Ok((colors, out))
}

/// Describe the ground as it now stands.
fn ground_state(session: &Session, spec: &GroundSpec) -> Reply<BandState> {
    let pattern = &session.document.pattern;
    let bands = pattern
        .ground
        .as_ref()
        .and_then(pattern_core::Ground::as_bands);
    let slots = pattern
        .ground
        .as_ref()
        .map(pattern_core::Ground::slots_used)
        .unwrap_or_default();

    // Echoed back from the sequence in the palette rather than from the
    // request, so the editor shows what was stored and not what was asked for.
    let shown: Vec<GroundBand> = bands
        .map(|b| {
            let sequence = if b.vertical.is_empty() {
                &b.horizontal
            } else {
                &b.vertical
            };
            sequence
                .iter()
                .map(|band| GroundBand {
                    width_mm: band.width_mm,
                    colour: (band.slot != 0)
                        .then(|| pattern.palette.resolve(band.slot).map(|c| hex(&c)))
                        .flatten(),
                })
                .collect()
        })
        .unwrap_or_default();

    let texture = pattern
        .ground
        .as_ref()
        .and_then(pattern_core::Ground::as_texture);
    let ornament = pattern
        .ground
        .as_ref()
        .and_then(pattern_core::Ground::as_ornament);
    let level_shares = texture.map_or_else(Vec::new, |t| {
        // Through the renderer's memo, not `Texture::grown`, so that looking
        // at the field costs nothing beyond the render that is about to use
        // the same one.
        let field = pattern_render::laid_ground(
            pattern.ground.as_ref().expect("a texture is a ground"),
            pattern.tile_size_mm.x,
            pattern.tile_size_mm.y,
        );
        let share_of = |slot: u8| field.share_of(slot).unwrap_or(0.0);
        t.levels
            .iter()
            .map(|slot| {
                if *slot == pattern_core::GROUND_SLOT {
                    // Slot 0 is absence, and a field records absence as
                    // absence rather than as slot 0, so asking after its share
                    // would report nothing wherever the fabric shows.
                    //
                    // Summed over the *distinct* painted slots: a colour used
                    // for two levels covers its share once, not twice.
                    1.0 - t.slots_used().iter().map(|s| share_of(*s)).sum::<f32>()
                } else {
                    share_of(*slot)
                }
            })
            .collect()
    });

    Ok(BandState {
        slots: slots.clone(),
        bands: shown,
        direction: spec.direction.clone(),
        crossings: spec.crossings.clone(),
        ground_colours: slots.len(),
        palette: pattern.palette.colors.iter().map(hex).collect(),
        lattice: lattice_name(&pattern.lattice, pattern.tile_size_mm),
        kind: match (texture.is_some(), ornament.is_some()) {
            (true, _) => "texture".into(),
            (_, true) => "ornament".into(),
            _ => spec.kind.clone(),
        },
        texture: texture.map_or_else(
            || spec.texture.clone(),
            |t| texture_spec(t, &pattern.palette),
        ),
        ornament: ornament.map_or_else(
            || spec.ornament.clone(),
            |o| ornament_spec(o, &pattern.palette),
        ),
        // The cell size asked for is a wish and the grid is what was drawn, so
        // the interface says which it got rather than echoing the request back
        // as though it had been honoured.
        grid: ornament.map(|o| {
            let g = o.grid_on(pattern.tile_size_mm.x, pattern.tile_size_mm.y);
            FittedGrid {
                cols: g.cols,
                rows: g.rows,
                cell_w_mm: g.cell_w,
                cell_h_mm: g.cell_h,
            }
        }),
        level_shares,
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
            new_plain,
            cut_for,
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
            set_ground,
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
    /// covered without needing a window, which matters because a window is the
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
        // to make one, so the only route in was hand-writing JSON, which is
        // the thing the milestone exists to remove.
        let files = artwork();
        if !files.iter().all(|f| f.exists()) {
            // The generated artwork is not committed, so a fresh clone cannot
            // run this. Say so: a silent skip still reports ok.
            eprintln!("SKIP: generated artwork is not committed; see examples/generated-motifs/.");
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
    fn every_dialogue_the_interface_opens_is_one_it_is_allowed_to() {
        // A gap between two layers that no test on either side can see. The
        // component tests mock the dialog plugin, so they will happily drive a
        // dialogue the shipped app is not permitted to open; the Rust tests
        // never reach the front end at all. What is left is the manifest, and
        // the manifest is the thing nobody remembers to edit.
        //
        // This was live: `confirm` arrived with the Close button and the
        // capability file had not been touched since the shell was built, so
        // closing a document with unsaved work failed with a permission error,
        // and only with unsaved work, which is the path a designer meets
        // exactly when they have something to lose.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let manifest = std::fs::read_to_string(root.join("src-tauri/capabilities/default.json"))
            .expect("the capability file should be readable");

        let mut wanted: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(root.join("src")).expect("the front end") {
            let path = entry.expect("entry").path();
            let is_source = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "ts" || e == "tsx");
            let is_test = path.to_string_lossy().contains(".test.");
            if !is_source || is_test {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("readable");
            wanted.extend(dialogs_used(&source));
        }
        assert!(
            wanted.contains(&"confirm".to_string()),
            "this test found no dialogue calls at all, so it is not reading the source it              thinks it is"
        );

        for api in &wanted {
            let permission = format!("dialog:allow-{api}");
            assert!(
                manifest.contains(&permission),
                "the interface opens a '{api}' dialogue and the capability file does not                  grant '{permission}'. In the shipped app that fails at the moment it is                  used, which no mocked test can reach."
            );
        }
    }

    /// Which dialogue APIs a source file imports from the Tauri plugin.
    ///
    /// Read from the import rather than from the call sites, because the names
    /// are aliased where they would collide, as `confirm as confirmDialog`, and
    /// the import is where the true name survives.
    fn dialogs_used(source: &str) -> Vec<String> {
        const FROM: &str = "} from \"@tauri-apps/plugin-dialog\";";
        let Some(end) = source.find(FROM) else {
            return Vec::new();
        };
        let Some(start) = source[..end].rfind("import {") else {
            return Vec::new();
        };
        source[start + "import {".len()..end]
            .split(',')
            .filter_map(|name| {
                let first = name.split_whitespace().next()?;
                (!first.is_empty()).then(|| first.to_string())
            })
            .collect()
    }

    #[test]
    fn a_failure_reaches_the_interface_with_its_cause_attached() {
        // Reported from the app: loading an SVG of gradients said only
        // "reading <path>" and stopped there. The sentence that said it was a
        // gradient, and what to do about it, was the innermost link of an
        // `anyhow` chain, and plain `Display` prints only the outermost.
        //
        // The difference is between a message a designer can act on and one
        // that tells them a file exists.
        let deep = anyhow::anyhow!("it uses a gradient, which has no single colour")
            .context("reading the drawing")
            .context("ingesting the artwork");

        let said = fail("could not start a pattern", deep);
        assert!(
            said.contains("gradient"),
            "the cause was dropped on the way out: {said}"
        );
        assert!(said.contains("could not start a pattern"), "{said}");
        assert!(said.contains("reading the drawing"), "{said}");
    }

    #[test]
    fn a_message_with_no_chain_is_not_decorated() {
        // The same formatter carries plain strings and lock failures, which
        // must come through as themselves.
        assert_eq!(
            fail("state", "the lock was poisoned"),
            "state: the lock was poisoned"
        );
    }

    #[test]
    fn a_motif_share_moves_the_mix_and_not_the_count() {
        // What the share control does, and, just as important, what it does
        // not. Reported from the app as the slider doing nothing: a designer
        // raised it expecting more motifs on the fabric and the number did not
        // move, because it never can. Coverage decides how many; this decides
        // which.
        let files = artwork();
        if !files.iter().all(|f| f.exists()) {
            eprintln!("SKIP: generated artwork is not committed; see examples/generated-motifs/.");
            return;
        }
        let mut s = session();
        let document = pattern_doc::new_from_images(&files, 60.0, DEFAULT_COLOURS)
            .expect("two motifs should ingest");
        s.document = document;
        s.bank = s.document.load_motifs(&s.path).expect("motifs");

        let ids: Vec<String> = s.document.motifs.iter().map(|m| m.id.clone()).collect();
        assert_eq!(ids.len(), 2, "this test needs two motifs to have a mix");

        let counts = |s: &Session| {
            ids.iter()
                .map(|id| {
                    s.document
                        .pattern
                        .placements
                        .iter()
                        .filter(|p| &p.motif_id == id)
                        .count()
                })
                .collect::<Vec<_>>()
        };
        let even = |w: f32| ids.iter().map(|id| (id.clone(), w)).collect::<Vec<_>>();

        let mut p = params(0.3, 1);
        // Both the same size, so nothing but the mix can move the count.
        p.sizes = ids.iter().map(|id| (id.clone(), 50.0)).collect();
        p.weights = even(1.0);
        write_params(&mut s, &p).expect("apply");
        let balanced = counts(&s);
        let total = s.document.pattern.placements.len();

        // Raising every share by the same amount is no change at all: they are
        // relative to each other and to nothing else.
        p.weights = even(8.0);
        write_params(&mut s, &p).expect("apply");
        assert_eq!(
            counts(&s),
            balanced,
            "raising every share together should change nothing"
        );

        // Raising one alone moves the mix, and leaves the total where it was.
        p.weights = vec![(ids[0].clone(), 6.0), (ids[1].clone(), 1.0)];
        write_params(&mut s, &p).expect("apply");
        let skewed = counts(&s);
        assert!(
            skewed[0] > balanced[0] && skewed[1] < balanced[1],
            "a six-to-one share gave {skewed:?} where even gave {balanced:?}"
        );
        assert_eq!(
            s.document.pattern.placements.len(),
            total,
            "the share decides which motif, not how many"
        );
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

    /// A ground spec, the way the sequence editor sends one.
    fn spec(bands: &[(f32, Option<&str>)], direction: &str) -> GroundSpec {
        GroundSpec {
            bands: bands
                .iter()
                .map(|(w, c)| GroundBand {
                    width_mm: *w,
                    colour: c.map(str::to_string),
                })
                .collect(),
            mirrored: false,
            direction: direction.into(),
            crossings: blend(),
            kind: bands_kind(),
            texture: TextureSpec::default(),
            ornament: OrnamentSpec::default(),
        }
    }

    fn texture_spec_of(growth: &str, levels: &[Option<&str>]) -> GroundSpec {
        GroundSpec {
            kind: "texture".into(),
            texture: TextureSpec {
                growth: growth.into(),
                cells: 48,
                steps: 400,
                seed: 4,
                levels: levels.iter().map(|c| c.map(str::to_string)).collect(),
            },
            ..spec(&[], "both")
        }
    }

    fn ornament_spec_of(figure: &str, levels: &[Option<&str>]) -> GroundSpec {
        GroundSpec {
            kind: "ornament".into(),
            ornament: OrnamentSpec {
                figure: figure.into(),
                cell_mm: 20.0,
                depth: 3,
                stroke_mm: 4.0,
                variation: 0.2,
                seed: 6,
                levels: levels.iter().map(|c| c.map(str::to_string)).collect(),
            },
            ..spec(&[], "both")
        }
    }

    #[test]
    fn an_ornament_takes_slots_at_the_tail_like_any_other_ground() {
        let mut s = session();
        let before = s.document.pattern.palette.len();
        apply_ground(&mut s, &ornament_spec_of("truchet", &[Some("#204080")])).expect("ornament");

        let palette = &s.document.pattern.palette;
        assert_eq!(palette.len(), before + 1);
        assert_eq!(
            palette.resolve(before as u8).map(|c| hex(&c)).as_deref(),
            Some("#204080")
        );

        // Rebuilt, not accumulated.
        apply_ground(&mut s, &ornament_spec_of("carpet", &[Some("#802040")])).expect("ornament");
        assert_eq!(s.document.pattern.palette.len(), before + 1);
    }

    #[test]
    fn an_ornament_comes_back_as_the_panel_sent_it() {
        let mut s = session();
        apply_ground(
            &mut s,
            &ornament_spec_of("cross", &[Some("#204080"), None, Some("#0b1020")]),
        )
        .expect("ornament");

        let summary = ground_summary(&s.document.pattern);
        assert_eq!(summary.kind, "ornament");
        assert_eq!(summary.ornament.figure, "cross");
        assert_eq!(summary.ornament.depth, 3);
        assert_eq!(summary.ornament.seed, 6);
        assert_eq!(
            summary.ornament.levels,
            vec![
                Some("#204080".to_string()),
                None,
                Some("#0b1020".to_string())
            ],
            "the fabric comes back as absence, not as a colour"
        );
        assert!(summary.bands.is_empty());
    }

    #[test]
    fn the_interface_is_told_the_grid_it_got_rather_than_the_one_it_asked_for() {
        // The cell size is a wish: the grid is fitted to the tile, so a
        // request that does not divide comes back as the nearest one that
        // does. Echoing the request would let a designer believe a 7 mm cell
        // was drawn when a 7.5 mm one was.
        let mut s = session();
        let mut request = ornament_spec_of("truchet", &[Some("#204080")]);
        request.ornament.cell_mm = 7.0;
        apply_ground(&mut s, &request).expect("ornament");

        let state = ground_state(&s, &request).expect("state");
        let grid = state.grid.expect("an ornament has a grid");
        let tile = s.document.pattern.tile_size_mm;
        assert!(
            (grid.cols as f32 * grid.cell_w_mm - tile.x).abs() < 1e-3,
            "{} cells of {} mm is not {} mm",
            grid.cols,
            grid.cell_w_mm,
            tile.x
        );
        assert!((grid.rows as f32 * grid.cell_h_mm - tile.y).abs() < 1e-3);
        assert_ne!(
            grid.cell_w_mm, 7.0,
            "7 mm does not divide this tile, so it should not have been drawn at 7 mm"
        );
    }

    #[test]
    fn a_bricked_figure_is_given_an_even_number_of_rows_through_the_app() {
        // The constraint that only one figure has, checked where a designer
        // would meet it rather than only in the engine.
        let mut s = session();
        let mut request = ornament_spec_of("scale", &[Some("#204080")]);
        request.ornament.cell_mm = 26.0;
        apply_ground(&mut s, &request).expect("ornament");

        let grid = ground_state(&s, &request)
            .expect("state")
            .grid
            .expect("a grid");
        assert_eq!(grid.rows % 2, 0, "a clamshell got {} rows", grid.rows);
    }

    #[test]
    fn an_unknown_figure_is_refused_rather_than_guessed() {
        let mut s = session();
        let e = apply_ground(&mut s, &ornament_spec_of("basketweave", &[Some("#204080")]))
            .expect_err("should refuse");
        assert!(e.to_string().contains("basketweave"), "{e}");
    }

    #[test]
    fn switching_between_the_three_kinds_takes_the_other_two_away() {
        // A pattern has one ground. Leaving the previous kind behind would
        // have the palette paying for colours nothing draws.
        let mut s = session();
        let plain = s.document.pattern.palette.len();

        for request in [
            spec(&[(10.0, Some("#3c78a0")), (10.0, Some("#e8eef2"))], "both"),
            texture_spec_of("spots", &[None, Some("#204080")]),
            ornament_spec_of("truchet", &[Some("#204080")]),
        ] {
            apply_ground(&mut s, &request).expect("ground");
        }

        assert!(s
            .document
            .pattern
            .ground
            .as_ref()
            .unwrap()
            .as_ornament()
            .is_some());
        assert!(s
            .document
            .pattern
            .ground
            .as_ref()
            .unwrap()
            .as_bands()
            .is_none());
        assert!(s
            .document
            .pattern
            .ground
            .as_ref()
            .unwrap()
            .as_texture()
            .is_none());
        assert_eq!(
            s.document.pattern.palette.len(),
            plain + 1,
            "only the ornament's own colour should be left at the tail"
        );
    }

    #[test]
    fn an_ornament_survives_a_change_of_colour_count() {
        // The third sibling of the resize tests. It passes because the
        // renumbering lives on the enum: writing this one needed no change to
        // `set_palette_size` at all.
        let mut s = session();
        apply_ground(&mut s, &ornament_spec_of("weave", &[Some("#3c78a0")])).expect("ornament");
        let ink = s
            .document
            .pattern
            .palette
            .resolve(s.document.pattern.ground.as_ref().unwrap().slots_used()[0])
            .expect("the ink");

        s.document
            .set_palette_size(16, &s.path)
            .expect("should re-derive");
        s.bank = s.document.load_motifs(&s.path).expect("re-quantise");

        let after = s.document.pattern.ground.as_ref().expect("survived");
        assert_eq!(
            s.document.pattern.palette.resolve(after.slots_used()[0]),
            Some(ink),
            "the ornament kept its colour"
        );
        s.document.pattern.validate().expect("still renderable");
    }

    #[test]
    fn a_texture_takes_slots_at_the_tail_like_any_other_ground() {
        // The slot discipline is what lets either kind of ground be rebuilt by
        // truncating and appending. If a texture allocated anywhere else, the
        // next edit would take a motif's colour with it.
        let mut s = session();
        let before = s.document.pattern.palette.len();
        apply_ground(&mut s, &texture_spec_of("spots", &[None, Some("#204080")])).expect("texture");

        let palette = &s.document.pattern.palette;
        assert_eq!(palette.len(), before + 1, "one colour, one slot");
        assert_eq!(
            palette.resolve(before as u8).map(|c| hex(&c)).as_deref(),
            Some("#204080"),
            "and it is the last slot"
        );

        // Rebuilt, not accumulated: a second edit replaces the tail.
        apply_ground(&mut s, &texture_spec_of("maze", &[None, Some("#802040")])).expect("texture");
        assert_eq!(s.document.pattern.palette.len(), before + 1);
    }

    #[test]
    fn a_texture_comes_back_as_the_panel_sent_it() {
        let mut s = session();
        apply_ground(
            &mut s,
            &texture_spec_of("rings", &[None, Some("#204080"), Some("#0b1020")]),
        )
        .expect("texture");

        let summary = ground_summary(&s.document.pattern);
        assert_eq!(summary.kind, "texture");
        assert_eq!(summary.texture.growth, "rings");
        assert_eq!(summary.texture.cells, 48);
        assert_eq!(summary.texture.seed, 4);
        assert_eq!(
            summary.texture.levels,
            vec![
                None,
                Some("#204080".to_string()),
                Some("#0b1020".to_string())
            ],
            "the fabric comes back as absence, not as a colour"
        );
        assert!(summary.bands.is_empty(), "a texture has no band sequence");
    }

    #[test]
    fn switching_kind_takes_the_other_ground_away() {
        // Otherwise a pattern would carry a band sequence it no longer shows,
        // and the palette would keep paying for its colours.
        let mut s = session();
        apply_ground(
            &mut s,
            &spec(&[(10.0, Some("#3c78a0")), (10.0, Some("#e8eef2"))], "both"),
        )
        .expect("bands");
        let with_bands = s.document.pattern.palette.len();

        apply_ground(&mut s, &texture_spec_of("spots", &[None, Some("#204080")])).expect("texture");
        assert!(s
            .document
            .pattern
            .ground
            .as_ref()
            .unwrap()
            .as_bands()
            .is_none());
        assert!(
            s.document.pattern.palette.len() < with_bands,
            "the check's three tones should have gone with it"
        );
    }

    #[test]
    fn a_texture_that_paints_nothing_is_no_ground_at_all() {
        let mut s = session();
        apply_ground(&mut s, &texture_spec_of("spots", &[None, None])).expect("texture");
        assert!(s.document.pattern.ground.is_none());
    }

    #[test]
    fn an_unknown_growth_is_refused_rather_than_guessed() {
        let mut s = session();
        let e = apply_ground(
            &mut s,
            &texture_spec_of("paisley", &[None, Some("#204080")]),
        )
        .expect_err("should refuse");
        assert!(e.to_string().contains("paisley"), "{e}");
    }

    #[test]
    fn a_texture_reports_what_each_level_actually_covered() {
        // The number a designer needs and cannot get from the parameters: a
        // level can fall between two concentrations the field never reaches,
        // and then it is a screen paid for and not printed.
        let mut s = session();
        let request = texture_spec_of("spots", &[None, Some("#204080")]);
        apply_ground(&mut s, &request).expect("texture");
        let state = ground_state(&s, &request).expect("state");

        assert_eq!(state.level_shares.len(), 2);
        let total: f32 = state.level_shares.iter().sum();
        assert!(
            (total - 1.0).abs() < 1e-3,
            "the levels should account for the whole tile, not {total}"
        );
        assert!(
            state.level_shares[1] > 0.01,
            "spots covered {:.3} of the tile",
            state.level_shares[1]
        );
    }

    #[test]
    fn a_texture_moves_the_repeat_onto_a_block_lattice() {
        // Grown on a torus, so its left edge is its right edge. A half-drop
        // says those edges do not meet, and the engine refuses the pair.
        let mut s = session();
        s.document.pattern.lattice = Lattice::half_drop(s.document.pattern.tile_size_mm);
        apply_ground(&mut s, &texture_spec_of("worms", &[None, Some("#204080")])).expect("texture");
        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "block"
        );
    }

    #[test]
    fn a_ground_is_read_back_as_the_editor_needs_it() {
        // Read off the bands rather than stored, so a reopened document shows
        // the sequence it has rather than an empty editor, and a hand-edited
        // one is described honestly.
        let mut s = session();
        for (direction, expected) in [
            ("vertical", "vertical"),
            ("horizontal", "horizontal"),
            ("both", "both"),
        ] {
            apply_ground(
                &mut s,
                &spec(
                    &[(10.0, Some("#204080")), (10.0, Some("#faf6ee"))],
                    direction,
                ),
            )
            .expect("ground");

            let summary = ground_summary(&s.document.pattern);
            assert_eq!(summary.direction, expected);
            assert_eq!(summary.bands.len(), 2, "both threads come back");
            assert_eq!(summary.bands[0].colour.as_deref(), Some("#204080"));
        }
    }

    #[test]
    fn a_check_gets_its_third_tone_from_two_threads() {
        // Two colours, three tones, the thing no arrangement of motifs can
        // do. It needs two *threads* rather than a thread and a gap: a gap has
        // nothing to blend with, so an ink and a gap give only ink and fabric.
        let mut s = session();
        apply_ground(
            &mut s,
            &spec(&[(10.0, Some("#3c78a0")), (10.0, Some("#e8eef2"))], "both"),
        )
        .expect("ground");

        let bands = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_bands)
            .cloned()
            .expect("bands");
        let painted: Vec<u8> = bands.vertical.iter().map(|b| b.slot).collect();
        let (a, b) = (painted[0], painted[1]);

        let tone = |x: u8, y: u8| bands.resolve(Some(x), Some(y)).expect("a crossing");
        let tones: std::collections::HashSet<u8> =
            [tone(a, a), tone(a, b), tone(b, b)].into_iter().collect();
        assert_eq!(tones.len(), 3, "three tones from two threads");

        // A thread crossing itself is itself, which is what keeps a tartan's
        // colours rather than driving them towards black.
        assert_eq!(tone(a, a), a);
        assert_eq!(tone(b, b), b);
    }

    #[test]
    fn a_thread_crossing_itself_stays_itself() {
        // The mistake this replaced: crossings were multiplied, on the
        // reasoning that a crossing is a second pass of ink. Right for ink and
        // wrong for cloth: a navy sett came out very nearly black, which the
        // first render showed and the model did not.
        let navy = pattern_core::Color::new(0x1b, 0x2a, 0x4a);
        assert_eq!(crossing_colour("blend", navy, navy), Some(navy));

        let multiplied = crossing_colour("overprint", navy, navy).expect("overprint");
        assert!(
            multiplied.r < navy.r / 2 && multiplied.b < navy.b / 2,
            "multiplying a dark colour by itself is far darker: {multiplied:?}"
        );

        assert_eq!(crossing_colour("across", navy, navy), None);
    }

    #[test]
    fn a_tartan_gets_a_crossing_for_every_pair() {
        // Four thread colours cross in ten ways, and a printed tartan really
        // does carry the colours those crossings make. A count that reported
        // only the threads would be quoting for the wrong job.
        let mut s = session();
        let threads = [
            (24.0, Some("#1b2a4a")),
            (4.0, Some("#0b0b0b")),
            (12.0, Some("#1f4020")),
            (3.0, Some("#c8a020")),
        ];
        apply_ground(&mut s, &spec(&threads, "both")).expect("ground");

        let bands = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_bands)
            .cloned()
            .expect("bands");
        assert_eq!(bands.crossings.len(), 10, "four threads cross in ten ways");
        // Four threads, plus a colour only where two *different* ones meet: a
        // thread crossing itself is itself and needs no slot of its own.
        assert_eq!(bands.slots_used().len(), 4 + 6);

        for c in &bands.crossings {
            assert_eq!(bands.resolve(Some(c.a), Some(c.b)), Some(c.slot));
        }
    }

    #[test]
    fn a_one_way_ground_has_no_crossings_to_derive() {
        // Nothing crosses a stripe, so the palette should not grow for it.
        let mut s = session();
        apply_ground(
            &mut s,
            &spec(&[(10.0, Some("#3c78a0")), (10.0, None)], "vertical"),
        )
        .expect("ground");
        let bands = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_bands)
            .cloned()
            .expect("bands");
        assert!(bands.crossings.is_empty());
        assert_eq!(bands.slots_used().len(), 1);
    }

    #[test]
    fn one_slot_per_colour_however_often_it_repeats() {
        // A tartan names the same thread many times. Allocating a slot per
        // mention would fill the palette with duplicates and make recolouring
        // one band leave the others behind.
        let mut s = session();
        let repeated = [
            (20.0, Some("#1b2a4a")),
            (4.0, Some("#0b0b0b")),
            (20.0, Some("#1b2a4a")),
            (4.0, Some("#0b0b0b")),
        ];
        apply_ground(&mut s, &spec(&repeated, "vertical")).expect("ground");
        assert_eq!(
            s.document
                .pattern
                .ground
                .as_ref()
                .unwrap()
                .slots_used()
                .len(),
            2
        );
    }

    #[test]
    fn rebuilding_a_ground_does_not_leave_its_old_slots_behind() {
        // The ground owns the tail of the palette, so changing it truncates
        // and appends. Without that the palette would grow on every drag of a
        // width.
        let mut s = session();
        let base = s.document.pattern.palette.len();
        apply_ground(
            &mut s,
            &spec(&[(10.0, Some("#3c78a0")), (10.0, Some("#faf6ee"))], "both"),
        )
        .expect("ground");
        let after_first = s.document.pattern.palette.len();
        assert!(after_first > base);

        apply_ground(
            &mut s,
            &spec(&[(14.0, Some("#a03c78")), (6.0, Some("#eeeeee"))], "both"),
        )
        .expect("ground");
        assert_eq!(s.document.pattern.palette.len(), after_first);

        // And taking it away gives the slots back.
        apply_ground(&mut s, &spec(&[(10.0, None)], "both")).expect("ground");
        assert!(s.document.pattern.ground.is_none());
        assert_eq!(s.document.pattern.palette.len(), base);
    }

    #[test]
    fn a_mirrored_sett_lays_down_the_reflected_sequence() {
        let mut s = session();
        let mut spec = spec(
            &[
                (20.0, Some("#1b2a4a")),
                (4.0, Some("#0b0b0b")),
                (12.0, Some("#1f4020")),
            ],
            "both",
        );
        spec.mirrored = true;
        apply_ground(&mut s, &spec).expect("ground");

        let bands = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_bands)
            .cloned()
            .expect("bands");
        let widths: Vec<f32> = bands.vertical.iter().map(|b| b.width_mm).collect();
        assert_eq!(widths, vec![20.0, 4.0, 12.0, 4.0], "pivots are not doubled");
    }

    #[test]
    fn a_banded_ground_puts_the_repeat_back_on_a_block_lattice() {
        // A sequence lives in the tile and repeats with it, so a half-drop
        // steps the stripes at every join. Held in write_params rather than in
        // apply_layout, because a document that places its motifs by hand
        // never reaches apply_layout and can still have a striped ground.
        let mut s = session();
        apply_ground(
            &mut s,
            &spec(&[(12.0, Some("#3c78a0")), (12.0, None)], "vertical"),
        )
        .expect("ground");

        let mut p = params(0.3, 1);
        p.lattice = "half-drop".into();
        write_params(&mut s, &p).expect("apply");

        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "block"
        );
        s.document.pattern.validate().expect("should be renderable");
    }

    #[test]
    fn a_ground_counts_toward_what_a_printer_charges_for() {
        // Ink on the fabric is ink on the fabric. A colour count that omitted
        // the stripes would be quoting for the wrong job.
        let mut s = session();
        let motifs: Vec<pattern_core::Motif> = s.bank.motifs().into_iter().cloned().collect();
        let plain = pattern_core::resolved_color_count(&s.document.pattern, &motifs);

        apply_ground(
            &mut s,
            &spec(&[(10.0, Some("#3c78a0")), (10.0, Some("#d8e4ec"))], "both"),
        )
        .expect("ground");
        let checked = pattern_core::resolved_color_count(&s.document.pattern, &motifs);
        assert_eq!(
            checked,
            plain + 3,
            "two threads and the tone where they cross"
        );
    }

    #[test]
    fn a_ground_survives_a_change_of_colour_count() {
        // The palette is replaced wholesale by a resize, and the ground's
        // slots live at its tail. Without carrying them across, a striped
        // document would come back referring to slots that no longer exist and
        // stop rendering at all, which is a worse failure than the one the
        // colour count was changed to fix.
        let mut s = session();
        apply_ground(
            &mut s,
            &spec(&[(10.0, Some("#3c78a0")), (10.0, None)], "both"),
        )
        .expect("ground");

        let before = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_bands)
            .cloned()
            .expect("bands");
        let band_colour = s
            .document
            .pattern
            .palette
            .resolve(before.slots_used()[0])
            .expect("band colour");

        s.document
            .set_palette_size(16, &s.path)
            .expect("should re-derive");
        s.bank = s.document.load_motifs(&s.path).expect("re-quantise");

        let after = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_bands)
            .cloned()
            .expect("bands survived");
        assert_eq!(after.vertical.len(), before.vertical.len());
        assert_eq!(after.crossings.len(), before.crossings.len());
        assert_eq!(
            s.document.pattern.palette.resolve(after.slots_used()[0]),
            Some(band_colour),
            "the band kept its colour"
        );
        s.document
            .pattern
            .validate()
            .expect("still renderable after the resize");
    }

    #[test]
    fn a_grown_ground_survives_a_change_of_colour_count_too() {
        // The sibling of the test above, and the reason the renumbering lives
        // on the enum rather than in one arm of a match: the colours are
        // carried across for every kind of ground, so a kind that is not
        // renumbered keeps indices pointing into the *motif* colours of the new
        // palette and comes back in somebody else's colour.
        let mut s = session();
        apply_ground(
            &mut s,
            &GroundSpec {
                kind: "texture".into(),
                texture: TextureSpec {
                    growth: "spots".into(),
                    cells: 48,
                    steps: 200,
                    seed: 2,
                    levels: vec![None, Some("#3c78a0".into())],
                },
                ..spec(&[], "both")
            },
        )
        .expect("texture");

        let before = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_texture)
            .cloned()
            .expect("texture");
        let ink = s
            .document
            .pattern
            .palette
            .resolve(before.slots_used()[0])
            .expect("the ink");

        s.document
            .set_palette_size(16, &s.path)
            .expect("should re-derive");
        s.bank = s.document.load_motifs(&s.path).expect("re-quantise");

        let after = s
            .document
            .pattern
            .ground
            .as_ref()
            .and_then(pattern_core::Ground::as_texture)
            .cloned()
            .expect("texture survived");
        assert_eq!(after.levels.len(), before.levels.len());
        assert_eq!(
            after.levels[0],
            pattern_core::GROUND_SLOT,
            "the fabric level is absence and does not move"
        );
        assert_eq!(
            s.document.pattern.palette.resolve(after.slots_used()[0]),
            Some(ink),
            "the texture kept its colour"
        );
        s.document
            .pattern
            .validate()
            .expect("still renderable after the resize");
    }

    /// A document with no artwork, as the start screen now makes one.
    fn plain_session() -> Session {
        let mut s = Session {
            path: PathBuf::from("untitled.json"),
            document: pattern_doc::new_plain(pattern_core::Color::new(250, 246, 238)),
            bank: MotifBank::new(),
            saved: false,
            written: String::new(),
        };
        s.mark_written();
        s
    }

    #[test]
    fn a_pattern_with_no_artwork_still_renders() {
        // The whole point of the front door: a tartan has no motifs in it, and
        // every path through the app has to survive an empty bank rather than
        // assuming somebody brought PNGs.
        let mut s = plain_session();
        apply_ground(
            &mut s,
            &spec(&[(10.0, Some("#1b2a4a")), (10.0, Some("#1f4020"))], "both"),
        )
        .expect("ground");

        let p = Params {
            weights: vec![],
            sizes: vec![],
            ..params(0.3, 1)
        };
        write_params(&mut s, &p).expect("apply");
        let preview = render(&s, &p).expect("render");

        assert_eq!(preview.placements, 0);
        assert!(
            inked_share(&preview, &s) > 0.5,
            "the ground should cover it"
        );
        // Two threads and the tone where they cross, plus the fabric.
        assert_eq!(preview.colours, 4);
    }

    #[test]
    fn a_plain_document_starts_with_nothing_but_its_fabric() {
        let s = plain_session();
        assert!(s.document.motifs.is_empty());
        assert!(s.document.scatter.is_none());
        assert!(s.document.pattern.ground.is_none());
        assert_eq!(s.document.pattern.palette.len(), 1);
        // Block, because a ground repeats by the tile and there is nothing
        // strewn that would want an oblique one.
        assert_eq!(
            lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
            "block"
        );
        s.document
            .pattern
            .validate()
            .expect("renderable from the start");
    }

    #[test]
    fn a_plain_document_has_something_to_lose_from_the_moment_it_exists() {
        // It has never been written anywhere, so closing discards it, the
        // same rule as a document started from artwork.
        assert!(plain_session().unsaved_changes());
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
        // does. A document carrying one, from a hand-written file or from
        // the days when a grid preset chose one, would otherwise be left with
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
        // entirely correct in a screenshot. It is simply the wrong picture.
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
