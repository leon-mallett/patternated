//! Reading outlines out of an SVG.
//!
//! Not rendering it. The distinction matters: a renderer's job is to make this
//! file look as it was drawn, and ours is to find the shapes a press could put
//! ink into. Everything that is neither — a gradient, a bitmap dropped into
//! the drawing, a blur — has no palette slot to resolve to and is refused by
//! name, with what to do about it, rather than approximated into a print
//! nobody asked for.
//!
//! Only the geometry is taken. `usvg` parses and simplifies; `tiny-skia-path`
//! carries the outlines; nothing rasterises until a render knows what
//! resolution it wants. That is the whole point of keeping a motif vector.
//!
//! # Strokes become outlines here
//!
//! A stroke is a width around a line, which is a different kind of thing from
//! a filled shape, and carrying both to the renderer would mean two code paths
//! for the rest of the motif's life. Stroking is geometry, so it can be done
//! once at import and stay exact at any resolution.
//!
//! # Text is refused, and that is a determinism decision
//!
//! `usvg` is built here without its `text` feature. With it, type is resolved
//! through the fonts installed on *this machine*, so the same file would
//! rasterise differently elsewhere and the engine's central promise — same
//! input, same bytes, any platform — would be a claim it could no longer make.
//! Outlining text before export is one checkbox in every drawing program.

use pattern_core::{Color, Palette, Unprintable, VectorMotif, VectorShape};
use tiny_skia_path::{PathStroker, Stroke, Transform};

use crate::quantise::colour_distance_sq;
use crate::source::SourceImage;

/// Pixels per inch `usvg` resolves physical units against.
///
/// The CSS definition, and the one `usvg::Options` defaults to. It is here as
/// a name because it is also how a declared size is read back out: an SVG that
/// says it is 80 mm wide resolves to 302.36 units, and dividing by this is the
/// only way back.
const CSS_DPI: f32 = 96.0;

/// One outline as the drawing gives it, before any palette exists.
#[derive(Debug, Clone)]
pub struct RawShape {
    pub path: tiny_skia_path::Path,
    pub colour: Color,
    pub even_odd: bool,
    pub opacity: u8,
}

/// What reading an SVG produced.
///
/// Deliberately short of a motif. A motif carries palette slots, and the
/// palette is often derived *from* this drawing — so reading and quantising
/// are two steps rather than one. It also means changing a document's colour
/// count re-quantises a drawing exactly, from its own colours, rather than
/// from a rasterisation of them.
#[derive(Debug, Clone)]
pub struct SvgArt {
    pub view: tiny_skia_path::Rect,
    pub shapes: Vec<RawShape>,
    /// The size the drawing declares, in millimetres.
    ///
    /// A suggestion rather than an answer: a file that gives its size in
    /// pixels or as a bare `viewBox` still resolves to *some* number, and
    /// there is no way to tell afterwards which it was. The interface offers
    /// it as the default and lets it be overridden, which is what it does for
    /// artwork of every other kind anyway.
    pub declared_size_mm: (f32, f32),
    /// What had to be left out, with no duplicates.
    pub refused: Vec<Unprintable>,
}

/// Read an SVG into outlines, keeping the colours it was drawn in.
///
/// # Errors
///
/// If the file is not SVG this engine can parse, or if nothing printable
/// survives it.
pub fn read_svg(data: &[u8]) -> Result<SvgArt, SvgError> {
    let options = usvg::Options {
        // Named rather than left to the default so that the assumption is
        // visible where the millimetres come back out.
        dpi: CSS_DPI,
        ..usvg::Options::default()
    };
    let tree =
        usvg::Tree::from_data(data, &options).map_err(|e| SvgError::Unreadable(e.to_string()))?;

    let mut shapes = Vec::new();
    let mut refused = Vec::new();
    collect(tree.root(), &mut shapes, &mut refused);
    refused.sort_unstable_by_key(|u| format!("{u:?}"));
    refused.dedup();

    let size = tree.size();
    let view = tiny_skia_path::Rect::from_xywh(0.0, 0.0, size.width(), size.height())
        .ok_or(SvgError::NoSize)?;
    if shapes.iter().all(|s| s.opacity == 0) {
        return Err(SvgError::NothingToPrint(refused));
    }

    Ok(SvgArt {
        view,
        shapes,
        declared_size_mm: (
            size.width() / CSS_DPI * pattern_core::MM_PER_INCH,
            size.height() / CSS_DPI * pattern_core::MM_PER_INCH,
        ),
        refused,
    })
}

