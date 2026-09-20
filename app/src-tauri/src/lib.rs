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
}

#[derive(Serialize)]
struct MotifEntry {
    id: String,
    natural_size_mm: [f32; 2],
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
    weights: Vec<(String, f32)>,
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
                weight: scatter
                    .and_then(|s| s.mix.iter().find(|e| e.motif_id == m.id))
                    .map(|e| e.weight)
                    .unwrap_or(1.0),
            })
            .collect(),
        palette: p.palette.colors.iter().map(hex).collect(),
        scattered: scatter.is_some(),
    }
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
    } else {
        "custom".into()
    }
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
        _ => Lattice::half_drop(tile),
    };

    if let Some(spec) = session.document.scatter.as_mut() {
        spec.density = params.density.clamp(0.0, 2.0);
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

    let bank = session.bank.clone();
    session.document.reseed(params.seed, &bank)?;
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

#[tauri::command]
fn save_document(app: State<'_, App>, path: Option<String>) -> Reply<String> {
    let mut guard = app.session.lock().map_err(|e| fail("state", e))?;
    let session = guard.as_mut().ok_or("no document is open")?;
    let target = path
        .map(PathBuf::from)
        .unwrap_or_else(|| session.path.clone());

    // Placements are saved with the document. A scatter regenerates them from
    // the seed, but writing them out means the file still renders identically
    // if the sampler ever changes — the document is the record of what was
    // approved, not a recipe for approximating it.
    let text = serde_json::to_string_pretty(&session.document)
        .map_err(|e| fail("could not serialise the document", e))?;
    std::fs::write(&target, text).map_err(|e| fail("could not write the file", e))?;
    session.path = target.clone();
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
            apply,
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
            preview_width_px: 600,
            cols: 3,
            rows: 3,
        }
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
