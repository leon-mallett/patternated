use pattern_core::{Color, IndexedMotif, Palette};
use serde::{Deserialize, Serialize};
use visioncortex::clusters::Cluster;
use visioncortex::{BinaryImage, PathSimplifyMode, PointI32};

/// One traced outline, tagged with the palette slot that fills it.
///
/// Slot rather than colour, for the same reason the raster carries slots: a
/// vector motif has to recolour with a colourway like everything else, and an
/// SVG with `fill="#c81e2d"` baked into it does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotPath {
    pub slot: u8,
    /// SVG path data — the `d` attribute, without the element around it.
    pub d: String,
}

/// A motif as outlines rather than pixels.
///
/// Kept *alongside* the raster, never instead of it. Some illustration styles —
/// watercolour, grain, anything with texture — vectorise badly, and the spec is
/// explicit that those should stay raster with a high-resolution original. The
/// pipeline therefore always produces both and lets the choice happen at
/// export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorMotif {
    pub width: u32,
    pub height: u32,
    pub paths: Vec<SlotPath>,
}

impl VectorMotif {
    /// Total number of traced outlines.
    ///
    /// A blunt measure of how complicated a motif is, and the one the style
    /// coherence work at M7 will normalise on: a dinosaur traced to 400 paths
    /// beside a football traced to 12 will never sit right together.
    #[must_use]
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Render to standalone SVG, resolving slots through a palette.
    ///
    /// Colour enters here and nowhere earlier.
    #[must_use]
    pub fn to_svg(&self, palette: &Palette) -> String {
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" \
             height=\"{}\">",
            self.width, self.height, self.width, self.height
        );
        for path in &self.paths {
            let Color { r, g, b } = palette.resolve(path.slot).unwrap_or(Color::new(0, 0, 0));
            svg.push_str(&format!(
                "<path fill=\"#{r:02x}{g:02x}{b:02x}\" d=\"{}\"/>",
                path.d
            ));
        }
        svg.push_str("</svg>");
        svg
    }
}

/// Tracing controls.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VectoriseOptions {
    /// Absolute floor on the area of a kept outline, in pixels.
    ///
    /// Quantisation leaves specks at colour boundaries. Traced, each becomes
    /// its own path, and a motif acquires outlines nobody can see and every
    /// downstream tool has to carry.
    pub min_area_px: usize,
    /// Floor on the area of a kept outline, as a share of the whole motif.
    ///
    /// The absolute floor alone was not enough, and the case that showed it is
    /// worth keeping: a lemon quantised onto a palette with no yellow
    /// fragmented along every segment boundary and traced to 172 outlines at
    /// 763 x 763, because eight pixels is nothing at that size. A threshold
    /// that does not scale with the motif means the same artwork behaves
    /// differently at different scan resolutions, which is its own bug.
    pub min_area_fraction: f32,
    /// Corner detection angle, in degrees.
    pub corner_threshold_deg: f64,
    /// Shortest segment the simplifier will keep.
    pub segment_length: f64,
    /// How aggressively adjacent curve segments are spliced into one.
    pub splice_threshold_deg: f64,
    /// Fit curves rather than emitting straight-edged polygons.
    pub smooth: bool,
}

impl Default for VectoriseOptions {
    fn default() -> Self {
        // visioncortex's own defaults, which are tuned for exactly this job:
        // flat-coloured illustration rather than photographs.
        Self {
            min_area_px: 8,
            min_area_fraction: 0.0004,
            corner_threshold_deg: 60.0,
            segment_length: 4.0,
            splice_threshold_deg: 45.0,
            smooth: true,
        }
    }
}

/// Trace an already-quantised motif into per-slot outlines.
///
/// Works from the indexed raster rather than from the original artwork, which
/// is the whole reason this step is simple. Tracing normally has to *discover*
/// regions of similar colour — the hard, parameter-heavy part of any
/// vectoriser. Quantisation has already decided them, so each slot is traced as
/// a plain binary mask and the result cannot disagree with the raster about
/// what colour anything is.
#[must_use]
pub fn vectorise(motif: &IndexedMotif, options: VectoriseOptions) -> VectorMotif {
    let mode = if options.smooth {
        PathSimplifyMode::Spline
    } else {
        PathSimplifyMode::Polygon
    };

    let area = motif.width as usize * motif.height as usize;
    let min_area = options
        .min_area_px
        .max((area as f32 * options.min_area_fraction) as usize);

    let mut paths = Vec::new();
    for slot in motif.slots_used() {
        let mut mask = BinaryImage::new_w_h(motif.width as usize, motif.height as usize);
        for y in 0..motif.height {
            for x in 0..motif.width {
                let i = y as usize * motif.width as usize + x as usize;
                // Half coverage is the boundary: a feathered edge has to land
                // on one side or the other, and the midpoint keeps the traced
                // outline where the eye reads the edge.
                if motif.slots[i] == slot && motif.alpha[i] >= 128 {
                    mask.set_pixel(x as usize, y as usize, true);
                }
            }
        }

        for cluster in mask.to_clusters(false).iter() {
            if cluster.size() < min_area {
                continue;
            }
            let compound = Cluster::image_to_compound_path(
                &PointI32 {
                    x: cluster.rect.left,
                    y: cluster.rect.top,
                },
                &cluster.to_binary_image(),
                mode,
                options.corner_threshold_deg.to_radians(),
                options.segment_length,
                10,
                options.splice_threshold_deg.to_radians(),
            );
            let (d, _) = compound.to_svg_string(true, PointI32 { x: 0, y: 0 }, Some(2));
            if !d.trim().is_empty() {
                paths.push(SlotPath { slot, d });
            }
        }
    }
    VectorMotif {
        width: motif.width,
        height: motif.height,
        paths,
    }
}