/// Walk the tree back to front, turning what can be printed into shapes and
/// noting by name what cannot.
fn collect(group: &usvg::Group, shapes: &mut Vec<RawShape>, refused: &mut Vec<Unprintable>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(inner) => {
                // A group that filters, clips or masks its children changes
                // what they are, so the outlines inside it are no longer what
                // gets drawn.
                if !inner.filters().is_empty()
                    || inner.clip_path().is_some()
                    || inner.mask().is_some()
                {
                    refused.push(Unprintable::NotTheShapeDescribed);
                    continue;
                }
                collect(inner, shapes, refused);
            }
            usvg::Node::Image(_) => refused.push(Unprintable::EmbeddedImage),
            usvg::Node::Text(_) => refused.push(Unprintable::LiveText),
            usvg::Node::Path(path) => {
                // No clip or mask check here: `usvg` normalises both onto
                // the enclosing group, which the arm above refuses.
                //
                // Absolute, because a path's outline is stored in whatever
                // space its groups left it in and the motif has only one.
                let at = path.abs_transform();

                if let Some(fill) = path.fill() {
                    match flat(fill.paint()) {
                        Some(colour) => shapes.push(RawShape {
                            path: placed(path.data(), at),
                            colour,
                            even_odd: fill.rule() == usvg::FillRule::EvenOdd,
                            opacity: coverage(fill.opacity()),
                        }),
                        None => refused.push(Unprintable::NotAFlatColour),
                    }
                }

                if let Some(stroke) = path.stroke() {
                    match flat(stroke.paint()) {
                        Some(colour) => {
                            if let Some(outlined) = outline(path.data(), stroke) {
                                shapes.push(RawShape {
                                    path: placed(&outlined, at),
                                    colour,
                                    // A stroke outline encloses the area
                                    // between its two edges, which is what
                                    // even-odd means and what non-zero would
                                    // fill solid.
                                    even_odd: true,
                                    opacity: coverage(stroke.opacity()),
                                });
                            }
                        }
                        None => refused.push(Unprintable::NotAFlatColour),
                    }
                }
            }
        }
    }
}

/// The single colour a paint is, if it is a single colour.
fn flat(paint: &usvg::Paint) -> Option<Color> {
    match paint {
        usvg::Paint::Color(c) => Some(Color::new(c.red, c.green, c.blue)),
        _ => None,
    }
}

fn coverage(opacity: usvg::Opacity) -> u8 {
    (opacity.get() * 255.0).round().clamp(0.0, 255.0) as u8
}

fn placed(path: &tiny_skia_path::Path, at: Transform) -> tiny_skia_path::Path {
    path.clone().transform(at).unwrap_or_else(|| path.clone())
}

/// Turn a stroke into the outline of the ink it lays down.
fn outline(path: &tiny_skia_path::Path, stroke: &usvg::Stroke) -> Option<tiny_skia_path::Path> {
    let mut props = Stroke {
        width: stroke.width().get(),
        miter_limit: stroke.miterlimit().get(),
        line_cap: match stroke.linecap() {
            usvg::LineCap::Butt => tiny_skia_path::LineCap::Butt,
            usvg::LineCap::Round => tiny_skia_path::LineCap::Round,
            usvg::LineCap::Square => tiny_skia_path::LineCap::Square,
        },
        line_join: match stroke.linejoin() {
            usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => tiny_skia_path::LineJoin::Miter,
            usvg::LineJoin::Round => tiny_skia_path::LineJoin::Round,
            usvg::LineJoin::Bevel => tiny_skia_path::LineJoin::Bevel,
        },
        dash: None,
    };
    if let Some(dash) = stroke.dasharray() {
        props.dash = tiny_skia_path::StrokeDash::new(dash.to_vec(), stroke.dashoffset());
    }
    PathStroker::new().stroke(path, &props, 1.0)
}

