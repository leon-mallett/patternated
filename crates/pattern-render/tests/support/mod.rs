#![allow(dead_code)]
//! A synthetic motif and pattern, shared by the integration tests.
//!
//! Compiled separately into each test binary, so a helper only one of them uses
//! reads as dead code in the others. Hence the blanket allow: the alternative
//! is inventing a caller in every binary, which makes the tests worse to serve
//! the lint.
//!
//! The motif is drawn in code rather than loaded from a file. That keeps the
//! tests independent of artwork that may be redrawn, keeps binary assets out of
//! the repository's history, and means a golden image can be regenerated on any
//! machine without hunting for the original illustration.

use glam::Vec2;
use motif_ingest::{matte, quantise, trim, MatteMethod, SourceImage};
use pattern_core::{
    scatter, Bands, Color, Lattice, Mirror, Motif, MotifWeight, Palette, Pattern, Placement,
    ScatterParams,
};
use pattern_render::MotifBank;
use tiny_skia::{Paint, PathBuilder, Pixmap, Transform};

pub const TILE: Vec2 = Vec2::new(240.0, 180.0);

/// A cherry: two circles and a stem, asymmetric in both axes.
///
/// The asymmetry is deliberate. A motif symmetric about its centre hides a
/// whole class of tiling bug — a mirrored or half-shifted copy looks identical
/// to a correct one — so the test motif is made lopsided on purpose.
pub fn cherry(size_px: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(size_px, size_px).expect("test pixmap should allocate");
    let s = size_px as f32;

    let mut fruit = Paint::default();
    fruit.set_color_rgba8(200, 30, 45, 255);
    fruit.anti_alias = true;

    let mut stem = Paint::default();
    stem.set_color_rgba8(60, 120, 55, 255);
    stem.anti_alias = true;

    let mut circle = |cx: f32, cy: f32, r: f32, paint: &Paint| {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, r);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    };

    circle(s * 0.36, s * 0.70, s * 0.26, &fruit);
    circle(s * 0.68, s * 0.78, s * 0.20, &fruit);

    let mut pb = PathBuilder::new();
    pb.move_to(s * 0.36, s * 0.50);
    pb.line_to(s * 0.52, s * 0.10);
    pb.line_to(s * 0.62, s * 0.12);
    pb.line_to(s * 0.68, s * 0.60);
    pb.line_to(s * 0.60, s * 0.60);
    pb.line_to(s * 0.56, s * 0.20);
    pb.line_to(s * 0.44, s * 0.52);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(
            &path,
            &stem,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    pixmap
}

/// The test palette. Slot 0 ground, 1 fruit, 2 stem.
pub fn palette() -> Palette {
    Palette::new(vec![
        Color::new(250, 246, 238),
        Color::new(200, 30, 45),
        Color::new(60, 120, 55),
    ])
}

/// The test cherry, put through the real ingest pipeline.
///
/// Deliberately not a shortcut past ingest. Since M3 there is one motif
/// representation in the renderer and one way to produce it, so the tiling and
/// scatter tests exercise matting, trimming and quantisation on every run —
/// which is also how a regression in ingest shows up as a seam rather than as
/// nothing at all.
pub fn bank() -> MotifBank {
    let drawn = cherry(128);
    let mut rgba = Vec::with_capacity(drawn.width() as usize * drawn.height() as usize * 4);
    for px in drawn.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let source = SourceImage::new(drawn.width(), drawn.height(), rgba)
        .expect("drawn motif should be well formed");

    // The drawn cherry already carries alpha, so there is no paper to cut away.
    let matted = matte(&source, MatteMethod::ExistingAlpha);
    let (cropped, _) = trim(&matted, 8).expect("the cherry is not blank");
    let (indexed, _) = quantise(&cropped, &palette()).expect("should quantise");

    let mut bank = MotifBank::new();
    bank.insert(Motif {
        id: "cherry".into(),
        natural_size_mm: Vec2::new(52.0, 52.0),
        indexed,
    });
    bank
}

