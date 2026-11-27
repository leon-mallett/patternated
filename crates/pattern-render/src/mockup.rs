//! Showing a print on a garment, at true scale.
//!
//! Worth building despite looking cosmetic. Seeing a repeat at its real size
//! against a real garment is how a scale error gets caught before fabric is
//! bought, and it is the fastest way for two people to agree on a design
//! without printing anything.
//!
//! The garment is a silhouette rather than a photograph. A photograph would
//! need the pattern warped onto folds, which is a different and much larger
//! piece of work, and it would make the one thing this exists for, the
//! relationship between the repeat and the body, harder to judge rather than
//! easier.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tiny_skia::{
    FillRule, FilterQuality, Paint, Path, PathBuilder, PathSegment, Pattern as PatternShader,
    Pixmap, SpreadMode, Stroke, Transform,
};

use crate::{render_field, MotifBank, RenderError};
use pattern_core::Pattern;

/// What shape a garment is, and the measurements only that shape has.
///
/// Was inferred from whether `sleeve_mm` was greater than one, which worked
/// for exactly two shapes and could not express a third. A hat has no sleeve
/// length to be zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum Cut {
    /// A flat square of cloth: a cushion, a swatch.
    Panel,
    /// A body with sleeves.
    Top { shoulder_mm: f32, sleeve_mm: f32 },
    /// A waist and two legs. Shorts are the same cut, shorter.
    ///
    /// `rise_mm` is the waist to the crotch, which is what decides how much of
    /// the print is one unbroken field and how much is split between legs.
    Legs { rise_mm: f32 },
    /// Legs with a bib and straps over them.
    Dungarees { rise_mm: f32, bib_mm: f32 },
    /// A crown and a brim.
    Hat { crown_mm: f32 },
    /// A rectangle with two handles.
    Tote { handle_mm: f32 },
    /// A top and trousers together, in the one cloth, which is what makes it
    /// a pair of pyjamas rather than two garments that happen to match.
    Pyjamas {
        shoulder_mm: f32,
        sleeve_mm: f32,
        rise_mm: f32,
    },
}

/// A garment, by the measurements a size chart actually lists.
///
/// Flat measurements in millimetres, the width across the garment laid flat,
/// not the body circumference, because that is how garments are specified and
/// how fabric is cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Garment {
    pub name: String,
    /// The width the print reads across, laid flat.
    ///
    /// A chest on a top, a hip on trousers, a brim on a hat, the bag on a
    /// tote. Named for what it is *for* rather than for one garment's anatomy,
    /// because it was called `chest_mm` and a hat has no chest, and because
    /// this is the number a repeat is scaled against, so it has to mean the
    /// same thing for every shape.
    pub across_mm: f32,
    /// Top to hem, not counting anything that rises above it like a handle.
    pub length_mm: f32,
    pub cut: Cut,
}