/// The longest edge, in pixels, a drawing is rasterised at when it cannot be
/// kept as outlines.
///
/// Generous, because this is the one representation the motif will have from
/// then on and the quantiser wants detail to work from. It is also the moment
/// the resolution independence is given up, so it is worth being obvious about
/// where the number is.
pub const FALLBACK_PX: u32 = 1024;

/// Draw an SVG as artwork, for the things outlines cannot carry.
///
/// A gradient has no single colour, so no palette slot can stand for it and
/// [`read_svg`] refuses it. Rendering it and quantising the pixels is exactly
/// what happens to a photograph, and what a designer would otherwise do by
/// hand — export a PNG and come back — so the tool does it instead.
///
/// The cost is the whole reason SVG was worth reading as geometry: this motif
/// is pixels from here on and will not sharpen when it is printed large. That
/// is a trade a caller should make deliberately, which is why it is a separate
/// function rather than something [`read_svg`] falls back to on its own.
///
/// # Errors
///
/// If the file cannot be parsed, or has no area to draw.
pub fn rasterise_svg(data: &[u8]) -> Result<SourceImage, SvgError> {
    let options = usvg::Options {
        dpi: CSS_DPI,
        ..usvg::Options::default()
    };
    let tree =
        usvg::Tree::from_data(data, &options).map_err(|e| SvgError::Unreadable(e.to_string()))?;

    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w > 0.0 && h > 0.0) {
        return Err(SvgError::NoSize);
    }
    let scale = FALLBACK_PX as f32 / w.max(h);
    let (px, py) = (
        ((w * scale).round() as u32).max(1),
        ((h * scale).round() as u32).max(1),
    );

    let mut canvas = tiny_skia::Pixmap::new(px, py).ok_or(SvgError::NoSize)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut canvas.as_mut(),
    );

    // Straight RGBA, because ingest reasons about colour independently of
    // coverage and a premultiplied edge would drag every rim toward black.
    let mut rgba = Vec::with_capacity(canvas.pixels().len() * 4);
    for p in canvas.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    SourceImage::new(px, py, rgba).ok_or(SvgError::NoSize)
}

impl SvgArt {
    /// Whether this drawing is better rendered than read as outlines.
    ///
    /// True when something was left out that a renderer would have drawn. A
    /// drawing with one gradient among twenty flat shapes reads as outlines
    /// perfectly well and is *wrong*: the gradient shape is simply missing,
    /// which is a worse failure than a motif that will not sharpen.
    #[must_use]
    pub fn needs_rendering(&self) -> bool {
        self.refused.iter().any(|k| k.helped_by_rendering())
    }

    /// Put this drawing onto a palette.
    ///
    /// Every colour goes to its nearest slot, by the comparison the bitmap
    /// path uses — so a drawing and a photograph of the same thing land on the
    /// same slots.
    #[must_use]
    pub fn onto(&self, palette: &Palette) -> VectorMotif {
        VectorMotif {
            view: self.view,
            shapes: self
                .shapes
                .iter()
                .map(|s| VectorShape {
                    path: s.path.clone(),
                    slot: nearest(s.colour, palette),
                    even_odd: s.even_odd,
                    opacity: s.opacity,
                })
                .collect(),
        }
    }

