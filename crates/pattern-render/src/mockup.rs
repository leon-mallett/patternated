//! Showing a print on a garment, at true scale.
//!
//! Worth building despite looking cosmetic. Seeing a repeat at its real size
//! against a real garment is how a scale error gets caught before fabric is
//! bought, and it is the fastest way for two people to agree on a design
//! without printing anything.
//!
//! The garment is a silhouette rather than a photograph. A photograph would
//! need the pattern warped onto folds, which is a different and much larger
//! piece of work, and it would make the one thing this exists for — the
//! relationship between the repeat and the body — harder to judge rather than
//! easier.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tiny_skia::{
    FillRule, FilterQuality, Paint, Path, PathBuilder, Pattern as PatternShader, Pixmap,
    SpreadMode, Stroke, Transform,
};

use crate::{render_field, MotifBank, RenderError};
use pattern_core::Pattern;

/// A garment, by the measurements a size chart actually lists.
///
/// Flat measurements in millimetres — the width across the chest with the
/// garment laid flat, not the body circumference, because that is how garments
/// are specified and how fabric is cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Garment {
    pub name: String,
    /// Width across the chest, laid flat.
    pub chest_mm: f32,
    /// Length from the high point of the shoulder to the hem.
    pub length_mm: f32,
    /// Width across the shoulders.
    pub shoulder_mm: f32,
    /// Sleeve length from the shoulder seam.
    pub sleeve_mm: f32,
}

impl Garment {
    /// The sizes a range is actually cut in.
    ///
    /// Approximate standard figures rather than any one manufacturer's chart,
    /// and that is worth knowing before anyone quotes them: they are right
    /// enough to judge a repeat against and not a substitute for a real spec.
    ///
    /// The two children's sizes are here for the reason the spec gives: a
    /// 240 mm repeat reads completely differently on an adult shirt and on a
    /// three-year-old's, and the only way to see that is side by side.
    #[must_use]
    pub fn standard() -> Vec<Garment> {
        vec![
            Garment::tee("T-shirt, age 3–4", 330.0, 440.0, 290.0, 120.0),
            Garment::tee("T-shirt, age 9–10", 400.0, 580.0, 350.0, 150.0),
            Garment::tee("T-shirt, adult S", 460.0, 710.0, 430.0, 190.0),
            Garment::tee("T-shirt, adult M", 510.0, 740.0, 460.0, 200.0),
            Garment::tee("T-shirt, adult L", 560.0, 760.0, 490.0, 210.0),
            Garment {
                name: "Cushion, 45 cm".into(),
                chest_mm: 450.0,
                length_mm: 450.0,
                shoulder_mm: 450.0,
                sleeve_mm: 0.0,
            },
        ]
    }

    fn tee(name: &str, chest: f32, length: f32, shoulder: f32, sleeve: f32) -> Self {
        Self {
            name: name.into(),
            chest_mm: chest,
            length_mm: length,
            shoulder_mm: shoulder,
            sleeve_mm: sleeve,
        }
    }

    /// Whether this is a garment with sleeves or a flat panel.
    fn has_sleeves(&self) -> bool {
        self.sleeve_mm > 1.0
    }

    /// Overall extent of the silhouette, in millimetres.
    #[must_use]
    pub fn extent_mm(&self) -> Vec2 {
        if self.has_sleeves() {
            // Sleeves reach out past the chest on both sides.
            Vec2::new(self.chest_mm + self.sleeve_mm * 1.7, self.length_mm)
        } else {
            Vec2::new(self.chest_mm, self.length_mm)
        }
    }

    /// How many repeats fit across the chest.
    ///
    /// The number this whole view exists to make obvious. Two repeats across a
    /// chest is a bold print; eleven is a texture; a fifth of one is a placement
    /// print that happens to be tiled.
    #[must_use]
    pub fn repeats_across(&self, tile_width_mm: f32) -> f32 {
        if tile_width_mm <= 0.0 {
            0.0
        } else {
            self.chest_mm / tile_width_mm
        }
    }

