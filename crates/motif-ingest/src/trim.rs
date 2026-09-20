use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::source::SourceImage;

/// Where a motif sat inside the image it arrived in.
///
/// Kept because it is the only record of how much empty space was thrown away,
/// which is the first thing to look at when an ingested motif comes out the
/// wrong size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrimBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Crop to the motif's own extent and record the physical size it stands for.
///
/// Two jobs in one pass because they are the same decision: the physical size
/// is a property of the *subject*, not of however much paper it was drawn on.
/// Sizing before trimming would make a motif's scale depend on the scanner's
/// margins.
///
/// `alpha_floor` is the coverage below which a pixel counts as background. Not
/// zero by default: a feathered matte leaves a faint halo of one- and two-value
/// alpha well outside the subject, and trimming to any non-zero pixel would
/// keep most of it.
#[must_use]
pub fn trim(image: &SourceImage, alpha_floor: u8) -> Option<(SourceImage, TrimBounds)> {
    let (mut min_x, mut min_y) = (u32::MAX, u32::MAX);
    let (mut max_x, mut max_y) = (0u32, 0u32);
    let mut any = false;

    for y in 0..image.height {
        for x in 0..image.width {
            if image.pixel(x, y)[3] > alpha_floor {
                any = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if !any {
        return None;
    }

    let bounds = TrimBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    };

    let mut pixels = Vec::with_capacity(bounds.width as usize * bounds.height as usize * 4);
    for y in bounds.y..bounds.y + bounds.height {
        for x in bounds.x..bounds.x + bounds.width {
            pixels.extend_from_slice(&image.pixel(x, y));
        }
    }
    let cropped = SourceImage::new(bounds.width, bounds.height, pixels)?;
    Some((cropped, bounds))
}

/// The physical size a trimmed motif stands for, from its longest edge.
///
/// Expressed as a longest edge rather than a width or a height so that a motif
/// keeps a consistent presence in a pattern whichever way round it is drawn.
/// Aspect ratio comes from the pixels and is never adjusted — a motif that
/// arrives at 3:2 stays at 3:2, because distorting artwork to hit a target
/// shape is not something a pipeline should do quietly.
#[must_use]
pub fn natural_size_mm(width: u32, height: u32, longest_edge_mm: f32) -> Vec2 {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    let longest = w.max(h);
    Vec2::new(longest_edge_mm * w / longest, longest_edge_mm * h / longest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    const INK: [u8; 4] = [200, 30, 45, 255];

    fn ink_in_the_middle() -> SourceImage {
        let mut img = SourceImage::filled(8, 6, CLEAR);
        img.set_pixel(2, 1, INK);
        img.set_pixel(3, 1, INK);
        img.set_pixel(3, 2, INK);
        img
    }

    #[test]
    fn trimming_crops_to_the_subject() {
        let (cropped, bounds) = trim(&ink_in_the_middle(), 0).expect("there is ink to find");
        assert_eq!(
            bounds,
            TrimBounds {
                x: 2,
                y: 1,
                width: 2,
                height: 2
            }
        );
        assert_eq!((cropped.width, cropped.height), (2, 2));
        assert_eq!(cropped.pixel(0, 0), INK);
        assert_eq!(cropped.pixel(0, 1), CLEAR);
    }

    #[test]
    fn a_faint_halo_is_not_mistaken_for_the_subject() {
        // The case the floor exists for: feathering leaves alpha 1 or 2 far
        // outside the motif, and trimming to any non-zero pixel keeps the lot.
        let mut img = ink_in_the_middle();
        img.set_pixel(0, 5, [200, 30, 45, 2]);
        img.set_pixel(7, 0, [200, 30, 45, 1]);

        let (_, tight) = trim(&img, 8).expect("ink is above the floor");
        assert_eq!(tight.width, 2, "halo should not widen the crop");

        let (_, loose) = trim(&img, 0).expect("ink is above zero");
        assert_eq!(
            loose.width, 8,
            "with no floor the halo is kept, as expected"
        );
    }

    #[test]
    fn a_blank_image_trims_to_nothing_rather_than_to_a_pixel() {
        assert!(trim(&SourceImage::filled(4, 4, CLEAR), 0).is_none());
    }

    #[test]
    fn a_full_bleed_motif_is_left_alone() {
        let img = SourceImage::filled(3, 3, INK);
        let (cropped, bounds) = trim(&img, 0).expect("all ink");
        assert_eq!((bounds.width, bounds.height), (3, 3));
        assert_eq!(cropped, img);
    }

    #[test]
    fn physical_size_preserves_aspect_and_scales_the_longest_edge() {
        let size = natural_size_mm(300, 200, 60.0);
        assert!((size.x - 60.0).abs() < 1e-3);
        assert!((size.y - 40.0).abs() < 1e-3);
    }

    #[test]
    fn a_tall_motif_scales_on_its_height() {
        let size = natural_size_mm(200, 300, 60.0);
        assert!((size.y - 60.0).abs() < 1e-3);
        assert!((size.x - 40.0).abs() < 1e-3);
    }

    #[test]
    fn a_square_motif_is_square() {
        let size = natural_size_mm(128, 128, 52.0);
        assert!((size.x - size.y).abs() < 1e-4);
    }
}
