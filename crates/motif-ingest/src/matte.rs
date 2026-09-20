use serde::{Deserialize, Serialize};

use crate::source::SourceImage;

/// How to separate a motif from its background.
///
/// No background-removal model here, deliberately. The spec notes that hand
/// illustrations on white paper can usually skip one — a luminance threshold
/// with a feathered edge is faster and more faithful — and generated motifs are
/// asked for on a flat background for the same reason. A model of the BiRefNet
/// or SAM class becomes worth its weight when real artwork arrives that these
/// cannot handle, and it would slot in as another variant here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MatteMethod {
    /// Trust the alpha already in the file.
    ///
    /// Right for artwork exported with transparency, and for anything that has
    /// been through this pipeline before.
    ExistingAlpha,

    /// Cut a subject away from a light background by brightness.
    ///
    /// Pixels darker than `opaque_below` are kept whole; pixels brighter than
    /// `opaque_below + feather` are dropped; between the two the edge fades.
    /// Both are luminance in 0..1.
    LuminanceThreshold {
        opaque_below: f32,
        /// Width of the fade, in luminance. Zero gives a hard, aliased edge —
        /// which is what a naive threshold produces, and why this is a
        /// parameter rather than a constant.
        feather: f32,
    },
}

impl Default for MatteMethod {
    fn default() -> Self {
        // Chosen for illustration on white paper: paper scans rarely read as
        // pure white, so the cut sits well below 1.0, and the fade is wide
        // enough to keep a pencil edge from turning into a staircase.
        Self::LuminanceThreshold {
            opaque_below: 0.82,
            feather: 0.10,
        }
    }
}

/// Perceived brightness in 0..1, by the Rec. 709 luma weights.
///
/// Computed on sRGB values directly rather than after converting to linear
/// light. That is the usual approximation and it is the right one here: the
/// threshold is a knob a person turns until the edge looks right, so matching
/// their perception of the source matters more than photometric correctness.
#[must_use]
pub fn luminance(rgb: [u8; 3]) -> f32 {
    (0.2126 * rgb[0] as f32 + 0.7152 * rgb[1] as f32 + 0.0722 * rgb[2] as f32) / 255.0
}

/// Smooth Hermite interpolation between two edges, clamped outside them.
///
/// A polynomial rather than a trigonometric ease, so matting stays exactly
/// reproducible across platforms for the same reason the scatter does.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Produce a copy of `image` with its alpha channel replaced by a matte.
///
/// Colour is left alone. A matte decides coverage, and a pipeline that also
/// altered colour here would be making two decisions in one place and would
/// make the quantisation step downstream harder to reason about.
#[must_use]
pub fn matte(image: &SourceImage, method: MatteMethod) -> SourceImage {
    match method {
        MatteMethod::ExistingAlpha => image.clone(),
        MatteMethod::LuminanceThreshold {
            opaque_below,
            feather,
        } => {
            let mut out = image.clone();
            for i in 0..image.pixel_count() {
                let p = i * 4;
                let l = luminance([image.pixels[p], image.pixels[p + 1], image.pixels[p + 2]]);
                let coverage = 1.0 - smoothstep(opaque_below, opaque_below + feather, l);
                // Multiply rather than replace, so alpha already present in the
                // source is respected instead of being invented over.
                let existing = image.pixels[p + 3] as f32 / 255.0;
                out.pixels[p + 3] = ((coverage * existing) * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const RED: [u8; 4] = [200, 30, 45, 255];
    const GREY: [u8; 4] = [220, 220, 220, 255];

    fn paper_with_a_mark() -> SourceImage {
        let mut img = SourceImage::filled(4, 4, WHITE);
        img.set_pixel(1, 1, RED);
        img.set_pixel(2, 1, RED);
        img
    }

    #[test]
    fn paper_becomes_transparent_and_ink_stays() {
        let out = matte(&paper_with_a_mark(), MatteMethod::default());
        assert_eq!(out.pixel(0, 0)[3], 0, "paper should be cut away");
        assert_eq!(out.pixel(1, 1)[3], 255, "ink should be untouched");
    }

    #[test]
    fn colour_is_never_altered_by_matting() {
        let out = matte(&paper_with_a_mark(), MatteMethod::default());
        assert_eq!(&out.pixel(1, 1)[0..3], &RED[0..3]);
        // Even where it was cut away, the colour is left as it was: only
        // coverage is a matting decision.
        assert_eq!(&out.pixel(0, 0)[0..3], &WHITE[0..3]);
    }

    #[test]
    fn a_midtone_inside_the_feather_band_comes_out_partly_covered() {
        // The whole point of feathering: without it this pixel would be either
        // fully in or fully out, and a scanned pencil edge would stairstep.
        let mut img = SourceImage::filled(1, 1, GREY);
        img.set_pixel(0, 0, GREY);
        let out = matte(
            &img,
            MatteMethod::LuminanceThreshold {
                opaque_below: 0.80,
                feather: 0.20,
            },
        );
        let a = out.pixel(0, 0)[3];
        assert!(a > 0 && a < 255, "expected a soft edge, got alpha {a}");
    }

    #[test]
    fn zero_feather_gives_a_hard_edge() {
        let out = matte(
            &paper_with_a_mark(),
            MatteMethod::LuminanceThreshold {
                opaque_below: 0.9,
                feather: 0.0,
            },
        );
        for y in 0..4 {
            for x in 0..4 {
                let a = out.pixel(x, y)[3];
                assert!(a == 0 || a == 255, "alpha {a} at ({x}, {y}) is not binary");
            }
        }
    }

    #[test]
    fn existing_transparency_is_respected_rather_than_overwritten() {
        // A generated motif may arrive already cut out. Thresholding it must
        // not resurrect background the exporter had already removed.
        let mut img = SourceImage::filled(2, 1, RED);
        img.set_pixel(0, 0, [200, 30, 45, 0]);
        let out = matte(&img, MatteMethod::default());
        assert_eq!(out.pixel(0, 0)[3], 0);
        assert_eq!(out.pixel(1, 0)[3], 255);
    }

    #[test]
    fn existing_alpha_method_changes_nothing() {
        let img = paper_with_a_mark();
        assert_eq!(matte(&img, MatteMethod::ExistingAlpha), img);
    }

    #[test]
    fn luminance_weights_green_most() {
        // Guards the Rec. 709 weights against being transposed, which would be
        // invisible on greyscale artwork and wrong on everything else.
        assert!(luminance([0, 255, 0]) > luminance([255, 0, 0]));
        assert!(luminance([255, 0, 0]) > luminance([0, 0, 255]));
    }
}
