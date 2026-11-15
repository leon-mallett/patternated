//! A motif as outlines rather than pixels.
//!
//! The engine's other motif representation, [`IndexedMotif`](crate::IndexedMotif),
//! is a palette slot and a coverage value per pixel, and it is the right shape
//! for artwork that arrives as artwork: a drawing, a scan, a generated image.
//! Vector artwork arrives as the thing a bitmap is a sample of, and sampling it
//! once at import throws away the only reason it was vector: a motif drawn at
//! 20 mm and exported at 300 DPI wants 236 pixels, and the same motif on a
//! poster wants two thousand.
//!
//! So this is kept as geometry and rasterised per render, at the resolution
//! that render needs. Nothing downstream of the bank knows: it is handed an
//! `IndexedMotif` as it always was, made to measure.
//!
//! # Slots, not colours
//!
//! Each shape carries a palette slot, exactly as a pixel does, so a colourway
//! recolours vector and raster motifs by the same mechanism. It is cleaner
//! here than in a bitmap, because a fill is one flat colour by construction, so the
//! quantisation that a photograph needs is a lookup.
//!
//! # What a shape is not
//!
//! A filled outline and nothing else. Strokes are converted to outlines when
//! the artwork is read, so that everything after that point is one kind of
//! thing. Gradients, patterns, embedded images and filters have no slot to
//! resolve to and are refused at import rather than approximated. A gradient
//! quantised to its midpoint is a print nobody asked for.

use serde::{Deserialize, Serialize};
use tiny_skia_path::{Path, Rect};

use crate::palette::GROUND_SLOT;

/// One filled outline, in the motif's own coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorShape {
    pub path: Path,
    /// The palette slot this shape paints.
    pub slot: u8,
    /// Whether the outline is filled by the even-odd rule rather than
    /// non-zero. Kept because it changes what a self-intersecting path means,
    /// and a motif with a hole in it is a common thing to draw.
    pub even_odd: bool,
    /// Coverage this shape paints with, 0-255, from the artwork's own opacity.
    ///
    /// Separate from the slot for the same reason a pixel's alpha is: it is
    /// continuous where the slot is discrete, so a half-opaque shape stays
    /// half-opaque rather than being quantised to a different colour.
    pub opacity: u8,
}

/// A motif held as outlines, to be rasterised when a resolution is known.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorMotif {
    /// The artwork's own coordinate box. Shapes are positioned inside it.
    pub view: Rect,
    /// Back to front, as the artwork was drawn.
    pub shapes: Vec<VectorShape>,
}

impl VectorMotif {
    /// Every slot this motif needs a palette to define, ascending.
    ///
    /// Shapes with no coverage do not count, on the same reasoning as a
    /// transparent pixel: nothing printed is nothing charged for.
    #[must_use]
    pub fn slots_used(&self) -> Vec<u8> {
        let mut used: Vec<u8> = self
            .shapes
            .iter()
            .filter(|s| s.opacity > 0)
            .map(|s| s.slot)
            .collect();
        used.sort_unstable();
        used.dedup();
        used
    }

    /// The highest slot this motif needs.
    #[must_use]
    pub fn max_slot(&self) -> Option<u8> {
        self.slots_used().last().copied()
    }

    /// Whether this motif would print anything at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shapes.iter().all(|s| s.opacity == 0)
    }

    /// Share of the motif's shapes that resolve to the fabric colour.
    ///
    /// The vector counterpart of [`IndexedMotif::ground_share`][gs], and
    /// deliberately *not* the same measurement: that one counts pixels,
    /// because a bitmap has no other unit. Here the honest unit is area, and
    /// area of overlapping outlines is a harder question than it is worth
    /// answering before rasterising. Reported once the motif is rasterised
    /// instead; this says only whether any shape is on the fabric slot at all.
    ///
    /// [gs]: crate::IndexedMotif::ground_share
    #[must_use]
    pub fn paints_on_the_fabric_slot(&self) -> bool {
        self.shapes
            .iter()
            .any(|s| s.opacity > 0 && s.slot == GROUND_SLOT)
    }
}

/// What an SVG carried that this engine has no way to print.
///
/// Listed rather than collapsed into one error, because the answer a designer
/// needs differs: a gradient is a decision to make, an embedded bitmap is the
/// wrong file to have exported, and text is one checkbox in the drawing
/// program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Unprintable {
    /// A gradient or pattern fill: more than one colour in one shape, and no
    /// slot that could stand for it.
    NotAFlatColour,
    /// A bitmap placed inside the vector file.
    EmbeddedImage,
    /// Live text, which needs a font to have a shape at all, and the font
    /// installed on *this* machine, which is why it cannot be allowed.
    LiveText,
    /// A filter, clip or mask: the shape drawn is not the shape described.
    NotTheShapeDescribed,
}