impl Garment {
    /// The sizes a range is actually cut in.
    ///
    /// Approximate standard figures rather than any one manufacturer's chart,
    /// and that is worth knowing before anyone quotes them: they are right
    /// enough to judge a repeat against and not a substitute for a real spec.
    ///
    /// The children's sizes are here for the reason the spec gives: a 240 mm
    /// repeat reads completely differently on an adult shirt and on a
    /// three-year-old's, and the only way to see that is side by side.
    #[must_use]
    pub fn standard() -> Vec<Garment> {
        vec![
            Self::top("T-shirt, age 3–4", 330.0, 440.0, 290.0, 120.0),
            Self::top("T-shirt, age 9–10", 400.0, 580.0, 350.0, 150.0),
            Self::top("T-shirt, adult S", 460.0, 710.0, 430.0, 190.0),
            Self::top("T-shirt, adult M", 510.0, 740.0, 460.0, 200.0),
            Self::top("T-shirt, adult L", 560.0, 760.0, 490.0, 210.0),
            Self::top("Long sleeve, adult M", 510.0, 740.0, 460.0, 580.0),
            Self::legs("Shorts, age 3–4", 260.0, 260.0, 190.0),
            Self::legs("Shorts, adult M", 420.0, 430.0, 290.0),
            Self::legs("Trousers, age 3–4", 260.0, 540.0, 190.0),
            Self::legs("Trousers, age 9–10", 320.0, 760.0, 230.0),
            Self::legs("Trousers, adult M", 420.0, 1030.0, 290.0),
            Garment {
                name: "Dungarees, age 3–4".into(),
                across_mm: 260.0,
                length_mm: 700.0,
                cut: Cut::Dungarees {
                    rise_mm: 190.0,
                    bib_mm: 160.0,
                },
            },
            Garment {
                name: "Pyjamas, age 3–4".into(),
                across_mm: 330.0,
                length_mm: 540.0,
                cut: Cut::Pyjamas {
                    shoulder_mm: 290.0,
                    sleeve_mm: 330.0,
                    rise_mm: 190.0,
                },
            },
            Garment {
                name: "Pyjamas, age 9–10".into(),
                across_mm: 400.0,
                length_mm: 760.0,
                cut: Cut::Pyjamas {
                    shoulder_mm: 350.0,
                    sleeve_mm: 430.0,
                    rise_mm: 230.0,
                },
            },
            Garment {
                name: "Bucket hat, child".into(),
                across_mm: 290.0,
                length_mm: 190.0,
                cut: Cut::Hat { crown_mm: 175.0 },
            },
            Garment {
                name: "Bucket hat, adult".into(),
                across_mm: 340.0,
                length_mm: 220.0,
                cut: Cut::Hat { crown_mm: 205.0 },
            },
            Garment {
                name: "Tote bag".into(),
                across_mm: 380.0,
                length_mm: 420.0,
                cut: Cut::Tote { handle_mm: 180.0 },
            },
            Garment {
                name: "Cushion, 45 cm".into(),
                across_mm: 450.0,
                length_mm: 450.0,
                cut: Cut::Panel,
            },
        ]
    }

    fn top(name: &str, chest: f32, length: f32, shoulder: f32, sleeve: f32) -> Self {
        Self {
            name: name.into(),
            across_mm: chest,
            length_mm: length,
            cut: Cut::Top {
                shoulder_mm: shoulder,
                sleeve_mm: sleeve,
            },
        }
    }

    fn legs(name: &str, hip: f32, length: f32, rise: f32) -> Self {
        Self {
            name: name.into(),
            across_mm: hip,
            length_mm: length,
            cut: Cut::Legs { rise_mm: rise },
        }
    }

    /// Overall extent of the silhouette, in millimetres.
    ///
    /// Wider than `across_mm` wherever something reaches past the body, such as
    /// sleeves or a pair of pyjamas standing side by side, and taller than
    /// `length_mm` where a handle rises above the hem.
    #[must_use]
    pub fn extent_mm(&self) -> Vec2 {
        match &self.cut {
            Cut::Panel | Cut::Legs { .. } | Cut::Dungarees { .. } | Cut::Hat { .. } => {
                Vec2::new(self.across_mm, self.length_mm)
            }
            Cut::Top {
                shoulder_mm,
                sleeve_mm,
            } => top_extent(self.across_mm, self.length_mm, *shoulder_mm, *sleeve_mm),
            Cut::Tote { handle_mm } => Vec2::new(self.across_mm, self.length_mm + handle_mm),
            Cut::Pyjamas {
                shoulder_mm,
                sleeve_mm,
                rise_mm,
            } => {
                let top = top_extent(self.across_mm, self.length_mm, *shoulder_mm, *sleeve_mm);
                let (trousers, trouser_length) = self.pyjama_trousers(*rise_mm);
                Vec2::new(top.x + PIECE_GAP + trousers, top.y.max(trouser_length))
            }
        }
    }