    /// The drawing rendered as colours to derive a palette from.
    ///
    /// Not a rasterisation: a list of the colours used, each repeated in
    /// proportion to how much of the drawing it covers, so that median cut
    /// weighs them the way it weighs a photograph's. Flat artwork has few
    /// colours and they are exact, which is the one place a drawing is a
    /// better source for a palette than a picture of one.
    ///
    /// Area is taken from each shape's bounding box. It overstates a thin
    /// diagonal and understates nothing, and the question being asked — which
    /// of these colours matter — does not turn on the difference.
    #[must_use]
    pub fn palette_sample(&self) -> SourceImage {
        let mut weighted: Vec<(Color, f64)> = Vec::new();
        for shape in self.shapes.iter().filter(|s| s.opacity > 0) {
            let b = shape.path.bounds();
            let area = f64::from(b.width().max(0.0)) * f64::from(b.height().max(0.0));
            match weighted.iter_mut().find(|(c, _)| *c == shape.colour) {
                Some((_, total)) => *total += area,
                None => weighted.push((shape.colour, area)),
            }
        }

        // A fixed budget of pixels shared out by area, so the sample is the
        // same size whatever the drawing and every colour present gets at
        // least one.
        const PIXELS: f64 = 4096.0;
        let total: f64 = weighted.iter().map(|(_, a)| *a).sum();
        let mut rgba = Vec::new();
        for (colour, area) in &weighted {
            let share = if total > 0.0 { area / total } else { 0.0 };
            let count = ((share * PIXELS).round() as usize).max(1);
            for _ in 0..count {
                rgba.extend_from_slice(&[colour.r, colour.g, colour.b, 255]);
            }
        }
        let pixels = rgba.len() / 4;
        SourceImage::new(pixels as u32, 1, rgba)
            .unwrap_or_else(|| SourceImage::new(1, 1, vec![0, 0, 0, 0]).expect("one pixel"))
    }
}

/// The palette slot nearest a colour.
///
/// The same comparison the bitmap path uses, so a drawing and a photograph of
/// the same thing land on the same slots.
fn nearest(colour: Color, palette: &Palette) -> u8 {
    let mut best = (pattern_core::GROUND_SLOT, f32::MAX);
    for (index, candidate) in palette.colors.iter().enumerate() {
        let d = colour_distance_sq(colour, *candidate);
        if d < best.1 {
            best = (index as u8, d);
        }
    }
    best.0
}

/// Why an SVG could not become a motif.
#[derive(Debug, thiserror::Error)]
pub enum SvgError {
    #[error("this file could not be read as SVG: {0}")]
    Unreadable(String),
    #[error("this drawing declares no size, so there is nothing to scale it by")]
    NoSize,
    #[error(
        "nothing in this drawing can be printed as flat colour{}",
        describe(.0)
    )]
    NothingToPrint(Vec<Unprintable>),
}