    /// The silhouette, in millimetres, with the origin at its top-left.
    fn outline(&self) -> Option<Path> {
        let mut pb = PathBuilder::new();
        let extent = self.extent_mm();

        if !self.has_sleeves() {
            pb.push_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, extent.x, extent.y)?);
            return pb.finish();
        }

        let centre = extent.x / 2.0;
        let body_half = self.chest_mm / 2.0;
        let shoulder_half = self.shoulder_mm / 2.0;
        let sleeve_reach = self.sleeve_mm * 1.7 / 2.0;
        // Proportions of the length rather than fixed millimetres, so the shape
        // stays recognisable from a three-year-old's to an adult L.
        let armhole_y = self.length_mm * 0.30;
        let sleeve_hem_y = self.length_mm * 0.26;
        let neck_dip = self.length_mm * 0.07;
        let neck_half = self.shoulder_mm * 0.19;

        // Down the left side from the shoulder, out along the sleeve, and back.
        pb.move_to(centre - shoulder_half, 0.0);
        pb.line_to(centre - shoulder_half - sleeve_reach, sleeve_hem_y * 0.45);
        pb.line_to(centre - shoulder_half - sleeve_reach * 0.86, sleeve_hem_y);
        pb.line_to(centre - body_half, armhole_y);
        pb.line_to(centre - body_half * 0.97, self.length_mm);
        pb.line_to(centre + body_half * 0.97, self.length_mm);
        pb.line_to(centre + body_half, armhole_y);
        pb.line_to(centre + shoulder_half + sleeve_reach * 0.86, sleeve_hem_y);
        pb.line_to(centre + shoulder_half + sleeve_reach, sleeve_hem_y * 0.45);
        pb.line_to(centre + shoulder_half, 0.0);
        // The neckline, scooped rather than notched so it reads as a garment.
        pb.line_to(centre + neck_half, 0.0);
        pb.quad_to(centre, neck_dip, centre - neck_half, 0.0);
        pb.close();
        pb.finish()
    }
}

/// A garment with the print on it.
pub struct Mockup {
    pub pixmap: Pixmap,
    /// How many repeats span the chest.
    pub repeats_across: f32,
    /// Physical size of the garment, for the caption.
    pub extent_mm: Vec2,
}

/// Render a pattern onto a garment at true scale.
///
/// `width_px` is how wide the picture should come back; the resolution follows
/// from the garment's real width, which is what keeps the scale honest. Nothing
/// here is allowed to choose a size that makes the print look good.
pub fn render_mockup(
    pattern: &Pattern,
    bank: &MotifBank,
    garment: &Garment,
    width_px: u32,
) -> Result<Mockup, RenderError> {
    let extent = garment.extent_mm();
    if extent.x <= 0.0 || extent.y <= 0.0 {
        return Err(RenderError::Unallocatable {
            width: extent.x as i32,
            height: extent.y as i32,
        });
    }

    // Margin enough for the garment to sit in rather than fill the frame.
    const MARGIN: f32 = 0.06;
    let scale = width_px as f32 * (1.0 - MARGIN * 2.0) / extent.x;
    let dpi = scale * pattern_core::MM_PER_INCH;
    let height_px = ((extent.y * scale) / (1.0 - MARGIN * 2.0)).round() as u32;

    let mut canvas =
        Pixmap::new(width_px.max(32), height_px.max(32)).ok_or(RenderError::Unallocatable {
            width: width_px as i32,
            height: height_px as i32,
        })?;

    // The swatch used to fill the garment must itself be a repeating unit,
    // because a shader tiles it as a plain rectangle. A three-by-three window
    // of a half-drop is not one, and the garment would show a seam every three
    // repeats — the same trap as a pre-repeated export.
    let (cols, rows) = pattern.lattice.repeating_window(pattern.tile_size_mm, 1, 1);
    let swatch = render_field(pattern, bank, dpi, cols, rows)?;

    let outline = garment.outline().ok_or(RenderError::Unallocatable {
        width: width_px as i32,
        height: height_px as i32,
    })?;
    let offset = Vec2::new(width_px as f32 * MARGIN, height_px as f32 * MARGIN);
    let placed = outline
        .transform(Transform::from_translate(offset.x, offset.y).pre_scale(scale, scale))
        .ok_or(RenderError::Unallocatable {
            width: width_px as i32,
            height: height_px as i32,
        })?;

    // A plain ground behind the garment, so the print is judged against
    // nothing in particular.
    let mut backdrop = Paint::default();
    backdrop.set_color_rgba8(242, 240, 236, 255);
    canvas.fill_rect(
        tiny_skia::Rect::from_xywh(0.0, 0.0, width_px as f32, height_px as f32).ok_or(
            RenderError::Unallocatable {
                width: width_px as i32,
                height: height_px as i32,
            },
        )?,
        &backdrop,
        Transform::identity(),
        None,
    );

    let cloth = Paint {
        anti_alias: true,
        shader: PatternShader::new(
            swatch.as_ref(),
            SpreadMode::Repeat,
            FilterQuality::Bilinear,
            1.0,
            Transform::identity(),
        ),
        ..Default::default()
    };
    canvas.fill_path(
        &placed,
        &cloth,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    // A hairline edge, so a pale print does not dissolve into the backdrop.
    let mut edge = Paint {
        anti_alias: true,
        ..Default::default()
    };
    edge.set_color_rgba8(150, 146, 140, 255);
    canvas.stroke_path(
        &placed,
        &edge,
        &Stroke {
            width: (width_px as f32 / 500.0).max(1.0),
            ..Default::default()
        },
        Transform::identity(),
        None,
    );

    Ok(Mockup {
        pixmap: canvas,
        repeats_across: garment.repeats_across(pattern.tile_size_mm.x),
        extent_mm: extent,
    })
}