    /// The trousers that go with a pyjama top: narrower than the chest, and
    /// longer than the top, as they are on any pair.
    fn pyjama_trousers(&self, _rise_mm: f32) -> (f32, f32) {
        (self.across_mm * 0.78, self.length_mm * 1.28)
    }

    /// How many repeats fit across the garment.
    ///
    /// The number this whole view exists to make obvious. Two repeats across a
    /// chest is a bold print; eleven is a texture; a fifth of one is a
    /// placement print that happens to be tiled.
    #[must_use]
    pub fn repeats_across(&self, tile_width_mm: f32) -> f32 {
        if tile_width_mm <= 0.0 {
            0.0
        } else {
            self.across_mm / tile_width_mm
        }
    }

    /// The box the silhouette actually occupies, in millimetres.
    ///
    /// Public so a test can check it against [`Self::extent_mm`]: the extent
    /// decides the canvas and the outline is drawn into it, so anything the
    /// outline reaches past is clipped silently.
    ///
    /// Tight, not `Path::bounds`, which is the box around the *control*
    /// points and is wider than the curve wherever there is one. A brim
    /// bending toward the hem has a control point well past it, and a check
    /// against the loose bound would refuse a shape that fits.
    #[must_use]
    pub fn outline_bounds(&self) -> Option<tiny_skia::Rect> {
        self.outline()?.compute_tight_bounds()
    }

