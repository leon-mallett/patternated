//! Turning outlines into pixels, at the size the render actually wants.
//!
//! This is what makes a vector motif worth keeping as one. A raster motif is
//! sampled once, when it is imported, at whatever resolution somebody guessed;
//! this is sampled every render, at the resolution that render needs, so the
//! same drawing is 60 pixels across in a preview and 600 in a print and is
//! exactly right in both.
//!
//! What comes out is an ordinary [`IndexedMotif`], so nothing downstream knows
//! or cares. The sprite cache, the mip chain, the overlap radius and the
//! colourway all go on working on the one motif representation they always
//! had.
//!
//! # Slots over an edge
//!
//! A pixel where two shapes meet is covered partly by each, and the model has
//! one slot and one coverage for it. The slot is the shape covering most of
//! the pixel, and the coverage is everything the shapes lay down together.
//! That keeps a flat interior exactly flat, which is what this engine's split
//! channels exist for, while an edge between two colours stays soft.

use pattern_core::{IndexedMotif, PatternError, VectorMotif, GROUND_SLOT};
use tiny_skia::{FillRule, Paint, Pixmap, Transform};

/// Draw a vector motif at a given pixel size.
///
/// # Errors
///
/// If the size is zero, or too large for a pixmap.
pub fn draw_vector(
    motif: &VectorMotif,
    width: u32,
    height: u32,
) -> Result<IndexedMotif, RasterError> {
    let (width, height) = (width.max(1), height.max(1));
    let count = width as usize * height as usize;

    // The scale from the drawing's own coordinates to the pixels asked for.
    // Non-uniform on purpose: the caller has decided the pixel box, and a
    // motif's aspect ratio is already settled by its physical size.
    let view = motif.view;
    if view.width() <= 0.0 || view.height() <= 0.0 {
        return Err(RasterError::NoSize);
    }
    let to_pixels =
        Transform::from_scale(width as f32 / view.width(), height as f32 / view.height())
            .post_translate(-view.left(), -view.top());

    let mut slots = vec![GROUND_SLOT; count];
    let mut alpha = vec![0u8; count];
    // How much of each pixel the winning shape covers, so a later shape only
    // takes the slot where it covers more than whatever held it.
    let mut held = vec![0u8; count];

    let mut ink = Pixmap::new(width, height).ok_or(RasterError::TooLarge { width, height })?;
    let paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    for shape in &motif.shapes {
        if shape.opacity == 0 {
            continue;
        }
        // One shape at a time into a cleared pixmap, in white, so what comes
        // back is coverage and nothing else. Drawing them together would give
        // a colour to un-quantise, which is the bitmap path's problem and not
        // one worth importing.
        ink.fill(tiny_skia::Color::TRANSPARENT);
        ink.fill_path(
            &shape.path,
            &paint,
            if shape.even_odd {
                FillRule::EvenOdd
            } else {
                FillRule::Winding
            },
            to_pixels,
            None,
        );

        for (i, px) in ink.pixels().iter().enumerate() {
            let covered = px.alpha();
            if covered == 0 {
                continue;
            }
            let laid = scale(covered, shape.opacity);
            if laid == 0 {
                continue;
            }
            // Greater *or equal*: shapes arrive in painter's order, so where
            // two cover a pixel equally the later one is on top. Strictly
            // greater leaves a shape drawn over a solid background showing the
            // background's colour, which is a drawing with its layers
            // reversed.
            if laid >= held[i] {
                held[i] = laid;
                slots[i] = shape.slot;
            }
            // Coverage accumulates the way paint does: what is already there
            // plus what this adds to the part still showing.
            alpha[i] = alpha[i].saturating_add(scale(laid, 255 - alpha[i]));
        }
    }

    IndexedMotif::new(width, height, slots, alpha).map_err(RasterError::Malformed)
}

/// `a * b / 255`, rounded, without leaving `u8`.
fn scale(a: u8, b: u8) -> u8 {
    ((u16::from(a) * u16::from(b) + 127) / 255) as u8
}