fn describe(refused: &[Unprintable]) -> String {
    if refused.is_empty() {
        return String::new();
    }
    let mut out = String::from(" — it ");
    for (i, kind) in refused.iter().enumerate() {
        if i > 0 {
            out.push_str("; and it ");
        }
        out.push_str(kind.advice());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pattern_core::Color;

    fn palette() -> Palette {
        Palette::new(vec![
            Color::new(250, 246, 238),
            Color::new(200, 30, 45),
            Color::new(60, 120, 55),
            Color::new(27, 27, 27),
        ])
    }

    fn svg(body: &str) -> Vec<u8> {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="80mm" height="60mm" viewBox="0 0 160 120">{body}</svg>"##
        )
        .into_bytes()
    }

    #[test]
    fn a_gradient_that_has_no_outline_can_still_be_drawn() {
        // The fallback's whole job. Reading this as outlines gives nothing —
        // a gradient has no single colour a palette slot could stand for — and
        // drawing it gives a picture that quantises like any other artwork.
        let data = svg(
            r##"<defs><linearGradient id="g"><stop stop-color="#c8202d"/>
                  <stop offset="1" stop-color="#3c7837"/></linearGradient></defs>
                <rect x="20" y="20" width="120" height="80" fill="url(#g)"/>"##,
        );
        assert!(read_svg(&data).is_err(), "nothing here is an outline");

        let drawn = rasterise_svg(&data).expect("but it can be drawn");
        assert_eq!(drawn.width.max(drawn.height), FALLBACK_PX);

        // The rectangle is three quarters of the drawing's width and two
        // thirds of its height, so about half of it should be opaque.
        let covered = (0..drawn.pixel_count())
            .filter(|i| drawn.pixels[i * 4 + 3] > 127)
            .count() as f32
            / drawn.pixel_count() as f32;
        assert!(
            (0.4..0.6).contains(&covered),
            "the drawing came out {:.0}% opaque",
            covered * 100.0
        );

        // And it is a gradient rather than a flat fill: the two ends differ.
        let at = |x: u32, y: u32| {
            let i = (y * drawn.width + x) as usize * 4;
            (drawn.pixels[i], drawn.pixels[i + 1], drawn.pixels[i + 2])
        };
        let (left, right) = (
            at(drawn.width / 4, drawn.height / 2),
            at(drawn.width * 3 / 4, drawn.height / 2),
        );
        assert_ne!(left, right, "the gradient was drawn as one colour");
    }

    #[test]
    fn what_is_not_a_drawing_is_refused_by_both_routes() {
        // The fallback must not turn an unreadable file into a blank motif.
        // An empty `<svg>` is *not* the case to test here — `usvg` gives one a
        // nominal size and it draws as nothing, which is a drawing of nothing
        // rather than a failure.
        let rubbish = b"this is not a drawing";
        assert!(read_svg(rubbish).is_err());
        assert!(rasterise_svg(rubbish).is_err());
    }

    #[test]
    fn a_fill_becomes_a_shape_on_the_nearest_slot() {
        // The whole of the happy path: geometry kept, colour resolved to a
        // slot, so a colourway recolours a drawing exactly as it recolours a
        // photograph.
        let art = read_svg(&svg(
            r##"<rect x="10" y="10" width="60" height="40" fill="#c8202d"/>"##,
        ))
        .expect("should read");
        assert_eq!(art.shapes.len(), 1);
        assert_eq!(art.onto(&palette()).shapes[0].slot, 1, "the red slot");
        assert_eq!(art.shapes[0].opacity, 255);
        assert!(art.refused.is_empty());
    }

    #[test]
    fn a_stroke_becomes_an_outline_of_its_own() {
        // A stroke is a width around a line. Converting it here means one kind
        // of thing downstream, and it stays exact at any resolution because it
        // is done on geometry rather than on pixels.
        let art = read_svg(&svg(
            r##"<path d="M10 60 L150 60" stroke="#1b1b1b" stroke-width="8" fill="none"/>"##,
        ))
        .expect("should read");
        assert_eq!(art.shapes.len(), 1, "one shape, from the stroke");
        assert_eq!(
            art.onto(&palette()).shapes[0].slot,
            3,
            "the near-black slot"
        );

        // The outline encloses the ink, where the original path had no height
        // at all.
        //
        // Measured as a fraction of the view rather than in the numbers the
        // file used, because `usvg` resolves the viewBox into the geometry:
        // this drawing is 160 units wide in its own terms and 302 in the ones
        // the shapes come back in. The fraction is the same either way, and it
        // is the only thing about the number that means anything downstream.
        let bounds = art.shapes[0].path.bounds();
        let across = art.view.width();
        assert!(
            (bounds.height() / across - 8.0 / 160.0).abs() < 0.001,
            "a stroke of 8 in a 160-wide drawing came back as {} of {across}",
            bounds.height()
        );
    }

    #[test]
    fn a_shape_with_both_gets_both() {
        let art = read_svg(&svg(r##"<rect x="10" y="10" width="60" height="40" fill="#3c7837"  stroke="#1b1b1b" stroke-width="4"/>"##))
        .expect("should read");
        assert_eq!(art.shapes.len(), 2);
        assert_eq!(art.onto(&palette()).slots_used(), vec![2, 3]);
    }

    #[test]
    fn the_declared_size_comes_back_in_millimetres() {
        // Physical units throughout, including on the way in. A drawing that
        // says it is 80 mm wide should not arrive needing to be told.
        let art = read_svg(&svg(
            r##"<rect x="0" y="0" width="160" height="120" fill="#c8202d"/>"##,
        ))
        .expect("should read");
        assert!(
            (art.declared_size_mm.0 - 80.0).abs() < 0.01,
            "{:?}",
            art.declared_size_mm
        );
        assert!((art.declared_size_mm.1 - 60.0).abs() < 0.01);
    }

    #[test]
    fn shapes_keep_the_position_their_groups_gave_them() {
        // A path's outline is stored in whatever space its groups left it in,
        // and a motif has only one. Getting this wrong stacks every shape at
        // the origin, which looks like a drawing with its parts fallen off.
        let art = read_svg(&svg(r##"<g transform="translate(40,30)">
                     <rect x="0" y="0" width="20" height="20" fill="#c8202d"/>
                   </g>"##))
        .expect("should read");
        let bounds = art.shapes[0].path.bounds();
        let (across, down) = (art.view.width(), art.view.height());
        assert!(
            (bounds.left() / across - 40.0 / 160.0).abs() < 0.001
                && (bounds.top() / down - 30.0 / 120.0).abs() < 0.001,
            "the group's translation was lost: {bounds:?} in {across} x {down}"
        );
    }

    #[test]
    fn a_gradient_is_refused_by_name_rather_than_averaged() {
        // A gradient has no single colour to become. Quantising it to its
        // midpoint would print something nobody chose, and would look like a
        // decision rather than a fault.
        let art = read_svg(&svg(
            r##"<defs><linearGradient id="g"><stop offset="0" stop-color="#c8202d"/>
                     <stop offset="1" stop-color="#3c7837"/></linearGradient></defs>
                   <rect x="0" y="0" width="40" height="40" fill="url(#g)"/>
                   <rect x="60" y="0" width="40" height="40" fill="#c8202d"/>"##,
        ))
        .expect("the flat rectangle still prints");
        assert_eq!(art.shapes.len(), 1, "only the flat fill survives");
        assert_eq!(art.refused, vec![Unprintable::NotAFlatColour]);
    }

    #[test]
    fn a_drawing_with_nothing_printable_says_what_to_do() {
        // The error is the feature. Somebody meeting this is holding the wrong
        // export, and the fix is in the drawing program rather than here.
        let err = read_svg(&svg(
            r##"<defs><linearGradient id="g"><stop offset="0" stop-color="#000"/>
                     <stop offset="1" stop-color="#fff"/></linearGradient></defs>
                   <rect x="0" y="0" width="160" height="120" fill="url(#g)"/>"##,
        ))
        .expect_err("nothing here can be printed");
        let said = err.to_string();
        assert!(said.contains("gradient"), "{said}");
        assert!(said.contains("flat shapes"), "{said}");
    }

    #[test]
    fn opacity_is_carried_rather_than_folded_into_the_colour() {
        // The same separation a pixel makes: coverage is continuous, colour is
        // discrete. Folding a half-opaque red into the palette would look for
        // a pink that is not there and land somewhere arbitrary.
        let art = read_svg(&svg(
            r##"<rect x="0" y="0" width="40" height="40" fill="#c8202d" fill-opacity="0.5"/>"##,
        ))
        .expect("should read");
        assert_eq!(art.onto(&palette()).shapes[0].slot, 1, "still the red slot");
        assert!(
            (i16::from(art.shapes[0].opacity) - 128).abs() <= 1,
            "{}",
            art.shapes[0].opacity
        );
    }

    #[test]
    fn what_is_not_there_is_not_refused() {
        // A clean drawing should report nothing, or the warning becomes noise
        // that gets ignored when it matters.
        let art = read_svg(&svg(r##"<circle cx="80" cy="60" r="40" fill="#3c7837"/>"##))
            .expect("should read");
        assert!(art.refused.is_empty(), "{:?}", art.refused);
    }
}