    /// The silhouette as a coarse occupancy grid, `side` by `side`, stretched
    /// into its own frame. Row by row from the top left; empty if it will not
    /// draw.
    ///
    /// A shape measure rather than a size one, so a child's and an adult's of
    /// the same cut agree and two different cuts do not. Public so a test can
    /// say that two cuts draw different shapes, which is about the only thing
    /// a test can say about a silhouette: the rest is judged by eye.
    ///
    /// Was the share of the frame covered, a single number, which is a weak
    /// way to tell two shapes apart and in the end told a pair of pyjamas from
    /// a hat not at all: both cover 0.5643 of their own frames. The area of a
    /// bounding box came before that and is one for a pair of legs and one for
    /// a pair of dungarees.
    #[must_use]
    pub fn occupancy(&self, side: u32) -> Vec<bool> {
        let extent = self.extent_mm();
        let (Some(outline), Some(mut canvas)) = (self.outline(), Pixmap::new(side, side)) else {
            return Vec::new();
        };
        let Some(placed) = outline.transform(Transform::from_scale(
            side as f32 / extent.x.max(1e-3),
            side as f32 / extent.y.max(1e-3),
        )) else {
            return Vec::new();
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        canvas.fill_path(
            &placed,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        canvas.pixels().iter().map(|p| p.alpha() > 127).collect()
    }

    /// The silhouette as SVG path data, in millimetres, with the origin at its
    /// top-left and [`Self::extent_mm`] for the viewBox.
    ///
    /// Here because the shape is wanted outside a raster. Something that tiles
    /// the cloth itself and clips it to this keeps the repeat an actual
    /// repeat, which a picture of a garment cannot be; and it takes the
    /// geometry from the one place that has it rather than drawing a second
    /// pyjama by eye, which would then disagree with the one the app shows.
    ///
    /// Several contours where a cut has several pieces, so the fill rule is
    /// the caller's to choose. Non-zero is what [`render_mockup`] uses and
    /// what these shapes are drawn for. Even-odd is also safe here and is what
    /// a caller punching the garment out of a backdrop needs: a neck or a
    /// crotch is a notch in its own contour rather than a contour of its own,
    /// and no two pieces of a set overlap.
    #[must_use]
    pub fn outline_svg_path(&self) -> Option<String> {
        let mut d = String::new();
        for segment in self.outline()?.segments() {
            if !d.is_empty() {
                d.push(' ');
            }
            match segment {
                PathSegment::MoveTo(p) => d.push_str(&format!("M{} {}", mm(p.x), mm(p.y))),
                PathSegment::LineTo(p) => d.push_str(&format!("L{} {}", mm(p.x), mm(p.y))),
                PathSegment::QuadTo(c, p) => {
                    d.push_str(&format!("Q{} {} {} {}", mm(c.x), mm(c.y), mm(p.x), mm(p.y)));
                }
                PathSegment::CubicTo(a, b, p) => d.push_str(&format!(
                    "C{} {} {} {} {} {}",
                    mm(a.x),
                    mm(a.y),
                    mm(b.x),
                    mm(b.y),
                    mm(p.x),
                    mm(p.y)
                )),
                PathSegment::Close => d.push('Z'),
            }
        }
        Some(d)
    }

    /// The silhouette, in millimetres, with the origin at its top-left.
    ///
    /// May be several contours: a tote's handles, a pair of pyjamas. They are
    /// filled and stroked as one shape, which is what puts the same cloth
    /// across both halves of a pair rather than two prints that happen to
    /// match.
    fn outline(&self) -> Option<Path> {
        let mut pb = PathBuilder::new();
        let extent = self.extent_mm();
        match &self.cut {
            Cut::Panel => {
                pb.push_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, extent.x, extent.y)?);
            }
            Cut::Top {
                shoulder_mm,
                sleeve_mm,
            } => top_contour(
                &mut pb,
                0.0,
                extent.x,
                self.across_mm,
                self.length_mm,
                *shoulder_mm,
                *sleeve_mm,
            ),
            Cut::Legs { rise_mm } => {
                legs_contour(&mut pb, 0.0, self.across_mm, self.length_mm, *rise_mm);
            }
            Cut::Dungarees { rise_mm, bib_mm } => {
                dungarees_contour(&mut pb, self.across_mm, self.length_mm, *rise_mm, *bib_mm);
            }
            Cut::Hat { crown_mm } => {
                hat_contour(&mut pb, self.across_mm, self.length_mm, *crown_mm)
            }
            Cut::Tote { handle_mm } => {
                tote_contour(&mut pb, self.across_mm, self.length_mm, *handle_mm);
            }
            Cut::Pyjamas {
                shoulder_mm,
                sleeve_mm,
                rise_mm,
            } => {
                let top_width =
                    top_extent(self.across_mm, self.length_mm, *shoulder_mm, *sleeve_mm).x;
                top_contour(
                    &mut pb,
                    0.0,
                    top_width,
                    self.across_mm,
                    self.length_mm,
                    *shoulder_mm,
                    *sleeve_mm,
                );
                let (trousers, trouser_length) = self.pyjama_trousers(*rise_mm);
                legs_contour(
                    &mut pb,
                    top_width + PIECE_GAP,
                    trousers,
                    trouser_length,
                    *rise_mm,
                );
            }
        }
        pb.finish()
    }
}