/// Why a vector motif could not be drawn.
#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error("this drawing has no area to rasterise")]
    NoSize,
    #[error("a {width} x {height} px motif is more than this can allocate")]
    TooLarge { width: u32, height: u32 },
    #[error(transparent)]
    Malformed(PatternError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use pattern_core::VectorShape;
    use tiny_skia_path::{PathBuilder, Rect};

    fn rect(x: f32, y: f32, w: f32, h: f32, slot: u8, opacity: u8) -> VectorShape {
        let mut b = PathBuilder::new();
        b.push_rect(Rect::from_xywh(x, y, w, h).expect("rect"));
        VectorShape {
            path: b.finish().expect("path"),
            slot,
            even_odd: false,
            opacity,
        }
    }

    fn motif(shapes: Vec<VectorShape>) -> VectorMotif {
        VectorMotif {
            view: Rect::from_xywh(0.0, 0.0, 100.0, 100.0).expect("rect"),
            shapes,
        }
    }

    #[test]
    fn the_same_drawing_is_drawn_at_whatever_size_is_asked_for() {
        // The whole reason a vector motif is kept as one. Not "close enough at
        // both sizes" but correct at both, from the same outlines.
        let m = motif(vec![rect(25.0, 25.0, 50.0, 50.0, 1, 255)]);
        for size in [16u32, 64, 256, 1024] {
            let drawn = draw_vector(&m, size, size).expect("should draw");
            assert_eq!((drawn.width, drawn.height), (size, size));
            let covered = drawn.alpha.iter().filter(|a| **a > 128).count() as f32;
            let share = covered / (size * size) as f32;
            // The square is a quarter of the view at every resolution.
            assert!(
                (share - 0.25).abs() < 0.01,
                "at {size} px the square covered {share:.3} of the motif"
            );
        }
    }

    #[test]
    fn a_flat_interior_stays_exactly_flat() {
        // The property the split channels exist for, and the one an edge rule
        // could quietly break: inside a shape there is one slot and full
        // coverage, whatever is happening at its rim.
        let m = motif(vec![rect(20.0, 20.0, 60.0, 60.0, 3, 255)]);
        let drawn = draw_vector(&m, 100, 100).expect("should draw");
        let at = |x: usize, y: usize| (drawn.slots[y * 100 + x], drawn.alpha[y * 100 + x]);
        assert_eq!(at(50, 50), (3, 255));
        assert_eq!(at(25, 25), (3, 255));
        assert_eq!(at(5, 5), (GROUND_SLOT, 0), "outside is untouched");
    }

    #[test]
    fn the_shape_covering_most_of_a_pixel_takes_its_slot() {
        // Two shapes meeting inside one pixel. The model has one slot for it,
        // so the honest answer is whichever covers more, and the coverage is
        // what both of them laid down.
        let m = motif(vec![
            rect(0.0, 0.0, 100.0, 100.0, 1, 255),
            rect(0.0, 0.0, 50.0, 100.0, 2, 255),
        ]);
        let drawn = draw_vector(&m, 100, 100).expect("should draw");
        assert_eq!(drawn.slots[50 * 100 + 10], 2, "under the top shape");
        assert_eq!(drawn.slots[50 * 100 + 90], 1, "beyond it");
        assert!(
            drawn.alpha.iter().all(|a| *a == 255),
            "all of it is covered"
        );
    }

    #[test]
    fn a_half_opaque_shape_keeps_its_slot_and_loses_coverage() {
        // Folding opacity into the colour would go looking for a pink that is
        // not in the palette. It belongs in the channel that is continuous.
        let m = motif(vec![rect(0.0, 0.0, 100.0, 100.0, 2, 128)]);
        let drawn = draw_vector(&m, 8, 8).expect("should draw");
        assert!(drawn.slots.iter().all(|s| *s == 2));
        assert!(
            drawn.alpha.iter().all(|a| (i16::from(*a) - 128).abs() <= 2),
            "{:?}",
            &drawn.alpha[..4]
        );
    }

    #[test]
    fn an_edge_is_soft_while_the_inside_is_not() {
        // A circle, so the rim crosses pixels at every angle. There must be
        // partial coverage at the edge and none of it in the middle.
        let mut b = PathBuilder::new();
        b.push_circle(50.0, 50.0, 40.0);
        let m = motif(vec![VectorShape {
            path: b.finish().expect("path"),
            slot: 1,
            even_odd: false,
            opacity: 255,
        }]);
        let drawn = draw_vector(&m, 100, 100).expect("should draw");
        let partial = drawn.alpha.iter().filter(|a| **a > 0 && **a < 255).count();
        assert!(partial > 100, "only {partial} pixels are partly covered");
        assert_eq!(drawn.alpha[50 * 100 + 50], 255, "the middle is solid");
    }

    #[test]
    fn an_invisible_shape_draws_nothing() {
        let m = motif(vec![rect(0.0, 0.0, 100.0, 100.0, 4, 0)]);
        let drawn = draw_vector(&m, 16, 16).expect("should draw");
        assert!(drawn.alpha.iter().all(|a| *a == 0));
        assert!(drawn.slots_used().is_empty(), "nothing to pay for");
    }
}
