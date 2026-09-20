//! A synthetic motif and pattern, shared by the integration tests.
//!
//! The motif is drawn in code rather than loaded from a file. That keeps the
//! tests independent of artwork that may be redrawn, keeps binary assets out of
//! the repository's history, and means a golden image can be regenerated on any
//! machine without hunting for the original illustration.

use glam::Vec2;
use pattern_core::{Color, Lattice, Palette, Pattern, Placement};
use pattern_render::{MotifBank, RasterMotif};
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

pub fn bank() -> MotifBank {
    let mut bank = MotifBank::new();
    bank.insert(
        "cherry",
        RasterMotif::new(cherry(128), Vec2::new(52.0, 52.0)),
    );
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
        palette: Palette::new(vec![
            Color::new(250, 246, 238),
            Color::new(200, 30, 45),
            Color::new(60, 120, 55),
        ]),
        seed: 20260920,
    }
}