/// A millimetre as a path writes it: two decimals at most, and no trailing
/// zeros, so the geometry reads as the round numbers it mostly is.
fn mm(v: f32) -> String {
    let fixed = format!("{v:.2}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    // `-0` comes out of a signed zero and means nothing to a reader.
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Where the underarm sits, as a share of the body's length.
const ARMHOLE_Y: f32 = 0.30;

/// How far the shoulder point sits below the neck, as a share of the length.
///
/// A level shoulder line is half of what made the old sleeve read as a wing:
/// the arm left the body at a corner rather than off a slope.
const SHOULDER_SLOPE: f32 = 0.045;

/// The angles a sleeve hangs at, shortest to longest, in degrees below level.
const SLEEVE_ANGLE: (f32, f32) = (25.0, 70.0);

/// The sleeve lengths those two angles belong to, as a share of the body.
const SLEEVE_RUN: (f32, f32) = (0.2, 0.7);

/// How much of the armhole's width the cuff keeps, over that same run.
///
/// A cap sleeve finishes near the bicep and barely narrows; a full-length one
/// finishes at a wrist.
const CUFF_OF_ARMHOLE: (f32, f32) = (0.85, 0.5);

/// How a sleeve is posed and how it tapers.
///
/// The angle is a choice about the picture rather than a measurement: a flat
/// silhouette can only be honest about the sleeve's length, and an arm has to
/// be put somewhere. It is put further down the longer the sleeve is, from a
/// cap sleeve standing almost out to a full-length one lying beside the body.
/// That is how a flat lay is photographed, and it is also what keeps a
/// long-sleeved top from becoming a T-pose three times as wide as it is tall,
/// which would leave the body this view exists to judge a repeat against as a
/// narrow strip down the middle.
///
/// The sleeve's own length is drawn undistorted either way. What used to be
/// capped was the reach, with the remainder becoming drop and the shape
/// tapering to a point, so there was no cuff and no arm, only a spike.
struct SleeveShape {
    /// Down and out from the shoulder point, for the LEFT sleeve. Unit length.
    axis: Vec2,
    /// Along that axis: the sleeve's own measurement, undistorted.
    length: f32,
    /// Half the cuff, measured across the axis.
    cuff_half: f32,
}

/// One function because the overall extent and the outline both need the
/// answer and must not disagree: they each worked it out separately before,
/// and a silhouette drawn wider than the frame it was measured for is clipped
/// without anything failing.
fn sleeve_shape(chest: f32, length: f32, shoulder: f32, sleeve: f32) -> SleeveShape {
    let run = if length > 0.0 {
        (sleeve / length - SLEEVE_RUN.0) / (SLEEVE_RUN.1 - SLEEVE_RUN.0)
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    let angle = (SLEEVE_ANGLE.0 + run * (SLEEVE_ANGLE.1 - SLEEVE_ANGLE.0)).to_radians();
    // The cuff is a share of the armhole rather than a figure of its own,
    // because the armhole is the seam the sleeve is cut to. Set from the chest
    // instead it would flare on a narrow shoulder and pinch on a broad one.
    let armhole = Vec2::new((shoulder - chest) / 2.0, length * ARMHOLE_Y).length();
    let keep = CUFF_OF_ARMHOLE.0 + run * (CUFF_OF_ARMHOLE.1 - CUFF_OF_ARMHOLE.0);
    SleeveShape {
        axis: Vec2::new(-angle.cos(), angle.sin()),
        length: sleeve,
        cuff_half: armhole * keep / 2.0,
    }
}

impl SleeveShape {
    /// Across the sleeve, away from the body. Unit length.
    fn across(&self) -> Vec2 {
        Vec2::new(-self.axis.y, self.axis.x)
    }

    /// The two corners of the cuff, outer first, for a sleeve hung from `s`.
    fn cuff(&self, s: Vec2) -> (Vec2, Vec2) {
        let end = s + self.axis * self.length;
        let across = self.across() * self.cuff_half;
        (end + across, end - across)
    }

    /// How far past the shoulder point the sleeve reaches, and how far below
    /// it the lower corner of the cuff falls.
    fn beyond_shoulder(&self) -> Vec2 {
        let (outer, inner) = self.cuff(Vec2::ZERO);
        Vec2::new(-outer.x.min(inner.x), outer.y.max(inner.y))
    }
}

/// The box a top needs.
///
/// The sleeves decide the width, and the hem the height unless a sleeve long
/// enough falls past it.
fn top_extent(across: f32, length: f32, shoulder: f32, sleeve: f32) -> Vec2 {
    let beyond = sleeve_shape(across, length, shoulder, sleeve).beyond_shoulder();
    Vec2::new(
        across.max(shoulder + beyond.x * 2.0),
        length.max(length * SHOULDER_SLOPE + beyond.y),
    )
}

/// The gap between two pieces of a set, in millimetres.
const PIECE_GAP: f32 = 40.0;

fn top_contour(
    pb: &mut PathBuilder,
    left: f32,
    width: f32,
    chest: f32,
    length: f32,
    shoulder: f32,
    sleeve: f32,
) {
    let centre = left + width / 2.0;
    let body_half = chest / 2.0;
    let shoulder_half = shoulder / 2.0;
    // Proportions of the length rather than fixed millimetres, so the shape
    // stays recognisable from a three-year-old's to an adult L.
    let armhole_y = length * ARMHOLE_Y;
    let neck_dip = length * 0.07;
    let neck_half = shoulder * 0.19;
    let shoulder_y = length * SHOULDER_SLOPE;
    let hem_half = body_half * 0.97;

    let arm = sleeve_shape(chest, length, shoulder, sleeve);

    // Worked out on the left and mirrored, so the two sleeves cannot drift
    // apart under an edit to one of them.
    let tip = Vec2::new(centre - shoulder_half, shoulder_y);
    let armpit = Vec2::new(centre - body_half, armhole_y);
    let (outer, inner) = arm.cuff(tip);
    // The top of the sleeve leaves the shoulder on a curve rather than a
    // crease. Its control point stays inside the chord's own corner, so the
    // curve cannot bulge past the frame the extent was measured for.
    let cap = (tip + outer) / 2.0 + arm.across() * (outer - tip).length() * 0.06;
    // The underarm seam runs straight most of the way and then turns into the
    // body. The armpit is a concave corner, so the quadratic rounds it with
    // the corner itself as the control point, rather than leaving the sharp V
    // that a sleeve meeting a side seam would otherwise be.
    let seam = inner + (armpit - inner) * 0.72;
    let side = armpit + (Vec2::new(centre - hem_half, length) - armpit) * 0.18;

    let flip = |p: Vec2| Vec2::new(2.0 * centre - p.x, p.y);
    let (r_tip, r_cap, r_outer, r_inner, r_seam, r_armpit, r_side) = (
        flip(tip),
        flip(cap),
        flip(outer),
        flip(inner),
        flip(seam),
        flip(armpit),
        flip(side),
    );

    pb.move_to(tip.x, tip.y);
    pb.quad_to(cap.x, cap.y, outer.x, outer.y);
    pb.line_to(inner.x, inner.y);
    pb.line_to(seam.x, seam.y);
    pb.quad_to(armpit.x, armpit.y, side.x, side.y);
    pb.line_to(centre - hem_half, length);
    pb.line_to(centre + hem_half, length);
    pb.line_to(r_side.x, r_side.y);
    pb.quad_to(r_armpit.x, r_armpit.y, r_seam.x, r_seam.y);
    pb.line_to(r_inner.x, r_inner.y);
    pb.line_to(r_outer.x, r_outer.y);
    pb.quad_to(r_cap.x, r_cap.y, r_tip.x, r_tip.y);
    pb.line_to(centre + neck_half, 0.0);
    pb.quad_to(centre, neck_dip, centre - neck_half, 0.0);
    pb.close();
}

/// A waist and two legs, with the crotch at `rise` below the waist.
fn legs_contour(pb: &mut PathBuilder, left: f32, width: f32, length: f32, rise: f32) {
    let centre = left + width / 2.0;
    let hem = width * 0.40;
    let rise = rise.clamp(width * 0.2, length * 0.85);

    pb.move_to(left, 0.0);
    pb.line_to(left + width, 0.0);
    // Right leg, outside down and inside back up to the crotch.
    pb.line_to(left + width - (width - hem * 2.0) * 0.06, length);
    pb.line_to(left + width - hem, length);
    pb.line_to(centre, rise);
    // Left leg.
    pb.line_to(left + hem, length);
    pb.line_to(left + (width - hem * 2.0) * 0.06, length);
    pb.close();
}

/// Legs, a bib above them, and two straps over the shoulders.
fn dungarees_contour(pb: &mut PathBuilder, width: f32, length: f32, rise: f32, bib: f32) {
    // The straps rise above the bib and have to fit inside the same box the
    // extent describes: drawn from zero they went off the top of the frame and
    // were quietly clipped, which nothing failed on and a picture showed at
    // once.
    let strap_rise = length * 0.11;
    let bib_height = (length * 0.24).min(bib);
    let bib_top = strap_rise;
    let strap = width * 0.09;
    let bib_half = bib / 2.0;
    let centre = width / 2.0;

    // Body and bib as one contour, so the cloth runs between them.
    pb.move_to(centre - bib_half, bib_top);
    pb.line_to(centre + bib_half, bib_top);
    pb.line_to(centre + bib_half, bib_top + bib_height);
    pb.line_to(width, bib_top + bib_height);
    pb.line_to(width, bib_top + bib_height + rise * 0.35);
    legs_tail(pb, width, length, bib_top + bib_height, rise);
    pb.line_to(0.0, bib_top + bib_height + rise * 0.35);
    pb.line_to(0.0, bib_top + bib_height);
    pb.line_to(centre - bib_half, bib_top + bib_height);
    pb.close();

    // Two straps over the shoulders, as their own contours.
    for side in [-1.0f32, 1.0] {
        let foot = centre + side * (bib_half - strap * 0.5);
        let head = foot + side * strap * 0.8;
        pb.move_to(foot - strap / 2.0, bib_top);
        pb.line_to(foot + strap / 2.0, bib_top);
        pb.line_to(head + strap / 2.0, 0.0);
        pb.line_to(head - strap / 2.0, 0.0);
        pb.close();
    }
}

/// The two legs of a dungaree body, from the right hip round to the left.
fn legs_tail(pb: &mut PathBuilder, width: f32, length: f32, top: f32, rise: f32) {
    let centre = width / 2.0;
    let hem = width * 0.40;
    pb.line_to(width - (width - hem * 2.0) * 0.06, length);
    pb.line_to(width - hem, length);
    pb.line_to(centre, top + rise);
    pb.line_to(hem, length);
    pb.line_to((width - hem * 2.0) * 0.06, length);
}

/// A crown with a brim under it.
fn hat_contour(pb: &mut PathBuilder, width: f32, height: f32, crown: f32) {
    let centre = width / 2.0;
    let crown_half = crown / 2.0;
    let brim_y = height * 0.66;

    pb.move_to(centre - crown_half, brim_y);
    // Over the crown.
    pb.quad_to(centre - crown_half * 0.95, 0.0, centre, 0.0);
    pb.quad_to(centre + crown_half * 0.95, 0.0, centre + crown_half, brim_y);
    // Out to the brim and round its underside.
    let brim_edge_y = brim_y + (height - brim_y) * 0.30;
    pb.line_to(width, brim_edge_y);
    // A quadratic's furthest point is a quarter of each end plus half the
    // control, so the control has to be placed from where the curve should
    // reach rather than where it should pass through. Put at the hem itself it
    // overshot by ten millimetres, which nothing failed on and a tight bound
    // showed at once.
    pb.quad_to(centre, (height - brim_edge_y * 0.5) * 2.0, 0.0, brim_edge_y);
    pb.close();
}

/// A bag with two handles over it.
fn tote_contour(pb: &mut PathBuilder, width: f32, length: f32, handle: f32) {
    // The body sits below the handles.
    let top = handle;
    pb.push_rect(
        tiny_skia::Rect::from_xywh(0.0, top, width, length).unwrap_or(
            tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("a unit rect is valid"),
        ),
    );

    // Two straps, each a loop from the bag's top edge and back.
    let thickness = width * 0.055;
    for side in [-1.0f32, 1.0] {
        let inner = width / 2.0 + side * width * 0.14;
        let outer = width / 2.0 + side * width * 0.30;
        pb.move_to(inner, top);
        pb.quad_to((inner + outer) / 2.0, top - handle * 1.25, outer, top);
        pb.line_to(outer - side * thickness, top);
        pb.quad_to(
            (inner + outer) / 2.0,
            top - handle * 1.25 + thickness * 1.9,
            inner + side * thickness,
            top,
        );
        pb.close();
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
    // repeats, the same trap as a pre-repeated export.
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
