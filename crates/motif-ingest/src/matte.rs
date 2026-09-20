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
    ///
    /// **An absolute threshold only works on a digitally clean background.** It
    /// was the default until real artwork was measured against it: photographed
    /// watercolour paper came in at 0.895 under even light and 0.818 on the
    /// shadowed side of a sheet, both below the 0.92 this cuts above, so the
    /// paper survived as motif. Use [`MatteMethod::PaperRelative`] for anything
    /// that has met a camera or a scanner.
    LuminanceThreshold {
        opaque_below: f32,
        /// Width of the fade, in luminance. Zero gives a hard, aliased edge —
        /// which is what a naive threshold produces, and why this is a
        /// parameter rather than a constant.
        feather: f32,
    },

    /// Cut the background away relative to the paper's own brightness, measured
    /// locally.
    ///
    /// Estimates how bright the paper is in each part of the image and keeps
    /// whatever is darker than its own surroundings by more than `margin`. Two
    /// things fall out of that which an absolute threshold cannot do:
    ///
    /// Cream paper, grey paper and bright white paper all work without anyone
    /// tuning a number, because the number is measured rather than guessed.
    ///
    /// And a sheet lit from one side works, because the estimate varies across
    /// the image. A shadowed corner of paper is still paper *relative to its
    /// own corner*, while an absolute threshold sees it as darker than the
    /// cut-off and keeps it.
    ///
    /// It also rescues the opposite case: a pale subject on pale paper. An
    /// elderflower measured 0.917 against paper at 0.971 — an absolute
    /// threshold anywhere near the paper removes the flower too, while a
    /// relative one keeps anything meaningfully darker than its surroundings.
    PaperRelative {
        /// How far below the local paper level a pixel must sit to be kept, as
        /// a fraction of that level. Small: the gap between pale ink and paper
        /// is small.
        margin: f32,
        /// Width of the fade, in the same fraction.
        feather: f32,
    },
}