/// A pattern whose motifs deliberately straddle every tile edge.
///
/// A placement safely inside the tile exercises none of the tiling machinery.
/// These sit on the right edge, the bottom edge and the top-left corner, so a
/// wrong neighbour offset has somewhere to show itself.
pub fn straddling_pattern(lattice: Lattice) -> Pattern {
    Pattern {
        tile_size_mm: TILE,
        lattice,
        placements: vec![
            Placement::new("cherry", Vec2::new(238.0, 60.0)),
            Placement::new("cherry", Vec2::new(90.0, 178.0)),
            Placement::new("cherry", Vec2::new(2.0, 3.0)),
            Placement::new("cherry", Vec2::new(140.0, 95.0)),
        ],
        ground: None,
        palette: palette(),
        seed: 20260920,
    }
}

/// A checked ground with motifs strewn over it.
///
/// Both halves on purpose: bands are laid between the ground and the motifs,
/// so a fixture with only one of them would not exercise the ordering that
/// decision exists for.
///
/// The band widths deliberately do not divide the tile — 17 mm of sequence
/// into 240 and 180 — because the interesting question is whether the engine
/// makes them close.
pub fn checked_pattern(lattice: Lattice, seed: u64) -> Pattern {
    let mut pattern = scattered_pattern(lattice, seed, 0.2);
    pattern.ground = Some(Bands::check(9.0, 8.0, 1, 2).into());
    pattern
}

/// A scattered pattern — what M2 actually produces, as opposed to the
/// hand-placed one the tiling tests use.
///
/// Worth rendering separately: scatter puts motifs wherever the sampler
/// chooses, so edge-straddling happens by accident rather than by the test
/// author's design.
///
/// Density is a parameter because the two callers want different things. The
/// seamlessness tests want it high enough that motifs overlap each other across
/// a tile edge, which is the case where compositing order could diverge between
/// the two render routes. The golden wants a print somebody could actually look
/// at and judge.
pub fn scattered_pattern(lattice: Lattice, seed: u64, density: f32) -> Pattern {
    let params = ScatterParams {
        density,
        rotation_jitter_deg: 30.0,
        scale_range: (0.82, 1.18),
        motif_mix: vec![MotifWeight {
            motif_id: "cherry".into(),
            weight: 1.0,
            natural_size_mm: Vec2::new(26.0, 26.0),
            radius_fraction: 1.0,
        }],
        // Off here: these fixtures exist to stress the tiling with motifs that
        // straddle and overlap edges, which is exactly what the guard removes.
        prevent_overlap: false,
        mirror: Mirror::None,
        on_axis_share: 0.0,
        seed,
    };
    Pattern {
        tile_size_mm: TILE,
        lattice,
        placements: scatter(&params, TILE, &lattice),
        ground: None,
        palette: palette(),
        seed,
    }
}

/// A mirror repeat: the same scatter, designed in part of the tile and
/// reflected into the rest.
///
/// Overlap prevention is left *on* here, unlike the fixture above. A mirror's
/// characteristic failure is a motif astride an axis colliding with its own
/// reflection, and that is precisely what the guard has to stop — turning it
/// off would make the interesting case untestable.
pub fn mirrored_pattern(mirror: Mirror, seed: u64, density: f32) -> Pattern {
    mirrored_pattern_with(mirror, seed, density, 0.0)
}

/// The same, with a share of motifs mounted on the mirror lines.
pub fn mirrored_pattern_with(
    mirror: Mirror,
    seed: u64,
    density: f32,
    on_axis_share: f32,
) -> Pattern {
    let params = ScatterParams {
        density,
        rotation_jitter_deg: 30.0,
        // Drawn at about half size, so that a cell which is half or a quarter
        // of the tile still holds a dozen motifs. A mirror asks more of the
        // motif-to-repeat ratio than a plain scatter does: the cell is
        // smaller, and nothing may come within its own radius of an axis.
        scale_range: (0.40, 0.56),
        motif_mix: vec![MotifWeight {
            motif_id: "cherry".into(),
            weight: 1.0,
            // The size the bank actually draws, unlike the fixture above,
            // which understates it on purpose to force overlaps. Here the
            // sampler's separation has to be the true one or motifs intersect
            // their own reflections, and the symmetry the mirror tests assert
            // is lost to compositing order rather than to any mirror bug.
            natural_size_mm: Vec2::new(52.0, 52.0),
            radius_fraction: 1.0,
        }],
        prevent_overlap: true,
        mirror,
        on_axis_share,
        seed,
    };
    let lattice = Lattice::block(TILE);
    Pattern {
        tile_size_mm: TILE,
        lattice,
        placements: scatter(&params, TILE, &lattice),
        ground: None,
        palette: palette(),
        seed,
    }
}
