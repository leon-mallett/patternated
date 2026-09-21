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
use pattern_core::{Color, Lattice};
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
    scattered: bool,
    layout: String,
    prevent_overlap: bool,
    grid_rows: u32,
    grid_cols: u32,
}

#[derive(Serialize)]
struct MotifEntry {
    id: String,
    natural_size_mm: [f32; 2],
    /// Longest edge in millimetres, which is the handle a designer actually
    /// wants: aspect ratio comes from the artwork and is never adjusted.
    longest_edge_mm: f32,
    weight: f32,
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
    #[serde(default)]
    prevent_overlap: bool,
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
    *app.session.lock().map_err(|e| fail("state", e))? = Some(Session {
        path,
        document,
        bank,
        saved: true,
    });
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
        motifs: bank
            .motifs()
            .into_iter()
            .map(|m| MotifEntry {
                id: m.id.clone(),
                natural_size_mm: [m.natural_size_mm.x, m.natural_size_mm.y],
                longest_edge_mm: m.natural_size_mm.x.max(m.natural_size_mm.y),
                weight: scatter
                    .and_then(|s| s.mix.iter().find(|e| e.motif_id == m.id))
                    .map(|e| e.weight)
                    .unwrap_or(1.0),
            })
            .collect(),
        palette: p.palette.colors.iter().map(hex).collect(),
        scattered: scatter.is_some() || document.grid.is_some(),
        layout: document
            .grid
            .as_ref()
            .map(|g| preset_name(g.row_offset))
            .unwrap_or_else(|| "tossed".into()),
        prevent_overlap: scatter.map(|s| s.prevent_overlap).unwrap_or(true),
        grid_rows: document.grid.as_ref().map(|g| g.rows).unwrap_or(3),
        grid_cols: document.grid.as_ref().map(|g| g.cols).unwrap_or(3),
    }
}

/// The classic repeats, as a designer names them.
///
/// Each is a row offset plus a lattice, both of which the engine already had —
/// a square grid is a block lattice with no offset, brickwork is the same grid
/// with alternate rows shifted half a cell, and a diagonal is rows that walk
/// steadily across. Naming them is most of the work, because the names are how
/// anybody asks for them.
const PRESETS: &[(&str, f32, &str)] = &[
    ("square", 0.0, "block"),
    ("brick", 0.5, "brick"),
    ("half-drop", 0.5, "half-drop"),
    ("diagonal", 1.0 / 3.0, "half-drop"),
    ("gentle-diagonal", 0.25, "quarter-drop"),
];

fn preset_name(row_offset: f32) -> String {
    PRESETS
        .iter()
        .find(|(_, offset, _)| (offset - row_offset).abs() < 1e-3)
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

/// Start a pattern from image files, with no document in existence.
///
/// The route M4 was missing. Until this existed the only way into the app was
/// a hand-written JSON file, which is the thing the milestone is supposed to
/// remove.
#[tauri::command]
fn new_from_images(app: State<'_, App>, files: Vec<String>, longest_edge_mm: f32) -> Reply<Opened> {
    let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let document = pattern_doc::new_from_images(&paths, longest_edge_mm.max(2.0), 6)
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
    let wants_grid = params.layout != "tossed";
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
        let (_, row_offset, lattice) = PRESETS
            .iter()
            .find(|(name, _, _)| *name == params.layout)
            .copied()
            .unwrap_or(("square", 0.0, "block"));

        let tile = session.document.pattern.tile_size_mm;
        session.document.pattern.lattice = match lattice {
            "block" => Lattice::block(tile),
            "brick" => Lattice::brick(tile),
            "quarter-drop" => Lattice::quarter_drop(tile),
            _ => Lattice::half_drop(tile),
        };
        session.document.scatter = None;
        session.document.grid = Some(pattern_doc::GridSpec {
            rows: params.grid_rows.clamp(1, 24),
            cols: params.grid_cols.clamp(1, 24),
            row_offset,
            rotation_jitter_deg: params.rotation_jitter_deg,
            scale_range: (params.scale_range[0], params.scale_range[1]),
            mix,
            seed: params.seed,
        });
    } else if session.document.scatter.is_none() {
        session.document.grid = None;
        session.document.scatter = Some(pattern_doc::ScatterSpec {
            density: params.density.clamp(0.0, 2.0),
            prevent_overlap: params.prevent_overlap,
            rotation_jitter_deg: params.rotation_jitter_deg,
            scale_range: (params.scale_range[0], params.scale_range[1]),
            mix,
            seed: params.seed,
        });
    }
    session.document.pattern.placements.clear();
    Ok(())
}

fn render(session: &Session, params: &Params) -> anyhow::Result<Preview> {
    let pattern = &session.document.pattern;
    let cols = params.cols.max(1);
    let rows = params.rows.max(1);

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
    })
}

#[tauri::command]
fn export_png(app: State<'_, App>, path: String, dpi: f32, cols: u32, rows: u32) -> Reply<String> {
    let guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_ref().ok_or("no document is open")?;
    let path = PathBuf::from(path);

    let pixmap = if cols <= 1 && rows <= 1 {
        pattern_render::render_tile(&session.document.pattern, &session.bank, dpi)
    } else {
        pattern_render::render_field(
            &session.document.pattern,
            &session.bank,
            dpi,
            cols.max(1),
            rows.max(1),
        )
    }
    .map_err(|e| fail("could not render for export", e))?;

    pattern_doc::write_png(&path, &pixmap, dpi).map_err(|e| fail("could not write the file", e))?;
    Ok(path.display().to_string())
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
    Ok(target.display().to_string())
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
            export_png,
            save_document,
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
        Session {
            path,
            document,
            bank,
            saved: true,
        }
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
        assert_eq!(
            s.document.pattern.placements.len(),
            (p.grid_rows * p.grid_cols) as usize
        );

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
    fn each_preset_picks_its_own_repeat() {
        let mut s = session();
        let mut p = params(0.3, 1);
        for (preset, expected) in [
            ("square", "block"),
            ("brick", "brick"),
            ("gentle-diagonal", "quarter-drop"),
            ("diagonal", "half-drop"),
        ] {
            p.layout = preset.into();
            write_params(&mut s, &p).expect("apply");
            assert_eq!(
                lattice_name(&s.document.pattern.lattice, s.document.pattern.tile_size_mm),
                expected,
                "preset {preset}"
            );
        }
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
    fn colours_render_as_css_hex() {
        assert_eq!(hex(&Color::new(250, 246, 238)), "#faf6ee");
        assert_eq!(hex(&Color::new(0, 0, 0)), "#000000");
    }
}