impl Unprintable {
    /// Whether drawing the file instead of reading its outlines would recover
    /// this.
    ///
    /// A gradient has no outline a palette slot can stand for, and a renderer
    /// draws it perfectly well, so rasterising and quantising is a real
    /// answer, and the same one a designer would reach by exporting a PNG.
    ///
    /// The other two are not answers. Live text needs a font, and the renderer
    /// is built without font support on purpose, so rendering would drop it
    /// exactly as reading it did, and would trade the whole drawing's
    /// sharpness for nothing. An embedded bitmap is the same: this engine's
    /// renderer cannot decode one either, so falling back would lose the
    /// outlines and still not show the image.
    #[must_use]
    pub fn helped_by_rendering(self) -> bool {
        matches!(
            self,
            Unprintable::NotAFlatColour | Unprintable::NotTheShapeDescribed
        )
    }

    /// What to tell a designer, in terms of the drawing rather than the file
    /// format.
    #[must_use]
    pub fn advice(self) -> &'static str {
        match self {
            Unprintable::NotAFlatColour => {
                "uses a gradient or a pattern fill. A print is flat colours, and there is no \
                 single colour this could become. Replace it with flat shapes, or accept it \
                 as artwork and export a PNG instead."
            }
            Unprintable::EmbeddedImage => {
                "has a bitmap placed inside it. Export that part as a PNG and bring it in as \
                 its own motif, or expand it to outlines if it is traced artwork."
            }
            Unprintable::LiveText => {
                "has live text in it. Convert text to outlines before exporting: type rendered \
                 here would depend on the fonts installed on this machine, and the same file \
                 would print differently elsewhere."
            }
            Unprintable::NotTheShapeDescribed => {
                "uses a filter, clip or mask, so what is drawn is not the outline described. \
                 Flatten or expand it in the drawing program."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia_path::PathBuilder;

    fn shape(slot: u8, opacity: u8) -> VectorShape {
        let mut b = PathBuilder::new();
        b.push_rect(Rect::from_xywh(0.0, 0.0, 10.0, 10.0).expect("rect"));
        VectorShape {
            path: b.finish().expect("path"),
            slot,
            even_odd: false,
            opacity,
        }
    }

    fn motif(shapes: Vec<VectorShape>) -> VectorMotif {
        VectorMotif {
            view: Rect::from_xywh(0.0, 0.0, 10.0, 10.0).expect("rect"),
            shapes,
        }
    }

    #[test]
    fn an_invisible_shape_does_not_cost_a_colour() {
        // The same rule a fully transparent pixel follows, for the same
        // reason: nothing printed is nothing charged for.
        let m = motif(vec![shape(1, 255), shape(2, 0), shape(1, 128)]);
        assert_eq!(m.slots_used(), vec![1]);
        assert_eq!(m.max_slot(), Some(1));
    }

    #[test]
    fn a_motif_of_invisible_shapes_is_empty() {
        assert!(motif(vec![shape(1, 0)]).is_empty());
        assert!(!motif(vec![shape(1, 1)]).is_empty());
    }

    #[test]
    fn ink_the_colour_of_the_fabric_is_reported() {
        assert!(motif(vec![shape(GROUND_SLOT, 255)]).paints_on_the_fabric_slot());
        assert!(!motif(vec![shape(GROUND_SLOT, 0)]).paints_on_the_fabric_slot());
        assert!(!motif(vec![shape(3, 255)]).paints_on_the_fabric_slot());
    }

    #[test]
    fn only_what_a_renderer_could_draw_is_worth_rendering_for() {
        // Falling back to a rasterisation costs the drawing its sharpness, so
        // it is only worth it where the renderer can actually recover what the
        // outlines could not carry. Text and an embedded bitmap would come out
        // just as missing, having given that up for nothing.
        assert!(Unprintable::NotAFlatColour.helped_by_rendering());
        assert!(Unprintable::NotTheShapeDescribed.helped_by_rendering());
        assert!(!Unprintable::LiveText.helped_by_rendering());
        assert!(!Unprintable::EmbeddedImage.helped_by_rendering());
    }

    #[test]
    fn every_refusal_says_what_to_do_about_it() {
        // The message is the feature. A designer meeting one of these is
        // holding the wrong export, and the fix is in the drawing program.
        for kind in [
            Unprintable::NotAFlatColour,
            Unprintable::EmbeddedImage,
            Unprintable::LiveText,
            Unprintable::NotTheShapeDescribed,
        ] {
            let advice = kind.advice();
            assert!(advice.len() > 60, "{kind:?} says too little: {advice}");
            assert!(
                !advice.contains("SVG") && !advice.contains("usvg"),
                "{kind:?} talks about the file format rather than the drawing"
            );
        }
    }
}