impl Default for MatteMethod {
    fn default() -> Self {
        // Relative rather than absolute, because paper is not a known colour.
        // The margin is small on purpose: pale ink on pale paper is a real
        // case and a generous margin erases it.
        Self::PaperRelative {
            margin: 0.02,
            feather: 0.035,
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

/// Blocks per axis in the illumination estimate.
///
/// Coarse on purpose. The estimate must follow the lighting, which varies
/// slowly, without following the artwork, which does not. Too fine and a motif
/// large enough to fill a block makes that block think the paper is dark.
const ILLUMINATION_BLOCKS: usize = 20;

/// Estimate how bright the paper is across the image, as a plane.
///
/// Per block, a high percentile of luminance — high because paper is the
/// brightest thing present and a mean would be dragged down by ink. Then a
/// plane is fitted through those blocks, and refitted three times, each round
/// dropping blocks that sit well below the current fit.
///
/// **A plane, rather than a smoothed version of the measurements.** Two
/// failures pushed it here, and both are worth keeping written down because
/// each looked like the obvious approach:
///
/// Dilating the block measurements — taking a local maximum so that paper
/// level reaches blocks containing no paper — over-estimates on a gradient. A
/// shadowed corner of a sheet inherits the brightness of its sunnier
/// neighbours, decides its own paper is too dark to be paper, and keeps the
/// whole corner as motif.
///
/// And no local operator of any radius reaches the middle of a large motif. A
/// subject filling half the frame leaves interior blocks many blocks deep in
/// ink, which is exactly where a wrong estimate does most damage.
///
/// Fitting a global surface to the blocks that look like paper solves both: it
/// extrapolates *through* the motif rather than around it, and it follows a
/// one-sided light because that is what a plane is for. Refitting is what makes
/// it robust — the first fit is dragged down by the ink, and dropping whatever
/// falls well below it leaves the paper.
fn illumination_field(image: &SourceImage) -> Paper {
    let n = ILLUMINATION_BLOCKS;
    let mut measured = vec![0.0f32; n * n];
    let mut spread = vec![0.0f32; n * n];

    for by in 0..n {
        for bx in 0..n {
            let x0 = (bx * image.width as usize) / n;
            let x1 = (((bx + 1) * image.width as usize) / n).max(x0 + 1);
            let y0 = (by * image.height as usize) / n;
            let y1 = (((by + 1) * image.height as usize) / n).max(y0 + 1);

            let mut samples: Vec<f32> = Vec::new();
            for y in y0..y1.min(image.height as usize) {
                for x in x0..x1.min(image.width as usize) {
                    let p = (y * image.width as usize + x) * 4;
                    samples.push(luminance([
                        image.pixels[p],
                        image.pixels[p + 1],
                        image.pixels[p + 2],
                    ]));
                }
            }
            if samples.is_empty() {
                continue;
            }
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let high = samples[(samples.len() * 9) / 10];
            let low = samples[samples.len() / 10];
            measured[by * n + bx] = high;
            // How much this block's brightness varies within itself. On paper
            // that is the grain; on a motif it is the artwork, which is why
            // only blocks the fit keeps are used below.
            spread[by * n + bx] = if high > 1e-3 { 1.0 - low / high } else { 0.0 };
        }
    }

    let mut keep = vec![true; n * n];
    let mut plane = (0.0f32, 0.0f32, 0.0f32);
    for round in 0..4 {
        plane = fit_plane(&measured, &keep, n);
        if round == 3 {
            break;
        }
        // Drop blocks well below the fit: those are ink, not paper.
        let mut any = false;
        for by in 0..n {
            for bx in 0..n {
                let fitted = plane.0 + plane.1 * bx as f32 + plane.2 * by as f32;
                let below = fitted - measured[by * n + bx];
                keep[by * n + bx] = below < 0.04;
                any |= keep[by * n + bx];
            }
        }
        if !any {
            // Every block fell below the fit, which means there is no paper to
            // find. Keep the plane as it stands rather than refitting on
            // nothing.
            break;
        }
    }

    let mut field = vec![0.0f32; n * n];
    for by in 0..n {
        for bx in 0..n {
            // Never below what was actually measured: a plane that dips under
            // the paper it is modelling would start keeping paper as motif.
            field[by * n + bx] = (plane.0 + plane.1 * bx as f32 + plane.2 * by as f32)
                .max(measured[by * n + bx])
                .clamp(0.02, 1.0);
        }
    }

    // The grain of the paper itself, from the blocks the fit decided were
    // paper. Median rather than mean so one ink-heavy block cannot inflate it.
    let mut grains: Vec<f32> = (0..n * n).filter(|i| keep[*i]).map(|i| spread[i]).collect();
    grains.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let grain = if grains.is_empty() {
        0.0
    } else {
        grains[grains.len() / 2]
    };

    Paper { field, grain }
}

/// What the paper is doing across an image: how bright, and how uneven.
struct Paper {
    /// Estimated paper brightness per block.
    field: Vec<f32>,
    /// How much paper brightness varies within a block, as a fraction.
    ///
    /// This is the number that makes a fixed margin unworkable, and measuring
    /// it is what replaces one. Photographed watercolour paper varies by about
    /// 5.5% within itself, so anything closer to the paper than that is
    /// indistinguishable from grain; a flat digital background varies by under
    /// 1%, where the same 5.5% margin would erase a pale subject. The rule that
    /// works for both is *keep what is darker than the paper by more than the
    /// paper varies*.
    grain: f32,
}

/// Least squares `z = a + b*x + c*y` over the blocks still marked as paper.
fn fit_plane(z: &[f32], keep: &[bool], n: usize) -> (f32, f32, f32) {
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy, mut syy) = (0.0f64, 0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut sz, mut sxz, mut syz) = (0.0f64, 0.0, 0.0);
    for by in 0..n {
        for bx in 0..n {
            if !keep[by * n + bx] {
                continue;
            }
            let (x, y, v) = (bx as f64, by as f64, z[by * n + bx] as f64);
            sw += 1.0;
            sx += x;
            sy += y;
            sxx += x * x;
            sxy += x * y;
            syy += y * y;
            sz += v;
            sxz += x * v;
            syz += y * v;
        }
    }
    if sw < 3.0 {
        let mean = if sw > 0.0 { sz / sw } else { 0.9 };
        return (mean as f32, 0.0, 0.0);
    }

    // Cramer's rule on the 3x3 normal equations.
    let m = [[sw, sx, sy], [sx, sxx, sxy], [sy, sxy, syy]];
    let rhs = [sz, sxz, syz];
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det3(m);
    if d.abs() < 1e-9 {
        return ((sz / sw) as f32, 0.0, 0.0);
    }
    let mut out = [0.0f64; 3];
    for i in 0..3 {
        let mut mi = m;
        for r in 0..3 {
            mi[r][i] = rhs[r];
        }
        out[i] = det3(mi) / d;
    }
    (out[0] as f32, out[1] as f32, out[2] as f32)
}

/// Read the illumination field at a pixel, interpolating between blocks.
fn sample_field(field: &[f32], width: u32, height: u32, x: u32, y: u32) -> f32 {
    let n = ILLUMINATION_BLOCKS;
    let fx = ((x as f32 + 0.5) * n as f32 / width as f32 - 0.5).clamp(0.0, n as f32 - 1.0);
    let fy = ((y as f32 + 0.5) * n as f32 / height as f32 - 0.5).clamp(0.0, n as f32 - 1.0);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(n - 1), (y0 + 1).min(n - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);

    let top = field[y0 * n + x0] * (1.0 - tx) + field[y0 * n + x1] * tx;
    let bottom = field[y1 * n + x0] * (1.0 - tx) + field[y1 * n + x1] * tx;
    top * (1.0 - ty) + bottom * ty
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
        MatteMethod::PaperRelative { margin, feather } => {
            let paper = illumination_field(image);
            // The configured values are floors. Noisy paper needs a wider
            // margin than any constant could know in advance.
            let margin = margin.max(paper.grain * 1.3);
            let feather = feather.max(paper.grain * 0.8);
            let mut out = image.clone();
            for y in 0..image.height {
                for x in 0..image.width {
                    let p = (y as usize * image.width as usize + x as usize) * 4;
                    let l = luminance([image.pixels[p], image.pixels[p + 1], image.pixels[p + 2]]);
                    let level =
                        sample_field(&paper.field, image.width, image.height, x, y).max(1e-3);
                    // How bright this pixel is relative to its own paper. At or
                    // above 1.0 it is paper; well below it is ink.
                    let relative = l / level;
                    let coverage = 1.0 - smoothstep(1.0 - margin - feather, 1.0 - margin, relative);
                    let existing = image.pixels[p + 3] as f32 / 255.0;
                    out.pixels[p + 3] =
                        ((coverage * existing) * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
            out
        }
    }
}

/// Share of the image's outer border still covered after matting, 0..1.
///
/// A cheap check on whether matting worked, and the one signal available for
/// the case it cannot fix. The illumination estimate is a plane, which follows
/// a sheet lit unevenly from one side but cannot represent a hard-edged cast
/// shadow — an object shadow falling across the paper, say, or a phone held
/// between the lamp and the page. Where that happens the shadowed region
/// survives as motif, and the giveaway is that the background reaches the edge
/// of the frame.
///
/// Artwork is photographed with a margin, so a correctly matted image retains
/// almost none of its border. Anything above a few percent means background
/// was kept, and the fix is a better photograph rather than a better threshold.
#[must_use]
pub fn border_retained(matted: &SourceImage, alpha_floor: u8) -> f32 {
    let (w, h) = (matted.width, matted.height);
    if w < 3 || h < 3 {
        return 0.0;
    }
    // A ring two pixels deep, so a single stray row cannot dominate.
    let depth = 2;
    let mut total = 0usize;
    let mut kept = 0usize;
    for y in 0..h {
        for x in 0..w {
            let on_border = x < depth || y < depth || x >= w - depth || y >= h - depth;
            if !on_border {
                continue;
            }
            total += 1;
            if matted.pixel(x, y)[3] > alpha_floor {
                kept += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        kept as f32 / total as f32
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
        // A motif may arrive partly cut out already. Matting must not
        // resurrect background the exporter had removed.
        //
        // The fixture used to be a two-pixel image of solid ink, which stopped
        // making sense once matting measured the paper rather than assuming a
        // brightness: with no paper in frame, the ink *is* the brightest thing
        // present and is correctly read as background. It needs real paper to
        // be a real test.
        let mut img = paper_with_a_mark();
        img.set_pixel(2, 1, [200, 30, 45, 0]);

        let out = matte(&img, MatteMethod::default());
        assert_eq!(out.pixel(2, 1)[3], 0, "an erased pixel must stay erased");
        assert_eq!(out.pixel(1, 1)[3], 255, "ink beside it is untouched");
    }

    #[test]
    fn an_image_with_no_paper_in_it_keeps_nothing() {
        // Worth stating rather than discovering. Relative matting asks what is
        // darker than its surroundings; given a frame of uniform colour the
        // honest answer is nothing, so everything reads as background.
        //
        // This is why artwork already cut out should be ingested with
        // ExistingAlpha. The pipeline fails loudly rather than quietly — trim
        // returns nothing and the library refuses the motif — which is the
        // right outcome, but the reason is not obvious from the error.
        let uniform = SourceImage::filled(8, 8, RED);
        let out = matte(&uniform, MatteMethod::default());
        assert!(out.pixels.chunks(4).all(|p| p[3] == 0));

        assert_eq!(
            matte(&uniform, MatteMethod::ExistingAlpha),
            uniform,
            "ExistingAlpha is the method for artwork that is already cut out"
        );
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
